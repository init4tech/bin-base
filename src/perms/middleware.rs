//! Middleware to check if a builder is allowed to sign a block.
//! Implemented as a [`tower::Layer`] and [`tower::Service`],
//! which can be used in an Axum application to enforce builder permissions
//! based on the current slot and builder configuration.

use crate::perms::Builders;
use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use core::fmt;
use serde::Serialize;
use std::{future::Future, pin::Pin, sync::Arc};
use tower::{Layer, Service};
use tracing::info;

/// Possible API error responses when a builder permissioning check fails.
#[derive(Serialize)]
struct ApiError {
    /// The error itself.
    error: &'static str,
    /// A human-readable message describing the error.
    message: &'static str,
    /// A human-readable hint for the error, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<&'static str>,
}

impl ApiError {
    /// API error for missing authentication header.
    const fn missing_header() -> (StatusCode, Json<ApiError>) {
        (
            StatusCode::UNAUTHORIZED,
            Json(ApiError {
                error: "MISSING_AUTH_HEADER",
                message: "Missing authentication header",
                hint: Some("Please provide the 'x-jwt-claim-sub' header with your JWT claim sub."),
            }),
        )
    }

    const fn invalid_encoding() -> (StatusCode, Json<ApiError>) {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "INVALID_ENCODING",
                message: "Invalid encoding in header value",
                hint: Some("Ensure the 'x-jwt-claim-sub' header is properly encoded."),
            }),
        )
    }

    const fn header_empty() -> (StatusCode, Json<ApiError>) {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "EMPTY_HEADER",
                message: "Empty header value",
                hint: Some("Ensure the 'x-jwt-claim-sub' header is not empty."),
            }),
        )
    }

    /// API error for permission denied.
    const fn permission_denied(hint: Option<&'static str>) -> (StatusCode, Json<ApiError>) {
        (
            StatusCode::FORBIDDEN,
            Json(ApiError {
                error: "PERMISSION_DENIED",
                message: "Builder permission denied",
                hint,
            }),
        )
    }
}

/// A middleware layer that can check if a builder is allowed to perform an action
/// during the current request.
///
/// Contains a pointer to the [`Builders`] struct, which holds the configuration and
/// builders for the permissioning system.
#[derive(Clone)]
pub struct BuilderPermissioningLayer {
    /// The configured builders.
    builders: Arc<Builders>,
}

impl BuilderPermissioningLayer {
    /// Create a new `BuilderPermissioningLayer` with the given builders.
    pub const fn new(builders: Arc<Builders>) -> Self {
        Self { builders }
    }
}

impl fmt::Debug for BuilderPermissioningLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuilderPermissioningLayer").finish()
    }
}

impl<S> Layer<S> for BuilderPermissioningLayer {
    type Service = BuilderPermissioningService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        BuilderPermissioningService {
            inner,
            builders: self.builders.clone(),
        }
    }
}

/// A service that checks if a builder is allowed to perform an action during the
/// current request.
///
/// Contains a pointer to the [`Builders`] struct, which holds the configuration and
/// builders for the permissioning system. Meant to be nestable and cheaply cloneable.
#[derive(Clone)]
pub struct BuilderPermissioningService<S> {
    inner: S,
    builders: Arc<Builders>,
}

impl<S> BuilderPermissioningService<S> {
    /// Create a new `BuilderPermissioningService` with the given inner service and builders.
    pub const fn new(inner: S, builders: Arc<Builders>) -> Self {
        Self { inner, builders }
    }
}

impl fmt::Debug for BuilderPermissioningService<()> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuilderPermissioningService").finish()
    }
}

impl<S> Service<Request> for BuilderPermissioningService<S>
where
    S: Service<Request, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let mut this = self.clone();

        Box::pin(async move {
            let span = tracing::info_span!(
                "builder::permissioning",
                builder = tracing::field::Empty,
                permissioned_builder = this.builders.current_builder().sub(),
                current_slot = this.builders.calc().current_slot(),
                permissioning_error = tracing::field::Empty,
            );

            let guard = span.enter();

            info!("builder permissioning check started");

            // Check if the sub is in the header.
            let sub = match validate_header_sub(req.headers().get("x-jwt-claim-sub")) {
                Ok(sub) => sub,
                Err(err) => {
                    info!(api_err = %err.1.message, "permission denied");
                    span.record("permissioning_error", err.1.message);
                    return Ok(err.into_response());
                }
            };

            if let Err(err) = this.builders.is_builder_permissioned(sub) {
                info!(api_err = %err, "permission denied");
                span.record("permissioning_error", err.to_string());

                let hint = builder_permissioning_hint(&err);

                return Ok(ApiError::permission_denied(hint).into_response());
            }

            info!("builder permissioned successfully");

            drop(guard);

            this.inner.call(req).await
        })
    }
}

fn validate_header_sub(sub: Option<&HeaderValue>) -> Result<&str, (StatusCode, Json<ApiError>)> {
    let Some(sub) = sub else {
        return Err(ApiError::missing_header());
    };

    let Some(sub) = sub.to_str().ok() else {
        return Err(ApiError::invalid_encoding());
    };

    if sub.is_empty() {
        return Err(ApiError::header_empty());
    }

    Ok(sub)
}

const fn builder_permissioning_hint(
    err: &crate::perms::BuilderPermissionError,
) -> Option<&'static str> {
    match err {
        crate::perms::BuilderPermissionError::ActionAttemptTooEarly => {
            Some("Action attempted too early in the slot.")
        }
        crate::perms::BuilderPermissionError::ActionAttemptTooLate => {
            Some("Action attempted too late in the slot.")
        }
        crate::perms::BuilderPermissionError::NotPermissioned => {
            Some("Builder is not permissioned for this slot.")
        }
    }
}
