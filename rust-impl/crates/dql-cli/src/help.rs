use dql_engine::{help_page, HelpBlock, HelpPage};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub use dql_engine::topic_help;

/// Back-compat alias used by older call sites / tests.
pub fn statement_help(topic: &str) -> Option<&'static str> {
    topic_help(topic)
}

/// Render top-level help or a specific topic as styled ratatui lines.
pub fn render_lines(args: &[String], width: usize) -> Result<Vec<Line<'static>>, String> {
    let _ = width;
    let page = help_page(args.first().map(String::as_str))?;
    Ok(page_lines(&page))
}

pub fn page_lines(page: &HelpPage) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled(
            page.title.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    for block in &page.blocks {
        match block {
            HelpBlock::Heading(text) => {
                lines.push(Line::from(Span::styled(
                    text.clone(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )));
            }
            HelpBlock::Prose(text) => {
                for line in text.lines() {
                    lines.push(Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(Color::Gray),
                    )));
                }
                lines.push(Line::from(""));
            }
            HelpBlock::Code(text) => {
                for line in text.lines() {
                    lines.push(crate::repl::highlight::highlight_line(line));
                }
                lines.push(Line::from(""));
            }
            HelpBlock::Entry { name, summary } => {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        name.clone(),
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("  ", Style::default().fg(Color::DarkGray)),
                    Span::raw(summary.clone()),
                ]));
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_docs() {
        for topic in [
            "alter", "analyze", "create", "delete", "describe", "drop", "dump", "explain",
            "insert", "load", "scan", "select", "show", "update", "options", "opt", "history",
            "clear", "exit", "ls", "help",
        ] {
            assert!(topic_help(topic).is_some(), "missing help for {topic}");
        }
        assert!(topic_help("opt").unwrap().contains("rich"));
    }

    #[test]
    fn overview_lists_queries_and_meta() {
        let text: String = render_lines(&[], 80)
            .unwrap()
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Available commands"));
        assert!(text.contains("Queries"));
        assert!(text.contains("DQL statements for reading and writing DynamoDB data"));
        assert!(text.contains("select"));
        assert!(text.contains("Query items from a table or index"));
        assert!(text.contains("history"));
        assert!(text.contains("List, search, dedupe, remove, or edit history"));
        assert!(text.contains("Session"));
        assert!(text.contains("HELP <topic>;"));
    }

    fn example_lines(text: &str) -> Vec<String> {
        let mut lines = Vec::new();
        let mut in_examples = false;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed == "Examples" {
                in_examples = true;
                continue;
            }
            if !in_examples || trimmed.is_empty() || trimmed.starts_with('-') {
                continue;
            }
            lines.push(trimmed.to_string());
        }
        lines
    }

    fn serve_error(envelope: &crate::serve::protocol::ServeEnvelope) -> String {
        envelope
            .error
            .as_ref()
            .map(|err| err.message.clone())
            .unwrap_or_else(|| "not ok".to_string())
    }

    fn run_like_web(example: &str) -> Result<(), String> {
        let mut session = crate::session::Session::new_memory_headless("us-west-1");
        let parts = crate::web::shape::split_statements(example);
        if parts.is_empty() {
            return Err("web split produced no statements".to_string());
        }
        for part in parts {
            let envelope = session.execute_for_serve(&part);
            if !envelope.ok {
                return Err(format!("{part}: {}", serve_error(&envelope)));
            }
        }
        Ok(())
    }

    fn run_like_notebook(example: &str) -> Result<(), String> {
        let mut session = crate::session::Session::new_memory_headless("us-west-1");
        let envelope = session.execute_for_serve(example);
        if envelope.ok {
            Ok(())
        } else {
            Err(serve_error(&envelope))
        }
    }

    fn run_like_cli(example: &str) -> Result<(), String> {
        let mut session = crate::session::Session::new_memory_headless("us-west-1");
        session
            .run_command(example, true)
            .map_err(|err| err.to_string())
    }

    #[test]
    fn help_examples_run_from_web_cli_notebook_and_command() {
        let root = tempfile::tempdir().unwrap();
        let previous = std::env::current_dir().unwrap();
        struct Restore(std::path::PathBuf);
        impl Drop for Restore {
            fn drop(&mut self) {
                let _ = std::env::set_current_dir(&self.0);
            }
        }
        let _restore = Restore(previous);
        std::env::set_current_dir(root.path()).unwrap();

        let mut failures = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for topic in [
            "alter", "analyze", "create", "delete", "drop", "dump", "explain", "insert", "load",
            "select", "update", "ls", "describe", "show",
        ] {
            let text = topic_help(topic).unwrap();
            for example in example_lines(text) {
                if !seen.insert(example.clone()) {
                    continue;
                }
                if let Err(err) = run_like_web(&example) {
                    failures.push(format!("web {example}\n  {err}"));
                }
                if let Err(err) = run_like_cli(&example) {
                    failures.push(format!("cli -c {example}\n  {err}"));
                }
                if let Err(err) = run_like_notebook(&example) {
                    failures.push(format!("notebook {example}\n  {err}"));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    }

    #[test]
    fn render_lines_topic_and_overview() {
        let overview = render_lines(&[], 80).unwrap();
        assert!(!overview.is_empty());
        let select = render_lines(&[String::from("select")], 80).unwrap();
        assert!(select.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.content.contains("HELP select"))
        }));
        assert!(select.iter().any(|line| {
            line.spans.iter().any(|span| {
                span.content.as_ref() == "SELECT" && span.style.fg == Some(Color::Magenta)
            })
        }));
        assert!(render_lines(&[String::from("nope")], 80).is_err());
    }
}
