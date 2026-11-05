//! Layout for the native window. It follows the web UI: header, file tree,
//! editor, results, and a table browser.

use iced::widget::{button, column, container, mouse_area, row, scrollable, text, text_input};
use iced::{Element, Fill, Length};

use crate::web::files::Node;
use crate::web::shape::split_statements;
use crate::web::studio::{menu_labels, StatementCard};

use super::highlight;
use super::style::{self, Palette};
use super::{App, Dialog, Message, Mode};

pub fn view(app: &App) -> Element<'_, Message> {
    let colors = style::palette(app.prefs.dark);
    let body = match app.mode {
        Mode::Query => query(app),
        Mode::Tables => tables(app, colors),
    };
    column![header(app), body, status_bar(app)]
        .spacing(0)
        .into()
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
    let endpoint = button(text(app.endpoint.clone()).size(12))
        .on_press(Message::ToggleEnv)
        .padding([1, 8])
        .style(style::paint(p, style::Look::Pill));
    let theme = button(text(if app.prefs.dark { "☀" } else { "☾" }).size(13))
        .on_press(Message::ToggleTheme)
        .padding([2, 8])
        .style(style::paint(p, style::Look::Bar));
    let settings = button(text("☰").size(14))
        .on_press(Message::ToggleSettings)
        .padding([2, 8])
        .style(style::paint(p, style::Look::Bar));
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
    if !app.hits.is_empty() || app.show_env || app.show_settings {
        col = col.push(header_pop(app, p));
    }
    col.into()
}

fn header_pop(app: &App, p: Palette) -> Element<'_, Message> {
    let mut body = column![].spacing(6).padding(10);
    for (index, hit) in app.hits.iter().take(8).enumerate() {
        body = body.push(
            button(
                row![
                    text(hit.kind.clone())
                        .size(11)
                        .color(p.muted)
                        .width(Length::Fixed(58.0)),
                    text(hit.title.clone()).size(13).color(p.text),
                ]
                .spacing(8),
            )
            .on_press(Message::PickHit(index))
            .width(Fill)
            .padding([4, 6])
            .style(style::paint(p, style::Look::Quiet)),
        );
    }
    if app.show_env {
        for (key, value) in &app.env_lines {
            body = body.push(text(format!("{key}: {value}")).size(12).color(p.muted));
        }
    }
    if app.show_settings {
        body = body.push(settings_panel(app, p));
    }
    container(body)
        .width(Fill)
        .padding([4, 12])
        .style(style::fill(p.paper))
        .into()
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
    button(text(label).size(13))
        .on_press(message)
        .padding([3, 14])
        .style(style)
        .into()
}

fn query(app: &App) -> Element<'_, Message> {
    let p = style::palette(app.prefs.dark);
    row![
        tree_pane(app, p),
        column![
            editor_head(app, p),
            dialog(app, p),
            editor_body(app, p),
            results(app, p)
        ]
        .spacing(0)
        .width(Fill)
    ]
    .spacing(0)
    .height(Fill)
    .into()
}

fn tree_pane(app: &App, p: Palette) -> Element<'_, Message> {
    let head = row![
        button(text("Files").size(13).color(p.text))
            .on_press(Message::MenuRoot)
            .padding([2, 4])
            .style(style::paint(p, style::Look::Quiet)),
        button(text("‹").size(14).color(p.muted))
            .on_press(Message::Nav(-24.0))
            .padding([2, 4])
            .style(style::paint(p, style::Look::Quiet)),
        button(text("›").size(14).color(p.muted))
            .on_press(Message::Nav(24.0))
            .padding([2, 4])
            .style(style::paint(p, style::Look::Quiet)),
    ]
    .spacing(2)
    .align_y(iced::alignment::Vertical::Center)
    .padding([4, 6]);
    let mut tree = column![head].spacing(0);
    if let Some(menu) = &app.menu {
        let mut items = column![text(display_path(&menu.path)).size(11).color(p.muted)].spacing(0);
        for label in menu_labels(&menu.kind) {
            let style = style::paint(
                p,
                if label == "Delete" || label == "Run file" || label == "Run folder" {
                    style::Look::Danger
                } else {
                    style::Look::Quiet
                },
            );
            items = items.push(
                button(text(label).size(13))
                    .on_press(Message::MenuAct(label))
                    .width(Fill)
                    .padding([5, 8])
                    .style(style),
            );
        }
        items = items.push(
            button(text("Close menu").size(12).color(p.muted))
                .on_press(Message::CloseMenu)
                .width(Fill)
                .padding([4, 8])
                .style(style::paint(p, style::Look::Quiet)),
        );
        tree = tree.push(
            container(items)
                .padding(4)
                .width(Fill)
                .style(style::menu_card(p)),
        );
    }
    if let Some(root) = &app.tree {
        if let Some(children) = &root.children {
            for child in children {
                tree = tree.push(node_view(app, child, 0, p));
            }
        }
    }
    container(scrollable(tree).height(Fill))
        .width(Length::Fixed(app.prefs.nav))
        .height(Fill)
        .style(style::fill(p.side))
        .into()
}

fn node_view<'a>(app: &'a App, node: &'a Node, depth: usize, p: Palette) -> Element<'a, Message> {
    let pad = Length::Fixed(8.0 + depth as f32 * 14.0);
    if node.kind == "dir" {
        let mut col = column![tree_row(app, node, pad, p)].spacing(0);
        if let Some(children) = &node.children {
            for child in children {
                col = col.push(node_view(app, child, depth + 1, p));
            }
        }
        col.into()
    } else {
        tree_row(app, node, pad, p)
    }
}

fn tree_row<'a>(app: &'a App, node: &'a Node, pad: Length, p: Palette) -> Element<'a, Message> {
    let open = node.kind == "file" && node.path == app.open_path;
    let running = node.kind == "file" && node.path == app.running_path;
    let mark = if running {
        text(if app.pulse { "●" } else { "○" })
            .size(9)
            .color(p.blue)
    } else {
        text("").size(9)
    };
    let label = text(node.name.clone()).size(13).color(p.text);
    let inner = row![
        container(text("")).width(pad),
        label,
        mark,
        iced::widget::Space::new().width(Fill),
        container(text(""))
            .width(2)
            .height(Length::Fixed(22.0))
            .style(style::fill(if open { p.teal } else { p.side })),
    ]
    .spacing(6)
    .height(Length::Fixed(22.0))
    .align_y(iced::alignment::Vertical::Center);
    mouse_area(inner)
        .on_press(Message::Open(node.path.clone()))
        .on_right_press(Message::Menu(node.path.clone(), node.kind.to_string()))
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
    if !app.open_path.is_empty() {
        buttons = buttons.push(
            button(text("Run file").size(12))
                .on_press(Message::RunFile)
                .padding([3, 10])
                .style(style::paint(p, style::Look::Primary)),
        );
        buttons = buttons.push(
            button(text("Run").size(12))
                .on_press(Message::RunCursor)
                .padding([3, 10])
                .style(style::paint(p, style::Look::Primary)),
        );
    }
    if !app.running_path.is_empty() {
        buttons = buttons.push(
            button(text("Stop").size(12))
                .on_press(Message::Stop)
                .padding([3, 10])
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
                button(text("Load disk").size(12))
                    .on_press(Message::ReloadDisk)
                    .padding([3, 8])
                    .style(style::paint(p, style::Look::Primary)),
                button(text("Keep mine").size(12))
                    .on_press(Message::KeepEdits)
                    .padding([3, 8])
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
                button(text("OK").size(12))
                    .on_press(Message::DialogOk)
                    .padding([3, 10])
                    .style(style::paint(p, style::Look::Primary)),
                button(text("Cancel").size(12))
                    .on_press(Message::DialogCancel)
                    .padding([3, 8])
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
                button(text("Move").size(12))
                    .on_press(Message::DialogOk)
                    .padding([3, 10])
                    .style(style::paint(p, style::Look::Primary)),
                button(text("Cancel").size(12))
                    .on_press(Message::DialogCancel)
                    .padding([3, 8])
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
    let statements = split_statements(&source);
    let mut gutter = column![].spacing(0).width(Length::Fixed(28.0));
    for statement in &statements {
        let write = app.prefs.write_bands
            && statement_word(statement).is_some_and(|word| {
                matches!(word, "INSERT" | "UPDATE" | "DELETE" | "DROP" | "CREATE")
            });
        let style = style::paint(
            p,
            if write {
                style::Look::Danger
            } else {
                style::Look::Quiet
            },
        );
        gutter = gutter.push(
            button(text(if write { "▸" } else { "▶" }).size(11))
                .on_press(Message::RunStatement(statement.clone()))
                .padding(0)
                .style(style),
        );
    }
    let editor = highlight::editor(&app.editor)
        .on_action(Message::Editor)
        .height(Fill)
        .padding(8);
    let mut below = column![].spacing(2);
    if !app.running_path.is_empty() {
        let total = app.live.total.unwrap_or(0);
        let bar = progress_bar(app.live.done, total);
        below = below.push(
            text(format!(
                "Running {}  {} {}/{} {bar}",
                app.running_path, app.live.phase, app.live.done, total
            ))
            .size(12)
            .color(p.blue),
        );
    }
    if !app.suggest.is_empty() {
        let mut suggestions = row![text("Suggestions").size(11).color(p.muted)].spacing(4);
        for (index, hit) in app.suggest.iter().take(6).enumerate() {
            suggestions = suggestions.push(
                button(
                    text(format!("{} {}", hit.kind, hit.title))
                        .size(11)
                        .color(p.text),
                )
                .on_press(Message::UseSuggest(index))
                .padding([2, 6])
                .style(style::paint(p, style::Look::Quiet)),
            );
        }
        below = below.push(suggestions);
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
    container(
        column![
            row![
                container(gutter).style(style::fill(p.gutter)).height(Fill),
                editor
            ]
            .spacing(0)
            .height(Fill),
            below.padding([4, 10])
        ]
        .height(Fill),
    )
    .width(Fill)
    .height(Fill)
    .style(style::fill(p.paper))
    .into()
}

fn editor_text_view(app: &App) -> String {
    super::editor_text(&app.editor)
}

fn statement_word(statement: &str) -> Option<&str> {
    statement.split_whitespace().next()
}

fn results(app: &App, p: Palette) -> Element<'_, Message> {
    let grip = row![
        text("Results").size(12).color(p.muted),
        button(text("–").size(14).color(p.muted))
            .on_press(Message::Results(-40.0))
            .padding([0, 6])
            .style(style::paint(p, style::Look::Quiet)),
        button(text("+").size(14).color(p.muted))
            .on_press(Message::Results(40.0))
            .padding([0, 6])
            .style(style::paint(p, style::Look::Quiet)),
    ]
    .spacing(4)
    .align_y(iced::alignment::Vertical::Center);
    let mut cards = column![grip].spacing(8).padding(8);
    if let Some(json) = &app.row_json {
        cards = cards.push(
            container(
                column![
                    button(text("Close row").size(12).color(p.text))
                        .on_press(Message::CloseRow)
                        .padding([2, 6])
                        .style(style::paint(p, style::Look::Quiet)),
                    text(json.clone())
                        .font(iced::Font::MONOSPACE)
                        .size(12)
                        .color(p.text),
                ]
                .spacing(4)
                .padding(8),
            )
            .width(Fill)
            .style(style::card(p)),
        );
    }
    for (index, card) in app.results.iter().take(20).enumerate() {
        cards = cards.push(card_view(app, index, card, p));
    }
    container(scrollable(cards).height(Fill))
        .height(Length::Fixed(app.prefs.results))
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
    let (flag, flag_color, flag_bg) = if card.ok {
        ("ok", p.ok, p.ok_bg)
    } else {
        ("failed", p.danger, p.amber_bg)
    };
    let mut head = row![
        button(
            text(if app.open_cards.get(index).copied().unwrap_or(false) {
                "●"
            } else {
                "○"
            })
            .size(10)
            .color(p.muted)
        )
        .on_press(Message::ToggleCard(index))
        .padding(2)
        .style(style::paint(p, style::Look::Quiet)),
        container(text(flag).size(11).color(flag_color))
            .padding([1, 6])
            .style(style::chip(flag_bg, flag_color)),
        text(card.kind.clone()).size(12).color(p.muted),
        text(format!("{} ms", card.elapsed_ms))
            .size(12)
            .color(p.muted),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center);
    if card.read_units > 0.0 {
        head = head.push(
            container(
                text(format!("RCU {:.1}", card.read_units))
                    .size(11)
                    .color(p.text),
            )
            .padding([1, 6])
            .style(style::chip(p.chip, p.text)),
        );
    }
    if card.write_units > 0.0 {
        head = head.push(
            container(
                text(format!("WCU {:.1}", card.write_units))
                    .size(11)
                    .color(p.text),
            )
            .padding([1, 6])
            .style(style::chip(p.chip, p.text)),
        );
    }
    let mut body = column![head].spacing(4);
    if app.open_cards.get(index).copied().unwrap_or(false) {
        body = body.push(
            text(card.statement.clone())
                .font(iced::Font::MONOSPACE)
                .size(12)
                .color(p.text),
        );
    }
    if !card.ok {
        body = body.push(text(card.failure_text()).size(12).color(p.danger));
    } else if !card.message.is_empty() && card.items.is_empty() {
        body = body.push(text(card.message.clone()).size(12).color(p.text));
    }
    if let Some(help) = &card.help {
        let pretty = serde_json::to_string_pretty(help).unwrap_or_else(|_| help.to_string());
        body = body.push(
            text(pretty)
                .font(iced::Font::MONOSPACE)
                .size(12)
                .color(p.text),
        );
    }
    if !card.items.is_empty() {
        body = body.push(items_table(&card.columns, &card.items, true, false, p));
    }
    container(body)
        .padding(8)
        .width(Fill)
        .style(style::card(p))
        .into()
}

fn progress_bar(done: u64, total: u64) -> String {
    if total == 0 {
        return String::new();
    }
    let width = 16usize;
    let filled = ((done as f64 / total as f64) * width as f64).round() as usize;
    let filled = filled.min(width);
    format!("[{}{}]", "#".repeat(filled), ".".repeat(width - filled))
}

fn items_table<'a>(
    columns: &'a [crate::web::shape::Column],
    items: &'a [serde_json::Value],
    clickable: bool,
    pin_header: bool,
    p: Palette,
) -> Element<'a, Message> {
    let names: Vec<String> = if columns.is_empty() {
        items
            .first()
            .and_then(|item| item.as_object())
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default()
    } else {
        columns.iter().map(|column| column.name.clone()).collect()
    };
    let mut header = row![].spacing(8);
    for name in &names {
        header = header.push(text(name.clone()).size(12).color(p.muted));
    }
    let mut rows = column![].spacing(2);
    for (row_index, item) in items.iter().take(50).enumerate() {
        let mut line = row![].spacing(8);
        for name in &names {
            let cell = item.get(name).map(value_text).unwrap_or_default();
            line = line.push(
                text(cell)
                    .size(12)
                    .font(iced::Font::MONOSPACE)
                    .color(p.text),
            );
        }
        if clickable {
            let json = serde_json::to_string_pretty(item).unwrap_or_default();
            rows = rows.push(
                button(line)
                    .on_press(Message::ShowRow(json))
                    .padding(2)
                    .style(style::paint(p, style::Look::Quiet)),
            );
        } else {
            let _ = row_index;
            rows = rows.push(line);
        }
    }
    let both = scrollable::Direction::Both {
        vertical: scrollable::Scrollbar::default(),
        horizontal: scrollable::Scrollbar::default(),
    };
    if pin_header {
        column![header, scrollable(rows).direction(both)]
            .spacing(4)
            .into()
    } else {
        scrollable(column![header, rows].spacing(2))
            .direction(both)
            .into()
    }
}

fn tables(app: &App, p: Palette) -> Element<'_, Message> {
    let mut list = column![
        row![
            text_input("Filter tables", &app.table_filter)
                .on_input(Message::TableFilter)
                .size(13)
                .padding([4, 8])
                .style(style::field(p)),
            button(text("Refresh").size(12))
                .on_press(Message::RefreshTables(true))
                .padding([3, 8])
                .style(style::paint(p, style::Look::Quiet)),
            button(text("Clear cache").size(12))
                .on_press(Message::ClearCache)
                .padding([3, 8])
                .style(style::paint(p, style::Look::Quiet)),
        ]
        .spacing(8),
        text(if app.cache_text.is_empty() {
            app.table_note.clone()
        } else {
            format!("{}  {}", app.table_note, app.cache_text)
        })
        .size(12)
        .color(p.muted),
    ]
    .spacing(6)
    .padding(8);
    for table in &app.tables {
        let selected = table.name == app.open_table;
        let mut label =
            row![text(table.name.clone())
                .size(13)
                .color(if selected { p.teal } else { p.text })]
            .spacing(6)
            .align_y(iced::alignment::Vertical::Center);
        for key in &table.keys {
            label = label.push(
                container(
                    text(format!("{} {}", key.role, key.name))
                        .size(11)
                        .color(p.muted),
                )
                .padding([1, 6])
                .style(style::chip(p.chip, p.muted)),
            );
        }
        list = list.push(
            button(label)
                .on_press(Message::OpenTable(table.name.clone()))
                .width(Fill)
                .padding([4, 6])
                .style(style::paint(p, style::Look::Quiet)),
        );
    }
    let mut page = column![
        text(if app.open_table.is_empty() {
            "Select a table".to_string()
        } else {
            format!("{}  page {}", app.open_table, app.table_page + 1)
        })
        .size(14)
        .color(p.text),
        row![
            button(text("Previous").size(12))
                .on_press(Message::TablePage(-1))
                .padding([3, 8])
                .style(style::paint(p, style::Look::Quiet)),
            button(text("Next").size(12))
                .on_press(Message::TablePage(1))
                .padding([3, 8])
                .style(style::paint(p, style::Look::Quiet)),
            text(if app.table_has_more {
                "more rows"
            } else {
                "end"
            })
            .size(12)
            .color(p.muted),
        ]
        .spacing(8),
    ]
    .spacing(8)
    .padding(8);
    if !app.table_items.is_empty() {
        page = page.push(items_table(
            &app.table_columns,
            &app.table_items,
            true,
            true,
            p,
        ));
    }
    row![
        container(scrollable(list).height(Fill))
            .width(Length::Fixed(360.0))
            .height(Fill)
            .style(style::fill(p.side)),
        container(scrollable(page).height(Fill))
            .width(Fill)
            .height(Fill)
            .style(style::fill(p.paper)),
    ]
    .spacing(0)
    .height(Fill)
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

fn display_path(path: &str) -> &str {
    if path.is_empty() {
        "workspace"
    } else {
        path
    }
}

fn value_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}
