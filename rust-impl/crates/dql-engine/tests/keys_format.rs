//! One key format for KEYS IN, WHERE ... IN, and the web table browser's Keys box:
//! `a, b, c` is a list of hash keys, `(a, 1)` is a (hash, range) pair, and
//! `(a, 1), (b, 2)` is a list of pairs. Whole keys are fetched with one batch get; on a
//! table with a hash and range key, a lone hash value reads every item under it.

use dql_engine::{InMemoryEngine, StatementResult};
use dql_parser::{parse_statement, Condition, Statement, Value};

fn engine(schema: &str, rows: &str) -> InMemoryEngine {
    let mut engine = InMemoryEngine::default();
    engine.execute(&format!("{schema}; {rows}")).unwrap();
    engine
}

fn ids(result: StatementResult, field: &str) -> Vec<String> {
    let StatementResult::Items(items) = result else {
        panic!("expected items, got {result:?}");
    };
    let mut ids: Vec<String> = items
        .iter()
        .map(|item| match item.get(field) {
            Some(Value::String(value)) => value.clone(),
            Some(Value::Number(value)) => value.clone(),
            other => panic!("unexpected {field}: {other:?}"),
        })
        .collect();
    ids.sort();
    ids
}

fn explain(engine: &mut InMemoryEngine, statement: &str) -> String {
    match engine.execute(&format!("EXPLAIN {statement}")).unwrap() {
        StatementResult::Schema(text) => text,
        other => panic!("unexpected explain result: {other:?}"),
    }
}

fn hash_and_range() -> InMemoryEngine {
    engine(
        "CREATE TABLE t (id STRING HASH KEY, sk NUMBER RANGE KEY)",
        "INSERT INTO t (id, sk, tag) VALUES ('a', 1, 'a1'), ('a', 2, 'a2'), ('b', 1, 'b1'), ('c', 3, 'c3'), ('d', 4, 'd4')",
    )
}

fn hash_only() -> InMemoryEngine {
    engine(
        "CREATE TABLE h (id STRING HASH KEY)",
        "INSERT INTO h (id) VALUES ('a'), ('b'), ('c'), ('d')",
    )
}

#[test]
fn a_list_is_hash_keys_and_tuples_are_pairs_in_keys_in() {
    let mut table = hash_and_range();
    let pairs = table
        .execute("SELECT * FROM t KEYS IN (a, 1), (b, 1), (c, 3)")
        .unwrap();
    assert_eq!(ids(pairs, "tag"), ["a1", "b1", "c3"]);
    let wrapped = table
        .execute("SELECT * FROM t KEYS IN ((a, 1), (b, 1))")
        .unwrap();
    assert_eq!(ids(wrapped, "tag"), ["a1", "b1"]);
    // A lone hash value is every item under that hash key.
    let hashes = table.execute("SELECT * FROM t KEYS IN a, b").unwrap();
    assert_eq!(ids(hashes, "tag"), ["a1", "a2", "b1"]);
    let mixed = table.execute("SELECT * FROM t KEYS IN (a, 1), c").unwrap();
    assert_eq!(ids(mixed, "tag"), ["a1", "c3"]);
    let count = table
        .execute("SELECT count(*) FROM t KEYS IN (d, 4), a")
        .unwrap();
    assert_eq!(count, StatementResult::Affected(3));

    let mut hashes_only = hash_only();
    let listed = hashes_only
        .execute("SELECT * FROM h KEYS IN a, b, c")
        .unwrap();
    assert_eq!(ids(listed, "id"), ["a", "b", "c"]);
}

#[test]
fn a_tuple_has_at_most_two_parts() {
    let mut table = hash_and_range();
    for statement in [
        "SELECT * FROM t KEYS IN (a, b, c)",
        "SELECT * FROM t WHERE (id, sk, tag) IN ((a, 1, x))",
    ] {
        let error = table.execute(statement).unwrap_err();
        assert!(
            error.to_string().contains("at most two parts"),
            "{statement}: {error}"
        );
    }
}

#[test]
fn where_in_uses_the_same_format() {
    let mut table = hash_and_range();
    let pairs = table
        .execute("SELECT * FROM t WHERE (id, sk) IN ((a, 1), (c, 3))")
        .unwrap();
    assert_eq!(ids(pairs, "tag"), ["a1", "c3"]);
    let hashes = table.execute("SELECT * FROM t WHERE id IN (a, b)").unwrap();
    assert_eq!(ids(hashes, "tag"), ["a1", "a2", "b1"]);
    let quoted = table
        .execute("SELECT * FROM t WHERE (id, sk) IN (('a', 2))")
        .unwrap();
    assert_eq!(ids(quoted, "tag"), ["a2"]);

    let mut hashes_only = hash_only();
    let listed = hashes_only
        .execute("SELECT * FROM h WHERE id IN (b, d)")
        .unwrap();
    assert_eq!(ids(listed, "id"), ["b", "d"]);
}

#[test]
fn whole_keys_are_one_batch_get_and_lone_hash_keys_are_queries() {
    let mut table = hash_and_range();
    let pairs = explain(
        &mut table,
        "SELECT * FROM t WHERE (id, sk) IN ((a, 1), (c, 3))",
    );
    assert!(pairs.contains("batch_get_item t"), "{pairs}");
    assert!(!pairs.contains("scan"), "{pairs}");
    let keys = explain(&mut table, "SELECT * FROM t KEYS IN (a, 1), (b, 1)");
    assert_eq!(keys.matches("batch_get_item").count(), 1, "{keys}");
    let hashes = explain(&mut table, "SELECT * FROM t WHERE id IN (a, b)");
    assert_eq!(hashes.matches("query t").count(), 2, "{hashes}");
    assert!(!hashes.contains("scan"), "{hashes}");

    let mut hashes_only = hash_only();
    let listed = explain(&mut hashes_only, "SELECT * FROM h WHERE id IN (a, b)");
    assert!(listed.contains("batch_get_item h"), "{listed}");
    assert!(!listed.contains("scan"), "{listed}");
}

#[test]
fn where_in_mixed_with_other_conditions_stays_a_filter() {
    let mut table = hash_and_range();
    let filtered = table
        .execute("SCAN * FROM t WHERE (id, sk) IN ((a, 1), (a, 2), (b, 1)) AND tag <> 'a2'")
        .unwrap();
    assert_eq!(ids(filtered, "tag"), ["a1", "b1"]);
    let plan = explain(
        &mut table,
        "SCAN * FROM t WHERE (id, sk) IN ((a, 1), (b, 1))",
    );
    assert!(plan.contains("scan t"), "{plan}");
    // A key list reads like KEYS IN, so LIMIT gets the same error.
    let error = table
        .execute("SELECT * FROM t WHERE id IN (a, b) LIMIT 1")
        .unwrap_err();
    assert!(error.to_string().contains("Cannot use LIMIT"), "{error}");
}

#[test]
fn where_in_parses_lists_and_tuples() {
    let condition = |sql: &str| match parse_statement(sql).unwrap() {
        Statement::Select { condition, .. } => condition.expect("condition"),
        other => panic!("unexpected statement: {other:?}"),
    };
    let s = |value: &str| Value::String(value.to_string());
    let n = |value: &str| Value::Number(value.to_string());
    assert_eq!(
        condition("SELECT * FROM t WHERE id IN (a, 'b')"),
        Condition::In {
            field: "id".into(),
            values: vec![s("a"), s("b")]
        }
    );
    assert_eq!(
        condition("SELECT * FROM t WHERE (id, sk) IN ((a, 1), (b, 2))"),
        Condition::InTuples {
            fields: vec!["id".into(), "sk".into()],
            tuples: vec![vec![s("a"), n("1")], vec![s("b"), n("2")]],
        }
    );
    // A grouped condition still parses as before.
    assert!(matches!(
        condition("SELECT * FROM t WHERE (id = 'a' OR id = 'b')"),
        Condition::Or(_)
    ));
    for (sql, message) in [
        ("SELECT * FROM t WHERE id IN ((a, 1))", "single values"),
        (
            "SELECT * FROM t WHERE (id, sk) IN ((a))",
            "each tuple needs 2 values",
        ),
        ("SELECT * FROM t WHERE (id, sk) IN (a, 1)", "takes tuples"),
    ] {
        let error = parse_statement(sql).unwrap_err();
        assert!(error.to_string().contains(message), "{sql}: {error:?}");
    }
}
