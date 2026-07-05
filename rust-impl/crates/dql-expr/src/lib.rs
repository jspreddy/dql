use dql_parser::{
    CompareOp, Condition, ConditionOperand, Selection, TimestampExpr, UpdateClauseKind, UpdateExpr,
    Value,
};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExprError {
    message: String,
}

impl ExprError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for ExprError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamoValue {
    Null(bool),
    Bool(bool),
    Number(String),
    String(String),
    Binary(String),
    NumberSet(Vec<String>),
    StringSet(Vec<String>),
    BinarySet(Vec<String>),
    List(Vec<DynamoValue>),
    Map(BTreeMap<String, DynamoValue>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    List(Vec<JsonValue>),
    Map(BTreeMap<String, JsonValue>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedExpression {
    pub expression: String,
    pub attribute_names: Option<BTreeMap<String, String>>,
    pub expression_values: Option<BTreeMap<String, DynamoValue>>,
}

#[derive(Debug, Clone)]
pub struct Visitor {
    reserved_words: Option<BTreeSet<String>>,
    fields: BTreeMap<String, String>,
    field_to_key: BTreeMap<String, String>,
    values: BTreeMap<String, DynamoValue>,
    next_field: usize,
    next_value: usize,
}

impl Visitor {
    pub fn new(reserved_words: Option<BTreeSet<String>>) -> Self {
        Self {
            reserved_words,
            fields: BTreeMap::new(),
            field_to_key: BTreeMap::new(),
            values: BTreeMap::new(),
            next_field: 1,
            next_value: 1,
        }
    }

    pub fn with_default_reserved_words() -> Self {
        Self::new(Some(default_reserved_words()))
    }

    pub fn get_field(&mut self, field: &str) -> String {
        encode_field_path(field, |segment| self.maybe_replace_segment(segment))
    }

    pub fn get_value(&mut self, value: &Value) -> Result<String, ExprError> {
        let key = format!(":v{}", self.next_value);
        self.next_value += 1;
        self.values.insert(key.clone(), value_to_dynamo(value)?);
        Ok(key)
    }

    pub fn attribute_names(&self) -> Option<BTreeMap<String, String>> {
        (!self.fields.is_empty()).then(|| self.fields.clone())
    }

    pub fn expression_values(&self) -> Option<BTreeMap<String, DynamoValue>> {
        (!self.values.is_empty()).then(|| self.values.clone())
    }

    fn maybe_replace_segment(&mut self, segment: &str) -> String {
        if self.should_replace(segment) {
            self.replace_segment(segment)
        } else {
            segment.to_string()
        }
    }

    fn should_replace(&self, segment: &str) -> bool {
        self.reserved_words.is_none()
            || self
                .reserved_words
                .as_ref()
                .is_some_and(|reserved| reserved.contains(&segment.to_ascii_uppercase()))
            || segment.contains('-')
            || segment.starts_with('_')
    }

    fn replace_segment(&mut self, segment: &str) -> String {
        if let Some(existing) = self.field_to_key.get(segment) {
            return existing.clone();
        }
        let key = format!("#f{}", self.next_field);
        self.next_field += 1;
        self.field_to_key.insert(segment.to_string(), key.clone());
        self.fields.insert(key.clone(), segment.to_string());
        key
    }
}

pub fn render_condition(condition: &Condition) -> Result<RenderedExpression, ExprError> {
    let mut visitor = Visitor::with_default_reserved_words();
    let expression = render_condition_with_visitor(condition, &mut visitor)?;
    Ok(RenderedExpression {
        expression,
        attribute_names: visitor.attribute_names(),
        expression_values: visitor.expression_values(),
    })
}

pub fn render_condition_with_visitor(
    condition: &Condition,
    visitor: &mut Visitor,
) -> Result<String, ExprError> {
    match condition {
        Condition::Compare { field, op, rhs } => Ok(format!(
            "{} {} {}",
            visitor.get_field(field),
            render_compare_op(op),
            render_operand(rhs, visitor)?
        )),
        Condition::Between { field, low, high } => Ok(format!(
            "{} BETWEEN {} AND {}",
            visitor.get_field(field),
            visitor.get_value(low)?,
            visitor.get_value(high)?
        )),
        Condition::In { field, values } => {
            let rendered = values
                .iter()
                .map(|value| visitor.get_value(value))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            Ok(format!("{} IN ({})", visitor.get_field(field), rendered))
        }
        Condition::Function { name, args } => {
            let rendered = args
                .iter()
                .map(|arg| render_operand(arg, visitor))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            Ok(format!("{name}({rendered})"))
        }
        Condition::Size { field, op, value } => Ok(format!(
            "size({}) {} {}",
            visitor.get_field(field),
            render_compare_op(op),
            visitor.get_value(value)?
        )),
        Condition::AttributeType { field, ty } => Ok(format!(
            "attribute_type({}, {})",
            visitor.get_field(field),
            visitor.get_value(&Value::String(ty.clone()))?
        )),
        Condition::And(conditions) => render_joined_conditions("AND", conditions, visitor),
        Condition::Or(conditions) => render_joined_conditions("OR", conditions, visitor),
        Condition::Not(condition) => Ok(format!(
            "NOT ({})",
            render_condition_with_visitor(condition, visitor)?
        )),
    }
}

pub fn render_update(update: &UpdateExpr) -> Result<RenderedExpression, ExprError> {
    let mut visitor = Visitor::with_default_reserved_words();
    let mut sections: BTreeMap<UpdateClauseKind, Vec<String>> = BTreeMap::new();
    for clause in &update.clauses {
        let rendered = match clause.kind {
            UpdateClauseKind::Set => format!(
                "{} = {}",
                visitor.get_field(&clause.path),
                render_raw_expression(
                    clause.expression.as_deref().unwrap_or_default(),
                    &mut visitor
                )?
            ),
            UpdateClauseKind::Add | UpdateClauseKind::Delete => format!(
                "{} {}",
                visitor.get_field(&clause.path),
                render_raw_expression(
                    clause.expression.as_deref().unwrap_or_default(),
                    &mut visitor
                )?
            ),
            UpdateClauseKind::Remove => visitor.get_field(&clause.path),
        };
        sections
            .entry(clause.kind.clone())
            .or_default()
            .push(rendered);
    }
    let mut parts = Vec::new();
    for (kind, label) in [
        (UpdateClauseKind::Set, "SET"),
        (UpdateClauseKind::Remove, "REMOVE"),
        (UpdateClauseKind::Add, "ADD"),
        (UpdateClauseKind::Delete, "DELETE"),
    ] {
        if let Some(items) = sections.remove(&kind) {
            parts.push(format!("{label} {}", items.join(", ")));
        }
    }
    Ok(RenderedExpression {
        expression: parts.join(" "),
        attribute_names: visitor.attribute_names(),
        expression_values: visitor.expression_values(),
    })
}

pub fn render_projection(selection: &Selection) -> RenderedExpression {
    let mut visitor = Visitor::with_default_reserved_words();
    let expression = match selection {
        Selection::All | Selection::CountAll => String::new(),
        Selection::Items(items) => items
            .iter()
            .flat_map(|item| extract_fields(&item.expression))
            .map(|field| visitor.get_field(&field))
            .collect::<Vec<_>>()
            .join(", "),
    };
    RenderedExpression {
        expression,
        attribute_names: visitor.attribute_names(),
        expression_values: visitor.expression_values(),
    }
}

pub fn dynamo_to_value(value: &DynamoValue) -> Result<Value, ExprError> {
    match value {
        DynamoValue::Null(true) => Ok(Value::Null),
        DynamoValue::Null(false) => Ok(Value::Null),
        DynamoValue::Bool(value) => Ok(Value::Bool(*value)),
        DynamoValue::Number(value) => Ok(Value::Number(value.clone())),
        DynamoValue::String(value) => Ok(Value::String(value.clone())),
        DynamoValue::Binary(value) => Ok(Value::Binary(decode_base64(value)?)),
        DynamoValue::NumberSet(values) => Ok(Value::Set(
            values
                .iter()
                .map(|value| Value::Number(value.clone()))
                .collect(),
        )),
        DynamoValue::StringSet(values) => Ok(Value::Set(
            values
                .iter()
                .map(|value| Value::String(value.clone()))
                .collect(),
        )),
        DynamoValue::BinarySet(values) => Ok(Value::Set(
            values
                .iter()
                .map(|value| Ok(Value::Binary(decode_base64(value)?)))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        DynamoValue::List(values) => values
            .iter()
            .map(dynamo_to_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::List),
        DynamoValue::Map(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), dynamo_to_value(value)?)))
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(Value::Map),
    }
}

pub fn value_to_dynamo(value: &Value) -> Result<DynamoValue, ExprError> {
    match value {
        Value::Null => Ok(DynamoValue::Null(true)),
        Value::Bool(value) => Ok(DynamoValue::Bool(*value)),
        Value::Number(value) => Ok(DynamoValue::Number(value.clone())),
        Value::String(value) => Ok(DynamoValue::String(value.clone())),
        Value::Binary(value) => Ok(DynamoValue::Binary(base64(value))),
        Value::List(values) => values
            .iter()
            .map(value_to_dynamo)
            .collect::<Result<Vec<_>, _>>()
            .map(DynamoValue::List),
        Value::Map(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), value_to_dynamo(value)?)))
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(DynamoValue::Map),
        Value::Set(values) => set_to_dynamo(values),
        Value::Timestamp(expr) => Ok(DynamoValue::Number(resolve_timestamp(expr).to_string())),
        Value::Interval(value) => Ok(DynamoValue::String(value.clone())),
    }
}

pub fn value_to_json(value: &Value, lossy_float: bool) -> Result<JsonValue, ExprError> {
    match value {
        Value::Null => Ok(JsonValue::Null),
        Value::Bool(value) => Ok(JsonValue::Bool(*value)),
        Value::Number(value) if lossy_float => Ok(JsonValue::Number(value.clone())),
        Value::Number(value) => Ok(JsonValue::String(value.clone())),
        Value::String(value) => Ok(JsonValue::String(value.clone())),
        Value::Binary(value) => Ok(JsonValue::String(base64(value))),
        Value::List(values) | Value::Set(values) => values
            .iter()
            .map(|value| value_to_json(value, lossy_float))
            .collect::<Result<Vec<_>, _>>()
            .map(JsonValue::List),
        Value::Map(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), value_to_json(value, lossy_float)?)))
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(JsonValue::Map),
        Value::Timestamp(expr) => Ok(JsonValue::Number(resolve_timestamp(expr).to_string())),
        Value::Interval(value) => Ok(JsonValue::String(value.clone())),
    }
}

fn render_joined_conditions(
    op: &str,
    conditions: &[Condition],
    visitor: &mut Visitor,
) -> Result<String, ExprError> {
    let rendered = conditions
        .iter()
        .map(|condition| render_condition_with_visitor(condition, visitor))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("({})", rendered.join(&format!(" {op} "))))
}

fn render_operand(operand: &ConditionOperand, visitor: &mut Visitor) -> Result<String, ExprError> {
    match operand {
        ConditionOperand::Field(field) => Ok(visitor.get_field(field)),
        ConditionOperand::Value(value) => visitor.get_value(value),
    }
}

fn render_compare_op(op: &CompareOp) -> &'static str {
    match op {
        CompareOp::Eq => "=",
        CompareOp::Ne => "<>",
        CompareOp::Lt => "<",
        CompareOp::Le => "<=",
        CompareOp::Gt => ">",
        CompareOp::Ge => ">=",
    }
}

fn render_raw_expression(raw: &str, visitor: &mut Visitor) -> Result<String, ExprError> {
    let mut parts = Vec::new();
    for token in raw.split_whitespace() {
        let trimmed = token.trim_matches(',');
        let rendered = if is_number(trimmed) {
            visitor.get_value(&Value::Number(trimmed.to_string()))?
        } else if is_quoted(trimmed) {
            visitor.get_value(&Value::String(
                trimmed.trim_matches('"').trim_matches('\'').to_string(),
            ))?
        } else if matches!(trimmed, "+" | "-" | "(" | ")" | "," | "") {
            trimmed.to_string()
        } else if trimmed.contains('(') {
            render_function_expression(trimmed, visitor)?
        } else {
            visitor.get_field(trimmed)
        };
        parts.push(rendered);
    }
    Ok(parts.join(" "))
}

fn render_function_expression(raw: &str, visitor: &mut Visitor) -> Result<String, ExprError> {
    let Some((name, rest)) = raw.split_once('(') else {
        return Ok(visitor.get_field(raw));
    };
    let args = rest.trim_end_matches(')');
    let rendered = args
        .split(',')
        .map(str::trim)
        .filter(|arg| !arg.is_empty())
        .map(|arg| {
            if is_number(arg) {
                visitor.get_value(&Value::Number(arg.to_string()))
            } else if is_quoted(arg) {
                visitor.get_value(&Value::String(
                    arg.trim_matches('"').trim_matches('\'').to_string(),
                ))
            } else {
                Ok(visitor.get_field(arg))
            }
        })
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    Ok(format!("{name}({rendered})"))
}

fn set_to_dynamo(values: &[Value]) -> Result<DynamoValue, ExprError> {
    let Some(first) = values.first() else {
        return Ok(DynamoValue::Null(true));
    };
    match first {
        Value::Number(_) => values
            .iter()
            .map(|value| match value {
                Value::Number(value) => Ok(value.clone()),
                _ => Err(ExprError::new("mixed-type number set")),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(DynamoValue::NumberSet),
        Value::String(_) => values
            .iter()
            .map(|value| match value {
                Value::String(value) => Ok(value.clone()),
                _ => Err(ExprError::new("mixed-type string set")),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(DynamoValue::StringSet),
        Value::Binary(_) => values
            .iter()
            .map(|value| match value {
                Value::Binary(value) => Ok(base64(value)),
                _ => Err(ExprError::new("mixed-type binary set")),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(DynamoValue::BinarySet),
        _ => Err(ExprError::new("unsupported set member type")),
    }
}

fn encode_field_path<F>(field: &str, mut encode_segment: F) -> String
where
    F: FnMut(&str) -> String,
{
    let mut output = String::new();
    let mut chars = field.chars().peekable();
    while let Some(ch) = chars.peek().copied() {
        if ch == '[' {
            for index_ch in chars.by_ref() {
                output.push(index_ch);
                if index_ch == ']' {
                    break;
                }
            }
        } else if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            let mut segment = String::new();
            while let Some(seg_ch) = chars.peek().copied() {
                if seg_ch.is_alphanumeric() || seg_ch == '_' || seg_ch == '-' {
                    segment.push(seg_ch);
                    chars.next();
                } else {
                    break;
                }
            }
            output.push_str(&encode_segment(&segment));
        } else {
            output.push(ch);
            chars.next();
        }
    }
    output
}

fn extract_fields(expression: &str) -> Vec<String> {
    expression
        .split(|ch: char| {
            ch.is_whitespace() || matches!(ch, '+' | '-' | '*' | '/' | '(' | ')' | ',')
        })
        .filter(|token| {
            !token.is_empty()
                && !is_number(token)
                && !matches!(
                    token.to_ascii_uppercase().as_str(),
                    "AS" | "NOW" | "UTCNOW" | "TIMESTAMP" | "TS" | "UTCTIMESTAMP" | "UTCTS"
                )
        })
        .map(str::to_string)
        .collect()
}

pub fn resolve_timestamp(expr: &TimestampExpr) -> f64 {
    match expr {
        TimestampExpr::Now => local_now_secs(),
        TimestampExpr::UtcNow => utc_now_secs(),
        TimestampExpr::Parse { function, value } => {
            if function.eq_ignore_ascii_case("UTCTIMESTAMP")
                || function.eq_ignore_ascii_case("UTCTS")
            {
                parse_utc_date(value).unwrap_or(0.0)
            } else {
                parse_local_date(value).unwrap_or(0.0)
            }
        }
        TimestampExpr::Ms(expr) => resolve_timestamp(expr) * 1000.0,
        TimestampExpr::AddInterval { base, interval } => {
            resolve_timestamp(base) + parse_interval_seconds(interval)
        }
        TimestampExpr::SubInterval { base, interval } => {
            resolve_timestamp(base) - parse_interval_seconds(interval)
        }
    }
}

fn local_now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}

fn utc_now_secs() -> f64 {
    local_now_secs()
}

fn parse_utc_date(value: &str) -> Option<f64> {
    parse_date(value)
}

fn parse_local_date(value: &str) -> Option<f64> {
    parse_date(value)
}

fn parse_date(value: &str) -> Option<f64> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    Some((days_from_civil(year, month, day) * 86_400) as f64)
}

fn parse_interval_seconds(value: &str) -> f64 {
    let mut total = 0.0;
    let mut pending = None;
    for part in value.split_whitespace() {
        if let Ok(amount) = part.parse::<f64>() {
            pending = Some(amount);
            continue;
        }
        if let Some((amount, unit)) = split_amount_unit(part) {
            total += interval_unit_seconds(unit) * amount;
        } else if let Some(amount) = pending.take() {
            total += interval_unit_seconds(part) * amount;
        }
    }
    total
}

fn split_amount_unit(value: &str) -> Option<(f64, &str)> {
    let index = value
        .char_indices()
        .find_map(|(idx, ch)| ch.is_alphabetic().then_some(idx))?;
    let (amount, unit) = value.split_at(index);
    Some((amount.parse().ok()?, unit))
}

fn interval_unit_seconds(unit: &str) -> f64 {
    match unit.to_ascii_lowercase().as_str() {
        "y" | "year" | "years" => 31_536_000.0,
        "month" | "months" => 2_592_000.0,
        "w" | "week" | "weeks" => 604_800.0,
        "d" | "day" | "days" => 86_400.0,
        "h" | "hour" | "hours" => 3_600.0,
        "m" | "minute" | "minutes" => 60.0,
        "s" | "second" | "seconds" => 1.0,
        "ms" | "millisecond" | "milliseconds" => 0.001,
        "us" | "microsecond" | "microseconds" => 0.000001,
        _ => 0.0,
    }
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - (month <= 2) as i32;
    let era = (if year >= 0 { year } else { year - 399 }) / 400;
    let yoe = year - era * 400;
    let month = month as i32;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day as i32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146097 + doe - 719468) as i64
}

fn default_reserved_words() -> BTreeSet<String> {
    [
        "ABORT", "ACTION", "ADD", "COUNT", "DELETE", "FROM", "HASH", "INDEX", "ORDER", "RANGE",
        "SELECT", "SET", "SIZE", "TABLE", "UPDATE",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn is_number(value: &str) -> bool {
    !value.is_empty() && value.parse::<f64>().is_ok()
}

fn is_quoted(value: &str) -> bool {
    (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
}

fn decode_base64(input: &str) -> Result<Vec<u8>, ExprError> {
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
        if ch == b'=' {
            break;
        }
        let value = decode
            .get(ch as usize)
            .copied()
            .ok_or_else(|| ExprError::new(format!("invalid base64 character '{ch}'")))?;
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

fn base64(input: &[u8]) -> String {
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

pub fn project_selection(
    item: &BTreeMap<String, Value>,
    selection: &Selection,
) -> BTreeMap<String, Value> {
    let Selection::Items(items) = selection else {
        return item.clone();
    };
    let mut projected = BTreeMap::new();
    for entry in items {
        let key = entry
            .alias
            .clone()
            .unwrap_or_else(|| entry.expression.trim().to_string());
        match eval_selection_expression(item, &entry.expression) {
            Ok(value) => {
                projected.insert(key, value);
            }
            Err(_) => {
                projected.insert(key, Value::Null);
            }
        }
    }
    projected
}

fn eval_selection_expression(
    item: &BTreeMap<String, Value>,
    expression: &str,
) -> Result<Value, ExprError> {
    let tokens = tokenize_selection(expression);
    let mut parser = SelectionParser {
        item,
        tokens,
        pos: 0,
    };
    parser.parse_expression()
}

struct SelectionParser<'a> {
    item: &'a BTreeMap<String, Value>,
    tokens: Vec<String>,
    pos: usize,
}

impl SelectionParser<'_> {
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.pos).map(String::as_str)
    }

    fn next(&mut self) -> Option<String> {
        if self.pos < self.tokens.len() {
            let token = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(token)
        } else {
            None
        }
    }

    fn parse_expression(&mut self) -> Result<Value, ExprError> {
        self.parse_additive()
    }

    fn parse_additive(&mut self) -> Result<Value, ExprError> {
        let mut value = self.parse_multiplicative()?;
        while matches!(self.peek(), Some("+") | Some("-")) {
            let op = self.next().unwrap();
            let rhs = self.parse_multiplicative()?;
            value = match op.as_str() {
                "+" => add_values(&value, &rhs),
                "-" => sub_values(&value, &rhs),
                _ => unreachable!(),
            }?;
        }
        Ok(value)
    }

    fn parse_multiplicative(&mut self) -> Result<Value, ExprError> {
        let mut value = self.parse_unary()?;
        while matches!(self.peek(), Some("*") | Some("/")) {
            let op = self.next().unwrap();
            let rhs = self.parse_unary()?;
            value = match op.as_str() {
                "*" => mul_values(&value, &rhs),
                "/" => div_values(&value, &rhs),
                _ => unreachable!(),
            }?;
        }
        Ok(value)
    }

    fn parse_unary(&mut self) -> Result<Value, ExprError> {
        if self.peek() == Some("-") {
            self.next();
            let value = self.parse_unary()?;
            return mul_values(&Value::Number("-1".to_string()), &value);
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Value, ExprError> {
        if self.peek() == Some("(") {
            self.next();
            let value = self.parse_expression()?;
            if self.next().as_deref() != Some(")") {
                return Err(ExprError::new("expected ')'"));
            }
            return Ok(value);
        }
        let token = self
            .next()
            .ok_or_else(|| ExprError::new("expected selection expression"))?;
        if self.peek() == Some("(") {
            self.next();
            return self.parse_function(&token.to_ascii_uppercase());
        }
        if is_number(&token) {
            return Ok(Value::Number(token));
        }
        if is_quoted(&token) {
            return Ok(Value::String(
                token.trim_matches('"').trim_matches('\'').to_string(),
            ));
        }
        Ok(item_field(self.item, &token).unwrap_or(Value::Null))
    }

    fn parse_function(&mut self, name: &str) -> Result<Value, ExprError> {
        let value = if matches!(name, "NOW" | "UTCNOW") {
            Value::Timestamp(if name == "UTCNOW" {
                TimestampExpr::UtcNow
            } else {
                TimestampExpr::Now
            })
        } else if matches!(name, "MS") {
            let inner = self.parse_expression()?;
            Value::Timestamp(TimestampExpr::Ms(Box::new(timestamp_expr_from_value(
                &inner,
            )?)))
        } else if matches!(name, "TS" | "TIMESTAMP" | "UTCTIMESTAMP" | "UTCTS") {
            let inner = self.parse_expression()?;
            timestamp_from_value(&inner, name)?
        } else {
            return Err(ExprError::new(format!("unknown function '{name}'")));
        };
        if self.next().as_deref() != Some(")") {
            return Err(ExprError::new("expected ')'"));
        }
        Ok(value)
    }
}

fn tokenize_selection(expression: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in expression.chars() {
        match ch {
            '+' | '-' | '*' | '/' | '(' | ')' | ',' => {
                if !current.trim().is_empty() {
                    tokens.push(current.trim().to_string());
                    current.clear();
                }
                tokens.push(ch.to_string());
            }
            ' ' | '\t' | '\n' => {
                if !current.trim().is_empty() {
                    tokens.push(current.trim().to_string());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        tokens.push(current.trim().to_string());
    }
    tokens
}

fn item_field(item: &BTreeMap<String, Value>, field: &str) -> Option<Value> {
    item.get(field).cloned()
}

fn timestamp_expr_from_value(value: &Value) -> Result<TimestampExpr, ExprError> {
    match value {
        Value::Timestamp(expr) => Ok(expr.clone()),
        Value::String(text) => Ok(TimestampExpr::Parse {
            function: "TS".into(),
            value: text.clone(),
        }),
        Value::Number(number) => Ok(TimestampExpr::Parse {
            function: "TS".into(),
            value: number.clone(),
        }),
        Value::Null => Err(ExprError::new("unsupported timestamp input")),
        _ => Err(ExprError::new("unsupported timestamp input")),
    }
}

fn timestamp_from_value(value: &Value, function: &str) -> Result<Value, ExprError> {
    match value {
        Value::Timestamp(TimestampExpr::Ms(expr)) => {
            let ms = resolve_timestamp(&TimestampExpr::Ms(expr.clone()));
            Ok(Value::Timestamp(TimestampExpr::Parse {
                function: function.to_string(),
                value: (ms / 1000.0).to_string(),
            }))
        }
        Value::Timestamp(expr) => Ok(Value::Timestamp(expr.clone())),
        Value::String(text) => Ok(Value::Timestamp(TimestampExpr::Parse {
            function: function.to_string(),
            value: text.clone(),
        })),
        Value::Number(number) => Ok(Value::Timestamp(TimestampExpr::Parse {
            function: function.to_string(),
            value: number.clone(),
        })),
        Value::Null => Ok(Value::Null),
        _ => Err(ExprError::new("unsupported timestamp input")),
    }
}

fn add_values(left: &Value, right: &Value) -> Result<Value, ExprError> {
    if matches!(left, Value::Null) {
        return Ok(right.clone());
    }
    if matches!(right, Value::Null) {
        return Ok(left.clone());
    }
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => {
            let sum = left
                .parse::<f64>()
                .map_err(|err| ExprError::new(err.to_string()))?
                + right
                    .parse::<f64>()
                    .map_err(|err| ExprError::new(err.to_string()))?;
            Ok(Value::Number(sum.to_string()))
        }
        (Value::Timestamp(left), Value::Timestamp(right)) => {
            let diff = resolve_timestamp(left) - resolve_timestamp(right);
            Ok(Value::Interval(format!("{diff} seconds")))
        }
        _ => Err(ExprError::new("unsupported selection arithmetic")),
    }
}

fn sub_values(left: &Value, right: &Value) -> Result<Value, ExprError> {
    if matches!(left, Value::Null) && matches!(right, Value::Null) {
        return Ok(Value::Null);
    }
    if matches!(left, Value::Null) {
        return mul_values(&Value::Number("-1".to_string()), right);
    }
    if matches!(right, Value::Null) {
        return Ok(left.clone());
    }
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => {
            let diff = left
                .parse::<f64>()
                .map_err(|err| ExprError::new(err.to_string()))?
                - right
                    .parse::<f64>()
                    .map_err(|err| ExprError::new(err.to_string()))?;
            Ok(Value::Number(diff.to_string()))
        }
        (Value::Timestamp(left), Value::Timestamp(right)) => {
            let diff = resolve_timestamp(left) - resolve_timestamp(right);
            Ok(Value::Interval(format!("{diff} seconds")))
        }
        _ => Err(ExprError::new("unsupported selection arithmetic")),
    }
}

fn mul_values(left: &Value, right: &Value) -> Result<Value, ExprError> {
    if matches!(left, Value::Null) {
        return Ok(right.clone());
    }
    if matches!(right, Value::Null) {
        return Ok(left.clone());
    }
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => {
            let product = left
                .parse::<f64>()
                .map_err(|err| ExprError::new(err.to_string()))?
                * right
                    .parse::<f64>()
                    .map_err(|err| ExprError::new(err.to_string()))?;
            Ok(Value::Number(product.to_string()))
        }
        _ => Err(ExprError::new("unsupported selection arithmetic")),
    }
}

fn div_values(left: &Value, right: &Value) -> Result<Value, ExprError> {
    if matches!(left, Value::Null) && matches!(right, Value::Null) {
        return Ok(Value::Null);
    }
    if matches!(left, Value::Null) {
        return match right {
            Value::Number(right) => {
                let value = 1.0
                    / right
                        .parse::<f64>()
                        .map_err(|err| ExprError::new(err.to_string()))?;
                Ok(Value::Number(value.to_string()))
            }
            _ => Err(ExprError::new("unsupported selection arithmetic")),
        };
    }
    if matches!(right, Value::Null) {
        return Ok(left.clone());
    }
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => {
            let quotient = left
                .parse::<f64>()
                .map_err(|err| ExprError::new(err.to_string()))?
                / right
                    .parse::<f64>()
                    .map_err(|err| ExprError::new(err.to_string()))?;
            Ok(Value::Number(quotient.to_string()))
        }
        _ => Err(ExprError::new("unsupported selection arithmetic")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dql_parser::{parse_selection, parse_statement, parse_update_expr, parse_value, Statement};

    #[test]
    fn visitor_escapes_reserved_dashed_and_underscored_segments() {
        let mut visitor = Visitor::with_default_reserved_words();
        assert_eq!(visitor.get_field("order.foo-bar._baz[0]"), "#f1.#f2.#f3[0]");
        assert_eq!(
            visitor.attribute_names().unwrap(),
            BTreeMap::from([
                ("#f1".to_string(), "order".to_string()),
                ("#f2".to_string(), "foo-bar".to_string()),
                ("#f3".to_string(), "_baz".to_string()),
            ])
        );
    }

    #[test]
    fn renders_condition_with_placeholders() {
        let statement = parse_statement("SELECT * FROM t WHERE order IN (1, 2)").unwrap();
        let Statement::Select {
            condition: Some(condition),
            ..
        } = statement
        else {
            panic!("expected select");
        };
        let rendered = render_condition(&condition).unwrap();
        assert_eq!(rendered.expression, "#f1 IN (:v1, :v2)");
        assert_eq!(
            rendered.attribute_names.unwrap(),
            BTreeMap::from([("#f1".to_string(), "order".to_string())])
        );
    }

    #[test]
    fn renders_projection_and_update() {
        let projection = render_projection(&parse_selection("foo, order AS ord").unwrap());
        assert_eq!(projection.expression, "foo, #f1");
        let update =
            render_update(&parse_update_expr("SET order = 1 REMOVE _old").unwrap()).unwrap();
        assert!(update.expression.contains("SET #f1 = :v1"));
        assert!(update.expression.contains("REMOVE #f2"));
    }

    #[test]
    fn converts_values_to_dynamo_and_json() {
        assert_eq!(
            value_to_dynamo(&parse_value("b'a'").unwrap()).unwrap(),
            DynamoValue::Binary("YQ==".to_string())
        );
        assert_eq!(
            value_to_json(&parse_value("1.25").unwrap(), false).unwrap(),
            JsonValue::String("1.25".to_string())
        );
        assert_eq!(
            value_to_json(&parse_value("1.25").unwrap(), true).unwrap(),
            JsonValue::Number("1.25".to_string())
        );
    }
}
