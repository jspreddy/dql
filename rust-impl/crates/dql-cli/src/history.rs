use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

pub struct HistoryManager {
    initial_history_length: usize,
    in_memory: VecDeque<String>,
    history_dir: Option<PathBuf>,
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoryManager {
    pub const HISTORY_FILE_NAME: &'static str = "history";

    pub fn new() -> Self {
        Self {
            initial_history_length: 0,
            in_memory: VecDeque::new(),
            history_dir: None,
        }
    }

    pub fn with_dir(mut self, dir: PathBuf) -> Self {
        self.history_dir = Some(dir);
        self
    }

    fn history_dir(&self) -> PathBuf {
        self.history_dir.clone().unwrap_or_else(default_history_dir)
    }

    pub fn history_file(&self) -> PathBuf {
        self.history_dir().join(Self::HISTORY_FILE_NAME)
    }

    fn prep_history_file(&self) -> io::Result<PathBuf> {
        let dir = self.history_dir();
        fs::create_dir_all(&dir)?;
        let file = self.history_file();
        OpenOptions::new().create(true).append(true).open(&file)?;
        Ok(file)
    }

    pub fn try_to_load_history(&mut self) {
        if let Err(err) = self.load_history() {
            eprintln!("Error reading history file: {err}");
            self.initial_history_length = 0;
        }
    }

    fn load_history(&mut self) -> io::Result<()> {
        let _ = self.prep_history_file()?;
        let file = fs::File::open(self.history_file())?;
        let reader = io::BufReader::new(file);
        self.in_memory.clear();
        for line in reader.lines() {
            self.in_memory.push_back(decode_history_line(&line?));
        }
        self.initial_history_length = self.in_memory.len();
        Ok(())
    }

    pub fn try_to_write_history(&mut self) {
        if let Err(err) = self.prep_history_file().map(|_| ()) {
            eprintln!("Error writing history file: {err}");
            return;
        }
        if let Err(err) = self.write_history() {
            eprintln!("Error writing history file: {err}");
        }
    }

    fn write_history(&mut self) -> io::Result<()> {
        let current = self.in_memory.len();
        let new_entries = current.saturating_sub(self.initial_history_length);
        if new_entries == 0 {
            return Ok(());
        }
        if current < self.initial_history_length {
            return Err(io::Error::other(format!(
                "Unable to write new history. Length is less than 0. ({current} - {})",
                self.initial_history_length
            )));
        }
        let file = self.prep_history_file()?;
        let mut handle = OpenOptions::new().append(true).open(file)?;
        for entry in self
            .in_memory
            .iter()
            .skip(self.initial_history_length)
            .take(new_entries)
        {
            writeln!(handle, "{}", encode_history_line(entry))?;
        }
        self.initial_history_length = current;
        Ok(())
    }

    pub fn add_entry(&mut self, entry: impl Into<String>) {
        let entry = entry.into();
        if self.in_memory.back().is_some_and(|last| last == &entry) {
            return;
        }
        self.in_memory.push_back(entry);
    }

    pub fn entries(&self) -> &VecDeque<String> {
        &self.in_memory
    }

    pub fn remove_items(&mut self, n: usize) -> Result<(), String> {
        if n == 0 {
            return Ok(());
        }
        let current = self.in_memory.len();
        if current.saturating_sub(n) < self.initial_history_length {
            return Err(format!(
                "Requested history item removal is not in current session history range. ({}, {current})",
                self.initial_history_length
            ));
        }
        for _ in 0..n {
            self.in_memory.pop_back();
        }
        Ok(())
    }

    /// Persist any pending session entries, then return decoded history entries
    /// in chronological order (oldest first).
    pub fn snapshot(&mut self) -> Vec<String> {
        self.try_to_write_history();
        self.in_memory.iter().cloned().collect()
    }

    /// Drop skippable commands and duplicate entries, keeping the most recent
    /// occurrence of each unique command. Rewrites the history file.
    /// Returns `(kept, removed)`.
    pub fn dedupe(&mut self) -> Result<(usize, usize), String> {
        self.try_to_write_history();
        let before = self.in_memory.len();
        let mut kept_rev = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for entry in self.in_memory.iter().rev() {
            if !should_record_history(entry) {
                continue;
            }
            if seen.insert(entry.clone()) {
                kept_rev.push(entry.clone());
            }
        }
        kept_rev.reverse();
        let kept = kept_rev.len();
        let removed = before.saturating_sub(kept);
        self.replace_all(kept_rev)
            .map_err(|err| format!("Failed to rewrite history file: {err}"))?;
        Ok((kept, removed))
    }

    /// Remove every history entry that exactly equals `exact`. Rewrites the file.
    /// Returns the number of removed entries.
    pub fn remove_exact(&mut self, exact: &str) -> Result<usize, String> {
        self.try_to_write_history();
        let before = self.in_memory.len();
        let kept: Vec<String> = self
            .in_memory
            .iter()
            .filter(|entry| entry.as_str() != exact)
            .cloned()
            .collect();
        let removed = before.saturating_sub(kept.len());
        self.replace_all(kept)
            .map_err(|err| format!("Failed to rewrite history file: {err}"))?;
        Ok(removed)
    }

    fn replace_all(&mut self, entries: Vec<String>) -> io::Result<()> {
        let file = self.prep_history_file()?;
        let mut handle = fs::File::create(file)?;
        for entry in &entries {
            writeln!(handle, "{}", encode_history_line(entry))?;
        }
        self.in_memory = entries.into();
        self.initial_history_length = self.in_memory.len();
        Ok(())
    }
}

fn default_history_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".dql");
    }
    PathBuf::from(".dql")
}

/// Whether a completed command should be stored in Up/Down history.
pub fn should_record_history(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let mut parts = trimmed.split_whitespace();
    let cmd = parts.next().unwrap_or("").to_ascii_lowercase();
    match cmd.as_str() {
        "clear" | "cls" | "c" | "exit" | "quit" | "help" | "history" => false,
        // Bare `ls` is omitted; `ls <tablename>` (or any args) is kept.
        "ls" => parts.next().is_some(),
        _ => true,
    }
}

/// Encode embedded newlines so each history entry stays on one physical file line.
fn encode_history_line(entry: &str) -> String {
    let mut out = String::with_capacity(entry.len());
    for ch in entry.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(ch),
        }
    }
    out
}

fn decode_history_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod encode_tests {
    use super::{decode_history_line, encode_history_line, should_record_history};

    #[test]
    fn roundtrips_multiline_commands() {
        let entry = "scan * from t\nwhere x = 1;";
        let encoded = encode_history_line(entry);
        assert!(!encoded.contains('\n'));
        assert_eq!(decode_history_line(&encoded), entry);
    }

    #[test]
    fn filters_meta_and_bare_ls() {
        assert!(!should_record_history("ls"));
        assert!(!should_record_history("clear"));
        assert!(!should_record_history("exit"));
        assert!(!should_record_history("c"));
        assert!(!should_record_history("help"));
        assert!(!should_record_history("history"));
        assert!(!should_record_history("history dedupe"));
        assert!(should_record_history("ls mytable"));
        assert!(should_record_history("scan * from t;"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_history_file_is_created_on_load() {
        let dir = tempdir().unwrap();
        let mut history = HistoryManager::new().with_dir(dir.path().to_path_buf());
        history.try_to_load_history();
        assert!(history.history_file().is_file());
    }

    #[test]
    fn test_history_file_is_created_on_write() {
        let dir = tempdir().unwrap();
        let mut history = HistoryManager::new().with_dir(dir.path().to_path_buf());
        history.try_to_write_history();
        assert!(history.history_file().is_file());
    }

    #[test]
    fn test_history_file_contains_history_from_readline() {
        let dir = tempdir().unwrap();
        let mut history = HistoryManager::new().with_dir(dir.path().to_path_buf());
        history.add_entry("this is a simulated cli input");
        history.try_to_write_history();
        let contents = fs::read_to_string(history.history_file()).unwrap();
        assert_eq!(contents, "this is a simulated cli input\n");
    }

    #[test]
    fn test_history_file_contains_proper_appended_history() {
        let dir = tempdir().unwrap();
        let mut history = HistoryManager::new().with_dir(dir.path().to_path_buf());
        history.add_entry("this is a simulated cli input");
        history.try_to_write_history();
        history.try_to_load_history();
        history.add_entry("another simulated cli input");
        history.try_to_write_history();
        let contents = fs::read_to_string(history.history_file()).unwrap();
        assert_eq!(
            contents,
            "this is a simulated cli input\nanother simulated cli input\n"
        );
    }

    #[test]
    fn test_write_history_handles_append_failure() {
        let dir = tempdir().unwrap();
        let mut history = HistoryManager::new().with_dir(dir.path().to_path_buf());
        history.add_entry("this is a simulated cli input");
        fs::write(history.history_file(), "").unwrap();
        let readonly = history.history_file();
        let mut perms = fs::metadata(&readonly).unwrap().permissions();
        perms.set_readonly(true);
        let _ = fs::set_permissions(&readonly, perms);
        history.try_to_write_history();
    }

    #[test]
    fn test_write_history_handles_get_length_failure() {
        let dir = tempdir().unwrap();
        let mut history = HistoryManager::new().with_dir(dir.path().to_path_buf());
        history.try_to_write_history();
    }

    #[test]
    fn test_remove_items_handles_readline_failure() {
        let dir = tempdir().unwrap();
        let mut history = HistoryManager::new().with_dir(dir.path().to_path_buf());
        history.add_entry("this is a simulated cli input");
        assert!(history.remove_items(1).is_ok());
    }

    #[test]
    fn test_dedupe_keeps_most_recent_and_drops_skippable() {
        let dir = tempdir().unwrap();
        let mut history = HistoryManager::new().with_dir(dir.path().to_path_buf());
        history.add_entry("scan * from a;");
        history.add_entry("ls");
        history.add_entry("scan * from b;");
        history.add_entry("scan * from a;");
        history.add_entry("clear");
        history.add_entry("history");
        let (kept, removed) = history.dedupe().unwrap();
        assert_eq!(kept, 2);
        assert_eq!(removed, 4);
        assert_eq!(
            history.snapshot(),
            vec!["scan * from b;".to_string(), "scan * from a;".to_string()]
        );
        let contents = fs::read_to_string(history.history_file()).unwrap();
        assert_eq!(contents, "scan * from b;\nscan * from a;\n");
    }

    #[test]
    fn test_remove_exact_drops_matching_entries() {
        let dir = tempdir().unwrap();
        let mut history = HistoryManager::new().with_dir(dir.path().to_path_buf());
        history.add_entry("scan * from a;");
        history.add_entry("scan * from b;");
        history.add_entry("scan * from a;");
        let removed = history.remove_exact("scan * from a;").unwrap();
        assert_eq!(removed, 2);
        assert_eq!(history.snapshot(), vec!["scan * from b;".to_string()]);
        let contents = fs::read_to_string(history.history_file()).unwrap();
        assert_eq!(contents, "scan * from b;\n");
    }
}
