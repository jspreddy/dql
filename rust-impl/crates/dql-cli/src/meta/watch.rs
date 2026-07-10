use crate::session::Session;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use dql_engine::format_throughput;
use dql_models::TableMeta;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use std::collections::HashMap;
use std::io::Write;
use std::time::{Duration, Instant};

const REFRESH_SECS: u64 = 30;

thread_local! {
    static PENDING_WATCH: std::cell::RefCell<Option<Vec<String>>> =
        std::cell::RefCell::new(None);
}

pub fn take_watch_request() -> Option<Vec<String>> {
    PENDING_WATCH.with(|cell| cell.borrow_mut().take())
}

pub fn handle(
    session: &mut Session,
    args: &[String],
    _: &HashMap<String, String>,
    out: &mut dyn Write,
    repl: bool,
) -> Result<(), String> {
    let tables = resolve_tables(session, args)?;
    if tables.is_empty() {
        return Err("No tables matched".to_string());
    }
    if repl {
        PENDING_WATCH.with(|cell| {
            *cell.borrow_mut() = Some(tables);
        });
        writeln!(out, "Starting watch… (q / Ctrl-C to stop)")
            .map_err(|err| err.to_string())?;
        return Ok(());
    }
    run_monitor(session, &tables)
}

pub fn run_monitor(session: &mut Session, tables: &[String]) -> Result<(), String> {
    let mut terminal = ratatui::init();
    let result = monitor_loop(session, tables, &mut terminal);
    ratatui::restore();
    result
}

fn monitor_loop(
    session: &mut Session,
    tables: &[String],
    terminal: &mut ratatui::DefaultTerminal,
) -> Result<(), String> {
    let mut last_fetch = Instant::now() - Duration::from_secs(REFRESH_SECS);
    let mut metas: Vec<TableMeta> = Vec::new();
    loop {
        if last_fetch.elapsed() >= Duration::from_secs(REFRESH_SECS) || metas.is_empty() {
            metas = fetch_tables(session, tables)?;
            last_fetch = Instant::now();
        }
        let status = format!(
            "{}  {} table(s)  refresh {}s  (q to quit)",
            chrono_now(),
            tables.len(),
            REFRESH_SECS
        );
        terminal
            .draw(|frame| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(1)])
                    .split(frame.area());
                frame.render_widget(Paragraph::new(status.clone()), chunks[0]);
                let lines = render_capacity_lines(&metas);
                let body = Paragraph::new(lines).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Consumed capacity"),
                );
                frame.render_widget(body, chunks[1]);
            })
            .map_err(|err| err.to_string())?;

        if event::poll(Duration::from_millis(200)).map_err(|err| err.to_string())? {
            if let Event::Key(key) = event::read().map_err(|err| err.to_string())? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Char('r') => {
                        metas = fetch_tables(session, tables)?;
                        last_fetch = Instant::now();
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn resolve_tables(session: &mut Session, args: &[String]) -> Result<Vec<String>, String> {
    let all = session
        .engine
        .list_tables()
        .map_err(|err| err.to_string())?;
    if args.is_empty() {
        return Ok(all);
    }
    let mut matched = Vec::new();
    for pattern in args {
        let pat = glob::Pattern::new(pattern).map_err(|err| err.to_string())?;
        for name in &all {
            if pat.matches(name) && !matched.contains(name) {
                matched.push(name.clone());
            }
        }
    }
    matched.sort();
    Ok(matched)
}

fn fetch_tables(session: &mut Session, tables: &[String]) -> Result<Vec<TableMeta>, String> {
    let metrics = !session.engine.is_local_or_memory();
    let mut metas = Vec::new();
    for name in tables {
        if let Some(meta) = session
            .engine
            .describe_with_metrics(name, true, metrics)
            .map_err(|err| err.to_string())?
        {
            metas.push(meta);
        }
    }
    Ok(metas)
}

fn render_capacity_lines(metas: &[TableMeta]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for meta in metas {
        lines.push(Line::from(Span::styled(
            meta.name.clone(),
            Style::default().fg(Color::Cyan),
        )));
        let cap = meta.consumed_capacity.get("__table__");
        let read = cap.map(|c| c.read).unwrap_or(0.0);
        let write = cap.map(|c| c.write).unwrap_or(0.0);
        lines.push(capacity_line(
            "",
            "R",
            meta.table_read_throughput(),
            read,
        ));
        lines.push(capacity_line(
            "",
            "W",
            meta.table_write_throughput(),
            write,
        ));
        for (index_name, gindex) in &meta.global_indexes {
            let idx_cap = meta.consumed_capacity.get(index_name);
            let read = idx_cap.map(|c| c.read).unwrap_or(0.0);
            let write = idx_cap.map(|c| c.write).unwrap_or(0.0);
            let avail_read = gindex.throughput.as_ref().and_then(|tp| match &tp.read {
                dql_parser::Value::Number(n) => n.parse().ok(),
                _ => None,
            });
            let avail_write = gindex.throughput.as_ref().and_then(|tp| match &tp.write {
                dql_parser::Value::Number(n) => n.parse().ok(),
                _ => None,
            });
            lines.push(capacity_line(index_name, "R", avail_read, read));
            lines.push(capacity_line(index_name, "W", avail_write, write));
        }
        lines.push(Line::from(""));
    }
    if lines.is_empty() {
        lines.push(Line::from("No table data"));
    }
    lines
}

fn capacity_line(title: &str, op: &str, available: Option<f64>, used: f64) -> Line<'static> {
    let label = if title.is_empty() {
        format_throughput(available, Some(used))
    } else {
        format!("{title} {}", format_throughput(available, Some(used)))
    };
    let percent = match available {
        Some(avail) if avail > 0.0 => used / avail,
        _ => 0.0,
    };
    let color = if percent < 0.7 {
        Color::Green
    } else if percent < 0.9 {
        Color::Yellow
    } else {
        Color::Red
    };
    let bar_width = 24usize;
    let filled = ((percent.clamp(0.0, 1.0)) * bar_width as f64).round() as usize;
    let bar = format!(
        "[{}{}] {}:{}",
        "|".repeat(filled),
        " ".repeat(bar_width.saturating_sub(filled)),
        label,
        op
    );
    Line::from(Span::styled(bar, Style::default().fg(color)))
}

fn chrono_now() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let hours = (secs / 3600) % 24;
    let mins = (secs / 60) % 60;
    let s = secs % 60;
    // UTC clock is fine for a status line; avoids extra chrono dependency.
    format!("{hours:02}:{mins:02}:{s:02} UTC")
}
