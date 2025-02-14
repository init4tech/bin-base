use init4_bin_base::init4;
use std::sync::{atomic::AtomicBool, Arc};

fn main() {
    let term: Arc<AtomicBool> = Default::default();

    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term));

    init4();
}
