use crate::session::Session;
use crate::table_cache::age_phrase;
use dql_engine::{StatementResult, TableCacheStore};
use dql_models::TableMeta;
use dql_output::{format_table_summary_table, TableStats};
use dql_parser::Value;

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

pub fn web_table_list(
    session: &mut Session,
    pattern: &str,
    refresh: bool,
    background: bool,
) -> Result<crate::web::shape::TableList, String> {
    let pattern = pattern.trim();
    let report = match session.engine.ls_report(
        if pattern.is_empty() {
            None
        } else {
            Some(pattern)
        },
        refresh,
        background,
        false,
    ) {
        Ok(report) => report,
        Err(err) => {
            let message = err.to_string();
            if missing_table(&message) {
                return Ok(listed_tables(
                    Vec::new(),
                    String::new(),
                    session,
                    cache_hint(session).unwrap_or_default(),
                ));
            }
            return Err(message);
        }
    };
    let note = report.note.unwrap_or_default();
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
    let tables = report
        .rows
        .into_iter()
        .map(|row| {
            let meta = row.meta;
            crate::web::shape::ListedTable {
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
            }
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

fn table_stats_with_count(item_count: usize) -> TableStats {
    TableStats {
        item_count,
        size_bytes: 0,
    }
}
