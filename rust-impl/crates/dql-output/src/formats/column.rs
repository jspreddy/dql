use super::{format_field, Format};
use dql_engine::Item;
use std::collections::BTreeMap;
use std::io::{self, Write};

pub struct ColumnFormat<'a> {
    items: &'a [Item],
    pagesize: usize,
    columns: Vec<String>,
    col_width: BTreeMap<String, usize>,
    header: String,
}

impl<'a> ColumnFormat<'a> {
    pub fn new(items: &'a [Item], width: usize, pagesize: usize) -> Self {
        let mut col_width: BTreeMap<String, usize> = BTreeMap::new();
        for item in items {
            for (key, value) in item {
                let entry = col_width.entry(key.clone()).or_insert(key.len());
                *entry = (*entry).max(format_field(value).len());
            }
        }
        let columns: Vec<_> = col_width.keys().cloned().collect();
        let width_requested = 3 + columns.len() + col_width.values().sum::<usize>();
        if width_requested > width && !columns.is_empty() {
            let even_width = ((width.saturating_sub(1)) / columns.len()).saturating_sub(3);
            for key in &columns {
                col_width.insert(key.clone(), even_width.max(1));
            }
        }
        let mut header = String::from("|");
        for col in &columns {
            let w = col_width[col];
            header.push(' ');
            header.push_str(&truncate_center(col, w));
            header.push_str(" |");
        }
        Self {
            items,
            pagesize,
            columns,
            col_width,
            header,
        }
    }
}

impl Format for ColumnFormat<'_> {
    fn display(&self, writer: &mut dyn Write) -> io::Result<()> {
        if self.items.is_empty() {
            writeln!(writer, "No results")?;
            return Ok(());
        }
        let pagesize = if self.pagesize == 0 {
            self.items.len()
        } else {
            self.pagesize
        };
        for (page_start, _) in self
            .items
            .chunks(pagesize)
            .enumerate()
            .map(|(i, chunk)| (i * pagesize, chunk))
        {
            if page_start > 0 {
                writeln!(writer)?;
            }
            writeln!(writer, "{}", "-".repeat(self.header.len()))?;
            writeln!(writer, "{}", self.header)?;
            writeln!(writer, "{}", "-".repeat(self.header.len()))?;
            let end = (page_start + pagesize).min(self.items.len());
            for item in &self.items[page_start..end] {
                let mut row = String::from("|");
                for col in &self.columns {
                    let w = self.col_width[col];
                    row.push(' ');
                    let value = item
                        .get(col)
                        .map(format_field)
                        .unwrap_or_else(|| "NULL".to_string());
                    row.push_str(&truncate(&value, w));
                    row.push_str(" |");
                }
                writeln!(writer, "{row}")?;
            }
            writeln!(writer, "{}", "-".repeat(self.header.len()))?;
        }
        Ok(())
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

fn truncate_center(string: &str, length: usize) -> String {
    truncate(string, length)
}
