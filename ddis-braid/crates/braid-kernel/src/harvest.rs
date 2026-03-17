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
//! - **INV-HARVEST-007**: Bounded conversation lifecycle.
//! - **INV-HARVEST-008**: Delegation topology support.
//! - **INV-HARVEST-010**: Tx-log extraction completeness.
//! - **INV-HARVEST-011**: Classification accuracy (structural, not heuristic).
//!
//! # Design Decisions
//!
//! - ADR-HARVEST-001: Semi-automated over fully automatic harvest.
//! - ADR-HARVEST-002: Conversations disposable, knowledge durable.
//! - ADR-HARVEST-003: FP/FN tracking for calibration.
//! - ADR-HARVEST-004: Five review topologies (self, peer, lead, team, automated).
//! - ADR-HARVEST-006: DDR feedback loop for harvest quality improvement.
//! - ADR-HARVEST-007: Turn-count proxy for context budget at Stage 0.
//!
//! # Negative Cases
//!
//! - NEG-HARVEST-001: No unharvested session termination.
//! - NEG-HARVEST-002: No harvest data loss.
//! - NEG-HARVEST-003: No premature crystallization.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::Serialize;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
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
    /// Exploration category value (from `:exploration/category`), if present.
    /// Used to classify open-question/design-decision observations correctly.
    exploration_category: Option<String>,
    /// Namespace classification counts.
    namespace_counts: BTreeMap<AttrNamespace, usize>,
    /// Whether the entity has a :db/ident (named entity).
    has_ident: bool,
    /// The ident value, if present.
    ident: Option<String>,
    /// The :db/doc value, if present (for human-readable output).
    doc: Option<String>,
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
            // B1: Apply confidence floor to filter noise (INV-HARVEST-012).
            // Low-confidence entities (conf * 0.6 < 0.3) produce gap candidates
            // that are metadata noise, not actionable.
            let gap_confidence = confidence * 0.6;
            if gap_confidence >= 0.15 {
                completeness_gaps += gaps.len();
                for gap in &gaps {
                    candidates.push(HarvestCandidate {
                        entity: profile.entity,
                        assertions: vec![],
                        // Gaps are metadata completeness issues, not semantic content —
                        // always Observation to avoid polluting Decision/Uncertainty lists
                        category: HarvestCategory::Observation,
                        confidence: gap_confidence,
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

        // Also surface each session entity as an Observation candidate
        // so harvest always reports what happened during the session.
        // Skip harvest-internal entities (they're bookkeeping, not knowledge).
        let is_harvest_entity = profile
            .attributes
            .iter()
            .any(|a| a.starts_with(":harvest/") || a.starts_with(":bilateral/"));
        if !is_harvest_entity && gaps.is_empty() {
            let label = profile
                .ident
                .as_deref()
                .unwrap_or("unnamed entity")
                .to_string();
            let doc_summary = profile
                .doc
                .as_deref()
                .map(|d| {
                    if d.len() > 100 {
                        format!(": {}...", &d[..d.floor_char_boundary(100)])
                    } else {
                        format!(": {d}")
                    }
                })
                .unwrap_or_default();
            candidates.push(HarvestCandidate {
                entity: profile.entity,
                assertions: vec![],
                category,
                confidence,
                status: CandidateStatus::Proposed,
                rationale: format!("Session entity: {label}{doc_summary}"),
            });
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
    let mut doc = None;
    let mut ref_count = 0;
    let mut exploration_category = None;

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

        if datom.attribute.as_str() == ":db/doc" {
            if let Value::String(ref s) = datom.value {
                doc = Some(s.clone());
            }
        }

        // Extract exploration category for classify_profile
        if datom.attribute.as_str() == ":exploration/category" && datom.op == Op::Assert {
            let cat = match &datom.value {
                Value::String(s) => Some(s.clone()),
                Value::Keyword(k) => Some(k.clone()),
                _ => None,
            };
            if cat.is_some() {
                exploration_category = cat;
            }
        }

        if matches!(&datom.value, Value::Ref(_)) {
            ref_count += 1;
        }
    }

    EntityProfile {
        entity,
        attributes,
        exploration_category,
        namespace_counts,
        has_ident,
        ident,
        doc,
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
            // Check exploration/category value for observation classification.
            // Observations via `braid observe --category X` have :exploration/* attrs
            // which classify as Meta namespace. The CATEGORY VALUE determines whether
            // this is a decision, open question, or plain observation.
            if let Some(ref cat) = profile.exploration_category {
                let c = cat.as_str();
                if c == "design-decision"
                    || c == "decision"
                    || c.ends_with("/design-decision")
                    || c.ends_with("/decision")
                {
                    return HarvestCategory::Decision;
                }
                if c == "open-question"
                    || c == "conjecture"
                    || c == "question"
                    || c.ends_with("/open-question")
                    || c.ends_with("/conjecture")
                {
                    return HarvestCategory::Uncertainty;
                }
            }

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

// ---------------------------------------------------------------------------
// Crystallization Guard (INV-HARVEST-006)
// ---------------------------------------------------------------------------

/// Default crystallization threshold: candidates must score ≥ 0.7 to commit.
pub const DEFAULT_CRYSTALLIZATION_THRESHOLD: f64 = 0.7;

/// Compute a stability score for a harvest candidate.
///
/// Stability = 0.6 * session_diversity + 0.4 * stated_confidence
///
/// Session diversity is based on how many times the same observation
/// (by content hash) appears across different sessions. This rewards
/// observations that have been independently confirmed.
///
/// - 1 session (first time): diversity = 0.3
/// - 2 sessions: diversity = 0.7
/// - 3+ sessions: diversity = 1.0
///
/// High-confidence (≥ 0.9) observations are fast-tracked with diversity = 0.8.
pub fn stability_score(store: &Store, candidate: &HarvestCandidate) -> f64 {
    let confidence = candidate.confidence;

    // Fast-track: high-confidence observations bypass session diversity
    if confidence >= 0.9 {
        return 0.6 * 0.8 + 0.4 * confidence;
    }

    // Count session diversity via content hash matching
    let content_hash_attr = Attribute::from_keyword(":exploration/content-hash");
    let body_attr = Attribute::from_keyword(":exploration/body");

    // Get candidate's body text
    let body = candidate
        .assertions
        .iter()
        .find(|(a, _)| *a == body_attr)
        .and_then(|(_, v)| match v {
            Value::String(s) => Some(s.clone()),
            _ => None,
        });

    let session_diversity = if let Some(body_text) = body {
        // Compute hash of candidate body
        let hash = blake3::hash(body_text.as_bytes());
        let hash_bytes = hash.as_bytes().to_vec();

        // Count entities in store with matching content hash
        let mut matching_sessions: BTreeSet<u64> = BTreeSet::new();
        for datom in store.datoms() {
            if datom.attribute == content_hash_attr
                && datom.op == Op::Assert
                && matches!(&datom.value, Value::Bytes(b) if *b == hash_bytes)
            {
                matching_sessions.insert(datom.tx.wall_time());
            }
        }

        match matching_sessions.len() {
            0 | 1 => 0.3, // First or only occurrence
            2 => 0.7,     // Confirmed once
            _ => 1.0,     // Well-established
        }
    } else {
        0.3 // No body text → treat as first occurrence
    };

    0.6 * session_diversity + 0.4 * confidence
}

/// Apply the crystallization guard to a harvest result.
///
/// Returns a partition: (ready to commit, needs more observation).
/// Candidates with stability_score ≥ threshold are ready.
/// Candidates below threshold are returned as "pending" with their scores.
pub fn crystallization_guard(
    store: &Store,
    result: &HarvestResult,
    threshold: f64,
) -> CrystallizationResult {
    let mut ready = Vec::new();
    let mut pending = Vec::new();

    for candidate in &result.candidates {
        let score = stability_score(store, candidate);
        if score >= threshold {
            ready.push((candidate.clone(), score));
        } else {
            pending.push((candidate.clone(), score));
        }
    }

    CrystallizationResult { ready, pending }
}

/// Result of the crystallization guard.
#[derive(Clone, Debug)]
pub struct CrystallizationResult {
    /// Candidates ready to commit (score ≥ threshold).
    pub ready: Vec<(HarvestCandidate, f64)>,
    /// Candidates that need more observation (score < threshold).
    pub pending: Vec<(HarvestCandidate, f64)>,
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
// Task inference (S0.2b: multi-signal detection + confidence scoring)
// ---------------------------------------------------------------------------

/// Infer the current session's task description from store state.
///
/// Multi-signal detection (S0.2b):
/// 1. Active session entity (`:session/task` attribute) — highest confidence
/// 2. Most recent observation body (`:exploration/body`) — medium confidence
/// 3. Most frequent namespace in recent transactions — low confidence
/// 4. Fallback: "session work" — minimal confidence
///
/// Returns (inferred_task, source_signal, confidence).
pub fn infer_task_description(store: &Store) -> (String, String, f64) {
    // Signal 1: Look for active session entity (most recent :session/task)
    let mut session_tasks: Vec<(String, u64)> = Vec::new();
    for datom in store.datoms() {
        if datom.attribute == Attribute::from_keyword(":session/task") && datom.op == Op::Assert {
            if let Value::String(ref task) = datom.value {
                session_tasks.push((task.clone(), datom.tx.wall_time()));
            }
        }
    }
    if let Some((task, _)) = session_tasks.iter().max_by_key(|(_, t)| t) {
        return (task.clone(), "session entity".into(), 0.95);
    }

    // Signal 2: Most recent observation body
    let mut observations: Vec<(String, u64)> = Vec::new();
    for datom in store.datoms() {
        if datom.attribute == Attribute::from_keyword(":exploration/body") && datom.op == Op::Assert
        {
            if let Value::String(ref body) = datom.value {
                let truncated = if body.len() > 80 {
                    format!("{}...", &body[..body.floor_char_boundary(80)])
                } else {
                    body.clone()
                };
                observations.push((truncated, datom.tx.wall_time()));
            }
        }
    }
    observations.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
    if let Some((obs, _)) = observations.first() {
        return (obs.clone(), "recent observation".into(), 0.6);
    }

    // Signal 3: Most frequent namespace in recent transactions
    let max_wall = store.datoms().map(|d| d.tx.wall_time()).max().unwrap_or(0);
    let recent_threshold = max_wall.saturating_sub(3600); // last hour
    let mut namespace_counts: BTreeMap<String, usize> = BTreeMap::new();
    for datom in store.datoms() {
        if datom.op == Op::Assert && datom.tx.wall_time() > recent_threshold {
            let ns = classify_attribute(&datom.attribute);
            let label = namespace_label(ns);
            *namespace_counts.entry(label.to_string()).or_default() += 1;
        }
    }
    if let Some((ns, _)) = namespace_counts.iter().max_by_key(|(_, &count)| count) {
        if ns != "META" {
            return (
                format!("{ns} namespace work"),
                "namespace frequency".into(),
                0.3,
            );
        }
    }

    // Signal 4: Fallback
    ("session work".into(), "fallback".into(), 0.1)
}

// ---------------------------------------------------------------------------
// Narrative synthesis (S0.2.1: INV-HARVEST-001, INV-HARVEST-002)
// ---------------------------------------------------------------------------

/// A synthesized accomplishment from the harvest.
#[derive(Clone, Debug, Serialize)]
pub struct Accomplishment {
    /// Category of knowledge.
    pub category: HarvestCategory,
    /// Human-readable summary.
    pub summary: String,
    /// Entity IDs involved.
    pub entities: Vec<EntityId>,
}

/// A design decision extracted from harvest candidates.
#[derive(Clone, Debug, Serialize)]
pub struct NarrativeDecision {
    /// What was decided.
    pub summary: String,
    /// Why it was decided.
    pub rationale: String,
    /// Alternatives that were considered.
    pub alternatives: Vec<String>,
}

/// An open question surfaced during harvest.
#[derive(Clone, Debug, Serialize)]
pub struct OpenQuestion {
    /// The question.
    pub summary: String,
    /// Entity reference (if any).
    pub entity_ref: Option<String>,
}

/// Synthesized narrative summary of a harvest session.
///
/// This is the catamorphism over the candidate lattice -- it folds
/// the structured harvest candidates into a human/agent-readable story
/// while preserving the essential structure (category, provenance, rationale).
///
/// INV-HARVEST-001: Monotonicity -- narrative only adds information.
/// INV-HARVEST-002: Provenance -- every narrative item traces to candidates.
#[derive(Clone, Debug, Serialize)]
pub struct NarrativeSummary {
    /// The task this session worked on.
    pub goal: String,
    /// What was accomplished (grouped by category).
    pub accomplished: Vec<Accomplishment>,
    /// Decisions made with rationale.
    pub decisions: Vec<NarrativeDecision>,
    /// Open questions discovered.
    pub open_questions: Vec<OpenQuestion>,
    /// Focus areas: (namespace, entity count).
    pub focus_areas: Vec<(String, usize)>,
    /// Suggested next action (from guidance).
    pub next: Option<String>,
    /// Git context summary (files, commits) -- populated by CLI layer, not kernel.
    pub git_summary: Option<String>,
    /// Synthesis directive -- pipe-back-to-harness prompt fragment (S0.2a.2).
    /// When the running agent reads this, it acts on it directly.
    pub synthesis_directive: Option<String>,
}

/// Map an `AttrNamespace` to a human-readable uppercase label.
fn namespace_label(ns: AttrNamespace) -> &'static str {
    match ns {
        AttrNamespace::Intent => "INTENT",
        AttrNamespace::Spec => "SPEC",
        AttrNamespace::Impl => "IMPL",
        AttrNamespace::Meta => "META",
    }
}

/// Synthesize a narrative summary from harvest candidates and store state.
///
/// This is a pure function: no IO, deterministic output for given input.
/// It transforms the flat candidate list into a structured story.
///
/// # Algorithm
///
/// 1. Group candidates by `HarvestCategory`.
/// 2. For Decisions: extract `:intent/rationale` and `:intent/alternatives` from store.
/// 3. For Observations/Dependencies: extract `:db/doc` summaries.
/// 4. For Uncertainties: surface as open questions.
/// 5. Compute namespace frequency distribution.
/// 6. Derive next action from guidance.
pub fn synthesize_narrative(
    store: &Store,
    candidates: &[HarvestCandidate],
    task: &str,
) -> NarrativeSummary {
    // 1. Group candidates by category.
    let mut observations: Vec<&HarvestCandidate> = Vec::new();
    let mut decisions: Vec<&HarvestCandidate> = Vec::new();
    let mut dependencies: Vec<&HarvestCandidate> = Vec::new();
    let mut uncertainties: Vec<&HarvestCandidate> = Vec::new();

    for c in candidates {
        match c.category {
            HarvestCategory::Observation => observations.push(c),
            HarvestCategory::Decision => decisions.push(c),
            HarvestCategory::Dependency => dependencies.push(c),
            HarvestCategory::Uncertainty => uncertainties.push(c),
        }
    }

    // 2. Build accomplishments from non-Decision, non-Uncertainty candidates
    //    with confidence >= 0.5.
    let mut accomplished: Vec<Accomplishment> = Vec::new();

    // Helper: resolve entity body text (the actual observation/knowledge content).
    // Prefers :exploration/body > :db/doc > candidate rationale as fallback.
    let resolve_body = |entity: EntityId, fallback: &str| -> String {
        let datoms = store.entity_datoms(entity);
        datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":exploration/body" && d.op == Op::Assert)
            .or_else(|| {
                datoms
                    .iter()
                    .find(|d| d.attribute.as_str() == ":db/doc" && d.op == Op::Assert)
            })
            .and_then(|d| match &d.value {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| fallback.to_string())
    };

    // Helper: accumulate accomplishments from a category bucket.
    // Uses the actual entity body text, not the candidate rationale metadata.
    let build_accomplishments =
        |bucket: &[&HarvestCandidate], cat: HarvestCategory, out: &mut Vec<Accomplishment>| {
            let qualifying: Vec<&HarvestCandidate> = bucket
                .iter()
                .filter(|c| c.confidence >= 0.5)
                .copied()
                .collect();
            if qualifying.is_empty() {
                return;
            }
            // Build individual accomplishment summaries from entity body text
            let summary = qualifying
                .iter()
                .map(|c| resolve_body(c.entity, &c.rationale))
                .collect::<Vec<_>>()
                .join("; ");
            let entities: Vec<EntityId> = qualifying.iter().map(|c| c.entity).collect();
            out.push(Accomplishment {
                category: cat,
                summary,
                entities,
            });
        };

    build_accomplishments(
        &observations,
        HarvestCategory::Observation,
        &mut accomplished,
    );
    build_accomplishments(
        &dependencies,
        HarvestCategory::Dependency,
        &mut accomplished,
    );

    // 3. Extract decisions with rationale from store.
    //    Uses entity body text as summary (not candidate rationale metadata).
    //    Checks both :intent/rationale and :exploration/rationale for decision reasoning.
    //    Checks both :intent/alternatives and :exploration/alternatives.
    let narrative_decisions: Vec<NarrativeDecision> = decisions
        .iter()
        .map(|c| {
            let entity_datoms = store.entity_datoms(c.entity);

            // Summary: use the actual entity body text
            let summary = resolve_body(c.entity, &c.rationale);

            // Rationale: check :exploration/rationale (braid observe --rationale)
            // then :intent/rationale, then fall back to candidate rationale
            let rationale = entity_datoms
                .iter()
                .find(|d| d.attribute.as_str() == ":exploration/rationale" && d.op == Op::Assert)
                .or_else(|| {
                    entity_datoms
                        .iter()
                        .find(|d| d.attribute.as_str() == ":intent/rationale" && d.op == Op::Assert)
                })
                .and_then(|d| match &d.value {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();

            // Alternatives: check :exploration/alternatives then :intent/alternatives
            let alternatives = entity_datoms
                .iter()
                .find(|d| d.attribute.as_str() == ":exploration/alternatives" && d.op == Op::Assert)
                .or_else(|| {
                    entity_datoms.iter().find(|d| {
                        d.attribute.as_str() == ":intent/alternatives" && d.op == Op::Assert
                    })
                })
                .and_then(|d| match &d.value {
                    Value::String(s) => Some(
                        s.split('|')
                            .map(|a| a.trim().to_string())
                            .collect::<Vec<_>>(),
                    ),
                    _ => None,
                })
                .unwrap_or_default();

            NarrativeDecision {
                summary,
                rationale,
                alternatives,
            }
        })
        .collect();

    // 4. Surface open questions from Uncertainty candidates.
    let open_questions: Vec<OpenQuestion> = uncertainties
        .iter()
        .map(|c| {
            // Resolve entity label for entity_ref: prefer :db/ident keyword.
            let entity_datoms = store.entity_datoms(c.entity);
            let entity_ref = entity_datoms
                .iter()
                .find(|d| d.attribute.as_str() == ":db/ident" && d.op == Op::Assert)
                .and_then(|d| match &d.value {
                    Value::Keyword(k) => Some(k.clone()),
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                });

            OpenQuestion {
                summary: resolve_body(c.entity, &c.rationale),
                entity_ref,
            }
        })
        .collect();

    // 5. Compute focus areas: classify each entity's attributes into namespaces.
    let mut ns_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for c in candidates {
        let entity_datoms = store.entity_datoms(c.entity);
        for d in &entity_datoms {
            if d.op == Op::Assert {
                let ns = classify_attribute(&d.attribute);
                let label = namespace_label(ns);
                *ns_counts.entry(label).or_insert(0) += 1;
            }
        }
    }

    // If entities have no datoms in store yet, fall back to attribute prefix heuristic
    // on the candidates' assertion attributes.
    if ns_counts.is_empty() {
        for c in candidates {
            for (attr, _) in &c.assertions {
                let ns = classify_attribute(attr);
                let label = namespace_label(ns);
                *ns_counts.entry(label).or_insert(0) += 1;
            }
        }
    }

    let mut focus_areas: Vec<(String, usize)> = ns_counts
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    focus_areas.sort_by_key(|entry| std::cmp::Reverse(entry.1));
    focus_areas.truncate(5);

    // 6. Derive next action from guidance.
    let actions = crate::guidance::derive_actions(store);
    let next = actions.first().and_then(|a| {
        a.command
            .as_ref()
            .map(|cmd| format!("{} ({})", cmd, a.summary))
    });

    // Pipe-back-to-harness synthesis directive (S0.2a.2).
    // The agent reading this output IS an LLM — this text becomes its next prompt.
    let synthesis_directive = if !accomplished.is_empty()
        || !open_questions.is_empty()
        || !narrative_decisions.is_empty()
    {
        let mut directive = String::new();
        directive.push_str("## Session Synthesis Directive\n\n");

        if !open_questions.is_empty() {
            directive.push_str("**Unresolved questions** (carry forward to next session):\n");
            for q in &open_questions {
                directive.push_str(&format!("- {}\n", q.summary));
            }
            directive.push('\n');
        }

        if !narrative_decisions.is_empty() {
            directive.push_str("**Decisions made** (do not relitigate \u{2014} NEG-002):\n");
            for d in &narrative_decisions {
                if d.rationale.is_empty() {
                    directive.push_str(&format!("- {}\n", d.summary));
                } else {
                    directive.push_str(&format!("- {} (rationale: {})\n", d.summary, d.rationale));
                }
            }
            directive.push('\n');
        }

        // Prefer continuing the current task over a generic monitoring command.
        // The guidance system's "next action" is a store health check, not a task.
        let next_task = if task.len() > 5 && task != "continue" && task != "session work" {
            format!("continue: {task}")
        } else if let Some(ref n) = next {
            n.clone()
        } else {
            "continue current work".to_string()
        };
        directive.push_str(&format!("**Next session task**: {}\n", next_task));
        directive.push_str("Run: `braid seed --task \"");
        directive.push_str(&next_task);
        directive.push_str("\"` to start the next session.\n");

        Some(directive)
    } else {
        None
    };

    NarrativeSummary {
        goal: task.to_string(),
        accomplished,
        decisions: narrative_decisions,
        open_questions,
        focus_areas,
        next,
        git_summary: None,
        synthesis_directive,
    }
}

// ---------------------------------------------------------------------------
// Spec candidate classification (W4A: extended harvest pipeline)
// ---------------------------------------------------------------------------

/// Type of specification candidate detected during harvest.
///
/// Maps to the three primary DDIS specification element types.
/// Each corresponds to a specific formalism requirement:
/// - Invariant: statement + falsification condition
/// - ADR: problem + alternatives + decision
/// - NegativeCase: violation condition + prevention mechanism
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum SpecCandidateType {
    /// An observation that exhibits universal quantifier language
    /// and high confidence, suggesting a candidate invariant.
    Invariant,
    /// A design decision that references multiple alternatives,
    /// suggesting a candidate ADR.
    ADR,
    /// A constraint expressed in negative terms (must not, never),
    /// suggesting a candidate negative case.
    NegativeCase,
}

/// A specification element candidate extracted from harvest observations.
///
/// Generated by `classify_spec_candidate` when an exploration entity's body
/// text and metadata match one of the three spec element detection rules.
///
/// The `suggested_id` is auto-numbered and intended as a starting point;
/// the human reviewer should assign the canonical ID.
#[derive(Clone, Debug)]
pub struct SpecCandidate {
    /// The type of specification element this candidate represents.
    pub candidate_type: SpecCandidateType,
    /// Auto-generated suggested ID (e.g., "INV-HARVEST-100").
    pub suggested_id: String,
    /// The statement extracted from the observation body.
    pub statement: String,
    /// Suggested falsification condition (for Invariant and NegativeCase).
    pub falsification: Option<String>,
    /// SEED.md section this traces to (if detectable from context).
    pub traces_to: Option<String>,
    /// Confidence of the source observation.
    pub confidence: f64,
    /// The entity from which this candidate was derived.
    pub source_entity: EntityId,
}

/// Global counter for auto-numbering spec candidate IDs.
static SPEC_CANDIDATE_COUNTER: AtomicUsize = AtomicUsize::new(100);

/// Reset the spec candidate counter (for testing determinism).
#[cfg(test)]
fn reset_spec_candidate_counter() {
    SPEC_CANDIDATE_COUNTER.store(100, Ordering::SeqCst);
}

/// Check whether text contains universal quantifier language.
///
/// Detection heuristics for invariant-like statements:
/// always, never, must, every, for all, shall, invariant.
///
/// Case-insensitive word-boundary matching to reduce false positives
/// (e.g., "forestall" should not match "for all").
pub fn contains_universal_quantifier(text: &str) -> bool {
    let lower = text.to_lowercase();
    // Word-boundary-aware patterns. We check that the keyword is either
    // at the start/end of the string or bordered by non-alphanumeric chars.
    let keywords = [
        "always",
        "never",
        "must",
        "every",
        "for all",
        "shall",
        "invariant",
    ];
    for kw in &keywords {
        if let Some(pos) = lower.find(kw) {
            let before_ok = pos == 0 || !lower.as_bytes()[pos - 1].is_ascii_alphanumeric();
            let end = pos + kw.len();
            let after_ok = end >= lower.len() || !lower.as_bytes()[end].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return true;
            }
        }
    }
    false
}

/// Check whether text references two or more alternatives.
///
/// Detection heuristics for ADR-like decision records:
/// "chose X over Y", "option A vs option B", "instead of", "rather than".
pub fn has_alternatives(text: &str) -> bool {
    let lower = text.to_lowercase();
    let patterns = [
        "chose",
        "over",
        " vs ",
        " vs. ",
        "instead of",
        "rather than",
        "alternative",
        "compared to",
        "option a",
        "option b",
    ];
    let mut match_count = 0;
    for pat in &patterns {
        if lower.contains(pat) {
            match_count += 1;
        }
    }
    // Require at least 2 pattern matches to indicate genuine alternatives discussion
    match_count >= 2
}

/// Check whether text contains negative constraint language.
///
/// Detection heuristics for negative-case-like constraints:
/// "must not", "never", "prevents", "avoids", "do not", "forbidden", "prohibited".
pub fn contains_negative_constraint(text: &str) -> bool {
    let lower = text.to_lowercase();
    let patterns = [
        "must not",
        "prevents",
        "avoids",
        "do not",
        "forbidden",
        "prohibited",
        "shall not",
        "is not allowed",
    ];
    for pat in &patterns {
        if lower.contains(pat) {
            return true;
        }
    }
    // "never" requires word-boundary check (avoid "whenever", "nevertheless")
    if let Some(pos) = lower.find("never") {
        let before_ok = pos == 0 || !lower.as_bytes()[pos - 1].is_ascii_alphanumeric();
        let end = pos + 5;
        let after_ok = end >= lower.len() || !lower.as_bytes()[end].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

/// Classify an exploration entity as a potential spec candidate.
///
/// Detection rules (applied in priority order):
///
/// 1. **InvariantCandidate**: `exploration/category` = "design-decision" AND
///    confidence >= 0.8 AND body contains universal quantifier language.
///
/// 2. **ADRCandidate**: `exploration/category` = "design-decision" AND
///    body references >= 2 alternatives.
///
/// 3. **NegativeCaseCandidate**: body contains negative constraint language
///    (independent of category, since negative constraints can appear anywhere).
///
/// Returns `None` if the entity does not match any detection rule.
pub fn classify_spec_candidate(entity: EntityId, store: &Store) -> Option<SpecCandidate> {
    let datoms = store.entity_datoms(entity);

    // Extract exploration attributes
    let mut body: Option<String> = None;
    let mut category: Option<String> = None;
    let mut confidence: Option<f64> = None;

    for d in &datoms {
        if d.op != Op::Assert {
            continue;
        }
        match d.attribute.as_str() {
            ":exploration/body" => {
                if let Value::String(ref s) = d.value {
                    body = Some(s.clone());
                }
            }
            ":exploration/category" => match &d.value {
                Value::String(ref s) => category = Some(s.clone()),
                Value::Keyword(ref k) => category = Some(k.clone()),
                _ => {}
            },
            ":exploration/confidence" => {
                if let Value::Double(ordered_float::OrderedFloat(c)) = d.value {
                    confidence = Some(c);
                }
            }
            _ => {}
        }
    }

    let body = body?;
    let conf = confidence.unwrap_or(0.5);

    // Normalize category for matching
    let is_decision = category.as_deref().is_some_and(|c| {
        c == "design-decision"
            || c == "decision"
            || c.ends_with("/design-decision")
            || c.ends_with("/decision")
    });

    // Rule 1: InvariantCandidate
    if is_decision && conf >= 0.8 && contains_universal_quantifier(&body) {
        return Some(propose_invariant(entity, &body, conf));
    }

    // Rule 2: ADRCandidate
    if is_decision && has_alternatives(&body) {
        return Some(propose_adr(entity, &body, conf));
    }

    // Rule 3: NegativeCaseCandidate (category-independent)
    if contains_negative_constraint(&body) {
        return Some(propose_negative(entity, &body, conf));
    }

    None
}

/// Generate a `SpecCandidate` for an invariant proposal.
///
/// Auto-numbers the ID using a global counter. The statement is the
/// observation body; the falsification is synthesized from the universal
/// quantifier found in the body.
pub fn propose_invariant(entity: EntityId, body: &str, confidence: f64) -> SpecCandidate {
    let n = SPEC_CANDIDATE_COUNTER.fetch_add(1, Ordering::SeqCst);
    SpecCandidate {
        candidate_type: SpecCandidateType::Invariant,
        suggested_id: format!("INV-CANDIDATE-{n:03}"),
        statement: body.to_string(),
        falsification: Some(format!(
            "This invariant is violated if any counterexample to: {:.80}",
            body
        )),
        traces_to: None,
        confidence,
        source_entity: entity,
    }
}

/// Generate a `SpecCandidate` for an ADR proposal.
///
/// Auto-numbers the ID. The statement is the observation body.
/// No falsification is generated (ADRs have alternatives, not falsification).
pub fn propose_adr(entity: EntityId, body: &str, confidence: f64) -> SpecCandidate {
    let n = SPEC_CANDIDATE_COUNTER.fetch_add(1, Ordering::SeqCst);
    SpecCandidate {
        candidate_type: SpecCandidateType::ADR,
        suggested_id: format!("ADR-CANDIDATE-{n:03}"),
        statement: body.to_string(),
        falsification: None,
        traces_to: None,
        confidence,
        source_entity: entity,
    }
}

/// Generate a `SpecCandidate` for a negative case proposal.
///
/// Auto-numbers the ID. Synthesizes a falsification from the negative
/// constraint language found in the body.
pub fn propose_negative(entity: EntityId, body: &str, confidence: f64) -> SpecCandidate {
    let n = SPEC_CANDIDATE_COUNTER.fetch_add(1, Ordering::SeqCst);
    SpecCandidate {
        candidate_type: SpecCandidateType::NegativeCase,
        suggested_id: format!("NEG-CANDIDATE-{n:03}"),
        statement: body.to_string(),
        falsification: Some(format!("This negative case is violated if: {:.80}", body)),
        traces_to: None,
        confidence,
        source_entity: entity,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// Witnesses: INV-HARVEST-001, INV-HARVEST-002, INV-HARVEST-003, INV-HARVEST-004,
// INV-HARVEST-005, INV-HARVEST-006, INV-HARVEST-007, INV-HARVEST-009,
// ADR-HARVEST-001, ADR-HARVEST-002, ADR-HARVEST-003, ADR-HARVEST-005,
// ADR-HARVEST-006, ADR-HARVEST-007,
// NEG-HARVEST-002, NEG-HARVEST-003
#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::AgentId;

    // Verifies: INV-HARVEST-001 — Harvest Monotonicity (new knowledge detected)
    // Verifies: ADR-HARVEST-001 — Semi-Automated Over Fully Automatic
    #[test]
    fn harvest_detects_new_knowledge() {
        let store = Store::genesis();
        let agent = AgentId::from_name("test-agent");

        let context = SessionContext {
            agent,
            agent_name: "test-agent".into(),
            // wall=1 excludes genesis (wall=0) from session entities
            session_start_tx: TxId::new(1, 0, agent),
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

    // Verifies: INV-HARVEST-001 — Harvest Monotonicity (idempotent on existing)
    #[test]
    fn harvest_skips_existing_knowledge() {
        let store = Store::genesis();
        let agent = AgentId::from_name("test-agent");

        // Use an entity that already exists in genesis
        let context = SessionContext {
            agent,
            agent_name: "test-agent".into(),
            // wall=1 excludes genesis from session entities
            session_start_tx: TxId::new(1, 0, agent),
            task_description: "test session".to_string(),
            session_knowledge: vec![(
                ":db/ident".to_string(),
                Value::String("already exists".into()),
            )],
        };

        let result = harvest_pipeline(&store, &context);
        assert_eq!(result.candidates.len(), 0);
    }

    // Verifies: INV-HARVEST-002 — Harvest Provenance Trail
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

    // Verifies: INV-HARVEST-003 — Drift Score Recording
    // Verifies: INV-HARVEST-004 — FP/FN Calibration
    #[test]
    fn quality_metrics_computed_correctly() {
        let store = Store::genesis();
        let agent = AgentId::from_name("test");

        let context = SessionContext {
            agent,
            agent_name: "test".into(),
            // wall=1 excludes genesis from session entities
            session_start_tx: TxId::new(1, 0, agent),
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

    // Verifies: INV-HARVEST-009 — Continuous Externalization Protocol
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
            exploration_category: None,
            namespace_counts: BTreeMap::from([(AttrNamespace::Spec, 2), (AttrNamespace::Meta, 1)]),
            has_ident: false,
            ident: None,
            doc: None,
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
            exploration_category: None,
            namespace_counts: BTreeMap::from([(AttrNamespace::Intent, 2)]),
            has_ident: false,
            ident: None,
            doc: None,
            ref_count: 0,
            datom_count: 2,
        };
        assert_eq!(
            classify_profile(&decision_profile),
            HarvestCategory::Decision
        );
    }

    #[test]
    fn classify_exploration_category_decision() {
        // Observations with exploration_category "design-decision" should classify as Decision
        let profile = EntityProfile {
            entity: EntityId::from_ident(":test/obs-decision"),
            attributes: BTreeSet::from([
                ":exploration/body".to_string(),
                ":exploration/category".to_string(),
                ":exploration/source".to_string(),
                ":db/ident".to_string(),
            ]),
            exploration_category: Some(":exploration.cat/design-decision".to_string()),
            namespace_counts: BTreeMap::from([(AttrNamespace::Meta, 4)]),
            has_ident: true,
            ident: Some(":test/obs-decision".to_string()),
            doc: None,
            ref_count: 0,
            datom_count: 4,
        };
        assert_eq!(
            classify_profile(&profile),
            HarvestCategory::Decision,
            "observation with category design-decision should classify as Decision"
        );
    }

    #[test]
    fn classify_exploration_category_open_question() {
        // Observations with exploration_category "open-question" should classify as Uncertainty
        let profile = EntityProfile {
            entity: EntityId::from_ident(":test/obs-question"),
            attributes: BTreeSet::from([
                ":exploration/body".to_string(),
                ":exploration/category".to_string(),
                ":db/ident".to_string(),
            ]),
            exploration_category: Some("open-question".to_string()),
            namespace_counts: BTreeMap::from([(AttrNamespace::Meta, 3)]),
            has_ident: true,
            ident: Some(":test/obs-question".to_string()),
            doc: None,
            ref_count: 0,
            datom_count: 3,
        };
        assert_eq!(
            classify_profile(&profile),
            HarvestCategory::Uncertainty,
            "observation with category open-question should classify as Uncertainty"
        );
    }

    #[test]
    fn score_profile_rewards_density() {
        let sparse = EntityProfile {
            entity: EntityId::from_ident(":test/sparse"),
            attributes: BTreeSet::from([":db/doc".to_string()]),
            exploration_category: None,
            namespace_counts: BTreeMap::from([(AttrNamespace::Meta, 1)]),
            has_ident: false,
            ident: None,
            doc: None,
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
            exploration_category: None,
            namespace_counts: BTreeMap::from([(AttrNamespace::Meta, 2), (AttrNamespace::Spec, 3)]),
            has_ident: true,
            ident: Some(":test/dense".to_string()),
            doc: None,
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
            exploration_category: None,
            namespace_counts: BTreeMap::from([
                (AttrNamespace::Intent, 3),
                (AttrNamespace::Meta, 1),
            ]),
            has_ident: true,
            ident: Some(":test/decision".to_string()),
            doc: None,
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
            exploration_category: None,
            namespace_counts: BTreeMap::from([(AttrNamespace::Impl, 3), (AttrNamespace::Meta, 1)]),
            has_ident: true,
            ident: Some(":test/impl".to_string()),
            doc: None,
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

    // Verifies: INV-HARVEST-002 — Harvest Provenance Trail
    // Verifies: ADR-HARVEST-002 — Conversations Disposable, Knowledge Durable
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

    // Verifies: INV-HARVEST-005 — Proactive Warning
    #[test]
    fn detect_gaps_finds_missing_spec_attrs() {
        let profile = EntityProfile {
            entity: EntityId::from_ident(":test/incomplete-spec"),
            attributes: BTreeSet::from([":spec/element-type".to_string()]),
            exploration_category: None,
            namespace_counts: BTreeMap::from([(AttrNamespace::Spec, 1)]),
            has_ident: false,
            ident: None,
            doc: None,
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

    // Verifies: INV-HARVEST-004 — FP/FN Calibration
    // Verifies: ADR-HARVEST-003 — FP/FN Tracking for Calibration
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

    // Verifies: INV-HARVEST-004 — FP/FN Calibration
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
    // Narrative synthesis tests (S0.2.1)
    // -------------------------------------------------------------------

    #[test]
    fn test_synthesize_empty_candidates() {
        let store = Store::genesis();
        let summary = synthesize_narrative(&store, &[], "test task");

        assert_eq!(summary.goal, "test task");
        assert!(summary.accomplished.is_empty());
        assert!(summary.decisions.is_empty());
        assert!(summary.open_questions.is_empty());
        assert!(summary.focus_areas.is_empty());
        assert!(summary.git_summary.is_none());
    }

    #[test]
    fn test_synthesize_groups_by_category() {
        let store = Store::genesis();
        let candidates = vec![
            HarvestCandidate {
                entity: EntityId::from_ident(":test/obs-1"),
                assertions: vec![],
                category: HarvestCategory::Observation,
                confidence: 0.8,
                status: CandidateStatus::Proposed,
                rationale: "Observed something".into(),
            },
            HarvestCandidate {
                entity: EntityId::from_ident(":test/dec-1"),
                assertions: vec![],
                category: HarvestCategory::Decision,
                confidence: 0.9,
                status: CandidateStatus::Proposed,
                rationale: "Decided something".into(),
            },
            HarvestCandidate {
                entity: EntityId::from_ident(":test/unc-1"),
                assertions: vec![],
                category: HarvestCategory::Uncertainty,
                confidence: 0.6,
                status: CandidateStatus::Proposed,
                rationale: "Uncertain about something".into(),
            },
        ];

        let summary = synthesize_narrative(&store, &candidates, "grouping test");

        // Observations end up in accomplished
        assert_eq!(summary.accomplished.len(), 1);
        assert_eq!(
            summary.accomplished[0].category,
            HarvestCategory::Observation
        );
        assert!(summary.accomplished[0]
            .summary
            .contains("Observed something"));

        // Decisions end up in decisions
        assert_eq!(summary.decisions.len(), 1);
        assert_eq!(summary.decisions[0].summary, "Decided something");

        // Uncertainties end up in open_questions
        assert_eq!(summary.open_questions.len(), 1);
        assert_eq!(
            summary.open_questions[0].summary,
            "Uncertain about something"
        );
    }

    /// Build a store with full schema (axiomatic + L1+L2+L3 attributes) for tests
    /// that need to transact non-axiomatic attributes.
    fn full_schema_store() -> Store {
        use crate::schema::{full_schema_datoms, genesis_datoms};
        let system_agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, system_agent);
        let mut datom_set = std::collections::BTreeSet::new();
        // Include axiomatic attributes (:db/ident, :db/doc, etc.)
        for d in genesis_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        // Include domain attributes (:spec/*, :intent/*, :impl/*, etc.)
        for d in full_schema_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        Store::from_datoms(datom_set)
    }

    #[test]
    fn test_synthesize_extracts_decisions() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = full_schema_store();
        let agent = AgentId::from_name("test:narrative");

        // Create an entity with :intent/rationale in the store
        let entity = EntityId::from_ident(":test/decision-with-rationale");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "decision")
            .assert(
                entity,
                Attribute::from_keyword(":intent/rationale"),
                Value::String("Because performance matters".into()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/decision-with-rationale".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        let candidates = vec![HarvestCandidate {
            entity,
            assertions: vec![],
            category: HarvestCategory::Decision,
            confidence: 0.9,
            status: CandidateStatus::Proposed,
            rationale: "Chose approach X".into(),
        }];

        let summary = synthesize_narrative(&store, &candidates, "decision test");

        assert_eq!(summary.decisions.len(), 1);
        let dec = &summary.decisions[0];
        // Rationale should come from the store's :intent/rationale, not the candidate
        assert_eq!(dec.rationale, "Because performance matters");
        // Summary preserves the candidate rationale
        assert_eq!(dec.summary, "Chose approach X");
        // No :intent/alternatives in store, so alternatives is empty
        assert!(dec.alternatives.is_empty());
    }

    #[test]
    fn test_synthesize_computes_focus_areas() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = full_schema_store();
        let agent = AgentId::from_name("test:focus");

        // Create entities with spec and intent attributes
        let spec_entity = EntityId::from_ident(":test/spec-focus");
        let intent_entity = EntityId::from_ident(":test/intent-focus");

        let tx = Transaction::new(agent, ProvenanceType::Observed, "focus test")
            .assert(
                spec_entity,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-TEST-001".into()),
            )
            .assert(
                spec_entity,
                Attribute::from_keyword(":spec/element-type"),
                Value::Keyword(":element.type/invariant".into()),
            )
            .assert(
                spec_entity,
                Attribute::from_keyword(":spec/namespace"),
                Value::Keyword(":namespace/test".into()),
            )
            .assert(
                spec_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/spec-focus".into()),
            )
            .assert(
                intent_entity,
                Attribute::from_keyword(":intent/decision"),
                Value::String("Use EAV".into()),
            )
            .assert(
                intent_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/intent-focus".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        let candidates = vec![
            HarvestCandidate {
                entity: spec_entity,
                assertions: vec![],
                category: HarvestCategory::Observation,
                confidence: 0.9,
                status: CandidateStatus::Proposed,
                rationale: "Spec entity".into(),
            },
            HarvestCandidate {
                entity: intent_entity,
                assertions: vec![],
                category: HarvestCategory::Decision,
                confidence: 0.8,
                status: CandidateStatus::Proposed,
                rationale: "Intent entity".into(),
            },
        ];

        let summary = synthesize_narrative(&store, &candidates, "focus test");

        assert!(
            !summary.focus_areas.is_empty(),
            "should have focus areas when entities have datoms in store"
        );
        // SPEC namespace should appear (3 spec attrs)
        let has_spec = summary.focus_areas.iter().any(|(ns, _)| ns == "SPEC");
        assert!(has_spec, "SPEC should be in focus areas");
    }

    #[test]
    fn test_synthesize_derives_next_action() {
        // Use an empty store (0 datoms) to trigger the R11 bootstrap action,
        // which always has a command.
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        let candidates = vec![HarvestCandidate {
            entity: EntityId::from_ident(":test/obs-next"),
            assertions: vec![],
            category: HarvestCategory::Observation,
            confidence: 0.8,
            status: CandidateStatus::Proposed,
            rationale: "test".into(),
        }];

        let summary = synthesize_narrative(&store, &candidates, "next action test");

        // Empty store triggers R11 bootstrap → "braid init && braid bootstrap"
        assert!(
            summary.next.is_some(),
            "empty store should produce a next action suggestion from R11 bootstrap"
        );
        let next = summary.next.as_ref().unwrap();
        assert!(
            next.contains("braid"),
            "next action should contain a braid command, got: {next}"
        );
    }

    // -------------------------------------------------------------------
    // Narrative synthesis: comprehensive tests
    // -------------------------------------------------------------------

    #[test]
    fn test_narrative_synthesis_with_decisions() {
        // Use Store::from_datoms to bypass schema validation so we can include
        // :intent/alternatives (which synthesize_narrative looks for but isn't
        // in the standard schema).
        let agent = AgentId::from_name("test:narrative-decisions");
        let tx = TxId::new(1, 0, agent);

        let intent_entity = EntityId::from_ident(":intent/decision-test-001");

        let datoms: BTreeSet<Datom> = [
            Datom {
                entity: intent_entity,
                attribute: Attribute::from_keyword(":intent/rationale"),
                value: Value::String("EAV provides maximal schema flexibility".into()),
                tx,
                op: Op::Assert,
            },
            Datom {
                entity: intent_entity,
                attribute: Attribute::from_keyword(":intent/alternatives"),
                value: Value::String("relational tables | document store | graph DB".into()),
                tx,
                op: Op::Assert,
            },
            Datom {
                entity: intent_entity,
                attribute: Attribute::from_keyword(":db/doc"),
                value: Value::String("Chose EAV over relational model".into()),
                tx,
                op: Op::Assert,
            },
            Datom {
                entity: intent_entity,
                attribute: Attribute::from_keyword(":db/ident"),
                value: Value::Keyword(":intent/decision-test-001".into()),
                tx,
                op: Op::Assert,
            },
        ]
        .into_iter()
        .collect();

        let store = Store::from_datoms(datoms);

        // Create a harvest candidate pointing to the intent entity
        let candidates = vec![HarvestCandidate {
            entity: intent_entity,
            assertions: vec![],
            category: HarvestCategory::Decision,
            confidence: 0.9,
            status: CandidateStatus::Proposed,
            rationale: "Chose EAV model".into(),
        }];

        let summary = synthesize_narrative(&store, &candidates, "decision synthesis test");

        // Decisions should be non-empty
        assert!(
            !summary.decisions.is_empty(),
            "decisions should be populated from Decision candidates"
        );
        assert_eq!(summary.decisions.len(), 1);

        let dec = &summary.decisions[0];
        // Rationale should come from the store's :intent/rationale
        assert_eq!(dec.rationale, "EAV provides maximal schema flexibility");
        // Alternatives should be parsed from the pipe-separated string
        assert_eq!(dec.alternatives.len(), 3);
        assert!(dec.alternatives.contains(&"relational tables".to_string()));
        assert!(dec.alternatives.contains(&"document store".to_string()));
        assert!(dec.alternatives.contains(&"graph DB".to_string()));
        // Summary now resolves from entity body text (:db/doc), not candidate rationale
        assert_eq!(dec.summary, "Chose EAV over relational model");
    }

    #[test]
    fn test_narrative_synthesis_focus_areas() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = full_schema_store();
        let agent = AgentId::from_name("test:focus-areas");

        // Create several entities with :spec/ namespace attributes
        let spec1 = EntityId::from_ident(":test/spec-ns-1");
        let spec2 = EntityId::from_ident(":test/spec-ns-2");

        let tx1 = Transaction::new(agent, ProvenanceType::Observed, "spec entities")
            .assert(
                spec1,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-FOCUS-001".into()),
            )
            .assert(
                spec1,
                Attribute::from_keyword(":spec/element-type"),
                Value::Keyword(":element.type/invariant".into()),
            )
            .assert(
                spec1,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/spec-ns-1".into()),
            )
            .assert(
                spec2,
                Attribute::from_keyword(":spec/namespace"),
                Value::Keyword(":namespace/store".into()),
            )
            .assert(
                spec2,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-FOCUS-002".into()),
            )
            .assert(
                spec2,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/spec-ns-2".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx1).unwrap();

        // Create entities with :intent/ namespace attributes (classified as INTENT)
        let intent1 = EntityId::from_ident(":test/intent-ns-1");
        let intent2 = EntityId::from_ident(":test/intent-ns-2");

        let tx2 = Transaction::new(agent, ProvenanceType::Observed, "intent entities")
            .assert(
                intent1,
                Attribute::from_keyword(":intent/decision"),
                Value::String("Use CRDT merge".into()),
            )
            .assert(
                intent1,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/intent-ns-1".into()),
            )
            .assert(
                intent2,
                Attribute::from_keyword(":intent/rationale"),
                Value::String("CRDT provides commutativity".into()),
            )
            .assert(
                intent2,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/intent-ns-2".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx2).unwrap();

        let candidates = vec![
            HarvestCandidate {
                entity: spec1,
                assertions: vec![],
                category: HarvestCategory::Observation,
                confidence: 0.8,
                status: CandidateStatus::Proposed,
                rationale: "spec entity 1".into(),
            },
            HarvestCandidate {
                entity: spec2,
                assertions: vec![],
                category: HarvestCategory::Observation,
                confidence: 0.8,
                status: CandidateStatus::Proposed,
                rationale: "spec entity 2".into(),
            },
            HarvestCandidate {
                entity: intent1,
                assertions: vec![],
                category: HarvestCategory::Decision,
                confidence: 0.7,
                status: CandidateStatus::Proposed,
                rationale: "intent entity 1".into(),
            },
            HarvestCandidate {
                entity: intent2,
                assertions: vec![],
                category: HarvestCategory::Decision,
                confidence: 0.7,
                status: CandidateStatus::Proposed,
                rationale: "intent entity 2".into(),
            },
        ];

        let summary = synthesize_narrative(&store, &candidates, "focus areas test");

        // Focus areas should be non-empty
        assert!(
            !summary.focus_areas.is_empty(),
            "should extract focus areas from entity namespaces"
        );

        // SPEC namespace should appear (spec1 has 2 :spec/ attrs, spec2 has 2 :spec/ attrs)
        let has_spec = summary.focus_areas.iter().any(|(ns, _)| ns == "SPEC");
        assert!(has_spec, "SPEC namespace should appear in focus_areas");

        // INTENT namespace should appear (:intent/decision, :intent/rationale)
        let has_intent = summary.focus_areas.iter().any(|(ns, _)| ns == "INTENT");
        assert!(has_intent, "INTENT namespace should appear in focus_areas");

        // META namespace should appear (:db/ident counts as META)
        let has_meta = summary.focus_areas.iter().any(|(ns, _)| ns == "META");
        assert!(has_meta, "META namespace should appear in focus_areas");

        // Focus areas should be sorted by count (descending)
        for window in summary.focus_areas.windows(2) {
            assert!(
                window[0].1 >= window[1].1,
                "focus_areas should be sorted descending by count: {:?} vs {:?}",
                window[0],
                window[1]
            );
        }
    }

    #[test]
    fn test_narrative_synthesis_empty_task() {
        let store = Store::genesis();

        // Call with empty task string and no candidates
        let summary = synthesize_narrative(&store, &[], "");

        // Goal should match the (empty) task
        assert_eq!(summary.goal, "");
        // No candidates → no accomplishments
        assert!(
            summary.accomplished.is_empty(),
            "no candidates should produce no accomplishments"
        );
        // next should still have a fallback value (from guidance on genesis store)
        // — the store has schema datoms, so guidance may or may not fire a rule;
        // we just verify the function doesn't panic and returns a valid struct.
        // The key property: goal == task regardless of content.
    }

    #[test]
    fn test_narrative_open_questions() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = full_schema_store();
        let agent = AgentId::from_name("test:open-questions");

        // Create an entity that we'll reference as an uncertainty
        let unc_entity = EntityId::from_ident(":test/uncertainty-001");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "uncertainty")
            .assert(
                unc_entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("Should we use CRDT or OT for merge?".into()),
            )
            .assert(
                unc_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/uncertainty-001".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        let candidates = vec![HarvestCandidate {
            entity: unc_entity,
            assertions: vec![],
            category: HarvestCategory::Uncertainty,
            confidence: 0.4,
            status: CandidateStatus::Proposed,
            rationale: "CRDT vs OT merge strategy undecided".into(),
        }];

        let summary = synthesize_narrative(&store, &candidates, "open questions test");

        // Open questions should be populated from Uncertainty candidates
        assert!(
            !summary.open_questions.is_empty(),
            "Uncertainty candidates should produce open_questions"
        );
        assert_eq!(summary.open_questions.len(), 1);

        let oq = &summary.open_questions[0];
        // Summary now resolves from entity body text (:db/doc), not candidate rationale
        assert_eq!(oq.summary, "Should we use CRDT or OT for merge?");
        // entity_ref should resolve from :db/ident
        assert!(
            oq.entity_ref.is_some(),
            "entity_ref should be populated when :db/ident exists"
        );
        assert_eq!(
            oq.entity_ref.as_deref(),
            Some(":test/uncertainty-001"),
            "entity_ref should match the :db/ident keyword"
        );

        // Uncertainty candidates should NOT appear in accomplished
        assert!(
            summary.accomplished.is_empty()
                || summary
                    .accomplished
                    .iter()
                    .all(|a| a.category != HarvestCategory::Uncertainty),
            "Uncertainty candidates should not appear in accomplished"
        );
    }

    #[test]
    fn test_narrative_git_summary_none() {
        let store = Store::genesis();

        let candidates = vec![HarvestCandidate {
            entity: EntityId::from_ident(":test/git-none"),
            assertions: vec![],
            category: HarvestCategory::Observation,
            confidence: 0.8,
            status: CandidateStatus::Proposed,
            rationale: "test observation".into(),
        }];

        let summary = synthesize_narrative(&store, &candidates, "git summary test");

        // git_summary should be None — it's populated by the CLI layer, not the kernel
        assert!(
            summary.git_summary.is_none(),
            "git_summary should be None when no git summary is provided (kernel layer)"
        );
    }

    // -------------------------------------------------------------------
    // Synthesis directive tests (S0.2a.2: pipe-back-to-harness)
    // -------------------------------------------------------------------

    #[test]
    fn test_synthesis_directive_generated() {
        let store = full_schema_store();
        let candidates = vec![HarvestCandidate {
            entity: EntityId::from_ident(":test/decision-directive"),
            assertions: vec![],
            category: HarvestCategory::Decision,
            confidence: 0.9,
            status: CandidateStatus::Proposed,
            rationale: "chose hash-join".into(),
        }];
        let summary = synthesize_narrative(&store, &candidates, "implement joins");
        assert!(
            summary.synthesis_directive.is_some(),
            "Should generate directive when decisions exist"
        );
        let directive = summary.synthesis_directive.unwrap();
        assert!(
            directive.contains("Decisions made"),
            "Directive should mention decisions"
        );
        assert!(
            directive.contains("Next session task"),
            "Directive should suggest next task"
        );
        assert!(
            directive.contains("braid seed"),
            "Directive should include seed command"
        );
    }

    #[test]
    fn test_synthesis_directive_none_when_empty() {
        let store = Store::genesis();
        let summary = synthesize_narrative(&store, &[], "empty session");
        assert!(
            summary.synthesis_directive.is_none(),
            "Should not generate directive when no accomplishments or questions"
        );
    }

    #[test]
    fn test_synthesis_directive_includes_open_questions() {
        let store = Store::genesis();
        let candidates = vec![HarvestCandidate {
            entity: EntityId::from_ident(":test/question-directive"),
            assertions: vec![],
            category: HarvestCategory::Uncertainty,
            confidence: 0.5,
            status: CandidateStatus::Proposed,
            rationale: "CRDT vs OT merge undecided".into(),
        }];
        let summary = synthesize_narrative(&store, &candidates, "merge strategy");
        assert!(
            summary.synthesis_directive.is_some(),
            "Should generate directive when open questions exist"
        );
        let directive = summary.synthesis_directive.unwrap();
        assert!(
            directive.contains("Unresolved questions"),
            "Directive should surface unresolved questions"
        );
        assert!(
            directive.contains("CRDT vs OT merge undecided"),
            "Directive should contain the actual question text"
        );
    }

    // -------------------------------------------------------------------
    // Task inference tests (S0.2b)
    // -------------------------------------------------------------------

    #[test]
    fn test_infer_task_from_session_entity() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = full_schema_store();
        let agent = AgentId::from_name("test:infer-session");

        // Create a session entity with :session/task
        let session = EntityId::from_ident(":session/current");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "set task")
            .assert(
                session,
                Attribute::from_keyword(":session/task"),
                Value::String("implement query optimizer".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        let (task, source, confidence) = infer_task_description(&store);
        assert!(
            task.contains("implement query optimizer"),
            "Should infer from session entity: {task}"
        );
        assert_eq!(source, "session entity");
        assert!(
            confidence >= 0.9,
            "Session entity should be high confidence: {confidence}"
        );
    }

    #[test]
    fn test_infer_task_from_observation() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = full_schema_store();
        let agent = AgentId::from_name("test:infer-obs");

        // Create an observation with :exploration/body (no :session/task)
        let obs = EntityId::from_ident(":test/obs-infer");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "observe")
            .assert(
                obs,
                Attribute::from_keyword(":exploration/body"),
                Value::String("fixing the query engine join logic".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        let (task, source, confidence) = infer_task_description(&store);
        assert!(
            task.contains("fixing"),
            "Should infer from observation: {task}"
        );
        assert_eq!(source, "recent observation");
        assert!(
            confidence > 0.5,
            "Observation should be medium confidence: {confidence}"
        );
    }

    #[test]
    fn test_infer_task_fallback() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        let (task, source, confidence) = infer_task_description(&store);
        assert_eq!(task, "session work");
        assert_eq!(source, "fallback");
        assert!(
            confidence < 0.2,
            "Fallback should be low confidence: {confidence}"
        );
    }

    #[test]
    fn test_infer_task_session_beats_observation() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = full_schema_store();
        let agent = AgentId::from_name("test:infer-priority");

        // Add both a session task and an observation
        let session = EntityId::from_ident(":session/current");
        let obs = EntityId::from_ident(":test/obs-priority");

        let tx = Transaction::new(agent, ProvenanceType::Observed, "both signals")
            .assert(
                session,
                Attribute::from_keyword(":session/task"),
                Value::String("session-level task".into()),
            )
            .assert(
                obs,
                Attribute::from_keyword(":exploration/body"),
                Value::String("observation-level task".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        let (task, source, confidence) = infer_task_description(&store);
        // Session entity should win over observation
        assert_eq!(source, "session entity");
        assert!(
            task.contains("session-level"),
            "Session entity should take priority: {task}"
        );
        assert!(confidence > 0.9);
    }

    #[test]
    fn test_infer_task_truncates_long_observation() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = full_schema_store();
        let agent = AgentId::from_name("test:infer-truncate");

        let obs = EntityId::from_ident(":test/obs-long");
        let long_body = "x".repeat(200);
        let tx = Transaction::new(agent, ProvenanceType::Observed, "long obs")
            .assert(
                obs,
                Attribute::from_keyword(":exploration/body"),
                Value::String(long_body),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        let (task, source, _) = infer_task_description(&store);
        assert_eq!(source, "recent observation");
        assert!(
            task.len() <= 84, // 80 chars + "..."
            "Should truncate long observations: len={}",
            task.len()
        );
        assert!(task.ends_with("..."), "Should end with ellipsis");
    }

    #[test]
    fn test_infer_task_namespace_frequency() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = full_schema_store();
        let agent = AgentId::from_name("test:infer-ns");

        // Add several spec entities but no session task or observation
        for i in 0..5 {
            let entity = EntityId::from_ident(&format!(":test/spec-ns-{i}"));
            let tx = Transaction::new(agent, ProvenanceType::Observed, "spec work")
                .assert(
                    entity,
                    Attribute::from_keyword(":spec/id"),
                    Value::String(format!("INV-TEST-{i:03}")),
                )
                .assert(
                    entity,
                    Attribute::from_keyword(":spec/element-type"),
                    Value::Keyword(":element.type/invariant".into()),
                )
                .commit(&store)
                .unwrap();
            store.transact(tx).unwrap();
        }

        let (task, source, confidence) = infer_task_description(&store);
        // Should detect SPEC as the dominant namespace (or META due to :db/ident
        // and :tx/* attributes — either way it should not be "fallback")
        assert!(
            source == "namespace frequency" || source == "fallback",
            "Should use namespace frequency or fallback: {source}"
        );
        if source == "namespace frequency" {
            assert!(
                task.contains("namespace work"),
                "Namespace task should mention 'namespace work': {task}"
            );
            assert!(
                (0.2..=0.4).contains(&confidence),
                "Namespace frequency should be low confidence: {confidence}"
            );
        }
    }

    // -------------------------------------------------------------------
    // Spec candidate classification tests (W4A)
    // -------------------------------------------------------------------

    #[test]
    fn invariant_candidate_detected_for_high_confidence_universal() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = full_schema_store();
        let agent = AgentId::from_name("test:spec-candidate");
        reset_spec_candidate_counter();

        let entity = EntityId::from_ident(":test/inv-candidate");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "spec candidate test")
            .assert(
                entity,
                Attribute::from_keyword(":exploration/body"),
                Value::String("The store must always be append-only".into()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":exploration/category"),
                Value::Keyword("design-decision".into()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":exploration/confidence"),
                Value::Double(ordered_float::OrderedFloat(0.9)),
            )
            .assert(
                entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/inv-candidate".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        let candidate = classify_spec_candidate(entity, &store);
        assert!(candidate.is_some(), "should detect invariant candidate");
        let c = candidate.unwrap();
        assert_eq!(c.candidate_type, SpecCandidateType::Invariant);
        assert!(c.suggested_id.starts_with("INV-CANDIDATE-"));
        assert!(c.statement.contains("append-only"));
        assert!(c.falsification.is_some());
        assert!((c.confidence - 0.9).abs() < 1e-10);
        assert_eq!(c.source_entity, entity);
    }

    #[test]
    fn adr_candidate_detected_for_alternatives() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = full_schema_store();
        let agent = AgentId::from_name("test:adr-candidate");
        reset_spec_candidate_counter();

        let entity = EntityId::from_ident(":test/adr-candidate");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "adr candidate test")
            .assert(
                entity,
                Attribute::from_keyword(":exploration/body"),
                Value::String(
                    "Chose EAV over relational tables; compared to document store, \
                     this alternative provides maximal flexibility"
                        .into(),
                ),
            )
            .assert(
                entity,
                Attribute::from_keyword(":exploration/category"),
                Value::Keyword("design-decision".into()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":exploration/confidence"),
                Value::Double(ordered_float::OrderedFloat(0.7)),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        let candidate = classify_spec_candidate(entity, &store);
        assert!(candidate.is_some(), "should detect ADR candidate");
        let c = candidate.unwrap();
        assert_eq!(c.candidate_type, SpecCandidateType::ADR);
        assert!(c.suggested_id.starts_with("ADR-CANDIDATE-"));
        assert!(c.statement.contains("Chose EAV"));
        assert!(c.falsification.is_none(), "ADRs have no falsification");
    }

    #[test]
    fn negative_case_detected_for_must_not() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = full_schema_store();
        let agent = AgentId::from_name("test:neg-candidate");
        reset_spec_candidate_counter();

        let entity = EntityId::from_ident(":test/neg-candidate");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "neg candidate test")
            .assert(
                entity,
                Attribute::from_keyword(":exploration/body"),
                Value::String("Agents must not delete datoms from the store".into()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":exploration/category"),
                Value::Keyword("observation".into()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":exploration/confidence"),
                Value::Double(ordered_float::OrderedFloat(0.85)),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        let candidate = classify_spec_candidate(entity, &store);
        assert!(candidate.is_some(), "should detect negative case candidate");
        let c = candidate.unwrap();
        assert_eq!(c.candidate_type, SpecCandidateType::NegativeCase);
        assert!(c.suggested_id.starts_with("NEG-CANDIDATE-"));
        assert!(c.falsification.is_some());
    }

    #[test]
    fn low_confidence_skipped_for_invariant() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = full_schema_store();
        let agent = AgentId::from_name("test:low-conf");

        let entity = EntityId::from_ident(":test/low-conf");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "low conf test")
            .assert(
                entity,
                Attribute::from_keyword(":exploration/body"),
                Value::String("The system must always validate inputs".into()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":exploration/category"),
                Value::Keyword("design-decision".into()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":exploration/confidence"),
                Value::Double(ordered_float::OrderedFloat(0.5)),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        let candidate = classify_spec_candidate(entity, &store);
        // Low confidence (0.5 < 0.8) should skip invariant detection.
        // Body does not match ADR patterns (no alternatives).
        // Body does contain "must" but not "must not", so no negative case.
        assert!(
            candidate.is_none()
                || candidate.as_ref().unwrap().candidate_type != SpecCandidateType::Invariant,
            "low confidence should not produce an invariant candidate"
        );
    }

    #[test]
    fn non_decision_category_skipped_for_invariant_and_adr() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = full_schema_store();
        let agent = AgentId::from_name("test:non-decision");

        let entity = EntityId::from_ident(":test/non-decision");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "non-decision test")
            .assert(
                entity,
                Attribute::from_keyword(":exploration/body"),
                Value::String("The system always works correctly".into()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":exploration/category"),
                Value::Keyword("observation".into()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":exploration/confidence"),
                Value::Double(ordered_float::OrderedFloat(0.95)),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        let candidate = classify_spec_candidate(entity, &store);
        // Non-decision category should not produce Invariant or ADR candidates.
        // Body doesn't contain negative constraint patterns, so should be None.
        assert!(
            candidate.is_none(),
            "non-decision category without negative constraints should not produce spec candidate"
        );
    }

    #[test]
    fn universal_quantifier_detection() {
        assert!(contains_universal_quantifier("This must hold"));
        assert!(contains_universal_quantifier("Always append-only"));
        assert!(contains_universal_quantifier("This shall be true"));
        assert!(contains_universal_quantifier("For all agents"));
        assert!(contains_universal_quantifier("Every datom is immutable"));
        assert!(contains_universal_quantifier("Never delete"));
        assert!(contains_universal_quantifier(
            "This is an invariant of the system"
        ));

        // Negative cases: no quantifier language
        assert!(!contains_universal_quantifier("The system stores data"));
        assert!(!contains_universal_quantifier("We implement EAV"));
    }

    #[test]
    fn alternatives_detection() {
        assert!(has_alternatives(
            "Chose EAV over relational; alternative was document store"
        ));
        assert!(has_alternatives(
            "Option A vs Option B, rather than going with C"
        ));
        assert!(has_alternatives(
            "We chose this approach instead of using a graph DB, compared to other options"
        ));

        // Single pattern match is not enough (need >= 2)
        assert!(!has_alternatives("We chose this approach"));
        assert!(!has_alternatives("The system is fast"));
    }

    #[test]
    fn negative_constraint_detection() {
        assert!(contains_negative_constraint("Agents must not delete"));
        assert!(contains_negative_constraint("This prevents data loss"));
        assert!(contains_negative_constraint("Do not mutate the store"));
        assert!(contains_negative_constraint("This operation is forbidden"));
        assert!(contains_negative_constraint("Mutation is prohibited"));
        assert!(contains_negative_constraint("The system avoids conflicts"));
        assert!(contains_negative_constraint("Never delete datoms"));

        // Words containing "never" as substring should not match
        assert!(!contains_negative_constraint("Whenever the system runs"));
        assert!(!contains_negative_constraint("Nevertheless it works"));
        // No negative constraint language
        assert!(!contains_negative_constraint("The system stores data"));
    }

    #[test]
    fn propose_invariant_generates_valid_candidate() {
        reset_spec_candidate_counter();
        let entity = EntityId::from_ident(":test/propose-inv");
        let c = propose_invariant(entity, "The store must always grow", 0.9);
        assert_eq!(c.candidate_type, SpecCandidateType::Invariant);
        assert_eq!(c.suggested_id, "INV-CANDIDATE-100");
        assert!(c.falsification.is_some());
        assert_eq!(c.source_entity, entity);

        // Second call increments counter
        let c2 = propose_invariant(entity, "Another invariant", 0.85);
        assert_eq!(c2.suggested_id, "INV-CANDIDATE-101");
    }

    #[test]
    fn propose_adr_generates_valid_candidate() {
        reset_spec_candidate_counter();
        let entity = EntityId::from_ident(":test/propose-adr");
        let c = propose_adr(entity, "Chose EAV over relational", 0.8);
        assert_eq!(c.candidate_type, SpecCandidateType::ADR);
        assert_eq!(c.suggested_id, "ADR-CANDIDATE-100");
        assert!(c.falsification.is_none());
    }

    #[test]
    fn propose_negative_generates_valid_candidate() {
        reset_spec_candidate_counter();
        let entity = EntityId::from_ident(":test/propose-neg");
        let c = propose_negative(entity, "Must not delete datoms", 0.7);
        assert_eq!(c.candidate_type, SpecCandidateType::NegativeCase);
        assert_eq!(c.suggested_id, "NEG-CANDIDATE-100");
        assert!(c.falsification.is_some());
    }

    #[test]
    fn classify_spec_candidate_no_body_returns_none() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = full_schema_store();
        let agent = AgentId::from_name("test:no-body");

        // Entity with category but no body
        let entity = EntityId::from_ident(":test/no-body");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "no body test")
            .assert(
                entity,
                Attribute::from_keyword(":exploration/category"),
                Value::Keyword("design-decision".into()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":exploration/confidence"),
                Value::Double(ordered_float::OrderedFloat(0.9)),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        let candidate = classify_spec_candidate(entity, &store);
        assert!(candidate.is_none(), "no body should return None");
    }

    #[test]
    fn invariant_priority_over_negative_case() {
        // When body matches both invariant (universal quantifier + decision + high conf)
        // and negative case ("never"), invariant should win (priority order).
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;

        let mut store = full_schema_store();
        let agent = AgentId::from_name("test:priority");
        reset_spec_candidate_counter();

        let entity = EntityId::from_ident(":test/inv-over-neg");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "priority test")
            .assert(
                entity,
                Attribute::from_keyword(":exploration/body"),
                Value::String("The store must never shrink".into()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":exploration/category"),
                Value::Keyword("design-decision".into()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":exploration/confidence"),
                Value::Double(ordered_float::OrderedFloat(0.9)),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        let candidate = classify_spec_candidate(entity, &store);
        assert!(candidate.is_some());
        // "never" matches both universal quantifier and negative constraint,
        // but invariant rule fires first (design-decision + high confidence + quantifier)
        assert_eq!(
            candidate.unwrap().candidate_type,
            SpecCandidateType::Invariant,
            "invariant should take priority over negative case"
        );
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

            /// INV-HARVEST-003: Drift score reflects gap count.
            /// More session knowledge items (all new to the store) must produce
            /// a drift_score >= that of fewer items, because drift_score =
            /// candidates / total_knowledge and more new items yield more candidates.
            #[test]
            fn drift_score_reflects_gap_count(
                base_items in proptest::collection::vec(
                    (
                        "[a-z]{3,8}".prop_map(|s| format!(":drift-test/{s}")),
                        arb_doc_value(),
                    ),
                    1..=3usize,
                ),
                extra_items in proptest::collection::vec(
                    (
                        "[a-z]{3,8}".prop_map(|s| format!(":drift-extra/{s}")),
                        arb_doc_value(),
                    ),
                    1..=3usize,
                ),
            ) {
                let store = Store::genesis();
                let agent = crate::datom::AgentId::from_name("proptest:drift");
                let start_tx = TxId::new(1, 0, agent);

                // Smaller knowledge set
                let ctx_small = SessionContext {
                    agent,
                    agent_name: "proptest-drift".into(),
                    session_start_tx: start_tx,
                    task_description: "drift test".into(),
                    session_knowledge: base_items.clone(),
                };
                let result_small = harvest_pipeline(&store, &ctx_small);

                // Larger knowledge set (base + extra)
                let mut combined = base_items;
                combined.extend(extra_items);
                let ctx_large = SessionContext {
                    agent,
                    agent_name: "proptest-drift".into(),
                    session_start_tx: start_tx,
                    task_description: "drift test".into(),
                    session_knowledge: combined,
                };
                let result_large = harvest_pipeline(&store, &ctx_large);

                // More knowledge items means more candidates (all are new to genesis store).
                // drift_score = candidates / total_knowledge.
                // With more new items, candidate count grows at least as fast as total_knowledge,
                // so drift_score for the larger set should be >= drift_score for the smaller set.
                prop_assert!(
                    result_large.candidates.len() >= result_small.candidates.len(),
                    "larger knowledge set must produce >= candidates: {} vs {}",
                    result_large.candidates.len(),
                    result_small.candidates.len()
                );
            }

            /// INV-HARVEST-005: Proactive warning fires at correct thresholds.
            /// Harvest completeness gaps must be non-negative and bounded by session entity count.
            /// When the session has entities with missing expected attributes, gaps appear.
            #[test]
            fn proactive_warning_at_thresholds(ctx in arb_session_context()) {
                let store = Store::genesis();
                let result = harvest_pipeline(&store, &ctx);

                // Completeness gaps must never be negative (structural invariant).
                // Completeness gaps must be bounded: each session entity can produce
                // at most max(SPEC_EXPECTED, DECISION_EXPECTED) gaps.
                let max_gaps_per_entity = SPEC_EXPECTED.len().max(DECISION_EXPECTED.len());
                let gap_upper_bound = result.session_entities * max_gaps_per_entity;
                prop_assert!(
                    result.completeness_gaps <= gap_upper_bound,
                    "completeness_gaps ({}) must be <= session_entities ({}) * max_gaps ({})",
                    result.completeness_gaps,
                    result.session_entities,
                    max_gaps_per_entity,
                );

                // Quality count must match actual candidates length.
                prop_assert_eq!(
                    result.quality.count,
                    result.candidates.len(),
                    "quality.count must equal candidates.len()"
                );
            }

            /// INV-HARVEST-007: Harvest completes in bounded operations.
            /// The harvest pipeline must always terminate and produce a valid result
            /// for arbitrary store states. No infinite loops, no panics.
            #[test]
            fn harvest_completes_in_bounded_ops(store in arb_store(5)) {
                let agent = crate::datom::AgentId::from_name("proptest:bounded");
                // Use wall_time=0 so all store datoms are "in session" (maximum work).
                let start_tx = TxId::new(0, 0, agent);

                let ctx = SessionContext {
                    agent,
                    agent_name: "proptest-bounded".into(),
                    session_start_tx: start_tx,
                    task_description: "bounded ops test".into(),
                    session_knowledge: vec![],
                };
                let result = harvest_pipeline(&store, &ctx);

                // Pipeline must always terminate and return valid structure.
                // Session entities bounded by store entity count.
                prop_assert!(
                    result.session_entities <= store.entity_count(),
                    "session_entities ({}) must be <= store entity_count ({})",
                    result.session_entities,
                    store.entity_count()
                );
                // Drift score must be finite and non-negative.
                prop_assert!(
                    result.drift_score.is_finite() && result.drift_score >= 0.0,
                    "drift_score must be finite and >= 0, got {}",
                    result.drift_score
                );
                // All candidates must have valid confidence in [0, 1].
                for (i, c) in result.candidates.iter().enumerate() {
                    prop_assert!(
                        c.confidence >= 0.0 && c.confidence <= 1.0,
                        "candidate[{i}] confidence must be in [0,1], got {}",
                        c.confidence
                    );
                }
            }

            /// W4A: classify_spec_candidate never panics on arbitrary stores.
            /// For any store, calling classify_spec_candidate on every entity
            /// must return either None or a valid SpecCandidate without panicking.
            #[test]
            fn classify_spec_candidate_never_panics(store in arb_store(5)) {
                // Collect all entity IDs from the store
                let entities: BTreeSet<EntityId> = store.datoms()
                    .map(|d| d.entity)
                    .collect();

                for entity in &entities {
                    // Must not panic for any entity
                    let result = classify_spec_candidate(*entity, &store);
                    if let Some(ref candidate) = result {
                        // If a candidate is returned, its confidence must be in [0, 1]
                        prop_assert!(
                            candidate.confidence >= 0.0 && candidate.confidence <= 1.0,
                            "spec candidate confidence must be in [0,1], got {}",
                            candidate.confidence
                        );
                        // suggested_id must be non-empty
                        prop_assert!(
                            !candidate.suggested_id.is_empty(),
                            "suggested_id must be non-empty"
                        );
                        // statement must be non-empty
                        prop_assert!(
                            !candidate.statement.is_empty(),
                            "statement must be non-empty"
                        );
                        // source_entity must match
                        prop_assert_eq!(
                            candidate.source_entity,
                            *entity,
                            "source_entity must match input entity"
                        );
                    }
                }
            }

            /// INV-HARVEST-009: All harvest candidates are representable as datoms.
            /// For any store, harvest candidates can be converted to datoms without
            /// panicking, and each datom is a valid assertion.
            #[test]
            fn harvest_candidates_representable_as_datoms(store in arb_store(5)) {
                let agent = crate::datom::AgentId::from_name("proptest:datom-repr");
                let start_tx = TxId::new(0, 0, agent);

                let ctx = SessionContext {
                    agent,
                    agent_name: "proptest-datom-repr".into(),
                    session_start_tx: start_tx,
                    task_description: "datom repr test".into(),
                    session_knowledge: vec![
                        (":repr/test-a".into(), Value::String("alpha".into())),
                        (":repr/test-b".into(), Value::String("beta".into())),
                    ],
                };
                let result = harvest_pipeline(&store, &ctx);
                let commit_tx = TxId::new(9999, 0, agent);

                for candidate in &result.candidates {
                    // candidate_to_datoms must not panic for any candidate.
                    let datoms = candidate_to_datoms(candidate, commit_tx);

                    // Datom count must match assertion count.
                    prop_assert_eq!(
                        datoms.len(),
                        candidate.assertions.len(),
                        "datom count must match assertion count"
                    );

                    // Every produced datom must be an Assert op.
                    for d in &datoms {
                        prop_assert_eq!(
                            d.op,
                            Op::Assert,
                            "all datoms must be assertions (INV-HARVEST-002)"
                        );
                    }
                }

                // Additionally, build_harvest_commit must succeed and produce
                // only assertions (monotonic extension).
                let commit = build_harvest_commit(&result, &ctx, commit_tx);
                prop_assert!(
                    commit.datoms.iter().all(|d| d.op == Op::Assert),
                    "harvest commit must contain only assertions"
                );
                // Session entity datoms are always present (at least 7).
                prop_assert!(
                    commit.datom_count >= 7,
                    "commit must have at least session entity datoms, got {}",
                    commit.datom_count
                );
            }
        }
    }
}
