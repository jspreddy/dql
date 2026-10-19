use crate::error::report_lines;
use crate::meta::lifecycle::{take_exit_request, take_history_edit_request};
#[cfg(feature = "watch")]
use crate::meta::watch::take_watch_request;
use crate::session::Session;
use color_eyre::eyre::{self, eyre, WrapErr};
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use crossterm::execute;
#[cfg(feature = "watch")]
use crossterm::terminal::enable_raw_mode;
use dql_engine::StatementResult;
use dql_output::{build_rich_layout, render_result, DisplayBackend, OutputFormat};
use ratatui::backend::{Backend, ClearType, CrosstermBackend};
use ratatui::layout::{Position, Rect};
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

pub fn run_repl(session: &mut Session) -> eyre::Result<()> {
    let mut inline_height = PANEL_CHROME + 1;
    let mut terminal = ratatui::init_with_options(TerminalOptions {
        viewport: Viewport::Inline(inline_height),
    });
    // So multi-line pastes arrive as Event::Paste instead of synthetic Enter
    // keypresses between lines (which would submit a partial statement).
    if let Err(err) = execute!(io::stdout(), EnableBracketedPaste) {
        eprintln!("failed to enable bracketed paste: {err}");
    }
    let result = repl_loop(session, &mut terminal, &mut inline_height);
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    if let Err(err) = ratatui::try_restore() {
        eprintln!(
            "failed to restore terminal. Run `reset` or restart your terminal to recover: {err}"
        );
    }
    session.history.try_to_write_history();
    if let Some((editor, path)) = take_history_edit_request() {
        let status = std::process::Command::new(&editor)
            .arg(&path)
            .status()
            .wrap_err_with(|| format!("Failed to open history with {editor:?}"))?;
        if !status.success() {
            return Err(eyre!(
                "Editor {editor:?} exited with status {status} while editing {}",
                path.display()
            ));
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

fn screen_rows() -> u16 {
    crossterm::terminal::size()
        .map(|(_, height)| height)
        .unwrap_or(24)
        .max(1)
}

/// A wide result kept in the live viewport so left/right can slide it.
struct PinnedResult {
    lines: Vec<Line<'static>>,
    hscroll: usize,
}

impl PinnedResult {
    fn max_width(&self) -> usize {
        self.lines.iter().map(line_width).max().unwrap_or(0)
    }

    fn hscroll_clamped(&self, width: usize) -> usize {
        let max_scroll = self.max_width().saturating_sub(width);
        self.hscroll.min(max_scroll)
    }

    /// Wide rows slide by `hscroll`. Rows that already fit stay put, so the
    /// cutoff card does not scroll off the left edge.
    fn visible_lines(&self, width: usize) -> Vec<Line<'static>> {
        let scroll = self.hscroll_clamped(width);
        self.lines
            .iter()
            .map(|line| {
                if line_width(line) > width {
                    slice_line(line, scroll, width)
                } else {
                    slice_line(line, 0, width)
                }
            })
            .collect()
    }
}

fn line_width(line: &Line<'_>) -> usize {
    line.spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum()
}

fn slice_line(line: &Line<'_>, start: usize, width: usize) -> Line<'static> {
    if width == 0 {
        return Line::from("");
    }
    let mut skipped = 0usize;
    let mut taken = 0usize;
    let mut spans = Vec::new();
    for span in &line.spans {
        let len = span.content.chars().count();
        if skipped + len <= start {
            skipped += len;
            continue;
        }
        let local_start = start.saturating_sub(skipped);
        let available = len - local_start;
        let take = available.min(width.saturating_sub(taken));
        let piece: String = span.content.chars().skip(local_start).take(take).collect();
        if !piece.is_empty() {
            spans.push(Span::styled(piece, span.style));
        }
        skipped += len;
        taken += take;
        if taken >= width {
            break;
        }
    }
    Line::from(spans)
}

/// SQL submits only when the buffer ends with `;`. Without one, Enter
/// inserts a newline. Meta-commands (`ls`, `opt`, …) still submit without `;`.
fn buffer_submits_on_enter(text: &str) -> bool {
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        return false;
    }
    let first = trimmed
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches(';');
    trimmed.ends_with(';') || crate::meta::is_meta_command(first)
}

fn panel_height(content_lines: usize) -> u16 {
    let lines = content_lines.max(1) as u16;
    lines.saturating_add(PANEL_CHROME)
}

pub(crate) fn live_panel_bg() -> Color {
    Color::Rgb(15, 35, 45)
}

pub(crate) fn status_panel_bg(ok: bool) -> Color {
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

fn panel_top_border(width: usize, color: Color) -> Line<'static> {
    let rule = "─".repeat(width.max(1));
    Line::from(Span::styled(
        rule,
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    ))
}

fn command_panel_lines(
    title: &str,
    status: Option<&str>,
    command_lines: &[String],
    title_fg: Color,
    width: usize,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        panel_top_border(width, title_fg),
        panel_title_line(title, status, title_fg),
    ];
    lines.extend(highlight_command(command_lines));
    lines.push(Line::from("")); // bottom padding
    lines
}

/// Push the consolidated command panel and result lines into terminal scrollback.
fn flush_to_scrollback(terminal: &mut ReplTerminal, app: &mut ReplApp<'_>) -> io::Result<()> {
    let command_lines = std::mem::take(&mut app.committed_command);
    let output = std::mem::take(&mut app.output_lines);
    let ok = app.command_ok;
    app.command_ok = true;

    if command_lines.is_empty() && output.is_empty() {
        return Ok(());
    }

    let width = terminal_width();

    if !command_lines.is_empty() {
        let title = full_prompt(app.session);
        let status = if ok { "ok" } else { "error" };
        let lines = command_panel_lines(
            &title,
            Some(status),
            &command_lines,
            status_panel_fg(ok),
            width,
        );
        let height = lines.len() as u16;
        let bg = status_panel_bg(ok);
        terminal.insert_before(height, |buf| {
            Paragraph::new(lines)
                .style(Style::default().bg(bg))
                .render(buf.area, buf);
        })?;
    }

    if !command_lines.is_empty() && !output.is_empty() {
        terminal.insert_before(1, |buf| {
            Paragraph::new(Line::from("")).render(buf.area, buf);
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

    terminal.insert_before(1, |buf| {
        Paragraph::new(Line::from("")).render(buf.area, buf);
    })?;
    Ok(())
}

/// Scroll the visible screen into the terminal scrollback and leave the cursor
/// at the top-left. History stays in the scrollback, so the scroll wheel can
/// still move through it. This does not enter the alternate screen or erase
/// scrollback.
fn scroll_visible_screen_into_scrollback<B: Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    let screen_height = terminal.size()?.height.max(1);
    let bottom = screen_height.saturating_sub(1);
    terminal.set_cursor_position(Position::new(0, bottom))?;
    terminal.backend_mut().append_lines(screen_height)?;
    terminal.set_cursor_position(Position::new(0, 0))?;
    terminal
        .backend_mut()
        .clear_region(ClearType::AfterCursor)?;
    Ok(())
}

fn repl_loop(
    session: &mut Session,
    terminal: &mut ReplTerminal,
    inline_height: &mut u16,
) -> eyre::Result<()> {
    let mut app = ReplApp::new(session);
    let mut needs_redraw = true;
    loop {
        if needs_redraw {
            let height = app.viewport_height(terminal.size()?.height);
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
                // Crossterm may emit Press+Release when the terminal uses the
                // kitty keyboard protocol; only handle Press.
                if key.kind != KeyEventKind::Press {
                    continue;
                }
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
                        let submitted = app.on_enter()?;
                        if submitted && !app.partial {
                            if app.take_scroll_to_top() {
                                // `clear` scrolls the visible screen into the
                                // terminal scrollback and parks a new input at
                                // the top. The mouse wheel still reaches the
                                // previous output.
                                scroll_visible_screen_into_scrollback(terminal)?;
                                app.pinned = None;
                                let height = panel_height(1);
                                *inline_height = height;
                                *terminal = inline_terminal(height)?;
                            } else {
                                app.commit_pinned(terminal)?;
                                app.pin_if_wide(terminal_width(), screen_rows());
                                ensure_inline_height(
                                    terminal,
                                    inline_height,
                                    panel_height(app.lines.len().max(1)),
                                )?;
                                flush_to_scrollback(terminal, &mut app)?;
                            }
                        }
                        needs_redraw = true;
                        if submitted {
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
                                    app.push_error(eyre!(err).wrap_err("watch failed"));
                                    flush_to_scrollback(terminal, &mut app)?;
                                }
                                needs_redraw = true;
                            }
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
                    KeyCode::Left if key.modifiers.is_empty() => {
                        if !app.scroll_result(-1) {
                            app.move_cursor_left();
                        }
                        needs_redraw = true;
                    }
                    KeyCode::Right if key.modifiers.is_empty() => {
                        if !app.scroll_result(1) {
                            app.move_cursor_right();
                        }
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
            Event::Paste(text) => {
                app.insert_paste(&text);
                needs_redraw = true;
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
    /// `clear` should scroll the visible screen into the terminal scrollback.
    scroll_to_top: bool,
    /// Wide result kept on screen so left/right can scroll it.
    pinned: Option<PinnedResult>,
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
            scroll_to_top: false,
            pinned: None,
        }
    }

    fn take_scroll_to_top(&mut self) -> bool {
        let scroll = self.scroll_to_top;
        self.scroll_to_top = false;
        scroll
    }

    fn viewport_height(&self, screen_rows: u16) -> u16 {
        let prompt = panel_height(self.lines.len().max(1));
        let result = self
            .pinned
            .as_ref()
            .map(|pinned| pinned.lines.len() as u16)
            .unwrap_or(0);
        let total = prompt.saturating_add(result);
        if screen_rows <= prompt {
            prompt
        } else {
            total.min(screen_rows)
        }
    }

    fn editor_is_empty(&self) -> bool {
        self.row == 0 && self.col == 0 && self.lines.len() == 1 && self.lines[0].is_empty()
    }

    /// Slide a wide pinned result. Returns whether the arrow was used for that.
    fn scroll_result(&mut self, direction: i32) -> bool {
        self.scroll_result_by(direction, terminal_width())
    }

    fn scroll_result_by(&mut self, direction: i32, width: usize) -> bool {
        if !self.editor_is_empty() {
            return false;
        }
        let Some(pinned) = self.pinned.as_mut() else {
            return false;
        };
        if pinned.max_width() <= width {
            return false;
        }
        let step = (width / 4).max(1);
        let max_scroll = pinned.max_width() - width;
        let next = if direction < 0 {
            pinned.hscroll.saturating_sub(step)
        } else {
            pinned.hscroll.saturating_add(step).min(max_scroll)
        };
        pinned.hscroll = next;
        true
    }

    /// Keep a wide result in the live viewport when it also fits vertically.
    fn pin_if_wide(&mut self, width: usize, screen_rows: u16) -> bool {
        let wide = self
            .output_lines
            .iter()
            .any(|line| line_width(line) > width);
        let prompt = panel_height(self.lines.len().max(1));
        let room = usize::from(screen_rows.saturating_sub(prompt));
        if !wide || self.output_lines.len() > room {
            return false;
        }
        self.pinned = Some(PinnedResult {
            lines: std::mem::take(&mut self.output_lines),
            hscroll: 0,
        });
        true
    }

    /// Write the current horizontal window into scrollback and drop the pin.
    fn commit_pinned(&mut self, terminal: &mut ReplTerminal) -> io::Result<()> {
        let Some(pinned) = self.pinned.take() else {
            return Ok(());
        };
        let lines = pinned.visible_lines(terminal_width());
        if lines.is_empty() {
            return Ok(());
        }
        let height = u16::try_from(lines.len()).unwrap_or(u16::MAX);
        terminal.insert_before(height, |buf| {
            Paragraph::new(lines).render(buf.area, buf);
        })?;
        terminal.insert_before(1, |buf| {
            Paragraph::new(Line::from("")).render(buf.area, buf);
        })?;
        Ok(())
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
        if ch == '\n' {
            self.insert_newline();
            return;
        }
        if ch == '\r' {
            return;
        }
        let idx = self.byte_index_at_col();
        self.current_line_mut().insert(idx, ch);
        self.col += 1;
    }

    fn insert_newline(&mut self) {
        let idx = self.byte_index_at_col();
        let rest = self.current_line_mut().split_off(idx);
        self.row += 1;
        self.lines.insert(self.row, rest);
        self.col = 0;
    }

    /// Insert clipboard / bracketed-paste text without treating embedded
    /// newlines as submit.
    fn insert_paste(&mut self, text: &str) {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        for ch in normalized.chars() {
            self.insert_char(ch);
        }
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

    fn push_error(&mut self, report: eyre::Report) {
        self.command_ok = false;
        let style = Style::default().fg(Color::Red);
        for line in report_lines(&report) {
            self.output_lines
                .push(Line::from(Span::styled(line, style)));
        }
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

    /// Enter either runs the buffer or inserts a newline. Returns whether it submitted.
    fn on_enter(&mut self) -> eyre::Result<bool> {
        self.history_index = None;
        if buffer_submits_on_enter(&self.command_text()) {
            self.handle_submit()?;
            Ok(true)
        } else {
            self.insert_newline();
            Ok(false)
        }
    }

    fn handle_submit(&mut self) -> eyre::Result<()> {
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
        if first == "help" {
            let arglist = execute_text
                .split_once(char::is_whitespace)
                .map(|(_, rest)| rest.trim())
                .unwrap_or("");
            let (args, _) = crate::meta::parse_repl_args(arglist);
            match crate::help::render_lines(&args, terminal_width()) {
                Ok(lines) => self.output_lines.extend(lines),
                Err(err) => self.push_error(eyre!(err).wrap_err("help failed")),
            }
        } else if output_config.format == OutputFormat::Rich && first == "ls" {
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
                Err(err) => self.push_error(eyre!(err).wrap_err("ls failed")),
            }
        } else {
            let mut writer = backend.writer();
            match crate::meta::dispatch(self.session, &execute_text, writer.as_mut(), true) {
                Ok(Some(result)) => {
                    drop(writer);
                    let result = crate::meta::ls::annotate_table_list(self.session, result);
                    let render = if output_config.format == OutputFormat::Rich {
                        match &result {
                            StatementResult::Items(items) => {
                                let layout = build_rich_layout(items, rich_context.as_ref());
                                self.output_lines
                                    .extend(rich_layout_to_lines(&layout, terminal_width() as u16));
                                Ok(())
                            }
                            StatementResult::ItemsWithNote { note, .. } => {
                                for line in note.lines() {
                                    self.output_lines.push(Line::from(line.to_string()));
                                }
                                Ok(())
                            }
                            _ => render_result(
                                &result,
                                &output_config,
                                &mut backend,
                                rich_context.as_ref(),
                            ),
                        }
                    } else {
                        render_result(&result, &output_config, &mut backend, rich_context.as_ref())
                    };
                    if let Err(err) = render {
                        self.push_error(eyre!(err).wrap_err("failed to render result"));
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    drop(writer);
                    self.push_error(eyre!(err).wrap_err("query failed"));
                }
            }
        }

        if matches!(first.as_str(), "clear" | "cls" | "c") {
            self.committed_command.clear();
            self.output_lines.clear();
            self.command_ok = true;
            self.partial = false;
            self.reset_buffer();
            self.scroll_to_top = true;
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
    session.prompt_title()
}

fn live_panel_lines(app: &ReplApp<'_>, width: usize) -> Vec<Line<'static>> {
    let title = full_prompt(app.session);
    command_panel_lines(&title, None, &app.lines, Color::Cyan, width)
}

fn draw(frame: &mut Frame, app: &ReplApp<'_>) {
    let area = frame.area();
    let width = area.width.max(1) as usize;
    let prompt_h = panel_height(app.lines.len().max(1)).min(area.height);
    let result_h = if app.pinned.is_some() {
        area.height.saturating_sub(prompt_h)
    } else {
        0
    };
    if result_h > 0 {
        if let Some(pinned) = &app.pinned {
            let lines = pinned
                .visible_lines(width)
                .into_iter()
                .take(result_h as usize)
                .collect::<Vec<_>>();
            frame.render_widget(
                Paragraph::new(lines),
                Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: result_h,
                },
            );
        }
    }
    let prompt_area = if result_h == 0 {
        area
    } else {
        Rect {
            x: area.x,
            y: area.y.saturating_add(result_h),
            width: area.width,
            height: prompt_h,
        }
    };
    let lines = live_panel_lines(app, width);
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(live_panel_bg())),
        prompt_area,
    );

    // Caret on the active edit row (below top border + title).
    if prompt_area.width > 0 && prompt_area.height > 2 {
        let row = (2 + app.row as u16).min(prompt_area.height.saturating_sub(1));
        let col = (app.col as u16).min(prompt_area.width.saturating_sub(1));
        frame.set_cursor_position(Position {
            x: prompt_area.x.saturating_add(col),
            y: prompt_area.y.saturating_add(row),
        });
    }
}

#[cfg(test)]
#[path = "tui_tests.rs"]
mod tui_tests;
