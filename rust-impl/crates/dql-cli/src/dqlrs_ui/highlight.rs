//! Line highlighter for DQL. Keywords, strings, and comments.
//!
//! Lint underlines are a list under the editor. A line highlighter cannot draw
//! a squiggle on a byte range, so the issue list is the usable form of that mark.

use std::ops::Range;

use iced::advanced::text::highlighter::{Format, Highlighter};
use iced::widget::text_editor;
use iced::{Color, Font};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Keyword,
    String,
    Comment,
    Write,
}

impl Tone {
    pub fn format(tone: &Self, _theme: &iced::Theme) -> Format<Font> {
        let color = match *tone {
            Self::Plain => None,
            Self::Keyword => Some(Color::from_rgb8(0x0f, 0x76, 0x6e)),
            Self::String => Some(Color::from_rgb8(0xb4, 0x53, 0x09)),
            Self::Comment => Some(Color::from_rgb8(0x6b, 0x72, 0x80)),
            Self::Write => Some(Color::from_rgb8(0xb9, 0x1c, 0x1c)),
        };
        let font = match *tone {
            Self::Keyword | Self::Write => Some(Font {
                weight: iced::font::Weight::Bold,
                ..Font::MONOSPACE
            }),
            _ => None,
        };
        Format { color, font }
    }
}

#[derive(Debug)]
pub struct DqlHighlighter {
    current_line: usize,
}

impl Highlighter for DqlHighlighter {
    type Settings = ();
    type Highlight = Tone;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Tone)>;

    fn new(_settings: &Self::Settings) -> Self {
        Self { current_line: 0 }
    }

    fn update(&mut self, _settings: &Self::Settings) {}

    fn change_line(&mut self, line: usize) {
        self.current_line = line;
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        spans(line).into_iter()
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

pub fn editor<'a, Message: Clone + 'a>(
    content: &'a text_editor::Content,
) -> text_editor::TextEditor<'a, DqlHighlighter, Message> {
    text_editor(content)
        .font(Font::MONOSPACE)
        .highlight_with::<DqlHighlighter>((), Tone::format)
}

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
    "LOAD",
    "EXPLAIN",
    "ANALYZE",
    "SHOW",
    "LIKE",
    "DESCRIBE",
    "LS",
    "HELP",
    "SCHEMA",
    "PARENTS",
    "SAVE",
    "FILTER",
    "BEGINS",
    "WITH",
    "NULL",
    "TRUE",
    "FALSE",
    "THROTTLE",
];

pub(super) fn spans(line: &str) -> Vec<(Range<usize>, Tone)> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("--") {
        return vec![(0..line.len(), Tone::Comment)];
    }
    let write = first_word(trimmed).is_some_and(|word| {
        matches!(
            word.to_ascii_uppercase().as_str(),
            "INSERT" | "UPDATE" | "DELETE" | "DROP" | "CREATE"
        )
    });
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\'' || bytes[i] == b'"' {
            let quote = bytes[i];
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != quote {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            out.push((start..i, Tone::String));
            continue;
        }
        if bytes[i].is_ascii_alphabetic() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &line[start..i];
            let tone = if KEYWORDS.iter().any(|kw| kw.eq_ignore_ascii_case(word)) {
                if write && start == line.len() - line.trim_start().len() {
                    Tone::Write
                } else {
                    Tone::Keyword
                }
            } else {
                Tone::Plain
            };
            if tone != Tone::Plain {
                out.push((start..i, tone));
            }
            continue;
        }
        i += 1;
    }
    out
}

fn first_word(text: &str) -> Option<&str> {
    text.split_whitespace().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_keywords_strings_and_write_statements() {
        let line = "SELECT id FROM widgets; -- note";
        let painted = spans(line);
        assert!(painted
            .iter()
            .any(|(range, tone)| { &line[range.clone()] == "SELECT" && *tone == Tone::Keyword }));
        let write = spans("INSERT INTO widgets VALUES ('a')");
        assert!(write.iter().any(|(_, tone)| *tone == Tone::Write));
        assert!(write.iter().any(|(range, tone)| {
            let text = &"INSERT INTO widgets VALUES ('a')"[range.clone()];
            text.contains('a') && *tone == Tone::String
        }));
        assert_eq!(spans("  -- hidden").len(), 1);
        let keyword = Tone::format(&Tone::Keyword, &iced::Theme::Light);
        let face = keyword.font.expect("keywords are bold");
        assert_eq!(face.weight, iced::font::Weight::Bold);
        assert_eq!(face.family, iced::font::Family::Monospace);
        let write = Tone::format(&Tone::Write, &iced::Theme::Light);
        assert_eq!(
            write.font.expect("write keywords are bold").weight,
            iced::font::Weight::Bold
        );
        assert!(Tone::format(&Tone::Plain, &iced::Theme::Light)
            .font
            .is_none());
        assert!(Tone::format(&Tone::String, &iced::Theme::Light)
            .font
            .is_none());
        assert!(Tone::format(&Tone::Comment, &iced::Theme::Light)
            .font
            .is_none());
    }
}
