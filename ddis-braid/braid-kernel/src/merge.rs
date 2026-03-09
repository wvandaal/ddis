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
//! - **INV-MERGE-008**: Deduplication by content identity.
//! - **INV-MERGE-009**: Cascade: schema rebuild → resolution recompute → LIVE invalidation.
//! - **INV-MERGE-010**: MergeReceipt captures new datom count and conflict set.

use std::collections::BTreeSet;

use crate::datom::Datom;
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
pub fn merge_stores(target: &mut Store, source: &Store) -> MergeReceipt {
    target.merge(source)
}

/// Detect conflicts introduced by merging two stores.
///
/// Returns a list of (entity, attribute) pairs that have conflicting assertions
/// under their resolution mode.
pub fn detect_merge_conflicts(store: &Store) -> Vec<ConflictSet> {
    let mut conflicts = Vec::new();

    // Collect all unique (entity, attribute) pairs
    let mut pairs: BTreeSet<(crate::datom::EntityId, crate::datom::Attribute)> = BTreeSet::new();
    for d in store.datoms() {
        pairs.insert((d.entity, d.attribute.clone()));
    }

    for (entity, attr) in pairs {
        let datoms: Vec<&Datom> = store
            .datoms()
            .filter(|d| d.entity == entity && d.attribute == attr)
            .collect();

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{AgentId, Attribute, EntityId, ProvenanceType, Value};
    use crate::store::Transaction;

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

    #[test]
    fn verify_monotonicity_holds() {
        let store = Store::genesis();
        let pre = store.datom_set().clone();
        let mut s = store.clone_store();
        s.merge(&Store::genesis());
        assert!(verify_monotonicity(&pre, s.datom_set()));
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
