use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct RateLimit {
    read_per_second: f64,
    write_per_second: f64,
    read_tokens: f64,
    write_tokens: f64,
    last_refill: Instant,
}

impl RateLimit {
    pub fn new(read_per_second: f64, write_per_second: f64) -> Self {
        Self {
            read_per_second,
            write_per_second,
            read_tokens: read_per_second,
            write_tokens: write_per_second,
            last_refill: Instant::now(),
        }
    }

    pub fn on_capacity(&mut self, read_units: f64, write_units: f64) -> Duration {
        self.refill();
        self.read_tokens = (self.read_tokens - read_units).max(0.0);
        self.write_tokens = (self.write_tokens - write_units).max(0.0);
        let read_wait = if self.read_tokens < 0.0 && self.read_per_second > 0.0 {
            Duration::from_secs_f64((-self.read_tokens) / self.read_per_second)
        } else {
            Duration::ZERO
        };
        let write_wait = if self.write_tokens < 0.0 && self.write_per_second > 0.0 {
            Duration::from_secs_f64((-self.write_tokens) / self.write_per_second)
        } else {
            Duration::ZERO
        };
        read_wait.max(write_wait)
    }

    fn refill(&mut self) {
        let elapsed = self.last_refill.elapsed().as_secs_f64();
        if elapsed <= 0.0 {
            return;
        }
        self.read_tokens = (self.read_tokens + elapsed * self.read_per_second)
            .min(self.read_per_second);
        self.write_tokens = (self.write_tokens + elapsed * self.write_per_second)
            .min(self.write_per_second);
        self.last_refill = Instant::now();
    }
}
