fn main() {
    if let Err(err) = dql_cli::run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
