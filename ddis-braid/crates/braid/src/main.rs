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
    #[arg(long, global = true, hide_long_help = true)]
    budget: Option<u32>,

    /// Fraction of context window already consumed (0.0–1.0).
    /// Used to compute k*_eff for attention-quality-adjusted output.
    /// Example: --context-used 0.7 means 70% consumed, 30% remaining.
    #[arg(long, global = true, hide_long_help = true)]
    context_used: Option<f64>,

    /// Output format: json, agent, or human.
    ///
    /// Resolution priority (INV-OUTPUT-001):
    ///   1. --format flag (this)
    ///   2. BRAID_OUTPUT env var
    ///   3. TTY detection: interactive terminal → human
    ///   4. Default: agent (AI agents are the primary consumer)
    #[arg(long, global = true, hide_long_help = true)]
    format: Option<String>,

    /// Show orientation prompt for AI agents (complete workflow guide).
    #[arg(long)]
    orientation: bool,

    /// Suppress guidance footer (M(t), next action) without suppressing errors.
    /// Use this instead of `2>/dev/null` to keep error messages visible.
    #[arg(long, short = 'q', global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Option<commands::Command>,
}

/// Whether a command should bypass the budget gate.
///
/// Queries with explicit `--limit` or `--count` have user-controlled output bounds,
/// so the automatic budget truncation would defeat the purpose of pagination.
fn is_budget_exempt(cmd: &commands::Command) -> bool {
    matches!(
        cmd,
        commands::Command::Query { limit: Some(_), .. }
            | commands::Command::Query { count: true, .. }
    )
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
    // Extract metadata before cmd is consumed by run().
    let cmd_name = commands::command_name_for(&cmd);
    let skip_exit_warning = commands::is_harvest_command(&cmd);
    let budget_exempt = is_budget_exempt(&cmd);
    let exit_warn_path = commands::store_path(&cmd).map(|p| p.to_path_buf());
    let result = commands::run(cmd, &budget_ctx, mode, cli.quiet);
    match result {
        Ok(cmd_output) => {
            // INV-BUDGET-001 + INV-BUDGET-005: enforce per-command token ceiling
            // as the last gate before rendering. JSON mode is exempt.
            // Explicit pagination (--limit or --count) also bypasses the budget gate
            // because the user has explicitly bounded the output themselves.
            let cmd_output = if budget_exempt {
                cmd_output
            } else {
                commands::apply_budget_gate(cmd_output, mode, &budget_ctx, cmd_name)
            };
            print!("{}", cmd_output.render(mode));

            // RFL-2: Record projected action as datom for R(t) feedback loop.
            // If the command produced ACP output (_acp field), extract the action
            // and auto-transact it as an :action/* entity. This is the PREDICTION
            // half — the outcome is classified on the NEXT command (RFL-3).
            if let Some(acp) = cmd_output.json.get("_acp") {
                if let Some(ref path) = exit_warn_path {
                    if let Ok(lo) = layout::DiskLayout::open(path) {
                        if let Ok(store) = lo.load_store() {
                            if let Some(action) = acp.get("action") {
                                let cmd_str = action.get("command")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let impact = action.get("impact")
                                    .and_then(|v| v.as_f64())
                                    .unwrap_or(0.0);
                                let wall_ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as i64;

                                use braid_kernel::datom::*;
                                let agent = AgentId::from_name("braid:rfl");
                                let tx = commands::write::next_tx_id(&store, agent);
                                let ident = format!(
                                    ":action/{}",
                                    &blake3::hash(format!("{}-{}", cmd_str, wall_ms).as_bytes()).to_hex()[..16]
                                );
                                let entity = EntityId::from_ident(&ident);
                                let datoms = vec![
                                    Datom::new(entity, Attribute::from_keyword(":db/ident"), Value::Keyword(ident), tx, Op::Assert),
                                    Datom::new(entity, Attribute::from_keyword(":action/recommended-command"), Value::String(cmd_str.to_string()), tx, Op::Assert),
                                    Datom::new(entity, Attribute::from_keyword(":action/recommended-impact"), Value::Double(ordered_float::OrderedFloat(impact)), tx, Op::Assert),
                                    Datom::new(entity, Attribute::from_keyword(":action/timestamp"), Value::Long(wall_ms), tx, Op::Assert),
                                ];
                                let tx_file = braid_kernel::layout::TxFile {
                                    tx_id: tx,
                                    agent,
                                    provenance: ProvenanceType::Derived,
                                    rationale: "RFL-2: action prediction recorded".to_string(),
                                    causal_predecessors: vec![],
                                    datoms,
                                };
                                let _ = lo.write_tx(&tx_file);
                            }
                        }
                    }
                }
            }

            // NEG-HARVEST-001: warn on exit if unharvested work is at risk.
            // Skip for harvest commands (they just harvested) and JSON mode
            // (structured output should not have side-channel stderr noise).
            if !skip_exit_warning && !cli.quiet && mode != output::OutputMode::Json {
                if let Some(ref path) = exit_warn_path {
                    if let Ok(lo) = layout::DiskLayout::open(path) {
                        if let Ok(store) = lo.load_store() {
                            if let Some(warning) =
                                braid_kernel::guidance::should_warn_on_exit(&store, None)
                            {
                                eprintln!("{warning}");
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprint!("{}", e.render(mode));
            std::process::exit(1);
        }
    }
}
