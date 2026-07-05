use crate::convert::keys_in_to_items;
use crate::json_util::json_value_to_item;
use crate::{
    BackendResponse, CapacityRecord, DynamoBackend, EngineError, Item, ReadOperation, ReadRequest,
    StatementResult,
};
use dql_expr::{render_condition, render_projection};
use dql_models::{plan_read, Operation, PlanError, PlanInput, QueryPlan, ReadKind, TableMeta};
use dql_parser::{
    parse_script, Condition, InsertForm, OrderBy, QueryOptions, Selection, Statement, UpdateExpr,
    Value,
};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub struct Engine<B: DynamoBackend> {
    backend: B,
    explain: bool,
    explain_log: Vec<String>,
    analyzing: bool,
    consumed_capacities: Vec<CapacityRecord>,
    allow_select_scan: bool,
}

impl<B: DynamoBackend> Engine<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            explain: false,
            explain_log: Vec::new(),
            analyzing: false,
            consumed_capacities: Vec::new(),
            allow_select_scan: false,
        }
    }

    pub fn with_allow_select_scan(mut self, allow_select_scan: bool) -> Self {
        self.allow_select_scan = allow_select_scan;
        self
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
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
            Statement::Insert { table, form } => self.insert(table, form),
            Statement::Delete {
                table,
                condition,
                options,
            } => self.delete(table, condition.as_ref(), options),
            Statement::Update {
                table,
                update,
                condition,
                options,
                returns,
                ..
            } => self.update(
                table,
                update,
                condition.as_ref(),
                options,
                returns.as_deref(),
            ),
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
            Statement::AlterTable { table, action } => self.alter_table(table, action),
            Statement::DumpSchema { tables } => self.dump_schema(tables.as_deref()),
            Statement::Load { file, table } => self.load(file, table),
            Statement::Explain(inner) => self.explain(inner),
            Statement::Analyze(inner) => self.analyze(inner),
        }
    }

    pub fn table_names(&self) -> Result<Vec<String>, EngineError> {
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

    fn insert(&mut self, table: &str, form: &InsertForm) -> Result<StatementResult, EngineError> {
        self.record("batch_write_item", table);
        let (columns, rows) = match form {
            InsertForm::Values { columns, rows } => (columns.as_slice(), rows.as_slice()),
            InsertForm::Keyword { rows } => {
                return self.insert_keyword_rows(table, rows);
            }
        };
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

    fn insert_keyword_rows(
        &mut self,
        table: &str,
        rows: &[Vec<(String, Value)>],
    ) -> Result<StatementResult, EngineError> {
        let mut items = Vec::new();
        for row in rows {
            let mut item = Item::new();
            for (column, value) in row {
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
        options: &QueryOptions,
    ) -> Result<StatementResult, EngineError> {
        if let Some(keys_in) = &options.keys_in {
            validate_mutation_keys_in(options)?;
            let keys = self.keys_in_items(table, keys_in)?;
            self.record("delete_item", table);
            let response = self.backend.delete_by_keys(table, &keys, condition)?;
            self.capture_capacity(response.capacity);
            return Ok(StatementResult::Affected(response.output));
        }
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

    fn update(
        &mut self,
        table: &str,
        update: &UpdateExpr,
        condition: Option<&Condition>,
        options: &QueryOptions,
        returns: Option<&str>,
    ) -> Result<StatementResult, EngineError> {
        let return_items = returns.is_some_and(|value| {
            value
                .split_whitespace()
                .any(|part| part.eq_ignore_ascii_case("NEW"))
        });
        if let Some(keys_in) = &options.keys_in {
            validate_mutation_keys_in(options)?;
            let keys = self.keys_in_items(table, keys_in)?;
            self.record("update_item", table);
            let response =
                self.backend
                    .update_by_keys(table, &keys, update, condition, return_items)?;
            self.capture_capacity(response.capacity.clone());
            return finalize_update_result(response);
        }
        if condition.is_some() {
            self.record_update_read(table, condition, options);
        } else {
            self.record("scan", table);
        }
        self.record("update_item", table);
        let response = self
            .backend
            .update_matching(table, update, condition, return_items)?;
        self.capture_capacity(response.capacity.clone());
        finalize_update_result(response)
    }

    fn scan(
        &mut self,
        table: &str,
        selection: &Selection,
        condition: Option<&Condition>,
        options: &QueryOptions,
    ) -> Result<StatementResult, EngineError> {
        if options.keys_in.is_some() {
            return self.execute_keys_in_read(table, selection, condition, options);
        }
        let plan =
            self.plan_read_operation(table, ReadKind::Scan, selection, condition, options)?;
        self.execute_read(table, &plan, selection, condition, options)
    }

    fn select(
        &mut self,
        table: &str,
        selection: &Selection,
        condition: Option<&Condition>,
        options: &QueryOptions,
    ) -> Result<StatementResult, EngineError> {
        if options.keys_in.is_some() {
            return self.execute_keys_in_read(table, selection, condition, options);
        }
        let plan =
            self.plan_read_operation(table, ReadKind::Select, selection, condition, options)?;
        self.execute_read(table, &plan, selection, condition, options)
    }

    fn execute_keys_in_read(
        &mut self,
        table: &str,
        selection: &Selection,
        condition: Option<&Condition>,
        options: &QueryOptions,
    ) -> Result<StatementResult, EngineError> {
        validate_read_keys_in(options, condition)?;
        let keys_in = options.keys_in.as_ref().expect("keys_in checked above");
        let keys = self.keys_in_items(table, keys_in)?;
        self.record("batch_get_item", table);
        let response = self
            .backend
            .batch_get_keys(table, &keys, options.consistent)?;
        let capacity = response.capacity.clone();
        self.capture_capacity(capacity);
        finalize_read_result(response, selection, options.order_by.as_ref())
    }

    fn keys_in_items(&self, table: &str, keys_in: &[Vec<Value>]) -> Result<Vec<Item>, EngineError> {
        let meta = self
            .backend
            .describe_table(table)?
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        keys_in_to_items(&meta, keys_in)
    }

    fn execute_read(
        &mut self,
        table: &str,
        plan: &QueryPlan,
        selection: &Selection,
        condition: Option<&Condition>,
        options: &QueryOptions,
    ) -> Result<StatementResult, EngineError> {
        self.record_read_operation(plan.operation, table);
        if plan.follow_up_batch_get {
            self.record("batch_get_item", table);
        }
        let request = ReadRequest {
            operation: operation_to_backend(plan.operation),
            index_name: plan.index.as_ref().map(|index| index.name.as_str()),
            key_condition: plan.key_condition.as_ref(),
            filter_condition: plan
                .filter_condition
                .as_ref()
                .or(if plan.key_condition.is_none() {
                    condition
                } else {
                    None
                }),
            condition,
            selection,
            options,
            consistent: options.consistent,
            order_by: options.order_by.as_ref(),
            follow_up_batch_get: plan.follow_up_batch_get,
        };
        let response = self.backend.execute_read(table, &request)?;
        let capacity = response.capacity.clone();
        self.capture_capacity(capacity);
        finalize_read_result(response, selection, options.order_by.as_ref())
    }

    fn alter_table(
        &mut self,
        table: &str,
        action: &dql_parser::AlterAction,
    ) -> Result<StatementResult, EngineError> {
        self.record("update_table", table);
        let response = self.backend.alter_table(table, action)?;
        self.capture_capacity(response.capacity);
        Ok(StatementResult::Status(response.output))
    }

    fn load(&mut self, file: &str, table: &str) -> Result<StatementResult, EngineError> {
        self.record("batch_write_item", table);
        let path = Path::new(file.trim_matches('"').trim_matches('\''));
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let reader = BufReader::new(
            File::open(path)
                .map_err(|err| EngineError::Runtime(format!("failed to open '{file}': {err}")))?,
        );
        let mut items = Vec::new();
        match extension.as_str() {
            "json" => {
                for line in reader.lines() {
                    let line = line.map_err(|err| EngineError::Runtime(err.to_string()))?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    let value: serde_json::Value = serde_json::from_str(&line)
                        .map_err(|err| EngineError::Runtime(err.to_string()))?;
                    items
                        .push(json_value_to_item(&value).map_err(|err| EngineError::Runtime(err))?);
                }
            }
            other => {
                return Err(EngineError::Runtime(format!(
                    "unsupported LOAD file format '{other}'"
                )));
            }
        }
        let response = self.backend.batch_write(table, items)?;
        self.capture_capacity(response.capacity);
        Ok(StatementResult::Affected(response.output))
    }

    fn dump_schema(&mut self, tables: Option<&[String]>) -> Result<StatementResult, EngineError> {
        self.record("dump_schema", "*");
        let names = match tables {
            Some(names) => names.to_vec(),
            None => self.table_names()?,
        };
        let mut lines = Vec::new();
        for name in names {
            let table = self
                .backend
                .describe_table(&name)?
                .ok_or_else(|| EngineError::Runtime(format!("Table '{name}' not found")))?;
            lines.push(table.schema_dql().trim_end_matches(';').to_string());
        }
        Ok(StatementResult::Schema(lines.join("\n\n")))
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

    fn record_update_read(
        &mut self,
        table: &str,
        condition: Option<&Condition>,
        options: &QueryOptions,
    ) {
        let operation = if condition.is_some() {
            self.plan_read_operation(table, ReadKind::Scan, &Selection::All, condition, options)
                .map(|plan| plan.operation)
                .unwrap_or(Operation::Scan)
        } else {
            Operation::Scan
        };
        self.record_read_operation(operation, table);
    }

    fn plan_read_operation(
        &self,
        table: &str,
        kind: ReadKind,
        selection: &Selection,
        condition: Option<&Condition>,
        options: &QueryOptions,
    ) -> Result<QueryPlan, EngineError> {
        let meta = self
            .backend
            .describe_table(table)?
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        if let Some(condition) = condition {
            let _ =
                render_condition(condition).map_err(|err| EngineError::Runtime(err.to_string()))?;
        }
        let _ = render_projection(selection);
        let allow_select_scan = match kind {
            ReadKind::Select => self.allow_select_scan,
            ReadKind::Scan => true,
        };
        plan_read(PlanInput {
            table: &meta,
            kind,
            condition,
            selection: Some(selection),
            using_index: options.using_index.as_deref(),
            allow_select_scan,
        })
        .map_err(plan_error_to_engine_error)
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

fn plan_error_to_engine_error(err: PlanError) -> EngineError {
    match err {
        PlanError::SelectScanRejected => EngineError::Runtime(
            "Cannot perform SELECT without an indexed WHERE clause. Use SCAN or specify USING -"
                .to_string(),
        ),
        PlanError::AmbiguousIndex(names) => {
            EngineError::Runtime(format!("Ambiguous index selection: {}", names.join(", ")))
        }
        PlanError::CannotScanLocalIndex(name) => {
            EngineError::Runtime(format!("Cannot scan local index '{name}'"))
        }
        PlanError::UnknownIndex(name) => EngineError::Runtime(format!("Unknown index '{name}'")),
    }
}

fn operation_to_backend(operation: Operation) -> ReadOperation {
    match operation {
        Operation::Query => ReadOperation::Query,
        Operation::Scan => ReadOperation::Scan,
        Operation::BatchGetKeys => ReadOperation::BatchGetKeys,
    }
}

fn validate_read_keys_in(
    options: &QueryOptions,
    condition: Option<&Condition>,
) -> Result<(), EngineError> {
    if options.limit.is_some() {
        return Err(EngineError::Runtime(
            "Cannot use LIMIT with KEYS IN".to_string(),
        ));
    }
    if options.using_index.is_some() {
        return Err(EngineError::Runtime(
            "Cannot use USING with KEYS IN".to_string(),
        ));
    }
    if options.order_by.is_some() {
        return Err(EngineError::Runtime(
            "Cannot use ORDER BY with KEYS IN".to_string(),
        ));
    }
    if condition.is_some() {
        return Err(EngineError::Runtime(
            "Cannot use WHERE with KEYS IN".to_string(),
        ));
    }
    Ok(())
}

fn validate_mutation_keys_in(options: &QueryOptions) -> Result<(), EngineError> {
    if options.using_index.is_some() {
        return Err(EngineError::Runtime(
            "Cannot use USING with KEYS IN".to_string(),
        ));
    }
    Ok(())
}

fn finalize_update_result(
    response: BackendResponse<usize>,
) -> Result<StatementResult, EngineError> {
    if let Some(items) = response.updated_items {
        Ok(StatementResult::Items(items))
    } else {
        Ok(StatementResult::Affected(response.output))
    }
}

fn finalize_read_result(
    mut response: BackendResponse<Vec<Item>>,
    selection: &Selection,
    order_by: Option<&OrderBy>,
) -> Result<StatementResult, EngineError> {
    if matches!(selection, Selection::CountAll) {
        let count = response.count.unwrap_or(response.output.len());
        return Ok(StatementResult::Affected(count));
    }
    if let Some(order_by) = order_by {
        sort_items(&mut response.output, order_by);
    }
    apply_projection(&mut response.output, selection);
    Ok(StatementResult::Items(response.output))
}

fn apply_projection(items: &mut [Item], selection: &Selection) {
    let Selection::Items(projections) = selection else {
        return;
    };
    let fields = projections
        .iter()
        .map(|item| item.expression.trim().to_string())
        .collect::<Vec<_>>();
    if fields.is_empty() {
        return;
    }
    for item in items.iter_mut() {
        item.retain(|key, _| fields.iter().any(|field| field == key));
    }
}

fn sort_items(items: &mut [Item], order_by: &OrderBy) {
    items.sort_by(|left, right| {
        let left_value = left.get(&order_by.field);
        let right_value = right.get(&order_by.field);
        let ordering = compare_sort_values(left_value, right_value);
        if order_by.descending {
            ordering.reverse()
        } else {
            ordering
        }
    });
}

fn compare_sort_values(left: Option<&Value>, right: Option<&Value>) -> std::cmp::Ordering {
    match (left, right) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (Some(left), Some(right)) => match (left, right) {
            (Value::Number(left), Value::Number(right)) => left
                .parse::<f64>()
                .unwrap_or(0.0)
                .partial_cmp(&right.parse::<f64>().unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal),
            (Value::String(left), Value::String(right)) => left.cmp(right),
            _ => std::cmp::Ordering::Equal,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryBackend;

    #[test]
    fn executes_create_insert_scan_script() {
        let mut engine = Engine::new(MemoryBackend::new());
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
        let mut engine = Engine::new(MemoryBackend::new());
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
        let mut engine = Engine::new(MemoryBackend::new());
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
        let mut engine = Engine::new(MemoryBackend::new());
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
        let mut engine = Engine::new(MemoryBackend::new());
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
    fn update_modifies_matching_items() {
        let mut engine = Engine::new(MemoryBackend::new());
        engine
            .execute(
                "CREATE TABLE t (id STRING HASH KEY, score NUMBER);
                 INSERT INTO t (id, score) VALUES ('a', 1), ('b', 2);
                 UPDATE t SET score = 3 WHERE id = 'b'",
            )
            .unwrap();
        let result = engine.execute("SCAN * FROM t WHERE id = 'b'").unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items[0].get("score"), Some(&Value::Number("3".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }
}
