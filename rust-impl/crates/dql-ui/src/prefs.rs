//! Window preferences. `write_bands` from older files is read and ignored.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tokens::HighlightOptions;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prefs {
    #[serde(default)]
    pub dark: bool,
    #[serde(default = "default_nav")]
    pub nav: f32,
    #[serde(default = "default_results")]
    pub results: f32,
    #[serde(default)]
    pub minimal_write: bool,
    #[serde(default = "default_even")]
    pub even_odd: bool,
    #[serde(default)]
    pub explain_json: bool,
    #[serde(default)]
    pub write_bands: Option<bool>,
}

fn default_nav() -> f32 {
    220.0
}

fn default_results() -> f32 {
    0.46
}

fn default_even() -> bool {
    true
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            dark: false,
            nav: 220.0,
            results: 0.46,
            minimal_write: false,
            even_odd: true,
            explain_json: false,
            write_bands: None,
        }
    }
}

impl Prefs {
    pub fn options(&self) -> HighlightOptions {
        HighlightOptions {
            minimal_write: self.minimal_write,
            even_odd: self.even_odd,
        }
    }

    pub fn capture(
        dark: bool,
        nav: f32,
        results: f32,
        options: HighlightOptions,
        explain_json: bool,
    ) -> Self {
        Self {
            dark,
            nav,
            results,
            minimal_write: options.minimal_write,
            even_odd: options.even_odd,
            explain_json,
            write_bands: None,
        }
    }
}

pub fn path_for(workspace: &Path) -> PathBuf {
    if let Some(path) = std::env::var_os("DQLRS_UI_PREFS") {
        return PathBuf::from(path);
    }
    workspace.join(".dqlrs-ui.json")
}

pub fn load(path: &Path) -> Prefs {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Prefs::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save(path: &Path, prefs: &Prefs) {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    if let Ok(text) = serde_json::to_string_pretty(prefs) {
        let _ = std::fs::write(path, text);
    }
}
