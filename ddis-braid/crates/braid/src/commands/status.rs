//! `braid status` — The agent's complete dashboard.
//!
//! Absorbs guidance, bilateral, and analyze into a single orientation command.
//!
//! Modes:
//! - **Terse** (default, <150 tokens): store + coherence + M(t) + harvest + next action
//! - **Verbose** (`--verbose`): full methodology + all actions
//! - **Deep** (`--deep`): bilateral F(S) + graph analytics + convergence
//! - **Verify** (`--verify`): on-disk integrity check
//!
//! Traces to: INV-GUIDANCE-008 (M(t)), INV-GUIDANCE-010 (R(t)),
//!            INV-BILATERAL-002 (coherence), ADR-GUIDANCE-008 (progressive enrichment)

use std::path::Path;

use braid_kernel::bilateral::{
    cycle_to_datoms, format_terse as bilateral_format_terse,
    format_verbose as bilateral_format_verbose, load_trajectory, run_cycle,
};
use braid_kernel::datom::{AgentId, Attribute, Op, ProvenanceType, Value};
use braid_kernel::guidance::{
    adjust_gaps, compute_action_from_routing, compute_methodology_score,
    compute_routing_with_calibration, count_txns_since_last_harvest, derive_actions,
    derive_actions_with_precomputed, detect_activity_mode, format_actions, methodology_gaps,
    telemetry_from_store, CalibrationReport, TaskRouting, Trend,
};
use braid_kernel::layout::TxFile;
use braid_kernel::trilateral::{check_coherence_fast, CoherenceReport};

use crate::error::BraidError;
use crate::layout::DiskLayout;
use crate::output::{AgentOutput, CommandOutput};

use super::trace::{check_staleness, TraceStaleStatus};

/// Compute trace staleness for the project.
///
/// `path` is the `.braid` directory. The source root is `path/../crates/`.
fn trace_staleness(store: &braid_kernel::Store, path: &Path) -> TraceStaleStatus {
    let source_root = path.parent().unwrap_or(path).join("crates");
    check_staleness(store, &source_root)
}

/// PERF-2: Pre-computed status values.
///
/// All expensive computations (fitness, coherence, telemetry, session deltas, task counts,
/// gaps) are done once here and then passed to `build_json()` and `build_status_projection()`
/// as immutable data. This eliminates the 2x redundant computation that caused `braid status`
/// to take 8.5s instead of ~1s.
pub(crate) struct StatusSnapshot {
    fitness: braid_kernel::bilateral::FitnessScore,
    coherence: braid_kernel::trilateral::CoherenceReport,
    telemetry: braid_kernel::guidance::SessionTelemetry,
    methodology_score: braid_kernel::guidance::MethodologyScore,
    session_start_fitness: Option<f64>,
    session_start_datom_count: usize,
    task_counts: (usize, usize, usize),
    ready_set: Vec<braid_kernel::task::TaskSummary>,
    trace_status: TraceStaleStatus,
    #[allow(dead_code)]
    gaps: braid_kernel::guidance::MethodologyGaps,
    #[allow(dead_code)]
    activity_mode: braid_kernel::guidance::ActivityMode,
    adjusted_gaps: braid_kernel::guidance::AdjustedGaps,
}

impl StatusSnapshot {
    /// Compute with optional DiskLayout for cache acceleration.
    /// When layout is provided, fitness and coherence are loaded from
    /// `.cache/fitness.json` and `.cache/coherence.json` if the cache
    /// is fresh (same txn_fingerprint as store.bin). This turns 29s of
    /// CPU computation into a ~1ms JSON read.
    pub(crate) fn compute_with_layout(
        store: &braid_kernel::Store,
        path: &Path,
        _layout: Option<&crate::layout::DiskLayout>,
    ) -> Self {
        // CE-4: Use materialized views for O(1) fitness (was O(n) compute_fitness)
        let fitness = store.fitness();
        let coherence = check_coherence_fast(store);

        let telemetry = telemetry_from_store(store);
        let methodology_score = compute_methodology_score(&telemetry);
        let session_start_fitness = query_session_start_fitness(store);
        let session_start_datom_count = query_session_start_datom_count(store);
        let task_counts = braid_kernel::task_counts(store);
        let ready_set = braid_kernel::compute_ready_set(store);
        let trace_status = trace_staleness(store, path);
        let gaps = methodology_gaps(store);
        let activity_mode = detect_activity_mode(&telemetry);
        let adjusted_gaps = adjust_gaps(gaps.clone(), activity_mode);
        StatusSnapshot {
            fitness,
            coherence,
            telemetry,
            methodology_score,
            session_start_fitness,
            session_start_datom_count,
            task_counts,
            ready_set,
            trace_status,
            gaps,
            activity_mode,
            adjusted_gaps,
        }
    }
}

/// Format trace staleness for human-readable output (one line, newline-terminated).
fn format_trace_line(status: &TraceStaleStatus) -> String {
    match status {
        TraceStaleStatus::Fresh { total } => {
            format!("trace: fresh ({} files scanned)\n", total)
        }
        TraceStaleStatus::Stale { stale, .. } => {
            format!(
                "trace: stale ({} files modified since last scan) \u{2014} run: braid trace --commit\n",
                stale
            )
        }
    }
}

/// Convert trace staleness to a JSON value.
fn trace_staleness_json(status: &TraceStaleStatus) -> serde_json::Value {
    match status {
        TraceStaleStatus::Fresh { .. } => serde_json::json!("fresh"),
        TraceStaleStatus::Stale { stale, .. } => serde_json::json!({ "stale": stale }),
    }
}

/// Run status and return structured CommandOutput (INV-INTERFACE-001: three output modes).
#[allow(clippy::too_many_arguments)]
pub fn run(
    path: &Path,
    agent_name: &str,
    json: bool,
    verbose: bool,
    deep: bool,
    spectral: bool,
    full: bool,
    verify: bool,
    commit: bool,
) -> Result<CommandOutput, BraidError> {
    // Verify mode: check integrity with structured output
    if verify {
        let layout = DiskLayout::open(path)?;
        let report = layout.verify_integrity()?;
        let is_clean = report.is_clean();
        let human = if is_clean {
            format!("integrity: OK ({} files verified)\n", report.verified)
        } else {
            format!(
                "integrity: FAILED ({} corrupted, {} orphaned out of {})\n",
                report.corrupted.len(),
                report.orphaned.len(),
                report.total_files,
            )
        };
        let json = serde_json::json!({
            "integrity": if is_clean { "ok" } else { "failed" },
            "verified": report.verified,
            "corrupted": report.corrupted.len(),
            "orphaned": report.orphaned.len(),
            "total_files": report.total_files,
        });
        let agent = AgentOutput {
            context: format!(
                "integrity: {} ({} files)",
                if is_clean { "OK" } else { "FAILED" },
                report.total_files,
            ),
            content: if is_clean {
                format!("{} files verified, 0 errors", report.verified)
            } else {
                format!(
                    "{} corrupted, {} orphaned out of {}",
                    report.corrupted.len(),
                    report.orphaned.len(),
                    report.total_files,
                )
            },
            footer: if is_clean {
                "status: braid status".to_string()
            } else {
                "fix: check .braid/txns/ for corrupted files".to_string()
            },
        };
        return Ok(CommandOutput { json, agent, human });
    }

    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;
    let hashes = layout.list_tx_hashes()?;
    let tx_since_harvest = count_txns_since_last_harvest(&store);

    // Deep mode: bilateral F(S) + optional graph analytics
    if deep {
        let deep_str = run_deep(path, &store, agent_name, spectral, full, commit)?;
        let fitness = store.fitness();
        let json = serde_json::json!({
            "mode": "deep",
            "fitness": fitness.total,
            "components": {
                "validation": fitness.components.validation,
                "coverage": fitness.components.coverage,
                "drift": fitness.components.drift,
                "harvest_quality": fitness.components.harvest_quality,
                "contradiction": fitness.components.contradiction,
                "incompleteness": fitness.components.incompleteness,
                "uncertainty": fitness.components.uncertainty,
            },
            "output": deep_str,
        });
        let agent = AgentOutput {
            context: format!("status --deep: F(S)={:.2}", fitness.total),
            content: format!(
                "V={:.2} C={:.2} D={:.2} H={:.2} K={:.2} I={:.2} U={:.2}",
                fitness.components.validation,
                fitness.components.coverage,
                fitness.components.drift,
                fitness.components.harvest_quality,
                fitness.components.contradiction,
                fitness.components.incompleteness,
                fitness.components.uncertainty,
            ),
            footer: "improve: braid status --verbose | commit: braid status --deep --commit"
                .to_string(),
        };
        return Ok(CommandOutput {
            json,
            agent,
            human: deep_str,
        });
    }

    // PERF-2a: Compute R(t) routing + calibration ONCE for the entire status invocation.
    // Previously called 2-5x (compute_action_from_store, derive_actions R18, build_verbose,
    // build_json), each O(tasks × datoms) ≈ 10s on a 70K datom / 256 task store.
    let (routings, calibration) = compute_routing_with_calibration(&store);

    // PERF-2: Compute all expensive values once (was computed 2x in build_json + build_status_projection)
    // PERF-3/4: Use layout cache for fitness/coherence acceleration
    let snapshot = StatusSnapshot::compute_with_layout(&store, path, Some(&layout));

    // ACP: Build the ActionProjection for status (INV-BUDGET-007)
    let projection = build_status_projection(
        path,
        &store,
        &hashes,
        tx_since_harvest,
        &snapshot,
        &routings,
        &calibration,
    );

    // UAQ-6 / ACP-TRACK-1: Record presentation counts for blocks that survive budget.
    // Done here (not in maybe_inject_footer) because the store is already loaded.
    // Other ACP commands track lazily on next status call.
    {
        let budget = braid_kernel::ActivationStrategy::Navigate.max_context_tokens();
        let labels = braid_kernel::extract_block_labels(&projection.context, budget);
        if !labels.is_empty() {
            let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
            let agent = braid_kernel::datom::AgentId::from_name("braid:attention");
            let tx = crate::commands::write::next_tx_id(&store, agent);
            let datoms = braid_kernel::record_block_presentations(&store, &label_refs, tx);
            if !datoms.is_empty() {
                let tx_file = braid_kernel::layout::TxFile {
                    tx_id: tx,
                    agent,
                    provenance: braid_kernel::datom::ProvenanceType::Derived,
                    rationale: "UAQ-6: presentation count tracking".to_string(),
                    causal_predecessors: vec![],
                    datoms,
                };
                let _ = layout.write_tx(&tx_file); // best-effort
            }
        }
    }

    // EXT-BUG-2: --json returns structured JSON early, before building text output
    if json {
        let fitness = &snapshot.fitness;
        let coherence = &snapshot.coherence;
        let score = &snapshot.methodology_score;
        let (open, in_progress, closed) = snapshot.task_counts;
        let total_all = open + in_progress + closed;
        let p_t = if total_all > 0 { closed as f64 / total_all as f64 } else { 0.0 };
        let registry = braid_kernel::default_boundaries();
        let evals = registry.evaluate_all(&store);
        let boundaries_json: Vec<serde_json::Value> = evals
            .iter()
            .map(|e| {
                serde_json::json!({
                    "name": e.name,
                    "coverage": e.relation.coverage,
                    "gaps": e.divergences.len(),
                })
            })
            .collect();
        let actions = derive_actions_with_precomputed(
            &store, &routings, &snapshot.coherence, None,
        );
        let actions_json: Vec<serde_json::Value> = actions
            .iter()
            .map(|a| {
                serde_json::json!({
                    "priority": a.priority,
                    "category": format!("{}", a.category),
                    "summary": a.summary,
                    "command": a.command,
                    "relates_to": a.relates_to,
                })
            })
            .collect();
        let session_boundary = braid_kernel::guidance::last_harvest_wall_time(&store);
        let session_tasks_closed = store
            .attribute_datoms(&Attribute::from_keyword(":task/status"))
            .iter()
            .filter(|d| {
                d.op == Op::Assert
                    && d.tx.wall_time() > session_boundary
                    && matches!(&d.value, Value::Keyword(k) if k.contains("closed"))
            })
            .count();
        let datom_delta = store
            .len()
            .saturating_sub(snapshot.session_start_datom_count);
        let tx_since = count_txns_since_last_harvest(&store);
        let trend_str = match score.trend {
            Trend::Up => "up",
            Trend::Down => "down",
            Trend::Stable => "stable",
        };
        let mut json_out = serde_json::json!({
            "store": {
                "datoms": store.len(),
                "entities": store.entity_count(),
                "txns": hashes.len(),
            },
            "coherence": {
                "fs": fitness.total,
                "phi": coherence.phi,
                "b1": coherence.beta_1,
                "mt": score.score,
                "quadrant": format!("{:?}", coherence.quadrant),
            },
            "boundaries": boundaries_json,
            "session": {
                "tasks_closed": session_tasks_closed,
                "datoms_added": datom_delta,
                "tx_since_harvest": tx_since,
            },
            "methodology": {
                "score": score.score,
                "trend": trend_str,
                "drift_signal": score.drift_signal,
            },
            "tasks": {
                "open": open,
                "in_progress": in_progress,
                "closed": closed,
                "p_t": p_t,
            },
            "actions": actions_json,
        });
        // Merge ACP metadata
        if let serde_json::Value::Object(ref mut map) = json_out {
            let acp = projection.to_json();
            if let serde_json::Value::Object(acp_map) = acp {
                for (k, v) in acp_map {
                    map.insert(k, v);
                }
            }
        }
        let json_str = serde_json::to_string_pretty(&json_out).unwrap() + "\n";
        let agent_output = AgentOutput {
            context: String::new(),
            content: projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate),
            footer: String::new(),
        };
        return Ok(CommandOutput {
            json: json_out,
            agent: agent_output,
            human: json_str,
        });
    }

    // Build human representation
    let human = if verbose {
        build_verbose(
            path,
            agent_name,
            &store,
            &hashes,
            tx_since_harvest,
            &routings,
            &snapshot.coherence,
        )
    } else {
        // Use ACP projection for human output (full detail, no truncation)
        projection.project(usize::MAX)
    };

    // EXT-BUG-1: When --verbose, use large budget to expand all sections (no omissions).
    // Navigate level (100 tokens) is too small and produces identical [+N omitted] output.
    let projected_agent = if verbose {
        projection.project(10000)
    } else {
        projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate)
    };
    let agent_output = AgentOutput {
        context: String::new(),
        content: projected_agent,
        footer: String::new(),
    };

    // Build JSON representation (deferred to non-json path to avoid redundant derive_actions)
    let json_value = build_json(StatusJsonParams {
        path,
        store: &store,
        hashes: &hashes,
        tx_since_harvest,
        agent_name,
        deep: false,
        spectral,
        verbose,
        snapshot: &snapshot,
        routings: &routings,
    });

    // Always merge ACP field into JSON (enables BAO-2 footer suppression)
    let mut final_json = json_value;
    if let serde_json::Value::Object(ref mut map) = final_json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    Ok(CommandOutput {
        json: final_json,
        agent: agent_output,
        human,
    })
}

/// Query the most recent active session's `:session/start-fitness` datom.
///
/// Returns `None` if no session has a start-fitness recorded (SD-1).
fn query_session_start_fitness(store: &braid_kernel::Store) -> Option<f64> {
    let attr = Attribute::from_keyword(":session/start-fitness");
    let started_attr = Attribute::from_keyword(":session/started-at");
    let status_attr = Attribute::from_keyword(":session/status");

    // Find the most recent active session entity
    let mut latest_wall = 0u64;
    let mut latest_session = None;
    for datom in store.attribute_datoms(&started_attr) {
        if datom.op != Op::Assert {
            continue;
        }
        if let Value::Long(wall) = datom.value {
            let wall = wall as u64;
            let is_active = store.entity_datoms(datom.entity).iter().any(|d| {
                d.attribute == status_attr
                    && d.op == Op::Assert
                    && matches!(&d.value, Value::Keyword(k) if k.contains("active"))
            });
            if is_active && wall > latest_wall {
                latest_wall = wall;
                latest_session = Some(datom.entity);
            }
        }
    }

    // Look for :session/start-fitness on the session entity
    let session = latest_session?;
    store
        .entity_datoms(session)
        .iter()
        .find(|d| d.attribute == attr && d.op == Op::Assert)
        .and_then(|d| match d.value {
            Value::Double(f) => Some(f.into_inner()),
            _ => None,
        })
}

/// Query the datom count at session start for computing session-level deltas (META-7).
fn query_session_start_datom_count(store: &braid_kernel::Store) -> usize {
    let started_attr = Attribute::from_keyword(":session/started-at");
    let status_attr = Attribute::from_keyword(":session/status");
    let count_attr = Attribute::from_keyword(":session/start-datom-count");

    let mut latest_wall = 0u64;
    let mut latest_session = None;
    for datom in store.attribute_datoms(&started_attr) {
        if datom.op != Op::Assert {
            continue;
        }
        if let Value::Long(wall) = datom.value {
            let wall = wall as u64;
            let is_active = store.entity_datoms(datom.entity).iter().any(|d| {
                d.attribute == status_attr
                    && d.op == Op::Assert
                    && matches!(&d.value, Value::Keyword(k) if k.contains("active"))
            });
            if is_active && wall > latest_wall {
                latest_wall = wall;
                latest_session = Some(datom.entity);
            }
        }
    }

    latest_session
        .and_then(|session| {
            store
                .entity_datoms(session)
                .iter()
                .find(|d| d.attribute == count_attr && d.op == Op::Assert)
                .and_then(|d| match d.value {
                    Value::Long(n) => Some(n as usize),
                    _ => None,
                })
        })
        .unwrap_or(0)
}

/// Build an ActionProjection for the status command (ACP-5, INV-BUDGET-007).
///
/// Decomposes status into Action + Context blocks at appropriate precedence:
/// - Action: R(t) top recommendation via compute_action_from_store()
/// - Context[System]: store identity (datoms, entities, txns)
/// - Context[Methodology]: coherence F(S) + M(t) + trend
/// - Context[Methodology]: boundary coverage
/// - Context[UserRequested]: task summary (open/ready/blocked)
/// - Context[Speculative]: methodology gaps
/// - Context[Methodology]: harvest status
/// - Evidence: "braid status --verbose"
pub fn build_status_projection(
    path: &Path,
    store: &braid_kernel::Store,
    hashes: &[String],
    tx_since_harvest: usize,
    snapshot: &StatusSnapshot,
    routings: &[TaskRouting],
    calibration: &CalibrationReport,
) -> braid_kernel::ActionProjection {
    use braid_kernel::budget::{ContextBlock, OutputPrecedence};

    // Action: unified R(t) recommendation (PERF-2a: use pre-computed routing)
    let action = compute_action_from_routing(store, routings);

    // Build context blocks in precedence order (highest first)
    let mut context = Vec::new();

    // 1. Store identity (System -- always shown)
    context.push(ContextBlock::new_scored(
        OutputPrecedence::System,
        format!(
            "store: {} ({} datoms, {} entities, {} txns)",
            path.display(),
            store.len(),
            store.entity_count(),
            hashes.len(),
        ),
        15,
    ));

    // 2. Coherence + F(S) with session delta (SD-1) + M(t) with session age (SD-2)
    // PERF-2: Use pre-computed snapshot values
    let fitness = &snapshot.fitness;
    let coherence = &snapshot.coherence;
    let score = &snapshot.methodology_score;
    let trend_str = match score.trend {
        Trend::Up => "up",
        Trend::Down => "down",
        Trend::Stable => "stable",
    };

    // SD-1: F(S) session delta — use snapshot
    let fitness_delta_str = match snapshot.session_start_fitness {
        Some(start) => {
            let delta = fitness.total - start;
            if delta.abs() < 0.005 {
                " (=)".to_string()
            } else if delta > 0.0 {
                format!(" (+{:.2})", delta)
            } else {
                format!(" ({:.2})", delta)
            }
        }
        None => String::new(),
    };

    // SD-2: M(t) session-age qualifier
    let session_age_str = if tx_since_harvest < 5 {
        " (new session)"
    } else {
        ""
    };

    context.push(ContextBlock::new_scored(
        OutputPrecedence::Methodology,
        format!(
            "coherence: F(S)={:.2}{} Phi={:.1} B1={} {:?} | M(t)={:.2} {}{}{}",
            fitness.total,
            fitness_delta_str,
            coherence.phi,
            coherence.beta_1,
            coherence.quadrant,
            score.score,
            trend_str,
            if score.drift_signal { " DRIFT" } else { "" },
            session_age_str,
        ),
        25,
    ));

    // 3. Boundary coverage (Methodology)
    let registry = braid_kernel::default_boundaries();
    let evals = registry.evaluate_all(store);
    if !evals.is_empty() {
        let boundary_parts: Vec<String> = evals
            .iter()
            .map(|e| {
                // FIX-VACUOUS: empty boundaries show "not measured" instead of vacuous 1.00
                if e.relation.source_total == 0 && e.relation.target_total == 0 {
                    format!("{} (not measured)", e.name)
                } else {
                    let gap_count = e.divergences.len();
                    if gap_count > 0 {
                        format!("{} {:.2} ({} gaps)", e.name, e.relation.coverage, gap_count)
                    } else {
                        format!("{} {:.2}", e.name, e.relation.coverage)
                    }
                }
            })
            .collect();
        context.push(ContextBlock::new_scored(
            OutputPrecedence::Methodology,
            format!("boundaries: {}", boundary_parts.join(" | ")),
            12,
        ));
    }

    // 4. Task summary (UserRequested) — use snapshot
    let (open, in_progress, closed) = snapshot.task_counts;
    let total_open = open + in_progress;
    if total_open > 0 {
        let ready_count = snapshot.ready_set.len();
        let blocked = open.saturating_sub(ready_count);
        let total_all = open + in_progress + closed;
        let p_t = if total_all > 0 {
            closed as f64 / total_all as f64
        } else {
            0.0
        };
        context.push(ContextBlock::new_scored(
            OutputPrecedence::UserRequested,
            format!(
                "tasks: {} open ({} ready, {} blocked, {} in-progress, {} closed) | P(t)={:.2}",
                total_open, ready_count, blocked, in_progress, closed, p_t
            ),
            18,
        ));
    }

    // 4b. Session progress (Methodology, META-7) — use snapshot
    if let Some(start_fitness) = snapshot.session_start_fitness {
        let start_datom_count = snapshot.session_start_datom_count;
        let session_boundary = braid_kernel::guidance::last_harvest_wall_time(store);
        let session_tasks_closed = store
            .attribute_datoms(&braid_kernel::datom::Attribute::from_keyword(":task/status"))
            .iter()
            .filter(|d| {
                d.op == braid_kernel::datom::Op::Assert
                    && d.tx.wall_time() > session_boundary
                    && matches!(&d.value, braid_kernel::datom::Value::Keyword(k) if k.contains("closed"))
            })
            .count();
        let datom_delta = store.len().saturating_sub(start_datom_count);
        let fitness_delta = fitness.total - start_fitness;
        let mut session_str = format!("session: +{session_tasks_closed} tasks, +{datom_delta} datoms");
        if fitness_delta.abs() > 0.005 {
            session_str.push_str(&format!(", F(S) {:+.2}", fitness_delta));
        }
        context.push(ContextBlock::new_scored(
            OutputPrecedence::Methodology,
            session_str,
            12,
        ));
    }

    // 5. Harvest status (Methodology)
    let harvest_status = if tx_since_harvest >= 15 {
        format!("harvest: {} tx since last -- OVERDUE", tx_since_harvest)
    } else if tx_since_harvest >= 8 {
        format!("harvest: {} tx since last (due soon)", tx_since_harvest)
    } else {
        format!("harvest: {} tx since last (ok)", tx_since_harvest)
    };
    context.push(ContextBlock::new_scored(
        OutputPrecedence::Methodology,
        harvest_status,
        10,
    ));

    // 5a. HL-4: Hypothesis calibration metrics (PERF-2a: use pre-computed calibration)
    if calibration.total_hypotheses > 0 {
        let trend_str = match calibration.trend {
            braid_kernel::guidance::CalibrationTrend::Improving => "improving",
            braid_kernel::guidance::CalibrationTrend::Stable => "stable",
            braid_kernel::guidance::CalibrationTrend::Degrading => "degrading",
            braid_kernel::guidance::CalibrationTrend::Insufficient => "insufficient data",
        };
        context.push(ContextBlock::new_scored(
            OutputPrecedence::Methodology,
            format!(
                "hypotheses: {}/{} completed, mean error {:.3}, trend: {}",
                calibration.completed_hypotheses, calibration.total_hypotheses,
                calibration.mean_error, trend_str
            ),
            12,
        ));
    }

    // 5b. Trace staleness (Methodology, SC-2) — use snapshot
    let ts = &snapshot.trace_status;
    let trace_content = match ts {
        TraceStaleStatus::Fresh { total } => {
            format!("trace: fresh ({} files scanned)", total)
        }
        TraceStaleStatus::Stale { stale, .. } => {
            format!(
                "trace: stale ({} files modified since last scan) \u{2014} run: braid trace --commit",
                stale
            )
        }
    };
    context.push(ContextBlock::new_scored(
        OutputPrecedence::Methodology,
        trace_content,
        12,
    ));

    // 6. Methodology gaps — use snapshot (AGP-4.2, INV-GUIDANCE-021)
    let ag = &snapshot.adjusted_gaps;
    if !ag.is_empty() {
        let mut gap_parts = Vec::new();
        if ag.adjusted.crystallization > 0 {
            gap_parts.push(format!("{} uncrystallized", ag.adjusted.crystallization));
        }
        if ag.adjusted.unanchored > 0 {
            gap_parts.push(format!("{} unanchored", ag.adjusted.unanchored));
        }
        if ag.adjusted.untested > 0 {
            gap_parts.push(format!("{} untested", ag.adjusted.untested));
        }
        if ag.adjusted.stale_witnesses > 0 {
            gap_parts.push(format!("{} stale witnesses", ag.adjusted.stale_witnesses));
        }
        for cs in &ag.adjusted.concentration {
            gap_parts.push(format!(
                "concentration: {} traces in {} \u{2014} review related tasks",
                cs.trace_count, cs.neighborhood
            ));
        }
        context.push(ContextBlock::new_scored(
            OutputPrecedence::Speculative,
            format!(
                "gaps: {} ({} mode) \u{2014} {}",
                ag.total(),
                ag.mode_label(),
                gap_parts.join(", ")
            ),
            15,
        ));
    }

    // FEGH-1: Surface bridge hypotheses as speculative context blocks.
    // The free energy gradient suggests questions that bridge disconnected
    // knowledge communities — the highest-value observations to make next.
    let bridges = braid_kernel::guidance::generate_bridge_hypotheses(store, 3);
    if let Some(top) = bridges.first() {
        context.push(ContextBlock::new_scored(
            OutputPrecedence::Speculative,
            format!(
                "bridge: {} (ΔF(S)={:+.3}, α={:.4})",
                top.question, top.delta_fs, top.alpha
            ),
            8,
        ));
    }

    // Add methodology M(t) context blocks (ACP-9: footer -> context)
    // PERF-2a: Reuse calibration from pre-computed routing (was redundant O(H*K) scan).
    let methodology_blocks =
        braid_kernel::guidance::methodology_context_blocks_with_calibration(store, Some(calibration));
    context.extend(methodology_blocks);

    // Sort context blocks by precedence (highest first) so that
    // budget-constrained modes (Agent/Navigate at 100 tokens) render
    // the most important blocks first (INV-BUDGET-008: monotonic fill).
    context.sort_by(|a, b| {
        (b.precedence as u8).cmp(&(a.precedence as u8))
    });

    braid_kernel::ActionProjection {
        action,
        context,
        evidence_pointer: "details: braid status --verbose".to_string(),
    }
}

/// Build the terse dashboard string (default mode).
#[allow(dead_code)]
fn build_terse(
    path: &Path,
    store: &braid_kernel::Store,
    hashes: &[String],
    tx_since_harvest: usize,
) -> String {
    let coherence = check_coherence_fast(store);
    let telemetry = telemetry_from_store(store);
    let score = compute_methodology_score(&telemetry);
    let actions = derive_actions(store);
    // CE-4: O(1) fitness via materialized views
    let fitness = store.fitness();

    let harvest_tag = if tx_since_harvest >= 15 {
        " OVERDUE"
    } else if tx_since_harvest >= 8 {
        " (harvest?)"
    } else {
        " (ok)"
    };

    let trend_str = match score.trend {
        Trend::Up => "up",
        Trend::Down => "down",
        Trend::Stable => "stable",
    };

    // SD-1: F(S) session delta
    let fitness_delta_str = match query_session_start_fitness(store) {
        Some(start) => {
            let delta = fitness.total - start;
            if delta.abs() < 0.005 {
                " (=)".to_string()
            } else if delta > 0.0 {
                format!(" (+{:.2})", delta)
            } else {
                format!(" ({:.2})", delta)
            }
        }
        None => String::new(),
    };

    // SD-2: M(t) session-age qualifier
    let session_age_str = if tx_since_harvest < 5 {
        " (new session)"
    } else {
        ""
    };

    let mut out = String::new();
    out.push_str(&format!(
        "store: {} ({} datoms, {} entities, {} txns)\n",
        path.display(),
        store.len(),
        store.entity_count(),
        hashes.len(),
    ));
    out.push_str(&format!(
        "coherence: F(S)={:.2}{} Phi={:.1} B1={} {:?} | M(t)={:.2} {}{}{}\n",
        fitness.total,
        fitness_delta_str,
        coherence.phi,
        coherence.beta_1,
        coherence.quadrant,
        score.score,
        trend_str,
        if score.drift_signal { " DRIFT" } else { "" },
        session_age_str,
    ));
    out.push_str(&format!(
        "live: intent={} spec={} impl={} | agents={}\n",
        coherence.live_intent,
        coherence.live_spec,
        coherence.live_impl,
        store.frontier().len(),
    ));

    // Per-boundary coverage display (INTEGRATION-1, INV-BILATERAL-009)
    let registry = braid_kernel::default_boundaries();
    let evals = registry.evaluate_all(store);
    if !evals.is_empty() {
        let boundary_parts: Vec<String> = evals
            .iter()
            .map(|e| {
                // FIX-VACUOUS: empty boundaries show "not measured" instead of vacuous 1.00
                if e.relation.source_total == 0 && e.relation.target_total == 0 {
                    format!("{} (not measured)", e.name)
                } else {
                    let gap_count = e.divergences.len();
                    if gap_count > 0 {
                        format!("{} {:.2} ({} gaps)", e.name, e.relation.coverage, gap_count)
                    } else {
                        format!("{} {:.2}", e.name, e.relation.coverage)
                    }
                }
            })
            .collect();
        out.push_str(&format!("boundaries: {}\n", boundary_parts.join(" | ")));
    }

    out.push_str(&format!(
        "harvest: {} tx since last{}\n",
        tx_since_harvest, harvest_tag,
    ));

    // HL-4: Calibration metrics — show hypothesis ledger stats
    let cal = braid_kernel::guidance::compute_calibration_metrics(store);
    if cal.total_hypotheses > 0 {
        let trend_str = match cal.trend {
            braid_kernel::guidance::CalibrationTrend::Improving => "improving",
            braid_kernel::guidance::CalibrationTrend::Stable => "stable",
            braid_kernel::guidance::CalibrationTrend::Degrading => "degrading",
            braid_kernel::guidance::CalibrationTrend::Insufficient => "insufficient data",
        };
        out.push_str(&format!(
            "hypotheses: {}/{} completed, mean error {:.3}, trend: {}\n",
            cal.completed_hypotheses, cal.total_hypotheses, cal.mean_error, trend_str
        ));
    }

    // HL-CALIBRATE: Weight adjustment recommendations from hypothesis outcomes
    let weight_adjustments = braid_kernel::calibrate_boundary_weights(store);
    if !weight_adjustments.is_empty() {
        out.push_str(&format!(
            "calibration: {} boundaries need weight adjustment\n",
            weight_adjustments.len()
        ));
        for adj in &weight_adjustments {
            out.push_str(&format!(
                "  {} ({} samples, error {:.3}): {:.3} \u{2192} {:.3}\n",
                adj.boundary_name, adj.sample_count, adj.mean_error,
                adj.current_weight, adj.recommended_weight,
            ));
        }
    }

    // Trace staleness (SC-2)
    let ts = trace_staleness(store, path);
    out.push_str(&format_trace_line(&ts));

    // Task summary (D4.1: INV-INTERFACE-011) + P(t) progress metric (ZCM-PT)
    let (open, in_progress, closed) = braid_kernel::task_counts(store);
    let total = open + in_progress + closed;
    if total > 0 {
        let ready_count = braid_kernel::compute_ready_set(store).len();
        let blocked = open - ready_count;
        let p_t = closed as f64 / total as f64;
        let mut task_line = format!("tasks: {open} open");
        if in_progress > 0 {
            task_line.push_str(&format!(", {in_progress} active"));
        }
        task_line.push_str(&format!(
            " ({ready_count} ready, {blocked} blocked) | {closed} closed | P(t)={p_t:.2}\n"
        ));
        out.push_str(&task_line);
    }

    // META-7: Session progress dashboard — session-level deltas (INV-INTERFACE-008)
    if let Some(start_fitness) = query_session_start_fitness(store) {
        let start_datom_count = query_session_start_datom_count(store);
        let session_boundary = braid_kernel::guidance::last_harvest_wall_time(store);

        // Count tasks closed this session (status=closed with wall_time > session_boundary)
        let session_tasks_closed = store
            .datoms()
            .filter(|d| {
                d.attribute.as_str() == ":task/status"
                    && d.op == braid_kernel::datom::Op::Assert
                    && d.tx.wall_time() > session_boundary
                    && matches!(&d.value, braid_kernel::datom::Value::Keyword(k) if k.contains("closed"))
            })
            .count();

        let datom_delta = store.len().saturating_sub(start_datom_count);
        let fitness_delta = fitness.total - start_fitness;

        let mut session_line = format!("session: +{session_tasks_closed} tasks");
        session_line.push_str(&format!(", +{datom_delta} datoms"));
        if fitness_delta.abs() > 0.005 {
            session_line.push_str(&format!(", F(S) {:+.2}", fitness_delta));
        }
        session_line.push('\n');
        out.push_str(&session_line);
    }

    // Methodology gaps with activity-mode suppression (T6-1, INV-GUIDANCE-021)
    let raw_gaps = methodology_gaps(store);
    let mode = detect_activity_mode(&telemetry);
    let ag = adjust_gaps(raw_gaps, mode);
    if !ag.raw.is_empty() {
        out.push_str(&format!(
            "\u{26a0} methodology gaps ({} mode):\n",
            ag.mode_label()
        ));
        if ag.adjusted.crystallization > 0 {
            out.push_str(&format!(
                "  crystallization: {} uncrystallized spec IDs ({} raw)\n",
                ag.adjusted.crystallization, ag.raw.crystallization
            ));
        }
        if ag.adjusted.unanchored > 0 {
            out.push_str(&format!(
                "  unanchored: {} tasks with unresolved spec refs ({} raw)\n",
                ag.adjusted.unanchored, ag.raw.unanchored
            ));
        }
        if ag.adjusted.untested > 0 {
            out.push_str(&format!(
                "  untested: {} INVs with only L1 witnesses ({} raw)\n",
                ag.adjusted.untested, ag.raw.untested
            ));
        }
        if ag.adjusted.stale_witnesses > 0 {
            out.push_str(&format!(
                "  stale: {} invalidated witnesses ({} raw)\n",
                ag.adjusted.stale_witnesses, ag.raw.stale_witnesses
            ));
        }
        for cs in &ag.adjusted.concentration {
            out.push_str(&format!(
                "  concentration: {} traces in {} \u{2014} review related tasks\n",
                cs.trace_count, cs.neighborhood
            ));
        }
    }

    // Top action with copy-pasteable command
    if let Some(action) = actions.first() {
        let cmd = action.command.as_deref().unwrap_or("-");
        let spec_ref = if action.relates_to.is_empty() {
            String::new()
        } else {
            format!(" [{}]", action.relates_to.join(", "))
        };
        out.push_str(&format!(
            "next: {} \u{2014} {}\n  run: {}\n  ref:{}\n",
            action.category, action.summary, cmd, spec_ref,
        ));
    } else {
        out.push_str("next: none\n");
    }

    out
}

/// Build agent-mode three-part structure (INV-OUTPUT-002, <=300 tokens).
#[allow(dead_code)]
fn build_agent(
    path: &Path,
    store: &braid_kernel::Store,
    hashes: &[String],
    tx_since_harvest: usize,
) -> AgentOutput {
    let coherence = check_coherence_fast(store);
    let telemetry = telemetry_from_store(store);
    let score = compute_methodology_score(&telemetry);
    let actions = derive_actions(store);
    // CE-4: O(1) fitness via materialized views
    let fitness = store.fitness();

    let trend_str = match score.trend {
        Trend::Up => "up",
        Trend::Down => "down",
        Trend::Stable => "stable",
    };

    let harvest_status = if tx_since_harvest >= 15 {
        "OVERDUE"
    } else if tx_since_harvest >= 8 {
        "due"
    } else {
        "ok"
    };

    // SD-1: F(S) session delta
    let fitness_delta_str = match query_session_start_fitness(store) {
        Some(start) => {
            let delta = fitness.total - start;
            if delta.abs() < 0.005 {
                " (=)".to_string()
            } else if delta > 0.0 {
                format!(" (+{:.2})", delta)
            } else {
                format!(" ({:.2})", delta)
            }
        }
        None => String::new(),
    };

    // SD-2: M(t) session-age qualifier
    let session_age_str = if tx_since_harvest < 5 {
        " (new session)"
    } else {
        ""
    };

    // Context: what store this is about
    let context = format!(
        "store: {} ({} datoms, {} entities, {} txns)",
        path.display(),
        store.len(),
        store.entity_count(),
        hashes.len(),
    );

    // Trace staleness (SC-2)
    let ts = trace_staleness(store, path);
    let trace_agent = match &ts {
        TraceStaleStatus::Fresh { total } => format!("trace: fresh ({})", total),
        TraceStaleStatus::Stale { stale, .. } => format!("trace: stale ({})", stale),
    };

    // Content: coherence + F(S) + methodology + harvest + trace + tasks
    let mut content = format!(
        "coherence: F(S)={:.2}{} Phi={:.1} B1={} {:?} | M(t)={:.2} {}{}\nharvest: {} tx since last ({}) | {}",
        fitness.total,
        fitness_delta_str,
        coherence.phi,
        coherence.beta_1,
        coherence.quadrant,
        score.score,
        trend_str,
        session_age_str,
        tx_since_harvest,
        harvest_status,
        trace_agent,
    );

    let (open, in_progress, _closed) = braid_kernel::task_counts(store);
    let total_open = open + in_progress;
    if total_open > 0 {
        let ready_count = braid_kernel::compute_ready_set(store).len();
        content.push_str(&format!(
            " | tasks: {} open ({} ready)",
            total_open, ready_count
        ));
    }

    // Methodology gaps with activity-mode suppression (T6-1, INV-GUIDANCE-021)
    let raw_gaps = methodology_gaps(store);
    let mode = detect_activity_mode(&telemetry);
    let ag = adjust_gaps(raw_gaps, mode);
    if !ag.raw.is_empty() {
        content.push_str(&format!(
            " | gaps: {} ({} mode, {} raw)",
            ag.total(),
            ag.mode_label(),
            ag.raw.total()
        ));
        if ag.adjusted.crystallization > 0 {
            content.push_str(&format!(" (cryst:{})", ag.adjusted.crystallization));
        }
        if ag.adjusted.unanchored > 0 {
            content.push_str(&format!(" (unanchored:{})", ag.adjusted.unanchored));
        }
        if ag.adjusted.untested > 0 {
            content.push_str(&format!(" (untested:{})", ag.adjusted.untested));
        }
        if ag.adjusted.stale_witnesses > 0 {
            content.push_str(&format!(" (stale:{})", ag.adjusted.stale_witnesses));
        }
        for cs in &ag.adjusted.concentration {
            content.push_str(&format!(" (conc:{}/{})", cs.trace_count, cs.neighborhood));
        }
    }

    // Footer: next action as a runnable command
    let footer = if let Some(action) = actions.first() {
        let cmd = action.command.as_deref().unwrap_or(&action.summary);
        format!("next: {cmd}")
    } else {
        "next: braid observe \"...\" --confidence 0.7".to_string()
    };

    AgentOutput {
        context,
        content,
        footer,
    }
}

/// Full verbose output with all metrics and actions.
fn build_verbose(
    path: &Path,
    agent_name: &str,
    store: &braid_kernel::Store,
    hashes: &[String],
    tx_since_harvest: usize,
    routings: &[TaskRouting],
    coherence: &CoherenceReport,
) -> String {
    use braid_kernel::bilateral::{
        W_CONTRADICTION, W_COVERAGE, W_DRIFT, W_HARVEST, W_INCOMPLETENESS, W_UNCERTAINTY,
        W_VALIDATION,
    };

    let telemetry = telemetry_from_store(store);
    let score = compute_methodology_score(&telemetry);
    let actions = derive_actions_with_precomputed(store, routings, coherence, None);
    let fitness = store.fitness();

    // SD-1: F(S) session delta for verbose
    let fitness_delta_str = match query_session_start_fitness(store) {
        Some(start) => {
            let delta = fitness.total - start;
            if delta.abs() < 0.005 {
                " (=)".to_string()
            } else if delta > 0.0 {
                format!(" (+{:.2})", delta)
            } else {
                format!(" ({:.2})", delta)
            }
        }
        None => String::new(),
    };

    let mut out = String::new();
    out.push_str(&format!(
        "status: agent={} store={} ({} datoms, {} entities, {} txns)\n",
        agent_name,
        path.display(),
        store.len(),
        store.entity_count(),
        hashes.len(),
    ));
    out.push_str(&format!(
        "coherence: Phi={:.1} B1={} quadrant={:?}\n",
        coherence.phi, coherence.beta_1, coherence.quadrant,
    ));
    out.push_str(&format!(
        "  D_IS={} D_SP={} ISP_bypasses={}\n",
        coherence.components.d_is, coherence.components.d_sp, coherence.isp_bypasses,
    ));
    out.push_str(&format!(
        "  LIVE: intent={} spec={} impl={}\n",
        coherence.live_intent, coherence.live_spec, coherence.live_impl,
    ));
    out.push_str(&format!(
        "  entropy: S_vN={:.3} normalized={:.3} effective_rank={:.1}\n",
        coherence.entropy.entropy, coherence.entropy.normalized, coherence.entropy.effective_rank,
    ));

    // F(S) formula breakdown with component weights and values + session delta
    let c = &fitness.components;
    out.push_str(&format!(
        "F(S) = {:.2}{} = {:.2}*validation({:.2}) + {:.2}*coverage({:.2}) + {:.2}*drift({:.2}) + {:.2}*harvest({:.2}) + {:.2}*contradiction({:.2}) + {:.2}*incompleteness({:.2}) + {:.2}*uncertainty({:.2})\n",
        fitness.total,
        fitness_delta_str,
        W_VALIDATION, c.validation,
        W_COVERAGE, c.coverage,
        W_DRIFT, c.drift,
        W_HARVEST, c.harvest_quality,
        W_CONTRADICTION, c.contradiction,
        W_INCOMPLETENESS, c.incompleteness,
        W_UNCERTAINTY, c.uncertainty,
    ));

    // Weakest component identification with improvement suggestion
    let components: [(&str, f64, &str); 7] = [
        ("validation", c.validation, "braid verify"),
        ("coverage", c.coverage, "braid trace"),
        ("drift", c.drift, "braid status --deep"),
        (
            "harvest_quality",
            c.harvest_quality,
            "braid harvest --commit",
        ),
        (
            "contradiction",
            c.contradiction,
            "braid query [:find ?e :where [?e :spec/element-type \"invariant\"]]",
        ),
        (
            "incompleteness",
            c.incompleteness,
            "braid observe \"...\" --confidence 0.8",
        ),
        (
            "uncertainty",
            c.uncertainty,
            "braid observe \"...\" --confidence 0.9",
        ),
    ];
    if let Some((name, val, cmd)) = components
        .iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    {
        out.push_str(&format!(
            "Weakest: {} ({:.2}) \u{2014} improve: {}\n",
            name, val, cmd,
        ));
    }

    // M(t) sub-metric details with thresholds and pass/fail
    let m = &score.components;
    let trend_str = match score.trend {
        Trend::Up => "up",
        Trend::Down => "down",
        Trend::Stable => "stable",
    };
    // SD-2: session age qualifier
    let session_age_str = if tx_since_harvest < 5 {
        " (new session)"
    } else {
        ""
    };
    out.push_str(&format!(
        "methodology: M(t)={:.2} trend={}{}\n",
        score.score, trend_str, session_age_str,
    ));
    if score.drift_signal {
        out.push_str("  WARNING: drift signal active (M(t) < 0.5)\n");
    }
    // Sub-metrics: name, value, weight, threshold (healthy = above 0.4)
    let sub_metrics: [(&str, f64, f64, f64); 4] = [
        ("transact_frequency", m.transact_frequency, 0.30, 0.40),
        ("spec_language_ratio", m.spec_language_ratio, 0.23, 0.30),
        ("query_diversity", m.query_diversity, 0.17, 0.25),
        ("harvest_quality", m.harvest_quality, 0.30, 0.50),
    ];
    out.push_str("M(t) sub-metrics:\n");
    for (name, val, weight, threshold) in &sub_metrics {
        let status = if *val >= *threshold { "above" } else { "below" };
        out.push_str(&format!(
            "  {}: {:.2} (weight: {:.2}, threshold: {:.2}) \u{2014} {}\n",
            name, val, weight, threshold, status,
        ));
    }

    // R(t) Routing weights dashboard (RFL-5: follow-through visibility)
    {
        use braid_kernel::guidance::routing_dashboard;
        let rd = routing_dashboard(store);
        let mode_label = if rd.learned {
            "learned"
        } else if rd.preview {
            "preview (defaults)"
        } else {
            "defaults"
        };
        out.push_str(&format!("R(t) routing: {}\n", mode_label));
        out.push_str("  weights:\n");
        for (name, &w) in rd.feature_names.iter().zip(rd.weights.iter()) {
            out.push_str(&format!("    {}: {:.3}\n", name, w));
        }
        out.push_str(&format!(
            "  action-outcome pairs: {} total, {} with outcome, {} followed\n",
            rd.total_actions, rd.actions_with_outcome, rd.followed_count,
        ));
        out.push_str(&format!(
            "  follow-through rate: {:.1}%\n",
            rd.follow_through_rate * 100.0,
        ));
        if rd.preview {
            out.push_str(&format!(
                "  note: {} / 50 data points \u{2014} using default weights (collect more action-outcome pairs to enable learned weights)\n",
                rd.total_actions,
            ));
        }
    }

    // Harvest health
    let harvest_warning = if tx_since_harvest >= 15 {
        " [OVERDUE \u{2014} harvest immediately]"
    } else if tx_since_harvest >= 8 {
        " [consider harvesting]"
    } else {
        ""
    };
    out.push_str(&format!(
        "harvest: {} tx since last{}\n",
        tx_since_harvest, harvest_warning
    ));

    // HL-4: Calibration metrics in verbose status
    let cal = braid_kernel::guidance::compute_calibration_metrics(store);
    if cal.total_hypotheses > 0 {
        let trend_str = match cal.trend {
            braid_kernel::guidance::CalibrationTrend::Improving => "improving",
            braid_kernel::guidance::CalibrationTrend::Stable => "stable",
            braid_kernel::guidance::CalibrationTrend::Degrading => "degrading",
            braid_kernel::guidance::CalibrationTrend::Insufficient => "insufficient data",
        };
        out.push_str(&format!(
            "hypotheses: {}/{} completed, mean error {:.3}, trend: {}\n",
            cal.completed_hypotheses, cal.total_hypotheses, cal.mean_error, trend_str
        ));
    }

    // Trace staleness (SC-2)
    let ts = trace_staleness(store, path);
    out.push_str(&format_trace_line(&ts));

    // Methodology gaps with activity-mode suppression (T6-1, INV-GUIDANCE-021)
    let raw_gaps = methodology_gaps(store);
    let mode = detect_activity_mode(&telemetry);
    let ag = adjust_gaps(raw_gaps, mode);
    if !ag.raw.is_empty() {
        out.push_str(&format!(
            "methodology gaps: {} adjusted ({} mode, {} raw)\n",
            ag.total(),
            ag.mode_label(),
            ag.raw.total()
        ));
        if ag.adjusted.crystallization > 0 {
            out.push_str(&format!(
                "  crystallization: {} uncrystallized spec IDs ({} raw, run: braid spec create <ID>)\n",
                ag.adjusted.crystallization, ag.raw.crystallization
            ));
        }
        if ag.adjusted.unanchored > 0 {
            out.push_str(&format!(
                "  unanchored: {} tasks with unresolved spec refs ({} raw)\n",
                ag.adjusted.unanchored, ag.raw.unanchored
            ));
        }
        if ag.adjusted.untested > 0 {
            out.push_str(&format!(
                "  untested: {} INVs with only L1 witnesses ({} raw)\n",
                ag.adjusted.untested, ag.raw.untested
            ));
        }
        if ag.adjusted.stale_witnesses > 0 {
            out.push_str(&format!(
                "  stale: {} invalidated witnesses ({} raw)\n",
                ag.adjusted.stale_witnesses, ag.raw.stale_witnesses
            ));
        }
        for cs in &ag.adjusted.concentration {
            out.push_str(&format!(
                "  concentration: {} traces in {} \u{2014} review related tasks\n",
                cs.trace_count, cs.neighborhood
            ));
        }
    }

    // Frontier
    out.push_str("frontier:\n");
    for (agent, tx_id) in store.frontier() {
        out.push_str(&format!("  {:?}: wall={}\n", agent, tx_id.wall_time()));
    }

    // All R(t) actions (guidance-derived)
    out.push_str(&format_actions(&actions));

    // R(t) task routing (graph-based, INV-GUIDANCE-010) — PERF-2a: use pre-computed
    if !routings.is_empty() {
        out.push_str("All R(t) actions:\n");
        for (i, r) in routings.iter().enumerate() {
            out.push_str(&format!(
                "  [{}] WORK: \"{}\" (impact={:.2}) \u{2192} braid go {}\n",
                i + 1,
                r.label,
                r.impact,
                r.label,
            ));
        }
    }

    out
}

/// Deep mode: bilateral F(S) + graph analytics + convergence.
fn run_deep(
    path: &Path,
    store: &braid_kernel::Store,
    agent_name: &str,
    spectral: bool,
    full: bool,
    commit: bool,
) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;

    // Bilateral cycle
    let history = load_trajectory(store);
    let state = run_cycle(store, &history, spectral);

    let mut out = if full {
        bilateral_format_verbose(&state)
    } else {
        bilateral_format_terse(&state)
    };

    // Graph analytics (when --full)
    if full {
        out.push_str("\n--- graph analytics ---\n");
        match super::analyze::run_budget(path, 500, false) {
            Ok(analytics) => out.push_str(&analytics),
            Err(e) => out.push_str(&format!("analytics error: {e}\n")),
        }
    }

    // Commit bilateral results if requested
    if commit {
        let agent = AgentId::from_name(agent_name);
        let tx_id = super::write::next_tx_id(store, agent);
        let datoms = cycle_to_datoms(&state, tx_id);
        let datom_count = datoms.len();

        let tx = TxFile {
            tx_id,
            agent,
            provenance: ProvenanceType::Derived,
            rationale: format!(
                "Bilateral cycle {}: F(S)={:.4}",
                state.cycle_count, state.fitness.total
            ),
            causal_predecessors: vec![],
            datoms,
        };

        let file_path = layout.write_tx(&tx)?;
        out.push_str(&format!(
            "committed: {} datoms \u{2192} {}\n",
            datom_count,
            file_path.relative_path()
        ));
    }

    Ok(out)
}

/// Parameters for building the JSON status representation.
struct StatusJsonParams<'a> {
    path: &'a Path,
    store: &'a braid_kernel::Store,
    hashes: &'a [String],
    tx_since_harvest: usize,
    agent_name: &'a str,
    deep: bool,
    spectral: bool,
    verbose: bool,
    snapshot: &'a StatusSnapshot,
    routings: &'a [TaskRouting],
}

/// Build JSON value with all structured data.
fn build_json(params: StatusJsonParams<'_>) -> serde_json::Value {
    let StatusJsonParams {
        path,
        store,
        hashes,
        tx_since_harvest,
        agent_name,
        deep,
        spectral,
        verbose,
        snapshot,
        routings,
    } = params;
    // PERF-2: Use pre-computed snapshot values instead of re-computing
    let coherence = &snapshot.coherence;
    let _telemetry = &snapshot.telemetry;
    let score = &snapshot.methodology_score;
    let actions = derive_actions_with_precomputed(store, routings, coherence, None);

    let frontier: Vec<serde_json::Value> = store
        .frontier()
        .iter()
        .map(|(agent, tx_id)| {
            serde_json::json!({
                "agent": format!("{:?}", agent),
                "wall_time": tx_id.wall_time(),
            })
        })
        .collect();

    let actions_json: Vec<serde_json::Value> = actions
        .iter()
        .map(|a| {
            serde_json::json!({
                "priority": a.priority,
                "category": format!("{}", a.category),
                "summary": a.summary,
                "command": a.command,
                "relates_to": a.relates_to,
            })
        })
        .collect();

    // SD-1: F(S) session delta from snapshot
    let fitness = &snapshot.fitness;
    let session_fitness_delta = snapshot
        .session_start_fitness
        .map(|start| fitness.total - start);

    // ZCM-PT: P(t) progress metric from snapshot
    let (pt_open, pt_ip, pt_closed) = snapshot.task_counts;
    let pt_total = pt_open + pt_ip + pt_closed;
    let p_t_value = if pt_total > 0 {
        pt_closed as f64 / pt_total as f64
    } else {
        0.0
    };
    let progress_json = serde_json::json!({
        "p_t": p_t_value,
        "closed": pt_closed,
        "total": pt_total,
    });

    // SD-2: Session age for JSON
    let session_age = if tx_since_harvest < 5 {
        "new"
    } else {
        "established"
    };

    let mut result = serde_json::json!({
        "store": path.display().to_string(),
        "datom_count": store.len(),
        "transaction_count": hashes.len(),
        "entity_count": store.entities().len(),
        "schema_attribute_count": store.schema().len(),
        "frontier": frontier,
        "tx_since_last_harvest": tx_since_harvest,
        "coherence": {
            "phi": coherence.phi,
            "beta_1": coherence.beta_1,
            "quadrant": format!("{:?}", coherence.quadrant),
            "live_intent": coherence.live_intent,
            "live_spec": coherence.live_spec,
            "live_impl": coherence.live_impl,
        },
        "fitness": {
            "total": fitness.total,
            "session_fitness_delta": session_fitness_delta,
        },
        "progress": progress_json,
        "session_age": session_age,
        "methodology": {
            "score": score.score,
            "trend": match score.trend {
                Trend::Up => "up",
                Trend::Down => "down",
                Trend::Stable => "stable",
            },
            "drift_signal": score.drift_signal,
        },
        "agent": agent_name,
        "actions": actions_json,
    });

    // META-7: Session progress in JSON — use snapshot values
    if let Some(start_fitness) = snapshot.session_start_fitness {
        let start_datom_count = snapshot.session_start_datom_count;
        let session_boundary = braid_kernel::guidance::last_harvest_wall_time(store);
        let session_tasks_closed = store
            .attribute_datoms(&braid_kernel::datom::Attribute::from_keyword(":task/status"))
            .iter()
            .filter(|d| {
                d.op == braid_kernel::datom::Op::Assert
                    && d.tx.wall_time() > session_boundary
                    && matches!(&d.value, braid_kernel::datom::Value::Keyword(k) if k.contains("closed"))
            })
            .count();
        result["session_progress"] = serde_json::json!({
            "tasks_closed": session_tasks_closed,
            "datom_delta": store.len().saturating_sub(start_datom_count),
            "fitness_delta": fitness.total - start_fitness,
        });
    }

    // Trace staleness (SC-2) — use snapshot
    let ts = &snapshot.trace_status;
    result["trace_status"] = trace_staleness_json(ts);

    // Methodology gaps — use snapshot
    let ag = &snapshot.adjusted_gaps;
    if !ag.raw.is_empty() {
        let concentration_json: Vec<serde_json::Value> = ag
            .adjusted
            .concentration
            .iter()
            .map(|cs| {
                serde_json::json!({
                    "neighborhood": cs.neighborhood,
                    "trace_count": cs.trace_count,
                    "suggestion": cs.suggestion,
                })
            })
            .collect();
        result["methodology_gaps"] = serde_json::json!({
            "activity_mode": ag.mode_label(),
            "raw_gaps": {
                "crystallization": ag.raw.crystallization,
                "unanchored": ag.raw.unanchored,
                "untested": ag.raw.untested,
                "stale_witnesses": ag.raw.stale_witnesses,
                "concentration": ag.raw.concentration.len(),
                "total": ag.raw.total(),
            },
            "adjusted_gaps": {
                "crystallization": ag.adjusted.crystallization,
                "unanchored": ag.adjusted.unanchored,
                "untested": ag.adjusted.untested,
                "stale_witnesses": ag.adjusted.stale_witnesses,
                "concentration": concentration_json,
                "total": ag.total(),
            },
        });
    }

    // R(t) routing dashboard (RFL-5: verbose-only)
    if verbose {
        use braid_kernel::guidance::routing_dashboard;
        let db = routing_dashboard(store);
        let weights_obj: serde_json::Map<String, serde_json::Value> = db
            .feature_names
            .iter()
            .zip(db.weights.iter())
            .map(|(name, &w)| (name.to_string(), serde_json::json!(w)))
            .collect();
        result["routing"] = serde_json::json!({
            "mode": if db.learned { "learned" } else if db.preview { "preview" } else { "defaults" },
            "weights": weights_obj,
            "total_actions": db.total_actions,
            "actions_with_outcome": db.actions_with_outcome,
            "followed_count": db.followed_count,
            "follow_through_rate": db.follow_through_rate,
            "preview": db.preview,
            "data_points_needed": if db.preview { 50 - db.total_actions } else { 0 },
        });
    }

    // Add bilateral data if deep
    if deep {
        let history = load_trajectory(store);
        let state = run_cycle(store, &history, spectral);
        let c = &state.fitness.components;
        result["bilateral"] = serde_json::json!({
            "fitness": state.fitness.total,
            "components": {
                "validation": c.validation,
                "coverage": c.coverage,
                "drift": c.drift,
                "harvest_quality": c.harvest_quality,
                "contradiction": c.contradiction,
                "incompleteness": c.incompleteness,
                "uncertainty": c.uncertainty,
            },
            "conditions": {
                "overall": state.conditions.overall,
                "cc1": state.conditions.cc1_no_contradictions.satisfied,
                "cc2": state.conditions.cc2_impl_satisfies_spec.satisfied,
                "cc3": state.conditions.cc3_spec_approximates_intent.satisfied,
                "cc4": state.conditions.cc4_agent_agreement.satisfied,
                "cc5": state.conditions.cc5_methodology_adherence.satisfied,
            },
        });
    }

    result
}
