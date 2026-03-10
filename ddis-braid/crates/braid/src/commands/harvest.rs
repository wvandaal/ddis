//! `braid harvest` — Run the harvest pipeline to detect knowledge gaps.

use std::path::Path;

use braid_kernel::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};
use braid_kernel::guidance::count_txns_since_last_harvest;
use braid_kernel::harvest::{
    candidate_to_datoms, crystallization_guard, harvest_pipeline, SessionContext,
    DEFAULT_CRYSTALLIZATION_THRESHOLD,
};
use braid_kernel::layout::TxFile;

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(
    path: &Path,
    agent_name: &str,
    task: &str,
    knowledge_raw: &[String],
    commit: bool,
    force: bool,
) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    // Check if there are new transactions to harvest
    let tx_since_harvest = count_txns_since_last_harvest(&store);
    let harvest_warning = tx_since_harvest == 0 && knowledge_raw.is_empty();

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
        agent_name: agent_name.to_string(),
        session_start_tx: TxId::new(current_wall, 0, agent),
        task_description: task.to_string(),
        session_knowledge,
    };

    let result = harvest_pipeline(&store, &context);

    let mut out = String::new();
    if harvest_warning {
        out.push_str("warning: nothing to harvest (0 transactions since last harvest)\n");
        out.push_str(
            "  hint: transact new datoms before harvesting, or provide knowledge items\n\n",
        );
    }
    out.push_str(&format!(
        "harvest: {} candidate(s)\n",
        result.candidates.len()
    ));
    out.push_str(&format!("  drift_score: {:.2}\n", result.drift_score));
    out.push_str(&format!(
        "  session_entities: {} (tx-log extraction)\n",
        result.session_entities
    ));
    out.push_str(&format!(
        "  completeness_gaps: {}\n",
        result.completeness_gaps
    ));
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
    } else {
        // Diagnostic feedback: explain why there are no candidates
        out.push_str("\ndiagnostic: no candidates found\n");
        if result.session_entities == 0 && context.session_knowledge.is_empty() {
            out.push_str(
                "  reason: no session transactions detected and no knowledge items provided\n",
            );
            out.push_str(
                "  hint: the harvest pipeline scans for datoms with tx > session_start_tx\n",
            );
            out.push_str(&format!("  session_start: wall_time={}\n", current_wall));
            out.push_str(&format!("  store_entities: {}\n", store.entity_count()));
            out.push_str(
                "  suggestion: transact new datoms via `braid transact` before harvesting,\n",
            );
            out.push_str(
                "    or provide knowledge items: `braid harvest --task T key1 val1 key2 val2`\n",
            );
        } else if result.session_entities > 0 {
            out.push_str(
                "  reason: session entities were found but all are already in the store\n",
            );
            out.push_str("  hint: harvest only proposes entities not already present\n");
        } else {
            out.push_str(
                "  reason: knowledge items provided but all correspond to existing entities\n",
            );
            out.push_str(
                "  hint: the entities already exist in the store — nothing new to harvest\n",
            );
        }
    }

    // If --commit: apply crystallization guard then persist
    if commit && !result.candidates.is_empty() {
        // Crystallization guard (INV-HARVEST-006): partition candidates by stability
        let candidates_to_commit = if force {
            // --force bypasses crystallization guard
            out.push_str("\ncrystallization: bypassed (--force)\n");
            result.candidates.clone()
        } else {
            let guard = crystallization_guard(&store, &result, DEFAULT_CRYSTALLIZATION_THRESHOLD);
            if !guard.pending.is_empty() {
                out.push_str(&format!(
                    "\ncrystallization: {} ready, {} pending (threshold={:.1})\n",
                    guard.ready.len(),
                    guard.pending.len(),
                    DEFAULT_CRYSTALLIZATION_THRESHOLD,
                ));
                for (c, score) in &guard.pending {
                    out.push_str(&format!(
                        "  pending: {:?} (stability={:.2}, needs ≥{:.1})\n",
                        c.category, score, DEFAULT_CRYSTALLIZATION_THRESHOLD,
                    ));
                }
            }
            guard.ready.into_iter().map(|(c, _)| c).collect()
        };

        if candidates_to_commit.is_empty() {
            out.push_str("nothing committed (all candidates below crystallization threshold)\n");
            out.push_str("  hint: use --force to bypass, or re-observe to increase stability\n");
            return Ok(out);
        }

        let harvest_tx_id = TxId::new(current_wall + 1, 0, agent);
        let mut all_datoms: Vec<Datom> = Vec::new();

        for candidate in &candidates_to_commit {
            let candidate_datoms = candidate_to_datoms(candidate, harvest_tx_id);
            all_datoms.extend(candidate_datoms);
        }

        // Create HarvestSession entity (INV-HARVEST-002: provenance trail)
        let safe_agent = agent_name.replace(':', "-");
        let session_ident = format!(":harvest/session-{}-{}", safe_agent, current_wall + 1);
        let session_entity = EntityId::from_ident(&session_ident);

        all_datoms.push(Datom::new(
            session_entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(session_ident.clone()),
            harvest_tx_id,
            Op::Assert,
        ));
        all_datoms.push(Datom::new(
            session_entity,
            Attribute::from_keyword(":db/doc"),
            Value::String(format!("Harvest session for task: {task}")),
            harvest_tx_id,
            Op::Assert,
        ));
        all_datoms.push(Datom::new(
            session_entity,
            Attribute::from_keyword(":harvest/agent"),
            Value::String(agent_name.to_string()),
            harvest_tx_id,
            Op::Assert,
        ));
        all_datoms.push(Datom::new(
            session_entity,
            Attribute::from_keyword(":harvest/candidate-count"),
            Value::Long(candidates_to_commit.len() as i64),
            harvest_tx_id,
            Op::Assert,
        ));
        all_datoms.push(Datom::new(
            session_entity,
            Attribute::from_keyword(":harvest/drift-score"),
            Value::Double(ordered_float::OrderedFloat(result.drift_score)),
            harvest_tx_id,
            Op::Assert,
        ));

        let tx_file = TxFile {
            tx_id: harvest_tx_id,
            agent,
            provenance: ProvenanceType::Observed,
            rationale: format!(
                "Harvest commit: {} candidates from task '{}'",
                candidates_to_commit.len(),
                task
            ),
            causal_predecessors: vec![],
            datoms: all_datoms,
        };

        let datom_count = tx_file.datoms.len();
        let file_path = layout.write_tx(&tx_file)?;

        out.push_str(&format!(
            "committed: {} datoms → {}\n",
            datom_count,
            file_path.relative_path()
        ));
        out.push_str(&format!("  harvest session: {session_ident}\n"));
    } else if commit && result.candidates.is_empty() {
        out.push_str("\nnothing to commit (no candidates)\n");
    }

    Ok(out)
}
