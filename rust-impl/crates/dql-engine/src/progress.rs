//! Callbacks for long-running INSERT / LOAD / UPDATE / DELETE / SCAN work.

use std::sync::{Arc, Mutex};

/// How many items to write before emitting another progress event.
pub const WRITE_PROGRESS_CHUNK: usize = 25;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressEvent {
    pub done: u64,
    pub total: Option<u64>,
    pub phase: String,
}

impl ProgressEvent {
    pub fn new(done: u64, total: Option<u64>, phase: impl Into<String>) -> Self {
        Self {
            done,
            total,
            phase: phase.into(),
        }
    }
}

/// Shared progress hook. `Engine` and AWS backends clone the same sink so a
/// `--serve` client (or test) can observe mid-statement work.
type ProgressCallback = Box<dyn FnMut(ProgressEvent) + Send>;

#[derive(Clone, Default)]
pub struct ProgressSink {
    inner: Arc<Mutex<Option<ProgressCallback>>>,
}

impl std::fmt::Debug for ProgressSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressSink")
            .field("active", &self.is_active())
            .finish()
    }
}

impl ProgressSink {
    pub fn set(&self, callback: Option<ProgressCallback>) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = callback;
        }
    }

    pub fn clear(&self) {
        self.set(None);
    }

    pub fn is_active(&self) -> bool {
        self.inner
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    pub fn report(&self, done: u64, total: Option<u64>, phase: &str) {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(callback) = guard.as_mut() {
                callback(ProgressEvent::new(done, total, phase));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn reports_when_callback_is_set() {
        let sink = ProgressSink::default();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_cb = Arc::clone(&seen);
        sink.set(Some(Box::new(move |event| {
            seen_cb.lock().unwrap().push(event);
        })));
        sink.report(1, Some(2), "write");
        sink.clear();
        sink.report(2, Some(2), "write");
        let events = seen.lock().unwrap().clone();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], ProgressEvent::new(1, Some(2), "write"));
    }
}
