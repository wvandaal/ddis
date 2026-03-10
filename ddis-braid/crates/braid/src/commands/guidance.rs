//! `braid guidance` — Display current methodology state and warnings.

use std::path::Path;

use braid_kernel::guidance::{
    build_footer, compute_methodology_score, format_footer, SessionTelemetry, Trend,
};
use braid_kernel::trilateral::check_coherence;

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(path: &Path, agent_name: &str) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    // Coherence report (Φ, β₁, quadrant)
    let coherence = check_coherence(&store);

    // Build methodology score from minimal telemetry
    // (a real session would accumulate this; here we provide defaults)
    let telemetry = SessionTelemetry {
        total_turns: 0,
        transact_turns: 0,
        spec_language_turns: 0,
        query_type_count: 0,
        harvest_quality: 0.0,
        history: vec![],
    };
    let score = compute_methodology_score(&telemetry);

    // Build guidance footer
    let footer = build_footer(&telemetry, &store, None, vec![]);

    let mut out = String::new();
    out.push_str("guidance state:\n");
    out.push_str(&format!("  agent: {agent_name}\n"));
    out.push_str(&format!("  divergence (Φ): {:.4}\n", coherence.phi));
    out.push_str(&format!(
        "    D_IS (intent↔spec): {}\n",
        coherence.components.d_is
    ));
    out.push_str(&format!(
        "    D_SP (spec↔impl):   {}\n",
        coherence.components.d_sp
    ));
    out.push_str(&format!(
        "  coherence: {:?} (β₁={})\n",
        coherence.quadrant, coherence.beta_1
    ));
    out.push_str(&format!(
        "  methodology score: {:.2} (trend: {})\n",
        score.score,
        match score.trend {
            Trend::Up => "up",
            Trend::Down => "down",
            Trend::Stable => "stable",
        }
    ));
    out.push_str(&format!(
        "    transact_frequency: {:.2}\n",
        score.components.transact_frequency
    ));
    out.push_str(&format!(
        "    spec_language_ratio: {:.2}\n",
        score.components.spec_language_ratio
    ));
    out.push_str(&format!(
        "    query_diversity: {:.2}\n",
        score.components.query_diversity
    ));
    out.push_str(&format!(
        "    harvest_quality: {:.2}\n",
        score.components.harvest_quality
    ));
    out.push_str(&format!(
        "  drift signal: {}\n",
        if score.drift_signal { "YES" } else { "no" }
    ));
    out.push_str("\nguidance footer:\n");
    out.push_str(&format_footer(&footer));
    out.push('\n');

    Ok(out)
}
