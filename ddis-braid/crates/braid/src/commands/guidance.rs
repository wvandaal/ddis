//! `braid guidance` — Display current methodology state, coherence, and actions.
//!
//! Default: terse action-first output (what to do next).
//! `--verbose`: full coherence metrics, methodology components, entropy.

use std::path::Path;

use braid_kernel::guidance::{
    compute_methodology_score, derive_actions, format_actions, SessionTelemetry, Trend,
};
use braid_kernel::trilateral::check_coherence_fast;

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(path: &Path, agent_name: &str, json: bool, verbose: bool) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    // Coherence report (Phi, beta_1, quadrant)
    let coherence = check_coherence_fast(&store);

    // Build methodology score from tx-count proxy telemetry
    let telemetry = SessionTelemetry::default();
    let score = compute_methodology_score(&telemetry);

    // Derive context-sensitive actions from store state
    let actions = derive_actions(&store);

    if json {
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

        let result = serde_json::json!({
            "agent": agent_name,
            "store": {
                "datom_count": store.len(),
                "entity_count": store.entity_count(),
            },
            "coherence": {
                "phi": coherence.phi,
                "beta_1": coherence.beta_1,
                "quadrant": format!("{:?}", coherence.quadrant),
                "d_is": coherence.components.d_is,
                "d_sp": coherence.components.d_sp,
                "isp_bypasses": coherence.isp_bypasses,
                "live_intent": coherence.live_intent,
                "live_spec": coherence.live_spec,
                "live_impl": coherence.live_impl,
                "entropy": {
                    "s_vn": coherence.entropy.entropy,
                    "normalized": coherence.entropy.normalized,
                    "effective_rank": coherence.entropy.effective_rank,
                },
            },
            "methodology": {
                "score": score.score,
                "trend": match score.trend {
                    Trend::Up => "up",
                    Trend::Down => "down",
                    Trend::Stable => "stable",
                },
                "drift_signal": score.drift_signal,
                "components": {
                    "transact_frequency": score.components.transact_frequency,
                    "spec_language_ratio": score.components.spec_language_ratio,
                    "query_diversity": score.components.query_diversity,
                    "harvest_quality": score.components.harvest_quality,
                },
            },
            "actions": actions_json,
        });
        return Ok(serde_json::to_string_pretty(&result).unwrap() + "\n");
    }

    if verbose {
        return run_verbose(agent_name, &coherence, &score, &actions, &store);
    }

    // Terse default: actions-first, metrics-second (≤8 lines)
    let trend_str = match score.trend {
        Trend::Up => "up",
        Trend::Down => "down",
        Trend::Stable => "stable",
    };

    let mut out = String::new();
    out.push_str(&format!(
        "phi={:.1} beta1={} {:?} | M(t)={:.2} {}{}\n",
        coherence.phi,
        coherence.beta_1,
        coherence.quadrant,
        score.score,
        trend_str,
        if score.drift_signal { " DRIFT" } else { "" },
    ));

    // Actions: compact one-liner per action (max 5)
    for (i, a) in actions.iter().take(5).enumerate() {
        let cmd = a.command.as_deref().unwrap_or("-");
        out.push_str(&format!(
            "  {}. {} {} → {}\n",
            i + 1,
            a.category,
            a.summary,
            cmd,
        ));
    }

    if actions.len() > 5 {
        out.push_str(&format!("  (+{} more, use --verbose)\n", actions.len() - 5));
    }

    Ok(out)
}

/// Full verbose output with all coherence metrics and methodology components.
fn run_verbose(
    agent_name: &str,
    coherence: &braid_kernel::trilateral::CoherenceReport,
    score: &braid_kernel::guidance::MethodologyScore,
    actions: &[braid_kernel::guidance::GuidanceAction],
    store: &braid_kernel::Store,
) -> Result<String, BraidError> {
    let mut out = String::new();

    out.push_str(&format!(
        "guidance: agent={} store={} entities={}\n",
        agent_name,
        store.len(),
        store.entity_count(),
    ));
    out.push_str(&format!(
        "coherence: phi={:.1} beta1={} quadrant={:?}\n",
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
    out.push_str(&format_actions(actions));

    Ok(out)
}
