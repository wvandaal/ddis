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
    /// Token budget for output. Overrides all other budget sources.
    /// Controls guidance footer compression and projection level selection.
    /// Budget source precedence: --budget > --context-used > default (10000).
    #[arg(long, global = true)]
    budget: Option<u32>,

    /// Fraction of context window already consumed (0.0–1.0).
    /// Used to compute k*_eff for attention-quality-adjusted output.
    /// Example: --context-used 0.7 means 70% consumed, 30% remaining.
    #[arg(long, global = true)]
    context_used: Option<f64>,

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

    let budget_ctx = commands::BudgetCtx::from_flags(cli.budget, cli.context_used);
    let result = commands::run(cmd, &budget_ctx);
    match result {
        Ok(output) => {
            print!("{output}");
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
