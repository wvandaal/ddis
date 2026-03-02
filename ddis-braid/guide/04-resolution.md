# §4. RESOLUTION — Build Plan

> **Spec reference**: [spec/04-resolution.md](../spec/04-resolution.md) — read FIRST
> **Stage 0 elements**: INV-RESOLUTION-001–002, 004–006, 008 (6 INV), ADR-RESOLUTION-001–004, NEG-RESOLUTION-001–003
> **Dependencies**: STORE (§1), SCHEMA (§2), QUERY (§3)
> **Cognitive mode**: Order-theoretic — lattices, partial orders, conflict predicates

---

## §4.1 Module Structure

```
braid-kernel/src/
└── resolution.rs   ← ResolutionMode, ConflictSet, resolve, LIVE index computation
```

### Public API Surface

```rust
pub enum ResolutionMode {
    Lattice(LatticeDef),
    LastWriterWins,
    MultiValue,
}

pub struct LatticeDef {
    pub elements: Vec<Value>,
    pub partial_order: fn(&Value, &Value) -> Option<Ordering>,
    pub join: fn(&Value, &Value) -> Value,
    pub bottom: Value,
}

/// A set of competing assertions for a single (entity, attribute) pair.
pub struct ConflictSet {
    pub entity:    EntityId,
    pub attribute: Attribute,
    pub assertions: Vec<(Value, TxId)>,  // (value, asserting transaction)
    pub retractions: Vec<(Value, TxId)>,
}

/// Resolve a conflict set using the attribute's resolution mode.
pub fn resolve(conflict: &ConflictSet, mode: &ResolutionMode) -> ResolvedValue;

pub enum ResolvedValue {
    Single(Value),                // LWW or Lattice result
    Multi(Vec<Value>),            // Multi-value (all unretracted)
    Conflict(Vec<(Value, TxId)>), // Unresolvable (conservative detection)
}

/// Conflict predicate: does this (entity, attribute) have unresolved conflict?
pub fn has_conflict(conflict: &ConflictSet, mode: &ResolutionMode) -> bool;

/// Compute the LIVE view of an entity by resolving all attributes.
pub fn live_entity(store: &Store, entity: EntityId) -> HashMap<Attribute, ResolvedValue>;
```

---

## §4.2 Three-Box Decomposition

### Resolution Engine

**Black box** (contract):
- INV-RESOLUTION-001: Resolution mode is algebraic — LWW, Lattice, MultiValue each satisfy
  their algebraic laws (commutativity, associativity, idempotency).
- INV-RESOLUTION-002: Lattice resolution produces the join (least upper bound) of all
  unretracted values.
- INV-RESOLUTION-004: Conflict predicate is decidable — for every (entity, attribute),
  `has_conflict` terminates and returns a boolean.
- INV-RESOLUTION-005: LWW resolution is commutative — independent of assertion arrival order.
- INV-RESOLUTION-006: MultiValue resolution is commutative — set of values independent of order.
- INV-RESOLUTION-008: Resolution is deterministic — same ConflictSet + mode → same ResolvedValue.

**State box** (internal design):
- ConflictSet is stateless — constructed per-query from the datom set.
- Resolution mode is read from schema: `schema.resolution_mode(attribute)`.
- Three-tier routing: 1. Lattice (if defined), 2. LWW (default), 3. MultiValue (opt-in).
- LIVE index: precomputed resolution for current state, updated incrementally.

**Clear box** (implementation):
- `resolve`:
  - Collect all assertions for (entity, attribute), subtract retractions.
  - Match on mode:
    - `Lattice(def)`: fold values using `def.join`. Result is the LUB.
    - `LastWriterWins`: pick value with max TxId (by HLC ordering).
    - `MultiValue`: return set of all unretracted values.
- `has_conflict`:
  - Lattice: conflict if two unretracted values are incomparable in the partial order.
  - LWW: conflict if two assertions have identical TxId (exact same timestamp — very rare).
  - Multi: never a conflict (all values kept).
- `live_entity`: for each attribute of entity → build ConflictSet → resolve → collect.

---

## §4.3 Type-Level Encoding

| INV | Compile-Time Guarantee | Mechanism |
|-----|----------------------|-----------|
| INV-RESOLUTION-001 | Exhaustive match on `ResolutionMode` | `match mode { Lattice(..) => .., LWW => .., Multi => .. }` |

---

## §4.4 LLM-Facing Outputs

### Agent-Mode Output — Conflict Detection

```
[RESOLUTION] Entity {eid}: attribute :task/status has conflicting values.
  [:open] asserted in tx {tx1} (LWW: this wins)
  [:closed] asserted in tx {tx2}
  Resolved: :open (LWW, most recent HLC)
---
↳ Is this the expected resolution? If not, check :db/resolutionMode for :task/status.
```

### Error Messages

- **Lattice incomparability**: `Resolution warning: {attr} has incomparable values {v1}, {v2} — define lattice join or switch to :multi — See: INV-RESOLUTION-002`
- **Missing resolution mode**: `Resolution error: {attr} has no :db/resolutionMode — defaults to LWW — See: ADR-RESOLUTION-001`

---

## §4.5 Verification

### Key Properties

```rust
proptest! {
    // INV-RESOLUTION-005: LWW commutativity
    fn inv_resolution_005(assertions in arb_assertions(5)) {
        let mut a1 = assertions.clone();
        let mut a2 = assertions.clone();
        a1.sort(); a2.sort_by(|a, b| b.cmp(a));  // different order
        let r1 = resolve_lww(&a1);
        let r2 = resolve_lww(&a2);
        prop_assert_eq!(r1, r2);
    }

    // INV-RESOLUTION-008: Determinism
    fn inv_resolution_008(conflict in arb_conflict_set(), mode in arb_resolution_mode()) {
        let r1 = resolve(&conflict, &mode);
        let r2 = resolve(&conflict, &mode);
        prop_assert_eq!(r1, r2);
    }
}
```

### Kani Harnesses

INV-RESOLUTION-002, 004, 005, 006 have V:KANI tags.

---

## §4.6 Implementation Checklist

- [ ] `ResolutionMode`, `ConflictSet`, `ResolvedValue` types defined
- [ ] `LatticeDef` with join and partial_order
- [ ] `resolve()` handles all three modes correctly
- [ ] `has_conflict()` detects lattice incomparability
- [ ] `live_entity()` resolves all attributes per schema mode
- [ ] LWW commutativity verified (proptest + Kani)
- [ ] Integration with STORE: transact conflicting values → resolve → correct LIVE view
- [ ] Integration with SCHEMA: resolution mode read from schema datoms

---
