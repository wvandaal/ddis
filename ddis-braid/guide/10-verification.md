# §10. Verification Pipeline

> **Spec reference**: [spec/16-verification.md](../spec/16-verification.md)
> **Prerequisite**: Read spec §16 (per-invariant verification matrix) before this file.
> **Purpose**: Translate the verification plan into executable CI gates and testing protocols.

---

## §10.1 Tiered Verification Gates

Every commit runs through a staged pipeline. Gates are ordered by cost: cheapest first,
most expensive last. A failure at any gate blocks progression.

### Gate 1: Compile (`cargo check --all-targets`)

**Checks**: All `V:TYPE` invariants — typestate patterns, newtype boundaries, exhaustive
match arms. Zero runtime cost verification.

**Time**: <30 seconds.

**Invariants verified at compile time**:
- INV-STORE-001 (Transaction typestate: Building → Committed → Applied)
- INV-STORE-002 (EntityId: no raw constructor)
- INV-STORE-003 (content identity via Hash/Eq derivation)
- INV-SCHEMA-003 (Attribute newtype)
- INV-SCHEMA-004 (SchemaEvolution: no DROP/ALTER)
- INV-QUERY-005 (QueryMode enum)
- INV-QUERY-006 (FFI purity marker)
- INV-QUERY-007 (typed clause patterns)
- INV-RESOLUTION-001 (ResolutionMode exhaustive match)
- INV-INTERFACE-001 (OutputFormat enum)
- INV-INTERFACE-003 (MCP_TOOLS: fixed-size array)
- INV-INTERFACE-009 (RecoveryAction enum exhaustive match)

### Gate 2: Test (`cargo test`)

**Checks**: All `V:PROP` invariants via proptest properties. Every one of the 121 invariants
has a proptest strategy (100% coverage by spec requirement).

**Time**: <5 minutes (256 cases per property, default proptest config).

**Configuration**:
```toml
# proptest.toml (in workspace root)
[default]
cases = 256
max_shrink_iters = 1000
```

**Regression file management**: proptest regression files (`*.proptest-regressions`) are
committed to git. They contain minimal failure cases that must continue to pass.

### Gate 3: Kani (`cargo kani`)

**Checks**: `V:KANI` invariants — bounded model checking. Exhaustive verification for all
inputs up to the configured bound.

**Time**: <15 minutes.

**Coverage**: 38 invariants (31.4%) with critical-path verification:
- All STORE CRDT laws (INV-STORE-004–008)
- All SCHEMA bootstrap properties (INV-SCHEMA-001–002, 004)
- All RESOLUTION algebraic laws (INV-RESOLUTION-002, 004–006)
- HARVEST gap detection (INV-HARVEST-001, 006)
- SEED idempotence (INV-SEED-002–003)
- MERGE preservation (INV-MERGE-001)
- BUDGET token efficiency (INV-BUDGET-006)

**Configuration**:
```rust
// Kani harness examples (2 of 38 — see full list below)
#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// INV-STORE-001: Append-only — transact never removes datoms.
    #[kani::proof]
    #[kani::unwind(8)]
    fn inv_store_001_append_only() {
        let store = kani::any::<Store>();
        let tx = kani::any::<Transaction<Committed>>();
        let pre_len = store.len();
        let result = store.transact(tx);
        if let Ok(new_store) = result {
            kani::assert(new_store.len() >= pre_len);
        }
    }

    /// INV-STORE-005: Store immutability — reads never see partial transactions.
    #[kani::proof]
    #[kani::unwind(8)]
    fn inv_store_005_read_immutability() {
        let store = kani::any::<Store>();
        let snapshot: BTreeSet<_> = store.datoms().collect();
        // Any concurrent read during a transact must see either pre or post state
        let tx = kani::any::<Transaction<Committed>>();
        let _ = store.clone().transact(tx);  // clone — original untouched
        let after: BTreeSet<_> = store.datoms().collect();
        kani::assert(snapshot == after);  // original store unchanged
    }
}
```

**Solver bounds**: `#[kani::unwind(8)]` for most harnesses. Increase to 16 for
CRDT commutativity/associativity proofs (three-store merge scenarios).

### Complete V:KANI Harness List (38 total)

All INVs tagged V:KANI in the verification matrix, grouped by namespace:

| Namespace | INV | Harness Property |
|-----------|-----|-----------------|
| STORE | INV-STORE-001 | Append-only: transact never decreases datom count |
| STORE | INV-STORE-004 | Merge commutativity: `merge(A,B) = merge(B,A)` |
| STORE | INV-STORE-005 | Store immutability: reads unaffected by concurrent writes |
| STORE | INV-STORE-006 | Merge idempotency: `merge(A,A) = A` |
| STORE | INV-STORE-007 | Merge monotonicity: `|merge(A,B)| ≥ max(|A|,|B|)` |
| STORE | INV-STORE-008 | Genesis determinism: `genesis() = genesis()` |
| STORE | INV-STORE-010 | Causal ordering: predecessor `<` successor in HLC |
| STORE | INV-STORE-012 | LIVE index matches manual resolution |
| SCHEMA | INV-SCHEMA-001 | Schema-as-data: schema extracted only from datoms |
| SCHEMA | INV-SCHEMA-002 | Genesis completeness: exactly 17 axiomatic attributes |
| SCHEMA | INV-SCHEMA-004 | Schema validation: rejects malformed datoms |
| QUERY | INV-QUERY-001 | Query determinism: same inputs → same bindings |
| QUERY | INV-QUERY-004 | Stratified negation: no unstratifiable queries accepted |
| QUERY | INV-QUERY-012 | SCC correctness: Tarjan's produces valid decomposition |
| QUERY | INV-QUERY-013 | Condensation DAG: acyclic after SCC contraction |
| QUERY | INV-QUERY-017 | Critical path: longest path in DAG |
| RESOLUTION | INV-RESOLUTION-002 | Resolution commutativity: order-independent |
| RESOLUTION | INV-RESOLUTION-004 | Conflict predicate: six-condition correctness |
| RESOLUTION | INV-RESOLUTION-005 | LWW semilattice: comm + assoc + idem |
| RESOLUTION | INV-RESOLUTION-006 | Lattice join: LUB correctness |
| RESOLUTION | INV-RESOLUTION-007 | Three-tier routing: totality (no unrouted conflicts) |
| HARVEST | INV-HARVEST-001 | Harvest monotonicity: never removes datoms |
| HARVEST | INV-HARVEST-006 | Crystallization guard: high-weight stability check |
| SEED | INV-SEED-002 | Budget compliance: output ≤ budget |
| SEED | INV-SEED-003 | ASSOCIATE boundedness: ≤ depth × breadth |
| MERGE | INV-MERGE-001 | No datom loss: both inputs preserved |
| MERGE | INV-MERGE-003 | Branch isolation: branches can't see each other |
| MERGE | INV-MERGE-004 | DCC completeness: diverge-compare-converge |
| MERGE | INV-MERGE-005 | Competing branch lock: at most 2 active branches |
| SIGNAL | INV-SIGNAL-001 | Signal monotonicity: signals grow, never shrink |
| SIGNAL | INV-SIGNAL-003 | Signal correctness: derived from datom state |
| SIGNAL | INV-SIGNAL-005 | Threshold detection: triggers fire at boundary |
| DELIBERATION | INV-DELIBERATION-002 | Quorum correctness: majority required |
| DELIBERATION | INV-DELIBERATION-005 | Decision finality: committed decisions immutable |
| GUIDANCE | INV-GUIDANCE-006 | M(t) bounded: methodology score ∈ [0,1] |
| BUDGET | INV-BUDGET-001 | Output budget cap: `|output| ≤ budget` |
| BUDGET | INV-BUDGET-003 | Projection monotonicity: higher level ≤ lower tokens |
| BUDGET | INV-BUDGET-006 | Token efficiency: density monotonically non-decreasing |

### Gate 4: Model Checking (`cargo test --features stateright`)

**Checks**: `V:MODEL` invariants — protocol safety and liveness over all reachable states.

**Time**: <30 minutes.

**Coverage**: 15 invariants (12.4%) for protocol-level properties:
- STORE CRDT algebra (INV-STORE-004–005)
- MERGE cascade (INV-MERGE-002, 004)
- SYNC barrier (INV-SYNC-001, 003–004)
- DELIBERATION lifecycle (INV-DELIBERATION-001, 006)
- RESOLUTION convergence (INV-RESOLUTION-003, 007–008)
- BILATERAL convergence (INV-BILATERAL-001)

**Gate progression**:
- Gates 1–2: every commit
- Gate 3: PRs targeting main
- Gate 4: nightly, or on protocol-affecting changes
- Gate 5 (Miri): on any `unsafe` code changes (should be never — `#![forbid(unsafe_code)]`)

---

## §10.2 CI Configuration

```yaml
# .github/workflows/ci.yml
name: Braid CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  gate-1-compile:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo check --all-targets

  gate-2-test:
    needs: gate-1-compile
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --all-targets

  gate-3-clippy-fmt:
    needs: gate-1-compile
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo fmt --check

  gate-4-kani:
    if: github.event_name == 'pull_request'
    needs: gate-2-test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: model-checking/kani-verifier-action@v1
      - run: cargo kani --workspace

  gate-5-model:
    if: github.event_name == 'schedule'  # nightly
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --features stateright --test model_check
```

---

## §10.3 Coverage Matrix Template

The verification matrix maps each Stage 0 INV (61 total) to its verification methods.
This template is filled as implementation proceeds:

| INV | V:TYPE | V:PROP | V:KANI | V:MODEL | Status |
|-----|--------|--------|--------|---------|--------|
| INV-STORE-001 | ✓ (typestate) | ☐ | ☐ | — | ☐ |
| INV-STORE-002 | ✓ (newtype) | ☐ | ☐ | — | ☐ |
| INV-STORE-003 | ✓ (Hash/Eq) | ☐ | — | — | ☐ |
| INV-STORE-004 | — | ☐ | ☐ | ☐ | ☐ |
| ... | | | | | |
| INV-INTERFACE-008 | — | ☐ | — | — | ☐ |
| INV-INTERFACE-009 | ✓ (enum) | ☐ | — | — | ☐ |
| INV-BUDGET-006 | — | ☐ | ☐ | — | ☐ |

**Legend**: ✓ = implemented and passing, ☐ = pending, — = not applicable.

Track as a file `tests/coverage-matrix.md` updated after each namespace completion.

---

## §10.4 Proptest Configuration

### Strategy Hierarchy

Build strategies bottom-up, composing simple strategies into complex ones:

```rust
// Level 1: Primitive strategies
fn arb_entity_id() -> impl Strategy<Value = EntityId> {
    any::<[u8; 32]>().prop_map(|bytes| {
        EntityId::from_content(&bytes)
    })
}

fn arb_attribute() -> impl Strategy<Value = Attribute> {
    ("[a-z]{2,8}", "[a-z]{2,8}").prop_map(|(ns, name)| {
        Attribute::new(&format!(":{ns}/{name}")).unwrap()
    })
}

fn arb_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<String>().prop_map(Value::String),
        any::<i64>().prop_map(Value::Long),
        any::<bool>().prop_map(Value::Boolean),
        arb_entity_id().prop_map(Value::Ref),
    ]
}

// Level 2: Datom strategy
fn arb_datom() -> impl Strategy<Value = Datom> {
    (arb_entity_id(), arb_attribute(), arb_value(), arb_tx_id(), arb_op())
        .prop_map(|(e, a, v, tx, op)| Datom { entity: e, attribute: a, value: v, tx, op })
}

// Level 3: Store strategy (genesis + random transactions)
fn arb_store(max_txs: usize) -> impl Strategy<Value = Store> {
    prop::collection::vec(arb_datom(), 0..max_txs * 5)
        .prop_map(|datoms| {
            let mut store = Store::genesis();
            // ... transact datoms in batches
            store
        })
}

// Level 3: Two overlapping stores (for merge tests)
fn arb_overlapping_stores(max_txs: usize) -> impl Strategy<Value = (Store, Store)> {
    (arb_store(max_txs), prop::collection::vec(arb_datom(), 0..max_txs * 3))
        .prop_map(|(base, extra)| {
            let s1 = base.clone();
            let mut s2 = base;
            // s2 gets extra datoms — stores share a common base
            for d in extra { s2.insert_datom(d); }
            (s1, s2)
        })
}

// Level 3: Schema strategy (axiomatic + random user attributes)
fn arb_schema(extra_attrs: usize) -> impl Strategy<Value = Store> {
    prop::collection::vec(arb_attribute_spec(), 0..extra_attrs)
        .prop_map(|specs| {
            let mut store = Store::genesis();
            for spec in specs { store.transact(Schema::new_attribute(&spec)); }
            store
        })
}

fn arb_attribute_spec() -> impl Strategy<Value = AttributeSpec> {
    (arb_attribute(), arb_value_type(), arb_cardinality(), arb_resolution_mode())
        .prop_map(|(ident, vt, card, rm)| AttributeSpec { ident, value_type: vt, cardinality: card, resolution_mode: rm, ..Default::default() })
}

fn arb_resolution_mode() -> impl Strategy<Value = ResolutionMode> {
    prop_oneof![
        Just(ResolutionMode::LastWriterWins),
        Just(ResolutionMode::MultiValue),
        // Lattice omitted — requires valid lattice entity in store
    ]
}

// Level 3: Harvest candidate strategy
fn arb_harvest_candidate() -> impl Strategy<Value = HarvestCandidate> {
    (0..100usize, prop::collection::vec(arb_datom(), 1..5), 0.0..1.0f64)
        .prop_map(|(id, datoms, confidence)| HarvestCandidate {
            id, datoms, confidence,
            category: HarvestCategory::Observation,
            source: "test".into(), weight: 1.0,
            reconciliation_type: ReconciliationType::Epistemic,
            status: HarvestStatus::Pending,
        })
}

// Level 3: Session context strategy
fn arb_session_context() -> impl Strategy<Value = SessionContext> {
    (arb_agent_id(), arb_tx_id(), ".*")
        .prop_map(|(agent, tx, desc)| SessionContext {
            agent, session_start_tx: tx,
            recent_transactions: vec![], task_description: desc,
        })
}

// Level 3: Conflict set strategy (for resolution tests)
fn arb_conflict_set() -> impl Strategy<Value = ConflictSet> {
    (arb_entity_id(), arb_attribute(),
     prop::collection::vec((arb_value(), arb_tx_id()), 2..5))
        .prop_map(|(e, a, assertions)| ConflictSet {
            entity: e, attribute: a, assertions, retractions: vec![],
        })
}

// Level 3: Kernel error strategy (for error recovery tests)
fn arb_kernel_error() -> impl Strategy<Value = KernelError> {
    prop_oneof![
        arb_attribute().prop_map(KernelError::UnknownAttribute),
        (arb_tx_id()).prop_map(KernelError::MissingCausalPredecessor),
        Just(KernelError::EmptyTransaction),
        ".*".prop_map(KernelError::QueryParseError),
    ]
}
```

### Property Naming Convention

Every proptest property is named after its INV ID:

```rust
proptest! {
    #[test]
    fn inv_store_001_append_only(
        store in arb_store(5),
        datoms in prop::collection::vec(arb_datom(), 1..10),
    ) {
        let pre_datoms: BTreeSet<_> = store.datoms().collect();
        let new_store = store.transact_raw(datoms);
        for d in &pre_datoms {
            prop_assert!(new_store.contains(d));
        }
    }

    #[test]
    fn inv_store_004_merge_commutative(
        s1 in arb_store(3),
        s2 in arb_store(3),
    ) {
        let m1 = s1.clone().merge(&s2);
        let m2 = s2.clone().merge(&s1);
        prop_assert_eq!(m1.datoms().collect::<BTreeSet<_>>(),
                        m2.datoms().collect::<BTreeSet<_>>());
    }
}
```

---

## §10.5 Quality Gate Protocol

### What blocks a commit

- Any Gate 1–2 failure
- Clippy warnings (`-D warnings`)
- Format violations

### What blocks a namespace completion

Before advancing from namespace N to namespace N+1:

1. All INVs for namespace N have proptest properties passing
2. All V:KANI-tagged INVs for namespace N have Kani harnesses passing
3. Integration test with prior namespaces passes
4. Three-box decomposition documented (black/state/clear for each core type)

### What blocks Stage 0 completion

1. All 61 Stage 0 INVs verified (proptest minimum, Kani where tagged)
2. Self-bootstrap test passes: spec elements transacted as datoms, queryable
3. Harvest/seed round-trip test: 25-turn → harvest → seed → resume without re-explanation
4. Dynamic CLAUDE.md generation produces valid output from store state

---

## §10.6 Defect Tracking

Defects discovered during verification are recorded as datoms in the store (C7 self-bootstrap).
Until the store exists, track defects as issues.

**Defect density target**: <1 defect per 100 lines of kernel code after Gate 2 passes.
This is a cleanroom software engineering standard metric.

---
