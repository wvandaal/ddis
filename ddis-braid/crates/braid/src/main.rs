use clap::Parser;

pub mod bootstrap;
mod commands;
mod error;
pub mod git;
pub mod inject;
pub mod layout;
pub mod mcp;
pub mod output;

/// Braid — append-only datom store for human/AI coherence verification.
///
/// Quick start:
///   braid init                              # create store, detect env, inject seed
///   braid status                            # F(S)=0.77, M(t)=0.82, next action
///   braid observe "insight" -c 0.8          # capture knowledge as datom
///   braid harvest --commit                  # end-of-session: crystallize
///   braid seed --inject AGENTS.md           # refresh context for next session
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

    /// Output format: json, agent, or human.
    ///
    /// Resolution priority (INV-OUTPUT-001):
    ///   1. --format flag (this)
    ///   2. BRAID_OUTPUT env var
    ///   3. TTY detection: interactive terminal → human
    ///   4. Default: agent (AI agents are the primary consumer)
    #[arg(long, global = true)]
    format: Option<String>,

    /// Show orientation prompt for AI agents (complete workflow guide).
    #[arg(long)]
    orientation: bool,

    #[command(subcommand)]
    command: Option<commands::Command>,
}

fn main() {
    let cli = Cli::parse();

    // Resolve output mode once (INV-OUTPUT-001: deterministic resolution)
    let mode = output::resolve_mode(cli.format.as_deref());

    // --orientation: output the complete agent onboarding prompt
    if cli.orientation {
        let orientation = commands::orientation::build_orientation(mode);
        print!("{orientation}");
        return;
    }

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
    let result = commands::run(cmd, &budget_ctx, mode);
    match result {
        Ok(cmd_output) => {
            print!("{}", cmd_output.render(mode));
        }
        Err(e) => {
            eprint!("{}", e.render(mode));
            std::process::exit(1);
        }
    }
}
