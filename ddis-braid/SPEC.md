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

## §5. HARVEST — End-of-Session Extraction

### §5.0 Overview

Harvest is the mechanism by which knowledge survives conversation boundaries. At the end
of a conversation (or when context budget is critically low), the agent extracts durable
knowledge — observations, decisions, dependencies, uncertainties — from the ephemeral
conversation into the permanent datom store.

The fundamental insight: **conversations are disposable; knowledge is durable.** Harvest
transforms the workflow from "fight to keep conversations alive" to "ride bounded context
waves, extracting knowledge at each crest."

**Traces to**: SEED.md §5
**ADRS.md sources**: LM-005–006, LM-011–013, IB-012, CR-005, UA-007

---

### §5.1 Level 0: Algebraic Specification

#### Epistemic Gap

```
Let K_agent(t) = knowledge held by the agent at time t (in conversation context)
Let K_store(t) = knowledge in the datom store at time t

Epistemic gap: Δ(t) = K_agent(t) \ K_store(t)
  (knowledge the agent has that the store does not)

Harvest: HARVEST(Δ(t)) → K_store(t') where K_store(t') ⊇ K_store(t) ∪ Δ(t)
  The store grows to include the agent's un-transacted knowledge.

Perfect harvest: Δ(t') = ∅
  (all agent knowledge is in the store after harvest)

Practical harvest: |Δ(t')| ≤ ε
  (residual gap below acceptable threshold)
```

#### Harvest as Monotonic Extension

```
∀ harvest operations H:
  K_store(pre) ⊆ K_store(post)          — store grows (C1)
  |K_store(post)| ≥ |K_store(pre)|      — never shrinks (L5)

Harvest does not modify existing datoms. It only adds new datoms
representing the agent's un-transacted observations and decisions.
```

#### Harvest Quality Metrics

```
false_positive_rate = |{candidates committed then later retracted}| / |{committed}|
false_negative_rate = |{candidates rejected then later re-discovered}| / |{rejected}|
drift_score = |Δ(t)| at session end before harvest

Calibration: High FP → raise thresholds. High FN → lower thresholds.
Both high → improve extractor.

Quality bands:
  0–2 uncommitted observations at harvest time = excellent
  3–5 = minor drift
  6+  = significant drift (methodology not followed)
```

---

### §5.2 Level 1: State Machine Specification

#### Harvest Pipeline

```
HARVEST(S, agent, transcript_context) → S'

PIPELINE:
  1. DETECT: Scan agent's recent transactions. Identify:
     - Observations made but not transacted (implicit knowledge)
     - Decisions made but not recorded as ADR datoms
     - Dependencies discovered but not linked
     - Uncertainties encountered but not marked

  2. PROPOSE: Generate harvest candidates.
     Each candidate c has:
       c.datom_spec    — the datom(s) to transact
       c.category      — observation | decision | dependency | uncertainty
       c.confidence    — extraction confidence (0.0–1.0)
       c.weight        — commitment weight estimate

  3. REVIEW: Agent/human confirms or rejects each candidate.
     Review topology (LM-012) determines who reviews:
       single-agent self-review (default)
       bilateral peer review
       swarm broadcast + voting
       hierarchical specialist delegation
       human review

  4. COMMIT: Confirmed candidates transacted as datoms.
     Each committed candidate becomes a Transaction with:
       provenance = :observed or :derived
       rationale = harvest extraction context
       causal_predecessors = session's transaction chain

  5. RECORD: Harvest session entity created.
     Records: session_id, agent, topology, candidate_count,
     committed_count, rejected_count, drift_score, timestamp.

POST:
  S'.datoms ⊇ S.datoms                          — monotonic (C1)
  harvest_session entity in S'                    — provenance trail
  ∀ committed candidates: datoms in S'            — knowledge captured
  drift_score(S') recorded for calibration        — learning signal
```

#### Proactive Harvest Warnings

```
When Q(t) < 0.15 (~75% context consumed):
  Every CLI response includes harvest warning.
  "Context budget low. Run `braid harvest` to preserve session knowledge."

When Q(t) < 0.05 (~85% context consumed):
  CLI emits ONLY the harvest imperative.
  "HARVEST NOW. Run `braid harvest`. Further work will degrade."

Continuing past harvest threshold produces diminishing returns —
outputs become parasitic (consuming budget without producing value).
```

#### Crystallization Stability Guard

```
Harvest candidates with high commitment weight require stability check:

crystallizable(candidate) =
  candidate.status = :refined ∧
  candidate.confidence ≥ 0.6 ∧
  candidate.coherence ≥ 0.6 ∧
  no_unresolved_conflicts(candidate) ∧
  stability_score(candidate) ≥ stability_min (default 0.7)

Candidates below stability threshold remain as :proposed in the harvest
session, not committed. They surface in the next session's seed as
"pending crystallization."
```

#### Observation Staleness Model

```
Observation datoms carry freshness metadata:
  :observation/source    — :filesystem | :shell | :network | :git | :process
  :observation/timestamp — when observed
  :observation/hash      — content hash at observation time
  :observation/stale-after — TTL (source-dependent)

Freshness check during harvest:
  if now - observation.timestamp > stale_after:
    flag as potentially stale
    ASSEMBLE applies freshness-mode: :warn (default) | :refresh | :accept
```

---

### §5.3 Level 2: Interface Specification

```rust
/// Harvest candidate — proposed datom extraction from conversation.
pub struct HarvestCandidate {
    pub datom_spec: Vec<Datom>,
    pub category: HarvestCategory,
    pub confidence: f64,            // 0.0–1.0
    pub weight: f64,                // estimated commitment weight
    pub status: CandidateStatus,    // lattice: :proposed < :under-review < :committed < :rejected
    pub extraction_context: String, // why this was extracted
}

pub enum HarvestCategory {
    Observation,     // fact observed but not transacted
    Decision,        // choice made but not recorded as ADR
    Dependency,      // link discovered but not asserted
    Uncertainty,     // unknown encountered but not marked
}

/// Harvest session entity.
pub struct HarvestSession {
    pub session_id: EntityId,
    pub agent: AgentId,
    pub review_topology: ReviewTopology,
    pub candidates: Vec<HarvestCandidate>,
    pub drift_score: u32,           // count of uncommitted observations
    pub timestamp: Instant,
}

pub enum ReviewTopology {
    SelfReview,                     // single agent reviews own work
    PeerReview { reviewer: AgentId },
    SwarmVote { quorum: u32 },
    HierarchicalDelegation { specialist: AgentId },
    HumanReview,
}

impl Store {
    /// Detect and propose harvest candidates.
    pub fn harvest_detect(&self, agent: AgentId) -> Vec<HarvestCandidate>;

    /// Commit confirmed candidates.
    pub fn harvest_commit(
        &mut self,
        agent: AgentId,
        candidates: &[HarvestCandidate],
        topology: ReviewTopology,
    ) -> Result<HarvestSession, HarvestError>;
}
```

#### CLI Commands

```
braid harvest                       # Interactive: detect, propose, review, commit
braid harvest --auto                # Auto-commit candidates above confidence threshold
braid harvest --dry-run             # Show candidates without committing
braid harvest --topology peer       # Use peer review topology
braid harvest --stats               # Show harvest quality metrics (FP/FN rates)
```

---

### §5.4 Invariants

### INV-HARVEST-001: Harvest Monotonicity

**Traces to**: SEED §5, C1
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ harvest operations H on store S:
  S ⊆ HARVEST(S)
  (harvest only adds datoms, never removes)
```

#### Level 1 (State Invariant)
Every harvest commit is a TRANSACT operation, inheriting all STORE invariants.
No existing datom is modified or removed during harvest.

#### Level 2 (Implementation Contract)
```rust
// Harvest commit delegates to Store::transact, which preserves INV-STORE-001.
pub fn harvest_commit(&mut self, ...) -> Result<HarvestSession, HarvestError> {
    let tx = Transaction::<Building>::new(agent);
    for candidate in confirmed_candidates {
        for datom in &candidate.datom_spec {
            tx = tx.assert_datom(datom.entity, datom.attribute.clone(), datom.value.clone());
        }
    }
    let tx = tx.commit(&self.schema)?;
    self.transact(tx)?;
    // ...
}
```

**Falsification**: A harvest operation that reduces the datom count or removes existing datoms.

---

### INV-HARVEST-002: Harvest Provenance Trail

**Traces to**: SEED §5, ADRS FD-012
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ harvest operations:
  ∃ HarvestSession entity in S' recording:
    agent, timestamp, candidate_count, drift_score, topology
  ∀ committed candidates:
    ∃ transaction with provenance tracing to the harvest session
```

#### Level 1 (State Invariant)
Every harvest creates a HarvestSession entity. Every committed candidate has a
transaction whose causal predecessors include the harvest session entity.

**Falsification**: A harvest that commits candidates without creating a HarvestSession
entity, or candidates whose transactions have no provenance link to the session.

---

### INV-HARVEST-003: Drift Score Recording

**Traces to**: ADRS LM-006
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ harvest sessions:
  drift_score = |uncommitted observations| at harvest time
  drift_score is stored as a datom on the HarvestSession entity
```

#### Level 1 (State Invariant)
The drift score is recorded per session, enabling longitudinal tracking of
harvest discipline. Quality bands: 0–2 = excellent, 3–5 = minor, 6+ = significant.

**Falsification**: A harvest session entity without a drift_score attribute.

---

### INV-HARVEST-004: FP/FN Calibration

**Traces to**: ADRS LM-006
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
∀ committed candidates c:
  if c is later retracted: FP_count += 1
∀ rejected candidates c:
  if c's knowledge is later re-discovered: FN_count += 1

Calibration rule:
  FP_rate > threshold → raise extraction confidence threshold
  FN_rate > threshold → lower extraction confidence threshold
  Both high → improve extractor (not just thresholds)
```

#### Level 1 (State Invariant)
The harvest system tracks empirical quality and adjusts thresholds.
False positives and false negatives are both measurable from the store.

**Falsification**: Harvest thresholds that never adjust despite persistent FP/FN rates.

---

### INV-HARVEST-005: Proactive Warning

**Traces to**: ADRS IB-012
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ CLI responses when Q(t) < 0.15:
  response includes harvest warning
∀ CLI responses when Q(t) < 0.05:
  response = ONLY harvest imperative (no other content)
```

#### Level 1 (State Invariant)
The budget system triggers harvest warnings at context consumption thresholds.
Below the critical threshold, all output is suppressed except the harvest command.

**Falsification**: A CLI response at Q(t) < 0.05 that contains content other than
the harvest imperative.

---

### INV-HARVEST-006: Crystallization Guard

**Traces to**: ADRS CR-005
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
∀ harvest candidates c with commitment_weight(c) > crystallization_threshold:
  c MUST NOT be committed unless:
    c.confidence ≥ 0.6 ∧
    c.coherence ≥ 0.6 ∧
    no_unresolved_conflicts(c) ∧
    stability_score(c) ≥ stability_min
```

#### Level 1 (State Invariant)
High-weight candidates require stability verification before commitment.
This prevents premature crystallization of uncertain knowledge into
load-bearing datoms.

**Falsification**: A high-weight candidate committed with stability_score below threshold.

**proptest strategy**: Generate candidates with varying weights and stability scores.
Verify that only stable, high-confidence candidates with weights above threshold
pass the crystallization guard.

---

### INV-HARVEST-007: Bounded Conversation Lifecycle

**Traces to**: ADRS LM-011
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Agent lifecycle is a bounded cycle:
  SEED → work(20–30 turns) → HARVEST → conversation_end → SEED → ...

Each conversation is a bounded trajectory:
  high-quality reasoning for a limited window before attention degrades.
  Produces: durable knowledge (datoms) + ephemeral reasoning (conversation).
  At end: ephemeral released, durable persists.
```

#### Level 1 (State Invariant)
The system enforces a bounded lifecycle through proactive warnings (INV-HARVEST-005)
and budget monitoring. Conversations that exceed the attention degradation threshold
without harvesting produce lower-quality output.

**Falsification**: An agent operating for 50+ turns without a harvest or harvest warning.

---

### INV-HARVEST-008: Delegation Topology Support

**Traces to**: ADRS LM-012
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
∀ harvest sessions: topology ∈ {self, peer, swarm, hierarchical, human}

Topology selection based on commitment weight:
  auto_threshold = 0.15: self-review sufficient
  peer_threshold = 0.40: peer review recommended
  human_threshold = 0.70: human review required

harvest_weight(candidate) = intrinsic_weight(candidate) × confidence(extraction)
```

#### Level 1 (State Invariant)
High-weight harvest candidates are routed to higher-authority review topologies.
The topology is recorded on the HarvestSession entity.

**Falsification**: A harvest session with high-weight candidates using self-review topology.

---

### §5.5 ADRs

### ADR-HARVEST-001: Semi-Automated Over Fully Automatic

**Traces to**: ADRS LM-005
**Stage**: 0

#### Problem
Should harvest be fully automatic or require agent/human confirmation?

#### Options
A) **Fully automatic** — system extracts and commits without review.
B) **Semi-automated** — system proposes candidates; agent/human confirms.
C) **Fully manual** — agent must explicitly identify all harvestable knowledge.

#### Decision
**Option B.** The system detects harvestable knowledge from transaction analysis and
presents candidates for confirmation. This balances extraction coverage (higher than C)
with precision (lower FP rate than A).

#### Formal Justification
Fully automatic harvest (Option A) risks high false positive rates — committing
speculative observations as established facts. Fully manual (Option C) risks high
false negative rates — agents forgetting to harvest key decisions. Semi-automated
balances both failure modes and provides calibration data (FP/FN rates) for improvement.

---

### ADR-HARVEST-002: Conversations Disposable, Knowledge Durable

**Traces to**: SEED §5, ADRS LM-003
**Stage**: 0

#### Problem
What is the relationship between conversations and durable state?

#### Options
A) **Conversations are disposable** — knowledge extracted to store; conversation discarded.
B) **Conversations are archival** — full transcripts preserved alongside store.
C) **Conversations are primary** — store is an index into conversations.

#### Decision
**Option A.** Conversations are bounded reasoning trajectories. Knowledge lives in the
store. Conversations are lightweight and replaceable — start one, work 20–30 turns,
harvest, discard, start fresh. The agent never loses anything.

#### Formal Justification
Option B preserves too much — conversation transcripts are voluminous and mostly
redundant with the extracted datoms. Option C inverts the architecture — makes the
store dependent on ephemeral artifacts. Option A aligns with the harvest/seed lifecycle:
knowledge survives; reasoning sessions do not.

---

### ADR-HARVEST-003: FP/FN Tracking for Calibration

**Traces to**: ADRS LM-006
**Stage**: 1

#### Problem
How to improve harvest quality over time?

#### Decision
Track empirical FP/FN rates per agent and per category. A committed candidate later
retracted is a false positive. A rejected candidate whose knowledge is later re-discovered
is a false negative. Rates feed back into threshold adjustment.

#### Formal Justification
Harvest quality is measurable from the store: retractions of harvest-committed datoms
and re-discoveries of rejected candidates are both detectable by querying transaction
history. This makes harvest improvement a data-driven process.

---

### ADR-HARVEST-004: Five Review Topologies

**Traces to**: ADRS LM-012
**Stage**: 2

#### Problem
Who reviews harvest candidates?

#### Decision
Five topologies: (1) self-review (default — agent reviews own harvest), (2) bilateral
peer review (a second agent reviews), (3) swarm broadcast + voting (multiple agents vote),
(4) hierarchical specialist delegation (route to domain expert), (5) human review.

The "Fresh-Agent Self-Review" pattern exploits maximum context asymmetry: the depleted
agent proposes candidates, a fresh session reviews them with full attention budget.

---

### §5.6 Negative Cases

### NEG-HARVEST-001: No Unharvested Session Termination

**Traces to**: SEED §5, ADRS IB-012
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ session termination with drift_score > 0 and no harvest warning issued)`

**Formal statement**: Every session that ends with uncommitted observations MUST have
issued at least one harvest warning before termination. The warning is triggered by
Q(t) threshold crossing, not by session end detection.

**proptest strategy**: Simulate sessions with varying transaction/observation ratios.
Verify that all sessions with uncommitted observations receive harvest warnings.

---

### NEG-HARVEST-002: No Harvest Data Loss

**Traces to**: C1
**Verification**: `V:PROP`, `V:KANI`

**Safety property**: `□ ¬(∃ committed harvest candidate whose datoms are not in the store post-harvest)`

**Formal statement**: Every candidate with `status = :committed` has its datom_spec
present in `S'.datoms` after the harvest transaction completes.

**Kani harness**: Bounded check that for any set of committed candidates, all specified
datoms appear in the post-harvest store.

---

### NEG-HARVEST-003: No Premature Crystallization

**Traces to**: ADRS CR-005
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ high-weight candidate committed with stability < stability_min)`

**proptest strategy**: Generate harvest sessions with candidates of varying weight and
stability. Verify that no high-weight candidate bypasses the crystallization guard.

---

## §6. SEED — Start-of-Session Assembly

### §6.0 Overview

Seed is the complement of harvest: where harvest extracts knowledge at session end,
seed assembles relevant knowledge at session start. The seed provides a fresh agent
with full relevant context, zero irrelevant noise, and a fresh attention budget.

The seed collapses three concerns into one mechanism: ambient awareness (CLAUDE.md),
guidance (methodology steering), and trajectory management (carry-over from prior sessions).

**Traces to**: SEED.md §5, §8
**ADRS.md sources**: IB-010, PO-002, PO-003, PO-014, GU-004, SQ-007

---

### §6.1 Level 0: Algebraic Specification

#### Seed as Projection

```
SEED : Store × TaskContext × Budget → AssembledContext

SEED(S, task, k*) = ASSEMBLE(QUERY(ASSOCIATE(S, task)), k*)

The seed is a projection of the store onto the relevant subset,
compressed to fit the available attention budget.

Formally: SEED = assemble ∘ query ∘ associate
  where associate : Store × TaskContext → SchemaNeighborhood
        query     : SchemaNeighborhood → QueryResult
        assemble  : QueryResult × Budget → AssembledContext
```

#### Assembly Priority Function

```
For each entity e in the query result:
  score(e) = α × relevance(e, task) + β × significance(e) + γ × recency(e)
  where α = 0.5, β = 0.3, γ = 0.2 (defaults, configurable as datoms)

Assembly selects entities in score order until budget is exhausted.
Higher-priority entities get richer projections (π₀ → π₁ → π₂ → π₃).
```

#### Dynamic CLAUDE.md as Seed

```
GENERATE-CLAUDE-MD : Store × Focus × Agent × Budget → Markdown

The dynamic CLAUDE.md collapses three concerns:
  1. Ambient awareness (Layer 0) — CLAUDE.md IS the ambient context
  2. Guidance (Layer 3) — seed context IS the first guidance (zero tool-call cost)
  3. Trajectory management — CLAUDE.md IS the seed turn

Seven-step generation:
  (1) ASSOCIATE with focus
  (2) QUERY active intentions
  (3) QUERY governing invariants
  (4) QUERY uncertainty markers
  (5) QUERY competing branches
  (6) QUERY drift patterns
  (7) ASSEMBLE at budget

Priority ordering: tools > task_context > risks > drift_corrections > seed_context
```

---

### §6.2 Level 1: State Machine Specification

#### ASSOCIATE — Schema Discovery

```
ASSOCIATE(S, cue) → SchemaNeighborhood

Two modes:
  SemanticCue(text): natural language → schema search → graph expansion
  ExplicitSeeds([EntityId]): start from known entities → graph expansion

POST:
  |result| ≤ depth × breadth (bounded)
  high-significance entities preferred (AS-007)
  learned associations traversed alongside structural edges (AA-004)

SchemaNeighborhood = {entities, attributes, types} — NOT values
  (schema-level discovery, not data retrieval)
```

#### ASSEMBLE — Rate-Distortion Context

```
ASSEMBLE(query_results, schema_neighborhood, budget) → AssembledContext

PRE:
  budget > 0

PIPELINE:
  1. Score entities: score(e) = α×relevance + β×significance + γ×recency
  2. Sort by score (descending)
  3. For each entity in order:
     a. Select projection level based on remaining budget:
        >2000 tokens: π₀ (full datoms) for top entities, π₁ for others
        500–2000:     π₁/π₂
        200–500:      π₂ for top, omit others
        ≤200:         single-line status + single guidance action
     b. Subtract token cost from remaining budget
     c. If budget exhausted, stop
  4. Pin intentions at π₀ regardless of budget (INV-ASSEMBLE-INTENTION-001)
  5. Record projection pattern for reification learning (AS-008)
  6. Check staleness for observation entities (UA-007)

POST:
  |result| ≤ budget (token count)
  structural dependency coherence (no entity without its dependencies)
  all active intentions included
```

#### Seed Output Template

```
Seed output follows a five-part template:
  (1) Context — 1–2 sentences: what was last worked on, current project state
  (2) Invariants — active invariants governing the next task
  (3) Artifacts — files modified, decisions made, entities created
  (4) Open questions — from deliberations, uncertainties, pending crystallizations
  (5) Active guidance — next methodologically correct actions

Formatted as spec-language (INV-GUIDANCE-SEED-001): invariants and formal
structure, NOT instruction-language (steps, checklists).
```

---

### §6.3 Level 2: Interface Specification

```rust
/// Schema neighborhood — what ASSOCIATE discovers.
pub struct SchemaNeighborhood {
    pub entities: Vec<EntityId>,
    pub attributes: Vec<Attribute>,
    pub entity_types: Vec<Keyword>,
}

/// Assembled context — what ASSEMBLE produces.
pub struct AssembledContext {
    pub sections: Vec<ContextSection>,
    pub total_tokens: usize,
    pub budget_remaining: usize,
    pub projection_pattern: ProjectionPattern,
}

pub struct ContextSection {
    pub entity: EntityId,
    pub projection_level: ProjectionLevel,
    pub content: String,
    pub score: f64,
}

pub enum ProjectionLevel {
    Full,       // π₀ — all datoms
    Summary,    // π₁ — entity summary
    TypeLevel,  // π₂ — type summary
    Pointer,    // π₃ — single-line reference
}

/// Dynamic CLAUDE.md generator.
pub struct ClaudeMdGenerator {
    pub store: Store,
}

impl ClaudeMdGenerator {
    /// Generate dynamic CLAUDE.md for a session.
    pub fn generate(
        &self,
        focus: &str,
        agent: AgentId,
        budget: usize,
    ) -> Result<String, SeedError>;
}

impl Store {
    /// ASSOCIATE — discover relevant schema neighborhood.
    pub fn associate(&self, cue: AssociateCue) -> SchemaNeighborhood;

    /// ASSEMBLE — build budget-aware context.
    pub fn assemble(
        &self,
        query_results: &QueryResult,
        neighborhood: &SchemaNeighborhood,
        budget: usize,
    ) -> AssembledContext;

    /// SEED — full pipeline: associate → query → assemble.
    pub fn seed(&mut self, task: &str, budget: usize) -> Result<AssembledContext, SeedError>;
}
```

#### CLI Commands

```
braid seed --task "implement datom store"     # Full seed for task
braid seed --budget 2000                      # With explicit token budget
braid associate "conflict resolution"         # Schema neighborhood only
braid assemble --budget 500                   # Assemble from last query
braid claude-md --focus "stage 0"             # Generate dynamic CLAUDE.md
```

---

### §6.4 Invariants

### INV-SEED-001: Seed as Store Projection

**Traces to**: SEED §5, ADRS IB-010
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ seed operations: SEED(S, task, k*) ⊆ S
  (the seed contains only information from the store — nothing fabricated)
```

#### Level 1 (State Invariant)
Every datum in the seed output traces to a datom in the store.
The seed is a view, not a source of truth.

**Falsification**: Any claim in the seed output that does not correspond to a datom in the store.

---

### INV-SEED-002: Budget Compliance

**Traces to**: ADRS IB-004, PO-003
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ ASSEMBLE operations with budget B:
  |output| ≤ B (in tokens)
```

#### Level 1 (State Invariant)
The assembled context never exceeds the declared budget. If the relevant
information exceeds the budget, lower-priority content is dropped (projected
to coarser levels), never the budget exceeded.

**Falsification**: An ASSEMBLE output whose token count exceeds the budget parameter.

**proptest strategy**: Generate stores of varying sizes. Assemble with varying budgets.
Verify output token count ≤ budget for all combinations.

---

### INV-SEED-003: ASSOCIATE Boundedness

**Traces to**: ADRS PO-002
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ ASSOCIATE operations with depth d and breadth b:
  |result.entities| ≤ d × b
```

#### Level 1 (State Invariant)
ASSOCIATE graph expansion is bounded to prevent unbounded traversal.
The bound is `depth × breadth`, both configurable.

**Falsification**: An ASSOCIATE result with more entities than `depth × breadth`.

---

### INV-SEED-004: Intention Anchoring

**Traces to**: ADRS AA-005, PO-003
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ ASSEMBLE operations with include_intentions=true:
  ∀ active intentions I: I ∈ assembled_context at projection level π₀
  regardless of budget pressure
```

#### Level 1 (State Invariant)
Active intentions are pinned at full detail (π₀) even when the budget would
otherwise compress or omit them. Intentions are never sacrificed for budget.

**Falsification**: An active intention omitted from the assembled context when
`include_intentions=true`, or projected below π₀.

---

### INV-SEED-005: Dynamic CLAUDE.md Relevance

**Traces to**: ADRS PO-014
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
∀ sections s in GENERATE-CLAUDE-MD output:
  removing s would change agent behavior
  (no irrelevant padding or boilerplate)
```

#### Level 1 (State Invariant)
Every section of the dynamic CLAUDE.md is relevant to the declared focus.
Irrelevant sections waste attention budget.

**Falsification**: A section in the generated CLAUDE.md that, if removed, would not
change agent behavior (deadweight content).

---

### INV-SEED-006: Dynamic CLAUDE.md Improvement

**Traces to**: ADRS PO-014
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
∀ drift corrections in GENERATE-CLAUDE-MD:
  correction derived from empirical drift data (not speculation)
  corrections showing no effect after 5 sessions → replaced
```

#### Level 1 (State Invariant)
Drift corrections are data-driven. The system tracks which corrections
change agent behavior and removes ineffective ones.

**Falsification**: A drift correction that has been included for 5+ sessions
with no measurable effect on agent behavior, and is not replaced.

---

### §6.5 ADRs

### ADR-SEED-001: Three-Concern Collapse

**Traces to**: ADRS GU-004
**Stage**: 0

#### Problem
How to handle ambient awareness, guidance, and trajectory management?

#### Options
A) **Three separate mechanisms** — CLAUDE.md for awareness, guidance API for steering,
   seed file for carry-over.
B) **Single mechanism** — dynamic CLAUDE.md that collapses all three.

#### Decision
**Option B.** One mechanism, three problems solved. CLAUDE.md IS the ambient awareness
(Layer 0). The seed context IS the first guidance (pre-computed, zero tool-call cost).
CLAUDE.md IS the seed turn (trajectory management).

#### Formal Justification
Option A triples the attention cost: agent must process three separate information
sources. Option B is rate-distortion optimal: one compressed channel carrying all
three signals, prioritized by the budget system.

---

### ADR-SEED-002: Rate-Distortion Assembly

**Traces to**: ADRS IB-011
**Stage**: 0

#### Problem
How to compress knowledge to fit the attention budget?

#### Decision
Rate-distortion theory: maximize information value while minimizing attention cost.
The projection pyramid (π₀ → π₃) provides controlled lossy compression. The score
function (α×relevance + β×significance + γ×recency) determines what survives.

#### Formal Justification
The attention budget is a hard constraint (INV-SEED-002). Within that constraint,
the score function and projection pyramid maximize information value — high-relevance,
high-significance, recent entities get richer projections; low-value entities get
compressed or omitted.

---

### ADR-SEED-003: Spec-Language Over Instruction-Language

**Traces to**: ADRS GU-003
**Stage**: 0

#### Problem
What style should seed output use?

#### Options
A) **Instruction-language** — "Step 1: do X. Step 2: do Y." (checklists, procedures)
B) **Spec-language** — invariants, formal structure, constraints.

#### Decision
**Option B.** Spec-language activates the deep formal-methods substrate in the LLM.
Instruction-language activates the surface procedural substrate. Spec-language produces
more rigorous, consistent output because it frames the task as constraint satisfaction
rather than instruction following.

---

### §6.6 Negative Cases

### NEG-SEED-001: No Fabricated Context

**Traces to**: C5
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ claim in seed output not traceable to a datom)`

**proptest strategy**: For each entity in the seed output, verify a corresponding
datom exists in the store. Flag any content without store backing.

---

### NEG-SEED-002: No Budget Overflow

**Traces to**: ADRS IB-004
**Verification**: `V:PROP`, `V:KANI`

**Safety property**: `□ ¬(∃ ASSEMBLE output exceeding declared budget)`

**Kani harness**: For all stores of size ≤ N and budgets ≤ M, verify output ≤ budget.

---

## §7. MERGE — Store Merge & CRDT

### §7.0 Overview

Merge combines knowledge from independent agents. The core operation is set union (C4),
but merge also triggers a cascade of consequences: conflict detection, cache invalidation,
uncertainty updates, and subscription notifications. The branching extension (W_α, patch
branches) provides isolated workspaces with explicit commit to the shared store.

**Traces to**: SEED.md §6
**ADRS.md sources**: AS-001, AS-003–006, AS-010, PD-001, PD-004, PO-006, PO-007

---

### §7.1 Level 0: Algebraic Specification

#### Core Merge

```
MERGE : Store × Store → Store
MERGE(S₁, S₂) = S₁ ∪ S₂

Properties (from STORE namespace, restated for completeness):
  L1: MERGE(S₁, S₂) = MERGE(S₂, S₁)           — commutativity
  L2: MERGE(MERGE(S₁, S₂), S₃) = MERGE(S₁, MERGE(S₂, S₃))  — associativity
  L3: MERGE(S, S) = S                             — idempotency
  L4: S ⊆ MERGE(S, S')                            — monotonicity
```

#### Branching Extension

```
Branching G-Set: (S, B, ⊑, commit, combine)
  S = trunk (shared store, a G-Set over D)
  B = set of branches, each a G-Set over D
  ⊑ = ancestry relation
  commit : Branch × S → S'
  combine : Branch × Branch → Branch

Properties:
  P1 (Monotonicity):    commit(b, S) ⊇ S
  P2 (Isolation):       ∀ b₁ ≠ b₂: visible(b₁) ∩ branch_only(b₂) = ∅
  P3 (Combination commutativity): combine(b₁, b₂) = combine(b₂, b₁)
  P4 (Commit-combine equivalence): commit(combine(b₁, b₂), S) = commit(b₂, commit(b₁, S))
  P5 (Fork snapshot):   b.base = S|_{frontier(t_fork)}
```

#### Working Set (W_α)

```
Each agent α maintains private W_α using the same datom structure as S.

Local query view: visible(α) = W_α ∪ S
Commit: commit(W_α, S) = S ∪ selected(W_α)   — agent chooses what to commit

W_α datoms are NOT included in MERGE operations.
W_α datoms are invisible to other agents.
```

---

### §7.2 Level 1: State Machine Specification

#### Merge Cascade

```
MERGE(S₁, S₂) → S'

POST (set union):
  S'.datoms = S₁.datoms ∪ S₂.datoms

CASCADE (all produce datoms):
  1. DETECT CONFLICTS:
     For each new datom d entering from the merge:
       if conflict(d, d_existing) → assert Conflict entity
  2. INVALIDATE CACHES:
     Mark query results as stale for entities affected by new datoms
  3. MARK STALE PROJECTIONS:
     Existing projection patterns touching affected entities → refresh needed
  4. RECOMPUTE UNCERTAINTY:
     σ(e) updated for entities with new assertions or conflicts
  5. FIRE SUBSCRIPTIONS:
     Notify subscribers whose patterns match new datoms
```

#### Branch Operations

```
Six sub-operations:

FORK(S, agent, purpose) → Branch
  POST: branch.base_tx = current frontier
        branch.status = :active
        branch entity created in S

COMMIT(branch, S) → S'
  PRE:  branch.status = :active
        if branch.competing_with ≠ ∅: comparison/deliberation completed
  POST: S' = S ∪ branch.datoms
        branch.status = :committed

COMBINE(b₁, b₂, strategy) → Branch
  strategies:
    Union — b₁.datoms ∪ b₂.datoms
    SelectiveUnion — agent-curated subset
    ConflictToDeliberation — conflicts → Deliberation entity
  POST: result preserves properties P1–P4

REBASE(branch, S_new) → Branch'
  POST: branch'.base_tx = S_new.frontier
        branch' sees trunk datoms up to S_new.frontier

ABANDON(branch) → ()
  POST: branch.status = :abandoned (datom, not deletion)

COMPARE(branches, criterion) → BranchComparison
  criteria: FitnessScore | TestSuite | UncertaintyReduction | AgentReview | Custom
  POST: BranchComparison entity created with scores, winner, rationale
```

#### Competing Branch Lock

```
∀ branches b₁, b₂ where b₁.competing_with = b₂:
  COMMIT(b₁, S) is BLOCKED until:
    ∃ BranchComparison c: c.branches ⊇ {b₁, b₂} ∧ c.winner is decided
  OR:
    ∃ Deliberation d resolving the competition

This prevents first-to-commit from winning by default.
```

---

### §7.3 Level 2: Interface Specification

```rust
/// Branch entity.
pub struct Branch {
    pub id: EntityId,
    pub ident: String,
    pub base_tx: TxId,
    pub agent: AgentId,
    pub status: BranchStatus,       // lattice: :active < :proposed < :committed < :abandoned
    pub purpose: String,
    pub competing_with: Vec<EntityId>,
    pub datoms: BTreeSet<Datom>,
}

pub enum CombineStrategy {
    Union,
    SelectiveUnion { selected: Vec<Datom> },
    ConflictToDeliberation,
}

pub enum ComparisonCriterion {
    FitnessScore,
    TestSuite,
    UncertaintyReduction,
    AgentReview,
    Custom(String),
}

pub struct BranchComparison {
    pub branches: Vec<EntityId>,
    pub criterion: ComparisonCriterion,
    pub scores: HashMap<EntityId, f64>,
    pub winner: Option<EntityId>,
    pub rationale: String,
}

/// Merge receipt — records what happened during merge.
pub struct MergeReceipt {
    pub datoms_added: usize,
    pub conflicts_detected: Vec<Conflict>,
    pub subscriptions_fired: usize,
    pub stale_projections: usize,
}

impl Store {
    /// Merge another store (set union + cascade).
    pub fn merge(&mut self, other: &Store) -> MergeReceipt;

    /// Create a branch.
    pub fn fork(&mut self, agent: AgentId, purpose: &str) -> Result<Branch, BranchError>;

    /// Commit a branch to trunk.
    pub fn commit_branch(&mut self, branch: &Branch) -> Result<TxReceipt, BranchError>;

    /// Compare branches.
    pub fn compare_branches(
        &mut self,
        branches: &[EntityId],
        criterion: ComparisonCriterion,
    ) -> Result<BranchComparison, BranchError>;
}
```

#### CLI Commands

```
braid merge --from <store-path>       # Merge another store
braid branch create "experiment-x"    # Fork a branch
braid branch list                     # List all branches
braid branch commit <branch>          # Commit branch to trunk
braid branch compare <b1> <b2>        # Compare two branches
braid branch abandon <branch>         # Mark branch as abandoned
```

---

### §7.4 Invariants

### INV-MERGE-001: Merge Is Set Union

**Traces to**: SEED §4, C4, ADRS AS-001
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ S₁, S₂: MERGE(S₁, S₂).datoms = S₁.datoms ∪ S₂.datoms
  (no heuristics, no resolution, no filtering — pure set union)
```

#### Level 1 (State Invariant)
The merge operation at the store level is exactly set union. All conflict detection,
resolution, and cascade effects are post-merge operations, not part of merge itself.

**Falsification**: A merge operation that produces a datom set different from the
mathematical set union of the two input sets.

---

### INV-MERGE-002: Merge Cascade Completeness

**Traces to**: ADRS PO-006
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ merge operations MERGE(S₁, S₂):
  all 5 cascade steps execute:
    (1) conflict detection, (2) cache invalidation,
    (3) projection staleness, (4) uncertainty update,
    (5) subscription notification
  all cascade steps produce datoms
```

#### Level 1 (State Invariant)
No cascade step is skipped. Each step produces datoms recording its effects.
The merge cascade is atomic — either all 5 steps complete or the merge fails.

**Falsification**: A merge that completes without running conflict detection,
or a cascade step that produces no datom trail.

**Stateright model**: Model merge operations between 3 agents. Verify that
every merge triggers all 5 cascade steps in all interleavings.

---

### INV-MERGE-003: Branch Isolation

**Traces to**: ADRS AS-003, AS-004
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
∀ branches b₁, b₂ where b₁ ≠ b₂:
  branch_datoms(b₁) ∩ visible(b₂) = ∅
  (branches cannot see each other's uncommitted datoms)
```

#### Level 1 (State Invariant)
A query against branch b₁ never returns datoms from branch b₂.
Branch visibility is exactly: `{trunk datoms at fork point} ∪ {b₁'s own datoms}`.

**Falsification**: A query against branch b₁ returning a datom from b₂.

**proptest strategy**: Create two branches from the same fork point. Add different
datoms to each. Verify queries against each branch see only their own datoms.

---

### INV-MERGE-004: Competing Branch Lock

**Traces to**: ADRS AS-005, PO-007
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
∀ branches b₁, b₂ where b₁.competing_with = b₂:
  COMMIT(b₁) is BLOCKED until:
    ∃ comparison or deliberation resolving {b₁, b₂}
```

#### Level 1 (State Invariant)
A branch marked as competing with another branch cannot be committed
until a BranchComparison or Deliberation entity exists that resolves
the competition.

**Falsification**: A competing branch committed without a prior comparison or deliberation.

**Stateright model**: Two competing branches, two agents. Verify that no
interleaving allows commit without comparison.

---

### INV-MERGE-005: Branch Commit Monotonicity

**Traces to**: ADRS AS-003 Property P1
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
∀ branch commits: commit(b, S) ⊇ S
  (committing a branch only adds datoms to trunk)
```

#### Level 1 (State Invariant)
Branch commit is a union operation: trunk grows, never shrinks.

**Falsification**: A branch commit that removes datoms from trunk.

---

### INV-MERGE-006: Branch as First-Class Entity

**Traces to**: ADRS AS-005
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
∀ branches b: b is an entity in the datom store with:
  :branch/ident, :branch/base-tx, :branch/agent,
  :branch/status, :branch/purpose, :branch/competing-with
```

#### Level 1 (State Invariant)
Branch metadata is queryable via the same Datalog engine as any other data.
The `:branch/competing-with` attribute enables the competing branch lock.

**Falsification**: A branch whose metadata is not queryable via Datalog.

---

### INV-MERGE-007: Bilateral Branch Duality

**Traces to**: ADRS AS-006
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
The DCC (diverge-compare-converge) pattern works identically:
  Forward: spec → competing implementations → selection
  Backward: implementation → competing spec updates → selection
Same algebraic structure, same comparison machinery.
```

#### Level 1 (State Invariant)
If the system supports branching for implementation alternatives, it must
also support branching for specification alternatives.

**Falsification**: The system supports implementation branches but requires
linear (non-branching) spec modifications.

---

### INV-MERGE-008: At-Least-Once Idempotent Delivery

**Traces to**: ADRS PD-004
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ stores S, R:
  MERGE(MERGE(S, R), R) = MERGE(S, R)
  (duplicate delivery produces same result — idempotency from L3)
```

#### Level 1 (State Invariant)
Duplicate merge operations are harmless. An agent that receives the same
datoms twice produces the same store state as receiving them once.

**Falsification**: A duplicate merge that changes the store state.

---

### §7.5 ADRs

### ADR-MERGE-001: Set Union Over Heuristic Merge

**Traces to**: C4, ADRS AS-001
**Stage**: 0

#### Problem
How should stores be merged?

#### Options
A) **Pure set union** — mathematical operation. Conflicts detected post-merge.
B) **Resolution during merge** — apply conflict resolution during merge.
C) **Selective merge** — agent chooses which datoms to accept.

#### Decision
**Option A.** MERGE is `S₁ ∪ S₂`. Conflict detection and resolution are separate
operations (RESOLUTION namespace) that run after merge completes. This preserves
L1–L3 (CRDT properties) and avoids making merge depend on schema.

#### Formal Justification
Option B makes MERGE depend on resolution modes (schema), creating a circular dependency:
merge needs schema, schema is data in the store, store is modified by merge. Option A
breaks this cycle: merge is pure set union, resolution is query-time.

---

### ADR-MERGE-002: Branching G-Set Extension

**Traces to**: ADRS AS-003
**Stage**: 2

#### Problem
How do agents get isolated workspaces?

#### Decision
The pure G-Set is extended to a Branching G-Set with five properties (P1–P5).
Branches are G-Sets themselves, preserving all CRDT properties. Trunk monotonicity
is preserved: `commit(b, S) ⊇ S`.

#### Formal Justification
The extension preserves the core G-Set properties while adding isolation.
Each branch is a G-Set that can be composed with trunk via union (commit).

---

### ADR-MERGE-003: Competing Branch Lock

**Traces to**: ADRS AS-005, PO-007
**Stage**: 2

#### Problem
How to prevent first-to-commit from winning by default?

#### Decision
Branches can declare `:branch/competing-with` pointing to another branch.
Competing branches MUST NOT commit until a BranchComparison or Deliberation
resolves the competition.

#### Formal Justification
Without the lock, the first agent to commit "wins" by making its datoms part
of trunk. The competing branch then sees those datoms and may be unable to
diverge. The lock ensures comparison before commitment.

---

### ADR-MERGE-004: Three Combine Strategies

**Traces to**: ADRS PO-007
**Stage**: 2

#### Problem
How to combine two branches?

#### Decision
Three strategies: Union (merge both), SelectiveUnion (agent curates),
ConflictToDeliberation (conflicts become Deliberation entities).

ConflictToDeliberation opens a structured resolution process instead
of forcing an immediate choice.

---

### §7.6 Negative Cases

### NEG-MERGE-001: No Merge Data Loss

**Traces to**: C4, L4
**Verification**: `V:KANI`, `V:PROP`

**Safety property**: `□ ¬(∃ d ∈ S₁ ∪ S₂: d ∉ MERGE(S₁, S₂))`
No datom from either input is lost during merge.

**Kani harness**: For all store pairs of size ≤ N, verify merged datom set
is the exact union.

---

### NEG-MERGE-002: No Merge Without Cascade

**Traces to**: ADRS PO-006
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ merge completing without all 5 cascade steps)`

**proptest strategy**: Instrument each cascade step. After merge, verify all 5
were executed and produced datom trails.

---

### NEG-MERGE-003: No Working Set Leak

**Traces to**: ADRS PD-001
**Verification**: `V:PROP`, `V:KANI`

**Safety property**: `□ ¬(∃ W_α datom visible to agent β where α ≠ β)`
Working set datoms are never included in merge operations.

**Kani harness**: For two agents with working sets, verify merge of their
shared stores does not include any working set datom.

---

## §8. SYNC — Sync Barriers

### §8.0 Overview

Sync barriers establish consistent cuts — shared reference points where all participating
agents agree on the same facts. This is the most expensive coordination mechanism because
it requires blocking until all participants report their frontiers. It is necessary for
decisions that depend on the absence of certain facts (non-monotonic queries).

**Traces to**: SEED.md §6
**ADRS.md sources**: PO-010, SQ-001, SQ-004, PD-005

---

### §8.1 Level 0: Algebraic Specification

#### Consistent Cut

```
A consistent cut C is a set of frontiers such that:
  ∀ agents α, β participating in C:
    C[α] and C[β] are causally consistent
    (no message "in flight" — all sent messages are received)

Formally: C = {(α, F_α) | α ∈ participants}
  where ∀ α: F_α = frontier of α at barrier completion

A consistent cut enables answering "what is NOT in the store" —
the set of facts absent from the cut is meaningful because all
participants agree on what IS present.
```

#### Barrier as Frontier Intersection

```
Given agents {α₁, ..., αₙ} with frontiers {F₁, ..., Fₙ}:

Barrier establishes: ∀ i, j: known(αᵢ, F_barrier) = known(αⱼ, F_barrier)
  where F_barrier = the consistent cut

Post-barrier: non-monotonic queries at F_barrier produce
deterministic results across all participants.
```

---

### §8.2 Level 1: State Machine Specification

#### Barrier Protocol

```
SYNC-BARRIER(participants, timeout) → BarrierResult

PROTOCOL:
  1. INITIATE: Barrier initiator creates Barrier entity in store.
     barrier.status = :initiated
     barrier.participants = [agent IDs]
     barrier.timeout = duration

  2. EXCHANGE: Each participant:
     a. Reports current frontier to barrier entity
     b. Shares all datoms not yet received by others (delta sync)
     c. Waits for all other participants to report

  3. RESOLVE:
     If all participants report within timeout:
       barrier.status = :resolved
       barrier.cut = consistent cut (the agreed-upon frontier)
       All participants now have identical datom sets (up to the cut)
     If timeout expires:
       barrier.status = :timed-out
       barrier records which participants responded

  4. QUERY-ENABLE:
     Post-resolution, non-monotonic queries reference the barrier:
       QueryMode::Barriered(barrier_id)
     Results are deterministic across all participants.

POST:
  Barrier entity in store with full provenance
  All participants at same frontier (if resolved)
```

#### Topology-Dependent Implementation

```
The protocol provides primitives; deployment chooses topology.

Star topology:   coordinator collects and distributes
Ring topology:   each agent passes to next
Mesh topology:   all-to-all exchange
Hierarchical:    tree-structured aggregation

The sync result is topology-independent (SQ-005):
  same participants + same datoms → same consistent cut
```

---

### §8.3 Level 2: Interface Specification

```rust
/// Sync barrier entity.
pub struct Barrier {
    pub id: EntityId,
    pub participants: Vec<AgentId>,
    pub status: BarrierStatus,     // lattice: :initiated < :exchanging < :resolved | :timed-out
    pub timeout: Duration,
    pub cut: Option<Frontier>,     // set after resolution
    pub responses: HashMap<AgentId, Frontier>,
}

pub enum BarrierResult {
    Resolved { cut: Frontier },
    TimedOut { responded: Vec<AgentId>, missing: Vec<AgentId> },
}

impl Store {
    /// Initiate a sync barrier.
    pub fn sync_barrier(
        &mut self,
        participants: &[AgentId],
        timeout: Duration,
    ) -> Result<EntityId, SyncError>;

    /// Participate in a barrier (report frontier, share deltas).
    pub fn barrier_participate(
        &mut self,
        barrier_id: EntityId,
        agent: AgentId,
    ) -> Result<(), SyncError>;

    /// Check barrier status.
    pub fn barrier_status(&self, barrier_id: EntityId) -> BarrierStatus;

    /// Query at a barrier's consistent cut.
    pub fn query_at_barrier(
        &mut self,
        expr: &QueryExpr,
        barrier_id: EntityId,
    ) -> Result<QueryResult, QueryError>;
}
```

#### CLI Commands

```
braid sync --with agent-1,agent-2       # Initiate barrier
braid sync --timeout 30s                # With timeout
braid sync status <barrier-id>          # Check barrier status
braid query --barrier <barrier-id> '[:find ...]'  # Query at barrier
```

---

### §8.4 Invariants

### INV-SYNC-001: Barrier Produces Consistent Cut

**Traces to**: SEED §6, ADRS PO-010
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 3

#### Level 0 (Algebraic Law)
```
∀ resolved barriers B with participants {α₁, ..., αₙ}:
  ∀ i, j: datoms_visible(αᵢ, B.cut) = datoms_visible(αⱼ, B.cut)
  (all participants see the same datom set at the cut)
```

#### Level 1 (State Invariant)
A resolved barrier guarantees that all participants have exchanged all
datoms up to the cut point. Non-monotonic queries at this cut produce
identical results regardless of which participant evaluates them.

**Falsification**: Two participants at a resolved barrier producing different
results for the same non-monotonic query.

**Stateright model**: 3 agents with different initial datom sets. Run barrier
protocol. Verify post-barrier query determinism across all agents.

---

### INV-SYNC-002: Barrier Timeout Safety

**Traces to**: ADRS PO-010
**Verification**: `V:PROP`
**Stage**: 3

#### Level 0 (Algebraic Law)
```
∀ barriers B with timeout T:
  B resolves within T OR B times out with status :timed-out
  No barrier hangs indefinitely.
```

#### Level 1 (State Invariant)
A barrier always terminates — either by resolution (all respond) or by
timeout (deadline reached). The timed-out barrier records which participants
responded and which did not, for crash-recovery (PD-003).

**Falsification**: A barrier that neither resolves nor times out.

---

### INV-SYNC-003: Barrier Is Topology-Independent

**Traces to**: ADRS PD-005, SQ-005
**Verification**: `V:MODEL`
**Stage**: 3

#### Level 0 (Algebraic Law)
```
∀ topologies T₁, T₂, ∀ participant sets P, ∀ datom sets D:
  barrier(P, D, T₁).cut = barrier(P, D, T₂).cut
  (the consistent cut depends only on participants and datoms, not topology)
```

#### Level 1 (State Invariant)
Star, ring, mesh, and hierarchical topologies all produce the same consistent
cut for the same inputs.

**Falsification**: Two different topologies producing different cuts for the same
participants and datom sets.

**Stateright model**: Run barrier protocol under 3 topologies (star, ring, mesh).
Verify identical cuts.

---

### INV-SYNC-004: Barrier Entity Provenance

**Traces to**: ADRS FD-012
**Verification**: `V:PROP`
**Stage**: 3

#### Level 0 (Algebraic Law)
```
∀ barrier operations:
  ∃ Barrier entity in the store recording:
    participants, status, timeout, cut (if resolved), responses
```

#### Level 1 (State Invariant)
Every barrier — resolved or timed-out — produces a Barrier entity in the store.
The barrier history is queryable.

**Falsification**: A barrier operation that completes without creating a Barrier entity.

---

### INV-SYNC-005: Non-Monotonic Queries Require Barrier

**Traces to**: ADRS SQ-004
**Verification**: `V:PROP`
**Stage**: 3

#### Level 0 (Algebraic Law)
```
∀ queries Q with mode = Barriered(barrier_id):
  barrier_id references a resolved Barrier entity
  Q is evaluated against barrier.cut (not local frontier)
```

#### Level 1 (State Invariant)
A Barriered query mode requires a valid, resolved barrier. The query engine
rejects Barriered queries referencing unresolved or timed-out barriers.

**Falsification**: A Barriered query executing against a timed-out or nonexistent barrier.

---

### §8.5 ADRs

### ADR-SYNC-001: Barrier as Explicit Coordination Point

**Traces to**: ADRS SQ-001, PO-010
**Stage**: 3

#### Problem
How to handle non-monotonic queries that depend on the absence of facts?

#### Options
A) **Always consistent** — all queries require global consistency. Too expensive.
B) **Never consistent** — all queries are local frontier. Non-monotonic results vary.
C) **Explicit barriers** — monotonic queries run locally; non-monotonic queries can
   optionally use a barrier for consistency.

#### Decision
**Option C.** Most queries (Strata 0–1) are monotonic and need no coordination.
Non-monotonic queries (Strata 2–5) produce useful approximate results at local
frontier but can use a barrier when precision is critical.

#### Formal Justification
CALM theorem: monotonic programs have coordination-free implementations.
Barriers are needed only for non-monotonic queries where correctness
depends on knowing what is NOT present.

---

### ADR-SYNC-002: Topology-Agnostic Protocol

**Traces to**: ADRS PD-005
**Stage**: 3

#### Problem
Should the sync protocol prescribe a topology?

#### Decision
No. The protocol provides primitives (initiate, report, exchange, resolve).
Topology emerges from deployment. Single-agent (trivial — barrier with self),
bilateral (two agents exchange), flat swarm (all-to-all), hierarchy (tree) are
all valid using the same primitives.

#### Formal Justification
Prescribing topology limits applicability. The invariant (INV-SYNC-003) that
results are topology-independent means the protocol can support any topology
without changing the correctness guarantees.

---

### ADR-SYNC-003: Barrier Timeout Over Blocking

**Traces to**: ADRS PO-010
**Stage**: 3

#### Problem
What happens when a barrier participant doesn't respond?

#### Decision
Timeouts. Every barrier has a deadline. Unresponsive participants cause
timeout, not deadlock. The timed-out barrier records who responded,
enabling crash-recovery (PD-003) — the recovering agent can query the
barrier record to understand what was missed.

---

### §8.6 Negative Cases

### NEG-SYNC-001: No Unbounded Barrier Wait

**Traces to**: ADRS PO-010
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ barrier that blocks indefinitely)`
Every barrier either resolves or times out within its declared timeout.

**proptest strategy**: Create barriers with varying participant counts and
response patterns. Verify all complete within timeout.

---

### NEG-SYNC-002: No Barrier at Inconsistent Cut

**Traces to**: ADRS PO-010
**Verification**: `V:MODEL`

**Safety property**: `□ ¬(∃ resolved barrier where participants disagree on datom set)`

**Stateright model**: 3 agents with partial connectivity. Run barrier protocol.
Verify that resolution only occurs when all participants have identical visible sets.

---

---

## §9. SIGNAL — Divergence Signal Routing

> **Purpose**: Signals are the nervous system of DDIS — typed events that detect divergence
> and route it to the appropriate resolution mechanism. Every signal is a datom, making
> the system's self-awareness queryable and auditable.
>
> **Traces to**: SEED.md §6 (Reconciliation Mechanisms), ADRS PO-004, PO-005, PO-008,
> CO-003, CR-002, CR-003, AS-009

### §9.1 Level 0: Algebraic Specification

A **signal** is a typed divergence detection event:

```
Signal = (type: SignalType, source: EntityId, target: EntityId,
          severity: Severity, payload: Value)

SignalType = Confusion | Conflict | UncertaintySpike | ResolutionProposal
           | DelegationRequest | GoalDrift | BranchReady | DeliberationTurn

Severity = Low | Medium | High | Critical
  with total order: Low < Medium < High < Critical
```

The **signal dispatch function** maps signal types to resolution mechanisms:

```
dispatch : Signal → ResolutionMechanism
dispatch(Confusion(_))         = ReAssociate       — epistemic divergence
dispatch(Conflict(_))          = Route(severity)    — aleatory divergence
dispatch(UncertaintySpike(_))  = Guidance           — consequential divergence
dispatch(GoalDrift(_))         = Escalate(human)    — axiological divergence
dispatch(DelegationRequest(_)) = Delegate           — authority resolution
dispatch(BranchReady(_))       = Compare            — structural divergence
dispatch(DeliberationTurn(_))  = Deliberate         — logical divergence
dispatch(ResolutionProposal(_))= Evaluate           — resolution convergence
```

**Laws**:
- **L1 (Totality)**: Every signal type has a defined dispatch target
- **L2 (Monotonicity)**: `severity(s1) ≤ severity(s2) ⟹ cost(dispatch(s1)) ≤ cost(dispatch(s2))` — higher severity signals route to more expensive resolution mechanisms
- **L3 (Completeness)**: Every divergence type in the reconciliation taxonomy (CO-003) maps to at least one signal type

### §9.2 Level 1: State Machine Specification

**State**: `Σ_signal = (pending: Set<Signal>, active: Set<Signal>, resolved: Set<Signal>, subscriptions: Map<Pattern, Set<Callback>>)`

**Transitions**:

```
EMIT(Σ, signal) → Σ' where:
  PRE:  signal.source ∈ known_entities(store)
  POST: Σ'.pending = Σ.pending ∪ {signal}
  POST: signal recorded as datom in store
  POST: matching subscriptions fired

ROUTE(Σ, signal) → Σ' where:
  PRE:  signal ∈ Σ.pending
  POST: Σ'.pending = Σ.pending \ {signal}
  POST: Σ'.active = Σ.active ∪ {signal}
  POST: dispatch(signal) invoked

RESOLVE(Σ, signal, resolution) → Σ' where:
  PRE:  signal ∈ Σ.active
  POST: Σ'.active = Σ.active \ {signal}
  POST: Σ'.resolved = Σ.resolved ∪ {signal}
  POST: resolution recorded as datom with causal link to signal

SUBSCRIBE(Σ, pattern, callback) → Σ' where:
  POST: Σ'.subscriptions[pattern] = Σ.subscriptions[pattern] ∪ {callback}
  INV:  subscription persists until explicitly removed
```

**Conflict routing cascade** (from CR-003):
1. Assert Conflict entity as datom
2. Compute severity = `max(w(d₁), w(d₂))` (commitment weights)
3. Route by severity tier: automated (Low) → agent-with-notification (Medium) → human-required (High/Critical)
4. Fire TUI notification if severity ≥ Medium
5. Update uncertainty tensor for affected entities
6. Invalidate cached query results touching affected entities

### §9.3 Level 2: Implementation Contract

```rust
/// Signal types — sum type covering all divergence classes
#[derive(Clone, Debug)]
pub enum SignalType {
    Confusion(ConfusionKind),
    Conflict { datom_a: DatomRef, datom_b: DatomRef },
    UncertaintySpike { entity: EntityId, delta: f64 },
    ResolutionProposal { deliberation: EntityId, position: EntityId },
    DelegationRequest { entity: EntityId, from: AgentId, to: AgentId },
    GoalDrift { intention: EntityId, observed_delta: f64 },
    BranchReady { branch: EntityId, comparison_criteria: Vec<Criterion> },
    DeliberationTurn { deliberation: EntityId, position: EntityId },
}

#[derive(Clone, Debug)]
pub enum ConfusionKind {
    NeedMore,       // insufficient context
    Contradictory,  // conflicting information
    GoalUnclear,    // ambiguous intention
    SchemaUnknown,  // unknown entity type or attribute
}

pub struct Signal {
    pub signal_type: SignalType,
    pub source: EntityId,
    pub target: EntityId,
    pub severity: Severity,
    pub timestamp: TxId,
}

/// Subscription — Datalog-like pattern with callback
pub struct Subscription {
    pub pattern: SignalPattern,
    pub callback: Box<dyn Fn(&Signal) -> Vec<Datom>>,
    pub debounce: Option<Duration>,
}
```

### §9.4 Invariants

### INV-SIGNAL-001: Signal as Datom

**Traces to**: ADRS PO-004
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 3

#### Level 0 (Algebraic Law)
Every signal is a datom. Signal history is a subset of the store:
`∀ s ∈ signals_emitted: ∃ d ∈ S such that d encodes s`

#### Level 1 (State Invariant)
For all reachable states, every emitted signal has a corresponding datom in the store
with entity type `:signal/*` and attributes recording type, source, target, severity.

#### Level 2 (Implementation Contract)
```rust
// Every emit produces a transact
fn emit_signal(store: &mut Store, signal: Signal) -> TxReceipt {
    let datoms = signal.to_datoms(); // deterministic encoding
    store.transact(Transaction::from(datoms).commit(&store.schema()).unwrap())
        .unwrap()
}
```

**Falsification**: A signal is emitted but no corresponding datom exists in the store.

**proptest strategy**: Emit random signals. After each, query store for `:signal/type`
matching the emitted type. Verify 1:1 correspondence.

---

### INV-SIGNAL-002: Confusion Triggers Re-Association

**Traces to**: ADRS PO-005
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
`dispatch(Confusion(cue)) = ReAssociate(cue)` — confusion signals trigger the
associative retrieval pipeline within one agent cycle (not a full round-trip).

#### Level 1 (State Invariant)
For all reachable states where a Confusion signal is emitted:
within the same agent cycle, ASSOCIATE + ASSEMBLE execute with the confusion cue
as input, producing an updated context.

#### Level 2 (Implementation Contract)
The agent cycle handler intercepts Confusion signals and invokes the
`associate → query → assemble` pipeline before proceeding to the next action.

**Falsification**: A Confusion signal is emitted and the agent proceeds to the next
action without re-association.

**proptest strategy**: Inject Confusion signals at random points in agent cycle
simulations. Verify re-association always executes before the next action.

---

### INV-SIGNAL-003: Subscription Completeness

**Traces to**: ADRS PO-008
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 3

#### Level 0 (Algebraic Law)
`∀ subscription s, signal σ: matches(s.pattern, σ) ⟹ s.callback(σ) is invoked`

No matching signal is silently dropped.

#### Level 1 (State Invariant)
For all reachable states where EMIT produces a signal matching a subscription pattern,
the subscription callback fires within one refresh cycle. Debounced subscriptions
batch within their declared window but still fire.

#### Level 2 (Implementation Contract)
```rust
#[kani::ensures(|_| subscriptions.iter()
    .filter(|s| s.pattern.matches(&signal))
    .all(|s| s.fired_count > old(s.fired_count)))]
fn emit_and_dispatch(signal: Signal, subscriptions: &mut [Subscription]) { ... }
```

**Falsification**: A subscription pattern matches a signal, but the callback is never invoked.

---

### INV-SIGNAL-004: Severity-Ordered Routing

**Traces to**: ADRS CR-002
**Verification**: `V:PROP`
**Stage**: 3

#### Level 0 (Algebraic Law)
`severity(s) = max(w(d₁), w(d₂))` for conflict signals. The routing tier is
monotonically determined by severity:
```
Low      → Tier 1 (automated lattice/LWW resolution)
Medium   → Tier 2 (agent-with-notification)
High     → Tier 3 (human-required, blocks progress)
Critical → Tier 3 + immediate TUI alert
```

#### Level 1 (State Invariant)
No High/Critical severity signal is resolved by an automated mechanism.
No Low severity signal blocks agent progress.

#### Level 2 (Implementation Contract)
The routing function's output tier is determined by a match on severity,
with the mapping configured as datoms (enabling per-deployment tuning).

**Falsification**: A Critical-severity conflict is silently resolved by LWW
without human/agent review.

---

### INV-SIGNAL-005: Diamond Lattice Signal Generation

**Traces to**: ADRS AS-009
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 2

#### Level 0 (Algebraic Law)
For diamond lattices (challenge-verdict, finding-lifecycle, proposal-lifecycle),
when two incomparable values are merged (CRDT join), the result is the lattice top
which encodes a coordination signal:
```
join(confirmed, refuted) = contradicted    → emits Conflict signal
join(proposed_A, proposed_B) = contested   → emits DeliberationTurn signal
```

#### Level 1 (State Invariant)
For all reachable states where a lattice merge produces a diamond-top value,
a signal of the corresponding type is emitted within the same transaction.

#### Level 2 (Implementation Contract)
Lattice join implementations for diamond lattices include a signal-emission
side effect when the join produces the top element.

**Falsification**: Two incomparable lattice values merge to produce a top element
but no signal is emitted.

**proptest strategy**: Generate random concurrent assertions on diamond-lattice
attributes. Verify that every top-join produces exactly one signal.

---

### INV-SIGNAL-006: Taxonomy Completeness

**Traces to**: ADRS CO-003
**Verification**: `V:PROP`
**Stage**: 3

#### Level 0 (Algebraic Law)
The signal type set covers all eight divergence types in the reconciliation taxonomy:
```
Epistemic    → Confusion
Structural   → BranchReady (forward), GoalDrift (backward)
Consequential → UncertaintySpike
Aleatory     → Conflict
Logical      → DeliberationTurn
Axiological  → GoalDrift
Temporal     → (detected by frontier comparison, surfaced as UncertaintySpike)
Procedural   → (detected by drift detection, surfaced as GoalDrift)
```

#### Level 1 (State Invariant)
Every detected divergence, regardless of type, produces at least one signal.
No divergence class lacks a signal pathway.

**Falsification**: A divergence is detected by some mechanism but no signal
is emitted, leaving it invisible to the resolution layer.

---

### §9.5 ADRs

### ADR-SIGNAL-001: Eight Signal Types Cover Reconciliation Taxonomy

**Traces to**: ADRS PO-004, CO-003
**Stage**: 3

#### Problem
How many signal types are needed, and how do they map to divergence types?

#### Options
A) One generic signal type with metadata — simple but loses type safety
B) One signal type per divergence type (8) — exact coverage but some divergence
   types don't map to a natural signal
C) Eight signal types, some covering multiple divergence types — pragmatic mapping

#### Decision
**Option C.** Eight concrete signal types (from PO-004) with a surjective mapping
from divergence types. Some divergence types (Temporal, Procedural) are detected by
specialized mechanisms and surfaced through existing signal types.

#### Formal Justification
The taxonomy completeness law (L3) requires surjection from divergence types to signal
types, not bijection. A 1:1 mapping would force artificial signal types for divergences
that are better detected by existing mechanisms (e.g., temporal divergence is naturally
frontier comparison, not a separate signal).

---

### ADR-SIGNAL-002: Conflict Routing Cascade as Datom Trail

**Traces to**: ADRS CR-003
**Stage**: 3

#### Problem
Should conflict routing produce durable records or be ephemeral dispatch?

#### Decision
Every step of the routing cascade (assert conflict → compute severity → route → notify →
update uncertainty → invalidate caches) produces datoms. The cascade is a transaction.
This makes the full resolution history queryable: "How was this conflict detected?
What severity was it assigned? Who resolved it? What was the rationale?"

#### Formal Justification
FD-012 (every command is a transaction) applies to signal routing. Ephemeral routing
would create state outside the store, violating the single-source-of-truth property.

---

### ADR-SIGNAL-003: Subscription Debounce Over Immediate Fire

**Traces to**: ADRS PO-008
**Stage**: 3

#### Problem
Should subscriptions fire immediately on every match, or debounce rapid-fire events?

#### Decision
Optional debounce parameter per subscription. Debounced subscriptions batch matching
signals within a time window and fire once with the full batch. Immediate fire
remains the default for latency-sensitive subscriptions (e.g., TUI notifications).

#### Formal Justification
MERGE cascade can produce many signals in rapid succession. Without debounce,
N conflicts from a single merge produce N subscription fires. Debounce reduces
to 1 batched fire containing N signals — same information, lower overhead.

---

### §9.6 Negative Cases

### NEG-SIGNAL-001: No Silent Signal Drop

**Traces to**: ADRS PO-004
**Verification**: `V:PROP`, `V:KANI`

**Safety property**: `□ ¬(∃ signal emitted ∧ ¬recorded_as_datom)`

Every emitted signal is recorded in the store. No signal is lost between
emission and recording.

**proptest strategy**: Emit signals under concurrent load (multiple agents).
Verify store contains exactly the emitted signal set after quiescence.

---

### NEG-SIGNAL-002: No Confusion Without Re-Association

**Traces to**: ADRS PO-005
**Verification**: `V:PROP`

**Safety property**: `□ ¬(confusion_emitted ∧ ¬reassociation_within_cycle)`

A Confusion signal that doesn't trigger re-association is a protocol violation.
The agent must not proceed with stale context after signaling confusion.

**proptest strategy**: Inject Confusion signals. Verify agent cycle always
executes ASSOCIATE+ASSEMBLE before the next action step.

---

### NEG-SIGNAL-003: No High-Severity Automated Resolution

**Traces to**: ADRS CR-002
**Verification**: `V:PROP`

**Safety property**: `□ ¬(severity ≥ High ∧ resolved_by_automated_mechanism)`

High and Critical severity conflicts must involve agent or human review.
Automated resolution (lattice join, LWW) is restricted to Low severity.

**proptest strategy**: Generate conflicts with all severity levels. Verify
that High/Critical conflicts are never closed by automated resolution.

---

## §10. BILATERAL — Bilateral Feedback Loop

> **Purpose**: The bilateral loop is the convergence mechanism — it continuously checks
> alignment between specification and implementation in both directions until the gap
> between them reaches zero (or an explicitly documented residual).
>
> **Traces to**: SEED.md §3 (Bilateral feedback loop), §6 (Reconciliation Mechanisms),
> ADRS CO-004, CO-008, CO-009, CO-010, SQ-006, AS-006

### §10.1 Level 0: Algebraic Specification

The bilateral loop is an **adjunction** between forward and backward projections:

```
Forward:  F : Spec → ImplStatus     — does the implementation satisfy the spec?
Backward: B : Impl → SpecAlignment  — does the spec accurately describe the implementation?

The loop is the composition: (B ∘ F) applied repeatedly until fixpoint.
```

**Divergence measure** over the four-boundary chain (CO-010):

```
D(spec, impl) = Σᵢ wᵢ × |boundary_gap(i)|

where boundaries are:
  i=1: Intent → Spec       (axiological gap)
  i=2: Spec → Spec         (logical gap — contradictions)
  i=3: Spec → Impl         (structural gap)
  i=4: Impl → Behavior     (behavioral gap)
```

**Laws**:
- **L1 (Monotonic convergence)**: `D(spec', impl') ≤ D(spec, impl)` after each bilateral cycle — total divergence never increases
- **L2 (Fixpoint existence)**: The loop terminates when `D(spec, impl) = 0` or when all remaining divergence is explicitly documented as residual
- **L3 (Bilateral symmetry)**: Forward and backward checks use the same Datalog query apparatus (SQ-006)

**Fitness function** (CO-009):
```
F(S) = 0.18×V + 0.18×C + 0.18×(1-D) + 0.13×H + 0.13×(1-K) + 0.08×(1-I) + 0.12×(1-U)

where:
  V = validation score (invariants verified / total)
  C = coverage (goals traced to invariants and back)
  D = drift (spec-impl divergence)
  H = harvest quality (FP/FN rates)
  K = contradictions (weighted by severity)
  I = incompleteness (gaps between spec and impl)
  U = mean uncertainty
```

Target: `F(S) → 1.0`

### §10.2 Level 1: State Machine Specification

**State**: `Σ_bilateral = (divergence_map: Map<Boundary, Set<Gap>>, fitness: f64, cycle_count: u64, residuals: Set<DocumentedResidual>)`

**Transitions**:

```
FORWARD_SCAN(Σ, spec, impl) → Σ' where:
  POST: Σ'.divergence_map[SpecToImpl] = detected structural gaps
  POST: for each gap: emit Signal(type=BranchReady or GoalDrift)

BACKWARD_SCAN(Σ, impl, spec) → Σ' where:
  POST: Σ'.divergence_map[ImplToSpec] = detected spec inaccuracies
  POST: for each inaccuracy: emit Signal(type=GoalDrift)

COMPUTE_FITNESS(Σ) → Σ' where:
  POST: Σ'.fitness = F(S) computed from current state
  POST: fitness value recorded as datom

DOCUMENT_RESIDUAL(Σ, gap, rationale) → Σ' where:
  PRE:  gap ∈ Σ.divergence_map[any]
  POST: gap moved from divergence_map to residuals
  POST: rationale recorded with uncertainty marker

CYCLE(Σ, spec, impl) → Σ' where:
  POST: FORWARD_SCAN then BACKWARD_SCAN then COMPUTE_FITNESS
  POST: Σ'.cycle_count = Σ.cycle_count + 1
  INV:  Σ'.fitness ≥ Σ.fitness (monotonic convergence)
```

**Query layer bilateral structure** (SQ-006):

Forward-flow queries (planning):
- Epistemic uncertainty: what does the system not know?
- Crystallization candidates: what is stable enough to commit?
- Delegation: who should work on what?

Backward-flow queries (assessment):
- Conflict detection: where do agents disagree?
- Drift candidates: where has implementation departed from spec?
- Absorption triggers: what implementation patterns should update the spec?

Bridge queries (both):
- Commitment weight: how costly is changing this decision?
- Spectral authority: who has demonstrated competence here?

### §10.3 Level 2: Implementation Contract

```rust
pub struct BilateralLoop {
    pub divergence_map: HashMap<Boundary, Vec<Gap>>,
    pub fitness: f64,
    pub cycle_count: u64,
    pub residuals: Vec<DocumentedResidual>,
}

#[derive(Clone, Debug)]
pub enum Boundary {
    IntentToSpec,
    SpecToSpec,
    SpecToImpl,
    ImplToBehavior,
}

pub struct Gap {
    pub boundary: Boundary,
    pub source: EntityId,
    pub target: Option<EntityId>,
    pub severity: Severity,
    pub description: String,
}

impl BilateralLoop {
    /// Run one complete bilateral cycle
    pub fn cycle(&mut self, store: &mut Store) -> CycleReport {
        let forward = self.forward_scan(store);
        let backward = self.backward_scan(store);
        let fitness = self.compute_fitness(store);
        CycleReport { forward, backward, fitness, cycle: self.cycle_count }
    }
}
```

### §10.4 Invariants

### INV-BILATERAL-001: Monotonic Convergence

**Traces to**: ADRS CO-004
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 1

#### Level 0 (Algebraic Law)
`∀ cycle n: F(S_{n+1}) ≥ F(S_n)`
The fitness function never decreases across bilateral cycles. Each cycle either
reduces divergence or documents residual — both are non-decreasing fitness operations.

#### Level 1 (State Invariant)
For all reachable states (Σ, Σ') where Σ →[CYCLE] Σ':
`Σ'.fitness ≥ Σ.fitness`

#### Level 2 (Implementation Contract)
```rust
#[kani::ensures(|report| report.fitness >= old(self.fitness))]
fn cycle(&mut self, store: &mut Store) -> CycleReport { ... }
```

**Falsification**: A bilateral cycle produces a lower fitness score than the previous cycle.

**proptest strategy**: Run random sequences of bilateral cycles with random
spec/impl states. Verify fitness is monotonically non-decreasing.

**Stateright model**: 2 agents, 1 spec, 1 impl. Run bilateral cycles.
Verify fitness monotonicity across all reachable states.

---

### INV-BILATERAL-002: Five-Point Coherence Statement

**Traces to**: ADRS CO-008
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
The bilateral loop checks five coherence conditions:
```
C1: ¬∃ contradiction in spec         (spec self-consistency)
C2: impl ⊨ spec                      (impl satisfies spec)
C3: spec ≈ intent                     (spec matches intent)
C4: ∀ agents α,β: store_α ∪ store_β converges  (agent agreement)
C5: agent_behavior ⊨ methodology      (process adherence)
```

Full coherence: `C1 ∧ C2 ∧ C3 ∧ C4 ∧ C5`

#### Level 1 (State Invariant)
Each CYCLE evaluates all five conditions. The divergence map partitions gaps
by which coherence condition they violate.

**Falsification**: A bilateral cycle evaluates fewer than five conditions,
leaving a coherence dimension unchecked.

---

### INV-BILATERAL-003: Bilateral Symmetry

**Traces to**: ADRS SQ-006, AS-006
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
Forward and backward scans use the same Datalog query apparatus.
The branching mechanism (AS-006) works identically in both directions:
forward (spec → competing implementations → selection) and backward
(implementation → competing spec updates → selection).

#### Level 1 (State Invariant)
For all reachable states, the forward and backward scans produce gap types
drawn from the same type set, using the same query engine, stored as the
same datom types. No structural asymmetry exists between directions.

**Falsification**: The system supports branching for competing implementations
but requires linear spec modifications (or vice versa).

---

### INV-BILATERAL-004: Residual Documentation

**Traces to**: SEED §6 (explicitly acknowledged residual)
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
Every gap that persists beyond a bilateral cycle is either:
(a) resolved in the next cycle, or
(b) documented as a residual with uncertainty marker and rationale.

No gap persists undocumented.

#### Level 1 (State Invariant)
`∀ gap ∈ divergence_map: age(gap) > 1 cycle ⟹ gap ∈ residuals ∨ gap resolved`

**Falsification**: A gap appears in the divergence map for two consecutive cycles
without being either resolved or documented as a residual.

---

### INV-BILATERAL-005: Test Results as Datoms

**Traces to**: ADRS CO-011
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
Test outcomes are datoms in the store:
`test_passed(X, frontier_F) ⟺ ∃ d ∈ S: d.a = :test/result ∧ d.v = :passed ∧ d.e = X`

This extends the bilateral loop to the Impl→Behavior boundary.

#### Level 1 (State Invariant)
After any test execution, the result (pass/fail, error, frontier) is transacted
into the store as a datom with entity type `:test-result/*`.

**Falsification**: A test runs but its result is not in the store.

---

### §10.5 ADRs

### ADR-BILATERAL-001: Fitness Function Weights

**Traces to**: ADRS CO-009
**Stage**: 1

#### Problem
How should the fitness function weight its seven components?

#### Decision
Weights from CO-009: V=0.18, C=0.18, D=0.18, H=0.13, K=0.13, I=0.08, U=0.12.
Validation, coverage, and drift weighted equally (primary triad). Harvest and
contradiction weighted equally (secondary). Incompleteness lowest (subsumes coverage).
Uncertainty moderate (important coordination metric).

#### Formal Justification
The primary triad (V,C,D) directly measures the spec↔impl correspondence.
The secondary pair (H,K) measures methodology health. Incompleteness is partially
redundant with coverage. Uncertainty is actionable but not a defect per se.

**Uncertainty**: UNC-BILATERAL-001 — weights are theoretical. Empirical calibration
during Stage 0 may revise them. Confidence: 0.6.

---

### ADR-BILATERAL-002: Divergence Metric as Weighted Boundary Sum

**Traces to**: ADRS CO-010
**Stage**: 1

#### Problem
How should total divergence be quantified across the four-boundary chain?

#### Decision
`D(spec, impl) = Σᵢ wᵢ × |boundary_gap(i)|` where boundary weights
reflect the cost of divergence at each boundary. Default: equal weights.

#### Formal Justification
Each boundary contributes independently to total divergence. Weighted sum
is the simplest combination that captures per-boundary severity while
remaining decomposable for targeted remediation.

**Uncertainty**: UNC-BILATERAL-002 — boundary weights may need per-project tuning.
Confidence: 0.5.

---

### ADR-BILATERAL-003: Intent Validation as Periodic Session

**Traces to**: ADRS CO-012
**Stage**: 2

#### Problem
How is the Intent→Spec boundary checked?

#### Decision
Periodic intent validation sessions where the system assembles current spec state
for human review: "Does this still describe what I want?" The human's response
is a datom — either confirming alignment or asserting axiological divergence.

#### Formal Justification
The Intent→Spec boundary uniquely requires human judgment. No automated mechanism
can verify that a specification captures intent (this is the fundamental
limitation — intent exists outside the formal system). Periodic sessions
with structured output make this otherwise invisible boundary checkable.

---

### §10.6 Negative Cases

### NEG-BILATERAL-001: No Fitness Regression

**Traces to**: ADRS CO-004
**Verification**: `V:PROP`, `V:MODEL`

**Safety property**: `□ ¬(F(S_{n+1}) < F(S_n))`
No bilateral cycle may reduce the fitness score.

**proptest strategy**: Run 1000 random bilateral cycles. Verify strict
monotonicity of the fitness sequence.

**Stateright model**: Verify across all reachable states of a
2-agent, 10-invariant model.

---

### NEG-BILATERAL-002: No Unchecked Coherence Dimension

**Traces to**: ADRS CO-008
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ cycle that skips any of C1–C5)`
Every bilateral cycle must evaluate all five coherence conditions.

**proptest strategy**: Instrument cycle execution. Verify all five checks
execute for every cycle invocation.

---

## §11. DELIBERATION — Structured Conflict Resolution

> **Purpose**: Deliberation is the structured resolution mechanism for conflicts that
> automated mechanisms (lattice join, LWW) cannot handle. It produces three entity types
> — Deliberation, Position, Decision — stored as datoms, creating a queryable case law
> system where past decisions inform future conflicts.
>
> **Traces to**: SEED.md §6 (Deliberation and Decision), ADRS CR-004, CR-005, CR-007,
> PO-007, AS-002, AA-001

### §11.1 Level 0: Algebraic Specification

A **deliberation** is a convergence process over a lattice of positions:

```
Deliberation = (question: String, positions: Set<Position>, decision: Option<Decision>)
Position = (stance: Stance, rationale: String, evidence: Set<DatomRef>)
Decision = (method: DecisionMethod, chosen: Position, rationale: String)

Stance = Advocate | Oppose | Neutral | Synthesize
DecisionMethod = Consensus | Majority | Authority | HumanOverride | Automated
```

**Deliberation lifecycle lattice**:
```
:open < :active < :decided < :superseded
         ↗ :stalled (incomparable with :decided)
```

**Laws**:
- **L1 (Convergence)**: Every deliberation either reaches `:decided` or `:stalled` in finite steps
- **L2 (Monotonicity)**: `lifecycle(d, t1) ⊑ lifecycle(d, t2)` for `t1 < t2` — deliberations progress forward in the lattice, never backward
- **L3 (Precedent preservation)**: Decided deliberations remain queryable as precedent (growth-only store guarantees this by construction)
- **L4 (Stability guard)**: A decision may only be reached when crystallization conditions are met (CR-005)

### §11.2 Level 1: State Machine Specification

**State**: `Σ_delib = (deliberations: Map<EntityId, Deliberation>, precedent_index: Map<(EntityType, Attr), Set<EntityId>>)`

**Transitions**:

```
OPEN(Σ, question, context) → Σ' where:
  POST: new deliberation entity with status :open
  POST: conflict signal recorded as causal predecessor

POSITION(Σ, delib_id, stance, rationale, evidence) → Σ' where:
  PRE:  Σ.deliberations[delib_id].status ∈ {:open, :active}
  POST: new position entity linked to deliberation
  POST: Σ.deliberations[delib_id].status = :active (if was :open)

DECIDE(Σ, delib_id, method, chosen, rationale) → Σ' where:
  PRE:  Σ.deliberations[delib_id].status = :active
  PRE:  stability_guard(chosen) passes (CR-005)
  POST: new decision entity linked to deliberation
  POST: Σ.deliberations[delib_id].status = :decided
  POST: competing branches resolved (winner committed, losers marked :abandoned)

STALL(Σ, delib_id, reason) → Σ' where:
  PRE:  Σ.deliberations[delib_id].status = :active
  POST: Σ.deliberations[delib_id].status = :stalled
  POST: reason recorded as uncertainty marker (UNC-*)
  POST: escalation signal emitted (DelegationRequest or GoalDrift)
```

**Crystallization stability guard** (CR-005):
- Status `:refined` (or position has substantive evidence)
- Thread `:active` (deliberation is ongoing, not stalled)
- Parent entity confidence ≥ 0.6
- Coherence score ≥ 0.6
- No unresolved conflicts on the decided entity
- Commitment weight `w(d) ≥ stability_min` (default 0.7)

### §11.3 Level 2: Implementation Contract

```rust
/// Deliberation entity — stored as datoms via schema Layer 2
pub struct Deliberation {
    pub entity: EntityId,
    pub question: String,
    pub status: DeliberationStatus,
    pub positions: Vec<EntityId>,  // refs to Position entities
    pub decision: Option<EntityId>, // ref to Decision entity
}

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub enum DeliberationStatus {
    Open,
    Active,
    Stalled,
    Decided,
    Superseded,
}

pub struct Position {
    pub entity: EntityId,
    pub deliberation: EntityId,
    pub stance: Stance,
    pub rationale: String,
    pub evidence: Vec<DatomRef>,
    pub agent: AgentId,
}

pub struct Decision {
    pub entity: EntityId,
    pub deliberation: EntityId,
    pub method: DecisionMethod,
    pub chosen_position: EntityId,
    pub rationale: String,
    pub commitment_weight: f64,
}
```

### §11.4 Invariants

### INV-DELIBERATION-001: Deliberation Convergence

**Traces to**: ADRS CR-004
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 2

#### Level 0 (Algebraic Law)
`∀ deliberation d: ◇(d.status = :decided ∨ d.status = :stalled)`
Every deliberation eventually reaches a terminal state.

#### Level 1 (State Invariant)
No deliberation remains in `:open` or `:active` indefinitely. Either positions
converge to a decision, or a timeout/stall condition triggers escalation.

#### Level 2 (Implementation Contract)
Deliberations carry a timeout. If no decision is reached within the timeout,
the deliberation transitions to `:stalled` and emits an escalation signal.

**Falsification**: A deliberation remains `:active` past its timeout without
transitioning to `:decided` or `:stalled`.

**Stateright model**: 3 agents filing positions on a deliberation. Verify
that all executions reach a terminal state.

---

### INV-DELIBERATION-002: Stability Guard Enforcement

**Traces to**: ADRS CR-005
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 2

#### Level 0 (Algebraic Law)
`∀ decision d: decide(d) ⟹ stability(d.chosen) ≥ stability_min`
No decision is recorded unless the crystallization stability guard passes.

#### Level 1 (State Invariant)
The DECIDE transition requires all stability guard conditions (CR-005) to hold.
A decision attempted with insufficient stability is rejected.

#### Level 2 (Implementation Contract)
```rust
#[kani::requires(stability_score(&position) >= STABILITY_MIN)]
fn decide(delib: &mut Deliberation, position: EntityId, method: DecisionMethod)
    -> Result<Decision, StabilityError> { ... }
```

**Falsification**: A decision is recorded where `stability(chosen) < stability_min`.

---

### INV-DELIBERATION-003: Precedent Queryability

**Traces to**: ADRS CR-007
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
All decided deliberations are indexed by entity type and contested attributes,
enabling precedent lookup:
```
find-precedent(entity_type, attributes) =
  {d ∈ deliberations | d.status = :decided
                     ∧ d.entity_type = entity_type
                     ∧ d.contested_attrs ∩ attributes ≠ ∅}
```

#### Level 1 (State Invariant)
The precedent index is maintained as a materialized view, updated on every DECIDE.
Precedent queries return all matching decided deliberations.

**Falsification**: A decided deliberation with matching entity type and attributes
is not returned by a precedent query.

---

### INV-DELIBERATION-004: Bilateral Deliberation Symmetry

**Traces to**: ADRS CR-004 (INV-DELIBERATION-BILATERAL-001), AS-006
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
Deliberation supports both forward and backward flow with identical entity structure:
- Forward: "Given this spec, which of these competing implementations is better?"
- Backward: "Given this implementation, which of these spec interpretations is correct?"

#### Level 1 (State Invariant)
The Deliberation/Position/Decision entity structure is direction-agnostic.
Forward and backward deliberations use the same schema, same lifecycle,
same stability guard, same precedent query.

**Falsification**: The system creates a structural asymmetry where forward
deliberations have capabilities that backward deliberations lack (or vice versa).

---

### INV-DELIBERATION-005: Commitment Weight Integration

**Traces to**: ADRS AS-002
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 2

#### Level 0 (Algebraic Law)
The decision's commitment weight is computed from its forward causal cone:
`w(decision) = |{d' ∈ S : decision ∈ causes*(d')}|`

Decisions with high commitment weight are harder to overturn (require
stronger evidence, higher authority).

#### Level 1 (State Invariant)
When a new decision is recorded, its commitment weight is computed and stored.
As downstream decisions reference it, the weight monotonically increases.

**Falsification**: A decision's commitment weight decreases after downstream
decisions are recorded.

---

### INV-DELIBERATION-006: Competing Branch Resolution

**Traces to**: ADRS PO-007
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 2

#### Level 0 (Algebraic Law)
When a deliberation produces a decision selecting one competing branch:
- The winning branch is committed to trunk
- Losing branches are marked `:abandoned` (remain readable for provenance)
- No losers' datoms leak into trunk

#### Level 1 (State Invariant)
For all reachable states where DECIDE selects a branch:
```
trunk' = trunk ∪ winner.datoms
∀ loser: loser.status = :abandoned
∀ loser: loser.datoms ∩ trunk' = loser.datoms ∩ trunk  (no new datoms from losers)
```

**Falsification**: A losing branch's datoms appear in trunk after the decision.

**Stateright model**: 2 agents with competing branches. Deliberation decides.
Verify loser's datoms never appear in trunk.

---

### §11.5 ADRs

### ADR-DELIBERATION-001: Three Entity Types for Structured Resolution

**Traces to**: ADRS CR-004
**Stage**: 2

#### Problem
What entities are needed for structured conflict resolution?

#### Decision
Three: Deliberation (the process), Position (a stance with rationale and evidence),
Decision (the outcome with method and chosen position). All stored as datoms.

#### Formal Justification
The separation into three entity types mirrors legal proceedings: a case (Deliberation),
arguments (Positions), and a ruling (Decision). This structure enables precedent queries
(CR-007) — past Decisions inform future Deliberations. A single entity type would lose
the distinction between process, argument, and outcome.

---

### ADR-DELIBERATION-002: Five Decision Methods

**Traces to**: ADRS CR-004
**Stage**: 2

#### Problem
What decision methods should be supported?

#### Options
A) Consensus only — simplest, but may never converge
B) Authority only — fast, but ignores evidence quality
C) Five methods: Consensus, Majority, Authority, HumanOverride, Automated

#### Decision
**Option C.** Different conflicts warrant different resolution methods. Low-stakes
conflicts can use Automated (lattice join). Medium-stakes use Majority or Authority.
High-stakes require HumanOverride. Consensus is the ideal but not always achievable.

#### Formal Justification
The method selection aligns with the three-tier conflict routing (CR-002):
Tier 1 (Low) → Automated, Tier 2 (Medium) → Majority/Authority,
Tier 3 (High) → HumanOverride. Consensus is orthogonal — achievable at any tier
but never required.

---

### ADR-DELIBERATION-003: Precedent as Case Law

**Traces to**: ADRS CR-007
**Stage**: 2

#### Problem
Should past deliberation outcomes inform future conflicts?

#### Decision
Yes. Decided deliberations are indexed by entity type and contested attributes.
When a new conflict arises, the system queries for precedent — past decisions
on the same entity type and attributes. Precedent doesn't bind (not stare decisis)
but is surfaced as context for the new deliberation.

#### Formal Justification
The growth-only store guarantees precedent preservation by construction (no deliberation
is ever deleted). Indexing by entity type and attributes is the natural decomposition:
conflicts on the same kind of entity tend to have similar resolution patterns.

---

### ADR-DELIBERATION-004: Crystallization Guard Over Immediate Commit

**Traces to**: ADRS CR-005
**Stage**: 2

#### Problem
Should decisions take effect immediately or after a stability period?

#### Decision
Stability guard. The default `stability_min = 0.7` ensures a decision is not
committed prematurely. The guard checks six conditions (status, thread, confidence,
coherence, conflicts, commitment weight). This prevents the failure mode where
a quick decision with incomplete evidence cascades into downstream errors.

#### Formal Justification
Premature crystallization is an S0-severity failure mode (silently wrong artifacts
with no detection signal). The stability guard is the direct countermeasure.
The cost (delayed commitment) is justified by the risk (cascading incompleteness,
FM-004).

---

### §11.6 Negative Cases

### NEG-DELIBERATION-001: No Decision Without Stability Guard

**Traces to**: ADRS CR-005
**Verification**: `V:PROP`, `V:KANI`

**Safety property**: `□ ¬(decision_recorded ∧ stability < stability_min)`

No decision may be recorded if the stability guard conditions are not met.

**proptest strategy**: Generate random deliberation states with varying stability.
Attempt DECIDE. Verify rejection when stability < threshold.

**Kani harness**: Exhaustive check over all stability dimension combinations
that `decide()` rejects when any dimension is below threshold.

---

### NEG-DELIBERATION-002: No Losing Branch Leak

**Traces to**: ADRS PO-007
**Verification**: `V:PROP`, `V:MODEL`

**Safety property**: `□ ¬(branch.status = :abandoned ∧ branch.datoms ∩ trunk' ⊃ branch.datoms ∩ trunk)`

No new datoms from an abandoned branch appear in trunk.

**Stateright model**: 3 agents, 2 competing branches. Decision selects one.
Verify no datoms from the loser appear in trunk post-decision.

---

### NEG-DELIBERATION-003: No Backward Lifecycle Transition

**Traces to**: ADRS CR-004
**Verification**: `V:PROP`, `V:KANI`

**Safety property**: `□ ¬(lifecycle(d,t2) ⊏ lifecycle(d,t1) for t2 > t1)`

Deliberation lifecycle progresses monotonically: open → active → decided/stalled.
No backward transitions (e.g., decided → active) are permitted.

**proptest strategy**: Generate random transition sequences. Verify lattice
monotonicity after each transition.

---

## §12. GUIDANCE — Methodology Steering

> **Purpose**: Guidance is the anti-drift mechanism — continuous methodology steering
> that counteracts the basin competition between DDIS methodology (Basin A) and pretrained
> coding patterns (Basin B). Without guidance, agents drift into Basin B within 15–20 turns.
>
> **Traces to**: SEED.md §7 (Self-Improvement Loop), §8 (Interface Principles),
> ADRS GU-001–008

### §12.1 Level 0: Algebraic Specification

The guidance system is a **comonad** (GU-001):

```
W(A) = (StoreState, A)

extract : W(A) → A
  — given the store state and a value, extract the value (current guidance)

extend : (W(A) → B) → W(A) → W(B)
  — given a function that uses store context to produce guidance,
    lift it to produce guidance at every store state
```

**Basin competition model** (GU-006):
```
P(Basin_A, t) = probability of methodology-adherent behavior at time t
P(Basin_B, t) = probability of pretrained-pattern behavior at time t

P(Basin_A, t) + P(Basin_B, t) = 1

Without intervention: P(Basin_B, t) → 1 as t → ∞ (pretrained patterns dominate)
With guidance injection: P(Basin_A, t) maintained above threshold τ
```

**Anti-drift energy** is injected via six mechanisms (GU-007) that collectively
maintain `P(Basin_A) > τ`:

```
E_drift = E_preemption + E_injection + E_detection + E_gate + E_alarm + E_harvest

Each Eᵢ > 0 is a positive contribution to Basin A probability.
The system is stable when E_drift > E_decay (natural drift toward Basin B).
```

**Laws**:
- **L1 (Continuous steering)**: Every tool response includes a guidance footer (GU-005)
- **L2 (Spec-language phrasing)**: Guidance uses invariant references and formal structure, not checklists (GU-003)
- **L3 (Intention coherence)**: Actions scored higher if they advance active intentions (GU-008)
- **L4 (Empirical improvement)**: Learned guidance is effectiveness-tracked and pruned below threshold (GU-001)

### §12.2 Level 1: State Machine Specification

**State**: `Σ_guidance = (topology: Graph<GuidanceNode>, learned: Map<EntityId, Effectiveness>, drift_score: f64, mechanisms: [Mechanism; 6])`

**Transitions**:

```
QUERY_GUIDANCE(Σ, agent_state, lookahead) → (actions, tree) where:
  POST: evaluates guidance node predicates against agent state
  POST: returns scored actions + optional lookahead tree (1–5 steps)
  POST: intention-aligned actions scored higher: if postconditions(a) ∩ goals(i) ≠ ∅:
        score(a) += intention_alignment_bonus

INJECT(Σ, tool_response) → tool_response' where:
  POST: tool_response' = tool_response + guidance_footer
  POST: footer contains: (a) specific ddis command, (b) active invariant refs,
        (c) uncommitted observation count, (d) drift warning if applicable
  POST: footer size determined by k*_eff (GU-005)

DETECT_DRIFT(Σ, access_log) → Σ' where:
  POST: analyze transact gap (> 5 bash commands without transact = drift signal)
  POST: analyze tool absence (key tools unused for > threshold turns)
  POST: Σ'.drift_score updated
  POST: if drift_score > threshold: emit GoalDrift signal

EVOLVE(Σ, outcome_data) → Σ' where:
  POST: update effectiveness scores for learned guidance based on outcomes
  POST: prune guidance below effectiveness threshold (0.3)
  POST: effective patterns promoted to higher confidence
```

**Six anti-drift mechanisms** (GU-007):
1. **Guidance Pre-emption**: CLAUDE.md rules require `ddis guidance` before code writing
2. **Guidance Injection**: Every tool response includes next-action footer
3. **Drift Detection**: Access log analysis for transact gap, tool absence
4. **Pre-Implementation Gate**: `ddis pre-check --file <path>` returns GO/CAUTION/STOP
5. **Statusline Drift Alarm**: Uncommitted count, time since last transact, warning indicator
6. **Harvest Safety Net**: Recovers un-transacted observations at session end

### §12.3 Level 2: Implementation Contract

```rust
pub struct GuidanceTopology {
    pub nodes: HashMap<EntityId, GuidanceNode>,
    pub edges: Vec<(EntityId, EntityId)>,
}

pub struct GuidanceNode {
    pub entity: EntityId,
    pub predicate: QueryExpr,  // Datalog predicate over store state
    pub actions: Vec<GuidanceAction>,
    pub learned: bool,
    pub effectiveness: f64,
}

pub struct GuidanceAction {
    pub command: String,          // specific ddis command
    pub invariant_refs: Vec<String>, // e.g., "INV-STORE-001"
    pub postconditions: Vec<EntityId>,
    pub score: f64,
}

pub struct GuidanceFooter {
    pub next_action: String,
    pub invariant_refs: Vec<String>,
    pub uncommitted_count: u32,
    pub drift_warning: Option<String>,
}

impl GuidanceTopology {
    /// Query guidance for current state with lookahead
    pub fn query(&self, store: &Store, agent: &AgentId, lookahead: u8)
        -> GuidanceResult { ... }

    /// Generate footer for tool response
    pub fn footer(&self, store: &Store, k_eff: f64) -> GuidanceFooter { ... }
}
```

### §12.4 Invariants

### INV-GUIDANCE-001: Continuous Injection

**Traces to**: ADRS GU-005
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
`∀ tool_response r: ∃ footer f: r' = r ⊕ f`
Every tool response includes a guidance footer.

#### Level 1 (State Invariant)
The INJECT transition always fires as post-processing on tool output.
No tool response reaches the agent without a guidance footer.

#### Level 2 (Implementation Contract)
The CLI output pipeline appends a footer to every response. The footer
is computed from current store state and k*_eff.

**Falsification**: Any tool response reaches the agent without a guidance footer.

---

### INV-GUIDANCE-002: Spec-Language Phrasing

**Traces to**: ADRS GU-003
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
Guidance text references invariant IDs, formal structures, and spec elements.
Never instruction-language ("do step 1, then step 2") — always spec-language
("INV-STORE-001 requires append-only; current operation would mutate").

#### Level 1 (State Invariant)
Guidance generation templates use invariant references. The template engine
pulls from the store's invariant index, not from hardcoded instruction strings.

**Falsification**: Guidance output contains a numbered checklist or imperative
instruction without invariant reference.

---

### INV-GUIDANCE-003: Intention-Action Coherence

**Traces to**: ADRS GU-008
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
`∀ action a, intention i:
  postconditions(a) ∩ goals(i) ≠ ∅ ⟹ score(a) += intention_alignment_bonus`

Actions that advance active intentions are scored higher in guidance output.

#### Level 1 (State Invariant)
The QUERY_GUIDANCE transition computes intersection between action postconditions
and active intention goals. Non-empty intersection adds a bonus to action score.

**Falsification**: An action that advances an active intention is scored
identically to an action that does not.

---

### INV-GUIDANCE-004: Drift Detection Responsiveness

**Traces to**: ADRS GU-007 (mechanism 3)
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
`transact_gap > 5 ⟹ drift_signal_emitted`
If an agent executes more than 5 bash commands without a transact, the drift
detection mechanism emits a GoalDrift signal.

#### Level 1 (State Invariant)
The DETECT_DRIFT transition monitors the access log for transact gaps and
tool absence patterns. When thresholds are exceeded, a signal is emitted.

**Falsification**: An agent executes 10+ bash commands without a transact
and no drift signal is emitted.

---

### INV-GUIDANCE-005: Learned Guidance Effectiveness Tracking

**Traces to**: ADRS GU-001
**Verification**: `V:PROP`
**Stage**: 4

#### Level 0 (Algebraic Law)
`∀ learned_guidance g: effectiveness(g) < 0.3 ⟹ ◇ retracted(g)`
Learned guidance below the effectiveness threshold is eventually retracted.

Effectiveness is computed from outcome data:
`effectiveness(g) = success_rate(actions_taken_following_g)`

#### Level 1 (State Invariant)
The EVOLVE transition updates effectiveness scores and prunes below-threshold
learned guidance. System-default guidance is never pruned.

**Falsification**: Learned guidance with effectiveness < 0.3 persists after
5+ sessions without being retracted.

---

### INV-GUIDANCE-006: Lookahead via Branch Simulation

**Traces to**: ADRS GU-002
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
Lookahead (1–5 steps) simulates action consequences by creating a virtual branch,
applying hypothetical actions, and evaluating the resulting store state.

`lookahead(actions, n) = evaluate(apply(fork(store), actions[0..n]))`

#### Level 1 (State Invariant)
Virtual branches created for lookahead are never committed to trunk.
Lookahead branches are ephemeral — created, evaluated, and discarded within
the QUERY_GUIDANCE transition.

**Falsification**: A lookahead branch persists after the guidance query completes
or its datoms leak into trunk.

---

### INV-GUIDANCE-007: Dynamic CLAUDE.md Improvement

**Traces to**: ADRS GU-004, PO-014
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
Dynamic CLAUDE.md generation incorporates empirical drift corrections.
Corrections that show no measurable effect after 5 sessions are replaced.

`∀ correction c: sessions_without_effect(c) > 5 ⟹ ◇ replaced(c)`

#### Level 1 (State Invariant)
The GENERATE-CLAUDE-MD operation tracks correction effectiveness across sessions.
Ineffective corrections are replaced by new corrections derived from recent
drift patterns.

**Falsification**: A drift correction persists in generated CLAUDE.md for 10+
sessions with no measurable improvement in the targeted drift metric.

---

### §12.5 ADRs

### ADR-GUIDANCE-001: Comonadic Topology Over Flat Rules

**Traces to**: ADRS GU-001
**Stage**: 1

#### Problem
How should guidance be structured — flat rules or a graph topology?

#### Decision
Comonadic topology: guidance nodes are entities with Datalog predicates.
The `(StoreState, A)` comonad means guidance is always contextualized by the
full store state. Nodes can be traversed, composed, and extended.

#### Formal Justification
Flat rules don't compose (interaction between rules is implicit and fragile).
The comonadic structure makes composition explicit: `extend` lifts a guidance
function to operate over the full topology. Agents can contribute new guidance
nodes that integrate with existing ones via graph edges.

---

### ADR-GUIDANCE-002: Basin Competition as Central Failure Model

**Traces to**: ADRS GU-006
**Stage**: 0

#### Problem
What is the primary failure mode in agent-methodology interaction?

#### Decision
Basin competition between DDIS methodology (Basin A) and pretrained coding patterns
(Basin B). As k*_eff decreases, Basin B's pull increases. At crossover, Basin B
captures the trajectory and the agent's own non-DDIS outputs reinforce it.

#### Formal Justification
This is not a memory problem (bigger context doesn't help — it just delays
the crossover). It is a dynamical systems problem: two attractors competing for
trajectory. The six anti-drift mechanisms are energy injections that maintain
Basin A dominance. Understanding this is prerequisite to designing effective
countermeasures.

---

### ADR-GUIDANCE-003: Six Integrated Mechanisms Over Single Solution

**Traces to**: ADRS GU-007
**Stage**: 1

#### Problem
How many anti-drift mechanisms are needed?

#### Decision
Six. No single mechanism is sufficient — they compose: pre-emption prevents,
injection steers, detection catches, gate forces, alarm makes visible, harvest
recovers. The failure mode of each mechanism is covered by the others.

#### Formal Justification
Defense in depth. Pre-emption fails when agents skip the CLAUDE.md check.
Injection fails when agents ignore footer. Detection fails for novel drift
patterns. Gate fails if agents don't call pre-check. Alarm fails if agent
doesn't read statusline. Harvest fails if session terminates abnormally.
No mechanism is single-point-of-failure because each covers the others' gaps.

---

### ADR-GUIDANCE-004: Spec-Language Over Instruction-Language

**Traces to**: ADRS GU-003
**Stage**: 0

#### Problem
What language register should guidance use?

#### Options
A) Instruction-language — "Do X, then Y, then Z" (checklists)
B) Spec-language — "INV-STORE-001 requires X; current state violates Y"

#### Decision
**Option B.** Spec-language activates the deep reasoning substrate of LLMs
(formal pattern matching, logical inference). Instruction-language activates
the surface substrate (compliance, procedure following). The deep substrate
produces more robust behavior under context pressure.

#### Formal Justification
This is empirically validated: demonstration-style prompts outperform
constraint-style prompts for LLMs. "Demonstration, not constraint list" (IB-002).
Spec-language is the formal analogue of demonstration style applied to methodology.

---

### §12.6 Negative Cases

### NEG-GUIDANCE-001: No Tool Response Without Footer

**Traces to**: ADRS GU-005
**Verification**: `V:PROP`

**Safety property**: `□ ¬(tool_response_sent ∧ ¬footer_appended)`

Every tool response includes a guidance footer. No response reaches the agent
without methodology steering.

**proptest strategy**: Invoke all CLI commands with random arguments. Verify
every output contains a guidance footer section.

---

### NEG-GUIDANCE-002: No Lookahead Branch Leak

**Traces to**: ADRS GU-002
**Verification**: `V:PROP`, `V:KANI`

**Safety property**: `□ ¬(lookahead_branch_committed_to_trunk)`

Virtual branches created for lookahead simulation must never be committed.
They are ephemeral evaluation contexts, not real branches.

**proptest strategy**: Run random lookahead sequences. After each, verify
trunk contains exactly the datoms it had before lookahead.

**Kani harness**: Verify that the `lookahead` function cannot call `commit`.

---

### NEG-GUIDANCE-003: No Ineffective Guidance Persistence

**Traces to**: ADRS GU-001
**Verification**: `V:PROP`

**Safety property**: `□ ¬(learned_guidance_effectiveness < 0.3 ∧ age > 5_sessions ∧ ¬retracted)`

Learned guidance that fails to improve outcomes must be pruned. The system
must not accumulate ineffective guidance that wastes agent attention budget.

**proptest strategy**: Create learned guidance with low effectiveness scores.
Run EVOLVE transitions. Verify pruning occurs within 5 sessions.

---

## §13. BUDGET — Attention Budget Management

> **Purpose**: The attention budget is the fundamental constraint on agent output quality.
> Budget management ensures that high-priority information is never displaced by
> low-priority output, and that tool responses degrade gracefully as context fills.
>
> **Traces to**: SEED.md §8 (Interface Principles), ADRS IB-004–007, IB-011,
> SQ-007, UA-001

### §13.1 Level 0: Algebraic Specification

The attention budget is a **monotonically decreasing resource**:

```
k*_eff : Time → [0, 1]
  — effective remaining attention at time t, measured from actual context consumption

Q(t) = k*_eff(t) × attention_decay(k*_eff(t))
  — quality-adjusted budget incorporating attention degradation

attention_decay(k) =
  | 1.0           if k > 0.6      (full quality)
  | k / 0.6       if 0.3 ≤ k ≤ 0.6 (linear degradation)
  | (k / 0.3)²    if k < 0.3      (quadratic degradation)
```

**Five-level output precedence**:
```
System > Methodology > UserRequested > Speculative > Ambient

Truncation order: Ambient first, System last.
Lower-priority output is truncated before higher-priority output is touched.
```

**Projection pyramid** (SQ-007):
```
π₀ = full datoms           (> 2000 tokens available)
π₁ = entity summaries      (500–2000 tokens)
π₂ = type summaries         (200–500 tokens)
π₃ = store summary          (≤ 200 tokens — single-line status + single guidance action)
```

**Laws**:
- **L1 (Budget monotonicity)**: `k*_eff(t+1) ≤ k*_eff(t)` — effective attention never increases within a session
- **L2 (Precedence ordering)**: Truncation always follows the five-level ordering — no level N content is truncated while level N+1 content remains
- **L3 (Minimum output)**: `output_size ≥ MIN_OUTPUT` (50 tokens) — even at critical budget, a harvest signal is always emitted

### §13.2 Level 1: State Machine Specification

**State**: `Σ_budget = (k_eff: f64, q: f64, output_budget: u32, precedence_stack: [Level; 5])`

**Transitions**:

```
MEASURE(Σ, context_data) → Σ' where:
  POST: Σ'.k_eff computed from measured context consumption
  POST: Σ'.q = Q(t) formula applied
  POST: Σ'.output_budget = max(50, Σ'.q × 200000 × 0.05)

ALLOCATE(Σ, content, priority) → output where:
  POST: content truncated to fit output_budget
  POST: truncation follows precedence: lowest priority first
  POST: guidance compression follows IB-006:
        k > 0.7: full (100–200 tokens)
        0.4–0.7: compressed (30–60 tokens)
        ≤ 0.4: minimal (10–20 tokens)
        ≤ 0.2: harvest signal only

PROJECT(Σ, entities, budget) → projection where:
  POST: pyramid level selected based on budget:
        > 2000: π₀ for top, π₁ for others
        500–2000: π₁/π₂
        200–500: π₂ for top, omit others
        ≤ 200: π₃ (single-line)
```

**Budget source precedence** (IB-004):
1. `--budget` flag (explicit)
2. `--context-used` flag (from caller)
3. Session state file `.ddis/session/context.json` (from statusline hook)
4. Transcript tail-parse (fallback)
5. Conservative default: 500 tokens

Staleness threshold: 30 seconds. Sources older than 30s are deprioritized.

### §13.3 Level 2: Implementation Contract

```rust
pub struct BudgetManager {
    pub k_eff: f64,
    pub q: f64,
    pub output_budget: u32,
}

#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub enum OutputPrecedence {
    Ambient = 0,
    Speculative = 1,
    UserRequested = 2,
    Methodology = 3,
    System = 4,
}

impl BudgetManager {
    /// Measure k*_eff from context data
    pub fn measure(&mut self, context_used_pct: f64) {
        self.k_eff = 1.0 - context_used_pct;
        self.q = self.k_eff * self.attention_decay(self.k_eff);
        self.output_budget = (50.0_f64).max(self.q * 200_000.0 * 0.05) as u32;
    }

    fn attention_decay(&self, k: f64) -> f64 {
        if k > 0.6 { 1.0 }
        else if k >= 0.3 { k / 0.6 }
        else { (k / 0.3).powi(2) }
    }

    /// Project entities to the appropriate pyramid level
    pub fn project(&self, entities: &[EntitySummary]) -> Projection {
        match self.output_budget {
            b if b > 2000 => Projection::Full(entities),
            b if b > 500  => Projection::EntitySummary(entities),
            b if b > 200  => Projection::TypeSummary(entities),
            _             => Projection::StoreSummary,
        }
    }
}
```

### §13.4 Invariants

### INV-BUDGET-001: Output Budget as Hard Cap

**Traces to**: ADRS IB-004
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 1

#### Level 0 (Algebraic Law)
`∀ tool_response r: |r| ≤ max(MIN_OUTPUT, Q(t) × W × budget_fraction)`

where W = context window size, budget_fraction = 0.05 (5% of remaining capacity).

#### Level 1 (State Invariant)
The ALLOCATE transition enforces the cap. Content exceeding the budget is
truncated according to precedence ordering.

#### Level 2 (Implementation Contract)
```rust
#[kani::ensures(|output| output.len() <= self.output_budget as usize)]
fn allocate(&self, content: &[OutputBlock]) -> Vec<u8> { ... }
```

**Falsification**: A tool response exceeds the computed output budget.

---

### INV-BUDGET-002: Precedence-Ordered Truncation

**Traces to**: ADRS IB-004
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
`∀ content blocks b₁, b₂ where priority(b₁) < priority(b₂):
  truncated(b₂) ⟹ truncated(b₁)`

Higher-priority content is never truncated while lower-priority content remains.

#### Level 1 (State Invariant)
The ALLOCATE transition sorts content by precedence and fills from highest to lowest.
When budget is exhausted, remaining lower-priority content is truncated.

**Falsification**: System output truncates a Methodology-level block while
Speculative-level blocks remain in the output.

---

### INV-BUDGET-003: Quality-Adjusted Degradation

**Traces to**: ADRS IB-005
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 1

#### Level 0 (Algebraic Law)
The Q(t) formula accounts for attention quality degradation:
```
Q(t) = k*_eff(t) × attention_decay(k*_eff(t))

Q(t) degrades faster than k*_eff(t) when k*_eff < 0.6
  because attention quality drops before context fills.
```

#### Level 1 (State Invariant)
The MEASURE transition computes Q(t) using the piecewise attention_decay function.
Output budget is derived from Q(t), not raw k*_eff.

**Falsification**: Output budget is computed from raw k*_eff without applying
the attention_decay quality adjustment.

---

### INV-BUDGET-004: Guidance Compression by Budget

**Traces to**: ADRS IB-006
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
Guidance footer size is a function of k*_eff:
```
k > 0.7:    full (100–200 tokens)
0.4–0.7:    compressed (30–60 tokens)
≤ 0.4:      minimal (10–20 tokens)
≤ 0.2:      harvest signal only ("Run ddis harvest")
```

#### Level 1 (State Invariant)
The INJECT transition (from GUIDANCE namespace) selects footer size
based on the current k*_eff from the budget manager.

**Falsification**: At k*_eff = 0.1, the guidance footer is 100+ tokens instead
of a minimal harvest signal.

---

### INV-BUDGET-005: Command Attention Profile

**Traces to**: ADRS IB-007
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
Commands are classified by attention cost:
```
CHEAP    (≤ 50 tokens):  status, guidance, frontier, branch ls
MODERATE (50–300):        associate, query, assemble, diff
EXPENSIVE (300+):         assemble --full, seed
META     (side effects):  harvest, transact, merge
```

The budget manager adjusts output to stay within the allocated cost.

#### Level 1 (State Invariant)
Each CLI command has a declared attention profile. The output pipeline
respects the profile ceiling, truncating to fit.

**Falsification**: A CHEAP command produces 300+ tokens of output.

---

### §13.5 ADRs

### ADR-BUDGET-001: Measured Context Over Heuristic

**Traces to**: ADRS IB-005
**Stage**: 1

#### Problem
Should attention budget be estimated heuristically or measured from actual consumption?

#### Decision
Measured. Claude Code exposes `context_window.used_percentage` via the statusline hook.
This gives ground truth. The heuristic `k*_eff = k*_base × e^{-0.03n}` becomes fallback
only when measurement is unavailable.

#### Formal Justification
Heuristic is inaccurate because conversation structure varies — a session with many
long tool outputs consumes context faster than one with short exchanges. Measured
consumption eliminates this source of error.

---

### ADR-BUDGET-002: Piecewise Attention Decay

**Traces to**: ADRS IB-005
**Verification**: Used in Q(t) computation
**Stage**: 1

#### Problem
How should attention quality degrade with context consumption?

#### Decision
Piecewise: full quality above 60% remaining, linear degradation 30–60%,
quadratic degradation below 30%.

#### Formal Justification
Empirical observation: LLM attention quality degrades faster than a simple linear
model would predict. The piecewise function captures three regimes: comfortable
(no degradation), pressured (graceful degradation), critical (rapid degradation).
The quadratic regime below 30% reflects the observed cliff in output quality.

---

### ADR-BUDGET-003: Rate-Distortion Framework

**Traces to**: ADRS IB-011
**Stage**: 1

#### Problem
What theoretical framework governs the budget-information tradeoff?

#### Decision
Rate-distortion theory. The interface is a channel with rate constraint (budget).
The system maximizes information value while minimizing distortion (loss of
important facts) at the given rate. The projection pyramid (π₀–π₃) is the
codebook with decreasing rate requirements.

#### Formal Justification
Rate-distortion is the information-theoretic framework for lossy compression
with quality guarantees. It formalizes the intuition that "less budget = less
detail, but the most important things survive." The precedence ordering defines
what "most important" means.

---

### §13.6 Negative Cases

### NEG-BUDGET-001: No Budget Overflow

**Traces to**: ADRS IB-004
**Verification**: `V:PROP`, `V:KANI`

**Safety property**: `□ ¬(output_size > output_budget ∧ output_budget > MIN_OUTPUT)`

No tool response exceeds the computed output budget (except at the minimum
floor of 50 tokens).

**proptest strategy**: Generate random tool outputs at various budget levels.
Verify truncation to budget ceiling in all cases.

**Kani harness**: Verify `allocate()` output size ≤ budget for all inputs.

---

### NEG-BUDGET-002: No High-Priority Truncation Before Low

**Traces to**: ADRS IB-004
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ b_high, b_low: priority(b_high) > priority(b_low) ∧ truncated(b_high) ∧ ¬truncated(b_low))`

Precedence ordering is inviolable. System and Methodology content is never
truncated while Speculative or Ambient content remains.

**proptest strategy**: Generate output with blocks at all five precedence levels.
Apply budget pressure. Verify truncation order matches precedence.

---

## §14. INTERFACE — CLI/MCP/TUI Layers

> **Purpose**: The interface layers are the graded information channels through which
> agents, humans, and machines interact with the store. Each layer serves a different
> consumer at a different frequency, all backed by the same datom store.
>
> **Traces to**: SEED.md §8 (Interface Principles), ADRS IB-001–003, IB-008–009,
> SR-011, AA-003

### §14.1 Level 0: Algebraic Specification

The interface is a **five-layer graded information channel**:

```
Layer 0 (Ambient):    CLAUDE.md — ~80 tokens, k*-exempt, always present
Layer 1 (CLI):        Rust binary — primary agent interface, budget-aware
Layer 2 (MCP):        Thin wrapper — machine-to-machine, nine tools
Layer 3 (Guidance):   Comonadic — spec-language, injected in every response
Layer 4 (TUI):        Subscription-driven — human monitoring dashboard
Layer 4.5 (Statusline): Bridge — persistent low-bandwidth agent↔human signal
```

**Information flow**:
```
Store → CLI (agent reads/writes)
Store → MCP → Agent (machine-to-machine)
Store → TUI (human reads)
TUI → Store (human injects signals via IB-009)
Statusline → Session State → CLI (budget measurement)
```

**Laws**:
- **L1 (Layer independence)**: Each layer can operate independently of other layers
- **L2 (Store as sole truth)**: All layers read from and write to the same datom store. No layer-local state that isn't a projection of the store.
- **L3 (Budget awareness)**: Layers 1–3 respect the attention budget. Layer 0 is k*-exempt (always present). Layer 4/4.5 is unconstrained (human, not agent).

### §14.2 Level 1: State Machine Specification

**State**: `Σ_interface = (cli: CLIState, mcp: MCPState, tui: TUIState, statusline: StatuslineState)`

**Transitions**:

```
CLI_COMMAND(Σ, command, args, budget) → (output, Σ') where:
  PRE:  command ∈ known_commands
  POST: output = execute(command, args, store)
  POST: |output| ≤ budget (truncated per precedence)
  POST: output includes guidance footer
  POST: store updated if command is META type

MCP_CALL(Σ, tool_name, params) → (result, Σ') where:
  PRE:  tool_name ∈ {ddis_status, ddis_guidance, ddis_associate, ddis_query,
                      ddis_transact, ddis_branch, ddis_signal, ddis_harvest, ddis_seed}
  POST: reads session state, computes Q(t), passes --budget to CLI
  POST: appends pending notifications
  POST: updates session state
  POST: checks harvest warning thresholds

TUI_UPDATE(Σ, subscriptions) → display where:
  POST: continuous projection via SUBSCRIBE
  POST: NOT k*-constrained (human interface)
  POST: delegation changes and conflicts above threshold trigger notification

SIGNAL_INJECT(Σ, signal_from_human) → Σ' where:
  POST: signal recorded as datom (high authority — human source)
  POST: queued in MCP notification queue for agent's next tool response
  POST: entity type `:signal/*` with provenance `:observed`

STATUSLINE_TICK(Σ, context_data) → Σ' where:
  POST: writes session state to .ddis/session/context.json
  POST: fields: used_percentage, input_tokens, remaining_tokens,
        k_eff, quality_adjusted, output_budget, timestamp, session_id
  POST: zero cost to agent context (side effect only)
```

### §14.3 Level 2: Implementation Contract

```rust
/// CLI output modes (IB-002)
pub enum OutputMode {
    Structured,  // JSON — machine-parseable
    Agent,       // 100–300 tokens, headline + entities + signals + guidance + pointers
    Human,       // TTY — full formatting, color, tables
}

/// MCP server — thin wrapper calling CLI for all computation
pub struct MCPServer {
    pub session_state: SessionState,
    pub notification_queue: Vec<Signal>,
}

/// Nine MCP tools (IB-003)
pub enum MCPTool {
    Status,     // cheap: ≤50 tokens
    Guidance,   // cheap: ≤50 tokens
    Associate,  // moderate: 50–300 tokens
    Query,      // moderate: 50–300 tokens
    Transact,   // meta: side effect
    Branch,     // meta: side effect
    Signal,     // meta: side effect
    Harvest,    // meta: side effect
    Seed,       // expensive: 300+ tokens
}

/// Session state file (SR-011)
#[derive(Serialize, Deserialize)]
pub struct SessionState {
    pub used_percentage: f64,
    pub input_tokens: u64,
    pub remaining_tokens: u64,
    pub k_eff: f64,
    pub quality_adjusted: f64,
    pub output_budget: u32,
    pub timestamp: u64,
    pub session_id: String,
}

/// TUI — subscription-driven push projection
pub struct TUIState {
    pub subscriptions: Vec<Subscription>,
    pub active_display: DisplayState,
}
```

### §14.4 Invariants

### INV-INTERFACE-001: Three CLI Output Modes

**Traces to**: ADRS IB-002
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
The CLI produces output in exactly one of three modes per invocation:
Structured (JSON), Agent (budget-constrained), Human (TTY-formatted).
Mode selection is explicit (flag) or inferred from terminal context.

#### Level 1 (State Invariant)
Every CLI_COMMAND invocation selects exactly one mode. The mode determines
formatting, token budget, and content selection.

**Falsification**: A CLI command produces mixed-mode output (e.g., JSON with
TTY escape codes, or agent-mode output without budget constraint).

---

### INV-INTERFACE-002: MCP as Thin Wrapper

**Traces to**: ADRS IB-003
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
The MCP server performs no computation. All computation is delegated to the CLI.
MCP adds: session state management, budget adjustment, tool descriptions,
notification queuing.

`∀ mcp_call: result = cli_execute(mcp_call.to_cli_args()) + mcp_metadata`

#### Level 1 (State Invariant)
Every MCP_CALL transition invokes a CLI command as a subprocess. The MCP server
reads/writes session state and manages notifications but does not duplicate
any CLI logic.

**Falsification**: The MCP server implements query parsing, store access, or
any other logic that exists in the CLI binary.

---

### INV-INTERFACE-003: Nine MCP Tools

**Traces to**: ADRS IB-003
**Verification**: `V:PROP`, `V:TYPE`
**Stage**: 0

#### Level 0 (Algebraic Law)
The MCP server exposes exactly nine tools:
`{status, guidance, associate, query, transact, branch, signal, harvest, seed}`

#### Level 1 (State Invariant)
The tool set is fixed. Adding tools requires a spec update.
Each tool maps to a specific CLI command.

#### Level 2 (Implementation Contract)
```rust
// Type-level guarantee: exactly 9 tools
const MCP_TOOLS: [MCPTool; 9] = [
    MCPTool::Status, MCPTool::Guidance, MCPTool::Associate,
    MCPTool::Query, MCPTool::Transact, MCPTool::Branch,
    MCPTool::Signal, MCPTool::Harvest, MCPTool::Seed,
];
```

**Falsification**: The MCP server exposes a tool not in the defined set of nine.

---

### INV-INTERFACE-004: Statusline Zero-Cost to Agent

**Traces to**: ADRS IB-001 (Layer 4.5), SR-011
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
The statusline hook produces side effects (writes session state file) but
consumes zero tokens from the agent's context window.

#### Level 1 (State Invariant)
STATUSLINE_TICK writes to `.ddis/session/context.json` as an external side effect.
The statusline output is consumed by the human display and the CLI budget system,
never by the agent's context.

**Falsification**: Statusline output appears in the agent's context window,
consuming attention budget.

---

### INV-INTERFACE-005: TUI Subscription Liveness

**Traces to**: ADRS IB-008
**Verification**: `V:PROP`
**Stage**: 4

#### Level 0 (Algebraic Law)
Delegation changes and conflicts above severity threshold trigger TUI notification
within one refresh cycle.

`∀ event e where severity(e) ≥ threshold: ◇ displayed_in_tui(e)`

#### Level 1 (State Invariant)
The TUI subscribes to store changes. When a matching event occurs (delegation change,
conflict above threshold), the TUI display updates within the subscription's
refresh interval.

**Falsification**: A High-severity conflict is recorded in the store but the TUI
does not display a notification.

---

### INV-INTERFACE-006: Human Signal Injection

**Traces to**: ADRS IB-009
**Verification**: `V:PROP`
**Stage**: 3

#### Level 0 (Algebraic Law)
A human can inject signals from the TUI. The signal is:
1. Recorded as a datom with human provenance (`:observed`, axiomatically high authority)
2. Queued in the MCP notification queue
3. Delivered to the agent in the next tool response

#### Level 1 (State Invariant)
SIGNAL_INJECT always produces both a datom and a notification queue entry.
The agent receives the signal at the next MCP_CALL.

**Falsification**: A human injects a signal from TUI and the agent never receives it.

---

### INV-INTERFACE-007: Proactive Harvest Warning

**Traces to**: ADRS IB-012
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
Q(t) < 0.15 (~75% consumed) ⟹ every response includes harvest warning
Q(t) < 0.05 (~85% consumed) ⟹ CLI emits ONLY the harvest imperative
```

#### Level 1 (State Invariant)
When k*_eff drops below thresholds, the response format changes:
below 0.15, a harvest warning is appended; below 0.05, only the harvest
imperative is emitted, suppressing all other output.

**Falsification**: k*_eff = 0.03 and the CLI still produces full output without
a harvest warning.

---

### §14.5 ADRs

### ADR-INTERFACE-001: Five Layers Plus Statusline Bridge

**Traces to**: ADRS IB-001
**Stage**: 0–4 (layers implemented across stages)

#### Problem
How many interface layers are needed, and what does each serve?

#### Decision
Five layers plus a Layer 4.5 statusline bridge:
- Layer 0 (Ambient): CLAUDE.md — always-present, ~80 tokens, most important
  ("agents fail to invoke tools 56% without ambient awareness")
- Layer 1 (CLI): Primary agent interface, budget-aware
- Layer 2 (MCP): Machine-to-machine, thin wrapper
- Layer 3 (Guidance): Comonadic, injected in responses
- Layer 4 (TUI): Human monitoring, subscription-driven
- Layer 4.5 (Statusline): Zero-cost bridge, writes session state

#### Formal Justification
Each layer serves a different consumer (agent/machine/human) at a different
frequency (always/per-command/continuous). The statusline bridge is the critical
innovation: it connects the human display (Layer 4) to the agent budget system
(Layer 1) with zero context cost to the agent.

---

### ADR-INTERFACE-002: Agent-Mode Demonstration Style

**Traces to**: ADRS IB-002
**Stage**: 0

#### Problem
How should agent-mode CLI output be structured?

#### Decision
Demonstration style: headline + entities (3–7) + signals (0–3) + guidance (1–3)
+ pointers (1–3). Total: 100–300 tokens. "Demonstration, not constraint list."

#### Formal Justification
Demonstration-style output activates the deep reasoning substrate of LLMs
(pattern matching, analogy, formal inference). Constraint-style output
("DO NOT do X, MUST do Y") activates the surface compliance substrate,
which produces brittle behavior under context pressure.

---

### ADR-INTERFACE-003: Store-Mediated Trajectory Management

**Traces to**: ADRS IB-010
**Stage**: 0

#### Problem
How should agent work sessions be managed across conversation boundaries?

#### Decision
Store-mediated: `ddis harvest` extracts durable facts, `ddis seed` generates
carry-over. Agent lifecycle: SEED → work 20–30 turns → HARVEST → reset → GOTO SEED.

Seed output follows a five-part template:
1. Context (1–2 sentences)
2. Invariants established
3. Artifacts produced
4. Open questions from deliberations
5. Active guidance

#### Formal Justification
The store is the sole truth (FD-012). Trajectory management through the store
means conversation boundaries become knowledge extraction points, not knowledge
loss points. The five-part template provides structure for the seed while
keeping it within budget.

---

### §14.6 Negative Cases

### NEG-INTERFACE-001: No Layer-Local State

**Traces to**: ADRS AA-003
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ layer_state that is not a projection of the store)`

No interface layer maintains state that isn't derivable from the store.
Session state (SR-011) is a projection of measured context data.
MCP notification queues are projections of pending signals.

**proptest strategy**: After any sequence of interface operations, verify
that all layer state can be reconstructed from the store alone.

---

### NEG-INTERFACE-002: No MCP Logic Duplication

**Traces to**: ADRS IB-003
**Verification**: `V:PROP`

**Safety property**: `□ ¬(mcp_server implements logic that exists in cli_binary)`

The MCP server is a thin wrapper. Any computation that appears in both the
MCP server and the CLI binary is a duplication bug.

**proptest strategy**: Structural analysis — verify MCP tool handlers
contain only: subprocess call, session state read/write, notification
queue management. No query parsing, store access, or domain logic.

---

### NEG-INTERFACE-003: No Harvest Warning Suppression

**Traces to**: ADRS IB-012
**Verification**: `V:PROP`

**Safety property**: `□ ¬(Q(t) < 0.15 ∧ response_without_harvest_warning)`

When context is critically low, the harvest warning must appear. No configuration,
flag, or output mode may suppress it.

**proptest strategy**: Set k*_eff to values below 0.15. Invoke all CLI commands.
Verify every response contains a harvest warning.

---

---

## §15. Uncertainty Register

> **Purpose**: All claims in this specification with confidence < 1.0, organized by
> resolution urgency. Each entry identifies what is uncertain, why it matters, what
> would resolve it, and what breaks if the assumption is wrong.
>
> **Methodology**: Uncertainty markers follow UA-006 — explicit confidence levels with
> resolution criteria. Claims without markers are considered confidence 1.0 (settled).

### §15.1 Explicit Uncertainty Markers

These are claims explicitly flagged during specification production.

#### UNC-BILATERAL-001: Fitness Function Component Weights

**Source**: ADR-BILATERAL-001 (§10.5)
**Confidence**: 0.6
**Stage affected**: 1+

**Claim**: F(S) = 0.18×V + 0.18×C + 0.18×(1-D) + 0.13×H + 0.13×(1-K) + 0.08×(1-I) + 0.12×(1-U)

**Why uncertain**: Weights are derived from theoretical analysis of component importance
(primary triad V/C/D = 0.18, secondary pair H/K = 0.13, etc.) but have not been
calibrated against empirical data from actual Braid usage.

**Impact if wrong**: Fitness score gives misleading convergence signal. A high F(S) could
mask real divergence if a low-weight component is actually critical, or vice versa.

**Resolution**: Run Stage 0 for ≥10 sessions. Compute F(S) after each. Compare weights
that correlate with successful outcomes (sessions where harvested knowledge was correct
and complete) against weights that correlate with failures. Adjust weights to maximize
predictive power.

**What breaks**: INV-BILATERAL-001 (monotonic convergence) still holds regardless of weights.
The question is whether convergence is toward actual coherence or toward a local optimum.

---

#### UNC-BILATERAL-002: Divergence Boundary Weights

**Source**: ADR-BILATERAL-002 (§10.5)
**Confidence**: 0.5
**Stage affected**: 1+

**Claim**: D(spec, impl) = Σᵢ wᵢ × |boundary_gap(i)| with default equal weights
across the four boundaries (Intent→Spec, Spec→Spec, Spec→Impl, Impl→Behavior).

**Why uncertain**: Different projects may have very different boundary-gap distributions.
A project with a stable spec but turbulent implementation needs different weights than
one where intent is shifting.

**Impact if wrong**: Divergence metric under-weights the critical boundary, causing the
bilateral loop to focus remediation effort on the wrong gaps.

**Resolution**: After Stage 0, analyze which boundaries produce the most actionable
gaps. Weight boundaries proportional to their remediation cost × occurrence frequency.
Consider per-project weight profiles.

**What breaks**: The bilateral loop still detects all gaps (completeness is structural,
not weight-dependent). But prioritization of remediation effort may be misguided.

---

### §15.2 Implicit Uncertainties

These are areas where the specification makes commitments that depend on assumptions
not yet validated by implementation experience.

#### UNC-STORE-001: Content-Addressable EntityId Collision Rate

**Source**: INV-STORE-002, ADR-STORE-002
**Confidence**: 0.95
**Stage affected**: 0

**Claim**: SHA-256 hash of content produces unique EntityIds with negligible collision
probability.

**Why uncertain**: SHA-256 collision probability is astronomically low (2^{-128} for
random inputs) but content-addressed systems at scale can hit birthday-bound issues
with certain workload patterns.

**Impact if wrong**: Two different entities map to the same EntityId. Silent data
corruption — one entity's attributes overwrite another's.

**Resolution**: Monitor EntityId generation during implementation. Verify uniqueness
across ≥10^6 datoms. Consider a secondary check (entity content comparison on hash
match) as defense in depth.

**What breaks**: INV-STORE-002 (content identity), INV-STORE-003 (merge deduplication).

---

#### UNC-STORE-002: HLC Clock Skew Tolerance

**Source**: INV-STORE-008, ADR-STORE-004
**Confidence**: 0.9
**Stage affected**: 0

**Claim**: Hybrid Logical Clocks maintain causal ordering across agents with bounded
clock skew.

**Why uncertain**: HLC assumes clock skew is bounded. On a single VPS with NTP, skew
is typically <1ms. But container environments, VM migration, or suspended processes
can introduce larger skew.

**Impact if wrong**: Transaction ordering violations. Causally-later transactions appear
before causally-earlier ones, breaking frontier monotonicity (INV-STORE-009).

**Resolution**: Implement HLC with configurable max-skew parameter. Alert when observed
skew exceeds threshold. For Stage 0 (single VPS), this is very low risk.

**What breaks**: INV-STORE-008 (HLC monotonicity), INV-STORE-009 (frontier durability).

---

#### UNC-QUERY-001: Datalog Evaluation Performance at Scale

**Source**: INV-QUERY-002, ADR-QUERY-001
**Confidence**: 0.8
**Stage affected**: 1+

**Claim**: Semi-naive Datalog evaluation is efficient for Braid's query patterns
at expected scale (thousands of datoms, dozens of query patterns).

**Why uncertain**: Semi-naive evaluation is well-studied for databases but Braid's
query patterns include recursive graph traversal (causal-ancestor, depends-on),
aggregation (uncertainty tensor), and derived functions (spectral authority). These
may have pathological performance characteristics on certain store topologies.

**Impact if wrong**: Query latency exceeds acceptable limits, making the CLI unusable
for interactive agent workflows. Budget-aware output degrades to π₃ not from attention
pressure but from query timeout.

**Resolution**: Benchmark query patterns against synthetic stores of 10^3, 10^4, 10^5
datoms. Identify performance cliffs. Optimize hot paths (EAVT/AEVT index lookups).
Consider incremental materialization for Stratum 4–5 queries.

**What breaks**: INV-QUERY-002 (fixpoint termination — technically guaranteed by
Datalog semantics, but timeout is a practical termination condition).

---

#### UNC-HARVEST-001: Proactive Warning Thresholds

**Source**: INV-HARVEST-005, INV-INTERFACE-007
**Confidence**: 0.7
**Stage affected**: 0

**Claim**: Q(t) < 0.15 (~75% consumed) triggers harvest warning; Q(t) < 0.05 (~85%)
triggers harvest-only mode.

**Why uncertain**: Thresholds are calibrated to Claude Code context windows (~200K tokens)
with observed attention degradation patterns. Different LLM providers, model sizes, or
future context window changes may shift the optimal thresholds.

**Impact if wrong**: Warnings too early → annoyance, wasted budget on premature harvest.
Warnings too late → unharvested knowledge loss (FM-001).

**Resolution**: Track harvest outcomes vs. Q(t) at harvest time across 50+ sessions.
Compute the Q(t) threshold below which harvest quality degrades measurably. Adjust
thresholds to match.

**What breaks**: INV-HARVEST-005 (warning correctness), INV-INTERFACE-007 (proactive warning).

---

#### UNC-GUIDANCE-001: Basin Competition Crossover Point

**Source**: ADR-GUIDANCE-002, INV-GUIDANCE-004
**Confidence**: 0.7
**Stage affected**: 0

**Claim**: Without intervention, agents drift to Basin B (pretrained patterns) within
15–20 turns. The six anti-drift mechanisms maintain Basin A dominance.

**Why uncertain**: The "15–20 turns" figure is based on observed behavior with specific
LLM models (Claude). Different models may have different crossover points. The
effectiveness of the six mechanisms is theoretical — no empirical measurement yet.

**Impact if wrong**: If crossover is earlier (10 turns), the mechanisms may be insufficient.
If later (30+ turns), the mechanisms may be unnecessarily aggressive (wasting budget).

**Resolution**: Instrument drift detection during Stage 0. Measure turn count at which
agents first skip a DDIS step (transact gap, guidance miss). Plot Basin A probability
over turns. Calibrate mechanism intensity to the measured crossover.

**What breaks**: INV-GUIDANCE-004 (drift detection responsiveness — the threshold of 5
bash commands may be too lenient or too strict).

---

#### UNC-DELIBERATION-001: Crystallization Stability Threshold

**Source**: INV-DELIBERATION-002, ADR-DELIBERATION-004
**Confidence**: 0.7
**Stage affected**: 2

**Claim**: Default stability_min = 0.7 provides the right balance between premature
crystallization and unnecessary delay.

**Why uncertain**: The threshold interacts with commitment weight, confidence, coherence,
and conflict state. The optimal threshold may vary by entity type (architectural
decisions need higher stability than implementation details).

**Impact if wrong**: Too high → deliberation takes too long, blocking downstream work.
Too low → premature decisions create cascading incompleteness (FM-004).

**Resolution**: Run deliberation simulations with varying thresholds during Stage 2.
Measure: time-to-decision, downstream error rate from premature decisions, developer
frustration from blocked work. Find the Pareto frontier.

**What breaks**: INV-DELIBERATION-002 (stability guard enforcement — the invariant holds
regardless, but the quality of decisions may suffer).

---

#### UNC-SCHEMA-001: Seventeen Axiomatic Attributes Sufficiency

**Source**: INV-SCHEMA-001, ADR-SCHEMA-001
**Confidence**: 0.85
**Stage affected**: 0

**Claim**: Exactly 17 axiomatic meta-schema attributes are sufficient to bootstrap
the full schema system.

**Why uncertain**: The 17 were identified through design analysis (Transcript 02:379–420)
but have not been tested against a real implementation. Missing an attribute at Layer 0
requires a breaking change to the genesis transaction.

**Impact if wrong**: Schema system cannot express a required concept. Workaround:
add attributes at Layer 1+ (non-breaking) or revise genesis (breaking — all stores
become incompatible).

**Resolution**: Implement genesis transaction during Stage 0. Attempt to define all
Layer 1–5 schema using only the 17 axiomatic attributes. Any failure reveals a gap.

**What breaks**: INV-SCHEMA-001 (genesis completeness), INV-SCHEMA-008 (self-description).

---

#### UNC-RESOLUTION-001: Per-Attribute Resolution Mode Ergonomics

**Source**: ADR-RESOLUTION-001, ADR-RESOLUTION-002
**Confidence**: 0.8
**Stage affected**: 0

**Claim**: Schema authors can and will correctly declare resolution modes (LWW, Lattice,
Multi) for each attribute.

**Why uncertain**: This places a cognitive burden on schema designers. Incorrect mode
selection (e.g., LWW on an attribute that should be Lattice) silently loses data.
There's no mechanism to detect "probably wrong" mode selections.

**Impact if wrong**: Silent data loss or incorrect conflict resolution. The system
behaves correctly per its configuration but the configuration doesn't match intent.

**Resolution**: Provide sensible defaults (LWW for scalar attributes, Multi for set-valued,
Lattice for lifecycle/status). Emit warnings when resolution mode selection looks unusual
(e.g., LWW on a set-valued attribute). Consider a `ddis schema audit` command.

**What breaks**: INV-RESOLUTION-001 (algebraic law holds by construction, but semantic
correctness depends on correct mode selection).

---

### §15.3 Summary

| ID | Confidence | Stage | Impact | Resolution Urgency |
|----|-----------|-------|--------|-------------------|
| UNC-BILATERAL-001 | 0.6 | 1+ | Misleading convergence signal | Medium — calibrate during Stage 0 |
| UNC-BILATERAL-002 | 0.5 | 1+ | Misguided remediation priority | Medium — calibrate during Stage 0 |
| UNC-STORE-001 | 0.95 | 0 | Silent data corruption (extremely unlikely) | Low — monitor during implementation |
| UNC-STORE-002 | 0.9 | 0 | Transaction ordering violation | Low — mitigated by single-VPS deployment |
| UNC-QUERY-001 | 0.8 | 1+ | Query timeout in interactive use | Medium — benchmark during Stage 0 |
| UNC-HARVEST-001 | 0.7 | 0 | Knowledge loss or wasted budget | High — calibrate during Stage 0 |
| UNC-GUIDANCE-001 | 0.7 | 0 | Insufficient or excessive drift correction | High — instrument during Stage 0 |
| UNC-DELIBERATION-001 | 0.7 | 2 | Premature or delayed decisions | Medium — simulate during Stage 2 |
| UNC-SCHEMA-001 | 0.85 | 0 | Missing bootstrap attribute | High — verify during Stage 0 |
| UNC-RESOLUTION-001 | 0.8 | 0 | Incorrect conflict resolution | Medium — provide defaults + warnings |

**By resolution urgency**:
- **High** (resolve during Stage 0): UNC-HARVEST-001, UNC-GUIDANCE-001, UNC-SCHEMA-001
- **Medium** (resolve during Stage 0–2): UNC-BILATERAL-001/002, UNC-QUERY-001, UNC-DELIBERATION-001, UNC-RESOLUTION-001
- **Low** (monitor, resolve if observed): UNC-STORE-001/002

---

## §16. Verification Plan

> **Purpose**: Maps every invariant to its verification method(s), tool, implementation
> stage, and CI gate. This section is the implementor's guide to "how do I prove this
> invariant holds?"

### §16.1 Per-Invariant Verification Matrix

#### STORE (14 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-STORE-001 | V:TYPE | V:KANI | rustc + kani | compile + kani | 0 |
| INV-STORE-002 | V:TYPE | — | rustc | compile | 0 |
| INV-STORE-003 | V:TYPE | V:PROP | rustc + proptest | compile + test | 0 |
| INV-STORE-004 | V:PROP | V:KANI, V:MODEL | proptest + kani + stateright | test + kani + model | 0 |
| INV-STORE-005 | V:PROP | V:KANI, V:MODEL | proptest + kani + stateright | test + kani + model | 0 |
| INV-STORE-006 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-STORE-007 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-STORE-008 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-STORE-009 | V:PROP | — | proptest | test | 0 |
| INV-STORE-010 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-STORE-011 | V:PROP | — | proptest | test | 0 |
| INV-STORE-012 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-STORE-013 | V:PROP | V:MODEL | proptest + stateright | test + model | 2 |
| INV-STORE-014 | V:PROP | — | proptest | test | 0 |

#### SCHEMA (8 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-SCHEMA-001 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-SCHEMA-002 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-SCHEMA-003 | V:TYPE | — | rustc | compile | 0 |
| INV-SCHEMA-004 | V:TYPE | V:KANI, V:PROP | rustc + kani + proptest | compile + kani + test | 0 |
| INV-SCHEMA-005 | V:PROP | — | proptest | test | 0 |
| INV-SCHEMA-006 | V:PROP | — | proptest | test | 0 |
| INV-SCHEMA-007 | V:PROP | — | proptest | test | 0 |
| INV-SCHEMA-008 | V:PROP | — | proptest | test | 0 |

#### QUERY (11 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-QUERY-001 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-QUERY-002 | V:PROP | — | proptest | test | 0 |
| INV-QUERY-003 | V:PROP | — | proptest | test | 1 |
| INV-QUERY-004 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-QUERY-005 | V:TYPE | V:PROP | rustc + proptest | compile + test | 0 |
| INV-QUERY-006 | V:TYPE | — | rustc | compile | 0 |
| INV-QUERY-007 | V:TYPE | V:PROP | rustc + proptest | compile + test | 0 |
| INV-QUERY-008 | V:PROP | — | proptest | test | 1 |
| INV-QUERY-009 | V:PROP | — | proptest | test | 1 |
| INV-QUERY-010 | V:PROP | V:MODEL | proptest + stateright | test + model | 2 |
| INV-QUERY-011 | V:PROP | — | proptest | test | 2 |

#### RESOLUTION (8 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-RESOLUTION-001 | V:TYPE | V:PROP | rustc + proptest | compile + test | 0 |
| INV-RESOLUTION-002 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-RESOLUTION-003 | V:PROP | V:MODEL | proptest + stateright | test + model | 3 |
| INV-RESOLUTION-004 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-RESOLUTION-005 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-RESOLUTION-006 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-RESOLUTION-007 | V:PROP | V:MODEL, V:KANI | proptest + stateright + kani | test + model + kani | 2 |
| INV-RESOLUTION-008 | V:PROP | V:MODEL | proptest + stateright | test + model | 0 |

#### HARVEST (8 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-HARVEST-001 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-HARVEST-002 | V:PROP | — | proptest | test | 0 |
| INV-HARVEST-003 | V:PROP | — | proptest | test | 0 |
| INV-HARVEST-004 | V:PROP | — | proptest | test | 0 |
| INV-HARVEST-005 | V:PROP | — | proptest | test | 0 |
| INV-HARVEST-006 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-HARVEST-007 | V:PROP | — | proptest | test | 0 |
| INV-HARVEST-008 | V:PROP | — | proptest | test | 0 |

#### SEED (6 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-SEED-001 | V:PROP | — | proptest | test | 0 |
| INV-SEED-002 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-SEED-003 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-SEED-004 | V:PROP | — | proptest | test | 0 |
| INV-SEED-005 | V:PROP | — | proptest | test | 1 |
| INV-SEED-006 | V:PROP | — | proptest | test | 2 |

#### MERGE (8 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-MERGE-001 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-MERGE-002 | V:PROP | V:MODEL | proptest + stateright | test + model | 2 |
| INV-MERGE-003 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-MERGE-004 | V:PROP | V:KANI, V:MODEL | proptest + kani + stateright | test + kani + model | 2 |
| INV-MERGE-005 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-MERGE-006 | V:PROP | — | proptest | test | 2 |
| INV-MERGE-007 | V:PROP | — | proptest | test | 2 |
| INV-MERGE-008 | V:PROP | — | proptest | test | 0 |

#### SYNC (5 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-SYNC-001 | V:PROP | V:MODEL | proptest + stateright | test + model | 3 |
| INV-SYNC-002 | V:PROP | — | proptest | test | 3 |
| INV-SYNC-003 | V:PROP | V:MODEL | proptest + stateright | test + model | 3 |
| INV-SYNC-004 | V:PROP | V:MODEL | proptest + stateright | test + model | 3 |
| INV-SYNC-005 | V:PROP | — | proptest | test | 3 |

#### SIGNAL (6 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-SIGNAL-001 | V:PROP | V:KANI | proptest + kani | test + kani | 3 |
| INV-SIGNAL-002 | V:PROP | — | proptest | test | 1 |
| INV-SIGNAL-003 | V:PROP | V:KANI | proptest + kani | test + kani | 3 |
| INV-SIGNAL-004 | V:PROP | — | proptest | test | 3 |
| INV-SIGNAL-005 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-SIGNAL-006 | V:PROP | — | proptest | test | 3 |

#### BILATERAL (5 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-BILATERAL-001 | V:PROP | V:MODEL | proptest + stateright | test + model | 1 |
| INV-BILATERAL-002 | V:PROP | — | proptest | test | 1 |
| INV-BILATERAL-003 | V:PROP | — | proptest | test | 2 |
| INV-BILATERAL-004 | V:PROP | — | proptest | test | 1 |
| INV-BILATERAL-005 | V:PROP | — | proptest | test | 1 |

#### DELIBERATION (6 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-DELIBERATION-001 | V:PROP | V:MODEL | proptest + stateright | test + model | 2 |
| INV-DELIBERATION-002 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-DELIBERATION-003 | V:PROP | — | proptest | test | 2 |
| INV-DELIBERATION-004 | V:PROP | — | proptest | test | 2 |
| INV-DELIBERATION-005 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-DELIBERATION-006 | V:PROP | V:MODEL | proptest + stateright | test + model | 2 |

#### GUIDANCE (7 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-GUIDANCE-001 | V:PROP | — | proptest | test | 0 |
| INV-GUIDANCE-002 | V:PROP | — | proptest | test | 0 |
| INV-GUIDANCE-003 | V:PROP | — | proptest | test | 1 |
| INV-GUIDANCE-004 | V:PROP | — | proptest | test | 1 |
| INV-GUIDANCE-005 | V:PROP | — | proptest | test | 4 |
| INV-GUIDANCE-006 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-GUIDANCE-007 | V:PROP | — | proptest | test | 0 |

#### BUDGET (5 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-BUDGET-001 | V:PROP | V:KANI | proptest + kani | test + kani | 1 |
| INV-BUDGET-002 | V:PROP | — | proptest | test | 1 |
| INV-BUDGET-003 | V:PROP | V:KANI | proptest + kani | test + kani | 1 |
| INV-BUDGET-004 | V:PROP | — | proptest | test | 1 |
| INV-BUDGET-005 | V:PROP | — | proptest | test | 1 |

#### INTERFACE (7 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-INTERFACE-001 | V:PROP | — | proptest | test | 0 |
| INV-INTERFACE-002 | V:PROP | — | proptest | test | 0 |
| INV-INTERFACE-003 | V:PROP | V:TYPE | proptest + rustc | test + compile | 0 |
| INV-INTERFACE-004 | V:PROP | — | proptest | test | 1 |
| INV-INTERFACE-005 | V:PROP | — | proptest | test | 4 |
| INV-INTERFACE-006 | V:PROP | — | proptest | test | 3 |
| INV-INTERFACE-007 | V:PROP | — | proptest | test | 1 |

### §16.2 CI Pipeline Gates

Every commit runs through a staged verification pipeline:

```
Gate 1: compile           — cargo check --all-targets
                            Checks: V:TYPE (all typestate patterns compile)
                            Time: <30s

Gate 2: test              — cargo test
                            Checks: V:PROP (all proptest properties hold)
                            Coverage: 104/104 INVs have proptest strategies
                            Time: <5m (proptest default: 256 cases per property)

Gate 3: kani              — cargo kani
                            Checks: V:KANI (bounded model checking)
                            Coverage: 42 INVs with critical-path verification
                            Time: <15m (bounded; unwind limit configurable)

Gate 4: model             — cargo test --features stateright
                            Checks: V:MODEL (protocol model checking)
                            Coverage: 15 INVs with protocol safety/liveness
                            Time: <30m (state space exploration)

Gate 5: miri (optional)   — cargo +nightly miri test
                            Checks: V:MIRI (undefined behavior detection)
                            Coverage: all unsafe code paths
                            Time: <10m
```

**Gate progression**: Gates 1–2 run on every commit. Gate 3 runs on PRs targeting main.
Gate 4 runs nightly or on protocol-affecting changes. Gate 5 runs on any `unsafe` code changes.

**Failure handling**: A gate failure blocks merge. The implementing agent must fix the
failing invariant before proceeding. Gate failures are recorded as datoms (CO-011).

### §16.3 Typestate Encoding Catalog

Protocols enforced at compile time via Rust's type system (zero runtime cost):

| Protocol | Types | Transitions | INV |
|----------|-------|-------------|-----|
| Transaction lifecycle | `Building → Committed → Applied` | `commit()`, `apply()` | INV-STORE-001 |
| EntityId construction | `EntityId(hash)` — no public constructor from arbitrary bytes | content-addressed only | INV-STORE-002 |
| Store immutability | `&Store` for reads, `&mut Store` only via `transact`/`merge` | borrow checker | INV-STORE-005 |
| Schema attribute | `Attribute` newtype — cannot confuse with raw strings | type-safe attribute refs | INV-SCHEMA-003 |
| Schema monotonicity | `SchemaEvolution(datoms)` — no `DROP` or `ALTER DELETE` | append-only by type | INV-SCHEMA-004 |
| Query mode | `QueryMode::Monotonic \| Stratified(Frontier) \| Barriered(BarrierId)` | parse-time enforcement | INV-QUERY-005 |
| FFI boundary | `FfiFunction` trait with `pure` marker — host-language functions can't mutate store | type-level purity | INV-QUERY-006 |
| Resolution mode | `ResolutionMode` enum — exhaustive match required | compile-time completeness | INV-RESOLUTION-001 |
| MCP tool set | `const MCP_TOOLS: [MCPTool; 9]` — fixed-size array | compile-time tool count | INV-INTERFACE-003 |

### §16.4 Deductive Verification Candidates

Invariants where deductive verification (Verus/Creusot) would provide mathematical proof
of correctness, justifying the higher cost:

| INV | Property | Justification |
|-----|----------|---------------|
| INV-STORE-004 | CRDT commutativity: `S₁ ∪ S₂ = S₂ ∪ S₁` | Foundational — all merge correctness depends on this. Proof by construction (set union) but a formal proof would close the loop. |
| INV-STORE-005 | CRDT associativity: `(S₁ ∪ S₂) ∪ S₃ = S₁ ∪ (S₂ ∪ S₃)` | Same justification as commutativity. |
| INV-STORE-006 | CRDT idempotency: `S ∪ S = S` | Completes the CRDT law triad. |
| INV-MERGE-001 | Merge preserves all datoms: `S ⊆ merge(S, S')` | Critical safety — no data loss during merge. |
| INV-RESOLUTION-005 | LWW commutativity | Per-attribute resolution correctness. |

**Recommendation**: Defer deductive verification to post-Stage 2. The cost is high
and the properties are well-served by proptest + Kani during initial implementation.
Pursue deductive proofs when the implementation stabilizes.

### §16.5 Verification Statistics

| Metric | Count | Coverage |
|--------|-------|----------|
| Total invariants | 104 | — |
| With V:PROP (minimum) | 104 | 100% |
| With V:KANI | 42 | 40.4% |
| With V:MODEL | 15 | 14.4% |
| With V:TYPE | 11 | 10.6% |
| Stage 0 invariants | 62 | 59.6% |
| Stage 1 invariants | 17 | 16.3% |
| Stage 2 invariants | 17 | 16.3% |
| Stage 3 invariants | 6 | 5.8% |
| Stage 4 invariants | 2 | 1.9% |

---

## §17. Cross-Reference Index

> **Purpose**: Maps every element to its source documents, tracks inter-invariant
> dependencies, and provides stage-based views for implementation planning.

### §17.1 Namespace → SEED.md → ADRS.md

| Namespace | SEED.md §§ | ADRS.md Categories | Primary Concerns |
|-----------|------------|---------------------|-----------------|
| STORE | §4, §9, §11 | FD-001–012, AS-001–010, SR-001–011, PD-001–004, PO-001, PO-012 | Append-only datom store, CRDT merge, content identity, HLC ordering, indexes |
| SCHEMA | §4 | SR-008–010, FD-005, FD-008 | 17 axiomatic attributes, genesis, schema-as-data, six-layer architecture |
| QUERY | §4 | FD-003, SQ-001–010, PO-013, AS-007 | Datalog, CALM, six strata, FFI boundary, significance tracking |
| RESOLUTION | §4 | FD-005, CR-001–006 | Per-attribute resolution, conflict predicate, three-tier routing |
| HARVEST | §5 | LM-005–006, LM-012–013, IB-012, CR-005 | Epistemic gap detection, pipeline, FP/FN calibration, proactive warnings |
| SEED | §5, §8 | IB-010, PO-002–003, PO-014, GU-003–004, SQ-007 | Associate→query→assemble, dynamic CLAUDE.md, rate-distortion, projection pyramid |
| MERGE | §6 | AS-001, AS-003–006, PD-001, PD-004, PO-006–007 | Set-union merge, branching G-Set, W_α, merge cascade, competing branch lock |
| SYNC | §6 | PO-010, SQ-001, SQ-004, PD-005 | Consistent cut, barrier protocol, topology independence |
| SIGNAL | §6 | PO-004–005, PO-008, CR-002–003, AS-009, CO-003 | Eight signal types, dispatch, subscription, diamond lattice signals |
| BILATERAL | §3, §6 | CO-004, CO-008–010, SQ-006, AS-006, CO-011 | Adjunction, fitness function, five-point coherence, bilateral symmetry |
| DELIBERATION | §6 | CR-004–005, CR-007, PO-007, AS-002, AA-001 | Three entity types, stability guard, precedent, commitment weight |
| GUIDANCE | §7, §8 | GU-001–008, IB-006 | Comonad, basin competition, six anti-drift mechanisms, spec-language |
| BUDGET | §8 | IB-004–007, IB-011, SQ-007 | k* measurement, Q(t), precedence, projection pyramid, rate-distortion |
| INTERFACE | §8 | IB-001–003, IB-008–009, IB-012, SR-011, AA-003 | Five layers, CLI modes, MCP tools, TUI, statusline, harvest warning |

### §17.2 Invariant Dependency Graph

Key inter-invariant dependencies (an edge A → B means B depends on A holding):

```
INV-STORE-001 (append-only) ──→ INV-MERGE-001 (merge preserves)
                              ──→ INV-HARVEST-001 (harvest commits to store)
                              ──→ INV-SCHEMA-004 (schema monotonicity)

INV-STORE-002 (content identity) ──→ INV-STORE-006 (idempotency)
                                  ──→ INV-MERGE-001 (deduplication)

INV-STORE-004/005/006 (CRDT laws) ──→ INV-MERGE-002 (merge cascade)
                                    ──→ INV-SYNC-001 (consistent cut)
                                    ──→ INV-BILATERAL-001 (convergence)

INV-STORE-008 (HLC monotonicity) ──→ INV-STORE-009 (frontier durability)
                                   ──→ INV-SYNC-003 (topology independence)

INV-SCHEMA-001 (genesis) ──→ INV-SCHEMA-002 (self-description)
                           ──→ INV-STORE-014 (every command is transaction)

INV-QUERY-001 (CALM) ──→ INV-SYNC-001 (barriers for non-monotonic)
                       ──→ INV-RESOLUTION-003 (conservative detection)

INV-RESOLUTION-004 (conflict predicate) ──→ INV-SIGNAL-004 (severity routing)
                                          ──→ INV-DELIBERATION-001 (deliberation entry)

INV-HARVEST-001 (epistemic gap) ──→ INV-SEED-001 (seed from store)
                                  ──→ INV-BILATERAL-001 (convergence)

INV-GUIDANCE-001 (continuous injection) ──→ INV-BUDGET-004 (compression by budget)
                                         ──→ INV-INTERFACE-007 (harvest warning)

INV-BILATERAL-001 (convergence) ──→ INV-DELIBERATION-001 (deliberation convergence)
                                  ──→ INV-GUIDANCE-004 (drift detection)
```

**Dependency depth** (longest chain from leaf to root):
- Depth 0: INV-STORE-001/002/008, INV-SCHEMA-001, INV-QUERY-001
- Depth 1: INV-STORE-004–006, INV-STORE-009, INV-MERGE-001, INV-SCHEMA-002
- Depth 2: INV-MERGE-002, INV-SYNC-001, INV-BILATERAL-001, INV-HARVEST-001
- Depth 3: INV-SEED-001, INV-DELIBERATION-001, INV-GUIDANCE-001
- Depth 4: INV-BUDGET-004, INV-INTERFACE-007

This confirms the implementation order: STORE → SCHEMA → QUERY → RESOLUTION → HARVEST
→ SEED → MERGE → SYNC → SIGNAL → BILATERAL → DELIBERATION → GUIDANCE → BUDGET → INTERFACE.

### §17.3 Stage Mapping

#### Stage 0 — Harvest/Seed Cycle (62 INV, core)

The foundational layer. Must be complete before any other stage.

**Namespaces fully included**: STORE (13/14 INV), SCHEMA (8/8), HARVEST (8/8), SEED (4/6)
**Namespaces partially included**: QUERY (5/11), RESOLUTION (5/8), MERGE (2/8), GUIDANCE (3/7), INTERFACE (3/7)
**Namespaces excluded**: SYNC, SIGNAL, BILATERAL, DELIBERATION, BUDGET

**Success criterion**: Work 25 turns, harvest, start fresh with seed — new session
picks up without manual re-explanation. First act: migrate SPEC.md elements as datoms.

#### Stage 1 — Budget-Aware Output + Guidance Injection (17 INV)

Builds on Stage 0 with attention budget management and enhanced guidance.

**New capabilities**: Q(t) measurement, output precedence, guidance compression,
harvest warnings, statusline bridge, significance tracking, frontier-scoped queries,
bilateral loop (basic), signal processing (confusion only).

**Key invariants**: INV-BUDGET-001–005, INV-GUIDANCE-003–004, INV-BILATERAL-001–002/004–005,
INV-INTERFACE-004/007, INV-QUERY-003/008–009, INV-SIGNAL-002.

#### Stage 2 — Branching + Deliberation (17 INV)

Adds isolated workspaces, competing proposals, and structured conflict resolution.

**New capabilities**: W_α working set, patch branches, branch comparison, deliberation
lifecycle, precedent queries, stability guard, lookahead via branch simulation,
bilateral symmetry, diamond lattice signal generation.

**Key invariants**: INV-STORE-013, INV-MERGE-002–007, INV-SEED-006,
INV-DELIBERATION-001–006, INV-SIGNAL-005, INV-GUIDANCE-006, INV-BILATERAL-003.

#### Stage 3 — Multi-Agent Coordination (6 INV)

Adds multi-agent primitives: sync barriers, signal routing, subscription system.

**New capabilities**: Full sync barrier protocol, eight signal types with three-tier
routing, subscription completeness, taxonomy coverage, human signal injection.

**Key invariants**: INV-SYNC-001–005, INV-SIGNAL-001/003–004/006,
INV-RESOLUTION-003, INV-INTERFACE-006.

#### Stage 4 — Advanced Intelligence (2 INV)

Adds learned guidance, spectral authority, significance-weighted retrieval, TUI.

**New capabilities**: Learned guidance effectiveness tracking, TUI subscription liveness.

**Key invariants**: INV-GUIDANCE-005, INV-INTERFACE-005.

### §17.4 Hard Constraint Traceability

Every hard constraint (C1–C7) traces to specific invariants:

| Constraint | Description | Enforcing Invariants |
|------------|-------------|---------------------|
| C1 | Append-only store | INV-STORE-001, INV-STORE-005, NEG-STORE-001 |
| C2 | Identity by content | INV-STORE-002, NEG-STORE-002 |
| C3 | Schema-as-data | INV-SCHEMA-003, INV-SCHEMA-004, INV-SCHEMA-008, NEG-SCHEMA-001 |
| C4 | CRDT merge by set union | INV-STORE-003, INV-STORE-004–007, INV-MERGE-001, NEG-MERGE-001 |
| C5 | Traceability | All elements have `Traces to` fields; INV-BILATERAL-002 (five-point coherence) |
| C6 | Falsifiability | All INVs have `Falsification` sections; structural property of the specification |
| C7 | Self-bootstrap | INV-SCHEMA-001 (genesis), INV-STORE-014 (every command is transaction), INV-BILATERAL-005 (test results as datoms) |

### §17.5 Failure Mode Traceability

Each failure mode (FAILURE_MODES.md) maps to the DDIS/Braid mechanisms that prevent it:

| FM | Class | Preventing Invariants | Preventing ADRs |
|----|-------|-----------------------|-----------------|
| FM-001 | Knowledge loss across sessions | INV-HARVEST-001–005, INV-SEED-001–004, INV-INTERFACE-007 | ADR-HARVEST-001, ADR-SEED-001 |
| FM-002 | Provenance fabrication | INV-STORE-014 (every command is tx), INV-SIGNAL-001 (signal as datom) | ADR-STORE-008 (provenance typing) |
| FM-003 | Anchoring bias in analysis scope | INV-SEED-001 (full store query), INV-BILATERAL-003 (bilateral symmetry) | ADR-SEED-002 (priority scoring) |
| FM-004 | Cascading incompleteness | INV-BILATERAL-001 (convergence), INV-DELIBERATION-002 (stability guard) | ADR-DELIBERATION-004 (crystallization guard) |

---

## Appendix A: Element Count Summary (Complete)

| Namespace | INV | ADR | NEG | Total | Wave |
|-----------|-----|-----|-----|-------|------|
| STORE     | 14  | 12  | 5   | 31    | 1    |
| SCHEMA    | 8   | 4   | 3   | 15    | 1    |
| QUERY     | 11  | 8   | 4   | 23    | 1    |
| RESOLUTION| 8   | 5   | 3   | 16    | 1    |
| HARVEST   | 8   | 4   | 3   | 15    | 2    |
| SEED      | 6   | 3   | 2   | 11    | 2    |
| MERGE     | 8   | 4   | 3   | 15    | 2    |
| SYNC      | 5   | 3   | 2   | 10    | 2    |
| SIGNAL    | 6   | 3   | 3   | 12    | 3    |
| BILATERAL | 5   | 3   | 2   | 10    | 3    |
| DELIBERATION | 6 | 4  | 3   | 13    | 3    |
| GUIDANCE  | 7   | 4   | 3   | 14    | 3    |
| BUDGET    | 5   | 3   | 2   | 10    | 3    |
| INTERFACE | 7   | 3   | 3   | 13    | 3    |
| **Total** | **104** | **63** | **41** | **208** |      |

**Additional Wave 4 content**: 10 uncertainty entries (§15), 104-row verification matrix (§16),
14-namespace cross-reference index with dependency graph and stage mapping (§17).

## Appendix B: Verification Statistics (Final)

| Metric | Count | Coverage |
|--------|-------|----------|
| Total INVs | 104 | — |
| V:PROP (minimum) | 104/104 | 100.0% |
| V:KANI (critical) | 42/104 | 40.4% |
| V:MODEL (protocol) | 15/104 | 14.4% |
| V:TYPE (compile-time) | 11/104 | 10.6% |
| V:DEDUCTIVE (candidate) | 5 | Deferred to post-Stage 2 |
| Stage 0 INVs | 62 | 59.6% |
| Stage 1 INVs | 17 | 16.3% |
| Stage 2 INVs | 17 | 16.3% |
| Stage 3 INVs | 6 | 5.8% |
| Stage 4 INVs | 2 | 1.9% |
| Uncertainty markers | 10 | — |
| High-urgency uncertainties | 3 | Resolve during Stage 0 |

## Appendix C: Stage 0 Elements

Elements required for Stage 0 (Harvest/Seed cycle):

| Element | Namespace | Summary |
|---------|-----------|---------|
| INV-STORE-001–012, 014 | STORE | Core store operations |
| INV-SCHEMA-001–008 | SCHEMA | Schema bootstrap (all 8) |
| INV-QUERY-001–002, 005–007 | QUERY | Core query engine |
| INV-RESOLUTION-001–002, 004–006, 008 | RESOLUTION | Basic conflict handling |
| INV-HARVEST-001–008 | HARVEST | Full harvest pipeline |
| INV-SEED-001–004 | SEED | Seed assembly pipeline |
| INV-MERGE-001, 008 | MERGE | Core merge (no branching) |
| INV-GUIDANCE-001–002, 007 | GUIDANCE | Injection, spec-language, dynamic CLAUDE.md |
| INV-INTERFACE-001–003 | INTERFACE | CLI modes, MCP wrapper, nine tools |
| ADR-STORE-001–012 | STORE | Foundation decisions |
| ADR-SCHEMA-001–004 | SCHEMA | Schema decisions |
| ADR-QUERY-001–003, 005–006 | QUERY | Query engine decisions |
| ADR-RESOLUTION-001–004 | RESOLUTION | Resolution decisions |
| ADR-HARVEST-001–004 | HARVEST | Harvest decisions |
| ADR-SEED-001–003 | SEED | Seed decisions |
| ADR-MERGE-001 | MERGE | Core merge decision |
| ADR-GUIDANCE-002, 004 | GUIDANCE | Basin competition, spec-language |
| ADR-INTERFACE-001–003 | INTERFACE | Layers, agent-mode, trajectory |
| NEG-STORE-001–005 | STORE | Store safety |
| NEG-SCHEMA-001–003 | SCHEMA | Schema safety |
| NEG-QUERY-001–004 | QUERY | Query safety |
| NEG-RESOLUTION-001–003 | RESOLUTION | Resolution safety |
| NEG-HARVEST-001–003 | HARVEST | Harvest safety |
| NEG-SEED-001–002 | SEED | Seed safety |
| NEG-MERGE-001, 003 | MERGE | Merge safety (no data loss, no W_α leak) |
| NEG-GUIDANCE-001 | GUIDANCE | No tool response without footer |
| NEG-INTERFACE-003 | INTERFACE | No harvest warning suppression |
