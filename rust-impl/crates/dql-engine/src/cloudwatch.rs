use crate::aws::SdkConfig;
use crate::EngineError;
use aws_sdk_cloudwatch::types::{Dimension, Statistic};
use aws_sdk_cloudwatch::Client;
use dql_models::{ConsumedCapacity, TableMeta};
use std::time::{SystemTime, UNIX_EPOCH};

/// Fetch consumed RCU/WCU for a table (and optionally a GSI) over a 3-minute window.
pub async fn get_capacity(
    config: &SdkConfig,
    tablename: &str,
    index_name: Option<&str>,
) -> Result<(f64, f64), EngineError> {
    // DynamoDB Local has no CloudWatch metrics.
    if config.host.is_some() {
        return Ok((0.0, 0.0));
    }
    match get_capacity_inner(config, tablename, index_name).await {
        Ok(values) => Ok(values),
        Err(_) => Ok((0.0, 0.0)),
    }
}

/// Attach CloudWatch consumed capacity to `meta` (table + GSIs).
pub async fn attach_metrics(config: &SdkConfig, meta: &mut TableMeta) -> Result<(), EngineError> {
    let (read, write) = get_capacity(config, &meta.name, None).await?;
    meta.consumed_capacity
        .insert("__table__".to_string(), ConsumedCapacity { read, write });
    let index_names: Vec<String> = meta.global_indexes.keys().cloned().collect();
    for index_name in index_names {
        let (read, write) = get_capacity(config, &meta.name, Some(&index_name)).await?;
        meta.consumed_capacity
            .insert(index_name, ConsumedCapacity { read, write });
    }
    Ok(())
}

async fn get_capacity_inner(
    config: &SdkConfig,
    tablename: &str,
    index_name: Option<&str>,
) -> Result<(f64, f64), EngineError> {
    let client = build_client(config).await?;
    let read = get_metric(&client, "ConsumedReadCapacityUnits", tablename, index_name).await?;
    let write = get_metric(&client, "ConsumedWriteCapacityUnits", tablename, index_name).await?;
    Ok((read, write))
}

async fn build_client(config: &SdkConfig) -> Result<Client, EngineError> {
    let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new(config.region.clone()));
    if let (Some(access_key), Some(secret_key)) = (&config.access_key, &config.secret_key) {
        loader = loader.credentials_provider(aws_credential_types::Credentials::new(
            access_key, secret_key, None, None, "dql",
        ));
    }
    let shared = loader.load().await;
    Ok(Client::new(&shared))
}

async fn get_metric(
    client: &Client,
    metric: &str,
    tablename: &str,
    index_name: Option<&str>,
) -> Result<f64, EngineError> {
    let end = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| EngineError::Runtime(err.to_string()))?
        .as_secs() as i64;
    let begin = end - 3 * 60;
    let period = 60i32;
    let mut dimensions = vec![Dimension::builder()
        .name("TableName")
        .value(tablename)
        .build()];
    if let Some(index_name) = index_name {
        dimensions.push(
            Dimension::builder()
                .name("GlobalSecondaryIndexName")
                .value(index_name)
                .build(),
        );
    }
    let data = client
        .get_metric_statistics()
        .namespace("AWS/DynamoDB")
        .metric_name(metric)
        .start_time(aws_sdk_cloudwatch::primitives::DateTime::from_secs(begin))
        .end_time(aws_sdk_cloudwatch::primitives::DateTime::from_secs(end))
        .period(period)
        .statistics(Statistic::Sum)
        .set_dimensions(Some(dimensions))
        .send()
        .await
        .map_err(|err| EngineError::Runtime(err.to_string()))?;
    let mut points = data.datapoints.unwrap_or_default();
    if points.is_empty() {
        return Ok(0.0);
    }
    points.sort_by_key(|point| point.timestamp.map(|ts| ts.secs()).unwrap_or(0));
    let sum = points.last().and_then(|point| point.sum).unwrap_or(0.0);
    Ok(sum / f64::from(period))
}
