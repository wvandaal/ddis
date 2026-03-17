//! Per-agent private store isolation via a two-layer AgentStore.
//!
//! `AgentStore` implements the W_alpha working set from the DDIS protocol:
//! each agent gets a private `Store` instance (the working set) layered on
//! top of a shared `Store` (the trunk). The working set uses the same datom
//! structure and operations as the shared store — it IS a Store.
//!
//! # Formal Model
//!
//! ```text
//! AgentStore_α = (S_shared, W_α, α)
//!
//! query_local(AgentStore_α) = merge(S_shared, W_α)
//! assert_local(AgentStore_α, datoms) → W_α' = W_α ∪ datoms
//! commit(AgentStore_α, entities) → S_shared' = S_shared ∪ select(W_α, entities)
//!   where coherence_check(S_shared, select(W_α, entities)) = Ok
//! ```
//!
//! # Invariants
//!
//! - **INV-STORE-013**: Working set isolation — `query(W_α) ∩ visible(β) = ∅`.
//!   Other agents do NOT see this agent's working set. The working set is
//!   private to the agent that owns it.
//!
//! - **INV-MERGE-003**: Branch isolation — working sets not leaked into merge.
//!   Only committed datoms (promoted through `commit()`) enter the shared store.
//!
//! - **INV-TRANSACT-COHERENCE-001**: Commit-time coherence — commit() runs
//!   coherence_check() before promoting datoms to shared. Violations are
//!   rejected, leaving the working set unchanged.
//!
//! # Design Decisions
//!
//! - ADR-STORE-022: W_alpha IS a Store (same type, same operations). The
//!   compositor pattern wraps two Store instances rather than modifying Store.
//!
//! - ADR-MERGE-002: Branching G-Set extension for working set isolation.
//!
//! # Negative Cases
//!
//! - NEG-MERGE-003: No working set leak — uncommitted datoms excluded from
//!   shared store operations. Only `commit()` promotes datoms.
//!
//! # Traces To
//!
//! - SEED.md §4 (Design Commitment #2: append-only)
//! - spec/01-store.md (INV-STORE-013)
//! - spec/05-merge.md (INV-MERGE-003, ADR-MERGE-002)
//! - docs/history/transcripts/04-datom-protocol-interface-design.md (PQ1: private datoms/W_α)

use std::collections::BTreeSet;

use crate::coherence::{coherence_check, CoherenceViolation};
use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, Value};
use crate::merge::merge_stores;
use crate::store::{Store, Transaction, TxReceipt};

// ===========================================================================
// CommitError
// ===========================================================================

/// Error type for AgentStore commit operations.
///
/// The `Coherence` variant is boxed because `CoherenceViolation` is large (>200 bytes).
/// This keeps `CommitError` small enough to pass clippy's `result_large_err` check.
#[derive(Clone, Debug)]
pub enum CommitError {
    /// Coherence check failed — the datoms to be promoted contradict the shared store.
    Coherence(Box<CoherenceViolation>),
    /// No datoms found for the specified entity IDs in the working set.
    EmptyCommit,
    /// Transaction failed during promotion to shared store.
    TransactFailed(String),
}

impl std::fmt::Display for CommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommitError::Coherence(v) => write!(f, "coherence violation during commit: {v}"),
            CommitError::EmptyCommit => {
                write!(f, "no datoms in working set match the specified entity IDs")
            }
            CommitError::TransactFailed(msg) => {
                write!(f, "transaction failed during commit: {msg}")
            }
        }
    }
}

impl std::error::Error for CommitError {}

// ===========================================================================
// AgentStore
// ===========================================================================

/// Per-agent private store with two-layer isolation.
///
/// Wraps a shared `Store` (trunk) and a private `Store` (working set).
/// The working set holds uncommitted datoms visible only to this agent.
/// `commit()` promotes selected entities through a coherence gate into
/// the shared store.
///
/// # Three-Box Decomposition
///
/// **Black box**: Private workspace with commit-to-shared promotion.
/// **State box**: `Store` (shared) + `Store` (working) + `AgentId`.
/// **Clear box**: See implementation below.
pub struct AgentStore {
    /// The committed shared store (trunk). All agents see this.
    shared: Store,
    /// Private working set — same type as shared. Only this agent sees it.
    working: Store,
    /// The agent that owns this working set.
    agent_id: AgentId,
}

impl AgentStore {
    /// Create a new AgentStore with a shared store and agent identity.
    ///
    /// The working set starts as a genesis store (containing only the
    /// axiomatic schema). User datoms are added via `assert_local()`.
    pub fn new(shared: Store, agent_id: AgentId) -> Self {
        AgentStore {
            shared,
            working: Store::genesis(),
            agent_id,
        }
    }

    /// Return a merged view of shared + working for local queries.
    ///
    /// The returned store contains all datoms from both the shared store
    /// and this agent's working set. This is an ephemeral read-only view
    /// — mutations should go through `assert_local()` or `commit()`.
    ///
    /// Uses `merge_stores()` (CRDT set union) to combine the two stores.
    /// INV-MERGE-001: merge = set union of datom sets.
    pub fn query_local(&self) -> Store {
        let mut merged = self.shared.clone_store();
        merge_stores(&mut merged, &self.working);
        merged
    }

    /// Add datoms to the working set only (private, invisible to other agents).
    ///
    /// Returns the number of datoms actually inserted (deduplication may
    /// reduce this below the input count).
    ///
    /// The datoms are added via a proper transaction on the working store,
    /// preserving all Store invariants (schema validation, tx metadata, etc.).
    ///
    /// INV-STORE-013: These datoms are invisible to other agents until committed.
    /// NEG-MERGE-003: No working set leak — datoms stay in `self.working`.
    pub fn assert_local(
        &mut self,
        datoms: Vec<(EntityId, Attribute, Value)>,
        provenance: ProvenanceType,
        rationale: &str,
    ) -> Result<TxReceipt, crate::error::StoreError> {
        let mut tx = Transaction::new(self.agent_id, provenance, rationale);
        for (entity, attribute, value) in datoms {
            tx = tx.assert(entity, attribute, value);
        }
        let committed = tx.commit(&self.working)?;
        self.working.transact(committed)
    }

    /// Promote selected entities from working set to shared store.
    ///
    /// This is the coherence gate: datoms matching the specified entity IDs
    /// are extracted from the working set, checked against the shared store
    /// for contradictions, and — if coherent — transacted into shared.
    ///
    /// On success, the promoted datoms remain in the working set (append-only,
    /// INV-STORE-001) but are now also present in shared. This is harmless
    /// because Store deduplicates by content identity (INV-STORE-003).
    ///
    /// # Errors
    ///
    /// - `CommitError::EmptyCommit` if no datoms match the entity IDs.
    /// - `CommitError::Coherence` if the datoms violate shared store coherence.
    /// - `CommitError::TransactFailed` if the underlying transact fails.
    ///
    /// INV-TRANSACT-COHERENCE-001: coherence_check runs before promotion.
    /// INV-MERGE-003: Only explicitly committed datoms enter shared.
    pub fn commit(&mut self, entity_ids: &[EntityId]) -> Result<TxReceipt, CommitError> {
        // Extract datoms from working set that match the entity IDs.
        // Skip genesis/schema datoms (tx metadata) — only promote user assertions.
        let entity_set: BTreeSet<EntityId> = entity_ids.iter().copied().collect();
        let candidate_datoms: Vec<&Datom> = self
            .working
            .datoms()
            .filter(|d| entity_set.contains(&d.entity))
            .collect();

        if candidate_datoms.is_empty() {
            return Err(CommitError::EmptyCommit);
        }

        // Collect the raw datom data for coherence check and transact.
        let datom_triples: Vec<(EntityId, Attribute, Value, Op)> = candidate_datoms
            .iter()
            .map(|d| (d.entity, d.attribute.clone(), d.value.clone(), d.op))
            .collect();

        // Build new datoms stamped with a fresh TxId from the shared store's clock.
        // We need to create a proper Transaction for the shared store.
        let mut tx = Transaction::new(self.agent_id, ProvenanceType::Observed, "W_alpha commit");
        for (entity, attribute, value, op) in &datom_triples {
            match op {
                Op::Assert => {
                    tx = tx.assert(*entity, attribute.clone(), value.clone());
                }
                Op::Retract => {
                    tx = tx.retract(*entity, attribute.clone(), value.clone());
                }
            }
        }

        let committed_tx = tx
            .commit(&self.shared)
            .map_err(|e| CommitError::TransactFailed(e.to_string()))?;

        // Run coherence check against shared store BEFORE applying.
        coherence_check(&self.shared, committed_tx.datoms())
            .map_err(|v| CommitError::Coherence(Box::new(v)))?;

        // Coherence passed — apply to shared store.
        let receipt = self
            .shared
            .transact(committed_tx)
            .map_err(|e| CommitError::TransactFailed(e.to_string()))?;

        Ok(receipt)
    }

    /// Read-only access to the shared store (trunk).
    pub fn shared(&self) -> &Store {
        &self.shared
    }

    /// Read-only access to the private working set.
    pub fn working(&self) -> &Store {
        &self.working
    }

    /// The agent identity that owns this working set.
    pub fn agent_id(&self) -> AgentId {
        self.agent_id
    }

    /// Reconstruct an AgentStore from persisted datom sets after a crash.
    ///
    /// When an agent crashes, its working set may have been persisted to disk
    /// (using the same layout format as the trunk). This function rebuilds
    /// the AgentStore from the shared store and the recovered working datoms.
    ///
    /// The working set is reconstructed via `Store::from_datoms()`, which
    /// rebuilds the schema, frontier, and indexes from the raw datom set.
    ///
    /// # Invariants
    ///
    /// - ADR-STORE-009: Crash-recovery model (replay from durable datoms).
    /// - INV-STORE-013: Working set isolation preserved after recovery.
    /// - INV-STORE-016: Frontier computability -- frontier derived from datom set alone.
    ///
    /// # Traces To
    ///
    /// - SEED.md section 4 (Design Commitment #2: append-only)
    /// - spec/01-store.md (INV-STORE-009, INV-STORE-013)
    /// - docs/history/transcripts/04-datom-protocol-interface-design.md (PQ3: crash-recovery)
    pub fn recover(shared: Store, working_datoms: BTreeSet<Datom>, agent_id: AgentId) -> Self {
        let working = Store::from_datoms(working_datoms);
        AgentStore {
            shared,
            working,
            agent_id,
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

// Witnesses: INV-STORE-013, INV-MERGE-003, INV-TRANSACT-COHERENCE-001,
// ADR-STORE-022, ADR-MERGE-002, NEG-MERGE-003
#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{AgentId, Attribute, EntityId, ProvenanceType, TxId, Value};
    use crate::schema;
    use crate::store::Store;
    use std::collections::BTreeSet;

    fn alice() -> AgentId {
        AgentId::from_name("alice")
    }

    fn bob() -> AgentId {
        AgentId::from_name("bob")
    }

    /// Helper: build a shared store with full schema (Layers 0-3).
    fn full_schema_store() -> Store {
        let system_agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, system_agent);
        let mut datom_set: BTreeSet<Datom> = BTreeSet::new();
        for d in schema::genesis_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        for d in schema::full_schema_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        Store::from_datoms(datom_set)
    }

    // --- INV-STORE-013: Working set invisible to other agents ---

    /// Verifies: INV-STORE-013 — Working set isolation.
    /// Datoms in alice's working set are NOT visible to bob's agent store.
    #[test]
    fn working_set_invisible_to_other_agents() {
        let shared = full_schema_store();
        let mut alice_store = AgentStore::new(shared.clone_store(), alice());
        let bob_store = AgentStore::new(shared, bob());

        let entity = EntityId::from_ident(":test/alice-private");
        alice_store
            .assert_local(
                vec![(
                    entity,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("alice's secret".into()),
                )],
                ProvenanceType::Observed,
                "private work",
            )
            .unwrap();

        // Alice can see her own working set data
        let alice_view = alice_store.query_local();
        let alice_has_it = alice_view
            .datoms()
            .any(|d| d.entity == entity && d.attribute.as_str() == ":db/doc");
        assert!(alice_has_it, "Alice should see her own working set data");

        // Bob cannot see alice's working set data — he only sees the shared store.
        let bob_view = bob_store.query_local();
        let bob_has_it = bob_view
            .datoms()
            .any(|d| d.entity == entity && d.attribute.as_str() == ":db/doc");
        assert!(
            !bob_has_it,
            "INV-STORE-013: Bob must NOT see Alice's working set"
        );

        // Also verify: shared store itself has no trace of alice's working data.
        let shared_has_it = alice_store
            .shared()
            .datoms()
            .any(|d| d.entity == entity && d.attribute.as_str() == ":db/doc");
        assert!(
            !shared_has_it,
            "INV-STORE-013: Shared store must not contain working set datoms"
        );
    }

    // --- commit() promotes datoms to shared ---

    /// Verifies: INV-MERGE-003 — Only committed datoms enter shared.
    /// After commit(), the entity's datoms appear in the shared store.
    #[test]
    fn commit_promotes_datoms_to_shared() {
        let shared = full_schema_store();
        let mut agent = AgentStore::new(shared, alice());

        let entity = EntityId::from_ident(":test/promote-me");
        agent
            .assert_local(
                vec![(
                    entity,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("to be promoted".into()),
                )],
                ProvenanceType::Observed,
                "prepare for commit",
            )
            .unwrap();

        // Before commit: not in shared
        let pre_shared = agent
            .shared()
            .datoms()
            .any(|d| d.entity == entity && d.attribute.as_str() == ":db/doc");
        assert!(!pre_shared, "Before commit: entity not in shared");

        // Commit
        let receipt = agent.commit(&[entity]).unwrap();
        assert!(receipt.datom_count > 0, "Commit should produce datoms");

        // After commit: in shared
        let post_shared = agent.shared().datoms().any(|d| {
            d.entity == entity
                && d.attribute.as_str() == ":db/doc"
                && d.value == Value::String("to be promoted".into())
        });
        assert!(post_shared, "After commit: entity should be in shared");
    }

    // --- Local query sees shared + working ---

    /// Verifies: query_local() returns merged(shared, working).
    /// Data from both shared and working are visible in the local view.
    #[test]
    fn local_query_sees_shared_and_working() {
        let mut shared = full_schema_store();
        let shared_entity = EntityId::from_ident(":test/shared-data");
        let agent = alice();

        // Add something to shared directly
        let tx = Transaction::new(agent, ProvenanceType::Observed, "shared data")
            .assert(
                shared_entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("in shared".into()),
            )
            .commit(&shared)
            .unwrap();
        shared.transact(tx).unwrap();

        // Create agent store and add something to working
        let mut agent_store = AgentStore::new(shared, agent);
        let working_entity = EntityId::from_ident(":test/working-data");
        agent_store
            .assert_local(
                vec![(
                    working_entity,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("in working".into()),
                )],
                ProvenanceType::Observed,
                "working data",
            )
            .unwrap();

        let local_view = agent_store.query_local();

        // Shared data visible
        let has_shared = local_view
            .datoms()
            .any(|d| d.entity == shared_entity && d.value == Value::String("in shared".into()));
        assert!(has_shared, "query_local should see shared data");

        // Working data visible
        let has_working = local_view
            .datoms()
            .any(|d| d.entity == working_entity && d.value == Value::String("in working".into()));
        assert!(has_working, "query_local should see working set data");
    }

    // --- Working sets independent between agents ---

    /// Verifies: Two agents' working sets are independent.
    /// Asserting in alice's working set does not affect bob's.
    #[test]
    fn working_sets_independent_between_agents() {
        let shared = full_schema_store();
        let mut alice_store = AgentStore::new(shared.clone_store(), alice());
        let mut bob_store = AgentStore::new(shared, bob());

        let alice_entity = EntityId::from_ident(":test/alice-work");
        let bob_entity = EntityId::from_ident(":test/bob-work");

        alice_store
            .assert_local(
                vec![(
                    alice_entity,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("alice work".into()),
                )],
                ProvenanceType::Observed,
                "alice's task",
            )
            .unwrap();

        bob_store
            .assert_local(
                vec![(
                    bob_entity,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("bob work".into()),
                )],
                ProvenanceType::Observed,
                "bob's task",
            )
            .unwrap();

        // Alice sees only her own working data (plus shared)
        let alice_view = alice_store.query_local();
        let alice_sees_bob = alice_view.datoms().any(|d| d.entity == bob_entity);
        assert!(!alice_sees_bob, "Alice must not see Bob's working set data");

        // Bob sees only his own working data (plus shared)
        let bob_view = bob_store.query_local();
        let bob_sees_alice = bob_view.datoms().any(|d| d.entity == alice_entity);
        assert!(!bob_sees_alice, "Bob must not see Alice's working set data");

        // Each sees their own
        let alice_sees_own = alice_view.datoms().any(|d| d.entity == alice_entity);
        let bob_sees_own = bob_view.datoms().any(|d| d.entity == bob_entity);
        assert!(alice_sees_own, "Alice should see her own working data");
        assert!(bob_sees_own, "Bob should see his own working data");
    }

    // --- Empty commit error ---

    /// Verifies: commit with non-existent entity IDs returns EmptyCommit error.
    #[test]
    fn commit_empty_returns_error() {
        let shared = full_schema_store();
        let mut agent = AgentStore::new(shared, alice());

        let nonexistent = EntityId::from_ident(":test/does-not-exist");
        let result = agent.commit(&[nonexistent]);
        assert!(
            matches!(result, Err(CommitError::EmptyCommit)),
            "Committing non-existent entity should return EmptyCommit"
        );
    }

    // --- Commit through coherence gate ---

    /// Verifies: INV-TRANSACT-COHERENCE-001 — commit() rejects incoherent promotions.
    /// If the working set has a datom that contradicts the shared store,
    /// commit() must fail with a CoherenceViolation.
    #[test]
    fn commit_rejects_incoherent_datoms() {
        let mut shared = full_schema_store();
        let entity = EntityId::from_ident(":test/conflict-entity");
        let agent = alice();

        // Put a value in shared store first
        let tx = Transaction::new(agent, ProvenanceType::Observed, "initial value")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("original".into()),
            )
            .commit(&shared)
            .unwrap();
        shared.transact(tx).unwrap();

        // Create agent store with the shared store containing "original"
        let mut agent_store = AgentStore::new(shared, agent);

        // Put a conflicting value in the working set
        agent_store
            .assert_local(
                vec![(
                    entity,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("conflicting".into()),
                )],
                ProvenanceType::Observed,
                "conflicting assertion",
            )
            .unwrap();

        // Commit should fail with coherence violation
        let result = agent_store.commit(&[entity]);
        assert!(
            matches!(&result, Err(CommitError::Coherence(_))),
            "Commit must reject incoherent datoms: got {:?}",
            result
        );

        // Shared store unchanged (no partial application)
        let shared_val = agent_store
            .shared()
            .datoms()
            .find(|d| d.entity == entity && d.attribute.as_str() == ":db/doc");
        assert_eq!(
            shared_val.map(|d| &d.value),
            Some(&Value::String("original".into())),
            "Shared store must be unchanged after failed commit"
        );
    }

    // --- Selective commit (only specified entities) ---

    /// Verifies: commit() only promotes the specified entity IDs, not all
    /// working set datoms.
    #[test]
    fn commit_is_selective() {
        let shared = full_schema_store();
        let mut agent = AgentStore::new(shared, alice());

        let entity_a = EntityId::from_ident(":test/commit-a");
        let entity_b = EntityId::from_ident(":test/commit-b");

        agent
            .assert_local(
                vec![
                    (
                        entity_a,
                        Attribute::from_keyword(":db/doc"),
                        Value::String("A data".into()),
                    ),
                    (
                        entity_b,
                        Attribute::from_keyword(":db/doc"),
                        Value::String("B data".into()),
                    ),
                ],
                ProvenanceType::Observed,
                "two entities",
            )
            .unwrap();

        // Commit only entity_a
        agent.commit(&[entity_a]).unwrap();

        // entity_a in shared
        let a_in_shared = agent.shared().datoms().any(|d| {
            d.entity == entity_a
                && d.attribute.as_str() == ":db/doc"
                && d.value == Value::String("A data".into())
        });
        assert!(a_in_shared, "entity_a should be in shared after commit");

        // entity_b NOT in shared
        let b_in_shared = agent.shared().datoms().any(|d| {
            d.entity == entity_b
                && d.attribute.as_str() == ":db/doc"
                && d.value == Value::String("B data".into())
        });
        assert!(
            !b_in_shared,
            "entity_b should NOT be in shared (not committed)"
        );
    }

    // --- Agent ID accessor ---

    #[test]
    fn agent_id_accessor() {
        let shared = Store::genesis();
        let agent = AgentStore::new(shared, alice());
        assert_eq!(agent.agent_id(), alice());
    }

    // --- Shared/Working accessors ---

    #[test]
    fn shared_and_working_accessors() {
        let shared = Store::genesis();
        let agent = AgentStore::new(shared, alice());

        // Shared has genesis datoms
        assert!(!agent.shared().is_empty());
        // Working also has genesis datoms (starts from genesis)
        assert!(!agent.working().is_empty());
    }

    // --- Multiple commits ---

    /// Verifies: Multiple sequential commits work correctly.
    #[test]
    fn multiple_sequential_commits() {
        let shared = full_schema_store();
        let mut agent = AgentStore::new(shared, alice());

        let entity1 = EntityId::from_ident(":test/seq-1");
        let entity2 = EntityId::from_ident(":test/seq-2");

        // First assert + commit
        agent
            .assert_local(
                vec![(
                    entity1,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("first".into()),
                )],
                ProvenanceType::Observed,
                "first batch",
            )
            .unwrap();
        agent.commit(&[entity1]).unwrap();

        // Second assert + commit
        agent
            .assert_local(
                vec![(
                    entity2,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("second".into()),
                )],
                ProvenanceType::Observed,
                "second batch",
            )
            .unwrap();
        agent.commit(&[entity2]).unwrap();

        // Both entities in shared
        let has_1 = agent
            .shared()
            .datoms()
            .any(|d| d.entity == entity1 && d.value == Value::String("first".into()));
        let has_2 = agent
            .shared()
            .datoms()
            .any(|d| d.entity == entity2 && d.value == Value::String("second".into()));
        assert!(has_1, "First committed entity should be in shared");
        assert!(has_2, "Second committed entity should be in shared");
    }

    // --- Proptest: commit through coherence never violates store invariants ---

    mod proptests {
        use super::*;
        use crate::proptest_strategies::{arb_doc_value, arb_entity_id};
        use proptest::prelude::*;

        proptest! {
            /// INV-STORE-013 + INV-MERGE-003: After commit, the shared store is
            /// a superset of its pre-commit state (monotonic growth), and the
            /// working set is never modified by commit.
            #[test]
            fn commit_preserves_store_monotonicity(
                entity in arb_entity_id(),
                value in arb_doc_value(),
            ) {
                let shared = Store::genesis();
                let pre_shared_count = shared.len();
                let mut agent = AgentStore::new(shared, alice());

                // Assert locally
                let _ = agent.assert_local(
                    vec![(entity, Attribute::from_keyword(":db/doc"), value)],
                    ProvenanceType::Observed,
                    "proptest",
                );

                let pre_working_count = agent.working().len();

                // Commit
                let result = agent.commit(&[entity]);

                // Whether commit succeeds or fails:
                // 1. Shared store monotonically grows (or stays same on failure)
                prop_assert!(agent.shared().len() >= pre_shared_count,
                    "Shared store must not shrink: was {}, now {}",
                    pre_shared_count, agent.shared().len());

                // 2. Working set is never modified by commit (append-only)
                prop_assert!(agent.working().len() >= pre_working_count,
                    "Working set must not shrink: was {}, now {}",
                    pre_working_count, agent.working().len());

                // 3. If commit succeeded, shared store grew
                if result.is_ok() {
                    prop_assert!(agent.shared().len() > pre_shared_count,
                        "Successful commit must grow shared store");
                }
            }

            /// INV-STORE-013: Working set isolation — two agent stores with
            /// independent working sets never see each other's data.
            #[test]
            fn working_set_isolation_property(
                entity_a in arb_entity_id(),
                value_a in arb_doc_value(),
                entity_b in arb_entity_id(),
                value_b in arb_doc_value(),
            ) {
                let shared = Store::genesis();
                let mut alice_s = AgentStore::new(shared.clone_store(), alice());
                let mut bob_s = AgentStore::new(shared, bob());

                // Each agent adds to their own working set
                let _ = alice_s.assert_local(
                    vec![(entity_a, Attribute::from_keyword(":db/doc"), value_a)],
                    ProvenanceType::Observed,
                    "alice proptest",
                );
                let _ = bob_s.assert_local(
                    vec![(entity_b, Attribute::from_keyword(":db/doc"), value_b)],
                    ProvenanceType::Observed,
                    "bob proptest",
                );

                // Alice's working set datoms not in bob's local view
                // (unless entity_a happens to be a genesis entity)
                let alice_working_entities: BTreeSet<EntityId> = alice_s.working()
                    .datoms()
                    .filter(|d| d.attribute.as_str() == ":db/doc")
                    .map(|d| d.entity)
                    .collect();

                let bob_local_entities: BTreeSet<EntityId> = bob_s.query_local()
                    .datoms()
                    .filter(|d| d.attribute.as_str() == ":db/doc")
                    .map(|d| d.entity)
                    .collect();

                // Any entity in alice's working set but not in shared genesis
                // must not appear in bob's local view
                let shared_entities: BTreeSet<EntityId> = Store::genesis()
                    .datoms()
                    .filter(|d| d.attribute.as_str() == ":db/doc")
                    .map(|d| d.entity)
                    .collect();

                for entity in &alice_working_entities {
                    if !shared_entities.contains(entity) {
                        prop_assert!(!bob_local_entities.contains(entity),
                            "INV-STORE-013: Bob must not see Alice's private entity {:?}", entity);
                    }
                }
            }
        }
    }
}
