//! Where SAVE, LOAD, and local-table file paths resolve.
//!
//! An absolute path is used as written. A path that starts with `./` is relative to the
//! folder of the `.dql` file being run. Any other relative path is relative to the
//! workspace, the folder dqlrs was started with. With no workspace set (the REPL,
//! `--command`, `--serve`), a relative path stays relative to the process directory.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileBase {
    workspace: Option<PathBuf>,
    script_dir: Option<PathBuf>,
}

impl FileBase {
    /// `script_dir` is the folder of the `.dql` file being run. Without one, `./` paths use
    /// the workspace.
    pub fn new(workspace: Option<PathBuf>, script_dir: Option<PathBuf>) -> Self {
        Self {
            workspace,
            script_dir,
        }
    }

    pub fn resolve(&self, raw: &str) -> PathBuf {
        let raw = raw.trim_matches('"').trim_matches('\'');
        let path = Path::new(raw);
        if path.is_absolute() {
            return path.to_path_buf();
        }
        let script_relative = raw.strip_prefix("./").or_else(|| raw.strip_prefix(".\\"));
        if let Some(rest) = script_relative {
            if let Some(dir) = self.script_dir.as_ref().or(self.workspace.as_ref()) {
                return dir.join(rest);
            }
        }
        match &self.workspace {
            Some(workspace) => workspace.join(path),
            None => path.to_path_buf(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> FileBase {
        FileBase::new(
            Some(PathBuf::from("/work")),
            Some(PathBuf::from("/work/queries/daily")),
        )
    }

    #[test]
    fn dot_slash_is_relative_to_the_dql_file() {
        assert_eq!(
            base().resolve("./out.json"),
            PathBuf::from("/work/queries/daily/out.json")
        );
        assert_eq!(
            base().resolve("'./data/in.csv'"),
            PathBuf::from("/work/queries/daily/data/in.csv")
        );
    }

    #[test]
    fn other_relative_paths_are_relative_to_the_workspace() {
        assert_eq!(base().resolve("out.json"), PathBuf::from("/work/out.json"));
        assert_eq!(
            base().resolve("exports/out.json"),
            PathBuf::from("/work/exports/out.json")
        );
        assert_eq!(
            base().resolve("../shared/out.json"),
            PathBuf::from("/work/../shared/out.json")
        );
    }

    #[test]
    fn absolute_paths_are_kept() {
        assert_eq!(
            base().resolve("/tmp/out.json"),
            PathBuf::from("/tmp/out.json")
        );
    }

    #[test]
    fn dot_slash_without_a_dql_file_uses_the_workspace() {
        let base = FileBase::new(Some(PathBuf::from("/work")), None);
        assert_eq!(base.resolve("./out.json"), PathBuf::from("/work/out.json"));
    }

    #[test]
    fn no_workspace_keeps_the_process_directory() {
        let base = FileBase::default();
        assert_eq!(base.resolve("./out.json"), PathBuf::from("./out.json"));
        assert_eq!(base.resolve("out.json"), PathBuf::from("out.json"));
    }
}
