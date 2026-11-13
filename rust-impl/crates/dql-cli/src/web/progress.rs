//! Step-by-step progress for one-off CLI tasks such as `dqlrs web --init-examples`.
//!
//! In a terminal each step shows a spinner and a live detail while it runs, then a green
//! check and a summary, or a red cross. Piped output gets one plain line per finished step.
//! `NO_COLOR` turns colors off.

use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const TICK: Duration = Duration::from_millis(80);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// Print nothing (tests).
    #[cfg_attr(not(test), allow(dead_code))]
    Quiet,
    /// One line per finished step, no escapes.
    Plain,
    /// Spinner, redraws, and colors when `color`.
    Live { color: bool },
}

pub(crate) struct Progress {
    mode: Mode,
}

/// A running step's live detail, such as a file count.
#[derive(Clone, Default)]
pub(crate) struct Detail(Arc<Mutex<String>>);

impl Detail {
    pub(crate) fn set(&self, text: impl Into<String>) {
        if let Ok(mut detail) = self.0.lock() {
            *detail = text.into();
        }
    }

    fn get(&self) -> String {
        self.0
            .lock()
            .map(|detail| detail.clone())
            .unwrap_or_default()
    }
}

impl Progress {
    /// Live in a terminal, plain otherwise.
    pub(crate) fn stdout() -> Self {
        let mode = if io::stdout().is_terminal() {
            Mode::Live {
                color: std::env::var_os("NO_COLOR").is_none(),
            }
        } else {
            Mode::Plain
        };
        Self { mode }
    }

    #[cfg(test)]
    pub(crate) fn quiet() -> Self {
        Self { mode: Mode::Quiet }
    }

    /// Run `work` as one step labelled `label`. On success `work` returns its value and the
    /// summary shown after the check mark.
    pub(crate) fn step<T>(
        &self,
        label: &str,
        work: impl FnOnce(&Detail) -> color_eyre::Result<(T, String)>,
    ) -> color_eyre::Result<T> {
        let detail = Detail::default();
        let spinner = match self.mode {
            Mode::Live { color } => Some(Spinner::start(label.to_string(), detail.clone(), color)),
            _ => None,
        };
        let result = work(&detail);
        if let Some(spinner) = spinner {
            spinner.stop();
        }
        match &result {
            Ok((_, summary)) => self.line(true, summary),
            Err(_) => self.line(false, label),
        }
        result.map(|(value, _)| value)
    }

    /// A blank line or a heading between groups of steps.
    pub(crate) fn say(&self, text: &str) {
        if self.mode != Mode::Quiet {
            println!("{text}");
        }
    }

    fn line(&self, ok: bool, text: &str) {
        let (mark, code) = if ok { ("✓", "32") } else { ("✗", "31") };
        match self.mode {
            Mode::Quiet => {}
            Mode::Plain => println!("  {mark} {text}"),
            Mode::Live { color } => {
                let mut out = io::stdout().lock();
                let _ = writeln!(out, "\r\x1b[2K  {} {text}", paint(mark, code, color));
                let _ = out.flush();
            }
        }
    }
}

struct Spinner {
    stop: Arc<AtomicBool>,
    handle: thread::JoinHandle<()>,
}

impl Spinner {
    fn start(label: String, detail: Detail, color: bool) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            let mut frame = 0;
            let mut out = io::stdout();
            let _ = write!(out, "\x1b[?25l");
            while !flag.load(Ordering::Relaxed) {
                let extra = detail.get();
                let extra = if extra.is_empty() {
                    String::new()
                } else {
                    format!("  {}", paint(&extra, "2", color))
                };
                let _ = write!(
                    out,
                    "\r\x1b[2K  {} {label}{extra}",
                    paint(FRAMES[frame % FRAMES.len()], "36", color)
                );
                let _ = out.flush();
                frame += 1;
                thread::sleep(TICK);
            }
            let _ = write!(out, "\r\x1b[2K\x1b[?25h");
            let _ = out.flush();
        });
        Self { stop, handle }
    }

    fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.handle.join();
    }
}

fn paint(text: &str, code: &str, color: bool) -> String {
    if color {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_returns_the_work_value_and_errors_pass_through() {
        let progress = Progress::quiet();
        let value = progress.step("adding", |detail| {
            detail.set("1/2");
            Ok((7, "added".to_string()))
        });
        assert_eq!(value.unwrap(), 7);
        let failed: color_eyre::Result<u8> =
            progress.step("failing", |_| Err(color_eyre::eyre::eyre!("nope")));
        assert_eq!(failed.unwrap_err().to_string(), "nope");
    }

    #[test]
    fn colors_are_ansi_only_when_asked() {
        assert_eq!(paint("✓", "32", true), "\x1b[32m✓\x1b[0m");
        assert_eq!(paint("✓", "32", false), "✓");
    }
}
