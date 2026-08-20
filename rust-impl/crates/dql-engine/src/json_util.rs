use crate::Item;
use dql_parser::Value;
use std::collections::BTreeMap;

pub fn item_to_json(item: &Item, indent: usize) -> String {
    let mut output = String::from("{\n");
    let len = item.len();
    for (index, (key, value)) in item.iter().enumerate() {
        output.push_str(&" ".repeat(indent + 4));
        output.push_str(&string_to_json(key));
        output.push_str(": ");
        output.push_str(&value_to_json(value, indent + 4));
        if index + 1 != len {
            output.push(',');
        }
        output.push('\n');
    }
    output.push_str(&" ".repeat(indent));
    output.push('}');
    output
}

pub fn value_to_json(value: &Value, indent: usize) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.clone(),
        Value::String(value) => string_to_json(value),
        Value::Binary(value) => string_to_json(&base64(value)),
        Value::Timestamp(value) => string_to_json(&format!("{value:?}")),
        Value::Interval(value) => string_to_json(value),
        Value::List(values) | Value::Set(values) => {
            if values.is_empty() {
                return "[]".to_string();
            }
            let mut output = String::from("[\n");
            for (index, value) in values.iter().enumerate() {
                output.push_str(&" ".repeat(indent + 4));
                output.push_str(&value_to_json(value, indent + 4));
                if index + 1 != values.len() {
                    output.push(',');
                }
                output.push('\n');
            }
            output.push_str(&" ".repeat(indent));
            output.push(']');
            output
        }
        Value::Map(values) => {
            let mut item = Item::new();
            for (key, value) in values {
                item.insert(key.clone(), value.clone());
            }
            item_to_json(&item, indent)
        }
    }
}

pub fn string_to_json(value: &str) -> String {
    let mut output = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c => output.push(c),
        }
    }
    output.push('"');
    output
}

fn base64(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        output.push(ALPHABET[(b0 >> 2) as usize] as char);
        output.push(ALPHABET[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(ALPHABET[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(ALPHABET[(b2 & 0b0011_1111) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

pub fn json_value_to_item(value: &serde_json::Value) -> Result<Item, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "expected JSON object".to_string())?;
    object
        .iter()
        .map(|(key, value)| json_value_to_dql_value(value).map(|parsed| (key.clone(), parsed)))
        .collect()
}

fn json_value_to_dql_value(value: &serde_json::Value) -> Result<Value, String> {
    match value {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(value) => Ok(Value::Bool(*value)),
        serde_json::Value::Number(value) => Ok(Value::Number(value.to_string())),
        serde_json::Value::String(value) => Ok(Value::String(value.clone())),
        serde_json::Value::Array(values) => values
            .iter()
            .map(json_value_to_dql_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::List),
        serde_json::Value::Object(values) => values
            .iter()
            .map(|(key, value)| json_value_to_dql_value(value).map(|parsed| (key.clone(), parsed)))
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(Value::Map),
    }
}
