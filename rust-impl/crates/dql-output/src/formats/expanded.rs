use super::{format_field, Format};
use dql_engine::Item;
use std::io::{self, Write};

pub struct ExpandedFormat<'a> {
    items: &'a [Item],
    width: usize,
    pagesize: usize,
}

impl<'a> ExpandedFormat<'a> {
    pub fn new(items: &'a [Item], width: usize, pagesize: usize) -> Self {
        let pagesize = if pagesize == 0 { 1 } else { pagesize };
        Self {
            items,
            width,
            pagesize,
        }
    }
}

impl Format for ExpandedFormat<'_> {
    fn display(&self, writer: &mut dyn Write) -> io::Result<()> {
        if self.items.is_empty() {
            writeln!(writer, "No results")?;
            return Ok(());
        }
        for item in self.items {
            writeln!(writer, "{}", "-".repeat(self.width))?;
            let max_key = item.keys().map(|k| k.len()).max().unwrap_or(0);
            for (key, value) in item {
                let formatted = format_field(value);
                writeln!(writer, "{:>width$} : {formatted}", key, width = max_key)?;
            }
        }
        Ok(())
    }
}
