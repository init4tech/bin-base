use crate::utils::otlp::{OtelConfig, OtelGuard};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Init tracing, returning an optional guard for the OTEL provider.
///
/// If the OTEL environment variables are not set, this function will
/// initialize a basic tracing subscriber with a `fmt` layer. If the
/// environment variables are set, it will initialize the OTEL provider
/// with the specified configuration, as well as the `fmt` layer.
///
/// See [`OtelConfig`] documentation for env var information.
///
/// ## Panics
///
/// This function will panic if a global subscriber has already been set.
///
/// [`OtelConfig`]: crate::utils::otlp::OtelConfig
pub fn init_tracing() -> Option<OtelGuard> {
    let registry = tracing_subscriber::registry().with(tracing_subscriber::fmt::layer());

    if let Some(cfg) = OtelConfig::load() {
        let guard = cfg.provider();
        registry.with(guard.layer()).init();
        Some(guard)
    } else {
        registry.init();
        None
    }
}
