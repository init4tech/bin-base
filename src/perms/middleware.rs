//! Middleware to check if a builder is allowed to sign a block.

use crate::perms::Builders;
use axum::{
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use core::fmt;
use std::{future::Future, pin::Pin, sync::Arc};
use tower::{Layer, Service};

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
            // Check if the sub is in the header.
            let sub = if let Some(sub) = req.headers().get("x-jwt-claim-sub") {
                // If so, attempt to convert it to a string.
                match sub.to_str() {
                    Ok(sub) => sub,
                    Err(_) => {
                        return Ok((StatusCode::BAD_REQUEST, "invalid header").into_response());
                    }
                }
            } else {
                return Ok((StatusCode::UNAUTHORIZED, "missing sub header").into_response());
            };

            if let Err(err) = this.builders.is_builder_permissioned(sub) {
                return Ok((StatusCode::FORBIDDEN, err.to_string()).into_response());
            }

            this.inner.call(req).await
        })
    }
}
