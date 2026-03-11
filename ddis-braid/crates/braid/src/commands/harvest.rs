//! `braid harvest` — Run the harvest pipeline to detect knowledge gaps.
//!
//! Task is auto-detected from active session, git branch, or tx rationales.

use std::path::Path;

use braid_kernel::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};
use braid_kernel::guidance::{count_txns_since_last_harvest, last_harvest_wall_time};
use braid_kernel::harvest::{
    candidate_to_datoms, crystallization_guard, harvest_pipeline, infer_task_description,
    synthesize_narrative, SessionContext, DEFAULT_CRYSTALLIZATION_THRESHOLD,
};
use braid_kernel::layout::TxFile;

use crate::error::BraidError;
use crate::git;
use crate::layout::DiskLayout;

/// Infer task description from store state and git branch.
///
/// Delegates to the kernel's `infer_task_description` for store-based signals
/// (session entity, observation body, namespace frequency), then augments
/// with CLI-specific signals (tx rationale, git branch) when the kernel
/// returns low-confidence results.
///
/// Priority: session entity (0.95) > tx rationale > observation (0.6) >
///           namespace freq (0.3) > git branch > fallback (0.1).
fn infer_task(store: &braid_kernel::Store, path: &Path) -> (String, &'static str) {
    // Try kernel multi-signal inference first
    let (kernel_task, kernel_source, kernel_confidence) = infer_task_description(store);

    // High-confidence kernel results (session entity, observation) are authoritative
    if kernel_confidence >= 0.5 {
        let label = match kernel_source.as_str() {
            "session entity" => "session",
            "recent observation" => "observation",
            _ => "kernel",
        };
        return (kernel_task, label);
    }

    // For low-confidence kernel results, try CLI-specific signals first:
    // tx rationale (not available in kernel because it's a store-internal attribute)
    let mut latest_wall = 0u64;
    let mut latest_rationale = String::new();
    for d in store.datoms() {
        if d.attribute == Attribute::from_keyword(":tx/rationale")
            && d.op == braid_kernel::datom::Op::Assert
        {
            let wall = d.tx.wall_time();
            if wall > latest_wall {
                if let Value::String(ref s) = d.value {
                    latest_wall = wall;
                    latest_rationale = s.clone();
                }
            }
        }
    }
    if !latest_rationale.is_empty() {
        return (latest_rationale, "tx rationale");
    }

    // Kernel namespace-frequency signal (confidence 0.3) beats git branch
    if kernel_confidence >= 0.2 {
        let label = match kernel_source.as_str() {
            "namespace frequency" => "namespace",
            _ => "kernel",
        };
        return (kernel_task, label);
    }

    // Fall back to git branch name
    if let Some(root) = git::detect_git_root(path) {
        if let Some(branch) = git::current_branch(&root) {
            if branch != "main" && branch != "master" {
                return (branch.replace(['-', '_'], " "), "git branch");
            }
        }
    }

    ("session work".to_string(), "default")
}

pub fn run(
    path: &Path,
    agent_name: &str,
    task_override: Option<&str>,
    knowledge_raw: &[String],
    commit: bool,
    force: bool,
) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    // Auto-detect or use explicit task
    let (task, task_source) = match task_override {
        Some(t) => (t.to_string(), "explicit"),
        None => infer_task(&store, path),
    };

    let tx_since_harvest = count_txns_since_last_harvest(&store);
    let harvest_warning = tx_since_harvest == 0 && knowledge_raw.is_empty();

    // Session boundary for harvest pipeline context
    let session_boundary = last_harvest_wall_time(&store);

    // Collect git context (graceful degradation if not in a repo)
    // Pass the session boundary timestamp — git.rs auto-detects whether
    // it's a real Unix timestamp or a legacy sequential wall_time.
    let git_ctx = git::changes_since(path, session_boundary);

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

    let context = SessionContext {
        agent,
        agent_name: agent_name.to_string(),
        session_start_tx: TxId::new(session_boundary, 0, agent),
        task_description: task.clone(),
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
    out.push_str(&format!("harvest: \"{}\" ({})\n", task, task_source));
    out.push_str(&format!(
        "  candidates: {} | drift: {:.2}\n",
        result.candidates.len(),
        result.drift_score
    ));
    out.push_str(&format!(
        "  session_entities: {} | completeness_gaps: {}\n",
        result.session_entities, result.completeness_gaps
    ));
    out.push_str(&format!(
        "  quality: {} total ({} high, {} medium, {} low)\n",
        result.quality.count,
        result.quality.high_confidence,
        result.quality.medium_confidence,
        result.quality.low_confidence,
    ));

    // Git context (if available)
    let git_line = git::format_git_context(&git_ctx);
    if !git_line.is_empty() {
        out.push_str(&git_line);
        out.push('\n');
    }

    if !result.candidates.is_empty() {
        out.push_str("\ncandidates:\n");
        for (i, c) in result.candidates.iter().enumerate() {
            out.push_str(&format!(
                "  [{}] {:?} \u{2014} {:?} (confidence: {:.2})\n      {}\n",
                i + 1,
                c.category,
                c.status,
                c.confidence,
                c.rationale,
            ));
        }
    } else {
        out.push_str("\ndiagnostic: no candidates found\n");
        if result.session_entities == 0 && context.session_knowledge.is_empty() {
            out.push_str(
                "  reason: no session transactions detected and no knowledge items provided\n",
            );
            out.push_str("  suggestion: use `braid observe` to capture knowledge, then harvest\n");
        } else if result.session_entities > 0 {
            out.push_str(
                "  reason: session entities were found but all are already in the store\n",
            );
        } else {
            out.push_str(
                "  reason: knowledge items provided but all correspond to existing entities\n",
            );
        }
    }

    // Pipe-back-to-harness: synthesis directive for the running agent (S0.2a.2)
    let narrative = synthesize_narrative(&store, &result.candidates, &task);
    if let Some(ref directive) = narrative.synthesis_directive {
        out.push('\n');
        out.push_str(directive);
    }

    // If --commit: apply crystallization guard then persist
    if commit && !result.candidates.is_empty() {
        let candidates_to_commit = if force {
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
                        "  pending: {:?} (stability={:.2}, needs \u{2265}{:.1})\n",
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

        let harvest_tx_id = super::write::next_tx_id(&store, agent);
        let mut all_datoms: Vec<Datom> = Vec::new();

        for candidate in &candidates_to_commit {
            let candidate_datoms = candidate_to_datoms(candidate, harvest_tx_id);
            all_datoms.extend(candidate_datoms);
        }

        // Create HarvestSession entity (INV-HARVEST-002: provenance trail)
        let safe_agent = agent_name.replace(':', "-");
        let session_ident = format!(
            ":harvest/session-{}-{}",
            safe_agent,
            harvest_tx_id.wall_time()
        );
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

        // Close active session if present
        let active_session = EntityId::from_ident(":session/current");
        let has_active = store
            .entity_datoms(active_session)
            .iter()
            .any(|d| d.op == Op::Assert);
        if has_active {
            all_datoms.push(Datom::new(
                active_session,
                Attribute::from_keyword(":session/status"),
                Value::Keyword(":session.status/closed".to_string()),
                harvest_tx_id,
                Op::Assert,
            ));
        }

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
            "\ncommitted: {} datoms \u{2192} {}\n",
            datom_count,
            file_path.relative_path()
        ));
        out.push_str(&format!("  harvest session: {session_ident}\n"));
        out.push_str("next session: braid seed\n");
    } else if commit && result.candidates.is_empty() {
        out.push_str("\nnothing to commit (no candidates)\n");
    }

    Ok(out)
}
