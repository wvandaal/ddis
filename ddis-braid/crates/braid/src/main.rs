use clap::Parser;

pub mod bootstrap;
mod commands;
mod error;
pub mod git;
pub mod inject;
pub mod layout;
pub mod live_store;
pub mod mcp;
pub mod daemon;
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

    // LIVESTORE-5a: ST-1 session auto-detect using LiveStore write-through.
    // Previously created DiskLayout + load_store + write_tx (which invalidated cache).
    // Now uses LiveStore: write goes through write_tx (no invalidation), flush on drop.
    if cmd_name != "init" && cmd_name != "session" {
        if let Some(ref store_path) = exit_warn_path {
            if let Ok(mut live) = live_store::LiveStore::open(store_path) {
                if braid_kernel::guidance::detect_session_start(live.store()) {
                    let agent = braid_kernel::datom::AgentId::from_name("braid:session");
                    let tx_id = commands::write::next_tx_id(live.store(), agent);
                    let datoms = braid_kernel::guidance::create_session_start_datoms(
                        live.store(),
                        agent,
                        tx_id,
                    );
                    let tx_file = braid_kernel::layout::TxFile {
                        tx_id,
                        agent,
                        provenance: braid_kernel::datom::ProvenanceType::Derived,
                        rationale: "ST-1: auto-detected session start".to_string(),
                        causal_predecessors: vec![],
                        datoms,
                    };
                    let _ = live.write_tx(&tx_file);
                }
                // LiveStore drops here — flush writes store.bin if dirty.
            }
        }
    }

    // D4-8: Try daemon routing before direct execution (INV-DAEMON-007).
    // If the daemon is running, route supported commands through the socket.
    // Falls back silently to direct mode on any failure.
    if let Some(ref store_path) = exit_warn_path {
        if let Some(text) = daemon::try_route_through_daemon(
            store_path,
            cmd_name,
            &serde_json::json!({}), // Minimal args for daemon-routed commands
        ) {
            println!("{text}");
            return;
        }
    }

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

            // T2-1: Single post-command store load for RFL-2, AR-2 trace, and exit warning.
            // All three paths need (DiskLayout, Store) for the same braid root.
            // Load once, reuse for all purposes.
            let needs_rfl2 = cmd_output.json.get("_acp").is_some();
            let needs_exit_warning = !skip_exit_warning
                && !cli.quiet
                && mode != output::OutputMode::Json
                && mode != output::OutputMode::Tsv;
            // AR-2: Only knowledge-producing commands produce reconciliation traces.
            // Read commands (status, query, task list) do NOT write traces.
            let is_knowledge_producing = matches!(
                cmd_name,
                "observe" | "transact" | "write" | "task" | "spec"
            );

            // LIVESTORE-5a: Post-command store access via LiveStore.
            // Opens fresh (picks up any txns written by the command).
            let mut post_cmd_live =
                if (needs_rfl2 || needs_exit_warning || is_knowledge_producing)
                    && exit_warn_path.is_some()
                {
                exit_warn_path
                    .as_ref()
                    .and_then(|path| live_store::LiveStore::open(path).ok())
            } else {
                None
            };

            // RFL-2: Record projected action as datom for R(t) feedback loop.
            // If the command produced ACP output (_acp field), extract the action
            // and auto-transact it as an :action/* entity. This is the PREDICTION
            // half — the outcome is classified on the NEXT command (RFL-3).
            if let (Some(acp), Some(ref mut live)) =
                (cmd_output.json.get("_acp"), post_cmd_live.as_mut())
            {
                if let Some(action) = acp.get("action") {
                    let cmd_str = action.get("command").and_then(|v| v.as_str()).unwrap_or("");
                    let impact = action.get("impact").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let wall_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64;

                    use braid_kernel::datom::*;
                    let agent = AgentId::from_name("braid:rfl");
                    let tx = commands::write::next_tx_id(live.store(), agent);
                    let ident = format!(
                        ":action/{}",
                        &blake3::hash(format!("{}-{}", cmd_str, wall_ms).as_bytes()).to_hex()[..16]
                    );
                    let entity = EntityId::from_ident(&ident);
                    let datoms = vec![
                        Datom::new(
                            entity,
                            Attribute::from_keyword(":db/ident"),
                            Value::Keyword(ident),
                            tx,
                            Op::Assert,
                        ),
                        Datom::new(
                            entity,
                            Attribute::from_keyword(":action/recommended-command"),
                            Value::String(cmd_str.to_string()),
                            tx,
                            Op::Assert,
                        ),
                        Datom::new(
                            entity,
                            Attribute::from_keyword(":action/recommended-impact"),
                            Value::Double(ordered_float::OrderedFloat(impact)),
                            tx,
                            Op::Assert,
                        ),
                        Datom::new(
                            entity,
                            Attribute::from_keyword(":action/timestamp"),
                            Value::Long(wall_ms),
                            tx,
                            Op::Assert,
                        ),
                    ];
                    let tx_file = braid_kernel::layout::TxFile {
                        tx_id: tx,
                        agent,
                        provenance: ProvenanceType::Derived,
                        rationale: "RFL-2: action prediction recorded".to_string(),
                        causal_predecessors: vec![],
                        datoms,
                    };
                    let _ = live.write_tx(&tx_file);
                }
            }

            // AR-2: Reconciliation trace — write :recon/trace-* datoms for
            // knowledge-producing commands. These traces feed the concentration
            // detector (AR-4) which detects sustained work in a spec neighborhood.
            // Traces are OPTIONAL: if anything fails, silently skip.
            if is_knowledge_producing {
                if let Some(ref mut live) = post_cmd_live {
                    // Extract spec refs from the full JSON output
                    let json_str = serde_json::to_string(&cmd_output.json).unwrap_or_default();
                    let spec_refs = braid_kernel::task::parse_spec_refs(&json_str);

                    if !spec_refs.is_empty() {
                        use braid_kernel::datom::*;

                        // Graph traversal: find related entities via spec dependency graph
                        let neighbors =
                            braid_kernel::guidance::spec_graph_neighbors(live.store(), &spec_refs);
                        let namespace = spec_refs
                            .first()
                            .map(|r| {
                                braid_kernel::guidance::extract_spec_namespace(r).to_string()
                            })
                            .unwrap_or_default();

                        let agent = AgentId::from_name("braid:recon");
                        let tx = commands::write::next_tx_id(live.store(), agent);
                        let wall_secs = tx.wall_time();
                        let trace_ident = format!(
                            ":recon/trace-{}",
                            &blake3::hash(
                                format!("{}-{}", cmd_name, wall_secs).as_bytes()
                            )
                            .to_hex()[..16]
                        );
                        let trace_entity = EntityId::from_ident(&trace_ident);

                        let mut datoms = vec![
                            Datom::new(
                                trace_entity,
                                Attribute::from_keyword(":db/ident"),
                                Value::Keyword(trace_ident),
                                tx,
                                Op::Assert,
                            ),
                            Datom::new(
                                trace_entity,
                                Attribute::from_keyword(":recon/trace-command"),
                                Value::String(cmd_name.to_string()),
                                tx,
                                Op::Assert,
                            ),
                        ];

                        if !namespace.is_empty() {
                            datoms.push(Datom::new(
                                trace_entity,
                                Attribute::from_keyword(":recon/trace-neighborhood"),
                                Value::String(namespace),
                                tx,
                                Op::Assert,
                            ));
                        }

                        // Cardinality::Many — one datom per spec ref
                        for spec_ref in &spec_refs {
                            datoms.push(Datom::new(
                                trace_entity,
                                Attribute::from_keyword(":recon/trace-refs"),
                                Value::String(spec_ref.clone()),
                                tx,
                                Op::Assert,
                            ));
                        }

                        // Cardinality::Many — one datom per neighbor ident
                        for (neighbor_entity, _score) in &neighbors {
                            // Resolve ident from store if available
                            let ident_attr = Attribute::from_keyword(":db/ident");
                            let neighbor_ident = live.store()
                                .entity_datoms(*neighbor_entity)
                                .iter()
                                .find(|d| d.attribute == ident_attr && d.op == Op::Assert)
                                .and_then(|d| match &d.value {
                                    Value::Keyword(k) => Some(k.clone()),
                                    _ => None,
                                });
                            if let Some(ident) = neighbor_ident {
                                datoms.push(Datom::new(
                                    trace_entity,
                                    Attribute::from_keyword(":recon/trace-neighbors"),
                                    Value::String(ident),
                                    tx,
                                    Op::Assert,
                                ));
                            }
                        }

                        let tx_file = braid_kernel::layout::TxFile {
                            tx_id: tx,
                            agent,
                            provenance: ProvenanceType::Derived,
                            rationale: "AR-2: reconciliation trace".to_string(),
                            causal_predecessors: vec![],
                            datoms,
                        };
                        let _ = live.write_tx(&tx_file);
                    }
                }
            }

            // NEG-HARVEST-001: warn on exit if unharvested work is at risk.
            if needs_exit_warning {
                if let Some(ref live) = post_cmd_live {
                    if let Some(warning) =
                        braid_kernel::guidance::should_warn_on_exit(live.store(), None)
                    {
                        eprintln!("{warning}");
                    }

                    // D2.1: Show active divergence types alongside harvest warning
                    // (INV-SIGNAL-001). Uses lightweight detection — no spectral analysis.
                    let detector = braid_kernel::signal::ConfusionDetector::default();
                    let source = braid_kernel::datom::EntityId::from_ident(":system/exit-check");
                    let budget = live.store().len() as u64;
                    let divergences = braid_kernel::signal::detect_all_divergence(
                        live.store(),
                        &detector,
                        source,
                        budget,
                    );
                    if !divergences.is_empty() {
                        let types: Vec<String> = divergences
                            .iter()
                            .map(|(dt, _)| format!("{:?}", dt))
                            .collect();
                        eprintln!(
                            "braid divergence \u{2014} {} active: {}",
                            divergences.len(),
                            types.join(", ")
                        );
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
