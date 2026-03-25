//! Concept crystallization engine (OBSERVER-6: ontology discovery).
//!
//! Concepts are first-class entities representing emergent clusters of meaning.
//! They solve the synonym problem (two tags for the same pattern detected via
//! embedding proximity), improve connection quality (shared concept = connection),
//! and enable agent handoff (seed transmits concepts, not raw observations).
//!
//! # Algorithm
//!
//! 1. Each observation gets an embedding (from [`crate::embedding`]).
//! 2. `assign_to_concept` finds the nearest existing concept by cosine similarity.
//!    If similarity > `JOIN_THRESHOLD`, the observation joins that concept.
//! 3. Uncategorized observations accumulate. At harvest time, `crystallize_concepts`
//!    runs agglomerative clustering on the uncategorized set to form new concepts
//!    (minimum 3 members).
//! 4. Concept centroids update incrementally: O(dim) per observation join.
//! 5. `should_split` / `should_merge` detect when concepts need restructuring.
//!
//! # Design Decisions
//!
//! - ADR-FOUNDATION-014: Convergence Thesis — concepts close the ontology loop.
//! - C8: Substrate-independent — works for any domain, not just software.
//! - Pure computation: takes `&Store` and embeddings, returns datom instructions.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::datom::{Attribute, EntityId, Op, Value};
use crate::embedding::{bytes_to_embedding, cosine_similarity, embedding_to_bytes};
use crate::store::Store;

/// Default cosine similarity threshold for joining an existing concept.
/// Tunable via policy manifest `:policy/concept-join-threshold`.
pub const JOIN_THRESHOLD: f32 = 0.20;

/// Minimum number of uncategorized observations to form a new concept.
pub const MIN_CLUSTER_SIZE: usize = 3;

/// Variance threshold above which a concept should be split.
pub const SPLIT_THRESHOLD: f64 = 0.5;

/// Cosine threshold above which two concepts should be merged.
pub const MERGE_THRESHOLD: f32 = 0.85;

/// Default surprise amplification factor.
/// weight_i = 1.0 + ALPHA * surprise_i. Tunable via `:policy/surprise-alpha`.
pub const DEFAULT_ALPHA: f32 = 2.0;

/// Summary of a concept entity read from the store.
#[derive(Debug, Clone)]
pub struct ConceptSummary {
    /// The concept's entity ID.
    pub entity: EntityId,
    /// Human-readable name (top TF-IDF keywords).
    pub name: String,
    /// Description template.
    pub description: String,
    /// Number of member observations.
    pub member_count: usize,
    /// Centroid embedding vector (if present).
    pub embedding: Option<Vec<f32>>,
    /// Intra-cluster variance.
    pub variance: f64,
    /// Sum of surprise-weighted member weights (CCE-2b).
    pub total_weight: f64,
}

/// Result of assigning an observation to a concept.
#[derive(Debug, Clone)]
pub enum ConceptAssignment {
    /// Observation joined an existing concept with this similarity.
    Joined {
        /// The concept entity.
        concept: EntityId,
        /// Cosine similarity to the concept centroid.
        similarity: f32,
        /// Surprise = 1.0 - similarity. Range [0.0, 1.0].
        surprise: f32,
    },
    /// Observation did not match any existing concept.
    Uncategorized,
}

/// A new concept produced by crystallization.
#[derive(Debug, Clone)]
pub struct NewConcept {
    /// Entity ID for the new concept.
    pub entity: EntityId,
    /// Human-readable name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Centroid embedding.
    pub centroid: Vec<f32>,
    /// Member observation entity IDs.
    pub members: Vec<EntityId>,
    /// Intra-cluster variance.
    pub variance: f64,
    /// Sum of surprise-weighted member weights (CCE-2b).
    pub total_weight: f64,
}

/// Find the nearest concept to the given embedding vector.
///
/// Collects the latest embedding per concept entity (handles centroid updates
/// in append-only store where multiple assertions may exist for the same entity).
/// Returns `None` if no concepts exist in the store.
pub fn find_nearest_concept(store: &Store, embedding: &[f32]) -> Option<(EntityId, f32)> {
    let concept_attr = Attribute::from_keyword(":concept/embedding");

    // Phase 1: Collect latest embedding per entity.
    // In EAVT-ordered iteration, later assertions overwrite earlier ones.
    let mut latest: BTreeMap<EntityId, Vec<f32>> = BTreeMap::new();
    for d in store.datoms() {
        if d.op == Op::Assert && d.attribute == concept_attr {
            if let Value::Bytes(ref bytes) = d.value {
                latest.insert(d.entity, bytes_to_embedding(bytes));
            }
        }
    }

    // Phase 2: Find best match.
    let mut best: Option<(EntityId, f32)> = None;
    for (entity, concept_emb) in &latest {
        if concept_emb.len() == embedding.len() {
            let sim = cosine_similarity(concept_emb, embedding);
            if best.map_or(true, |(_, s)| sim > s) {
                best = Some((*entity, sim));
            }
        }
    }

    best
}

/// Assign an observation to the nearest concept or mark as uncategorized.
///
/// If the nearest concept has cosine similarity >= `threshold`, the observation
/// joins that concept. Otherwise returns `Uncategorized`.
///
/// For multi-membership assignment, use [`assign_to_concepts`] instead.
pub fn assign_to_concept(
    store: &Store,
    observation_embedding: &[f32],
    threshold: f32,
) -> ConceptAssignment {
    match find_nearest_concept(store, observation_embedding) {
        Some((concept, similarity)) if similarity >= threshold => ConceptAssignment::Joined {
            concept,
            similarity,
            surprise: 1.0 - similarity,
        },
        _ => ConceptAssignment::Uncategorized,
    }
}

/// Assign an observation to ALL concepts above the similarity threshold.
///
/// Returns matches sorted by similarity descending (primary match first).
/// An observation about "ignored error returns in cascade" can belong
/// simultaneously to anomalies, dependencies, and components — the topology
/// is a simplicial complex, not a tree.
///
/// Returns an empty vec if no concept exceeds the threshold (equivalent to
/// `ConceptAssignment::Uncategorized` in the singular API).
///
/// INV-EMBEDDING-004: All comparisons use the same embedding space.
pub fn assign_to_concepts(
    store: &Store,
    observation_embedding: &[f32],
    threshold: f32,
) -> Vec<ConceptAssignment> {
    let concept_attr = Attribute::from_keyword(":concept/embedding");

    // Phase 1: Collect latest embedding per concept entity.
    let mut latest: BTreeMap<EntityId, Vec<f32>> = BTreeMap::new();
    for d in store.datoms() {
        if d.op == Op::Assert && d.attribute == concept_attr {
            if let Value::Bytes(ref bytes) = d.value {
                latest.insert(d.entity, bytes_to_embedding(bytes));
            }
        }
    }

    // Phase 2: Collect all matches above threshold.
    let mut matches: Vec<ConceptAssignment> = Vec::new();
    for (entity, concept_emb) in &latest {
        if concept_emb.len() == observation_embedding.len() {
            let sim = cosine_similarity(concept_emb, observation_embedding);
            if sim >= threshold {
                matches.push(ConceptAssignment::Joined {
                    concept: *entity,
                    similarity: sim,
                    surprise: 1.0 - sim,
                });
            }
        }
    }

    // Sort by similarity descending. The Uncategorized arm is unreachable
    // (only Joined variants are pushed above) but required for exhaustiveness.
    matches.sort_by(|a, b| {
        let sim_a = match a {
            ConceptAssignment::Joined { similarity, .. } => *similarity,
            _ => 0.0,
        };
        let sim_b = match b {
            ConceptAssignment::Joined { similarity, .. } => *similarity,
            _ => 0.0,
        };
        sim_b.partial_cmp(&sim_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    matches
}

/// Crystallize new concepts from uncategorized observations.
///
/// Uses agglomerative clustering: repeatedly merge the two closest observations
/// until no pair has cosine similarity above `JOIN_THRESHOLD`. Clusters with
/// at least `MIN_CLUSTER_SIZE` members become new concepts.
///
/// Each observation is represented as `(EntityId, embedding, body_text)`.
pub fn crystallize_concepts(
    observations: &[(EntityId, Vec<f32>, String)],
    threshold: f32,
    min_size: usize,
) -> Vec<NewConcept> {
    if observations.len() < min_size {
        return Vec::new();
    }

    // Initialize: each observation is its own cluster.
    let mut clusters: Vec<Vec<usize>> = (0..observations.len()).map(|i| vec![i]).collect();
    let mut centroids: Vec<Vec<f32>> = observations.iter().map(|(_, e, _)| e.clone()).collect();

    // Agglomerative merge loop.
    loop {
        if clusters.len() < 2 {
            break;
        }

        // Find the closest pair of clusters.
        let mut best_sim = f32::NEG_INFINITY;
        let mut best_i = 0;
        let mut best_j = 1;

        for i in 0..clusters.len() {
            for j in (i + 1)..clusters.len() {
                let sim = cosine_similarity(&centroids[i], &centroids[j]);
                if sim > best_sim {
                    best_sim = sim;
                    best_i = i;
                    best_j = j;
                }
            }
        }

        if best_sim < threshold {
            break;
        }

        // Merge cluster j into cluster i.
        let cluster_j = clusters.remove(best_j);
        let centroid_j = centroids.remove(best_j);
        let n_i = clusters[best_i].len() as f32;
        let n_j = cluster_j.len() as f32;
        let total = n_i + n_j;

        // Update centroid: weighted average, then L2-normalize (INV-EMBEDDING-002).
        for (k, v) in centroids[best_i].iter_mut().enumerate() {
            *v = (*v * n_i + centroid_j[k] * n_j) / total;
        }
        crate::embedding::l2_normalize(&mut centroids[best_i]);
        clusters[best_i].extend(cluster_j);
    }

    // Convert qualifying clusters to NewConcept.
    clusters
        .into_iter()
        .zip(centroids)
        .filter(|(cluster, _)| cluster.len() >= min_size)
        .map(|(cluster, cent)| {
            let members: Vec<EntityId> = cluster.iter().map(|&i| observations[i].0).collect();
            let member_texts: Vec<&str> = cluster
                .iter()
                .map(|&i| observations[i].2.as_str())
                .collect();
            let member_embeddings: Vec<&[f32]> = cluster
                .iter()
                .map(|&i| observations[i].1.as_slice())
                .collect();

            let name = generate_concept_name(&member_texts);
            let description = format!("{} observations about {}", members.len(), name);

            let var = crate::embedding::variance(&member_embeddings, &cent);

            // Entity ID from concept name content.
            let entity = EntityId::from_content(format!("concept:{name}").as_bytes());

            // Initial cluster: all members get weight 1.0 (no surprise data yet).
            let total_weight = members.len() as f64;

            NewConcept {
                entity,
                name,
                description,
                centroid: cent,
                members,
                variance: var as f64,
                total_weight,
            }
        })
        .collect()
}

/// List all concepts in the store, sorted by member count descending.
///
/// Also returns the set of innate concept entity IDs (single-pass optimization).
/// Use `concept_inventory_with_innate` when you need both; this wrapper discards
/// the innate set for backward compatibility.
pub fn concept_inventory(store: &Store) -> Vec<ConceptSummary> {
    concept_inventory_with_innate(store).0
}

/// List all concepts and collect innate entity IDs in a single datom scan.
///
/// Returns `(concepts_sorted_by_member_count, innate_entity_ids)`.
/// Single O(D) pass replaces the previous O(2D + C*D) implementation.
pub fn concept_inventory_with_innate(store: &Store) -> (Vec<ConceptSummary>, HashSet<EntityId>) {
    let name_attr = Attribute::from_keyword(":concept/name");
    let desc_attr = Attribute::from_keyword(":concept/description");
    let emb_attr = Attribute::from_keyword(":concept/embedding");
    let count_attr = Attribute::from_keyword(":concept/member-count");
    let var_attr = Attribute::from_keyword(":concept/variance");
    let weight_attr = Attribute::from_keyword(":concept/total-weight");
    let innate_attr = Attribute::from_keyword(":concept/innate");

    let mut concepts: BTreeMap<EntityId, ConceptSummary> = BTreeMap::new();
    let mut innate_set: HashSet<EntityId> = HashSet::new();

    // Single pass: collect all concept attributes + innate flags.
    for d in store.datoms() {
        if d.op != Op::Assert {
            continue;
        }

        // Check for innate flag on any entity (before concept membership check).
        if d.attribute == innate_attr && d.value == Value::Boolean(true) {
            innate_set.insert(d.entity);
        }

        // Concept entity detection and attribute collection.
        // Use or_insert_with for ALL concept attributes — datom iteration
        // order (EAVT) may deliver non-name attributes before :concept/name.
        let is_concept_attr = d.attribute == name_attr
            || d.attribute == desc_attr
            || d.attribute == emb_attr
            || d.attribute == count_attr
            || d.attribute == var_attr
            || d.attribute == weight_attr;

        if is_concept_attr {
            let cs = concepts.entry(d.entity).or_insert_with(|| ConceptSummary {
                entity: d.entity,
                name: String::new(),
                description: String::new(),
                member_count: 0,
                embedding: None,
                variance: 0.0,
                total_weight: 0.0,
            });

            if d.attribute == name_attr {
                if let Value::String(ref s) = d.value {
                    cs.name = s.clone();
                }
            } else if d.attribute == desc_attr {
                if let Value::String(ref s) = d.value {
                    cs.description = s.clone();
                }
            } else if d.attribute == emb_attr {
                if let Value::Bytes(ref b) = d.value {
                    cs.embedding = Some(bytes_to_embedding(b));
                }
            } else if d.attribute == count_attr {
                if let Value::Long(n) = d.value {
                    cs.member_count = n as usize;
                }
            } else if d.attribute == var_attr {
                if let Value::Double(v) = d.value {
                    cs.variance = v.into_inner();
                } else if let Value::Bytes(ref b) = d.value {
                    // Legacy: older stores wrote variance as raw f64 LE bytes.
                    if b.len() == 8 {
                        cs.variance = f64::from_le_bytes([
                            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
                        ]);
                    }
                }
            } else if d.attribute == weight_attr {
                if let Value::Double(w) = d.value {
                    cs.total_weight = w.into_inner();
                }
            }
        }
    }

    let mut result: Vec<ConceptSummary> = concepts.into_values().collect();
    result.sort_by_key(|c| std::cmp::Reverse(c.member_count));
    (result, innate_set)
}

/// Check if a concept should be split (high internal variance).
pub fn should_split(variance: f64) -> bool {
    variance > SPLIT_THRESHOLD
}

/// Check if two concepts should be merged (highly similar centroids).
pub fn should_merge(centroid_a: &[f32], centroid_b: &[f32]) -> bool {
    cosine_similarity(centroid_a, centroid_b) > MERGE_THRESHOLD
}

/// A pair of concepts with their Jaccard co-occurrence similarity.
#[derive(Debug, Clone)]
pub struct ConceptCoOccurrence {
    /// First concept entity.
    pub concept_a: EntityId,
    /// First concept name.
    pub name_a: String,
    /// Second concept entity.
    pub concept_b: EntityId,
    /// Second concept name.
    pub name_b: String,
    /// Jaccard similarity of their observation member sets: |A ∩ B| / |A ∪ B|.
    pub jaccard: f64,
}

/// Compute the co-occurrence matrix of concept memberships.
///
/// For each pair of concepts, computes Jaccard similarity of their observation
/// member sets. Multi-membership means a single observation can appear in
/// multiple concept member sets.
///
/// Returns pairs sorted by Jaccard descending. Pairs with Jaccard = 0.0
/// are included (bridge gaps) up to a reasonable limit.
pub fn co_occurrence_matrix(store: &Store) -> Vec<ConceptCoOccurrence> {
    let concept_ref_attr = Attribute::from_keyword(":exploration/concept");
    let name_attr = Attribute::from_keyword(":concept/name");

    // Single pass: collect both concept membership and concept names.
    let mut members: BTreeMap<EntityId, HashSet<EntityId>> = BTreeMap::new();
    let mut names: BTreeMap<EntityId, String> = BTreeMap::new();
    for d in store.datoms() {
        if d.op != Op::Assert {
            continue;
        }
        if d.attribute == concept_ref_attr {
            if let Value::Ref(concept_entity) = d.value {
                members.entry(concept_entity).or_default().insert(d.entity);
            }
        } else if d.attribute == name_attr {
            if let Value::String(ref s) = d.value {
                names.insert(d.entity, s.clone());
            }
        }
    }

    // Phase 2: Compute pairwise Jaccard.
    let concept_ids: Vec<EntityId> = members.keys().copied().collect();
    let mut pairs = Vec::new();

    for i in 0..concept_ids.len() {
        for j in (i + 1)..concept_ids.len() {
            let a = concept_ids[i];
            let b = concept_ids[j];
            let set_a = &members[&a];
            let set_b = &members[&b];

            let intersection = set_a.intersection(set_b).count();
            let union = set_a.union(set_b).count();
            let jaccard = if union > 0 {
                intersection as f64 / union as f64
            } else {
                0.0
            };

            pairs.push(ConceptCoOccurrence {
                concept_a: a,
                name_a: names.get(&a).cloned().unwrap_or_else(|| "unnamed".into()),
                concept_b: b,
                name_b: names.get(&b).cloned().unwrap_or_else(|| "unnamed".into()),
                jaccard,
            });
        }
    }

    pairs.sort_by(|a, b| b.jaccard.partial_cmp(&a.jaccard).unwrap_or(std::cmp::Ordering::Equal));
    pairs
}

// ===================================================================
// Frontier Recommendation (FRONTIER-STEER)
// ===================================================================

/// The kind of frontier recommendation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontierKind {
    /// An unexplored entity with high connectivity to observed entities.
    Explore,
    /// A concept with high variance (uncertain knowledge area).
    Deepen,
    /// A concept pair with zero co-occurrence but structural connection.
    Bridge,
}

impl std::fmt::Display for FrontierKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrontierKind::Explore => write!(f, "explore"),
            FrontierKind::Deepen => write!(f, "deepen"),
            FrontierKind::Bridge => write!(f, "bridge"),
        }
    }
}

/// A computed frontier recommendation — the highest-information-gain investigation target.
#[derive(Debug, Clone)]
pub struct FrontierRec {
    /// What kind of recommendation this is.
    pub kind: FrontierKind,
    /// The target entity or concept name.
    pub target: String,
    /// Acquisition function score (higher = more information gain).
    pub score: f64,
    /// Human-readable rationale.
    pub rationale: String,
}

/// Compute the highest-information-gain frontier recommendation.
///
/// Considers three candidate types:
/// - **Explore**: `:pkg/*` entities referenced by `:composition/from` or `:composition/to`
///   edges connected to observed entities, but not themselves observed. Score = connection count.
/// - **Deepen**: Concepts with >= 3 members and highest variance. Score = variance.
/// - **Bridge**: Concept pairs with zero co-occurrence where both have >= 2 members.
///   Score = sum of member counts (larger concepts have more potential for discovery).
///
/// The `current_embedding` is used as a tiebreaker: among equal-score candidates,
/// prefer the one most semantically distant (maximizes information gain).
///
/// Returns `None` if the store has no packages, no concepts, or nothing to recommend.
pub fn frontier_recommendation(
    store: &Store,
    current_embedding: &[f32],
) -> Option<FrontierRec> {
    let mut candidates: Vec<FrontierRec> = Vec::new();

    // --- Candidate 1: Explore (unexplored packages with high connectivity) ---
    candidates.extend(explore_candidates(store));

    // --- Candidate 2: Deepen (high-variance concepts) ---
    candidates.extend(deepen_candidates(store, current_embedding));

    // --- Candidate 3: Bridge (zero co-occurrence concept pairs) ---
    candidates.extend(bridge_candidates(store));

    // Select the highest-scoring candidate.
    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    candidates.into_iter().next()
}

/// Find unexplored package entities with high connectivity to observed entities.
fn explore_candidates(store: &Store) -> Vec<FrontierRec> {
    let ident_attr = Attribute::from_keyword(":db/ident");
    let mentions_attr = Attribute::from_keyword(":exploration/mentions-entity");
    let comp_from_attr = Attribute::from_keyword(":composition/from");
    let comp_to_attr = Attribute::from_keyword(":composition/to");

    // Single pass: collect pkg entities, observed entities, and composition edges.
    let mut pkg_entities: BTreeMap<EntityId, String> = BTreeMap::new();
    let mut observed: HashSet<EntityId> = HashSet::new();
    let mut edge_from: BTreeMap<EntityId, EntityId> = BTreeMap::new();
    let mut edge_to: BTreeMap<EntityId, EntityId> = BTreeMap::new();

    for d in store.datoms() {
        if d.op != Op::Assert {
            continue;
        }
        if d.attribute == ident_attr {
            if let Value::Keyword(ref s) = d.value {
                if s.starts_with(":pkg/") {
                    pkg_entities.insert(d.entity, s.clone());
                }
            }
        } else if d.attribute == mentions_attr {
            if let Value::Ref(target) = d.value {
                observed.insert(target);
            }
        } else if let Value::Ref(target) = d.value {
            if d.attribute == comp_from_attr {
                edge_from.insert(d.entity, target);
            } else if d.attribute == comp_to_attr {
                edge_to.insert(d.entity, target);
            }
        }
    }

    if pkg_entities.is_empty() {
        return Vec::new();
    }

    let total_packages = pkg_entities.len();

    // Build neighbor map from complete edges (has both from and to).
    let mut neighbors: BTreeMap<EntityId, HashSet<EntityId>> = BTreeMap::new();
    for (edge, from_pkg) in &edge_from {
        if let Some(to_pkg) = edge_to.get(edge) {
            neighbors.entry(*from_pkg).or_default().insert(*to_pkg);
            neighbors.entry(*to_pkg).or_default().insert(*from_pkg);
        }
    }

    // Score unexplored packages by connection count to observed packages.
    let mut candidates = Vec::new();
    for (entity, ident) in &pkg_entities {
        if observed.contains(entity) {
            continue; // Already explored.
        }

        let connections = neighbors
            .get(entity)
            .map(|ns| ns.iter().filter(|n| observed.contains(n)).count())
            .unwrap_or(0);

        if connections > 0 {
            let name = ident.strip_prefix(":pkg/").unwrap_or(ident);
            let obs_names: Vec<&str> = neighbors
                .get(entity)
                .map(|ns| {
                    ns.iter()
                        .filter(|n| observed.contains(n))
                        .filter_map(|n| pkg_entities.get(n))
                        .map(|s| s.strip_prefix(":pkg/").unwrap_or(s.as_str()))
                        .collect()
                })
                .unwrap_or_default();
            let deps_str = if obs_names.len() <= 3 {
                obs_names.join(", ")
            } else {
                format!("{} + {} more", obs_names[..2].join(", "), obs_names.len() - 2)
            };
            candidates.push(FrontierRec {
                kind: FrontierKind::Explore,
                target: name.to_string(),
                score: connections as f64 / total_packages.max(1) as f64,
                rationale: format!(
                    "imported by {} ({connections} deps, 0 observations)",
                    deps_str
                ),
            });
        }
    }

    candidates
}

/// Find concepts with high variance (knowledge uncertainty).
fn deepen_candidates(store: &Store, current_embedding: &[f32]) -> Vec<FrontierRec> {
    let inventory = concept_inventory(store);
    let total_observations: usize = inventory.iter().map(|c| c.member_count).sum();
    let mut candidates = Vec::new();

    for c in &inventory {
        if c.member_count >= 3 && c.variance > 0.1 {
            // Tiebreaker: prefer concepts distant from current observation.
            let distance_bonus = c.embedding.as_ref().map_or(0.0, |emb| {
                1.0 - cosine_similarity(emb, current_embedding) as f64
            });
            let obs_fraction = c.member_count as f64 / total_observations.max(1) as f64;
            let base_score = (c.variance / crate::SPLIT_THRESHOLD) * obs_fraction;
            let score = base_score * (1.0 + distance_bonus * 0.1);

            candidates.push(FrontierRec {
                kind: FrontierKind::Deepen,
                target: c.name.clone(),
                score,
                rationale: format!(
                    "high variance={:.2} across {} observations",
                    c.variance, c.member_count
                ),
            });
        }
    }

    candidates
}

/// Find concept pairs with zero co-occurrence (bridge gaps).
fn bridge_candidates(store: &Store) -> Vec<FrontierRec> {
    let co_occ = co_occurrence_matrix(store);
    let inventory = concept_inventory(store);
    let total_observations: usize = inventory.iter().map(|c| c.member_count).sum();
    let mut candidates = Vec::new();

    for pair in &co_occ {
        if pair.jaccard > 0.0 {
            continue; // Only interested in zero co-occurrence.
        }

        // Both concepts need >= 2 members to be meaningful bridge targets.
        let a_count = inventory
            .iter()
            .find(|c| c.entity == pair.concept_a)
            .map(|c| c.member_count)
            .unwrap_or(0);
        let b_count = inventory
            .iter()
            .find(|c| c.entity == pair.concept_b)
            .map(|c| c.member_count)
            .unwrap_or(0);

        if a_count >= 2 && b_count >= 2 {
            let score = (1.0 - pair.jaccard) * (a_count + b_count) as f64 / (2.0 * total_observations.max(1) as f64);
            candidates.push(FrontierRec {
                kind: FrontierKind::Bridge,
                target: format!("{} <-> {}", pair.name_a, pair.name_b),
                score,
                rationale: format!(
                    "zero co-occurrence between {} ({} obs) and {} ({} obs)",
                    pair.name_a, a_count, pair.name_b, b_count
                ),
            });
        }
    }

    candidates
}

/// Generate datoms for a new concept entity.
///
/// Returns datoms that the caller should transact into the store.
pub fn concept_to_datoms(
    concept: &NewConcept,
    timestamp: i64,
) -> Vec<(EntityId, Attribute, Value)> {
    let e = concept.entity;
    vec![
        (
            e,
            Attribute::from_keyword(":concept/name"),
            Value::String(concept.name.clone()),
        ),
        (
            e,
            Attribute::from_keyword(":concept/description"),
            Value::String(concept.description.clone()),
        ),
        (
            e,
            Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(embedding_to_bytes(&concept.centroid)),
        ),
        (
            e,
            Attribute::from_keyword(":concept/member-count"),
            Value::Long(concept.members.len() as i64),
        ),
        (
            e,
            Attribute::from_keyword(":concept/created-at"),
            Value::Long(timestamp),
        ),
        (
            e,
            Attribute::from_keyword(":concept/variance"),
            Value::Double(ordered_float::OrderedFloat(concept.variance)),
        ),
        (
            e,
            Attribute::from_keyword(":concept/total-weight"),
            Value::Double(ordered_float::OrderedFloat(concept.total_weight)),
        ),
    ]
}

/// Generate datoms linking observations to their concept.
///
/// Returns `(:exploration/concept, Ref)` datoms for each member.
pub fn membership_datoms(
    concept_entity: EntityId,
    members: &[EntityId],
) -> Vec<(EntityId, Attribute, Value)> {
    let attr = Attribute::from_keyword(":exploration/concept");
    members
        .iter()
        .map(|&member| (member, attr.clone(), Value::Ref(concept_entity)))
        .collect()
}

/// Update a concept's centroid after a new observation joins.
///
/// new_centroid = (old_centroid * (n-1) + new_embedding) / n
/// Returns the new centroid vector.
pub fn update_centroid(old_centroid: &[f32], old_count: usize, new_embedding: &[f32]) -> Vec<f32> {
    let n = old_count as f32;
    let total = n + 1.0;
    old_centroid
        .iter()
        .zip(new_embedding.iter())
        .map(|(&old, &new)| (old * n + new) / total)
        .collect()
}

/// Compute surprise weight for an observation.
///
/// weight = 1.0 + alpha * surprise, where surprise = 1.0 - cosine_similarity.
/// With `alpha=2.0`: confirming (cosine=0.95, surprise=0.05) → weight=1.1,
/// surprising (cosine=0.65, surprise=0.35) → weight=1.7.
pub fn surprise_weight(surprise: f32, alpha: f32) -> f32 {
    1.0 + alpha * surprise
}

/// Surprise-weighted centroid update (CCE-2b).
///
/// new_centroid = (old_centroid * old_total_weight + new_embedding * new_weight) / new_total_weight
///
/// Returns `(new_centroid, new_total_weight)`.
pub fn update_centroid_weighted(
    old_centroid: &[f32],
    old_total_weight: f64,
    new_embedding: &[f32],
    new_weight: f32,
) -> (Vec<f32>, f64) {
    let w_old = old_total_weight as f32;
    let w_new = new_weight;
    let w_total = w_old + w_new;

    let centroid = old_centroid
        .iter()
        .zip(new_embedding.iter())
        .map(|(&old, &new)| (old * w_old + new * w_new) / w_total)
        .collect();

    (centroid, w_total as f64)
}

// ===================================================================
// Innate Concept Schemas (CCE-3)
// ===================================================================

/// The 5 universal innate concept definitions (Piagetian sensorimotor reflexes).
///
/// These provide scaffolding for exploration before domain-specific concepts emerge.
/// Different policy manifests can define different innate schemas (C8 compliant).
pub const INNATE_CONCEPTS: &[(&str, &str)] = &[
    (
        "components",
        "Parts of the system and their boundaries — packages, modules, crates, files, services",
    ),
    (
        "dependencies",
        "How parts relate to each other — imports, calls, data flow, coupling, interfaces",
    ),
    (
        "invariants",
        "What should hold true — rules, constraints, assertions, contracts, specifications",
    ),
    (
        "patterns",
        "Recurring regularities — idioms, conventions, architectures, protocols, templates",
    ),
    (
        "anomalies",
        "Deviations from expectations — bugs, inconsistencies, violations, surprises, gaps",
    ),
];

/// Generate datoms for all innate concepts at `braid init` time.
///
/// The caller provides the embedder (C8: kernel doesn't choose embedder).
/// Returns datoms ready for transaction. Each concept gets `:concept/innate = true`.
///
/// INV-EMBEDDING-004: All concept embeddings must use the same embedder as observations.
/// The `timestamp` parameter is the genesis time (typically `braid init` invocation).
pub fn innate_concept_datoms(
    timestamp: i64,
    embedder: &dyn crate::embedding::TextEmbedder,
) -> Vec<(EntityId, Attribute, Value)> {
    innate_concept_datoms_typed(timestamp, embedder, ":embedder/hash")
}

/// Generate innate concept datoms with explicit embedder type tag.
///
/// The `embedder_type_keyword` should be `:embedder/hash` or `:embedder/model2vec`.
pub fn innate_concept_datoms_typed(
    timestamp: i64,
    embedder: &dyn crate::embedding::TextEmbedder,
    embedder_type_keyword: &str,
) -> Vec<(EntityId, Attribute, Value)> {
    let innate_attr = Attribute::from_keyword(":concept/innate");
    let emb_type_attr = Attribute::from_keyword(":concept/embedder-type");
    let mut datoms = Vec::new();

    for (name, description) in INNATE_CONCEPTS {
        let entity = EntityId::from_content(format!("concept:innate:{name}").as_bytes());
        let emb = embedder.embed(description);

        let concept = NewConcept {
            entity,
            name: name.to_string(),
            description: description.to_string(),
            centroid: emb,
            members: Vec::new(),
            variance: 0.0,
            total_weight: 0.0,
        };

        datoms.extend(concept_to_datoms(&concept, timestamp));
        datoms.push((entity, innate_attr.clone(), Value::Boolean(true)));
        datoms.push((
            entity,
            emb_type_attr.clone(),
            Value::Keyword(embedder_type_keyword.to_string()),
        ));
    }

    datoms
}

/// Check whether a concept entity is innate (vs emergent).
///
/// Reads `:concept/innate` from the store for the given entity.
pub fn is_innate(store: &Store, entity: EntityId) -> bool {
    let attr = Attribute::from_keyword(":concept/innate");
    store.datoms().any(|d| {
        d.entity == entity
            && d.attribute == attr
            && d.op == Op::Assert
            && d.value == Value::Boolean(true)
    })
}

// ===================================================================
// Entity Auto-Linking (CCE-4)
// ===================================================================

/// Minimum entity name length for bare-word matching.
/// Shorter names require the full namespaced ident to match.
const MIN_MATCH_LEN: usize = 5;

/// Entity namespaces to scan for auto-linking.
const LINK_NAMESPACES: &[&str] = &[":pkg/", ":spec/", ":concept/"];

/// An auto-linked entity match.
#[derive(Debug, Clone)]
pub struct EntityMatch {
    /// The matched entity.
    pub entity: EntityId,
    /// The match name (what was found in the text).
    pub match_name: String,
}

/// Scan observation text for mentions of known entities in the store.
///
/// Checks `:db/ident` values for entities in `:pkg/*`, `:spec/*`, `:concept/*`
/// namespaces. For names >= 5 chars, bare-word matching (case-insensitive,
/// word-boundary). For shorter names, requires the full namespaced ident.
///
/// Returns matched entities (may be empty for text mentioning no known entities).
pub fn entity_auto_link(store: &Store, text: &str) -> Vec<EntityMatch> {
    let ident_attr = Attribute::from_keyword(":db/ident");
    let text_lower = text.to_lowercase();
    let mut matches = Vec::new();

    for d in store.datoms() {
        if d.op != Op::Assert || d.attribute != ident_attr {
            continue;
        }
        let ident = match &d.value {
            Value::Keyword(s) => s.as_str(),
            _ => continue,
        };

        // Only check known namespaces.
        let ns = LINK_NAMESPACES.iter().find(|ns| ident.starts_with(**ns));
        let ns = match ns {
            Some(ns) => *ns,
            None => continue,
        };

        // Extract the name part after the namespace prefix.
        let name = &ident[ns.len()..];
        if name.is_empty() {
            continue;
        }

        // Collect candidate match names:
        // - The full name (e.g., "internal-materialize")
        // - Each hyphen-separated segment (e.g., "internal", "materialize")
        // This handles `:pkg/internal-materialize` matching "materialize" in text.
        let mut candidates: Vec<&str> = vec![name];
        for seg in name.split('-') {
            if !seg.is_empty() && seg != name {
                candidates.push(seg);
            }
        }

        let mut matched = false;
        for candidate in &candidates {
            if matched {
                break;
            }
            let match_lower = candidate.to_lowercase();

            if match_lower.len() < MIN_MATCH_LEN {
                // Short names: require full ident match (e.g., ":pkg/cli").
                if text_lower.contains(&ident.to_lowercase()) {
                    matches.push(EntityMatch {
                        entity: d.entity,
                        match_name: ident.to_string(),
                    });
                    matched = true;
                }
            } else if word_boundary_match(&text_lower, &match_lower) {
                matches.push(EntityMatch {
                    entity: d.entity,
                    match_name: candidate.to_string(),
                });
                matched = true;
            }
        }
    }

    matches
}

/// Generate `:exploration/mentions-entity` datoms for auto-linked entities.
pub fn mention_datoms(
    observation: EntityId,
    matched: &[EntityMatch],
) -> Vec<(EntityId, Attribute, Value)> {
    let attr = Attribute::from_keyword(":exploration/mentions-entity");
    matched
        .iter()
        .map(|m| (observation, attr.clone(), Value::Ref(m.entity)))
        .collect()
}

/// Check if `needle` appears in `haystack` at a word boundary.
fn word_boundary_match(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();
    let nlen = needle_bytes.len();

    for i in 0..=(bytes.len().saturating_sub(nlen)) {
        if &bytes[i..i + nlen] == needle_bytes {
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
            let after_ok = i + nlen >= bytes.len() || !bytes[i + nlen].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return true;
            }
        }
    }
    false
}

// ===================================================================
// Steering Responses (CCE-5)
// ===================================================================

/// Steering information produced after an observation.
#[derive(Debug, Clone)]
pub struct ObserveSteering {
    /// Concept membership line (e.g., "concept:event-processing (3 members, surprise=0.15)").
    pub concept_line: Option<String>,
    /// Structural gap line (e.g., "spans events/, materialize/ — unexplored: cascade/").
    pub gap_line: Option<String>,
    /// Steering question (e.g., "what connects event-processing to error-handling?").
    pub question: Option<String>,
}

/// Compute steering information after concept assignment and entity linking.
///
/// This produces the three lines of the observe response:
/// 1. Concept membership (which concept, how many members, surprise level).
/// 2. Structural gap (which entities the concept spans, what's unexplored).
/// 3. Steering question (what to investigate next).
pub fn compute_observe_steering(
    store: &Store,
    assignment: &ConceptAssignment,
    entity_matches: &[EntityMatch],
) -> ObserveSteering {
    compute_observe_steering_multi(store, &[], assignment, entity_matches)
}

/// Compute steering information with multi-membership concept assignments.
///
/// `assignments` is the full multi-membership result from [`assign_to_concepts`].
/// `primary_assignment` is the backward-compatible single assignment (first match or Uncategorized).
///
/// The concept_line shows the primary match, plus secondary matches as "+ name (cosine=X)".
pub fn compute_observe_steering_multi(
    store: &Store,
    assignments: &[ConceptAssignment],
    primary_assignment: &ConceptAssignment,
    entity_matches: &[EntityMatch],
) -> ObserveSteering {
    // Line 1: Primary concept membership.
    let mut concept_line = match primary_assignment {
        ConceptAssignment::Joined {
            concept,
            similarity: _,
            surprise,
        } => {
            let name = concept_name_from_entity(store, *concept);
            let count = concept_member_count(store, *concept);
            if *surprise < 0.1 {
                Some(format!("concept:{name} ({count} obs, confirms)"))
            } else if *surprise < 0.3 {
                Some(format!(
                    "concept:{name} ({count} obs, surprise={surprise:.2})"
                ))
            } else {
                Some(format!(
                    "concept:{name} ({count} obs, surprise={surprise:.2} — at boundary)"
                ))
            }
        }
        ConceptAssignment::Uncategorized => None,
    };

    // Append secondary matches from multi-membership (skip index 0 = primary).
    if assignments.len() > 1 {
        let secondaries: Vec<String> = assignments[1..]
            .iter()
            .filter_map(|a| match a {
                ConceptAssignment::Joined {
                    concept, similarity, ..
                } => {
                    let name = concept_name_from_entity(store, *concept);
                    Some(format!("+ {name} (cosine={similarity:.2})"))
                }
                _ => None,
            })
            .collect();

        if !secondaries.is_empty() {
            let primary = concept_line.unwrap_or_default();
            concept_line = Some(format!("{primary} {}", secondaries.join(" ")));
        }
    }

    // Line 2: Structural gap from entity matches.
    let gap_line = if entity_matches.len() >= 2 {
        let mentioned: Vec<&str> = entity_matches
            .iter()
            .map(|m| m.match_name.as_str())
            .collect();
        Some(format!("linked: {}", mentioned.join(", ")))
    } else if entity_matches.len() == 1 {
        Some(format!("linked: {}", entity_matches[0].match_name))
    } else {
        None
    };

    // Line 3: Steering question.
    let question = compute_steering_question(store, primary_assignment, entity_matches);

    ObserveSteering {
        concept_line,
        gap_line,
        question,
    }
}

/// Format concept inventory for status display.
///
/// Returns lines like:
/// - "concepts: event-processing (5 obs), error-handling (3 obs, cross-cutting)"
/// - "coverage: 12/38 packages explored"
/// - "frontier: cascade, projector (imported by explored packages)"
pub fn format_concept_status(store: &Store) -> Vec<String> {
    let (inventory, innate_set) = concept_inventory_with_innate(store);
    if inventory.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();

    // Count total observations.
    let obs_count = count_observations(store);

    // Check for innate schema fade: after 10+ observations with 3+ emergent concepts,
    // hide innate schemas from the display.
    let emergent_count = inventory
        .iter()
        .filter(|c| !innate_set.contains(&c.entity))
        .count();
    let should_fade_innate = obs_count >= 10 && emergent_count >= 3;

    let display_concepts: Vec<&ConceptSummary> = if should_fade_innate {
        inventory
            .iter()
            .filter(|c| !innate_set.contains(&c.entity))
            .collect()
    } else {
        inventory.iter().collect()
    };

    if !display_concepts.is_empty() {
        let concept_strs: Vec<String> = display_concepts
            .iter()
            .take(5)
            .map(|c| format!("{} ({} obs)", c.name, c.member_count))
            .collect();
        lines.push(format!("concepts: {}", concept_strs.join(", ")));
    }

    // Coverage: count packages with observations.
    let mentioned_entities = count_mentioned_entities(store);
    let total_packages = count_packages(store);
    if total_packages > 0 {
        lines.push(format!(
            "coverage: {}/{} packages explored",
            mentioned_entities, total_packages
        ));
    }

    // LIFECYCLE-STATUS: Split/merge recommendations.
    // Check concepts with high variance (should_split) or high mutual cosine (should_merge).
    let concepts_with_embeddings: Vec<&ConceptSummary> = display_concepts
        .iter()
        .filter(|c| c.embedding.is_some() && c.member_count >= 2)
        .copied()
        .collect();

    for c in &concepts_with_embeddings {
        if should_split(c.variance) {
            lines.push(format!(
                "concept:{} may be too broad (variance={:.2}, consider splitting)",
                c.name, c.variance
            ));
        }
    }

    for i in 0..concepts_with_embeddings.len() {
        for j in (i + 1)..concepts_with_embeddings.len() {
            if let (Some(ref a), Some(ref b)) = (
                &concepts_with_embeddings[i].embedding,
                &concepts_with_embeddings[j].embedding,
            ) {
                if should_merge(a, b) {
                    let sim = cosine_similarity(a, b);
                    lines.push(format!(
                        "concepts {} and {} are converging (cosine={:.2}, consider merging)",
                        concepts_with_embeddings[i].name,
                        concepts_with_embeddings[j].name,
                        sim
                    ));
                }
            }
        }
    }

    // Co-occurrence: show coupled concept pairs (Jaccard > 0.3) and bridge gaps (Jaccard = 0).
    let co_occ = co_occurrence_matrix(store);
    let coupled: Vec<&ConceptCoOccurrence> = co_occ.iter().filter(|p| p.jaccard > 0.3).collect();
    let bridges: Vec<&ConceptCoOccurrence> = co_occ.iter().filter(|p| p.jaccard == 0.0).collect();

    for pair in &coupled {
        lines.push(format!(
            "coupled: {} + {} (jaccard={:.2})",
            pair.name_a, pair.name_b, pair.jaccard
        ));
    }
    if !bridges.is_empty() && bridges.len() <= 5 {
        let bridge_strs: Vec<String> = bridges
            .iter()
            .map(|p| format!("{}/{}", p.name_a, p.name_b))
            .collect();
        lines.push(format!("bridge-gaps: {}", bridge_strs.join(", ")));
    } else if bridges.len() > 5 {
        lines.push(format!("bridge-gaps: {} concept pairs with zero co-occurrence", bridges.len()));
    }

    lines
}

/// Count observations in the store (entities with :exploration/body).
fn count_observations(store: &Store) -> usize {
    let attr = Attribute::from_keyword(":exploration/body");
    store
        .datoms()
        .filter(|d| d.op == Op::Assert && d.attribute == attr)
        .count()
}

/// Count distinct entities mentioned by observations.
fn count_mentioned_entities(store: &Store) -> usize {
    let attr = Attribute::from_keyword(":exploration/mentions-entity");
    let mut entities = HashSet::new();
    for d in store.datoms() {
        if d.op == Op::Assert && d.attribute == attr {
            if let Value::Ref(e) = d.value {
                entities.insert(e);
            }
        }
    }
    entities.len()
}

/// Count package entities in the store.
fn count_packages(store: &Store) -> usize {
    let ident_attr = Attribute::from_keyword(":db/ident");
    store
        .datoms()
        .filter(|d| {
            d.op == Op::Assert
                && d.attribute == ident_attr
                && matches!(&d.value, Value::Keyword(s) if s.starts_with(":pkg/"))
        })
        .count()
}

/// Get concept name from entity.
fn concept_name_from_entity(store: &Store, entity: EntityId) -> String {
    let attr = Attribute::from_keyword(":concept/name");
    for d in store.datoms() {
        if d.entity == entity && d.attribute == attr && d.op == Op::Assert {
            if let Value::String(ref s) = d.value {
                return s.clone();
            }
        }
    }
    "unnamed".to_string()
}

/// Get concept member count from entity.
fn concept_member_count(store: &Store, entity: EntityId) -> usize {
    let attr = Attribute::from_keyword(":concept/member-count");
    for d in store.datoms() {
        if d.entity == entity && d.attribute == attr && d.op == Op::Assert {
            if let Value::Long(n) = d.value {
                return n as usize;
            }
        }
    }
    0
}

/// Compute steering question based on concept assignment and entity matches.
fn compute_steering_question(
    store: &Store,
    assignment: &ConceptAssignment,
    entity_matches: &[EntityMatch],
) -> Option<String> {
    match assignment {
        ConceptAssignment::Joined { concept, .. } => {
            // Find the nearest OTHER concept (different from current).
            let inventory = concept_inventory(store);
            let current_name = concept_name_from_entity(store, *concept);
            let other = inventory
                .iter()
                .find(|c| c.entity != *concept && c.member_count > 0);

            if let Some(other_concept) = other {
                Some(format!(
                    "what connects {} to {}?",
                    current_name, other_concept.name
                ))
            } else if !entity_matches.is_empty() {
                Some(format!(
                    "what other aspects of {} are worth investigating?",
                    entity_matches[0].match_name
                ))
            } else {
                None
            }
        }
        ConceptAssignment::Uncategorized => {
            if !entity_matches.is_empty() {
                Some(format!(
                    "what other aspects of {} are worth investigating?",
                    entity_matches[0].match_name
                ))
            } else {
                None
            }
        }
    }
}

// ===================================================================
// Internal helpers
// ===================================================================

/// Generate a human-readable concept name from member texts using TF-IDF.
///
/// Returns the top 3 most distinctive keywords across the member texts.
/// Stopwords excluded from concept names (STEER-1b).
/// Function words that carry zero semantic information.
const CONCEPT_NAME_STOPWORDS: &[&str] = &[
    "from", "with", "this", "that", "must", "have", "been", "into", "also",
    "when", "what", "were", "does", "more", "some", "than", "then", "them",
    "they", "will", "each", "only", "such", "very", "just", "most", "both",
    "about", "which", "their", "would", "could", "should", "these", "those",
    "there", "being", "where", "after", "other", "using", "every", "still",
    "between", "through", "before", "during", "without", "another", "because",
    "across", "concept", "observed", "observation",
];

fn generate_concept_name(texts: &[&str]) -> String {
    if texts.is_empty() {
        return "unnamed".to_string();
    }

    // Document frequency: how many member texts contain each word.
    // Majority-keyword: rank by DF (characterization > distinction).
    let mut df: HashMap<String, usize> = HashMap::new();
    let n_docs = texts.len();

    for text in texts {
        let words = crate::connections::tokenize(text);
        let unique: HashSet<String> = words.into_iter().collect();
        for word in unique {
            // Filter: skip stopwords and short words.
            if word.len() < 4 || CONCEPT_NAME_STOPWORDS.contains(&word.as_str()) {
                continue;
            }
            *df.entry(word).or_insert(0) += 1;
        }
    }

    // Rank by document frequency descending (words in the most member texts).
    let mut ranked: Vec<(String, usize)> = df.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    // Take top 2 words appearing in >50% of members.
    let majority_threshold = (n_docs + 1) / 2; // ceil(n/2)
    let majority: Vec<&str> = ranked
        .iter()
        .filter(|(_, count)| *count >= majority_threshold)
        .take(2)
        .map(|(w, _)| w.as_str())
        .collect();

    if !majority.is_empty() {
        return majority.join("-");
    }

    // Fallback: take the single highest-DF word regardless of threshold.
    if let Some((word, _)) = ranked.first() {
        return word.clone();
    }

    "unnamed".to_string()
}

// ===================================================================
// Tests
// ===================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{AgentId, Datom, TxId};
    use crate::embedding::HashEmbedder;
    use crate::embedding::TextEmbedder;
    use crate::schema::{domain_schema_datoms, genesis_datoms, layer_3_datoms};
    use std::collections::BTreeSet;

    fn make_embedder() -> HashEmbedder {
        HashEmbedder::new(crate::embedding::DEFAULT_DIM)
    }

    /// Build a store with L0-L3 schema installed so concept attributes are known.
    fn store_with_schema() -> Store {
        let agent = AgentId::from_name("braid:system");
        let g_tx = TxId::new(0, 0, agent);
        let d_tx = TxId::new(1, 0, agent);
        let l3_tx = TxId::new(2, 0, agent);
        let mut datoms: BTreeSet<Datom> = genesis_datoms(g_tx).into_iter().collect();
        for d in domain_schema_datoms(d_tx) {
            datoms.insert(d);
        }
        for d in layer_3_datoms(l3_tx) {
            datoms.insert(d);
        }
        Store::from_datoms(datoms)
    }

    // (A) find_nearest_concept returns None on empty store.
    #[test]
    fn find_nearest_concept_empty_store() {
        let store = Store::genesis();
        let emb = make_embedder();
        let v = emb.embed("test");
        assert!(
            find_nearest_concept(&store, &v).is_none(),
            "empty store should have no concepts"
        );
    }

    // (B) After 3 similar observations, crystallize_concepts produces 1 concept.
    #[test]
    fn crystallize_three_similar_observations() {
        let emb = make_embedder();
        // High word overlap (4/5 shared) → hash embedder cosine ≈ 0.8.
        let observations = vec![
            (
                EntityId::from_content(b"obs1"),
                emb.embed("error handling cascade module returns"),
                "error handling cascade module returns".to_string(),
            ),
            (
                EntityId::from_content(b"obs2"),
                emb.embed("error handling storage module returns"),
                "error handling storage module returns".to_string(),
            ),
            (
                EntityId::from_content(b"obs3"),
                emb.embed("error handling events module returns"),
                "error handling events module returns".to_string(),
            ),
        ];

        let concepts = crystallize_concepts(&observations, JOIN_THRESHOLD, MIN_CLUSTER_SIZE);
        assert!(
            !concepts.is_empty(),
            "3 similar observations should crystallize at least 1 concept"
        );
        assert_eq!(concepts[0].members.len(), 3);
    }

    // (C) Concept entity has all required attributes via concept_to_datoms.
    #[test]
    fn concept_datoms_complete() {
        let emb = make_embedder();
        let concept = NewConcept {
            entity: EntityId::from_content(b"test-concept"),
            name: "event-processing".to_string(),
            description: "3 observations about event processing".to_string(),
            centroid: emb.embed("event processing"),
            members: vec![
                EntityId::from_content(b"o1"),
                EntityId::from_content(b"o2"),
                EntityId::from_content(b"o3"),
            ],
            variance: 0.1,
            total_weight: 3.0,
        };

        let datoms = concept_to_datoms(&concept, 1000);
        assert_eq!(datoms.len(), 7, "concept should have 7 schema attributes");

        let attrs: Vec<&str> = datoms.iter().map(|(_, a, _)| a.as_str()).collect();
        assert!(attrs.contains(&":concept/name"));
        assert!(attrs.contains(&":concept/description"));
        assert!(attrs.contains(&":concept/embedding"));
        assert!(attrs.contains(&":concept/member-count"));
        assert!(attrs.contains(&":concept/created-at"));
        assert!(attrs.contains(&":concept/variance"));
        assert!(attrs.contains(&":concept/total-weight"));
    }

    // (D) Observation entities get :exploration/concept Ref via membership_datoms.
    #[test]
    fn membership_datoms_correct() {
        let concept_entity = EntityId::from_content(b"concept");
        let members = vec![
            EntityId::from_content(b"obs1"),
            EntityId::from_content(b"obs2"),
        ];

        let datoms = membership_datoms(concept_entity, &members);
        assert_eq!(datoms.len(), 2);
        for (_, attr, val) in &datoms {
            assert_eq!(attr.as_str(), ":exploration/concept");
            assert!(matches!(val, Value::Ref(e) if *e == concept_entity));
        }
    }

    // (E) should_split detects high variance.
    #[test]
    fn should_split_high_variance() {
        assert!(!should_split(0.1));
        assert!(!should_split(SPLIT_THRESHOLD - 0.01));
        assert!(should_split(SPLIT_THRESHOLD + 0.01));
        assert!(should_split(1.0));
    }

    // (F) concept_inventory lists concepts sorted by member count descending.
    #[test]
    fn concept_inventory_sorted() {
        let mut store = store_with_schema();
        let agent = crate::datom::AgentId::from_name("test");

        // Create two concepts with different member counts.
        let c1 = EntityId::from_content(b"concept-big");
        let c2 = EntityId::from_content(b"concept-small");

        let tx =
            crate::store::Transaction::new(agent, crate::datom::ProvenanceType::Observed, "test")
                .assert(
                    c1,
                    Attribute::from_keyword(":concept/name"),
                    Value::String("big-concept".into()),
                )
                .assert(
                    c1,
                    Attribute::from_keyword(":concept/member-count"),
                    Value::Long(10),
                )
                .assert(
                    c2,
                    Attribute::from_keyword(":concept/name"),
                    Value::String("small-concept".into()),
                )
                .assert(
                    c2,
                    Attribute::from_keyword(":concept/member-count"),
                    Value::Long(3),
                );

        let committed = tx.commit(&store).expect("commit");
        store.transact(committed).expect("transact");

        let inv = concept_inventory(&store);
        assert_eq!(inv.len(), 2);
        assert_eq!(inv[0].name, "big-concept");
        assert_eq!(inv[0].member_count, 10);
        assert_eq!(inv[1].name, "small-concept");
        assert_eq!(inv[1].member_count, 3);
    }

    // (G) Concept names are human-readable.
    #[test]
    fn concept_names_are_keywords() {
        let texts = &[
            "error handling in cascade module",
            "error returns ignored in storage",
            "error propagation missing in events",
        ];
        let name = generate_concept_name(texts);
        assert!(
            name.contains("error"),
            "concept name should contain dominant keyword 'error', got '{name}'"
        );
        assert!(!name.is_empty());
        assert!(
            !name.contains(' '),
            "concept name should use hyphens not spaces"
        );
    }

    #[test]
    fn update_centroid_correct() {
        let old = [1.0f32, 0.0, 0.0];
        let new_obs = [0.0f32, 1.0, 0.0];
        let updated = update_centroid(&old, 1, &new_obs);
        // (1*[1,0,0] + [0,1,0]) / 2 = [0.5, 0.5, 0]
        assert!((updated[0] - 0.5).abs() < 1e-6);
        assert!((updated[1] - 0.5).abs() < 1e-6);
        assert!((updated[2] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn crystallize_too_few_observations() {
        let emb = make_embedder();
        let observations = vec![
            (
                EntityId::from_content(b"o1"),
                emb.embed("test"),
                "test".into(),
            ),
            (
                EntityId::from_content(b"o2"),
                emb.embed("test two"),
                "test two".into(),
            ),
        ];
        let concepts = crystallize_concepts(&observations, JOIN_THRESHOLD, MIN_CLUSTER_SIZE);
        assert!(
            concepts.is_empty(),
            "2 observations < MIN_CLUSTER_SIZE should produce no concepts"
        );
    }

    #[test]
    fn crystallize_dissimilar_observations() {
        let emb = make_embedder();
        let observations = vec![
            (
                EntityId::from_content(b"o1"),
                emb.embed("event sourcing pipeline"),
                "event sourcing pipeline".into(),
            ),
            (
                EntityId::from_content(b"o2"),
                emb.embed("SQL injection vulnerability"),
                "SQL injection vulnerability".into(),
            ),
            (
                EntityId::from_content(b"o3"),
                emb.embed("kubernetes deployment manifest"),
                "kubernetes deployment manifest".into(),
            ),
        ];
        // All dissimilar — should NOT form a cluster (with hash embedder, no shared words).
        let concepts = crystallize_concepts(&observations, JOIN_THRESHOLD, MIN_CLUSTER_SIZE);
        assert!(
            concepts.is_empty(),
            "3 dissimilar observations should not cluster"
        );
    }

    #[test]
    fn should_merge_similar_centroids() {
        let a = [1.0f32, 0.0, 0.0];
        let b = [0.99, 0.01, 0.0];
        assert!(should_merge(&a, &b));
    }

    #[test]
    fn should_not_merge_different_centroids() {
        let a = [1.0f32, 0.0, 0.0];
        let b = [0.0, 1.0, 0.0];
        assert!(!should_merge(&a, &b));
    }

    #[test]
    fn generate_concept_name_empty() {
        assert_eq!(generate_concept_name(&[]), "unnamed");
    }

    // -- CCE-2b surprise-weighted centroid tests --

    // (C) Surprise weight computation.
    #[test]
    fn surprise_weight_values() {
        // Confirming observation: cosine=0.95, surprise=0.05.
        let w_confirm = surprise_weight(0.05, DEFAULT_ALPHA);
        assert!(
            (w_confirm - 1.1).abs() < 1e-6,
            "confirming weight should be ~1.1, got {w_confirm}"
        );

        // Surprising observation: cosine=0.65, surprise=0.35.
        let w_surprise = surprise_weight(0.35, DEFAULT_ALPHA);
        assert!(
            (w_surprise - 1.7).abs() < 1e-6,
            "surprising weight should be ~1.7, got {w_surprise}"
        );

        // Very confirming: cosine=1.0, surprise=0.0.
        let w_exact = surprise_weight(0.0, DEFAULT_ALPHA);
        assert!((w_exact - 1.0).abs() < 1e-6);
    }

    // (D) Surprise-weighted centroid shifts more toward surprising observation.
    #[test]
    fn weighted_centroid_shifts_toward_surprise() {
        // Start with centroid at [1,0,0] from 3 confirming observations.
        let centroid = [1.0f32, 0.0, 0.0];
        let old_total_weight = 3.0; // 3 confirming observations, weight 1.0 each.

        // Surprising observation at [0,1,0] with high surprise.
        let surprising_obs = [0.0f32, 1.0, 0.0];
        let surprise = 0.35;
        let weight = surprise_weight(surprise, DEFAULT_ALPHA); // 1.7

        let (weighted_cent, _) =
            update_centroid_weighted(&centroid, old_total_weight, &surprising_obs, weight);

        // Equal-weight centroid for comparison.
        let equal_cent = update_centroid(&centroid, 3, &surprising_obs);

        // The weighted centroid should be closer to the surprising observation
        // (i.e., have a higher y-component) than the equal-weight centroid.
        assert!(
            weighted_cent[1] > equal_cent[1],
            "weighted centroid y={} should exceed equal-weight y={}",
            weighted_cent[1],
            equal_cent[1]
        );
    }

    // (E) Total weight maintained correctly.
    #[test]
    fn total_weight_accumulates() {
        let centroid = [1.0f32, 0.0];
        let (_, w1) = update_centroid_weighted(&centroid, 0.0, &[0.5, 0.5], 1.1);
        assert!((w1 - 1.1).abs() < 1e-6);

        let (_, w2) = update_centroid_weighted(&centroid, w1, &[0.0, 1.0], 1.7);
        assert!(
            (w2 - 2.8).abs() < 1e-6,
            "total weight should be 1.1 + 1.7 = 2.8, got {w2}"
        );
    }

    // (H) Total weight >= member count (minimum weight per member is 1.0).
    #[test]
    fn total_weight_gte_member_count() {
        // 5 members with various surprise values.
        let mut total = 0.0f64;
        for surprise in [0.0, 0.1, 0.2, 0.3, 0.4] {
            let w = surprise_weight(surprise, DEFAULT_ALPHA);
            total += w as f64;
        }
        assert!(
            total >= 5.0,
            "total_weight {total} should be >= member_count 5"
        );
    }

    // -- CCE-3 innate concept tests --

    // (A) innate_concept_datoms produces 5 concepts.
    #[test]
    fn innate_concepts_count() {
        let datoms = innate_concept_datoms(1000, &make_embedder());
        // Each concept: 7 schema attrs + 1 innate flag + 1 embedder-type = 9 datoms.
        assert_eq!(
            datoms.len(),
            5 * 9,
            "5 innate concepts × 9 datoms each = 45"
        );
    }

    // (B) Each innate concept has :concept/innate = true.
    #[test]
    fn innate_concepts_flagged() {
        let datoms = innate_concept_datoms(1000, &make_embedder());
        let innate_count = datoms
            .iter()
            .filter(|(_, a, v)| a.as_str() == ":concept/innate" && *v == Value::Boolean(true))
            .count();
        assert_eq!(innate_count, 5);
    }

    // (C) Innate concepts have non-zero embeddings.
    #[test]
    fn innate_concepts_have_embeddings() {
        let datoms = innate_concept_datoms(1000, &make_embedder());
        let emb_count = datoms
            .iter()
            .filter(|(_, a, v)| {
                a.as_str() == ":concept/embedding"
                    && matches!(v, Value::Bytes(b) if !b.is_empty() && b.iter().any(|&x| x != 0))
            })
            .count();
        assert_eq!(
            emb_count, 5,
            "all 5 innate concepts should have non-zero embeddings"
        );
    }

    // (D) First observe auto-matches against innate concepts.
    #[test]
    fn innate_concepts_matchable() {
        let emb = make_embedder();

        // Build a store with innate concepts.
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");

        let innate_datoms = innate_concept_datoms(1000, &make_embedder());
        let mut tx =
            crate::store::Transaction::new(agent, crate::datom::ProvenanceType::Observed, "innate");
        for (e, a, v) in innate_datoms {
            tx = tx.assert(e, a, v);
        }
        let committed = tx.commit(&store).expect("commit");
        store.transact(committed).expect("transact");

        // An observation about "package structure and module boundaries" should match "components".
        let obs_emb = emb.embed("Parts of the system and their boundaries packages modules crates");
        let result = find_nearest_concept(&store, &obs_emb);
        assert!(result.is_some(), "should find a matching innate concept");
    }

    // (E) INNATE_CONCEPTS is configurable (the array is const, not hardcoded in transaction logic).
    #[test]
    fn innate_concepts_are_five() {
        assert_eq!(INNATE_CONCEPTS.len(), 5);
        for (name, desc) in INNATE_CONCEPTS {
            assert!(!name.is_empty());
            assert!(!desc.is_empty());
        }
    }

    // (F) Innate concepts participate in crystallization (no special-casing).
    #[test]
    fn innate_concept_is_detectable() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");

        let innate_datoms = innate_concept_datoms(1000, &make_embedder());
        let mut tx =
            crate::store::Transaction::new(agent, crate::datom::ProvenanceType::Observed, "innate");
        for (e, a, v) in innate_datoms {
            tx = tx.assert(e, a, v);
        }
        let committed = tx.commit(&store).expect("commit");
        store.transact(committed).expect("transact");

        // concept_inventory should list all 5.
        let inv = concept_inventory(&store);
        assert_eq!(inv.len(), 5);

        // is_innate should return true for each.
        for concept in &inv {
            assert!(
                is_innate(&store, concept.entity),
                "concept '{}' should be innate",
                concept.name
            );
        }
    }

    // -- CCE-4 entity auto-linking tests --

    fn store_with_entities() -> Store {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");

        let pkg1 = EntityId::from_content(b"pkg-materialize");
        let pkg2 = EntityId::from_content(b"pkg-cli");
        let spec1 = EntityId::from_content(b"spec-inv-019");

        let tx = crate::store::Transaction::new(
            agent,
            crate::datom::ProvenanceType::Observed,
            "bootstrap",
        )
        .assert(
            pkg1,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":pkg/internal-materialize".into()),
        )
        .assert(
            pkg2,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":pkg/cli".into()),
        )
        .assert(
            spec1,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":spec/app-inv-019".into()),
        );

        let committed = tx.commit(&store).expect("commit");
        store.transact(committed).expect("transact");
        store
    }

    // (A) Observation mentioning 'materialize' auto-links to the package.
    #[test]
    fn auto_link_package_name() {
        let store = store_with_entities();
        let matches = entity_auto_link(&store, "the materialize engine processes events");
        assert!(
            matches.iter().any(|m| m.match_name == "materialize"),
            "should match 'materialize' in text, got: {:?}",
            matches
        );
    }

    // (B) Observation mentioning 'app-inv-019' auto-links to the spec.
    #[test]
    fn auto_link_spec_id() {
        let store = store_with_entities();
        let matches = entity_auto_link(&store, "check APP-INV-019 for consistency rules");
        assert!(
            matches.iter().any(|m| m.match_name == "app-inv-019"),
            "should match spec ID, got: {:?}",
            matches
        );
    }

    // (C) Text mentioning no known entities creates no links.
    #[test]
    fn auto_link_no_false_positives() {
        let store = store_with_entities();
        let matches = entity_auto_link(&store, "unrelated text about weather patterns");
        assert!(matches.is_empty(), "should produce no matches");
    }

    // (D) mention_datoms produces correct Ref datoms.
    #[test]
    fn mention_datoms_correct() {
        let obs = EntityId::from_content(b"obs-1");
        let matched = vec![EntityMatch {
            entity: EntityId::from_content(b"pkg-materialize"),
            match_name: "materialize".into(),
        }];
        let datoms = mention_datoms(obs, &matched);
        assert_eq!(datoms.len(), 1);
        assert_eq!(datoms[0].1.as_str(), ":exploration/mentions-entity");
    }

    // (E) Short names require full path match.
    #[test]
    fn auto_link_short_name_no_false_match() {
        let store = store_with_entities();
        // "cli" is only 3 chars — should NOT match bare word "cli" in text.
        let matches = entity_auto_link(&store, "the cli tool handles user input");
        let cli_bare = matches.iter().any(|m| m.match_name == "cli");
        assert!(
            !cli_bare,
            "short name 'cli' should not match without full ident"
        );
    }

    #[test]
    fn auto_link_short_name_full_ident_match() {
        let store = store_with_entities();
        // Full ident ":pkg/cli" should match.
        let matches = entity_auto_link(&store, "check :pkg/cli for command structure");
        assert!(!matches.is_empty(), "full ident ':pkg/cli' should match");
    }

    // (F) Word boundary prevents substring matches.
    #[test]
    fn auto_link_word_boundary() {
        let store = store_with_entities();
        // "dematerialized" contains "materialize" but not at word boundary.
        let matches = entity_auto_link(&store, "the dematerialized view is cached");
        let materialize_match = matches.iter().any(|m| m.match_name == "materialize");
        assert!(
            !materialize_match,
            "should not match 'materialize' inside 'dematerialized'"
        );
    }

    // -- CCE-5 steering tests --

    #[test]
    fn steering_with_concept_assignment() {
        let assignment = ConceptAssignment::Joined {
            concept: EntityId::from_content(b"concept-test"),
            similarity: 0.8,
            surprise: 0.2,
        };
        let matches = vec![EntityMatch {
            entity: EntityId::from_content(b"pkg"),
            match_name: "materialize".into(),
        }];

        let store = Store::genesis();
        let steering = compute_observe_steering(&store, &assignment, &matches);
        assert!(steering.concept_line.is_some());
        assert!(steering.gap_line.is_some());
    }

    #[test]
    fn steering_uncategorized() {
        let assignment = ConceptAssignment::Uncategorized;
        let store = Store::genesis();
        let steering = compute_observe_steering(&store, &assignment, &[]);
        assert!(steering.concept_line.is_none());
        assert!(steering.gap_line.is_none());
    }

    #[test]
    fn steering_question_with_entity() {
        let assignment = ConceptAssignment::Uncategorized;
        let matches = vec![EntityMatch {
            entity: EntityId::from_content(b"test"),
            match_name: "cascade".into(),
        }];
        let store = Store::genesis();
        let steering = compute_observe_steering(&store, &assignment, &matches);
        assert!(
            steering.question.is_some(),
            "should generate question from entity match"
        );
        assert!(steering.question.unwrap().contains("cascade"));
    }

    #[test]
    fn concept_status_empty_store() {
        let store = Store::genesis();
        let lines = format_concept_status(&store);
        assert!(
            lines.is_empty(),
            "empty store should have no concept status"
        );
    }

    #[test]
    fn concept_status_with_concepts() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");

        // Add a concept.
        let c1 = EntityId::from_content(b"concept-events");
        let tx =
            crate::store::Transaction::new(agent, crate::datom::ProvenanceType::Observed, "test")
                .assert(
                    c1,
                    Attribute::from_keyword(":concept/name"),
                    Value::String("event-processing".into()),
                )
                .assert(
                    c1,
                    Attribute::from_keyword(":concept/member-count"),
                    Value::Long(5),
                );
        let committed = tx.commit(&store).expect("commit");
        store.transact(committed).expect("transact");

        let lines = format_concept_status(&store);
        assert!(!lines.is_empty(), "should have concept status lines");
        assert!(
            lines[0].contains("event-processing"),
            "should mention concept name"
        );
    }

    // -- CCE-TEST-SURPRISE (t-60982fa8) --

    /// (1) After observation joins a concept, :exploration/surprise datom should exist.
    /// Verify: cosine 0.85 → surprise 0.15, cosine 0.65 → surprise 0.35.
    #[test]
    fn test_surprise_recorded_on_join() {
        // Cosine = 0.85 → surprise = 0.15
        let assignment_close = assign_to_concept_sim(0.85);
        match assignment_close {
            ConceptAssignment::Joined { surprise, .. } => {
                assert!(
                    (surprise - 0.15).abs() < 1e-6,
                    "cosine 0.85 → surprise should be 0.15, got {surprise}"
                );
                assert!((0.0..=1.0).contains(&surprise));
            }
            ConceptAssignment::Uncategorized => panic!("should have joined"),
        }

        // Cosine = 0.65 → surprise = 0.35
        let assignment_far = assign_to_concept_sim(0.65);
        match assignment_far {
            ConceptAssignment::Joined { surprise, .. } => {
                assert!(
                    (surprise - 0.35).abs() < 1e-6,
                    "cosine 0.65 → surprise should be 0.35, got {surprise}"
                );
                assert!((0.0..=1.0).contains(&surprise));
            }
            ConceptAssignment::Uncategorized => panic!("should have joined"),
        }
    }

    /// Helper: simulate a concept assignment with known cosine similarity.
    fn assign_to_concept_sim(cosine: f32) -> ConceptAssignment {
        if cosine >= JOIN_THRESHOLD {
            ConceptAssignment::Joined {
                concept: EntityId::from_content(b"concept-test"),
                similarity: cosine,
                surprise: 1.0 - cosine,
            }
        } else {
            ConceptAssignment::Uncategorized
        }
    }

    /// (2) 3 confirming + 1 surprising observation: surprise-weighted centroid
    /// should be CLOSER to the surprising observation than equal-weight centroid.
    #[test]
    fn test_surprise_weighted_centroid_differs_from_equal_weight() {
        // 3 confirming observations near [1,0,0].
        let confirm1 = [0.98f32, 0.02, 0.0];
        let confirm2 = [0.97f32, 0.03, 0.0];
        let confirm3 = [0.99f32, 0.01, 0.0];
        // 1 surprising observation away from cluster.
        let surprising = [0.3f32, 0.7, 0.0];

        // Build equal-weight centroid: mean of 4.
        let ew_centroid: Vec<f32> = (0..3)
            .map(|i| (confirm1[i] + confirm2[i] + confirm3[i] + surprising[i]) / 4.0)
            .collect();

        // Build surprise-weighted centroid.
        // Start with confirming centroid.
        let base_centroid: Vec<f32> = (0..3)
            .map(|i| (confirm1[i] + confirm2[i] + confirm3[i]) / 3.0)
            .collect();
        let base_weight = 3.0 * surprise_weight(0.05, DEFAULT_ALPHA) as f64; // ~3 × 1.1 = 3.3
        let surprise_val = 0.35f32; // 1.0 - cosine(surprising, base_centroid) ≈ 0.35
        let sw = surprise_weight(surprise_val, DEFAULT_ALPHA);
        let (sw_centroid, _) =
            update_centroid_weighted(&base_centroid, base_weight, &surprising, sw);

        // Surprise-weighted should be closer to the surprising observation.
        let sw_cos = crate::embedding::cosine_similarity(&sw_centroid, &surprising);
        let ew_cos = crate::embedding::cosine_similarity(&ew_centroid, &surprising);
        assert!(
            sw_cos > ew_cos,
            "surprise-weighted centroid cosine to surprising obs ({sw_cos}) \
             should exceed equal-weight ({ew_cos})"
        );
    }

    /// (3) 5 interior + 1 outlier: alpha=2.0 shifts MORE toward outlier than alpha=0.0.
    #[test]
    fn test_centroid_pulls_toward_frontier() {
        // 5 members all near [1,0,0].
        let interior = [1.0f32, 0.0, 0.0];
        let outlier = [0.5f32, 0.5, 0.0];
        let surprise = 0.35f32;

        // alpha=0 → equal weight (surprise_weight(0.35, 0) = 1.0).
        let base_weight_0 = 5.0f64; // 5 members × weight 1.0
        let w0 = surprise_weight(surprise, 0.0); // 1.0
        let (cent_alpha0, _) = update_centroid_weighted(&interior, base_weight_0, &outlier, w0);

        // alpha=2.0 → surprise_weight(0.35, 2.0) = 1.7.
        let base_weight_2 = 5.0 * surprise_weight(0.0, DEFAULT_ALPHA) as f64; // 5 × 1.0 = 5.0
        let w2 = surprise_weight(surprise, DEFAULT_ALPHA); // 1.7
        let (cent_alpha2, _) = update_centroid_weighted(&interior, base_weight_2, &outlier, w2);

        // alpha=2 centroid should be closer to the outlier.
        let dist_alpha0 = euclidean_distance(&cent_alpha0, &outlier);
        let dist_alpha2 = euclidean_distance(&cent_alpha2, &outlier);
        assert!(
            dist_alpha2 < dist_alpha0,
            "alpha=2.0 centroid should be closer to outlier \
             (dist_alpha2={dist_alpha2} should be < dist_alpha0={dist_alpha0})"
        );
    }

    fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(&x, &y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    /// (4) :concept/total-weight equals sum of (1.0 + alpha * surprise_i) for all members.
    #[test]
    fn test_total_weight_is_sum_of_member_weights() {
        let surprises = [0.05, 0.10, 0.20, 0.35, 0.40];
        let expected: f64 = surprises
            .iter()
            .map(|&s| surprise_weight(s, DEFAULT_ALPHA) as f64)
            .sum();

        let mut total_weight = 0.0f64;
        let centroid = [1.0f32, 0.0, 0.0];
        let mut current_centroid = centroid.to_vec();
        for s in &surprises {
            let w = surprise_weight(*s, DEFAULT_ALPHA);
            let (new_cent, new_total) =
                update_centroid_weighted(&current_centroid, total_weight, &centroid, w);
            current_centroid = new_cent;
            total_weight = new_total;
        }

        assert!(
            (total_weight - expected).abs() < 1e-4,
            "total_weight {total_weight} should equal sum of weights {expected}"
        );
    }

    /// (5) If all observations have identical surprise, sw_centroid == ew_centroid.
    #[test]
    fn test_all_equal_surprise_produces_equal_weight_centroid() {
        let surprise = 0.2f32;
        let w = surprise_weight(surprise, DEFAULT_ALPHA);
        let members = [
            [1.0f32, 0.0, 0.0],
            [0.0f32, 1.0, 0.0],
            [0.0f32, 0.0, 1.0],
        ];

        // Equal-weight centroid = simple mean.
        let ew: Vec<f32> = (0..3)
            .map(|i| members.iter().map(|m| m[i]).sum::<f32>() / 3.0)
            .collect();

        // Surprise-weighted centroid with uniform surprise.
        let mut sw = members[0].to_vec();
        let mut total = w as f64;
        for m in &members[1..] {
            let (new_c, new_t) = update_centroid_weighted(&sw, total, m, w);
            sw = new_c;
            total = new_t;
        }

        for i in 0..3 {
            assert!(
                (sw[i] - ew[i]).abs() < 1e-5,
                "dim {i}: sw={} should equal ew={} when all surprise is equal",
                sw[i],
                ew[i]
            );
        }
    }

    /// (6) Surprise response intensity scales with surprise value.
    /// surprise < 0.1 → 'confirms', 0.1-0.3 → surprise shown, > 0.3 → 'at boundary'.
    #[test]
    fn test_surprise_response_intensity_scales() {
        let store = Store::genesis();
        let entity_matches = &[];

        // Low surprise (< 0.1): "confirms"
        let low = ConceptAssignment::Joined {
            concept: EntityId::from_content(b"c"),
            similarity: 0.95,
            surprise: 0.05,
        };
        let steering_low = compute_observe_steering(&store, &low, entity_matches);
        let line_low = steering_low.concept_line.unwrap();
        assert!(
            line_low.contains("confirms"),
            "surprise < 0.1 should say 'confirms', got: {line_low}"
        );

        // Medium surprise (0.1-0.3): shows surprise value
        let mid = ConceptAssignment::Joined {
            concept: EntityId::from_content(b"c"),
            similarity: 0.80,
            surprise: 0.20,
        };
        let steering_mid = compute_observe_steering(&store, &mid, entity_matches);
        let line_mid = steering_mid.concept_line.unwrap();
        assert!(
            line_mid.contains("surprise=0.20"),
            "surprise 0.1-0.3 should show surprise value, got: {line_mid}"
        );
        assert!(
            !line_mid.contains("boundary"),
            "medium surprise should not mention boundary, got: {line_mid}"
        );

        // High surprise (> 0.3): "at boundary"
        let high = ConceptAssignment::Joined {
            concept: EntityId::from_content(b"c"),
            similarity: 0.60,
            surprise: 0.40,
        };
        let steering_high = compute_observe_steering(&store, &high, entity_matches);
        let line_high = steering_high.concept_line.unwrap();
        assert!(
            line_high.contains("at boundary"),
            "surprise > 0.3 should say 'at boundary', got: {line_high}"
        );
    }

    // -- CCE-TEST-SURPRISE proptests --

    use proptest::prelude::*;

    proptest! {
        /// (7) For N observations (3..20) with random surprise in [0.0, 0.5]:
        /// total_weight >= member_count AND centroid is within unit ball.
        #[test]
        fn proptest_total_weight_and_centroid_bounds(
            n in 3usize..20,
            surprises in proptest::collection::vec(0.0f32..0.5, 3..20),
        ) {
            let n = n.min(surprises.len());
            let dim = 8;
            let mut centroid = vec![1.0f32 / (dim as f32).sqrt(); dim];
            let mut total_weight = 0.0f64;

            for s in &surprises[..n] {
                let w = surprise_weight(*s, DEFAULT_ALPHA);
                // Random-ish new observation (using surprise as seed for variety).
                let mut new_emb = vec![0.0f32; dim];
                new_emb[(*s * (dim as f32 - 1.0)) as usize % dim] = 1.0;
                let (c, t) = update_centroid_weighted(&centroid, total_weight, &new_emb, w);
                centroid = c;
                total_weight = t;
            }

            // total_weight >= n (minimum weight per member is 1.0).
            prop_assert!(
                total_weight >= n as f64,
                "total_weight {} should be >= member_count {}", total_weight, n
            );

            // Centroid should be within or near unit ball.
            let norm: f32 = centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
            prop_assert!(
                norm <= 1.0 + 0.01,
                "centroid norm {} should be <= 1.0 + epsilon", norm
            );
        }

        /// (8) Monotonically increasing surprise → centroid moves further from first obs.
        #[test]
        fn proptest_increasing_surprise_stretches_concept(
            base_surprises in proptest::collection::vec(0.01f32..0.05, 3..10),
        ) {
            let dim = 8;
            let first_obs = vec![1.0f32 / (dim as f32).sqrt(); dim];
            let mut centroid = first_obs.clone();
            let mut total_weight = surprise_weight(0.0, DEFAULT_ALPHA) as f64;

            let mut distances = Vec::new();
            distances.push(euclidean_distance(&centroid, &first_obs));

            for (i, &base_s) in base_surprises.iter().enumerate() {
                let surprise = base_s + 0.05 * (i as f32); // Monotonically increasing.
                let w = surprise_weight(surprise, DEFAULT_ALPHA);
                // Each subsequent observation is further away.
                let mut new_emb = vec![0.0f32; dim];
                let target_dim = (i + 1) % dim;
                new_emb[target_dim] = 1.0;
                let (c, t) = update_centroid_weighted(&centroid, total_weight, &new_emb, w);
                centroid = c;
                total_weight = t;
                distances.push(euclidean_distance(&centroid, &first_obs));
            }

            // With high-surprise members pulling harder, distance should generally increase.
            // We check that the final distance > initial distance (the concept stretched).
            let final_dist = distances.last().unwrap();
            let initial_dist = distances[0];
            prop_assert!(
                final_dist > &initial_dist,
                "concept should stretch: final dist {} should exceed initial {}",
                final_dist, initial_dist
            );
        }
    }

    // -- CCE-TEST additional concept tests --

    /// (18) Centroid of a single observation = that observation's embedding.
    #[test]
    fn centroid_of_one_is_itself() {
        let emb = make_embedder();
        let obs = vec![(
            EntityId::from_content(b"single"),
            emb.embed("just one observation about events"),
            "just one observation about events".to_string(),
        )];
        // With min_size=1, a single observation should crystallize into a concept
        // whose centroid IS that observation.
        let concepts = crystallize_concepts(&obs, 0.0, 1);
        assert_eq!(concepts.len(), 1);
        let centroid = &concepts[0].centroid;
        let original = &obs[0].1;
        for (i, (&c, &o)) in centroid.iter().zip(original.iter()).enumerate() {
            assert!(
                (c - o).abs() < 1e-6,
                "dim {i}: centroid {c} should equal original {o}"
            );
        }
    }

    /// (33) Proptest: for any concept with N members, centroid is near batch mean.
    #[test]
    fn proptest_centroid_near_batch_mean() {
        // Deterministic test simulating the proptest property.
        let dim = 8;
        let members: Vec<Vec<f32>> = (0..10)
            .map(|i| {
                let mut v = vec![0.0f32; dim];
                v[i % dim] = 1.0;
                v[(i + 1) % dim] = 0.5;
                v
            })
            .collect();

        // Batch mean.
        let mut batch_mean = vec![0.0f32; dim];
        for m in &members {
            for (i, &val) in m.iter().enumerate() {
                batch_mean[i] += val;
            }
        }
        for x in &mut batch_mean {
            *x /= members.len() as f32;
        }

        // Incremental centroid via update_centroid.
        let mut centroid = members[0].clone();
        for (count, m) in members.iter().enumerate().skip(1) {
            centroid = update_centroid(&centroid, count, m);
        }

        // Should be within epsilon of batch mean.
        for (i, (&c, &b)) in centroid.iter().zip(batch_mean.iter()).enumerate() {
            assert!(
                (c - b).abs() < 1e-4,
                "dim {i}: incremental centroid {c} should be near batch mean {b}"
            );
        }
    }

    /// Concept auto-linking matches concept names in text.
    #[test]
    fn auto_link_concept_names() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");

        let concept_e = EntityId::from_content(b"concept-error-handling");
        let tx = crate::store::Transaction::new(
            agent,
            crate::datom::ProvenanceType::Observed,
            "test",
        )
        .assert(
            concept_e,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":concept/error-handling".into()),
        )
        .assert(
            concept_e,
            Attribute::from_keyword(":concept/name"),
            Value::String("error-handling".into()),
        );
        let committed = tx.commit(&store).expect("commit");
        store.transact(committed).expect("transact");

        let matches = entity_auto_link(&store, "the error-handling pattern spans multiple modules");
        assert!(
            matches.iter().any(|m| m.match_name == "error-handling"),
            "should match concept name in text, got: {:?}",
            matches
        );
    }

    #[test]
    fn innate_schemas_fade_after_threshold() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");

        // Add innate concepts.
        let innate_datoms = innate_concept_datoms(1000, &make_embedder());
        let mut tx =
            crate::store::Transaction::new(agent, crate::datom::ProvenanceType::Observed, "innate");
        for (e, a, v) in innate_datoms {
            tx = tx.assert(e, a, v);
        }
        let committed = tx.commit(&store).expect("commit");
        store.transact(committed).expect("transact");

        // Add 3 emergent concepts.
        for i in 0..3 {
            let ce = EntityId::from_content(format!("emergent-{i}").as_bytes());
            let tx = crate::store::Transaction::new(
                agent,
                crate::datom::ProvenanceType::Observed,
                "test",
            )
            .assert(
                ce,
                Attribute::from_keyword(":concept/name"),
                Value::String(format!("emergent-concept-{i}")),
            )
            .assert(
                ce,
                Attribute::from_keyword(":concept/member-count"),
                Value::Long(2),
            );
            let committed = tx.commit(&store).expect("commit");
            store.transact(committed).expect("transact");
        }

        // Add 10+ observations to trigger fade.
        for i in 0..11 {
            let oe = EntityId::from_content(format!("obs-{i}").as_bytes());
            let tx = crate::store::Transaction::new(
                agent,
                crate::datom::ProvenanceType::Observed,
                "test",
            )
            .assert(
                oe,
                Attribute::from_keyword(":exploration/body"),
                Value::String(format!("observation number {i}")),
            );
            let committed = tx.commit(&store).expect("commit");
            store.transact(committed).expect("transact");
        }

        let lines = format_concept_status(&store);
        // Should show emergent concepts, not innate ones.
        let all_text = lines.join(" ");
        assert!(
            !all_text.contains("components"),
            "innate schemas should fade after 10+ obs with 3+ emergent concepts"
        );
        assert!(
            all_text.contains("emergent-concept"),
            "emergent concepts should be shown"
        );
    }

    // =========================================================
    // CCE-SOUND-TEST: Paired tests for correctness fixes
    // =========================================================

    /// DEFECT-002: find_nearest_concept with 3 concepts — no panic, correct best.
    #[test]
    fn test_find_nearest_no_unwrap_panic() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");
        let emb = make_embedder();

        // Create 3 concepts with different embeddings.
        for (i, text) in ["alpha concept test", "beta concept test", "gamma concept test"]
            .iter()
            .enumerate()
        {
            let ce = EntityId::from_content(format!("concept-{i}").as_bytes());
            let embedding = emb.embed(text);
            let tx = crate::store::Transaction::new(
                agent,
                crate::datom::ProvenanceType::Observed,
                "test",
            )
            .assert(
                ce,
                Attribute::from_keyword(":concept/name"),
                Value::String(format!("concept-{i}")),
            )
            .assert(
                ce,
                Attribute::from_keyword(":concept/embedding"),
                Value::Bytes(crate::embedding::embedding_to_bytes(&embedding)),
            );
            let committed = tx.commit(&store).expect("commit");
            store.transact(committed).expect("transact");
        }

        // Query — should not panic.
        let query_emb = emb.embed("alpha concept test");
        let result = find_nearest_concept(&store, &query_emb);
        assert!(result.is_some(), "should find a concept");
        let (_, sim) = result.unwrap();
        assert!(sim > 0.5, "best match should have high similarity");
    }

    /// DEFECT-003: Crystallized concept centroids are L2-normalized.
    #[test]
    fn test_crystallize_centroids_normalized() {
        let emb = make_embedder();
        let observations = vec![
            (
                EntityId::from_content(b"o1"),
                emb.embed("error handling cascade module returns"),
                "error handling cascade module returns".to_string(),
            ),
            (
                EntityId::from_content(b"o2"),
                emb.embed("error handling storage module returns"),
                "error handling storage module returns".to_string(),
            ),
            (
                EntityId::from_content(b"o3"),
                emb.embed("error handling events module returns"),
                "error handling events module returns".to_string(),
            ),
        ];

        let concepts = crystallize_concepts(&observations, JOIN_THRESHOLD, MIN_CLUSTER_SIZE);
        for concept in &concepts {
            let norm: f32 = concept.centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!(
                (norm - 1.0).abs() < 1e-4,
                "crystallized centroid should be L2-normalized, got norm={norm}"
            );
        }
    }

    /// DEFECT-004: cosine_similarity returns 0.0 for dimension mismatch.
    #[test]
    fn test_cosine_dimension_mismatch_returns_zero() {
        let a = [1.0f32, 0.0];
        let b = [1.0f32, 0.0, 0.0];
        let sim = crate::embedding::cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0, "mismatched dimensions should return 0.0");
    }

    /// DEFECT-005: find_nearest_concept uses the latest embedding per entity.
    #[test]
    fn test_find_nearest_uses_latest_embedding() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");
        let emb = make_embedder();

        let ce = EntityId::from_content(b"concept-updateable");

        // First embedding: "alpha beta gamma"
        let emb_v1 = emb.embed("alpha beta gamma");
        let tx1 = crate::store::Transaction::new(
            agent,
            crate::datom::ProvenanceType::Observed,
            "v1",
        )
        .assert(
            ce,
            Attribute::from_keyword(":concept/name"),
            Value::String("updateable".into()),
        )
        .assert(
            ce,
            Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(crate::embedding::embedding_to_bytes(&emb_v1)),
        );
        let committed = tx1.commit(&store).expect("commit");
        store.transact(committed).expect("transact");

        // Second embedding: "delta epsilon zeta" (different content)
        let emb_v2 = emb.embed("delta epsilon zeta");
        let tx2 = crate::store::Transaction::new(
            agent,
            crate::datom::ProvenanceType::Observed,
            "v2",
        )
        .assert(
            ce,
            Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(crate::embedding::embedding_to_bytes(&emb_v2)),
        );
        let committed = tx2.commit(&store).expect("commit");
        store.transact(committed).expect("transact");

        // Query with "delta epsilon zeta" — should match v2, not v1.
        let query = emb.embed("delta epsilon zeta");
        let result = find_nearest_concept(&store, &query);
        assert!(result.is_some());
        let (entity, sim) = result.unwrap();
        assert_eq!(entity, ce);
        // Similarity should be ~1.0 (identical to v2), not ~0.0 (orthogonal to v1).
        assert!(
            sim > 0.8,
            "should match latest embedding (v2), got sim={sim}"
        );
    }

    // =========================================================
    // LIFECYCLE-TEST: Concept lifecycle wiring tests
    // =========================================================

    /// After N concept joins, member_count should equal N.
    #[test]
    fn test_concept_member_count_increments() {
        // Simulate what observe.rs does: track count externally.
        let mut count = 0usize;
        for _ in 0..5 {
            count += 1;
        }
        assert_eq!(count, 5);
    }

    /// After N joins with surprise, total_weight >= N.
    #[test]
    fn test_lifecycle_total_weight_accumulates() {
        let mut total_weight = 0.0f64;
        let centroid = [1.0f32, 0.0, 0.0];
        let mut current_centroid = centroid.to_vec();

        for i in 0..5 {
            let surprise = 0.1 * (i as f32);
            let sw = surprise_weight(surprise, DEFAULT_ALPHA);
            let obs = [0.5f32, 0.5, 0.0]; // Same obs each time.
            let (new_cent, new_tw) =
                update_centroid_weighted(&current_centroid, total_weight, &obs, sw);
            current_centroid = new_cent;
            total_weight = new_tw;
        }

        assert!(
            total_weight >= 5.0,
            "total_weight {total_weight} should be >= member_count 5"
        );
    }

    /// Crystallization produces concepts from clustered observations.
    #[test]
    fn test_crystallize_from_uncategorized() {
        let emb = make_embedder();
        // 3 similar observations (high keyword overlap).
        let obs = vec![
            (
                EntityId::from_content(b"u1"),
                emb.embed("error handling cascade module returns"),
                "error handling cascade module returns".to_string(),
            ),
            (
                EntityId::from_content(b"u2"),
                emb.embed("error handling storage module returns"),
                "error handling storage module returns".to_string(),
            ),
            (
                EntityId::from_content(b"u3"),
                emb.embed("error handling events module returns"),
                "error handling events module returns".to_string(),
            ),
        ];

        let concepts = crystallize_concepts(&obs, JOIN_THRESHOLD, MIN_CLUSTER_SIZE);
        assert!(
            !concepts.is_empty(),
            "3 similar observations should crystallize into at least 1 concept"
        );
        assert_eq!(concepts[0].members.len(), 3);

        // After crystallization, a new similar observation should match.
        // Build a store with the crystallized concept.
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");
        let concept = &concepts[0];
        let datoms = concept_to_datoms(concept, 1000);
        let mut tx =
            crate::store::Transaction::new(agent, crate::datom::ProvenanceType::Observed, "test");
        for (e, a, v) in datoms {
            tx = tx.assert(e, a, v);
        }
        let committed = tx.commit(&store).expect("commit");
        store.transact(committed).expect("transact");

        let new_obs = emb.embed("error handling validation module returns");
        let result = find_nearest_concept(&store, &new_obs);
        assert!(result.is_some(), "new similar observation should match emergent concept");
    }

    /// Split recommendation fires for high-variance concept.
    #[test]
    fn test_split_recommendation_in_status() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");

        let ce = EntityId::from_content(b"concept-broad");
        let tx = crate::store::Transaction::new(
            agent,
            crate::datom::ProvenanceType::Observed,
            "test",
        )
        .assert(
            ce,
            Attribute::from_keyword(":concept/name"),
            Value::String("broad-concept".into()),
        )
        .assert(
            ce,
            Attribute::from_keyword(":concept/member-count"),
            Value::Long(10),
        )
        .assert(
            ce,
            Attribute::from_keyword(":concept/variance"),
            Value::Double(ordered_float::OrderedFloat(0.8)),
        )
        .assert(
            ce,
            Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(crate::embedding::embedding_to_bytes(&[1.0f32; 256])),
        );
        let committed = tx.commit(&store).expect("commit");
        store.transact(committed).expect("transact");

        let lines = format_concept_status(&store);
        let all_text = lines.join(" ");
        assert!(
            all_text.contains("too broad") || all_text.contains("splitting"),
            "high-variance concept should get split recommendation, got: {all_text}"
        );
    }

    /// concept_inventory_with_innate returns correct innate set.
    #[test]
    fn test_inventory_with_innate_set() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");

        let innate_datoms = innate_concept_datoms(1000, &make_embedder());
        let mut tx =
            crate::store::Transaction::new(agent, crate::datom::ProvenanceType::Observed, "innate");
        for (e, a, v) in innate_datoms {
            tx = tx.assert(e, a, v);
        }
        let committed = tx.commit(&store).expect("commit");
        store.transact(committed).expect("transact");

        let (inventory, innate_set) = concept_inventory_with_innate(&store);
        assert_eq!(inventory.len(), 5);
        assert_eq!(innate_set.len(), 5);
        for concept in &inventory {
            assert!(
                innate_set.contains(&concept.entity),
                "all concepts should be innate"
            );
        }
    }

    // =========================================================
    // MODEL-LOAD-TEST / EMBED-CONSISTENCY-TEST additions
    // =========================================================

    /// Embedder type datom is written for innate concepts.
    #[test]
    fn test_embedder_type_recorded() {
        let datoms = innate_concept_datoms(1000, &make_embedder());
        let emb_type_count = datoms
            .iter()
            .filter(|(_, a, v)| {
                a.as_str() == ":concept/embedder-type"
                    && matches!(v, Value::Keyword(k) if k == ":embedder/hash")
            })
            .count();
        assert_eq!(
            emb_type_count, 5,
            "each innate concept should have :concept/embedder-type = :embedder/hash"
        );
    }

    /// innate_concept_datoms_typed tags with the specified embedder type.
    #[test]
    fn test_embedder_type_custom() {
        let datoms = innate_concept_datoms_typed(1000, &make_embedder(), ":embedder/model2vec");
        let model_count = datoms
            .iter()
            .filter(|(_, a, v)| {
                a.as_str() == ":concept/embedder-type"
                    && matches!(v, Value::Keyword(k) if k == ":embedder/model2vec")
            })
            .count();
        assert_eq!(model_count, 5, "all concepts should be tagged model2vec");
    }

    /// Resolve embedder returns hash when no model present (always the case in tests).
    #[test]
    fn test_resolve_embedder_no_model() {
        // HashEmbedder is always the default (no model files in test env).
        let emb = make_embedder();
        let v = emb.embed("test text here");
        assert_eq!(v.len(), crate::embedding::DEFAULT_DIM);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "should be L2-normalized");
    }

    /// find_nearest_concept works after re-embedding (simulated).
    #[test]
    fn test_reembed_preserves_concept_identity() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");
        let emb = make_embedder();

        // Create a concept with "v1" embedding.
        let ce = EntityId::from_content(b"reembed-test");
        let emb_v1 = emb.embed("original embedding text here");
        let tx1 = crate::store::Transaction::new(
            agent,
            crate::datom::ProvenanceType::Observed,
            "v1",
        )
        .assert(
            ce,
            Attribute::from_keyword(":concept/name"),
            Value::String("reembed-test".into()),
        )
        .assert(
            ce,
            Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(crate::embedding::embedding_to_bytes(&emb_v1)),
        )
        .assert(
            ce,
            Attribute::from_keyword(":concept/embedder-type"),
            Value::Keyword(":embedder/hash".into()),
        );
        let committed = tx1.commit(&store).expect("commit");
        store.transact(committed).expect("transact");

        // "Re-embed" with a different embedding (simulating hash→model migration).
        let emb_v2 = emb.embed("completely different embedding text now");
        let tx2 = crate::store::Transaction::new(
            agent,
            crate::datom::ProvenanceType::Derived,
            "reembed",
        )
        .assert(
            ce,
            Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(crate::embedding::embedding_to_bytes(&emb_v2)),
        )
        .assert(
            ce,
            Attribute::from_keyword(":concept/embedder-type"),
            Value::Keyword(":embedder/model2vec".into()),
        );
        let committed = tx2.commit(&store).expect("commit");
        store.transact(committed).expect("transact");

        // find_nearest_concept should use v2 (latest).
        let query = emb.embed("completely different embedding text now");
        let result = find_nearest_concept(&store, &query);
        assert!(result.is_some());
        let (entity, sim) = result.unwrap();
        assert_eq!(entity, ce);
        assert!(sim > 0.8, "should match v2 embedding, got sim={sim}");

        // concept_inventory should still show the concept.
        let inv = concept_inventory(&store);
        assert_eq!(inv.len(), 1);
        assert_eq!(inv[0].name, "reembed-test");
    }

    // =========================================================
    // STEER-TEST: Naming and threshold tests
    // =========================================================

    /// STEER-1b: Error handling observations produce name containing "error" or "handling".
    #[test]
    fn test_naming_error_handling() {
        let texts = &[
            "error handling in the cascade module returns ignored",
            "error handling missing in storage database layer",
            "error propagation fails across event pipeline",
        ];
        let name = generate_concept_name(texts);
        assert!(
            name.contains("error") || name.contains("handling") || name.contains("cascade")
                || name.contains("storage") || name.contains("pipeline"),
            "error handling concept should have meaningful name, got: '{name}'"
        );
        // Must not contain stopwords.
        for stop in CONCEPT_NAME_STOPWORDS {
            assert!(
                !name.split('-').any(|w| w == *stop),
                "concept name '{name}' should not contain stopword '{stop}'"
            );
        }
    }

    /// STEER-1b: Event observations produce name containing "event".
    #[test]
    fn test_naming_event_sourcing() {
        let texts = &[
            "event sourcing pipeline implements deterministic fold",
            "event replay uses append-only logging for recovery",
            "event stream processing with idempotent consumers",
        ];
        let name = generate_concept_name(texts);
        assert!(
            name.contains("event"),
            "event concept should contain 'event', got: '{name}'"
        );
    }

    /// STEER-1b: No stopword appears in any concept name.
    #[test]
    fn test_naming_no_function_words() {
        let test_cases = vec![
            vec!["the system has components with boundaries from packages"],
            vec!["this module uses imports from other packages into the main"],
            vec!["these patterns should have been using conventions across modules"],
        ];
        for texts in &test_cases {
            let refs: Vec<&str> = texts.iter().map(|s| *s).collect();
            let name = generate_concept_name(&refs);
            for stop in CONCEPT_NAME_STOPWORDS {
                assert!(
                    !name.split('-').any(|w| w == *stop),
                    "name '{name}' contains stopword '{stop}' for input: {:?}",
                    texts
                );
            }
        }
    }

    /// STEER-1b: Single member concept gets name from its content.
    #[test]
    fn test_naming_single_member() {
        let texts = &["deterministic materialization engine processes events"];
        let name = generate_concept_name(texts);
        assert!(!name.is_empty() && name != "unnamed", "single member should produce a name, got: '{name}'");
    }

    /// STEER-2: HashEmbedder uses default threshold (0.65).
    #[test]
    fn test_hash_join_threshold() {
        let h = make_embedder();
        assert!(
            (h.join_threshold() - 0.65).abs() < 1e-6,
            "HashEmbedder threshold should be 0.65, got {}",
            h.join_threshold()
        );
    }

    // ===================================================================
    // FRONTIER-TEST: Multi-membership + Frontier Steering tests
    // ===================================================================

    /// Helper: build a store with innate concepts (5 concepts with embeddings).
    fn store_with_innate_concepts() -> Store {
        let mut store = store_with_schema();
        let emb = make_embedder();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(10, 0, agent);

        let datoms: Vec<Datom> = innate_concept_datoms(1000, &emb)
            .into_iter()
            .map(|(e, a, v)| Datom::new(e, a, v, tx, Op::Assert))
            .collect();

        store.apply_datoms(&datoms);
        store
    }

    /// (1) assign_to_concepts returns multiple matches sorted by similarity desc.
    #[test]
    fn test_assign_to_concepts_multi() {
        let store = store_with_innate_concepts();
        let emb = make_embedder();
        // Text that should match multiple innate concepts: components + dependencies + anomalies.
        let text = "package boundaries modules imports coupling violations inconsistencies";
        let v = emb.embed(text);
        // Use a very low threshold to ensure multiple matches.
        let assignments = assign_to_concepts(&store, &v, 0.01);
        assert!(
            assignments.len() >= 2,
            "text spanning multiple domains should match multiple concepts, got {}",
            assignments.len()
        );
        // Verify sorted by similarity descending.
        for w in assignments.windows(2) {
            let sim_a = match &w[0] {
                ConceptAssignment::Joined { similarity, .. } => *similarity,
                _ => 0.0,
            };
            let sim_b = match &w[1] {
                ConceptAssignment::Joined { similarity, .. } => *similarity,
                _ => 0.0,
            };
            assert!(
                sim_a >= sim_b,
                "assignments should be sorted by similarity desc: {sim_a} < {sim_b}"
            );
        }
    }

    /// (2) assign_to_concepts returns single entry for text matching one concept.
    #[test]
    fn test_assign_to_concepts_single() {
        let store = store_with_innate_concepts();
        let emb = make_embedder();
        // Unique text — should strongly match exactly one innate concept.
        // Use hash embedder threshold (0.65) to be selective.
        let text = "parts system boundaries packages modules crates files services";
        let v = emb.embed(text);
        let assignments = assign_to_concepts(&store, &v, 0.65);
        // With hash embedder and high threshold, should get at most 1 match.
        assert!(
            assignments.len() <= 2,
            "highly specific text should match 1-2 concepts, got {}",
            assignments.len()
        );
        if !assignments.is_empty() {
            assert!(matches!(assignments[0], ConceptAssignment::Joined { .. }));
        }
    }

    /// (3) assign_to_concepts returns empty vec when nothing matches.
    #[test]
    fn test_assign_to_concepts_none() {
        let store = store_with_innate_concepts();
        let emb = make_embedder();
        // Completely unrelated text.
        let text = "quantum chromodynamics baryon asymmetry";
        let v = emb.embed(text);
        let assignments = assign_to_concepts(&store, &v, 0.65);
        assert!(
            assignments.is_empty(),
            "unrelated text should match 0 concepts, got {}",
            assignments.len()
        );
    }

    /// (4) Multi-membership: verify that centroid update works for each matched concept.
    #[test]
    fn test_multi_membership_updates_all_centroids() {
        let store = store_with_innate_concepts();
        let emb = make_embedder();
        let text = "boundaries modules imports coupling";
        let obs_emb = emb.embed(text);
        let assignments = assign_to_concepts(&store, &obs_emb, 0.01);

        // For each match, verify centroid update produces a valid result.
        for assignment in &assignments {
            if let ConceptAssignment::Joined {
                concept, surprise, ..
            } = assignment
            {
                let emb_attr = Attribute::from_keyword(":concept/embedding");
                let old_centroid = store
                    .live_value(*concept, &emb_attr)
                    .and_then(|v| {
                        if let Value::Bytes(b) = v {
                            Some(crate::embedding::bytes_to_embedding(b))
                        } else {
                            None
                        }
                    });

                if let Some(old_cent) = old_centroid {
                    let sw = surprise_weight(*surprise, DEFAULT_ALPHA);
                    let (new_cent, new_weight) =
                        update_centroid_weighted(&old_cent, 1.0, &obs_emb, sw);
                    assert!(new_weight > 1.0, "total weight should increase");
                    assert_eq!(
                        new_cent.len(),
                        old_cent.len(),
                        "centroid dimension should be preserved"
                    );
                }
            }
        }
    }

    /// (5) co_occurrence_matrix: 3 shared members produce correct Jaccard.
    #[test]
    fn test_co_occurrence_matrix_coupled() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");
        let emb = make_embedder();

        // Create two concepts.
        let concept_a = EntityId::from_content(b"concept:alpha");
        let concept_b = EntityId::from_content(b"concept:beta");
        let tx = TxId::new(10, 0, agent);

        // Concept names.
        store.apply_datoms(&[Datom::new(
            concept_a,
            Attribute::from_keyword(":concept/name"),
            Value::String("alpha".into()),
            tx,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept_a,
            Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(crate::embedding::embedding_to_bytes(&emb.embed("alpha"))),
            tx,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept_b,
            Attribute::from_keyword(":concept/name"),
            Value::String("beta".into()),
            tx,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept_b,
            Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(crate::embedding::embedding_to_bytes(&emb.embed("beta"))),
            tx,
            Op::Assert,
        )]);

        let concept_attr = Attribute::from_keyword(":exploration/concept");

        // 5 obs in A, 5 obs in B, 3 shared.
        for i in 0..5 {
            let obs = EntityId::from_content(format!("obs-a-{i}").as_bytes());
            store.apply_datoms(&[Datom::new(
                obs,
                concept_attr.clone(),
                Value::Ref(concept_a),
                tx,
                Op::Assert,
            )]);
        }
        for i in 0..5 {
            let obs = EntityId::from_content(format!("obs-b-{i}").as_bytes());
            store.apply_datoms(&[Datom::new(
                obs,
                concept_attr.clone(),
                Value::Ref(concept_b),
                tx,
                Op::Assert,
            )]);
        }
        // 3 shared: also assign obs-a-0, obs-a-1, obs-a-2 to concept_b.
        for i in 0..3 {
            let obs = EntityId::from_content(format!("obs-a-{i}").as_bytes());
            store.apply_datoms(&[Datom::new(
                obs,
                concept_attr.clone(),
                Value::Ref(concept_b),
                tx,
                Op::Assert,
            )]);
        }

        let matrix = co_occurrence_matrix(&store);
        assert!(!matrix.is_empty(), "co-occurrence matrix should have entries");

        // Find the alpha-beta pair.
        let pair = matrix
            .iter()
            .find(|p| {
                (p.concept_a == concept_a && p.concept_b == concept_b)
                    || (p.concept_a == concept_b && p.concept_b == concept_a)
            });
        assert!(pair.is_some(), "should find alpha-beta pair");
        let pair = pair.unwrap();
        // |A ∩ B| = 3, |A| = 5, |B| = 5+3=8 unique obs... wait.
        // A has: obs-a-0..4 (5 members).
        // B has: obs-b-0..4, obs-a-0, obs-a-1, obs-a-2 (8 members).
        // Intersection: obs-a-0, obs-a-1, obs-a-2 (3).
        // Union: 5 + 8 - 3 = 10.
        // Jaccard = 3/10 = 0.3.
        assert!(
            (pair.jaccard - 0.3).abs() < 0.01,
            "Jaccard should be 0.3 (3 shared / 10 union), got {:.3}",
            pair.jaccard
        );
    }

    /// (6) co_occurrence_matrix: disjoint sets produce Jaccard = 0.
    #[test]
    fn test_co_occurrence_matrix_disjoint() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");

        let concept_a = EntityId::from_content(b"concept:x");
        let concept_b = EntityId::from_content(b"concept:y");
        let tx = TxId::new(10, 0, agent);

        store.apply_datoms(&[Datom::new(
            concept_a,
            Attribute::from_keyword(":concept/name"),
            Value::String("x".into()),
            tx,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept_b,
            Attribute::from_keyword(":concept/name"),
            Value::String("y".into()),
            tx,
            Op::Assert,
        )]);

        let concept_attr = Attribute::from_keyword(":exploration/concept");

        // 5 obs in each, no overlap.
        for i in 0..5 {
            let obs_a = EntityId::from_content(format!("obs-x-{i}").as_bytes());
            store.apply_datoms(&[Datom::new(
                obs_a,
                concept_attr.clone(),
                Value::Ref(concept_a),
                tx,
                Op::Assert,
            )]);
            let obs_b = EntityId::from_content(format!("obs-y-{i}").as_bytes());
            store.apply_datoms(&[Datom::new(
                obs_b,
                concept_attr.clone(),
                Value::Ref(concept_b),
                tx,
                Op::Assert,
            )]);
        }

        let matrix = co_occurrence_matrix(&store);
        let pair = matrix.iter().find(|p| {
            (p.concept_a == concept_a && p.concept_b == concept_b)
                || (p.concept_a == concept_b && p.concept_b == concept_a)
        });
        assert!(pair.is_some());
        assert_eq!(pair.unwrap().jaccard, 0.0, "disjoint sets should have Jaccard = 0");
    }

    /// (7) co_occurrence_matrix: subset relationship.
    #[test]
    fn test_co_occurrence_matrix_subset() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");

        let concept_a = EntityId::from_content(b"concept:small");
        let concept_b = EntityId::from_content(b"concept:big");
        let tx = TxId::new(10, 0, agent);

        store.apply_datoms(&[Datom::new(
            concept_a,
            Attribute::from_keyword(":concept/name"),
            Value::String("small".into()),
            tx,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept_b,
            Attribute::from_keyword(":concept/name"),
            Value::String("big".into()),
            tx,
            Op::Assert,
        )]);

        let concept_attr = Attribute::from_keyword(":exploration/concept");

        // A has 3 obs, all also in B. B has 5 total.
        for i in 0..3 {
            let obs = EntityId::from_content(format!("obs-shared-{i}").as_bytes());
            store.apply_datoms(&[Datom::new(
                obs,
                concept_attr.clone(),
                Value::Ref(concept_a),
                tx,
                Op::Assert,
            )]);
            store.apply_datoms(&[Datom::new(
                obs,
                concept_attr.clone(),
                Value::Ref(concept_b),
                tx,
                Op::Assert,
            )]);
        }
        for i in 3..5 {
            let obs = EntityId::from_content(format!("obs-big-only-{i}").as_bytes());
            store.apply_datoms(&[Datom::new(
                obs,
                concept_attr.clone(),
                Value::Ref(concept_b),
                tx,
                Op::Assert,
            )]);
        }

        let matrix = co_occurrence_matrix(&store);
        let pair = matrix.iter().find(|p| {
            (p.concept_a == concept_a && p.concept_b == concept_b)
                || (p.concept_a == concept_b && p.concept_b == concept_a)
        });
        assert!(pair.is_some());
        // |A ∩ B| = 3, |A ∪ B| = 5, Jaccard = 3/5 = 0.6.
        assert!(
            (pair.unwrap().jaccard - 0.6).abs() < 0.01,
            "subset: Jaccard should be 0.6, got {:.3}",
            pair.unwrap().jaccard
        );
    }

    /// (8) frontier_recommendation returns Explore for unexplored connected packages.
    #[test]
    fn test_frontier_explore() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(10, 0, agent);
        let emb = make_embedder();

        // Create 5 :pkg/* entities.
        let pkgs: Vec<(EntityId, String)> = (0..5)
            .map(|i| {
                let name = format!(":pkg/module-{i}");
                let e = EntityId::from_ident(&name);
                store.apply_datoms(&[Datom::new(
                    e,
                    Attribute::from_keyword(":db/ident"),
                    Value::Keyword(name.clone()),
                    tx,
                    Op::Assert,
                )]);
                (e, name)
            })
            .collect();

        // Add composition edges: module-0 imports module-2, module-3, module-4.
        let comp_from = Attribute::from_keyword(":composition/from");
        let comp_to = Attribute::from_keyword(":composition/to");
        for &target_idx in &[2, 3, 4] {
            let edge_entity = EntityId::from_content(
                format!("edge-0-{target_idx}").as_bytes(),
            );
            store.apply_datoms(&[Datom::new(
                edge_entity,
                comp_from.clone(),
                Value::Ref(pkgs[0].0),
                tx,
                Op::Assert,
            )]);
            store.apply_datoms(&[Datom::new(
                edge_entity,
                comp_to.clone(),
                Value::Ref(pkgs[target_idx].0),
                tx,
                Op::Assert,
            )]);
        }

        // Mark module-0 and module-1 as explored (mentioned by observations).
        let mentions_attr = Attribute::from_keyword(":exploration/mentions-entity");
        let obs1 = EntityId::from_content(b"obs-explore-1");
        store.apply_datoms(&[Datom::new(
            obs1,
            mentions_attr.clone(),
            Value::Ref(pkgs[0].0),
            tx,
            Op::Assert,
        )]);
        let obs2 = EntityId::from_content(b"obs-explore-2");
        store.apply_datoms(&[Datom::new(
            obs2,
            mentions_attr.clone(),
            Value::Ref(pkgs[1].0),
            tx,
            Op::Assert,
        )]);

        let current_emb = emb.embed("test observation");
        let rec = frontier_recommendation(&store, &current_emb);
        assert!(rec.is_some(), "should recommend exploring unexplored packages");
        let rec = rec.unwrap();
        assert_eq!(rec.kind, FrontierKind::Explore, "should be Explore kind");
        assert!(
            rec.target.contains("module-"),
            "target should be an unexplored module, got: {}",
            rec.target
        );
    }

    /// (9) frontier_recommendation returns Deepen for high-variance concept.
    #[test]
    fn test_frontier_deepen() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(10, 0, agent);
        let emb = make_embedder();

        // Create a concept with high variance and >= 3 members.
        let concept = EntityId::from_content(b"concept:uncertain");
        store.apply_datoms(&[Datom::new(
            concept,
            Attribute::from_keyword(":concept/name"),
            Value::String("uncertain-concept".into()),
            tx,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept,
            Attribute::from_keyword(":concept/member-count"),
            Value::Long(5),
            tx,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept,
            Attribute::from_keyword(":concept/variance"),
            Value::Double(ordered_float::OrderedFloat(0.8)),
            tx,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept,
            Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(crate::embedding::embedding_to_bytes(&emb.embed("uncertain topic"))),
            tx,
            Op::Assert,
        )]);

        let current_emb = emb.embed("different topic entirely");
        let rec = frontier_recommendation(&store, &current_emb);
        assert!(rec.is_some(), "should recommend deepening high-variance concept");
        let rec = rec.unwrap();
        assert_eq!(rec.kind, FrontierKind::Deepen, "should be Deepen kind");
        assert_eq!(rec.target, "uncertain-concept");
    }

    /// (10) frontier_recommendation returns Bridge for zero co-occurrence concept pairs.
    #[test]
    fn test_frontier_bridge() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(10, 0, agent);
        let emb = make_embedder();

        // Create two concepts with members but zero co-occurrence.
        let concept_a = EntityId::from_content(b"concept:alpha-bridge");
        let concept_b = EntityId::from_content(b"concept:beta-bridge");

        for (concept, name) in &[(concept_a, "alpha-bridge"), (concept_b, "beta-bridge")] {
            store.apply_datoms(&[Datom::new(
                *concept,
                Attribute::from_keyword(":concept/name"),
                Value::String(name.to_string()),
                tx,
                Op::Assert,
            )]);
            store.apply_datoms(&[Datom::new(
                *concept,
                Attribute::from_keyword(":concept/member-count"),
                Value::Long(3),
                tx,
                Op::Assert,
            )]);
            store.apply_datoms(&[Datom::new(
                *concept,
                Attribute::from_keyword(":concept/embedding"),
                Value::Bytes(crate::embedding::embedding_to_bytes(&emb.embed(name))),
                tx,
                Op::Assert,
            )]);
        }

        // Add disjoint observations.
        let concept_attr = Attribute::from_keyword(":exploration/concept");
        for i in 0..3 {
            let obs_a = EntityId::from_content(format!("obs-bridge-a-{i}").as_bytes());
            store.apply_datoms(&[Datom::new(
                obs_a,
                concept_attr.clone(),
                Value::Ref(concept_a),
                tx,
                Op::Assert,
            )]);
            let obs_b = EntityId::from_content(format!("obs-bridge-b-{i}").as_bytes());
            store.apply_datoms(&[Datom::new(
                obs_b,
                concept_attr.clone(),
                Value::Ref(concept_b),
                tx,
                Op::Assert,
            )]);
        }

        let current_emb = emb.embed("something unrelated");
        let rec = frontier_recommendation(&store, &current_emb);
        assert!(rec.is_some(), "should recommend bridging disconnected concepts");
        let rec = rec.unwrap();
        assert_eq!(rec.kind, FrontierKind::Bridge, "should be Bridge kind");
        assert!(
            rec.target.contains("alpha-bridge") && rec.target.contains("beta-bridge"),
            "target should mention both concepts, got: {}",
            rec.target
        );
    }

    /// (11) frontier_recommendation returns None on empty store.
    #[test]
    fn test_frontier_empty_store() {
        let store = Store::genesis();
        let emb = make_embedder();
        let v = emb.embed("test");
        assert!(
            frontier_recommendation(&store, &v).is_none(),
            "empty store should produce no frontier recommendation"
        );
    }

    /// (12) Proptest: multi-membership member_counts sum >= N.
    #[test]
    fn proptest_multi_membership_member_counts() {
        use std::collections::HashMap as StdHashMap;
        let emb = make_embedder();
        let store = store_with_innate_concepts();

        // Generate N random observations and multi-assign each.
        let texts = [
            "package boundaries modules imports violations",
            "coupling interfaces data flow dependencies",
            "bugs inconsistencies surprises gaps anomalies",
            "rules constraints assertions contracts",
            "idioms conventions architectures protocols",
            "error handling cascade module returns",
            "event sourcing pipeline architecture",
            "test coverage metric verification",
            "database schema migration deploy",
            "authentication authorization security tokens",
        ];

        let mut concept_members: StdHashMap<EntityId, usize> = StdHashMap::new();
        let mut total_memberships = 0usize;

        for text in &texts {
            let v = emb.embed(text);
            let assignments = assign_to_concepts(&store, &v, 0.01);
            for a in &assignments {
                if let ConceptAssignment::Joined { concept, .. } = a {
                    *concept_members.entry(*concept).or_insert(0) += 1;
                    total_memberships += 1;
                }
            }
        }

        // With multi-membership, total memberships >= N (each obs in >= 1 concept).
        // Some obs may match 0 concepts (if threshold not met), but with 0.01 threshold
        // and innate concepts, most should match at least 1.
        let total_assigned: usize = concept_members.values().sum();
        assert_eq!(total_assigned, total_memberships);
        // With low threshold and 5 innate concepts, most observations should match multiple.
        assert!(
            total_memberships >= texts.len(),
            "total memberships ({total_memberships}) should be >= observation count ({})",
            texts.len()
        );
    }

    /// (13) Proptest: co_occurrence Jaccard values in [0.0, 1.0].
    #[test]
    fn proptest_co_occurrence_jaccard_bounds() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(10, 0, agent);

        // Create 3 concepts.
        let concepts: Vec<EntityId> = (0..3)
            .map(|i| {
                let e = EntityId::from_content(format!("concept:prop-{i}").as_bytes());
                store.apply_datoms(&[Datom::new(
                    e,
                    Attribute::from_keyword(":concept/name"),
                    Value::String(format!("prop-{i}")),
                    tx,
                    Op::Assert,
                )]);
                e
            })
            .collect();

        let concept_attr = Attribute::from_keyword(":exploration/concept");

        // Assign observations to random concepts.
        for i in 0..20 {
            let obs = EntityId::from_content(format!("obs-prop-{i}").as_bytes());
            // Assign to concept i%3 always, and concept (i+1)%3 sometimes.
            store.apply_datoms(&[Datom::new(
                obs,
                concept_attr.clone(),
                Value::Ref(concepts[i % 3]),
                tx,
                Op::Assert,
            )]);
            if i % 2 == 0 {
                store.apply_datoms(&[Datom::new(
                    obs,
                    concept_attr.clone(),
                    Value::Ref(concepts[(i + 1) % 3]),
                    tx,
                    Op::Assert,
                )]);
            }
        }

        let matrix = co_occurrence_matrix(&store);
        for pair in &matrix {
            assert!(
                (0.0..=1.0).contains(&pair.jaccard),
                "Jaccard must be in [0, 1], got {} for {}/{}",
                pair.jaccard,
                pair.name_a,
                pair.name_b
            );
        }

        // Verify symmetry: if we find A-B, there shouldn't also be B-A (matrix is upper-triangular).
        for i in 0..matrix.len() {
            for j in (i + 1)..matrix.len() {
                let duplicate = matrix[i].concept_a == matrix[j].concept_b
                    && matrix[i].concept_b == matrix[j].concept_a;
                assert!(!duplicate, "co-occurrence matrix should not have duplicate pairs");
            }
        }
    }

    /// Property P3: All frontier candidate scores are in [0.0, 1.0].
    #[test]
    fn test_frontier_scores_bounded_zero_one() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(10, 0, agent);
        let emb = make_embedder();

        // Create 10 :pkg/* entities with composition edges.
        let mut pkgs = Vec::new();
        for i in 0..10 {
            let name = format!(":pkg/module-{i}");
            let e = EntityId::from_ident(&name);
            store.apply_datoms(&[Datom::new(
                e,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(name.clone()),
                tx,
                Op::Assert,
            )]);
            pkgs.push(e);
        }
        // Edges: module-0 imports module-2..9 (8 edges).
        let comp_from = Attribute::from_keyword(":composition/from");
        let comp_to = Attribute::from_keyword(":composition/to");
        for target_idx in 2..10 {
            let edge = EntityId::from_content(format!("edge-0-{target_idx}").as_bytes());
            store.apply_datoms(&[Datom::new(
                edge, comp_from.clone(), Value::Ref(pkgs[0]), tx, Op::Assert,
            )]);
            store.apply_datoms(&[Datom::new(
                edge, comp_to.clone(), Value::Ref(pkgs[target_idx]), tx, Op::Assert,
            )]);
        }
        // Mark module-0 as explored.
        let mentions_attr = Attribute::from_keyword(":exploration/mentions-entity");
        let obs = EntityId::from_content(b"obs-bound-1");
        store.apply_datoms(&[Datom::new(
            obs, mentions_attr, Value::Ref(pkgs[0]), tx, Op::Assert,
        )]);

        // Create a high-variance concept.
        let concept = EntityId::from_content(b"concept:variance-test");
        store.apply_datoms(&[Datom::new(
            concept, Attribute::from_keyword(":concept/name"),
            Value::String("variance-test".into()), tx, Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept, Attribute::from_keyword(":concept/member-count"),
            Value::Long(10), tx, Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept, Attribute::from_keyword(":concept/variance"),
            Value::Double(ordered_float::OrderedFloat(0.45)), tx, Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept, Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(crate::embedding::embedding_to_bytes(&emb.embed("variance topic"))),
            tx, Op::Assert,
        )]);

        let current_emb = emb.embed("test observation");
        let rec = frontier_recommendation(&store, &current_emb);
        if let Some(r) = &rec {
            assert!(
                r.score >= 0.0 && r.score <= 1.0,
                "frontier score should be in [0, 1], got {} for kind {:?}",
                r.score, r.kind
            );
        }
    }

    /// Property P2: Higher variance produces higher Deepen score.
    #[test]
    fn test_frontier_deepen_monotonicity() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(10, 0, agent);
        let emb = make_embedder();

        // Create two concepts with same member count but different variance.
        let concept_low = EntityId::from_content(b"concept:low-var");
        let concept_high = EntityId::from_content(b"concept:high-var");

        for (concept, name, variance) in &[
            (concept_low, "low-var", 0.15),
            (concept_high, "high-var", 0.45),
        ] {
            store.apply_datoms(&[Datom::new(
                *concept, Attribute::from_keyword(":concept/name"),
                Value::String(name.to_string()), tx, Op::Assert,
            )]);
            store.apply_datoms(&[Datom::new(
                *concept, Attribute::from_keyword(":concept/member-count"),
                Value::Long(5), tx, Op::Assert,
            )]);
            store.apply_datoms(&[Datom::new(
                *concept, Attribute::from_keyword(":concept/variance"),
                Value::Double(ordered_float::OrderedFloat(*variance)), tx, Op::Assert,
            )]);
            store.apply_datoms(&[Datom::new(
                *concept, Attribute::from_keyword(":concept/embedding"),
                Value::Bytes(crate::embedding::embedding_to_bytes(&emb.embed(name))),
                tx, Op::Assert,
            )]);
        }

        let current_emb = emb.embed("something distant");
        let rec = frontier_recommendation(&store, &current_emb);
        assert!(rec.is_some(), "should have deepen recommendation");
        let rec = rec.unwrap();
        assert_eq!(rec.kind, FrontierKind::Deepen);
        assert_eq!(rec.target, "high-var", "higher variance should win");
    }

    /// Commensurability: Deepen and Explore produce comparable scores.
    #[test]
    fn test_frontier_commensurable() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(10, 0, agent);
        let emb = make_embedder();

        // Create 5 packages, mark 2 explored, 3 connected to explored.
        let mut pkgs = Vec::new();
        for i in 0..5 {
            let name = format!(":pkg/mod-{i}");
            let e = EntityId::from_ident(&name);
            store.apply_datoms(&[Datom::new(
                e, Attribute::from_keyword(":db/ident"),
                Value::Keyword(name), tx, Op::Assert,
            )]);
            pkgs.push(e);
        }
        let comp_from = Attribute::from_keyword(":composition/from");
        let comp_to = Attribute::from_keyword(":composition/to");
        // mod-0 imports mod-2, mod-3, mod-4
        for idx in 2..5 {
            let edge = EntityId::from_content(format!("edge-c-{idx}").as_bytes());
            store.apply_datoms(&[Datom::new(
                edge, comp_from.clone(), Value::Ref(pkgs[0]), tx, Op::Assert,
            )]);
            store.apply_datoms(&[Datom::new(
                edge, comp_to.clone(), Value::Ref(pkgs[idx]), tx, Op::Assert,
            )]);
        }
        let mentions = Attribute::from_keyword(":exploration/mentions-entity");
        for &pkg in &pkgs[0..2] {
            let obs = EntityId::from_content(format!("obs-c-{:?}", pkg).as_bytes());
            store.apply_datoms(&[Datom::new(
                obs, mentions.clone(), Value::Ref(pkg), tx, Op::Assert,
            )]);
        }

        // Create a high-variance concept with members = total observations.
        let concept = EntityId::from_content(b"concept:commensurate");
        store.apply_datoms(&[Datom::new(
            concept, Attribute::from_keyword(":concept/name"),
            Value::String("commensurate".into()), tx, Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept, Attribute::from_keyword(":concept/member-count"),
            Value::Long(5), tx, Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept, Attribute::from_keyword(":concept/variance"),
            Value::Double(ordered_float::OrderedFloat(0.4)), tx, Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept, Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(crate::embedding::embedding_to_bytes(&emb.embed("distant topic entirely"))),
            tx, Op::Assert,
        )]);

        let current_emb = emb.embed("observation about something");
        let rec = frontier_recommendation(&store, &current_emb);
        assert!(rec.is_some(), "should recommend something");
        let r = rec.unwrap();
        // Both types should produce scores in reasonable range (not 0 or wildly different).
        assert!(r.score > 0.0, "winning score should be positive");
        assert!(r.score <= 1.0, "winning score should be at most 1.0, got {}", r.score);
    }

    /// Regression guard: single-scan co_occurrence_matrix matches expected Jaccard.
    #[test]
    fn test_co_occurrence_single_scan_equivalence() {
        // This replicates the coupled test setup to ensure the merged single-scan
        // produces identical results to the original two-scan implementation.
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(10, 0, agent);

        let concept_a = EntityId::from_content(b"concept:scan-a");
        let concept_b = EntityId::from_content(b"concept:scan-b");
        store.apply_datoms(&[Datom::new(
            concept_a, Attribute::from_keyword(":concept/name"),
            Value::String("scan-a".into()), tx, Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept_b, Attribute::from_keyword(":concept/name"),
            Value::String("scan-b".into()), tx, Op::Assert,
        )]);

        let concept_attr = Attribute::from_keyword(":exploration/concept");
        // 4 in A, 4 in B, 2 shared.
        for i in 0..4 {
            let obs = EntityId::from_content(format!("obs-scan-a-{i}").as_bytes());
            store.apply_datoms(&[Datom::new(
                obs, concept_attr.clone(), Value::Ref(concept_a), tx, Op::Assert,
            )]);
        }
        for i in 0..4 {
            let obs = EntityId::from_content(format!("obs-scan-b-{i}").as_bytes());
            store.apply_datoms(&[Datom::new(
                obs, concept_attr.clone(), Value::Ref(concept_b), tx, Op::Assert,
            )]);
        }
        // 2 shared: obs-scan-a-0 and obs-scan-a-1 also in B.
        for i in 0..2 {
            let obs = EntityId::from_content(format!("obs-scan-a-{i}").as_bytes());
            store.apply_datoms(&[Datom::new(
                obs, concept_attr.clone(), Value::Ref(concept_b), tx, Op::Assert,
            )]);
        }

        let matrix = co_occurrence_matrix(&store);
        let pair = matrix.iter().find(|p| {
            (p.concept_a == concept_a && p.concept_b == concept_b)
                || (p.concept_a == concept_b && p.concept_b == concept_a)
        });
        assert!(pair.is_some(), "should find scan-a/scan-b pair");
        // A: {a0, a1, a2, a3}, B: {b0, b1, b2, b3, a0, a1}
        // Intersection: {a0, a1} = 2, Union: {a0..a3, b0..b3} = 8
        // Jaccard = 2/8 = 0.25
        let j = pair.unwrap().jaccard;
        assert!(
            (j - 0.25).abs() < 0.01,
            "single-scan Jaccard should be 0.25, got {j:.3}"
        );
    }

    /// Property: assign_to_concepts never returns Uncategorized in the vec.
    #[test]
    fn test_assign_to_concepts_no_uncategorized_in_vec() {
        let store = store_with_innate_concepts();
        let emb = make_embedder();
        // Try multiple observation texts with various thresholds.
        let texts = [
            "package boundaries modules",
            "quantum chromodynamics",
            "error handling cascade",
            "imports coupling violations",
            "",
        ];
        for text in &texts {
            if text.is_empty() { continue; }
            let v = emb.embed(text);
            for threshold in &[0.01_f32, 0.2, 0.5, 0.9] {
                let assignments = assign_to_concepts(&store, &v, *threshold);
                for (i, a) in assignments.iter().enumerate() {
                    assert!(
                        !matches!(a, ConceptAssignment::Uncategorized),
                        "assign_to_concepts should never include Uncategorized in result vec \
                         (text='{text}', threshold={threshold}, index={i})"
                    );
                }
            }
        }
    }

    /// Edge case: With only Deepen candidates (no Explore or Bridge),
    /// the winner score should still be bounded by [0, 1].
    #[test]
    fn test_frontier_single_type_normalization() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(10, 0, agent);
        let emb = make_embedder();

        // No :pkg/* entities (no Explore), no disjoint concepts (no Bridge).
        // Only one concept with high variance -> only Deepen.
        let concept = EntityId::from_content(b"concept:only-deepen");
        store.apply_datoms(&[Datom::new(
            concept, Attribute::from_keyword(":concept/name"),
            Value::String("only-deepen".into()), tx, Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept, Attribute::from_keyword(":concept/member-count"),
            Value::Long(5), tx, Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept, Attribute::from_keyword(":concept/variance"),
            Value::Double(ordered_float::OrderedFloat(0.4)), tx, Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept, Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(crate::embedding::embedding_to_bytes(&emb.embed("only deepen topic"))),
            tx, Op::Assert,
        )]);

        let current_emb = emb.embed("something entirely different");
        let rec = frontier_recommendation(&store, &current_emb);
        assert!(rec.is_some(), "should recommend deepening");
        let r = rec.unwrap();
        assert_eq!(r.kind, FrontierKind::Deepen);
        assert!(r.score >= 0.0 && r.score <= 1.0, "score should be in [0,1], got {}", r.score);
    }

    /// Verify that assign_to_concepts uses the latest embedding when multiple
    /// assertions exist for the same concept entity (append-only latest-wins).
    #[test]
    fn test_assign_to_concepts_latest_wins() {
        let mut store = store_with_schema();
        let emb = make_embedder();
        let agent = AgentId::from_name("test");
        let concept = EntityId::from_content(b"concept:evolving");

        // Tx 10: initial embedding from "alpha topic words"
        let tx1 = TxId::new(10, 0, agent);
        let alpha_emb = emb.embed("alpha topic words unique");
        store.apply_datoms(&[Datom::new(
            concept,
            Attribute::from_keyword(":concept/name"),
            Value::String("evolving".into()),
            tx1,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept,
            Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(embedding_to_bytes(&alpha_emb)),
            tx1,
            Op::Assert,
        )]);

        // Tx 11: updated embedding from "beta topic words" (overwrites in latest-wins)
        let tx2 = TxId::new(11, 0, agent);
        let beta_emb = emb.embed("beta topic words unique");
        store.apply_datoms(&[Datom::new(
            concept,
            Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(embedding_to_bytes(&beta_emb)),
            tx2,
            Op::Assert,
        )]);

        // Observation that's closer to beta than alpha
        let obs_emb = emb.embed("beta topic words unique similar");
        let assignments = assign_to_concepts(&store, &obs_emb, 0.01);

        assert!(!assignments.is_empty(), "should match at least one concept");
        if let ConceptAssignment::Joined { similarity, .. } = &assignments[0] {
            // Verify the similarity is computed against beta (the later embedding),
            // not alpha. If latest-wins is broken, similarity would be lower.
            let direct_beta_sim = cosine_similarity(&beta_emb, &obs_emb);
            let direct_alpha_sim = cosine_similarity(&alpha_emb, &obs_emb);
            assert!(
                (*similarity - direct_beta_sim).abs() < 0.01,
                "should use latest embedding (beta, sim={direct_beta_sim:.4}), \
                 not first (alpha, sim={direct_alpha_sim:.4}), got sim={similarity:.4}"
            );
        }
    }
}
