use dql_parser::{
    AttributeType, CompareOp, Condition, ConditionOperand, KeyType, ProjectionKind, Selection,
    Statement, Throughput, Value,
};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelError {
    message: String,
}

impl ModelError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for ModelError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BillingMode {
    Provisioned,
    OnDemand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableStatus {
    Active,
    Creating,
    Updating,
    Deleting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionType {
    All,
    KeysOnly,
    Include(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableField {
    pub name: String,
    pub data_type: AttributeType,
    pub key_type: Option<KeyType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlobalIndexMeta {
    pub name: String,
    pub hash_key: TableField,
    pub range_key: Option<TableField>,
    pub projection: ProjectionType,
    pub throughput: Option<Throughput>,
    pub status: TableStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryIndex {
    pub name: String,
    pub is_global: bool,
    pub hash_key: String,
    pub range_key: Option<String>,
    pub projection: ProjectionType,
    pub projected_attributes: Option<BTreeSet<String>>,
}

impl QueryIndex {
    pub fn projects_all_attributes(&self, attrs: Option<&BTreeSet<String>>) -> bool {
        let Some(projected) = &self.projected_attributes else {
            return true;
        };
        let Some(attrs) = attrs else {
            return false;
        };
        attrs.iter().all(|attr| projected.contains(attr))
    }

    pub fn scannable(&self) -> bool {
        self.is_global
    }

    pub fn primary_key_attributes(&self) -> Vec<String> {
        let mut attrs = vec![self.hash_key.clone()];
        if let Some(range_key) = &self.range_key {
            attrs.push(range_key.clone());
        }
        attrs
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConsumedCapacity {
    pub read: f64,
    pub write: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableMeta {
    pub name: String,
    pub attrs: BTreeMap<String, TableField>,
    pub hash_key: String,
    pub range_key: Option<String>,
    pub local_indexes: BTreeMap<String, QueryIndex>,
    pub global_indexes: BTreeMap<String, GlobalIndexMeta>,
    pub throughput: Option<Throughput>,
    pub billing_mode: BillingMode,
    pub status: TableStatus,
    /// CloudWatch consumed capacity keyed by `"__table__"` or GSI name.
    pub consumed_capacity: BTreeMap<String, ConsumedCapacity>,
}

impl TableMeta {
    pub fn from_create_statement(statement: &Statement) -> Result<Self, ModelError> {
        let Statement::CreateTable {
            name,
            attributes,
            throughput,
            global_indexes,
            ..
        } = statement
        else {
            return Err(ModelError::new("expected CREATE TABLE statement"));
        };
        let hash_key = attributes
            .iter()
            .find(|attr| attr.key_type == Some(KeyType::Hash))
            .map(|attr| attr.name.clone())
            .ok_or_else(|| ModelError::new("missing hash key"))?;
        let range_key = attributes
            .iter()
            .find(|attr| attr.key_type == Some(KeyType::Range))
            .map(|attr| attr.name.clone());
        let attrs = attributes
            .iter()
            .map(|attr| {
                (
                    attr.name.clone(),
                    TableField {
                        name: attr.name.clone(),
                        data_type: attr.attr_type.clone(),
                        key_type: attr.key_type.clone(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let local_indexes = attributes
            .iter()
            .filter_map(|attr| {
                attr.local_index.as_ref().map(|index| {
                    let projection =
                        projection_from_kind(&index.projection, index.includes.clone());
                    let query_index = QueryIndex {
                        name: index.name.clone(),
                        is_global: false,
                        hash_key: hash_key.clone(),
                        range_key: Some(attr.name.clone()),
                        projection: projection.clone(),
                        projected_attributes: projection_attributes(
                            &hash_key,
                            range_key.as_deref(),
                            Some(attr.name.as_str()),
                            None,
                            &projection,
                        ),
                    };
                    (index.name.clone(), query_index)
                })
            })
            .collect::<BTreeMap<_, _>>();
        let global_indexes = global_indexes
            .iter()
            .map(|index| {
                let hash_type = index.hash_key_type.clone().or_else(|| {
                    attrs
                        .get(&index.hash_key)
                        .map(|field| field.data_type.clone())
                });
                let range_type = index.range_key.as_ref().and_then(|name| {
                    index
                        .range_key_type
                        .clone()
                        .or_else(|| attrs.get(name).map(|field| field.data_type.clone()))
                });
                (
                    index.name.clone(),
                    GlobalIndexMeta {
                        name: index.name.clone(),
                        hash_key: TableField {
                            name: index.hash_key.clone(),
                            data_type: hash_type
                                .clone()
                                .unwrap_or(AttributeType::Other("UNKNOWN".to_string())),
                            key_type: Some(KeyType::Hash),
                        },
                        range_key: index.range_key.as_ref().map(|range_key| TableField {
                            name: range_key.clone(),
                            data_type: range_type
                                .clone()
                                .unwrap_or(AttributeType::Other("UNKNOWN".to_string())),
                            key_type: Some(KeyType::Range),
                        }),
                        projection: projection_from_kind(&index.projection, index.includes.clone()),
                        throughput: index.throughput.clone(),
                        status: TableStatus::Active,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut attrs = attrs;
        for index in global_indexes.values() {
            if !attrs.contains_key(&index.hash_key.name) {
                attrs.insert(index.hash_key.name.clone(), index.hash_key.clone());
            }
            if let Some(range_key) = &index.range_key {
                if !attrs.contains_key(&range_key.name) {
                    attrs.insert(range_key.name.clone(), range_key.clone());
                }
            }
        }
        Ok(Self {
            name: name.clone(),
            attrs,
            hash_key,
            range_key,
            local_indexes,
            global_indexes,
            throughput: throughput.clone(),
            billing_mode: if throughput.is_none() || is_on_demand(throughput) {
                BillingMode::OnDemand
            } else {
                BillingMode::Provisioned
            },
            status: TableStatus::Active,
            consumed_capacity: BTreeMap::new(),
        })
    }

    pub fn iter_query_indexes(&self) -> Vec<QueryIndex> {
        let mut indexes = vec![QueryIndex {
            name: "TABLE".to_string(),
            is_global: true,
            hash_key: self.hash_key.clone(),
            range_key: self.range_key.clone(),
            projection: ProjectionType::All,
            projected_attributes: None,
        }];
        indexes.extend(self.local_indexes.values().cloned());
        indexes.extend(self.global_indexes.values().map(|index| QueryIndex {
            name: index.name.clone(),
            is_global: true,
            hash_key: index.hash_key.name.clone(),
            range_key: index.range_key.as_ref().map(|field| field.name.clone()),
            projection: index.projection.clone(),
            projected_attributes: projection_attributes(
                &self.hash_key,
                self.range_key.as_deref(),
                Some(&index.hash_key.name),
                index.range_key.as_ref().map(|field| field.name.as_str()),
                &index.projection,
            ),
        }));
        indexes
    }

    pub fn get_index(&self, name: &str) -> Result<QueryIndex, ModelError> {
        self.iter_query_indexes()
            .into_iter()
            .find(|index| index.name == name)
            .ok_or_else(|| ModelError::new(format!("unknown index '{name}'")))
    }

    pub fn get_matching_indexes(
        &self,
        possible_hash: &BTreeSet<String>,
        possible_range: &BTreeSet<String>,
    ) -> Vec<QueryIndex> {
        let matches = self
            .iter_query_indexes()
            .into_iter()
            .filter(|index| possible_hash.contains(&index.hash_key))
            .collect::<Vec<_>>();
        let range_matches = matches
            .iter()
            .filter(|index| {
                index
                    .range_key
                    .as_ref()
                    .is_some_and(|range| possible_range.contains(range))
            })
            .cloned()
            .collect::<Vec<_>>();
        if range_matches.is_empty() {
            matches
        } else {
            range_matches
        }
    }

    pub fn total_read_throughput(&self) -> Option<f64> {
        self.throughput_value(true).map(|table| {
            table
                + self
                    .global_indexes
                    .values()
                    .filter_map(|index| throughput_number(index.throughput.as_ref(), true))
                    .sum::<f64>()
        })
    }

    pub fn total_write_throughput(&self) -> Option<f64> {
        self.throughput_value(false).map(|table| {
            table
                + self
                    .global_indexes
                    .values()
                    .filter_map(|index| throughput_number(index.throughput.as_ref(), false))
                    .sum::<f64>()
        })
    }

    pub fn table_read_throughput(&self) -> Option<f64> {
        self.throughput_value(true)
    }

    pub fn table_write_throughput(&self) -> Option<f64> {
        self.throughput_value(false)
    }

    fn throughput_value(&self, read: bool) -> Option<f64> {
        throughput_number(self.throughput.as_ref(), read)
    }

    pub fn schema_dql(&self) -> String {
        let mut attrs = self.attrs.clone();
        let mut field_parts = Vec::new();
        field_parts.push(field_schema(attrs.get(&self.hash_key).expect("hash key")));
        attrs.remove(&self.hash_key);
        if let Some(range_key) = &self.range_key {
            field_parts.push(field_schema(attrs.get(range_key).expect("range key")));
            attrs.remove(range_key);
        }
        field_parts.extend(attrs.values().map(|field| {
            local_index_schema(field, self.local_indexes.values())
                .unwrap_or_else(|| field_schema(field))
        }));
        let mut body = field_parts.join(", ");
        if let Some(throughput) = &self.throughput {
            body.push_str(&format!(
                ", THROUGHPUT ({}, {})",
                throughput_literal(&throughput.read),
                throughput_literal(&throughput.write)
            ));
        }
        let mut output = format!("CREATE TABLE {} ({})", self.name, body);
        for gsi in self.global_indexes.values() {
            let schema = global_index_schema(gsi);
            if !schema.is_empty() {
                output.push(' ');
                output.push_str(&schema);
            }
        }
        format!("{output};")
    }

    pub fn primary_key_attributes(&self) -> Vec<String> {
        let mut attrs = vec![self.hash_key.clone()];
        if let Some(range_key) = &self.range_key {
            attrs.push(range_key.clone());
        }
        attrs
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadKind {
    Select,
    Scan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Query,
    Scan,
    BatchGetKeys,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    SelectScanRejected,
    AmbiguousIndex(Vec<String>),
    CannotScanLocalIndex(String),
    UnknownIndex(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryPlan {
    pub operation: Operation,
    pub index: Option<QueryIndex>,
    pub key_condition: Option<Condition>,
    pub filter_condition: Option<Condition>,
    pub follow_up_batch_get: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanInput<'a> {
    pub table: &'a TableMeta,
    pub kind: ReadKind,
    pub condition: Option<&'a Condition>,
    pub selection: Option<&'a Selection>,
    pub using_index: Option<&'a str>,
    pub allow_select_scan: bool,
}

pub fn plan_read(input: PlanInput<'_>) -> Result<QueryPlan, PlanError> {
    let selection_attrs = input.selection.and_then(selection_fields);
    let scan_plan = |index: Option<QueryIndex>, filter_condition: Option<Condition>| {
        if input.kind == ReadKind::Select && !input.allow_select_scan {
            return Err(PlanError::SelectScanRejected);
        }
        let follow_up_batch_get = index
            .as_ref()
            .is_some_and(|index| !index.projects_all_attributes(selection_attrs.as_ref()));
        Ok(QueryPlan {
            operation: Operation::Scan,
            index,
            key_condition: None,
            filter_condition,
            follow_up_batch_get,
        })
    };
    let Some(condition) = input.condition else {
        if let Some(index_name) = input.using_index {
            if index_name == "-" {
                return scan_plan(None, None);
            }
            let index = input
                .table
                .get_index(index_name)
                .map_err(|_| PlanError::UnknownIndex(index_name.to_string()))?;
            return scan_plan(Some(index), None);
        }
        return scan_plan(None, None);
    };
    let possible_hash = possible_hash_fields(condition);
    let possible_range = possible_range_fields(condition);
    if let Some(index_name) = input.using_index {
        if index_name == "-" {
            return scan_plan(None, Some(condition.clone()));
        }
        let index = input
            .table
            .get_index(index_name)
            .map_err(|_| PlanError::UnknownIndex(index_name.to_string()))?;
        if possible_hash.contains(&index.hash_key) {
            return Ok(query_plan(condition, index, selection_attrs.as_ref()));
        }
        if !index.scannable() {
            return Err(PlanError::CannotScanLocalIndex(index.name));
        }
        return scan_plan(Some(index), Some(condition.clone()));
    }
    let matches = input
        .table
        .get_matching_indexes(&possible_hash, &possible_range);
    match matches.as_slice() {
        [] => scan_plan(None, Some(condition.clone())),
        [index] => Ok(query_plan(
            condition,
            index.clone(),
            selection_attrs.as_ref(),
        )),
        indexes if indexes[0].name == "TABLE" => Ok(query_plan(
            condition,
            indexes[0].clone(),
            selection_attrs.as_ref(),
        )),
        indexes => Err(PlanError::AmbiguousIndex(
            indexes.iter().map(|index| index.name.clone()).collect(),
        )),
    }
}

pub fn possible_hash_fields(condition: &Condition) -> BTreeSet<String> {
    match condition {
        Condition::Compare {
            field,
            op: CompareOp::Eq,
            rhs: ConditionOperand::Value(_),
        } => BTreeSet::from([field.clone()]),
        Condition::And(parts) => parts.iter().flat_map(possible_hash_fields).collect(),
        _ => BTreeSet::new(),
    }
}

pub fn possible_range_fields(condition: &Condition) -> BTreeSet<String> {
    match condition {
        Condition::Compare {
            op: CompareOp::Ne, ..
        } => BTreeSet::new(),
        Condition::Compare {
            field,
            rhs: ConditionOperand::Value(_),
            ..
        }
        | Condition::Between { field, .. } => BTreeSet::from([field.clone()]),
        Condition::Function { name, args } if name.eq_ignore_ascii_case("begins_with") => args
            .first()
            .and_then(|arg| match arg {
                ConditionOperand::Field(field) => Some(BTreeSet::from([field.clone()])),
                ConditionOperand::Value(_) => None,
            })
            .unwrap_or_default(),
        Condition::And(parts) => parts.iter().flat_map(possible_range_fields).collect(),
        _ => BTreeSet::new(),
    }
}

fn query_plan(
    condition: &Condition,
    index: QueryIndex,
    selection_attrs: Option<&BTreeSet<String>>,
) -> QueryPlan {
    let (key_condition, filter_condition) = split_condition_for_index(condition, &index);
    let follow_up_batch_get = !index.projects_all_attributes(selection_attrs);
    QueryPlan {
        operation: Operation::Query,
        index: Some(index),
        key_condition,
        filter_condition,
        follow_up_batch_get,
    }
}

fn split_condition_for_index(
    condition: &Condition,
    index: &QueryIndex,
) -> (Option<Condition>, Option<Condition>) {
    match condition {
        Condition::And(parts) => {
            let mut key = Vec::new();
            let mut filter = Vec::new();
            for part in parts {
                if condition_matches_index(part, index) {
                    key.push(part.clone());
                } else {
                    filter.push(part.clone());
                }
            }
            (combine_conditions(key), combine_conditions(filter))
        }
        other if condition_matches_index(other, index) => (Some(other.clone()), None),
        other => (None, Some(other.clone())),
    }
}

fn condition_matches_index(condition: &Condition, index: &QueryIndex) -> bool {
    match condition {
        Condition::Compare {
            field,
            op: CompareOp::Eq,
            rhs: ConditionOperand::Value(_),
        } if field == &index.hash_key => true,
        Condition::Compare {
            field,
            rhs: ConditionOperand::Value(_),
            ..
        }
        | Condition::Between { field, .. }
            if index.range_key.as_ref().is_some_and(|range| range == field) =>
        {
            true
        }
        Condition::Function { name, args }
            if name.eq_ignore_ascii_case("begins_with")
                && matches!(args.first(), Some(ConditionOperand::Field(field)) if index.range_key.as_ref().is_some_and(|range| range == field)) =>
        {
            true
        }
        _ => false,
    }
}

fn combine_conditions(conditions: Vec<Condition>) -> Option<Condition> {
    match conditions.as_slice() {
        [] => None,
        [condition] => Some(condition.clone()),
        _ => Some(Condition::And(conditions)),
    }
}

fn selection_fields(selection: &Selection) -> Option<BTreeSet<String>> {
    match selection {
        Selection::All => None,
        Selection::CountAll => Some(BTreeSet::new()),
        Selection::Items(items) => Some(
            items
                .iter()
                .flat_map(|item| {
                    item.expression
                        .split(|ch: char| {
                            ch.is_whitespace()
                                || matches!(ch, '+' | '-' | '*' | '/' | '(' | ')' | ',')
                        })
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .filter(|token| !token.is_empty() && token.parse::<f64>().is_err())
                .collect(),
        ),
    }
}

fn projection_from_kind(kind: &ProjectionKind, includes: Vec<String>) -> ProjectionType {
    match kind {
        ProjectionKind::All => ProjectionType::All,
        ProjectionKind::Keys => ProjectionType::KeysOnly,
        ProjectionKind::Include => ProjectionType::Include(includes),
    }
}

fn projection_attributes(
    table_hash: &str,
    table_range: Option<&str>,
    index_hash: Option<&str>,
    index_range: Option<&str>,
    projection: &ProjectionType,
) -> Option<BTreeSet<String>> {
    match projection {
        ProjectionType::All => None,
        ProjectionType::KeysOnly | ProjectionType::Include(_) => {
            let mut attrs = BTreeSet::from([table_hash.to_string()]);
            if let Some(table_range) = table_range {
                attrs.insert(table_range.to_string());
            }
            if let Some(index_hash) = index_hash {
                attrs.insert(index_hash.to_string());
            }
            if let Some(index_range) = index_range {
                attrs.insert(index_range.to_string());
            }
            if let ProjectionType::Include(includes) = projection {
                attrs.extend(includes.iter().cloned());
            }
            Some(attrs)
        }
    }
}

pub fn format_throughput(available: Option<f64>, used: Option<f64>) -> String {
    match (available, used) {
        (Some(0.0), Some(used)) | (None, Some(used)) => format!("{used:.0}/∞"),
        (Some(0.0), None) | (None, None) => "N/A".to_string(),
        (Some(available), None) => format!("{available:.0}"),
        (Some(available), Some(used)) => {
            let percent = used / available * 100.0;
            format!("{used:.0}/{available:.0} ({percent:.0}%)")
        }
    }
}

fn throughput_number(throughput: Option<&Throughput>, read: bool) -> Option<f64> {
    let value = if read {
        throughput?.read.clone()
    } else {
        throughput?.write.clone()
    };
    match value {
        Value::Number(value) => value.parse().ok(),
        _ => None,
    }
}

fn is_on_demand(throughput: &Option<Throughput>) -> bool {
    throughput_number(throughput.as_ref(), true) == Some(0.0)
        && throughput_number(throughput.as_ref(), false) == Some(0.0)
}

fn local_index_schema<'a>(
    field: &TableField,
    indexes: impl IntoIterator<Item = &'a QueryIndex>,
) -> Option<String> {
    let index = indexes
        .into_iter()
        .find(|index| index.range_key.as_deref() == Some(field.name.as_str()))?;
    let type_name = match &field.data_type {
        AttributeType::String => "STRING",
        AttributeType::Number => "NUMBER",
        AttributeType::Binary => "BINARY",
        AttributeType::Bool => "BOOL",
        AttributeType::Other(value) => value.as_str(),
    };
    let projection = match &index.projection {
        ProjectionType::All => format!("INDEX('{}')", index.name),
        ProjectionType::KeysOnly => format!("KEYS INDEX('{}')", index.name),
        ProjectionType::Include(values) => format!(
            "INCLUDE INDEX('{}', [{}])",
            index.name,
            values
                .iter()
                .map(|value| format!("'{value}'"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    Some(format!("{} {type_name} {projection}", field.name))
}

fn field_schema(field: &TableField) -> String {
    let type_name = match &field.data_type {
        AttributeType::String => "STRING",
        AttributeType::Number => "NUMBER",
        AttributeType::Binary => "BINARY",
        AttributeType::Bool => "BOOL",
        AttributeType::Other(value) => value.as_str(),
    };
    match field.key_type {
        Some(KeyType::Hash) => format!("{} {type_name} HASH KEY", field.name),
        Some(KeyType::Range) => format!("{} {type_name} RANGE KEY", field.name),
        None => format!("{} {type_name}", field.name),
    }
}

fn throughput_literal(value: &Value) -> String {
    match value {
        Value::Number(value) => value.clone(),
        Value::String(value) => value.clone(),
        _ => "0".to_string(),
    }
}

fn global_index_schema(index: &GlobalIndexMeta) -> String {
    if matches!(index.status, TableStatus::Deleting) {
        return String::new();
    }
    let hash_type = attribute_type_name(&index.hash_key.data_type);
    let mut body = format!("('{}', {}", index.name, index.hash_key.name);
    if hash_type != "UNKNOWN" {
        body.push(' ');
        body.push_str(&hash_type.to_ascii_lowercase());
    }
    match &index.projection {
        ProjectionType::All => {
            if let Some(range_key) = &index.range_key {
                body.push_str(&format!(", {}", range_key.name));
                let range_type = attribute_type_name(&range_key.data_type);
                if range_type != "UNKNOWN" {
                    body.push(' ');
                    body.push_str(&range_type.to_ascii_lowercase());
                }
            }
        }
        ProjectionType::KeysOnly => {}
        ProjectionType::Include(values) => {
            body.push_str(&format!(
                ", [{}]",
                values
                    .iter()
                    .map(|value| format!("'{value}'"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    if let Some(throughput) = &index.throughput {
        body.push_str(&format!(
            ", THROUGHPUT ({}, {}))",
            throughput_literal(&throughput.read),
            throughput_literal(&throughput.write)
        ));
    } else {
        body.push(')');
    }
    let prefix = match &index.projection {
        ProjectionType::All if index.range_key.is_some() => "GLOBAL INDEX",
        ProjectionType::All => "GLOBAL ALL INDEX",
        ProjectionType::KeysOnly => "GLOBAL KEYS INDEX",
        ProjectionType::Include(_) => "GLOBAL INCLUDE INDEX",
    };
    format!("{prefix} {body}")
}

fn attribute_type_name(data_type: &AttributeType) -> &str {
    match data_type {
        AttributeType::String => "STRING",
        AttributeType::Number => "NUMBER",
        AttributeType::Binary => "BINARY",
        AttributeType::Bool => "BOOL",
        AttributeType::Other(value) => value.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dql_parser::parse_statement;

    fn meta(sql: &str) -> TableMeta {
        let statement = parse_statement(sql).unwrap();
        TableMeta::from_create_statement(&statement).unwrap()
    }

    #[test]
    fn ports_format_throughput_tests() {
        assert_eq!(format_throughput(Some(20.0), None), "20");
        assert_eq!(format_throughput(Some(20.0), Some(10.0)), "10/20 (50%)");
        assert_eq!(format_throughput(Some(20.0), Some(7.0)), "7/20 (35%)");
        assert_eq!(format_throughput(Some(0.0), Some(7.0)), "7/∞");
        assert_eq!(format_throughput(Some(0.0), None), "N/A");
    }

    #[test]
    fn computes_total_throughput_with_global_indexes() {
        let meta = meta(
            "CREATE TABLE foobar (id STRING HASH KEY, foo NUMBER, THROUGHPUT (1, 1)) \
             GLOBAL INDEX ('idx', id, foo, THROUGHPUT(1, 1))",
        );
        assert_eq!(meta.total_read_throughput(), Some(2.0));
        assert_eq!(meta.total_write_throughput(), Some(2.0));
    }

    #[test]
    fn matches_hash_and_range_indexes() {
        let meta = meta(
            "CREATE TABLE foobar (id STRING HASH KEY, ts NUMBER INDEX('ts-index'), \
             baz STRING) GLOBAL INDEX ('baz-index', baz)",
        );
        let possible_hash = BTreeSet::from(["id".to_string()]);
        let possible_range = BTreeSet::from(["ts".to_string()]);
        let indexes = meta.get_matching_indexes(&possible_hash, &possible_range);
        assert_eq!(indexes[0].name, "ts-index");
    }

    #[test]
    fn plans_select_query_and_scan_rejection() {
        let meta = meta("CREATE TABLE foobar (id STRING HASH KEY, foo NUMBER)");
        let condition = Condition::Compare {
            field: "id".to_string(),
            op: CompareOp::Eq,
            rhs: ConditionOperand::Value(Value::String("a".to_string())),
        };
        let plan = plan_read(PlanInput {
            table: &meta,
            kind: ReadKind::Select,
            condition: Some(&condition),
            selection: Some(&Selection::All),
            using_index: None,
            allow_select_scan: false,
        })
        .unwrap();
        assert_eq!(plan.operation, Operation::Query);

        let no_index = Condition::Compare {
            field: "foo".to_string(),
            op: CompareOp::Eq,
            rhs: ConditionOperand::Value(Value::Number("1".to_string())),
        };
        assert_eq!(
            plan_read(PlanInput {
                table: &meta,
                kind: ReadKind::Select,
                condition: Some(&no_index),
                selection: Some(&Selection::All),
                using_index: None,
                allow_select_scan: false,
            }),
            Err(PlanError::SelectScanRejected)
        );
    }

    #[test]
    fn plans_scan_with_using_index_without_where() {
        let meta = meta(
            "CREATE TABLE foobar (id STRING HASH KEY, name STRING) \
             GLOBAL INCLUDE INDEX ('gindex', name, ['foo'])",
        );
        let plan = plan_read(PlanInput {
            table: &meta,
            kind: ReadKind::Scan,
            condition: None,
            selection: Some(&Selection::CountAll),
            using_index: Some("gindex"),
            allow_select_scan: true,
        })
        .unwrap();
        assert_eq!(plan.operation, Operation::Scan);
        assert_eq!(
            plan.index.as_ref().map(|index| index.name.as_str()),
            Some("gindex")
        );
        assert!(!plan.follow_up_batch_get);
    }

    #[test]
    fn detects_partial_projection_follow_up_batch_get() {
        let meta = meta(
            "CREATE TABLE foobar (id STRING HASH KEY, foo NUMBER) \
             GLOBAL KEYS INDEX ('foo-index', foo)",
        );
        let condition = Condition::Compare {
            field: "foo".to_string(),
            op: CompareOp::Eq,
            rhs: ConditionOperand::Value(Value::Number("1".to_string())),
        };
        let selection = Selection::Items(vec![dql_parser::SelectionItem {
            expression: "bar".to_string(),
            alias: None,
        }]);
        let plan = plan_read(PlanInput {
            table: &meta,
            kind: ReadKind::Select,
            condition: Some(&condition),
            selection: Some(&selection),
            using_index: Some("foo-index"),
            allow_select_scan: false,
        })
        .unwrap();
        assert!(plan.follow_up_batch_get);
    }
}
