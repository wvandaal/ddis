//! `braid harvest` — Run the harvest pipeline to detect knowledge gaps.
//!
//! Task is auto-detected from active session, git branch, or tx rationales.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use braid_kernel::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};
use braid_kernel::guidance::{
    compute_routing_from_store, count_txns_since_last_harvest, last_harvest_wall_time,
};
use braid_kernel::harvest::{
    candidate_to_datoms, crystallization_guard, harvest_pipeline, infer_task_description,
    synthesize_narrative, SessionContext, DEFAULT_CRYSTALLIZATION_THRESHOLD,
};
use braid_kernel::layout::TxFile;

use crate::error::BraidError;
use crate::git;
use crate::layout::DiskLayout;
use crate::output::{AgentOutput, CommandOutput};

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

    // High-confidence kernel results (session entity, observation) are authoritative,
    // but session entities older than 2 hours are stale — use a generic label instead.
    if kernel_confidence >= 0.5 {
        if kernel_source == "session entity" {
            // Check if the session task is stale (> 7200s old)
            let latest_session_wall = store
                .datoms()
                .filter(|d| {
                    d.attribute == Attribute::from_keyword(":session/task")
                        && d.op == braid_kernel::datom::Op::Assert
                })
                .map(|d| d.tx.wall_time())
                .max()
                .unwrap_or(0);
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            if now_ms.saturating_sub(latest_session_wall) > 7_200_000 {
                let tx_count = count_txns_since_last_harvest(store);
                return (
                    format!("unsessioned harvest ({tx_count} txns since last harvest)"),
                    "default",
                );
            }
            return (kernel_task, "session");
        }
        let label = match kernel_source.as_str() {
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
    no_reconcile: bool,
) -> Result<CommandOutput, BraidError> {
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
    if !knowledge_raw.len().is_multiple_of(2) {
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
        // WP7: Progressive disclosure — filter display by confidence floor
        let confidence_floor: f64 =
            braid_kernel::config::get_config(&store, "harvest.confidence-floor")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.3);

        let (visible, filtered): (Vec<_>, Vec<_>) = result
            .candidates
            .iter()
            .partition(|c| c.confidence >= confidence_floor);

        out.push_str("\ncandidates:\n");
        for (i, c) in visible.iter().enumerate() {
            out.push_str(&format!(
                "  [{}] {:?} \u{2014} {:?} (confidence: {:.2})\n      {}\n",
                i + 1,
                c.category,
                c.status,
                c.confidence,
                c.rationale,
            ));
        }
        if !filtered.is_empty() {
            out.push_str(&format!(
                "  ({} below threshold {:.1}, use --verbose for details)\n",
                filtered.len(),
                confidence_floor,
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

    // T5-2 + META-4: Task audit + harvest-integrated reconciliation (INV-TASK-006)
    // Run audit, display results, and auto-close tasks above confidence threshold.
    let reconciliation_datoms = {
        let mut audit_results = braid_kernel::task::audit_tasks_from_store(&store);
        audit_results.sort_by(|a, b| {
            b.1.confidence
                .partial_cmp(&a.1.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut reconcile_datoms: Vec<Datom> = Vec::new();
        let reconcile_threshold = 0.7;

        if !audit_results.is_empty() {
            // Split into auto-closeable (>= threshold) and display-only (< threshold)
            let (auto_close, display_only): (Vec<_>, Vec<_>) = audit_results
                .iter()
                .partition(|(_, e)| e.confidence >= reconcile_threshold);

            // META-4: Auto-close high-confidence tasks if reconciliation enabled
            if commit && !no_reconcile && !auto_close.is_empty() {
                let recon_agent = AgentId::from_name(agent_name);
                let recon_tx = super::write::next_tx_id(&store, recon_agent);
                for (task, evidence) in &auto_close {
                    let attest = format!(
                        "Auto-closed by harvest reconciliation: spec_coverage={}/{}, criteria_confidence={:.0}%",
                        evidence.spec_coverage,
                        evidence.spec_total,
                        evidence.criteria_confidence.unwrap_or(0.0) * 100.0,
                    );
                    let close = braid_kernel::task::close_task_datoms(task.entity, &attest, recon_tx);
                    reconcile_datoms.extend(close);

                    // Record completion method
                    reconcile_datoms.push(Datom::new(
                        task.entity,
                        Attribute::from_keyword(":task/completion-method"),
                        Value::Keyword(":task.completion/harvest-reconciliation".to_string()),
                        recon_tx,
                        Op::Assert,
                    ));
                    reconcile_datoms.push(Datom::new(
                        task.entity,
                        Attribute::from_keyword(":task/completion-evidence"),
                        Value::String(attest.clone()),
                        recon_tx,
                        Op::Assert,
                    ));
                }
                out.push_str(&format!(
                    "\nreconciled: {} tasks auto-closed (confidence >= {:.0}%)\n",
                    auto_close.len(),
                    reconcile_threshold * 100.0,
                ));
                for (task, evidence) in &auto_close {
                    let pct = (evidence.confidence * 100.0) as u32;
                    out.push_str(&format!("  [{pct:>3}%] {} closed\n", task.id));
                }
            } else if !auto_close.is_empty() {
                // Show audit results without auto-close
                out.push_str(&format!(
                    "\naudit: {} tasks appear implemented (>={:.0}% confidence)\n",
                    auto_close.len(),
                    reconcile_threshold * 100.0,
                ));
                for (task, evidence) in &auto_close {
                    let pct = (evidence.confidence * 100.0) as u32;
                    let title_display = braid_kernel::task::short_title(&task.title);
                    out.push_str(&format!("  [{pct:>3}%] {} \"{title_display}\"\n", task.id));
                }
                let close_ids: Vec<&str> =
                    auto_close.iter().map(|(t, _)| t.id.as_str()).collect();
                out.push_str(&format!(
                    "  close: braid task close {}\n",
                    close_ids.join(" ")
                ));
            }

            // Display lower-confidence results as hints
            if !display_only.is_empty() {
                let display_slice: Vec<_> = display_only.into_iter().take(3).collect();
                out.push_str(&format!(
                    "\naudit hints: {} tasks may be implemented (review needed)\n",
                    display_slice.len(),
                ));
                for (task, evidence) in display_slice {
                    let pct = (evidence.confidence * 100.0) as u32;
                    let title_display = braid_kernel::task::short_title(&task.title);
                    out.push_str(&format!("  [{pct:>3}%] {} \"{title_display}\"\n", task.id));
                }
            }
        }

        reconcile_datoms
    };

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
            return Ok(build_harvest_output(
                out,
                &task,
                task_source,
                &result,
                false,
            ));
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

        // CONTEXT-TELEMETRY: Record session metrics for cross-session analysis
        {
            let evidence = braid_kernel::budget::EvidenceVector::from_store(&store);
            let k_est = braid_kernel::budget::estimate_k_eff(&evidence);
            let tasks_closed_this_session = store
                .datoms()
                .filter(|d| {
                    d.attribute.as_str() == ":task/status"
                        && d.op == Op::Assert
                        && d.tx.wall_time() > session_boundary
                        && matches!(&d.value, Value::Keyword(k) if k.contains("closed"))
                })
                .count();

            let metrics = serde_json::json!({
                "k_eff_estimated": format!("{k_est:.2}"),
                "tx_count": evidence.tx_count_since_session,
                "wall_elapsed_s": evidence.wall_elapsed_seconds,
                "output_tokens_est": evidence.cumulative_output_estimate,
                "observations": evidence.observe_count,
                "tasks_closed": tasks_closed_this_session,
                "candidates": candidates_to_commit.len(),
            });

            all_datoms.push(Datom::new(
                session_entity,
                Attribute::from_keyword(":harvest/context-metrics"),
                Value::String(metrics.to_string()),
                harvest_tx_id,
                Op::Assert,
            ));
        }

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

        // SWS-4: Record R(t) top-3 recommendations at harvest time.
        // This enables the retrospective (SWS-5) to compare what R(t) recommended
        // vs what the agent actually did during the session.
        {
            let routing = compute_routing_from_store(&store);
            let top_ids: Vec<String> = routing
                .iter()
                .filter(|r| r.impact > 0.0)
                .take(3)
                .filter_map(|r| {
                    // Find task ID from entity
                    braid_kernel::task::all_tasks(&store)
                        .iter()
                        .find(|t| t.entity == r.entity)
                        .map(|t| t.id.clone())
                })
                .collect();
            if !top_ids.is_empty() {
                all_datoms.push(Datom::new(
                    session_entity,
                    Attribute::from_keyword(":harvest/recommended-tasks"),
                    Value::String(top_ids.join(",")),
                    harvest_tx_id,
                    Op::Assert,
                ));
            }
        }

        // META-4: Inject reconciliation datoms into harvest transaction
        all_datoms.extend(reconciliation_datoms);

        // COTX-1: Atomic session rotation — close current session and open new
        // session in the same harvest transaction. This ensures no gap between
        // sessions where observations could be lost (cotransaction principle:
        // when event A implies event B, they share a transaction).
        //
        // Step 1: Close active session if present
        if let Some(active_entity) = braid_kernel::guidance::find_active_session(&store) {
            all_datoms.push(Datom::new(
                active_entity,
                Attribute::from_keyword(":session/status"),
                Value::Keyword(":session.status/closed".to_string()),
                harvest_tx_id,
                Op::Assert,
            ));
        }

        // Step 2: Create new session entity in the same atomic transaction
        // Use milliseconds for the ident to avoid collision with the session
        // created by auto-detect (which uses seconds). This ensures the harvest
        // session is always a distinct entity even if both happen within 1 second.
        let new_wall_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let new_wall_time = new_wall_millis / 1000;
        let new_session_ident = format!(":session/s-{}", new_wall_millis);
        let new_session_entity = EntityId::from_ident(&new_session_ident);

        // Compute start-fitness for the new session (pre-harvest F(S) is correct —
        // the new session "starts" at the moment of harvest)
        // CE-4: O(1) fitness via materialized views
        let new_fitness = store.fitness();

        // CRITICAL: Include harvest datoms in count (they're in the same tx)
        let new_datom_count = store.len() + all_datoms.len();

        // ISO 8601 timestamp
        let iso_time = {
            let secs = new_wall_time;
            let days_since_epoch = secs / 86400;
            let time_of_day = secs % 86400;
            let hours = time_of_day / 3600;
            let minutes = (time_of_day % 3600) / 60;
            let seconds = time_of_day % 60;
            let (year, month, day) = braid_kernel::guidance::days_to_ymd(days_since_epoch);
            format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                year, month, day, hours, minutes, seconds
            )
        };

        // Resolve next task from synthesis directive or harvest task
        let next_task = narrative
            .synthesis_directive
            .as_deref()
            .unwrap_or(&task);

        // 9 session datoms: 8 standard + :session/task
        all_datoms.extend([
            Datom::new(
                new_session_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(new_session_ident.clone()),
                harvest_tx_id,
                Op::Assert,
            ),
            Datom::new(
                new_session_entity,
                Attribute::from_keyword(":session/started-at"),
                Value::Long(new_wall_time as i64),
                harvest_tx_id,
                Op::Assert,
            ),
            Datom::new(
                new_session_entity,
                Attribute::from_keyword(":session/start-time"),
                Value::String(iso_time),
                harvest_tx_id,
                Op::Assert,
            ),
            Datom::new(
                new_session_entity,
                Attribute::from_keyword(":session/start-fitness"),
                Value::Double(ordered_float::OrderedFloat(new_fitness.total)),
                harvest_tx_id,
                Op::Assert,
            ),
            Datom::new(
                new_session_entity,
                Attribute::from_keyword(":session/start-datom-count"),
                Value::Long(new_datom_count as i64),
                harvest_tx_id,
                Op::Assert,
            ),
            Datom::new(
                new_session_entity,
                Attribute::from_keyword(":session/agent"),
                Value::Ref(EntityId::from_ident(&format!(":agent/{}", agent_name))),
                harvest_tx_id,
                Op::Assert,
            ),
            Datom::new(
                new_session_entity,
                Attribute::from_keyword(":session/status"),
                Value::Keyword(":session.status/active".to_string()),
                harvest_tx_id,
                Op::Assert,
            ),
            Datom::new(
                new_session_entity,
                Attribute::from_keyword(":session/current"),
                Value::Ref(new_session_entity),
                harvest_tx_id,
                Op::Assert,
            ),
            Datom::new(
                new_session_entity,
                Attribute::from_keyword(":session/task"),
                Value::String(next_task.to_string()),
                harvest_tx_id,
                Op::Assert,
            ),
        ]);

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

        // T7-1: Auto-trace scan as harvest step 9.5 (INV-WITNESS-011).
        // Closes the bilateral verification loop: harvest → trace → witness → F(S).
        let source_root = path.to_path_buf();
        if let Ok(Some(trace_result)) =
            super::trace::auto_trace_scan(&layout, &store, &source_root, agent_name)
        {
            if trace_result.new_links > 0 || trace_result.new_witnesses > 0 {
                out.push_str(&format!(
                    "  trace: {} files, {} refs, +{} impl links, +{} witnesses\n",
                    trace_result.files_scanned,
                    trace_result.refs_found,
                    trace_result.new_links,
                    trace_result.new_witnesses,
                ));
            }
        }

        // T7-2: Auto-create FBW witnesses from trace data (INV-WITNESS-001, INV-WITNESS-004).
        // After trace scan produces :impl/* datoms, create FBW witnesses for spec
        // elements that now have L2+ trace links. This binds verification evidence
        // to spec elements via content-addressed triple hashes.
        let reloaded_store = layout.load_store()?;
        let auto_witness_count =
            auto_create_witnesses(&layout, &reloaded_store, agent_name, &mut out);
        // Reload again if witnesses were created (so R(t) refit sees them)
        let reloaded_store = if auto_witness_count > 0 {
            layout.load_store()?
        } else {
            reloaded_store
        };

        // RFL-6: Trigger R(t) weight refit at harvest time.
        // If we have 50+ action-outcome pairs, learn new routing weights.
        if let Some(new_weights) = braid_kernel::guidance::refit_routing_weights(&reloaded_store) {
            // Store learned weights as a :routing/weights datom
            use braid_kernel::datom::*;
            let rfl_agent = AgentId::from_name("braid:rfl");
            let rfl_tx = crate::commands::write::next_tx_id(&reloaded_store, rfl_agent);
            let weights_json = serde_json::to_string(&new_weights.to_vec()).unwrap_or_default();
            let weights_entity = EntityId::from_ident(":routing/learned-weights");
            let weights_datom = Datom::new(
                weights_entity,
                Attribute::from_keyword(":routing/weights"),
                Value::String(weights_json.clone()),
                rfl_tx,
                Op::Assert,
            );
            let rfl_tx_file = braid_kernel::layout::TxFile {
                tx_id: rfl_tx,
                agent: rfl_agent,
                provenance: ProvenanceType::Derived,
                rationale: "RFL-6: R(t) weight refit from action-outcome history".to_string(),
                causal_predecessors: vec![],
                datoms: vec![weights_datom],
            };
            if layout.write_tx(&rfl_tx_file).is_ok() {
                out.push_str(&format!(
                    "  routing: weights updated [{}]\n",
                    new_weights
                        .iter()
                        .map(|w| format!("{:.3}", w))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }

        out.push_str("next session: braid seed\n");
    } else if commit && result.candidates.is_empty() {
        out.push_str("\nnothing to commit (no candidates)\n");
    }

    let committed = commit && !result.candidates.is_empty();
    Ok(build_harvest_output(
        out,
        &task,
        task_source,
        &result,
        committed,
    ))
}

/// Build a `CommandOutput` from the harvest results.
fn build_harvest_output(
    human: String,
    task: &str,
    task_source: &str,
    result: &braid_kernel::harvest::HarvestResult,
    committed: bool,
) -> CommandOutput {
    let candidate_count = result.candidates.len();
    let committed_count = result
        .candidates
        .iter()
        .filter(|c| matches!(c.status, braid_kernel::harvest::CandidateStatus::Committed))
        .count();
    let proposed = result
        .candidates
        .iter()
        .filter(|c| {
            matches!(
                c.status,
                braid_kernel::harvest::CandidateStatus::Proposed
                    | braid_kernel::harvest::CandidateStatus::UnderReview
            )
        })
        .count();
    let rejected = candidate_count - committed_count - proposed;

    // --- ACP: Build ActionProjection (INV-BUDGET-007) ---
    let (action_cmd, action_rationale) = if committed {
        (
            "braid seed".to_string(),
            "refresh context after harvest".to_string(),
        )
    } else {
        (
            "braid harvest --commit".to_string(),
            "persist harvest candidates".to_string(),
        )
    };

    let action = braid_kernel::budget::ProjectedAction {
        command: action_cmd,
        rationale: action_rationale,
        impact: 0.4,
    };

    let mut context_blocks = Vec::new();

    // Harvest summary (System)
    context_blocks.push(braid_kernel::budget::ContextBlock {
        precedence: braid_kernel::budget::OutputPrecedence::System,
        content: format!(
            "harvest: \"{task}\" ({task_source}) | candidates: {candidate_count} ({committed_count}c/{proposed}p/{rejected}r)"
        ),
        tokens: 15,
    });

    // Drift and quality (Methodology)
    context_blocks.push(braid_kernel::budget::ContextBlock {
        precedence: braid_kernel::budget::OutputPrecedence::Methodology,
        content: format!(
            "drift={:.2} | quality: {}h/{}m/{}l | session_entities: {}",
            result.drift_score,
            result.quality.high_confidence,
            result.quality.medium_confidence,
            result.quality.low_confidence,
            result.session_entities,
        ),
        tokens: 15,
    });

    // Completeness gaps (UserRequested — shown when non-zero)
    if result.completeness_gaps > 0 {
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::UserRequested,
            content: format!("completeness_gaps: {}", result.completeness_gaps),
            tokens: 5,
        });
    }

    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: "details: braid harvest --verbose".to_string(),
    };

    // Agent output uses ACP Navigate projection
    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    // JSON output with _acp merged
    let mut json = serde_json::json!({
        "task": task,
        "task_source": task_source,
        "candidate_count": candidate_count,
        "drift_score": result.drift_score,
        "session_entities": result.session_entities,
        "quality": {
            "total": result.quality.count,
            "high": result.quality.high_confidence,
            "medium": result.quality.medium_confidence,
            "low": result.quality.low_confidence,
        },
        "committed": committed,
    });
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    CommandOutput { json, agent, human }
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

/// T7-2: Auto-create FBW witnesses from trace scan data.
///
/// For each spec element with L2+ `:impl/implements` links, create an FBW
/// witness binding (spec_hash, falsification_hash, test_body_hash). The witness
/// is challenged via the Stage 1 keyword alignment protocol.
///
/// Returns the number of new witness datoms created.
fn auto_create_witnesses(
    layout: &DiskLayout,
    store: &braid_kernel::Store,
    agent_name: &str,
    out: &mut String,
) -> usize {
    use braid_kernel::datom::*;
    use braid_kernel::witness;
    use std::collections::BTreeSet;

    let implements_attr = Attribute::from_keyword(":impl/implements");
    let depth_attr = Attribute::from_keyword(":impl/verification-depth");
    let statement_attr = Attribute::from_keyword(":element/statement");
    let falsification_attr = Attribute::from_keyword(":spec/falsification");
    let witness_traces_attr = Attribute::from_keyword(":witness/traces-to");

    // Find spec entities that already have witnesses (avoid duplicates)
    let already_witnessed: BTreeSet<EntityId> = store
        .attribute_datoms(&witness_traces_attr)
        .iter()
        .filter(|d| d.op == Op::Assert)
        .filter_map(|d| match &d.value {
            Value::Ref(e) => Some(*e),
            _ => None,
        })
        .collect();

    // Find spec entities with L2+ impl links (candidates for witnessing)
    let mut candidates: Vec<(EntityId, i64)> = Vec::new(); // (spec_entity, max_depth)
    for datom in store.attribute_datoms(&implements_attr) {
        if datom.op != Op::Assert {
            continue;
        }
        let spec_entity = match &datom.value {
            Value::Ref(e) => *e,
            _ => continue,
        };

        if already_witnessed.contains(&spec_entity) {
            continue;
        }

        // Get verification depth for this impl entity
        let impl_entity = datom.entity;
        let depth = store
            .entity_datoms(impl_entity)
            .iter()
            .filter(|d| d.attribute == depth_attr && d.op == Op::Assert)
            .filter_map(|d| match &d.value {
                Value::Long(v) => Some(*v),
                _ => None,
            })
            .max()
            .unwrap_or(1);

        if depth >= 2 {
            candidates.push((spec_entity, depth));
        }
    }

    if candidates.is_empty() {
        return 0;
    }

    // Deduplicate: keep max depth per spec entity
    let mut spec_depths: std::collections::BTreeMap<EntityId, i64> =
        std::collections::BTreeMap::new();
    for (spec_entity, depth) in &candidates {
        let entry = spec_depths.entry(*spec_entity).or_insert(0);
        if *depth > *entry {
            *entry = *depth;
        }
    }

    let agent = AgentId::from_name(agent_name);
    let tx = crate::commands::write::next_tx_id(store, agent);

    let mut all_datoms = Vec::new();
    let mut witness_count = 0usize;

    for (&spec_entity, &depth) in &spec_depths {
        let entity_datoms = store.entity_datoms(spec_entity);

        // Get spec statement text
        let statement = entity_datoms
            .iter()
            .rfind(|d| d.attribute == statement_attr && d.op == Op::Assert)
            .and_then(|d| match &d.value {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();

        if statement.is_empty() {
            continue; // No statement to hash — skip
        }

        // Get falsification condition (may be empty — Stage 1 acceptable)
        let falsification = entity_datoms
            .iter()
            .rfind(|d| d.attribute == falsification_attr && d.op == Op::Assert)
            .and_then(|d| match &d.value {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();

        // Create FBW (test_body is empty for now — will be populated when
        // trace scanner extracts test hashes in a future enhancement)
        let fbw = witness::create_fbw(
            spec_entity,
            &statement,
            &falsification,
            "", // test body — trace scanner doesn't provide this yet
            "", // test file
            depth,
            agent_name,
        );

        // Run challenge protocol
        let (verdict, _results) = witness::challenge_witness(
            "", // test body not available yet
            &falsification,
            depth,
        );

        // Set status based on challenge result
        let mut fbw = fbw;
        fbw.verdict = verdict;
        fbw.challenge_count = 1;
        fbw.status = if !falsification.is_empty() && verdict == witness::WitnessVerdict::Confirmed {
            witness::WitnessStatus::Valid
        } else {
            witness::WitnessStatus::Pending
        };

        let datoms = witness::fbw_to_datoms(&fbw, tx);
        all_datoms.extend(datoms);
        witness_count += 1;
    }

    if all_datoms.is_empty() {
        return 0;
    }

    let tx_file = braid_kernel::layout::TxFile {
        tx_id: tx,
        agent,
        provenance: ProvenanceType::Derived,
        rationale: format!(
            "T7-2 auto-witness: {} FBW witnesses from trace evidence",
            witness_count,
        ),
        causal_predecessors: vec![],
        datoms: all_datoms,
    };

    if layout.write_tx(&tx_file).is_ok() {
        out.push_str(&format!(
            "  witness: {} FBW witnesses created from trace evidence\n",
            witness_count,
        ));
    }

    witness_count
}
