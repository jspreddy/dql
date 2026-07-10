use crate::meta::lifecycle::{take_exit_request, take_history_edit_request};
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
    if let Some((editor, path)) = take_history_edit_request() {
        let status = std::process::Command::new(&editor)
            .arg(&path)
            .status()
            .map_err(|err| format!("Failed to open history with {editor:?}: {err}"))?;
        if !status.success() {
            return Err(format!(
                "Editor {editor:?} exited with status {status} while editing {}",
                path.display()
            )
            .into());
        }
    }
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
    let command_lines = std::mem::take(&mut app.committed_command);
    let output = std::mem::take(&mut app.output_lines);
    let ok = app.command_ok;
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
            Event::Key(key) => {
                let alt = key.modifiers.contains(KeyModifiers::ALT);
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.session.engine.reset_fragment();
                        app.output_lines.clear();
                        app.command_ok = true;
                        app.reset_buffer();
                        app.partial = false;
                        needs_redraw = true;
                    }
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        break;
                    }
                    KeyCode::Enter => {
                        app.history_index = None;
                        app.handle_submit()?;
                        if !app.partial {
                            ensure_inline_height(terminal, inline_height, panel_height(1))?;
                            flush_to_scrollback(terminal, &mut app)?;
                        }
                        needs_redraw = true;
                        if take_exit_request() {
                            break;
                        }
                        #[cfg(feature = "watch")]
                        if let Some(tables) = take_watch_request() {
                            let watch_result =
                                crate::meta::watch::run_monitor(app.session, &tables);
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
                    KeyCode::Left if alt => {
                        app.move_word_left();
                        needs_redraw = true;
                    }
                    KeyCode::Right if alt => {
                        app.move_word_right();
                        needs_redraw = true;
                    }
                    // macOS Option often sends Meta+b / Meta+f for word motion.
                    KeyCode::Char('b') if alt => {
                        app.move_word_left();
                        needs_redraw = true;
                    }
                    KeyCode::Char('f') if alt => {
                        app.move_word_right();
                        needs_redraw = true;
                    }
                    KeyCode::Up if alt => {
                        app.move_line_up();
                        needs_redraw = true;
                    }
                    KeyCode::Down if alt => {
                        app.move_line_down();
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
                        app.col = 0;
                        needs_redraw = true;
                    }
                    KeyCode::End => {
                        app.cursor_to_end_of_line();
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
                }
            }
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
    /// Editable lines of the in-progress command.
    lines: Vec<String>,
    /// Caret row into `lines`.
    row: usize,
    /// Caret column as a Unicode scalar index into the current line.
    col: usize,
    /// Command lines captured for the scrollback panel on successful submit.
    committed_command: Vec<String>,
    /// Result lines for the statement currently being committed.
    output_lines: Vec<Line<'static>>,
    /// Whether the statement about to be flushed succeeded.
    command_ok: bool,
    partial: bool,
    history_index: Option<usize>,
}

impl<'a> ReplApp<'a> {
    fn new(session: &'a mut Session) -> Self {
        Self {
            session,
            lines: vec![String::new()],
            row: 0,
            col: 0,
            committed_command: Vec::new(),
            output_lines: Vec::new(),
            command_ok: true,
            partial: false,
            history_index: None,
        }
    }

    fn live_content_line_count(&self) -> usize {
        self.lines.len().max(1)
    }

    fn current_line(&self) -> &str {
        self.lines.get(self.row).map(String::as_str).unwrap_or("")
    }

    fn current_line_mut(&mut self) -> &mut String {
        if self.row >= self.lines.len() {
            self.row = self.lines.len().saturating_sub(1);
        }
        &mut self.lines[self.row]
    }

    fn clamp_caret(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        if self.row >= self.lines.len() {
            self.row = self.lines.len() - 1;
        }
        let len = self.lines[self.row].chars().count();
        if self.col > len {
            self.col = len;
        }
    }

    fn reset_buffer(&mut self) {
        self.lines = vec![String::new()];
        self.row = 0;
        self.col = 0;
    }

    fn cursor_to_end_of_line(&mut self) {
        self.col = self.current_line().chars().count();
    }

    fn move_cursor_left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.cursor_to_end_of_line();
        }
    }

    fn move_cursor_right(&mut self) {
        let len = self.current_line().chars().count();
        if self.col < len {
            self.col += 1;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
    }

    fn move_line_up(&mut self) {
        if self.row > 0 {
            self.row -= 1;
            self.clamp_caret();
        }
    }

    fn move_line_down(&mut self) {
        if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.clamp_caret();
        }
    }

    fn move_word_left(&mut self) {
        self.clamp_caret();
        if self.col == 0 {
            if self.row > 0 {
                self.row -= 1;
                self.cursor_to_end_of_line();
            }
            return;
        }
        let chars: Vec<char> = self.current_line().chars().collect();
        let mut i = self.col.min(chars.len());
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        self.col = i;
    }

    fn move_word_right(&mut self) {
        self.clamp_caret();
        let chars: Vec<char> = self.current_line().chars().collect();
        let len = chars.len();
        if self.col >= len {
            if self.row + 1 < self.lines.len() {
                self.row += 1;
                self.col = 0;
            }
            return;
        }
        let mut i = self.col;
        while i < len && !chars[i].is_whitespace() {
            i += 1;
        }
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }
        self.col = i;
    }

    fn byte_index_at_col(&self) -> usize {
        self.current_line()
            .char_indices()
            .nth(self.col)
            .map(|(i, _)| i)
            .unwrap_or(self.current_line().len())
    }

    fn insert_char(&mut self, ch: char) {
        let idx = self.byte_index_at_col();
        self.current_line_mut().insert(idx, ch);
        self.col += 1;
    }

    fn backspace(&mut self) {
        if self.col > 0 {
            self.col -= 1;
            let idx = self.byte_index_at_col();
            self.current_line_mut().remove(idx);
            return;
        }
        if self.row == 0 {
            return;
        }
        // Merge with previous line.
        let current = self.lines.remove(self.row);
        self.row -= 1;
        self.col = self.lines[self.row].chars().count();
        self.lines[self.row].push_str(&current);
    }

    fn delete_forward(&mut self) {
        let len = self.current_line().chars().count();
        if self.col < len {
            let idx = self.byte_index_at_col();
            self.current_line_mut().remove(idx);
            return;
        }
        if self.row + 1 >= self.lines.len() {
            return;
        }
        let next = self.lines.remove(self.row + 1);
        self.lines[self.row].push_str(&next);
    }

    /// Load a history entry into the live panel (multi-line aware).
    fn apply_history_entry(&mut self, entry: String) {
        self.session.engine.reset_fragment();
        self.partial = false;
        let mut lines: Vec<String> = entry.lines().map(str::to_string).collect();
        if lines.is_empty() {
            lines.push(entry);
        }
        if lines.is_empty() {
            lines.push(String::new());
        }
        self.lines = lines;
        self.row = self.lines.len() - 1;
        self.cursor_to_end_of_line();
    }

    fn push_error(&mut self, err: impl ToString) {
        self.command_ok = false;
        self.output_lines.push(Line::from(Span::styled(
            err.to_string(),
            Style::default().fg(Color::Red),
        )));
    }

    fn command_text(&self) -> String {
        self.lines.join("\n")
    }

    fn trimmed_command_lines(&self) -> Vec<String> {
        let mut lines = self.lines.clone();
        while lines.last().is_some_and(|l| l.is_empty()) && lines.len() > 1 {
            lines.pop();
        }
        lines
    }

    fn record_history_if_complete(&mut self) {
        let lines = self.trimmed_command_lines();
        let full = lines.join("\n");
        if crate::history::should_record_history(&full) {
            self.session.history.add_entry(full);
        }
    }

    fn handle_submit(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let execute_text = self.command_text();
        let first = execute_text
            .lines()
            .next()
            .unwrap_or("")
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();

        self.output_lines.clear();
        self.command_ok = true;

        // Always re-run the full editable buffer so any line can be corrected.
        self.session.engine.reset_fragment();

        let output_config = self.session.config.output_config();
        let mut backend = BufferBackend::default();
        let rich_context = self.session.engine.rich_context();
        if output_config.format == OutputFormat::Rich && first == "ls" {
            let arglist = execute_text
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
            match crate::meta::dispatch(self.session, &execute_text, writer.as_mut(), true) {
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

        if matches!(first.as_str(), "clear" | "cls" | "c") {
            self.committed_command.clear();
            self.output_lines.clear();
            self.command_ok = true;
            self.partial = false;
            self.reset_buffer();
            return Ok(());
        }

        if let Ok(text) = String::from_utf8(backend.buffer) {
            for line in text.lines() {
                self.output_lines.push(Line::from(line.to_string()));
            }
        }

        self.partial = self.session.engine.partial();
        if self.partial {
            // Keep editing; ensure a trailing blank line for the next fragment.
            if !self.lines.last().is_some_and(|l| l.is_empty()) {
                self.lines.push(String::new());
            }
            self.row = self.lines.len() - 1;
            self.col = 0;
        } else {
            self.record_history_if_complete();
            self.committed_command = self.trimmed_command_lines();
            self.reset_buffer();
        }
        Ok(())
    }

    fn history_up(&mut self) {
        let len = self.session.history.entries().len();
        if len == 0 {
            return;
        }
        let index = self.history_index.unwrap_or(len).saturating_sub(1);
        if let Some(entry) = self.session.history.entries().get(index) {
            self.apply_history_entry(entry.clone());
            self.history_index = Some(index);
        }
    }

    fn history_down(&mut self) {
        let len = self.session.history.entries().len();
        let Some(index) = self.history_index else {
            return;
        };
        if index + 1 >= len {
            self.reset_buffer();
            self.session.engine.reset_fragment();
            self.partial = false;
            self.history_index = None;
        } else if let Some(entry) = self.session.history.entries().get(index + 1) {
            self.apply_history_entry(entry.clone());
            self.history_index = Some(index + 1);
        }
    }

    fn complete(&mut self) {
        let tables = with_tables(self.session);
        let current = self.current_line().to_string();
        let lower = current.to_ascii_lowercase();
        for token in ["from", "into", "table", "update", "dump"] {
            if lower.contains(token) {
                if let Some(table) = tables.iter().find(|name| name.starts_with(&current)) {
                    *self.current_line_mut() = format!("{table} ");
                    self.cursor_to_end_of_line();
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
    command_panel_lines(&title, None, &app.lines, Color::Cyan)
}

fn draw(frame: &mut Frame, app: &ReplApp<'_>) {
    let area = frame.area();
    let lines = live_panel_lines(app);
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(live_panel_bg())),
        area,
    );

    // Caret on the active edit row (below top pad + title).
    if area.width > 0 && area.height > 2 {
        let row = (2 + app.row as u16).min(area.height.saturating_sub(1));
        let col = (app.col as u16).min(area.width.saturating_sub(1));
        frame.set_cursor_position(Position {
            x: area.x.saturating_add(col),
            y: area.y.saturating_add(row),
        });
    }
}
