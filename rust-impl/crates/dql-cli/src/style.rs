//! Color for what the `dqlrs` command prints to the console: the startup blocks of
//! `dqlrs web`, `--mcp` and `dqlrs notebook`, `--help`, setup progress, and warnings and
//! errors.
//!
//! A stream is colored only when it is a terminal and `NO_COLOR` is unset, so piped output,
//! scripts, and tests see exactly the plain text. The TUI does its own styling; query results,
//! `--json`, and `--version` stay plain.
//!
//! [`Style::rich`] colors the recognizable parts of a plain sentence: commands in backticks and
//! flags, URLs, paths, variables and placeholders, and keyboard shortcuts. [`Style::json`]
//! highlights JSON. Neither changes the text, only its color.

use std::io::IsTerminal;
use std::sync::OnceLock;

use regex::Regex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Style {
    color: bool,
}

const DIM: &str = "2";
const RED: &str = "31";
const YELLOW: &str = "33";
const BOLD_CYAN: &str = "1;36";
const HEADING: &str = "1;33";
const CODE: &str = "36";
const URL: &str = "4;94";
const PATH: &str = "34";
const VAR: &str = "35";
const PLACEHOLDER: &str = "3;35";
const KEY: &str = "1;7";
const NOTICE: &str = "1;32";
const PROMPT: &str = "1";
const JSON_KEY: &str = "34";
const JSON_STRING: &str = "32";
const JSON_LITERAL: &str = "35";

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
        Self::new(terminal && std::env::var_os("NO_COLOR").is_none())
    }

    /// Colored or plain, as decided by the caller (the progress module already knows).
    pub(crate) fn new(color: bool) -> Self {
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
        self.paint(text, HEADING)
    }

    /// The label column of a `label   value` block.
    pub(crate) fn label(&self, text: &str) -> String {
        self.paint(text, DIM)
    }

    /// A URL to open.
    pub(crate) fn url(&self, text: &str) -> String {
        self.paint(text, URL)
    }

    /// A file or folder path.
    pub(crate) fn path(&self, text: &str) -> String {
        self.paint(text, PATH)
    }

    /// A command, or a flag, to type.
    pub(crate) fn code(&self, text: &str) -> String {
        self.paint(text, CODE)
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

    /// An important notice, such as `Ready. Serving …`.
    pub(crate) fn notice(&self, text: &str) -> String {
        self.paint(text, NOTICE)
    }

    /// A question waiting for an answer, such as `Continue? [y/N]`.
    pub(crate) fn prompt(&self, text: &str) -> String {
        self.paint(text, PROMPT)
    }

    /// `label` padded to `width` and dimmed, then `value`: one line of a startup block.
    pub(crate) fn row(&self, label: &str, width: usize, value: &str) -> String {
        format!("{}{value}", self.label(&format!("{label:<width$}")))
    }

    /// Color the recognizable parts of `text`; the rest stays as it is.
    pub(crate) fn rich(&self, text: &str) -> String {
        if !self.color {
            return text.to_string();
        }
        let mut out = String::new();
        let mut last = 0;
        for capture in rich_pattern().captures_iter(text) {
            let (whole, code) = if let Some(m) = capture.name("code") {
                (m, CODE)
            } else if let Some(m) = capture.name("url") {
                (m, URL)
            } else if let Some(m) = capture.name("key") {
                (m, KEY)
            } else if let Some(m) = capture.name("var") {
                (m, VAR)
            } else if let Some(m) = capture.name("placeholder") {
                (m, PLACEHOLDER)
            } else if let Some(m) = capture.name("path") {
                (m, PATH)
            } else if let Some(m) = capture.name("flag") {
                (m, CODE)
            } else {
                continue;
            };
            // Leave sentence punctuation after a URL or path uncolored.
            let token = whole.as_str();
            let trimmed = if code == URL || code == PATH {
                token.trim_end_matches(['.', ',', ')', ';', ':'])
            } else {
                token
            };
            out.push_str(&text[last..whole.start()]);
            out.push_str(&self.paint(trimmed, code));
            last = whole.start() + trimmed.len();
        }
        out.push_str(&text[last..]);
        out
    }

    /// Highlight JSON: keys blue, strings green, numbers and `true`/`false`/`null` magenta,
    /// punctuation dim.
    pub(crate) fn json(&self, text: &str) -> String {
        if !self.color {
            return text.to_string();
        }
        let mut out = String::new();
        let mut last = 0;
        for m in json_pattern().captures_iter(text) {
            let whole = m.get(0).expect("match");
            out.push_str(&text[last..whole.start()]);
            let token = whole.as_str();
            if let Some(key) = m.name("key") {
                // The key, then its colon dimmed.
                out.push_str(&self.paint(key.as_str(), JSON_KEY));
                out.push_str(&self.paint(&token[key.as_str().len()..], DIM));
            } else {
                let code = if m.name("string").is_some() {
                    JSON_STRING
                } else if m.name("literal").is_some() {
                    JSON_LITERAL
                } else {
                    DIM
                };
                out.push_str(&self.paint(token, code));
            }
            last = whole.end();
        }
        out.push_str(&text[last..]);
        out
    }

    /// Text that mixes headings (`Cursor (.cursor/mcp.json):`) and JSON blocks, such as the
    /// MCP client config snippets.
    pub(crate) fn config(&self, text: &str) -> String {
        text.lines()
            .map(|line| {
                if !line.starts_with([' ', '{', '}', '[', ']', '"']) && line.ends_with(':') {
                    self.heading(line)
                } else {
                    self.json(line)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// `dqlrs --help`: headings, then on usage and option lines what you type (commands and
    /// flags cyan, placeholders and variables magenta) and a rich description.
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
        let typed_line = body.starts_with('-')
            || body.starts_with("dqlrs")
            || body.starts_with("AWS_")
            || body.starts_with("DQL_");
        if typed_line {
            // `-c, --command <command>  Run this command`: what you type, then the description.
            let (typed, rest) = match body.find("  ") {
                Some(gap) => body.split_at(gap),
                None => (body, ""),
            };
            return format!(
                "{}{}{}",
                &line[..indent],
                self.typed(typed),
                self.rich(rest)
            );
        }
        format!("{}{}", &line[..indent], self.rich(body))
    }

    /// What you type on a help line: variables and placeholders magenta, the rest cyan.
    fn typed(&self, text: &str) -> String {
        let mut out = String::new();
        let mut last = 0;
        for m in typed_pattern().find_iter(text) {
            // `-H` is a flag, not a placeholder named H.
            if text[..m.start()].ends_with('-') {
                continue;
            }
            out.push_str(&self.code(&text[last..m.start()]));
            let token = m.as_str();
            let code =
                if token.starts_with(['<', '[']) || token.chars().all(|c| c.is_ascii_uppercase()) {
                    PLACEHOLDER
                } else {
                    VAR
                };
            out.push_str(&self.paint(token, code));
            last = m.end();
        }
        out.push_str(&self.code(&text[last..]));
        out
    }
}

fn rich_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(concat!(
            r"(?P<code>`[^`]+`|\bdqlrs(?: (?:web|notebook))?(?: --?[a-z][a-z-]*)+)",
            r"|(?P<url>https?://[^\s'`]+|\b\d{1,3}(?:\.\d{1,3}){3}(?::\d+)?\b|\blocalhost:\d+\b)",
            r"|(?P<key>\b(?:Ctrl|Cmd|Alt|Shift|Option)[-+][A-Za-z0-9]+\b|⌘\S)",
            r"|(?P<var>\b[A-Z][A-Z0-9]*_[A-Z0-9_]+\b(?:=\S+)?|\b[a-z_]+=[^\s,)]+)",
            r"|(?P<placeholder><[A-Za-z][A-Za-z0-9_|-]*>)",
            r"|(?P<path>(?:^|\B)(?:~/|/)[A-Za-z0-9._@-]+(?:/[A-Za-z0-9._@-]*)*)",
            r"|(?P<flag>(?:^|\B)--?[A-Za-z][A-Za-z0-9-]*)",
        ))
        .expect("rich pattern")
    })
}

fn json_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(concat!(
            r#"(?P<key>"(?:[^"\\]|\\.)*")\s*:"#,
            r#"|(?P<string>"(?:[^"\\]|\\.)*")"#,
            r"|(?P<literal>-?\b\d+(?:\.\d+)?\b|\btrue\b|\bfalse\b|\bnull\b)",
            r"|(?P<punct>[{}\[\],])",
        ))
        .expect("json pattern")
    })
}

fn typed_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"<[^>]+>|\[[^\]]+\]|\b[A-Z][A-Z0-9_]+\b(?:=\S+)?").expect("typed pattern")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip(text: &str) -> String {
        Regex::new("\x1b\\[[0-9;]*m")
            .unwrap()
            .replace_all(text, "")
            .into_owned()
    }

    #[test]
    fn plain_style_returns_the_text_unchanged() {
        let plain = Style::new(false);
        assert_eq!(plain.title("DQLRS Web"), "DQLRS Web");
        assert_eq!(plain.row("MCP", 11, "off"), "MCP        off");
        let help = "Usage:\n  dqlrs web [options]      Serve\n";
        assert_eq!(plain.help(help), help);
        let json = "{\n  \"url\": \"http://x\"\n}";
        assert_eq!(plain.json(json), json);
        assert_eq!(
            plain.rich("Run `dqlrs web` at http://x"),
            "Run `dqlrs web` at http://x"
        );
    }

    #[test]
    fn color_never_changes_the_text() {
        let color = Style::new(true);
        let samples = [
            "Stop it with `kill 123`. Logs in /tmp/x/dynamodb-local.log.",
            "Ctrl-C, or `dqlrs notebook --shutdown` from another terminal",
            "AWS_REGION and DQL_BACKEND=memory, region=us-west-1, see http://127.0.0.1:8765/mcp.",
            "--load-tables  With web, load every table definition (with progress)",
            "-c, --command <command>  Run this command and exit",
        ];
        for sample in samples {
            assert_eq!(strip(&color.rich(sample)), sample);
            assert_eq!(strip(&color.help(sample)), sample);
        }
        let config = "Cursor (.cursor/mcp.json):\n{\n  \"mcpServers\": {\n    \"port\": 8000,\n    \"ok\": true\n  }\n}";
        assert_eq!(strip(&color.config(config)), config);
    }

    #[test]
    fn rich_colors_each_kind() {
        let color = Style::new(true);
        let text = color.rich(
            "Run `dqlrs web` at http://127.0.0.1:8765. Press Ctrl-C. Set AWS_REGION in ~/queries, see --mcp",
        );
        assert!(text.contains("\x1b[36m`dqlrs web`\x1b[0m"), "{text:?}");
        assert!(
            text.contains("\x1b[4;94mhttp://127.0.0.1:8765\x1b[0m."),
            "{text:?}"
        );
        assert!(text.contains("\x1b[1;7mCtrl-C\x1b[0m"), "{text:?}");
        assert!(text.contains("\x1b[35mAWS_REGION\x1b[0m"), "{text:?}");
        assert!(text.contains("\x1b[34m~/queries\x1b[0m"), "{text:?}");
        assert!(text.contains("\x1b[36m--mcp\x1b[0m"), "{text:?}");
        let sample = color.rich("From another terminal: dqlrs notebook --shutdown");
        assert!(
            sample.ends_with("\x1b[36mdqlrs notebook --shutdown\x1b[0m"),
            "{sample:?}"
        );
        let prose = color.rich("Committed \"Initialized dqlrs web examples\"");
        assert!(!prose.contains("\x1b[36mdqlrs"), "{prose:?}");
        let plan = color.rich("dqlrs web --init-examples will set up /x:");
        assert!(
            plan.starts_with("\x1b[36mdqlrs web --init-examples\x1b[0m will set up"),
            "{plan:?}"
        );
        let local = color.rich("running on localhost:8000");
        assert!(
            local.ends_with("\x1b[4;94mlocalhost:8000\x1b[0m"),
            "{local:?}"
        );
        let address = color.rich("listen on 127.0.0.1:8765)");
        assert!(
            address.contains("\x1b[4;94m127.0.0.1:8765\x1b[0m)"),
            "{address:?}"
        );
    }

    #[test]
    fn json_colors_keys_strings_and_literals() {
        let color = Style::new(true);
        let line = color.json(r#"  "port": 8000, "url": "http://x", "ok": true"#);
        assert!(
            line.contains("\x1b[34m\"port\"\x1b[0m\x1b[2m:\x1b[0m"),
            "{line:?}"
        );
        assert!(line.contains("\x1b[35m8000\x1b[0m"), "{line:?}");
        assert!(line.contains("\x1b[32m\"http://x\"\x1b[0m"), "{line:?}");
        assert!(line.contains("\x1b[35mtrue\x1b[0m"), "{line:?}");
    }

    #[test]
    fn help_colors_headings_typed_parts_and_placeholders() {
        let color = Style::new(true);
        let help = color.help("Options:\n  -c, --command <command>  Run this command and exit");
        let lines: Vec<&str> = help.lines().collect();
        assert_eq!(lines[0], "\x1b[1;33mOptions:\x1b[0m");
        assert!(
            lines[1].contains("\x1b[36m-c, --command \x1b[0m\x1b[3;35m<command>\x1b[0m"),
            "{:?}",
            lines[1]
        );
        let host = color.help("  -H, --host <host>        Host");
        assert!(host.contains("\x1b[36m-H, --host \x1b[0m"), "{host:?}");
        let port = color.help("  -p PORT              DynamoDB port");
        assert!(port.contains("\x1b[3;35mPORT\x1b[0m"), "{port:?}");
    }
}
