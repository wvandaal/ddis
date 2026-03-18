//! Task management kernel functions — issue tracking as datoms.
//!
//! Tasks are stored as entities in the datom store with `:task/*` attributes.
//! Status uses lattice resolution (open < in-progress < closed) ensuring
//! monotonic progression under CRDT merge (INV-TASK-001).
//! Dependencies form a DAG (INV-TASK-002) enforced at insertion time.
//!
//! # Formal Properties
//!
//! - **INV-TASK-001**: Status monotonicity — resolved status never decreases.
//! - **INV-TASK-002**: Dependency acyclicity — `:task/depends-on` graph is a DAG.
//! - **INV-TASK-003**: Ready computation — ready iff open AND all deps closed.
//! - **INV-TASK-004**: Priority ordering — ready set sorted by priority, then age.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use crate::datom::{Attribute, Datom, EntityId, Op, TxId, Value};
use crate::store::Store;

// ---------------------------------------------------------------------------
// Task status lattice (INV-TASK-001)
// ---------------------------------------------------------------------------

/// Task status values forming a bounded join-semilattice.
///
/// ⊥ = Open < InProgress < Closed = ⊤
/// join(a, b) = max(a, b) in this ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskStatus {
    /// Task is open (not started).
    Open = 0,
    /// Task is in progress.
    InProgress = 1,
    /// Task is closed (completed or abandoned).
    Closed = 2,
}

impl TaskStatus {
    /// Parse from a keyword string.
    pub fn from_keyword(kw: &str) -> Option<Self> {
        match kw {
            ":task.status/open" | "open" => Some(TaskStatus::Open),
            ":task.status/in-progress" | "in-progress" => Some(TaskStatus::InProgress),
            ":task.status/closed" | "closed" => Some(TaskStatus::Closed),
            _ => None,
        }
    }

    /// Convert to keyword string.
    pub fn as_keyword(&self) -> &'static str {
        match self {
            TaskStatus::Open => ":task.status/open",
            TaskStatus::InProgress => ":task.status/in-progress",
            TaskStatus::Closed => ":task.status/closed",
        }
    }

    /// Lattice join: max of two status values.
    pub fn join(self, other: Self) -> Self {
        if self >= other {
            self
        } else {
            other
        }
    }
}

/// Task type classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskType {
    /// General task.
    Task,
    /// Bug report.
    Bug,
    /// Feature request.
    Feature,
    /// Test task.
    Test,
    /// Epic (parent container).
    Epic,
    /// Question.
    Question,
    /// Documentation.
    Docs,
}

impl TaskType {
    /// Parse from a keyword string.
    pub fn from_keyword(kw: &str) -> Option<Self> {
        match kw {
            ":task.type/task" | "task" => Some(TaskType::Task),
            ":task.type/bug" | "bug" => Some(TaskType::Bug),
            ":task.type/feature" | "feature" => Some(TaskType::Feature),
            ":task.type/test" | "test" => Some(TaskType::Test),
            ":task.type/epic" | "epic" => Some(TaskType::Epic),
            ":task.type/question" | "question" => Some(TaskType::Question),
            ":task.type/docs" | "docs" => Some(TaskType::Docs),
            _ => None,
        }
    }

    /// Convert to keyword string.
    pub fn as_keyword(&self) -> &'static str {
        match self {
            TaskType::Task => ":task.type/task",
            TaskType::Bug => ":task.type/bug",
            TaskType::Feature => ":task.type/feature",
            TaskType::Test => ":task.type/test",
            TaskType::Epic => ":task.type/epic",
            TaskType::Question => ":task.type/question",
            TaskType::Docs => ":task.type/docs",
        }
    }

    /// Routing weight multiplier for task type.
    ///
    /// Weights downstream tasks by their type to reflect direct value:
    /// - Implementation/bug tasks (1.0) are highest value to unblock
    /// - Features (0.9) and tests (0.8) are high value
    /// - Epics (0.0) are containers with no direct work value
    /// - Docs (0.3) and questions (0.2) are low-urgency
    pub fn type_multiplier(self) -> f64 {
        match self {
            TaskType::Task => 1.0,
            TaskType::Bug => 1.0,
            TaskType::Feature => 0.9,
            TaskType::Test => 0.8,
            TaskType::Epic => 0.0,
            TaskType::Docs => 0.3,
            TaskType::Question => 0.2,
        }
    }
}

// ---------------------------------------------------------------------------
// Task summary (query result)
// ---------------------------------------------------------------------------

/// Summary of a task for display and sorting.
#[derive(Clone, Debug)]
pub struct TaskSummary {
    /// Entity ID.
    pub entity: EntityId,
    /// Short task ID (e.g., "t-aB3c").
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Current resolved status.
    pub status: TaskStatus,
    /// Priority (0=critical..4=backlog).
    pub priority: i64,
    /// Task type keyword.
    pub task_type: String,
    /// Creation time (unix seconds).
    pub created_at: u64,
    /// Dependencies (entity IDs of tasks this depends on).
    pub depends_on: Vec<EntityId>,
    /// Spec element references.
    pub traces_to: Vec<EntityId>,
    /// Labels.
    pub labels: Vec<String>,
    /// Source (e.g., "beads:brai-114c").
    pub source: Option<String>,
    /// Close reason (if closed).
    pub close_reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Task ID generation
// ---------------------------------------------------------------------------

/// Generate a short task ID from title text.
///
/// Format: `t-{8 chars}` derived from BLAKE3 hash of the title.
/// Deterministic: same title → same ID.
///
/// # History
///
/// Previously used 4 hex chars (16 bits = 65,536 possibilities). At 375 tasks,
/// the birthday problem gave 66% collision probability, and a confirmed collision
/// was found: t-09cc had two different task titles mapped to the same entity.
/// Increased to 8 hex chars (32 bits = ~4 billion possibilities). Expected
/// collisions at 2,000 tasks: 0.0005. At 100,000 tasks: 1.2.
///
/// See: Session 024 audit, FM-034 (Task ID Birthday Collision).
pub fn generate_task_id(title: &str) -> String {
    let hash = blake3::hash(title.as_bytes());
    let hex = hash.to_hex();
    format!("t-{}", &hex[..8])
}

// ---------------------------------------------------------------------------
// Datom construction
// ---------------------------------------------------------------------------

/// Parameters for creating a task.
pub struct CreateTaskParams<'a> {
    /// Task title.
    pub title: &'a str,
    /// Optional longer description.
    pub description: Option<&'a str>,
    /// Priority (0=critical, 4=backlog).
    pub priority: i64,
    /// Task type (task, bug, feature, etc.).
    pub task_type: TaskType,
    /// Transaction ID for datom construction.
    pub tx: TxId,
    /// Spec elements this task traces to.
    pub traces_to: &'a [EntityId],
    /// Categorical labels.
    pub labels: &'a [String],
}

/// Create datoms for a new task entity.
///
/// Returns (entity_id, datoms) for transaction construction.
pub fn create_task_datoms(params: CreateTaskParams<'_>) -> (EntityId, Vec<Datom>) {
    let CreateTaskParams {
        title,
        description,
        priority,
        task_type,
        tx,
        traces_to,
        labels,
    } = params;
    let task_id = generate_task_id(title);
    let ident = format!(":task/{task_id}");
    let entity = EntityId::from_ident(&ident);

    let wall_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut datoms = vec![
        Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(ident),
            tx,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":task/id"),
            Value::String(task_id),
            tx,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":task/title"),
            Value::String(title.to_string()),
            tx,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":task/status"),
            Value::Keyword(TaskStatus::Open.as_keyword().to_string()),
            tx,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":task/priority"),
            Value::Long(priority),
            tx,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":task/type"),
            Value::Keyword(task_type.as_keyword().to_string()),
            tx,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":task/created-at"),
            Value::Long(wall_time as i64),
            tx,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":task/source"),
            Value::String("braid:task".to_string()),
            tx,
            Op::Assert,
        ),
    ];

    if let Some(desc) = description {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":task/description"),
            Value::String(desc.to_string()),
            tx,
            Op::Assert,
        ));
    }

    for trace in traces_to {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":task/traces-to"),
            Value::Ref(*trace),
            tx,
            Op::Assert,
        ));
    }

    for label in labels {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":task/labels"),
            Value::Keyword(format!(":label/{label}")),
            tx,
            Op::Assert,
        ));
    }

    (entity, datoms)
}

/// Create datoms to close a task.
pub fn close_task_datoms(entity: EntityId, reason: &str, tx: TxId) -> Vec<Datom> {
    let wall_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    vec![
        Datom::new(
            entity,
            Attribute::from_keyword(":task/status"),
            Value::Keyword(TaskStatus::Closed.as_keyword().to_string()),
            tx,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":task/closed-at"),
            Value::Long(wall_time as i64),
            tx,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":task/close-reason"),
            Value::String(reason.to_string()),
            tx,
            Op::Assert,
        ),
    ]
}

/// Create a datom to update task status.
pub fn update_status_datom(entity: EntityId, status: TaskStatus, tx: TxId) -> Datom {
    Datom::new(
        entity,
        Attribute::from_keyword(":task/status"),
        Value::Keyword(status.as_keyword().to_string()),
        tx,
        Op::Assert,
    )
}

/// Create a dependency edge datom.
pub fn dep_add_datom(from: EntityId, to: EntityId, tx: TxId) -> Datom {
    Datom::new(
        from,
        Attribute::from_keyword(":task/depends-on"),
        Value::Ref(to),
        tx,
        Op::Assert,
    )
}

/// Map a user-friendly attribute name to a datom attribute keyword and value.
///
/// Supported attributes:
/// - `"priority"` -> `:task/priority` (Long)
/// - `"status"` -> `:task/status` (Keyword via TaskStatus lattice)
/// - `"type"` -> `:task/type` (Keyword via TaskType)
/// - `"title"` -> `:task/title` (String)
///
/// Returns `Err` with a human-readable message if the attribute name is unknown
/// or the value cannot be parsed for that attribute's type.
pub fn set_attribute_datom(
    entity: EntityId,
    attribute: &str,
    value: &str,
    tx: TxId,
) -> Result<Datom, String> {
    match attribute {
        "priority" => {
            let n: i64 = value
                .parse()
                .map_err(|_| format!("invalid priority: {value} (use 0-4)"))?;
            if !(0..=4).contains(&n) {
                return Err(format!("priority out of range: {n} (use 0-4)"));
            }
            Ok(Datom::new(
                entity,
                Attribute::from_keyword(":task/priority"),
                Value::Long(n),
                tx,
                Op::Assert,
            ))
        }
        "status" => {
            let status = TaskStatus::from_keyword(value).ok_or_else(|| {
                format!("invalid status: {value} (use open, in-progress, closed)")
            })?;
            Ok(Datom::new(
                entity,
                Attribute::from_keyword(":task/status"),
                Value::Keyword(status.as_keyword().to_string()),
                tx,
                Op::Assert,
            ))
        }
        "type" => {
            let tt = TaskType::from_keyword(value).ok_or_else(|| {
                format!("invalid type: {value} (use task, bug, feature, epic, question, docs)")
            })?;
            Ok(Datom::new(
                entity,
                Attribute::from_keyword(":task/type"),
                Value::Keyword(tt.as_keyword().to_string()),
                tx,
                Op::Assert,
            ))
        }
        "title" => {
            if value.is_empty() {
                return Err("title cannot be empty".to_string());
            }
            Ok(Datom::new(
                entity,
                Attribute::from_keyword(":task/title"),
                Value::String(value.to_string()),
                tx,
                Op::Assert,
            ))
        }
        _ => Err(format!(
            "unknown attribute: {attribute} (use priority, status, type, title)"
        )),
    }
}

// ---------------------------------------------------------------------------
// Spec reference extraction and resolution (SFE-2.1, SFE-2.2)
// ---------------------------------------------------------------------------

/// Extract spec element IDs from a task title string.
///
/// Matches patterns: `INV-{NAMESPACE}-{NNN}`, `ADR-{NAMESPACE}-{NNN}`,
/// `NEG-{NAMESPACE}-{NNN}` where NAMESPACE is one or more uppercase ASCII
/// letters and NNN is one or more digits.
///
/// Returns unique, sorted list.
///
/// # Examples
///
/// ```
/// use braid_kernel::task::parse_spec_refs;
/// let refs = parse_spec_refs("Fix merge (INV-MERGE-001, ADR-MERGE-005)");
/// assert_eq!(refs, vec!["ADR-MERGE-005", "INV-MERGE-001"]);
/// ```
pub fn parse_spec_refs(title: &str) -> Vec<String> {
    let prefixes = ["INV-", "ADR-", "NEG-"];
    let mut results = BTreeSet::new();
    let bytes = title.as_bytes();
    let len = bytes.len();

    let mut i = 0;
    while i < len {
        // Check if any prefix starts here
        let mut matched_prefix: Option<&str> = None;
        for prefix in &prefixes {
            if i + prefix.len() <= len && &title[i..i + prefix.len()] == *prefix {
                // Ensure word boundary: start of string or non-alphanumeric before prefix
                if i == 0 || !bytes[i - 1].is_ascii_alphanumeric() {
                    matched_prefix = Some(prefix);
                    break;
                }
            }
        }

        if let Some(prefix) = matched_prefix {
            let after_prefix = i + prefix.len();
            // Expect NAMESPACE: one or more uppercase ASCII letters
            let ns_start = after_prefix;
            let mut ns_end = ns_start;
            while ns_end < len && bytes[ns_end].is_ascii_uppercase() {
                ns_end += 1;
            }
            if ns_end > ns_start && ns_end < len && bytes[ns_end] == b'-' {
                // Expect digits after the hyphen
                let digit_start = ns_end + 1;
                let mut digit_end = digit_start;
                while digit_end < len && bytes[digit_end].is_ascii_digit() {
                    digit_end += 1;
                }
                if digit_end > digit_start {
                    results.insert(title[i..digit_end].to_string());
                    i = digit_end;
                    continue;
                }
            }
        }
        i += 1;
    }

    results.into_iter().collect()
}

/// Resolve spec references against the store.
///
/// Returns `(resolved, unresolved)` where:
/// - `resolved`: pairs of (original ID string, entity ID) for refs that exist
///   in the store as formal spec elements (have `:spec/falsification` attribute).
/// - `unresolved`: ID strings that either don't exist or lack falsification
///   (observations, not formal spec elements).
///
/// A ref is "resolved" if an entity `:spec/{id-lowercase}` exists with
/// a `:spec/falsification` attribute (formal spec element, not observation).
pub fn resolve_spec_refs(store: &Store, refs: &[String]) -> (Vec<(String, EntityId)>, Vec<String>) {
    let mut resolved = Vec::new();
    let mut unresolved = Vec::new();
    for ref_id in refs {
        let ident = format!(":spec/{}", ref_id.to_lowercase());
        let entity = EntityId::from_ident(&ident);
        // Check if this entity has :spec/falsification
        let has_falsification = store
            .entity_datoms(entity)
            .iter()
            .any(|d| d.attribute.as_str() == ":spec/falsification" && d.op == Op::Assert);
        if has_falsification {
            resolved.push((ref_id.clone(), entity));
        } else {
            unresolved.push(ref_id.clone());
        }
    }
    (resolved, unresolved)
}

// ---------------------------------------------------------------------------
// Task queries
// ---------------------------------------------------------------------------

/// Resolve task status from store using lattice-join over all assertions.
///
/// INV-TASK-001: Returns the join (max) of all `:task/status` values for the entity.
pub fn resolve_task_status(store: &Store, entity: EntityId) -> Option<TaskStatus> {
    let mut result: Option<TaskStatus> = None;
    for d in store.entity_datoms(entity) {
        if d.attribute.as_str() == ":task/status" && d.op == Op::Assert {
            if let Value::Keyword(ref kw) = d.value {
                if let Some(status) = TaskStatus::from_keyword(kw) {
                    result = Some(match result {
                        Some(prev) => prev.join(status),
                        None => status,
                    });
                }
            }
        }
    }
    result
}

/// Extract a full TaskSummary from the store for a given entity.
pub fn task_summary(store: &Store, entity: EntityId) -> Option<TaskSummary> {
    let datoms = store.entity_datoms(entity);
    if datoms.is_empty() {
        return None;
    }

    let mut id = None;
    let mut title = None;
    let mut priority = 2i64; // default: medium
    let mut task_type = String::from(":task.type/task");
    let mut created_at = 0u64;
    let mut depends_on = Vec::new();
    let mut traces_to = Vec::new();
    let mut labels = Vec::new();
    let mut source = None;
    let mut close_reason = None;

    for d in &datoms {
        if d.op != Op::Assert {
            continue;
        }
        match d.attribute.as_str() {
            ":task/id" => {
                if let Value::String(ref s) = d.value {
                    id = Some(s.clone());
                }
            }
            ":task/title" => {
                if let Value::String(ref s) = d.value {
                    title = Some(s.clone());
                }
            }
            ":task/priority" => {
                if let Value::Long(n) = d.value {
                    priority = n;
                }
            }
            ":task/type" => {
                if let Value::Keyword(ref k) = d.value {
                    task_type = k.clone();
                }
            }
            ":task/created-at" => {
                if let Value::Long(n) = d.value {
                    created_at = n as u64;
                }
            }
            ":task/depends-on" => {
                if let Value::Ref(e) = d.value {
                    depends_on.push(e);
                }
            }
            ":task/traces-to" => {
                if let Value::Ref(e) = d.value {
                    traces_to.push(e);
                }
            }
            ":task/labels" => {
                if let Value::Keyword(ref k) = d.value {
                    labels.push(k.clone());
                }
            }
            ":task/source" => {
                if let Value::String(ref s) = d.value {
                    source = Some(s.clone());
                }
            }
            ":task/close-reason" => {
                if let Value::String(ref s) = d.value {
                    close_reason = Some(s.clone());
                }
            }
            _ => {}
        }
    }

    let id = id?;
    let title = title?;
    let status = resolve_task_status(store, entity)?;

    Some(TaskSummary {
        entity,
        id,
        title,
        status,
        priority,
        task_type,
        created_at,
        depends_on,
        traces_to,
        labels,
        source,
        close_reason,
    })
}

/// Find all task entities in the store.
pub fn all_tasks(store: &Store) -> Vec<TaskSummary> {
    let mut seen = BTreeSet::new();
    let mut tasks = Vec::new();

    for d in store.datoms() {
        if d.attribute.as_str() == ":task/id" && d.op == Op::Assert && seen.insert(d.entity) {
            if let Some(summary) = task_summary(store, d.entity) {
                tasks.push(summary);
            }
        }
    }

    tasks
}

/// Compute the ready set: open tasks with all dependencies satisfied.
///
/// INV-TASK-003: A task is "ready" iff status = Open AND all `:task/depends-on`
/// targets are either: (a) Closed, (b) not found, or (c) an EPIC (type=epic).
///
/// EPICs are container tasks — their children execute in parallel, not sequentially.
/// A leaf task depending on an EPIC means "belongs to this epic", not "blocked by it."
///
/// INV-TASK-004: Sorted by priority (ascending), then created_at (ascending).
pub fn compute_ready_set(store: &Store) -> Vec<TaskSummary> {
    let tasks = all_tasks(store);

    // Build status + type lookup for all tasks
    let status_map: HashMap<EntityId, TaskStatus> =
        tasks.iter().map(|t| (t.entity, t.status)).collect();
    let type_map: HashMap<EntityId, String> = tasks
        .iter()
        .map(|t| (t.entity, t.task_type.clone()))
        .collect();

    let mut ready: Vec<TaskSummary> = tasks
        .into_iter()
        .filter(|t| {
            t.status == TaskStatus::Open
                && t.depends_on.iter().all(|dep| {
                    // Dependency satisfied if: closed, unknown, or an EPIC (container)
                    let is_closed = status_map.get(dep).is_none_or(|s| *s == TaskStatus::Closed);
                    let is_epic = type_map.get(dep).is_some_and(|ty| *ty == ":task.type/epic");
                    is_closed || is_epic
                })
        })
        .collect();

    // INV-TASK-004: sort by priority (lower = higher priority), then age
    ready.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then(a.created_at.cmp(&b.created_at))
    });

    ready
}

/// Check if adding a dependency edge would create a cycle (INV-TASK-002).
///
/// Returns Err with cycle description if adding from→to would create a cycle.
pub fn check_dependency_acyclicity(
    store: &Store,
    from: EntityId,
    to: EntityId,
) -> Result<(), String> {
    if from == to {
        return Err("self-dependency".to_string());
    }

    // Build adjacency list from existing dependencies + proposed edge
    let mut adj: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
    for d in store.datoms() {
        if d.attribute.as_str() == ":task/depends-on" && d.op == Op::Assert {
            if let Value::Ref(target) = d.value {
                adj.entry(d.entity).or_default().push(target);
            }
        }
    }
    // Add proposed edge
    adj.entry(from).or_default().push(to);

    // BFS cycle detection from `to` — can we reach `from`?
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(to);

    while let Some(node) = queue.pop_front() {
        if node == from {
            return Err(format!(
                "cycle detected: adding dependency would create a loop through {:?}",
                from
            ));
        }
        if visited.insert(node) {
            if let Some(neighbors) = adj.get(&node) {
                for &n in neighbors {
                    queue.push_back(n);
                }
            }
        }
    }

    Ok(())
}

/// Find a task entity by its short ID (e.g., "t-aB3c").
pub fn find_task_by_id(store: &Store, task_id: &str) -> Option<EntityId> {
    for d in store.datoms() {
        if d.attribute.as_str() == ":task/id"
            && d.op == Op::Assert
            && matches!(&d.value, Value::String(s) if s == task_id)
        {
            return Some(d.entity);
        }
    }
    None
}

/// Count tasks by status category.
pub fn task_counts(store: &Store) -> (usize, usize, usize) {
    let tasks = all_tasks(store);
    let open = tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Open)
        .count();
    let in_progress = tasks
        .iter()
        .filter(|t| t.status == TaskStatus::InProgress)
        .count();
    let closed = tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Closed)
        .count();
    (open, in_progress, closed)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// Witnesses: INV-STORE-001, INV-STORE-003, INV-SCHEMA-001, INV-RESOLUTION-001,
//   ADR-STORE-003, ADR-RESOLUTION-001
// (Task module exercises datom construction via the append-only store,
//  content-addressable identity, schema-as-data patterns, and lattice
//  resolution for status monotonicity.)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::AgentId;
    use crate::schema::{full_schema_datoms, genesis_datoms};
    use std::collections::BTreeSet;

    /// Create a BTreeSet with full schema (including Layer 4).
    fn schema_datoms() -> BTreeSet<Datom> {
        let agent = AgentId::from_name("test");
        let genesis_tx = TxId::new(0, 0, agent);
        let mut datoms: BTreeSet<Datom> = BTreeSet::new();
        for d in genesis_datoms(genesis_tx) {
            datoms.insert(d);
        }
        for d in full_schema_datoms(genesis_tx) {
            datoms.insert(d);
        }
        datoms
    }

    /// Create a store with full schema (including Layer 4).
    fn test_store() -> Store {
        Store::from_datoms(schema_datoms())
    }

    /// Rebuild a store from its datoms plus additional datoms.
    fn store_with(store: &Store, extra: impl IntoIterator<Item = Datom>) -> Store {
        let mut datoms: BTreeSet<Datom> = store.datom_set().clone();
        for d in extra {
            datoms.insert(d);
        }
        Store::from_datoms(datoms)
    }

    // Verifies: INV-RESOLUTION-001, ADR-RESOLUTION-001
    // (Task status forms a bounded join-semilattice: lattice resolution for CRDT merge.)
    #[test]
    fn status_lattice_join_is_monotone() {
        // INV-TASK-001: join(a, b) >= a && join(a, b) >= b
        let statuses = [TaskStatus::Open, TaskStatus::InProgress, TaskStatus::Closed];
        for &a in &statuses {
            for &b in &statuses {
                let j = a.join(b);
                assert!(j >= a, "join({a:?}, {b:?}) must be >= {a:?}");
                assert!(j >= b, "join({a:?}, {b:?}) must be >= {b:?}");
            }
        }
    }

    // Verifies: INV-STORE-004, INV-RESOLUTION-001
    // (Commutativity of status lattice join — required for CRDT merge commutativity.)
    #[test]
    fn status_lattice_is_commutative() {
        let statuses = [TaskStatus::Open, TaskStatus::InProgress, TaskStatus::Closed];
        for &a in &statuses {
            for &b in &statuses {
                assert_eq!(
                    a.join(b),
                    b.join(a),
                    "join must be commutative for {a:?}, {b:?}"
                );
            }
        }
    }

    // Verifies: INV-STORE-006, INV-RESOLUTION-001
    // (Idempotency of status lattice join — required for CRDT merge idempotency.)
    #[test]
    fn status_lattice_is_idempotent() {
        let statuses = [TaskStatus::Open, TaskStatus::InProgress, TaskStatus::Closed];
        for &a in &statuses {
            assert_eq!(a.join(a), a, "join(a, a) must equal a for {a:?}");
        }
    }

    // Verifies: INV-STORE-003
    // (Content-addressable identity: same title produces same task ID deterministically.)
    #[test]
    fn task_id_generation_deterministic() {
        let id1 = generate_task_id("Fix harvest noise");
        let id2 = generate_task_id("Fix harvest noise");
        assert_eq!(id1, id2, "same title → same ID");
    }

    // Verifies: INV-STORE-003
    // (Different content produces different IDs — content-addressable identity.)
    #[test]
    fn task_id_different_for_different_titles() {
        let id1 = generate_task_id("Fix harvest noise");
        let id2 = generate_task_id("Implement query engine");
        assert_ne!(id1, id2);
    }

    // Verifies: INV-STORE-001, INV-STORE-003, INV-SCHEMA-001, ADR-STORE-003
    // (Creates task datoms in append-only store using schema-as-data attributes.)
    #[test]
    fn create_and_query_task() {
        let store = test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        let (entity, task_datoms) = create_task_datoms(CreateTaskParams {
            title: "Fix harvest noise",
            description: Some("Reduce false positive completeness gaps"),
            priority: 1,
            task_type: TaskType::Bug,
            tx,
            traces_to: &[],
            labels: &["phase-b".to_string()],
        });

        let store = store_with(&store, task_datoms);

        let summary = task_summary(&store, entity).expect("task should exist");
        assert_eq!(summary.title, "Fix harvest noise");
        assert_eq!(summary.status, TaskStatus::Open);
        assert_eq!(summary.priority, 1);
        assert_eq!(summary.task_type, ":task.type/bug");
    }

    // Verifies: INV-STORE-001, ADR-STORE-003
    // (Ready set computed from append-only store; dependency DAG from datom assertions.)
    #[test]
    fn ready_set_excludes_blocked_tasks() {
        let store = test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        // Create task A (no deps)
        let (entity_a, datoms_a) = create_task_datoms(CreateTaskParams {
            title: "Task A",
            description: None,
            priority: 1,
            task_type: TaskType::Task,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = store_with(&store, datoms_a);

        // Create task B (depends on A)
        let (_, datoms_b) = create_task_datoms(CreateTaskParams {
            title: "Task B",
            description: None,
            priority: 0,
            task_type: TaskType::Task,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = store_with(&store, datoms_b);

        // Add dependency B → A
        let entity_b = find_task_by_id(&store, &generate_task_id("Task B")).unwrap();
        let store = store_with(&store, vec![dep_add_datom(entity_b, entity_a, tx)]);

        let ready = compute_ready_set(&store);
        assert_eq!(ready.len(), 1, "only task A should be ready");
        assert_eq!(ready[0].title, "Task A");
    }

    // Verifies: INV-STORE-001, ADR-STORE-003
    // (Priority-sorted ready set from append-only store data.)
    #[test]
    fn ready_set_sorted_by_priority() {
        let mut store = test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        // Create tasks at different priorities
        for (title, priority) in [("Low priority", 3), ("High priority", 0), ("Medium", 2)] {
            let (_, task_datoms) = create_task_datoms(CreateTaskParams {
                title,
                description: None,
                priority,
                task_type: TaskType::Task,
                tx,
                traces_to: &[],
                labels: &[],
            });
            store = store_with(&store, task_datoms);
        }

        let ready = compute_ready_set(&store);
        assert_eq!(ready[0].title, "High priority");
        assert_eq!(ready[1].title, "Medium");
        assert_eq!(ready[2].title, "Low priority");
    }

    // Verifies: NEG-STORE-001
    // (Self-dependency violates DAG constraint — negative case for store integrity.)
    #[test]
    fn cycle_detection_rejects_self_dep() {
        let store = test_store();
        let entity = EntityId::from_ident(":task/t-xxxx");
        assert!(check_dependency_acyclicity(&store, entity, entity).is_err());
    }

    // Verifies: NEG-STORE-001
    // (Cycle in dependency graph violates DAG constraint.)
    #[test]
    fn cycle_detection_rejects_cycle() {
        let store = test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        let (entity_a, datoms_a) = create_task_datoms(CreateTaskParams {
            title: "Task A",
            description: None,
            priority: 1,
            task_type: TaskType::Task,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let (entity_b, datoms_b) = create_task_datoms(CreateTaskParams {
            title: "Task B",
            description: None,
            priority: 1,
            task_type: TaskType::Task,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = store_with(&store, datoms_a);
        let store = store_with(&store, datoms_b);

        // A depends on B (existing)
        let store = store_with(&store, vec![dep_add_datom(entity_a, entity_b, tx)]);

        // Adding B depends on A would create a cycle
        assert!(check_dependency_acyclicity(&store, entity_b, entity_a).is_err());
    }

    // Verifies: INV-STORE-001, INV-SCHEMA-001
    // (set_attribute_datom correctly maps user-friendly names to datom attributes.)
    #[test]
    fn set_attribute_priority() {
        let entity = EntityId::from_ident(":task/t-test1234");
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        let datom = set_attribute_datom(entity, "priority", "0", tx).unwrap();
        assert_eq!(datom.attribute.as_str(), ":task/priority");
        assert_eq!(datom.value, Value::Long(0));

        // Out of range
        assert!(set_attribute_datom(entity, "priority", "5", tx).is_err());
        // Not a number
        assert!(set_attribute_datom(entity, "priority", "high", tx).is_err());
    }

    // Verifies: INV-STORE-001, INV-SCHEMA-001
    // (set_attribute_datom maps status to keyword via TaskStatus lattice.)
    #[test]
    fn set_attribute_status() {
        let entity = EntityId::from_ident(":task/t-test1234");
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        let datom = set_attribute_datom(entity, "status", "in-progress", tx).unwrap();
        assert_eq!(datom.attribute.as_str(), ":task/status");
        assert_eq!(
            datom.value,
            Value::Keyword(":task.status/in-progress".to_string())
        );

        assert!(set_attribute_datom(entity, "status", "invalid", tx).is_err());
    }

    // Verifies: INV-STORE-001, INV-SCHEMA-001
    // (set_attribute_datom maps type to keyword via TaskType enum.)
    #[test]
    fn set_attribute_type() {
        let entity = EntityId::from_ident(":task/t-test1234");
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        let datom = set_attribute_datom(entity, "type", "bug", tx).unwrap();
        assert_eq!(datom.attribute.as_str(), ":task/type");
        assert_eq!(datom.value, Value::Keyword(":task.type/bug".to_string()));

        assert!(set_attribute_datom(entity, "type", "invalid", tx).is_err());
    }

    // Verifies: INV-STORE-001, INV-SCHEMA-001
    // (set_attribute_datom maps title to string value.)
    #[test]
    fn set_attribute_title() {
        let entity = EntityId::from_ident(":task/t-test1234");
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        let datom = set_attribute_datom(entity, "title", "New title", tx).unwrap();
        assert_eq!(datom.attribute.as_str(), ":task/title");
        assert_eq!(datom.value, Value::String("New title".to_string()));

        // Empty title rejected
        assert!(set_attribute_datom(entity, "title", "", tx).is_err());
    }

    // Verifies: INV-INTERFACE-001
    // (Unknown attribute names produce clear error messages.)
    #[test]
    fn set_attribute_unknown_rejected() {
        let entity = EntityId::from_ident(":task/t-test1234");
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        let err = set_attribute_datom(entity, "color", "red", tx).unwrap_err();
        assert!(err.contains("unknown attribute"), "got: {err}");
    }

    // Verifies: INV-STORE-001, INV-RESOLUTION-001, ADR-RESOLUTION-001
    // (Closing a task asserts new datoms — append-only status advancement via lattice join.)
    #[test]
    fn close_task_advances_status() {
        let store = test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        let (entity, task_datoms) = create_task_datoms(CreateTaskParams {
            title: "Task to close",
            description: None,
            priority: 1,
            task_type: TaskType::Task,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = store_with(&store, task_datoms);

        assert_eq!(resolve_task_status(&store, entity), Some(TaskStatus::Open));

        let tx2 = TxId::new(2, 0, agent);
        let store = store_with(&store, close_task_datoms(entity, "Done", tx2));

        // INV-TASK-001: Lattice join yields Closed
        assert_eq!(
            resolve_task_status(&store, entity),
            Some(TaskStatus::Closed)
        );
    }

    // -----------------------------------------------------------------------
    // SFE-2.1: parse_spec_refs
    // -----------------------------------------------------------------------

    // Verifies: INV-TASK-005
    // (Spec ref extraction from task titles — sorted, deduplicated results.)
    #[test]
    fn parse_spec_refs_extracts_multiple() {
        let refs = parse_spec_refs("Fix merge (INV-MERGE-001, ADR-MERGE-005)");
        // BTreeSet-based: sorted lexicographically
        assert_eq!(refs, vec!["ADR-MERGE-005", "INV-MERGE-001"]);
    }

    #[test]
    fn parse_spec_refs_extracts_neg() {
        let refs = parse_spec_refs("Handle NEG-STORE-001 violation");
        assert_eq!(refs, vec!["NEG-STORE-001"]);
    }

    #[test]
    fn parse_spec_refs_no_refs() {
        let refs = parse_spec_refs("No spec refs here");
        assert!(refs.is_empty());
    }

    #[test]
    fn parse_spec_refs_deduplicates() {
        let refs = parse_spec_refs("Multiple INV-STORE-001 and INV-STORE-001 duplicates");
        assert_eq!(refs, vec!["INV-STORE-001"]);
    }

    #[test]
    fn parse_spec_refs_mixed_types() {
        let refs = parse_spec_refs("INV-QUERY-001 + ADR-GUIDANCE-013 + NEG-MERGE-002");
        // Sorted: ADR- < INV- < NEG-
        assert_eq!(
            refs,
            vec!["ADR-GUIDANCE-013", "INV-QUERY-001", "NEG-MERGE-002"]
        );
    }

    #[test]
    fn parse_spec_refs_accepts_varying_digit_counts() {
        // One or more digits is valid; sorted lexicographically
        let refs = parse_spec_refs("INV-STORE-01 and INV-STORE-0001");
        assert_eq!(refs, vec!["INV-STORE-0001", "INV-STORE-01"]);
    }

    #[test]
    fn parse_spec_refs_ignores_lowercase_namespace() {
        let refs = parse_spec_refs("INV-store-001");
        assert!(refs.is_empty());
    }

    // -----------------------------------------------------------------------
    // SFE-2.2: resolve_spec_refs
    // -----------------------------------------------------------------------

    // Verifies: INV-TASK-005
    // (Existing spec element with :spec/falsification resolves; nonexistent doesn't.)
    #[test]
    fn resolve_spec_refs_resolves_existing() {
        let store = test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        // Create a spec element entity with :spec/falsification
        let spec_entity = EntityId::from_ident(":spec/inv-store-001");
        let spec_datoms = vec![
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":spec/inv-store-001".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":spec/falsification"),
                Value::String("Any mutation of existing datom".to_string()),
                tx,
                Op::Assert,
            ),
        ];
        let store = store_with(&store, spec_datoms);

        let refs = vec!["INV-STORE-001".to_string(), "INV-FAKE-999".to_string()];
        let (resolved, unresolved) = resolve_spec_refs(&store, &refs);

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0, "INV-STORE-001");
        assert_eq!(resolved[0].1, spec_entity);

        assert_eq!(unresolved, vec!["INV-FAKE-999"]);
    }

    #[test]
    fn resolve_spec_refs_empty_input() {
        let store = test_store();
        let (resolved, unresolved) = resolve_spec_refs(&store, &[]);
        assert!(resolved.is_empty());
        assert!(unresolved.is_empty());
    }

    #[test]
    fn resolve_spec_refs_requires_falsification() {
        let store = test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        // Create a spec entity WITHOUT :spec/falsification (observation, not formal)
        let spec_entity = EntityId::from_ident(":spec/inv-store-002");
        let spec_datoms = vec![Datom::new(
            spec_entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":spec/inv-store-002".to_string()),
            tx,
            Op::Assert,
        )];
        let store = store_with(&store, spec_datoms);

        let refs = vec!["INV-STORE-002".to_string()];
        let (resolved, unresolved) = resolve_spec_refs(&store, &refs);

        assert!(
            resolved.is_empty(),
            "entity without falsification should not resolve"
        );
        assert_eq!(unresolved, vec!["INV-STORE-002"]);
    }
}
