use dql_parser::{
    parse_fragment, parse_script, parse_selection, parse_statement, parse_update_expr, parse_value,
    AttributeType, CompareOp, Condition, FragmentStatus, KeyType, OrderBy, Statement, Value,
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
                ..
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
    fn test_create_global_rejects_invalid_argument_counts() {
        assert_parse_err(r#"CREATE TABLE foobars (foo string hash key) GLOBAL INDEX ("gindex")"#);
        assert_parse_err(
            r#"CREATE TABLE foobars (foo string hash key) GLOBAL INDEX ("gindex", foo, bar, baz)"#,
        );
    }

    #[test]
    fn test_insert() {
        assert_parse_ok("INSERT INTO foobars (foo, bar) VALUES (1, 2)");
        assert_parse_ok("INSERT INTO foobars (foo, bar) VALUES (1, 2), (3, 4)");
        assert_parse_ok(
            r#"INSERT INTO foobars (foo, bar) VALUES (b"binary", ("set", "of", "values"))"#,
        );
        assert_parse_ok("INSERT INTO foobars (id='a', bar=1), (id='b', baz=4)");
        assert_parse_ok("INSERT INTO foobars (id='a', my-field=1), (id='b', my-field=4)");
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
    fn test_alter() {
        assert_parse_ok("ALTER TABLE foobars SET THROUGHPUT (3, 4)");
        assert_parse_ok("ALTER TABLE foobars SET THROUGHPUT (0, *)");
        assert_parse_ok("ALTER TABLE foobars SET INDEX foo_idx THROUGHPUT (3, 4)");
        assert_parse_ok("ALTER TABLE foobars DROP INDEX foo_idx");
        assert_parse_ok("ALTER TABLE foobars CREATE GLOBAL INDEX ('foo_idx', foo)");
        assert_parse_err("ALTER TABLE foobars SET foo = bar");
        assert_parse_err("ALTER TABLE foobars SET THROUGHPUT 1, 1");
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
    fn test_variables_timestamp_forms() {
        assert!(matches!(
            parse_value(r#"timestamp("2012")"#).unwrap(),
            Value::Timestamp(_)
        ));
        assert!(matches!(
            parse_value(r#"utctimestamp "2012""#).unwrap(),
            Value::Timestamp(_)
        ));
        assert!(matches!(
            parse_value(r#"ts("2012")"#).unwrap(),
            Value::Timestamp(_)
        ));
        assert!(matches!(parse_value("now()").unwrap(), Value::Timestamp(_)));
        assert!(matches!(
            parse_value(r#"ms(now() + interval("1 day"))"#).unwrap(),
            Value::Timestamp(_)
        ));
        assert_eq!(
            parse_value(r#"interval("1 day")"#).unwrap(),
            Value::Interval("1 day".to_string())
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
    fn test_constraints_advanced_cases() {
        for expression in [
            "WHERE foo != bar",
            "WHERE NOT foo > 3",
            "WHERE size(foo) < 3",
            r#"WHERE begins_with(foo, "bar")"#,
            "WHERE attribute_exists(foo)",
            "WHERE attribute_not_exists(foo)",
            "WHERE attribute_type(foo, N)",
            r#"WHERE contains(foo, "test")"#,
            "WHERE foo between 1 and 5",
            "WHERE foo in (1, 5, 7)",
            r#"WHERE foo > utcts("2015-12-5")"#,
            r#"WHERE foo > ms(utcts "2015-12-5")"#,
            r#"WHERE foo > utcts "2015-12-5" + interval "1 minute 1s""#,
            r#"WHERE foo < 1 AND (bar >= 0 OR baz < "str" OR qux = 1)"#,
            "WHERE my-field = 1",
            "WHERE a.b-c = 1",
            "WHERE hash in (1, 2, 3)",
        ] {
            parse_statement(&format!("SELECT * FROM foobars {expression}"))
                .unwrap_or_else(|err| panic!("{expression:?} should parse: {err}"));
        }
    }

    #[test]
    fn test_updates() {
        for expression in [
            "set foo = 1",
            "set foo = foo + 1",
            "set foo = 1 + foo",
            "set foo = foo + foo",
            "set foo = 1 + 2",
            "set foo = foo - 2",
            "set foo = foo - 2, bar = 3, baz = qux + 4",
            "SET foo[2] = 4",
            "SET foo.bar = 4",
            "SET foo = if_not_exists(foo, 2)",
            "SET foo = list_append(foo, 2)",
            "SET foo = list_append(2, foo)",
            "SET my-field = 2",
            "REMOVE foo",
            "REMOVE foo, bar",
            "REMOVE foo[0]",
            "REMOVE foo.bar",
            "ADD foo 1",
            r#"ADD foo 1, bar "a""#,
            "DELETE foo 1",
            "DELETE foo 1, bar 2",
        ] {
            parse_update_expr(expression)
                .unwrap_or_else(|err| panic!("{expression:?} should parse: {err}"));
        }
    }

    #[test]
    fn test_selection() {
        for expression in [
            "foo",
            "foo + bar",
            "foo + bar * baz",
            "foo - (bar - baz)",
            "foo + bar AS baz",
            "foo + 2",
            "*",
            "count(*)",
            "timestamp(foo)",
            "utcts(foo - bar)",
            "now() - now()",
        ] {
            parse_selection(expression)
                .unwrap_or_else(|err| panic!("{expression:?} should parse: {err}"));
        }
    }
}

mod test_delete {
    use super::*;

    #[test]
    fn parses_delete_from_table() {
        assert_parse_ok("DELETE FROM foobars");
        assert_parse_ok("DELETE FROM foobars WHERE id = 'a'");
        assert_parse_ok("DELETE FROM foobars KEYS IN 'a', 'b'");
        assert_parse_ok("DELETE FROM foobars KEYS IN ('a', 1), ('b', 2) WHERE foo = 1 USING idx");
        assert_parse_err("DELETE foobars");
    }
}

mod test_query_options {
    use super::*;

    #[test]
    fn parses_select_query_options() {
        let statement =
            parse_statement("SELECT CONSISTENT * FROM foobars WHERE id = 'a' USING ts-index ORDER BY ts DESC LIMIT 5")
                .unwrap();
        match statement {
            Statement::Select { options, .. } => {
                assert!(options.consistent);
                assert_eq!(options.using_index.as_deref(), Some("ts-index"));
                assert_eq!(
                    options.order_by,
                    Some(OrderBy {
                        field: "ts".to_string(),
                        descending: true
                    })
                );
                assert_eq!(options.descending, Some(true));
                assert_eq!(options.limit, Some(5));
            }
            other => panic!("unexpected statement: {other:?}"),
        }
    }

    #[test]
    fn parses_bare_asc_desc() {
        let statement = parse_statement("SELECT * FROM foobars WHERE id = 'a' DESC").unwrap();
        match statement {
            Statement::Select { options, .. } => {
                assert!(options.order_by.is_none());
                assert_eq!(options.descending, Some(true));
            }
            other => panic!("unexpected statement: {other:?}"),
        }
        let statement = parse_statement("SELECT * FROM foobars WHERE id = 'a' ASC").unwrap();
        match statement {
            Statement::Select { options, .. } => {
                assert!(options.order_by.is_none());
                assert_eq!(options.descending, Some(false));
            }
            other => panic!("unexpected statement: {other:?}"),
        }
    }

    #[test]
    fn parses_select_keys_in_and_using_dash() {
        let statement = parse_statement("SELECT * FROM foobars KEYS IN 'a', 'b' USING -").unwrap();
        match statement {
            Statement::Select {
                options, condition, ..
            } => {
                assert!(condition.is_none());
                assert_eq!(options.keys_in.as_ref().map(|keys| keys.len()), Some(2));
                assert_eq!(options.using_index.as_deref(), Some("-"));
            }
            other => panic!("unexpected statement: {other:?}"),
        }
    }

    #[test]
    fn parses_scan_limits_and_order_by() {
        let statement =
            parse_statement("SCAN * FROM foobars LIMIT 3 SCAN LIMIT 4 ORDER BY foo ASC").unwrap();
        match statement {
            Statement::Scan { options, .. } => {
                assert_eq!(options.limit, Some(3));
                assert_eq!(options.scan_limit, Some(4));
                assert_eq!(
                    options.order_by,
                    Some(OrderBy {
                        field: "foo".to_string(),
                        descending: false
                    })
                );
                assert_eq!(options.descending, Some(false));
            }
            other => panic!("unexpected statement: {other:?}"),
        }
    }

    #[test]
    fn parses_update_keys_in_and_using() {
        assert_parse_ok("UPDATE foobars SET foo = 1 KEYS IN 'a' USING idx");
        assert_parse_ok("UPDATE foobars SET foo = 1 WHERE id = 'a' USING idx RETURNS ALL NEW");
    }
}

mod phase_1_statements {
    use super::*;

    #[test]
    fn parses_update_load_explain_analyze() {
        assert_parse_ok("UPDATE foobars SET foo = 1 WHERE id = 'a'");
        assert_parse_ok("LOAD 'items.json' INTO foobars");
        assert_parse_ok("EXPLAIN ALTER TABLE foobars SET THROUGHPUT (1, 1)");
        assert_parse_ok("ANALYZE INSERT INTO foobars (id) VALUES ('a')");
    }

    #[test]
    fn parses_fragments() {
        assert_eq!(
            parse_fragment("CREATE TABLE test "),
            FragmentStatus::Incomplete
        );
        assert!(matches!(
            parse_fragment("CREATE TABLE test (id STRING HASH KEY);"),
            FragmentStatus::Complete(_)
        ));
    }

    #[test]
    fn parses_readme_and_query_doc_examples() {
        for statement in [
            "CREATE TABLE forum_threads (name STRING HASH KEY, subject STRING RANGE KEY, THROUGHPUT (4, 2))",
            "INSERT INTO forum_threads (name, subject, views, replies) VALUES ('Self Defense', 'Defense from Banana', 67, 4)",
            "SCAN * FROM forum_threads",
            "SELECT count(*) FROM forum_threads WHERE name = 'Self Defense'",
            "UPDATE forum_threads ADD views 1 WHERE name = 'Self Defense' AND subject = 'Defense from Banana'",
            "DELETE FROM forum_threads WHERE name = 'Cheese Shop'",
            "ALTER TABLE forum_threads SET THROUGHPUT (8, 4)",
            "DROP TABLE forum_threads",
            "LOAD 'forum_threads.json' INTO forum_threads",
            "DUMP SCHEMA forum_threads",
            "EXPLAIN SELECT * FROM forum_threads WHERE name = 'Self Defense'",
            "ANALYZE INSERT INTO forum_threads (name) VALUES ('x')",
        ] {
            parse_statement(statement)
                .unwrap_or_else(|err| panic!("{statement:?} should parse: {err}"));
        }
    }
}
