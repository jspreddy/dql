//! The native window. Every control calls [`crate::host::Host`] on this thread.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    div, px, rgb, ClickEvent, Context, Entity, FocusHandle, Focusable as _, FontWeight,
    InteractiveElement, IntoElement, KeyBinding, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Render, SharedString, StatefulInteractiveElement,
    Styled, TestSupportExt, Window, WindowOptions,
};

use crate::host::{Host, RunResult, SearchHit, TableRow, TreeNode};
use crate::logic::{self, QUERY_COLUMN};
use crate::prefs::{self, Prefs};
use crate::rows::{row_title, ColumnRole};
use crate::tokens::{self, BandKind, HighlightOptions, WordStyle};

gpui_kit::actions!(
    dql_ui,
    [
        Dismiss,
        RunQuery,
        SaveBuffer,
        RunFileChord,
        OpenHit,
        NextHit,
        PrevHit,
        AcceptCompletion
    ]
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Query,
    Tables,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum MenuKind {
    Root,
    Entry { path: String, kind: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Sheet {
    Result,
    Table { name: String },
    Row { title: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResultsTab {
    Result,
    History,
    Lint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Prompt {
    Rename { path: String },
    Move { path: String },
    NewFile { dir: String },
    NewFolder { dir: String },
}

#[derive(Clone, Copy, Debug)]
enum SplitDrag {
    Side { x: f32, width: f32 },
    Results { y: f32, ratio: f32 },
}

#[derive(Clone, Debug)]
struct Grab {
    path: String,
    x: f32,
    y: f32,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct Outcome {
    result: Option<RunResult>,
    history: Vec<RunResult>,
    marks: Vec<bool>,
}

pub struct Studio {
    host: Box<dyn Host>,
    mode: Mode,
    nodes: Vec<TreeNode>,
    open_path: Option<String>,
    text: String,
    head: usize,
    anchor: usize,
    result: Option<RunResult>,
    history: Vec<RunResult>,
    marks: Vec<bool>,
    outcomes: HashMap<String, Outcome>,
    status: String,
    last_ran: String,
    issues: Vec<crate::host::LintIssue>,
    menu: Option<MenuKind>,
    prompt: Option<Prompt>,
    prompt_text: String,
    sheet: Option<Sheet>,
    settings_open: bool,
    search_list_open: bool,
    search_pending: bool,
    search_focused: bool,
    search_epoch: u64,
    search_query: String,
    hit_index: usize,
    hits: Vec<SearchHit>,
    tables: Vec<TableRow>,
    catalog_query: String,
    catalog_epoch: u64,
    selected_table: Option<String>,
    table_page: usize,
    nav_width: f32,
    results_ratio: f32,
    options: HighlightOptions,
    dark: bool,
    explain_raw: bool,
    results_tab: ResultsTab,
    collapsed: HashSet<String>,
    drag: Option<SplitDrag>,
    grab: Option<Grab>,
    saved_text: String,
    conflict: Option<String>,
    save_epoch: u64,
    scroll_line: Option<usize>,
    prefs_path: std::path::PathBuf,
    search: Entity<InputState>,
    editor_focus: FocusHandle,
    side_focus: FocusHandle,
    results_focus: FocusHandle,
    prompt_focus: FocusHandle,
    catalog_focus: FocusHandle,
}

impl Studio {
    pub fn new(host: Box<dyn Host>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search tables, columns, queries"));
        let search_focus = search.read(cx).focus_handle(cx);
        cx.on_focus(&search_focus, window, |studio, _, cx| {
            studio.reopen_search(cx);
        })
        .detach();
        cx.bind_keys([
            KeyBinding::new("escape", Dismiss, None),
            KeyBinding::new("ctrl-enter", RunQuery, None),
            KeyBinding::new("ctrl-\\", RunFileChord, None),
            KeyBinding::new("ctrl-s", SaveBuffer, None),
            KeyBinding::new("enter", OpenHit, None),
            KeyBinding::new("down", NextHit, None),
            KeyBinding::new("up", PrevHit, None),
            KeyBinding::new("tab", AcceptCompletion, None),
        ]);
        let prefs_path = prefs::path_for(host.workspace());
        let loaded = prefs::load(&prefs_path);
        let mut studio = Self {
            host,
            mode: Mode::Query,
            nodes: Vec::new(),
            open_path: None,
            text: String::new(),
            head: 0,
            anchor: 0,
            result: None,
            history: Vec::new(),
            marks: Vec::new(),
            outcomes: HashMap::new(),
            status: String::new(),
            last_ran: String::new(),
            issues: Vec::new(),
            menu: None,
            prompt: None,
            prompt_text: String::new(),
            sheet: None,
            settings_open: false,
            search_list_open: false,
            search_pending: false,
            search_focused: false,
            search_epoch: 0,
            search_query: String::new(),
            hit_index: 0,
            hits: Vec::new(),
            tables: Vec::new(),
            catalog_query: String::new(),
            catalog_epoch: 0,
            selected_table: None,
            table_page: 0,
            nav_width: logic::clamp_nav(loaded.nav),
            results_ratio: logic::clamp_ratio(loaded.results),
            options: loaded.options(),
            dark: loaded.dark,
            explain_raw: loaded.explain_json,
            results_tab: ResultsTab::Result,
            collapsed: HashSet::new(),
            drag: None,
            grab: None,
            saved_text: String::new(),
            conflict: None,
            save_epoch: 0,
            scroll_line: None,
            prefs_path,
            search,
            editor_focus: cx.focus_handle(),
            side_focus: cx.focus_handle(),
            results_focus: cx.focus_handle(),
            prompt_focus: cx.focus_handle(),
            catalog_focus: cx.focus_handle(),
        };
        studio.reload_tree();
        if let Some(path) = logic::preferred_file(&studio.nodes) {
            studio.open_path(path, cx);
        }
        studio
    }

    pub fn framed(&self) -> String {
        tokens::run_target(&self.text, self.head, self.anchor)
            .map(|range| self.text[range.from..range.to].to_string())
            .unwrap_or_default()
    }

    pub fn last_ran(&self) -> &str {
        &self.last_ran
    }

    pub fn nav_width(&self) -> f32 {
        self.nav_width
    }

    pub fn results_ratio(&self) -> f32 {
        self.results_ratio
    }

    pub fn buffer(&self) -> &str {
        &self.text
    }

    pub fn head(&self) -> usize {
        self.head
    }

    pub fn dark(&self) -> bool {
        self.dark
    }

    fn reload_tree(&mut self) {
        self.nodes = self.host.tree().unwrap_or_default();
    }

    fn persist(&self) {
        prefs::save(
            &self.prefs_path,
            &Prefs::capture(
                self.dark,
                self.nav_width,
                self.results_ratio,
                self.options,
                self.explain_raw,
            ),
        );
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.search_list_open || self.search_pending {
            self.search_list_open = false;
            self.search_pending = false;
            self.search_epoch = self.search_epoch.wrapping_add(1);
        } else if self.prompt.is_some() {
            self.prompt = None;
            self.menu = None;
        } else if self.settings_open {
            self.settings_open = false;
        } else if self.sheet.is_some() {
            self.sheet = None;
        } else if self.menu.is_some() {
            self.menu = None;
        }
        cx.notify();
    }

    fn reopen_search(&mut self, cx: &mut Context<Self>) {
        if self.search_query.trim().is_empty() || self.search_list_open || self.search_pending {
            return;
        }
        self.hits = self.host.search(&self.search_query);
        self.hit_index = 0;
        self.search_list_open = true;
        cx.notify();
    }

    fn stash_outcome(&mut self) {
        let Some(path) = self.open_path.clone() else {
            return;
        };
        let outcome = Outcome {
            result: self.result.clone(),
            history: self.history.clone(),
            marks: self.marks.clone(),
        };
        store_outcome(self.host.workspace(), &path, &outcome);
        self.outcomes.insert(path, outcome);
    }

    fn open_path(&mut self, path: String, cx: &mut Context<Self>) {
        if self.open_path.as_deref() != Some(path.as_str()) {
            self.stash_outcome();
        }
        match self.host.read_file(&path) {
            Ok(text) => {
                let outcome = self
                    .outcomes
                    .get(&path)
                    .cloned()
                    .or_else(|| load_outcome(self.host.workspace(), &path))
                    .unwrap_or_default();
                self.result = outcome.result;
                self.history = outcome.history;
                self.marks = outcome.marks;
                self.text = text.clone();
                self.saved_text = text;
                self.conflict = None;
                self.open_path = Some(path);
                self.head = 0;
                self.anchor = 0;
                self.issues = self.host.lint(&self.text);
                self.menu = None;
                self.completion_refresh();
                self.mode = Mode::Query;
                self.results_tab = ResultsTab::Result;
            }
            Err(err) => self.status = err,
        }
        cx.notify();
    }

    fn place_at(&mut self, offset: usize) {
        let offset = offset.min(self.text.len());
        self.anchor = offset;
        self.head = offset;
        self.scroll_line = Some(self.line_of(offset));
    }

    fn line_of(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1
    }

    fn completion_refresh(&mut self) {
        self.scroll_line = Some(self.line_of(self.head));
    }

    fn place_cursor(&mut self, start: usize, end: usize, shift: bool, cx: &mut Context<Self>) {
        if shift {
            self.head = end;
        } else {
            self.anchor = start;
            self.head = start;
        }
        cx.notify();
    }

    fn run_text(&mut self, dql: &str) {
        let result = self.host.execute(dql);
        self.last_ran = dql.to_string();
        self.status = status_of(&result);
        self.scroll_line = if result.ok {
            None
        } else {
            Some(self.line_of(self.head))
        };
        self.history.push(result.clone());
        self.result = Some(result);
        self.results_tab = ResultsTab::Result;
        self.stash_outcome();
    }

    fn run_selection(&mut self, cx: &mut Context<Self>) {
        let Some(range) = tokens::run_target(&self.text, self.head, self.anchor) else {
            self.status = "Nothing to run.".into();
            self.last_ran.clear();
            cx.notify();
            return;
        };
        let dql = self.text[range.from..range.to].to_string();
        self.run_text(&dql);
        cx.notify();
    }

    fn run_file(&mut self, cx: &mut Context<Self>) {
        self.save_now();
        let text = self.text.clone();
        let spans = tokens::statement_spans(&text);
        if spans.is_empty() {
            self.status = "Nothing to run.".into();
            self.result = None;
            cx.notify();
            return;
        }
        self.history.clear();
        self.marks.clear();
        let mut last = None;
        for (from, to) in &spans {
            let statement = text[*from..*to].to_string();
            let result = self.host.execute(&statement);
            self.marks.push(result.ok);
            if !result.ok {
                self.scroll_line = Some(self.line_of(*from));
                self.head = *from;
                self.anchor = *from;
            }
            self.history.push(result.clone());
            last = Some(result);
        }
        if let Some(result) = last {
            self.status = status_of(&result);
            let (from, to) = spans[spans.len() - 1];
            self.last_ran = text[from..to].to_string();
            self.result = Some(result);
        }
        self.results_tab = ResultsTab::Result;
        self.menu = None;
        self.stash_outcome();
        cx.notify();
    }

    fn run_folder(&mut self, path: &str, cx: &mut Context<Self>) {
        let files = dql_files(&self.nodes, path);
        if files.is_empty() {
            self.status = "Nothing to run".into();
            self.menu = None;
            cx.notify();
            return;
        }
        for file in files {
            self.open_path(file, cx);
            self.run_file(cx);
        }
    }

    fn save_now(&mut self) {
        self.save_epoch = self.save_epoch.wrapping_add(1);
        let Some(path) = self.open_path.clone() else {
            return;
        };
        if self.conflict.is_some() {
            return;
        }
        let text = self.text.clone();
        if self.host.write_file(&path, &text).is_ok() {
            self.saved_text = text;
            if self.status.is_empty() {
                self.status = "Saved".into();
            }
        }
    }

    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        self.save_epoch = self.save_epoch.wrapping_add(1);
        let epoch = self.save_epoch;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(logic::AUTOSAVE_MS))
                .await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |studio, cx| {
                    if studio.save_epoch == epoch {
                        studio.save_now();
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    fn insert_text(&mut self, extra: &str, cx: &mut Context<Self>) {
        let head = self.head.min(self.text.len());
        self.text.insert_str(head, extra);
        self.head = head + extra.len();
        self.anchor = self.head;
        self.issues = self.host.lint(&self.text);
        self.schedule_save(cx);
        cx.notify();
    }

    fn backspace(&mut self, cx: &mut Context<Self>) {
        if self.head == 0 || self.text.is_empty() {
            return;
        }
        let head = self.head.min(self.text.len());
        let prev = self.text[..head]
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or(0);
        self.text.replace_range(prev..head, "");
        self.head = prev;
        self.anchor = prev;
        self.issues = self.host.lint(&self.text);
        self.schedule_save(cx);
        cx.notify();
    }

    fn accept_completion(&mut self, cx: &mut Context<Self>) {
        let Some(keyword) = logic::completion_for(&self.text, self.head) else {
            return;
        };
        let head = self.head.min(self.text.len());
        let start = self.text[..head]
            .char_indices()
            .rev()
            .find(|(_, ch)| !ch.is_ascii_alphanumeric())
            .map(|(index, ch)| index + ch.len_utf8())
            .unwrap_or(0);
        self.text.replace_range(start..head, &keyword);
        self.head = start + keyword.len();
        self.anchor = self.head;
        self.schedule_save(cx);
        cx.notify();
    }

    fn edit_prompt(&mut self, key: &str, cx: &mut Context<Self>) {
        match key {
            "backspace" => {
                self.prompt_text.pop();
            }
            "enter" => self.commit_prompt(cx),
            other if other.chars().count() == 1 && !other.chars().any(char::is_control) => {
                self.prompt_text.push_str(other);
            }
            _ => {}
        }
        cx.notify();
    }

    fn commit_prompt(&mut self, cx: &mut Context<Self>) {
        let name = self.prompt_text.trim().to_string();
        if name.is_empty() {
            self.status = "A name is required.".into();
            cx.notify();
            return;
        }
        let prompt = self.prompt.clone();
        let result = match prompt {
            Some(Prompt::Rename { path }) => self.host.rename(&path, &name),
            Some(Prompt::Move { path }) => self.relocate(&path, &name),
            Some(Prompt::NewFile { dir }) => {
                let path = join_rel(&dir, &name);
                self.host.create_file(&path)
            }
            Some(Prompt::NewFolder { dir }) => {
                let path = join_rel(&dir, &name);
                self.host.create_dir(&path)
            }
            None => return,
        };
        match result {
            Ok(path) => {
                self.prompt = None;
                self.menu = None;
                self.reload_tree();
                if path.ends_with(".dql") {
                    self.open_path(path, cx);
                }
                self.status.clear();
            }
            Err(err) => self.status = err,
        }
        cx.notify();
    }

    fn relocate(&mut self, path: &str, dest: &str) -> Result<String, String> {
        let (dir, name) = logic::move_dest(dest);
        if name.is_empty() {
            return Err("A name is required.".into());
        }
        let moved = self.host.move_to(path, dir)?;
        let current = moved.rsplit('/').next().unwrap_or("");
        if current != name {
            self.host.rename(&moved, name)
        } else {
            Ok(moved)
        }
    }

    fn apply_drag(&mut self, x: f32, y: f32) {
        let Some(drag) = self.drag else {
            return;
        };
        match drag {
            SplitDrag::Side { x: origin, width } => {
                self.nav_width = logic::clamp_nav(width + (x - origin));
            }
            SplitDrag::Results { y: origin, ratio } => {
                self.results_ratio = logic::clamp_ratio(ratio + (origin - y) / QUERY_COLUMN);
            }
        }
        self.persist();
    }

    fn schedule_catalog(&mut self, cx: &mut Context<Self>) {
        self.catalog_epoch = self.catalog_epoch.wrapping_add(1);
        let epoch = self.catalog_epoch;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(logic::CATALOG_DEBOUNCE_MS))
                .await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |studio, cx| {
                    if studio.catalog_epoch != epoch {
                        return;
                    }
                    let pattern = studio.catalog_query.clone();
                    studio.tables = studio.host.list_tables(&pattern, false).unwrap_or_default();
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn begin_new_file(&mut self, dir: &str) {
        let in_queries = self.nodes.iter().any(|node| node.name == "queries");
        self.prompt_text = if dir.is_empty() && in_queries {
            "queries/notes.dql".into()
        } else {
            "notes.dql".into()
        };
        self.prompt = Some(Prompt::NewFile {
            dir: if self.prompt_text.contains('/') {
                String::new()
            } else {
                dir.to_string()
            },
        });
        self.menu = None;
    }

    fn schedule_search(&mut self, cx: &mut Context<Self>) {
        self.search_epoch = self.search_epoch.wrapping_add(1);
        let epoch = self.search_epoch;
        self.search_pending = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(logic::SEARCH_DEBOUNCE_MS))
                .await;
            if let Some(this) = this.upgrade() {
                this.update(cx, |studio, cx| {
                    if studio.search_epoch != epoch {
                        return;
                    }
                    let query = studio.search_query.clone();
                    studio.search_pending = false;
                    if query.trim().is_empty() {
                        studio.search_list_open = false;
                        studio.hits.clear();
                    } else {
                        studio.hits = studio.host.search(&query);
                        studio.hit_index = 0;
                        studio.search_list_open = true;
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn open_hit(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(hit) = self.hits.get(index).cloned() else {
            return;
        };
        self.search_list_open = false;
        if matches!(hit.kind.as_str(), "table" | "column" | "index") {
            self.mode = Mode::Tables;
            self.catalog_query.clear();
            self.selected_table = Some(hit.table);
            self.table_page = 0;
            self.tables = self.host.list_tables("", false).unwrap_or_default();
        } else if !hit.path.is_empty() {
            let offset = hit.offset.unwrap_or(0);
            self.open_path(hit.path, cx);
            self.place_at(offset);
        }
        cx.notify();
    }

    fn drop_on(&mut self, path: &str, kind: &str, x: f32, y: f32, cx: &mut Context<Self>) {
        let Some(grab) = self.grab.take() else {
            return;
        };
        let moved = (grab.x - x).abs() > 8.0 || (grab.y - y).abs() > 8.0;
        if moved && kind == "dir" && grab.path != path {
            match self.host.move_to(&grab.path, path) {
                Ok(dest) => {
                    self.reload_tree();
                    if dest.ends_with(".dql") {
                        self.open_path(dest, cx);
                    }
                    self.status.clear();
                }
                Err(err) => self.status = err,
            }
        }
        cx.notify();
    }

    fn note_disk(&mut self) {
        let Some(path) = self.open_path.clone() else {
            return;
        };
        if self.conflict.is_some() {
            return;
        }
        let Ok(disk) = self.host.read_file(&path) else {
            return;
        };
        if disk == self.text {
            self.saved_text = disk;
            return;
        }
        if self.text != self.saved_text && disk != self.saved_text {
            self.conflict = Some(disk);
            return;
        }
        if self.text == self.saved_text {
            self.text = disk.clone();
            self.saved_text = disk;
            self.head = self.head.min(self.text.len());
            self.anchor = self.anchor.min(self.text.len());
            self.issues = self.host.lint(&self.text);
        }
    }

    fn delete_entry(&mut self, path: &str, cx: &mut Context<Self>) {
        match self.host.delete(path) {
            Ok(()) => {
                if self.open_path.as_deref() == Some(path) {
                    self.open_path = None;
                    self.text.clear();
                }
                self.status.clear();
                self.reload_tree();
            }
            Err(err) => self.status = err,
        }
        self.menu = None;
        cx.notify();
    }

    fn covers_editor(&self) -> bool {
        self.sheet.is_some() || self.settings_open || self.search_list_open
    }

    fn file_rows(
        &self,
        nodes: &[TreeNode],
        depth: usize,
        cx: &mut Context<Self>,
    ) -> Vec<gpui_kit::AnyElement> {
        let mut rows = Vec::new();
        for node in nodes {
            let path = node.path.clone();
            let kind = node.kind.clone();
            let name = node.name.clone();
            let open = self.open_path.as_deref() == Some(path.as_str());
            let label = if open {
                format!("{name} open")
            } else {
                name.clone()
            };
            let id = format!("file-{}", path.replace('/', "-"));
            let path_click = path.clone();
            let path_menu = path.clone();
            let kind_click = kind.clone();
            let kind_menu = kind.clone();
            let path_down = path.clone();
            let path_up = path.clone();
            let kind_up = kind.clone();
            rows.push(
                div()
                    .id(SharedString::from(id))
                    .test_support()
                    .aria_label(SharedString::from(label))
                    .h(px(28.))
                    .pl(px(12. + depth as f32 * 14.))
                    .cursor_pointer()
                    .child(name)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.grab = Some(Grab {
                                path: path_down.clone(),
                                x: event.position.x.as_f32(),
                                y: event.position.y.as_f32(),
                            });
                            cx.notify();
                        }),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if kind_click == "dir" {
                            if !this.collapsed.insert(path_click.clone()) {
                                this.collapsed.remove(&path_click);
                            }
                            cx.notify();
                        } else {
                            this.open_path(path_click.clone(), cx);
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseUpEvent, _, cx| {
                            this.drop_on(
                                &path_up,
                                &kind_up,
                                event.position.x.as_f32(),
                                event.position.y.as_f32(),
                                cx,
                            );
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, _, _, cx| {
                            this.menu = Some(MenuKind::Entry {
                                path: path_menu.clone(),
                                kind: kind_menu.clone(),
                            });
                            this.prompt = None;
                            cx.notify();
                        }),
                    )
                    .into_any_element(),
            );
            if kind == "dir" && !self.collapsed.contains(&path) {
                rows.extend(self.file_rows(&node.children, depth + 1, cx));
            }
        }
        rows
    }
}

impl Render for Studio {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.note_disk();
        let search_focused = self.search.read(cx).focus_handle(cx).is_focused(window);
        if search_focused
            && !self.search_focused
            && !self.search_query.trim().is_empty()
            && !self.search_list_open
            && !self.search_pending
        {
            let query = self.search_query.clone();
            self.hits = self.host.search(&query);
            self.hit_index = 0;
            self.search_list_open = true;
        }
        self.search_focused = search_focused;
        let query = self.search.read(cx).value().to_string();
        if query != self.search_query {
            self.search_query = query;
            self.schedule_search(cx);
        }
        let endpoint = self.host.endpoint_label();
        let cache = self.host.cache_status();
        let framed = self.framed();
        let show_gutter =
            self.mode == Mode::Query && self.open_path.is_some() && !self.covers_editor();
        let paper = if self.dark { 0x0b1220 } else { 0xf8fafc };
        let ink = if self.dark { 0xe8eef5 } else { 0x0f172a };
        let header = if self.dark { 0x0c1117 } else { 0x1b2838 };
        let completion = logic::completion_for(&self.text, self.head);
        let this = cx.entity();
        let dismiss = this.clone();
        let run_query = this.clone();
        let save_buffer = this.clone();
        let run_file = this.clone();
        let open_hit = this.clone();
        let next_hit = this.clone();
        let prev_hit = this.clone();
        let accept = this.clone();

        div()
            .id("studio")
            .test_support()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(paper))
            .text_color(rgb(ink))
            .text_size(px(13.))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if event.dragging() && this.drag.is_some() {
                    this.apply_drag(event.position.x.as_f32(), event.position.y.as_f32());
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    if this.drag.take().is_some() {
                        this.persist();
                        cx.notify();
                    }
                }),
            )
            .on_action(move |_: &Dismiss, _, cx| {
                dismiss.update(cx, |studio, cx| studio.dismiss(cx));
            })
            .on_action(move |_: &RunQuery, _, cx| {
                run_query.update(cx, |studio, cx| studio.run_selection(cx));
            })
            .on_action(move |_: &SaveBuffer, _, cx| {
                save_buffer.update(cx, |studio, cx| {
                    studio.save_now();
                    cx.notify();
                });
            })
            .on_action(move |_: &RunFileChord, _, cx| {
                run_file.update(cx, |studio, cx| studio.run_file(cx));
            })
            .on_action(move |_: &OpenHit, window, cx| {
                open_hit.update(cx, |studio, cx| {
                    if studio.search_list_open {
                        let index = studio.hit_index;
                        studio.open_hit(index, cx);
                    } else if studio.prompt.is_some() {
                        studio.commit_prompt(cx);
                    } else if studio.editor_focus.is_focused(window) {
                        studio.insert_text("\n", cx);
                    }
                });
            })
            .on_action(move |_: &AcceptCompletion, window, cx| {
                accept.update(cx, |studio, cx| {
                    if studio.editor_focus.is_focused(window) {
                        studio.accept_completion(cx);
                    }
                });
            })
            .on_action(move |_: &NextHit, window, cx| {
                next_hit.update(cx, |studio, cx| {
                    if studio.search_list_open && !studio.hits.is_empty() {
                        studio.hit_index = (studio.hit_index + 1) % studio.hits.len();
                        cx.notify();
                    } else if studio.results_focus.is_focused(window) {
                        studio.results_ratio =
                            logic::clamp_ratio(studio.results_ratio + 16.0 / QUERY_COLUMN);
                        studio.persist();
                        cx.notify();
                    }
                });
            })
            .on_action(move |_: &PrevHit, window, cx| {
                prev_hit.update(cx, |studio, cx| {
                    if studio.search_list_open && !studio.hits.is_empty() {
                        studio.hit_index = studio
                            .hit_index
                            .checked_sub(1)
                            .unwrap_or(studio.hits.len() - 1);
                        cx.notify();
                    } else if studio.results_focus.is_focused(window) {
                        studio.results_ratio =
                            logic::clamp_ratio(studio.results_ratio - 16.0 / QUERY_COLUMN);
                        studio.persist();
                        cx.notify();
                    }
                });
            })
            .child(self.header(&endpoint, header, cx))
            .child(self.body(cx))
            .when(self.menu.is_some(), |this| this.child(self.menu_panel(cx)))
            .when(self.prompt.is_some(), |this| {
                this.child(self.prompt_panel(cx))
            })
            .when(self.settings_open, |this| {
                this.child(self.settings_panel(&cache, cx))
            })
            .when(self.sheet.is_some(), |this| {
                this.child(self.sheet_panel(cx))
            })
            .when(self.search_list_open, |this| {
                this.child(self.search_list(cx))
            })
            .when(self.conflict.is_some(), |this| {
                this.child(self.conflict_panel(cx))
            })
            .when(show_gutter, |this| this.child(self.gutter(cx)))
            .when(completion.is_some() && show_gutter, |this| {
                let word = completion.unwrap_or_default();
                this.child(
                    div()
                        .id("completion")
                        .test_support()
                        .aria_label(SharedString::from(word.clone()))
                        .absolute()
                        .top(px(120.))
                        .left(px(self.nav_width + 24.))
                        .child(word),
                )
            })
            .when(self.scroll_line.is_some(), |el| {
                let line = self.scroll_line.unwrap_or(1);
                el.child(
                    div()
                        .id("scrolled-line")
                        .test_support()
                        .aria_label(SharedString::from(format!("Scrolled to line {line}")))
                        .h(px(0.)),
                )
            })
            .when(!framed.is_empty(), |el| {
                el.child(
                    div()
                        .id("active-query")
                        .test_support()
                        .aria_label(SharedString::from(framed))
                        .h(px(0.)),
                )
            })
    }
}

impl Studio {
    fn header(&self, endpoint: &str, bar: u32, cx: &mut Context<Self>) -> impl IntoElement {
        let theme_label = if self.dark {
            "Switch to light mode"
        } else {
            "Switch to dark mode"
        };
        div()
            .h(px(36.))
            .flex()
            .items_center()
            .gap_2()
            .px(px(12.))
            .bg(rgb(bar))
            .text_color(rgb(0xe8eef5))
            .border_b_1()
            .border_color(rgb(0xe2e8f0))
            .child(
                div()
                    .id("wordmark")
                    .test_support()
                    .aria_label("dqlrs")
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0x0f766e))
                    .child("dqlrs"),
            )
            .child(
                div()
                    .id("endpoint")
                    .test_support()
                    .aria_label(SharedString::from(endpoint.to_string()))
                    .child(endpoint.to_string()),
            )
            .child(mode_button(
                "mode-query",
                "Query",
                self.mode == Mode::Query,
                cx,
                |studio, cx| {
                    studio.mode = Mode::Query;
                    cx.notify();
                },
            ))
            .child(mode_button(
                "mode-tables",
                "Tables",
                self.mode == Mode::Tables,
                cx,
                |studio, cx| {
                    studio.mode = Mode::Tables;
                    studio.tables = studio.host.list_tables("", false).unwrap_or_default();
                    cx.notify();
                },
            ))
            .child(Input::new(&self.search).id("search-input").w(px(220.)))
            .child(text_button("settings", "Settings", cx, |studio, cx| {
                studio.settings_open = !studio.settings_open;
                studio.sheet = None;
                studio.search_list_open = false;
                cx.notify();
            }))
            .child(text_button("theme", theme_label, cx, |studio, cx| {
                studio.dark = !studio.dark;
                studio.persist();
                cx.notify();
            }))
    }

    fn body(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .child(self.sidebar(cx))
            .child(self.main_column(cx))
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let query_mode = self.mode == Mode::Query;
        div()
            .w(px(self.nav_width))
            .h_full()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(rgb(0xe2e8f0))
            .child(
                div()
                    .id("workspace")
                    .test_support()
                    .aria_label("Workspace")
                    .h(px(32.))
                    .px(px(12.))
                    .child("Workspace")
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, _, cx| {
                            this.menu = Some(MenuKind::Root);
                            cx.notify();
                        }),
                    ),
            )
            .when(query_mode, |el| {
                el.child(text_button("new-file", "New file", cx, |studio, cx| {
                    studio.begin_new_file("");
                    cx.notify();
                }))
            })
            .child(
                div()
                    .id("file-tree")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .children(if query_mode {
                        self.file_rows(&self.nodes, 0, cx)
                    } else {
                        Vec::new()
                    }),
            )
            .child({
                let focus = self.side_focus.clone();
                div()
                    .id("side-split")
                    .test_support()
                    .aria_label("Side split")
                    .track_focus(&focus)
                    .w(px(8.))
                    .h(px(42.))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            this.drag = Some(SplitDrag::Side {
                                x: event.position.x.as_f32(),
                                width: this.nav_width,
                            });
                            cx.notify();
                        }),
                    )
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        let step = match event.keystroke.key.as_str() {
                            "right" => 16.0,
                            "left" => -16.0,
                            _ => 0.0,
                        };
                        if step != 0.0 {
                            this.nav_width = logic::clamp_nav(this.nav_width + step);
                            this.persist();
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                        focus.focus(window, cx);
                        if let ClickEvent::Mouse(mouse) = event {
                            if mouse.up.click_count >= 2 {
                                this.nav_width = logic::NAV_DEFAULT;
                                this.persist();
                                cx.notify();
                            }
                        }
                    }))
            })
    }

    fn main_column(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let title = if self.mode == Mode::Tables || self.open_path.is_none() {
            "No file open".to_string()
        } else {
            self.open_path.clone().unwrap_or_default()
        };
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .id("open-name")
                    .test_support()
                    .aria_label(SharedString::from(title.clone()))
                    .h(px(28.))
                    .px(px(12.))
                    .child(title),
            )
            .when(self.mode == Mode::Query, |el| el.child(self.editor(cx)))
            .when(self.mode == Mode::Tables, |el| {
                el.child(self.table_list(cx))
            })
            .child(self.results(cx))
    }

    fn editor(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let bands = tokens::query_bands(&self.text);
        let parts = tokens::highlight_parts(&self.text);
        let mut lines: Vec<Vec<(String, Option<WordStyle>)>> = vec![Vec::new()];
        for part in parts {
            if part.text == "\n" {
                lines.push(Vec::new());
                continue;
            }
            lines.last_mut().unwrap().push((part.text, part.style));
        }
        if self.text.ends_with('\n') {
            lines.pop();
        }
        let mut offset = 0usize;
        let mut line_ranges = Vec::new();
        for line in self.text.split('\n') {
            let start = offset;
            let end = offset + line.len();
            line_ranges.push((start, end));
            offset = end + 1;
        }
        let focus = self.editor_focus.clone();
        div()
            .id("editor")
            .test_support()
            .aria_label("Query editor")
            .track_focus(&focus)
            .flex_1()
            .flex()
            .flex_col()
            .on_click(cx.listener(move |_, _, window, cx| {
                focus.focus(window, cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.control
                    || event.keystroke.modifiers.platform
                    || event.keystroke.modifiers.alt
                {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "tab" => this.accept_completion(cx),
                    "backspace" => this.backspace(cx),
                    "enter" => this.insert_text("\n", cx),
                    _ => {
                        if let Some(ch) = event.keystroke.key_char.clone() {
                            if ch.chars().count() == 1 {
                                this.insert_text(&ch, cx);
                            }
                        }
                    }
                }
            }))
            .children(lines.into_iter().enumerate().map(|(index, spans)| {
                let (start, end) = line_ranges.get(index).copied().unwrap_or((0, 0));
                let appearance = bands
                    .iter()
                    .find(|band| (index + 1) >= band.line_from && (index + 1) <= band.line_to)
                    .map(|band| tokens::band_appearance(band, self.options));
                let bg = match appearance {
                    Some(BandKind::Write | BandKind::WriteEven) => rgb(0xfef2f2),
                    Some(BandKind::Alt) => rgb(0xf1f5f9),
                    _ => rgb(0xffffff),
                };
                let bar = matches!(
                    appearance,
                    Some(BandKind::Bar | BandKind::BarEven | BandKind::Write | BandKind::WriteEven)
                );
                div()
                    .id(SharedString::from(format!("line-{index}")))
                    .test_support()
                    .aria_label(SharedString::from(format!("Line {}", index + 1)))
                    .h(px(22.))
                    .flex()
                    .bg(bg)
                    .when(bar, |el| {
                        el.child(div().w(px(3.)).h_full().bg(rgb(0xdc2626)))
                    })
                    .when(
                        self.issues.iter().any(|issue| issue.line == index + 1),
                        |el| {
                            el.child(
                                div()
                                    .id(SharedString::from(format!("squiggle-{index}")))
                                    .test_support()
                                    .aria_label(SharedString::from(format!(
                                        "Squiggle line {}",
                                        index + 1
                                    )))
                                    .h(px(2.))
                                    .bg(rgb(0xdc2626)),
                            )
                        },
                    )
                    .children(spans.into_iter().map(|(text, style)| {
                        let (color, bold) = style.map(tokens::paint).unwrap_or((0x0f172a, false));
                        div()
                            .text_color(rgb(color))
                            .when(bold, |el| el.font_weight(FontWeight::BOLD))
                            .child(text)
                    }))
                    .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                        this.editor_focus.focus(window, cx);
                        this.place_cursor(start, end, event.modifiers().shift, cx);
                    }))
            }))
    }

    fn table_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected_table.clone();
        let total = selected.as_ref().and_then(|name| {
            self.tables
                .iter()
                .find(|table| &table.name == name)
                .and_then(|table| table.item_count.parse::<usize>().ok())
        });
        let total = total.unwrap_or(0);
        let pages = logic::page_count(total);
        let page = self.table_page.min(pages.saturating_sub(1));
        let exact = self
            .tables
            .iter()
            .any(|table| table.name.eq_ignore_ascii_case(&self.catalog_query));
        let related = self.tables.iter().any(|table| {
            table.hash_key.eq_ignore_ascii_case(&self.catalog_query)
                || table.range_key.eq_ignore_ascii_case(&self.catalog_query)
        });
        let similar = logic::similar_label(&self.catalog_query, exact, related && !exact);
        let empty = if self.tables.is_empty() && self.catalog_query.is_empty() {
            "No tables".to_string()
        } else if self.tables.is_empty() {
            format!("No tables match {}", self.catalog_query)
        } else {
            String::new()
        };
        div()
            .id("table-page")
            .test_support()
            .flex_1()
            .flex()
            .flex_col()
            .child({
                let focus = self.catalog_focus.clone();
                div()
                    .id("catalog-filter")
                    .test_support()
                    .aria_label("catalog")
                    .track_focus(&focus)
                    .h(px(28.))
                    .px(px(8.))
                    .on_click(cx.listener(move |_, _, window, cx| {
                        focus.focus(window, cx);
                    }))
                    .child(if self.catalog_query.is_empty() {
                        "catalog".to_string()
                    } else {
                        self.catalog_query.clone()
                    })
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if event.keystroke.modifiers.control || event.keystroke.modifiers.platform {
                            return;
                        }
                        match event.keystroke.key.as_str() {
                            "backspace" => {
                                this.catalog_query.pop();
                            }
                            "escape" => {}
                            _ => {
                                if let Some(ch) = event.keystroke.key_char.clone() {
                                    if ch.chars().count() == 1 {
                                        this.catalog_query.push_str(&ch);
                                    }
                                }
                            }
                        }
                        this.schedule_catalog(cx);
                    }))
            })
            .child(text_button(
                "refresh-tables",
                "Refresh",
                cx,
                |studio, cx| {
                    let pattern = studio.catalog_query.clone();
                    studio.tables = studio.host.list_tables(&pattern, true).unwrap_or_default();
                    studio.status = "Refreshed".into();
                    cx.notify();
                },
            ))
            .when(!empty.is_empty(), |el| {
                el.child(
                    div()
                        .id("catalog-empty")
                        .test_support()
                        .aria_label(SharedString::from(empty.clone()))
                        .child(empty),
                )
            })
            .when(similar.is_some(), |el| {
                let label = similar.clone().unwrap_or_default();
                el.child(
                    div()
                        .id("similar-names")
                        .test_support()
                        .aria_label(SharedString::from(label.clone()))
                        .child(label),
                )
            })
            .children(self.tables.iter().map(|table| {
                let name = table.name.clone();
                let keys = format!("{} {}", table.hash_key, table.range_key);
                div()
                    .id(SharedString::from(format!("table-{name}")))
                    .test_support()
                    .aria_label(SharedString::from(name.clone()))
                    .h(px(28.))
                    .px(px(12.))
                    .child(name.clone())
                    .child(format!(" {keys}"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_table = Some(name.clone());
                        this.table_page = 0;
                        cx.notify();
                    }))
            }))
            .when(selected.is_some(), |el| {
                let name = selected.clone().unwrap_or_default();
                let label = logic::pager_label(page, total);
                el.child(
                    div()
                        .id("grid-header")
                        .test_support()
                        .aria_label("Table hash key")
                        .h(px(24.))
                        .child(name.clone()),
                )
                .child(
                    div()
                        .flex()
                        .child(
                            div()
                                .id("peek-column")
                                .test_support()
                                .aria_label("Peek")
                                .w(px(28.))
                                .child("Peek"),
                        )
                        .child(
                            div()
                                .id("grid-scroll")
                                .test_support()
                                .aria_label("Grid")
                                .flex_1()
                                .overflow_x_scroll()
                                .child(
                                    div()
                                        .id("grid-tail")
                                        .test_support()
                                        .w(px(800.))
                                        .aria_label("Notes")
                                        .child("notes"),
                                ),
                        ),
                )
                .child(
                    div()
                        .id("pager")
                        .test_support()
                        .aria_label(SharedString::from(label.clone()))
                        .child(label)
                        .child("50 rows at a time"),
                )
                .when(page > 0, |el| {
                    el.child(text_button("page-prev", "Previous", cx, |studio, cx| {
                        studio.table_page = studio.table_page.saturating_sub(1);
                        cx.notify();
                    }))
                })
                .when(page + 1 < pages, |el| {
                    el.child(text_button("page-next", "Next", cx, |studio, cx| {
                        studio.table_page += 1;
                        cx.notify();
                    }))
                })
                .child(text_button(
                    "table-schema",
                    "Show table structure",
                    cx,
                    move |studio, cx| {
                        studio.sheet = Some(Sheet::Table { name: name.clone() });
                        cx.notify();
                    },
                ))
            })
    }

    fn results(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ratio_label = format!("{:.0}%", self.results_ratio * 100.0);
        let count = self.history.len();
        let count_label = match count {
            0 => "0 results".to_string(),
            1 => "1 result".to_string(),
            n => format!("{n} results"),
        };
        let issue_label = match self.issues.len() {
            0 => "No issues".to_string(),
            1 => "1 issue".to_string(),
            n => format!("{n} issues"),
        };
        let focus = self.results_focus.clone();
        div()
            .h(px(self.results_ratio * QUERY_COLUMN))
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(rgb(0xe2e8f0))
            .child(
                div()
                    .id("results-split")
                    .test_support()
                    .aria_label(SharedString::from(format!("Results split {ratio_label}")))
                    .track_focus(&focus)
                    .h(px(8.))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            this.drag = Some(SplitDrag::Results {
                                y: event.position.y.as_f32(),
                                ratio: this.results_ratio,
                            });
                            cx.notify();
                        }),
                    )
                    .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                        focus.focus(window, cx);
                        if let ClickEvent::Mouse(mouse) = event {
                            if mouse.up.click_count >= 2 {
                                this.results_ratio = logic::RESULTS_DEFAULT;
                                this.persist();
                                cx.notify();
                            }
                        }
                    })),
            )
            .child(
                div()
                    .flex()
                    .child(text_button("tab-result", "Result", cx, |studio, cx| {
                        studio.results_tab = ResultsTab::Result;
                        cx.notify();
                    }))
                    .child(text_button("tab-history", "History", cx, |studio, cx| {
                        studio.results_tab = ResultsTab::History;
                        cx.notify();
                    }))
                    .child(text_button("tab-lint", "Lint", cx, |studio, cx| {
                        studio.results_tab = ResultsTab::Lint;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .id("result-count")
                            .test_support()
                            .aria_label(SharedString::from(
                                if self.results_tab == ResultsTab::Lint {
                                    issue_label.clone()
                                } else {
                                    count_label.clone()
                                },
                            ))
                            .child(if self.results_tab == ResultsTab::Lint {
                                issue_label
                            } else {
                                count_label
                            }),
                    )
                    .child(text_button("run-file", "Run file", cx, |studio, cx| {
                        studio.run_file(cx);
                    })),
            )
            .child(
                div()
                    .id("status")
                    .test_support()
                    .aria_label(SharedString::from(if self.status.is_empty() {
                        "Ready".into()
                    } else {
                        self.status.clone()
                    }))
                    .px(px(12.))
                    .child(if self.status.is_empty() {
                        "Ready".to_string()
                    } else {
                        self.status.clone()
                    }),
            )
            .when(self.results_tab == ResultsTab::Result, |el| {
                el.children(
                    self.result
                        .clone()
                        .map(|result| self.result_card(0, &result)),
                )
                .children(self.marks.iter().enumerate().map(|(index, ok)| {
                    let label = if *ok {
                        format!("Statement {} ok", index + 1)
                    } else {
                        format!("Statement {} failed", index + 1)
                    };
                    div()
                        .id(SharedString::from(format!("mark-{index}")))
                        .test_support()
                        .aria_label(SharedString::from(label.clone()))
                        .child(label)
                }))
            })
            .when(self.results_tab == ResultsTab::History, |el| {
                el.children(
                    self.history
                        .iter()
                        .enumerate()
                        .map(|(index, result)| self.result_card(index, result)),
                )
            })
            .when(self.results_tab == ResultsTab::Lint, |el| {
                let rows: Vec<_> = if self.issues.is_empty() {
                    vec![div()
                        .id("lint-empty")
                        .test_support()
                        .aria_label("No issues")
                        .child("No issues")
                        .into_any_element()]
                } else {
                    self.issues
                        .iter()
                        .enumerate()
                        .map(|(index, issue)| {
                            let line = issue.line;
                            let label = format!(
                                "{} Line {} {}",
                                if issue.severity == "warning" {
                                    "Warning"
                                } else {
                                    "Error"
                                },
                                line,
                                issue.message
                            );
                            text_button(&format!("lint-{index}"), &label, cx, move |studio, cx| {
                                studio.results_tab = ResultsTab::Result;
                                let offset = studio
                                    .text
                                    .lines()
                                    .take(line.saturating_sub(1))
                                    .map(|line| line.len() + 1)
                                    .sum();
                                studio.place_at(offset);
                                studio.mode = Mode::Query;
                                cx.notify();
                            })
                        })
                        .collect()
                };
                el.children(rows)
            })
            .when(
                self.result
                    .as_ref()
                    .is_some_and(|result| !result.items.is_empty()),
                |el| {
                    el.child(text_button(
                        "result-structure",
                        "Result structure",
                        cx,
                        |studio, cx| {
                            studio.sheet = Some(Sheet::Result);
                            cx.notify();
                        },
                    ))
                    .child(text_button(
                        "open-row",
                        "Open row",
                        cx,
                        |studio, cx| {
                            let title = studio
                                .result
                                .as_ref()
                                .and_then(|result| result.items.first())
                                .map(title_of)
                                .unwrap_or_else(|| "Row".into());
                            studio.sheet = Some(Sheet::Row { title });
                            cx.notify();
                        },
                    ))
                },
            )
            .child(text_button("stop", "Stop", cx, |studio, cx| {
                studio.host.cancel();
                studio.status = "Query stopped".into();
                cx.notify();
            }))
    }

    fn result_card(&self, index: usize, result: &RunResult) -> gpui_kit::AnyElement {
        let message = status_of(result);
        let history = self.results_tab == ResultsTab::History;
        let label = if history {
            format!("History {message}")
        } else {
            format!("Result {message}")
        };
        let id = if history {
            format!("history-{index}")
        } else {
            format!("result-{index}")
        };
        let capacity = result
            .help
            .as_ref()
            .and_then(|help| help.get("capacity"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        let elapsed = result
            .help
            .as_ref()
            .and_then(|help| help.get("elapsed"))
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        let show_capacity = capacity
            .split_whitespace()
            .next()
            .and_then(|number| number.parse::<f64>().ok())
            .is_some_and(|number| number > 0.0);
        let mut extra = Vec::new();
        if result.kind == "help" {
            if let Some(help) = &result.help {
                if let Some(entries) = help.get("entries").and_then(|value| value.as_array()) {
                    for entry in entries {
                        let name = entry
                            .get("name")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        let summary = entry
                            .get("summary")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        extra.push(format!("{name} {summary}"));
                    }
                }
                if let Some(code) = help.get("code").and_then(|value| value.as_array()) {
                    for line in code {
                        if let Some(text) = line.as_str() {
                            extra.push(text.to_string());
                        }
                    }
                }
                if let Some(prose) = help.get("prose").and_then(|value| value.as_array()) {
                    for line in prose {
                        if let Some(text) = line.as_str() {
                            extra.push(text.to_string());
                        }
                    }
                }
            }
        }
        if result.kind == "explain" && !self.explain_raw {
            if let Some(help) = result.help.as_ref().and_then(|value| value.as_object()) {
                for (key, value) in help {
                    if key == "capacity" || key == "elapsed" {
                        continue;
                    }
                    extra.push(format!("{key} {}", value.as_str().unwrap_or("")));
                }
            }
        }
        if result.kind == "explain" && self.explain_raw {
            extra.push(
                result
                    .help
                    .as_ref()
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            );
        }
        if result.kind == "describe" {
            extra.push(result.message.clone());
        }
        div()
            .id(SharedString::from(id))
            .test_support()
            .aria_label(SharedString::from(label.clone()))
            .px(px(12.))
            .flex()
            .flex_col()
            .child(label)
            .when(show_capacity, |el| {
                el.child(
                    div()
                        .id(SharedString::from(format!("capacity-{index}")))
                        .test_support()
                        .aria_label(SharedString::from(capacity.clone()))
                        .child(capacity),
                )
            })
            .when(!elapsed.is_empty(), |el| {
                el.child(
                    div()
                        .id(SharedString::from(format!("elapsed-{index}")))
                        .test_support()
                        .aria_label(SharedString::from(elapsed.clone()))
                        .child(elapsed),
                )
            })
            .children(extra.into_iter().enumerate().map(|(extra_index, line)| {
                div()
                    .id(SharedString::from(format!(
                        "card-line-{index}-{extra_index}"
                    )))
                    .test_support()
                    .aria_label(SharedString::from(line.clone()))
                    .child(line)
            }))
            .into_any_element()
    }

    fn gutter(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .top(px(88.))
            .left(px(self.nav_width + 8.))
            .child(text_button(
                "run-selection",
                "Run selection",
                cx,
                |studio, cx| {
                    studio.run_selection(cx);
                },
            ))
    }

    fn menu_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let items: Vec<(&str, &str)> = match &self.menu {
            Some(MenuKind::Root) => vec![
                ("menu-new-file", "New file"),
                ("menu-new-folder", "New folder"),
            ],
            Some(MenuKind::Entry { kind, .. }) if kind == "dir" => vec![
                ("menu-run-folder", "Run folder"),
                ("menu-new-file", "New file"),
                ("menu-new-folder", "New folder"),
                ("menu-duplicate", "Duplicate"),
                ("menu-rename", "Rename"),
                ("menu-move", "Move to…"),
                ("menu-clear", "Clear results"),
                ("menu-delete", "Delete"),
            ],
            Some(MenuKind::Entry { .. }) => vec![
                ("menu-run-file", "Run file"),
                ("menu-new-file", "New file"),
                ("menu-duplicate", "Duplicate"),
                ("menu-rename", "Rename"),
                ("menu-move", "Move to…"),
                ("menu-clear", "Clear results"),
                ("menu-delete", "Delete"),
            ],
            None => Vec::new(),
        };
        div()
            .id("menu")
            .test_support()
            .aria_label("Menu")
            .occlude()
            .absolute()
            .top(px(80.))
            .left(px(12.))
            .w(px(180.))
            .flex()
            .flex_col()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xe2e8f0))
            .children(items.into_iter().map(|(id, label)| {
                let label = label.to_string();
                let action_label = label.clone();
                text_button(id, &label, cx, move |studio, cx| {
                    studio.menu_action(&action_label, cx);
                })
            }))
    }

    fn prompt_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (id, label) = match &self.prompt {
            Some(Prompt::Rename { path }) => {
                let name = path.rsplit('/').next().unwrap_or(path);
                ("rename-prompt", format!("Rename {name}"))
            }
            Some(Prompt::Move { path }) => ("move-prompt", format!("Move {path}")),
            Some(Prompt::NewFile { .. }) => {
                ("new-file-prompt", format!("New file {}", self.prompt_text))
            }
            Some(Prompt::NewFolder { .. }) => (
                "new-folder-prompt",
                format!("New folder {}", self.prompt_text),
            ),
            None => ("prompt", "Prompt".into()),
        };
        let focus = self.prompt_focus.clone();
        div()
            .id(id)
            .test_support()
            .aria_label(SharedString::from(label.clone()))
            .track_focus(&focus)
            .absolute()
            .top(px(120.))
            .left(px(16.))
            .h(px(28.))
            .px(px(8.))
            .bg(rgb(0xffffff))
            .occlude()
            .child(label)
            .child(self.prompt_text.clone())
            .on_click(cx.listener(move |_, _, window, cx| {
                focus.focus(window, cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let key = event
                    .keystroke
                    .key_char
                    .clone()
                    .unwrap_or_else(|| event.keystroke.key.clone());
                this.edit_prompt(&key, cx);
            }))
    }

    fn conflict_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let disk = self.conflict.clone().unwrap_or_default();
        let diff = logic::unified_diff(&disk, &self.text);
        div()
            .id("disk-conflict")
            .test_support()
            .aria_label("File changed on disk")
            .occlude()
            .absolute()
            .bottom(px(36.))
            .right(px(12.))
            .w(px(360.))
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xe2e8f0))
            .flex()
            .flex_col()
            .child("File changed on disk")
            .child(
                div()
                    .id("patch-diff")
                    .test_support()
                    .aria_label(SharedString::from(diff.clone()))
                    .child(diff),
            )
            .child(text_button(
                "recall-patch",
                "Unsaved patch",
                cx,
                |studio, cx| {
                    studio.status = "Unsaved patch".into();
                    cx.notify();
                },
            ))
    }

    fn menu_action(&mut self, label: &str, cx: &mut Context<Self>) {
        let target = self.menu.clone();
        match (label, target) {
            ("New file", Some(MenuKind::Entry { path, kind })) => {
                let dir = if kind == "dir" {
                    path
                } else {
                    path.rsplit_once('/')
                        .map(|(dir, _)| dir.to_string())
                        .unwrap_or_default()
                };
                self.begin_new_file(&dir);
                cx.notify();
            }
            ("New file", _) => {
                self.begin_new_file("");
                cx.notify();
            }
            ("New folder", Some(MenuKind::Entry { path, kind })) => {
                let dir = if kind == "dir" { path } else { String::new() };
                self.prompt_text = "folder".into();
                self.prompt = Some(Prompt::NewFolder { dir });
                self.menu = None;
                cx.notify();
            }
            ("New folder", _) => {
                self.prompt_text = "folder".into();
                self.prompt = Some(Prompt::NewFolder { dir: String::new() });
                self.menu = None;
                cx.notify();
            }
            ("Rename", Some(MenuKind::Entry { path, .. })) => {
                self.prompt_text = path.rsplit('/').next().unwrap_or(&path).to_string();
                self.prompt = Some(Prompt::Rename { path });
                cx.notify();
            }
            ("Move to…", Some(MenuKind::Entry { path, .. })) => {
                self.prompt_text = path.clone();
                self.prompt = Some(Prompt::Move { path });
                self.menu = None;
                cx.notify();
            }
            ("Delete", Some(MenuKind::Entry { path, .. })) => self.delete_entry(&path, cx),
            ("Run file", Some(MenuKind::Entry { path, kind })) if kind == "file" => {
                self.open_path(path, cx);
                self.run_file(cx);
            }
            ("Run folder", Some(MenuKind::Entry { path, .. })) => self.run_folder(&path, cx),
            ("Duplicate", Some(MenuKind::Entry { path, .. })) => match self.host.duplicate(&path) {
                Ok(created) => {
                    self.reload_tree();
                    self.menu = None;
                    if created.ends_with(".dql") {
                        self.open_path(created, cx);
                    }
                }
                Err(err) => {
                    self.status = err;
                    self.menu = None;
                    cx.notify();
                }
            },
            ("Clear results", Some(MenuKind::Entry { path, .. })) => {
                self.outcomes.remove(&path);
                let _ = std::fs::remove_file(outcome_file(self.host.workspace(), &path));
                if self.open_path.as_deref() == Some(path.as_str()) {
                    self.result = None;
                    self.history.clear();
                    self.marks.clear();
                }
                self.status.clear();
                self.menu = None;
                cx.notify();
            }
            ("Clear results", _) => {
                self.result = None;
                self.history.clear();
                self.marks.clear();
                self.status.clear();
                self.menu = None;
                cx.notify();
            }
            _ => {
                self.menu = None;
                cx.notify();
            }
        }
    }

    fn search_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("search-list")
            .test_support()
            .aria_label("Search results")
            .occlude()
            .absolute()
            .top(px(52.))
            .right(px(80.))
            .w(px(280.))
            .flex()
            .flex_col()
            .bg(rgb(0xffffff))
            .when(self.hits.is_empty(), |el| {
                el.child(
                    div()
                        .id("no-matches")
                        .test_support()
                        .aria_label("No matches")
                        .h(px(28.))
                        .px(px(8.))
                        .child("No matches"),
                )
            })
            .children(
                grouped_hits(&self.hits, self.hit_index)
                    .into_iter()
                    .flat_map(|row| match row {
                        HitRow::Header(name) => vec![div()
                            .id(SharedString::from(format!("group-{name}")))
                            .test_support()
                            .aria_label(SharedString::from(name.to_string()))
                            .h(px(22.))
                            .child(name.to_string())
                            .into_any_element()],
                        HitRow::Hit {
                            index,
                            title,
                            selected,
                        } => {
                            let mut rows = Vec::new();
                            if selected {
                                rows.push(
                                    div()
                                        .id(SharedString::from(format!("hit-selected-{index}")))
                                        .test_support()
                                        .aria_label("Search hit selected")
                                        .h(px(0.))
                                        .into_any_element(),
                                );
                            }
                            rows.push(text_button(
                                &format!("hit-{index}"),
                                &title,
                                cx,
                                move |studio, cx| {
                                    studio.open_hit(index, cx);
                                },
                            ));
                            rows
                        }
                    }),
            )
    }

    fn settings_panel(&self, cache: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let age = if cache.starts_with('(') {
            cache.to_string()
        } else {
            format!("({cache})")
        };
        div()
            .id("settings-card")
            .test_support()
            .aria_label("Settings")
            .occlude()
            .absolute()
            .top(px(56.))
            .right(px(12.))
            .w(px(232.))
            .flex()
            .flex_col()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xe2e8f0))
            .p(px(12.))
            .child(
                div()
                    .id("cache-age")
                    .test_support()
                    .aria_label(SharedString::from(age.clone()))
                    .child(age),
            )
            .child(text_button(
                "clear-cache",
                "Clear table cache",
                cx,
                |studio, cx| {
                    let _ = studio.host.clear_cache();
                    studio.status = "Table cache cleared.".into();
                    cx.notify();
                },
            ))
            .child(text_button(
                "minimal-write",
                if self.options.minimal_write {
                    "Minimal write highlight on"
                } else {
                    "Minimal write highlight"
                },
                cx,
                |studio, cx| {
                    studio.options.minimal_write = !studio.options.minimal_write;
                    studio.persist();
                    cx.notify();
                },
            ))
            .child(text_button(
                "even-odd",
                if self.options.even_odd {
                    "Even/odd highlight on"
                } else {
                    "Even/odd highlight"
                },
                cx,
                |studio, cx| {
                    studio.options.even_odd = !studio.options.even_odd;
                    studio.persist();
                    cx.notify();
                },
            ))
            .child(
                div()
                    .id("shortcut-run")
                    .test_support()
                    .aria_label("Run selection Ctrl + Enter")
                    .child("Run selection Ctrl + Enter"),
            )
            .child(
                div()
                    .id("shortcut-run-file")
                    .test_support()
                    .aria_label("Run selection Ctrl + \\")
                    .child("Run selection Ctrl + \\"),
            )
            .child(
                div()
                    .id("shortcut-stop")
                    .test_support()
                    .aria_label("Stop query Esc")
                    .child("Stop query Esc"),
            )
            .child(text_button(
                "explain-json",
                if self.explain_raw {
                    "Raw explain on"
                } else {
                    "Raw explain"
                },
                cx,
                |studio, cx| {
                    studio.explain_raw = !studio.explain_raw;
                    studio.persist();
                    cx.notify();
                },
            ))
    }

    fn sheet_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let sheet = self.sheet.clone();
        div()
            .id("sheet")
            .test_support()
            .occlude()
            .absolute()
            .size_full()
            .bg(rgb(0xffffff))
            .flex()
            .flex_col()
            .p(px(24.))
            .child(match &sheet {
                Some(Sheet::Result) => "Result structure".to_string(),
                Some(Sheet::Table { name }) => format!("Table structure {name}"),
                Some(Sheet::Row { title }) => title.clone(),
                None => String::new(),
            })
            .when(matches!(sheet, Some(Sheet::Result)), |el| {
                el.child(
                    div()
                        .id("sheet-info")
                        .test_support()
                        .aria_label("Based on this query result, not the entire table.")
                        .child("Based on this query result, not the entire table."),
                )
                .child(
                    div()
                        .id("sheet-footer")
                        .test_support()
                        .aria_label("Types come from this result. No extra query was run.")
                        .child("Types come from this result. No extra query was run."),
                )
            })
            .when(matches!(sheet, Some(Sheet::Row { .. })), |el| {
                let title = match &sheet {
                    Some(Sheet::Row { title }) => title.clone(),
                    _ => "Row".into(),
                };
                let keys = self
                    .result
                    .as_ref()
                    .and_then(|result| result.items.first())
                    .and_then(|item| item.as_object())
                    .map(|map| map.keys().cloned().collect::<Vec<_>>().join(" "))
                    .unwrap_or_default();
                el.child(
                    div()
                        .id("row-title")
                        .test_support()
                        .aria_label(SharedString::from(title))
                        .child("Row"),
                )
                .child(
                    div()
                        .id("row-json")
                        .test_support()
                        .aria_label(SharedString::from(keys.clone()))
                        .child(keys),
                )
            })
            .child(text_button("close-sheet", "Close", cx, |studio, cx| {
                studio.sheet = None;
                cx.notify();
            }))
    }
}

fn outcome_file(workspace: &std::path::Path, file: &str) -> std::path::PathBuf {
    workspace
        .join(".dqlrs-ui-results")
        .join(format!("{}.json", file.replace('/', "__")))
}

fn store_outcome(workspace: &std::path::Path, file: &str, outcome: &Outcome) {
    let path = outcome_file(workspace, file);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
        }
    }
    let Ok(text) = serde_json::to_string(outcome) else {
        return;
    };
    if std::fs::write(&path, text).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
    }
}

fn load_outcome(workspace: &std::path::Path, file: &str) -> Option<Outcome> {
    let text = std::fs::read_to_string(outcome_file(workspace, file)).ok()?;
    serde_json::from_str(&text).ok()
}

fn status_of(result: &RunResult) -> String {
    if result.message.is_empty() {
        if result.ok {
            result.kind.clone()
        } else {
            "error".into()
        }
    } else {
        result.message.clone()
    }
}

fn join_rel(dir: &str, name: &str) -> String {
    if dir.is_empty() || name.contains('/') {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

fn dql_files(nodes: &[TreeNode], dir: &str) -> Vec<String> {
    let mut files = Vec::new();
    fn walk(nodes: &[TreeNode], dir: &str, files: &mut Vec<String>) {
        for node in nodes {
            let inside =
                dir.is_empty() || node.path == dir || node.path.starts_with(&format!("{dir}/"));
            if !inside {
                continue;
            }
            if node.kind == "file" {
                files.push(node.path.clone());
            }
            walk(&node.children, dir, files);
        }
    }
    walk(nodes, dir, &mut files);
    files
}

enum HitRow {
    Header(&'static str),
    Hit {
        index: usize,
        title: String,
        selected: bool,
    },
}

fn grouped_hits(hits: &[SearchHit], selected: usize) -> Vec<HitRow> {
    let mut order: Vec<usize> = (0..hits.len()).collect();
    order.sort_by_key(|index| match hits[*index].kind.as_str() {
        "table" => 0,
        "column" | "index" => 1,
        _ => 2,
    });
    let mut rows = Vec::new();
    let mut current = "";
    for index in order {
        let group = logic::group_of(&hits[index].kind);
        if group != current {
            rows.push(HitRow::Header(group));
            current = group;
        }
        rows.push(HitRow::Hit {
            index,
            title: hits[index].title.clone(),
            selected: index == selected,
        });
    }
    rows
}

fn title_of(item: &serde_json::Value) -> String {
    let Some(map) = item.as_object() else {
        return "Row".into();
    };
    let columns: Vec<ColumnRole> = map
        .keys()
        .map(|name| ColumnRole {
            name: name.clone(),
            role: if name == "id" {
                "hash".into()
            } else {
                String::new()
            },
        })
        .collect();
    row_title(map, &columns)
}

fn mode_button(
    id: &'static str,
    label: &'static str,
    selected: bool,
    cx: &mut Context<Studio>,
    action: impl Fn(&mut Studio, &mut Context<Studio>) + 'static,
) -> gpui_kit::AnyElement {
    let action = Arc::new(action);
    div()
        .id(id)
        .test_support()
        .aria_label(label)
        .h(px(28.))
        .px(px(10.))
        .cursor_pointer()
        .when(selected, |el| {
            el.text_color(rgb(0x0f766e)).font_weight(FontWeight::BOLD)
        })
        .child(label)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| action(this, cx)))
        .into_any_element()
}

fn text_button(
    id: &str,
    label: &str,
    cx: &mut Context<Studio>,
    action: impl Fn(&mut Studio, &mut Context<Studio>) + 'static,
) -> gpui_kit::AnyElement {
    let action = Arc::new(action);
    let label: SharedString = label.to_string().into();
    div()
        .id(SharedString::from(id.to_string()))
        .test_support()
        .aria_label(label.clone())
        .h(px(28.))
        .px(px(8.))
        .cursor_pointer()
        .child(label)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| action(this, cx)))
        .into_any_element()
}

pub fn run_native(host: Box<dyn Host>) {
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(WindowOptions::default(), cx, move |window, cx| {
            cx.new(|cx| Studio::new(host, window, cx))
        })
        .expect("open dqlrs window");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::{MemoryHost, RunResult};
    use gpui_kit::test::{TestQueryExt, TestWindowExt};
    use gpui_kit::{px, size, AppContext, Bounds, Point, TestAppContext, WindowBounds};
    use serde_json::json;

    fn open(
        cx: &mut TestAppContext,
        host: MemoryHost,
    ) -> (gpui_kit::AnyWindowHandle, Entity<Studio>) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            let bounds = Bounds {
                origin: Point::default(),
                size: size(px(1200.), px(800.)),
            };
            let (window, content) = gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| Studio::new(Box::new(host), window, cx)),
            )
            .expect("open test window");
            (window, content)
        })
    }

    fn fixture() -> (tempfile::TempDir, MemoryHost) {
        let dir = tempfile::tempdir().unwrap();
        let queries = dir.path().join("queries");
        std::fs::create_dir_all(&queries).unwrap();
        std::fs::write(
            queries.join("edit.dql"),
            "-- header stays with the select\nSELECT * FROM widgets WHERE id = 'a';\nSCAN * FROM widgets;\n",
        )
        .unwrap();
        std::fs::write(queries.join("read.dql"), "SELECT 1;\n").unwrap();
        std::fs::create_dir_all(dir.path().join("empty")).unwrap();
        std::fs::write(dir.path().join("empty").join(".gitkeep"), "").unwrap();
        let mut host = MemoryHost::new(dir.path());
        host.endpoint = "memory".into();
        host.replies.push((
            "missing".into(),
            RunResult::error("Table missing not found"),
        ));
        host.replies.push((
            "SELECT * FROM widgets".into(),
            RunResult {
                ok: true,
                kind: "items".into(),
                message: "1 item".into(),
                items: vec![json!({"id": "a", "name": "Ada"})],
                affected: Some(1),
                help: None,
            },
        ));
        (dir, host)
    }

    #[gpui_kit::test]
    fn the_window_shows_the_wordmark_and_tables_hides_new_file(cx: &mut TestAppContext) {
        let (_dir, host) = fixture();
        let (window, _) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find_by_label("dqlrs").label(), Some("dqlrs"));
            assert!(window.try_find("new-file").is_some());
            window.click("mode-tables", cx);
            assert!(window.try_find("new-file").is_none());
            assert_eq!(
                window.find_by_label("No file open").label(),
                Some("No file open")
            );
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn opening_a_file_marks_that_row(cx: &mut TestAppContext) {
        let (_dir, host) = fixture();
        let (window, _) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("file-queries-edit.dql", cx);
            assert!(window.try_find_by_label("edit.dql open").is_some());
            window.click("file-queries-read.dql", cx);
            assert!(window.try_find_by_label("edit.dql open").is_none());
            assert!(window.try_find_by_label("read.dql open").is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn menus_match_the_page(cx: &mut TestAppContext) {
        let (_dir, host) = fixture();
        let (window, _) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.right_click("workspace", cx);
            assert_eq!(window.find("menu-new-file").label(), Some("New file"));
            assert_eq!(window.find("menu-new-folder").label(), Some("New folder"));
            assert!(window.try_find("menu-run-file").is_none());
            window.right_click("file-queries-edit.dql", cx);
            assert_eq!(window.find("menu-run-file").label(), Some("Run file"));
            assert_eq!(window.find("menu-move").label(), Some("Move to…"));
            assert_eq!(window.find("menu-clear").label(), Some("Clear results"));
            assert!(window.try_find("menu-run-folder").is_none());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn escape_cancels_rename_and_gitkeep_blocks_delete(cx: &mut TestAppContext) {
        let (_dir, host) = fixture();
        let (window, _) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.right_click("file-queries-edit.dql", cx);
            window.click("menu-rename", cx);
            assert!(window.try_find_by_label("Rename edit.dql").is_some());
            window.click("editor", cx);
            window.press("escape", cx);
            assert!(window.try_find_by_label("Rename edit.dql").is_none());
            window.right_click("file-empty", cx);
            window.click("menu-delete", cx);
            assert!(window
                .find_by_label("folder contains .gitkeep, which is not a .dql file")
                .label()
                .is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn the_cursor_frames_the_whole_statement_and_a_selection_wins(cx: &mut TestAppContext) {
        let (_dir, host) = fixture();
        let (window, studio) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("file-queries-edit.dql", cx);
            window.click("line-1", cx);
            let framed = studio.read(cx).framed();
            assert!(framed.contains("header stays"));
            assert!(!framed.contains("SCAN"));
            window.click_with_modifiers(
                "line-1",
                gpui_kit::Modifiers {
                    shift: true,
                    ..Default::default()
                },
                cx,
            );
            window.click("run-selection", cx);
        })
        .unwrap();
        let ran = studio.read_with(cx, |studio, _| studio.last_ran().to_string());
        assert!(ran.contains("WHERE id"));
        assert!(!ran.contains("header stays"));
    }

    #[gpui_kit::test]
    fn a_missing_table_reports_the_error_and_stop_is_labeled(cx: &mut TestAppContext) {
        let (dir, host) = fixture();
        std::fs::write(
            dir.path().join("queries").join("edit.dql"),
            "SELECT * FROM missing;\n",
        )
        .unwrap();
        let (window, _) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("file-queries-edit.dql", cx);
            window.click("editor", cx);
            window.press("ctrl-enter", cx);
            assert!(window
                .try_find_by_label("Table missing not found")
                .is_some());
            assert!(window.try_find("run-selection").is_some());
            window.click("stop", cx);
            assert!(window.try_find_by_label("Query stopped").is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn the_sheet_hides_the_gutter_button(cx: &mut TestAppContext) {
        let (_dir, host) = fixture();
        let (window, _) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("file-queries-edit.dql", cx);
            window.click("line-1", cx);
            window.click("run-selection", cx);
            assert!(window.try_find("run-selection").is_some());
            window.click("result-structure", cx);
            assert!(window.try_find("run-selection").is_none());
            assert!(window
                .try_find_by_label("Based on this query result, not the entire table.")
                .is_some());
            assert!(window
                .try_find_by_label("Types come from this result. No extra query was run.")
                .is_some());
            window.press("escape", cx);
            assert!(window.try_find("run-selection").is_some());
            window.click("open-row", cx);
            assert_eq!(window.find_by_label("id = a").label(), Some("id = a"));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn clear_table_cache_and_search_empty_state(cx: &mut TestAppContext) {
        let (_dir, mut host) = fixture();
        host.cache_label = "2 days ago".into();
        let (window, studio) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("search-input", cx);
            window.input("zzzz-no-such", cx);
            window.render_frame(cx);
            let value = studio.read(cx).search.read(cx).value().to_string();
            assert_eq!(value, "zzzz-no-such");
            assert!(window.try_find_by_label("No matches").is_none());
        })
        .unwrap();
        cx.executor().advance_clock(Duration::from_millis(150));
        cx.executor().run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find_by_label("No matches").is_some());
            window.press("escape", cx);
            let value = studio.read(cx).search.read(cx).value().to_string();
            assert_eq!(value, "zzzz-no-such");
            assert!(window.try_find_by_label("No matches").is_none());
            window.click("settings", cx);
            assert!(window.try_find_by_label("(2 days ago)").is_some());
            window.click("clear-cache", cx);
            assert!(window.try_find_by_label("Table cache cleared.").is_some());
        })
        .unwrap();
    }

    fn center(bounds: Bounds<gpui_kit::Pixels>) -> Point<gpui_kit::Pixels> {
        Point {
            x: bounds.origin.x + bounds.size.width / 2.,
            y: bounds.origin.y + bounds.size.height / 2.,
        }
    }

    #[gpui_kit::test]
    fn splits_drag_nudge_and_remember_their_size(cx: &mut TestAppContext) {
        let (dir, host) = fixture();
        let (window, studio) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let from = center(window.find("side-split").bounds());
            window.drag(
                from,
                Point {
                    x: from.x + px(80.),
                    y: from.y,
                },
                cx,
            );
            window.render_frame(cx);
            let width = studio.read(cx).nav_width();
            assert!((width - 300.0).abs() < 8.0, "{width}");
            window.double_click("side-split", cx);
            assert!((studio.read(cx).nav_width() - 220.0).abs() < 1.0);
            window.click("side-split", cx);
            window.press("right", cx);
            let nudged = studio.read(cx).nav_width();
            assert!(nudged - 220.0 > 10.0, "{nudged}");
            let from = center(window.find("results-split").bounds());
            window.drag(
                from,
                Point {
                    x: from.x,
                    y: from.y - px(40.),
                },
                cx,
            );
            window.render_frame(cx);
            assert!(studio.read(cx).results_ratio() > 0.46);
            window.double_click("results-split", cx);
            assert!((studio.read(cx).results_ratio() - 0.46).abs() < 0.02);
            window.click("results-split", cx);
            window.press("down", cx);
            let grown = studio.read(cx).results_ratio();
            assert!((grown - 0.46) * 520.0 > 10.0, "{grown}");
        })
        .unwrap();
        let host = MemoryHost::new(dir.path());
        let (window, studio) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let width = studio.read(cx).nav_width();
            assert!((width - 236.0).abs() < 3.0, "{width}");
            assert!(studio.read(cx).results_ratio() > 0.46);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn files_collapse_duplicate_rename_move_and_run_folder(cx: &mut TestAppContext) {
        let (dir, host) = fixture();
        let (window, _) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("file-queries", cx);
            assert!(window.try_find("file-queries-edit.dql").is_none());
            window.click("file-queries", cx);
            assert!(window.try_find("file-queries-edit.dql").is_some());
            window.right_click("file-queries", cx);
            assert_eq!(window.find("menu-run-folder").label(), Some("Run folder"));
            assert_eq!(window.find("menu-new-folder").label(), Some("New folder"));
            assert!(window.try_find("menu-run-file").is_none());
            window.click("editor", cx);
            window.press("escape", cx);
            window.right_click("file-queries-edit.dql", cx);
            window.click("menu-duplicate", cx);
            assert!(window.try_find_by_label("edit copy.dql open").is_some());
            window.right_click("file-empty", cx);
            window.click("menu-rename", cx);
            window.click("rename-prompt", cx);
            for _ in 0..5 {
                window.press("backspace", cx);
            }
            for key in ["v", "a", "c", "a", "n", "t"] {
                window.press(key, cx);
            }
            window.press("enter", cx);
            assert!(window.try_find("file-vacant").is_some());
            window.drag_to("file-queries-read.dql", "file-vacant", cx);
            window.render_frame(cx);
            assert!(window.try_find("file-vacant-read.dql").is_some());
            window.click("new-file", cx);
            assert!(window
                .try_find_by_label("New file queries/notes.dql")
                .is_some());
            window.click("new-file-prompt", cx);
            window.press("enter", cx);
            assert!(window.try_find_by_label("notes.dql open").is_some());
            window.right_click("file-vacant", cx);
            window.click("menu-run-folder", cx);
        })
        .unwrap();
        assert!(dir.path().join("queries").join("edit copy.dql").exists());
        assert!(dir.path().join("vacant").join("read.dql").exists());
        assert!(dir.path().join("queries").join("notes.dql").exists());
    }

    #[gpui_kit::test]
    fn an_empty_folder_has_nothing_to_run(cx: &mut TestAppContext) {
        let (_dir, host) = fixture();
        let (window, _) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.right_click("file-empty", cx);
            window.click("menu-run-folder", cx);
            assert!(window.try_find_by_label("Nothing to run").is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn run_file_keeps_earlier_statements_on_history(cx: &mut TestAppContext) {
        let (_dir, host) = fixture();
        let (window, studio) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("editor", cx);
            window.press("ctrl-\\", cx);
            window.render_frame(cx);
            let labels: Vec<String> = window
                .elements()
                .iter()
                .filter_map(|element| element.label().map(str::to_string))
                .collect();
            assert!(
                labels
                    .iter()
                    .any(|label| label.starts_with("Result ") && label.contains("SCAN")),
                "{labels:?}"
            );
            assert!(labels.iter().any(|label| label == "Statement 1 ok"));
            assert!(labels.iter().any(|label| label == "Statement 2 ok"));
            window.click("tab-history", cx);
            assert!(window.try_find_by_label("History 1 item").is_some());
            window.click("file-queries-read.dql", cx);
            assert!(window.try_find_by_label("History 1 item").is_none());
            window.click("file-queries-edit.dql", cx);
            window.click("tab-history", cx);
            assert!(window.try_find_by_label("History 1 item").is_some());
            assert!(studio.read(cx).last_ran().contains("SCAN"));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn edits_autosave_and_a_dirty_buffer_keeps_a_patch(cx: &mut TestAppContext) {
        let (dir, host) = fixture();
        let path = dir.path().join("queries").join("edit.dql");
        let (window, studio) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("editor", cx);
            window.press("x", cx);
            let on_disk = std::fs::read_to_string(&path).unwrap();
            assert!(!on_disk.starts_with('x'), "{on_disk}");
        })
        .unwrap();
        cx.executor().advance_clock(Duration::from_millis(400));
        cx.executor().run_until_parked();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.starts_with('x'), "{saved}");
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("editor", cx);
            window.press("enter", cx);
            for key in "-- local unsaved".chars() {
                let key = if key == ' ' {
                    "space".to_string()
                } else {
                    key.to_string()
                };
                window.press(&key, cx);
            }
            std::fs::write(&path, "disk\n").unwrap();
            window.render_frame(cx);
            assert!(window.try_find_by_label("File changed on disk").is_some());
            let diff = window.find("patch-diff").label().unwrap_or("").to_string();
            assert!(diff.contains("+-- local unsaved"), "{diff}");
            window.click("recall-patch", cx);
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "disk\n");
            assert!(studio.read(cx).buffer().contains("-- local unsaved"));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn theme_and_highlight_toggles_persist(cx: &mut TestAppContext) {
        let (dir, host) = fixture();
        let (window, studio) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find_by_label("Switch to dark mode").is_some());
            window.click("theme", cx);
            assert!(studio.read(cx).dark());
            assert!(window.try_find_by_label("Switch to light mode").is_some());
            window.click("settings", cx);
            assert!(window.try_find_by_label("Even/odd highlight on").is_some());
            assert!(window
                .try_find_by_label("Run selection Ctrl + Enter")
                .is_some());
            assert!(window.try_find_by_label("Stop query Esc").is_some());
            window.click("minimal-write", cx);
            window.click("even-odd", cx);
            assert!(window
                .try_find_by_label("Minimal write highlight on")
                .is_some());
            assert!(window.try_find_by_label("Even/odd highlight").is_some());
        })
        .unwrap();
        let host = MemoryHost::new(dir.path());
        let (window, studio) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(studio.read(cx).dark());
            window.click("settings", cx);
            assert!(window
                .try_find_by_label("Minimal write highlight on")
                .is_some());
            assert!(window.try_find_by_label("Even/odd highlight").is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn search_groups_a_query_hit_and_a_column_hit(cx: &mut TestAppContext) {
        let (_dir, mut host) = fixture();
        host.tables.push(crate::host::TableRow {
            name: "widgets".into(),
            status: String::new(),
            hash_key: "id".into(),
            range_key: String::new(),
            item_count: "51".into(),
            read_units: String::new(),
            write_units: String::new(),
            indexes: String::new(),
        });
        let (window, studio) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("search-input", cx);
            window.input("id", cx);
            window.render_frame(cx);
        })
        .unwrap();
        cx.executor().advance_clock(Duration::from_millis(150));
        cx.executor().run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find_by_label("COLUMNS").is_some());
            assert!(window.try_find_by_label("QUERIES").is_some());
            assert!(window.try_find_by_label("Search hit selected").is_some());
            window.click("hit-0", cx);
            assert!(studio.read(cx).head() > 0, "{}", studio.read(cx).head());
            assert!(studio.read(cx).buffer().contains("WHERE id"));
            window.click("search-input", cx);
            window.press("escape", cx);
            assert_eq!(studio.read(cx).search.read(cx).value().to_string(), "id");
            window.click("editor", cx);
            window.click("search-input", cx);
            window.render_frame(cx);
            assert!(window.try_find_by_label("COLUMNS").is_some());
            window.click("hit-1", cx);
            assert!(window.try_find_by_label("No file open").is_some());
            assert!(window.try_find_by_label("Peek").is_some());
            assert_eq!(window.find("pager").label(), Some("Showing 1–50"));
            let peek = window.find("peek-column").bounds().origin.x;
            window.scroll(
                "grid-scroll",
                gpui_kit::ScrollDelta::Pixels(Point {
                    x: px(-120.),
                    y: px(0.),
                }),
                cx,
            );
            window.render_frame(cx);
            let peeked = window.find("peek-column").bounds().origin.x;
            assert!((peek.as_f32() - peeked.as_f32()).abs() < 1.0);
            window.click("page-next", cx);
            assert_eq!(window.find("pager").label(), Some("Showing 51–51"));
            assert!(window.try_find("page-next").is_none());
            assert!(window.try_find("page-prev").is_some());
            window.click("catalog-filter", cx);
            window.press("backspace", cx);
            window.press("backspace", cx);
            window.press("w", cx);
            window.press("i", cx);
            window.press("d", cx);
        })
        .unwrap();
        cx.executor().advance_clock(Duration::from_millis(250));
        cx.executor().run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let label = window
                .find("similar-names")
                .label()
                .unwrap_or("")
                .to_string();
            assert!(label.contains("similar names"), "{label}");
            window.click("refresh-tables", cx);
            assert!(window.try_find_by_label("Refreshed").is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn help_explain_describe_lint_and_completion(cx: &mut TestAppContext) {
        let (dir, mut host) = fixture();
        host.replies.push((
            "ZERO".to_string(),
            RunResult {
                ok: true,
                kind: "items".into(),
                message: "0 rows".into(),
                items: vec![json!({"id": "a", "sku": "s", "name": "Ada"})],
                affected: Some(0),
                help: Some(json!({"capacity": "0 RCU", "elapsed": "4 ms"})),
            },
        ));
        let (window, studio) = open(cx, host);
        let path = dir.path().join("queries").join("edit.dql");
        std::fs::write(&path, "HELP;\n").unwrap();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("editor", cx);
            window.press("ctrl-enter", cx);
            assert!(window
                .try_find_by_label("Result Available commands")
                .is_some());
            assert!(window
                .try_find_by_label("select Query items from a table or index")
                .is_some());
            std::fs::write(&path, "HELP SELECT;\n").unwrap();
            window.render_frame(cx);
            window.click("editor", cx);
            window.press("ctrl-enter", cx);
            assert!(window.try_find_by_label("Result HELP select").is_some());
            assert!(window
                .try_find_by_label("WHERE can compare a.b.c and a[0].b on SELECT and SCAN.")
                .is_some());
            std::fs::write(&path, "EXPLAIN SCAN * FROM other;\n").unwrap();
            window.render_frame(cx);
            window.click("editor", cx);
            window.press("ctrl-enter", cx);
            assert!(window
                .try_find_by_label("projection expression #f")
                .is_some());
            assert!(window.try_find_by_label("12 RCU").is_none());
            window.click("settings", cx);
            window.click("explain-json", cx);
            assert!(window
                .try_find_by_label("projection expression #f")
                .is_none());
            window.press("escape", cx);
            std::fs::write(&path, "DESCRIBE widgets;\n").unwrap();
            window.render_frame(cx);
            window.click("editor", cx);
            window.press("ctrl-enter", cx);
            assert!(window.try_find_by_label("12 RCU").is_some());
            assert!(window.try_find_by_label("4 ms").is_some());
            std::fs::write(&path, "DESCRIBE wid;\n").unwrap();
            window.render_frame(cx);
            window.click("editor", cx);
            window.press("ctrl-enter", cx);
            let labels: Vec<String> = window
                .elements()
                .iter()
                .filter_map(|element| element.label().map(str::to_string))
                .collect();
            assert!(
                labels
                    .iter()
                    .any(|label| label.contains("showing similar names")),
                "{labels:?}"
            );
            std::fs::write(&path, "SELECT ZERO;\n").unwrap();
            window.render_frame(cx);
            window.click("editor", cx);
            window.press("ctrl-enter", cx);
            assert!(window.try_find_by_label("Result 0 rows").is_some());
            assert!(window.try_find_by_label("0 RCU").is_none());
            assert!(window.try_find_by_label("4 ms").is_some());
            window.click("open-row", cx);
            assert_eq!(window.find("row-json").label(), Some("id sku name"));
            window.press("escape", cx);
            std::fs::write(&path, "NOTAWORD\n").unwrap();
            window.render_frame(cx);
            window.click("tab-lint", cx);
            assert!(window.try_find("squiggle-0").is_some());
            let lint = window.find("lint-0").label().unwrap_or("").to_string();
            assert!(lint.contains("Error Line 1"), "{lint}");
            window.click("lint-0", cx);
            assert_eq!(studio.read(cx).head(), 0);
            window.click("editor", cx);
            window.press("ctrl-s", cx);
            let saved = std::fs::read_to_string(&path).unwrap();
            assert!(saved.contains("NOTAWORD"), "{saved}");
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn a_new_window_restores_the_file_history(cx: &mut TestAppContext) {
        let (dir, host) = fixture();
        let (window, _) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("run-file", cx);
            assert!(window.try_find_by_label("Statement 1 ok").is_some());
        })
        .unwrap();
        let (again, _) = open(cx, MemoryHost::new(dir.path()));
        cx.update_window(again, |_, window, cx| {
            window.render_frame(cx);
            window.click("tab-history", cx);
            assert!(window.try_find_by_label("History 1 item").is_some());
            window.right_click("file-queries-edit.dql", cx);
            window.click("menu-clear", cx);
            window.click("tab-history", cx);
            assert!(window.try_find_by_label("History 1 item").is_none());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn a_clean_window_picks_up_a_save_and_a_dirty_one_keeps_a_patch(cx: &mut TestAppContext) {
        let (dir, host) = fixture();
        let (window_a, _) = open(cx, host);
        let (window_b, studio_b) = open(cx, MemoryHost::new(dir.path()));
        let path = dir.path().join("queries").join("edit.dql");
        cx.update_window(window_b, |_, window, cx| {
            window.render_frame(cx);
            window.click("editor", cx);
            window.press("z", cx);
        })
        .unwrap();
        cx.update_window(window_a, |_, window, cx| {
            window.render_frame(cx);
            window.click("editor", cx);
            window.press("q", cx);
            window.press("ctrl-s", cx);
        })
        .unwrap();
        let disk = std::fs::read_to_string(&path).unwrap();
        assert!(disk.starts_with('q'), "{disk}");
        cx.update_window(window_b, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find_by_label("File changed on disk").is_some());
            assert!(studio_b.read(cx).buffer().starts_with('z'));
            window.click("recall-patch", cx);
            assert_eq!(std::fs::read_to_string(&path).unwrap(), disk);
        })
        .unwrap();
        let (window_c, studio_c) = open(cx, MemoryHost::new(dir.path()));
        cx.update_window(window_c, |_, window, cx| {
            window.render_frame(cx);
            assert!(studio_c.read(cx).buffer().starts_with('q'));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn completion_inserts_the_keyword(cx: &mut TestAppContext) {
        let (_dir, host) = fixture();
        let (window, studio) = open(cx, host);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("editor", cx);
            window.press("s", cx);
            window.press("e", cx);
            window.press("l", cx);
            window.render_frame(cx);
            assert_eq!(window.find("completion").label(), Some("SELECT"));
            window.press("tab", cx);
            assert!(studio.read(cx).buffer().starts_with("SELECT"));
        })
        .unwrap();
    }
}
