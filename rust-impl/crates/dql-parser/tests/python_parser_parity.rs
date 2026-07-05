use dql_parser::{
    parse_script, parse_statement, parse_value, AttributeType, CompareOp, Condition, KeyType,
    Statement, Value,
};

fn assert_parse_ok(input: &str) {
    parse_statement(input).unwrap_or_else(|err| panic!("{input:?} should parse: {err}"));
}

fn assert_parse_err(input: &str) {
    assert!(
        parse_statement(input).is_err(),
        "{input:?} should fail to parse"
    );
}

fn pending(source: &str, reason: &str) {
    panic!("pending Python parity for {source}: {reason}");
}

mod test_parser {
    use super::*;

    #[test]
    fn test_create() {
        let statement = parse_statement("CREATE TABLE foobars (foo string hash key)").unwrap();
        match statement {
            Statement::CreateTable {
                if_not_exists,
                name,
                attributes,
                throughput,
            } => {
                assert!(!if_not_exists);
                assert_eq!(name, "foobars");
                assert_eq!(throughput, None);
                assert_eq!(attributes.len(), 1);
                assert_eq!(attributes[0].name, "foo");
                assert_eq!(attributes[0].attr_type, AttributeType::String);
                assert_eq!(attributes[0].key_type, Some(KeyType::Hash));
            }
            other => panic!("unexpected statement: {other:?}"),
        }

        assert_parse_ok("CREATE TABLE foobars (foo string hash key, bar NUMBER)");
        assert_parse_ok("CREATE TABLE foobars (foo string hash key, THROUGHPUT (1, 1))");
        assert_parse_ok("CREATE TABLE IF NOT EXISTS foobars (foo string hash key)");
        assert_parse_ok("CREATE TABLE foobars (foo string hash key, bar number range key)");
        assert_parse_err("CREATE TABLE foobars foo binary hash key");
        assert_parse_err("CREATE TABLE foobars (foo hash key)");
        assert_parse_err("CREATE TABLE foobars (foo binary hash key) garbage");
    }

    #[test]
    fn test_create_index() {
        assert_parse_ok(r#"CREATE TABLE foobars (foo binary index("foo-index"))"#);
        assert_parse_ok(r#"CREATE TABLE foobars (foo binary index("idxname"))"#);
        assert_parse_ok(r#"CREATE TABLE foobars (foo binary keys index("idxname"))"#);
        assert_parse_ok(r#"CREATE TABLE foobars (foo binary include index("idxname", ["foo"]))"#);
        assert_parse_ok(
            r#"CREATE TABLE foobars (foo binary INCLUDE INDEX("idxname", ["foo", "bar"]))"#,
        );
        assert_parse_err("CREATE foobars (foo binary index(idxname))");
    }

    #[test]
    fn test_create_global() {
        assert_parse_ok(
            r#"CREATE TABLE foobars (foo string hash key) GLOBAL INDEX ("gindex", foo)"#,
        );
        assert_parse_ok(
            r#"CREATE TABLE foobars (foo string hash key) GLOBAL INDEX ("gindex", foo, bar)"#,
        );
        assert_parse_ok(
            r#"CREATE TABLE foobars (foo string hash key) GLOBAL INDEX ("gindex", foo) GLOBAL INDEX ("g2idx", bar, foo)"#,
        );
        assert_parse_ok(
            r#"CREATE TABLE foobars (foo string hash key) GLOBAL INDEX ("gindex", foo, bar, THROUGHPUT (2, 4))"#,
        );
        assert_parse_ok(
            r#"CREATE TABLE foobars (foo string hash key) GLOBAL INDEX ("gindex", foo, THROUGHPUT (2, 4))"#,
        );
        assert_parse_ok(
            r#"CREATE TABLE foobars (foo string hash key) GLOBAL KEYS INDEX ("gindex", foo)"#,
        );
        assert_parse_ok(
            r#"CREATE TABLE foobars (foo string hash key) GLOBAL INCLUDE INDEX ("g2idx", bar, foo, ["baz"])"#,
        );
        assert_parse_err(
            r#"CREATE TABLE foobars (foo string hash key) GLOBAL INDEX ("gindex", foo, bar),"#,
        );
    }

    #[test]
    #[ignore = "needs full GLOBAL INDEX argument validation"]
    fn test_create_global_rejects_invalid_argument_counts() {
        pending(
            "tests/test_parser.py::TestParser::test_create_global",
            "current parser skips GLOBAL INDEX internals",
        );
    }

    #[test]
    fn test_insert() {
        assert_parse_ok("INSERT INTO foobars (foo, bar) VALUES (1, 2)");
        assert_parse_ok("INSERT INTO foobars (foo, bar) VALUES (1, 2), (3, 4)");
        assert_parse_ok(
            r#"INSERT INTO foobars (foo, bar) VALUES (b"binary", ("set", "of", "values"))"#,
        );
        assert_parse_err("INSERT foobars (foo, bar) VALUES (1, 2)");
        assert_parse_err("INSERT INTO foobars foo, bar VALUES (1, 2)");
        assert_parse_err("INSERT INTO foobars (foo, bar) VALUES 1, 2");
        assert_parse_err("INSERT INTO foobars (foo, bar) VALUES (1, 2) garbage");
    }

    #[test]
    fn test_drop() {
        assert_parse_ok("DROP TABLE foobars");
        assert_parse_ok("DROP TABLE IF EXISTS foobars");
        assert_parse_err("DROP foobars");
        assert_parse_err("DROP TABLE foobars garbage");
    }

    #[test]
    #[ignore = "needs ALTER parser"]
    fn test_alter() {
        pending(
            "tests/test_parser.py::TestParser::test_alter",
            "ALTER statement is deferred",
        );
    }

    #[test]
    fn test_dump() {
        assert_eq!(
            parse_statement("DUMP SCHEMA").unwrap(),
            Statement::DumpSchema { tables: None }
        );
        assert_eq!(
            parse_statement("DUMP SCHEMA foobars, wibbles").unwrap(),
            Statement::DumpSchema {
                tables: Some(vec!["foobars".to_string(), "wibbles".to_string()])
            }
        );
        assert_parse_err("DUMP SCHEMA foobars wibbles");
    }

    #[test]
    fn test_multiple_statements() {
        assert_eq!(parse_script("DUMP SCHEMA;DUMP SCHEMA").unwrap().len(), 2);
        assert_eq!(parse_script("DUMP SCHEMA;\nDUMP SCHEMA").unwrap().len(), 2);
        assert_eq!(
            parse_script("DUMP SCHEMA\n;\nDUMP SCHEMA").unwrap().len(),
            2
        );
    }

    #[test]
    fn test_variables() {
        assert_eq!(
            parse_value("'foo'").unwrap(),
            Value::String("foo".to_string())
        );
        assert_eq!(parse_value("1").unwrap(), Value::Number("1".to_string()));
        assert_eq!(
            parse_value("1.25").unwrap(),
            Value::Number("1.25".to_string())
        );
        assert_eq!(
            parse_value("b'abc'").unwrap(),
            Value::Binary(b"abc".to_vec())
        );
        assert_eq!(parse_value("null").unwrap(), Value::Null);
        assert_eq!(parse_value("TRUE").unwrap(), Value::Bool(true));
        assert_eq!(parse_value("FALSE").unwrap(), Value::Bool(false));
        assert_eq!(parse_value("()").unwrap(), Value::Set(vec![]));
        assert_eq!(
            parse_value("(1, 2)").unwrap(),
            Value::Set(vec![
                Value::Number("1".to_string()),
                Value::Number("2".to_string())
            ])
        );
        assert_eq!(parse_value("[]").unwrap(), Value::List(vec![]));
        assert_eq!(
            parse_value("[1, ['a', 2]]").unwrap(),
            Value::List(vec![
                Value::Number("1".to_string()),
                Value::List(vec![
                    Value::String("a".to_string()),
                    Value::Number("2".to_string())
                ])
            ])
        );
        assert!(matches!(parse_value("{}").unwrap(), Value::Map(map) if map.is_empty()));
        assert!(matches!(
            parse_value("{'a': {'b': true}}").unwrap(),
            Value::Map(_)
        ));
    }

    #[test]
    #[ignore = "needs timestamp and interval value parser"]
    fn test_variables_timestamp_forms() {
        pending(
            "tests/test_parser.py::TestParser::test_variables",
            "timestamp, now, interval, and ms expressions are deferred",
        );
    }
}

mod test_expressions {
    use super::*;

    #[test]
    fn test_constraints() {
        let statement =
            parse_statement("SELECT * FROM foobars WHERE foo != 1 AND bar >= 0").unwrap();
        match statement {
            Statement::Select {
                condition: Some(Condition::And(parts)),
                ..
            } => {
                assert_eq!(parts.len(), 2);
                assert!(matches!(
                    parts[0],
                    Condition::Compare {
                        op: CompareOp::Ne,
                        ..
                    }
                ));
                assert!(matches!(
                    parts[1],
                    Condition::Compare {
                        op: CompareOp::Ge,
                        ..
                    }
                ));
            }
            other => panic!("unexpected statement: {other:?}"),
        }
    }

    #[test]
    fn test_constraints_boolean_precedence() {
        let statement =
            parse_statement("SELECT * FROM foobars WHERE foo = 1 AND NOT (bar = 2 OR baz = 3)")
                .unwrap();
        match statement {
            Statement::Select {
                condition: Some(Condition::And(parts)),
                ..
            } => {
                assert_eq!(parts.len(), 2);
                assert!(matches!(parts[1], Condition::Not(_)));
            }
            other => panic!("unexpected statement: {other:?}"),
        }
    }

    #[test]
    #[ignore = "needs full WHERE expression parser"]
    fn test_constraints_advanced_cases() {
        pending(
            "tests/test_parser.py::TestExpressions::test_constraints",
            "OR, NOT, functions, field comparisons, BETWEEN, IN, grouping, and timestamp expressions are deferred",
        );
    }

    #[test]
    #[ignore = "needs UPDATE expression parser"]
    fn test_updates() {
        pending(
            "tests/test_parser.py::TestExpressions::test_updates",
            "SET, ADD, DELETE, REMOVE, if_not_exists, list_append, and path update parsing are deferred",
        );
    }

    #[test]
    #[ignore = "needs SELECT projection expression parser"]
    fn test_selection() {
        pending(
            "tests/test_parser.py::TestExpressions::test_selection",
            "projection arithmetic, aliases, count(*), and timestamp functions are deferred",
        );
    }
}

mod test_delete {
    use super::*;

    #[test]
    fn parses_delete_from_table() {
        assert_parse_ok("DELETE FROM foobars");
        assert_parse_ok("DELETE FROM foobars WHERE id = 'a'");
        assert_parse_err("DELETE foobars");
    }
}
