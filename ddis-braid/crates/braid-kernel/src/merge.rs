//! Pure set-union merge with cascade and receipt.
//!
//! Merge at the kernel level is mathematical set union of datom sets.
//! This module provides the merge operation as a free function (not a method)
//! and the cascade logic for propagating merge effects.
//!
//! # Invariants
//!
//! - **INV-MERGE-001**: Merge = set union of datom sets.
//! - **INV-MERGE-002**: Merge Cascade Completeness (all 5 cascade steps produce datoms).
//! - **INV-MERGE-003**: Branch isolation (working sets not leaked into merge).
//! - **INV-MERGE-004**: Competing branch lock (concurrent merge safety).
//! - **INV-MERGE-005**: Branch commit monotonicity (merged store >= both inputs).
//! - **INV-MERGE-006**: Branch as first-class entity (branch metadata in datoms).
//! - **INV-MERGE-007**: Bilateral branch duality (forward/backward merge symmetry).
//! - **INV-MERGE-008**: At-least-once idempotent delivery.
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
use crate::trilateral::{classify_attribute, AttrNamespace};

// ---------------------------------------------------------------------------
// CascadeReceipt
// ---------------------------------------------------------------------------

/// Aggregated result of all cascade steps after a merge operation.
///
/// At Stage 0, only step 1 (conflict detection) is fully implemented.
/// Steps 2-5 produce stub datoms that preserve the audit trail required
/// by INV-MERGE-009 (all 5 cascade steps produce datoms).
///
/// # Invariants
///
/// - **INV-MERGE-009**: Cascade completeness — all 5 steps produce datoms.
/// - **INV-MERGE-010**: MergeReceipt captures conflict set.
#[derive(Clone, Debug)]
pub struct CascadeReceipt {
    /// Number of conflicts detected in step 1.
    pub conflicts_detected: usize,
    /// The full set of detected conflicts (step 1).
    pub conflicts: Vec<ConflictSet>,
    /// Stub datoms for steps 2-5 plus cascade metadata.
    pub stub_datoms: Vec<Datom>,
    /// Number of cascade steps completed (1-5). At Stage 0, always 1
    /// (step 1 real, steps 2-5 are stubs).
    pub steps_completed: u8,
    /// Whether any schema-affecting datoms were in the merged set (step 2).
    /// True if any merged datom has attribute :db/valueType or :db/cardinality.
    pub schema_affected: bool,
    /// Entities with conflicting (entity, attribute) pairs (step 3).
    /// Deduplicated set of entity IDs that appear in at least one conflict.
    pub conflicted_entities: Vec<EntityId>,
    /// LIVE projection entities whose source entities changed (step 4).
    /// These entities belong to an Intent/Spec/Impl LIVE view and had new
    /// datoms introduced by the merge, so their projections may be stale.
    pub stale_live_views: Vec<EntityId>,
}

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

/// Cascade step 1: detect conflicts in the post-merge store.
///
/// Takes the merged store and the merge receipt, scans for (entity, attribute)
/// pairs with conflicting assertions under their resolution mode, and returns
/// the detected conflicts. This is the real implementation of cascade step 1;
/// steps 2-5 are stubs at Stage 0 (see `cascade_stub_datoms`).
///
/// # Invariants
///
/// - **INV-RESOLUTION-003**: Conservative conflict detection (no false negatives).
/// - **INV-RESOLUTION-004**: Six-condition conflict predicate.
/// - **INV-MERGE-009**: Step 1 of the five-step post-merge cascade.
/// - **INV-MERGE-010**: MergeReceipt captures conflict set.
pub fn cascade_step1_conflicts(store: &Store, _receipt: &MergeReceipt) -> Vec<ConflictSet> {
    detect_merge_conflicts(store)
}

/// Run the full post-merge cascade and return an aggregated receipt.
///
/// 1. Step 1 — conflict detection (real, via `cascade_step1_conflicts`)
/// 2. Steps 2-5 — stub datoms (via `cascade_stub_datoms`)
///
/// The returned `CascadeReceipt` combines the conflict set from step 1
/// with the stub datoms for steps 2-5. `steps_completed` is always 1 at
/// Stage 0 because only step 1 performs real work.
///
/// # Arguments
///
/// * `store` - The post-merge store to scan for conflicts
/// * `receipt` - The merge receipt from the just-completed merge
/// * `tx` - The transaction ID for the cascade datoms (provenance)
///
/// # Invariants
///
/// - **INV-MERGE-009**: All 5 cascade steps produce datoms.
/// - **INV-MERGE-010**: MergeReceipt captures new datom count and conflict set.
/// - **ADR-MERGE-005**: Cascade as post-merge deterministic layer.
/// - **ADR-MERGE-007**: Merge cascade stub datoms at Stage 0.
pub fn run_cascade(store: &Store, receipt: &MergeReceipt, tx: TxId) -> CascadeReceipt {
    // Step 1: real conflict detection
    let conflicts = cascade_step1_conflicts(store, receipt);
    let conflicts_detected = conflicts.len();

    // Steps 2-5: stub datoms (Stage 0 placeholders)
    let stub_datoms = cascade_stub_datoms(receipt, tx);

    CascadeReceipt {
        conflicts_detected,
        conflicts,
        stub_datoms,
        steps_completed: 1,
        schema_affected: false,
        conflicted_entities: Vec::new(),
        stale_live_views: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Schema-affecting attribute constants
// ---------------------------------------------------------------------------

/// Attributes whose presence in merged datoms indicates a schema structural change.
/// If any merged datom carries one of these attributes, the schema view
/// must be rebuilt (cascade step 2, INV-MERGE-009).
///
/// Note: `:db/ident` is intentionally excluded. It is used for entity naming
/// (including agent entities auto-created by `transact`), not just schema
/// definition. Only attributes that define type, cardinality, uniqueness, or
/// resolution semantics trigger a schema rebuild.
const SCHEMA_AFFECTING_ATTRS: &[&str] = &[
    ":db/valueType",
    ":db/cardinality",
    ":db/unique",
    ":db/resolutionMode",
];

/// Check whether a datom's attribute is schema-affecting.
fn is_schema_affecting(attr: &Attribute) -> bool {
    SCHEMA_AFFECTING_ATTRS.contains(&attr.as_str())
}

// ---------------------------------------------------------------------------
// cascade_full — Stage 1 real cascade (INV-MERGE-009)
// ---------------------------------------------------------------------------

/// Run the full post-merge cascade with real logic for all four steps.
///
/// Unlike `run_cascade` (Stage 0, step 1 real + stub datoms for steps 2-5),
/// `cascade_full` performs real work for all steps:
///
/// 1. **Conflict detection** — scan for (entity, attribute) pairs with conflicting
///    assertions under their resolution mode.
/// 2. **Schema rebuild detection** — check whether any merged datom carries a
///    schema-affecting attribute (`:db/valueType`, `:db/cardinality`, `:db/unique`,
///    `:db/resolutionMode`). If so, the schema view is stale and callers must
///    rebuild it.
/// 3. **Resolution recompute** — collect the deduplicated set of entity IDs that
///    appear in at least one conflict. These entities need resolution recomputation
///    at the query layer.
/// 4. **LIVE invalidation** — identify entities from `merged_datoms` that belong
///    to an Intent, Spec, or Impl LIVE projection (INV-TRILATERAL-001). These
///    entities' projections are stale and must be refreshed.
///
/// The function also generates stub datoms for the audit trail (ADR-MERGE-007),
/// preserving backward compatibility with the cascade provenance chain.
///
/// # Arguments
///
/// * `store` - The post-merge store (already contains datoms from both sides)
/// * `receipt` - The merge receipt from the just-completed set-union merge
/// * `merged_datoms` - The specific datoms introduced by the merge (the delta)
/// * `tx` - The transaction ID for cascade provenance
///
/// # Invariants
///
/// - **INV-MERGE-009**: Cascade: schema rebuild -> resolution recompute -> LIVE invalidation.
/// - **INV-MERGE-010**: MergeReceipt captures new datom count and conflict set.
/// - **ADR-MERGE-005**: Cascade as post-merge deterministic layer.
/// - **INV-TRILATERAL-001**: LIVE projections are monotone functions of the store.
pub fn cascade_full(
    store: &Store,
    receipt: &MergeReceipt,
    merged_datoms: &[Datom],
    tx: TxId,
) -> CascadeReceipt {
    // Step 1: Conflict detection (same as run_cascade)
    let conflicts = cascade_step1_conflicts(store, receipt);
    let conflicts_detected = conflicts.len();

    // Step 2: Schema rebuild detection — scan merged datoms for schema-affecting attributes
    let schema_affected = merged_datoms
        .iter()
        .any(|d| is_schema_affecting(&d.attribute));

    // Step 3: Resolution recompute — collect entities that have conflicts.
    // Deduplicate via BTreeSet for deterministic ordering (INV-MERGE-010).
    let conflicted_entity_set: BTreeSet<EntityId> = conflicts.iter().map(|c| c.entity).collect();
    let conflicted_entities: Vec<EntityId> = conflicted_entity_set.into_iter().collect();

    // Step 4: LIVE invalidation — find merged datoms whose entities belong to
    // an Intent/Spec/Impl LIVE projection. Only Assert datoms contribute to
    // LIVE views (matching trilateral::live_projections filtering).
    let stale_entity_set: BTreeSet<EntityId> = merged_datoms
        .iter()
        .filter(|d| d.op == Op::Assert)
        .filter(|d| {
            matches!(
                classify_attribute(&d.attribute),
                AttrNamespace::Intent | AttrNamespace::Spec | AttrNamespace::Impl
            )
        })
        .map(|d| d.entity)
        .collect();
    let stale_live_views: Vec<EntityId> = stale_entity_set.into_iter().collect();

    // Stub datoms for audit trail (backward compatibility with ADR-MERGE-007)
    let stub_datoms = cascade_stub_datoms(receipt, tx);

    CascadeReceipt {
        conflicts_detected,
        conflicts,
        stub_datoms,
        // All 4 cascade steps perform real work in Stage 1
        steps_completed: 4,
        schema_affected,
        conflicted_entities,
        stale_live_views,
    }
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
    // cascade_step1_conflicts tests
    // -------------------------------------------------------------------

    // Verifies: INV-MERGE-009 — Cascade step 1 detects conflicts
    // Verifies: INV-RESOLUTION-003 — Conservative Conflict Detection
    #[test]
    fn cascade_step1_detects_conflicts_after_merge() {
        let mut s1 = Store::genesis();
        let mut s2 = Store::genesis();

        let entity = EntityId::from_ident(":test/cascade-conflict");
        let a1 = AgentId::from_name("alice");
        let a2 = AgentId::from_name("bob");

        // Two agents assert different values for same entity+attribute
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
        let receipt = merge_stores(&mut merged, &s2);

        let conflicts = cascade_step1_conflicts(&merged, &receipt);
        let has_doc_conflict = conflicts
            .iter()
            .any(|c| c.entity == entity && c.attribute == Attribute::from_keyword(":db/doc"));
        assert!(
            has_doc_conflict,
            "cascade step 1 should detect conflicting :db/doc values"
        );
    }

    // Verifies: INV-MERGE-009 — Cascade step 1 returns empty for no-conflict merge
    #[test]
    fn cascade_step1_no_conflicts_for_disjoint_merge() {
        let mut s1 = Store::genesis();
        let mut s2 = Store::genesis();

        let a1 = AgentId::from_name("alice");
        let a2 = AgentId::from_name("bob");

        // Disjoint entities — no conflicts possible
        let tx1 = Transaction::new(a1, ProvenanceType::Observed, "a")
            .assert(
                EntityId::from_ident(":test/entity-a"),
                Attribute::from_keyword(":db/doc"),
                Value::String("from alice".into()),
            )
            .commit(&s1)
            .unwrap();
        s1.transact(tx1).unwrap();

        let tx2 = Transaction::new(a2, ProvenanceType::Observed, "b")
            .assert(
                EntityId::from_ident(":test/entity-b"),
                Attribute::from_keyword(":db/doc"),
                Value::String("from bob".into()),
            )
            .commit(&s2)
            .unwrap();
        s2.transact(tx2).unwrap();

        let mut merged = s1.clone_store();
        let receipt = merge_stores(&mut merged, &s2);

        let conflicts = cascade_step1_conflicts(&merged, &receipt);
        assert!(
            conflicts.is_empty(),
            "disjoint entities should produce no conflicts"
        );
    }

    // -------------------------------------------------------------------
    // run_cascade tests
    // -------------------------------------------------------------------

    // Verifies: INV-MERGE-009 — Full cascade produces conflicts + stub datoms
    // Verifies: ADR-MERGE-005 — Cascade as post-merge deterministic layer
    // Verifies: ADR-MERGE-007 — Merge cascade stub datoms at Stage 0
    #[test]
    fn run_cascade_with_conflicts() {
        let mut s1 = Store::genesis();
        let mut s2 = Store::genesis();

        let entity = EntityId::from_ident(":test/cascade-full");
        let a1 = AgentId::from_name("alice");
        let a2 = AgentId::from_name("bob");

        let tx1 = Transaction::new(a1, ProvenanceType::Observed, "alice's value")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("alice".into()),
            )
            .commit(&s1)
            .unwrap();
        s1.transact(tx1).unwrap();

        let tx2 = Transaction::new(a2, ProvenanceType::Observed, "bob's value")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("bob".into()),
            )
            .commit(&s2)
            .unwrap();
        s2.transact(tx2).unwrap();

        let mut merged = s1.clone_store();
        let receipt = merge_stores(&mut merged, &s2);
        let cascade_tx = TxId::new(300, 0, a1);

        let cascade = run_cascade(&merged, &receipt, cascade_tx);

        // Step 1 should detect at least one conflict
        assert!(
            cascade.conflicts_detected > 0,
            "run_cascade should detect conflicts after conflicting merge"
        );
        assert_eq!(
            cascade.conflicts_detected,
            cascade.conflicts.len(),
            "conflicts_detected must equal conflicts.len()"
        );

        // Steps 2-5 stubs should be present (7 datoms: 3 metadata + 4 step stubs)
        assert_eq!(
            cascade.stub_datoms.len(),
            7,
            "expected 7 cascade stub datoms"
        );

        // At Stage 0, only step 1 is real
        assert_eq!(
            cascade.steps_completed, 1,
            "Stage 0: steps_completed must be 1"
        );
    }

    // Verifies: INV-MERGE-009 — Cascade with no conflicts
    #[test]
    fn run_cascade_without_conflicts() {
        let mut s1 = Store::genesis();
        let s2 = Store::genesis();

        let receipt = merge_stores(&mut s1, &s2);
        let agent = AgentId::from_name("test-agent");
        let cascade_tx = TxId::new(400, 0, agent);

        let cascade = run_cascade(&s1, &receipt, cascade_tx);

        assert_eq!(
            cascade.conflicts_detected, 0,
            "identical stores should produce no conflicts"
        );
        assert!(
            cascade.conflicts.is_empty(),
            "conflicts vec should be empty"
        );
        assert_eq!(cascade.stub_datoms.len(), 7, "stubs always produced");
        assert_eq!(cascade.steps_completed, 1);
    }

    // -------------------------------------------------------------------
    // merge_with_cascade integration tests
    // -------------------------------------------------------------------

    // Verifies: INV-MERGE-009 — merge_with_cascade runs cascade automatically
    // Verifies: ADR-MERGE-007 — Cascade stub datoms transacted into store
    // Verifies: NEG-MERGE-002 — No merge without cascade
    #[test]
    fn merge_with_cascade_conflicting_stores() {
        let mut s1 = Store::genesis();
        let mut s2 = Store::genesis();

        let entity = EntityId::from_ident(":test/cascade-wire");
        let a1 = AgentId::from_name("alice");
        let a2 = AgentId::from_name("bob");

        // Two agents assert different values for same entity+attribute
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

        let pre_datom_count = s1.len();
        let receipt = s1.merge_with_cascade(&s2, a1);

        // MergeReceipt: new datoms from s2
        assert!(
            receipt.merge.new_datoms > 0,
            "merge should introduce new datoms from s2"
        );

        // CascadeReceipt: conflicts detected
        assert!(
            receipt.cascade.conflicts_detected > 0,
            "cascade should detect conflicts on :db/doc"
        );

        // CascadeReceipt: exactly 7 stub datoms
        assert_eq!(
            receipt.cascade.stub_datoms.len(),
            7,
            "ADR-MERGE-007: cascade must produce exactly 7 stub datoms"
        );

        // CascadeReceipt: steps_completed == 1 at Stage 0
        assert_eq!(
            receipt.cascade.steps_completed, 1,
            "Stage 0: only step 1 is real"
        );

        // Stub datoms are now IN the store
        let post_datom_count = s1.len();
        assert!(
            post_datom_count >= pre_datom_count + receipt.merge.new_datoms + 7,
            "store should contain merge datoms + 7 cascade stubs: \
             pre={pre_datom_count}, post={post_datom_count}, \
             new_merge={}, stubs=7",
            receipt.merge.new_datoms,
        );

        // Verify cascade stubs are queryable in the store
        let cascade_status: Vec<_> = s1
            .datoms()
            .filter(|d| d.attribute == Attribute::from_keyword(":merge/cascade-status"))
            .collect();
        assert!(
            !cascade_status.is_empty(),
            "store must contain :merge/cascade-status datom after merge_with_cascade"
        );
        assert_eq!(
            cascade_status[0].value,
            Value::Keyword(":stub".into()),
            "cascade-status should be :stub at Stage 0"
        );
    }

    // Verifies: INV-MERGE-009 — cascade stubs generated even with 0 conflicts
    // Verifies: ADR-MERGE-007 — stubs always produced
    #[test]
    fn merge_with_cascade_disjoint_stores() {
        let mut s1 = Store::genesis();
        let mut s2 = Store::genesis();

        let a1 = AgentId::from_name("alice");
        let a2 = AgentId::from_name("bob");

        // Disjoint entities — no conflicts
        let tx1 = Transaction::new(a1, ProvenanceType::Observed, "a")
            .assert(
                EntityId::from_ident(":test/disjoint-a"),
                Attribute::from_keyword(":db/doc"),
                Value::String("alice".into()),
            )
            .commit(&s1)
            .unwrap();
        s1.transact(tx1).unwrap();

        let tx2 = Transaction::new(a2, ProvenanceType::Observed, "b")
            .assert(
                EntityId::from_ident(":test/disjoint-b"),
                Attribute::from_keyword(":db/doc"),
                Value::String("bob".into()),
            )
            .commit(&s2)
            .unwrap();
        s2.transact(tx2).unwrap();

        let receipt = s1.merge_with_cascade(&s2, a1);

        assert!(receipt.merge.new_datoms > 0, "disjoint merge adds datoms");
        assert_eq!(
            receipt.cascade.conflicts_detected, 0,
            "disjoint entities produce no conflicts"
        );
        assert_eq!(
            receipt.cascade.stub_datoms.len(),
            7,
            "stubs always produced even with 0 conflicts"
        );

        // Stubs are in the store
        let triggered: Vec<_> = s1
            .datoms()
            .filter(|d| d.attribute == Attribute::from_keyword(":merge/cascade-triggered"))
            .collect();
        assert!(
            !triggered.is_empty(),
            "cascade-triggered datom must be in store"
        );
    }

    // Verifies: Edge case — merging identical stores
    // 0 new datoms, 0 conflicts, but stubs still generated and injected
    #[test]
    fn merge_with_cascade_identical_stores() {
        let s1 = Store::genesis();
        let s2 = Store::genesis();

        let agent = AgentId::from_name("test-agent");
        let pre_count = s1.len();

        let mut target = s1.clone_store();
        let receipt = target.merge_with_cascade(&s2, agent);

        assert_eq!(
            receipt.merge.new_datoms, 0,
            "identical stores should produce 0 new datoms"
        );
        assert_eq!(
            receipt.cascade.conflicts_detected, 0,
            "identical stores should produce 0 conflicts"
        );
        assert_eq!(
            receipt.cascade.stub_datoms.len(),
            7,
            "stubs always produced even for identical stores"
        );

        // Store grew by exactly 7 (the cascade stubs)
        assert_eq!(
            target.len(),
            pre_count + 7,
            "store should grow by exactly 7 cascade stub datoms"
        );
    }

    // Verifies: cascade stubs have correct provenance (cascade_agent)
    #[test]
    fn merge_with_cascade_stubs_have_correct_agent() {
        let mut s1 = Store::genesis();
        let s2 = Store::genesis();

        let merge_agent = AgentId::from_name("merge-operator");
        let receipt = s1.merge_with_cascade(&s2, merge_agent);

        // All stub datoms should reference the cascade agent
        for stub in &receipt.cascade.stub_datoms {
            assert_eq!(
                stub.tx.agent, merge_agent,
                "cascade stub datom TxId must reference the cascade_agent"
            );
        }
    }

    // Verifies: cascade stubs are queryable via attribute index
    #[test]
    fn merge_with_cascade_stubs_indexed() {
        let mut s1 = Store::genesis();
        let s2 = Store::genesis();

        let agent = AgentId::from_name("indexer");
        s1.merge_with_cascade(&s2, agent);

        // Query all cascade-related attributes
        let cascade_attrs = [
            ":merge/cascade-status",
            ":merge/cascade-triggered",
            ":merge/duplicate-count",
            ":cascade/cache-invalidation",
            ":cascade/secondary-conflicts",
            ":cascade/uncertainty-delta",
            ":cascade/projection-staleness",
        ];

        for attr_str in &cascade_attrs {
            let attr = Attribute::from_keyword(attr_str);
            let found = s1.datoms().any(|d| d.attribute == attr);
            assert!(
                found,
                "cascade attribute {attr_str} must be queryable in store after merge_with_cascade"
            );
        }
    }

    // -------------------------------------------------------------------
    // cascade_full tests (INV-MERGE-009 Stage 1)
    // -------------------------------------------------------------------

    // Verifies: INV-MERGE-009 — cascade_full with no conflicts, no schema, no LIVE
    // All fields should be empty/false, steps_completed = 4.
    #[test]
    fn cascade_full_no_conflicts_empty_receipt() {
        let mut s1 = Store::genesis();
        let s2 = Store::genesis();

        let receipt = merge_stores(&mut s1, &s2);
        let agent = AgentId::from_name("test-agent");
        let cascade_tx = TxId::new(500, 0, agent);

        // No new datoms from merging identical stores
        let merged_datoms: Vec<Datom> = Vec::new();
        let cascade = cascade_full(&s1, &receipt, &merged_datoms, cascade_tx);

        assert_eq!(
            cascade.conflicts_detected, 0,
            "identical stores → no conflicts"
        );
        assert!(
            !cascade.schema_affected,
            "no schema datoms → schema_affected = false"
        );
        assert!(
            cascade.conflicted_entities.is_empty(),
            "no conflicts → no conflicted entities"
        );
        assert!(
            cascade.stale_live_views.is_empty(),
            "no LIVE-layer datoms → no stale views"
        );
        assert_eq!(cascade.steps_completed, 4, "Stage 1: all 4 steps ran");
        assert_eq!(
            cascade.stub_datoms.len(),
            7,
            "audit trail stubs still produced"
        );
    }

    // Verifies: INV-MERGE-009 — cascade_full detects schema-affecting datoms
    // When merged_datoms contain :db/valueType, schema_affected must be true.
    #[test]
    fn cascade_full_schema_datom_detected() {
        let mut s1 = Store::genesis();
        let mut s2 = Store::genesis();

        let a1 = AgentId::from_name("alice");
        let a2 = AgentId::from_name("bob");

        // Agent Bob defines a new attribute with :db/valueType in s2
        let new_attr_entity = EntityId::from_ident(":test/custom-attr");
        let tx2 = Transaction::new(a2, ProvenanceType::Observed, "define custom attr")
            .assert(
                new_attr_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/custom-attr".into()),
            )
            .assert(
                new_attr_entity,
                Attribute::from_keyword(":db/valueType"),
                Value::Keyword(":db.type/string".into()),
            )
            .assert(
                new_attr_entity,
                Attribute::from_keyword(":db/cardinality"),
                Value::Keyword(":db.cardinality/one".into()),
            )
            .commit(&s2)
            .unwrap();
        s2.transact(tx2).unwrap();

        // Collect the datoms that will be new to s1 when merging s2
        let pre_datoms: BTreeSet<Datom> = s1.datom_set().clone();
        let receipt = merge_stores(&mut s1, &s2);
        let merged_datoms: Vec<Datom> = s1
            .datoms()
            .filter(|d| !pre_datoms.contains(d))
            .cloned()
            .collect();

        let cascade_tx = TxId::new(600, 0, a1);
        let cascade = cascade_full(&s1, &receipt, &merged_datoms, cascade_tx);

        assert!(
            cascade.schema_affected,
            "merged datoms with :db/valueType → schema_affected = true"
        );
        assert_eq!(cascade.steps_completed, 4, "Stage 1: all 4 steps ran");
    }

    // Verifies: INV-MERGE-009 — cascade_full collects conflicted entity IDs
    // When two agents assert different values for the same entity+attribute,
    // that entity must appear in conflicted_entities.
    #[test]
    fn cascade_full_conflicting_entities_collected() {
        let mut s1 = Store::genesis();
        let mut s2 = Store::genesis();

        let entity = EntityId::from_ident(":test/cascade-full-conflict");
        let a1 = AgentId::from_name("alice");
        let a2 = AgentId::from_name("bob");

        let tx1 = Transaction::new(a1, ProvenanceType::Observed, "alice's value")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("alice version".into()),
            )
            .commit(&s1)
            .unwrap();
        s1.transact(tx1).unwrap();

        let tx2 = Transaction::new(a2, ProvenanceType::Observed, "bob's value")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("bob version".into()),
            )
            .commit(&s2)
            .unwrap();
        s2.transact(tx2).unwrap();

        let pre_datoms: BTreeSet<Datom> = s1.datom_set().clone();
        let receipt = merge_stores(&mut s1, &s2);
        let merged_datoms: Vec<Datom> = s1
            .datoms()
            .filter(|d| !pre_datoms.contains(d))
            .cloned()
            .collect();

        let cascade_tx = TxId::new(700, 0, a1);
        let cascade = cascade_full(&s1, &receipt, &merged_datoms, cascade_tx);

        assert!(
            cascade.conflicts_detected > 0,
            "conflicting merge → conflicts detected"
        );
        assert!(
            cascade.conflicted_entities.contains(&entity),
            "conflicted entity must appear in conflicted_entities"
        );
        assert_eq!(cascade.steps_completed, 4);
    }

    // Verifies: INV-MERGE-009 + INV-TRILATERAL-001 — LIVE invalidation
    // When merged datoms carry spec-layer attributes, the affected entities
    // must appear in stale_live_views.
    #[test]
    fn cascade_full_live_invalidation_spec_entities() {
        use crate::schema::{full_schema_datoms, genesis_datoms};

        // Build stores with full schema so :spec/* attributes are known
        let system_agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, system_agent);
        let mut datom_set = BTreeSet::new();
        for d in genesis_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        for d in full_schema_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        let mut s1 = Store::from_datoms(datom_set.clone());
        let mut s2 = Store::from_datoms(datom_set);

        let spec_entity = EntityId::from_ident(":test/spec-element");
        let a1 = AgentId::from_name("alice");
        let a2 = AgentId::from_name("bob");

        // Bob creates a spec-layer datom in s2
        let tx2 = Transaction::new(a2, ProvenanceType::Observed, "spec element")
            .assert(
                spec_entity,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-TEST-001".into()),
            )
            .assert(
                spec_entity,
                Attribute::from_keyword(":spec/statement"),
                Value::String("Test invariant".into()),
            )
            .commit(&s2)
            .unwrap();
        s2.transact(tx2).unwrap();

        let pre_datoms: BTreeSet<Datom> = s1.datom_set().clone();
        let receipt = merge_stores(&mut s1, &s2);
        let merged_datoms: Vec<Datom> = s1
            .datoms()
            .filter(|d| !pre_datoms.contains(d))
            .cloned()
            .collect();

        let cascade_tx = TxId::new(800, 0, a1);
        let cascade = cascade_full(&s1, &receipt, &merged_datoms, cascade_tx);

        assert!(
            cascade.stale_live_views.contains(&spec_entity),
            "spec entity from merged datoms must appear in stale_live_views"
        );
        // No conflicts (disjoint entities)
        assert!(cascade.conflicted_entities.is_empty());
        // No schema change (spec attributes are domain attrs, not schema-affecting)
        assert!(!cascade.schema_affected);
        assert_eq!(cascade.steps_completed, 4);
    }

    // Verifies: INV-MERGE-009 — cascade_full with all effects combined
    // Schema change + conflicts + LIVE invalidation all at once.
    #[test]
    fn cascade_full_all_effects_combined() {
        use crate::schema::{full_schema_datoms, genesis_datoms};

        // Build stores with full schema so :spec/* attributes are known
        let system_agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, system_agent);
        let mut datom_set = BTreeSet::new();
        for d in genesis_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        for d in full_schema_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        let mut s1 = Store::from_datoms(datom_set.clone());
        let mut s2 = Store::from_datoms(datom_set);

        let conflict_entity = EntityId::from_ident(":test/combined-conflict");
        let spec_entity = EntityId::from_ident(":test/combined-spec");
        let schema_entity = EntityId::from_ident(":test/combined-schema-attr");
        let a1 = AgentId::from_name("alice");
        let a2 = AgentId::from_name("bob");

        // Alice: assert doc on conflict entity
        let tx1 = Transaction::new(a1, ProvenanceType::Observed, "alice's data")
            .assert(
                conflict_entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("alice".into()),
            )
            .commit(&s1)
            .unwrap();
        s1.transact(tx1).unwrap();

        // Bob: conflicting doc + spec datom + schema datom
        let tx2 = Transaction::new(a2, ProvenanceType::Observed, "bob's data")
            .assert(
                conflict_entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("bob".into()),
            )
            .assert(
                spec_entity,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-COMBINED-001".into()),
            )
            .assert(
                schema_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":test/combined-schema-attr".into()),
            )
            .assert(
                schema_entity,
                Attribute::from_keyword(":db/valueType"),
                Value::Keyword(":db.type/long".into()),
            )
            .commit(&s2)
            .unwrap();
        s2.transact(tx2).unwrap();

        let pre_datoms: BTreeSet<Datom> = s1.datom_set().clone();
        let receipt = merge_stores(&mut s1, &s2);
        let merged_datoms: Vec<Datom> = s1
            .datoms()
            .filter(|d| !pre_datoms.contains(d))
            .cloned()
            .collect();

        let cascade_tx = TxId::new(900, 0, a1);
        let cascade = cascade_full(&s1, &receipt, &merged_datoms, cascade_tx);

        // All three effects present
        assert!(
            cascade.schema_affected,
            "schema datom merged → schema_affected"
        );
        assert!(
            cascade.conflicted_entities.contains(&conflict_entity),
            "conflicting entity must be listed"
        );
        assert!(
            cascade.stale_live_views.contains(&spec_entity),
            "spec entity must be in stale_live_views"
        );
        assert_eq!(cascade.steps_completed, 4);
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

            // Verifies: ADR-MERGE-007 — cascade stubs always exactly 7 datoms
            // Verifies: INV-MERGE-010 — cascade determinism (same inputs → same outputs)
            #[test]
            fn cascade_stubs_always_seven_datoms((s1, s2) in arb_store_pair(2)) {
                let mut target = s1.clone_store();
                let agent = AgentId::from_name("prop-agent");
                let receipt = target.merge_with_cascade(&s2, agent);

                prop_assert_eq!(
                    receipt.cascade.stub_datoms.len(),
                    7,
                    "ADR-MERGE-007: cascade must always produce exactly 7 stub datoms"
                );
            }

            // Verifies: INV-MERGE-010 — cascade determinism
            // Same inputs produce same cascade stub datoms
            #[test]
            fn cascade_stubs_deterministic((s1, s2) in arb_store_pair(2)) {
                let agent = AgentId::from_name("det-agent");

                let mut target_a = s1.clone_store();
                let receipt_a = target_a.merge_with_cascade(&s2, agent);

                let mut target_b = s1.clone_store();
                let receipt_b = target_b.merge_with_cascade(&s2, agent);

                prop_assert_eq!(
                    receipt_a.cascade.stub_datoms,
                    receipt_b.cascade.stub_datoms,
                    "INV-MERGE-010: cascade stubs must be deterministic"
                );
            }
        }
    }
}
