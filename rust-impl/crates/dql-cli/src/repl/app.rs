use crate::meta::lifecycle::take_exit_request;
#[cfg(feature = "watch")]
use crate::meta::watch::take_watch_request;
use crate::session::Session;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
#[cfg(feature = "watch")]
use crossterm::terminal::enable_raw_mode;
use dql_engine::StatementResult;
use dql_output::{build_rich_layout, render_result, DisplayBackend, OutputFormat};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Position;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use ratatui::{Frame, Terminal, TerminalOptions, Viewport};
use std::io::{self, Stdout, Write};
use std::time::Duration;

use super::highlight::highlight_command;
use super::rich_table::rich_layout_to_lines;

type ReplTerminal = Terminal<CrosstermBackend<Stdout>>;

/// Title row plus top/bottom padding rows inside the panel.
const PANEL_CHROME: u16 = 3;

#[derive(Default)]
struct BufferBackend {
    buffer: Vec<u8>,
}

struct BufferWriter<'a>(&'a mut Vec<u8>);

impl Write for BufferWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl dql_output::DisplayBackend for BufferBackend {
    fn writer(&mut self) -> Box<dyn Write + '_> {
        Box::new(BufferWriter(&mut self.buffer))
    }
}

pub fn run_repl(session: &mut Session) -> Result<(), Box<dyn std::error::Error>> {
    let mut inline_height = PANEL_CHROME + 1;
    let mut terminal = ratatui::init_with_options(TerminalOptions {
        viewport: Viewport::Inline(inline_height),
    });
    let result = repl_loop(session, &mut terminal, &mut inline_height);
    ratatui::restore();
    session.history.try_to_write_history();
    result
}

fn inline_terminal(height: u16) -> io::Result<ReplTerminal> {
    Terminal::with_options(
        CrosstermBackend::new(stdout_handle()),
        TerminalOptions {
            viewport: Viewport::Inline(height.max(1)),
        },
    )
}

fn stdout_handle() -> Stdout {
    io::stdout()
}

fn ensure_inline_height(
    terminal: &mut ReplTerminal,
    inline_height: &mut u16,
    height: u16,
) -> io::Result<bool> {
    let height = height.max(PANEL_CHROME + 1);
    if *inline_height == height {
        return Ok(false);
    }
    // Erase the current inline area before replacing the viewport. Shrinking
    // without this leaves the old panel title/chrome in the main scrollback.
    terminal.clear()?;
    *inline_height = height;
    *terminal = inline_terminal(height)?;
    Ok(true)
}

fn terminal_width() -> usize {
    crossterm::terminal::size()
        .map(|(width, _)| width as usize)
        .unwrap_or(80)
        .max(40)
}

fn panel_height(content_lines: usize) -> u16 {
    let lines = content_lines.max(1) as u16;
    lines.saturating_add(PANEL_CHROME)
}

fn live_panel_bg() -> Color {
    Color::Rgb(15, 35, 45)
}

fn status_panel_bg(ok: bool) -> Color {
    if ok {
        Color::Rgb(10, 40, 20)
    } else {
        Color::Rgb(50, 15, 15)
    }
}

fn status_panel_fg(ok: bool) -> Color {
    if ok {
        Color::Green
    } else {
        Color::Red
    }
}

fn panel_title_line(title: &str, status: Option<&str>, fg: Color) -> Line<'static> {
    let text = match status {
        Some(label) => format!("{title} · {label}"),
        None => title.to_string(),
    };
    Line::from(Span::styled(
        text,
        Style::default().fg(fg).add_modifier(Modifier::BOLD),
    ))
}

fn command_panel_lines(
    title: &str,
    status: Option<&str>,
    command_lines: &[String],
    title_fg: Color,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(""), // top padding
        panel_title_line(title, status, title_fg),
    ];
    lines.extend(highlight_command(command_lines));
    lines.push(Line::from("")); // bottom padding
    lines
}

/// Push a blank separator, the consolidated command panel, then result lines
/// into terminal scrollback.
fn flush_to_scrollback(terminal: &mut ReplTerminal, app: &mut ReplApp<'_>) -> io::Result<()> {
    let command_lines = std::mem::take(&mut app.pending_command);
    let output = std::mem::take(&mut app.output_lines);
    let ok = app.command_ok;
    app.command_start = 0;
    app.command_ok = true;

    if command_lines.is_empty() && output.is_empty() {
        return Ok(());
    }

    // Blank line before every input block.
    terminal.insert_before(1, |buf| {
        Paragraph::new(Line::from("")).render(buf.area, buf);
    })?;

    if !command_lines.is_empty() {
        let title = full_prompt(app.session);
        let status = if ok { "ok" } else { "error" };
        let lines = command_panel_lines(&title, Some(status), &command_lines, status_panel_fg(ok));
        let height = lines.len() as u16;
        let bg = status_panel_bg(ok);
        terminal.insert_before(height, |buf| {
            Paragraph::new(lines)
                .style(Style::default().bg(bg))
                .render(buf.area, buf);
        })?;
    }

    const CHUNK: usize = 256;
    for chunk in output.chunks(CHUNK) {
        let height = chunk.len() as u16;
        let chunk_lines: Vec<Line<'static>> = chunk.to_vec();
        terminal.insert_before(height, |buf| {
            Paragraph::new(chunk_lines).render(buf.area, buf);
        })?;
    }

    // Status-colored rule below the result.
    let rule_width = terminal_width().max(1);
    let rule = "─".repeat(rule_width);
    let rule_style = Style::default()
        .fg(status_panel_fg(ok))
        .add_modifier(Modifier::BOLD);
    terminal.insert_before(1, |buf| {
        Paragraph::new(Line::from(Span::styled(rule, rule_style))).render(buf.area, buf);
    })?;

    // Trailing blank line after the result block.
    terminal.insert_before(1, |buf| {
        Paragraph::new(Line::from("")).render(buf.area, buf);
    })?;
    Ok(())
}

fn repl_loop(
    session: &mut Session,
    terminal: &mut ReplTerminal,
    inline_height: &mut u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = ReplApp::new(session);
    let mut needs_redraw = true;
    loop {
        if needs_redraw {
            let height = panel_height(app.live_content_line_count());
            ensure_inline_height(terminal, inline_height, height)?;
            terminal.draw(|frame| draw(frame, &app))?;
            needs_redraw = false;
        }

        if !event::poll(Duration::from_millis(250))? {
            if take_exit_request() {
                break;
            }
            continue;
        }

        match event::read()? {
            Event::Key(key) => match key.code {
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.session.engine.reset_fragment();
                    app.pending_command.clear();
                    app.output_lines.clear();
                    app.command_ok = true;
                    app.command_start = 0;
                    app.clear_input();
                    app.partial = false;
                    needs_redraw = true;
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let _ = app.session.history.remove_items(1);
                    break;
                }
                KeyCode::Enter => {
                    let line = app.input.clone();
                    let continuation = app.partial;
                    app.history_index = None;
                    if !line.trim().is_empty() {
                        app.session.history.add_entry(line.clone());
                    }
                    app.handle_submit(line, continuation)?;
                    app.clear_input();
                    if !app.partial {
                        // Drop the live multi-line panel before committing, so
                        // its cyan title isn't left above the scrolled result.
                        ensure_inline_height(terminal, inline_height, panel_height(1))?;
                        flush_to_scrollback(terminal, &mut app)?;
                    }
                    needs_redraw = true;
                    if take_exit_request() {
                        break;
                    }
                    #[cfg(feature = "watch")]
                    if let Some(tables) = take_watch_request() {
                        let watch_result = crate::meta::watch::run_monitor(app.session, &tables);
                        enable_raw_mode()?;
                        *inline_height = panel_height(1);
                        *terminal = inline_terminal(*inline_height)?;
                        if let Err(err) = watch_result {
                            app.command_ok = false;
                            app.push_error(format!("watch error: {err}"));
                            flush_to_scrollback(terminal, &mut app)?;
                        }
                        needs_redraw = true;
                    }
                }
                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.history_up();
                    needs_redraw = true;
                }
                KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.history_down();
                    needs_redraw = true;
                }
                KeyCode::Left => {
                    app.move_cursor_left();
                    needs_redraw = true;
                }
                KeyCode::Right => {
                    app.move_cursor_right();
                    needs_redraw = true;
                }
                KeyCode::Home => {
                    app.cursor = 0;
                    needs_redraw = true;
                }
                KeyCode::End => {
                    app.cursor_to_end();
                    needs_redraw = true;
                }
                KeyCode::Up => {
                    app.history_up();
                    needs_redraw = true;
                }
                KeyCode::Down => {
                    app.history_down();
                    needs_redraw = true;
                }
                KeyCode::Tab => {
                    app.complete();
                    needs_redraw = true;
                }
                KeyCode::Backspace => {
                    app.backspace();
                    needs_redraw = true;
                }
                KeyCode::Delete => {
                    app.delete_forward();
                    needs_redraw = true;
                }
                KeyCode::Char(ch) => {
                    app.insert_char(ch);
                    needs_redraw = true;
                }
                _ => {}
            },
            Event::Resize(_, _) => {
                needs_redraw = true;
            }
            _ => {}
        }

        if take_exit_request() {
            break;
        }
    }
    Ok(())
}

struct ReplApp<'a> {
    session: &'a mut Session,
    /// Prior lines of the in-progress multi-line statement (not yet flushed).
    pending_command: Vec<String>,
    /// Result lines for the statement currently being committed.
    output_lines: Vec<Line<'static>>,
    /// Whether the statement about to be flushed succeeded.
    command_ok: bool,
    command_start: usize,
    input: String,
    /// Caret position as a Unicode scalar index into `input`.
    cursor: usize,
    partial: bool,
    history_index: Option<usize>,
}

impl<'a> ReplApp<'a> {
    fn new(session: &'a mut Session) -> Self {
        Self {
            session,
            pending_command: Vec::new(),
            output_lines: Vec::new(),
            command_ok: true,
            command_start: 0,
            input: String::new(),
            cursor: 0,
            partial: false,
            history_index: None,
        }
    }

    fn live_content_line_count(&self) -> usize {
        self.pending_command.len() + 1
    }

    fn clear_input(&mut self) {
        self.input.clear();
        self.cursor = 0;
    }

    fn cursor_to_end(&mut self) {
        self.cursor = self.input.chars().count();
    }

    fn move_cursor_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    fn move_cursor_right(&mut self) {
        if self.cursor < self.input.chars().count() {
            self.cursor += 1;
        }
    }

    fn byte_index_at_cursor(&self) -> usize {
        self.input
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.input.len())
    }

    fn insert_char(&mut self, ch: char) {
        let idx = self.byte_index_at_cursor();
        self.input.insert(idx, ch);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
        let idx = self.byte_index_at_cursor();
        self.input.remove(idx);
    }

    fn delete_forward(&mut self) {
        if self.cursor >= self.input.chars().count() {
            return;
        }
        let idx = self.byte_index_at_cursor();
        self.input.remove(idx);
    }

    fn set_input(&mut self, text: String) {
        self.input = text;
        self.cursor_to_end();
    }

    fn push_error(&mut self, err: impl ToString) {
        self.command_ok = false;
        self.output_lines.push(Line::from(Span::styled(
            err.to_string(),
            Style::default().fg(Color::Red),
        )));
    }

    fn handle_submit(
        &mut self,
        line: String,
        continuation: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cmd = line
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();

        if !continuation {
            self.command_start = self.pending_command.len();
            self.output_lines.clear();
            self.command_ok = true;
        }
        if !line.is_empty() || continuation {
            self.pending_command.push(line.clone());
        }

        let output_config = self.session.config.output_config();
        let mut backend = BufferBackend::default();
        let rich_context = self.session.engine.rich_context();
        if output_config.format == OutputFormat::Rich && cmd == "ls" {
            let arglist = line
                .split_once(char::is_whitespace)
                .map(|(_, rest)| rest.trim())
                .unwrap_or("");
            let (args, kwargs) = crate::meta::parse_repl_args(arglist);
            match crate::meta::ls::render_rich_lines(
                self.session,
                &args,
                &kwargs,
                terminal_width() as u16,
            ) {
                Ok(lines) => self.output_lines.extend(lines),
                Err(err) => self.push_error(err),
            }
        } else {
            let mut writer = backend.writer();
            match crate::meta::dispatch(self.session, &line, writer.as_mut(), true) {
                Ok(Some(result)) => {
                    drop(writer);
                    let render = if output_config.format == OutputFormat::Rich {
                        if let StatementResult::Items(items) = &result {
                            let layout = build_rich_layout(items, rich_context.as_ref());
                            self.output_lines
                                .extend(rich_layout_to_lines(&layout, terminal_width() as u16));
                            Ok(())
                        } else {
                            render_result(
                                &result,
                                &output_config,
                                &mut backend,
                                rich_context.as_ref(),
                            )
                        }
                    } else {
                        render_result(&result, &output_config, &mut backend, rich_context.as_ref())
                    };
                    if let Err(err) = render {
                        self.push_error(err);
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    drop(writer);
                    self.push_error(err);
                }
            }
        }
        if matches!(cmd.as_str(), "clear" | "cls" | "c") {
            self.pending_command.clear();
            self.output_lines.clear();
            self.command_start = 0;
            self.command_ok = true;
            self.partial = false;
            return Ok(());
        }
        if let Ok(text) = String::from_utf8(backend.buffer) {
            for line in text.lines() {
                self.output_lines.push(Line::from(line.to_string()));
            }
        }
        self.partial = self.session.engine.partial();
        Ok(())
    }

    fn history_up(&mut self) {
        let len = self.session.history.entries().len();
        if len == 0 {
            return;
        }
        let index = self.history_index.unwrap_or(len).saturating_sub(1);
        if let Some(entry) = self.session.history.entries().get(index) {
            self.set_input(entry.clone());
            self.history_index = Some(index);
        }
    }

    fn history_down(&mut self) {
        let len = self.session.history.entries().len();
        let Some(index) = self.history_index else {
            return;
        };
        if index + 1 >= len {
            self.clear_input();
            self.history_index = None;
        } else if let Some(entry) = self.session.history.entries().get(index + 1) {
            self.set_input(entry.clone());
            self.history_index = Some(index + 1);
        }
    }

    fn complete(&mut self) {
        let tables = with_tables(self.session);
        let lower = self.input.to_ascii_lowercase();
        for token in ["from", "into", "table", "update", "dump"] {
            if lower.contains(token) {
                if let Some(table) = tables.iter().find(|name| name.starts_with(&self.input)) {
                    self.set_input(format!("{table} "));
                }
                return;
            }
        }
    }
}

fn with_tables(session: &Session) -> Vec<String> {
    let mut tables = Vec::new();
    if let Some(engine) = session.engine.with_memory(|fragment| {
        fragment
            .inner()
            .cached_descriptions
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    }) {
        tables = engine;
    } else if let Some(engine) = session.engine.with_remote(|fragment| {
        fragment
            .inner()
            .cached_descriptions
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    }) {
        tables = engine;
    }
    tables.sort();
    tables
}

fn full_prompt(session: &Session) -> String {
    if let Some((host, port)) = &session.local_endpoint {
        format!("({host}:{port}) {}", session.region)
    } else {
        session.region.clone()
    }
}

fn live_panel_lines(app: &ReplApp<'_>) -> Vec<Line<'static>> {
    let title = full_prompt(app.session);
    let mut command = app.pending_command.clone();
    command.push(app.input.clone());
    command_panel_lines(&title, None, &command, Color::Cyan)
}

fn draw(frame: &mut Frame, app: &ReplApp<'_>) {
    let area = frame.area();
    let lines = live_panel_lines(app);
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(live_panel_bg())),
        area,
    );

    // Caret on the active input row (below top pad + title).
    if area.width > 0 && area.height > 2 {
        let row = (2 + app.pending_command.len() as u16).min(area.height.saturating_sub(1));
        let col = (app.cursor as u16).min(area.width.saturating_sub(1));
        frame.set_cursor_position(Position {
            x: area.x.saturating_add(col),
            y: area.y.saturating_add(row),
        });
    }
}
