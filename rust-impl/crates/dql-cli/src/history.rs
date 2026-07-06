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
            self.in_memory.push_back(line?);
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
            writeln!(handle, "{entry}")?;
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
}

fn default_history_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".dql");
    }
    PathBuf::from(".dql")
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
}
