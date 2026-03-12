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
            TaskType::Epic => ":task.type/epic",
            TaskType::Question => ":task.type/question",
            TaskType::Docs => ":task.type/docs",
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
/// Format: `t-{4 chars}` derived from BLAKE3 hash of the title.
/// Deterministic: same title → same ID.
pub fn generate_task_id(title: &str) -> String {
    let hash = blake3::hash(title.as_bytes());
    let hex = hash.to_hex();
    format!("t-{}", &hex[..4])
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

/// Compute the ready set: open tasks with all dependencies closed.
///
/// INV-TASK-003: A task is "ready" iff status = Open AND all `:task/depends-on`
/// targets have status = Closed.
///
/// INV-TASK-004: Sorted by priority (ascending), then created_at (ascending).
pub fn compute_ready_set(store: &Store) -> Vec<TaskSummary> {
    let tasks = all_tasks(store);

    // Build status lookup for all tasks
    let status_map: HashMap<EntityId, TaskStatus> =
        tasks.iter().map(|t| (t.entity, t.status)).collect();

    let mut ready: Vec<TaskSummary> = tasks
        .into_iter()
        .filter(|t| {
            t.status == TaskStatus::Open
                && t.depends_on.iter().all(|dep| {
                    status_map
                        .get(dep)
                        .map_or(true, |s| *s == TaskStatus::Closed)
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
}
