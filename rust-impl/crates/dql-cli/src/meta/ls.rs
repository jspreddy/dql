use crate::session::Session;
use dql_models::TableMeta;
use dql_output::{format_table_detail, format_table_summary_table, TableStats};
use ratatui::text::Line;
use std::collections::HashMap;
use std::io::Write;

pub enum LsView {
    Summary(Vec<(TableMeta, TableStats)>),
    Detail(TableMeta, TableStats),
}

pub fn handle(
    session: &mut Session,
    args: &[String],
    kwargs: &HashMap<String, String>,
    out: &mut dyn Write,
    _repl: bool,
) -> Result<(), String> {
    let metrics = parse_bool(kwargs.get("metrics"), false);
    if metrics {
        if let Some(note) = metrics_note(session) {
            writeln!(out, "{note}").map_err(|err| err.to_string())?;
        }
    }
    match collect_view(session, args, kwargs)? {
        LsView::Summary(rows) => {
            writeln!(out, "{}", format_table_summary_table(&rows)).map_err(|err| err.to_string())?
        }
        LsView::Detail(meta, stats) => writeln!(out, "{}", format_table_detail(&meta, &stats))
            .map_err(|err| err.to_string())?,
    }
    Ok(())
}

pub fn render_rich_lines(
    session: &mut Session,
    args: &[String],
    kwargs: &HashMap<String, String>,
    width: u16,
) -> Result<Vec<Line<'static>>, String> {
    use crate::repl::rich_table::{table_detail_to_lines, table_summary_to_lines};

    let metrics = parse_bool(kwargs.get("metrics"), false);
    let mut lines = Vec::new();
    if metrics {
        if let Some(note) = metrics_note(session) {
            lines.push(Line::from(note));
        }
    }
    match collect_view(session, args, kwargs)? {
        LsView::Summary(rows) => lines.extend(table_summary_to_lines(&rows, width)),
        LsView::Detail(meta, stats) => lines.extend(table_detail_to_lines(&meta, &stats, width)),
    }
    Ok(lines)
}

pub fn collect_view(
    session: &mut Session,
    args: &[String],
    kwargs: &HashMap<String, String>,
) -> Result<LsView, String> {
    let refresh = parse_bool(kwargs.get("refresh"), false);
    let metrics = parse_bool(kwargs.get("metrics"), false);
    if args.is_empty() {
        let tables = session
            .engine
            .describe_all(refresh)
            .map_err(|err| err.to_string())?;
        let rows = tables
            .into_iter()
            .map(|meta| table_row(session, meta))
            .collect();
        return Ok(LsView::Summary(rows));
    }
    let pattern = args[0].trim_end_matches(';');
    let tables = session
        .engine
        .list_tables()
        .map_err(|err| err.to_string())?;
    let filtered: Vec<_> = tables
        .into_iter()
        .filter(|name| {
            glob::Pattern::new(pattern)
                .map(|p| p.matches(name))
                .unwrap_or(false)
        })
        .collect();
    match filtered.len() {
        0 => Err(format!("Table {pattern:?} not found")),
        1 => {
            let name = &filtered[0];
            let meta = session
                .engine
                .describe_with_metrics(name, refresh, metrics)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| format!("Table {name:?} not found"))?;
            Ok(LsView::Detail(meta, table_stats(session, name)))
        }
        _ => {
            let mut rows = Vec::new();
            for name in filtered {
                let meta = session
                    .engine
                    .describe_with_metrics(&name, refresh, metrics)
                    .map_err(|err| err.to_string())?
                    .ok_or_else(|| format!("Table {name:?} not found"))?;
                rows.push((meta, table_stats(session, &name)));
            }
            Ok(LsView::Summary(rows))
        }
    }
}

fn table_row(session: &Session, meta: TableMeta) -> (TableMeta, TableStats) {
    let count = session.engine.table_item_count(&meta.name);
    (meta, table_stats_with_count(count))
}

fn table_stats(session: &Session, name: &str) -> TableStats {
    table_stats_with_count(session.engine.table_item_count(name))
}

fn table_stats_with_count(item_count: usize) -> TableStats {
    TableStats {
        item_count,
        size_bytes: 0,
    }
}

fn metrics_note(session: &Session) -> Option<String> {
    if session.engine.is_local_or_memory() {
        return Some(
            "note: metrics=True has no CloudWatch data for local/memory backends".to_string(),
        );
    }
    #[cfg(not(feature = "watch"))]
    {
        let _ = session;
        Some("note: CloudWatch metrics require rebuilding with --features watch".to_string())
    }
    #[cfg(feature = "watch")]
    {
        let _ = session;
        None
    }
}

fn parse_bool(value: Option<&String>, default: bool) -> bool {
    value
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(default)
}
