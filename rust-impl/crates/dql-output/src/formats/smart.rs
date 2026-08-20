use super::{ColumnFormat, ExpandedFormat, Format};
use dql_engine::Item;

pub struct SmartFormat<'a> {
    inner: Box<dyn Format + 'a>,
}

impl<'a> SmartFormat<'a> {
    pub fn new(items: &'a [Item], width: usize, pagesize: usize, lossy_json_float: bool) -> Self {
        let _ = lossy_json_float;
        let _column_width = ColumnFormat::new(items, width, pagesize);
        let inner: Box<dyn Format + 'a> = if column_width_needs_expanded(items, width) {
            Box::new(ExpandedFormat::new(items, width, pagesize))
        } else {
            Box::new(ColumnFormat::new(items, width, pagesize))
        };
        Self { inner }
    }
}

impl Format for SmartFormat<'_> {
    fn display(&self, writer: &mut dyn io::Write) -> std::io::Result<()> {
        self.inner.display(writer)
    }
}

use std::io;

fn column_width_needs_expanded(items: &[Item], width: usize) -> bool {
    if items.is_empty() {
        return false;
    }
    let mut col_width = std::collections::BTreeMap::new();
    for item in items {
        for (key, value) in item {
            let entry = col_width.entry(key.clone()).or_insert(key.len());
            *entry = (*entry).max(super::format_field(value).len());
        }
    }
    let columns: Vec<_> = col_width.keys().cloned().collect();
    let width_requested = 3 + columns.len() + col_width.values().sum::<usize>();
    width_requested > width
}
