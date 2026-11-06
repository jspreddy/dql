//! Layout for the native window. It follows the web UI: header, file tree,
//! editor, results, and a table browser.

use std::time::Duration;

use iced::widget::{
    button, checkbox, column, container, float, keyed, mouse_area, opaque, pane_grid, progress_bar,
    row, rule, scrollable, stack, text, text_editor, text_input, tooltip,
};
use iced::{Color, Element, Fill, Length, Point, Rectangle, Vector};

use crate::web::files::Node;
use crate::web::studio::{menu_labels, StatementCard};

use super::highlight;
use super::icons::{self, Mark};
use super::query;
use super::slot::row_slot;
use super::style::{self, Palette};
use super::{caret_bytes, App, Dialog, Message, Mode};

pub fn view(app: &App) -> Element<'_, Message> {
    let colors = style::palette(app.prefs.dark);
    let body = match app.mode {
        Mode::Query => query(app),
        Mode::Tables => tables(app, colors),
    };
    let window = column![header(app), body, status_bar(app)].spacing(0);
    // The root stays a stack. Column and Stack are both stateless, so swapping
    // the root when a menu opens makes iced reuse the file tree's empty state.
    let mut layers: Vec<Element<'_, Message>> = vec![window.into()];
    if let Some(menu) = floating_menu(app) {
        layers.push(menu);
    }
    if app.search_open {
        layers.push(floating_search(app));
    }
    if let Some(layer) = popup(app) {
        layers.push(layer);
    }
    if app.show_settings {
        layers.push(settings_layer(app));
    }
    if !app.notices.is_empty() {
        layers.push(notices(app));
    }
    stack(layers).width(Fill).height(Fill).into()
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
    let search = report_bounds(
        text_input("Search tables, columns, queries", &app.search)
            .on_input(Message::Search)
            .on_submit(Message::SearchSubmit)
            .size(13)
            .padding([2, 10])
            .width(Fill)
            .style(style::header_field(p)),
        Message::SearchBounds,
    );
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
            settings,
            theme,
        ]
        .spacing(10)
        .align_y(iced::alignment::Vertical::Center)
        .padding([0, 10])
        .height(Length::Fixed(36.0)),
    )
    .width(Fill)
    .style(style::fill(p.bar));
    let mut col = column![bar].spacing(0);
    if app.show_env {
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
    container(body)
        .width(Fill)
        .style(style::fill(p.canvas))
        .into()
}

fn search_list(app: &App, p: Palette, width: f32) -> Element<'_, Message> {
    let groups = [
        ("Tables", &["table"][..]),
        ("Columns", &["column", "index"][..]),
        ("Queries", &["query"][..]),
    ];
    let mut list = column![].spacing(0);
    let mut visual = 0usize;
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
        list = list.push(search_group(label, p));
        for (index, hit) in rows {
            let selected = app.search_active == Some(visual);
            visual += 1;
            let title = container(
                text(hit.title.clone())
                    .size(13)
                    .color(p.text)
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .width(Fill)
            .clip(true);
            let mut lines = column![row![
                text(search_kind(&hit.kind))
                    .size(11)
                    .color(p.muted)
                    .width(Length::Fixed(58.0)),
                title,
            ]
            .spacing(8)
            .align_y(iced::alignment::Vertical::Center)]
            .spacing(0)
            .width(Fill);
            if !hit.detail.is_empty() {
                lines = lines.push(
                    row![
                        text("").width(Length::Fixed(58.0)),
                        container(
                            text(hit.detail.clone())
                                .size(12)
                                .color(p.muted)
                                .wrapping(iced::widget::text::Wrapping::None),
                        )
                        .width(Fill)
                        .clip(true),
                    ]
                    .spacing(8),
                );
            }
            list = list.push(
                button(lines)
                    .on_press(Message::PickHit(index))
                    .width(Fill)
                    .padding([5, 6])
                    .style(style::search_row(p, selected)),
            );
        }
    }
    if visual == 0 {
        list = list.push(
            container(text("No matches").size(12).color(p.muted)).padding(iced::Padding {
                top: 8.0,
                right: 6.0,
                bottom: 8.0,
                left: 6.0,
            }),
        );
    }
    let content = search_content_height(&app.hits);
    let max_card = search_max_card(app.window_height);
    let body: Element<'_, Message> = if content + 12.0 > max_card {
        scrollable(list)
            .height(Length::Fixed((max_card - 12.0).max(1.0)))
            .width(Fill)
            .into()
    } else {
        list.into()
    };
    container(body)
        .width(Length::Fixed(width))
        .padding(6)
        .style(style::settings_pop(p))
        .into()
}

fn search_group(label: &str, p: Palette) -> Element<'_, Message> {
    container(
        text(label.to_ascii_uppercase())
            .size(11)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..iced::Font::default()
            })
            .color(p.muted),
    )
    .padding(iced::Padding {
        top: 6.0,
        right: 4.0,
        bottom: 2.0,
        left: 4.0,
    })
    .into()
}

pub(super) fn search_popup_rect(
    field: Rectangle,
    hits: &[crate::web::search::SearchHit],
    window_h: f32,
) -> Rectangle {
    let height = search_card_height(hits, window_h);
    Rectangle {
        x: field.x,
        y: field.y + field.height + 6.0,
        width: field.width,
        height,
    }
}

fn search_content_height(hits: &[crate::web::search::SearchHit]) -> f32 {
    let groups = [&["table"][..], &["column", "index"][..], &["query"][..]];
    let mut content = 0.0_f32;
    let mut rows = 0usize;
    for kinds in groups {
        let matched: Vec<_> = hits
            .iter()
            .filter(|hit| kinds.contains(&hit.kind.as_str()))
            .collect();
        if matched.is_empty() {
            continue;
        }
        content += 22.0;
        for hit in matched {
            content += if hit.detail.is_empty() { 28.0 } else { 42.0 };
            rows += 1;
        }
    }
    if rows == 0 {
        32.0
    } else {
        content
    }
}

fn search_max_card(window_h: f32) -> f32 {
    (window_h * 0.7).clamp(160.0, 420.0)
}

fn search_card_height(hits: &[crate::web::search::SearchHit], window_h: f32) -> f32 {
    (search_content_height(hits) + 12.0)
        .min(search_max_card(window_h))
        .max(28.0)
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

fn settings_layer(app: &App) -> Element<'_, Message> {
    let p = style::palette(app.prefs.dark);
    let scrim = opaque(
        mouse_area(container(text("")).width(Fill).height(Fill)).on_press(Message::ToggleSettings),
    );
    let width = 232.0;
    let left = (app.window_width - 46.0 - width).max(8.0);
    let top = 35.0;
    let card = float(settings_card(app, p))
        .translate(move |bounds, _viewport| Vector::new(left - bounds.x, top - bounds.y));
    stack![scrim, card].into()
}

fn settings_card(app: &App, p: Palette) -> Element<'_, Message> {
    let age = settings_cache_age(app.cache.saved_at_ms, now_ms());
    let detail = if app.cache.text.is_empty() {
        "No table list cached.".to_string()
    } else {
        app.cache.text.clone()
    };
    let age_color = if app.cache.refreshing {
        if p.dark {
            style::rgb(0x9a, 0xab, 0xbd)
        } else {
            style::rgb(0x52, 0x60, 0x71)
        }
    } else {
        p.muted
    };
    let mut body = column![
        section_label("Highlight", p),
        container(setting_check(
            "Minimal write highlight",
            app.prefs.minimal_write,
            Message::SetMinimalWrite,
            p,
        ))
        .padding([4, 4]),
        container(setting_check(
            "Even/odd highlight",
            app.prefs.even_odd,
            Message::SetEvenOdd,
            p,
        ))
        .padding([4, 4]),
        settings_rule(p),
        row![
            section_label("Table cache", p),
            iced::widget::Space::new().width(Fill),
            tooltip(
                text(age).size(12).color(age_color),
                container(text(detail).size(12).color(p.text).width(220))
                    .padding([4, 8])
                    .style(style::menu_card(p)),
                tooltip::Position::Bottom,
            ),
        ]
        .align_y(iced::alignment::Vertical::Center),
        button(button_label("Clear table cache", 13.0))
            .on_press(Message::ClearCache)
            .width(Fill)
            .height(26)
            .style(style::cache_clear(p)),
    ]
    .spacing(0);
    if !app.cache_note.is_empty() {
        body = body.push(
            container(text(app.cache_note.clone()).size(12).color(p.muted)).padding(
                iced::Padding {
                    top: 6.0,
                    right: 4.0,
                    bottom: 0.0,
                    left: 4.0,
                },
            ),
        );
    }
    body = body.push(settings_rule(p));
    body = body.push(section_label("Keyboard", p));
    body = body.push(shortcut_row(
        "Run selection",
        column![
            text("Ctrl + Enter").size(12).color(p.muted),
            text("Ctrl + \\").size(12).color(p.muted),
        ]
        .spacing(2)
        .align_x(iced::alignment::Horizontal::Right)
        .into(),
        p,
    ));
    body = body.push(shortcut_row(
        "Stop query",
        text("Esc").size(12).color(p.muted).into(),
        p,
    ));
    container(body)
        .width(Length::Fixed(232.0))
        .padding(8)
        .style(style::settings_pop(p))
        .into()
}

fn section_label(label: &str, p: Palette) -> Element<'_, Message> {
    container(
        text(label.to_ascii_uppercase())
            .size(11)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..iced::Font::default()
            })
            .color(p.muted),
    )
    .padding(iced::Padding {
        top: 2.0,
        right: 4.0,
        bottom: 6.0,
        left: 4.0,
    })
    .into()
}

fn setting_check<'a>(
    label: &'a str,
    checked: bool,
    toggle: fn(bool) -> Message,
    p: Palette,
) -> Element<'a, Message> {
    checkbox(checked)
        .label(label)
        .size(14)
        .spacing(8)
        .text_size(13)
        .on_toggle(toggle)
        .style(style::setting_check(p))
        .into()
}

fn settings_rule(p: Palette) -> Element<'static, Message> {
    container(separator(p)).padding([8, 4]).into()
}

fn shortcut_row<'a>(
    action: &'a str,
    keys: Element<'a, Message>,
    p: Palette,
) -> Element<'a, Message> {
    row![
        text(action).size(12).color(p.text),
        iced::widget::Space::new().width(Fill),
        keys,
    ]
    .align_y(iced::alignment::Vertical::Center)
    .padding(iced::Padding {
        top: 0.0,
        right: 4.0,
        bottom: 6.0,
        left: 4.0,
    })
    .into()
}

fn settings_cache_age(saved_at_ms: Option<u64>, now_ms: u64) -> String {
    match saved_at_ms {
        Some(saved) => format!("({})", cache_age_phrase(saved, now_ms)),
        None => "not cached".to_string(),
    }
}

fn cache_age_phrase(saved_at_ms: u64, now_ms: u64) -> String {
    let elapsed = now_ms.saturating_sub(saved_at_ms) / 1000;
    if elapsed < 5 {
        "just now".to_string()
    } else if elapsed < 60 {
        format!("{elapsed}s ago")
    } else if elapsed < 3600 {
        format!("{} min ago", elapsed / 60)
    } else if elapsed < 86_400 {
        format!("{} hr ago", elapsed / 3600)
    } else {
        let days = elapsed / 86_400;
        if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{days} days ago")
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
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
    // Each entry keeps its own state. The header, the Files menu, and every
    // node are stateless columns, and iced will reuse that empty state when
    // a click inserts or removes one of them.
    let mut tree = column![row_slot(head)].spacing(0);
    if app.menu.as_ref().is_some_and(|menu| menu.kind == "root") {
        tree = tree.push(row_slot(context_menu("root", p)));
    }
    if let Some(root) = &app.tree {
        if let Some(children) = &root.children {
            for child in children {
                tree = tree.push(row_slot(node_view(app, child, 0, p)));
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
    let paints = query::line_paint(&source, app.prefs.minimal_write, app.prefs.even_odd);
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
    for (index, _line) in lines.iter().enumerate() {
        let number = index + 1;
        let issue = app.lint.iter().find(|issue| issue.line == number);
        let mark = if issue.is_some() { "●" } else { "" };
        let numbers: Element<'_, Message> = row![
            text(mark)
                .size(8)
                .color(if issue.is_some_and(|issue| issue.severity == "error") {
                    p.danger
                } else {
                    p.amber
                }),
            text(number.to_string()).size(11).color(p.muted),
        ]
        .spacing(2)
        .align_y(iced::alignment::Vertical::Center)
        .padding([0.0, 4.0])
        .width(Fill)
        .height(Fill)
        .into();
        let paint = paints
            .get(index)
            .copied()
            .unwrap_or(query::BandPaint::Plain);
        let inner = if let Some(color) = gutter_bar(paint, p.dark) {
            stack![
                container(iced::widget::Space::new())
                    .width(Length::Fixed(3.0))
                    .height(Fill)
                    .style(style::fill(color)),
                numbers,
            ]
            .into()
        } else {
            numbers
        };
        gutter = gutter.push(
            container(inner)
                .height(Length::Fixed(query::LINE_H))
                .width(Fill),
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
    document = document.push_under(band_layer(&paints, p.dark));
    let scroller = scrollable(document)
        .id(app.editor_scroll.clone())
        .on_scroll(Message::EditorScrolled)
        .width(Fill)
        .height(Fill);
    let mut editor_stack = iced::widget::Stack::new()
        .width(Fill)
        .height(Fill)
        .push(scroller);
    // The run button is a float, and floats share one overlay index, so it
    // paints over a later stack layer. Hide it while a card covers the editor.
    if !covers_editor(app) {
        if let Some(button) = run_button(app, p, &source) {
            editor_stack = editor_stack.push(button);
        }
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

pub(super) fn covers_editor(app: &App) -> bool {
    app.show_settings || app.search_open || app.schema.is_some() || app.row_json.is_some()
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

fn band_layer(paints: &[query::BandPaint], dark: bool) -> Element<'static, Message> {
    let mut rows = column![].spacing(0).width(Fill);
    for paint in paints {
        rows = rows.push(
            container(iced::widget::Space::new())
                .height(Length::Fixed(query::LINE_H))
                .width(Fill)
                .style(style::fill(
                    content_band(*paint, dark).unwrap_or(Color::TRANSPARENT),
                )),
        );
    }
    container(rows)
        .width(Fill)
        .padding(iced::Padding {
            top: query::PAD_TOP,
            right: 0.0,
            bottom: 0.0,
            left: query::GUTTER_W,
        })
        .into()
}

fn content_band(paint: query::BandPaint, dark: bool) -> Option<Color> {
    let color = match paint {
        query::BandPaint::Alt => {
            if dark {
                (0x1e, 0x29, 0x36)
            } else {
                (0xf4, 0xf7, 0xf8)
            }
        }
        query::BandPaint::Write => {
            if dark {
                (0x3a, 0x1d, 0x24)
            } else {
                (0xfd, 0xec, 0xec)
            }
        }
        query::BandPaint::WriteEven => {
            if dark {
                (0x5a, 0x28, 0x32)
            } else {
                (0xf3, 0xc4, 0xc4)
            }
        }
        query::BandPaint::Plain | query::BandPaint::Bar | query::BandPaint::BarEven => return None,
    };
    Some(style::rgb(color.0, color.1, color.2))
}

fn gutter_bar(paint: query::BandPaint, dark: bool) -> Option<Color> {
    match paint {
        query::BandPaint::Bar => Some(style::rgb(0xfb, 0x71, 0x85)),
        query::BandPaint::BarEven => Some(if dark {
            style::rgb(0xfd, 0xa4, 0xaf)
        } else {
            style::rgb(0xbe, 0x12, 0x3c)
        }),
        _ => None,
    }
}

fn results(app: &App, p: Palette) -> Element<'_, Message> {
    let mut cards = keyed::Column::new().spacing(8).padding(8).width(Fill);
    for (index, card) in app.results.iter().take(20).enumerate() {
        cards = cards.push(index, row_slot(card_view(app, index, card, p)));
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
            "Show result structure",
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
        "Show row JSON",
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
    list = list.push(0, row_slot(head));
    list = list.push(
        1,
        row_slot(
            text(if app.cache_text.is_empty() {
                app.table_note.clone()
            } else {
                format!("{}  {}", app.table_note, app.cache_text)
            })
            .size(11)
            .color(p.muted),
        ),
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
            row_slot(
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
            ),
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
                "Show table structure",
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
    // A leading comment is part of the statement. Strip each comment the way
    // the web title does, then read the table after FROM.
    let text: String = statement
        .lines()
        .map(|line| line.split("--").next().unwrap_or(line))
        .collect::<Vec<_>>()
        .join(" ");
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
    if app.row_json.is_some() {
        return Some(opaque(row_sheet(app, p)));
    }
    app.schema
        .as_ref()
        .map(|kind| opaque(schema_sheet(app, kind, p)))
}

fn sheet(body: Element<'static, Message>, p: Palette) -> Element<'static, Message> {
    container(body)
        .width(Fill)
        .height(Fill)
        .style(move |_| {
            iced::widget::container::Style::default()
                .background(p.paper)
                .color(p.text)
                .border(iced::Border {
                    color: p.line,
                    width: 1.0,
                    radius: 0.0.into(),
                })
        })
        .into()
}

fn popup_head(
    body: Element<'static, Message>,
    close: Element<'static, Message>,
    p: Palette,
) -> Element<'static, Message> {
    let ink = popup_ink(p.dark);
    container(
        row![body, close]
            .spacing(12)
            .align_y(iced::alignment::Vertical::Top),
    )
    .width(Fill)
    .padding(iced::Padding {
        top: 16.0,
        right: 16.0,
        bottom: 12.0,
        left: 20.0,
    })
    .style(style::fill(ink.card_a))
    .into()
}

fn close_button(message: Message, color: Color, hover: Color) -> Element<'static, Message> {
    button(icon_only(Mark::Close, color, 12.0))
        .on_press(message)
        .padding(0)
        .width(28)
        .height(28)
        .style(move |_theme, status| {
            let background = matches!(status, button::Status::Hovered | button::Status::Pressed)
                .then_some(iced::Background::Color(hover));
            button::Style {
                background,
                text_color: color,
                border: iced::Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 8.0.into(),
                },
                shadow: iced::Shadow::default(),
                snap: true,
            }
        })
        .into()
}

fn kicker(label: &'static str, p: Palette) -> Element<'static, Message> {
    text(label)
        .size(11)
        .color(popup_ink(p.dark).teal_ink)
        .font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..iced::Font::default()
        })
        .into()
}

fn row_sheet(app: &App, p: Palette) -> Element<'static, Message> {
    let json = app.row_json.clone().unwrap_or_default();
    let (title, chips, shown) = row_view(app, &json);
    let ink = popup_ink(p.dark);
    let mut heading = column![kicker("ROW", p), title_text(&title, ink.text_strong)].spacing(4);
    if !chips.is_empty() {
        let mut line = row![].spacing(6);
        for (column, value) in chips {
            line = line.push(row_key_chip(&column, &value, p));
        }
        heading = heading.push(container(line).padding(iced::Padding {
            top: 8.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        }));
    }
    let head = popup_head(
        heading.width(Fill).into(),
        close_button(Message::CloseRow, ink.label_2, ink.hover_quiet),
        p,
    );
    let body = container(scrollable(json_block(&shown, p)).height(Fill).width(Fill))
        .width(Fill)
        .height(Fill)
        .padding(iced::Padding {
            top: 14.0,
            right: 16.0,
            bottom: 16.0,
            left: 16.0,
        });
    sheet(
        column![head, hairline(ink.line), body]
            .spacing(0)
            .width(Fill)
            .height(Fill)
            .into(),
        p,
    )
}

fn title_text(title: &str, color: Color) -> Element<'static, Message> {
    text(title.to_string())
        .size(16)
        .line_height(1.3)
        .color(color)
        .font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..iced::Font::default()
        })
        .into()
}

fn row_view(app: &App, json: &str) -> (String, Vec<(crate::web::shape::Column, String)>, String) {
    let mut sources: Vec<(&[serde_json::Value], &[crate::web::shape::Column])> =
        vec![(&app.table_items, &app.table_columns)];
    for card in &app.results {
        sources.push((&card.items, &card.columns));
    }
    for (items, columns) in sources {
        for item in items {
            if serde_json::to_string_pretty(item).ok().as_deref() != Some(json) {
                continue;
            }
            let ordered = ordered_columns(columns, items);
            let title_column = row_title_column(&ordered, item);
            let title = title_column.map_or_else(
                || "Row".to_string(),
                |column| format!("{} = {}", column.name, item_string(item, &column.name)),
            );
            let chips = row_chips(
                &ordered,
                item,
                title_column.map(|column| column.name.as_str()),
            );
            return (title, chips, ordered_json(item, &ordered));
        }
    }
    ("Row".to_string(), Vec::new(), json.to_string())
}

fn row_title_column<'a>(
    columns: &'a [crate::web::shape::Column],
    item: &'a serde_json::Value,
) -> Option<&'a crate::web::shape::Column> {
    let hash = columns
        .iter()
        .find(|column| column.table.as_deref() == Some("hash"));
    let mut preferred = Vec::new();
    if let Some(column) = hash {
        preferred.push(column);
    }
    preferred.extend(columns);
    preferred.into_iter().find(|column| {
        matches!(item.get(&column.name), Some(serde_json::Value::String(value)) if !value.is_empty() && value.chars().count() <= 48)
    })
}

fn item_string(item: &serde_json::Value, name: &str) -> String {
    match item.get(name) {
        Some(serde_json::Value::String(value)) => value.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn row_chips(
    columns: &[crate::web::shape::Column],
    item: &serde_json::Value,
    title: Option<&str>,
) -> Vec<(crate::web::shape::Column, String)> {
    let Some(map) = item.as_object() else {
        return Vec::new();
    };
    columns
        .iter()
        .filter(|column| title != Some(column.name.as_str()))
        .filter(|column| is_key_column(column))
        .filter_map(|column| {
            map.get(&column.name)
                .map(|value| (column.clone(), chip_value(value)))
        })
        .collect()
}

fn is_key_column(column: &crate::web::shape::Column) -> bool {
    matches!(column.table.as_deref(), Some("hash" | "range"))
        || matches!(column.index.as_deref(), Some("hash" | "range"))
}

fn chip_value(value: &serde_json::Value) -> String {
    if value.is_null() {
        return "null".to_string();
    }
    let text = match value {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    let count = text.chars().count();
    if count > 48 {
        text.chars().take(47).collect::<String>() + "…"
    } else {
        text
    }
}

fn ordered_json(item: &serde_json::Value, columns: &[crate::web::shape::Column]) -> String {
    let Some(map) = item.as_object() else {
        return serde_json::to_string_pretty(item).unwrap_or_default();
    };
    // `serde_json::Map` sorts keys. The web popup keeps table column order,
    // then any leftover fields, so the text is built by hand.
    let mut names = Vec::new();
    for column in columns {
        if map.contains_key(&column.name) && !names.contains(&column.name) {
            names.push(column.name.clone());
        }
    }
    for key in map.keys() {
        if !names.contains(key) {
            names.push(key.clone());
        }
    }
    let mut lines = vec!["{".to_string()];
    for (index, name) in names.iter().enumerate() {
        let key = serde_json::to_string(name).unwrap_or_else(|_| "\"\"".to_string());
        let pretty =
            serde_json::to_string_pretty(&map[name]).unwrap_or_else(|_| "null".to_string());
        let comma = if index + 1 == names.len() { "" } else { "," };
        lines.push(format!("  {key}: {}{comma}", indent_json(&pretty)));
    }
    lines.push("}".to_string());
    lines.join("\n")
}

fn indent_json(pretty: &str) -> String {
    let mut lines = pretty.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    let mut out = first.to_string();
    for line in lines {
        out.push('\n');
        out.push_str("  ");
        out.push_str(line);
    }
    out
}

fn row_key_chip(
    column: &crate::web::shape::Column,
    value: &str,
    p: Palette,
) -> Element<'static, Message> {
    let paint = chip_paint(p, row_chip_role(column));
    let mut line = row![].spacing(5).align_y(iced::alignment::Vertical::Center);
    if column_flag(column, "hash") {
        line = line.push(icons::mark(Mark::Hash, paint.mark, 11.0));
    }
    if column_flag(column, "range") {
        line = line.push(icons::mark(Mark::Range, paint.mark, 11.0));
    }
    line = line.push(
        text(format!("{} = {value}", column.name))
            .size(12)
            .color(paint.mark)
            .font(semibold()),
    );
    container(line)
        .padding(iced::Padding {
            top: 3.0,
            right: 8.0,
            bottom: 3.0,
            left: 6.0,
        })
        .style(style::chip(paint.head, paint.mark))
        .into()
}

fn row_chip_role(column: &crate::web::shape::Column) -> &'static str {
    if column.table.is_some() && column.index.is_some() {
        "both"
    } else if column.table.as_deref() == Some("range") {
        "range"
    } else if column.table.as_deref() == Some("hash") {
        "hash"
    } else if column.index.is_some() {
        "index"
    } else {
        "hash"
    }
}

fn column_flag(column: &crate::web::shape::Column, role: &str) -> bool {
    column.table.as_deref() == Some(role) || column.index.as_deref() == Some(role)
}

fn chip_paint(p: Palette, role: &str) -> style::ColumnPaint {
    match role {
        "both" => style::column_paint(p, Some("hash"), Some("range")),
        "range" | "index" => style::column_paint(p, None, Some("hash")),
        _ => style::column_paint(p, Some("hash"), None),
    }
}

fn schema_sheet(app: &App, kind: &super::SchemaKind, p: Palette) -> Element<'static, Message> {
    let (browser, title, statement, columns, items, keys) = match kind {
        super::SchemaKind::Result(index) => app.results.get(*index).map_or_else(
            || {
                (
                    false,
                    "Result".to_string(),
                    String::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
            },
            |card| {
                (
                    false,
                    table_name(&card.statement).unwrap_or_else(|| "Result".to_string()),
                    card.statement.clone(),
                    card.columns.clone(),
                    card.items.clone(),
                    Vec::new(),
                )
            },
        ),
        super::SchemaKind::Table => {
            let keys = app
                .tables
                .iter()
                .find(|table| table.name == app.open_table)
                .map(|table| table.keys.clone())
                .unwrap_or_default();
            (
                true,
                if app.open_table.is_empty() {
                    "Table".to_string()
                } else {
                    app.open_table.clone()
                },
                String::new(),
                app.table_columns.clone(),
                app.table_items.clone(),
                keys,
            )
        }
    };
    let described = schema_columns(&columns, &items, &keys);
    let fields = inspect_fields(&described, &items);
    let chips = schema_title_chips(&described, &keys);
    let summary = schema_summary(fields.len(), items.len(), browser);
    let ink = popup_ink(p.dark);
    let mut kicker_row = row![kicker(
        if browser {
            "TABLE STRUCTURE"
        } else {
            "RESULT STRUCTURE"
        },
        p,
    )]
    .spacing(5)
    .align_y(iced::alignment::Vertical::Center);
    if !browser {
        kicker_row = kicker_row.push(bar_tip(
            container(icons::mark(Mark::Info, ink.teal_ink, 14.0))
                .width(16)
                .height(16)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center),
            "Based on this query result, not the entire table.",
            p,
        ));
    }
    let mut title_row = row![title_text(&title, p.text)]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center);
    for (name, role) in &chips {
        title_row = title_row.push(schema_key_chip(name, role, p));
    }
    let mut heading = column![kicker_row, title_row].spacing(8).width(Fill);
    if !browser {
        if let Some(query) = query_block(&statement, p) {
            heading = heading.push(container(query).padding(iced::Padding {
                top: 10.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            }));
        }
    }
    heading = heading.push(container(text(summary).size(12.5).color(p.muted)).padding(
        iced::Padding {
            top: 8.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        },
    ));
    let head = popup_head(
        heading.into(),
        close_button(Message::CloseSchema, p.muted, ink.hover_quiet),
        p,
    );
    let table = schema_grid(&fields, items.len(), app.window_width, p);
    let foot = if browser {
        "Types come from this page and the keys already listed. No extra query was run."
    } else {
        "Types come from this result. No extra query was run."
    };
    sheet(
        column![
            head,
            hairline(p.line),
            container(table)
                .width(Fill)
                .height(Fill)
                .padding(iced::Padding {
                    top: 12.0,
                    right: 16.0,
                    bottom: 8.0,
                    left: 16.0,
                }),
            container(text(foot).size(11.5).color(p.muted))
                .width(Fill)
                .padding(iced::Padding {
                    top: 8.0,
                    right: 20.0,
                    bottom: 14.0,
                    left: 20.0,
                }),
        ]
        .spacing(0)
        .width(Fill)
        .height(Fill)
        .into(),
        p,
    )
}

fn schema_summary(columns: usize, rows: usize, table: bool) -> String {
    let columns = if columns == 1 {
        "1 column".to_string()
    } else {
        format!("{columns} columns")
    };
    let rows = if rows == 1 {
        "1 row".to_string()
    } else {
        format!("{rows} rows")
    };
    let page = if table { " on this page" } else { "" };
    format!("{columns} · {rows}{page}")
}

fn schema_columns(
    columns: &[crate::web::shape::Column],
    items: &[serde_json::Value],
    keys: &[crate::web::shape::KeyRole],
) -> Vec<crate::web::shape::Column> {
    let mut described = Vec::new();
    let mut seen = Vec::new();
    for column in columns {
        if column.name.is_empty() || seen.iter().any(|name| name == &column.name) {
            continue;
        }
        seen.push(column.name.clone());
        described.push(column.clone());
    }
    for key in keys {
        if key.name.is_empty() || seen.iter().any(|name| name == &key.name) {
            continue;
        }
        seen.push(key.name.clone());
        let table = matches!(key.role.as_str(), "hash" | "range").then(|| key.role.clone());
        described.push(crate::web::shape::Column {
            name: key.name.clone(),
            table,
            index: None,
        });
    }
    let mut present = Vec::new();
    for item in items {
        let Some(map) = item.as_object() else {
            continue;
        };
        for name in map.keys() {
            if !present.iter().any(|kept| kept == name) {
                present.push(name.clone());
            }
        }
    }
    let mut extra: Vec<_> = present
        .into_iter()
        .filter(|name| !seen.iter().any(|kept| kept == name))
        .collect();
    extra.sort();
    described.extend(extra.into_iter().map(|name| crate::web::shape::Column {
        name,
        table: None,
        index: None,
    }));
    described
}

fn schema_title_chips(
    columns: &[crate::web::shape::Column],
    keys: &[crate::web::shape::KeyRole],
) -> Vec<(String, String)> {
    let mut chips = Vec::new();
    for column in columns {
        if let Some(role) = column.table.as_deref() {
            if role == "hash" || role == "range" {
                chips.push((column.name.clone(), role.to_string()));
            }
        }
    }
    if chips.is_empty() {
        for key in keys {
            if key.role == "hash" || key.role == "range" {
                chips.push((key.name.clone(), key.role.clone()));
            }
        }
    }
    chips
}

struct SchemaField {
    name: String,
    kinds: Vec<&'static str>,
    roles: Vec<&'static str>,
    hash: bool,
    range: bool,
    sample: String,
    present: usize,
}

fn inspect_fields(
    columns: &[crate::web::shape::Column],
    items: &[serde_json::Value],
) -> Vec<SchemaField> {
    columns
        .iter()
        .map(|column| {
            let mut present = 0usize;
            let mut kinds = Vec::new();
            let mut sample = None;
            for item in items {
                let Some(map) = item.as_object() else {
                    continue;
                };
                let Some(value) = map.get(&column.name) else {
                    continue;
                };
                present += 1;
                if value.is_null() {
                    continue;
                }
                let kind = value_kind(value);
                if !kinds.contains(&kind) {
                    kinds.push(kind);
                }
                if sample.is_none() {
                    sample = Some(format_sample(value));
                }
            }
            if kinds.is_empty() && present > 0 {
                kinds.push("NULL");
            }
            SchemaField {
                name: column.name.clone(),
                kinds,
                roles: role_labels(column),
                hash: column_flag(column, "hash"),
                range: column_flag(column, "range"),
                sample: sample.unwrap_or_default(),
                present,
            }
        })
        .collect()
}

fn role_labels(column: &crate::web::shape::Column) -> Vec<&'static str> {
    let mut labels = Vec::new();
    if column.table.as_deref() == Some("hash") {
        labels.push("Hash key");
    }
    if column.table.as_deref() == Some("range") {
        labels.push("Range key");
    }
    if column.index.as_deref() == Some("hash") {
        labels.push("Index hash");
    }
    if column.index.as_deref() == Some("range") {
        labels.push("Index range");
    }
    labels
}

fn format_sample(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
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

fn schema_key_chip(name: &str, role: &str, p: Palette) -> Element<'static, Message> {
    let paint = chip_paint(p, role);
    let mark = if role == "range" {
        Mark::Range
    } else {
        Mark::Hash
    };
    container(
        row![
            icons::mark(mark, paint.mark, 11.0),
            text(name.to_string())
                .size(12)
                .color(paint.mark)
                .font(semibold()),
        ]
        .spacing(5)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(iced::Padding {
        top: 3.0,
        right: 8.0,
        bottom: 3.0,
        left: 6.0,
    })
    .style(style::chip(paint.head, paint.mark))
    .into()
}

fn schema_grid(
    fields: &[SchemaField],
    total: usize,
    window_width: f32,
    p: Palette,
) -> Element<'static, Message> {
    if fields.is_empty() {
        return container(text("No columns").size(12).color(p.muted))
            .padding([12, 4])
            .into();
    }
    let ink = popup_ink(p.dark);
    let widths = schema_widths(fields, total, window_width);
    let span = widths.name + widths.kind + widths.role + widths.present + widths.sample;
    let header = container(schema_head_row(&widths, ink.label, ink.schema_line))
        .width(Length::Fixed(span))
        .style(style::fill(p.canvas));
    let mut rows = column![].spacing(0).width(Length::Fixed(span));
    let last = fields.len() - 1;
    for (index, field) in fields.iter().enumerate() {
        rows = rows.push(schema_data_row(
            field,
            total,
            &widths,
            index != last,
            ink.schema_line,
            p,
        ));
    }
    let head = scrollable(header)
        .id(iced::widget::Id::from("schema-pop-head"))
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::hidden(),
        ))
        .width(Fill)
        .height(Length::Shrink);
    let body = scrollable(rows)
        .direction(scrollable::Direction::Both {
            vertical: scrollable::Scrollbar::default(),
            horizontal: scrollable::Scrollbar::default(),
        })
        .width(Fill)
        .height(Fill)
        .on_scroll(|viewport| {
            Message::SyncGrid("schema-pop-head".to_string(), viewport.absolute_offset().x)
        });
    column![head, body]
        .spacing(0)
        .width(Fill)
        .height(Fill)
        .into()
}

struct SchemaWidths {
    name: f32,
    kind: f32,
    role: f32,
    present: f32,
    sample: f32,
}

fn schema_widths(fields: &[SchemaField], total: usize, window_width: f32) -> SchemaWidths {
    let pad = 28.0;
    let name = fields
        .iter()
        .map(|field| mono_px(&field.name, 12.5))
        .fold(mono_px("COLUMN", 11.0), f32::max)
        + pad;
    let kind = fields
        .iter()
        .map(|field| kinds_px(&field.kinds))
        .fold(mono_px("TYPE", 11.0), f32::max)
        + pad;
    let role = fields
        .iter()
        .map(role_px)
        .fold(mono_px("ROLE", 11.0), f32::max)
        + pad;
    let present = fields
        .iter()
        .map(|field| present_px(field.present, total))
        .fold(mono_px("PRESENT", 11.0), f32::max)
        + pad;
    let mut sample = fields
        .iter()
        .map(|field| mono_px(&field.sample, 12.0))
        .fold(mono_px("SAMPLE", 11.0), f32::max)
        + pad;
    let room = (window_width - 32.0 - (name + kind + role + present)).max(80.0);
    if sample < room {
        sample = room;
    }
    SchemaWidths {
        name,
        kind,
        role,
        present,
        sample,
    }
}

fn mono_px(text: &str, size: f32) -> f32 {
    text.chars().count() as f32 * size * 0.62
}

fn kinds_px(kinds: &[&str]) -> f32 {
    if kinds.is_empty() {
        return mono_px("—", 10.5) + 16.0;
    }
    kinds
        .iter()
        .map(|kind| mono_px(kind, 10.5) + 16.0)
        .sum::<f32>()
        + 4.0 * kinds.len().saturating_sub(1) as f32
}

fn role_px(field: &SchemaField) -> f32 {
    if field.roles.is_empty() {
        return mono_px("—", 12.0);
    }
    let icons = usize::from(field.hash) + usize::from(field.range);
    mono_px(&field.roles.join(", "), 12.0) + icons as f32 * 16.0
}

fn present_px(present: usize, total: usize) -> f32 {
    if total == 0 {
        mono_px("—", 12.0)
    } else {
        52.0 + 8.0 + mono_px(&format!("{present}/{total}"), 12.0)
    }
}

fn schema_head_row(widths: &SchemaWidths, color: Color, line: Color) -> Element<'static, Message> {
    row![
        schema_label("COLUMN", widths.name, color, line),
        schema_label("TYPE", widths.kind, color, line),
        schema_label("ROLE", widths.role, color, line),
        schema_label("PRESENT", widths.present, color, line),
        schema_label("SAMPLE", widths.sample, color, line),
    ]
    .spacing(0)
    .into()
}

fn schema_label(
    label: &'static str,
    width: f32,
    color: Color,
    line: Color,
) -> Element<'static, Message> {
    column![
        container(text(label).size(11).color(color).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..iced::Font::default()
        }))
        .width(Length::Fixed(width))
        .padding([8.0, 10.0]),
        hairline_fixed(line, width),
    ]
    .spacing(0)
    .width(Length::Fixed(width))
    .into()
}

fn schema_data_row(
    field: &SchemaField,
    total: usize,
    widths: &SchemaWidths,
    rule: bool,
    line: Color,
    p: Palette,
) -> Element<'static, Message> {
    let ink = popup_ink(p.dark);
    row![
        schema_box(
            text(field.name.clone())
                .size(12.5)
                .font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..iced::Font::MONOSPACE
                })
                .color(p.text)
                .wrapping(iced::widget::text::Wrapping::None)
                .into(),
            widths.name,
            rule,
            line,
        ),
        schema_box(type_chips(&field.kinds, p), widths.kind, rule, line),
        schema_box(
            role_line(field, ink.key_table, ink.grip),
            widths.role,
            rule,
            line,
        ),
        schema_box(
            present_line(field.present, total, p),
            widths.present,
            rule,
            line,
        ),
        schema_box(
            text(field.sample.clone())
                .size(12)
                .font(iced::Font::MONOSPACE)
                .color(ink.text_secondary)
                .wrapping(iced::widget::text::Wrapping::None)
                .into(),
            widths.sample,
            rule,
            line,
        ),
    ]
    .spacing(0)
    .into()
}

fn schema_box(
    body: Element<'static, Message>,
    width: f32,
    rule: bool,
    line: Color,
) -> Element<'static, Message> {
    let mut cell = column![container(body)
        .width(Length::Fixed(width))
        .padding([8.0, 10.0])]
    .spacing(0)
    .width(Length::Fixed(width));
    if rule {
        cell = cell.push(hairline_fixed(line, width));
    }
    cell.into()
}

fn type_chips(kinds: &[&str], p: Palette) -> Element<'static, Message> {
    let shown: Vec<&str> = if kinds.is_empty() {
        vec!["—"]
    } else {
        kinds.to_vec()
    };
    let mut line = row![].spacing(4).align_y(iced::alignment::Vertical::Center);
    for kind in shown {
        let (bg, fg) = type_colors(if kind == "—" { "empty" } else { kind }, p.dark);
        line = line.push(
            container(
                text(kind.to_string())
                    .size(10.5)
                    .font(iced::Font {
                        weight: iced::font::Weight::Bold,
                        ..iced::Font::MONOSPACE
                    })
                    .color(fg),
            )
            .padding([2.0, 7.0])
            .style(style::chip(bg, fg)),
        );
    }
    line.into()
}

fn type_colors(kind: &str, dark: bool) -> (Color, Color) {
    let ink = popup_ink(dark);
    match kind {
        "STRING" => (ink.ok_bg, ink.teal_ink),
        "NUMBER" => (ink.orange_bg, ink.orange),
        "BOOL" => (ink.blue_wash, ink.blue_ink),
        "NULL" => (ink.status_bg, ink.muted_2),
        "LIST" => (ink.purple_bg, ink.purple),
        "MAP" => (ink.magenta_bg, ink.magenta),
        _ => (ink.paper_2, ink.grip),
    }
}

fn role_line(field: &SchemaField, color: Color, grip: Color) -> Element<'static, Message> {
    if field.roles.is_empty() {
        return text("—").size(12).color(grip).into();
    }
    let mut line = row![].spacing(6).align_y(iced::alignment::Vertical::Center);
    if field.hash {
        line = line.push(icons::mark(Mark::Hash, color, 12.0));
    }
    if field.range {
        line = line.push(icons::mark(Mark::Range, color, 12.0));
    }
    line = line.push(text(field.roles.join(", ")).size(12).color(color));
    line.into()
}

fn present_line(present: usize, total: usize, p: Palette) -> Element<'static, Message> {
    let ink = popup_ink(p.dark);
    if total == 0 {
        return text("—").size(12).color(ink.grip).into();
    }
    let share = present as f32 / total as f32;
    let fill = if present < total {
        ink.sparse
    } else {
        ink.teal_ink
    };
    row![
        container(
            container(text(""))
                .width(Length::Fixed(52.0 * share))
                .height(Length::Fill)
                .style(style::fill(fill))
        )
        .width(Length::Fixed(52.0))
        .height(4)
        .clip(true)
        .style(round_fill(p.line)),
        text(format!("{present}/{total}")).size(12).color(p.text),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center)
    .into()
}

fn round_fill(color: Color) -> impl Fn(&iced::Theme) -> iced::widget::container::Style {
    move |_| {
        iced::widget::container::Style::default()
            .background(color)
            .border(iced::Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 999.0.into(),
            })
    }
}

fn query_block(statement: &str, p: Palette) -> Option<Element<'static, Message>> {
    let text = statement.trim();
    if text.is_empty() {
        return None;
    }
    let ink = popup_ink(p.dark);
    let mut lines = column![].spacing(0);
    for line in text.lines() {
        lines = lines.push(query_line(line, p));
    }
    Some(
        container(lines)
            .width(Fill)
            .padding([10.0, 12.0])
            .style(move |_| {
                iced::widget::container::Style::default()
                    .background(ink.query_bg)
                    .border(iced::Border {
                        color: ink.query_border,
                        width: 1.0,
                        radius: 8.0.into(),
                    })
            })
            .into(),
    )
}

fn query_line(line: &str, p: Palette) -> Element<'static, Message> {
    let ink = popup_ink(p.dark);
    let mut parts: Vec<Element<'static, Message>> = Vec::new();
    let mut cursor = 0usize;
    for (range, tone) in highlight::spans(line) {
        if range.start > cursor {
            if let Some(slice) = line.get(cursor..range.start) {
                parts.push(query_span(slice, highlight::Tone::Plain, &ink));
            }
        }
        if let Some(slice) = line.get(range.clone()) {
            parts.push(query_span(slice, tone, &ink));
        }
        cursor = range.end;
    }
    if let Some(slice) = line.get(cursor..) {
        if !slice.is_empty() || parts.is_empty() {
            parts.push(query_span(slice, highlight::Tone::Plain, &ink));
        }
    }
    row(parts).spacing(0).into()
}

fn query_span(slice: &str, tone: highlight::Tone, ink: &PopupInk) -> Element<'static, Message> {
    let (color, weight, style) = match tone {
        highlight::Tone::Keyword => (
            ink.teal_ink,
            iced::font::Weight::Bold,
            iced::font::Style::Normal,
        ),
        highlight::Tone::Write => (
            ink.danger_bright,
            iced::font::Weight::Bold,
            iced::font::Style::Normal,
        ),
        highlight::Tone::String => (
            ink.rose,
            iced::font::Weight::Medium,
            iced::font::Style::Normal,
        ),
        highlight::Tone::Comment => (
            ink.comment,
            iced::font::Weight::Normal,
            iced::font::Style::Italic,
        ),
        highlight::Tone::Plain => (
            ink.text_strong,
            iced::font::Weight::Medium,
            iced::font::Style::Normal,
        ),
    };
    text(slice.to_string())
        .size(12.5)
        .color(color)
        .font(iced::Font {
            weight,
            style,
            ..iced::Font::MONOSPACE
        })
        .wrapping(iced::widget::text::Wrapping::None)
        .into()
}

fn json_block(json: &str, p: Palette) -> Element<'static, Message> {
    let ink = popup_ink(p.dark);
    let mut lines = column![].spacing(0);
    for line in json.lines() {
        lines = lines.push(json_line(line, &ink));
    }
    container(lines)
        .width(Fill)
        .padding([12.0, 14.0])
        .style(move |_| {
            iced::widget::container::Style::default()
                .background(ink.paper_2)
                .border(iced::Border {
                    color: ink.line_mid,
                    width: 1.0,
                    radius: 8.0.into(),
                })
        })
        .into()
}

fn json_line(line: &str, ink: &PopupInk) -> Element<'static, Message> {
    let mut parts: Vec<Element<'static, Message>> = Vec::new();
    for (token, tone) in json_tokens(line) {
        let (color, weight) = match tone {
            JsonTone::Key => (ink.teal_ink, iced::font::Weight::Semibold),
            JsonTone::String => (ink.rose, iced::font::Weight::Normal),
            JsonTone::Number => (ink.amber, iced::font::Weight::Normal),
            JsonTone::Bool | JsonTone::Null => (ink.sky, iced::font::Weight::Semibold),
            JsonTone::Operator | JsonTone::Plain => {
                (ink.text_secondary, iced::font::Weight::Normal)
            }
        };
        parts.push(
            text(token)
                .size(12.5)
                .color(color)
                .font(iced::Font {
                    weight,
                    ..iced::Font::MONOSPACE
                })
                .wrapping(iced::widget::text::Wrapping::None)
                .into(),
        );
    }
    if parts.is_empty() {
        parts.push(text("").size(12.5).font(iced::Font::MONOSPACE).into());
    }
    row(parts).spacing(0).into()
}

#[derive(Clone, Copy)]
enum JsonTone {
    Plain,
    Key,
    String,
    Number,
    Bool,
    Null,
    Operator,
}

fn json_tokens(line: &str) -> Vec<(String, JsonTone)> {
    let chars: Vec<char> = line.chars().collect();
    let mut index = 0usize;
    let mut out = Vec::new();
    while index < chars.len() {
        let ch = chars[index];
        if ch.is_whitespace() {
            let start = index;
            while index < chars.len() && chars[index].is_whitespace() {
                index += 1;
            }
            out.push((chars[start..index].iter().collect(), JsonTone::Plain));
            continue;
        }
        if ch == '"' {
            let start = index;
            index += 1;
            while index < chars.len() {
                if chars[index] == '\\' {
                    index += 2;
                    continue;
                }
                if chars[index] == '"' {
                    index += 1;
                    break;
                }
                index += 1;
            }
            let token: String = chars[start..index].iter().collect();
            let rest: String = chars[index..].iter().collect();
            let tone = if rest.trim_start().starts_with(':') {
                JsonTone::Key
            } else {
                JsonTone::String
            };
            out.push((token, tone));
            continue;
        }
        if ch == '-' || ch.is_ascii_digit() {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_digit()
                    || matches!(chars[index], '.' | 'e' | 'E' | '+' | '-'))
            {
                index += 1;
            }
            out.push((chars[start..index].iter().collect(), JsonTone::Number));
            continue;
        }
        let rest: String = chars[index..].iter().collect();
        if let Some((word, tone)) = ["true", "false", "null"]
            .into_iter()
            .find(|word| rest.starts_with(word))
            .map(|word| {
                let tone = if word == "null" {
                    JsonTone::Null
                } else {
                    JsonTone::Bool
                };
                (word, tone)
            })
        {
            index += word.chars().count();
            out.push((word.to_string(), tone));
            continue;
        }
        index += 1;
        let tone = if "{}[],:".contains(ch) {
            JsonTone::Operator
        } else {
            JsonTone::Plain
        };
        out.push((ch.to_string(), tone));
    }
    out
}

fn semibold() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Semibold,
        ..iced::Font::default()
    }
}

fn bar_tip<'a>(
    target: impl Into<Element<'a, Message>>,
    label: &'static str,
    p: Palette,
) -> Element<'a, Message> {
    tooltip(
        target,
        container(text(label).size(12).color(p.header_fg).width(260))
            .padding([6, 8])
            .style(move |_| {
                iced::widget::container::Style::default()
                    .background(p.bar)
                    .color(p.header_fg)
                    .border(iced::Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: 6.0.into(),
                    })
            }),
        tooltip::Position::Bottom,
    )
    .delay(Duration::from_millis(200))
    .into()
}

struct PopupInk {
    teal_ink: Color,
    card_a: Color,
    query_bg: Color,
    query_border: Color,
    text_strong: Color,
    text_secondary: Color,
    label: Color,
    label_2: Color,
    schema_line: Color,
    paper_2: Color,
    line_mid: Color,
    rose: Color,
    amber: Color,
    sky: Color,
    sparse: Color,
    orange: Color,
    orange_bg: Color,
    ok_bg: Color,
    blue_wash: Color,
    blue_ink: Color,
    purple: Color,
    purple_bg: Color,
    magenta: Color,
    magenta_bg: Color,
    status_bg: Color,
    grip: Color,
    comment: Color,
    danger_bright: Color,
    hover_quiet: Color,
    key_table: Color,
    muted_2: Color,
    line: Color,
}

fn popup_ink(dark: bool) -> PopupInk {
    let rgb = style::rgb;
    if dark {
        PopupInk {
            teal_ink: rgb(0x5e, 0xea, 0xd4),
            card_a: rgb(0x15, 0x24, 0x2a),
            query_bg: rgb(0x15, 0x24, 0x20),
            query_border: rgb(0x2a, 0x45, 0x40),
            text_strong: rgb(0xf5, 0xf9, 0xfc),
            text_secondary: rgb(0xc5, 0xd2, 0xe0),
            label: rgb(0x9a, 0xab, 0xbd),
            label_2: rgb(0x9a, 0xab, 0xbd),
            schema_line: rgb(0x2a, 0x38, 0x48),
            paper_2: rgb(0x1c, 0x28, 0x36),
            line_mid: rgb(0x2c, 0x3d, 0x50),
            rose: rgb(0xfd, 0xa4, 0xaf),
            amber: rgb(0xfd, 0xba, 0x74),
            sky: rgb(0x7d, 0xd3, 0xfc),
            sparse: rgb(0xfb, 0xbf, 0x24),
            orange: rgb(0xfd, 0xba, 0x74),
            orange_bg: rgb(0x2a, 0x1c, 0x14),
            ok_bg: rgb(0x12, 0x30, 0x28),
            blue_wash: rgb(0x15, 0x20, 0x33),
            blue_ink: rgb(0x93, 0xc5, 0xfd),
            purple: rgb(0xc4, 0xb5, 0xfd),
            purple_bg: rgb(0x24, 0x1c, 0x33),
            magenta: rgb(0xf0, 0xab, 0xfc),
            magenta_bg: rgb(0x2a, 0x1a, 0x2e),
            status_bg: rgb(0x1e, 0x29, 0x36),
            grip: rgb(0x94, 0xa3, 0xb8),
            comment: rgb(0x8b, 0x97, 0xa8),
            danger_bright: rgb(0xfc, 0xa5, 0xa5),
            hover_quiet: rgb(0x2c, 0x3c, 0x50),
            key_table: rgb(0x86, 0xef, 0xac),
            muted_2: rgb(0x8b, 0x9e, 0xb3),
            line: rgb(0x2a, 0x38, 0x48),
        }
    } else {
        PopupInk {
            teal_ink: rgb(0x0f, 0x76, 0x6e),
            card_a: rgb(0xf6, 0xfb, 0xf9),
            query_bg: rgb(0xf4, 0xf8, 0xf6),
            query_border: rgb(0xdc, 0xe8, 0xe2),
            text_strong: rgb(0x12, 0x20, 0x2b),
            text_secondary: rgb(0x33, 0x41, 0x55),
            label: rgb(0x52, 0x60, 0x71),
            label_2: rgb(0x52, 0x62, 0x73),
            schema_line: rgb(0xed, 0xf0, 0xf3),
            paper_2: rgb(0xf8, 0xfa, 0xfc),
            line_mid: rgb(0xe6, 0xeb, 0xf0),
            rose: rgb(0x9f, 0x12, 0x39),
            amber: rgb(0xb4, 0x53, 0x09),
            sky: rgb(0x03, 0x69, 0xa1),
            sparse: rgb(0xd9, 0x77, 0x06),
            orange: rgb(0xc2, 0x41, 0x0c),
            orange_bg: rgb(0xff, 0xf7, 0xed),
            ok_bg: rgb(0xec, 0xfd, 0xf5),
            blue_wash: rgb(0xef, 0xf6, 0xff),
            blue_ink: rgb(0x1d, 0x4e, 0xd8),
            purple: rgb(0x6d, 0x28, 0xd9),
            purple_bg: rgb(0xf5, 0xf3, 0xff),
            magenta: rgb(0xa2, 0x1c, 0xaf),
            magenta_bg: rgb(0xfd, 0xf4, 0xff),
            status_bg: rgb(0xf1, 0xf5, 0xf9),
            grip: rgb(0x94, 0xa3, 0xb8),
            comment: rgb(0x6b, 0x72, 0x80),
            danger_bright: rgb(0xdc, 0x26, 0x26),
            hover_quiet: rgb(0xee, 0xf2, 0xf6),
            key_table: rgb(0x3f, 0x6f, 0x57),
            muted_2: rgb(0x64, 0x74, 0x8b),
            line: rgb(0xe4, 0xe7, 0xeb),
        }
    }
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
    let at = menu.at;
    Some(
        float(card)
            .translate(move |bounds, viewport| place(at, bounds, viewport))
            .into(),
    )
}

fn floating_search(app: &App) -> Element<'_, Message> {
    let p = style::palette(app.prefs.dark);
    let field = if app.search_bounds.width > 1.0 {
        app.search_bounds
    } else {
        Rectangle {
            x: 220.0,
            y: 6.0,
            width: 480.0,
            height: 24.0,
        }
    };
    let card = search_list(app, p, field.width);
    let left = field.x;
    let top = field.y + field.height + 6.0;
    float(card)
        .translate(move |bounds, _viewport| Vector::new(left - bounds.x, top - bounds.y))
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

fn report_bounds<'a>(
    content: impl Into<Element<'a, Message>>,
    report: fn(Rectangle) -> Message,
) -> Element<'a, Message> {
    BoundsProbe {
        content: content.into(),
        report,
    }
    .into()
}

struct BoundsProbe<'a> {
    content: Element<'a, Message>,
    report: fn(Rectangle) -> Message,
}

#[derive(Default)]
struct BoundsState {
    last: Option<Rectangle>,
}

impl<'a> iced::advanced::Widget<Message, iced::Theme, iced::Renderer> for BoundsProbe<'a> {
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<BoundsState>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(BoundsState::default())
    }

    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        vec![iced::advanced::widget::Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        tree.diff_children(&[&self.content]);
    }

    fn size(&self) -> iced::Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<BoundsState>();
        if state.last != Some(bounds) {
            state.last = Some(bounds);
            shell.publish((self.report)(bounds));
        }
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> iced::advanced::mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a> From<BoundsProbe<'a>> for Element<'a, Message> {
    fn from(probe: BoundsProbe<'a>) -> Self {
        Element::new(probe)
    }
}

#[cfg(test)]
mod tests {
    use super::settings_cache_age;

    #[test]
    fn cache_age_matches_the_web_settings_label() {
        let now = 1_700_000_000_000;
        assert_eq!(settings_cache_age(None, now), "not cached");
        assert_eq!(settings_cache_age(Some(now), now), "(just now)");
        assert_eq!(settings_cache_age(Some(now - 12_000), now), "(12s ago)");
        assert_eq!(settings_cache_age(Some(now - 120_000), now), "(2 min ago)");
        assert_eq!(settings_cache_age(Some(now - 7_200_000), now), "(2 hr ago)");
        assert_eq!(
            settings_cache_age(Some(now - 86_400_000), now),
            "(1 day ago)"
        );
        assert_eq!(
            settings_cache_age(Some(now - 2 * 86_400_000), now),
            "(2 days ago)"
        );
    }

    #[test]
    fn table_name_skips_a_leading_comment() {
        assert_eq!(
            super::table_name("-- the read\nSELECT * FROM widgets WHERE id = 'a';").as_deref(),
            Some("widgets")
        );
    }

    #[test]
    fn row_json_follows_column_order() {
        let item = serde_json::json!({"name": "bolt", "id": "a", "sku": "s1"});
        let columns = vec![
            crate::web::shape::Column {
                name: "id".to_string(),
                table: Some("hash".to_string()),
                index: None,
            },
            crate::web::shape::Column {
                name: "sku".to_string(),
                table: Some("range".to_string()),
                index: None,
            },
            crate::web::shape::Column {
                name: "name".to_string(),
                table: None,
                index: None,
            },
        ];
        let text = super::ordered_json(&item, &columns);
        let id = text.find("\"id\"").unwrap();
        let sku = text.find("\"sku\"").unwrap();
        let name = text.find("\"name\"").unwrap();
        assert!(id < sku && sku < name, "{text}");
    }
}
