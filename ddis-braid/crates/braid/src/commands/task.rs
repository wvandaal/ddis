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

/// List tasks (backward-compat wrapper around list_filtered).
#[allow(dead_code)]
pub fn list(path: &Path, show_all: bool) -> Result<CommandOutput, BraidError> {
    list_filtered(path, show_all, None, None, None, None)
}

/// List tasks with optional filters.
///
/// Filters narrow the result set:
/// - `task_type`: Only show tasks of this type (e.g., "epic", "bug")
/// - `prefix`: Only show tasks whose title starts with this prefix (case-insensitive)
/// - `limit`: Maximum number of tasks to display
/// - `priority`: Only show tasks with this priority
///
/// Traces to: INV-INTERFACE-001 (Three CLI Output Modes)
pub fn list_filtered(
    path: &Path,
    show_all: bool,
    task_type: Option<&str>,
    prefix: Option<&str>,
    limit: Option<usize>,
    priority: Option<i64>,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let tasks = task::all_tasks(&store);
    if tasks.is_empty() {
        return Ok(CommandOutput::from_human(
            "No tasks found. Create one: braid task \"title\"\n".to_string(),
        ));
    }

    // Apply filters
    let filtered: Vec<_> = tasks
        .iter()
        .filter(|t| show_all || t.status != TaskStatus::Closed)
        .filter(|t| {
            task_type
                .map(|ty| {
                    let actual = t
                        .task_type
                        .strip_prefix(":task.type/")
                        .unwrap_or(&t.task_type);
                    actual.eq_ignore_ascii_case(ty)
                })
                .unwrap_or(true)
        })
        .filter(|t| {
            prefix
                .map(|p| {
                    t.title
                        .to_ascii_lowercase()
                        .starts_with(&p.to_ascii_lowercase())
                })
                .unwrap_or(true)
        })
        .filter(|t| priority.map(|p| t.priority == p).unwrap_or(true))
        .collect();

    let display_count = limit.unwrap_or(filtered.len()).min(filtered.len());
    let display_tasks = &filtered[..display_count];

    let (open, in_progress, closed) = task_counts(&store);
    let mut human = String::new();
    human.push_str(&format!(
        "Tasks: {} matched ({} total, {} open, {} in-progress, {} closed)\n",
        filtered.len(),
        tasks.len(),
        open,
        in_progress,
        closed
    ));

    let mut tasks_json = Vec::new();
    for t in display_tasks {
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
        // API-as-prompt (INV-INTERFACE-008): show short activation title in list,
        // full context available via `braid task show <id>`.
        let display_title = task::short_title(&t.title);
        human.push_str(&format!(
            "  P{} {:4} {:4}  {}  \"{}\"{}\n",
            t.priority, type_short, status, t.id, display_title, traces
        ));
        tasks_json.push(serde_json::json!({
            "id": t.id,
            "title": t.title,
            "short_title": braid_kernel::task::short_title(&t.title),
            "priority": t.priority,
            "type": type_short,
            "status": status,
            "traces_to_count": t.traces_to.len(),
        }));
    }

    if display_count < filtered.len() {
        human.push_str(&format!(
            "  ... and {} more (use --limit to see more)\n",
            filtered.len() - display_count
        ));
    }

    let json = serde_json::json!({
        "total": tasks.len(),
        "matched": filtered.len(),
        "displayed": display_count,
        "open": open,
        "in_progress": in_progress,
        "closed": closed,
        "tasks": tasks_json,
    });

    let agent_lines: Vec<String> = tasks_json
        .iter()
        .take(10)
        .map(|t| {
            let title = t["title"].as_str().unwrap_or("?");
            let short = task::short_title(title);
            format!(
                "[P{}] {} {} \"{}\"",
                t["priority"],
                t["id"].as_str().unwrap_or("?"),
                t["status"].as_str().unwrap_or("?"),
                short,
            )
        })
        .collect();
    let mut agent_content = agent_lines.join("\n");
    if filtered.len() > 10 {
        agent_content.push_str(&format!(
            "\n... ({} more, use --format json for full list)",
            filtered.len() - 10
        ));
    }
    let agent = AgentOutput {
        context: format!(
            "tasks: {} matched ({} displayed)",
            filtered.len(),
            display_count,
        ),
        content: agent_content,
        footer: "ready: braid task ready | next: braid next".to_string(),
    };

    Ok(CommandOutput { json, agent, human })
}

/// Full-text search across task titles and descriptions.
///
/// Returns tasks whose title or description contains the pattern (case-insensitive).
///
/// Traces to: INV-INTERFACE-001 (Three CLI Output Modes)
pub fn search(
    path: &Path,
    pattern: &str,
    include_closed: bool,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let tasks = task::all_tasks(&store);
    let pattern_lower = pattern.to_ascii_lowercase();

    let matches: Vec<_> = tasks
        .iter()
        .filter(|t| include_closed || t.status != TaskStatus::Closed)
        .filter(|t| {
            t.title.to_ascii_lowercase().contains(&pattern_lower)
                || t.id.to_ascii_lowercase().contains(&pattern_lower)
        })
        .collect();

    let mut human = String::new();
    human.push_str(&format!(
        "search \"{}\": {} result{}\n",
        pattern,
        matches.len(),
        if matches.len() == 1 { "" } else { "s" }
    ));

    if matches.is_empty() {
        human.push_str("  No tasks match. Try a broader term or --all to include closed tasks.\n");
    }

    let mut tasks_json = Vec::new();
    for t in &matches {
        let status = match t.status {
            TaskStatus::Open => "open",
            TaskStatus::InProgress => "work",
            TaskStatus::Closed => "done",
        };
        let type_short = t
            .task_type
            .strip_prefix(":task.type/")
            .unwrap_or(&t.task_type);
        human.push_str(&format!(
            "  P{} {:4} {:4}  {}  \"{}\"\n",
            t.priority, type_short, status, t.id, t.title
        ));
        tasks_json.push(serde_json::json!({
            "id": t.id,
            "title": t.title,
            "priority": t.priority,
            "type": type_short,
            "status": status,
        }));
    }

    let json = serde_json::json!({
        "pattern": pattern,
        "match_count": matches.len(),
        "tasks": tasks_json,
    });

    let agent = AgentOutput {
        context: format!("search \"{}\": {} results", pattern, matches.len()),
        content: tasks_json
            .iter()
            .take(10)
            .map(|t| {
                format!(
                    "[P{}] {} \"{}\"",
                    t["priority"],
                    t["id"].as_str().unwrap_or("?"),
                    t["title"].as_str().unwrap_or("?"),
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        footer: "show: braid task show <id> | ready: braid task ready".to_string(),
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
    // ACP projection for task ready (ACP-6, INV-BUDGET-007)
    // Action = top R(t) task, Context = task list entries with title pyramid levels
    let action = braid_kernel::guidance::compute_action_from_store(&store);

    let mut context_blocks = Vec::new();

    // Summary context (System — always shown)
    context_blocks.push(braid_kernel::budget::ContextBlock {
        precedence: braid_kernel::budget::OutputPrecedence::System,
        content: format!(
            "ready: {} tasks ({} total open)",
            ready_set.len(),
            ready_set.len()
        ),
        tokens: 8,
    });

    // Task entries as individual context blocks (UserRequested)
    // Use title pyramid: L1 (short) for each entry to keep tokens manageable
    let max_entries = 20; // cap at 20 even at max budget
    for (i, t) in ready_set.iter().take(max_entries).enumerate() {
        let type_short = t
            .task_type
            .strip_prefix(":task.type/")
            .unwrap_or(&t.task_type);

        // Try to get L1 title from store (pyramid), fall back to truncated full title
        let title_display = store
            .entity_datoms(braid_kernel::EntityId::from_ident(&format!(
                ":task/{}",
                t.id
            )))
            .iter()
            .find(|d| {
                d.attribute.as_str() == ":task/title-l1" && d.op == braid_kernel::datom::Op::Assert
            })
            .and_then(|d| match &d.value {
                braid_kernel::datom::Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| {
                // Fallback: first sentence of title
                let first = t.title.split_once(". ").map(|(s, _)| s).unwrap_or(&t.title);
                if first.len() > 80 {
                    let mut end = 0;
                    for (j, _) in first.char_indices() {
                        if j > 77 {
                            break;
                        }
                        end = j;
                    }
                    format!("{}...", &first[..end])
                } else {
                    first.to_string()
                }
            });

        // Higher-priority tasks get higher precedence blocks
        let precedence = if i < 3 {
            braid_kernel::budget::OutputPrecedence::UserRequested
        } else if i < 10 {
            braid_kernel::budget::OutputPrecedence::Speculative
        } else {
            braid_kernel::budget::OutputPrecedence::Ambient
        };

        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence,
            content: format!(
                "[P{}] {} {} \"{}\"",
                t.priority, t.id, type_short, title_display
            ),
            tokens: 12,
        });
    }

    if ready_set.len() > max_entries {
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::Ambient,
            content: format!(
                "... and {} more (braid task ready --all)",
                ready_set.len() - max_entries
            ),
            tokens: 5,
        });
    }

    let top_id = ready_set.first().map(|t| t.id.as_str()).unwrap_or("?");
    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: format!("all: braid task ready | show: braid task show {top_id}"),
    };

    // Human output uses ACP full projection
    let human = projection.project(usize::MAX);

    // Agent output uses ACP Navigate projection
    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    // Merge ACP into JSON
    let mut json = serde_json::json!({
        "ready_count": ready_set.len(),
        "tasks": tasks_json,
    });
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

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

    // ACP projection (INV-BUDGET-007)
    let action = braid_kernel::budget::ProjectedAction {
        command: format!("braid go {}", top.id),
        rationale: {
            let words: Vec<&str> = top.title.split_whitespace().collect();
            if words.len() > 8 {
                format!("{} ...", words[..8].join(" "))
            } else {
                top.title.clone()
            }
        },
        impact: impact_score.unwrap_or(0.0),
    };

    let mut context_blocks = vec![
        braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::UserRequested,
            content: format!(
                "P{} {} | {} deps | {} traces{}",
                top.priority,
                type_short,
                top.depends_on.len(),
                top.traces_to.len(),
                impact_str,
            ),
            tokens: 12,
        },
        braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::Speculative,
            content: format!("{} total ready | all: braid task ready", ready_count),
            tokens: 8,
        },
    ];

    // Add title L1 as context if available (from pyramid)
    let title_l1 = store
        .entity_datoms(braid_kernel::EntityId::from_ident(&format!(
            ":task/{}",
            top.id
        )))
        .iter()
        .find(|d| {
            d.attribute.as_str() == ":task/title-l1" && d.op == braid_kernel::datom::Op::Assert
        })
        .and_then(|d| match &d.value {
            braid_kernel::datom::Value::String(s) => Some(s.clone()),
            _ => None,
        });
    if let Some(l1) = title_l1 {
        context_blocks.insert(
            0,
            braid_kernel::budget::ContextBlock {
                precedence: braid_kernel::budget::OutputPrecedence::System,
                content: l1,
                tokens: 10,
            },
        );
    }

    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: format!("details: braid task show {}", top.id),
    };

    // Human output — use ACP full projection
    let human = projection.project(usize::MAX);

    // JSON output — includes impact_score + ACP fields
    let mut json = serde_json::json!({
        "id": top.id,
        "title": top.title,
        "priority": top.priority,
        "type": type_short,
        "impact_score": impact_score,
        "claim_command": format!("braid go {}", top.id),
        "ready_count": ready_count,
    });
    // Merge ACP JSON
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    // Agent output — use ACP Navigate projection
    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
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

    // TAP-3: Extract structured sections from entity datoms.
    let entity_datoms = store.entity_datoms(entity);
    let mut background: Option<String> = None;
    let mut acceptance: Option<String> = None;
    let mut approach: Option<String> = None;
    let mut files: Vec<String> = Vec::new();
    for d in &entity_datoms {
        if d.op != Op::Assert {
            continue;
        }
        match d.attribute.as_str() {
            ":task/background" => {
                if let Value::String(ref s) = d.value {
                    background = Some(s.clone());
                }
            }
            ":task/acceptance" => {
                if let Value::String(ref s) = d.value {
                    acceptance = Some(s.clone());
                }
            }
            ":task/approach" => {
                if let Value::String(ref s) = d.value {
                    approach = Some(s.clone());
                }
            }
            ":task/files" => {
                if let Value::String(ref s) = d.value {
                    files.push(s.clone());
                }
            }
            _ => {}
        }
    }
    files.sort();
    files.dedup();

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
    // TAP-3: Structured sections in human output
    if let Some(ref bg) = background {
        human.push_str("\nBACKGROUND:\n");
        for line in bg.lines() {
            human.push_str(&format!("  {line}\n"));
        }
    }
    if let Some(ref acc) = acceptance {
        human.push_str("\nACCEPTANCE:\n");
        for line in acc.lines() {
            human.push_str(&format!("  {line}\n"));
        }
    }
    if let Some(ref app) = approach {
        human.push_str("\nAPPROACH:\n");
        for line in app.lines() {
            human.push_str(&format!("  {line}\n"));
        }
    }
    if !files.is_empty() {
        human.push_str("\nFILES:\n");
        for f in &files {
            human.push_str(&format!("  - {f}\n"));
        }
    }
    human.push_str(&format!("\n  entity: :task/{}\n", t.id));

    let mut json = serde_json::json!({
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
        // TAP-3: Structured sections in JSON
        "background": background,
        "acceptance": acceptance,
        "approach": approach,
        "files": files,
    });

    // ACP projection (ACP-10b, INV-BUDGET-007)
    let action = braid_kernel::budget::ProjectedAction {
        command: if t.status == TaskStatus::Closed {
            "braid task ready".to_string()
        } else {
            format!("braid go {}", t.id)
        },
        rationale: {
            let (l0, _) = braid_kernel::task::generate_title_levels(&t.title);
            l0
        },
        impact: 0.0,
    };

    let mut context_blocks = vec![
        braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::System,
            content: format!("{} \"{}\"", t.id, t.title),
            tokens: 15,
        },
        braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::UserRequested,
            content: format!(
                "status: {} | P{} {} | deps: {} | traces: {}",
                status,
                t.priority,
                type_short,
                t.depends_on.len(),
                t.traces_to.len(),
            ),
            tokens: 10,
        },
    ];

    if !t.labels.is_empty() {
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::Speculative,
            content: format!("labels: {}", t.labels.join(", ")),
            tokens: 5,
        });
    }

    if let Some(ref reason) = t.close_reason {
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::Speculative,
            content: format!("close-reason: {reason}"),
            tokens: 5,
        });
    }

    // TAP-3: Structured section ACP context blocks
    if let Some(ref bg) = background {
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::UserRequested,
            content: format!("BACKGROUND: {bg}"),
            tokens: bg.len() / 4 + 5,
        });
    }
    if let Some(ref acc) = acceptance {
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::UserRequested,
            content: format!("ACCEPTANCE: {acc}"),
            tokens: acc.len() / 4 + 5,
        });
    }
    if let Some(ref app) = approach {
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::Speculative,
            content: format!("APPROACH: {app}"),
            tokens: app.len() / 4 + 5,
        });
    }
    if !files.is_empty() {
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::Speculative,
            content: format!("FILES: {}", files.join(", ")),
            tokens: files.len() * 5 + 3,
        });
    }

    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: format!("full: braid query --entity :task/{}", t.id),
    };

    // Use ACP for agent output
    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    // Merge ACP into JSON
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

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
    let mut store = layout.load_store()?;
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

    // Apply the close datoms to the in-memory store instead of reloading
    // from disk. This avoids an expensive full load_store() call (T2-2).
    {
        let rationale = format!("Close task(s): {}", closed_ids.join(", "));
        let mut builder =
            braid_kernel::store::Transaction::new(agent_id, ProvenanceType::Observed, &rationale);
        for datom in &tx.datoms {
            match datom.op {
                Op::Assert => {
                    builder =
                        builder.assert(datom.entity, datom.attribute.clone(), datom.value.clone());
                }
                Op::Retract => {
                    builder =
                        builder.retract(datom.entity, datom.attribute.clone(), datom.value.clone());
                }
            }
        }
        let committed = builder
            .commit(&store)
            .map_err(braid_kernel::KernelError::from)?;
        store
            .transact(committed)
            .map_err(braid_kernel::KernelError::from)?;
    }

    let mut human = String::new();
    // Show blocked tasks first (if any in a batch)
    for (id, msg) in &blocked_ids {
        human.push_str(&format!("BLOCKED: {id} — {msg}\n"));
    }
    for id in &closed_ids {
        human.push_str(&format!("closed: {id}\n"));
    }

    // ZCM-4: Completion reward — show F(S) as feedback signal.
    // From reinforcement learning: explicit reward signals strengthen behavioral patterns.
    let fitness = braid_kernel::bilateral::compute_fitness(&store);
    human.push_str(&format!("F(S)={:.2}\n", fitness.total));

    let json = serde_json::json!({
        "closed": closed_ids,
        "count": closed_ids.len(),
        "reason": reason,
        "fitness": fitness.total,
    });

    // ACP for close: action = next task
    let action = braid_kernel::guidance::compute_action_from_store(&store);
    let mut context_blocks = vec![braid_kernel::budget::ContextBlock {
        precedence: braid_kernel::budget::OutputPrecedence::System,
        content: format!(
            "closed: {} task(s) | F(S)={:.2}",
            closed_ids.len(),
            fitness.total
        ),
        tokens: 10,
    }];
    for id in &closed_ids {
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::UserRequested,
            content: format!("closed: {id}"),
            tokens: 3,
        });
    }
    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: "next: braid task ready | status: braid status".to_string(),
    };

    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    // Merge ACP into JSON
    let mut json = json;
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

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

    // Reload store for task summary context
    let store = layout.load_store()?;

    let human = format!("updated: {task_id} \u{2192} {status}\n");

    let mut json = serde_json::json!({
        "id": task_id,
        "status": status,
    });

    // ACP: action = view the claimed task
    let mut context_blocks = vec![braid_kernel::budget::ContextBlock {
        precedence: braid_kernel::budget::OutputPrecedence::System,
        content: format!("updated: {task_id} \u{2192} {status}"),
        tokens: 5,
    }];
    if let Some(summary) = task_summary(&store, entity) {
        let title_trunc = if summary.title.len() > 80 {
            format!(
                "{}...",
                &summary.title[..summary.title.floor_char_boundary(77)]
            )
        } else {
            summary.title.clone()
        };
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::UserRequested,
            content: format!("{}: {}", task_id, title_trunc),
            tokens: 10,
        });
    }

    let projection = braid_kernel::ActionProjection {
        action: braid_kernel::budget::ProjectedAction {
            command: format!("braid task show {task_id}"),
            rationale: "view claimed task details".to_string(),
            impact: 0.5,
        },
        context: context_blocks,
        evidence_pointer: format!("details: braid task show {task_id}"),
    };

    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    // Merge ACP into JSON
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    Ok(CommandOutput { json, agent, human })
}

/// Map a friendly attribute name to its store keyword for current value lookup.
fn attribute_to_keyword(attribute: &str) -> Option<&'static str> {
    match attribute {
        "priority" => Some(":task/priority"),
        "status" => Some(":task/status"),
        "type" => Some(":task/type"),
        "title" => Some(":task/title"),
        _ => None,
    }
}

/// Format a store Value as a human-friendly display string for diff output.
fn format_value_for_display(val: &Value) -> String {
    match val {
        Value::Long(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Keyword(k) => {
            // Strip namespace prefix for readability: ":task.status/open" → "open"
            k.rsplit('/').next().unwrap_or(k).to_string()
        }
        Value::Double(f) => f.to_string(),
        Value::Boolean(b) => b.to_string(),
        _ => format!("{val:?}"),
    }
}

/// Set an arbitrary task attribute by friendly name.
///
/// Supported attributes: priority (0-4), status (open/in-progress/closed),
/// type (task/bug/feature/epic/question/docs), title (non-empty string).
/// LWW resolution handles the "update" semantics.
///
/// Shows old→new transition diff in output. If the value is unchanged,
/// reports "(unchanged)" and still writes the datom (append-only, C1).
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

    // Query current value before writing (for old→new diff)
    let old_display = attribute_to_keyword(attribute).and_then(|kw| {
        let attr = Attribute::from_keyword(kw);
        store
            .live_value(entity, &attr)
            .map(format_value_for_display)
    });

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

    // Build transition display: "old→new" or "new (unchanged)"
    let unchanged = old_display.as_deref() == Some(value);
    let transition = if unchanged {
        format!("{value} (unchanged)")
    } else if let Some(old) = &old_display {
        format!("{old}\u{2192}{value}")
    } else {
        // No previous value (first time setting this attribute)
        value.to_string()
    };

    let human = format!("set: {task_id} {attribute} {transition}\n");

    let json = serde_json::json!({
        "id": task_id,
        "attribute": attribute,
        "value": value,
        "old_value": old_display,
        "unchanged": unchanged,
    });

    let agent_out = AgentOutput {
        context: format!("set: {task_id} {attribute} {transition}"),
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
// ===========================================================================
// Task Audit — detect likely-implemented open tasks (T5-1)
// ===========================================================================

/// Run `braid task audit` — detect open tasks with store-based completion evidence.
///
/// Uses the kernel's `audit_tasks_from_store` (pure, no IO) to find open tasks
/// where spec refs have :impl/implements links at L2+.
pub fn audit(path: &Path) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let results = braid_kernel::task::audit_tasks_from_store(&store);

    if results.is_empty() {
        return Ok(CommandOutput::from_human(
            "No tasks with store-based completion evidence found.\n".to_string(),
        ));
    }

    let mut close_ids = Vec::new();
    for (task, _) in &results {
        close_ids.push(task.id.clone());
    }
    let close_cmd = format!("braid task close {}", close_ids.join(" "));

    // Human output (kept as-is)
    let mut human = format!(
        "audit: {} tasks may be implemented but not closed\n\n",
        results.len()
    );
    for (task, evidence) in &results {
        human.push_str(&format!(
            "[{:.0}%] {} \"{}\"\n",
            evidence.confidence * 100.0,
            task.id,
            if task.title.len() > 80 {
                let end = task.title.floor_char_boundary(77);
                format!("{}...", &task.title[..end])
            } else {
                task.title.clone()
            }
        ));
        if evidence.spec_total > 0 {
            human.push_str(&format!(
                "  spec: {}/{} refs have impl links\n",
                evidence.spec_coverage, evidence.spec_total
            ));
        }
        if let Some(cc) = evidence.criteria_confidence {
            human.push_str(&format!(
                "  criteria: {:.0}% ({} acceptance criteria parsed)\n",
                cc * 100.0,
                evidence.acceptance_criteria.len()
            ));
        }
        if !evidence.file_paths.is_empty() {
            human.push_str(&format!("  files: {}\n", evidence.file_paths.join(", ")));
        }
    }
    human.push_str(&format!("\nclose: {}\n", close_cmd));

    // ACP-AUDIT: Build ActionProjection
    let action = braid_kernel::budget::ProjectedAction {
        command: close_cmd.clone(),
        rationale: format!("{} tasks likely implemented", results.len()),
        impact: 0.6,
    };

    let mut context_blocks = Vec::new();

    // Context per task: each audit result is evidence
    for (task, evidence) in &results {
        let title_display = if task.title.len() > 60 {
            let end = task.title.floor_char_boundary(57);
            format!("{}...", &task.title[..end])
        } else {
            task.title.clone()
        };
        let mut detail = format!(
            "[{:.0}%] {} \"{}\"",
            evidence.confidence * 100.0,
            task.id,
            title_display,
        );
        if evidence.spec_total > 0 {
            detail.push_str(&format!(
                " (spec: {}/{})",
                evidence.spec_coverage, evidence.spec_total
            ));
        }
        if let Some(cc) = evidence.criteria_confidence {
            detail.push_str(&format!(" (criteria: {:.0}%)", cc * 100.0));
        }
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::UserRequested,
            content: detail,
            tokens: 15,
        });
    }

    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: "details: braid task list | refresh: braid task audit".to_string(),
    };

    // JSON with _acp
    let mut json = serde_json::json!({
        "audit_results": results.iter().map(|(t, e)| serde_json::json!({
            "id": t.id,
            "title": t.title,
            "confidence": e.confidence,
            "spec_coverage": e.spec_coverage,
            "spec_total": e.spec_total,
            "file_paths": e.file_paths,
        })).collect::<Vec<_>>(),
        "close_command": close_cmd,
    });
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    // Agent output via ACP Navigate projection
    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    Ok(CommandOutput { json, agent, human })
}

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

    // Verifies: T-UX-2 (task set shows old→new value diff)
    #[test]
    fn set_shows_old_to_new_diff() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");
        crate::commands::init::run(&path, Path::new("spec")).unwrap();

        // Create task with priority 2
        create_test_task(&path, "Priority diff test", 2);
        let task_id = generate_task_id("Priority diff test");

        // Set priority from 2 to 0 — should show "2→0"
        let result = set(&path, &task_id, "priority", "0", "test").unwrap();
        assert!(
            result.human.contains("2\u{2192}0"),
            "should show old→new transition: {}",
            result.human
        );
        assert!(
            result.json["old_value"].as_str() == Some("2"),
            "JSON old_value should be '2': {:?}",
            result.json
        );
        assert!(!result.json["unchanged"].as_bool().unwrap_or(true));
    }

    // Verifies: T-UX-2 (task set shows "(unchanged)" when value is same)
    #[test]
    fn set_shows_unchanged_when_same() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");
        crate::commands::init::run(&path, Path::new("spec")).unwrap();

        // Create task with priority 2
        create_test_task(&path, "Unchanged diff test", 2);
        let task_id = generate_task_id("Unchanged diff test");

        // Set priority to same value (2) — should show "(unchanged)"
        let result = set(&path, &task_id, "priority", "2", "test").unwrap();
        assert!(
            result.human.contains("(unchanged)"),
            "should show (unchanged) for same value: {}",
            result.human
        );
        assert!(result.json["unchanged"].as_bool().unwrap_or(false));
    }

    // Verifies: T-UX-2 (task set shows status transition)
    #[test]
    fn set_shows_status_transition() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");
        crate::commands::init::run(&path, Path::new("spec")).unwrap();

        create_test_task(&path, "Status diff test", 2);
        let task_id = generate_task_id("Status diff test");

        // Set status from open to in-progress
        let result = set(&path, &task_id, "status", "in-progress", "test").unwrap();
        assert!(
            result.human.contains("open\u{2192}in-progress"),
            "should show status transition: {}",
            result.human
        );
    }
}
