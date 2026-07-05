use dql_parser::{
    parse_script, Attribute, AttributeType, CompareOp, Condition, KeyType, ParseError,
    QueryOptions, Statement, Value,
};
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fmt;

pub type Item = BTreeMap<String, Value>;

pub fn format_throughput(available: Option<f64>, used: Option<f64>) -> String {
    match (available, used) {
        (Some(0.0), Some(used)) | (None, Some(used)) => format!("{used:.0}/∞"),
        (Some(0.0), None) | (None, None) => "N/A".to_string(),
        (Some(available), None) => format!("{available:.0}"),
        (Some(available), Some(used)) => {
            let percent = used / available * 100.0;
            format!("{used:.0}/{available:.0} ({percent:.0}%)")
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatementResult {
    None,
    Status(String),
    Affected(usize),
    Items(Vec<Item>),
    Schema(String),
}

impl StatementResult {
    pub fn to_json_lines(&self) -> String {
        match self {
            StatementResult::Items(items) => {
                let mut output = String::new();
                for item in items {
                    output.push_str(&item_to_json(item, 0));
                    output.push('\n');
                }
                output
            }
            StatementResult::Affected(count) => format!("{count}\n"),
            StatementResult::Schema(schema) => string_to_json(schema),
            StatementResult::Status(_) | StatementResult::None => String::new(),
        }
    }
}

impl fmt::Display for StatementResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatementResult::None => Ok(()),
            StatementResult::Status(status) => writeln!(f, "{status}"),
            StatementResult::Affected(count) => writeln!(f, "{count} item(s) affected"),
            StatementResult::Items(items) => {
                for item in items {
                    writeln!(f, "{}", item_to_json(item, 0))?;
                }
                Ok(())
            }
            StatementResult::Schema(schema) => writeln!(f, "{schema}"),
        }
    }
}

#[derive(Debug)]
pub enum EngineError {
    Parse(ParseError),
    Runtime(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Parse(err) => write!(f, "{err}"),
            EngineError::Runtime(message) => write!(f, "{message}"),
        }
    }
}

impl Error for EngineError {}

impl From<ParseError> for EngineError {
    fn from(value: ParseError) -> Self {
        EngineError::Parse(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableSchema {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub hash_key: String,
    pub range_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct TableData {
    schema: TableSchema,
    items: Vec<Item>,
}

#[derive(Debug, Default)]
pub struct InMemoryEngine {
    tables: HashMap<String, TableData>,
    explain: bool,
    explain_log: Vec<String>,
}

impl InMemoryEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn execute(&mut self, input: &str) -> Result<StatementResult, EngineError> {
        let statements = parse_script(input)?;
        let mut last = StatementResult::None;
        for statement in &statements {
            last = self.run(statement)?;
        }
        Ok(last)
    }

    pub fn run(&mut self, statement: &Statement) -> Result<StatementResult, EngineError> {
        match statement {
            Statement::CreateTable {
                if_not_exists,
                name,
                attributes,
                ..
            } => self.create_table(*if_not_exists, name, attributes),
            Statement::DropTable { if_exists, name } => self.drop_table(*if_exists, name),
            Statement::Insert {
                table,
                columns,
                rows,
            } => self.insert(table, columns, rows),
            Statement::Scan {
                table,
                condition,
                options,
            } => self.scan(table, condition.as_ref(), options),
            Statement::Select {
                table,
                condition,
                options,
            } => self.select(table, condition.as_ref(), options),
            Statement::DumpSchema { tables } => self.dump_schema(tables.as_deref()),
            Statement::Explain(inner) => self.explain(inner),
            Statement::Analyze(inner) => self.run(inner),
        }
    }

    pub fn table_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.tables.keys().cloned().collect();
        names.sort();
        names
    }

    fn create_table(
        &mut self,
        if_not_exists: bool,
        name: &str,
        attributes: &[Attribute],
    ) -> Result<StatementResult, EngineError> {
        self.record("create_table", name);
        if self.tables.contains_key(name) {
            if if_not_exists {
                return Ok(StatementResult::Status(format!(
                    "Table '{name}' already exists"
                )));
            }
            return Err(EngineError::Runtime(format!(
                "Table '{name}' already exists"
            )));
        }
        let hash_key = attributes
            .iter()
            .find(|attribute| attribute.key_type == Some(KeyType::Hash))
            .map(|attribute| attribute.name.clone())
            .ok_or_else(|| EngineError::Runtime("CREATE TABLE requires a hash key".to_string()))?;
        let range_key = attributes
            .iter()
            .find(|attribute| attribute.key_type == Some(KeyType::Range))
            .map(|attribute| attribute.name.clone());
        let schema = TableSchema {
            name: name.to_string(),
            attributes: attributes.to_vec(),
            hash_key,
            range_key,
        };
        self.tables.insert(
            name.to_string(),
            TableData {
                schema,
                items: Vec::new(),
            },
        );
        Ok(StatementResult::Status(format!("Created table '{name}'")))
    }

    fn drop_table(&mut self, if_exists: bool, name: &str) -> Result<StatementResult, EngineError> {
        self.record("delete_table", name);
        if self.tables.remove(name).is_some() {
            Ok(StatementResult::Status(format!("Dropped table '{name}'")))
        } else if if_exists {
            Ok(StatementResult::Status(format!(
                "Table '{name}' did not exist"
            )))
        } else {
            Err(EngineError::Runtime(format!("Table '{name}' not found")))
        }
    }

    fn insert(
        &mut self,
        table: &str,
        columns: &[String],
        rows: &[Vec<Value>],
    ) -> Result<StatementResult, EngineError> {
        self.record("batch_write_item", table);
        let table_data = self
            .tables
            .get_mut(table)
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        for row in rows {
            if row.len() != columns.len() {
                return Err(EngineError::Runtime(format!(
                    "Values '{row:?}' do not match attributes '{columns:?}'"
                )));
            }
            let mut item = Item::new();
            for (column, value) in columns.iter().zip(row.iter()) {
                item.insert(column.clone(), value.clone());
            }
            table_data.items.push(item);
        }
        Ok(StatementResult::Affected(rows.len()))
    }

    fn scan(
        &mut self,
        table: &str,
        condition: Option<&Condition>,
        options: &QueryOptions,
    ) -> Result<StatementResult, EngineError> {
        self.record("scan", table);
        let table_data = self
            .tables
            .get(table)
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let items = apply_read_options(table_data.items.iter(), condition, options);
        Ok(StatementResult::Items(items))
    }

    fn select(
        &mut self,
        table: &str,
        condition: Option<&Condition>,
        options: &QueryOptions,
    ) -> Result<StatementResult, EngineError> {
        self.record("query", table);
        let table_data = self
            .tables
            .get(table)
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let items = apply_read_options(table_data.items.iter(), condition, options);
        Ok(StatementResult::Items(items))
    }

    fn dump_schema(&mut self, tables: Option<&[String]>) -> Result<StatementResult, EngineError> {
        self.record("dump_schema", "*");
        let names = match tables {
            Some(names) => names.to_vec(),
            None => self.table_names(),
        };
        let mut lines = Vec::new();
        for name in names {
            let table = self
                .tables
                .get(&name)
                .ok_or_else(|| EngineError::Runtime(format!("Table '{name}' not found")))?;
            lines.push(schema_to_dql(&table.schema));
        }
        Ok(StatementResult::Schema(lines.join("\n")))
    }

    fn explain(&mut self, inner: &Statement) -> Result<StatementResult, EngineError> {
        let previous = self.explain;
        let previous_log = std::mem::take(&mut self.explain_log);
        self.explain = true;
        let result = self.run(inner);
        self.explain = previous;
        let log = std::mem::replace(&mut self.explain_log, previous_log).join("\n");
        if log.is_empty() {
            result.map(|_| StatementResult::Schema(log))
        } else {
            Ok(StatementResult::Schema(log))
        }
    }

    fn record(&mut self, operation: &str, target: &str) {
        if self.explain {
            self.explain_log.push(format!("{operation} {target}"));
        }
    }
}

fn matches_condition(item: &Item, condition: &Condition) -> bool {
    match condition {
        Condition::Compare { field, op, value } => item
            .get(field)
            .is_some_and(|item_value| compare_values(item_value, op, value)),
        Condition::And(conditions) => conditions
            .iter()
            .all(|condition| matches_condition(item, condition)),
        Condition::Or(conditions) => conditions
            .iter()
            .any(|condition| matches_condition(item, condition)),
        Condition::Not(condition) => !matches_condition(item, condition),
    }
}

fn apply_read_options<'a>(
    items: impl Iterator<Item = &'a Item>,
    condition: Option<&Condition>,
    options: &QueryOptions,
) -> Vec<Item> {
    let scanned = items.take(options.scan_limit.unwrap_or(usize::MAX));
    let filtered =
        scanned.filter(|item| condition.is_none_or(|condition| matches_condition(item, condition)));
    filtered
        .take(options.limit.unwrap_or(usize::MAX))
        .cloned()
        .collect()
}

fn compare_values(left: &Value, op: &CompareOp, right: &Value) -> bool {
    match op {
        CompareOp::Eq => left == right,
        CompareOp::Ne => left != right,
        CompareOp::Lt => compare_order(left, right).is_some_and(|value| value.is_lt()),
        CompareOp::Le => compare_order(left, right).is_some_and(|value| !value.is_gt()),
        CompareOp::Gt => compare_order(left, right).is_some_and(|value| value.is_gt()),
        CompareOp::Ge => compare_order(left, right).is_some_and(|value| !value.is_lt()),
    }
}

fn compare_order(left: &Value, right: &Value) -> Option<std::cmp::Ordering> {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => {
            let left = left.parse::<f64>().ok()?;
            let right = right.parse::<f64>().ok()?;
            left.partial_cmp(&right)
        }
        (Value::String(left), Value::String(right)) => Some(left.cmp(right)),
        _ => None,
    }
}

fn schema_to_dql(schema: &TableSchema) -> String {
    let attrs = schema
        .attributes
        .iter()
        .map(attribute_to_dql)
        .collect::<Vec<_>>()
        .join(", ");
    format!("CREATE TABLE {} ({})", schema.name, attrs)
}

fn attribute_to_dql(attribute: &Attribute) -> String {
    let mut parts = vec![
        attribute.name.clone(),
        match &attribute.attr_type {
            AttributeType::String => "STRING".to_string(),
            AttributeType::Number => "NUMBER".to_string(),
            AttributeType::Binary => "BINARY".to_string(),
            AttributeType::Bool => "BOOL".to_string(),
            AttributeType::Other(value) => value.to_ascii_uppercase(),
        },
    ];
    match attribute.key_type {
        Some(KeyType::Hash) => {
            parts.push("HASH".to_string());
            parts.push("KEY".to_string());
        }
        Some(KeyType::Range) => {
            parts.push("RANGE".to_string());
            parts.push("KEY".to_string());
        }
        None => {}
    }
    parts.join(" ")
}

fn item_to_json(item: &Item, indent: usize) -> String {
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

fn value_to_json(value: &Value, indent: usize) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.clone(),
        Value::String(value) => string_to_json(value),
        Value::Binary(value) => string_to_json(&base64(value)),
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

fn string_to_json(value: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executes_create_insert_scan_script() {
        let mut engine = InMemoryEngine::new();
        let result = engine
            .execute(
                "CREATE TABLE t (id STRING HASH KEY);
                 INSERT INTO t (id, score) VALUES ('a', 1), ('b', 2);
                 SCAN * FROM t",
            )
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].get("id"), Some(&Value::String("a".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn select_filters_conditions() {
        let mut engine = InMemoryEngine::new();
        let result = engine
            .execute(
                "CREATE TABLE t (id STRING HASH KEY, score NUMBER);
                 INSERT INTO t (id, score) VALUES ('a', 1), ('b', 2);
                 SELECT * FROM t WHERE id = 'b' AND score >= 2",
            )
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
    fn dump_schema_returns_create_statement() {
        let mut engine = InMemoryEngine::new();
        let result = engine
            .execute("CREATE TABLE t (id STRING HASH KEY, ts NUMBER RANGE KEY); DUMP SCHEMA")
            .unwrap();
        assert_eq!(
            result,
            StatementResult::Schema(
                "CREATE TABLE t (id STRING HASH KEY, ts NUMBER RANGE KEY)".to_string()
            )
        );
    }

    #[test]
    fn explain_records_backend_operations() {
        let mut engine = InMemoryEngine::new();
        let result = engine
            .execute("EXPLAIN CREATE TABLE t (id STRING HASH KEY)")
            .unwrap();
        assert_eq!(
            result,
            StatementResult::Schema("create_table t".to_string())
        );
    }

    #[test]
    fn json_lines_pretty_print_items() {
        let mut item = Item::new();
        item.insert("bin".to_string(), Value::Binary(b"a".to_vec()));
        item.insert("id".to_string(), Value::String("x".to_string()));
        let result = StatementResult::Items(vec![item]);
        assert_eq!(
            result.to_json_lines(),
            "{\n    \"bin\": \"YQ==\",\n    \"id\": \"x\"\n}\n"
        );
    }
}
