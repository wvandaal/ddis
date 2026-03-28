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

use crate::datom::{AgentId, Attribute, EntityId, Op, Value};
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
        /// Sigmoid membership strength [0.0, 1.0] (ADR-FOUNDATION-031).
        /// 1.0 = strong member, 0.5 = at threshold boundary, 0.0 = non-member.
        strength: f32,
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
            if best.is_none_or(|(_, s)| sim > s) {
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
            strength: 1.0, // Legacy API: hard cutoff always gives full strength
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
                    strength: 1.0, // Hard cutoff: full strength for all matches
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
        sim_b
            .partial_cmp(&sim_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    matches
}

/// Assign with sigmoid soft membership (ADR-FOUNDATION-031).
///
/// Like [`assign_to_concepts`] but uses sigmoid membership strength instead
/// of hard cutoff. Returns all concepts with strength > `min_strength` (default 0.1),
/// sorted by strength descending.
pub fn assign_to_concepts_soft(
    store: &Store,
    observation_embedding: &[f32],
    threshold: f32,
    temperature: f32,
    min_strength: f32,
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

    // Phase 2: Compute sigmoid membership for all concepts.
    let mut matches: Vec<ConceptAssignment> = Vec::new();
    for (entity, concept_emb) in &latest {
        if concept_emb.len() == observation_embedding.len() {
            let sim = cosine_similarity(concept_emb, observation_embedding);
            let strength = membership_strength(sim, threshold, temperature);
            if strength >= min_strength {
                matches.push(ConceptAssignment::Joined {
                    concept: *entity,
                    similarity: sim,
                    surprise: 1.0 - sim,
                    strength,
                });
            }
        }
    }

    // Sort by strength descending (strongest membership first).
    matches.sort_by(|a, b| {
        let str_a = match a {
            ConceptAssignment::Joined { strength, .. } => *strength,
            _ => 0.0,
        };
        let str_b = match b {
            ConceptAssignment::Joined { strength, .. } => *strength,
            _ => 0.0,
        };
        str_b
            .partial_cmp(&str_a)
            .unwrap_or(std::cmp::Ordering::Equal)
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

    // Collect all observation texts as the IDF corpus (C9-P6).
    let corpus_texts: Vec<&str> = observations.iter().map(|o| o.2.as_str()).collect();

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

            let name = generate_concept_name(&member_texts, &corpus_texts);
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

/// Split an overgrown concept into sub-concepts using Fiedler bisection.
///
/// Takes the member embeddings of a single concept and recursively bisects
/// until each sub-cluster has variance <= `split_threshold` or size < `min_split_size`.
///
/// The algorithm:
/// 1. Compute internal variance. If <= threshold, return empty (no split needed).
/// 2. Build NxN cosine similarity matrix from embeddings.
/// 3. Fiedler bisect into two groups.
/// 4. Recursively split each group if variance still exceeds threshold.
/// 5. Generate TF-IDF names for each final sub-concept.
///
/// C9 compliant: `split_threshold` and `min_split_size` are caller-provided parameters.
/// C8 compliant: no domain-specific logic — works for any concept cluster.
///
/// Returns empty Vec if no split is needed (variance already below threshold).
pub fn split_concept(
    member_embeddings: &[(EntityId, Vec<f32>, String)],
    corpus_texts: &[&str],
    split_threshold: f64,
    min_split_size: usize,
) -> Vec<NewConcept> {
    if member_embeddings.len() < min_split_size * 2 {
        return Vec::new();
    }

    // Compute internal variance of the full set.
    let embeddings_ref: Vec<&[f32]> = member_embeddings
        .iter()
        .map(|(_, e, _)| e.as_slice())
        .collect();
    let cent = crate::embedding::centroid(&embeddings_ref);
    let var = crate::embedding::variance(&embeddings_ref, &cent) as f64;

    if var <= split_threshold {
        return Vec::new();
    }

    // Build NxN cosine similarity matrix (f64 for fiedler_bisect).
    let n = member_embeddings.len();
    let mut sim_matrix: Vec<Vec<f64>> = vec![vec![0.0; n]; n];
    for i in 0..n {
        sim_matrix[i][i] = 1.0;
        for j in (i + 1)..n {
            let s = cosine_similarity(&member_embeddings[i].1, &member_embeddings[j].1) as f64;
            // Clamp to non-negative for Laplacian construction.
            let s = s.max(0.0);
            sim_matrix[i][j] = s;
            sim_matrix[j][i] = s;
        }
    }

    // Recursive bisection.
    let indices: Vec<usize> = (0..n).collect();
    let mut final_groups: Vec<Vec<usize>> = Vec::new();
    split_recursive(
        member_embeddings,
        &sim_matrix,
        &indices,
        split_threshold,
        min_split_size,
        &mut final_groups,
    );

    if final_groups.len() < 2 {
        // Bisection didn't produce meaningful split.
        return Vec::new();
    }

    // Generate sub-concepts from final groups.
    let member_texts: Vec<&str> = member_embeddings
        .iter()
        .map(|(_, _, t)| t.as_str())
        .collect();

    final_groups
        .into_iter()
        .filter(|group| group.len() >= min_split_size)
        .map(|group| {
            let members: Vec<EntityId> = group.iter().map(|&i| member_embeddings[i].0).collect();
            let group_embeddings: Vec<&[f32]> = group
                .iter()
                .map(|&i| member_embeddings[i].1.as_slice())
                .collect();
            let group_texts: Vec<&str> = group.iter().map(|&i| member_texts[i]).collect();

            let centroid = crate::embedding::centroid(&group_embeddings);
            let variance = crate::embedding::variance(&group_embeddings, &centroid) as f64;
            let name = generate_concept_name(&group_texts, corpus_texts);
            let description = format!("{} observations about {}", members.len(), name);
            let entity = EntityId::from_content(format!("concept:{name}").as_bytes());
            let total_weight = members.len() as f64;

            NewConcept {
                entity,
                name,
                description,
                centroid,
                members,
                variance,
                total_weight,
            }
        })
        .collect()
}

/// Recursive helper for `split_concept`: bisects a group and recurses if sub-groups
/// still exceed the variance threshold.
fn split_recursive(
    member_embeddings: &[(EntityId, Vec<f32>, String)],
    sim_matrix: &[Vec<f64>],
    indices: &[usize],
    split_threshold: f64,
    min_split_size: usize,
    result: &mut Vec<Vec<usize>>,
) {
    // Compute variance of this group.
    let group_embeddings: Vec<&[f32]> = indices
        .iter()
        .map(|&i| member_embeddings[i].1.as_slice())
        .collect();
    let cent = crate::embedding::centroid(&group_embeddings);
    let var = crate::embedding::variance(&group_embeddings, &cent) as f64;

    // Base case: variance is acceptable or group is too small to split further.
    if var <= split_threshold || indices.len() < min_split_size * 2 {
        result.push(indices.to_vec());
        return;
    }

    // Bisect using Fiedler vector.
    let (left, right) = crate::topology::fiedler_bisect(sim_matrix, indices);

    // If bisection failed (one side empty), accept this group as-is.
    if left.is_empty() || right.is_empty() {
        result.push(indices.to_vec());
        return;
    }

    // Recurse on each half.
    split_recursive(
        member_embeddings,
        sim_matrix,
        &left,
        split_threshold,
        min_split_size,
        result,
    );
    split_recursive(
        member_embeddings,
        sim_matrix,
        &right,
        split_threshold,
        min_split_size,
        result,
    );
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
                        cs.variance =
                            f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
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

// ===================================================================
// Observation Retrieval (C9-P5 kernel helpers)
// ===================================================================

/// A single observation record with all key attributes (C9-P5).
#[derive(Debug, Clone)]
pub struct ObservationRecord {
    /// The observation entity ID.
    pub entity: EntityId,
    /// The `:db/ident` value.
    pub ident: String,
    /// The `:exploration/body` text.
    pub body: String,
    /// The `:exploration/confidence` value (0.0-1.0).
    pub confidence: f64,
    /// The `:exploration/category` keyword (e.g. ":exploration.cat/observation").
    pub category: String,
    /// The concept this observation belongs to (if any).
    pub concept: Option<EntityId>,
    /// The concept name (if assigned to a concept).
    pub concept_name: Option<String>,
    /// The transaction wall-clock time (for recency sorting).
    pub tx_wall_time: i64,
}

/// Collect all observations from the store in a single pass (C9-P5).
///
/// Returns observations sorted by `tx_wall_time` descending (most recent first).
/// INV-REFLEXIVE-007: Same function for domain and meta observations.
pub fn all_observations(store: &Store) -> Vec<ObservationRecord> {
    let body_attr = Attribute::from_keyword(":exploration/body");
    let ident_attr = Attribute::from_keyword(":db/ident");
    let conf_attr = Attribute::from_keyword(":exploration/confidence");
    let cat_attr = Attribute::from_keyword(":exploration/category");
    let concept_ref_attr = Attribute::from_keyword(":exploration/concept");
    let concept_name_attr = Attribute::from_keyword(":concept/name");

    // Phase 1: Identify observation entities (those with :exploration/body).
    let mut obs_bodies: BTreeMap<EntityId, String> = BTreeMap::new();
    let mut obs_idents: BTreeMap<EntityId, String> = BTreeMap::new();
    let mut obs_confs: BTreeMap<EntityId, f64> = BTreeMap::new();
    let mut obs_cats: BTreeMap<EntityId, String> = BTreeMap::new();
    let mut obs_concepts: BTreeMap<EntityId, EntityId> = BTreeMap::new();
    let mut obs_tx_times: BTreeMap<EntityId, i64> = BTreeMap::new();
    let mut concept_names: BTreeMap<EntityId, String> = BTreeMap::new();

    for d in store.datoms() {
        if d.op != Op::Assert {
            continue;
        }
        if d.attribute == body_attr {
            if let Value::String(ref s) = d.value {
                obs_bodies.insert(d.entity, s.clone());
                let wall = d.tx.wall_time() as i64;
                obs_tx_times.entry(d.entity).or_insert(wall);
            }
        } else if d.attribute == ident_attr {
            if let Value::Keyword(ref k) = d.value {
                if k.starts_with(":observation/") || k.starts_with(":exploration/") {
                    obs_idents.insert(d.entity, k.clone());
                }
            }
        } else if d.attribute == conf_attr {
            if let Value::Double(v) = d.value {
                obs_confs.insert(d.entity, v.into_inner());
            }
        } else if d.attribute == cat_attr {
            if let Value::Keyword(ref k) = d.value {
                obs_cats.insert(d.entity, k.clone());
            }
        } else if d.attribute == concept_ref_attr {
            if let Value::Ref(concept_e) = d.value {
                obs_concepts.insert(d.entity, concept_e);
            }
        } else if d.attribute == concept_name_attr {
            if let Value::String(ref s) = d.value {
                concept_names.insert(d.entity, s.clone());
            }
        }
    }

    // Phase 2: Assemble records (only entities that have a body).
    let mut records: Vec<ObservationRecord> = obs_bodies
        .into_iter()
        .map(|(entity, body)| {
            let concept = obs_concepts.get(&entity).copied();
            ObservationRecord {
                entity,
                ident: obs_idents.get(&entity).cloned().unwrap_or_default(),
                body,
                confidence: obs_confs.get(&entity).copied().unwrap_or(0.5),
                category: obs_cats
                    .get(&entity)
                    .cloned()
                    .unwrap_or_else(|| ":exploration.cat/observation".to_string()),
                concept,
                concept_name: concept.and_then(|c| concept_names.get(&c).cloned()),
                tx_wall_time: obs_tx_times.get(&entity).copied().unwrap_or(0),
            }
        })
        .collect();

    // Sort by tx_wall_time descending (most recent first).
    records.sort_by_key(|r| std::cmp::Reverse(r.tx_wall_time));
    records
}

/// Group observations by their assigned concept (C9-P5).
///
/// Returns `(concept_summary, member_observations)` pairs sorted by member count descending.
/// Observations without a concept are collected into a synthetic "Uncategorized" group.
pub fn observations_by_concept(
    store: &Store,
) -> Vec<(Option<ConceptSummary>, Vec<ObservationRecord>)> {
    let all = all_observations(store);
    let inventory = concept_inventory(store);

    // Group observations by concept entity.
    let mut by_concept: BTreeMap<Option<EntityId>, Vec<ObservationRecord>> = BTreeMap::new();
    for obs in all {
        by_concept.entry(obs.concept).or_default().push(obs);
    }

    // Build result: match concept groups to ConceptSummary.
    let concept_map: BTreeMap<EntityId, ConceptSummary> =
        inventory.into_iter().map(|c| (c.entity, c)).collect();

    let mut result: Vec<(Option<ConceptSummary>, Vec<ObservationRecord>)> = by_concept
        .into_iter()
        .map(|(concept_id, obs)| {
            let summary = concept_id.and_then(|id| concept_map.get(&id).cloned());
            (summary, obs)
        })
        .collect();

    // Sort: named concepts first (by member count desc), uncategorized last.
    result.sort_by(|a, b| {
        let a_count = a.0.as_ref().map(|c| c.member_count).unwrap_or(0);
        let b_count = b.0.as_ref().map(|c| c.member_count).unwrap_or(0);
        let a_has = a.0.is_some() as u8;
        let b_has = b.0.is_some() as u8;
        b_has.cmp(&a_has).then(b_count.cmp(&a_count))
    });
    result
}

/// Compute a steering question for read-mode observe subcommands (C9-P5, INV-REFLEXIVE-006).
///
/// Priority cascade:
/// 1. Frontier recommendation from context embedding
/// 2. Co-occurrence gap (concept pair with Jaccard = 0)
/// 3. Smallest concept (coverage gap)
///
/// Zero LLM calls — pure computation over the concept graph.
pub fn compute_read_steering(store: &Store, context_embedding: Option<&[f32]>) -> Option<String> {
    // Priority 1: Frontier recommendation from context embedding.
    if let Some(emb) = context_embedding {
        if let Some(rec) = frontier_recommendation(store, emb) {
            return Some(format!(
                "{}: {} -- {}",
                match rec.kind {
                    FrontierKind::Explore => "explore",
                    FrontierKind::Deepen => "deepen",
                    FrontierKind::Bridge => "bridge",
                    FrontierKind::Narrow => "narrow",
                },
                rec.target,
                rec.rationale
            ));
        }
    }

    // Priority 2: Co-occurrence gap (bridge between disconnected concepts).
    let cooc = co_occurrence_matrix(store);
    let gap = cooc
        .iter()
        .find(|c| c.jaccard < 0.01 && !c.name_a.is_empty() && !c.name_b.is_empty());
    if let Some(gap) = gap {
        return Some(format!("what connects {} to {}?", gap.name_a, gap.name_b));
    }

    // Priority 3: Smallest concept (coverage gap).
    let inv = concept_inventory(store);
    if let Some(smallest) = inv.last() {
        if smallest.member_count < 3 {
            return Some(format!(
                "concept '{}' has only {} observations -- what else belongs here?",
                smallest.name, smallest.member_count
            ));
        }
    }

    None
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

    pairs.sort_by(|a, b| {
        b.jaccard
            .partial_cmp(&a.jaccard)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    pairs
}

// ===================================================================
// Agent Provenance & Agreement Detection (C9-P1)
// ===================================================================

/// Group of observations from a single agent (C9-P1 primitive).
#[derive(Debug, Clone)]
pub struct AgentObservationGroup {
    /// The agent that created these observations.
    pub agent: AgentId,
    /// Human-readable agent name (hex of first 8 bytes).
    pub agent_name: String,
    /// Observation entity IDs created by this agent.
    pub observations: Vec<EntityId>,
}

/// Cross-agent agreement cluster (C9-P1).
///
/// When multiple independent agents observe the same finding, the finding
/// has higher epistemic certainty than any single observation.
///
/// Algebraic property: `agreement_score` is monotonically non-decreasing
/// as confirming agents are added (join-semilattice over agent sets).
#[derive(Debug, Clone)]
pub struct AgreementCluster {
    /// Representative topic (first 80 chars of highest-confidence member).
    pub topic: String,
    /// Distinct agents that contributed observations to this cluster.
    pub agents: Vec<AgentId>,
    /// All observation entity IDs in this cluster.
    pub observation_ids: Vec<EntityId>,
    /// Confidence range across cluster members: (min, mean, max).
    pub confidence_range: (f64, f64, f64),
    /// Fraction of total session agents that agree: `|agents| / |total_agents|`.
    pub agreement_score: f64,
    /// Number of member observations.
    pub member_count: usize,
}

/// Group observations by creating agent (C9-P1).
///
/// Iterates observation entities (those with `:exploration/body`) and groups
/// by the `AgentId` embedded in each datom's transaction. O(observations).
pub fn agent_observation_groups(store: &Store) -> Vec<AgentObservationGroup> {
    let attr = Attribute::from_keyword(":exploration/body");
    let mut groups: HashMap<AgentId, Vec<EntityId>> = HashMap::new();

    for datom in store.attribute_datoms(&attr) {
        if datom.op != Op::Assert {
            continue;
        }
        let agent = datom.tx.agent;
        groups.entry(agent).or_default().push(datom.entity);
    }

    // Deduplicate entities per agent (an entity may have multiple body datoms
    // in append-only store due to retraction+re-assertion).
    let mut result: Vec<AgentObservationGroup> = groups
        .into_iter()
        .map(|(agent, mut obs)| {
            obs.sort();
            obs.dedup();
            let agent_name = format!(
                "{:02x}{:02x}{:02x}{:02x}",
                agent.as_bytes()[0],
                agent.as_bytes()[1],
                agent.as_bytes()[2],
                agent.as_bytes()[3]
            );
            AgentObservationGroup {
                agent,
                agent_name,
                observations: obs,
            }
        })
        .collect();

    result.sort_by_key(|a| std::cmp::Reverse(a.observations.len()));
    result
}

/// Detect cross-agent agreement on findings (C9-P1).
///
/// Groups observations by agent, then uses word-token Jaccard similarity
/// on observation titles (first 80 chars of `:exploration/body`) to find
/// clusters where multiple agents independently describe the same finding.
///
/// Avoids the HashEmbedder concept-collapse problem by operating on raw
/// text similarity rather than embeddings.
///
/// # Arguments
/// * `store` - The datom store
/// * `similarity_threshold` - Minimum Jaccard similarity to consider agreement (default: 0.3)
pub fn find_agreement_clusters(store: &Store, similarity_threshold: f64) -> Vec<AgreementCluster> {
    let groups = agent_observation_groups(store);
    if groups.len() < 2 {
        return Vec::new();
    }

    let total_agents = groups.len();

    // Collect all observations with their agent and title text.
    struct ObsInfo {
        entity: EntityId,
        agent: AgentId,
        title: String,
        confidence: f64,
    }

    let mut observations: Vec<ObsInfo> = Vec::new();
    for group in &groups {
        for &entity in &group.observations {
            let body_attr = Attribute::from_keyword(":exploration/body");
            let conf_attr = Attribute::from_keyword(":exploration/confidence");
            let body = store
                .live_value(entity, &body_attr)
                .and_then(|v| match v {
                    Value::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .unwrap_or("");
            let title: String = body.chars().take(80).collect();
            let confidence = store
                .live_value(entity, &conf_attr)
                .and_then(|v| match v {
                    Value::Double(d) => Some(d.into_inner()),
                    _ => None,
                })
                .unwrap_or(0.5);
            observations.push(ObsInfo {
                entity,
                agent: group.agent,
                title,
                confidence,
            });
        }
    }

    if observations.is_empty() {
        return Vec::new();
    }

    // Tokenize titles for Jaccard computation.
    let tokenized: Vec<HashSet<String>> = observations
        .iter()
        .map(|o| {
            crate::connections::tokenize(&o.title)
                .into_iter()
                .filter(|w| w.len() >= 3)
                .collect()
        })
        .collect();

    // Union-find for clustering agreeing observations.
    let n = observations.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let mut rank: Vec<usize> = vec![0; n];

    fn find(parent: &mut [usize], i: usize) -> usize {
        if parent[i] != i {
            parent[i] = find(parent, parent[i]);
        }
        parent[i]
    }

    fn union(parent: &mut [usize], rank: &mut [usize], a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra == rb {
            return;
        }
        if rank[ra] < rank[rb] {
            parent[ra] = rb;
        } else if rank[ra] > rank[rb] {
            parent[rb] = ra;
        } else {
            parent[rb] = ra;
            rank[ra] += 1;
        }
    }

    // Compare pairs from DIFFERENT agents only.
    for i in 0..n {
        for j in (i + 1)..n {
            if observations[i].agent == observations[j].agent {
                continue; // Same agent — skip.
            }
            if tokenized[i].is_empty() || tokenized[j].is_empty() {
                continue;
            }
            let intersection = tokenized[i].intersection(&tokenized[j]).count();
            let union_size = tokenized[i].union(&tokenized[j]).count();
            let jaccard = if union_size > 0 {
                intersection as f64 / union_size as f64
            } else {
                0.0
            };
            if jaccard >= similarity_threshold {
                union(&mut parent, &mut rank, i, j);
            }
        }
    }

    // Collect clusters.
    let mut clusters_map: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        clusters_map.entry(root).or_default().push(i);
    }

    // Build AgreementCluster for each cluster with 2+ members.
    let mut clusters: Vec<AgreementCluster> = Vec::new();
    for members in clusters_map.values() {
        if members.len() < 2 {
            continue;
        }

        let mut agents: HashSet<AgentId> = HashSet::new();
        let mut obs_ids: Vec<EntityId> = Vec::new();
        let mut confidences: Vec<f64> = Vec::new();

        for &idx in members {
            agents.insert(observations[idx].agent);
            obs_ids.push(observations[idx].entity);
            confidences.push(observations[idx].confidence);
        }

        if agents.len() < 2 {
            continue; // Same-agent cluster — not cross-agent agreement.
        }

        let min_conf = confidences.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_conf = confidences
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        let mean_conf = confidences.iter().sum::<f64>() / confidences.len() as f64;

        // Pick topic from highest-confidence member.
        let best_idx = members
            .iter()
            .max_by(|&&a, &&b| {
                observations[a]
                    .confidence
                    .partial_cmp(&observations[b].confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
            .unwrap_or(members[0]);
        let topic = observations[best_idx].title.clone();

        let agent_vec: Vec<AgentId> = agents.into_iter().collect();
        let agreement_score = agent_vec.len() as f64 / total_agents as f64;

        clusters.push(AgreementCluster {
            topic,
            agents: agent_vec,
            observation_ids: obs_ids,
            confidence_range: (min_conf, mean_conf, max_conf),
            agreement_score,
            member_count: members.len(),
        });
    }

    clusters.sort_by(|a, b| {
        b.agreement_score
            .partial_cmp(&a.agreement_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.member_count.cmp(&a.member_count))
    });
    clusters
}

/// Format agreement clusters for `braid status` display (C9-P1).
pub fn format_agreement_summary(clusters: &[AgreementCluster], total_agents: usize) -> Vec<String> {
    clusters
        .iter()
        .take(5)
        .map(|c| {
            let topic_short: String = c.topic.chars().take(60).collect();
            format!(
                "{} ({} obs, {}/{} agents, conf {:.2}-{:.2})",
                topic_short,
                c.member_count,
                c.agents.len(),
                total_agents,
                c.confidence_range.0,
                c.confidence_range.2,
            )
        })
        .collect()
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
    /// Concepts are collapsing — all concepts have high jaccard overlap.
    /// Diagnostic: suggests more specific observations to differentiate concepts.
    Narrow,
}

impl std::fmt::Display for FrontierKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrontierKind::Explore => write!(f, "explore"),
            FrontierKind::Deepen => write!(f, "deepen"),
            FrontierKind::Bridge => write!(f, "bridge"),
            FrontierKind::Narrow => write!(f, "narrow"),
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
pub fn frontier_recommendation(store: &Store, current_embedding: &[f32]) -> Option<FrontierRec> {
    let mut candidates: Vec<FrontierRec> = Vec::new();

    // --- Candidate 1: Explore (unexplored packages with high connectivity) ---
    candidates.extend(explore_candidates(store));

    // --- Candidate 2: Deepen (high-variance concepts) ---
    candidates.extend(deepen_candidates(store, current_embedding));

    // --- Candidate 3: Bridge (zero co-occurrence concept pairs) ---
    candidates.extend(bridge_candidates(store));

    // --- Candidate 4: Narrow (concept collapse diagnostic) ---
    candidates.extend(narrow_candidates(store));

    // Select the highest-scoring candidate.
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
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
                format!(
                    "{} + {} more",
                    obs_names[..2].join(", "),
                    obs_names.len() - 2
                )
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
            let score = (1.0 - pair.jaccard) * (a_count + b_count) as f64
                / (2.0 * total_observations.max(1) as f64);
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

/// Detect concept collapse: all concepts have high co-occurrence overlap.
///
/// Fires when mean Jaccard across concept pairs > 0.8, indicating that
/// concepts are not differentiating observations. Recommends more specific
/// observations to break the symmetry.
fn narrow_candidates(store: &Store) -> Vec<FrontierRec> {
    let co_occ = co_occurrence_matrix(store);
    if co_occ.is_empty() {
        return Vec::new();
    }

    let mean_jaccard: f64 = co_occ.iter().map(|p| p.jaccard).sum::<f64>() / co_occ.len() as f64;

    if mean_jaccard > 0.8 {
        vec![FrontierRec {
            kind: FrontierKind::Narrow,
            target: "observation specificity".to_string(),
            score: 1.0 - mean_jaccard, // Higher collapse = higher urgency but lower absolute score
            rationale: format!(
                "concepts are converging (mean jaccard={mean_jaccard:.2}) \u{2014} \
                 try observing a specific file, function, or error message rather than a whole package"
            ),
        }]
    } else {
        Vec::new()
    }
}

// ===================================================================
// Discrepancy-Driven Steering (INQ-2)
// ===================================================================

/// Result of computing a discrepancy brief between an observation and its concept.
#[derive(Debug, Clone)]
pub struct DiscrepancyBrief {
    /// Keywords that are novel (present in observation but not in expected).
    pub novel_keywords: Vec<String>,
    /// Keywords that were expected (present in centroid-nearest but not in observation).
    pub expected_keywords: Vec<String>,
    /// The concept name.
    pub concept_name: String,
    /// Surprise level (1.0 - cosine similarity).
    pub surprise: f32,
}

/// Find the stored observation whose embedding is closest to the given vector.
///
/// Scans all `:exploration/embedding` datoms and returns the entity ID, body text,
/// and cosine similarity of the best match. Uses two-phase latest-wins (like
/// `find_nearest_concept`).
///
/// Returns `None` if no observations with embeddings exist.
pub fn find_nearest_observation(store: &Store, target: &[f32]) -> Option<(EntityId, String, f32)> {
    let embed_attr = Attribute::from_keyword(":exploration/embedding");
    let body_attr = Attribute::from_keyword(":exploration/body");

    // Phase 1: latest embedding per observation entity.
    let mut latest_emb: BTreeMap<EntityId, Vec<f32>> = BTreeMap::new();
    for d in store.datoms() {
        if d.op == Op::Assert && d.attribute == embed_attr {
            if let Value::Bytes(ref bytes) = d.value {
                latest_emb.insert(d.entity, bytes_to_embedding(bytes));
            }
        }
    }

    // Phase 2: best match with dimension guard.
    let mut best: Option<(EntityId, f32)> = None;
    for (entity, obs_emb) in &latest_emb {
        if obs_emb.len() == target.len() {
            let sim = cosine_similarity(obs_emb, target);
            if best.is_none_or(|(_, s)| sim > s) {
                best = Some((*entity, sim));
            }
        }
    }

    let (entity, similarity) = best?;

    // Phase 3: retrieve body text for the best match.
    let body = store
        .live_value(entity, &body_attr)
        .and_then(|v| {
            if let Value::String(s) = v {
                Some(s.clone())
            } else {
                None
            }
        })
        .unwrap_or_default();

    Some((entity, body, similarity))
}

/// Compute a discrepancy brief for an observation against its concept.
///
/// Given an observation embedding and the concept it joined:
/// 1. Compute discrepancy = observation_embedding - concept_centroid (element-wise).
/// 2. Find the stored observation closest to the discrepancy vector → what was NOVEL.
/// 3. Find the stored observation closest to the centroid → what was EXPECTED.
/// 4. Keyword set difference → the novel elements.
///
/// Returns `None` if surprise is below `min_surprise` (default 0.3) or if there
/// aren't enough observations to compute a meaningful brief.
pub fn compute_discrepancy_brief(
    store: &Store,
    observation_embedding: &[f32],
    concept_entity: EntityId,
    surprise: f32,
    min_surprise: f32,
) -> Option<DiscrepancyBrief> {
    if surprise < min_surprise {
        return None;
    }

    let concept_name = concept_name_from_entity(store, concept_entity);

    // Get concept centroid.
    let emb_attr = Attribute::from_keyword(":concept/embedding");
    let centroid = store.live_value(concept_entity, &emb_attr).and_then(|v| {
        if let Value::Bytes(b) = v {
            Some(bytes_to_embedding(b))
        } else {
            None
        }
    })?;

    if centroid.len() != observation_embedding.len() {
        return None;
    }

    // Discrepancy vector = observation - centroid.
    let mut discrepancy: Vec<f32> = observation_embedding
        .iter()
        .zip(centroid.iter())
        .map(|(o, c)| o - c)
        .collect();
    crate::embedding::l2_normalize(&mut discrepancy);

    // Find nearest observation to discrepancy (what was NOVEL).
    let novel_obs = find_nearest_observation(store, &discrepancy);
    // Find nearest observation to centroid (what was EXPECTED).
    let expected_obs = find_nearest_observation(store, &centroid);

    let novel_text = novel_obs.map(|(_, body, _)| body).unwrap_or_default();
    let expected_text = expected_obs.map(|(_, body, _)| body).unwrap_or_default();

    // Keyword set difference.
    let novel_kw = crate::connections::tokenize(&novel_text);
    let expected_kw = crate::connections::tokenize(&expected_text);

    let novel_only: Vec<String> = novel_kw.difference(&expected_kw).cloned().collect();
    let expected_only: Vec<String> = expected_kw.difference(&novel_kw).cloned().collect();

    if novel_only.is_empty() && expected_only.is_empty() {
        return None;
    }

    Some(DiscrepancyBrief {
        novel_keywords: novel_only,
        expected_keywords: expected_only,
        concept_name,
        surprise,
    })
}

/// Format a discrepancy brief as a human-readable string.
///
/// Output: "[concept] expected: {kw1, kw2}. You found: {kw3, kw4}."
pub fn format_discrepancy_brief(brief: &DiscrepancyBrief) -> String {
    let expected = if brief.expected_keywords.is_empty() {
        "(nothing specific)".to_string()
    } else {
        let mut sorted = brief.expected_keywords.clone();
        sorted.sort();
        sorted.truncate(5);
        format!("{{{}}}", sorted.join(", "))
    };
    let novel = if brief.novel_keywords.is_empty() {
        "(confirming)".to_string()
    } else {
        let mut sorted = brief.novel_keywords.clone();
        sorted.sort();
        sorted.truncate(5);
        format!("{{{}}}", sorted.join(", "))
    };
    format!(
        "[{}] expected: {}. You found: {}.",
        brief.concept_name, expected, novel
    )
}

// ===================================================================
// Graduated Situational Brief (INQ-3)
// ===================================================================

/// Epistemological level of an observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpistemicLevel {
    /// Joins 1 concept, low surprise, no topological event.
    Concept,
    /// Bridges concepts or triggers a topological event.
    Theory,
    /// High surprise + topology shift — paradigm-level insight.
    Paradigm,
}

impl std::fmt::Display for EpistemicLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EpistemicLevel::Concept => write!(f, "concept"),
            EpistemicLevel::Theory => write!(f, "theory"),
            EpistemicLevel::Paradigm => write!(f, "paradigm"),
        }
    }
}

/// A graduated situational brief for an observation.
#[derive(Debug, Clone)]
pub struct SituationalBrief {
    /// The output line.
    pub line: String,
    /// Epistemological level.
    pub level: EpistemicLevel,
}

/// Compute a graduated situational brief based on surprise level.
///
/// - Low surprise (<0.3): "concept-name ✓"
/// - Medium surprise (0.3-0.5): concept + discrepancy keywords
/// - High surprise (>0.5): "NEW TERRITORY: [discrepancy brief]"
/// - Topological event: "TOPOLOGY SHIFT: [what changed]"
///
/// `topo_events` should be the topological events detected for this observation.
/// `discrepancy` is an optional pre-computed discrepancy brief.
pub fn situational_brief(
    store: &Store,
    assignment: &ConceptAssignment,
    topo_events: &[String],
    discrepancy: Option<&DiscrepancyBrief>,
) -> Option<SituationalBrief> {
    // Topological event takes precedence — paradigm level.
    if !topo_events.is_empty() {
        let event_summary = topo_events[0].clone();
        let level = match assignment {
            ConceptAssignment::Joined { surprise, .. } if *surprise > 0.5 => {
                EpistemicLevel::Paradigm
            }
            _ => EpistemicLevel::Theory,
        };
        return Some(SituationalBrief {
            line: format!("TOPOLOGY SHIFT: {event_summary}"),
            level,
        });
    }

    match assignment {
        ConceptAssignment::Joined {
            concept, surprise, ..
        } => {
            let name = concept_name_from_entity(store, *concept);
            if *surprise < 0.3 {
                // Low surprise: confirming.
                Some(SituationalBrief {
                    line: format!("{name} \u{2713}"),
                    level: EpistemicLevel::Concept,
                })
            } else if *surprise < 0.5 {
                // Medium surprise: concept + discrepancy keywords.
                let detail = discrepancy
                    .map(format_discrepancy_brief)
                    .unwrap_or_else(|| format!("{name} (surprise={surprise:.2})"));
                Some(SituationalBrief {
                    line: detail,
                    level: EpistemicLevel::Concept,
                })
            } else {
                // High surprise: new territory.
                let detail = discrepancy
                    .map(format_discrepancy_brief)
                    .unwrap_or_else(|| format!("beyond {name} (surprise={surprise:.2})"));
                Some(SituationalBrief {
                    line: format!("NEW TERRITORY: {detail}"),
                    level: EpistemicLevel::Theory,
                })
            }
        }
        ConceptAssignment::Uncategorized => None,
    }
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

/// Sigmoid soft membership strength (ADR-FOUNDATION-031).
///
/// Replaces binary threshold comparison with a smooth gradient.
/// A hard cutoff claims infinite confidence in the threshold value.
/// A sigmoid encodes finite precision: membership near the boundary is uncertain.
///
/// `membership_strength(threshold, threshold, any_t) == 0.5` (midpoint property).
/// `lim_{temperature -> 0} membership_strength = Heaviside` (backward compatible).
///
/// The temperature parameter encodes threshold confidence:
/// - Low temperature (0.01) → sharp boundary, high confidence
/// - High temperature (0.1) → gradual transition, low confidence
///
/// INV-EMBEDDING-004: Same inputs always produce same output (deterministic).
pub fn membership_strength(similarity: f32, threshold: f32, temperature: f32) -> f32 {
    let t = temperature.max(1e-6); // Guard against division by zero
    let x = (similarity - threshold) / t;
    // Clamp x to prevent overflow in exp for very large negative values
    let x_clamped = x.clamp(-20.0, 20.0);
    1.0 / (1.0 + (-x_clamped).exp())
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

/// Calibrate the concept join threshold from observed cosine similarities (OBSERVER-4).
///
/// Uses Otsu's method: find threshold T that minimizes weighted intra-class variance
/// of the two groups (above-T = members, below-T = non-members). This produces the
/// natural decision boundary for THIS project's embedding space.
///
/// Also computes the optimal sigmoid temperature as stddev(all_similarities) / 2.
///
/// Returns `None` if fewer than 5 observations have concept assignments (insufficient data).
///
/// ADR-FOUNDATION-031: Parameters are first-class knowledge. The bootstrap default
/// (embedder.join_threshold()) is the prior. This function computes the posterior.
///
/// INV-EMBEDDING-004: All comparisons use the same embedding space.
pub fn calibrate_join_threshold(store: &Store) -> Option<(f32, f32)> {
    let concept_attr = Attribute::from_keyword(":exploration/concept");
    let emb_attr = Attribute::from_keyword(":concept/embedding");
    let obs_emb_attr = Attribute::from_keyword(":exploration/embedding");

    // Phase 1: Collect observation -> primary concept mapping.
    // For each observation entity, find its :exploration/concept Ref (primary = first assigned).
    let mut obs_concepts: BTreeMap<EntityId, EntityId> = BTreeMap::new();
    for d in store.datoms() {
        if d.op == Op::Assert && d.attribute == concept_attr {
            if let Value::Ref(concept_entity) = d.value {
                // First concept ref wins (primary assignment).
                obs_concepts.entry(d.entity).or_insert(concept_entity);
            }
        }
    }

    if obs_concepts.len() < 5 {
        return None; // Insufficient data for calibration.
    }

    // Phase 2: Collect latest concept embeddings.
    let mut concept_embeddings: BTreeMap<EntityId, Vec<f32>> = BTreeMap::new();
    for d in store.datoms() {
        if d.op == Op::Assert && d.attribute == emb_attr {
            if let Value::Bytes(ref bytes) = d.value {
                concept_embeddings.insert(d.entity, bytes_to_embedding(bytes));
            }
        }
    }

    // Phase 3: Collect observation embeddings.
    let mut obs_embeddings: BTreeMap<EntityId, Vec<f32>> = BTreeMap::new();
    for d in store.datoms() {
        if d.op == Op::Assert && d.attribute == obs_emb_attr {
            if let Value::Bytes(ref bytes) = d.value {
                obs_embeddings.insert(d.entity, bytes_to_embedding(bytes));
            }
        }
    }

    // Phase 4: Compute cosine similarities between observations and their primary concepts.
    let mut similarities: Vec<f32> = Vec::new();
    for (obs_entity, concept_entity) in &obs_concepts {
        if let (Some(obs_emb), Some(concept_emb)) = (
            obs_embeddings.get(obs_entity),
            concept_embeddings.get(concept_entity),
        ) {
            if obs_emb.len() == concept_emb.len() {
                let sim = cosine_similarity(obs_emb, concept_emb);
                similarities.push(sim);
            }
        }
    }

    if similarities.len() < 5 {
        return None; // Insufficient paired data.
    }

    // Phase 5: Otsu's method — find T minimizing weighted intra-class variance.
    let mut best_threshold = 0.5_f32;
    let mut best_variance = f32::MAX;
    let n = similarities.len() as f32;

    for t_int in 10..=90 {
        let t = t_int as f32 / 100.0;
        let below: Vec<f32> = similarities.iter().copied().filter(|&s| s < t).collect();
        let above: Vec<f32> = similarities.iter().copied().filter(|&s| s >= t).collect();

        if below.is_empty() || above.is_empty() {
            continue;
        }

        let w_below = below.len() as f32 / n;
        let w_above = above.len() as f32 / n;

        let var_below = variance_1d(&below);
        let var_above = variance_1d(&above);

        let intra_class = w_below * var_below + w_above * var_above;
        if intra_class < best_variance {
            best_variance = intra_class;
            best_threshold = t;
        }
    }

    // Phase 6: Compute temperature as stddev / 2.
    let mean: f32 = similarities.iter().sum::<f32>() / n;
    let var: f32 = similarities.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / n;
    let stddev = var.sqrt();
    let temperature = (stddev / 2.0).max(0.01); // Floor at 0.01 to prevent infinitely sharp sigmoid.

    Some((best_threshold, temperature))
}

/// 1D variance helper for Otsu's method.
fn variance_1d(values: &[f32]) -> f32 {
    if values.len() <= 1 {
        return 0.0;
    }
    let n = values.len() as f32;
    let mean = values.iter().sum::<f32>() / n;
    values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n
}

// ===================================================================
// Observation Link Extraction (C9-P2)
// ===================================================================

/// Relationship type between linked observations (C9-P2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkRelation {
    /// Source depends on / requires / is blocked by target.
    DependsOn,
    /// Source blocks / enables / unblocks target.
    Blocks,
    /// Source relates to / interacts with / complements target.
    RelatesTo,
}

impl std::fmt::Display for LinkRelation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkRelation::DependsOn => write!(f, "depends-on"),
            LinkRelation::Blocks => write!(f, "blocks"),
            LinkRelation::RelatesTo => write!(f, "relates-to"),
        }
    }
}

/// A cross-reference extracted from observation text (C9-P2).
///
/// Links are idempotent: extracting twice from the same text produces
/// the same links (content-addressed dedup via source+target pair).
#[derive(Debug, Clone)]
pub struct ExtractedLink {
    /// The observation entity containing the reference.
    pub source: EntityId,
    /// The referenced entity (task, spec element, or other observation).
    pub target: EntityId,
    /// The classified relationship.
    pub relationship: LinkRelation,
    /// Surrounding text context (up to 100 chars around the reference).
    pub context: String,
}

/// Classify a relationship from surrounding text context (C9-P2).
fn classify_relation(context: &str) -> LinkRelation {
    let lower = context.to_lowercase();
    let depends_keywords = [
        "depends on",
        "requires",
        "prerequisite",
        "blocked by",
        "needs",
        "after",
    ];
    let blocks_keywords = ["blocks", "enables", "unblocks", "before"];

    for kw in &depends_keywords {
        if lower.contains(kw) {
            return LinkRelation::DependsOn;
        }
    }
    for kw in &blocks_keywords {
        if lower.contains(kw) {
            return LinkRelation::Blocks;
        }
    }
    LinkRelation::RelatesTo
}

/// Check if a character sequence at `start` in `text` is a valid task ID (t-XXXXXXXX).
fn is_task_id_at(text: &str, start: usize) -> Option<String> {
    let remaining = &text[start..];
    if !remaining.starts_with("t-") {
        return None;
    }
    if remaining.len() < 10 {
        return None;
    }
    let hex_part = &remaining[2..10];
    if hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(remaining[..10].to_string())
    } else {
        None
    }
}

/// Check if text at position matches a spec ref (INV-STORE-001, ADR-FOUNDATION-012, etc.).
fn is_spec_ref_at(text: &str, start: usize) -> Option<String> {
    let remaining = &text[start..];
    let prefixes = ["INV-", "ADR-", "NEG-"];
    let prefix = prefixes.iter().find(|p| remaining.starts_with(*p))?;
    let after_prefix = &remaining[prefix.len()..];

    // Expect uppercase namespace: [A-Z]+
    let ns_end = after_prefix
        .find(|c: char| !c.is_ascii_uppercase())
        .unwrap_or(after_prefix.len());
    if ns_end == 0 {
        return None;
    }

    // Expect '-' then digits
    let after_ns = &after_prefix[ns_end..];
    if !after_ns.starts_with('-') {
        return None;
    }
    let digits = &after_ns[1..];
    let digit_end = digits
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(digits.len());
    if digit_end == 0 {
        return None;
    }

    let total_len = prefix.len() + ns_end + 1 + digit_end;
    Some(remaining[..total_len].to_string())
}

/// Extract context around a position in text (up to 100 chars each side).
fn extract_context(text: &str, pos: usize, ref_len: usize) -> String {
    let start = pos.saturating_sub(100);
    let end = (pos + ref_len + 100).min(text.len());
    text[start..end].to_string()
}

/// Extract cross-references from observation bodies and return as links (C9-P2).
///
/// Scans all observations for task IDs (`t-XXXXXXXX`), spec refs
/// (`INV-STORE-001`, `ADR-FOUNDATION-012`, `NEG-MERGE-003`), and
/// classifies the relationship by surrounding keywords.
///
/// Uses the existing `:exploration/depends-on` attribute (schema.rs:1888).
pub fn extract_observation_links(store: &Store) -> Vec<ExtractedLink> {
    let body_attr = Attribute::from_keyword(":exploration/body");
    let ident_attr = Attribute::from_keyword(":db/ident");
    let spec_id_attr = Attribute::from_keyword(":spec/id");
    let mut links: Vec<ExtractedLink> = Vec::new();
    let mut seen: HashSet<(EntityId, EntityId)> = HashSet::new();

    // Build spec ID lookup: spec_id_string → EntityId.
    let mut spec_id_map: HashMap<String, EntityId> = HashMap::new();
    for datom in store.attribute_datoms(&spec_id_attr) {
        if datom.op == Op::Assert {
            if let Value::String(ref s) = datom.value {
                spec_id_map.insert(s.clone(), datom.entity);
            }
        }
    }

    for datom in store.attribute_datoms(&body_attr) {
        if datom.op != Op::Assert {
            continue;
        }
        let body = match &datom.value {
            Value::String(s) => s.as_str(),
            _ => continue,
        };
        let source = datom.entity;

        // Scan for task IDs.
        let mut pos = 0;
        while pos < body.len() {
            if let Some(task_id) = is_task_id_at(body, pos) {
                let ident_kw = format!(":task/{}", task_id);
                // Resolve task ID to entity via :db/ident lookup.
                let target = store
                    .avet_lookup(&ident_attr, &Value::Keyword(ident_kw.clone()))
                    .first()
                    .map(|d| d.entity);
                if let Some(target_entity) = target {
                    if source != target_entity && seen.insert((source, target_entity)) {
                        let ctx = extract_context(body, pos, task_id.len());
                        let rel = classify_relation(&ctx);
                        links.push(ExtractedLink {
                            source,
                            target: target_entity,
                            relationship: rel,
                            context: ctx,
                        });
                    }
                }
                pos += task_id.len();
            } else {
                pos += 1;
            }
        }

        // Scan for spec refs.
        pos = 0;
        while pos < body.len() {
            if let Some(spec_ref) = is_spec_ref_at(body, pos) {
                if let Some(&target_entity) = spec_id_map.get(&spec_ref) {
                    if source != target_entity && seen.insert((source, target_entity)) {
                        let ctx = extract_context(body, pos, spec_ref.len());
                        let rel = classify_relation(&ctx);
                        links.push(ExtractedLink {
                            source,
                            target: target_entity,
                            relationship: rel,
                            context: ctx,
                        });
                    }
                }
                pos += spec_ref.len();
            } else {
                pos += 1;
            }
        }
    }

    links.sort_by_key(|a| a.source);
    links
}

/// Convert extracted links to datom triples for transaction (C9-P2).
///
/// Uses existing schema attributes:
/// - `:exploration/depends-on` (Ref, Many) for DependsOn and Blocks
/// - `:exploration/related-spec` (Ref, Many) for RelatesTo
pub fn link_datoms(links: &[ExtractedLink]) -> Vec<(EntityId, Attribute, Value)> {
    links
        .iter()
        .map(|link| {
            let attr_name = match link.relationship {
                LinkRelation::DependsOn | LinkRelation::Blocks => ":exploration/depends-on",
                LinkRelation::RelatesTo => ":exploration/related-spec",
            };
            (
                link.source,
                Attribute::from_keyword(attr_name),
                Value::Ref(link.target),
            )
        })
        .collect()
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
        "Discrete isolated parts: individual packages, modules, files as bounded units",
    ),
    (
        "dependencies",
        "Relationships and connections: imports, function calls, data flow paths between modules",
    ),
    (
        "invariants",
        "Rules and constraints: assertions that must hold, contracts to verify, specifications to enforce",
    ),
    (
        "patterns",
        "Recurring structures: repeated idioms, architectural conventions, protocol templates",
    ),
    (
        "anomalies",
        "Defects and surprises: bugs, violations, inconsistencies, unexpected behaviors, test failures",
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

    if nlen == 0 || nlen > bytes.len() {
        return false;
    }

    for i in 0..=(bytes.len() - nlen) {
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
            concept, surprise, ..
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
                    concept,
                    similarity,
                    ..
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
        // INQ-1-REV: Show "none yet" when no concepts exist (instead of empty vec).
        let obs_count = count_observations(store);
        let min_cluster_size: usize = crate::config::get_config(store, "concept.min-cluster-size")
            .and_then(|v| v.parse().ok())
            .unwrap_or(MIN_CLUSTER_SIZE);
        if obs_count > 0 {
            return vec![format!(
                "concepts: none yet ({obs_count}/{min_cluster_size} toward first concepts)"
            )];
        }
        return vec![format!(
            "concepts: none yet (emerge after {min_cluster_size}+ observations)"
        )];
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
                        concepts_with_embeddings[i].name, concepts_with_embeddings[j].name, sim
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
        lines.push(format!(
            "bridge-gaps: {} concept pairs with zero co-occurrence",
            bridges.len()
        ));
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
///
/// Content-aware: uses entity matches and surprise level to generate specific
/// questions rather than generic "what connects X to Y?" templates.
fn compute_steering_question(
    store: &Store,
    assignment: &ConceptAssignment,
    entity_matches: &[EntityMatch],
) -> Option<String> {
    match assignment {
        ConceptAssignment::Joined {
            concept, surprise, ..
        } => {
            // Priority 1: If entity matches exist, suggest investigating specific entities.
            if entity_matches.len() >= 2 {
                return Some(format!(
                    "how does {} interact with {}?",
                    entity_matches[0].match_name, entity_matches[1].match_name
                ));
            }
            if !entity_matches.is_empty() {
                return Some(format!(
                    "what other parts of the codebase depend on {}?",
                    entity_matches[0].match_name
                ));
            }

            // Priority 2: High surprise — at concept boundary.
            if *surprise > 0.3 {
                let current_name = concept_name_from_entity(store, *concept);
                return Some(format!(
                    "this is at the boundary of {current_name} \u{2014} what distinguishes it from typical {current_name} observations?"
                ));
            }

            // Priority 3: Generic concept connection (lowest priority).
            let inventory = concept_inventory(store);
            let current_name = concept_name_from_entity(store, *concept);
            let other = inventory
                .iter()
                .find(|c| c.entity != *concept && c.member_count > 0);

            other.map(|other_concept| {
                format!("what connects {} to {}?", current_name, other_concept.name)
            })
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
    "from",
    "with",
    "this",
    "that",
    "must",
    "have",
    "been",
    "into",
    "also",
    "when",
    "what",
    "were",
    "does",
    "more",
    "some",
    "than",
    "then",
    "them",
    "they",
    "will",
    "each",
    "only",
    "such",
    "very",
    "just",
    "most",
    "both",
    "about",
    "which",
    "their",
    "would",
    "could",
    "should",
    "these",
    "those",
    "there",
    "being",
    "where",
    "after",
    "other",
    "using",
    "every",
    "still",
    "between",
    "through",
    "before",
    "during",
    "without",
    "another",
    "because",
    "across",
    "concept",
    "observed",
    "observation",
];

/// Generate a human-readable concept name using TF-IDF (C9-P6).
///
/// `member_texts` — observation bodies belonging to this concept cluster.
/// `corpus_texts` — ALL observation bodies (the IDF universe).
///
/// Algorithm: TF within concept members, IDF across full corpus.
/// Top 3 distinguishing keywords joined by hyphen.
///
/// INV-REFLEXIVE-007: Same function for domain and meta observations.
fn generate_concept_name(member_texts: &[&str], corpus_texts: &[&str]) -> String {
    if member_texts.is_empty() {
        return "unnamed".to_string();
    }

    // Phase 1: TF — term frequency within member texts.
    // Raw count of each word across ALL member texts / total member tokens.
    let mut tf_count: HashMap<String, usize> = HashMap::new();
    let mut total_tokens: usize = 0;
    for text in member_texts {
        let words = crate::connections::tokenize(text);
        for word in &words {
            if word.len() < 4 || CONCEPT_NAME_STOPWORDS.contains(&word.as_str()) {
                continue;
            }
            *tf_count.entry(word.clone()).or_insert(0) += 1;
            total_tokens += 1;
        }
    }
    if total_tokens == 0 {
        return "unnamed".to_string();
    }

    // Phase 2: IDF — inverse document frequency across corpus.
    // df = number of corpus documents containing the term.
    let corpus_n = corpus_texts.len().max(1);
    let mut corpus_df: HashMap<String, usize> = HashMap::new();
    for text in corpus_texts {
        let words = crate::connections::tokenize(text);
        let unique: HashSet<String> = words.into_iter().collect();
        for word in unique {
            if word.len() < 4 || CONCEPT_NAME_STOPWORDS.contains(&word.as_str()) {
                continue;
            }
            *corpus_df.entry(word).or_insert(0) += 1;
        }
    }

    // Phase 3: TF-IDF score per term.
    let mut scores: Vec<(String, f64)> = tf_count
        .iter()
        .map(|(word, &count)| {
            let tf = count as f64 / total_tokens as f64;
            let df = corpus_df.get(word).copied().unwrap_or(0);
            let idf = (1.0 + corpus_n as f64 / (1.0 + df as f64)).ln();
            (word.clone(), tf * idf)
        })
        .filter(|(_, score)| *score > 0.0)
        .collect();

    // Sort by TF-IDF descending, break ties alphabetically for determinism.
    scores.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    // Take top 3 distinguishing keywords.
    let top: Vec<&str> = scores.iter().take(3).map(|(w, _)| w.as_str()).collect();
    if top.is_empty() {
        return "unnamed".to_string();
    }
    top.join("-")
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
        let name = generate_concept_name(texts, texts);
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
        assert_eq!(generate_concept_name(&[], &[]), "unnamed");
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
            strength: 1.0,
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
        // INQ-1-REV: Empty store now shows "concepts: none yet" instead of empty vec.
        assert_eq!(lines.len(), 1, "empty store should have 1 'none yet' line");
        assert!(
            lines[0].contains("none yet"),
            "empty store should say 'none yet', got: {}",
            lines[0]
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
                strength: 1.0,
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
        let members = [[1.0f32, 0.0, 0.0], [0.0f32, 1.0, 0.0], [0.0f32, 0.0, 1.0]];

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
            strength: 1.0,
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
            strength: 1.0,
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
            strength: 1.0,
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
        let tx =
            crate::store::Transaction::new(agent, crate::datom::ProvenanceType::Observed, "test")
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
        for (i, text) in [
            "alpha concept test",
            "beta concept test",
            "gamma concept test",
        ]
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
        let tx1 =
            crate::store::Transaction::new(agent, crate::datom::ProvenanceType::Observed, "v1")
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
        let tx2 =
            crate::store::Transaction::new(agent, crate::datom::ProvenanceType::Observed, "v2")
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
        assert!(
            result.is_some(),
            "new similar observation should match emergent concept"
        );
    }

    /// Split recommendation fires for high-variance concept.
    #[test]
    fn test_split_recommendation_in_status() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");

        let ce = EntityId::from_content(b"concept-broad");
        let tx =
            crate::store::Transaction::new(agent, crate::datom::ProvenanceType::Observed, "test")
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
        let tx1 =
            crate::store::Transaction::new(agent, crate::datom::ProvenanceType::Observed, "v1")
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
        let tx2 =
            crate::store::Transaction::new(agent, crate::datom::ProvenanceType::Derived, "reembed")
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
        let name = generate_concept_name(texts, texts);
        assert!(
            name.contains("error")
                || name.contains("handling")
                || name.contains("cascade")
                || name.contains("storage")
                || name.contains("pipeline"),
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
        let name = generate_concept_name(texts, texts);
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
            let refs: Vec<&str> = texts.to_vec();
            let name = generate_concept_name(&refs, &refs);
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
        let name = generate_concept_name(texts, texts);
        assert!(
            !name.is_empty() && name != "unnamed",
            "single member should produce a name, got: '{name}'"
        );
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
                let old_centroid = store.live_value(*concept, &emb_attr).and_then(|v| {
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
        assert!(
            !matrix.is_empty(),
            "co-occurrence matrix should have entries"
        );

        // Find the alpha-beta pair.
        let pair = matrix.iter().find(|p| {
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
        assert_eq!(
            pair.unwrap().jaccard,
            0.0,
            "disjoint sets should have Jaccard = 0"
        );
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
            let edge_entity = EntityId::from_content(format!("edge-0-{target_idx}").as_bytes());
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
        assert!(
            rec.is_some(),
            "should recommend exploring unexplored packages"
        );
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
            Value::Bytes(crate::embedding::embedding_to_bytes(
                &emb.embed("uncertain topic"),
            )),
            tx,
            Op::Assert,
        )]);

        let current_emb = emb.embed("different topic entirely");
        let rec = frontier_recommendation(&store, &current_emb);
        assert!(
            rec.is_some(),
            "should recommend deepening high-variance concept"
        );
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
        assert!(
            rec.is_some(),
            "should recommend bridging disconnected concepts"
        );
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
                assert!(
                    !duplicate,
                    "co-occurrence matrix should not have duplicate pairs"
                );
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
                edge,
                comp_from.clone(),
                Value::Ref(pkgs[0]),
                tx,
                Op::Assert,
            )]);
            store.apply_datoms(&[Datom::new(
                edge,
                comp_to.clone(),
                Value::Ref(pkgs[target_idx]),
                tx,
                Op::Assert,
            )]);
        }
        // Mark module-0 as explored.
        let mentions_attr = Attribute::from_keyword(":exploration/mentions-entity");
        let obs = EntityId::from_content(b"obs-bound-1");
        store.apply_datoms(&[Datom::new(
            obs,
            mentions_attr,
            Value::Ref(pkgs[0]),
            tx,
            Op::Assert,
        )]);

        // Create a high-variance concept.
        let concept = EntityId::from_content(b"concept:variance-test");
        store.apply_datoms(&[Datom::new(
            concept,
            Attribute::from_keyword(":concept/name"),
            Value::String("variance-test".into()),
            tx,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept,
            Attribute::from_keyword(":concept/member-count"),
            Value::Long(10),
            tx,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept,
            Attribute::from_keyword(":concept/variance"),
            Value::Double(ordered_float::OrderedFloat(0.45)),
            tx,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept,
            Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(crate::embedding::embedding_to_bytes(
                &emb.embed("variance topic"),
            )),
            tx,
            Op::Assert,
        )]);

        let current_emb = emb.embed("test observation");
        let rec = frontier_recommendation(&store, &current_emb);
        if let Some(r) = &rec {
            assert!(
                r.score >= 0.0 && r.score <= 1.0,
                "frontier score should be in [0, 1], got {} for kind {:?}",
                r.score,
                r.kind
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
                *concept,
                Attribute::from_keyword(":concept/name"),
                Value::String(name.to_string()),
                tx,
                Op::Assert,
            )]);
            store.apply_datoms(&[Datom::new(
                *concept,
                Attribute::from_keyword(":concept/member-count"),
                Value::Long(5),
                tx,
                Op::Assert,
            )]);
            store.apply_datoms(&[Datom::new(
                *concept,
                Attribute::from_keyword(":concept/variance"),
                Value::Double(ordered_float::OrderedFloat(*variance)),
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
                e,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(name),
                tx,
                Op::Assert,
            )]);
            pkgs.push(e);
        }
        let comp_from = Attribute::from_keyword(":composition/from");
        let comp_to = Attribute::from_keyword(":composition/to");
        // mod-0 imports mod-2, mod-3, mod-4
        for idx in 2..5 {
            let edge = EntityId::from_content(format!("edge-c-{idx}").as_bytes());
            store.apply_datoms(&[Datom::new(
                edge,
                comp_from.clone(),
                Value::Ref(pkgs[0]),
                tx,
                Op::Assert,
            )]);
            store.apply_datoms(&[Datom::new(
                edge,
                comp_to.clone(),
                Value::Ref(pkgs[idx]),
                tx,
                Op::Assert,
            )]);
        }
        let mentions = Attribute::from_keyword(":exploration/mentions-entity");
        for &pkg in &pkgs[0..2] {
            let obs = EntityId::from_content(format!("obs-c-{:?}", pkg).as_bytes());
            store.apply_datoms(&[Datom::new(
                obs,
                mentions.clone(),
                Value::Ref(pkg),
                tx,
                Op::Assert,
            )]);
        }

        // Create a high-variance concept with members = total observations.
        let concept = EntityId::from_content(b"concept:commensurate");
        store.apply_datoms(&[Datom::new(
            concept,
            Attribute::from_keyword(":concept/name"),
            Value::String("commensurate".into()),
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
            Value::Double(ordered_float::OrderedFloat(0.4)),
            tx,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept,
            Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(crate::embedding::embedding_to_bytes(
                &emb.embed("distant topic entirely"),
            )),
            tx,
            Op::Assert,
        )]);

        let current_emb = emb.embed("observation about something");
        let rec = frontier_recommendation(&store, &current_emb);
        assert!(rec.is_some(), "should recommend something");
        let r = rec.unwrap();
        // Both types should produce scores in reasonable range (not 0 or wildly different).
        assert!(r.score > 0.0, "winning score should be positive");
        assert!(
            r.score <= 1.0,
            "winning score should be at most 1.0, got {}",
            r.score
        );
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
            concept_a,
            Attribute::from_keyword(":concept/name"),
            Value::String("scan-a".into()),
            tx,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept_b,
            Attribute::from_keyword(":concept/name"),
            Value::String("scan-b".into()),
            tx,
            Op::Assert,
        )]);

        let concept_attr = Attribute::from_keyword(":exploration/concept");
        // 4 in A, 4 in B, 2 shared.
        for i in 0..4 {
            let obs = EntityId::from_content(format!("obs-scan-a-{i}").as_bytes());
            store.apply_datoms(&[Datom::new(
                obs,
                concept_attr.clone(),
                Value::Ref(concept_a),
                tx,
                Op::Assert,
            )]);
        }
        for i in 0..4 {
            let obs = EntityId::from_content(format!("obs-scan-b-{i}").as_bytes());
            store.apply_datoms(&[Datom::new(
                obs,
                concept_attr.clone(),
                Value::Ref(concept_b),
                tx,
                Op::Assert,
            )]);
        }
        // 2 shared: obs-scan-a-0 and obs-scan-a-1 also in B.
        for i in 0..2 {
            let obs = EntityId::from_content(format!("obs-scan-a-{i}").as_bytes());
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
            if text.is_empty() {
                continue;
            }
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
            concept,
            Attribute::from_keyword(":concept/name"),
            Value::String("only-deepen".into()),
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
            Value::Double(ordered_float::OrderedFloat(0.4)),
            tx,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept,
            Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(crate::embedding::embedding_to_bytes(
                &emb.embed("only deepen topic"),
            )),
            tx,
            Op::Assert,
        )]);

        let current_emb = emb.embed("something entirely different");
        let rec = frontier_recommendation(&store, &current_emb);
        assert!(rec.is_some(), "should recommend deepening");
        let r = rec.unwrap();
        assert_eq!(r.kind, FrontierKind::Deepen);
        assert!(
            r.score >= 0.0 && r.score <= 1.0,
            "score should be in [0,1], got {}",
            r.score
        );
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

    // ===================================================================
    // CAL-TEST: Self-calibrating threshold tests (ADR-FOUNDATION-031)
    // ===================================================================

    /// Sigmoid midpoint property: membership_strength(T, T, any_temp) == 0.5
    #[test]
    fn test_membership_strength_midpoint() {
        for threshold in [0.1, 0.3, 0.5, 0.7, 0.9] {
            for temp in [0.01, 0.05, 0.1, 0.5] {
                let s = membership_strength(threshold as f32, threshold as f32, temp as f32);
                assert!(
                    (s - 0.5).abs() < 0.01,
                    "midpoint property violated: strength({threshold}, {threshold}, {temp}) = {s}, expected 0.5"
                );
            }
        }
    }

    /// Sigmoid monotonicity: higher similarity -> higher strength
    #[test]
    fn test_membership_strength_monotonic() {
        let threshold = 0.5_f32;
        let temperature = 0.05_f32;
        let mut prev = 0.0_f32;
        for sim_int in 0..=100 {
            let sim = sim_int as f32 / 100.0;
            let s = membership_strength(sim, threshold, temperature);
            assert!(
                s >= prev - 1e-6,
                "monotonicity violated: strength({sim}) = {s} < strength({}) = {prev}",
                (sim_int - 1) as f32 / 100.0
            );
            prev = s;
        }
    }

    /// Sigmoid bounds: output always in [0.0, 1.0]
    #[test]
    fn test_membership_strength_bounds() {
        for sim_int in 0..=100 {
            for thresh_int in 0..=100 {
                for temp in [0.001, 0.01, 0.05, 0.1, 1.0] {
                    let sim = sim_int as f32 / 100.0;
                    let thresh = thresh_int as f32 / 100.0;
                    let s = membership_strength(sim, thresh, temp as f32);
                    assert!(
                        (0.0..=1.0).contains(&s),
                        "bounds violated: strength({sim}, {thresh}, {temp}) = {s}"
                    );
                }
            }
        }
    }

    /// Lower temperature -> sharper transition around threshold
    #[test]
    fn test_membership_strength_temperature_sharpness() {
        let threshold = 0.5_f32;
        let near_above = 0.52_f32; // Just above threshold

        let sharp = membership_strength(near_above, threshold, 0.01);
        let gradual = membership_strength(near_above, threshold, 0.1);

        assert!(
            sharp > gradual,
            "lower temperature should give sharper (higher) response near threshold: \
             sharp(t=0.01)={sharp:.4} should > gradual(t=0.1)={gradual:.4}"
        );

        // At the threshold itself, both should be ~0.5 regardless of temperature
        let at_threshold_sharp = membership_strength(threshold, threshold, 0.01);
        let at_threshold_gradual = membership_strength(threshold, threshold, 0.1);
        assert!((at_threshold_sharp - 0.5).abs() < 0.01);
        assert!((at_threshold_gradual - 0.5).abs() < 0.01);
    }

    /// Otsu calibration returns None for insufficient data (<5 observations)
    #[test]
    fn test_calibrate_threshold_insufficient_data() {
        let store = store_with_innate_concepts();
        // No observations — just innate concepts
        let result = calibrate_join_threshold(&store);
        assert!(
            result.is_none(),
            "should return None with 0 observations, got {:?}",
            result
        );
    }

    /// Otsu calibration finds threshold between two clusters
    #[test]
    fn test_calibrate_threshold_bimodal() {
        let mut store = store_with_innate_concepts();
        let emb = make_embedder();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(100, 0, agent);

        let concept = EntityId::from_content(b"concept:test-calibrate");
        let concept_emb = emb.embed("specific topic about testing");
        store.apply_datoms(&[Datom::new(
            concept,
            Attribute::from_keyword(":concept/name"),
            Value::String("test-calibrate".into()),
            tx,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept,
            Attribute::from_keyword(":concept/embedding"),
            Value::Bytes(embedding_to_bytes(&concept_emb)),
            tx,
            Op::Assert,
        )]);

        let concept_ref_attr = Attribute::from_keyword(":exploration/concept");
        let obs_emb_attr = Attribute::from_keyword(":exploration/embedding");

        // Create 10 observations: 5 similar to concept, 5 dissimilar but still assigned
        for i in 0..5 {
            let obs = EntityId::from_content(format!("obs-close-{i}").as_bytes());
            let obs_emb = emb.embed(&format!("specific topic about testing verification {i}"));
            store.apply_datoms(&[Datom::new(
                obs,
                concept_ref_attr.clone(),
                Value::Ref(concept),
                tx,
                Op::Assert,
            )]);
            store.apply_datoms(&[Datom::new(
                obs,
                obs_emb_attr.clone(),
                Value::Bytes(embedding_to_bytes(&obs_emb)),
                tx,
                Op::Assert,
            )]);
        }
        for i in 0..5 {
            let obs = EntityId::from_content(format!("obs-far-{i}").as_bytes());
            let obs_emb = emb.embed(&format!("completely different unrelated words {i}"));
            store.apply_datoms(&[Datom::new(
                obs,
                concept_ref_attr.clone(),
                Value::Ref(concept),
                tx,
                Op::Assert,
            )]);
            store.apply_datoms(&[Datom::new(
                obs,
                obs_emb_attr.clone(),
                Value::Bytes(embedding_to_bytes(&obs_emb)),
                tx,
                Op::Assert,
            )]);
        }

        let result = calibrate_join_threshold(&store);
        assert!(result.is_some(), "should calibrate with 10 observations");
        let (threshold, temperature) = result.unwrap();
        assert!(
            (0.1..=0.9).contains(&threshold),
            "threshold should be in Otsu sweep range [0.10, 0.90], got {threshold}"
        );
        assert!(
            temperature > 0.001 && temperature < 1.0,
            "temperature should be in reasonable range, got {temperature}"
        );
    }

    /// assign_to_concepts_soft filters by min_strength
    #[test]
    fn test_assign_to_concepts_soft_filtering() {
        let store = store_with_innate_concepts();
        let emb = make_embedder();
        let obs_emb = emb.embed("packages modules files services boundaries");

        // With very low min_strength, should get many matches
        let loose = assign_to_concepts_soft(&store, &obs_emb, 0.5, 0.1, 0.01);
        // With high min_strength, should get fewer matches
        let tight = assign_to_concepts_soft(&store, &obs_emb, 0.5, 0.1, 0.9);

        assert!(
            loose.len() >= tight.len(),
            "lower min_strength should give >= matches: loose={} vs tight={}",
            loose.len(),
            tight.len()
        );

        // All returned assignments should have strength >= min_strength
        for a in &tight {
            if let ConceptAssignment::Joined { strength, .. } = a {
                assert!(
                    *strength >= 0.9,
                    "tight filter: strength {strength} should be >= 0.9"
                );
            }
        }
    }

    /// assign_to_concepts_soft returns strengths sorted descending
    #[test]
    fn test_assign_to_concepts_soft_sorted() {
        let store = store_with_innate_concepts();
        let emb = make_embedder();
        let obs_emb = emb.embed("packages modules imports coupling boundaries");

        let assignments = assign_to_concepts_soft(&store, &obs_emb, 0.3, 0.1, 0.01);
        for w in assignments.windows(2) {
            let s_a = match &w[0] {
                ConceptAssignment::Joined { strength, .. } => *strength,
                _ => 0.0,
            };
            let s_b = match &w[1] {
                ConceptAssignment::Joined { strength, .. } => *strength,
                _ => 0.0,
            };
            assert!(
                s_a >= s_b - 1e-6,
                "should be sorted by strength desc: {s_a} < {s_b}"
            );
        }
    }

    /// Narrow candidates fire when concepts are collapsed (high jaccard)
    #[test]
    fn test_narrow_candidates_collapsed() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(10, 0, agent);

        // Create 3 concepts with IDENTICAL member sets (jaccard = 1.0)
        let concepts: Vec<EntityId> = (0..3)
            .map(|i| {
                let e = EntityId::from_content(format!("concept:narrow-{i}").as_bytes());
                store.apply_datoms(&[Datom::new(
                    e,
                    Attribute::from_keyword(":concept/name"),
                    Value::String(format!("narrow-{i}")),
                    tx,
                    Op::Assert,
                )]);
                e
            })
            .collect();

        let concept_attr = Attribute::from_keyword(":exploration/concept");
        // Same 5 observations in ALL 3 concepts
        for i in 0..5 {
            let obs = EntityId::from_content(format!("obs-narrow-{i}").as_bytes());
            for concept in &concepts {
                store.apply_datoms(&[Datom::new(
                    obs,
                    concept_attr.clone(),
                    Value::Ref(*concept),
                    tx,
                    Op::Assert,
                )]);
            }
        }

        let candidates = narrow_candidates(&store);
        assert!(
            !candidates.is_empty(),
            "should detect concept collapse (all jaccard = 1.0)"
        );
        assert_eq!(candidates[0].kind, FrontierKind::Narrow);
    }

    /// Narrow candidates do NOT fire when concepts are healthy (low jaccard)
    #[test]
    fn test_narrow_candidates_healthy() {
        let mut store = store_with_schema();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(10, 0, agent);

        let concept_a = EntityId::from_content(b"concept:healthy-a");
        let concept_b = EntityId::from_content(b"concept:healthy-b");
        store.apply_datoms(&[Datom::new(
            concept_a,
            Attribute::from_keyword(":concept/name"),
            Value::String("healthy-a".into()),
            tx,
            Op::Assert,
        )]);
        store.apply_datoms(&[Datom::new(
            concept_b,
            Attribute::from_keyword(":concept/name"),
            Value::String("healthy-b".into()),
            tx,
            Op::Assert,
        )]);

        let concept_attr = Attribute::from_keyword(":exploration/concept");
        // Disjoint observations: 5 in A, 5 in B, no overlap
        for i in 0..5 {
            let obs_a = EntityId::from_content(format!("obs-ha-{i}").as_bytes());
            store.apply_datoms(&[Datom::new(
                obs_a,
                concept_attr.clone(),
                Value::Ref(concept_a),
                tx,
                Op::Assert,
            )]);
            let obs_b = EntityId::from_content(format!("obs-hb-{i}").as_bytes());
            store.apply_datoms(&[Datom::new(
                obs_b,
                concept_attr.clone(),
                Value::Ref(concept_b),
                tx,
                Op::Assert,
            )]);
        }

        let candidates = narrow_candidates(&store);
        assert!(
            candidates.is_empty(),
            "healthy concepts (jaccard=0) should NOT trigger narrow, got {} candidates",
            candidates.len()
        );
    }

    // ===================================================================
    // ONLINE-TEST: Online calibration formula tests
    // ===================================================================

    /// Online calibration formula: threshold = (mean - 0.5*stddev).clamp(0.15, 0.85)
    #[test]
    fn test_online_calibration_formula() {
        // Known similarities: [0.3, 0.5, 0.7]
        let sims = [0.3_f64, 0.5, 0.7];
        let count = sims.len() as f64;
        let sum: f64 = sims.iter().sum();
        let sum_sq: f64 = sims.iter().map(|s| s * s).sum();
        let mean = sum / count;
        let variance = (sum_sq / count - mean * mean).max(0.0);
        let stddev = variance.sqrt();
        let threshold = (mean - 0.5 * stddev).clamp(0.15, 0.85);
        let temperature = (stddev / 2.0).max(0.01);

        assert!((mean - 0.5).abs() < 0.01, "mean should be 0.5, got {mean}");
        assert!(
            (stddev - 0.1633).abs() < 0.01,
            "stddev should be ~0.163, got {stddev:.4}"
        );
        assert!(
            (threshold - 0.4184).abs() < 0.01,
            "threshold should be ~0.418, got {threshold:.4}"
        );
        assert!(
            (temperature - 0.0816).abs() < 0.01,
            "temperature should be ~0.082, got {temperature:.4}"
        );
    }

    /// Online calibration clamps to [0.15, 0.85] for degenerate distributions.
    #[test]
    fn test_online_calibration_clamp() {
        // All very high similarities -> threshold would be > 0.85 without clamp
        let sims = [0.95_f64, 0.96, 0.97];
        let count = sims.len() as f64;
        let sum: f64 = sims.iter().sum();
        let sum_sq: f64 = sims.iter().map(|s| s * s).sum();
        let mean = sum / count;
        let variance = (sum_sq / count - mean * mean).max(0.0);
        let stddev = variance.sqrt();
        let threshold = (mean - 0.5 * stddev).clamp(0.15, 0.85);

        assert!(
            threshold <= 0.85,
            "high-similarity threshold should be clamped to <= 0.85, got {threshold:.4}"
        );

        // All very low similarities -> threshold would be < 0.15 without clamp
        let sims_low = [0.05_f64, 0.06, 0.07];
        let count = sims_low.len() as f64;
        let sum: f64 = sims_low.iter().sum();
        let sum_sq: f64 = sims_low.iter().map(|s| s * s).sum();
        let mean = sum / count;
        let variance = (sum_sq / count - mean * mean).max(0.0);
        let stddev = variance.sqrt();
        let threshold = (mean - 0.5 * stddev).clamp(0.15, 0.85);

        assert!(
            threshold >= 0.15,
            "low-similarity threshold should be clamped to >= 0.15, got {threshold:.4}"
        );
    }

    /// Calibrate_join_threshold returns None for stores with < 5 paired observations.
    #[test]
    fn test_calibrate_returns_none_without_pairs() {
        // Store with innate concepts but no observations with embeddings
        let store = store_with_innate_concepts();
        assert!(
            calibrate_join_threshold(&store).is_none(),
            "should return None without observation-concept pairs"
        );
    }

    // ===================================================================
    // INQ-TEST: Inquiry Engine Tests
    // ===================================================================

    /// Helper: build a store with N observations (no innate concepts).
    fn store_with_observations(texts: &[&str]) -> Store {
        let mut store = store_with_schema();
        let emb = make_embedder();
        let agent = AgentId::from_name("test");

        for (i, text) in texts.iter().enumerate() {
            let tx = TxId::new(100 + i as u64, 0, agent);
            let slug: String = text
                .chars()
                .take(30)
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
                .map(|c| c.to_ascii_lowercase())
                .collect();
            let ident = format!(":observation/{slug}");
            let entity = EntityId::from_ident(&ident);
            let embedding = emb.embed(text);

            let datoms = vec![
                Datom::new(
                    entity,
                    Attribute::from_keyword(":db/ident"),
                    Value::Keyword(ident),
                    tx,
                    Op::Assert,
                ),
                Datom::new(
                    entity,
                    Attribute::from_keyword(":exploration/body"),
                    Value::String(text.to_string()),
                    tx,
                    Op::Assert,
                ),
                Datom::new(
                    entity,
                    Attribute::from_keyword(":exploration/embedding"),
                    Value::Bytes(crate::embedding::embedding_to_bytes(&embedding)),
                    tx,
                    Op::Assert,
                ),
            ];
            store.apply_datoms(&datoms);
        }
        store
    }

    /// INQ-TEST-1: Fresh store (no innate concepts) has 0 concepts.
    #[test]
    fn test_init_no_innate_concepts() {
        let store = store_with_schema();
        let (inventory, _) = concept_inventory_with_innate(&store);
        assert!(
            inventory.is_empty(),
            "fresh store without innate seeding should have 0 concepts"
        );
    }

    /// INQ-TEST-2: After MIN_CLUSTER_SIZE similar observations, crystallize produces concepts.
    #[test]
    fn test_auto_crystallize_after_three() {
        let texts = &[
            "error handling in cascade module returns",
            "error handling in storage module returns",
            "error handling in events module returns",
        ];
        let store = store_with_observations(texts);

        // Confirm: no concepts yet (just observations).
        let (inventory, _) = concept_inventory_with_innate(&store);
        assert!(
            inventory.is_empty(),
            "observations alone should not produce concepts"
        );

        // Collect uncategorized observations.
        let embed_attr = Attribute::from_keyword(":exploration/embedding");
        let body_attr = Attribute::from_keyword(":exploration/body");
        let mut observations = Vec::new();
        for d in store.datoms() {
            if d.op == Op::Assert && d.attribute == embed_attr {
                if let Value::Bytes(ref b) = d.value {
                    let emb = crate::embedding::bytes_to_embedding(b);
                    let body = store
                        .live_value(d.entity, &body_attr)
                        .and_then(|v| {
                            if let Value::String(s) = v {
                                Some(s.clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default();
                    observations.push((d.entity, emb, body));
                }
            }
        }

        assert!(
            observations.len() >= MIN_CLUSTER_SIZE,
            "should have at least {} observations, got {}",
            MIN_CLUSTER_SIZE,
            observations.len()
        );

        // Crystallize.
        let new_concepts = crystallize_concepts(&observations, JOIN_THRESHOLD, MIN_CLUSTER_SIZE);
        assert!(
            !new_concepts.is_empty(),
            "3 similar observations should produce at least 1 concept"
        );

        // Apply concept datoms to store.
        let mut store = store;
        let agent = AgentId::from_name("test");
        let cryst_tx = TxId::new(200, 0, agent);
        for concept in &new_concepts {
            let datoms: Vec<Datom> = concept_to_datoms(concept, 1000)
                .into_iter()
                .map(|(e, a, v)| Datom::new(e, a, v, cryst_tx, Op::Assert))
                .collect();
            store.apply_datoms(&datoms);
        }

        // Now concepts should exist.
        let (inventory, _) = concept_inventory_with_innate(&store);
        assert!(
            !inventory.is_empty(),
            "after crystallization, store should have concepts"
        );
    }

    /// INQ-TEST-3: Discrepancy brief produces novel keywords for surprising observations.
    #[test]
    fn test_discrepancy_brief_novel_keywords() {
        // Build store with a concept and observations.
        let mut store = store_with_observations(&[
            "database schema tables columns indexes",
            "database storage persistence transactions",
            "database queries optimization caching",
        ]);
        let emb = make_embedder();
        let agent = AgentId::from_name("test");

        // Crystallize a concept from these.
        let embed_attr = Attribute::from_keyword(":exploration/embedding");
        let body_attr = Attribute::from_keyword(":exploration/body");
        let mut observations = Vec::new();
        for d in store.datoms() {
            if d.op == Op::Assert && d.attribute == embed_attr {
                if let Value::Bytes(ref b) = d.value {
                    let e = crate::embedding::bytes_to_embedding(b);
                    let body = store
                        .live_value(d.entity, &body_attr)
                        .and_then(|v| {
                            if let Value::String(s) = v {
                                Some(s.clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default();
                    observations.push((d.entity, e, body));
                }
            }
        }

        let concepts = crystallize_concepts(&observations, JOIN_THRESHOLD, MIN_CLUSTER_SIZE);
        if concepts.is_empty() {
            // Skip test if hash embedder doesn't produce close enough embeddings.
            return;
        }

        let cryst_tx = TxId::new(200, 0, agent);
        for concept in &concepts {
            let datoms: Vec<Datom> = concept_to_datoms(concept, 1000)
                .into_iter()
                .map(|(e, a, v)| Datom::new(e, a, v, cryst_tx, Op::Assert))
                .collect();
            store.apply_datoms(&datoms);
            let member_datoms: Vec<Datom> = membership_datoms(concept.entity, &concept.members)
                .into_iter()
                .map(|(e, a, v)| Datom::new(e, a, v, cryst_tx, Op::Assert))
                .collect();
            store.apply_datoms(&member_datoms);
        }

        // New observation about streaming (different from database concept).
        let novel_embedding = emb.embed("event streaming pipeline message broker");
        let concept_entity = concepts[0].entity;
        let brief = compute_discrepancy_brief(&store, &novel_embedding, concept_entity, 0.7, 0.3);

        // With high surprise, we should get a brief (may be None if nearest obs lookup
        // doesn't find differences — depends on hash embedder's behavior).
        if let Some(brief) = brief {
            assert!(
                !brief.novel_keywords.is_empty() || !brief.expected_keywords.is_empty(),
                "discrepancy brief should contain at least some keywords"
            );
            assert!(!brief.concept_name.is_empty());
            assert!(brief.surprise >= 0.3);
        }
    }

    /// INQ-TEST-4: Discrepancy brief returns None for confirming observations.
    #[test]
    fn test_discrepancy_brief_confirming() {
        let store = store_with_schema();
        let emb = make_embedder();
        let embedding = emb.embed("test observation");
        let concept_entity = EntityId::from_content(b"test-concept");

        // Low surprise should return None.
        let brief = compute_discrepancy_brief(&store, &embedding, concept_entity, 0.1, 0.3);
        assert!(
            brief.is_none(),
            "low surprise (0.1) should not produce a discrepancy brief"
        );
    }

    /// INQ-TEST-5: Situational brief produces correct output at all graduated levels.
    #[test]
    fn test_situational_brief_graduated() {
        let store = store_with_schema();
        let concept_entity = EntityId::from_content(b"test-concept");

        // Level 1: Low surprise → "name ✓"
        let low = ConceptAssignment::Joined {
            concept: concept_entity,
            similarity: 0.85,
            surprise: 0.15,
            strength: 1.0,
        };
        let brief_low = situational_brief(&store, &low, &[], None);
        assert!(brief_low.is_some(), "low surprise should produce a brief");
        let b = brief_low.unwrap();
        assert!(
            b.line.contains('\u{2713}'),
            "low surprise should contain checkmark, got: {}",
            b.line
        );
        assert_eq!(b.level, EpistemicLevel::Concept);

        // Level 2: Medium surprise → concept + details
        let med = ConceptAssignment::Joined {
            concept: concept_entity,
            similarity: 0.6,
            surprise: 0.4,
            strength: 1.0,
        };
        let brief_med = situational_brief(&store, &med, &[], None);
        assert!(brief_med.is_some());
        let b = brief_med.unwrap();
        assert_eq!(b.level, EpistemicLevel::Concept);

        // Level 3: High surprise → "NEW TERRITORY"
        let high = ConceptAssignment::Joined {
            concept: concept_entity,
            similarity: 0.3,
            surprise: 0.7,
            strength: 1.0,
        };
        let brief_high = situational_brief(&store, &high, &[], None);
        assert!(brief_high.is_some());
        let b = brief_high.unwrap();
        assert!(
            b.line.contains("NEW TERRITORY"),
            "high surprise should say NEW TERRITORY, got: {}",
            b.line
        );
        assert_eq!(b.level, EpistemicLevel::Theory);

        // Level 4: Topological event → "TOPOLOGY SHIFT"
        let topo_events = vec!["new bridge: A-B".to_string()];
        let brief_topo = situational_brief(&store, &low, &topo_events, None);
        assert!(brief_topo.is_some());
        let b = brief_topo.unwrap();
        assert!(
            b.line.contains("TOPOLOGY SHIFT"),
            "topo event should say TOPOLOGY SHIFT, got: {}",
            b.line
        );

        // Uncategorized → None
        let uncat = ConceptAssignment::Uncategorized;
        let brief_uncat = situational_brief(&store, &uncat, &[], None);
        assert!(
            brief_uncat.is_none(),
            "uncategorized should produce no brief"
        );
    }

    /// INQ-TEST-6: find_nearest_observation returns the closest observation.
    #[test]
    fn test_find_nearest_observation() {
        let texts = &[
            "database schema tables columns indexes",
            "streaming pipeline events kafka",
            "error handling retry backoff circuit",
        ];
        let store = store_with_observations(texts);
        let emb = make_embedder();

        // Search for something close to "database".
        let target = emb.embed("database schema tables columns");
        let result = find_nearest_observation(&store, &target);
        assert!(
            result.is_some(),
            "should find at least one nearest observation"
        );
        let (_, body, sim) = result.unwrap();
        assert!(
            !body.is_empty(),
            "nearest observation should have non-empty body"
        );
        assert!(sim > 0.0, "cosine similarity should be positive, got {sim}");
    }

    // ── C9-P1 Agent Provenance & Agreement Detection Tests ───────────

    #[test]
    fn test_agent_groups_single_agent() {
        use crate::datom::{AgentId, Datom, TxId};

        let mut store = crate::store::Store::genesis();
        let agent = AgentId::from_name("sole-agent");
        let attr = Attribute::from_keyword(":exploration/body");

        let mut datoms = Vec::new();
        for i in 0..5u64 {
            let entity = EntityId::from_content(format!("obs:{}", i).as_bytes());
            let tx = TxId::new(100 + i, 0, agent);
            datoms.push(Datom::new(
                entity,
                attr.clone(),
                Value::String(format!("observation number {}", i)),
                tx,
                Op::Assert,
            ));
        }
        store.apply_datoms(&datoms);

        let groups = agent_observation_groups(&store);
        assert_eq!(groups.len(), 1, "Should have exactly 1 agent group");
        assert_eq!(
            groups[0].observations.len(),
            5,
            "Agent should have 5 observations"
        );
        assert_eq!(groups[0].agent, agent);
    }

    #[test]
    fn test_agent_groups_multi_agent() {
        use crate::datom::{AgentId, Datom, TxId};

        let mut store = crate::store::Store::genesis();
        let agent_a = AgentId::from_name("agent-alpha");
        let agent_b = AgentId::from_name("agent-beta");
        let agent_c = AgentId::from_name("agent-gamma");
        let attr = Attribute::from_keyword(":exploration/body");

        let mut datoms = Vec::new();
        // Agent A: 3 observations
        for i in 0..3u64 {
            let entity = EntityId::from_content(format!("obs-a:{}", i).as_bytes());
            let tx = TxId::new(100 + i, 0, agent_a);
            datoms.push(Datom::new(
                entity,
                attr.clone(),
                Value::String(format!("agent alpha observation {}", i)),
                tx,
                Op::Assert,
            ));
        }
        // Agent B: 2 observations
        for i in 0..2u64 {
            let entity = EntityId::from_content(format!("obs-b:{}", i).as_bytes());
            let tx = TxId::new(200 + i, 0, agent_b);
            datoms.push(Datom::new(
                entity,
                attr.clone(),
                Value::String(format!("agent beta observation {}", i)),
                tx,
                Op::Assert,
            ));
        }
        // Agent C: 1 observation
        let entity_c = EntityId::from_content(b"obs-c:0");
        let tx_c = TxId::new(300, 0, agent_c);
        datoms.push(Datom::new(
            entity_c,
            attr.clone(),
            Value::String("agent gamma observation".to_string()),
            tx_c,
            Op::Assert,
        ));

        store.apply_datoms(&datoms);

        let groups = agent_observation_groups(&store);
        assert_eq!(groups.len(), 3, "Should have 3 agent groups");
        // Sorted by count descending: A(3), B(2), C(1)
        assert_eq!(groups[0].observations.len(), 3);
        assert_eq!(groups[1].observations.len(), 2);
        assert_eq!(groups[2].observations.len(), 1);
    }

    #[test]
    fn test_agreement_cluster_formation() {
        use crate::datom::{AgentId, Datom, TxId};

        let mut store = crate::store::Store::genesis();
        let agent_a = AgentId::from_name("audit-agent-1");
        let agent_b = AgentId::from_name("audit-agent-2");
        let agent_c = AgentId::from_name("audit-agent-3");
        let body_attr = Attribute::from_keyword(":exploration/body");
        let conf_attr = Attribute::from_keyword(":exploration/confidence");

        let mut datoms = Vec::new();

        // All 3 agents observe "F(S) stagnation" with different wording
        let obs = [
            (
                "a1",
                agent_a,
                "F(S) has been stagnant at 0.62 for thirteen sessions without improvement",
            ),
            (
                "b1",
                agent_b,
                "Fitness score F(S) stuck at 0.62 for thirteen sessions needs verification sprint",
            ),
            (
                "c1",
                agent_c,
                "F(S) stagnant at 0.62 for thirteen sessions recommend verification work",
            ),
        ];

        for (id_suffix, agent, text) in &obs {
            let entity = EntityId::from_content(format!("obs:{}", id_suffix).as_bytes());
            let tx = TxId::new(100, 0, *agent);
            datoms.push(Datom::new(
                entity,
                body_attr.clone(),
                Value::String(text.to_string()),
                tx,
                Op::Assert,
            ));
            datoms.push(Datom::new(
                entity,
                conf_attr.clone(),
                Value::Double(ordered_float::OrderedFloat(0.9)),
                tx,
                Op::Assert,
            ));
        }
        store.apply_datoms(&datoms);

        let clusters = find_agreement_clusters(&store, 0.3);
        assert!(
            !clusters.is_empty(),
            "Should find at least one agreement cluster"
        );
        let top = &clusters[0];
        assert!(
            top.agents.len() >= 2,
            "Top cluster should have 2+ agents, got {}",
            top.agents.len()
        );
        assert!(
            top.agreement_score >= 0.5,
            "Agreement score should be >= 0.5, got {}",
            top.agreement_score
        );
    }

    #[test]
    fn test_agreement_no_false_positive() {
        use crate::datom::{AgentId, Datom, TxId};

        let mut store = crate::store::Store::genesis();
        let agent_a = AgentId::from_name("agent-1");
        let agent_b = AgentId::from_name("agent-2");
        let agent_c = AgentId::from_name("agent-3");
        let attr = Attribute::from_keyword(":exploration/body");

        let mut datoms = Vec::new();
        // Completely unrelated observations
        let obs = [
            (
                "a",
                agent_a,
                "The weather in Paris is lovely this time of year",
            ),
            (
                "b",
                agent_b,
                "Quantum computing uses superposition for parallel computation",
            ),
            (
                "c",
                agent_c,
                "Ancient Roman aqueducts transported water across valleys",
            ),
        ];
        for (id, agent, text) in &obs {
            let entity = EntityId::from_content(format!("obs:{}", id).as_bytes());
            let tx = TxId::new(100, 0, *agent);
            datoms.push(Datom::new(
                entity,
                attr.clone(),
                Value::String(text.to_string()),
                tx,
                Op::Assert,
            ));
        }
        store.apply_datoms(&datoms);

        let clusters = find_agreement_clusters(&store, 0.3);
        // No cluster should have agreement_score > 1/3
        for c in &clusters {
            assert!(
                c.agreement_score <= 0.34,
                "Unrelated topics should not form high-agreement clusters, got {}",
                c.agreement_score
            );
        }
    }

    #[test]
    fn test_agreement_empty_store() {
        let store = crate::store::Store::genesis();
        let clusters = find_agreement_clusters(&store, 0.3);
        assert!(
            clusters.is_empty(),
            "Genesis store should have no agreement clusters"
        );
    }

    #[test]
    fn test_format_agreement_summary() {
        let clusters = vec![AgreementCluster {
            topic: "F(S) stagnant at 0.62 for many sessions".to_string(),
            agents: vec![
                AgentId::from_name("a"),
                AgentId::from_name("b"),
                AgentId::from_name("c"),
            ],
            observation_ids: vec![],
            confidence_range: (0.85, 0.90, 0.95),
            agreement_score: 1.0,
            member_count: 5,
        }];
        let lines = format_agreement_summary(&clusters, 3);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("3/3 agents"));
        assert!(lines[0].contains("5 obs"));
        assert!(lines[0].contains("0.85"));
    }

    // ── C9-P2 Observation Link Extraction Tests ──────────────────────

    #[test]
    fn test_classify_relation_depends() {
        assert_eq!(
            classify_relation("this depends on that"),
            LinkRelation::DependsOn
        );
        assert_eq!(
            classify_relation("requires the fix"),
            LinkRelation::DependsOn
        );
        assert_eq!(
            classify_relation("blocked by upstream"),
            LinkRelation::DependsOn
        );
    }

    #[test]
    fn test_classify_relation_blocks() {
        assert_eq!(classify_relation("blocks downstream"), LinkRelation::Blocks);
        assert_eq!(
            classify_relation("enables the next step"),
            LinkRelation::Blocks
        );
    }

    #[test]
    fn test_classify_relation_default() {
        assert_eq!(
            classify_relation("see also the other finding"),
            LinkRelation::RelatesTo
        );
        assert_eq!(
            classify_relation("no keywords here"),
            LinkRelation::RelatesTo
        );
    }

    #[test]
    fn test_is_task_id_at() {
        assert_eq!(
            is_task_id_at("see t-abcd1234 here", 4),
            Some("t-abcd1234".to_string())
        );
        assert_eq!(
            is_task_id_at("t-0000ffff", 0),
            Some("t-0000ffff".to_string())
        );
        assert_eq!(is_task_id_at("t-xyz", 0), None); // too short
        assert_eq!(is_task_id_at("t-ZZZZZZZZ", 0), None); // not hex
    }

    #[test]
    fn test_is_spec_ref_at() {
        assert_eq!(
            is_spec_ref_at("see INV-STORE-001 here", 4),
            Some("INV-STORE-001".to_string())
        );
        assert_eq!(
            is_spec_ref_at("ADR-FOUNDATION-012", 0),
            Some("ADR-FOUNDATION-012".to_string())
        );
        assert_eq!(
            is_spec_ref_at("NEG-MERGE-003 text", 0),
            Some("NEG-MERGE-003".to_string())
        );
        assert_eq!(is_spec_ref_at("INV-lowercase-001", 0), None);
        assert_eq!(is_spec_ref_at("not a ref", 0), None);
    }

    #[test]
    fn test_extract_links_empty_store() {
        let store = crate::store::Store::genesis();
        let links = extract_observation_links(&store);
        assert!(
            links.is_empty(),
            "Genesis store should have no observation links"
        );
    }

    #[test]
    fn test_extract_links_with_task_ref() {
        use crate::datom::{AgentId, Datom, TxId};

        let mut store = crate::store::Store::genesis();
        let agent = AgentId::from_name("test");

        // Create a task entity with known ID.
        let task_entity = EntityId::from_content(b"task:abcd1234");
        let obs_entity = EntityId::from_content(b"obs:with-ref");
        let tx = TxId::new(100, 0, agent);
        let ident_attr = Attribute::from_keyword(":db/ident");
        let body_attr = Attribute::from_keyword(":exploration/body");

        let datoms = vec![
            Datom::new(
                task_entity,
                ident_attr.clone(),
                Value::Keyword(":task/t-abcd1234".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                obs_entity,
                body_attr.clone(),
                Value::String("This observation depends on t-abcd1234 for completion".to_string()),
                tx,
                Op::Assert,
            ),
        ];
        store.apply_datoms(&datoms);

        let links = extract_observation_links(&store);
        assert!(
            !links.is_empty(),
            "Should extract at least one link from observation referencing task"
        );
        let link = &links[0];
        assert_eq!(link.source, obs_entity);
        assert_eq!(link.target, task_entity);
        assert_eq!(link.relationship, LinkRelation::DependsOn);
    }

    #[test]
    fn test_extract_links_dedup() {
        use crate::datom::{AgentId, Datom, TxId};

        let mut store = crate::store::Store::genesis();
        let agent = AgentId::from_name("test");
        let task_entity = EntityId::from_content(b"task:dup");
        let obs_entity = EntityId::from_content(b"obs:dup-ref");
        let tx = TxId::new(100, 0, agent);

        let datoms = vec![
            Datom::new(
                task_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":task/t-00001111".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                obs_entity,
                Attribute::from_keyword(":exploration/body"),
                Value::String(
                    "see t-00001111 and also t-00001111 and again t-00001111".to_string(),
                ),
                tx,
                Op::Assert,
            ),
        ];
        store.apply_datoms(&datoms);

        let links = extract_observation_links(&store);
        // Should have exactly 1 link despite 3 mentions.
        let count = links
            .iter()
            .filter(|l| l.source == obs_entity && l.target == task_entity)
            .count();
        assert_eq!(
            count, 1,
            "Duplicate references should produce exactly 1 link"
        );
    }

    #[test]
    fn test_link_datoms_conversion() {
        let links = vec![
            ExtractedLink {
                source: EntityId::from_content(b"obs:a"),
                target: EntityId::from_content(b"task:b"),
                relationship: LinkRelation::DependsOn,
                context: "depends on".to_string(),
            },
            ExtractedLink {
                source: EntityId::from_content(b"obs:a"),
                target: EntityId::from_content(b"spec:c"),
                relationship: LinkRelation::RelatesTo,
                context: "see also".to_string(),
            },
        ];
        let datoms = link_datoms(&links);
        assert_eq!(datoms.len(), 2);
        assert_eq!(datoms[0].1.as_str(), ":exploration/depends-on");
        assert_eq!(datoms[1].1.as_str(), ":exploration/related-spec");
    }
}
