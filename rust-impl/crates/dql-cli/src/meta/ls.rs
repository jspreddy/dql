use crate::session::Session;
use crate::table_cache::age_phrase;
use dql_engine::{StatementResult, TableCacheStore};
use dql_models::TableMeta;
use dql_output::{format_table_detail, format_table_summary_table, TableStats};
use dql_parser::Value;
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
    let background = parse_bool(kwargs.get("async"), false);
    let metrics = parse_bool(kwargs.get("metrics"), false);
    let refresh_hint = prepare_cache(session, refresh, background)?;
    if args.is_empty() {
        let names = session
            .engine
            .cached_table_names(false)
            .map_err(|err| err.to_string())?;
        let rows = rows_for_names(session, &names);
        let hint = refresh_hint.or_else(|| cache_hint(session));
        return Ok(with_hint(LsView::Summary { rows, note: None }, hint));
    }
    let pattern = args[0].trim_end_matches(';');
    let (filtered, note) = resolve_names(session, pattern)?;
    let hint = refresh_hint.or_else(|| cache_hint(session));
    match filtered.len() {
        0 => Err(format!("Table {pattern:?} not found")),
        1 => {
            let name = &filtered[0];
            let fetch = refresh && !background;
            let meta = if fetch {
                session
                    .engine
                    .describe_with_metrics(name, true, metrics)
                    .map_err(|err| err.to_string())?
            } else {
                match session.engine.cached_definition(name) {
                    Some(meta) if !metrics => Some(meta),
                    _ => session
                        .engine
                        .describe_with_metrics(name, false, metrics)
                        .map_err(|err| err.to_string())?,
                }
            }
            .ok_or_else(|| format!("Table {name:?} not found"))?;
            Ok(with_hint(
                LsView::Detail {
                    meta: Box::new(meta),
                    stats: table_stats(session, name),
                    note,
                },
                hint,
            ))
        }
        _ => {
            let rows = rows_for_names(session, &filtered);
            Ok(with_hint(LsView::Summary { rows, note }, hint))
        }
    }
}

fn rows_for_names(session: &mut Session, names: &[String]) -> Vec<(TableMeta, TableStats)> {
    names
        .iter()
        .map(|name| {
            let meta = session
                .engine
                .cached_definition(name)
                .unwrap_or_else(|| TableMeta::name_only(name));
            let stats = if meta.has_definition() {
                table_stats(session, name)
            } else {
                TableStats::default()
            };
            (meta, stats)
        })
        .collect()
}

fn prepare_cache(
    session: &mut Session,
    refresh: bool,
    background: bool,
) -> Result<Option<String>, String> {
    if background {
        session.start_background_refresh();
        return Ok(None);
    }
    if !refresh {
        return Ok(None);
    }
    let definitions = session
        .engine
        .refresh_cached_definitions()
        .map_err(|err| err.to_string())?;
    session
        .engine
        .cached_table_names(true)
        .map_err(|err| err.to_string())?;
    let text = if definitions == 0 {
        "Refreshed the cached table list.".to_string()
    } else if definitions == 1 {
        "Refreshed the cached table list and 1 cached definition.".to_string()
    } else {
        format!("Refreshed the cached table list and {definitions} cached definitions.")
    };
    Ok(Some(text))
}

/// Saved table-list age for the settings menu. Does not list or describe tables.
pub fn web_table_cache_status(session: &Session) -> crate::web::shape::TableCacheHint {
    let Some(disk) = session.table_disk.as_ref() else {
        return crate::web::shape::TableCacheHint {
            text: "No table list cached.".to_string(),
            ..crate::web::shape::TableCacheHint::default()
        };
    };
    let saved_at_ms = disk.get_names().map(|cached| cached.saved_at_ms);
    let refreshing = session.cache_refreshing();
    if let Some(error) = session.cache_error() {
        if !refreshing {
            return crate::web::shape::TableCacheHint {
                text: format!("Cache refresh failed: {error}"),
                from_cache: saved_at_ms.is_some(),
                saved_at_ms,
                refreshing: false,
            };
        }
    }
    let Some(saved_at_ms) = saved_at_ms else {
        return crate::web::shape::TableCacheHint {
            text: if refreshing {
                "Refreshing cached tables in the background.".to_string()
            } else {
                "No table list cached.".to_string()
            },
            from_cache: false,
            saved_at_ms: None,
            refreshing,
        };
    };
    let age = age_phrase(saved_at_ms);
    let text = if refreshing {
        format!("Showing cached table list from {age}. Refreshing cached tables in the background.")
    } else {
        format!("Showing cached table list from {age}.")
    };
    crate::web::shape::TableCacheHint {
        text,
        from_cache: true,
        saved_at_ms: Some(saved_at_ms),
        refreshing,
    }
}

fn cache_hint(session: &Session) -> Option<String> {
    session.table_disk.as_ref()?;
    if let Some(error) = session.cache_error() {
        if !session.cache_refreshing() {
            return Some(format!("Cache refresh failed: {error}"));
        }
    }
    let age = session
        .engine
        .names_saved_at_ms()
        .map(age_phrase)
        .unwrap_or_else(|| "just now".to_string());
    if session.cache_refreshing() {
        if session.engine.names_from_cache() {
            return Some(format!(
                "Showing cached table list from {age}. Refreshing cached tables in the background."
            ));
        }
        return Some("Refreshing cached tables in the background.".to_string());
    }
    if session.engine.names_from_cache() {
        return Some(format!("Showing cached table list from {age}."));
    }
    Some("Fetched the table list.".to_string())
}

fn with_hint(view: LsView, hint: Option<String>) -> LsView {
    let hint = hint.filter(|text| !text.is_empty());
    match view {
        LsView::Summary { rows, note } => LsView::Summary {
            rows,
            note: merge_note(note, hint),
        },
        LsView::Detail { meta, stats, note } => LsView::Detail {
            meta,
            stats,
            note: merge_note(note, hint),
        },
    }
}

fn merge_note(note: Option<String>, hint: Option<String>) -> Option<String> {
    match (note.filter(|text| !text.is_empty()), hint) {
        (Some(note), Some(hint)) => Some(format!("{note}\ncache: {hint}")),
        (Some(note), None) => Some(note),
        (None, Some(hint)) => Some(format!("cache: {hint}")),
        (None, None) => None,
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
) -> Result<(Vec<String>, Option<String>), String> {
    let names = session
        .engine
        .cached_table_names(false)
        .map_err(|err| err.to_string())?;
    let exact: Vec<String> = names
        .iter()
        .filter(|name| glob_matches(pattern, name))
        .cloned()
        .collect();
    if !exact.is_empty() {
        return Ok((exact, None));
    }
    let tables: Vec<_> = names
        .iter()
        .filter_map(|name| session.engine.cached_definition(name))
        .map(|meta| dql_engine::MatchTable::from_meta(&meta))
        .collect();
    let hit = dql_engine::intelligent_matches(&tables, pattern)?;
    Ok((hit.names, Some(hit.note)))
}

pub fn web_table_list(
    session: &mut Session,
    pattern: &str,
    refresh: bool,
    background: bool,
) -> Result<crate::web::shape::TableList, String> {
    let mut kwargs = HashMap::new();
    if refresh {
        kwargs.insert("refresh".to_string(), "true".to_string());
    }
    if background {
        kwargs.insert("async".to_string(), "true".to_string());
    }
    let args = if pattern.trim().is_empty() {
        Vec::new()
    } else {
        vec![pattern.to_string()]
    };
    let view = match collect_view(session, &args, &kwargs) {
        Ok(view) => view,
        Err(message) if missing_table(&message) => {
            return Ok(listed_tables(
                Vec::new(),
                String::new(),
                session,
                cache_hint(session).unwrap_or_default(),
            ));
        }
        Err(message) => return Err(message),
    };
    let (rows, note) = match view {
        LsView::Summary { rows, note } => (rows, note.unwrap_or_default()),
        LsView::Detail { meta, note, .. } => (
            vec![(*meta, TableStats::default())],
            note.unwrap_or_default(),
        ),
    };
    let cache_text = note
        .lines()
        .find(|line| line.starts_with("cache:"))
        .map(|line| line.trim_start_matches("cache:").trim().to_string())
        .unwrap_or_default();
    let note = note
        .lines()
        .filter(|line| !line.starts_with("cache:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    let tables = rows
        .into_iter()
        .map(|(meta, _)| crate::web::shape::ListedTable {
            keys: if meta.has_definition() {
                let mut keys = vec![crate::web::shape::KeyRole {
                    name: meta.hash_key.clone(),
                    role: "hash".to_string(),
                }];
                if let Some(range_key) = meta.range_key.clone() {
                    keys.push(crate::web::shape::KeyRole {
                        name: range_key,
                        role: "range".to_string(),
                    });
                }
                keys
            } else {
                Vec::new()
            },
            name: meta.name,
        })
        .collect();
    Ok(listed_tables(tables, note, session, cache_text))
}

fn missing_table(message: &str) -> bool {
    message.starts_with("Table ") && message.ends_with(" not found")
}

fn listed_tables(
    tables: Vec<crate::web::shape::ListedTable>,
    note: String,
    session: &Session,
    cache_text: String,
) -> crate::web::shape::TableList {
    crate::web::shape::TableList {
        tables,
        note,
        cache: crate::web::shape::TableCacheHint {
            from_cache: cache_text.starts_with("Showing cached"),
            refreshing: session.cache_refreshing() || cache_text.contains("background"),
            saved_at_ms: session.engine.names_saved_at_ms(),
            text: cache_text,
        },
    }
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

/// Turn a `SHOW TABLES LIKE` fallback into the same list `ls` prints.
pub fn annotate_table_list(session: &mut Session, result: StatementResult) -> StatementResult {
    let StatementResult::ItemsWithNote { items, note } = result else {
        return result;
    };
    let mut rows = Vec::new();
    for item in &items {
        let Some(Value::String(name)) = item.get("name") else {
            continue;
        };
        let Some(meta) = session.engine.cached_definition(name) else {
            rows.push((TableMeta::name_only(name), table_stats_with_count(0)));
            continue;
        };
        rows.push(table_row(session, meta));
    }
    if rows.is_empty() {
        if note.is_empty() {
            return StatementResult::Items(items);
        }
        return StatementResult::ItemsWithNote { items, note };
    }
    let summary = format_table_summary_table(&rows);
    let note = if note.is_empty() {
        summary.trim_end().to_string()
    } else {
        format!("{note}\n\n{}", summary.trim_end())
    };
    StatementResult::ItemsWithNote { items, note }
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
