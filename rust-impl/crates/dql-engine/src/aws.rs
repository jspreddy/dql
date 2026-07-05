use crate::convert::{
    attributes_to_item, build_create_table_input, expression_values_to_attributes,
    item_to_attributes, table_meta_from_description,
};
use crate::throttle::RateLimit;
use crate::{
    BackendResponse, CapacityRecord, DynamoBackend, EngineError, Item, ReadOperation, ReadRequest,
};
use aws_sdk_dynamodb::types::{
    GlobalSecondaryIndexUpdate, KeySchemaElement, KeyType as AwsKeyType, Projection,
    ProjectionType, ProvisionedThroughput, ReturnConsumedCapacity, ReturnValue, WriteRequest,
};
use aws_sdk_dynamodb::Client;
use dql_expr::{render_condition, render_update, RenderedExpression};
use dql_models::TableMeta;
use dql_parser::{AlterAction, Condition, QueryOptions, Selection, UpdateExpr};
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;
use tokio::runtime::Runtime;

const BATCH_WRITE_CHUNK: usize = 25;
const BATCH_GET_CHUNK: usize = 100;

#[derive(Debug, Clone)]
pub struct SdkConfig {
    pub region: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
}

impl SdkConfig {
    pub fn local(region: impl Into<String>, host: impl Into<String>, port: u16) -> Self {
        Self {
            region: region.into(),
            host: Some(host.into()),
            port: Some(port),
            access_key: Some("test".to_string()),
            secret_key: Some("test".to_string()),
        }
    }
}

pub struct SdkBackend {
    client: Client,
    runtime: Runtime,
    cache: HashMap<String, TableMeta>,
    rate_limit: Option<RateLimit>,
}

impl SdkBackend {
    pub fn connect(config: SdkConfig) -> Result<Self, EngineError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|err| EngineError::Runtime(err.to_string()))?;
        let client = runtime
            .block_on(async { build_client(config).await })
            .map_err(|err| EngineError::Runtime(err))?;
        Ok(Self {
            client,
            runtime,
            cache: HashMap::new(),
            rate_limit: None,
        })
    }

    pub fn set_rate_limit(&mut self, read_per_second: f64, write_per_second: f64) {
        self.rate_limit = Some(RateLimit::new(read_per_second, write_per_second));
    }

    fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }

    fn invalidate_cache(&mut self, table: &str) {
        self.cache.remove(table);
    }

    fn apply_throttle(&mut self, capacity: &CapacityRecord) -> Result<(), EngineError> {
        let Some(rate_limit) = self.rate_limit.as_mut() else {
            return Ok(());
        };
        let wait = rate_limit.on_capacity(capacity.read_units, capacity.write_units);
        if wait > Duration::ZERO {
            std::thread::sleep(wait);
        }
        Ok(())
    }

    fn capacity_from(
        operation: &str,
        table: &str,
        read_units: f64,
        write_units: f64,
    ) -> CapacityRecord {
        CapacityRecord {
            operation: operation.to_string(),
            table: table.to_string(),
            read_units,
            write_units,
        }
    }

    fn aws_error(err: impl std::fmt::Display) -> EngineError {
        EngineError::Runtime(err.to_string())
    }
}

impl DynamoBackend for SdkBackend {
    fn list_tables(&self) -> Result<Vec<String>, EngineError> {
        self.block_on(async {
            let mut names = Vec::new();
            let mut last = None;
            loop {
                let mut request = self.client.list_tables();
                if let Some(last) = &last {
                    request = request.exclusive_start_table_name(last);
                }
                let response = request.send().await.map_err(Self::aws_error)?;
                names.extend(response.table_names().iter().cloned());
                last = response.last_evaluated_table_name().map(str::to_string);
                if last.is_none() {
                    break;
                }
            }
            Ok(names)
        })
    }

    fn describe_table(&self, table: &str) -> Result<Option<TableMeta>, EngineError> {
        if let Some(meta) = self.cache.get(table) {
            return Ok(Some(meta.clone()));
        }
        self.block_on(async {
            let response = self
                .client
                .describe_table()
                .table_name(table)
                .send()
                .await;
            match response {
                Ok(output) => {
                    let description = output.table().ok_or_else(|| {
                        EngineError::Runtime("missing table description".to_string())
                    })?;
                    Ok(Some(table_meta_from_description(description)?))
                }
                Err(err) if err.to_string().contains("ResourceNotFoundException") => Ok(None),
                Err(err) => Err(Self::aws_error(err)),
            }
        })
    }

    fn create_table(
        &mut self,
        meta: TableMeta,
        if_not_exists: bool,
    ) -> Result<BackendResponse<String>, EngineError> {
        let name = meta.name.clone();
        let builder = build_create_table_input(&meta)?;
        let result = self.block_on(async { builder.send_with(&self.client).await });
        match result {
            Ok(_) => {
                self.invalidate_cache(&name);
                Ok(BackendResponse::new(
                    "create_table",
                    &name,
                    format!("Created table '{name}'"),
                ))
            }
            Err(err) if if_not_exists && err.to_string().contains("ResourceInUseException") => {
                Ok(BackendResponse::new(
                    "create_table",
                    &name,
                    format!("Table '{name}' already exists"),
                ))
            }
            Err(err) => Err(Self::aws_error(err)),
        }
    }

    fn delete_table(
        &mut self,
        table: &str,
        if_exists: bool,
    ) -> Result<BackendResponse<String>, EngineError> {
        let result = self.block_on(async {
            self.client.delete_table().table_name(table).send().await
        });
        match result {
            Ok(_) => {
                self.invalidate_cache(table);
                Ok(BackendResponse::new(
                    "delete_table",
                    table,
                    format!("Dropped table '{table}'"),
                ))
            }
            Err(err) if if_exists && err.to_string().contains("ResourceNotFoundException") => {
                Ok(BackendResponse::new(
                    "delete_table",
                    table,
                    format!("Table '{table}' did not exist"),
                ))
            }
            Err(err) => Err(Self::aws_error(err)),
        }
    }

    fn batch_write(
        &mut self,
        table: &str,
        items: Vec<Item>,
    ) -> Result<BackendResponse<usize>, EngineError> {
        let mut written = 0usize;
        let mut read_units = 0.0;
        let mut write_units = 0.0;
        for chunk in items.chunks(BATCH_WRITE_CHUNK) {
            let write_requests = chunk
                .iter()
                .map(|item| {
                    item_to_attributes(item).map(|attributes| {
                        WriteRequest::builder()
                            .put_request(
                                aws_sdk_dynamodb::types::PutRequest::builder()
                                    .set_item(Some(attributes))
                                    .build()
                                    .expect("valid put request"),
                            )
                            .build()
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut pending: HashMap<String, Vec<WriteRequest>> =
                HashMap::from([(table.to_string(), write_requests)]);
            while !pending.is_empty() {
                let response = self
                    .block_on(async {
                        self.client
                            .batch_write_item()
                            .set_request_items(Some(pending.clone()))
                            .return_consumed_capacity(ReturnConsumedCapacity::Total)
                            .send()
                            .await
                    })
                    .map_err(Self::aws_error)?;
                if let Some(capacity) = response.consumed_capacity().first() {
                    read_units += capacity.capacity_units().unwrap_or(0.0);
                    write_units += capacity.write_capacity_units().unwrap_or(0.0);
                }
                pending = response
                    .unprocessed_items()
                    .cloned()
                    .unwrap_or_default();
                if !pending.is_empty() {
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            written += chunk.len();
        }
        let capacity = Self::capacity_from("batch_write_item", table, read_units, write_units);
        self.apply_throttle(&capacity)?;
        Ok(BackendResponse {
            output: written,
            capacity: Some(capacity),
        })
    }

    fn execute_read(
        &self,
        table: &str,
        request: &ReadRequest<'_>,
    ) -> Result<BackendResponse<Vec<Item>>, EngineError> {
        let rendered_filter = request
            .filter_condition
            .map(render_condition)
            .transpose()
            .map_err(|err| EngineError::Runtime(err.to_string()))?;
        let rendered_key = request
            .key_condition
            .map(render_condition)
            .transpose()
            .map_err(|err| EngineError::Runtime(err.to_string()))?;
        let mut items = match request.operation {
            ReadOperation::Query => self.query_items(
                table,
                request.index_name,
                rendered_key.as_ref(),
                rendered_filter.as_ref(),
                request.options,
            )?,
            ReadOperation::Scan => self.scan_items(
                table,
                request.index_name,
                rendered_filter.as_ref(),
                request.options,
            )?,
        };
        if request.follow_up_batch_get {
            items = self.batch_get_items(table, &items)?;
        }
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
        let keys = self.keys_for_condition(table, condition)?;
        let mut deleted = 0usize;
        let mut read_units = 0.0;
        let mut write_units = 0.0;
        for key in keys {
            let response = self
                .block_on(async {
                    self.client
                        .delete_item()
                        .table_name(table)
                        .set_key(Some(key))
                        .return_consumed_capacity(ReturnConsumedCapacity::Total)
                        .send()
                        .await
                })
                .map_err(Self::aws_error)?;
            if let Some(capacity) = response.consumed_capacity() {
                read_units += capacity.capacity_units().unwrap_or(0.0);
                write_units += capacity.write_capacity_units().unwrap_or(0.0);
            }
            deleted += 1;
        }
        let capacity = Self::capacity_from("delete_item", table, read_units, write_units);
        self.apply_throttle(&capacity)?;
        Ok(BackendResponse {
            output: deleted,
            capacity: Some(capacity),
        })
    }

    fn update_matching(
        &mut self,
        table: &str,
        update: &UpdateExpr,
        condition: Option<&Condition>,
    ) -> Result<BackendResponse<usize>, EngineError> {
        let rendered =
            render_update(update).map_err(|err| EngineError::Runtime(err.to_string()))?;
        let condition_expr = condition
            .map(render_condition)
            .transpose()
            .map_err(|err| EngineError::Runtime(err.to_string()))?;
        let keys = self.keys_for_condition(table, condition)?;
        let mut updated = 0usize;
        let mut read_units = 0.0;
        let mut write_units = 0.0;
        for key in keys {
            let mut request = self
                .client
                .update_item()
                .table_name(table)
                .set_key(Some(key))
                .update_expression(rendered.expression.clone())
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .return_values(ReturnValue::None);
            if let Some(names) = rendered.attribute_names.clone() {
                request = request.set_expression_attribute_names(names_to_hash(Some(names)));
            }
            if let Some(values) = rendered.expression_values.clone() {
                request = request.set_expression_attribute_values(Some(
                    expression_values_to_attributes(&values)?,
                ));
            }
            if let Some(condition_expr) = condition_expr.as_ref() {
                request = request
                    .condition_expression(condition_expr.expression.clone())
                    .set_expression_attribute_names(merge_names(
                        rendered.attribute_names.as_ref(),
                        condition_expr.attribute_names.as_ref(),
                    ))
                    .set_expression_attribute_values(merge_values(
                        rendered.expression_values.as_ref(),
                        condition_expr.expression_values.as_ref(),
                    )?);
            }
            let response = self
                .block_on(async { request.send().await })
                .map_err(Self::aws_error)?;
            if let Some(capacity) = response.consumed_capacity() {
                read_units += capacity.capacity_units().unwrap_or(0.0);
                write_units += capacity.write_capacity_units().unwrap_or(0.0);
            }
            updated += 1;
        }
        let capacity = Self::capacity_from("update_item", table, read_units, write_units);
        self.apply_throttle(&capacity)?;
        Ok(BackendResponse {
            output: updated,
            capacity: Some(capacity),
        })
    }

    fn alter_table(
        &mut self,
        table: &str,
        action: &AlterAction,
    ) -> Result<BackendResponse<String>, EngineError> {
        let message = match action {
            AlterAction::SetThroughput { index, throughput } => {
                let read = match &throughput.read {
                    dql_parser::Value::Number(value) => value
                        .parse::<i64>()
                        .map_err(|err| EngineError::Runtime(err.to_string()))?,
                    _ => return Err(EngineError::Runtime("invalid read throughput".to_string())),
                };
                let write = match &throughput.write {
                    dql_parser::Value::Number(value) => value
                        .parse::<i64>()
                        .map_err(|err| EngineError::Runtime(err.to_string()))?,
                    _ => return Err(EngineError::Runtime("invalid write throughput".to_string())),
                };
                let throughput = ProvisionedThroughput::builder()
                    .read_capacity_units(read)
                    .write_capacity_units(write)
                    .build()
                    .map_err(|err| EngineError::Runtime(err.to_string()))?;
                let mut request = self.client.update_table().table_name(table);
                if let Some(index_name) = index {
                    request = request.global_secondary_index_updates(
                        GlobalSecondaryIndexUpdate::builder()
                            .update(
                                aws_sdk_dynamodb::types::UpdateGlobalSecondaryIndexAction::builder()
                                    .index_name(index_name)
                                    .provisioned_throughput(throughput)
                                    .build()
                                    .map_err(|err| EngineError::Runtime(err.to_string()))?,
                            )
                            .build(),
                    );
                } else {
                    request = request.provisioned_throughput(throughput);
                }
                self.block_on(async { request.send().await })
                    .map_err(Self::aws_error)?;
                format!(
                    "Updated throughput for {} on '{table}'",
                    index.as_deref().unwrap_or("table")
                )
            }
            AlterAction::DropIndex { name, if_exists } => {
                let delete = aws_sdk_dynamodb::types::DeleteGlobalSecondaryIndexAction::builder()
                    .index_name(name)
                    .build()
                    .map_err(|err| EngineError::Runtime(err.to_string()))?;
                let update = GlobalSecondaryIndexUpdate::builder()
                    .delete(delete)
                    .build();
                let result = self.block_on(async {
                    self.client
                        .update_table()
                        .table_name(table)
                        .global_secondary_index_updates(update)
                        .send()
                        .await
                });
                match result {
                    Ok(_) => format!("Dropped index '{name}' from '{table}'"),
                    Err(err)
                        if *if_exists
                            && (err.to_string().contains("ResourceNotFoundException")
                                || err.to_string().contains("does not exist")) =>
                    {
                        format!("Index '{name}' did not exist on '{table}'")
                    }
                    Err(err) => return Err(Self::aws_error(err)),
                }
            }
            AlterAction::CreateGlobalIndex {
                index,
                if_not_exists,
            } => {
                let projection = match index.projection {
                    dql_parser::ProjectionKind::All => Projection::builder()
                        .projection_type(ProjectionType::All)
                        .build(),
                    dql_parser::ProjectionKind::Keys => Projection::builder()
                        .projection_type(ProjectionType::KeysOnly)
                        .build(),
                    dql_parser::ProjectionKind::Include => Projection::builder()
                        .projection_type(ProjectionType::Include)
                        .set_non_key_attributes(Some(index.includes.clone()))
                        .build(),
                };
                let mut key_schema = vec![
                    KeySchemaElement::builder()
                        .attribute_name(index.hash_key.clone())
                        .key_type(AwsKeyType::Hash)
                        .build()
                        .map_err(|err| EngineError::Runtime(err.to_string()))?,
                ];
                if let Some(range_key) = &index.range_key {
                    key_schema.push(
                        KeySchemaElement::builder()
                            .attribute_name(range_key.clone())
                            .key_type(AwsKeyType::Range)
                            .build()
                            .map_err(|err| EngineError::Runtime(err.to_string()))?,
                    );
                }
                let mut create =
                    aws_sdk_dynamodb::types::CreateGlobalSecondaryIndexAction::builder()
                        .index_name(index.name.clone())
                        .set_key_schema(Some(key_schema))
                        .projection(projection);
                if let Some(throughput) = index.throughput.as_ref() {
                    let read = match &throughput.read {
                        dql_parser::Value::Number(value) => value
                            .parse::<i64>()
                            .map_err(|err| EngineError::Runtime(err.to_string()))?,
                        _ => {
                            return Err(EngineError::Runtime(
                                "invalid read throughput".to_string(),
                            ))
                        }
                    };
                    let write = match &throughput.write {
                        dql_parser::Value::Number(value) => value
                            .parse::<i64>()
                            .map_err(|err| EngineError::Runtime(err.to_string()))?,
                        _ => {
                            return Err(EngineError::Runtime(
                                "invalid write throughput".to_string(),
                            ))
                        }
                    };
                    create = create.provisioned_throughput(
                        ProvisionedThroughput::builder()
                            .read_capacity_units(read)
                            .write_capacity_units(write)
                            .build()
                            .map_err(|err| EngineError::Runtime(err.to_string()))?,
                    );
                }
                let create = create
                    .build()
                    .map_err(|err| EngineError::Runtime(err.to_string()))?;
                let update = GlobalSecondaryIndexUpdate::builder().create(create).build();
                let result = self.block_on(async {
                    self.client
                        .update_table()
                        .table_name(table)
                        .global_secondary_index_updates(update)
                        .send()
                        .await
                });
                match result {
                    Ok(_) => format!("Created global index '{}' on '{table}'", index.name),
                    Err(err)
                        if *if_not_exists && err.to_string().contains("already exists") =>
                    {
                        format!("Index '{}' already exists on '{table}'", index.name)
                    }
                    Err(err) => return Err(Self::aws_error(err)),
                }
            }
        };
        self.invalidate_cache(table);
        Ok(BackendResponse::new("update_table", table, message))
    }
}

impl SdkBackend {
    fn keys_for_condition(
        &self,
        table: &str,
        condition: Option<&Condition>,
    ) -> Result<Vec<HashMap<String, aws_sdk_dynamodb::types::AttributeValue>>, EngineError> {
        let request = ReadRequest {
            operation: ReadOperation::Scan,
            index_name: None,
            key_condition: None,
            filter_condition: condition,
            condition,
            selection: &Selection::All,
            options: &QueryOptions::default(),
            follow_up_batch_get: false,
        };
        let items = self.execute_read(table, &request)?.output;
        items
            .iter()
            .map(|item| primary_key_attributes(table, item, self))
            .collect()
    }

    fn query_items(
        &self,
        table: &str,
        index_name: Option<&str>,
        key_condition: Option<&RenderedExpression>,
        filter_condition: Option<&RenderedExpression>,
        options: &QueryOptions,
    ) -> Result<Vec<Item>, EngineError> {
        let mut items = Vec::new();
        let mut last_key: Option<HashMap<String, aws_sdk_dynamodb::types::AttributeValue>> = None;
        let item_limit = options.limit.unwrap_or(usize::MAX);
        let scan_limit = options.scan_limit.unwrap_or(usize::MAX);
        let mut scanned = 0usize;
        loop {
            let mut request = self
                .client
                .query()
                .table_name(table)
                .return_consumed_capacity(ReturnConsumedCapacity::Total);
            if let Some(index_name) = index_name {
                request = request.index_name(index_name);
            }
            if let Some(key_condition) = key_condition {
                request = request
                    .key_condition_expression(key_condition.expression.clone())
                    .set_expression_attribute_names(names_to_hash(
                        key_condition.attribute_names.clone(),
                    ))
                    .set_expression_attribute_values(Some(expression_values_to_attributes(
                        key_condition
                            .expression_values
                            .as_ref()
                            .unwrap_or(&BTreeMap::new()),
                    )?));
            }
            if let Some(filter_condition) = filter_condition {
                request = request
                    .filter_expression(filter_condition.expression.clone())
                    .set_expression_attribute_names(merge_names(
                        key_condition.and_then(|expr| expr.attribute_names.as_ref()),
                        filter_condition.attribute_names.as_ref(),
                    ))
                    .set_expression_attribute_values(merge_values(
                        key_condition.and_then(|expr| expr.expression_values.as_ref()),
                        filter_condition.expression_values.as_ref(),
                    )?);
            }
            if let Some(last_key) = &last_key {
                request = request.set_exclusive_start_key(Some(last_key.clone()));
            }
            let remaining = item_limit.saturating_sub(items.len());
            if remaining == 0 {
                break;
            }
            request = request.limit((remaining.min(100)) as i32);
            let response = self
                .block_on(async { request.send().await })
                .map_err(Self::aws_error)?;
            scanned += response.count() as usize;
            for item in response.items() {
                items.push(attributes_to_item(item)?);
                if items.len() >= item_limit {
                    break;
                }
            }
            last_key = response.last_evaluated_key().cloned();
            if items.len() >= item_limit || last_key.is_none() || scanned >= scan_limit {
                break;
            }
        }
        Ok(items)
    }

    fn scan_items(
        &self,
        table: &str,
        index_name: Option<&str>,
        filter_condition: Option<&RenderedExpression>,
        options: &QueryOptions,
    ) -> Result<Vec<Item>, EngineError> {
        let mut items = Vec::new();
        let mut last_key: Option<HashMap<String, aws_sdk_dynamodb::types::AttributeValue>> = None;
        let item_limit = options.limit.unwrap_or(usize::MAX);
        let scan_limit = options.scan_limit.unwrap_or(usize::MAX);
        let mut scanned = 0usize;
        loop {
            let mut request = self
                .client
                .scan()
                .table_name(table)
                .return_consumed_capacity(ReturnConsumedCapacity::Total);
            if let Some(index_name) = index_name {
                request = request.index_name(index_name);
            }
            if let Some(filter_condition) = filter_condition {
                request = request
                    .filter_expression(filter_condition.expression.clone())
                    .set_expression_attribute_names(names_to_hash(
                        filter_condition.attribute_names.clone(),
                    ))
                    .set_expression_attribute_values(Some(expression_values_to_attributes(
                        filter_condition
                            .expression_values
                            .as_ref()
                            .unwrap_or(&BTreeMap::new()),
                    )?));
            }
            if let Some(last_key) = &last_key {
                request = request.set_exclusive_start_key(Some(last_key.clone()));
            }
            let remaining = item_limit.saturating_sub(items.len());
            if remaining == 0 {
                break;
            }
            request = request.limit((remaining.min(100)) as i32);
            let response = self
                .block_on(async { request.send().await })
                .map_err(Self::aws_error)?;
            scanned += response.scanned_count() as usize;
            for item in response.items() {
                items.push(attributes_to_item(item)?);
                if items.len() >= item_limit {
                    break;
                }
            }
            last_key = response.last_evaluated_key().cloned();
            if items.len() >= item_limit || last_key.is_none() || scanned >= scan_limit {
                break;
            }
        }
        Ok(items)
    }

    fn batch_get_items(&self, table: &str, partial_items: &[Item]) -> Result<Vec<Item>, EngineError> {
        let meta = self
            .describe_table(table)?
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let mut results = Vec::new();
        for chunk in partial_items.chunks(BATCH_GET_CHUNK) {
            let keys = chunk
                .iter()
                .map(|item| primary_key_from_meta(&meta, item))
                .collect::<Result<Vec<_>, _>>()?;
            let keys_and_attributes = aws_sdk_dynamodb::types::KeysAndAttributes::builder()
                .set_keys(Some(keys))
                .build()
                .map_err(|err| EngineError::Runtime(err.to_string()))?;
            let response = self
                .block_on(async {
                    self.client
                        .batch_get_item()
                        .request_items(table, keys_and_attributes)
                        .return_consumed_capacity(ReturnConsumedCapacity::Total)
                        .send()
                        .await
                })
                .map_err(Self::aws_error)?;
            if let Some(items) = response.responses().and_then(|responses| responses.get(table)) {
                for item in items {
                    results.push(attributes_to_item(item)?);
                }
            }
        }
        Ok(results)
    }
}

async fn build_client(config: SdkConfig) -> Result<Client, String> {
    let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new(config.region.clone()));
    if let (Some(access_key), Some(secret_key)) = (&config.access_key, &config.secret_key) {
        loader = loader.credentials_provider(aws_credential_types::Credentials::new(
            access_key,
            secret_key,
            None,
            None,
            "dql",
        ));
    }
    let shared = loader.load().await;
    let mut builder = aws_sdk_dynamodb::config::Builder::from(&shared);
    if let Some(host) = config.host {
        let port = config.port.unwrap_or(8000);
        builder = builder.endpoint_url(format!("http://{host}:{port}"));
    }
    Ok(Client::from_conf(builder.build()))
}

fn primary_key_attributes(
    table: &str,
    item: &Item,
    backend: &SdkBackend,
) -> Result<HashMap<String, aws_sdk_dynamodb::types::AttributeValue>, EngineError> {
    let meta = backend
        .describe_table(table)?
        .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
    primary_key_from_meta(&meta, item)
}

fn primary_key_from_meta(
    meta: &TableMeta,
    item: &Item,
) -> Result<HashMap<String, aws_sdk_dynamodb::types::AttributeValue>, EngineError> {
    let mut key = HashMap::new();
    let hash = item
        .get(&meta.hash_key)
        .ok_or_else(|| EngineError::Runtime(format!("missing hash key '{}'", meta.hash_key)))?;
    key.insert(
        meta.hash_key.clone(),
        {
            let mut single = Item::new();
            single.insert(meta.hash_key.clone(), hash.clone());
            item_to_attributes(&single)?
                .remove(&meta.hash_key)
                .unwrap()
        },
    );
    if let Some(range_key) = &meta.range_key {
        let value = item
            .get(range_key)
            .ok_or_else(|| EngineError::Runtime(format!("missing range key '{range_key}'")))?;
        key.insert(
            range_key.clone(),
            {
                let mut single = Item::new();
                single.insert(range_key.clone(), value.clone());
                item_to_attributes(&single)?.remove(range_key).unwrap()
            },
        );
    }
    Ok(key)
}

fn merge_names(
    left: Option<&BTreeMap<String, String>>,
    right: Option<&BTreeMap<String, String>>,
) -> Option<HashMap<String, String>> {
    match (left, right) {
        (None, None) => None,
        (Some(left), None) => Some(left.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
        (None, Some(right)) => Some(right.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
        (Some(left), Some(right)) => {
            let mut merged: HashMap<String, String> =
                left.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            merged.extend(right.iter().map(|(k, v)| (k.clone(), v.clone())));
            Some(merged)
        }
    }
}

fn names_to_hash(names: Option<BTreeMap<String, String>>) -> Option<HashMap<String, String>> {
    names.map(|map| map.into_iter().collect())
}

fn merge_values(
    left: Option<&BTreeMap<String, dql_expr::DynamoValue>>,
    right: Option<&BTreeMap<String, dql_expr::DynamoValue>>,
) -> Result<Option<HashMap<String, aws_sdk_dynamodb::types::AttributeValue>>, EngineError> {
    match (left, right) {
        (None, None) => Ok(None),
        (Some(left), None) => expression_values_to_attributes(left).map(Some),
        (None, Some(right)) => expression_values_to_attributes(right).map(Some),
        (Some(left), Some(right)) => {
            let mut merged = left.clone();
            merged.extend(right.clone());
            expression_values_to_attributes(&merged).map(Some)
        }
    }
}
