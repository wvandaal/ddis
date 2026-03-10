//! `braid harvest` — Run the harvest pipeline to detect knowledge gaps.

use std::path::Path;

use braid_kernel::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};
use braid_kernel::harvest::{candidate_to_datoms, harvest_pipeline, SessionContext};
use braid_kernel::layout::TxFile;

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(
    path: &Path,
    agent_name: &str,
    task: &str,
    knowledge_raw: &[String],
    commit: bool,
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
        agent_name: agent_name.to_string(),
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

    // If --commit: persist candidates as datoms + create HarvestSession entity
    if commit && !result.candidates.is_empty() {
        let harvest_tx_id = TxId::new(current_wall + 1, 0, agent);
        let mut all_datoms: Vec<Datom> = Vec::new();

        // Convert each candidate to datoms
        for candidate in &result.candidates {
            let candidate_datoms = candidate_to_datoms(candidate, harvest_tx_id);
            all_datoms.extend(candidate_datoms);
        }

        // Create HarvestSession entity (INV-HARVEST-002: provenance trail)
        // Sanitize agent name: replace colons with dashes for valid EDN keywords
        let safe_agent = agent_name.replace(':', "-");
        let session_ident = format!(":harvest/session-{}-{}", safe_agent, current_wall + 1);
        let session_entity = EntityId::from_ident(&session_ident);

        // :db/ident — session identity
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
            Value::Long(result.candidates.len() as i64),
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
                result.candidates.len(),
                task
            ),
            causal_predecessors: vec![],
            datoms: all_datoms,
        };

        let datom_count = tx_file.datoms.len();
        let file_path = layout.write_tx(&tx_file)?;

        out.push_str(&format!(
            "\ncommitted: {} datoms → {}\n",
            datom_count,
            file_path.relative_path()
        ));
        out.push_str(&format!("  harvest session: {session_ident}\n"));
    } else if commit && result.candidates.is_empty() {
        out.push_str("\nnothing to commit (no candidates)\n");
    }

    Ok(out)
}
