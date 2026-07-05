use dql_expr::{render_condition, render_projection};
use dql_models::TableMeta;
use dql_models::{plan_read, Operation, PlanInput, ReadKind};
use dql_parser::{
    parse_script, AttributeType, CompareOp, Condition, ConditionOperand, KeyType, ParseError,
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

#[derive(Debug, Clone, PartialEq)]
pub struct BackendCall {
    pub operation: String,
    pub table: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapacityRecord {
    pub operation: String,
    pub table: String,
    pub read_units: f64,
    pub write_units: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BackendResponse<T> {
    pub output: T,
    pub capacity: Option<CapacityRecord>,
}

impl<T> BackendResponse<T> {
    fn new(operation: &str, table: &str, output: T) -> Self {
        Self {
            output,
            capacity: Some(CapacityRecord {
                operation: operation.to_string(),
                table: table.to_string(),
                read_units: 0.0,
                write_units: 0.0,
            }),
        }
    }
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
struct TableData {
    meta: TableMeta,
    items: Vec<Item>,
}

pub trait DynamoBackend {
    fn list_tables(&self) -> Vec<String>;
    fn describe_table(&self, table: &str) -> Option<TableMeta>;
    fn create_table(
        &mut self,
        meta: TableMeta,
        if_not_exists: bool,
    ) -> Result<BackendResponse<String>, EngineError>;
    fn delete_table(
        &mut self,
        table: &str,
        if_exists: bool,
    ) -> Result<BackendResponse<String>, EngineError>;
    fn batch_write(
        &mut self,
        table: &str,
        items: Vec<Item>,
    ) -> Result<BackendResponse<usize>, EngineError>;
    fn read(
        &self,
        operation: ReadOperation,
        table: &str,
        condition: Option<&Condition>,
        options: &QueryOptions,
    ) -> Result<BackendResponse<Vec<Item>>, EngineError>;
    fn delete_matching(
        &mut self,
        table: &str,
        condition: Option<&Condition>,
    ) -> Result<BackendResponse<usize>, EngineError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOperation {
    Query,
    Scan,
}

#[derive(Debug, Default)]
pub struct MemoryBackend {
    tables: HashMap<String, TableData>,
}

impl DynamoBackend for MemoryBackend {
    fn list_tables(&self) -> Vec<String> {
        let mut names: Vec<_> = self.tables.keys().cloned().collect();
        names.sort();
        names
    }

    fn describe_table(&self, table: &str) -> Option<TableMeta> {
        self.tables.get(table).map(|table| table.meta.clone())
    }

    fn create_table(
        &mut self,
        meta: TableMeta,
        if_not_exists: bool,
    ) -> Result<BackendResponse<String>, EngineError> {
        let name = meta.name.clone();
        if self.tables.contains_key(&name) {
            if if_not_exists {
                return Ok(BackendResponse::new(
                    "create_table",
                    &name,
                    format!("Table '{name}' already exists"),
                ));
            }
            return Err(EngineError::Runtime(format!(
                "Table '{name}' already exists"
            )));
        }
        self.tables.insert(
            name.clone(),
            TableData {
                meta,
                items: Vec::new(),
            },
        );
        Ok(BackendResponse::new(
            "create_table",
            &name,
            format!("Created table '{name}'"),
        ))
    }

    fn delete_table(
        &mut self,
        table: &str,
        if_exists: bool,
    ) -> Result<BackendResponse<String>, EngineError> {
        let output = if self.tables.remove(table).is_some() {
            format!("Dropped table '{table}'")
        } else if if_exists {
            format!("Table '{table}' did not exist")
        } else {
            return Err(EngineError::Runtime(format!("Table '{table}' not found")));
        };
        Ok(BackendResponse::new("delete_table", table, output))
    }

    fn batch_write(
        &mut self,
        table: &str,
        items: Vec<Item>,
    ) -> Result<BackendResponse<usize>, EngineError> {
        let table_data = self
            .tables
            .get_mut(table)
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let count = items.len();
        table_data.items.extend(items);
        Ok(BackendResponse::new("batch_write_item", table, count))
    }

    fn read(
        &self,
        operation: ReadOperation,
        table: &str,
        condition: Option<&Condition>,
        options: &QueryOptions,
    ) -> Result<BackendResponse<Vec<Item>>, EngineError> {
        let table_data = self
            .tables
            .get(table)
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let items = apply_read_options(table_data.items.iter(), condition, options);
        let op_name = match operation {
            ReadOperation::Query => "query",
            ReadOperation::Scan => "scan",
        };
        Ok(BackendResponse::new(op_name, table, items))
    }

    fn delete_matching(
        &mut self,
        table: &str,
        condition: Option<&Condition>,
    ) -> Result<BackendResponse<usize>, EngineError> {
        let table_data = self
            .tables
            .get_mut(table)
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let before = table_data.items.len();
        table_data
            .items
            .retain(|item| !condition.is_none_or(|condition| matches_condition(item, condition)));
        Ok(BackendResponse::new(
            "delete_item",
            table,
            before - table_data.items.len(),
        ))
    }
}

#[derive(Debug, Default)]
pub struct InMemoryEngine {
    backend: MemoryBackend,
    explain: bool,
    explain_log: Vec<String>,
    analyzing: bool,
    consumed_capacities: Vec<CapacityRecord>,
}

impl InMemoryEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn execute(&mut self, input: &str) -> Result<StatementResult, EngineError> {
        let statements = parse_script(input)?;
        let mut last = StatementResult::None;
        self.consumed_capacities.clear();
        for statement in &statements {
            last = self.run(statement)?;
        }
        Ok(last)
    }

    pub fn run(&mut self, statement: &Statement) -> Result<StatementResult, EngineError> {
        match statement {
            Statement::CreateTable { if_not_exists, .. } => {
                self.create_table(statement, *if_not_exists)
            }
            Statement::DropTable { if_exists, name } => self.drop_table(*if_exists, name),
            Statement::Insert {
                table,
                columns,
                rows,
            } => self.insert(table, columns, rows),
            Statement::Delete { table, condition } => self.delete(table, condition.as_ref()),
            Statement::Update { .. } => Err(EngineError::Runtime(
                "UPDATE execution is not implemented in the in-memory scaffold".to_string(),
            )),
            Statement::Scan {
                table,
                selection,
                condition,
                options,
                ..
            } => self.scan(table, selection, condition.as_ref(), options),
            Statement::Select {
                table,
                selection,
                condition,
                options,
                ..
            } => self.select(table, selection, condition.as_ref(), options),
            Statement::AlterTable { .. } => Err(EngineError::Runtime(
                "ALTER execution is not implemented in the in-memory scaffold".to_string(),
            )),
            Statement::DumpSchema { tables } => self.dump_schema(tables.as_deref()),
            Statement::Load { .. } => Err(EngineError::Runtime(
                "LOAD execution is not implemented in the in-memory scaffold".to_string(),
            )),
            Statement::Explain(inner) => self.explain(inner),
            Statement::Analyze(inner) => self.analyze(inner),
        }
    }

    pub fn table_names(&self) -> Vec<String> {
        self.backend.list_tables()
    }

    pub fn consumed_capacities(&self) -> &[CapacityRecord] {
        &self.consumed_capacities
    }

    fn create_table(
        &mut self,
        statement: &Statement,
        if_not_exists: bool,
    ) -> Result<StatementResult, EngineError> {
        let meta = TableMeta::from_create_statement(statement)
            .map_err(|err| EngineError::Runtime(err.to_string()))?;
        self.record("create_table", &meta.name);
        let response = self.backend.create_table(meta, if_not_exists)?;
        self.capture_capacity(response.capacity);
        Ok(StatementResult::Status(response.output))
    }

    fn drop_table(&mut self, if_exists: bool, name: &str) -> Result<StatementResult, EngineError> {
        self.record("delete_table", name);
        let response = self.backend.delete_table(name, if_exists)?;
        self.capture_capacity(response.capacity);
        Ok(StatementResult::Status(response.output))
    }

    fn insert(
        &mut self,
        table: &str,
        columns: &[String],
        rows: &[Vec<Value>],
    ) -> Result<StatementResult, EngineError> {
        self.record("batch_write_item", table);
        let mut items = Vec::new();
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
            items.push(item);
        }
        let response = self.backend.batch_write(table, items)?;
        self.capture_capacity(response.capacity);
        Ok(StatementResult::Affected(response.output))
    }

    fn delete(
        &mut self,
        table: &str,
        condition: Option<&Condition>,
    ) -> Result<StatementResult, EngineError> {
        if condition.is_some() {
            self.record("query", table);
        } else {
            self.record("scan", table);
        }
        self.record("delete_item", table);
        let response = self.backend.delete_matching(table, condition)?;
        self.capture_capacity(response.capacity);
        Ok(StatementResult::Affected(response.output))
    }

    fn scan(
        &mut self,
        table: &str,
        selection: &dql_parser::Selection,
        condition: Option<&Condition>,
        options: &QueryOptions,
    ) -> Result<StatementResult, EngineError> {
        let operation =
            self.plan_read_operation(table, ReadKind::Scan, selection, condition, true)?;
        self.record_read_operation(operation, table);
        let response =
            self.backend
                .read(operation_to_backend(operation), table, condition, options)?;
        self.capture_capacity(response.capacity);
        Ok(StatementResult::Items(response.output))
    }

    fn select(
        &mut self,
        table: &str,
        selection: &dql_parser::Selection,
        condition: Option<&Condition>,
        options: &QueryOptions,
    ) -> Result<StatementResult, EngineError> {
        let operation =
            self.plan_read_operation(table, ReadKind::Select, selection, condition, true)?;
        self.record_read_operation(operation, table);
        let response =
            self.backend
                .read(operation_to_backend(operation), table, condition, options)?;
        self.capture_capacity(response.capacity);
        Ok(StatementResult::Items(response.output))
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
                .backend
                .describe_table(&name)
                .ok_or_else(|| EngineError::Runtime(format!("Table '{name}' not found")))?;
            lines.push(schema_to_dql(&table));
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

    fn record_read_operation(&mut self, operation: Operation, table: &str) {
        let operation = match operation {
            Operation::Query => "query",
            Operation::Scan => "scan",
            Operation::BatchGetKeys => "batch_get_item",
        };
        self.record(operation, table);
    }

    fn plan_read_operation(
        &self,
        table: &str,
        kind: ReadKind,
        selection: &dql_parser::Selection,
        condition: Option<&Condition>,
        allow_select_scan: bool,
    ) -> Result<Operation, EngineError> {
        let meta = self
            .backend
            .describe_table(table)
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        if let Some(condition) = condition {
            let _ =
                render_condition(condition).map_err(|err| EngineError::Runtime(err.to_string()))?;
        }
        let _ = render_projection(selection);
        let plan = plan_read(PlanInput {
            table: &meta,
            kind,
            condition,
            selection: Some(selection),
            using_index: None,
            allow_select_scan,
        })
        .map_err(|err| EngineError::Runtime(format!("query planning failed: {err:?}")))?;
        Ok(plan.operation)
    }

    fn analyze(&mut self, inner: &Statement) -> Result<StatementResult, EngineError> {
        let previous = self.analyzing;
        self.analyzing = true;
        let result = self.run(inner);
        self.analyzing = previous;
        result
    }

    fn capture_capacity(&mut self, capacity: Option<CapacityRecord>) {
        if self.analyzing {
            if let Some(capacity) = capacity {
                self.consumed_capacities.push(capacity);
            }
        }
    }
}

fn matches_condition(item: &Item, condition: &Condition) -> bool {
    match condition {
        Condition::Compare { field, op, rhs } => item
            .get(field)
            .is_some_and(|item_value| compare_operand(item, item_value, op, rhs)),
        Condition::Between { field, low, high } => item.get(field).is_some_and(|item_value| {
            compare_values(item_value, &CompareOp::Ge, low)
                && compare_values(item_value, &CompareOp::Le, high)
        }),
        Condition::In { field, values } => item
            .get(field)
            .is_some_and(|item_value| values.iter().any(|value| item_value == value)),
        Condition::Function { .. } | Condition::Size { .. } | Condition::AttributeType { .. } => {
            false
        }
        Condition::And(conditions) => conditions
            .iter()
            .all(|condition| matches_condition(item, condition)),
        Condition::Or(conditions) => conditions
            .iter()
            .any(|condition| matches_condition(item, condition)),
        Condition::Not(condition) => !matches_condition(item, condition),
    }
}

fn operation_to_backend(operation: Operation) -> ReadOperation {
    match operation {
        Operation::Query | Operation::BatchGetKeys => ReadOperation::Query,
        Operation::Scan => ReadOperation::Scan,
    }
}

fn compare_operand(item: &Item, left: &Value, op: &CompareOp, rhs: &ConditionOperand) -> bool {
    match rhs {
        ConditionOperand::Value(value) => compare_values(left, op, value),
        ConditionOperand::Field(field) => item
            .get(field)
            .is_some_and(|right| compare_values(left, op, right)),
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

fn schema_to_dql(meta: &TableMeta) -> String {
    let mut ordered = Vec::new();
    if let Some(field) = meta.attrs.get(&meta.hash_key) {
        ordered.push(table_field_to_dql(field));
    }
    if let Some(range_key) = &meta.range_key {
        if let Some(field) = meta.attrs.get(range_key) {
            ordered.push(table_field_to_dql(field));
        }
    }
    ordered.extend(
        meta.attrs
            .iter()
            .filter(|(name, _)| {
                *name != &meta.hash_key
                    && meta
                        .range_key
                        .as_ref()
                        .is_none_or(|range_key| *name != range_key)
            })
            .map(|(_, field)| table_field_to_dql(field)),
    );
    let attrs = ordered.join(", ");
    format!("CREATE TABLE {} ({})", meta.name, attrs)
}

fn table_field_to_dql(attribute: &dql_models::TableField) -> String {
    let mut parts = vec![
        attribute.name.clone(),
        match &attribute.data_type {
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

    #[test]
    fn analyze_records_backend_capacity() {
        let mut engine = InMemoryEngine::new();
        engine
            .execute(
                "CREATE TABLE t (id STRING HASH KEY);
                 ANALYZE INSERT INTO t (id) VALUES ('a')",
            )
            .unwrap();
        assert_eq!(engine.consumed_capacities().len(), 1);
        assert_eq!(
            engine.consumed_capacities()[0].operation,
            "batch_write_item"
        );
    }

    #[test]
    fn backend_describes_created_table_metadata() {
        let mut backend = MemoryBackend::default();
        let statement = dql_parser::parse_statement("CREATE TABLE t (id STRING HASH KEY)").unwrap();
        let meta = TableMeta::from_create_statement(&statement).unwrap();
        backend.create_table(meta, false).unwrap();
        let desc = backend.describe_table("t").unwrap();
        assert_eq!(desc.hash_key, "id");
    }
}
