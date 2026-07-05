use dql_engine::{format_throughput, InMemoryEngine, Item, StatementResult};
use dql_parser::Value;

fn pending(source: &str, reason: &str) {
    panic!("pending Python parity for {source}: {reason}");
}

fn scan_after_insert(value: &str) -> Item {
    let mut engine = InMemoryEngine::default();
    let result = engine
        .execute(&format!(
            "CREATE TABLE foobar (id STRING HASH KEY);
             INSERT INTO foobar (id, bar) VALUES ('a', {value});
             SCAN * FROM foobar"
        ))
        .unwrap();
    match result {
        StatementResult::Items(mut items) => items.remove(0),
        other => panic!("unexpected result: {other:?}"),
    }
}

fn assert_items_len(result: StatementResult, len: usize) {
    match result {
        StatementResult::Items(items) => assert_eq!(items.len(), len),
        other => panic!("unexpected result: {other:?}"),
    }
}

mod test_data_types {
    use super::*;

    #[test]
    fn test_str() {
        let item = scan_after_insert("'a'");
        assert_eq!(item.get("bar"), Some(&Value::String("a".to_string())));
    }

    #[test]
    fn test_int() {
        let item = scan_after_insert("5");
        assert_eq!(item.get("bar"), Some(&Value::Number("5".to_string())));
    }

    #[test]
    fn test_float() {
        let item = scan_after_insert("1.2345");
        assert_eq!(item.get("bar"), Some(&Value::Number("1.2345".to_string())));
    }

    #[test]
    fn test_bool() {
        let item = scan_after_insert("false");
        assert_eq!(item.get("bar"), Some(&Value::Bool(false)));
    }

    #[test]
    fn test_binary() {
        let item = scan_after_insert("b'abc'");
        assert_eq!(item.get("bar"), Some(&Value::Binary(b"abc".to_vec())));
    }

    #[test]
    fn test_list() {
        let item = scan_after_insert("[1, null, 'a']");
        assert_eq!(
            item.get("bar"),
            Some(&Value::List(vec![
                Value::Number("1".to_string()),
                Value::Null,
                Value::String("a".to_string())
            ]))
        );
    }

    #[test]
    fn test_empty_list() {
        let item = scan_after_insert("[]");
        assert_eq!(item.get("bar"), Some(&Value::List(vec![])));
    }

    #[test]
    fn test_nested_list() {
        let item = scan_after_insert("[1, [2, 3]]");
        assert!(matches!(item.get("bar"), Some(Value::List(_))));
    }

    #[test]
    fn test_dict() {
        let item = scan_after_insert("{'a': 2}");
        assert!(matches!(item.get("bar"), Some(Value::Map(map)) if map.contains_key("a")));
    }

    #[test]
    fn test_empty_dict() {
        let item = scan_after_insert("{}");
        assert!(matches!(item.get("bar"), Some(Value::Map(map)) if map.is_empty()));
    }

    #[test]
    fn test_nested_dict() {
        let item = scan_after_insert("{'a': {'b': null}}");
        assert!(matches!(item.get("bar"), Some(Value::Map(map)) if map.contains_key("a")));
    }
}

mod test_fragment_engine {
    use super::*;

    #[test]
    #[ignore = "needs FragmentEngine equivalent"]
    fn test_no_run_fragment() {
        pending(
            "tests/test_engine.py::TestFragmentEngine::test_no_run_fragment",
            "fragment buffering is deferred",
        );
    }

    #[test]
    #[ignore = "needs FragmentEngine equivalent"]
    fn test_no_run_multi_fragment() {
        pending(
            "tests/test_engine.py::TestFragmentEngine::test_no_run_multi_fragment",
            "fragment buffering is deferred",
        );
    }

    #[test]
    #[ignore = "needs FragmentEngine equivalent"]
    fn test_run_query() {
        pending(
            "tests/test_engine.py::TestFragmentEngine::test_run_query",
            "fragment buffering is deferred",
        );
    }

    #[test]
    #[ignore = "needs parse-error pretty formatter"]
    fn test_format_exc() {
        pending(
            "tests/test_engine.py::TestFragmentEngine::test_format_exc",
            "Python pformat_exc caret output is deferred",
        );
    }

    #[test]
    #[ignore = "needs FragmentEngine equivalent"]
    fn test_preserve_whitespace() {
        pending(
            "tests/test_engine.py::TestFragmentEngine::test_preserve_whitespace",
            "fragment whitespace preservation is deferred",
        );
    }
}

mod test_queries {
    use super::*;

    #[test]
    fn test_drop() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY); DROP TABLE foobar")
            .unwrap();
        assert!(engine.execute("SCAN * FROM foobar").is_err());
    }

    #[test]
    fn test_drop_if_exists() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY);
                 DROP TABLE foobar;
                 DROP TABLE IF EXISTS foobar",
            )
            .unwrap();
    }

    #[test]
    fn test_explain_drop() {
        let mut engine = InMemoryEngine::default();
        let result = engine.execute("EXPLAIN DROP TABLE foobar").unwrap();
        assert_eq!(
            result,
            StatementResult::Schema("delete_table foobar".to_string())
        );
    }

    #[test]
    fn test_dump() {
        let mut engine = InMemoryEngine::default();
        let schema = engine
            .execute("CREATE TABLE test (id STRING HASH KEY, bar NUMBER RANGE KEY); DUMP SCHEMA")
            .unwrap();
        assert_eq!(
            schema,
            StatementResult::Schema(
                "CREATE TABLE test (id STRING HASH KEY, bar NUMBER RANGE KEY)".to_string()
            )
        );
    }

    #[test]
    fn test_dump_tables() {
        let mut engine = InMemoryEngine::default();
        let result = engine
            .execute(
                "CREATE TABLE test (id STRING HASH KEY);
                 CREATE TABLE test2 (id STRING HASH KEY);
                 DUMP SCHEMA test2",
            )
            .unwrap();
        assert_eq!(
            result,
            StatementResult::Schema("CREATE TABLE test2 (id STRING HASH KEY)".to_string())
        );
    }

    #[test]
    fn test_multiple_statements() {
        let mut engine = InMemoryEngine::default();
        let result = engine
            .execute(
                "CREATE TABLE test (id STRING HASH KEY);
                 INSERT INTO test (id, foo) VALUES ('a', 1), ('b', 2);
                 SCAN * FROM test",
            )
            .unwrap();
        assert_items_len(result, 2);
    }
}

mod test_alter {
    use super::*;
    use dql_engine::DynamoBackend;
    use dql_models::{BillingMode, TableMeta};

    fn describe_table(engine: &InMemoryEngine, name: &str) -> TableMeta {
        engine
            .backend()
            .describe_table(name)
            .unwrap()
            .expect("table should exist")
    }

    #[test]
    fn test_alter_throughput() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY, THROUGHPUT (1, 1))")
            .unwrap();
        engine
            .execute("ALTER TABLE foobar SET THROUGHPUT (2, 2)")
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        assert_eq!(
            desc.throughput.as_ref().map(|tp| tp.read.clone()),
            Some(Value::Number("2".to_string()))
        );
        assert_eq!(
            desc.throughput.as_ref().map(|tp| tp.write.clone()),
            Some(Value::Number("2".to_string()))
        );
    }

    #[test]
    fn test_alter_throughput_partial_star() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY, THROUGHPUT (1, 1))")
            .unwrap();
        engine
            .execute("ALTER TABLE foobar SET THROUGHPUT (2, *)")
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        assert_eq!(
            desc.throughput.as_ref().map(|tp| tp.read.clone()),
            Some(Value::Number("2".to_string()))
        );
        assert_eq!(
            desc.throughput.as_ref().map(|tp| tp.write.clone()),
            Some(Value::Number("1".to_string()))
        );
    }

    #[test]
    fn test_alter_billing_mode() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY, THROUGHPUT (1, 1))")
            .unwrap();
        engine
            .execute("ALTER TABLE foobar SET THROUGHPUT (0, 0)")
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        assert_eq!(desc.billing_mode, BillingMode::OnDemand);
    }

    #[test]
    fn test_alter_billing_mode_provisioned() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY)")
            .unwrap();
        engine
            .execute("ALTER TABLE foobar SET THROUGHPUT (2, 3)")
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        assert_eq!(desc.billing_mode, BillingMode::Provisioned);
        assert_eq!(
            desc.throughput.as_ref().map(|tp| tp.read.clone()),
            Some(Value::Number("2".to_string()))
        );
        assert_eq!(
            desc.throughput.as_ref().map(|tp| tp.write.clone()),
            Some(Value::Number("3".to_string()))
        );
    }

    #[test]
    #[ignore = "DynamoDB Local GSI throughput bug"]
    fn test_alter_index_throughput() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, foo NUMBER) \
                 GLOBAL INDEX ('foo_index', foo, THROUGHPUT(1, 1))",
            )
            .unwrap();
        engine
            .execute("ALTER TABLE foobar SET INDEX foo_index THROUGHPUT (2, 2)")
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        let index = desc.global_indexes.get("foo_index").unwrap();
        assert_eq!(
            index.throughput.as_ref().map(|tp| tp.read.clone()),
            Some(Value::Number("2".to_string()))
        );
    }

    #[test]
    fn test_alter_drop() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, foo NUMBER) \
                 GLOBAL INDEX ('foo_index', foo, THROUGHPUT(1, 1))",
            )
            .unwrap();
        engine
            .execute("ALTER TABLE foobar DROP INDEX foo_index")
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        assert!(desc.global_indexes.is_empty());
    }

    #[test]
    fn test_alter_create() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY, foo NUMBER, THROUGHPUT (1, 1))")
            .unwrap();
        engine
            .execute("ALTER TABLE foobar CREATE GLOBAL INDEX ('foo_index', baz STRING, TP (2, 3))")
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        let index = desc.global_indexes.get("foo_index").unwrap();
        assert_eq!(index.hash_key.name, "baz");
        assert!(index.range_key.is_none());
        assert_eq!(
            index.throughput.as_ref().map(|tp| tp.read.clone()),
            Some(Value::Number("2".to_string()))
        );
    }

    #[test]
    fn test_explain_throughput() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY)")
            .unwrap();
        let result = engine
            .execute("EXPLAIN ALTER TABLE foobar SET THROUGHPUT (2, 2)")
            .unwrap();
        assert_eq!(
            result,
            StatementResult::Schema("update_table foobar".to_string())
        );
    }

    #[test]
    fn test_explain_create_index() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY)")
            .unwrap();
        let result = engine
            .execute("EXPLAIN ALTER TABLE foobar CREATE GLOBAL INDEX('foo_index', baz STRING)")
            .unwrap();
        assert_eq!(
            result,
            StatementResult::Schema("update_table foobar".to_string())
        );
    }

    #[test]
    fn test_alter_create_if_not_exists() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, foo NUMBER) \
                 GLOBAL INDEX ('foo_index', foo, THROUGHPUT(1, 1))",
            )
            .unwrap();
        engine
            .execute(
                "ALTER TABLE foobar CREATE GLOBAL INDEX ('foo_index', baz STRING) IF NOT EXISTS",
            )
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        let index = desc.global_indexes.get("foo_index").unwrap();
        assert_eq!(index.hash_key.name, "foo");
    }

    #[test]
    fn test_alter_drop_if_exists() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY)")
            .unwrap();
        engine
            .execute("ALTER TABLE foobar DROP INDEX foo_index IF EXISTS")
            .unwrap();
    }
}

mod test_insert {
    use super::*;

    #[test]
    fn test_insert() {
        let mut engine = InMemoryEngine::default();
        let result = engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY);
                 INSERT INTO foobar (id, foo) VALUES ('a', 1), ('b', 2);
                 SCAN * FROM foobar",
            )
            .unwrap();
        assert_items_len(result, 2);
    }

    #[test]
    fn test_insert_binary() {
        let item = scan_after_insert("b'abc'");
        assert_eq!(item.get("bar"), Some(&Value::Binary(b"abc".to_vec())));
    }

    #[test]
    fn test_insert_keywords() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY)")
            .unwrap();
        engine
            .execute("INSERT INTO foobar (id='a', bar=1), (id='b', baz=4)")
            .unwrap();
        let result = engine.execute("SCAN * FROM foobar").unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 2);
                let a = items
                    .iter()
                    .find(|item| item.get("id") == Some(&Value::String("a".to_string())))
                    .unwrap();
                let b = items
                    .iter()
                    .find(|item| item.get("id") == Some(&Value::String("b".to_string())))
                    .unwrap();
                assert_eq!(a.get("bar"), Some(&Value::Number("1".to_string())));
                assert_eq!(b.get("baz"), Some(&Value::Number("4".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_insert_timestamps() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY)")
            .unwrap();
        engine
            .execute("INSERT INTO foobar (id='a', bar=NOW() + INTERVAL '1 hour')")
            .unwrap();
        let result = engine.execute("SCAN * FROM foobar").unwrap();
        match result {
            StatementResult::Items(items) => {
                let bar = items[0]
                    .get("bar")
                    .and_then(|value| match value {
                        Value::Number(number) => number.parse::<f64>().ok(),
                        _ => None,
                    })
                    .expect("bar should be numeric timestamp");
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs_f64();
                assert!((bar - (now + 3600.0)).abs() <= 2.0);
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_explain() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY)")
            .unwrap();
        let result = engine
            .execute("EXPLAIN INSERT INTO foobar (id) VALUES ('a')")
            .unwrap();
        assert_eq!(
            result,
            StatementResult::Schema("batch_write_item foobar".to_string())
        );
    }
}

mod test_select {
    use super::*;

    #[test]
    fn test_hash_key() {
        let mut engine = seeded_table();
        let result = engine
            .execute("SELECT * FROM foobar WHERE id = 'a'")
            .unwrap();
        assert_items_len(result, 1);
    }

    #[test]
    fn test_consistent() {
        let mut engine = seeded_table();
        let result = engine
            .execute("SELECT CONSISTENT * FROM foobar WHERE id = 'a'")
            .unwrap();
        assert_items_len(result, 1);
    }

    #[test]
    fn test_hash_range() {
        let mut engine = seeded_table();
        let result = engine
            .execute("SELECT * FROM foobar WHERE id = 'a' AND range = 1")
            .unwrap();
        assert_items_len(result, 1);
    }

    macro_rules! ignored_select {
        ($($name:ident => $reason:expr),+ $(,)?) => {
            $(
                #[test]
                #[ignore = $reason]
                fn $name() {
                    pending(concat!("tests/test_queries.py::TestSelect::", stringify!($name)), $reason);
                }
            )+
        };
    }

    ignored_select!(
        test_reverse => "needs ORDER BY DESC support",
    );

    fn make_indexed_table(engine: &mut InMemoryEngine) {
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY, ts NUMBER INDEX('ts-index'));
                 INSERT INTO foobar (id, bar, ts) VALUES ('a', 1, 100), ('a', 2, 200)",
            )
            .unwrap();
    }

    #[test]
    fn test_hash_index() {
        let mut engine = InMemoryEngine::default();
        make_indexed_table(&mut engine);
        let result = engine
            .execute("SELECT * FROM foobar WHERE id = 'a' AND ts < 150 USING ts-index")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].get("ts"), Some(&Value::Number("100".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_smart_index() {
        let mut engine = InMemoryEngine::default();
        make_indexed_table(&mut engine);
        let result = engine
            .execute("SELECT * FROM foobar WHERE id = 'a' AND ts < 150")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].get("ts"), Some(&Value::Number("100".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_smart_global_index() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, foo STRING RANGE KEY, bar NUMBER INDEX('bar-index'), baz STRING) \
                 GLOBAL INDEX ('gindex', baz);
                 INSERT INTO foobar (id, foo, bar, baz) VALUES ('a', 'a', 1, 'a'), ('b', 'b', 2, 'b')",
            )
            .unwrap();
        let result = engine
            .execute("SELECT * FROM foobar WHERE baz = 'a'")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].get("id"), Some(&Value::String("a".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_scan_item_limit() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY, ts NUMBER INDEX('ts-index'));
                 INSERT INTO foobar (id, bar, ts) VALUES ('a', 1, 100), ('a', 2, 200), ('a', 3, 300)",
            )
            .unwrap();
        let result = engine
            .execute("SELECT * FROM foobar WHERE id = 'a' AND ts > 200 LIMIT 1 SCAN LIMIT 2")
            .unwrap();
        assert_items_len(result, 0);
    }

    #[test]
    fn test_attrs() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY, order STRING);
                 INSERT INTO foobar (id, bar, order) VALUES ('a', 1, 'first'), ('a', 2, 'second')",
            )
            .unwrap();
        let result = engine
            .execute("SELECT order FROM foobar WHERE id = 'a' AND bar = 1")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(
                    items[0].get("order"),
                    Some(&Value::String("first".to_string()))
                );
                assert!(!items[0].contains_key("bar"));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_begins_with() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id NUMBER HASH KEY, bar STRING RANGE KEY);
                 INSERT INTO foobar (id, bar) VALUES (1, 'abc'), (1, 'def')",
            )
            .unwrap();
        let result = engine
            .execute("SELECT * FROM foobar WHERE id = 1 AND begins_with(bar, 'a')")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].get("bar"), Some(&Value::String("abc".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_between() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY);
                 INSERT INTO foobar (id, bar) VALUES ('a', 5), ('a', 10)",
            )
            .unwrap();
        let result = engine
            .execute("SELECT * FROM foobar WHERE id = 'a' AND bar BETWEEN 1 AND 8")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].get("bar"), Some(&Value::Number("5".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_get() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY);
                 INSERT INTO foobar (id, bar) VALUES ('a', 1), ('b', 2)",
            )
            .unwrap();
        let result = engine
            .execute("SELECT * FROM foobar KEYS IN ('a', 1), ('b', 2)")
            .unwrap();
        assert_items_len(result, 2);
    }

    #[test]
    fn test_limit() {
        let mut engine = seeded_table();
        let result = engine
            .execute("SELECT * FROM foobar WHERE id = 'a' LIMIT 1")
            .unwrap();
        assert_items_len(result, 1);
    }

    #[test]
    fn test_filter() {
        let mut engine = seeded_table();
        let result = engine
            .execute("SELECT * FROM foobar WHERE id = 'b' AND foo = 2")
            .unwrap();
        assert_items_len(result, 1);
    }

    #[test]
    fn test_filter_and() {
        let mut engine = seeded_table();
        let result = engine
            .execute("SELECT * FROM foobar WHERE id = 'b' AND foo = 2")
            .unwrap();
        assert_items_len(result, 1);
    }

    #[test]
    fn test_filter_or() {
        let mut engine = InMemoryEngine::default();
        let result = engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, foo NUMBER, bar NUMBER, baz NUMBER);
                 INSERT INTO foobar (id, foo, bar, baz) VALUES ('a', 1, 1, 1), ('a', 2, 2, 2);
                 SELECT * FROM foobar WHERE id = 'a' AND (baz = 1 OR foo = 2)",
            )
            .unwrap();
        assert_items_len(result, 2);
    }

    ignored_select!(
        test_count_smart_index => "needs count selection and index planner",
    );

    #[test]
    fn test_count() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY);
                 INSERT INTO foobar (id, bar) VALUES ('a', 1), ('a', 2)",
            )
            .unwrap();
        let result = engine
            .execute("SELECT count(*) FROM foobar WHERE id = 'a'")
            .unwrap();
        assert_eq!(result, StatementResult::Affected(2));
    }

    #[test]
    fn test_count_filter() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, range NUMBER RANGE KEY, foo NUMBER);
                 INSERT INTO foobar (id, range, foo) VALUES ('a', 1, 1), ('a', 2, 2)",
            )
            .unwrap();
        let result = engine
            .execute("SELECT count(*) FROM foobar WHERE id = 'a' AND foo = 1")
            .unwrap();
        assert_eq!(result, StatementResult::Affected(1));
    }

    #[test]
    fn test_explain_select() {
        let mut engine = seeded_table();
        let result = engine
            .execute("EXPLAIN SELECT * FROM foobar WHERE id = 'a'")
            .unwrap();
        assert_eq!(result, StatementResult::Schema("query foobar".to_string()));
    }

    #[test]
    fn test_explain_select_keys_in() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY)")
            .unwrap();
        let result = engine
            .execute("EXPLAIN SELECT * FROM foobar KEYS IN 'a', 'b'")
            .unwrap();
        assert_eq!(
            result,
            StatementResult::Schema("batch_get_item foobar".to_string())
        );
    }

    ignored_select!(
        test_order_by_index => "needs ORDER BY with index support",
        test_order_by => "needs ORDER BY support",
        test_select_non_projected => "needs projection and index follow-up support",
    );

    fn seeded_table() -> InMemoryEngine {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, range NUMBER RANGE KEY, foo NUMBER);
                 INSERT INTO foobar (id, range, foo) VALUES ('a', 1, 1), ('b', 1, 2)",
            )
            .unwrap();
        engine
    }
}

mod test_select_scan {
    use super::*;

    #[test]
    fn test_scan() {
        let mut engine = InMemoryEngine::default();
        let result = engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY);
                 INSERT INTO foobar (id) VALUES ('a'), ('b');
                 SCAN * FROM foobar",
            )
            .unwrap();
        assert_items_len(result, 2);
    }

    macro_rules! ignored_scan {
        ($($name:ident => $reason:expr),+ $(,)?) => {
            $(
                #[test]
                #[ignore = $reason]
                fn $name() {
                    pending(concat!("tests/test_queries.py::TestSelectScan::", stringify!($name)), $reason);
                }
            )+
        };
    }

    #[test]
    fn test_filter() {
        let mut engine = seeded_scan_table();
        let result = engine.execute("SCAN * FROM foobar WHERE bar = 2").unwrap();
        assert_items_len(result, 1);
    }

    #[test]
    fn test_limit() {
        let mut engine = seeded_scan_table();
        let result = engine.execute("SCAN * FROM foobar LIMIT 1").unwrap();
        assert_items_len(result, 1);
    }

    #[test]
    fn test_scan_limit() {
        let mut engine = seeded_scan_table();
        let result = engine.execute("SCAN * FROM foobar SCAN LIMIT 1").unwrap();
        assert_items_len(result, 1);
    }

    #[test]
    fn test_filter_and() {
        let mut engine = seeded_scan_table();
        let result = engine
            .execute("SCAN * FROM foobar WHERE id = 'b' AND bar = 2")
            .unwrap();
        assert_items_len(result, 1);
    }

    fn seeded_scan_table_no_range() -> InMemoryEngine {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY)")
            .unwrap();
        engine
    }

    #[test]
    fn test_select_filter_timestamp() {
        let mut engine = seeded_scan_table_no_range();
        engine
            .execute("INSERT INTO foobar (id, bar) VALUES ('a', NOW() + INTERVAL '1 hour')")
            .unwrap();
        let result = engine
            .execute("SCAN * FROM foobar WHERE bar > NOW()")
            .unwrap();
        assert_items_len(result, 1);
    }

    #[test]
    fn test_select_alias() {
        let mut engine = seeded_scan_table_no_range();
        engine
            .execute("INSERT INTO foobar (id, bar) VALUES ('a', 5)")
            .unwrap();
        let result = engine.execute("SCAN id, bar AS baz FROM foobar").unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items[0].get("baz"), Some(&Value::Number("5".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_select_operation() {
        let mut engine = seeded_scan_table_no_range();
        engine
            .execute("INSERT INTO foobar (id, bar, baz) VALUES ('a', 5, 3)")
            .unwrap();
        let result = engine.execute("SCAN bar + baz AS ret FROM foobar").unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items[0].get("ret"), Some(&Value::Number("8".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_select_none_operation() {
        let mut engine = seeded_scan_table_no_range();
        engine
            .execute("INSERT INTO foobar (id, bar) VALUES ('a', 5)")
            .unwrap();
        let result = engine.execute("SCAN bar + baz AS ret FROM foobar").unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items[0].get("ret"), Some(&Value::Number("5".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_select_type_error_operation() {
        let mut engine = seeded_scan_table_no_range();
        engine
            .execute("INSERT INTO foobar (id, bar, baz) VALUES ('a', 5, (1, 2))")
            .unwrap();
        let result = engine.execute("SCAN bar + baz AS ret FROM foobar").unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items[0].get("ret"), Some(&Value::Null));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_nested_operation() {
        let mut engine = seeded_scan_table_no_range();
        engine
            .execute("INSERT INTO foobar (id, foo, bar, baz) VALUES ('a', 10, 5, 3)")
            .unwrap();
        let result = engine
            .execute("SCAN foo - (bar - baz) AS ret FROM foobar")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items[0].get("ret"), Some(&Value::Number("8".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_select_timestamp() {
        let mut engine = seeded_scan_table_no_range();
        engine
            .execute("INSERT INTO foobar (id, bar) VALUES ('a', NOW())")
            .unwrap();
        let result = engine.execute("SCAN ts(bar) AS bar FROM foobar").unwrap();
        match result {
            StatementResult::Items(items) => {
                assert!(matches!(items[0].get("bar"), Some(Value::Timestamp(_))));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_select_timestamp_ms() {
        let mut engine = seeded_scan_table_no_range();
        engine
            .execute("INSERT INTO foobar (id, bar) VALUES ('a', MS(NOW()))")
            .unwrap();
        let result = engine.execute("SCAN ts(bar) AS bar FROM foobar").unwrap();
        match result {
            StatementResult::Items(items) => {
                assert!(matches!(
                    items[0].get("bar"),
                    Some(Value::Timestamp(_)) | Some(Value::Number(_))
                ));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_select_timestamp_literal() {
        let mut engine = seeded_scan_table_no_range();
        engine
            .execute("INSERT INTO foobar (id, bar) VALUES ('a', 4)")
            .unwrap();
        let result = engine
            .execute("SCAN ts('2015-12-5') AS d FROM foobar")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert!(matches!(items[0].get("d"), Some(Value::Timestamp(_))));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_select_now() {
        let mut engine = seeded_scan_table_no_range();
        engine
            .execute("INSERT INTO foobar (id, bar) VALUES ('a', 4)")
            .unwrap();
        let result = engine.execute("SCAN now() AS d FROM foobar").unwrap();
        match result {
            StatementResult::Items(items) => {
                assert!(matches!(items[0].get("d"), Some(Value::Timestamp(_))));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_select_timedelta() {
        let mut engine = seeded_scan_table_no_range();
        engine
            .execute("INSERT INTO foobar (id, bar) VALUES ('a', NOW())")
            .unwrap();
        let result = engine
            .execute("SCAN now() - ts(bar) AS d FROM foobar")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert!(matches!(items[0].get("d"), Some(Value::Interval(_))));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_begins_with() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id NUMBER HASH KEY, bar STRING RANGE KEY);
                 INSERT INTO foobar (id, bar) VALUES (1, 'abc'), (1, 'def')",
            )
            .unwrap();
        let result = engine
            .execute("SCAN * FROM foobar WHERE begins_with(bar, 'a')")
            .unwrap();
        match result {
            StatementResult::Items(items) => assert_eq!(items.len(), 1),
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_between() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY);
                 INSERT INTO foobar (id, bar) VALUES ('a', 5), ('a', 10);
                 SCAN * FROM foobar WHERE bar BETWEEN 1 AND 8",
            )
            .unwrap();
        let result = engine
            .execute("SCAN * FROM foobar WHERE bar BETWEEN 1 AND 8")
            .unwrap();
        match result {
            StatementResult::Items(items) => assert_eq!(items.len(), 1),
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_null() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY);
                 INSERT INTO foobar (id, bar) VALUES ('a', 5);
                 INSERT INTO foobar (id, bar, baz) VALUES ('a', 1, 1);
                 SCAN * FROM foobar WHERE attribute_not_exists(baz)",
            )
            .unwrap();
        let result = engine
            .execute("SCAN * FROM foobar WHERE attribute_not_exists(baz)")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].get("bar"), Some(&Value::Number("5".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_attribute_not_exists_quoted() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY);
                 INSERT INTO foobar (id, bar) VALUES ('a', 5);
                 INSERT INTO foobar (id, bar, baz) VALUES ('a', 1, 1);
                 SCAN * FROM foobar WHERE attribute_not_exists('baz')",
            )
            .unwrap();
        let result = engine
            .execute("SCAN * FROM foobar WHERE attribute_not_exists('baz')")
            .unwrap();
        assert_items_len(result, 1);
    }

    #[test]
    fn test_not_null() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY);
                 INSERT INTO foobar (id, bar) VALUES ('a', 5);
                 INSERT INTO foobar (id, bar, baz) VALUES ('a', 1, 1);
                 SCAN * FROM foobar WHERE attribute_exists(baz)",
            )
            .unwrap();
        let result = engine
            .execute("SCAN * FROM foobar WHERE attribute_exists(baz)")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].get("baz"), Some(&Value::Number("1".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_attribute_exists_quoted() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY);
                 INSERT INTO foobar (id, bar) VALUES ('a', 5);
                 INSERT INTO foobar (id, bar, baz) VALUES ('a', 1, 1);
                 SCAN * FROM foobar WHERE attribute_exists('baz')",
            )
            .unwrap();
        let result = engine
            .execute("SCAN * FROM foobar WHERE attribute_exists('baz')")
            .unwrap();
        assert_items_len(result, 1);
    }

    #[test]
    fn test_in() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY);
                 INSERT INTO foobar (id, bar) VALUES ('a', 5), ('a', 2);
                 SCAN * FROM foobar WHERE bar IN (1, 3, 5)",
            )
            .unwrap();
        let result = engine
            .execute("SCAN * FROM foobar WHERE bar IN (1, 3, 5)")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].get("bar"), Some(&Value::Number("5".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_contains() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY);
                 INSERT INTO foobar (id, bar, baz) VALUES ('a', 5, (1, 2, 3)), ('a', 1, (4, 5, 6));
                 SCAN * FROM foobar WHERE contains(baz, 2)",
            )
            .unwrap();
        let result = engine
            .execute("SCAN * FROM foobar WHERE contains(baz, 2)")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].get("bar"), Some(&Value::Number("5".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_scan_global() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, foo STRING) \
                 GLOBAL KEYS INDEX ('gindex', foo);
                 INSERT INTO foobar (id, foo) VALUES ('a', 'a');
                 SCAN * FROM foobar USING gindex",
            )
            .unwrap();
        assert_items_len(
            engine.execute("SCAN * FROM foobar USING gindex").unwrap(),
            1,
        );
    }

    #[test]
    fn test_scan_global_with_constraints() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, foo STRING) \
                 GLOBAL KEYS INDEX ('gindex', foo);
                 INSERT INTO foobar (id, foo) VALUES ('a', 'a'), ('b', 'b');
                 SCAN * FROM foobar WHERE id = 'a' USING gindex",
            )
            .unwrap();
        let result = engine
            .execute("SCAN * FROM foobar WHERE id = 'a' USING gindex")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].get("id"), Some(&Value::String("a".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_filter_list() {
        let mut engine = seeded_scan_table_no_range();
        engine
            .execute("INSERT INTO foobar (id, bar) VALUES ('a', [1, 2]), ('b', [2, 3])")
            .unwrap();
        let result = engine
            .execute("SCAN * FROM foobar WHERE bar[0] = 2")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].get("id"), Some(&Value::String("b".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_filter_map() {
        let mut engine = seeded_scan_table_no_range();
        engine
            .execute("INSERT INTO foobar (id, bar) VALUES ('a', {'b': 1}), ('b', {'b': 2})")
            .unwrap();
        let result = engine
            .execute("SCAN * FROM foobar WHERE bar.b = 2")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].get("id"), Some(&Value::String("b".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_explain_scan() {
        let mut engine = seeded_scan_table_no_range();
        let result = engine
            .execute("EXPLAIN SCAN * FROM foobar WHERE bar = 1")
            .unwrap();
        assert_eq!(result, StatementResult::Schema("scan foobar".to_string()));
    }

    #[test]
    fn test_field_ne_field() {
        let mut engine = seeded_scan_table_no_range();
        engine
            .execute("INSERT INTO foobar (id, bar, baz) VALUES ('a', 1, 1), ('b', 2, 3)")
            .unwrap();
        let result = engine
            .execute("SCAN * FROM foobar WHERE bar <> baz")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].get("id"), Some(&Value::String("b".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_filter_or() {
        let mut engine = seeded_scan_table();
        let result = engine
            .execute("SCAN * FROM foobar WHERE bar = 1 OR bar = 2")
            .unwrap();
        assert_items_len(result, 2);
    }

    #[test]
    fn test_filter_nested() {
        let mut engine = InMemoryEngine::default();
        let result = engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, foo NUMBER, bar NUMBER);
                 INSERT INTO foobar (id, foo, bar) VALUES ('a', 1, 1), ('b', 1, 2), ('c', 1, 3);
                 SCAN * FROM foobar WHERE foo = 1 AND NOT (bar = 2 OR bar = 3)",
            )
            .unwrap();
        assert_items_len(result, 1);
    }

    fn seeded_scan_table() -> InMemoryEngine {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER);
                 INSERT INTO foobar (id, bar) VALUES ('a', 1), ('b', 2)",
            )
            .unwrap();
        engine
    }
}

mod test_create {
    use super::*;
    use dql_engine::DynamoBackend;
    use dql_models::{ProjectionType, TableMeta};
    use dql_parser::AttributeType;

    fn describe_table(engine: &InMemoryEngine, name: &str) -> TableMeta {
        engine
            .backend()
            .describe_table(name)
            .unwrap()
            .expect("table should exist")
    }

    fn dump_schema(engine: &mut InMemoryEngine) -> String {
        match engine.execute("DUMP SCHEMA").unwrap() {
            StatementResult::Schema(schema) => schema,
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_create() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (owner STRING HASH KEY, id BINARY RANGE KEY, ts NUMBER INDEX('ts-index'))",
            )
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        assert_eq!(desc.hash_key, "owner");
        assert_eq!(desc.range_key.as_deref(), Some("id"));
        assert!(desc.local_indexes.contains_key("ts-index"));
    }

    #[test]
    fn test_create_throughput() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY, THROUGHPUT (1, 2))")
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        assert_eq!(
            desc.throughput.as_ref().map(|tp| tp.read.clone()),
            Some(Value::Number("1".to_string()))
        );
    }

    #[test]
    fn test_create_if_not_exists() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY);
                 CREATE TABLE IF NOT EXISTS foobar (id STRING HASH KEY)",
            )
            .unwrap();
    }

    #[test]
    fn test_create_keys_index() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (owner STRING HASH KEY, id BINARY RANGE KEY, ts NUMBER KEYS INDEX('ts-index'))",
            )
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        let index = desc.local_indexes.get("ts-index").unwrap();
        assert_eq!(index.projection, ProjectionType::KeysOnly);
    }

    #[test]
    fn test_create_include_index() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (owner STRING HASH KEY, id BINARY RANGE KEY, ts NUMBER INCLUDE INDEX('ts-index', ['foo', 'bar']))",
            )
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        let index = desc.local_indexes.get("ts-index").unwrap();
        assert!(matches!(index.projection, ProjectionType::Include(_)));
    }

    #[test]
    fn test_create_global_indexes() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, foo NUMBER RANGE KEY, THROUGHPUT (1, 1)) \
                 GLOBAL INDEX ('myindex', foo, id, THROUGHPUT (1, 2))",
            )
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        let index = desc.global_indexes.get("myindex").unwrap();
        assert_eq!(index.hash_key.name, "foo");
        assert_eq!(
            index.range_key.as_ref().map(|field| field.name.as_str()),
            Some("id")
        );
    }

    #[test]
    fn test_create_global_index_types() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, foo NUMBER RANGE KEY, THROUGHPUT (1, 1)) \
                 GLOBAL INDEX ('myindex', foo number, baz string, THROUGHPUT (1, 2))",
            )
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        let index = desc.global_indexes.get("myindex").unwrap();
        assert_eq!(index.hash_key.data_type, AttributeType::Number);
        assert_eq!(
            index
                .range_key
                .as_ref()
                .map(|field| field.data_type.clone()),
            Some(AttributeType::String)
        );
        assert!(desc.attrs.contains_key("baz"));
    }

    #[test]
    fn test_create_global_index_no_range() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, foo NUMBER, THROUGHPUT (1, 1)) \
                 GLOBAL ALL INDEX ('myindex', foo, THROUGHPUT (1, 2))",
            )
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        let index = desc.global_indexes.get("myindex").unwrap();
        assert_eq!(index.hash_key.name, "foo");
        assert!(index.range_key.is_none());
    }

    #[test]
    fn test_create_global_keys_index() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, foo NUMBER, THROUGHPUT (1, 1)) \
                 GLOBAL KEYS INDEX ('myindex', foo, THROUGHPUT (1, 2))",
            )
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        let index = desc.global_indexes.get("myindex").unwrap();
        assert_eq!(index.projection, ProjectionType::KeysOnly);
    }

    #[test]
    fn test_create_global_include_index() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, foo NUMBER, THROUGHPUT (1, 1)) \
                 GLOBAL INCLUDE INDEX ('myindex', foo, ['bar', 'baz'], THROUGHPUT (1, 2))",
            )
            .unwrap();
        let desc = describe_table(&engine, "foobar");
        let index = desc.global_indexes.get("myindex").unwrap();
        assert!(matches!(index.projection, ProjectionType::Include(_)));
    }

    #[test]
    fn test_create_lsi_dump_round_trip() {
        let mut engine = InMemoryEngine::default();
        let create = "CREATE TABLE foobar (owner STRING HASH KEY, id BINARY RANGE KEY, ts NUMBER KEYS INDEX('ts-index'))";
        engine.execute(create).unwrap();
        let dumped = dump_schema(&mut engine);
        assert!(dumped.contains("KEYS INDEX('ts-index')"), "{dumped}");
        engine.execute("DROP TABLE foobar").unwrap();
        engine.execute(&dumped).unwrap();
        let desc = describe_table(&engine, "foobar");
        assert_eq!(
            desc.local_indexes.get("ts-index").unwrap().projection,
            ProjectionType::KeysOnly
        );
    }

    #[test]
    fn test_create_explain() {
        let mut engine = InMemoryEngine::default();
        let result = engine
            .execute("EXPLAIN CREATE TABLE foobar (id STRING HASH KEY)")
            .unwrap();
        assert_eq!(
            result,
            StatementResult::Schema("create_table foobar".to_string())
        );
    }
}

mod test_update {
    use super::*;

    fn make_table(engine: &mut InMemoryEngine, range_key: bool, index: Option<&str>) {
        let mut create = "CREATE TABLE foobar (id STRING HASH KEY".to_string();
        if range_key {
            create.push_str(", bar NUMBER RANGE KEY");
        }
        if let Some(index) = index {
            create.push_str(&format!(", {index} NUMBER INDEX('{index}-index')"));
        }
        create.push(')');
        engine.execute(&create).unwrap();
    }

    fn scan_items(engine: &mut InMemoryEngine) -> Vec<Item> {
        match engine.execute("SCAN * FROM foobar").unwrap() {
            StatementResult::Items(items) => items,
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_update() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, true, None);
        engine
            .execute("INSERT INTO foobar (id, bar, baz) VALUES ('a', 1, 1), ('b', 2, 2)")
            .unwrap();
        engine.execute("UPDATE foobar SET baz = 3").unwrap();
        let items = scan_items(&mut engine);
        assert_eq!(items.len(), 2);
        assert!(items
            .iter()
            .all(|item| item.get("baz") == Some(&Value::Number("3".to_string()))));
    }

    #[test]
    fn test_update_where() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, true, None);
        engine
            .execute("INSERT INTO foobar (id, bar, baz) VALUES ('a', 1, 1), ('b', 2, 2)")
            .unwrap();
        engine
            .execute("UPDATE foobar SET baz = 3 WHERE id = 'a'")
            .unwrap();
        let items = scan_items(&mut engine);
        assert_eq!(
            items
                .iter()
                .find(|item| item.get("id") == Some(&Value::String("a".to_string())))
                .and_then(|item| item.get("baz")),
            Some(&Value::Number("3".to_string()))
        );
        assert_eq!(
            items
                .iter()
                .find(|item| item.get("id") == Some(&Value::String("b".to_string())))
                .and_then(|item| item.get("baz")),
            Some(&Value::Number("2".to_string()))
        );
    }

    #[test]
    fn test_update_count() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, true, None);
        engine
            .execute("INSERT INTO foobar (id, bar, baz) VALUES ('a', 1, 1), ('b', 2, 2)")
            .unwrap();
        let result = engine
            .execute("UPDATE foobar SET baz = 3 WHERE id = 'a'")
            .unwrap();
        assert_eq!(result, StatementResult::Affected(1));
    }

    #[test]
    fn test_update_in_condition() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, false, None);
        engine
            .execute("INSERT INTO foobar (id, bar) VALUES ('a', 1), ('b', 2)")
            .unwrap();
        engine
            .execute("UPDATE foobar SET bar = 3 KEYS IN ('a'), ('b') WHERE bar < 2")
            .unwrap();
        let items = scan_items(&mut engine);
        assert_eq!(
            items
                .iter()
                .find(|item| item.get("id") == Some(&Value::String("a".to_string())))
                .and_then(|item| item.get("bar")),
            Some(&Value::Number("3".to_string()))
        );
        assert_eq!(
            items
                .iter()
                .find(|item| item.get("id") == Some(&Value::String("b".to_string())))
                .and_then(|item| item.get("bar")),
            Some(&Value::Number("2".to_string()))
        );
    }

    #[test]
    fn test_update_keys_count() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, true, None);
        engine
            .execute("INSERT INTO foobar (id, bar, baz) VALUES ('a', 1, 1), ('b', 2, 2)")
            .unwrap();
        let result = engine
            .execute("UPDATE foobar SET baz = 3 KEYS IN ('a', 1), ('b', 2)")
            .unwrap();
        assert_eq!(result, StatementResult::Affected(2));
    }

    #[test]
    fn test_update_increment() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, true, None);
        engine
            .execute("INSERT INTO foobar (id, bar, baz) VALUES ('a', 1, 1), ('b', 2, 2)")
            .unwrap();
        engine.execute("UPDATE foobar ADD baz 2").unwrap();
        engine.execute("UPDATE foobar ADD baz -1").unwrap();
        let items = scan_items(&mut engine);
        assert_eq!(
            items
                .iter()
                .find(|item| item.get("id") == Some(&Value::String("a".to_string())))
                .and_then(|item| item.get("baz")),
            Some(&Value::Number("2".to_string()))
        );
        assert_eq!(
            items
                .iter()
                .find(|item| item.get("id") == Some(&Value::String("b".to_string())))
                .and_then(|item| item.get("baz")),
            Some(&Value::Number("3".to_string()))
        );
    }

    #[test]
    fn test_update_add() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, true, None);
        engine
            .execute("INSERT INTO foobar (id, bar, baz) VALUES ('a', 1, ())")
            .unwrap();
        engine.execute("UPDATE foobar ADD baz (1)").unwrap();
        engine.execute("UPDATE foobar ADD baz (2, 3)").unwrap();
        let items = scan_items(&mut engine);
        assert_eq!(
            items[0].get("baz"),
            Some(&Value::Set(vec![
                Value::Number("1".to_string()),
                Value::Number("2".to_string()),
                Value::Number("3".to_string()),
            ]))
        );
    }

    #[test]
    fn test_update_delete() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, true, None);
        engine
            .execute("INSERT INTO foobar (id, bar, baz) VALUES ('a', 1, (1, 2, 3, 4))")
            .unwrap();
        engine.execute("UPDATE foobar DELETE baz (2)").unwrap();
        engine.execute("UPDATE foobar DELETE baz (1, 3)").unwrap();
        let items = scan_items(&mut engine);
        assert_eq!(
            items[0].get("baz"),
            Some(&Value::Set(vec![Value::Number("4".to_string())]))
        );
    }

    #[test]
    fn test_update_remove() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, true, None);
        engine
            .execute("INSERT INTO foobar (id, bar, baz) VALUES ('a', 1, 1), ('b', 2, 2)")
            .unwrap();
        engine.execute("UPDATE foobar REMOVE baz").unwrap();
        let items = scan_items(&mut engine);
        assert!(items.iter().all(|item| !item.contains_key("baz")));
    }

    #[test]
    fn test_update_returns() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, true, None);
        engine
            .execute("INSERT INTO foobar (id, bar, baz) VALUES ('a', 1, 1), ('b', 2, 2)")
            .unwrap();
        let result = engine
            .execute("UPDATE foobar REMOVE baz RETURNS ALL NEW")
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 2);
                assert!(items.iter().all(|item| !item.contains_key("baz")));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_update_soft() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, false, None);
        engine
            .execute("INSERT INTO foobar (id, bar) VALUES ('a', 1), ('b', NULL)")
            .unwrap();
        engine
            .execute("UPDATE foobar SET bar = if_not_exists(bar, 2)")
            .unwrap();
        let items = scan_items(&mut engine);
        assert_eq!(
            items
                .iter()
                .find(|item| item.get("id") == Some(&Value::String("b".to_string())))
                .and_then(|item| item.get("bar")),
            Some(&Value::Number("2".to_string()))
        );
    }

    #[test]
    fn test_update_append() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, false, None);
        engine
            .execute("INSERT INTO foobar (id, bar) VALUES ('a', [1])")
            .unwrap();
        engine
            .execute("UPDATE foobar SET bar = list_append(bar, [2])")
            .unwrap();
        let items = scan_items(&mut engine);
        assert_eq!(
            items[0].get("bar"),
            Some(&Value::List(vec![
                Value::Number("1".to_string()),
                Value::Number("2".to_string())
            ]))
        );
    }

    #[test]
    fn test_update_prepend() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, false, None);
        engine
            .execute("INSERT INTO foobar (id, bar) VALUES ('a', [1])")
            .unwrap();
        engine
            .execute("UPDATE foobar SET bar = list_append([2], bar)")
            .unwrap();
        let items = scan_items(&mut engine);
        assert_eq!(
            items[0].get("bar"),
            Some(&Value::List(vec![
                Value::Number("2".to_string()),
                Value::Number("1".to_string())
            ]))
        );
    }

    #[test]
    fn test_update_condition() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, false, None);
        engine
            .execute("INSERT INTO foobar (id, bar) VALUES ('a', 1), ('b', 2)")
            .unwrap();
        engine
            .execute("UPDATE foobar SET bar = 3 WHERE bar < 2")
            .unwrap();
        let items = scan_items(&mut engine);
        assert_eq!(
            items
                .iter()
                .find(|item| item.get("id") == Some(&Value::String("a".to_string())))
                .and_then(|item| item.get("bar")),
            Some(&Value::Number("3".to_string()))
        );
    }

    #[test]
    fn test_update_index() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, true, Some("ts"));
        engine
            .execute("INSERT INTO foobar (id, bar, ts) VALUES ('a', 1, 100)")
            .unwrap();
        engine
            .execute("UPDATE foobar SET ts = 3 WHERE id = 'a' USING ts-index")
            .unwrap();
        let items = scan_items(&mut engine);
        assert_eq!(items[0].get("ts"), Some(&Value::Number("3".to_string())));
    }

    #[test]
    fn test_explain_update() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, true, None);
        let result = engine
            .execute("EXPLAIN UPDATE foobar SET baz = 1 WHERE id = 'a'")
            .unwrap();
        assert_eq!(
            result,
            StatementResult::Schema("query foobar\nupdate_item foobar".to_string())
        );
    }

    #[test]
    fn test_explain_update_get() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, false, None);
        let result = engine
            .execute("EXPLAIN UPDATE foobar SET baz = 1 KEYS IN 'a', 'b'")
            .unwrap();
        assert_eq!(
            result,
            StatementResult::Schema("update_item foobar".to_string())
        );
    }

    #[test]
    fn test_explain_update_scan() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, false, None);
        let result = engine
            .execute("EXPLAIN UPDATE foobar SET baz = 1 WHERE bar = 1")
            .unwrap();
        assert_eq!(
            result,
            StatementResult::Schema("scan foobar\nupdate_item foobar".to_string())
        );
    }

    #[test]
    fn test_update_bool() {
        let mut engine = InMemoryEngine::default();
        make_table(&mut engine, false, None);
        engine
            .execute("INSERT INTO foobar (id, bar) VALUES ('a', true)")
            .unwrap();
        engine.execute("UPDATE foobar SET bar = false").unwrap();
        let items = scan_items(&mut engine);
        assert_eq!(items[0].get("bar"), Some(&Value::Bool(false)));
    }

    #[test]
    fn test_update_where_in() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY);
                 INSERT INTO foobar (id, bar, baz) VALUES ('a', 1, 1), ('b', 2, 2);
                 UPDATE foobar SET baz = 3 KEYS IN ('a', 1), ('b', 2)",
            )
            .unwrap();
        let result = engine.execute("SCAN * FROM foobar").unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 2);
                assert!(items
                    .iter()
                    .all(|item| item.get("baz") == Some(&Value::Number("3".to_string()))));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }
}

mod test_delete {
    use super::*;

    #[test]
    fn test_delete() {
        let mut engine = seeded_delete_table();
        let result = engine.execute("DELETE FROM foobar").unwrap();
        assert_eq!(result, StatementResult::Affected(2));
        let result = engine.execute("SCAN * FROM foobar").unwrap();
        assert_items_len(result, 0);
    }

    #[test]
    fn test_delete_where() {
        let mut engine = seeded_delete_table();
        let result = engine.execute("DELETE FROM foobar WHERE id = 'a'").unwrap();
        assert_eq!(result, StatementResult::Affected(1));
        let result = engine.execute("SCAN * FROM foobar").unwrap();
        assert_items_len(result, 1);
    }

    #[test]
    fn test_explain_delete_query() {
        let mut engine = seeded_delete_table();
        let result = engine
            .execute("EXPLAIN DELETE FROM foobar WHERE id = 'a'")
            .unwrap();
        assert_eq!(
            result,
            StatementResult::Schema("query foobar\ndelete_item foobar".to_string())
        );
    }

    #[test]
    fn test_explain_delete_scan() {
        let mut engine = seeded_delete_table();
        let result = engine.execute("EXPLAIN DELETE FROM foobar").unwrap();
        assert_eq!(
            result,
            StatementResult::Schema("scan foobar\ndelete_item foobar".to_string())
        );
    }

    #[test]
    fn test_delete_in() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY);
                 INSERT INTO foobar (id, bar) VALUES ('a', 1), ('b', 2);
                 DELETE FROM foobar KEYS IN ('a', 1)",
            )
            .unwrap();
        let result = engine.execute("SCAN * FROM foobar").unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].get("id"), Some(&Value::String("b".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_delete_in_filter() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY);
                 INSERT INTO foobar (id, bar) VALUES ('a', 1), ('b', 2);
                 DELETE FROM foobar KEYS IN 'a', 'b' WHERE bar = 1",
            )
            .unwrap();
        let result = engine.execute("SCAN * FROM foobar").unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].get("id"), Some(&Value::String("b".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_delete_smart_index() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY, ts NUMBER INDEX('ts-index'));
                 INSERT INTO foobar (id, bar, ts) VALUES ('a', 1, 100), ('a', 2, 200);
                 DELETE FROM foobar WHERE id = 'a' AND ts > 150",
            )
            .unwrap();
        let result = engine.execute("SCAN * FROM foobar").unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].get("ts"), Some(&Value::Number("100".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn test_delete_using() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER RANGE KEY, ts NUMBER INDEX('ts-index'));
                 INSERT INTO foobar (id, bar, ts) VALUES ('a', 1, 0), ('a', 2, 5);
                 DELETE FROM foobar WHERE id = 'a' AND ts < 8 USING ts-index",
            )
            .unwrap();
        let result = engine.execute("SCAN * FROM foobar").unwrap();
        assert_items_len(result, 0);
    }

    #[test]
    fn test_explain_delete_get() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY)")
            .unwrap();
        let result = engine
            .execute("EXPLAIN DELETE FROM foobar KEYS IN 'a', 'b'")
            .unwrap();
        assert_eq!(
            result,
            StatementResult::Schema("delete_item foobar".to_string())
        );
    }

    fn seeded_delete_table() -> InMemoryEngine {
        let mut engine = InMemoryEngine::default();
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY, bar NUMBER);
                 INSERT INTO foobar (id, bar) VALUES ('a', 1), ('b', 2)",
            )
            .unwrap();
        engine
    }
}

mod test_regressions {
    use super::*;

    macro_rules! ignored_regression {
        ($($name:ident => $reason:expr),+ $(,)?) => {
            $(
                #[test]
                #[ignore = $reason]
                fn $name() {
                    pending(concat!("tests/test_queries.py::TestRegressions::", stringify!($name)), $reason);
                }
            )+
        };
    }

    ignored_regression!(
        test_filter_banned_word => "needs DynamoDB expression reserved-word escaping",
        test_filter_with_dash => "needs dashed field path support",
        test_count_on_index => "needs count on GSI support",
    );
}

mod test_models {
    use super::*;

    #[test]
    #[ignore = "needs GSI throughput metadata"]
    fn test_total_throughput() {
        pending(
            "tests/test_models.py::TestModels::test_total_throughput",
            "global index throughput metadata is deferred",
        );
    }

    #[test]
    fn test_format_throughput_for_available_throughput() {
        assert_eq!(format_throughput(Some(20.0), None), "20");
    }

    #[test]
    fn test_format_throughput_for_two_inputs() {
        assert_eq!(format_throughput(Some(20.0), Some(10.0)), "10/20 (50%)");
    }

    #[test]
    fn test_format_throughput_for_two_inputs_which_will_result_in_a_fraction() {
        assert_eq!(format_throughput(Some(20.0), Some(7.0)), "7/20 (35%)");
    }

    #[test]
    fn test_format_throughput_for_when_available_is_zero() {
        assert_eq!(format_throughput(Some(0.0), Some(7.0)), "7/∞");
        assert_eq!(format_throughput(Some(0.0), None), "N/A");
    }
}

mod test_save {
    use super::*;

    #[test]
    #[ignore = "needs SAVE and LOAD file format implementation"]
    fn test_file_formats() {
        pending(
            "tests/test_save.py::TestSave::test_file_formats",
            "SAVE/LOAD CSV, JSON, gz, and pickle formats are deferred",
        );
    }
}
