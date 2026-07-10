use crate::convert::{
    attributes_to_item, build_create_table_input, expression_values_to_attributes,
    item_to_attributes, keys_in_to_items, table_meta_from_description,
};
use crate::throttle::RateLimit;
use crate::{
    BackendResponse, CapacityRecord, DynamoBackend, EngineError, Item, ReadOperation, ReadRequest,
};
use aws_sdk_dynamodb::types::{
    BillingMode as AwsBillingMode, GlobalSecondaryIndexUpdate, KeySchemaElement,
    KeyType as AwsKeyType, Projection, ProjectionType, ProvisionedThroughput,
    ReturnConsumedCapacity, ReturnValue, Select, WriteRequest,
};
use aws_sdk_dynamodb::Client;
use dql_expr::{
    render_condition, render_projection, render_update, renumber_rendered_expression,
    RenderedExpression, Visitor,
};
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

    /// Live AWS endpoint using the default credential chain.
    pub fn aws(region: impl Into<String>) -> Self {
        Self {
            region: region.into(),
            host: None,
            port: None,
            access_key: None,
            secret_key: None,
        }
    }
}

pub struct SdkBackend {
    client: Client,
    runtime: Runtime,
    cache: HashMap<String, TableMeta>,
    rate_limit: Option<RateLimit>,
    region: String,
    config: SdkConfig,
}

impl SdkBackend {
    pub fn connect(config: SdkConfig) -> Result<Self, EngineError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|err| EngineError::Runtime(err.to_string()))?;
        let region = config.region.clone();
        let client = runtime
            .block_on(async { build_client(config.clone()).await })
            .map_err(EngineError::Runtime)?;
        Ok(Self {
            client,
            runtime,
            cache: HashMap::new(),
            rate_limit: None,
            region,
            config,
        })
    }

    pub fn region(&self) -> &str {
        &self.region
    }

    pub fn reconnect(&mut self, config: SdkConfig) -> Result<(), EngineError> {
        let client = self
            .runtime
            .block_on(async { build_client(config.clone()).await })
            .map_err(EngineError::Runtime)?;
        self.client = client;
        self.region = config.region.clone();
        self.config = config;
        self.cache.clear();
        Ok(())
    }

    pub fn session_identity(&self) -> Result<String, EngineError> {
        if self.config.host.is_some() {
            return Ok("local".to_string());
        }
        let output = self
            .runtime
            .block_on(async {
                let sts_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
                    .region(aws_config::Region::new(self.region.clone()))
                    .load()
                    .await;
                let sts = aws_sdk_sts::Client::new(&sts_config);
                sts.get_caller_identity().send().await
            })
            .map_err(|err| EngineError::Runtime(err.to_string()))?;
        Ok(output
            .arn()
            .or(output.user_id())
            .unwrap_or("unknown")
            .to_string())
    }

    pub fn set_rate_limit(&mut self, read_per_second: f64, write_per_second: f64) {
        self.rate_limit = Some(RateLimit::new(read_per_second, write_per_second));
    }

    /// Attach CloudWatch consumed capacity metrics when the `cloudwatch` feature is enabled.
    pub fn attach_cloudwatch_metrics(&self, meta: &mut TableMeta) -> Result<(), EngineError> {
        #[cfg(feature = "cloudwatch")]
        {
            return self.block_on(crate::cloudwatch::attach_metrics(&self.config, meta));
        }
        #[cfg(not(feature = "cloudwatch"))]
        {
            let _ = meta;
            Ok(())
        }
    }

    pub fn is_local(&self) -> bool {
        self.config.host.is_some()
    }

    pub fn sdk_config(&self) -> &SdkConfig {
        &self.config
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
            let response = self.client.describe_table().table_name(table).send().await;
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
        let result =
            self.block_on(async { self.client.delete_table().table_name(table).send().await });
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
                pending = response.unprocessed_items().cloned().unwrap_or_default();
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
            count: None,
            updated_items: None,
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
                request,
            )?,
            ReadOperation::Scan => {
                self.scan_items(table, request.index_name, rendered_filter.as_ref(), request)?
            }
            ReadOperation::BatchGetKeys => {
                let keys_in = request.options.keys_in.as_ref().ok_or_else(|| {
                    EngineError::Runtime("batch get requires KEYS IN".to_string())
                })?;
                let meta = self
                    .describe_table(table)?
                    .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
                let keys = keys_in_to_items(&meta, keys_in)?;
                self.batch_get_items(table, &keys, request.consistent, Some(request.selection))?
            }
        };
        if request.follow_up_batch_get {
            items =
                self.batch_get_items(table, &items, request.consistent, Some(request.selection))?;
        }
        let count = matches!(request.selection, Selection::CountAll).then_some(items.len());
        let op_name = match request.operation {
            ReadOperation::Query => "query",
            ReadOperation::Scan => "scan",
            ReadOperation::BatchGetKeys => "batch_get_item",
        };
        let mut response = BackendResponse::new(op_name, table, items);
        response.count = count;
        Ok(response)
    }

    fn delete_matching(
        &mut self,
        table: &str,
        condition: Option<&Condition>,
        _plan: Option<&dql_models::QueryPlan>,
        _options: &QueryOptions,
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
            count: None,
            updated_items: None,
        })
    }

    fn update_matching(
        &mut self,
        table: &str,
        update: &UpdateExpr,
        condition: Option<&Condition>,
        return_items: bool,
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
        let mut updated_items = Vec::new();
        for key in keys {
            let response = self.send_update_item(
                table,
                key,
                &rendered,
                condition_expr.as_ref(),
                return_items,
            )?;
            if let Some(capacity) = response.consumed_capacity() {
                read_units += capacity.capacity_units().unwrap_or(0.0);
                write_units += capacity.write_capacity_units().unwrap_or(0.0);
            }
            if return_items {
                if let Some(attributes) = response.attributes() {
                    updated_items.push(attributes_to_item(attributes)?);
                }
            }
            updated += 1;
        }
        let capacity = Self::capacity_from("update_item", table, read_units, write_units);
        self.apply_throttle(&capacity)?;
        Ok(BackendResponse {
            output: updated,
            capacity: Some(capacity),
            count: None,
            updated_items: return_items.then_some(updated_items),
        })
    }

    fn batch_get_keys(
        &self,
        table: &str,
        keys: &[Item],
        consistent: bool,
    ) -> Result<BackendResponse<Vec<Item>>, EngineError> {
        let items = self.batch_get_items(table, keys, consistent, None)?;
        Ok(BackendResponse::new("batch_get_item", table, items))
    }

    fn delete_by_keys(
        &mut self,
        table: &str,
        keys: &[Item],
        condition: Option<&Condition>,
    ) -> Result<BackendResponse<usize>, EngineError> {
        let meta = self
            .describe_table(table)?
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let condition_expr = condition
            .map(render_condition)
            .transpose()
            .map_err(|err| EngineError::Runtime(err.to_string()))?;
        let mut deleted = 0usize;
        let mut read_units = 0.0;
        let mut write_units = 0.0;
        for key_item in keys {
            let key = primary_key_from_meta(&meta, key_item)?;
            let mut request = self
                .client
                .delete_item()
                .table_name(table)
                .set_key(Some(key))
                .return_consumed_capacity(ReturnConsumedCapacity::Total);
            if let Some(condition_expr) = condition_expr.as_ref() {
                request = request
                    .condition_expression(condition_expr.expression.clone())
                    .set_expression_attribute_names(names_to_hash(
                        condition_expr.attribute_names.clone(),
                    ))
                    .set_expression_attribute_values(Some(expression_values_to_attributes(
                        condition_expr
                            .expression_values
                            .as_ref()
                            .unwrap_or(&BTreeMap::new()),
                    )?));
            }
            let response = self
                .block_on(async { request.send().await })
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
            count: None,
            updated_items: None,
        })
    }

    fn update_by_keys(
        &mut self,
        table: &str,
        keys: &[Item],
        update: &UpdateExpr,
        condition: Option<&Condition>,
        return_items: bool,
    ) -> Result<BackendResponse<usize>, EngineError> {
        let meta = self
            .describe_table(table)?
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let rendered =
            render_update(update).map_err(|err| EngineError::Runtime(err.to_string()))?;
        let condition_expr = condition
            .map(render_condition)
            .transpose()
            .map_err(|err| EngineError::Runtime(err.to_string()))?;
        let mut updated = 0usize;
        let mut read_units = 0.0;
        let mut write_units = 0.0;
        let mut updated_items = Vec::new();
        for key_item in keys {
            let key = primary_key_from_meta(&meta, key_item)?;
            let response = self.send_update_item(
                table,
                key,
                &rendered,
                condition_expr.as_ref(),
                return_items,
            )?;
            if let Some(capacity) = response.consumed_capacity() {
                read_units += capacity.capacity_units().unwrap_or(0.0);
                write_units += capacity.write_capacity_units().unwrap_or(0.0);
            }
            if return_items {
                if let Some(attributes) = response.attributes() {
                    updated_items.push(attributes_to_item(attributes)?);
                }
            }
            updated += 1;
        }
        let capacity = Self::capacity_from("update_item", table, read_units, write_units);
        self.apply_throttle(&capacity)?;
        Ok(BackendResponse {
            output: updated,
            capacity: Some(capacity),
            count: None,
            updated_items: return_items.then_some(updated_items),
        })
    }

    fn alter_table(
        &mut self,
        table: &str,
        action: &AlterAction,
    ) -> Result<BackendResponse<String>, EngineError> {
        let message = match action {
            AlterAction::SetThroughput { index, throughput } => {
                let meta = self
                    .describe_table(table)?
                    .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
                let (read, write) = resolve_alter_throughput(throughput, &meta, index.as_deref())?;
                let mut request = self.client.update_table().table_name(table);
                if let Some(index_name) = index {
                    let provisioned = ProvisionedThroughput::builder()
                        .read_capacity_units(read)
                        .write_capacity_units(write)
                        .build()
                        .map_err(|err| EngineError::Runtime(err.to_string()))?;
                    request = request.global_secondary_index_updates(
                        GlobalSecondaryIndexUpdate::builder()
                            .update(
                                aws_sdk_dynamodb::types::UpdateGlobalSecondaryIndexAction::builder(
                                )
                                .index_name(index_name)
                                .provisioned_throughput(provisioned)
                                .build()
                                .map_err(|err| EngineError::Runtime(err.to_string()))?,
                            )
                            .build(),
                    );
                } else if read == 0 && write == 0 {
                    request = request.billing_mode(AwsBillingMode::PayPerRequest);
                } else {
                    let provisioned = ProvisionedThroughput::builder()
                        .read_capacity_units(read)
                        .write_capacity_units(write)
                        .build()
                        .map_err(|err| EngineError::Runtime(err.to_string()))?;
                    request = request
                        .billing_mode(AwsBillingMode::Provisioned)
                        .provisioned_throughput(provisioned);
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
                let update = GlobalSecondaryIndexUpdate::builder().delete(delete).build();
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
                let mut key_schema = vec![KeySchemaElement::builder()
                    .attribute_name(index.hash_key.clone())
                    .key_type(AwsKeyType::Hash)
                    .build()
                    .map_err(|err| EngineError::Runtime(err.to_string()))?];
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
                            return Err(EngineError::Runtime("invalid read throughput".to_string()))
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
                    Err(err) if *if_not_exists && err.to_string().contains("already exists") => {
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
    fn send_update_item(
        &self,
        table: &str,
        key: HashMap<String, aws_sdk_dynamodb::types::AttributeValue>,
        rendered: &RenderedExpression,
        condition_expr: Option<&RenderedExpression>,
        return_items: bool,
    ) -> Result<aws_sdk_dynamodb::operation::update_item::UpdateItemOutput, EngineError> {
        let mut request = self
            .client
            .update_item()
            .table_name(table)
            .set_key(Some(key))
            .update_expression(rendered.expression.clone())
            .return_consumed_capacity(ReturnConsumedCapacity::Total)
            .return_values(if return_items {
                ReturnValue::AllNew
            } else {
                ReturnValue::None
            });
        if let Some(names) = rendered.attribute_names.clone() {
            request = request.set_expression_attribute_names(names_to_hash(Some(names)));
        }
        if let Some(values) = rendered.expression_values.clone() {
            request = request
                .set_expression_attribute_values(Some(expression_values_to_attributes(&values)?));
        }
        if let Some(condition_expr) = condition_expr {
            let condition_expr = renumber_rendered_expression(condition_expr, rendered);
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
        self.block_on(async { request.send().await })
            .map_err(Self::aws_error)
    }

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
            consistent: false,
            order_by: None,
            scan_index_forward: None,
            range_key: None,
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
        request: &ReadRequest<'_>,
    ) -> Result<Vec<Item>, EngineError> {
        let options = request.options;
        let is_count = matches!(request.selection, Selection::CountAll);
        let mut items = Vec::new();
        let mut last_key: Option<HashMap<String, aws_sdk_dynamodb::types::AttributeValue>> = None;
        let item_limit = options.limit.unwrap_or(usize::MAX);
        let scan_limit = options.scan_limit.unwrap_or(usize::MAX);
        let mut scanned = 0usize;
        let mut total_count = 0usize;
        loop {
            let mut query = self
                .client
                .query()
                .table_name(table)
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .consistent_read(request.consistent);
            if is_count {
                query = query.select(Select::Count);
            }
            if let Some(index_name) = index_name {
                query = query.index_name(index_name);
            }
            if let Some(forward) = request.scan_index_forward {
                query = query.scan_index_forward(forward);
            }
            if let Some(key_condition) = key_condition {
                query = query
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
                let filter_condition = match key_condition {
                    Some(key) => renumber_rendered_expression(filter_condition, key),
                    None => filter_condition.clone(),
                };
                query = query
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
            if let Some(rendered) = self.read_projection(table, request)? {
                query = query
                    .projection_expression(rendered.expression.clone())
                    .set_expression_attribute_names(names_to_hash(
                        rendered.attribute_names.clone(),
                    ));
            }
            if let Some(last_key) = &last_key {
                query = query.set_exclusive_start_key(Some(last_key.clone()));
            }
            let remaining =
                item_limit.saturating_sub(if is_count { total_count } else { items.len() });
            if remaining == 0 {
                break;
            }
            query = query.limit((remaining.min(100)) as i32);
            let response = self
                .block_on(async { query.send().await })
                .map_err(Self::aws_error)?;
            scanned += response.scanned_count() as usize;
            if is_count {
                total_count += response.count() as usize;
            } else {
                for item in response.items() {
                    items.push(attributes_to_item(item)?);
                    if items.len() >= item_limit {
                        break;
                    }
                }
            }
            last_key = response.last_evaluated_key().cloned();
            let fetched = if is_count { total_count } else { items.len() };
            if fetched >= item_limit || last_key.is_none() || scanned >= scan_limit {
                break;
            }
        }
        if is_count {
            items = (0..total_count).map(|_| Item::new()).collect();
        }
        Ok(items)
    }

    fn scan_items(
        &self,
        table: &str,
        index_name: Option<&str>,
        filter_condition: Option<&RenderedExpression>,
        request: &ReadRequest<'_>,
    ) -> Result<Vec<Item>, EngineError> {
        let options = request.options;
        let is_count = matches!(request.selection, Selection::CountAll);
        let mut items = Vec::new();
        let mut last_key: Option<HashMap<String, aws_sdk_dynamodb::types::AttributeValue>> = None;
        let item_limit = options.limit.unwrap_or(usize::MAX);
        let scan_limit = options.scan_limit.unwrap_or(usize::MAX);
        let mut scanned = 0usize;
        let mut total_count = 0usize;
        loop {
            let mut scan = self
                .client
                .scan()
                .table_name(table)
                .return_consumed_capacity(ReturnConsumedCapacity::Total)
                .consistent_read(request.consistent);
            if is_count {
                scan = scan.select(Select::Count);
            }
            if let Some(index_name) = index_name {
                scan = scan.index_name(index_name);
            }
            if let Some(filter_condition) = filter_condition {
                scan = scan
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
            if let Some(rendered) = self.read_projection(table, request)? {
                scan = scan
                    .projection_expression(rendered.expression.clone())
                    .set_expression_attribute_names(names_to_hash(
                        rendered.attribute_names.clone(),
                    ));
            }
            if let Some(last_key) = &last_key {
                scan = scan.set_exclusive_start_key(Some(last_key.clone()));
            }
            let remaining =
                item_limit.saturating_sub(if is_count { total_count } else { items.len() });
            if remaining == 0 {
                break;
            }
            scan = scan.limit((remaining.min(100)) as i32);
            let response = self
                .block_on(async { scan.send().await })
                .map_err(Self::aws_error)?;
            scanned += response.scanned_count() as usize;
            if is_count {
                total_count += response.count() as usize;
            } else {
                for item in response.items() {
                    items.push(attributes_to_item(item)?);
                    if items.len() >= item_limit {
                        break;
                    }
                }
            }
            last_key = response.last_evaluated_key().cloned();
            let fetched = if is_count { total_count } else { items.len() };
            if fetched >= item_limit || last_key.is_none() || scanned >= scan_limit {
                break;
            }
        }
        if is_count {
            items = (0..total_count).map(|_| Item::new()).collect();
        }
        Ok(items)
    }

    fn read_projection(
        &self,
        table: &str,
        request: &ReadRequest<'_>,
    ) -> Result<Option<RenderedExpression>, EngineError> {
        if matches!(request.selection, Selection::CountAll) {
            return Ok(None);
        }
        if request.follow_up_batch_get {
            let meta = self
                .describe_table(table)?
                .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
            return Ok(Some(render_field_list_projection(
                &meta.primary_key_attributes(),
            )));
        }
        match request.selection {
            Selection::Items(_) => {
                let rendered = render_projection(request.selection);
                Ok((!rendered.expression.is_empty()).then_some(rendered))
            }
            _ => Ok(None),
        }
    }

    fn batch_get_items(
        &self,
        table: &str,
        partial_items: &[Item],
        consistent: bool,
        selection: Option<&Selection>,
    ) -> Result<Vec<Item>, EngineError> {
        let meta = self
            .describe_table(table)?
            .ok_or_else(|| EngineError::Runtime(format!("Table '{table}' not found")))?;
        let projection = selection.and_then(|selection| {
            let rendered = render_projection(selection);
            (!rendered.expression.is_empty()).then_some(rendered)
        });
        let mut results = Vec::new();
        for chunk in partial_items.chunks(BATCH_GET_CHUNK) {
            let keys = chunk
                .iter()
                .map(|item| primary_key_from_meta(&meta, item))
                .collect::<Result<Vec<_>, _>>()?;
            let mut keys_and_attributes = aws_sdk_dynamodb::types::KeysAndAttributes::builder()
                .set_keys(Some(keys))
                .consistent_read(consistent);
            if let Some(rendered) = &projection {
                keys_and_attributes = keys_and_attributes
                    .projection_expression(rendered.expression.clone())
                    .set_expression_attribute_names(names_to_hash(
                        rendered.attribute_names.clone(),
                    ));
            }
            let keys_and_attributes = keys_and_attributes
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
            if let Some(items) = response
                .responses()
                .and_then(|responses| responses.get(table))
            {
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
            access_key, secret_key, None, None, "dql",
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

fn resolve_alter_throughput(
    throughput: &dql_parser::Throughput,
    meta: &TableMeta,
    index: Option<&str>,
) -> Result<(i64, i64), EngineError> {
    let current = if let Some(index_name) = index {
        meta.global_indexes
            .get(index_name)
            .and_then(|index| index.throughput.as_ref())
    } else {
        meta.throughput.as_ref()
    };
    let read = resolve_alter_throughput_value(&throughput.read, current.map(|value| &value.read))?;
    let write =
        resolve_alter_throughput_value(&throughput.write, current.map(|value| &value.write))?;
    Ok((read, write))
}

fn resolve_alter_throughput_value(
    value: &dql_parser::Value,
    current: Option<&dql_parser::Value>,
) -> Result<i64, EngineError> {
    match value {
        dql_parser::Value::String(star) if star == "*" => current
            .and_then(|value| match value {
                dql_parser::Value::Number(number) => number.parse().ok(),
                _ => None,
            })
            .ok_or_else(|| EngineError::Runtime("missing throughput for '*'".to_string())),
        dql_parser::Value::Number(number) => number
            .parse::<i64>()
            .map_err(|err: std::num::ParseIntError| EngineError::Runtime(err.to_string())),
        _ => Err(EngineError::Runtime("invalid throughput value".to_string())),
    }
}

fn render_field_list_projection(fields: &[String]) -> RenderedExpression {
    let mut visitor = Visitor::with_default_reserved_words();
    let expression = fields
        .iter()
        .map(|field| visitor.get_field(field))
        .collect::<Vec<_>>()
        .join(", ");
    RenderedExpression {
        expression,
        attribute_names: visitor.attribute_names(),
        expression_values: visitor.expression_values(),
    }
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
    key.insert(meta.hash_key.clone(), {
        let mut single = Item::new();
        single.insert(meta.hash_key.clone(), hash.clone());
        item_to_attributes(&single)?.remove(&meta.hash_key).unwrap()
    });
    if let Some(range_key) = &meta.range_key {
        let value = item
            .get(range_key)
            .ok_or_else(|| EngineError::Runtime(format!("missing range key '{range_key}'")))?;
        key.insert(range_key.clone(), {
            let mut single = Item::new();
            single.insert(range_key.clone(), value.clone());
            item_to_attributes(&single)?.remove(range_key).unwrap()
        });
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
