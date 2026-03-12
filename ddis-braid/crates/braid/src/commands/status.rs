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
use braid_kernel::datom::{AgentId, ProvenanceType};
use braid_kernel::guidance::{
    compute_methodology_score, count_txns_since_last_harvest, derive_actions, format_actions,
    telemetry_from_store, Trend,
};
use braid_kernel::layout::TxFile;
use braid_kernel::trilateral::check_coherence_fast;

use crate::error::BraidError;
use crate::layout::DiskLayout;

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
) -> Result<String, BraidError> {
    // Verify mode: just check integrity
    if verify {
        let layout = DiskLayout::open(path)?;
        let report = layout.verify_integrity()?;
        if report.is_clean() {
            return Ok(format!(
                "integrity: OK ({} files verified)\n",
                report.verified
            ));
        } else {
            return Ok(format!(
                "integrity: FAILED ({} corrupted, {} orphaned out of {})\n",
                report.corrupted.len(),
                report.orphaned.len(),
                report.total_files,
            ));
        }
    }

    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;
    let hashes = layout.list_tx_hashes()?;
    let tx_since_harvest = count_txns_since_last_harvest(&store);

    if json {
        return run_json(
            path,
            &store,
            &hashes,
            tx_since_harvest,
            agent_name,
            deep,
            spectral,
        );
    }

    // Deep mode: bilateral F(S) + optional graph analytics
    if deep {
        return run_deep(path, &store, agent_name, spectral, full, commit);
    }

    if verbose {
        return run_verbose(path, agent_name, &store, &hashes, tx_since_harvest);
    }

    // ── Terse default: 6-line dashboard ──────────────────────────────────────
    let coherence = check_coherence_fast(&store);
    let telemetry = telemetry_from_store(&store);
    let score = compute_methodology_score(&telemetry);
    let actions = derive_actions(&store);

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
        "coherence: Phi={:.1} B1={} {:?} | M(t)={:.2} {}{}\n",
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
    let (open, in_progress, closed) = braid_kernel::task_counts(&store);
    let total = open + in_progress + closed;
    if total > 0 {
        let ready_count = braid_kernel::compute_ready_set(&store).len();
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

    Ok(out)
}

/// Full verbose output with all metrics and actions.
fn run_verbose(
    path: &Path,
    agent_name: &str,
    store: &braid_kernel::Store,
    hashes: &[String],
    tx_since_harvest: usize,
) -> Result<String, BraidError> {
    let coherence = check_coherence_fast(store);
    let telemetry = telemetry_from_store(store);
    let score = compute_methodology_score(&telemetry);
    let actions = derive_actions(store);

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
    out.push_str(&format!(
        "methodology: M(t)={:.2} trend={}\n",
        score.score,
        match score.trend {
            Trend::Up => "up",
            Trend::Down => "down",
            Trend::Stable => "stable",
        }
    ));
    if score.drift_signal {
        out.push_str("  WARNING: drift signal active (M(t) < 0.5)\n");
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

    // Frontier
    out.push_str("frontier:\n");
    for (agent, tx_id) in store.frontier() {
        out.push_str(&format!("  {:?}: wall={}\n", agent, tx_id.wall_time()));
    }

    // All actions
    out.push_str(&format_actions(&actions));

    Ok(out)
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

/// JSON output with all structured data.
fn run_json(
    path: &Path,
    store: &braid_kernel::Store,
    hashes: &[String],
    tx_since_harvest: usize,
    agent_name: &str,
    deep: bool,
    spectral: bool,
) -> Result<String, BraidError> {
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

    Ok(serde_json::to_string_pretty(&result).unwrap() + "\n")
}
