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
//!
//! ```no_compile
//! curl -X POST \
//!     -H 'Content-Type: application/json' \
//!      -d '{"jsonrpc":"2.0","id":"id","method":"helloWorld"}' \
//!      http://localhost:8080/rpc
//! ```
use ajj::Router;
use init4_bin_base::init4;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = init4();

    let router = Router::<()>::new()
        .route("helloWorld", || async {
            tracing::info!("serving hello world");
            Ok::<_, ()>("Hello, world!")
        })
        .into_axum("/rpc");

    let listener = tokio::net::TcpListener::bind("localhost:8080")
        .await
        .unwrap();
    axum::serve(listener, router).await.unwrap();
    Ok(())
}
