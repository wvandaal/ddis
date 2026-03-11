use clap::Parser;

pub mod bootstrap;
mod commands;
mod error;
pub mod git;
pub mod inject;
pub mod layout;
pub mod mcp;
pub mod output;

/// Braid — append-only datom store with CRDT merge, Datalog queries, and coherence verification.
///
/// Workflow: init → observe → query → status → harvest → seed
///
/// Quick start:
///   braid init                                          # create store
///   braid observe "merge is a bottleneck" -c 0.8       # capture knowledge
///   braid query '[:find ?e ?v :where [?e :db/doc ?v]]' # query
///   braid status                                        # dashboard + next action
#[derive(Parser)]
#[command(name = "braid", version, about, long_about)]
struct Cli {
    #[command(subcommand)]
    command: Option<commands::Command>,
}

fn main() {
    let cli = Cli::parse();

    // Default: bare `braid` shows terse status dashboard
    let cmd = cli.command.unwrap_or(commands::Command::Status {
        path: ".braid".into(),
        json: false,
        verbose: false,
        deep: false,
        spectral: false,
        full: false,
        verify: false,
        agent: "braid:user".into(),
        commit: false,
    });

    let result = commands::run(cmd);
    match result {
        Ok(output) => {
            print!("{output}");
        }
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!("hint: {}", e.recovery_hint());
            std::process::exit(1);
        }
    }
}
