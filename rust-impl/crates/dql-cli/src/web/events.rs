//! One server-sent event stream per browser tab.
//!
//! The hub fans out small facts. A workspace watch reports files changed
//! outside this process. Saves, creates, and deletes in this process publish
//! the same facts directly. Presence and table changes never touch the disk,
//! so those are published by the caller.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use std::time::{Duration, Instant};

use notify::event::ModifyKind;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::{json, Value};

use super::files;
use super::server::App;

const QUIET: Duration = Duration::from_millis(80);

struct Subscriber {
    id: u64,
    tx: SyncSender<String>,
}

struct Inner {
    next_sub: u64,
    subs: Vec<Subscriber>,
    last_file: HashMap<String, String>,
    last_tree: Option<Instant>,
}

pub(crate) struct Hub {
    inner: Mutex<Inner>,
}

impl Hub {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                next_sub: 1,
                subs: Vec::new(),
                last_file: HashMap::new(),
                last_tree: None,
            }),
        }
    }

    pub(crate) fn subscribe(&self) -> (u64, Receiver<String>) {
        let (tx, rx) = mpsc::sync_channel(64);
        let mut inner = self.lock();
        let id = inner.next_sub;
        inner.next_sub = inner.next_sub.saturating_add(1);
        inner.subs.push(Subscriber { id, tx });
        (id, rx)
    }

    pub(crate) fn unsubscribe(&self, id: u64) {
        self.lock().subs.retain(|sub| sub.id != id);
    }

    pub(crate) fn file(&self, path: &str, revision: &str) {
        let frame = frame("file", &json!({ "path": path, "revision": revision }));
        let mut inner = self.lock();
        if inner.last_file.get(path).map(String::as_str) == Some(revision) {
            return;
        }
        inner
            .last_file
            .insert(path.to_string(), revision.to_string());
        publish(&mut inner, frame);
    }

    pub(crate) fn tree(&self) {
        let mut inner = self.lock();
        let now = Instant::now();
        if inner
            .last_tree
            .is_some_and(|last| now.saturating_duration_since(last) < QUIET)
        {
            return;
        }
        inner.last_tree = Some(now);
        publish(&mut inner, frame("tree", &json!({})));
    }

    /// `names` are the tables a create, drop, or alter just changed.
    pub(crate) fn tables(&self, names: &[String]) {
        if names.is_empty() {
            return;
        }
        publish(
            &mut self.lock(),
            frame("tables", &json!({ "tables": names })),
        );
    }

    pub(crate) fn presence(&self, snapshot: &Value) {
        publish(&mut self.lock(), frame("presence", snapshot));
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|err| err.into_inner())
    }
}

fn frame(event: &str, data: &Value) -> String {
    format!("event: {event}\ndata: {data}\n\n")
}

fn publish(inner: &mut Inner, frame: String) {
    inner
        .subs
        .retain(|sub| match sub.tx.try_send(frame.clone()) {
            Ok(()) => true,
            Err(TrySendError::Disconnected(_)) => false,
            Err(TrySendError::Full(_)) => false,
        });
}

/// Watch `app.root` until the server is dropped.
///
/// Returns after the watch is installed. A change that happens before that
/// is not delivered, so the server waits here before it accepts clients.
pub(crate) fn start(app: &Arc<App>) {
    let weak = Arc::downgrade(app);
    let root = app.root.clone();
    let (ready_tx, ready_rx) = mpsc::channel();
    thread::spawn(move || watch(weak, root, ready_tx));
    if let Err(err) = ready_rx
        .recv()
        .unwrap_or_else(|_| Err("stopped before it started".to_string()))
    {
        eprintln!(
            "{}",
            crate::style::Style::stderr().warn(&format!("dqlrs: workspace watch: {err}"))
        );
    }
}

fn watch(app: Weak<App>, root: PathBuf, ready: Sender<Result<(), String>>) {
    let root = root.canonicalize().unwrap_or(root);
    let (tx, rx) = mpsc::channel();
    let mut watcher = match RecommendedWatcher::new(
        move |result: notify::Result<Event>| {
            if let Ok(event) = result {
                let _ = tx.send(event);
            }
        },
        notify::Config::default(),
    ) {
        Ok(watcher) => watcher,
        Err(err) => {
            let _ = ready.send(Err(err.to_string()));
            return;
        }
    };
    if let Err(err) = watcher.watch(&root, RecursiveMode::Recursive) {
        let _ = ready.send(Err(err.to_string()));
        return;
    }
    let _ = ready.send(Ok(()));
    loop {
        if app.upgrade().is_none() {
            break;
        }
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(event) => {
                let mut batch = vec![event];
                let quiet = Instant::now() + QUIET;
                while let Ok(more) =
                    rx.recv_timeout(quiet.saturating_duration_since(Instant::now()))
                {
                    batch.push(more);
                }
                if let Some(app) = app.upgrade() {
                    apply(&app, &root, &batch);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn apply(app: &App, root: &Path, events: &[Event]) {
    let mut tree = false;
    let mut files_changed: HashMap<String, String> = HashMap::new();
    for event in events {
        for path in &event.paths {
            let Some(relative) = files::relative_watch(root, path) else {
                continue;
            };
            match event.kind {
                EventKind::Modify(ModifyKind::Metadata(_))
                | EventKind::Access(_)
                | EventKind::Other => {}
                EventKind::Remove(_) => {
                    tree = true;
                    if files::viewable_relative(&relative) {
                        files_changed.insert(relative, "missing".to_string());
                    }
                }
                EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(_)) => {
                    tree = true;
                    note_file(path, &relative, &mut files_changed);
                }
                EventKind::Modify(_) | EventKind::Any => {
                    if path.is_dir() {
                        tree = true;
                    } else {
                        note_file(path, &relative, &mut files_changed);
                    }
                }
            }
        }
    }
    for (path, revision) in files_changed {
        app.hub.file(&path, &revision);
    }
    if tree {
        app.hub.tree();
    }
}

fn note_file(path: &Path, relative: &str, files_changed: &mut HashMap<String, String>) {
    if !path.is_file() || !files::viewable_relative(relative) {
        return;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    files_changed.insert(relative.to_string(), files::content_revision(&text));
}
