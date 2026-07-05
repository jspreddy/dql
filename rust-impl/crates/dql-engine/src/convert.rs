use crate::{EngineError, Item};
use aws_sdk_dynamodb::types::{
    AttributeDefinition, AttributeValue, BillingMode as AwsBillingMode, GlobalSecondaryIndex,
    KeySchemaElement, KeyType as AwsKeyType, LocalSecondaryIndex, Projection, ProjectionType,
    ProvisionedThroughput, ProvisionedThroughputDescription, ScalarAttributeType, TableDescription,
};
use dql_expr::{dynamo_to_value, value_to_dynamo, DynamoValue};
use dql_models::{
    BillingMode, GlobalIndexMeta, ProjectionType as ModelProjectionType, QueryIndex, TableField,
    TableMeta, TableStatus,
};
use dql_parser::{AttributeType, KeyType, Throughput, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub fn item_to_attributes(item: &Item) -> Result<HashMap<String, AttributeValue>, EngineError> {
    item.iter()
        .map(|(key, value)| {
            value_to_dynamo(value)
                .map(|dynamo| (key.clone(), dynamo_to_attribute(&dynamo)))
                .map_err(|err| EngineError::Runtime(err.to_string()))
        })
        .collect()
}

pub fn attributes_to_item(
    attributes: &HashMap<String, AttributeValue>,
) -> Result<Item, EngineError> {
    attributes
        .iter()
        .map(|(key, value)| {
            attribute_to_dynamo(value)
                .and_then(|dynamo| {
                    dynamo_to_value(&dynamo).map_err(|err| EngineError::Runtime(err.to_string()))
                })
                .map(|parsed| (key.clone(), parsed))
                .map_err(|err| EngineError::Runtime(err.to_string()))
        })
        .collect()
}

pub fn table_meta_from_description(table: &TableDescription) -> Result<TableMeta, EngineError> {
    let name = table
        .table_name()
        .ok_or_else(|| EngineError::Runtime("missing table name".to_string()))?
        .to_string();
    let mut attrs = BTreeMap::new();
    for definition in table.attribute_definitions() {
        let attr_name = definition.attribute_name().to_string();
        let data_type = scalar_type_to_attribute_type(definition.attribute_type());
        attrs.insert(
            attr_name.clone(),
            TableField {
                name: attr_name,
                data_type,
                key_type: None,
            },
        );
    }
    for key in table.key_schema() {
        let attr_name = key.attribute_name();
        if let Some(field) = attrs.get_mut(attr_name) {
            field.key_type = Some(match key.key_type() {
                AwsKeyType::Hash => KeyType::Hash,
                AwsKeyType::Range => KeyType::Range,
                _ => continue,
            });
        }
    }
    let hash_key = attrs
        .values()
        .find(|field| field.key_type == Some(KeyType::Hash))
        .map(|field| field.name.clone())
        .ok_or_else(|| EngineError::Runtime(format!("table '{name}' missing hash key")))?;
    let range_key = attrs
        .values()
        .find(|field| field.key_type == Some(KeyType::Range))
        .map(|field| field.name.clone());
    let mut local_indexes = BTreeMap::new();
    for index in table.local_secondary_indexes() {
        let index_name = index
            .index_name()
            .ok_or_else(|| EngineError::Runtime("missing local index name".to_string()))?
            .to_string();
        let index_range_key = index
            .key_schema()
            .iter()
            .find(|key| key.key_type() == &AwsKeyType::Range)
            .map(|key| key.attribute_name().to_string());
        local_indexes.insert(
            index_name.clone(),
            QueryIndex {
                name: index_name,
                is_global: false,
                hash_key: hash_key.clone(),
                range_key: index_range_key.clone(),
                projected_attributes: projection_to_attributes(
                    index.projection(),
                    &hash_key,
                    index_range_key.as_deref(),
                ),
            },
        );
    }
    let mut global_indexes = BTreeMap::new();
    for index in table.global_secondary_indexes() {
        let index_name = index
            .index_name()
            .ok_or_else(|| EngineError::Runtime("missing global index name".to_string()))?
            .to_string();
        let hash = index
            .key_schema()
            .iter()
            .find(|key| key.key_type() == &AwsKeyType::Hash)
            .map(|key| key.attribute_name().to_string())
            .unwrap_or_default();
        let range = index
            .key_schema()
            .iter()
            .find(|key| key.key_type() == &AwsKeyType::Range)
            .map(|key| key.attribute_name().to_string());
        global_indexes.insert(
            index_name.clone(),
            GlobalIndexMeta {
                name: index_name,
                hash_key: TableField {
                    name: hash.clone(),
                    data_type: attrs
                        .get(&hash)
                        .map(|field| field.data_type.clone())
                        .unwrap_or(AttributeType::Other("UNKNOWN".to_string())),
                    key_type: Some(KeyType::Hash),
                },
                range_key: range.as_ref().map(|name| TableField {
                    name: name.clone(),
                    data_type: attrs
                        .get(name)
                        .map(|field| field.data_type.clone())
                        .unwrap_or(AttributeType::Other("UNKNOWN".to_string())),
                    key_type: Some(KeyType::Range),
                }),
                projection: aws_projection_to_model(index.projection()),
                throughput: index
                    .provisioned_throughput()
                    .and_then(throughput_from_description),
                status: index
                    .index_status()
                    .map(index_status_to_model)
                    .unwrap_or(TableStatus::Active),
            },
        );
    }
    let billing_mode = match table
        .billing_mode_summary()
        .and_then(|summary| summary.billing_mode())
    {
        Some(AwsBillingMode::PayPerRequest) => BillingMode::OnDemand,
        _ => BillingMode::Provisioned,
    };
    Ok(TableMeta {
        name,
        attrs,
        hash_key,
        range_key,
        local_indexes,
        global_indexes,
        throughput: table
            .provisioned_throughput()
            .and_then(throughput_from_description),
        billing_mode,
        status: table
            .table_status()
            .map(table_status_to_model)
            .unwrap_or(TableStatus::Active),
    })
}

pub fn build_create_table_input(
    meta: &TableMeta,
) -> Result<aws_sdk_dynamodb::operation::create_table::builders::CreateTableInputBuilder, EngineError>
{
    let attribute_definitions = meta
        .attrs
        .values()
        .map(|field| {
            AttributeDefinition::builder()
                .attribute_name(field.name.clone())
                .attribute_type(attribute_type_to_scalar(&field.data_type))
                .build()
                .map_err(|err| EngineError::Runtime(err.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut key_schema = vec![KeySchemaElement::builder()
        .attribute_name(meta.hash_key.clone())
        .key_type(AwsKeyType::Hash)
        .build()
        .map_err(|err| EngineError::Runtime(err.to_string()))?];
    if let Some(range_key) = &meta.range_key {
        key_schema.push(
            KeySchemaElement::builder()
                .attribute_name(range_key.clone())
                .key_type(AwsKeyType::Range)
                .build()
                .map_err(|err| EngineError::Runtime(err.to_string()))?,
        );
    }
    let mut builder = aws_sdk_dynamodb::operation::create_table::CreateTableInput::builder()
        .table_name(meta.name.clone())
        .set_attribute_definitions(Some(attribute_definitions))
        .set_key_schema(Some(key_schema));
    if meta.billing_mode == BillingMode::OnDemand {
        builder = builder.billing_mode(AwsBillingMode::PayPerRequest);
    } else if let Some(throughput) = throughput_to_aws(meta.throughput.as_ref()) {
        builder = builder.provisioned_throughput(throughput);
    }
    if !meta.local_indexes.is_empty() {
        let indexes = meta
            .local_indexes
            .values()
            .filter_map(|index| {
                let range_key = index.range_key.as_ref()?;
                Some(
                    LocalSecondaryIndex::builder()
                        .index_name(index.name.clone())
                        .key_schema(
                            KeySchemaElement::builder()
                                .attribute_name(index.hash_key.clone())
                                .key_type(AwsKeyType::Hash)
                                .build()
                                .ok()?,
                        )
                        .key_schema(
                            KeySchemaElement::builder()
                                .attribute_name(range_key.clone())
                                .key_type(AwsKeyType::Range)
                                .build()
                                .ok()?,
                        )
                        .projection(projection_from_query_index(index))
                        .build()
                        .ok()?,
                )
            })
            .collect::<Vec<_>>();
        if !indexes.is_empty() {
            builder = builder.set_local_secondary_indexes(Some(indexes));
        }
    }
    if !meta.global_indexes.is_empty() {
        let indexes = meta
            .global_indexes
            .values()
            .map(|index| {
                let mut key_schema = vec![KeySchemaElement::builder()
                    .attribute_name(index.hash_key.name.clone())
                    .key_type(AwsKeyType::Hash)
                    .build()
                    .map_err(|err| EngineError::Runtime(err.to_string()))?];
                if let Some(range_key) = &index.range_key {
                    key_schema.push(
                        KeySchemaElement::builder()
                            .attribute_name(range_key.name.clone())
                            .key_type(AwsKeyType::Range)
                            .build()
                            .map_err(|err| EngineError::Runtime(err.to_string()))?,
                    );
                }
                let mut gsi = GlobalSecondaryIndex::builder()
                    .index_name(index.name.clone())
                    .set_key_schema(Some(key_schema))
                    .projection(model_projection_to_aws(&index.projection));
                if let Some(throughput) = throughput_to_aws(index.throughput.as_ref()) {
                    gsi = gsi.provisioned_throughput(throughput);
                }
                gsi.build()
                    .map_err(|err| EngineError::Runtime(err.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        builder = builder.set_global_secondary_indexes(Some(indexes));
    }
    Ok(builder)
}

pub fn expression_values_to_attributes(
    values: &BTreeMap<String, DynamoValue>,
) -> Result<HashMap<String, AttributeValue>, EngineError> {
    values
        .iter()
        .map(|(key, value)| Ok((key.clone(), dynamo_to_attribute(value))))
        .collect()
}

pub fn dynamo_to_attribute(value: &DynamoValue) -> AttributeValue {
    match value {
        DynamoValue::Null(_) => AttributeValue::Null(true),
        DynamoValue::Bool(value) => AttributeValue::Bool(*value),
        DynamoValue::Number(value) => AttributeValue::N(value.clone()),
        DynamoValue::String(value) => AttributeValue::S(value.clone()),
        DynamoValue::Binary(value) => AttributeValue::B(aws_sdk_dynamodb::primitives::Blob::new(
            base64_decode(value).unwrap_or_default(),
        )),
        DynamoValue::NumberSet(values) => AttributeValue::Ns(values.clone()),
        DynamoValue::StringSet(values) => AttributeValue::Ss(values.clone()),
        DynamoValue::BinarySet(values) => AttributeValue::Bs(
            values
                .iter()
                .filter_map(|value| {
                    Some(aws_sdk_dynamodb::primitives::Blob::new(
                        base64_decode(value).ok()?,
                    ))
                })
                .collect(),
        ),
        DynamoValue::List(values) => {
            AttributeValue::L(values.iter().map(dynamo_to_attribute).collect())
        }
        DynamoValue::Map(values) => AttributeValue::M(
            values
                .iter()
                .map(|(key, value)| (key.clone(), dynamo_to_attribute(value)))
                .collect::<HashMap<_, _>>(),
        ),
    }
}

pub fn attribute_to_dynamo(value: &AttributeValue) -> Result<DynamoValue, EngineError> {
    Ok(match value {
        AttributeValue::Null(_) => DynamoValue::Null(true),
        AttributeValue::Bool(value) => DynamoValue::Bool(*value),
        AttributeValue::N(value) => DynamoValue::Number(value.clone()),
        AttributeValue::S(value) => DynamoValue::String(value.clone()),
        AttributeValue::B(value) => DynamoValue::Binary(base64_encode(value.as_ref())),
        AttributeValue::Ns(values) => DynamoValue::NumberSet(values.clone()),
        AttributeValue::Ss(values) => DynamoValue::StringSet(values.clone()),
        AttributeValue::Bs(values) => DynamoValue::BinarySet(
            values
                .iter()
                .map(|value| base64_encode(value.as_ref()))
                .collect(),
        ),
        AttributeValue::L(values) => DynamoValue::List(
            values
                .iter()
                .map(attribute_to_dynamo)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        AttributeValue::M(values) => DynamoValue::Map(
            values
                .iter()
                .map(|(key, value)| attribute_to_dynamo(value).map(|parsed| (key.clone(), parsed)))
                .collect::<Result<BTreeMap<_, _>, _>>()?,
        ),
        _ => {
            return Err(EngineError::Runtime(
                "unsupported DynamoDB attribute value".to_string(),
            ))
        }
    })
}

fn scalar_type_to_attribute_type(value: &ScalarAttributeType) -> AttributeType {
    match value {
        ScalarAttributeType::S => AttributeType::String,
        ScalarAttributeType::N => AttributeType::Number,
        ScalarAttributeType::B => AttributeType::Binary,
        _ => AttributeType::Other("UNKNOWN".to_string()),
    }
}

fn attribute_type_to_scalar(value: &AttributeType) -> ScalarAttributeType {
    match value {
        AttributeType::String => ScalarAttributeType::S,
        AttributeType::Number => ScalarAttributeType::N,
        AttributeType::Binary => ScalarAttributeType::B,
        AttributeType::Bool => ScalarAttributeType::S,
        AttributeType::Other(value) if value.eq_ignore_ascii_case("STRING") => {
            ScalarAttributeType::S
        }
        AttributeType::Other(value) if value.eq_ignore_ascii_case("NUMBER") => {
            ScalarAttributeType::N
        }
        AttributeType::Other(value) if value.eq_ignore_ascii_case("BINARY") => {
            ScalarAttributeType::B
        }
        AttributeType::Other(_) => ScalarAttributeType::S,
    }
}

fn throughput_from_description(
    throughput: &ProvisionedThroughputDescription,
) -> Option<Throughput> {
    Some(Throughput {
        read: Value::Number(throughput.read_capacity_units()?.to_string()),
        write: Value::Number(throughput.write_capacity_units()?.to_string()),
    })
}

fn throughput_from_aws(throughput: &ProvisionedThroughput) -> Option<Throughput> {
    Some(Throughput {
        read: Value::Number(throughput.read_capacity_units().to_string()),
        write: Value::Number(throughput.write_capacity_units().to_string()),
    })
}

fn throughput_to_aws(throughput: Option<&Throughput>) -> Option<ProvisionedThroughput> {
    let throughput = throughput?;
    let read = match &throughput.read {
        Value::Number(value) => value.parse::<i64>().ok()?,
        _ => return None,
    };
    let write = match &throughput.write {
        Value::Number(value) => value.parse::<i64>().ok()?,
        _ => return None,
    };
    ProvisionedThroughput::builder()
        .read_capacity_units(read)
        .write_capacity_units(write)
        .build()
        .ok()
}

fn table_status_to_model(status: &aws_sdk_dynamodb::types::TableStatus) -> TableStatus {
    use aws_sdk_dynamodb::types::TableStatus as AwsStatus;
    match status {
        AwsStatus::Active => TableStatus::Active,
        AwsStatus::Creating => TableStatus::Creating,
        AwsStatus::Updating => TableStatus::Updating,
        AwsStatus::Deleting => TableStatus::Deleting,
        _ => TableStatus::Active,
    }
}

fn index_status_to_model(status: &aws_sdk_dynamodb::types::IndexStatus) -> TableStatus {
    use aws_sdk_dynamodb::types::IndexStatus as AwsStatus;
    match status {
        AwsStatus::Active => TableStatus::Active,
        AwsStatus::Creating => TableStatus::Creating,
        AwsStatus::Updating => TableStatus::Updating,
        AwsStatus::Deleting => TableStatus::Deleting,
        _ => TableStatus::Active,
    }
}

fn aws_projection_to_model(projection: Option<&Projection>) -> ModelProjectionType {
    let Some(projection) = projection else {
        return ModelProjectionType::All;
    };
    match projection.projection_type() {
        Some(ProjectionType::KeysOnly) => ModelProjectionType::KeysOnly,
        Some(ProjectionType::Include) => {
            ModelProjectionType::Include(projection.non_key_attributes().iter().cloned().collect())
        }
        _ => ModelProjectionType::All,
    }
}

fn model_projection_to_aws(projection: &ModelProjectionType) -> Projection {
    match projection {
        ModelProjectionType::All => Projection::builder()
            .projection_type(ProjectionType::All)
            .build(),
        ModelProjectionType::KeysOnly => Projection::builder()
            .projection_type(ProjectionType::KeysOnly)
            .build(),
        ModelProjectionType::Include(values) => Projection::builder()
            .projection_type(ProjectionType::Include)
            .set_non_key_attributes(Some(values.clone()))
            .build(),
    }
}

fn projection_from_query_index(index: &QueryIndex) -> Projection {
    match &index.projected_attributes {
        None => Projection::builder()
            .projection_type(ProjectionType::All)
            .build(),
        Some(values) => {
            let mut key_attrs = BTreeSet::from([index.hash_key.clone()]);
            if let Some(range_key) = &index.range_key {
                key_attrs.insert(range_key.clone());
            }
            if values.len() == key_attrs.len()
                && values.iter().all(|value| key_attrs.contains(value))
            {
                Projection::builder()
                    .projection_type(ProjectionType::KeysOnly)
                    .build()
            } else {
                Projection::builder()
                    .projection_type(ProjectionType::Include)
                    .set_non_key_attributes(Some(
                        values
                            .iter()
                            .filter(|value| !key_attrs.contains(*value))
                            .cloned()
                            .collect(),
                    ))
                    .build()
            }
        }
    }
}

fn projection_to_attributes(
    projection: Option<&Projection>,
    hash_key: &str,
    range_key: Option<&str>,
) -> Option<BTreeSet<String>> {
    match projection.and_then(|projection| projection.projection_type()) {
        Some(ProjectionType::All) | None => None,
        Some(ProjectionType::KeysOnly) => {
            let mut attrs = BTreeSet::from([hash_key.to_string()]);
            if let Some(range_key) = range_key {
                attrs.insert(range_key.to_string());
            }
            Some(attrs)
        }
        Some(ProjectionType::Include) => {
            let mut attrs = BTreeSet::from([hash_key.to_string()]);
            if let Some(range_key) = range_key {
                attrs.insert(range_key.to_string());
            }
            if let Some(projection) = projection {
                attrs.extend(projection.non_key_attributes().iter().cloned());
            }
            Some(attrs)
        }
        Some(_) => None,
    }
}

fn base64_encode(input: &[u8]) -> String {
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

fn base64_decode(input: &str) -> Result<Vec<u8>, EngineError> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut decode = [0u8; 256];
    for (index, ch) in ALPHABET.iter().enumerate() {
        decode[*ch as usize] = index as u8;
    }
    let input = input.trim_end_matches('=');
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for ch in input.bytes() {
        let value = decode
            .get(ch as usize)
            .copied()
            .ok_or_else(|| EngineError::Runtime(format!("invalid base64 character '{ch}'")))?;
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Ok(output)
}

pub fn keys_in_to_items(
    meta: &TableMeta,
    keys_in: &[Vec<Value>],
) -> Result<Vec<Item>, EngineError> {
    let pk_attrs = meta.primary_key_attributes();
    keys_in
        .iter()
        .map(|key_values| {
            if key_values.len() != pk_attrs.len() {
                return Err(EngineError::Runtime(format!(
                    "Primary key {key_values:?} does not match table key schema {pk_attrs:?}"
                )));
            }
            let mut item = Item::new();
            for (attr, value) in pk_attrs.iter().zip(key_values.iter()) {
                item.insert(attr.clone(), value.clone());
            }
            Ok(item)
        })
        .collect()
}

pub fn item_matches_primary_key(item: &Item, key: &Item, meta: &TableMeta) -> bool {
    item.get(&meta.hash_key) == key.get(&meta.hash_key)
        && meta
            .range_key
            .as_ref()
            .map(|range_key| item.get(range_key) == key.get(range_key))
            .unwrap_or(true)
}
