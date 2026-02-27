use crate::utils::{
    from_env::FromEnvVar,
    otlp::{OtelConfig, OtelGuard},
};
use tracing_subscriber::{filter::EnvFilter, layer::SubscriberExt, util::SubscriberInitExt, Layer};

const TRACING_LOG_JSON: &str = "TRACING_LOG_JSON";
const TRACING_WITH_FILE_AND_LINE_NO: &str = "TRACING_WITH_FILE_AND_LINE_NO";

/// Install a format layer based on the `TRACING_LOG_JSON` and
/// `TRACING_WITH_FILE_AND_LINE_NO` environment variables, and then install
/// the registry.
macro_rules! install_fmt {
    (json @ $registry:ident, $filter:ident, $file_line:expr) => {{
        let fmt = tracing_subscriber::fmt::layer()
            .json()
            .with_span_list(true)
            .with_current_span(false)
            .with_file($file_line)
            .with_line_number($file_line)
            .with_filter($filter);
        $registry.with(fmt).init();
    }};
    (log @ $registry:ident, $filter:ident, $file_line:expr) => {{
        let fmt = tracing_subscriber::fmt::layer()
            .with_file($file_line)
            .with_line_number($file_line)
            .with_filter($filter);
        $registry.with(fmt).init();
    }};
    ($registry:ident, $filter:ident) => {{
        let json = bool::from_env_var(TRACING_LOG_JSON).unwrap_or(false);
        let file_line = bool::from_env_var(TRACING_WITH_FILE_AND_LINE_NO).unwrap_or(false);
        if json {
            install_fmt!(json @ $registry, $filter, file_line);
        } else {
            install_fmt!(log @ $registry, $filter, file_line);
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
/// - `TRACING_LOG_JSON` - If set to a non-empty value, will enable JSON logging.
/// - `TRACING_WITH_FILE_AND_LINE_NO` - If set to a non-empty value, will include file names and
///   line numbers in tracing output.
/// - See [`OtelConfig`] documentation for env var information.
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
        let (guard, layer) = cfg.into_guard_and_layer();
        let registry = registry.with(layer);
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
