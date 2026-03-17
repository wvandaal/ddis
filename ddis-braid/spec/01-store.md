> **Namespace**: STORE | **Wave**: 1 (Foundation) | **Stage**: 0
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §1. STORE — Datom Store

### §1.0 Overview

The datom store is the foundational substrate of Braid. All state — specification elements,
implementation facts, observations, decisions, provenance — lives as datoms in a single
append-only store. The store is a G-Set CvRDT: a grow-only set of datoms under set union.

**Traces to**: SEED.md §4, §11
**docs/design/ADRS.md sources**: FD-001–012, AS-001–010, SR-001–011, PD-001, PD-003–004, PO-001, PO-012

---

### §1.1 Level 0: Algebraic Specification

#### Definitions

```
Datom d = (e, a, v, tx, op)
  where e  : EntityId       — content-addressed entity identifier
        a  : Attribute       — keyword naming the property
        v  : Value           — the asserted value (polymorphic)
        tx : TxId            — transaction identifier (HLC timestamp)
        op : Operation       — assert | retract

D = the set of all possible datoms
Store S ∈ P(D)              — a store is a subset of all possible datoms
```

#### Identity Axiom

```
identity(d) = hash(d.e, d.a, d.v, d.tx, d.op)

∀ d₁, d₂ ∈ D:
  (d₁.e = d₂.e ∧ d₁.a = d₂.a ∧ d₁.v = d₂.v ∧ d₁.tx = d₂.tx ∧ d₁.op = d₂.op)
  ⟹ d₁ = d₂
```

Two agents independently asserting the same fact about the same entity in the same transaction
produce one datom. Identity is structural, not positional.

#### Store Algebra: (P(D), ∪)

The store forms a **join-semilattice** under set union:

```
L1 (Commutativity):   S₁ ∪ S₂ = S₂ ∪ S₁
L2 (Associativity):   (S₁ ∪ S₂) ∪ S₃ = S₁ ∪ (S₂ ∪ S₃)
L3 (Idempotency):     S ∪ S = S
L4 (Monotonicity):    S ⊆ S ∪ S'           for all S' ∈ P(D)
L5 (Growth-only):     |S(t+1)| ≥ |S(t)|    for all transitions t → t+1
```

**Proof**: L1–L3 hold by the definition of set union. L4 follows from L1–L3 (S ⊆ S ∪ S'
because S ∪ (S ∪ S') = S ∪ S' by L2, L3). L5 follows from L4: every transition is a union
with a non-empty set (the transaction datoms), so cardinality is non-decreasing.

**CRDT classification**: The store is a **G-Set CvRDT** (Grow-only Set, Convergent Replicated
Data Type). Strong eventual consistency follows from L1–L3: any two replicas that have received
the same set of updates are in the same state, regardless of delivery order.

#### Partial Order and Join-Semilattice Structure

The claim that `(P(D), ∪)` is a join-semilattice rests on an underlying partial order. This
section makes that partial order explicit and proves the semilattice structure.

**Definition (Partial Order).** The relation ⊆ (subset inclusion) on `P(D)` is defined by:

```
S₁ ⊆ S₂  ⟺  ∀ d ∈ S₁: d ∈ S₂
```

**Theorem PO-1.** `(P(D), ⊆)` is a partial order.

```
PO-1a (Reflexivity):      ∀ S ∈ P(D): S ⊆ S
  Proof: Every element of S is an element of S. □

PO-1b (Antisymmetry):     ∀ S₁, S₂ ∈ P(D): S₁ ⊆ S₂ ∧ S₂ ⊆ S₁ ⟹ S₁ = S₂
  Proof: If every element of S₁ is in S₂ and every element of S₂ is in S₁,
  then S₁ and S₂ have the same elements, so S₁ = S₂ by extensionality. □

PO-1c (Transitivity):     ∀ S₁, S₂, S₃ ∈ P(D): S₁ ⊆ S₂ ∧ S₂ ⊆ S₃ ⟹ S₁ ⊆ S₃
  Proof: Let d ∈ S₁. Then d ∈ S₂ (by S₁ ⊆ S₂), then d ∈ S₃ (by S₂ ⊆ S₃). □
```

**Theorem PO-2.** Set union ∪ is the join (least upper bound) under ⊆.

```
For all S₁, S₂ ∈ P(D), the set S₁ ∪ S₂ satisfies:

PO-2a (Upper bound):      S₁ ⊆ S₁ ∪ S₂  ∧  S₂ ⊆ S₁ ∪ S₂
  Proof: By definition of union, every element of S₁ is in S₁ ∪ S₂,
  and every element of S₂ is in S₁ ∪ S₂. □

PO-2b (Least upper bound): ∀ U ∈ P(D): (S₁ ⊆ U ∧ S₂ ⊆ U) ⟹ S₁ ∪ S₂ ⊆ U
  Proof: Let d ∈ S₁ ∪ S₂. Then d ∈ S₁ or d ∈ S₂. In either case, d ∈ U
  (by S₁ ⊆ U or S₂ ⊆ U respectively). So S₁ ∪ S₂ ⊆ U. □
```

**Corollary PO-3.** `(P(D), ⊆, ∪)` is a join-semilattice.

```
Proof: (P(D), ⊆) is a partial order (PO-1), and every pair of elements has a
join — namely their set union (PO-2). Therefore (P(D), ⊆, ∪) is a
join-semilattice. L1–L3 (commutativity, associativity, idempotency) follow
as properties of the join operation in any semilattice. □
```

**Distinguished Elements.**

```
Bottom element:  ∅ ∈ P(D)
  ∀ S ∈ P(D): ∅ ⊆ S  ∧  S ∪ ∅ = S
  The empty store is the identity element of the join operation.

  NOTE: The bottom element ∅ is NOT genesis. Genesis S₀ = {meta_schema_datoms}
  is the initial operational state, with S₀ ⊋ ∅. The algebraic bottom is the
  empty set; the operational bottom is genesis. These are distinct:
    ∅ is the algebraic identity (S ∪ ∅ = S for all S).
    S₀ is the smallest VALID store (contains the 19 axiomatic attributes).

Top element:     Does not exist.
  D is the set of all POSSIBLE datoms. Since D is constructed from unbounded
  domains (Value includes arbitrary strings, BigInt, etc.), D itself is
  unbounded — there is no finite set containing all possible datoms.
  Consequently P(D) has no top element.
  This is expected: a grow-only store in an open world has no maximal state.
```

**Connection to L4 and L5.** Laws L4 (monotonicity) and L5 (growth-only) are consequences
of the partial order:

```
L4: S ⊆ S ∪ S'
  This is PO-2a — the join is an upper bound of its arguments.
  Equivalently: every store transition via MERGE or TRANSACT moves UP in the
  partial order (P(D), ⊆). The store never moves down.

L5: |S(t+1)| ≥ |S(t)|
  If S(t) ⊆ S(t+1) (L4), then |S(t)| ≤ |S(t+1)| because subset inclusion
  implies cardinality inequality for finite sets. L5 is the cardinality shadow
  of L4. Strict growth (INV-STORE-002) holds because every transaction adds at
  least its tx_entity metadata datoms.
```

**Separation of Store and Resolution Layers.** Per-attribute resolution modes
(LWW, Lattice, Multi — see §4 RESOLUTION) operate on the LIVE index, not on
the store. This separation is critical to preserving the join-semilattice property:

```
Store layer:      (P(D), ⊆, ∪)  — pure join-semilattice, no resolution logic
LIVE layer:       LIVE(S) = fold(causal_sort(S), apply_resolution)

The LIVE function is a monotone function from the store semilattice to a
per-attribute value semilattice:
  S₁ ⊆ S₂ ⟹ LIVE(S₂) reflects all information in LIVE(S₁)
              (though resolved values may change as more datoms arrive)

Why this separation preserves the semilattice:
  1. MERGE(S₁, S₂) = S₁ ∪ S₂ — set union, no schema or resolution dependency
  2. Resolution modes are schema data, and schema lives IN the store (C3)
  3. If MERGE applied resolution, it would need to read schema from the store
     being merged — a circular dependency that breaks L1–L3
  4. By deferring resolution to LIVE, the store remains a pure G-Set CvRDT
     and the resolution layer composes on top without violating the algebra

See ADR-RESOLUTION-002 for the full decision record on resolution timing.
```

#### Transaction Algebra

```
Transaction T = (datoms: Set<Datom>, tx_entity: EntityId, provenance: ProvenanceType,
                 causal_predecessors: Set<TxId>, agent: AgentId, rationale: String)

TRANSACT : Store × Transaction → Store
TRANSACT(S, T) = S ∪ T.datoms ∪ {tx_datom}
  where tx_datom records T's metadata as datoms about T.tx_entity

∀ S, T: S ⊆ TRANSACT(S, T)                    — monotonicity
∀ S, T: |TRANSACT(S, T)| > |S|                 — strict growth (tx_entity adds at least one datom)
∀ S, T₁, T₂: TRANSACT(TRANSACT(S, T₁), T₂) is defined  — composability
```

#### Value Domain

```
Value = String | Keyword | Boolean | Long | Double | Instant | UUID
      | Ref EntityId | Bytes                                         // Stage 0 (9 variants)
      | URI | BigInt | BigDec                                        // Stage 1 (3 variants)
      | Tuple [Value] | Json String                                  // Stage 2 (2 variants — Tuple is recursive, Json requires parser)

ProvenanceType = Observed | Derived | Inferred | Hypothesized
  with ordering: Observed > Derived > Inferred > Hypothesized
  and provenance factors: Observed=1.0, Derived=0.8, Inferred=0.5, Hypothesized=0.2

Operation = Assert | Retract
```

---

### §1.2 Level 1: State Machine Specification

#### State

```
StoreState = {
  datoms:   Set<Datom>,                    — the append-only datom set
  frontier: Map<AgentId, TxId>,            — per-agent latest known transaction
  indexes:  { eavt, aevt, vaet, avet, live }  — materialized index views
}
```

#### Transitions

##### TRANSACT

```
TRANSACT(S, agent, datoms, tx_data) → S'

PRE:
  tx_data.causal_predecessors ⊆ known_txs(S)
  ∀ d ∈ datoms: d.a is a known attribute in S (schema validation)
  ∀ d ∈ datoms: typeof(d.v) matches schema_type(S, d.a)

POST:
  S'.datoms = S.datoms ∪ new_datoms ∪ tx_metadata_datoms
  S'.frontier[agent] = tx_id
  |S'.datoms| > |S.datoms|
  ∀ d ∈ S.datoms: d ∈ S'.datoms           — no datom removed

SIDE EFFECTS:
  All indexes updated incrementally
  LIVE index recomputed for affected entities
  Frontier durably persisted before response (INV-STORE-009)
```

##### MERGE

```
MERGE(S₁, S₂) → S'

POST:
  S' = S₁ ∪ S₂                            — set union, no heuristics
  ∀ d ∈ S₁: d ∈ S'
  ∀ d ∈ S₂: d ∈ S'
  |S'| ≤ |S₁| + |S₂|                      — dedup by content identity
  |S'| ≥ max(|S₁|, |S₂|)                  — at least as large as the larger input

INVARIANT: MERGE is commutative, associative, idempotent (L1–L3)
```

##### Genesis

```
GENESIS() → S₀

POST:
  S₀.datoms = {meta_schema_datoms} ∪ {system_agent_datoms}
  where:
    meta_schema_datoms = the 19 axiomatic attribute definitions
    system_agent_datoms = {
      (SYSTEM_AGENT, :agent/ident,   :system, tx_0, assert),
      (SYSTEM_AGENT, :agent/program, :braid,  tx_0, assert),
      (SYSTEM_AGENT, :agent/model,   :system, tx_0, assert),
    }
    SYSTEM_AGENT = BLAKE3("system" + "braid" + "genesis")

  S₀.frontier = { SYSTEM_AGENT: tx_0 }
  ∀ S₁, S₂ created by GENESIS: S₁ = S₂   — deterministic (constant hash)
  tx_0 has no causal predecessors
  tx_0.:tx/agent = SYSTEM_AGENT            — genesis tx references its own agent (INV-STORE-015)
```

#### Index Invariants

```
EAVT: sorted by (entity, attribute, value, tx)    — entity lookup
AEVT: sorted by (attribute, entity, value, tx)    — attribute-centric queries
VAET: sorted by (value, attribute, entity, tx)    — reverse reference traversal
AVET: sorted by (attribute, value, entity, tx)    — unique/range lookups

LIVE: materialized current-state view
  LIVE(S) = fold(causal_sort(S), apply_resolution)
  where apply_resolution uses the per-attribute resolution mode:
    LWW:     greatest HLC assertion (ties broken by BLAKE3 hash; ADR-RESOLUTION-009)
    Lattice: join over unretracted assertions
    Multi:   set of all unretracted values
```

---

### §1.3 Level 2: Interface Specification

#### Core Types (Rust)

```rust
/// A datom — the atomic unit of information.
/// Content-addressed: identity is the hash of all five fields.
#[derive(Clone, Hash, Eq, PartialEq)]
pub struct Datom {
    pub entity:    EntityId,
    pub attribute: Attribute,
    pub value:     Value,
    pub tx:        TxId,
    pub op:        Op,
}

/// Content-addressed entity identifier (INV-STORE-002).
/// Private inner field — construction only via EntityId::from_content().
#[derive(Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct EntityId([u8; 32]);  // BLAKE3 of content

impl EntityId {
    /// The ONLY constructor — hashes content with BLAKE3 (ADR-STORE-013).
    pub fn from_content(content: &[u8]) -> Self { EntityId(blake3::hash(content).into()) }
    /// Read-only access for serialization.
    pub fn as_bytes(&self) -> &[u8; 32] { &self.0 }
}

/// Hybrid Logical Clock — causally ordered, globally unique.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct TxId {
    pub wall_time: u64,    // milliseconds since epoch
    pub logical:   u32,    // logical counter for same-millisecond ordering
    pub agent:     AgentId,
}

/// Assert or retract.
#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub enum Op { Assert, Retract }
```

#### Typestate Transaction Lifecycle

```rust
/// Transaction states — enforced at compile time.
pub struct Building;
pub struct Committed;
pub struct Applied;

pub struct Transaction<S: TxState> {
    datoms:     Vec<Datom>,
    tx_data:    TxData,
    _state:     PhantomData<S>,
}

impl Transaction<Building> {
    pub fn new(agent: AgentId) -> Self;
    pub fn assert_datom(self, e: EntityId, a: Attribute, v: Value) -> Self;
    pub fn retract_datom(self, e: EntityId, a: Attribute, v: Value) -> Self;
    pub fn with_provenance(self, p: ProvenanceType) -> Self;
    pub fn with_causal_predecessors(self, preds: &[TxId]) -> Self;
    pub fn with_rationale(self, rationale: &str) -> Self;

    /// Validate and seal. Compile error if you try to apply without committing.
    pub fn commit(self, schema: &Schema) -> Result<Transaction<Committed>, TxValidationError>;
}

impl Transaction<Committed> {
    /// Apply to store. Cannot be called on Building state (type error).
    pub fn apply(self, store: &mut Store) -> Result<Transaction<Applied>, TxApplyError>;
}

impl Transaction<Applied> {
    pub fn tx_id(&self) -> TxId;
    pub fn receipt(&self) -> &TxReceipt;
}
// Compile error: Transaction<Building>.apply() — invalid state transition
// Compile error: Transaction<Applied>.assert_datom() — sealed

/// Receipt returned after a transaction is applied to the store.
/// Provides the assigned TxId and summary metadata for the caller.
/// Motivating invariant: INV-STORE-001, INV-STORE-002 (transact returns proof of growth).
pub struct TxReceipt {
    pub tx_id: TxId,            // HLC timestamp assigned to this transaction
    pub datom_count: usize,     // number of datoms added by this transaction
    pub new_entities: Vec<EntityId>, // entities created (first assertion for each)
}

/// Errors that prevent a Transaction<Building> from committing.
/// Each variant corresponds to a schema validation failure (INV-SCHEMA-004).
pub enum TxValidationError {
    /// Datom references an attribute not in the schema.
    UnknownAttribute(Attribute),
    /// Datom value type does not match schema-declared type.
    SchemaViolation { attr: Attribute, expected: ValueType, got: ValueType },
    /// Retraction targets a (entity, attribute) pair with no prior assertion.
    InvalidRetraction(EntityId, Attribute),
}

/// Errors that prevent a Transaction<Committed> from being applied to the store.
/// Distinct from TxValidationError: validation is schema-level, apply errors are
/// store-level (duplicate detection, storage failures).
pub enum TxApplyError {
    /// All datoms in this transaction are already present in the store
    /// (content-addressed deduplication, INV-STORE-003).
    DuplicateTransaction(TxId),
    /// Storage backend failure (e.g., filesystem write error).
    StorageFailure(String),
}

/// Map from agent to that agent's latest known transaction.
/// Used for frontier-scoped queries (INV-QUERY-007) and conservative conflict
/// detection (INV-RESOLUTION-003). Equivalent to a vector clock.
pub type Frontier = HashMap<AgentId, TxId>;

/// Current state of an entity as resolved by the LIVE index.
/// Returned by Store::current() — provides resolved attribute values
/// after applying per-attribute resolution modes (§4 RESOLUTION).
pub struct EntityView {
    pub entity: EntityId,
    pub attributes: HashMap<Attribute, Value>,  // resolved values (LIVE)
    pub as_of: TxId,                            // latest transaction affecting this entity
}

/// Snapshot of the store at a specific frontier.
/// Returned by Store::as_of() — provides a read-only view of the store
/// restricted to datoms visible at the given frontier.
pub struct SnapshotView<'a> {
    store: &'a Store,
    frontier: Frontier,
}

impl<'a> SnapshotView<'a> {
    /// Query the LIVE index for entity state at this snapshot's frontier.
    pub fn current(&self, entity: EntityId) -> EntityView;
    /// Count of datoms visible at this frontier.
    pub fn len(&self) -> usize;
}
```

#### Store API

```rust
pub struct Store {
    datoms: BTreeSet<Datom>,
    indexes: Indexes,
    frontier: HashMap<AgentId, TxId>,
    schema: Schema,
}

impl Store {
    /// Create a new store with genesis transaction.
    pub fn genesis() -> Self;

    /// Transact a committed transaction.
    pub fn transact(&mut self, tx: Transaction<Committed>) -> Result<TxReceipt, TxApplyError>;

    /// Query the LIVE index for current state of an entity.
    pub fn current(&self, entity: EntityId) -> EntityView;

    /// Query at a specific frontier.
    pub fn as_of(&self, frontier: &Frontier) -> SnapshotView;

    /// Datom count (monotonically non-decreasing).
    pub fn len(&self) -> usize;
}

// --- Free functions (ADR-ARCHITECTURE-001) ---

/// Merge another store into target (set union + cascade).
/// Free function: merge is a set-algebraic operation spanning the MERGE namespace.
/// See spec/07-merge.md for full cascade specification.
/// Returns (MergeReceipt, CascadeReceipt) — merge statistics and cascade effects.
pub fn merge(target: &mut Store, source: &Store) -> (MergeReceipt, CascadeReceipt);

/// Query the store using Datalog expressions.
/// Free function: query is a complex operation spanning the QUERY namespace
/// (stratum classification, FFI boundary, access log). Provenance recording
/// (INV-STORE-014) is handled by an explicit transact call within the
/// query function body, not by implicit Store mutation.
/// See spec/03-query.md for full query specification.
pub fn query(store: &Store, expr: &QueryExpr, mode: QueryMode) -> Result<QueryResult, QueryError>;
```

#### CLI Commands

```
braid transact --file <datoms.edn>    # Apply a transaction from file
braid transact --inline '<edn>'       # Apply inline transaction
braid status                          # Store summary: datom count, frontier, schema stats
braid entity <entity-id>              # Show all datoms for an entity
braid history <entity-id> <attr>      # Show all values of an attribute over time
```

---

### §1.4 Invariants

### INV-STORE-001: Append-Only Immutability

**Traces to**: SEED §4 Axiom 2, C1, ADRS FD-001
**Verification**: `V:TYPE`, `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ S ∈ Store, S' = TRANSACT(S, T) for any T:
  S ⊆ S'
  (monotonicity: once asserted, never lost)
```

#### Level 1 (State Invariant)
For all reachable states (S, S') where S →[op] S':
  `S.datoms ⊆ S'.datoms`

#### Level 2 (Implementation Contract)
```rust
#[kani::ensures(|result| old(store.datoms.len()) <= store.datoms.len())]
fn transact(store: &mut Store, tx: Transaction<Committed>) -> Result<TxReceipt, TxApplyError>;
```

**Falsification**: Any operation that reduces `store.datoms.len()` or removes a
previously-observed datom from the set.

**proptest strategy**: Generate random sequences of TRANSACT/RETRACT operations.
After each operation, verify all previously-observed datoms remain present.

---

### INV-STORE-002: Strict Transaction Growth

**Traces to**: SEED §4, ADRS PO-001
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ S, T: |TRANSACT(S, T)| > |S|
  (every transaction adds at least its tx_entity metadata datoms)
```

#### Level 1 (State Invariant)
For all transitions S →[TRANSACT(T)] S':
  `|S'.datoms| > |S.datoms|`

#### Level 2 (Implementation Contract)
```rust
#[kani::ensures(|result| old(store.len()) < store.len())]
fn transact(store: &mut Store, tx: Transaction<Committed>) -> Result<TxReceipt, TxApplyError>;
```

**Falsification**: A TRANSACT operation that leaves the store size unchanged.

**proptest strategy**: After every transact, assert `store.len() > pre_len`.

---

### INV-STORE-003: Content-Addressable Identity

**Traces to**: SEED §4 Axiom 1, C2, ADRS FD-007
**Verification**: `V:TYPE`, `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ d₁, d₂ ∈ D:
  (d₁.e, d₁.a, d₁.v, d₁.tx, d₁.op) = (d₂.e, d₂.a, d₂.v, d₂.tx, d₂.op)
  ⟺ d₁ = d₂
```

#### Level 1 (State Invariant)
For all reachable states S:
  No two distinct datoms in `S.datoms` have identical five-tuple values.

#### Level 2 (Implementation Contract)
```rust
// Enforced by BTreeSet/HashSet with (e, a, v, tx, op) as the key.
// Two insertions of the same five-tuple result in one stored datom.
impl Hash for Datom { /* hash all five fields */ }
impl Eq for Datom { /* compare all five fields */ }
```

**Falsification**: Two datoms with identical `(e, a, v, tx, op)` coexisting in the store
as distinct entries. Or: two datoms with different `(e, a, v, tx, op)` comparing as equal.

**proptest strategy**: Generate pairs of datoms with varying field equality. Verify Hash/Eq
consistency: equal datoms produce identical hashes; distinct datoms stored separately.

---

### INV-STORE-004: CRDT Merge Commutativity

**Traces to**: SEED §4 Axiom 2, C4, ADRS AS-001, L1
**Verification**: `V:PROP`, `V:KANI`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ S₁, S₂ ∈ Store: MERGE(S₁, S₂) = MERGE(S₂, S₁)
```

#### Level 1 (State Invariant)
For all reachable store pairs (S₁, S₂):
  `MERGE(S₁, S₂).datoms = MERGE(S₂, S₁).datoms`

#### Level 2 (Implementation Contract)
```rust
#[kani::ensures(|_| merge(s1, s2).datoms == merge(s2, s1).datoms)]
fn merge(s1: &Store, s2: &Store) -> Store;
```

**Falsification**: Any pair of stores where `MERGE(S₁, S₂) ≠ MERGE(S₂, S₁)`.

**proptest strategy**: Generate two random stores, merge in both orders, assert identical
datom sets. Run 100,000+ iterations with varying store sizes.

---

### INV-STORE-005: CRDT Merge Associativity

**Traces to**: SEED §4, C4, ADRS AS-001, L2
**Verification**: `V:PROP`, `V:KANI`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ S₁, S₂, S₃ ∈ Store: MERGE(MERGE(S₁, S₂), S₃) = MERGE(S₁, MERGE(S₂, S₃))
```

#### Level 1 (State Invariant)
For all reachable store triples:
  Merge order does not affect the final datom set.

#### Level 2 (Implementation Contract)
```rust
#[kani::ensures(|_| merge(&merge(s1, s2), s3).datoms == merge(s1, &merge(s2, s3)).datoms)]
```

**Falsification**: Any triple of stores where regrouping merge operations produces different results.

**proptest strategy**: Generate three random stores, merge in both groupings, assert equal.

---

### INV-STORE-006: CRDT Merge Idempotency

**Traces to**: SEED §4, C4, ADRS AS-001, PD-004, L3
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ S ∈ Store: MERGE(S, S) = S
```

#### Level 1 (State Invariant)
Merging a store with itself produces no change.

#### Level 2 (Implementation Contract)
```rust
#[kani::ensures(|_| merge(s, s).datoms == s.datoms)]
```

**Falsification**: A store where `MERGE(S, S)` differs from `S`.

**proptest strategy**: Generate random store, merge with self, assert datom sets equal.

---

### INV-STORE-007: CRDT Merge Monotonicity

**Traces to**: SEED §4, C4, L4
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ S₁, S₂ ∈ Store: S₁ ⊆ MERGE(S₁, S₂) ∧ S₂ ⊆ MERGE(S₁, S₂)
```

#### Level 1 (State Invariant)
Merging never loses datoms from either input.

#### Level 2 (Implementation Contract)
```rust
#[kani::ensures(|result| s1.datoms.is_subset(&result.datoms)
                      && s2.datoms.is_subset(&result.datoms))]
```

**Falsification**: Any datom present in S₁ or S₂ but absent from `MERGE(S₁, S₂)`.

**proptest strategy**: Generate two stores, merge, verify both inputs are subsets of result.

---

### INV-STORE-008: Genesis Determinism

**Traces to**: SEED §10, ADRS PO-012
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ S₁, S₂ created by GENESIS(): S₁ = S₂
  (genesis is a constant function)
```

#### Level 1 (State Invariant)
The genesis transaction installs exactly the 19 axiomatic meta-schema attributes.
No other datoms exist. The hash of the genesis datom set is a compile-time constant.

#### Level 2 (Implementation Contract)
```rust
const GENESIS_HASH: [u8; 32] = /* compile-time constant */;

#[kani::ensures(|result| hash(result.datoms) == GENESIS_HASH)]
fn genesis() -> Store;
```

**Falsification**: Two independently-created stores with different genesis datom sets.

**proptest strategy**: Create 1000 stores via `genesis()`, assert all have identical datom sets.

---

### INV-STORE-009: Frontier Durability

**Traces to**: SEED §4, ADRS PD-003
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ agent α, ∀ TRANSACT or MERGE operation:
  frontier(α) is durably stored BEFORE the operation returns
```

#### Level 1 (State Invariant)
On crash and recovery, the agent's frontier is recoverable from durable storage.
The recovered frontier is the frontier at the last completed operation.

#### Level 2 (Implementation Contract)
```rust
impl Store {
    fn transact(&mut self, tx: Transaction<Committed>) -> Result<TxReceipt, TxApplyError> {
        // ... apply datoms ...
        self.persist_frontier()?;  // fsync before returning
        Ok(receipt)
    }
}
```

**Falsification**: A crash after a successful TRANSACT where the frontier is lost,
causing the agent to replay already-committed transactions on recovery.

---

### INV-STORE-010: Causal Ordering

**Traces to**: SEED §4, ADRS PO-001, SR-004
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ T with causal_predecessors P:
  ∀ p ∈ P: p ∈ S                (predecessors exist in the store)

Causal order (happens-before) is defined by the predecessor set:
  T1 ->_causal T2  iff  T1.tx_id ∈ T2.causal_predecessors
  T1 <_causal T2   iff  transitive closure of ->_causal

HLC ordering is a CONSEQUENCE, not the DEFINITION:
  T1 <_causal T2  ==>  T1.tx_id <_hlc T2.tx_id   (HLC respects causality)
  T1.tx_id <_hlc T2.tx_id  =/=>  T1 <_causal T2   (HLC does NOT imply causality)

Two transactions are CAUSALLY INDEPENDENT (concurrent) iff:
  T1 || T2  iff  not(T1 <_causal T2) and not(T2 <_causal T1)

Causally independent transactions may have any HLC ordering -- they may even
share the same wall-clock time or have "reversed" HLC timestamps relative to
an external observer. HLC ordering among concurrent transactions is arbitrary
and MUST NOT be interpreted as causal precedence.
```

#### Level 1 (State Invariant)
A transaction's causal predecessors must all be present in the store at the time of
the transaction. HLC timestamps are monotonically increasing per agent (INV-STORE-011),
which ensures that within a single agent's history, HLC order and causal order coincide.
Across agents, however, HLC ordering is a convenience for display, time-travel queries,
and LWW tie-breaking -- it does NOT establish causality. Only the explicit predecessor
set defines the happens-before relation.

**Clarification (R2.3)**: The conflict predicate in §4 RESOLUTION (INV-RESOLUTION-004)
requires causal independence, which is determined by predecessor sets, not by HLC
comparison. Two transactions T1, T2 are causally independent iff neither is transitively
reachable from the other via the predecessor graph. HLC comparison alone is insufficient:
`T1.tx_id <_hlc T2.tx_id` does not mean T1 happened before T2 in the causal sense --
T2's agent may simply have a faster wall clock.

#### Level 2 (Implementation Contract)
```rust
impl Transaction<Building> {
    pub fn commit(self, schema: &Schema) -> Result<Transaction<Committed>, TxValidationError> {
        for pred in &self.tx_data.causal_predecessors {
            if !schema.store_contains_tx(pred) {
                return Err(TxValidationError::MissingCausalPredecessor(*pred));
            }
        }
        // ...
    }
}

/// Causal independence check -- used by the conflict predicate.
/// Determined by predecessor graph reachability, NOT by HLC comparison.
fn causally_independent(store: &Store, tx1: TxId, tx2: TxId) -> bool {
    !store.is_causal_ancestor(tx1, tx2) && !store.is_causal_ancestor(tx2, tx1)
}

/// Transitive closure over causal_predecessors.
fn is_causal_ancestor(store: &Store, ancestor: TxId, descendant: TxId) -> bool {
    // BFS/DFS over the predecessor graph from descendant back to ancestor.
    let mut visited = HashSet::new();
    let mut queue = VecDeque::from(store.causal_predecessors(descendant));
    while let Some(pred) = queue.pop_front() {
        if pred == ancestor { return true; }
        if visited.insert(pred) {
            queue.extend(store.causal_predecessors(pred));
        }
    }
    false
}
```

**Falsification**: A transaction referencing a causal predecessor that does not exist in the store.
Also: any code path that uses `tx1.tx_id <_hlc tx2.tx_id` as a proxy for causal precedence
across different agents, rather than consulting the predecessor graph.

**proptest strategy**: Generate transaction chains with random predecessor references.
Verify that commits with invalid predecessors are rejected. Additionally, generate pairs
of transactions from different agents with arbitrary HLC orderings and verify that
`causally_independent` returns true iff neither is reachable from the other in the
predecessor graph, regardless of HLC ordering.

---

### INV-STORE-011: HLC Monotonicity

**Traces to**: ADRS SR-004
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ transactions T₁, T₂ from agent α where T₁ precedes T₂:
  T₁.tx_id < T₂.tx_id
```

#### Level 1 (State Invariant)
An agent's transaction IDs are strictly monotonically increasing.
The HLC combines wall-clock time with a logical counter to ensure uniqueness
even when wall-clock resolution is insufficient.

#### Level 2 (Implementation Contract)
```rust
impl HlcClock {
    pub fn tick(&mut self) -> TxId {
        let now = wall_time();
        if now > self.last.wall_time {
            self.last = TxId { wall_time: now, logical: 0, agent: self.agent };
        } else {
            self.last.logical += 1;
        }
        self.last
    }
}
```

**Falsification**: Two transactions from the same agent with the same or decreasing TxId.

---

### INV-STORE-012: LIVE Index Correctness

**Traces to**: ADRS SR-002
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
LIVE(S) = fold(causal_sort(S), apply_resolution)

∀ entity e, attribute a with cardinality :one:
  LIVE(S, e, a) = resolution_mode(a).resolve(
    {d.v | d ∈ S, d.e = e, d.a = a, d.op = Assert,
           ¬∃ r ∈ S: r.e = e, r.a = a, r.v = d.v, r.op = Retract, r.tx > d.tx}
  )
```

#### Level 1 (State Invariant)
The LIVE index is the deterministic result of applying all assert and retract datoms
in causal order with the declared resolution mode per attribute.

#### Level 2 (Implementation Contract)
```rust
impl LiveIndex {
    /// Incrementally update after a transaction.
    pub fn apply_tx(&mut self, tx: &[Datom], schema: &Schema);

    /// Full recompute from scratch (for verification).
    pub fn recompute(datoms: &BTreeSet<Datom>, schema: &Schema) -> Self;
}

// Verification: incremental update produces same result as full recompute
#[cfg(test)]
fn verify_live_consistency(store: &Store) {
    let incremental = &store.indexes.live;
    let full_recompute = LiveIndex::recompute(&store.datoms, &store.schema);
    assert_eq!(incremental, &full_recompute);
}
```

**Falsification**: LIVE shows a value whose retraction has no subsequent re-assertion,
or LIVE differs from a full recompute.

**proptest strategy**: Generate random transaction sequences with asserts and retracts.
After each transaction, verify `incremental_live == full_recompute_live`.

---

### INV-STORE-013: Working Set Isolation

**Traces to**: SEED §4, ADRS PD-001
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
∀ agents α, β where α ≠ β:
  W_α ∩ visible(β) = ∅
  (agent α's working set is invisible to agent β)

commit : W_α × S → S'
  where S' = S ∪ {d ∈ W_α | agent chose to commit d}
  and post-commit: d ∉ W_α for committed datoms
```

#### Level 1 (State Invariant)
Uncommitted working set datoms are local to one agent. MERGE operations do not
include working set datoms. Only explicit `commit` promotes W_α datoms to the shared store.

#### Level 2 (Implementation Contract)
```rust
pub struct WorkingSet {
    local_datoms: BTreeSet<Datom>,
    agent: AgentId,
}

impl WorkingSet {
    /// Query sees W_α ∪ S (local override).
    pub fn query_view<'a>(&'a self, store: &'a Store) -> MergedView<'a>;

    /// Promote selected datoms to the shared store.
    pub fn commit(&mut self, store: &mut Store, datoms: &[Datom]) -> Result<TxReceipt, TxApplyError>;
}
```

**Falsification**: Agent β can query and observe datoms from Agent α's working set
before α has committed them.

---

### INV-STORE-014: Every Command Is a Transaction

**Traces to**: SEED §10, ADRS FD-012
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ DDIS commands C: ∃ provenance record P such that executing C produces P

Provenance scope depends on command type:
  Mutating commands (transact, harvest, merge):
    P is a transaction in the shared store S — globally visible after merge.
  Read commands (query, status, seed, guidance):
    P is a local record in the agent's working set W_α — NOT included
    in merge operations. This preserves CALM compliance (INV-QUERY-001):
    the shared store sees no mutation from read commands.
```

#### Level 1 (State Invariant)
Every CLI command generates a provenance record. For mutating commands, this
is a transaction in the shared store (new datoms recording who, what, when, why).
For read commands, provenance is recorded in the agent's working set (W_α,
INV-STORE-013) as local datoms that are not promoted to the shared store
during merge operations. This ensures that queries remain pure reads from
the shared store's perspective, preserving CALM compliance and monotonic
read guarantees (INV-QUERY-001).

Read-command provenance in W_α serves debugging and audit purposes — an agent
can inspect its own query history — but does not pollute the shared store
with non-convergent side effects.

#### Level 2 (Implementation Contract)
```rust
/// Read commands produce provenance in W_α (local working set), not the shared store.
/// This preserves CALM compliance: the shared store sees no mutation from queries.
/// The caller records provenance in their working set via WorkingSet::record_provenance().
pub fn query(store: &Store, q: &QueryExpr, mode: QueryMode) -> QueryResult {
    let result = evaluate(store, q, mode);
    // Provenance: caller records in W_α, NOT in store.
    // WorkingSet::record_provenance(query_metadata) — local only.
    result
}

/// Mutating commands produce provenance in the shared store.
pub fn transact(store: &mut Store, datoms: Vec<Datom>) -> TxReceipt {
    // Transaction includes provenance datoms in the shared store.
    store.apply(datoms) // provenance is part of the transaction
}
```

**Falsification**: A mutating command (transact, harvest, merge) completes without
producing a transaction record in the shared store, OR a read command (query, status,
seed, guidance) mutates the shared store as a side effect of provenance recording.

---

### INV-STORE-015: Agent Entity Completeness

**Traces to**: SEED §4 (Axiom 1), exploration/03-topology-definition.md §1, ADR-STORE-020
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Every :tx/agent Ref in the store points to a valid agent entity:
  ∀ d = (_, :tx/agent, agent_ref, _, _) ∈ S:
    ∃ d' = (agent_ref, :agent/ident, _, _, assert) ∈ S
```

#### Level 1 (State Invariant)
After genesis, at least SYSTEM_AGENT exists:
  |{e | (e, :agent/ident, _, _, assert) ∈ S₀}| >= 1

Every transaction's `:tx/agent` Ref resolves to an entity with at minimum
`:agent/ident`, `:agent/program`, and `:agent/model` attributes.

#### Level 2 (Implementation Contract)
```rust
/// Store::transact() ensures that if :tx/agent references an EntityId not yet
/// in the store, the transaction ALSO includes assertion datoms creating that
/// agent entity with :agent/ident, :agent/program, :agent/model.
fn ensure_agent_entity(store: &Store, tx: &mut Transaction<Building>, agent_id: EntityId) {
    if store.current(agent_id).attributes.is_empty() {
        // Agent entity does not exist — add creation datoms to this transaction.
        // The caller must provide program, model, session_id.
    }
}
```

**Falsification**: A `:tx/agent` Ref that does not resolve to an entity with
`:agent/ident` in the store.

**proptest strategy**: Generate random transaction sequences with varying agent
IDs. Assert every `:tx/agent` ref resolves to an agent entity.

---

### INV-STORE-016: Frontier Computability

**Traces to**: SEED §4 (Axiom 3), exploration/07-fitness-function.md §3.3, ADR-STORE-021
**Verification**: `V:PROP`
**Stage**: 0 (definition); 3 (multi-agent usage)

#### Level 0 (Algebraic Law)
```
For any agent α and store S:
  staleness(α, S) = |{β ∈ agents(S) | frontier(α)[β] < latest(β, S)}| / |agents(S)|

staleness(α, S) ∈ [0, 1]
staleness(α, S) = 0 ⟺ α is fully up-to-date with all agents

Frontier representation is a per-agent vector clock (ADR-STORE-021):
  frontier(α) = {(β, max_tx_β) | β ∈ agents, max_tx_β = latest tx from β seen by α}
```

#### Level 1 (State Invariant)
In single-agent mode (Stage 0):
  |agents(S)| = 1
  frontier(self)[self] = latest(self, S)  — always
  staleness(self, S) = 0                  — always

#### Level 2 (Implementation Contract)
```rust
/// Frontier staleness is computable via a Datalog query at Stratum 0 (monotonic,
/// coordination-free). Each frontier entry is a compound entity with
/// :frontier/agent (Ref) and :frontier/tx (Ref).
pub fn staleness(store: &Store, agent: EntityId) -> f64 {
    let agents = all_agents(store);
    if agents.len() <= 1 { return 0.0; }
    let frontier = agent_frontier(store, agent);
    let stale_count = agents.iter()
        .filter(|b| frontier.get(b).map_or(true, |tx| *tx < latest_tx(store, **b)))
        .count();
    stale_count as f64 / agents.len() as f64
}
```

**Falsification**: staleness(α, S) requires a non-monotonic query (Stratum 2+)
or cannot be expressed in Datalog.

**proptest strategy**: In single-agent mode, staleness is always 0. In simulated
multi-agent mode, staleness correctly reflects the gap between an agent's frontier
and the global state.

---

### §1.5 ADRs

### ADR-STORE-001: G-Set CvRDT as Store Algebra

**Traces to**: SEED §4 Axiom 2, ADRS AS-001
**Stage**: 0

#### Problem
What CRDT type should the datom store use?

#### Options
A) **G-Set (grow-only set)** — simplest CRDT with set union merge. Content-addressable identity
   ensures deduplication. Preserves L1–L5 by construction.
B) **OR-Set** — supports element removal. More complex merge (requires unique tags per add).
   Would enable true deletion, violating C1.
C) **2P-Set** — separate add/remove sets. Once removed, never re-assertable.
   Gives any agent permanent veto power.
D) **Custom merge logic** — application-specific heuristics. Breaks formal CRDT guarantees.

#### Decision
**Option A.** G-Set is the minimal CRDT that provides strong eventual consistency under set
union. Retractions are modeled as new datoms with `op=Retract`, preserving append-only
semantics. The "current state" is a query-layer concern (LIVE index), not a store-layer concern.

#### Formal Justification
G-Set preserves L1–L5 by the definition of set union. Options B–D either violate C1 (append-only),
introduce complexity without algebraic benefit, or break formal guarantees.

#### Consequences
- Merge is always safe — set union cannot produce invalid state
- The store never shrinks — storage is monotonically consumed
- "Deletion" is modeled as retraction (a new fact), preserving full history
- Conflict resolution is deferred to the query layer (RESOLUTION namespace)

#### Falsification
Evidence that G-Set is insufficient would be: a required operation that cannot be expressed
as a new datom assertion, requiring actual removal of existing datoms from the set.

---

### ADR-STORE-002: EAV Over Relational

**Traces to**: SEED §4, §11, ADRS FD-002
**Stage**: 0

#### Problem
What data model should the datom store use?

#### Options
A) **EAV (Entity-Attribute-Value)** — schema-on-read. Structure determined at query time.
   No migrations. Schema crystallizes from usage.
B) **Relational tables** — schema-on-write. Structure determined at design time.
   Requires DDL migrations for evolution.
C) **Document store** — schema-per-document. Poor graph traversal.

#### Decision
**Option A.** EAV aligns with how AI agents actually work: they discover the structure of
the problem as they explore it. Early in development, you don't know what entity types
you'll need. With EAV, you assert facts and the schema crystallizes from usage.

#### Formal Justification
EAV preserves C3 (schema-as-data): the schema is itself a collection of datoms, not a
separate DDL. Relational tables (Option B) would require the Go CLI's approach — 39
`CREATE TABLE` statements in Go source, each a potential divergence point.

#### Consequences
- Schema evolution is a transaction, not a migration
- All queries go through the same EAV access path
- Query performance depends on indexing strategy (four core indexes + LIVE)
- The Datalog query engine is natural (EAV triples are Datalog's native data model)

---

### ADR-STORE-003: Content-Addressable Entity IDs

**Traces to**: SEED §4 Axiom 1, ADRS FD-007
**Stage**: 0

#### Problem
How are entities identified?

#### Options
A) **Content-addressed** — EntityId = hash of semantic content. Same fact = same entity.
B) **Sequential integers** — auto-incrementing per agent. Requires ID mapping during merge.
C) **Random UUIDs** — globally unique but no deduplication. Same fact gets different IDs.

#### Decision
**Option A.** Content-addressable identity eliminates the "same fact, different ID" problem
in multi-agent settings. When two agents independently assert the same fact, the datoms
have the same EntityId and naturally deduplicate during merge (set union).

#### Formal Justification
Content-addressable identity is what makes set-union merge (L1–L3) produce correct results.
With sequential IDs (Option B), merge would need an ID remapping step that could introduce
errors. With random UUIDs (Option C), merge would preserve duplicates.

#### Consequences
- EntityId computation is deterministic and reproducible
- Two agents asserting "entity X has attribute A with value V" produce identical datoms
- Merge is pure set union with no post-processing
- Entity lookup requires either the content or a known EntityId (no sequential scan)

---

### ADR-STORE-004: Hybrid Logical Clocks for Transaction IDs

**Traces to**: SEED §4, ADRS SR-004
**Stage**: 0

#### Problem
How are transactions ordered?

#### Options
A) **Hybrid Logical Clocks (HLC)** — combines physical wall-clock time with logical counter.
   Causally ordered, globally unique without central coordination.
B) **Sequential integers** — simple but requires centralized counter. Conflicts across agents.
C) **Lamport clocks** — causal ordering but no wall-clock correlation. Cannot answer
   "what was true at 3pm?"
D) **UUIDs** — unique but unordered. No temporal queries.

#### Decision
**Option A.** HLC preserves temporal ordering (critical for time-travel queries) while
maintaining uniqueness across agents without centralized coordination. The agent field
in the TxId breaks ties.

#### Formal Justification
HLC satisfies: (1) monotonicity within an agent (INV-STORE-011), (2) causal consistency
(if T₁ causally precedes T₂, then T₁.tx_id < T₂.tx_id), (3) wall-clock approximation
(for human-readable time queries), (4) no central coordination (agents generate independently).

---

### ADR-STORE-005: Four Core Indexes Plus LIVE

**Traces to**: SEED §4, ADRS SR-001, SR-002
**Stage**: 0

#### Problem
What indexes should the store maintain?

#### Options
A) **Four Datomic indexes (EAVT, AEVT, VAET, AVET) + LIVE materialized view** —
   covers all query patterns. LIVE provides O(1) current-state lookup.
B) **Single index with secondary lookups** — simpler but poor query performance.
C) **Ad-hoc indexes per query** — unpredictable performance, index explosion.

#### Decision
**Option A.** Datomic's four-index design is proven at scale. The LIVE index (SR-002)
is added as a fifth index that materializes the current-state view, eliminating the need
for stratified negation in the most common query pattern.

#### Formal Justification
Each index covers a different access pattern: EAVT (entity lookup), AEVT (attribute-centric),
VAET (reverse reference traversal), AVET (unique/range). LIVE handles the non-monotonic
"current value" query without requiring Datalog negation, keeping common queries in the
CALM-compliant monotonic fragment.

---

### ADR-STORE-006: Embedded Deployment

**Traces to**: SEED §4, ADRS FD-010
**Stage**: 0

#### Problem
How is the datom store deployed?

#### Options
A) **Embedded library** — no daemon process. Agents invoke as CLI or link as library.
B) **Client-server database** — separate database server.
C) **Distributed database** — multi-node with consensus.

#### Decision
**Option A.** For the target deployment (single VPS, single-digit agents, thousands of
datoms), embedded is sufficient. Minimizes operational complexity. Multiple agents coordinate
through the shared filesystem via content-addressed transaction files (SR-014; SR-007 flock coordination superseded by ADR-LAYOUT-006).

#### Formal Justification
Embedded deployment preserves the property that all coordination goes through the store.
A separate database server (Option B) introduces a coordination channel outside the datom
store, potentially violating FD-012 (every command is a transaction).

---

### ADR-STORE-007: File-Backed Store with Git

> **SUPERSEDED** by ADR-LAYOUT-001 (per-transaction files) and ADR-LAYOUT-005 (pure filesystem)
> in [spec/01b-storage-layout.md](01b-storage-layout.md). Both Option A (trunk.ednl) and
> Option B (redb target) are replaced by content-addressed per-transaction files with
> directory-union merge. See SR-014 in docs/design/ADRS.md.

**Traces to**: ADRS SR-006, SR-007
**Stage**: 0

#### Problem
What is the physical storage format?

#### Options
A) **Append-only files with git** — `trunk.ednl`, `branches/{name}.ednl`, git for history.
B) **LMDB/redb** — MVCC storage with B-tree indexes.
C) **SQLite** — proven, familiar, but mutable (UPDATE/DELETE).

#### Decision
**Option A for initial implementation, Option B as target.** The file-backed approach enables
immediate bootstrapping (shell tools can manipulate the store). The target architecture uses
redb (Rust-native MVCC) for production performance. SQLite is rejected as the store backend
because its mutable semantics conflict with C1, though it could serve as an index cache.

#### Formal Justification
Append-only files naturally enforce C1 (no mutation). Git provides audit history and
time-travel. The file format (EDNL — EDN per line) is human-readable and grep-able,
supporting the shell bootstrap phase (SR-005).

---

### ADR-STORE-008: Provenance Typing Lattice

**Traces to**: ADRS PD-002
**Stage**: 0

#### Problem
How are different epistemic statuses of transactions distinguished?

#### Options
A) **No provenance typing** — all transactions equal. Rely on challenge system post-hoc.
B) **Provenance typing lattice** — `:observed > :derived > :inferred > :hypothesized`.
   Transaction declares its provenance; system can audit the declaration.
C) **Structural inference only** — imperfect classification without agent declaration.

#### Decision
**Option B.** Provenance factors feed into authority computation (UA-003): `:observed` = 1.0,
`:derived` = 0.8, `:inferred` = 0.5, `:hypothesized` = 0.2. A transaction labeled `:observed`
without corresponding tool read operations is flagged as misclassified.

#### Formal Justification
Provenance typing enables differential trust. Self-authored associations (`:inferred`) carry
less weight than direct observations (`:observed`). This feeds into spectral authority
computation (UA-003) without requiring post-hoc challenge for every assertion.

---

### ADR-STORE-009: Crash-Recovery Model

**Traces to**: ADRS PD-003
**Stage**: 0

#### Problem
What failure model do agents follow?

#### Options
A) **Crash-stop** — once crashed, never recovers. Too rigid for LLM agents.
B) **Crash-recovery** — agent recovers from last durable frontier.
C) **Byzantine** — assumes agents may be actively malicious. Overkill for controlled environment.

#### Decision
**Option B.** Crash-recovery matches the reality of LLM agents: conversations end (crash),
new conversations begin (recovery). The agent announces its frontier on recovery and receives
the delta of new datoms.

#### Formal Justification
Crash-recovery requires INV-STORE-009 (frontier durability). Combined with at-least-once
delivery (PD-004) and idempotent operations, an agent that crashes mid-transaction can
safely replay or discard without corrupting the store.

---

### ADR-STORE-010: At-Least-Once Delivery

**Traces to**: ADRS PD-004
**Stage**: 0

#### Problem
What delivery semantics for inter-agent communication?

#### Options
A) **At-most-once** — messages may be lost. Risks data loss.
B) **At-least-once** — messages may be duplicated. All operations must be idempotent.
C) **Exactly-once** — requires 2PC or equivalent. Expensive coordination.

#### Decision
**Option B.** Idempotent operations (MERGE is idempotent by L3; TRANSACT with content-addressed
identity is idempotent) make at-least-once delivery safe. The SYNC-BARRIER operation
requires stronger guarantees but is explicitly rare.

#### Formal Justification
```
merge(merge(S, R), R) = merge(S, R)    — by L3 (idempotency)
```
Duplicate message delivery produces the same result as single delivery.

---

### ADR-STORE-011: Every Command as Transaction

**Traces to**: SEED §10, ADRS FD-012
**Stage**: 0

#### Problem
Should read-only operations bypass the store?

#### Options
A) **Read-only bypass** — queries don't produce transactions. Simpler, faster.
B) **Every command is a transaction** — including queries. Full provenance trail.

#### Decision
**Option B.** If any DDIS operation produces state outside the store, that state cannot be
queried, conflict-detected, or coherence-verified. The store is the sole truth. Query
provenance enables "what was this agent curious about?" — useful for ASSOCIATE significance
weighting and drift detection.

#### Formal Justification
Violating this principle creates a blind spot in the coherence verification framework.
The cost (small metadata transactions for queries) is amortized by the value (full
provenance, significance computation, drift detection).

---

### ADR-STORE-012: Three-Phase Implementation Path

**Traces to**: ADRS SR-005
**Stage**: 0

#### Problem
How to bootstrap the query engine when the system doesn't exist yet?

#### Options
A) **Start directly with Rust binary** — most efficient but no system to build with.
B) **Shell tools → SQLite → Rust binary** — three substitutable implementations.
C) **Single intermediate step** — shell tools → Rust binary.

#### Decision
**Option B.** Three-phase: (a) shell tools (grep/jq + Python) for immediate bootstrap,
(b) SQLite with EAV schema as intermediate, (c) Rust binary as final target. All three
implementations are substitutable (same protocol interface, tested against same invariants).

#### Formal Justification
The bootstrapping problem: you need the system to build the system. Shell tools enable
immediate use of harvest/seed before the store exists. The invariants (this specification)
constrain all three implementations identically.

---

### ADR-STORE-013: BLAKE3 for Content Hashing

**Traces to**: SEED §4 Axiom 1, ADRS FD-007, FD-013
**Stage**: 0

#### Problem
Which hash algorithm should be used for content-addressable identity (EntityId generation,
datom identity, genesis hash)?

#### Options
A) **BLAKE3** — 256-bit output, ~14x faster than SHA-256, pure Rust crate, tree-hashable,
   streaming API, designed for content-addressed systems (IPFS, Bao).
B) **SHA-256** — ubiquitous, FIPS-certified, but slower and requires C dependency (`ring`
   or `openssl`) for optimal performance.
C) **BLAKE2b** — predecessor to BLAKE3, fast, well-audited, but less optimized tree structure,
   no SIMD auto-detection.
D) **xxHash** — fastest non-cryptographic hash, but insufficient collision resistance for
   content-addressed identity where collisions cause silent data corruption.

#### Decision
**Option A.** BLAKE3 provides the optimal balance of speed, collision resistance (2^{-128}
birthday bound), and ecosystem fit. Performance matters because every datom insertion, every
entity lookup, and every merge deduplication involves hashing.

#### Formal Justification
Content-addressable identity (C2, ADR-STORE-003) requires collision resistance: a collision
means two different facts share an EntityId, causing silent data loss. xxHash (Option D) is
non-cryptographic and unacceptable. Among cryptographic options, BLAKE3 is fastest in pure
Rust (no C dependency), ships with tree-hashing (future parallelism for large values), and
is the only option where the performance overhead of hashing is negligible relative to I/O.

#### Consequences
- `blake3` crate dependency (pure Rust, no `unsafe`, actively maintained)
- 32-byte (`[u8; 32]`) hash output everywhere EntityId appears
- Not FIPS-certified (acceptable — Braid is not a compliance-regulated system)
- Consistent with ADR-STORE-003 (content-addressable) and ADR-STORE-011 (HLC timestamps)

---

### ADR-STORE-014: Private EntityId Inner Field

**Traces to**: SEED §4 Axiom 1, C2, INV-STORE-002, ADRS FD-014
**Stage**: 0

#### Problem
Should `EntityId`'s inner `[u8; 32]` field be `pub` or private?

#### Options
A) **Private** — construction only via `EntityId::from_content()`. Read access via
   `as_bytes()`. Enforces INV-STORE-002 at compile time.
B) **Public** — `EntityId(pub [u8; 32])`. Simpler, allows pattern matching and
   direct construction from arbitrary bytes.

#### Decision
**Option A.** Private inner field. Content-addressable identity (C2) means EntityIds
must correspond to actual content hashes. A public constructor allows creating
EntityIds from arbitrary bytes, bypassing the hash — a type-level violation of C2.

#### Consequences
- `EntityId::from_content(content)` is the sole constructor (hashes with BLAKE3)
- `EntityId::as_bytes()` provides read-only access for serialization
- Deserialization from storage uses a `pub(crate)` constructor (trusted boundary)
- INV-STORE-002 is enforced at compile time with zero runtime cost

---

### ADR-STORE-015: Free Functions Over Store Methods for Namespace Operations

**Traces to**: SEED.md §4, §10, ADRS FD-010, FD-012
**Stage**: 0

#### Problem
Should namespace operations (query, harvest, seed, merge, guidance, derivation, routing,
drift detection) be implemented as Store methods or as free functions taking `&Store` /
`&mut Store`?

#### Options
A) **Free functions** — `query(store, expr, mode)`, `harvest_pipeline(store, session)`,
   `assemble_seed(store, task, budget)`, `merge(target, source)`. Store methods reserved
   for core datom operations: `genesis()`, `transact()`, `current()`, `as_of()`, `len()`,
   `datoms()`, `frontier()`, `schema()`.
B) **Store methods** — `store.query(expr)`, `store.harvest(session)`, `store.seed(task)`.
   All operations as methods on Store.
C) **Trait-based** — `Harvestable`, `Seedable`, `Queryable` traits implemented on Store.

#### Decision
**Option A.** Free functions for all namespace operations. Store methods are limited to:
constructors (`genesis`), core mutation (`transact`), and trivial read accessors
(`current`, `as_of`, `len`, `datoms`, `frontier`, `schema`). Everything else is a free
function in its respective namespace module.

#### Formal Justification
1. **Keeps Store lean**: Store is the foundational abstraction. Methods for every namespace
   would make Store a God-object growing with every new feature.
2. **Enables independent testing**: Free functions can be tested with minimal Store fixtures.
3. **Matches Rust idioms**: Namespace operations need only Store's public API. Free functions
   make this dependency explicit.
4. **Consistent with guide convention**: All guide files already use free functions.

#### Consequences
- Store's `impl` block contains only: `genesis()`, `transact()`, `current()`, `as_of()`,
  `len()`, `datoms()`, `frontier()`, `schema()`.
- Adding a new namespace never requires modifying Store's API surface.

#### Falsification
Evidence this decision is wrong: a namespace operation that requires access to Store's
private fields and cannot be expressed through Store's public API. In that case, the
Store API needs a new public method, not a namespace method on Store.

---

### ADR-STORE-016: ArcSwap MVCC Concurrency Model

**Traces to**: SEED §4 (immutable store values), §10 (Stage 3 multi-agent), ADRS SR-003, C1, C4
**Type**: ADR
**Stage**: 0 (core infrastructure; critical at Stage 3)

#### Problem

How do concurrent readers and a single writer access the Store? The Store is immutable
after construction (C1), but write operations (transact, harvest, merge) produce new
Store values that must be visible to subsequent readers without blocking in-flight reads.

#### Options

A) **`Arc<RwLock<Store>>`** — Correct but high contention. Readers acquire read locks;
   the writer acquires a write lock. Under read-heavy workloads (MCP tool handlers, seed
   assembly, query evaluation), write operations (transact, harvest) are blocked by
   concurrent readers. Degrades to serial execution under load.

B) **Actor model** — All Store access goes through a message queue to a single actor.
   Serializes all operations. Simple but eliminates read concurrency entirely — every
   query waits behind every other query and every write.

C) **`ArcSwap<Store>`** — Lock-free reads, atomic pointer swap for writes. Readers call
   `store.load()` to get an `Arc<Store>` snapshot — zero contention, zero blocking.
   The writer constructs a new Store value, then atomically swaps the pointer via
   `store.swap(new_store)`. In-flight readers continue with their snapshot; new readers
   see the updated Store.

#### Decision

**Option C.** Store values are immutable (C1). `ArcSwap` provides snapshot isolation
naturally: each `load()` returns a consistent, immutable `Arc<Store>` that cannot be
invalidated by subsequent writes. The writer never needs to wait for readers to finish.

Schema inherits this concurrency model because it is owned by Store (ADR-SCHEMA-005,
Option C). Each MVCC snapshot contains a Schema consistent with its datoms — no split-brain
possible. See ADR-SCHEMA-005 Stage 3 Concurrency Analysis for Option B rejection rationale.

#### Formal Justification

1. G-Set CvRDT (INV-STORE-003) + C1 immutability → `&Store` is always safe to read.
2. `ArcSwap` only swaps the pointer, never the pointed-to data → readers never observe
   partially-constructed state.
3. Snapshot isolation is automatic: each reader gets an `Arc<Store>` that remains valid
   for the reader's lifetime, regardless of subsequent swaps.
4. Schema consistency is structural: Schema is part of Store (ADR-SCHEMA-005), so each
   snapshot's Schema matches its datoms. No independent Schema versioning needed or permitted.

#### Consequences

- `ArcSwap<Arc<Store>>` is the canonical concurrency primitive for Store access.
- CLI commands load a snapshot once per invocation — no concurrent access concern.
- MCP server holds `ArcSwap<Arc<Store>>` for session lifetime (ADR-INTERFACE-004).
- Write operations (transact, harvest, merge) construct a new `Arc<Store>` and swap atomically.
- In-flight reads are never invalidated, never blocked, never see partial state.
- No `RwLock`, no `Mutex`, no actor — lock-free by design.

#### Falsification

This decision is wrong if: (a) a scenario exists where `ArcSwap` readers observe a
partially-constructed Store (violates C1 + ArcSwap guarantees), or (b) a write is lost
because two concurrent writers both swap (mitigated by single-writer discipline at the
application level — only one transact/harvest/merge executes at a time, producing a
new Store value and swapping it).

---

### ADR-STORE-017: Datom Store Over Vector DB / RAG

**Traces to**: SEED §11, ADRS FD-004
**Stage**: 0

#### Problem
What is the foundational substrate for DDIS? Should the system be built on a vector
database with retrieval-augmented generation (RAG), or on a datom store with structured
queries?

#### Options
A) **Datom store** — Append-only set of `[e, a, v, tx, op]` tuples with Datalog queries.
   Supports logical coherence verification, contradiction detection, causal dependency
   tracing, and CRDT merge. Provides exact answers to structured queries.
B) **Vector database with RAG** — Embed all knowledge as vectors, retrieve by semantic
   similarity. Good at finding "related" content across large corpora. Standard approach
   in LLM-based systems.
C) **Hybrid** — Vector DB for retrieval, datom store for verification. Two substrates
   with a bridging layer.

#### Decision
**Option A.** The core substrate is a datom store. Vector similarity retrieval finds
"related" content but cannot verify logical coherence, detect contradictions, or trace
causal dependencies. The fundamental problem DDIS solves is divergence — the gap between
intent, specification, implementation, and behavior. Divergence detection requires
structured reasoning over exact relationships, not approximate similarity.

#### Formal Justification
The "filing cabinet vs. bigger desk" argument: RAG gives you a bigger desk (more context
in the LLM window). But the problem is not desk size — it is that the documents on the
desk may contradict each other, and the LLM cannot reliably detect this. A datom store
gives you a filing cabinet with a verification system: every fact has provenance, every
relationship is queryable, contradictions are mechanically detectable.

Formally: contradiction detection requires negation (`not(P and not-P)`). Vector similarity
is a continuous distance metric — it has no concept of logical negation. You cannot build
a sound contradiction detector on cosine similarity alone. The datom store's Datalog engine
supports stratified negation (INV-QUERY-001), enabling formal contradiction detection.

#### Consequences
- No vector embeddings in the core store architecture
- Semantic retrieval (ASSOCIATE) is a separate, optional layer that feeds into Datalog
  queries — not a replacement for them
- The store's query power comes from logical inference, not statistical similarity
- Content-addressable identity (C2) enables exact deduplication across agents — something
  vector similarity can only approximate

#### Falsification
This decision is wrong if: a vector-based coherence verification system is demonstrated
that can reliably detect logical contradictions, trace causal dependencies, and produce
CRDT-mergeable state — matching the datom store's verification guarantees.

---

### ADR-STORE-018: Datom Store Replaces JSONL Event Stream

**Traces to**: SEED §4, ADRS FD-009
**Stage**: 0

#### Problem
The Go CLI uses a JSONL-based event stream as its canonical data substrate. Events are
sequential, file-scoped, and processed through a crystallize-materialize-project pipeline.
Should Braid preserve this event-sourcing architecture, layer the datom store on top of it,
or replace it entirely?

#### Options
A) **Datom store as sole canonical substrate** — Replace the JSONL event stream entirely.
   Events become transactions in the datom store. The store subsumes event-sourcing:
   transactions are ordered, provenance-tracked, and content-addressed.
B) **JSONL as application layer, datom store underneath** — Keep JSONL events as the
   user-facing format, with the datom store as the backing storage. Dual source of truth
   creates consistency hazards (dual-write problem).
C) **JSONL as derived view, projected from datom store** — The datom store is canonical;
   JSONL events are projected (rendered) from it. Preserves backward compatibility but
   creates impedance mismatch between datom transactions and JSONL event semantics.

#### Decision
**Option A.** The datom store is the canonical substrate, replacing JSONL event streams.
JSONL events are sequential and file-scoped — they cannot represent concurrent assertions
from multiple agents, cannot be merged by set union, and their identity is positional
(line number in file) rather than content-addressed.

#### Formal Justification
The datom model subsumes event-sourcing:
- An event "entity X changed attribute A from V1 to V2" is two datoms:
  `(X, A, V1, tx, Retract)` and `(X, A, V2, tx, Assert)`
- Event ordering is captured by transaction ordering (HLC timestamps)
- Event provenance is captured by transaction metadata
- Event replay is captured by time-travel queries (`as_of(tx)`)

The datom model adds capabilities JSONL lacks:
- Content-addressed identity (C2): same fact from two agents = one datom
- CRDT merge (C4): set union of two datom sets, no coordination
- Graph-structured queries: Datalog joins across entity relationships
- Schema validation: type-checked assertions at transact time

Option B creates a dual-write problem: every operation must update both JSONL and datom
store consistently, and inconsistency between them is a new divergence type the system
must manage. Option C creates an impedance mismatch: JSONL events have different
semantics (sequential, file-scoped) than datom transactions (set-scoped, content-addressed),
and the projection between them is lossy in both directions.

#### Consequences
- No JSONL event files in Braid's data model
- The Go CLI's crystallize-materialize-project pipeline has no equivalent; its function
  is subsumed by transact + query
- Import from the Go CLI (if needed) is a one-time migration, not an ongoing integration
- The store file format (content-addressed EDN transaction files; see [spec/01b-storage-layout.md](01b-storage-layout.md)) is the sole persistent representation

#### Falsification
This decision is wrong if: a use case is identified where JSONL event semantics (sequential,
file-scoped, streaming) provide capabilities that the datom store cannot replicate, making
some DDIS workflow impossible without JSONL.

---

### ADR-STORE-019: All Durable Information as Datoms

**Traces to**: SEED §5, ADRS LM-007
**Stage**: 0

#### Problem
Should all persistent information live in the datom store, or should some categories of
information (configuration, logs, session history, intermediate computations) live in
external files, databases, or runtime state?

#### Options
A) **Datom-exclusive durability** — All durable information must exist as datoms in the
   store. External representations (Markdown files, CLI output, CLAUDE.md) are projections
   from the store, not independent truth sources.
B) **Datom store + sidecar files** — Core data as datoms, configuration and logs as
   separate files. Simpler initial implementation but creates multiple truth sources.
C) **Datom store + external database** — Core data as datoms, derived/cached data in
   SQLite or similar. Performance optimization but risks cache-source divergence.

#### Decision
**Option A.** All durable information must exist as datoms. This is a direct consequence
of the D-centric formalism (ADR-FOUNDATION-003): if protocol-level state lives outside D,
it cannot be merged (C4), queried (Datalog), or verified (bilateral loop). External
representations are projections — derived views that are always reproducible from the store.

#### Formal Justification
Let I be all durable information. Under Option A:
```
I ⊂ D                           — all information is datoms
projection(D) = external_repr   — Markdown, CLI output, CLAUDE.md are derived
D is the sole source of truth   — no external file can contradict D
```

Under Option B or C, `I = D ∪ E` where E is external state:
```
merge(I₁, I₂) = merge(D₁, D₂) ∪ merge(E₁, E₂)
```
But `merge(E₁, E₂)` has no CRDT guarantee — external files have no content-addressed
identity, no set-union merge, no conflict detection. Any information in E is outside
the coherence verification boundary.

#### Consequences
- Configuration is datoms (schema attributes, agent preferences, guidance parameters)
- Session history is datoms (transactions with provenance)
- Intermediate computations, when they need to persist across sessions, become datoms
- The only non-datom persistent artifacts are the store files themselves (the physical
  representation of the datom set)
- CLAUDE.md, Markdown specs, and CLI output are projections: `render(query(D))`

#### Falsification
This decision is wrong if: a category of durable information is identified that cannot be
represented as datoms without unacceptable overhead (e.g., large binary artifacts where
content-addressed hashing is prohibitively expensive).

---

### ADR-STORE-020: Agent Entity Identification

**Traces to**: SEED §4 (Axiom 1), exploration/03-topology-definition.md §1
**Stage**: 0

#### Problem
Agents are referenced by `:tx/agent` (Ref) in every transaction. What is the
EntityId of an agent entity? Content-addressed identity (INV-STORE-002) requires
that the EntityId be derived from content. But what content identifies an agent?

#### Options
A) **Hash of agent name string** — EntityId = BLAKE3("agent:" + name)
   - Pro: Simple, deterministic, human-readable name
   - Con: Name collision across deployments; renaming destroys identity

B) **Hash of (program, model, instance-salt)** — EntityId = BLAKE3(program + model + salt)
   - Pro: Distinguishes agent types
   - Con: instance-salt must be managed; same model in different instances
     gets different IDs

C) **Hash of agent configuration datoms** — EntityId = BLAKE3(canonical serialization
   of the agent's initial assertion datoms)
   - Pro: Consistent with INV-STORE-002 (content-addressed)
   - Pro: Agent identity IS its first assertion set
   - Con: Slightly more complex; requires canonical serialization order

D) **Hash of (program, model, session-context)** — EntityId = BLAKE3(program + model + session-id)
   - Pro: Each agent session is a distinct entity
   - Pro: Deterministic (same inputs = same EntityId)
   - Pro: Natural isolation (concurrent sessions get distinct identities)
   - Con: Same agent type across sessions = different entities

#### Decision
**Option D.** Agent identity is BLAKE3(program + model + session-context).

Rationale:
- Content-addressed (INV-STORE-002 compliant)
- Deterministic (same agent type in same session = same EntityId)
- Session isolation (concurrent sessions get distinct agent entities)
- Aligns with how `:tx/agent` is used: the agent IS the (program, model, session)
  tuple, not a persistent cross-session identity

#### Consequences
- Agent entities created lazily on first transaction from that (program, model, session)
- Genesis creates SYSTEM_AGENT with fixed EntityId = BLAKE3("system" + "braid" + "genesis")
- `:agent/ident` attribute stores human-readable name
- `:agent/program` and `:agent/model` attributes store the ID components
- `:agent/session-id` stores the session disambiguation token

#### Falsification
This decision is wrong if: a use case requires cross-session agent identity
(e.g., "what has agent claude-code/opus-4.6 contributed across all sessions?")
that cannot be answered by querying agents sharing the same `:agent/program`
and `:agent/model` values.

---

### ADR-STORE-021: Frontier Representation

**Traces to**: SEED §4 (Axiom 3), exploration/07-fitness-function.md §3.3 (D3 staleness)
**Stage**: 0 (data model); 3 (multi-agent usage)

#### Problem
An agent's frontier is the set of datoms it knows about. How should this be
represented in the store for queryability?

#### Options
A) **Set of TxIds** — frontier(α) = {tx₁, tx₂, ..., txₙ} = all transactions α has seen.
   visible(α) = {d ∈ S | d.tx ∈ frontier(α)}
   - Pro: Exact; every datom is attributable to a transaction
   - Pro: Staleness = |{tx ∈ S | tx ∉ frontier(α)}| / |{tx ∈ S}|
   - Con: Large frontier for long-running agents (grows with transaction count)

B) **High-water-mark TxId** — frontier(α) = max(tx seen by α)
   visible(α) = {d ∈ S | d.tx <= frontier(α)} (using HLC ordering)
   - Pro: Compact (single value)
   - Con: Assumes total ordering of transactions; not true in concurrent multi-agent
     (agent A's tx₅ is incomparable with agent B's tx₃ under HLC partial order)

C) **Per-agent high-water-mark (vector clock style)** —
   frontier(α) = {(β, max_tx_β) | β ∈ agents, max_tx_β = latest tx from β seen by α}
   - Pro: Compact (one entry per agent)
   - Pro: Comparison is pointwise: α is ahead of β on agent γ iff frontier(α)[γ] >= frontier(β)[γ]
   - Pro: Staleness = number of entries where frontier(α)[β] < latest(β)
   - Con: Requires knowing the set of all agents (grows with agent count, not tx count)

#### Decision
**Option C.** Per-agent vector clock. Each agent's frontier is a map from agent
IDs to the latest transaction from that agent that the current agent has seen.

Rationale:
- Compact (one entry per agent, not per transaction)
- Directly supports staleness computation (D3 in F(T))
- Comparison is efficient (pointwise max)
- Merge is pointwise max (CRDT: vector clocks form a join-semilattice)
- Consistent with HLC causality (if α saw β's tx₅, α also saw β's tx₁..tx₄)
- In single-agent Stage 0, frontier = {(self, latest_tx)} — trivially degenerate

#### Encoding
Frontier entries are compound entities in the datom store (not JSON blobs or tuples):
```
Each frontier entry is an entity with:
  (entry, :frontier/agent, agent_ref, tx, assert)   — which agent's tx this tracks
  (entry, :frontier/tx,    tx_ref,    tx, assert)    — the latest tx seen from that agent
```
This encoding is the most DDIS-native: frontier entries are facts, facts are datoms,
datoms are queryable via standard Datalog joins.

Resolution: LWW per (observing-agent, tracked-agent) pair — latest observation wins.

#### Consequences
- `:frontier/agent` and `:frontier/tx` attributes defined in Layer 1
- Frontier merge: pointwise max of vector clock entries (join-semilattice)
- Staleness query: count agents where frontier[agent] < that agent's latest tx
- Single-agent: frontier always up-to-date (staleness = 0)
- Entity proliferation bounded: one entity per (observing-agent, tracked-agent) pair

#### Falsification
This decision is wrong if: a multi-agent scenario exists where per-agent
high-water-marks lose information that TxId sets preserve (e.g., an agent
selectively seeing tx₃ and tx₅ but not tx₄ from the same source).

---

### §1.6 Negative Cases

### NEG-STORE-001: No Datom Deletion

**Traces to**: SEED §4, C1, ADRS FD-001
**Verification**: `V:KANI`, `V:PROP`

**Safety property**: `□ ¬(∃ d ∈ S, S' = next(S): d ∉ S')`
In all reachable states, no datom ever disappears from the store.

**Formal statement**:
For all execution traces T and all datoms d:
  if d ∈ S at time t, then d ∈ S at all times t' > t.

**proptest strategy**: Generate adversarial operation sequences (rapid assert/retract cycles,
concurrent merges, crash/recovery). After each operation, verify the datom set is a
superset of all previously-observed sets.

**Kani harness**: Bounded exhaustive check that no sequence of ≤ 20 operations
reduces the datom count.

---

### NEG-STORE-002: No Mutable State in Store

**Traces to**: SEED §4, C1
**Verification**: `V:TYPE`, `V:KANI`

**Safety property**: `□ ¬(∃ operation that modifies an existing datom's fields)`
No operation changes the `(e, a, v, tx, op)` tuple of an existing datom.

**Formal statement**:
The store exposes no `&mut Datom` reference to existing datoms. The only way to
"change" a fact is to assert a new datom.

**Rust type-level enforcement**: `Store` provides only `fn insert(&mut self, d: Datom)`
and `fn iter(&self) -> impl Iterator<Item = &Datom>`. No `fn get_mut`.

---

### NEG-STORE-003: No Sequential ID Assignment

**Traces to**: SEED §4 Axiom 1, C2, ADRS FD-007
**Verification**: `V:TYPE`

**Safety property**: `□ ¬(∃ auto-increment counter for EntityId generation)`
EntityIds are never assigned sequentially.

**Formal statement**:
The `EntityId` type has no `fn next()` or auto-increment constructor.
All EntityId creation goes through content-hashing.

**Rust type-level enforcement**: `EntityId` has a single constructor:
`pub fn from_content(content: &[u8]) -> EntityId`. No `fn new(n: u64)`.

---

### NEG-STORE-004: No Merge Heuristics

**Traces to**: SEED §4, C4
**Verification**: `V:TYPE`, `V:PROP`

**Safety property**: `□ ¬(∃ merge operation that applies heuristics or conflict resolution)`
Merge is pure set union. No heuristic, no conflict resolution, no "smart" merging at the
store level.

**Formal statement**:
`MERGE(S₁, S₂) = S₁ ∪ S₂` — the mathematical set union. Any conflict resolution happens
in the RESOLUTION namespace at query time, not during merge.

**proptest strategy**: Merge two stores with conflicting values for the same entity-attribute.
Verify both values are present in the merged store (no automatic resolution).

---

### NEG-STORE-005: No Store Compaction

**Traces to**: C1, ADRS FD-001
**Verification**: `V:TYPE`

**Safety property**: `□ ¬(∃ operation that removes "old" or "superseded" datoms)`
No garbage collection, compaction, or pruning of the datom set.

**Formal statement**:
The store API exposes no `compact()`, `gc()`, `prune()`, or `vacuum()` operations.
Retracted datoms remain in the store permanently.

---

