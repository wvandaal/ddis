//! Spec proposal lifecycle: propose, review, accept, reject.
//!
//! When the harvest pipeline detects an observation that resembles a
//! formal specification element (via `classify_spec_candidate`), this
//! module turns the `SpecCandidate` into a set of datoms with
//! `:proposal/*` attributes and manages the review lifecycle.
//!
//! # Lifecycle
//!
//! 1. **Propose**: `proposal_to_datoms` converts a `SpecCandidate` into an
//!    entity with `:proposal/status :proposal.status/proposed`.
//! 2. **Review**: A human or agent inspects the proposal.
//! 3. **Accept**: `accept_proposal` transitions status to `:proposal.status/accepted`
//!    and generates `:spec/*` + `:element/*` datoms via promotion.
//! 4. **Reject**: `reject_proposal` transitions status to `:proposal.status/rejected`
//!    with a rationale note.
//!
//! Proposals with confidence >= `auto_accept_threshold()` (0.9) are
//! candidates for automated acceptance without explicit human review.
//!
//! # Invariants
//!
//! - Append-only: status transitions never retract the original proposal datoms (C1).
//! - Content-addressable: the proposal entity ID is derived from its content (C2).
//! - Traceability: every proposal must trace to a SEED.md section (C5).
//!
//! # Design Decisions
//!
//! - Proposals are stored as datoms in the same append-only store, not in a
//!   separate review database. This keeps the single-store invariant (C1, C7).
//! - The review lifecycle uses keyword status values rather than separate
//!   entity types, keeping the schema flat and queryable.

use crate::datom::{Attribute, Datom, EntityId, Op, TxId, Value};
use crate::harvest::{SpecCandidate, SpecCandidateType};
use crate::store::Store;

/// Confidence threshold at or above which proposals may be auto-accepted.
///
/// Proposals with `confidence >= 0.9` are considered high enough quality
/// that automated acceptance is appropriate. Below this threshold, human
/// review is expected.
pub fn auto_accept_threshold() -> f64 {
    0.9
}

/// Convert a `SpecCandidate` into proposal datoms for a new entity.
///
/// The returned datoms assert `:proposal/*` attributes on a content-addressed
/// entity derived from the candidate's suggested ID and statement. The
/// initial status is `:proposal.status/proposed`.
///
/// # Arguments
///
/// * `candidate` - The classified spec candidate from harvest.
/// * `tx_id` - Transaction ID to stamp on the datoms.
///
/// # Returns
///
/// A vector of datoms representing the proposal entity. Transact these
/// into the store to register the proposal.
pub fn proposal_to_datoms(candidate: &SpecCandidate, tx_id: TxId) -> Vec<Datom> {
    // Content-addressable entity ID from statement + source entity.
    // Uses statement + source (not suggested_id) to ensure identical content
    // produces identical entities regardless of auto-numbering (C2).
    let content = format!(
        "proposal:{}:{:?}",
        candidate.statement, candidate.source_entity
    );
    let entity = EntityId::from_content(content.as_bytes());

    let type_kw = match candidate.candidate_type {
        SpecCandidateType::Invariant => ":proposal.type/invariant",
        SpecCandidateType::ADR => ":proposal.type/adr",
        SpecCandidateType::NegativeCase => ":proposal.type/negative-case",
    };

    let mut datoms = vec![
        Datom::new(
            entity,
            Attribute::from_keyword(":proposal/type"),
            Value::Keyword(type_kw.to_string()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":proposal/status"),
            Value::Keyword(":proposal.status/proposed".to_string()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":proposal/source"),
            Value::Ref(candidate.source_entity),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":proposal/suggested-id"),
            Value::String(candidate.suggested_id.clone()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":proposal/statement"),
            Value::String(candidate.statement.clone()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":proposal/confidence"),
            Value::Double(candidate.confidence.into()),
            tx_id,
            Op::Assert,
        ),
    ];

    // Optional: falsification condition (for invariants and negative cases).
    if let Some(ref falsification) = candidate.falsification {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":proposal/falsification"),
            Value::String(falsification.clone()),
            tx_id,
            Op::Assert,
        ));
    }

    // Optional: SEED.md section trace.
    if let Some(ref traces_to) = candidate.traces_to {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":proposal/traces-to"),
            Value::String(traces_to.clone()),
            tx_id,
            Op::Assert,
        ));
    }

    datoms
}

/// Accept a proposal: transition its status to accepted and generate
/// promotion-ready `:spec/*` datoms.
///
/// This reads the proposal entity from the store, verifies it exists and
/// is in a proposable state (`:proposal.status/proposed` or
/// `:proposal.status/reviewed`), then produces datoms that:
///
/// 1. Assert `:proposal/status :proposal.status/accepted` (append-only update).
/// 2. Assert `:element/*` and `:spec/*` attributes based on the proposal content,
///    making the entity a first-class spec element.
///
/// # Returns
///
/// Datoms to transact, or an empty vector if the proposal entity is
/// not found or already accepted/rejected.
pub fn accept_proposal(store: &Store, proposal_entity: EntityId, accept_tx: TxId) -> Vec<Datom> {
    let entity_datoms = store.entity_datoms(proposal_entity);
    if entity_datoms.is_empty() {
        return Vec::new();
    }

    // Check LATEST status — only accept from proposed or reviewed.
    // Must use max_by_key(d.tx) to find the most recent status assertion,
    // since the store is append-only and may contain multiple status datoms.
    let current_status = entity_datoms
        .iter()
        .filter(|d| d.attribute.as_str() == ":proposal/status" && d.op == Op::Assert)
        .max_by_key(|d| d.tx)
        .and_then(|d| {
            if let Value::Keyword(ref k) = d.value {
                Some(k.clone())
            } else {
                None
            }
        });

    match current_status.as_deref() {
        Some(":proposal.status/proposed") | Some(":proposal.status/reviewed") => {}
        _ => return Vec::new(), // Already accepted/rejected or unknown.
    }

    // Extract proposal fields for promotion.
    let suggested_id = entity_datoms.iter().find_map(|d| {
        if d.attribute.as_str() == ":proposal/suggested-id" && d.op == Op::Assert {
            if let Value::String(ref s) = d.value {
                return Some(s.clone());
            }
        }
        None
    });

    let statement = entity_datoms.iter().find_map(|d| {
        if d.attribute.as_str() == ":proposal/statement" && d.op == Op::Assert {
            if let Value::String(ref s) = d.value {
                return Some(s.clone());
            }
        }
        None
    });

    let proposal_type = entity_datoms.iter().find_map(|d| {
        if d.attribute.as_str() == ":proposal/type" && d.op == Op::Assert {
            if let Value::Keyword(ref k) = d.value {
                return Some(k.clone());
            }
        }
        None
    });

    let falsification = entity_datoms.iter().find_map(|d| {
        if d.attribute.as_str() == ":proposal/falsification" && d.op == Op::Assert {
            if let Value::String(ref s) = d.value {
                return Some(s.clone());
            }
        }
        None
    });

    let traces_to = entity_datoms.iter().find_map(|d| {
        if d.attribute.as_str() == ":proposal/traces-to" && d.op == Op::Assert {
            if let Value::String(ref s) = d.value {
                return Some(s.clone());
            }
        }
        None
    });

    let mut datoms = vec![
        // Transition status to accepted using the provided tx_id.
        Datom::new(
            proposal_entity,
            Attribute::from_keyword(":proposal/status"),
            Value::Keyword(":proposal.status/accepted".to_string()),
            accept_tx,
            Op::Assert,
        ),
    ];

    // Map proposal type to element type keyword.
    let element_type = match proposal_type.as_deref() {
        Some(":proposal.type/invariant") => ":element.type/invariant",
        Some(":proposal.type/adr") => ":element.type/adr",
        Some(":proposal.type/negative-case") => ":element.type/negative-case",
        _ => return datoms, // Unknown type — just update status.
    };

    // Add spec element attributes so this entity becomes a first-class spec element.
    datoms.push(Datom::new(
        proposal_entity,
        Attribute::from_keyword(":spec/element-type"),
        Value::Keyword(element_type.to_string()),
        accept_tx,
        Op::Assert,
    ));

    if let Some(ref id) = suggested_id {
        datoms.push(Datom::new(
            proposal_entity,
            Attribute::from_keyword(":spec/id"),
            Value::String(id.clone()),
            accept_tx,
            Op::Assert,
        ));
    }

    if let Some(ref stmt) = statement {
        datoms.push(Datom::new(
            proposal_entity,
            Attribute::from_keyword(":spec/statement"),
            Value::String(stmt.clone()),
            accept_tx,
            Op::Assert,
        ));
    }

    if let Some(ref fals) = falsification {
        datoms.push(Datom::new(
            proposal_entity,
            Attribute::from_keyword(":spec/falsification"),
            Value::String(fals.clone()),
            accept_tx,
            Op::Assert,
        ));
    }

    if let Some(ref trace) = traces_to {
        datoms.push(Datom::new(
            proposal_entity,
            Attribute::from_keyword(":element/traces-to"),
            Value::String(trace.clone()),
            accept_tx,
            Op::Assert,
        ));
    }

    // Derive namespace from suggested_id (e.g., "INV-STORE-017" -> "STORE").
    if let Some(ref id) = suggested_id {
        let parts: Vec<&str> = id.split('-').collect();
        if parts.len() >= 2 {
            let ns = format!(":element.ns/{}", parts[1].to_lowercase());
            datoms.push(Datom::new(
                proposal_entity,
                Attribute::from_keyword(":spec/namespace"),
                Value::Keyword(ns),
                accept_tx,
                Op::Assert,
            ));
        }
    }

    datoms
}

/// Reject a proposal with a rationale note.
///
/// Produces datoms that transition the proposal to `:proposal.status/rejected`
/// and record the reviewer and their reasoning. Append-only: the original
/// proposal datoms remain (C1).
///
/// # Arguments
///
/// * `proposal_entity` - Entity ID of the proposal to reject.
/// * `reason` - Human-readable rationale for rejection.
/// * `reviewer` - Entity ID of the reviewing agent or human.
/// * `tx_id` - Transaction ID for the rejection datoms.
pub fn reject_proposal(
    proposal_entity: EntityId,
    reason: &str,
    reviewer: EntityId,
    tx_id: TxId,
) -> Vec<Datom> {
    vec![
        Datom::new(
            proposal_entity,
            Attribute::from_keyword(":proposal/status"),
            Value::Keyword(":proposal.status/rejected".to_string()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            proposal_entity,
            Attribute::from_keyword(":proposal/reviewer"),
            Value::Ref(reviewer),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            proposal_entity,
            Attribute::from_keyword(":proposal/review-note"),
            Value::String(reason.to_string()),
            tx_id,
            Op::Assert,
        ),
    ]
}

/// Query the store for all pending proposals (status = proposed).
///
/// Returns tuples of (entity_id, suggested_id, confidence) sorted by
/// confidence descending (highest confidence first).
pub fn pending_proposals(store: &Store) -> Vec<(EntityId, String, f64)> {
    let status_datoms = store.attribute_datoms(&Attribute::from_keyword(":proposal/status"));

    // Collect entities whose latest status assertion is "proposed".
    let proposed_entities: Vec<EntityId> = status_datoms
        .iter()
        .filter(|d| {
            d.op == Op::Assert
                && matches!(&d.value, Value::Keyword(k) if k == ":proposal.status/proposed")
        })
        .map(|d| d.entity)
        .collect();

    // For each proposed entity, we need to verify it hasn't been superseded
    // by a later status assertion (accepted/rejected). Since the store is
    // append-only, a later assertion with a different status overrides.
    let mut results: Vec<(EntityId, String, f64)> = Vec::new();

    for entity in proposed_entities {
        let entity_datoms = store.entity_datoms(entity);

        // Find the latest status — if it's still "proposed", include it.
        let latest_status = entity_datoms
            .iter()
            .filter(|d| d.attribute.as_str() == ":proposal/status" && d.op == Op::Assert)
            .max_by_key(|d| d.tx)
            .and_then(|d| {
                if let Value::Keyword(ref k) = d.value {
                    Some(k.clone())
                } else {
                    None
                }
            });

        if latest_status.as_deref() != Some(":proposal.status/proposed") {
            continue; // Status has been updated to something else.
        }

        let suggested_id = entity_datoms
            .iter()
            .find_map(|d| {
                if d.attribute.as_str() == ":proposal/suggested-id" && d.op == Op::Assert {
                    if let Value::String(ref s) = d.value {
                        return Some(s.clone());
                    }
                }
                None
            })
            .unwrap_or_default();

        let confidence = entity_datoms
            .iter()
            .find_map(|d| {
                if d.attribute.as_str() == ":proposal/confidence" && d.op == Op::Assert {
                    if let Value::Double(ordered_float::OrderedFloat(c)) = d.value {
                        return Some(c);
                    }
                }
                None
            })
            .unwrap_or(0.0);

        results.push((entity, suggested_id, confidence));
    }

    // Sort by confidence descending.
    results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    results
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::AgentId;
    use crate::harvest::{propose_adr, propose_invariant, propose_negative};
    use crate::schema::{domain_schema_datoms, genesis_datoms, layer_3_datoms, layer_4_datoms};
    use crate::store::Store;
    use std::collections::BTreeSet;

    /// Build a store with all schema layers installed (L0 through L4).
    fn store_with_full_schema() -> Store {
        let agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, agent);
        let domain_tx = TxId::new(1, 0, agent);
        let l3_tx = TxId::new(2, 0, agent);
        let l4_tx = TxId::new(3, 0, agent);

        let mut datoms: BTreeSet<Datom> = genesis_datoms(genesis_tx).into_iter().collect();
        for d in domain_schema_datoms(domain_tx) {
            datoms.insert(d);
        }
        for d in layer_3_datoms(l3_tx) {
            datoms.insert(d);
        }
        for d in layer_4_datoms(l4_tx) {
            datoms.insert(d);
        }
        Store::from_datoms(datoms)
    }

    /// Rebuild a store from its existing datoms plus additional datoms.
    fn store_with(store: &Store, extra: impl IntoIterator<Item = Datom>) -> Store {
        let mut datoms: BTreeSet<Datom> = store.datom_set().clone();
        for d in extra {
            datoms.insert(d);
        }
        Store::from_datoms(datoms)
    }

    #[test]
    fn proposal_to_datoms_creates_correct_entity() {
        let source = EntityId::from_ident(":test/source-entity");
        let candidate = propose_invariant(source, "The store must never delete datoms", 0.85);
        let agent = AgentId::from_name("test:agent");
        let tx = TxId::new(10, 0, agent);
        let datoms = proposal_to_datoms(&candidate, tx);

        // Should have: type, status, source, suggested-id, statement, confidence = 6
        // Plus falsification if present.
        assert!(
            datoms.len() >= 6,
            "expected at least 6 datoms, got {}",
            datoms.len()
        );

        // All datoms share the same content-addressed entity ID.
        let entity = datoms[0].entity;
        assert!(datoms.iter().all(|d| d.entity == entity));

        // Verify status is proposed.
        let status = datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":proposal/status");
        assert!(status.is_some());
        assert_eq!(
            status.unwrap().value,
            Value::Keyword(":proposal.status/proposed".to_string())
        );

        // Verify type is invariant.
        let ptype = datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":proposal/type");
        assert!(ptype.is_some());
        assert_eq!(
            ptype.unwrap().value,
            Value::Keyword(":proposal.type/invariant".to_string())
        );
    }

    #[test]
    fn proposal_to_datoms_includes_optional_fields() {
        let source = EntityId::from_ident(":test/source-entity");
        let mut candidate = propose_invariant(
            source,
            "Every spec element has a falsification condition",
            0.95,
        );
        candidate.traces_to = Some("SEED.md section 4".to_string());

        let agent = AgentId::from_name("test:agent");
        let tx = TxId::new(10, 0, agent);
        let datoms = proposal_to_datoms(&candidate, tx);

        let traces = datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":proposal/traces-to");
        assert!(traces.is_some(), "traces-to should be present");

        let fals = datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":proposal/falsification");
        assert!(
            fals.is_some(),
            "falsification should be present for invariants"
        );
    }

    #[test]
    fn reject_proposal_produces_three_datoms() {
        let proposal_entity = EntityId::from_ident(":test/proposal-1");
        let reviewer = EntityId::from_ident(":agent/reviewer-1");
        let agent = AgentId::from_name("test:agent");
        let tx = TxId::new(20, 0, agent);

        let datoms = reject_proposal(
            proposal_entity,
            "Too vague for a formal invariant",
            reviewer,
            tx,
        );

        assert_eq!(datoms.len(), 3);

        let status = datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":proposal/status");
        assert_eq!(
            status.unwrap().value,
            Value::Keyword(":proposal.status/rejected".to_string())
        );

        let note = datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":proposal/review-note");
        assert_eq!(
            note.unwrap().value,
            Value::String("Too vague for a formal invariant".to_string())
        );

        let rev = datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":proposal/reviewer");
        assert_eq!(rev.unwrap().value, Value::Ref(reviewer));
    }

    #[test]
    fn auto_accept_threshold_is_0_9() {
        assert!((auto_accept_threshold() - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn pending_proposals_returns_proposed_entities() {
        let base = store_with_full_schema();
        let agent = AgentId::from_name("test:agent");

        // Create two proposals with different confidences.
        let source1 = EntityId::from_ident(":test/obs-1");
        let candidate1 = propose_invariant(source1, "Datoms are immutable", 0.85);
        let tx1 = TxId::new(100, 0, agent);
        let p1_datoms = proposal_to_datoms(&candidate1, tx1);
        let p1_entity = p1_datoms[0].entity;

        let source2 = EntityId::from_ident(":test/obs-2");
        let candidate2 = propose_adr(source2, "Use EAV over relational", 0.92);
        let tx2 = TxId::new(101, 0, agent);
        let p2_datoms = proposal_to_datoms(&candidate2, tx2);
        let p2_entity = p2_datoms[0].entity;

        // Insert both into the store via rebuild.
        let store = store_with(&base, p1_datoms.into_iter().chain(p2_datoms));

        let pending = pending_proposals(&store);
        assert_eq!(pending.len(), 2, "should have 2 pending proposals");

        // Sorted by confidence descending: p2 (0.92) first, p1 (0.85) second.
        assert_eq!(pending[0].0, p2_entity);
        assert!((pending[0].2 - 0.92).abs() < f64::EPSILON);
        assert_eq!(pending[1].0, p1_entity);
        assert!((pending[1].2 - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn pending_proposals_excludes_rejected() {
        let base = store_with_full_schema();
        let agent = AgentId::from_name("test:agent");

        // Create and insert a proposal.
        let source = EntityId::from_ident(":test/obs-3");
        let candidate = propose_invariant(source, "Schema is append-only", 0.75);
        let tx1 = TxId::new(100, 0, agent);
        let p_datoms = proposal_to_datoms(&candidate, tx1);
        let p_entity = p_datoms[0].entity;

        let store_proposed = store_with(&base, p_datoms);
        assert_eq!(pending_proposals(&store_proposed).len(), 1);

        // Reject it.
        let reviewer = EntityId::from_ident(":agent/reviewer");
        let tx2 = TxId::new(101, 0, agent);
        let rej = reject_proposal(p_entity, "Duplicate of INV-SCHEMA-003", reviewer, tx2);
        let store_rejected = store_with(&store_proposed, rej);

        assert_eq!(pending_proposals(&store_rejected).len(), 0);
    }

    #[test]
    fn accept_proposal_generates_spec_datoms() {
        let base = store_with_full_schema();
        let agent = AgentId::from_name("test:agent");

        // Create and insert a proposal.
        let source = EntityId::from_ident(":test/obs-4");
        let mut candidate = propose_invariant(source, "Merge is set union", 0.95);
        candidate.traces_to = Some("SEED.md section 5".to_string());

        let tx1 = TxId::new(100, 0, agent);
        let p_datoms = proposal_to_datoms(&candidate, tx1);
        let p_entity = p_datoms[0].entity;

        let store = store_with(&base, p_datoms);

        // Accept the proposal (later tx_id for correct status ordering).
        let tx2 = TxId::new(200, 0, agent);
        let accept_datoms = accept_proposal(&store, p_entity, tx2);
        assert!(!accept_datoms.is_empty(), "accept should produce datoms");

        // Should contain status transition to accepted.
        let status = accept_datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":proposal/status");
        assert_eq!(
            status.unwrap().value,
            Value::Keyword(":proposal.status/accepted".to_string())
        );

        // Should contain spec element type.
        let elem_type = accept_datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":spec/element-type");
        assert!(elem_type.is_some(), "should add :spec/element-type");
        assert_eq!(
            elem_type.unwrap().value,
            Value::Keyword(":element.type/invariant".to_string())
        );

        // Should contain spec/id.
        let spec_id = accept_datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":spec/id");
        assert!(spec_id.is_some(), "should add :spec/id");

        // Should derive namespace from suggested-id.
        let ns = accept_datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":spec/namespace");
        assert!(ns.is_some(), "should derive :spec/namespace");
    }

    #[test]
    fn accept_proposal_noop_for_unknown_entity() {
        let store = store_with_full_schema();
        let unknown = EntityId::from_ident(":test/nonexistent");
        let tx = TxId::new(999, 0, AgentId::from_name("test"));
        let datoms = accept_proposal(&store, unknown, tx);
        assert!(datoms.is_empty(), "should return empty for unknown entity");
    }

    #[test]
    fn accept_proposal_noop_for_already_rejected() {
        let base = store_with_full_schema();
        let agent = AgentId::from_name("test:agent");

        // Create, insert, and reject a proposal.
        let source = EntityId::from_ident(":test/obs-5");
        let candidate = propose_negative(source, "Must not delete datoms", 0.8);
        let tx1 = TxId::new(100, 0, agent);
        let p_datoms = proposal_to_datoms(&candidate, tx1);
        let p_entity = p_datoms[0].entity;

        let store_proposed = store_with(&base, p_datoms);

        let reviewer = EntityId::from_ident(":agent/reviewer");
        let tx2 = TxId::new(101, 0, agent);
        let rej = reject_proposal(p_entity, "Already covered", reviewer, tx2);
        let store_rejected = store_with(&store_proposed, rej);

        // Attempting to accept a rejected proposal should be a no-op.
        let tx3 = TxId::new(200, 0, agent);
        let accept_datoms = accept_proposal(&store_rejected, p_entity, tx3);
        assert!(
            accept_datoms.is_empty(),
            "should not accept a rejected proposal"
        );
    }

    #[test]
    fn full_lifecycle_propose_review_accept() {
        let base = store_with_full_schema();
        let agent = AgentId::from_name("test:agent");

        // Step 1: Propose.
        let source = EntityId::from_ident(":test/obs-lifecycle");
        let candidate = propose_invariant(source, "The frontier advances monotonically", 0.88);
        let tx1 = TxId::new(100, 0, agent);
        let p_datoms = proposal_to_datoms(&candidate, tx1);
        let p_entity = p_datoms[0].entity;

        let store_proposed = store_with(&base, p_datoms);

        // Verify it appears in pending.
        let pending = pending_proposals(&store_proposed);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].0, p_entity);

        // Step 2: Accept (with a LATER tx_id so status ordering is correct).
        let tx2 = TxId::new(200, 0, agent);
        let accept_datoms = accept_proposal(&store_proposed, p_entity, tx2);
        assert!(!accept_datoms.is_empty());
        let store_accepted = store_with(&store_proposed, accept_datoms);

        // Step 3: Verify no longer pending.
        let pending_after = pending_proposals(&store_accepted);
        assert_eq!(
            pending_after.len(),
            0,
            "accepted proposal should not be pending"
        );
    }

    #[test]
    fn full_lifecycle_propose_review_reject() {
        let base = store_with_full_schema();
        let agent = AgentId::from_name("test:agent");

        // Step 1: Propose.
        let source = EntityId::from_ident(":test/obs-reject-lifecycle");
        let candidate = propose_adr(source, "Use JSON over EDN for config", 0.6);
        let tx1 = TxId::new(100, 0, agent);
        let p_datoms = proposal_to_datoms(&candidate, tx1);
        let p_entity = p_datoms[0].entity;

        let store_proposed = store_with(&base, p_datoms);
        assert_eq!(pending_proposals(&store_proposed).len(), 1);

        // Step 2: Reject.
        let reviewer = EntityId::from_ident(":agent/human-reviewer");
        let tx2 = TxId::new(101, 0, agent);
        let rej = reject_proposal(
            p_entity,
            "EDN is the native format per ADR-STORE-007",
            reviewer,
            tx2,
        );
        let store_rejected = store_with(&store_proposed, rej);

        // Step 3: Verify no longer pending.
        assert_eq!(pending_proposals(&store_rejected).len(), 0);
    }

    #[test]
    fn proposal_entity_is_content_addressed() {
        let source = EntityId::from_ident(":test/same-source");
        let c1 = propose_invariant(source, "Same statement", 0.8);
        let c2 = propose_invariant(source, "Same statement", 0.8);
        let agent = AgentId::from_name("test:agent");
        let tx = TxId::new(10, 0, agent);

        let d1 = proposal_to_datoms(&c1, tx);
        let d2 = proposal_to_datoms(&c2, tx);

        // Same content -> same entity ID (C2: content-addressable identity).
        assert_eq!(d1[0].entity, d2[0].entity);
    }

    #[test]
    fn different_proposals_get_different_entities() {
        let source = EntityId::from_ident(":test/source");
        let c1 = propose_invariant(source, "Statement A", 0.8);
        let c2 = propose_invariant(source, "Statement B", 0.8);
        let agent = AgentId::from_name("test:agent");
        let tx = TxId::new(10, 0, agent);

        let d1 = proposal_to_datoms(&c1, tx);
        let d2 = proposal_to_datoms(&c2, tx);

        assert_ne!(
            d1[0].entity, d2[0].entity,
            "different content must produce different entity IDs"
        );
    }
}
