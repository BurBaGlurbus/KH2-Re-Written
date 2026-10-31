//! Load-stage timing for load-time work (off unless asked for).
use std::sync::OnceLock;
use std::time::Instant;

/// When the first timer started (the stages' running total).
static FIRST: OnceLock<Instant> = OnceLock::new();

/// `KH2_LOAD_TIMES=1`: each load stage's time on stderr (load-time work).
pub struct LoadTimer {
    last: Instant,
    on: bool,
}

impl LoadTimer {
    pub fn new() -> Self {
        FIRST.get_or_init(Instant::now);
        Self {
            last: Instant::now(),
            on: std::env::var_os("KH2_LOAD_TIMES").is_some(),
        }
    }

    pub fn mark(&mut self, stage: &str) {
        if self.on {
            let ms = |t: Instant| t.elapsed().as_secs_f64() * 1e3;
            eprintln!(
                "load: {stage} {:.0} ms (at {:.0} ms)",
                ms(self.last),
                FIRST.get().map_or(0., |&t| ms(t))
            );
        }
        self.last = Instant::now();
    }
}
