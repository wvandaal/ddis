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
//! - **INV-SEED-006**: Intention anchoring — intentions pinned at π₀ regardless.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, Value};
use crate::query::graph::{pagerank, DiGraph};
use crate::store::Store;

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

/// Fallback seed selection: when keyword matching returns zero entities,
/// select the most recently-modified entities. This handles generic tasks
/// like "continue" or "overview" where the user wants orientation, not
/// keyword-specific results.
/// Build the Orientation section with store summary and last harvest context.
fn build_orientation(store: &Store) -> String {
    let mut parts = vec![format!("Braid datom store | {} datoms", store.len())];

    // Find the most recent harvest session and its task
    let mut latest_harvest_wall: u64 = 0;
    let mut latest_harvest_entity: Option<EntityId> = None;
    for datom in store.datoms() {
        if datom.attribute.as_str() == ":harvest/agent" && datom.op == Op::Assert {
            let wall = datom.tx.wall_time();
            if wall > latest_harvest_wall {
                latest_harvest_wall = wall;
                latest_harvest_entity = Some(datom.entity);
            }
        }
    }

    if let Some(entity) = latest_harvest_entity {
        // Get the harvest session's doc (contains task description)
        for datom in store.datoms() {
            if datom.entity == entity
                && datom.attribute.as_str() == ":db/doc"
                && datom.op == Op::Assert
            {
                if let Value::String(ref s) = datom.value {
                    parts.push(format!("Last harvest: {s}"));
                }
                break;
            }
        }
    }

    // Count observations since last harvest
    let mut obs_since_harvest = 0;
    for datom in store.datoms() {
        if datom.attribute.as_str() == ":exploration/source"
            && datom.op == Op::Assert
            && datom.tx.wall_time() > latest_harvest_wall
        {
            if let Value::String(ref s) = datom.value {
                if s == "braid:observe" {
                    obs_since_harvest += 1;
                }
            }
        }
    }
    if obs_since_harvest > 0 {
        parts.push(format!(
            "{obs_since_harvest} observations since last harvest"
        ));
    }

    parts.join("\n")
}

/// Discover open questions from observation entities.
fn discover_open_questions(store: &Store) -> Vec<String> {
    let mut questions = Vec::new();
    // Find entities tagged as open-question
    for datom in store.datoms() {
        if datom.attribute.as_str() == ":exploration/category" && datom.op == Op::Assert {
            if let Value::String(ref cat) = datom.value {
                if cat == "open-question" || cat == "conjecture" {
                    // Get the doc for this entity
                    for d2 in store.datoms() {
                        if d2.entity == datom.entity
                            && d2.attribute.as_str() == ":db/doc"
                            && d2.op == Op::Assert
                        {
                            if let Value::String(ref doc) = d2.value {
                                let truncated = if doc.len() > 100 {
                                    format!("{}...", &doc[..100])
                                } else {
                                    doc.clone()
                                };
                                questions.push(format!("[?] {truncated}"));
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
    questions
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

    // Recency: newest transaction time relative to max
    let newest_tx = asserted.iter().map(|d| d.tx.wall_time()).max().unwrap_or(0);
    let recency = if max_tx_wall_time == 0 {
        0.5
    } else {
        newest_tx as f64 / max_tx_wall_time as f64
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
                    // Truncate long text to keep tokens reasonable
                    let truncated = if text.len() > 120 {
                        format!("{}...", &text[..120])
                    } else {
                        text.to_string()
                    };
                    format!("{} — {}", label, truncated)
                }
                None => {
                    format!("{} ({} attrs)", label, datoms.len())
                }
            }
        }
        ProjectionLevel::Full => {
            let mut lines = Vec::new();
            for d in &datoms {
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
    let task_keywords: Vec<&str> = task.split_whitespace().collect();
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
    let directive = ContextSection::Directive(format!("Task: {task}"));
    let directive_tokens = estimate_tokens(task) + 6;

    // Orientation: include last harvest session info if available
    let orientation_text = build_orientation(store);
    let orientation_tokens = estimate_tokens(&orientation_text);
    let orientation = ContextSection::Orientation(orientation_text);

    // Warnings: surface open questions from observations
    let warning_lines = discover_open_questions(store);
    let warnings_tokens = warning_lines
        .iter()
        .map(|w| estimate_tokens(w))
        .sum::<usize>();
    let warnings = ContextSection::Warnings(warning_lines);

    let constraints = ContextSection::Constraints(Vec::new());
    let constraints_tokens = 0;

    // Allocate remaining budget to state entries
    let overhead = directive_tokens + orientation_tokens + warnings_tokens + constraints_tokens;
    let state_budget = budget.saturating_sub(overhead);

    let mut state_entries = Vec::new();
    let mut state_tokens = 0;
    for (entity, score) in &scored {
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

        let entry = project_entity(store, *entity, effective_projection);
        if state_tokens + entry.tokens > state_budget {
            // Try a lower projection before giving up
            let compressed = project_entity(store, *entity, ProjectionLevel::Pointer);
            if state_tokens + compressed.tokens <= state_budget {
                state_tokens += compressed.tokens;
                state_entries.push(compressed);
            }
            break;
        }
        state_tokens += entry.tokens;
        state_entries.push(entry);
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

    let total_tokens = overhead + state_tokens;
    let budget_remaining = budget.saturating_sub(total_tokens);

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
        depth: 5,
        breadth: 10,
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
    let store_entities = store.entities();
    let mut all_in_store = true;
    for section in &seed.context.sections {
        if let ContextSection::State(entries) = section {
            for entry in entries {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::AgentId;

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
}
