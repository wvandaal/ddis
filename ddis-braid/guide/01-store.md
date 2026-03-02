# §1. STORE — Build Plan

> **Spec reference**: [spec/01-store.md](../spec/01-store.md) — read FIRST
> **Stage 0 elements**: INV-STORE-001–012, 014 (13 INV), ADR-STORE-001–012, NEG-STORE-001–005
> **Dependencies**: None (foundational namespace)
> **Cognitive mode**: Algebraic — set theory, CRDT laws, commutativity proofs

---

## §1.1 Module Structure

```
braid-kernel/src/
├── datom.rs        ← Datom, EntityId, TxId, AgentId, Op, Value, Attribute
├── store.rs        ← Store, transact, merge, genesis, indexes
└── frontier.rs     ← Frontier, HLC clock
```

### Public API Surface

```rust
// datom.rs
pub struct Datom { entity, attribute, value, tx, op }
pub struct EntityId([u8; 32]);
pub struct TxId { wall_time, logical, agent }
pub struct AgentId([u8; 16]);
pub enum Op { Assert, Retract }
pub enum Value { String, Keyword, Boolean, Long, Double, Instant, Uuid, Ref, Bytes }
pub struct Attribute(String);
pub enum ProvenanceType { Hypothesized, Inferred, Derived, Observed }

// store.rs
pub struct Store { /* opaque */ }
impl Store {
    pub fn genesis() -> Self;
    pub fn transact(&mut self, tx: Transaction<Committed>) -> Result<TxReceipt, TxApplyError>;
    pub fn merge(&mut self, other: &Store) -> MergeReceipt;
    pub fn current(&self, entity: EntityId) -> EntityView;
    pub fn as_of(&self, frontier: &Frontier) -> SnapshotView;
    pub fn len(&self) -> usize;
    pub fn datoms(&self) -> impl Iterator<Item = &Datom>;
    pub fn frontier(&self) -> &HashMap<AgentId, TxId>;
}

// Transaction typestate
pub struct Transaction<S: TxState> { /* opaque */ }
impl Transaction<Building> { fn new, assert_datom, retract_datom, commit }
impl Transaction<Committed> { fn apply (via Store::transact) }
impl Transaction<Applied> { fn tx_id, receipt }
```

---

## §1.2 Three-Box Decomposition

### Datom

**Black box** (contract):
- Immutable after construction. Five fields: `(entity, attribute, value, tx, op)`.
- Hash and Eq derive from all five fields (INV-STORE-003).
- Content-addressed: identity IS the five-tuple.

**State box** (internal design):
- No internal state transitions — a Datom is a value type.
- `Clone` + `Eq` + `Hash` + `Ord` + `Serialize`/`Deserialize`.
- Ordering: entity → attribute → value → tx → op (for BTreeSet indexing).

**Clear box** (implementation):
- Derive all traits. No custom logic except `Ord` for deterministic ordering.
- `EntityId` construction: `blake3::hash(content).into()` — no raw constructor.
- `Value::Double` wraps `ordered_float::OrderedFloat<f64>` for Ord/Eq/Hash compliance.

### Store

**Black box** (contract):
- INV-STORE-001: `∀ op: S ⊆ op(S)` — monotonic growth.
- INV-STORE-004–006: merge is commutative, associative, idempotent.
- INV-STORE-008: genesis is deterministic (constant function).
- INV-STORE-009: frontier is durable before response.
- INV-STORE-014: every command produces a transaction.

**State box** (internal design):
- `datoms: BTreeSet<Datom>` — the canonical set.
- `indexes: Indexes` — EAVT, AEVT, VAET, AVET as BTreeMaps.
- `frontier: HashMap<AgentId, TxId>` — per-agent latest tx.
- `schema: Schema` — attribute registry (delegated to schema module).
- State transitions: only `transact` and `merge` modify state. Both are `&mut self`.
- Read operations: `&self` only.

**Clear box** (implementation):
- `transact`: validate against schema → generate TxId → compute EntityId for tx metadata →
  insert datoms into BTreeSet → update indexes incrementally → update frontier → return receipt.
- `merge`: BTreeSet union → index rebuild → frontier merge (pointwise max per agent).
- `genesis`: hardcoded 17 axiomatic attributes as datoms. Verify hash matches compile-time constant.
- `as_of`: filter datoms by `d.tx <= frontier_txid` → apply resolution per attribute.
- Indexes are maintained incrementally on transact (not rebuilt from scratch).

### Transaction (Typestate)

**Black box** (contract):
- Three states: Building → Committed → Applied.
- Building: mutable, accepts datom additions.
- Committed: immutable, schema-validated, ready to apply.
- Applied: immutable, holds receipt with TxId.
- Invalid transitions are compile errors (INV-STORE-001).

**State box** (internal design):
- `datoms: Vec<Datom>` — accumulated datoms.
- `tx_data: TxData` — provenance, causal predecessors, agent, rationale.
- `_state: PhantomData<S>` — zero-sized type marker.

**Clear box** (implementation):
- `commit(schema)`: validate each datom's attribute exists in schema, validate value types match
  schema cardinality, generate TxId, seal the transaction. Return `Err(TxValidationError)` on failure.
- `apply(store)`: called by `Store::transact`. Insert datoms, update indexes.
- Builder pattern: `Transaction::new(agent).assert_datom(...).assert_datom(...).commit(schema)?`.

---

## §1.3 Type-Level Encoding

| INV | Compile-Time Guarantee | Mechanism |
|-----|----------------------|-----------|
| INV-STORE-001 | Transaction typestate prevents applying without commit | `PhantomData<S>` |
| INV-STORE-002 | EntityId has no `new(raw_bytes)` constructor | Private field + `from_content` only |
| INV-STORE-003 | Content identity via derived Hash/Eq | `#[derive(Hash, Eq, PartialEq)]` on all 5 fields |
| INV-STORE-005 | Store immutability for reads | `&Store` for reads, `&mut Store` only via `transact`/`merge` |

---

## §1.4 LLM-Facing Outputs

### Agent-Mode Output — `braid transact`

```
[STORE] Transacted {N} datoms in tx {tx_id}. Store: {total} datoms.
{summary_of_what_changed — attributes and entities affected}
---
↳ {guidance_footer}
```

### Agent-Mode Output — `braid status`

```
[STATUS] Store: {N} datoms, {M} entities. Frontier: {frontier_map}.
Schema: {attr_count} attributes ({genesis_count} axiomatic + {user_count} user-defined).
---
↳ {guidance_footer}
```

### Error Messages

- **Missing causal predecessor**: `Tx error: causal predecessor {txid} not in store — ensure predecessor was transacted first — See: INV-STORE-010`
- **Schema violation**: `Tx error: attribute {attr} not in schema — add via schema transaction first — See: INV-SCHEMA-003`
- **Type mismatch**: `Tx error: value type {got} for {attr}, expected {expected} — check schema definition — See: INV-SCHEMA-005`

---

## §1.5 Verification

### Proptest Strategies

```rust
// See guide/10-verification.md §10.4 for the full strategy hierarchy.
// Key properties for STORE:

proptest! {
    // INV-STORE-001: Append-only
    fn inv_store_001(store in arb_store(5), datoms in arb_datoms(10)) { ... }
    // INV-STORE-002: Strict growth
    fn inv_store_002(store in arb_store(5), datoms in arb_datoms(10)) { ... }
    // INV-STORE-003: Content identity
    fn inv_store_003(d1 in arb_datom(), d2 in arb_datom()) { ... }
    // INV-STORE-004: Merge commutativity
    fn inv_store_004(s1 in arb_store(3), s2 in arb_store(3)) { ... }
    // INV-STORE-005: Merge associativity
    fn inv_store_005(s1 in arb_store(2), s2 in arb_store(2), s3 in arb_store(2)) { ... }
    // INV-STORE-006: Merge idempotency
    fn inv_store_006(s in arb_store(5)) { ... }
    // INV-STORE-007: Merge monotonicity
    fn inv_store_007(s1 in arb_store(3), s2 in arb_store(3)) { ... }
    // INV-STORE-008: Genesis determinism
    fn inv_store_008() { assert_eq!(Store::genesis(), Store::genesis()); }
}
```

### Kani Harnesses

INV-STORE-001, 002, 003, 004, 005, 006, 007, 008, 010, 012 have V:KANI tags.

```rust
#[cfg(kani)]
mod kani_proofs {
    #[kani::proof]
    #[kani::unwind(8)]
    fn inv_store_004_commutative() {
        let s1: Store = kani::any();
        let s2: Store = kani::any();
        assert_eq!(s1.merge(&s2).datom_set(), s2.merge(&s1).datom_set());
    }

    #[kani::proof]
    #[kani::unwind(16)]
    fn inv_store_005_associative() {
        let s1: Store = kani::any();
        let s2: Store = kani::any();
        let s3: Store = kani::any();
        let left = s1.merge(&s2).merge(&s3);
        let right = s1.merge(&s2.merge(&s3));
        assert_eq!(left.datom_set(), right.datom_set());
    }
}
```

---

## §1.6 Implementation Checklist

- [ ] `Datom`, `EntityId`, `TxId`, `AgentId`, `Op`, `Value`, `Attribute` types defined
- [ ] `Store::genesis()` produces deterministic 17-attribute store
- [ ] `Transaction<Building/Committed/Applied>` typestate compiles
- [ ] `Store::transact()` validates and appends
- [ ] `Store::merge()` implements set union
- [ ] `Store::current()` and `Store::as_of()` query with resolution
- [ ] Indexes (EAVT, AEVT, VAET, AVET) maintained incrementally
- [ ] HLC clock generates monotonic TxIds
- [ ] Frontier updated and durable
- [ ] `cargo check` passes (Gate 1)
- [ ] All proptest properties pass (Gate 2)
- [ ] All Kani harnesses pass (Gate 3)
- [ ] Integration: genesis + transact + query round-trip works

---
