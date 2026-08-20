use dql_output::{OutputConfig, OutputFormat, PageSize, WidthSetting};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const DEFAULT_DISPLAY: &str = "stdout";
pub const DEFAULT_FORMAT: &str = "smart";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CliConfig {
    #[serde(default = "default_width")]
    pub width: serde_json::Value,
    #[serde(default = "default_pagesize")]
    pub pagesize: serde_json::Value,
    #[serde(default = "default_display")]
    pub display: String,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default)]
    pub allow_select_scan: bool,
    #[serde(default = "default_true")]
    pub lossy_json_float: bool,
    #[serde(default = "default_throttle", rename = "_throttle")]
    pub throttle: serde_json::Value,
}

fn default_throttle() -> serde_json::Value {
    serde_json::json!({})
}

fn default_width() -> serde_json::Value {
    serde_json::Value::String("auto".to_string())
}

fn default_pagesize() -> serde_json::Value {
    serde_json::Value::String("auto".to_string())
}

fn default_display() -> String {
    DEFAULT_DISPLAY.to_string()
}

fn default_format() -> String {
    DEFAULT_FORMAT.to_string()
}

fn default_true() -> bool {
    true
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            width: default_width(),
            pagesize: default_pagesize(),
            display: default_display(),
            format: default_format(),
            allow_select_scan: false,
            lossy_json_float: true,
            throttle: default_throttle(),
        }
    }
}

impl CliConfig {
    pub fn config_dir() -> PathBuf {
        dirs_config()
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("dql.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if !path.exists() {
            return Self::default();
        }
        fs::read_to_string(path)
            .ok()
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .map(|mut loaded: Self| {
                let defaults = Self::default();
                if loaded.width.is_null() {
                    loaded.width = defaults.width;
                }
                if loaded.pagesize.is_null() {
                    loaded.pagesize = defaults.pagesize;
                }
                if loaded.display.is_empty() {
                    loaded.display = defaults.display;
                }
                if loaded.format.is_empty() {
                    loaded.format = defaults.format;
                }
                loaded
            })
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let dir = Self::config_dir();
        fs::create_dir_all(&dir)?;
        let contents = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        fs::write(Self::config_path(), contents)
    }

    pub fn output_config(&self) -> OutputConfig {
        OutputConfig {
            width: WidthSetting::from_config(&self.width),
            pagesize: PageSize::from_config(&self.pagesize),
            format: OutputFormat::from_name(&self.format).unwrap_or(OutputFormat::Smart),
            lossy_json_float: self.lossy_json_float,
            silent: false,
        }
    }

    pub fn public_keys(&self) -> Vec<String> {
        vec![
            "width".to_string(),
            "pagesize".to_string(),
            "display".to_string(),
            "format".to_string(),
            "allow_select_scan".to_string(),
            "lossy_json_float".to_string(),
        ]
    }

    pub fn get_value(&self, key: &str) -> String {
        match key {
            "width" => self.width.to_string(),
            "pagesize" => self.pagesize.to_string(),
            "display" => self.config_display(),
            "format" => self.format.clone(),
            "allow_select_scan" => self.allow_select_scan.to_string(),
            "lossy_json_float" => self.lossy_json_float.to_string(),
            _ => String::new(),
        }
    }

    fn config_display(&self) -> String {
        self.display.clone()
    }
}

fn dirs_config() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(dir);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config");
    }
    PathBuf::from(".config")
}
