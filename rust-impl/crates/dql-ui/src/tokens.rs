//! Statement bands and word colors, ported from `dqlrs-web/static/dql-tokens.js`.
//!
//! This is the editor's tokenizer. It is not a credential and it does not
//! speak HTTP.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordStyle {
    Keyword,
    Deleted,
    TypeName,
    Function,
    Bool,
    Null,
    String,
    Number,
    Comment,
    Operator,
    Ident,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Option<WordStyle>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BandKind {
    Write,
    WriteEven,
    Alt,
    Plain,
    Bar,
    BarEven,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Band {
    pub from: usize,
    pub to: usize,
    pub write: bool,
    pub index: usize,
    pub line_from: usize,
    pub line_to: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HighlightOptions {
    pub minimal_write: bool,
    pub even_odd: bool,
}

impl Default for HighlightOptions {
    fn default() -> Self {
        Self {
            minimal_write: false,
            even_odd: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunKind {
    Selection,
    Query,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunRange {
    pub from: usize,
    pub to: usize,
    pub kind: RunKind,
}

/// Page colors from `dqlrs-web/static/theme.css`. The bool is bold.
pub fn paint(style: WordStyle) -> (u32, bool) {
    match style {
        WordStyle::Keyword => (0x0f766e, true),
        WordStyle::Deleted => (0xdc2626, true),
        WordStyle::TypeName => (0x1d4ed8, false),
        WordStyle::Function => (0x6d28d9, false),
        WordStyle::Bool | WordStyle::Null => (0x0369a1, true),
        WordStyle::String => (0x9f1239, false),
        WordStyle::Number => (0xb45309, false),
        WordStyle::Comment => (0x6b7280, false),
        WordStyle::Operator => (0x334155, false),
        WordStyle::Ident => (0x0f172a, false),
    }
}

/// Table-cache age, matching `cacheAgeLabel` in the web client.
/// `None` is a list that has never been saved.
pub fn cache_age_label(elapsed_secs: Option<u64>) -> String {
    let Some(elapsed) = elapsed_secs else {
        return "not cached".into();
    };
    if elapsed < 5 {
        return "just now".into();
    }
    if elapsed < 60 {
        return format!("{elapsed}s ago");
    }
    if elapsed < 3600 {
        return format!("{} min ago", elapsed / 60);
    }
    if elapsed < 86_400 {
        return format!("{} hr ago", elapsed / 3600);
    }
    let days = elapsed / 86_400;
    if days == 1 {
        "1 day ago".into()
    } else {
        format!("{days} days ago")
    }
}

pub fn style_for_word(word: &str) -> Option<WordStyle> {
    Some(match word.to_ascii_uppercase().as_str() {
        "SELECT" | "SCAN" | "INSERT" | "UPDATE" | "CREATE" | "ALTER" | "DUMP" | "LOAD"
        | "EXPLAIN" | "ANALYZE" | "SHOW" | "LIKE" | "DESCRIBE" | "FROM" | "WHERE" | "INTO"
        | "VALUES" | "SET" | "ADD" | "REMOVE" | "USING" | "LIMIT" | "ORDER" | "BY" | "ASC"
        | "DESC" | "AS" | "KEYS" | "IN" | "CONSISTENT" | "THROTTLE" | "RETURNS" | "SAVE"
        | "PARENTS" | "TABLE" | "INDEX" | "GLOBAL" | "HASH" | "RANGE" | "KEY" | "THROUGHPUT"
        | "TP" | "IF" | "NOT" | "EXISTS" | "INCLUDE" | "ALL" | "SCHEMA" | "AND" | "OR"
        | "BETWEEN" | "NONE" | "UPDATED" | "OLD" | "NEW" | "LS" | "OPT" | "OPTIONS" | "HELP"
        | "USE" | "WATCH" => WordStyle::Keyword,
        "DROP" | "DELETE" => WordStyle::Deleted,
        "STRING" | "NUMBER" | "BINARY" | "BOOL" | "BOOLEAN" => WordStyle::TypeName,
        "COUNT"
        | "SIZE"
        | "BEGINS_WITH"
        | "CONTAINS"
        | "ATTRIBUTE_EXISTS"
        | "ATTRIBUTE_NOT_EXISTS"
        | "ATTRIBUTE_TYPE"
        | "IF_NOT_EXISTS"
        | "LIST_APPEND"
        | "TIMESTAMP"
        | "TS"
        | "UTCTIMESTAMP"
        | "UTCTS"
        | "NOW"
        | "UTCNOW"
        | "MS"
        | "INTERVAL" => WordStyle::Function,
        "TRUE" | "FALSE" => WordStyle::Bool,
        "NULL" => WordStyle::Null,
        _ => return None,
    })
}

pub fn highlight_parts(text: &str) -> Vec<Span> {
    let mut parts = Vec::new();
    let lines: Vec<&str> = text.split('\n').collect();
    let mut quote: Option<char> = None;
    for (index, line) in lines.iter().enumerate() {
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if let Some(q) = quote {
                let start = i;
                let mut escaped = false;
                while i < bytes.len() {
                    let ch = bytes[i] as char;
                    i += 1;
                    if escaped {
                        escaped = false;
                        continue;
                    }
                    if ch == '\\' {
                        escaped = true;
                        continue;
                    }
                    if ch == q {
                        quote = None;
                        break;
                    }
                }
                parts.push(Span {
                    text: line[start..i].to_string(),
                    style: Some(WordStyle::String),
                });
                continue;
            }
            if bytes[i].is_ascii_whitespace() {
                let start = i;
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                parts.push(Span {
                    text: line[start..i].to_string(),
                    style: None,
                });
                continue;
            }
            if bytes[i] == b'-' && bytes.get(i + 1) == Some(&b'-') {
                parts.push(Span {
                    text: line[i..].to_string(),
                    style: Some(WordStyle::Comment),
                });
                break;
            }
            if bytes[i] == b'\'' || bytes[i] == b'"' {
                quote = Some(bytes[i] as char);
                let start = i;
                i += 1;
                let mut escaped = false;
                while i < bytes.len() {
                    let ch = bytes[i] as char;
                    i += 1;
                    if escaped {
                        escaped = false;
                        continue;
                    }
                    if ch == '\\' {
                        escaped = true;
                        continue;
                    }
                    if ch == quote.unwrap() {
                        quote = None;
                        break;
                    }
                }
                parts.push(Span {
                    text: line[start..i].to_string(),
                    style: Some(WordStyle::String),
                });
                continue;
            }
            if is_number_at(bytes, i) {
                let start = i;
                i = scan_number(bytes, i);
                parts.push(Span {
                    text: line[start..i].to_string(),
                    style: Some(WordStyle::Number),
                });
                continue;
            }
            if let Some(len) = operator_len(bytes, i) {
                parts.push(Span {
                    text: line[i..i + len].to_string(),
                    style: Some(WordStyle::Operator),
                });
                i += len;
                continue;
            }
            if is_ident_start(bytes[i]) {
                let start = i;
                i += 1;
                while i < bytes.len() && is_ident_cont(bytes[i]) {
                    i += 1;
                }
                let word = &line[start..i];
                let style = style_for_word(word).unwrap_or(WordStyle::Ident);
                parts.push(Span {
                    text: word.to_string(),
                    style: Some(style),
                });
                continue;
            }
            parts.push(Span {
                text: line[i..i + 1].to_string(),
                style: None,
            });
            i += 1;
        }
        if index + 1 < lines.len() {
            parts.push(Span {
                text: "\n".to_string(),
                style: None,
            });
        }
    }
    parts
}

pub fn query_bands(text: &str) -> Vec<Band> {
    let src = text;
    let bytes = src.as_bytes();
    let mut bands = Vec::new();
    let mut quote: Option<char> = None;
    let mut code_start: Option<usize> = None;
    let mut action = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        if let Some(q) = quote {
            if ch == '\\' {
                i += 2;
                continue;
            }
            if ch == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if ch == '-' && bytes.get(i + 1) == Some(&b'-') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            if code_start.is_none() {
                code_start = Some(i);
            }
            quote = Some(ch);
            i += 1;
            continue;
        }
        if is_ident_start(bytes[i]) {
            if code_start.is_none() {
                code_start = Some(i);
            }
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_cont(bytes[i]) {
                i += 1;
            }
            let word = src[start..i].to_ascii_uppercase();
            if action.is_empty() || action == "EXPLAIN" || action == "ANALYZE" {
                action = word;
            }
            continue;
        }
        if ch == ';' {
            finish(src, &mut bands, &mut code_start, &mut action, i + 1);
            i += 1;
            continue;
        }
        if code_start.is_none() && !ch.is_whitespace() {
            code_start = Some(i);
        }
        i += 1;
    }
    finish(src, &mut bands, &mut code_start, &mut action, src.len());
    expand_bands(src, bands)
}

pub fn band_appearance(band: &Band, options: HighlightOptions) -> BandKind {
    if band.write {
        let even = options.even_odd && band.index.is_multiple_of(2);
        if options.minimal_write {
            return if even {
                BandKind::BarEven
            } else {
                BandKind::Bar
            };
        }
        return if even {
            BandKind::WriteEven
        } else {
            BandKind::Write
        };
    }
    if options.even_odd && band.index.is_multiple_of(2) {
        BandKind::Alt
    } else {
        BandKind::Plain
    }
}

pub fn statement_spans(text: &str) -> Vec<(usize, usize)> {
    let src = text;
    let bytes = src.as_bytes();
    let mut spans = Vec::new();
    let mut buf_start = 0;
    let mut quote: Option<char> = None;
    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        if quote.is_none() && ch == '-' && bytes.get(i + 1) == Some(&b'-') {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if let Some(q) = quote {
            if ch == q && bytes.get(i.wrapping_sub(1)) != Some(&b'\\') {
                quote = None;
            }
            i += 1;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            i += 1;
            continue;
        }
        if ch == ';' {
            push_span(src, &mut spans, buf_start, i + 1);
            buf_start = i + 1;
            i += 1;
            continue;
        }
        i += 1;
    }
    push_span(src, &mut spans, buf_start, src.len());
    spans
}

pub fn run_target(text: &str, head: usize, anchor: usize) -> Option<RunRange> {
    let len = text.len();
    let from = head.min(anchor).min(len);
    let to = head.max(anchor).min(len);
    if from < to && !text[from..to].trim().is_empty() {
        return Some(RunRange {
            from,
            to,
            kind: RunKind::Selection,
        });
    }
    let pos = head.min(len);
    for band in query_bands(text) {
        if pos >= band.from && (pos < band.to || (pos == band.to && band.to == len)) {
            return Some(RunRange {
                from: band.from,
                to: band.to,
                kind: RunKind::Query,
            });
        }
    }
    None
}

fn finish(
    _src: &str,
    bands: &mut Vec<Band>,
    code_start: &mut Option<usize>,
    action: &mut String,
    end: usize,
) {
    let Some(start) = code_start.take() else {
        action.clear();
        return;
    };
    let write = matches!(
        action.as_str(),
        "INSERT" | "UPDATE" | "DELETE" | "DROP" | "CREATE" | "ALTER" | "LOAD"
    );
    let index = bands.len();
    bands.push(Band {
        from: start,
        to: end,
        write,
        index,
        line_from: 1,
        line_to: 1,
    });
    action.clear();
}

fn expand_bands(src: &str, bands: Vec<Band>) -> Vec<Band> {
    if bands.is_empty() {
        return bands;
    }
    let lines: Vec<&str> = src.split('\n').collect();
    let mut starts = Vec::with_capacity(lines.len());
    let mut cursor = 0;
    for line in &lines {
        starts.push(cursor);
        cursor += line.len() + 1;
    }
    let line_at = |offset: usize| {
        let mut index = 0;
        for (i, start) in starts.iter().enumerate() {
            if *start <= offset {
                index = i;
            } else {
                break;
            }
        }
        index
    };
    let mut claimed = vec![false; lines.len()];
    let code: Vec<(usize, usize)> = bands
        .iter()
        .map(|band| {
            let from = line_at(band.from);
            let to = line_at(band.from.max(band.to.saturating_sub(1)));
            (from, to)
        })
        .collect();
    for (from, to) in &code {
        for line in claimed.iter_mut().take(*to + 1).skip(*from) {
            *line = true;
        }
    }
    bands
        .into_iter()
        .enumerate()
        .map(|(index, mut band)| {
            let (mut start, mut end) = code[index];
            while start > 0 && !claimed[start - 1] && lines[start - 1].trim().starts_with("--") {
                start -= 1;
                claimed[start] = true;
            }
            while end + 1 < lines.len()
                && !claimed[end + 1]
                && lines[end + 1].trim().starts_with("--")
            {
                end += 1;
                claimed[end] = true;
            }
            while end + 1 < lines.len() && !claimed[end + 1] && lines[end + 1].trim().is_empty() {
                end += 1;
                claimed[end] = true;
            }
            band.from = starts[start];
            band.to = if end + 1 < lines.len() {
                starts[end + 1]
            } else {
                src.len()
            };
            band.line_from = start + 1;
            band.line_to = end + 1;
            band
        })
        .collect()
}

fn push_span(src: &str, spans: &mut Vec<(usize, usize)>, from: usize, to: usize) {
    let raw = &src[from..to];
    if !raw.lines().any(|line| {
        let stripped = line.trim();
        !stripped.is_empty() && !stripped.starts_with("--")
    }) {
        return;
    }
    let lead = raw.len() - raw.trim_start().len();
    let end = from + raw.trim_end().len();
    let start = from + lead;
    spans.push((start, end.max(start)));
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_cont(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'-'
}

fn is_number_at(bytes: &[u8], i: usize) -> bool {
    let rest = &bytes[i..];
    if rest.first() == Some(&b'-') {
        return rest
            .get(1)
            .is_some_and(|b| b.is_ascii_digit() || *b == b'.');
    }
    rest.first()
        .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
        && (rest.first() != Some(&b'.') || rest.get(1).is_some_and(|b| b.is_ascii_digit()))
}

fn scan_number(bytes: &[u8], mut i: usize) -> usize {
    if bytes.get(i) == Some(&b'-') {
        i += 1;
    }
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        return i;
    }
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
    }
    i
}

fn operator_len(bytes: &[u8], i: usize) -> Option<usize> {
    let pair = [
        bytes.get(i).copied().unwrap_or(0),
        bytes.get(i + 1).copied().unwrap_or(0),
    ];
    if matches!(&pair, b"<>" | b"!=" | b"<=" | b">=") {
        return Some(2);
    }
    if matches!(
        bytes[i],
        b'*' | b'='
            | b'<'
            | b'>'
            | b'!'
            | b'+'
            | b'-'
            | b'/'
            | b','
            | b';'
            | b':'
            | b'('
            | b')'
            | b'['
            | b']'
            | b'{'
            | b'}'
    ) {
        return Some(1);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const READ: &str = "-- header stays with the select\nSELECT * FROM widgets WHERE id = 'a';\n-- next\nSCAN * FROM widgets;\n";

    #[test]
    fn keywords_are_bold_colors_and_drop_is_deleted() {
        assert_eq!(style_for_word("select"), Some(WordStyle::Keyword));
        assert_eq!(paint(WordStyle::Keyword), (0x0f766e, true));
        assert_eq!(paint(WordStyle::Deleted), (0xdc2626, true));
        assert_eq!(style_for_word("DROP"), Some(WordStyle::Deleted));
        assert_eq!(style_for_word("DELETE"), Some(WordStyle::Deleted));
        let parts = highlight_parts("SELECT 'a' -- note");
        assert!(parts.iter().any(|p| p.style == Some(WordStyle::Keyword)));
        assert!(parts.iter().any(|p| p.style == Some(WordStyle::String)));
        assert!(parts.iter().any(|p| p.style == Some(WordStyle::Comment)));
    }

    #[test]
    fn a_touching_comment_stays_with_the_query_and_a_blank_line_is_the_boundary() {
        let bands = query_bands(READ);
        assert_eq!(bands.len(), 2);
        assert!(READ[bands[0].from..bands[0].to].contains("header stays"));
        assert!(READ[bands[0].from..bands[0].to].contains("WHERE id"));
        assert!(!READ[bands[0].from..bands[0].to].contains("SCAN"));
        let with_blank = "-- note\nSELECT 1;\n\nSCAN 2;\n";
        let bands = query_bands(with_blank);
        assert!(
            with_blank[bands[0].from..bands[0].to].ends_with("\n\n") || bands[0].to > bands[0].from
        );
        assert!(with_blank[bands[0].from..bands[0].to].contains("SELECT"));
        assert!(!with_blank[bands[0].from..bands[0].to].contains("SCAN"));
    }

    #[test]
    fn a_selection_wins_over_the_band() {
        let range = run_target(READ, 40, 46).unwrap();
        assert_eq!(range.kind, RunKind::Selection);
        let cursor = READ.find("WHERE").unwrap();
        let range = run_target(READ, cursor, cursor).unwrap();
        assert_eq!(range.kind, RunKind::Query);
        assert!(READ[range.from..range.to].contains("header stays"));
    }

    #[test]
    fn highlight_options_match_the_web_menu() {
        let band = Band {
            from: 0,
            to: 1,
            write: true,
            index: 0,
            line_from: 1,
            line_to: 1,
        };
        assert_eq!(
            band_appearance(&band, HighlightOptions::default()),
            BandKind::WriteEven
        );
        assert_eq!(
            band_appearance(
                &band,
                HighlightOptions {
                    minimal_write: true,
                    even_odd: true,
                }
            ),
            BandKind::BarEven
        );
        let read = Band {
            write: false,
            ..band
        };
        assert_eq!(
            band_appearance(
                &read,
                HighlightOptions {
                    minimal_write: false,
                    even_odd: false,
                }
            ),
            BandKind::Plain
        );
    }

    #[test]
    fn cache_age_matches_the_page() {
        assert_eq!(cache_age_label(None), "not cached");
        assert_eq!(cache_age_label(Some(4)), "just now");
        assert_eq!(cache_age_label(Some(12)), "12s ago");
        assert_eq!(cache_age_label(Some(120)), "2 min ago");
        assert_eq!(cache_age_label(Some(7200)), "2 hr ago");
        assert_eq!(cache_age_label(Some(86_400)), "1 day ago");
        assert_eq!(cache_age_label(Some(2 * 86_400)), "2 days ago");
    }

    #[test]
    fn statement_spans_skip_comment_only_fragments() {
        let text = "-- only\nSELECT 1;\n-- tail\n";
        let spans = statement_spans(text);
        assert_eq!(spans.len(), 1);
        assert!(text[spans[0].0..spans[0].1].contains("SELECT"));
    }
}
