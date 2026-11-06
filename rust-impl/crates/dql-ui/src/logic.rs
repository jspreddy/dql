//! Pure studio rules shared by the window and the headless tests.

use crate::host::TreeNode;

pub const PAGE_SIZE: usize = 50;
pub const SEARCH_DEBOUNCE_MS: u64 = 150;
pub const CATALOG_DEBOUNCE_MS: u64 = 250;
pub const AUTOSAVE_MS: u64 = 400;
pub const NAV_DEFAULT: f32 = 220.0;
pub const RESULTS_DEFAULT: f32 = 0.46;
pub const NAV_MIN: f32 = 140.0;
pub const NAV_MAX: f32 = 640.0;
pub const PANE_MIN: f32 = 140.0;
pub const RESULTS_MIN: f32 = 120.0;
pub const QUERY_COLUMN: f32 = 520.0;

const KEYWORDS: &[&str] = &[
    "ALTER", "ANALYZE", "CREATE", "DELETE", "DESCRIBE", "DROP", "EXPLAIN", "HELP", "INSERT",
    "LOAD", "SCAN", "SELECT", "SHOW", "UPDATE",
];

pub fn clamp_nav(width: f32) -> f32 {
    width.clamp(NAV_MIN, NAV_MAX)
}

pub fn clamp_ratio(ratio: f32) -> f32 {
    let min = RESULTS_MIN / QUERY_COLUMN;
    let max = (QUERY_COLUMN - PANE_MIN) / QUERY_COLUMN;
    ratio.clamp(min, max)
}

pub fn pager_label(page: usize, total: usize) -> String {
    if total == 0 {
        return "Showing 0–0".into();
    }
    let start = page * PAGE_SIZE + 1;
    let end = ((page + 1) * PAGE_SIZE).min(total);
    format!("Showing {start}–{end}")
}

pub fn page_count(total: usize) -> usize {
    if total == 0 {
        1
    } else {
        total.div_ceil(PAGE_SIZE)
    }
}

pub fn completion_for(text: &str, head: usize) -> Option<String> {
    let head = head.min(text.len());
    let start = text[..head]
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_ascii_alphanumeric())
        .map(|(index, ch)| index + ch.len_utf8())
        .unwrap_or(0);
    let word = &text[start..head];
    if word.len() < 2 {
        return None;
    }
    let upper = word.to_ascii_uppercase();
    KEYWORDS.iter().find_map(|keyword| {
        if keyword.starts_with(&upper) && !keyword.eq_ignore_ascii_case(word) {
            Some((*keyword).to_string())
        } else {
            None
        }
    })
}

pub fn unified_diff(disk: &str, local: &str) -> String {
    let mut out = String::from("--- disk\n+++ local\n");
    for line in disk.lines() {
        out.push('-');
        out.push_str(line);
        out.push('\n');
    }
    for line in local.lines() {
        out.push('+');
        out.push_str(line);
        out.push('\n');
    }
    out
}

pub fn group_of(kind: &str) -> &'static str {
    match kind {
        "table" => "TABLES",
        "column" | "index" => "COLUMNS",
        _ => "QUERIES",
    }
}

pub fn preferred_file(nodes: &[TreeNode]) -> Option<String> {
    fn walk(nodes: &[TreeNode], found: &mut Option<String>, first: &mut Option<String>) {
        for node in nodes {
            if node.kind == "file" {
                if first.is_none() {
                    *first = Some(node.path.clone());
                }
                if node.path == "queries/edit.dql" {
                    *found = Some(node.path.clone());
                }
            }
            walk(&node.children, found, first);
        }
    }
    let mut found = None;
    let mut first = None;
    walk(nodes, &mut found, &mut first);
    found.or(first)
}

/// Split `empty/moved.dql` into the directory and the final file name.
pub fn move_dest(dest: &str) -> (&str, &str) {
    let dest = dest.trim().trim_matches('/');
    match dest.rfind('/') {
        Some(index) => (&dest[..index], &dest[index + 1..]),
        None => ("", dest),
    }
}

pub fn similar_label(pattern: &str, exact: bool, related: bool) -> Option<String> {
    if pattern.is_empty() || exact {
        return None;
    }
    if related {
        Some(format!(
            "No exact match for \"{pattern}\", so showing related keys."
        ))
    } else {
        Some(format!(
            "No exact match for \"{pattern}\", so showing similar names."
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pager_uses_pages_of_fifty() {
        assert_eq!(pager_label(0, 51), "Showing 1–50");
        assert_eq!(pager_label(1, 51), "Showing 51–51");
        assert_eq!(page_count(51), 2);
        assert_eq!(page_count(0), 1);
    }

    #[test]
    fn completion_offers_the_longer_keyword() {
        assert_eq!(completion_for("sel", 3).as_deref(), Some("SELECT"));
        assert_eq!(completion_for("SELECT", 6), None);
        assert_eq!(completion_for("s", 1), None);
    }

    #[test]
    fn the_diff_marks_the_local_line() {
        let diff = unified_diff("disk\n", "-- local unsaved\n");
        assert!(diff.contains("+-- local unsaved"));
        assert!(diff.contains("-disk"));
    }

    #[test]
    fn move_dest_splits_the_prompt() {
        assert_eq!(move_dest("empty/moved.dql"), ("empty", "moved.dql"));
        assert_eq!(move_dest("moved.dql"), ("", "moved.dql"));
    }

    #[test]
    fn similar_names_and_related_keys() {
        assert!(similar_label("", false, false).is_none());
        assert!(similar_label("widgets", true, false).is_none());
        assert!(similar_label("wid", false, false)
            .unwrap()
            .contains("similar names"));
        assert!(similar_label("id", false, true)
            .unwrap()
            .contains("related keys"));
    }

    #[test]
    fn nav_and_results_stay_in_range() {
        assert_eq!(clamp_nav(10.0), NAV_MIN);
        assert_eq!(clamp_nav(900.0), NAV_MAX);
        assert!(clamp_ratio(0.01) * QUERY_COLUMN >= RESULTS_MIN - 0.1);
        assert!((1.0 - clamp_ratio(0.99)) * QUERY_COLUMN >= PANE_MIN - 0.1);
    }
}
