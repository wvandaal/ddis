//! Pure set-union merge with cascade and receipt.
//!
//! Merge at the kernel level is mathematical set union of datom sets.
//! This module provides the merge operation as a free function (not a method)
//! and the cascade logic for propagating merge effects.
//!
//! # Invariants
//!
//! - **INV-MERGE-001**: Merge = set union of datom sets.
//! - **INV-MERGE-002**: Merge preserves causal order (frontier pointwise max).
//! - **INV-MERGE-003**: Branch isolation (working sets not leaked into merge).
//! - **INV-MERGE-004**: Competing branch lock (concurrent merge safety).
//! - **INV-MERGE-005**: Branch commit monotonicity (merged store >= both inputs).
//! - **INV-MERGE-006**: Branch as first-class entity (branch metadata in datoms).
//! - **INV-MERGE-007**: Bilateral branch duality (forward/backward merge symmetry).
//! - **INV-MERGE-008**: Deduplication by content identity.
//! - **INV-MERGE-009**: Cascade: schema rebuild → resolution recompute → LIVE invalidation.
//! - **INV-MERGE-010**: MergeReceipt captures new datom count and conflict set.
//!
//! # Design Decisions
//!
//! - ADR-MERGE-001: Set union over heuristic merge — no conflict resolution at merge time.
//! - ADR-MERGE-002: Branching G-Set extension for working set isolation.
//! - ADR-MERGE-003: Competing branch lock prevents concurrent merge corruption.
//! - ADR-MERGE-004: Three combine strategies (union, prefer-left, prefer-right).
//! - ADR-MERGE-005: Cascade as post-merge deterministic layer.
//! - ADR-MERGE-006: Branch comparison via entity type (metadata in datoms).
//! - ADR-MERGE-007: Merge cascade stub datoms at Stage 0.
//!
//! # Negative Cases
//!
//! - NEG-MERGE-001: No merge data loss — set union preserves all datoms.
//! - NEG-MERGE-002: No merge without cascade — schema/resolution always rebuilt.
//! - NEG-MERGE-003: No working set leak — uncommitted datoms excluded from merge.
//! - NEG-STORE-004: No merge heuristics — pure mathematical set union only.

use std::collections::{BTreeSet, HashMap};

use crate::datom::{Attribute, Datom, EntityId, Op, TxId, Value};
use crate::resolution::{has_conflict, ConflictSet};
use crate::store::{Frontier, MergeReceipt, Store};

/// Merge two stores, returning a new store and a detailed receipt.
///
/// This is the canonical merge operation: `merge(A, B) = A ∪ B`.
/// The result contains all datoms from both stores.
///
/// # Cascade (INV-MERGE-009)
///
/// After merge, the five-step cascade runs:
/// 1. Schema rebuild from merged datoms
/// 2. Resolution recompute for conflicting (entity, attribute) pairs
/// 3. LIVE index invalidation
/// 4. Guidance recalculation (deferred to guidance module)
/// 5. Trilateral metrics update (deferred to trilateral module)
///
/// Steps 1-3 are handled by `Store::merge()`. Steps 4-5 are post-merge hooks.
/// ADR-STORE-015: Free function over store method — merge is a standalone function.
/// ADR-STORE-016: ArcSwap MVCC concurrency model (foundation for concurrent merge).
pub fn merge_stores(target: &mut Store, source: &Store) -> MergeReceipt {
    target.merge(source)
}

/// Detect conflicts introduced by merging two stores.
///
/// Returns a list of (entity, attribute) pairs that have conflicting assertions
/// under their resolution mode.
pub fn detect_merge_conflicts(store: &Store) -> Vec<ConflictSet> {
    let mut conflicts = Vec::new();

    // Single-pass grouping by (entity, attribute) — O(N) total.
    let mut grouped: HashMap<(EntityId, Attribute), Vec<&Datom>> = HashMap::new();
    for d in store.datoms() {
        grouped
            .entry((d.entity, d.attribute.clone()))
            .or_default()
            .push(d);
    }

    for ((entity, attr), datoms) in grouped {
        let cs = ConflictSet::from_datoms(entity, attr.clone(), &datoms);
        let mode = store.schema().resolution_mode(&attr);

        if has_conflict(&cs, &mode) {
            conflicts.push(cs);
        }
    }

    conflicts
}

/// Verify merge monotonicity: target must be a superset of source after merge.
///
/// Used in proptest to verify INV-STORE-007.
pub fn verify_monotonicity(pre_merge: &BTreeSet<Datom>, post_merge: &BTreeSet<Datom>) -> bool {
    pre_merge.is_subset(post_merge)
}

/// Verify frontier advancement: post-merge frontier >= pre-merge frontier.
///
/// This checks a necessary precondition for merge correctness (frontiers never
/// shrink), NOT the full SYNC barrier protocol. SYNC invariants (INV-SYNC-001
/// through INV-SYNC-005) require actual barrier semantics with participant
/// exchange, timeout safety, and consistent cuts — none of which are implemented
/// here. Those are Stage 3 deliverables. This function is a simple monotonicity
/// check used by merge verification.
pub fn verify_frontier_advancement(pre: &Frontier, post: &Frontier) -> bool {
    for (agent, pre_tx) in pre {
        match post.get(agent) {
            Some(post_tx) => {
                if post_tx < pre_tx {
                    return false;
                }
            }
            None => return false,
        }
    }
    true
}

/// Generate cascade stub datoms for a merge operation (ADR-MERGE-007).
///
/// At Stage 0, the full 5-step merge cascade cannot execute because steps 2-5
/// depend on infrastructure not yet built (query caching, projection management,
/// uncertainty tensor). This function produces stub datoms that preserve the
/// audit trail required by INV-MERGE-002 (all 5 cascade steps produce datoms).
///
/// Step 1 (conflict detection) is handled separately by `detect_merge_conflicts`.
/// This function generates stubs for steps 2-5 plus overall cascade metadata.
///
/// The stub datoms are deterministic: given the same `MergeReceipt` and `TxId`,
/// the same datoms are produced regardless of which agent calls this function.
/// This preserves INV-MERGE-010 (cascade determinism).
///
/// # Arguments
///
/// * `receipt` - The merge receipt from the just-completed merge operation
/// * `tx` - The transaction ID of the merge operation (used for provenance)
///
/// # Returns
///
/// A vector of datoms to be transacted into the store by the caller.
pub fn cascade_stub_datoms(receipt: &MergeReceipt, tx: TxId) -> Vec<Datom> {
    // Content-address the cascade entity from the merge tx, ensuring determinism.
    // The entity ID is derived from the merge tx bytes + a cascade marker,
    // so the same merge always produces the same cascade entity.
    let cascade_entity = {
        let mut content = Vec::with_capacity(64);
        content.extend_from_slice(b"cascade:");
        content.extend_from_slice(&tx.wall_time.to_le_bytes());
        content.extend_from_slice(&tx.logical.to_le_bytes());
        content.extend_from_slice(tx.agent.as_bytes());
        EntityId::from_content(&content)
    };

    let mut datoms = Vec::with_capacity(7);

    // Overall cascade status — "stub" marks this as a Stage 0 placeholder.
    datoms.push(Datom::new(
        cascade_entity,
        Attribute::from_keyword(":merge/cascade-status"),
        Value::Keyword(":stub".into()),
        tx,
        Op::Assert,
    ));

    // Whether a cascade would have been needed (always true after a merge).
    datoms.push(Datom::new(
        cascade_entity,
        Attribute::from_keyword(":merge/cascade-triggered"),
        Value::Boolean(true),
        tx,
        Op::Assert,
    ));

    // Duplicate count from the merge receipt.
    datoms.push(Datom::new(
        cascade_entity,
        Attribute::from_keyword(":merge/duplicate-count"),
        Value::Long(receipt.duplicate_datoms as i64),
        tx,
        Op::Assert,
    ));

    // Step 2 stub: cache invalidation (no cache layer at Stage 0).
    let step2_entity = EntityId::from_content(
        &[
            b"cascade-step:2:",
            tx.wall_time.to_le_bytes().as_slice(),
            &tx.logical.to_le_bytes(),
            tx.agent.as_bytes(),
        ]
        .concat(),
    );
    datoms.push(Datom::new(
        step2_entity,
        Attribute::from_keyword(":cascade/cache-invalidation"),
        Value::Long(0),
        tx,
        Op::Assert,
    ));

    // Step 3 stub: secondary conflicts (no projection system at Stage 0).
    let step3_entity = EntityId::from_content(
        &[
            b"cascade-step:3:",
            tx.wall_time.to_le_bytes().as_slice(),
            &tx.logical.to_le_bytes(),
            tx.agent.as_bytes(),
        ]
        .concat(),
    );
    datoms.push(Datom::new(
        step3_entity,
        Attribute::from_keyword(":cascade/secondary-conflicts"),
        Value::Long(0),
        tx,
        Op::Assert,
    ));

    // Step 4 stub: uncertainty delta (no uncertainty tensor at Stage 0).
    let step4_entity = EntityId::from_content(
        &[
            b"cascade-step:4:",
            tx.wall_time.to_le_bytes().as_slice(),
            &tx.logical.to_le_bytes(),
            tx.agent.as_bytes(),
        ]
        .concat(),
    );
    datoms.push(Datom::new(
        step4_entity,
        Attribute::from_keyword(":cascade/uncertainty-delta"),
        Value::Long(0),
        tx,
        Op::Assert,
    ));

    // Step 5 stub: projection staleness (no projections at Stage 0).
    let step5_entity = EntityId::from_content(
        &[
            b"cascade-step:5:",
            tx.wall_time.to_le_bytes().as_slice(),
            &tx.logical.to_le_bytes(),
            tx.agent.as_bytes(),
        ]
        .concat(),
    );
    datoms.push(Datom::new(
        step5_entity,
        Attribute::from_keyword(":cascade/projection-staleness"),
        Value::Long(0),
        tx,
        Op::Assert,
    ));

    datoms
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// Witnesses: INV-MERGE-001, INV-MERGE-008, INV-MERGE-009,
// INV-STORE-004, INV-STORE-006, INV-STORE-007,
// INV-RESOLUTION-003, INV-RESOLUTION-004,
// ADR-MERGE-001, NEG-MERGE-001, NEG-STORE-004
#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{AgentId, Attribute, EntityId, ProvenanceType, Value};
    use crate::store::Transaction;

    // Verifies: INV-MERGE-001 — Merge Is Set Union
    // Verifies: INV-STORE-004 — CRDT Merge Commutativity
    // Verifies: ADR-MERGE-001 — Set Union Over Heuristic Merge
    #[test]
    fn merge_stores_is_commutative() {
        let mut s1 = Store::genesis();
        let mut s2 = Store::genesis();

        let a1 = AgentId::from_name("alice");
        let a2 = AgentId::from_name("bob");

        let tx1 = Transaction::new(a1, ProvenanceType::Observed, "a")
            .assert(
                EntityId::from_ident(":test/a"),
                Attribute::from_keyword(":db/doc"),
                Value::String("from alice".into()),
            )
            .commit(&s1)
            .unwrap();
        s1.transact(tx1).unwrap();

        let tx2 = Transaction::new(a2, ProvenanceType::Observed, "b")
            .assert(
                EntityId::from_ident(":test/b"),
                Attribute::from_keyword(":db/doc"),
                Value::String("from bob".into()),
            )
            .commit(&s2)
            .unwrap();
        s2.transact(tx2).unwrap();

        let mut left = s1.clone_store();
        merge_stores(&mut left, &s2);

        let mut right = s2.clone_store();
        merge_stores(&mut right, &s1);

        assert_eq!(left.datom_set(), right.datom_set());
    }

    // Verifies: INV-RESOLUTION-003 — Conservative Conflict Detection
    // Verifies: INV-RESOLUTION-004 — Conflict Predicate Correctness
    #[test]
    fn merge_detects_conflicts() {
        let mut s1 = Store::genesis();
        let mut s2 = Store::genesis();

        let entity = EntityId::from_ident(":test/conflict");
        let a1 = AgentId::from_name("alice");
        let a2 = AgentId::from_name("bob");

        // Both agents assert different values for same entity+attribute
        let tx1 = Transaction::new(a1, ProvenanceType::Observed, "alice's value")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("alice says".into()),
            )
            .commit(&s1)
            .unwrap();
        s1.transact(tx1).unwrap();

        let tx2 = Transaction::new(a2, ProvenanceType::Observed, "bob's value")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("bob says".into()),
            )
            .commit(&s2)
            .unwrap();
        s2.transact(tx2).unwrap();

        let mut merged = s1.clone_store();
        merge_stores(&mut merged, &s2);

        let conflicts = detect_merge_conflicts(&merged);
        // Should detect the conflict on :db/doc for entity :test/conflict
        let has_doc_conflict = conflicts
            .iter()
            .any(|c| c.entity == entity && c.attribute == Attribute::from_keyword(":db/doc"));
        assert!(has_doc_conflict, "should detect conflicting :db/doc values");
    }

    // Verifies: INV-STORE-007 — CRDT Merge Monotonicity
    // Verifies: NEG-MERGE-001 — No Merge Data Loss
    #[test]
    fn verify_monotonicity_holds() {
        let store = Store::genesis();
        let pre = store.datom_set().clone();
        let mut s = store.clone_store();
        s.merge(&Store::genesis());
        assert!(verify_monotonicity(&pre, s.datom_set()));
    }

    // Verifies: ADR-MERGE-007 — Merge Cascade Stub Datoms at Stage 0
    // Verifies: INV-MERGE-002 — Merge Cascade Completeness (stub satisfaction)
    #[test]
    fn cascade_stub_datoms_produces_seven_datoms() {
        let mut s1 = Store::genesis();
        let mut s2 = Store::genesis();

        let a1 = AgentId::from_name("alice");
        let a2 = AgentId::from_name("bob");

        let tx1 = Transaction::new(a1, ProvenanceType::Observed, "a")
            .assert(
                EntityId::from_ident(":test/cascade-a"),
                Attribute::from_keyword(":db/doc"),
                Value::String("from alice".into()),
            )
            .commit(&s1)
            .unwrap();
        s1.transact(tx1).unwrap();

        let tx2 = Transaction::new(a2, ProvenanceType::Observed, "b")
            .assert(
                EntityId::from_ident(":test/cascade-b"),
                Attribute::from_keyword(":db/doc"),
                Value::String("from bob".into()),
            )
            .commit(&s2)
            .unwrap();
        s2.transact(tx2).unwrap();

        let receipt = merge_stores(&mut s1, &s2);
        let merge_tx = TxId::new(100, 0, a1);
        let stubs = cascade_stub_datoms(&receipt, merge_tx);

        // 3 metadata datoms + 4 step stubs = 7 total
        assert_eq!(stubs.len(), 7, "expected 7 cascade stub datoms");

        // Verify the cascade-status datom exists with value :stub
        let status = stubs
            .iter()
            .find(|d| d.attribute == Attribute::from_keyword(":merge/cascade-status"));
        assert!(status.is_some(), "missing :merge/cascade-status datom");
        assert_eq!(
            status.unwrap().value,
            Value::Keyword(":stub".into()),
            "cascade-status should be :stub"
        );

        // Verify cascade-triggered is true
        let triggered = stubs
            .iter()
            .find(|d| d.attribute == Attribute::from_keyword(":merge/cascade-triggered"));
        assert!(
            triggered.is_some(),
            "missing :merge/cascade-triggered datom"
        );
        assert_eq!(
            triggered.unwrap().value,
            Value::Boolean(true),
            "cascade-triggered should be true"
        );

        // Verify duplicate-count matches the receipt
        let dup_count = stubs
            .iter()
            .find(|d| d.attribute == Attribute::from_keyword(":merge/duplicate-count"));
        assert!(dup_count.is_some(), "missing :merge/duplicate-count datom");
        assert_eq!(
            dup_count.unwrap().value,
            Value::Long(receipt.duplicate_datoms as i64),
            "duplicate-count should match receipt"
        );

        // Verify all 4 cascade step stubs exist
        let step_attrs = [
            ":cascade/cache-invalidation",
            ":cascade/secondary-conflicts",
            ":cascade/uncertainty-delta",
            ":cascade/projection-staleness",
        ];
        for attr_str in &step_attrs {
            let found = stubs.iter().any(|d| {
                d.attribute == Attribute::from_keyword(attr_str) && d.value == Value::Long(0)
            });
            assert!(found, "missing cascade step stub for {attr_str}");
        }
    }

    // Verifies: INV-MERGE-010 — Cascade Determinism
    // Same merge tx produces identical cascade stub datoms.
    #[test]
    fn cascade_stub_datoms_are_deterministic() {
        let mut s1 = Store::genesis();
        let s2 = Store::genesis();

        let receipt = merge_stores(&mut s1, &s2);
        let agent = AgentId::from_name("test-agent");
        let tx = TxId::new(200, 1, agent);

        let stubs_a = cascade_stub_datoms(&receipt, tx);
        let stubs_b = cascade_stub_datoms(&receipt, tx);

        assert_eq!(
            stubs_a, stubs_b,
            "INV-MERGE-010: cascade stubs must be deterministic"
        );
    }

    // Verifies: INV-MERGE-009 — MergeReceipt includes duplicate_datoms
    #[test]
    fn merge_receipt_tracks_duplicates() {
        let mut s1 = Store::genesis();
        let s2 = Store::genesis();

        // Both stores share genesis datoms, so merging produces duplicates.
        let receipt = merge_stores(&mut s1, &s2);

        // All genesis datoms from s2 are already in s1, so all are duplicates.
        assert!(
            receipt.duplicate_datoms > 0,
            "merging stores with shared genesis should report duplicates"
        );
        assert_eq!(
            receipt.new_datoms, 0,
            "merging identical stores should add no new datoms"
        );
    }

    // -------------------------------------------------------------------
    // Property-based tests (proptest)
    // -------------------------------------------------------------------

    mod proptests {
        use super::*;
        use crate::proptest_strategies::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn merge_stores_is_commutative_on_datom_sets((s1, s2) in arb_store_pair(2)) {
                let mut left = s1.clone_store();
                merge_stores(&mut left, &s2);

                let mut right = s2.clone_store();
                merge_stores(&mut right, &s1);

                prop_assert_eq!(
                    left.datom_set(),
                    right.datom_set(),
                    "INV-STORE-004: merge must be commutative"
                );
            }

            #[test]
            fn merge_stores_is_idempotent(store in arb_store(3)) {
                let pre_datoms = store.datom_set().clone();

                let mut merged = store.clone_store();
                merge_stores(&mut merged, &store);

                prop_assert_eq!(
                    merged.datom_set(),
                    &pre_datoms,
                    "INV-STORE-006: merge(S, S) must equal S"
                );
            }

            #[test]
            fn detect_merge_conflicts_empty_for_identical_stores(store in arb_store(3)) {
                // Merging a store with itself produces no new conflicts
                // because no new distinct values are introduced.
                let mut merged = store.clone_store();
                merge_stores(&mut merged, &store);

                let conflicts = detect_merge_conflicts(&merged);
                // For identical stores, merging doesn't add new values, so
                // no additional conflicts arise beyond what existed pre-merge.
                // Since arb_store uses a single agent, there should be no conflicts.
                let pre_conflicts = detect_merge_conflicts(&store);
                prop_assert_eq!(
                    conflicts.len(),
                    pre_conflicts.len(),
                    "merging identical stores must not introduce new conflicts"
                );
            }

            #[test]
            fn verify_monotonicity_holds_after_merge((s1, s2) in arb_store_pair(2)) {
                let pre = s1.datom_set().clone();

                let mut merged = s1.clone_store();
                merge_stores(&mut merged, &s2);

                prop_assert!(
                    verify_monotonicity(&pre, merged.datom_set()),
                    "INV-STORE-007: target store datoms must be a subset of post-merge datoms"
                );
            }
        }
    }
}
