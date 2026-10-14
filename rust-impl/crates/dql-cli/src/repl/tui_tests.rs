//! In-memory TUI coverage for the REPL.
//!
//! Ratatui's [`TestBackend`](ratatui::backend::TestBackend) renders into a
//! fixed buffer, so these tests check layout, styles, and editor transitions
//! without a real terminal. Crossterm key events are applied through the same
//! `on_enter` / insert path the event loop uses.

use super::{
    buffer_submits_on_enter, command_panel_lines, draw, live_panel_bg, status_panel_bg,
    status_panel_fg, ReplApp,
};
use crate::session::Session;
use dql_output::build_rich_layout;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};
use ratatui::Terminal;

fn session() -> Session {
    let mut session = Session::new_memory_headless("us-west-1");
    session.config.format = "rich".to_string();
    session
}

fn type_str(app: &mut ReplApp<'_>, text: &str) {
    for ch in text.chars() {
        app.insert_char(ch);
    }
}

fn render_app(app: &ReplApp<'_>, width: u16, height: u16) -> (TestBackend, Position) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| draw(frame, app)).unwrap();
    let cursor = terminal.get_cursor_position().unwrap();
    let backend = terminal.backend().clone();
    (backend, cursor)
}

fn line_text(backend: &TestBackend, y: u16) -> String {
    let width = backend.buffer().area.width;
    (0..width)
        .map(|x| backend.buffer()[(x, y)].symbol())
        .collect()
}

fn screen_text(backend: &TestBackend) -> String {
    (0..backend.buffer().area.height)
        .map(|y| line_text(backend, y))
        .collect::<Vec<_>>()
        .join("\n")
}

fn fg_at(backend: &TestBackend, needle: &str) -> Option<Color> {
    style_at(backend, needle).and_then(|style| style.fg)
}

fn bg_at(backend: &TestBackend, needle: &str) -> Option<Color> {
    style_at(backend, needle).and_then(|style| style.bg)
}

fn style_at(backend: &TestBackend, needle: &str) -> Option<Style> {
    let height = backend.buffer().area.height;
    for y in 0..height {
        let line = line_text(backend, y);
        if let Some(index) = line.find(needle) {
            return Some(backend.buffer()[(index as u16, y)].style());
        }
    }
    None
}

fn paint_lines(lines: Vec<Line<'static>>, width: u16) -> TestBackend {
    let height = u16::try_from(lines.len()).unwrap_or(1).max(1);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            frame.render_widget(Paragraph::new(lines.clone()), frame.area());
        })
        .unwrap();
    terminal.backend().clone()
}

fn screen_of_output(app: &ReplApp<'_>) -> String {
    if app.output_lines.is_empty() {
        return String::new();
    }
    screen_text(&paint_lines(app.output_lines.clone(), 80))
}

#[test]
fn enter_submits_sql_only_when_terminated() {
    assert!(!buffer_submits_on_enter(""));
    assert!(!buffer_submits_on_enter("SELECT name"));
    assert!(!buffer_submits_on_enter("SELECT name\nFROM t"));
    assert!(buffer_submits_on_enter("SELECT name FROM t;"));
    assert!(buffer_submits_on_enter("SELECT name\nFROM t;"));
    assert!(buffer_submits_on_enter("ls"));
    assert!(buffer_submits_on_enter("  opt format rich"));
    assert!(buffer_submits_on_enter("help;"));
    assert!(!buffer_submits_on_enter("SCAN * FROM t"));
}

#[test]
fn enter_without_semicolon_inserts_a_newline() {
    let mut session = session();
    let mut app = ReplApp::new(&mut session);
    type_str(&mut app, "SELECT name, kind");
    assert!(!app.on_enter().unwrap());
    assert_eq!(
        app.lines,
        vec!["SELECT name, kind".to_string(), String::new()]
    );
    assert!(app.output_lines.is_empty());
    assert!(app.committed_command.is_empty());
    assert_eq!(app.row, 1);
    assert_eq!(app.col, 0);

    type_str(&mut app, "FROM sample_cosmos");
    let (backend, cursor) = render_app(&app, 60, 8);
    let screen = screen_text(&backend);
    assert!(screen.contains("SELECT name, kind"), "{screen}");
    assert!(screen.contains("FROM sample_cosmos"), "{screen}");
    assert!(screen.contains("us-west-1"), "{screen}");
    assert_eq!(fg_at(&backend, "SELECT"), Some(Color::Magenta));
    assert_eq!(fg_at(&backend, "FROM"), Some(Color::Magenta));
    assert_eq!(cursor.y, 3);
    assert_eq!(cursor.x, u16::try_from("FROM sample_cosmos".len()).unwrap());
    assert_eq!(backend.buffer()[(0, 1)].style().bg, Some(live_panel_bg()));
}

#[test]
fn paste_keeps_newlines_until_semicolon_submit() {
    let mut session = session();
    let mut app = ReplApp::new(&mut session);
    app.insert_paste("SELECT name\nFROM planets");
    assert_eq!(app.lines.len(), 2);
    assert!(app.output_lines.is_empty());
    assert!(!app.on_enter().unwrap());
    assert_eq!(app.lines.len(), 3);

    app.insert_paste("WHERE id = 'a';");
    assert!(app.on_enter().unwrap());
    assert!(!app.command_ok);
    let screen = screen_of_output(&app);
    assert!(screen.contains("query failed"), "{screen}");
}

#[test]
fn semicolon_runs_query_and_paints_rich_results() {
    let mut session = session();
    let mut app = ReplApp::new(&mut session);
    type_str(
        &mut app,
        "CREATE TABLE planets (id STRING HASH KEY, name STRING);",
    );
    assert!(app.on_enter().unwrap());
    assert!(app.command_ok, "{}", screen_of_output(&app));

    type_str(
        &mut app,
        "INSERT INTO planets (id, name) VALUES ('a', 'Earth');",
    );
    assert!(app.on_enter().unwrap());
    assert!(app.command_ok, "{}", screen_of_output(&app));

    type_str(&mut app, "SELECT name FROM planets");
    assert!(!app.on_enter().unwrap());
    type_str(&mut app, "WHERE id = 'a';");
    assert!(app.on_enter().unwrap());
    assert!(app.command_ok, "{}", screen_of_output(&app));
    assert_eq!(
        app.committed_command,
        vec![
            "SELECT name FROM planets".to_string(),
            "WHERE id = 'a';".to_string(),
        ]
    );
    assert_eq!(app.lines, vec![String::new()]);

    let painted = paint_lines(app.output_lines.clone(), 40);
    let screen = screen_text(&painted);
    assert!(screen.contains("Results"), "{screen}");
    assert!(screen.contains("name"), "{screen}");
    assert!(screen.contains("Earth"), "{screen}");
    assert_eq!(fg_at(&painted, "Results"), Some(Color::Rgb(18, 32, 43)));
    assert_eq!(bg_at(&painted, "Results"), Some(Color::Rgb(255, 255, 255)));
    assert_eq!(bg_at(&painted, "Earth"), Some(Color::Rgb(255, 255, 255)));
    assert!(app
        .session
        .history
        .entries()
        .iter()
        .any(|entry| entry.contains("Earth")));
}

#[test]
fn failed_query_uses_the_error_panel_colors() {
    let mut session = session();
    let mut app = ReplApp::new(&mut session);
    type_str(&mut app, "SELECT name FROM missing;");
    assert!(app.on_enter().unwrap());
    assert!(!app.command_ok);
    let painted = paint_lines(app.output_lines.clone(), 72);
    let screen = screen_text(&painted);
    assert!(screen.contains("query failed"), "{screen}");
    assert_eq!(fg_at(&painted, "query failed"), Some(Color::Red));

    let panel = command_panel_lines(
        "(memory)",
        Some("error"),
        &["SELECT name FROM missing;".to_string()],
        status_panel_fg(false),
        40,
    );
    let area = Rect::new(0, 0, 40, 4);
    let mut buffer = Buffer::empty(area);
    Paragraph::new(panel)
        .style(Style::default().bg(status_panel_bg(false)))
        .render(area, &mut buffer);
    let title: String = (0..area.width).map(|x| buffer[(x, 1)].symbol()).collect();
    assert!(title.contains("error"), "{title}");
    assert_eq!(buffer[(0, 1)].style().bg, Some(status_panel_bg(false)));
    assert_eq!(buffer[(0, 1)].style().fg, Some(Color::Red));
    assert_eq!(buffer[(0, 2)].symbol(), "S");
    assert_eq!(buffer[(0, 2)].style().fg, Some(Color::Magenta));
}

#[test]
fn meta_commands_run_without_a_semicolon() {
    let mut session = session();
    let mut app = ReplApp::new(&mut session);
    type_str(&mut app, "ls");
    assert!(app.on_enter().unwrap());
    assert!(app.command_ok);
    let screen = screen_of_output(&app);
    assert!(
        screen.contains("Tables") || screen.contains("Name"),
        "{screen}"
    );

    type_str(&mut app, "help");
    assert!(app.on_enter().unwrap());
    let screen = screen_of_output(&app);
    assert!(screen.contains("Available commands"), "{screen}");
}

#[test]
fn backspace_merges_a_wrapped_line() {
    let mut session = session();
    let mut app = ReplApp::new(&mut session);
    type_str(&mut app, "SELECT");
    assert!(!app.on_enter().unwrap());
    type_str(&mut app, "name");
    app.col = 0;
    app.backspace();
    assert_eq!(app.lines, vec!["SELECTname".to_string()]);
    assert_eq!(app.row, 0);
}

#[test]
fn empty_result_layout_says_no_results() {
    let layout = build_rich_layout(&[], None);
    let lines = super::super::rich_table::rich_layout_to_lines(&layout, 40);
    let painted = paint_lines(lines, 20);
    assert!(screen_text(&painted).contains("No results"));
}

#[test]
fn highlight_styles_survive_a_widget_buffer() {
    let line = super::super::highlight::highlight_line(
        "SELECT name FROM t WHERE id = 'abc' LIMIT 2; -- note",
    );
    let area = Rect::new(0, 0, 80, 1);
    let mut buffer = Buffer::empty(area);
    Paragraph::new(line).render(area, &mut buffer);
    assert_eq!(buffer[(0, 0)].symbol(), "S");
    assert_eq!(buffer[(0, 0)].style().fg, Some(Color::Magenta));
    let text: String = (0..area.width).map(|x| buffer[(x, 0)].symbol()).collect();
    let quote = text.find('\'').unwrap() as u16;
    assert_eq!(buffer[(quote, 0)].style().fg, Some(Color::Green));
    let two = text.find('2').unwrap() as u16;
    assert_eq!(buffer[(two, 0)].style().fg, Some(Color::Yellow));
    let dashes = text.find("--").unwrap() as u16;
    assert_eq!(buffer[(dashes, 0)].style().fg, Some(Color::DarkGray));
}

#[test]
fn live_panel_text_matches_the_editor() {
    let mut session = session();
    let mut app = ReplApp::new(&mut session);
    type_str(&mut app, "SCAN * FROM planets;");
    let (backend, cursor) = render_app(&app, 36, 5);
    let lines: Vec<String> = (0..5)
        .map(|y| line_text(&backend, y).trim_end().to_string())
        .collect();
    assert_eq!(
        lines,
        vec![
            "─".repeat(36),
            "us-west-1".to_string(),
            "SCAN * FROM planets;".to_string(),
            String::new(),
            String::new(),
        ]
    );
    assert_eq!(cursor, Position::new(20, 2));
}

#[test]
fn history_up_restores_a_multiline_query_into_the_editor() {
    let mut session = session();
    let mut app = ReplApp::new(&mut session);
    app.insert_paste("SELECT name\nFROM planets;");
    assert!(app.on_enter().unwrap());
    app.history_up();
    assert_eq!(
        app.lines,
        vec!["SELECT name".to_string(), "FROM planets;".to_string()]
    );
    let (backend, cursor) = render_app(&app, 24, 6);
    let screen = screen_text(&backend);
    assert!(screen.contains("SELECT name"), "{screen}");
    assert!(screen.contains("FROM planets;"), "{screen}");
    assert_eq!(cursor.y, 3);
}

#[test]
fn clear_resets_the_editor_without_a_semicolon() {
    let mut session = session();
    let mut app = ReplApp::new(&mut session);
    type_str(&mut app, "clear");
    assert!(app.on_enter().unwrap());
    assert_eq!(app.lines, vec![String::new()]);
    assert!(app.output_lines.is_empty());
    assert!(app.committed_command.is_empty());
    assert!(app.take_scroll_to_top());
}

#[test]
fn clear_scrolls_the_visible_screen_into_scrollback() {
    let mut terminal = Terminal::new(TestBackend::new(20, 6)).unwrap();
    terminal
        .draw(|frame| {
            frame.render_widget(Paragraph::new("previous-history"), frame.area());
        })
        .unwrap();
    super::scroll_visible_screen_into_scrollback(&mut terminal).unwrap();

    let saved = buffer_plain(terminal.backend().scrollback());
    let visible = buffer_plain(terminal.backend().buffer());
    assert!(
        saved.contains("previous-history"),
        "scrollback lost the screen:\n{saved}"
    );
    assert!(
        !visible.contains("previous-history"),
        "visible screen still shows cleared text:\n{visible}"
    );
    assert_eq!(terminal.get_cursor_position().unwrap(), Position::new(0, 0));

    let mut parked = Terminal::with_options(
        terminal.backend().clone(),
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Inline(4),
        },
    )
    .unwrap();
    assert_eq!(parked.get_frame().area().y, 0);
}

fn buffer_plain(buffer: &Buffer) -> String {
    let width = buffer.area.width;
    (0..buffer.area.height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
