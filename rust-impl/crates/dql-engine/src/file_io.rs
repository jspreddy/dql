//! SAVE / LOAD file formats (JSON lines, CSV, and gzip wrappers).

use crate::json_util::{json_value_to_item, value_to_json};
use crate::Item;
use dql_parser::Value;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Json,
    Csv,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSpec {
    pub path: PathBuf,
    pub format: FileFormat,
    pub gzip: bool,
}

pub fn parse_file_spec(path: &Path) -> Result<FileSpec, String> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("invalid file path '{}'", path.display()))?
        .to_ascii_lowercase();

    let mut gzip = false;
    let mut stem = file_name.as_str();
    if let Some(rest) = stem
        .strip_suffix(".gz")
        .or_else(|| stem.strip_suffix(".gzip"))
    {
        gzip = true;
        stem = rest;
    }

    let format = if stem.ends_with(".json") {
        FileFormat::Json
    } else if stem.ends_with(".csv") {
        FileFormat::Csv
    } else {
        return Err(format!(
            "unsupported LOAD/SAVE file format for '{}'",
            path.display()
        ));
    };

    Ok(FileSpec {
        path: path.to_path_buf(),
        format,
        gzip,
    })
}

pub fn save_items(path: &Path, items: &[Item]) -> Result<usize, String> {
    let spec = parse_file_spec(path)?;
    match spec.format {
        FileFormat::Json => save_json(&spec, items),
        FileFormat::Csv => save_csv(&spec, items),
    }
}

pub fn load_items(path: &Path) -> Result<Vec<Item>, String> {
    let spec = parse_file_spec(path)?;
    match spec.format {
        FileFormat::Json => load_json(&spec),
        FileFormat::Csv => load_csv(&spec),
    }
}

fn save_json(spec: &FileSpec, items: &[Item]) -> Result<usize, String> {
    let mut writer = open_writer(spec)?;
    for item in items {
        let line = item_to_json_line(item);
        writer
            .write_all(line.as_bytes())
            .and_then(|_| writer.write_all(b"\n"))
            .map_err(|err| err.to_string())?;
    }
    writer.flush().map_err(|err| err.to_string())?;
    Ok(items.len())
}

fn load_json(spec: &FileSpec) -> Result<Vec<Item>, String> {
    let reader = open_text_reader(spec)?;
    let mut items = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|err| err.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(&line).map_err(|err| err.to_string())?;
        items.push(json_value_to_item(&value)?);
    }
    Ok(items)
}

fn save_csv(spec: &FileSpec, items: &[Item]) -> Result<usize, String> {
    let mut headers = BTreeSet::new();
    for item in items {
        headers.extend(item.keys().cloned());
    }
    let headers: Vec<String> = headers.into_iter().collect();
    let mut writer = open_writer(spec)?;
    writeln!(writer, "{}", headers.join(",")).map_err(|err| err.to_string())?;
    for item in items {
        let row = headers
            .iter()
            .map(|header| match item.get(header) {
                Some(value) => csv_escape(&value_to_csv_field(value)),
                None => String::new(),
            })
            .collect::<Vec<_>>()
            .join(",");
        writeln!(writer, "{row}").map_err(|err| err.to_string())?;
    }
    writer.flush().map_err(|err| err.to_string())?;
    Ok(items.len())
}

fn load_csv(spec: &FileSpec) -> Result<Vec<Item>, String> {
    let reader = open_text_reader(spec)?;
    let mut lines = reader.lines();
    let header = lines
        .next()
        .transpose()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "CSV file is missing a header row".to_string())?;
    let headers = split_csv_line(&header);
    let mut items = Vec::new();
    for line in lines {
        let line = line.map_err(|err| err.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let fields = split_csv_line(&line);
        let mut item = Item::new();
        for (column, value) in headers.iter().zip(fields.iter()) {
            item.insert(column.clone(), csv_field_to_value(value));
        }
        items.push(item);
    }
    Ok(items)
}

fn open_writer(spec: &FileSpec) -> Result<Box<dyn Write>, String> {
    let file = File::create(&spec.path)
        .map_err(|err| format!("failed to create '{}': {err}", spec.path.display()))?;
    if spec.gzip {
        Ok(Box::new(GzEncoder::new(file, Compression::default())))
    } else {
        Ok(Box::new(file))
    }
}

fn open_text_reader(spec: &FileSpec) -> Result<Box<dyn BufRead>, String> {
    let file = File::open(&spec.path)
        .map_err(|err| format!("failed to open '{}': {err}", spec.path.display()))?;
    if spec.gzip {
        Ok(Box::new(BufReader::new(GzDecoder::new(file))))
    } else {
        Ok(Box::new(BufReader::new(file)))
    }
}

fn item_to_json_line(item: &Item) -> String {
    let mut output = String::from("{");
    for (index, (key, value)) in item.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&serde_json::to_string(key).unwrap_or_else(|_| format!("\"{key}\"")));
        output.push(':');
        output.push_str(&compact_value_json(value));
    }
    output.push('}');
    output
}

fn compact_value_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.clone(),
        Value::String(value) => {
            serde_json::to_string(value).unwrap_or_else(|_| format!("\"{value}\""))
        }
        Value::Binary(value) => value_to_json(&Value::Binary(value.clone()), 0),
        Value::List(values) | Value::Set(values) => {
            let mut output = String::from("[");
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&compact_value_json(value));
            }
            output.push(']');
            output
        }
        Value::Map(values) => {
            let mut item = Item::new();
            for (key, value) in values {
                item.insert(key.clone(), value.clone());
            }
            item_to_json_line(&item)
        }
        Value::Timestamp(expr) => format!("\"{expr:?}\""),
        Value::Interval(value) => {
            serde_json::to_string(value).unwrap_or_else(|_| format!("\"{value}\""))
        }
    }
}

fn value_to_csv_field(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) | Value::String(value) | Value::Interval(value) => value.clone(),
        Value::Binary(value) => value_to_json(&Value::Binary(value.clone()), 0)
            .trim_matches('"')
            .to_string(),
        Value::Timestamp(expr) => format!("{expr:?}"),
        Value::List(_) | Value::Set(_) | Value::Map(_) => compact_value_json(value),
    }
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn split_csv_line(line: &str) -> Vec<String> {
    line.split(',')
        .map(str::trim)
        .map(str::to_string)
        .collect()
}

fn csv_field_to_value(field: &str) -> Value {
    if field.parse::<f64>().is_ok() {
        Value::Number(field.to_string())
    } else {
        Value::String(field.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn sample_items() -> Vec<Item> {
        let mut a = BTreeMap::new();
        a.insert("id".to_string(), Value::String("a".to_string()));
        a.insert("foo".to_string(), Value::Number("1".to_string()));
        let mut b = BTreeMap::new();
        b.insert("id".to_string(), Value::String("b".to_string()));
        b.insert("foo".to_string(), Value::Number("2".to_string()));
        vec![a, b]
    }

    #[test]
    fn round_trip_json_and_csv_with_gzip() {
        let dir = std::env::temp_dir().join("dql_file_io_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let items = sample_items();

        for name in ["out.json", "out.json.gz", "out.csv", "out.csv.gz"] {
            let path = dir.join(name);
            let count = save_items(&path, &items).unwrap();
            assert_eq!(count, 2, "{name}");
            let loaded = load_items(&path).unwrap();
            assert_eq!(loaded.len(), 2, "{name}");
            assert_eq!(loaded[0].get("id"), Some(&Value::String("a".to_string())));
            assert_eq!(loaded[0].get("foo"), Some(&Value::Number("1".to_string())));
        }
    }
}
