//! The native window. Every control calls [`crate::host::Host`] on this thread.

use std::sync::Arc;

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    div, px, rgb, ClickEvent, Context, Entity, FocusHandle, FontWeight, InteractiveElement,
    IntoElement, KeyBinding, MouseButton, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, TestSupportExt, Window, WindowOptions,
};

use crate::host::{Host, RunResult, TableRow, TreeNode};
use crate::rows::{row_title, ColumnRole};
use crate::tokens::{self, BandKind, HighlightOptions, WordStyle};

gpui_kit::actions!(dql_ui, [Dismiss, RunQuery]);

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

pub struct Studio {
    host: Box<dyn Host>,
    mode: Mode,
    nodes: Vec<TreeNode>,
    open_path: Option<String>,
    text: String,
    head: usize,
    anchor: usize,
    result: Option<RunResult>,
    history: Vec<String>,
    status: String,
    last_ran: String,
    issues: Vec<String>,
    menu: Option<MenuKind>,
    rename: Option<String>,
    sheet: Option<Sheet>,
    settings_open: bool,
    search_list_open: bool,
    search_query: String,
    hits: Vec<crate::host::SearchHit>,
    tables: Vec<TableRow>,
    nav_width: f32,
    results_ratio: f32,
    options: HighlightOptions,
    search: Entity<InputState>,
    editor_focus: FocusHandle,
}

impl Studio {
    pub fn new(host: Box<dyn Host>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        cx.bind_keys([
            KeyBinding::new("escape", Dismiss, None),
            KeyBinding::new("ctrl-enter", RunQuery, None),
        ]);
        let editor_focus = cx.focus_handle();
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
            status: String::new(),
            last_ran: String::new(),
            issues: Vec::new(),
            menu: None,
            rename: None,
            sheet: None,
            settings_open: false,
            search_list_open: false,
            search_query: String::new(),
            hits: Vec::new(),
            tables: Vec::new(),
            nav_width: 220.0,
            results_ratio: 0.46,
            options: HighlightOptions::default(),
            search,
            editor_focus,
        };
        studio.reload_tree();
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

    fn reload_tree(&mut self) {
        self.nodes = self.host.tree().unwrap_or_default();
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.search_list_open {
            self.search_list_open = false;
        } else if self.rename.is_some() {
            self.rename = None;
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

    fn open_path(&mut self, path: String, cx: &mut Context<Self>) {
        match self.host.read_file(&path) {
            Ok(text) => {
                self.text = text;
                self.open_path = Some(path);
                self.head = 0;
                self.anchor = 0;
                self.issues = self
                    .host
                    .lint(&self.text)
                    .into_iter()
                    .map(|issue| issue.message)
                    .collect();
                self.menu = None;
            }
            Err(err) => self.status = err,
        }
        cx.notify();
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
        self.status = if result.message.is_empty() {
            if result.ok {
                result.kind.clone()
            } else {
                "error".into()
            }
        } else {
            result.message.clone()
        };
        self.result = Some(result);
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
        let text = self.text.clone();
        let spans = tokens::statement_spans(&text);
        if spans.is_empty() {
            self.status = "Nothing to run.".into();
            cx.notify();
            return;
        }
        self.history.clear();
        for (index, (from, to)) in spans.iter().enumerate() {
            let statement = text[*from..*to].to_string();
            if index + 1 == spans.len() {
                self.run_text(&statement);
            } else {
                let result = self.host.execute(&statement);
                self.history.push(if result.message.is_empty() {
                    result.kind.clone()
                } else {
                    result.message
                });
            }
        }
        self.menu = None;
        cx.notify();
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
            let kind_menu = kind.clone();
            rows.push(
                div()
                    .id(SharedString::from(id))
                    .test_support()
                    .aria_label(SharedString::from(label))
                    .h(px(28.))
                    .pl(px(12. + depth as f32 * 14.))
                    .cursor_pointer()
                    .child(name)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.open_path(path_click.clone(), cx);
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, _, _, cx| {
                            this.menu = Some(MenuKind::Entry {
                                path: path_menu.clone(),
                                kind: kind_menu.clone(),
                            });
                            this.rename = None;
                            cx.notify();
                        }),
                    )
                    .into_any_element(),
            );
            if kind == "dir" {
                rows.extend(self.file_rows(&node.children, depth + 1, cx));
            }
        }
        rows
    }
}

impl Render for Studio {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.search.read(cx).value().to_string();
        if query != self.search_query {
            self.search_query = query.clone();
            self.search_list_open = !query.trim().is_empty();
            self.hits = if self.search_list_open {
                self.host.search(&query)
            } else {
                Vec::new()
            };
        }
        let endpoint = self.host.endpoint_label();
        let cache = self.host.cache_status();
        let framed = self.framed();
        let show_gutter =
            self.mode == Mode::Query && self.open_path.is_some() && !self.covers_editor();
        let this = cx.entity();
        let dismiss = this.clone();
        let run_query = this.clone();

        div()
            .id("studio")
            .test_support()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xf8fafc))
            .text_color(rgb(0x0f172a))
            .text_size(px(13.))
            .on_action(move |_: &Dismiss, _, cx| {
                dismiss.update(cx, |studio, cx| studio.dismiss(cx));
            })
            .on_action(move |_: &RunQuery, _, cx| {
                run_query.update(cx, |studio, cx| studio.run_selection(cx));
            })
            .child(self.header(&endpoint, cx))
            .child(self.body(cx))
            .when(self.menu.is_some(), |this| this.child(self.menu_panel(cx)))
            .when(self.rename.is_some(), |this| {
                this.child(self.rename_prompt())
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
            .when(show_gutter, |this| this.child(self.gutter(cx)))
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
    fn header(&self, endpoint: &str, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h(px(48.))
            .flex()
            .items_center()
            .gap_2()
            .px(px(12.))
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
                    let name = "untitled.dql";
                    if let Ok(path) = studio.host.create_file(name) {
                        studio.reload_tree();
                        studio.open_path(path, cx);
                    }
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
            .child(
                div()
                    .id("side-split")
                    .test_support()
                    .aria_label("Side split")
                    .h(px(8.))
                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                        if let ClickEvent::Mouse(mouse) = event {
                            if mouse.up.click_count >= 2 {
                                this.nav_width = 220.0;
                                cx.notify();
                            }
                        }
                    })),
            )
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
        div()
            .flex_1()
            .flex()
            .flex_col()
            .children(self.tables.iter().map(|table| {
                let name = table.name.clone();
                div()
                    .id(SharedString::from(format!("table-{name}")))
                    .test_support()
                    .aria_label(SharedString::from(name.clone()))
                    .h(px(28.))
                    .px(px(12.))
                    .child(name.clone())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.sheet = Some(Sheet::Table { name: name.clone() });
                        cx.notify();
                    }))
            }))
    }

    fn results(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ratio_label = format!("{:.0}%", self.results_ratio * 100.0);
        div()
            .h(px(180. * self.results_ratio / 0.46))
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(rgb(0xe2e8f0))
            .child(
                div()
                    .id("results-split")
                    .test_support()
                    .aria_label(SharedString::from(format!("Results split {ratio_label}")))
                    .h(px(8.))
                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                        if let ClickEvent::Mouse(mouse) = event {
                            if mouse.up.click_count >= 2 {
                                this.results_ratio = 0.46;
                                cx.notify();
                            }
                        }
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
            .children(self.history.iter().enumerate().map(|(index, message)| {
                div()
                    .id(SharedString::from(format!("history-{index}")))
                    .test_support()
                    .aria_label(SharedString::from(format!("History {message}")))
                    .px(px(12.))
                    .child(message.clone())
            }))
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
                ("menu-rename", "Rename"),
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

    fn rename_prompt(&self) -> impl IntoElement {
        let name = self
            .rename
            .as_deref()
            .and_then(|path| path.rsplit('/').next())
            .unwrap_or("");
        div()
            .id("rename-prompt")
            .test_support()
            .aria_label(SharedString::from(format!("Rename {name}")))
            .absolute()
            .top(px(120.))
            .left(px(16.))
            .h(px(28.))
            .px(px(8.))
            .bg(rgb(0xffffff))
            .occlude()
            .child(format!("Rename {name}"))
    }

    fn menu_action(&mut self, label: &str, cx: &mut Context<Self>) {
        let target = self.menu.clone();
        match (label, target) {
            ("New file", _) => {
                if let Ok(path) = self.host.create_file("untitled.dql") {
                    self.reload_tree();
                    self.open_path(path, cx);
                }
                self.menu = None;
            }
            ("New folder", _) => {
                let _ = self.host.create_dir("untitled");
                self.reload_tree();
                self.menu = None;
                cx.notify();
            }
            ("Rename", Some(MenuKind::Entry { path, .. })) => {
                self.rename = Some(path);
                cx.notify();
            }
            ("Delete", Some(MenuKind::Entry { path, .. })) => self.delete_entry(&path, cx),
            ("Run file", Some(MenuKind::Entry { path, kind })) if kind == "file" => {
                self.open_path(path, cx);
                self.run_file(cx);
            }
            ("Duplicate", Some(MenuKind::Entry { path, .. })) => {
                let _ = self.host.duplicate(&path);
                self.reload_tree();
                self.menu = None;
                cx.notify();
            }
            ("Clear results", _) => {
                self.result = None;
                self.history.clear();
                self.status.clear();
                self.menu = None;
                cx.notify();
            }
            ("Run folder" | "Move to…", _) => {
                self.status = label.to_string();
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
            .children(self.hits.iter().enumerate().map(|(index, hit)| {
                let path = hit.path.clone();
                let offset = hit.offset.unwrap_or(0);
                text_button(
                    &format!("hit-{index}"),
                    &hit.title,
                    cx,
                    move |studio, cx| {
                        if !path.is_empty() {
                            studio.open_path(path.clone(), cx);
                            studio.head = offset;
                            studio.anchor = offset;
                        }
                        studio.search_list_open = false;
                        cx.notify();
                    },
                )
            }))
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
                el.child(
                    div()
                        .id("row-title")
                        .test_support()
                        .aria_label(SharedString::from(title))
                        .child("Row"),
                )
            })
            .child(text_button("close-sheet", "Close", cx, |studio, cx| {
                studio.sheet = None;
                cx.notify();
            }))
    }
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
            assert!(window.try_find_by_label("No matches").is_some());
            window.press("escape", cx);
            window.click("settings", cx);
            assert!(window.try_find_by_label("(2 days ago)").is_some());
            window.click("clear-cache", cx);
            assert!(window.try_find_by_label("Table cache cleared.").is_some());
        })
        .unwrap();
    }
}
