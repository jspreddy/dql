mod column;
mod expanded;
mod json;
mod rich;
mod smart;

pub use column::ColumnFormat;
pub use expanded::ExpandedFormat;
pub use json::JsonFormat;
pub use rich::{
    build_rich_layout, rich_layout_to_text, RichColumn, RichContext, RichFormat, RichLayout,
};
pub use smart::SmartFormat;

use std::io::{self, Write};

pub trait Format {
    fn display(&self, writer: &mut dyn Write) -> io::Result<()>;
}

pub fn format_field(value: &dql_parser::Value) -> String {
    use dql_parser::Value;
    match value {
        Value::Null => "NULL".to_string(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.clone(),
        Value::String(v) => v.clone(),
        Value::Binary(v) => format!("<Binary {}>", v.len()),
        Value::Timestamp(v) => format!("{v:?}"),
        Value::Interval(v) => v.clone(),
        Value::List(values) | Value::Set(values) => {
            let parts: Vec<_> = values.iter().map(format_field).collect();
            format!("({})", parts.join(", "))
        }
        Value::Map(map) => {
            let parts: Vec<_> = map
                .iter()
                .map(|(k, v)| format!("{k}: {}", format_field(v)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

#[allow(dead_code)]
pub fn write_paged(
    writer: &mut dyn Write,
    pagesize: usize,
    total: usize,
    mut write_page: impl FnMut(&mut dyn Write) -> io::Result<()>,
) -> io::Result<()> {
    if total == 0 {
        writeln!(writer, "No results")?;
        return Ok(());
    }
    if pagesize == 0 || total <= pagesize {
        write_page(writer)?;
        return Ok(());
    }
    write_page(writer)?;
    Ok(())
}
