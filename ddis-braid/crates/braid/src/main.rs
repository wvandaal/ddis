use clap::Parser;

pub mod bootstrap;
mod commands;
pub mod daemon;
mod error;
pub mod git;
pub mod inject;
pub mod layout;
pub mod live_store;
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

    // Resolve store path (needed by both daemon routing and direct mode).
    let resolved_path =
        commands::store_path(&cmd).map(|p| commands::resolve_store_path(p.to_path_buf()));

    // INV-DAEMON-007: Try daemon routing FIRST, BEFORE opening LiveStore.
    // If the daemon is running, the CLI sends the request over the Unix socket
    // and the daemon's warm in-memory store handles it — zero deserialization.
    // Previously this was AFTER LiveStore::open(), defeating the purpose:
    // the CLI paid the full ~3s store load before even checking the daemon.
    // DW2: marshal_command maps all 11 routable commands to MCP tool names + JSON args.
    if cmd_name != "init" && cmd_name != "daemon" {
        if let Some(ref store_path) = resolved_path {
            if let Some(text) = daemon::try_route_through_daemon(store_path, &cmd) {
                println!("{text}");
                return;
            }
        }
    }

    // Direct mode: daemon not running or command not routable.
    // L1-SINGLE: Open LiveStore ONCE for entire process.
    let mut live = if cmd_name != "init" {
        resolved_path
            .as_ref()
            .and_then(|p| live_store::LiveStore::open(p).ok())
    } else {
        None
    };

    // Session auto-detect (only for commands using pre_opened store).
    let uses_pre_opened = matches!(cmd_name, "status");
    if cmd_name != "init" && cmd_name != "session" && uses_pre_opened {
        if let Some(ref mut live) = live {
            if braid_kernel::guidance::detect_session_start(live.store()) {
                let resolved = commands::resolve_agent_identity("braid:user");
                let agent = braid_kernel::datom::AgentId::from_name(&resolved);
                let tx_id = commands::write::next_tx_id(live.store(), agent);
                let datoms =
                    braid_kernel::guidance::create_session_start_datoms(live.store(), agent, tx_id);
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
        }
    }

    // L1-SINGLE: Pass the pre-opened LiveStore to command dispatch.
    // The command reuses it (zero deserialization) instead of opening its own.
    let result = commands::run(cmd, &budget_ctx, mode, cli.quiet, live.as_mut());
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

            // L1-SINGLE: Post-command hooks reuse the same LiveStore.
            // Previously opened a THIRD LiveStore here (~2-3s deserialization).
            // Now the store is already warm in memory with all command writes visible.
            let needs_rfl2 = cmd_output.json.get("_acp").is_some();
            let needs_exit_warning = !skip_exit_warning
                && !cli.quiet
                && mode != output::OutputMode::Json
                && mode != output::OutputMode::Tsv;
            // AR-2: Only knowledge-producing commands produce reconciliation traces.
            // Read commands (status, query, task list) do NOT write traces.
            let is_knowledge_producing =
                matches!(cmd_name, "observe" | "transact" | "write" | "task" | "spec");

            // L1-SINGLE: For commands that need post-command hooks, refresh the store
            // to pick up any txns written by the command. For read-only commands, skip
            // the refresh entirely — the store is already fresh from the single open.
            let needs_post_hooks = needs_rfl2 || needs_exit_warning || is_knowledge_producing;
            if needs_post_hooks {
                if live.is_none() {
                    live = resolved_path
                        .as_ref()
                        .and_then(|path| live_store::LiveStore::open(path).ok());
                } else if let Some(ref mut l) = live {
                    let _ = l.refresh_if_needed();
                }
            }

            // RFL-2: Record projected action as datom for R(t) feedback loop.
            if let (Some(acp), Some(ref mut live)) = (cmd_output.json.get("_acp"), live.as_mut()) {
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
            // knowledge-producing commands.
            if is_knowledge_producing {
                if let Some(ref mut live) = live {
                    let json_str = serde_json::to_string(&cmd_output.json).unwrap_or_default();
                    let spec_refs = braid_kernel::task::parse_spec_refs(&json_str);

                    if !spec_refs.is_empty() {
                        use braid_kernel::datom::*;

                        let neighbors =
                            braid_kernel::guidance::spec_graph_neighbors(live.store(), &spec_refs);
                        let namespace = spec_refs
                            .first()
                            .map(|r| braid_kernel::guidance::extract_spec_namespace(r).to_string())
                            .unwrap_or_default();

                        let agent = AgentId::from_name("braid:recon");
                        let tx = commands::write::next_tx_id(live.store(), agent);
                        let wall_secs = tx.wall_time();
                        let trace_ident = format!(
                            ":recon/trace-{}",
                            &blake3::hash(format!("{}-{}", cmd_name, wall_secs).as_bytes())
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

                        for spec_ref in &spec_refs {
                            datoms.push(Datom::new(
                                trace_entity,
                                Attribute::from_keyword(":recon/trace-refs"),
                                Value::String(spec_ref.clone()),
                                tx,
                                Op::Assert,
                            ));
                        }

                        for (neighbor_entity, _score) in &neighbors {
                            let ident_attr = Attribute::from_keyword(":db/ident");
                            let neighbor_ident = live
                                .store()
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
                if let Some(ref live) = live {
                    if let Some(warning) =
                        braid_kernel::guidance::should_warn_on_exit(live.store(), None)
                    {
                        eprintln!("{warning}");
                    }

                    // D2.1: Show active divergence types alongside harvest warning
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
                        let count = divergences.len();
                        if count <= 5 {
                            let types: Vec<String> = divergences
                                .iter()
                                .map(|(dt, _)| format!("{:?}", dt))
                                .collect();
                            eprintln!(
                                "braid divergence \u{2014} {} active: {}",
                                count,
                                types.join(", ")
                            );
                        } else {
                            eprintln!(
                                "braid divergence \u{2014} {} active divergence types (use --verbose for details)",
                                count,
                            );
                        }
                    }
                }
            }
            // L1-SINGLE: LiveStore drops here — single flush of store.bin for entire process.
        }
        Err(e) => {
            eprint!("{}", e.render(mode));
            std::process::exit(1);
        }
    }
}
