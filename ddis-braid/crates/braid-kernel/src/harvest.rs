//! HARVEST namespace — end-of-session epistemic gap detection pipeline.
//!
//! Detects knowledge gaps between an agent's session context and the store,
//! proposes candidates for externalization, and commits approved candidates.
//!
//! # Pipeline (INV-HARVEST-005)
//!
//! DETECT → CLASSIFY → SCORE → PROPOSE → REVIEW → COMMIT → RECORD
//!
//! # v2 Architecture
//!
//! Harvest v2 replaces the naive set-difference (K_agent \ K_store) with
//! tx-log extraction and classification:
//!
//! 1. **EXTRACT**: Identify entities touched in session transactions
//!    (wall_time > session_start_tx.wall_time).
//! 2. **CLASSIFY**: Categorize each entity by attribute namespace analysis
//!    (not keyword heuristics).
//! 3. **SCORE**: Compute confidence via information density — entities with
//!    more attributes and cross-references score higher.
//! 4. **GAP DETECT**: Identify entities missing expected attributes for their
//!    category (a spec entity without :spec/falsification is a gap).
//!
//! # Invariants
//!
//! - **INV-HARVEST-001**: Epistemic gap Δ(t) = K_agent \ K_store.
//! - **INV-HARVEST-002**: Monotonic extension (store grows, never shrinks).
//! - **INV-HARVEST-003**: Quality metrics (FP rate, FN rate, drift_score).
//! - **INV-HARVEST-005**: Pipeline correctness (5-step state machine).
//! - **INV-HARVEST-010**: Tx-log extraction completeness.
//! - **INV-HARVEST-011**: Classification accuracy (structural, not heuristic).

use std::collections::{BTreeMap, BTreeSet};

use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, TxId, Value};
use crate::store::Store;
use crate::trilateral::{classify_attribute, AttrNamespace};

/// A knowledge unit proposed for externalization into the store.
#[derive(Clone, Debug)]
pub struct HarvestCandidate {
    /// Entity to create or update.
    pub entity: EntityId,
    /// Attribute-value pairs to assert.
    pub assertions: Vec<(Attribute, Value)>,
    /// Category of knowledge.
    pub category: HarvestCategory,
    /// Confidence in this candidate (0.0–1.0).
    pub confidence: f64,
    /// Current status in the pipeline.
    pub status: CandidateStatus,
    /// Human-readable rationale.
    pub rationale: String,
}

/// Categories of harvested knowledge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HarvestCategory {
    /// Factual observation from the session.
    Observation,
    /// Design decision made during the session.
    Decision,
    /// Dependency discovered between artifacts.
    Dependency,
    /// Open question or uncertainty.
    Uncertainty,
}

/// Status lattice for harvest candidates (INV-HARVEST-005).
///
/// Forms a total order: Proposed < UnderReview < Committed | Rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CandidateStatus {
    /// Newly proposed by gap detection.
    Proposed,
    /// Under agent review.
    UnderReview,
    /// Approved and committed to the store.
    Committed,
    /// Rejected by the agent.
    Rejected,
}

/// Session context for the harvest pipeline.
#[derive(Clone, Debug)]
pub struct SessionContext {
    /// The agent performing the harvest.
    pub agent: AgentId,
    /// Human-readable agent name (for provenance metadata).
    pub agent_name: String,
    /// TxId at session start.
    pub session_start_tx: TxId,
    /// Description of the task worked on.
    pub task_description: String,
    /// Knowledge items from the session (pre-candidates — v1 compat).
    pub session_knowledge: Vec<(String, Value)>,
}

/// Result of running the harvest pipeline.
#[derive(Clone, Debug)]
pub struct HarvestResult {
    /// Proposed candidates.
    pub candidates: Vec<HarvestCandidate>,
    /// Drift score: how much knowledge was NOT in the store.
    pub drift_score: f64,
    /// Quality metrics.
    pub quality: HarvestQuality,
    /// v2: Entities touched during the session (tx-log extraction).
    pub session_entities: usize,
    /// v2: Completeness gaps detected (missing expected attributes).
    pub completeness_gaps: usize,
}

/// Quality metrics for a harvest.
#[derive(Clone, Debug, Default)]
pub struct HarvestQuality {
    /// Total candidates.
    pub count: usize,
    /// High confidence (>= 0.8).
    pub high_confidence: usize,
    /// Medium confidence (0.5–0.8).
    pub medium_confidence: usize,
    /// Low confidence (< 0.5).
    pub low_confidence: usize,
}

// ---------------------------------------------------------------------------
// v2: Tx-log entity profile (intermediate representation)
// ---------------------------------------------------------------------------

/// Profile of an entity as seen in the session's transactions.
#[derive(Clone, Debug)]
struct EntityProfile {
    /// The entity.
    entity: EntityId,
    /// All attributes asserted for this entity.
    attributes: BTreeSet<String>,
    /// Namespace classification counts.
    namespace_counts: BTreeMap<AttrNamespace, usize>,
    /// Whether the entity has a :db/ident (named entity).
    has_ident: bool,
    /// The ident value, if present.
    ident: Option<String>,
    /// Number of cross-reference (Ref) values.
    ref_count: usize,
    /// Total datom count for this entity.
    datom_count: usize,
}

/// Expected attributes for completeness checking per category.
const SPEC_EXPECTED: &[&str] = &[":spec/id", ":spec/element-type", ":db/doc"];
const DECISION_EXPECTED: &[&str] = &[":intent/decision", ":intent/rationale"];

// ---------------------------------------------------------------------------
// Pipeline: harvest_pipeline (v2)
// ---------------------------------------------------------------------------

/// Run the harvest pipeline: EXTRACT → CLASSIFY → SCORE → GAP-DETECT → PROPOSE.
///
/// v2: Combines tx-log extraction with the original session_knowledge input.
/// Both sources are merged to maximize harvest coverage.
pub fn harvest_pipeline(store: &Store, context: &SessionContext) -> HarvestResult {
    let mut candidates = Vec::new();

    // -----------------------------------------------------------------------
    // Phase 1: Tx-log extraction (INV-HARVEST-010)
    // -----------------------------------------------------------------------
    let profiles = extract_session_profiles(store, &context.session_start_tx);
    let session_entities = profiles.len();

    // Phase 2: Classify + Score + Gap-detect for tx-log entities
    let mut completeness_gaps = 0;
    let mut seen_entities: BTreeSet<EntityId> = BTreeSet::new();

    for profile in &profiles {
        seen_entities.insert(profile.entity);
        let category = classify_profile(profile);
        let confidence = score_profile(profile, category);
        let gaps = detect_gaps(profile, category);

        if !gaps.is_empty() {
            completeness_gaps += gaps.len();
            for gap in &gaps {
                candidates.push(HarvestCandidate {
                    entity: profile.entity,
                    assertions: vec![], // Gap detection only — no value to assert
                    category,
                    confidence: confidence * 0.6, // Reduce confidence for gap-only candidates
                    status: CandidateStatus::Proposed,
                    rationale: format!(
                        "Completeness gap: {} missing for {}",
                        gap,
                        profile.ident.as_deref().unwrap_or("unnamed entity")
                    ),
                });
            }
        }
    }

    // -----------------------------------------------------------------------
    // Phase 3: Session knowledge integration (v1 compat + enhancement)
    // -----------------------------------------------------------------------
    for (key, value) in &context.session_knowledge {
        let entity = EntityId::from_ident(key);

        // Skip if already profiled from tx-log
        if seen_entities.contains(&entity) {
            continue;
        }

        // Check if entity exists in store via entity index (O(1))
        let existing = store.entity_datoms(entity);
        if existing.is_empty() {
            candidates.push(HarvestCandidate {
                entity,
                assertions: vec![(Attribute::from_keyword(":db/doc"), value.clone())],
                category: classify_value(value),
                confidence: 0.8,
                status: CandidateStatus::Proposed,
                rationale: format!("New knowledge from session: {key}"),
            });
        }
    }

    // -----------------------------------------------------------------------
    // Compute metrics
    // -----------------------------------------------------------------------
    let count = candidates.len();
    let high_confidence = candidates.iter().filter(|c| c.confidence >= 0.8).count();
    let medium_confidence = candidates
        .iter()
        .filter(|c| c.confidence >= 0.5 && c.confidence < 0.8)
        .count();
    let low_confidence = candidates.iter().filter(|c| c.confidence < 0.5).count();

    let total_knowledge = context.session_knowledge.len() + session_entities;
    let drift_score = if total_knowledge == 0 {
        0.0
    } else {
        count as f64 / total_knowledge as f64
    };

    HarvestResult {
        candidates,
        drift_score,
        quality: HarvestQuality {
            count,
            high_confidence,
            medium_confidence,
            low_confidence,
        },
        session_entities,
        completeness_gaps,
    }
}

// ---------------------------------------------------------------------------
// Phase 1: Tx-log extraction
// ---------------------------------------------------------------------------

/// Extract entity profiles from transactions that occurred during or after the session.
///
/// Scans the store for datoms with tx > session_start_tx (using the full
/// HLC ordering which considers wall_time, logical, and agent).
/// This correctly handles the case where wall_time doesn't advance
/// (pure-kernel mode where clock is deterministic).
fn extract_session_profiles(store: &Store, session_start_tx: &TxId) -> Vec<EntityProfile> {
    let mut entity_datoms: BTreeMap<EntityId, Vec<&Datom>> = BTreeMap::new();

    for datom in store.datoms() {
        if datom.tx > *session_start_tx && datom.op == Op::Assert {
            entity_datoms.entry(datom.entity).or_default().push(datom);
        }
    }

    entity_datoms
        .into_iter()
        .map(|(entity, datoms)| build_profile(entity, &datoms))
        .collect()
}

/// Build an entity profile from its datoms.
fn build_profile(entity: EntityId, datoms: &[&Datom]) -> EntityProfile {
    let mut attributes = BTreeSet::new();
    let mut namespace_counts: BTreeMap<AttrNamespace, usize> = BTreeMap::new();
    let mut has_ident = false;
    let mut ident = None;
    let mut ref_count = 0;

    for datom in datoms {
        let attr_str = datom.attribute.as_str().to_string();
        attributes.insert(attr_str);

        let ns = classify_attribute(&datom.attribute);
        *namespace_counts.entry(ns).or_default() += 1;

        if datom.attribute.as_str() == ":db/ident" {
            has_ident = true;
            if let Value::Keyword(kw) = &datom.value {
                ident = Some(kw.clone());
            }
        }

        if matches!(&datom.value, Value::Ref(_)) {
            ref_count += 1;
        }
    }

    EntityProfile {
        entity,
        attributes,
        namespace_counts,
        has_ident,
        ident,
        ref_count,
        datom_count: datoms.len(),
    }
}

// ---------------------------------------------------------------------------
// Phase 2: Classification (structural, not heuristic)
// ---------------------------------------------------------------------------

/// Classify an entity profile by its dominant attribute namespace.
///
/// INV-HARVEST-011: Uses structural namespace analysis, not keyword matching.
fn classify_profile(profile: &EntityProfile) -> HarvestCategory {
    // Find dominant namespace
    let dominant = profile
        .namespace_counts
        .iter()
        .filter(|(ns, _)| **ns != AttrNamespace::Meta)
        .max_by_key(|(_, count)| *count)
        .map(|(ns, _)| *ns);

    match dominant {
        Some(AttrNamespace::Intent) => {
            // Check for decision markers
            if profile.attributes.contains(":intent/decision")
                || profile.attributes.contains(":intent/rationale")
            {
                HarvestCategory::Decision
            } else {
                HarvestCategory::Observation
            }
        }
        Some(AttrNamespace::Spec) => {
            // Check for uncertainty markers
            if profile.attributes.iter().any(|a| a.contains("uncertain")) {
                HarvestCategory::Uncertainty
            } else if profile.ref_count > 0 {
                HarvestCategory::Dependency
            } else {
                HarvestCategory::Observation
            }
        }
        Some(AttrNamespace::Impl) => HarvestCategory::Dependency,
        _ => {
            // Meta-only or no dominant namespace — infer from attributes
            if profile.ref_count > 2 {
                HarvestCategory::Dependency
            } else {
                HarvestCategory::Observation
            }
        }
    }
}

/// Classify a value into a harvest category (v1 compat).
fn classify_value(value: &Value) -> HarvestCategory {
    match value {
        Value::Keyword(kw) if kw.contains("decision") || kw.contains("adr") => {
            HarvestCategory::Decision
        }
        Value::Keyword(kw) if kw.contains("dep") || kw.contains("block") => {
            HarvestCategory::Dependency
        }
        Value::Keyword(kw) if kw.contains("question") || kw.contains("uncertain") => {
            HarvestCategory::Uncertainty
        }
        _ => HarvestCategory::Observation,
    }
}

// ---------------------------------------------------------------------------
// Phase 3: Fisher scoring (information-geometric confidence)
// ---------------------------------------------------------------------------

/// Ideal namespace distributions for each harvest category.
///
/// These define the expected namespace distribution on the probability
/// simplex Δ³ for each category. The Fisher-Rao distance from an entity's
/// empirical distribution to the ideal determines classification confidence.
///
/// Distributions: [Intent, Spec, Impl, Meta]
fn ideal_distribution(category: HarvestCategory) -> [f64; 4] {
    match category {
        HarvestCategory::Decision => [0.6, 0.1, 0.1, 0.2],
        HarvestCategory::Observation => [0.1, 0.4, 0.2, 0.3],
        HarvestCategory::Dependency => [0.05, 0.3, 0.5, 0.15],
        HarvestCategory::Uncertainty => [0.3, 0.3, 0.1, 0.3],
    }
}

/// Fisher-Rao distance on the probability simplex.
///
/// d_FR(p, q) = 2 arccos(Σᵢ √(pᵢ qᵢ))
///
/// This is the geodesic distance on the statistical manifold under the
/// Fisher information metric — the unique Riemannian metric invariant
/// under sufficient statistics (Chentsov's theorem, 1972).
///
/// Range: [0, π]. d_FR = 0 iff p = q; d_FR = π iff p ⊥ q.
fn fisher_rao_distance(p: &[f64], q: &[f64]) -> f64 {
    let bhattacharyya: f64 = p
        .iter()
        .zip(q.iter())
        .map(|(pi, qi)| (pi * qi).sqrt())
        .sum();
    2.0 * bhattacharyya.clamp(0.0, 1.0).acos()
}

/// Score an entity profile using Fisher-Rao information geometry.
///
/// Computes the geodesic distance on Δ³ (the 3-simplex of namespace
/// distributions) between the entity's empirical distribution and the
/// ideal distribution for its classified category. Closer = higher
/// confidence.
///
/// The score combines:
/// - **Fisher-Rao proximity** (50%): 1 - d_FR(p̂, p*)/π
/// - **Sample size** (25%): n/10 (more observations = more confident)
/// - **Identity** (10%): named entities are more trustworthy
/// - **Reference density** (15%): cross-references indicate real relationships
fn score_profile(profile: &EntityProfile, category: HarvestCategory) -> f64 {
    let total: usize = profile.namespace_counts.values().sum();
    if total == 0 {
        return 0.1; // Minimal confidence for empty profiles
    }

    // Empirical distribution across namespaces
    let emp = [
        *profile
            .namespace_counts
            .get(&AttrNamespace::Intent)
            .unwrap_or(&0) as f64
            / total as f64,
        *profile
            .namespace_counts
            .get(&AttrNamespace::Spec)
            .unwrap_or(&0) as f64
            / total as f64,
        *profile
            .namespace_counts
            .get(&AttrNamespace::Impl)
            .unwrap_or(&0) as f64
            / total as f64,
        *profile
            .namespace_counts
            .get(&AttrNamespace::Meta)
            .unwrap_or(&0) as f64
            / total as f64,
    ];

    // Laplace smoothing to avoid zero-probability singularities
    // (the Fisher-Rao metric diverges at the simplex boundary)
    let alpha = 0.01;
    let emp_smooth: Vec<f64> = emp.iter().map(|&p| p + alpha).collect();
    let sum: f64 = emp_smooth.iter().sum();
    let emp_normalized: Vec<f64> = emp_smooth.iter().map(|&p| p / sum).collect();

    let ideal = ideal_distribution(category);
    let distance = fisher_rao_distance(&emp_normalized, &ideal);

    // Convert distance to confidence: closer to ideal = higher confidence
    // Fisher-Rao distance on Δ³ is in [0, π]
    let confidence_from_distance = 1.0 - (distance / std::f64::consts::PI);

    // Sample size factor: use datom_count as the full observation count
    // (namespace counts may be a subset if some attributes aren't classified)
    let n = profile.datom_count.max(total);
    let sample_factor = (n as f64 / 10.0).min(1.0);

    // Identity bonus (named entities carry more epistemic weight)
    let identity = if profile.has_ident { 0.1 } else { 0.0 };

    // Reference density bonus (cross-references indicate real relationships)
    let ref_bonus = (profile.ref_count as f64 / 5.0).min(0.15);

    (0.5 * confidence_from_distance + 0.25 * sample_factor + identity + ref_bonus).min(1.0)
}

// ---------------------------------------------------------------------------
// Phase 4: Completeness gap detection
// ---------------------------------------------------------------------------

/// Detect missing expected attributes for an entity's category.
fn detect_gaps(profile: &EntityProfile, category: HarvestCategory) -> Vec<String> {
    let mut gaps = Vec::new();

    // Check spec completeness
    if profile
        .namespace_counts
        .get(&AttrNamespace::Spec)
        .copied()
        .unwrap_or(0)
        > 0
    {
        for expected in SPEC_EXPECTED {
            if !profile.attributes.contains(*expected) {
                gaps.push(expected.to_string());
            }
        }
    }

    // Check decision completeness
    if category == HarvestCategory::Decision {
        for expected in DECISION_EXPECTED {
            if !profile.attributes.contains(*expected) {
                gaps.push(expected.to_string());
            }
        }
    }

    gaps
}

/// Convert an approved candidate to datoms for transacting.
///
/// INV-HARVEST-002: This only adds datoms, never removes.
pub fn candidate_to_datoms(candidate: &HarvestCandidate, tx: TxId) -> Vec<Datom> {
    let mut datoms = Vec::new();
    for (attr, value) in &candidate.assertions {
        datoms.push(Datom::new(
            candidate.entity,
            attr.clone(),
            value.clone(),
            tx,
            Op::Assert,
        ));
    }
    datoms
}

// ---------------------------------------------------------------------------
// Phase 4-5: Session entity + commit pipeline
// ---------------------------------------------------------------------------

/// A complete harvest commit — candidates + session entity + provenance.
///
/// This is the kernel-level abstraction for committing a harvest result.
/// The CLI uses this to build the transaction file.
#[derive(Clone, Debug)]
pub struct HarvestCommit {
    /// All datoms to assert (candidates + session entity).
    pub datoms: Vec<Datom>,
    /// The session entity ident (e.g., ":harvest/session-agent-123").
    pub session_ident: String,
    /// Transaction ID for this commit.
    pub tx_id: TxId,
    /// Summary statistics.
    pub candidate_count: usize,
    /// Total datom count.
    pub datom_count: usize,
}

/// Build a complete harvest commit from a pipeline result.
///
/// Creates:
/// 1. Datoms for each candidate (Phase 5: commit)
/// 2. A HarvestSession entity with provenance metadata (Phase 4: session entity)
///
/// INV-HARVEST-002: monotonic extension (only assertions, no retractions).
/// INV-HARVEST-005: pipeline terminal state (Proposed → Committed).
pub fn build_harvest_commit(
    result: &HarvestResult,
    context: &SessionContext,
    tx_id: TxId,
) -> HarvestCommit {
    let mut all_datoms: Vec<Datom> = Vec::new();

    // Phase 5: Convert each candidate to datoms
    for candidate in &result.candidates {
        all_datoms.extend(candidate_to_datoms(candidate, tx_id));
    }

    // Phase 4: Create HarvestSession entity (provenance trail)
    let safe_agent = context.agent_name.replace([':', '/'], "-");
    let session_ident = format!(":harvest/session-{}-{}", safe_agent, tx_id.wall_time());
    let session_entity = EntityId::from_ident(&session_ident);

    // :db/ident — session identity
    all_datoms.push(Datom::new(
        session_entity,
        Attribute::from_keyword(":db/ident"),
        Value::Keyword(session_ident.clone()),
        tx_id,
        Op::Assert,
    ));
    // :db/doc — session description
    all_datoms.push(Datom::new(
        session_entity,
        Attribute::from_keyword(":db/doc"),
        Value::String(format!(
            "Harvest session for task: {}",
            context.task_description
        )),
        tx_id,
        Op::Assert,
    ));
    // :harvest/agent — who harvested
    all_datoms.push(Datom::new(
        session_entity,
        Attribute::from_keyword(":harvest/agent"),
        Value::String(context.agent_name.clone()),
        tx_id,
        Op::Assert,
    ));
    // :harvest/candidate-count — how many candidates
    all_datoms.push(Datom::new(
        session_entity,
        Attribute::from_keyword(":harvest/candidate-count"),
        Value::Long(result.candidates.len() as i64),
        tx_id,
        Op::Assert,
    ));
    // :harvest/drift-score — epistemic drift metric
    all_datoms.push(Datom::new(
        session_entity,
        Attribute::from_keyword(":harvest/drift-score"),
        Value::Double(ordered_float::OrderedFloat(result.drift_score)),
        tx_id,
        Op::Assert,
    ));
    // :harvest/session-entities — tx-log extraction count (v2 metric)
    all_datoms.push(Datom::new(
        session_entity,
        Attribute::from_keyword(":harvest/session-entities"),
        Value::Long(result.session_entities as i64),
        tx_id,
        Op::Assert,
    ));
    // :harvest/completeness-gaps — gap count (v2 metric)
    all_datoms.push(Datom::new(
        session_entity,
        Attribute::from_keyword(":harvest/completeness-gaps"),
        Value::Long(result.completeness_gaps as i64),
        tx_id,
        Op::Assert,
    ));

    let datom_count = all_datoms.len();

    HarvestCommit {
        datoms: all_datoms,
        session_ident,
        tx_id,
        candidate_count: result.candidates.len(),
        datom_count,
    }
}

// ---------------------------------------------------------------------------
// Phase 6: FP/FN Calibration (INV-HARVEST-004)
// ---------------------------------------------------------------------------

/// Confusion matrix for harvest candidate calibration.
///
/// Compares proposed candidates against ground truth entity sets.
/// - **True Positive**: Entity correctly proposed for harvesting.
/// - **False Positive**: Entity proposed but should not have been (noise).
/// - **False Negative**: Entity that should have been harvested but was missed.
#[derive(Clone, Debug)]
pub struct CalibrationResult {
    /// True positives (correctly proposed entities).
    pub true_positives: usize,
    /// False positives (incorrectly proposed entities).
    pub false_positives: usize,
    /// False negatives (missed entities).
    pub false_negatives: usize,
    /// Precision = TP / (TP + FP). In [0, 1].
    pub precision: f64,
    /// Recall = TP / (TP + FN). In [0, 1].
    pub recall: f64,
    /// F₁ score = 2 * P * R / (P + R). Harmonic mean of precision and recall.
    pub f1_score: f64,
    /// Matthews Correlation Coefficient — balanced metric even with class imbalance.
    /// MCC ∈ [-1, 1]. +1 = perfect, 0 = random, -1 = inverse.
    pub mcc: f64,
    /// Confidence-stratified metrics: (threshold, precision_at_threshold, recall_at_threshold).
    pub stratified: Vec<(f64, f64, f64)>,
}

/// Calibrate harvest candidates against ground truth (INV-HARVEST-004).
///
/// Given a set of entities that *should* have been harvested (ground truth)
/// and the actual harvest result, computes precision, recall, F₁, and MCC.
///
/// Also computes confidence-stratified metrics: at each confidence threshold
/// (0.3, 0.5, 0.7, 0.9), what are the precision and recall for candidates
/// above that threshold?
///
/// # Arguments
///
/// * `result` — The harvest pipeline result containing proposed candidates.
/// * `ground_truth` — Entity IDs that should have been harvested.
/// * `total_entities` — Total entity count in the store (for TN computation).
pub fn calibrate_harvest(
    result: &HarvestResult,
    ground_truth: &BTreeSet<EntityId>,
    total_entities: usize,
) -> CalibrationResult {
    // Proposed entity set (deduplicated — multiple candidates may target same entity)
    let proposed: BTreeSet<EntityId> = result.candidates.iter().map(|c| c.entity).collect();

    let true_positives = proposed.intersection(ground_truth).count();
    let false_positives = proposed.difference(ground_truth).count();
    let false_negatives = ground_truth.difference(&proposed).count();

    // True negatives: entities neither proposed nor in ground truth
    let true_negatives =
        total_entities.saturating_sub(true_positives + false_positives + false_negatives);

    let precision = if true_positives + false_positives == 0 {
        1.0 // No predictions → vacuously precise
    } else {
        true_positives as f64 / (true_positives + false_positives) as f64
    };

    let recall = if ground_truth.is_empty() {
        1.0 // No ground truth → vacuously complete
    } else {
        true_positives as f64 / ground_truth.len() as f64
    };

    let f1_score = if precision + recall == 0.0 {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    };

    // Matthews Correlation Coefficient
    // MCC = (TP*TN - FP*FN) / sqrt((TP+FP)(TP+FN)(TN+FP)(TN+FN))
    let tp = true_positives as f64;
    let fp = false_positives as f64;
    let fn_ = false_negatives as f64;
    let tn = true_negatives as f64;
    let denom = ((tp + fp) * (tp + fn_) * (tn + fp) * (tn + fn_)).sqrt();
    let mcc = if denom < 1e-15 {
        0.0
    } else {
        (tp * tn - fp * fn_) / denom
    };

    // Confidence-stratified metrics
    let thresholds = [0.3, 0.5, 0.7, 0.9];
    let stratified = thresholds
        .iter()
        .map(|&threshold| {
            let above: BTreeSet<EntityId> = result
                .candidates
                .iter()
                .filter(|c| c.confidence >= threshold)
                .map(|c| c.entity)
                .collect();
            let tp_at = above.intersection(ground_truth).count();
            let fp_at = above.difference(ground_truth).count();
            let p_at = if tp_at + fp_at == 0 {
                1.0
            } else {
                tp_at as f64 / (tp_at + fp_at) as f64
            };
            let r_at = if ground_truth.is_empty() {
                1.0
            } else {
                tp_at as f64 / ground_truth.len() as f64
            };
            (threshold, p_at, r_at)
        })
        .collect();

    CalibrationResult {
        true_positives,
        false_positives,
        false_negatives,
        precision,
        recall,
        f1_score,
        mcc,
        stratified,
    }
}

/// Compute the optimal confidence threshold that maximizes F₁.
///
/// Sweeps candidate confidence values and finds the threshold where
/// F₁ is maximized. Returns (best_threshold, best_f1).
pub fn optimal_threshold(result: &HarvestResult, ground_truth: &BTreeSet<EntityId>) -> (f64, f64) {
    if result.candidates.is_empty() || ground_truth.is_empty() {
        return (0.5, 0.0);
    }

    // Collect unique confidence values as potential thresholds
    let mut thresholds: Vec<f64> = result.candidates.iter().map(|c| c.confidence).collect();
    thresholds.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    thresholds.dedup();

    let mut best_threshold = 0.5;
    let mut best_f1 = 0.0;

    for &threshold in &thresholds {
        let above: BTreeSet<EntityId> = result
            .candidates
            .iter()
            .filter(|c| c.confidence >= threshold)
            .map(|c| c.entity)
            .collect();

        let tp = above.intersection(ground_truth).count() as f64;
        let fp = above.difference(ground_truth).count() as f64;
        let fn_ = ground_truth.difference(&above).count() as f64;

        let p = if tp + fp == 0.0 { 1.0 } else { tp / (tp + fp) };
        let r = if tp + fn_ == 0.0 {
            1.0
        } else {
            tp / (tp + fn_)
        };
        let f1 = if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        };

        if f1 > best_f1 {
            best_f1 = f1;
            best_threshold = threshold;
        }
    }

    (best_threshold, best_f1)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::AgentId;

    #[test]
    fn harvest_detects_new_knowledge() {
        let store = Store::genesis();
        let agent = AgentId::from_name("test-agent");

        let context = SessionContext {
            agent,
            agent_name: "test-agent".into(),
            session_start_tx: TxId::new(0, 0, agent),
            task_description: "test session".to_string(),
            session_knowledge: vec![
                (
                    ":session/finding-1".to_string(),
                    Value::String("new fact".into()),
                ),
                (
                    ":session/finding-2".to_string(),
                    Value::String("another fact".into()),
                ),
            ],
        };

        let result = harvest_pipeline(&store, &context);
        assert_eq!(result.candidates.len(), 2);
        assert!(result.drift_score > 0.0);
    }

    #[test]
    fn harvest_skips_existing_knowledge() {
        let store = Store::genesis();
        let agent = AgentId::from_name("test-agent");

        // Use an entity that already exists in genesis
        let context = SessionContext {
            agent,
            agent_name: "test-agent".into(),
            session_start_tx: TxId::new(0, 0, agent),
            task_description: "test session".to_string(),
            session_knowledge: vec![(
                ":db/ident".to_string(),
                Value::String("already exists".into()),
            )],
        };

        let result = harvest_pipeline(&store, &context);
        assert_eq!(result.candidates.len(), 0);
    }

    #[test]
    fn candidate_to_datoms_produces_correct_count() {
        let agent = AgentId::from_name("test");
        let tx = TxId::new(100, 0, agent);
        let candidate = HarvestCandidate {
            entity: EntityId::from_ident(":test/entity"),
            assertions: vec![
                (
                    Attribute::from_keyword(":db/doc"),
                    Value::String("doc".into()),
                ),
                (
                    Attribute::from_keyword(":db/ident"),
                    Value::Keyword(":test/entity".into()),
                ),
            ],
            category: HarvestCategory::Observation,
            confidence: 0.9,
            status: CandidateStatus::Committed,
            rationale: "test".to_string(),
        };

        let datoms = candidate_to_datoms(&candidate, tx);
        assert_eq!(datoms.len(), 2);
        assert!(datoms.iter().all(|d| d.op == Op::Assert));
    }

    #[test]
    fn quality_metrics_computed_correctly() {
        let store = Store::genesis();
        let agent = AgentId::from_name("test");

        let context = SessionContext {
            agent,
            agent_name: "test".into(),
            session_start_tx: TxId::new(0, 0, agent),
            task_description: "test".to_string(),
            session_knowledge: vec![
                (
                    ":test/a".to_string(),
                    Value::String("high confidence".into()),
                ),
                (":test/b".to_string(), Value::String("also high".into())),
            ],
        };

        let result = harvest_pipeline(&store, &context);
        assert_eq!(result.quality.count, 2);
        assert_eq!(result.quality.high_confidence, 2);
        assert_eq!(result.quality.medium_confidence, 0);
        assert_eq!(result.quality.low_confidence, 0);
    }

    #[test]
    fn candidate_status_ordering() {
        assert!(CandidateStatus::Proposed < CandidateStatus::UnderReview);
        assert!(CandidateStatus::UnderReview < CandidateStatus::Committed);
        assert!(CandidateStatus::UnderReview < CandidateStatus::Rejected);
    }

    // -------------------------------------------------------------------
    // v2: Tx-log extraction tests
    // -------------------------------------------------------------------

    #[test]
    fn extract_session_profiles_finds_new_entities() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = Store::genesis();
        let agent = AgentId::from_name("test:harvest-v2");

        // Transact something at wall_time=1
        let entity = EntityId::from_ident(":test/session-entity");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "session work")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("session observation".into()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/session-entity".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        // Extract profiles for entities after the genesis tx (session start)
        let genesis_tx = TxId::new(0, 0, AgentId::from_name("braid:system"));
        let profiles = extract_session_profiles(&store, &genesis_tx);
        assert!(
            !profiles.is_empty(),
            "should find entities from session transactions"
        );

        // Find our entity's profile
        let our_profile = profiles.iter().find(|p| p.entity == entity);
        assert!(our_profile.is_some(), "should profile our session entity");
        let profile = our_profile.unwrap();
        assert!(profile.has_ident);
        assert_eq!(profile.ident.as_deref(), Some(":test/session-entity"));
    }

    #[test]
    fn classify_profile_structural() {
        let spec_profile = EntityProfile {
            entity: EntityId::from_ident(":test/spec"),
            attributes: BTreeSet::from([
                ":spec/id".to_string(),
                ":spec/element-type".to_string(),
                ":db/doc".to_string(),
            ]),
            namespace_counts: BTreeMap::from([(AttrNamespace::Spec, 2), (AttrNamespace::Meta, 1)]),
            has_ident: false,
            ident: None,
            ref_count: 0,
            datom_count: 3,
        };
        assert_eq!(
            classify_profile(&spec_profile),
            HarvestCategory::Observation
        );

        let decision_profile = EntityProfile {
            entity: EntityId::from_ident(":test/decision"),
            attributes: BTreeSet::from([
                ":intent/decision".to_string(),
                ":intent/rationale".to_string(),
            ]),
            namespace_counts: BTreeMap::from([(AttrNamespace::Intent, 2)]),
            has_ident: false,
            ident: None,
            ref_count: 0,
            datom_count: 2,
        };
        assert_eq!(
            classify_profile(&decision_profile),
            HarvestCategory::Decision
        );
    }

    #[test]
    fn score_profile_rewards_density() {
        let sparse = EntityProfile {
            entity: EntityId::from_ident(":test/sparse"),
            attributes: BTreeSet::from([":db/doc".to_string()]),
            namespace_counts: BTreeMap::from([(AttrNamespace::Meta, 1)]),
            has_ident: false,
            ident: None,
            ref_count: 0,
            datom_count: 1,
        };
        let dense = EntityProfile {
            entity: EntityId::from_ident(":test/dense"),
            attributes: BTreeSet::from([
                ":db/doc".to_string(),
                ":db/ident".to_string(),
                ":spec/id".to_string(),
                ":spec/element-type".to_string(),
                ":spec/namespace".to_string(),
            ]),
            namespace_counts: BTreeMap::from([(AttrNamespace::Meta, 2), (AttrNamespace::Spec, 3)]),
            has_ident: true,
            ident: Some(":test/dense".to_string()),
            ref_count: 2,
            datom_count: 5,
        };

        assert!(
            score_profile(&dense, HarvestCategory::Observation)
                > score_profile(&sparse, HarvestCategory::Observation),
            "denser entities should score higher"
        );
    }

    // -------------------------------------------------------------------
    // v2 Phase 3: Fisher scoring tests
    // -------------------------------------------------------------------

    #[test]
    fn fisher_rao_distance_self_is_zero() {
        let p = [0.25, 0.25, 0.25, 0.25];
        let d = fisher_rao_distance(&p, &p);
        assert!(d.abs() < 1e-10, "d(p,p) must be 0, got {d}");
    }

    #[test]
    fn fisher_rao_distance_symmetric() {
        let p = [0.6, 0.1, 0.1, 0.2];
        let q = [0.1, 0.4, 0.2, 0.3];
        let d_pq = fisher_rao_distance(&p, &q);
        let d_qp = fisher_rao_distance(&q, &p);
        assert!(
            (d_pq - d_qp).abs() < 1e-10,
            "Fisher-Rao must be symmetric: d(p,q)={d_pq}, d(q,p)={d_qp}"
        );
    }

    #[test]
    fn fisher_rao_distance_bounded_by_pi() {
        let p = [1.0, 0.0, 0.0, 0.0];
        let q = [0.0, 1.0, 0.0, 0.0];
        let d = fisher_rao_distance(&p, &q);
        assert!(
            d <= std::f64::consts::PI + 1e-10,
            "d_FR must be <= π, got {d}"
        );
    }

    #[test]
    fn fisher_score_decision_profile_high_confidence() {
        // Decision-like profile matching the ideal distribution
        let profile = EntityProfile {
            entity: EntityId::from_ident(":test/decision"),
            attributes: BTreeSet::from([
                ":intent/decision".to_string(),
                ":intent/rationale".to_string(),
                ":intent/alternatives".to_string(),
                ":db/doc".to_string(),
            ]),
            namespace_counts: BTreeMap::from([
                (AttrNamespace::Intent, 3),
                (AttrNamespace::Meta, 1),
            ]),
            has_ident: true,
            ident: Some(":test/decision".to_string()),
            ref_count: 0,
            datom_count: 4,
        };
        let score = score_profile(&profile, HarvestCategory::Decision);
        assert!(
            score > 0.5,
            "decision profile matching ideal should have high Fisher score, got {score}"
        );
    }

    #[test]
    fn fisher_score_mismatched_category_lower() {
        // Same profile scored as two different categories
        let profile = EntityProfile {
            entity: EntityId::from_ident(":test/impl"),
            attributes: BTreeSet::from([
                ":impl/file-path".to_string(),
                ":impl/function".to_string(),
                ":impl/implements".to_string(),
                ":db/ident".to_string(),
            ]),
            namespace_counts: BTreeMap::from([(AttrNamespace::Impl, 3), (AttrNamespace::Meta, 1)]),
            has_ident: true,
            ident: Some(":test/impl".to_string()),
            ref_count: 1,
            datom_count: 4,
        };

        let as_dep = score_profile(&profile, HarvestCategory::Dependency);
        let as_decision = score_profile(&profile, HarvestCategory::Decision);

        assert!(
            as_dep > as_decision,
            "impl-heavy profile should score higher as Dependency ({as_dep:.4}) than Decision ({as_decision:.4})"
        );
    }

    // -------------------------------------------------------------------
    // v2 Phase 4-5: Session entity + commit tests
    // -------------------------------------------------------------------

    #[test]
    fn build_harvest_commit_creates_session_entity() {
        let agent = AgentId::from_name("test:harvester");
        let tx_id = TxId::new(100, 0, agent);
        let result = HarvestResult {
            candidates: vec![HarvestCandidate {
                entity: EntityId::from_ident(":test/entity"),
                assertions: vec![(
                    Attribute::from_keyword(":db/doc"),
                    Value::String("test".into()),
                )],
                category: HarvestCategory::Observation,
                confidence: 0.9,
                status: CandidateStatus::Proposed,
                rationale: "test candidate".into(),
            }],
            drift_score: 0.5,
            quality: HarvestQuality {
                count: 1,
                high_confidence: 1,
                medium_confidence: 0,
                low_confidence: 0,
            },
            session_entities: 1,
            completeness_gaps: 0,
        };
        let context = SessionContext {
            agent,
            agent_name: "test:harvester".into(),
            session_start_tx: TxId::new(0, 0, agent),
            task_description: "test session".into(),
            session_knowledge: vec![],
        };

        let commit = build_harvest_commit(&result, &context, tx_id);

        // 1 candidate datom + 7 session entity datoms = 8
        assert_eq!(commit.datom_count, 8);
        assert_eq!(commit.candidate_count, 1);
        assert!(commit.session_ident.starts_with(":harvest/session-"));

        // Verify session entity has the right attributes
        let session_entity = EntityId::from_ident(&commit.session_ident);
        let session_datoms: Vec<&Datom> = commit
            .datoms
            .iter()
            .filter(|d| d.entity == session_entity)
            .collect();
        assert_eq!(
            session_datoms.len(),
            7,
            "session entity should have 7 datoms"
        );

        // Check :db/ident is present
        assert!(
            session_datoms
                .iter()
                .any(|d| d.attribute.as_str() == ":db/ident"),
            "session entity must have :db/ident"
        );
    }

    #[test]
    fn build_harvest_commit_all_assertions() {
        let agent = AgentId::from_name("test");
        let tx_id = TxId::new(50, 0, agent);
        let result = HarvestResult {
            candidates: vec![],
            drift_score: 0.0,
            quality: HarvestQuality::default(),
            session_entities: 0,
            completeness_gaps: 0,
        };
        let context = SessionContext {
            agent,
            agent_name: "test".into(),
            session_start_tx: TxId::new(0, 0, agent),
            task_description: "empty session".into(),
            session_knowledge: vec![],
        };

        let commit = build_harvest_commit(&result, &context, tx_id);

        // Even with no candidates, session entity is created
        assert_eq!(commit.candidate_count, 0);
        assert!(
            commit.datom_count > 0,
            "session entity datoms always present"
        );

        // All datoms are assertions
        assert!(
            commit.datoms.iter().all(|d| d.op == Op::Assert),
            "harvest commit must only contain assertions (INV-HARVEST-002)"
        );
    }

    #[test]
    fn detect_gaps_finds_missing_spec_attrs() {
        let profile = EntityProfile {
            entity: EntityId::from_ident(":test/incomplete-spec"),
            attributes: BTreeSet::from([":spec/element-type".to_string()]),
            namespace_counts: BTreeMap::from([(AttrNamespace::Spec, 1)]),
            has_ident: false,
            ident: None,
            ref_count: 0,
            datom_count: 1,
        };
        let gaps = detect_gaps(&profile, HarvestCategory::Observation);
        assert!(gaps.contains(&":spec/id".to_string()));
        assert!(gaps.contains(&":db/doc".to_string()));
    }

    #[test]
    fn harvest_v2_tx_log_integration() {
        use crate::datom::ProvenanceType;
        use crate::schema::full_schema_datoms;
        use crate::store::Transaction;

        // Build store with full schema so L2 attributes are available
        let system_agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, system_agent);
        let schema_datoms = full_schema_datoms(genesis_tx);
        let mut datom_set = std::collections::BTreeSet::new();
        for d in schema_datoms {
            datom_set.insert(d);
        }
        let mut store = Store::from_datoms(datom_set);

        let agent = AgentId::from_name("test:harvest-v2-int");

        // Create a spec entity with incomplete attributes
        let entity = EntityId::from_ident(":test/inv-test-001");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "spec creation")
            .assert(
                entity,
                Attribute::from_keyword(":spec/element-type"),
                Value::Keyword(":element.type/invariant".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        // Run harvest with session start at genesis
        let context = SessionContext {
            agent,
            agent_name: "test:harvest-v2-int".into(),
            session_start_tx: genesis_tx,
            task_description: "test session".to_string(),
            session_knowledge: vec![],
        };

        let result = harvest_pipeline(&store, &context);

        // Should find the entity we created
        assert!(result.session_entities > 0, "should find session entities");
        // Should detect completeness gaps (missing :spec/id, :db/doc)
        assert!(
            result.completeness_gaps > 0,
            "should detect completeness gaps for incomplete spec entity"
        );
    }

    // -------------------------------------------------------------------
    // Phase 6: FP/FN Calibration tests (INV-HARVEST-004)
    // -------------------------------------------------------------------

    #[test]
    fn calibration_perfect_precision_recall() {
        let e1 = EntityId::from_ident(":test/cal-a");
        let e2 = EntityId::from_ident(":test/cal-b");

        let result = HarvestResult {
            candidates: vec![
                HarvestCandidate {
                    entity: e1,
                    assertions: vec![],
                    category: HarvestCategory::Observation,
                    confidence: 0.9,
                    status: CandidateStatus::Proposed,
                    rationale: "test".into(),
                },
                HarvestCandidate {
                    entity: e2,
                    assertions: vec![],
                    category: HarvestCategory::Decision,
                    confidence: 0.8,
                    status: CandidateStatus::Proposed,
                    rationale: "test".into(),
                },
            ],
            drift_score: 0.5,
            quality: HarvestQuality {
                count: 2,
                high_confidence: 2,
                medium_confidence: 0,
                low_confidence: 0,
            },
            session_entities: 2,
            completeness_gaps: 0,
        };

        let ground_truth: BTreeSet<EntityId> = BTreeSet::from([e1, e2]);
        let cal = calibrate_harvest(&result, &ground_truth, 10);

        assert_eq!(cal.true_positives, 2);
        assert_eq!(cal.false_positives, 0);
        assert_eq!(cal.false_negatives, 0);
        assert!((cal.precision - 1.0).abs() < 1e-10);
        assert!((cal.recall - 1.0).abs() < 1e-10);
        assert!((cal.f1_score - 1.0).abs() < 1e-10);
    }

    #[test]
    fn calibration_with_false_positives() {
        let e1 = EntityId::from_ident(":test/fp-a");
        let e2 = EntityId::from_ident(":test/fp-b");
        let e3 = EntityId::from_ident(":test/fp-noise");

        let result = HarvestResult {
            candidates: vec![
                HarvestCandidate {
                    entity: e1,
                    assertions: vec![],
                    category: HarvestCategory::Observation,
                    confidence: 0.9,
                    status: CandidateStatus::Proposed,
                    rationale: "real".into(),
                },
                HarvestCandidate {
                    entity: e3,
                    assertions: vec![],
                    category: HarvestCategory::Observation,
                    confidence: 0.3,
                    status: CandidateStatus::Proposed,
                    rationale: "noise".into(),
                },
            ],
            drift_score: 0.5,
            quality: HarvestQuality {
                count: 2,
                high_confidence: 1,
                medium_confidence: 0,
                low_confidence: 1,
            },
            session_entities: 2,
            completeness_gaps: 0,
        };

        let ground_truth = BTreeSet::from([e1, e2]);
        let cal = calibrate_harvest(&result, &ground_truth, 10);

        assert_eq!(cal.true_positives, 1); // e1
        assert_eq!(cal.false_positives, 1); // e3
        assert_eq!(cal.false_negatives, 1); // e2
        assert!((cal.precision - 0.5).abs() < 1e-10);
        assert!((cal.recall - 0.5).abs() < 1e-10);
    }

    #[test]
    fn calibration_mcc_perfect() {
        let e1 = EntityId::from_ident(":test/mcc-a");
        let result = HarvestResult {
            candidates: vec![HarvestCandidate {
                entity: e1,
                assertions: vec![],
                category: HarvestCategory::Observation,
                confidence: 0.9,
                status: CandidateStatus::Proposed,
                rationale: "test".into(),
            }],
            drift_score: 0.5,
            quality: HarvestQuality {
                count: 1,
                high_confidence: 1,
                medium_confidence: 0,
                low_confidence: 0,
            },
            session_entities: 1,
            completeness_gaps: 0,
        };
        let ground_truth = BTreeSet::from([e1]);
        let cal = calibrate_harvest(&result, &ground_truth, 5);
        // Perfect prediction → MCC should be close to 1.0
        assert!(
            cal.mcc > 0.5,
            "perfect prediction MCC should be high, got {}",
            cal.mcc
        );
    }

    #[test]
    fn calibration_empty_candidates_empty_truth() {
        let result = HarvestResult {
            candidates: vec![],
            drift_score: 0.0,
            quality: HarvestQuality::default(),
            session_entities: 0,
            completeness_gaps: 0,
        };
        let ground_truth = BTreeSet::new();
        let cal = calibrate_harvest(&result, &ground_truth, 10);
        assert_eq!(cal.true_positives, 0);
        assert_eq!(cal.false_positives, 0);
        assert_eq!(cal.false_negatives, 0);
        assert!((cal.precision - 1.0).abs() < 1e-10, "vacuous precision");
        assert!((cal.recall - 1.0).abs() < 1e-10, "vacuous recall");
    }

    #[test]
    fn calibration_stratified_monotone_recall() {
        // Higher thresholds → fewer candidates → recall non-increasing
        let e1 = EntityId::from_ident(":test/strat-a");
        let e2 = EntityId::from_ident(":test/strat-b");
        let result = HarvestResult {
            candidates: vec![
                HarvestCandidate {
                    entity: e1,
                    assertions: vec![],
                    category: HarvestCategory::Observation,
                    confidence: 0.9,
                    status: CandidateStatus::Proposed,
                    rationale: "high".into(),
                },
                HarvestCandidate {
                    entity: e2,
                    assertions: vec![],
                    category: HarvestCategory::Observation,
                    confidence: 0.4,
                    status: CandidateStatus::Proposed,
                    rationale: "low".into(),
                },
            ],
            drift_score: 0.5,
            quality: HarvestQuality {
                count: 2,
                high_confidence: 1,
                medium_confidence: 0,
                low_confidence: 1,
            },
            session_entities: 2,
            completeness_gaps: 0,
        };
        let ground_truth = BTreeSet::from([e1, e2]);
        let cal = calibrate_harvest(&result, &ground_truth, 10);

        // Recall at lower threshold >= recall at higher threshold
        for window in cal.stratified.windows(2) {
            let (_, _, r_low) = window[0];
            let (_, _, r_high) = window[1];
            assert!(
                r_low >= r_high - 1e-10,
                "recall must be non-increasing with threshold: {} < {}",
                r_low,
                r_high
            );
        }
    }

    #[test]
    fn optimal_threshold_finds_best_f1() {
        let e1 = EntityId::from_ident(":test/opt-a");
        let e2 = EntityId::from_ident(":test/opt-b");
        let noise = EntityId::from_ident(":test/opt-noise");

        let result = HarvestResult {
            candidates: vec![
                HarvestCandidate {
                    entity: e1,
                    assertions: vec![],
                    category: HarvestCategory::Observation,
                    confidence: 0.95,
                    status: CandidateStatus::Proposed,
                    rationale: "real".into(),
                },
                HarvestCandidate {
                    entity: e2,
                    assertions: vec![],
                    category: HarvestCategory::Decision,
                    confidence: 0.7,
                    status: CandidateStatus::Proposed,
                    rationale: "real".into(),
                },
                HarvestCandidate {
                    entity: noise,
                    assertions: vec![],
                    category: HarvestCategory::Observation,
                    confidence: 0.2,
                    status: CandidateStatus::Proposed,
                    rationale: "noise".into(),
                },
            ],
            drift_score: 0.5,
            quality: HarvestQuality {
                count: 3,
                high_confidence: 1,
                medium_confidence: 1,
                low_confidence: 1,
            },
            session_entities: 3,
            completeness_gaps: 0,
        };
        let ground_truth = BTreeSet::from([e1, e2]);
        let (threshold, f1) = optimal_threshold(&result, &ground_truth);

        // Best threshold should exclude noise (0.2) but include both real entities
        // At threshold = 0.7: TP=2, FP=0, FN=0, F1=1.0
        assert!(f1 >= 0.9, "optimal F₁ should be near-perfect, got {f1}");
        assert!(threshold >= 0.2, "threshold should be above noise level");
    }

    // -------------------------------------------------------------------
    // Property-based tests (proptest)
    // -------------------------------------------------------------------

    mod proptests {
        use super::*;
        use crate::proptest_strategies::*;
        use proptest::prelude::*;

        fn arb_session_knowledge(max_items: usize) -> impl Strategy<Value = Vec<(String, Value)>> {
            proptest::collection::vec(
                (
                    "[a-z]{3,8}".prop_map(|s| format!(":session/{s}")),
                    arb_doc_value(),
                ),
                0..=max_items,
            )
        }

        fn arb_session_context() -> impl Strategy<Value = SessionContext> {
            (arb_agent_id(), arb_tx_id(), arb_session_knowledge(5)).prop_map(
                |(agent, tx, knowledge)| SessionContext {
                    agent,
                    agent_name: "proptest-agent".into(),
                    session_start_tx: tx,
                    task_description: "proptest session".to_string(),
                    session_knowledge: knowledge,
                },
            )
        }

        proptest! {
            #[test]
            fn harvest_pipeline_always_returns_result(ctx in arb_session_context()) {
                let store = Store::genesis();
                let result = harvest_pipeline(&store, &ctx);
                // harvest_pipeline must always produce a HarvestResult
                // (never panic, always structurally valid)
                let _ = result.candidates.len();
                let _ = result.drift_score;
                let _ = result.quality.count;
                let _ = result.session_entities;
                let _ = result.completeness_gaps;
            }

            #[test]
            fn drift_score_in_unit_interval(ctx in arb_session_context()) {
                let store = Store::genesis();
                let result = harvest_pipeline(&store, &ctx);
                prop_assert!(
                    result.drift_score >= 0.0 && result.drift_score <= 1.0,
                    "drift_score must be in [0.0, 1.0], got {}",
                    result.drift_score
                );
            }

            #[test]
            fn quality_counts_sum_correctly(ctx in arb_session_context()) {
                let store = Store::genesis();
                let result = harvest_pipeline(&store, &ctx);
                let q = &result.quality;
                prop_assert_eq!(
                    q.high_confidence + q.medium_confidence + q.low_confidence,
                    q.count,
                    "quality confidence buckets must sum to total count"
                );
            }

            #[test]
            fn candidate_to_datoms_produces_valid_datoms(ctx in arb_session_context()) {
                let store = Store::genesis();
                let result = harvest_pipeline(&store, &ctx);
                let agent = AgentId::from_name("proptest:harvester");
                let tx = TxId::new(1000, 0, agent);

                for candidate in &result.candidates {
                    let datoms = candidate_to_datoms(candidate, tx);

                    // Each datom must be an assertion
                    for d in &datoms {
                        prop_assert_eq!(
                            d.op,
                            crate::datom::Op::Assert,
                            "candidate_to_datoms must only produce assertions"
                        );
                        prop_assert_eq!(
                            d.entity, candidate.entity,
                            "datom entity must match candidate entity"
                        );
                        prop_assert_eq!(
                            d.tx, tx,
                            "datom tx must match supplied tx"
                        );
                    }

                    // Number of datoms must equal number of assertions in the candidate
                    prop_assert_eq!(
                        datoms.len(),
                        candidate.assertions.len(),
                        "datom count must match assertion count"
                    );
                }
            }

            /// INV-HARVEST-004: Calibration metrics are bounded and consistent.
            #[test]
            fn calibration_metrics_bounded(ctx in arb_session_context()) {
                let store = Store::genesis();
                let result = harvest_pipeline(&store, &ctx);

                // Use candidate entities as "ground truth" (perfect recall scenario)
                let ground_truth: BTreeSet<EntityId> =
                    result.candidates.iter().map(|c| c.entity).collect();
                let cal = calibrate_harvest(&result, &ground_truth, store.entity_count());

                prop_assert!(
                    cal.precision >= 0.0 && cal.precision <= 1.0,
                    "precision must be in [0, 1], got {}",
                    cal.precision
                );
                prop_assert!(
                    cal.recall >= 0.0 && cal.recall <= 1.0,
                    "recall must be in [0, 1], got {}",
                    cal.recall
                );
                prop_assert!(
                    cal.f1_score >= 0.0 && cal.f1_score <= 1.0,
                    "F₁ must be in [0, 1], got {}",
                    cal.f1_score
                );
                prop_assert!(
                    cal.mcc >= -1.0 && cal.mcc <= 1.0 + 1e-10,
                    "MCC must be in [-1, 1], got {}",
                    cal.mcc
                );
            }

            /// v2: Session entities count is non-negative and bounded.
            #[test]
            fn session_entities_bounded(ctx in arb_session_context()) {
                let store = Store::genesis();
                let result = harvest_pipeline(&store, &ctx);
                // Session entities can be 0 (genesis only) or > 0
                // but must be bounded by total store entities
                prop_assert!(
                    result.session_entities <= store.entity_count(),
                    "session_entities must be <= total entities"
                );
            }
        }
    }
}
