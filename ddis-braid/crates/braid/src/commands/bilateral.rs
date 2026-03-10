//! `braid bilateral` — Run bilateral coherence verification cycle.
//!
//! Computes F(S) fitness, forward/backward scans, CC-1..CC-5,
//! convergence analysis, and optional spectral certificate.
//! Results can be persisted as datoms with --commit.

use std::path::Path;

use braid_kernel::bilateral::{
    cycle_to_datoms, format_terse, format_verbose, load_trajectory, run_cycle,
};
use braid_kernel::datom::{AgentId, ProvenanceType, TxId};
use braid_kernel::layout::TxFile;

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(
    path: &Path,
    agent_name: &str,
    spectral: bool,
    commit: bool,
    json: bool,
    verbose: bool,
) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    // Load F(S) trajectory from previous cycles
    let history = load_trajectory(&store);

    // Run the bilateral cycle
    let state = run_cycle(&store, &history, spectral || verbose);

    if json {
        let cc = &state.conditions;
        let c = &state.fitness.components;

        let mut result = serde_json::json!({
            "cycle": state.cycle_count,
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
                "unmeasured": state.fitness.unmeasured,
            },
            "conditions": {
                "overall": cc.overall,
                "cc1_no_contradictions": cc.cc1_no_contradictions.satisfied,
                "cc2_impl_satisfies_spec": cc.cc2_impl_satisfies_spec.satisfied,
                "cc3_spec_approximates_intent": cc.cc3_spec_approximates_intent.satisfied,
                "cc4_agent_agreement": cc.cc4_agent_agreement.satisfied,
                "cc5_methodology_adherence": cc.cc5_methodology_adherence.satisfied,
            },
            "scan": {
                "forward_coverage": state.scan.forward.coverage_ratio,
                "forward_gaps": state.scan.forward.gaps.len(),
                "backward_coverage": state.scan.backward.coverage_ratio,
                "backward_orphans": state.scan.backward.gaps.len(),
            },
            "convergence": {
                "is_monotonic": state.convergence.is_monotonic,
                "lyapunov_exponent": state.convergence.lyapunov_exponent,
                "convergence_rate": state.convergence.convergence_rate,
                "steps_to_target": state.convergence.steps_to_target,
                "trajectory": state.convergence.trajectory,
            },
        });

        // Add spectral certificate if computed
        if let Some(ref cert) = state.spectral {
            result["spectral"] = serde_json::json!({
                "fiedler_value": cert.fiedler_value,
                "cheeger_constant": cert.cheeger_constant,
                "spectral_gap": cert.spectral_gap,
                "total_persistence": cert.total_persistence,
                "cycle_births": cert.cycle_births,
                "mean_ricci": cert.mean_ricci,
                "min_ricci": cert.min_ricci,
                "convergence_rate_bound": cert.convergence_rate_bound,
                "mixing_time_bound": cert.mixing_time_bound,
                "renyi": {
                    "s0_hartley": cert.renyi.s0_hartley,
                    "s1_von_neumann": cert.renyi.s1_von_neumann,
                    "s2_collision": cert.renyi.s2_collision,
                    "s_inf_min": cert.renyi.s_inf_min,
                },
                "entropy_decomposition": {
                    "s_total": cert.entropy_decomposition.s_total,
                    "s_intent": cert.entropy_decomposition.s_intent,
                    "s_spec": cert.entropy_decomposition.s_spec,
                    "s_impl": cert.entropy_decomposition.s_impl,
                    "s_within": cert.entropy_decomposition.s_within,
                    "delta_boundary": cert.entropy_decomposition.delta_boundary,
                },
            });
        }

        let mut out = serde_json::to_string_pretty(&result).unwrap();
        out.push('\n');

        if commit {
            let agent = AgentId::from_name(agent_name);
            let current_wall = store
                .frontier()
                .values()
                .map(|tx| tx.wall_time())
                .max()
                .unwrap_or(0);
            let tx_id = TxId::new(current_wall + 1, 0, agent);
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
                "committed: {} datoms -> {}\n",
                datom_count,
                file_path.relative_path()
            ));
        }

        return Ok(out);
    }

    let out = if verbose {
        format_verbose(&state)
    } else {
        format_terse(&state)
    };

    let mut result = out;

    // Commit cycle results if requested
    if commit {
        let agent = AgentId::from_name(agent_name);
        let current_wall = store
            .frontier()
            .values()
            .map(|tx| tx.wall_time())
            .max()
            .unwrap_or(0);
        let tx_id = TxId::new(current_wall + 1, 0, agent);
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
        result.push_str(&format!(
            "committed: {} datoms -> {}\n",
            datom_count,
            file_path.relative_path()
        ));
    }

    Ok(result)
}
