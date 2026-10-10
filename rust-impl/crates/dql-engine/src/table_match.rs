use dql_models::TableMeta;

pub struct MatchTable {
    pub name: String,
    pub fields: Vec<String>,
}

struct ScoredMatch {
    name: String,
    name_match: bool,
    score: i32,
}

pub struct IntelligentHit {
    pub names: Vec<String>,
    pub note: String,
}

const MATCH_THRESHOLD: i32 = 85;

impl MatchTable {
    pub fn from_meta(meta: &TableMeta) -> Self {
        let mut fields = Vec::new();
        fields.push(meta.hash_key.clone());
        if let Some(range_key) = &meta.range_key {
            fields.push(range_key.clone());
        }
        fields.extend(meta.attrs.keys().cloned());
        for (name, index) in &meta.local_indexes {
            fields.push(name.clone());
            fields.push(index.hash_key.clone());
            if let Some(range_key) = &index.range_key {
                fields.push(range_key.clone());
            }
        }
        for (name, index) in &meta.global_indexes {
            fields.push(name.clone());
            fields.push(index.hash_key.name.clone());
            if let Some(range_key) = &index.range_key {
                fields.push(range_key.name.clone());
            }
        }
        Self {
            name: meta.name.clone(),
            fields,
        }
    }

    #[cfg(test)]
    fn named(name: &str, hash_key: &str) -> Self {
        Self {
            name: name.to_string(),
            fields: vec![hash_key.to_string()],
        }
    }
}

/// Names and keys similar to `pattern`. `*` and `?` are wildcards and are not part of the text.
pub fn intelligent_matches(tables: &[MatchTable], pattern: &str) -> Result<IntelligentHit, String> {
    score_matches(tables, pattern, &['*', '?'])
}

/// Same ranking as [`intelligent_matches`], treating `%` as the SQL LIKE wildcard.
pub fn intelligent_like_matches(
    tables: &[MatchTable],
    pattern: &str,
) -> Result<IntelligentHit, String> {
    score_matches(tables, pattern, &['%'])
}

fn score_matches(
    tables: &[MatchTable],
    pattern: &str,
    wildcards: &[char],
) -> Result<IntelligentHit, String> {
    let needle = intelligent_needle(pattern, wildcards);
    if compact(&needle).chars().count() < 2 {
        return Err(format!("Table {pattern:?} not found"));
    }
    let mut scored = Vec::new();
    for table in tables {
        let name = name_similarity(&needle, &table.name);
        let schema = schema_similarity(&needle, table);
        scored.push((table.name.clone(), name, schema));
    }
    let allow_fuzzy = !scored.iter().any(|(_, name, schema)| {
        name.structural >= MATCH_THRESHOLD || schema.structural >= MATCH_THRESHOLD
    });
    let mut chosen = Vec::new();
    for (name, name_score, schema_score) in scored {
        let name_points = name_score.pick(allow_fuzzy);
        let schema_points = schema_score.pick(allow_fuzzy);
        if name_points >= MATCH_THRESHOLD {
            chosen.push(ScoredMatch {
                name,
                name_match: true,
                score: name_points,
            });
        } else if schema_points >= MATCH_THRESHOLD {
            chosen.push(ScoredMatch {
                name,
                name_match: false,
                score: schema_points,
            });
        }
    }
    if chosen.is_empty() {
        return Err(format!("Table {pattern:?} not found"));
    }
    chosen.sort_by(|left, right| {
        right
            .name_match
            .cmp(&left.name_match)
            .then(right.score.cmp(&left.score))
            .then(left.name.len().cmp(&right.name.len()))
            .then(left.name.cmp(&right.name))
    });
    let names_hit = chosen.iter().any(|item| item.name_match);
    let related_hit = chosen.iter().any(|item| !item.name_match);
    let names = chosen.into_iter().map(|item| item.name).collect();
    Ok(IntelligentHit {
        names,
        note: intelligent_note(pattern, names_hit, related_hit),
    })
}

fn schema_similarity(needle: &str, table: &MatchTable) -> Similarity {
    if compact(needle).chars().count() < 3 {
        return Similarity {
            structural: 0,
            fuzzy: 0,
        };
    }
    table
        .fields
        .iter()
        .map(|field| name_similarity(needle, field))
        .fold(Similarity::default(), Similarity::max)
}

#[derive(Clone, Copy, Default)]
struct Similarity {
    structural: i32,
    fuzzy: i32,
}

impl Similarity {
    fn pick(self, allow_fuzzy: bool) -> i32 {
        if allow_fuzzy {
            self.structural.max(self.fuzzy)
        } else {
            self.structural
        }
    }

    fn max(self, other: Self) -> Self {
        Self {
            structural: self.structural.max(other.structural),
            fuzzy: self.fuzzy.max(other.fuzzy),
        }
    }
}

fn name_similarity(needle: &str, name: &str) -> Similarity {
    let needle_c = compact(needle);
    let name_c = compact(name);
    if needle_c.chars().count() < 2 || name_c.is_empty() {
        return Similarity::default();
    }
    if needle_c.chars().count() < 3 {
        return Similarity {
            structural: if name_c.starts_with(&needle_c) { 93 } else { 0 },
            fuzzy: 0,
        };
    }
    if needle_c == name_c {
        return Similarity {
            structural: 100,
            fuzzy: 100,
        };
    }
    let mut structural = 0;
    if name_c.starts_with(&needle_c) {
        structural = 93;
    } else if name_c.contains(&needle_c) {
        structural = 86;
    }
    let needle_tokens = tokens(needle);
    let name_tokens = tokens(name);
    let mut token_fuzzy = 0;
    if !needle_tokens.is_empty() && !name_tokens.is_empty() {
        let mut structural_floor = i32::MAX;
        let mut fuzzy_floor = i32::MAX;
        for needle_token in &needle_tokens {
            let mut best_structural = 0;
            let mut best_fuzzy = 0;
            for token in &name_tokens {
                let part = token_similarity(needle_token, token);
                best_structural = best_structural.max(part.structural);
                best_fuzzy = best_fuzzy.max(part.fuzzy);
            }
            structural_floor = structural_floor.min(best_structural);
            fuzzy_floor = fuzzy_floor.min(best_fuzzy);
        }
        structural = structural.max(structural_floor);
        token_fuzzy = fuzzy_floor;
    }
    let mut fuzzy = token_fuzzy;
    if needle_c.chars().count() >= 3 && name_c.chars().count() >= 3 {
        fuzzy = fuzzy.max(winkler_score(&needle_c, &name_c));
    }
    Similarity { structural, fuzzy }
}

fn token_similarity(needle: &str, token: &str) -> Similarity {
    if needle == token {
        return Similarity {
            structural: 100,
            fuzzy: 100,
        };
    }
    let mut structural = 0;
    if needle.chars().count() >= 3 && (token.starts_with(needle) || needle.starts_with(token)) {
        structural = 92;
    } else if needle.chars().count() >= 3 && (token.contains(needle) || needle.contains(token)) {
        structural = 88;
    }
    let fuzzy = if needle.chars().count() >= 3 && token.chars().count() >= 3 {
        winkler_score(needle, token).max(structural)
    } else {
        structural
    };
    Similarity { structural, fuzzy }
}

fn tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut last_digit = None;
    for ch in value.chars() {
        if ch == '_' || ch == '-' {
            push_token(&mut tokens, &mut current);
            last_digit = None;
            continue;
        }
        let digit = ch.is_ascii_digit();
        if last_digit == Some(!digit) && !current.is_empty() {
            push_token(&mut tokens, &mut current);
        }
        current.push(ch.to_ascii_lowercase());
        last_digit = Some(digit);
    }
    push_token(&mut tokens, &mut current);
    tokens
}

fn push_token(tokens: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        tokens.push(std::mem::take(current));
    }
}

fn compact(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

fn winkler_score(left: &str, right: &str) -> i32 {
    (jaro_winkler(left, right) * 100.0).round() as i32
}

fn jaro_winkler(left: &str, right: &str) -> f64 {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    let jaro = jaro(&left, &right);
    let mut prefix = 0;
    for (left_ch, right_ch) in left.iter().zip(right.iter()) {
        if left_ch != right_ch || prefix == 4 {
            break;
        }
        prefix += 1;
    }
    jaro + (prefix as f64) * 0.1 * (1.0 - jaro)
}

fn jaro(left: &[char], right: &[char]) -> f64 {
    if left == right {
        return 1.0;
    }
    let (len_left, len_right) = (left.len(), right.len());
    if len_left == 0 || len_right == 0 {
        return 0.0;
    }
    let window = (len_left.max(len_right) / 2).saturating_sub(1);
    let mut left_matches = vec![false; len_left];
    let mut right_matches = vec![false; len_right];
    let mut matches = 0usize;
    for (index, ch) in left.iter().enumerate() {
        let start = index.saturating_sub(window);
        let end = (index + window + 1).min(len_right);
        for (candidate, candidate_ch) in right.iter().enumerate().take(end).skip(start) {
            if right_matches[candidate] || ch != candidate_ch {
                continue;
            }
            left_matches[index] = true;
            right_matches[candidate] = true;
            matches += 1;
            break;
        }
    }
    if matches == 0 {
        return 0.0;
    }
    let mut transpositions = 0.0;
    let mut paired = 0;
    for (index, ch) in left.iter().enumerate() {
        if !left_matches[index] {
            continue;
        }
        while !right_matches[paired] {
            paired += 1;
        }
        if *ch != right[paired] {
            transpositions += 1.0;
        }
        paired += 1;
    }
    transpositions /= 2.0;
    let matches = matches as f64;
    (matches / len_left as f64 + matches / len_right as f64 + (matches - transpositions) / matches)
        / 3.0
}

fn intelligent_needle(pattern: &str, wildcards: &[char]) -> String {
    pattern
        .chars()
        .filter(|ch| !wildcards.contains(ch))
        .collect::<String>()
        .to_ascii_lowercase()
}

fn intelligent_note(pattern: &str, names: bool, related: bool) -> String {
    let kind = match (names, related) {
        (true, true) => "similar names and related keys",
        (false, true) => "related keys",
        _ => "similar names",
    };
    format!("No exact match for {pattern:?}, so showing {kind}.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(name: &str, hash_key: &str) -> MatchTable {
        MatchTable::named(name, hash_key)
    }

    #[test]
    fn partial_names_are_similar_matches() {
        let tables = vec![
            named("nb_posts", "id"),
            named("posts_v2", "id"),
            named("gamma", "id"),
        ];
        let hit = intelligent_matches(&tables, "post").unwrap();
        assert_eq!(
            hit.names,
            vec!["posts_v2".to_string(), "nb_posts".to_string()]
        );
        assert_eq!(
            hit.note,
            "No exact match for \"post\", so showing similar names."
        );
        assert!(intelligent_matches(&tables, "missing").is_err());
        assert!(intelligent_matches(&tables, "a").is_err());
        assert!(intelligent_matches(&tables, "id").is_err());
    }

    #[test]
    fn like_percent_is_not_part_of_the_needle() {
        let tables = vec![named("posts", "id"), named("gamma", "id")];
        let hit = intelligent_like_matches(&tables, "post%").unwrap();
        assert_eq!(hit.names, vec!["posts".to_string()]);
        assert_eq!(
            hit.note,
            "No exact match for \"post%\", so showing similar names."
        );
    }

    #[test]
    fn prefix_match_skips_a_near_duplicate() {
        let tables = vec![named("shotalpha", "id"), named("shotbeta", "id")];
        let hit = intelligent_matches(&tables, "shotalp").unwrap();
        assert_eq!(hit.names, vec!["shotalpha".to_string()]);

        let long = vec![
            named("pwmv2jduon2oje95alpha", "id"),
            named("pwmv2jduon2oje95beta", "id"),
        ];
        let hit = intelligent_matches(&long, "pwmv2jduon2oje95alp").unwrap();
        assert_eq!(hit.names, vec!["pwmv2jduon2oje95alpha".to_string()]);
    }

    #[test]
    fn typos_match_similar_names() {
        let tables = vec![
            named("posts", "id"),
            named("users", "id"),
            named("gamma", "id"),
        ];
        let posts = intelligent_matches(&tables, "psots").unwrap();
        assert_eq!(posts.names, vec!["posts".to_string()]);
        let users = intelligent_matches(&tables, "usr").unwrap();
        assert_eq!(users.names, vec!["users".to_string()]);
    }

    #[test]
    fn related_keys_match_when_names_do_not() {
        let tables = vec![
            named("orders", "customer_id"),
            named("invoices", "customer_id"),
            named("gamma", "id"),
        ];
        let hit = intelligent_matches(&tables, "customer_id").unwrap();
        assert_eq!(
            hit.names,
            vec!["orders".to_string(), "invoices".to_string()]
        );
        assert_eq!(
            hit.note,
            "No exact match for \"customer_id\", so showing related keys."
        );
    }

    #[test]
    fn similar_names_and_related_keys_share_one_list() {
        let tables = vec![
            named("orders", "order_id"),
            named("order_items", "order_id"),
            named("shipments", "order_id"),
            named("gamma", "id"),
        ];
        let hit = intelligent_matches(&tables, "order").unwrap();
        assert_eq!(
            hit.names,
            vec![
                "order_items".to_string(),
                "orders".to_string(),
                "shipments".to_string()
            ]
        );
        assert_eq!(
            hit.note,
            "No exact match for \"order\", so showing similar names and related keys."
        );
    }

    #[test]
    fn index_names_are_related_keys() {
        let mut by_status = named("orders", "id");
        by_status.fields.push("by_status".to_string());
        let hit = intelligent_matches(&[by_status, named("gamma", "id")], "by_status").unwrap();
        assert_eq!(hit.names, vec!["orders".to_string()]);
        assert!(hit.note.contains("related keys"));
    }
}
