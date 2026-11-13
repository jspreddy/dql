//! `dqlrs web --init-examples`: set up an empty folder as a dqlrs workspace holding the
//! examples, before the web UI serves it.
//!
//! The checks run before anything is written: the folder is empty (or missing), git is on
//! PATH, and git knows who is committing. The user confirms the plan, then the folder gets
//! `git init`, the examples bundled into this binary, and one commit.

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use color_eyre::eyre::{bail, eyre, WrapErr};
use include_dir::{include_dir, Dir};

use super::dynamodb_local::{self, Runtime};
use super::progress::Progress;

/// `dqlrs-web/examples`, staged by build.rs without ignored caches.
static EXAMPLES: Dir<'_> = include_dir!("$OUT_DIR/examples");

pub(crate) const COMMIT_MESSAGE: &str = "Initialized dqlrs web examples";

/// What `run` did.
pub(crate) struct Outcome {
    /// False when the user answered no; nothing was changed then.
    pub(crate) initialized: bool,
    /// DynamoDB Local answers on localhost:8000 (already running, or started here).
    pub(crate) dynamodb_local: bool,
}

/// Run the checks, ask how to run DynamoDB Local and whether to go ahead, then initialize
/// `dir` and set up DynamoDB Local. `yes` skips both questions; `dynamodb` (from
/// `--dynamodb`) answers the first. With `yes` and no `dynamodb`, DynamoDB Local is skipped,
/// so nothing is downloaded without a choice.
pub(crate) fn run(dir: &Path, yes: bool, dynamodb: Option<Runtime>) -> color_eyre::Result<Outcome> {
    let progress = Progress::stdout();
    progress.step("Checking the folder", |_| {
        check_empty(dir).map(|()| ((), format!("{} is empty", dir.display())))
    })?;
    progress.step("Checking git", |_| {
        check_git().map(|git| {
            (
                (),
                format!("{}, committing as {}", git.version, git.identity),
            )
        })
    })?;
    let detected = dynamodb_local::detect(&progress);
    if !yes && !io::stdin().is_terminal() {
        bail!(
            "dqlrs web --init-examples asks before it changes {}; run it in a terminal, or pass --yes",
            dir.display()
        );
    }
    let stdin = io::stdin();
    let runtime = dynamodb_local::choose(
        &detected,
        dynamodb,
        !yes,
        &mut stdin.lock(),
        &mut io::stdout(),
    )?;
    if !detected.running && detected.available().is_empty() && dynamodb.is_none() {
        progress.say(
            "    The examples run against DynamoDB Local, which needs Java 17 or newer, or Docker.",
        );
        progress.say("    Install Docker and run `docker compose up -d` in this folder later, or install Java");
        progress.say("    and run `dqlrs web --init-examples` in a new empty folder.");
    }
    progress.say("");
    let plan = plan_text(dir, runtime);
    if !yes {
        if !confirm(&mut stdin.lock(), &mut io::stdout(), &plan)? {
            println!("Cancelled. {} was not changed.", dir.display());
            return Ok(Outcome {
                initialized: false,
                dynamodb_local: false,
            });
        }
    } else {
        print!("{plan}");
    }
    progress.say("");
    initialize(dir, &progress)?;
    let dynamodb_local =
        detected.running || dynamodb_local::set_up(dir, runtime, &detected, &progress);
    progress.say("");
    progress.say(&format!("Ready. Serving {}", dir.display()));
    Ok(Outcome {
        initialized: true,
        dynamodb_local,
    })
}

/// The folder must be empty. A missing folder counts as empty; `initialize` creates it after
/// the confirm, so a cancelled run leaves nothing behind.
pub(crate) fn check_empty(dir: &Path) -> color_eyre::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    if !dir.is_dir() {
        bail!("{} is not a folder", dir.display());
    }
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .wrap_err_with(|| format!("read {}", dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    if names.is_empty() {
        return Ok(());
    }
    names.sort();
    let shown = names.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
    let more = if names.len() > 5 {
        format!(" and {} more", names.len() - 5)
    } else {
        String::new()
    };
    bail!(
        "dqlrs web --init-examples needs an empty folder; {} has {shown}{more}",
        dir.display()
    )
}

/// What `check_git` found, for the progress line.
pub(crate) struct GitInfo {
    /// `git 2.43.0`
    pub(crate) version: String,
    /// `Name <email>` of the author.
    pub(crate) identity: String,
}

/// git must run, and it must know the author and committer (user.name and user.email, or the
/// GIT_* variables), so the commit cannot fail halfway.
pub(crate) fn check_git() -> color_eyre::Result<GitInfo> {
    let version = match Command::new("git").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            text.trim().replacen("git version", "git", 1)
        }
        _ => bail!("dqlrs web --init-examples needs git, and git was not found on PATH"),
    };
    let mut identity = String::new();
    for ident in ["GIT_AUTHOR_IDENT", "GIT_COMMITTER_IDENT"] {
        let output = Command::new("git")
            .args(["var", ident])
            .output()
            .wrap_err("run git var")?;
        if !output.status.success() {
            bail!(
                "git does not know who is committing; set it with\n  git config --global user.name \"Your Name\"\n  git config --global user.email you@example.com"
            );
        }
        if identity.is_empty() {
            // `Name <email> 1700000000 +0000`: keep the name and email.
            let text = String::from_utf8_lossy(&output.stdout);
            identity = match text.find('>') {
                Some(end) => text[..=end].to_string(),
                None => text.trim().to_string(),
            };
        }
    }
    Ok(GitInfo { version, identity })
}

/// What `--init-examples` is about to do, shown before the question.
pub(crate) fn plan_text(dir: &Path, runtime: Runtime) -> String {
    let mut text = format!("dqlrs web --init-examples will set up {}:\n", dir.display());
    text.push_str("  1. Create a git repository (git init).\n");
    text.push_str(&format!(
        "  2. Copy the dqlrs examples ({} files: queries, data model walkthroughs, agent rules).\n",
        example_file_count()
    ));
    text.push_str(&format!("  3. Commit them as \"{COMMIT_MESSAGE}\".\n"));
    if let Some(line) = dynamodb_local::plan_line(runtime) {
        text.push_str(&format!("  4. {line}\n"));
    }
    text.push_str("Then the web UI serves this folder.\n");
    text
}

/// Print the plan and ask. Only `y` or `yes` (any case) continues.
pub(crate) fn confirm(
    input: &mut impl BufRead,
    out: &mut impl Write,
    plan: &str,
) -> color_eyre::Result<bool> {
    write!(out, "{plan}Continue? [y/N] ")?;
    out.flush()?;
    let mut line = String::new();
    input.read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

/// Create the folder, `git init`, copy the examples in, and commit them, one progress step
/// each.
pub(crate) fn initialize(dir: &Path, progress: &Progress) -> color_eyre::Result<()> {
    progress.step("Creating a git repository", |_| {
        std::fs::create_dir_all(dir).wrap_err_with(|| format!("create {}", dir.display()))?;
        git(dir, &["init", "-q"])?;
        Ok(((), "Created a git repository".to_string()))
    })?;
    progress.step("Copying the examples", |detail| {
        let total = example_file_count();
        let written = write_examples(dir, &mut |done| detail.set(format!("{done}/{total}")))?;
        Ok(((), format!("Copied {} example files", written.len())))
    })?;
    progress.step("Committing", |_| {
        git(dir, &["add", "-A"])?;
        git(dir, &["commit", "-q", "-m", COMMIT_MESSAGE])?;
        let head = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["rev-parse", "--short", "HEAD"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .unwrap_or_default();
        let summary = if head.is_empty() {
            format!("Committed \"{COMMIT_MESSAGE}\"")
        } else {
            format!("Committed \"{COMMIT_MESSAGE}\" ({head})")
        };
        Ok(((), summary))
    })
}

/// Write the bundled examples into `dir` and return the paths written. `on_file` gets the
/// count so far after each file. Shell scripts (the agent hooks) are made executable.
pub(crate) fn write_examples(
    dir: &Path,
    on_file: &mut dyn FnMut(usize),
) -> color_eyre::Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    write_dir(&EXAMPLES, dir, &mut written, on_file)?;
    Ok(written)
}

fn write_dir(
    source: &Dir<'_>,
    root: &Path,
    written: &mut Vec<PathBuf>,
    on_file: &mut dyn FnMut(usize),
) -> color_eyre::Result<()> {
    for file in source.files() {
        let target = root.join(file.path());
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .wrap_err_with(|| format!("create {}", parent.display()))?;
        }
        std::fs::write(&target, file.contents())
            .wrap_err_with(|| format!("write {}", target.display()))?;
        if target.extension().is_some_and(|ext| ext == "sh") {
            make_executable(&target)?;
        }
        written.push(target);
        on_file(written.len());
    }
    for child in source.dirs() {
        write_dir(child, root, written, on_file)?;
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> color_eyre::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(permissions.mode() | 0o755);
    std::fs::set_permissions(path, permissions)
        .wrap_err_with(|| format!("chmod {}", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> color_eyre::Result<()> {
    Ok(())
}

/// A bundled example file's text, by path inside the examples folder.
#[cfg(test)]
pub(crate) fn bundled_file(path: &str) -> Option<&'static str> {
    EXAMPLES
        .get_file(path)
        .and_then(|file| file.contents_utf8())
}

pub(crate) fn example_file_count() -> usize {
    fn count(dir: &Dir<'_>) -> usize {
        dir.files().count() + dir.dirs().map(count).sum::<usize>()
    }
    count(&EXAMPLES)
}

fn git(dir: &Path, args: &[&str]) -> color_eyre::Result<()> {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .wrap_err_with(|| format!("run git {}", args.join(" ")))?;
    if !status.success() {
        return Err(eyre!(
            "git {} failed in {} ({status})",
            args.join(" "),
            dir.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dqlrs-init-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_missing_folder_counts_as_empty_and_is_not_created_by_the_check() {
        let dir = temp_dir("missing").join("nested");
        check_empty(&dir).unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn a_folder_with_files_is_refused_and_names_them() {
        let dir = temp_dir("busy");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let err = check_empty(&dir).unwrap_err().to_string();
        assert!(err.contains("needs an empty folder"), "{err}");
        assert!(err.contains(".git, notes.txt"), "{err}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_plan_lists_each_step_and_the_commit_message() {
        let plan = plan_text(Path::new("/work/demo"), Runtime::Skip);
        assert!(plan.contains("/work/demo"));
        assert!(plan.contains("1. Create a git repository"));
        assert!(plan.contains(&format!(
            "2. Copy the dqlrs examples ({} files",
            example_file_count()
        )));
        assert!(plan.contains("3. Commit them as \"Initialized dqlrs web examples\""));
        assert!(!plan.contains("4."));
        let java = plan_text(Path::new("/work/demo"), Runtime::Java);
        assert!(
            java.contains("4. Download DynamoDB Local from AWS into .dynamodb-local/"),
            "{java}"
        );
        let docker = plan_text(Path::new("/work/demo"), Runtime::Docker);
        assert!(
            docker.contains("4. Start DynamoDB Local with docker compose on localhost:8000."),
            "{docker}"
        );
    }

    #[test]
    fn only_yes_confirms() {
        for (answer, expected) in [
            ("y\n", true),
            ("YES\n", true),
            ("\n", false),
            ("n\n", false),
            ("sure\n", false),
        ] {
            let mut out = Vec::new();
            let ok = confirm(&mut answer.as_bytes(), &mut out, "plan\n").unwrap();
            assert_eq!(ok, expected, "answer {answer:?}");
            assert!(String::from_utf8(out)
                .unwrap()
                .ends_with("Continue? [y/N] "));
        }
    }

    #[test]
    fn the_bundle_holds_the_examples_without_caches() {
        assert!(EXAMPLES.get_file("AGENTS.md").is_some());
        assert!(EXAMPLES.get_file(".gitignore").is_some());
        assert!(EXAMPLES.get_file(".cursor/hooks/deny-shell.sh").is_some());
        assert!(EXAMPLES.get_file("queries/README.md").is_some());
        assert!(EXAMPLES.get_dir(".dqlrs-cache").is_none());
        assert!(EXAMPLES.get_dir(".cache").is_none());
    }

    #[test]
    fn initialize_creates_a_repository_with_one_commit_of_the_examples() {
        let dir = temp_dir("full").join("new-workspace");
        check_empty(&dir).unwrap();
        // The test sets its own identity so it does not depend on the machine's git config.
        std::env::set_var("GIT_AUTHOR_NAME", "dqlrs test");
        std::env::set_var("GIT_AUTHOR_EMAIL", "test@example.com");
        std::env::set_var("GIT_COMMITTER_NAME", "dqlrs test");
        std::env::set_var("GIT_COMMITTER_EMAIL", "test@example.com");
        let git = check_git().unwrap();
        assert!(git.version.starts_with("git "), "{}", git.version);
        assert_eq!(git.identity, "dqlrs test <test@example.com>");
        initialize(&dir, &Progress::quiet()).unwrap();

        let log = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["log", "--format=%s"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&log.stdout).trim(), COMMIT_MESSAGE);
        let status = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["status", "--porcelain"])
            .output()
            .unwrap();
        assert!(status.stdout.is_empty(), "everything is committed");
        let tracked = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["ls-files"])
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&tracked.stdout).lines().count(),
            example_file_count()
        );
        assert!(dir.join("queries/README.md").is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join(".cursor/hooks/deny-shell.sh"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111, "hooks are executable");
        }
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }
}
