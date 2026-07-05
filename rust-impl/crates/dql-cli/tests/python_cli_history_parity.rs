use std::process::Command;

fn dql() -> Command {
    Command::new(env!("CARGO_BIN_EXE_dql"))
}

fn pending(source: &str, reason: &str) {
    panic!("pending Python parity for {source}: {reason}");
}

mod test_cli {
    use super::*;

    #[test]
    #[ignore = "needs shlex-style REPL meta-command decorator equivalent"]
    fn test_repl_command_args() {
        pending(
            "tests/test_cli.py::TestCli::test_repl_command_args",
            "Rust CLI has not implemented meta-command arg parsing yet",
        );
    }

    #[test]
    #[ignore = "needs shlex-style REPL meta-command decorator equivalent"]
    fn test_repl_command_kwargs() {
        pending(
            "tests/test_cli.py::TestCli::test_repl_command_kwargs",
            "Rust CLI has not implemented meta-command kwarg parsing yet",
        );
    }

    #[test]
    #[ignore = "needs full help text parity"]
    fn test_help_docs() {
        pending(
            "tests/test_cli.py::TestCli::test_help_docs",
            "statement-specific help docs are deferred",
        );
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
    #[ignore = "needs ls meta-command and Rich table output"]
    fn test_ls() {
        pending(
            "tests/test_cli.py::TestCliCommands::test_ls",
            "ls metadata output is deferred",
        );
    }

    #[test]
    #[ignore = "needs ls meta-command and Rich table output"]
    fn test_ls_with_multiple_tables() {
        pending(
            "tests/test_cli.py::TestCliCommands::test_ls_with_multiple_tables",
            "ls metadata output is deferred",
        );
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

    macro_rules! ignored_history {
        ($($name:ident),+ $(,)?) => {
            $(
                #[test]
                #[ignore = "needs Rust history manager"]
                fn $name() {
                    pending(concat!("tests/test_history.py::TestHistoryManager::", stringify!($name)), "persistent readline history is deferred");
                }
            )+
        };
    }

    ignored_history!(
        test_history_file_is_created_on_load,
        test_history_file_is_created_on_write,
        test_history_file_contains_history_from_readline,
        test_history_file_contains_proper_appended_history,
        test_write_history_handles_append_failure,
        test_write_history_handles_get_length_failure,
        test_remove_items_handles_readline_failure,
    );
}

mod test_readline_compat {
    use super::*;

    #[test]
    #[ignore = "Python readline module registration is not applicable until Rust line editor is selected"]
    fn test_gnureadline_is_registered_as_readline() {
        pending(
            "tests/test_readline_compat.py::TestReadlineCompat::test_gnureadline_is_registered_as_readline",
            "Python sys.modules compatibility has no Rust equivalent yet",
        );
    }

    #[test]
    #[ignore = "needs Rust line editor and history integration"]
    fn test_cmd_and_history_share_readline_buffer() {
        pending(
            "tests/test_readline_compat.py::TestReadlineCompat::test_cmd_and_history_share_readline_buffer",
            "line editor/history shared state is deferred",
        );
    }
}
