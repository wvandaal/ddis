# §4. RESOLUTION — Build Plan

> **Spec reference**: [spec/04-resolution.md](../spec/04-resolution.md) — read FIRST
> **Stage 0 elements**: INV-RESOLUTION-001–008 (all 8), ADR-RESOLUTION-001–004, NEG-RESOLUTION-001–003
> **Dependencies**: STORE (§1), SCHEMA (§2), QUERY (§3)
> **Cognitive mode**: Order-theoretic — lattices, partial orders, conflict predicates

---

## §4.1 Module Structure

```
crates/braid-kernel/src/
└── resolution.rs   ← ResolutionMode, ConflictSet, resolve, LIVE index computation
```

### Public API Surface

```rust
pub enum ResolutionMode {
    /// Join-semilattice resolution — lattice definition stored as datoms (C3).
    Lattice { lattice_id: EntityId },
    /// Last-writer-wins, ordered by HLC timestamp.
    LastWriterWins,
    /// Keep all values (cardinality :many semantics).
    MultiValue,
}
// Lattice definitions are stored AS DATOMS in the store (C3, ADR-SCHEMA-004).
// The lattice_id references an entity with :lattice/* attributes defining
// the partial order, join operation, and bottom element.

/// A set of competing assertions for a single (entity, attribute) pair.
pub struct ConflictSet {
    pub entity:    EntityId,
    pub attribute: Attribute,
    pub assertions: Vec<(Value, TxId)>,  // (value, asserting transaction)
    pub retractions: Vec<(Value, TxId)>,
}

/// Resolve a conflict set using the attribute's resolution mode.
pub fn resolve(conflict: &ConflictSet, mode: &ResolutionMode) -> ResolvedValue;

// [GUIDE-ONLY] — convenience wrapper for resolution output
pub enum ResolvedValue {
    Single(Value),                // LWW or Lattice result
    Multi(Vec<Value>),            // Multi-value (all unretracted)
    Conflict(Vec<(Value, TxId)>), // Unresolvable (conservative detection)
}

/// Conflict predicate: does this (entity, attribute) have unresolved conflict?
pub fn has_conflict(conflict: &ConflictSet, mode: &ResolutionMode) -> bool;

/// Clock selection for LWW resolution (spec/02-schema.md: :db/lwwClock).
/// Read from schema datoms at resolution time; not embedded in ResolutionMode.
pub enum LwwClock {
    Hlc,        // Hybrid Logical Clock (default, most precise)
    Wall,       // Wall-clock ordering
    AgentRank,  // Deterministic agent hierarchy
}

/// Resolution provenance entity (NEG-RESOLUTION-003).
/// Every resolution — automatic, agent, or human — produces a Resolution
/// entity in the store recording method, value, and rationale.
pub struct Resolution {
    pub conflict: EntityId,         // references the ConflictSet's entity
    pub resolved_value: Value,      // the winning value
    pub method: ResolutionMethod,   // :lww | :lattice | :deliberation | :human
    pub rationale: String,          // human-readable explanation
}

/// Compute the LIVE view of an entity by resolving all attributes.
pub fn live_entity(store: &Store, entity: EntityId) -> HashMap<Attribute, ResolvedValue>;
```

---

## §4.2 Three-Box Decomposition

### Resolution Engine

**Black box** (contract):
- INV-RESOLUTION-001: Per-Attribute Resolution — every attribute declares its resolution mode
  (LWW, Lattice, Multi); default is LWW with HLC clock.
- INV-RESOLUTION-002: Resolution Commutativity — resolution is order-independent; two agents
  with the same datom set produce the same resolved value (critical for CRDT consistency).
- INV-RESOLUTION-004: Conflict Predicate Correctness — conflict requires all six conditions
  (same entity, same attribute, different value, both assert, cardinality one, causally independent).
- INV-RESOLUTION-005: LWW Semilattice Properties — commutativity, associativity, idempotency.
  Ties on identical HLC timestamps broken by BLAKE3 hash comparison (ADR-RESOLUTION-009).
- INV-RESOLUTION-006: Lattice Join Correctness — lattice resolution produces the least upper
  bound; diamond lattices produce error signal element for incomparable values.
- INV-RESOLUTION-008: Conflict Entity Datom Trail — full conflict lifecycle (assert, severity,
  route, fire TUI, update uncertainty, invalidate caches) all produce datoms in the store.

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
    - `LastWriterWins`: pick value with max TxId (by HLC ordering). If HLC timestamps
      are equal, break ties by BLAKE3 hash of the full datom content — the datom with the
      lexicographically greater hash wins (ADR-RESOLUTION-009, ADR-STORE-013). This is
      always decisive: different values produce different hashes (collision resistance),
      so equal-timestamp ties are resolved deterministically without additional mechanism.
    - `MultiValue`: return set of all unretracted values.
- `has_conflict`:
  - Lattice: conflict if two unretracted values are incomparable in the partial order.
  - LWW: conflict if two causally independent assertions exist for the same (entity, attribute)
    with cardinality :one (INV-RESOLUTION-004). Causal independence is determined by walking
    the causal_predecessors chain — if neither assertion's transaction is a causal ancestor
    of the other, they are genuinely concurrent and thus conflicting.
  - Multi: never a conflict (all values kept).

```rust
fn has_conflict(store: &Store, entity: EntityId, attr: &Attribute) -> bool {
    let assertions = store.datoms()
        .filter(|d| d.entity == entity && d.attribute == *attr && d.op == Op::Assert)
        .collect::<Vec<_>>();
    if assertions.len() <= 1 { return false; }
    // Check if any pair of assertions are causally independent
    for i in 0..assertions.len() {
        for j in (i+1)..assertions.len() {
            if !is_causal_ancestor(store, assertions[i].tx, assertions[j].tx)
                && !is_causal_ancestor(store, assertions[j].tx, assertions[i].tx)
            {
                return true;  // Causally independent — genuine conflict
            }
        }
    }
    false
}

fn is_causal_ancestor(store: &Store, ancestor: TxId, descendant: TxId) -> bool {
    // Walk causal_predecessors chain from descendant backward
    let mut frontier = vec![descendant];
    let mut visited = HashSet::new();
    while let Some(tx) = frontier.pop() {
        if tx == ancestor { return true; }
        if !visited.insert(tx) { continue; }
        // Look up causal predecessors from tx entity datoms
        for pred in store.causal_predecessors(tx) {
            frontier.push(pred);
        }
    }
    false
}
```
- `live_entity`: for each attribute of entity → build ConflictSet → resolve → collect.

### Conservative Conflict Detection (INV-RESOLUTION-003)

**Black box** (contract):
- INV-RESOLUTION-003: For any local frontier F_local ⊆ F_global:
  `conflicts(F_local) ⊇ conflicts(F_global)`.
  A partial view may overestimate conflicts (safe — wasted effort) but never underestimate
  (critical — silent data corruption). No false negatives.

**State box** (internal design):
- Conflict detection operates on the local agent's datom set (their frontier).
- The frontier determines visibility: an agent only sees datoms in its frontier.
- With partial visibility, some retractions may not be visible yet → conservative assumption
  that the conflict still exists.

**Clear box** (implementation):
```rust
/// Detect conflicts visible at a frontier. Conservative — may overestimate.
pub fn detect_conflicts(
    store: &Store,
    frontier: &HashMap<AgentId, TxId>,
) -> Vec<ConflictSet> {
    let visible = store.datoms_at_frontier(frontier);
    let mut conflicts = Vec::new();
    for (entity, attr) in unique_ea_pairs(&visible) {
        let assertions = visible.assertions_for(entity, attr);
        if assertions.len() > 1 && store.schema().cardinality(attr) == Cardinality::One {
            // Multiple unretracted values for a :one attribute → conflict
            conflicts.push(ConflictSet { entity, attribute: attr, assertions, retractions: vec![] });
        }
    }
    conflicts
}
```

**proptest strategy**: Generate two stores S₁ ⊂ S₂. Verify that
`detect_conflicts(S₁, F₁) ⊇ detect_conflicts(S₂, F₂)` where F₁ ⊂ F₂.

### Resolution Cascade: Lattice → LWW → Multi-Value Fallback

The resolution engine follows a deterministic cascade when resolving a conflict set:

1. **Lattice resolution** (if attribute has `ResolutionMode::Lattice`): Compute `join_L(v1, v2)`
   using the lattice definition stored as datoms (C3). If the values are comparable in the partial
   order, the least upper bound (LUB) is the resolved value. If the values are **incomparable**
   (diamond lattice case), the lattice produces an error signal element and the conflict is
   escalated to `RoutingTier::AgentNotification` for agent or human resolution.

2. **LWW resolution** (default for all attributes without explicit mode): Pick the value with the
   highest HLC timestamp. If timestamps are equal, break ties deterministically via BLAKE3 hash
   comparison of the full datom content (ADR-RESOLUTION-009). LWW always produces a single winner.

3. **Multi-value resolution** (for `:many` cardinality attributes): Keep all unretracted values.
   No conflict is possible — all values coexist. This mode is opt-in via `ResolutionMode::MultiValue`.

The cascade is **not a fallback chain** — each attribute has exactly one mode, declared in the schema.
The ordering reflects design preference: lattice when domain semantics define a natural partial order,
LWW as the safe default, multi-value when multiple concurrent values are the intended behavior.

### Three-Tier Routing (INV-RESOLUTION-007)

**Black box** (contract):
- INV-RESOLUTION-007: Every detected conflict is routed to exactly one of
  `{Automatic, AgentNotification, HumanRequired}`. No conflict remains unrouted.
  Routing is total and deterministic (same severity → same tier).

**State box** (internal design):
- Routing is a pure function from conflict severity to tier.
- Severity is computed as the product of cardinality (number of conflicting assertions) and
  domain impact (attribute importance), mapped to a 1-5 scale per INV-RESOLUTION-005.
  In practice, `severity = max(commitment_weight(d1), commitment_weight(d2))` from the
  conflicting datoms (spec/04-resolution.md §4.1 Level 1).
- Thresholds: `Automatic` for low-severity (e.g., LWW-resolvable), `AgentNotification` for
  medium (lattice incomparability), `HumanRequired` for high (axiological conflicts).

**Clear box** (implementation):
```rust
pub enum RoutingTier { Automatic, AgentNotification, HumanRequired }

pub fn route_conflict(conflict: &ConflictSet, mode: &ResolutionMode) -> RoutingTier {
    match mode {
        ResolutionMode::LastWriterWins => RoutingTier::Automatic,
        ResolutionMode::MultiValue => RoutingTier::Automatic,
        ResolutionMode::Lattice { lattice_id } => {
            // Check if values are comparable in the lattice
            if lattice_values_comparable(conflict, *lattice_id) {
                RoutingTier::Automatic
            } else {
                RoutingTier::AgentNotification  // incomparable → needs agent attention
            }
        }
    }
}
```

**proptest strategy**: Generate arbitrary `ConflictSet` × `ResolutionMode` pairs.
Verify that `route_conflict` always returns a valid `RoutingTier` (totality via exhaustive match).

---

## §4.3 Type-Level Encoding

| INV | Compile-Time Guarantee | Mechanism |
|-----|----------------------|-----------|
| INV-RESOLUTION-001 | Exhaustive match on `ResolutionMode` | `match mode { Lattice(..) => .., LastWriterWins => .., MultiValue => .. }` |

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

- **Lattice incomparability**: `Resolution warning: {attr} has incomparable values {v1}, {v2} — define lattice join or switch to :multi — See: INV-RESOLUTION-006`
- **Missing resolution mode**: `Resolution error: {attr} has no :db/resolutionMode — defaults to LWW — See: ADR-RESOLUTION-001`

---

## §4.4b Negative Cases

### NEG-RESOLUTION-001: No Merge-Time Resolution
The `merge()` function (§7) has no `Schema` parameter — it cannot access resolution modes.
Merge is pure set union (C4). Conflicting values coexist in the merged store; resolution
happens only at query time via the LIVE index.

**Enforcement**: `fn merge(&mut self, other: &Store)` — no schema access possible at the type level.

### NEG-RESOLUTION-002: No False Negative Conflict Detection
INV-RESOLUTION-003 guarantees conservative detection (overestimates, never underestimates).
Any frontier containing both conflicting datoms MUST detect the conflict. False negatives
(missed real conflicts) are a correctness violation that enables silent data corruption.

**Enforcement**: Stateright model (V:MODEL) with 3 agents and all merge interleavings.

### NEG-RESOLUTION-003: No Resolution Without Provenance
Every conflict resolution — automatic LWW, lattice join, or human choice — produces a
Resolution entity in the store with `:resolution/method`, `:resolution/conflict`,
`:resolution/value`. Resolution without a datom trail is invisible to audit.

**Enforcement**: `resolve()` returns `ResolvedValue` plus side-effect datoms. The caller
must transact the resolution datoms.

---

## §4.5 Verification

### Key Properties

```rust
proptest! {
    // INV-RESOLUTION-005: LWW commutativity (including BLAKE3 tie-breaking)
    fn inv_resolution_005(assertions in arb_assertions(5)) {
        let mut a1 = assertions.clone();
        let mut a2 = assertions.clone();
        a1.sort(); a2.sort_by(|a, b| b.cmp(a));  // different order
        let r1 = resolve_lww(&a1);
        let r2 = resolve_lww(&a2);
        prop_assert_eq!(r1, r2);
    }

    // ADR-RESOLUTION-009: BLAKE3 tie-breaking determinism for equal HLC timestamps
    fn adr_resolution_009_tiebreak(v1 in arb_value(), v2 in arb_value()) {
        prop_assume!(v1 != v2);
        let tx = TxId::from_hlc(same_timestamp());  // identical HLC for both
        let a1 = vec![(v1.clone(), tx.clone()), (v2.clone(), tx.clone())];
        let a2 = vec![(v2.clone(), tx.clone()), (v1.clone(), tx.clone())];
        let r1 = resolve_lww(&a1);
        let r2 = resolve_lww(&a2);
        prop_assert_eq!(r1, r2);  // same winner regardless of input order
    }

    // INV-RESOLUTION-002: Resolution Commutativity (order-independent)
    fn inv_resolution_002(assertions in arb_assertions(5)) {
        let mut a1 = assertions.clone();
        let mut a2 = assertions.clone();
        a1.sort(); a2.sort_by(|a, b| b.cmp(a));  // different order
        let r1 = resolve_set(&a1);
        let r2 = resolve_set(&a2);
        prop_assert_eq!(r1, r2);
    }

    // INV-RESOLUTION-004: Conflict Predicate Correctness (six conditions)
    fn inv_resolution_004(conflict in arb_conflict_set(), mode in arb_resolution_mode()) {
        let is_conflict = has_conflict(&conflict, &mode);
        // If cardinality is :many, there should be no conflict
        if mode == ResolutionMode::MultiValue {
            prop_assert!(!is_conflict);
        }
        // If only one assertion, no conflict
        if conflict.assertions.len() <= 1 {
            prop_assert!(!is_conflict);
        }
    }

    // INV-RESOLUTION-006: Lattice Join Correctness
    fn inv_resolution_006(conflict in arb_conflict_set()) {
        // For LWW mode: result is the value with max TxId
        let result = resolve(&conflict, &ResolutionMode::LastWriterWins);
        if let ResolvedValue::Single(v) = result {
            let max_tx = conflict.assertions.iter().max_by_key(|(_, tx)| tx).unwrap();
            prop_assert_eq!(v, max_tx.0.clone());
        }
    }

    // INV-RESOLUTION-003: Conservative Conflict Detection — partial view never underestimates
    fn inv_resolution_003(
        (s_local, s_global) in arb_overlapping_stores(5, 10)
    ) {
        let f_local = s_local.frontier().clone();
        let f_global = s_global.frontier().clone();
        let conflicts_local = detect_conflicts(&s_local, &f_local);
        let conflicts_global = detect_conflicts(&s_global, &f_global);
        // Local (partial) must be superset of global (full)
        for gc in &conflicts_global {
            prop_assert!(conflicts_local.iter().any(|lc|
                lc.entity == gc.entity && lc.attribute == gc.attribute
            ));
        }
    }

    // INV-RESOLUTION-007: Three-Tier Routing Totality — every conflict is routed
    fn inv_resolution_007(conflict in arb_conflict_set(), mode in arb_resolution_mode()) {
        let tier = route_conflict(&conflict, &mode);
        // Totality: result is always one of the three tiers (exhaustive match guarantees)
        prop_assert!(matches!(tier,
            RoutingTier::Automatic | RoutingTier::AgentNotification | RoutingTier::HumanRequired
        ));
        // LWW and MultiValue always resolve automatically
        if matches!(mode, ResolutionMode::LastWriterWins | ResolutionMode::MultiValue) {
            prop_assert_eq!(tier, RoutingTier::Automatic);
        }
    }

    // INV-RESOLUTION-008: Conflict Entity Datom Trail — all lifecycle steps produce datoms
    fn inv_resolution_008(store in arb_store(5), conflict in arb_conflict_set()) {
        let mut s = store;
        let pre_len = s.len();
        record_conflict_lifecycle(&mut s, &conflict);
        // At least one datom per lifecycle step
        prop_assert!(s.len() >= pre_len + 6);  // 6 lifecycle steps
    }
}
```

### Kani Harnesses

INV-RESOLUTION-001, 002, 004, 005, 006 have V:KANI tags.

---

## §4.6 Implementation Checklist

- [ ] `ResolutionMode`, `ConflictSet`, `ResolvedValue` types defined
- [ ] Lattice definitions as datoms (C3) — no function pointers
- [ ] `resolve()` handles all three modes correctly
- [ ] `has_conflict()` detects lattice incomparability
- [ ] `live_entity()` resolves all attributes per schema mode
- [ ] LWW commutativity verified (proptest + Kani)
- [ ] LWW BLAKE3 tie-breaking for equal HLC timestamps (ADR-RESOLUTION-009)
- [ ] Integration with STORE: transact conflicting values → resolve → correct LIVE view
- [ ] Integration with SCHEMA: resolution mode read from schema datoms

---
