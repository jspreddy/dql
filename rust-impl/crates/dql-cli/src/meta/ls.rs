use crate::session::Session;
use dql_output::{format_table_detail, format_table_summary_table, TableStats};
use std::collections::HashMap;
use std::io::Write;

pub fn handle(
    session: &mut Session,
    args: &[String],
    kwargs: &HashMap<String, String>,
    out: &mut dyn Write,
    _repl: bool,
) -> Result<(), String> {
    let refresh = parse_bool(kwargs.get("refresh"), false);
    let metrics = parse_bool(kwargs.get("metrics"), false);
    let _ = metrics;
    if args.is_empty() {
        let tables = session
            .engine
            .describe_all(refresh)
            .map_err(|err| err.to_string())?;
        let rows = tables
            .into_iter()
            .map(|meta| {
                let count = session.engine.table_item_count(&meta.name);
                (
                    meta,
                    TableStats {
                        item_count: count,
                        size_bytes: 0,
                    },
                )
            })
            .collect::<Vec<_>>();
        writeln!(
            out,
            "{}",
            format_table_summary_table(&rows)
        )
        .map_err(|err| err.to_string())?;
        return Ok(());
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
                .describe(name, refresh)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| format!("Table {name:?} not found"))?;
            let count = session.engine.table_item_count(name);
            writeln!(
                out,
                "{}",
                format_table_detail(
                    &meta,
                    &TableStats {
                        item_count: count,
                        size_bytes: 0,
                    }
                )
            )
            .map_err(|err| err.to_string())?;
            Ok(())
        }
        _ => {
            let mut rows = Vec::new();
            for name in filtered {
                let meta = session
                    .engine
                    .describe(&name, refresh)
                    .map_err(|err| err.to_string())?
                    .ok_or_else(|| format!("Table {name:?} not found"))?;
                let count = session.engine.table_item_count(&name);
                rows.push((
                    meta,
                    TableStats {
                        item_count: count,
                        size_bytes: 0,
                    },
                ));
            }
            writeln!(
                out,
                "{}",
                format_table_summary_table(&rows)
            )
            .map_err(|err| err.to_string())?;
            Ok(())
        }
    }
}

fn parse_bool(value: Option<&String>, default: bool) -> bool {
    value
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(default)
}
