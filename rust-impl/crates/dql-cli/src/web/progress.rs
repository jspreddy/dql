//! Step-by-step progress for one-off CLI tasks such as `dqlrs web --init-examples` and
//! `dqlrs web --load-tables`.
//!
//! In a terminal each step shows a spinner and a live detail while it runs, then a green
//! check and a summary, or a red cross. A step can also log finished items (one table each)
//! above its spinner. Piped output gets one plain line per item and per finished step.
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

/// A running step: its live detail (such as a count) and finished items logged above it.
#[derive(Clone)]
pub(crate) struct Detail {
    text: Arc<Mutex<String>>,
    /// `(done, total)` for a bar drawn before the text.
    bar: Arc<Mutex<Option<(usize, usize)>>>,
    mode: Mode,
    /// Held while writing, so the spinner and logged lines never interleave.
    out: Arc<Mutex<()>>,
}

impl Detail {
    fn new(mode: Mode) -> Self {
        Self {
            text: Arc::default(),
            bar: Arc::default(),
            mode,
            out: Arc::default(),
        }
    }

    pub(crate) fn set(&self, text: impl Into<String>) {
        if let Ok(mut detail) = self.text.lock() {
            *detail = text.into();
        }
    }

    /// Show a bar for `done` of `total` before the detail text.
    pub(crate) fn set_bar(&self, done: usize, total: usize) {
        if let Ok(mut bar) = self.bar.lock() {
            *bar = Some((done, total));
        }
    }

    fn get_bar(&self) -> Option<(usize, usize)> {
        self.bar.lock().ok().and_then(|bar| *bar)
    }

    fn get(&self) -> String {
        self.text
            .lock()
            .map(|detail| detail.clone())
            .unwrap_or_default()
    }

    /// Print one finished item above the spinner: a check or a cross, `text`, and a dimmed
    /// `note`.
    pub(crate) fn log(&self, ok: bool, text: &str, note: &str) {
        let (mark, code) = if ok { ("✓", "32") } else { ("✗", "31") };
        match self.mode {
            Mode::Quiet => {}
            Mode::Plain => {
                if note.is_empty() {
                    println!("    {mark} {text}");
                } else {
                    println!("    {mark} {text}  {note}");
                }
            }
            Mode::Live { color } => {
                let _guard = self.out.lock();
                let note = if note.is_empty() {
                    String::new()
                } else {
                    format!("  {}", paint(note, "2", color))
                };
                let mut out = io::stdout().lock();
                let _ = writeln!(
                    out,
                    "\r\x1b[2K    {} {text}{note}",
                    paint(mark, code, color)
                );
                let _ = out.flush();
            }
        }
    }
}

const BAR_WIDTH: usize = 24;

/// How many of `width` cells a bar for `done` of `total` fills.
fn filled(done: usize, total: usize, width: usize) -> usize {
    // An empty total counts as complete.
    (done.min(total) * width)
        .checked_div(total)
        .unwrap_or(width)
}

/// A plain text bar such as `██████░░░░` for `done` of `total`.
#[cfg(test)]
fn bar(done: usize, total: usize, width: usize) -> String {
    let filled = filled(done, total, width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

/// The bar in color: done cells cyan, the rest dim, then `done/total`.
fn painted_bar(done: usize, total: usize, color: bool) -> String {
    let full = filled(done, total, BAR_WIDTH);
    format!(
        "{}{}  {done}/{total}",
        paint(&"━".repeat(full), "36", color),
        paint(&"━".repeat(BAR_WIDTH - full), "2;37", color)
    )
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
        let detail = Detail::new(self.mode);
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
                let guard = detail.out.lock();
                let text = detail.get();
                let mut extra = String::new();
                if let Some((done, total)) = detail.get_bar() {
                    extra.push_str("  ");
                    extra.push_str(&painted_bar(done, total, color));
                }
                if !text.is_empty() {
                    extra.push_str("  ");
                    extra.push_str(&paint(&text, "2", color));
                }
                let _ = write!(
                    out,
                    "\r\x1b[2K  {} {label}{extra}",
                    paint(FRAMES[frame % FRAMES.len()], "36", color)
                );
                let _ = out.flush();
                drop(guard);
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
    fn the_bar_fills_in_proportion() {
        assert_eq!(bar(0, 4, 8), "░░░░░░░░");
        assert_eq!(bar(1, 4, 8), "██░░░░░░");
        assert_eq!(bar(4, 4, 8), "████████");
        assert_eq!(bar(9, 4, 8), "████████");
        assert_eq!(bar(0, 0, 8), "████████");
    }

    #[test]
    fn colors_are_ansi_only_when_asked() {
        assert_eq!(paint("✓", "32", true), "\x1b[32m✓\x1b[0m");
        assert_eq!(paint("✓", "32", false), "✓");
    }
}
