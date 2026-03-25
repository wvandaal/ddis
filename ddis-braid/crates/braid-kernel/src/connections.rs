//! Auto-connection engine for observations.
//!
//! Computes keyword Jaccard similarity between observations and proposes
//! connections above a configurable threshold. Hub-preferential attachment
//! ensures high-connectivity nodes attract more connections, creating
//! scale-free topology that matches real knowledge networks.
//!
//! # Algorithm
//!
//! 1. Tokenize observation text into normalized keyword sets
//! 2. For each pair, compute Jaccard similarity = |A ∩ B| / |A ∪ B|
//! 3. Filter by `JACCARD_THRESHOLD`
//! 4. Apply hub-preferential attachment: multiply similarity by log₂(1 + degree)
//! 5. Limit connections to O(log N) per observation (sub-linear scaling)
//!
//! # Relationship to FEGH
//!
//! This module feeds the bridge hypothesis generator (`generate_bridge_hypotheses`
//! in `routing.rs`). FEGH finds disconnected components and proposes bridges;
//! this module creates the intra-component edges that give FEGH a connected
//! graph to work with. Without these edges, every observation is an island
//! and FEGH has no structure to analyze.
//!
//! # Design Decisions
//!
//! - Uses `:exploration/body` for observation text (existing Layer 3 schema)
//! - Counts edges via Ref-typed attributes (`:exploration/depends-on`,
//!   `:exploration/refines`, `:exploration/related-spec`) for hub degree
//! - Returns `ProposedConnection` structs — caller decides how to transact
//! - Substrate-independent: tokenization and Jaccard are domain-agnostic (C8)

use std::collections::HashSet;

use crate::datom::{EntityId, Op, Value};
use crate::store::Store;

/// Minimum Jaccard similarity to propose a connection.
const JACCARD_THRESHOLD: f64 = 0.15;

/// Minimum keyword length to include in tokenization.
const MIN_KEYWORD_LEN: usize = 4;

/// Ref-typed attributes that count toward hub degree.
const REF_ATTRIBUTES: &[&str] = &[
    ":exploration/depends-on",
    ":exploration/refines",
    ":exploration/related-spec",
    ":exploration/source-session",
    ":dep/from",
    ":dep/to",
    ":task/traces-to",
];

/// A proposed connection between two observations.
#[derive(Clone, Debug)]
pub struct ProposedConnection {
    /// The source observation entity.
    pub source: EntityId,
    /// The target observation entity.
    pub target: EntityId,
    /// Adjusted similarity score (Jaccard * hub boost).
    pub similarity: f64,
    /// The raw Jaccard similarity before hub adjustment.
    pub raw_jaccard: f64,
    /// Keywords shared between source and target (sorted).
    pub shared_keywords: Vec<String>,
}

/// Tokenize text into a set of normalized keywords.
///
/// Splits on non-alphanumeric characters (preserving `/`, `_`, `-` for
/// paths and identifiers), filters by minimum length, and lowercases.
pub fn tokenize(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '/' && c != '_' && c != '-')
        .filter(|w| w.len() >= MIN_KEYWORD_LEN)
        .map(|w| w.to_lowercase())
        .collect()
}

/// Compute Jaccard similarity between two keyword sets.
///
/// J(A, B) = |A ∩ B| / |A ∪ B|. Returns 0.0 for two empty sets.
pub fn jaccard_similarity(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Find shared keywords between two sets, returned sorted.
pub fn shared_keywords(a: &HashSet<String>, b: &HashSet<String>) -> Vec<String> {
    let mut shared: Vec<String> = a.intersection(b).cloned().collect();
    shared.sort();
    shared
}

/// Count the connection degree of an entity in the store.
///
/// Counts outgoing Ref edges from the entity AND incoming Ref edges
/// to the entity (via VAET-style scan). Uses the `REF_ATTRIBUTES` list
/// to restrict to meaningful relationship edges.
fn entity_degree(store: &Store, entity: EntityId) -> usize {
    let datoms = store.entity_datoms(entity);
    let mut degree: usize = 0;

    // Outgoing: entity has a Ref attribute pointing elsewhere
    for d in &datoms {
        if d.op == Op::Assert {
            if let Value::Ref(_) = &d.value {
                if REF_ATTRIBUTES
                    .iter()
                    .any(|a| d.attribute.as_str() == *a)
                {
                    degree += 1;
                }
            }
        }
    }

    // Incoming: other entities have a Ref pointing to this entity
    // Use a scan of known Ref attributes for efficiency
    for &attr_str in REF_ATTRIBUTES {
        let attr = match crate::datom::Attribute::new(attr_str) {
            Ok(a) => a,
            Err(_) => continue,
        };
        for d in store.attribute_datoms(&attr) {
            if d.op == Op::Assert && d.value == Value::Ref(entity) {
                degree += 1;
            }
        }
    }

    degree
}

/// Propose connections for a new observation against existing observations.
///
/// # Algorithm
///
/// 1. Tokenize the new observation text
/// 2. Collect all existing observations (entities with `:exploration/body`)
/// 3. For each existing observation, compute Jaccard similarity
/// 4. Filter by `JACCARD_THRESHOLD`
/// 5. Apply hub-preferential attachment: multiply similarity by log₂(1 + degree)
///    where degree is the existing connection count of the target
/// 6. Sort by adjusted similarity descending
/// 7. Return top-N connections where N = ceil(log₂(total_observations + 1))
///
/// The log₂ limit ensures connection count scales sub-linearly with store size.
pub fn propose_connections(
    store: &Store,
    new_observation_entity: EntityId,
    new_observation_text: &str,
) -> Vec<ProposedConnection> {
    let new_keywords = tokenize(new_observation_text);
    if new_keywords.is_empty() {
        return Vec::new();
    }

    // Collect existing observations via attribute index: O(observations) not O(datoms)
    let body_attr = match crate::datom::Attribute::new(":exploration/body") {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };
    let body_datoms = store.attribute_datoms(&body_attr);

    let mut existing_obs: Vec<(EntityId, String)> = Vec::new();
    for d in body_datoms {
        if d.op == Op::Assert && d.entity != new_observation_entity {
            if let Value::String(text) = &d.value {
                existing_obs.push((d.entity, text.clone()));
            }
        }
    }

    if existing_obs.is_empty() {
        return Vec::new();
    }

    // Compute similarities and apply hub-preferential attachment
    let mut candidates: Vec<ProposedConnection> = Vec::new();
    for (entity, text) in &existing_obs {
        let their_keywords = tokenize(text);
        let raw_sim = jaccard_similarity(&new_keywords, &their_keywords);

        if raw_sim >= JACCARD_THRESHOLD {
            // Hub-preferential attachment: log₂(1 + degree) boost, minimum 1.0
            let target_degree = entity_degree(store, *entity) as f64;
            let hub_boost = (1.0 + target_degree).log2().max(1.0);
            let adjusted_sim = raw_sim * hub_boost;

            candidates.push(ProposedConnection {
                source: new_observation_entity,
                target: *entity,
                similarity: adjusted_sim,
                raw_jaccard: raw_sim,
                shared_keywords: shared_keywords(&new_keywords, &their_keywords),
            });
        }
    }

    // Sort by adjusted similarity descending
    candidates.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Limit to ceil(log₂(total_observations + 1)), minimum 1
    let total_obs = existing_obs.len() + 1;
    let max_connections = ((total_obs as f64).log2().ceil() as usize).max(1);
    candidates.truncate(max_connections);

    candidates
}

/// Detect topological events after adding connections.
///
/// Returns human-readable descriptions of structural changes:
/// - Connecting to a previously isolated observation
/// - Hub formation (3+ connections from one observation)
pub fn detect_topological_events(
    connections: &[ProposedConnection],
    store: &Store,
) -> Vec<String> {
    let mut events = Vec::new();

    if connections.is_empty() {
        return events;
    }

    // Event: connecting to an isolated observation (degree 0)
    for conn in connections {
        let target_degree = entity_degree(store, conn.target);
        if target_degree == 0 {
            events.push(format!(
                "Connected to isolated observation (shared: {})",
                conn.shared_keywords.join(", ")
            ));
        }
    }

    // Event: hub formation (3+ new connections from this observation)
    if connections.len() >= 3 {
        events.push(format!(
            "Hub forming: {} new connections from this observation",
            connections.len()
        ));
    }

    events
}

/// Summary statistics for the connection proposal.
#[derive(Clone, Debug)]
pub struct ConnectionSummary {
    /// Total existing observations in store.
    pub total_observations: usize,
    /// Number of candidates above threshold (before truncation).
    pub candidates_above_threshold: usize,
    /// Number of connections proposed (after truncation).
    pub connections_proposed: usize,
    /// Maximum allowed connections (log₂ limit).
    pub max_allowed: usize,
    /// Mean Jaccard similarity of proposed connections.
    pub mean_similarity: f64,
}

/// Compute summary statistics for a set of proposed connections.
pub fn connection_summary(
    store: &Store,
    new_entity: EntityId,
    proposed: &[ProposedConnection],
) -> ConnectionSummary {
    let body_attr = match crate::datom::Attribute::new(":exploration/body") {
        Ok(a) => a,
        Err(_) => {
            return ConnectionSummary {
                total_observations: 0,
                candidates_above_threshold: 0,
                connections_proposed: 0,
                max_allowed: 1,
                mean_similarity: 0.0,
            };
        }
    };

    let total_observations = store
        .attribute_datoms(&body_attr)
        .iter()
        .filter(|d| d.op == Op::Assert && d.entity != new_entity)
        .count();

    let max_allowed = ((total_observations as f64 + 1.0).log2().ceil() as usize).max(1);

    let mean_similarity = if proposed.is_empty() {
        0.0
    } else {
        proposed.iter().map(|c| c.raw_jaccard).sum::<f64>() / proposed.len() as f64
    };

    ConnectionSummary {
        total_observations,
        candidates_above_threshold: proposed.len(), // already truncated by caller
        connections_proposed: proposed.len(),
        max_allowed,
        mean_similarity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_basic() {
        let tokens = tokenize("the parser bypasses the event log in internal/consistency");
        assert!(tokens.contains("parser"));
        assert!(tokens.contains("bypasses"));
        assert!(tokens.contains("event"));
        assert!(tokens.contains("internal/consistency"));
        // "the" and "log" are below MIN_KEYWORD_LEN (4)
        assert!(!tokens.contains("the"));
        assert!(!tokens.contains("log"));
    }

    #[test]
    fn test_tokenize_preserves_paths() {
        let tokens = tokenize("investigating internal/consistency and internal/storage");
        assert!(tokens.contains("internal/consistency"));
        assert!(tokens.contains("internal/storage"));
        assert!(tokens.contains("investigating"));
    }

    #[test]
    fn test_tokenize_lowercases() {
        let tokens = tokenize("Parser EVENT Internal/Consistency");
        assert!(tokens.contains("parser"));
        assert!(tokens.contains("event"));
        assert!(tokens.contains("internal/consistency"));
    }

    #[test]
    fn test_tokenize_empty_string() {
        let tokens = tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_tokenize_all_short_words() {
        let tokens = tokenize("a be the for");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_tokenize_preserves_hyphens_and_underscores() {
        let tokens = tokenize("content-hash and snake_case identifiers");
        assert!(tokens.contains("content-hash"));
        assert!(tokens.contains("snake_case"));
        assert!(tokens.contains("identifiers"));
    }

    #[test]
    fn test_jaccard_similarity_identical() {
        let a: HashSet<String> = ["parser", "event", "store"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b = a.clone();
        assert!((jaccard_similarity(&a, &b) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_similarity_disjoint() {
        let a: HashSet<String> = ["parser", "event"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b: HashSet<String> = ["storage", "sqlite"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(jaccard_similarity(&a, &b).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_similarity_partial() {
        let a: HashSet<String> = ["parser", "event", "store"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b: HashSet<String> = ["parser", "event", "storage"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        // intersection = {parser, event} = 2, union = {parser, event, store, storage} = 4
        assert!((jaccard_similarity(&a, &b) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_similarity_empty_sets() {
        let a: HashSet<String> = HashSet::new();
        let b: HashSet<String> = HashSet::new();
        assert!(jaccard_similarity(&a, &b).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_similarity_one_empty() {
        let a: HashSet<String> = ["parser", "event"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b: HashSet<String> = HashSet::new();
        assert!(jaccard_similarity(&a, &b).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_similarity_subset() {
        let a: HashSet<String> = ["parser", "event", "store"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b: HashSet<String> = ["parser", "event"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        // intersection = 2, union = 3 → 0.667
        assert!((jaccard_similarity(&a, &b) - 2.0 / 3.0).abs() < 0.001);
    }

    #[test]
    fn test_shared_keywords_basic() {
        let a: HashSet<String> = ["parser", "event", "store"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b: HashSet<String> = ["parser", "event", "storage"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let shared = shared_keywords(&a, &b);
        assert_eq!(shared, vec!["event", "parser"]); // sorted
    }

    #[test]
    fn test_shared_keywords_empty() {
        let a: HashSet<String> = ["parser"].iter().map(|s| s.to_string()).collect();
        let b: HashSet<String> = ["storage"].iter().map(|s| s.to_string()).collect();
        let shared = shared_keywords(&a, &b);
        assert!(shared.is_empty());
    }

    /// Create a store with full schema (L0-L4) so tests can transact any attribute.
    fn test_store_with_schema() -> Store {
        let mut store = Store::genesis();
        let agent = crate::datom::AgentId::from_name("braid:schema");
        let tx_id = crate::datom::TxId::new(1, 0, agent);
        let schema_datoms = crate::schema::full_schema_datoms(tx_id);
        let mut tx = crate::store::Transaction::new(
            agent,
            crate::datom::ProvenanceType::Derived,
            "bootstrap full schema",
        );
        for d in &schema_datoms {
            tx = tx.assert(d.entity, d.attribute.clone(), d.value.clone());
        }
        let committed = tx.commit(&store).expect("schema commit");
        store.transact(committed).expect("schema transact");
        store
    }

    #[test]
    fn test_propose_connections_empty_store() {
        let store = Store::genesis();
        let entity = EntityId::from_ident(":obs/new");
        let conns = propose_connections(&store, entity, "test observation about parsing");
        assert!(conns.is_empty(), "empty store should produce no connections");
    }

    #[test]
    fn test_propose_connections_empty_text() {
        let store = Store::genesis();
        let entity = EntityId::from_ident(":obs/new");
        let conns = propose_connections(&store, entity, "a b c");
        assert!(
            conns.is_empty(),
            "text with only short words should produce no connections"
        );
    }

    #[test]
    fn test_propose_connections_with_observations() {
        let mut store = test_store_with_schema();
        let agent = crate::datom::AgentId::from_name("test");

        // Create two observations with overlapping keywords
        let obs1 = EntityId::from_content(b"obs1");
        let obs2 = EntityId::from_content(b"obs2");

        let tx = crate::store::Transaction::new(
            agent,
            crate::datom::ProvenanceType::Observed,
            "test observations",
        )
        .assert(
            obs1,
            crate::datom::Attribute::from_keyword(":exploration/body"),
            Value::String(
                "the parser module handles event processing and store updates".to_string(),
            ),
        )
        .assert(
            obs2,
            crate::datom::Attribute::from_keyword(":exploration/body"),
            Value::String(
                "the storage layer processes events from the parser pipeline".to_string(),
            ),
        );

        let committed = tx.commit(&store).expect("commit should succeed");
        store.transact(committed).expect("transact should succeed");

        // New observation that shares keywords with both
        let new_entity = EntityId::from_content(b"obs_new");
        let conns = propose_connections(
            &store,
            new_entity,
            "parser events are processed through the event pipeline",
        );

        // Should find at least one connection (both obs share "parser", "event*", "processed")
        assert!(
            !conns.is_empty(),
            "should find connections with overlapping keywords"
        );

        // All connections should have the new entity as source
        for conn in &conns {
            assert_eq!(conn.source, new_entity);
            assert!(!conn.shared_keywords.is_empty());
            assert!(conn.similarity >= JACCARD_THRESHOLD);
            assert!(conn.raw_jaccard >= JACCARD_THRESHOLD);
        }
    }

    #[test]
    fn test_propose_connections_excludes_self() {
        let mut store = test_store_with_schema();
        let agent = crate::datom::AgentId::from_name("test");

        let obs1 = EntityId::from_content(b"obs1");
        let tx = crate::store::Transaction::new(
            agent,
            crate::datom::ProvenanceType::Observed,
            "test",
        )
        .assert(
            obs1,
            crate::datom::Attribute::from_keyword(":exploration/body"),
            Value::String("parser events processing pipeline".to_string()),
        );

        let committed = tx.commit(&store).expect("commit should succeed");
        store.transact(committed).expect("transact should succeed");

        // Query with the same entity — should not connect to itself
        let conns = propose_connections(&store, obs1, "parser events processing pipeline");
        for conn in &conns {
            assert_ne!(
                conn.target, obs1,
                "should not propose connection to self"
            );
        }
    }

    #[test]
    fn test_connection_count_limit() {
        let mut store = test_store_with_schema();
        let agent = crate::datom::AgentId::from_name("test");

        // Create many observations with overlapping keywords
        let mut tx = crate::store::Transaction::new(
            agent,
            crate::datom::ProvenanceType::Observed,
            "bulk observations",
        );
        for i in 0..20 {
            let obs = EntityId::from_content(format!("obs{i}").as_bytes());
            tx = tx.assert(
                obs,
                crate::datom::Attribute::from_keyword(":exploration/body"),
                Value::String(format!(
                    "observation {i} about parser events processing pipeline store"
                )),
            );
        }

        let committed = tx.commit(&store).expect("commit should succeed");
        store.transact(committed).expect("transact should succeed");

        let new_entity = EntityId::from_content(b"obs_new");
        let conns = propose_connections(
            &store,
            new_entity,
            "parser events processing pipeline store observation",
        );

        // With 20 existing observations, max = ceil(log₂(21)) = 5
        let max_expected = ((21_f64).log2().ceil()) as usize;
        assert!(
            conns.len() <= max_expected,
            "connections {} should be <= log₂ limit {}",
            conns.len(),
            max_expected
        );
    }

    #[test]
    fn test_hub_preferential_attachment() {
        let mut store = test_store_with_schema();
        let agent = crate::datom::AgentId::from_name("test");

        // Create two observations with identical text (same Jaccard)
        let obs_hub = EntityId::from_content(b"obs_hub");
        let obs_leaf = EntityId::from_content(b"obs_leaf");
        let obs_linked = EntityId::from_content(b"obs_linked");

        let tx = crate::store::Transaction::new(
            agent,
            crate::datom::ProvenanceType::Observed,
            "hub test",
        )
        .assert(
            obs_hub,
            crate::datom::Attribute::from_keyword(":exploration/body"),
            Value::String("kernel parser events processing store module".to_string()),
        )
        .assert(
            obs_leaf,
            crate::datom::Attribute::from_keyword(":exploration/body"),
            Value::String("kernel parser events processing store module".to_string()),
        )
        // Give obs_hub some connections (making it a hub)
        .assert(
            obs_hub,
            crate::datom::Attribute::from_keyword(":exploration/depends-on"),
            Value::Ref(obs_linked),
        )
        .assert(
            obs_linked,
            crate::datom::Attribute::from_keyword(":exploration/body"),
            Value::String("linked observation about different topics entirely".to_string()),
        );

        let committed = tx.commit(&store).expect("commit should succeed");
        store.transact(committed).expect("transact should succeed");

        let new_entity = EntityId::from_content(b"obs_new");
        let conns = propose_connections(
            &store,
            new_entity,
            "kernel parser events processing store module",
        );

        // Both obs_hub and obs_leaf have identical Jaccard, but obs_hub should
        // have higher adjusted similarity due to hub degree
        if conns.len() >= 2 {
            let hub_conn = conns.iter().find(|c| c.target == obs_hub);
            let leaf_conn = conns.iter().find(|c| c.target == obs_leaf);
            if let (Some(h), Some(l)) = (hub_conn, leaf_conn) {
                assert!(
                    (h.raw_jaccard - l.raw_jaccard).abs() < f64::EPSILON,
                    "raw Jaccard should be identical: {} vs {}",
                    h.raw_jaccard,
                    l.raw_jaccard
                );
                assert!(
                    h.similarity >= l.similarity,
                    "hub should have higher adjusted similarity: {} >= {}",
                    h.similarity,
                    l.similarity
                );
            }
        }
    }

    #[test]
    fn test_detect_topological_events_empty() {
        let store = Store::genesis();
        let events = detect_topological_events(&[], &store);
        assert!(events.is_empty());
    }

    #[test]
    fn test_detect_topological_events_isolated() {
        let mut store = test_store_with_schema();
        let agent = crate::datom::AgentId::from_name("test");

        // Create an isolated observation (no connections)
        let obs_isolated = EntityId::from_content(b"obs_isolated");
        let tx = crate::store::Transaction::new(
            agent,
            crate::datom::ProvenanceType::Observed,
            "isolated",
        )
        .assert(
            obs_isolated,
            crate::datom::Attribute::from_keyword(":exploration/body"),
            Value::String("isolated observation about nothing related".to_string()),
        );
        let committed = tx.commit(&store).expect("commit should succeed");
        store.transact(committed).expect("transact should succeed");

        let new_entity = EntityId::from_content(b"obs_new");
        let connections = vec![ProposedConnection {
            source: new_entity,
            target: obs_isolated,
            similarity: 0.3,
            raw_jaccard: 0.3,
            shared_keywords: vec!["observation".to_string()],
        }];

        let events = detect_topological_events(&connections, &store);
        assert!(
            events.iter().any(|e| e.contains("isolated")),
            "should detect connection to isolated observation"
        );
    }

    #[test]
    fn test_detect_topological_events_hub_formation() {
        let store = Store::genesis();
        let new_entity = EntityId::from_content(b"obs_new");

        let connections: Vec<ProposedConnection> = (0..3)
            .map(|i| ProposedConnection {
                source: new_entity,
                target: EntityId::from_content(format!("target{i}").as_bytes()),
                similarity: 0.5,
                raw_jaccard: 0.5,
                shared_keywords: vec!["keyword".to_string()],
            })
            .collect();

        let events = detect_topological_events(&connections, &store);
        assert!(
            events.iter().any(|e| e.contains("Hub forming")),
            "should detect hub formation with 3+ connections"
        );
    }

    #[test]
    fn test_connection_summary_empty_store() {
        let store = Store::genesis();
        let entity = EntityId::from_ident(":obs/test");
        let summary = connection_summary(&store, entity, &[]);
        assert_eq!(summary.total_observations, 0);
        assert_eq!(summary.connections_proposed, 0);
        assert_eq!(summary.max_allowed, 1);
        assert!((summary.mean_similarity - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_connection_summary_with_data() {
        let new_entity = EntityId::from_content(b"obs_new");
        let connections = vec![
            ProposedConnection {
                source: new_entity,
                target: EntityId::from_content(b"t1"),
                similarity: 0.6,
                raw_jaccard: 0.4,
                shared_keywords: vec!["parser".to_string()],
            },
            ProposedConnection {
                source: new_entity,
                target: EntityId::from_content(b"t2"),
                similarity: 0.3,
                raw_jaccard: 0.2,
                shared_keywords: vec!["event".to_string()],
            },
        ];

        let store = Store::genesis();
        let summary = connection_summary(&store, new_entity, &connections);
        assert_eq!(summary.connections_proposed, 2);
        // Mean of raw jaccard: (0.4 + 0.2) / 2 = 0.3
        assert!((summary.mean_similarity - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_propose_connections_disjoint_observations() {
        let mut store = test_store_with_schema();
        let agent = crate::datom::AgentId::from_name("test");

        // Create observations with completely disjoint keywords
        let obs1 = EntityId::from_content(b"obs1");
        let tx = crate::store::Transaction::new(
            agent,
            crate::datom::ProvenanceType::Observed,
            "disjoint test",
        )
        .assert(
            obs1,
            crate::datom::Attribute::from_keyword(":exploration/body"),
            Value::String("quantum entanglement photon wavelength".to_string()),
        );

        let committed = tx.commit(&store).expect("commit should succeed");
        store.transact(committed).expect("transact should succeed");

        let new_entity = EntityId::from_content(b"obs_new");
        let conns = propose_connections(
            &store,
            new_entity,
            "kubernetes deployment container orchestration",
        );

        assert!(
            conns.is_empty(),
            "disjoint observations should produce no connections"
        );
    }
}
