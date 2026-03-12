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

    // D4.4: Propose closing tasks whose traces-to spec elements now have impl coverage
    let closeable = find_closeable_tasks(&store);
    if !closeable.is_empty() {
        out.push_str(&format!(
            "\ntask proposals ({} potentially closeable):\n",
            closeable.len()
        ));
        for (id, title, covered, total) in &closeable {
            out.push_str(&format!(
                "  {id} \"{title}\" — {covered}/{total} traced specs implemented\n"
            ));
        }
        out.push_str("  close with: braid task close <id> --reason \"impl complete\"\n");
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

        // Persist NarrativeSummary fields as datoms (Wave 2.2: A1 fix)
        // Task description
        all_datoms.push(Datom::new(
            session_entity,
            Attribute::from_keyword(":harvest/task"),
            Value::String(task.clone()),
            harvest_tx_id,
            Op::Assert,
        ));
        // Accomplishments: join summaries with newlines
        if !narrative.accomplished.is_empty() {
            let text = narrative
                .accomplished
                .iter()
                .map(|a| a.summary.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            all_datoms.push(Datom::new(
                session_entity,
                Attribute::from_keyword(":harvest/accomplishments"),
                Value::String(text),
                harvest_tx_id,
                Op::Assert,
            ));
        }
        // Decisions: preserve rationale in serialized form
        if !narrative.decisions.is_empty() {
            let text = narrative
                .decisions
                .iter()
                .map(|d| {
                    if d.rationale.is_empty() {
                        d.summary.clone()
                    } else {
                        format!("{} (rationale: {})", d.summary, d.rationale)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            all_datoms.push(Datom::new(
                session_entity,
                Attribute::from_keyword(":harvest/decisions"),
                Value::String(text),
                harvest_tx_id,
                Op::Assert,
            ));
        }
        // Open questions: prefix with [?]
        if !narrative.open_questions.is_empty() {
            let text = narrative
                .open_questions
                .iter()
                .map(|q| format!("[?] {}", q.summary))
                .collect::<Vec<_>>()
                .join("\n");
            all_datoms.push(Datom::new(
                session_entity,
                Attribute::from_keyword(":harvest/open-questions"),
                Value::String(text),
                harvest_tx_id,
                Op::Assert,
            ));
        }
        // Synthesis directive
        if let Some(ref directive) = narrative.synthesis_directive {
            all_datoms.push(Datom::new(
                session_entity,
                Attribute::from_keyword(":harvest/synthesis-directive"),
                Value::String(directive.clone()),
                harvest_tx_id,
                Op::Assert,
            ));
        }
        // Git context summary (Wave 5: A2 fix + top modified files)
        let git_summary_text = if !git_ctx.commits.is_empty() {
            let mut lines = vec![format!(
                "branch={}, {} commits, {} files (+{}/-{})",
                git_ctx.branch.as_deref().unwrap_or("?"),
                git_ctx.commits.len(),
                git_ctx.files_changed,
                git_ctx.insertions,
                git_ctx.deletions,
            )];
            for c in git_ctx.commits.iter().take(5) {
                lines.push(format!("  {} {}", c.hash, c.subject));
            }
            // Top modified files — gives incoming agent a codebase map
            let top_files = git::top_modified_files(path, session_boundary, 8);
            if !top_files.is_empty() {
                lines.push("Hot files:".to_string());
                for (f, changes) in &top_files {
                    lines.push(format!("  {f} ({changes} lines changed)"));
                }
            }
            Some(lines.join("\n"))
        } else {
            None
        };
        if let Some(ref git_text) = git_summary_text {
            all_datoms.push(Datom::new(
                session_entity,
                Attribute::from_keyword(":harvest/git-summary"),
                Value::String(git_text.clone()),
                harvest_tx_id,
                Op::Assert,
            ));
        }

        // Store metrics: datom count at harvest time (enables session delta tracking)
        all_datoms.push(Datom::new(
            session_entity,
            Attribute::from_keyword(":harvest/store-datom-count"),
            Value::Long(store.len() as i64),
            harvest_tx_id,
            Op::Assert,
        ));
        all_datoms.push(Datom::new(
            session_entity,
            Attribute::from_keyword(":harvest/store-entity-count"),
            Value::Long(store.entity_count() as i64),
            harvest_tx_id,
            Op::Assert,
        ));

        // Codebase snapshot — gives incoming agents a project map
        if let Some(snapshot) = git::codebase_snapshot(path) {
            all_datoms.push(Datom::new(
                session_entity,
                Attribute::from_keyword(":harvest/codebase-snapshot"),
                Value::String(snapshot),
                harvest_tx_id,
                Op::Assert,
            ));
        }

        // E2: Session metrics — transactions and observations since last harvest
        all_datoms.push(Datom::new(
            session_entity,
            Attribute::from_keyword(":harvest/tx-since-last"),
            Value::Long(tx_since_harvest as i64),
            harvest_tx_id,
            Op::Assert,
        ));
        // Count observations in this session window
        let obs_count = store
            .datoms()
            .filter(|d| {
                d.attribute.as_str() == ":exploration/source"
                    && d.op == Op::Assert
                    && d.tx.wall_time() > session_boundary
            })
            .count();
        all_datoms.push(Datom::new(
            session_entity,
            Attribute::from_keyword(":harvest/observation-count"),
            Value::Long(obs_count as i64),
            harvest_tx_id,
            Op::Assert,
        ));

        // Session delta: how much the store grew since last harvest.
        // Incoming agents can see "last session added 126 datoms, 23 entities"
        // which contextualizes the rate of progress.
        {
            let prev_datoms = store
                .datoms()
                .filter(|d| {
                    d.attribute.as_str() == ":harvest/store-datom-count"
                        && d.op == Op::Assert
                        && d.tx.wall_time() < harvest_tx_id.wall_time()
                })
                .filter_map(|d| match d.value {
                    Value::Long(n) => Some(n),
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            let prev_entities = store
                .datoms()
                .filter(|d| {
                    d.attribute.as_str() == ":harvest/store-entity-count"
                        && d.op == Op::Assert
                        && d.tx.wall_time() < harvest_tx_id.wall_time()
                })
                .filter_map(|d| match d.value {
                    Value::Long(n) => Some(n),
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            let delta_datoms = store.len() as i64 - prev_datoms;
            let delta_entities = store.entity_count() as i64 - prev_entities;
            if prev_datoms > 0 {
                all_datoms.push(Datom::new(
                    session_entity,
                    Attribute::from_keyword(":harvest/delta-summary"),
                    Value::String(format!(
                        "+{} datoms, +{} entities",
                        delta_datoms, delta_entities
                    )),
                    harvest_tx_id,
                    Op::Assert,
                ));
            }
        }

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

/// Find open tasks whose :task/traces-to spec elements all have :impl/implements links.
///
/// Returns Vec<(task_id, title, covered_count, total_traces)> for tasks where
/// covered_count == total_traces (all traced specs are implemented).
fn find_closeable_tasks(store: &braid_kernel::Store) -> Vec<(String, String, usize, usize)> {
    use braid_kernel::datom::Op;
    use braid_kernel::task::{all_tasks, TaskStatus};

    let impl_attr = braid_kernel::datom::Attribute::from_keyword(":impl/implements");

    // Build set of spec entities that have :impl/implements links
    let mut implemented: std::collections::HashSet<braid_kernel::datom::EntityId> =
        std::collections::HashSet::new();
    for d in store.datoms() {
        if d.attribute == impl_attr && d.op == Op::Assert {
            if let braid_kernel::datom::Value::Ref(spec) = &d.value {
                implemented.insert(*spec);
            }
        }
    }

    let mut closeable = Vec::new();
    for task in all_tasks(store) {
        // Only open/in-progress tasks with traces-to refs
        if task.status == TaskStatus::Closed || task.traces_to.is_empty() {
            continue;
        }
        let total = task.traces_to.len();
        let covered = task
            .traces_to
            .iter()
            .filter(|e| implemented.contains(e))
            .count();
        if covered == total {
            closeable.push((task.id, task.title, covered, total));
        }
    }

    closeable
}
