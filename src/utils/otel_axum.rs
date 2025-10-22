use axum::extract::{MatchedPath, Request};
use tower::{Layer, Service};
use tracing::{info_span, instrument::Instrumented, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// A [`Layer`] that adds OpenTelemetry spans to Axum requests.
#[derive(Debug, Clone, Copy)]
pub struct OtelAxumSpanLayer;

/// A simple service
#[derive(Debug, Clone)]
pub struct OtelAxumSpanner<S> {
    inner: S,
}

impl<S> Layer<S> for OtelAxumSpanLayer {
    type Service = OtelAxumSpanner<S>;

    fn layer(&self, inner: S) -> Self::Service {
        OtelAxumSpanner { inner }
    }
}

impl<S, Body> Service<Request<Body>> for OtelAxumSpanner<S>
where
    S: Service<Request<Body>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Instrumented<S::Future>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let parent_context = opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.extract(&opentelemetry_http::HeaderExtractor(req.headers()))
        });

        let method = req.method().to_string();
        let uri = req.uri().clone();
        let route = req
            .extensions()
            .get::<MatchedPath>()
            .map(|r| r.as_str())
            .unwrap_or_else(|| uri.path());
        let name = format!("{method} {route}");
        let name = name.trim();

        let span = info_span!(
            "Http Request",
            "otel.name" = name,
            "otel.target" = name,
            "otel.kind" = "server",
            "http.request.method" = method,
            "url.path" = uri.path(),
            "url.scheme" = uri.scheme_str().unwrap_or(""),
            "http.route" = route,
            "http.response.status_code" = tracing::field::Empty,
        );
        span.set_parent(parent_context);

        self.inner.call(req).instrument(span)
    }
}
