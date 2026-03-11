use init4_bin_base::{
    utils::{from_env::FromEnv, metrics::MetricsConfig, tracing::TracingConfig},
    Init4Config,
};
use std::sync::{atomic::AtomicBool, Arc};

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

fn main() -> eyre::Result<()> {
    let term: Arc<AtomicBool> = Default::default();

    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term));

    let _config_and_guard = init4_bin_base::init::<Config>()?;
    Ok(())
}
