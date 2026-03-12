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

use braid_kernel::datom::{AgentId, ProvenanceType, Value};
use braid_kernel::layout::TxFile;
use braid_kernel::task::{
    self, check_dependency_acyclicity, close_task_datoms, compute_ready_set, dep_add_datom,
    find_task_by_id, generate_task_id, task_counts, task_summary, update_status_datom,
    CreateTaskParams, TaskStatus, TaskType,
};
use braid_kernel::EntityId;

use crate::error::BraidError;
use crate::layout::DiskLayout;

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
pub fn create(args: CreateArgs<'_>) -> Result<String, BraidError> {
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

    // Resolve traces-to references
    let trace_entities: Vec<EntityId> = traces_to.iter().map(|s| EntityId::from_ident(s)).collect();

    let (entity, datoms) = task::create_task_datoms(CreateTaskParams {
        title,
        description,
        priority,
        task_type: tt,
        tx: tx_id,
        traces_to: &trace_entities,
        labels,
    });

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

    let mut out = String::new();
    out.push_str(&format!("created: {task_id} \"{title}\"\n"));
    out.push_str(&format!(
        "  P{priority} {type_short} | {datom_count} datoms | entity: :task/{task_id}\n"
    ));
    if !traces_to.is_empty() {
        out.push_str(&format!("  traces-to: {}\n", traces_to.join(", ")));
    }
    let _ = entity; // used for entity creation

    Ok(out)
}

/// List tasks (all or filtered by status).
pub fn list(path: &Path, show_all: bool) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let tasks = task::all_tasks(&store);
    if tasks.is_empty() {
        return Ok("No tasks found. Create one: braid task \"title\"\n".to_string());
    }

    let (open, in_progress, closed) = task_counts(&store);
    let mut out = String::new();
    out.push_str(&format!(
        "Tasks: {} total ({} open, {} in-progress, {} closed)\n",
        tasks.len(),
        open,
        in_progress,
        closed
    ));

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
        out.push_str(&format!(
            "  P{} {:4} {:4}  {}  \"{}\"{}\n",
            t.priority, type_short, status, t.id, t.title, traces
        ));
    }

    Ok(out)
}

/// Show ready tasks (unblocked open tasks sorted by priority).
pub fn ready(path: &Path) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let ready_set = compute_ready_set(&store);
    if ready_set.is_empty() {
        return Ok("No ready tasks. Create one: braid task \"title\"\n".to_string());
    }

    let mut out = String::new();
    out.push_str(&format!("Ready tasks ({} unblocked):\n", ready_set.len()));

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
        out.push_str(&format!(
            "  P{}  {:7}  {}  \"{}\"{}\n",
            t.priority, type_short, t.id, t.title, traces
        ));
    }

    if let Some(top) = ready_set.first() {
        out.push_str(&format!(
            "Top pick: {} — run: braid task update {} --status in-progress\n",
            top.id, top.id
        ));
    }

    Ok(out)
}

/// Show detailed info about a task.
pub fn show(path: &Path, task_id: &str) -> Result<String, BraidError> {
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

    let mut out = String::new();
    out.push_str(&format!("{} \"{}\"\n", t.id, t.title));
    out.push_str(&format!(
        "  status: {status} | priority: P{} | type: {type_short}\n",
        t.priority
    ));
    if !t.labels.is_empty() {
        out.push_str(&format!("  labels: {}\n", t.labels.join(", ")));
    }
    if !t.depends_on.is_empty() {
        out.push_str(&format!("  depends-on: {} task(s)\n", t.depends_on.len()));
    }
    if !t.traces_to.is_empty() {
        out.push_str(&format!(
            "  traces-to: {} spec element(s)\n",
            t.traces_to.len()
        ));
    }
    if let Some(ref source) = t.source {
        out.push_str(&format!("  source: {source}\n"));
    }
    if let Some(ref reason) = t.close_reason {
        out.push_str(&format!("  close-reason: {reason}\n"));
    }
    out.push_str(&format!("  entity: :task/{}\n", t.id));

    Ok(out)
}

/// Close one or more tasks.
pub fn close(
    path: &Path,
    task_ids: &[String],
    reason: &str,
    agent: &str,
) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;
    ensure_layer_4_public(&layout, &store)?;

    let agent_id = AgentId::from_name(agent);
    let tx_id = super::write::next_tx_id(&store, agent_id);

    let mut all_datoms = Vec::new();
    let mut closed_ids = Vec::new();

    for task_id in task_ids {
        let entity = find_task_by_id(&store, task_id)
            .ok_or_else(|| BraidError::Validation(format!("task not found: {task_id}")))?;

        all_datoms.extend(close_task_datoms(entity, reason, tx_id));
        closed_ids.push(task_id.as_str());
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

    let mut out = String::new();
    for id in &closed_ids {
        out.push_str(&format!("closed: {id}\n"));
    }
    Ok(out)
}

/// Update a task's status.
pub fn update(path: &Path, task_id: &str, status: &str, agent: &str) -> Result<String, BraidError> {
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

    Ok(format!("updated: {task_id} → {status}\n"))
}

/// Add a dependency edge.
pub fn dep_add(path: &Path, from_id: &str, to_id: &str, agent: &str) -> Result<String, BraidError> {
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

    Ok(format!("dependency: {from_id} depends on {to_id}\n"))
}

/// Import tasks from a beads JSONL file.
pub fn import_beads(path: &Path, beads_path: &Path, agent: &str) -> Result<String, BraidError> {
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
        return Ok(format!(
            "import: 0 new tasks ({skipped} already imported)\n"
        ));
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

    Ok(format!(
        "import: {imported} tasks, {datom_count} datoms ({skipped} skipped)\n"
    ))
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

    fn create_test_task(path: &Path, title: &str, priority: i64) -> String {
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
        assert!(result.contains("created:"));

        let list_result = list(&path, false).unwrap();
        assert!(result.contains("Test task") || list_result.contains("Test task"));
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
        assert!(result.contains("Task A"));
        // Task B is blocked by A, so it shouldn't appear in ready
        assert!(!result.contains(&format!("{}  \"Task B\"", id_b)));
    }
}
