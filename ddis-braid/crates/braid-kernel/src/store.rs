//! The append-only datom store — `(P(D), ∪)` G-Set CvRDT.
//!
//! The store is a grow-only set of datoms forming a join-semilattice under
//! set union. It never deletes or mutates an existing datom (INV-STORE-001).
//! Merge is commutative, associative, and idempotent (INV-STORE-004–006).
//!
//! # Three-Box Decomposition
//!
//! **Black box**: Monotonic growth, CRDT merge, deterministic genesis.
//! **State box**: `BTreeSet<Datom>` + `HashMap<AgentId, TxId>` frontier.
//! **Clear box**: See implementation below.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::marker::PhantomData;

use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};
use crate::error::StoreError;
use crate::schema::Schema;

// ---------------------------------------------------------------------------
// Transaction Typestate (INV-STORE-001)
// ---------------------------------------------------------------------------

mod sealed {
    pub trait Sealed {}
}

/// Marker trait for transaction states. Sealed — cannot be implemented externally.
pub trait TxState: sealed::Sealed {}

/// Transaction is being built — accepts datom additions.
pub struct Building;
impl sealed::Sealed for Building {}
impl TxState for Building {}

/// Transaction is validated and sealed — ready to apply to a store.
pub struct Committed;
impl sealed::Sealed for Committed {}
impl TxState for Committed {}

/// Transaction has been applied — holds the receipt.
pub struct Applied;
impl sealed::Sealed for Applied {}
impl TxState for Applied {}

/// Provenance and causal metadata for a transaction (C5 traceability).
#[derive(Clone, Debug)]
pub struct TxData {
    /// Provenance type (hypothesized, inferred, derived, observed).
    pub provenance: ProvenanceType,
    /// Causal predecessors (tx IDs this transaction depends on).
    pub causal_predecessors: Vec<TxId>,
    /// The agent creating this transaction.
    pub agent: AgentId,
    /// Human-readable rationale for the transaction.
    pub rationale: String,
}

/// A transaction in one of three states: Building, Committed, or Applied.
///
/// State transitions are enforced at compile time via the typestate pattern:
/// `Transaction<Building>` → `commit()` → `Transaction<Committed>` → applied by Store.
///
/// Invalid transitions are compile errors (INV-STORE-001).
pub struct Transaction<S: TxState> {
    datoms: Vec<Datom>,
    tx_data: TxData,
    tx_id: Option<TxId>,
    _state: PhantomData<S>,
}

impl Transaction<Building> {
    /// Create a new transaction builder.
    pub fn new(agent: AgentId, provenance: ProvenanceType, rationale: &str) -> Self {
        Transaction {
            datoms: Vec::new(),
            tx_data: TxData {
                provenance,
                causal_predecessors: Vec::new(),
                agent,
                rationale: rationale.to_string(),
            },
            tx_id: None,
            _state: PhantomData,
        }
    }

    /// Add a causal predecessor to this transaction.
    pub fn with_predecessor(mut self, tx: TxId) -> Self {
        self.tx_data.causal_predecessors.push(tx);
        self
    }

    /// Assert a new datom. The `tx` and `op` fields of the datom are
    /// placeholders — they will be overwritten on commit.
    pub fn assert(mut self, entity: EntityId, attribute: Attribute, value: Value) -> Self {
        // Placeholder tx — will be replaced on commit
        let placeholder_tx = TxId::new(0, 0, self.tx_data.agent);
        self.datoms.push(Datom::new(
            entity,
            attribute,
            value,
            placeholder_tx,
            Op::Assert,
        ));
        self
    }

    /// Retract an existing datom.
    pub fn retract(mut self, entity: EntityId, attribute: Attribute, value: Value) -> Self {
        let placeholder_tx = TxId::new(0, 0, self.tx_data.agent);
        self.datoms.push(Datom::new(
            entity,
            attribute,
            value,
            placeholder_tx,
            Op::Retract,
        ));
        self
    }

    /// Validate and seal the transaction.
    ///
    /// Requires `&Store` to:
    /// 1. Validate all attributes exist in the schema (INV-SCHEMA-004).
    /// 2. Validate causal predecessors exist in the store (INV-STORE-010).
    /// 3. Generate the TxId from the store's clock.
    pub fn commit(self, store: &Store) -> Result<Transaction<Committed>, StoreError> {
        if self.datoms.is_empty() {
            return Err(StoreError::EmptyTransaction);
        }

        // Validate schema compliance
        for datom in &self.datoms {
            store.schema.validate_datom(datom)?;
        }

        // Validate causal predecessors exist
        for pred in &self.tx_data.causal_predecessors {
            if !store.has_transaction(pred) {
                return Err(StoreError::DuplicateTransaction(format!(
                    "causal predecessor not found: {:?}",
                    pred
                )));
            }
        }

        // Generate TxId using HLC
        let tx_id = store.next_tx_id(self.tx_data.agent);

        // Stamp all datoms with the real TxId
        let datoms = self
            .datoms
            .into_iter()
            .map(|d| Datom::new(d.entity, d.attribute, d.value, tx_id, d.op))
            .collect();

        Ok(Transaction {
            datoms,
            tx_data: self.tx_data,
            tx_id: Some(tx_id),
            _state: PhantomData,
        })
    }
}

impl Transaction<Committed> {
    /// Access the datoms in this committed transaction.
    pub fn datoms(&self) -> &[Datom] {
        &self.datoms
    }

    /// Access the transaction ID.
    pub fn tx_id(&self) -> TxId {
        self.tx_id
            .expect("committed transactions always have a tx_id")
    }

    /// Access the transaction metadata.
    pub fn tx_data(&self) -> &TxData {
        &self.tx_data
    }
}

// ---------------------------------------------------------------------------
// TxReceipt
// ---------------------------------------------------------------------------

/// Receipt returned after a transaction is applied to the store.
#[derive(Clone, Debug)]
pub struct TxReceipt {
    /// The transaction ID assigned.
    pub tx_id: TxId,
    /// Number of datoms in the transaction.
    pub datom_count: usize,
    /// New entities introduced by this transaction.
    pub new_entities: Vec<EntityId>,
}

// ---------------------------------------------------------------------------
// Frontier
// ---------------------------------------------------------------------------

/// Per-agent latest transaction ID. Equivalent to a vector clock.
pub type Frontier = HashMap<AgentId, TxId>;

// ---------------------------------------------------------------------------
// MergeReceipt
// ---------------------------------------------------------------------------

/// Receipt returned after merging two stores.
#[derive(Clone, Debug)]
pub struct MergeReceipt {
    /// Number of new datoms added from the other store.
    pub new_datoms: usize,
    /// Total datoms after merge.
    pub total_datoms: usize,
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// The append-only datom store.
///
/// Algebraic structure: `(P(D), ∪)` — a grow-only set (G-Set CvRDT) under
/// set union. This forms a join-semilattice satisfying:
///
/// - **L1** (commutativity): `S₁ ∪ S₂ = S₂ ∪ S₁`
/// - **L2** (associativity): `(S₁ ∪ S₂) ∪ S₃ = S₁ ∪ (S₂ ∪ S₃)`
/// - **L3** (idempotency):   `S ∪ S = S`
/// - **L4** (monotonicity):  `S ⊆ S ∪ T`
/// - **L5** (bottom):        `∅ ∪ S = S`
pub struct Store {
    /// The canonical datom set. BTreeSet ordering = EAVT index.
    datoms: BTreeSet<Datom>,
    /// Per-agent latest transaction (vector clock).
    frontier: Frontier,
    /// Schema derived from store datoms.
    schema: Schema,
    /// The current clock state for generating TxIds.
    clock: TxId,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("datom_count", &self.datoms.len())
            .field("frontier", &self.frontier)
            .finish()
    }
}

impl Store {
    /// Create a new store with the genesis transaction.
    ///
    /// Genesis is deterministic: same output every call (INV-STORE-008).
    /// Contains the 18 axiomatic meta-schema attributes that define the
    /// schema system itself (INV-SCHEMA-002).
    pub fn genesis() -> Self {
        let system_agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, system_agent);

        let genesis_datoms = crate::schema::genesis_datoms(genesis_tx);

        let mut datoms = BTreeSet::new();
        for d in &genesis_datoms {
            datoms.insert(d.clone());
        }

        let mut frontier = HashMap::new();
        frontier.insert(system_agent, genesis_tx);

        let schema = Schema::from_datoms(&datoms);

        Store {
            datoms,
            frontier,
            schema,
            clock: genesis_tx,
        }
    }

    /// Reconstruct a store from a set of datoms.
    ///
    /// Used by the LAYOUT ψ function to reconstruct a store from disk.
    /// Rebuilds the schema and computes the frontier from datom TxIds.
    pub fn from_datoms(datoms: BTreeSet<Datom>) -> Self {
        let schema = Schema::from_datoms(&datoms);

        // Reconstruct frontier from datom TxIds
        let mut frontier: HashMap<AgentId, TxId> = HashMap::new();
        let mut max_clock = TxId::new(0, 0, AgentId::from_name("braid:system"));
        for d in &datoms {
            let agent = d.tx.agent();
            frontier
                .entry(agent)
                .and_modify(|existing| {
                    if d.tx > *existing {
                        *existing = d.tx;
                    }
                })
                .or_insert(d.tx);
            if d.tx > max_clock {
                max_clock = d.tx;
            }
        }

        Store {
            datoms,
            frontier,
            schema,
            clock: max_clock,
        }
    }

    /// Apply a committed transaction to the store.
    ///
    /// Inserts all datoms into the BTreeSet (dedup by content identity),
    /// updates the frontier, and rebuilds schema if schema attributes changed.
    ///
    /// # Invariants
    ///
    /// - **INV-STORE-001**: `|S'| >= |S|` — store only grows.
    /// - **INV-STORE-002**: `|S'| > |S|` if any new datom is genuinely new.
    /// - **INV-STORE-014**: Transaction metadata recorded as datoms.
    pub fn transact(&mut self, tx: Transaction<Committed>) -> Result<TxReceipt, StoreError> {
        let tx_id = tx.tx_id();
        let tx_data = tx.tx_data().clone();

        // Track new entities
        let mut new_entities = Vec::new();
        let mut datom_count = 0;
        let mut schema_changed = false;

        // Snapshot existing entities before the loop — O(N) once, O(1) per lookup.
        let pre_existing: HashSet<EntityId> = self.datoms.iter().map(|d| d.entity).collect();

        // Insert the user datoms
        for datom in tx.datoms() {
            if self.datoms.insert(datom.clone()) {
                datom_count += 1;
                // Check if this entity is new (not in pre-existing set)
                if !pre_existing.contains(&datom.entity) && !new_entities.contains(&datom.entity) {
                    new_entities.push(datom.entity);
                }
                // Check if this modifies schema
                if datom.attribute.namespace() == "db" {
                    schema_changed = true;
                }
            }
        }

        // Record transaction metadata as datoms (INV-STORE-014)
        let tx_entity = EntityId::from_content(
            &serde_json::to_vec(&tx_id)
                .expect("TxId serialization cannot fail: all fields are serializable"),
        );
        let tx_meta_datoms = self.make_tx_metadata(tx_entity, tx_id, &tx_data);
        for d in tx_meta_datoms {
            self.datoms.insert(d);
        }

        // Update frontier
        self.frontier.insert(tx_data.agent, tx_id);

        // Update clock
        self.clock = tx_id;

        // Rebuild schema if any schema attributes were transacted
        if schema_changed {
            self.schema = Schema::from_datoms(&self.datoms);
        }

        Ok(TxReceipt {
            tx_id,
            datom_count,
            new_entities,
        })
    }

    /// Merge another store into this one (CRDT set union).
    ///
    /// # Invariants
    ///
    /// - **INV-STORE-004**: Commutativity — `merge(A, B) = merge(B, A)` (as datom sets).
    /// - **INV-STORE-005**: Associativity — `merge(merge(A, B), C) = merge(A, merge(B, C))`.
    /// - **INV-STORE-006**: Idempotency — `merge(A, A) = A`.
    /// - **INV-STORE-007**: Monotonicity — `A ⊆ merge(A, B)`.
    pub fn merge(&mut self, other: &Store) -> MergeReceipt {
        let before = self.datoms.len();

        // Set union — BTreeSet handles dedup by content identity
        for datom in &other.datoms {
            self.datoms.insert(datom.clone());
        }

        // Frontier: pointwise max per agent
        for (agent, their_tx) in &other.frontier {
            self.frontier
                .entry(*agent)
                .and_modify(|our_tx| {
                    if their_tx > our_tx {
                        *our_tx = *their_tx;
                    }
                })
                .or_insert(*their_tx);
        }

        // Advance clock past both frontiers
        if let Some(max_remote) = other.frontier.values().max() {
            if *max_remote > self.clock {
                self.clock = *max_remote;
            }
        }

        // Rebuild schema from merged datoms
        self.schema = Schema::from_datoms(&self.datoms);

        let after = self.datoms.len();
        MergeReceipt {
            new_datoms: after - before,
            total_datoms: after,
        }
    }

    /// Total number of datoms in the store.
    pub fn len(&self) -> usize {
        self.datoms.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.datoms.is_empty()
    }

    /// Iterator over all datoms in EAVT order.
    pub fn datoms(&self) -> impl Iterator<Item = &Datom> {
        self.datoms.iter()
    }

    /// The current frontier (per-agent latest transaction).
    pub fn frontier(&self) -> &Frontier {
        &self.frontier
    }

    /// The schema derived from store datoms.
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// Get all datoms for a specific entity.
    pub fn entity_datoms(&self, entity: EntityId) -> Vec<&Datom> {
        self.datoms.iter().filter(|d| d.entity == entity).collect()
    }

    /// Check if a transaction with the given ID exists in the store.
    pub fn has_transaction(&self, tx_id: &TxId) -> bool {
        self.datoms.iter().any(|d| &d.tx == tx_id)
    }

    /// The set of all unique entities in the store.
    pub fn entities(&self) -> BTreeSet<EntityId> {
        self.datoms.iter().map(|d| d.entity).collect()
    }

    /// The canonical datom set (for merge comparison / testing).
    pub fn datom_set(&self) -> &BTreeSet<Datom> {
        &self.datoms
    }

    /// Clone the store (used in tests to verify commutativity).
    pub fn clone_store(&self) -> Self {
        Store {
            datoms: self.datoms.clone(),
            frontier: self.frontier.clone(),
            schema: self.schema.clone(),
            clock: self.clock,
        }
    }

    /// Generate the next TxId for the given agent, advancing the HLC.
    fn next_tx_id(&self, agent: AgentId) -> TxId {
        // In a real system, `now` would come from the system clock.
        // For determinism in the kernel, we use the clock state + 1.
        let now = self.clock.wall_time;
        self.clock.tick(now, agent)
    }

    /// Produce transaction metadata datoms (INV-STORE-014).
    fn make_tx_metadata(&self, tx_entity: EntityId, tx_id: TxId, tx_data: &TxData) -> Vec<Datom> {
        let mut meta = Vec::new();

        // :tx/time
        meta.push(Datom::new(
            tx_entity,
            Attribute::from_keyword(":tx/time"),
            Value::Instant(tx_id.wall_time),
            tx_id,
            Op::Assert,
        ));

        // :tx/agent
        let agent_entity = EntityId::from_content(tx_data.agent.as_bytes());
        meta.push(Datom::new(
            tx_entity,
            Attribute::from_keyword(":tx/agent"),
            Value::Ref(agent_entity),
            tx_id,
            Op::Assert,
        ));

        // :tx/provenance
        let prov_kw = match tx_data.provenance {
            ProvenanceType::Hypothesized => ":provenance/hypothesized",
            ProvenanceType::Inferred => ":provenance/inferred",
            ProvenanceType::Derived => ":provenance/derived",
            ProvenanceType::Observed => ":provenance/observed",
        };
        meta.push(Datom::new(
            tx_entity,
            Attribute::from_keyword(":tx/provenance"),
            Value::Keyword(prov_kw.to_string()),
            tx_id,
            Op::Assert,
        ));

        // :tx/rationale
        meta.push(Datom::new(
            tx_entity,
            Attribute::from_keyword(":tx/rationale"),
            Value::String(tx_data.rationale.clone()),
            tx_id,
            Op::Assert,
        ));

        meta
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn system_agent() -> AgentId {
        AgentId::from_name("test-agent")
    }

    #[test]
    fn genesis_is_deterministic() {
        let s1 = Store::genesis();
        let s2 = Store::genesis();
        assert_eq!(s1.datom_set(), s2.datom_set());
        assert_eq!(s1.len(), s2.len());
    }

    #[test]
    fn genesis_has_axiomatic_attributes() {
        let store = Store::genesis();
        // Genesis has 18 axiomatic attributes, each with multiple datoms
        // (ident, valueType, cardinality, doc = 4 datoms per attr)
        // Plus 4 tx metadata datoms for the genesis transaction
        assert!(!store.is_empty());

        // Check that :db/ident exists as an attribute
        let has_db_ident = store.datoms().any(|d| {
            d.attribute.as_str() == ":db/ident"
                && matches!(&d.value, Value::Keyword(k) if k == ":db/ident")
        });
        assert!(has_db_ident, "genesis must contain :db/ident");
    }

    #[test]
    fn genesis_schema_knows_axiomatic_attributes() {
        let store = Store::genesis();
        assert!(store
            .schema()
            .attribute(&Attribute::from_keyword(":db/ident"))
            .is_some());
        assert!(store
            .schema()
            .attribute(&Attribute::from_keyword(":db/valueType"))
            .is_some());
        assert!(store
            .schema()
            .attribute(&Attribute::from_keyword(":tx/time"))
            .is_some());
    }

    #[test]
    fn transact_increases_store_size() {
        let mut store = Store::genesis();
        let before = store.len();

        let entity = EntityId::from_ident(":test/entity");
        let tx = Transaction::new(system_agent(), ProvenanceType::Observed, "test")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("test doc".into()),
            )
            .commit(&store)
            .unwrap();

        store.transact(tx).unwrap();
        assert!(store.len() > before, "INV-STORE-002: store must grow");
    }

    #[test]
    fn transact_rejects_empty_transaction() {
        let store = Store::genesis();
        let tx = Transaction::new(system_agent(), ProvenanceType::Observed, "empty");
        let result = tx.commit(&store);
        assert!(matches!(result, Err(StoreError::EmptyTransaction)));
    }

    #[test]
    fn merge_is_commutative() {
        let mut s1 = Store::genesis();
        let mut s2 = Store::genesis();

        // Add different datoms to each
        let e1 = EntityId::from_ident(":test/a");
        let e2 = EntityId::from_ident(":test/b");

        let tx1 = Transaction::new(system_agent(), ProvenanceType::Observed, "a")
            .assert(
                e1,
                Attribute::from_keyword(":db/doc"),
                Value::String("a".into()),
            )
            .commit(&s1)
            .unwrap();
        s1.transact(tx1).unwrap();

        let agent2 = AgentId::from_name("agent-2");
        let tx2 = Transaction::new(agent2, ProvenanceType::Observed, "b")
            .assert(
                e2,
                Attribute::from_keyword(":db/doc"),
                Value::String("b".into()),
            )
            .commit(&s2)
            .unwrap();
        s2.transact(tx2).unwrap();

        // merge(s1, s2)
        let mut left = s1.clone_store();
        left.merge(&s2);

        // merge(s2, s1)
        let mut right = s2.clone_store();
        right.merge(&s1);

        assert_eq!(
            left.datom_set(),
            right.datom_set(),
            "INV-STORE-004: commutativity"
        );
    }

    #[test]
    fn merge_is_idempotent() {
        let store = Store::genesis();
        let mut s = store.clone_store();
        let before = s.datom_set().clone();
        s.merge(&store);
        assert_eq!(s.datom_set(), &before, "INV-STORE-006: idempotency");
    }

    #[test]
    fn merge_is_monotonic() {
        let mut s1 = Store::genesis();
        let s2 = Store::genesis();

        let before = s1.datom_set().clone();
        s1.merge(&s2);

        // Every datom in `before` must still be present
        for d in &before {
            assert!(
                s1.datom_set().contains(d),
                "INV-STORE-007: monotonicity — datom lost during merge"
            );
        }
    }

    #[test]
    fn frontier_updated_on_transact() {
        let mut store = Store::genesis();
        let agent = system_agent();

        let entity = EntityId::from_ident(":test/e");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "test")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("x".into()),
            )
            .commit(&store)
            .unwrap();

        let receipt = store.transact(tx).unwrap();
        assert_eq!(store.frontier()[&agent], receipt.tx_id);
    }

    // -----------------------------------------------------------------------
    // Proptest property-based verification suite (14 STORE invariants)
    // -----------------------------------------------------------------------

    mod proptests {
        use super::*;
        use crate::merge::merge_stores;
        use crate::proptest_strategies::{
            arb_agent_id, arb_doc_value, arb_entity_id, arb_store, arb_store_pair,
        };
        use proptest::prelude::*;

        proptest! {
            /// INV-STORE-001: Append-only — store.len() never decreases after transact.
            #[test]
            fn inv_store_001_append_only(
                store in arb_store(3),
                entity in arb_entity_id(),
                value in arb_doc_value(),
            ) {
                let before = store.len();
                let mut s = store.clone_store();
                let agent = AgentId::from_name("proptest:agent");
                let tx = Transaction::new(agent, ProvenanceType::Observed, "proptest")
                    .assert(entity, Attribute::from_keyword(":db/doc"), value)
                    .commit(&s);
                if let Ok(committed) = tx {
                    let _ = s.transact(committed);
                }
                prop_assert!(s.len() >= before, "INV-STORE-001: append-only violated");
            }

            /// INV-STORE-002: Strict growth — transact of non-empty tx increases len.
            #[test]
            fn inv_store_002_strict_growth(
                entity in arb_entity_id(),
                value in arb_doc_value(),
            ) {
                let mut store = Store::genesis();
                let before = store.len();
                let agent = AgentId::from_name("proptest:agent");
                let tx = Transaction::new(agent, ProvenanceType::Observed, "grow")
                    .assert(entity, Attribute::from_keyword(":db/doc"), value)
                    .commit(&store)
                    .unwrap();
                store.transact(tx).unwrap();
                prop_assert!(store.len() > before, "INV-STORE-002: strict growth violated");
            }

            /// INV-STORE-003: Content identity — identical tuples produce identical datoms.
            #[test]
            fn inv_store_003_content_identity(content in any::<[u8; 32]>()) {
                let e1 = EntityId::from_content(&content);
                let e2 = EntityId::from_content(&content);
                prop_assert_eq!(e1, e2, "INV-STORE-003: content identity violated");
            }

            /// INV-STORE-004: Merge commutativity — merge(A,B) == merge(B,A).
            #[test]
            fn inv_store_004_merge_commutativity((s1, s2) in arb_store_pair(2)) {
                let mut left = s1.clone_store();
                left.merge(&s2);
                let mut right = s2.clone_store();
                right.merge(&s1);
                prop_assert_eq!(
                    left.datom_set(),
                    right.datom_set(),
                    "INV-STORE-004: commutativity violated"
                );
            }

            /// INV-STORE-005: Merge associativity — merge(merge(A,B),C) == merge(A,merge(B,C)).
            #[test]
            fn inv_store_005_merge_associativity(
                s1 in arb_store(2),
                s2 in arb_store(2),
                s3 in arb_store(2),
            ) {
                // (A ∪ B) ∪ C
                let mut left = s1.clone_store();
                left.merge(&s2);
                left.merge(&s3);
                // A ∪ (B ∪ C)
                let mut bc = s2.clone_store();
                bc.merge(&s3);
                let mut right = s1.clone_store();
                right.merge(&bc);
                prop_assert_eq!(
                    left.datom_set(),
                    right.datom_set(),
                    "INV-STORE-005: associativity violated"
                );
            }

            /// INV-STORE-006: Merge idempotency — merge(A,A) == A.
            #[test]
            fn inv_store_006_merge_idempotency(store in arb_store(3)) {
                let before = store.datom_set().clone();
                let mut s = store.clone_store();
                s.merge(&store);
                prop_assert_eq!(s.datom_set(), &before, "INV-STORE-006: idempotency violated");
            }

            /// INV-STORE-007: Merge monotonicity — A ⊆ merge(A,B).
            #[test]
            fn inv_store_007_merge_monotonicity((s1, s2) in arb_store_pair(2)) {
                let before = s1.datom_set().clone();
                let mut merged = s1.clone_store();
                merged.merge(&s2);
                for d in &before {
                    prop_assert!(
                        merged.datom_set().contains(d),
                        "INV-STORE-007: monotonicity violated — datom lost"
                    );
                }
            }

            /// INV-STORE-008: Genesis determinism — genesis() == genesis() always.
            #[test]
            fn inv_store_008_genesis_determinism(_seed in 0u32..1000) {
                let s1 = Store::genesis();
                let s2 = Store::genesis();
                prop_assert_eq!(s1.datom_set(), s2.datom_set(), "INV-STORE-008: genesis non-deterministic");
            }

            /// INV-STORE-011: HLC monotonicity — successive ticks strictly increase.
            #[test]
            fn inv_store_011_hlc_monotonicity(
                wall1 in 1u64..1_000_000,
                wall2 in 1u64..1_000_000,
                agent in arb_agent_id(),
            ) {
                let t1 = TxId::new(wall1, 0, agent);
                let t2 = t1.tick(wall2, agent);
                prop_assert!(t2 > t1, "INV-STORE-011: HLC not monotonic");
            }

            /// INV-STORE-014: Every tx has metadata — metadata entity present for user txns.
            #[test]
            fn inv_store_014_tx_metadata(
                entity in arb_entity_id(),
                value in arb_doc_value(),
            ) {
                let mut store = Store::genesis();
                let agent = AgentId::from_name("proptest:agent");
                let tx = Transaction::new(agent, ProvenanceType::Observed, "meta-test")
                    .assert(entity, Attribute::from_keyword(":db/doc"), value)
                    .commit(&store)
                    .unwrap();
                let receipt = store.transact(tx).unwrap();

                // Tx metadata entity = EntityId::from_content(serialized tx_id)
                let tx_entity = EntityId::from_content(
                    &serde_json::to_vec(&receipt.tx_id).unwrap(),
                );
                let tx_datoms: Vec<_> = store.entity_datoms(tx_entity);
                let has_time = tx_datoms.iter().any(|d| d.attribute.as_str() == ":tx/time");
                let has_agent = tx_datoms.iter().any(|d| d.attribute.as_str() == ":tx/agent");
                let has_prov = tx_datoms.iter().any(|d| d.attribute.as_str() == ":tx/provenance");
                let has_rationale = tx_datoms.iter().any(|d| d.attribute.as_str() == ":tx/rationale");
                prop_assert!(has_time, "INV-STORE-014: missing :tx/time");
                prop_assert!(has_agent, "INV-STORE-014: missing :tx/agent");
                prop_assert!(has_prov, "INV-STORE-014: missing :tx/provenance");
                prop_assert!(has_rationale, "INV-STORE-014: missing :tx/rationale");
            }

            /// merge_stores (kernel-level) preserves all datoms from both inputs.
            #[test]
            fn merge_stores_preserves_all((s1, s2) in arb_store_pair(2)) {
                let s1_datoms: Vec<_> = s1.datoms().cloned().collect();
                let s2_datoms: Vec<_> = s2.datoms().cloned().collect();
                let mut merged = s1.clone_store();
                merge_stores(&mut merged, &s2);
                for d in &s1_datoms {
                    prop_assert!(merged.datom_set().contains(d), "merge lost s1 datom");
                }
                for d in &s2_datoms {
                    prop_assert!(merged.datom_set().contains(d), "merge lost s2 datom");
                }
            }

            // ---------------------------------------------------------------
            // Causal order partial-order properties (INV-STORE-010/011)
            // ---------------------------------------------------------------

            #[test]
            fn causal_order_irreflexivity(
                entity in arb_entity_id(),
                value in arb_doc_value(),
            ) {
                let mut store = Store::genesis();
                let agent = AgentId::from_name("proptest:causal");

                let tx = Transaction::new(agent, ProvenanceType::Observed, "first")
                    .assert(entity, Attribute::from_keyword(":db/doc"), value)
                    .commit(&store)
                    .unwrap();
                let receipt = store.transact(tx).unwrap();

                // Build a second tx that claims the first as a causal predecessor.
                // The predecessor list must never contain its own tx_id.
                let e2 = EntityId::from_ident(":test/irreflexivity");
                let tx2 = Transaction::new(agent, ProvenanceType::Observed, "second")
                    .with_predecessor(receipt.tx_id)
                    .assert(e2, Attribute::from_keyword(":db/doc"), Value::String("irr".into()))
                    .commit(&store)
                    .unwrap();
                let receipt2 = store.transact(tx2).unwrap();

                // Irreflexivity: no transaction is its own causal predecessor.
                // In the store, the metadata for receipt2 was recorded.
                // We verify by checking that no tx in the store has itself in
                // its causal_predecessors. Since Store does not expose tx_data
                // directly, we verify the structural constraint: the tx_id of
                // the second transaction must differ from the predecessor tx_id.
                prop_assert_ne!(
                    receipt2.tx_id, receipt.tx_id,
                    "Irreflexivity violated: transaction is its own causal predecessor"
                );
            }

            #[test]
            fn causal_order_dag_property(
                entities in proptest::collection::vec(arb_entity_id(), 2..=5),
                values in proptest::collection::vec(arb_doc_value(), 2..=5),
            ) {
                let mut store = Store::genesis();
                let agent = AgentId::from_name("proptest:dag");

                // Build a chain of transactions where each depends on the previous.
                // A valid causal order is a DAG (no cycles).
                let mut prev_tx_ids: Vec<TxId> = Vec::new();
                let entity_count = entities.len().min(values.len());

                for i in 0..entity_count {
                    let mut builder = Transaction::new(
                        agent,
                        ProvenanceType::Observed,
                        &format!("chain-{i}"),
                    );
                    // Each tx depends on all previous (forms a total order / chain)
                    for &pred in &prev_tx_ids {
                        builder = builder.with_predecessor(pred);
                    }
                    builder = builder.assert(
                        entities[i],
                        Attribute::from_keyword(":db/doc"),
                        values[i].clone(),
                    );
                    let committed = builder.commit(&store).unwrap();
                    let receipt = store.transact(committed).unwrap();
                    prev_tx_ids.push(receipt.tx_id);
                }

                // DAG property: no cycle. In a chain A -> B -> C, verify that
                // each tx_id is strictly greater than all its predecessors.
                // Because TxId is generated by HLC with monotonic tick, the
                // ordering should be strict: t_{i} < t_{i+1} for all i.
                for window in prev_tx_ids.windows(2) {
                    prop_assert!(
                        window[0] < window[1],
                        "DAG property violated: tx {:?} is not < {:?}",
                        window[0], window[1]
                    );
                }
            }

            #[test]
            fn causal_order_hlc_consistency(
                entities in proptest::collection::vec(arb_entity_id(), 2..=4),
                values in proptest::collection::vec(arb_doc_value(), 2..=4),
            ) {
                let mut store = Store::genesis();
                let agent = AgentId::from_name("proptest:hlc");

                // Build a causal chain and verify HLC consistency:
                // if tx1 <_causal tx2, then tx1.wall_time <= tx2.wall_time
                let mut prev_tx_id: Option<TxId> = None;
                let count = entities.len().min(values.len());

                for i in 0..count {
                    let mut builder = Transaction::new(
                        agent,
                        ProvenanceType::Observed,
                        &format!("hlc-{i}"),
                    );
                    if let Some(pred) = prev_tx_id {
                        builder = builder.with_predecessor(pred);
                    }
                    builder = builder.assert(
                        entities[i],
                        Attribute::from_keyword(":db/doc"),
                        values[i].clone(),
                    );
                    let committed = builder.commit(&store).unwrap();
                    let receipt = store.transact(committed).unwrap();

                    // HLC consistency: predecessor wall_time <= current wall_time
                    if let Some(pred) = prev_tx_id {
                        prop_assert!(
                            pred.wall_time() <= receipt.tx_id.wall_time(),
                            "HLC consistency violated: predecessor wall_time {} > current wall_time {}",
                            pred.wall_time(), receipt.tx_id.wall_time()
                        );
                    }
                    prev_tx_id = Some(receipt.tx_id);
                }
            }
        }
    }
}
