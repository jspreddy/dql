mod config;
mod display;
mod formats;
mod table_meta;

pub use config::{OutputConfig, OutputFormat, PageSize, WidthSetting};
pub use display::{less_display, stdout_display, DisplayBackend, DisplayMode};
pub use formats::{
    build_rich_layout, rich_layout_to_text, ColumnFormat, ExpandedFormat, Format, JsonFormat,
    RichColumn, RichContext, RichFormat, RichLayout, SmartFormat,
};
pub use table_meta::{format_table_detail, format_table_summary_table, TableStats};

use dql_engine::{Item, StatementResult};
use std::io;

pub fn render_result(
    result: &StatementResult,
    config: &OutputConfig,
    backend: &mut dyn DisplayBackend,
    rich_context: Option<&RichContext>,
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
            let formatter = config.formatter(items, rich_context);
            formatter.display(&mut writer)
        }
        StatementResult::ItemsWithNote { items, note } => {
            let listed = is_formatted_table_list(note);
            if !config.silent {
                backend.write_line(note)?;
            }
            if config.silent || !listed {
                let mut writer = backend.writer();
                let formatter = config.formatter(items, rich_context);
                formatter.display(&mut writer)?;
            }
            Ok(())
        }
        StatementResult::Status(_) | StatementResult::Affected(_) => Ok(()),
    }
}

fn is_formatted_table_list(note: &str) -> bool {
    let body = note.trim_start();
    body.starts_with("Tables\n") || body.contains("\nTables\n")
}

pub fn format_items(
    items: &[Item],
    config: &OutputConfig,
    rich_context: Option<&RichContext>,
) -> String {
    let mut output = Vec::new();
    let formatter = config.formatter(items, rich_context);
    formatter.display(&mut output).unwrap();
    String::from_utf8(output).unwrap_or_default()
}
