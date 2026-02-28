use eyre::WrapErr;
use init4_bin_base::{
    init,
    utils::{from_env::FromEnv, metrics::MetricsConfig, tracing::TracingConfig},
};
use std::sync::{atomic::AtomicBool, Arc};

fn main() -> eyre::Result<()> {
    let term: Arc<AtomicBool> = Default::default();

    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term));

    let tracing_config =
        TracingConfig::from_env().wrap_err("failed to get tracing config from environment")?;
    let metrics_config =
        MetricsConfig::from_env().wrap_err("failed to get metrics config from environment")?;
    init(tracing_config, metrics_config);
    Ok(())
}
