use crate::utils::from_env::{EnvItemInfo, FromEnv, FromEnvErr, FromEnvVar};
use opentelemetry::{trace::TracerProvider, KeyValue};
use opentelemetry_sdk::{trace::SdkTracerProvider, Resource};
use opentelemetry_semantic_conventions::{
    attribute::{DEPLOYMENT_ENVIRONMENT_NAME, SERVICE_NAME, SERVICE_VERSION},
    SCHEMA_URL,
};
use tracing_subscriber::{EnvFilter, Layer};
use url::Url;

const OTEL_ENDPOINT: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";
const OTEL_LEVEL: &str = "OTEL_LEVEL";
const OTEL_ENVIRONMENT: &str = "OTEL_ENVIRONMENT_NAME";

/// Drop guard for the Otel provider. This will shutdown the provider when
/// dropped, and generally should be held for the lifetime of the `main`
/// function.
///
/// ```
/// # use init4_bin_base::utils::otlp::{OtelConfig, OtelGuard};
/// # fn test() {
/// use init4_bin_base::utils::from_env::FromEnv;
/// fn main() {
///     let cfg = OtelConfig::from_env().unwrap();
///     let guard = cfg.provider();
///     // do stuff
///     // drop the guard when the program is done
/// }
/// # }
/// ```
#[derive(Debug)]
pub struct OtelGuard(SdkTracerProvider, EnvFilter);

impl OtelGuard {
    /// Get a tracer from the provider.
    fn tracer(&self, s: &'static str) -> opentelemetry_sdk::trace::Tracer {
        self.0.tracer(s)
    }

    /// Create a filtered tracing layer.
    pub fn layer<S>(&self) -> impl Layer<S>
    where
        S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
    {
        let tracer = self.tracer("tracing-otel-subscriber");
        tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_filter(self.1.clone())
    }
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Err(err) = self.0.shutdown() {
            eprintln!("{err:?}");
        }
    }
}

/// Otel configuration. This struct is intended to be loaded from the env vars
///
/// The env vars it checks are:
/// - `OTEL_EXPORTER_OTLP_ENDPOINT` - optional. The endpoint to send traces to,
///   should be some valid URL. If not specified, then [`OtelConfig::load`]
///   will return [`None`].
/// - OTEL_LEVEL - optional. Specifies the minimum [`tracing::Level`] to
///   export in the [`EnvFilter`] format. Defaults to [`tracing::Level::DEBUG`].
/// - OTEL_TIMEOUT - optional. Specifies the timeout for the exporter in
///   **milliseconds**. Defaults to 1000ms, which is equivalent to 1 second.
/// - OTEL_ENVIRONMENT_NAME - optional. Value for the `deployment.environment.
///   name` resource key according to the OTEL conventions.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct OtelConfig {
    /// The endpoint to send traces to, should be some valid HTTP endpoint for
    /// OTLP.
    pub endpoint: Url,

    /// Defaults to DEBUG.
    pub level: EnvFilter,

    /// OTEL convenition `deployment.environment.name`
    pub environment: String,
}

impl FromEnv for OtelConfig {
    fn inventory() -> Vec<&'static EnvItemInfo> {
        vec![
            &EnvItemInfo {
                var: OTEL_ENDPOINT,
                description:
                    "OTLP endpoint to send traces to, a url. If missing, disables OTLP exporting.",
                optional: true,
            },
            &EnvItemInfo {
                var: OTEL_LEVEL,
                description: "OTLP level to export. Follows the RUST_LOG env filter format. e.g. `OTEL_LEVEL=warn,my_crate=info`. Defaults to the value of `RUST_LOG` if not present.",
                optional: true,
            },
            &EnvItemInfo {
                var: OTEL_ENVIRONMENT,
                description: "OTLP environment name, a string",
                optional: true,
            },
        ]
    }

    fn from_env() -> Result<Self, FromEnvErr> {
        // load endpoint from env. ignore empty values (shortcut return None), parse, and print the error if any using inspect_err
        let endpoint = Url::from_env_var(OTEL_ENDPOINT)?;

        let level = if std::env::var(OTEL_LEVEL)
            .as_ref()
            .map(String::len)
            .unwrap_or_default()
            > 0
        {
            EnvFilter::from_env(OTEL_LEVEL)
        } else {
            EnvFilter::from_default_env()
        };

        let environment = String::from_env_var(OTEL_ENVIRONMENT).unwrap_or("unknown".into());

        Ok(Self {
            endpoint,
            level,
            environment,
        })
    }
}

impl OtelConfig {
    /// Load from env vars.
    ///
    /// The env vars it checks are:
    /// - `OTEL_EXPORTER_OTLP_ENDPOINT` - optional. The endpoint to send traces
    ///   to. If missing or unparsable, this function will return [`None`], and
    ///   OTLP exporting will be disabled.
    /// - `OTEL_LEVEL` - optional. Specifies the minimum [`tracing::Level`] to
    ///   export. Defaults to [`tracing::Level::DEBUG`].
    /// - `OTEL_TIMEOUT` - optional. Specifies the timeout for the exporter in
    ///   **milliseconds**. Defaults to 1000ms, which is equivalent to 1 second.
    /// - `OTEL_ENVIRONMENT_NAME` - optional. Value for the
    ///   `deployment.environment.name` resource key according to the OTEL
    ///   conventions. Defaults to `"unknown"`.
    pub fn load() -> Option<Self> {
        Self::from_env().ok()
    }

    fn resource(&self) -> Resource {
        Resource::builder()
            .with_schema_url(
                [
                    KeyValue::new(SERVICE_NAME, env!("CARGO_PKG_NAME")),
                    KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
                    KeyValue::new(DEPLOYMENT_ENVIRONMENT_NAME, self.environment.clone()),
                ],
                SCHEMA_URL,
            )
            .build()
    }

    /// Instantiate a new Otel provider, and start relevant tasks. Return a
    /// guard that will shut down the provider when dropped.
    pub fn provider(&self) -> OtelGuard {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .build()
            .unwrap();

        let provider = SdkTracerProvider::builder()
            // Customize sampling strategy
            // If export trace to AWS X-Ray, you can use XrayIdGenerator
            .with_resource(self.resource())
            .with_batch_exporter(exporter)
            .build();

        OtelGuard(provider, self.level.clone())
    }

    /// Create a new Otel provider, returning both the guard and a tracing
    /// layer that can be added to a subscriber.
    ///
    pub fn into_guard_and_layer<S>(self) -> (OtelGuard, impl Layer<S>)
    where
        S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
    {
        let guard = self.provider();
        let layer = guard.layer();
        (guard, layer)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const URL: &str = "http://localhost:4317";

    fn clear_env() {
        std::env::remove_var(OTEL_ENDPOINT);
        std::env::remove_var(OTEL_LEVEL);
        std::env::remove_var(OTEL_ENVIRONMENT);
    }

    fn run_clear_env<F>(f: F)
    where
        F: FnOnce(),
    {
        f();
        clear_env();
    }

    #[test]
    #[serial_test::serial]

    fn test_env_read() {
        run_clear_env(|| {
            std::env::set_var(OTEL_ENDPOINT, URL);
            std::env::set_var(OTEL_LEVEL, "debug");

            let cfg = OtelConfig::load().unwrap();
            assert_eq!(cfg.endpoint, URL.parse().unwrap());
            assert_eq!(
                cfg.level.max_level_hint(),
                Some(tracing::Level::DEBUG.into())
            );
            assert_eq!(cfg.environment, "unknown");
        })
    }

    #[test]
    #[serial_test::serial]
    fn test_env_read_level() {
        run_clear_env(|| {
            std::env::set_var(OTEL_ENDPOINT, URL);
            std::env::set_var(OTEL_LEVEL, "warn,my_app=info");

            let cfg = OtelConfig::load().unwrap();
            let s = cfg.level.to_string();
            let iter = s.split(",");
            assert!(iter.clone().any(|x| x == "warn"));
            assert!(iter.clone().any(|x| x == "my_app=info"));
        })
    }

    #[test]
    #[serial_test::serial]
    fn invalid_url() {
        run_clear_env(|| {
            std::env::set_var(OTEL_ENDPOINT, "not a url");

            let cfg = OtelConfig::load();
            assert!(cfg.is_none());
        })
    }
}
