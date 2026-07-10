//! SAVE / LOAD file formats (JSON lines, CSV, gzip, and MessagePack).

use crate::json_util::{json_value_to_item, value_to_json};
use crate::Item;
use dql_parser::Value;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use rmpv::Value as MsgValue;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

/// Optional magic prefix written before MessagePack records for future versioning.
const MSGPACK_MAGIC: &[u8; 4] = b"DQL1";
const SET_TAG: &str = "__dql_set__";
const NUMBER_TAG: &str = "__dql_number__";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Json,
    Csv,
    MsgPack,
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

    if is_pickle_filename(&file_name) {
        return Err(
            "pickle is not supported; use MessagePack (.msgpack) or export JSON/CSV from Python DQL"
                .to_string(),
        );
    }

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
    } else if stem.ends_with(".msgpack") || !stem.contains('.') {
        // `.msgpack` and unknown/default binary extensions replace Python pickle.
        FileFormat::MsgPack
    } else {
        FileFormat::MsgPack
    };

    Ok(FileSpec {
        path: path.to_path_buf(),
        format,
        gzip,
    })
}

fn is_pickle_filename(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    for ext in [".p", ".pkl", ".pickle"] {
        if lower.ends_with(ext)
            || lower.ends_with(&format!("{ext}.gz"))
            || lower.ends_with(&format!("{ext}.gzip"))
        {
            return true;
        }
    }
    false
}

pub fn save_items(path: &Path, items: &[Item]) -> Result<usize, String> {
    let spec = parse_file_spec(path)?;
    match spec.format {
        FileFormat::Json => save_json(&spec, items),
        FileFormat::Csv => save_csv(&spec, items),
        FileFormat::MsgPack => save_msgpack(&spec, items),
    }
}

pub fn load_items(path: &Path) -> Result<Vec<Item>, String> {
    let spec = parse_file_spec(path)?;
    match spec.format {
        FileFormat::Json => load_json(&spec),
        FileFormat::Csv => load_csv(&spec),
        FileFormat::MsgPack => load_msgpack(&spec),
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

fn open_binary_reader(spec: &FileSpec) -> Result<Box<dyn Read>, String> {
    let file = File::open(&spec.path)
        .map_err(|err| format!("failed to open '{}': {err}", spec.path.display()))?;
    if spec.gzip {
        Ok(Box::new(GzDecoder::new(file)))
    } else {
        Ok(Box::new(file))
    }
}

fn save_msgpack(spec: &FileSpec, items: &[Item]) -> Result<usize, String> {
    let mut writer = open_writer(spec)?;
    writer
        .write_all(MSGPACK_MAGIC)
        .map_err(|err| err.to_string())?;
    for item in items {
        let encoded = item_to_msgpack(item);
        rmpv::encode::write_value(&mut writer, &encoded).map_err(|err| err.to_string())?;
    }
    writer.flush().map_err(|err| err.to_string())?;
    Ok(items.len())
}

fn load_msgpack(spec: &FileSpec) -> Result<Vec<Item>, String> {
    let mut reader = open_binary_reader(spec)?;
    let mut peek = [0u8; 4];
    let mut filled = 0usize;
    while filled < 4 {
        match reader.read(&mut peek[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(err) => return Err(err.to_string()),
        }
    }
    let mut rest: Box<dyn Read> = if filled == 4 && &peek == MSGPACK_MAGIC {
        reader
    } else if filled == 0 {
        return Ok(Vec::new());
    } else {
        Box::new(PrefixReader {
            prefix: peek,
            prefix_len: filled,
            prefix_pos: 0,
            inner: reader,
        })
    };

    let mut items = Vec::new();
    loop {
        match rmpv::decode::read_value(&mut rest) {
            Ok(value) => items.push(msgpack_to_item(&value)?),
            Err(rmpv::decode::Error::InvalidMarkerRead(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(rmpv::decode::Error::InvalidDataRead(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(err) => return Err(err.to_string()),
        }
    }
    Ok(items)
}

struct PrefixReader {
    prefix: [u8; 4],
    prefix_len: usize,
    prefix_pos: usize,
    inner: Box<dyn Read>,
}

impl Read for PrefixReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.prefix_pos < self.prefix_len {
            let available = self.prefix_len - self.prefix_pos;
            let take = available.min(buf.len());
            buf[..take].copy_from_slice(&self.prefix[self.prefix_pos..self.prefix_pos + take]);
            self.prefix_pos += take;
            return Ok(take);
        }
        self.inner.read(buf)
    }
}

fn item_to_msgpack(item: &Item) -> MsgValue {
    let map = item
        .iter()
        .map(|(key, value)| {
            (
                MsgValue::String(key.as_str().into()),
                value_to_msgpack(value),
            )
        })
        .collect::<Vec<_>>();
    MsgValue::Map(map)
}

fn value_to_msgpack(value: &Value) -> MsgValue {
    match value {
        Value::Null => MsgValue::Nil,
        Value::Bool(value) => MsgValue::Boolean(*value),
        // Preserve DynamoDB decimal fidelity via an explicit number tag.
        Value::Number(value) => MsgValue::Map(vec![(
            MsgValue::String(NUMBER_TAG.into()),
            MsgValue::String(value.as_str().into()),
        )]),
        Value::String(value) => MsgValue::String(value.as_str().into()),
        Value::Binary(value) => MsgValue::Binary(value.clone()),
        Value::List(values) => MsgValue::Array(values.iter().map(value_to_msgpack).collect()),
        Value::Set(values) => {
            let elements = values.iter().map(value_to_msgpack).collect::<Vec<_>>();
            MsgValue::Map(vec![(
                MsgValue::String(SET_TAG.into()),
                MsgValue::Array(elements),
            )])
        }
        Value::Map(values) => {
            let map = values
                .iter()
                .map(|(key, value)| {
                    (
                        MsgValue::String(key.as_str().into()),
                        value_to_msgpack(value),
                    )
                })
                .collect();
            MsgValue::Map(map)
        }
        Value::Timestamp(expr) => MsgValue::String(format!("{expr:?}").into()),
        Value::Interval(value) => MsgValue::String(value.as_str().into()),
    }
}

fn msgpack_to_item(value: &MsgValue) -> Result<Item, String> {
    let MsgValue::Map(entries) = value else {
        return Err("expected MessagePack map for item".to_string());
    };
    let mut item = Item::new();
    for (key, value) in entries {
        let key = msgpack_key(key)?;
        item.insert(key, msgpack_to_value(value)?);
    }
    Ok(item)
}

fn msgpack_to_value(value: &MsgValue) -> Result<Value, String> {
    match value {
        MsgValue::Nil => Ok(Value::Null),
        MsgValue::Boolean(value) => Ok(Value::Bool(*value)),
        MsgValue::Integer(value) => Ok(Value::Number(value.to_string())),
        MsgValue::F32(value) => Ok(Value::Number(value.to_string())),
        MsgValue::F64(value) => Ok(Value::Number(value.to_string())),
        MsgValue::String(value) => {
            let text = value
                .as_str()
                .ok_or_else(|| "invalid MessagePack string".to_string())?;
            Ok(Value::String(text.to_string()))
        }
        MsgValue::Binary(value) => Ok(Value::Binary(value.clone())),
        MsgValue::Array(values) => values
            .iter()
            .map(msgpack_to_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::List),
        MsgValue::Map(entries) => {
            if entries.len() == 1 {
                if let Some((MsgValue::String(tag), inner)) = entries.first() {
                    match (tag.as_str(), inner) {
                        (Some(NUMBER_TAG), MsgValue::String(number)) => {
                            let text = number
                                .as_str()
                                .ok_or_else(|| "invalid tagged number".to_string())?;
                            return Ok(Value::Number(text.to_string()));
                        }
                        (Some(SET_TAG), MsgValue::Array(values)) => {
                            let elements = values
                                .iter()
                                .map(msgpack_to_value)
                                .collect::<Result<Vec<_>, _>>()?;
                            return Ok(Value::Set(elements));
                        }
                        _ => {}
                    }
                }
            }
            let mut map = BTreeMap::new();
            for (key, value) in entries {
                map.insert(msgpack_key(key)?, msgpack_to_value(value)?);
            }
            Ok(Value::Map(map))
        }
        MsgValue::Ext(..) => Err("unsupported MessagePack extension type".to_string()),
    }
}

fn msgpack_key(value: &MsgValue) -> Result<String, String> {
    match value {
        MsgValue::String(value) => value
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "invalid MessagePack map key".to_string()),
        other => Err(format!("expected string map key, got {other:?}")),
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

    #[test]
    fn round_trip_msgpack_all_value_types() {
        let dir = std::env::temp_dir().join("dql_file_io_msgpack");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut nested = BTreeMap::new();
        nested.insert("k".to_string(), Value::String("v".to_string()));
        let mut item = BTreeMap::new();
        item.insert("s".to_string(), Value::String("hello".to_string()));
        item.insert("n".to_string(), Value::Number("1.2500".to_string()));
        item.insert("b".to_string(), Value::Bool(true));
        item.insert("z".to_string(), Value::Null);
        item.insert("bin".to_string(), Value::Binary(b"abc".to_vec()));
        item.insert(
            "list".to_string(),
            Value::List(vec![Value::Number("1".to_string()), Value::String("x".to_string())]),
        );
        item.insert(
            "set".to_string(),
            Value::Set(vec![Value::String("a".to_string()), Value::String("b".to_string())]),
        );
        item.insert("map".to_string(), Value::Map(nested));

        for name in ["out.msgpack", "out.msgpack.gz", "out.bin"] {
            let path = dir.join(name);
            save_items(&path, &[item.clone()]).unwrap();
            let loaded = load_items(&path).unwrap();
            assert_eq!(loaded, vec![item.clone()], "{name}");
        }
    }

    #[test]
    fn rejects_pickle_extensions() {
        let err = parse_file_spec(Path::new("archive.p")).unwrap_err();
        assert!(err.contains("pickle"), "{err}");
        let err = parse_file_spec(Path::new("archive.pickle.gz")).unwrap_err();
        assert!(err.contains("MessagePack"), "{err}");
    }
}
