//! Color for what the `dqlrs` command prints to the console: the startup blocks of
//! `dqlrs web`, `--mcp` and `dqlrs notebook`, `--help`, and warnings and errors.
//!
//! A stream is colored only when it is a terminal and `NO_COLOR` is unset, so piped output,
//! scripts, and tests see exactly the plain text. The TUI does its own styling; query results,
//! JSON, and `--version` stay plain.

use std::io::IsTerminal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Style {
    color: bool,
}

const BOLD: &str = "1";
const DIM: &str = "2";
const RED: &str = "31";
const YELLOW: &str = "33";
const CYAN: &str = "36";
const BOLD_CYAN: &str = "1;36";
const UNDERLINE_CYAN: &str = "4;36";

impl Style {
    /// Colors for stdout.
    pub(crate) fn stdout() -> Self {
        Self::for_terminal(std::io::stdout().is_terminal())
    }

    /// Colors for stderr.
    pub(crate) fn stderr() -> Self {
        Self::for_terminal(std::io::stderr().is_terminal())
    }

    fn for_terminal(terminal: bool) -> Self {
        Self {
            color: terminal && std::env::var_os("NO_COLOR").is_none(),
        }
    }

    /// Always plain or always colored (tests).
    #[cfg(test)]
    pub(crate) fn with_color(color: bool) -> Self {
        Self { color }
    }

    pub(crate) fn paint(&self, text: &str, code: &str) -> String {
        if self.color && !text.is_empty() {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    /// A product name or headline, such as `DQLRS Web`.
    pub(crate) fn title(&self, text: &str) -> String {
        self.paint(text, BOLD_CYAN)
    }

    /// A section heading, such as `Usage:`.
    pub(crate) fn heading(&self, text: &str) -> String {
        self.paint(text, BOLD)
    }

    /// The label column of a `label   value` block.
    pub(crate) fn label(&self, text: &str) -> String {
        self.paint(text, DIM)
    }

    /// A URL to open.
    pub(crate) fn url(&self, text: &str) -> String {
        self.paint(text, UNDERLINE_CYAN)
    }

    /// A flag or command to type.
    pub(crate) fn code(&self, text: &str) -> String {
        self.paint(text, CYAN)
    }

    /// Secondary detail.
    pub(crate) fn dim(&self, text: &str) -> String {
        self.paint(text, DIM)
    }

    pub(crate) fn warn(&self, text: &str) -> String {
        self.paint(text, YELLOW)
    }

    pub(crate) fn error(&self, text: &str) -> String {
        self.paint(text, RED)
    }

    /// `label` padded to `width` and dimmed, then `value`: one line of a startup block.
    pub(crate) fn row(&self, label: &str, width: usize, value: &str) -> String {
        format!("{}{value}", self.label(&format!("{label:<width$}")))
    }

    /// `dqlrs --help`: headings bold, and on option and usage lines the flags or command cyan.
    pub(crate) fn help(&self, text: &str) -> String {
        if !self.color {
            return text.to_string();
        }
        text.lines()
            .map(|line| self.help_line(line))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn help_line(&self, line: &str) -> String {
        if !line.starts_with(' ') && line.ends_with(':') {
            return self.heading(line);
        }
        let indent = line.len() - line.trim_start().len();
        let body = &line[indent..];
        // `  -c, --command <command>  Run this command`: the part before the two-space gap is
        // what you type. Continuation lines (no flag, deep indent) stay plain.
        if body.starts_with('-')
            || body.starts_with("dqlrs")
            || body.starts_with("AWS_")
            || body.starts_with("DQL_")
        {
            let (typed, rest) = match body.find("  ") {
                Some(gap) => body.split_at(gap),
                None => (body, ""),
            };
            return format!("{}{}{}", &line[..indent], self.code(typed), self.dim(rest));
        }
        if indent > 0 {
            return format!("{}{}", &line[..indent], self.dim(body));
        }
        line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_style_returns_the_text_unchanged() {
        let plain = Style::with_color(false);
        assert_eq!(plain.title("DQLRS Web"), "DQLRS Web");
        assert_eq!(plain.row("MCP", 11, "off"), "MCP        off");
        let help = "Usage:\n  dqlrs web [options]      Serve\n";
        assert_eq!(plain.help(help), help);
    }

    #[test]
    fn colored_rows_dim_the_padded_label() {
        let color = Style::with_color(true);
        assert_eq!(color.row("MCP", 11, "off"), "\x1b[2mMCP        \x1b[0moff");
        assert_eq!(color.paint("", "1"), "");
    }

    #[test]
    fn help_colors_headings_and_what_you_type() {
        let color = Style::with_color(true);
        let help = color.help("Options:\n  -c, --command <command>  Run this command and exit\n                           more text");
        let lines: Vec<&str> = help.lines().collect();
        assert_eq!(lines[0], "\x1b[1mOptions:\x1b[0m");
        assert_eq!(
            lines[1],
            "  \x1b[36m-c, --command <command>\x1b[0m\x1b[2m  Run this command and exit\x1b[0m"
        );
        assert_eq!(
            lines[2],
            "                           \x1b[2mmore text\x1b[0m"
        );
    }
}
