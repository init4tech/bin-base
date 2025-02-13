//! This example is for testing exporting data to an OTLP collector.
//!
//! It produces
//! - 1 spawn for the lifetime of the program
//! - 1 span every 5 seconds
//! - 1 event every 5 seconds
//!
//! It can be killed via

use init4_bin_base::{
    deps::tracing::{info, info_span},
    init4,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let term: Arc<AtomicBool> = Default::default();
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term))?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term))?;

    let _guard = init4();

    let mut counter = 0;
    let _outer = info_span!("outer span").entered();

    while !term.load(Ordering::Relaxed) {
        let _inner = info_span!("inner span").entered();

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        counter += 1;
        info!(counter, "this is an event");
    }

    info!("signal received, shutting down");
    Ok(())
}
