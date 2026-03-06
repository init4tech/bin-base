//! This example allows you to play with ajj instrumentation.
//!
//! ## Observing traces
//!
//! We recommend the following:
//! - set `RUST_LOG=info` (or `trace` for more detail) to see log lines
//! - use [otel-desktop-viewer](https://github.com/CtrlSpice/otel-desktop-viewer)
//!
//! ## Running this example
//!
//! ```no_compile
//! export OTEL_EXPORTER_OTLP_ENDPOINT="http://localhost:4318"
//! export OTEL_TRACES_EXPORTER="otlp"
//! export OTEL_EXPORTER_OTLP_PROTOCOL="http/protobuf"
//! export RUST_LOG=info
//! cargo run --example ajj
//! ```
//!
//! ```no_compile
//! curl -X POST \
//!     -H 'Content-Type: application/json' \
//!      -d '{"jsonrpc":"2.0","id":"id","method":"helloWorld"}' \
//!      http://localhost:8080/rpc
//! ```
use ajj::Router;
use init4_bin_base::{
    utils::{from_env::FromEnv, metrics::MetricsConfig, tracing::TracingConfig},
    Init4Config,
};

#[derive(Debug, FromEnv)]
struct Config {
    tracing: TracingConfig,
    metrics: MetricsConfig,
}

impl Init4Config for Config {
    fn tracing(&self) -> &TracingConfig {
        &self.tracing
    }
    fn metrics(&self) -> &MetricsConfig {
        &self.metrics
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _config_and_guard = init4_bin_base::init::<Config>()?;

    let router = Router::<()>::new()
        .route("helloWorld", || async {
            tracing::info!("serving hello world");
            Ok::<_, ()>("Hello, world!")
        })
        .route("addNumbers", |(a, b): (u32, u32)| async move {
            tracing::info!("serving addNumbers");
            Ok::<_, ()>(a + b)
        })
        .into_axum("/rpc");

    let listener = tokio::net::TcpListener::bind("localhost:8080")
        .await
        .unwrap();
    axum::serve(listener, router).await.unwrap();
    Ok(())
}
