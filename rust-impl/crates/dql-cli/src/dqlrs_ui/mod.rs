//! Native window shipped inside the `dqlrs` binary.
//!
//! `dqlrs --ui` builds this iced interface and calls [`crate::web::studio`] in
//! process. There is no HTTP hop and no second install.

mod highlight;
mod style;
mod view;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced::widget::text_editor;
use iced::{Element, Subscription, Task, Theme};
use serde::{Deserialize, Serialize};

use crate::session::Session;
use crate::web::files::{Node, SaveFile};
use crate::web::lint::LintIssue;
use crate::web::search::SearchHit;
use crate::web::shape::{Column, ListedTable};
use crate::web::studio::{self, LiveProgress, StatementCard, Studio};

pub fn run(session: Session, root: PathBuf, load_tables: bool) -> color_eyre::Result<()> {
    let studio = Studio::open(session, root).map_err(|err| color_eyre::eyre::eyre!("{err}"))?;
    if load_tables {
        let _ = studio.start_table_load();
    }
    let prefs = Prefs::load();
    let studio = Arc::new(Mutex::new(studio));
    let progress = Arc::new(Mutex::new(LiveProgress::default()));
    iced::application(
        move || App::new(Arc::clone(&studio), Arc::clone(&progress), prefs.clone()),
        App::update,
        App::view,
    )
    .title("dqlrs")
    .theme(App::theme)
    .style(|state, _theme| {
        let colors = style::palette(state.prefs.dark);
        iced::theme::Style {
            background_color: colors.canvas,
            text_color: colors.text,
        }
    })
    .subscription(App::subscription)
    .window_size(iced::Size::new(1280.0, 800.0))
    .run()
    .map_err(|err| color_eyre::eyre::eyre!("{err}"))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Prefs {
    dark: bool,
    nav: f32,
    results: f32,
    write_bands: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            dark: false,
            nav: 260.0,
            results: 280.0,
            write_bands: true,
        }
    }
}

impl Prefs {
    fn load() -> Self {
        let path = prefs_path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    fn store(&self) {
        let path = prefs_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string(self) {
            let _ = std::fs::write(path, text);
        }
    }
}

fn prefs_path() -> PathBuf {
    if let Some(path) = std::env::var_os("DQLRS_UI_PREFS") {
        return PathBuf::from(path);
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("dqlrs").join("ui.json");
    }
    if cfg!(target_os = "macos") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("dqlrs")
                .join("ui.json");
        }
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("dqlrs").join("ui.json");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("dqlrs")
            .join("ui.json");
    }
    std::env::temp_dir().join("dqlrs-ui.json")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Query,
    Tables,
}

#[derive(Clone, Debug)]
enum Dialog {
    None,
    Name {
        title: &'static str,
        parent: String,
        dir: bool,
        previous: Option<String>,
    },
    Move {
        path: String,
    },
    Conflict,
}

#[derive(Clone, Debug)]
struct MenuAt {
    path: String,
    kind: String,
}

struct App {
    studio: Arc<Mutex<Studio>>,
    progress: Arc<Mutex<LiveProgress>>,
    cancel: dql_engine::CancelFlag,
    prefs: Prefs,
    mode: Mode,
    tree: Option<Node>,
    endpoint: String,
    env_lines: Vec<(String, String)>,
    open_path: String,
    revision: String,
    editor: text_editor::Content,
    dirty: bool,
    conflict_text: Option<String>,
    lint: Vec<LintIssue>,
    results: Vec<StatementCard>,
    running_path: String,
    pulse: bool,
    live: LiveProgress,
    search: String,
    hits: Vec<SearchHit>,
    suggest: Vec<SearchHit>,
    menu: Option<MenuAt>,
    dialog: Dialog,
    dialog_text: String,
    tables: Vec<ListedTable>,
    table_note: String,
    cache_text: String,
    table_filter: String,
    open_table: String,
    table_page: usize,
    table_columns: Vec<Column>,
    table_items: Vec<serde_json::Value>,
    table_has_more: bool,
    show_settings: bool,
    show_env: bool,
    open_cards: Vec<bool>,
    row_json: Option<String>,
    status: String,
}

#[derive(Debug, Clone)]
enum Message {
    Mode(Mode),
    Search(String),
    PickHit(usize),
    Open(String),
    Menu(String, String),
    MenuRoot,
    CloseMenu,
    MenuAct(&'static str),
    Editor(text_editor::Action),
    Save,
    ReloadDisk,
    KeepEdits,
    RunFile,
    RunCursor,
    RunStatement(String),
    Stop,
    Tick,
    Pulse,
    Watch,
    RunDone(Vec<StatementCard>),
    RefreshTables(bool),
    TableFilter(String),
    OpenTable(String),
    TablePage(i32),
    ClearCache,
    DialogText(String),
    DialogOk,
    DialogCancel,
    ToggleSettings,
    ToggleEnv,
    ToggleTheme,
    ToggleBands,
    Nav(f32),
    Results(f32),
    ToggleCard(usize),
    ShowRow(String),
    CloseRow,
    UseSuggest(usize),
}

impl App {
    fn new(studio: Arc<Mutex<Studio>>, progress: Arc<Mutex<LiveProgress>>, prefs: Prefs) -> Self {
        let (endpoint, env_lines, cancel) = {
            let studio = lock(&studio);
            (
                studio.endpoint_label(),
                studio.environment_lines(),
                studio.cancel_flag(),
            )
        };
        let mut app = Self {
            studio,
            progress,
            cancel,
            prefs,
            mode: Mode::Query,
            tree: None,
            endpoint,
            env_lines,
            open_path: String::new(),
            revision: String::new(),
            editor: text_editor::Content::with_text(""),
            dirty: false,
            conflict_text: None,
            lint: Vec::new(),
            results: Vec::new(),
            running_path: String::new(),
            pulse: true,
            live: LiveProgress::default(),
            search: String::new(),
            hits: Vec::new(),
            suggest: Vec::new(),
            menu: None,
            dialog: Dialog::None,
            dialog_text: String::new(),
            tables: Vec::new(),
            table_note: String::new(),
            cache_text: String::new(),
            table_filter: String::new(),
            open_table: String::new(),
            table_page: 0,
            table_columns: Vec::new(),
            table_items: Vec::new(),
            table_has_more: false,
            show_settings: false,
            show_env: false,
            open_cards: Vec::new(),
            row_json: None,
            status: String::new(),
        };
        app.refresh_tree();
        app
    }

    fn theme(&self) -> Theme {
        if self.prefs.dark {
            Theme::Dark
        } else {
            Theme::Light
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![keyboard_subscription()];
        if !self.running_path.is_empty() {
            subs.push(iced::time::every(Duration::from_millis(180)).map(|_| Message::Pulse));
            subs.push(iced::time::every(Duration::from_millis(120)).map(|_| Message::Tick));
        }
        subs.push(iced::time::every(Duration::from_secs(1)).map(|_| Message::Watch));
        Subscription::batch(subs)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Mode(mode) => {
                self.mode = mode;
                self.menu = None;
                if mode == Mode::Tables {
                    return self.update(Message::RefreshTables(false));
                }
            }
            Message::Search(query) => {
                self.search = query;
                self.hits = if self.search.trim().is_empty() {
                    Vec::new()
                } else {
                    lock(&self.studio).search(&self.search).items
                };
            }
            Message::PickHit(index) => {
                if let Some(hit) = self.hits.get(index).cloned() {
                    if let Some(path) = hit.path {
                        self.open_file(path);
                    } else if let Some(table) = hit.table {
                        self.mode = Mode::Tables;
                        self.open_table(table);
                    }
                }
                self.search.clear();
                self.hits.clear();
                self.suggest.clear();
            }
            Message::Open(path) => {
                if path.ends_with(".dql") {
                    self.open_file(path);
                }
                self.menu = None;
            }
            Message::Menu(path, kind) => {
                self.menu = Some(MenuAt { path, kind });
            }
            Message::MenuRoot => {
                self.menu = Some(MenuAt {
                    path: String::new(),
                    kind: "root".to_string(),
                });
            }
            Message::CloseMenu => self.menu = None,
            Message::MenuAct(label) => {
                let menu = self.menu.clone();
                self.menu = None;
                if let Some(menu) = menu {
                    return self.menu_act(&menu, label);
                }
            }
            Message::Editor(action) => {
                let is_edit = matches!(action, text_editor::Action::Edit(_));
                self.editor.perform(action);
                if is_edit && !self.open_path.is_empty() {
                    self.dirty = true;
                    self.save_editor();
                    self.refresh_lint();
                }
                if is_edit {
                    self.refresh_suggest();
                }
            }
            Message::Save => self.save_editor(),
            Message::ReloadDisk => {
                if let Some(text) = self.conflict_text.clone() {
                    self.editor = text_editor::Content::with_text(&text);
                    self.revision = crate::web::files::content_revision(&text);
                    self.dirty = false;
                    self.conflict_text = None;
                    self.dialog = Dialog::None;
                    self.refresh_lint();
                    self.status = "Loaded the file from disk.".to_string();
                }
            }
            Message::KeepEdits => {
                self.conflict_text = None;
                self.dialog = Dialog::None;
                if let Ok(file) = lock(&self.studio).read_file(&self.open_path) {
                    self.revision = file.revision;
                }
                self.save_editor();
            }
            Message::RunFile => {
                if !self.open_path.is_empty() {
                    self.save_editor();
                    let text = editor_text(&self.editor);
                    return self.start_run(vec![(self.open_path.clone(), text)]);
                }
            }
            Message::RunCursor => {
                let text = editor_text(&self.editor);
                let at = cursor_byte(&self.editor, &text);
                if let Some(statement) = studio::statement_at(&text, at) {
                    self.save_editor();
                    return self.start_run(vec![(self.open_path.clone(), statement)]);
                }
                self.status = "No statement at the cursor.".to_string();
            }
            Message::RunStatement(statement) => {
                self.save_editor();
                return self.start_run(vec![(self.open_path.clone(), statement)]);
            }
            Message::Stop => {
                self.cancel.request();
                self.status = "Stopping…".to_string();
            }
            Message::Pulse => self.pulse = !self.pulse,
            Message::Tick => {
                self.live = lock_progress(&self.progress);
                if !self.live.path.is_empty() {
                    self.running_path = self.live.path.clone();
                }
            }
            Message::Watch => self.watch_disk(),
            Message::RunDone(cards) => {
                self.running_path.clear();
                self.live = LiveProgress::default();
                if cards.is_empty() {
                    self.status = "Nothing to run.".to_string();
                } else {
                    let failed = cards
                        .iter()
                        .find(|card| !card.ok)
                        .map(|card| card.failure_text());
                    self.status = if let Some(err) = failed {
                        format!("A statement failed. {err}")
                    } else {
                        format!("{} statement(s) finished.", cards.len())
                    };
                    for card in cards.into_iter().rev() {
                        self.open_cards.insert(0, false);
                        self.results.insert(0, card);
                    }
                }
                if !self.open_path.is_empty() {
                    if let Ok(stored) = lock(&self.studio).load_results(&self.open_path) {
                        if !stored.is_empty() {
                            self.results = stored;
                            self.open_cards = vec![false; self.results.len()];
                        }
                    }
                }
            }
            Message::RefreshTables(refresh) => {
                match lock(&self.studio).tables(&self.table_filter, refresh) {
                    Ok(list) => {
                        self.tables = list.tables;
                        self.table_note = list.note;
                        self.cache_text = list.cache.text;
                        self.status.clear();
                    }
                    Err(err) => self.status = err,
                }
                if lock(&self.studio).cache_refreshing() && self.status.is_empty() {
                    self.status = "Refreshing the table cache.".to_string();
                }
            }
            Message::TableFilter(filter) => {
                self.table_filter = filter;
                return self.update(Message::RefreshTables(false));
            }
            Message::OpenTable(name) => self.open_table(name),
            Message::TablePage(delta) => {
                if self.open_table.is_empty() {
                    return Task::none();
                }
                let next = self.table_page as i32 + delta;
                if next >= 0 {
                    self.table_page = next as usize;
                    self.load_table_page();
                }
            }
            Message::ClearCache => {
                lock(&self.studio).clear_table_cache();
                self.tables.clear();
                self.cache_text = "Cache cleared.".to_string();
                return self.update(Message::RefreshTables(true));
            }
            Message::DialogText(text) => self.dialog_text = text,
            Message::DialogOk => return self.confirm_dialog(),
            Message::DialogCancel => {
                self.dialog = Dialog::None;
                self.dialog_text.clear();
            }
            Message::ToggleSettings => self.show_settings = !self.show_settings,
            Message::ToggleEnv => self.show_env = !self.show_env,
            Message::ToggleTheme => {
                self.prefs.dark = !self.prefs.dark;
                self.prefs.store();
            }
            Message::ToggleBands => {
                self.prefs.write_bands = !self.prefs.write_bands;
                self.prefs.store();
            }
            Message::Nav(delta) => {
                self.prefs.nav = (self.prefs.nav + delta).clamp(160.0, 520.0);
                self.prefs.store();
            }
            Message::Results(delta) => {
                self.prefs.results = (self.prefs.results + delta).clamp(120.0, 640.0);
                self.prefs.store();
            }
            Message::ToggleCard(index) => {
                if let Some(open) = self.open_cards.get_mut(index) {
                    *open = !*open;
                }
            }
            Message::ShowRow(json) => self.row_json = Some(json),
            Message::CloseRow => self.row_json = None,
            Message::UseSuggest(index) => {
                if let Some(hit) = self.suggest.get(index).cloned() {
                    self.use_suggest(hit);
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        view::view(self)
    }

    fn menu_act(&mut self, menu: &MenuAt, label: &'static str) -> Task<Message> {
        let parent = parent_dir(&menu.path, &menu.kind);
        match label {
            "Run file" => {
                let text = if menu.path == self.open_path {
                    self.save_editor();
                    editor_text(&self.editor)
                } else {
                    lock(&self.studio)
                        .read_file(&menu.path)
                        .map(|file| file.text)
                        .unwrap_or_default()
                };
                return self.start_run(vec![(menu.path.clone(), text)]);
            }
            "Run folder" => {
                self.save_editor();
                let files = lock(&self.studio)
                    .dql_files_under(&menu.path)
                    .unwrap_or_default();
                if files.is_empty() {
                    self.status = "That folder has no .dql files.".to_string();
                    self.running_path.clear();
                    return Task::none();
                }
                let mut jobs = Vec::new();
                for path in files {
                    let text = if path == self.open_path {
                        editor_text(&self.editor)
                    } else {
                        lock(&self.studio)
                            .read_file(&path)
                            .map(|file| file.text)
                            .unwrap_or_default()
                    };
                    jobs.push((path, text));
                }
                return self.start_run(jobs);
            }
            "New file" => {
                self.dialog = Dialog::Name {
                    title: "New file",
                    parent,
                    dir: false,
                    previous: None,
                };
                self.dialog_text = "new.dql".to_string();
            }
            "New folder" => {
                self.dialog = Dialog::Name {
                    title: "New folder",
                    parent,
                    dir: true,
                    previous: None,
                };
                self.dialog_text = "folder".to_string();
            }
            "Duplicate" => {
                let duplicated = {
                    let studio = lock(&self.studio);
                    studio.duplicate_path(&menu.path)
                };
                match duplicated {
                    Ok((path, kind)) => {
                        self.refresh_tree();
                        self.status = format!("Duplicated to {path}");
                        if kind == "file" {
                            self.open_file(path);
                        }
                    }
                    Err(err) => self.status = err,
                }
            }
            "Rename" => {
                let name = menu
                    .path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&menu.path)
                    .to_string();
                self.dialog = Dialog::Name {
                    title: "Rename",
                    parent,
                    dir: menu.kind == "dir",
                    previous: Some(menu.path.clone()),
                };
                self.dialog_text = name;
            }
            "Move" => {
                self.dialog = Dialog::Move {
                    path: menu.path.clone(),
                };
                self.dialog_text = parent_dir(&menu.path, "file");
            }
            "Delete" => {
                let deleted = {
                    let studio = lock(&self.studio);
                    studio.delete_path(&menu.path)
                };
                match deleted {
                    Ok(()) => {
                        if self.open_path == menu.path {
                            self.open_path.clear();
                            self.editor = text_editor::Content::with_text("");
                            self.results.clear();
                        }
                        self.refresh_tree();
                        self.status = "Deleted.".to_string();
                    }
                    Err(err) => self.status = err,
                }
            }
            "Clear results" => {
                let cleared = {
                    let studio = lock(&self.studio);
                    studio.clear_results(&menu.path)
                };
                match cleared {
                    Ok(paths) => {
                        if paths.iter().any(|path| path == &self.open_path) {
                            self.results.clear();
                            self.open_cards.clear();
                        }
                        self.status = format!("Cleared results for {} file(s).", paths.len());
                    }
                    Err(err) => self.status = err,
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn confirm_dialog(&mut self) -> Task<Message> {
        let dialog = self.dialog.clone();
        let name = self.dialog_text.trim().to_string();
        self.dialog = Dialog::None;
        if name.is_empty() {
            self.status = "A name is required.".to_string();
            return Task::none();
        }
        match dialog {
            Dialog::Name {
                parent,
                dir,
                previous,
                ..
            } => {
                let relative = join_rel(&parent, &name);
                if let Some(previous) = previous {
                    let moved = {
                        let studio = lock(&self.studio);
                        studio.move_path(&previous, &relative)
                    };
                    match moved {
                        Ok((path, _)) => {
                            if self.open_path == previous {
                                self.open_path = path.clone();
                            }
                            self.refresh_tree();
                            self.status = format!("Renamed to {path}");
                        }
                        Err(err) => self.status = err,
                    }
                } else if dir {
                    let created = {
                        let studio = lock(&self.studio);
                        studio.make_dir(&relative)
                    };
                    match created {
                        Ok(path) => {
                            self.refresh_tree();
                            self.status = format!("Created {path}");
                        }
                        Err(err) => self.status = err,
                    }
                } else {
                    let relative = if relative.ends_with(".dql") {
                        relative
                    } else {
                        format!("{relative}.dql")
                    };
                    let created = {
                        let studio = lock(&self.studio);
                        studio.create_file(&relative)
                    };
                    match created {
                        Ok(path) => {
                            self.refresh_tree();
                            self.open_file(path);
                        }
                        Err(err) => self.status = err,
                    }
                }
            }
            Dialog::Move { path } => {
                let file_name = path.rsplit('/').next().unwrap_or(&path);
                let dest = if name.ends_with(".dql") || name.ends_with('/') {
                    name.trim_end_matches('/').to_string()
                } else {
                    join_rel(&name, file_name)
                };
                let moved = {
                    let studio = lock(&self.studio);
                    studio.move_path(&path, &dest)
                };
                match moved {
                    Ok((moved, _)) => {
                        if self.open_path == path {
                            self.open_path = moved.clone();
                        }
                        self.refresh_tree();
                        self.status = format!("Moved to {moved}");
                    }
                    Err(err) => self.status = err,
                }
            }
            Dialog::Conflict | Dialog::None => {}
        }
        self.dialog_text.clear();
        Task::none()
    }

    fn start_run(&mut self, jobs: Vec<(String, String)>) -> Task<Message> {
        let jobs: Vec<_> = jobs
            .into_iter()
            .filter(|(_, text)| studio::statement_at(text, 0).is_some() || text.contains(';'))
            .collect();
        if jobs.is_empty() || jobs.iter().all(|(_, text)| text.trim().is_empty()) {
            self.running_path.clear();
            self.status = "Nothing to run.".to_string();
            return Task::none();
        }
        self.cancel.clear();
        self.running_path = jobs[0].0.clone();
        self.status = format!("Running {}", self.running_path);
        let studio = Arc::clone(&self.studio);
        let progress = Arc::clone(&self.progress);
        Task::perform(
            async move {
                let handle = std::thread::spawn(move || {
                    let mut cards = Vec::new();
                    for (path, text) in jobs {
                        {
                            let mut live = progress.lock().unwrap_or_else(|err| err.into_inner());
                            live.path = path.clone();
                            live.phase = "running".to_string();
                            live.index = 0;
                            live.done = 0;
                            live.total = None;
                        }
                        let batch = {
                            let mut studio = studio.lock().unwrap_or_else(|err| err.into_inner());
                            studio.run_text(&path, &text, &progress)
                        };
                        let failed = batch.iter().any(|card| !card.ok);
                        cards.extend(batch);
                        if failed {
                            break;
                        }
                    }
                    cards
                });
                handle.join().unwrap_or_default()
            },
            Message::RunDone,
        )
    }

    fn open_file(&mut self, path: String) {
        self.save_editor();
        let read = {
            let studio = lock(&self.studio);
            studio.read_file(&path)
        };
        match read {
            Ok(file) => {
                self.editor = text_editor::Content::with_text(&file.text);
                self.open_path = file.path;
                self.revision = file.revision;
                self.dirty = false;
                self.conflict_text = None;
                self.dialog = Dialog::None;
                let results = {
                    let studio = lock(&self.studio);
                    studio.load_results(&self.open_path).unwrap_or_default()
                };
                self.results = results;
                self.open_cards = vec![false; self.results.len()];
                self.mode = Mode::Query;
                self.suggest.clear();
                self.refresh_lint();
                self.status.clear();
            }
            Err(err) => self.status = err,
        }
    }

    fn save_editor(&mut self) {
        if self.open_path.is_empty() || self.conflict_text.is_some() {
            return;
        }
        let text = editor_text(&self.editor);
        let base = if self.revision.is_empty() {
            None
        } else {
            Some(self.revision.as_str())
        };
        match lock(&self.studio).save_file(&self.open_path, &text, base) {
            Ok(SaveFile::Stored(file)) => {
                self.revision = file.revision;
                self.dirty = false;
            }
            Ok(SaveFile::Conflict(file)) => {
                self.conflict_text = Some(file.text);
                self.dialog = Dialog::Conflict;
                self.status = "The file changed on disk.".to_string();
            }
            Err(err) => self.status = err,
        }
    }

    fn refresh_lint(&mut self) {
        let text = editor_text(&self.editor);
        self.lint = lock(&self.studio).lint(&text).issues;
    }

    fn refresh_suggest(&mut self) {
        let text = editor_text(&self.editor);
        let at = cursor_byte(&self.editor, &text);
        let token = token_at(&text, at);
        self.suggest = if token.chars().count() < 2 {
            Vec::new()
        } else {
            lock(&self.studio).search(&token).items
        };
    }

    fn use_suggest(&mut self, hit: SearchHit) {
        if hit.kind == "query" {
            if let Some(path) = hit.path {
                self.suggest.clear();
                self.open_file(path);
                return;
            }
        }
        let name = hit.table.unwrap_or(hit.title);
        self.editor
            .perform(text_editor::Action::Edit(text_editor::Edit::Paste(
                Arc::new(name),
            )));
        self.suggest.clear();
        if !self.open_path.is_empty() {
            self.dirty = true;
            self.save_editor();
            self.refresh_lint();
        }
    }

    fn refresh_tree(&mut self) {
        self.tree = lock(&self.studio).tree().ok();
    }

    fn watch_disk(&mut self) {
        if self.open_path.is_empty() || !self.running_path.is_empty() {
            return;
        }
        let Ok(file) = lock(&self.studio).read_file(&self.open_path) else {
            return;
        };
        if file.revision == self.revision {
            return;
        }
        if self.dirty {
            self.conflict_text = Some(file.text);
            self.dialog = Dialog::Conflict;
            self.status = "The file changed on disk.".to_string();
            return;
        }
        self.editor = text_editor::Content::with_text(&file.text);
        self.revision = file.revision;
        self.refresh_lint();
    }

    fn open_table(&mut self, name: String) {
        self.mode = Mode::Tables;
        self.open_table = name;
        self.table_page = 0;
        self.load_table_page();
    }

    fn load_table_page(&mut self) {
        match lock(&self.studio).table_page(&self.open_table, self.table_page) {
            Ok(page) => {
                self.table_page = page.page;
                self.table_items = page.items;
                self.table_columns = page.columns;
                self.table_has_more = page.has_more;
                self.status = format!("{} · {} rows per page", page.name, page.page_size);
            }
            Err(err) => self.status = err,
        }
    }
}

fn lock(studio: &Arc<Mutex<Studio>>) -> std::sync::MutexGuard<'_, Studio> {
    studio.lock().unwrap_or_else(|err| err.into_inner())
}

fn lock_progress(progress: &Arc<Mutex<LiveProgress>>) -> LiveProgress {
    progress
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clone()
}

fn parent_dir(path: &str, kind: &str) -> String {
    if kind == "root" {
        return String::new();
    }
    if kind == "dir" {
        return path.to_string();
    }
    match path.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => String::new(),
    }
}

fn join_rel(parent: &str, name: &str) -> String {
    let name = name.trim().trim_matches('/');
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

fn editor_text(content: &text_editor::Content) -> String {
    let mut lines = Vec::new();
    for line in content.lines() {
        lines.push(line.text.to_string());
    }
    lines.join("\n")
}

fn cursor_byte(content: &text_editor::Content, text: &str) -> usize {
    let cursor = content.cursor();
    let (line, column) = cursor_line_column(&cursor);
    let mut byte = 0usize;
    for (index, row) in text.split_inclusive('\n').enumerate() {
        if index == line {
            for (seen, (offset, _ch)) in row.char_indices().enumerate() {
                if seen == column {
                    return byte + offset;
                }
            }
            return byte + row.len();
        }
        byte += row.len();
    }
    text.len()
}

fn cursor_line_column(cursor: &impl std::fmt::Debug) -> (usize, usize) {
    let rendered = format!("{cursor:?}");
    parse_line_column(&rendered).unwrap_or((0, 0))
}

fn parse_line_column(rendered: &str) -> Option<(usize, usize)> {
    let line = number_after(rendered, "line:")?;
    let column = number_after(rendered, "column:").or_else(|| number_after(rendered, "index:"))?;
    Some((line, column))
}

fn token_at(text: &str, byte: usize) -> String {
    let end = byte.min(text.len());
    let bytes = text.as_bytes();
    let mut start = end;
    while start > 0 && is_name_byte(bytes[start - 1]) {
        start -= 1;
    }
    let mut stop = end;
    while stop < bytes.len() && is_name_byte(bytes[stop]) {
        stop += 1;
    }
    text[start..stop].to_string()
}

fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn number_after(text: &str, label: &str) -> Option<usize> {
    let rest = text.split(label).nth(1)?;
    let digits: String = rest
        .trim_start()
        .chars()
        .skip_while(|ch| !ch.is_ascii_digit())
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn keyboard_subscription() -> Subscription<Message> {
    iced::event::listen_with(|event, _status, _id| match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
            let command = modifiers.control() || modifiers.command();
            if command && key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) {
                return Some(Message::RunCursor);
            }
            if let iced::keyboard::Key::Character(ch) = &key {
                if command && (ch.as_str() == "\\" || ch.as_str() == "|") {
                    return Some(Message::RunCursor);
                }
                if command && ch.as_str().eq_ignore_ascii_case("s") {
                    return Some(Message::Save);
                }
            }
            if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) {
                return Some(Message::Stop);
            }
            None
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> (Scratch, App) {
        let dir = Scratch::new();
        std::fs::write(dir.path.join("widgets.dql"), "SELECT 1;\n").unwrap();
        std::env::set_var("DQLRS_UI_PREFS", dir.path.join("ui.json"));
        let studio = Studio::memory(&dir.path).unwrap();
        let app = App::new(
            Arc::new(Mutex::new(studio)),
            Arc::new(Mutex::new(LiveProgress::default())),
            Prefs::default(),
        );
        (dir, app)
    }

    fn click(app: &mut App, label: &str) {
        std::env::set_var("ICED_TEST_BACKEND", "tiny-skia");
        let mut simulator = iced_test::Simulator::new(app.view());
        simulator
            .click(label)
            .unwrap_or_else(|err| panic!("click {label}: {err:?}"));
        let messages: Vec<Message> = simulator.into_messages().collect();
        assert!(!messages.is_empty(), "no message from {label}");
        for message in messages {
            let _ = app.update(message);
        }
    }

    #[test]
    fn window_covers_file_menu_run_conflict_and_theme() {
        let (dir, mut app) = workspace();
        click(&mut app, "Files");
        assert_eq!(
            app.menu.as_ref().map(|menu| menu.kind.as_str()),
            Some("root")
        );
        click(&mut app, "New file");
        assert!(matches!(app.dialog, Dialog::Name { dir: false, .. }));
        let _ = app.update(Message::DialogOk);
        assert!(dir.path.join("new.dql").is_file());

        click(&mut app, "widgets.dql");
        assert_eq!(app.open_path, "widgets.dql");
        assert!(editor_text(&app.editor).contains("SELECT 1"));
        assert!(app.lint.iter().any(|issue| issue.severity == "error"));

        let script = "\
CREATE TABLE demo (id STRING HASH KEY);
INSERT INTO demo (id) VALUES ('a');
SELECT * FROM demo WHERE id = 'a';
";
        let cards = {
            let mut studio = lock(&app.studio);
            studio.run_text("widgets.dql", script, &app.progress)
        };
        assert!(cards.iter().all(|card| card.ok), "{cards:?}");
        let _ = app.update(Message::RunDone(cards));
        assert!(app.results.iter().any(|card| card.ok));
        assert!(app.status.contains("finished"));
        {
            std::env::set_var("ICED_TEST_BACKEND", "tiny-skia");
            let mut simulator = iced_test::Simulator::new(app.view());
            simulator
                .click("id")
                .unwrap_or_else(|err| panic!("header id: {err:?}"));
            simulator
                .click("a")
                .unwrap_or_else(|err| panic!("cell a: {err:?}"));
        }

        std::fs::write(dir.path.join("widgets.dql"), "SELECT 2;\n").unwrap();
        app.dirty = true;
        app.watch_disk();
        assert!(matches!(app.dialog, Dialog::Conflict));

        lock(&app.studio).make_dir("empty").unwrap();
        app.refresh_tree();
        let _ = app.update(Message::Menu("empty".to_string(), "dir".to_string()));
        let _ = app.update(Message::MenuAct("Run folder"));
        assert!(app.status.contains("no .dql"));

        std::fs::create_dir_all(dir.path.join("notes")).unwrap();
        std::fs::write(dir.path.join("notes/readme.txt"), "hi").unwrap();
        app.refresh_tree();
        let _ = app.update(Message::Menu("notes".to_string(), "dir".to_string()));
        let _ = app.update(Message::MenuAct("Delete"));
        assert!(dir.path.join("notes/readme.txt").is_file());

        let _ = app.update(Message::ToggleTheme);
        assert!(app.prefs.dark);
        assert!(dir.path.join("ui.json").is_file());
        let _ = app.update(Message::ToggleSettings);
        assert!(app.show_settings);
        let _ = app.update(Message::Mode(Mode::Tables));
        assert_eq!(app.mode, Mode::Tables);

        assert_eq!(token_at("SELECT widgets", 13), "widgets");
        assert_eq!(
            parse_line_column("Cursor { line: 2, column: 4 }"),
            Some((2, 4))
        );
    }

    struct Scratch {
        path: std::path::PathBuf,
    }

    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "dqlrs-ui-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
            if std::env::var_os("DQLRS_UI_PREFS")
                .is_some_and(|path| std::path::PathBuf::from(path).starts_with(&self.path))
            {
                std::env::remove_var("DQLRS_UI_PREFS");
            }
        }
    }
}
