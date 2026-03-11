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
};
use braid_kernel::datom::{AgentId, ProvenanceType};
use braid_kernel::layout::TxFile;

use crate::error::BraidError;
use crate::layout::DiskLayout;

/// Run the bilateral coherence scan.
pub fn run(
    path: &Path,
    agent_name: &str,
    full: bool,
    spectral: bool,
    history: bool,
    json: bool,
    commit: bool,
) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    // Load convergence trajectory from prior bilateral cycles
    let trajectory = load_trajectory(&store);

    // Run the bilateral cycle
    let state = run_cycle(&store, &trajectory, spectral || full);

    if json {
        return format_json(&state);
    }

    if history {
        return format_history(&state, &trajectory);
    }

    // Default or full text output
    let mut out = if full {
        bilateral_format_verbose(&state)
    } else {
        bilateral_format_terse(&state)
    };

    // Actionable next steps
    out.push('\n');
    out.push_str(&format_next_steps(&state));

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
        out.push_str(&format!(
            "\ncommitted: {} datoms \u{2192} {}\n",
            datom_count,
            file_path.relative_path()
        ));
    }

    Ok(out)
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
fn format_history(
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

/// Format bilateral state as JSON.
fn format_json(state: &braid_kernel::bilateral::BilateralState) -> Result<String, BraidError> {
    let c = &state.fitness.components;
    let cc = &state.conditions;

    let mut result = serde_json::json!({
        "fitness": {
            "total": state.fitness.total,
            "components": {
                "validation": c.validation,
                "coverage": c.coverage,
                "drift": c.drift,
                "harvest_quality": c.harvest_quality,
                "contradiction": c.contradiction,
                "incompleteness": c.incompleteness,
                "uncertainty": c.uncertainty,
            },
        },
        "conditions": {
            "overall": cc.overall,
            "cc1_no_contradictions": {
                "satisfied": cc.cc1_no_contradictions.satisfied,
                "detail": cc.cc1_no_contradictions.evidence,
            },
            "cc2_impl_satisfies_spec": {
                "satisfied": cc.cc2_impl_satisfies_spec.satisfied,
                "detail": cc.cc2_impl_satisfies_spec.evidence,
            },
            "cc3_spec_approximates_intent": {
                "satisfied": cc.cc3_spec_approximates_intent.satisfied,
                "detail": cc.cc3_spec_approximates_intent.evidence,
            },
            "cc4_agent_agreement": {
                "satisfied": cc.cc4_agent_agreement.satisfied,
                "detail": cc.cc4_agent_agreement.evidence,
            },
            "cc5_methodology_adherence": {
                "satisfied": cc.cc5_methodology_adherence.satisfied,
                "detail": cc.cc5_methodology_adherence.evidence,
            },
        },
        "convergence": {
            "trajectory": state.convergence.trajectory,
            "is_monotonic": state.convergence.is_monotonic,
            "lyapunov_exponent": state.convergence.lyapunov_exponent,
            "convergence_rate": state.convergence.convergence_rate,
            "steps_to_target": state.convergence.steps_to_target,
        },
        "cycle_count": state.cycle_count,
        "scan": {
            "forward_coverage": state.scan.forward.coverage_ratio,
            "backward_coverage": state.scan.backward.coverage_ratio,
            "forward_gaps": state.scan.forward.gaps.len(),
            "backward_gaps": state.scan.backward.gaps.len(),
        },
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

    Ok(serde_json::to_string_pretty(&result).unwrap() + "\n")
}

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
