use crate::formats::{
    ColumnFormat, ExpandedFormat, Format, JsonFormat, RichContext, RichFormat, SmartFormat,
};
use dql_engine::Item;

#[derive(Debug, Clone, PartialEq)]
pub enum WidthSetting {
    Auto,
    Fixed(usize),
}

impl WidthSetting {
    pub fn resolve(&self) -> usize {
        match self {
            Self::Auto => terminal_width(),
            Self::Fixed(width) => *width,
        }
    }

    pub fn from_config(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::String(s) if s == "auto" => Self::Auto,
            serde_json::Value::Number(n) => Self::Fixed(n.as_u64().unwrap_or(80) as usize),
            _ => Self::Auto,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Auto => serde_json::Value::String("auto".to_string()),
            Self::Fixed(width) => serde_json::Value::Number((*width as u64).into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputFormat {
    Smart,
    Column,
    Expanded,
    Json,
    Rich,
}

impl OutputFormat {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "smart" => Some(Self::Smart),
            "column" => Some(Self::Column),
            "expanded" => Some(Self::Expanded),
            "json" => Some(Self::Json),
            "rich" => Some(Self::Rich),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Smart => "smart",
            Self::Column => "column",
            Self::Expanded => "expanded",
            Self::Json => "json",
            Self::Rich => "rich",
        }
    }

    pub fn all_names() -> &'static [&'static str] {
        &["smart", "column", "expanded", "json", "rich"]
    }
}

#[derive(Debug, Clone)]
pub struct OutputConfig {
    pub width: WidthSetting,
    pub pagesize: PageSize,
    pub format: OutputFormat,
    pub lossy_json_float: bool,
    pub silent: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PageSize {
    Auto,
    Fixed(usize),
}

impl PageSize {
    pub fn resolve(&self) -> usize {
        match self {
            Self::Auto => terminal_height().saturating_sub(5),
            Self::Fixed(size) => *size,
        }
    }

    pub fn from_config(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::String(s) if s == "auto" => Self::Auto,
            serde_json::Value::Number(n) => Self::Fixed(n.as_u64().unwrap_or(0) as usize),
            _ => Self::Auto,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Auto => serde_json::Value::String("auto".to_string()),
            Self::Fixed(size) => serde_json::Value::Number((*size as u64).into()),
        }
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            width: WidthSetting::Auto,
            pagesize: PageSize::Auto,
            format: OutputFormat::Smart,
            lossy_json_float: true,
            silent: false,
        }
    }
}

impl OutputConfig {
    pub fn for_json_command() -> Self {
        Self {
            pagesize: PageSize::Fixed(0),
            format: OutputFormat::Json,
            silent: true,
            ..Self::default()
        }
    }

    pub fn formatter<'a>(
        &self,
        items: &'a [Item],
        rich_context: Option<&'a RichContext>,
    ) -> Box<dyn Format + 'a> {
        match self.format {
            OutputFormat::Json => Box::new(JsonFormat::new(items, self.lossy_json_float)),
            OutputFormat::Column => Box::new(ColumnFormat::new(
                items,
                self.width.resolve(),
                self.pagesize.resolve(),
            )),
            OutputFormat::Expanded => Box::new(ExpandedFormat::new(
                items,
                self.width.resolve(),
                self.pagesize.resolve(),
            )),
            OutputFormat::Rich => {
                Box::new(RichFormat::new(items, self.width.resolve(), rich_context))
            }
            OutputFormat::Smart => Box::new(SmartFormat::new(
                items,
                self.width.resolve(),
                self.pagesize.resolve(),
                self.lossy_json_float,
            )),
        }
    }
}

fn terminal_width() -> usize {
    crossterm::terminal::size()
        .map(|(_, cols)| cols as usize)
        .unwrap_or(80)
        .max(40)
}

fn terminal_height() -> usize {
    crossterm::terminal::size()
        .map(|(rows, _)| rows as usize)
        .unwrap_or(24)
}
