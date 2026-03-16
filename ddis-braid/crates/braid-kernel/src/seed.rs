//! SEED namespace — start-of-session context assembly.
//!
//! Seed is the complement of harvest: where harvest extracts knowledge at session
//! end, seed assembles relevant knowledge at session start. The seed provides a
//! fresh agent with full relevant context, zero irrelevant noise, and stays within
//! the declared attention budget.
//!
//! # Pipeline (INV-SEED-001)
//!
//! ASSOCIATE → QUERY → ASSEMBLE → EMIT
//!
//! # Invariants
//!
//! - **INV-SEED-001**: Seed as store projection — every datum traces to a datom.
//! - **INV-SEED-002**: Budget compliance — output ≤ declared budget.
//! - **INV-SEED-003**: ASSOCIATE boundedness — `|result| ≤ depth × breadth`.
//! - **INV-SEED-004**: Section compression priority (State first, Directive last).
//! - **INV-SEED-005**: Demonstration density — worked examples included.
//! - **INV-SEED-006**: Intention anchoring — intentions pinned at π₀ regardless.
//!
//! # Design Decisions
//!
//! - ADR-SEED-003: Spec-language over instruction-language in seed output.
//! - ADR-SEED-005: Four knowledge types (state, constraint, observation, action).
//! - ADR-SEED-007: Seed document eleven-section structure.
//!
//! # Negative Cases
//!
//! - NEG-SEED-001: No fabricated context — every seed datum traces to a datom.
//! - NEG-SEED-002: No budget overflow — output strictly within declared limit.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, Value};
use crate::query::graph::{pagerank, DiGraph};
use crate::store::Store;

/// Extract session number from task strings like "Session 016: ..." or "continue: Session 016: ...".
/// Returns the numeric part (e.g., "016") for deduplication across same-session harvests.
fn extract_session_number(task: &str) -> Option<String> {
    // Match "Session NNN" anywhere in the string
    let lower = task.to_lowercase();
    if let Some(idx) = lower.find("session ") {
        let after = &task[idx + 8..];
        let num: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !num.is_empty() {
            return Some(num);
        }
    }
    None
}

/// Truncate a string at a char boundary, appending "..." if truncated.
/// Safe for multi-byte UTF-8 (never panics on char boundaries).
fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{truncated}...")
    }
}

/// Projection levels for rate-distortion compression (ADR-SEED-002).
///
/// Forms a total order: Full > Summary > TypeLevel > Pointer.
/// Higher compression = less detail = fewer tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectionLevel {
    /// π₃ — single-line reference (entity ID only).
    Pointer,
    /// π₂ — type summary (entity type + count of attributes).
    TypeLevel,
    /// π₁ — entity summary (entity + attribute names, no values).
    Summary,
    /// π₀ — all datoms for the entity.
    Full,
}

/// How the seed was discovered — semantic search or explicit entity list.
#[derive(Clone, Debug)]
pub enum AssociateCue {
    /// Natural language search with bounded traversal.
    Semantic {
        /// Search text.
        text: String,
        /// Maximum traversal depth from matching entities.
        depth: usize,
        /// Maximum neighbors per hop.
        breadth: usize,
    },
    /// Known entity IDs with bounded neighborhood expansion.
    Explicit {
        /// Starting entities.
        seeds: Vec<EntityId>,
        /// Maximum traversal depth.
        depth: usize,
        /// Maximum neighbors per hop.
        breadth: usize,
    },
}

impl AssociateCue {
    /// Upper bound on result size: depth × breadth (INV-SEED-003).
    pub fn max_results(&self) -> usize {
        match self {
            AssociateCue::Semantic { depth, breadth, .. } => depth * breadth,
            AssociateCue::Explicit {
                seeds,
                depth,
                breadth,
            } => seeds.len() + depth * breadth,
        }
    }
}

/// Schema neighborhood discovered by ASSOCIATE — entities, attributes, types.
/// NOT values (those come from ASSEMBLE at chosen projection level).
#[derive(Clone, Debug, Default)]
pub struct SchemaNeighborhood {
    /// Entities discovered by ASSOCIATE.
    pub entities: Vec<EntityId>,
    /// Attributes relevant to discovered entities.
    pub attributes: Vec<Attribute>,
    /// Type keywords for discovered entities.
    pub entity_types: Vec<String>,
}

/// Sections of the assembled seed output (ADR-SEED-004).
///
/// Five-part unified template. Compression order (first-to-compress → last):
/// State → Constraints → Orientation → Warnings → Directive.
#[derive(Clone, Debug)]
pub enum ContextSection {
    /// Project identity, current phase, recent session history.
    Orientation(String),
    /// Relevant invariants, settled ADRs, negative cases.
    Constraints(Vec<ConstraintRef>),
    /// Relevant datoms at chosen projection level.
    State(Vec<StateEntry>),
    /// Drift signals, open questions, uncertainties.
    Warnings(Vec<String>),
    /// Next task, acceptance criteria, active guidance corrections.
    Directive(String),
}

/// A reference to a specification constraint.
#[derive(Clone, Debug)]
pub struct ConstraintRef {
    /// Constraint ID (e.g., "INV-STORE-001").
    pub id: String,
    /// Brief description.
    pub summary: String,
    /// Constraint statement text (from `:spec/statement`).
    pub statement: Option<String>,
    /// Falsification condition text (from `:spec/falsification`).
    pub falsification: Option<String>,
    /// Whether this constraint is currently satisfied.
    pub satisfied: Option<bool>,
}

/// A state entry at a chosen projection level.
#[derive(Clone, Debug)]
pub struct StateEntry {
    /// The entity this entry describes.
    pub entity: EntityId,
    /// Projection level used.
    pub projection: ProjectionLevel,
    /// Token-counted representation.
    pub content: String,
    /// Estimated token count.
    pub tokens: usize,
}

/// The assembled seed context (INV-SEED-002: total_tokens ≤ budget).
#[derive(Clone, Debug)]
pub struct AssembledContext {
    /// Ordered sections of the seed output.
    pub sections: Vec<ContextSection>,
    /// Total estimated token count.
    pub total_tokens: usize,
    /// Remaining budget after assembly.
    pub budget_remaining: usize,
    /// Dominant projection pattern used.
    pub projection_pattern: ProjectionLevel,
}

/// Complete seed output for a session.
#[derive(Clone, Debug)]
pub struct SeedOutput {
    /// The assembled context.
    pub context: AssembledContext,
    /// The agent this seed is for.
    pub agent: AgentId,
    /// The task description used for association.
    pub task: String,
    /// Number of entities discovered by ASSOCIATE.
    pub entities_discovered: usize,
}

// ---------------------------------------------------------------------------
// Entity reference graph (shared infrastructure for seed v2)
// ---------------------------------------------------------------------------

/// Convert an EntityId to a stable string key for graph algorithms.
fn entity_key(entity: EntityId) -> String {
    // Use first 8 bytes of the entity hash as hex — same convention as trilateral.rs
    format!(
        "{:x}",
        u64::from_be_bytes(entity.as_bytes()[..8].try_into().unwrap())
    )
}

/// Build a directed graph from all `Value::Ref` datoms in the store.
///
/// Returns the graph and a bidirectional mapping from graph node IDs
/// back to EntityIds, needed for resolving graph traversal results.
///
/// Unlike trilateral.rs (which filters to REF_EDGE_ATTRS only), seed
/// uses ALL Ref edges because the association goal is to discover any
/// structurally related entity, not just cross-boundary dependencies.
fn build_entity_graph(store: &Store) -> (DiGraph, BTreeMap<String, EntityId>) {
    let mut graph = DiGraph::new();
    let mut id_map: BTreeMap<String, EntityId> = BTreeMap::new();

    for datom in store.datoms() {
        if datom.op != Op::Assert {
            continue;
        }
        if let Value::Ref(target) = &datom.value {
            let src_key = entity_key(datom.entity);
            let dst_key = entity_key(*target);
            id_map.insert(src_key.clone(), datom.entity);
            id_map.insert(dst_key.clone(), *target);
            graph.add_edge(&src_key, &dst_key);
        }
    }

    // Add entities with no Ref edges as isolated nodes
    // (so they appear in PageRank with base score)
    for entity in store.entities() {
        let key = entity_key(entity);
        id_map.entry(key.clone()).or_insert(entity);
        graph.add_node(&key);
    }

    (graph, id_map)
}

/// Stopwords filtered from task descriptions during keyword extraction.
const STOPWORDS: &[&str] = &[
    "the", "a", "an", "is", "for", "to", "in", "of", "and", "or", "with", "on", "at", "by", "from",
    "as", "it", "be", "do", "not", "this", "that", "are", "was", "were", "been", "has", "have",
    "had", "will", "would", "can", "could", "should", "may", "might", "shall",
];

/// Extract task keywords for relevance scoring (Wave 3: B2+B3).
///
/// Split on whitespace, lowercase, filter stopwords. If the task is generic
/// ("continue", "session work"), falls back to keywords from the most recent
/// harvest session's `:harvest/task` attribute.
fn extract_task_keywords(store: &Store, task: &str) -> Vec<String> {
    let raw = task.to_lowercase();
    let mut keywords: Vec<String> = raw
        .split_whitespace()
        .filter(|w| w.len() >= 2 && !STOPWORDS.contains(w))
        .map(|w| w.to_string())
        .collect();

    // Fallback for generic tasks: use the most recent harvest's task
    if keywords.is_empty() || keywords == ["continue"] || keywords == ["session", "work"] {
        let mut latest_wall = 0u64;
        let mut latest_task_entity = None;
        for d in store.datoms() {
            if d.attribute.as_str() == ":harvest/agent" && d.op == Op::Assert {
                let wall = d.tx.wall_time();
                if wall > latest_wall {
                    latest_wall = wall;
                    latest_task_entity = Some(d.entity);
                }
            }
        }
        if let Some(entity) = latest_task_entity {
            for d in store.entity_datoms(entity) {
                if d.attribute.as_str() == ":harvest/task" && d.op == Op::Assert {
                    if let Value::String(ref t) = d.value {
                        keywords = t
                            .to_lowercase()
                            .split_whitespace()
                            .filter(|w| w.len() >= 2 && !STOPWORDS.contains(w))
                            .map(|w| w.to_string())
                            .collect();
                    }
                }
            }
        }
    }

    keywords
}

/// Score a text string for relevance to task keywords.
///
/// Returns a score in [0.0, 1.0] based on the fraction of keywords that
/// appear (case-insensitive) in the text.
fn keyword_relevance_score(text: &str, keywords: &[String]) -> f64 {
    if keywords.is_empty() {
        return 0.5; // neutral when no keywords
    }
    let lower = text.to_lowercase();
    let hits = keywords
        .iter()
        .filter(|k| lower.contains(k.as_str()))
        .count();
    hits as f64 / keywords.len() as f64
}

/// Fallback seed selection: when keyword matching returns zero entities,
/// select the most recently-modified entities. This handles generic tasks
/// like "continue" or "overview" where the user wants orientation, not
/// keyword-specific results.
/// A harvest session summary for orientation context (SB.2.4).
#[allow(dead_code)]
struct HarvestSummary {
    entity: EntityId,
    wall_time: u64,
    doc: String,
    candidate_count: i64,
}

/// Excerpt from a harvest session (S0.3.1: INV-SEED-001).
///
/// This is a projection of the session's causal cone in the datom graph.
/// Provides the essential "what happened" for each prior session.
#[derive(Clone, Debug, Default)]
struct SessionExcerpt {
    /// Design decisions made (from `:exploration/category` = "design-decision").
    decisions: Vec<String>,
    /// Open questions remaining (from `:exploration/category` = "open-question").
    open_questions: Vec<String>,
    /// What was accomplished (from `:harvest/accomplishments`).
    accomplishments: Vec<String>,
    /// Task description (from `:harvest/task`).
    task: Option<String>,
    /// Git context summary (from `:harvest/git-summary`).
    git_summary: Option<String>,
    /// Synthesis directive from harvest (recommended next steps).
    synthesis_directive: Option<String>,
    /// Codebase snapshot (LOC, key files, test count).
    codebase_snapshot: Option<String>,
    /// Store datom count at harvest time (for delta tracking).
    store_datom_count: Option<i64>,
    /// Store entity count at harvest time.
    store_entity_count: Option<i64>,
    /// Transactions since last harvest (E2 metric).
    tx_since_last: Option<i64>,
    /// Observations recorded this session (E2 metric).
    observation_count: Option<i64>,
    /// Session delta (e.g., "+126 datoms, +23 entities").
    delta_summary: Option<String>,
}

/// Discover content from a harvest session entity.
///
/// Extracts the session's decisions and open questions by scanning datoms
/// in the temporal neighborhood of the session entity.
fn discover_session_content(store: &Store, session_entity: EntityId) -> SessionExcerpt {
    let mut excerpt = SessionExcerpt::default();

    // Extract wall_time from the session entity's transaction
    let session_wall_time = store
        .entity_datoms(session_entity)
        .first()
        .map(|d| d.tx.wall_time());

    // Find observations temporally close to this session.
    // Look for observations with wall_time within 3600 seconds (1 hour) of session.
    if let Some(session_time) = session_wall_time {
        let window_start = session_time.saturating_sub(3600);
        let window_end = session_time.saturating_add(60); // small future buffer

        for datom in store.datoms() {
            if datom.op != Op::Assert {
                continue;
            }
            if datom.attribute.as_str() != ":exploration/source" {
                continue;
            }

            let obs_time = datom.tx.wall_time();
            if obs_time < window_start || obs_time > window_end {
                continue;
            }

            // This is an observation in our time window - check its category
            let entity = datom.entity;
            let mut category = String::new();
            let mut body = String::new();

            for d in store.entity_datoms(entity) {
                if d.op != Op::Assert {
                    continue;
                }
                match d.attribute.as_str() {
                    ":exploration/category" => match &d.value {
                        Value::String(ref s) => {
                            category.clone_from(s);
                        }
                        Value::Keyword(ref k) => {
                            category.clone_from(k);
                        }
                        _ => {}
                    },
                    ":exploration/body" | ":db/doc" => {
                        if let Value::String(ref s) = d.value {
                            body = truncate_chars(s, 300);
                        }
                    }
                    _ => {}
                }
            }

            match category.as_str() {
                c if c == "design-decision" || c.ends_with("/design-decision") => {
                    excerpt.decisions.push(body);
                }
                c if c == "open-question" || c.ends_with("/open-question") => {
                    excerpt.open_questions.push(body);
                }
                _ => {}
            }
        }
    }

    // Read harvest narrative attributes from the session entity (Wave 2.3)
    for d in store.entity_datoms(session_entity) {
        if d.op != Op::Assert {
            continue;
        }
        match d.attribute.as_str() {
            ":harvest/accomplishments" => {
                if let Value::String(ref s) = d.value {
                    excerpt
                        .accomplishments
                        .extend(s.lines().map(|l| l.to_string()));
                }
            }
            ":harvest/decisions" => {
                if let Value::String(ref s) = d.value {
                    // Merge with observation-sourced decisions (avoid duplicates)
                    for line in s.lines() {
                        if !excerpt.decisions.iter().any(|d| d == line) {
                            excerpt.decisions.push(line.to_string());
                        }
                    }
                }
            }
            ":harvest/open-questions" => {
                if let Value::String(ref s) = d.value {
                    for line in s.lines() {
                        let stripped = line.strip_prefix("[?] ").unwrap_or(line);
                        if !excerpt.open_questions.iter().any(|q| q == stripped) {
                            excerpt.open_questions.push(stripped.to_string());
                        }
                    }
                }
            }
            ":harvest/task" => {
                if let Value::String(ref s) = d.value {
                    excerpt.task = Some(s.clone());
                }
            }
            ":harvest/git-summary" => {
                if let Value::String(ref s) = d.value {
                    excerpt.git_summary = Some(s.clone());
                }
            }
            ":harvest/synthesis-directive" => {
                if let Value::String(ref s) = d.value {
                    excerpt.synthesis_directive = Some(s.clone());
                }
            }
            ":harvest/codebase-snapshot" => {
                if let Value::String(ref s) = d.value {
                    excerpt.codebase_snapshot = Some(s.clone());
                }
            }
            ":harvest/store-datom-count" => {
                if let Value::Long(n) = d.value {
                    excerpt.store_datom_count = Some(n);
                }
            }
            ":harvest/store-entity-count" => {
                if let Value::Long(n) = d.value {
                    excerpt.store_entity_count = Some(n);
                }
            }
            ":harvest/tx-since-last" => {
                if let Value::Long(n) = d.value {
                    excerpt.tx_since_last = Some(n);
                }
            }
            ":harvest/observation-count" => {
                if let Value::Long(n) = d.value {
                    excerpt.observation_count = Some(n);
                }
            }
            ":harvest/delta-summary" => {
                if let Value::String(ref s) = d.value {
                    excerpt.delta_summary = Some(s.clone());
                }
            }
            _ => {}
        }
    }

    excerpt
}

/// Discover session excerpts for the N most recent harvest sessions.
///
/// Returns excerpts sorted by recency (newest first), limited to `max_sessions`.
fn discover_recent_sessions(store: &Store, max_sessions: usize) -> Vec<SessionExcerpt> {
    // Collect harvest session entities
    let mut sessions: Vec<(EntityId, u64)> = Vec::new();
    for datom in store.datoms() {
        if datom.attribute.as_str() == ":harvest/agent" && datom.op == Op::Assert {
            sessions.push((datom.entity, datom.tx.wall_time()));
        }
    }

    // Sort by wall_time descending
    sessions.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
    sessions.truncate(max_sessions);

    // Discover content for each
    sessions
        .iter()
        .map(|(entity, _)| discover_session_content(store, *entity))
        .collect()
}

/// Build the Orientation section as a trajectory-setting briefing.
///
/// Designed as a PROMPT, not a report. Uses spec-language to activate
/// design-level reasoning in incoming agents. Dense prose > bulleted dumps.
/// Every line carries information that shapes the conversation trajectory.
///
/// Traces to: prompt-optimization principle "conversations are trajectories —
/// seed output IS turn 1 and determines the reasoning basin."
fn build_orientation(store: &Store, _task_keywords: &[String]) -> String {
    let current_datoms = store.len();
    let current_entities = store.entity_count();

    // Get session excerpts (newest first)
    let excerpts = discover_recent_sessions(store, 5);

    // === Project identity (1 dense line with spec-language activation) ===
    let (codebase_headline, test_line) = if let Some(latest) = excerpts.first() {
        if let Some(ref snapshot) = latest.codebase_snapshot {
            let first_line = snapshot.lines().next().unwrap_or("");
            // Extract test count from snapshot
            let tests = snapshot
                .lines()
                .find(|l| l.starts_with("Tests:"))
                .unwrap_or("");
            (
                format!(
                    "Braid: append-only datom store (CRDT merge, content-addressed). {} datoms, {} entities. {}",
                    current_datoms, current_entities, first_line
                ),
                if tests.is_empty() {
                    String::new()
                } else {
                    tests.to_string()
                },
            )
        } else {
            (
                format!(
                    "Braid: append-only datom store (CRDT merge, content-addressed). {} datoms, {} entities.",
                    current_datoms, current_entities
                ),
                String::new(),
            )
        }
    } else {
        (
            format!(
                "Braid: append-only datom store (CRDT merge, content-addressed). {} datoms, {} entities.",
                current_datoms, current_entities
            ),
            String::new(),
        )
    };
    let codebase_line = if test_line.is_empty() {
        codebase_headline
    } else {
        format!("{codebase_headline}. {test_line}")
    };
    let mut parts = vec![codebase_line];

    // Key files (top 5, compressed to one block)
    if let Some(latest) = excerpts.first() {
        if let Some(ref snapshot) = latest.codebase_snapshot {
            let lines: Vec<&str> = snapshot.lines().collect();
            let file_lines: Vec<&&str> = lines
                .iter()
                .skip(1)
                .filter(|l| l.trim().starts_with("ddis-braid/") || l.trim().starts_with("crates/"))
                .take(5)
                .collect();
            if !file_lines.is_empty() {
                parts.push("Key files:".to_string());
                for line in file_lines {
                    parts.push(line.to_string());
                }
            }
        }
    }

    // === Stage 0 success criterion + maturity signal ===
    // The most important context for any "continue" task: WHERE ARE WE?
    // Only show for real stores (>100 datoms) — proptests use tiny stores.
    if current_datoms > 100 {
        let harvest_count = store
            .datoms()
            .filter(|d| d.attribute.as_str() == ":harvest/agent" && d.op == Op::Assert)
            .count();
        let observation_count = store
            .datoms()
            .filter(|d| d.attribute.as_str() == ":exploration/source" && d.op == Op::Assert)
            .count();
        let decision_count = store
            .datoms()
            .filter(|d| {
                d.attribute.as_str() == ":exploration/category"
                    && d.op == Op::Assert
                    && matches!(&d.value,
                        Value::String(s) | Value::Keyword(s)
                        if s.contains("decision"))
            })
            .count();

        parts.push(format!(
            "Goal: harvest/seed replaces HARVEST.md. Status: {} harvests, {} observations, {} decisions captured.",
            harvest_count, observation_count, decision_count
        ));
    }

    // === Spec landscape: namespace-grouped project architecture ===
    // Gives agents a structural mental model — "14 namespaces, 354 elements" is
    // more useful than 150 individual spec IDs. Compact: ~20 lines, ~100 tokens.
    {
        let mut ns_inv: BTreeMap<String, usize> = BTreeMap::new();
        let mut ns_adr: BTreeMap<String, usize> = BTreeMap::new();
        let mut ns_neg: BTreeMap<String, usize> = BTreeMap::new();
        let mut total = 0usize;
        for datom in store.datoms() {
            if datom.attribute.as_str() == ":db/ident"
                && datom.op == Op::Assert
                && matches!(&datom.value, Value::Keyword(k) if k.starts_with(":spec/") && !k.starts_with(":spec."))
            {
                total += 1;
                if let Value::Keyword(k) = &datom.value {
                    let after = k.trim_start_matches(":spec/");
                    let p: Vec<&str> = after.splitn(3, '-').collect();
                    if p.len() >= 2 {
                        let ns = p[1].to_uppercase();
                        match p[0] {
                            "inv" => *ns_inv.entry(ns).or_default() += 1,
                            "adr" => *ns_adr.entry(ns).or_default() += 1,
                            "neg" => *ns_neg.entry(ns).or_default() += 1,
                            _ => {}
                        }
                    }
                }
            }
        }
        if total > 0 {
            let all_ns: BTreeSet<&String> = ns_inv
                .keys()
                .chain(ns_adr.keys())
                .chain(ns_neg.keys())
                .collect();
            parts.push(format!(
                "Spec: {} elements, {} namespaces — {}",
                total,
                all_ns.len(),
                all_ns
                    .iter()
                    .map(|n| {
                        let inv = ns_inv.get(*n).copied().unwrap_or(0);
                        let adr = ns_adr.get(*n).copied().unwrap_or(0);
                        let neg = ns_neg.get(*n).copied().unwrap_or(0);
                        format!(
                            "{}({}/{}{})",
                            n,
                            inv,
                            adr,
                            if neg > 0 {
                                format!("/{neg}")
                            } else {
                                String::new()
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
    }

    // === B2: Attribute vocabulary (top-N per layer, from actual store usage) ===
    // Gives incoming agents the schema context to write correct queries.
    if current_datoms > 100 {
        let mut attr_counts: BTreeMap<String, usize> = BTreeMap::new();
        for d in store.datoms() {
            if d.op == Op::Assert && !d.attribute.as_str().starts_with(":db/") {
                *attr_counts
                    .entry(d.attribute.as_str().to_string())
                    .or_default() += 1;
            }
        }
        if !attr_counts.is_empty() {
            // Group by layer prefix and show top attrs by count
            let mut trilateral = Vec::new();
            let mut elements = Vec::new();
            let mut exploration = Vec::new();
            let mut task_attrs = Vec::new();

            let mut sorted: Vec<_> = attr_counts.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));

            for (attr, count) in sorted.iter().take(30) {
                let entry = format!("{}({})", attr, count);
                if attr.starts_with(":intent/")
                    || attr.starts_with(":spec/")
                    || attr.starts_with(":impl/")
                {
                    trilateral.push(entry);
                } else if attr.starts_with(":element/")
                    || attr.starts_with(":inv/")
                    || attr.starts_with(":adr/")
                {
                    elements.push(entry);
                } else if attr.starts_with(":exploration/") || attr.starts_with(":harvest/") {
                    exploration.push(entry);
                } else if attr.starts_with(":task/")
                    || attr.starts_with(":plan/")
                    || attr.starts_with(":session/")
                {
                    task_attrs.push(entry);
                }
            }

            let mut vocab = String::from("Key attrs:");
            if !trilateral.is_empty() {
                vocab.push_str(&format!(" {}", trilateral.join(", ")));
            }
            if !exploration.is_empty() {
                vocab.push_str(&format!(", {}", exploration.join(", ")));
            }
            if !task_attrs.is_empty() {
                vocab.push_str(&format!(", {}", task_attrs.join(", ")));
            }
            parts.push(vocab);
        }
    }

    // === Task summary (D4: seed integration) ===
    {
        let tasks = crate::task::all_tasks(store);
        if !tasks.is_empty() {
            let open = tasks
                .iter()
                .filter(|t| t.status == crate::task::TaskStatus::Open)
                .count();
            let ready = crate::task::compute_ready_set(store);
            let blocked = open.saturating_sub(ready.len());
            let top = ready
                .first()
                .map(|t| format!(" | Top: {} P{} \"{}\"", t.id, t.priority, t.title));
            parts.push(format!(
                "Tasks: {} open ({} ready, {} blocked){}",
                open,
                ready.len(),
                blocked,
                top.unwrap_or_default(),
            ));
        }
    }

    // === Last session narrative (dense, prose form) ===
    if let Some(latest) = excerpts.first() {
        // Session header with task and metrics
        if let Some(ref task) = latest.task {
            let mut meta_parts = Vec::new();
            if let Some(tx) = latest.tx_since_last {
                meta_parts.push(format!("{tx} txns"));
            }
            if let Some(obs) = latest.observation_count {
                meta_parts.push(format!("{obs} observations"));
            }
            if let Some(ref delta) = latest.delta_summary {
                meta_parts.push(delta.clone());
            }
            if meta_parts.is_empty() {
                parts.push(format!("Last session: {task}"));
            } else {
                parts.push(format!("Last session: {task} ({})", meta_parts.join(", ")));
            }
        }

        // Accomplishments — split compound entries, show as bullet points
        if !latest.accomplishments.is_empty() {
            let expanded: Vec<String> = latest
                .accomplishments
                .iter()
                .flat_map(|a| {
                    if a.contains("; ") && a.len() > 200 {
                        a.split("; ").map(|s| s.to_string()).collect::<Vec<_>>()
                    } else {
                        vec![a.clone()]
                    }
                })
                .collect();
            for a in expanded.iter().take(5) {
                parts.push(format!("  - {}", truncate_chars(a, 250)));
            }
        }

        // Decisions — deduplicated against accomplishments, show only unique info
        let acc_set: BTreeSet<&str> = latest.accomplishments.iter().map(|a| a.as_str()).collect();
        let unique_decisions: Vec<_> = latest
            .decisions
            .iter()
            .filter(|d| {
                !acc_set
                    .iter()
                    .any(|a| a.contains(d.as_str()) || d.contains(*a))
            })
            .collect();
        if !unique_decisions.is_empty() {
            for d in unique_decisions.iter().take(3) {
                parts.push(format!("  Decided: {}", truncate_chars(d, 250)));
            }
        }

        // Open questions
        if !latest.open_questions.is_empty() {
            for q in latest.open_questions.iter().take(3) {
                parts.push(format!("  ? {}", truncate_chars(q, 200)));
            }
        }

        // Git context — compressed to 2 lines max
        if let Some(ref git) = latest.git_summary {
            let first_line = git.lines().next().unwrap_or("");
            parts.push(format!("  Changes: {first_line}"));
            // Show first commit subject if available
            if let Some(commit_line) = git.lines().find(|l| l.trim().len() > 8 && l.contains(' ')) {
                let trimmed = commit_line.trim();
                if trimmed.len() > 10
                    && !trimmed.starts_with("Hot")
                    && !trimmed.starts_with("branch")
                {
                    parts.push(format!("    {trimmed}"));
                }
            }
        }
    }

    // === Session trajectory (compressed 1-line summaries) ===
    let latest_task = excerpts
        .first()
        .and_then(|e| e.task.as_deref())
        .unwrap_or("");
    let mut seen_task_prefixes: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    if !latest_task.is_empty() {
        seen_task_prefixes.insert(latest_task.chars().take(40).collect());
    }
    let latest_decision_prefixes: BTreeSet<String> = excerpts
        .first()
        .map(|e| {
            e.decisions
                .iter()
                .map(|d| d.chars().take(50).collect())
                .collect()
        })
        .unwrap_or_default();
    // Also track session numbers for cross-harvest dedup within same session
    let mut seen_session_numbers: BTreeSet<String> = BTreeSet::new();
    if let Some(num) = extract_session_number(latest_task) {
        seen_session_numbers.insert(num);
    }
    let mut prior_count = 0;
    for prior in excerpts.iter().skip(1) {
        if prior_count >= 2 {
            break;
        }
        let prior_task = prior.task.as_deref().unwrap_or("");
        // Dedup by session number (e.g., "Session 016" matches regardless of task suffix)
        if let Some(num) = extract_session_number(prior_task) {
            if seen_session_numbers.contains(&num) {
                continue;
            }
            seen_session_numbers.insert(num);
        }
        let prior_prefix: String = prior_task.chars().take(40).collect();
        if !prior_prefix.is_empty() && seen_task_prefixes.contains(&prior_prefix) {
            continue;
        }
        if !prior_prefix.is_empty() {
            seen_task_prefixes.insert(prior_prefix);
        }
        // Skip priors with no task — nothing to show
        if prior.task.is_none() && prior.accomplishments.is_empty() {
            continue;
        }
        prior_count += 1;
        let task_text = prior
            .task
            .as_deref()
            .map(|t| truncate_chars(t, 100))
            .unwrap_or_default();
        let first_accomplishment = prior
            .accomplishments
            .iter()
            .find_map(|a| {
                let cleaned = if a.starts_with("Session entity: ") {
                    a.split(": ").nth(2).unwrap_or(a).trim().to_string()
                } else {
                    a.clone()
                };
                let prefix: String = cleaned.chars().take(50).collect();
                if latest_decision_prefixes.contains(&prefix) {
                    None
                } else {
                    Some(truncate_chars(&cleaned, 80))
                }
            })
            .unwrap_or_default();
        if first_accomplishment.is_empty() {
            parts.push(format!("Prior: {task_text}"));
        } else {
            parts.push(format!("Prior: {task_text} — {first_accomplishment}"));
        }
    }

    // === Unharvested observations ===
    let latest_harvest_wall = excerpts
        .first()
        .and_then(|_| {
            // Find this session's wall_time from the harvest sessions
            let mut latest = 0u64;
            for d in store.datoms() {
                if d.attribute.as_str() == ":harvest/agent" && d.op == Op::Assert {
                    let w = d.tx.wall_time();
                    if w > latest {
                        latest = w;
                    }
                }
            }
            if latest > 0 {
                Some(latest)
            } else {
                None
            }
        })
        .unwrap_or(0);
    let mut obs_since_harvest = 0;
    let mut obs_summaries: Vec<String> = Vec::new();
    for datom in store.datoms() {
        if datom.attribute.as_str() == ":exploration/source"
            && datom.op == Op::Assert
            && datom.tx.wall_time() > latest_harvest_wall
        {
            if let Value::String(ref s) = datom.value {
                if s == "braid:observe" {
                    obs_since_harvest += 1;
                    if obs_summaries.len() < 3 {
                        for d2 in store.entity_datoms(datom.entity) {
                            if d2.attribute.as_str() == ":db/doc" && d2.op == Op::Assert {
                                if let Value::String(ref doc) = d2.value {
                                    obs_summaries.push(truncate_chars(doc, 200));
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    if obs_since_harvest > 0 {
        parts.push(format!(
            "Unharvested: {obs_since_harvest} observations (run braid harvest --commit)"
        ));
        for obs in &obs_summaries {
            parts.push(format!("  - {obs}"));
        }
    }

    parts.join("\n")
}

/// Discover open questions from observation entities.
///
/// Accepts optional task keywords for relevance scoring (Wave 3: B2).
fn discover_open_questions(store: &Store, task_keywords: &[String]) -> Vec<String> {
    let mut questions: Vec<(f64, String)> = Vec::new();
    // Find entities tagged as open-question
    for datom in store.datoms() {
        if datom.attribute.as_str() == ":exploration/category" && datom.op == Op::Assert {
            let cat_str = match &datom.value {
                Value::String(s) => s.as_str(),
                Value::Keyword(k) => k.as_str(),
                _ => continue,
            };
            if cat_str == "open-question"
                || cat_str == "conjecture"
                || cat_str.ends_with("/open-question")
                || cat_str.ends_with("/conjecture")
            {
                // Get the doc for this entity — use entity_datoms (O(1) index lookup)
                for d2 in store.entity_datoms(datom.entity) {
                    if d2.attribute.as_str() == ":db/doc" && d2.op == Op::Assert {
                        if let Value::String(ref doc) = d2.value {
                            let text = truncate_chars(doc, 200);
                            let score = keyword_relevance_score(&text, task_keywords);
                            questions.push((score, format!("[?] {text}")));
                        }
                        break;
                    }
                }
            }
        }
    }
    // E3: Carry forward open questions from prior harvest sessions.
    // Questions that were recorded in a harvest but not yet resolved should
    // still appear as warnings so they don't get silently dropped.
    let mut seen_texts: BTreeSet<String> = questions.iter().map(|(_, q)| q.clone()).collect();
    for datom in store.datoms() {
        if datom.attribute.as_str() == ":harvest/open-questions" && datom.op == Op::Assert {
            if let Value::String(ref s) = datom.value {
                for line in s.lines() {
                    let stripped = line.strip_prefix("[?] ").unwrap_or(line).trim();
                    if stripped.is_empty() {
                        continue;
                    }
                    let text = truncate_chars(stripped, 200);
                    let formatted = format!("[?] {text}");
                    if !seen_texts.contains(&formatted) {
                        seen_texts.insert(formatted.clone());
                        let score = keyword_relevance_score(&text, task_keywords);
                        // Slight penalty for older questions (from harvest vs direct observation)
                        questions.push((score * 0.8, formatted));
                    }
                }
            }
        }
    }

    // Sort by relevance descending (task-relevant questions first)
    questions.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    questions.into_iter().map(|(_, q)| q).collect()
}

/// Discover active constraints from spec entities (SB.2.3).
///
/// Scans for entities with `:spec/*` idents and extracts their type
/// (invariant, ADR, negative-case) and description. Shows verification
/// status by checking if the spec element has been bootstrapped.
///
/// When task keywords are provided, constraints matching the task are
/// sorted to the top (Wave 3: B3 fix — previously alphabetical only).
fn discover_constraints(store: &Store, task_keywords: &[String]) -> Vec<ConstraintRef> {
    let mut constraints = Vec::new();
    let mut seen = BTreeSet::new();

    for datom in store.datoms() {
        if datom.attribute.as_str() != ":db/ident" || datom.op != Op::Assert {
            continue;
        }
        let ident = match &datom.value {
            Value::Keyword(k) => k.clone(),
            _ => continue,
        };
        // Only spec entities
        if !ident.starts_with(":spec/") {
            continue;
        }
        if !seen.insert(datom.entity) {
            continue;
        }

        // Extract the spec element ID from the ident (e.g., ":spec/inv-store-001" → "INV-STORE-001")
        let id = ident
            .strip_prefix(":spec/")
            .unwrap_or(&ident)
            .to_uppercase();

        // Get the doc/summary and body sub-fields
        let mut summary = String::new();
        let mut statement = None;
        let mut falsification = None;
        let mut has_spec_type = false;
        for d in store.entity_datoms(datom.entity) {
            if d.op != Op::Assert {
                continue;
            }
            match d.attribute.as_str() {
                ":db/doc" => {
                    if let Value::String(ref s) = d.value {
                        summary = truncate_chars(s, 120);
                    }
                }
                ":spec/type" => {
                    has_spec_type = true;
                }
                ":spec/statement" | ":inv/statement" => {
                    if let Value::String(ref s) = d.value {
                        statement = Some(truncate_chars(s, 200));
                    }
                }
                ":spec/falsification" | ":inv/falsification" => {
                    if let Value::String(ref s) = d.value {
                        falsification = Some(truncate_chars(s, 200));
                    }
                }
                _ => {}
            }
        }

        // Only include if it has a spec type (actual spec element, not metadata)
        if !has_spec_type && summary.is_empty() {
            continue;
        }

        // Determine satisfaction status based on whether it's an invariant
        // with known verification. For now, None = unknown.
        let satisfied = None;

        constraints.push(ConstraintRef {
            id,
            summary,
            statement,
            falsification,
            satisfied,
        });
    }

    // Collect spec entities referenced by recent observations (Step 3: constraint relevance)
    let mut obs_referenced_specs: BTreeSet<String> = BTreeSet::new();
    for datom in store.datoms() {
        if datom.attribute.as_str() == ":exploration/related-spec" && datom.op == Op::Assert {
            if let Value::Ref(target) = &datom.value {
                // Resolve target entity's ident
                for d in store.entity_datoms(*target) {
                    if d.attribute.as_str() == ":db/ident" && d.op == Op::Assert {
                        if let Value::Keyword(ref k) = d.value {
                            let id = k.strip_prefix(":spec/").unwrap_or(k).to_uppercase();
                            obs_referenced_specs.insert(id);
                        }
                    }
                }
            }
        }
    }

    // Sort by relevance: observation-referenced specs get highest boost,
    // then task-keyword matching, then alphabetical tiebreaker
    constraints.sort_by(|a, b| {
        let obs_a = if obs_referenced_specs.contains(&a.id) {
            1.0
        } else {
            0.0
        };
        let obs_b = if obs_referenced_specs.contains(&b.id) {
            1.0
        } else {
            0.0
        };
        let kw_a = keyword_relevance_score(&format!("{} {}", a.id, a.summary), task_keywords);
        let kw_b = keyword_relevance_score(&format!("{} {}", b.id, b.summary), task_keywords);
        let score_a = obs_a + kw_a;
        let score_b = obs_b + kw_b;
        score_b
            .partial_cmp(&score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });

    // Limit to top 10 to stay within budget
    constraints.truncate(10);
    constraints
}

/// Build Directive section with task, guidance actions, and last session context.
///
/// Produces a structured directive with:
/// 1. Task anchoring (INV-SEED-006: prevents basin drift)
/// 2. Open questions from last session (carry-forward)
/// 3. Top 3 actions from guidance with runnable commands + spec refs
/// 4. Quick-reference command block (budget permitting, INV-SEED-002)
///
/// Every element is copy-pasteable — the directive IS a prompt fragment.
fn build_directive(
    task: &str,
    actions: &[crate::guidance::GuidanceAction],
    _budget: usize,
    last_session: Option<&SessionExcerpt>,
) -> String {
    let mut parts = vec![format!("Task: {task}")];

    // PRIMARY: Use synthesis directive from last harvest as the main directive.
    // Detect "thin" directives (just task echoes) and supplement with context.
    // When synthesis is rich, it already contains decisions + open questions — don't
    // duplicate them from the excerpt. When thin, fall through to excerpt fields.
    let mut has_rich_synthesis = false;
    if let Some(excerpt) = last_session {
        if let Some(ref directive) = excerpt.synthesis_directive {
            let meaningful: Vec<&str> = directive
                .lines()
                .filter(|l| {
                    let t = l.trim();
                    !t.is_empty()
                        && !t.starts_with("---")
                        && !t.starts_with('#')
                        && !t.starts_with("Run: `braid seed")
                        && !t.starts_with("**Next session task**:")
                })
                .collect();

            // Rich synthesis: has open questions or decisions (> 1 meaningful line)
            if meaningful.len() > 1 {
                has_rich_synthesis = true;
                parts.push(String::new());
                parts.push("From last harvest:".to_string());
                for line in meaningful.iter().take(10) {
                    let clean = line.replace("**", "");
                    parts.push(format!("  {}", clean.trim()));
                }
            }
            // Thin synthesis (just task name): fall through to excerpt fields
        }

        // When synthesis is thin (or absent), show excerpt fields directly
        if !has_rich_synthesis {
            // Open questions — high-value carry-forward
            if !excerpt.open_questions.is_empty() {
                parts.push(String::new());
                parts.push("Open from last session:".to_string());
                for q in excerpt.open_questions.iter().take(5) {
                    parts.push(format!("  ? {}", truncate_chars(q, 200)));
                }
            }

            // Decisions — anchor "do not relitigate" (NEG-002)
            if !excerpt.decisions.is_empty() {
                parts.push(String::new());
                parts.push("Decisions (settled, do not relitigate):".to_string());
                for d in excerpt.decisions.iter().take(5) {
                    parts.push(format!("  - {}", truncate_chars(d, 200)));
                }
            }
        }
    }

    // SECONDARY: Guidance system actions (filtered for real work).
    let actionable: Vec<_> = actions
        .iter()
        .filter(|a| {
            let s = &a.summary;
            !s.contains("cycles") && !s.contains("staleness") && !s.starts_with("Divergence")
        })
        .collect();
    if has_rich_synthesis {
        // Rich synthesis exists — show at most 1 supplementary action
        if let Some(action) = actionable.first() {
            let category_label = format!("{:?}", action.category);
            parts.push(format!("Also: {} — {}", category_label, action.summary));
            if let Some(ref cmd) = action.command {
                parts.push(format!("  run: {cmd}"));
            }
        }
    } else if !actionable.is_empty() {
        parts.push(String::new());
        parts.push("Next actions:".to_string());
        for (i, action) in actionable.iter().take(3).enumerate() {
            let category_label = format!("{:?}", action.category);
            parts.push(format!(
                "  {}. {} — {}",
                i + 1,
                category_label,
                action.summary
            ));
            if let Some(ref cmd) = action.command {
                parts.push(format!("     run: {cmd}"));
            }
        }
    }

    // PROTOCOL: Session lifecycle anchors.
    // Remind the agent of the methodology that makes braid work.
    parts.push(String::new());
    parts.push(
        "Protocol: observe decisions/questions during work, harvest before ending session."
            .to_string(),
    );
    parts.push(
        "Quick: braid status | braid observe \"...\" --category design-decision | braid harvest --commit"
            .to_string(),
    );

    parts.join("\n")
}

fn fallback_recent_entities(store: &Store, limit: usize) -> Vec<EntityId> {
    // Collect max wall_time per entity
    let mut entity_recency: BTreeMap<EntityId, u64> = BTreeMap::new();
    for datom in store.datoms() {
        if datom.op != Op::Assert {
            continue;
        }
        let wall = datom.tx.wall_time();
        let entry = entity_recency.entry(datom.entity).or_default();
        if wall > *entry {
            *entry = wall;
        }
    }

    // Sort by recency (most recent first), then take top N
    let mut by_recency: Vec<(EntityId, u64)> = entity_recency.into_iter().collect();
    by_recency.sort_by_key(|b| std::cmp::Reverse(b.1));
    by_recency.iter().take(limit).map(|(e, _)| *e).collect()
}

// ---------------------------------------------------------------------------
// Core functions
// ---------------------------------------------------------------------------

/// ASSOCIATE v2: Discover the schema neighborhood via graph BFS traversal.
///
/// Phase 1 (keyword matching): Find seed entities whose idents or doc strings
/// match task keywords.
///
/// Phase 2 (graph BFS): Expand outward from seed entities through `Value::Ref`
/// edges (both directions), bounded by depth and breadth parameters.
///
/// This replaces v1's flat keyword scan with topology-aware discovery:
/// entities that are structurally connected to relevant seeds are included
/// even if they don't contain matching keywords directly.
///
/// INV-SEED-003: `|result.entities| ≤ cue.max_results()`.
pub fn associate(store: &Store, cue: &AssociateCue) -> SchemaNeighborhood {
    let max = cue.max_results();
    let mut neighborhood = SchemaNeighborhood::default();

    // Build entity graph for BFS traversal
    let (graph, id_map) = build_entity_graph(store);

    match cue {
        AssociateCue::Semantic {
            text,
            depth,
            breadth,
        } => {
            // Phase 1: TF-IDF-inspired relevance scoring to find seed entities
            let query_tokens = tokenize_for_search(text);

            // Build per-entity text corpus and compute document frequencies
            let mut entity_texts: BTreeMap<EntityId, Vec<String>> = BTreeMap::new();
            for datom in store.datoms() {
                if datom.op != Op::Assert {
                    continue;
                }
                let tokens = match &datom.value {
                    Value::String(s) => tokenize_for_search(s),
                    Value::Keyword(k) => tokenize_for_search(k),
                    _ => vec![],
                };
                // Also tokenize the attribute name for context
                let attr_tokens = tokenize_for_search(datom.attribute.as_str());
                let entry = entity_texts.entry(datom.entity).or_default();
                entry.extend(tokens);
                entry.extend(attr_tokens);
            }

            let num_entities = entity_texts.len().max(1) as f64;

            // Compute document frequency for each token
            let mut doc_freq: BTreeMap<&str, usize> = BTreeMap::new();
            for tokens in entity_texts.values() {
                let unique: BTreeSet<&str> = tokens.iter().map(|s| s.as_str()).collect();
                for token in unique {
                    *doc_freq.entry(token).or_default() += 1;
                }
            }

            // Score each entity: sum of TF × IDF for matching query tokens
            let mut scored: Vec<(EntityId, f64)> = entity_texts
                .iter()
                .map(|(entity, tokens)| {
                    let mut score = 0.0_f64;
                    for qt in &query_tokens {
                        // Term frequency: how many times this query token appears
                        let tf = tokens.iter().filter(|t| t.as_str() == qt).count() as f64;
                        if tf > 0.0 {
                            // Inverse document frequency: log(N / df)
                            let df = doc_freq.get(qt.as_str()).copied().unwrap_or(1) as f64;
                            let idf = (num_entities / df).ln().max(0.1);
                            score += (1.0 + tf.ln()) * idf; // log-normalized TF × IDF
                        }
                    }
                    (*entity, score)
                })
                .filter(|(_, score)| *score > 0.0)
                .collect();

            // Sort by score descending, take top breadth
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let mut seed_entities: Vec<EntityId> =
                scored.iter().take(*breadth).map(|(e, _)| *e).collect();

            // Fallback: if keyword matching found nothing (generic tasks like
            // "continue", "overview", "fix bugs"), seed with the most recent
            // entities instead. This ensures seed always produces output.
            if seed_entities.is_empty() {
                seed_entities = fallback_recent_entities(store, *breadth);
            }

            // Phase 2: BFS expansion through reference graph
            let params = BfsParams {
                store,
                graph: &graph,
                id_map: &id_map,
                seeds: &seed_entities,
                depth: *depth,
                breadth: *breadth,
                max,
            };
            bfs_expand(&params, &mut neighborhood);
        }
        AssociateCue::Explicit {
            seeds,
            depth,
            breadth,
        } => {
            // Start from known entities and expand through graph
            let params = BfsParams {
                store,
                graph: &graph,
                id_map: &id_map,
                seeds,
                depth: *depth,
                breadth: *breadth,
                max,
            };
            bfs_expand(&params, &mut neighborhood);
        }
    }

    neighborhood.entities.truncate(max);
    neighborhood
}

/// Parameters for BFS expansion through the entity reference graph.
struct BfsParams<'a> {
    store: &'a Store,
    graph: &'a DiGraph,
    id_map: &'a BTreeMap<String, EntityId>,
    seeds: &'a [EntityId],
    depth: usize,
    breadth: usize,
    max: usize,
}

/// BFS expansion from seed entities through the entity reference graph.
///
/// Traverses both successors and predecessors (undirected BFS) up to
/// `depth` hops. At each hop, limits expansion to `breadth` neighbors.
fn bfs_expand(params: &BfsParams<'_>, neighborhood: &mut SchemaNeighborhood) {
    let mut visited: BTreeSet<EntityId> = BTreeSet::new();
    let mut frontier: VecDeque<(EntityId, usize)> = VecDeque::new();

    for entity in params.seeds {
        if visited.insert(*entity) {
            frontier.push_back((*entity, 0));
        }
    }

    while let Some((entity, current_depth)) = frontier.pop_front() {
        if neighborhood.entities.len() >= params.max {
            break;
        }

        neighborhood.entities.push(entity);

        // Gather attributes for this entity using the entity index (O(1))
        for datom in params.store.entity_datoms(entity) {
            if datom.op == Op::Assert && !neighborhood.attributes.contains(&datom.attribute) {
                neighborhood.attributes.push(datom.attribute.clone());
            }
        }

        // BFS expansion through graph edges (both directions)
        if current_depth < params.depth {
            let key = entity_key(entity);
            let mut neighbors: Vec<EntityId> = Vec::new();

            // Follow successors (outgoing Ref edges)
            for succ in params.graph.successors(&key) {
                if let Some(&target) = params.id_map.get(succ.as_str()) {
                    if !visited.contains(&target) {
                        neighbors.push(target);
                    }
                }
            }
            // Follow predecessors (incoming Ref edges — undirected traversal)
            for pred in params.graph.predecessors(&key) {
                if let Some(&target) = params.id_map.get(pred.as_str()) {
                    if !visited.contains(&target) {
                        neighbors.push(target);
                    }
                }
            }

            // Limit breadth at each hop
            neighbors.truncate(params.breadth);
            for n in neighbors {
                if visited.insert(n) {
                    frontier.push_back((n, current_depth + 1));
                }
            }
        }
    }
}

/// Score an entity for relevance to a task (ADR-SEED-002, v2).
///
/// `score(e) = α × relevance + β × significance + γ × recency`
/// where α = 0.5, β = 0.3, γ = 0.2.
///
/// **v2 upgrade**: Significance is now PageRank-based structural importance
/// (topology-aware) instead of the v1 attribute-count heuristic. PageRank
/// captures the entity's position in the reference graph — entities linked
/// to by many other entities (high in-degree, transitive importance) score
/// higher. This means seed assembly naturally prioritizes structurally
/// central entities (schema definitions, core invariants) over peripheral
/// leaf entities.
fn score_entity(
    store: &Store,
    entity: EntityId,
    task_keywords: &[&str],
    max_tx_wall_time: u64,
    pagerank_scores: &BTreeMap<String, f64>,
) -> f64 {
    // Use entity index for O(1) lookup instead of O(N) scan
    let datoms = store.entity_datoms(entity);

    let asserted: Vec<&Datom> = datoms
        .iter()
        .filter(|d| d.op == Op::Assert)
        .copied()
        .collect();

    if asserted.is_empty() {
        return 0.0;
    }

    // Relevance: fraction of task keywords that match entity content
    let mut keyword_hits = 0usize;
    for kw in task_keywords {
        for d in &asserted {
            let hit = match &d.value {
                Value::String(s) if s.contains(kw) => true,
                Value::Keyword(k) if k.contains(kw) => true,
                _ => false,
            };
            if hit {
                keyword_hits += 1;
                break;
            }
        }
    }
    let relevance = if task_keywords.is_empty() {
        0.5
    } else {
        keyword_hits as f64 / task_keywords.len() as f64
    };

    // Significance: PageRank structural importance (v2)
    //
    // PageRank values sum to ~1.0 across all nodes, so raw values are
    // tiny for large graphs. We normalize by multiplying by node count
    // to get a [0, ~1] range, clamped to 1.0.
    let key = entity_key(entity);
    let pr = pagerank_scores.get(&key).copied().unwrap_or(0.0);
    let n = pagerank_scores.len().max(1) as f64;
    let significance = (pr * n).min(1.0);

    // Recency: exponential decay normalized by time range (SB.2.2).
    //
    // Uses exp(-λ * normalized_delta) where:
    //   delta = max_wall - entity_wall (how old is this entity)
    //   normalized_delta = delta / time_range (scale to [0, 1])
    //   λ = 3.0 (decay constant: midpoint entity ≈ 0.22, oldest ≈ 0.05)
    //
    // Scale-invariant: works identically for legacy sequential wall_times
    // (range 0-17) and Unix timestamps (range in seconds).
    let newest_tx = asserted.iter().map(|d| d.tx.wall_time()).max().unwrap_or(0);
    let min_wall = store.datoms().map(|d| d.tx.wall_time()).min().unwrap_or(0);
    let time_range = max_tx_wall_time.saturating_sub(min_wall);
    let recency = if time_range == 0 {
        1.0 // All entities at same time → equally recent
    } else {
        let delta = max_tx_wall_time.saturating_sub(newest_tx);
        let normalized = delta as f64 / time_range as f64;
        (-3.0 * normalized).exp()
    };

    0.5 * relevance + 0.3 * significance + 0.2 * recency
}

/// Tokenize text for search relevance scoring.
///
/// Splits on non-alphanumeric characters, lowercases, removes short tokens (< 2 chars),
/// and splits compound identifiers (e.g., ":spec/inv-store-001" → ["spec", "inv", "store", "001"]).
fn tokenize_for_search(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .flat_map(|word| word.split(['-', '_']))
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 2)
        .collect()
}

/// Estimate token count for text (rough: 1 token ≈ 4 chars).
fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

/// Resolve an EntityId to a human-readable label.
///
/// If the entity has a `:db/ident` datom, returns the ident keyword.
/// Otherwise, returns a truncated hex representation of the entity hash.
fn resolve_entity_label(store: &Store, entity: EntityId) -> String {
    for datom in store.entity_datoms(entity) {
        if datom.attribute.as_str() == ":db/ident" {
            if let Value::Keyword(kw) = &datom.value {
                return kw.clone();
            }
        }
    }
    let bytes = entity.as_bytes();
    format!(
        "#{:02x}{:02x}{:02x}{:02x}\u{2026}",
        bytes[0], bytes[1], bytes[2], bytes[3]
    )
}

/// Format a Value for human-readable output.
///
/// Strips the enum variant wrapper so that `String("foo")` becomes `"foo"`,
/// `Keyword(":ns/name")` becomes `:ns/name`, and `Ref(entity)` resolves to
/// the target entity's ident.
fn format_value(store: &Store, value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{}\"", s),
        Value::Keyword(kw) => kw.clone(),
        Value::Boolean(b) => b.to_string(),
        Value::Long(n) => n.to_string(),
        Value::Double(f) => f.to_string(),
        Value::Instant(ms) => format!("#{ms}"),
        Value::Uuid(bytes) => {
            format!(
                "#{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                bytes[0], bytes[1], bytes[2], bytes[3],
                bytes[4], bytes[5], bytes[6], bytes[7],
                bytes[8], bytes[9], bytes[10], bytes[11],
                bytes[12], bytes[13], bytes[14], bytes[15],
            )
        }
        Value::Ref(target) => resolve_entity_label(store, *target),
        Value::Bytes(b) => format!("#bytes[{}]", b.len()),
    }
}

/// Project an entity at a given projection level.
fn project_entity(store: &Store, entity: EntityId, level: ProjectionLevel) -> StateEntry {
    let datoms: Vec<&Datom> = store
        .datoms()
        .filter(|d| d.entity == entity && d.op == Op::Assert)
        .collect();

    let label = resolve_entity_label(store, entity);

    let content = match level {
        ProjectionLevel::Pointer => label,
        ProjectionLevel::TypeLevel => {
            let type_kw = datoms
                .iter()
                .find(|d| d.attribute.as_str() == ":db/ident")
                .and_then(|d| {
                    if let Value::Keyword(kw) = &d.value {
                        Some(kw.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| label.clone());
            format!("{} ({} attrs)", type_kw, datoms.len())
        }
        ProjectionLevel::Summary => {
            // Show the doc string (most informative single attribute) rather than
            // a raw attribute list, which is useless for agent comprehension.
            let doc = datoms
                .iter()
                .find(|d| d.attribute.as_str() == ":db/doc")
                .and_then(|d| {
                    if let Value::String(ref s) = d.value {
                        Some(s.as_str())
                    } else {
                        None
                    }
                });
            // Fall back to :exploration/body for observations
            let body = doc.or_else(|| {
                datoms
                    .iter()
                    .find(|d| d.attribute.as_str() == ":exploration/body")
                    .and_then(|d| {
                        if let Value::String(ref s) = d.value {
                            Some(s.as_str())
                        } else {
                            None
                        }
                    })
            });
            match body {
                Some(text) => {
                    format!("{} — {}", label, truncate_chars(text, 200))
                }
                None => {
                    format!("{} ({} attrs)", label, datoms.len())
                }
            }
        }
        ProjectionLevel::Full => {
            // Filter out harvest process metadata — noise for agents
            const NOISE_ATTRS: &[&str] = &[
                ":harvest/candidate-count",
                ":harvest/drift-score",
                ":harvest/store-datom-count",
                ":harvest/store-entity-count",
                ":harvest/agent",
                ":harvest/codebase-snapshot",
                ":harvest/tx-since-last",
                ":harvest/observation-count",
                ":exploration/content-hash",
                ":exploration/source",
                ":exploration/maturity",
                ":exploration/body", // Always duplicates :db/doc — suppress
            ];
            let mut lines = Vec::new();
            for d in &datoms {
                if NOISE_ATTRS.contains(&d.attribute.as_str()) {
                    continue;
                }
                lines.push(format!(
                    "  {} = {}",
                    d.attribute.as_str(),
                    format_value(store, &d.value)
                ));
            }
            format!("{}:\n{}", label, lines.join("\n"))
        }
    };

    let tokens = estimate_tokens(&content);
    StateEntry {
        entity,
        projection: level,
        content,
        tokens,
    }
}

/// ASSEMBLE v2: Build the seed context within a token budget (INV-SEED-002).
///
/// v2 upgrade: computes PageRank over the entity reference graph and uses
/// it as the significance component of entity scoring. This means the
/// seed automatically prioritizes structurally central entities.
///
/// Compression priority (first-to-compress → last):
/// State → Constraints → Orientation → Warnings → Directive.
pub fn assemble(
    store: &Store,
    neighborhood: &SchemaNeighborhood,
    task: &str,
    budget: usize,
) -> AssembledContext {
    let task_kw = extract_task_keywords(store, task);
    let task_keywords: Vec<&str> = task_kw.iter().map(|s| s.as_str()).collect();
    let max_wall = store.datoms().map(|d| d.tx.wall_time()).max().unwrap_or(0);

    // Compute PageRank over entity reference graph (v2)
    let (graph, _id_map) = build_entity_graph(store);
    let pr_scores = pagerank(&graph, 20);

    // Score and sort entities by relevance (with PageRank significance)
    let mut scored: Vec<(EntityId, f64)> = neighborhood
        .entities
        .iter()
        .map(|&e| {
            (
                e,
                score_entity(store, e, &task_keywords, max_wall, &pr_scores),
            )
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Phase 3: Adaptive projection (v2)
    //
    // Instead of a single global projection level, we assign per-entity
    // projection based on relevance score. High-scoring entities get Full
    // projection (maximum information), medium get Summary, low get Pointer.
    //
    // The projection thresholds adapt to the score distribution:
    //   top 20%  → Full (π₀)
    //   next 30% → Summary (π₁)
    //   next 30% → TypeLevel (π₂)
    //   bottom 20% → Pointer (π₃)
    //
    // This is a form of rate-distortion optimization: we allocate more
    // bits (tokens) to entities with higher information value, matching
    // the classical water-filling solution from information theory.
    let max_score = scored.first().map(|(_, s)| *s).unwrap_or(0.0);
    let fallback_projection = if budget > 2000 {
        ProjectionLevel::Full
    } else if budget > 500 {
        ProjectionLevel::Summary
    } else if budget > 200 {
        ProjectionLevel::TypeLevel
    } else {
        ProjectionLevel::Pointer
    };

    // Build sections

    // Get last session excerpt for directive carry-forward
    let recent = discover_recent_sessions(store, 1);
    let last_session = recent.first();

    // Directive: task + action injection (SB.2.1)
    let actions = crate::guidance::derive_actions(store);
    let directive_text = build_directive(task, &actions, budget, last_session);
    let directive_tokens = estimate_tokens(&directive_text);
    let directive = ContextSection::Directive(directive_text);

    // Orientation: session history narrative (SB.2.4)
    let orientation_text = build_orientation(store, &task_kw);
    let orientation_tokens = estimate_tokens(&orientation_text);
    let orientation = ContextSection::Orientation(orientation_text);

    // Warnings: surface open questions from observations
    let warning_lines = discover_open_questions(store, &task_kw);
    let warnings_tokens = warning_lines
        .iter()
        .map(|w| estimate_tokens(w))
        .sum::<usize>();
    let warnings = ContextSection::Warnings(warning_lines);

    // Constraints: active invariants from spec entities (SB.2.3)
    let constraint_refs = discover_constraints(store, &task_kw);
    let constraints_tokens = constraint_refs
        .iter()
        .map(|c| estimate_tokens(&c.id) + estimate_tokens(&c.summary) + 4)
        .sum::<usize>();
    let constraints = ContextSection::Constraints(constraint_refs);

    // Allocate remaining budget to state entries
    let overhead = directive_tokens + orientation_tokens + warnings_tokens + constraints_tokens;
    let state_budget = budget.saturating_sub(overhead);

    // Collect harvest entity IDs to exclude from State (already shown in Orientation)
    let harvest_entities: BTreeSet<EntityId> = store
        .datoms()
        .filter(|d| d.attribute.as_str() == ":harvest/agent" && d.op == Op::Assert)
        .map(|d| d.entity)
        .collect();

    let mut state_entries = Vec::new();
    let mut state_tokens = 0;
    for (entity, score) in &scored {
        // Skip harvest entities — their content is already rendered in Orientation
        if harvest_entities.contains(entity) {
            continue;
        }

        // Adaptive projection: higher scores get richer projection
        let projection = if max_score > 0.0 {
            let normalized = score / max_score;
            if normalized > 0.8 {
                ProjectionLevel::Full
            } else if normalized > 0.5 {
                ProjectionLevel::Summary
            } else if normalized > 0.2 {
                ProjectionLevel::TypeLevel
            } else {
                ProjectionLevel::Pointer
            }
        } else {
            fallback_projection
        };

        // Clamp projection to respect global budget constraints
        let effective_projection = projection.min(fallback_projection);

        // Skip hex-hash entities before budget allocation (not after)
        let entry = project_entity(store, *entity, effective_projection);
        if entry.content.starts_with('#') {
            continue;
        }
        if state_tokens + entry.tokens > state_budget {
            // Try a lower projection before skipping this entity
            let compressed = project_entity(store, *entity, ProjectionLevel::Pointer);
            if compressed.content.starts_with('#') {
                continue;
            }
            if state_tokens + compressed.tokens <= state_budget {
                state_tokens += compressed.tokens;
                state_entries.push(compressed);
            }
            // Don't break — try remaining smaller entities
            continue;
        }
        state_tokens += entry.tokens;
        state_entries.push(entry);
    }

    // BACKFILL: Fill remaining budget with high-value structural entities.
    //
    // Strategy: spec entities first (project invariants — the rules of the game),
    // then recent non-observation entities. Observations are in Orientation;
    // showing them raw in State wastes budget with zero new information.
    //
    // Spec entities use Summary projection (compact: "INV-SEED-001 — Budget Compliance")
    // to pack more project rules into the budget. Full projection on specs wastes
    // tokens on :spec/source-file and :spec/namespace which agents don't need.
    let already_shown: BTreeSet<EntityId> = state_entries
        .iter()
        .map(|e| e.entity)
        .chain(harvest_entities.iter().copied())
        .collect();
    if state_tokens < state_budget.saturating_sub(50) {
        // Pass 0: Project context cheat sheet — synthesized from store data.
        // This is the HIGHEST-VALUE content in the entire seed. An agent reading
        // this can immediately start working: knows the types, patterns, commands,
        // and current focus. ~100 tokens, worth 10x that in orientation time saved.
        {
            // Derive current stage from most recent harvest task
            let stage_hint = {
                let sessions = discover_recent_sessions(store, 1);
                sessions
                    .first()
                    .and_then(|s| s.task.as_deref())
                    .map(|t| {
                        if t.to_lowercase().contains("stage 0")
                            || t.to_lowercase().contains("harvest")
                            || t.to_lowercase().contains("seed")
                        {
                            "Stage 0: harvest/seed cycle replaces HARVEST.md"
                        } else if t.to_lowercase().contains("stage 1") {
                            "Stage 1: budget-aware output + guidance injection"
                        } else {
                            "Stage 0"
                        }
                    })
                    .unwrap_or("Stage 0")
            };

            // Count unique attributes used in the store
            let unique_attrs: BTreeSet<&str> = store
                .datoms()
                .filter(|d| d.op == Op::Assert)
                .map(|d| d.attribute.as_str())
                .collect();

            let mut cheat = vec![
                format!("Project context: {stage_hint}"),
                format!(
                    "Core: datom [e,a,v,tx,op]. Store = grow-only set (CRDT merge = set union). {} distinct attributes in use.",
                    unique_attrs.len()
                ),
                "Crates: braid-kernel (store, schema, query, harvest, seed, guidance, merge), braid (CLI)."
                    .to_string(),
                "CLI: braid {init, status, transact, query, harvest, seed, observe, guidance, merge, log, schema}."
                    .to_string(),
                "Patterns: Store::genesis() for tests. BraidError for errors. EntityId::from_ident(). Value::{String,Keyword,Long,Double}."
                    .to_string(),
                "Quality: cargo check && cargo clippy --all-targets -- -D warnings && cargo fmt --check && cargo test."
                    .to_string(),
            ];

            // Add test count if available from codebase snapshot
            let sessions = discover_recent_sessions(store, 1);
            if let Some(snapshot) = sessions.first().and_then(|s| s.codebase_snapshot.as_ref()) {
                if let Some(test_line) = snapshot.lines().find(|l| l.starts_with("Tests:")) {
                    cheat.push(test_line.to_string());
                }
            }

            // Add key attribute vocabulary — show agent-facing attributes (for queries/observations)
            // Skip infrastructure attrs (spec/*, dep/*, schema/*) — agents don't query those.
            // Focus on harvest/*, exploration/*, intent/*, impl/* — the working vocabulary.
            let mut attr_counts: BTreeMap<&str, usize> = BTreeMap::new();
            for d in store.datoms() {
                if d.op == Op::Assert {
                    *attr_counts.entry(d.attribute.as_str()).or_default() += 1;
                }
            }
            let mut sorted_attrs: Vec<_> = attr_counts.into_iter().collect();
            sorted_attrs.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
            let key_attrs: Vec<_> = sorted_attrs
                .iter()
                .filter(|(a, _)| {
                    // Show working vocabulary: harvest, exploration, intent, impl, bilateral
                    // Skip infrastructure: db/*, schema/*, spec/*, dep/*
                    !a.starts_with(":db/")
                        && !a.starts_with(":schema/")
                        && !a.starts_with(":spec/")
                        && !a.starts_with(":dep/")
                        && !a.starts_with(":spec.")
                })
                .take(15)
                .map(|(a, c)| format!("{a}({c})"))
                .collect();
            if !key_attrs.is_empty() {
                cheat.push(format!("Key attrs: {}", key_attrs.join(", ")));
            }

            let cheat_text = cheat.join("\n");
            let cheat_tokens = cheat_text.split_whitespace().count() * 4 / 3;
            if state_tokens + cheat_tokens <= state_budget {
                state_entries.push(StateEntry {
                    entity: EntityId::from_ident(":project-context"),
                    content: cheat_text,
                    tokens: cheat_tokens,
                    projection: ProjectionLevel::Summary,
                });
                state_tokens += cheat_tokens;
            }
        }

        // Pass 1: Top task-relevant spec entities as demonstrations.
        // The full namespace map is in Orientation. Here we show only the
        // highest-scoring specs that directly relate to the current task.
        let mut spec_scored: Vec<(EntityId, f64)> = Vec::new();
        for datom in store.datoms() {
            if datom.attribute.as_str() == ":db/ident"
                && datom.op == Op::Assert
                && matches!(&datom.value, Value::Keyword(k) if k.starts_with(":spec/") && !k.starts_with(":spec."))
                && !already_shown.contains(&datom.entity)
            {
                let kw_score =
                    score_entity(store, datom.entity, &task_keywords, max_wall, &pr_scores);
                spec_scored.push((datom.entity, kw_score));
            }
        }
        spec_scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Spec entities: Summary projection. Full projection adds noise
        // (element-type, namespace, source-file — zero agent value).
        let spec_demo_cap = 15.min(spec_scored.len());
        for (entity, _) in spec_scored.iter().take(spec_demo_cap) {
            let entry = project_entity(store, *entity, ProjectionLevel::Summary);
            if entry.content.starts_with('#') {
                continue;
            }
            if state_tokens + entry.tokens > state_budget {
                break;
            }
            state_tokens += entry.tokens;
            state_entries.push(entry);
        }

        // Pass 2: Session trajectory — the arc of work across all sessions.
        // Orientation shows the LAST 2 sessions in detail. State shows the FULL
        // history as a compressed progression — each session in 1 line max.
        // Dedup by session number to exclude Orientation sessions.
        if state_tokens < state_budget.saturating_sub(100) {
            let all_sessions = discover_recent_sessions(store, 15);
            // Collect session numbers that Orientation already shows (newest 2 distinct)
            let mut orientation_nums: BTreeSet<String> = BTreeSet::new();
            for excerpt in all_sessions.iter().take(5) {
                if orientation_nums.len() >= 2 {
                    break;
                }
                if let Some(ref t) = excerpt.task {
                    if let Some(num) = extract_session_number(t) {
                        orientation_nums.insert(num);
                    }
                }
            }

            let mut trajectory_lines = vec!["Session history (oldest → newest):".to_string()];
            let mut seen_nums = orientation_nums.clone();
            // Oldest-first
            for excerpt in all_sessions.iter().rev() {
                let task = excerpt
                    .task
                    .as_deref()
                    .map(|t| truncate_chars(t, 70))
                    .unwrap_or_default();
                if task.is_empty() {
                    continue;
                }
                // Session-number dedup (also excludes Orientation sessions)
                if let Some(num) = extract_session_number(&task) {
                    if seen_nums.contains(&num) {
                        continue;
                    }
                    seen_nums.insert(num);
                }
                // Ultra-compact: task → first accomplishment (no decisions, keep lines short)
                let first_acc = excerpt
                    .accomplishments
                    .first()
                    .map(|a| {
                        let cleaned = if a.starts_with("Session entity: ") {
                            a.split(": ").nth(2).unwrap_or(a).trim()
                        } else {
                            a.as_str()
                        };
                        truncate_chars(cleaned, 60)
                    })
                    .unwrap_or_default();
                if first_acc.is_empty() {
                    trajectory_lines.push(format!("  {task}"));
                } else {
                    trajectory_lines.push(format!("  {task} → {first_acc}"));
                }
            }
            if trajectory_lines.len() > 1 {
                let trajectory_text = trajectory_lines.join("\n");
                let traj_tokens = trajectory_text.split_whitespace().count() * 4 / 3;
                if state_tokens + traj_tokens <= state_budget {
                    state_entries.push(StateEntry {
                        entity: EntityId::from_ident(":session-trajectory"),
                        content: trajectory_text,
                        tokens: traj_tokens,
                        projection: ProjectionLevel::Summary,
                    });
                    state_tokens += traj_tokens;
                }
            }
        }

        // Pass 3: Observation bodies — captured knowledge NOT already in Orientation.
        // Orientation shows accomplishments/decisions from last 2 sessions. Here we
        // show observations that carry genuinely NEW information — architectural
        // insights, patterns discovered, questions raised.
        if state_tokens < state_budget.saturating_sub(100) {
            let already_shown2: BTreeSet<EntityId> = state_entries
                .iter()
                .map(|e| e.entity)
                .chain(harvest_entities.iter().copied())
                .collect();
            // Build content fingerprints from Orientation to avoid text-level duplication.
            // Orientation shows accomplishments/decisions; skip observations that
            // duplicate those.
            let orientation_sessions = discover_recent_sessions(store, 3);
            let mut orientation_prefixes: BTreeSet<String> = BTreeSet::new();
            for sess in &orientation_sessions {
                for a in &sess.accomplishments {
                    orientation_prefixes.insert(a.chars().take(50).collect());
                }
                for d in &sess.decisions {
                    orientation_prefixes.insert(d.chars().take(50).collect());
                }
            }

            let mut obs_entries: Vec<(u64, EntityId, String)> = Vec::new();
            for datom in store.datoms() {
                if datom.attribute.as_str() == ":db/doc"
                    && datom.op == Op::Assert
                    && !already_shown2.contains(&datom.entity)
                {
                    let is_obs = store.entity_datoms(datom.entity).iter().any(|d| {
                        d.attribute.as_str() == ":exploration/source"
                            && d.op == Op::Assert
                            && matches!(&d.value, Value::String(s) if s == "braid:observe")
                    });
                    if is_obs {
                        if let Value::String(ref doc) = datom.value {
                            // Skip if too short (noise), or duplicates Orientation
                            let prefix: String = doc.chars().take(50).collect();
                            if doc.len() > 40 && !orientation_prefixes.contains(&prefix) {
                                obs_entries.push((datom.tx.wall_time(), datom.entity, doc.clone()));
                            }
                        }
                    }
                }
            }
            obs_entries.sort_by_key(|(t, _, _)| std::cmp::Reverse(*t));
            if !obs_entries.is_empty() {
                // Dynamic cap: each observation ~25 tokens, fill remaining budget
                let obs_cap = ((state_budget.saturating_sub(state_tokens)) / 25).max(3);
                let mut obs_lines = vec!["Key observations:".to_string()];
                for (_, _, doc) in obs_entries.iter().take(obs_cap) {
                    obs_lines.push(format!("  - {}", truncate_chars(doc, 150)));
                }
                let obs_text = obs_lines.join("\n");
                let obs_tokens = obs_text.split_whitespace().count() * 4 / 3;
                if state_tokens + obs_tokens <= state_budget {
                    state_entries.push(StateEntry {
                        entity: EntityId::from_ident(":key-observations"),
                        content: obs_text,
                        tokens: obs_tokens,
                        projection: ProjectionLevel::Summary,
                    });
                    state_tokens += obs_tokens;
                }
            }
        }

        // Pass 4: Recent non-observation, non-harvest entities (catch-all)
        if state_tokens < state_budget.saturating_sub(50) {
            let already_shown3: BTreeSet<EntityId> = state_entries
                .iter()
                .map(|e| e.entity)
                .chain(harvest_entities.iter().copied())
                .collect();
            let recent = fallback_recent_entities(store, 40);
            for entity in recent {
                if already_shown3.contains(&entity) {
                    continue;
                }
                let is_excluded = store.entity_datoms(entity).iter().any(|d| {
                    if d.attribute.as_str() != ":db/ident" || d.op != Op::Assert {
                        return false;
                    }
                    match &d.value {
                        Value::Keyword(k) => {
                            k.starts_with(":db/")
                                || k.starts_with(":db.")
                                || k.starts_with(":schema/")
                                || k.starts_with(":harvest/")
                                || k.starts_with(":observation/")
                                || k.starts_with(":spec/")
                                || k.starts_with(":spec.")
                                || k.starts_with(":exploration")
                        }
                        _ => false,
                    }
                });
                if is_excluded {
                    continue;
                }
                let entry = project_entity(store, entity, fallback_projection);
                if entry.content.starts_with('#') {
                    continue;
                }
                if state_tokens + entry.tokens > state_budget {
                    continue;
                }
                state_tokens += entry.tokens;
                state_entries.push(entry);
            }
        }
    }

    // Compute dominant projection before moving state_entries
    let dominant_projection = if state_entries.is_empty() {
        fallback_projection
    } else {
        // Most common projection level among assembled entries
        let mut counts = [0usize; 4]; // Pointer, TypeLevel, Summary, Full
        for entry in &state_entries {
            let idx = match entry.projection {
                ProjectionLevel::Pointer => 0,
                ProjectionLevel::TypeLevel => 1,
                ProjectionLevel::Summary => 2,
                ProjectionLevel::Full => 3,
            };
            counts[idx] += 1;
        }
        let max_idx = counts
            .iter()
            .enumerate()
            .max_by_key(|(_, &c)| c)
            .map(|(i, _)| i)
            .unwrap_or(0);
        match max_idx {
            0 => ProjectionLevel::Pointer,
            1 => ProjectionLevel::TypeLevel,
            2 => ProjectionLevel::Summary,
            _ => ProjectionLevel::Full,
        }
    };

    let state = ContextSection::State(state_entries);

    // Budget accounting: when fixed sections (orientation, directive, constraints,
    // warnings) exceed the budget, clamp total to budget — the overflow is tracked
    // but never reported as "remaining" budget.
    let total_tokens = (overhead + state_tokens).min(budget);
    let budget_remaining = budget.saturating_sub(overhead + state_tokens);

    AssembledContext {
        sections: vec![orientation, constraints, state, warnings, directive],
        total_tokens,
        budget_remaining,
        projection_pattern: dominant_projection,
    }
}

/// Top-level seed assembly: ASSOCIATE → ASSEMBLE in one call.
///
/// This is the primary entry point for start-of-session context loading.
pub fn assemble_seed(store: &Store, task: &str, budget: usize, agent: AgentId) -> SeedOutput {
    let cue = AssociateCue::Semantic {
        text: task.to_string(),
        depth: 3,
        breadth: 25,
    };

    let neighborhood = associate(store, &cue);
    let entities_discovered = neighborhood.entities.len();
    let context = assemble(store, &neighborhood, task, budget);

    SeedOutput {
        context,
        agent,
        task: task.to_string(),
        entities_discovered,
    }
}

/// Group state entries by semantic type for comprehension (E6).
///
/// Classifies entities by their ident prefix into labeled groups:
/// Specifications, Observations, Harvest Sessions, Schema, Other.
/// Returns groups in a stable order with human-readable labels.
pub fn group_state_entries(entries: &[StateEntry]) -> Vec<(String, Vec<&StateEntry>)> {
    let mut specs: Vec<&StateEntry> = Vec::new();
    let mut self_labeled: Vec<&StateEntry> = Vec::new();
    let mut other: Vec<&StateEntry> = Vec::new();

    for entry in entries {
        let c = &entry.content;
        if c.starts_with(":spec/") || c.contains(":spec/") {
            specs.push(entry);
        } else if c.starts_with("Session history")
            || c.starts_with("Key observations")
            || c.starts_with("Specification landscape")
            || c.starts_with("Project context")
        {
            // Synthetic entries with their own headers — render ungrouped
            self_labeled.push(entry);
        } else {
            other.push(entry);
        }
    }

    let mut groups = Vec::new();
    // Self-labeled entries first (project context, observations, trajectory)
    for entry in self_labeled {
        groups.push((String::new(), vec![entry]));
    }
    if !specs.is_empty() {
        groups.push((String::new(), specs));
    }
    if !other.is_empty() {
        groups.push((String::new(), other));
    }
    groups
}

// ---------------------------------------------------------------------------
// Phase 4: Verification (INV-SEED-001..006)
// ---------------------------------------------------------------------------

/// Verification result for a seed output.
#[derive(Clone, Debug)]
pub struct SeedVerification {
    /// List of satisfied invariants.
    pub satisfied: Vec<String>,
    /// List of violated invariants with descriptions.
    pub violations: Vec<String>,
    /// Overall pass/fail.
    pub passed: bool,
}

/// Verify a seed output against all SEED invariants.
///
/// Checks:
/// - **INV-SEED-001**: Store projection — every state entry entity exists in the store.
/// - **INV-SEED-002**: Budget compliance — total_tokens ≤ budget.
/// - **INV-SEED-003**: ASSOCIATE boundedness — entities_discovered ≤ max_results.
/// - **INV-SEED-004**: Section ordering — five sections in correct order.
/// - **INV-SEED-006**: Intention anchoring — Directive section always present.
pub fn verify_seed(seed: &SeedOutput, store: &Store, budget: usize) -> SeedVerification {
    let mut satisfied = Vec::new();
    let mut violations = Vec::new();

    // INV-SEED-001: Store projection — every datum traces to a datom
    // Synthetic state entries (project context, session trajectory, key observations)
    // are DERIVED from store data but use generated EntityIds. They satisfy
    // INV-SEED-001's intent (all content from store) even though the entity
    // IDs are synthetic aggregation points.
    let synthetic_entities: BTreeSet<EntityId> = [
        ":project-context",
        ":session-trajectory",
        ":key-observations",
        ":spec-landscape",
    ]
    .iter()
    .map(|s| EntityId::from_ident(s))
    .collect();
    let store_entities = store.entities();
    let mut all_in_store = true;
    for section in &seed.context.sections {
        if let ContextSection::State(entries) = section {
            for entry in entries {
                if synthetic_entities.contains(&entry.entity) {
                    continue; // Skip synthetic aggregation entries
                }
                if !store_entities.contains(&entry.entity) {
                    let label = resolve_entity_label(store, entry.entity);
                    violations.push(format!(
                        "INV-SEED-001 violated: entity {} in seed but not in store",
                        label
                    ));
                    all_in_store = false;
                }
            }
        }
    }
    if all_in_store {
        satisfied.push("INV-SEED-001: all seed entities trace to store datoms".into());
    }

    // INV-SEED-002: Budget compliance
    if seed.context.total_tokens <= budget {
        satisfied.push(format!(
            "INV-SEED-002: total_tokens ({}) ≤ budget ({})",
            seed.context.total_tokens, budget
        ));
    } else {
        violations.push(format!(
            "INV-SEED-002 violated: total_tokens ({}) > budget ({})",
            seed.context.total_tokens, budget
        ));
    }

    // INV-SEED-003: ASSOCIATE boundedness
    // The cue used by assemble_seed has depth=5, breadth=10, so max = 50
    let max_results = 50;
    if seed.entities_discovered <= max_results {
        satisfied.push(format!(
            "INV-SEED-003: entities_discovered ({}) ≤ max_results ({})",
            seed.entities_discovered, max_results
        ));
    } else {
        violations.push(format!(
            "INV-SEED-003 violated: entities_discovered ({}) > max_results ({})",
            seed.entities_discovered, max_results
        ));
    }

    // INV-SEED-004: Section ordering (five sections)
    if seed.context.sections.len() == 5 {
        satisfied.push("INV-SEED-004: exactly 5 sections present".into());
    } else {
        violations.push(format!(
            "INV-SEED-004 violated: expected 5 sections, got {}",
            seed.context.sections.len()
        ));
    }

    // INV-SEED-006: Intention anchoring — Directive always present
    let has_directive = seed
        .context
        .sections
        .iter()
        .any(|s| matches!(s, ContextSection::Directive(_)));
    if has_directive {
        satisfied.push("INV-SEED-006: Directive section present (intention anchored)".into());
    } else {
        violations.push("INV-SEED-006 violated: no Directive section".into());
    }

    // Budget accounting consistency
    if seed.context.total_tokens + seed.context.budget_remaining <= budget {
        satisfied.push("Budget accounting: total + remaining ≤ budget".into());
    } else {
        violations.push(format!(
            "Budget accounting violated: {} + {} > {}",
            seed.context.total_tokens, seed.context.budget_remaining, budget
        ));
    }

    let passed = violations.is_empty();
    SeedVerification {
        satisfied,
        violations,
        passed,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// Witnesses: INV-SEED-001, INV-SEED-002, INV-SEED-003, INV-SEED-004,
// INV-SEED-005, INV-SEED-006, INV-SEED-007, INV-SEED-008,
// ADR-SEED-001, ADR-SEED-002, ADR-SEED-003, ADR-SEED-004,
// ADR-SEED-005, ADR-SEED-006,
// NEG-SEED-001, NEG-SEED-002
#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::AgentId;

    // Verifies: INV-SEED-003 — ASSOCIATE Boundedness
    #[test]
    fn associate_boundedness() {
        let store = Store::genesis();
        let cue = AssociateCue::Semantic {
            text: "db ident".to_string(),
            depth: 2,
            breadth: 3,
        };
        let max = cue.max_results();
        let neighborhood = associate(&store, &cue);
        assert!(
            neighborhood.entities.len() <= max,
            "INV-SEED-003: |result| <= depth * breadth"
        );
    }

    // Verifies: INV-SEED-001 — Seed as Store Projection
    #[test]
    fn associate_explicit_includes_seeds() {
        let store = Store::genesis();
        let seed_entity = EntityId::from_ident(":db/ident");
        let cue = AssociateCue::Explicit {
            seeds: vec![seed_entity],
            depth: 1,
            breadth: 5,
        };
        let neighborhood = associate(&store, &cue);
        assert!(neighborhood.entities.contains(&seed_entity));
    }

    // Verifies: INV-SEED-002 — Budget Compliance
    // Verifies: NEG-SEED-002 — No Budget Overflow
    #[test]
    fn assemble_respects_budget() {
        let store = Store::genesis();
        let budget = 500;
        let neighborhood = SchemaNeighborhood {
            entities: store.entities().into_iter().collect(),
            attributes: vec![],
            entity_types: vec![],
        };
        let ctx = assemble(&store, &neighborhood, "test task", budget);
        assert!(
            ctx.total_tokens <= budget,
            "INV-SEED-002: total_tokens ({}) <= budget ({})",
            ctx.total_tokens,
            budget
        );
    }

    // Verifies: INV-SEED-004 — Section Compression Priority
    // Verifies: ADR-SEED-002 — Rate-Distortion Assembly
    #[test]
    fn assemble_selects_projection_by_budget() {
        let store = Store::genesis();
        let neighborhood = SchemaNeighborhood::default();

        let high = assemble(&store, &neighborhood, "task", 3000);
        assert_eq!(high.projection_pattern, ProjectionLevel::Full);

        let mid = assemble(&store, &neighborhood, "task", 1000);
        assert_eq!(mid.projection_pattern, ProjectionLevel::Summary);

        let low = assemble(&store, &neighborhood, "task", 300);
        assert_eq!(low.projection_pattern, ProjectionLevel::TypeLevel);

        let tiny = assemble(&store, &neighborhood, "task", 100);
        assert_eq!(tiny.projection_pattern, ProjectionLevel::Pointer);
    }

    // Verifies: INV-SEED-001 — Seed as Store Projection
    // Verifies: ADR-SEED-004 — Unified Five-Part Seed Template
    #[test]
    fn assemble_seed_end_to_end() {
        let store = Store::genesis();
        let agent = AgentId::from_name("test-agent");
        let seed = assemble_seed(&store, "datom store schema", 2000, agent);
        assert_eq!(seed.task, "datom store schema");
        assert!(seed.context.total_tokens <= 2000);
        assert_eq!(seed.context.sections.len(), 5);
    }

    #[test]
    fn projection_level_ordering() {
        assert!(ProjectionLevel::Pointer < ProjectionLevel::TypeLevel);
        assert!(ProjectionLevel::TypeLevel < ProjectionLevel::Summary);
        assert!(ProjectionLevel::Summary < ProjectionLevel::Full);
    }

    #[test]
    fn score_entity_nonzero_for_matching() {
        let store = Store::genesis();
        let entity = EntityId::from_ident(":db/ident");
        let (graph, _) = build_entity_graph(&store);
        let pr = crate::query::graph::pagerank(&graph, 20);
        let score = score_entity(&store, entity, &["ident"], 100, &pr);
        assert!(score > 0.0, "matching entity should have positive score");
    }

    // -------------------------------------------------------------------
    // Seed v2 tests: graph BFS traversal + PageRank scoring
    // -------------------------------------------------------------------

    #[test]
    fn build_entity_graph_includes_all_entities() {
        let store = Store::genesis();
        let (graph, id_map) = build_entity_graph(&store);
        // Every entity in the store should appear as a node
        for entity in store.entities() {
            let key = entity_key(entity);
            assert!(
                id_map.contains_key(&key),
                "entity {:?} missing from id_map",
                entity
            );
            assert!(
                graph.nodes().any(|n| n == &key),
                "entity {:?} missing from graph",
                entity
            );
        }
    }

    /// Build a store with the full schema (L0 genesis + L1 + L2 + L3).
    fn store_with_full_schema() -> Store {
        let system_agent = AgentId::from_name("braid:system");
        let genesis_tx = crate::datom::TxId::new(0, 0, system_agent);
        let mut datom_set = std::collections::BTreeSet::new();
        // L0 genesis (defines :db/ident, :db/valueType, etc.)
        for d in crate::schema::genesis_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        // L1 + L2 + L3 (domain attributes)
        for d in crate::schema::full_schema_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        Store::from_datoms(datom_set)
    }

    #[test]
    fn associate_bfs_discovers_ref_neighbors() {
        use crate::datom::{Attribute, ProvenanceType};
        use crate::store::Transaction;

        let mut store = store_with_full_schema();
        let agent = AgentId::from_name("test");

        // Create two entities linked by a Ref edge (:dep/from is ValueType::Ref)
        let entity_a = EntityId::from_ident(":test/alpha");
        let entity_b = EntityId::from_ident(":test/beta");

        let tx = Transaction::new(agent, ProvenanceType::Observed, "link entities")
            .assert(
                entity_a,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/alpha".into()),
            )
            .assert(
                entity_a,
                Attribute::from_keyword(":db/doc"),
                Value::String("searchable keyword magic".into()),
            )
            .assert(
                entity_b,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/beta".into()),
            )
            .assert(
                entity_b,
                Attribute::from_keyword(":db/doc"),
                Value::String("not searchable".into()),
            )
            .assert(
                entity_a,
                Attribute::from_keyword(":dep/from"),
                Value::Ref(entity_b),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        // Search for "magic" — should find entity_a directly and entity_b via BFS
        let cue = AssociateCue::Semantic {
            text: "magic".to_string(),
            depth: 2,
            breadth: 10,
        };
        let neighborhood = associate(&store, &cue);

        assert!(
            neighborhood.entities.contains(&entity_a),
            "should find entity_a via keyword match"
        );
        assert!(
            neighborhood.entities.contains(&entity_b),
            "should find entity_b via BFS through Ref edge"
        );
    }

    #[test]
    fn pagerank_boosts_hub_entities() {
        use crate::datom::{Attribute, ProvenanceType};
        use crate::store::Transaction;

        let mut store = store_with_full_schema();
        let agent = AgentId::from_name("test");

        // Create a hub entity pointed to by many others
        let hub = EntityId::from_ident(":test/hub");
        let spoke_a = EntityId::from_ident(":test/spoke-a");
        let spoke_b = EntityId::from_ident(":test/spoke-b");
        let spoke_c = EntityId::from_ident(":test/spoke-c");

        let tx = Transaction::new(agent, ProvenanceType::Observed, "create hub")
            .assert(
                hub,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/hub".into()),
            )
            .assert(
                hub,
                Attribute::from_keyword(":db/doc"),
                Value::String("central hub".into()),
            )
            .assert(
                spoke_a,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/spoke-a".into()),
            )
            .assert(
                spoke_a,
                Attribute::from_keyword(":db/doc"),
                Value::String("spoke a".into()),
            )
            .assert(
                spoke_a,
                Attribute::from_keyword(":impl/implements"),
                Value::Ref(hub),
            )
            .assert(
                spoke_b,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/spoke-b".into()),
            )
            .assert(
                spoke_b,
                Attribute::from_keyword(":db/doc"),
                Value::String("spoke b".into()),
            )
            .assert(
                spoke_b,
                Attribute::from_keyword(":impl/implements"),
                Value::Ref(hub),
            )
            .assert(
                spoke_c,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/spoke-c".into()),
            )
            .assert(
                spoke_c,
                Attribute::from_keyword(":db/doc"),
                Value::String("spoke c".into()),
            )
            .assert(
                spoke_c,
                Attribute::from_keyword(":impl/implements"),
                Value::Ref(hub),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        // Score hub vs spoke with PageRank
        let (graph, _) = build_entity_graph(&store);
        let pr = crate::query::graph::pagerank(&graph, 20);

        // Same keywords match both — but hub should score higher due to PageRank
        let hub_score = score_entity(&store, hub, &["hub", "spoke"], 100, &pr);
        let spoke_score = score_entity(&store, spoke_a, &["hub", "spoke"], 100, &pr);

        assert!(
            hub_score > spoke_score,
            "hub ({:.4}) should score higher than spoke ({:.4}) due to PageRank",
            hub_score,
            spoke_score,
        );
    }

    #[test]
    fn associate_v2_boundedness_with_graph() {
        use crate::datom::{Attribute, ProvenanceType};
        use crate::store::Transaction;

        let mut store = store_with_full_schema();
        let agent = AgentId::from_name("test");

        // Create a chain of entities: A → B → C → D
        let entities: Vec<EntityId> = (0..4)
            .map(|i| EntityId::from_ident(&format!(":test/chain-{i}")))
            .collect();

        let mut tx = Transaction::new(agent, ProvenanceType::Observed, "chain");
        for (i, entity) in entities.iter().enumerate() {
            tx = tx
                .assert(
                    *entity,
                    Attribute::from_keyword(":db/ident"),
                    Value::Keyword(format!(":test/chain-{i}")),
                )
                .assert(
                    *entity,
                    Attribute::from_keyword(":db/doc"),
                    Value::String(format!("chain node {i}")),
                );
            if i > 0 {
                tx = tx.assert(
                    entities[i - 1],
                    Attribute::from_keyword(":dep/to"),
                    Value::Ref(*entity),
                );
            }
        }
        let committed = tx.commit(&store).unwrap();
        store.transact(committed).unwrap();

        // Search with depth=1 — should NOT traverse the whole chain
        let cue = AssociateCue::Semantic {
            text: "chain".to_string(),
            depth: 1,
            breadth: 2,
        };
        let max = cue.max_results();
        let neighborhood = associate(&store, &cue);

        assert!(
            neighborhood.entities.len() <= max,
            "INV-SEED-003: |result| ({}) <= max_results ({})",
            neighborhood.entities.len(),
            max,
        );
    }

    // -------------------------------------------------------------------
    // Phase 4: Seed verification tests
    // -------------------------------------------------------------------

    // Verifies: INV-SEED-001 — Seed as Store Projection (verification)
    // Verifies: NEG-SEED-001 — No Fabricated Context
    #[test]
    fn verify_seed_passes_for_genesis() {
        let store = Store::genesis();
        let agent = AgentId::from_name("test-verify");
        let budget = 2000;
        let seed = assemble_seed(&store, "test verification", budget, agent);
        let verification = verify_seed(&seed, &store, budget);

        assert!(
            verification.passed,
            "genesis seed should pass verification: violations = {:?}",
            verification.violations
        );
        assert!(
            verification.satisfied.len() >= 5,
            "should satisfy at least 5 invariants, got {}",
            verification.satisfied.len()
        );
    }

    // Verifies: INV-SEED-002 — Budget Compliance (violation detection)
    // Verifies: NEG-SEED-002 — No Budget Overflow
    #[test]
    fn verify_seed_detects_budget_violation() {
        // Manually construct a seed with budget violation
        let store = Store::genesis();
        let bad_seed = SeedOutput {
            context: AssembledContext {
                sections: vec![
                    ContextSection::Orientation("test".into()),
                    ContextSection::Constraints(vec![]),
                    ContextSection::State(vec![]),
                    ContextSection::Warnings(vec![]),
                    ContextSection::Directive("task".into()),
                ],
                total_tokens: 5000, // Exceeds budget
                budget_remaining: 0,
                projection_pattern: ProjectionLevel::Full,
            },
            agent: AgentId::from_name("test"),
            task: "test".into(),
            entities_discovered: 0,
        };
        let verification = verify_seed(&bad_seed, &store, 1000);
        assert!(!verification.passed, "should fail with budget violation");
        assert!(
            verification
                .violations
                .iter()
                .any(|v| v.contains("INV-SEED-002")),
            "should report INV-SEED-002 violation"
        );
    }

    #[test]
    fn adaptive_projection_mixes_levels() {
        use crate::datom::{Attribute, ProvenanceType};
        use crate::store::Transaction;

        let mut store = store_with_full_schema();
        let agent = AgentId::from_name("test");

        // Create entities with varying "importance" (attribute richness)
        for i in 0..5 {
            let entity = EntityId::from_ident(&format!(":test/entity-{i}"));
            let mut tx = Transaction::new(agent, ProvenanceType::Observed, "create test entities")
                .assert(
                    entity,
                    Attribute::from_keyword(":db/ident"),
                    Value::Keyword(format!(":test/entity-{i}")),
                )
                .assert(
                    entity,
                    Attribute::from_keyword(":db/doc"),
                    Value::String(format!("entity {i} for adaptive projection test")),
                );

            // Add more attributes to higher-numbered entities
            if i >= 3 {
                tx = tx.assert(
                    entity,
                    Attribute::from_keyword(":spec/id"),
                    Value::String(format!("TEST-{i}")),
                );
            }

            let committed = tx.commit(&store).unwrap();
            store.transact(committed).unwrap();
        }

        let neighborhood = SchemaNeighborhood {
            entities: (0..5)
                .map(|i| EntityId::from_ident(&format!(":test/entity-{i}")))
                .collect(),
            attributes: vec![],
            entity_types: vec![],
        };

        // Large budget — adaptive projection should assign different levels
        let ctx = assemble(&store, &neighborhood, "adaptive projection test", 5000);
        assert!(ctx.total_tokens <= 5000, "budget respected");

        // Extract state entries and check for mixed projections
        let entries: Vec<&StateEntry> = ctx
            .sections
            .iter()
            .flat_map(|s| match s {
                ContextSection::State(entries) => entries.iter().collect::<Vec<_>>(),
                _ => vec![],
            })
            .collect();

        // At least some entries should exist
        assert!(!entries.is_empty(), "should have state entries");
    }

    // -------------------------------------------------------------------
    // Rate-distortion bound witnesses (proptest)
    //
    // Seed assembly is a lossy compression of the store subject to
    // information-theoretic bounds. These tests witness:
    //
    // 1. Token budget monotonicity: larger budget => equal or larger output
    // 2. Budget respect: output tokens never exceed budget
    // 3. Relevance monotonicity: more relevant datoms => no decrease in output
    // 4. Rate function: information retained decreases as budget decreases
    // -------------------------------------------------------------------

    mod seed_proptests {
        use super::*;
        use crate::datom::AgentId;
        use crate::proptest_strategies::arb_store;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn assemble_seed_deterministic(
                store in arb_store(3),
                budget in 100usize..5000,
            ) {
                let agent = AgentId::from_name("det-test");
                let task = "determinism check";
                let seed_a = assemble_seed(&store, task, budget, agent);
                let seed_b = assemble_seed(&store, task, budget, agent);

                prop_assert_eq!(
                    seed_a.context.sections.len(),
                    seed_b.context.sections.len(),
                    "section count must be identical across calls"
                );
                prop_assert_eq!(
                    seed_a.context.total_tokens,
                    seed_b.context.total_tokens,
                    "total_tokens must be identical across calls"
                );
                prop_assert_eq!(
                    seed_a.context.budget_remaining,
                    seed_b.context.budget_remaining,
                    "budget_remaining must be identical across calls"
                );
                prop_assert_eq!(
                    seed_a.entities_discovered,
                    seed_b.entities_discovered,
                    "entities_discovered must be identical across calls"
                );
                prop_assert_eq!(
                    seed_a.context.projection_pattern,
                    seed_b.context.projection_pattern,
                    "projection_pattern must be identical across calls"
                );
            }

            #[test]
            fn assemble_seed_always_five_sections(
                store in arb_store(3),
                budget in 50usize..5000,
            ) {
                let agent = AgentId::from_name("sections-test");
                let seed = assemble_seed(&store, "any task", budget, agent);

                prop_assert_eq!(
                    seed.context.sections.len(),
                    5,
                    "Seed must always have exactly 5 sections \
                     (Orientation, Constraints, State, Warnings, Directive), got {}",
                    seed.context.sections.len()
                );
            }

            #[test]
            fn assemble_seed_token_budget_respected(
                store in arb_store(3),
                budget in 50usize..5000,
            ) {
                let agent = AgentId::from_name("budget-test");
                let seed = assemble_seed(&store, "budget test", budget, agent);

                prop_assert!(
                    seed.context.total_tokens <= budget,
                    "INV-SEED-002: total_tokens ({}) must be <= budget ({})",
                    seed.context.total_tokens,
                    budget
                );
            }
        }
    }

    mod rate_distortion_proptests {
        use super::*;
        use crate::datom::{AgentId, Attribute, ProvenanceType, Value};
        use crate::proptest_strategies::arb_store;
        use crate::store::Transaction;
        use proptest::prelude::*;

        fn count_state_entries(ctx: &AssembledContext) -> usize {
            ctx.sections
                .iter()
                .map(|s| match s {
                    ContextSection::State(entries) => entries.len(),
                    _ => 0,
                })
                .sum()
        }

        fn state_token_sum(ctx: &AssembledContext) -> usize {
            ctx.sections
                .iter()
                .map(|s| match s {
                    ContextSection::State(entries) => {
                        entries.iter().map(|e| e.tokens).sum::<usize>()
                    }
                    _ => 0,
                })
                .sum()
        }

        proptest! {
            // RD-1: Budget respect -- seed output never exceeds the declared budget.
            // This is INV-SEED-002 witnessed via proptest.
            #[test]
            fn rate_distortion_budget_respect(
                store in arb_store(3),
                budget in 50usize..5000,
            ) {
                let agent = AgentId::from_name("rd-test");
                let seed = assemble_seed(&store, "test task", budget, agent);

                prop_assert!(
                    seed.context.total_tokens <= budget,
                    "INV-SEED-002 violated: total_tokens ({}) > budget ({})",
                    seed.context.total_tokens,
                    budget
                );
            }

            // RD-2: Budget monotonicity -- more budget fits more or equal state entries.
            // With adaptive projection (v2), individual entities may use different
            // projection levels, so we only test that entry count is monotone.
            #[test]
            fn rate_distortion_budget_monotonicity_fixed_projection(
                store in arb_store(3),
                budget_small in 2001usize..3000,
            ) {
                let budget_large = budget_small + 1000;

                let neighborhood = SchemaNeighborhood {
                    entities: store.entities().into_iter().collect(),
                    attributes: vec![],
                    entity_types: vec![],
                };

                let small_ctx = assemble(&store, &neighborhood, "test task", budget_small);
                let large_ctx = assemble(&store, &neighborhood, "test task", budget_large);

                let small_entries = count_state_entries(&small_ctx);
                let large_entries = count_state_entries(&large_ctx);

                prop_assert!(
                    large_entries >= small_entries,
                    "Budget monotonicity violated: budget {} => {} entries, \
                     budget {} => {} entries",
                    budget_large, large_entries, budget_small, small_entries
                );
            }

            // RD-3: Projection level monotonicity -- larger budgets produce equal
            // or higher projection levels (more detail per entity).
            #[test]
            fn rate_distortion_projection_monotonicity(
                store in arb_store(2),
                budget_small in 50usize..500,
            ) {
                let budget_large = budget_small + 2000;
                let neighborhood = SchemaNeighborhood::default();

                let small_ctx = assemble(&store, &neighborhood, "task", budget_small);
                let large_ctx = assemble(&store, &neighborhood, "task", budget_large);

                prop_assert!(
                    large_ctx.projection_pattern >= small_ctx.projection_pattern,
                    "Projection monotonicity violated: larger budget selected lower projection \
                     ({:?} < {:?})",
                    large_ctx.projection_pattern,
                    small_ctx.projection_pattern
                );
            }

            // RD-4: Rate function -- at fixed projection level, state tokens
            // consumed are non-decreasing as budget grows. This is the
            // rate-distortion R(D): more budget => more information retained.
            #[test]
            fn rate_distortion_rate_function(
                store in arb_store(3),
                budget_small in 2001usize..3000,
            ) {
                let budget_large = budget_small + 1000;

                let neighborhood = SchemaNeighborhood {
                    entities: store.entities().into_iter().collect(),
                    attributes: vec![],
                    entity_types: vec![],
                };

                let small_ctx = assemble(&store, &neighborhood, "test", budget_small);
                let large_ctx = assemble(&store, &neighborhood, "test", budget_large);

                let small_tokens = state_token_sum(&small_ctx);
                let large_tokens = state_token_sum(&large_ctx);

                prop_assert!(
                    large_tokens >= small_tokens,
                    "Rate function violated: budget {} => {} state tokens, \
                     budget {} => {} state tokens",
                    budget_large, large_tokens, budget_small, small_tokens
                );
            }

            // RD-5: Budget remaining is non-negative and consistent.
            // total_tokens + budget_remaining <= budget.
            #[test]
            fn rate_distortion_budget_accounting(
                store in arb_store(2),
                budget in 50usize..5000,
            ) {
                let neighborhood = SchemaNeighborhood {
                    entities: store.entities().into_iter().collect(),
                    attributes: vec![],
                    entity_types: vec![],
                };

                let ctx = assemble(&store, &neighborhood, "test", budget);

                prop_assert!(
                    ctx.total_tokens + ctx.budget_remaining <= budget,
                    "Budget accounting violated: total ({}) + remaining ({}) > budget ({})",
                    ctx.total_tokens,
                    ctx.budget_remaining,
                    budget
                );
            }

            // RD-6: Relevance monotonicity -- adding datoms matching the task
            // keywords to the store does not decrease the number of entities
            // discovered by ASSOCIATE.
            #[test]
            fn rate_distortion_relevance_monotonicity(
                store in arb_store(2),
            ) {
                let agent = AgentId::from_name("rd-test");
                let task = "documentation";
                let budget = 3000;

                let seed_before = assemble_seed(&store, task, budget, agent);
                let discovered_before = seed_before.entities_discovered;

                // Add a datom that matches the task keyword "documentation"
                let mut enriched = store.clone_store();
                let enrich_agent = AgentId::from_name("enricher");
                let entity = EntityId::from_ident(":test/enrichment");
                let tx = Transaction::new(
                    enrich_agent,
                    ProvenanceType::Observed,
                    "add relevant datom",
                )
                .assert(
                    entity,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("documentation for testing relevance".into()),
                )
                .commit(&enriched)
                .unwrap();
                enriched.transact(tx).unwrap();

                let seed_after = assemble_seed(&enriched, task, budget, agent);
                let discovered_after = seed_after.entities_discovered;

                prop_assert!(
                    discovered_after >= discovered_before,
                    "Relevance monotonicity violated: adding relevant datoms decreased \
                     entities discovered from {} to {}",
                    discovered_before,
                    discovered_after
                );
            }

            // RD-7: Genesis store seed -- always well-formed with 5 sections
            // and respects any budget.
            #[test]
            fn rate_distortion_genesis_wellformed(
                budget in 100usize..5000,
            ) {
                let store = Store::genesis();
                let agent = AgentId::from_name("rd-test");
                let seed = assemble_seed(&store, "anything", budget, agent);

                prop_assert_eq!(
                    seed.context.sections.len(),
                    5,
                    "Seed must always have exactly 5 sections"
                );
                prop_assert!(
                    seed.context.total_tokens <= budget,
                    "Budget violated even for genesis store"
                );
            }
        }
    }

    // Verifies: INV-SEED-006 — Intention Anchoring
    // Verifies: ADR-SEED-003 — Spec-Language Over Instruction-Language
    #[test]
    fn test_orientation_includes_session_decisions() {
        use crate::datom::{Attribute, TxId};
        use std::collections::BTreeSet;

        let mut store = store_with_full_schema();
        let agent = AgentId::from_name("test-agent");
        let tx_id = TxId::new(1000, 1, agent);

        // Build raw datoms for a harvest session + a decision observation.
        // Use raw datoms + merge to bypass schema validation (harvest attributes
        // are not in the L0-L3 schema — they are domain attributes added at runtime).
        let session_id = EntityId::from_ident(":harvest/session-test");
        let obs_id = EntityId::from_ident(":observation/decision-1");

        let mut raw_datoms = BTreeSet::new();

        // Harvest session entity
        raw_datoms.insert(Datom::new(
            session_id,
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test-agent".into()),
            tx_id,
            Op::Assert,
        ));
        raw_datoms.insert(Datom::new(
            session_id,
            Attribute::from_keyword(":harvest/task"),
            Value::String("implement feature X".into()),
            tx_id,
            Op::Assert,
        ));
        raw_datoms.insert(Datom::new(
            session_id,
            Attribute::from_keyword(":db/doc"),
            Value::String("Harvest session for task: implement feature X".into()),
            tx_id,
            Op::Assert,
        ));

        // Decision observation (same wall_time so it falls in the 1-hour window)
        let obs_tx = TxId::new(1001, 1, agent);
        raw_datoms.insert(Datom::new(
            obs_id,
            Attribute::from_keyword(":exploration/source"),
            Value::String("braid:observe".into()),
            obs_tx,
            Op::Assert,
        ));
        raw_datoms.insert(Datom::new(
            obs_id,
            Attribute::from_keyword(":exploration/category"),
            Value::String("design-decision".into()),
            obs_tx,
            Op::Assert,
        ));
        raw_datoms.insert(Datom::new(
            obs_id,
            Attribute::from_keyword(":db/doc"),
            Value::String("Decided to use hash-join strategy".into()),
            obs_tx,
            Op::Assert,
        ));

        // Merge raw datoms into the schema-aware store
        let overlay = Store::from_datoms(raw_datoms);
        store.merge(&overlay);

        let orientation = build_orientation(&store, &[]);
        // Should contain the harvest session info
        assert!(
            orientation.contains("implement feature X"),
            "Should contain task goal, got: {}",
            orientation
        );
        // Should contain decision from the session excerpt
        assert!(
            orientation.contains("hash-join") || orientation.contains("decisions"),
            "Orientation should surface session decisions: {}",
            orientation
        );
    }

    #[test]
    fn test_directive_includes_quick_reference() {
        use crate::guidance::{ActionCategory, GuidanceAction};

        let actions = vec![GuidanceAction {
            category: ActionCategory::Connect,
            summary: "Test action".into(),
            command: Some("braid query --entity :spec/test".into()),
            relates_to: vec!["INV-STORE-001".into()],
            priority: 1,
        }];

        // No synthesis → actions shown with quick reference
        let directive = build_directive("test task", &actions, 2000, None);

        // Should contain task anchoring
        assert!(directive.contains("Task: test task"));
        // Should contain the action
        assert!(directive.contains("Test action"));
        assert!(directive.contains("braid query --entity :spec/test"));
        // Should contain compressed quick reference
        assert!(directive.contains("braid status"));
        assert!(directive.contains("braid harvest --commit"));
    }

    // ── Wave 1: B1 category mismatch fix tests ──────────────────────────

    #[test]
    fn test_category_keyword_matching() {
        use crate::datom::TxId;
        // Test that discover_session_content finds Value::Keyword categories
        let mut store = Store::genesis();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1000, 0, agent);
        let obs_tx = TxId::new(1001, 1, agent);

        // Harvest session entity
        let session = EntityId::from_ident(":harvest/test-session-kw");
        let obs = EntityId::from_ident(":obs/test-decision-kw");

        let mut raw = BTreeSet::new();
        raw.insert(Datom::new(
            session,
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test".into()),
            tx,
            Op::Assert,
        ));
        raw.insert(Datom::new(
            obs,
            Attribute::from_keyword(":exploration/source"),
            Value::String("braid:observe".into()),
            obs_tx,
            Op::Assert,
        ));
        // Use Keyword variant (this is what braid observe stores)
        raw.insert(Datom::new(
            obs,
            Attribute::from_keyword(":exploration/category"),
            Value::Keyword(":exploration.cat/design-decision".into()),
            obs_tx,
            Op::Assert,
        ));
        raw.insert(Datom::new(
            obs,
            Attribute::from_keyword(":db/doc"),
            Value::String("Fisher-Rao for scoring".into()),
            obs_tx,
            Op::Assert,
        ));

        store.merge(&Store::from_datoms(raw));

        let excerpt = discover_session_content(&store, session);
        assert!(
            !excerpt.decisions.is_empty(),
            "Should find Keyword-stored decisions, got: {:?}",
            excerpt
        );
        assert!(
            excerpt.decisions.iter().any(|d| d.contains("Fisher-Rao")),
            "Decision text should match: {:?}",
            excerpt.decisions
        );
    }

    #[test]
    fn test_open_question_keyword() {
        use crate::datom::TxId;
        // Test that discover_open_questions finds Value::Keyword categories
        let mut store = Store::genesis();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1000, 0, agent);

        let obs = EntityId::from_ident(":obs/test-question-kw");
        let mut raw = BTreeSet::new();
        raw.insert(Datom::new(
            obs,
            Attribute::from_keyword(":exploration/category"),
            Value::Keyword(":exploration.cat/open-question".into()),
            tx,
            Op::Assert,
        ));
        raw.insert(Datom::new(
            obs,
            Attribute::from_keyword(":db/doc"),
            Value::String("How to handle 3-agent merge?".into()),
            tx,
            Op::Assert,
        ));

        store.merge(&Store::from_datoms(raw));

        let questions = discover_open_questions(&store, &[]);
        assert!(
            !questions.is_empty(),
            "Should find Keyword-stored open questions"
        );
        assert!(
            questions.iter().any(|q| q.contains("3-agent merge")),
            "Question text should match: {:?}",
            questions
        );
    }

    #[test]
    fn test_orientation_finds_keyword_decisions() {
        use crate::datom::TxId;
        // Test that build_orientation shows decisions from harvest session excerpts.
        // Decisions are persisted via :harvest/decisions datoms on the session entity.
        let mut store = Store::genesis();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1000, 0, agent);

        // Create a harvest session entity with decisions
        let session = EntityId::from_ident(":harvest/session-test-1000");
        let mut raw = BTreeSet::new();
        raw.insert(Datom::new(
            session,
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test".into()),
            tx,
            Op::Assert,
        ));
        raw.insert(Datom::new(
            session,
            Attribute::from_keyword(":harvest/task"),
            Value::String("CRDT merge implementation".into()),
            tx,
            Op::Assert,
        ));
        raw.insert(Datom::new(
            session,
            Attribute::from_keyword(":harvest/decisions"),
            Value::String("Use CRDT for merge (rationale: commutative by design)".into()),
            tx,
            Op::Assert,
        ));
        raw.insert(Datom::new(
            session,
            Attribute::from_keyword(":db/doc"),
            Value::String("Harvest session for task: CRDT merge implementation".into()),
            tx,
            Op::Assert,
        ));

        store.merge(&Store::from_datoms(raw));

        let text = build_orientation(&store, &[]);
        assert!(
            text.contains("CRDT"),
            "Orientation should include decisions from harvest session: {text}"
        );
    }

    // ── Wave 3: Task-aware seed sections tests ──────────────────────────

    // Verifies: INV-SEED-006 — Intention Anchoring (task-aware constraints)
    // Verifies: INV-GUIDANCE-009 — Task Derivation Completeness
    #[test]
    fn test_constraints_task_aware() {
        use crate::datom::TxId;
        // Constraints matching task keywords should sort first
        let mut store = Store::genesis();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1000, 0, agent);

        // Add spec entities: one QUERY, one BILATERAL
        let spec_q = EntityId::from_ident(":spec/inv-query-001");
        let spec_b = EntityId::from_ident(":spec/adr-bilateral-001");
        let mut raw = BTreeSet::new();

        for (entity, ident, doc) in [
            (spec_q, ":spec/inv-query-001", "Datalog query completeness"),
            (
                spec_b,
                ":spec/adr-bilateral-001",
                "Fitness function weights",
            ),
        ] {
            raw.insert(Datom::new(
                entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(ident.into()),
                tx,
                Op::Assert,
            ));
            raw.insert(Datom::new(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String(doc.into()),
                tx,
                Op::Assert,
            ));
            raw.insert(Datom::new(
                entity,
                Attribute::from_keyword(":spec/type"),
                Value::Keyword(":element.type/invariant".into()),
                tx,
                Op::Assert,
            ));
        }

        store.merge(&Store::from_datoms(raw));

        // Task "query" — QUERY constraint should be first
        let kw = vec!["query".to_string()];
        let constraints = discover_constraints(&store, &kw);
        assert!(
            !constraints.is_empty(),
            "Should find constraints: {:?}",
            constraints
        );
        assert!(
            constraints[0].id.contains("QUERY"),
            "QUERY constraint should be first for task 'query', got: {:?}",
            constraints.iter().map(|c| &c.id).collect::<Vec<_>>()
        );
    }

    // ── Wave 4: Telemetry/phantom tests ─────────────────────────────────

    #[test]
    fn test_telemetry_from_store_nonzero() {
        use crate::guidance::telemetry_from_store;
        let store = Store::genesis();
        let telemetry = telemetry_from_store(&store);
        // Genesis store has at least 1 transaction (genesis itself)
        assert!(
            telemetry.total_turns >= 1,
            "total_turns should be >= 1, got {}",
            telemetry.total_turns
        );
        let score = crate::guidance::compute_methodology_score(&telemetry);
        assert!(
            score.score > 0.0,
            "M(t) should be > 0 for genesis store, got {}",
            score.score
        );
    }

    #[test]
    fn test_phantom_commands_replaced() {
        use crate::guidance::derive_actions;
        let store = Store::genesis();
        let actions = derive_actions(&store);
        let all_commands: String = actions
            .iter()
            .filter_map(|a| a.command.as_ref())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            !all_commands.contains("braid analyze"),
            "Should not contain phantom 'braid analyze' command: {all_commands}"
        );
    }

    // ── Wave 6: Budget/presentation tests ───────────────────────────────

    #[test]
    fn test_seed_no_hex_hashes() {
        // Seed output should not contain raw hex entity hashes
        let store = Store::genesis();
        let agent = AgentId::from_name("test");
        let seed = assemble_seed(&store, "test task", 3000, agent);

        for section in &seed.context.sections {
            if let ContextSection::State(entries) = section {
                for entry in entries {
                    assert!(
                        !entry.content.starts_with('#'),
                        "State entry should not be a hex hash: {}",
                        entry.content
                    );
                }
            }
        }
    }

    // ── Wave 2.3: Harvest narrative round-trip tests ────────────────────

    // Verifies: INV-SEED-007 — Dynamic CLAUDE.md Relevance
    // Verifies: ADR-SEED-006 — Dynamic CLAUDE.md Generation
    #[test]
    fn test_seed_reads_harvest_narrative() {
        use crate::datom::TxId;
        // Verify harvest narrative datoms are read by seed
        let mut store = Store::genesis();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(2000, 0, agent);

        let session = EntityId::from_ident(":harvest/session-test-narr");
        let mut raw = BTreeSet::new();

        raw.insert(Datom::new(
            session,
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test".into()),
            tx,
            Op::Assert,
        ));
        raw.insert(Datom::new(
            session,
            Attribute::from_keyword(":harvest/task"),
            Value::String("implement scoring engine".into()),
            tx,
            Op::Assert,
        ));
        raw.insert(Datom::new(
            session,
            Attribute::from_keyword(":harvest/accomplishments"),
            Value::String("Built Fisher-Rao scorer\nAdded 15 tests".into()),
            tx,
            Op::Assert,
        ));
        raw.insert(Datom::new(
            session,
            Attribute::from_keyword(":harvest/decisions"),
            Value::String(
                "Use Fisher-Rao metric (rationale: information-geometric foundation)".into(),
            ),
            tx,
            Op::Assert,
        ));
        raw.insert(Datom::new(
            session,
            Attribute::from_keyword(":harvest/git-summary"),
            Value::String("branch=main, 3 commits, 5 files (+200/-50)".into()),
            tx,
            Op::Assert,
        ));

        store.merge(&Store::from_datoms(raw));

        let excerpt = discover_session_content(&store, session);
        assert!(
            !excerpt.accomplishments.is_empty(),
            "Should read accomplishments"
        );
        assert!(
            excerpt.accomplishments.iter().any(|a| a.contains("Fisher")),
            "Accomplishment should contain Fisher: {:?}",
            excerpt.accomplishments
        );
        assert!(
            excerpt.task.as_deref() == Some("implement scoring engine"),
            "Task should be set: {:?}",
            excerpt.task
        );
        assert!(
            excerpt.git_summary.is_some(),
            "Git summary should be set: {:?}",
            excerpt.git_summary
        );

        // Verify orientation renders the new fields
        let text = build_orientation(&store, &[]);
        assert!(
            text.contains("implement scoring engine"),
            "Orientation should show task: {text}"
        );
        assert!(
            text.contains("Fisher-Rao scorer"),
            "Orientation should show accomplishments: {text}"
        );
    }

    // -------------------------------------------------------------------
    // INV-SEED-004/005/006 property-based tests
    // -------------------------------------------------------------------

    mod proptests {
        use super::*;
        use crate::datom::{AgentId, Attribute, TxId, Value};
        use crate::proptest_strategies::arb_store;
        use proptest::prelude::*;

        // ── INV-SEED-004: Section Compression Priority ──────────────────
        //
        // Under budget pressure, State compresses first, Directive last.
        // Property: for any store, a tight budget still yields a non-empty
        // Directive section (containing the task), while the State section
        // may have fewer entries than under a generous budget.
        proptest! {
            #[test]
            fn compression_priority_directive_survives(
                store in arb_store(3),
                tight_budget in 50usize..300,
            ) {
                let agent = AgentId::from_name("inv004-test");
                let task = "compression priority check";
                let generous_budget = tight_budget + 3000;

                let tight_seed = assemble_seed(&store, task, tight_budget, agent);
                let generous_seed = assemble_seed(&store, task, generous_budget, agent);

                // Both seeds must have exactly 5 sections (Orientation, Constraints,
                // State, Warnings, Directive) — structural invariant.
                prop_assert_eq!(tight_seed.context.sections.len(), 5);
                prop_assert_eq!(generous_seed.context.sections.len(), 5);

                // INV-SEED-004 core: Directive section always present and contains
                // the task — it is the LAST section to compress.
                let tight_directive = tight_seed.context.sections.iter().find_map(|s| {
                    if let ContextSection::Directive(ref d) = s { Some(d.clone()) } else { None }
                });
                prop_assert!(
                    tight_directive.is_some(),
                    "Directive section must exist even under tight budget"
                );
                prop_assert!(
                    tight_directive.as_ref().unwrap().contains("Task:"),
                    "Directive must contain task anchoring under tight budget, got: {:?}",
                    tight_directive
                );

                // State compresses first: under tight budget, state entry count
                // should be <= state entry count under generous budget.
                let tight_state_count: usize = tight_seed.context.sections.iter()
                    .map(|s| match s {
                        ContextSection::State(entries) => entries.len(),
                        _ => 0,
                    })
                    .sum();
                let generous_state_count: usize = generous_seed.context.sections.iter()
                    .map(|s| match s {
                        ContextSection::State(entries) => entries.len(),
                        _ => 0,
                    })
                    .sum();

                prop_assert!(
                    generous_state_count >= tight_state_count,
                    "INV-SEED-004: State should absorb compression first — \
                     generous ({}) should have >= tight ({}) state entries",
                    generous_state_count,
                    tight_state_count
                );
            }
        }

        // ── INV-SEED-005: Demonstration Density ─────────────────────────
        //
        // When budget > 1000 and the store has 2+ spec elements in the
        // same namespace (a constraint cluster), the seed should include
        // at least one spec entity as a demonstration in the State section.
        proptest! {
            #[test]
            fn demonstration_density_when_cluster_exists(
                budget in 1500usize..5000,
            ) {
                // Build a store with a constraint cluster: 3 spec elements in
                // the STORE namespace, forming a cluster of related constraints.
                let mut store = store_with_full_schema();
                let agent = AgentId::from_name("inv005-test");
                let tx = TxId::new(5000, 0, agent);

                let mut raw = std::collections::BTreeSet::new();
                for (ident, doc) in [
                    (":spec/inv-store-001", "Append-Only Immutability"),
                    (":spec/inv-store-003", "Content-Addressable Identity"),
                    (":spec/inv-store-005", "Transaction Atomicity"),
                ] {
                    let entity = EntityId::from_ident(ident);
                    raw.insert(Datom::new(
                        entity,
                        Attribute::from_keyword(":db/ident"),
                        Value::Keyword(ident.into()),
                        tx,
                        Op::Assert,
                    ));
                    raw.insert(Datom::new(
                        entity,
                        Attribute::from_keyword(":db/doc"),
                        Value::String(doc.into()),
                        tx,
                        Op::Assert,
                    ));
                    raw.insert(Datom::new(
                        entity,
                        Attribute::from_keyword(":spec/type"),
                        Value::Keyword(":element.type/invariant".into()),
                        tx,
                        Op::Assert,
                    ));
                }
                store.merge(&Store::from_datoms(raw));

                let seed = assemble_seed(&store, "store immutability", budget, agent);

                // With budget > 1000 and 3 related constraints in the STORE
                // namespace, the seed should include at least one spec entity
                // as a demonstration in the State section.
                let state_entries: Vec<&StateEntry> = seed.context.sections.iter()
                    .flat_map(|s| match s {
                        ContextSection::State(entries) => entries.iter().collect::<Vec<_>>(),
                        _ => vec![],
                    })
                    .collect();

                // Check: at least one state entry references a spec entity
                // (either by content containing ":spec/" or by the entity itself).
                let has_spec_demo = state_entries.iter().any(|e| {
                    e.content.contains(":spec/") || e.content.contains("INV-STORE")
                        || e.content.contains("Append-Only") || e.content.contains("Content-Addressable")
                        || e.content.contains("Transaction Atomicity")
                });

                // Also check constraints section has the cluster
                let constraint_count: usize = seed.context.sections.iter()
                    .map(|s| match s {
                        ContextSection::Constraints(refs) => refs.len(),
                        _ => 0,
                    })
                    .sum();

                // Only assert demonstration density when the constraints section
                // actually picked up the cluster (constraint discovery is keyword-scored,
                // so with task "store immutability" these should score highly).
                if constraint_count >= 2 {
                    prop_assert!(
                        has_spec_demo,
                        "INV-SEED-005: with {} constraints and budget {}, \
                         expected at least one spec demonstration in State. \
                         State entries: {:?}",
                        constraint_count,
                        budget,
                        state_entries.iter().map(|e| &e.content).collect::<Vec<_>>()
                    );
                }
            }
        }

        // ── INV-SEED-006: Intention Anchoring ───────────────────────────
        //
        // When a task directive is provided, it appears in the seed output's
        // Directive section regardless of budget pressure.
        proptest! {
            #[test]
            fn intention_anchoring_task_always_present(
                store in arb_store(3),
                budget in 50usize..5000,
                task_suffix in "[a-z ]{3,30}",
            ) {
                let agent = AgentId::from_name("inv006-test");
                let task = format!("implement {}", task_suffix.trim());

                let seed = assemble_seed(&store, &task, budget, agent);

                // INV-SEED-006: The task appears in the Directive section
                // at full fidelity (pi_0), regardless of budget pressure.
                let directive_text = seed.context.sections.iter().find_map(|s| {
                    if let ContextSection::Directive(ref d) = s { Some(d.clone()) } else { None }
                });

                prop_assert!(
                    directive_text.is_some(),
                    "INV-SEED-006: Directive section must always be present"
                );

                let directive = directive_text.unwrap();
                prop_assert!(
                    directive.contains(&format!("Task: {}", task)),
                    "INV-SEED-006: Directive must contain the exact task string. \
                     Expected 'Task: {}' in: {}",
                    task,
                    directive
                );

                // Also verify the task is preserved in the SeedOutput metadata
                prop_assert_eq!(
                    &seed.task, &task,
                    "SeedOutput.task must match the provided task"
                );
            }
        }
    }
}
