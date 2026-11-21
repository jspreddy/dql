//! Two classifiers for a temp table the user is about to grant.
//!
//! Attribute name patterns are one generic list for every dataset. Value
//! classifiers may match a shape that only some data has. Neither one grants.

use dql_engine::Item;
use dql_parser::Value;
use serde_json::{json, Value as Json};
use std::collections::BTreeSet;

const SCAN_CAP: usize = 1000;
const VALUE_CAP: usize = 3;

const NAME_PATTERNS: &[&str] = &[
    "email",
    "phone",
    "ssn",
    "password",
    "passwd",
    "secret",
    "token",
    "apikey",
    "creditcard",
    "cardnumber",
    "cvv",
    "dob",
    "birthdate",
    "address",
    "passport",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameHit {
    pub attribute: String,
    pub pattern: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueHit {
    pub attribute: String,
    pub classifier: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classification {
    pub attributes: Vec<NameHit>,
    pub values: Vec<ValueHit>,
    pub scanned: usize,
    pub total: usize,
}

pub fn classify(rows: &[Item], total: usize) -> Classification {
    let scanned = rows.len().min(SCAN_CAP);
    let rows = &rows[..scanned];
    let mut attributes = Vec::new();
    let mut seen_names = BTreeSet::new();
    let mut values = Vec::new();
    let mut seen_values = BTreeSet::new();
    for row in rows {
        walk(
            row,
            "",
            &mut attributes,
            &mut seen_names,
            &mut values,
            &mut seen_values,
        );
    }
    Classification {
        attributes,
        values,
        scanned,
        total,
    }
}

pub fn line(found: &Classification) -> String {
    if found.attributes.is_empty() && found.values.is_empty() {
        return if found.total > found.scanned {
            format!(
                "No sensitive attributes or values found in the first {} of {} rows",
                found.scanned, found.total
            )
        } else {
            format!(
                "No sensitive attributes or values found in {} rows",
                found.scanned
            )
        };
    }
    let mut text = String::from("Recommend not granting.");
    text.push_str(" Attributes: ");
    if found.attributes.is_empty() {
        text.push_str("none");
    } else {
        text.push_str(
            &found
                .attributes
                .iter()
                .map(|hit| format!("{} ({})", hit.attribute, hit.pattern))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    text.push_str(". Values: ");
    if found.values.is_empty() {
        text.push_str("none");
    } else {
        text.push_str(&value_phrase(&found.values));
    }
    text
}

pub fn to_json(found: &Classification) -> Json {
    json!({
        "line": line(found),
        "scanned": found.scanned,
        "total": found.total,
        "attributes": found.attributes.iter().map(|hit| json!({
            "attribute": hit.attribute,
            "pattern": hit.pattern,
        })).collect::<Vec<_>>(),
        "values": found.values.iter().map(|hit| json!({
            "attribute": hit.attribute,
            "classifier": hit.classifier,
            "value": hit.value,
        })).collect::<Vec<_>>(),
    })
}

fn value_phrase(hits: &[ValueHit]) -> String {
    let shown = hits.len().min(VALUE_CAP);
    let mut parts: Vec<String> = hits[..shown]
        .iter()
        .map(|hit| format!("{} in {} ({})", hit.value, hit.attribute, hit.classifier))
        .collect();
    let rest = hits.len().saturating_sub(shown);
    if rest > 0 {
        parts.push(format!("and {rest} more"));
    }
    parts.join(", ")
}

fn walk(
    row: &Item,
    prefix: &str,
    attributes: &mut Vec<NameHit>,
    seen_names: &mut BTreeSet<(String, String)>,
    values: &mut Vec<ValueHit>,
    seen_values: &mut BTreeSet<(String, String, String)>,
) {
    for (key, value) in row {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        note_name(&path, key, attributes, seen_names);
        walk_value(&path, value, attributes, seen_names, values, seen_values);
    }
}

fn walk_value(
    path: &str,
    value: &Value,
    attributes: &mut Vec<NameHit>,
    seen_names: &mut BTreeSet<(String, String)>,
    values: &mut Vec<ValueHit>,
    seen_values: &mut BTreeSet<(String, String, String)>,
) {
    match value {
        Value::Null | Value::Binary(_) | Value::Timestamp(_) | Value::Interval(_) => {}
        Value::Bool(flag) => note_values(
            path,
            if *flag { "true" } else { "false" },
            values,
            seen_values,
        ),
        Value::Number(text) | Value::String(text) => note_values(path, text, values, seen_values),
        Value::List(items) | Value::Set(items) => {
            for item in items {
                walk_value(path, item, attributes, seen_names, values, seen_values);
            }
        }
        Value::Map(map) => {
            for (key, child) in map {
                let child_path = format!("{path}.{key}");
                note_name(&child_path, key, attributes, seen_names);
                walk_value(
                    &child_path,
                    child,
                    attributes,
                    seen_names,
                    values,
                    seen_values,
                );
            }
        }
    }
}

fn note_name(
    path: &str,
    key: &str,
    attributes: &mut Vec<NameHit>,
    seen: &mut BTreeSet<(String, String)>,
) {
    let Some(pattern) = name_pattern(key) else {
        return;
    };
    if seen.insert((path.to_string(), pattern.to_string())) {
        attributes.push(NameHit {
            attribute: path.to_string(),
            pattern: pattern.to_string(),
        });
    }
}

fn note_values(
    path: &str,
    text: &str,
    values: &mut Vec<ValueHit>,
    seen: &mut BTreeSet<(String, String, String)>,
) {
    for (classifier, sample) in value_matches(text) {
        if seen.insert((path.to_string(), classifier.to_string(), sample.clone())) {
            values.push(ValueHit {
                attribute: path.to_string(),
                classifier: classifier.to_string(),
                value: sample,
            });
        }
    }
}

/// A pattern matches one word, or two or three adjacent words joined.
pub fn name_pattern(name: &str) -> Option<&'static str> {
    let words = words(name);
    if words.is_empty() {
        return None;
    }
    for size in 1..=3 {
        if words.len() < size {
            break;
        }
        for start in 0..=words.len() - size {
            let joined = words[start..start + size].join("");
            if let Some(pattern) = NAME_PATTERNS
                .iter()
                .copied()
                .find(|pattern| *pattern == joined)
            {
                return Some(pattern);
            }
        }
    }
    None
}

fn words(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut words = Vec::new();
    let mut current = String::new();
    for (index, ch) in chars.iter().copied().enumerate() {
        if ch == '_' || ch == '-' || ch == '.' {
            push_word(&mut words, &mut current);
            continue;
        }
        if ch.is_uppercase() && !current.is_empty() {
            let prev = chars[index - 1];
            let next_lower = chars.get(index + 1).is_some_and(|next| next.is_lowercase());
            if prev.is_lowercase() || (prev.is_uppercase() && next_lower) {
                push_word(&mut words, &mut current);
            }
        }
        current.push(ch.to_ascii_lowercase());
    }
    push_word(&mut words, &mut current);
    words
}

fn push_word(words: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        words.push(std::mem::take(current));
    }
}

fn value_matches(text: &str) -> Vec<(&'static str, String)> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    // The whole value still covers a card that contains spaces. A phone is a
    // contiguous phone-shaped span, so ten digits scattered through a sentence do not match.
    push_value_hit(&mut hits, trimmed);
    for piece in trimmed.split(|ch: char| {
        ch.is_whitespace() || matches!(ch, ',' | ';' | '(' | ')' | '<' | '>' | '"' | '\'')
    }) {
        let piece = piece.trim_matches(|ch: char| ch == '.' || ch == ':' || ch == '!' || ch == '?');
        if piece.is_empty() || piece == trimmed {
            continue;
        }
        push_value_hit(&mut hits, piece);
    }
    for phone in phone_spans(trimmed) {
        hits.push(("phone", phone));
    }
    hits
}

fn push_value_hit(hits: &mut Vec<(&'static str, String)>, text: &str) {
    if is_email(text) {
        hits.push(("email", text.to_string()));
    }
    if is_ssn(text) {
        hits.push(("ssn", text.to_string()));
    }
    if is_card(text) {
        hits.push(("card", text.to_string()));
    }
    if is_aws_key(text) {
        hits.push(("aws-key", text.to_string()));
    }
    if is_token(text) {
        hits.push(("token", text.to_string()));
    }
}

fn is_email(text: &str) -> bool {
    let Some((name, host)) = text.split_once('@') else {
        return false;
    };
    !name.is_empty()
        && !name.contains(char::is_whitespace)
        && host.split_once('.').is_some_and(|(left, right)| {
            !left.is_empty() && !right.is_empty() && !host.contains(char::is_whitespace)
        })
}

fn phone_spans(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if !is_phone_char(chars[index]) {
            index += 1;
            continue;
        }
        let start = index;
        while index < chars.len() && is_phone_char(chars[index]) {
            index += 1;
        }
        let raw: String = chars[start..index].iter().collect();
        let candidate = raw.trim_matches(|ch: char| matches!(ch, ' ' | '.' | '-'));
        if is_phone_shape(candidate) {
            found.push(candidate.to_string());
        }
    }
    found
}

fn is_phone_char(ch: char) -> bool {
    ch.is_ascii_digit() || matches!(ch, '+' | '-' | '.' | '(' | ')' | ' ')
}

fn is_phone_shape(text: &str) -> bool {
    if text.is_empty()
        || text.chars().any(|ch| !is_phone_char(ch))
        || text.matches('+').count() > 1
        || (text.contains('+') && !text.starts_with('+'))
    {
        return false;
    }
    let digits: String = text.chars().filter(|ch| ch.is_ascii_digit()).collect();
    let national = digits
        .strip_prefix('1')
        .filter(|_| digits.len() == 11)
        .unwrap_or(digits.as_str());
    if national.len() != 10 || national.as_bytes()[0] == b'0' || national.as_bytes()[0] == b'1' {
        return false;
    }
    let mut groups = Vec::new();
    let mut count = 0;
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            count += 1;
        } else if count > 0 {
            groups.push(count);
            count = 0;
        }
    }
    if count > 0 {
        groups.push(count);
    }
    // A dotted version such as 1.2.3.4.5.6.7.8.9.0 is ten digits, not a phone.
    !(groups.len() > 1 && groups.iter().all(|len| *len < 3))
}

fn is_ssn(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 11
        && bytes[3] == b'-'
        && bytes[6] == b'-'
        && bytes[..3].iter().all(u8::is_ascii_digit)
        && bytes[4..6].iter().all(u8::is_ascii_digit)
        && bytes[7..].iter().all(u8::is_ascii_digit)
}

fn is_card(text: &str) -> bool {
    let digits: Vec<u8> = text
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '-')
        .map(|ch| ch as u8)
        .collect();
    if !(13..=19).contains(&digits.len()) || digits.iter().any(|byte| !byte.is_ascii_digit()) {
        return false;
    }
    luhn(&digits)
}

fn luhn(digits: &[u8]) -> bool {
    let mut sum = 0;
    for (index, byte) in digits.iter().rev().enumerate() {
        let mut value = (byte - b'0') as i32;
        if index % 2 == 1 {
            value *= 2;
            if value > 9 {
                value -= 9;
            }
        }
        sum += value;
    }
    sum % 10 == 0
}

fn is_aws_key(text: &str) -> bool {
    let rest = text.strip_prefix("AKIA").unwrap_or("");
    rest.len() == 16 && rest.chars().all(|ch| ch.is_ascii_alphanumeric())
}

fn is_token(text: &str) -> bool {
    text.len() >= 32
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use dql_parser::Value;
    use std::collections::BTreeMap;

    fn row(pairs: &[(&str, &str)]) -> Item {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), Value::String((*value).to_string())))
            .collect()
    }

    #[test]
    fn name_patterns_are_words_not_substrings() {
        assert_eq!(name_pattern("user_email"), Some("email"));
        assert_eq!(name_pattern("userEmail"), Some("email"));
        assert_eq!(name_pattern("api_key"), Some("apikey"));
        assert_eq!(name_pattern("credit_card"), Some("creditcard"));
        assert_eq!(name_pattern("order_id"), None);
        assert_eq!(name_pattern("adobe_id"), None);
        assert_eq!(name_pattern("tokens_used"), None);
    }

    #[test]
    fn a_second_table_uses_the_same_name_list() {
        let first = classify(&[row(&[("user_email", "a")])], 1);
        let second = classify(&[row(&[("user_email", "b")])], 1);
        assert_eq!(first.attributes[0].pattern, second.attributes[0].pattern);
    }

    #[test]
    fn values_name_the_email_and_a_luhn_card() {
        let mut nested = BTreeMap::new();
        nested.insert(
            "email".to_string(),
            Value::String("ada@example.com".to_string()),
        );
        let mut item = Item::new();
        item.insert(
            "notes".to_string(),
            Value::String("ada@example.com".to_string()),
        );
        item.insert(
            "card".to_string(),
            Value::String("4111-1111-1111-1111".to_string()),
        );
        item.insert("profile".to_string(), Value::Map(nested));
        let found = classify(&[item], 1);
        assert!(found
            .attributes
            .iter()
            .any(|hit| hit.attribute == "profile.email" && hit.pattern == "email"));
        assert!(found.values.iter().any(|hit| {
            hit.attribute == "notes" && hit.classifier == "email" && hit.value == "ada@example.com"
        }));
        assert!(found
            .values
            .iter()
            .any(|hit| hit.classifier == "card" && hit.attribute == "card"));
        assert!(line(&found).starts_with("Recommend not granting."));
    }

    #[test]
    fn an_email_inside_a_sentence_is_named() {
        let found = classify(
            &[row(&[(
                "notes",
                "contact ada@example.com or key AKIAIOSFODNN7EXAMPLE leaked",
            )])],
            1,
        );
        assert!(found.values.iter().any(|hit| {
            hit.attribute == "notes" && hit.classifier == "email" && hit.value == "ada@example.com"
        }));
        assert!(found.values.iter().any(|hit| {
            hit.attribute == "notes"
                && hit.classifier == "aws-key"
                && hit.value == "AKIAIOSFODNN7EXAMPLE"
        }));
        assert!(line(&found).contains("ada@example.com"));
    }

    #[test]
    fn a_sentence_of_ten_digits_is_not_a_phone() {
        let scattered = classify(&[row(&[("notes", "order 12345 shipped to room 67890")])], 1);
        assert!(scattered.values.iter().all(|hit| hit.classifier != "phone"));
        let version = classify(&[row(&[("notes", "version 1.2.3.4.5.6.7.8.9.0")])], 1);
        assert!(version.values.iter().all(|hit| hit.classifier != "phone"));
    }

    #[test]
    fn a_phone_shape_is_named_on_its_own_and_inside_a_sentence() {
        let plain = classify(&[row(&[("notes", "415-555-0134")])], 1);
        assert!(plain
            .values
            .iter()
            .any(|hit| hit.classifier == "phone" && hit.value == "415-555-0134"));
        let spaced = classify(&[row(&[("notes", "+1 (415) 555-0134")])], 1);
        assert!(spaced
            .values
            .iter()
            .any(|hit| hit.classifier == "phone" && hit.value == "+1 (415) 555-0134"));
        let sentence = classify(&[row(&[("notes", "call (415) 555-0134 today")])], 1);
        assert!(sentence
            .values
            .iter()
            .any(|hit| hit.classifier == "phone" && hit.value == "(415) 555-0134"));
    }

    #[test]
    fn a_clean_table_names_the_rows_scanned() {
        let found = classify(&[row(&[("order_id", "100")])], 1);
        assert!(found.attributes.is_empty());
        assert!(found.values.is_empty());
        assert_eq!(
            line(&found),
            "No sensitive attributes or values found in 1 rows"
        );
        let wide = Classification {
            attributes: Vec::new(),
            values: Vec::new(),
            scanned: 1000,
            total: 8000,
        };
        assert_eq!(
            line(&wide),
            "No sensitive attributes or values found in the first 1000 of 8000 rows"
        );
    }
}
