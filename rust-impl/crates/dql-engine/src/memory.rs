use crate::{
    BackendResponse, CapacityRecord, DynamoBackend, EngineError, Item, ReadOperation, ReadRequest,
};
use dql_expr::{render_condition, render_update};
use dql_models::TableMeta;
use dql_parser::{
    AlterAction, AttributeType, CompareOp, Condition, ConditionOperand, KeyType, QueryOptions,
    Selection, UpdateClauseKind, UpdateExpr, Value,
};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq)]
struct TableData {
    meta: TableMeta,
    items: Vec<Item>,
}

#[derive(Debug, Default)]
pub struct MemoryBackend {
    tables: HashMap<String, TableData>,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DynamoBackend for MemoryBackend {
    fn list_tables(&self) -> Result<Vec<String>, EngineError> {
        let mut names: Vec<_> = self.tables.keys().cloned().collect();
        names.sort();
        Ok(names)
    }

    fn describe_table(&self, table: &str) -> Result<Option<TableMeta>, EngineError> {
        Ok(self.tables.get(table).map(|table| table.meta.clone()))
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

    fn execute_read(
        &self,
        table: &str,
        request: &ReadRequest<'_>,
    ) -> Result<BackendResponse<Vec<Item>>, EngineError> {
        let table_data = self
            .tables
            .get(table)
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let condition = request
            .filter_condition
            .or(request.key_condition)
            .or(request.condition);
        let items = apply_read_options(table_data.items.iter(), condition, &request.options);
        let op_name = match request.operation {
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

    fn update_matching(
        &mut self,
        table: &str,
        update: &UpdateExpr,
        condition: Option<&Condition>,
    ) -> Result<BackendResponse<usize>, EngineError> {
        let _ = render_update(update).map_err(|err| EngineError::Runtime(err.to_string()))?;
        let table_data = self
            .tables
            .get_mut(table)
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let mut count = 0;
        for item in &mut table_data.items {
            if condition.is_none_or(|condition| matches_condition(item, condition)) {
                apply_update(item, update)?;
                count += 1;
            }
        }
        Ok(BackendResponse::new("update_item", table, count))
    }

    fn alter_table(
        &mut self,
        table: &str,
        action: &AlterAction,
    ) -> Result<BackendResponse<String>, EngineError> {
        let table_data = self
            .tables
            .get_mut(table)
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let message = match action {
            AlterAction::SetThroughput { index, throughput } => {
                if let Some(index_name) = index {
                    let index = table_data
                        .meta
                        .global_indexes
                        .get_mut(index_name)
                        .ok_or_else(|| {
                            EngineError::Runtime(format!("unknown index '{index_name}'"))
                        })?;
                    index.throughput = Some(throughput.clone());
                    format!("Updated throughput for index '{index_name}' on '{table}'")
                } else {
                    table_data.meta.throughput = Some(throughput.clone());
                    format!("Updated throughput for table '{table}'")
                }
            }
            AlterAction::DropIndex { name, if_exists } => {
                if table_data.meta.global_indexes.remove(name).is_some() {
                    format!("Dropped index '{name}' from '{table}'")
                } else if *if_exists {
                    format!("Index '{name}' did not exist on '{table}'")
                } else {
                    return Err(EngineError::Runtime(format!(
                        "Index '{name}' not found on '{table}'"
                    )));
                }
            }
            AlterAction::CreateGlobalIndex {
                index,
                if_not_exists,
            } => {
                if table_data.meta.global_indexes.contains_key(&index.name) {
                    if *if_not_exists {
                        format!("Index '{}' already exists on '{table}'", index.name)
                    } else {
                        return Err(EngineError::Runtime(format!(
                            "Index '{}' already exists on '{table}'",
                            index.name
                        )));
                    }
                } else {
                    use dql_models::{GlobalIndexMeta, ProjectionType, TableField, TableStatus};
                    use dql_parser::ProjectionKind;
                    table_data.meta.global_indexes.insert(
                        index.name.clone(),
                        GlobalIndexMeta {
                            name: index.name.clone(),
                            hash_key: TableField {
                                name: index.hash_key.clone(),
                                data_type: AttributeType::Other("UNKNOWN".to_string()),
                                key_type: Some(KeyType::Hash),
                            },
                            range_key: index.range_key.as_ref().map(|name| TableField {
                                name: name.clone(),
                                data_type: AttributeType::Other("UNKNOWN".to_string()),
                                key_type: Some(KeyType::Range),
                            }),
                            projection: match index.projection {
                                ProjectionKind::All => ProjectionType::All,
                                ProjectionKind::Keys => ProjectionType::KeysOnly,
                                ProjectionKind::Include => {
                                    ProjectionType::Include(index.includes.clone())
                                }
                            },
                            throughput: index.throughput.clone(),
                            status: TableStatus::Active,
                        },
                    );
                    format!("Created global index '{}' on '{table}'", index.name)
                }
            }
        };
        Ok(BackendResponse::new("update_table", table, message))
    }
}

pub fn matches_condition(item: &Item, condition: &Condition) -> bool {
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

fn compare_operand(item: &Item, left: &Value, op: &CompareOp, rhs: &ConditionOperand) -> bool {
    match rhs {
        ConditionOperand::Value(value) => compare_values(left, op, value),
        ConditionOperand::Field(field) => item
            .get(field)
            .is_some_and(|right| compare_values(left, op, right)),
    }
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

fn apply_update(item: &mut Item, update: &UpdateExpr) -> Result<(), EngineError> {
    for clause in &update.clauses {
        match clause.kind {
            UpdateClauseKind::Set => {
                let value = parse_update_value(clause.expression.as_deref().unwrap_or_default())?;
                item.insert(clause.path.clone(), value);
            }
            UpdateClauseKind::Remove => {
                item.remove(&clause.path);
            }
            UpdateClauseKind::Add => {
                let delta = parse_update_value(clause.expression.as_deref().unwrap_or_default())?;
                let current = item.get(&clause.path).cloned().unwrap_or(Value::Number("0".to_string()));
                item.insert(clause.path.clone(), add_values(&current, &delta)?);
            }
            UpdateClauseKind::Delete => {
                let value = parse_update_value(clause.expression.as_deref().unwrap_or_default())?;
                if let Some(Value::Set(mut values)) = item.remove(&clause.path) {
                    if let Value::Set(remove) = value {
                        values.retain(|entry| !remove.contains(entry));
                    }
                    if !values.is_empty() {
                        item.insert(clause.path.clone(), Value::Set(values));
                    }
                }
            }
        }
    }
    Ok(())
}

fn add_values(left: &Value, right: &Value) -> Result<Value, EngineError> {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => {
            let sum = left.parse::<f64>().map_err(|err| EngineError::Runtime(err.to_string()))?
                + right.parse::<f64>().map_err(|err| EngineError::Runtime(err.to_string()))?;
            Ok(Value::Number(sum.to_string()))
        }
        _ => Err(EngineError::Runtime("unsupported ADD update".to_string())),
    }
}

fn parse_update_value(raw: &str) -> Result<Value, EngineError> {
    let trimmed = raw.trim();
    if is_number(trimmed) {
        return Ok(Value::Number(trimmed.to_string()));
    }
    if is_quoted(trimmed) {
        return Ok(Value::String(
            trimmed.trim_matches('"').trim_matches('\'').to_string(),
        ));
    }
    if trimmed.eq_ignore_ascii_case("true") {
        return Ok(Value::Bool(true));
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return Ok(Value::Bool(false));
    }
    if trimmed.eq_ignore_ascii_case("null") {
        return Ok(Value::Null);
    }
    Err(EngineError::Runtime(format!("unsupported update value '{raw}'")))
}

fn is_number(value: &str) -> bool {
    value.parse::<f64>().is_ok()
}

fn is_quoted(value: &str) -> bool {
    (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dql_models::TableMeta;
    use dql_parser::parse_statement;

    #[test]
    fn backend_describes_created_table_metadata() {
        let mut backend = MemoryBackend::default();
        let statement = parse_statement("CREATE TABLE t (id STRING HASH KEY)").unwrap();
        let meta = TableMeta::from_create_statement(&statement).unwrap();
        backend.create_table(meta, false).unwrap();
        let desc = backend.describe_table("t").unwrap().unwrap();
        assert_eq!(desc.hash_key, "id");
    }
}
