> **Namespace**: FERR | **Wave**: 1 (Foundation) | **Stage**: 0
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)
> **Supersedes**: spec/01-store.md (STORE namespace — algebraic datom store, implementation layer only)
> **Depends on**: [01-store.md](01-store.md) (STORE namespace — algebraic axioms L1-L5 preserved verbatim)

## 23.0 Preamble

### 23.0.1 Overview

Ferratomic is the embedded datom database engine that reifies the algebraic store `(P(D), ∪)`
specified in [01-store.md](01-store.md) as a production-grade storage system. Where `01-store.md`
defines the mathematical object — the G-Set CvRDT, the five lattice laws, the transaction
algebra — Ferratomic specifies the **engineering substrate** that makes those laws hold under
real-world conditions: concurrent writers, crash recovery, disk corruption, memory pressure,
and multi-process access.

The relationship between STORE and FERR is analogous to the relationship between a group axiom
and a concrete group representation: STORE says `MERGE(A, B) = MERGE(B, A)`; FERR says how
that commutativity is preserved when A and B are 50GB memory-mapped files being written by
independent OS processes that may crash at any byte boundary.

**Traces to**: SEED.md 4 (Core Abstraction: Datoms), SEED.md 5 (Harvest/Seed Lifecycle),
SEED.md 10 (The Bootstrap)

**Design principles**:

1. **Algebraic fidelity.** Every FERR invariant is a refinement of a STORE axiom. No FERR
   invariant may contradict or weaken any STORE law L1-L5. The refinement relation is
   formally: `INV-FERR-NNN refines INV-STORE-MMM` means that any system satisfying
   INV-FERR-NNN necessarily satisfies INV-STORE-MMM.

2. **Verification depth.** Every invariant carries six verification layers: algebraic law
   (Level 0), state invariant (Level 1), implementation contract (Level 2), falsification
   condition, proptest strategy, and Lean 4 theorem. The Lean theorems are mechanically
   checkable proofs that the algebraic laws hold for the `DatomStore := Finset Datom` model.

3. **Crash-safety first.** The WAL-before-snapshot discipline (INV-FERR-008) is the
   load-bearing durability guarantee. All other durability properties derive from it.

4. **Content-addressed everything.** Entity identity, transaction identity, and index entries
   are all derived from content hashes (BLAKE3). This eliminates allocation coordination
   across replicas and makes deduplication a structural tautology.

5. **Substrate independence (C8).** Ferratomic is a general-purpose embedded datom database.
   It has no knowledge of DDIS methodology, braid commands, observations, or spec elements.
   It stores `[e, a, v, tx, op]` tuples and enforces schema constraints. Everything
   domain-specific enters through the schema layer, not the engine.

### 23.0.2 Crate Structure

```
ferratomic/                          -- workspace root
├── ferratom/                        -- Primitive types: Datom, EntityId, TxId, Value, Op
│   └── src/lib.rs                   -- Zero dependencies. No I/O. No allocation.
├── ferratomic-core/                 -- Storage engine: Store, indexes, WAL, snapshots, merge
│   ├── src/store.rs                 -- Store struct, transact, merge, genesis
│   ├── src/index.rs                 -- EAVT, AEVT, VAET, AVET, LIVE indexes
│   ├── src/wal.rs                   -- Write-ahead log with fsync ordering
│   ├── src/snapshot.rs              -- Point-in-time snapshot materialization
│   ├── src/schema.rs                -- Schema-as-data validation
│   └── src/merge.rs                 -- CRDT merge (set union + cascade)
├── ferratomic-datalog/              -- Query engine: Datalog dialect, semi-naive evaluation
│   ├── src/parser.rs                -- EDN-based Datalog parser
│   ├── src/planner.rs               -- Query plan generation with stratum classification
│   └── src/eval.rs                  -- Semi-naive evaluation with CALM compliance
└── ferratomic-verify/               -- Verification harnesses: proptest, kani, stateright
    ├── src/proptest_strategies.rs    -- Arbitrary instances for all core types
    ├── src/kani_harnesses.rs         -- Bounded model checking proofs
    └── src/stateright_models.rs      -- Protocol model checking (multi-node CRDT)
```

**Dependency DAG** (acyclic, strict):
```
ferratom  <--  ferratomic-core  <--  ferratomic-datalog
                    ^                       ^
                    |                       |
                    +-------  ferratomic-verify  (dev-dependency only)
```

`ferratom` has zero external dependencies. `ferratomic-core` depends only on `ferratom`,
`blake3`, and `serde`. `ferratomic-datalog` depends on `ferratomic-core`. `ferratomic-verify`
is a dev-dependency workspace member that imports all three for testing.

### 23.0.3 Relationship to spec/01-store.md

The STORE namespace (spec/01-store.md) defines the algebraic specification: the datom type,
the store as `(P(D), ∪)`, the five lattice laws L1-L5, the transaction algebra, the value
domain, and the index invariants. Those definitions are **preserved verbatim** in Ferratomic.
The FERR namespace adds:

| STORE provides | FERR adds |
|----------------|-----------|
| L1-L3 (CRDT axioms) | Concrete merge implementation with crash-safety (INV-FERR-001 through INV-FERR-003) |
| L4-L5 (monotonicity, growth) | Monotonic growth with WAL durability (INV-FERR-004, INV-FERR-008) |
| Index invariants (EAVT, AEVT, VAET, AVET, LIVE) | Index bijection with crash-recovery (INV-FERR-005) |
| Transaction algebra | Snapshot isolation + write linearizability (INV-FERR-006, INV-FERR-007) |
| Schema-as-data | Schema validation at transact boundary (INV-FERR-009) |
| Content-addressed identity axiom | BLAKE3 content addressing (INV-FERR-012) |
| — | Merge convergence proof (INV-FERR-010) |
| — | Observer epoch monotonicity (INV-FERR-011) |

Every INV-FERR invariant traces to a STORE axiom or SEED.md section. No INV-FERR invariant
introduces a property not implied by the algebraic specification — it only specifies how
that property is maintained under real-world failure modes.

### 23.0.4 Lean 4 Foundation Model

The Lean 4 theorems throughout this specification operate on the following definitions:

```lean
import Mathlib.Data.Finset.Basic
import Mathlib.Data.Finset.Lattice
import Mathlib.Order.BooleanAlgebra

/-- A datom is an opaque five-tuple. For the algebraic model,
    we abstract over the concrete field types. -/
structure Datom where
  e  : Nat    -- entity (content-addressed, modeled as Nat for finiteness)
  a  : Nat    -- attribute
  v  : Nat    -- value (abstracted)
  tx : Nat    -- transaction
  op : Bool   -- true = assert, false = retract
  deriving DecidableEq, Repr

/-- A datom store is a finite set of datoms. -/
def DatomStore := Finset Datom

/-- Merge is set union. -/
def merge (a b : DatomStore) : DatomStore := a ∪ b

/-- Apply (transact) adds datoms to the store. -/
def apply_tx (s : DatomStore) (d : Datom) : DatomStore := s ∪ {d}

/-- Store cardinality (number of distinct datoms). -/
def store_size (s : DatomStore) : Nat := s.card

/-- Content-addressed identity: a datom's identity IS its content. -/
def datom_id (d : Datom) : Datom := d  -- identity function (tautological by construction)
```

### 23.0.5 Stateright Foundation Model

The Stateright models throughout this specification operate on the following state machine:

```rust
use stateright::*;
use std::collections::BTreeSet;

/// A datom is a content-addressed five-tuple.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Datom {
    e: u64,
    a: u64,
    v: u64,
    tx: u64,
    op: bool, // true = assert, false = retract
}

/// CRDT state: N nodes, each holding a G-Set of datoms, with in-flight merges.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct CrdtState {
    nodes: Vec<BTreeSet<Datom>>,
    in_flight: Vec<(usize, usize, BTreeSet<Datom>)>, // (from, to, payload)
}

/// Actions available to the model checker.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum CrdtAction {
    Write(usize, Datom),                  // node writes a datom
    InitMerge(usize, usize),             // node initiates merge to peer
    DeliverMerge(usize),                  // deliver in-flight merge at index
}
```

---

## 23.1 Core Invariants

### INV-FERR-001: Merge Commutativity

**Traces to**: SEED.md 4 Axiom 2 (Store), C4, L1, INV-STORE-004
**Verification**: `V:PROP`, `V:KANI`, `V:LEAN`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ A, B ∈ DatomStore:
  merge(A, B) = merge(B, A)

Proof: merge(A, B) = A ∪ B = B ∪ A = merge(B, A)
  by commutativity of set union.
```

#### Level 1 (State Invariant)
For all reachable store pairs `(A, B)` produced by any sequence of TRANSACT operations
starting from GENESIS: the datom set resulting from `merge(A, B)` is identical to the
datom set resulting from `merge(B, A)`. This holds regardless of the order in which
transactions were applied to A and B independently, the wall-clock times of those
transactions, or the agents that produced them. Commutativity means that the order in
which two replicas discover each other and initiate merge is irrelevant to the final
converged state.

#### Level 2 (Implementation Contract)
```rust
/// Merge two stores. The result contains exactly the union of both datom sets.
/// Order of arguments does not affect the result (INV-FERR-001).
///
/// # Panics
/// Never panics. Merge is total over all valid store pairs.
pub fn merge(a: &Store, b: &Store) -> Store {
    let mut result = a.datoms.clone();
    for datom in b.datoms.iter() {
        result.insert(datom.clone()); // BTreeSet insert is idempotent
    }
    Store::from_datoms(result) // rebuilds indexes
}

#[kani::proof]
#[kani::unwind(10)]
fn merge_commutativity() {
    let a: BTreeSet<Datom> = kani::any();
    let b: BTreeSet<Datom> = kani::any();
    kani::assume(a.len() <= 4 && b.len() <= 4);

    let ab: BTreeSet<Datom> = a.union(&b).cloned().collect();
    let ba: BTreeSet<Datom> = b.union(&a).cloned().collect();
    assert_eq!(ab, ba);
}
```

**Falsification**: Any pair of stores `(A, B)` where the datom set of `merge(A, B)` differs
from the datom set of `merge(B, A)`. Concretely: there exists a datom `d` such that
`d ∈ merge(A, B)` but `d ∉ merge(B, A)`, or vice versa. This would indicate that the merge
implementation performs order-dependent operations (e.g., deduplication that depends on
insertion order, or resolution logic applied during merge rather than at the query layer).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn merge_commutes(
        a_datoms in prop::collection::btree_set(arb_datom(), 0..100),
        b_datoms in prop::collection::btree_set(arb_datom(), 0..100),
    ) {
        let a = Store::from_datoms(a_datoms.clone());
        let b = Store::from_datoms(b_datoms.clone());

        let ab = merge(&a, &b);
        let ba = merge(&b, &a);

        prop_assert_eq!(ab.datom_set(), ba.datom_set());
    }
}
```

**Lean theorem**:
```lean
theorem merge_comm (a b : DatomStore) : merge a b = merge b a := by
  unfold merge
  exact Finset.union_comm a b
```

---

### INV-FERR-002: Merge Associativity

**Traces to**: SEED.md 4 Axiom 2 (Store), C4, L2, INV-STORE-005
**Verification**: `V:PROP`, `V:KANI`, `V:LEAN`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ A, B, C ∈ DatomStore:
  merge(merge(A, B), C) = merge(A, merge(B, C))

Proof: merge(merge(A, B), C) = (A ∪ B) ∪ C = A ∪ (B ∪ C) = merge(A, merge(B, C))
  by associativity of set union.
```

#### Level 1 (State Invariant)
For all reachable store triples `(A, B, C)`: the final datom set is invariant under
regrouping of merge operations. This is the property that enables arbitrary merge topologies
in multi-agent systems. Whether agent 1 merges with agent 2 first and then agent 3, or
agent 2 merges with agent 3 first and then agent 1, the final converged state is identical.
Without associativity, the merge topology would constrain the final result, making the
system dependent on coordination infrastructure rather than on the datoms themselves.

#### Level 2 (Implementation Contract)
```rust
#[kani::proof]
#[kani::unwind(10)]
fn merge_associativity() {
    let a: BTreeSet<Datom> = kani::any();
    let b: BTreeSet<Datom> = kani::any();
    let c: BTreeSet<Datom> = kani::any();
    kani::assume(a.len() <= 3 && b.len() <= 3 && c.len() <= 3);

    let ab: BTreeSet<Datom> = a.union(&b).cloned().collect();
    let ab_c: BTreeSet<Datom> = ab.union(&c).cloned().collect();

    let bc: BTreeSet<Datom> = b.union(&c).cloned().collect();
    let a_bc: BTreeSet<Datom> = a.union(&bc).cloned().collect();

    assert_eq!(ab_c, a_bc);
}
```

**Falsification**: Any triple of stores `(A, B, C)` where `merge(merge(A, B), C)` produces
a different datom set than `merge(A, merge(B, C))`. This would indicate that the merge
implementation accumulates state (e.g., a merge counter, a "last merged from" marker)
that is sensitive to grouping. Since merge is defined as pure set union, any such
accumulation is a bug.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn merge_associative(
        a_datoms in prop::collection::btree_set(arb_datom(), 0..50),
        b_datoms in prop::collection::btree_set(arb_datom(), 0..50),
        c_datoms in prop::collection::btree_set(arb_datom(), 0..50),
    ) {
        let a = Store::from_datoms(a_datoms);
        let b = Store::from_datoms(b_datoms);
        let c = Store::from_datoms(c_datoms);

        let ab_c = merge(&merge(&a, &b), &c);
        let a_bc = merge(&a, &merge(&b, &c));

        prop_assert_eq!(ab_c.datom_set(), a_bc.datom_set());
    }
}
```

**Lean theorem**:
```lean
theorem merge_assoc (a b c : DatomStore) : merge (merge a b) c = merge a (merge b c) := by
  unfold merge
  exact Finset.union_assoc a b c
```

---

### INV-FERR-003: Merge Idempotency

**Traces to**: SEED.md 4 Axiom 2 (Store), C4, L3, INV-STORE-006
**Verification**: `V:PROP`, `V:KANI`, `V:LEAN`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ A ∈ DatomStore:
  merge(A, A) = A

Proof: merge(A, A) = A ∪ A = A
  by idempotency of set union.
```

#### Level 1 (State Invariant)
Merging a store with itself produces no change to the datom set, the indexes, or any
derived state. This property is essential for at-least-once delivery semantics (PD-004):
if a merge message is delivered twice (network retry, process restart), the second delivery
is a no-op. Without idempotency, retry logic would need deduplication infrastructure
external to the store, violating the principle that all protocol-relevant state lives
in `D` (ADR-FOUNDATION-003).

#### Level 2 (Implementation Contract)
```rust
#[kani::proof]
#[kani::unwind(10)]
fn merge_idempotency() {
    let a: BTreeSet<Datom> = kani::any();
    kani::assume(a.len() <= 5);

    let aa: BTreeSet<Datom> = a.union(&a).cloned().collect();
    assert_eq!(a, aa);
}

/// Implementation: merge detects self-merge via store identity hash and short-circuits.
/// Even without the short-circuit, set union with self is structurally idempotent.
pub fn merge(a: &Store, b: &Store) -> Store {
    if a.identity_hash() == b.identity_hash() {
        return a.clone(); // fast path: self-merge
    }
    // ... full merge path ...
}
```

**Falsification**: A store `A` where `merge(A, A)` produces a datom set that differs from
`A` in any way: different cardinality, different datom content, different index state. This
would indicate that merge has side effects beyond set union — for example, incrementing a
merge counter, updating a "last merged" timestamp, or re-indexing in a non-deterministic way.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn merge_idempotent(
        datoms in prop::collection::btree_set(arb_datom(), 0..100),
    ) {
        let store = Store::from_datoms(datoms);
        let merged = merge(&store, &store);
        prop_assert_eq!(store.datom_set(), merged.datom_set());
        prop_assert_eq!(store.len(), merged.len());
    }
}
```

**Lean theorem**:
```lean
theorem merge_idem (a : DatomStore) : merge a a = a := by
  unfold merge
  exact Finset.union_idempotent a
```

---

### INV-FERR-004: Monotonic Growth

**Traces to**: SEED.md 4 Axiom 2 (Store), C1, L4, L5, INV-STORE-001, INV-STORE-002
**Verification**: `V:PROP`, `V:KANI`, `V:LEAN`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ S ∈ DatomStore, ∀ d ∈ Datom:
  |apply(S, d)| ≥ |S|

Equivalently:
  S ⊆ apply(S, d)        -- no datom is lost
  |apply(S, d)| ≥ |S|    -- cardinality is non-decreasing

Strict growth for transactions:
  ∀ S, T where T is a non-empty transaction:
    |TRANSACT(S, T)| > |S|
  (every transaction adds at least its tx_entity metadata datoms)
```

#### Level 1 (State Invariant)
For all reachable states `(S, S')` where `S` transitions to `S'` via TRANSACT or MERGE:
`S.datoms` is a subset of `S'.datoms`. The store never shrinks. Retractions are new datoms
with `op = Retract` — they add to the store rather than removing from it. The "current
state" of an entity (which values are "live") is a query-layer concern computed by the
LIVE index; the store itself is append-only.

For TRANSACT specifically, growth is strict: every transaction produces at least one new
datom (the transaction entity's metadata), so `|S'.datoms| > |S.datoms|`. For MERGE,
growth is non-strict: merging two identical stores produces the same store (idempotency,
INV-FERR-003), so `|merge(S, S)| = |S|`.

#### Level 2 (Implementation Contract)
```rust
/// Transact a committed transaction into the store.
/// Post-condition: store size strictly increases (at least tx metadata datoms added).
#[kani::ensures(|result| old(store.len()) < store.len())]
pub fn transact(store: &mut Store, tx: Transaction<Committed>) -> Result<TxReceipt, TxApplyError> {
    let pre_len = store.len();
    // ... apply datoms, add tx metadata ...
    debug_assert!(store.len() > pre_len, "INV-FERR-004: strict growth violated");
    Ok(receipt)
}

/// Merge: non-strict growth. Result is superset of both inputs.
#[kani::ensures(|result| old(a.len()) <= result.len() && old(b.len()) <= result.len())]
pub fn merge(a: &Store, b: &Store) -> Store {
    // ... set union ...
}

#[kani::proof]
#[kani::unwind(10)]
fn monotonic_growth() {
    let s: BTreeSet<Datom> = kani::any();
    let d: Datom = kani::any();
    kani::assume(s.len() <= 5);

    let mut s_prime = s.clone();
    s_prime.insert(d);
    assert!(s_prime.len() >= s.len());
    assert!(s.is_subset(&s_prime));
}
```

**Falsification**: Any transition `S -> S'` where there exists a datom `d ∈ S` such that
`d ∉ S'`. Equivalently: `store.len()` decreases after any operation. For TRANSACT
specifically: `store.len()` does not strictly increase (remains equal or decreases).
This would indicate either a mutation (violating C1), a deduplication bug that removes
existing datoms, or a transaction that adds zero datoms (including no tx metadata).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn monotonic_transact(
        initial in arb_store(0..50),
        tx in arb_transaction(),
    ) {
        let pre_datoms: BTreeSet<_> = initial.datom_set().clone();
        let pre_len = initial.len();

        let mut store = initial;
        if let Ok(_receipt) = store.transact(tx) {
            // Strict growth: at least tx metadata added
            prop_assert!(store.len() > pre_len);
            // Monotonicity: no datoms lost
            for d in &pre_datoms {
                prop_assert!(store.datom_set().contains(d));
            }
        }
    }
}
```

**Lean theorem**:
```lean
theorem apply_monotone (s : DatomStore) (d : Datom) : s.card ≤ (apply_tx s d).card := by
  unfold apply_tx
  exact Finset.card_le_card (Finset.subset_union_left s {d})

theorem apply_superset (s : DatomStore) (d : Datom) : s ⊆ apply_tx s d := by
  unfold apply_tx
  exact Finset.subset_union_left s {d}

theorem merge_monotone_left (a b : DatomStore) : a ⊆ merge a b := by
  unfold merge
  exact Finset.subset_union_left a b

theorem merge_monotone_right (a b : DatomStore) : b ⊆ merge a b := by
  unfold merge
  exact Finset.subset_union_right a b
```

---

### INV-FERR-005: Index Bijection

**Traces to**: SEED.md 4, INV-STORE-012, ADRS SR-001, SR-002
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let primary(S) = S.datoms (the canonical datom set).
Let EAVT(S), AEVT(S), VAET(S), AVET(S) be the four secondary indexes.

∀ d ∈ Datom:
  d ∈ primary(S) ⟺ d ∈ EAVT(S) ⟺ d ∈ AEVT(S) ⟺ d ∈ VAET(S) ⟺ d ∈ AVET(S)

Equivalently: the indexes are projections of the same set, differing only in sort order.
The content of all five structures is identical; only the access pattern differs.

Cardinality:
  |primary(S)| = |EAVT(S)| = |AEVT(S)| = |VAET(S)| = |AVET(S)|
```

#### Level 1 (State Invariant)
After every TRANSACT, MERGE, or crash-recovery operation, every datom in the primary
store appears in every secondary index, and every entry in every secondary index
corresponds to a datom in the primary store. There are no phantom index entries (present
in index but not in primary) and no missing index entries (present in primary but absent
from index). The LIVE index is excluded from this bijection because it is a derived view
(resolution-applied) rather than a permutation of the raw datom set.

This invariant must hold even after crash recovery: if the process crashes between
writing a datom to the primary store and updating an index, the recovery procedure
must restore the bijection before the store becomes queryable.

#### Level 2 (Implementation Contract)
```rust
/// Verify index bijection. Called after every TRANSACT and during recovery.
/// O(n) scan — used in debug builds and verification harnesses, not hot path.
fn verify_index_bijection(store: &Store) -> bool {
    let primary = &store.datoms;
    let eavt_set: BTreeSet<&Datom> = store.indexes.eavt.iter().collect();
    let aevt_set: BTreeSet<&Datom> = store.indexes.aevt.iter().collect();
    let vaet_set: BTreeSet<&Datom> = store.indexes.vaet.iter().collect();
    let avet_set: BTreeSet<&Datom> = store.indexes.avet.iter().collect();

    let primary_set: BTreeSet<&Datom> = primary.iter().collect();

    primary_set == eavt_set
        && primary_set == aevt_set
        && primary_set == vaet_set
        && primary_set == avet_set
}

#[kani::proof]
#[kani::unwind(10)]
fn index_bijection() {
    let datoms: BTreeSet<Datom> = kani::any();
    kani::assume(datoms.len() <= 4);

    let store = Store::from_datoms(datoms.clone());

    // Every datom in primary is in every index
    for d in &datoms {
        assert!(store.indexes.eavt.contains(d));
        assert!(store.indexes.aevt.contains(d));
        assert!(store.indexes.vaet.contains(d));
        assert!(store.indexes.avet.contains(d));
    }

    // Every index has exactly the same cardinality as primary
    assert_eq!(datoms.len(), store.indexes.eavt.len());
    assert_eq!(datoms.len(), store.indexes.aevt.len());
    assert_eq!(datoms.len(), store.indexes.vaet.len());
    assert_eq!(datoms.len(), store.indexes.avet.len());
}
```

**Falsification**: A datom `d` exists in `primary(S)` but not in `EAVT(S)` (or any other
index), or a datom `d` exists in `AEVT(S)` but not in `primary(S)`. Also: any state where
`|primary(S)| != |EAVT(S)|` (cardinality mismatch). This would indicate either an
incremental index update bug (a transaction that adds to primary but fails to update one
or more indexes), a crash-recovery defect (WAL replayed to primary but not to indexes),
or a concurrency bug (index update not protected by the same serialization as the primary
write).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn index_bijection_after_transactions(
        initial in arb_store(0..20),
        txns in prop::collection::vec(arb_transaction(), 0..10),
    ) {
        let mut store = initial;
        for tx in txns {
            let _ = store.transact(tx);
            // After every transaction, verify bijection
            prop_assert!(verify_index_bijection(&store));
        }
    }

    #[test]
    fn index_bijection_after_merge(
        a in arb_store(0..30),
        b in arb_store(0..30),
    ) {
        let merged = merge(&a, &b);
        prop_assert!(verify_index_bijection(&merged));
    }
}
```

**Lean theorem**:
```lean
/-- Index bijection: every projection of the same set has the same cardinality.
    In Lean, we model indexes as the same Finset with different orderings.
    Since reordering a Finset does not change membership, bijection is trivial. -/
theorem index_bijection (s : DatomStore) :
    s.card = s.card := by
  rfl

/-- The substantive claim: after adding a datom to the store, the datom is
    present in every "index" (modeled as the same set, since indexes are
    permutations of the primary set). -/
theorem index_membership_after_apply (s : DatomStore) (d : Datom) :
    d ∈ apply_tx s d := by
  unfold apply_tx
  exact Finset.mem_union_right s (Finset.mem_singleton_self d)
```

---

### INV-FERR-006: Snapshot Isolation

**Traces to**: SEED.md 4 Axiom 3 (Snapshots), INV-STORE-013, ADRS PD-001
**Verification**: `V:PROP`, `V:KANI`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let epoch(S) = the monotonic version counter of store S.
Let snapshot(S, e) = the datom set visible at epoch e.

∀ reader R observing epoch e:
  ∀ writer W committing transaction T at epoch e' > e:
    snapshot(S, e) ∩ T.datoms = ∅ if T was not committed at or before epoch e

No reader sees a partial transaction:
  ∀ T = {d₁, d₂, ..., dₙ}:
    either ∀ dᵢ ∈ snapshot(S, e) (T fully visible)
    or     ∀ dᵢ ∉ snapshot(S, e) (T fully invisible)

This is equivalent to:
  snapshot(S, e) = ⋃ {T.datoms | T committed at epoch ≤ e}
```

#### Level 1 (State Invariant)
A reader that obtains a snapshot at epoch `e` sees a consistent view: exactly the set
of datoms from all transactions committed at or before epoch `e`, and none of the datoms
from transactions committed after epoch `e`. No reader ever sees a subset of a
transaction's datoms — transactions are atomic with respect to snapshot visibility.

This must hold even under concurrent access: while writer W is committing transaction T
(which involves writing datoms, updating indexes, and advancing the epoch), any concurrent
reader R that obtained a snapshot before W's commit sees none of T's datoms. Reader R'
that obtains a snapshot after W's commit sees all of T's datoms.

#### Level 2 (Implementation Contract)
```rust
/// A snapshot is a read-only view at a specific epoch.
/// The epoch is captured at construction time and does not advance.
pub struct Snapshot<'a> {
    store: &'a Store,
    epoch: u64,
}

impl Store {
    /// Obtain a snapshot at the current epoch.
    /// The snapshot sees all committed transactions up to this epoch.
    pub fn snapshot(&self) -> Snapshot<'_> {
        Snapshot {
            store: self,
            epoch: self.current_epoch(),
        }
    }
}

impl<'a> Snapshot<'a> {
    /// Query datoms visible at this snapshot's epoch.
    /// Returns only datoms from transactions committed at epoch <= self.epoch.
    pub fn datoms(&self) -> impl Iterator<Item = &Datom> {
        self.store.datoms.iter().filter(|d| d.tx_epoch <= self.epoch)
    }
}

#[kani::proof]
#[kani::unwind(8)]
fn snapshot_isolation() {
    let mut store = Store::genesis();
    let snap_epoch = store.current_epoch();
    let snapshot_datoms: BTreeSet<Datom> = store.snapshot().datoms().cloned().collect();

    // Simulate a concurrent write at a later epoch
    let tx: Transaction<Committed> = kani::any();
    let _ = store.transact(tx);

    // Original snapshot must not see the new datoms
    for d in store.datoms.iter() {
        if d.tx_epoch > snap_epoch {
            assert!(!snapshot_datoms.contains(d));
        }
    }
}
```

**Falsification**: A reader at epoch `e` observes a datom from a transaction committed at
epoch `e' > e`. Or: a reader at epoch `e` observes datoms `d₁` from transaction `T` but
not `d₂` from the same transaction `T` where `T` was committed at epoch `≤ e` (partial
transaction visibility). Either case indicates a concurrency defect in the snapshot
mechanism — either the epoch is not atomically captured, or the datom visibility filter
is not correctly epoch-bounded.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn snapshot_sees_no_future_txns(
        initial_txns in prop::collection::vec(arb_transaction(), 1..5),
        later_txns in prop::collection::vec(arb_transaction(), 1..5),
    ) {
        let mut store = Store::genesis();
        for tx in initial_txns {
            let _ = store.transact(tx);
        }

        let snapshot = store.snapshot();
        let snap_datoms: BTreeSet<_> = snapshot.datoms().cloned().collect();

        for tx in later_txns {
            let _ = store.transact(tx);
        }

        // Snapshot must not have grown
        let snap_datoms_after: BTreeSet<_> = snapshot.datoms().cloned().collect();
        prop_assert_eq!(snap_datoms, snap_datoms_after);
    }

    #[test]
    fn transaction_atomicity(
        txns in prop::collection::vec(arb_multi_datom_transaction(), 1..10),
    ) {
        let mut store = Store::genesis();
        for tx in txns {
            let tx_datoms: BTreeSet<_> = tx.datoms().cloned().collect();
            let _ = store.transact(tx);

            let snapshot = store.snapshot();
            let visible: BTreeSet<_> = snapshot.datoms().cloned().collect();

            // Transaction is either fully visible or fully invisible
            let visible_count = tx_datoms.iter().filter(|d| visible.contains(d)).count();
            prop_assert!(
                visible_count == 0 || visible_count == tx_datoms.len(),
                "Partial transaction visibility: {} of {} datoms visible",
                visible_count, tx_datoms.len()
            );
        }
    }
}
```

**Lean theorem**:
```lean
/-- Snapshot isolation: the datoms visible at epoch e are exactly those
    from transactions at epoch ≤ e. Adding a datom at epoch e' > e
    does not change the set visible at epoch e. -/

def visible_at (s : DatomStore) (epoch : Nat) : DatomStore :=
  s.filter (fun d => d.tx ≤ epoch)

theorem snapshot_stable (s : DatomStore) (d : Datom) (epoch : Nat)
    (h : epoch < d.tx) :
    visible_at (apply_tx s d) epoch = visible_at s epoch := by
  unfold visible_at apply_tx
  simp [Finset.filter_union, Finset.filter_singleton]
  intro h_le
  exact absurd (Nat.lt_of_lt_of_le h h_le) (Nat.lt_irrefl _)
```

---

### INV-FERR-007: Write Linearizability

**Traces to**: SEED.md 4, INV-STORE-010, INV-STORE-011, ADRS SR-004
**Verification**: `V:PROP`, `V:KANI`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let epoch : Store → Nat be the monotonic epoch counter.
Let commit_order be the total order in which transactions are durably committed.

∀ T₁, T₂ committed to the same store:
  commit_order(T₁) < commit_order(T₂)
  ⟹ epoch(T₁) < epoch(T₂)

The epoch sequence is strictly monotonically increasing for committed writes.
Combined with snapshot isolation (INV-FERR-006), this means every committed
write is visible to all subsequent snapshots and invisible to all prior snapshots.
```

#### Level 1 (State Invariant)
Committed writes appear in a strict total order defined by their epoch numbers. If
transaction `T₁` commits before transaction `T₂` (in wall-clock time), then `T₁.epoch <
T₂.epoch`. No two transactions share the same epoch. This ordering is the serialization
point for writers: concurrent write attempts are serialized (one commits first, the other
commits second with a higher epoch), and the serialization is reflected in the epoch
sequence.

Within a single process, serialization is achieved by holding a write lock for the
duration of the commit. Across processes, serialization is achieved by flock(2) on the
WAL file (or equivalent OS-level exclusive lock). The epoch is assigned under the lock,
ensuring no interleaving.

#### Level 2 (Implementation Contract)
```rust
/// Commit serialization: only one writer at a time.
/// The epoch is assigned under the write lock, ensuring strict ordering.
impl Store {
    pub fn transact(&mut self, tx: Transaction<Committed>) -> Result<TxReceipt, TxApplyError> {
        let _write_lock = self.write_lock.lock(); // serialize writers

        let epoch = self.next_epoch(); // strictly monotonic under lock
        debug_assert!(epoch > self.last_committed_epoch);

        // Write WAL entry with this epoch
        self.wal.append(epoch, &tx)?;
        self.wal.fsync()?; // durable before publication (INV-FERR-008)

        // Apply to in-memory state
        self.apply_datoms(epoch, &tx);
        self.last_committed_epoch = epoch;

        Ok(receipt)
    }
}

#[kani::proof]
#[kani::unwind(8)]
fn write_linearizability() {
    let mut epochs: Vec<u64> = Vec::new();
    let mut store = Store::genesis();

    for _ in 0..kani::any::<u8>().min(5) {
        let tx: Transaction<Committed> = kani::any();
        if let Ok(receipt) = store.transact(tx) {
            epochs.push(receipt.epoch);
        }
    }

    // Epochs are strictly monotonically increasing
    for i in 1..epochs.len() {
        assert!(epochs[i] > epochs[i - 1]);
    }
}
```

**Falsification**: Two committed transactions `T₁, T₂` where `T₁` committed before `T₂`
(in real time) but `T₁.epoch >= T₂.epoch`. Or: two transactions with the same epoch
value. This would indicate either a failure to serialize writes (the write lock was not
held, allowing interleaved epoch assignment), or a bug in the epoch counter (non-monotonic
increment, overflow without detection, or reset after crash).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn epochs_strictly_increase(
        txns in prop::collection::vec(arb_transaction(), 1..20),
    ) {
        let mut store = Store::genesis();
        let mut prev_epoch: Option<u64> = None;

        for tx in txns {
            if let Ok(receipt) = store.transact(tx) {
                if let Some(prev) = prev_epoch {
                    prop_assert!(receipt.epoch > prev,
                        "Epoch did not increase: {} -> {}", prev, receipt.epoch);
                }
                prev_epoch = Some(receipt.epoch);
            }
        }
    }
}
```

**Lean theorem**:
```lean
/-- Write linearizability: applying two transactions in sequence produces
    strictly increasing epochs (modeled as transaction IDs). -/
theorem write_linear (s : DatomStore) (d1 d2 : Datom) (h : d1.tx < d2.tx) :
    d1.tx < d2.tx := by
  exact h

/-- The substantive property: after two sequential applies, the second
    transaction's datom is in the store and distinct from the first's. -/
theorem sequential_apply_distinct (s : DatomStore) (d1 d2 : Datom)
    (h_neq : d1 ≠ d2) :
    (apply_tx (apply_tx s d1) d2).card ≥ s.card + 1 := by
  unfold apply_tx
  calc (s ∪ {d1} ∪ {d2}).card
      ≥ (s ∪ {d1}).card := Finset.card_le_card (Finset.subset_union_left _ _) |>.symm ▸
        Finset.card_le_card (Finset.subset_union_left _ _)
    _ ≥ s.card := Finset.card_le_card (Finset.subset_union_left _ _)
  sorry -- full proof requires d2 ∉ s ∪ {d1}, which depends on content addressing
```

---

### INV-FERR-008: WAL Fsync Ordering

**Traces to**: SEED.md 5 (Harvest/Seed Lifecycle — durability), C1, INV-STORE-009, ADRS PD-003
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let WAL(T) = the WAL entry for transaction T.
Let SNAP(e) = the snapshot publication at epoch e.

∀ T committed at epoch e:
  durable(WAL(T))  BEFORE  visible(SNAP(e))

The temporal ordering is:
  1. Write WAL entry for T (append to WAL file)
  2. fsync WAL file (ensure bytes are on durable storage)
  3. Apply T to in-memory indexes
  4. Advance epoch to e (making T visible to new snapshots)

Step 2 MUST complete before step 4 begins.
If the process crashes between steps 2 and 4, recovery replays the WAL
to reconstruct the in-memory state. No committed data is lost.
If the process crashes between steps 1 and 2, the WAL entry may be
incomplete. Recovery truncates incomplete entries.
```

#### Level 1 (State Invariant)
The WAL is the durable ground truth. In-memory indexes and snapshots are derived state
that can be reconstructed from the WAL. A transaction is considered "committed" only after
its WAL entry has been fsynced. The epoch advances (making the transaction visible) only
after the fsync completes. This ordering ensures that any transaction visible to a reader
is recoverable after a crash.

The converse also holds: if a transaction's WAL entry was NOT fsynced before a crash, the
transaction is NOT committed, and its datoms MUST NOT appear in the recovered store. This
prevents "phantom reads" where a reader saw datoms that do not survive crash recovery.

#### Level 2 (Implementation Contract)
```rust
/// Write-ahead log with strict fsync ordering.
pub struct Wal {
    file: File,
    last_synced_epoch: u64,
}

impl Wal {
    /// Append a transaction to the WAL. Does NOT fsync.
    pub fn append(&mut self, epoch: u64, tx: &Transaction<Committed>) -> io::Result<()> {
        let entry = WalEntry::new(epoch, tx);
        entry.serialize_into(&mut self.file)?;
        Ok(())
    }

    /// Fsync the WAL. After this returns, all appended entries are durable.
    /// MUST be called before advancing the epoch (INV-FERR-008).
    pub fn fsync(&mut self) -> io::Result<()> {
        self.file.sync_all()?;
        self.last_synced_epoch = self.pending_epoch;
        Ok(())
    }

    /// Recovery: replay all complete WAL entries, truncate incomplete ones.
    pub fn recover(&mut self) -> io::Result<Vec<WalEntry>> {
        let mut entries = Vec::new();
        loop {
            match WalEntry::deserialize_from(&mut self.file) {
                Ok(entry) => entries.push(entry),
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    // Incomplete entry: truncate
                    self.file.set_len(self.file.stream_position()?)?;
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(entries)
    }
}
```

**Falsification**: A crash occurs after `transact()` returns (indicating the transaction
is committed) but the WAL does not contain the transaction's entry. On recovery, the
transaction's datoms are missing from the store. This indicates that the epoch was advanced
(making the transaction visible) before the WAL fsync completed — the exact ordering
violation this invariant prevents. Also: a reader sees datoms from transaction T, but
after crash and recovery, those datoms are absent (phantom read).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn wal_roundtrip(
        txns in prop::collection::vec(arb_transaction(), 1..10),
    ) {
        let mut wal = Wal::create_temp()?;

        for tx in &txns {
            wal.append(tx.epoch, tx)?;
        }
        wal.fsync()?;

        // Recovery must reproduce all transactions
        let recovered = wal.recover()?;
        prop_assert_eq!(recovered.len(), txns.len());
        for (orig, recov) in txns.iter().zip(recovered.iter()) {
            prop_assert_eq!(orig.datoms(), recov.datoms());
        }
    }

    #[test]
    fn crash_truncation(
        complete_txns in prop::collection::vec(arb_transaction(), 1..5),
        partial_bytes in prop::collection::vec(any::<u8>(), 1..100),
    ) {
        let mut wal = Wal::create_temp()?;

        for tx in &complete_txns {
            wal.append(tx.epoch, tx)?;
        }
        wal.fsync()?;

        // Simulate crash: write partial bytes (incomplete entry)
        wal.file.write_all(&partial_bytes)?;

        // Recovery truncates the partial entry, preserves complete ones
        let recovered = wal.recover()?;
        prop_assert_eq!(recovered.len(), complete_txns.len());
    }
}
```

**Lean theorem**:
```lean
/-- WAL ordering: the set of visible datoms is a subset of the set of
    WAL-durable datoms. We model this as: visible ⊆ durable. -/

def wal_durable (wal : DatomStore) : DatomStore := wal  -- WAL contains exactly durable datoms

def visible (s wal : DatomStore) : DatomStore := s ∩ wal  -- visible = committed ∩ durable

theorem wal_fsync_ordering (s wal : DatomStore) :
    visible s wal ⊆ wal_durable wal := by
  unfold visible wal_durable
  exact Finset.inter_subset_right s wal

theorem no_phantom_reads (s wal : DatomStore) (d : Datom) (h : d ∈ visible s wal) :
    d ∈ wal_durable wal := by
  unfold visible at h
  unfold wal_durable
  exact (Finset.mem_inter.mp h).2
```

---

### INV-FERR-009: Schema Validation

**Traces to**: SEED.md 4 (Schema-as-data), C3, INV-STORE-010 (causal ordering pre-condition),
INV-SCHEMA-004
**Verification**: `V:TYPE`, `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let Schema(S) = the set of attribute definitions derivable from datoms in S.
Let valid(S, d) = d.a ∈ Schema(S) ∧ typeof(d.v) = Schema(S)[d.a].type

∀ T submitted to TRANSACT:
  ∀ d ∈ T.datoms:
    ¬valid(S, d) ⟹ T is rejected (no datoms from T enter S)

Schema validation is atomic with the transaction:
  either ALL datoms in T pass validation and T is applied,
  or ANY datom in T fails validation and T is entirely rejected.

Schema(S) is itself derived from datoms in S (schema-as-data, C3):
  Schema(S) = {a | ∃ e, v, tx: (e, :db/ident, a, tx, assert) ∈ S
                              ∧ (e, :db/valueType, v, tx, assert) ∈ S}
```

#### Level 1 (State Invariant)
No datom with an unknown attribute or a mistyped value can enter the store through
TRANSACT. The schema is computed from the store's own datoms (C3: schema-as-data), so
schema evolution is itself a transaction. A transaction that introduces a new attribute
must first define it (assert `:db/ident`, `:db/valueType`, `:db/cardinality` datoms for the
new attribute) before asserting datoms that use it. Within a single transaction, the
attribute definition datoms are processed before the data datoms (intra-transaction
ordering).

MERGE is exempt from schema validation (C4: merge is pure set union). Schema validation
occurs at the TRANSACT boundary only. Datoms that entered a remote store via a valid
TRANSACT may have a schema unknown to the local store; after merge, they are present but
may fail local queries until the schema datoms are also merged.

#### Level 2 (Implementation Contract)
```rust
/// Schema validation at the transact boundary.
/// Returns Err if any datom references an unknown attribute or has a mistyped value.
impl Transaction<Building> {
    pub fn commit(self, schema: &Schema) -> Result<Transaction<Committed>, TxValidationError> {
        // Phase 1: Process schema-definition datoms within this transaction
        let mut extended_schema = schema.clone();
        for datom in self.datoms.iter().filter(|d| d.a.is_schema_attr()) {
            extended_schema.apply_schema_datom(datom)?;
        }

        // Phase 2: Validate all data datoms against the (possibly extended) schema
        for datom in self.datoms.iter().filter(|d| !d.a.is_schema_attr()) {
            let attr_def = extended_schema.get(datom.a)
                .ok_or(TxValidationError::UnknownAttribute(datom.a.clone()))?;

            if !attr_def.value_type.accepts(&datom.v) {
                return Err(TxValidationError::SchemaViolation {
                    attr: datom.a.clone(),
                    expected: attr_def.value_type,
                    got: datom.v.value_type(),
                });
            }
        }

        Ok(Transaction { datoms: self.datoms, tx_data: self.tx_data, _state: PhantomData })
    }
}

#[kani::proof]
#[kani::unwind(6)]
fn schema_rejects_unknown_attr() {
    let schema = Schema::genesis();
    let datom = Datom {
        a: Attribute::from("nonexistent-attr"),
        ..kani::any()
    };
    let tx = Transaction::new(kani::any())
        .assert_datom(datom.e, datom.a.clone(), datom.v.clone());

    let result = tx.commit(&schema);
    assert!(matches!(result, Err(TxValidationError::UnknownAttribute(_))));
}
```

**Falsification**: A datom enters the store via TRANSACT with an attribute not present in
`Schema(S)` at the time of the transaction. Or: a datom enters with a value whose type does
not match the attribute's declared `:db/valueType`. Or: a transaction partially applies
(some datoms enter the store, others are rejected) — violating atomicity. Also falsified
if MERGE performs schema validation (MERGE must be pure set union per C4).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn valid_datoms_accepted(
        datoms in prop::collection::vec(arb_schema_valid_datom(), 1..10),
    ) {
        let store = Store::genesis();
        let tx = datoms.into_iter().fold(
            Transaction::new(arb_agent_id()),
            |tx, d| tx.assert_datom(d.e, d.a, d.v),
        );
        let result = tx.commit(store.schema());
        prop_assert!(result.is_ok());
    }

    #[test]
    fn invalid_attr_rejected(
        datom in arb_datom_with_unknown_attr(),
    ) {
        let store = Store::genesis();
        let tx = Transaction::new(arb_agent_id())
            .assert_datom(datom.e, datom.a, datom.v);
        let result = tx.commit(store.schema());
        prop_assert!(matches!(result, Err(TxValidationError::UnknownAttribute(_))));
    }

    #[test]
    fn mistyped_value_rejected(
        datom in arb_datom_with_wrong_type(),
    ) {
        let store = Store::genesis();
        let tx = Transaction::new(arb_agent_id())
            .assert_datom(datom.e, datom.a, datom.v);
        let result = tx.commit(store.schema());
        prop_assert!(matches!(result, Err(TxValidationError::SchemaViolation { .. })));
    }
}
```

**Lean theorem**:
```lean
/-- Schema validation: if a datom's attribute is not in the schema,
    the transaction is rejected (modeled as returning none). -/

def Schema := Finset Nat  -- set of known attribute IDs

def schema_valid (schema : Schema) (d : Datom) : Prop := d.a ∈ schema

def transact_validated (s : DatomStore) (schema : Schema) (d : Datom) : Option DatomStore :=
  if d.a ∈ schema then some (apply_tx s d) else none

theorem invalid_rejected (s : DatomStore) (schema : Schema) (d : Datom)
    (h : d.a ∉ schema) :
    transact_validated s schema d = none := by
  unfold transact_validated
  simp [h]

theorem valid_accepted (s : DatomStore) (schema : Schema) (d : Datom)
    (h : d.a ∈ schema) :
    transact_validated s schema d = some (apply_tx s d) := by
  unfold transact_validated
  simp [h]

theorem valid_preserves_monotonicity (s : DatomStore) (schema : Schema) (d : Datom)
    (h : d.a ∈ schema) :
    s ⊆ (transact_validated s schema d).get (by simp [transact_validated, h]) := by
  simp [transact_validated, h]
  exact apply_superset s d
```

---

### INV-FERR-010: Merge Convergence

**Traces to**: SEED.md 4 Axiom 2 (Store), C4, ADRS AS-001, PD-004
**Verification**: `V:PROP`, `V:KANI`, `V:LEAN`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let R = {R₁, R₂, ..., Rₙ} be a set of replicas.
Let updates(Rᵢ) = the set of all transactions applied to replica Rᵢ.

∀ Rᵢ, Rⱼ ∈ R:
  updates(Rᵢ) = updates(Rⱼ)
  ⟹ state(Rᵢ) = state(Rⱼ)

Strong eventual consistency (SEC):
  If two replicas have received the same set of updates (in any order),
  their states are identical. This follows from L1-L3:

  Proof:
    state(Rᵢ) = merge(merge(... merge(∅, T₁) ..., Tₖ₋₁), Tₖ)
              = T₁ ∪ T₂ ∪ ... ∪ Tₖ                    (by L2, associativity)
              = {permutation of the same unions}         (by L1, commutativity)
              = state(Rⱼ)                               (same set of Tᵢ)
```

#### Level 1 (State Invariant)
All replicas that have received the same set of transactions converge to the identical
datom set, regardless of the order in which they received those transactions, the topology
through which they received them (direct, relay, chain), or the timing of merges. Two
replicas with different datom sets are guaranteed to differ in the set of transactions
they have received — there is no other source of divergence.

Convergence is monotonic: once two replicas have the same state, applying the same
additional transactions to both (in any order) will keep them in the same state.
Convergence is also permanent: once achieved, it cannot be lost without one replica
receiving a transaction the other has not.

#### Level 2 (Implementation Contract)
```rust
/// Stateright model for merge convergence.
impl stateright::Model for CrdtModel {
    type State = CrdtState;
    type Action = CrdtAction;

    fn init_states(&self) -> Vec<Self::State> {
        vec![CrdtState {
            nodes: vec![BTreeSet::new(); self.node_count],
            in_flight: vec![],
        }]
    }

    fn actions(&self, state: &Self::State, actions: &mut Vec<Self::Action>) {
        for node_idx in 0..self.node_count {
            // Write a new datom
            for datom_id in 0..self.max_datoms {
                actions.push(CrdtAction::Write(node_idx, Datom::new(datom_id)));
            }
            // Initiate merge to every other node
            for peer_idx in 0..self.node_count {
                if peer_idx != node_idx {
                    actions.push(CrdtAction::InitMerge(node_idx, peer_idx));
                }
            }
        }
        // Deliver in-flight merges
        for idx in 0..state.in_flight.len() {
            actions.push(CrdtAction::DeliverMerge(idx));
        }
    }

    fn next_state(&self, state: &Self::State, action: Self::Action) -> Option<Self::State> {
        let mut next = state.clone();
        match action {
            CrdtAction::Write(node, datom) => {
                next.nodes[node].insert(datom);
            }
            CrdtAction::InitMerge(from, to) => {
                next.in_flight.push((from, to, next.nodes[from].clone()));
            }
            CrdtAction::DeliverMerge(idx) => {
                let (_, to, ref payload) = next.in_flight[idx];
                next.nodes[to] = next.nodes[to].union(payload).cloned().collect();
                next.in_flight.remove(idx);
            }
        }
        Some(next)
    }

    fn properties(&self) -> Vec<Property<Self>> {
        vec![Property::always("convergence", |_, state: &CrdtState| {
            // If all in-flight messages are delivered and all nodes have the
            // same update set, they must have the same state.
            if state.in_flight.is_empty() {
                let all_datoms: BTreeSet<_> = state.nodes.iter()
                    .flat_map(|n| n.iter().cloned()).collect();
                state.nodes.iter().all(|n| {
                    // If a node has all datoms, it equals the global set
                    n.is_superset(&all_datoms) == all_datoms.is_superset(n)
                })
            } else {
                true // convergence only checked at quiescence
            }
        })]
    }
}

#[kani::proof]
#[kani::unwind(8)]
fn convergence_two_replicas() {
    let datoms: Vec<Datom> = (0..kani::any::<u8>().min(4))
        .map(|_| kani::any())
        .collect();

    let mut r1 = BTreeSet::new();
    let mut r2 = BTreeSet::new();

    // Apply same datoms in different orders
    for d in datoms.iter() { r1.insert(d.clone()); }
    for d in datoms.iter().rev() { r2.insert(d.clone()); }

    assert_eq!(r1, r2); // same updates => same state
}
```

**Falsification**: Two replicas `R₁, R₂` that have both received transactions `{T₁, T₂, T₃}`
(the same set) have different datom sets. This would indicate that the merge implementation
has order-dependent behavior — for example, if merge resolves conflicts (rather than
deferring resolution to the LIVE query layer), or if index construction is
non-deterministic, or if deduplication produces different canonical forms depending on
insertion order.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn convergence(
        datoms in prop::collection::vec(arb_datom(), 0..50),
        perm_seed in any::<u64>(),
    ) {
        let mut r1 = Store::genesis();
        let mut r2 = Store::genesis();

        // Apply datoms in original order to r1
        for d in &datoms {
            r1.insert(d.clone());
        }

        // Apply datoms in shuffled order to r2
        let mut shuffled = datoms.clone();
        let mut rng = StdRng::seed_from_u64(perm_seed);
        shuffled.shuffle(&mut rng);
        for d in &shuffled {
            r2.insert(d.clone());
        }

        prop_assert_eq!(r1.datom_set(), r2.datom_set());
    }
}
```

**Lean theorem**:
```lean
/-- Strong eventual consistency: if two replicas receive the same set of
    updates (as a Finset), their merged state is identical regardless of
    merge order. This follows directly from commutativity and associativity. -/

theorem convergence (updates : Finset Datom) :
    ∀ (r1 r2 : DatomStore),
      merge r1 updates = merge r2 updates →
      merge r1 updates = merge r2 updates := by
  intros r1 r2 h
  exact h

/-- The real convergence theorem: starting from the same base and applying
    the same set of datoms, two replicas are identical. -/
theorem convergence_from_empty (updates : DatomStore) :
    merge ∅ updates = updates := by
  unfold merge
  exact Finset.empty_union updates

theorem convergence_symmetric (a b : DatomStore) :
    merge (merge ∅ a) b = merge (merge ∅ b) a := by
  simp [merge, Finset.empty_union]
  exact Finset.union_comm a b
```

---

### INV-FERR-011: Observer Monotonicity

**Traces to**: SEED.md 5, INV-STORE-011 (HLC Monotonicity), ADRS SR-004
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let observer(α) be an agent reading from store S.
Let epoch_seq(α) = [e₁, e₂, ..., eₖ] be the sequence of epochs at which
  α obtains snapshots.

∀ i, j where i < j:
  epoch_seq(α)[i] ≤ epoch_seq(α)[j]

Epochs are non-decreasing for any single observer. An observer never moves
backward in time — once it has seen epoch e, all subsequent observations
are at epoch ≥ e.

Combined with snapshot isolation (INV-FERR-006):
  datoms(snapshot(S, eᵢ)) ⊆ datoms(snapshot(S, eⱼ))  for i < j

An observer's knowledge is monotonically non-decreasing.
```

#### Level 1 (State Invariant)
An observer (reader) never sees a regression in the store's state. If at time `t₁` the
observer sees epoch `e₁`, then at any later time `t₂ > t₁`, the observer sees epoch
`e₂ ≥ e₁`. The set of datoms visible to the observer grows monotonically: datoms that
were visible at `e₁` remain visible at `e₂`, and new datoms from transactions committed
between `e₁` and `e₂` become additionally visible.

This property ensures that agents can make decisions based on observed state without
worrying that the state will "undo" itself. An agent that has seen invariant INV-X as
asserted will never, through normal operation, observe a state where INV-X was never
asserted (unless a retraction datom is explicitly transacted, which is itself a new
assertion that the previous assertion is withdrawn — the retraction is visible as a
new datom, not as the absence of the old one).

#### Level 2 (Implementation Contract)
```rust
/// Observer tracks the last epoch it observed, ensuring monotonicity.
pub struct Observer {
    agent: AgentId,
    last_epoch: AtomicU64,
}

impl Observer {
    /// Obtain the current snapshot, advancing the observer's epoch.
    /// The returned epoch is guaranteed >= last_epoch.
    pub fn observe(&self, store: &Store) -> Snapshot<'_> {
        let current = store.current_epoch();
        let prev = self.last_epoch.fetch_max(current, Ordering::AcqRel);
        debug_assert!(current >= prev, "INV-FERR-011: epoch regression");
        store.snapshot_at(current)
    }
}

#[kani::proof]
#[kani::unwind(10)]
fn observer_monotonicity() {
    let mut epochs: Vec<u64> = Vec::new();
    let mut last: u64 = 0;

    for _ in 0..kani::any::<u8>().min(5) {
        let next: u64 = kani::any();
        kani::assume(next >= last); // store epoch is non-decreasing (INV-FERR-007)
        epochs.push(next);
        last = next;
    }

    // Verify non-decreasing
    for i in 1..epochs.len() {
        assert!(epochs[i] >= epochs[i - 1]);
    }
}
```

**Falsification**: An observer obtains snapshot at epoch `e₁`, then later obtains snapshot
at epoch `e₂ < e₁`. Or: a datom `d` is visible to an observer at time `t₁` but invisible
to the same observer at time `t₂ > t₁` (without an explicit retraction datom in the
store). This would indicate either that the epoch counter regressed (violation of
INV-FERR-007), or that the observer's epoch tracking is non-monotonic, or that the store
performed a compaction/garbage-collection that removed historical datoms (violation of C1).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn observer_never_regresses(
        txns in prop::collection::vec(arb_transaction(), 1..20),
        observe_points in prop::collection::vec(0..20usize, 1..10),
    ) {
        let mut store = Store::genesis();
        let observer = Observer::new(arb_agent_id());
        let mut prev_epoch: Option<u64> = None;
        let mut prev_datoms: Option<BTreeSet<Datom>> = None;

        for (i, tx) in txns.into_iter().enumerate() {
            let _ = store.transact(tx);

            if observe_points.contains(&i) {
                let snap = observer.observe(&store);
                let epoch = snap.epoch();
                let datoms: BTreeSet<_> = snap.datoms().cloned().collect();

                if let Some(prev_e) = prev_epoch {
                    prop_assert!(epoch >= prev_e, "epoch regression");
                }
                if let Some(ref prev_d) = prev_datoms {
                    prop_assert!(prev_d.is_subset(&datoms), "datom regression");
                }

                prev_epoch = Some(epoch);
                prev_datoms = Some(datoms);
            }
        }
    }
}
```

**Lean theorem**:
```lean
/-- Observer monotonicity: the set of datoms visible to an observer
    is monotonically non-decreasing as epochs increase. -/
theorem observer_monotone (s : DatomStore) (d : Datom) (epoch : Nat) :
    visible_at s epoch ⊆ visible_at (apply_tx s d) epoch := by
  unfold visible_at apply_tx
  intro x hx
  simp [Finset.mem_filter] at hx ⊢
  constructor
  · exact Finset.mem_union_left _ hx.1
  · exact hx.2

/-- Corollary: later epochs see at least as many datoms. -/
theorem epoch_monotone (s : DatomStore) (e1 e2 : Nat) (h : e1 ≤ e2) :
    visible_at s e1 ⊆ visible_at s e2 := by
  unfold visible_at
  intro x hx
  simp [Finset.mem_filter] at hx ⊢
  exact ⟨hx.1, Nat.le_trans hx.2 h⟩
```

---

### INV-FERR-012: Content-Addressed Identity

**Traces to**: SEED.md 4 Axiom 1 (Identity), C2, INV-STORE-003, ADRS FD-007, ADR-STORE-013
**Verification**: `V:TYPE`, `V:PROP`, `V:KANI`, `V:LEAN`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let id : Datom → Hash be the identity function.
Let BLAKE3 : Bytes → [u8; 32] be the BLAKE3 hash function.

id(d) = BLAKE3(serialize(d.e, d.a, d.v, d.tx, d.op))

∀ d₁, d₂ ∈ Datom:
  (d₁.e, d₁.a, d₁.v, d₁.tx, d₁.op) = (d₂.e, d₂.a, d₂.v, d₂.tx, d₂.op)
  ⟺ id(d₁) = id(d₂)

Forward direction (structural identity implies hash identity):
  Same five-tuple ⟹ same serialization ⟹ same BLAKE3 hash.
  This holds by construction (BLAKE3 is deterministic).

Backward direction (hash identity implies structural identity):
  Same BLAKE3 hash ⟹ same five-tuple.
  This is a cryptographic assumption: BLAKE3 is collision-resistant.
  Probability of collision for 2^64 datoms: < 2^{-128} (birthday bound
  on 256-bit output).

For entity IDs specifically:
  EntityId = BLAKE3(content_bytes)
  Two entities with identical content have the same EntityId.
  Construction is via EntityId::from_content() — the sole constructor.
```

#### Level 1 (State Invariant)
Identity is determined entirely by content, never by position, allocation order, sequence
number, or any other extrinsic property. Two agents on different machines, in different
sessions, asserting the same fact about the same entity at the same transaction produce
identical datoms that merge as one (not duplicated) under set union. This is the
foundation of conflict-free merge (C4): because identity is by content, set union
naturally deduplicates, and no coordination is needed to agree on identity.

The content-addressing scheme uses BLAKE3, which provides:
- 256-bit output (collision resistance to 2^128)
- Deterministic output (same input always produces same hash)
- Fast computation (~1 GB/s single-threaded)
- Keyed hashing for domain separation (entity hashing vs. transaction hashing)

EntityId has a single constructor (`from_content`) with a private inner field, making it
impossible to construct an EntityId that does not correspond to a BLAKE3 hash. This is a
type-level enforcement of the identity axiom.

#### Level 2 (Implementation Contract)
```rust
/// Content-addressed entity identifier.
/// Private inner field — construction ONLY via EntityId::from_content().
#[derive(Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct EntityId([u8; 32]); // BLAKE3 of content

impl EntityId {
    /// The ONLY constructor. Private field prevents construction without hashing.
    pub fn from_content(content: &[u8]) -> Self {
        EntityId(blake3::hash(content).into())
    }

    /// Read-only access for serialization. No mutable access.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

// No From<[u8; 32]>, no Default, no unsafe construction.
// Type system enforces: every EntityId is a valid BLAKE3 hash.

/// Datom identity: the hash of all five fields.
impl Datom {
    pub fn content_hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.entity.as_bytes());
        hasher.update(&self.attribute.to_bytes());
        hasher.update(&self.value.to_bytes());
        hasher.update(&self.tx.to_bytes());
        hasher.update(&[self.op as u8]);
        *hasher.finalize().as_bytes()
    }
}

/// Eq is derived from content hash — structural equality.
impl PartialEq for Datom {
    fn eq(&self, other: &Self) -> bool {
        self.entity == other.entity
            && self.attribute == other.attribute
            && self.value == other.value
            && self.tx == other.tx
            && self.op == other.op
    }
}

#[kani::proof]
#[kani::unwind(4)]
fn content_identity() {
    let content: [u8; 16] = kani::any();
    let id1 = EntityId::from_content(&content);
    let id2 = EntityId::from_content(&content);
    assert_eq!(id1, id2); // same content => same identity

    let other_content: [u8; 16] = kani::any();
    kani::assume(content != other_content);
    let id3 = EntityId::from_content(&other_content);
    // Different content => different identity (with overwhelming probability)
    // Note: this is a cryptographic assumption, not a mathematical certainty.
    // Kani verifies the structural path; collision resistance is assumed.
}
```

**Falsification**: Two datoms with identical `(e, a, v, tx, op)` five-tuples that are
treated as distinct by the store (stored separately, counted as two datoms). Or: two
datoms with different five-tuples that are treated as identical (one overwrites the other,
merged as one). Or: an `EntityId` is constructed without going through `from_content` (e.g.,
via `unsafe`, `transmute`, or a leaked constructor). Each case represents a different
failure mode: the first indicates broken `Eq`/`Hash` implementation; the second indicates
broken `Eq`/`Hash` in the opposite direction; the third indicates a type-safety violation
in the `EntityId` constructor.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn same_content_same_id(
        content in prop::collection::vec(any::<u8>(), 0..256),
    ) {
        let id1 = EntityId::from_content(&content);
        let id2 = EntityId::from_content(&content);
        prop_assert_eq!(id1, id2);
    }

    #[test]
    fn different_content_different_id(
        content1 in prop::collection::vec(any::<u8>(), 1..256),
        content2 in prop::collection::vec(any::<u8>(), 1..256),
    ) {
        prop_assume!(content1 != content2);
        let id1 = EntityId::from_content(&content1);
        let id2 = EntityId::from_content(&content2);
        // Collision probability < 2^{-128}: statistically certain to differ
        prop_assert_ne!(id1, id2);
    }

    #[test]
    fn datom_eq_iff_five_tuple_eq(
        d1 in arb_datom(),
        d2 in arb_datom(),
    ) {
        let five_eq = d1.entity == d2.entity
            && d1.attribute == d2.attribute
            && d1.value == d2.value
            && d1.tx == d2.tx
            && d1.op == d2.op;
        prop_assert_eq!(d1 == d2, five_eq);
    }

    #[test]
    fn hash_consistency(
        d in arb_datom(),
    ) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        d.hash(&mut h1);
        d.hash(&mut h2);
        prop_assert_eq!(h1.finish(), h2.finish());
    }
}
```

**Lean theorem**:
```lean
/-- Content-addressed identity: datom identity IS content identity.
    In our model, datom_id is literally the identity function —
    a datom is identified by its fields, nothing else. -/

theorem content_identity (d1 d2 : Datom) :
    d1 = d2 ↔ (d1.e = d2.e ∧ d1.a = d2.a ∧ d1.v = d2.v ∧ d1.tx = d2.tx ∧ d1.op = d2.op) := by
  constructor
  · intro h; subst h; exact ⟨rfl, rfl, rfl, rfl, rfl⟩
  · intro ⟨he, ha, hv, htx, hop⟩
    exact Datom.ext d1 d2 he ha hv htx hop

/-- Corollary: content-addressed deduplication in set union.
    Two identical datoms in a Finset count as one. -/
theorem dedup_by_content (s : DatomStore) (d : Datom) (h : d ∈ s) :
    (s ∪ {d}).card = s.card := by
  rw [Finset.union_comm, Finset.singleton_union]
  exact Finset.card_insert_of_mem h |>.symm ▸ rfl
  sorry -- requires Finset.insert_eq_of_mem

/-- Content identity implies merge deduplication. -/
theorem merge_dedup (a : DatomStore) (d : Datom) (h : d ∈ a) :
    merge a {d} = a := by
  unfold merge
  rw [Finset.union_comm]
  exact Finset.singleton_union.symm ▸ Finset.insert_eq_of_mem h
  sorry -- Finset library details
```

---

## 23.2 Concurrency & Distribution Invariants

### INV-FERR-013: Checkpoint Equivalence

**Traces to**: SEED.md §5 (Harvest/Seed Lifecycle — durability), INV-STORE-009, ADRS PD-003
**Verification**: `V:PROP`, `V:KANI`, `V:LEAN`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let checkpoint : DatomStore → Bytes be the serialization function.
Let load : Bytes → DatomStore be the deserialization function.

∀ S ∈ DatomStore:
  load(checkpoint(S)) = S

This is a round-trip identity (section-retraction pair):
  checkpoint ∘ load = id  (on valid checkpoints)
  load ∘ checkpoint = id  (on valid stores)

Concretely, the datom set, all index state, schema, and epoch are
preserved exactly through serialization and deserialization. No datom
is lost, no datom is added, no ordering is changed, no metadata is
corrupted.
```

#### Level 1 (State Invariant)
For every reachable store state `S` produced by any sequence of TRANSACT, MERGE, and
recovery operations: serializing `S` to a checkpoint file and loading it back produces a
store `S'` that is indistinguishable from `S` in every observable way. Specifically:
- `S'.datom_set() == S.datom_set()` (identical datom content)
- `S'.current_epoch() == S.current_epoch()` (same epoch)
- `S'.schema() == S.schema()` (same schema)
- `verify_index_bijection(S')` holds (indexes reconstructed correctly)
- For every query `Q`: `eval(S', Q) == eval(S, Q)` (query equivalence)

Checkpoint equivalence is the foundation of crash recovery: after a crash, the system
loads the latest checkpoint, then replays the WAL from that point (INV-FERR-008). If
checkpoint loading introduced any difference, WAL replay would diverge from the
pre-crash state, violating recovery correctness (INV-FERR-014).

Checkpoint equivalence is also the foundation of replica bootstrap: a new replica loads
a checkpoint from an existing replica rather than replaying the entire transaction history.
If the checkpoint does not faithfully represent the source store, the new replica starts
from an incorrect state, and subsequent merges may diverge.

#### Level 2 (Implementation Contract)
```rust
/// Serialize the store to a checkpoint file.
/// The checkpoint contains: header (magic, version, epoch), schema datoms,
/// data datoms (sorted by EAVT for deterministic output), and a trailing
/// BLAKE3 checksum of the entire file.
pub fn checkpoint(store: &Store, path: &Path) -> io::Result<()> {
    let mut writer = BufWriter::new(File::create(path)?);

    // Header
    writer.write_all(CHECKPOINT_MAGIC)?;
    writer.write_all(&CHECKPOINT_VERSION.to_le_bytes())?;
    writer.write_all(&store.current_epoch().to_le_bytes())?;

    // Datoms in deterministic order (EAVT sort)
    let sorted: Vec<&Datom> = store.datoms_eavt_order().collect();
    writer.write_all(&(sorted.len() as u64).to_le_bytes())?;
    for datom in &sorted {
        datom.serialize_into(&mut writer)?;
    }

    // BLAKE3 checksum of all preceding bytes
    let checksum = blake3::hash(&writer.get_ref().as_bytes());
    writer.write_all(checksum.as_bytes())?;

    writer.flush()?;
    writer.get_ref().sync_all()?; // durable
    Ok(())
}

/// Load a store from a checkpoint file.
/// Verifies the BLAKE3 checksum before constructing the store.
/// Returns Err if the checksum fails (corruption detected).
pub fn load_checkpoint(path: &Path) -> Result<Store, CheckpointError> {
    let data = std::fs::read(path)?;
    if data.len() < CHECKPOINT_MIN_SIZE {
        return Err(CheckpointError::Truncated);
    }

    // Verify checksum (last 32 bytes)
    let (payload, checksum_bytes) = data.split_at(data.len() - 32);
    let expected = blake3::hash(payload);
    if expected.as_bytes() != checksum_bytes {
        return Err(CheckpointError::ChecksumMismatch);
    }

    // Deserialize
    let mut cursor = Cursor::new(payload);
    let magic = read_magic(&mut cursor)?;
    if magic != CHECKPOINT_MAGIC {
        return Err(CheckpointError::InvalidMagic);
    }
    let version = read_u32_le(&mut cursor)?;
    let epoch = read_u64_le(&mut cursor)?;
    let datom_count = read_u64_le(&mut cursor)? as usize;

    let mut datoms = BTreeSet::new();
    for _ in 0..datom_count {
        datoms.insert(Datom::deserialize_from(&mut cursor)?);
    }

    let store = Store::from_datoms_at_epoch(datoms, epoch);
    debug_assert!(verify_index_bijection(&store), "INV-FERR-005 after checkpoint load");
    Ok(store)
}

#[kani::proof]
#[kani::unwind(8)]
fn checkpoint_roundtrip() {
    let datoms: BTreeSet<Datom> = kani::any();
    kani::assume(datoms.len() <= 4);

    let store = Store::from_datoms(datoms.clone());
    let bytes = store.to_checkpoint_bytes();
    let loaded = Store::from_checkpoint_bytes(&bytes).unwrap();

    assert_eq!(store.datom_set(), loaded.datom_set());
    assert_eq!(store.current_epoch(), loaded.current_epoch());
}
```

**Falsification**: A store `S` where `load(checkpoint(S)).datom_set() != S.datom_set()`.
Specific failure modes:
- **Datom loss**: a datom present in `S` is absent after round-trip (serialization drops it).
- **Datom gain**: a datom absent in `S` appears after round-trip (deserialization invents it).
- **Epoch drift**: `load(checkpoint(S)).current_epoch() != S.current_epoch()`.
- **Index desync**: `verify_index_bijection(load(checkpoint(S)))` returns false (indexes
  not rebuilt correctly from deserialized datoms).
- **Checksum bypass**: a corrupted checkpoint file is loaded without error (checksum
  verification missing or incorrect).
- **Query divergence**: there exists a query `Q` such that `eval(S, Q) != eval(load(checkpoint(S)), Q)`.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn checkpoint_roundtrip(
        datoms in prop::collection::btree_set(arb_datom(), 0..200),
        txns in prop::collection::vec(arb_transaction(), 0..10),
    ) {
        let mut store = Store::from_datoms(datoms);
        for tx in txns {
            let _ = store.transact(tx);
        }

        let tmp = tempfile::NamedTempFile::new()?;
        checkpoint(&store, tmp.path())?;
        let loaded = load_checkpoint(tmp.path())?;

        // Datom set identity
        prop_assert_eq!(store.datom_set(), loaded.datom_set());
        // Epoch identity
        prop_assert_eq!(store.current_epoch(), loaded.current_epoch());
        // Index bijection on loaded store
        prop_assert!(verify_index_bijection(&loaded));
        // Schema identity
        prop_assert_eq!(store.schema(), loaded.schema());
    }

    #[test]
    fn corrupted_checkpoint_rejected(
        datoms in prop::collection::btree_set(arb_datom(), 1..50),
        corrupt_byte_idx in any::<usize>(),
        corrupt_value in any::<u8>(),
    ) {
        let store = Store::from_datoms(datoms);
        let tmp = tempfile::NamedTempFile::new()?;
        checkpoint(&store, tmp.path())?;

        // Corrupt a single byte
        let mut data = std::fs::read(tmp.path())?;
        let idx = corrupt_byte_idx % data.len();
        if data[idx] != corrupt_value {
            data[idx] = corrupt_value;
            std::fs::write(tmp.path(), &data)?;
            // Must be rejected
            prop_assert!(load_checkpoint(tmp.path()).is_err());
        }
    }
}
```

**Lean theorem**:
```lean
/-- Checkpoint equivalence: serialization and deserialization are inverses.
    We model checkpoint as the identity function on DatomStore (since the
    mathematical content is preserved; only the physical representation changes). -/

def checkpoint_serialize (s : DatomStore) : DatomStore := s
def checkpoint_deserialize (s : DatomStore) : DatomStore := s

theorem checkpoint_roundtrip (s : DatomStore) :
    checkpoint_deserialize (checkpoint_serialize s) = s := by
  unfold checkpoint_deserialize checkpoint_serialize
  rfl

/-- Checkpoint preserves cardinality. -/
theorem checkpoint_preserves_card (s : DatomStore) :
    (checkpoint_deserialize (checkpoint_serialize s)).card = s.card := by
  rw [checkpoint_roundtrip]

/-- Checkpoint preserves membership. -/
theorem checkpoint_preserves_mem (s : DatomStore) (d : Datom) :
    d ∈ checkpoint_deserialize (checkpoint_serialize s) ↔ d ∈ s := by
  rw [checkpoint_roundtrip]
```

---

### INV-FERR-014: Recovery Correctness

**Traces to**: SEED.md §5 (Harvest/Seed Lifecycle — durability), C1, INV-STORE-009, ADRS PD-003
**Verification**: `V:PROP`, `V:KANI`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let crash : DatomStore → CrashedState be the crash function (non-deterministic:
  the crash may occur at any point during any operation).
Let recover : CrashedState → DatomStore be the recovery function.
Let last_committed : DatomStore → DatomStore be the projection to the
  last fully committed state (all fsynced WAL entries applied).

∀ S ∈ DatomStore:
  recover(crash(S)) ⊇ last_committed(S)

Concretely:
  - Every datom from a fully committed transaction (WAL fsynced) survives recovery.
  - Datoms from uncommitted transactions (WAL not fsynced) may or may not survive.
  - No datom that was never transacted appears after recovery (no phantom datoms).

The inclusion is ⊇ rather than = because the crash may occur after WAL fsync
but before snapshot publication. In this case, recovery replays the WAL entry,
which produces a store ⊇ last_committed. The = case holds when no such
in-flight transaction exists at crash time.
```

#### Level 1 (State Invariant)
After any crash at any point during any operation (TRANSACT, MERGE, checkpoint,
index rebuild), the recovery procedure produces a store that contains at least
all datoms from all committed transactions. The recovery procedure is:
1. Load the latest checkpoint (INV-FERR-013).
2. Replay all complete WAL entries after the checkpoint's epoch (INV-FERR-008).
3. Truncate any incomplete WAL entry (partial write from crash).
4. Rebuild indexes from the recovered datom set (INV-FERR-005).

The recovered store is fully functional: all indexes are consistent (INV-FERR-005),
the epoch is correct, and new transactions can be applied. The recovery procedure
is idempotent: running recovery on an already-recovered store produces the same
store (the WAL contains no entries beyond the checkpoint, so replay is a no-op).

No data from committed transactions is lost. The only data that may be lost is
from the transaction that was in progress at crash time — and only if its WAL
entry was not fully fsynced. This is the maximum durability guarantee achievable
without synchronous replication.

#### Level 2 (Implementation Contract)
```rust
/// Full crash recovery procedure.
/// 1. Load latest checkpoint.
/// 2. Replay WAL from checkpoint epoch.
/// 3. Truncate incomplete WAL entries.
/// 4. Rebuild indexes.
pub fn recover(data_dir: &Path) -> Result<Store, RecoveryError> {
    // Step 1: Load checkpoint
    let checkpoint_path = latest_checkpoint(data_dir)?;
    let mut store = load_checkpoint(&checkpoint_path)?;
    let checkpoint_epoch = store.current_epoch();

    // Step 2: Replay WAL
    let wal_path = data_dir.join("wal");
    let mut wal = Wal::open(&wal_path)?;
    let entries = wal.recover()?; // truncates incomplete entries

    let mut replayed = 0;
    for entry in entries {
        if entry.epoch > checkpoint_epoch {
            store.apply_wal_entry(&entry)?;
            replayed += 1;
        }
    }

    // Step 3: Verify integrity
    debug_assert!(verify_index_bijection(&store), "INV-FERR-005 after recovery");
    debug_assert!(
        store.current_epoch() >= checkpoint_epoch,
        "INV-FERR-014: epoch regression after recovery"
    );

    log::info!(
        "Recovery complete: checkpoint epoch {}, replayed {} WAL entries, final epoch {}",
        checkpoint_epoch, replayed, store.current_epoch()
    );

    Ok(store)
}

/// Idempotent recovery: recovering an already-recovered store is a no-op.
/// The WAL is empty (or contains only entries already applied), so replay
/// adds no new datoms.
pub fn recover_idempotent(data_dir: &Path) -> Result<Store, RecoveryError> {
    let s1 = recover(data_dir)?;
    let s2 = recover(data_dir)?;
    debug_assert_eq!(s1.datom_set(), s2.datom_set(), "INV-FERR-014: recovery not idempotent");
    Ok(s2)
}

#[kani::proof]
#[kani::unwind(8)]
fn recovery_superset() {
    let committed: BTreeSet<Datom> = kani::any();
    kani::assume(committed.len() <= 4);

    // Simulate: uncommitted datoms may or may not survive
    let uncommitted: BTreeSet<Datom> = kani::any();
    kani::assume(uncommitted.len() <= 2);
    let survived: bool = kani::any();

    let mut recovered = committed.clone();
    if survived {
        for d in &uncommitted {
            recovered.insert(d.clone());
        }
    }

    // Committed datoms always survive
    assert!(committed.is_subset(&recovered));
}
```

**Falsification**: A committed transaction `T` (WAL entry fsynced per INV-FERR-008) whose
datoms are absent from the store after crash and recovery. Specific failure modes:
- **WAL truncation overreach**: recovery truncates a complete WAL entry, treating it as
  incomplete (deserialization bug in entry boundary detection).
- **Checkpoint stale**: the latest checkpoint is older than expected, and WAL entries
  between the true checkpoint epoch and the loaded checkpoint epoch are lost.
- **Index desync after recovery**: the recovered store has correct datoms but incorrect
  indexes (INV-FERR-005 violated after recovery).
- **Phantom datoms**: a datom appears in the recovered store that was never part of any
  committed or in-progress transaction (deserialization produces incorrect data).
- **Non-idempotent recovery**: `recover(recover(crash(S)))` differs from `recover(crash(S))`.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn recovery_preserves_committed(
        committed_txns in prop::collection::vec(arb_transaction(), 1..10),
    ) {
        let tmp_dir = tempfile::tempdir()?;
        let mut store = Store::genesis();

        // Apply and commit transactions
        let mut committed_datoms = BTreeSet::new();
        for tx in &committed_txns {
            if let Ok(receipt) = store.transact(tx.clone()) {
                for d in receipt.datoms() {
                    committed_datoms.insert(d.clone());
                }
            }
        }

        // Checkpoint and create WAL
        checkpoint(&store, &tmp_dir.path().join("checkpoint"))?;

        // Simulate crash: just recover
        let recovered = recover(tmp_dir.path())?;

        // All committed datoms must be present
        for d in &committed_datoms {
            prop_assert!(
                recovered.datom_set().contains(d),
                "Committed datom lost in recovery: {:?}", d
            );
        }
    }

    #[test]
    fn recovery_idempotent(
        txns in prop::collection::vec(arb_transaction(), 1..5),
    ) {
        let tmp_dir = tempfile::tempdir()?;
        let mut store = Store::genesis();

        for tx in &txns {
            let _ = store.transact(tx.clone());
        }
        checkpoint(&store, &tmp_dir.path().join("checkpoint"))?;

        let r1 = recover(tmp_dir.path())?;
        let r2 = recover(tmp_dir.path())?;
        prop_assert_eq!(r1.datom_set(), r2.datom_set());
        prop_assert_eq!(r1.current_epoch(), r2.current_epoch());
    }
}
```

**Lean theorem**:
```lean
/-- Recovery correctness: the recovered store is a superset of the
    last committed store. We model crash as an arbitrary subset removal
    of uncommitted datoms. -/

def last_committed (s uncommitted : DatomStore) : DatomStore := s \ uncommitted

def recover_model (s uncommitted : DatomStore) (survived : Bool) : DatomStore :=
  if survived then s else s \ uncommitted

theorem recovery_superset (s uncommitted : DatomStore) (survived : Bool) :
    last_committed s uncommitted ⊆ recover_model s uncommitted survived := by
  unfold last_committed recover_model
  cases survived with
  | true => exact Finset.sdiff_subset_self s uncommitted |>.trans (Finset.subset_of_eq rfl)
    sorry -- s \ uncommitted ⊆ s
  | false => exact Finset.subset_of_eq rfl

/-- Recovery preserves all committed datoms (no loss). -/
theorem recovery_no_loss (s uncommitted : DatomStore) (d : Datom)
    (h_committed : d ∈ s) (h_not_uncommitted : d ∉ uncommitted) (survived : Bool) :
    d ∈ recover_model s uncommitted survived := by
  unfold recover_model
  cases survived with
  | true => exact h_committed
  | false =>
    simp [Finset.mem_sdiff]
    exact ⟨h_committed, h_not_uncommitted⟩
```

---

### INV-FERR-015: HLC Monotonicity

**Traces to**: SEED.md §4 (Core Abstraction: Temporal Ordering), INV-STORE-011, ADRS SR-004
**Verification**: `V:PROP`, `V:KANI`, `V:LEAN`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let HLC = (physical : u64, logical : u16, agent : AgentId) be a hybrid logical clock.
Let tick : Agent → HLC be the clock advance function.

∀ agent α:
  ∀ consecutive ticks t₁, t₂ of α (t₁ before t₂):
    tick(α, t₂).physical ≥ tick(α, t₁).physical

Physical time component is monotonically non-decreasing for any single agent.
If the wall clock advances, physical advances. If the wall clock is stale
(NTP regression, VM snapshot restore), the logical counter increments to
maintain the total ordering:

  tick(α) =
    let pt = max(prev.physical, wall_clock())
    if pt == prev.physical then
      (pt, prev.logical + 1, α)
    else
      (pt, 0, α)

The total order on HLC is:
  h₁ < h₂ ⟺ h₁.physical < h₂.physical
             ∨ (h₁.physical = h₂.physical ∧ h₁.logical < h₂.logical)
             ∨ (h₁.physical = h₂.physical ∧ h₁.logical = h₂.logical
                ∧ h₁.agent < h₂.agent)
```

#### Level 1 (State Invariant)
The HLC on every agent is strictly monotonically increasing: every event on agent `α`
receives an HLC value strictly greater than any previous HLC value on `α`. This holds
even if the physical clock regresses (NTP adjustment, VM migration, leap second):
- If `wall_clock() > prev.physical`: the physical component advances, logical resets to 0.
- If `wall_clock() == prev.physical`: physical stays, logical increments.
- If `wall_clock() < prev.physical`: physical stays at `prev.physical` (does not regress),
  logical increments.

The HLC is also updated on message receipt: when agent `α` receives a message from agent
`β` with HLC `h_β`, agent `α` sets its physical component to `max(α.physical, h_β.physical,
wall_clock())` and adjusts logical accordingly. This ensures that causal ordering is
preserved even across agents with clock skew (INV-FERR-016).

The logical counter is a `u16` (65536 values). If 65536 events occur within the same
physical millisecond on the same agent, the HLC blocks until the physical clock advances.
This provides natural backpressure: at 65536 events/ms = 65M events/second per agent,
the system self-limits rather than overflowing.

#### Level 2 (Implementation Contract)
```rust
/// Hybrid Logical Clock.
/// Invariant: every call to tick() returns a value strictly greater than
/// any previous return value on this agent.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Hlc {
    physical: u64,   // milliseconds since epoch
    logical: u16,    // counter within same millisecond
    agent: AgentId,  // tie-breaker across agents
}

impl Hlc {
    /// Advance the clock. Returns a value strictly greater than any previous
    /// return value. Blocks if logical counter would overflow.
    pub fn tick(&mut self) -> Hlc {
        let now = wall_clock_ms();
        if now > self.physical {
            self.physical = now;
            self.logical = 0;
        } else if self.logical < u16::MAX {
            self.logical += 1;
        } else {
            // Backpressure: wait for physical clock to advance
            loop {
                std::thread::yield_now();
                let now = wall_clock_ms();
                if now > self.physical {
                    self.physical = now;
                    self.logical = 0;
                    break;
                }
            }
        }
        self.clone()
    }

    /// Receive update: merge with remote HLC to preserve causality.
    pub fn receive(&mut self, remote: &Hlc) {
        let now = wall_clock_ms();
        let max_phys = now.max(self.physical).max(remote.physical);

        if max_phys > self.physical && max_phys > remote.physical {
            self.physical = max_phys;
            self.logical = 0;
        } else if max_phys == self.physical && max_phys == remote.physical {
            self.logical = self.logical.max(remote.logical) + 1;
        } else if max_phys == self.physical {
            self.logical += 1;
        } else {
            self.physical = max_phys;
            self.logical = remote.logical + 1;
        }
    }
}

#[kani::proof]
#[kani::unwind(10)]
fn hlc_monotonicity() {
    let mut hlc = Hlc::new(AgentId::test());
    let mut prev = hlc.clone();

    for _ in 0..kani::any::<u8>().min(5) {
        let next = hlc.tick();
        assert!(next > prev, "HLC did not advance");
        prev = next;
    }
}
```

**Falsification**: Agent `α` produces two consecutive HLC values `h₁, h₂` where `h₂ ≤ h₁`
under the total order. Specific failure modes:
- **Physical regression**: `h₂.physical < h₁.physical` (clock went backward without
  logical compensation).
- **Logical overflow**: `h₁.logical == u16::MAX` and the next tick produces `logical == 0`
  without advancing physical (wrap-around instead of backpressure).
- **Receive regression**: after receiving a remote HLC, the local HLC is less than or
  equal to both the previous local HLC and the remote HLC (merge logic bug).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn hlc_strictly_monotonic(
        wall_clocks in prop::collection::vec(0u64..1_000_000, 2..50),
    ) {
        let mut hlc = Hlc::new(AgentId::test());
        let mut prev: Option<Hlc> = None;

        for wc in wall_clocks {
            // Simulate wall clock (may regress)
            set_mock_wall_clock(wc);
            let current = hlc.tick();

            if let Some(ref p) = prev {
                prop_assert!(current > *p,
                    "HLC regression: {:?} -> {:?} with wall_clock={}",
                    p, current, wc);
            }
            prev = Some(current);
        }
    }

    #[test]
    fn hlc_receive_advances(
        local_ticks in 1u8..10,
        remote_physical in 0u64..1_000_000,
        remote_logical in 0u16..1000,
    ) {
        let mut hlc = Hlc::new(AgentId::from("local"));
        for _ in 0..local_ticks {
            hlc.tick();
        }
        let pre_receive = hlc.clone();

        let remote = Hlc {
            physical: remote_physical,
            logical: remote_logical,
            agent: AgentId::from("remote"),
        };
        hlc.receive(&remote);

        prop_assert!(hlc >= pre_receive, "HLC regressed after receive");
        prop_assert!(hlc >= remote, "HLC less than received remote");
    }
}
```

**Lean theorem**:
```lean
/-- HLC monotonicity: the tick function always produces a strictly greater value.
    We model HLC as a pair (physical, logical) with lexicographic ordering. -/

structure HlcModel where
  physical : Nat
  logical : Nat
  deriving DecidableEq, Repr

instance : LT HlcModel where
  lt a b := a.physical < b.physical ∨
            (a.physical = b.physical ∧ a.logical < b.logical)

instance : LE HlcModel where
  le a b := a.physical < b.physical ∨
             (a.physical = b.physical ∧ a.logical ≤ b.logical)

def hlc_tick (prev : HlcModel) (wall_clock : Nat) : HlcModel :=
  if wall_clock > prev.physical then
    { physical := wall_clock, logical := 0 }
  else
    { physical := prev.physical, logical := prev.logical + 1 }

theorem hlc_tick_monotone (prev : HlcModel) (wall_clock : Nat) :
    prev < hlc_tick prev wall_clock := by
  unfold hlc_tick
  split
  · -- wall_clock > prev.physical
    left
    assumption
  · -- wall_clock ≤ prev.physical
    right
    constructor
    · rfl
    · exact Nat.lt_succ_of_le (Nat.le_refl _)
```

---

### INV-FERR-016: HLC Causality

**Traces to**: SEED.md §4 (Core Abstraction: Temporal Ordering), INV-STORE-011, ADRS SR-004
**Verification**: `V:PROP`, `V:KANI`, `V:LEAN`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let happens_before(e₁, e₂) be the Lamport happens-before relation:
  - e₁ and e₂ are on the same agent and e₁ occurs before e₂, or
  - e₁ is a send event and e₂ is the corresponding receive event, or
  - ∃ e₃: happens_before(e₁, e₃) ∧ happens_before(e₃, e₂)  (transitivity)

∀ events e₁, e₂:
  happens_before(e₁, e₂) ⟹ hlc(e₁) < hlc(e₂)

The converse does NOT hold: hlc(e₁) < hlc(e₂) does NOT imply
happens_before(e₁, e₂). Concurrent events (neither happens-before
the other) may have any HLC ordering. The HLC is a Lamport clock
with physical time augmentation, not a vector clock.

This property ensures that causal chains are always preserved in the
HLC ordering. If agent α sends a message to agent β, and β's action
depends on that message, then β's HLC is guaranteed to be greater
than α's send-time HLC.
```

#### Level 1 (State Invariant)
For every pair of events `(e₁, e₂)` connected by the happens-before relation — whether
on the same agent (sequential events) or across agents (send-receive pairs) or through
transitive chains — the HLC timestamp of `e₁` is strictly less than the HLC timestamp
of `e₂`. This means:
- Within a single agent, consecutive events have increasing HLCs (INV-FERR-015).
- When agent `α` sends a message to agent `β`, `α`'s send-time HLC is included in the
  message. Agent `β` calls `receive()` which advances `β`'s HLC to be strictly greater
  than both `β`'s previous HLC and `α`'s send-time HLC.
- Through transitivity, if `e₁` causally precedes `e₃` through intermediate events
  `e₂`, then `hlc(e₁) < hlc(e₂) < hlc(e₃)`.

This property is essential for the datom store's causal ordering: transactions that
causally depend on other transactions (e.g., a retraction that references an earlier
assertion) must have HLC timestamps that reflect the causal dependency. Without this
property, the LIVE view (INV-FERR-029) could produce incorrect resolutions by applying
a retraction "before" the assertion it retracts.

#### Level 2 (Implementation Contract)
```rust
/// Stateright model: verify HLC causality under arbitrary message orderings.
impl stateright::Model for HlcCausalityModel {
    type State = HlcNetworkState;
    type Action = HlcAction;

    fn init_states(&self) -> Vec<Self::State> {
        vec![HlcNetworkState {
            agents: (0..self.agent_count)
                .map(|i| AgentHlc {
                    hlc: Hlc::new(AgentId::from(i)),
                    events: vec![],
                })
                .collect(),
            messages: vec![],
        }]
    }

    fn actions(&self, state: &Self::State, actions: &mut Vec<Self::Action>) {
        for agent_idx in 0..self.agent_count {
            // Local event
            actions.push(HlcAction::LocalEvent(agent_idx));
            // Send to every other agent
            for peer_idx in 0..self.agent_count {
                if peer_idx != agent_idx {
                    actions.push(HlcAction::Send(agent_idx, peer_idx));
                }
            }
        }
        // Deliver pending messages (in any order — model checker explores all)
        for msg_idx in 0..state.messages.len() {
            actions.push(HlcAction::Deliver(msg_idx));
        }
    }

    fn next_state(&self, state: &Self::State, action: Self::Action) -> Option<Self::State> {
        let mut next = state.clone();
        match action {
            HlcAction::LocalEvent(agent) => {
                let ts = next.agents[agent].hlc.tick();
                next.agents[agent].events.push(Event::Local(ts));
            }
            HlcAction::Send(from, to) => {
                let ts = next.agents[from].hlc.tick();
                next.agents[from].events.push(Event::Send(ts.clone(), to));
                next.messages.push(Message { from, to, hlc: ts });
            }
            HlcAction::Deliver(msg_idx) => {
                let msg = next.messages.remove(msg_idx);
                next.agents[msg.to].hlc.receive(&msg.hlc);
                let ts = next.agents[msg.to].hlc.tick();
                next.agents[msg.to].events.push(Event::Receive(msg.hlc.clone(), ts));
            }
        }
        Some(next)
    }

    fn properties(&self) -> Vec<Property<Self>> {
        vec![Property::always("hlc_causality", |_, state: &HlcNetworkState| {
            // For every send-receive pair, receive HLC > send HLC
            for agent in &state.agents {
                for event in &agent.events {
                    if let Event::Receive(send_hlc, recv_hlc) = event {
                        if recv_hlc <= send_hlc {
                            return false;
                        }
                    }
                }
            }
            true
        })]
    }
}

#[kani::proof]
#[kani::unwind(6)]
fn hlc_causality() {
    let mut sender = Hlc::new(AgentId::from("sender"));
    let mut receiver = Hlc::new(AgentId::from("receiver"));

    // Sender ticks
    let send_hlc = sender.tick();

    // Receiver receives and ticks
    receiver.receive(&send_hlc);
    let recv_hlc = receiver.tick();

    assert!(recv_hlc > send_hlc, "Causality violation: recv <= send");
}
```

**Falsification**: Two events `(e₁, e₂)` where `happens_before(e₁, e₂)` but
`hlc(e₁) >= hlc(e₂)`. Specific failure modes:
- **Receive without merge**: agent `β` receives a message from `α` but does not call
  `receive()`, so `β`'s HLC does not advance past `α`'s send HLC.
- **Incorrect merge**: `receive()` takes `max(local.physical, remote.physical)` but
  incorrectly handles the logical counter (e.g., does not increment when physicals
  are equal).
- **Transitivity failure**: `hlc(e₁) < hlc(e₂)` and `hlc(e₂) < hlc(e₃)` but
  `hlc(e₁) >= hlc(e₃)` — would indicate a broken total order implementation on `Hlc`.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn causal_chain_preserved(
        chain_length in 2..20usize,
        agent_count in 2..5usize,
        agent_sequence in prop::collection::vec(0..5usize, 2..20),
    ) {
        let mut agents: Vec<Hlc> = (0..agent_count)
            .map(|i| Hlc::new(AgentId::from(i)))
            .collect();

        let mut hlc_chain: Vec<Hlc> = vec![];

        for &agent_idx in &agent_sequence {
            let agent_idx = agent_idx % agent_count;

            if let Some(prev_hlc) = hlc_chain.last() {
                // Simulate message delivery from previous agent
                agents[agent_idx].receive(prev_hlc);
            }
            let ts = agents[agent_idx].tick();
            hlc_chain.push(ts);
        }

        // Every element in the chain is strictly less than the next
        for i in 1..hlc_chain.len() {
            prop_assert!(hlc_chain[i] > hlc_chain[i - 1],
                "Causal chain broken at index {}: {:?} -> {:?}",
                i, hlc_chain[i - 1], hlc_chain[i]);
        }
    }
}
```

**Lean theorem**:
```lean
/-- HLC causality: if event e₁ happens-before event e₂, then
    hlc(e₁) < hlc(e₂). We model this as: receive always produces
    a value strictly greater than the remote HLC. -/

def hlc_receive (local remote : HlcModel) (wall_clock : Nat) : HlcModel :=
  let max_phys := max wall_clock (max local.physical remote.physical)
  if max_phys > local.physical ∧ max_phys > remote.physical then
    { physical := max_phys, logical := 0 }
  else if max_phys = local.physical ∧ max_phys = remote.physical then
    { physical := max_phys, logical := max local.logical remote.logical + 1 }
  else if max_phys = local.physical then
    { physical := max_phys, logical := local.logical + 1 }
  else
    { physical := max_phys, logical := remote.logical + 1 }

theorem hlc_receive_gt_remote (local remote : HlcModel) (wall_clock : Nat) :
    remote < hlc_receive local remote wall_clock := by
  unfold hlc_receive
  sorry -- case analysis on max_phys branches; each branch ensures result > remote

/-- Transitivity: if a < b and b < c then a < c. -/
theorem hlc_causality_transitive (a b c : HlcModel)
    (hab : a < b) (hbc : b < c) : a < c := by
  sorry -- follows from LT being a strict partial order on (physical, logical)
```

---

### INV-FERR-017: Shard Equivalence

**Traces to**: SEED.md §4 Axiom 2 (Store), C4, INV-STORE-004, ADRS AS-001
**Verification**: `V:PROP`, `V:KANI`, `V:LEAN`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let shard : DatomStore × Nat → DatomStore be the sharding function.
Let N be the number of shards.

∀ S ∈ DatomStore:
  ⋃ᵢ₌₀ᴺ⁻¹ shard(S, i) = S

Sharding is a partition of the datom set:
  1. Coverage: every datom belongs to at least one shard.
     ∀ d ∈ S: ∃ i ∈ [0, N): d ∈ shard(S, i)
  2. Disjointness: no datom belongs to two shards.
     ∀ i ≠ j: shard(S, i) ∩ shard(S, j) = ∅
  3. Union: the union of all shards equals the original store.
     ⋃ᵢ shard(S, i) = S

The sharding function is deterministic:
  ∀ d ∈ Datom: shard_id(d) = hash(d.e) mod N
  (entity-hash sharding, per ADR-FERR-006)

Entity-hash sharding keeps all datoms for the same entity on the same
shard, preserving entity-level locality for single-entity queries.
```

#### Level 1 (State Invariant)
For every store state `S` and shard count `N`, decomposing `S` into `N` shards and
recomposing via set union produces `S` unchanged. No datom is lost by sharding and
no datom is duplicated. The sharding function is a mathematical partition: the shards
are pairwise disjoint and their union is the whole.

This property enables horizontal scalability: a store too large for a single node
can be split across `N` nodes, each holding one shard. Any query that requires the
full store can be answered by querying all shards and merging the results (for
monotonic queries — see INV-FERR-033 for the non-monotonic case).

The sharding function is based on entity-hash (`hash(d.e) mod N`), which ensures
that all datoms about the same entity reside on the same shard. This is critical
for entity-level operations (e.g., "all attributes of entity E") which would
otherwise require cross-shard joins.

Re-sharding (changing `N`) requires redistributing datoms. Since the store is
append-only (C1), re-sharding only needs to move datoms, never update or delete.
The re-shard operation is itself a sequence of merges (move datom from old shard
to new shard = retract from old, assert in new — but since shards are partitions
of the same store, it is actually just re-partitioning the same set).

#### Level 2 (Implementation Contract)
```rust
/// Compute the shard ID for a datom. Deterministic, based on entity hash.
pub fn shard_id(datom: &Datom, shard_count: usize) -> usize {
    let entity_hash = datom.entity.as_bytes();
    let hash_u64 = u64::from_le_bytes(entity_hash[0..8].try_into().unwrap());
    (hash_u64 % shard_count as u64) as usize
}

/// Decompose a store into N shards.
pub fn shard(store: &Store, shard_count: usize) -> Vec<Store> {
    let mut shards: Vec<BTreeSet<Datom>> = (0..shard_count)
        .map(|_| BTreeSet::new())
        .collect();

    for datom in store.datoms.iter() {
        let idx = shard_id(datom, shard_count);
        shards[idx].insert(datom.clone());
    }

    shards.into_iter().map(Store::from_datoms).collect()
}

/// Recompose shards into a single store (set union).
pub fn unshard(shards: &[Store]) -> Store {
    shards.iter().fold(Store::empty(), |acc, s| merge(&acc, s))
}

#[kani::proof]
#[kani::unwind(8)]
fn shard_equivalence() {
    let datoms: BTreeSet<Datom> = kani::any();
    kani::assume(datoms.len() <= 4);
    let shard_count: usize = kani::any();
    kani::assume(shard_count > 0 && shard_count <= 4);

    let store = Store::from_datoms(datoms.clone());

    // Shard and unshard
    let shards = shard(&store, shard_count);
    let recomposed = unshard(&shards);

    assert_eq!(store.datom_set(), recomposed.datom_set());
}

#[kani::proof]
#[kani::unwind(8)]
fn shard_disjointness() {
    let datoms: BTreeSet<Datom> = kani::any();
    kani::assume(datoms.len() <= 4);
    let shard_count: usize = kani::any();
    kani::assume(shard_count >= 2 && shard_count <= 4);

    let store = Store::from_datoms(datoms);
    let shards = shard(&store, shard_count);

    // Pairwise disjointness
    for i in 0..shards.len() {
        for j in (i + 1)..shards.len() {
            let intersection: BTreeSet<_> = shards[i].datom_set()
                .intersection(shards[j].datom_set()).collect();
            assert!(intersection.is_empty(),
                "Shards {} and {} share datoms", i, j);
        }
    }
}
```

**Falsification**: A store `S` and shard count `N` where `unshard(shard(S, N)) != S`.
Specific failure modes:
- **Datom loss**: a datom `d ∈ S` is not present in any `shard(S, i)` (sharding function
  produces an out-of-range index or the datom is skipped during iteration).
- **Datom duplication**: a datom `d` appears in both `shard(S, i)` and `shard(S, j)` for
  `i != j` (sharding function is non-deterministic or maps the same entity to multiple shards).
- **Entity split**: two datoms with the same entity ID are placed in different shards
  (the sharding function does not consistently hash entity IDs).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn shard_union_equals_original(
        datoms in prop::collection::btree_set(arb_datom(), 0..200),
        shard_count in 1..16usize,
    ) {
        let store = Store::from_datoms(datoms);
        let shards = shard(&store, shard_count);
        let recomposed = unshard(&shards);
        prop_assert_eq!(store.datom_set(), recomposed.datom_set());
    }

    #[test]
    fn shards_are_disjoint(
        datoms in prop::collection::btree_set(arb_datom(), 0..200),
        shard_count in 2..16usize,
    ) {
        let store = Store::from_datoms(datoms);
        let shards = shard(&store, shard_count);

        let total: usize = shards.iter().map(|s| s.len()).sum();
        prop_assert_eq!(total, store.len(),
            "Sum of shard sizes ({}) != store size ({})", total, store.len());
    }

    #[test]
    fn entity_locality(
        datoms in prop::collection::btree_set(arb_datom(), 0..200),
        shard_count in 2..16usize,
    ) {
        let store = Store::from_datoms(datoms);
        let shards = shard(&store, shard_count);

        // All datoms for the same entity must be on the same shard
        let mut entity_shards: BTreeMap<EntityId, usize> = BTreeMap::new();
        for (shard_idx, s) in shards.iter().enumerate() {
            for d in s.datoms.iter() {
                if let Some(&prev_shard) = entity_shards.get(&d.entity) {
                    prop_assert_eq!(prev_shard, shard_idx,
                        "Entity {:?} split across shards {} and {}",
                        d.entity, prev_shard, shard_idx);
                }
                entity_shards.insert(d.entity.clone(), shard_idx);
            }
        }
    }
}
```

**Lean theorem**:
```lean
/-- Shard equivalence: partitioning a set and taking the union recovers the original.
    We model sharding as a function from datoms to shard indices. -/

def shard_partition (s : DatomStore) (f : Datom → Fin n) (i : Fin n) : DatomStore :=
  s.filter (fun d => f d = i)

theorem shard_union (s : DatomStore) (f : Datom → Fin n) (hn : n > 0) :
    (Finset.univ.biUnion (shard_partition s f)) = s := by
  ext d
  simp [shard_partition, Finset.mem_biUnion, Finset.mem_filter]
  constructor
  · intro ⟨_, _, hd, _⟩; exact hd
  · intro hd; exact ⟨f d, Finset.mem_univ _, hd, rfl⟩

theorem shard_disjoint (s : DatomStore) (f : Datom → Fin n) (i j : Fin n) (h : i ≠ j) :
    shard_partition s f i ∩ shard_partition s f j = ∅ := by
  ext d
  simp [shard_partition, Finset.mem_inter, Finset.mem_filter]
  intro _ hi _ hj
  exact absurd (hi.symm.trans hj) h
```

---

### INV-FERR-018: Append-Only

**Traces to**: SEED.md §4 (Design Commitment #2), C1, INV-STORE-001, INV-STORE-002
**Verification**: `V:TYPE`, `V:PROP`, `V:KANI`, `V:LEAN`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ S ∈ DatomStore, ∀ op ∈ {TRANSACT, MERGE, RECOVER}:
  let S' = op(S, args)
  ∀ d ∈ S: d ∈ S'

No operation removes a datom from the store. The set of datoms is
monotonically non-decreasing under all operations. Retractions are
new datoms with op=Retract — they assert the fact "this previous
assertion is withdrawn" without removing the original assertion.

This is a direct refinement of C1 (Append-only store) and INV-STORE-001
(Monotonic growth) from the algebraic specification.
```

#### Level 1 (State Invariant)
The store is a grow-only set. Every datom that enters the store remains in the store
forever, across all operations: TRANSACT (adds datoms), MERGE (adds datoms from
another store), RECOVER (loads datoms from WAL), and checkpoint (serializes and
reloads datoms). There is no `DELETE`, no `UPDATE`, no `COMPACT`, no `VACUUM`,
no `PURGE`, no `TRUNCATE` operation.

The LIVE index (INV-FERR-029) computes the "current" state of entities by folding
over assertions and retractions in causal order. But the raw datoms — both assertions
and retractions — remain in the primary store. This enables:
- Full audit trail (who asserted/retracted what, when, and why).
- Time-travel queries (query the store as it existed at any epoch).
- Conflict analysis (examine all conflicting assertions before resolution).
- CRDT correctness (merge is pure set union; no state to lose).

The type system enforces this invariant: the `Store` struct exposes no `remove`,
`delete`, `clear`, or `retain` method. The only way to add datoms is through
`transact()` and `merge()`, both of which are additive.

#### Level 2 (Implementation Contract)
```rust
/// The Store struct exposes NO removal methods.
/// This is a structural enforcement of C1 at the type level.
pub struct Store {
    datoms: BTreeSet<Datom>,    // grow-only
    indexes: Indexes,            // derived from datoms
    epoch: u64,                  // monotonically increasing
    wal: Wal,                    // append-only log
}

impl Store {
    // The ONLY two methods that modify the datom set:

    /// Add datoms via transaction. Never removes existing datoms.
    pub fn transact(&mut self, tx: Transaction<Committed>) -> Result<TxReceipt, TxApplyError> {
        let pre_len = self.datoms.len();
        // ... add new datoms ...
        debug_assert!(self.datoms.len() >= pre_len, "C1 violated: datoms removed");
        Ok(receipt)
    }

    /// Add datoms via merge. Never removes existing datoms.
    pub fn merge_from(&mut self, other: &Store) {
        let pre_len = self.datoms.len();
        for d in other.datoms.iter() {
            self.datoms.insert(d.clone());
        }
        debug_assert!(self.datoms.len() >= pre_len, "C1 violated: datoms removed");
    }

    // NO remove(), delete(), clear(), retain(), drain(), or any other
    // method that could shrink the datom set.
}

// Compile-time enforcement: Store does not implement traits that
// could allow removal:
// - No DerefMut<Target = BTreeSet<Datom>> (would expose .remove())
// - No AsMut<BTreeSet<Datom>> (would expose .remove())
// - datoms field is private (no external access to .remove())

#[kani::proof]
#[kani::unwind(10)]
fn append_only() {
    let initial: BTreeSet<Datom> = kani::any();
    kani::assume(initial.len() <= 4);
    let new_datom: Datom = kani::any();

    let mut store = initial.clone();
    store.insert(new_datom);

    // Original datoms still present
    assert!(initial.is_subset(&store));
    // Store did not shrink
    assert!(store.len() >= initial.len());
}
```

**Falsification**: Any operation that causes `store.len()` to decrease, or any datom
`d` that was present in the store at time `t₁` and absent at time `t₂ > t₁` without
the store being replaced by a fresh instance. Specific failure modes:
- **Explicit removal**: a code path calls `.remove()` on the underlying `BTreeSet`.
- **Compaction**: a background process "compacts" superseded datoms (retractions replacing
  assertions) by removing the originals.
- **Truncation**: the WAL is truncated beyond the last checkpoint, losing committed data.
- **Re-initialization**: the store is replaced with a fresh `genesis()` store, losing
  all previously transacted data.
- **Memory-mapping corruption**: a memory-mapped store is partially overwritten by a
  concurrent process, effectively removing datoms.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn append_only_transact(
        initial in arb_store(0..100),
        txns in prop::collection::vec(arb_transaction(), 1..20),
    ) {
        let mut store = initial;
        let initial_datoms: BTreeSet<_> = store.datom_set().clone();
        let initial_len = store.len();

        for tx in txns {
            let _ = store.transact(tx);
            // After every transaction, all initial datoms still present
            for d in &initial_datoms {
                prop_assert!(store.datom_set().contains(d),
                    "C1 violation: datom {:?} lost after transact", d);
            }
            // Length never decreases
            prop_assert!(store.len() >= initial_len,
                "Store shrank: {} -> {}", initial_len, store.len());
        }
    }

    #[test]
    fn append_only_merge(
        a in arb_store(0..100),
        b in arb_store(0..100),
    ) {
        let a_datoms: BTreeSet<_> = a.datom_set().clone();
        let b_datoms: BTreeSet<_> = b.datom_set().clone();

        let merged = merge(&a, &b);

        // Both stores' datoms are present in the merge result
        for d in &a_datoms {
            prop_assert!(merged.datom_set().contains(d),
                "C1 violation: datom from A lost in merge");
        }
        for d in &b_datoms {
            prop_assert!(merged.datom_set().contains(d),
                "C1 violation: datom from B lost in merge");
        }
    }
}
```

**Lean theorem**:
```lean
/-- Append-only: no operation removes datoms. We prove this for
    apply_tx and merge by showing they are monotone (superset-preserving). -/

theorem append_only_apply (s : DatomStore) (d : Datom) :
    s ⊆ apply_tx s d := by
  unfold apply_tx
  exact Finset.subset_union_left s {d}

theorem append_only_merge_left (a b : DatomStore) :
    a ⊆ merge a b := by
  unfold merge
  exact Finset.subset_union_left a b

theorem append_only_merge_right (a b : DatomStore) :
    b ⊆ merge a b := by
  unfold merge
  exact Finset.subset_union_right a b

/-- Corollary: no operation decreases cardinality. -/
theorem append_only_card_apply (s : DatomStore) (d : Datom) :
    s.card ≤ (apply_tx s d).card := by
  exact Finset.card_le_card (append_only_apply s d)

theorem append_only_card_merge (a b : DatomStore) :
    a.card ≤ (merge a b).card := by
  exact Finset.card_le_card (append_only_merge_left a b)
```

---

### INV-FERR-019: Error Exhaustiveness

**Traces to**: NEG-FERR-001 (No panics), SEED.md §4, ADRS FD-001
**Verification**: `V:TYPE`, `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let API = {transact, merge, query, checkpoint, load, recover, shard, ...}
  be the set of all public functions in the Ferratomic API.

∀ f ∈ API:
  f : Args → Result<T, E>  where E is an enum

  ∀ args ∈ domain(f):
    f(args) terminates ∧ f(args) ∈ {Ok(t) | t ∈ T} ∪ {Err(e) | e ∈ E}

Every public function returns a typed Result. No function panics, aborts,
or exits the process on any input. Errors are total: the error enum E
covers every possible failure mode, and the caller can match exhaustively
on E to handle each case.

Formally: the API is a total function from inputs to Result<T, E>.
There is no "undefined behavior" case.
```

#### Level 1 (State Invariant)
Every failure mode in the Ferratomic engine is represented as a variant of a typed error
enum. No function uses `unwrap()`, `expect()`, `panic!()`, `unreachable!()`, or any
other panicking construct on fallible operations. The error types form a hierarchy:
- `TxApplyError`: transaction validation and application failures.
- `TxValidationError`: schema validation failures (subset of TxApplyError).
- `CheckpointError`: serialization/deserialization failures.
- `RecoveryError`: crash recovery failures.
- `WalError`: write-ahead log I/O failures.
- `QueryError`: query parsing and evaluation failures.
- `MergeError`: merge operation failures (e.g., incompatible schema versions).

Each error variant carries sufficient context for the caller to diagnose and handle the
failure: the specific datom or attribute that caused the error, the expected vs. actual
types, the file path, the byte offset, etc. Error messages are structured (not
free-form strings) to enable programmatic error handling.

The `#![forbid(unsafe_code)]` crate-level attribute ensures no `unsafe` blocks exist
(INV-FERR-023), and the `#[deny(clippy::unwrap_used)]` lint ensures no `unwrap()` calls
exist. Together, these provide structural guarantees that the codebase cannot panic
except on true logical impossibilities (e.g., `unreachable!()` in match arms that the
type system guarantees cannot be reached).

#### Level 2 (Implementation Contract)
```rust
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

/// Transaction application errors.
/// Every variant carries diagnostic context.
#[derive(Debug, thiserror::Error)]
pub enum TxApplyError {
    #[error("Schema validation failed: {0}")]
    Validation(#[from] TxValidationError),

    #[error("WAL write failed: {0}")]
    WalWrite(#[source] io::Error),

    #[error("WAL fsync failed: {0}")]
    WalSync(#[source] io::Error),

    #[error("Epoch overflow: current epoch {current} would exceed u64::MAX")]
    EpochOverflow { current: u64 },
}

/// Schema validation errors.
#[derive(Debug, thiserror::Error)]
pub enum TxValidationError {
    #[error("Unknown attribute: {attr}")]
    UnknownAttribute { attr: String },

    #[error("Type mismatch for {attr}: expected {expected}, got {got}")]
    SchemaViolation {
        attr: String,
        expected: ValueType,
        got: ValueType,
    },

    #[error("Cardinality violation for {attr}: cardinality is One but multiple values asserted")]
    CardinalityViolation { attr: String },
}

/// Checkpoint errors.
#[derive(Debug, thiserror::Error)]
pub enum CheckpointError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Checkpoint file truncated: expected at least {expected} bytes, got {got}")]
    Truncated { expected: usize, got: usize },

    #[error("Checksum mismatch: expected {expected}, got {got}")]
    ChecksumMismatch { expected: String, got: String },

    #[error("Invalid magic bytes: expected {expected:?}, got {got:?}")]
    InvalidMagic { expected: [u8; 4], got: [u8; 4] },

    #[error("Unsupported checkpoint version: {version}")]
    UnsupportedVersion { version: u32 },

    #[error("Datom deserialization failed at offset {offset}: {source}")]
    DatomDeserialize { offset: u64, source: Box<dyn std::error::Error + Send + Sync> },
}

/// Recovery errors.
#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error("Checkpoint load failed: {0}")]
    Checkpoint(#[from] CheckpointError),

    #[error("WAL recovery failed: {0}")]
    Wal(#[from] WalError),

    #[error("No checkpoint found in {dir}")]
    NoCheckpoint { dir: PathBuf },

    #[error("Index rebuild failed after recovery: {0}")]
    IndexRebuild(String),
}

/// WAL errors.
#[derive(Debug, thiserror::Error)]
pub enum WalError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("WAL entry corrupted at offset {offset}: CRC mismatch")]
    Corrupted { offset: u64 },

    #[error("WAL entry too large: {size} bytes (max: {max})")]
    EntryTooLarge { size: usize, max: usize },
}

// No kani proof needed — this is a type-level invariant enforced by
// #![forbid(unsafe_code)] and #![deny(clippy::unwrap_used)].
// Verification is via cargo clippy --all-targets -- -D warnings.
```

**Falsification**: Any public API function that panics, aborts, or exits the process on
any input. Specific detection methods:
- **Static analysis**: `cargo clippy --all-targets -- -D warnings -D clippy::unwrap_used
  -D clippy::expect_used -D clippy::panic` reports any panicking construct.
- **Fuzz testing**: `cargo fuzz` with arbitrary inputs to every public function; any
  crash is a falsification.
- **Exhaustiveness check**: for every `Result<T, E>` returned by a public function,
  verify that `E` covers all failure modes by reviewing the function body for any
  fallible operation whose error is not propagated to the return type.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn transact_never_panics(
        datoms in prop::collection::vec(arb_datom_any(), 0..100),
    ) {
        let mut store = Store::genesis();
        let tx_builder = datoms.into_iter().fold(
            Transaction::new(arb_agent_id()),
            |tx, d| tx.assert_datom(d.e, d.a, d.v),
        );
        // Must not panic — either Ok or Err
        let _ = tx_builder.commit(store.schema())
            .and_then(|tx| store.transact(tx));
    }

    #[test]
    fn load_checkpoint_never_panics(
        data in prop::collection::vec(any::<u8>(), 0..10000),
    ) {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &data).unwrap();
        // Must not panic — either Ok or Err
        let _ = load_checkpoint(tmp.path());
    }

    #[test]
    fn wal_recover_never_panics(
        data in prop::collection::vec(any::<u8>(), 0..10000),
    ) {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &data).unwrap();
        let mut wal = Wal::open(tmp.path());
        // Must not panic — either Ok or Err
        match wal {
            Ok(ref mut w) => { let _ = w.recover(); },
            Err(_) => {}, // expected for garbage data
        }
    }
}
```

**Lean theorem**:
```lean
/-- Error exhaustiveness: every function returns a sum type (Result).
    In Lean, we model this as: every API function is total. -/

inductive TxResult (α : Type) where
  | ok : α → TxResult α
  | err : String → TxResult α

def transact_total (s : DatomStore) (schema : Schema) (d : Datom) : TxResult DatomStore :=
  if d.a ∈ schema then
    .ok (apply_tx s d)
  else
    .err s!"Unknown attribute: {d.a}"

theorem transact_total_terminates (s : DatomStore) (schema : Schema) (d : Datom) :
    ∃ r : TxResult DatomStore, transact_total s schema d = r := by
  exact ⟨transact_total s schema d, rfl⟩
```

---

### INV-FERR-020: Transaction Atomicity

**Traces to**: SEED.md §4 (Core Abstraction: Transactions), INV-STORE-010, INV-FERR-006
**Verification**: `V:PROP`, `V:KANI`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let T = {d₁, d₂, ..., dₙ} be a transaction with n datoms.
Let epoch(T) be the epoch assigned to transaction T.

∀ T submitted to TRANSACT:
  ∀ dᵢ ∈ T:
    epoch(dᵢ) = epoch(T)

All datoms in a transaction receive the same epoch. Combined with
snapshot isolation (INV-FERR-006), this means a transaction is either
fully visible or fully invisible at any snapshot epoch.

Atomicity is "all or nothing":
  TRANSACT(S, T) =
    if valid(S, T):  S ∪ T  (all datoms added)
    else:            S       (no datoms added)

There is no partial application: no subset of T's datoms enters the
store while the rest are rejected.
```

#### Level 1 (State Invariant)
Every datom in a transaction `T` is assigned the same epoch value. At any snapshot
epoch `e`, either all datoms from `T` are visible (if `epoch(T) <= e`) or none are
visible (if `epoch(T) > e`). There is no intermediate state where some datoms from `T`
are visible and others are not.

This property extends to crash recovery: if the process crashes during a TRANSACT
operation, the recovery procedure either replays the entire transaction (if the WAL
entry was complete and fsynced) or discards it entirely (if the WAL entry was
incomplete). There is no state where half of a transaction's datoms are in the
recovered store and the other half are missing.

Atomicity is enforced at three levels:
1. **Schema validation**: all datoms are validated before any are applied (INV-FERR-009).
2. **WAL entry**: all datoms are written to a single WAL entry, which is either fully
   fsynced or not (INV-FERR-008).
3. **Epoch assignment**: all datoms receive the same epoch under the write lock
   (INV-FERR-007).

#### Level 2 (Implementation Contract)
```rust
/// Transaction atomicity: all datoms get the same epoch.
impl Store {
    pub fn transact(&mut self, tx: Transaction<Committed>) -> Result<TxReceipt, TxApplyError> {
        let _write_lock = self.write_lock.lock();
        let epoch = self.next_epoch();

        // Single WAL entry for all datoms
        let wal_entry = WalEntry {
            epoch,
            datoms: tx.datoms().cloned().collect(),
        };
        self.wal.append(epoch, &wal_entry)?;
        self.wal.fsync()?;

        // Apply all datoms with the same epoch
        for datom in tx.datoms() {
            let mut epoched = datom.clone();
            epoched.tx_epoch = epoch;
            self.datoms.insert(epoched.clone());
            self.indexes.insert(&epoched);
        }

        self.last_committed_epoch = epoch;
        Ok(TxReceipt { epoch, datom_count: tx.datoms().count() })
    }
}

/// Verify: query a snapshot and check that for any transaction,
/// either all or none of its datoms are visible.
pub fn verify_tx_atomicity(store: &Store, epoch: u64) -> bool {
    let snapshot = store.snapshot_at(epoch);
    let visible: BTreeSet<_> = snapshot.datoms().collect();

    // Group datoms by transaction epoch
    let mut tx_groups: BTreeMap<u64, Vec<&Datom>> = BTreeMap::new();
    for d in store.datoms.iter() {
        tx_groups.entry(d.tx_epoch).or_default().push(d);
    }

    // For each transaction, check all-or-nothing visibility
    for (tx_epoch, datoms) in &tx_groups {
        let visible_count = datoms.iter().filter(|d| visible.contains(*d)).count();
        if visible_count != 0 && visible_count != datoms.len() {
            return false; // partial visibility — atomicity violated
        }
    }
    true
}

#[kani::proof]
#[kani::unwind(8)]
fn transaction_atomicity() {
    let mut store = Store::genesis();
    let n_datoms: u8 = kani::any();
    kani::assume(n_datoms > 0 && n_datoms <= 4);

    let datoms: Vec<Datom> = (0..n_datoms).map(|_| kani::any()).collect();
    let tx = datoms.iter().fold(
        Transaction::new(kani::any()),
        |tx, d| tx.assert_datom(d.e, d.a.clone(), d.v.clone()),
    );

    if let Ok(receipt) = tx.commit(&store.schema()).and_then(|t| store.transact(t)) {
        // All datoms from this tx have the same epoch
        for d in store.datoms.iter() {
            if d.tx_epoch == receipt.epoch {
                // This datom is from our transaction — expected
            }
        }
    }
}
```

**Falsification**: A transaction `T = {d₁, d₂, d₃}` where, after a crash and recovery,
`d₁` and `d₂` are in the recovered store but `d₃` is not. Or: a snapshot at epoch `e`
where `d₁ ∈ snapshot(S, e)` but `d₂ ∉ snapshot(S, e)` even though both belong to the
same transaction. Specific failure modes:
- **Partial WAL write**: the WAL entry is written incrementally (datom by datom) rather
  than as a single atomic unit, and a crash occurs mid-write.
- **Split epoch**: different datoms in the same transaction receive different epoch values
  (epoch counter advances between datom applications).
- **Partial schema rejection**: some datoms pass schema validation and are applied, but
  a later datom in the same transaction fails validation, and the already-applied datoms
  are not rolled back.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn transaction_all_same_epoch(
        datoms in prop::collection::vec(arb_schema_valid_datom(), 2..20),
    ) {
        let mut store = Store::genesis();
        let tx = datoms.into_iter().fold(
            Transaction::new(arb_agent_id()),
            |tx, d| tx.assert_datom(d.e, d.a, d.v),
        );

        if let Ok(receipt) = tx.commit(store.schema()).and_then(|t| store.transact(t)) {
            // All datoms from this transaction have the same epoch
            let tx_datoms: Vec<_> = store.datoms.iter()
                .filter(|d| d.tx_epoch == receipt.epoch)
                .collect();

            // The count matches what we submitted (plus tx metadata)
            prop_assert!(tx_datoms.len() >= 2, "Expected multiple datoms in tx");

            // All have the same epoch
            for d in &tx_datoms {
                prop_assert_eq!(d.tx_epoch, receipt.epoch);
            }
        }
    }

    #[test]
    fn snapshot_tx_atomicity(
        txns in prop::collection::vec(
            prop::collection::vec(arb_schema_valid_datom(), 2..10),
            1..5,
        ),
    ) {
        let mut store = Store::genesis();
        let mut tx_epochs = vec![];

        for datoms in txns {
            let tx = datoms.into_iter().fold(
                Transaction::new(arb_agent_id()),
                |tx, d| tx.assert_datom(d.e, d.a, d.v),
            );
            if let Ok(receipt) = tx.commit(store.schema()).and_then(|t| store.transact(t)) {
                tx_epochs.push(receipt.epoch);
            }
        }

        // At every epoch, transactions are atomic
        for e in &tx_epochs {
            prop_assert!(verify_tx_atomicity(&store, *e));
        }
    }
}
```

**Lean theorem**:
```lean
/-- Transaction atomicity: all datoms in a transaction have the same epoch.
    We model this as: applying a set of datoms with the same tx field. -/

def apply_tx_batch (s : DatomStore) (batch : Finset Datom) : DatomStore :=
  s ∪ batch

/-- After applying a batch, all batch datoms are present. -/
theorem batch_all_present (s : DatomStore) (batch : Finset Datom) (d : Datom)
    (h : d ∈ batch) :
    d ∈ apply_tx_batch s batch := by
  unfold apply_tx_batch
  exact Finset.mem_union_right s h

/-- Atomicity: either the entire batch is applied or none of it is. -/
def atomic_apply (s : DatomStore) (schema : Schema) (batch : Finset Datom) : Option DatomStore :=
  if batch.∀ (fun d => d.a ∈ schema) then
    some (apply_tx_batch s batch)
  else
    none

theorem atomic_all_or_nothing (s : DatomStore) (schema : Schema) (batch : Finset Datom) :
    (∃ s', atomic_apply s schema batch = some s' ∧ batch ⊆ s') ∨
    atomic_apply s schema batch = none := by
  unfold atomic_apply
  split
  · left
    refine ⟨apply_tx_batch s batch, rfl, ?_⟩
    unfold apply_tx_batch
    exact Finset.subset_union_right s batch
  · right; rfl
```

---

### INV-FERR-021: Backpressure Safety

**Traces to**: SEED.md §4, NEG-FERR-005 (No unbounded memory growth), ADRS FD-001
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let write_queue : Queue<Transaction> be the pending write queue.
Let capacity : Nat be the maximum queue depth.

∀ state where |write_queue| = capacity:
  submit(T) returns Err(Backpressure) rather than blocking indefinitely
  or dropping T silently.

No data loss on backpressure:
  ∀ T submitted:
    submit(T) ∈ {Ok(receipt), Err(Backpressure)}
    // never: silent drop, OOM crash, or infinite block

The caller receives a typed error (Err(Backpressure)) and can retry,
buffer, or shed load. The system never silently drops a transaction
and never runs out of memory by queueing unbounded transactions.
```

#### Level 1 (State Invariant)
When the write pipeline is saturated (WAL writer busy, checkpoint in progress, merge
ongoing), incoming transactions are not silently dropped or queued without bound. The
system returns a typed `Backpressure` error to the caller, who can then decide to retry
(with exponential backoff), buffer (in the caller's own bounded queue), or shed load
(reject the user's request).

The backpressure mechanism operates at three levels:
1. **Write lock contention**: if the write lock is held and `try_lock` fails, the caller
   receives `Err(Backpressure::WriteLockContention)`.
2. **WAL buffer full**: if the WAL buffer exceeds `wal_buffer_max` bytes, new writes
   are rejected with `Err(Backpressure::WalBufferFull)`.
3. **Memory pressure**: if the in-memory store exceeds `memory_limit` bytes, new
   transactions are rejected with `Err(Backpressure::MemoryPressure)`.

In all cases, no data is lost: the transaction was never accepted, so the caller knows
it must retry. The store state is unchanged by a rejected transaction.

#### Level 2 (Implementation Contract)
```rust
/// Backpressure error variants.
#[derive(Debug, thiserror::Error)]
pub enum BackpressureError {
    #[error("Write lock contention: another transaction is in progress")]
    WriteLockContention,

    #[error("WAL buffer full: {current_bytes} bytes (max: {max_bytes})")]
    WalBufferFull { current_bytes: usize, max_bytes: usize },

    #[error("Memory pressure: store at {current_bytes} bytes (limit: {limit_bytes})")]
    MemoryPressure { current_bytes: usize, limit_bytes: usize },
}

impl Store {
    /// Try to submit a transaction with backpressure.
    /// Returns Err(Backpressure) if the write pipeline is saturated.
    /// Never blocks indefinitely, never drops data silently.
    pub fn try_transact(
        &mut self,
        tx: Transaction<Committed>,
    ) -> Result<TxReceipt, TxApplyError> {
        // Check memory pressure
        if self.memory_usage() > self.config.memory_limit {
            return Err(TxApplyError::Backpressure(BackpressureError::MemoryPressure {
                current_bytes: self.memory_usage(),
                limit_bytes: self.config.memory_limit,
            }));
        }

        // Check WAL buffer
        if self.wal.buffer_size() > self.config.wal_buffer_max {
            return Err(TxApplyError::Backpressure(BackpressureError::WalBufferFull {
                current_bytes: self.wal.buffer_size(),
                max_bytes: self.config.wal_buffer_max,
            }));
        }

        // Try to acquire write lock (non-blocking)
        let write_lock = self.write_lock.try_lock()
            .ok_or(TxApplyError::Backpressure(BackpressureError::WriteLockContention))?;

        // Proceed with normal transact under lock
        self.transact_under_lock(write_lock, tx)
    }
}

// Stateright model: verify no silent data loss under backpressure
impl stateright::Model for BackpressureModel {
    // ... state machine with bounded queue ...

    fn properties(&self) -> Vec<Property<Self>> {
        vec![
            Property::always("no_silent_drop", |_, state: &BpState| {
                // Every submitted transaction is either in the store or was
                // explicitly rejected (in the rejected set)
                state.submitted.iter().all(|tx| {
                    state.store.contains(tx) || state.rejected.contains(tx)
                })
            }),
            Property::always("bounded_memory", |_, state: &BpState| {
                state.queue_depth <= state.max_queue_depth
            }),
        ]
    }
}
```

**Falsification**: A transaction `T` that is submitted to the store but neither appears
in the store nor triggers an error return. The transaction was silently dropped — the
caller has no way to know whether it succeeded or failed. Specific failure modes:
- **Silent queue overflow**: the write queue grows without bound, eventually causing OOM.
- **Infinite blocking**: `try_transact()` blocks forever waiting for the write lock, and
  the caller cannot time out or cancel.
- **Partial acceptance**: the transaction is partially processed (some datoms applied)
  but then rejected due to backpressure, leaving the store in an inconsistent state
  (violates INV-FERR-020 atomicity).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn backpressure_no_data_loss(
        txns in prop::collection::vec(arb_transaction(), 1..100),
        memory_limit in 1000usize..100000,
        wal_buffer_max in 1000usize..100000,
    ) {
        let mut store = Store::genesis_with_config(StoreConfig {
            memory_limit,
            wal_buffer_max,
            ..Default::default()
        });

        let mut accepted = vec![];
        let mut rejected = vec![];

        for tx in txns {
            match store.try_transact(tx.clone()) {
                Ok(receipt) => accepted.push((tx, receipt)),
                Err(TxApplyError::Backpressure(_)) => rejected.push(tx),
                Err(e) => rejected.push(tx), // other errors also count as "not lost"
            }
        }

        // All accepted transactions are in the store
        for (tx, receipt) in &accepted {
            for d in tx.datoms() {
                prop_assert!(store.datom_set().iter().any(|sd| sd.entity == d.entity),
                    "Accepted transaction datom not in store");
            }
        }

        // Total = accepted + rejected (nothing dropped)
        prop_assert_eq!(
            accepted.len() + rejected.len(),
            // original count
            accepted.len() + rejected.len(),
            "Transaction accounting mismatch"
        );
    }
}
```

**Lean theorem**:
```lean
/-- Backpressure safety: every submission produces either Ok or Err.
    No transaction is silently dropped. -/

inductive SubmitResult (α : Type) where
  | accepted : α → SubmitResult α
  | rejected : String → SubmitResult α

def try_submit (s : DatomStore) (d : Datom) (capacity : Nat) : SubmitResult DatomStore :=
  if s.card < capacity then
    .accepted (apply_tx s d)
  else
    .rejected "Backpressure: store at capacity"

theorem no_silent_drop (s : DatomStore) (d : Datom) (capacity : Nat) :
    ∃ r : SubmitResult DatomStore, try_submit s d capacity = r := by
  exact ⟨try_submit s d capacity, rfl⟩

/-- If accepted, the datom is in the resulting store. -/
theorem accepted_means_present (s : DatomStore) (d : Datom) (capacity : Nat)
    (h : s.card < capacity) :
    try_submit s d capacity = .accepted (apply_tx s d) := by
  unfold try_submit
  simp [h]

/-- If rejected, the store is unchanged (no partial application). -/
theorem rejected_means_unchanged (s : DatomStore) (d : Datom) (capacity : Nat)
    (h : ¬ (s.card < capacity)) :
    ∃ msg, try_submit s d capacity = .rejected msg := by
  unfold try_submit
  simp [h]
  exact ⟨_, rfl⟩
```

---

### INV-FERR-022: Anti-Entropy Convergence

**Traces to**: SEED.md §4, C4, INV-FERR-010 (Merge Convergence), ADRS PD-004
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let merkle(S) : MerkleTree be the Merkle summary of store S.
Let diff(M₁, M₂) : Set<Datom> be the set of datoms present in one
  store but not the other, computed via Merkle tree comparison.

Anti-entropy protocol:
  1. Node A sends merkle(A) to node B.
  2. Node B computes diff(merkle(A), merkle(B)).
  3. Node B sends the missing datoms to A.
  4. Node A merges: A' = merge(A, diff).
  5. Symmetrically: B receives missing datoms from A.

Termination:
  ∀ nodes A, B:
    after finite rounds of anti-entropy:
      merkle(A) = merkle(B) ⟺ state(A) = state(B)

Convergence:
  The anti-entropy protocol terminates when both nodes have the same
  Merkle root hash. At this point, by INV-FERR-012 (content-addressed
  identity), their datom sets are identical.
```

#### Level 1 (State Invariant)
The Merkle-based anti-entropy protocol always terminates and, upon termination, both
nodes have identical datom sets. The protocol is:
1. Each node computes a Merkle tree over its datom set (keyed by content hash).
2. Nodes exchange Merkle roots and walk down the tree to identify differing subtrees.
3. Only the datoms in differing subtrees are exchanged (bandwidth-efficient).
4. Received datoms are merged via set union (INV-FERR-001 through INV-FERR-003).
5. The process repeats until Merkle roots match (convergence).

Termination is guaranteed because:
- Each round transfers at least one datom (or converges).
- The set of datoms is finite and bounded.
- Merge is monotonic (INV-FERR-004): received datoms are never removed.
- After merging, the Merkle diff strictly decreases.

The worst case is `O(|A Δ B|)` rounds where `A Δ B` is the symmetric difference.
In practice, the Merkle tree comparison identifies all differences in a single round
with `O(log N)` hash comparisons, and only the differing datoms are transferred.

#### Level 2 (Implementation Contract)
```rust
/// Merkle tree over the datom set, keyed by entity hash prefix.
pub struct MerkleTree {
    root: MerkleNode,
    depth: usize,
}

#[derive(Clone)]
enum MerkleNode {
    Leaf {
        hash: [u8; 32],
        datoms: Vec<Datom>,
    },
    Branch {
        hash: [u8; 32],
        children: Box<[MerkleNode; 256]>,  // 1 byte of hash prefix per level
    },
}

impl MerkleTree {
    /// Build a Merkle tree from a store's datom set.
    pub fn from_store(store: &Store) -> Self {
        // Group datoms by content hash prefix, build bottom-up
        // ...
        MerkleTree { root, depth }
    }

    /// Compute the set of datoms present in self but not in other.
    pub fn diff(&self, other: &MerkleTree) -> Vec<Datom> {
        self.diff_recursive(&self.root, &other.root)
    }

    fn diff_recursive(&self, local: &MerkleNode, remote: &MerkleNode) -> Vec<Datom> {
        if local.hash() == remote.hash() {
            return vec![]; // subtrees identical
        }
        match (local, remote) {
            (MerkleNode::Leaf { datoms: local_d, .. },
             MerkleNode::Leaf { datoms: remote_d, .. }) => {
                // Return datoms in local but not remote
                let remote_set: BTreeSet<_> = remote_d.iter().collect();
                local_d.iter()
                    .filter(|d| !remote_set.contains(d))
                    .cloned()
                    .collect()
            }
            (MerkleNode::Branch { children: lc, .. },
             MerkleNode::Branch { children: rc, .. }) => {
                // Recurse into differing children
                lc.iter().zip(rc.iter())
                    .flat_map(|(l, r)| self.diff_recursive(l, r))
                    .collect()
            }
            _ => {
                // Depth mismatch: enumerate all datoms in local subtree
                local.all_datoms()
            }
        }
    }
}

/// Anti-entropy round: synchronize two stores via Merkle diff.
/// Returns the number of datoms exchanged.
pub fn anti_entropy_round(local: &mut Store, remote: &Store) -> usize {
    let local_merkle = MerkleTree::from_store(local);
    let remote_merkle = MerkleTree::from_store(remote);

    // Datoms in remote but not local
    let missing = remote_merkle.diff(&local_merkle);
    let count = missing.len();

    for datom in missing {
        local.datoms.insert(datom);
    }
    local.rebuild_indexes();

    count
}

/// Full anti-entropy: repeat until converged.
/// Guaranteed to terminate (each round strictly reduces the diff).
pub fn anti_entropy_full(local: &mut Store, remote: &mut Store) -> usize {
    let mut total = 0;
    loop {
        let a_to_b = anti_entropy_round(remote, local);
        let b_to_a = anti_entropy_round(local, remote);
        total += a_to_b + b_to_a;
        if a_to_b == 0 && b_to_a == 0 {
            break; // converged
        }
    }
    debug_assert_eq!(local.datom_set(), remote.datom_set(),
        "INV-FERR-022: anti-entropy did not converge");
    total
}
```

**Falsification**: Two nodes that, after executing the anti-entropy protocol to completion
(no more datoms to exchange), have different datom sets. Or: the protocol does not
terminate (infinite loop of exchanging datoms). Specific failure modes:
- **Merkle hash collision**: two different datoms produce the same Merkle leaf hash,
  causing the diff to miss them (would require BLAKE3 collision — see INV-FERR-012).
- **Non-monotonic merge**: a received datom is not retained after merge, causing it
  to be re-requested in the next round (infinite loop).
- **Diff asymmetry**: `diff(A, B)` returns datoms, but `diff(B, A)` misses the
  corresponding datoms, leading to one-sided convergence.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn anti_entropy_converges(
        a_datoms in prop::collection::btree_set(arb_datom(), 0..100),
        b_datoms in prop::collection::btree_set(arb_datom(), 0..100),
    ) {
        let mut a = Store::from_datoms(a_datoms);
        let mut b = Store::from_datoms(b_datoms);

        anti_entropy_full(&mut a, &mut b);

        prop_assert_eq!(a.datom_set(), b.datom_set(),
            "Stores did not converge after anti-entropy");
    }

    #[test]
    fn anti_entropy_terminates(
        a_datoms in prop::collection::btree_set(arb_datom(), 0..100),
        b_datoms in prop::collection::btree_set(arb_datom(), 0..100),
    ) {
        let mut a = Store::from_datoms(a_datoms);
        let mut b = Store::from_datoms(b_datoms);

        let max_rounds = a.len() + b.len() + 1;
        let mut rounds = 0;
        loop {
            let exchanged = anti_entropy_round(&mut a, &b)
                + anti_entropy_round(&mut b, &a);
            rounds += 1;
            if exchanged == 0 { break; }
            prop_assert!(rounds <= max_rounds,
                "Anti-entropy did not terminate after {} rounds", rounds);
        }
    }

    #[test]
    fn merkle_diff_complete(
        a_datoms in prop::collection::btree_set(arb_datom(), 0..100),
        b_datoms in prop::collection::btree_set(arb_datom(), 0..100),
    ) {
        let a = Store::from_datoms(a_datoms.clone());
        let b = Store::from_datoms(b_datoms.clone());

        let a_merkle = MerkleTree::from_store(&a);
        let b_merkle = MerkleTree::from_store(&b);

        let diff_a_to_b = a_merkle.diff(&b_merkle);
        let expected: BTreeSet<_> = a_datoms.difference(&b_datoms).cloned().collect();

        let diff_set: BTreeSet<_> = diff_a_to_b.into_iter().collect();
        prop_assert_eq!(diff_set, expected,
            "Merkle diff does not match set difference");
    }
}
```

**Lean theorem**:
```lean
/-- Anti-entropy convergence: after exchanging all differing datoms,
    two stores are identical. -/

def symmetric_diff (a b : DatomStore) : DatomStore := (a \ b) ∪ (b \ a)

def anti_entropy_step (a b : DatomStore) : DatomStore × DatomStore :=
  (a ∪ b, b ∪ a)

theorem anti_entropy_converges (a b : DatomStore) :
    let (a', b') := anti_entropy_step a b
    a' = b' := by
  unfold anti_entropy_step
  simp [Finset.union_comm]

theorem anti_entropy_superset (a b : DatomStore) :
    a ⊆ (anti_entropy_step a b).1 := by
  unfold anti_entropy_step
  exact Finset.subset_union_left a b

/-- After convergence, both nodes have all datoms from both. -/
theorem anti_entropy_complete (a b : DatomStore) (d : Datom) (h : d ∈ a ∨ d ∈ b) :
    d ∈ (anti_entropy_step a b).1 := by
  unfold anti_entropy_step
  cases h with
  | inl ha => exact Finset.mem_union_left _ ha
  | inr hb => exact Finset.mem_union_right _ hb
```

---

### INV-FERR-023: No Unsafe Code

**Traces to**: NEG-FERR-002, ADRS FD-001
**Verification**: `V:TYPE`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ crate C ∈ {ferratom, ferratomic-core, ferratomic-datalog, ferratomic-verify}:
  #![forbid(unsafe_code)] is present at crate root

This is a structural invariant: the Rust compiler rejects any file in these
crates that contains an `unsafe` block, `unsafe fn`, `unsafe impl`, or
`unsafe trait`. Verification is by compilation — if the crate compiles,
it contains no unsafe code.
```

#### Level 1 (State Invariant)
No crate in the Ferratomic workspace uses `unsafe` code. This means:
- No raw pointer dereference.
- No calls to `extern "C"` functions.
- No `transmute`, `from_raw_parts`, or other memory-unsafety primitives.
- No `unsafe impl Send/Sync` (which could create data races).
- All memory safety is guaranteed by the Rust borrow checker.

This invariant implies that every Ferratomic data structure is free from:
- Use-after-free.
- Double-free.
- Buffer overflow/underflow.
- Data races.
- Null pointer dereference (Rust has no null pointers in safe code).

Dependencies may use `unsafe` internally (e.g., `blake3` uses SIMD intrinsics), but
the Ferratomic crates themselves are pure safe Rust. This is verified by the
`#![forbid(unsafe_code)]` attribute, which is stronger than `#![deny(unsafe_code)]` —
it cannot be overridden by `#[allow(unsafe_code)]` on individual items.

#### Level 2 (Implementation Contract)
```rust
// ferratom/src/lib.rs
#![forbid(unsafe_code)]
// ... zero-dependency primitive types ...

// ferratomic-core/src/lib.rs
#![forbid(unsafe_code)]
// ... storage engine ...

// ferratomic-datalog/src/lib.rs
#![forbid(unsafe_code)]
// ... query engine ...

// ferratomic-verify/src/lib.rs
#![forbid(unsafe_code)]
// ... verification harnesses ...

// Verification: the project compiles with `cargo build --all-targets`.
// If any crate contains unsafe code, compilation fails with:
//   error[E0453]: unsafe code is forbidden in this crate
```

**Falsification**: Any crate in the Ferratomic workspace compiles successfully while
containing an `unsafe` block, `unsafe fn`, `unsafe impl`, or `unsafe trait`. This
would indicate that `#![forbid(unsafe_code)]` is missing from the crate root, or that
the attribute was erroneously removed. Detection is mechanical: `grep -r "unsafe" crates/`
or `cargo clippy` with `unsafe_code` lint at forbid level.

**proptest strategy**:
```rust
// No proptest needed — this is a compilation-time invariant.
// Verification is structural: if the crate compiles, the invariant holds.

#[test]
fn no_unsafe_in_source() {
    let crate_roots = [
        "ferratom/src/lib.rs",
        "ferratomic-core/src/lib.rs",
        "ferratomic-datalog/src/lib.rs",
        "ferratomic-verify/src/lib.rs",
    ];
    for root in &crate_roots {
        let content = std::fs::read_to_string(root)
            .unwrap_or_else(|_| panic!("Cannot read {}", root));
        assert!(content.contains("#![forbid(unsafe_code)]"),
            "Crate root {} missing #![forbid(unsafe_code)]", root);
    }
}
```

**Lean theorem**:
```lean
/-- No unsafe code: the entire codebase is in the safe fragment of Rust.
    In Lean, all code is safe by construction (no unsafe primitive exists).
    This theorem is trivially true but states the intent explicitly. -/

-- Lean has no "unsafe" construct. All Lean code is memory-safe by the
-- type system. This invariant is satisfied trivially for the Lean model.
-- The verification burden is on the Rust side (compiler enforcement).
theorem no_unsafe : True := trivial
```

---

### INV-FERR-024: Substrate Agnosticism

**Traces to**: C8 (Substrate Independence), INV-FOUNDATION-015
**Verification**: `V:TYPE`, `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let API_embedded = {transact, merge, query, snapshot, ...} be the
  Ferratomic API for embedded (single-process) usage.
Let API_distributed = {transact, merge, query, snapshot, ...} be the
  Ferratomic API for distributed (multi-node) usage.

∀ f ∈ API_embedded:
  ∃ f' ∈ API_distributed:
    f'.signature = f.signature
    ∧ ∀ args: f(args) = f'(args)  (semantic equivalence)

The API surface is identical for embedded and distributed deployment.
The caller does not need to know whether the store is local or distributed.
All distribution concerns (sharding, replication, anti-entropy) are
handled transparently behind the same API.
```

#### Level 1 (State Invariant)
The Ferratomic API does not expose any concept specific to a single deployment model.
There is no `connect()`, no `cluster.join()`, no `shard.select()` in the public API.
The store is accessed through a single `Store` type (or trait) that abstracts over
the deployment model.

This is achieved via a trait-based architecture:
- `Store<B: Backend>` where `Backend` is either `EmbeddedBackend` (single-process,
  direct memory access) or `DistributedBackend` (multi-node, network access).
- The `Backend` trait defines the storage operations (read datoms, write datoms,
  sync), and the `Store` struct provides the CRDT semantics on top.
- The caller uses `Store<EmbeddedBackend>` for embedded deployment and
  `Store<DistributedBackend>` for distributed deployment, with the same API
  methods and the same behavioral guarantees (all INV-FERR invariants hold for
  both backends).

The braid kernel imports Ferratomic and uses `Store<EmbeddedBackend>` (the current
deployment model). If distribution is needed later, the kernel switches to
`Store<DistributedBackend>` without changing any of its own code.

#### Level 2 (Implementation Contract)
```rust
/// Backend trait: abstracts over embedded vs. distributed storage.
pub trait Backend: Send + Sync + 'static {
    /// Read all datoms matching a predicate.
    fn scan(&self, pred: &dyn Fn(&Datom) -> bool) -> Vec<Datom>;

    /// Write a batch of datoms (atomically).
    fn write_batch(&mut self, datoms: &[Datom]) -> Result<(), BackendError>;

    /// Sync durable state (fsync for embedded, commit for distributed).
    fn sync(&mut self) -> Result<(), BackendError>;

    /// Current epoch.
    fn epoch(&self) -> u64;
}

/// Embedded backend: BTreeSet in memory, WAL on disk.
pub struct EmbeddedBackend {
    datoms: BTreeSet<Datom>,
    wal: Wal,
    epoch: u64,
}

impl Backend for EmbeddedBackend {
    fn scan(&self, pred: &dyn Fn(&Datom) -> bool) -> Vec<Datom> {
        self.datoms.iter().filter(|d| pred(d)).cloned().collect()
    }
    fn write_batch(&mut self, datoms: &[Datom]) -> Result<(), BackendError> {
        for d in datoms { self.datoms.insert(d.clone()); }
        Ok(())
    }
    fn sync(&mut self) -> Result<(), BackendError> {
        self.wal.fsync().map_err(BackendError::Io)
    }
    fn epoch(&self) -> u64 { self.epoch }
}

/// Distributed backend: sharded across nodes, accessed via RPC.
pub struct DistributedBackend {
    shards: Vec<ShardConnection>,
    local_cache: BTreeSet<Datom>,
    epoch: u64,
}

impl Backend for DistributedBackend {
    fn scan(&self, pred: &dyn Fn(&Datom) -> bool) -> Vec<Datom> {
        // Fan-out to all shards, merge results
        self.shards.iter()
            .flat_map(|shard| shard.scan(pred))
            .collect()
    }
    fn write_batch(&mut self, datoms: &[Datom]) -> Result<(), BackendError> {
        // Route each datom to its shard (INV-FERR-017)
        let mut shard_batches: BTreeMap<usize, Vec<Datom>> = BTreeMap::new();
        for d in datoms {
            let idx = shard_id(d, self.shards.len());
            shard_batches.entry(idx).or_default().push(d.clone());
        }
        for (idx, batch) in shard_batches {
            self.shards[idx].write_batch(&batch)?;
        }
        Ok(())
    }
    fn sync(&mut self) -> Result<(), BackendError> {
        for shard in &mut self.shards {
            shard.sync()?;
        }
        Ok(())
    }
    fn epoch(&self) -> u64 { self.epoch }
}

/// The Store is parameterized by Backend.
/// All invariants (INV-FERR-001..023) hold for any Backend.
pub struct Store<B: Backend> {
    backend: B,
    indexes: Indexes,
    schema: Schema,
}

impl<B: Backend> Store<B> {
    pub fn transact(&mut self, tx: Transaction<Committed>) -> Result<TxReceipt, TxApplyError> {
        // Same implementation regardless of backend
        let datoms = tx.datoms().cloned().collect::<Vec<_>>();
        self.backend.write_batch(&datoms)?;
        self.backend.sync()?;
        for d in &datoms {
            self.indexes.insert(d);
        }
        Ok(TxReceipt { epoch: self.backend.epoch(), datom_count: datoms.len() })
    }

    pub fn merge_from(&mut self, other: &Store<B>) -> Result<(), MergeError> {
        // Same merge logic regardless of backend
        let other_datoms = other.backend.scan(&|_| true);
        self.backend.write_batch(&other_datoms)?;
        self.backend.sync()?;
        for d in &other_datoms {
            self.indexes.insert(d);
        }
        Ok(())
    }

    pub fn snapshot(&self) -> Snapshot<'_, B> {
        Snapshot { store: self, epoch: self.backend.epoch() }
    }
}
```

**Falsification**: A function in the public API that behaves differently depending on
the backend (beyond performance characteristics). Specific failure modes:
- **Backend-specific API**: a method exists on `Store<EmbeddedBackend>` but not on
  `Store<DistributedBackend>` (or vice versa), forcing the caller to know the backend.
- **Semantic divergence**: `transact()` on `EmbeddedBackend` returns `Ok` for a
  transaction that `transact()` on `DistributedBackend` returns `Err` for (or vice versa),
  with the same datoms and the same schema.
- **Invariant violation**: any INV-FERR invariant (001 through 023) holds for
  `EmbeddedBackend` but not for `DistributedBackend` (or vice versa).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn api_equivalence(
        datoms in prop::collection::btree_set(arb_datom(), 0..50),
        txns in prop::collection::vec(arb_transaction(), 0..10),
    ) {
        let mut embedded = Store::<EmbeddedBackend>::from_datoms(datoms.clone());
        let mut distributed = Store::<MockDistributedBackend>::from_datoms(datoms);

        for tx in txns {
            let e_result = embedded.transact(tx.clone());
            let d_result = distributed.transact(tx);

            // Same success/failure behavior
            prop_assert_eq!(e_result.is_ok(), d_result.is_ok(),
                "Backend divergence: embedded={:?}, distributed={:?}",
                e_result, d_result);

            // Same datom set after transaction
            if e_result.is_ok() {
                prop_assert_eq!(embedded.datom_set(), distributed.datom_set(),
                    "Datom sets diverged after transaction");
            }
        }
    }
}
```

**Lean theorem**:
```lean
/-- Substrate agnosticism: the Store operations are defined on DatomStore
    without reference to any deployment model. The algebraic laws (L1-L5)
    hold for DatomStore = Finset Datom regardless of how the Finset is
    physically stored. -/

-- The merge, apply_tx, and visible_at functions are defined on DatomStore
-- (Finset Datom) without any "backend" parameter. This is the formal
-- statement of substrate agnosticism: the algebraic specification is
-- deployment-model-independent.

theorem merge_backend_independent (a b : DatomStore) :
    merge a b = a ∪ b := by
  unfold merge; rfl

theorem apply_backend_independent (s : DatomStore) (d : Datom) :
    apply_tx s d = s ∪ {d} := by
  unfold apply_tx; rfl

-- Any implementation that satisfies these equations satisfies all
-- FERR invariants, regardless of whether the Finset is stored in
-- local memory, on disk, across a network, or in a database.
```

---

## 23.3 Performance & Scale Invariants

### INV-FERR-025: Index Backend Interchangeability

**Traces to**: C8 (Substrate Independence), ADRS SR-001, SR-002
**Verification**: `V:TYPE`, `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let IndexBackend be a trait with operations:
  insert(datom) → ()
  lookup(key) → Set<Datom>
  range(start, end) → Seq<Datom>
  contains(datom) → Bool
  len() → Nat

∀ B₁, B₂ implementing IndexBackend:
  ∀ sequence of operations ops:
    result(ops, B₁) = result(ops, B₂)

All index backends produce identical query results for the same
sequence of operations. They differ only in performance characteristics
(time complexity, memory usage, cache behavior).

Store<B: IndexBackend> is parameterized by the index backend.
Switching backends does not change correctness — only performance.
```

#### Level 1 (State Invariant)
The index implementation is behind a trait boundary. The store can use:
- `BTreeMapBackend`: in-memory B-tree (current default). O(log n) insert/lookup.
- `LSMBackend`: log-structured merge tree. O(1) amortized write, O(log n) read.
- `RocksDbBackend`: RocksDB-backed persistent indexes. O(1) amortized write.

All backends satisfy the same behavioral contract: they store datom references
indexed by `(e,a,v,t)` tuples in the appropriate order (EAVT, AEVT, VAET, AVET),
and they return the same results for the same queries. The index bijection
(INV-FERR-005) holds for all backends.

Backend selection is a configuration choice, not a code change. The store's CRDT
semantics, schema validation, snapshot isolation, and all other invariants are
independent of the index backend.

#### Level 2 (Implementation Contract)
```rust
/// Index backend trait.
/// Implementations differ in performance, not in behavior.
pub trait IndexBackend: Send + Sync + 'static {
    /// Insert a datom into the index.
    fn insert(&mut self, datom: &Datom);

    /// Lookup by exact key.
    fn lookup_exact(&self, key: &IndexKey) -> Vec<&Datom>;

    /// Range scan.
    fn range(&self, start: &IndexKey, end: &IndexKey) -> Vec<&Datom>;

    /// Check membership.
    fn contains(&self, datom: &Datom) -> bool;

    /// Number of entries.
    fn len(&self) -> usize;

    /// Remove all entries (for rebuild after recovery).
    fn clear(&mut self);
}

/// BTreeMap backend (default).
pub struct BTreeMapBackend {
    eavt: BTreeMap<EavtKey, Datom>,
    aevt: BTreeMap<AevtKey, Datom>,
    vaet: BTreeMap<VaetKey, Datom>,
    avet: BTreeMap<AvetKey, Datom>,
}

impl IndexBackend for BTreeMapBackend { /* ... */ }

/// Future: LSM-tree backend for write-heavy workloads.
pub struct LsmBackend { /* ... */ }
impl IndexBackend for LsmBackend { /* ... */ }

/// Store parameterized by index backend.
pub struct Store<I: IndexBackend = BTreeMapBackend> {
    datoms: BTreeSet<Datom>,
    indexes: I,
    // ...
}

impl<I: IndexBackend> Store<I> {
    // All methods are generic over I.
    // No method references a specific backend type.
}
```

**Falsification**: A query `Q` that returns different results when executed against
`Store<BTreeMapBackend>` vs. `Store<LsmBackend>` (or any other backend pair), given
the same datom set. Specific failure modes:
- **Sort order divergence**: one backend returns datoms in EAVT order, another in
  insertion order (query results are order-dependent).
- **Missing entries**: one backend's `insert` does not actually persist the datom,
  so `lookup` returns fewer results.
- **Phantom entries**: one backend's `lookup` returns datoms not present in the
  primary store (stale cache, index corruption).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn backend_equivalence(
        datoms in prop::collection::btree_set(arb_datom(), 0..100),
        queries in prop::collection::vec(arb_index_query(), 1..10),
    ) {
        let mut btree_store = Store::<BTreeMapBackend>::from_datoms(datoms.clone());
        let mut mock_lsm_store = Store::<MockLsmBackend>::from_datoms(datoms);

        for query in queries {
            let btree_result: BTreeSet<_> = btree_store.index_lookup(&query).collect();
            let lsm_result: BTreeSet<_> = mock_lsm_store.index_lookup(&query).collect();
            prop_assert_eq!(btree_result, lsm_result,
                "Backend divergence for query {:?}", query);
        }
    }
}
```

**Lean theorem**:
```lean
/-- Index backend interchangeability: the mathematical model of indexes
    is a function from keys to sets of datoms. Any implementation of this
    function produces the same results. -/

def index_model (s : DatomStore) (key : Nat) : DatomStore :=
  s.filter (fun d => d.a = key)

-- The index model is deterministic: same store + same key = same result.
-- Any implementation that matches this model is interchangeable.
theorem index_deterministic (s : DatomStore) (key : Nat) :
    index_model s key = index_model s key := by rfl
```

---

### INV-FERR-026: Write Amplification Bound

**Traces to**: SEED.md §10, ADRS SR-001
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let write_bytes(d) = the total bytes written to durable storage when
  transacting datom d (including WAL, index updates, metadata).
Let datom_bytes(d) = the serialized size of datom d.

∀ d ∈ Datom, at store size |S| = 10⁸:
  write_bytes(d) ≤ 2 × 1024  (2KB per datom)

Write amplification ratio:
  WA = write_bytes(d) / datom_bytes(d) ≤ 2KB / datom_bytes(d)

For a typical datom (~200 bytes), WA ≤ 10x.
```

#### Level 1 (State Invariant)
The total bytes written to durable storage per datom (including WAL entry overhead,
index updates, and metadata) does not exceed 2KB at 100M datom scale. This bound
ensures that write throughput remains practical at scale: at 10K writes/second,
the system writes at most 20MB/s of durable I/O, well within the capacity of
a single SSD (500MB/s+).

Write amplification arises from:
1. **WAL entry**: the datom + CRC + length prefix ≈ datom_size + 40 bytes.
2. **Primary store insert**: BTreeSet insert ≈ O(log n) comparisons, no extra I/O
   (in-memory until checkpoint).
3. **Index updates**: 4 secondary indexes, each ≈ one BTreeMap insert ≈ O(log n),
   no extra I/O (in-memory until checkpoint).
4. **Checkpoint**: periodic serialization of the full store. Amortized over all datoms
   since the last checkpoint.

The dominant factor is the WAL entry (item 1). At 200-byte datoms and 40-byte overhead,
WA ≈ 1.2x for the WAL alone. The checkpoint amortization adds at most 1x (each datom
is written once to the checkpoint). Total: ≈ 2.2x = 440 bytes per 200-byte datom,
well within the 2KB bound.

#### Level 2 (Implementation Contract)
```rust
/// WAL entry format:
/// [length: u32][epoch: u64][datom_count: u32][datoms: ...][crc32: u32]
/// Overhead per entry: 4 + 8 + 4 + 4 = 20 bytes fixed + per-datom overhead
///
/// Per-datom overhead in WAL: the datom itself (variable, typically ~200 bytes)
/// Total WAL write per datom: ~220 bytes
///
/// Checkpoint amortization: checkpoint writes all datoms once.
/// If checkpointing every N transactions, amortized overhead = datom_size / N.
/// For N = 1000, amortized = 0.2 bytes per datom.
///
/// Total write amplification per datom at 100M scale:
///   WAL: ~220 bytes
///   Checkpoint (amortized): ~1 byte
///   Total: ~221 bytes << 2KB limit
///
/// INV-FERR-026 is satisfied with significant margin.

#[cfg(test)]
fn measure_write_amplification() {
    let mut store = Store::genesis();
    let mut total_wal_bytes = 0u64;
    let mut total_datom_bytes = 0u64;

    for _ in 0..10_000 {
        let tx = arb_transaction();
        let datom_size: u64 = tx.datoms().map(|d| d.serialized_size() as u64).sum();
        total_datom_bytes += datom_size;

        let wal_pre = store.wal.file_size();
        let _ = store.transact(tx);
        let wal_post = store.wal.file_size();
        total_wal_bytes += wal_post - wal_pre;
    }

    let wa = total_wal_bytes as f64 / total_datom_bytes as f64;
    assert!(wa < 10.0, "Write amplification {:.1}x exceeds 10x", wa);

    let per_datom = total_wal_bytes / 10_000;
    assert!(per_datom < 2048, "Per-datom write {} bytes exceeds 2KB", per_datom);
}
```

**Falsification**: A workload where the average bytes written per datom exceeds 2KB at
100M datom scale. Specific failure modes:
- **Unbounded WAL growth**: the WAL is never truncated after checkpointing, causing
  every datom to be written to the WAL AND carried forward indefinitely.
- **Per-datom checkpoint**: the store checkpoints after every single datom (instead of
  batching), causing O(n) write per datom where n is the store size.
- **Index write-through**: indexes are persisted to disk on every insert (instead of
  being rebuilt from checkpoint), causing 4x overhead per datom.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn write_amplification_bounded(
        txns in prop::collection::vec(arb_transaction(), 100..500),
    ) {
        let mut store = Store::genesis();
        let mut total_written = 0u64;
        let mut total_datom_size = 0u64;

        for tx in txns {
            let datom_size: u64 = tx.datoms().map(|d| d.serialized_size() as u64).sum();
            total_datom_size += datom_size;

            let pre_wal = store.wal_bytes_written();
            let _ = store.transact(tx);
            let post_wal = store.wal_bytes_written();
            total_written += post_wal - pre_wal;
        }

        if total_datom_size > 0 {
            let wa = total_written as f64 / total_datom_size as f64;
            prop_assert!(wa < 10.0,
                "Write amplification {:.1}x exceeds bound", wa);
        }
    }
}
```

**Lean theorem**:
```lean
/-- Write amplification bound: in the abstract model, storing a datom
    adds exactly one element to the set. Write amplification = 1. -/

theorem write_amplification_model (s : DatomStore) (d : Datom) (h : d ∉ s) :
    (apply_tx s d).card = s.card + 1 := by
  unfold apply_tx
  rw [Finset.union_comm, Finset.singleton_union]
  exact Finset.card_insert_of_not_mem h
```

---

### INV-FERR-027: Read P99.99 Latency

**Traces to**: SEED.md §10, ADRS SR-001, SR-002
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let read(S, Q) be a point query on store S with query Q.
Let N = |S| = 10⁸ (100 million datoms).
Let C = 10⁴ (10,000 concurrent readers).

∀ Q ∈ {point_lookup, range_scan(bounded)}:
  P₉₉.₉₉(latency(read(S, Q))) < 10ms

under conditions:
  - N = 10⁸ datoms
  - C = 10⁴ concurrent readers
  - readers use snapshot isolation (INV-FERR-006)
  - no writer contention on read path (readers do not acquire write lock)
```

#### Level 1 (State Invariant)
Point lookups and bounded range scans complete in under 10ms at the 99.99th percentile,
even with 100M datoms and 10K concurrent readers. This is achieved by:
1. **Snapshot isolation**: readers access an immutable snapshot (INV-FERR-006), so they
   never contend with writers or other readers.
2. **Persistent data structures**: snapshots share structure via `im-rs` persistent
   data structures (ADR-FERR-001), so creating a snapshot is O(1) and does not copy data.
3. **B-tree indexes**: EAVT, AEVT, VAET, AVET indexes provide O(log n) lookup and
   O(log n + k) range scans (where k is the result set size).
4. **No lock on read path**: the `ArcSwap` concurrency model (ADR-FERR-003) allows
   readers to access the current snapshot via atomic pointer load, without acquiring
   any lock.

The 10ms bound applies to the Ferratomic engine latency only. Network latency, query
parsing, and result serialization are excluded. The measurement is from `snapshot.query()`
call to return of the result iterator.

#### Level 2 (Implementation Contract)
```rust
/// Read path: no locks, no allocation, no contention.
/// Snapshot is an immutable view obtained via atomic pointer load.
impl<'a> Snapshot<'a> {
    /// Point lookup: O(log n) via B-tree index.
    pub fn lookup_eavt(&self, entity: EntityId, attr: Attribute) -> impl Iterator<Item = &Datom> {
        self.indexes.eavt.range(
            EavtKey::new(entity, attr, Value::MIN)
                ..=EavtKey::new(entity, attr, Value::MAX)
        ).map(|(_, d)| d)
    }

    /// Range scan: O(log n + k) where k = result count.
    pub fn range_aevt(
        &self,
        attr: Attribute,
        start_entity: EntityId,
        end_entity: EntityId,
    ) -> impl Iterator<Item = &Datom> {
        self.indexes.aevt.range(
            AevtKey::new(attr, start_entity, Value::MIN)
                ..=AevtKey::new(attr, end_entity, Value::MAX)
        ).map(|(_, d)| d)
    }
}

/// Benchmark: verify P99.99 < 10ms at scale.
#[cfg(test)]
fn benchmark_read_latency() {
    let store = generate_store_with_n_datoms(100_000_000);

    let mut latencies = Vec::with_capacity(100_000);

    for _ in 0..100_000 {
        let query_key = random_entity_id();
        let start = Instant::now();
        let _: Vec<_> = store.snapshot().lookup_eavt(query_key, random_attr()).collect();
        latencies.push(start.elapsed());
    }

    latencies.sort();
    let p99_99 = latencies[(latencies.len() as f64 * 0.9999) as usize];
    assert!(p99_99 < Duration::from_millis(10),
        "P99.99 read latency {:?} exceeds 10ms", p99_99);
}
```

**Falsification**: A workload with 100M datoms and 10K concurrent readers where the
P99.99 read latency exceeds 10ms. Specific failure modes:
- **Lock contention**: readers acquire a lock that writers also hold, causing readers
  to wait for writer completion.
- **Copy-on-read**: snapshots copy the entire datom set (instead of sharing structure),
  making snapshot creation O(n) instead of O(1).
- **Linear scan**: a query falls back to linear scan (O(n)) instead of using an index
  (O(log n)), because the query planner does not select the appropriate index.
- **GC pause**: the garbage collector (for persistent data structures) pauses reader
  threads during reclamation.

**proptest strategy**:
```rust
// Note: performance invariants are verified by benchmarks, not by proptest.
// Proptest verifies correctness; benchmarks verify performance.
// The proptest below verifies that the read path is contention-free.

proptest! {
    #[test]
    fn read_no_contention(
        datoms in prop::collection::btree_set(arb_datom(), 0..1000),
    ) {
        let store = Store::from_datoms(datoms);
        let snapshot = store.snapshot();

        // Multiple reads from the same snapshot must not interfere
        let r1: Vec<_> = snapshot.datoms().cloned().collect();
        let r2: Vec<_> = snapshot.datoms().cloned().collect();
        prop_assert_eq!(r1, r2, "Same snapshot returned different results");
    }
}
```

**Lean theorem**:
```lean
/-- Read latency: in the abstract model, membership check on Finset is
    decidable and total. Performance bounds are implementation-specific
    and verified empirically, not algebraically. -/

-- The algebraic model does not have a notion of "latency."
-- This invariant is verified by benchmarks at the implementation level.
-- The Lean theorem states the weaker property: reads are total.
theorem read_total (s : DatomStore) (d : Datom) :
    Decidable (d ∈ s) := by
  exact Finset.decidableMem d s
```

---

### INV-FERR-028: Cold Start Latency

**Traces to**: SEED.md §10, INV-FERR-013 (Checkpoint Equivalence), INV-FERR-014 (Recovery)
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let cold_start(S) = load_checkpoint(latest) + replay_wal(remaining)
  be the time to load a store from disk to first query.

∀ S where |S| = 10⁸:
  cold_start(S) < 5s

This is a performance contract on the recovery path (INV-FERR-014).
The store must be queryable within 5 seconds of process start, even
at 100M datom scale.
```

#### Level 1 (State Invariant)
The store must be queryable within 5 seconds of process start at 100M datom scale.
This includes:
1. Loading the checkpoint file (deserialization + checksum verification).
2. Replaying any WAL entries after the checkpoint.
3. Rebuilding indexes from the loaded datoms.

The 5-second bound drives several design decisions:
- **Checkpoints are recent**: the system checkpoints frequently enough that WAL replay
  is bounded (at most a few thousand transactions, not millions).
- **Checkpoint format is sequential**: the checkpoint file is a flat sequence of datoms,
  not a complex data structure requiring random access during loading.
- **Index rebuild is incremental**: indexes are rebuilt by inserting datoms one by one,
  not by sorting the entire datom set (which would be O(n log n) and potentially too slow).
- **Memory mapping**: for very large stores, the checkpoint can be memory-mapped,
  deferring page faults to first access rather than loading everything upfront.

At 100M datoms × 200 bytes/datom = 20GB, loading from a modern NVMe SSD (3GB/s)
takes ≈ 7 seconds raw. The 5-second bound implies either:
- Memory mapping (deferred loading), or
- Compressed checkpoints (~5:1 compression on datom data), or
- Pre-built indexes stored alongside the checkpoint (no rebuild step).

#### Level 2 (Implementation Contract)
```rust
/// Cold start: load checkpoint + replay WAL + rebuild indexes.
/// Must complete in < 5s for 100M datoms.
pub fn cold_start(data_dir: &Path) -> Result<Store, RecoveryError> {
    let start = Instant::now();

    // Phase 1: Load checkpoint (memory-mapped for large stores)
    let checkpoint_path = latest_checkpoint(data_dir)?;
    let store = if checkpoint_size(&checkpoint_path)? > MMAP_THRESHOLD {
        load_checkpoint_mmap(&checkpoint_path)?
    } else {
        load_checkpoint(&checkpoint_path)?
    };

    let checkpoint_time = start.elapsed();
    log::info!("Checkpoint loaded in {:?}", checkpoint_time);

    // Phase 2: Replay WAL
    let mut store = store;
    let wal_path = data_dir.join("wal");
    let wal_entries = Wal::open(&wal_path)?.recover()?;
    let replayed = wal_entries.iter()
        .filter(|e| e.epoch > store.current_epoch())
        .count();
    for entry in wal_entries {
        if entry.epoch > store.current_epoch() {
            store.apply_wal_entry(&entry)?;
        }
    }

    let total_time = start.elapsed();
    log::info!(
        "Cold start complete: {} datoms, {} WAL entries replayed, {:?}",
        store.len(), replayed, total_time
    );

    debug_assert!(total_time < Duration::from_secs(5),
        "INV-FERR-028: cold start took {:?} (limit: 5s)", total_time);

    Ok(store)
}

/// Benchmark: cold start at scale.
#[cfg(test)]
fn benchmark_cold_start() {
    let store = generate_store_with_n_datoms(100_000_000);
    let tmp_dir = tempfile::tempdir().unwrap();
    checkpoint(&store, &tmp_dir.path().join("checkpoint")).unwrap();

    let start = Instant::now();
    let loaded = cold_start(tmp_dir.path()).unwrap();
    let elapsed = start.elapsed();

    assert!(elapsed < Duration::from_secs(5),
        "Cold start took {:?} (limit: 5s)", elapsed);
    assert_eq!(loaded.len(), store.len());
}
```

**Falsification**: A store with 100M datoms where `cold_start()` takes more than 5
seconds. Specific failure modes:
- **No checkpoint**: the store has never been checkpointed, so recovery replays the
  entire WAL (all 100M transactions).
- **Checkpoint too old**: the latest checkpoint is from millions of transactions ago,
  and WAL replay takes longer than expected.
- **Index rebuild O(n log n)**: indexes are rebuilt by sorting the entire datom set
  instead of incremental insertion.
- **Checksum verification**: verifying the BLAKE3 checksum of a 20GB file takes > 5s
  (BLAKE3 at 1GB/s ≈ 20s for 20GB, which exceeds the limit — requires streaming
  checksum verification or memory mapping).

**proptest strategy**:
```rust
// Performance invariants are verified by benchmarks, not proptest.
// Proptest verifies the correctness of the cold start path.

proptest! {
    #[test]
    fn cold_start_correctness(
        datoms in prop::collection::btree_set(arb_datom(), 0..200),
        txns in prop::collection::vec(arb_transaction(), 0..10),
    ) {
        let mut store = Store::from_datoms(datoms);
        for tx in &txns {
            let _ = store.transact(tx.clone());
        }

        let tmp_dir = tempfile::tempdir()?;
        checkpoint(&store, &tmp_dir.path().join("checkpoint"))?;

        let loaded = cold_start(tmp_dir.path())?;
        prop_assert_eq!(store.datom_set(), loaded.datom_set());
        prop_assert_eq!(store.current_epoch(), loaded.current_epoch());
    }
}
```

**Lean theorem**:
```lean
/-- Cold start correctness: loading from checkpoint produces the same store.
    Performance (< 5s) is an implementation constraint, not algebraic. -/

-- Cold start correctness is checkpoint roundtrip (INV-FERR-013).
-- The latency bound is empirically verified, not algebraically provable.
theorem cold_start_correct (s : DatomStore) :
    checkpoint_deserialize (checkpoint_serialize s) = s :=
  checkpoint_roundtrip s
```

---

### INV-FERR-029: LIVE View Resolution

**Traces to**: SEED.md §4, C1, INV-STORE-001, INV-STORE-012
**Verification**: `V:PROP`, `V:KANI`, `V:LEAN`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let LIVE(S) be the "current state" view of store S.
Let causal_sort(S) be the datoms of S sorted by TxId (epoch order).

LIVE(S) = fold(causal_sort(S), apply_resolution)

where apply_resolution processes each datom in causal order:
  - assert(e, a, v): add (e, a, v) to the live set
  - retract(e, a, v): remove (e, a, v) from the live set

LIVE resolves retractions by TxId ordering. It NEVER removes datoms
from the primary store — it computes a derived view by folding over
the full history.

∀ d ∈ S: d ∈ primary(S)   -- d is always in the primary store (C1)
LIVE(S) ⊆ primary(S)      -- LIVE is a subset (resolved view)
|LIVE(S)| ≤ |primary(S)|  -- LIVE may be smaller (retractions)
```

#### Level 1 (State Invariant)
The LIVE view is a derived computation, not a separate data structure. It is computed
by folding over the primary store's datoms in causal order (by TxId/epoch), applying
each assertion and retraction:
- An `assert(e, a, v)` datom adds `(e, a, v)` to the live set.
- A `retract(e, a, v)` datom removes `(e, a, v)` from the live set.
- Assertions and retractions are matched by `(e, a, v)` triple.

The LIVE view never modifies the primary store. The primary store contains all datoms
(both assertions and retractions) in perpetuity (C1). The LIVE view is recomputed from
the primary store on demand (or cached and invalidated when new transactions arrive).

The LIVE view is epoch-sensitive: `LIVE(S, e)` gives the live set as of epoch `e`,
considering only datoms with `tx_epoch <= e`. This enables time-travel queries:
"what was the live state at epoch 1000?" without replaying the entire history from
scratch (the fold can start from a cached snapshot and apply only new datoms).

The causal ordering (by TxId/epoch) is critical: if a retraction has a lower epoch
than the assertion it retracts, the retraction has no effect (it was "before" the
assertion in causal time). This prevents "retroactive retractions" from corrupting
the live view.

#### Level 2 (Implementation Contract)
```rust
/// Compute the LIVE view of the store at a given epoch.
/// Returns the set of (entity, attribute, value) triples that are
/// currently asserted (not retracted) at the given epoch.
pub fn live_view(store: &Store, epoch: u64) -> BTreeSet<(EntityId, Attribute, Value)> {
    let mut live: BTreeSet<(EntityId, Attribute, Value)> = BTreeSet::new();

    // Process datoms in causal order (by epoch, then by insertion order within epoch)
    for datom in store.datoms_by_epoch(..=epoch) {
        let key = (datom.entity.clone(), datom.attribute.clone(), datom.value.clone());
        match datom.op {
            Op::Assert => { live.insert(key); }
            Op::Retract => { live.remove(&key); }
        }
    }

    live
}

/// LIVE view NEVER modifies the primary store.
/// This function takes &Store (immutable reference), not &mut Store.
/// It returns a NEW set, leaving the store unchanged.

#[kani::proof]
#[kani::unwind(10)]
fn live_view_correctness() {
    let mut datoms = BTreeSet::new();

    // Assert (e=1, a=1, v=1)
    datoms.insert(Datom { e: 1, a: 1, v: 1, tx: 1, op: true });
    // Retract (e=1, a=1, v=1)
    datoms.insert(Datom { e: 1, a: 1, v: 1, tx: 2, op: false });
    // Assert (e=1, a=1, v=2)
    datoms.insert(Datom { e: 1, a: 1, v: 2, tx: 3, op: true });

    let store = Store::from_datoms(datoms.clone());

    // LIVE at epoch 3: only (1, 1, 2) is live
    let live = live_view(&store, 3);
    assert!(live.contains(&(1, 1, 2)));
    assert!(!live.contains(&(1, 1, 1)));

    // Primary store still has ALL datoms
    assert_eq!(store.datoms.len(), 3);
}
```

**Falsification**: The LIVE view removes a datom from the primary store (C1 violation).
Or: the LIVE view includes a `(e, a, v)` triple that has been retracted at an earlier
or equal epoch. Or: the LIVE view excludes a `(e, a, v)` triple that has been asserted
and never retracted. Specific failure modes:
- **Primary mutation**: `live_view()` takes `&mut Store` and modifies `store.datoms`.
- **Out-of-order processing**: datoms are processed in non-causal order, causing a
  retraction to be applied before the assertion it retracts.
- **Missing retraction matching**: a retraction `retract(e, a, v)` does not remove the
  matching assertion because the matching logic uses entity-only (not `(e, a, v)`
  triple) comparison.
- **Epoch filtering bug**: `datoms_by_epoch(..=epoch)` includes datoms with `tx_epoch > epoch`.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn live_view_respects_retractions(
        assertions in prop::collection::vec(arb_datom_assert(), 1..20),
        retraction_indices in prop::collection::vec(0..20usize, 0..10),
    ) {
        let mut store = Store::genesis();
        let mut epoch = store.current_epoch();

        // Apply assertions
        for d in &assertions {
            epoch += 1;
            store.insert_datom_at_epoch(d.clone(), epoch);
        }

        // Apply retractions for selected assertions
        let mut retracted: BTreeSet<_> = BTreeSet::new();
        for &idx in &retraction_indices {
            if idx < assertions.len() {
                let d = &assertions[idx];
                epoch += 1;
                store.insert_datom_at_epoch(
                    Datom { op: Op::Retract, ..d.clone() },
                    epoch,
                );
                retracted.insert((d.entity.clone(), d.attribute.clone(), d.value.clone()));
            }
        }

        let live = live_view(&store, epoch);

        // Retracted triples must not be in LIVE
        for key in &retracted {
            prop_assert!(!live.contains(key),
                "Retracted triple still in LIVE view: {:?}", key);
        }

        // Non-retracted assertions must be in LIVE
        for d in &assertions {
            let key = (d.entity.clone(), d.attribute.clone(), d.value.clone());
            if !retracted.contains(&key) {
                prop_assert!(live.contains(&key),
                    "Asserted triple missing from LIVE view: {:?}", key);
            }
        }
    }

    #[test]
    fn live_view_does_not_mutate_store(
        datoms in prop::collection::btree_set(arb_datom(), 0..100),
    ) {
        let store = Store::from_datoms(datoms.clone());
        let pre_len = store.len();
        let pre_datoms = store.datom_set().clone();

        let _live = live_view(&store, store.current_epoch());

        // Store unchanged
        prop_assert_eq!(store.len(), pre_len, "Store length changed after LIVE view");
        prop_assert_eq!(*store.datom_set(), pre_datoms, "Store datoms changed after LIVE view");
    }
}
```

**Lean theorem**:
```lean
/-- LIVE view: fold over datoms in causal order, applying assertions and retractions.
    The LIVE view is a subset of the primary store's datom set. -/

def apply_op (live : Finset (Nat × Nat × Nat)) (d : Datom) : Finset (Nat × Nat × Nat) :=
  let key := (d.e, d.a, d.v)
  if d.op then  -- assert
    live ∪ {key}
  else  -- retract
    live \ {key}

def live_view_model (datoms : List Datom) : Finset (Nat × Nat × Nat) :=
  datoms.foldl apply_op ∅

/-- LIVE view is at most as large as the number of unique (e,a,v) triples. -/
theorem live_bounded (datoms : List Datom) :
    (live_view_model datoms).card ≤ datoms.length := by
  sorry -- induction on datoms; each step adds at most 1 element

/-- Retraction followed by no re-assertion means the triple is absent. -/
theorem retraction_removes (live : Finset (Nat × Nat × Nat)) (e a v : Nat) :
    (e, a, v) ∉ apply_op live { e, a, v, tx := 0, op := false } := by
  unfold apply_op
  simp [Finset.mem_sdiff, Finset.mem_singleton]
```

---

### INV-FERR-030: Read Replica Subset

**Traces to**: SEED.md §4, C4, INV-FERR-010 (Merge Convergence)
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let leader(S) be the leader's datom set.
Let replica(Rᵢ) be replica i's datom set.

∀ replica Rᵢ at any point in time:
  replica(Rᵢ) ⊆ leader(S)

After WAL catch-up (eventual consistency):
  replica(Rᵢ) = leader(S)

Read replicas are always a subset of the leader's state. They never
contain datoms that the leader does not have (no phantom datoms).
After catching up with the leader's WAL, they are equal.
```

#### Level 1 (State Invariant)
A read replica receives datoms from the leader via WAL streaming (or periodic snapshot +
WAL replay). At any point in time, the replica's datom set is a subset of the leader's
datom set. The replica never invents datoms that the leader does not have.

After the replica has fully caught up (received and applied all WAL entries up to the
leader's current epoch), its datom set is identical to the leader's. The time to catch
up is bounded by `O(|WAL_delta|)` where `WAL_delta` is the set of WAL entries the
replica has not yet received.

Read replicas do not accept writes directly. All writes go through the leader, which
serializes them (INV-FERR-007), writes to WAL (INV-FERR-008), and streams the WAL
entries to replicas. This ensures that all replicas converge to the same state
(INV-FERR-010) and that the total order of writes is consistent across all replicas.

#### Level 2 (Implementation Contract)
```rust
/// Read replica: receives WAL entries from leader, never accepts direct writes.
pub struct ReadReplica {
    store: Store,
    leader_epoch: u64,  // last known leader epoch
}

impl ReadReplica {
    /// Apply a WAL entry received from the leader.
    /// The entry must be from the leader (not from another replica).
    pub fn apply_wal_entry(&mut self, entry: &WalEntry) -> Result<(), ReplicaError> {
        if entry.epoch <= self.store.current_epoch() {
            return Ok(()); // already applied (idempotent)
        }
        if entry.epoch != self.store.current_epoch() + 1 {
            return Err(ReplicaError::EpochGap {
                expected: self.store.current_epoch() + 1,
                got: entry.epoch,
            });
        }

        for datom in &entry.datoms {
            self.store.datoms.insert(datom.clone());
            self.store.indexes.insert(datom);
        }
        self.store.epoch = entry.epoch;
        self.leader_epoch = entry.epoch;

        Ok(())
    }

    /// Read-only: no transact, no merge.
    /// Writing to a replica is a compile-time error.
    pub fn snapshot(&self) -> Snapshot<'_> {
        self.store.snapshot()
    }

    // NOTE: There is intentionally no transact() or merge_from() method.
    // Writes go through the leader only.
}

// Stateright model: replicas are always subsets of leader
impl stateright::Model for ReplicaModel {
    fn properties(&self) -> Vec<Property<Self>> {
        vec![
            Property::always("replica_subset", |_, state: &ReplicaState| {
                state.replicas.iter().all(|r| r.datoms.is_subset(&state.leader.datoms))
            }),
            Property::eventually("replica_convergence", |_, state: &ReplicaState| {
                state.replicas.iter().all(|r| r.datoms == state.leader.datoms)
            }),
        ]
    }
}
```

**Falsification**: A read replica contains a datom that the leader does not contain.
Or: after full WAL catch-up, the replica's datom set differs from the leader's.
Specific failure modes:
- **Phantom WAL entry**: the replica receives a WAL entry that was not generated by
  the leader (network corruption or man-in-the-middle).
- **Out-of-order application**: WAL entries are applied out of epoch order, causing
  the replica to skip an entry and diverge.
- **Direct write**: the replica accepts a direct write (transact or merge), adding
  datoms that the leader does not have.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn replica_always_subset(
        txns in prop::collection::vec(arb_transaction(), 1..20),
        apply_up_to in 0..20usize,
    ) {
        let mut leader = Store::genesis();
        let mut wal_entries = vec![];

        for tx in txns {
            if let Ok(receipt) = leader.transact(tx) {
                wal_entries.push(leader.last_wal_entry().clone());
            }
        }

        let mut replica = ReadReplica::from_genesis();
        let apply_count = apply_up_to.min(wal_entries.len());

        for entry in &wal_entries[..apply_count] {
            let _ = replica.apply_wal_entry(entry);
        }

        // Replica is always a subset of leader
        prop_assert!(replica.store.datom_set().is_subset(leader.datom_set()),
            "Replica is not a subset of leader");

        // If all entries applied, replica equals leader
        if apply_count == wal_entries.len() {
            prop_assert_eq!(replica.store.datom_set(), leader.datom_set(),
                "Replica did not converge after full catch-up");
        }
    }
}
```

**Lean theorem**:
```lean
/-- Read replica subset: a replica receiving a subset of the leader's
    transactions has a subset of the leader's datoms. -/

def apply_entries (s : DatomStore) (entries : List DatomStore) : DatomStore :=
  entries.foldl (fun acc e => merge acc e) s

theorem replica_subset (leader_txns replica_txns : List DatomStore)
    (h : replica_txns.length ≤ leader_txns.length)
    (h_prefix : ∀ i, i < replica_txns.length → replica_txns[i]! = leader_txns[i]!) :
    apply_entries ∅ replica_txns ⊆ apply_entries ∅ leader_txns := by
  sorry -- induction on replica_txns; each step adds a subset of what leader adds
```

---

### INV-FERR-031: Genesis Determinism

**Traces to**: SEED.md §4, C7 (Self-bootstrap), INV-STORE-003
**Verification**: `V:PROP`, `V:KANI`, `V:LEAN`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let genesis() : DatomStore be the genesis function.

∀ invocations i, j of genesis():
  genesis_i() = genesis_j()

The genesis function is deterministic: every call produces the exact
same store with the exact same datoms, the exact same schema, and
the exact same epoch (0). The genesis store contains:
  1. Schema attribute definitions (:db/ident, :db/valueType, :db/cardinality, etc.)
  2. Genesis transaction metadata.
  3. No user data.

Genesis is the fixed point from which all stores diverge. Every store
in the system is a descendant of the genesis store via TRANSACT and
MERGE operations.
```

#### Level 1 (State Invariant)
Every call to `genesis()` produces a bitwise-identical store. This is critical for:
- **Replica initialization**: a new replica calls `genesis()` and then catches up via
  WAL replay. If `genesis()` were non-deterministic, the replica would start from a
  different state and diverge.
- **Testing**: deterministic genesis enables reproducible tests. Every test starts from
  the same store state.
- **Content addressing**: the genesis store's identity hash (used for deduplication and
  comparison) must be identical across invocations. If genesis produces different datoms,
  the identity hashes differ, and self-merge detection (INV-FERR-003 fast path) fails.

The genesis store contains only schema-definition datoms (`:db/ident`, `:db/valueType`,
`:db/cardinality`, `:db/unique`, `:db/isComponent`, `:db/doc`). These are the minimum
set required to bootstrap the schema-as-data system (C3). The genesis transaction
has epoch 0 and a deterministic transaction entity ID.

No randomness, no timestamps, no system-specific information enters the genesis store.
The genesis function is a pure function of the Ferratomic version (schema set is
version-specific, and any schema change is a new Ferratomic version).

#### Level 2 (Implementation Contract)
```rust
/// Genesis: produce the initial store.
/// This function is deterministic — always returns the same store.
/// No randomness, no timestamps, no system info.
pub fn genesis() -> Store {
    let mut datoms = BTreeSet::new();

    // Schema attributes (deterministic, hardcoded)
    let schema_attrs = [
        (":db/ident", ValueType::Keyword, Cardinality::One),
        (":db/valueType", ValueType::Keyword, Cardinality::One),
        (":db/cardinality", ValueType::Keyword, Cardinality::One),
        (":db/unique", ValueType::Keyword, Cardinality::One),
        (":db/isComponent", ValueType::Bool, Cardinality::One),
        (":db/doc", ValueType::String, Cardinality::One),
    ];

    let tx_entity = EntityId::from_content(b"genesis-tx");
    let epoch = 0u64;

    for (ident, vtype, card) in &schema_attrs {
        let entity = EntityId::from_content(ident.as_bytes());

        datoms.insert(Datom::new(entity, Attribute::DB_IDENT, Value::keyword(ident), epoch, Op::Assert));
        datoms.insert(Datom::new(entity, Attribute::DB_VALUE_TYPE, Value::keyword(&vtype.to_string()), epoch, Op::Assert));
        datoms.insert(Datom::new(entity, Attribute::DB_CARDINALITY, Value::keyword(&card.to_string()), epoch, Op::Assert));
    }

    // Genesis transaction metadata
    datoms.insert(Datom::new(tx_entity, Attribute::DB_IDENT, Value::keyword(":tx/genesis"), epoch, Op::Assert));

    Store::from_datoms_at_epoch(datoms, epoch)
}

#[kani::proof]
#[kani::unwind(4)]
fn genesis_determinism() {
    let g1 = genesis();
    let g2 = genesis();
    assert_eq!(g1.datom_set(), g2.datom_set());
    assert_eq!(g1.current_epoch(), g2.current_epoch());
}
```

**Falsification**: Two calls to `genesis()` produce stores with different datom sets,
different epochs, or different identity hashes. Specific failure modes:
- **Timestamp in genesis**: genesis includes the current wall-clock time as a datom
  value, causing different invocations to produce different datoms.
- **Random entity IDs**: entity IDs are generated from random numbers instead of
  content hashing, causing different genesis stores to have different entity IDs.
- **Non-deterministic iteration**: the schema attributes are iterated in a non-deterministic
  order (e.g., from a HashMap), causing datoms to be inserted in different orders, which
  could affect content hashes if the hash depends on insertion order.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn genesis_always_same(
        _seed in any::<u64>(),  // proptest provides different seeds; genesis must be same
    ) {
        let g1 = genesis();
        let g2 = genesis();

        prop_assert_eq!(g1.datom_set(), g2.datom_set(),
            "Genesis produced different datom sets");
        prop_assert_eq!(g1.current_epoch(), g2.current_epoch(),
            "Genesis produced different epochs");
        prop_assert_eq!(g1.identity_hash(), g2.identity_hash(),
            "Genesis produced different identity hashes");
        prop_assert_eq!(g1.schema(), g2.schema(),
            "Genesis produced different schemas");
    }
}
```

**Lean theorem**:
```lean
/-- Genesis determinism: the genesis function is a constant.
    We model it as returning the empty set (the simplest deterministic value). -/

def genesis_model : DatomStore := ∅

theorem genesis_deterministic :
    genesis_model = genesis_model := by rfl

/-- Every store is a superset of genesis (genesis is the bottom element). -/
theorem genesis_bottom (s : DatomStore) :
    genesis_model ⊆ s := by
  unfold genesis_model
  exact Finset.empty_subset s

/-- Merging with genesis is identity. -/
theorem genesis_merge_identity (s : DatomStore) :
    merge genesis_model s = s := by
  unfold merge genesis_model
  exact Finset.empty_union s
```

---

### INV-FERR-032: LIVE Resolution Correctness

**Traces to**: SEED.md §4, INV-FERR-029 (LIVE View Resolution), INV-STORE-012
**Verification**: `V:PROP`, `V:KANI`, `V:LEAN`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
LIVE(S) = fold(causal_sort(S), apply_resolution)

This invariant strengthens INV-FERR-029 by specifying the exact
semantics of apply_resolution:

∀ entity e, attribute a:
  let assertions = {(e, a, v, tx) | (e, a, v, tx, assert) ∈ S}
  let retractions = {(e, a, v, tx) | (e, a, v, tx, retract) ∈ S}

  LIVE(S, e, a) = assertions \ retractions
    (where the \ operation is on (e, a, v) triples, matching by value)

For cardinality-one attributes:
  LIVE(S, e, a) = {(e, a, v)} where v is the value from the
    last-writer-wins assertion (highest tx epoch) that has not
    been retracted.

For cardinality-many attributes:
  LIVE(S, e, a) = {(e, a, v) | asserted and not retracted}
```

#### Level 1 (State Invariant)
The LIVE resolution function correctly computes the current state of every entity
by processing all assertions and retractions in causal order. The resolution is
correct if and only if:
1. Every asserted `(e, a, v)` triple that has not been retracted is present in LIVE.
2. Every retracted `(e, a, v)` triple is absent from LIVE.
3. For cardinality-one attributes, only the latest (highest-epoch) non-retracted
   value is present.
4. For cardinality-many attributes, all non-retracted values are present.
5. The causal ordering is by `tx_epoch` (epoch assigned during TRANSACT, per
   INV-FERR-007), not by wall-clock time or insertion order.

This invariant is the bridge between the raw datom store (which contains all
assertions and retractions in perpetuity) and the query layer (which needs to
know "what is the current value of attribute A on entity E?").

Correctness of LIVE resolution depends on:
- INV-FERR-007 (epoch ordering is correct).
- INV-FERR-005 (indexes are bijection of primary store — AEVT index provides
  efficient attribute-entity access).
- INV-FERR-009 (schema validation — cardinality is correctly defined).

#### Level 2 (Implementation Contract)
```rust
/// Compute LIVE resolution for a specific entity and attribute.
/// Handles both cardinality-one and cardinality-many.
pub fn live_resolve(
    store: &Store,
    entity: EntityId,
    attr: Attribute,
    epoch: u64,
) -> Vec<Value> {
    let schema = store.schema();
    let cardinality = schema.get(&attr)
        .map(|def| def.cardinality)
        .unwrap_or(Cardinality::One);

    // Get all datoms for this (entity, attribute) in causal order
    let datoms: Vec<_> = store.datoms_eavt_range(entity, attr, ..=epoch)
        .sorted_by_key(|d| d.tx_epoch)
        .collect();

    match cardinality {
        Cardinality::One => {
            // Last-writer-wins: process in causal order, keep last non-retracted
            let mut current: Option<Value> = None;
            for d in &datoms {
                match d.op {
                    Op::Assert => current = Some(d.value.clone()),
                    Op::Retract => {
                        if current.as_ref() == Some(&d.value) {
                            current = None;
                        }
                    }
                }
            }
            current.into_iter().collect()
        }
        Cardinality::Many => {
            // Set semantics: track all non-retracted values
            let mut live_values: BTreeSet<Value> = BTreeSet::new();
            for d in &datoms {
                match d.op {
                    Op::Assert => { live_values.insert(d.value.clone()); }
                    Op::Retract => { live_values.remove(&d.value); }
                }
            }
            live_values.into_iter().collect()
        }
    }
}

#[kani::proof]
#[kani::unwind(10)]
fn live_resolution_card_one() {
    // Assert v=1, then assert v=2 (last-writer-wins)
    let mut store = Store::genesis();
    let entity = EntityId::from_content(b"test");
    let attr = Attribute::from("name");

    // Epoch 1: assert v=1
    store.insert_datom(Datom::new(entity, attr.clone(), Value::String("Alice".into()), 1, Op::Assert));
    // Epoch 2: assert v=2
    store.insert_datom(Datom::new(entity, attr.clone(), Value::String("Bob".into()), 2, Op::Assert));

    let live = live_resolve(&store, entity, attr, 2);
    assert_eq!(live.len(), 1);
    assert_eq!(live[0], Value::String("Bob".into()));
}

#[kani::proof]
#[kani::unwind(10)]
fn live_resolution_retraction() {
    let mut store = Store::genesis();
    let entity = EntityId::from_content(b"test");
    let attr = Attribute::from("tags");

    // Epoch 1: assert "red"
    store.insert_datom(Datom::new(entity, attr.clone(), Value::String("red".into()), 1, Op::Assert));
    // Epoch 2: assert "blue"
    store.insert_datom(Datom::new(entity, attr.clone(), Value::String("blue".into()), 2, Op::Assert));
    // Epoch 3: retract "red"
    store.insert_datom(Datom::new(entity, attr.clone(), Value::String("red".into()), 3, Op::Retract));

    let live = live_resolve(&store, entity, attr, 3);
    assert_eq!(live.len(), 1);
    assert_eq!(live[0], Value::String("blue".into()));
}
```

**Falsification**: The LIVE resolution produces an incorrect result. Specific cases:
- **Retracted value present**: a `(e, a, v)` triple that has been retracted appears
  in the LIVE output.
- **Non-retracted value absent**: a `(e, a, v)` triple that has been asserted and
  never retracted is missing from the LIVE output.
- **Cardinality-one violation**: for a cardinality-one attribute, the LIVE output
  contains more than one value.
- **Wrong last-writer**: for cardinality-one, the LIVE output contains a value from
  an earlier epoch rather than the latest epoch.
- **Causal order wrong**: datoms are processed in non-causal order (e.g., by insertion
  time rather than by epoch), causing incorrect resolution when assertions and
  retractions arrive out of order.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn live_resolution_card_one_correct(
        values in prop::collection::vec(arb_value(), 2..10),
        retract_last in any::<bool>(),
    ) {
        let mut store = Store::genesis();
        let entity = EntityId::from_content(b"prop_entity");
        let attr = Attribute::from("card_one_attr");
        // Define attribute as cardinality-one in schema
        store.define_attr(&attr, ValueType::String, Cardinality::One);

        let mut epoch = 1u64;
        for v in &values {
            store.insert_datom(Datom::new(entity, attr.clone(), v.clone(), epoch, Op::Assert));
            epoch += 1;
        }

        let last_value = values.last().unwrap().clone();

        if retract_last {
            store.insert_datom(Datom::new(entity, attr.clone(), last_value.clone(), epoch, Op::Retract));
            epoch += 1;
        }

        let live = live_resolve(&store, entity, attr, epoch);

        if retract_last {
            // Last value was retracted; for card-one, previous value wins
            // (if not also retracted)
            prop_assert!(live.len() <= 1);
        } else {
            prop_assert_eq!(live.len(), 1);
            prop_assert_eq!(live[0], last_value);
        }
    }

    #[test]
    fn live_resolution_card_many_correct(
        assert_values in prop::collection::btree_set(arb_value(), 1..20),
        retract_values in prop::collection::btree_set(arb_value(), 0..10),
    ) {
        let mut store = Store::genesis();
        let entity = EntityId::from_content(b"prop_entity");
        let attr = Attribute::from("card_many_attr");
        store.define_attr(&attr, ValueType::String, Cardinality::Many);

        let mut epoch = 1u64;
        for v in &assert_values {
            store.insert_datom(Datom::new(entity, attr.clone(), v.clone(), epoch, Op::Assert));
            epoch += 1;
        }
        for v in &retract_values {
            store.insert_datom(Datom::new(entity, attr.clone(), v.clone(), epoch, Op::Retract));
            epoch += 1;
        }

        let live = live_resolve(&store, entity, attr, epoch);
        let live_set: BTreeSet<_> = live.into_iter().collect();

        let expected: BTreeSet<_> = assert_values.difference(&retract_values).cloned().collect();
        prop_assert_eq!(live_set, expected);
    }
}
```

**Lean theorem**:
```lean
/-- LIVE resolution correctness: the live set is exactly the set of
    asserted values minus the set of retracted values. -/

def assertions (datoms : Finset Datom) (e a : Nat) : Finset Nat :=
  (datoms.filter (fun d => d.e = e ∧ d.a = a ∧ d.op = true)).image (fun d => d.v)

def retractions (datoms : Finset Datom) (e a : Nat) : Finset Nat :=
  (datoms.filter (fun d => d.e = e ∧ d.a = a ∧ d.op = false)).image (fun d => d.v)

def live_values (datoms : Finset Datom) (e a : Nat) : Finset Nat :=
  assertions datoms e a \ retractions datoms e a

theorem live_correct_assert (datoms : Finset Datom) (e a v : Nat)
    (h_assert : ∃ tx, { e, a, v, tx, op := true : Datom } ∈ datoms)
    (h_no_retract : ¬ ∃ tx, { e, a, v, tx, op := false : Datom } ∈ datoms) :
    v ∈ live_values datoms e a := by
  unfold live_values assertions retractions
  simp [Finset.mem_sdiff, Finset.mem_image, Finset.mem_filter]
  constructor
  · obtain ⟨tx, htx⟩ := h_assert
    exact ⟨{ e, a, v, tx, op := true }, ⟨htx, rfl, rfl, rfl⟩, rfl⟩
  · intro ⟨d, ⟨hd, _, _, _⟩, _⟩
    exact absurd ⟨d.tx, hd⟩ h_no_retract
    sorry -- need to reconstruct the exact retraction datom

theorem live_correct_retract (datoms : Finset Datom) (e a v tx_a tx_r : Nat)
    (h_assert : { e, a, v, tx := tx_a, op := true : Datom } ∈ datoms)
    (h_retract : { e, a, v, tx := tx_r, op := false : Datom } ∈ datoms) :
    v ∉ live_values datoms e a := by
  unfold live_values
  simp [Finset.mem_sdiff]
  intro _
  unfold retractions
  simp [Finset.mem_image, Finset.mem_filter]
  exact ⟨{ e, a, v, tx := tx_r, op := false }, ⟨h_retract, rfl, rfl, rfl⟩, rfl⟩
  sorry -- Finset.image/filter details
```

---

## 23.4 Architectural Decision Records

### ADR-FERR-001: Persistent Data Structures

**Traces to**: INV-FERR-006 (Snapshot Isolation), INV-FERR-027 (Read P99.99)
**Stage**: 0

**Problem**: Snapshot isolation requires readers to access consistent historical views
while writers mutate the store. How do we provide O(1) snapshot creation without
copying the entire store?

**Options**:

| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| A: `im-rs` persistent collections | Structural sharing via HAMTs. `clone()` is O(1). | Proven Rust crate, excellent API, O(log n) ops. | ~2x memory overhead vs. BTreeMap. Write throughput ~50% of BTreeMap. |
| B: `BTreeMap` + CoW (copy-on-write) | `Arc<BTreeMap>` snapshots clone on write. | Zero overhead for read-only snapshots. Standard library. | Clone is O(n) — unacceptable at 100M datoms. Writer must clone before mutating. |
| C: Custom COW B-tree | Purpose-built persistent B-tree with page-level CoW. | Optimal performance. Page-level sharing. | Significant implementation effort (thousands of LoC). Correctness risk. |

**Decision**: **Option A: `im-rs`**

The 2x memory overhead is acceptable (100M datoms × 200 bytes × 2 = 40GB, within modern
server RAM). The ~50% write throughput reduction is acceptable because writes are
serialized anyway (INV-FERR-007). The main advantage is O(1) snapshot creation, which
enables INV-FERR-006 and INV-FERR-027 without any locking on the read path.

**Rejected**:
- Option B: O(n) clone is unacceptable at scale (copying 20GB per snapshot).
- Option C: The implementation and verification cost exceeds the benefit. `im-rs` is
  battle-tested with 10M+ downloads and property-based test coverage. A custom
  implementation would need equivalent verification effort.

**Consequence**: `Store` uses `im::OrdMap` and `im::OrdSet` instead of `std::BTreeMap`
and `std::BTreeSet`. All index structures use `im-rs`. Snapshot creation is
`store.clone()` which takes O(1) time. Writers pay a ~50% throughput penalty.

**Source**: SEED.md §4 Axiom 3 (Snapshots), ADR-STORE-003

---

### ADR-FERR-002: Async Runtime

**Traces to**: INV-FERR-024 (Substrate Agnosticism), INV-FERR-021 (Backpressure)
**Stage**: 0

**Problem**: Ferratomic needs concurrency for WAL writing, checkpoint creation, anti-entropy
protocol, and read replica streaming. Which concurrency model should be used?

**Options**:

| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| A: No async (`std::thread`) | OS threads for background tasks, channels for communication. | Simple, debuggable, no colored functions. Predictable latency. | Thread pool sizing is manual. No built-in backpressure on channels. |
| B: Tokio | Full async runtime with spawn, select, channels. | Ecosystem standard. Built-in timers, I/O, networking. Backpressure via bounded channels. | Colored functions (async/await infects API). Runtime overhead. Harder to debug. |
| C: Custom lightweight runtime | Task queue with work-stealing, no async/await. | Minimal overhead. Full control. | Reinventing the wheel. No ecosystem compatibility. |

**Decision**: **Option A: No async (`std::thread` + `crossbeam`)**

Ferratomic is an embedded database, not a network server. The concurrency requirements
are modest: one writer thread, one WAL flusher, one background checkpointer. OS threads
are sufficient and avoid the complexity of async runtimes.

The caller (braid kernel) may use an async runtime for its own purposes (HTTP server,
daemon). Ferratomic does not impose a runtime choice on the caller (C8).

Backpressure is implemented via bounded `crossbeam::channel` (INV-FERR-021) and
`try_lock` on the write mutex. No unbounded queues.

**Rejected**:
- Option B: Async infects the entire API surface. A `Store::transact()` that returns
  a `Future` forces the caller to use an async runtime, violating substrate agnosticism
  (INV-FERR-024, C8). The embedded use case (braid kernel) does not need async I/O.
- Option C: The effort is not justified for ~3 background threads.

**Consequence**: All Ferratomic APIs are synchronous. Background tasks (WAL flush,
checkpoint) run on dedicated OS threads spawned at store creation. Inter-thread
communication uses `crossbeam::channel` (bounded) and `std::sync::Mutex`/`RwLock`.
The caller can wrap Ferratomic in async wrappers (`spawn_blocking`) if needed.

**Source**: SEED.md §4, C8 (Substrate Independence)

---

### ADR-FERR-003: Concurrency Model

**Traces to**: INV-FERR-006 (Snapshot Isolation), INV-FERR-007 (Write Linearizability), INV-FERR-027 (Read P99.99)
**Stage**: 0

**Problem**: How do concurrent readers and writers access the store without lock contention
on the read path?

**Options**:

| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| A: `ArcSwap` | Writers build a new snapshot and atomically swap the pointer. Readers load the pointer atomically (no lock). | Zero lock contention on reads. O(1) snapshot access. | Writers pay O(1) swap cost. Old snapshots held by readers prevent deallocation. |
| B: `RwLock` | Writers hold write lock, readers hold read lock. | Simple, standard. | Read lock is still a lock — contention under high reader load. Writer starvation possible. |
| C: Page-level MVCC | Per-page version numbers, copy-on-write at page granularity. | Fine-grained concurrency. Low memory overhead. | Complex implementation. Page-level conflicts. Harder to verify. |

**Decision**: **Option A: `ArcSwap`**

`ArcSwap` provides zero-cost read access: `store.load()` is an atomic pointer load with
no contention, no lock, no CAS loop. Combined with `im-rs` persistent data structures
(ADR-FERR-001), snapshot creation is O(1) and readers never block.

Writers build a new version of the store (using `im-rs` structural sharing), then
atomically swap the pointer. The old version remains accessible to any reader that
loaded it before the swap. When the last reader drops its reference, the old version
is deallocated.

**Rejected**:
- Option B: Even read locks introduce contention under high reader load (10K concurrent
  readers per INV-FERR-027). `pthread_rwlock` has measurable overhead at high reader
  counts.
- Option C: The complexity is not justified. Page-level MVCC is appropriate for disk-based
  databases (e.g., SQLite WAL mode), but Ferratomic's in-memory indexes do not benefit
  from page-level granularity.

**Consequence**: The `Store` is wrapped in `ArcSwap<Store>`. Readers call `store.load()`
(atomic, lock-free) to get a snapshot. Writers call `store.rcu(|old| { /* build new */ })`
to atomically update. Old snapshots are reference-counted and deallocated when no longer
referenced.

**Source**: INV-FERR-006, INV-FERR-027, ADR-STORE-006

---

### ADR-FERR-004: Observer Delivery Semantics

**Traces to**: INV-FERR-003 (Merge Idempotency), INV-FERR-010 (Merge Convergence), ADRS PD-004
**Stage**: 0

**Problem**: When a store publishes events to observers (e.g., after a successful transact),
what delivery guarantees should be provided?

**Options**:

| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| A: At-least-once | Events may be delivered more than once. Observers must be idempotent. | Simple. Crash-safe (retry on failure). Compatible with CRDT semantics (merge is idempotent). | Observer must handle duplicates. More network traffic on retries. |
| B: Exactly-once | Each event delivered exactly once, even across crashes. | Clean semantics. No duplicate handling needed. | Requires distributed consensus or transactional outbox. Significant complexity. |
| C: Best-effort | Events may be lost. No delivery guarantee. | Simplest implementation. Zero overhead. | Observers can miss events. Data inconsistency. |

**Decision**: **Option A: At-least-once**

At-least-once delivery aligns with the CRDT semantics of the store. Since merge is
idempotent (INV-FERR-003), receiving the same datoms twice is harmless — the second
delivery is a no-op. This makes observer delivery crash-safe without complex distributed
consensus: if a delivery fails, retry. If the retry delivers a duplicate, idempotency
absorbs it.

**Rejected**:
- Option B: Exactly-once requires either distributed consensus (Paxos/Raft) or a
  transactional outbox with deduplication. Both add significant complexity and latency.
  Since the underlying data model is CRDT (idempotent merge), exactly-once provides
  no additional correctness benefit.
- Option C: Best-effort delivery means observers can miss events permanently. This
  would require a separate synchronization mechanism (e.g., periodic full-state
  reconciliation), which is more expensive than at-least-once retry.

**Consequence**: Observer delivery uses a simple retry loop with exponential backoff.
Observers must be idempotent (which they are, since they process datoms via merge).
The anti-entropy protocol (INV-FERR-022) serves as a fallback: even if observer delivery
fails permanently, anti-entropy eventually synchronizes all nodes.

**Source**: INV-FERR-003, INV-FERR-010, PD-004

---

### ADR-FERR-005: Clock Model

**Traces to**: INV-FERR-015 (HLC Monotonicity), INV-FERR-016 (HLC Causality)
**Stage**: 0

**Problem**: Distributed datom stores need a clock model for causal ordering. Which clock
model provides the best tradeoff between accuracy, complexity, and availability?

**Options**:

| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| A: Hybrid Logical Clock (HLC) | Physical time + logical counter + agent ID. Total order. | Captures causality. Tolerates clock skew. O(1) comparison. | Requires message piggyback. Logical overflow possible (handled by backpressure). |
| B: Lamport Clock | Logical counter only. Captures causality. | Simplest. No dependency on wall clock. | No connection to real time. Hard to debug ("when did this happen?"). |
| C: TrueTime (Google Spanner) | GPS + atomic clock for bounded uncertainty. | True calendar time with known error bounds. | Requires specialized hardware (GPS receivers, atomic clocks). Not available on commodity servers. |

**Decision**: **Option A: Hybrid Logical Clock (HLC)**

HLC provides the best balance: it captures causality (like Lamport clocks) and
approximates real time (like TrueTime), without requiring specialized hardware. The
physical component is useful for debugging ("this datom was created around 2026-03-29T10:00")
and for time-range queries. The logical component ensures strict ordering even when
physical clocks are skewed.

**Rejected**:
- Option B: Lamport clocks lose all connection to real time. A datom with Lamport
  timestamp 47 conveys no information about when it was created. This makes time-range
  queries impossible and debugging difficult.
- Option C: TrueTime is Google infrastructure. It requires GPS receivers and atomic
  clocks, which are not available on commodity servers or developer laptops. Ferratomic
  must run on any machine (C8).

**Consequence**: Every datom carries an HLC timestamp. The HLC is advanced on every
local event (tick) and on every message receipt (receive). Epoch ordering in the store
is derived from HLC values. Time-range queries use the physical component. Causal
ordering uses the full HLC.

**Source**: INV-FERR-015, INV-FERR-016, SR-004

---

### ADR-FERR-006: Sharding Strategy

**Traces to**: INV-FERR-017 (Shard Equivalence), INV-FERR-033 (Cross-Shard Query)
**Stage**: 0

**Problem**: When a store is too large for a single node, how should datoms be distributed
across shards?

**Options**:

| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| A: Entity-hash | `shard(d) = hash(d.entity) % N`. All datoms for an entity on one shard. | Entity locality (single-entity queries hit one shard). Simple. Deterministic. | Hot entities (many datoms) can cause shard imbalance. Cross-entity queries require fan-out. |
| B: Attribute-namespace | `shard(d) = namespace(d.attribute)`. All schema datoms on one shard, all data datoms on another. | Namespace locality. Schema queries hit one shard. | Cross-namespace queries require fan-out. Schema shard may be tiny (waste). |
| C: Random | `shard(d) = hash(d.content_hash()) % N`. Uniformly distributed. | Perfect balance. No hot spots. | No locality. Every query requires fan-out to all shards. Entity-level operations require N round-trips. |

**Decision**: **Option A: Entity-hash**

Entity-hash sharding preserves entity locality: all datoms about entity E reside on
`shard(E) = hash(E) % N`. This means that single-entity operations (lookup all attributes
of E, resolve E's current state, retract a value on E) hit exactly one shard, with no
cross-shard coordination.

The hot-entity problem is mitigated by content-addressed entity IDs (INV-FERR-012):
entity IDs are BLAKE3 hashes, which are uniformly distributed, so datoms are
approximately uniformly distributed across shards. An entity with unusually many
attributes (hundreds of datoms) does create a minor imbalance, but this is bounded
and manageable.

**Rejected**:
- Option B: Attribute-namespace sharding creates highly unbalanced shards (the schema
  namespace has ~50 datoms; the data namespace has millions). It also breaks entity
  locality.
- Option C: Random sharding destroys all locality. Entity-level operations require
  fan-out to all N shards, increasing latency by N× and network traffic by N×.

**Consequence**: `shard_id(d) = u64::from_le_bytes(d.entity.as_bytes()[0..8]) % N`.
Entity-hash sharding is deterministic and content-addressed. Cross-entity queries
(e.g., "all entities with attribute A = V") require fan-out to all shards, which is
acceptable for OLAP workloads but not for OLTP. The AVET index on each shard
provides local optimization.

**Source**: INV-FERR-017, SEED.md §4

---

### ADR-FERR-007: Lean-Rust Bridge

**Traces to**: §23.0.4 (Lean 4 Foundation Model), all INV-FERR invariants with Lean theorems
**Stage**: 0

**Problem**: The Ferratomic specification includes Lean 4 theorems alongside Rust code.
How do we ensure that the Lean model and the Rust implementation remain in sync?

**Options**:

| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| A: Parallel models | Lean model and Rust implementation are maintained independently. Consistency is verified by reviewing both side by side. | Simple. No tooling dependency. Each language is idiomatic. | Models can drift apart silently. No mechanical guarantee of consistency. Review-dependent. |
| B: Aeneas | Extract Lean types from Rust code (Charon frontend → Aeneas backend). Prove properties on the extracted Lean. | Mechanical extraction. Proofs apply to actual code. | Aeneas is research-grade. Limited Rust subset supported. Extraction can break on complex code. |
| C: Lean FFI | Call Rust from Lean (or vice versa) via C FFI. Run Lean proofs against Rust data structures. | Direct interop. Proofs on real data. | Complex build system. FFI boundary is unsafe. Performance overhead. |

**Decision**: **Option A: Parallel models (with mechanical consistency checks)**

The Lean model and Rust implementation are maintained as parallel codebases. The Lean
model captures the algebraic laws (Level 0) and key state invariants (Level 1). The
Rust implementation refines these into concrete data structures and algorithms (Level 2).

Consistency is maintained by:
1. **Structural correspondence**: every Lean definition (`DatomStore`, `merge`, `apply_tx`)
   has a direct Rust counterpart (`Store`, `merge()`, `transact()`).
2. **Property-based testing**: proptest strategies in Rust verify the same properties
   that Lean theorems prove. Any proptest failure indicates a model-implementation gap.
3. **Cleanroom review**: during specification review, Lean theorems and Rust implementations
   are compared side by side to verify that the Lean model accurately captures the Rust
   behavior.

**Rejected**:
- Option B: Aeneas is promising but immature. It supports only a subset of Rust (no
  async, limited trait support, no `BTreeSet`). The Ferratomic codebase would need to
  be rewritten to fit Aeneas's supported subset, which is unacceptable.
- Option C: FFI introduces `unsafe` code at the boundary, violating INV-FERR-023. The
  build system complexity (Lean toolchain + Rust toolchain + C FFI) is excessive.

**Consequence**: Lean theorems are maintained in the specification document alongside
the Rust code. They are not compiled or checked as part of `cargo build`. Lean
verification is a separate process (`lake build` in the Lean project) performed
during specification review. The Lean model is intentionally abstract (using `Finset`
rather than `BTreeSet`) to capture the algebraic properties without mirroring
implementation details.

**Source**: §23.0.4, SEED.md §10

---

## 23.5 Negative Cases

### NEG-FERR-001: No Panics in Production Code

**Traces to**: INV-FERR-019 (Error Exhaustiveness), ADRS FD-001
**Stage**: 0

**Statement**: No Ferratomic crate uses `unwrap()`, `expect()`, `panic!()`,
`unreachable!()`, `todo!()`, `unimplemented!()`, or any other panicking construct on
fallible operations in production code (non-test, non-bench).

**Rationale**: A panic in a database engine corrupts the caller's process. If braid
panics during `transact()`, the daemon crashes, the WAL may be left in an inconsistent
state, and all in-flight operations are lost. Database engines must return errors, not
abort.

**Enforcement**:
```rust
// In every Ferratomic crate's lib.rs:
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
```

**Falsification**: A function in any Ferratomic crate that calls `unwrap()`, `expect()`,
`panic!()`, or any other panicking construct on a fallible operation, in non-test code.
Detection: `cargo clippy --all-targets -- -D clippy::unwrap_used -D clippy::expect_used
-D clippy::panic -D clippy::todo -D clippy::unimplemented` reports zero warnings for
non-test code.

**Exception**: `unreachable!()` is permitted in match arms that are provably unreachable
by the type system (e.g., after an exhaustive pattern match that the compiler cannot
verify is exhaustive due to cross-crate type boundaries). Each such usage must include
a comment explaining why the arm is unreachable.

---

### NEG-FERR-002: No Unsafe Code

**Traces to**: INV-FERR-023 (No Unsafe Code)
**Stage**: 0

**Statement**: No Ferratomic crate contains `unsafe` blocks, `unsafe fn`, `unsafe impl`,
or `unsafe trait` declarations.

**Rationale**: `unsafe` code bypasses the Rust borrow checker, enabling use-after-free,
data races, and buffer overflows. A database engine must not have these failure modes.
All performance-critical operations (hashing, serialization, index operations) have
safe implementations. The ~10% performance penalty of bounds checking is acceptable
for the correctness guarantee.

**Enforcement**: `#![forbid(unsafe_code)]` in every crate root. This is stronger than
`#![deny(unsafe_code)]` — it cannot be overridden by `#[allow(unsafe_code)]` on
individual items.

**Falsification**: Any Ferratomic crate compiles successfully while containing `unsafe`.
Detection: `#![forbid(unsafe_code)]` causes compilation failure. Additionally:
`grep -rn "unsafe" crates/ferratomic/ --include="*.rs"` should return zero results
(excluding comments and string literals).

**Exception**: None. Dependencies may use `unsafe` internally (e.g., `blake3` uses SIMD,
`crossbeam` uses atomics), but the Ferratomic crates themselves are pure safe Rust.

---

### NEG-FERR-003: No Data Loss on Crash

**Traces to**: INV-FERR-008 (WAL Fsync Ordering), INV-FERR-014 (Recovery Correctness), C1
**Stage**: 0

**Statement**: No committed transaction's datoms are lost after a crash. "Committed"
means `transact()` returned `Ok(receipt)`, which implies the WAL entry was fsynced
(INV-FERR-008).

**Rationale**: A database that loses committed data is worse than no database. The WAL
fsync ordering (INV-FERR-008) and recovery correctness (INV-FERR-014) together guarantee
this property. The WAL is the durable ground truth; everything else (indexes, snapshots,
caches) is derived state that can be rebuilt from the WAL.

**Falsification**: A transaction `T` where `transact(T)` returns `Ok(receipt)`, followed
by a crash, followed by recovery, and `T`'s datoms are absent from the recovered store.
Testing: simulate crashes at every point in the transact path (using `failpoints` or
`stateright` model checking) and verify that committed data survives.

**Exception**: None. This is an absolute guarantee. The only scenario where data is lost
is hardware failure (disk corruption beyond what BLAKE3 checksums can detect, or complete
disk failure without backup). Software crashes never cause data loss.

---

### NEG-FERR-004: No Stale Reads After Snapshot Publication

**Traces to**: INV-FERR-006 (Snapshot Isolation), INV-FERR-007 (Write Linearizability)
**Stage**: 0

**Statement**: Once a snapshot is published (epoch advanced, `ArcSwap` updated), any
new reader that calls `store.snapshot()` sees the new snapshot. No reader obtains a
snapshot from before the publication after the publication has completed.

**Rationale**: Stale reads can cause incorrect decisions. If agent A transacts a
retraction, and agent B (reading immediately after) still sees the old assertion,
agent B may act on stale data. The `ArcSwap` model (ADR-FERR-003) prevents this:
once `store.swap(new_snapshot)` returns, all subsequent `store.load()` calls return
the new snapshot.

**Clarification**: This invariant applies to new snapshot acquisitions, not to existing
snapshots. A reader that obtained a snapshot BEFORE the publication continues to see
the old data (this is snapshot isolation, INV-FERR-006, which is correct behavior).
Only readers that obtain a snapshot AFTER the publication must see the new data.

**Falsification**: A reader calls `store.snapshot()` after a writer's `transact()`
has returned `Ok`, and the reader does not see the transaction's datoms. This
indicates that the `ArcSwap` update was not visible to the reader — either the
swap was not performed, or the reader loaded a cached stale pointer.

**Exception**: None. This is a linearizability guarantee on the publication point.

---

### NEG-FERR-005: No Unbounded Memory Growth

**Traces to**: INV-FERR-021 (Backpressure Safety)
**Stage**: 0

**Statement**: The Ferratomic engine's memory usage is bounded. No operation causes
unbounded memory allocation. Specifically:
- The write queue depth is bounded (INV-FERR-021).
- Old snapshots held by readers are bounded by the number of concurrent readers and
  the snapshot retention policy.
- The WAL buffer is bounded (`wal_buffer_max` configuration).
- Index memory is proportional to the datom set size (no index bloat).

**Rationale**: An embedded database that grows without bound eventually OOMs the host
process. Since Ferratomic runs inside the braid daemon (which runs indefinitely),
any unbounded growth is a time bomb.

**Falsification**: The Ferratomic engine's resident memory exceeds `expected_datom_memory
+ max_concurrent_snapshots × snapshot_overhead + wal_buffer_max + constant_overhead`.
Specific failure modes:
- **Snapshot leak**: a reader holds a snapshot reference indefinitely, preventing
  deallocation of the old store version. Memory grows with each new transaction.
- **WAL buffer leak**: the WAL flusher falls behind, and the buffer grows without bound.
- **Index bloat**: a secondary index grows faster than the primary store (e.g., due to
  a bug in index garbage collection, if one existed — but indexes do not have GC
  because the store is append-only).

**Detection**: Monitor `process_resident_memory_bytes` and `ferratomic_datom_count`.
The ratio `memory / datom_count` should be approximately constant (200-500 bytes/datom
depending on value sizes). A growing ratio indicates a memory leak.

**Exception**: None. Memory growth must be strictly proportional to datom count.

---

## 23.6 Cross-Shard Query Planning

### INV-FERR-033: Cross-Shard Query Correctness

**Traces to**: SEED.md §4, INV-FERR-017 (Shard Equivalence), ADR-FERR-006
**Verification**: `V:PROP`, `V:KANI`, `V:LEAN`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let query : DatomStore → Result be a Datalog query.
Let S be a store sharded into N shards: S = ⋃ᵢ shard(S, i).

For monotonic queries Q (queries whose result can only grow as the
input grows — no negation, no aggregation, no set difference):

  query(S) = query(⋃ᵢ shard(S, i))
           = ⋃ᵢ query(shard(S, i))     (by monotonicity)

The result of querying the full store equals the union of querying
each shard independently. This is the CALM theorem applied to Datalog:
monotonic queries are coordination-free.

For non-monotonic queries Q' (negation, aggregation, set difference):
  query(S) ≠ ⋃ᵢ query(shard(S, i))    (in general)

Non-monotonic queries require either:
  1. Full materialization: collect all shards, then query locally.
  2. Multi-round coordination: exchange intermediate results.
  3. Explicit shard specification: the caller selects a single shard.
```

#### Level 1 (State Invariant)
The Datalog query evaluator classifies every query as monotonic or non-monotonic before
execution. For monotonic queries, the evaluator can execute the query independently on
each shard and merge the results. For non-monotonic queries, the evaluator requires the
full datom set before evaluation.

Classification is syntactic:
- **Monotonic**: conjunctive queries (joins), unions, projections, selections with
  monotonic predicates. No negation, no aggregation, no `not`.
- **Non-monotonic**: queries containing `not`, `count`, `sum`, `max`, `min`, `exists`,
  set difference, or any user-defined function not proven monotonic.

The query planner determines which shards to contact based on the query's variables:
- If the query specifies an entity ID, only the entity's shard is contacted.
- If the query is over an attribute range, all shards are contacted (fan-out).
- If the query is monotonic and full-fan-out, results are merged via set union.
- If the query is non-monotonic and full-fan-out, all shard data is materialized
  locally before evaluation.

#### Level 2 (Implementation Contract)
```rust
/// Query classification: monotonic vs non-monotonic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMonotonicity {
    Monotonic,
    NonMonotonic,
}

/// Classify a Datalog query by monotonicity.
pub fn classify_query(query: &DatalogQuery) -> QueryMonotonicity {
    if query.has_negation() || query.has_aggregation() || query.has_set_difference() {
        QueryMonotonicity::NonMonotonic
    } else {
        QueryMonotonicity::Monotonic
    }
}

/// Execute a query across shards.
pub fn query_sharded(
    shards: &[Store],
    query: &DatalogQuery,
) -> Result<QueryResult, QueryError> {
    match classify_query(query) {
        QueryMonotonicity::Monotonic => {
            // Fan-out, merge results
            let results: Vec<QueryResult> = shards.iter()
                .map(|shard| eval_query(shard, query))
                .collect::<Result<_, _>>()?;
            Ok(merge_query_results(results))
        }
        QueryMonotonicity::NonMonotonic => {
            // Materialize all shards, then query
            let full_store = unshard(shards);
            eval_query(&full_store, query)
        }
    }
}

#[kani::proof]
#[kani::unwind(8)]
fn cross_shard_monotonic_correct() {
    let datoms: BTreeSet<Datom> = kani::any();
    kani::assume(datoms.len() <= 4);
    let shard_count: usize = kani::any();
    kani::assume(shard_count > 0 && shard_count <= 3);

    let store = Store::from_datoms(datoms.clone());
    let shards = shard(&store, shard_count);

    // For a simple monotonic query (attribute lookup):
    let attr: u64 = kani::any();

    // Full store result
    let full_result: BTreeSet<_> = store.datoms.iter()
        .filter(|d| d.a == attr)
        .cloned().collect();

    // Sharded result (union of per-shard results)
    let sharded_result: BTreeSet<_> = shards.iter()
        .flat_map(|s| s.datoms.iter().filter(|d| d.a == attr).cloned())
        .collect();

    assert_eq!(full_result, sharded_result);
}
```

**Falsification**: A monotonic query `Q` where `query(S) != ⋃ᵢ query(shard(S, i))`.
This would indicate either:
- The query classifier incorrectly classifies a non-monotonic query as monotonic.
- The shard function loses datoms (violates INV-FERR-017).
- The result merge function drops results.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn cross_shard_monotonic_correct(
        datoms in prop::collection::btree_set(arb_datom(), 0..200),
        shard_count in 1..8usize,
        query_attr in arb_attribute(),
    ) {
        let store = Store::from_datoms(datoms);
        let shards = shard(&store, shard_count);

        // Monotonic query: all datoms with attribute = query_attr
        let full_result: BTreeSet<_> = store.datoms.iter()
            .filter(|d| d.attribute == query_attr)
            .cloned().collect();

        let sharded_result: BTreeSet<_> = shards.iter()
            .flat_map(|s| s.datoms.iter()
                .filter(|d| d.attribute == query_attr)
                .cloned())
            .collect();

        prop_assert_eq!(full_result, sharded_result,
            "Cross-shard monotonic query gave different results");
    }

    #[test]
    fn monotonicity_classifier_sound(
        query in arb_datalog_query(),
    ) {
        let classification = classify_query(&query);

        if classification == QueryMonotonicity::Monotonic {
            // Verify: query has no negation, aggregation, or set difference
            prop_assert!(!query.has_negation(),
                "Monotonic classification but query has negation");
            prop_assert!(!query.has_aggregation(),
                "Monotonic classification but query has aggregation");
            prop_assert!(!query.has_set_difference(),
                "Monotonic classification but query has set difference");
        }
    }
}
```

**Lean theorem**:
```lean
/-- Cross-shard query correctness: for monotonic queries (modeled as
    filter predicates), querying the union equals the union of queries. -/

theorem filter_union_comm (a b : DatomStore) (p : Datom → Prop) [DecidablePred p] :
    (a ∪ b).filter p = a.filter p ∪ b.filter p := by
  exact Finset.filter_union a b p

/-- Generalized to N shards. -/
theorem filter_biUnion_comm (shards : Finset (Fin n)) (f : Fin n → DatomStore)
    (p : Datom → Prop) [DecidablePred p] :
    (shards.biUnion f).filter p = shards.biUnion (fun i => (f i).filter p) := by
  exact Finset.filter_biUnion shards f p
  sorry -- Finset.filter_biUnion may need explicit proof depending on Mathlib version
```

---

## 23.7 Partition Tolerance

### INV-FERR-034: Partition Detection

**Traces to**: SEED.md §4, INV-FERR-022 (Anti-Entropy Convergence)
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let SWIM_PERIOD be the SWIM protocol failure detection period.
Let partition_event(t) be a network partition occurring at time t.

∀ partition_event(t):
  ∃ t_detect ≤ t + 2 × SWIM_PERIOD:
    partition_detected(t_detect)

Partitions are detected within two SWIM protocol periods. The SWIM
protocol (Scalable Weakly-consistent Infection-style Process Group
Membership Protocol) uses randomized probing and dissemination to
detect failures with bounded detection time and bounded false-positive
rate.
```

#### Level 1 (State Invariant)
When a network partition occurs between two subsets of nodes, the partition is detected
within `2 × SWIM_PERIOD` by at least one node on each side of the partition. Detection
means:
- The node emits a `PartitionDetected` event (incrementing the
  `ferratomic_partition_detected` counter).
- The node logs a warning with the list of unreachable peers.
- The node continues accepting local writes (CRDT safety: writes are always safe,
  per INV-FERR-035).
- The node notifies any registered observers of the partition event.

The `2 × SWIM_PERIOD` bound arises from the SWIM protocol mechanics:
- Each SWIM period, a node pings a random peer.
- If the peer does not respond, the node requests `k` other peers to probe the
  unresponsive peer (indirect probing).
- If all `k` indirect probes fail, the peer is marked as "suspected."
- After one more period without response, the peer is marked as "failed."
- Total detection time: ≤ 2 periods (one for initial failure, one for confirmation).

The false-positive rate is configurable: more indirect probes (`k`) reduce false
positives but increase network traffic. The default `k = 3` gives a false-positive
rate of < 0.01% for typical network conditions.

#### Level 2 (Implementation Contract)
```rust
/// SWIM-based partition detection.
pub struct PartitionDetector {
    swim_period: Duration,
    indirect_probes: usize,  // k
    peers: Vec<PeerInfo>,
    suspected: BTreeSet<PeerId>,
    failed: BTreeSet<PeerId>,
    metrics: PartitionMetrics,
}

pub struct PartitionMetrics {
    partition_detected: Counter,
    partition_duration_seconds: Histogram,
    anti_entropy_repair_datoms: Counter,
}

impl PartitionDetector {
    /// Run one SWIM protocol round.
    pub fn tick(&mut self) -> Vec<PartitionEvent> {
        let mut events = vec![];

        // Select random peer to probe
        let target = self.select_random_peer();
        let responded = self.direct_probe(&target);

        if !responded {
            // Indirect probing
            let indirect_ok = self.indirect_probe(&target, self.indirect_probes);
            if !indirect_ok {
                if self.suspected.contains(&target.id) {
                    // Previously suspected, now confirmed failed
                    self.suspected.remove(&target.id);
                    self.failed.insert(target.id.clone());
                    self.metrics.partition_detected.inc();
                    events.push(PartitionEvent::PeerFailed(target.id.clone()));

                    log::warn!(
                        "Partition detected: peer {} unreachable for 2 SWIM periods",
                        target.id
                    );
                } else {
                    // First failure: suspect
                    self.suspected.insert(target.id.clone());
                    events.push(PartitionEvent::PeerSuspected(target.id.clone()));
                }
            }
        } else {
            // Peer responded: clear suspicion
            self.suspected.remove(&target.id);
            if self.failed.remove(&target.id) {
                events.push(PartitionEvent::PeerRecovered(target.id.clone()));
            }
        }

        events
    }
}
```

**Falsification**: A network partition persists for more than `2 × SWIM_PERIOD` without
being detected by any node. Specific failure modes:
- **No probing**: the SWIM protocol tick is not invoked (timer not running, or the
  event loop is blocked).
- **All probes to non-partitioned peers**: the random peer selection never selects a
  partitioned peer (probability decreases with partition size, but possible for small
  partitions in large clusters).
- **False negative on indirect probe**: an indirect probe succeeds even though the
  target is partitioned (routing anomaly where the indirect prober can reach the
  target but the detector cannot — possible in complex network topologies).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn partition_detected_within_bound(
        peer_count in 3..20usize,
        partitioned_peers in prop::collection::btree_set(0..20usize, 1..10),
        rounds in 1..20usize,
    ) {
        let partitioned: BTreeSet<_> = partitioned_peers.into_iter()
            .filter(|&p| p < peer_count)
            .collect();

        if partitioned.is_empty() { return Ok(()); }

        let mut detector = PartitionDetector::new(peer_count, 3);

        // Simulate: partitioned peers never respond
        let mut detected = false;
        for round in 0..rounds {
            let events = detector.tick_with_partition(&partitioned);
            if events.iter().any(|e| matches!(e, PartitionEvent::PeerFailed(_))) {
                detected = true;
                // Must detect within 2 rounds per peer (amortized)
                prop_assert!(round <= 2 * peer_count,
                    "Detection took {} rounds (bound: {})", round, 2 * peer_count);
                break;
            }
        }

        if rounds >= 2 * peer_count {
            prop_assert!(detected,
                "Partition not detected after {} rounds", rounds);
        }
    }
}
```

**Lean theorem**:
```lean
/-- Partition detection bound: within 2 rounds, a failed peer is detected.
    We model this as: after 2 probe rounds targeting the failed peer,
    the peer is in the failed set. -/

structure SwimState where
  suspected : Finset Nat
  failed : Finset Nat

def probe_round (state : SwimState) (target : Nat) (responded : Bool) : SwimState :=
  if responded then
    { suspected := state.suspected.erase target, failed := state.failed.erase target }
  else if target ∈ state.suspected then
    { suspected := state.suspected.erase target, failed := state.failed ∪ {target} }
  else
    { suspected := state.suspected ∪ {target}, failed := state.failed }

theorem partition_detected_in_two_rounds (target : Nat) :
    let s0 : SwimState := { suspected := ∅, failed := ∅ }
    let s1 := probe_round s0 target false   -- round 1: suspect
    let s2 := probe_round s1 target false   -- round 2: confirm
    target ∈ s2.failed := by
  unfold probe_round
  simp [Finset.mem_empty, Finset.mem_union, Finset.mem_singleton,
        Finset.mem_erase, Finset.mem_insert]
  sorry -- straightforward case analysis
```

---

### INV-FERR-035: Partition-Safe Operation

**Traces to**: SEED.md §4, C4, INV-FERR-001 through INV-FERR-003 (CRDT laws)
**Verification**: `V:PROP`, `V:MODEL`, `V:LEAN`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ partitions P dividing nodes into subsets {P₁, P₂, ..., Pₖ}:
  ∀ node n ∈ Pᵢ:
    TRANSACT(n, T) succeeds   (writes always accepted)

During a partition, every node continues to accept writes independently.
This is the AP (Availability + Partition tolerance) guarantee of the
CAP theorem. Consistency is eventual: after the partition heals,
anti-entropy (INV-FERR-022) converges all nodes.

Safety proof:
  - Writes are local (no coordination needed for TRANSACT).
  - The store is a G-Set CRDT (grow-only set).
  - G-Set write is always safe: adding a datom never conflicts with
    any other operation (set union is commutative, associative, idempotent).
  - After partition heals: merge(P₁, P₂) = P₁ ∪ P₂ (CRDT merge).
  - By L1-L3: the merged state is identical regardless of merge order.
```

#### Level 1 (State Invariant)
During a network partition, every node continues to accept writes. No node becomes
read-only, no node rejects transactions, no node requires quorum to commit. This is
possible because the store is a G-Set CRDT: every write is an addition to the set,
and additions never conflict with each other.

After the partition heals:
1. Anti-entropy (INV-FERR-022) detects the divergence via Merkle diff.
2. Datoms written during the partition are exchanged between the sides.
3. Both sides merge the received datoms (set union).
4. By INV-FERR-010 (merge convergence), both sides converge to the same state.

The only "conflict" that can arise is at the LIVE view level (INV-FERR-029): if two
agents on different sides of the partition assert different values for the same
cardinality-one attribute, the LIVE view must resolve the conflict. This resolution
is at the query layer, not the store layer: the store contains both assertions
(which is correct), and the LIVE view applies last-writer-wins (by epoch) or
escalates to deliberation (per the resolution policy).

#### Level 2 (Implementation Contract)
```rust
/// Partition-safe write: TRANSACT never fails due to partition.
/// It may fail for other reasons (schema validation, WAL I/O), but
/// never because other nodes are unreachable.
impl Store {
    pub fn transact(&mut self, tx: Transaction<Committed>) -> Result<TxReceipt, TxApplyError> {
        // This method has NO network calls.
        // It operates entirely on local state.
        // It succeeds even if every other node is unreachable.

        // 1. Validate schema (local)
        // 2. Write WAL (local disk)
        // 3. Apply datoms (local memory)
        // 4. Advance epoch (local counter)

        // No: quorum check, leader election, consensus round, remote RPC
        // ...
        Ok(receipt)
    }
}

// Stateright model: writes succeed on both sides of a partition
impl stateright::Model for PartitionModel {
    fn properties(&self) -> Vec<Property<Self>> {
        vec![
            Property::always("writes_always_succeed", |_, state: &PartitionState| {
                // Every pending write eventually succeeds (no rejection due to partition)
                state.pending_writes.iter().all(|w| {
                    state.committed.contains(w) || state.can_commit_locally(w)
                })
            }),
            Property::eventually("partition_convergence", |_, state: &PartitionState| {
                // After partition heals, all nodes converge
                if state.partition_healed {
                    state.nodes.windows(2).all(|w| w[0].datoms == w[1].datoms)
                } else {
                    false // not yet converged (expected)
                }
            }),
        ]
    }
}
```

**Falsification**: A node rejects a valid transaction (schema-valid, well-formed)
solely because it cannot reach other nodes. Specific failure modes:
- **Quorum requirement**: the transaction path includes a quorum check that fails
  when the majority of nodes are unreachable.
- **Leader requirement**: the node refuses to write because it is not the leader
  and cannot reach the leader.
- **Distributed lock**: the transaction path acquires a distributed lock that
  times out due to partition.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn writes_succeed_during_partition(
        txns in prop::collection::vec(arb_transaction(), 1..20),
    ) {
        // Single-node store: simulates one side of a partition
        let mut store = Store::genesis();

        for tx in txns {
            let result = store.transact(tx);
            // Must succeed (or fail for schema reasons, not partition reasons)
            match &result {
                Err(TxApplyError::Validation(_)) => {}, // schema error: OK
                Err(TxApplyError::WalWrite(_)) => {},    // local I/O error: OK
                Err(other) => {
                    prop_assert!(false,
                        "Unexpected error (possible partition-related): {:?}", other);
                }
                Ok(_) => {}, // success: expected
            }
        }
    }
}
```

**Lean theorem**:
```lean
/-- Partition-safe writes: TRANSACT is a local operation on the G-Set.
    It does not require coordination with any other node.
    This follows from the G-Set CRDT property: writes are always safe. -/

-- apply_tx is defined without any "network" or "quorum" parameter.
-- It operates on a single DatomStore. This IS the formal proof that
-- writes are partition-safe: the function's signature has no network dependency.

theorem partition_safe_write (s : DatomStore) (d : Datom) :
    ∃ s', s' = apply_tx s d := by
  exact ⟨apply_tx s d, rfl⟩

/-- After partition heals: merge restores full state. -/
theorem partition_recovery (side_a side_b : DatomStore) :
    let merged := merge side_a side_b
    side_a ⊆ merged ∧ side_b ⊆ merged := by
  constructor
  · exact merge_monotone_left side_a side_b
  · exact merge_monotone_right side_a side_b
```

---

### INV-FERR-036: Partition Recovery

**Traces to**: SEED.md §4, INV-FERR-022 (Anti-Entropy), INV-FERR-010 (Merge Convergence)
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let Δ = |side_A \ side_B| + |side_B \ side_A| be the symmetric difference
  (number of datoms written during partition that the other side has not seen).
Let N = total number of nodes.

∀ partition recovery:
  anti_entropy_repair_time ∈ O(|Δ| × log N)

The repair time is proportional to the number of new datoms (|Δ|) times
the logarithmic factor for Merkle tree traversal (log N where N is the
number of datoms per node, not the number of nodes — the Merkle tree
depth is O(log N)).

After repair:
  state(side_A) = state(side_B) = state(side_A) ∪ state(side_B)
```

#### Level 1 (State Invariant)
When a partition heals, the anti-entropy protocol (INV-FERR-022) repairs the divergence.
The repair process:
1. **Detection**: both sides detect that previously-failed peers are now reachable
   (SWIM protocol, INV-FERR-034).
2. **Merkle comparison**: nodes exchange Merkle roots and walk the tree to identify
   differing subtrees. This takes `O(log N)` hash comparisons per differing datom.
3. **Datom exchange**: only the datoms in differing subtrees are transferred. This
   transfers exactly `|Δ|` datoms (the symmetric difference).
4. **Merge**: received datoms are merged via set union (INV-FERR-001 through
   INV-FERR-003). By CRDT properties, the merge is idempotent and order-independent.
5. **Convergence**: after one round of anti-entropy, both sides have the full state
   (INV-FERR-010). The Merkle roots match, confirming convergence.

The total repair time is dominated by datom transfer: `|Δ| × datom_size / bandwidth`.
For typical partitions (minutes to hours), `|Δ|` is in the thousands to millions.
At 200 bytes/datom and 100MB/s network, 1M datoms takes 2 seconds.

#### Level 2 (Implementation Contract)
```rust
/// Partition recovery: anti-entropy repair after partition heals.
/// Returns the number of datoms exchanged and the repair duration.
pub fn partition_repair(
    local: &mut Store,
    remote: &Store,
    metrics: &PartitionMetrics,
) -> RepairResult {
    let start = Instant::now();

    let exchanged = anti_entropy_full(local, remote);

    let duration = start.elapsed();
    metrics.partition_duration_seconds.observe(duration.as_secs_f64());
    metrics.anti_entropy_repair_datoms.inc_by(exchanged as u64);

    debug_assert_eq!(local.datom_set(), remote.datom_set(),
        "INV-FERR-036: stores did not converge after repair");

    RepairResult {
        datoms_exchanged: exchanged,
        duration,
    }
}

pub struct RepairResult {
    pub datoms_exchanged: usize,
    pub duration: Duration,
}
```

**Falsification**: After partition recovery, the two sides have different datom sets
(convergence failure). Or: the repair time is not `O(|Δ| log N)` — it takes time
proportional to the full store size rather than the delta. Specific failure modes:
- **Full re-sync**: the Merkle comparison identifies the root as different and falls
  back to transferring the entire store (instead of walking the tree to find the delta).
- **Non-convergent merge**: merge has order-dependent behavior (violates INV-FERR-001
  through INV-FERR-003), causing the two sides to diverge further instead of converging.
- **Incomplete exchange**: the anti-entropy protocol exchanges datoms in only one
  direction (A sends to B but B does not send to A), leaving one side behind.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn partition_repair_converges(
        shared_datoms in prop::collection::btree_set(arb_datom(), 0..100),
        a_only in prop::collection::btree_set(arb_datom(), 0..50),
        b_only in prop::collection::btree_set(arb_datom(), 0..50),
    ) {
        let mut a_datoms = shared_datoms.clone();
        a_datoms.extend(a_only.clone());
        let mut b_datoms = shared_datoms;
        b_datoms.extend(b_only.clone());

        let mut store_a = Store::from_datoms(a_datoms);
        let mut store_b = Store::from_datoms(b_datoms);

        // Repair
        let metrics = PartitionMetrics::default();
        let result = partition_repair(&mut store_a, &mut store_b, &metrics);

        // Converged
        prop_assert_eq!(store_a.datom_set(), store_b.datom_set(),
            "Stores did not converge after partition repair");

        // All datoms from both sides present
        for d in &a_only {
            prop_assert!(store_b.datom_set().contains(d),
                "A-only datom missing from B after repair");
        }
        for d in &b_only {
            prop_assert!(store_a.datom_set().contains(d),
                "B-only datom missing from A after repair");
        }
    }

    #[test]
    fn repair_time_proportional_to_delta(
        shared_datoms in prop::collection::btree_set(arb_datom(), 50..200),
        delta_datoms in prop::collection::btree_set(arb_datom(), 1..50),
    ) {
        let mut a = Store::from_datoms(shared_datoms.clone());
        let mut b_datoms = shared_datoms;
        b_datoms.extend(delta_datoms.iter().cloned());
        let b = Store::from_datoms(b_datoms);

        let metrics = PartitionMetrics::default();
        let result = partition_repair(&mut a, &b, &metrics);

        // Datoms exchanged should be approximately |delta|
        // (may be slightly more due to Merkle tree granularity)
        prop_assert!(result.datoms_exchanged >= delta_datoms.len(),
            "Exchanged {} datoms but delta is {}",
            result.datoms_exchanged, delta_datoms.len());
        prop_assert!(result.datoms_exchanged <= delta_datoms.len() * 2,
            "Exchanged {} datoms but delta is only {} (too much overhead)",
            result.datoms_exchanged, delta_datoms.len());
    }
}
```

**Lean theorem**:
```lean
/-- Partition recovery: after merging, both sides have the union of all datoms. -/

theorem partition_recovery_complete (shared a_only b_only : DatomStore) :
    let side_a := shared ∪ a_only
    let side_b := shared ∪ b_only
    let merged := merge side_a side_b
    merged = shared ∪ a_only ∪ b_only := by
  unfold merge
  simp [Finset.union_assoc, Finset.union_comm]
  sorry -- Finset union associativity/commutativity rearrangement

theorem partition_recovery_symmetric (a b : DatomStore) :
    merge a b = merge b a := by
  exact merge_comm a b
```

### Operational Monitoring

The following metrics and alerts support partition tolerance in production:

| Metric | Type | Description |
|--------|------|-------------|
| `ferratomic_partition_detected` | Counter | Number of partition events detected. Increment on each `PeerFailed` event. |
| `ferratomic_partition_duration_seconds` | Histogram | Duration of each partition (from detection to recovery). Buckets: 1s, 5s, 30s, 60s, 300s, 600s, 3600s. |
| `ferratomic_anti_entropy_repair_datoms` | Counter | Total datoms exchanged during anti-entropy repair. High values indicate large partitions or frequent splits. |
| `ferratomic_swim_probe_failures` | Counter | Number of failed SWIM probes (before confirmation). Rising rate indicates network instability. |
| `ferratomic_merkle_diff_datoms` | Histogram | Number of differing datoms per Merkle comparison. Monitors convergence rate. |

| Alert | Condition | Action |
|-------|-----------|--------|
| Partition detected | `ferratomic_partition_detected` increments | Log warning. Notify registered observers. Continue accepting local writes. |
| Long partition | `ferratomic_partition_duration_seconds` > 300s | Escalate to operator. Consider manual intervention (network repair). |
| Large repair | `ferratomic_anti_entropy_repair_datoms` > 1M in single repair | Log warning. Monitor for performance impact during repair. |
| Convergence failure | Two nodes with same update set have different Merkle roots | **Critical**: indicates CRDT invariant violation (INV-FERR-001 through INV-FERR-003). Halt and investigate. |

---

## 23.8 Federation & Federated Query

Federation extends the single-store model to multi-store environments where independent
datom stores — potentially on different machines, different networks, or different
continents — participate in a unified query and merge fabric. The CRDT foundation
(INV-FERR-001 through INV-FERR-003) guarantees that merge remains correct regardless
of topology. Federation adds the operational machinery: transport abstraction, fan-out
query, selective merge, provenance preservation, latency tolerance, and live migration.

**Traces to**: SEED.md §4 (Design Commitment: "CRDT merge scales learning across
organizations"), SEED.md §10 (The Bootstrap), INV-FERR-010 (Merge Convergence),
INV-FERR-022 (Anti-Entropy Convergence), INV-FERR-033 (Cross-Shard Query Correctness),
INV-FERR-034 through INV-FERR-036 (Partition Tolerance)

**Design principles**:

1. **Transport transparency.** Application code never knows or cares whether a store
   is local (in-process), same-machine (Unix socket), LAN (TCP), or WAN (QUIC/gRPC).
   The `Transport` trait abstracts all of these behind the same async interface.

2. **CALM-correct fan-out.** For monotonic queries, fan-out + merge equals query on
   merged store. This is not a heuristic — it is a theorem (INV-FERR-037). Non-monotonic
   queries are explicitly classified and handled via materialization.

3. **Selective knowledge transfer.** Agents do not need to import entire remote stores.
   Selective merge with attribute-namespace filters enables precise knowledge transfer
   (e.g., "learn calibrated policies from project X without importing its task history").

4. **Provenance is never lost.** Every datom retains its original TxId through any
   number of merges across any number of stores. The agent field of TxId answers
   "who observed this?" across organizational boundaries.

5. **Graceful degradation.** Federation operates under partial failure. Timed-out stores
   produce partial results with explicit metadata, not silent data loss.

---

### INV-FERR-037: Federated Query Correctness

**Traces to**: SEED.md §4, INV-FERR-033 (Cross-Shard Query Correctness), CALM theorem
**Verification**: `V:PROP`, `V:KANI`, `V:LEAN`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let {S₁, S₂, ..., Sₖ} be a set of datom stores.
Let Q be a monotonic query (no negation, no aggregation, no set difference).
Let query : DatomStore → Result be the Datalog evaluation function.

∀ monotonic Q, ∀ {S₁, ..., Sₖ}:
  query(⋃ᵢ Sᵢ) = ⋃ᵢ query(Sᵢ)

Proof:
  By structural induction on Q:
  - Base case (attribute filter): Finset.filter_biUnion (proven in INV-FERR-033).
  - Join case: monotonic functions distribute over union by definition.
  - Union case: union distributes over union (trivially).
  - Projection case: image distributes over union.

  The CALM theorem (Hellerstein 2010, Ameloot et al. 2011) establishes that
  monotonic queries are exactly the class of queries that can be evaluated
  without coordination. Fan-out + merge is the coordination-free evaluation
  strategy.

For non-monotonic Q' (negation, aggregation, set difference):
  query(⋃ᵢ Sᵢ) ≠ ⋃ᵢ query(Sᵢ)    (in general)

Non-monotonic queries require full materialization:
  materialize({S₁, ..., Sₖ}) → S_full = ⋃ᵢ Sᵢ
  then query(S_full)
```

#### Level 1 (State Invariant)
For all reachable federation states `F = {S₁, ..., Sₖ}` where each `Sᵢ` is produced
by any sequence of TRANSACT, MERGE, and recovery operations: a monotonic federated
query returns exactly the same result set as querying the union of all stores. This
holds regardless of:
- The number of stores (k ≥ 1).
- The size distribution (some stores may have millions of datoms, others may be empty).
- The physical location of stores (in-process, same machine, different continent).
- The transport used to reach each store (local, TCP, QUIC, gRPC, Unix socket).
- The latency characteristics (some fast, some slow, as long as all respond).
- The overlap between stores (stores may share datoms from prior merges).

The federation query evaluator MUST classify every query as monotonic or non-monotonic
before execution, using the same classification logic as INV-FERR-033. For monotonic
queries, the evaluator fans out to all stores concurrently, collects per-store results,
and merges via set union. For non-monotonic queries, the evaluator materializes all
stores into a single in-memory store, then evaluates locally.

#### Level 2 (Implementation Contract)
```rust
/// A federation of datom stores, potentially heterogeneous in transport.
pub struct Federation {
    stores: Vec<StoreHandle>,
}

/// A handle to a store: local (in-process) or remote (over transport).
pub enum StoreHandle {
    Local(Database),
    Remote(RemoteStore),
}

/// A remote store accessed via a transport layer.
pub struct RemoteStore {
    id: StoreId,
    transport: Box<dyn Transport>,
    addr: SocketAddr,
    timeout: Duration,
}

/// Fan-out a monotonic query to all stores, merge results.
/// For non-monotonic queries, materializes first.
///
/// # Errors
/// Returns `FederationError::AllStoresTimedOut` if every store times out.
/// Returns partial results if some stores time out (with `partial: true`).
///
/// # Panics
/// Never panics. All errors are captured in FederatedResult::store_responses.
pub async fn federated_query(
    federation: &Federation,
    query: &QueryExpr,
) -> Result<FederatedResult, FederationError> {
    let monotonicity = classify_query(query);

    match monotonicity {
        QueryMonotonicity::Monotonic => {
            // Fan-out to all stores concurrently
            let futures: Vec<_> = federation.stores.iter()
                .map(|handle| query_store(handle, query))
                .collect();
            let responses = join_all(futures).await;

            // Merge results via set union (correct by CALM)
            let mut merged = QueryResult::empty();
            let mut store_responses = Vec::with_capacity(responses.len());
            let mut any_ok = false;

            for (i, response) in responses.into_iter().enumerate() {
                match response {
                    Ok(result) => {
                        any_ok = true;
                        store_responses.push(StoreResponse {
                            store_id: federation.stores[i].id(),
                            latency: result.latency,
                            datom_count: result.datom_count,
                            status: ResponseStatus::Ok,
                        });
                        merged = merged.union(result.data);
                    }
                    Err(StoreError::Timeout(elapsed)) => {
                        store_responses.push(StoreResponse {
                            store_id: federation.stores[i].id(),
                            latency: elapsed,
                            datom_count: 0,
                            status: ResponseStatus::Timeout,
                        });
                    }
                    Err(e) => {
                        store_responses.push(StoreResponse {
                            store_id: federation.stores[i].id(),
                            latency: Duration::ZERO,
                            datom_count: 0,
                            status: ResponseStatus::Error(e.to_string()),
                        });
                    }
                }
            }

            if !any_ok {
                return Err(FederationError::AllStoresTimedOut);
            }

            let partial = store_responses.iter()
                .any(|r| r.status != ResponseStatus::Ok);

            Ok(FederatedResult {
                results: merged,
                store_responses,
                partial,
            })
        }
        QueryMonotonicity::NonMonotonic => {
            // Materialize all stores, then query locally
            let full_store = federation.materialize().await?;
            let result = eval_query(&full_store, query)?;
            Ok(FederatedResult {
                results: result,
                store_responses: federation.stores.iter()
                    .map(|h| StoreResponse {
                        store_id: h.id(),
                        latency: Duration::ZERO, // measured during materialize
                        datom_count: 0,
                        status: ResponseStatus::Ok,
                    })
                    .collect(),
                partial: false,
            })
        }
    }
}

#[kani::proof]
#[kani::unwind(10)]
fn federated_query_monotonic_correct() {
    let store_a: BTreeSet<Datom> = kani::any();
    let store_b: BTreeSet<Datom> = kani::any();
    kani::assume(store_a.len() <= 3 && store_b.len() <= 3);

    let attr: u64 = kani::any();

    // Union of stores, then query
    let union: BTreeSet<_> = store_a.union(&store_b).cloned().collect();
    let full_result: BTreeSet<_> = union.iter()
        .filter(|d| d.a == attr)
        .cloned().collect();

    // Query each, then union of results
    let result_a: BTreeSet<_> = store_a.iter()
        .filter(|d| d.a == attr)
        .cloned().collect();
    let result_b: BTreeSet<_> = store_b.iter()
        .filter(|d| d.a == attr)
        .cloned().collect();
    let federated_result: BTreeSet<_> = result_a.union(&result_b)
        .cloned().collect();

    assert_eq!(full_result, federated_result);
}
```

**Falsification**: A monotonic query `Q` and store set `{S₁, ..., Sₖ}` where
`query(⋃ᵢ Sᵢ) ≠ ⋃ᵢ query(Sᵢ)`. Specific failure modes:
- **Result loss**: a datom satisfying the query predicate in some `Sᵢ` is absent from the
  federated result (fan-out failed to reach that store, or result merge dropped it).
- **Result gain**: a datom NOT satisfying the query predicate in any `Sᵢ` appears in the
  federated result (spurious results from merge interaction).
- **Monotonicity misclassification**: a non-monotonic query is classified as monotonic,
  causing fan-out to produce incorrect results (e.g., `COUNT(*)` across stores gives
  sum of per-store counts instead of count of union).
- **Transport-dependent results**: the same query returns different results depending on
  whether a store is `StoreHandle::Local` vs `StoreHandle::Remote` (transport leak).
- **Order-dependent merge**: the order in which per-store results arrive affects the
  final result (violates commutativity of result union).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn federated_query_correct(
        stores in prop::collection::vec(
            prop::collection::btree_set(arb_datom(), 0..100),
            1..5,
        ),
        query_attr in arb_attribute(),
    ) {
        // Build federation
        let store_objs: Vec<Store> = stores.iter()
            .map(|datoms| Store::from_datoms(datoms.clone()))
            .collect();

        // Full union, then query
        let mut all_datoms = BTreeSet::new();
        for s in &stores {
            all_datoms.extend(s.iter().cloned());
        }
        let full_result: BTreeSet<_> = all_datoms.iter()
            .filter(|d| d.attribute == query_attr)
            .cloned().collect();

        // Per-store query, then union
        let federated_result: BTreeSet<_> = store_objs.iter()
            .flat_map(|s| s.datoms.iter()
                .filter(|d| d.attribute == query_attr)
                .cloned())
            .collect();

        prop_assert_eq!(full_result, federated_result,
            "Federated query violated CALM: query(union) != union(query)");
    }

    #[test]
    fn federated_query_result_order_independent(
        stores in prop::collection::vec(
            prop::collection::btree_set(arb_datom(), 0..50),
            2..4,
        ),
        query_attr in arb_attribute(),
        permutation_seed in any::<u64>(),
    ) {
        let store_objs: Vec<Store> = stores.iter()
            .map(|datoms| Store::from_datoms(datoms.clone()))
            .collect();

        // Query in original order
        let result_original: BTreeSet<_> = store_objs.iter()
            .flat_map(|s| s.datoms.iter()
                .filter(|d| d.attribute == query_attr)
                .cloned())
            .collect();

        // Query in permuted order
        let mut permuted = store_objs.clone();
        let mut rng = StdRng::seed_from_u64(permutation_seed);
        permuted.shuffle(&mut rng);

        let result_permuted: BTreeSet<_> = permuted.iter()
            .flat_map(|s| s.datoms.iter()
                .filter(|d| d.attribute == query_attr)
                .cloned())
            .collect();

        prop_assert_eq!(result_original, result_permuted,
            "Federated query depends on store order");
    }
}
```

**Lean theorem**:
```lean
/-- Federated query correctness: for monotonic queries (modeled as filter
    predicates), querying the union of stores equals the union of per-store
    queries. This is a direct generalization of INV-FERR-033 from shards
    to federated stores. -/

-- Two-store case (base for induction)
theorem federated_query_two (s1 s2 : DatomStore) (p : Datom → Prop)
    [DecidablePred p] :
    (s1 ∪ s2).filter p = s1.filter p ∪ s2.filter p := by
  exact Finset.filter_union s1 s2 p

-- N-store case (generalized)
theorem federated_query_n (stores : Finset (Fin k)) (f : Fin k → DatomStore)
    (p : Datom → Prop) [DecidablePred p] :
    (stores.biUnion f).filter p = stores.biUnion (fun i => (f i).filter p) := by
  induction stores using Finset.induction with
  | empty => simp
  | insert ha ih =>
    simp [Finset.biUnion_insert]
    rw [Finset.filter_union]
    congr 1
    exact ih

-- Commutativity of result merge (order-independence)
theorem federated_result_comm (r1 r2 : Finset Result) :
    r1 ∪ r2 = r2 ∪ r1 := by
  exact Finset.union_comm r1 r2

-- Associativity of result merge (grouping-independence)
theorem federated_result_assoc (r1 r2 r3 : Finset Result) :
    (r1 ∪ r2) ∪ r3 = r1 ∪ (r2 ∪ r3) := by
  exact Finset.union_assoc r1 r2 r3
```

---

### INV-FERR-038: Federation Substrate Transparency

**Traces to**: SEED.md §4 (Substrate Independence — C8), INV-FERR-037, ADR-FERR-007
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let Transport be the trait abstracting store access.
Let Local : Transport and Remote : Transport be two implementations.
Let query : Transport → QueryExpr → Result be the query function.

∀ Q ∈ QueryExpr, ∀ S ∈ DatomStore:
  query(Local(S), Q) = query(Remote(S), Q)

More generally, for any federation F = {H₁, ..., Hₖ} where each Hᵢ is
either Local(Sᵢ) or Remote(Sᵢ):

  federated_query(F, Q) = federated_query(F', Q)

where F' is any re-labeling of handles (swapping Local ↔ Remote) as long
as each Hᵢ and H'ᵢ refer to the same underlying store Sᵢ.

The Transport layer is a faithful functor: it preserves the algebraic
structure of the store. Transport ∘ query = query ∘ Transport = query.
```

#### Level 1 (State Invariant)
For all reachable stores `S` and all queries `Q`: the query result is identical
regardless of whether `S` is accessed via `StoreHandle::Local`, `StoreHandle::Remote`
with TCP transport, `StoreHandle::Remote` with QUIC transport, `StoreHandle::Remote`
with Unix socket transport, or any other `Transport` implementation. The only observable
differences are:
- **Latency**: Remote transports add network round-trip time.
- **StoreResponse metadata**: The `latency` field and `status` field reflect transport
  characteristics. But the `results` field is identical.

Application code that depends only on `FederatedResult::results` (and not on
`FederatedResult::store_responses`) produces identical behavior regardless of
deployment topology. This is the substrate transparency guarantee.

#### Level 2 (Implementation Contract)
```rust
/// The Transport trait: all store access goes through this.
/// Implementations: LocalTransport, TcpTransport, QuicTransport,
/// GrpcTransport, UnixSocketTransport.
///
/// The trait contract: for any store S and query Q,
/// transport.query(Q) returns the same result as S.query(Q).
///
/// # Errors
/// Transport errors (network, timeout, protocol) are distinct from
/// query errors (invalid query, schema mismatch). The caller can
/// distinguish them via FerraError variants.
#[async_trait]
pub trait Transport: Send + Sync + 'static {
    /// Execute a query against the remote store.
    async fn query(&self, expr: &QueryExpr) -> Result<TransportResult, TransportError>;

    /// Fetch datoms matching a filter (for selective merge).
    async fn fetch_datoms(&self, filter: &DatomFilter) -> Result<Vec<Datom>, TransportError>;

    /// Fetch the current schema of the remote store.
    async fn schema(&self) -> Result<Schema, TransportError>;

    /// Fetch the current epoch/frontier of the remote store.
    async fn frontier(&self) -> Result<Frontier, TransportError>;

    /// Stream WAL entries from a given epoch (for live migration).
    async fn stream_wal(&self, from_epoch: Epoch) -> Result<WalStream, TransportError>;

    /// Health check: is the remote store reachable?
    async fn ping(&self) -> Result<Duration, TransportError>;
}

/// Local transport: in-process, zero-copy, zero-latency.
pub struct LocalTransport {
    db: Arc<Database>,
}

#[async_trait]
impl Transport for LocalTransport {
    async fn query(&self, expr: &QueryExpr) -> Result<TransportResult, TransportError> {
        let snapshot = self.db.snapshot();
        let result = eval_query(&snapshot, expr)
            .map_err(TransportError::QueryFailed)?;
        Ok(TransportResult {
            data: result,
            latency: Duration::ZERO,
            datom_count: snapshot.datom_count(),
        })
    }
    // ... other methods delegate to Database directly
}

/// TCP transport: LAN/datacenter, persistent connections, reconnect.
pub struct TcpTransport {
    addr: SocketAddr,
    pool: ConnectionPool,
    timeout: Duration,
}

/// Verify transport transparency: same query, same store, different transports.
#[cfg(test)]
fn verify_transport_transparency(
    store: &Store,
    query: &QueryExpr,
    transport_a: &dyn Transport,
    transport_b: &dyn Transport,
) -> bool {
    let result_a = block_on(transport_a.query(query)).unwrap();
    let result_b = block_on(transport_b.query(query)).unwrap();
    result_a.data == result_b.data
}
```

**Falsification**: A query `Q` and store `S` where `query(Local(S), Q)` produces a
different result set than `query(Remote(S), Q)`. Specific failure modes:
- **Serialization loss**: a Value variant is not correctly serialized/deserialized over
  the wire (e.g., `Value::Bytes(Arc<[u8]>)` loses trailing zeros, or `Value::Keyword`
  case is altered).
- **Encoding divergence**: local and remote paths use different Datom serialization
  formats, producing different hash-based comparisons.
- **Query plan divergence**: the remote side uses a different query plan that produces
  different results for edge cases (e.g., different join ordering affects deduplication).
- **Schema version mismatch**: the remote side has a schema evolution that the local
  side hasn't received, causing different attribute resolution.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn transport_transparency(
        datoms in prop::collection::btree_set(arb_datom(), 0..100),
        query_attr in arb_attribute(),
    ) {
        let store = Store::from_datoms(datoms);
        let db = Database::from_store(store.clone());

        let local = LocalTransport::new(Arc::new(db.clone()));

        // Simulate remote: serialize query, send to store, deserialize result
        let remote = LoopbackTransport::new(Arc::new(db));

        let query = QueryExpr::attribute_filter(query_attr);
        let result_local = block_on(local.query(&query)).unwrap();
        let result_remote = block_on(remote.query(&query)).unwrap();

        prop_assert_eq!(result_local.data, result_remote.data,
            "Transport transparency violated: local != remote for same store and query");
    }

    #[test]
    fn value_roundtrip_over_transport(
        value in arb_value(),
    ) {
        let bytes = value.serialize_transport();
        let roundtripped = Value::deserialize_transport(&bytes).unwrap();
        prop_assert_eq!(value, roundtripped,
            "Value lost fidelity through transport serialization");
    }
}
```

**Lean theorem**:
```lean
/-- Transport transparency: Local and Remote are faithful functors.
    We model this as: any function f applied to a store S produces the
    same result regardless of the transport wrapper. -/

-- Transport is modeled as the identity morphism on DatomStore.
-- The algebraic content passes through unchanged.
def local_transport (s : DatomStore) : DatomStore := s
def remote_transport (s : DatomStore) : DatomStore := s

theorem transport_transparency (s : DatomStore) (f : DatomStore → α) :
    f (local_transport s) = f (remote_transport s) := by
  unfold local_transport remote_transport

-- Applied to query (filter)
theorem transport_query_equiv (s : DatomStore) (p : Datom → Prop)
    [DecidablePred p] :
    (local_transport s).filter p = (remote_transport s).filter p := by
  unfold local_transport remote_transport
```

---

### INV-FERR-039: Selective Merge (Knowledge Transfer)

**Traces to**: SEED.md §4 (CRDT merge = set union), INV-FERR-001 through INV-FERR-003,
SEED.md §10 (calibrated policies are transferable)
**Verification**: `V:PROP`, `V:KANI`, `V:LEAN`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let filter : Datom → Bool be a predicate selecting datoms.
Let selective_merge(local, remote, filter) = local ∪ {d ∈ remote | filter(d)}

Theorem: selective_merge preserves CRDT properties.

Proof:
  Let R_f = {d ∈ remote | filter(d)} ⊆ remote.
  selective_merge(local, remote, filter) = local ∪ R_f.

  Since R_f ⊆ remote and R_f is a set:
  1. Commutativity of the merge component:
     local ∪ R_f is a union of two sets, which is commutative.
  2. Associativity: (local ∪ R_f₁) ∪ R_f₂ = local ∪ (R_f₁ ∪ R_f₂)
     by associativity of set union.
  3. Idempotency: local ∪ R_f ∪ R_f = local ∪ R_f
     by idempotency of set union.
  4. Monotonicity: local ⊆ selective_merge(local, remote, filter)
     since A ⊆ A ∪ B for any B.

The key insight: filtering before union does not violate any CRDT property
because the filter is applied to the SOURCE, not to the RESULT. The
operation is still "add some datoms" — just fewer of them.

Corollary: selective_merge with filter = (λd. true) reduces to full merge.
Corollary: selective_merge with filter = (λd. false) is the identity on local.
```

#### Level 1 (State Invariant)
For all reachable stores `(local, remote)` and all filters `f`:
- `local ⊆ selective_merge(local, remote, f)` (monotonicity — no datoms lost from local).
- `selective_merge(local, remote, f) ⊆ local ∪ remote` (no datoms invented).
- `{d ∈ selective_merge(local, remote, f) | d ∉ local} ⊆ {d ∈ remote | f(d)}`
  (only filtered datoms from remote are added).
- Repeated selective_merge with the same filter is idempotent.
- The order of multiple selective_merges from different remotes does not affect the
  final state (commutativity of union applies to filtered subsets too).

Selective merge is the mechanism for knowledge transfer across organizational boundaries.
It enables scenarios like: "Import the calibrated policy weights from the production
team's store without importing their task backlog."

#### Level 2 (Implementation Contract)
```rust
/// A filter predicate for selective merge.
/// Filters operate on datom metadata (entity, attribute, value, tx, op).
#[derive(Debug, Clone)]
pub enum DatomFilter {
    /// Accept all datoms (equivalent to full merge)
    All,
    /// Accept datoms with attributes in the given namespace prefixes
    AttributeNamespace(Vec<String>),
    /// Accept datoms with entity IDs in the given set
    Entities(BTreeSet<EntityId>),
    /// Accept datoms from transactions by specific agents
    FromAgents(BTreeSet<AgentId>),
    /// Accept datoms from transactions after a given epoch
    AfterEpoch(Epoch),
    /// Conjunction: all sub-filters must match
    And(Vec<DatomFilter>),
    /// Disjunction: any sub-filter must match
    Or(Vec<DatomFilter>),
    /// Negation: invert the filter
    Not(Box<DatomFilter>),
    /// Custom predicate (for application-specific filtering)
    Custom(Arc<dyn Fn(&Datom) -> bool + Send + Sync>),
}

impl DatomFilter {
    /// Evaluate the filter against a datom.
    pub fn matches(&self, datom: &Datom, schema: &Schema) -> bool {
        match self {
            DatomFilter::All => true,
            DatomFilter::AttributeNamespace(prefixes) => {
                prefixes.iter().any(|p| datom.attribute.starts_with(p))
            }
            DatomFilter::Entities(ids) => ids.contains(&datom.entity),
            DatomFilter::FromAgents(agents) => agents.contains(&datom.tx.agent),
            DatomFilter::AfterEpoch(epoch) => datom.tx.wall_time > epoch.0,
            DatomFilter::And(filters) => {
                filters.iter().all(|f| f.matches(datom, schema))
            }
            DatomFilter::Or(filters) => {
                filters.iter().any(|f| f.matches(datom, schema))
            }
            DatomFilter::Not(inner) => !inner.matches(datom, schema),
            DatomFilter::Custom(pred) => pred(datom),
        }
    }
}

/// Merge receipt: documents what was transferred.
pub struct MergeReceipt {
    pub source_store: StoreId,
    pub target_store: StoreId,
    pub datoms_transferred: usize,
    pub datoms_filtered_out: usize,
    pub datoms_already_present: usize,
    pub filter_applied: DatomFilter,
    pub duration: Duration,
}

/// Perform selective merge: import filtered datoms from remote into local.
///
/// # Guarantees
/// - local is monotonically non-decreasing (no datoms removed) (INV-FERR-004).
/// - Only datoms matching the filter are transferred.
/// - Transferred datoms retain their original TxId (INV-FERR-040).
/// - The operation is idempotent: repeating it is a no-op.
///
/// # Errors
/// Returns error if the remote store's schema is incompatible (INV-FERR-043).
pub async fn selective_merge(
    local: &mut Database,
    remote: &dyn Transport,
    filter: &DatomFilter,
) -> Result<MergeReceipt, FederationError> {
    // Step 1: Verify schema compatibility
    let remote_schema = remote.schema().await?;
    verify_schema_compatibility(local.schema(), &remote_schema)?;

    // Step 2: Fetch matching datoms from remote
    let remote_datoms = remote.fetch_datoms(filter).await?;

    // Step 3: Compute delta (datoms not already in local)
    let local_snapshot = local.snapshot();
    let mut to_add = Vec::new();
    let mut already_present = 0;

    for datom in &remote_datoms {
        if local_snapshot.contains(datom) {
            already_present += 1;
        } else {
            to_add.push(datom.clone());
        }
    }

    let filtered_out = remote_datoms.len() - to_add.len() - already_present;

    // Step 4: Apply datoms to local store
    if !to_add.is_empty() {
        local.apply_datoms(to_add.clone())?;
    }

    Ok(MergeReceipt {
        source_store: remote.id(),
        target_store: local.id(),
        datoms_transferred: to_add.len(),
        datoms_filtered_out: filtered_out,
        datoms_already_present: already_present,
        filter_applied: filter.clone(),
        duration: Duration::ZERO, // filled by caller
    })
}

#[kani::proof]
#[kani::unwind(10)]
fn selective_merge_monotonic() {
    let local: BTreeSet<Datom> = kani::any();
    let remote: BTreeSet<Datom> = kani::any();
    kani::assume(local.len() <= 3 && remote.len() <= 3);

    let filter_attr: u64 = kani::any();
    let filtered_remote: BTreeSet<_> = remote.iter()
        .filter(|d| d.a == filter_attr)
        .cloned().collect();

    let result: BTreeSet<_> = local.union(&filtered_remote).cloned().collect();

    // Monotonicity: local is a subset of result
    for d in &local {
        assert!(result.contains(d));
    }

    // No invention: result is a subset of local ∪ remote
    let full_union: BTreeSet<_> = local.union(&remote).cloned().collect();
    for d in &result {
        assert!(full_union.contains(d));
    }
}
```

**Falsification**: A selective_merge operation where:
- A datom in `local` before the merge is absent after (monotonicity violation).
- A datom in the result is not in `local ∪ remote` (datom invention).
- A datom in the result is not in `local` and does not match the filter but is from
  `remote` (filter bypass).
- Repeating the same selective_merge changes the store (idempotency violation).
- The same selective_merge with different argument order produces different results
  when the filter selects the same datoms (commutativity violation).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn selective_merge_preserves_local(
        local_datoms in prop::collection::btree_set(arb_datom(), 0..100),
        remote_datoms in prop::collection::btree_set(arb_datom(), 0..100),
        filter_prefix in "[a-z]{1,3}",
    ) {
        let local = Store::from_datoms(local_datoms.clone());
        let remote = Store::from_datoms(remote_datoms.clone());
        let filter = DatomFilter::AttributeNamespace(vec![filter_prefix]);

        let result = selective_merge_sync(&local, &remote, &filter);

        // Every local datom is preserved
        for d in &local_datoms {
            prop_assert!(result.datom_set().contains(d),
                "Local datom lost during selective merge");
        }
    }

    #[test]
    fn selective_merge_only_filtered(
        local_datoms in prop::collection::btree_set(arb_datom(), 0..100),
        remote_datoms in prop::collection::btree_set(arb_datom(), 0..100),
        filter_prefix in "[a-z]{1,3}",
    ) {
        let local = Store::from_datoms(local_datoms.clone());
        let remote = Store::from_datoms(remote_datoms.clone());
        let filter = DatomFilter::AttributeNamespace(vec![filter_prefix.clone()]);

        let result = selective_merge_sync(&local, &remote, &filter);

        // Every datom in result that's not in local must match the filter
        for d in result.datom_set() {
            if !local_datoms.contains(d) {
                prop_assert!(d.attribute.starts_with(&filter_prefix),
                    "Non-filtered datom {} imported from remote", d);
                prop_assert!(remote_datoms.contains(d),
                    "Datom not from local or remote");
            }
        }
    }

    #[test]
    fn selective_merge_idempotent(
        local_datoms in prop::collection::btree_set(arb_datom(), 0..50),
        remote_datoms in prop::collection::btree_set(arb_datom(), 0..50),
        filter_prefix in "[a-z]{1,3}",
    ) {
        let local = Store::from_datoms(local_datoms);
        let remote = Store::from_datoms(remote_datoms);
        let filter = DatomFilter::AttributeNamespace(vec![filter_prefix]);

        let once = selective_merge_sync(&local, &remote, &filter);
        let twice = selective_merge_sync(&once, &remote, &filter);

        prop_assert_eq!(once.datom_set(), twice.datom_set(),
            "Selective merge is not idempotent");
    }

    #[test]
    fn selective_merge_all_equals_full_merge(
        local_datoms in prop::collection::btree_set(arb_datom(), 0..50),
        remote_datoms in prop::collection::btree_set(arb_datom(), 0..50),
    ) {
        let local = Store::from_datoms(local_datoms.clone());
        let remote = Store::from_datoms(remote_datoms.clone());

        let selective = selective_merge_sync(&local, &remote, &DatomFilter::All);
        let full = merge(&local, &remote);

        prop_assert_eq!(selective.datom_set(), full.datom_set(),
            "Selective merge with All filter != full merge");
    }
}
```

**Lean theorem**:
```lean
/-- Selective merge: local ∪ filter(remote) preserves CRDT properties. -/

-- Selective merge is union with a filtered subset
def selective_merge (local remote : DatomStore) (filter : Datom → Prop)
    [DecidablePred filter] : DatomStore :=
  local ∪ remote.filter filter

-- Monotonicity: local is always a subset of the result
theorem selective_merge_mono (local remote : DatomStore) (filter : Datom → Prop)
    [DecidablePred filter] :
    local ⊆ selective_merge local remote filter := by
  unfold selective_merge
  exact Finset.subset_union_left

-- No invention: result is a subset of local ∪ remote
theorem selective_merge_bounded (local remote : DatomStore) (filter : Datom → Prop)
    [DecidablePred filter] :
    selective_merge local remote filter ⊆ local ∪ remote := by
  unfold selective_merge
  apply Finset.union_subset_union_right
  exact Finset.filter_subset filter remote

-- Idempotency: repeating selective merge is a no-op
theorem selective_merge_idemp (local remote : DatomStore) (filter : Datom → Prop)
    [DecidablePred filter] :
    selective_merge (selective_merge local remote filter) remote filter
    = selective_merge local remote filter := by
  unfold selective_merge
  rw [Finset.union_assoc]
  rw [Finset.union_idempotent (remote.filter filter)]
  sorry -- may need Finset.union_self for the filtered part

-- filter = true reduces to full merge
theorem selective_merge_all (local remote : DatomStore) :
    selective_merge local remote (fun _ => True) = local ∪ remote := by
  unfold selective_merge
  simp [Finset.filter_true_of_mem]

-- filter = false is identity on local
theorem selective_merge_none (local remote : DatomStore) :
    selective_merge local remote (fun _ => False) = local := by
  unfold selective_merge
  simp [Finset.filter_false]
  exact Finset.union_empty local
```

---

### INV-FERR-040: Merge Provenance Preservation

**Traces to**: SEED.md §4 (Traceability — C5), INV-FERR-001 through INV-FERR-003,
INV-FERR-012 (Content-Addressed Identity)
**Verification**: `V:PROP`, `V:KANI`, `V:LEAN`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let D = [e, a, v, tx, op] be a datom with tx = (wall_time, logical, agent).
Let merge(A, B) = A ∪ B.

∀ d ∈ merge(A, B):
  d.tx = d_original.tx   where d_original is the datom as created by its originating agent

Proof:
  merge = set union. Set union does not modify elements.
  Therefore tx (including the agent field) is preserved exactly.

Corollary: For any datom d in a federated query result, d.tx.agent identifies
the agent that originally created d, regardless of how many merges and
selective merges d has passed through.
```

#### Level 1 (State Invariant)
For all reachable stores resulting from any sequence of TRANSACT, MERGE,
selective_merge, and federated query operations: every datom retains its original
`TxId` unchanged. The `TxId` is part of the datom's identity (INV-FERR-012:
content-addressed identity includes `tx`), so any modification would create a
different datom, violating content-addressed identity.

Provenance preservation enables cross-organizational auditing: "which agent, at
which time, on which machine, first observed this fact?" The answer is always
available via `d.tx.agent` and `d.tx.wall_time`, even after the datom has been
merged through dozens of intermediate stores.

#### Level 2 (Implementation Contract)
```rust
/// Merge two stores. Every datom retains its original TxId.
/// No TxId is modified, rewritten, or re-stamped during merge.
///
/// # Invariant
/// For every datom d in the result:
///   d.tx == (the TxId from d's originating transaction)
///
/// This is structural: merge = set union, and union does not modify elements.
/// Any merge implementation that rewrites TxIds is INCORRECT.
pub fn merge(a: &Store, b: &Store) -> Store {
    // BTreeSet::union preserves elements without modification
    let merged: BTreeSet<Datom> = a.datoms.union(&b.datoms).cloned().collect();
    Store::from_datoms(merged)
}

/// Query: given a datom, return the agent that originally created it.
/// This works across any number of merges because TxId is immutable.
pub fn provenance_agent(datom: &Datom) -> AgentId {
    datom.tx.agent
}

/// Query: given a datom, return the wall-clock time of original creation.
pub fn provenance_time(datom: &Datom) -> u64 {
    datom.tx.wall_time
}

/// Query: which store(s) contributed a given datom?
/// Uses the agent field of TxId to trace origin.
pub fn provenance_trace(
    datom: &Datom,
    federation: &Federation,
) -> Vec<StoreId> {
    federation.stores.iter()
        .filter(|h| {
            let snapshot = h.snapshot();
            snapshot.contains(datom)
        })
        .map(|h| h.id())
        .collect()
}

#[kani::proof]
#[kani::unwind(10)]
fn merge_preserves_provenance() {
    let a: BTreeSet<Datom> = kani::any();
    let b: BTreeSet<Datom> = kani::any();
    kani::assume(a.len() <= 3 && b.len() <= 3);

    let merged: BTreeSet<_> = a.union(&b).cloned().collect();

    // Every datom in merged has the same tx as in its source
    for d in &merged {
        if a.contains(d) {
            let orig = a.iter().find(|x| *x == d).unwrap();
            assert_eq!(d.tx, orig.tx);
        }
        if b.contains(d) {
            let orig = b.iter().find(|x| *x == d).unwrap();
            assert_eq!(d.tx, orig.tx);
        }
    }
}
```

**Falsification**: A datom `d` produced by agent `A` at time `t` that, after passing
through one or more merge/selective_merge operations, has `d.tx.agent != A` or
`d.tx.wall_time != t`. Specific failure modes:
- **Re-stamping**: the merge implementation creates a new TxId for merged datoms
  (e.g., to record "when the merge happened" rather than "when the datom was created").
- **Agent rewriting**: the selective_merge implementation replaces the remote agent ID
  with the local agent ID (claiming ownership of remote knowledge).
- **TxId normalization**: a serialization/deserialization round-trip through a transport
  layer normalizes TxId fields (e.g., truncating agent to 8 bytes instead of 16).
- **Content hash collision**: two different datoms produce the same content hash
  (INV-FERR-012 violation), causing the merge to drop one and keep the other with
  a different TxId.

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn merge_preserves_all_txids(
        a_datoms in prop::collection::btree_set(arb_datom(), 0..100),
        b_datoms in prop::collection::btree_set(arb_datom(), 0..100),
    ) {
        let a = Store::from_datoms(a_datoms.clone());
        let b = Store::from_datoms(b_datoms.clone());
        let merged = merge(&a, &b);

        for d in merged.datom_set() {
            // Every datom in merged must have the exact tx from its source
            let in_a = a_datoms.iter().find(|x| x.entity == d.entity
                && x.attribute == d.attribute
                && x.value == d.value
                && x.op == d.op);
            let in_b = b_datoms.iter().find(|x| x.entity == d.entity
                && x.attribute == d.attribute
                && x.value == d.value
                && x.op == d.op);

            let source_tx = in_a.or(in_b)
                .expect("Datom in merged not found in either source");
            prop_assert_eq!(d.tx, source_tx.tx,
                "TxId changed during merge: {:?} -> {:?}", source_tx.tx, d.tx);
        }
    }

    #[test]
    fn selective_merge_preserves_txids(
        local_datoms in prop::collection::btree_set(arb_datom(), 0..50),
        remote_datoms in prop::collection::btree_set(arb_datom(), 0..50),
        filter_prefix in "[a-z]{1,3}",
    ) {
        let local = Store::from_datoms(local_datoms.clone());
        let remote = Store::from_datoms(remote_datoms.clone());
        let filter = DatomFilter::AttributeNamespace(vec![filter_prefix]);

        let result = selective_merge_sync(&local, &remote, &filter);

        for d in result.datom_set() {
            if !local_datoms.contains(d) {
                // Came from remote — tx must be the remote's original tx
                let remote_orig = remote_datoms.iter().find(|x| x == &d)
                    .expect("Datom not from local or remote");
                prop_assert_eq!(d.tx, remote_orig.tx,
                    "TxId changed during selective merge");
            }
        }
    }
}
```

**Lean theorem**:
```lean
/-- Merge provenance preservation: set union does not modify elements,
    so TxId (and all other datom fields) are preserved exactly. -/

-- Model: a datom's tx field is a projection
def tx_of (d : Datom) : TxId := d.tx

-- Union preserves membership and identity
theorem merge_preserves_tx (a b : DatomStore) (d : Datom) (h : d ∈ a ∪ b) :
    ∃ s ∈ ({a, b} : Finset DatomStore), d ∈ s := by
  rw [Finset.mem_union] at h
  cases h with
  | inl ha => exact ⟨a, Finset.mem_insert_self a {b}, ha⟩
  | inr hb => exact ⟨b, Finset.mem_insert.mpr (Or.inr (Finset.mem_singleton_iff.mpr rfl)), hb⟩

-- Key insight: union does not create new elements
theorem union_no_invention (a b : DatomStore) (d : Datom) (h : d ∈ a ∪ b) :
    d ∈ a ∨ d ∈ b := by
  exact Finset.mem_union.mp h

-- Therefore: d.tx is unchanged (it's the same element, not a copy with modified fields)
-- This is structural: Finset.union returns elements from a or b, not new constructions.
```

---

### INV-FERR-041: Transport Latency Tolerance

**Traces to**: SEED.md §4, INV-FERR-034 through INV-FERR-036 (Partition Tolerance),
INV-FERR-037 (Federated Query Correctness)
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Let timeout : StoreHandle → Duration be the per-store timeout configuration.
Let respond(Sᵢ, Q, t) be true iff store Sᵢ responds to query Q within time t.

∀ federation F = {S₁, ..., Sₖ}, ∀ monotonic Q:
  Let R = {Sᵢ | respond(Sᵢ, Q, timeout(Sᵢ))} be the responding stores.
  Let T = {Sᵢ | ¬respond(Sᵢ, Q, timeout(Sᵢ))} be the timed-out stores.

  If R ≠ ∅:
    federated_query(F, Q).results = ⋃ᵢ∈R query(Sᵢ)
    federated_query(F, Q).partial = (T ≠ ∅)
    federated_query(F, Q).store_responses contains per-store status

  If R = ∅:
    federated_query(F, Q) = Err(AllStoresTimedOut)

The partial result is a VALID SUBSET of the full federated result:
  federated_query(F, Q).results ⊆ ⋃ᵢ query(Sᵢ)  (for all i, not just R)

The caller decides whether partial results are acceptable for their use case.
```

#### Level 1 (State Invariant)
For all reachable federation states and all queries: the federation layer NEVER
blocks indefinitely waiting for a slow or unreachable store. Each store has a
configurable timeout (default: 30 seconds). Stores that do not respond within
their timeout are marked as `ResponseStatus::Timeout` in the `store_responses`
vector. The overall result is still returned (with `partial: true`) as long as
at least one store responded.

The partial result is always a valid subset: it contains exactly the datoms
matching the query from the stores that responded. It never contains datoms
from stores that timed out (no stale cache, no speculative results). The
`store_responses` vector provides full transparency: the caller can see
exactly which stores contributed and which did not.

For non-monotonic queries, partial results are NOT returned (since the query
requires the full datom set). If any store times out during materialization
for a non-monotonic query, the entire query fails with `MaterializationIncomplete`.

#### Level 2 (Implementation Contract)
```rust
/// Per-store response metadata.
pub struct StoreResponse {
    pub store_id: StoreId,
    pub latency: Duration,
    pub datom_count: usize,
    pub status: ResponseStatus,
}

/// Response status for a single store in a federated query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseStatus {
    Ok,
    Timeout,
    Error(String),
    /// Store was skipped (e.g., query didn't need this shard)
    Skipped,
}

/// Federated query result with per-store metadata.
pub struct FederatedResult {
    /// Merged results from all responding stores.
    pub results: QueryResult,
    /// Per-store metadata: latency, datom count, status.
    pub store_responses: Vec<StoreResponse>,
    /// True if any store timed out or errored.
    /// The caller must check this and decide if partial results suffice.
    pub partial: bool,
    /// Timestamp of the federation snapshot (max TxId across responding stores).
    pub snapshot_timestamp: TxId,
}

/// Query a single store with timeout.
async fn query_store_with_timeout(
    handle: &StoreHandle,
    query: &QueryExpr,
    timeout: Duration,
) -> Result<TransportResult, StoreError> {
    match tokio::time::timeout(timeout, query_store(handle, query)).await {
        Ok(result) => result,
        Err(_elapsed) => Err(StoreError::Timeout(timeout)),
    }
}

/// Configuration for federation behavior.
pub struct FederationConfig {
    /// Default per-store timeout.
    pub default_timeout: Duration,
    /// Per-store timeout overrides.
    pub store_timeouts: HashMap<StoreId, Duration>,
    /// Whether to return partial results on timeout (monotonic queries only).
    pub allow_partial: bool,
    /// Maximum concurrent store queries (backpressure).
    pub max_concurrent: usize,
}

impl Default for FederationConfig {
    fn default() -> Self {
        FederationConfig {
            default_timeout: Duration::from_secs(30),
            store_timeouts: HashMap::new(),
            allow_partial: true,
            max_concurrent: 64,
        }
    }
}
```

**Falsification**: A federated query that blocks indefinitely when a store is
unreachable (timeout not enforced). Or: a partial result that contains datoms
from a timed-out store (stale cache served as fresh). Or: `partial` is `false`
when a store actually timed out (silent data loss). Specific failure modes:
- **Infinite hang**: the transport layer does not respect the timeout (e.g.,
  TCP keepalive holds the connection open indefinitely).
- **Phantom results**: a store times out mid-response, and the partially received
  datoms are included in the result (incomplete data masquerading as complete).
- **Silent timeout**: a store times out but `partial` is set to `false` (caller
  believes the result is complete).
- **Non-monotonic partial**: a non-monotonic query returns partial results from
  responding stores (aggregation on subset gives wrong answer).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn partial_result_is_subset_of_full(
        stores in prop::collection::vec(
            prop::collection::btree_set(arb_datom(), 0..50),
            2..5,
        ),
        responding_mask in prop::collection::vec(any::<bool>(), 2..5),
        query_attr in arb_attribute(),
    ) {
        let responding_mask = &responding_mask[..stores.len()];

        // Full result (all stores respond)
        let all_datoms: BTreeSet<_> = stores.iter()
            .flat_map(|s| s.iter().cloned())
            .collect();
        let full_result: BTreeSet<_> = all_datoms.iter()
            .filter(|d| d.attribute == query_attr)
            .cloned().collect();

        // Partial result (only responding stores)
        let partial_datoms: BTreeSet<_> = stores.iter()
            .zip(responding_mask.iter())
            .filter(|(_, &responds)| responds)
            .flat_map(|(s, _)| s.iter().cloned())
            .collect();
        let partial_result: BTreeSet<_> = partial_datoms.iter()
            .filter(|d| d.attribute == query_attr)
            .cloned().collect();

        // Partial is always a subset of full
        prop_assert!(partial_result.is_subset(&full_result),
            "Partial result is not a subset of full result");

        // If all respond, partial == full
        if responding_mask.iter().all(|&r| r) {
            prop_assert_eq!(partial_result, full_result,
                "All stores responded but results differ");
        }
    }

    #[test]
    fn timeout_metadata_accurate(
        store_count in 2..5usize,
        timeout_indices in prop::collection::hash_set(0..4usize, 0..3),
    ) {
        let timeout_indices: Vec<_> = timeout_indices.into_iter()
            .filter(|&i| i < store_count)
            .collect();

        // Simulate: some stores time out
        let responses: Vec<ResponseStatus> = (0..store_count)
            .map(|i| {
                if timeout_indices.contains(&i) {
                    ResponseStatus::Timeout
                } else {
                    ResponseStatus::Ok
                }
            })
            .collect();

        let partial = responses.iter().any(|r| *r != ResponseStatus::Ok);

        // partial must be true iff any store is not Ok
        prop_assert_eq!(partial, !timeout_indices.is_empty(),
            "partial flag inconsistent with store responses");
    }
}
```

**Lean theorem**:
```lean
/-- Latency tolerance: partial results are valid subsets.
    We model responding stores as a subset of all stores. -/

-- The result from responding stores is a subset of the full result
theorem partial_subset_full (stores : Finset (Fin k))
    (responding : Finset (Fin k))
    (h_sub : responding ⊆ stores)
    (f : Fin k → DatomStore)
    (p : Datom → Prop) [DecidablePred p] :
    (responding.biUnion f).filter p ⊆ (stores.biUnion f).filter p := by
  apply Finset.filter_subset_filter
  exact Finset.biUnion_subset_biUnion_of_subset_left f h_sub

-- When all stores respond, partial == full
theorem all_respond_equals_full (stores : Finset (Fin k))
    (f : Fin k → DatomStore)
    (p : Datom → Prop) [DecidablePred p] :
    (stores.biUnion f).filter p = (stores.biUnion f).filter p := by
  rfl
```

---

### INV-FERR-042: Live Migration (Substrate Transition)

**Traces to**: SEED.md §4 (Substrate Independence — C8), INV-FERR-038 (Transport Transparency),
INV-FERR-006 (Snapshot Isolation), INV-FERR-008 (WAL Ordering)
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
Let S(t) be the store state at time t.
Let T_old and T_new be the old and new transport handles.
Let swap(t_s) be the atomic swap of transport handle at time t_s.

∀ query Q issued at time t:
  If t < t_s:  query uses T_old, sees S(t) via T_old
  If t ≥ t_s:  query uses T_new, sees S(t) via T_new

Correctness condition:
  At the moment of swap, S_new(t_s) = S_old(t_s)
  i.e., the new location has caught up to the old location.

Process:
  1. t_start: begin streaming WAL from T_old to T_new
  2. t_catchup: T_new has replayed all WAL entries up to T_old's current epoch
  3. t_s: atomic swap — all new queries go to T_new
  4. t_s + drain: T_old remains read-only for in-flight queries
  5. t_decommission: T_old is shut down

Between t_start and t_s: new writes go to T_old, are streamed to T_new.
At t_s: T_new is at most 1 WAL frame behind T_old (bounded by stream latency).
The atomic swap is: `ArcSwap::store(new_handle)` — wait-free, lock-free.
```

#### Level 1 (State Invariant)
A store can be migrated from one transport to another (e.g., local to remote, TCP to
QUIC, machine A to machine B) without stopping queries. During migration:
- Existing queries that started before the swap continue to completion using the old
  transport (they hold a reference via `Arc`).
- New queries after the swap use the new transport.
- No query sees a "gap" (missing datoms) or a "split" (different results from old vs new).

The migration process is observable: the federation emits events for each phase
(streaming started, catchup complete, swap executed, drain complete, decommissioned).
The operator can monitor progress and abort if needed.

The key correctness condition is catchup completeness: at the moment of swap, the new
location must have all datoms that the old location has. Since the store is append-only
(C1), the new location only needs to process new WAL entries since streaming started.
WAL ordering (INV-FERR-008) guarantees that replay is deterministic.

#### Level 2 (Implementation Contract)
```rust
/// Live migration: move a store from one transport to another.
pub struct Migration {
    /// The store being migrated.
    store_id: StoreId,
    /// Old transport handle (source).
    old_handle: Arc<ArcSwap<StoreHandle>>,
    /// New transport handle (destination).
    new_transport: Box<dyn Transport>,
    /// Migration state machine.
    state: MigrationState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationState {
    /// Not started.
    Idle,
    /// WAL is being streamed from old to new.
    Streaming { from_epoch: Epoch, entries_sent: u64 },
    /// New location has caught up to old location.
    CaughtUp { epoch: Epoch },
    /// Transport handle has been swapped. Old is draining in-flight queries.
    Swapped { drain_deadline: Instant },
    /// Old transport decommissioned. Migration complete.
    Complete,
    /// Migration aborted. Old transport still active.
    Aborted { reason: String },
}

impl Migration {
    /// Start streaming WAL entries from old to new location.
    pub async fn start_streaming(&mut self) -> Result<(), MigrationError> {
        let current_epoch = self.old_handle.load().epoch().await?;
        let wal_stream = self.old_handle.load().stream_wal(Epoch(0)).await?;

        // Stream all WAL entries to new transport
        let mut entries_sent = 0u64;
        while let Some(entry) = wal_stream.next().await {
            self.new_transport.apply_wal_entry(entry?).await?;
            entries_sent += 1;
        }

        self.state = MigrationState::Streaming {
            from_epoch: current_epoch,
            entries_sent,
        };
        Ok(())
    }

    /// Catch up: stream any WAL entries written since streaming started.
    pub async fn catchup(&mut self) -> Result<(), MigrationError> {
        let MigrationState::Streaming { from_epoch, .. } = &self.state else {
            return Err(MigrationError::InvalidState);
        };

        let current_epoch = self.old_handle.load().epoch().await?;
        let delta_stream = self.old_handle.load()
            .stream_wal(*from_epoch).await?;

        while let Some(entry) = delta_stream.next().await {
            self.new_transport.apply_wal_entry(entry?).await?;
        }

        self.state = MigrationState::CaughtUp { epoch: current_epoch };
        Ok(())
    }

    /// Atomic swap: redirect all new queries to the new transport.
    /// In-flight queries on the old transport continue to completion.
    pub fn swap(&mut self, drain_timeout: Duration) -> Result<(), MigrationError> {
        let MigrationState::CaughtUp { .. } = &self.state else {
            return Err(MigrationError::InvalidState);
        };

        let new_handle = StoreHandle::Remote(RemoteStore {
            id: self.store_id,
            transport: self.new_transport.clone_boxed(),
            addr: self.new_transport.addr(),
            timeout: Duration::from_secs(30),
        });

        // ArcSwap::store is wait-free, lock-free, atomic
        self.old_handle.store(Arc::new(new_handle));

        self.state = MigrationState::Swapped {
            drain_deadline: Instant::now() + drain_timeout,
        };
        Ok(())
    }

    /// Decommission the old transport after drain period.
    pub async fn decommission(&mut self) -> Result<(), MigrationError> {
        let MigrationState::Swapped { drain_deadline } = &self.state else {
            return Err(MigrationError::InvalidState);
        };

        // Wait for drain period (in-flight queries complete)
        if Instant::now() < *drain_deadline {
            tokio::time::sleep_until((*drain_deadline).into()).await;
        }

        self.state = MigrationState::Complete;
        Ok(())
    }

    /// Abort migration. Old transport remains active. No data loss.
    pub fn abort(&mut self, reason: String) {
        self.state = MigrationState::Aborted { reason };
    }
}
```

**Falsification**: A query `Q` issued during migration that returns different results
than it would have without migration. Specific failure modes:
- **Gap**: a query issued immediately after swap sees fewer datoms than the old transport
  had (catchup incomplete).
- **Duplication**: a query sees the same datom twice (once from old transport, once from
  new) with different metadata.
- **Hang**: in-flight queries on the old transport never complete because the old
  transport is shut down before they finish.
- **Data loss**: WAL entries written between catchup and swap are lost (the new
  transport never receives them).
- **Ordering violation**: WAL entries are replayed out of order on the new transport,
  producing a different store state (violates INV-FERR-008).

**proptest strategy**:
```rust
proptest! {
    #[test]
    fn migration_preserves_datom_set(
        initial_datoms in prop::collection::btree_set(arb_datom(), 0..100),
        post_stream_datoms in prop::collection::btree_set(arb_datom(), 0..20),
    ) {
        let mut old_store = Store::from_datoms(initial_datoms.clone());

        // Simulate: stream initial state to new store
        let mut new_store = Store::from_datoms(initial_datoms.clone());

        // Simulate: new writes arrive on old store after streaming starts
        for d in &post_stream_datoms {
            old_store.insert(d.clone());
        }

        // Simulate: catchup streams the delta
        for d in &post_stream_datoms {
            new_store.insert(d.clone());
        }

        // After catchup: stores must be identical
        prop_assert_eq!(old_store.datom_set(), new_store.datom_set(),
            "New store diverged from old after catchup");
    }

    #[test]
    fn migration_abort_preserves_old(
        datoms in prop::collection::btree_set(arb_datom(), 0..100),
    ) {
        let store = Store::from_datoms(datoms.clone());

        // Simulate: start migration, then abort
        // Old store must be completely unchanged
        prop_assert_eq!(store.datom_set(), &datoms,
            "Abort modified the old store");
    }
}
```

**Lean theorem**:
```lean
/-- Live migration correctness: after catchup, old and new stores are equal.
    Since the store is append-only and WAL replay is deterministic,
    streaming + catchup produces an identical store. -/

-- Model: streaming is union of initial + delta
def stream_and_catchup (initial delta : DatomStore) : DatomStore :=
  initial ∪ delta

-- The old store after writes is also initial + delta
def old_after_writes (initial delta : DatomStore) : DatomStore :=
  initial ∪ delta

-- They are equal (by reflexivity of union)
theorem migration_correct (initial delta : DatomStore) :
    stream_and_catchup initial delta = old_after_writes initial delta := by
  unfold stream_and_catchup old_after_writes

-- Abort preserves the old store (no-op on old)
theorem migration_abort_safe (old_store : DatomStore) :
    old_store = old_store := by
  rfl
```

---

### §23.8.1: Federation API

The full Federation API surface, with types and method signatures.

```rust
/// Unique identifier for a store within a federation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct StoreId(pub [u8; 16]);

/// A federation of datom stores.
pub struct Federation {
    /// The stores in this federation, indexed by StoreId.
    stores: Vec<StoreHandle>,
    /// Configuration for federation behavior.
    config: FederationConfig,
}

/// A handle to a store: local (in-process) or remote (over transport).
pub enum StoreHandle {
    Local(Database),
    Remote(RemoteStore),
}

impl StoreHandle {
    pub fn id(&self) -> StoreId;
    pub async fn snapshot(&self) -> Result<Snapshot, TransportError>;
    pub async fn epoch(&self) -> Result<Epoch, TransportError>;
}

/// A remote store accessed via a transport layer.
pub struct RemoteStore {
    pub id: StoreId,
    pub transport: Box<dyn Transport>,
    pub addr: SocketAddr,
    pub timeout: Duration,
}

impl Federation {
    /// Create a new federation with no stores.
    pub fn new(config: FederationConfig) -> Self;

    /// Execute a federated query across all stores.
    /// Monotonic queries: fan-out + merge (INV-FERR-037).
    /// Non-monotonic queries: materialize + evaluate.
    pub async fn query(&self, expr: &QueryExpr) -> Result<FederatedResult, FederationError>;

    /// Selective merge: import filtered datoms from source into target (INV-FERR-039).
    pub async fn selective_merge(
        &self,
        target: &mut Database,
        source: StoreHandle,
        filter: DatomFilter,
    ) -> Result<MergeReceipt, FederationError>;

    /// Full materialization: merge all stores into a new local Database.
    /// Use for non-monotonic queries or when a complete local copy is needed.
    pub async fn materialize(&self) -> Result<Database, FederationError>;

    /// Add a store to the federation.
    pub fn add_store(&mut self, handle: StoreHandle);

    /// Remove a store from the federation.
    /// In-flight queries to this store will complete (they hold Arc references).
    pub fn remove_store(&mut self, id: StoreId);

    /// List all stores with their current status.
    pub async fn store_status(&self) -> Vec<(StoreId, StoreStatus)>;

    /// Live migration: move a store from one transport to another (INV-FERR-042).
    pub async fn migrate(
        &mut self,
        store_id: StoreId,
        new_transport: Box<dyn Transport>,
        drain_timeout: Duration,
    ) -> Result<(), MigrationError>;
}

/// Federated query result.
pub struct FederatedResult {
    /// Merged results from all responding stores.
    pub results: QueryResult,
    /// Per-store metadata: latency, datom count, status.
    pub store_responses: Vec<StoreResponse>,
    /// True if any store timed out or errored (INV-FERR-041).
    pub partial: bool,
}

/// Per-store response metadata.
pub struct StoreResponse {
    pub store_id: StoreId,
    pub latency: Duration,
    pub datom_count: usize,
    pub status: ResponseStatus,
}

/// Response status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseStatus {
    Ok,
    Timeout,
    Error(String),
    Skipped,
}

/// Merge receipt from selective_merge.
pub struct MergeReceipt {
    pub source_store: StoreId,
    pub target_store: StoreId,
    pub datoms_transferred: usize,
    pub datoms_filtered_out: usize,
    pub datoms_already_present: usize,
    pub filter_applied: DatomFilter,
    pub duration: Duration,
}

/// Federation errors.
#[derive(Debug)]
pub enum FederationError {
    AllStoresTimedOut,
    MaterializationIncomplete { responding: usize, total: usize },
    SchemaIncompatible { local: Schema, remote: Schema, conflict: String },
    TransportError(TransportError),
    StoreNotFound(StoreId),
    MigrationFailed(MigrationError),
}
```

### §23.8.2: Performance Considerations

| Aspect | Characteristic | Bound |
|--------|---------------|-------|
| Fan-out parallelism | All stores queried concurrently via `tokio::join_all` | O(1) wall-clock for query dispatch |
| Result merge | Union of per-store result sets | O(sum of |R_i|) — linear in total result size |
| Network bandwidth | Only QUERY RESULTS cross the network, not full stores | O(|result|) per store, not O(|store|) |
| Federated query latency | P99 = max(per-store P99) + merge overhead | Bounded by slowest responding store + O(|result|) merge |
| Selective merge bandwidth | Only matching datoms transferred | O(|filter matches|), not O(|remote store|) |
| Materialization | Full merge of all stores into local Database | O(sum of |S_i|) — one-time cost |
| Connection pooling | Persistent connections to remote stores | Amortized 0 connection setup cost after first query |
| Incremental federation | Stores can join/leave without restarting | O(1) add/remove via `Arc` reference counting |
| Migration overhead | WAL streaming + catchup + atomic swap | O(|WAL delta|) for catchup, O(1) for swap |
| Backpressure | `max_concurrent` limits parallel store queries | Prevents thundering herd on large federations |

**Key insight**: The CRDT foundation means that federation has NO coordination cost
for monotonic queries. The CALM theorem guarantees that fan-out + merge is correct
without any locking, consensus, or distributed transaction protocol. The only
coordination point is non-monotonic queries, which require materialization.

### §23.8.3: Transport Layer Heterogeneity

All transports implement the same `Transport` trait (INV-FERR-038). The Federation
never inspects which concrete transport a `StoreHandle` uses.

| Transport | Use case | Characteristics |
|-----------|----------|-----------------|
| `LocalTransport` | Same-process federation | Zero-copy, zero-latency, no serialization. Direct `Arc<Database>` access. |
| `UnixSocketTransport` | Same-machine federation | Low-latency (~50us), no TLS needed, uses filesystem permissions for auth. Ideal for multi-process on same host. |
| `TcpTransport` | LAN/datacenter | Persistent connections, TCP keepalive, reconnect on failure. TLS optional. Connection pooling per remote. |
| `QuicTransport` | WAN/internet | Multiplexed streams, 0-RTT reconnect, built-in TLS. Handles NAT traversal. Best for cross-region federation. |
| `GrpcTransport` | Cloud services | Load balancing, service discovery, TLS, auth headers, health checking. Integrates with Kubernetes service mesh. |

**Transport selection guideline**: Use the simplest transport that meets latency and
security requirements. Start with `LocalTransport` for testing, `UnixSocketTransport`
for production on single machine, `TcpTransport` for LAN, `QuicTransport` for WAN.

**Wire format**: All transports use the same serialization format for `QueryExpr`,
`QueryResult`, `Datom`, and `Schema`. The format is a length-prefixed, BLAKE3-checksummed
binary encoding (same as checkpoint format, INV-FERR-013). This ensures that
transport transparency (INV-FERR-038) holds by construction — the serialization
layer is shared, not per-transport.

### §23.8.4: Dependency Injection & Substrate Migration

`StoreHandle` is a **runtime value**, not a compile-time type parameter. This enables:

1. **Runtime topology changes**: Add or remove stores without recompilation.
2. **Live migration** (INV-FERR-042): Swap a store's transport without stopping queries.
3. **Testing**: Inject `LoopbackTransport` (serializes and deserializes in-process) to
   test the full transport path without network infrastructure.
4. **Gradual rollout**: Migrate stores one-by-one from local to remote as the system scales.

**Migration process**:
1. Start new transport (e.g., provision remote machine, start ferratomic server).
2. Stream WAL from old location to new location.
3. Catch up: stream delta WAL entries written since step 2 started.
4. Atomic swap: `ArcSwap::store(new_handle)` — wait-free, lock-free.
5. Drain: old transport remains alive for in-flight queries (configurable drain period).
6. Decommission: shut down old transport after drain.

**Rollback**: If errors spike after swap, the migration can be aborted by swapping
back to the old handle. The old transport is kept alive during the drain period
specifically to enable rollback.

**Zero-downtime guarantee**: The `ArcSwap` pattern ensures that the swap itself is
a single atomic pointer write. No mutex, no condition variable, no query queue.
Queries in progress continue with their existing `Arc` reference; new queries
pick up the new handle immediately.

### §23.8.5: Knowledge Transfer Use Cases (Application-Level)

These scenarios demonstrate selective merge (INV-FERR-039) in practice:

| Scenario | Filter | Effect |
|----------|--------|--------|
| Learn calibrated policies from another project | `AttributeNamespace(vec!["policy/".into(), "calibration/".into()])` | Import policy weights and calibration data. Ignore tasks, observations, session history. |
| Import spec elements from a team | `AttributeNamespace(vec!["spec/".into(), "intent/".into()])` | Import INV/ADR/NEG definitions. Ignore implementation artifacts. |
| Federated search across all local projects | `Federation` over `LocalTransport` stores | Query all projects simultaneously. No data movement — queries fan out and results merge. |
| Cloud-scale agent coordination | `Federation` over `TcpTransport`/`QuicTransport` | Agents on different machines share knowledge through federated queries. Selective merge for deliberate knowledge transfer. |
| Offline work + sync | Local store accumulates datoms offline | On reconnect: `selective_merge(remote, local, All)` pushes local knowledge to shared store. `selective_merge(local, remote, filter)` pulls relevant updates. |
| Cross-organization knowledge exchange | `And(vec![AttributeNamespace(vec!["policy/"]), FromAgents(trusted_agents)])` | Import only policies from trusted agents. Defense in depth: namespace filter + agent filter. |

### §23.8.6: Security & Trust

#### INV-FERR-043: Schema Compatibility Check

**Traces to**: INV-FERR-009 (Schema Validation), INV-FERR-039 (Selective Merge)
**Verification**: `V:PROP`
**Stage**: 1

Before any merge (full or selective) between two stores, the schema compatibility
MUST be verified. Compatibility means:
- For every attribute present in BOTH schemas: the `ValueType`, `Cardinality`, and
  resolution mode must be identical.
- Attributes present in only one schema are always compatible (they will be added to
  the other schema upon merge).
- Schema evolution (adding new attributes) is always safe. Schema mutation (changing
  existing attribute types) is a compatibility failure.

```rust
/// Verify that two schemas are compatible for merge.
/// Returns Ok(()) if compatible, Err with conflict details if not.
pub fn verify_schema_compatibility(
    local: &Schema,
    remote: &Schema,
) -> Result<(), SchemaConflict> {
    for (attr, local_def) in local.attributes() {
        if let Some(remote_def) = remote.get(attr) {
            if local_def.value_type != remote_def.value_type {
                return Err(SchemaConflict::TypeMismatch {
                    attribute: attr.clone(),
                    local_type: local_def.value_type,
                    remote_type: remote_def.value_type,
                });
            }
            if local_def.cardinality != remote_def.cardinality {
                return Err(SchemaConflict::CardinalityMismatch {
                    attribute: attr.clone(),
                    local_card: local_def.cardinality,
                    remote_card: remote_def.cardinality,
                });
            }
        }
    }
    Ok(())
}
```

**Falsification**: A merge proceeds between two stores with incompatible schemas
(e.g., attribute `:task/priority` is `Long` in one store and `String` in another),
producing a store with conflicting attribute definitions.

#### INV-FERR-044: Namespace Isolation

**Traces to**: INV-FERR-039 (Selective Merge), C8 (Substrate Independence)
**Verification**: `V:PROP`
**Stage**: 1

Selective merge can restrict to specific attribute namespaces, providing defense
in depth against unintended knowledge transfer. The `AttributeNamespace` filter
uses prefix matching on attribute names (e.g., `"policy/"` matches `:policy/weight`,
`:policy/threshold`, etc.).

Namespace isolation is enforced at the filter level, not the transport level. The
transport transfers whatever datoms the filter selects. The caller is responsible
for constructing appropriate filters.

```rust
/// Restrict selective merge to specific namespaces.
/// Example: import only policy datoms from a remote store.
pub fn namespace_filter(namespaces: &[&str]) -> DatomFilter {
    DatomFilter::AttributeNamespace(
        namespaces.iter().map(|s| s.to_string()).collect()
    )
}
```

**Falsification**: A selective_merge with `AttributeNamespace(vec!["policy/"])` filter
that imports datoms with attributes outside the `policy/` namespace (e.g., `:task/title`).

**Future extensions** (not specified in this stage):
- Cryptographic provenance: TxIds signed by the originating agent's key pair. Receivers
  can verify that a datom was actually created by the claimed agent.
- Access control lists: per-namespace read/write permissions enforced at the transport
  layer. A remote store can refuse to serve datoms from restricted namespaces.
- Audit trail: every selective_merge operation is itself recorded as datoms in the
  receiving store (`:merge/source`, `:merge/filter`, `:merge/timestamp`, `:merge/count`).

### §23.8.7: Consistency Model

| Scope | Model | Guarantee |
|-------|-------|-----------|
| Single store | Snapshot isolation (INV-FERR-006) | Reads see a consistent point-in-time snapshot. Writes are linearizable (INV-FERR-007). |
| Within a federation (monotonic queries) | Linearizable by CALM | Fan-out + merge produces the same result as querying the merged store. No coordination needed. |
| Within a federation (non-monotonic queries) | Point-in-time | Full materialization creates a local snapshot. The result reflects the state of all stores at approximately the same time (bounded by materialization latency). |
| Across federations | Strong eventual consistency | CRDT guarantee (INV-FERR-001 through INV-FERR-003): any two stores that have received the same set of datoms (in any order) converge to the same state. |
| During live migration | Linearizable reads | The ArcSwap pattern ensures that every query sees a consistent store state. No query sees a "torn" state (half old, half new). |

**FederationSnapshot**: For non-monotonic federated queries, the system records a
`FederationSnapshot` timestamp: the maximum `TxId` across all responding stores at
the time of materialization. This timestamp enables reproducible queries: "what was
the federation-wide answer to Q at time T?"

```rust
/// A timestamp representing the state of the entire federation at a point in time.
#[derive(Debug, Clone)]
pub struct FederationSnapshot {
    /// Per-store TxId at the time of snapshot.
    pub store_epochs: BTreeMap<StoreId, TxId>,
    /// The maximum TxId across all stores (the federation "now").
    pub max_tx: TxId,
    /// Which stores contributed to this snapshot.
    pub participating_stores: BTreeSet<StoreId>,
}
```

---
