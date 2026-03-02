# SPEC.md — Braid Specification

> **Identity**: Cleanroom-grade specification for Braid, the Rust implementation of DDIS.
> Every invariant is formally falsifiable, every ADR grounded in algebraic properties,
> every negative case stated as a safety property. The specification enables formal
> verification at implementation time — type-level guarantees, property-based testing,
> bounded model checking, and protocol model checking.
>
> **Methodology**: Three-level cleanroom refinement (Mills). Each namespace proceeds:
> Level 0 (algebraic law) → Level 1 (state machine invariant) → Level 2 (implementation contract).
> Each level is verified against the level above it. Refinement is monotonic: Level 1 preserves
> Level 0 laws; Level 2 preserves Level 1 invariants.
>
> **Self-bootstrap**: This specification is the first dataset the system will manage (C7, FD-006).
> Every element has an ID, type, and traceability to SEED.md — structured for mechanical migration
> into the datom store at Stage 0.

---

## §0. Preamble

### §0.1 Scope and Purpose

This document specifies Braid — the Rust implementation of DDIS (Decision-Driven Implementation
Specification). Braid is an append-only datom store with CRDT merge semantics, a Datalog query
engine, a harvest/seed lifecycle for durable knowledge across conversation boundaries, and a
reconciliation framework that maintains verifiable coherence between intent, specification,
implementation, and observed behavior.

The specification covers 14 namespaces organized into four waves:

- **Foundation** (Wave 1): STORE, SCHEMA, QUERY, RESOLUTION — the algebraic core
- **Lifecycle** (Wave 2): HARVEST, SEED, MERGE, SYNC — session and coordination mechanics
- **Intelligence** (Wave 3): SIGNAL, BILATERAL, DELIBERATION, GUIDANCE, BUDGET, INTERFACE — steering and adaptation
- **Integration** (Wave 4): Uncertainty register, verification plan, cross-reference index

### §0.2 Conventions

#### Element ID Format

```
INV-{NAMESPACE}-{NNN}    Invariant (falsifiable claim with violation condition)
ADR-{NAMESPACE}-{NNN}    Architectural Decision Record (choice with alternatives and rationale)
NEG-{NAMESPACE}-{NNN}    Negative Case (safety property: what must NOT happen)
```

Namespaces: `STORE`, `SCHEMA`, `QUERY`, `RESOLUTION`, `HARVEST`, `SEED`, `MERGE`, `SYNC`,
`SIGNAL`, `BILATERAL`, `DELIBERATION`, `GUIDANCE`, `BUDGET`, `INTERFACE`.

#### Three-Level Refinement

Every invariant follows the cleanroom refinement chain:

| Level | Name | Content | Verification |
|-------|------|---------|--------------|
| 0 | Algebraic Law | Mathematical objects, operations, laws. No state, no time. | Proof by construction; proptest properties |
| 1 | State Machine | State, transitions, pre/postconditions, invariants over reachable states. | Stateright/TLA+ models; Kani function contracts |
| 2 | Implementation Contract | Rust types, function signatures, typestate patterns. | Type system; proptest; Kani harnesses; Miri |

#### Verification Tags

Every invariant is tagged with one or more verification methods:

| Tag | Method | Tool | Guarantee | Cost |
|-----|--------|------|-----------|------|
| `V:TYPE` | Type system | `rustc` | Compile-time state machine correctness | Free |
| `V:PROP` | Property-based testing | `proptest` | Holds for random inputs (probabilistic) | Low |
| `V:KANI` | Bounded model checking | `kani` | Holds for all inputs up to bound (exhaustive) | Moderate |
| `V:CONTRACT` | Function contracts | `kani::requires/ensures` | Modular correctness (compositional) | Moderate |
| `V:MODEL` | Protocol model checking | `stateright` or TLA+ | Protocol safety/liveness (all reachable states) | High |
| `V:DEDUCTIVE` | Deductive verification | `verus` or `creusot` | Full functional correctness (proof) | Very high |
| `V:MIRI` | UB detection | `cargo miri test` | No undefined behavior in test paths | Low |

**Minimum requirements**:
- Every invariant MUST have at least `V:PROP`.
- Critical invariants (STORE, MERGE, SCHEMA) MUST have `V:KANI`.
- Protocol invariants (SYNC, MERGE cascade, DELIBERATION) MUST have `V:MODEL`.

#### Traceability Notation

Every element traces to source documents:
- `SEED §N` — Section N of SEED.md
- `ADRS {CAT-NNN}` — Entry in ADRS.md (e.g., `ADRS FD-001`)
- `T{NN}:{line}` — Transcript line reference (e.g., `T01:328` = Transcript 01, line 328)
- `C{N}` — Hard constraint from CLAUDE.md (e.g., `C1` = append-only store)

#### Stage Assignment

Every element is assigned to an implementation stage:

| Stage | Scope | Dependencies |
|-------|-------|--------------|
| 0 | Harvest/Seed cycle — core store, query, schema, harvest, seed, guidance, dynamic CLAUDE.md | None |
| 1 | Budget-aware output + guidance injection | Stage 0 |
| 2 | Branching + deliberation | Stage 1 |
| 3 | Multi-agent coordination — CRDT merge, sync barriers, signal system | Stage 2 |
| 4 | Advanced intelligence — significance, spectral authority, learned guidance, TUI | Stage 3 |

### §0.3 Namespace Index

| § | Namespace | SEED.md §§ | ADRS.md Categories | Wave | Est. Elements |
|---|-----------|------------|---------------------|------|---------------|
| 1 | STORE | §4, §11 | FD-001–012, AS-001–010, SR-001–011 | 1 | ~15 INV, ~12 ADR, ~5 NEG |
| 2 | SCHEMA | §4 | SR-008–009, FD-005, FD-008 | 1 | ~8 INV, ~4 ADR, ~3 NEG |
| 3 | QUERY | §4 | FD-003, SQ-001–010, PO-013 | 1 | ~12 INV, ~8 ADR, ~4 NEG |
| 4 | RESOLUTION | §4 | FD-005, CR-001–007 | 1 | ~8 INV, ~5 ADR, ~3 NEG |
| 5 | HARVEST | §5 | LM-005–006, LM-012–013 | 2 | ~8 INV, ~4 ADR, ~3 NEG |
| 6 | SEED | §5, §8 | IB-010, PO-014, GU-004 | 2 | ~6 INV, ~3 ADR, ~2 NEG |
| 7 | MERGE | §6 | AS-001, PD-004, PO-006 | 2 | ~8 INV, ~4 ADR, ~3 NEG |
| 8 | SYNC | §6 | PO-010, SQ-001, SQ-004 | 2 | ~5 INV, ~3 ADR, ~2 NEG |
| 9 | SIGNAL | §6 | PO-004–005, PO-008 | 3 | ~6 INV, ~3 ADR, ~2 NEG |
| 10 | BILATERAL | §3, §6 | SQ-006, CO-004 | 3 | ~5 INV, ~3 ADR, ~2 NEG |
| 11 | DELIBERATION | §6 | CR-004–005, CR-007, PO-007 | 3 | ~6 INV, ~4 ADR, ~2 NEG |
| 12 | GUIDANCE | §7, §8 | GU-001–008 | 3 | ~8 INV, ~5 ADR, ~3 NEG |
| 13 | BUDGET | §8 | IB-004–007 | 3 | ~6 INV, ~4 ADR, ~2 NEG |
| 14 | INTERFACE | §8 | IB-001–003, IB-008–012 | 3 | ~8 INV, ~5 ADR, ~3 NEG |
| 15 | — | — | — | 4 | Uncertainty Register |
| 16 | — | — | — | 4 | Verification Plan |
| 17 | — | — | — | 4 | Cross-Reference Index |

### §0.4 Hard Constraints (Non-Negotiable)

These constraints from CLAUDE.md are axiomatic. Every element in this specification must be
consistent with all seven. Violation of any constraint is a defect regardless of other merits.

| ID | Constraint | Source |
|----|-----------|--------|
| C1 | **Append-only store.** The datom store never deletes or mutates. Retractions are new datoms with `op=retract`. | SEED §4 Axiom 2, FD-001 |
| C2 | **Identity by content.** A datom is `[e, a, v, tx, op]`. Same fact = same datom. | SEED §4 Axiom 1, FD-007 |
| C3 | **Schema-as-data.** Schema is defined as datoms, not separate DDL. Schema evolution is a transaction. | SEED §4, FD-008 |
| C4 | **CRDT merge by set union.** Merging two stores = mathematical set union of datom sets. | SEED §4 Axiom 2, AS-001 |
| C5 | **Traceability.** Every artifact traces to spec; every spec element traces to SEED.md goals. | SEED §3 |
| C6 | **Falsifiability.** Every invariant has an explicit violation condition. | SEED §3 |
| C7 | **Self-bootstrap.** DDIS specifies itself. Spec elements are the first data the system manages. | SEED §10, FD-006 |

---

## §1. STORE — Datom Store

### §1.0 Overview

The datom store is the foundational substrate of Braid. All state — specification elements,
implementation facts, observations, decisions, provenance — lives as datoms in a single
append-only store. The store is a G-Set CvRDT: a grow-only set of datoms under set union.

**Traces to**: SEED.md §4, §11
**ADRS.md sources**: FD-001–012, AS-001–010, SR-001–011, PD-001, PD-003–004, PO-001, PO-012

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
      | Ref EntityId | Bytes | URI | BigInt | BigDec | Tuple [Value] | Json String

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
  S₀.datoms = {meta_schema_datoms}         — exactly the 17 axiomatic attributes
  S₀.frontier = { system: tx_0 }
  ∀ S₁, S₂ created by GENESIS: S₁ = S₂   — deterministic (constant hash)
  tx_0 has no causal predecessors
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
    LWW:     greatest HLC assertion
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

/// Content-addressed entity identifier.
/// Derived from the semantic content, not sequentially assigned.
#[derive(Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct EntityId(pub [u8; 32]);  // SHA-256 of content

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

    /// Merge another store (set union).
    pub fn merge(&mut self, other: &Store) -> MergeReceipt;

    /// Query the LIVE index for current state of an entity.
    pub fn current(&self, entity: EntityId) -> EntityView;

    /// Query at a specific frontier.
    pub fn as_of(&self, frontier: &Frontier) -> SnapshotView;

    /// Datom count (monotonically non-decreasing).
    pub fn len(&self) -> usize;
}
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
**Verification**: `V:PROP`, `V:KANI`
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
**Verification**: `V:PROP`, `V:KANI`
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
The genesis transaction installs exactly the 17 axiomatic meta-schema attributes.
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
  ∀ p ∈ P: p.tx_id < T.tx_id   (HLC ordering)
  ∀ p ∈ P: p ∈ S                (predecessors exist in the store)
```

#### Level 1 (State Invariant)
A transaction's causal predecessors must all be present in the store at the time of
the transaction. HLC timestamps are monotonically increasing per agent.

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
```

**Falsification**: A transaction referencing a causal predecessor that does not exist in the store.

**proptest strategy**: Generate transaction chains with random predecessor references.
Verify that commits with invalid predecessors are rejected.

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
∀ DDIS commands C: ∃ transaction T such that executing C produces T
  (no read-only path bypasses the store; queries produce tx records for provenance)
```

#### Level 1 (State Invariant)
Every CLI command, including queries, generates a transaction recording provenance
(who queried, what, when, why). The transaction may contain only metadata datoms.

#### Level 2 (Implementation Contract)
```rust
impl Store {
    /// Even queries produce a provenance transaction.
    pub fn query(&mut self, q: &Query) -> QueryResult {
        let result = self.evaluate(q);
        self.record_query_provenance(q, &result);
        result
    }
}
```

**Falsification**: Any DDIS command that completes without producing a transaction record.

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
through the shared filesystem (SR-007).

#### Formal Justification
Embedded deployment preserves the property that all coordination goes through the store.
A separate database server (Option B) introduces a coordination channel outside the datom
store, potentially violating FD-012 (every command is a transaction).

---

### ADR-STORE-007: File-Backed Store with Git

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

## §2. SCHEMA — Schema-as-Data

### §2.0 Overview

Schema in Braid is not a separate DDL or configuration file — it is data in the store
itself. The schema is a set of datoms that describe what attributes exist, what types
they expect, and how they behave during conflict resolution. Schema evolution is a
transaction, not a migration.

**Traces to**: SEED.md §4, C3
**ADRS.md sources**: FD-002, FD-008, SR-008, SR-009, SR-010, PO-012

---

### §2.1 Level 0: Algebraic Specification

#### Meta-Schema Recursion

```
The schema S_schema ⊂ S is a subset of datoms in the store.
Schema datoms describe attributes; attributes describe datoms.

Self-reference: the meta-schema attributes describe themselves.
  e.g., :db/valueType has valueType :db.type/keyword
        :db/cardinality has cardinality :db.cardinality/one

Formally: Let A₀ = {a₁, ..., a₁₇} be the 17 axiomatic meta-schema attributes.
∀ aᵢ ∈ A₀: ∃ datoms in S₀ (genesis) that define aᵢ using A₀ itself.
The meta-schema is the fixed point of "attributes that describe attributes."
```

#### Schema as Monotonic Extension

```
Schema evolution is store growth:
  schema(S) ⊆ schema(S')   whenever S ⊆ S'

New attributes are added by asserting new datoms. Existing attributes are never removed
(C1 — append-only). Attribute properties can be "changed" by asserting new values and
retracting old ones, but the history of every schema change is preserved.
```

#### Attribute Algebra

```
Attribute a is fully specified by:
  :db/ident        — keyword name (e.g., :task/status)
  :db/valueType    — the value domain (one of the 14 value types)
  :db/cardinality  — :one | :many
  :db/resolutionMode — :lww | :lattice | :multi  (per-attribute conflict resolution)
  :db/doc          — documentation string

Optional:
  :db/unique       — :identity | :value (uniqueness constraint)
  :db/isComponent  — boolean (component entity lifecycle)
  :db/latticeOrder — ref to lattice definition (if resolutionMode = :lattice)
  :db/lwwClock     — :hlc | :wall | :agent-rank (if resolutionMode = :lww)
```

---

### §2.2 Level 1: State Machine Specification

#### Genesis Transaction

```
GENESIS() → S₀ containing exactly:

For each of the 17 axiomatic attributes aᵢ:
  (aᵢ, :db/ident,        <keyword>,     tx₀, Assert)
  (aᵢ, :db/valueType,    <type>,        tx₀, Assert)
  (aᵢ, :db/cardinality,  <cardinality>, tx₀, Assert)
  (aᵢ, :db/doc,          <description>, tx₀, Assert)
  ... (additional properties as needed)

tx₀ has no causal predecessors.
tx₀ is the root of the causal graph.
```

#### The 17 Axiomatic Attributes

```
Layer 0 — Meta-Schema (self-describing):
  :db/ident           — Keyword    :one    — attribute's keyword name
  :db/valueType       — Keyword    :one    — value type constraint
  :db/cardinality     — Keyword    :one    — :one or :many
  :db/doc             — String     :one    — documentation
  :db/unique          — Keyword    :one    — :identity or :value
  :db/isComponent     — Boolean    :one    — component lifecycle binding
  :db/resolutionMode  — Keyword    :one    — :lww, :lattice, or :multi
  :db/latticeOrder    — Ref        :one    — ref to lattice definition entity
  :db/lwwClock        — Keyword    :one    — :hlc, :wall, or :agent-rank

Lattice definition attributes:
  :lattice/ident      — Keyword    :one    — lattice name
  :lattice/elements   — Keyword    :many   — set of lattice elements
  :lattice/comparator — String     :one    — ordering function name
  :lattice/bottom     — Keyword    :one    — bottom element
  :lattice/top        — Keyword    :one    — top element (if bounded)

Transaction metadata:
  :tx/time            — Instant    :one    — wall-clock time
  :tx/agent           — Ref        :one    — agent who transacted
  :tx/provenance      — Keyword    :one    — provenance type
```

#### Schema Evolution as Transaction

```
ADD-ATTRIBUTE(S, attr_spec) → S'

PRE:
  attr_spec contains at minimum: :db/ident, :db/valueType, :db/cardinality
  No existing attribute has the same :db/ident (unless this is a schema update)

POST:
  S'.datoms = S.datoms ∪ {datoms defining the new attribute}
  schema(S') ⊃ schema(S)

SCHEMA-UPDATE(S, attr_ident, property, new_value) → S'

PRE:
  attr_ident exists in schema(S)
  property is a valid meta-schema attribute
  new_value is compatible with the meta-schema attribute's type

POST:
  S'.datoms = S.datoms ∪ {(attr_entity, property, new_value, tx, Assert)}
  The old value is NOT removed (append-only). LIVE index resolves to new value.
```

---

### §2.3 Level 2: Interface Specification

```rust
/// The 17 axiomatic attributes — hardcoded in the engine.
pub mod meta_schema {
    pub const DB_IDENT: Attribute = Attribute::from_keyword(":db/ident");
    pub const DB_VALUE_TYPE: Attribute = Attribute::from_keyword(":db/valueType");
    pub const DB_CARDINALITY: Attribute = Attribute::from_keyword(":db/cardinality");
    pub const DB_DOC: Attribute = Attribute::from_keyword(":db/doc");
    pub const DB_UNIQUE: Attribute = Attribute::from_keyword(":db/unique");
    pub const DB_IS_COMPONENT: Attribute = Attribute::from_keyword(":db/isComponent");
    pub const DB_RESOLUTION_MODE: Attribute = Attribute::from_keyword(":db/resolutionMode");
    pub const DB_LATTICE_ORDER: Attribute = Attribute::from_keyword(":db/latticeOrder");
    pub const DB_LWW_CLOCK: Attribute = Attribute::from_keyword(":db/lwwClock");
    pub const LATTICE_IDENT: Attribute = Attribute::from_keyword(":lattice/ident");
    pub const LATTICE_ELEMENTS: Attribute = Attribute::from_keyword(":lattice/elements");
    pub const LATTICE_COMPARATOR: Attribute = Attribute::from_keyword(":lattice/comparator");
    pub const LATTICE_BOTTOM: Attribute = Attribute::from_keyword(":lattice/bottom");
    pub const LATTICE_TOP: Attribute = Attribute::from_keyword(":lattice/top");
    pub const TX_TIME: Attribute = Attribute::from_keyword(":tx/time");
    pub const TX_AGENT: Attribute = Attribute::from_keyword(":tx/agent");
    pub const TX_PROVENANCE: Attribute = Attribute::from_keyword(":tx/provenance");
}

pub struct Schema {
    store: Store,  // schema IS the store (filtered to schema datoms)
}

impl Schema {
    /// Look up attribute definition.
    pub fn attribute(&self, ident: &Keyword) -> Option<AttributeDef>;

    /// Validate a value against an attribute's type.
    pub fn validate_value(&self, attr: &Attribute, value: &Value) -> Result<(), SchemaError>;

    /// Add a new attribute (returns a Transaction).
    pub fn define_attribute(&self, spec: AttributeSpec) -> Transaction<Building>;

    /// All known attributes.
    pub fn attributes(&self) -> impl Iterator<Item = AttributeDef>;
}
```

#### CLI Commands

```
braid schema                          # List all attributes with types
braid schema add --ident :task/status --type keyword --cardinality one --resolution lattice
braid schema show :task/status        # Show attribute definition and history
```

---

### §2.4 Invariants

### INV-SCHEMA-001: Schema-as-Data

**Traces to**: SEED §4, C3, ADRS FD-008
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
schema(S) ⊂ S
  (the schema is a subset of the store, not a separate structure)
∀ attribute definitions: they are datoms in the store
```

#### Level 1 (State Invariant)
There is no schema file, DDL, or configuration outside the store.
All attribute definitions are queryable via the same query engine as any other datoms.

#### Level 2 (Implementation Contract)
```rust
// Schema is a view over the store, not a separate data structure.
pub struct Schema<'a> { store: &'a Store }

impl<'a> Schema<'a> {
    pub fn attribute(&self, ident: &Keyword) -> Option<AttributeDef> {
        // Query the store for datoms about this attribute
        self.store.query_attribute(ident)
    }
}
```

**Falsification**: Any attribute definition that exists outside the datom store (e.g., in a
config file, hardcoded enum, or separate database table).

---

### INV-SCHEMA-002: Genesis Completeness

**Traces to**: SEED §10, ADRS PO-012, SR-008
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ aᵢ ∈ A₀ (the 17 axiomatic attributes):
  ∃ datoms in GENESIS() defining aᵢ
  AND those datoms use only attributes from A₀
  (the meta-schema is self-contained)
```

#### Level 1 (State Invariant)
The genesis transaction contains exactly the 17 axiomatic attribute definitions.
Each attribute is fully specified (ident, valueType, cardinality at minimum).
No non-meta-schema datoms exist in genesis.

#### Level 2 (Implementation Contract)
```rust
fn genesis() -> Store {
    let mut store = Store::empty();
    let tx = Transaction::<Building>::new(SYSTEM_AGENT)
        .with_provenance(ProvenanceType::Observed);
    // Assert exactly 17 attributes...
    // Assert each attribute's ident, valueType, cardinality, doc
    let tx = tx.commit_genesis();  // special: bypasses schema validation (bootstrap)
    store.apply_genesis(tx);
    assert_eq!(store.schema().attributes().count(), 17);
    store
}
```

**Falsification**: A genesis store where `schema.attributes().count() != 17`, or where
any axiomatic attribute lacks a complete definition.

---

### INV-SCHEMA-003: Schema Monotonicity

**Traces to**: SEED §4, C1, C3
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ S ⊆ S': schema(S) ⊆ schema(S')
  (schema can only grow; attributes are never removed)
```

#### Level 1 (State Invariant)
Once an attribute is defined, it is permanently part of the schema. Its properties
may be updated (via new datoms), but the attribute identity persists forever.

**Falsification**: An operation that removes an attribute from the schema.

---

### INV-SCHEMA-004: Schema Validation on Transact

**Traces to**: SEED §4, ADRS PO-001
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ TRANSACT(S, T):
  ∀ d ∈ T.datoms:
    d.a ∈ schema(S)                              — attribute must exist
    typeof(d.v) = schema(S).valueType(d.a)       — value type must match
    (d.op = Retract ⟹ ∃ d' ∈ S: d'.e = d.e ∧ d'.a = d.a ∧ d'.op = Assert)
      — can only retract what was asserted
```

#### Level 1 (State Invariant)
No datom with an undefined attribute or mistyped value enters the store.
Retractions require a prior assertion of the same entity-attribute pair.

#### Level 2 (Implementation Contract)
```rust
impl Transaction<Building> {
    pub fn commit(self, schema: &Schema) -> Result<Transaction<Committed>, TxValidationError> {
        for datom in &self.datoms {
            let attr_def = schema.attribute(&datom.attribute)
                .ok_or(TxValidationError::UnknownAttribute(datom.attribute.clone()))?;
            attr_def.validate_value(&datom.value)?;
        }
        Ok(Transaction { _state: PhantomData::<Committed>, ..self })
    }
}
```

**Falsification**: A datom with attribute `:foo/bar` entering the store when no attribute
`:foo/bar` is defined in the schema.

---

### INV-SCHEMA-005: Meta-Schema Self-Description

**Traces to**: ADRS SR-008
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ aᵢ ∈ A₀: aᵢ is described by datoms that use only attributes from A₀
  (the meta-schema is a fixed point: it describes itself using itself)
```

#### Level 1 (State Invariant)
The `:db/ident` attribute has a datom `(:db/ident, :db/valueType, :db.type/keyword, tx₀, Assert)`.
This datom describes `:db/ident`'s value type using the `:db/valueType` attribute, which is
itself one of the 17 axiomatic attributes.

**Falsification**: Any axiomatic attribute whose definition requires an attribute outside A₀.

---

### INV-SCHEMA-006: Six-Layer Schema Architecture

**Traces to**: ADRS SR-009
**Verification**: `V:PROP`
**Stage**: 0–4 (progressive)

#### Level 0 (Algebraic Law)
```
Schema is organized into 6 layers:
  Layer 0: Meta-schema (17 axiomatic attributes)        — Stage 0
  Layer 1: Agent & Provenance (2 types, 16 attributes)  — Stage 0
  Layer 2: DDIS Core (12 types, 72 attributes)          — Stage 0–1
  Layer 3: Discovery & Exploration (5 types, 28 attrs)  — Stage 1–2
  Layer 4: Coordination (7 types, 35 attributes)        — Stage 2–3
  Layer 5: Workflow & Task (5 types, 27 attributes)     — Stage 3–4

Each layer depends only on layers below it.
```

#### Level 1 (State Invariant)
Attributes in Layer N reference only entity types defined in Layers 0..N.

**Falsification**: A Layer 1 attribute that references a Layer 3 entity type.

---

### INV-SCHEMA-007: Lattice Definition Completeness

**Traces to**: ADRS SR-010
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ attributes a with :db/resolutionMode = :lattice:
  ∃ lattice entity L such that:
    a.:db/latticeOrder = L
    L.:lattice/ident is defined
    L.:lattice/elements is non-empty
    L.:lattice/comparator names a valid ordering function
    L.:lattice/bottom ∈ L.:lattice/elements
```

#### Level 1 (State Invariant)
Every lattice-resolved attribute has a complete lattice definition.

**Falsification**: An attribute declared as `:lattice` resolution mode with no corresponding
lattice definition, or a lattice definition missing required properties.

---

### INV-SCHEMA-008: Diamond Lattice Signal Generation

**Traces to**: ADRS AS-009, SR-010
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
For lattices with diamond structure (two incomparable top elements):
  join(a, b) where a ⊥ b = error_signal_element

Example: challenge-verdict lattice
  :confirmed ⊥ :refuted (incomparable)
  join(:confirmed, :refuted) = :contradicted (error signal)
```

#### Level 1 (State Invariant)
When concurrent assertions produce incomparable lattice values, the join operation
produces a first-class error signal (the top of the diamond), which triggers the
coordination layer's conflict detection.

**Falsification**: Two incomparable lattice values that silently merge without producing
a coordination signal.

---

### §2.5 ADRs

### ADR-SCHEMA-001: Schema-as-Data Over DDL

**Traces to**: SEED §4, C3, ADRS FD-008
**Stage**: 0

#### Problem
Where does the schema live?

#### Options
A) **Schema as datoms in the store** — self-describing, queryable, evolvable by transaction.
B) **Separate DDL file** — traditional approach (the Go CLI uses 39 CREATE TABLE statements).
C) **Hardcoded in source** — enums and structs in Rust source code.

#### Decision
**Option A.** The schema is datoms. Schema evolution is a transaction. Schema queries use the
same engine as data queries.

#### Formal Justification
Option A preserves C3 and C7 (self-bootstrap). The schema is the first data the system
manages — it describes itself. Options B and C create a separate truth source that can
diverge from the store.

---

### ADR-SCHEMA-002: 17 Axiomatic Attributes

**Traces to**: ADRS SR-008
**Stage**: 0

#### Problem
How does the schema bootstrap itself?

#### Options
A) **17 hardcoded meta-schema attributes** — the minimum set that can describe everything else.
B) **Empty genesis** — all attributes added post-genesis by user transactions.
C) **Full domain schema in genesis** — all 195+ attributes hardcoded.

#### Decision
**Option A.** Exactly 17 attributes are hardcoded in the engine (not defined by datoms that
reference themselves — that would be circular). Everything else is defined by datoms using
these 17. This is the only place where "code knows about schema" — all other schema is data.

#### Formal Justification
Option B has a chicken-and-egg problem: you can't define `:db/ident` as a datom before
`:db/ident` exists. Option C defeats the purpose of schema-as-data. Option A is the
minimal fixed point.

---

### ADR-SCHEMA-003: Six-Layer Architecture

**Traces to**: ADRS SR-009
**Stage**: 0

#### Problem
How should the ~195+ attributes be organized?

#### Options
A) **Six layers with dependency ordering** — each layer depends only on layers below it.
B) **Flat namespace** — all attributes at one level.
C) **Module-per-entity-type** — each entity type is an independent module.

#### Decision
**Option A.** Six layers enable incremental implementation. Stage 0 installs Layers 0–1
(meta-schema + agent/provenance). Each subsequent stage adds the next layer. The dependency
ordering ensures Layer N attributes can be fully defined using only Layer 0..N-1 entity types.

---

### ADR-SCHEMA-004: Twelve Named Lattices

**Traces to**: ADRS SR-010
**Stage**: 0–2

#### Problem
How many lattice definitions does the system need?

#### Decision
Twelve lattices, several with non-trivial diamond structure:
1. agent-lifecycle
2. confidence-level
3. adr-lifecycle
4. witness-lifecycle
5. challenge-verdict (diamond: `:confirmed`/`:refuted` → `:contradicted`)
6. thread-lifecycle
7. finding-lifecycle (diamond)
8. proposal-lifecycle (three-way incomparable → `:contested`)
9. delegation-level
10. conflict-lifecycle
11. task-lifecycle
12. numeric-max

The diamond patterns connect lattice algebra to coordination (INV-SCHEMA-008).

---

### §2.6 Negative Cases

### NEG-SCHEMA-001: No External Schema

**Traces to**: C3
**Verification**: `V:TYPE`

**Safety property**: `□ ¬(∃ schema definition outside the datom store)`
No YAML config, no CREATE TABLE, no schema.json.

**Formal statement**: The only source of truth for "what attributes exist" is
`store.query([:find ?a :where [?a :db/ident ?name]])`.

**Rust type-level enforcement**: `Schema` wraps a `&Store` reference. No `Schema::from_file()`.

---

### NEG-SCHEMA-002: No Schema Deletion

**Traces to**: C1, C3
**Verification**: `V:TYPE`, `V:PROP`

**Safety property**: `□ ¬(∃ operation that removes an attribute from the schema)`
Attributes can be deprecated (via new datoms marking them deprecated), but never deleted.

**Formal statement**: `∀ t, t' where t < t': attributes(S(t)) ⊆ attributes(S(t'))`

---

### NEG-SCHEMA-003: No Circular Layer Dependencies

**Traces to**: ADRS SR-009
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ attribute in Layer N referencing entity type from Layer M where M > N)`

**proptest strategy**: For each attribute, verify all referenced entity types are from
the same or lower layer.

---

## §3. QUERY — Datalog Query Engine

### §3.0 Overview

Queries in Braid use a Datomic-style Datalog dialect with semi-naive bottom-up evaluation.
The query engine classifies queries into six strata of increasing power and cost, with
CALM compliance determining which queries can run without coordination.

**Traces to**: SEED.md §4
**ADRS.md sources**: FD-003, SQ-001–010, PO-013, AA-001

---

### §3.1 Level 0: Algebraic Specification

#### Datalog Fixpoint

```
A Datalog program P over database D (the datom store) computes the minimal fixpoint:
  T_P(I) = I ∪ { head(r) | r ∈ P, body(r) ⊆ I }
  fixpoint(P, D) = lfp(T_P, D) = T_P^ω(D)

The fixpoint exists and is unique (by Knaster-Tarski, since T_P is monotone on
the lattice of interpretations ordered by subset inclusion).
```

#### CALM Theorem Compliance

```
CALM (Consistency As Logical Monotonicity):
  A program has a consistent, coordination-free distributed implementation
  iff it is monotone.

Monotone query: adding facts can only add results (never remove them).
  ∀ D ⊆ D': Q(D) ⊆ Q(D')

Non-monotone operations: negation, aggregation, set difference.
  These are frontier-relative: result depends on what is NOT in the store,
  which varies by agent frontier.
```

#### Semi-Naive Evaluation

```
Standard naive evaluation: iterate T_P until fixpoint.
Semi-naive optimization: on each iteration, only derive facts using at least
one NEW fact from the previous iteration.

ΔT_P^(i+1) = T_P(I^i ∪ ΔI^i) \ I^i
I^(i+1) = I^i ∪ ΔT_P^(i+1)

Terminates when ΔT_P^(i+1) = ∅.
```

#### Query Modes

```
QueryMode = Monotonic         — runs at any frontier without coordination
          | Stratified(FId)   — non-monotonic, evaluated at specific frontier
          | Barriered(BId)    — requires sync barrier for correctness

∀ queries Q:
  is_monotonic(Q) ⟹ mode(Q) = Monotonic
  has_negation(Q) ∨ has_aggregation(Q) ⟹ mode(Q) ∈ {Stratified, Barriered}
```

---

### §3.2 Level 1: State Machine Specification

#### Six-Stratum Classification

```
Stratum 0 — Primitive (monotonic):
  Current-value over LIVE index. No joins beyond entity lookup.
  QueryMode: Monotonic
  Examples: current-value, entity-attributes, type-instances

Stratum 1 — Graph Traversal (monotonic):
  Multi-hop joins following references. Transitive closure.
  QueryMode: Monotonic
  Examples: causal-ancestor, depends-on, cross-ref reachability

Stratum 2 — Uncertainty (mixed):
  Epistemic (count-distinct aggregation), aleatory (entropy — FFI),
  consequential (DAG traversal — FFI).
  QueryMode: Stratified
  Examples: epistemic-uncertainty, aleatory-uncertainty, consequential-risk

Stratum 3 — Authority (not pure Datalog):
  Linear algebra: SVD of agent-entity matrix.
  QueryMode: Stratified (FFI to Rust linear algebra)
  Examples: spectral-authority, delegation-threshold

Stratum 4 — Conflict Detection (conservatively monotonic):
  Concurrent assertion detection on cardinality-one attributes.
  QueryMode: Monotonic (conservative — may overcount)
  Examples: detect-conflicts, route-conflict

Stratum 5 — Bilateral Loop (non-monotonic):
  Fitness computation, crystallization readiness, drift measurement.
  QueryMode: Barriered (for correctness-critical decisions)
  Examples: spec-fitness, crystallization-candidates, drift-candidates
```

#### Query Evaluation Pipeline

```
QUERY(S, expression, frontier, mode) → QueryResult

PRE:
  expression is a valid Datalog program
  if mode = Monotonic: expression contains no negation/aggregation
  if mode = Barriered(id): barrier id is resolved

PIPELINE:
  1. Parse expression → AST
  2. Classify monotonicity → reject Monotonic mode if non-monotonic
  3. Determine stratum
  4. Select data source:
     - Monotonic: any available frontier (default: local)
     - Stratified: specified frontier
     - Barriered: barrier's consistent cut
  5. Evaluate via semi-naive bottom-up with FFI for derived functions
  6. Record query provenance as transaction (INV-STORE-014)
  7. Generate access event in access log (INV-QUERY-003)

POST:
  result is the minimal fixpoint of the program over the selected data
  provenance transaction recorded
  access event generated
```

#### Frontier-Scoped Evaluation

```
A query at frontier F sees exactly:
  visible(F) = {d ∈ S | d.tx ≤ max(F[d.tx.agent])}

Frontier is itself a datom attribute (:tx/frontier), enabling:
  [:find ?agent ?tx :where [?tx :tx/frontier ?f] [?f :frontier/agent ?agent]]
```

---

### §3.3 Level 2: Interface Specification

```rust
/// Datalog query expression.
pub enum QueryExpr {
    Find {
        variables: Vec<Variable>,
        clauses: Vec<Clause>,
    },
    Pull {
        pattern: PullPattern,
        entity: EntityRef,
    },
}

pub enum Clause {
    /// Pattern match: [?e ?a ?v]
    Pattern(EntityRef, AttributeRef, ValueRef),
    /// Frontier scope: [:frontier ?f]
    Frontier(FrontierRef),
    /// Negation: (not [?e :attr ?v])
    Not(Box<Clause>),
    /// Aggregation: (aggregate ?var fn)
    Aggregate(Variable, AggregateFunc),
    /// FFI: call Rust function
    Ffi(FfiCall),
}

pub enum QueryMode {
    Monotonic,
    Stratified { frontier: Frontier },
    Barriered { barrier_id: BarrierId },
}

pub struct QueryResult {
    pub tuples: Vec<Vec<Value>>,
    pub mode: QueryMode,
    pub stratum: u8,
    pub provenance_tx: TxId,
}

impl Store {
    pub fn query(&mut self, expr: &QueryExpr, mode: QueryMode) -> Result<QueryResult, QueryError>;
}
```

#### FFI Boundary

```rust
/// Derived functions that cannot be expressed in pure Datalog.
pub trait DerivedFunction {
    fn name(&self) -> &str;
    fn evaluate(&self, inputs: &[Value]) -> Result<Value, FfiError>;
}

/// Three core derived functions:
/// 1. σ_a (aleatory uncertainty) — requires entropy computation
/// 2. σ_c (consequential uncertainty) — requires bottom-up DAG traversal
/// 3. spectral_authority — requires SVD (linear algebra)
pub fn register_derived_functions(engine: &mut QueryEngine) {
    engine.register_ffi("aleatory_uncertainty", AleatoryUncertainty);
    engine.register_ffi("consequential_uncertainty", ConsequentialUncertainty);
    engine.register_ffi("spectral_authority", SpectralAuthority);
}
```

#### CLI Commands

```
braid query '[:find ?e ?name :where [?e :db/ident ?name]]'
braid query --file query.edn
braid query --mode monotonic '[:find ...]'    # Reject if non-monotonic
braid query --frontier agent-1 '[:find ...]'  # Query at specific frontier
```

---

### §3.4 Invariants

### INV-QUERY-001: CALM Compliance

**Traces to**: SEED §4 Axiom 4, ADRS FD-003, PO-013
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ monotonic queries Q, ∀ D ⊆ D':
  Q(D) ⊆ Q(D')
  (adding facts can only add results, never remove them)
```

#### Level 1 (State Invariant)
Queries declared as `Monotonic` mode MUST NOT contain negation or aggregation.
The query parser rejects non-monotonic constructs in Monotonic mode at parse time.

#### Level 2 (Implementation Contract)
```rust
impl QueryParser {
    pub fn parse(&self, expr: &str, mode: QueryMode) -> Result<QueryAst, QueryError> {
        let ast = self.parse_inner(expr)?;
        if mode == QueryMode::Monotonic && ast.has_negation_or_aggregation() {
            return Err(QueryError::NonMonotonicInMonotonicMode);
        }
        Ok(ast)
    }
}
```

**Falsification**: A query in Monotonic mode that contains negation or aggregation and
is not rejected at parse time.

---

### INV-QUERY-002: Query Determinism

**Traces to**: ADRS PO-013
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ queries Q, ∀ frontiers F:
  Q(S, F) at time t₁ = Q(S, F) at time t₂
  (identical expressions at identical frontiers return identical results)
```

#### Level 1 (State Invariant)
Query results are a pure function of the expression and the visible datom set.
No external randomness, no time-of-day dependency, no ordering dependency.

**Falsification**: Two evaluations of the same query at the same frontier returning
different results.

---

### INV-QUERY-003: Query Significance Tracking

**Traces to**: ADRS AS-007, PO-013
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
∀ queries Q executed against store S:
  an access event is recorded in the ACCESS LOG (separate from S)
  significance(d) = Σ decay(now - t) × query_weight(q) over queries returning d
```

#### Level 1 (State Invariant)
Every query generates an access event in the access log, NOT in the main store.
The access log feeds significance computation for ASSOCIATE.

**Falsification**: A query that completes without generating an access event, or
an access event recorded in the main store (violating AS-007's separation requirement).

---

### INV-QUERY-004: Branch Visibility

**Traces to**: ADRS AS-004
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
visible(branch b) = {d ∈ trunk | d.tx ≤ b.base_tx} ∪ {d | d.tx.branch = b}

Trunk commits after the fork point are NOT visible unless the branch rebases.
```

#### Level 1 (State Invariant)
A query against branch b sees exactly the trunk datoms at the fork point plus
the branch's own datoms. Snapshot isolation.

**Falsification**: A branch query that sees trunk datoms with tx > branch.base_tx
without an explicit rebase operation.

---

### INV-QUERY-005: Stratum Safety

**Traces to**: ADRS SQ-004, SQ-009
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ queries Q with stratum(Q) ∈ {0, 1}:         mode(Q) = Monotonic
∀ queries Q with stratum(Q) ∈ {2, 3}:         mode(Q) = Stratified
∀ queries Q with stratum(Q) = 4:              mode(Q) = Monotonic (conservative)
∀ queries Q with stratum(Q) = 5:              mode(Q) = Barriered (for critical decisions)
```

#### Level 1 (State Invariant)
The query engine classifies every query into a stratum and enforces the corresponding
mode constraint.

**Falsification**: A stratum 5 query executing in Monotonic mode.

---

### INV-QUERY-006: Semi-Naive Termination

**Traces to**: ADRS FD-003
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Semi-naive evaluation terminates iff the Datalog program is safe
(every variable in the head appears in a positive body literal).
Braid restricts to safe Datalog programs.

Termination: ΔT_P^(i+1) = ∅ after finitely many iterations
(because the Herbrand base is finite for a finite store).
```

#### Level 1 (State Invariant)
The parser rejects unsafe Datalog programs (unbound head variables).
Evaluation always terminates.

**Falsification**: A query that runs indefinitely (non-terminating fixpoint computation).

---

### INV-QUERY-007: Frontier as Queryable Data

**Traces to**: ADRS SQ-002, SQ-003
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Frontier is stored as :tx/frontier attribute.
Frontier = Map<AgentId, TxId> (vector-clock equivalent).

The Datalog extension [:frontier ?f] enables:
  "What does agent X know?" as an ordinary Datalog query.
```

#### Level 1 (State Invariant)
Frontier information is queryable via the same query engine as any other data.
No special-case API for frontier queries.

**Falsification**: Frontier data that is accessible only through a non-Datalog API.

---

### INV-QUERY-008: FFI Boundary Purity

**Traces to**: ADRS SQ-010
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
∀ derived functions f registered via FFI:
  f is a pure function: f(inputs) = f(inputs) always
  f has no side effects on the store
  Datalog provides the input query; f computes the result
```

#### Level 1 (State Invariant)
Three core computations are FFI: σ_a (entropy), σ_c (DAG traversal), spectral authority (SVD).
Each is a pure function from datom inputs to computed value.

**Falsification**: A derived function that modifies the store or returns different
results for identical inputs.

---

### INV-QUERY-009: Bilateral Query Symmetry

**Traces to**: ADRS SQ-006
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
The query layer is bilateral:
  Forward queries: spec → implementation status
  Backward queries: implementation → spec alignment

Both directions use the same Datalog apparatus. No asymmetric special-casing.
```

#### Level 1 (State Invariant)
For every forward query "does implementation X satisfy spec Y?" there is a symmetric
backward query "does spec Y accurately describe implementation X?"

**Falsification**: A forward query with no backward counterpart, or vice versa.

---

### INV-QUERY-010: Topology-Agnostic Results

**Traces to**: ADRS SQ-005
**Verification**: `V:MODEL`
**Stage**: 3

#### Level 0 (Algebraic Law)
```
∀ queries Q, ∀ dissemination topologies T₁, T₂:
  if all agents have received the same datom set:
    Q_T₁(S) = Q_T₂(S)
  (query results are independent of how datoms were distributed)
```

#### Level 1 (State Invariant)
Query results depend only on the datom set, not on the topology
(star, ring, mesh, hierarchy) used to distribute datoms.

**Falsification**: Two identical stores, assembled via different topologies, producing
different query results for the same expression.

---

### INV-QUERY-011: Projection Reification

**Traces to**: ADRS AS-008
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
∀ projection patterns P with access_count(P) > reification_threshold (default 3):
  P is stored as a first-class entity with significance score
  P is discoverable via ASSOCIATE
```

#### Level 1 (State Invariant)
Useful query patterns are promoted to entities, enabling the system to learn
"good ways to look at data."

**Falsification**: A projection pattern accessed 10+ times that is not stored as an entity.

---

### §3.5 ADRs

### ADR-QUERY-001: Datalog Over SQL

**Traces to**: SEED §4, §11, ADRS FD-003
**Stage**: 0

#### Problem
What query language should the datom store use?

#### Options
A) **Datalog** — declarative, natural graph joins, stratified evaluation maps to
   monotonic/non-monotonic distinction. CALM-compliant.
B) **SQL** — familiar but poor graph traversal. Requires recursive CTEs for transitive closure.
C) **Custom query language** — maximum flexibility but wheel reinvention.
D) **GraphQL** — web-oriented, not designed for formal verification.

#### Decision
**Option A.** Datalog's join semantics naturally express traceability queries
(goal → invariant → implementation → test). Stratified evaluation maps cleanly to
the monotonic/non-monotonic distinction (CALM theorem). Semi-naive evaluation avoids
redundant derivation.

#### Formal Justification
EAV triples are Datalog's native data model. The [entity, attribute, value] triple maps
directly to a Datalog fact `attr(entity, value)`. This eliminates the impedance mismatch
that SQL creates with EAV data.

---

### ADR-QUERY-002: Semi-Naive Bottom-Up Evaluation

**Traces to**: ADRS FD-003
**Stage**: 0

#### Problem
What evaluation strategy for Datalog?

#### Options
A) **Naive bottom-up** — iterate T_P until fixpoint. Correct but redundant.
B) **Semi-naive bottom-up** — only use new facts in each iteration. More efficient.
C) **Top-down (SLD resolution)** — goal-directed. Worse for materialized views.

#### Decision
**Option B.** Semi-naive avoids redundant derivation while maintaining bottom-up's
advantage for materialized views and incremental computation.

---

### ADR-QUERY-003: Six-Stratum Classification

**Traces to**: ADRS SQ-004, SQ-009
**Stage**: 0

#### Problem
How to organize query patterns by safety and cost?

#### Decision
Six strata: Stratum 0 (primitive, monotonic), Stratum 1 (graph traversal, monotonic),
Stratum 2 (uncertainty, mixed), Stratum 3 (authority, FFI), Stratum 4 (conflict detection,
conservatively monotonic), Stratum 5 (bilateral loop, non-monotonic).

The classification enables systematic safety analysis: Strata 0–1 are always safe.
Stratum 4 is safe but conservative (may overcount). Strata 2–3 and 5 require specific
frontier or barrier guarantees.

---

### ADR-QUERY-004: FFI for Derived Functions

**Traces to**: ADRS SQ-010
**Stage**: 1

#### Problem
Three core computations cannot be expressed in pure Datalog: σ_a (entropy), σ_c (DAG
traversal with memoization), spectral authority (SVD). How to handle this?

#### Options
A) **Extend Datalog** — add aggregation, recursion, linear algebra to the query language.
B) **FFI mechanism** — Datalog provides input data; Rust function computes result.
C) **Out-of-band computation** — separate process computes, results stored as datoms.

#### Decision
**Option B.** The FFI boundary cleanly separates declarative queries (Datalog's strength)
from imperative computation (Rust's strength). The derived function is pure — same inputs,
same output.

#### Formal Justification
Major architectural implication: three of four core coordination computations (σ_a, σ_c,
spectral authority) are derived functions. Option A would bloat the query language beyond
Datalog's well-understood theoretical properties. Option B preserves Datalog's properties
while enabling necessary computation.

---

### ADR-QUERY-005: Local Frontier as Default

**Traces to**: ADRS SQ-001
**Stage**: 0

#### Problem
What is the default query scope?

#### Options
A) **Local frontier only** — each agent sees only what it knows. No coordination.
B) **Consistent cut only** — all queries require sync barrier. Expensive.
C) **Local frontier default, consistent cut via optional sync barrier** — flexible.

#### Decision
**Option C.** Monotonic queries (Strata 0–1) are safe at any frontier, so local is fine.
Non-monotonic queries (Strata 2–5) may need a sync barrier for correctness-critical decisions,
but many non-monotonic queries produce useful approximate results at local frontier.

---

### ADR-QUERY-006: Frontier as Datom Attribute

**Traces to**: ADRS SQ-002, SQ-003
**Stage**: 0

#### Problem
Where is frontier information stored?

#### Options
A) **External metadata** — frontier in a separate data structure, not queryable via Datalog.
B) **Datom attribute** — `:tx/frontier` is a regular attribute, queryable like any other data.

#### Decision
**Option B.** Frontier as a datom attribute enables Datalog frontier clauses:
`[:frontier ?f]` queries "what does agent X know?" as ordinary data. No special-case API.

#### Formal Justification
Preserves FD-012 (every command is a transaction) — frontier updates are transactions.
Preserves schema-as-data (C3) — frontier structure is described by schema attributes.

---

### ADR-QUERY-007: Projection Pyramid

**Traces to**: ADRS SQ-007
**Stage**: 1

#### Problem
How to compress query results for budget-aware output?

#### Decision
Four-level projection pyramid:
- π₀: full datoms (>2000 tokens available)
- π₁: entity summaries (500–2000 tokens)
- π₂: type summaries (200–500 tokens)
- π₃: store summary (≤200 tokens — single-line status)

Selection is budget-driven: at high k*, full detail; at low k*, compressed pointers.

---

### ADR-QUERY-008: Bilateral Query Layer

**Traces to**: ADRS SQ-006
**Stage**: 1

#### Problem
How to structure the query layer for bilateral verification?

#### Decision
Queries naturally partition into:
- **Forward-flow** (planning): epistemic uncertainty, crystallization candidates,
  delegation, ready tasks
- **Backward-flow** (assessment): conflict detection, drift candidates, aleatory
  uncertainty, absorption triggers
- **Bridge** (both): commitment weight, consequential uncertainty, spectral authority

Spectral authority is the explicit bridge — updated by backward-flow observations,
consumed by forward-flow decisions.

---

### §3.6 Negative Cases

### NEG-QUERY-001: No Non-Monotonic Queries in Monotonic Mode

**Traces to**: ADRS PO-013, SQ-004
**Verification**: `V:TYPE`, `V:PROP`

**Safety property**: `□ ¬(∃ query Q in Monotonic mode containing negation or aggregation)`

**Rust type-level enforcement**: The `QueryMode::Monotonic` variant triggers a parse-time
check that rejects negation/aggregation constructs.

---

### NEG-QUERY-002: No Query Side Effects

**Traces to**: ADRS SQ-010
**Verification**: `V:TYPE`

**Safety property**: `□ ¬(∃ query evaluation that modifies the datom set)`
Queries are read-only over the datom set. The only write is the provenance transaction
(INV-STORE-014) and the access log event (INV-QUERY-003).

**Formal statement**: FFI derived functions have signature `fn(&[Value]) -> Value` —
no `&mut Store` parameter.

---

### NEG-QUERY-003: No Unbounded Query Evaluation

**Traces to**: ADRS FD-003
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ query that runs indefinitely)`
All accepted Datalog programs are safe (every head variable appears in a positive body
literal) and operate over a finite Herbrand base.

**proptest strategy**: Generate random safe Datalog programs over random stores.
Verify all evaluations terminate within a bounded number of iterations.

---

### NEG-QUERY-004: No Access Events in Main Store

**Traces to**: ADRS AS-007
**Verification**: `V:TYPE`, `V:PROP`

**Safety property**: `□ ¬(∃ access event stored as a datom in the main store)`
Access events go to the ACCESS LOG, never to the main datom store.

**Formal statement**: The access log is a separate append-only structure. Storing access
events as datoms would create unbounded positive feedback (querying generates events,
events are queryable, queries generate more events...).

---

## §4. RESOLUTION — Per-Attribute Conflict Resolution

### §4.0 Overview

Conflict resolution in Braid is per-attribute, not global. Different attributes have
different semantics and different natural resolution strategies. The resolution layer
operates at query time over the LIVE index, not during merge (merge is pure set union).

**Traces to**: SEED.md §4 Axiom 5
**ADRS.md sources**: FD-005, CR-001–007

---

### §4.1 Level 0: Algebraic Specification

#### Resolution as Semilattice

```
For each attribute a, the resolution mode defines a join-semilattice (V_a, ⊔_a):

Mode LWW (Last-Writer-Wins):
  V_a ordered by HLC timestamp
  v₁ ⊔ v₂ = v with max(v₁.tx, v₂.tx)
  Identity: ⊥ = no assertion

Mode Lattice:
  V_a ordered by a user-defined lattice L
  v₁ ⊔ v₂ = join_L(v₁, v₂)
  Identity: L.bottom

Mode Multi (Multi-Value):
  V_a = P(V) — power set of values
  v₁ ⊔ v₂ = v₁ ∪ v₂
  Identity: ∅

All three modes form semilattices, preserving CRDT semantics:
  ⊔ is commutative, associative, and idempotent.
```

#### Conflict Predicate

```
conflict(d₁, d₂) =
  d₁ = [e, a, v₁, t₁, Assert] ∧
  d₂ = [e, a, v₂, t₂, Assert] ∧
  v₁ ≠ v₂ ∧
  cardinality(a) = :one ∧
  ¬(t₁ < t₂) ∧ ¬(t₂ < t₁)

Critical: conflict requires CAUSAL INDEPENDENCE.
If one tx causally precedes the other, it is an update, not a conflict.
```

#### Resolution Composition

```
∀ attributes a, ∀ stores S:
  resolved_value(S, e, a) = resolution_mode(a).resolve(
    {d.v | d ∈ S, d.e = e, d.a = a, d.op = Assert, ¬retracted(S, d)}
  )

where retracted(S, d) = ∃ r ∈ S: r.e = d.e, r.a = d.a, r.v = d.v,
                                   r.op = Retract, r.tx > d.tx
```

---

### §4.2 Level 1: State Machine Specification

#### Three-Tier Conflict Routing

```
When conflict(d₁, d₂) is detected:

1. Compute severity = max(commitment_weight(d₁), commitment_weight(d₂))

2. Route by severity:
   TIER 1 — Automatic (low severity):
     Apply attribute's resolution mode (LWW/lattice/multi).
     Record resolution as datom. No human/agent notification.

   TIER 2 — Agent-with-Notification (medium severity):
     Apply resolution mode. Fire notification signal.
     Agent may override via deliberation.

   TIER 3 — Human-Required (high severity):
     Block resolution. Create Deliberation entity.
     Surface via TUI. Await human decision.

Severity thresholds are configurable as datoms.
```

#### Conflict Detection Pipeline

```
On MERGE or TRANSACT:
  1. For each new datom d = [e, a, v, tx, Assert] with cardinality(a) = :one:
     a. Find existing datom d' = [e, a, v', tx', Assert] where v ≠ v'
     b. Check causal independence: ¬(tx < tx') ∧ ¬(tx' < tx)
     c. If independent → assert Conflict entity
  2. Compute severity for each conflict
  3. Route to appropriate tier
  4. Update uncertainty (conflict increases σ_a for affected entity)
  5. Fire notification signals
  6. Invalidate cached query results for affected entities
```

#### Conservative Detection Invariant

```
conflicts_detected(frontier_local) ⊇ conflicts_actual(frontier_global)

Proof sketch: The causal-ancestor relation is monotonically growing.
Learning about new causal paths can only resolve apparent concurrency (discover that
two assertions are actually causally related), never create new concurrency.
An agent may waste effort on phantom conflicts (safe) but never miss a real one (critical).
```

---

### §4.3 Level 2: Interface Specification

```rust
/// Per-attribute resolution mode.
#[derive(Clone)]
pub enum ResolutionMode {
    /// Last-writer-wins, ordered by specified clock.
    Lww { clock: LwwClock },
    /// Join-semilattice resolution.
    Lattice { lattice_id: EntityId },
    /// Keep all values (cardinality :many semantics).
    Multi,
}

#[derive(Clone, Copy)]
pub enum LwwClock {
    Hlc,        // Hybrid Logical Clock (default)
    Wall,       // Wall-clock time
    AgentRank,  // Agent authority ranking
}

/// Conflict entity.
pub struct Conflict {
    pub entity: EntityId,
    pub attribute: Attribute,
    pub values: Vec<(Value, TxId)>,  // competing values with their transactions
    pub severity: f64,
    pub tier: ConflictTier,
    pub status: ConflictStatus,      // lattice: :detected < :routing < :resolving < :resolved
}

pub enum ConflictTier {
    Automatic,
    AgentNotification,
    HumanRequired,
}

/// Resolution result.
pub struct Resolution {
    pub conflict: EntityId,
    pub resolved_value: Value,
    pub method: ResolutionMethod,     // :lww | :lattice | :deliberation | :human
    pub rationale: String,
}

impl LiveIndex {
    /// Resolve current value for a cardinality-one attribute.
    pub fn resolve(&self, entity: EntityId, attr: &Attribute, schema: &Schema) -> Option<Value> {
        let mode = schema.resolution_mode(attr);
        let candidates = self.unretracted_values(entity, attr);
        mode.resolve(&candidates)
    }
}
```

#### CLI Commands

```
braid conflicts                           # List all unresolved conflicts
braid conflicts --entity <id>             # Conflicts for specific entity
braid resolve <conflict-id> --value <v>   # Manually resolve a conflict
braid resolve <conflict-id> --auto        # Apply automatic resolution
```

---

### §4.4 Invariants

### INV-RESOLUTION-001: Per-Attribute Resolution

**Traces to**: SEED §4 Axiom 5, ADRS FD-005
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ attributes a: ∃ resolution_mode(a) ∈ {LWW, Lattice, Multi}
  (every attribute declares its conflict resolution strategy)

resolution_mode is an attribute of the attribute entity:
  (attr_entity, :db/resolutionMode, mode, tx, Assert)
```

#### Level 1 (State Invariant)
No attribute exists without a declared resolution mode. The default (if not explicitly
set) is LWW with HLC clock.

#### Level 2 (Implementation Contract)
```rust
impl Schema {
    pub fn resolution_mode(&self, attr: &Attribute) -> ResolutionMode {
        self.attribute(attr)
            .and_then(|def| def.resolution_mode)
            .unwrap_or(ResolutionMode::Lww { clock: LwwClock::Hlc })
    }
}
```

**Falsification**: A conflict arising on an attribute with no defined resolution mode
and no default applied.

---

### INV-RESOLUTION-002: Resolution Commutativity

**Traces to**: ADRS AS-001, FD-005
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ resolution modes M, ∀ value sets V₁, V₂:
  M.resolve(V₁ ∪ V₂) = M.resolve(V₂ ∪ V₁)
  (resolution is order-independent — critical for CRDT consistency)
```

#### Level 1 (State Invariant)
Two agents independently resolving the same conflict arrive at the same value,
regardless of the order in which they receive the conflicting datoms.

**Falsification**: Two agents with the same datom set producing different resolved
values for the same entity-attribute pair.

**proptest strategy**: Generate random sets of conflicting values, resolve in all
permutations, assert identical results.

---

### INV-RESOLUTION-003: Conservative Conflict Detection

**Traces to**: ADRS CR-001
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ frontiers F_local, F_global where F_local ⊆ F_global:
  conflicts(F_local) ⊇ conflicts(F_global)
  (local frontier overestimates conflicts — no false negatives)
```

#### Level 1 (State Invariant)
An agent at a local frontier may see phantom conflicts (safe — wasted effort).
It MUST NOT miss real conflicts (critical — silent data corruption).

**Falsification**: A real conflict at the global frontier that is not detected at
some agent's local frontier that has received both conflicting datoms.

**Stateright model**: Model 3 agents independently transacting conflicting values.
Verify that every merge detects all conflicts, even with partial frontier views.

---

### INV-RESOLUTION-004: Conflict Predicate Correctness

**Traces to**: ADRS CR-006
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
conflict(d₁, d₂) ⟺
  same_entity(d₁, d₂) ∧ same_attribute(d₁, d₂) ∧
  different_value(d₁, d₂) ∧ both_assert(d₁, d₂) ∧
  cardinality_one(d₁.a) ∧ causally_independent(d₁.tx, d₂.tx)
```

#### Level 1 (State Invariant)
The conflict predicate requires ALL six conditions. Missing any condition
either misses real conflicts or flags non-conflicts:
- Without causal independence check: updates falsely flagged as conflicts
- Without cardinality check: multi-value attributes falsely flagged
- Without same-entity check: unrelated datoms falsely paired

**Falsification**: A pair of datoms satisfying all six conditions not flagged as conflict,
or a pair violating any condition that IS flagged.

**proptest strategy**: Generate datom pairs with systematic variation of each condition.
Verify conflict predicate matches expected boolean.

---

### INV-RESOLUTION-005: LWW Semilattice Properties

**Traces to**: ADRS FD-005
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
For LWW resolution with clock C:
  Commutativity: lww(v₁, v₂) = lww(v₂, v₁)
  Associativity: lww(lww(v₁, v₂), v₃) = lww(v₁, lww(v₂, v₃))
  Idempotency:   lww(v, v) = v
```

#### Level 1 (State Invariant)
LWW picks the value with the highest clock value. Ties broken by agent ID.

**Falsification**: Two agents resolving the same LWW conflict to different values.

---

### INV-RESOLUTION-006: Lattice Join Correctness

**Traces to**: ADRS SR-010, FD-005
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
For lattice resolution with lattice L:
  join_L(v₁, v₂) ≥ v₁ AND join_L(v₁, v₂) ≥ v₂    — upper bound
  ∀ u: u ≥ v₁ ∧ u ≥ v₂ ⟹ u ≥ join_L(v₁, v₂)       — least upper bound
  join_L(v₁, v₂) = join_L(v₂, v₁)                   — commutativity
```

#### Level 1 (State Invariant)
The lattice join produces the least upper bound of the competing values.
For diamond lattices (INV-SCHEMA-008), the join of two incomparable elements
produces the error signal element.

**Falsification**: A lattice join that is not the least upper bound, or
incomparable values in a diamond lattice that don't produce the error signal.

---

### INV-RESOLUTION-007: Three-Tier Routing Completeness

**Traces to**: ADRS CR-002
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ detected conflicts C:
  C is routed to exactly one of {Automatic, AgentNotification, HumanRequired}
  No conflict remains unrouted.
```

#### Level 1 (State Invariant)
Every detected conflict has a severity and a routing tier. The routing is total
(all conflicts are routed) and deterministic (same severity → same tier).

**Falsification**: A conflict that is detected but not routed to any tier.

---

### INV-RESOLUTION-008: Conflict Entity Datom Trail

**Traces to**: ADRS CR-003
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ conflict detections:
  Steps (1) assert Conflict entity, (2) compute severity, (3) route,
  (4) fire TUI, (5) update uncertainty, (6) invalidate caches
  ALL produce datoms in the store.
```

#### Level 1 (State Invariant)
The full conflict lifecycle is recorded as datoms, making it queryable
and auditable.

**Falsification**: Any step in the conflict pipeline that does not produce a datom.

---

### §4.5 ADRs

### ADR-RESOLUTION-001: Per-Attribute Over Global Policy

**Traces to**: SEED §4 Axiom 5, §11, ADRS FD-005
**Stage**: 0

#### Problem
Should conflict resolution be per-attribute or global?

#### Options
A) **Per-attribute** — each attribute declares its resolution mode (LWW, lattice, multi).
B) **Global policy** — one resolution strategy for all attributes.

#### Decision
**Option A.** Different attributes have different semantics. Task status has a natural
lattice (`todo < in-progress < done`). Person names do not. Forcing one resolution
policy on all attributes either loses information or produces nonsense.

#### Formal Justification
Per-attribute resolution preserves the semilattice property at the attribute level.
Global LWW would lose lattice semantics for status-like attributes. Global lattice
would require defining a lattice for every attribute, including those with no natural order.

---

### ADR-RESOLUTION-002: Resolution at Query Time, Not Merge Time

**Traces to**: C4, ADRS AS-001
**Stage**: 0

#### Problem
When does conflict resolution happen?

#### Options
A) **At merge time** — resolve conflicts during MERGE operation.
B) **At query time** — MERGE is pure set union; resolution happens in the LIVE index.

#### Decision
**Option B.** MERGE must be pure set union (C4). Conflict resolution at merge time
would make MERGE depend on schema and resolution mode, breaking the algebraic
properties (L1–L3 assume set union).

#### Formal Justification
If MERGE resolves conflicts, then `MERGE(S₁, S₂)` depends on schema — but schema is
itself data in the store. This creates a circular dependency that breaks L1–L3.
Resolution at query time avoids this: MERGE is always set union, and LIVE applies
resolution modes.

---

### ADR-RESOLUTION-003: Conservative Detection Over Precise

**Traces to**: ADRS CR-001
**Stage**: 0

#### Problem
Should conflict detection be conservative (may overcount) or precise (exact)?

#### Options
A) **Conservative** — flag potential conflicts even when uncertain.
   May waste effort on phantom conflicts. Never misses real conflicts.
B) **Precise** — only flag actual conflicts. Requires global knowledge.

#### Decision
**Option A.** The cost of a missed conflict (silent data corruption) far exceeds
the cost of a phantom conflict (wasted investigation effort). Conservative detection
is safe under partial information (local frontiers).

#### Formal Justification
Causal-ancestor relation is monotonically growing. Learning about new causal paths
can only resolve apparent concurrency, never create it. Conservative detection is
safe at any frontier.

---

### ADR-RESOLUTION-004: Three-Tier Routing

**Traces to**: ADRS CR-002
**Stage**: 0

#### Problem
How should conflicts be escalated?

#### Decision
Three tiers based on severity (commitment weight of conflicting datoms):
1. **Automatic** (low) — lattice/LWW per attribute. Recorded as datom.
2. **Agent-with-notification** (medium) — automatic + notification signal.
3. **Human-required** (high) — blocks. Creates Deliberation entity.

Severity = `max(commitment_weight(d₁), commitment_weight(d₂))`.
Thresholds configurable as datoms.

---

### ADR-RESOLUTION-005: Deliberation as Entity

**Traces to**: ADRS CR-004
**Stage**: 2

#### Problem
How to record conflict resolution decisions?

#### Decision
Three entity types: Deliberation (process), Position (stance), Decision (outcome).
Deliberation history forms a case law system — past decisions inform future conflicts
via the precedent query pattern (CR-007).

---

### §4.6 Negative Cases

### NEG-RESOLUTION-001: No Merge-Time Resolution

**Traces to**: C4
**Verification**: `V:TYPE`, `V:PROP`

**Safety property**: `□ ¬(∃ MERGE operation that applies conflict resolution)`
MERGE is pure set union. Conflict resolution happens at query time in the LIVE index.

**Rust type-level enforcement**: `fn merge(&mut self, other: &Store)` has no `Schema`
parameter. It cannot access resolution modes.

**proptest strategy**: Merge two stores with conflicting values. Verify both values
are present in the merged datom set (no automatic resolution during merge).

---

### NEG-RESOLUTION-002: No False Negative Conflict Detection

**Traces to**: ADRS CR-001
**Verification**: `V:MODEL`

**Safety property**: `□ ¬(∃ real conflict that is not detected at any frontier containing both datoms)`

**Stateright model**: 3 agents, each transacting conflicting values for shared entities.
Model all possible interleavings. Verify: if both conflicting datoms are in an agent's
frontier, the conflict is detected.

---

### NEG-RESOLUTION-003: No Resolution Without Provenance

**Traces to**: ADRS CR-003
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ conflict resolution that is not recorded as a datom)`
Every resolution — automatic, agent, or human — produces a datom trail.

**proptest strategy**: Trigger conflicts via random transactions. Verify every resolution
produces a Resolution entity in the store.

---

*Sections §5–§14 (Wave 2–3 namespaces) and §15–§17 (integration) will be produced in
subsequent sessions following the same three-level refinement methodology.*

---

## Appendix A: Element Count Summary (Wave 1)

| Namespace | INV | ADR | NEG | Total |
|-----------|-----|-----|-----|-------|
| STORE     | 14  | 12  | 5   | 31    |
| SCHEMA    | 8   | 4   | 3   | 15    |
| QUERY     | 11  | 8   | 4   | 23    |
| RESOLUTION| 8   | 5   | 3   | 16    |
| **Total** | **41** | **29** | **15** | **85** |

## Appendix B: Verification Coverage (Wave 1)

| Tag | Count | Namespaces |
|-----|-------|------------|
| V:PROP | 41/41 | All (minimum requirement met) |
| V:KANI | 22 | STORE (10), SCHEMA (3), QUERY (3), RESOLUTION (6) |
| V:MODEL | 5 | STORE (1), QUERY (1), RESOLUTION (3) |
| V:TYPE | 9 | STORE (2), SCHEMA (2), QUERY (3), RESOLUTION (2) |
| V:CONTRACT | 0 | (Applied during implementation, not spec) |
| V:DEDUCTIVE | 0 | (Candidate: INV-STORE-004/005/006 — CRDT laws) |
| V:MIRI | 0 | (Applied during implementation for unsafe code) |

## Appendix C: Stage 0 Elements

Elements required for Stage 0 (Harvest/Seed cycle):

| Element | Namespace | Summary |
|---------|-----------|---------|
| INV-STORE-001–012, 014 | STORE | Core store operations |
| INV-SCHEMA-001–007 | SCHEMA | Schema bootstrap |
| INV-QUERY-001–002, 005–007 | QUERY | Core query engine |
| INV-RESOLUTION-001–002, 004–008 | RESOLUTION | Basic conflict handling |
| ADR-STORE-001–012 | STORE | Foundation decisions |
| ADR-SCHEMA-001–003 | SCHEMA | Schema decisions |
| ADR-QUERY-001–003, 005–006 | QUERY | Query engine decisions |
| ADR-RESOLUTION-001–004 | RESOLUTION | Resolution decisions |
| NEG-STORE-001–005 | STORE | Store safety |
| NEG-SCHEMA-001–003 | SCHEMA | Schema safety |
| NEG-QUERY-001–004 | QUERY | Query safety |
| NEG-RESOLUTION-001–003 | RESOLUTION | Resolution safety |
