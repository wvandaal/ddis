//! HARVEST namespace — end-of-session epistemic gap detection pipeline.
//!
//! Detects knowledge gaps between an agent's session context and the store,
//! proposes candidates for externalization, and commits approved candidates.
//!
//! # Pipeline (INV-HARVEST-005)
//!
//! DETECT → PROPOSE → REVIEW → COMMIT → RECORD
//!
//! # Invariants
//!
//! - **INV-HARVEST-001**: Epistemic gap Δ(t) = K_agent \ K_store.
//! - **INV-HARVEST-002**: Monotonic extension (store grows, never shrinks).
//! - **INV-HARVEST-003**: Quality metrics (FP rate, FN rate, drift_score).
//! - **INV-HARVEST-005**: Pipeline correctness (5-step state machine).

use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, TxId, Value};
use crate::store::Store;

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
    /// TxId at session start.
    pub session_start_tx: TxId,
    /// Description of the task worked on.
    pub task_description: String,
    /// Knowledge items from the session (pre-candidates).
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

/// Run the harvest pipeline: DETECT → PROPOSE → REVIEW → COMMIT → RECORD.
///
/// Stage 0 implements the first two steps (DETECT, PROPOSE). REVIEW is manual.
/// COMMIT and RECORD happen through `accept_candidate` and `harvest_session_entity`.
pub fn harvest_pipeline(store: &Store, context: &SessionContext) -> HarvestResult {
    let mut candidates = Vec::new();

    // DETECT: Find knowledge gaps
    // For each session knowledge item, check if it's already in the store
    for (key, value) in &context.session_knowledge {
        let entity = EntityId::from_ident(key);
        let existing: Vec<&Datom> = store
            .datoms()
            .filter(|d| d.entity == entity && d.op == Op::Assert)
            .collect();

        if existing.is_empty() {
            // PROPOSE: New knowledge, not in store
            candidates.push(HarvestCandidate {
                entity,
                assertions: vec![(Attribute::from_keyword(":db/doc"), value.clone())],
                category: categorize_value(value),
                confidence: 0.8,
                status: CandidateStatus::Proposed,
                rationale: format!("New knowledge from session: {key}"),
            });
        }
    }

    let count = candidates.len();
    let high_confidence = candidates.iter().filter(|c| c.confidence >= 0.8).count();
    let medium_confidence = candidates
        .iter()
        .filter(|c| c.confidence >= 0.5 && c.confidence < 0.8)
        .count();
    let low_confidence = candidates.iter().filter(|c| c.confidence < 0.5).count();

    let drift_score = if context.session_knowledge.is_empty() {
        0.0
    } else {
        count as f64 / context.session_knowledge.len() as f64
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
    }
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

/// Categorize a value into a harvest category.
fn categorize_value(value: &Value) -> HarvestCategory {
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
            session_start_tx: TxId::new(0, 0, agent),
            task_description: "test session".to_string(),
            session_knowledge: vec![(
                ":db/ident".to_string(),
                Value::String("already exists".into()),
            )],
        };

        let result = harvest_pipeline(&store, &context);
        // :db/ident entity exists in genesis, but the exact check depends
        // on whether the entity ID matches — since we use from_ident,
        // it should match the genesis entity.
        // The check is by entity, so if the entity exists with ANY assertions,
        // we don't re-propose it.
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
}
