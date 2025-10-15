use crate::utils::{
    from_env::FromEnvVar,
    otlp::{OtelConfig, OtelGuard},
};
use tracing_subscriber::{filter::EnvFilter, layer::SubscriberExt, util::SubscriberInitExt, Layer};

const TRACING_LOG_JSON: &str = "TRACING_LOG_JSON";

/// Install a format layer based on the `TRACING_LOG_JSON` environment
/// variable, and then install the registr
///
macro_rules! install_fmt {
    (json @ $registry:ident, $filter:ident) => {{
        let fmt = tracing_subscriber::fmt::layer().json().with_filter($filter);
        $registry.with(fmt).init();
    }};
    (log @ $registry:ident, $filter:ident) => {{
        let fmt = tracing_subscriber::fmt::layer().with_filter($filter);
        $registry.with(fmt).init();
    }};
    ($registry:ident, $filter:ident) => {{
        let json = bool::from_env_var(TRACING_LOG_JSON).unwrap_or(false);
        if json {
            install_fmt!(json @ $registry, $filter);
        } else {
            install_fmt!(log @ $registry, $filter);
        }
    }};
}

/// Init tracing, returning an optional guard for the OTEL provider.
///
/// If the OTEL environment variables are not set, this function will
/// initialize a basic tracing subscriber with a `fmt` layer. If the
/// environment variables are set, it will initialize the OTEL provider
/// with the specified configuration, as well as the `fmt` layer.
///
/// ## Env Reads
///
/// - `TRACING_LOG_JSON` - If set, will enable JSON logging.
/// - As [`OtelConfig`] documentation for env var information.
///
/// ## Panics
///
/// This function will panic if a global subscriber has already been set.
///
/// [`OtelConfig`]: crate::utils::otlp::OtelConfig
pub fn init_tracing() -> Option<OtelGuard> {
    let registry = tracing_subscriber::registry();
    let filter = EnvFilter::from_default_env();

    if let Some(cfg) = OtelConfig::load() {
        let guard = cfg.provider();
        let registry = registry.with(guard.layer());
        install_fmt!(registry, filter);
        Some(guard)
    } else {
        install_fmt!(registry, filter);
        tracing::debug!(
            "No OTEL config found or error while loading otel config, using default tracing"
        );
        None
    }
}
