use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

const KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "AND",
    "OR",
    "NOT",
    "INSERT",
    "INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    "CREATE",
    "DROP",
    "TABLE",
    "ALTER",
    "SCAN",
    "INDEX",
    "HASH",
    "RANGE",
    "KEY",
    "KEYS",
    "THROUGHPUT",
    "TP",
    "THROTTLE",
    "GLOBAL",
    "LOCAL",
    "INCLUDE",
    "ALL",
    "IF",
    "EXISTS",
    "LIMIT",
    "ORDER",
    "BY",
    "ASC",
    "DESC",
    "BETWEEN",
    "IN",
    "USING",
    "CONSISTENT",
    "RETURNS",
    "DUMP",
    "LOAD",
    "EXPLAIN",
    "ANALYZE",
    "SAVE",
    "FILTER",
    "BEGINS",
    "WITH",
    "ATTRIBUTE_EXISTS",
    "ATTRIBUTE_NOT_EXISTS",
    "ATTRIBUTE_TYPE",
    "SIZE",
    "NULL",
    "TRUE",
    "FALSE",
];

fn is_keyword(word: &str) -> bool {
    KEYWORDS.iter().any(|kw| kw.eq_ignore_ascii_case(word))
}

fn keyword_style() -> Style {
    Style::default()
        .fg(Color::Magenta)
        .add_modifier(Modifier::BOLD)
}

fn string_style() -> Style {
    Style::default().fg(Color::Green)
}

fn number_style() -> Style {
    Style::default().fg(Color::Yellow)
}

fn comment_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn punct_style() -> Style {
    Style::default().fg(Color::Cyan)
}

/// Syntax-highlight a single line of DQL / SQL-like input.
pub fn highlight_line(text: &str) -> Line<'static> {
    let mut spans = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        // Line comment
        if ch == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
            let rest: String = chars[i..].iter().collect();
            spans.push(Span::styled(rest, comment_style()));
            break;
        }

        // String literal (single or double quotes)
        if ch == '\'' || ch == '"' {
            let quote = ch;
            let start = i;
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 2;
                    continue;
                }
                if chars[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let lit: String = chars[start..i].iter().collect();
            spans.push(Span::styled(lit, string_style()));
            continue;
        }

        // Number
        if ch.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let num: String = chars[start..i].iter().collect();
            spans.push(Span::styled(num, number_style()));
            continue;
        }

        // Identifier / keyword
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if is_keyword(&word) {
                spans.push(Span::styled(word, keyword_style()));
            } else {
                spans.push(Span::raw(word));
            }
            continue;
        }

        // Whitespace
        if ch.is_whitespace() {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            let ws: String = chars[start..i].iter().collect();
            spans.push(Span::raw(ws));
            continue;
        }

        // Punctuation / operators
        if matches!(
            ch,
            '=' | '<'
                | '>'
                | '!'
                | ','
                | '('
                | ')'
                | '*'
                | '+'
                | '-'
                | '/'
                | '%'
                | ';'
                | '.'
                | '['
                | ']'
        ) {
            spans.push(Span::styled(ch.to_string(), punct_style()));
            i += 1;
            continue;
        }

        spans.push(Span::raw(ch.to_string()));
        i += 1;
    }

    if spans.is_empty() {
        Line::from("")
    } else {
        Line::from(spans)
    }
}

/// Highlight each line of a multi-line command.
pub fn highlight_command(lines: &[String]) -> Vec<Line<'static>> {
    lines.iter().map(|line| highlight_line(line)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_select_keyword() {
        let line = highlight_line("SELECT * FROM users");
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "SELECT * FROM users");
        assert!(line
            .spans
            .iter()
            .any(|s| { s.content.as_ref() == "SELECT" && s.style.fg == Some(Color::Magenta) }));
    }
}
