//! Middleware to check if a builder is allowed to sign a block.

use crate::perms::Builders;
use axum::{
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use core::fmt;
use serde::Serialize;
use std::{future::Future, pin::Pin, sync::Arc};
use tower::{Layer, Service};
use tracing::{error, info};

/// Possible API error responses when a builder permissioning check fails.
#[derive(Serialize)]
struct ApiError {
    /// The error itself.
    error: &'static str,
    /// A human-readable message describing the error.
    message: &'static str,
}

impl ApiError {
    /// API error for missing authentication header.
    const fn missing_header() -> (StatusCode, Json<Self>) {
        (
            StatusCode::UNAUTHORIZED,
            Json(ApiError {
                error: "MISSING_AUTH_HEADER",
                message: "Missing authentication header",
            }),
        )
    }

    /// API error for invalid header encoding.
    const fn invalid_encoding() -> (StatusCode, Json<Self>) {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "INVALID_HEADER_ENCODING",
                message: "Invalid header encoding",
            }),
        )
    }

    /// API error for permission denied.
    const fn permission_denied() -> (StatusCode, Json<Self>) {
        (
            StatusCode::FORBIDDEN,
            Json(ApiError {
                error: "PERMISSION_DENIED",
                message: "Builder permission denied",
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
            );

            info!("builder permissioning check started");

            // Check if the sub is in the header.
            let sub = match req.headers().get("x-jwt-claim-sub") {
                Some(header_value) => match header_value.to_str() {
                    Ok(sub) => {
                        span.record("builder", sub);
                        sub
                    }
                    Err(_) => {
                        error!("builder request has invalid header encoding");
                        return Ok(ApiError::invalid_encoding().into_response());
                    }
                },
                None => {
                    error!("builder request missing header");
                    return Ok(ApiError::missing_header().into_response());
                }
            };

            if let Err(err) = this.builders.is_builder_permissioned(sub) {
                info!(%err, %sub, "permission denied");
                return Ok(ApiError::permission_denied().into_response());
            }

            info!(%sub, current_slot = %this.builders.calc().current_slot(), "builder permissioned successfully");

            this.inner.call(req).await
        })
    }
}
