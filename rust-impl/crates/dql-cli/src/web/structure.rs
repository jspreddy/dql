//! Column types, roles, and samples for the structure dialog.
//!
//! The rows passed in are the only rows inspected. Callers decide which rows
//! those are: every row of a data file, one already-fetched table page, or one
//! cached query result.

use std::collections::HashSet;

use serde::Serialize;
use serde_json::{json, Value};

use super::shape::{read_query, Column};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StructureColumn {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
    pub kinds: Vec<String>,
    pub present: usize,
    pub total: usize,
    pub sample: Value,
}

pub fn describe(columns: &[Column], rows: &[Value]) -> Vec<StructureColumn> {
    let mut ordered = Vec::new();
    let mut seen = HashSet::new();
    for column in columns {
        if column.name.is_empty() || !seen.insert(column.name.clone()) {
            continue;
        }
        ordered.push(column.clone());
    }
    let mut extra = Vec::new();
    for row in rows {
        let Some(object) = row.as_object() else {
            continue;
        };
        for key in object.keys() {
            if seen.insert(key.clone()) {
                extra.push(key.clone());
            }
        }
    }
    extra.sort_by(|left, right| {
        left.to_lowercase()
            .cmp(&right.to_lowercase())
            .then_with(|| left.cmp(right))
    });
    for name in extra {
        ordered.push(Column {
            name,
            table: None,
            index: None,
        });
    }
    ordered
        .into_iter()
        .map(|column| inspect(column, rows))
        .collect()
}

pub fn payload(
    kind: &str,
    title: &str,
    statement: &str,
    rows: usize,
    paged: bool,
    columns: Vec<StructureColumn>,
) -> Value {
    json!({
        "kind": kind,
        "title": title,
        "statement": statement,
        "rows": rows,
        "paged": paged,
        "columns": columns,
    })
}

pub fn result_title(statement: &str) -> String {
    read_query(statement)
        .0
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Result".to_string())
}

fn inspect(column: Column, rows: &[Value]) -> StructureColumn {
    let total = rows.len();
    let mut present = 0usize;
    let mut kinds = Vec::new();
    let mut sample = Value::Null;
    let mut saw_sample = false;
    for row in rows {
        let Some(object) = row.as_object() else {
            continue;
        };
        let Some(value) = object.get(&column.name) else {
            continue;
        };
        present += 1;
        let kind = value_kind(value);
        if kind != "NULL" && !kinds.iter().any(|item| item == kind) {
            kinds.push(kind.to_string());
        }
        if !saw_sample && !value.is_null() {
            sample = value.clone();
            saw_sample = true;
        }
    }
    if kinds.is_empty() && present > 0 {
        kinds.push("NULL".to_string());
    }
    StructureColumn {
        name: column.name,
        table: column.table,
        index: column.index,
        kinds,
        present,
        total,
        sample,
    }
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "NULL",
        Value::Bool(_) => "BOOL",
        Value::Number(_) => "NUMBER",
        Value::String(_) => "STRING",
        Value::Array(_) => "LIST",
        Value::Object(_) => "MAP",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn column(name: &str, role: Option<&str>) -> Column {
        Column {
            name: name.to_string(),
            table: role.map(str::to_string),
            index: None,
        }
    }

    #[test]
    fn describes_types_samples_and_columns_missing_from_some_rows() {
        let rows = vec![
            json!({"id": "a", "n": 1, "note": null}),
            json!({"id": "b", "n": "2", "extra": true}),
            json!({"id": "c", "flag": {"on": false}}),
        ];
        let described = describe(&[column("id", Some("hash")), column("n", None)], &rows);
        let names: Vec<_> = described
            .iter()
            .map(|column| column.name.as_str())
            .collect();
        assert_eq!(names, ["id", "n", "extra", "flag", "note"]);
        assert_eq!(described[0].table.as_deref(), Some("hash"));
        assert_eq!(described[0].kinds, ["STRING"]);
        assert_eq!(described[0].present, 3);
        assert_eq!(described[0].total, 3);
        assert_eq!(described[0].sample, json!("a"));

        assert_eq!(described[1].kinds, ["NUMBER", "STRING"]);
        assert_eq!(described[1].present, 2);
        assert_eq!(described[1].sample, json!(1));

        assert_eq!(described[2].kinds, ["BOOL"]);
        assert_eq!(described[2].present, 1);
        assert_eq!(described[2].sample, json!(true));

        assert_eq!(described[3].kinds, ["MAP"]);
        assert_eq!(described[3].present, 1);

        assert_eq!(described[4].kinds, ["NULL"]);
        assert_eq!(described[4].present, 1);
        assert_eq!(described[4].sample, Value::Null);
    }
}
