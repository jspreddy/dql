use crate::session::Session;
use dql_models::TableMeta;
use dql_output::{format_table_detail, format_table_summary_table, TableStats};
use ratatui::text::Line;
use std::collections::HashMap;
use std::io::Write;

pub enum LsView {
    Summary {
        rows: Vec<(TableMeta, TableStats)>,
        note: Option<String>,
    },
    Detail {
        meta: Box<TableMeta>,
        stats: TableStats,
        note: Option<String>,
    },
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
        LsView::Summary { rows, note } => {
            write_note(out, note.as_deref())?;
            writeln!(out, "{}", format_table_summary_table(&rows)).map_err(|err| err.to_string())?
        }
        LsView::Detail { meta, stats, note } => {
            write_note(out, note.as_deref())?;
            writeln!(out, "{}", format_table_detail(&meta, &stats))
                .map_err(|err| err.to_string())?
        }
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
        LsView::Summary { rows, note } => {
            push_note(&mut lines, note.as_deref());
            lines.extend(table_summary_to_lines(&rows, width));
        }
        LsView::Detail { meta, stats, note } => {
            push_note(&mut lines, note.as_deref());
            lines.extend(table_detail_to_lines(&meta, &stats, width));
        }
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
        return Ok(LsView::Summary { rows, note: None });
    }
    let pattern = args[0].trim_end_matches(';');
    let (filtered, note) = resolve_names(session, pattern, refresh)?;
    match filtered.len() {
        0 => Err(format!("Table {pattern:?} not found")),
        1 => {
            let name = &filtered[0];
            let meta = session
                .engine
                .describe_with_metrics(name, refresh, metrics)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| format!("Table {name:?} not found"))?;
            Ok(LsView::Detail {
                meta: Box::new(meta),
                stats: table_stats(session, name),
                note,
            })
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
            Ok(LsView::Summary { rows, note })
        }
    }
}

fn glob_matches(pattern: &str, name: &str) -> bool {
    glob::Pattern::new(pattern)
        .map(|pattern| pattern.matches(name))
        .unwrap_or(false)
}

fn resolve_names(
    session: &mut Session,
    pattern: &str,
    refresh: bool,
) -> Result<(Vec<String>, Option<String>), String> {
    let names = session
        .engine
        .list_tables()
        .map_err(|err| err.to_string())?;
    let exact: Vec<String> = names
        .into_iter()
        .filter(|name| glob_matches(pattern, name))
        .collect();
    if !exact.is_empty() {
        return Ok((exact, None));
    }
    let metas = session
        .engine
        .describe_all(refresh)
        .map_err(|err| err.to_string())?;
    let tables: Vec<_> = metas
        .iter()
        .map(dql_engine::MatchTable::from_meta)
        .collect();
    let hit = dql_engine::intelligent_matches(&tables, pattern)?;
    Ok((hit.names, Some(hit.note)))
}

fn write_note(out: &mut dyn Write, note: Option<&str>) -> Result<(), String> {
    if let Some(note) = note {
        writeln!(out, "{note}\n").map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn push_note(lines: &mut Vec<Line<'static>>, note: Option<&str>) {
    if let Some(note) = note {
        lines.push(Line::from(note.to_string()));
        lines.push(Line::from(""));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_glob_wins_over_a_partial_name() {
        let names = ["nb_posts", "posts"];
        let exact: Vec<_> = names
            .into_iter()
            .filter(|name| glob_matches("posts", name))
            .collect();
        assert_eq!(exact, vec!["posts"]);
    }
}
