# §13. TRILATERAL — Build Plan

> **Spec reference**: [spec/18-trilateral.md](../spec/18-trilateral.md)
> **Stage 0 elements**: INV-TRILATERAL-001-003, 005-007 (6 INV), ADR-TRILATERAL-001-002, NEG-TRILATERAL-001-003
> **Dependencies**: STORE (§1), LAYOUT (§1b), SCHEMA (§2), QUERY (§3)
> **Cognitive mode**: Coherence-theoretic — divergence metrics, formality gradients

---

## §13.1 Module Structure

All trilateral functions are pure computations over the store. No IO, no async, no
mutation beyond what `Store::transact` provides. Single module in the kernel crate.

```
braid-kernel/src/
└── trilateral.rs    ← AttrNamespace, classify_attribute, compute_phi, formality_level, live_intent
```

### Public API Surface

```rust
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub enum AttrNamespace { Intent, Spec, Impl, Meta }

pub struct LiveView { pub datoms: Vec<Datom>, pub entity_count: usize, pub namespace: AttrNamespace }

pub fn classify_attribute(attr: &Attribute) -> AttrNamespace;      // INV-TRILATERAL-005
pub fn compute_phi(store: &Store, w_is: f64, w_sp: f64) -> f64;   // INV-TRILATERAL-002
pub fn formality_level(store: &Store, entity: EntityId) -> u8;     // INV-TRILATERAL-003
pub fn live_intent(store: &Store) -> LiveView;                     // INV-TRILATERAL-001
pub fn live_spec(store: &Store) -> LiveView;
pub fn live_impl(store: &Store) -> LiveView;
pub fn count_unlinked_intent(store: &Store) -> usize;
pub fn count_untraced_spec(store: &Store) -> usize;
pub fn count_unimplemented_spec(store: &Store) -> usize;
pub fn count_unlinked_impl(store: &Store) -> usize;
```

---

## §13.2 Three-Box Decompositions

### (1) Attribute Namespace Classification (INV-TRILATERAL-005)

**Black box** (contract):
- Every attribute belongs to exactly one namespace. Three domain namespaces are pairwise disjoint (L3).
- Meta covers cross-cutting attributes (`:db/*`, `:tx/*`) outside all three LIVE views.
- V:TYPE: exhaustive `match` ensures classification is total.

**State box** (internal design):
- No internal state. Pure function. Classification by attribute namespace prefix (before `/`).

**Clear box** (implementation):
```rust
pub fn classify_attribute(attr: &Attribute) -> AttrNamespace {
    match attr.namespace() {
        "intent" => AttrNamespace::Intent,
        "spec"   => AttrNamespace::Spec,
        "impl"   => AttrNamespace::Impl,
        _        => AttrNamespace::Meta,
    }
}
```

The `_` arm captures all cross-cutting prefixes (`:db`, `:tx`, `:braid`). Infallible:
every attribute maps to exactly one namespace.

---

### (2) LIVE Projections (INV-TRILATERAL-001)

**Black box** (contract):
- Three projections filter by attribute namespace and apply resolution (INV-STORE-012).
- Each is monotone over the store semilattice: store growth only grows views.
- A datom appears in at most one LIVE view. Meta-namespace datoms appear in none.

**State box** (internal design):
- Iterate `store.datoms()`, filter by `classify_attribute`, collect into `LiveView`.
- `LiveView.entity_count` = distinct entities in the projection.

**Clear box** (implementation):
```rust
fn live_projection(store: &Store, namespace: AttrNamespace) -> LiveView {
    let datoms: Vec<Datom> = store.datoms()
        .filter(|d| classify_attribute(&d.attribute) == namespace)
        .cloned()
        .collect();
    let entity_count = datoms.iter().map(|d| d.entity).collect::<HashSet<_>>().len();
    LiveView { datoms, entity_count, namespace }
}

pub fn live_intent(store: &Store) -> LiveView { live_projection(store, AttrNamespace::Intent) }
pub fn live_spec(store: &Store) -> LiveView   { live_projection(store, AttrNamespace::Spec) }
pub fn live_impl(store: &Store) -> LiveView   { live_projection(store, AttrNamespace::Impl) }
```

Filtering via `classify_attribute` enforces INV-TRILATERAL-005 (partition) and
NEG-TRILATERAL-001 (no cross-view contamination) simultaneously.

---

### (3) Divergence Metric Phi (INV-TRILATERAL-002, INV-TRILATERAL-006)

**Black box** (contract):
- `Phi(S) = w_is * D_IS(S) + w_sp * D_SP(S)` where D_IS counts unlinked intent +
  untraced spec entities, and D_SP counts unimplemented spec + unlinked impl entities.
- Pure function of the store -- no external state (NEG-TRILATERAL-002). Phi >= 0.
- Phi = 0 iff full cross-boundary linkage exists. Expressible as Stratum 5 Datalog
  (INV-TRILATERAL-006), inheriting CALM compliance from INV-QUERY-001.

**State box** (internal design):
- Four counting functions scan for unlinked entities via set difference at each boundary.
- Boundary weights default to 0.5, stored as datoms (C3: schema-as-data).

**Clear box** (implementation):
```rust
pub fn compute_phi(store: &Store, w_is: f64, w_sp: f64) -> f64 {
    let d_is = count_unlinked_intent(store) + count_untraced_spec(store);
    let d_sp = count_unimplemented_spec(store) + count_unlinked_impl(store);
    w_is * d_is as f64 + w_sp * d_sp as f64
}

pub fn count_unlinked_intent(store: &Store) -> usize {
    let intent_entities: HashSet<EntityId> = store.datoms()
        .filter(|d| classify_attribute(&d.attribute) == AttrNamespace::Intent)
        .map(|d| d.entity).collect();
    let traced_targets: HashSet<EntityId> = store.datoms()
        .filter(|d| d.attribute == Attribute::new(":spec/traces-to").unwrap() && d.op == Op::Assert)
        .filter_map(|d| match &d.value { Value::Ref(e) => Some(*e), _ => None }).collect();
    intent_entities.difference(&traced_targets).count()
}
// count_untraced_spec, count_unimplemented_spec, count_unlinked_impl follow the same pattern:
// collect namespace entities, collect linked targets via :traces-to or :implements, set difference.
```

---

### (4) Formality Gradient (INV-TRILATERAL-003)

**Black box** (contract):
- `formality_level(e, S) -> {0, 1, 2, 3, 4}` based on cross-boundary link structure.
- L0: no links. L1: `:intent/noted`. L2: `:spec/id` + `:spec/type` + `:spec/statement`.
  L3: L2 + `:spec/falsification` + `:spec/traces-to`. L4: L3 + `:spec/witnessed` + `:spec/challenged`.
- Monotonically non-decreasing under store growth (L4): append-only (C1) means links
  can only be added, so formality can only increase.

**State box** (internal design):
- Query asserted datoms for the entity. Map present attributes to highest matching level.
- Cumulative: L3 requires all L2 attributes. Missing `:spec/type` keeps entity at L0.

**Clear box** (implementation):
```rust
pub fn formality_level(store: &Store, entity: EntityId) -> u8 {
    let attrs: HashSet<&Attribute> = store.datoms()
        .filter(|d| d.entity == entity && d.op == Op::Assert)
        .map(|d| &d.attribute).collect();
    let has = |name: &str| attrs.iter().any(|a| a.as_str() == name);

    if has(":spec/witnessed") && has(":spec/challenged") && has(":spec/falsification")
        && has(":spec/traces-to") && has(":spec/id") && has(":spec/type") && has(":spec/statement")
    { 4 }
    else if has(":spec/falsification") && has(":spec/traces-to")
        && has(":spec/id") && has(":spec/type") && has(":spec/statement")
    { 3 }
    else if has(":spec/id") && has(":spec/type") && has(":spec/statement") { 2 }
    else if has(":intent/noted") { 1 }
    else { 0 }
}
```

Cascading if-else from highest to lowest. Retraction datoms are excluded by the
`d.op == Op::Assert` filter -- retraction creates a new datom (C1) but the `has()`
check considers only surviving assertions.

---

### (5) Self-Bootstrap (INV-TRILATERAL-007)

**Black box** (contract):
- All TRILATERAL spec elements (INV-TRILATERAL-001-007, ADR-TRILATERAL-001-003,
  NEG-TRILATERAL-001-003) are datoms in the store after self-bootstrap.
- All trilateral spec elements appear in `LIVE_S(S)` — they are spec-namespace
  entities with `:spec/id`, `:spec/type`, `:spec/statement` attributes.
- Phi includes the trilateral spec elements themselves as entities requiring
  `:implements` links. The system measures its own coherence.

**State box** (internal design):
- During Phase 2 of the bootstrap path (guide/00-architecture.md §0.3b), the
  `spec-datoms.ednl` file includes trilateral elements alongside all other spec elements.
- Each INV/ADR/NEG is transacted as an entity with spec-namespace attributes.
- After bootstrap, `live_spec(store)` returns a view containing these entities.
- `compute_phi` counts them as spec entities — if they lack `:implements` links to
  impl entities (code implementing the trilateral module), they contribute to D_SP.

**Clear box** (implementation):
- No additional code beyond the functions above. Self-bootstrap is a data concern:
  the trilateral elements are included in the EDNL bootstrap file.
- Verification: after bootstrap, run:
  ```
  braid query '[:find ?id :where [?e :spec/id ?id] [(clojure.string/starts-with? ?id "INV-TRILATERAL")]]'
  ```
  to confirm all 7 invariants are present as datoms.
- The self-referential property validates that `classify_attribute`, `live_spec`,
  and `compute_phi` compose correctly on the system's own specification.

---

## §13.3 Type-Level Encoding

| INV | Compile-Time Guarantee | Mechanism |
|-----|----------------------|-----------|
| INV-TRILATERAL-005 | `AttrNamespace` enum has exactly 4 variants | `#[derive]`, exhaustive `match` |
| INV-TRILATERAL-005 | `classify_attribute` is total | `match` with `_` arm covers all prefixes |
| INV-TRILATERAL-003 | Formality level bounded to 0..=4 | Return type `u8`, all branches return literal 0-4 |

---

## §13.4 LLM-Facing Outputs

### Agent-Mode Output — `braid phi` (or `braid status` extended)

```
[TRILATERAL] Phi = {phi:.2}  (D_IS = {d_is}, D_SP = {d_sp}, w = [{w_is}, {w_sp}])
Entities: {intent_count} intent, {spec_count} spec, {impl_count} impl.
Unlinked: {unlinked_intent} intent, {untraced_spec} spec (I<>S); {unimpl_spec} spec, {unlinked_impl} impl (S<>P).
---
@@ {guidance_footer}
```

### Agent-Mode Output — `braid formality {entity_id}`

```
[TRILATERAL] Entity {entity_id}: formality level {level}/4.
Present: {present_attrs}. Missing for L{next_level}: {missing_attrs}.
---
@@ {guidance_footer}
```

### Error Messages

- **Unknown attribute namespace**: `Trilateral error: attribute {attr} has namespace "{ns}" not in {intent, spec, impl, meta} -- verify attribute spelling matches schema -- See: INV-TRILATERAL-005`
- **Negative weights**: `Trilateral error: boundary weights must be non-negative (got w_is={w_is}, w_sp={w_sp}) -- use default 0.5 or configure via store datoms -- See: INV-TRILATERAL-002`

---

## §13.5 Verification

### Proptest Strategies

```rust
proptest! {
    // INV-TRILATERAL-001: LIVE projection monotonicity — adding datoms only grows views.
    fn inv_trilateral_001(base in arb_store(3), extra in arb_datoms(5)) {
        let before = (live_intent(&base).entity_count, live_spec(&base).entity_count,
                      live_impl(&base).entity_count);
        let mut grown = base.clone();
        for d in extra { grown.transact(vec![d]).ok(); }
        prop_assert!(live_intent(&grown).entity_count >= before.0);
        prop_assert!(live_spec(&grown).entity_count >= before.1);
        prop_assert!(live_impl(&grown).entity_count >= before.2);
    }

    // INV-TRILATERAL-002: Phi >= 0 and pure (same store => same Phi).
    fn inv_trilateral_002(store in arb_store(5)) {
        let phi = compute_phi(&store, 0.5, 0.5);
        prop_assert!(phi >= 0.0);
        prop_assert_eq!(phi, compute_phi(&store, 0.5, 0.5));
    }

    // INV-TRILATERAL-003: Formality monotonicity under store growth.
    fn inv_trilateral_003(base in arb_store(3), e in arb_entity_id(), extra in arb_datoms(5)) {
        let before = formality_level(&base, e);
        let mut grown = base.clone();
        for d in extra { grown.transact(vec![d]).ok(); }
        prop_assert!(formality_level(&grown, e) >= before);
    }

    // INV-TRILATERAL-005: Each datom in a LIVE view belongs to that view's namespace.
    fn inv_trilateral_005(store in arb_store(5)) {
        for d in &live_intent(&store).datoms { prop_assert_eq!(classify_attribute(&d.attribute), AttrNamespace::Intent); }
        for d in &live_spec(&store).datoms   { prop_assert_eq!(classify_attribute(&d.attribute), AttrNamespace::Spec); }
        for d in &live_impl(&store).datoms   { prop_assert_eq!(classify_attribute(&d.attribute), AttrNamespace::Impl); }
    }

    // INV-TRILATERAL-006: Phi deterministic (weaker Datalog expressibility at Stage 0).
    fn inv_trilateral_006(store in arb_store(5), w1 in 0.0..1.0_f64, w2 in 0.0..1.0_f64) {
        prop_assert_eq!(compute_phi(&store, w1, w2), compute_phi(&store, w1, w2));
    }

    // INV-TRILATERAL-007: After self-bootstrap, all 7 INVs are in LIVE_S.
    fn inv_trilateral_007(store in arb_bootstrapped_store()) {
        let ids: Vec<String> = live_spec(&store).datoms.iter()
            .filter(|d| d.attribute == Attribute::new(":spec/id").unwrap())
            .filter_map(|d| match &d.value { Value::String(s) => Some(s.clone()), _ => None })
            .collect();
        for i in 1..=7 { prop_assert!(ids.contains(&format!("INV-TRILATERAL-{:03}", i))); }
    }

    // NEG-TRILATERAL-001: No cross-view contamination — views are pairwise disjoint.
    fn neg_trilateral_001(datoms in arb_datoms(20)) {
        let mut store = Store::genesis();
        for d in datoms { store.transact(vec![d]).ok(); }
        let (i, s, p) = (live_intent(&store).datoms.iter().collect::<HashSet<_>>(),
                         live_spec(&store).datoms.iter().collect::<HashSet<_>>(),
                         live_impl(&store).datoms.iter().collect::<HashSet<_>>());
        prop_assert!(i.is_disjoint(&s) && s.is_disjoint(&p) && i.is_disjoint(&p));
    }

    // NEG-TRILATERAL-003: Link addition never increases Phi.
    fn neg_trilateral_003(store in arb_store_with_unlinked(5)) {
        let phi_before = compute_phi(&store, 0.5, 0.5);
        let mut linked = store.clone();
        if let Some((ie, se)) = find_unlinkable_pair(&store) {
            linked.transact(vec![Datom {
                entity: ie, attribute: Attribute::new(":spec/traces-to").unwrap(),
                value: Value::Ref(se), tx: TxId::now(AgentId::test()), op: Op::Assert,
            }]).ok();
        }
        prop_assert!(compute_phi(&linked, 0.5, 0.5) <= phi_before);
    }
}
// NEG-TRILATERAL-002: Enforced by function signature — compute_phi takes only &Store + weights.
```

---

## §13.6 Implementation Checklist

- [ ] `AttrNamespace` enum with 4 variants defined
- [ ] `classify_attribute` function implemented and tested
- [ ] `LiveView` struct defined
- [ ] `live_intent`, `live_spec`, `live_impl` functions implemented
- [ ] `count_unlinked_intent`, `count_untraced_spec`, `count_unimplemented_spec`, `count_unlinked_impl` implemented
- [ ] `compute_phi` function implemented and tested for purity, non-negativity
- [ ] `formality_level` function implemented with all 5 levels
- [ ] Proptest: INV-TRILATERAL-001 (projection monotonicity) passes
- [ ] Proptest: INV-TRILATERAL-002 (Phi non-negative and pure) passes
- [ ] Proptest: INV-TRILATERAL-003 (formality monotonicity) passes
- [ ] Proptest: INV-TRILATERAL-005 (namespace partition, no cross-view contamination) passes
- [ ] Proptest: INV-TRILATERAL-006 (Phi determinism) passes
- [ ] Proptest: INV-TRILATERAL-007 (self-bootstrap) passes after spec datom ingestion
- [ ] Proptest: NEG-TRILATERAL-001 (disjoint views) passes
- [ ] Proptest: NEG-TRILATERAL-003 (link addition never increases Phi) passes
- [ ] `cargo check` passes (Gate 1)
- [ ] `cargo test` passes (Gate 2)
- [ ] Integration: genesis + spec bootstrap + `compute_phi` returns correct value

---
