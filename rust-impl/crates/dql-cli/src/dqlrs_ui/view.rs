//! Layout for the native window. It follows the web UI: header, file tree,
//! editor, results, and a table browser.

use iced::widget::{button, column, container, mouse_area, row, scrollable, text, text_input};
use iced::{Color, Element, Fill, Length};

use crate::web::files::Node;
use crate::web::shape::split_statements;
use crate::web::studio::{menu_labels, StatementCard};

use super::highlight;
use super::{App, Dialog, Message, Mode};

const TEAL: Color = Color::from_rgb8(0x0f, 0x76, 0x6e);
const BLUE: Color = Color::from_rgb8(0x25, 0x63, 0xeb);
const DANGER: Color = Color::from_rgb8(0xb9, 0x1c, 0x1c);

pub fn view(app: &App) -> Element<'_, Message> {
    let body = match app.mode {
        Mode::Query => query(app),
        Mode::Tables => tables(app),
    };
    column![header(app), body, status_bar(app)]
        .spacing(0)
        .into()
}

fn header(app: &App) -> Element<'_, Message> {
    let brand = container(text("dqlrs").size(16))
        .padding([8, 14])
        .style(|_theme| {
            container::Style::default().background(if app.show_env {
                TEAL
            } else {
                Color::TRANSPARENT
            })
        });
    let mode = row![
        mode_button("Query", app.mode == Mode::Query, Message::Mode(Mode::Query)),
        mode_button(
            "Tables",
            app.mode == Mode::Tables,
            Message::Mode(Mode::Tables)
        ),
    ]
    .spacing(0);
    let mut search = column![text_input("Search tables, columns, queries", &app.search)
        .on_input(Message::Search)
        .width(Fill)]
    .spacing(2);
    for (index, hit) in app.hits.iter().take(8).enumerate() {
        let label = format!("{}  {}", hit.kind, hit.title);
        search = search.push(
            button(text(label).size(12))
                .on_press(Message::PickHit(index))
                .width(Fill),
        );
    }
    let endpoint = button(text(app.endpoint.clone()).size(12)).on_press(Message::ToggleEnv);
    let theme = button(text(if app.prefs.dark { "Light" } else { "Dark" }).size(12))
        .on_press(Message::ToggleTheme);
    let settings = button(text("Settings").size(12)).on_press(Message::ToggleSettings);
    let bar = row![brand, mode, search, endpoint, theme, settings]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center)
        .padding([4, 8]);
    let mut col = column![bar].spacing(4);
    if app.show_env {
        for (key, value) in &app.env_lines {
            col = col.push(text(format!("{key}: {value}")).size(12));
        }
    }
    if app.show_settings {
        col = col.push(settings_panel(app));
    }
    col.into()
}

fn settings_panel(app: &App) -> Element<'_, Message> {
    column![
        button(text(if app.prefs.write_bands {
            "Write bands: on"
        } else {
            "Write bands: off"
        }))
        .on_press(Message::ToggleBands),
        text("Ctrl+Enter or Ctrl+\\ runs the statement at the cursor.").size(12),
        text("Esc stops a run. Ctrl+S saves. The editor also saves as you type.").size(12),
        text("Right-click a file for Run file, rename, move, and delete.").size(12),
        text("A word of two letters or more lists matching tables, columns, and queries under the editor.").size(12),
    ]
    .spacing(4)
    .padding(8)
    .into()
}

fn mode_button(label: &'static str, active: bool, message: Message) -> Element<'static, Message> {
    let button = button(text(label).size(13)).on_press(message);
    if active {
        button.style(button::primary)
    } else {
        button.style(button::secondary)
    }
    .into()
}

fn query(app: &App) -> Element<'_, Message> {
    row![
        tree_pane(app),
        column![
            editor_head(app),
            dialog(app),
            editor_body(app),
            results(app)
        ]
        .spacing(4)
        .width(Fill)
    ]
    .spacing(0)
    .height(Fill)
    .into()
}

fn tree_pane(app: &App) -> Element<'_, Message> {
    let mut tree = column![row![
        button(text("Files").size(12)).on_press(Message::MenuRoot),
        button(text("<").size(12)).on_press(Message::Nav(-24.0)),
        button(text(">").size(12)).on_press(Message::Nav(24.0)),
    ]
    .spacing(4)]
    .spacing(2)
    .padding(6);
    if let Some(menu) = &app.menu {
        tree = tree.push(text(format!("{}  {}", menu.kind, display_path(&menu.path))).size(11));
        for label in menu_labels(&menu.kind) {
            let mut button = button(text(label).size(12))
                .on_press(Message::MenuAct(label))
                .width(Fill);
            if label == "Delete" || label == "Run file" || label == "Run folder" {
                button = button.style(button::danger);
            }
            tree = tree.push(button);
        }
        tree = tree.push(button(text("Close menu").size(11)).on_press(Message::CloseMenu));
    }
    if let Some(root) = &app.tree {
        if let Some(children) = &root.children {
            for child in children {
                tree = tree.push(node_view(app, child, 0));
            }
        }
    }
    container(scrollable(tree).height(Fill))
        .width(Length::Fixed(app.prefs.nav))
        .height(Fill)
        .into()
}

fn node_view<'a>(app: &'a App, node: &'a Node, depth: usize) -> Element<'a, Message> {
    let pad = Length::Fixed(depth as f32 * 12.0);
    if node.kind == "dir" {
        let mut col = column![tree_row(app, node, pad)].spacing(0);
        if let Some(children) = &node.children {
            for child in children {
                col = col.push(node_view(app, child, depth + 1));
            }
        }
        col.into()
    } else {
        tree_row(app, node, pad)
    }
}

fn tree_row<'a>(app: &'a App, node: &'a Node, pad: Length) -> Element<'a, Message> {
    let open = node.kind == "file" && node.path == app.open_path;
    let running = node.kind == "file" && node.path == app.running_path;
    let mark = if running {
        text(if app.pulse { "●" } else { "○" }).size(12).color(BLUE)
    } else {
        text(" ").size(12)
    };
    let label = text(node.name.clone()).size(13);
    let inner = row![
        container(text("")).width(pad),
        label,
        mark,
        if open {
            container(text(""))
                .width(2)
                .height(18)
                .style(|_theme| container::Style::default().background(TEAL))
        } else {
            container(text("")).width(2).height(18)
        }
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center);
    mouse_area(inner)
        .on_press(Message::Open(node.path.clone()))
        .on_right_press(Message::Menu(node.path.clone(), node.kind.to_string()))
        .into()
}

fn editor_head(app: &App) -> Element<'_, Message> {
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
        text(name).size(14),
        if app.open_path.is_empty() {
            container(text("")).height(0)
        } else {
            container(text(""))
                .height(2)
                .width(Fill)
                .style(|_theme| container::Style::default().background(TEAL))
        }
    ]
    .spacing(0);
    let mut buttons = row![
        button(text("<").size(12)).on_press(Message::Results(-40.0)),
        button(text(">").size(12)).on_press(Message::Results(40.0)),
    ]
    .spacing(4);
    if !app.open_path.is_empty() {
        buttons = buttons.push(
            button(text("Run file"))
                .on_press(Message::RunFile)
                .style(button::danger),
        );
    }
    if !app.running_path.is_empty() {
        buttons = buttons.push(button(text("Stop")).on_press(Message::Stop));
    }
    row![title, buttons]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center)
        .padding([4, 8])
        .into()
}

fn dialog(app: &App) -> Element<'_, Message> {
    match &app.dialog {
        Dialog::None => container(text("")).height(0).into(),
        Dialog::Conflict => row![
            text("This file changed on disk. Your edits were kept.").size(12),
            button(text("Load disk")).on_press(Message::ReloadDisk),
            button(text("Keep mine")).on_press(Message::KeepEdits),
        ]
        .spacing(8)
        .padding(8)
        .into(),
        Dialog::Name { title, .. } => row![
            text(*title).size(13),
            text_input("name", &app.dialog_text).on_input(Message::DialogText),
            button(text("OK")).on_press(Message::DialogOk),
            button(text("Cancel")).on_press(Message::DialogCancel),
        ]
        .spacing(8)
        .padding(8)
        .into(),
        Dialog::Move { path } => row![
            text(format!("Move {path} to")).size(12),
            text_input("folder or path", &app.dialog_text).on_input(Message::DialogText),
            button(text("Move")).on_press(Message::DialogOk),
            button(text("Cancel")).on_press(Message::DialogCancel),
        ]
        .spacing(8)
        .padding(8)
        .into(),
    }
}

fn editor_body(app: &App) -> Element<'_, Message> {
    let source = editor_text_view(app);
    let statements = split_statements(&source);
    let mut gutter = column![].spacing(2).width(Length::Fixed(36.0));
    for statement in &statements {
        let write = app.prefs.write_bands
            && statement_word(statement).is_some_and(|word| {
                matches!(word, "INSERT" | "UPDATE" | "DELETE" | "DROP" | "CREATE")
            });
        let mut play = button(text(if write { "▸" } else { "▶" }).size(11))
            .on_press(Message::RunStatement(statement.clone()))
            .padding(2);
        if write {
            play = play.style(button::danger);
        }
        gutter = gutter.push(play);
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
            .color(BLUE),
        );
    }
    if !app.suggest.is_empty() {
        let mut suggestions = row![text("Suggestions").size(12)].spacing(4);
        for (index, hit) in app.suggest.iter().take(6).enumerate() {
            suggestions = suggestions.push(
                button(text(format!("{} {}", hit.kind, hit.title)).size(11))
                    .on_press(Message::UseSuggest(index)),
            );
        }
        below = below.push(suggestions);
    }
    for issue in app.lint.iter().take(6) {
        let color = if issue.severity == "error" {
            DANGER
        } else {
            Color::from_rgb8(0xb4, 0x53, 0x09)
        };
        below = below.push(
            text(format!("{}:{} {}", issue.line, issue.column, issue.message))
                .size(12)
                .color(color),
        );
    }
    column![row![gutter, editor].spacing(4).height(Fill), below]
        .height(Fill)
        .padding([0, 8])
        .into()
}

fn editor_text_view(app: &App) -> String {
    super::editor_text(&app.editor)
}

fn statement_word(statement: &str) -> Option<&str> {
    statement.split_whitespace().next()
}

fn results(app: &App) -> Element<'_, Message> {
    let mut cards = column![text("Results").size(13)].spacing(6);
    if let Some(json) = &app.row_json {
        cards = cards.push(
            column![
                button(text("Close row")).on_press(Message::CloseRow),
                text(json.clone()).font(iced::Font::MONOSPACE).size(12),
            ]
            .spacing(4),
        );
    }
    for (index, card) in app.results.iter().take(20).enumerate() {
        cards = cards.push(card_view(app, index, card));
    }
    container(scrollable(cards).height(Fill))
        .height(Length::Fixed(app.prefs.results))
        .padding(8)
        .into()
}

fn card_view<'a>(app: &'a App, index: usize, card: &'a StatementCard) -> Element<'a, Message> {
    let flag = if card.ok { "ok" } else { "failed" };
    let mut head = row![
        button(text(
            if app.open_cards.get(index).copied().unwrap_or(false) {
                "●"
            } else {
                "○"
            }
        ))
        .on_press(Message::ToggleCard(index))
        .padding(2),
        text(format!("{flag}  {}", card.kind)).size(12),
        text(format!("{} ms", card.elapsed_ms)).size(12),
    ]
    .spacing(8);
    if card.read_units > 0.0 {
        head = head.push(text(format!("RCU {:.1}", card.read_units)).size(12));
    }
    if card.write_units > 0.0 {
        head = head.push(text(format!("WCU {:.1}", card.write_units)).size(12));
    }
    let mut body = column![head].spacing(4);
    if app.open_cards.get(index).copied().unwrap_or(false) {
        body = body.push(
            text(card.statement.clone())
                .font(iced::Font::MONOSPACE)
                .size(12),
        );
    }
    if !card.ok {
        body = body.push(text(card.failure_text()).size(12).color(DANGER));
    } else if !card.message.is_empty() && card.items.is_empty() {
        body = body.push(text(card.message.clone()).size(12));
    }
    if let Some(help) = &card.help {
        let pretty = serde_json::to_string_pretty(help).unwrap_or_else(|_| help.to_string());
        body = body.push(text(pretty).font(iced::Font::MONOSPACE).size(12));
    }
    if !card.items.is_empty() {
        body = body.push(items_table(&card.columns, &card.items, true, false));
    }
    container(body).padding(6).width(Fill).into()
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
        header = header.push(text(name.clone()).size(12));
    }
    let mut rows = column![].spacing(2);
    for (row_index, item) in items.iter().take(50).enumerate() {
        let mut line = row![].spacing(8);
        for name in &names {
            let cell = item.get(name).map(value_text).unwrap_or_default();
            line = line.push(text(cell).size(12).font(iced::Font::MONOSPACE));
        }
        if clickable {
            let json = serde_json::to_string_pretty(item).unwrap_or_default();
            rows = rows.push(
                button(line)
                    .on_press(Message::ShowRow(json))
                    .padding(2)
                    .style(button::text),
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

fn tables(app: &App) -> Element<'_, Message> {
    let mut list = column![
        row![
            text_input("Filter tables", &app.table_filter).on_input(Message::TableFilter),
            button(text("Refresh")).on_press(Message::RefreshTables(true)),
            button(text("Clear cache")).on_press(Message::ClearCache),
        ]
        .spacing(8),
        text(if app.cache_text.is_empty() {
            app.table_note.clone()
        } else {
            format!("{}  {}", app.table_note, app.cache_text)
        })
        .size(12),
    ]
    .spacing(4);
    for table in &app.tables {
        let keys = table
            .keys
            .iter()
            .map(|key| format!("{} {}", key.role, key.name))
            .collect::<Vec<_>>()
            .join(", ");
        list = list.push(
            button(text(format!("{}   {keys}", table.name)).size(13))
                .on_press(Message::OpenTable(table.name.clone()))
                .width(Fill),
        );
    }
    let mut page = column![
        text(if app.open_table.is_empty() {
            "Select a table".to_string()
        } else {
            format!("{}  page {}", app.open_table, app.table_page + 1)
        })
        .size(14),
        row![
            button(text("Previous")).on_press(Message::TablePage(-1)),
            button(text("Next")).on_press(Message::TablePage(1)),
            text(if app.table_has_more {
                "more rows"
            } else {
                "end"
            })
            .size(12),
        ]
        .spacing(8),
    ]
    .spacing(6);
    if !app.table_items.is_empty() {
        page = page.push(items_table(
            &app.table_columns,
            &app.table_items,
            true,
            true,
        ));
    }
    row![
        scrollable(list).width(Length::Fixed(360.0)).height(Fill),
        scrollable(page).width(Fill).height(Fill),
    ]
    .spacing(8)
    .padding(8)
    .height(Fill)
    .into()
}

fn status_bar(app: &App) -> Element<'_, Message> {
    text(if app.status.is_empty() {
        "dqlrs-ui".to_string()
    } else {
        app.status.clone()
    })
    .size(12)
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
