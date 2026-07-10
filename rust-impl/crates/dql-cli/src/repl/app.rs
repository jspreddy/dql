use crate::meta::lifecycle::take_exit_request;
#[cfg(feature = "watch")]
use crate::meta::watch::take_watch_request;
use crate::session::Session;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseEventKind};
use dql_engine::StatementResult;
use dql_output::{build_rich_layout, render_result, DisplayBackend, OutputFormat};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::io::{self, Write};
use std::time::Duration;

use super::rich_table::rich_layout_to_lines;

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
    let mut terminal = ratatui::init();
    enable_mouse_capture()?;
    let result = repl_loop(session, &mut terminal);
    let _ = disable_mouse_capture();
    ratatui::restore();
    session.history.try_to_write_history();
    result
}

fn enable_mouse_capture() -> io::Result<()> {
    crossterm::execute!(io::stdout(), event::EnableMouseCapture)
}

fn disable_mouse_capture() -> io::Result<()> {
    crossterm::execute!(io::stdout(), event::DisableMouseCapture)
}

fn terminal_width() -> usize {
    crossterm::terminal::size()
        .map(|(width, _)| width as usize)
        .unwrap_or(80)
        .max(40)
}

fn terminal_height() -> usize {
    crossterm::terminal::size()
        .map(|(_, height)| height as usize)
        .unwrap_or(24)
}

fn repl_loop(
    session: &mut Session,
    terminal: &mut ratatui::DefaultTerminal,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = ReplApp::new(session);
    loop {
        terminal.draw(|frame| draw(frame, &app))?;
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.session.engine.reset_fragment();
                        app.transcript.truncate(app.command_start);
                        app.input.clear();
                        app.partial = false;
                        app.scroll_offset = 0;
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
                        app.input.clear();
                        app.scroll_offset = 0;
                        if take_exit_request() {
                            break;
                        }
                        #[cfg(feature = "watch")]
                        if let Some(tables) = take_watch_request() {
                            let _ = disable_mouse_capture();
                            ratatui::restore();
                            let watch_result =
                                crate::meta::watch::run_monitor(app.session, &tables);
                            *terminal = ratatui::init();
                            enable_mouse_capture()?;
                            if let Err(err) = watch_result {
                                app.transcript
                                    .push(Line::from(format!("watch error: {err}")));
                            }
                            app.clamp_scroll(terminal_height());
                        }
                    }
                    KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        app.scroll_up(3, terminal_height());
                    }
                    KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        app.scroll_down(3);
                    }
                    KeyCode::Up => app.history_up(),
                    KeyCode::Down => app.history_down(),
                    KeyCode::Tab => app.complete(),
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::Char(ch) => {
                        app.input.push(ch);
                        app.scroll_offset = 0;
                    }
                    _ => {}
                },
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => app.scroll_up(3, terminal_height()),
                    MouseEventKind::ScrollDown => app.scroll_down(3),
                    _ => {}
                },
                Event::Resize(_, _) => app.clamp_scroll(terminal_height()),
                _ => {}
            }
        }
        if take_exit_request() {
            break;
        }
    }
    Ok(())
}

struct ReplApp<'a> {
    session: &'a mut Session,
    /// Scrollback of prior prompts, command lines, and results in order.
    transcript: Vec<Line<'static>>,
    /// Index in `transcript` where the in-progress command began (for Ctrl-C).
    command_start: usize,
    input: String,
    partial: bool,
    history_index: Option<usize>,
    /// Lines scrolled up from the bottom of the transcript (0 = follow latest).
    scroll_offset: usize,
}

impl<'a> ReplApp<'a> {
    fn new(session: &'a mut Session) -> Self {
        Self {
            session,
            transcript: Vec::new(),
            command_start: 0,
            input: String::new(),
            partial: false,
            history_index: None,
            scroll_offset: 0,
        }
    }

    fn max_scroll_offset(&self, visible_height: usize) -> usize {
        display_lines(self).len().saturating_sub(visible_height)
    }

    fn clamp_scroll(&mut self, visible_height: usize) {
        let max = self.max_scroll_offset(visible_height);
        self.scroll_offset = self.scroll_offset.min(max);
    }

    fn scroll_up(&mut self, amount: usize, visible_height: usize) {
        let max = self.max_scroll_offset(visible_height);
        if max == 0 {
            return;
        }
        self.scroll_offset = (self.scroll_offset + amount).min(max);
    }

    fn scroll_down(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    fn push_command_line(&mut self, text: String, continuation: bool) {
        if continuation {
            self.transcript
                .push(Line::from(vec![Span::raw("   | "), Span::raw(text)]));
        } else {
            let prompt = full_prompt(self.session);
            self.transcript.push(Line::from(vec![
                Span::styled(format!("{prompt} ===> "), Style::default().fg(Color::Cyan)),
                Span::raw(text),
            ]));
        }
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
            self.command_start = self.transcript.len();
        }
        if !line.is_empty() {
            self.push_command_line(line.clone(), continuation);
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
                Ok(lines) => self.transcript.extend(lines),
                Err(err) => self.transcript.push(Line::from(err)),
            }
        } else {
            let mut writer = backend.writer();
            if let Some(result) = crate::meta::dispatch(self.session, &line, writer.as_mut(), true)?
            {
                drop(writer);
                if output_config.format == OutputFormat::Rich {
                    if let StatementResult::Items(items) = &result {
                        let layout = build_rich_layout(items, rich_context.as_ref());
                        self.transcript
                            .extend(rich_layout_to_lines(&layout, terminal_width() as u16));
                    } else {
                        render_result(
                            &result,
                            &output_config,
                            &mut backend,
                            rich_context.as_ref(),
                        )?;
                    }
                } else {
                    render_result(&result, &output_config, &mut backend, rich_context.as_ref())?;
                }
            }
        }
        if matches!(cmd.as_str(), "clear" | "cls" | "c") {
            self.transcript.clear();
            self.command_start = 0;
            self.partial = false;
            self.scroll_offset = 0;
            return Ok(());
        }
        if let Ok(text) = String::from_utf8(backend.buffer) {
            for line in text.lines() {
                self.transcript.push(Line::from(line.to_string()));
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
            self.input = entry.clone();
            self.history_index = Some(index);
        }
    }

    fn history_down(&mut self) {
        let len = self.session.history.entries().len();
        let Some(index) = self.history_index else {
            return;
        };
        if index + 1 >= len {
            self.input.clear();
            self.history_index = None;
        } else if let Some(entry) = self.session.history.entries().get(index + 1) {
            self.input = entry.clone();
            self.history_index = Some(index + 1);
        }
    }

    fn complete(&mut self) {
        let tables = with_tables(self.session);
        let lower = self.input.to_ascii_lowercase();
        for token in ["from", "into", "table", "update", "dump"] {
            if lower.contains(token) {
                if let Some(table) = tables.iter().find(|name| name.starts_with(&self.input)) {
                    self.input = format!("{table} ");
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

fn current_input_line(app: &ReplApp<'_>) -> Line<'static> {
    if app.partial {
        Line::from(vec![Span::raw("   | "), Span::raw(app.input.clone())])
    } else {
        let prompt = full_prompt(app.session);
        Line::from(vec![
            Span::styled(format!("{prompt} ===> "), Style::default().fg(Color::Cyan)),
            Span::raw(app.input.clone()),
        ])
    }
}

fn display_lines(app: &ReplApp<'_>) -> Vec<Line<'static>> {
    let mut lines = app.transcript.clone();
    lines.push(current_input_line(app));
    lines
}

fn draw(frame: &mut Frame, app: &ReplApp<'_>) {
    let lines = display_lines(app);
    let visible_height = frame.area().height as usize;
    let max_start = lines.len().saturating_sub(visible_height);
    let start = max_start.saturating_sub(app.scroll_offset);
    let visible = lines[start..].to_vec();
    frame.render_widget(Paragraph::new(visible), frame.area());
}
