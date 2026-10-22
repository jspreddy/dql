use crate::cancel::CancelFlag;
use crate::convert::keys_in_to_items;
use crate::file_io::{load_items as load_items_from_file, save_items, validate_save_path};
use crate::progress::ProgressSink;
use crate::query_context::{query_context_from_read, LastQueryContext};
use crate::table_cache::SharedTableCache;
use crate::throttle::RateLimit;
use crate::{
    help_page, BackendResponse, CapacityRecord, DynamoBackend, EngineError, Item, ReadOperation,
    ReadRequest, StatementResult, WRITE_PROGRESS_CHUNK,
};
use dql_expr::{project_selection, render_condition, render_projection, resolve_timestamp};
use dql_models::{plan_read, Operation, PlanError, PlanInput, QueryPlan, ReadKind, TableMeta};
use dql_parser::{
    parse_script, Condition, InsertForm, OrderBy, QueryOptions, Selection, Statement,
    ThrottleConfig, UpdateExpr, Value,
};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Engine<B: DynamoBackend> {
    backend: B,
    explain: bool,
    explain_log: Vec<String>,
    analyzing: bool,
    consumed_capacities: Vec<CapacityRecord>,
    statement_capacity: Vec<CapacityRecord>,
    allow_select_scan: bool,
    rate_limit: Option<RateLimit>,
    pub cached_descriptions: HashMap<String, TableMeta>,
    disk: Option<SharedTableCache>,
    cached_names: Option<Vec<String>>,
    names_saved_at_ms: Option<u64>,
    seen_generation: u64,
    names_were_cached: bool,
    last_read_from_cache: bool,
    last_query_context: Option<LastQueryContext>,
    progress: ProgressSink,
    cancel: CancelFlag,
}

impl<B: DynamoBackend> Engine<B> {
    pub fn new(mut backend: B) -> Self {
        let progress = ProgressSink::default();
        let cancel = CancelFlag::default();
        backend.set_progress_sink(progress.clone());
        backend.set_cancel_flag(cancel.clone());
        Self {
            backend,
            explain: false,
            explain_log: Vec::new(),
            analyzing: false,
            consumed_capacities: Vec::new(),
            statement_capacity: Vec::new(),
            allow_select_scan: false,
            rate_limit: None,
            cached_descriptions: HashMap::new(),
            disk: None,
            cached_names: None,
            names_saved_at_ms: None,
            seen_generation: 0,
            names_were_cached: false,
            last_read_from_cache: false,
            last_query_context: None,
            progress,
            cancel,
        }
    }

    pub fn set_rate_limit(&mut self, read_per_second: f64, write_per_second: f64) {
        self.rate_limit = Some(RateLimit::new(read_per_second, write_per_second));
    }

    pub fn clear_rate_limit(&mut self) {
        self.rate_limit = None;
    }

    pub fn set_rate_limit_option(&mut self, limit: Option<RateLimit>) {
        self.rate_limit = limit;
    }

    pub fn set_table_cache(&mut self, cache: Option<SharedTableCache>) {
        self.disk = cache;
        self.cached_names = None;
        self.names_saved_at_ms = None;
        self.seen_generation = 0;
        self.cached_descriptions.clear();
    }

    pub fn table_cache(&self) -> Option<&SharedTableCache> {
        self.disk.as_ref()
    }

    /// Drop the saved table list and every saved definition.
    pub fn clear_table_cache(&mut self) {
        if let Some(disk) = &self.disk {
            disk.clear();
            self.seen_generation = disk.generation();
        }
        self.cached_names = None;
        self.names_saved_at_ms = None;
        self.names_were_cached = false;
        self.last_read_from_cache = false;
        self.cached_descriptions.clear();
    }

    pub fn list_tables(&self) -> Result<Vec<String>, EngineError> {
        self.backend.list_tables()
    }

    /// Names from memory, then disk, then the backend.
    ///
    /// `refresh` skips memory and disk and stores the backend list.
    pub fn cached_table_names(&mut self, refresh: bool) -> Result<Vec<String>, EngineError> {
        self.note_disk_generation();
        if !refresh {
            if let Some(names) = &self.cached_names {
                self.names_were_cached = true;
                return Ok(names.clone());
            }
            if let Some(disk) = &self.disk {
                if let Some(cached) = disk.get_names() {
                    self.cached_names = Some(cached.names.clone());
                    self.names_saved_at_ms = Some(cached.saved_at_ms);
                    self.names_were_cached = true;
                    return Ok(cached.names);
                }
            }
        }
        let names = self.backend.list_tables()?;
        self.remember_names(&names);
        self.names_were_cached = false;
        Ok(names)
    }

    pub fn names_from_cache(&self) -> bool {
        self.names_were_cached
    }

    pub fn names_saved_at_ms(&self) -> Option<u64> {
        self.names_saved_at_ms
    }

    /// Definition already in memory or on disk. Does not call the backend.
    pub fn cached_definition(&mut self, table: &str) -> Option<TableMeta> {
        self.note_disk_generation();
        if let Some(meta) = self.cached_descriptions.get(table) {
            return Some(meta.clone());
        }
        let disk = self.disk.clone();
        if let Some(disk) = disk {
            if let Some(cached) = disk.get_definition(table) {
                self.cached_descriptions
                    .insert(table.to_string(), cached.meta.clone());
                return Some(cached.meta);
            }
        }
        None
    }

    /// Re-fetch definitions that are already cached. Does not describe anything else.
    pub fn refresh_cached_definitions(&mut self) -> Result<usize, EngineError> {
        self.note_disk_generation();
        let mut names: Vec<String> = self.cached_descriptions.keys().cloned().collect();
        if let Some(disk) = &self.disk {
            for name in disk.definition_names() {
                if !names.iter().any(|existing| existing == &name) {
                    names.push(name);
                }
            }
        }
        names.sort();
        let count = names.len();
        for name in names {
            match self.describe(&name, true)? {
                Some(_) => {}
                None => self.forget_definition(&name),
            }
        }
        Ok(count)
    }

    pub fn describe(
        &mut self,
        table: &str,
        refresh: bool,
    ) -> Result<Option<TableMeta>, EngineError> {
        if !refresh {
            if let Some(meta) = self.cached_definition(table) {
                self.last_read_from_cache = true;
                return Ok(Some(meta));
            }
        }
        let desc = self.backend.describe_table(table)?;
        if let Some(meta) = desc {
            self.store_definition(&meta);
            self.last_read_from_cache = false;
            Ok(Some(meta))
        } else {
            self.last_read_from_cache = false;
            Ok(None)
        }
    }

    pub fn last_read_from_cache(&self) -> bool {
        self.last_read_from_cache
    }

    fn note_disk_generation(&mut self) {
        let Some(disk) = self.disk.clone() else {
            return;
        };
        let generation = disk.generation();
        if generation != self.seen_generation {
            self.cached_descriptions.clear();
            self.cached_names = None;
            self.names_saved_at_ms = None;
            self.seen_generation = generation;
        }
    }

    fn remember_names(&mut self, names: &[String]) {
        self.cached_names = Some(names.to_vec());
        if let Some(disk) = &self.disk {
            self.names_saved_at_ms = Some(disk.put_names(names));
            self.seen_generation = disk.generation();
        } else {
            self.names_saved_at_ms = Some(now_ms());
        }
    }

    fn remember_created_name(&mut self, name: &str) {
        self.note_disk_generation();
        if self.cached_names.is_none() {
            if let Some(disk) = &self.disk {
                if let Some(cached) = disk.get_names() {
                    self.cached_names = Some(cached.names);
                    self.names_saved_at_ms = Some(cached.saved_at_ms);
                }
            }
        }
        let Some(names) = self.cached_names.clone() else {
            return;
        };
        if names.iter().any(|existing| existing == name) {
            return;
        }
        let mut names = names;
        names.push(name.to_string());
        names.sort();
        self.remember_names(&names);
    }

    fn forget_name(&mut self, name: &str) {
        self.note_disk_generation();
        if self.cached_names.is_none() {
            if let Some(disk) = &self.disk {
                if let Some(cached) = disk.get_names() {
                    self.cached_names = Some(cached.names);
                    self.names_saved_at_ms = Some(cached.saved_at_ms);
                }
            }
        }
        let Some(names) = self.cached_names.clone() else {
            return;
        };
        let names: Vec<String> = names
            .into_iter()
            .filter(|existing| existing != name)
            .collect();
        self.remember_names(&names);
    }

    fn store_definition(&mut self, meta: &TableMeta) {
        self.cached_descriptions
            .insert(meta.name.clone(), meta.clone());
        if let Some(disk) = &self.disk {
            disk.put_definition(meta);
            self.seen_generation = disk.generation();
        }
    }

    fn forget_definition(&mut self, name: &str) {
        self.cached_descriptions.remove(name);
        if let Some(disk) = &self.disk {
            disk.remove_definition(name);
            self.seen_generation = disk.generation();
        }
    }

    pub fn describe_all(&mut self, refresh: bool) -> Result<Vec<TableMeta>, EngineError> {
        let tables = self.backend.list_tables()?;
        let mut descs = Vec::new();
        for table in tables {
            if let Some(meta) = self.describe(&table, refresh)? {
                descs.push(meta);
            }
        }
        Ok(descs)
    }

    pub fn table_item_count(&self, table: &str) -> usize {
        self.backend.table_item_count(table)
    }

    pub fn with_allow_select_scan(mut self, allow_select_scan: bool) -> Self {
        self.allow_select_scan = allow_select_scan;
        self
    }

    pub fn set_allow_select_scan(&mut self, allow_select_scan: bool) {
        self.allow_select_scan = allow_select_scan;
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn last_query_context(&self) -> Option<&LastQueryContext> {
        self.last_query_context.as_ref()
    }

    pub fn progress(&self) -> &ProgressSink {
        &self.progress
    }

    pub fn cancel(&self) -> &CancelFlag {
        &self.cancel
    }

    fn report_progress(&self, done: u64, total: Option<u64>, phase: &str) {
        self.progress.report(done, total, phase);
    }

    fn write_items(
        &mut self,
        table: &str,
        items: Vec<Item>,
    ) -> Result<BackendResponse<usize>, EngineError> {
        let total = items.len();
        self.report_progress(0, Some(total as u64), "write");
        if items.is_empty() {
            return Ok(BackendResponse {
                output: 0,
                capacity: None,
                count: None,
                updated_items: None,
            });
        }
        let mut written = 0usize;
        let mut last_capacity = None;
        for chunk in items.chunks(WRITE_PROGRESS_CHUNK) {
            self.cancel.check()?;
            let response = self.backend.batch_write(table, chunk.to_vec())?;
            written += response.output;
            if let Some(capacity) = response.capacity {
                self.capture_capacity(Some(capacity.clone()))?;
                last_capacity = Some(capacity);
            }
            self.report_progress(written as u64, Some(total as u64), "write");
        }
        Ok(BackendResponse {
            output: written,
            capacity: last_capacity,
            count: None,
            updated_items: None,
        })
    }

    fn delete_key_chunks(
        &mut self,
        table: &str,
        keys: &[Item],
        condition: Option<&Condition>,
    ) -> Result<BackendResponse<usize>, EngineError> {
        let total = keys.len();
        self.report_progress(0, Some(total as u64), "write");
        if keys.is_empty() {
            return Ok(BackendResponse {
                output: 0,
                capacity: None,
                count: None,
                updated_items: None,
            });
        }
        let mut deleted = 0usize;
        let mut last_capacity = None;
        for chunk in keys.chunks(WRITE_PROGRESS_CHUNK) {
            self.cancel.check()?;
            let response = self.backend.delete_by_keys(table, chunk, condition)?;
            deleted += response.output;
            if let Some(capacity) = response.capacity {
                self.capture_capacity(Some(capacity.clone()))?;
                last_capacity = Some(capacity);
            }
            self.report_progress(deleted as u64, Some(total as u64), "write");
        }
        Ok(BackendResponse {
            output: deleted,
            capacity: last_capacity,
            count: None,
            updated_items: None,
        })
    }

    fn update_key_chunks(
        &mut self,
        table: &str,
        keys: &[Item],
        update: &UpdateExpr,
        condition: Option<&Condition>,
        return_items: bool,
    ) -> Result<BackendResponse<usize>, EngineError> {
        let total = keys.len();
        self.report_progress(0, Some(total as u64), "write");
        if keys.is_empty() {
            return Ok(BackendResponse {
                output: 0,
                capacity: None,
                count: None,
                updated_items: None,
            });
        }
        let mut updated = 0usize;
        let mut last_capacity = None;
        let mut updated_items = Vec::new();
        for chunk in keys.chunks(WRITE_PROGRESS_CHUNK) {
            self.cancel.check()?;
            let response =
                self.backend
                    .update_by_keys(table, chunk, update, condition, return_items)?;
            updated += response.output;
            if let Some(items) = response.updated_items {
                updated_items.extend(items);
            }
            if let Some(capacity) = response.capacity {
                self.capture_capacity(Some(capacity.clone()))?;
                last_capacity = Some(capacity);
            }
            self.report_progress(updated as u64, Some(total as u64), "write");
        }
        Ok(BackendResponse {
            output: updated,
            capacity: last_capacity,
            count: None,
            updated_items: if return_items {
                Some(updated_items)
            } else {
                None
            },
        })
    }

    pub fn execute(&mut self, input: &str) -> Result<StatementResult, EngineError> {
        let statements = parse_script(input)?;
        let mut last = StatementResult::None;
        self.consumed_capacities.clear();
        for statement in &statements {
            self.cancel.check()?;
            last = self.run(statement)?;
        }
        Ok(last)
    }

    pub fn run(&mut self, statement: &Statement) -> Result<StatementResult, EngineError> {
        self.statement_capacity.clear();
        self.cancel.check()?;
        match statement {
            Statement::CreateTable { if_not_exists, .. } => {
                self.create_table(statement, *if_not_exists)
            }
            Statement::DropTable { if_exists, name } => self.drop_table(*if_exists, name),
            Statement::Insert {
                table,
                form,
                throttle,
            } => {
                self.apply_throttle_config(throttle.as_ref());
                self.insert(table, form)
            }
            Statement::Delete {
                table,
                condition,
                options,
            } => {
                self.apply_throttle_config(options.throttle.as_ref());
                self.delete(table, condition.as_ref(), options)
            }
            Statement::Update {
                table,
                update,
                condition,
                options,
                returns,
                ..
            } => {
                self.apply_throttle_config(options.throttle.as_ref());
                self.update(
                    table,
                    update,
                    condition.as_ref(),
                    options,
                    returns.as_deref(),
                )
            }
            Statement::Scan {
                table,
                selection,
                condition,
                options,
                ..
            } => {
                self.apply_throttle_config(options.throttle.as_ref());
                self.scan(table, selection, condition.as_ref(), options)
            }
            Statement::Select {
                table,
                selection,
                condition,
                options,
                ..
            } => {
                self.apply_throttle_config(options.throttle.as_ref());
                self.select(table, selection, condition.as_ref(), options)
            }
            Statement::AlterTable { table, action } => self.alter_table(table, action),
            Statement::DumpSchema { tables } => self.dump_schema(tables.as_deref()),
            Statement::ShowTables { like } => self.show_tables(like.as_deref()),
            Statement::Describe { table } => self.describe_statement(table),
            Statement::Load { file, table } => self.load(file, table),
            Statement::Explain(inner) => self.explain(inner),
            Statement::Analyze(inner) => self.analyze(inner),
            Statement::Help { topic } => help_page(topic.as_deref())
                .map(StatementResult::Help)
                .map_err(EngineError::Runtime),
        }
    }

    pub fn table_names(&self) -> Result<Vec<String>, EngineError> {
        self.backend.list_tables()
    }

    pub fn consumed_capacities(&self) -> &[CapacityRecord] {
        &self.consumed_capacities
    }

    /// Capacity captured for the statement that just ran.
    pub fn statement_capacity(&self) -> &[CapacityRecord] {
        &self.statement_capacity
    }

    fn create_table(
        &mut self,
        statement: &Statement,
        if_not_exists: bool,
    ) -> Result<StatementResult, EngineError> {
        let meta = TableMeta::from_create_statement(statement)
            .map_err(|err| EngineError::Runtime(err.to_string()))?;
        self.record("create_table", &meta.name);
        let response = self.backend.create_table(meta.clone(), if_not_exists)?;
        self.capture_capacity(response.capacity)?;
        self.store_definition(&meta);
        self.remember_created_name(&meta.name);
        Ok(StatementResult::Status(response.output))
    }

    fn drop_table(&mut self, if_exists: bool, name: &str) -> Result<StatementResult, EngineError> {
        self.record("delete_table", name);
        let response = self.backend.delete_table(name, if_exists)?;
        self.capture_capacity(response.capacity)?;
        self.forget_definition(name);
        self.forget_name(name);
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
                item.insert(column.clone(), normalize_insert_value(value.clone()));
            }
            items.push(item);
        }
        let response = self.write_items(table, items)?;
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
                item.insert(column.clone(), normalize_insert_value(value.clone()));
            }
            items.push(item);
        }
        let response = self.write_items(table, items)?;
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
            let response = self.delete_key_chunks(table, &keys, condition)?;
            return Ok(StatementResult::Affected(response.output));
        }
        let plan = if condition.is_some() || options.using_index.is_some() {
            Some(self.plan_read_operation(
                table,
                ReadKind::Scan,
                &Selection::All,
                condition,
                options,
            )?)
        } else {
            None
        };
        if let Some(plan) = &plan {
            self.record_read_operation(plan.operation, table);
        } else if condition.is_some() {
            self.record("query", table);
        } else {
            self.record("scan", table);
        }
        self.record("delete_item", table);
        self.report_progress(0, None, "write");
        let response = self
            .backend
            .delete_matching(table, condition, plan.as_ref(), options)?;
        self.capture_capacity(response.capacity)?;
        self.report_progress(
            response.output as u64,
            Some(response.output as u64),
            "write",
        );
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
            let response = self.update_key_chunks(table, &keys, update, condition, return_items)?;
            return finalize_update_result(response);
        }
        if condition.is_some() {
            self.record_update_read(table, condition, options);
        } else {
            self.record("scan", table);
        }
        self.record("update_item", table);
        self.report_progress(0, None, "write");
        let response = self
            .backend
            .update_matching(table, update, condition, return_items)?;
        self.capture_capacity(response.capacity.clone())?;
        self.report_progress(
            response.output as u64,
            Some(response.output as u64),
            "write",
        );
        finalize_update_result(response)
    }

    fn scan(
        &mut self,
        table: &str,
        selection: &Selection,
        condition: Option<&Condition>,
        options: &QueryOptions,
    ) -> Result<StatementResult, EngineError> {
        self.ensure_save_path(options)?;
        let result = if options.keys_in.is_some() {
            self.capture_query_context(table, selection, options, None)?;
            self.execute_keys_in_read(table, selection, condition, options)?
        } else {
            let plan =
                self.plan_read_operation(table, ReadKind::Scan, selection, condition, options)?;
            self.capture_query_context(table, selection, options, Some(&plan))?;
            self.execute_read(table, &plan, selection, condition, options)?
        };
        self.maybe_save(options, result)
    }

    fn select(
        &mut self,
        table: &str,
        selection: &Selection,
        condition: Option<&Condition>,
        options: &QueryOptions,
    ) -> Result<StatementResult, EngineError> {
        self.ensure_save_path(options)?;
        let result = if options.keys_in.is_some() {
            self.capture_query_context(table, selection, options, None)?;
            self.execute_keys_in_read(table, selection, condition, options)?
        } else {
            let plan =
                self.plan_read_operation(table, ReadKind::Select, selection, condition, options)?;
            self.capture_query_context(table, selection, options, Some(&plan))?;
            self.execute_read(table, &plan, selection, condition, options)?
        };
        self.maybe_save(options, result)
    }

    fn capture_query_context(
        &mut self,
        table: &str,
        selection: &Selection,
        options: &QueryOptions,
        plan: Option<&QueryPlan>,
    ) -> Result<(), EngineError> {
        let meta = self
            .describe(table, false)?
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let index = plan.and_then(|plan| plan.index.clone()).or_else(|| {
            options.using_index.as_ref().and_then(|name| {
                if name == "-" {
                    None
                } else {
                    meta.get_index(name).ok()
                }
            })
        });
        let follow_up_batch_get = plan.is_some_and(|plan| plan.follow_up_batch_get);
        self.last_query_context = Some(query_context_from_read(
            meta,
            selection,
            index,
            follow_up_batch_get,
        ));
        Ok(())
    }

    fn ensure_save_path(&self, options: &QueryOptions) -> Result<(), EngineError> {
        let Some(filename) = options.save_file.as_deref() else {
            return Ok(());
        };
        validate_save_path(save_filename(filename), options.save_create_parents)
            .map_err(EngineError::Runtime)
    }

    fn maybe_save(
        &self,
        options: &QueryOptions,
        result: StatementResult,
    ) -> Result<StatementResult, EngineError> {
        let Some(filename) = options.save_file.as_deref() else {
            return Ok(result);
        };
        let StatementResult::Items(items) = result else {
            return Err(EngineError::Runtime(
                "Cannot use count(*) with SAVE".to_string(),
            ));
        };
        let count = save_items(save_filename(filename), &items, options.save_create_parents)
            .map_err(|err| EngineError::Runtime(err.to_string()))?;
        Ok(StatementResult::Affected(count))
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
        self.capture_capacity(capacity)?;
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
        let ordering = resolve_ordering(plan, options)?;
        let mut kwargs = explain_kwargs_for_plan(plan, condition, options);
        if let Some(forward) = ordering.scan_index_forward {
            kwargs.insert("desc".to_string(), (!forward).to_string());
            kwargs.insert("scan_index_forward".to_string(), forward.to_string());
        }
        let operation = match plan.operation {
            Operation::Query => "query",
            Operation::Scan => "scan",
            Operation::BatchGetKeys => "batch_get_item",
        };
        self.record_with_kwargs(operation, table, &kwargs);
        if plan.follow_up_batch_get {
            self.record("batch_get_item", table);
        }
        let range_key = plan
            .index
            .as_ref()
            .and_then(|index| index.range_key.as_deref());
        let request = ReadRequest {
            operation: operation_to_backend(plan.operation),
            index_name: plan.index.as_ref().and_then(|index| {
                if index.name == "TABLE" {
                    None
                } else {
                    Some(index.name.as_str())
                }
            }),
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
            scan_index_forward: ordering.scan_index_forward,
            range_key,
            follow_up_batch_get: plan.follow_up_batch_get,
        };
        let response = self.backend.execute_read(table, &request)?;
        let capacity = response.capacity.clone();
        self.capture_capacity(capacity)?;
        finalize_read_result(response, selection, ordering.client_order_by.as_ref())
    }

    fn alter_table(
        &mut self,
        table: &str,
        action: &dql_parser::AlterAction,
    ) -> Result<StatementResult, EngineError> {
        self.record("update_table", table);
        let response = self.backend.alter_table(table, action)?;
        self.capture_capacity(response.capacity)?;
        Ok(StatementResult::Status(response.output))
    }

    fn load(&mut self, file: &str, table: &str) -> Result<StatementResult, EngineError> {
        self.record("batch_write_item", table);
        let path = Path::new(file.trim_matches('"').trim_matches('\''));
        let items =
            load_items_from_file(path).map_err(|err| EngineError::Runtime(err.to_string()))?;
        let response = self.write_items(table, items)?;
        Ok(StatementResult::Affected(response.output))
    }

    fn show_tables(&mut self, like: Option<&str>) -> Result<StatementResult, EngineError> {
        self.record("list_tables", like.unwrap_or("*"));
        let mut names = self.cached_table_names(false)?;
        let Some(pattern) = like else {
            names.sort();
            // An empty note marks the full list. The CLI turns it into the same
            // summary `ls` prints; the items stay the name rows for JSON.
            return Ok(StatementResult::ItemsWithNote {
                items: name_items(&names),
                note: String::new(),
            });
        };
        let exact: Vec<String> = names
            .iter()
            .filter(|name| sql_like(name, pattern))
            .cloned()
            .collect();
        if !exact.is_empty() {
            let mut exact = exact;
            exact.sort();
            return Ok(StatementResult::Items(name_items(&exact)));
        }
        let metas: Vec<TableMeta> = names
            .iter()
            .filter_map(|name| self.cached_definition(name))
            .collect();
        let tables: Vec<_> = metas.iter().map(crate::MatchTable::from_meta).collect();
        match crate::intelligent_like_matches(&tables, pattern) {
            Ok(hit) => Ok(StatementResult::ItemsWithNote {
                items: name_items(&hit.names),
                note: hit.note,
            }),
            Err(_) => Ok(StatementResult::Items(Vec::new())),
        }
    }

    fn describe_statement(&mut self, table: &str) -> Result<StatementResult, EngineError> {
        self.record("describe_table", table);
        let meta = self
            .describe(table, false)?
            .ok_or_else(|| EngineError::Runtime(format!("Table {table:?} not found")))?;
        let text = crate::format_table_description(&meta, self.table_item_count(table), 0);
        Ok(StatementResult::Schema(text))
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
        self.record_with_kwargs(operation, target, &BTreeMap::new());
    }

    fn record_with_kwargs(
        &mut self,
        operation: &str,
        target: &str,
        kwargs: &BTreeMap<String, String>,
    ) {
        if self.explain {
            if kwargs.is_empty() {
                self.explain_log.push(format!("{operation} {target}"));
            } else {
                self.explain_log.push(format!(
                    "{operation} {target} {}",
                    format_explain_kwargs(kwargs)
                ));
            }
        }
    }

    fn apply_throttle_config(&mut self, throttle: Option<&ThrottleConfig>) {
        self.rate_limit =
            throttle.map(|config| RateLimit::new(config.read_per_second, config.write_per_second));
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

    fn capture_capacity(&mut self, capacity: Option<CapacityRecord>) -> Result<(), EngineError> {
        let Some(capacity) = capacity else {
            return Ok(());
        };
        self.statement_capacity.push(capacity.clone());
        if self.analyzing {
            self.consumed_capacities.push(capacity.clone());
        }
        if let Some(rate_limit) = self.rate_limit.as_mut() {
            let wait = rate_limit.on_capacity(capacity.read_units, capacity.write_units);
            self.cancel.sleep(wait)?;
        }
        Ok(())
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn save_filename(filename: &str) -> &Path {
    Path::new(filename.trim_matches('"').trim_matches('\''))
}

fn format_explain_kwargs(kwargs: &BTreeMap<String, String>) -> String {
    let parts = kwargs
        .iter()
        .map(|(key, value)| format!("'{key}': {value:?}"))
        .collect::<Vec<_>>();
    format!("{{{}}}", parts.join(", "))
}

fn explain_kwargs_for_plan(
    plan: &QueryPlan,
    condition: Option<&Condition>,
    options: &QueryOptions,
) -> BTreeMap<String, String> {
    let mut kwargs = BTreeMap::new();
    if let Some(index) = &plan.index {
        kwargs.insert("index".to_string(), index.name.clone());
    }
    if let Some(key_condition) = &plan.key_condition {
        if let Ok(rendered) = render_condition(key_condition) {
            kwargs.insert("key_condition".to_string(), rendered.expression);
        }
    }
    let filter = plan
        .filter_condition
        .as_ref()
        .or(if plan.key_condition.is_some() {
            condition
        } else {
            None
        });
    if let Some(filter) = filter {
        if let Ok(rendered) = render_condition(filter) {
            kwargs.insert("filter".to_string(), rendered.expression);
        }
    }
    if options.consistent {
        kwargs.insert("consistent".to_string(), "true".to_string());
    }
    kwargs
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct OrderingPolicy {
    /// `Some(true)` → ScanIndexForward true (ASC); `Some(false)` → DESC.
    scan_index_forward: Option<bool>,
    client_order_by: Option<OrderBy>,
}

/// Mirror Python `order()` / `_select` ASC/DESC and ORDER BY handling.
fn resolve_ordering(
    plan: &QueryPlan,
    options: &QueryOptions,
) -> Result<OrderingPolicy, EngineError> {
    // Bare ASC/DESC (or ORDER BY … ASC|DESC) sets `options.descending`.
    if options.descending.is_some()
        && options.order_by.is_none()
        && plan.operation == Operation::Scan
    {
        return Err(EngineError::Runtime(
            "No index found for query, cannot use ASC or DESC without ORDER BY <field>".to_string(),
        ));
    }

    let reverse = options.descending == Some(true);
    let range_key = plan
        .index
        .as_ref()
        .and_then(|index| index.range_key.as_deref());
    let order_field = options.order_by.as_ref().map(|order| order.field.as_str());

    let scan_index_forward = if plan.operation == Operation::Query && options.descending.is_some() {
        if order_field.is_none() || order_field == range_key {
            Some(!reverse)
        } else {
            None
        }
    } else {
        None
    };

    let client_order_by = match &options.order_by {
        Some(order_by) if plan.index.is_none() || range_key != Some(order_by.field.as_str()) => {
            Some(OrderBy {
                field: order_by.field.clone(),
                descending: reverse || order_by.descending,
            })
        }
        _ => None,
    };

    Ok(OrderingPolicy {
        scan_index_forward,
        client_order_by,
    })
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

fn normalize_insert_value(value: Value) -> Value {
    match value {
        Value::Timestamp(expr) => Value::Number(resolve_timestamp(&expr).to_string()),
        other => other,
    }
}

fn apply_projection(items: &mut [Item], selection: &Selection) {
    let Selection::Items(projections) = selection else {
        return;
    };
    let simple = projections.iter().all(|item| {
        item.expression
            .chars()
            .all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '-')
    });
    if simple {
        for item in items.iter_mut() {
            let mut projected = Item::new();
            for entry in projections {
                if let Some(value) = item.get(entry.expression.trim()) {
                    let key = entry
                        .alias
                        .clone()
                        .unwrap_or_else(|| entry.expression.trim().to_string());
                    projected.insert(key, value.clone());
                }
            }
            *item = projected;
        }
        return;
    }
    for item in items.iter_mut() {
        *item = project_selection(item, selection);
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

fn name_items(names: &[String]) -> Vec<Item> {
    names
        .iter()
        .map(|name| {
            let mut item = Item::new();
            item.insert("name".to_string(), Value::String(name.clone()));
            item
        })
        .collect()
}

/// SQL `LIKE`: `%` is any sequence, `_` is one character. Matching is case-sensitive.
fn sql_like(text: &str, pattern: &str) -> bool {
    fn matches(text: &[char], pattern: &[char]) -> bool {
        let mut text_index = 0;
        let mut pattern_index = 0;
        let mut wildcard_pattern: Option<usize> = None;
        let mut wildcard_text = 0;
        while text_index < text.len() {
            if pattern_index < pattern.len()
                && (pattern[pattern_index] == '_' || pattern[pattern_index] == text[text_index])
            {
                text_index += 1;
                pattern_index += 1;
                continue;
            }
            if pattern_index < pattern.len() && pattern[pattern_index] == '%' {
                wildcard_pattern = Some(pattern_index);
                wildcard_text = text_index;
                pattern_index += 1;
                continue;
            }
            if let Some(saved) = wildcard_pattern {
                wildcard_text += 1;
                text_index = wildcard_text;
                pattern_index = saved + 1;
                continue;
            }
            return false;
        }
        while pattern_index < pattern.len() && pattern[pattern_index] == '%' {
            pattern_index += 1;
        }
        pattern_index == pattern.len()
    }

    matches(
        &text.chars().collect::<Vec<_>>(),
        &pattern.chars().collect::<Vec<_>>(),
    )
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
    fn scan_populates_last_query_context() {
        let mut engine = Engine::new(MemoryBackend::new());
        engine
            .execute(
                "CREATE TABLE t (id STRING HASH KEY, score NUMBER);
                 INSERT INTO t (id, score) VALUES ('a', 1);
                 SCAN id, score FROM t",
            )
            .unwrap();
        let context = engine
            .last_query_context()
            .expect("scan should populate query context");
        assert_eq!(context.table.name, "t");
        assert_eq!(context.important_columns(), vec!["id"]);
        assert!(context.preserve_column_order());
    }

    #[test]
    fn show_tables_like_filters_names() {
        let mut engine = Engine::new(MemoryBackend::new());
        engine
            .execute(
                "CREATE TABLE alpha_show (id STRING HASH KEY);
                 CREATE TABLE alpha_other (id STRING HASH KEY);
                 CREATE TABLE beta_show (id STRING HASH KEY)",
            )
            .unwrap();
        match engine.execute("SHOW TABLES LIKE 'alpha%'").unwrap() {
            StatementResult::Items(items) => {
                let names: Vec<_> = items
                    .iter()
                    .map(|item| match item.get("name") {
                        Some(Value::String(name)) => name.as_str(),
                        other => panic!("unexpected name: {other:?}"),
                    })
                    .collect();
                assert_eq!(names, vec!["alpha_other", "alpha_show"]);
            }
            other => panic!("unexpected result: {other:?}"),
        }
        match engine.execute("SHOW TABLES LIKE 'alpha_show'").unwrap() {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(
                    items[0].get("name"),
                    Some(&Value::String("alpha_show".to_string()))
                );
            }
            other => panic!("unexpected result: {other:?}"),
        }
        match engine.execute("SHOW TABLES").unwrap() {
            StatementResult::ItemsWithNote { items, note } => {
                assert_eq!(items.len(), 3);
                assert!(note.is_empty(), "{note}");
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn show_tables_like_falls_back_to_similar_names_and_related_keys() {
        let mut engine = Engine::new(MemoryBackend::new());
        engine
            .execute(
                "CREATE TABLE posts_v2 (id STRING HASH KEY);
                 CREATE TABLE nb_posts (id STRING HASH KEY);
                 CREATE TABLE zzposts (id STRING HASH KEY);
                 CREATE TABLE orders (customer_id STRING HASH KEY);
                 CREATE TABLE invoices (customer_id STRING HASH KEY);
                 CREATE TABLE gamma (id STRING HASH KEY)",
            )
            .unwrap();

        match engine.execute("SHOW TABLES LIKE 'posts_v2'").unwrap() {
            StatementResult::Items(items) => {
                assert_eq!(item_names(&items), vec!["posts_v2".to_string()]);
            }
            other => panic!("exact name should stay a plain list: {other:?}"),
        }
        match engine.execute("SHOW TABLES LIKE 'post%'").unwrap() {
            StatementResult::Items(items) => {
                assert_eq!(item_names(&items), vec!["posts_v2".to_string()]);
            }
            other => panic!("a LIKE hit should stay a plain list: {other:?}"),
        }
        match engine.execute("SHOW TABLES").unwrap() {
            StatementResult::ItemsWithNote { items, note } => {
                assert_eq!(items.len(), 6);
                assert!(note.is_empty(), "{note}");
            }
            other => panic!("unexpected result: {other:?}"),
        }
        match engine.execute("SHOW TABLES LIKE 'post'").unwrap() {
            StatementResult::ItemsWithNote { items, note } => {
                assert_eq!(
                    item_names(&items),
                    vec![
                        "posts_v2".to_string(),
                        "nb_posts".to_string(),
                        "zzposts".to_string()
                    ]
                );
                assert_eq!(
                    note,
                    "No exact match for \"post\", so showing similar names."
                );
            }
            other => panic!("unexpected result: {other:?}"),
        }
        let mut percent = Engine::new(MemoryBackend::new());
        percent
            .execute(
                "CREATE TABLE zzposts (id STRING HASH KEY);
                 CREATE TABLE gamma (id STRING HASH KEY)",
            )
            .unwrap();
        match percent.execute("SHOW TABLES LIKE 'post%'").unwrap() {
            StatementResult::ItemsWithNote { items, note } => {
                assert_eq!(item_names(&items), vec!["zzposts".to_string()]);
                assert_eq!(
                    note,
                    "No exact match for \"post%\", so showing similar names."
                );
            }
            other => panic!("unexpected result: {other:?}"),
        }
        match engine.execute("SHOW TABLES LIKE 'customer_id'").unwrap() {
            StatementResult::ItemsWithNote { items, note } => {
                assert_eq!(
                    item_names(&items),
                    vec!["orders".to_string(), "invoices".to_string()]
                );
                assert_eq!(
                    note,
                    "No exact match for \"customer_id\", so showing related keys."
                );
            }
            other => panic!("unexpected result: {other:?}"),
        }
        match engine.execute("SHOW TABLES LIKE 'qqqxxyyzz'").unwrap() {
            StatementResult::Items(items) => assert!(items.is_empty()),
            other => panic!("an unmatched pattern stays empty: {other:?}"),
        }
    }

    fn item_names(items: &[Item]) -> Vec<String> {
        items
            .iter()
            .map(|item| match item.get("name") {
                Some(Value::String(name)) => name.clone(),
                other => panic!("unexpected name: {other:?}"),
            })
            .collect()
    }

    #[test]
    fn describe_prints_one_table() {
        let mut engine = Engine::new(MemoryBackend::new());
        engine
            .execute(
                "CREATE TABLE alpha_desc (id STRING HASH KEY, n NUMBER RANGE KEY);
                 CREATE TABLE beta_desc (id STRING HASH KEY)",
            )
            .unwrap();
        match engine.execute("DESCRIBE alpha_desc").unwrap() {
            StatementResult::Schema(text) => {
                assert!(text.contains("Name: alpha_desc"), "{text}");
                assert!(text.contains("Hash Key: id (STRING)"), "{text}");
                assert!(text.contains("Range Key: n (NUMBER)"), "{text}");
                assert!(text.contains("CREATE TABLE alpha_desc"), "{text}");
                assert!(!text.contains("beta_desc"), "{text}");
            }
            other => panic!("unexpected result: {other:?}"),
        }
        let missing = engine.execute("DESCRIBE missing").unwrap_err();
        assert!(
            missing.to_string().contains("not found"),
            "unexpected error: {missing}"
        );
    }

    #[test]
    fn sql_like_matches_percent_and_underscore() {
        assert!(sql_like("abc", "abc"));
        assert!(sql_like("abc", "a%"));
        assert!(sql_like("abc", "%c"));
        assert!(sql_like("abc", "%b%"));
        assert!(sql_like("abc", "a_c"));
        assert!(sql_like("axxb", "a%b"));
        assert!(sql_like("", "%"));
        assert!(sql_like("", ""));
        assert!(!sql_like("abc", "a_"));
        assert!(!sql_like("abc", "x"));
        assert!(!sql_like("a", ""));
        assert!(!sql_like("Foo", "foo"));
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
        assert!(engine.consumed_capacities()[0].write_units >= 0.0);
        assert_eq!(engine.statement_capacity().len(), 1);
        assert_eq!(engine.statement_capacity()[0].operation, "batch_write_item");
    }

    #[test]
    fn statement_capacity_follows_the_latest_statement() {
        let mut engine = Engine::new(MemoryBackend::new());
        engine
            .execute("CREATE TABLE t (id STRING HASH KEY)")
            .unwrap();
        assert!(!engine.statement_capacity().is_empty());
        engine.execute("INSERT INTO t (id) VALUES ('a')").unwrap();
        let write_units: f64 = engine
            .statement_capacity()
            .iter()
            .map(|record| record.write_units)
            .sum();
        assert!(write_units >= 1.0);
        engine.execute("SELECT * FROM t WHERE id = 'a'").unwrap();
        assert!(engine
            .statement_capacity()
            .iter()
            .any(|record| record.operation == "query"));
        assert_eq!(engine.consumed_capacities().len(), 0);
    }

    #[test]
    fn throttle_clause_limits_capacity_consumption() {
        let mut engine = Engine::new(MemoryBackend::new());
        engine
            .execute("CREATE TABLE t (id STRING HASH KEY)")
            .unwrap();
        let start = std::time::Instant::now();
        engine
            .execute("INSERT INTO t (id) VALUES ('a'), ('b') THROTTLE 1 1")
            .unwrap();
        assert!(start.elapsed() >= std::time::Duration::from_millis(900));
    }

    #[test]
    fn explain_includes_query_kwargs() {
        let mut engine = Engine::new(MemoryBackend::new());
        engine
            .execute("CREATE TABLE t (id STRING HASH KEY, ts NUMBER INDEX('ts-index'))")
            .unwrap();
        let result = engine
            .execute("EXPLAIN SELECT * FROM t WHERE id = 'a' AND ts < 150")
            .unwrap();
        match result {
            StatementResult::Schema(schema) => {
                assert!(schema.contains("query t"));
                assert!(schema.contains("index"));
                assert!(schema.contains("key_condition"));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn load_csv_writes_items() {
        let dir = std::env::temp_dir().join("dql_engine_load_csv");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("items.csv");
        std::fs::write(&path, "id,foo\na,1\nb,2\n").unwrap();

        let mut engine = Engine::new(MemoryBackend::new());
        engine
            .execute("CREATE TABLE destination (id STRING HASH KEY)")
            .unwrap();
        engine
            .execute(&format!("LOAD '{}' INTO destination", path.display()))
            .unwrap();
        let result = engine.execute("SCAN * FROM destination").unwrap();
        match result {
            StatementResult::Items(items) => {
                assert_eq!(items.len(), 2);
                let a = items
                    .iter()
                    .find(|item| item.get("id") == Some(&Value::String("a".to_string())))
                    .unwrap();
                assert_eq!(a.get("foo"), Some(&Value::Number("1".to_string())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn save_and_load_json_csv_gzip_round_trip() {
        let dir = std::env::temp_dir().join("dql_engine_save_load");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut engine = Engine::new(MemoryBackend::new());
        engine
            .execute(
                "CREATE TABLE foobar (id STRING HASH KEY);
                 INSERT INTO foobar (id, foo) VALUES ('a', 1), ('b', 2);
                 CREATE TABLE destination (id STRING HASH KEY)",
            )
            .unwrap();

        for name in ["out.json", "out.json.gz", "out.csv", "out.csv.gz"] {
            let path = dir.join(name);
            let saved = engine
                .execute(&format!("SCAN * FROM foobar SAVE '{}'", path.display()))
                .unwrap();
            assert_eq!(saved, StatementResult::Affected(2), "{name}");

            engine.execute("DELETE FROM destination").ok();
            engine
                .execute(&format!("LOAD '{}' INTO destination", path.display()))
                .unwrap();
            let source = engine.execute("SCAN * FROM foobar").unwrap();
            let dest = engine.execute("SCAN * FROM destination").unwrap();
            match (source, dest) {
                (StatementResult::Items(mut left), StatementResult::Items(mut right)) => {
                    left.sort_by(|a, b| {
                        format!("{:?}", a.get("id")).cmp(&format!("{:?}", b.get("id")))
                    });
                    right.sort_by(|a, b| {
                        format!("{:?}", a.get("id")).cmp(&format!("{:?}", b.get("id")))
                    });
                    assert_eq!(left, right, "{name}");
                }
                other => panic!("unexpected results for {name}: {other:?}"),
            }
        }
    }

    #[test]
    fn save_checks_the_path_before_reading() {
        let dir = std::env::temp_dir().join("dql_engine_save_preflight");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let missing = dir.join("missing").join("out.json");
        let existing = dir.join("kept.json");
        std::fs::write(&existing, "keep").unwrap();

        let mut engine = Engine::new(MemoryBackend::new());
        engine
            .execute(
                "CREATE TABLE t (id STRING HASH KEY);
                 INSERT INTO t (id) VALUES ('a')",
            )
            .unwrap();

        let err = engine
            .execute(&format!(
                "EXPLAIN SCAN * FROM t SAVE '{}'",
                missing.display()
            ))
            .unwrap_err();
        assert!(
            err.to_string().contains("No such file or directory"),
            "{err}"
        );
        assert!(!missing.exists());
        assert!(!dir.join("missing").exists());

        let err = engine
            .execute(&format!("SELECT * FROM t SAVE '{}'", missing.display()))
            .unwrap_err();
        assert!(
            err.to_string().contains("No such file or directory"),
            "{err}"
        );

        let err = engine
            .execute(&format!(
                "SCAN * FROM missing_table SAVE '{}'",
                existing.display()
            ))
            .unwrap_err();
        assert!(err.to_string().contains("not found"), "{err}");
        assert_eq!(std::fs::read_to_string(&existing).unwrap(), "keep");

        let pickle = dir.join("archive.p");
        let err = engine
            .execute(&format!("SCAN * FROM t SAVE '{}'", pickle.display()))
            .unwrap_err();
        assert!(err.to_string().contains("pickle"), "{err}");
        assert!(!pickle.exists());
    }

    #[test]
    fn save_create_parents_makes_directories_when_the_write_happens() {
        let dir = std::env::temp_dir().join("dql_engine_save_create_parents");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let nested = dir.join("exports").join("deep").join("out.json");
        let blocked = dir.join("not-a-dir");
        std::fs::write(&blocked, "file").unwrap();
        let blocked_target = blocked.join("out.json");

        let mut engine = Engine::new(MemoryBackend::new());
        engine
            .execute(
                "CREATE TABLE t (id STRING HASH KEY);
                 INSERT INTO t (id, name) VALUES ('a', 'alpha')",
            )
            .unwrap();

        let err = engine
            .execute(&format!(
                "SCAN * FROM missing_table SAVE '{}' CREATE PARENTS",
                nested.display()
            ))
            .unwrap_err();
        assert!(err.to_string().contains("not found"), "{err}");
        assert!(!dir.join("exports").exists());

        let err = engine
            .execute(&format!(
                "EXPLAIN SCAN * FROM t SAVE '{}' CREATE PARENTS",
                blocked_target.display()
            ))
            .unwrap_err();
        assert!(
            err.to_string().contains("directory") || err.to_string().contains("Not a directory"),
            "{err}"
        );
        assert!(blocked.is_file());
        assert!(!blocked_target.exists());

        let saved = engine
            .execute(&format!(
                "SCAN * FROM t SAVE '{}' CREATE PARENTS",
                nested.display()
            ))
            .unwrap();
        assert_eq!(saved, StatementResult::Affected(1));
        let text = std::fs::read_to_string(&nested).unwrap();
        assert!(text.contains("alpha"), "{text}");
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

    #[test]
    fn bare_desc_on_scan_without_order_by_errors() {
        let mut engine = Engine::new(MemoryBackend::new()).with_allow_select_scan(true);
        engine
            .execute("CREATE TABLE t (id STRING HASH KEY, score NUMBER)")
            .unwrap();
        let err = engine.execute("SCAN * FROM t DESC").unwrap_err();
        assert!(
            err.to_string().contains("ORDER BY"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn query_desc_uses_range_key_order() {
        let mut engine = Engine::new(MemoryBackend::new());
        let result = engine
            .execute(
                "CREATE TABLE t (id STRING HASH KEY, score NUMBER RANGE KEY);
                 INSERT INTO t (id, score) VALUES ('a', 1), ('a', 3), ('a', 2);
                 SELECT * FROM t WHERE id = 'a' DESC",
            )
            .unwrap();
        match result {
            StatementResult::Items(items) => {
                let scores: Vec<_> = items
                    .iter()
                    .map(|item| item.get("score").cloned())
                    .collect();
                assert_eq!(
                    scores,
                    vec![
                        Some(Value::Number("3".to_string())),
                        Some(Value::Number("2".to_string())),
                        Some(Value::Number("1".to_string())),
                    ]
                );
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn resolve_ordering_server_vs_client() {
        use dql_models::{Operation, ProjectionType, QueryIndex, QueryPlan};

        let index = QueryIndex {
            name: "TABLE".to_string(),
            is_global: true,
            hash_key: "id".to_string(),
            range_key: Some("bar".to_string()),
            projection: ProjectionType::All,
            projected_attributes: None,
        };
        let plan = QueryPlan {
            operation: Operation::Query,
            index: Some(index),
            key_condition: None,
            filter_condition: None,
            follow_up_batch_get: false,
        };

        let bare_desc = QueryOptions {
            descending: Some(true),
            ..QueryOptions::default()
        };
        let policy = resolve_ordering(&plan, &bare_desc).unwrap();
        assert_eq!(policy.scan_index_forward, Some(false));
        assert!(policy.client_order_by.is_none());

        let order_non_range = QueryOptions {
            order_by: Some(OrderBy {
                field: "baz".to_string(),
                descending: true,
            }),
            descending: Some(true),
            ..QueryOptions::default()
        };
        let policy = resolve_ordering(&plan, &order_non_range).unwrap();
        assert_eq!(policy.scan_index_forward, None);
        assert_eq!(
            policy.client_order_by,
            Some(OrderBy {
                field: "baz".to_string(),
                descending: true,
            })
        );

        let order_range = QueryOptions {
            order_by: Some(OrderBy {
                field: "bar".to_string(),
                descending: true,
            }),
            descending: Some(true),
            ..QueryOptions::default()
        };
        let policy = resolve_ordering(&plan, &order_range).unwrap();
        assert_eq!(policy.scan_index_forward, Some(false));
        assert!(policy.client_order_by.is_none());
    }

    #[test]
    fn insert_emits_chunked_write_progress() {
        use std::sync::{Arc, Mutex};

        let mut engine = Engine::new(MemoryBackend::new());
        engine
            .execute("CREATE TABLE t (id STRING HASH KEY)")
            .unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_cb = Arc::clone(&seen);
        engine.progress().set(Some(Box::new(move |event| {
            seen_cb
                .lock()
                .unwrap()
                .push((event.done, event.total, event.phase));
        })));
        let mut values = Vec::new();
        for i in 0..30 {
            values.push(format!("('{i}')"));
        }
        let sql = format!("INSERT INTO t (id) VALUES {}", values.join(", "));
        let result = engine.execute(&sql).unwrap();
        assert_eq!(result, StatementResult::Affected(30));
        engine.progress().clear();
        let events = seen.lock().unwrap().clone();
        assert_eq!(
            events,
            vec![
                (0, Some(30), "write".to_string()),
                (25, Some(30), "write".to_string()),
                (30, Some(30), "write".to_string()),
            ]
        );
    }

    #[test]
    fn insert_stops_when_cancel_is_requested() {
        use std::sync::{Arc, Mutex};

        let mut engine = Engine::new(MemoryBackend::new());
        engine
            .execute("CREATE TABLE t (id STRING HASH KEY)")
            .unwrap();
        let cancel = engine.cancel().clone();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_cb = Arc::clone(&seen);
        engine.progress().set(Some(Box::new(move |event| {
            seen_cb.lock().unwrap().push(event.done);
            if event.done >= 25 {
                cancel.request();
            }
        })));
        let mut values = Vec::new();
        for i in 0..60 {
            values.push(format!("('{i}')"));
        }
        let sql = format!("INSERT INTO t (id) VALUES {}", values.join(", "));
        let err = engine.execute(&sql).unwrap_err();
        assert!(matches!(err, EngineError::Cancelled));
        assert_eq!(err.to_string(), "Query stopped");
        engine.progress().clear();
        let events = seen.lock().unwrap().clone();
        assert!(events.contains(&25), "{events:?}");
        assert!(!events.contains(&60), "{events:?}");
    }

    #[test]
    fn refresh_reloads_only_cached_definitions() {
        use crate::table_cache::{CachedDefinition, CachedNameList, TableCacheStore};
        use std::sync::{Arc, Mutex};

        #[derive(Default)]
        struct MemStore {
            generation: std::sync::atomic::AtomicU64,
            names: Mutex<Option<CachedNameList>>,
            defs: Mutex<std::collections::BTreeMap<String, CachedDefinition>>,
        }

        impl TableCacheStore for MemStore {
            fn generation(&self) -> u64 {
                self.generation.load(std::sync::atomic::Ordering::Acquire)
            }

            fn refreshing(&self) -> bool {
                false
            }

            fn get_names(&self) -> Option<CachedNameList> {
                self.names.lock().expect("names").clone()
            }

            fn put_names(&self, names: &[String]) -> u64 {
                *self.names.lock().expect("names") = Some(CachedNameList {
                    saved_at_ms: 1,
                    names: names.to_vec(),
                });
                self.generation
                    .fetch_add(1, std::sync::atomic::Ordering::Release)
            }

            fn forget_names(&self) {
                *self.names.lock().expect("names") = None;
                self.generation
                    .fetch_add(1, std::sync::atomic::Ordering::Release);
            }

            fn get_definition(&self, name: &str) -> Option<CachedDefinition> {
                self.defs.lock().expect("defs").get(name).cloned()
            }

            fn put_definition(&self, meta: &TableMeta) -> u64 {
                self.defs.lock().expect("defs").insert(
                    meta.name.clone(),
                    CachedDefinition {
                        saved_at_ms: 1,
                        meta: meta.clone(),
                    },
                );
                self.generation
                    .fetch_add(1, std::sync::atomic::Ordering::Release)
            }

            fn remove_definition(&self, name: &str) {
                self.defs.lock().expect("defs").remove(name);
                self.generation
                    .fetch_add(1, std::sync::atomic::Ordering::Release);
            }

            fn definition_names(&self) -> Vec<String> {
                let mut names: Vec<_> = self.defs.lock().expect("defs").keys().cloned().collect();
                names.sort();
                names
            }

            fn clear(&self) {
                *self.names.lock().expect("names") = None;
                self.defs.lock().expect("defs").clear();
                self.generation
                    .fetch_add(1, std::sync::atomic::Ordering::Release);
            }
        }

        let store = Arc::new(MemStore::default());
        let mut engine = Engine::new(MemoryBackend::new());
        engine.set_table_cache(Some(store.clone()));
        engine
            .execute(
                "CREATE TABLE kept (id STRING HASH KEY);
                 CREATE TABLE other (n NUMBER HASH KEY)",
            )
            .unwrap();
        assert!(
            store.get_names().is_none(),
            "create does not invent a name list"
        );
        assert_eq!(
            store.definition_names(),
            vec!["kept".to_string(), "other".to_string()]
        );

        store.remove_definition("other");
        engine.set_table_cache(Some(store.clone()));
        let names = engine.cached_table_names(false).unwrap();
        assert_eq!(names, vec!["kept".to_string(), "other".to_string()]);
        assert!(engine.cached_definition("kept").is_some());
        assert!(engine.cached_definition("other").is_none());

        engine
            .execute("ALTER TABLE kept CREATE GLOBAL INDEX ('by-n', n NUMBER)")
            .unwrap();
        assert_eq!(engine.refresh_cached_definitions().unwrap(), 1);
        let kept = engine.cached_definition("kept").unwrap();
        assert!(kept.global_indexes.contains_key("by-n"));
        assert!(
            engine.cached_definition("other").is_none(),
            "refresh must not describe tables that were never cached"
        );
        engine.cached_table_names(true).unwrap();
        assert!(engine.cached_definition("other").is_none());

        engine.clear_table_cache();
        assert!(store.get_names().is_none());
        assert!(store.definition_names().is_empty());
        assert!(engine.cached_definition("kept").is_none());
    }
}
