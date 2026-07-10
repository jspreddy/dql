use crate::convert::{item_matches_primary_key, keys_in_to_items};
use crate::{
    BackendResponse, CapacityRecord, DynamoBackend, EngineError, Item, ReadOperation, ReadRequest,
};
use dql_expr::{render_update, resolve_timestamp};
use dql_models::{BillingMode, QueryPlan, TableMeta};
use dql_parser::{
    parse_value, AlterAction, AttributeType, CompareOp, Condition, ConditionOperand, KeyType,
    QueryOptions, Selection, Throughput, UpdateClauseKind, UpdateExpr, Value,
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

    fn table_item_count(&self, table: &str) -> usize {
        self.tables
            .get(table)
            .map(|table| table.items.len())
            .unwrap_or(0)
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
        Ok(BackendResponse {
            output: count,
            capacity: Some(CapacityRecord {
                operation: "batch_write_item".to_string(),
                table: table.to_string(),
                read_units: 0.0,
                write_units: count as f64,
            }),
            count: None,
            updated_items: None,
        })
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
        if request.operation == ReadOperation::BatchGetKeys {
            let keys_in =
                request.options.keys_in.as_ref().ok_or_else(|| {
                    EngineError::Runtime("batch get requires KEYS IN".to_string())
                })?;
            let keys = keys_in_to_items(&table_data.meta, keys_in)?;
            return self.batch_get_keys(table, &keys, request.consistent);
        }
        let items = apply_read_options(table_data.items.iter(), request);
        let count = matches!(request.selection, Selection::CountAll).then_some(items.len());
        let op_name = match request.operation {
            ReadOperation::Query => "query",
            ReadOperation::Scan => "scan",
            ReadOperation::BatchGetKeys => "batch_get_item",
        };
        Ok(BackendResponse {
            output: items,
            capacity: Some(CapacityRecord {
                operation: op_name.to_string(),
                table: table.to_string(),
                read_units: 0.0,
                write_units: 0.0,
            }),
            count,
            updated_items: None,
        })
    }

    fn batch_get_keys(
        &self,
        table: &str,
        keys: &[Item],
        _consistent: bool,
    ) -> Result<BackendResponse<Vec<Item>>, EngineError> {
        let table_data = self
            .tables
            .get(table)
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let mut items = Vec::new();
        for key in keys {
            if let Some(item) = table_data
                .items
                .iter()
                .find(|item| item_matches_primary_key(item, key, &table_data.meta))
            {
                items.push(item.clone());
            }
        }
        Ok(BackendResponse::new("batch_get_item", table, items))
    }

    fn delete_by_keys(
        &mut self,
        table: &str,
        keys: &[Item],
        condition: Option<&Condition>,
    ) -> Result<BackendResponse<usize>, EngineError> {
        let table_data = self
            .tables
            .get_mut(table)
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let before = table_data.items.len();
        table_data.items.retain(|item| {
            if !keys
                .iter()
                .any(|key| item_matches_primary_key(item, key, &table_data.meta))
            {
                return true;
            }
            !condition.is_none_or(|condition| matches_condition(item, condition))
        });
        Ok(BackendResponse::new(
            "delete_item",
            table,
            before - table_data.items.len(),
        ))
    }

    fn update_by_keys(
        &mut self,
        table: &str,
        keys: &[Item],
        update: &UpdateExpr,
        condition: Option<&Condition>,
        return_items: bool,
    ) -> Result<BackendResponse<usize>, EngineError> {
        let _ = render_update(update).map_err(|err| EngineError::Runtime(err.to_string()))?;
        let table_data = self
            .tables
            .get_mut(table)
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let meta = table_data.meta.clone();
        let mut count = 0;
        let mut updated_items = Vec::new();
        for item in &mut table_data.items {
            if keys
                .iter()
                .any(|key| item_matches_primary_key(item, key, &meta))
                && condition.is_none_or(|condition| matches_condition(item, condition))
            {
                apply_update(item, update)?;
                count += 1;
                if return_items {
                    updated_items.push(item.clone());
                }
            }
        }
        Ok(BackendResponse {
            updated_items: return_items.then_some(updated_items),
            ..BackendResponse::new("update_item", table, count)
        })
    }

    fn delete_matching(
        &mut self,
        table: &str,
        condition: Option<&Condition>,
        plan: Option<&QueryPlan>,
        options: &QueryOptions,
    ) -> Result<BackendResponse<usize>, EngineError> {
        let table_data = self
            .tables
            .get_mut(table)
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let before = table_data.items.len();
        if condition.is_none() && plan.is_none() {
            let count = table_data.items.len();
            table_data.items.clear();
            return Ok(BackendResponse::new("delete_item", table, count));
        }
        let key_condition = plan.and_then(|plan| plan.key_condition.as_ref());
        let filter_condition =
            plan.and_then(|plan| plan.filter_condition.as_ref())
                .or(if key_condition.is_none() {
                    condition
                } else {
                    None
                });
        table_data.items.retain(|item| {
            let filter_request = ReadRequest {
                operation: ReadOperation::Scan,
                index_name: None,
                key_condition,
                filter_condition,
                condition: if key_condition.is_none() {
                    condition
                } else {
                    None
                },
                selection: &Selection::All,
                options,
                consistent: false,
                order_by: None,
                scan_index_forward: None,
                range_key: None,
                follow_up_batch_get: false,
            };
            apply_read_options(std::iter::once(item), &filter_request).is_empty()
        });
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
        return_items: bool,
    ) -> Result<BackendResponse<usize>, EngineError> {
        let _ = render_update(update).map_err(|err| EngineError::Runtime(err.to_string()))?;
        let table_data = self
            .tables
            .get_mut(table)
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let mut count = 0;
        let mut updated_items = Vec::new();
        for item in &mut table_data.items {
            if condition.is_none_or(|condition| matches_condition(item, condition)) {
                apply_update(item, update)?;
                count += 1;
                if return_items {
                    updated_items.push(item.clone());
                }
            }
        }
        Ok(BackendResponse {
            updated_items: return_items.then_some(updated_items),
            ..BackendResponse::new("update_item", table, count)
        })
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
                    let current = index.throughput.as_ref();
                    index.throughput = Some(resolve_throughput(
                        throughput,
                        current.map(|value| &value.read),
                        current.map(|value| &value.write),
                    ));
                    format!("Updated throughput for index '{index_name}' on '{table}'")
                } else {
                    let current = table_data.meta.throughput.as_ref();
                    let resolved = resolve_throughput(
                        throughput,
                        current.map(|value| &value.read),
                        current.map(|value| &value.write),
                    );
                    table_data.meta.billing_mode = if throughput_is_on_demand(&resolved) {
                        BillingMode::OnDemand
                    } else {
                        BillingMode::Provisioned
                    };
                    table_data.meta.throughput = Some(resolved);
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
        Condition::Compare { field, op, rhs } => resolve_field_value(item, field)
            .is_some_and(|left| compare_operand(item, &left, op, rhs)),
        Condition::Between { field, low, high } => {
            resolve_field_value(item, field).is_some_and(|item_value| {
                compare_values(&item_value, &CompareOp::Ge, low)
                    && compare_values(&item_value, &CompareOp::Le, high)
            })
        }
        Condition::In { field, values } => {
            resolve_field_value(item, field).is_some_and(|item_value| values.contains(&item_value))
        }
        Condition::Function { name, args } => evaluate_function_condition(item, name, args),
        Condition::Size { .. } | Condition::AttributeType { .. } => false,
        Condition::And(conditions) => conditions
            .iter()
            .all(|condition| matches_condition(item, condition)),
        Condition::Or(conditions) => conditions
            .iter()
            .any(|condition| matches_condition(item, condition)),
        Condition::Not(condition) => !matches_condition(item, condition),
    }
}

fn evaluate_function_condition(item: &Item, name: &str, args: &[ConditionOperand]) -> bool {
    match name.to_ascii_lowercase().as_str() {
        "begins_with" => {
            let (field, prefix) = match args {
                [ConditionOperand::Field(field), ConditionOperand::Value(prefix)] => {
                    (field, prefix)
                }
                _ => return false,
            };
            resolve_field_value(item, field).is_some_and(|value| match (value, prefix) {
                (Value::String(value), Value::String(prefix)) => value.starts_with(prefix),
                _ => false,
            })
        }
        "attribute_exists" => {
            let field = function_field_arg(args);
            field.is_some_and(|field| field_exists(item, &field))
        }
        "attribute_not_exists" => {
            let field = function_field_arg(args);
            field.is_some_and(|field| !field_exists(item, &field))
        }
        "contains" => {
            let (field, needle) = match args {
                [ConditionOperand::Field(field), ConditionOperand::Value(needle)] => {
                    (field, needle)
                }
                _ => return false,
            };
            resolve_field_value(item, field).is_some_and(|value| match value {
                Value::String(value) => needle
                    .as_string()
                    .is_some_and(|needle| value.contains(needle)),
                Value::Set(values) => values.contains(needle),
                Value::List(values) => values.contains(needle),
                _ => false,
            })
        }
        _ => false,
    }
}

fn function_field_arg(args: &[ConditionOperand]) -> Option<String> {
    match args.first()? {
        ConditionOperand::Field(field) => Some(field.clone()),
        ConditionOperand::Value(Value::String(field)) => Some(field.clone()),
        _ => None,
    }
}

fn field_exists(item: &Item, path: &str) -> bool {
    resolve_field_value(item, path).is_some()
}

fn resolve_field_value(item: &Item, path: &str) -> Option<Value> {
    if let Some(index) = path.find('[') {
        let head = &path[..index];
        let rest = &path[index..];
        let end = rest.find(']')?;
        let list = resolve_field_value(item, head)?;
        let Value::List(values) = list else {
            return None;
        };
        let idx = parse_list_index(&rest[1..end], values.len())?;
        return values.get(idx).cloned();
    }
    if let Some(index) = path.find('.') {
        let head = &path[..index];
        let tail = &path[index + 1..];
        let map = resolve_field_value(item, head)?;
        let Value::Map(values) = map else {
            return None;
        };
        return resolve_field_value_map(&values, tail);
    }
    item.get(path).cloned()
}

fn resolve_field_value_map(values: &BTreeMap<String, Value>, path: &str) -> Option<Value> {
    if let Some(index) = path.find('[') {
        let head = &path[..index];
        let rest = &path[index..];
        let end = rest.find(']')?;
        let list = values.get(head)?.clone();
        let Value::List(values) = list else {
            return None;
        };
        let idx = parse_list_index(&rest[1..end], values.len())?;
        return values.get(idx).cloned();
    }
    if let Some(index) = path.find('.') {
        let head = &path[..index];
        let tail = &path[index + 1..];
        let Value::Map(values) = values.get(head)? else {
            return None;
        };
        return resolve_field_value_map(values, tail);
    }
    values.get(path).cloned()
}

fn parse_list_index(index: &str, len: usize) -> Option<usize> {
    index.parse::<usize>().ok().or_else(|| {
        index
            .parse::<isize>()
            .ok()
            .and_then(|index| len.checked_sub(index.unsigned_abs()))
    })
}

trait ValueExt {
    fn as_string(&self) -> Option<&str>;
}

impl ValueExt for Value {
    fn as_string(&self) -> Option<&str> {
        match self {
            Value::String(value) => Some(value),
            _ => None,
        }
    }
}

fn apply_read_options<'a>(
    items: impl Iterator<Item = &'a Item>,
    request: &ReadRequest<'_>,
) -> Vec<Item> {
    let scanned = items.take(request.options.scan_limit.unwrap_or(usize::MAX));
    let mut filtered: Vec<Item> = scanned
        .filter(|item| {
            let key_ok = match request.key_condition {
                Some(condition) => matches_condition(item, condition),
                None => request
                    .condition
                    .is_none_or(|condition| matches_condition(item, condition)),
            };
            request
                .filter_condition
                .is_none_or(|condition| matches_condition(item, condition))
                && key_ok
        })
        .cloned()
        .collect();

    // DynamoDB Query returns items in range-key order; emulate that for memory.
    if request.operation == ReadOperation::Query {
        if let Some(range_key) = request.range_key {
            let descending = request.scan_index_forward == Some(false);
            filtered.sort_by(|left, right| {
                let ordering = compare_order(
                    left.get(range_key).unwrap_or(&Value::Null),
                    right.get(range_key).unwrap_or(&Value::Null),
                )
                .unwrap_or(std::cmp::Ordering::Equal);
                if descending {
                    ordering.reverse()
                } else {
                    ordering
                }
            });
        }
    }

    filtered
        .into_iter()
        .take(request.options.limit.unwrap_or(usize::MAX))
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
        (Value::Timestamp(left), Value::Timestamp(right)) => {
            resolve_timestamp(left).partial_cmp(&resolve_timestamp(right))
        }
        (Value::Timestamp(left), Value::Number(right)) => {
            let right = right.parse::<f64>().ok()?;
            resolve_timestamp(left).partial_cmp(&right)
        }
        (Value::Number(left), Value::Timestamp(right)) => {
            let left = left.parse::<f64>().ok()?;
            left.partial_cmp(&resolve_timestamp(right))
        }
        _ => None,
    }
}

fn apply_update(item: &mut Item, update: &UpdateExpr) -> Result<(), EngineError> {
    for clause in &update.clauses {
        match clause.kind {
            UpdateClauseKind::Set => {
                let value =
                    eval_set_expression(item, clause.expression.as_deref().unwrap_or_default())?;
                item.insert(clause.path.clone(), value);
            }
            UpdateClauseKind::Remove => {
                item.remove(&clause.path);
            }
            UpdateClauseKind::Add => {
                let delta = parse_update_value(clause.expression.as_deref().unwrap_or_default())?;
                match item.get(&clause.path) {
                    Some(Value::Set(existing)) if matches!(delta, Value::Set(_)) => {
                        item.insert(clause.path.clone(), union_sets(existing, &delta)?);
                    }
                    Some(current) => {
                        item.insert(clause.path.clone(), add_values(current, &delta)?);
                    }
                    None if matches!(delta, Value::Set(_)) => {
                        item.insert(clause.path.clone(), delta);
                    }
                    None => {
                        item.insert(
                            clause.path.clone(),
                            add_values(&Value::Number("0".to_string()), &delta)?,
                        );
                    }
                }
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

fn eval_set_expression(item: &Item, raw: &str) -> Result<Value, EngineError> {
    let trimmed = normalize_update_expression(raw);
    if let Some(args) = function_args(&trimmed, "if_not_exists") {
        let (field, value_raw) = split_two_args(args)?;
        let field = field.trim();
        if item.get(field).is_none_or(|value| *value == Value::Null) {
            return parse_update_value(value_raw.trim());
        }
        return Ok(item.get(field).cloned().unwrap_or(Value::Null));
    }
    if let Some(args) = function_args(&trimmed, "list_append") {
        let (left_raw, right_raw) = split_two_args(args)?;
        let left = eval_set_operand(item, left_raw.trim())?;
        let right = eval_set_operand(item, right_raw.trim())?;
        return append_lists(left, right);
    }
    parse_update_value(&trimmed)
}

fn normalize_update_expression(raw: &str) -> String {
    raw.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(" (", "(")
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(" ,", ",")
        .replace(", ", ",")
}

fn eval_set_operand(item: &Item, raw: &str) -> Result<Value, EngineError> {
    if is_identifier(raw) && item.contains_key(raw) {
        Ok(item.get(raw).cloned().unwrap_or(Value::Null))
    } else {
        parse_update_value(raw)
    }
}

fn append_lists(left: Value, right: Value) -> Result<Value, EngineError> {
    let Value::List(mut values) = left else {
        return Err(EngineError::Runtime(
            "list_append requires list".to_string(),
        ));
    };
    let Value::List(additions) = right else {
        return Err(EngineError::Runtime(
            "list_append requires list".to_string(),
        ));
    };
    values.extend(additions);
    Ok(Value::List(values))
}

fn union_sets(left: &[Value], right: &Value) -> Result<Value, EngineError> {
    let Value::Set(additions) = right else {
        return Err(EngineError::Runtime(
            "ADD set requires set value".to_string(),
        ));
    };
    let mut values = left.to_vec();
    for entry in additions {
        if !values.contains(entry) {
            values.push(entry.clone());
        }
    }
    Ok(Value::Set(values))
}

fn function_args<'a>(raw: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}(");
    raw.strip_prefix(&prefix)?.strip_suffix(')')
}

fn split_two_args(raw: &str) -> Result<(&str, &str), EngineError> {
    let mut depth = 0usize;
    for (index, ch) in raw.char_indices() {
        if ch == '(' || ch == '[' || ch == '{' {
            depth += 1;
        } else if ch == ')' || ch == ']' || ch == '}' {
            depth = depth.saturating_sub(1);
        } else if ch == ',' && depth == 0 {
            return Ok((raw[..index].trim(), raw[index + 1..].trim()));
        }
    }
    Err(EngineError::Runtime(format!(
        "expected two arguments in '{raw}'"
    )))
}

fn resolve_throughput(
    throughput: &Throughput,
    current_read: Option<&Value>,
    current_write: Option<&Value>,
) -> Throughput {
    Throughput {
        read: resolve_throughput_value(&throughput.read, current_read),
        write: resolve_throughput_value(&throughput.write, current_write),
    }
}

fn resolve_throughput_value(value: &Value, current: Option<&Value>) -> Value {
    match value {
        Value::String(star) if star == "*" => {
            current.cloned().unwrap_or(Value::Number("0".to_string()))
        }
        other => other.clone(),
    }
}

fn throughput_is_on_demand(throughput: &Throughput) -> bool {
    matches!(&throughput.read, Value::Number(value) if value == "0")
        && matches!(&throughput.write, Value::Number(value) if value == "0")
}

fn add_values(left: &Value, right: &Value) -> Result<Value, EngineError> {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => {
            let sum = left
                .parse::<f64>()
                .map_err(|err| EngineError::Runtime(err.to_string()))?
                + right
                    .parse::<f64>()
                    .map_err(|err| EngineError::Runtime(err.to_string()))?;
            Ok(Value::Number(sum.to_string()))
        }
        _ => Err(EngineError::Runtime("unsupported ADD update".to_string())),
    }
}

fn parse_update_value(raw: &str) -> Result<Value, EngineError> {
    let trimmed = raw.trim();
    if trimmed.starts_with('(') {
        return parse_value(trimmed).map_err(|err| EngineError::Runtime(err.to_string()));
    }
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
    if trimmed.starts_with('[') {
        return parse_value(trimmed).map_err(|err| EngineError::Runtime(err.to_string()));
    }
    Err(EngineError::Runtime(format!(
        "unsupported update value '{raw}'"
    )))
}

fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
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
