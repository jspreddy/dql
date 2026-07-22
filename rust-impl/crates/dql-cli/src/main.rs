fn main() -> color_eyre::Result<()> {
    dql_cli::error::initialize_panic_handler()?;
    dql_cli::run()
}
