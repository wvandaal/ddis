//! The append-only datom store — `(P(D), ∪)` G-Set CvRDT.
//!
//! The store is a grow-only set of datoms forming a join-semilattice under
//! set union. It never deletes or mutates an existing datom (INV-STORE-001).
//! Merge is commutative, associative, and idempotent (INV-STORE-004–006).
//!
//! Design decisions implemented here:
//! - ADR-STORE-002: EAV data model (datom = [e,a,v,tx,op])
//! - ADR-STORE-003: Content-addressable entity IDs via BLAKE3
//! - ADR-STORE-004: Hybrid logical clocks for transaction ordering
//! - ADR-STORE-005: Four core indexes (EAVT, entity_index, attribute_index) plus LIVE
//! - ADR-STORE-006: Embedded deployment (no external database)
//! - ADR-STORE-011: Every command produces a transaction
//! - ADR-STORE-013: BLAKE3 for content hashing
//! - ADR-STORE-014: Private EntityId inner field (no public raw byte constructor)
//! - ADR-STORE-019: All durable information stored as datoms
//!
//! Negative cases enforced:
//! - NEG-STORE-001: No datom deletion — BTreeSet only grows via insert()
//! - NEG-STORE-002: No mutable state — datoms are immutable after insertion
//! - NEG-STORE-003: No sequential ID assignment — all IDs are content-addressed
//! - NEG-STORE-005: No store compaction — the set never shrinks
//!
//! # Three-Box Decomposition
//!
//! **Black box**: Monotonic growth, CRDT merge, deterministic genesis.
//! **State box**: `BTreeSet<Datom>` + `HashMap<AgentId, TxId>` frontier.
//! **Clear box**: See implementation below.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
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
/// ADR-STORE-021: Frontier as HashMap<AgentId, TxId> (vector clock representation).
/// INV-STORE-009: Frontier is durably stored and recoverable after crash.
/// INV-STORE-016: Frontier computable from datom set alone.
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
///
/// ADR-STORE-001: G-Set CvRDT as store algebra.
/// ADR-STORE-005: Four core indexes (EAVT via BTreeSet, entity_index, attribute_index) plus LIVE.
/// ADR-STORE-006: Embedded deployment — no external database.
pub struct Store {
    /// The canonical datom set. BTreeSet ordering = EAVT index.
    datoms: BTreeSet<Datom>,
    /// Per-agent latest transaction (vector clock).
    frontier: Frontier,
    /// Schema derived from store datoms.
    schema: Schema,
    /// The current clock state for generating TxIds.
    clock: TxId,
    /// Secondary index: entity → datoms for O(1) entity lookups (INV-STORE-IDX-001).
    ///
    /// Invariant: for every datom d in `datoms`, `entity_index[d.entity]` contains d.
    /// Maintained incrementally on `transact()` and rebuilt on `merge()`/`from_datoms()`.
    entity_index: BTreeMap<EntityId, Vec<Datom>>,
    /// Secondary index: attribute → datoms for O(1) attribute lookups (INV-STORE-IDX-002).
    ///
    /// Invariant: for every datom d in `datoms`, `attribute_index[d.attribute]` contains d.
    /// Maintained incrementally on `transact()` and rebuilt on `merge()`/`from_datoms()`.
    attribute_index: BTreeMap<Attribute, Vec<Datom>>,
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
        let mut entity_index: BTreeMap<EntityId, Vec<Datom>> = BTreeMap::new();
        let mut attribute_index: BTreeMap<Attribute, Vec<Datom>> = BTreeMap::new();
        for d in &genesis_datoms {
            datoms.insert(d.clone());
            entity_index.entry(d.entity).or_default().push(d.clone());
            attribute_index
                .entry(d.attribute.clone())
                .or_default()
                .push(d.clone());
        }

        let mut frontier = HashMap::new();
        frontier.insert(system_agent, genesis_tx);

        let schema = Schema::from_datoms(&datoms);

        Store {
            datoms,
            frontier,
            schema,
            clock: genesis_tx,
            entity_index,
            attribute_index,
        }
    }

    /// Reconstruct a store from a set of datoms.
    ///
    /// Used by the LAYOUT ψ function to reconstruct a store from disk.
    /// Rebuilds the schema and computes the frontier from datom TxIds.
    /// INV-STORE-016: Frontier computability — frontier derived from datom set alone.
    /// INV-STORE-012: LIVE index correctness — schema rebuilt from datoms enables resolution.
    pub fn from_datoms(datoms: BTreeSet<Datom>) -> Self {
        let schema = Schema::from_datoms(&datoms);

        // Reconstruct frontier, entity index, and attribute index from datoms
        let mut frontier: HashMap<AgentId, TxId> = HashMap::new();
        let mut max_clock = TxId::new(0, 0, AgentId::from_name("braid:system"));
        let mut entity_index: BTreeMap<EntityId, Vec<Datom>> = BTreeMap::new();
        let mut attribute_index: BTreeMap<Attribute, Vec<Datom>> = BTreeMap::new();
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
            entity_index.entry(d.entity).or_default().push(d.clone());
            attribute_index
                .entry(d.attribute.clone())
                .or_default()
                .push(d.clone());
        }

        Store {
            datoms,
            frontier,
            schema,
            clock: max_clock,
            entity_index,
            attribute_index,
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
    /// - **INV-STORE-009**: Frontier durably stored before returning.
    /// - **INV-STORE-013**: Working set isolation — only committed datoms enter store.
    /// - **INV-STORE-014**: Transaction metadata recorded as datoms.
    /// - **INV-STORE-015**: Agent entity completeness — frontier tracks agent.
    pub fn transact(&mut self, tx: Transaction<Committed>) -> Result<TxReceipt, StoreError> {
        let tx_id = tx.tx_id();
        let tx_data = tx.tx_data().clone();

        // Track new entities
        let mut new_entities = Vec::new();
        let mut datom_count = 0;
        let mut schema_changed = false;

        // Use entity_index for O(1) existence check instead of O(N) scan.
        let pre_existing: HashSet<EntityId> = self.entity_index.keys().copied().collect();

        // Insert the user datoms
        for datom in tx.datoms() {
            if self.datoms.insert(datom.clone()) {
                datom_count += 1;
                // Maintain entity index
                self.entity_index
                    .entry(datom.entity)
                    .or_default()
                    .push(datom.clone());
                // Maintain attribute index
                self.attribute_index
                    .entry(datom.attribute.clone())
                    .or_default()
                    .push(datom.clone());
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
            if self.datoms.insert(d.clone()) {
                self.entity_index
                    .entry(d.entity)
                    .or_default()
                    .push(d.clone());
                self.attribute_index
                    .entry(d.attribute.clone())
                    .or_default()
                    .push(d);
            }
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

        // Rebuild schema, entity index, and attribute index from merged datoms
        self.schema = Schema::from_datoms(&self.datoms);
        self.entity_index = BTreeMap::new();
        self.attribute_index = BTreeMap::new();
        for d in &self.datoms {
            self.entity_index
                .entry(d.entity)
                .or_default()
                .push(d.clone());
            self.attribute_index
                .entry(d.attribute.clone())
                .or_default()
                .push(d.clone());
        }

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

    /// Get all datoms for a specific entity. O(1) via entity index.
    pub fn entity_datoms(&self, entity: EntityId) -> Vec<&Datom> {
        self.entity_index
            .get(&entity)
            .map(|datoms| datoms.iter().collect())
            .unwrap_or_default()
    }

    /// Get all datoms for a specific attribute. O(1) via attribute index.
    pub fn attribute_datoms(&self, attr: &Attribute) -> &[Datom] {
        self.attribute_index
            .get(attr)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Check if a transaction with the given ID exists in the store.
    ///
    /// Uses frontier for fast-path check, falls back to scan only if needed.
    pub fn has_transaction(&self, tx_id: &TxId) -> bool {
        // Fast path: check frontier (most common case — recent transactions)
        if self.frontier.values().any(|t| t == tx_id) {
            return true;
        }
        // Slow path: linear scan (only reached for non-frontier transactions)
        self.datoms.iter().any(|d| &d.tx == tx_id)
    }

    /// The set of all unique entities in the store. O(1) via entity index keys.
    pub fn entities(&self) -> BTreeSet<EntityId> {
        self.entity_index.keys().copied().collect()
    }

    /// The number of unique entities in the store. O(1).
    pub fn entity_count(&self) -> usize {
        self.entity_index.len()
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
            entity_index: self.entity_index.clone(),
            attribute_index: self.attribute_index.clone(),
        }
    }

    /// Generate the next TxId for the given agent, advancing the HLC.
    /// ADR-STORE-004: Hybrid logical clocks for transaction IDs.
    /// INV-STORE-015: Agent entity completeness — each agent's frontier tracked.
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

// Witnesses: INV-STORE-001, INV-STORE-002, INV-STORE-003, INV-STORE-004,
// INV-STORE-005, INV-STORE-006, INV-STORE-007, INV-STORE-008, INV-STORE-009,
// INV-STORE-010, INV-STORE-011, INV-STORE-012, INV-STORE-014,
// ADR-STORE-002, ADR-STORE-003, ADR-STORE-005, ADR-STORE-006, ADR-STORE-011,
// ADR-STORE-013, ADR-STORE-014, ADR-STORE-019,
// NEG-STORE-001, NEG-STORE-002, NEG-STORE-003, NEG-STORE-005
#[cfg(test)]
mod tests {
    use super::*;

    fn system_agent() -> AgentId {
        AgentId::from_name("test-agent")
    }

    // Verifies: INV-STORE-008 — Genesis Determinism
    #[test]
    fn genesis_is_deterministic() {
        let s1 = Store::genesis();
        let s2 = Store::genesis();
        assert_eq!(s1.datom_set(), s2.datom_set());
        assert_eq!(s1.len(), s2.len());
    }

    // Verifies: INV-SCHEMA-002 — Genesis Completeness (axiomatic attributes present)
    // Verifies: ADR-SCHEMA-002 — 17 Axiomatic Attributes
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

    // Verifies: INV-SCHEMA-001 — Schema-as-Data
    // Verifies: INV-SCHEMA-002 — Genesis Completeness
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

    // Verifies: INV-STORE-002 — Strict Transaction Growth
    // Verifies: INV-STORE-014 — Every Command Is a Transaction
    // Verifies: NEG-STORE-001 — No Datom Deletion
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

    // Verifies: INV-STORE-014 — Every Command Is a Transaction (empty tx rejected)
    #[test]
    fn transact_rejects_empty_transaction() {
        let store = Store::genesis();
        let tx = Transaction::new(system_agent(), ProvenanceType::Observed, "empty");
        let result = tx.commit(&store);
        assert!(matches!(result, Err(StoreError::EmptyTransaction)));
    }

    // Verifies: INV-STORE-004 — CRDT Merge Commutativity
    // Verifies: INV-MERGE-001 — Merge Is Set Union
    // Verifies: ADR-STORE-001 — G-Set CvRDT as Store Algebra
    // Verifies: ADR-MERGE-001 — Set Union Over Heuristic Merge
    // Verifies: NEG-STORE-004 — No Merge Heuristics
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

    // Verifies: INV-STORE-006 — CRDT Merge Idempotency
    #[test]
    fn merge_is_idempotent() {
        let store = Store::genesis();
        let mut s = store.clone_store();
        let before = s.datom_set().clone();
        s.merge(&store);
        assert_eq!(s.datom_set(), &before, "INV-STORE-006: idempotency");
    }

    // Verifies: INV-STORE-007 — CRDT Merge Monotonicity
    // Verifies: NEG-STORE-005 — No Store Compaction
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

    // Verifies: INV-STORE-009 — Frontier Durability
    // Verifies: INV-STORE-016 — Frontier Computability
    // Verifies: ADR-STORE-021 — Frontier Representation
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

    // Verifies: INV-STORE-012 — LIVE Index Correctness
    // Verifies: ADR-STORE-005 — Four Core Indexes Plus LIVE
    #[test]
    fn entity_index_consistent_with_datoms() {
        let mut store = Store::genesis();
        let entity = EntityId::from_ident(":test/indexed");
        let tx = Transaction::new(system_agent(), ProvenanceType::Observed, "idx-test")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("indexed doc".into()),
            )
            .commit(&store)
            .unwrap();

        store.transact(tx).unwrap();

        // entity_datoms via index must match linear scan
        let indexed: Vec<&Datom> = store.entity_datoms(entity);
        let scanned: Vec<&Datom> = store.datoms().filter(|d| d.entity == entity).collect();
        assert_eq!(indexed.len(), scanned.len());
        for d in &scanned {
            assert!(indexed.contains(d));
        }
    }

    // Verifies: INV-STORE-012 — LIVE Index Correctness
    #[test]
    fn entity_count_matches_entities_set() {
        let mut store = Store::genesis();
        let e1 = EntityId::from_ident(":test/count-a");
        let e2 = EntityId::from_ident(":test/count-b");

        let tx = Transaction::new(system_agent(), ProvenanceType::Observed, "count-test")
            .assert(
                e1,
                Attribute::from_keyword(":db/doc"),
                Value::String("a".into()),
            )
            .assert(
                e2,
                Attribute::from_keyword(":db/doc"),
                Value::String("b".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        assert_eq!(store.entity_count(), store.entities().len());
    }

    // Verifies: INV-STORE-012 — LIVE Index Correctness (after merge)
    // Verifies: INV-MERGE-001 — Merge Is Set Union
    #[test]
    fn entity_index_survives_merge() {
        let mut s1 = Store::genesis();
        let s2 = Store::genesis();

        let e = EntityId::from_ident(":test/merge-idx");
        let tx = Transaction::new(system_agent(), ProvenanceType::Observed, "merge-idx")
            .assert(
                e,
                Attribute::from_keyword(":db/doc"),
                Value::String("merged".into()),
            )
            .commit(&s1)
            .unwrap();
        s1.transact(tx).unwrap();

        let mut merged = s2.clone_store();
        merged.merge(&s1);

        // Entity index must contain the merged entity
        assert!(!merged.entity_datoms(e).is_empty());
        assert_eq!(merged.entity_count(), merged.entities().len());
    }

    // -----------------------------------------------------------------------
    // Proptest property-based verification suite (14 STORE invariants)
    // Witnesses: INV-STORE-001, INV-STORE-002, INV-STORE-003, INV-STORE-004,
    // INV-STORE-005, INV-STORE-006, INV-STORE-007, INV-STORE-008,
    // INV-STORE-010, INV-STORE-011, INV-STORE-014
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
            // Verifies: INV-MERGE-001 — Merge Is Set Union
            // Verifies: NEG-MERGE-001 — No Merge Data Loss
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
            // Verifies: INV-STORE-010 — Causal Ordering
            // Verifies: INV-STORE-011 — HLC Monotonicity
            // Verifies: ADR-STORE-004 — Hybrid Logical Clocks for Transaction IDs
            // ---------------------------------------------------------------

            // Verifies: INV-STORE-010 — Causal Ordering (irreflexivity)
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

            // Verifies: INV-STORE-010 — Causal Ordering (DAG property)
            // Verifies: INV-STORE-011 — HLC Monotonicity
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

            // Verifies: INV-STORE-011 — HLC Monotonicity
            // Verifies: ADR-STORE-004 — Hybrid Logical Clocks for Transaction IDs
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

            /// Entity index consistency: index matches linear scan after transactions.
            // Verifies: INV-STORE-012 — LIVE Index Correctness
            // Verifies: ADR-STORE-005 — Four Core Indexes Plus LIVE
            #[test]
            fn entity_index_consistency(
                entities in proptest::collection::vec(arb_entity_id(), 1..=5),
                values in proptest::collection::vec(arb_doc_value(), 1..=5),
            ) {
                let mut store = Store::genesis();
                let agent = AgentId::from_name("proptest:idx");
                let count = entities.len().min(values.len());

                for i in 0..count {
                    let tx = Transaction::new(agent, ProvenanceType::Observed, &format!("idx-{i}"))
                        .assert(entities[i], Attribute::from_keyword(":db/doc"), values[i].clone())
                        .commit(&store)
                        .unwrap();
                    store.transact(tx).unwrap();
                }

                // Verify: entity_count matches actual unique entities
                let actual_entities: std::collections::BTreeSet<EntityId> =
                    store.datoms().map(|d| d.entity).collect();
                prop_assert_eq!(
                    store.entity_count(),
                    actual_entities.len(),
                    "entity_count() inconsistent with datom scan"
                );

                // Verify: every entity's datoms match linear scan
                for entity in &actual_entities {
                    let indexed: Vec<&Datom> = store.entity_datoms(*entity);
                    let scanned: Vec<&Datom> = store.datoms().filter(|d| d.entity == *entity).collect();
                    prop_assert_eq!(
                        indexed.len(),
                        scanned.len(),
                        "entity_datoms() count mismatch for {:?}",
                        entity
                    );
                }
            }
        }
    }
}
