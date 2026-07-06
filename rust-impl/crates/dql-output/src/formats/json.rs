use super::Format;
use dql_engine::json_util::{item_to_json, string_to_json};
use dql_engine::Item;
use std::io::{self, Write};

pub struct JsonFormat<'a> {
    items: &'a [Item],
    lossy_json_float: bool,
}

impl<'a> JsonFormat<'a> {
    pub fn new(items: &'a [Item], lossy_json_float: bool) -> Self {
        Self {
            items,
            lossy_json_float,
        }
    }
}

impl Format for JsonFormat<'_> {
    fn display(&self, writer: &mut dyn Write) -> io::Result<()> {
        let _ = self.lossy_json_float;
        if self.items.is_empty() {
            return Ok(());
        }
        for item in self.items {
            write!(writer, "{}", item_to_json(item, 0))?;
            writeln!(writer)?;
        }
        Ok(())
    }
}

pub fn schema_to_json(schema: &str) -> String {
    string_to_json(schema)
}
