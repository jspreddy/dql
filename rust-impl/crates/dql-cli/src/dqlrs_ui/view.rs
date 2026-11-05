//! Layout for the native window. It follows the web UI: header, file tree,
//! editor, results, and a table browser.

use std::time::Duration;

use iced::widget::{
    button, column, container, float, keyed, mouse_area, opaque, pane_grid, progress_bar, row,
    rule, scrollable, stack, text, text_editor, text_input, tooltip,
};
use iced::{Color, Element, Fill, Length, Point, Rectangle, Vector};

use crate::web::files::Node;
use crate::web::studio::{menu_labels, StatementCard};

use super::highlight;
use super::icons::{self, Mark};
use super::query;
use super::style::{self, Palette};
use super::{caret_bytes, App, Dialog, Message, Mode};

pub fn view(app: &App) -> Element<'_, Message> {
    let colors = style::palette(app.prefs.dark);
    let body = match app.mode {
        Mode::Query => query(app),
        Mode::Tables => tables(app, colors),
    };
    let window = column![header(app), body, status_bar(app)].spacing(0);
    let mut layers: Vec<Element<'_, Message>> = vec![window.into()];
    if let Some(menu) = floating_menu(app) {
        layers.push(menu);
    }
    if !app.hits.is_empty() {
        layers.push(floating_search(app));
    }
    if let Some(layer) = popup(app) {
        layers.push(layer);
    }
    if !app.notices.is_empty() {
        layers.push(notices(app));
    }
    if layers.len() == 1 {
        layers.pop().unwrap_or_else(|| text("").into())
    } else {
        stack(layers).into()
    }
}

fn header(app: &App) -> Element<'_, Message> {
    let p = style::palette(app.prefs.dark);
    let modes = container(
        row![
            mode_button(
                "Query",
                app.mode == Mode::Query,
                Message::Mode(Mode::Query),
                p
            ),
            mode_button(
                "Tables",
                app.mode == Mode::Tables,
                Message::Mode(Mode::Tables),
                p
            ),
        ]
        .spacing(0),
    )
    .style(move |_| {
        iced::widget::container::Style::default().border(iced::Border {
            color: p.header_line,
            width: 1.0,
            radius: 6.0.into(),
        })
    });
    let search = text_input("Search tables, columns, queries", &app.search)
        .on_input(Message::Search)
        .size(12)
        .padding([2, 10])
        .width(Fill)
        .style(style::header_field(p));
    let endpoint = button(button_label(app.endpoint.clone(), 12.0))
        .on_press(Message::ToggleEnv)
        .padding([0, 8])
        .height(22)
        .style(style::paint(p, style::Look::Pill));
    let theme = tip(
        button(icon_only(
            if app.prefs.dark {
                Mark::Sun
            } else {
                Mark::Moon
            },
            p.header_text,
            14.0,
        ))
        .on_press(Message::ToggleTheme)
        .padding(0)
        .width(26)
        .height(22)
        .style(style::paint(p, style::Look::Pill)),
        "Theme",
        p,
    );
    let settings = tip(
        button(icon_only(Mark::Settings, p.header_text, 14.0))
            .on_press(Message::ToggleSettings)
            .padding(0)
            .width(26)
            .height(22)
            .style(style::paint(p, style::Look::Pill)),
        "Settings",
        p,
    );
    let bar = container(
        row![
            text("dqlrs").size(13).color(p.header_fg),
            modes,
            search,
            endpoint,
            theme,
            settings,
        ]
        .spacing(10)
        .align_y(iced::alignment::Vertical::Center)
        .padding([0, 10])
        .height(Length::Fixed(36.0)),
    )
    .width(Fill)
    .style(style::fill(p.bar));
    let mut col = column![bar].spacing(0);
    if app.show_env || app.show_settings {
        col = col.push(header_pop(app, p));
    }
    col.into()
}

fn header_pop(app: &App, p: Palette) -> Element<'_, Message> {
    let mut body = column![].spacing(8).padding([8, 12]);
    if app.show_env {
        let mut env = column![].spacing(2);
        for (key, value) in &app.env_lines {
            env = env.push(text(format!("{key}: {value}")).size(12).color(p.muted));
        }
        body = body.push(
            container(env)
                .width(Fill)
                .padding(10)
                .style(style::menu_card(p)),
        );
    }
    if app.show_settings {
        body = body.push(
            container(settings_panel(app, p))
                .width(Fill)
                .padding(10)
                .style(style::menu_card(p)),
        );
    }
    container(body)
        .width(Fill)
        .style(style::fill(p.canvas))
        .into()
}

fn search_list(app: &App, p: Palette) -> Element<'_, Message> {
    let groups = [
        ("Tables", &["table"][..]),
        ("Columns", &["column", "index"][..]),
        ("Queries", &["query"][..]),
    ];
    let mut list = column![].spacing(0).padding(6);
    let mut shown = 0usize;
    for (label, kinds) in groups {
        let rows: Vec<_> = app
            .hits
            .iter()
            .enumerate()
            .filter(|(_, hit)| kinds.contains(&hit.kind.as_str()))
            .collect();
        if rows.is_empty() {
            continue;
        }
        list = list.push(text(label.to_ascii_uppercase()).size(11).color(p.muted));
        for (index, hit) in rows {
            shown += 1;
            let mut lines = column![row![
                text(search_kind(&hit.kind))
                    .size(11)
                    .color(p.muted)
                    .width(Length::Fixed(58.0)),
                text(hit.title.clone()).size(13).color(p.text),
            ]
            .spacing(8)]
            .spacing(0);
            if !hit.detail.is_empty() {
                lines = lines.push(
                    row![
                        text("").width(Length::Fixed(58.0)),
                        text(hit.detail.clone()).size(12).color(p.muted),
                    ]
                    .spacing(8),
                );
            }
            list = list.push(
                button(lines)
                    .on_press(Message::PickHit(index))
                    .width(Fill)
                    .padding([5, 6])
                    .style(style::paint(p, style::Look::Quiet)),
            );
        }
    }
    if shown == 0 {
        list = list.push(text("No matches").size(12).color(p.muted));
    }
    container(list)
        .width(Length::Fixed(480.0))
        .style(style::menu_card(p))
        .into()
}

fn search_kind(kind: &str) -> String {
    match kind {
        "table" => "Table",
        "column" => "Column",
        "index" => "Index",
        _ => "Query",
    }
    .to_string()
}

fn settings_panel(app: &App, p: Palette) -> Element<'_, Message> {
    column![
        text("Highlight").size(11).color(p.muted),
        button(
            text(if app.prefs.write_bands {
                "Write bands on"
            } else {
                "Write bands off"
            })
            .size(13)
            .color(p.text)
        )
        .on_press(Message::ToggleBands)
        .padding([4, 8])
        .style(style::paint(p, style::Look::Quiet)),
        text("Keyboard").size(11).color(p.muted),
        text("Run the statement at the cursor    Ctrl + Enter    Ctrl + \\")
            .size(12)
            .color(p.text),
        text("Stop a run    Esc     Save    Ctrl + S")
            .size(12)
            .color(p.text),
        text("Right-click a file for Run file, rename, move, and delete.")
            .size(12)
            .color(p.muted),
    ]
    .spacing(4)
    .into()
}

fn button_label(label: impl Into<String>, size: f32) -> Element<'static, Message> {
    text(label.into())
        .size(size)
        .line_height(1.0)
        .height(Fill)
        .align_y(iced::alignment::Vertical::Center)
        .into()
}

fn icon_only(mark: Mark, color: Color, px: f32) -> Element<'static, Message> {
    container(icons::mark(mark, color, px))
        .width(Fill)
        .height(Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .into()
}

fn labeled_icon(
    mark: Mark,
    color: Color,
    label: &'static str,
    size: f32,
) -> Element<'static, Message> {
    container(
        row![
            icons::mark(mark, color, 14.0),
            text(label).size(size).line_height(1.0),
        ]
        .spacing(6)
        .align_y(iced::alignment::Vertical::Center),
    )
    .height(Fill)
    .align_y(iced::alignment::Vertical::Center)
    .into()
}

fn key_icon(hash: bool, color: Color) -> Element<'static, Message> {
    icons::mark(if hash { Mark::Hash } else { Mark::Range }, color, 12.0)
}

fn mode_button(
    label: &'static str,
    active: bool,
    message: Message,
    p: Palette,
) -> Element<'static, Message> {
    let style = style::paint(
        p,
        if active {
            style::Look::ModeOn
        } else {
            style::Look::ModeOff
        },
    );
    button(button_label(label, 13.0))
        .on_press(message)
        .padding([0, 14])
        .height(26)
        .style(style)
        .into()
}

fn query(app: &App) -> Element<'_, Message> {
    let p = style::palette(app.prefs.dark);
    pane_grid::PaneGrid::new(&app.query_panes, |_pane, slot, _maximized| {
        let body: Element<'_, Message> = match slot {
            super::Slot::Side => tree_pane(app, p),
            super::Slot::Editor => {
                column![editor_head(app, p), dialog(app, p), editor_body(app, p)]
                    .spacing(0)
                    .width(Fill)
                    .height(Fill)
                    .into()
            }
            super::Slot::Results => results(app, p),
            super::Slot::Tables => text("").into(),
        };
        pane_grid::Content::new(body)
    })
    .width(Fill)
    .height(Fill)
    .spacing(1)
    .min_size(140)
    .on_resize(8, |event| Message::PaneRatio(event.split, event.ratio))
    .into()
}

fn tree_pane(app: &App, p: Palette) -> Element<'_, Message> {
    let head = container(
        row![
            button(button_label("Files", 13.0))
                .on_press(Message::MenuRoot)
                .padding([0, 2])
                .height(22)
                .style(style::paint(p, style::Look::Quiet)),
            iced::widget::Space::new().width(Fill),
            tip(
                button(button_label("+", 14.0))
                    .on_press(Message::MenuAct("New file"))
                    .padding([0, 7])
                    .height(22)
                    .style(style::paint(p, style::Look::Control)),
                "New file",
                p,
            ),
        ]
        .spacing(4)
        .align_y(iced::alignment::Vertical::Center),
    )
    .width(Fill)
    .height(Length::Fixed(30.0))
    .padding([0, 8])
    .style(style::fill(p.side));
    let mut tree = column![head].spacing(0);
    if app.menu.as_ref().is_some_and(|menu| menu.kind == "root") {
        tree = tree.push(context_menu("root", p));
    }
    if let Some(root) = &app.tree {
        if let Some(children) = &root.children {
            for child in children {
                tree = tree.push(node_view(app, child, 0, p));
            }
        }
    }
    container(scrollable(tree).height(Fill))
        .width(Fill)
        .height(Fill)
        .style(style::fill(p.side))
        .into()
}

fn context_menu(kind: &str, p: Palette) -> Element<'static, Message> {
    let labels = menu_labels(kind);
    let mut items = column![].spacing(0).padding(4).width(Length::Fixed(188.0));
    for (index, label) in labels.iter().copied().enumerate() {
        let rule = label == "Delete"
            || (index == 1 && matches!(labels.first().copied(), Some("Run file" | "Run folder")));
        if rule {
            items = items.push(separator(p));
        }
        let danger = matches!(label, "Delete" | "Run file" | "Run folder");
        let content = if matches!(label, "Run file" | "Run folder") {
            labeled_icon(Mark::Forward, p.danger, label, 13.0)
        } else {
            button_label(label, 13.0)
        };
        items = items.push(
            button(content)
                .on_press(Message::MenuAct(label))
                .width(Fill)
                .padding([0, 8])
                .height(26)
                .style(style::paint(
                    p,
                    if danger {
                        style::Look::Danger
                    } else {
                        style::Look::Quiet
                    },
                )),
        );
    }
    container(items)
        .padding([2, 6])
        .style(style::menu_card(p))
        .into()
}

fn node_view<'a>(app: &'a App, node: &'a Node, depth: usize, p: Palette) -> Element<'a, Message> {
    let pad = Length::Fixed(depth as f32 * 14.0);
    let mut col = column![tree_row(app, node, pad, p)].spacing(0);
    if node.kind == "dir" {
        if let Some(children) = &node.children {
            for child in children {
                col = col.push(node_view(app, child, depth + 1, p));
            }
        }
    }
    col.into()
}

fn tree_row<'a>(app: &'a App, node: &'a Node, pad: Length, p: Palette) -> Element<'a, Message> {
    let open = node.kind == "file" && node.path == app.open_path;
    let running = node.kind == "file" && node.path == app.running_path;
    let twist = if node.kind == "dir" { "▾" } else { " " };
    let mark = if running {
        container(text(""))
            .width(7)
            .height(7)
            .style(style::chip(p.blue, p.blue))
    } else {
        container(text("")).width(7).height(7)
    };
    let inner = row![
        container(text("")).width(pad),
        text(twist)
            .size(10)
            .color(p.muted)
            .width(Length::Fixed(14.0)),
        text(node.name.clone()).size(13).color(p.text),
        iced::widget::Space::new().width(Fill),
        mark,
        container(text(""))
            .width(2)
            .height(Length::Fixed(22.0))
            .style(style::fill(if open { p.teal } else { Color::TRANSPARENT })),
    ]
    .spacing(4)
    .height(Length::Fixed(22.0))
    .padding([0, 6])
    .align_y(iced::alignment::Vertical::Center);
    let row = mouse_area(inner)
        .on_press(Message::Open(node.path.clone()))
        .on_right_press(Message::Menu(node.path.clone(), node.kind.to_string()));
    container(row)
        .width(Fill)
        .style(style::fill(if open {
            p.selected
        } else {
            Color::TRANSPARENT
        }))
        .into()
}

fn editor_head(app: &App, p: Palette) -> Element<'_, Message> {
    let name = if app.open_path.is_empty() {
        "No file open".to_string()
    } else {
        app.open_path
            .rsplit('/')
            .next()
            .unwrap_or(&app.open_path)
            .to_string()
    };
    let title = column![
        text(name).size(13).color(p.text),
        container(text(""))
            .height(2)
            .width(Fill)
            .style(style::fill(if app.open_path.is_empty() {
                p.paper
            } else {
                p.teal
            })),
    ]
    .spacing(0);
    let mut buttons = row![].spacing(6);
    if !app.lint.is_empty() {
        let errors = app
            .lint
            .iter()
            .filter(|issue| issue.severity == "error")
            .count();
        let label = if errors > 0 {
            format!("{errors} error{}", if errors == 1 { "" } else { "s" })
        } else {
            format!(
                "{} warning{}",
                app.lint.len(),
                if app.lint.len() == 1 { "" } else { "s" }
            )
        };
        buttons = buttons.push(
            container(
                text(label)
                    .size(12)
                    .line_height(1.0)
                    .height(Fill)
                    .align_y(iced::alignment::Vertical::Center)
                    .color(if errors > 0 { p.danger } else { p.amber }),
            )
            .height(22)
            .padding([0, 7])
            .style(style::fill(if errors > 0 {
                if p.dark {
                    style::rgb(0x3f, 0x1d, 0x1d)
                } else {
                    style::rgb(0xfe, 0xf2, 0xf2)
                }
            } else {
                p.amber_bg
            })),
        );
    }
    if !app.open_path.is_empty() {
        buttons = buttons.push(
            button(labeled_icon(Mark::Forward, p.danger, "Run file", 12.0))
                .on_press(Message::RunFile)
                .padding([0, 8])
                .height(22)
                .style(style::paint(p, style::Look::Outline)),
        );
        buttons = buttons.push(
            button(labeled_icon(Mark::Play, Color::WHITE, "Run", 12.0))
                .on_press(Message::RunCursor)
                .padding([0, 8])
                .height(22)
                .style(style::paint(p, style::Look::Primary)),
        );
    }
    if !app.running_path.is_empty() {
        buttons = buttons.push(
            button(button_label("Stop", 12.0))
                .on_press(Message::Stop)
                .padding([0, 10])
                .height(22)
                .style(style::paint(p, style::Look::Stop)),
        );
    }
    container(
        row![title, buttons]
            .spacing(8)
            .align_y(iced::alignment::Vertical::Center)
            .padding([4, 10])
            .height(Length::Fixed(36.0)),
    )
    .width(Fill)
    .style(style::fill(p.paper))
    .into()
}

fn dialog(app: &App, p: Palette) -> Element<'_, Message> {
    match &app.dialog {
        Dialog::None => container(text("")).height(0).into(),
        Dialog::Conflict => container(
            row![
                text("This file changed on disk. Your edits were kept.")
                    .size(12)
                    .color(p.amber),
                button(button_label("Load disk", 12.0))
                    .on_press(Message::ReloadDisk)
                    .padding([0, 8])
                    .height(22)
                    .style(style::paint(p, style::Look::Primary)),
                button(button_label("Keep mine", 12.0))
                    .on_press(Message::KeepEdits)
                    .padding([0, 8])
                    .height(22)
                    .style(style::paint(p, style::Look::Quiet)),
            ]
            .spacing(8)
            .align_y(iced::alignment::Vertical::Center)
            .padding(8),
        )
        .width(Fill)
        .style(style::fill(p.amber_bg))
        .into(),
        Dialog::Name { title, .. } => container(
            row![
                text(*title).size(13).color(p.text),
                text_input("name", &app.dialog_text)
                    .on_input(Message::DialogText)
                    .size(13)
                    .padding([4, 8])
                    .style(style::field(p)),
                button(button_label("OK", 12.0))
                    .on_press(Message::DialogOk)
                    .padding([0, 10])
                    .height(22)
                    .style(style::paint(p, style::Look::Primary)),
                button(button_label("Cancel", 12.0))
                    .on_press(Message::DialogCancel)
                    .padding([0, 8])
                    .height(22)
                    .style(style::paint(p, style::Look::Quiet)),
            ]
            .spacing(8)
            .align_y(iced::alignment::Vertical::Center)
            .padding(8),
        )
        .width(Fill)
        .style(style::fill(p.paper))
        .into(),
        Dialog::Move { path } => container(
            row![
                text(format!("Move {path} to")).size(12).color(p.text),
                text_input("folder or path", &app.dialog_text)
                    .on_input(Message::DialogText)
                    .size(13)
                    .padding([4, 8])
                    .style(style::field(p)),
                button(button_label("Move", 12.0))
                    .on_press(Message::DialogOk)
                    .padding([0, 10])
                    .height(22)
                    .style(style::paint(p, style::Look::Primary)),
                button(button_label("Cancel", 12.0))
                    .on_press(Message::DialogCancel)
                    .padding([0, 8])
                    .height(22)
                    .style(style::paint(p, style::Look::Quiet)),
            ]
            .spacing(8)
            .align_y(iced::alignment::Vertical::Center)
            .padding(8),
        )
        .width(Fill)
        .style(style::fill(p.paper))
        .into(),
    }
}

fn editor_body(app: &App, p: Palette) -> Element<'_, Message> {
    let source = editor_text_view(app);
    let lines: Vec<&str> = if source.is_empty() {
        vec![""]
    } else {
        source.split('\n').collect()
    };
    let mut gutter = column![]
        .spacing(0)
        .width(Length::Fixed(query::GUTTER_W))
        .padding(iced::Padding {
            top: query::PAD_TOP,
            right: 0.0,
            bottom: query::PAD_TOP,
            left: 0.0,
        });
    for (index, line) in lines.iter().enumerate() {
        let number = index + 1;
        let issue = app.lint.iter().find(|issue| issue.line == number);
        let write = app.prefs.write_bands && line_is_write(line);
        let mark = if issue.is_some() { "●" } else { "" };
        gutter = gutter.push(
            container(
                row![
                    text(mark).size(8).color(
                        if issue.is_some_and(|issue| issue.severity == "error") {
                            p.danger
                        } else {
                            p.amber
                        }
                    ),
                    text(number.to_string()).size(11).color(p.muted),
                ]
                .spacing(2)
                .align_y(iced::alignment::Vertical::Center),
            )
            .height(Length::Fixed(query::LINE_H))
            .width(Fill)
            .padding([0.0, 4.0])
            .style(style::fill(if write {
                Color {
                    a: if p.dark { 0.28 } else { 0.10 },
                    ..p.danger
                }
            } else {
                Color::TRANSPARENT
            })),
        );
    }
    let editor = highlight::editor(&app.editor)
        .on_action(Message::Editor)
        .key_binding(editor_keys)
        .size(13)
        .wrapping(iced::widget::text::Wrapping::None)
        .line_height(iced::widget::text::LineHeight::Absolute(
            query::LINE_H.into(),
        ))
        .height(Length::Shrink)
        .padding([query::PAD_TOP, query::PAD_LEFT])
        .style(style::editor_surface(p));
    let mut below = column![].spacing(4);
    if !app.running_path.is_empty() {
        let total = app.live.total.unwrap_or(0);
        let mut line = row![text(format!(
            "Running {}  {} {}/{}",
            app.running_path, app.live.phase, app.live.done, total
        ))
        .size(12)
        .color(p.blue)]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center);
        if total > 0 {
            let done = app.live.done as f32;
            let goal = total as f32;
            line = line.push(
                progress_bar(0.0..=goal, done.min(goal))
                    .length(Length::Fixed(120.0))
                    .girth(Length::Fixed(6.0))
                    .style(move |_theme| progress_bar::Style {
                        background: p.line.into(),
                        bar: p.teal.into(),
                        border: iced::Border {
                            color: Color::TRANSPARENT,
                            width: 0.0,
                            radius: 3.0.into(),
                        },
                    }),
            );
        }
        below = below.push(line);
    }
    if !app.suggest.is_empty() {
        let mut suggestions = column![].spacing(0).padding(4);
        for (index, hit) in app.suggest.iter().take(8).enumerate() {
            suggestions = suggestions.push(
                button(
                    row![
                        text(hit.title.clone())
                            .size(12)
                            .line_height(1.0)
                            .font(iced::Font::MONOSPACE)
                            .color(p.text),
                        iced::widget::Space::new().width(Fill),
                        text(hit.kind.clone())
                            .size(11)
                            .line_height(1.0)
                            .color(p.muted),
                    ]
                    .spacing(8),
                )
                .on_press(Message::UseSuggest(index))
                .width(Fill)
                .padding([4, 8])
                .style(style::paint(p, style::Look::Quiet)),
            );
        }
        below = below.push(
            container(suggestions)
                .width(Length::Fixed(320.0))
                .style(style::menu_card(p)),
        );
    }
    for issue in app.lint.iter().take(6) {
        let color = if issue.severity == "error" {
            p.danger
        } else {
            p.amber
        };
        below = below.push(
            text(format!("{}:{} {}", issue.line, issue.column, issue.message))
                .size(12)
                .color(color),
        );
    }
    let document = row![container(gutter).style(style::fill(p.gutter)), editor]
        .spacing(0)
        .width(Fill);
    let mut document = iced::widget::Stack::new().push(document);
    if let Some(frame) = run_frame(app, p, &source) {
        document = document.push_under(frame);
    }
    let scroller = scrollable(document)
        .id(app.editor_scroll.clone())
        .on_scroll(Message::EditorScrolled)
        .width(Fill)
        .height(Fill);
    let mut editor_stack = iced::widget::Stack::new()
        .width(Fill)
        .height(Fill)
        .push(scroller);
    if let Some(button) = run_button(app, p, &source) {
        editor_stack = editor_stack.push(button);
    }
    container(column![editor_stack, below.padding([4, 10])].height(Fill))
        .width(Fill)
        .height(Fill)
        .style(style::fill(p.paper))
        .into()
}

fn run_frame(app: &App, p: Palette, source: &str) -> Option<Element<'static, Message>> {
    let (head, anchor) = caret_bytes(&app.editor, source);
    let target = query::run_target(source, head, anchor)?;
    let frame = query::frame(source, target)?;
    Some(
        container(
            container(iced::widget::Space::new())
                .width(Length::Fixed(frame.width))
                .height(Length::Fixed(frame.height))
                .style(style::run_window(p)),
        )
        .padding(iced::Padding {
            top: frame.y.max(0.0),
            right: 0.0,
            bottom: 0.0,
            left: frame.x.max(0.0),
        })
        .into(),
    )
}

fn run_button(app: &App, p: Palette, source: &str) -> Option<Element<'static, Message>> {
    let (head, anchor) = caret_bytes(&app.editor, source);
    let target = query::run_target(source, head, anchor)?;
    let frame = query::frame(source, target)?;
    let (left, top) = query::button_origin(
        frame,
        app.editor_offset_x,
        app.editor_offset_y,
        app.editor_view_w,
        app.editor_view_h,
    )?;
    let busy = !app.running_path.is_empty();
    let ink = if busy {
        Color::from_rgba(1.0, 1.0, 1.0, 0.55)
    } else {
        Color::WHITE
    };
    let mut control = button(icon_only(Mark::Play, ink, 11.0))
        .padding(0)
        .width(query::BUTTON)
        .height(query::BUTTON)
        .style(style::run_float(p, !busy));
    if !busy {
        control = control.on_press(Message::RunCursor);
    }
    Some(
        float(tip(control, "Run selection", p))
            .translate(move |_bounds, _viewport| Vector::new(left, top))
            .into(),
    )
}

fn editor_text_view(app: &App) -> String {
    super::editor_text(&app.editor)
}

fn line_is_write(line: &str) -> bool {
    let word = line
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    matches!(
        word.as_str(),
        "INSERT" | "UPDATE" | "DELETE" | "DROP" | "CREATE"
    )
}

fn results(app: &App, p: Palette) -> Element<'_, Message> {
    let mut cards = keyed::Column::new().spacing(8).padding(8).width(Fill);
    for (index, card) in app.results.iter().take(20).enumerate() {
        cards = cards.push(index, card_view(app, index, card, p));
    }
    container(
        column![
            text("Results").size(12).color(p.muted),
            scrollable(cards).height(Fill).width(Fill),
        ]
        .spacing(4)
        .padding([8, 0])
        .height(Fill)
        .width(Fill),
    )
    .height(Fill)
    .width(Fill)
    .style(style::fill(p.canvas))
    .into()
}

fn card_view<'a>(
    app: &'a App,
    index: usize,
    card: &'a StatementCard,
    p: Palette,
) -> Element<'a, Message> {
    if !card.items.is_empty() {
        return item_card(app, index, card, p);
    }
    let (bg, fg) = if card.ok {
        (p.ok_bg, p.ok)
    } else {
        (p.amber_bg, p.danger)
    };
    let message = if !card.ok {
        card.failure_text()
    } else if !card.message.is_empty() {
        card.message.clone()
    } else {
        card.kind.clone()
    };
    let note = container(text(message).size(13).color(fg))
        .padding([8, 10])
        .width(Fill)
        .style(move |_| {
            iced::widget::container::Style::default()
                .background(bg)
                .border(iced::Border {
                    color: bg,
                    width: 1.0,
                    radius: 8.0.into(),
                })
        });
    let Some(help) = &card.help else {
        return note.into();
    };
    let pretty = serde_json::to_string_pretty(help).unwrap_or_else(|_| help.to_string());
    column![
        note,
        text(pretty)
            .font(iced::Font::MONOSPACE)
            .size(12)
            .color(p.text),
    ]
    .spacing(6)
    .into()
}

fn item_card<'a>(
    app: &'a App,
    index: usize,
    card: &'a StatementCard,
    p: Palette,
) -> Element<'a, Message> {
    let title = table_name(&card.statement).unwrap_or_else(|| "Result".to_string());
    let rows = card.items.len();
    let open = app.open_cards.get(index).copied().unwrap_or(false);
    let mut head = row![
        text(title)
            .size(13.5)
            .font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..iced::Font::MONOSPACE
            })
            .color(if app.prefs.dark {
                style::rgb(0xe7, 0xee, 0xf6)
            } else {
                style::rgb(0x12, 0x20, 0x2b)
            }),
        stat_chip(
            format!("{rows} {}", if rows == 1 { "row" } else { "rows" }),
            p,
        ),
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center);
    if let Some(capacity) = capacity_label(card.read_units, card.write_units) {
        head = head.push(stat_chip(capacity, p));
    }
    head = head.push(stat_chip(elapsed_label(card.elapsed_ms), p));
    if !card.columns.is_empty() || !card.items.is_empty() {
        head = head.push(tip(
            button(icon_only(Mark::Schema, p.muted, 14.0))
                .on_press(Message::ShowResultSchema(index))
                .padding(0)
                .width(22)
                .height(22)
                .style(style::paint(p, style::Look::Quiet)),
            "Result structure",
            p,
        ));
    }
    let head = container(head)
        .width(Fill)
        .padding([8, 12])
        .style(style::fill(if app.prefs.dark {
            style::rgb(0x15, 0x24, 0x2a)
        } else {
            style::rgb(0xf6, 0xfb, 0xf9)
        }));
    let mut body = column![head, hairline(p.line_strong)]
        .width(Fill)
        .spacing(0);
    if open {
        body = body.push(
            container(
                text(card.statement.clone())
                    .font(iced::Font::MONOSPACE)
                    .size(12.5)
                    .color(if app.prefs.dark {
                        style::rgb(0xc5, 0xd2, 0xe0)
                    } else {
                        style::rgb(0x33, 0x41, 0x55)
                    }),
            )
            .width(Fill)
            .padding([10, 12])
            .style(style::fill(if app.prefs.dark {
                style::rgb(0x14, 0x1c, 0x26)
            } else {
                style::rgb(0xf8, 0xfa, 0xfc)
            })),
        );
        body = body.push(hairline(p.line_strong));
    }
    body = body.push(items_table(
        &card.columns,
        &card.items,
        true,
        true,
        p,
        &format!("result-{index}"),
    ));
    let card_face = container(body).width(Fill).style(style::card(p));
    row![query_dot(open, index, p), card_face]
        .spacing(6)
        .align_y(iced::alignment::Vertical::Top)
        .into()
}

fn query_dot(open: bool, index: usize, p: Palette) -> Element<'static, Message> {
    let color = if open {
        p.teal
    } else {
        style::rgb(0x9a, 0xa3, 0xad)
    };
    button(container(text("")).width(8).height(8).style(move |_| {
        iced::widget::container::Style::default()
            .background(color)
            .border(iced::Border {
                color,
                width: 0.0,
                radius: 4.0.into(),
            })
    }))
    .on_press(Message::ToggleCard(index))
    .padding([13, 0])
    .style(style::paint(p, style::Look::Quiet))
    .into()
}

fn stat_chip(label: String, p: Palette) -> Element<'static, Message> {
    let fg = if p.dark {
        style::rgb(0xc5, 0xd2, 0xe0)
    } else {
        style::rgb(0x33, 0x41, 0x55)
    };
    container(
        text(label)
            .size(12)
            .line_height(1.0)
            .height(Fill)
            .align_y(iced::alignment::Vertical::Center)
            .color(fg),
    )
    .height(22)
    .padding([0, 8])
    .style(style::chip(p.chip, fg))
    .into()
}

fn items_table<'a>(
    columns: &'a [crate::web::shape::Column],
    items: &'a [serde_json::Value],
    clickable: bool,
    result_card: bool,
    p: Palette,
    scope: &str,
) -> Element<'a, Message> {
    let columns = ordered_columns(columns, items);
    let shown: Vec<_> = items.iter().take(50).collect();
    let widths: Vec<f32> = columns
        .iter()
        .map(|column| {
            let header = cell_width(&column.name) + key_extra(column);
            shown
                .iter()
                .map(|item| cell_text(item.get(&column.name)))
                .map(|value| cell_width(&value))
                .fold(header, f32::max)
                .clamp(72.0, 420.0)
        })
        .collect();
    let header = grid_row(
        peek_cell(false, String::new(), true, result_card, p),
        columns.iter().zip(&widths).map(|(column, width)| {
            let paint = style::column_paint(p, column.table.as_deref(), column.index.as_deref());
            ruled(
                header_label(column, p.text, paint.mark),
                *width,
                paint.head,
                paint.head_line,
                true,
            )
        }),
        p.gutter,
    );
    let last = shown.len().saturating_sub(1);
    let mut rows = column![].spacing(0);
    for (index, item) in shown.iter().copied().enumerate() {
        let json = serde_json::to_string_pretty(item).unwrap_or_default();
        let stripe = index % 2 == 1;
        let rule = index != last;
        rows = rows.push(grid_row(
            peek_cell(clickable, json, false, result_card, p),
            columns.iter().zip(&widths).map(|(column, width)| {
                let paint =
                    style::column_paint(p, column.table.as_deref(), column.index.as_deref());
                let bg = if paint.keyed {
                    paint.cell
                } else if stripe {
                    row_stripe(p)
                } else {
                    paint.cell
                };
                ruled(
                    text(cell_text(item.get(&column.name)))
                        .size(12)
                        .color(p.text)
                        .into(),
                    *width,
                    bg,
                    paint.cell_line,
                    rule,
                )
            }),
            if stripe { row_stripe(p) } else { p.paper },
        ));
    }
    let head_id = format!("{scope}-head");
    let body_height = (shown.len().max(1) as f32 * 32.0).min(240.0);
    let head = scrollable(header)
        .id(iced::widget::Id::from(head_id.clone()))
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::default(),
        ))
        .width(Fill)
        .height(Length::Fixed(32.0));
    let body = scrollable(rows)
        .direction(scrollable::Direction::Both {
            vertical: scrollable::Scrollbar::default(),
            horizontal: scrollable::Scrollbar::default(),
        })
        .width(Fill)
        .height(Length::Fixed(body_height))
        .on_scroll(move |viewport| {
            Message::SyncGrid(head_id.clone(), viewport.absolute_offset().x)
        });
    column![head, body].spacing(0).width(Fill).into()
}

fn grid_row<'a>(
    peek: Element<'a, Message>,
    cells: impl Iterator<Item = Element<'a, Message>>,
    band: Color,
) -> Element<'a, Message> {
    let mut line = row![peek]
        .spacing(0)
        .align_y(iced::alignment::Vertical::Center);
    for cell in cells {
        line = line.push(cell);
    }
    container(line).style(style::fill(band)).into()
}

fn hairline(color: Color) -> Element<'static, Message> {
    container(text(""))
        .height(1)
        .width(Fill)
        .style(style::fill(color))
        .into()
}

fn peek_cell(
    clickable: bool,
    json: String,
    header: bool,
    result_card: bool,
    p: Palette,
) -> Element<'static, Message> {
    let bg = if header && result_card {
        if p.dark {
            style::rgb(0x1e, 0x29, 0x36)
        } else {
            style::rgb(0xf4, 0xf7, 0xf8)
        }
    } else if header {
        p.gutter
    } else {
        p.paper
    };
    let eye: Element<'static, Message> = if clickable {
        icons::mark(Mark::Eye, p.teal, 14.0)
    } else {
        text("").size(11).into()
    };
    let cell = container(eye)
        .width(28)
        .align_x(iced::alignment::Horizontal::Center)
        .padding([3.0, 0.0])
        .style(move |_| {
            iced::widget::container::Style::default()
                .background(bg)
                .border(iced::Border {
                    color: p.line,
                    width: 1.0,
                    radius: 0.0.into(),
                })
        });
    if !clickable {
        return cell.into();
    }
    tip(
        button(cell)
            .on_press(Message::ShowRow(json))
            .padding(0)
            .width(28)
            .style(style::paint(p, style::Look::Quiet)),
        "Row",
        p,
    )
}

fn ruled(
    label: Element<'static, Message>,
    width: f32,
    bg: Color,
    line: Color,
    rule: bool,
) -> Element<'static, Message> {
    let body = container(label)
        .width(Length::Fixed(width))
        .padding([3.0, 8.0]);
    let mut cell = column![body].spacing(0).width(Length::Fixed(width));
    if rule {
        cell = cell.push(hairline_fixed(line, width));
    }
    container(cell)
        .width(Length::Fixed(width))
        .style(style::fill(bg))
        .into()
}

fn hairline_fixed(color: Color, width: f32) -> Element<'static, Message> {
    container(text(""))
        .height(1)
        .width(Length::Fixed(width))
        .style(style::fill(color))
        .into()
}

fn row_stripe(p: Palette) -> Color {
    if p.dark {
        style::rgb(0x1a, 0x24, 0x30)
    } else {
        style::rgb(0xf8, 0xfa, 0xfb)
    }
}

fn header_label(
    column: &crate::web::shape::Column,
    fg: Color,
    mark: Color,
) -> Element<'static, Message> {
    let mut line = row![text(column.name.clone()).size(12).color(fg)]
        .spacing(4)
        .align_y(iced::alignment::Vertical::Center);
    if column.table.as_deref() == Some("hash") || column.index.as_deref() == Some("hash") {
        line = line.push(key_icon(true, mark));
    }
    if column.table.as_deref() == Some("range") || column.index.as_deref() == Some("range") {
        line = line.push(key_icon(false, mark));
    }
    line.into()
}

fn tables(app: &App, p: Palette) -> Element<'_, Message> {
    pane_grid::PaneGrid::new(&app.table_panes, |_pane, slot, _maximized| {
        let body = match slot {
            super::Slot::Side => table_side(app, p),
            super::Slot::Tables => table_page(app, p),
            _ => text("").into(),
        };
        pane_grid::Content::new(body)
    })
    .width(Fill)
    .height(Fill)
    .spacing(1)
    .min_size(140)
    .on_resize(8, |event| Message::PaneRatio(event.split, event.ratio))
    .into()
}

fn table_side(app: &App, p: Palette) -> Element<'_, Message> {
    let head = container(
        row![
            text_input("catalog", &app.table_filter)
                .on_input(Message::TableFilter)
                .size(12)
                .padding([2, 6])
                .width(Fill)
                .style(style::field(p)),
            tip(
                button(icon_only(Mark::Refresh, p.muted, 14.0))
                    .on_press(Message::RefreshTables(true))
                    .padding(0)
                    .width(22)
                    .height(22)
                    .style(style::paint(p, style::Look::Control)),
                "Refresh",
                p,
            ),
            button(button_label("Clear", 12.0))
                .on_press(Message::ClearCache)
                .padding([0, 8])
                .height(22)
                .style(style::paint(p, style::Look::Control)),
        ]
        .spacing(4)
        .align_y(iced::alignment::Vertical::Center),
    )
    .width(Fill)
    .padding([4, 8]);
    let mut list = keyed::Column::<u64, _, _, _>::new()
        .spacing(2)
        .padding(6)
        .width(Fill);
    list = list.push(0, head);
    list = list.push(
        1,
        text(if app.cache_text.is_empty() {
            app.table_note.clone()
        } else {
            format!("{}  {}", app.table_note, app.cache_text)
        })
        .size(11)
        .color(p.muted),
    );
    for table in &app.tables {
        let selected = table.name == app.open_table;
        let mut keys = row![].spacing(6);
        for key in &table.keys {
            let color = if key.role == "hash" {
                if p.dark {
                    style::rgb(0x86, 0xef, 0xac)
                } else {
                    style::rgb(0x3f, 0x6f, 0x57)
                }
            } else if p.dark {
                style::rgb(0x93, 0xc5, 0xfd)
            } else {
                style::rgb(0x3c, 0x63, 0x99)
            };
            keys = keys.push(
                row![
                    key_icon(key.role == "hash", color),
                    text(key.name.clone()).size(12).color(p.muted),
                ]
                .spacing(3)
                .align_y(iced::alignment::Vertical::Center),
            );
        }
        let label = column![
            text(table.name.clone()).size(13).font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..iced::Font::default()
            }),
            keys,
        ]
        .spacing(1);
        list = list.push(
            name_key(&table.name),
            button(label)
                .on_press(Message::OpenTable(table.name.clone()))
                .width(Fill)
                .padding([3, 8])
                .style(style::paint(
                    p,
                    if selected {
                        style::Look::Selected
                    } else {
                        style::Look::Quiet
                    },
                )),
        );
    }
    container(scrollable(list).height(Fill))
        .width(Fill)
        .height(Fill)
        .style(style::fill(p.side))
        .into()
}

fn table_page(app: &App, p: Palette) -> Element<'_, Message> {
    let count = app.table_items.len();
    let start = if count == 0 {
        0
    } else {
        app.table_page * 50 + 1
    };
    let end = if count == 0 { 0 } else { start + count - 1 };
    let showing = if count == 0 {
        "No rows".to_string()
    } else {
        format!("Showing {start}–{end}")
    };
    let title = if app.open_table.is_empty() {
        "Select a table".to_string()
    } else {
        app.open_table.clone()
    };
    let mut previous = button(button_label("Previous", 12.0))
        .padding([0, 8])
        .height(22)
        .style(style::paint(p, style::Look::Control));
    if app.table_page > 0 {
        previous = previous.on_press(Message::TablePage(-1));
    }
    let mut next = button(button_label("Next", 12.0))
        .padding([0, 8])
        .height(22)
        .style(style::paint(p, style::Look::Control));
    if app.table_has_more {
        next = next.on_press(Message::TablePage(1));
    }
    let mut page = column![container(
        row![
            text(title).size(16).font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..iced::Font::default()
            }),
            text("50 rows at a time").size(12).color(p.muted),
            iced::widget::Space::new().width(Fill),
            tip(
                button(icon_only(Mark::Schema, p.muted, 14.0))
                    .on_press(Message::ShowTableSchema)
                    .padding(0)
                    .width(22)
                    .height(22)
                    .style(style::paint(p, style::Look::Quiet)),
                "Table structure",
                p,
            ),
        ]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center),
    )
    .width(Fill)
    .padding([10, 14])]
    .spacing(0);
    if !app.table_items.is_empty() {
        page = page.push(items_table(
            &app.table_columns,
            &app.table_items,
            true,
            false,
            p,
            "table",
        ));
    }
    page = page.push(
        container(
            row![text(showing).size(12).color(p.muted), previous, next]
                .spacing(8)
                .align_y(iced::alignment::Vertical::Center),
        )
        .width(Fill)
        .padding([8, 14]),
    );
    container(scrollable(page).height(Fill))
        .width(Fill)
        .height(Fill)
        .style(style::fill(p.paper))
        .into()
}

fn status_bar(app: &App) -> Element<'_, Message> {
    let p = style::palette(app.prefs.dark);
    container(
        text(if app.status.is_empty() {
            "Ready".to_string()
        } else {
            app.status.clone()
        })
        .size(12)
        .color(p.muted),
    )
    .width(Fill)
    .padding([3, 10])
    .style(style::fill(p.status))
    .into()
}

fn table_name(statement: &str) -> Option<String> {
    let text = statement.split("--").next().unwrap_or(statement);
    let mut words = text.split_whitespace();
    let mut first = words.next()?.to_ascii_lowercase();
    if first == "analyze" {
        first = words.next()?.to_ascii_lowercase();
    }
    if first != "select" && first != "scan" {
        return None;
    }
    let mut from = false;
    for word in words {
        if from {
            let name: String = word
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-'))
                .collect();
            return if name.is_empty() { None } else { Some(name) };
        }
        if word.eq_ignore_ascii_case("from") {
            from = true;
        }
    }
    None
}

fn capacity_label(read_units: f64, write_units: f64) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(read) = unit_text(read_units) {
        parts.push(format!("{read} RCU"));
    }
    if let Some(write) = unit_text(write_units) {
        parts.push(format!("{write} WCU"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
    }
}

fn unit_text(value: f64) -> Option<String> {
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    let rounded = (value * 100.0).round() / 100.0;
    if rounded == 0.0 {
        None
    } else if rounded.fract() == 0.0 {
        Some(format!("{rounded:.0}"))
    } else {
        Some(rounded.to_string())
    }
}

fn elapsed_label(elapsed_ms: u64) -> String {
    if elapsed_ms < 1000 {
        return format!("{elapsed_ms} ms");
    }
    let seconds = elapsed_ms as f64 / 1000.0;
    let text = if seconds < 10.0 {
        format!("{seconds:.1}")
    } else {
        format!("{seconds:.0}")
    };
    format!("{} s", text.trim_end_matches(".0"))
}

fn ordered_columns(
    columns: &[crate::web::shape::Column],
    items: &[serde_json::Value],
) -> Vec<crate::web::shape::Column> {
    let mut present = Vec::new();
    for item in items {
        let Some(map) = item.as_object() else {
            continue;
        };
        for key in map.keys() {
            if !present.iter().any(|name| name == key) {
                present.push(key.clone());
            }
        }
    }
    let described = |name: String| crate::web::shape::Column {
        name,
        table: None,
        index: None,
    };
    if columns.is_empty() {
        return present.into_iter().map(described).collect();
    }
    let mut ordered = Vec::new();
    for column in columns {
        if present.iter().any(|name| name == &column.name)
            && !ordered
                .iter()
                .any(|kept: &crate::web::shape::Column| kept.name == column.name)
        {
            ordered.push(column.clone());
        }
    }
    if ordered.is_empty() {
        return present.into_iter().map(described).collect();
    }
    let mut extra: Vec<_> = present
        .into_iter()
        .filter(|name| !ordered.iter().any(|column| column.name == *name))
        .collect();
    extra.sort();
    ordered.extend(extra.into_iter().map(described));
    ordered
}

fn key_extra(column: &crate::web::shape::Column) -> f32 {
    let hash = column.table.as_deref() == Some("hash") || column.index.as_deref() == Some("hash");
    let range =
        column.table.as_deref() == Some("range") || column.index.as_deref() == Some("range");
    match (hash, range) {
        (true, true) => 32.0,
        (true, false) | (false, true) => 16.0,
        (false, false) => 0.0,
    }
}

fn cell_text(value: Option<&serde_json::Value>) -> String {
    match value {
        None | Some(serde_json::Value::Null) => String::new(),
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    }
}

fn cell_width(text: &str) -> f32 {
    text.chars().count() as f32 * 7.2 + 16.0
}

fn popup(app: &App) -> Option<Element<'_, Message>> {
    let p = style::palette(app.prefs.dark);
    if let Some(json) = &app.row_json {
        return Some(modal(row_dialog(app, json, p), Message::CloseRow, p));
    }
    app.schema
        .as_ref()
        .map(|kind| modal(schema_dialog(app, kind, p), Message::CloseSchema, p))
}

fn modal(card: Element<'static, Message>, close: Message, p: Palette) -> Element<'static, Message> {
    let scrim = opaque(
        mouse_area(
            container(text(""))
                .width(Fill)
                .height(Fill)
                .style(style::fill(style::scrim(p.dark))),
        )
        .on_press(close),
    );
    let sheet = opaque(
        container(card)
            .width(Fill)
            .height(Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .padding(28),
    );
    stack![scrim, sheet].into()
}

fn row_dialog(app: &App, json: &str, p: Palette) -> Element<'static, Message> {
    let (title, columns, _) = row_match(app, json);
    let mut chips = row![].spacing(6);
    for column in &columns {
        let hash =
            column.table.as_deref() == Some("hash") || column.index.as_deref() == Some("hash");
        let range =
            column.table.as_deref() == Some("range") || column.index.as_deref() == Some("range");
        if !hash && !range {
            continue;
        }
        if title.starts_with(&format!("{} = ", column.name)) {
            continue;
        }
        let paint = style::column_paint(p, column.table.as_deref(), column.index.as_deref());
        chips = chips.push(
            container(
                row![
                    key_icon(hash, paint.mark),
                    text(column.name.clone()).size(12).color(paint.mark),
                ]
                .spacing(3)
                .align_y(iced::alignment::Vertical::Center),
            )
            .padding([2, 6])
            .style(style::chip(paint.head, paint.head_line)),
        );
    }
    let head = row![
        column![
            text("ROW").size(11).color(p.teal).font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..iced::Font::default()
            }),
            text(title).size(16).font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..iced::Font::default()
            }),
            chips,
        ]
        .spacing(4),
        iced::widget::Space::new().width(Fill),
        button(icon_only(Mark::Close, p.muted, 12.0))
            .on_press(Message::CloseRow)
            .padding(0)
            .width(28)
            .height(28)
            .style(style::paint(p, style::Look::Quiet)),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center);
    dialog_shell("Row", head.into(), json_block(json, p), p)
}

fn row_match(
    app: &App,
    json: &str,
) -> (
    String,
    Vec<crate::web::shape::Column>,
    Option<serde_json::Value>,
) {
    let mut sources: Vec<(&[serde_json::Value], &[crate::web::shape::Column])> =
        vec![(&app.table_items, &app.table_columns)];
    for card in &app.results {
        sources.push((&card.items, &card.columns));
    }
    for (items, columns) in sources {
        for item in items {
            if serde_json::to_string_pretty(item).ok().as_deref() == Some(json) {
                return (
                    row_title(columns, item),
                    columns.to_vec(),
                    Some(item.clone()),
                );
            }
        }
    }
    ("Row".to_string(), Vec::new(), None)
}

fn row_title(columns: &[crate::web::shape::Column], item: &serde_json::Value) -> String {
    let hash = columns.iter().find(|column| {
        column.table.as_deref() == Some("hash") || column.index.as_deref() == Some("hash")
    });
    if let Some(column) = hash {
        if let Some(serde_json::Value::String(value)) = item.get(&column.name) {
            if !value.is_empty() && value.chars().count() <= 48 {
                return format!("{} = {value}", column.name);
            }
        }
    }
    "Row".to_string()
}

fn json_block(json: &str, p: Palette) -> Element<'static, Message> {
    let mut lines = column![].spacing(0);
    for line in json.lines() {
        lines = lines.push(json_line(line, p));
    }
    container(lines)
        .width(Fill)
        .padding([12, 14])
        .style(move |_| {
            iced::widget::container::Style::default()
                .background(if p.dark {
                    style::rgb(0x14, 0x1c, 0x26)
                } else {
                    style::rgb(0xf8, 0xfa, 0xfc)
                })
                .border(iced::Border {
                    color: p.line,
                    width: 1.0,
                    radius: 8.0.into(),
                })
        })
        .into()
}

fn json_line(line: &str, p: Palette) -> Element<'static, Message> {
    let trimmed = line.trim_start();
    let indent = " ".repeat(line.len() - trimmed.len());
    let mono = iced::Font::MONOSPACE;
    let mut parts: Vec<Element<'static, Message>> = vec![text(indent).size(12.5).font(mono).into()];
    if let Some(rest) = trimmed.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            let key = rest[..end].to_string();
            let after = rest[end + 1..].trim_start_matches(':').trim();
            parts.push(
                text(format!("\"{key}\""))
                    .size(12.5)
                    .font(mono)
                    .color(p.teal)
                    .into(),
            );
            parts.push(text(": ").size(12.5).font(mono).color(p.muted).into());
            parts.push(json_value(after.trim_end_matches(','), p));
            if after.ends_with(',') {
                parts.push(text(",").size(12.5).font(mono).color(p.muted).into());
            }
            return row(parts).spacing(0).into();
        }
    }
    parts.push(
        text(trimmed.to_string())
            .size(12.5)
            .font(mono)
            .color(p.text)
            .into(),
    );
    row(parts).spacing(0).into()
}

fn json_value(value: &str, p: Palette) -> Element<'static, Message> {
    let color = if value.starts_with('"') {
        if p.dark {
            style::rgb(0xfd, 0xba, 0x74)
        } else {
            style::rgb(0xbe, 0x12, 0x3c)
        }
    } else if value.starts_with(|ch: char| ch.is_ascii_digit() || ch == '-') {
        if p.dark {
            style::rgb(0xfd, 0xe6, 0x8a)
        } else {
            style::rgb(0xb4, 0x53, 0x09)
        }
    } else if matches!(value, "true" | "false" | "null") {
        if p.dark {
            style::rgb(0x7d, 0xd3, 0xfc)
        } else {
            style::rgb(0x03, 0x69, 0xa1)
        }
    } else {
        p.text
    };
    text(value.to_string())
        .size(12.5)
        .font(iced::Font::MONOSPACE)
        .color(color)
        .into()
}

fn schema_dialog(app: &App, kind: &super::SchemaKind, p: Palette) -> Element<'static, Message> {
    let (title, columns, items) = match kind {
        super::SchemaKind::Result(index) => app
            .results
            .get(*index)
            .map(|card| {
                (
                    table_name(&card.statement).unwrap_or_else(|| "Result".to_string()),
                    card.columns.clone(),
                    card.items.clone(),
                )
            })
            .unwrap_or_else(|| ("Result".to_string(), Vec::new(), Vec::new())),
        super::SchemaKind::Table => (
            if app.open_table.is_empty() {
                "Table".to_string()
            } else {
                app.open_table.clone()
            },
            app.table_columns.clone(),
            app.table_items.clone(),
        ),
    };
    let fields = schema_fields(&columns, &items);
    let summary = format!(
        "{} column{}",
        fields.len(),
        if fields.len() == 1 { "" } else { "s" }
    );
    let head = row![
        column![
            text("STRUCTURE").size(11).color(p.teal).font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..iced::Font::default()
            }),
            text(title).size(16).font(iced::Font {
                weight: iced::font::Weight::Bold,
                ..iced::Font::default()
            }),
            text(summary).size(12).color(p.muted),
        ]
        .spacing(2),
        iced::widget::Space::new().width(Fill),
        button(icon_only(Mark::Close, p.muted, 12.0))
            .on_press(Message::CloseSchema)
            .padding(0)
            .width(28)
            .height(28)
            .style(style::paint(p, style::Look::Quiet)),
    ]
    .align_y(iced::alignment::Vertical::Center);
    dialog_shell(
        "Structure",
        head.into(),
        schema_table(&fields, items.len(), p),
        p,
    )
}

struct SchemaField {
    name: String,
    kind: &'static str,
    role: String,
    sample: String,
    present: usize,
}

fn schema_fields(
    columns: &[crate::web::shape::Column],
    items: &[serde_json::Value],
) -> Vec<SchemaField> {
    ordered_columns(columns, items)
        .into_iter()
        .map(|column| {
            let mut kind = "NULL";
            let mut sample = String::new();
            let mut present = 0usize;
            for item in items {
                let Some(value) = item.get(&column.name) else {
                    continue;
                };
                if value.is_null() {
                    continue;
                }
                present += 1;
                if sample.is_empty() {
                    kind = value_kind(value);
                    sample = cell_text(Some(value));
                    if sample.chars().count() > 42 {
                        sample = sample.chars().take(42).collect::<String>() + "…";
                    }
                }
            }
            let hash =
                column.table.as_deref() == Some("hash") || column.index.as_deref() == Some("hash");
            let range = column.table.as_deref() == Some("range")
                || column.index.as_deref() == Some("range");
            let role = match (hash, range) {
                (true, true) => "hash + range",
                (true, false) => "hash",
                (false, true) => "range",
                (false, false) => "",
            };
            SchemaField {
                name: column.name,
                kind,
                role: role.to_string(),
                sample,
                present,
            }
        })
        .collect()
}

fn value_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "NULL",
        serde_json::Value::Bool(_) => "BOOL",
        serde_json::Value::Number(_) => "NUMBER",
        serde_json::Value::String(_) => "STRING",
        serde_json::Value::Array(_) => "LIST",
        serde_json::Value::Object(_) => "MAP",
    }
}

fn schema_table(fields: &[SchemaField], total: usize, p: Palette) -> Element<'static, Message> {
    let header = |label: &'static str, width: f32| -> Element<'static, Message> {
        container(text(label).size(11).color(p.muted))
            .width(Length::Fixed(width))
            .padding([6, 10])
            .into()
    };
    let mut rows = column![container(
        row![
            header("NAME", 160.0),
            header("TYPE", 90.0),
            header("ROLE", 110.0),
            header("PRESENT", 90.0),
            header("SAMPLE", 220.0),
        ]
        .spacing(0),
    )
    .style(style::fill(p.canvas))]
    .spacing(0)
    .width(Length::Fixed(670.0));
    for field in fields {
        let share = if total == 0 {
            0.0
        } else {
            field.present as f32 / total as f32
        };
        rows = rows.push(hairline(p.line));
        rows = rows.push(
            row![
                container(
                    text(field.name.clone())
                        .size(12.5)
                        .font(iced::Font::MONOSPACE)
                        .color(p.text)
                )
                .width(Length::Fixed(160.0))
                .padding([8, 10]),
                container(text(field.kind).size(11).color(kind_color(field.kind, p)))
                    .width(Length::Fixed(90.0))
                    .padding([8, 10]),
                container(text(field.role.clone()).size(12).color(p.muted))
                    .width(Length::Fixed(110.0))
                    .padding([8, 10]),
                container(
                    row![
                        text(field.present.to_string()).size(11).color(p.muted),
                        container(text(""))
                            .width(Length::Fixed((52.0 * share).max(if share > 0.0 {
                                2.0
                            } else {
                                0.0
                            })))
                            .height(4)
                            .style(style::fill(p.teal)),
                    ]
                    .spacing(6)
                    .align_y(iced::alignment::Vertical::Center),
                )
                .width(Length::Fixed(90.0))
                .padding([8, 10]),
                container(
                    text(field.sample.clone())
                        .size(12)
                        .font(iced::Font::MONOSPACE)
                        .color(p.muted)
                )
                .width(Length::Fixed(220.0))
                .padding([8, 10]),
            ]
            .spacing(0),
        );
    }
    rows.into()
}

fn kind_color(kind: &str, p: Palette) -> Color {
    match kind {
        "STRING" => p.teal,
        "NUMBER" => p.amber,
        "BOOL" => p.blue,
        "LIST" => style::rgb(0x7c, 0x3a, 0xed),
        "MAP" => style::rgb(0xc0, 0x26, 0xd3),
        _ => p.muted,
    }
}

fn dialog_shell(
    _name: &str,
    head: Element<'static, Message>,
    body: Element<'static, Message>,
    p: Palette,
) -> Element<'static, Message> {
    container(
        column![
            container(head).width(Fill).padding([14, 16]),
            separator(p),
            container(scrollable(body).height(Length::Fixed(420.0)))
                .width(Fill)
                .padding([12, 16]),
        ]
        .spacing(0)
        .width(Length::Fixed(760.0)),
    )
    .style(style::pop_card(p))
    .into()
}

fn separator(p: Palette) -> Element<'static, Message> {
    rule::horizontal(1)
        .style(move |_theme| rule::Style {
            color: p.line,
            radius: 0.0.into(),
            fill_mode: rule::FillMode::Full,
            snap: true,
        })
        .into()
}

fn tip<'a>(
    target: impl Into<Element<'a, Message>>,
    label: &'static str,
    p: Palette,
) -> Element<'a, Message> {
    tooltip(
        target,
        container(text(label).size(12).color(p.text))
            .padding([4, 8])
            .style(style::menu_card(p)),
        tooltip::Position::Bottom,
    )
    .delay(Duration::from_millis(400))
    .into()
}

fn floating_menu(app: &App) -> Option<Element<'_, Message>> {
    let menu = app.menu.as_ref()?;
    if menu.kind == "root" {
        return None;
    }
    let p = style::palette(app.prefs.dark);
    let card = context_menu(&menu.kind, p);
    let at = app.pointer;
    Some(
        float(card)
            .translate(move |bounds, viewport| place(at, bounds, viewport))
            .into(),
    )
}

fn floating_search(app: &App) -> Element<'_, Message> {
    let p = style::palette(app.prefs.dark);
    let card = search_list(app, p);
    float(card)
        .translate(|bounds, viewport| place(Point::new(220.0, 42.0), bounds, viewport))
        .into()
}

fn notices(app: &App) -> Element<'_, Message> {
    let p = style::palette(app.prefs.dark);
    let mut cards = column![].spacing(8);
    for notice in &app.notices {
        let tone = match notice.title.as_str() {
            "Conflict" | "Run failed" => p.danger,
            "Saved" | "Created" | "Duplicated" => p.ok,
            _ => p.text,
        };
        cards = cards.push(
            container(
                column![
                    text(notice.title.clone()).size(12).color(tone),
                    text(notice.body.clone()).size(12).color(p.text),
                ]
                .spacing(2),
            )
            .width(Length::Fixed(280.0))
            .padding(10)
            .style(style::menu_card(p)),
        );
    }
    float(cards)
        .translate(|bounds, viewport| {
            Vector::new(
                viewport.width - bounds.width - 16.0 - bounds.x,
                viewport.height - bounds.height - 28.0 - bounds.y,
            )
        })
        .into()
}

fn place(at: Point, bounds: Rectangle, viewport: Rectangle) -> Vector {
    let mut x = at.x - bounds.x;
    let mut y = at.y - bounds.y;
    let right = bounds.x + x + bounds.width;
    let bottom = bounds.y + y + bounds.height;
    if right > viewport.width - 8.0 {
        x -= right - (viewport.width - 8.0);
    }
    if bottom > viewport.height - 8.0 {
        y -= bottom - (viewport.height - 8.0);
    }
    if bounds.x + x < 8.0 {
        x = 8.0 - bounds.x;
    }
    if bounds.y + y < 8.0 {
        y = 8.0 - bounds.y;
    }
    Vector::new(x, y)
}

fn name_key(name: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in name.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash | 0xff00
}

fn editor_keys(press: text_editor::KeyPress) -> Option<text_editor::Binding<Message>> {
    let focused = matches!(press.status, text_editor::Status::Focused { .. });
    let command = press.modifiers.command() || press.modifiers.control();
    if focused && command {
        if press.key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) {
            return Some(text_editor::Binding::Custom(Message::RunCursor));
        }
        if let iced::keyboard::Key::Character(ch) = &press.key {
            if ch.as_str() == "\\" || ch.as_str() == "|" {
                return Some(text_editor::Binding::Custom(Message::RunCursor));
            }
            if ch.as_str().eq_ignore_ascii_case("s") {
                return Some(text_editor::Binding::Custom(Message::Save));
            }
        }
    }
    text_editor::Binding::from_key_press(press)
}
