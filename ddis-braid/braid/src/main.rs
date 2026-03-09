use clap::Parser;

mod commands;
mod error;
pub mod output;

/// Braid — DDIS datom store and coherence verification engine.
#[derive(Parser)]
#[command(name = "braid", version, about)]
struct Cli {
    #[command(subcommand)]
    command: commands::Command,
}

fn main() {
    let cli = Cli::parse();
    let result = commands::run(cli.command);
    match result {
        Ok(output) => {
            print!("{output}");
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
