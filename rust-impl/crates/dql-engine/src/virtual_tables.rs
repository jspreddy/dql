//! Local file tables and process-local temp tables.
//!
//! A local table is a CSV or JSON file registered with `CREATE LOCALTABLE`.
//! Its definition is saved by [`LocalDefinitionStore`] and read again on startup.
//! A temp table is the in-memory result of `CREATE TEMPTABLE ... AS (SELECT|SCAN)`
//! and disappears when this process exits.

use crate::file_io::load_tabular_file;
use crate::{BackendResponse, DynamoBackend, EngineError, Item, ReadRequest};
use dql_models::{BillingMode, TableField, TableMeta, TableStatus};
use dql_parser::{AttributeType, KeyType, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableKind {
    DynamoDb,
    Local,
    Temp,
}

impl TableKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DynamoDb => "dynamodb",
            Self::Local => "local",
            Self::Temp => "temp",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::DynamoDb => "DYNAMODB",
            Self::Local => "LOCALTABLE",
            Self::Temp => "TEMPTABLE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalColumn {
    pub name: String,
    pub data_type: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDefinition {
    pub name: String,
    pub path: String,
    pub columns: Vec<LocalColumn>,
}

/// Encrypted cache of local-table definitions. Temp tables are not stored.
pub trait LocalDefinitionStore: Send + Sync {
    fn load(&self) -> Vec<LocalDefinition>;
    fn save(&self, definitions: &[LocalDefinition]);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ColumnSpec {
    name: String,
    data_type: AttributeType,
    key: Option<KeyType>,
}

struct VirtualTable {
    kind: TableKind,
    path: Option<PathBuf>,
    query_text: Option<String>,
    columns: Vec<ColumnSpec>,
    /// Identity of this temp table. A later `CREATE TEMPTABLE` of the same name
    /// gets a new number, so an old grant does not apply.
    generation: u64,
}

struct Grant {
    generation: u64,
    granted_at: String,
}

/// One column of a granted temp table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantColumn {
    pub name: String,
    pub data_type: String,
    /// `hash`, `range`, or absent.
    pub key: Option<String>,
}

/// A live MCP read grant. The rows stay in the temp table; this is only the label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantInfo {
    pub name: String,
    pub rows: usize,
    pub columns: Vec<GrantColumn>,
    /// `name TYPE` and the key role, comma-separated.
    pub columns_text: String,
    pub granted_at: String,
    pub query: String,
}

/// One page of a granted temp table, copied out of memory.
#[derive(Debug, Clone, PartialEq)]
pub struct GrantedPage {
    pub rows: Vec<Item>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}

#[derive(Default)]
pub struct VirtualCatalog {
    memory: crate::MemoryBackend,
    tables: HashMap<String, VirtualTable>,
    grants: HashMap<String, Grant>,
    next_generation: u64,
}

impl VirtualCatalog {
    pub fn contains(&self, name: &str) -> bool {
        self.tables.contains_key(name)
    }

    pub fn kind(&self, name: &str) -> Option<TableKind> {
        self.tables.get(name).map(|table| table.kind)
    }

    pub fn meta(&self, name: &str) -> Option<TableMeta> {
        self.memory.describe_table(name).ok().flatten()
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.tables.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn item_count(&self, name: &str) -> usize {
        self.memory.table_item_count(name)
    }

    pub fn column_signature(&self, name: &str) -> String {
        self.tables
            .get(name)
            .map(|table| {
                table
                    .columns
                    .iter()
                    .map(|column| format!("{}:{}", column.name, type_name(&column.data_type)))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default()
    }

    pub fn local_definitions(&self) -> Vec<LocalDefinition> {
        let mut defs: Vec<_> = self
            .tables
            .iter()
            .filter(|(_, table)| table.kind == TableKind::Local)
            .map(|(name, table)| LocalDefinition {
                name: name.clone(),
                path: table
                    .path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default(),
                columns: table
                    .columns
                    .iter()
                    .map(|column| LocalColumn {
                        name: column.name.clone(),
                        data_type: type_name(&column.data_type).to_string(),
                        key: key_name(column.key.as_ref()).to_string(),
                    })
                    .collect(),
            })
            .collect();
        defs.sort_by(|left, right| left.name.cmp(&right.name));
        defs
    }

    pub fn schema_line(&self, name: &str) -> Option<String> {
        let table = self.tables.get(name)?;
        match table.kind {
            TableKind::Local => {
                let path = table
                    .path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default();
                Some(format!(
                    "CREATE LOCALTABLE {name} AS {};",
                    sql_string(&path)
                ))
            }
            TableKind::Temp => {
                let query = table.query_text.as_deref().unwrap_or("SCAN * FROM unknown");
                Some(format!("CREATE TEMPTABLE {name} AS ({query});"))
            }
            TableKind::DynamoDb => None,
        }
    }

    pub fn create_local(
        &mut self,
        name: &str,
        path: &Path,
        if_not_exists: bool,
        taken: bool,
    ) -> Result<String, EngineError> {
        if self.tables.contains_key(name) || taken {
            if if_not_exists {
                return Ok(format!("Table '{name}' already exists"));
            }
            return Err(EngineError::Runtime(format!(
                "Table '{name}' already exists"
            )));
        }
        let absolute = absolute_path(path);
        let file = load_tabular_file(&absolute).map_err(EngineError::Runtime)?;
        let columns = infer_columns(&file.columns, &file.items).map_err(EngineError::Runtime)?;
        self.install(
            name,
            TableKind::Local,
            Some(absolute),
            None,
            0,
            columns,
            file.items,
        )?;
        Ok(format!("Created local table '{name}'"))
    }

    pub fn create_temp(
        &mut self,
        name: &str,
        query_text: &str,
        items: Vec<Item>,
        preferred: &[LocalColumn],
        if_not_exists: bool,
        taken: bool,
    ) -> Result<String, EngineError> {
        if self.tables.contains_key(name) || taken {
            if if_not_exists {
                return Ok(format!("Table '{name}' already exists"));
            }
            return Err(EngineError::Runtime(format!(
                "Table '{name}' already exists"
            )));
        }
        let preferred = columns_from_def(preferred);
        let columns = if items.is_empty() {
            if preferred.is_empty() {
                return Err(EngineError::Runtime(
                    "query returned no rows and no columns".to_string(),
                ));
            }
            let mut columns = preferred;
            if let Some(first) = columns.first_mut() {
                first.key = Some(KeyType::Hash);
            }
            columns
        } else {
            columns_for_items(&preferred, &items).map_err(EngineError::Runtime)?
        };
        let generation = self.next_generation;
        self.next_generation = self.next_generation.saturating_add(1);
        self.install(
            name,
            TableKind::Temp,
            None,
            Some(query_text.to_string()),
            generation,
            columns,
            items,
        )?;
        Ok(format!("Created temp table '{name}'"))
    }

    /// Read a saved definition. A missing file still registers the saved columns.
    pub fn restore(&mut self, definition: LocalDefinition) -> Result<(), EngineError> {
        let path = PathBuf::from(&definition.path);
        match load_tabular_file(&path) {
            Ok(file) => {
                let columns =
                    infer_columns(&file.columns, &file.items).map_err(EngineError::Runtime)?;
                self.install(
                    &definition.name,
                    TableKind::Local,
                    Some(path),
                    None,
                    0,
                    columns,
                    file.items,
                )
            }
            Err(err) => {
                let columns = columns_from_def(&definition.columns);
                if columns.is_empty() {
                    return Err(EngineError::Runtime(format!(
                        "local table '{}': {err}",
                        definition.name
                    )));
                }
                self.install(
                    &definition.name,
                    TableKind::Local,
                    Some(path),
                    None,
                    0,
                    columns,
                    Vec::new(),
                )?;
                Err(EngineError::Runtime(format!(
                    "local table '{}': {err}",
                    definition.name
                )))
            }
        }
    }

    pub fn drop_kind(
        &mut self,
        name: &str,
        kind: TableKind,
        if_exists: bool,
    ) -> Result<String, EngineError> {
        match self.tables.get(name).map(|table| table.kind) {
            Some(found) if found == kind => {
                self.memory.delete_table(name, true)?;
                self.tables.remove(name);
                self.grants.remove(name);
                let noun = match kind {
                    TableKind::Local => "local table",
                    TableKind::Temp => "temp table",
                    TableKind::DynamoDb => "table",
                };
                Ok(format!("Dropped {noun} '{name}'"))
            }
            Some(_) => Err(EngineError::Runtime(format!(
                "'{name}' is not a {}",
                kind.label()
            ))),
            None if if_exists => Ok(format!("{} '{name}' did not exist", kind.label())),
            None => Err(EngineError::Runtime(format!(
                "{} '{name}' not found",
                kind.label()
            ))),
        }
    }

    pub fn reload_local(&mut self, name: &str) -> Result<(), EngineError> {
        let Some(path) = self
            .tables
            .get(name)
            .filter(|table| table.kind == TableKind::Local)
            .and_then(|table| table.path.clone())
        else {
            return Ok(());
        };
        let file = load_tabular_file(&path).map_err(EngineError::Runtime)?;
        let columns = infer_columns(&file.columns, &file.items).map_err(EngineError::Runtime)?;
        self.install(
            name,
            TableKind::Local,
            Some(path),
            None,
            0,
            columns,
            file.items,
        )
    }

    /// Grant MCP read on a temp table that already exists. A second grant of the
    /// same table keeps the original time and says it was already granted.
    pub fn grant(&mut self, name: &str) -> Result<String, EngineError> {
        let generation = self
            .tables
            .get(name)
            .filter(|table| table.kind == TableKind::Temp)
            .map(|table| table.generation)
            .ok_or_else(|| EngineError::Runtime(format!("No temp table named '{name}'.")))?;
        let rows = self.item_count(name);
        let already = self
            .grants
            .get(name)
            .is_some_and(|grant| grant.generation == generation);
        if !already {
            self.grants.insert(
                name.to_string(),
                Grant {
                    generation,
                    granted_at: rfc3339_now(),
                },
            );
        }
        let verb = if already {
            "Already granted"
        } else {
            "Granted"
        };
        Ok(format!(
            "{verb} MCP read on temp table {name} ({rows} {}) until this server stops.",
            rows_word(rows)
        ))
    }

    pub fn revoke(&mut self, name: &str) -> String {
        if self.grant_live(name) {
            self.grants.remove(name);
            format!("Revoked MCP read on temp table {name}.")
        } else {
            format!("MCP read on temp table {name} was not granted.")
        }
    }

    pub fn revoke_all(&mut self) -> String {
        let count = self
            .grants
            .keys()
            .filter(|name| self.grant_live(name))
            .count();
        self.grants.clear();
        match count {
            0 => "No MCP grants.".to_string(),
            1 => "Revoked MCP read on 1 temp table.".to_string(),
            _ => format!("Revoked MCP read on {count} temp tables."),
        }
    }

    pub fn is_granted(&self, name: &str) -> bool {
        self.grant_live(name)
    }

    pub fn granted(&self) -> Vec<GrantInfo> {
        let mut names: Vec<_> = self
            .grants
            .keys()
            .filter(|name| self.grant_live(name))
            .cloned()
            .collect();
        names.sort();
        names
            .into_iter()
            .filter_map(|name| self.grant_info(&name))
            .collect()
    }

    /// Copy one page of a granted temp table. `limit` is 1 to 1000.
    /// `attributes` keeps only those names; a missing one is left out of the row.
    pub fn read_granted(
        &self,
        name: &str,
        limit: usize,
        offset: usize,
        attributes: Option<&[String]>,
    ) -> Result<GrantedPage, EngineError> {
        if !(1..=1000).contains(&limit) {
            return Err(EngineError::Runtime(
                "limit must be from 1 to 1000".to_string(),
            ));
        }
        if !self.grant_live(name) {
            return Err(EngineError::Runtime(format!(
                "'{name}' is not granted. Call list_granted_tables to see what is."
            )));
        }
        let Some((mut rows, total)) = self.memory.table_page(name, offset, limit) else {
            return Err(EngineError::Runtime(format!(
                "'{name}' is not granted. Call list_granted_tables to see what is."
            )));
        };
        if let Some(attributes) = attributes {
            for row in &mut rows {
                row.retain(|key, _| attributes.iter().any(|wanted| wanted == key));
            }
        }
        let next = offset.saturating_add(rows.len());
        Ok(GrantedPage {
            rows,
            total,
            offset,
            next_offset: (next < total).then_some(next),
        })
    }

    fn grant_live(&self, name: &str) -> bool {
        let Some(grant) = self.grants.get(name) else {
            return false;
        };
        self.tables.get(name).is_some_and(|table| {
            table.kind == TableKind::Temp && table.generation == grant.generation
        })
    }

    fn grant_info(&self, name: &str) -> Option<GrantInfo> {
        let granted_at = self.grants.get(name)?.granted_at.clone();
        let table = self.tables.get(name)?;
        let query = table
            .query_text
            .clone()
            .unwrap_or_else(|| "SCAN * FROM unknown".to_string());
        let columns = table
            .columns
            .iter()
            .map(|column| {
                let key = key_name(column.key.as_ref());
                GrantColumn {
                    name: column.name.clone(),
                    data_type: type_name(&column.data_type).to_string(),
                    key: if key.is_empty() {
                        None
                    } else {
                        Some(key.to_string())
                    },
                }
            })
            .collect::<Vec<_>>();
        let columns_text = columns
            .iter()
            .map(|column| match &column.key {
                Some(key) => format!("{} {} {key}", column.name, column.data_type),
                None => format!("{} {}", column.name, column.data_type),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let rows = self.item_count(name);
        Some(GrantInfo {
            name: name.to_string(),
            rows,
            columns,
            columns_text,
            granted_at,
            query,
        })
    }

    pub fn read(
        &mut self,
        table: &str,
        request: &ReadRequest<'_>,
    ) -> Result<BackendResponse<Vec<Item>>, EngineError> {
        self.reload_local(table)?;
        self.memory.execute_read(table, request)
    }

    pub fn batch_get(
        &mut self,
        table: &str,
        keys: &[Item],
        consistent: bool,
    ) -> Result<BackendResponse<Vec<Item>>, EngineError> {
        self.reload_local(table)?;
        self.memory.batch_get_keys(table, keys, consistent)
    }

    pub fn batch_write(
        &mut self,
        table: &str,
        items: Vec<Item>,
    ) -> Result<BackendResponse<usize>, EngineError> {
        self.ensure_mutable(table)?;
        self.memory.batch_write(table, items)
    }

    pub fn delete_by_keys(
        &mut self,
        table: &str,
        keys: &[Item],
        condition: Option<&dql_parser::Condition>,
    ) -> Result<BackendResponse<usize>, EngineError> {
        self.ensure_mutable(table)?;
        self.memory.delete_by_keys(table, keys, condition)
    }

    pub fn delete_matching(
        &mut self,
        table: &str,
        condition: Option<&dql_parser::Condition>,
        plan: Option<&dql_models::QueryPlan>,
        options: &dql_parser::QueryOptions,
    ) -> Result<BackendResponse<usize>, EngineError> {
        self.ensure_mutable(table)?;
        self.memory.delete_matching(table, condition, plan, options)
    }

    pub fn update_by_keys(
        &mut self,
        table: &str,
        keys: &[Item],
        update: &dql_parser::UpdateExpr,
        condition: Option<&dql_parser::Condition>,
        return_items: bool,
    ) -> Result<BackendResponse<usize>, EngineError> {
        self.ensure_mutable(table)?;
        self.memory
            .update_by_keys(table, keys, update, condition, return_items)
    }

    pub fn update_matching(
        &mut self,
        table: &str,
        update: &dql_parser::UpdateExpr,
        condition: Option<&dql_parser::Condition>,
        return_items: bool,
    ) -> Result<BackendResponse<usize>, EngineError> {
        self.ensure_mutable(table)?;
        self.memory
            .update_matching(table, update, condition, return_items)
    }

    fn ensure_mutable(&self, table: &str) -> Result<(), EngineError> {
        if self.kind(table) == Some(TableKind::Local) {
            return Err(EngineError::Runtime(format!(
                "local table '{table}' is read from its file"
            )));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn install(
        &mut self,
        name: &str,
        kind: TableKind,
        path: Option<PathBuf>,
        query_text: Option<String>,
        generation: u64,
        columns: Vec<ColumnSpec>,
        items: Vec<Item>,
    ) -> Result<(), EngineError> {
        if self.memory.describe_table(name)?.is_some() {
            self.memory.delete_table(name, true)?;
        }
        let meta = meta_from_columns(name, &columns).map_err(EngineError::Runtime)?;
        self.memory.create_table(meta, false)?;
        if !items.is_empty() {
            self.memory.batch_write(name, items)?;
        }
        self.tables.insert(
            name.to_string(),
            VirtualTable {
                kind,
                path,
                query_text,
                columns,
                generation,
            },
        );
        Ok(())
    }
}

pub fn columns_from_meta(meta: &TableMeta) -> Vec<LocalColumn> {
    let mut names = vec![meta.hash_key.clone()];
    if let Some(range) = &meta.range_key {
        names.push(range.clone());
    }
    let mut rest: Vec<_> = meta
        .attrs
        .keys()
        .filter(|name| !names.iter().any(|existing| existing == *name))
        .cloned()
        .collect();
    rest.sort();
    names.extend(rest);
    names
        .into_iter()
        .filter_map(|name| {
            let field = meta.attrs.get(&name)?;
            Some(LocalColumn {
                name,
                data_type: type_name(&field.data_type).to_string(),
                key: key_name(field.key_type.as_ref()).to_string(),
            })
        })
        .collect()
}

fn columns_from_def(columns: &[LocalColumn]) -> Vec<ColumnSpec> {
    columns
        .iter()
        .map(|column| ColumnSpec {
            name: column.name.clone(),
            data_type: parse_type(&column.data_type),
            key: match column.key.as_str() {
                "hash" => Some(KeyType::Hash),
                "range" => Some(KeyType::Range),
                _ => None,
            },
        })
        .collect()
}

fn columns_for_items(preferred: &[ColumnSpec], items: &[Item]) -> Result<Vec<ColumnSpec>, String> {
    let mut order = Vec::new();
    for column in preferred {
        if items.iter().any(|item| item.contains_key(&column.name)) {
            order.push(column.name.clone());
        }
    }
    let mut extras = Vec::new();
    for item in items {
        for key in item.keys() {
            if !order.iter().any(|name| name == key) && !extras.iter().any(|name| name == key) {
                extras.push(key.clone());
            }
        }
    }
    extras.sort();
    order.extend(extras);
    infer_columns(&order, items)
}

fn infer_columns(order: &[String], items: &[Item]) -> Result<Vec<ColumnSpec>, String> {
    if order.is_empty() {
        return Err("file has no columns".to_string());
    }
    let mut columns = Vec::new();
    for (index, name) in order.iter().enumerate() {
        columns.push(ColumnSpec {
            name: name.clone(),
            data_type: infer_type(name, items),
            key: if index == 0 {
                Some(KeyType::Hash)
            } else {
                None
            },
        });
    }
    let hash = &columns[0];
    if !matches!(
        hash.data_type,
        AttributeType::String | AttributeType::Number | AttributeType::Binary | AttributeType::Bool
    ) {
        return Err(format!("column '{}' cannot be a hash key", hash.name));
    }
    Ok(columns)
}

fn infer_type(name: &str, items: &[Item]) -> AttributeType {
    let mut seen_number = false;
    let mut seen_bool = false;
    let mut seen_string = false;
    let mut seen_binary = false;
    let mut seen_other = false;
    for item in items {
        match item.get(name) {
            None | Some(Value::Null) => {}
            Some(Value::Number(_)) => seen_number = true,
            Some(Value::Bool(_)) => seen_bool = true,
            Some(Value::String(_)) => seen_string = true,
            Some(Value::Binary(_)) => seen_binary = true,
            Some(_) => seen_other = true,
        }
    }
    let kinds = [seen_number, seen_bool, seen_string, seen_binary, seen_other]
        .into_iter()
        .filter(|seen| *seen)
        .count();
    if kinds != 1 {
        return AttributeType::String;
    }
    if seen_number {
        AttributeType::Number
    } else if seen_bool {
        AttributeType::Bool
    } else if seen_binary {
        AttributeType::Binary
    } else if seen_other {
        AttributeType::Other("MAP".to_string())
    } else {
        AttributeType::String
    }
}

fn meta_from_columns(name: &str, columns: &[ColumnSpec]) -> Result<TableMeta, String> {
    let hash = columns
        .iter()
        .find(|column| column.key == Some(KeyType::Hash))
        .ok_or_else(|| "missing hash key".to_string())?;
    let range = columns
        .iter()
        .find(|column| column.key == Some(KeyType::Range))
        .map(|column| column.name.clone());
    let attrs = columns
        .iter()
        .map(|column| {
            (
                column.name.clone(),
                TableField {
                    name: column.name.clone(),
                    data_type: column.data_type.clone(),
                    key_type: column.key.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    Ok(TableMeta {
        name: name.to_string(),
        attrs,
        hash_key: hash.name.clone(),
        range_key: range,
        local_indexes: BTreeMap::new(),
        global_indexes: BTreeMap::new(),
        throughput: None,
        billing_mode: BillingMode::OnDemand,
        status: TableStatus::Active,
        consumed_capacity: BTreeMap::new(),
    })
}

fn rows_word(rows: usize) -> &'static str {
    if rows == 1 {
        "row"
    } else {
        "rows"
    }
}

fn rfc3339_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

/// Howard Hinnant's `civil_from_days`. `days` is days since 1970-01-01.
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = (if month <= 2 { y + 1 } else { y }) as i32;
    (year, month, day)
}

fn type_name(data_type: &AttributeType) -> &str {
    match data_type {
        AttributeType::String => "STRING",
        AttributeType::Number => "NUMBER",
        AttributeType::Binary => "BINARY",
        AttributeType::Bool => "BOOL",
        AttributeType::Other(value) => value.as_str(),
    }
}

fn parse_type(name: &str) -> AttributeType {
    match name {
        "STRING" => AttributeType::String,
        "NUMBER" => AttributeType::Number,
        "BINARY" => AttributeType::Binary,
        "BOOL" => AttributeType::Bool,
        other => AttributeType::Other(other.to_string()),
    }
}

fn key_name(key: Option<&KeyType>) -> &'static str {
    match key {
        Some(KeyType::Hash) => "hash",
        Some(KeyType::Range) => "range",
        None => "",
    }
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn sql_string(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{in_memory_engine, StatementResult};
    use dql_parser::Value;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    struct MemStore(Mutex<Vec<LocalDefinition>>);

    impl LocalDefinitionStore for MemStore {
        fn load(&self) -> Vec<LocalDefinition> {
            self.0.lock().unwrap().clone()
        }

        fn save(&self, definitions: &[LocalDefinition]) {
            *self.0.lock().unwrap() = definitions.to_vec();
        }
    }

    fn write_file(dir: &Path, name: &str, text: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    fn items_of(result: StatementResult) -> Vec<Item> {
        match result {
            StatementResult::Items(items) => items,
            other => panic!("expected items, got {other:?}"),
        }
    }

    #[test]
    fn local_and_temp_tables_have_a_cached_definition_without_a_describe() {
        let dir = std::env::temp_dir().join(format!("dqlrs-table-defs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let csv = write_file(&dir, "users.csv", "id,name\na,ada\n");
        let mut engine = in_memory_engine();
        engine
            .execute(&format!("CREATE LOCALTABLE users AS '{}'", csv.display()))
            .unwrap();
        engine
            .execute("CREATE TABLE posts (id STRING HASH KEY, title STRING)")
            .unwrap();
        engine
            .execute("CREATE TEMPTABLE hot AS (SCAN * FROM posts)")
            .unwrap();

        let users = engine.cached_definition("users").expect("local definition");
        assert!(users.has_definition());
        assert_eq!(users.hash_key, "id");
        let hot = engine.cached_definition("hot").expect("temp definition");
        assert!(hot.has_definition());

        // The local file is not re-read: a broken file does not change the cached definition.
        std::fs::write(&csv, "not,a\n\"csv").unwrap();
        assert_eq!(engine.cached_definition("users").unwrap().hash_key, "id");

        let (kind, line) = engine.virtual_schema_line("users").unwrap();
        assert_eq!(kind, "local");
        assert!(line.starts_with("CREATE LOCALTABLE users AS '"), "{line}");
        let (kind, line) = engine.virtual_schema_line("hot").unwrap();
        assert_eq!(kind, "temp");
        assert_eq!(line, "CREATE TEMPTABLE hot AS (SCAN * FROM posts);");
        assert!(engine.virtual_schema_line("posts").is_none());

        engine.execute("DROP TEMPTABLE hot").unwrap();
        assert!(engine.cached_definition("hot").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clearing_local_tables_forgets_them_and_keeps_temp_tables_and_files() {
        let dir = std::env::temp_dir().join(format!("dqlrs-clear-locals-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let users = write_file(&dir, "users.csv", "id,name\na,ada\n");
        let cities = write_file(&dir, "cities.csv", "id,city\na,nyc\n");
        let store = Arc::new(MemStore(Mutex::new(Vec::new())));
        let mut engine = in_memory_engine();
        engine.set_local_store(Some(store.clone()));
        engine
            .execute(&format!("CREATE LOCALTABLE users AS '{}'", users.display()))
            .unwrap();
        engine
            .execute(&format!(
                "CREATE LOCALTABLE cities AS '{}'",
                cities.display()
            ))
            .unwrap();
        engine
            .execute("CREATE TEMPTABLE hot AS (SCAN * FROM users)")
            .unwrap();
        assert_eq!(store.0.lock().unwrap().len(), 2);

        let cleared = engine.clear_local_tables().unwrap();
        assert_eq!(cleared, vec!["cities".to_string(), "users".to_string()]);
        assert!(store.0.lock().unwrap().is_empty());
        assert!(engine.cached_definition("users").is_none());
        assert!(engine.cached_definition("hot").is_some());
        assert!(users.exists() && cities.exists());
        assert!(engine.clear_local_tables().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn local_and_temp_tables_query_like_dynamodb() {
        let dir = std::env::temp_dir().join(format!("dqlrs-table-kinds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let csv = write_file(&dir, "users.csv", "id,name,n\na,ada,1\nb,grace,2\n");
        write_file(
            &dir,
            "more.json",
            r#"[{"id":"a","city":"nyc"},{"id":"b","city":"sf"}]"#,
        );
        let store = Arc::new(MemStore(Mutex::new(Vec::new())));
        let mut engine = in_memory_engine();
        engine.set_local_store(Some(store.clone()));

        engine
            .execute(&format!("CREATE LOCALTABLE users AS '{}'", csv.display()))
            .unwrap();
        engine
            .execute("CREATE DYNAMODB TABLE posts (id STRING HASH KEY, title STRING)")
            .unwrap();
        engine
            .execute("INSERT INTO posts (id, title) VALUES ('a', 'one'), ('b', 'two')")
            .unwrap();
        let created = engine
            .execute("CREATE TEMPTABLE hot AS (SELECT * FROM posts WHERE id = 'a')")
            .unwrap();
        assert!(
            matches!(created, StatementResult::Status(message) if message.contains("temp table"))
        );

        let local = items_of(
            engine
                .execute("SELECT * FROM users WHERE id = 'a'")
                .unwrap(),
        );
        assert_eq!(local.len(), 1);
        assert_eq!(
            local[0].get("name"),
            Some(&Value::String("ada".to_string()))
        );
        assert_eq!(
            items_of(engine.execute("SCAN * FROM users").unwrap()).len(),
            2
        );

        let json = dir.join("more.json");
        engine
            .execute(&format!("CREATE LOCALTABLE cities AS '{}'", json.display()))
            .unwrap();
        let cities = items_of(engine.execute("SCAN * FROM cities").unwrap());
        assert_eq!(cities.len(), 2);

        let hot = items_of(engine.execute("SCAN * FROM hot").unwrap());
        assert_eq!(hot.len(), 1);
        assert_eq!(hot[0].get("title"), Some(&Value::String("one".to_string())));

        let listed = engine.execute("LS").unwrap().to_string();
        assert!(listed.contains("users"), "{listed}");
        assert!(listed.contains("cities"), "{listed}");
        assert!(listed.contains("hot"), "{listed}");
        assert!(listed.contains("posts"), "{listed}");

        let described = engine.execute("DESCRIBE users").unwrap().to_string();
        assert!(described.contains("Type: LOCALTABLE"), "{described}");
        assert!(
            described.contains("CREATE LOCALTABLE users AS"),
            "{described}"
        );

        engine.execute("DROP TEMPTABLE hot").unwrap();
        assert!(engine.execute("SCAN * FROM hot").is_err());

        let mut restarted = in_memory_engine();
        restarted.set_local_store(Some(store));
        assert_eq!(
            items_of(restarted.execute("SCAN * FROM users").unwrap()).len(),
            2
        );
        assert!(restarted.execute("SCAN * FROM hot").is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn cell<'a>(row: &'a Item, key: &str) -> &'a str {
        match row.get(key) {
            Some(Value::String(text)) => text,
            other => panic!("expected string {key}, got {other:?}"),
        }
    }

    #[test]
    fn with_clause_joins_each_side_once_from_memory() {
        let mut engine = in_memory_engine();
        engine
            .execute("CREATE TABLE posts (id STRING HASH KEY, customer STRING)")
            .unwrap();
        engine
            .execute("CREATE TABLE customers (id STRING HASH KEY, name STRING)")
            .unwrap();
        engine
            .execute(
                "INSERT INTO posts (id, customer) VALUES ('a', 'c1'), ('b', 'c1'), ('c', 'c9')",
            )
            .unwrap();
        engine
            .execute("INSERT INTO customers (id, name) VALUES ('c1', 'Ada'), ('c2', 'Grace')")
            .unwrap();

        let joined = items_of(
            engine
                .execute(
                    "WITH orders AS (SCAN * FROM posts), people AS (SCAN * FROM customers) \
                     SELECT * FROM orders JOIN people ON orders.customer = people.id",
                )
                .unwrap(),
        );
        assert_eq!(joined.len(), 2, "{joined:?}");
        assert!(
            joined.iter().all(|row| cell(row, "name") == "Ada"),
            "{joined:?}"
        );
        assert!(
            joined.iter().all(|row| cell(row, "people.id") == "c1"),
            "{joined:?}"
        );
        let ids: Vec<&str> = joined.iter().map(|row| cell(row, "id")).collect();
        assert_eq!(ids, vec!["a", "b"]);

        assert!(
            engine.execute("SCAN * FROM orders").is_err(),
            "WITH names last only for that statement"
        );

        let filtered = items_of(
            engine
                .execute(
                    "WITH orders AS (SCAN * FROM posts), people AS (SCAN * FROM customers) \
                     SELECT name FROM orders JOIN people ON people.id = orders.customer WHERE id = 'b'",
                )
                .unwrap(),
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(
            filtered[0].get("name"),
            Some(&Value::String("Ada".to_string()))
        );
        assert!(!filtered[0].contains_key("id"));

        engine
            .execute("CREATE TEMPTABLE orders AS (SCAN * FROM posts)")
            .unwrap();
        engine
            .execute("CREATE TEMPTABLE people AS (SCAN * FROM customers)")
            .unwrap();
        let chained = items_of(
            engine
                .execute("SCAN name FROM orders JOIN people ON customer = id")
                .unwrap(),
        );
        assert_eq!(chained.len(), 2);

        let direct = engine
            .execute("SELECT * FROM posts JOIN customers ON posts.customer = customers.id")
            .unwrap_err()
            .to_string();
        assert!(direct.contains("Load it with CREATE TEMPTABLE"), "{direct}");
    }

    fn posts_and_customers(engine: &mut crate::InMemoryEngine) {
        engine
            .execute("CREATE TABLE posts (id STRING HASH KEY, customer STRING)")
            .unwrap();
        engine
            .execute("CREATE TABLE customers (id STRING HASH KEY, name STRING)")
            .unwrap();
        engine
            .execute(
                "INSERT INTO posts (id, customer) VALUES ('a', 'c1'), ('b', 'c1'), ('c', 'c9')",
            )
            .unwrap();
        engine
            .execute("INSERT INTO customers (id, name) VALUES ('c1', 'Ada'), ('c2', 'Grace')")
            .unwrap();
    }

    #[test]
    fn temp_table_from_a_join_of_two_tables() {
        let mut engine = in_memory_engine();
        posts_and_customers(&mut engine);
        engine
            .execute(
                "CREATE TEMPTABLE hot AS (\
                    WITH orders AS (SCAN * FROM posts), people AS (SCAN * FROM customers) \
                    SELECT * FROM orders JOIN people ON orders.customer = people.id\
                 )",
            )
            .unwrap();
        let hot = items_of(engine.execute("SCAN * FROM hot").unwrap());
        assert_eq!(hot.len(), 2, "{hot:?}");
        assert!(hot.iter().all(|row| cell(row, "name") == "Ada"), "{hot:?}");
        assert!(engine.execute("SCAN * FROM orders").is_err());
        assert!(engine.execute("SCAN * FROM people").is_err());
    }

    #[test]
    fn temp_table_from_a_join_of_two_temp_tables() {
        let mut engine = in_memory_engine();
        posts_and_customers(&mut engine);
        engine
            .execute("CREATE TEMPTABLE orders AS (SCAN * FROM posts)")
            .unwrap();
        engine
            .execute("CREATE TEMPTABLE people AS (SCAN * FROM customers)")
            .unwrap();
        engine
            .execute(
                "CREATE TEMPTABLE hot AS (\
                    SELECT * FROM orders JOIN people ON orders.customer = people.id\
                 )",
            )
            .unwrap();
        let hot = items_of(engine.execute("SCAN * FROM hot").unwrap());
        assert_eq!(hot.len(), 2, "{hot:?}");
        assert!(hot.iter().all(|row| cell(row, "name") == "Ada"), "{hot:?}");
        assert_eq!(
            items_of(engine.execute("SCAN * FROM orders").unwrap()).len(),
            3
        );
        assert_eq!(
            items_of(engine.execute("SCAN * FROM people").unwrap()).len(),
            2
        );
    }

    #[test]
    fn four_table_join() {
        let mut engine = in_memory_engine();
        engine
            .execute("CREATE TABLE a (id STRING HASH KEY, nxt STRING)")
            .unwrap();
        engine
            .execute("CREATE TABLE b (id STRING HASH KEY, nxt STRING)")
            .unwrap();
        engine
            .execute("CREATE TABLE c (id STRING HASH KEY, nxt STRING)")
            .unwrap();
        engine
            .execute("CREATE TABLE d (id STRING HASH KEY, label STRING)")
            .unwrap();
        engine
            .execute("INSERT INTO a (id, nxt) VALUES ('a', 'b')")
            .unwrap();
        engine
            .execute("INSERT INTO b (id, nxt) VALUES ('b', 'c')")
            .unwrap();
        engine
            .execute("INSERT INTO c (id, nxt) VALUES ('c', 'd')")
            .unwrap();
        engine
            .execute("INSERT INTO d (id, label) VALUES ('d', 'end')")
            .unwrap();
        let joined = items_of(
            engine
                .execute(
                    "WITH wa AS (SCAN * FROM a), wb AS (SCAN * FROM b), \
                     wc AS (SCAN * FROM c), wd AS (SCAN * FROM d) \
                     SELECT * FROM wa \
                     JOIN wb ON wa.nxt = wb.id \
                     JOIN wc ON wb.nxt = wc.id \
                     JOIN wd ON wc.nxt = wd.id",
                )
                .unwrap(),
        );
        assert_eq!(joined.len(), 1, "{joined:?}");
        assert_eq!(cell(&joined[0], "id"), "a");
        assert_eq!(cell(&joined[0], "label"), "end");
        assert_eq!(cell(&joined[0], "wb.id"), "b");
        assert_eq!(cell(&joined[0], "wc.id"), "c");
        assert_eq!(cell(&joined[0], "wd.id"), "d");
    }

    fn insert_numbered(
        engine: &mut crate::InMemoryEngine,
        table: &str,
        columns: &str,
        rows: &[(String, String)],
    ) {
        let values = rows
            .iter()
            .map(|(left, right)| format!("('{left}', '{right}')"))
            .collect::<Vec<_>>()
            .join(", ");
        engine
            .execute(&format!("INSERT INTO {table} ({columns}) VALUES {values}"))
            .unwrap();
    }

    #[test]
    fn join_one_thousand_rows_to_ten() {
        let mut engine = in_memory_engine();
        engine
            .execute("CREATE TABLE big (id STRING HASH KEY, bucket STRING)")
            .unwrap();
        engine
            .execute("CREATE TABLE small (id STRING HASH KEY, name STRING)")
            .unwrap();
        let left: Vec<(String, String)> = (0..1000)
            .map(|n| (n.to_string(), (n % 10).to_string()))
            .collect();
        let right: Vec<(String, String)> =
            (0..10).map(|n| (n.to_string(), format!("n{n}"))).collect();
        insert_numbered(&mut engine, "big", "id, bucket", &left);
        insert_numbered(&mut engine, "small", "id, name", &right);
        let joined = items_of(
            engine
                .execute(
                    "WITH lefts AS (SCAN * FROM big), rights AS (SCAN * FROM small) \
                     SELECT * FROM lefts JOIN rights ON lefts.bucket = rights.id",
                )
                .unwrap(),
        );
        assert_eq!(joined.len(), 1000);
        assert_eq!(
            joined
                .iter()
                .filter(|row| cell(row, "name") == "n5")
                .count(),
            100
        );
        let row = joined
            .iter()
            .find(|row| cell(row, "id") == "15")
            .expect("row 15");
        assert_eq!(cell(row, "bucket"), "5");
        assert_eq!(cell(row, "name"), "n5");
        assert_eq!(cell(row, "rights.id"), "5");
    }

    #[test]
    fn join_one_thousand_rows_to_one_thousand() {
        let mut engine = in_memory_engine();
        engine
            .execute("CREATE TABLE big (id STRING HASH KEY, mark STRING)")
            .unwrap();
        engine
            .execute("CREATE TABLE other (id STRING HASH KEY, note STRING)")
            .unwrap();
        let left: Vec<(String, String)> = (0..1000)
            .map(|n| (n.to_string(), format!("L{n}")))
            .collect();
        let right: Vec<(String, String)> = (0..1000)
            .map(|n| (n.to_string(), format!("R{n}")))
            .collect();
        insert_numbered(&mut engine, "big", "id, mark", &left);
        insert_numbered(&mut engine, "other", "id, note", &right);
        let joined = items_of(
            engine
                .execute(
                    "WITH lefts AS (SCAN * FROM big), rights AS (SCAN * FROM other) \
                     SELECT * FROM lefts JOIN rights ON lefts.id = rights.id",
                )
                .unwrap(),
        );
        assert_eq!(joined.len(), 1000);
        let row = joined
            .iter()
            .find(|row| cell(row, "id") == "15")
            .expect("row 15");
        assert_eq!(cell(row, "mark"), "L15");
        assert_eq!(cell(row, "note"), "R15");
        assert!(!row.contains_key("rights.id"));
    }
}
