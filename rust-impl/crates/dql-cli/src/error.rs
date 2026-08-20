use color_eyre::config::HookBuilder;
use color_eyre::eyre;

/// Install color-eyre / better-panic / human-panic hooks (Ratatui recipes).
///
/// Call once at process start, before any `ratatui::init*`. Ratatui will wrap
/// this panic hook so the terminal is restored before the report is printed.
pub fn initialize_panic_handler() -> eyre::Result<()> {
    let (panic_hook, eyre_hook) = HookBuilder::default()
        .panic_section(format!(
            "This is a bug. Consider reporting it at {}",
            env!("CARGO_PKG_REPOSITORY")
        ))
        .display_location_section(true)
        .display_env_section(true)
        .into_hooks();
    eyre_hook.install()?;

    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = ratatui::try_restore();

        let msg = format!("{}", panic_hook.panic_report(panic_info));

        #[cfg(not(debug_assertions))]
        {
            eprintln!("{msg}");
            use human_panic::{handle_dump, print_msg, Metadata};
            let support = format!(
                "You can open a support request at {}",
                env!("CARGO_PKG_REPOSITORY")
            );
            let meta = Metadata::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
                .authors(env!("CARGO_PKG_AUTHORS").replace(':', ", "))
                .support(support);
            let file_path = handle_dump(&meta, panic_info);
            let _ = print_msg(file_path.as_deref(), &meta);
        }

        #[cfg(debug_assertions)]
        {
            let _ = msg;
            better_panic::Settings::auto()
                .most_recent_first(false)
                .lineno_suffix(true)
                .verbosity(better_panic::Verbosity::Full)
                .create_panic_handler()(panic_info);
        }

        std::process::exit(1);
    }));

    Ok(())
}

/// Format a color-eyre report as plain lines for the REPL error panel.
pub fn report_lines(report: &eyre::Report) -> Vec<String> {
    let text = strip_ansi_escapes::strip_str(format!("{report:?}"));
    text.lines()
        .map(|line| line.trim_end().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use color_eyre::eyre::eyre;

    #[test]
    fn report_lines_include_context_and_cause() {
        let report = eyre!("expected TABLE").wrap_err("query failed");
        let rendered = report_lines(&report).join("\n");
        assert!(
            rendered.contains("query failed"),
            "missing context: {rendered}"
        );
        assert!(
            rendered.contains("expected TABLE"),
            "missing cause: {rendered}"
        );
    }
}
