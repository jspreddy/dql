//! Cooperative cancel for long-running statements.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::EngineError;

/// Shared stop flag. The web UI (or `--serve` interrupt) sets it from another
/// thread; statement loops poll it between pages, chunks, and throttle waits.
#[derive(Clone, Debug, Default)]
pub struct CancelFlag {
    inner: Arc<AtomicBool>,
}

impl CancelFlag {
    pub fn request(&self) {
        self.inner.store(true, Ordering::SeqCst);
    }

    pub fn clear(&self) {
        self.inner.store(false, Ordering::SeqCst);
    }

    pub fn is_requested(&self) -> bool {
        self.inner.load(Ordering::SeqCst)
    }

    pub fn check(&self) -> Result<(), EngineError> {
        if self.is_requested() {
            Err(EngineError::Cancelled)
        } else {
            Ok(())
        }
    }

    /// Sleep in short slices so a stop request is noticed during throttle waits.
    pub fn sleep(&self, wait: Duration) -> Result<(), EngineError> {
        if wait.is_zero() {
            return self.check();
        }
        let deadline = Instant::now() + wait;
        loop {
            self.check()?;
            let now = Instant::now();
            if now >= deadline {
                return Ok(());
            }
            let slice = (deadline - now).min(Duration::from_millis(50));
            thread::sleep(slice);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_fails_after_request() {
        let flag = CancelFlag::default();
        flag.check().unwrap();
        flag.request();
        assert!(matches!(flag.check(), Err(EngineError::Cancelled)));
        flag.clear();
        flag.check().unwrap();
    }

    #[test]
    fn sleep_returns_when_requested() {
        let flag = CancelFlag::default();
        let started = Instant::now();
        thread::spawn({
            let flag = flag.clone();
            move || {
                thread::sleep(Duration::from_millis(30));
                flag.request();
            }
        });
        let err = flag.sleep(Duration::from_secs(5)).unwrap_err();
        assert!(matches!(err, EngineError::Cancelled));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
