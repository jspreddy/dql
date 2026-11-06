//! Query bands and the run target, matching `dqlrs-web/static/dql-tokens.js`.
//!
//! A band starts at the first code of a statement and grows backward over
//! comment lines that touch it, forward over comment lines, then forward over
//! blank lines. A blank line stays with the query above it. A selection that
//! contains non-whitespace is run as-is; otherwise the band under the caret is.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunKind {
    Selection,
    Query,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunTarget {
    pub from: usize,
    pub to: usize,
    pub kind: RunKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Band {
    pub from: usize,
    pub to: usize,
    pub write: bool,
    pub index: usize,
}

/// How one query band is painted, matching `bandAppearance` in `dql-tokens.js`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BandPaint {
    Plain,
    Alt,
    Write,
    WriteEven,
    Bar,
    BarEven,
}

pub fn band_paint(band: &Band, minimal_write: bool, even_odd: bool) -> BandPaint {
    let even = even_odd && band.index.is_multiple_of(2);
    if band.write {
        if minimal_write {
            return if even {
                BandPaint::BarEven
            } else {
                BandPaint::Bar
            };
        }
        return if even {
            BandPaint::WriteEven
        } else {
            BandPaint::Write
        };
    }
    if even {
        BandPaint::Alt
    } else {
        BandPaint::Plain
    }
}

/// One paint per source line. Lines outside a band stay plain.
pub fn line_paint(text: &str, minimal_write: bool, even_odd: bool) -> Vec<BandPaint> {
    let count = if text.is_empty() {
        1
    } else {
        text.split('\n').count()
    };
    let mut paints = vec![BandPaint::Plain; count];
    for band in query_bands(text) {
        let paint = band_paint(&band, minimal_write, even_odd);
        if paint == BandPaint::Plain {
            continue;
        }
        let start = line_index(text, band.from);
        let end = line_index(text, band.to.saturating_sub(1));
        for slot in paints.iter_mut().take(end + 1).skip(start) {
            *slot = paint;
        }
    }
    paints
}

/// Blue window around the glyphs the web `runBox` frames.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub const LINE_H: f32 = 18.0;
pub const PAD_TOP: f32 = 8.0;
pub const PAD_LEFT: f32 = 10.0;
pub const GUTTER_W: f32 = 42.0;
pub const RUN_GAP: f32 = 5.0;
pub const RUN_BORDER: f32 = 1.0;
/// Monospace advance at 13px. Calibrated against the live window.
pub const CHAR_W: f32 = 7.8;
pub const BUTTON: f32 = 22.0;
pub const BUTTON_GAP: f32 = 6.0;

const WRITE: &[&str] = &[
    "INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER", "LOAD",
];

pub fn query_bands(text: &str) -> Vec<Band> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut bands = Vec::new();
    let mut quote: Option<char> = None;
    let mut code_start: Option<usize> = None;
    let mut action = String::new();
    let mut index = 0usize;
    while index < chars.len() {
        let (byte, ch) = chars[index];
        let next = chars.get(index + 1).map(|(_, ch)| *ch);
        if let Some(open) = quote {
            if ch == '\\' {
                index += 2;
                continue;
            }
            if ch == open {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '-' && next == Some('-') {
            index += 2;
            while index < chars.len() && chars[index].1 != '\n' {
                index += 1;
            }
            if index < chars.len() && chars[index].1 == '\n' {
                index += 1;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            if code_start.is_none() {
                code_start = Some(byte);
            }
            quote = Some(ch);
            index += 1;
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            if code_start.is_none() {
                code_start = Some(byte);
            }
            let mut end = index + 1;
            while end < chars.len() && is_word(chars[end].1) {
                end += 1;
            }
            let word: String = chars[index..end]
                .iter()
                .map(|(_, ch)| *ch)
                .collect::<String>()
                .to_ascii_uppercase();
            if action.is_empty() || action == "EXPLAIN" || action == "ANALYZE" {
                action = word;
            }
            index = end;
            continue;
        }
        if ch == ';' {
            finish(
                &mut bands,
                &mut code_start,
                &mut action,
                byte + ch.len_utf8(),
            );
            index += 1;
            continue;
        }
        if code_start.is_none() && !ch.is_whitespace() {
            code_start = Some(byte);
        }
        index += 1;
    }
    finish(&mut bands, &mut code_start, &mut action, text.len());
    expand(text, bands)
}

pub fn run_target(text: &str, head: usize, anchor: usize) -> Option<RunTarget> {
    let len = text.len();
    let head = head.min(len);
    let anchor = anchor.min(len);
    let from = head.min(anchor);
    let to = head.max(anchor);
    if from < to && !text[from..to].trim().is_empty() {
        return Some(RunTarget {
            from,
            to,
            kind: RunKind::Selection,
        });
    }
    for band in query_bands(text) {
        if head >= band.from && (head < band.to || (head == band.to && band.to == len)) {
            return Some(RunTarget {
                from: band.from,
                to: band.to,
                kind: RunKind::Query,
            });
        }
    }
    None
}

/// Whole lines for a selection. A query band keeps its own offsets.
pub fn window_range(text: &str, target: RunTarget) -> (usize, usize) {
    if target.kind != RunKind::Selection {
        return (target.from, target.to);
    }
    let start = line_start(text, target.from.min(text.len()));
    let last = target.from.max(target.to.saturating_sub(1)).min(text.len());
    (start, line_end(text, last))
}

/// Step back when `to` is the first byte of the next line, so the box does not cover it.
pub fn paint_end(text: &str, from: usize, to: usize) -> usize {
    if to > from && to < text.len() && line_start(text, to) == to {
        to - 1
    } else {
        to
    }
}

pub fn frame(text: &str, target: RunTarget) -> Option<Frame> {
    let (from, range_to) = window_range(text, target);
    let to = paint_end(text, from, range_to);
    if from >= to {
        return None;
    }
    let start = line_index(text, from);
    let end = line_index(text, to);
    let mut cols = 0usize;
    for (index, line) in text.split('\n').enumerate() {
        if index < start {
            continue;
        }
        if index > end {
            break;
        }
        cols = cols.max(line.chars().count());
    }
    let text_w = (cols.max(1) as f32) * CHAR_W;
    let text_h = (end - start + 1) as f32 * LINE_H;
    Some(Frame {
        x: GUTTER_W + PAD_LEFT - RUN_GAP - RUN_BORDER,
        y: PAD_TOP + start as f32 * LINE_H - RUN_GAP - RUN_BORDER,
        width: text_w + RUN_GAP * 2.0 + RUN_BORDER * 2.0,
        height: text_h + RUN_GAP * 2.0 + RUN_BORDER * 2.0,
    })
}

/// Viewport position of the circular play button, just left of the frame.
///
/// `scroll_*` and `view_*` are the editor scroller. The button stays inside
/// the visible editor, and is omitted when the frame is fully outside it.
pub fn button_origin(
    frame: Frame,
    scroll_x: f32,
    scroll_y: f32,
    view_w: f32,
    view_h: f32,
) -> Option<(f32, f32)> {
    let box_left = frame.x - scroll_x;
    let box_top = frame.y - scroll_y;
    let box_bottom = box_top + frame.height;
    let box_right = box_left + frame.width;
    if view_h > 0.0 && (box_bottom < 0.0 || box_top > view_h) {
        return None;
    }
    if view_w > 0.0 && (box_right < 0.0 || box_left > view_w) {
        return None;
    }
    let mut left = box_left - BUTTON_GAP - BUTTON;
    if left < 2.0 {
        left = 2.0;
    }
    let mut top = box_top + (frame.height - BUTTON) / 2.0;
    if view_h > 0.0 {
        let min_top = (box_top - 2.0).max(4.0);
        let max_top = (box_top + (frame.height - BUTTON).max(0.0) + 2.0).min(view_h - BUTTON - 4.0);
        if max_top >= min_top {
            top = top.clamp(min_top, max_top);
        }
        top = top.clamp(0.0, (view_h - BUTTON).max(0.0));
    } else {
        top = top.max(0.0);
    }
    Some((left, top))
}

fn finish(bands: &mut Vec<Band>, code_start: &mut Option<usize>, action: &mut String, end: usize) {
    let Some(start) = code_start.take() else {
        return;
    };
    let word = std::mem::take(action);
    let index = bands.len();
    bands.push(Band {
        from: start,
        to: end,
        write: WRITE.contains(&word.as_str()),
        index,
    });
}

fn is_word(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-')
}

fn expand(text: &str, bands: Vec<Band>) -> Vec<Band> {
    if bands.is_empty() {
        return bands;
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let mut starts = Vec::with_capacity(lines.len());
    let mut cursor = 0usize;
    for line in &lines {
        starts.push(cursor);
        cursor += line.len() + 1;
    }
    let line_at = |offset: usize| {
        let mut index = 0usize;
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
            (
                line_at(band.from),
                line_at(band.from.max(band.to.saturating_sub(1))),
            )
        })
        .collect();
    for (from, to) in &code {
        for line in claimed.iter_mut().take(to + 1).skip(*from) {
            *line = true;
        }
    }
    bands
        .into_iter()
        .enumerate()
        .map(|(index, band)| {
            let (mut start, mut end) = code[index];
            while start > 0 && !claimed[start - 1] && is_comment(lines[start - 1]) {
                start -= 1;
                claimed[start] = true;
            }
            while end + 1 < lines.len() && !claimed[end + 1] && is_comment(lines[end + 1]) {
                end += 1;
                claimed[end] = true;
            }
            while end + 1 < lines.len() && !claimed[end + 1] && is_blank(lines[end + 1]) {
                end += 1;
                claimed[end] = true;
            }
            let to = if end + 1 < lines.len() {
                starts[end + 1]
            } else {
                text.len()
            };
            Band {
                from: starts[start],
                to,
                ..band
            }
        })
        .collect()
}

fn is_comment(line: &str) -> bool {
    line.trim().starts_with("--")
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

fn line_start(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    text[..offset]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0)
}

fn line_end(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    text[offset..]
        .find('\n')
        .map(|index| offset + index)
        .unwrap_or(text.len())
}

fn line_index(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    text[..offset].bytes().filter(|byte| *byte == b'\n').count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn band_at(text: &str, head: usize) -> Option<RunTarget> {
        run_target(text, head, head)
    }

    #[test]
    fn comments_and_blank_lines_follow_the_web_bands() {
        let lead = "-- note\nSELECT 1;";
        assert_eq!(
            band_at(lead, 0),
            Some(RunTarget {
                from: 0,
                to: lead.len(),
                kind: RunKind::Query
            })
        );

        let blank = "SELECT 1;\n\nSELECT 2;";
        assert_eq!(band_at(blank, 10).unwrap().from, 0);
        assert_eq!(band_at(blank, 10).unwrap().to, 11);
        assert_eq!(band_at(blank, 11).unwrap().from, 11);

        let trail = "SELECT 1;\n-- about\nSELECT 2;";
        assert_eq!(band_at(trail, 10).unwrap().to, 19);
        assert_eq!(band_at(trail, 19).unwrap().from, 19);

        let after = "SELECT 1;\n\n-- next\nSELECT 2;";
        assert_eq!(band_at(after, 10).unwrap().to, 11);
        assert_eq!(band_at(after, 11).unwrap().from, 11);

        let leading = "\nSELECT 1;";
        assert_eq!(band_at(leading, 0), None);
        assert_eq!(band_at(leading, 1).unwrap().from, 1);

        let eof = "SELECT 1;\n";
        assert_eq!(band_at(eof, eof.len()).unwrap().to, eof.len());

        let write = "-- keep\nINSERT INTO t (id) VALUES (1);\n\n";
        let band = query_bands(write);
        assert_eq!(band.len(), 1);
        assert!(band[0].write);
        assert_eq!(band[0].from, 0);
        assert_eq!(band[0].to, write.len());

        let explained = "EXPLAIN INSERT INTO t VALUES (1);";
        assert!(query_bands(explained)[0].write);
        let quoted = "SELECT 'a;b';";
        assert_eq!(query_bands(quoted).len(), 1);
        assert_eq!(query_bands(quoted)[0].to, quoted.len());
    }

    #[test]
    fn highlight_options_paint_like_the_web_menu() {
        let text = "SELECT 1;\nCREATE TABLE t (id STRING HASH KEY);";
        assert_eq!(
            line_paint(text, false, true),
            vec![BandPaint::Alt, BandPaint::Write]
        );
        assert_eq!(
            line_paint(text, true, true),
            vec![BandPaint::Alt, BandPaint::Bar]
        );
        assert_eq!(
            line_paint(text, false, false),
            vec![BandPaint::Plain, BandPaint::Write]
        );
        assert_eq!(
            line_paint(text, true, false),
            vec![BandPaint::Plain, BandPaint::Bar]
        );
        let even_write = "CREATE TABLE t (id STRING HASH KEY);";
        assert_eq!(
            line_paint(even_write, false, true),
            vec![BandPaint::WriteEven]
        );
        assert_eq!(line_paint(even_write, true, true), vec![BandPaint::BarEven]);
    }

    #[test]
    fn a_non_whitespace_selection_wins_over_the_band() {
        let text = "SELECT 1;\nSELECT 2;";
        let selection = run_target(text, 7, 9).unwrap();
        assert_eq!(
            selection,
            RunTarget {
                from: 7,
                to: 9,
                kind: RunKind::Selection
            }
        );
        assert_eq!(window_range(text, selection), (0, 9));
        assert_eq!(paint_end(text, 0, 10), 9);
        let spaces = run_target(text, 6, 7).unwrap();
        assert_eq!(spaces.kind, RunKind::Query);
        assert_eq!(spaces.from, 0);
    }

    #[test]
    fn the_play_button_sits_left_of_the_frame_and_hides_when_scrolled_away() {
        let frame = Frame {
            x: 46.0,
            y: 2.0,
            width: 80.0,
            height: 30.0,
        };
        let (left, top) = button_origin(frame, 0.0, 0.0, 400.0, 300.0).unwrap();
        assert_eq!(left, 18.0);
        assert!((top - 6.0).abs() < 0.1, "{top}");
        assert!(button_origin(frame, 0.0, 400.0, 400.0, 200.0).is_none());
    }
}
