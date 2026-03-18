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
    compute_fitness, cycle_to_datoms, format_terse as bilateral_format_terse,
    format_verbose as bilateral_format_verbose, load_trajectory, run_cycle,
};
use braid_kernel::datom::{AgentId, ProvenanceType};
use braid_kernel::guidance::{
    compute_methodology_score, count_txns_since_last_harvest, derive_actions, format_actions,
    methodology_gaps, telemetry_from_store, Trend,
};
use braid_kernel::layout::TxFile;
use braid_kernel::trilateral::check_coherence_fast;

use crate::error::BraidError;
use crate::layout::DiskLayout;
use crate::output::{AgentOutput, CommandOutput};

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
        let fitness = compute_fitness(&store);
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

    // Build JSON representation (always computed — reused for --json and structured output)
    let json_value = build_json(
        path,
        &store,
        &hashes,
        tx_since_harvest,
        agent_name,
        false,
        spectral,
    );

    // Build human representation
    let human = if verbose {
        build_verbose(path, agent_name, &store, &hashes, tx_since_harvest)
    } else {
        build_terse(path, &store, &hashes, tx_since_harvest)
    };

    // Build agent representation (compact, ≤300 tokens, three-part structure)
    let agent_output = build_agent(path, &store, &hashes, tx_since_harvest);

    // If --json flag was used, return JSON as human output too (backward compat)
    if json {
        let json_str = serde_json::to_string_pretty(&json_value).unwrap() + "\n";
        return Ok(CommandOutput {
            json: json_value,
            agent: agent_output,
            human: json_str,
        });
    }

    Ok(CommandOutput {
        json: json_value,
        agent: agent_output,
        human,
    })
}

/// Build the terse dashboard string (default mode).
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
    // F(S) uses compute_fitness (no spectral analysis — fast, deterministic)
    let fitness = compute_fitness(store);

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

    let mut out = String::new();
    out.push_str(&format!(
        "store: {} ({} datoms, {} entities, {} txns)\n",
        path.display(),
        store.len(),
        store.entity_count(),
        hashes.len(),
    ));
    out.push_str(&format!(
        "coherence: F(S)={:.2} Phi={:.1} B1={} {:?} | M(t)={:.2} {}{}\n",
        fitness.total,
        coherence.phi,
        coherence.beta_1,
        coherence.quadrant,
        score.score,
        trend_str,
        if score.drift_signal { " DRIFT" } else { "" },
    ));
    out.push_str(&format!(
        "live: intent={} spec={} impl={} | agents={}\n",
        coherence.live_intent,
        coherence.live_spec,
        coherence.live_impl,
        store.frontier().len(),
    ));
    out.push_str(&format!(
        "harvest: {} tx since last{}\n",
        tx_since_harvest, harvest_tag,
    ));

    // Task summary (D4.1: INV-INTERFACE-011)
    let (open, in_progress, closed) = braid_kernel::task_counts(store);
    let total = open + in_progress + closed;
    if total > 0 {
        let ready_count = braid_kernel::compute_ready_set(store).len();
        let blocked = open - ready_count;
        let mut task_line = format!("tasks: {open} open");
        if in_progress > 0 {
            task_line.push_str(&format!(", {in_progress} active"));
        }
        task_line.push_str(&format!(
            " ({ready_count} ready, {blocked} blocked) | {closed} closed\n"
        ));
        out.push_str(&task_line);
    }

    // Methodology gaps (INV-GUIDANCE-021: unified gap dashboard)
    let gaps = methodology_gaps(store);
    if !gaps.is_empty() {
        out.push_str("\u{26a0} methodology gaps:\n");
        if gaps.crystallization > 0 {
            out.push_str(&format!(
                "  crystallization: {} uncrystallized spec IDs\n",
                gaps.crystallization
            ));
        }
        if gaps.unanchored > 0 {
            out.push_str(&format!(
                "  unanchored: {} tasks with unresolved spec refs\n",
                gaps.unanchored
            ));
        }
        if gaps.untested > 0 {
            out.push_str(&format!(
                "  untested: {} INVs with only L1 witnesses\n",
                gaps.untested
            ));
        }
        if gaps.stale_witnesses > 0 {
            out.push_str(&format!(
                "  stale: {} invalidated witnesses\n",
                gaps.stale_witnesses
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

/// Build agent-mode three-part structure (INV-OUTPUT-002, ≤300 tokens).
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
    // F(S) uses compute_fitness (no spectral analysis — fast, deterministic)
    let fitness = compute_fitness(store);

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

    // Context: what store this is about
    let context = format!(
        "store: {} ({} datoms, {} entities, {} txns)",
        path.display(),
        store.len(),
        store.entity_count(),
        hashes.len(),
    );

    // Content: coherence + F(S) + methodology + harvest + tasks
    let mut content = format!(
        "coherence: F(S)={:.2} Phi={:.1} B1={} {:?} | M(t)={:.2} {}\nharvest: {} tx since last ({})",
        fitness.total,
        coherence.phi,
        coherence.beta_1,
        coherence.quadrant,
        score.score,
        trend_str,
        tx_since_harvest,
        harvest_status,
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

    // Methodology gaps (INV-GUIDANCE-021)
    let gaps = methodology_gaps(store);
    if !gaps.is_empty() {
        content.push_str(&format!(" | gaps: {}", gaps.total()));
        if gaps.crystallization > 0 {
            content.push_str(&format!(" (cryst:{})", gaps.crystallization));
        }
        if gaps.unanchored > 0 {
            content.push_str(&format!(" (unanchored:{})", gaps.unanchored));
        }
        if gaps.untested > 0 {
            content.push_str(&format!(" (untested:{})", gaps.untested));
        }
        if gaps.stale_witnesses > 0 {
            content.push_str(&format!(" (stale:{})", gaps.stale_witnesses));
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
) -> String {
    use braid_kernel::bilateral::{
        W_CONTRADICTION, W_COVERAGE, W_DRIFT, W_HARVEST, W_INCOMPLETENESS, W_UNCERTAINTY,
        W_VALIDATION,
    };
    use braid_kernel::guidance::compute_routing_from_store;

    let coherence = check_coherence_fast(store);
    let telemetry = telemetry_from_store(store);
    let score = compute_methodology_score(&telemetry);
    let actions = derive_actions(store);
    let fitness = compute_fitness(store);

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

    // F(S) formula breakdown with component weights and values
    let c = &fitness.components;
    out.push_str(&format!(
        "F(S) = {:.2} = {:.2}*validation({:.2}) + {:.2}*coverage({:.2}) + {:.2}*drift({:.2}) + {:.2}*harvest({:.2}) + {:.2}*contradiction({:.2}) + {:.2}*incompleteness({:.2}) + {:.2}*uncertainty({:.2})\n",
        fitness.total,
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
    out.push_str(&format!(
        "methodology: M(t)={:.2} trend={}\n",
        score.score, trend_str,
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

    // Methodology gaps (INV-GUIDANCE-021: unified gap dashboard)
    let gaps = methodology_gaps(store);
    if !gaps.is_empty() {
        out.push_str(&format!("methodology gaps: {} total\n", gaps.total()));
        if gaps.crystallization > 0 {
            out.push_str(&format!(
                "  crystallization: {} uncrystallized spec IDs (run: braid spec create <ID>)\n",
                gaps.crystallization
            ));
        }
        if gaps.unanchored > 0 {
            out.push_str(&format!(
                "  unanchored: {} tasks with unresolved spec refs\n",
                gaps.unanchored
            ));
        }
        if gaps.untested > 0 {
            out.push_str(&format!(
                "  untested: {} INVs with only L1 witnesses\n",
                gaps.untested
            ));
        }
        if gaps.stale_witnesses > 0 {
            out.push_str(&format!(
                "  stale: {} invalidated witnesses\n",
                gaps.stale_witnesses
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

    // R(t) task routing (graph-based, INV-GUIDANCE-010)
    let routings = compute_routing_from_store(store);
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

/// Build JSON value with all structured data.
fn build_json(
    path: &Path,
    store: &braid_kernel::Store,
    hashes: &[String],
    tx_since_harvest: usize,
    agent_name: &str,
    deep: bool,
    spectral: bool,
) -> serde_json::Value {
    let coherence = check_coherence_fast(store);
    let telemetry = telemetry_from_store(store);
    let score = compute_methodology_score(&telemetry);
    let actions = derive_actions(store);

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

    // Methodology gaps (INV-GUIDANCE-021)
    let gaps = methodology_gaps(store);
    if !gaps.is_empty() {
        result["methodology_gaps"] = serde_json::json!({
            "crystallization": gaps.crystallization,
            "unanchored": gaps.unanchored,
            "untested": gaps.untested,
            "stale_witnesses": gaps.stale_witnesses,
            "total": gaps.total(),
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
