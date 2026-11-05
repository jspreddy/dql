//! Layout for the native window. It follows the web UI: header, file tree,
//! editor, results, and a table browser.

use iced::widget::{button, column, container, mouse_area, row, scrollable, text, text_input};
use iced::{Color, Element, Fill, Length};

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
    body = body.push(items_table(&card.columns, &card.items, true, true, p));
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
    container(text(label).size(12).color(fg))
        .height(22)
        .padding([0, 8])
        .style(style::chip(p.chip, fg))
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
    result_card: bool,
    p: Palette,
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
    let mut rows = column![header].spacing(0);
    for (index, item) in shown.into_iter().enumerate() {
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
    rows.into()
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
        text("◉").size(11).color(p.teal).into()
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
    button(cell)
        .on_press(Message::ShowRow(json))
        .padding(0)
        .width(28)
        .style(style::paint(p, style::Look::Quiet))
        .into()
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
        line = line.push(text("#").size(11).color(mark));
    }
    if column.table.as_deref() == Some("range") || column.index.as_deref() == Some("range") {
        line = line.push(text("↕").size(11).color(mark));
    }
    line.into()
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
            false,
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
