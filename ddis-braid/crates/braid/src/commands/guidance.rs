//! `braid guidance` — Display current methodology state, coherence, and actions.
//!
//! Shows diagnostics AND concrete next steps. Output is structured for LLM parsing:
//! sections are labeled, actions include runnable commands, and metrics are numeric.

use std::path::Path;

use braid_kernel::guidance::{
    compute_methodology_score, derive_actions, format_actions, SessionTelemetry, Trend,
};
use braid_kernel::trilateral::check_coherence_fast;

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(path: &Path, agent_name: &str) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    // Coherence report (Φ, β₁, quadrant)
    let coherence = check_coherence_fast(&store);

    // Build methodology score from tx-count proxy telemetry
    // Stage 0: telemetry defaults to zero (each CLI invocation is a separate process)
    // The tx-count proxy provides harvest urgency via derive_actions()
    let telemetry = SessionTelemetry::default();
    let score = compute_methodology_score(&telemetry);

    // Derive context-sensitive actions from store state
    let actions = derive_actions(&store);

    let mut out = String::new();

    // Section 1: Store summary (terse)
    out.push_str(&format!(
        "guidance: agent={} store={} entities={}\n",
        agent_name,
        store.len(),
        store.entity_count(),
    ));

    // Section 2: Coherence metrics
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

    // Section 3: Methodology score
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

    // Section 4: Actions (the key deliverable)
    out.push_str(&format_actions(&actions));

    Ok(out)
}
