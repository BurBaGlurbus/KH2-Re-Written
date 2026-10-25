//! Load-stage timing for load-time work (off unless asked for).
use std::time::Instant;

/// `KH2_LOAD_TIMES=1`: each load stage's time on stderr (load-time work).
pub(crate) struct LoadTimer {
    last: Instant,
    on: bool,
}

impl LoadTimer {
    pub(crate) fn new() -> Self {
        Self {
            last: Instant::now(),
            on: std::env::var_os("KH2_LOAD_TIMES").is_some(),
        }
    }

    pub(crate) fn mark(&mut self, stage: &str) {
        if self.on {
            eprintln!(
                "load: {stage} {:.0} ms",
                self.last.elapsed().as_secs_f64() * 1e3
            );
        }
        self.last = Instant::now();
    }
}
