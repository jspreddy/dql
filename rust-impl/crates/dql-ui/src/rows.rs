//! Row sheet titles, matching `rowPopupTitleColumn` on the web page.

use serde_json::{Map, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnRole {
    pub name: String,
    pub role: String,
}

pub fn row_title(row: &Map<String, Value>, columns: &[ColumnRole]) -> String {
    let mut preferred: Vec<&ColumnRole> = Vec::new();
    if let Some(hash) = columns.iter().find(|column| column.role == "hash") {
        preferred.push(hash);
    }
    for column in columns {
        if preferred
            .first()
            .is_some_and(|first| first.name == column.name)
        {
            continue;
        }
        preferred.push(column);
    }
    for column in preferred {
        if let Some(Value::String(value)) = row.get(&column.name) {
            if !value.is_empty() && value.chars().count() <= 48 {
                return format!("{} = {value}", column.name);
            }
        }
    }
    "Row".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_hash_column_names_the_row() {
        let row = json!({"name": "Ada", "id": "a"})
            .as_object()
            .unwrap()
            .clone();
        let columns = vec![
            ColumnRole {
                name: "name".into(),
                role: String::new(),
            },
            ColumnRole {
                name: "id".into(),
                role: "hash".into(),
            },
        ];
        assert_eq!(row_title(&row, &columns), "id = a");
    }
}
