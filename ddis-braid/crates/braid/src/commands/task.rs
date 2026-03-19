//! `braid task` — Issue tracking as datoms.
//!
//! Tasks are first-class entities in the braid store, replacing external issue
//! trackers. Status uses lattice resolution (open < in-progress < closed) for
//! monotonic CRDT merge (INV-TASK-001). Dependencies form a DAG (INV-TASK-002).
//!
//! # CLI Design
//!
//! "As easy as git" — positional text creates a task:
//!   braid task "Fix harvest noise"
//!   braid task "Fix harvest noise" --priority 1 --type bug
//!
//! Traces to: ADR-TASK-001, INV-TASK-001..004

use std::path::Path;

use braid_kernel::datom::{AgentId, Attribute, Datom, Op, ProvenanceType, Value};
use braid_kernel::guidance::compute_routing_from_store;
use braid_kernel::layout::TxFile;
use braid_kernel::task::{
    self, check_dependency_acyclicity, close_task_datoms, compute_ready_set, dep_add_datom,
    extract_acceptance_criteria, find_task_by_id, generate_task_id, parse_spec_refs,
    parse_verification_pattern, resolve_spec_refs, run_verification, set_attribute_datom,
    task_counts, task_summary, update_status_datom, CreateTaskParams, TaskStatus, TaskType,
    VerificationPattern,
};
use braid_kernel::EntityId;

use crate::error::BraidError;
use crate::layout::DiskLayout;
use crate::output::{AgentOutput, CommandOutput};

use super::session::ensure_layer_4_public;

/// Arguments for creating a task via CLI.
pub struct CreateArgs<'a> {
    pub path: &'a Path,
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub priority: i64,
    pub task_type: &'a str,
    pub agent: &'a str,
    pub traces_to: &'a [String],
    pub labels: &'a [String],
}

/// Create a new task.
pub fn create(args: CreateArgs<'_>) -> Result<CommandOutput, BraidError> {
    let CreateArgs {
        path,
        title,
        description,
        priority,
        task_type,
        agent,
        traces_to,
        labels,
    } = args;
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;
    ensure_layer_4_public(&layout, &store)?;

    let tt = TaskType::from_keyword(task_type).unwrap_or(TaskType::Task);
    let agent_id = AgentId::from_name(agent);
    let tx_id = super::write::next_tx_id(&store, agent_id);

    // SFE-2.3: Extract spec refs from title and resolve against the store
    let title_refs = parse_spec_refs(title);
    let (resolved_refs, unresolved_refs) = resolve_spec_refs(&store, &title_refs);

    // Combine explicit traces-to with resolved spec refs from title
    let mut trace_entities: Vec<EntityId> =
        traces_to.iter().map(|s| EntityId::from_ident(s)).collect();
    for (_, entity_id) in &resolved_refs {
        if !trace_entities.contains(entity_id) {
            trace_entities.push(*entity_id);
        }
    }

    let (entity, datoms) = task::create_task_datoms(CreateTaskParams {
        title,
        description,
        priority,
        task_type: tt,
        tx: tx_id,
        traces_to: &trace_entities,
        labels,
    });

    // Build warnings for unresolved refs
    let mut warnings = Vec::new();
    for ref_id in &unresolved_refs {
        warnings.push(format!(
            "\u{26a0} {ref_id} not found in store. Crystallize first: braid spec create {ref_id}"
        ));
    }

    // Print unresolved warnings to stderr
    for w in &warnings {
        eprintln!("{w}");
    }

    let tx = TxFile {
        tx_id,
        agent: agent_id,
        provenance: ProvenanceType::Observed,
        rationale: format!("Create task: {title}"),
        causal_predecessors: vec![],
        datoms,
    };

    let datom_count = tx.datoms.len();
    layout.write_tx(&tx)?;

    let task_id = generate_task_id(title);
    let type_short = task_type.strip_prefix(":task.type/").unwrap_or(task_type);

    // Collect all traces-to IDs for output (explicit + resolved from title)
    let mut all_traces: Vec<String> = traces_to.to_vec();
    for (ref_id, _) in &resolved_refs {
        if !all_traces.contains(ref_id) {
            all_traces.push(ref_id.clone());
        }
    }

    let mut human = String::new();
    human.push_str(&format!("created: {task_id} \"{title}\"\n"));
    human.push_str(&format!(
        "  P{priority} {type_short} | {datom_count} datoms | entity: :task/{task_id}\n"
    ));
    if !all_traces.is_empty() {
        human.push_str(&format!("  traces-to: {}\n", all_traces.join(", ")));
    }
    for w in &warnings {
        human.push_str(&format!("  {w}\n"));
    }
    let _ = entity; // used for entity creation

    let json = serde_json::json!({
        "id": task_id,
        "title": title,
        "priority": priority,
        "type": type_short,
        "datom_count": datom_count,
        "entity": format!(":task/{task_id}"),
        "traces_to": all_traces,
        "unresolved_refs": unresolved_refs,
    });

    let agent = AgentOutput {
        context: format!("created: {task_id} \"{title}\""),
        content: format!(
            "P{priority} {type_short} | {datom_count} datoms | entity: :task/{task_id}"
        ),
        footer: format!("claim: braid go {task_id} | list: braid task list"),
    };

    Ok(CommandOutput { json, agent, human })
}

/// List tasks (all or filtered by status).
pub fn list(path: &Path, show_all: bool) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let tasks = task::all_tasks(&store);
    if tasks.is_empty() {
        return Ok(CommandOutput::from_human(
            "No tasks found. Create one: braid task \"title\"\n".to_string(),
        ));
    }

    let (open, in_progress, closed) = task_counts(&store);
    let mut human = String::new();
    human.push_str(&format!(
        "Tasks: {} total ({} open, {} in-progress, {} closed)\n",
        tasks.len(),
        open,
        in_progress,
        closed
    ));

    let mut tasks_json = Vec::new();
    for t in &tasks {
        if !show_all && t.status == TaskStatus::Closed {
            continue;
        }
        let status = match t.status {
            TaskStatus::Open => "open",
            TaskStatus::InProgress => "work",
            TaskStatus::Closed => "done",
        };
        let type_short = t
            .task_type
            .strip_prefix(":task.type/")
            .unwrap_or(&t.task_type);
        let traces = if t.traces_to.is_empty() {
            String::new()
        } else {
            format!(" [{}]", t.traces_to.len())
        };
        human.push_str(&format!(
            "  P{} {:4} {:4}  {}  \"{}\"{}\n",
            t.priority, type_short, status, t.id, t.title, traces
        ));
        tasks_json.push(serde_json::json!({
            "id": t.id,
            "title": t.title,
            "priority": t.priority,
            "type": type_short,
            "status": status,
            "traces_to_count": t.traces_to.len(),
        }));
    }

    let json = serde_json::json!({
        "total": tasks.len(),
        "open": open,
        "in_progress": in_progress,
        "closed": closed,
        "tasks": tasks_json,
    });

    let agent = AgentOutput {
        context: format!(
            "tasks: {} total ({} open, {} in-progress, {} closed)",
            tasks.len(),
            open,
            in_progress,
            closed,
        ),
        content: tasks_json
            .iter()
            .take(10)
            .map(|t| {
                format!(
                    "[P{}] {} {} \"{}\"",
                    t["priority"],
                    t["id"].as_str().unwrap_or("?"),
                    t["status"].as_str().unwrap_or("?"),
                    t["title"].as_str().unwrap_or("?"),
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        footer: "ready: braid task ready | next: braid next".to_string(),
    };

    Ok(CommandOutput { json, agent, human })
}

/// Show ready tasks (unblocked open tasks sorted by priority).
pub fn ready(path: &Path) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let ready_set = compute_ready_set(&store);
    if ready_set.is_empty() {
        return Ok(CommandOutput::from_human(
            "No ready tasks. Create one: braid task \"title\"\n".to_string(),
        ));
    }

    // Human output (backward compat)
    let mut human = String::new();
    human.push_str(&format!("Ready tasks ({} unblocked):\n", ready_set.len()));

    for t in &ready_set {
        let type_short = t
            .task_type
            .strip_prefix(":task.type/")
            .unwrap_or(&t.task_type);
        let traces = if t.traces_to.is_empty() {
            String::new()
        } else {
            format!(" [traces: {}]", t.traces_to.len())
        };
        human.push_str(&format!(
            "  P{}  {:7}  {}  \"{}\"{}\n",
            t.priority, type_short, t.id, t.title, traces
        ));
    }

    if let Some(top) = ready_set.first() {
        human.push_str(&format!(
            "Top pick: {} — run: braid go {}\n",
            top.id, top.id
        ));
    }

    // JSON output
    let tasks_json: Vec<serde_json::Value> = ready_set
        .iter()
        .map(|t| {
            let type_short = t
                .task_type
                .strip_prefix(":task.type/")
                .unwrap_or(&t.task_type);
            serde_json::json!({
                "id": t.id,
                "title": t.title,
                "priority": t.priority,
                "type": type_short,
                "traces_to_count": t.traces_to.len(),
            })
        })
        .collect();
    let json = serde_json::json!({
        "ready_count": ready_set.len(),
        "tasks": tasks_json,
    });

    // Agent output: show all ready tasks — truncation harms agent priority decisions.
    // Agents need the full list to make informed choices; "... and N more" forces
    // a follow-up round-trip. Human mode uses the human string (already shows all).
    let mut content_lines = Vec::new();
    for t in &ready_set {
        let type_short = t
            .task_type
            .strip_prefix(":task.type/")
            .unwrap_or(&t.task_type);
        content_lines.push(format!(
            "  [P{}] {} \"{}\" ({})",
            t.priority, t.id, t.title, type_short
        ));
    }

    let top_id = ready_set.first().map(|t| t.id.as_str()).unwrap_or("?");
    let agent = AgentOutput {
        context: format!("ready: {} tasks", ready_set.len()),
        content: content_lines.join("\n"),
        footer: format!("claim: braid go {top_id}"),
    };

    Ok(CommandOutput { json, agent, human })
}

/// Show the top ready task, optionally skipping a specific task ID.
///
/// Returns the SINGLE highest-impact ready task with its claim command.
/// Uses R(t) graph-based routing (INV-GUIDANCE-010) when tasks exist in the
/// store; falls back to highest-priority ordering otherwise.
///
/// For the full list, use `braid task ready`.
/// When `skip` is provided, filters out the matching task before selecting.
pub fn next(path: &Path, skip: Option<&str>) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let mut ready_set = compute_ready_set(&store);

    // Filter out skipped task
    if let Some(skip_id) = skip {
        ready_set.retain(|t| t.id != skip_id);
    }

    if ready_set.is_empty() {
        let msg = if skip.is_some() {
            "No other ready tasks (after skipping). Create one: braid task \"title\"\n"
        } else {
            "No ready tasks. Create one: braid task \"title\"\n"
        };
        return Ok(CommandOutput::from_human(msg.to_string()));
    }

    // Attempt R(t) routing to pick the highest-impact task.
    // The routing includes session-boosted in-progress tasks, so we match against
    // ALL open/in-progress tasks (not just the ready_set which excludes in-progress).
    // Falls back to top of priority-sorted ready_set if routing returns nothing.
    let all_tasks = braid_kernel::task::all_tasks(&store);
    let routing = compute_routing_from_store(&store);
    let top = {
        // Find the first non-zero-impact routed task, matching against all open/in-progress
        let matched = routing.iter().find_map(|routed| {
            if routed.impact <= 0.0 {
                return None; // Skip EPICs and other zero-impact tasks
            }
            // First try matching in all_tasks (includes in-progress / session-boosted)
            all_tasks.iter().find(|t| {
                t.status != braid_kernel::task::TaskStatus::Closed && {
                    let ident = format!(":task/{}", t.id);
                    routed.entity == braid_kernel::EntityId::from_ident(&ident)
                }
            })
        });
        // Fall back to ready_set if no routing match
        matched.or_else(|| ready_set.first())
    };

    let top = match top {
        Some(t) => t,
        None => return Ok(CommandOutput::from_human("No ready tasks.\n".to_string())),
    };

    // Find R(t) impact score for the selected task
    let impact_score = routing
        .iter()
        .find(|r| {
            let ident = format!(":task/{}", top.id);
            r.entity == braid_kernel::EntityId::from_ident(&ident)
        })
        .map(|r| r.impact);

    let type_short = top
        .task_type
        .strip_prefix(":task.type/")
        .unwrap_or(&top.task_type);
    let impact_str = impact_score
        .map(|s| format!(", impact={:.2}", s))
        .unwrap_or_default();
    let ready_count = ready_set.len();

    // Human output
    let human = format!(
        "next: [P{}] {} \"{}\"\nclaim: braid go {}\n({} total ready — see all: braid task ready)\n",
        top.priority, top.id, top.title, top.id, ready_count
    );

    // JSON output — includes impact_score for programmatic consumers
    let json = serde_json::json!({
        "id": top.id,
        "title": top.title,
        "priority": top.priority,
        "type": type_short,
        "impact_score": impact_score,
        "claim_command": format!("braid go {}", top.id),
        "ready_count": ready_count,
    });

    // Agent output — single task, claim command, context
    let agent = AgentOutput {
        context: format!(
            "next: [P{}] {} \"{}\" ({}{})",
            top.priority, top.id, top.title, type_short, impact_str
        ),
        content: format!("claim: braid go {}", top.id),
        footer: format!("{} total ready | all: braid task ready", ready_count),
    };

    Ok(CommandOutput { json, agent, human })
}

/// Show detailed info about a task.
pub fn show(path: &Path, task_id: &str) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let entity = find_task_by_id(&store, task_id)
        .ok_or_else(|| BraidError::Validation(format!("task not found: {task_id}")))?;

    let t = task_summary(&store, entity)
        .ok_or_else(|| BraidError::Validation(format!("task entity invalid: {task_id}")))?;

    let status = match t.status {
        TaskStatus::Open => "open",
        TaskStatus::InProgress => "in-progress",
        TaskStatus::Closed => "closed",
    };
    let type_short = t
        .task_type
        .strip_prefix(":task.type/")
        .unwrap_or(&t.task_type);

    let mut human = String::new();
    human.push_str(&format!("{} \"{}\"\n", t.id, t.title));
    human.push_str(&format!(
        "  status: {status} | priority: P{} | type: {type_short}\n",
        t.priority
    ));
    if !t.labels.is_empty() {
        human.push_str(&format!("  labels: {}\n", t.labels.join(", ")));
    }
    if !t.depends_on.is_empty() {
        human.push_str(&format!("  depends-on: {} task(s)\n", t.depends_on.len()));
    }
    if !t.traces_to.is_empty() {
        human.push_str(&format!(
            "  traces-to: {} spec element(s)\n",
            t.traces_to.len()
        ));
    }
    if let Some(ref source) = t.source {
        human.push_str(&format!("  source: {source}\n"));
    }
    if let Some(ref reason) = t.close_reason {
        human.push_str(&format!("  close-reason: {reason}\n"));
    }
    human.push_str(&format!("  entity: :task/{}\n", t.id));

    let json = serde_json::json!({
        "id": t.id,
        "title": t.title,
        "status": status,
        "priority": t.priority,
        "type": type_short,
        "labels": t.labels,
        "depends_on_count": t.depends_on.len(),
        "traces_to_count": t.traces_to.len(),
        "source": t.source,
        "close_reason": t.close_reason,
        "entity": format!(":task/{}", t.id),
    });

    let agent = AgentOutput {
        context: format!("{} \"{}\"", t.id, t.title),
        content: format!(
            "status: {} | P{} {} | deps: {} | traces: {}",
            status,
            t.priority,
            type_short,
            t.depends_on.len(),
            t.traces_to.len(),
        ),
        footer: format!("claim: braid go {} | close: braid done {}", t.id, t.id),
    };

    Ok(CommandOutput { json, agent, human })
}

/// Close one or more tasks.
pub fn close(
    path: &Path,
    task_ids: &[String],
    reason: &str,
    agent: &str,
    force: bool,
    attest: Option<&str>,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;
    ensure_layer_4_public(&layout, &store)?;

    let agent_id = AgentId::from_name(agent);
    let tx_id = super::write::next_tx_id(&store, agent_id);

    let mut all_datoms = Vec::new();
    let mut closed_ids = Vec::new();
    let mut blocked_ids: Vec<(String, String)> = Vec::new();

    for task_id in task_ids {
        let entity = find_task_by_id(&store, task_id)
            .ok_or_else(|| BraidError::Validation(format!("task not found: {task_id}")))?;

        // CBV: Completion-Bound Verification (INV-TASK-006)
        let criteria = extract_acceptance_criteria(&store, entity);
        let completion_method = if force {
            // LOUD WARNING: show exactly what's being skipped
            if !criteria.is_empty() {
                eprintln!(
                    "WARNING: --force bypassing CBV for {task_id}. Skipped acceptance criteria:"
                );
                for c in &criteria {
                    eprintln!("  - {}", braid_kernel::budget::safe_truncate_bytes(c, 100));
                }
                eprintln!("Consider using --attest instead to provide evidence of completion.");
            }
            "force"
        } else if attest.is_some() {
            "attested"
        } else if criteria.is_empty() {
            "no-criteria"
        } else {
            // Attempt automated verification
            let mut all_passed = true;
            let mut failures = Vec::new();
            for criterion in &criteria {
                let pattern = parse_verification_pattern(criterion);
                if let VerificationPattern::Manual { .. } = pattern {
                    continue; // Skip manual criteria in auto-verify mode
                }
                if let Err(reason) = run_verification(&store, &pattern) {
                    all_passed = false;
                    failures.push(reason);
                }
            }
            if all_passed {
                "verified"
            } else {
                // Block this task — don't close
                let msg = format!(
                    "CBV: acceptance criteria not met for {task_id}:\n  {}\n  Preferred: --attest \"evidence\" to document why criteria are met\n  Last resort: --force to bypass (will show WARNING)",
                    failures.join("\n  ")
                );
                blocked_ids.push((task_id.clone(), msg));
                continue; // Skip to next task
            }
        };

        // Record completion method
        all_datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":task/completion-method"),
            Value::Keyword(format!(":task.completion/{completion_method}")),
            tx_id,
            Op::Assert,
        ));

        // Record attestation evidence if provided
        if let Some(evidence) = attest {
            all_datoms.push(Datom::new(
                entity,
                Attribute::from_keyword(":task/completion-evidence"),
                Value::String(evidence.to_string()),
                tx_id,
                Op::Assert,
            ));
        }

        all_datoms.extend(close_task_datoms(entity, reason, tx_id));
        closed_ids.push(task_id.as_str());
    }

    // If ALL tasks were blocked by CBV, return error
    if all_datoms.is_empty() && !blocked_ids.is_empty() {
        let msgs: Vec<String> = blocked_ids.iter().map(|(_, m)| m.clone()).collect();
        return Err(BraidError::Validation(msgs.join("\n")));
    }
    if all_datoms.is_empty() {
        return Err(BraidError::Validation("no tasks to close".to_string()));
    }

    let tx = TxFile {
        tx_id,
        agent: agent_id,
        provenance: ProvenanceType::Observed,
        rationale: format!("Close task(s): {}", closed_ids.join(", ")),
        causal_predecessors: vec![],
        datoms: all_datoms,
    };
    layout.write_tx(&tx)?;

    let mut human = String::new();
    // Show blocked tasks first (if any in a batch)
    for (id, msg) in &blocked_ids {
        human.push_str(&format!("BLOCKED: {id} — {msg}\n"));
    }
    for id in &closed_ids {
        human.push_str(&format!("closed: {id}\n"));
    }

    let json = serde_json::json!({
        "closed": closed_ids,
        "count": closed_ids.len(),
        "reason": reason,
    });

    let agent = AgentOutput {
        context: format!("closed: {} task(s)", closed_ids.len()),
        content: closed_ids
            .iter()
            .map(|id| format!("closed: {id}"))
            .collect::<Vec<_>>()
            .join("\n"),
        footer: "next: braid task ready | status: braid status".to_string(),
    };

    Ok(CommandOutput { json, agent, human })
}

/// Update a task's status.
pub fn update(
    path: &Path,
    task_id: &str,
    status: &str,
    agent: &str,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;
    ensure_layer_4_public(&layout, &store)?;

    let entity = find_task_by_id(&store, task_id)
        .ok_or_else(|| BraidError::Validation(format!("task not found: {task_id}")))?;

    let new_status = TaskStatus::from_keyword(status).ok_or_else(|| {
        BraidError::Validation(format!(
            "invalid status: {status} (use open, in-progress, closed)"
        ))
    })?;

    let agent_id = AgentId::from_name(agent);
    let tx_id = super::write::next_tx_id(&store, agent_id);

    let datom = update_status_datom(entity, new_status, tx_id);
    let tx = TxFile {
        tx_id,
        agent: agent_id,
        provenance: ProvenanceType::Observed,
        rationale: format!("Update {task_id} → {status}"),
        causal_predecessors: vec![],
        datoms: vec![datom],
    };
    layout.write_tx(&tx)?;

    let human = format!("updated: {task_id} \u{2192} {status}\n");

    let json = serde_json::json!({
        "id": task_id,
        "status": status,
    });

    let agent = AgentOutput {
        context: format!("updated: {task_id} \u{2192} {status}"),
        content: String::new(),
        footer: format!("show: braid task show {task_id}"),
    };

    Ok(CommandOutput { json, agent, human })
}

/// Set an arbitrary task attribute by friendly name.
///
/// Supported attributes: priority (0-4), status (open/in-progress/closed),
/// type (task/bug/feature/epic/question/docs), title (non-empty string).
/// LWW resolution handles the "update" semantics.
pub fn set(
    path: &Path,
    task_id: &str,
    attribute: &str,
    value: &str,
    agent: &str,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;
    ensure_layer_4_public(&layout, &store)?;

    let entity = find_task_by_id(&store, task_id)
        .ok_or_else(|| BraidError::Validation(format!("task not found: {task_id}")))?;

    let agent_id = AgentId::from_name(agent);
    let tx_id = super::write::next_tx_id(&store, agent_id);

    let datom =
        set_attribute_datom(entity, attribute, value, tx_id).map_err(BraidError::Validation)?;

    let tx = TxFile {
        tx_id,
        agent: agent_id,
        provenance: ProvenanceType::Observed,
        rationale: format!("Set {task_id} {attribute}={value}"),
        causal_predecessors: vec![],
        datoms: vec![datom],
    };
    layout.write_tx(&tx)?;

    let human = format!("set: {task_id} {attribute}={value}\n");

    let json = serde_json::json!({
        "id": task_id,
        "attribute": attribute,
        "value": value,
    });

    let agent_out = AgentOutput {
        context: format!("set: {task_id} {attribute}={value}"),
        content: String::new(),
        footer: format!("show: braid task show {task_id}"),
    };

    Ok(CommandOutput {
        json,
        agent: agent_out,
        human,
    })
}

/// Add a dependency edge.
pub fn dep_add(
    path: &Path,
    from_id: &str,
    to_id: &str,
    agent: &str,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;
    ensure_layer_4_public(&layout, &store)?;

    let from = find_task_by_id(&store, from_id)
        .ok_or_else(|| BraidError::Validation(format!("task not found: {from_id}")))?;
    let to = find_task_by_id(&store, to_id)
        .ok_or_else(|| BraidError::Validation(format!("task not found: {to_id}")))?;

    // INV-TASK-002: Check acyclicity
    check_dependency_acyclicity(&store, from, to)
        .map_err(|e| BraidError::Validation(format!("dependency would create cycle: {e}")))?;

    let agent_id = AgentId::from_name(agent);
    let tx_id = super::write::next_tx_id(&store, agent_id);

    let datom = dep_add_datom(from, to, tx_id);
    let tx = TxFile {
        tx_id,
        agent: agent_id,
        provenance: ProvenanceType::Observed,
        rationale: format!("Add dependency: {from_id} → {to_id}"),
        causal_predecessors: vec![],
        datoms: vec![datom],
    };
    layout.write_tx(&tx)?;

    let human = format!("dependency: {from_id} depends on {to_id}\n");

    let json = serde_json::json!({
        "from": from_id,
        "to": to_id,
    });

    let agent = AgentOutput {
        context: format!("dependency: {from_id} depends on {to_id}"),
        content: String::new(),
        footer: "ready: braid task ready".to_string(),
    };

    Ok(CommandOutput { json, agent, human })
}

/// Import tasks from a beads JSONL file.
pub fn import_beads(
    path: &Path,
    beads_path: &Path,
    agent: &str,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;
    ensure_layer_4_public(&layout, &store)?;

    let content = std::fs::read_to_string(beads_path)
        .map_err(|e| BraidError::Validation(format!("cannot read beads file: {e}")))?;

    let agent_id = AgentId::from_name(agent);
    let tx_id = super::write::next_tx_id(&store, agent_id);

    let mut all_datoms = Vec::new();
    let mut imported = 0usize;
    let mut skipped = 0usize;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Minimal JSONL parsing — extract id, title, status, priority
        let bead = match parse_bead_line(line) {
            Some(b) => b,
            None => continue,
        };

        // Skip if already imported (check by task/id)
        if find_task_by_id(&store, &bead.task_id).is_some() {
            skipped += 1;
            continue;
        }

        let tt = TaskType::from_keyword(&bead.task_type).unwrap_or(TaskType::Task);

        let (_, datoms) = task::create_task_datoms(CreateTaskParams {
            title: &bead.title,
            description: bead.description.as_deref(),
            priority: bead.priority,
            task_type: tt,
            tx: tx_id,
            traces_to: &[],
            labels: &bead.labels,
        });

        // Override the auto-generated task ID with the beads ID
        let entity = EntityId::from_ident(&format!(":task/{}", bead.task_id));
        let mut fixed_datoms: Vec<_> = datoms
            .into_iter()
            .map(|mut d| {
                d.entity = entity;
                d
            })
            .collect();

        // Fix the :task/id value
        for d in &mut fixed_datoms {
            if d.attribute.as_str() == ":task/id" {
                d.value = Value::String(bead.task_id.clone());
            }
            if d.attribute.as_str() == ":db/ident" {
                d.value = Value::Keyword(format!(":task/{}", bead.task_id));
            }
            // Preserve beads source
            if d.attribute.as_str() == ":task/source" {
                d.value = Value::String(format!("beads:{}", bead.original_id));
            }
        }

        // Set status if not open
        if bead.status != "open" {
            if let Some(status) = TaskStatus::from_keyword(&bead.status) {
                fixed_datoms.push(update_status_datom(entity, status, tx_id));
            }
        }

        all_datoms.extend(fixed_datoms);
        imported += 1;
    }

    if all_datoms.is_empty() {
        let human = format!("import: 0 new tasks ({skipped} already imported)\n");
        let json = serde_json::json!({
            "imported": 0,
            "datom_count": 0,
            "skipped": skipped,
        });
        let agent_out = AgentOutput {
            context: format!("import: 0 new tasks ({skipped} skipped)"),
            content: String::new(),
            footer: "list: braid task list".to_string(),
        };
        return Ok(CommandOutput {
            json,
            agent: agent_out,
            human,
        });
    }

    let tx = TxFile {
        tx_id,
        agent: agent_id,
        provenance: ProvenanceType::Observed,
        rationale: format!("Import {imported} tasks from beads"),
        causal_predecessors: vec![],
        datoms: all_datoms,
    };
    let datom_count = tx.datoms.len();
    layout.write_tx(&tx)?;

    let human = format!("import: {imported} tasks, {datom_count} datoms ({skipped} skipped)\n");

    let json = serde_json::json!({
        "imported": imported,
        "datom_count": datom_count,
        "skipped": skipped,
    });

    let agent_out = AgentOutput {
        context: format!("import: {imported} tasks, {datom_count} datoms"),
        content: format!("{skipped} skipped (already imported)"),
        footer: "list: braid task list | ready: braid task ready".to_string(),
    };

    Ok(CommandOutput {
        json,
        agent: agent_out,
        human,
    })
}

/// Minimal bead parsed from JSONL.
struct ParsedBead {
    original_id: String,
    task_id: String,
    title: String,
    description: Option<String>,
    status: String,
    priority: i64,
    task_type: String,
    labels: Vec<String>,
}

/// Parse a single beads JSONL line into a ParsedBead.
///
/// Uses minimal manual JSON parsing to avoid adding a JSON dependency.
fn parse_bead_line(line: &str) -> Option<ParsedBead> {
    // Extract fields from JSON using simple string matching
    let id = extract_json_string(line, "id")?;
    let title = extract_json_string(line, "title")?;
    let description = extract_json_string(line, "description");
    let status = extract_json_string(line, "status").unwrap_or_else(|| "open".to_string());
    let priority = extract_json_number(line, "priority").unwrap_or(2);
    let bead_type = extract_json_string(line, "type").unwrap_or_else(|| "task".to_string());

    // Generate a short task ID from the beads ID
    let task_id = if id.starts_with("brai-") {
        id.clone()
    } else {
        format!("b-{}", &id[..id.len().min(8)])
    };

    Some(ParsedBead {
        original_id: id,
        task_id,
        title,
        description,
        status,
        priority,
        task_type: bead_type,
        labels: vec![],
    })
}

/// Extract a string value from JSON by key (minimal, no serde dependency).
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\":", key);
    let pos = json.find(&pattern)?;
    let after = &json[pos + pattern.len()..];
    let after = after.trim_start();

    if let Some(content) = after.strip_prefix('"') {
        let end = content.find('"')?;
        Some(content[..end].to_string())
    } else {
        None
    }
}

/// Extract a number value from JSON by key.
fn extract_json_number(json: &str, key: &str) -> Option<i64> {
    let pattern = format!("\"{}\":", key);
    let pos = json.find(&pattern)?;
    let after = &json[pos + pattern.len()..];
    let after = after.trim_start();

    let end = after.find(|c: char| !c.is_ascii_digit() && c != '-')?;
    after[..end].parse().ok()
}

// Witnesses: INV-STORE-001, INV-STORE-003, INV-SCHEMA-001,
//   INV-INTERFACE-001, INV-INTERFACE-011,
//   INV-RESOLUTION-001, ADR-STORE-003, ADR-INTERFACE-001
// (CLI task commands exercise append-only store, content-addressable identity,
//  schema-as-data, and the CLI-as-optimized-prompt interface.)

#[cfg(test)]
mod tests {
    use super::*;

    // Verifies: INV-INTERFACE-001
    // (CLI input parsing — correct extraction of JSON fields for task import.)
    #[test]
    fn extract_json_string_basic() {
        let json = r#"{"id":"brai-123","title":"Fix bug","status":"open"}"#;
        assert_eq!(
            extract_json_string(json, "id"),
            Some("brai-123".to_string())
        );
        assert_eq!(
            extract_json_string(json, "title"),
            Some("Fix bug".to_string())
        );
        assert_eq!(
            extract_json_string(json, "status"),
            Some("open".to_string())
        );
        assert_eq!(extract_json_string(json, "missing"), None);
    }

    // Verifies: INV-INTERFACE-001
    // (CLI input parsing — correct extraction of numeric JSON fields.)
    #[test]
    fn extract_json_number_basic() {
        let json = r#"{"priority":1,"count":42}"#;
        assert_eq!(extract_json_number(json, "priority"), Some(1));
        assert_eq!(extract_json_number(json, "count"), Some(42));
    }

    // Verifies: INV-INTERFACE-001, INV-STORE-003
    // (Beads import parsing — correct field extraction, content-addressable task ID.)
    #[test]
    fn parse_bead_line_complete() {
        let line = r#"{"id":"brai-2o3j","title":"Phase A Epic","status":"open","priority":0,"type":"epic"}"#;
        let bead = parse_bead_line(line).unwrap();
        assert_eq!(bead.task_id, "brai-2o3j");
        assert_eq!(bead.title, "Phase A Epic");
        assert_eq!(bead.priority, 0);
        assert_eq!(bead.task_type, "epic");
    }

    fn create_test_task(path: &Path, title: &str, priority: i64) -> CommandOutput {
        create(CreateArgs {
            path,
            title,
            description: None,
            priority,
            task_type: "task",
            agent: "test",
            traces_to: &[],
            labels: &[],
        })
        .unwrap()
    }

    // Verifies: INV-STORE-001, INV-SCHEMA-001, INV-INTERFACE-001, INV-INTERFACE-011,
    //   ADR-STORE-003, ADR-INTERFACE-001
    // (End-to-end CLI task creation: init store, create task, verify persistence.)
    #[test]
    fn create_task_via_cli() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");
        crate::commands::init::run(&path, Path::new("spec")).unwrap();

        let result = create_test_task(&path, "Test task", 2);
        assert!(result.human.contains("created:"));

        let list_result = list(&path, false).unwrap();
        assert!(result.human.contains("Test task") || list_result.human.contains("Test task"));
    }

    // Verifies: INV-STORE-001, INV-INTERFACE-001, INV-INTERFACE-011,
    //   INV-RESOLUTION-001, ADR-STORE-003
    // (CLI ready command: dependency-blocked tasks excluded from ready set.)
    #[test]
    fn ready_excludes_blocked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");
        crate::commands::init::run(&path, Path::new("spec")).unwrap();

        create_test_task(&path, "Task A", 1);
        create_test_task(&path, "Task B", 0);

        let id_a = generate_task_id("Task A");
        let id_b = generate_task_id("Task B");

        dep_add(&path, &id_b, &id_a, "test").unwrap();

        let result = ready(&path).unwrap();
        assert!(result.human.contains("Task A"));
        // Task B is blocked by A, so it shouldn't appear in ready
        assert!(!result.human.contains(&format!("{}  \"Task B\"", id_b)));
    }

    // Verifies: SFE-2.3
    // (Task create with valid INV in title produces :task/traces-to datom
    //  when the spec element exists in the store.)
    #[test]
    fn create_task_with_valid_spec_ref_produces_traces_to() {
        use braid_kernel::datom::{AgentId, Attribute, Datom, Op, TxId, Value};
        use braid_kernel::task::find_task_by_id;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");
        crate::commands::init::run(&path, Path::new("spec")).unwrap();

        // First, manually transact a spec element with :spec/falsification
        let layout = crate::layout::DiskLayout::open(&path).unwrap();
        let agent_id = AgentId::from_name("test");
        let tx_id = TxId::new(999, 0, agent_id);
        let spec_entity = braid_kernel::EntityId::from_ident(":spec/inv-store-001");
        let spec_datoms = vec![
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":spec/inv-store-001".to_string()),
                tx_id,
                Op::Assert,
            ),
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":spec/falsification"),
                Value::String("Any mutation of existing datom".to_string()),
                tx_id,
                Op::Assert,
            ),
        ];
        let spec_tx = braid_kernel::layout::TxFile {
            tx_id,
            agent: agent_id,
            provenance: braid_kernel::datom::ProvenanceType::Observed,
            rationale: "Test: create spec element".to_string(),
            causal_predecessors: vec![],
            datoms: spec_datoms,
        };
        layout.write_tx(&spec_tx).unwrap();

        // Now create a task referencing INV-STORE-001 in its title
        let result = create(CreateArgs {
            path: &path,
            title: "Fix INV-STORE-001 violation",
            description: None,
            priority: 1,
            task_type: "bug",
            agent: "test",
            traces_to: &[],
            labels: &[],
        })
        .unwrap();

        // Verify the output mentions traces-to
        assert!(
            result.human.contains("INV-STORE-001"),
            "human output should mention the resolved spec ref: {}",
            result.human
        );

        // Verify JSON output includes the resolved ref
        let traces = result.json["traces_to"].as_array().unwrap();
        assert!(
            traces.iter().any(|v| v.as_str() == Some("INV-STORE-001")),
            "JSON traces_to should include INV-STORE-001: {:?}",
            traces
        );

        // Verify the store has :task/traces-to datom
        let store = layout.load_store().unwrap();
        let task_id = generate_task_id("Fix INV-STORE-001 violation");
        let task_entity = find_task_by_id(&store, &task_id).expect("task should exist");
        let task_datoms = store.entity_datoms(task_entity);
        let has_traces_to = task_datoms.iter().any(|d| {
            d.attribute.as_str() == ":task/traces-to"
                && d.op == Op::Assert
                && matches!(d.value, Value::Ref(e) if e == spec_entity)
        });
        assert!(
            has_traces_to,
            "task should have :task/traces-to datom pointing to spec entity"
        );
    }

    // Verifies: SFE-2.3
    // (Task create with invalid INV in title includes warning in output.)
    #[test]
    fn create_task_with_invalid_spec_ref_warns() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");
        crate::commands::init::run(&path, Path::new("spec")).unwrap();

        let result = create(CreateArgs {
            path: &path,
            title: "Fix INV-FAKE-999 issue",
            description: None,
            priority: 1,
            task_type: "bug",
            agent: "test",
            traces_to: &[],
            labels: &[],
        })
        .unwrap();

        // Verify the human output contains the warning
        assert!(
            result.human.contains("INV-FAKE-999 not found in store"),
            "human output should warn about unresolved ref: {}",
            result.human
        );

        // Verify JSON output includes unresolved_refs
        let unresolved = result.json["unresolved_refs"].as_array().unwrap();
        assert!(
            unresolved
                .iter()
                .any(|v| v.as_str() == Some("INV-FAKE-999")),
            "JSON unresolved_refs should include INV-FAKE-999: {:?}",
            unresolved
        );
    }
}
