mod config;
mod display;
mod formats;
mod table_meta;

pub use config::{OutputConfig, OutputFormat, PageSize, WidthSetting};
pub use display::{less_display, stdout_display, DisplayBackend, DisplayMode};
pub use formats::{ColumnFormat, ExpandedFormat, Format, JsonFormat, SmartFormat};
pub use table_meta::{
    format_table_detail, format_table_summary_table, format_table_summary_table_with_width,
    TableStats,
};

use dql_engine::{Item, StatementResult};
use std::io;

pub fn render_result(
    result: &StatementResult,
    config: &OutputConfig,
    backend: &mut dyn DisplayBackend,
) -> io::Result<()> {
    match result {
        StatementResult::None => Ok(()),
        StatementResult::Status(status) if !config.silent => backend.write_line(status),
        StatementResult::Affected(count) if !config.silent => {
            backend.write_line(&format!("{count} item(s) affected"))
        }
        StatementResult::Schema(schema) => backend.write_line(schema),
        StatementResult::Items(items) => {
            let mut writer = backend.writer();
            let formatter = config.formatter(items);
            formatter.display(&mut writer)
        }
        StatementResult::Status(_) | StatementResult::Affected(_) => Ok(()),
    }
}

pub fn format_items(items: &[Item], config: &OutputConfig) -> String {
    let mut output = Vec::new();
    let formatter = config.formatter(items);
    formatter.display(&mut output).unwrap();
    String::from_utf8(output).unwrap_or_default()
}
