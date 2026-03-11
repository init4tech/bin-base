use crate::utils::{
    from_env::{FromEnv, OptionalBoolWithDefault},
    otlp::{OtelConfig, OtelGuard},
};
use tracing_subscriber::{filter::EnvFilter, layer::SubscriberExt, util::SubscriberInitExt, Layer};

/// Tracing format configuration.
///
/// Uses the following environment variables:
/// - `TRACING_LOG_JSON` - optional. If `true`, enables JSON logging. Defaults to `false`.
/// - `TRACING_WITH_FILE_AND_LINE_NO` - optional. If `true`, includes file names and line numbers
///   in tracing output. Defaults to `false`.
#[derive(Debug, Clone, Default, FromEnv)]
#[non_exhaustive]
#[from_env(crate)]
pub struct TracingConfig {
    /// Whether to log in JSON or not.
    #[from_env(
        var = "TRACING_LOG_JSON",
        desc = "If non-empty, log in JSON format [default: disabled]",
        optional
    )]
    pub log_json: OptionalBoolWithDefault<false>,

    /// Whether to include file names and line numbers in log output.
    #[from_env(
        var = "TRACING_WITH_FILE_AND_LINE_NO",
        desc = "If non-empty, include file names and line numbers in tracing output [default: disabled]",
        optional
    )]
    pub with_file_and_line_number: OptionalBoolWithDefault<false>,

    /// OTEL configuration.
    pub otel_config: Option<OtelConfig>,
}

/// Install a format layer and the registry.
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
    ($registry:ident, $filter:ident, $cfg:expr) => {{
        let file_line = $cfg.with_file_and_line_number.into_inner();
        if $cfg.log_json.into_inner() {
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
/// See [`TracingConfig`] and [`OtelConfig`] for env var information.
///
/// ## Panics
///
/// This function will panic if a global subscriber has already been set.
///
/// [`OtelConfig`]: crate::utils::otlp::OtelConfig
pub fn init_tracing() -> Option<OtelGuard> {
    let tracing_config = TracingConfig::from_env().unwrap();
    init_tracing_with_config(tracing_config)
}

pub(crate) fn init_tracing_with_config(tracing_config: TracingConfig) -> Option<OtelGuard> {
    let registry = tracing_subscriber::registry();
    let filter = EnvFilter::from_default_env();

    if let Some(otel_config) = tracing_config.otel_config {
        let (guard, layer) = otel_config.into_guard_and_layer();
        let registry = registry.with(layer);
        install_fmt!(registry, filter, tracing_config);
        Some(guard)
    } else {
        install_fmt!(registry, filter, tracing_config);
        tracing::debug!(
            "No OTEL config found or error while loading otel config, using default tracing"
        );
        None
    }
}
