use super::{format_field, Format};
use dql_engine::Item;
use std::collections::BTreeMap;
use std::io::{self, Write};

const MAX_COLUMNS: usize = 15;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RichContext {
    pub important_columns: Vec<String>,
    pub preserve_column_order: bool,
    pub table_hash: Vec<String>,
    pub table_range: Vec<String>,
    pub index_hash: Vec<String>,
    pub index_range: Vec<String>,
}

/// Whether a column is a hash key or a range key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnKey {
    #[default]
    None,
    Hash,
    Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RichColumn {
    pub name: String,
    pub important: bool,
    pub ellipsis: bool,
    pub table_key: ColumnKey,
    pub index_key: ColumnKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RichLayout {
    pub columns: Vec<RichColumn>,
    pub rows: Vec<Vec<String>>,
    pub overflow_columns: Vec<String>,
}

pub struct RichFormat<'a> {
    items: &'a [Item],
    width: usize,
    context: Option<&'a RichContext>,
}

impl<'a> RichFormat<'a> {
    pub fn new(items: &'a [Item], width: usize, context: Option<&'a RichContext>) -> Self {
        Self {
            items,
            width,
            context,
        }
    }
}

impl Format for RichFormat<'_> {
    fn display(&self, writer: &mut dyn Write) -> io::Result<()> {
        if self.items.is_empty() {
            writeln!(writer, "No results")?;
            return Ok(());
        }
        let layout = build_rich_layout(self.items, self.context);
        write_rich_layout(writer, &layout, self.width)
    }
}

pub fn build_rich_layout(items: &[Item], context: Option<&RichContext>) -> RichLayout {
    let all_columns = collect_columns(items);
    if all_columns.is_empty() {
        return RichLayout {
            columns: Vec::new(),
            rows: Vec::new(),
            overflow_columns: Vec::new(),
        };
    }

    let important = context
        .map(|ctx| ctx.important_columns.clone())
        .unwrap_or_default();
    let preserve_order = context
        .map(|ctx| ctx.preserve_column_order)
        .unwrap_or(false);
    let empty: &[String] = &[];
    let table_hash = context
        .map(|ctx| ctx.table_hash.as_slice())
        .unwrap_or(empty);
    let table_range = context
        .map(|ctx| ctx.table_range.as_slice())
        .unwrap_or(empty);
    let index_hash = context
        .map(|ctx| ctx.index_hash.as_slice())
        .unwrap_or(empty);
    let index_range = context
        .map(|ctx| ctx.index_range.as_slice())
        .unwrap_or(empty);
    let ordered = order_columns(all_columns, &important, preserve_order);
    let (displayed, overflow) = split_columns(ordered, &important);

    let columns: Vec<RichColumn> = displayed
        .iter()
        .map(|name| RichColumn {
            important: important.iter().any(|col| col == name),
            ellipsis: name == "...",
            table_key: column_key(table_hash, table_range, name),
            index_key: column_key(index_hash, index_range, name),
            name: name.clone(),
        })
        .collect();

    let rows = items
        .iter()
        .map(|item| {
            displayed
                .iter()
                .map(|col| {
                    if col == "..." {
                        String::new()
                    } else {
                        item.get(col)
                            .map(format_field)
                            .unwrap_or_else(|| "NULL".to_string())
                    }
                })
                .collect()
        })
        .collect();

    RichLayout {
        columns,
        rows,
        overflow_columns: overflow,
    }
}

pub fn rich_layout_to_text(layout: &RichLayout, width: usize) -> String {
    let mut output = Vec::new();
    write_rich_layout(&mut output, layout, width).unwrap();
    String::from_utf8(output).unwrap_or_default()
}

fn write_rich_layout(writer: &mut dyn Write, layout: &RichLayout, width: usize) -> io::Result<()> {
    if layout.columns.is_empty() {
        writeln!(writer, "No results")?;
        return Ok(());
    }

    writeln!(writer, "Results")?;
    write_text_table(writer, &layout.columns, &layout.rows, width)?;

    if !layout.overflow_columns.is_empty() {
        writeln!(writer)?;
        writeln!(writer, "More columns available")?;
        for column in &layout.overflow_columns {
            writeln!(writer, "  {column}")?;
        }
    }
    Ok(())
}

fn write_text_table(
    writer: &mut dyn Write,
    columns: &[RichColumn],
    rows: &[Vec<String>],
    width: usize,
) -> io::Result<()> {
    let names: Vec<_> = columns.iter().map(|col| col.name.as_str()).collect();
    let mut col_width: BTreeMap<String, usize> = BTreeMap::new();
    for name in &names {
        col_width.insert((*name).to_string(), name.chars().count());
    }
    for row in rows {
        for (name, value) in names.iter().zip(row.iter()) {
            let entry = col_width.entry((*name).to_string()).or_insert(0);
            *entry = (*entry).max(value.chars().count());
        }
    }
    for width in col_width.values_mut() {
        *width = width.saturating_add(2);
    }

    let width_requested = 3 + names.len() + col_width.values().sum::<usize>();
    if width_requested > width && !names.is_empty() {
        let even_width = ((width.saturating_sub(1)) / names.len()).saturating_sub(3);
        for name in &names {
            col_width.insert((*name).to_string(), even_width.max(1));
        }
    }

    let mut header = String::from("|");
    for name in &names {
        let w = col_width[*name];
        header.push(' ');
        header.push_str(&truncate(name, w));
        header.push_str(" |");
    }
    writeln!(writer, "{}", "-".repeat(header.len()))?;
    writeln!(writer, "{header}")?;
    writeln!(writer, "{}", "-".repeat(header.len()))?;
    for row in rows {
        let mut line = String::from("|");
        for (name, value) in names.iter().zip(row.iter()) {
            let w = col_width[*name];
            line.push(' ');
            line.push_str(&truncate(value, w));
            line.push_str(" |");
        }
        writeln!(writer, "{line}")?;
    }
    writeln!(writer, "{}", "-".repeat(header.len()))?;
    Ok(())
}

fn collect_columns(items: &[Item]) -> Vec<String> {
    let mut columns = Vec::new();
    for item in items {
        for key in item.keys() {
            if !columns.iter().any(|existing| existing == key) {
                columns.push(key.clone());
            }
        }
    }
    columns
}

fn order_columns(
    mut columns: Vec<String>,
    important: &[String],
    preserve_order: bool,
) -> Vec<String> {
    if preserve_order {
        return columns;
    }
    columns.sort_by(|left, right| {
        let left_rank = important.iter().position(|name| name == left);
        let right_rank = important.iter().position(|name| name == right);
        match (left_rank, right_rank) {
            (Some(left_idx), Some(right_idx)) => left_idx.cmp(&right_idx),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left.to_ascii_uppercase().cmp(&right.to_ascii_uppercase()),
        }
    });
    columns
}

fn split_columns(columns: Vec<String>, important: &[String]) -> (Vec<String>, Vec<String>) {
    let mut displayed = Vec::new();
    let mut overflow = Vec::new();
    let mut remaining = MAX_COLUMNS;
    for column in columns {
        if remaining > 0 {
            displayed.push(column);
            remaining -= 1;
        } else {
            overflow.push(column);
        }
    }
    if !overflow.is_empty() {
        let _ = important;
        displayed.push("...".to_string());
    }
    (displayed, overflow)
}

fn column_key(hash_names: &[String], range_names: &[String], name: &str) -> ColumnKey {
    if hash_names.iter().any(|item| item == name) {
        ColumnKey::Hash
    } else if range_names.iter().any(|item| item == name) {
        ColumnKey::Range
    } else {
        ColumnKey::None
    }
}

fn truncate(string: &str, length: usize) -> String {
    if string.chars().count() <= length {
        return string.to_string();
    }
    let ellipsis = "…";
    let keep = length.saturating_sub(ellipsis.chars().count());
    format!(
        "{}{}",
        string.chars().take(keep).collect::<String>(),
        ellipsis
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use dql_parser::Value;

    fn item(pairs: &[(&str, &str)]) -> Item {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), Value::String(value.to_string())))
            .collect()
    }

    #[test]
    fn rich_layout_orders_primary_key_columns_first() {
        let items = vec![item(&[("zebra", "z"), ("id", "1"), ("name", "n")])];
        let context = RichContext {
            important_columns: vec!["id".to_string()],
            preserve_column_order: false,
            ..RichContext::default()
        };
        let layout = build_rich_layout(&items, Some(&context));
        assert_eq!(
            layout
                .columns
                .iter()
                .map(|col| col.name.as_str())
                .collect::<Vec<_>>(),
            vec!["id", "name", "zebra"]
        );
        assert!(layout.columns[0].important);
    }

    #[test]
    fn rich_layout_marks_table_and_index_keys() {
        let items = vec![item(&[("id", "1"), ("sk", "a"), ("name", "n")])];
        let context = RichContext {
            important_columns: vec!["id".to_string(), "sk".to_string(), "name".to_string()],
            table_hash: vec!["id".to_string()],
            table_range: vec!["sk".to_string()],
            index_hash: vec!["name".to_string()],
            index_range: vec!["sk".to_string()],
            ..RichContext::default()
        };
        let layout = build_rich_layout(&items, Some(&context));
        let column = |name: &str| {
            layout
                .columns
                .iter()
                .find(|column| column.name == name)
                .unwrap()
        };
        assert_eq!(column("id").table_key, ColumnKey::Hash);
        assert_eq!(column("id").index_key, ColumnKey::None);
        assert_eq!(column("sk").table_key, ColumnKey::Range);
        assert_eq!(column("sk").index_key, ColumnKey::Range);
        assert_eq!(column("name").table_key, ColumnKey::None);
        assert_eq!(column("name").index_key, ColumnKey::Hash);
    }

    #[test]
    fn rich_layout_preserves_column_order_when_requested() {
        let items = vec![item(&[("zebra", "z"), ("id", "1"), ("name", "n")])];
        let context = RichContext {
            important_columns: vec!["id".to_string()],
            preserve_column_order: true,
            ..RichContext::default()
        };
        let layout = build_rich_layout(&items, Some(&context));
        assert_eq!(
            layout
                .columns
                .iter()
                .map(|col| col.name.as_str())
                .collect::<Vec<_>>(),
            vec!["id", "name", "zebra"]
        );
    }

    #[test]
    fn rich_layout_overflows_beyond_fifteen_columns() {
        let mut pairs = Vec::new();
        for index in 0..16 {
            pairs.push((format!("col{index:02}"), "v"));
        }
        let items = vec![pairs
            .iter()
            .map(|(key, value)| (key.clone(), Value::String(value.to_string())))
            .collect::<Item>()];
        let layout = build_rich_layout(&items, None);
        assert_eq!(layout.columns.len(), 16);
        assert_eq!(layout.columns.last().unwrap().name, "...");
        assert_eq!(layout.overflow_columns, vec!["col15"]);
    }

    #[test]
    fn rich_format_from_name_round_trip() {
        use crate::OutputFormat;
        assert_eq!(OutputFormat::from_name("rich"), Some(OutputFormat::Rich));
        assert_eq!(OutputFormat::Rich.as_str(), "rich");
    }
}
