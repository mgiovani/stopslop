use clap::Parser;
use stopslop::cli::{self, Cli};

fn main() {
    let code = cli::run(Cli::parse()).unwrap_or_else(|e| {
        eprintln!("stopslop: {e:#}");
        2
    });
    std::process::exit(code);
}
