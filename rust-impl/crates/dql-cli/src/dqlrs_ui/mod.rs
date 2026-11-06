//! Native window shipped inside the `dqlrs` binary.
//!
//! `dqlrs --ui` builds this iced interface and calls [`crate::web::studio`] in
//! process. There is no HTTP hop and no second install.

mod highlight;
mod icons;
mod query;
mod slot;
mod style;
mod view;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced::widget::{pane_grid, text_editor};
use iced::{Element, Rectangle, Subscription, Task, Theme};
use serde::{Deserialize, Serialize};

use crate::session::Session;
use crate::web::files::{Node, SaveFile};
use crate::web::lint::LintIssue;
use crate::web::search::SearchHit;
use crate::web::shape::{Column, ListedTable, TableCacheHint};
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
    #[serde(default)]
    minimal_write: bool,
    #[serde(default = "default_even_odd")]
    even_odd: bool,
    /// File this value was loaded from. Later saves use it so a changed
    /// `DQLRS_UI_PREFS` cannot redirect the write.
    #[serde(skip)]
    path: PathBuf,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            dark: false,
            nav: 260.0,
            results: 280.0,
            write_bands: true,
            minimal_write: false,
            even_odd: true,
            path: PathBuf::new(),
        }
    }
}

impl Prefs {
    fn load() -> Self {
        let path = prefs_path();
        let mut prefs: Self = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        prefs.path = path;
        prefs
    }

    fn store(&self) {
        let path = if self.path.as_os_str().is_empty() {
            prefs_path()
        } else {
            self.path.clone()
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string(self) {
            let _ = std::fs::write(path, text);
        }
    }
}

fn default_even_odd() -> bool {
    true
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
    at: iced::Point,
}

#[derive(Clone, Debug)]
enum SchemaKind {
    Result(usize),
    Table,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Side,
    Editor,
    Results,
    Tables,
}

#[derive(Clone, Debug)]
struct Notice {
    title: String,
    body: String,
    seconds: u8,
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
    search_open: bool,
    search_active: Option<usize>,
    search_after: Option<std::time::Instant>,
    search_bounds: Rectangle,
    suggest: Vec<SearchHit>,
    menu: Option<MenuAt>,
    dialog: Dialog,
    dialog_text: String,
    tables: Vec<ListedTable>,
    table_note: String,
    cache_text: String,
    cache: TableCacheHint,
    cache_note: String,
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
    schema: Option<SchemaKind>,
    status: String,
    query_panes: pane_grid::State<Slot>,
    table_panes: pane_grid::State<Slot>,
    window_width: f32,
    window_height: f32,
    pointer: iced::Point,
    notices: Vec<Notice>,
    editor_scroll: iced::widget::Id,
    editor_offset_x: f32,
    editor_offset_y: f32,
    editor_view_w: f32,
    editor_view_h: f32,
    editor_jump_top: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Mode(Mode),
    Search(String),
    SearchTick,
    SearchMove(i32),
    SearchSubmit,
    SearchDismiss,
    SearchBounds(Rectangle),
    PickHit(usize),
    Open(String),
    Menu(String, String),
    MenuRoot,
    MenuAct(&'static str),
    Editor(text_editor::Action),
    Save,
    ReloadDisk,
    KeepEdits,
    RunFile,
    RunCursor,
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
    SetMinimalWrite(bool),
    SetEvenOdd(bool),
    ToggleCard(usize),
    ShowRow(String),
    CloseRow,
    ShowResultSchema(usize),
    ShowTableSchema,
    CloseSchema,
    UseSuggest(usize),
    PaneRatio(pane_grid::Split, f32),
    SyncGrid(String, f32),
    WindowSized(iced::Size),
    Pointer(iced::Point),
    NoticeTick,
    FocusShift(bool),
    EditorScrolled(iced::widget::scrollable::Viewport),
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
        let query_panes = query_layout(&prefs, 1280.0, 800.0);
        let table_panes = table_layout(&prefs, 1280.0);
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
            search_open: false,
            search_active: None,
            search_after: None,
            search_bounds: Rectangle::default(),
            suggest: Vec::new(),
            menu: None,
            dialog: Dialog::None,
            dialog_text: String::new(),
            tables: Vec::new(),
            table_note: String::new(),
            cache_text: String::new(),
            cache: TableCacheHint::default(),
            cache_note: String::new(),
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
            schema: None,
            status: String::new(),
            query_panes,
            table_panes,
            window_width: 1280.0,
            window_height: 800.0,
            pointer: iced::Point::new(16.0, 48.0),
            notices: Vec::new(),
            editor_scroll: iced::widget::Id::unique(),
            editor_offset_x: 0.0,
            editor_offset_y: 0.0,
            editor_view_w: 0.0,
            editor_view_h: 0.0,
            editor_jump_top: false,
        };
        app.refresh_tree();
        app
    }

    fn theme(&self) -> Theme {
        let colors = style::palette(self.prefs.dark);
        Theme::custom(
            if self.prefs.dark {
                "dqlrs-dark"
            } else {
                "dqlrs-light"
            },
            iced::theme::Palette {
                background: colors.paper,
                text: colors.text,
                primary: colors.teal,
                success: colors.ok,
                warning: colors.amber,
                danger: colors.danger,
            },
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            keyboard_subscription(),
            iced::window::resize_events().map(|(_id, size)| Message::WindowSized(size)),
        ];
        if !self.running_path.is_empty() {
            subs.push(iced::time::every(Duration::from_millis(180)).map(|_| Message::Pulse));
            subs.push(iced::time::every(Duration::from_millis(120)).map(|_| Message::Tick));
        }
        subs.push(iced::time::every(Duration::from_secs(1)).map(|_| Message::Watch));
        if !self.notices.is_empty() {
            subs.push(iced::time::every(Duration::from_secs(1)).map(|_| Message::NoticeTick));
        }
        if self.search_after.is_some() {
            subs.push(iced::time::every(Duration::from_millis(50)).map(|_| Message::SearchTick));
        }
        Subscription::batch(subs)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.dispatch(message);
        self.with_editor_jump(task)
    }

    fn with_editor_jump(&mut self, task: Task<Message>) -> Task<Message> {
        if !self.editor_jump_top {
            return task;
        }
        self.editor_jump_top = false;
        self.editor_offset_x = 0.0;
        self.editor_offset_y = 0.0;
        Task::batch([
            task,
            iced::widget::operation::scroll_to(
                self.editor_scroll.clone(),
                iced::widget::operation::AbsoluteOffset {
                    x: Some(0.0),
                    y: Some(0.0),
                },
            ),
        ])
    }

    fn dispatch(&mut self, message: Message) -> Task<Message> {
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
                self.search_active = None;
                if self.search.trim().is_empty() {
                    self.search_after = None;
                    self.search_open = false;
                    self.hits.clear();
                } else {
                    self.search_after =
                        Some(std::time::Instant::now() + Duration::from_millis(150));
                }
            }
            Message::SearchTick => {
                let due = self
                    .search_after
                    .is_some_and(|due| due <= std::time::Instant::now());
                if due {
                    self.search_after = None;
                    self.run_title_search();
                }
            }
            Message::SearchMove(delta) => self.move_search(delta),
            Message::SearchSubmit => return self.submit_search(),
            Message::SearchDismiss => self.dismiss_search(),
            Message::SearchBounds(bounds) => {
                if bounds.width > 1.0 && bounds.height > 1.0 {
                    self.search_bounds = bounds;
                }
            }
            Message::PickHit(index) => {
                let hit = self.hits.get(index).cloned();
                self.hide_search_list();
                self.suggest.clear();
                if let Some(hit) = hit {
                    return self.open_search_hit(hit);
                }
            }
            Message::Open(path) => {
                if path.ends_with(".dql") {
                    self.open_file(path);
                }
                self.menu = None;
            }
            Message::Menu(path, kind) => {
                self.menu = Some(MenuAt {
                    path,
                    kind,
                    at: self.pointer,
                });
            }
            Message::MenuRoot => {
                self.menu = Some(MenuAt {
                    path: String::new(),
                    kind: "root".to_string(),
                    at: self.pointer,
                });
            }
            Message::MenuAct(label) => {
                let menu = self.menu.clone().unwrap_or(MenuAt {
                    path: String::new(),
                    kind: "root".to_string(),
                    at: self.pointer,
                });
                self.menu = None;
                return self.menu_act(&menu, label);
            }
            Message::Editor(action) => {
                if let text_editor::Action::Scroll { lines } = action {
                    return self.scroll_editor(lines);
                }
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
                return self.follow_cursor();
            }
            Message::EditorScrolled(viewport) => {
                let offset = viewport.absolute_offset();
                self.editor_offset_x = offset.x;
                self.editor_offset_y = offset.y;
                self.editor_view_w = viewport.bounds().width;
                self.editor_view_h = viewport.bounds().height;
            }
            Message::Save => {
                self.save_editor();
                if self.conflict_text.is_none() && !self.open_path.is_empty() {
                    self.toast("Saved", format!("Saved {}", self.open_path));
                }
            }
            Message::ReloadDisk => {
                if let Some(text) = self.conflict_text.clone() {
                    self.editor = text_editor::Content::with_text(&text);
                    self.editor_jump_top = true;
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
                let (head, anchor) = caret_bytes(&self.editor, &text);
                if let Some(target) = query::run_target(&text, head, anchor) {
                    let slice = text.get(target.from..target.to).unwrap_or("").to_string();
                    self.save_editor();
                    return self.start_run(vec![(self.open_path.clone(), slice)]);
                }
                self.toast("Nothing to run", "No statement at the cursor.".to_string());
            }
            Message::Stop => {
                if self.search_open {
                    self.hide_search_list();
                    return Task::none();
                }
                if self.schema.is_some()
                    || self.row_json.is_some()
                    || self.menu.is_some()
                    || self.show_settings
                {
                    self.schema = None;
                    self.row_json = None;
                    self.menu = None;
                    self.show_settings = false;
                } else {
                    self.cancel.request();
                    self.status = "Stopping…".to_string();
                }
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
                    self.toast("Nothing to run", "Nothing to run.".to_string());
                } else {
                    let failed = cards
                        .iter()
                        .find(|card| !card.ok)
                        .map(|card| card.failure_text());
                    if let Some(err) = failed {
                        self.toast("Run failed", format!("A statement failed. {err}"));
                    } else {
                        self.status = format!("{} statement(s) finished.", cards.len());
                    }
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
                        self.cache_text = list.cache.text.clone();
                        self.cache = list.cache;
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
                self.cache_note = "Table cache cleared.".to_string();
                self.cache = lock(&self.studio).table_cache_status();
                return self.update(Message::RefreshTables(true));
            }
            Message::DialogText(text) => self.dialog_text = text,
            Message::DialogOk => return self.confirm_dialog(),
            Message::DialogCancel => {
                self.dialog = Dialog::None;
                self.dialog_text.clear();
            }
            Message::ToggleSettings => {
                self.show_settings = !self.show_settings;
                if self.show_settings {
                    self.cache_note.clear();
                    self.cache = lock(&self.studio).table_cache_status();
                }
            }
            Message::ToggleEnv => self.show_env = !self.show_env,
            Message::ToggleTheme => {
                self.prefs.dark = !self.prefs.dark;
                self.prefs.store();
            }
            Message::SetMinimalWrite(on) => {
                self.prefs.minimal_write = on;
                self.prefs.store();
            }
            Message::SetEvenOdd(on) => {
                self.prefs.even_odd = on;
                self.prefs.store();
            }
            Message::ToggleCard(index) => {
                if let Some(open) = self.open_cards.get_mut(index) {
                    *open = !*open;
                }
            }
            Message::ShowRow(json) => {
                self.schema = None;
                self.row_json = Some(json);
            }
            Message::CloseRow => self.row_json = None,
            Message::ShowResultSchema(index) => {
                self.row_json = None;
                self.schema = Some(SchemaKind::Result(index));
            }
            Message::ShowTableSchema => {
                self.row_json = None;
                self.schema = Some(SchemaKind::Table);
            }
            Message::CloseSchema => self.schema = None,
            Message::UseSuggest(index) => {
                if let Some(hit) = self.suggest.get(index).cloned() {
                    self.use_suggest(hit);
                }
            }
            Message::PaneRatio(split, ratio) => self.resize_panes(split, ratio),
            Message::SyncGrid(id, x) => {
                return iced::widget::operation::scroll_to(
                    iced::widget::Id::from(id),
                    iced::widget::operation::AbsoluteOffset {
                        x: Some(x),
                        y: None,
                    },
                );
            }
            Message::WindowSized(size) => {
                self.window_width = size.width;
                self.window_height = size.height;
            }
            Message::Pointer(point) => self.pointer = point,
            Message::NoticeTick => {
                for notice in &mut self.notices {
                    notice.seconds = notice.seconds.saturating_sub(1);
                }
                self.notices.retain(|notice| notice.seconds > 0);
            }
            Message::FocusShift(back) => {
                let open = !matches!(self.dialog, Dialog::None)
                    || self.menu.is_some()
                    || self.schema.is_some()
                    || self.row_json.is_some()
                    || self.show_settings
                    || self.show_env;
                if open {
                    return if back {
                        iced::widget::operation::focus_previous()
                    } else {
                        iced::widget::operation::focus_next()
                    };
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
                    self.toast(
                        "Nothing to run",
                        "That folder has no .dql files.".to_string(),
                    );
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
                return iced::widget::operation::focus_next();
            }
            "New folder" => {
                self.dialog = Dialog::Name {
                    title: "New folder",
                    parent,
                    dir: true,
                    previous: None,
                };
                self.dialog_text = "folder".to_string();
                return iced::widget::operation::focus_next();
            }
            "Duplicate" => {
                let duplicated = {
                    let studio = lock(&self.studio);
                    studio.duplicate_path(&menu.path)
                };
                match duplicated {
                    Ok((path, kind)) => {
                        self.refresh_tree();
                        self.toast("Duplicated", format!("Duplicated to {path}"));
                        if kind == "file" {
                            self.open_file(path);
                        }
                    }
                    Err(err) => self.toast("Duplicate failed", err),
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
                return iced::widget::operation::focus_next();
            }
            "Move" => {
                self.dialog = Dialog::Move {
                    path: menu.path.clone(),
                };
                self.dialog_text = parent_dir(&menu.path, "file");
                return iced::widget::operation::focus_next();
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
                        self.toast("Deleted", "Deleted.".to_string());
                    }
                    Err(err) => self.toast("Delete failed", err),
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
                        self.toast(
                            "Results cleared",
                            format!("Cleared results for {} file(s).", paths.len()),
                        );
                    }
                    Err(err) => self.toast("Clear failed", err),
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
            self.toast("Name required", "A name is required.".to_string());
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
                            self.toast("Renamed", format!("Renamed to {path}"));
                        }
                        Err(err) => self.toast("Rename failed", err),
                    }
                } else if dir {
                    let created = {
                        let studio = lock(&self.studio);
                        studio.make_dir(&relative)
                    };
                    match created {
                        Ok(path) => {
                            self.refresh_tree();
                            self.toast("Created", format!("Created {path}"));
                        }
                        Err(err) => self.toast("Create failed", err),
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
                            self.open_file(path.clone());
                            self.toast("Created", format!("Created {path}"));
                        }
                        Err(err) => self.toast("Create failed", err),
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
                        self.toast("Moved", format!("Moved to {moved}"));
                    }
                    Err(err) => self.toast("Move failed", err),
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
            self.toast("Nothing to run", "Nothing to run.".to_string());
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

    fn hide_search_list(&mut self) {
        self.search_open = false;
        self.search_active = None;
    }

    fn run_title_search(&mut self) {
        let query = self.search.trim().to_string();
        if query.is_empty() {
            self.hits.clear();
            self.hide_search_list();
            return;
        }
        self.hits = lock(&self.studio).search(&query).items;
        self.search_open = true;
        self.search_active = None;
    }

    fn move_search(&mut self, delta: i32) {
        if !self.search_open {
            return;
        }
        let order = search_order(&self.hits);
        if order.is_empty() || delta == 0 {
            return;
        }
        let len = order.len() as i32;
        let next = match self.search_active {
            Some(current) => (current as i32 + delta).rem_euclid(len),
            None if delta > 0 => 0,
            None => len - 1,
        };
        self.search_active = Some(next as usize);
    }

    fn submit_search(&mut self) -> Task<Message> {
        let order = search_order(&self.hits);
        let Some(slot) = self.search_active else {
            return Task::none();
        };
        let Some(index) = order.get(slot).copied() else {
            return Task::none();
        };
        let Some(hit) = self.hits.get(index).cloned() else {
            return Task::none();
        };
        self.hide_search_list();
        self.suggest.clear();
        self.open_search_hit(hit)
    }

    fn dismiss_search(&mut self) {
        let at = self.pointer;
        if self.search_bounds.contains(at) {
            if !self.search.trim().is_empty() && self.search_after.is_none() {
                self.run_title_search();
            }
            return;
        }
        if !self.search_open {
            return;
        }
        let popup = view::search_popup_rect(self.search_bounds, &self.hits, self.window_height);
        if popup.contains(at) {
            return;
        }
        self.hide_search_list();
    }

    fn open_search_hit(&mut self, hit: SearchHit) -> Task<Message> {
        if hit.kind == "query" {
            let Some(path) = hit.path else {
                return Task::none();
            };
            self.open_file(path);
            let Some(offset) = hit.offset else {
                return Task::none();
            };
            self.editor_jump_top = false;
            let text = editor_text(&self.editor);
            let at = statement_code_pos(&text, offset);
            let (line, column) = char_line_column(&text, at);
            self.editor.move_to(text_editor::Cursor {
                position: text_editor::Position { line, column },
                selection: None,
            });
            return self.scroll_line_centered(line);
        }
        let Some(table) = hit.table else {
            return Task::none();
        };
        self.table_filter.clear();
        self.open_table(table);
        self.update(Message::RefreshTables(false))
    }

    fn scroll_line_centered(&self, line: usize) -> Task<Message> {
        let line_top = query::PAD_TOP + line as f32 * query::LINE_H;
        let view = self.editor_view_h.max(query::LINE_H);
        let y = (line_top - (view - query::LINE_H) / 2.0).max(0.0);
        iced::widget::operation::scroll_to(
            self.editor_scroll.clone(),
            iced::widget::operation::AbsoluteOffset {
                x: Some(0.0),
                y: Some(y),
            },
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
                self.editor_jump_top = true;
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
        let saved = {
            let studio = lock(&self.studio);
            studio.save_file(&self.open_path, &text, base)
        };
        match saved {
            Ok(SaveFile::Stored(file)) => {
                self.revision = file.revision;
                self.dirty = false;
            }
            Ok(SaveFile::Conflict(file)) => {
                self.conflict_text = Some(file.text);
                self.dialog = Dialog::Conflict;
                self.toast("Conflict", "The file changed on disk.".to_string());
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

    fn toast(&mut self, title: &str, body: String) {
        if self.notices.len() >= 3 {
            self.notices.remove(0);
        }
        self.notices.push(Notice {
            title: title.to_string(),
            body: body.clone(),
            seconds: 6,
        });
        self.status = body;
    }

    fn resize_panes(&mut self, split: pane_grid::Split, ratio: f32) {
        let width = self.window_width.max(640.0);
        let height = self.window_height.max(480.0);
        let axis = match self.mode {
            Mode::Query => split_axis(self.query_panes.layout(), split),
            Mode::Tables => split_axis(self.table_panes.layout(), split),
        };
        match self.mode {
            Mode::Query => {
                self.query_panes.resize(split, ratio);
            }
            Mode::Tables => {
                self.table_panes.resize(split, ratio);
            }
        }
        match axis {
            Some(pane_grid::Axis::Vertical) => {
                self.prefs.nav = (ratio * width).clamp(160.0, 520.0);
                let nav_ratio = (self.prefs.nav / width).clamp(0.16, 0.45);
                if let Some(other) = match self.mode {
                    Mode::Query => vertical_split(self.table_panes.layout()),
                    Mode::Tables => vertical_split(self.query_panes.layout()),
                } {
                    match self.mode {
                        Mode::Query => self.table_panes.resize(other, nav_ratio),
                        Mode::Tables => self.query_panes.resize(other, nav_ratio),
                    };
                }
            }
            Some(pane_grid::Axis::Horizontal) => {
                let usable = (height - 72.0).max(320.0);
                self.prefs.results = ((1.0 - ratio) * usable).clamp(120.0, 640.0);
            }
            None => {}
        }
        self.prefs.store();
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
            self.toast("Conflict", "The file changed on disk.".to_string());
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
    content.text()
}

fn caret_bytes(content: &text_editor::Content, text: &str) -> (usize, usize) {
    let cursor = content.cursor();
    let head = position_byte(text, cursor.position.line, cursor.position.column);
    let anchor = match cursor.selection {
        Some(position) => position_byte(text, position.line, position.column),
        None => head,
    };
    (head, anchor)
}

fn cursor_byte(content: &text_editor::Content, text: &str) -> usize {
    caret_bytes(content, text).0
}

fn search_order(hits: &[SearchHit]) -> Vec<usize> {
    const GROUPS: [&[&str]; 3] = [&["table"], &["column", "index"], &["query"]];
    let mut order = Vec::new();
    for kinds in GROUPS {
        for (index, hit) in hits.iter().enumerate() {
            if kinds.contains(&hit.kind.as_str()) {
                order.push(index);
            }
        }
    }
    order
}

/// First code character at or after `pos`, matching `statementCodePos`.
fn statement_code_pos(text: &str, pos: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let start = pos.min(chars.len());
    let mut index = start;
    while index < chars.len() {
        let ch = chars[index];
        let next = chars.get(index + 1).copied().unwrap_or('\0');
        if ch == '-' && next == '-' {
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            if index < chars.len() && chars[index] == '\n' {
                index += 1;
            }
            continue;
        }
        if ch == ';' {
            return start;
        }
        if !ch.is_whitespace() {
            return index;
        }
        index += 1;
    }
    start
}

fn char_line_column(text: &str, char_offset: usize) -> (usize, usize) {
    let mut seen = 0usize;
    for (line, row) in text.split_inclusive('\n').enumerate() {
        let count = row.chars().count();
        if seen + count > char_offset {
            return (line, char_offset - seen);
        }
        seen += count;
    }
    (text.split_inclusive('\n').count().saturating_sub(1), 0)
}

fn position_byte(text: &str, line: usize, column: usize) -> usize {
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

impl App {
    fn scroll_editor(&self, lines: i32) -> Task<Message> {
        if lines == 0 {
            return Task::none();
        }
        iced::widget::operation::scroll_by(
            self.editor_scroll.clone(),
            iced::widget::operation::AbsoluteOffset {
                x: 0.0,
                y: lines as f32 * query::LINE_H,
            },
        )
    }

    fn follow_cursor(&self) -> Task<Message> {
        if self.editor_view_h <= 0.0 {
            return Task::none();
        }
        let line = self.editor.cursor().position.line as f32;
        let top = query::PAD_TOP + line * query::LINE_H;
        let bottom = top + query::LINE_H;
        let offset = self.editor_offset_y;
        let y = if top < offset {
            Some(top)
        } else if bottom > offset + self.editor_view_h {
            Some((bottom - self.editor_view_h).max(0.0))
        } else {
            None
        };
        let Some(y) = y else {
            return Task::none();
        };
        iced::widget::operation::scroll_to(
            self.editor_scroll.clone(),
            iced::widget::operation::AbsoluteOffset {
                x: None,
                y: Some(y),
            },
        )
    }
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

fn query_layout(prefs: &Prefs, width: f32, height: f32) -> pane_grid::State<Slot> {
    let width = width.max(640.0);
    let usable = (height - 72.0).max(320.0);
    let nav = (prefs.nav / width).clamp(0.16, 0.45);
    let results = (prefs.results / usable).clamp(0.18, 0.55);
    pane_grid::State::with_configuration(pane_grid::Configuration::Split {
        axis: pane_grid::Axis::Vertical,
        ratio: nav,
        a: Box::new(pane_grid::Configuration::Pane(Slot::Side)),
        b: Box::new(pane_grid::Configuration::Split {
            axis: pane_grid::Axis::Horizontal,
            ratio: 1.0 - results,
            a: Box::new(pane_grid::Configuration::Pane(Slot::Editor)),
            b: Box::new(pane_grid::Configuration::Pane(Slot::Results)),
        }),
    })
}

fn table_layout(prefs: &Prefs, width: f32) -> pane_grid::State<Slot> {
    let width = width.max(640.0);
    let nav = (prefs.nav / width).clamp(0.16, 0.45);
    pane_grid::State::with_configuration(pane_grid::Configuration::Split {
        axis: pane_grid::Axis::Vertical,
        ratio: nav,
        a: Box::new(pane_grid::Configuration::Pane(Slot::Side)),
        b: Box::new(pane_grid::Configuration::Pane(Slot::Tables)),
    })
}

fn split_axis(node: &pane_grid::Node, split: pane_grid::Split) -> Option<pane_grid::Axis> {
    match node {
        pane_grid::Node::Split { id, axis, a, b, .. } => {
            if *id == split {
                Some(*axis)
            } else {
                split_axis(a, split).or_else(|| split_axis(b, split))
            }
        }
        pane_grid::Node::Pane(_) => None,
    }
}

fn vertical_split(node: &pane_grid::Node) -> Option<pane_grid::Split> {
    match node {
        pane_grid::Node::Split { id, axis, a, b, .. } => {
            if *axis == pane_grid::Axis::Vertical {
                Some(*id)
            } else {
                vertical_split(a).or_else(|| vertical_split(b))
            }
        }
        pane_grid::Node::Pane(_) => None,
    }
}

fn keyboard_subscription() -> Subscription<Message> {
    iced::event::listen_with(|event, status, _id| match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
            if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) {
                return Some(Message::Stop);
            }
            if status == iced::event::Status::Ignored {
                if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown) {
                    return Some(Message::SearchMove(1));
                }
                if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp) {
                    return Some(Message::SearchMove(-1));
                }
            }
            if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Tab)
                && status == iced::event::Status::Ignored
            {
                return Some(Message::FocusShift(modifiers.shift()));
            }
            if status == iced::event::Status::Captured {
                return None;
            }
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
            None
        }
        iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
            Some(Message::Pointer(position))
        }
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_)) => Some(Message::SearchDismiss),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefs_env_lock() -> std::sync::MutexGuard<'static, ()> {
        static PREFS_ENV: Mutex<()> = Mutex::new(());
        PREFS_ENV.lock().unwrap_or_else(|err| err.into_inner())
    }

    fn workspace() -> (Scratch, App) {
        // `DQLRS_UI_PREFS` is process-wide. Hold it only long enough to snapshot
        // the path into this window; later tests may replace the variable.
        let _guard = prefs_env_lock();
        let dir = Scratch::new();
        std::fs::write(dir.path.join("widgets.dql"), "SELECT 1;\n").unwrap();
        std::env::set_var("DQLRS_UI_PREFS", dir.path.join("ui.json"));
        let studio = Studio::memory(&dir.path).unwrap();
        let app = App::new(
            Arc::new(Mutex::new(studio)),
            Arc::new(Mutex::new(LiveProgress::default())),
            Prefs::load(),
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
    fn run_cursor_includes_attributed_comments_and_keeps_a_selection() {
        let (_dir, mut app) = workspace();
        let text = "-- note\nSELECT 1;\n\nSELECT 2;\n";
        app.open_path = "widgets.dql".to_string();
        app.editor = text_editor::Content::with_text(text);
        let (head, anchor) = caret_bytes(&app.editor, text);
        let target = query::run_target(text, head, anchor).unwrap();
        assert_eq!(target.kind, query::RunKind::Query);
        assert_eq!(&text[target.from..target.to], "-- note\nSELECT 1;\n\n");

        app.editor.move_to(text_editor::Cursor {
            position: text_editor::Position { line: 3, column: 7 },
            selection: Some(text_editor::Position { line: 3, column: 0 }),
        });
        let (head, anchor) = caret_bytes(&app.editor, text);
        let target = query::run_target(text, head, anchor).unwrap();
        assert_eq!(target.kind, query::RunKind::Selection);
        assert_eq!(&text[target.from..target.to], "SELECT ");
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
        assert!(app.cache_note.is_empty());
        let _ = app.update(Message::SetMinimalWrite(true));
        let _ = app.update(Message::SetEvenOdd(false));
        assert!(app.prefs.minimal_write);
        assert!(!app.prefs.even_odd);
        let _ = app.update(Message::ClearCache);
        assert_eq!(app.cache_note, "Table cache cleared.");
        let _ = app.update(Message::Stop);
        assert!(!app.show_settings);
        let _ = app.update(Message::Mode(Mode::Tables));
        assert_eq!(app.mode, Mode::Tables);

        assert_eq!(token_at("SELECT widgets", 13), "widgets");
        assert_eq!(app.editor.cursor().position.line, 0);
        assert_eq!(app.editor.cursor().position.column, 0);
    }

    #[test]
    fn title_search_opens_the_statement_and_keeps_the_query() {
        let (dir, mut app) = workspace();
        std::fs::create_dir_all(dir.path.join("queries")).unwrap();
        std::fs::write(
            dir.path.join("queries/read.dql"),
            "-- header stays with the select\nSELECT * FROM pw_missing_table\nWHERE id = 'a';\n",
        )
        .unwrap();
        let _ = app.update(Message::Search("pw_missing".to_string()));
        assert!(!app.search_open);
        app.search_after = Some(std::time::Instant::now() - Duration::from_millis(1));
        let _ = app.update(Message::SearchTick);
        assert!(app.search_open);
        let query = app
            .hits
            .iter()
            .find(|hit| hit.kind == "query")
            .expect("query hit");
        assert!(query.title.contains("pw_missing_table"));
        assert_eq!(query.detail, "queries/read.dql");
        let _ = app.update(Message::SearchMove(1));
        assert_eq!(app.search_active, Some(0));
        let _ = app.update(Message::SearchSubmit);
        assert!(!app.search_open);
        assert_eq!(app.search, "pw_missing");
        assert_eq!(app.open_path, "queries/read.dql");
        assert_eq!(app.editor.cursor().position.line, 1);
        assert_eq!(app.editor.cursor().position.column, 0);

        let _ = app.update(Message::Search("zz_no_such_column".to_string()));
        app.search_after = Some(std::time::Instant::now() - Duration::from_millis(1));
        let _ = app.update(Message::SearchTick);
        assert!(app.search_open);
        assert!(app.hits.is_empty());
        let _ = app.update(Message::Stop);
        assert!(!app.search_open);
        assert_eq!(app.search, "zz_no_such_column");
        let _ = app.update(Message::Search("   ".to_string()));
        assert!(!app.search_open);
        assert!(app.hits.is_empty());
    }

    #[test]
    fn structure_and_row_popups_follow_the_web() {
        let (_dir, mut app) = workspace();
        let script = "\
DROP TABLE IF EXISTS shape_demo;
CREATE TABLE shape_demo (id STRING HASH KEY, sk NUMBER RANGE KEY);
INSERT INTO shape_demo (id, sk, extra) VALUES ('a', 1, 'wide-sample'), ('b', 2, 'wide-sample');
INSERT INTO shape_demo (id, sk) VALUES ('c', 3);
SCAN * FROM shape_demo;
CREATE TABLE keyed_demo (id STRING HASH KEY, sk NUMBER RANGE KEY) GLOBAL INDEX ('by-n', n NUMBER, sk NUMBER);
INSERT INTO keyed_demo (id, sk, label, n) VALUES ('a', 2, 'alpha', 7);
SELECT * FROM keyed_demo WHERE n = 7 AND sk = 2 USING by-n;
";
        let cards = {
            let mut studio = lock(&app.studio);
            studio.run_text("widgets.dql", script, &app.progress)
        };
        assert!(cards.iter().all(|card| card.ok), "{cards:?}");
        let _ = app.update(Message::RunDone(cards));

        let scan = app
            .results
            .iter()
            .position(|card| card.statement.contains("SCAN * FROM shape_demo"))
            .expect("scan card");
        let _ = app.update(Message::ShowResultSchema(scan));
        for label in [
            "RESULT STRUCTURE",
            "shape_demo",
            "3 columns · 3 rows",
            "Hash key",
            "Range key",
            "STRING",
            "NUMBER",
            "3/3",
            "2/3",
            "wide-sample",
            "—",
            "Types come from this result. No extra query was run.",
            "SCAN",
        ] {
            show_text(&app, label);
        }
        let _ = app.update(Message::CloseSchema);
        assert!(app.schema.is_none());

        let select = app
            .results
            .iter()
            .position(|card| card.statement.contains("USING by-n"))
            .expect("index read");
        let json = serde_json::to_string_pretty(&app.results[select].items[0]).unwrap();
        let _ = app.update(Message::ShowRow(json));
        for label in ["ROW", "id = a", "sk = 2", "n = 7", "\"label\"", "\"alpha\""] {
            show_text(&app, label);
        }
        let _ = app.update(Message::Stop);
        assert!(app.row_json.is_none());

        let _ = app.update(Message::OpenTable("shape_demo".to_string()));
        let _ = app.update(Message::ShowTableSchema);
        show_text(&app, "TABLE STRUCTURE");
        show_text(&app, "3 columns · 3 rows on this page");
        show_text(
            &app,
            "Types come from this page and the keys already listed. No extra query was run.",
        );
    }

    fn show_text(app: &App, label: &str) {
        std::env::set_var("ICED_TEST_BACKEND", "tiny-skia");
        let mut simulator = iced_test::Simulator::new(app.view());
        simulator
            .find(label)
            .unwrap_or_else(|err| panic!("missing {label}: {err:?}"));
    }

    #[test]
    fn growing_table_list_keeps_each_row_state() {
        let (_dir, mut app) = workspace();
        let _ = app.update(Message::Mode(Mode::Tables));
        app.tables.clear();
        let mut cache = show_frame(&app);
        app.tables = vec![
            crate::web::shape::ListedTable {
                name: "alpha".to_string(),
                keys: vec![crate::web::shape::KeyRole {
                    name: "id".to_string(),
                    role: "hash".to_string(),
                }],
            },
            crate::web::shape::ListedTable {
                name: "beta".to_string(),
                keys: Vec::new(),
            },
        ];
        cache = show_frame_with(&app, cache);
        app.tables.push(crate::web::shape::ListedTable {
            name: "gamma".to_string(),
            keys: Vec::new(),
        });
        let _ = show_frame_with(&app, cache);
    }

    #[test]
    fn nav_click_keeps_menu_anchor_and_widget_state() {
        let (_dir, mut app) = workspace();
        let mut cache = show_frame(&app);
        app.pointer = iced::Point::new(48.0, 90.0);
        let _ = app.update(Message::Menu("widgets.dql".to_string(), "file".to_string()));
        let anchor = app.menu.as_ref().map(|menu| menu.at);
        app.pointer = iced::Point::new(400.0, 500.0);
        cache = show_frame_with(&app, cache);
        assert_eq!(anchor, Some(iced::Point::new(48.0, 90.0)));
        app.results = vec![message_card(), item_card()];
        cache = show_frame_with(&app, cache);
        app.results = vec![item_card()];
        app.menu = None;
        let _ = show_frame_with(&app, cache);
    }

    fn message_card() -> crate::web::studio::StatementCard {
        crate::web::studio::StatementCard {
            statement: "SELECT 1".to_string(),
            ok: true,
            kind: "message".to_string(),
            message: "ok".to_string(),
            error: String::new(),
            items: Vec::new(),
            columns: Vec::new(),
            elapsed_ms: 1,
            read_units: 0.0,
            write_units: 0.0,
            help: None,
            affected: None,
        }
    }

    fn item_card() -> crate::web::studio::StatementCard {
        let mut card = message_card();
        card.kind = "items".to_string();
        card.message.clear();
        card.items = vec![serde_json::json!({"id": "a"})];
        card
    }

    fn show_frame(app: &App) -> iced_test::runtime::user_interface::Cache {
        show_frame_with(app, iced_test::runtime::user_interface::Cache::new())
    }

    fn show_frame_with(
        app: &App,
        cache: iced_test::runtime::user_interface::Cache,
    ) -> iced_test::runtime::user_interface::Cache {
        use iced::advanced::renderer::Headless;
        use iced::theme::Base;
        use iced::Renderer;
        use iced_test::core::clipboard;
        use iced_test::core::mouse;
        use iced_test::core::renderer;
        use iced_test::core::window;
        use iced_test::core::{Event, Point, Size};
        use iced_test::runtime::user_interface::UserInterface;

        std::env::set_var("ICED_TEST_BACKEND", "tiny-skia");
        let settings = iced::Settings::default();
        let mut renderer = iced_test::futures::futures::executor::block_on(Renderer::new(
            iced::Font::with_name("Fira Sans"),
            settings.default_text_size,
            Some("tiny-skia"),
        ))
        .expect("headless renderer");
        let mut ui =
            UserInterface::build(app.view(), Size::new(1280.0, 800.0), cache, &mut renderer);
        let cursor = mouse::Cursor::Available(Point::new(48.0, 640.0));
        let mut messages = Vec::new();
        let _ = ui.update(
            &[Event::Window(window::Event::RedrawRequested(
                iced_test::core::time::Instant::now(),
            ))],
            cursor,
            &mut renderer,
            &mut clipboard::Null,
            &mut messages,
        );
        let theme = app.theme();
        let base = theme.base();
        ui.draw(
            &mut renderer,
            &theme,
            &renderer::Style {
                text_color: base.text_color,
            },
            cursor,
        );
        ui.into_cache()
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
            let _guard = prefs_env_lock();
            let _ = std::fs::remove_dir_all(&self.path);
            if std::env::var_os("DQLRS_UI_PREFS")
                .is_some_and(|path| std::path::PathBuf::from(path).starts_with(&self.path))
            {
                std::env::remove_var("DQLRS_UI_PREFS");
            }
        }
    }
}
