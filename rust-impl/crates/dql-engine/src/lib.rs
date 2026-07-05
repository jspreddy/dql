mod aws;
mod convert;
mod engine;
mod json_util;
mod memory;
mod throttle;

pub use aws::{SdkBackend, SdkConfig};
pub use engine::Engine;
pub use memory::MemoryBackend;

use crate::json_util::{item_to_json, string_to_json};
use dql_parser::{AlterAction, Condition, OrderBy, QueryOptions, Selection, UpdateExpr};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

pub type Item = BTreeMap<String, dql_parser::Value>;
pub type InMemoryEngine = Engine<MemoryBackend>;

pub fn format_throughput(available: Option<f64>, used: Option<f64>) -> String {
    dql_models::format_throughput(available, used)
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
    pub count: Option<usize>,
    pub updated_items: Option<Vec<Item>>,
}

impl<T> BackendResponse<T> {
    pub fn new(operation: &str, table: &str, output: T) -> Self {
        Self {
            output,
            capacity: Some(CapacityRecord {
                operation: operation.to_string(),
                table: table.to_string(),
                read_units: 0.0,
                write_units: 0.0,
            }),
            count: None,
            updated_items: None,
        }
    }
}

#[derive(Debug)]
pub enum EngineError {
    Parse(dql_parser::ParseError),
    Runtime(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOperation {
    Query,
    Scan,
    BatchGetKeys,
}

pub struct ReadRequest<'a> {
    pub operation: ReadOperation,
    pub index_name: Option<&'a str>,
    pub key_condition: Option<&'a Condition>,
    pub filter_condition: Option<&'a Condition>,
    pub condition: Option<&'a Condition>,
    pub selection: &'a Selection,
    pub options: &'a QueryOptions,
    pub consistent: bool,
    pub order_by: Option<&'a OrderBy>,
    pub follow_up_batch_get: bool,
}

pub trait DynamoBackend {
    fn list_tables(&self) -> Result<Vec<String>, EngineError>;
    fn describe_table(&self, table: &str) -> Result<Option<dql_models::TableMeta>, EngineError>;
    fn create_table(
        &mut self,
        meta: dql_models::TableMeta,
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
    fn execute_read(
        &self,
        table: &str,
        request: &ReadRequest<'_>,
    ) -> Result<BackendResponse<Vec<Item>>, EngineError>;
    fn batch_get_keys(
        &self,
        table: &str,
        keys: &[Item],
        consistent: bool,
    ) -> Result<BackendResponse<Vec<Item>>, EngineError>;
    fn delete_by_keys(
        &mut self,
        table: &str,
        keys: &[Item],
        condition: Option<&Condition>,
    ) -> Result<BackendResponse<usize>, EngineError>;
    fn update_by_keys(
        &mut self,
        table: &str,
        keys: &[Item],
        update: &UpdateExpr,
        condition: Option<&Condition>,
        return_items: bool,
    ) -> Result<BackendResponse<usize>, EngineError>;
    fn delete_matching(
        &mut self,
        table: &str,
        condition: Option<&Condition>,
        plan: Option<&dql_models::QueryPlan>,
        options: &QueryOptions,
    ) -> Result<BackendResponse<usize>, EngineError>;
    fn update_matching(
        &mut self,
        table: &str,
        update: &UpdateExpr,
        condition: Option<&Condition>,
        return_items: bool,
    ) -> Result<BackendResponse<usize>, EngineError>;
    fn alter_table(
        &mut self,
        table: &str,
        action: &AlterAction,
    ) -> Result<BackendResponse<String>, EngineError>;
}

pub fn in_memory_engine() -> InMemoryEngine {
    Engine::new(MemoryBackend::new())
}

impl Default for InMemoryEngine {
    fn default() -> Self {
        in_memory_engine()
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

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Parse(err) => write!(f, "{err}"),
            EngineError::Runtime(message) => write!(f, "{message}"),
        }
    }
}

impl Error for EngineError {}

impl From<dql_parser::ParseError> for EngineError {
    fn from(value: dql_parser::ParseError) -> Self {
        EngineError::Parse(value)
    }
}
