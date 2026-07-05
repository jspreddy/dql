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

    macro_rules! ignored_alter {
        ($($name:ident),+ $(,)?) => {
            $(
                #[test]
                #[ignore = "needs ALTER implementation"]
                fn $name() {
                    pending(concat!("tests/test_queries.py::TestAlter::", stringify!($name)), "ALTER is deferred");
                }
            )+
        };
    }

    ignored_alter!(
        test_alter_throughput,
        test_alter_throughput_partial_star,
        test_alter_billing_mode,
        test_alter_billing_mode_provisioned,
        test_alter_index_throughput,
        test_alter_drop,
        test_alter_create,
        test_explain_throughput,
        test_explain_create_index,
        test_alter_create_if_not_exists,
        test_alter_drop_if_exists,
    );
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
    #[ignore = "needs keyword-style INSERT syntax"]
    fn test_insert_keywords() {
        pending(
            "tests/test_queries.py::TestInsert::test_insert_keywords",
            "INSERT map/keyword syntax is deferred",
        );
    }

    #[test]
    #[ignore = "needs timestamp value evaluation"]
    fn test_insert_timestamps() {
        pending(
            "tests/test_queries.py::TestInsert::test_insert_timestamps",
            "timestamp literals are deferred",
        );
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
        test_hash_index => "needs index planner",
        test_smart_index => "needs index planner",
        test_smart_global_index => "needs GSI planner",
        test_scan_item_limit => "needs SCAN LIMIT support",
        test_attrs => "needs projection support",
        test_begins_with => "needs function constraints",
        test_between => "needs BETWEEN constraints",
    );

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

    ignored_scan!(
        test_begins_with => "needs begins_with constraints",
        test_between => "needs BETWEEN constraints",
        test_null => "needs NULL constraints",
        test_attribute_not_exists_quoted => "needs attribute_not_exists constraints",
        test_not_null => "needs NOT constraints",
        test_attribute_exists_quoted => "needs attribute_exists constraints",
        test_in => "needs IN constraints",
        test_contains => "needs contains constraints",
        test_scan_global => "needs GSI scan support",
        test_scan_global_with_constraints => "needs GSI scan constraints",
        test_filter_list => "needs list path constraints",
        test_filter_map => "needs map path constraints",
        test_explain_scan => "needs scan explain parity",
        test_field_ne_field => "needs field-to-field constraints",
        test_select_filter_timestamp => "needs timestamp constraints",
        test_select_alias => "needs selection aliases",
        test_select_operation => "needs selection arithmetic",
        test_select_none_operation => "needs nullable selection arithmetic",
        test_select_type_error_operation => "needs Python-compatible selection type errors",
        test_nested_operation => "needs nested selection arithmetic",
        test_select_timestamp => "needs timestamp selection functions",
        test_select_timestamp_ms => "needs timestamp millisecond functions",
        test_select_timestamp_literal => "needs timestamp literal functions",
        test_select_now => "needs now() selection",
        test_select_timedelta => "needs interval arithmetic",
    );

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

    #[test]
    fn test_create() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY, range NUMBER RANGE KEY)")
            .unwrap();
        assert_eq!(engine.table_names().unwrap(), vec!["foobar".to_string()]);
    }

    #[test]
    fn test_create_throughput() {
        let mut engine = InMemoryEngine::default();
        engine
            .execute("CREATE TABLE foobar (id STRING HASH KEY, THROUGHPUT (1, 1))")
            .unwrap();
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

    macro_rules! ignored_create {
        ($($name:ident => $reason:expr),+ $(,)?) => {
            $(
                #[test]
                #[ignore = $reason]
                fn $name() {
                    pending(concat!("tests/test_queries.py::TestCreate::", stringify!($name)), $reason);
                }
            )+
        };
    }

    ignored_create!(
        test_create_keys_index => "needs LSI metadata",
        test_create_include_index => "needs LSI projection metadata",
        test_create_global_indexes => "needs GSI metadata",
        test_create_global_index_types => "needs GSI key type metadata",
        test_create_global_index_no_range => "needs GSI metadata",
        test_create_global_keys_index => "needs GSI projection metadata",
        test_create_global_include_index => "needs GSI projection metadata",
    );

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

    macro_rules! ignored_update {
        ($($name:ident),+ $(,)?) => {
            $(
                #[test]
                #[ignore = "needs UPDATE implementation"]
                fn $name() {
                    pending(concat!("tests/test_queries.py::TestUpdate::", stringify!($name)), "UPDATE is deferred");
                }
            )+
        };
    }

    ignored_update!(
        test_update,
        test_update_where,
        test_update_count,
        test_update_in_condition,
        test_update_keys_count,
        test_update_increment,
        test_update_add,
        test_update_delete,
        test_update_remove,
        test_update_returns,
        test_update_soft,
        test_update_append,
        test_update_prepend,
        test_update_condition,
        test_update_index,
        test_explain_update,
        test_explain_update_get,
        test_explain_update_scan,
        test_update_bool,
    );

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

    macro_rules! ignored_delete {
        ($($name:ident => $reason:expr),+ $(,)?) => {
            $(
                #[test]
                #[ignore = $reason]
                fn $name() {
                    pending(concat!("tests/test_queries.py::TestDelete::", stringify!($name)), $reason);
                }
            )+
        };
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

    ignored_delete!(
        test_delete_in_filter => "needs DELETE IN constraints",
        test_delete_smart_index => "needs DELETE index planner",
        test_delete_using => "needs DELETE USING support",
        test_explain_delete_get => "needs DELETE explain get support",
    );

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
