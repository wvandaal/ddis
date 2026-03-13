//! `braid bilateral` — Standalone coherence scan.
//!
//! Exposes the bilateral coherence engine (bilateral.rs, 2042 LOC kernel)
//! as a dedicated CLI command. Focused alternative to `braid status --deep`.
//!
//! Modes:
//! - **Default**: F(S) score with CC-1..CC-5 pass/fail + convergence trend
//! - **Full** (`--full`): Complete F(S) breakdown + spectral certificate
//! - **History** (`--history`): Convergence trajectory over bilateral cycles
//! - **JSON** (`--json`): Machine-parseable output
//!
//! Traces to: INV-BILATERAL-001–005, spec/11-bilateral.md,
//!            ADR-BILATERAL-001–003 (bilateral as coherence engine)

use std::path::Path;

use braid_kernel::bilateral::{
    cycle_to_datoms, format_terse as bilateral_format_terse,
    format_verbose as bilateral_format_verbose, load_trajectory, run_cycle,
    W_CONTRADICTION, W_COVERAGE, W_DRIFT, W_HARVEST, W_INCOMPLETENESS, W_UNCERTAINTY,
    W_VALIDATION,
};
use braid_kernel::datom::{AgentId, ProvenanceType};
use braid_kernel::layout::TxFile;

use crate::error::BraidError;
use crate::layout::DiskLayout;
use crate::output::{AgentOutput, CommandOutput};

/// Run the bilateral coherence scan.
pub fn run(
    path: &Path,
    agent_name: &str,
    full: bool,
    spectral: bool,
    history: bool,
    _json: bool,
    commit: bool,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    // Load convergence trajectory from prior bilateral cycles
    let trajectory = load_trajectory(&store);

    // Run the bilateral cycle
    let state = run_cycle(&store, &trajectory, spectral || full);

    // ── Human output (unchanged from prior behavior) ─────────────────

    let mut human = if history {
        format_history_text(&state, &trajectory)?
    } else if full {
        bilateral_format_verbose(&state)
    } else {
        bilateral_format_terse(&state)
    };

    // Actionable next steps (human output only, not history)
    if !history {
        human.push('\n');
        human.push_str(&format_next_steps(&state));
    }

    // Commit bilateral results if requested
    if commit {
        let agent = AgentId::from_name(agent_name);
        let tx_id = super::write::next_tx_id(&store, agent);
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
        human.push_str(&format!(
            "\ncommitted: {} datoms \u{2192} {}\n",
            datom_count,
            file_path.relative_path()
        ));
    }

    // ── Structured JSON ──────────────────────────────────────────────

    let structured_json = build_structured_json(&state);

    // ── Agent output (self-explanatory computation semantics) ────────

    let agent_output = build_agent_output(&state);

    Ok(CommandOutput {
        json: structured_json,
        agent: agent_output,
        human,
    })
}

/// Build structured JSON with full semantic field names and descriptions.
fn build_structured_json(state: &braid_kernel::bilateral::BilateralState) -> serde_json::Value {
    let c = &state.fitness.components;
    let cc = &state.conditions;
    let fwd = &state.scan.forward;
    let bwd = &state.scan.backward;

    let mut result = serde_json::json!({
        "fitness": {
            "total": state.fitness.total,
            "components": {
                "validation": {
                    "score": c.validation,
                    "weight": W_VALIDATION,
                    "description": "depth-weighted witness coverage (L1=0.15, L2=0.4, L3=0.7, L4=1.0 per spec element)"
                },
                "coverage": {
                    "score": c.coverage,
                    "weight": W_COVERAGE,
                    "description": "depth-weighted impl coverage — how thoroughly impl entities trace to spec (L1=syntactic/0.15, L2=structural/0.4, L3=property/0.7, L4=formal/1.0)"
                },
                "drift": {
                    "score": c.drift,
                    "weight": W_DRIFT,
                    "description": "1 minus normalized divergence (phi/phi_max) — measures spec-impl alignment"
                },
                "harvest_quality": {
                    "score": c.harvest_quality,
                    "weight": W_HARVEST,
                    "description": "methodology adherence score M(t) — how consistently harvest/seed discipline is followed"
                },
                "contradiction": {
                    "score": c.contradiction,
                    "weight": W_CONTRADICTION,
                    "description": "1 minus contradiction ratio — fraction of spec elements free of contradictions"
                },
                "incompleteness": {
                    "score": c.incompleteness,
                    "weight": W_INCOMPLETENESS,
                    "description": "1 minus incomplete ratio — fraction of spec elements with statements, falsification, and traces-to"
                },
                "uncertainty": {
                    "score": c.uncertainty,
                    "weight": W_UNCERTAINTY,
                    "description": "1 minus mean uncertainty across spec elements with confidence markers"
                }
            },
            "unmeasured": &state.fitness.unmeasured,
        },
        "coherence": {
            "overall": cc.overall,
            "conditions": [
                {
                    "id": "CC-1",
                    "name": "no contradictions",
                    "satisfied": cc.cc1_no_contradictions.satisfied,
                    "confidence": cc.cc1_no_contradictions.confidence,
                    "evidence": cc.cc1_no_contradictions.evidence,
                    "machine_evaluable": cc.cc1_no_contradictions.machine_evaluable,
                },
                {
                    "id": "CC-2",
                    "name": "impl satisfies spec",
                    "satisfied": cc.cc2_impl_satisfies_spec.satisfied,
                    "confidence": cc.cc2_impl_satisfies_spec.confidence,
                    "evidence": cc.cc2_impl_satisfies_spec.evidence,
                    "machine_evaluable": cc.cc2_impl_satisfies_spec.machine_evaluable,
                },
                {
                    "id": "CC-3",
                    "name": "spec approximates intent",
                    "satisfied": cc.cc3_spec_approximates_intent.satisfied,
                    "confidence": cc.cc3_spec_approximates_intent.confidence,
                    "evidence": cc.cc3_spec_approximates_intent.evidence,
                    "machine_evaluable": cc.cc3_spec_approximates_intent.machine_evaluable,
                },
                {
                    "id": "CC-4",
                    "name": "agent agreement",
                    "satisfied": cc.cc4_agent_agreement.satisfied,
                    "confidence": cc.cc4_agent_agreement.confidence,
                    "evidence": cc.cc4_agent_agreement.evidence,
                    "machine_evaluable": cc.cc4_agent_agreement.machine_evaluable,
                },
                {
                    "id": "CC-5",
                    "name": "methodology adherence",
                    "satisfied": cc.cc5_methodology_adherence.satisfied,
                    "confidence": cc.cc5_methodology_adherence.confidence,
                    "evidence": cc.cc5_methodology_adherence.evidence,
                    "machine_evaluable": cc.cc5_methodology_adherence.machine_evaluable,
                },
            ],
        },
        "scan": {
            "forward": format!("{}/{}", fwd.covered.len(), fwd.covered.len() + fwd.gaps.len()),
            "forward_ratio": fwd.coverage_ratio,
            "forward_gaps": fwd.gaps.len(),
            "backward": format!("{}/{}", bwd.covered.len(), bwd.covered.len() + bwd.gaps.len()),
            "backward_ratio": bwd.coverage_ratio,
            "backward_gaps": bwd.gaps.len(),
        },
        "convergence": {
            "trajectory": state.convergence.trajectory,
            "monotonic": state.convergence.is_monotonic,
            "lyapunov_exponent": state.convergence.lyapunov_exponent,
            "convergence_rate": state.convergence.convergence_rate,
            "steps_to_target": state.convergence.steps_to_target,
        },
        "cycle_count": state.cycle_count,
    });

    // Add spectral certificate if present
    if let Some(ref spectral) = state.spectral {
        result["spectral"] = serde_json::json!({
            "fiedler_value": spectral.fiedler_value,
            "cheeger_constant": spectral.cheeger_constant,
            "spectral_gap": spectral.spectral_gap,
            "mean_ricci": spectral.mean_ricci,
            "min_ricci": spectral.min_ricci,
            "convergence_rate_bound": spectral.convergence_rate_bound,
            "mixing_time_bound": spectral.mixing_time_bound,
        });
    }

    result
}

/// Build agent output with self-explanatory computation semantics.
///
/// Each component is described by its FULL NAME and what improves it,
/// so an AI agent reading this never needs to investigate source code.
fn build_agent_output(state: &braid_kernel::bilateral::BilateralState) -> AgentOutput {
    let c = &state.fitness.components;
    let cc = &state.conditions;
    let fwd = &state.scan.forward;

    // Context line: summary with failing conditions called out
    let failing_cc: Vec<&str> = [
        (cc.cc1_no_contradictions.satisfied, "CC-1"),
        (cc.cc2_impl_satisfies_spec.satisfied, "CC-2"),
        (cc.cc3_spec_approximates_intent.satisfied, "CC-3"),
        (cc.cc4_agent_agreement.satisfied, "CC-4"),
        (cc.cc5_methodology_adherence.satisfied, "CC-5"),
    ]
    .iter()
    .filter(|(sat, _)| !sat)
    .map(|(_, name)| *name)
    .collect();

    let fwd_total = fwd.covered.len() + fwd.gaps.len();
    let cc_summary = if failing_cc.is_empty() {
        "all CC PASS".to_string()
    } else {
        format!("{} FAIL", failing_cc.join(", "))
    };
    let context = format!(
        "bilateral: F(S)={:.4}, cycle {}, {} (fwd {}/{} impl coverage)",
        state.fitness.total,
        state.cycle_count,
        cc_summary,
        fwd.covered.len(),
        fwd_total,
    );

    // Content: the human output is already good for content
    let content = format!(
        "F(S) = {total:.4} = {wv}*validation({v:.2}) + {wc}*coverage({c:.2}) + {wd}*drift({d:.2}) \
         + {wh}*harvest({h:.2}) + {wk}*contradiction({k:.2}) + {wi}*incompleteness({i:.2}) \
         + {wu}*uncertainty({u:.2})",
        total = state.fitness.total,
        wv = W_VALIDATION, v = c.validation,
        wc = W_COVERAGE, c = c.coverage,
        wd = W_DRIFT, d = c.drift,
        wh = W_HARVEST, h = c.harvest_quality,
        wk = W_CONTRADICTION, k = c.contradiction,
        wi = W_INCOMPLETENESS, i = c.incompleteness,
        wu = W_UNCERTAINTY, u = c.uncertainty,
    );

    // Footer: identify weakest component with improvement advice
    let components = [
        ("validation", c.validation, "depth-weighted witness coverage. Improve: add :spec/witnessed datoms or increase :spec/verification-depth"),
        ("coverage", c.coverage, "depth-weighted impl-to-spec tracing. Most links are L1/syntactic (15% weight). Improve: write tests naming spec elements for L2/L3. Run: braid trace --commit"),
        ("drift", c.drift, "spec-impl alignment. Improve: fix divergences shown in scan gaps"),
        ("harvest_quality", c.harvest_quality, "methodology adherence M(t). Improve: run braid harvest --commit at session end"),
        ("contradiction", c.contradiction, "spec consistency. Improve: resolve contradictions in spec elements"),
        ("incompleteness", c.incompleteness, "spec completeness (statements + falsification + traces-to). Improve: add missing spec fields"),
        ("uncertainty", c.uncertainty, "spec confidence levels. Improve: resolve uncertain spec elements"),
    ];

    let weakest = components
        .iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .unwrap();

    let footer = format!(
        "weakest: {} ({:.2}, {})",
        weakest.0, weakest.1, weakest.2,
    );

    AgentOutput {
        context,
        content,
        footer,
    }
}

/// Generate actionable next steps from bilateral state.
fn format_next_steps(state: &braid_kernel::bilateral::BilateralState) -> String {
    let mut steps = Vec::new();

    // Check CC-1: No contradictions
    if !state.conditions.cc1_no_contradictions.satisfied {
        steps.push("CC-1 FAIL: resolve contradictions. Run: braid query '[:find ?e ?v :where [?e :spec/falsification ?v]]'".to_string());
    }

    // Check CC-2: Impl satisfies spec
    if !state.conditions.cc2_impl_satisfies_spec.satisfied {
        if state.conditions.cc2_impl_satisfies_spec.evidence
            == "skipped (no :impl/implements datoms)"
        {
            steps.push(
                "CC-2 SKIP: no :impl/implements datoms yet. Add implementation traces when available."
                    .to_string(),
            );
        } else {
            steps.push(format!(
                "CC-2 FAIL: {}",
                state.conditions.cc2_impl_satisfies_spec.evidence
            ));
        }
    }

    // Check CC-5: Methodology adherence
    if !state.conditions.cc5_methodology_adherence.satisfied {
        steps.push(format!(
            "CC-5 WARN: methodology score low. {}",
            state.conditions.cc5_methodology_adherence.evidence
        ));
    }

    // Check convergence
    if !state.convergence.is_monotonic && state.convergence.trajectory.len() > 1 {
        steps.push("Convergence: non-monotonic. F(S) has regressed in recent cycles.".to_string());
    }

    // F(S) improvement suggestions
    if state.fitness.total < 0.7 {
        let c = &state.fitness.components;
        let weakest = [
            ("coverage", c.coverage),
            ("harvest_quality", c.harvest_quality),
            ("contradiction", c.contradiction),
            ("uncertainty", c.uncertainty),
            ("drift", c.drift),
        ];
        if let Some((name, score)) = weakest.iter().min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()) {
            steps.push(format!(
                "Weakest component: {} ({:.2}). Focus improvement here for highest F(S) lift.",
                name, score
            ));
        }
    }

    if steps.is_empty() {
        "Next: F(S) is healthy. Continue working.\n".to_string()
    } else {
        let mut out = String::from("Next steps:\n");
        for step in &steps {
            out.push_str(&format!("  - {step}\n"));
        }
        out
    }
}

/// Format the convergence trajectory as a text-mode sparkline.
fn format_history_text(
    state: &braid_kernel::bilateral::BilateralState,
    trajectory: &[f64],
) -> Result<String, BraidError> {
    let mut out = String::new();
    out.push_str(&format!(
        "Bilateral convergence history ({} cycles)\n\n",
        trajectory.len() + 1
    ));

    // Current value
    out.push_str(&format!(
        "Current: F(S) = {:.4} (cycle {})\n",
        state.fitness.total, state.cycle_count
    ));

    if trajectory.is_empty() {
        out.push_str(
            "No prior cycles recorded. Run `braid bilateral --commit` to start tracking.\n",
        );
        return Ok(out);
    }

    // Text sparkline of trajectory
    out.push_str("\nTrajectory:\n");
    let all_values: Vec<f64> = trajectory
        .iter()
        .copied()
        .chain(std::iter::once(state.fitness.total))
        .collect();
    let min_val = all_values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let range = (max_val - min_val).max(0.01);

    let bar_chars = [
        ' ', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}',
        '\u{2588}',
    ];

    for (i, val) in all_values.iter().enumerate() {
        let normalized = ((val - min_val) / range * 8.0).min(8.0) as usize;
        let bar_idx = normalized.min(bar_chars.len() - 1);
        let marker = if i == all_values.len() - 1 {
            " <-- current"
        } else {
            ""
        };
        out.push_str(&format!(
            "  cycle {:>3}: {:.4} {}{}\n",
            i + 1,
            val,
            bar_chars[bar_idx],
            marker,
        ));
    }

    // Convergence analysis
    out.push_str(&format!(
        "\nMonotonic: {}\n",
        if state.convergence.is_monotonic {
            "yes (Law L1 satisfied)"
        } else {
            "NO (regressions detected)"
        }
    ));
    out.push_str(&format!(
        "Lyapunov exponent: {:.4} ({})\n",
        state.convergence.lyapunov_exponent,
        if state.convergence.lyapunov_exponent > 0.0 {
            "improving"
        } else if state.convergence.lyapunov_exponent < 0.0 {
            "degrading"
        } else {
            "stable"
        }
    ));
    if let Some(steps) = state.convergence.steps_to_target {
        out.push_str(&format!("Estimated steps to F(S) >= 0.95: {}\n", steps));
    }

    Ok(out)
}

// format_json removed — structured JSON now built by build_structured_json() above,
// and rendered by CommandOutput::render(OutputMode::Json).

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_next_steps_healthy() {
        // Construct a minimal healthy bilateral state
        let state = braid_kernel::bilateral::BilateralState {
            fitness: braid_kernel::bilateral::FitnessScore {
                total: 0.85,
                components: braid_kernel::bilateral::FitnessComponents {
                    validation: 0.9,
                    coverage: 0.8,
                    drift: 0.9,
                    harvest_quality: 0.8,
                    contradiction: 1.0,
                    incompleteness: 0.7,
                    uncertainty: 0.8,
                },
                unmeasured: vec![],
            },
            scan: braid_kernel::bilateral::BilateralScan {
                forward: braid_kernel::bilateral::ScanResult {
                    covered: vec![],
                    gaps: vec![],
                    coverage_ratio: 0.9,
                },
                backward: braid_kernel::bilateral::ScanResult {
                    covered: vec![],
                    gaps: vec![],
                    coverage_ratio: 0.8,
                },
            },
            conditions: braid_kernel::bilateral::CoherenceConditions {
                overall: true,
                cc1_no_contradictions: braid_kernel::bilateral::ConditionResult {
                    satisfied: true,
                    confidence: 1.0,
                    evidence: String::new(),
                    machine_evaluable: true,
                },
                cc2_impl_satisfies_spec: braid_kernel::bilateral::ConditionResult {
                    satisfied: true,
                    confidence: 1.0,
                    evidence: String::new(),
                    machine_evaluable: true,
                },
                cc3_spec_approximates_intent: braid_kernel::bilateral::ConditionResult {
                    satisfied: true,
                    confidence: 0.8,
                    evidence: String::new(),
                    machine_evaluable: false,
                },
                cc4_agent_agreement: braid_kernel::bilateral::ConditionResult {
                    satisfied: true,
                    confidence: 1.0,
                    evidence: String::new(),
                    machine_evaluable: true,
                },
                cc5_methodology_adherence: braid_kernel::bilateral::ConditionResult {
                    satisfied: true,
                    confidence: 0.9,
                    evidence: String::new(),
                    machine_evaluable: true,
                },
            },
            spectral: None,
            convergence: braid_kernel::bilateral::ConvergenceAnalysis {
                trajectory: vec![0.7, 0.75, 0.8, 0.85],
                is_monotonic: true,
                lyapunov_exponent: 0.05,
                steps_to_target: Some(5),
                convergence_rate: 0.95,
            },
            cycle_count: 5,
        };

        let steps = format_next_steps(&state);
        assert!(
            steps.contains("healthy"),
            "Healthy state should say 'healthy'"
        );
    }
}
