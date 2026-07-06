use crate::meta::lifecycle::take_exit_request;
use crate::session::Session;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use dql_output::{render_result, OutputConfig};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use std::io::{self, Write};
use std::time::Duration;

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
    let result = repl_loop(session, &mut terminal);
    ratatui::restore();
    session.history.try_to_write_history();
    result
}

fn repl_loop(
    session: &mut Session,
    terminal: &mut ratatui::DefaultTerminal,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = ReplApp::new(session);
    loop {
        terminal.draw(|frame| draw(frame, &app))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.session.engine.reset_fragment();
                        app.input.clear();
                        app.partial = false;
                    }
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let _ = app.session.history.remove_items(1);
                        break;
                    }
                    KeyCode::Enter => {
                        let line = app.input.clone();
                        app.history_index = None;
                        if !line.trim().is_empty() {
                            app.session.history.add_entry(line.clone());
                        }
                        app.handle_submit(line)?;
                        app.input.clear();
                        if take_exit_request() {
                            break;
                        }
                    }
                    KeyCode::Up => app.history_up(),
                    KeyCode::Down => app.history_down(),
                    KeyCode::Tab => app.complete(),
                    KeyCode::Backspace => {
                        app.input.pop();
                        if app.input.is_empty() {
                            app.partial = false;
                        }
                    }
                    KeyCode::Char(ch) => app.input.push(ch),
                    _ => {}
                }
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
    input: String,
    output: Vec<String>,
    partial: bool,
    history_index: Option<usize>,
}

impl<'a> ReplApp<'a> {
    fn new(session: &'a mut Session) -> Self {
        Self {
            session,
            input: String::new(),
            output: Vec::new(),
            partial: false,
            history_index: None,
        }
    }

    fn handle_submit(&mut self, line: String) -> Result<(), Box<dyn std::error::Error>> {
        let output_config = self.session.config.output_config();
        let mut backend = BufferBackend::default();
        if let Some(result) = crate::meta::dispatch(self.session, &line)? {
            render_result(&result, &output_config, &mut backend)?;
        }
        if let Ok(text) = String::from_utf8(backend.buffer) {
            for line in text.lines() {
                self.output.push(line.to_string());
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

fn draw(frame: &mut Frame, app: &ReplApp<'_>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(frame.area());
    let output_lines: Vec<ListItem> = app
        .output
        .iter()
        .rev()
        .take(frame.area().height as usize)
        .map(|line| ListItem::new(line.as_str()))
        .collect();
    let output = List::new(output_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Output"),
    );
    frame.render_widget(output, chunks[0]);

    let prompt = if app.partial {
        "   | ".to_string()
    } else if let Some((host, port)) = &app.session.local_endpoint {
        format!("({host}:{port}) {}", app.session.region)
    } else {
        app.session.region.clone()
    };
    let input = Paragraph::new(Line::from(vec![
        Span::styled(format!("{prompt} ===> "), Style::default().fg(Color::Cyan)),
        Span::raw(app.input.as_str()),
    ]))
    .block(Block::default().borders(Borders::ALL).title("Input"));
    frame.render_widget(input, chunks[1]);
}
