//! `braid harvest` — Run the harvest pipeline to detect knowledge gaps.

use std::path::Path;

use braid_kernel::datom::{AgentId, TxId, Value};
use braid_kernel::harvest::{harvest_pipeline, SessionContext};

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(
    path: &Path,
    agent_name: &str,
    task: &str,
    knowledge_raw: &[String],
) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let agent = AgentId::from_name(agent_name);

    // Parse knowledge pairs: each group of 2 strings = (key, value)
    if knowledge_raw.len() % 2 != 0 {
        return Err(BraidError::Parse(
            "knowledge items must be pairs: key value".into(),
        ));
    }

    let session_knowledge: Vec<(String, Value)> = knowledge_raw
        .chunks(2)
        .map(|chunk| (chunk[0].clone(), Value::String(chunk[1].clone())))
        .collect();

    let current_wall = store
        .frontier()
        .values()
        .map(|tx| tx.wall_time())
        .max()
        .unwrap_or(0);

    let context = SessionContext {
        agent,
        session_start_tx: TxId::new(current_wall, 0, agent),
        task_description: task.to_string(),
        session_knowledge,
    };

    let result = harvest_pipeline(&store, &context);

    let mut out = String::new();
    out.push_str(&format!(
        "harvest: {} candidate(s)\n",
        result.candidates.len()
    ));
    out.push_str(&format!("  drift_score: {:.2}\n", result.drift_score));
    out.push_str(&format!(
        "  quality: {} total ({} high, {} medium, {} low)\n",
        result.quality.count,
        result.quality.high_confidence,
        result.quality.medium_confidence,
        result.quality.low_confidence,
    ));

    if !result.candidates.is_empty() {
        out.push_str("\ncandidates:\n");
        for (i, c) in result.candidates.iter().enumerate() {
            out.push_str(&format!(
                "  [{}] {:?} — {:?} (confidence: {:.2})\n      {}\n",
                i + 1,
                c.category,
                c.status,
                c.confidence,
                c.rationale,
            ));
        }
    }

    Ok(out)
}
