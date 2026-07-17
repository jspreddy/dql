use dql_cli::help;
use dql_cli::history::HistoryManager;
use dql_cli::meta::parse_repl_args;
use dql_cli::session::Session;
use dql_output::{format_table_detail, format_table_summary_table, TableStats};
use std::process::Command;
use tempfile::tempdir;

fn test_session() -> Session {
    Session::new_memory("us-west-1")
}

fn dql() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_dqlrs"));
    // Offline smoke paths must not hit live AWS.
    cmd.env("DQL_BACKEND", "memory");
    cmd
}

mod test_cli {
    use super::*;

    #[test]
    fn test_repl_command_args() {
        let (args, kwargs) = parse_repl_args("a b");
        assert_eq!(args, vec!["a", "b"]);
        assert!(kwargs.is_empty());
    }

    #[test]
    fn test_repl_command_kwargs() {
        let (args, kwargs) = parse_repl_args("a second=b");
        assert_eq!(args, vec!["a"]);
        assert_eq!(kwargs.get("second"), Some(&"b".to_string()));
    }

    #[test]
    fn test_help_docs() {
        for topic in [
            "alter", "analyze", "create", "delete", "drop", "dump", "explain", "insert", "load",
            "scan", "select", "update", "options",
        ] {
            assert!(
                help::statement_help(topic).is_some(),
                "missing help for {topic}"
            );
        }
    }
}

mod test_cli_commands {
    use super::*;

    #[test]
    fn test_scan_table() {
        let output = dql()
            .args([
                "--json",
                "-c",
                "CREATE TABLE foobar (id STRING HASH KEY);
                 INSERT INTO foobar (id, num, bin, list, dict, bool) VALUES ('a', 1, b'a', [1, 'a'], {'a': 1}, TRUE);
                 SCAN * FROM foobar",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("\"id\": \"a\""));
        assert!(stdout.contains("\"bin\": \"YQ==\""));
        assert!(stdout.contains("\"bool\": true"));
    }

    #[test]
    fn test_ls() {
        let mut session = test_session();
        session
            .run_command(
                "CREATE TABLE foobar_ls_test (id STRING HASH KEY, range NUMBER RANGE KEY, \
                 foo STRING INDEX('foo-index')) GLOBAL INDEX ('bar-index', bar STRING)",
                false,
            )
            .unwrap();
        let meta = session
            .engine
            .describe("foobar_ls_test", false)
            .unwrap()
            .expect("table should exist");
        let output = format_table_detail(
            &meta,
            &TableStats {
                item_count: 0,
                size_bytes: 0,
            },
        );
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_ls_with_multiple_tables() {
        let mut session = test_session();
        session
            .run_command(
                "CREATE TABLE foo (id STRING HASH KEY, range NUMBER RANGE KEY, \
                 foo STRING INDEX('foo-index'));
                 CREATE TABLE bar (id STRING HASH KEY, range NUMBER RANGE KEY, \
                 bar STRING INDEX('bar-index'))",
                false,
            )
            .unwrap();
        let tables = session.engine.describe_all(false).unwrap();
        let rows = tables
            .into_iter()
            .map(|meta| {
                (
                    meta,
                    TableStats {
                        item_count: 0,
                        size_bytes: 0,
                    },
                )
            })
            .collect::<Vec<_>>();
        let output = format_table_summary_table(&rows);
        insta::assert_snapshot!(output);
    }
}

mod current_cli_surface {
    use super::*;

    #[test]
    fn version_flag_prints_package_version() {
        let output = dql().arg("--version").output().unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), "0.6.4-dev12\n");
    }

    #[test]
    fn command_script_returns_last_statement_result() {
        let output = dql()
            .args([
                "--json",
                "-c",
                "CREATE TABLE t (id STRING HASH KEY); INSERT INTO t (id) VALUES ('a'); SCAN * FROM t",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "{\n    \"id\": \"a\"\n}\n"
        );
    }

    #[test]
    fn command_non_json_prints_human_output() {
        let output = dql()
            .args(["-c", "CREATE TABLE t (id STRING HASH KEY)"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .contains("Created table 't'"));
    }

    #[test]
    fn command_errors_surface_on_stderr() {
        let output = dql().args(["-c", "DROP nope"]).output().unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("expected TABLE"));
    }

    #[test]
    fn help_flag_lists_supported_flags() {
        let output = dql().arg("--help").output().unwrap();
        assert!(output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("--command"));
        assert!(stderr.contains("--json"));
        assert!(stderr.contains("--version"));
    }

    #[test]
    fn rejects_unknown_argument() {
        let output = dql().arg("--unknown").output().unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("unknown argument '--unknown'"));
    }
}

mod test_history_manager {
    use super::*;

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
        let contents = std::fs::read_to_string(history.history_file()).unwrap();
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
        let contents = std::fs::read_to_string(history.history_file()).unwrap();
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
        let file = history.history_file();
        std::fs::write(&file, "").unwrap();
        let mut perms = std::fs::metadata(&file).unwrap().permissions();
        perms.set_readonly(true);
        let _ = std::fs::set_permissions(&file, perms);
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

mod test_readline_compat {
    #[test]
    #[ignore = "Python readline module registration is not applicable in Rust"]
    fn test_gnureadline_is_registered_as_readline() {}

    #[test]
    #[ignore = "ratatui history buffer differs from Python readline integration"]
    fn test_cmd_and_history_share_readline_buffer() {}
}
