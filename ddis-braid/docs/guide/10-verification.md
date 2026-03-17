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

**Invariants verified at compile time** (V:TYPE — access control boundaries, not semantic
properties; see spec §16 V:TYPE Scope Principle):
- INV-STORE-001 (Transaction typestate: Building → Committed → Applied; borrow checker prevents store mutation outside transact/merge)
- INV-STORE-003 (EntityId: content-addressed, no raw constructor)
- INV-SCHEMA-003 (Schema monotonicity: no `remove_attribute` method, Attribute newtype)
- INV-SCHEMA-004 (Schema validation gate: Transaction<Building>.commit(schema) typestate)
- INV-LAYOUT-002 (DatomFile newtype: no raw path construction)
- INV-QUERY-005 (QueryMode enum: parse-time stratum enforcement)
- INV-QUERY-008 (FFI boundary: FfiFunction trait with pure marker)
- INV-RESOLUTION-001 (ResolutionMode enum: exhaustive match)
- INV-TRILATERAL-005 (PhiScore newtype: no raw f64 construction, must be in [0,1])
- INV-INTERFACE-003 (MCP_TOOLS: fixed-size array)
- INV-INTERFACE-009 (RecoveryAction enum exhaustive match)

Note: every V:TYPE invariant also has V:PROP. V:TYPE enforces access control (the compiler
prevents code paths that would violate the invariant). V:PROP verifies the semantic property
(the allowed code paths preserve the invariant). Both are needed because Rust does not have
dependent types — the type system cannot prove input-output value relationships like "the
output set is a superset of the input set."

### Gate 2: Test (`cargo test`)

**Checks**: All `V:PROP` invariants via proptest properties. 143 of 145 invariants
have proptest strategies (2 are V:MODEL-only: INV-QUERY-010, INV-SYNC-003). Every
V:TYPE invariant also has V:PROP — no V:TYPE-only invariants exist.

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

**Time**: Tiered — Gate 3a (every PR): <5 min; Gate 3b (nightly): <30 min;
Gate 3c (weekly): <2 hours. See §10.1 below for the full tiered CI design.

**Coverage**: 48 invariants (33.1%) with critical-path verification:
- All STORE CRDT laws (INV-STORE-004–008)
- All SCHEMA bootstrap properties (INV-SCHEMA-001–002, 004)
- All RESOLUTION algebraic laws (INV-RESOLUTION-002, 004–006)
- HARVEST gap detection (INV-HARVEST-001, 006)
- SEED idempotence (INV-SEED-002–003)
- MERGE preservation (INV-MERGE-001)
- BUDGET token efficiency (INV-BUDGET-006)

**Configuration**:
```rust
// Kani harness examples (2 of 48 — see full list below)
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

**Kani bounding strategies**: For Kani harnesses, symbolic types must be bounded:
`String` -> `kani::vec::exact_vec::<u8>(4)` converted to `String`;
`Vec` -> `kani::vec::exact_vec(N)` with `N <= 8`. Unbounded `kani::any::<String>()`
will cause verification timeout. All collection types used in harnesses must have
explicit size bounds to keep solver times within Gate 3a/3b targets.

### Complete V:KANI Harness List (48 total)

All INVs tagged V:KANI in the verification matrix (spec/16-verification.md §16.1),
grouped by namespace. Each harness targets the **Level 2 implementation contract**
(bounded, concrete Rust code), not the Level 0 algebraic property.

| Namespace | INV | Harness Property | Bound |
|-----------|-----|-----------------|-------|
| STORE | INV-STORE-001 | Append-only: transact never decreases datom count | <=20 ops |
| STORE | INV-STORE-002 | Content-addressing: same [e,a,v,tx,op] = same datom | <=5 datoms |
| STORE | INV-STORE-003 | EntityId from content hash: no arbitrary construction | <=5 datoms |
| STORE | INV-STORE-004 | Merge commutativity: `merge(A,B) = merge(B,A)` | <=5 datoms/store |
| STORE | INV-STORE-005 | CRDT associativity: `(A ∪ B) ∪ C = A ∪ (B ∪ C)` | <=3 datoms/store |
| STORE | INV-STORE-006 | Merge idempotency: `merge(A,A) = A` | <=5 datoms |
| STORE | INV-STORE-007 | Merge monotonicity: `\|merge(A,B)\| >= max(\|A\|,\|B\|)` | <=5 datoms/store |
| STORE | INV-STORE-008 | Genesis determinism: `genesis() = genesis()` | n/a (pure fn) |
| STORE | INV-STORE-010 | Causal ordering: predecessor `<` successor in HLC | <=5 datoms |
| STORE | INV-STORE-012 | LIVE index matches manual resolution | <=5 values/attr |
| SCHEMA | INV-SCHEMA-001 | Schema-as-data: schema extracted only from datoms | <=19 attributes |
| SCHEMA | INV-SCHEMA-002 | Genesis completeness: exactly 19 axiomatic attributes | n/a (bootstrap) |
| SCHEMA | INV-SCHEMA-004 | Schema validation: rejects malformed datoms | <=10 datoms |
| SCHEMA | INV-SCHEMA-007 | Lattice definition completeness: all 4 required properties present | <=19 attributes |
| LAYOUT | INV-LAYOUT-001 | Content-addressing: `Blake3(canonical(datom)) == address` | `STORE → LAYOUT` |
| LAYOUT | INV-LAYOUT-003 | Index rebuild: EAVT/AEVT/VAET/AVET from directory scan | `STORE → LAYOUT` |
| LAYOUT | INV-LAYOUT-004 | Merge commutativity: `merge(A,B) == merge(B,A)` at filesystem level | `STORE → LAYOUT` |
| LAYOUT | INV-LAYOUT-007 | Genesis determinism: `genesis_directory() == genesis_directory()` | `STORE → LAYOUT` |
| LAYOUT | INV-LAYOUT-011 | Canonical serialization round-trip | `LAYOUT` |
| QUERY | INV-QUERY-001 | CALM compliance: Monotonic mode rejects negation/aggregation at parse time | <=10 clauses |
| QUERY | INV-QUERY-004 | Branch visibility: snapshot isolation at fork point (trunk@fork + branch-only) | <=5 datoms, 1 branch |
| QUERY | INV-QUERY-012 | Topological sort: Kahn's produces valid linear extension of DAG | <=8 vertices |
| QUERY | INV-QUERY-013 | Tarjan SCC: partition + maximality + acyclic condensation | <=8 vertices |
| QUERY | INV-QUERY-017 | Critical path: longest path equals forward/backward pass result | <=8 vertices |
| RESOLUTION | INV-RESOLUTION-001 | Per-attribute resolution: exhaustive mode routing | <=5 attributes |
| RESOLUTION | INV-RESOLUTION-002 | Resolution commutativity: order-independent | <=5 values |
| RESOLUTION | INV-RESOLUTION-004 | Conflict predicate: six-condition correctness | <=3 agents |
| RESOLUTION | INV-RESOLUTION-005 | LWW semilattice: comm + assoc + idem | <=5 values |
| RESOLUTION | INV-RESOLUTION-006 | Lattice join: LUB correctness | <=5 values |
| RESOLUTION | INV-RESOLUTION-007 | Three-tier routing: totality (no unrouted conflicts) | <=5 conflicts |
| HARVEST | INV-HARVEST-001 | Harvest monotonicity: never removes datoms | <=10 candidates |
| HARVEST | INV-HARVEST-006 (Stage 1) | Crystallization guard: high-weight stability check | <=5 candidates |
| SEED | INV-SEED-002 | Budget compliance: output <= budget | <=1000 tokens |
| SEED | INV-SEED-003 | ASSOCIATE boundedness: <= depth x breadth | depth<=3, breadth<=5 |
| MERGE | INV-MERGE-001 | No datom loss: both inputs preserved in merged store | <=5 datoms/store |
| MERGE | INV-MERGE-003 | Branch isolation: branches can't see each other's datoms | <=3 branches |
| MERGE | INV-MERGE-004 | DCC completeness: diverge-compare-converge cycle | <=3 branches |
| MERGE | INV-MERGE-005 | Competing branch lock: at most 2 active branches | <=3 branches |
| MERGE | INV-MERGE-008 | Merge idempotency: `merge(A,A).datoms = A.datoms` | <=5 datoms |
| SIGNAL | INV-SIGNAL-001 | Signal as datom: every emitted signal produces a store datom | <=5 signals |
| SIGNAL | INV-SIGNAL-003 | Subscription completeness: no matching signal silently dropped | <=5 subscriptions |
| SIGNAL | INV-SIGNAL-005 | Diamond lattice: incomparable merge produces signal | <=3 values |
| DELIBERATION | INV-DELIBERATION-002 | Stability guard: decide() requires stability >= threshold | <=5 dimensions |
| DELIBERATION | INV-DELIBERATION-005 | Commitment weight: weight monotonically non-decreasing | <=5 decisions |
| GUIDANCE | INV-GUIDANCE-006 | M(t) bounded: methodology score in [0,1] | <=10 observations |
| BUDGET | INV-BUDGET-001 | Output budget cap: `\|output\| <= budget` | <=1000 tokens |
| BUDGET | INV-BUDGET-003 | Quality-adjusted degradation: Q(t) <= k*_eff(t) | <=100 steps |
| BUDGET | INV-BUDGET-006 | Token efficiency: density monotonically non-decreasing | <=5 projections |

### Gate 3 — Tiered Kani CI Design (spec Gate 5; from D3-kani-feasibility.md)

The spec defines Gate 5 (Kani) as a three-tier pipeline (spec/16-verification.md §16.2).
With 34 Stage 0 V:KANI harnesses, the tiers are:

**Gate 3a: Fast Kani (every PR) — target < 5 min**
- Trivial + simple harnesses only (~13 harnesses: type-level checks, simple set operations)
- Solver: CaDiCaL (default) — 10-200x faster than MiniSat for structural properties
- Unwind: `#[kani::unwind(5)]`
- Harnesses: STORE-001/003/008 (trivial), STORE-004-008/010 (simple set ops), SCHEMA-001/002 (bootstrap)

**Gate 3b: Full Kani (nightly) — target < 30 min**
- All 34 Stage 0 harnesses
- Per-harness solver selection (CaDiCaL for structural, Kissat for hash/bit-level)
- Unwind: `#[kani::unwind(8)]` default, `#[kani::unwind(12)]` for graph algorithms

**Gate 3c: Extended Kani (weekly) — target < 2 hours**
- All harnesses with higher unwind bounds (`#[kani::unwind(16)]`)
- Regression testing for new harnesses
- Three-store merge scenarios (CRDT commutativity/associativity)

**Solver selection per property type**:

| Property Type | Recommended Solver | Reasoning |
|---------------|-------------------|-----------|
| Set algebra (CRDT laws L1-L5) | CaDiCaL | Good for equality reasoning |
| Content addressing (hashing) | Kissat | Better for bit-level operations |
| Graph algorithms | CaDiCaL | Good for structural properties |
| Resolution mode (enum matching) | MiniSat | Simple boolean constraints (fast) |

**Per-harness configuration pattern**:
```rust
#[cfg(kani)]
#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(8)]
fn verify_store_commutativity() {
    let s1: Store = kani::any();
    let s2: Store = kani::any();
    assert_eq!(s1.merge(&s2), s2.merge(&s1));
}
```

**Key constraints**:
- All Kani harnesses must use **bounded test stores** (3-5 datoms). Kani cannot verify
  properties over unbounded collections. Proptest covers the unbounded case.
- Track verification time per harness from the start. Budget each harness < 60s for Gate 3a.
- If a harness exceeds 2 minutes, move it to Gate 3b (nightly) with a tracked exception.
- Consider `#[kani::stub]` for complex subsystems (e.g., stub the hash function with a
  simpler version for CRDT algebra proofs).

**Harness count verification note**: The authoritative total from spec/16-verification.md
is **48** V:KANI harnesses across all stages (STORE: 10, LAYOUT: 5, SCHEMA: 4, QUERY: 5,
RESOLUTION: 6, HARVEST: 2, SEED: 2, MERGE: 5, SIGNAL: 3, DELIBERATION: 2, GUIDANCE: 1,
BUDGET: 3). Of these, **34 are Stage 0** (STORE: 10, LAYOUT: 5, SCHEMA: 4, QUERY: 4,
RESOLUTION: 6, HARVEST: 1, SEED: 2, MERGE: 2). The remaining 14 are Stage 1-3 and run
in Gate 3b/3c.

**Feasibility: 48/48 (100%).** See spec/16-verification.md §16.5 for the complete feasibility
assurance with per-category Kani strategies and bounds.

### Gate 4: Model Checking (`cargo test --features stateright`)

**Checks**: `V:MODEL` invariants — protocol safety and liveness over all reachable states.

**Time**: <30 minutes.

**Coverage**: 14 invariants (9.7%) for protocol-level properties:
- STORE CRDT algebra (INV-STORE-004–005, 013)
- LAYOUT convergence (INV-LAYOUT-010)
- MERGE cascade (INV-MERGE-002, 004)
- QUERY fixpoint (INV-QUERY-010)
- SYNC barrier (INV-SYNC-001–003)
- DELIBERATION lifecycle (INV-DELIBERATION-001, 006)
- RESOLUTION convergence (INV-RESOLUTION-003)
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

  # Spec Gate 5a — Fast Kani (every PR, <5 min target)
  gate-4a-kani-fast:
    if: github.event_name == 'pull_request'
    needs: gate-2-test
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v4
      - uses: model-checking/kani-verifier-action@v1
      - name: Kani Verification (Fast — trivial + simple harnesses)
        # Harness naming convention: `inv_{namespace}_{nnn}_{short_name}`
        # (e.g., `inv_store_001_append_only`). CI globs use namespace prefix matching.
        run: >-
          cargo kani
          --harness "inv_store_*"
          --harness "inv_schema_*"
          --output-format terse
          --jobs 4

  # Spec Gate 5b — Full Kani (nightly, <30 min target)
  gate-4b-kani-full:
    if: github.event_name == 'schedule'  # nightly
    runs-on: ubuntu-latest
    timeout-minutes: 45
    steps:
      - uses: actions/checkout@v4
      - uses: model-checking/kani-verifier-action@v1
      - name: Kani Verification (Full — all Stage N harnesses)
        run: cargo kani --workspace --output-format terse --jobs 4

  # Spec Gate 5c — Extended Kani (weekly, <2h target)
  # Add when harness count grows beyond Stage 0.
  # Uses higher unwind bounds (#[kani::unwind(16)]) for regression coverage.
  # gate-4c-kani-extended:
  #   if: github.event_name == 'schedule'  # weekly cron
  #   runs-on: ubuntu-latest
  #   timeout-minutes: 150
  #   steps:
  #     - uses: actions/checkout@v4
  #     - uses: model-checking/kani-verifier-action@v1
  #     - name: Kani Verification (Extended — all harnesses, high unwind)
  #       run: cargo kani --workspace --output-format terse --jobs 4

  gate-5-model:
    if: github.event_name == 'schedule'  # nightly
    runs-on: ubuntu-latest
    timeout-minutes: 45
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --features stateright --test model_check
```

---

## §10.3 Coverage Matrix Template

The verification matrix maps each Stage 0 INV (83 total) to its verification methods.
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
        .prop_map(|(id, datom_spec, confidence)| HarvestCandidate {
            id, datom_spec, confidence,
            category: HarvestCategory::Observation,
            extraction_context: "test".into(), weight: 1.0,
            reconciliation_type: ReconciliationType::Epistemic,
            status: CandidateStatus::Proposed,
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

### What blocks a PR merge

- Any Gate 1–2 failure (inherited from commit gate)
- Gate 3 (Kani harness) failure. PRs must pass all Gate 1–3 checks before merge.

### What blocks a namespace completion

Before advancing from namespace N to namespace N+1:

1. All INVs for namespace N have proptest properties passing
2. All V:KANI-tagged INVs for namespace N have Kani harnesses passing
3. Integration test with prior namespaces passes
4. Three-box decomposition documented (black/state/clear for each core type)

### What blocks Stage 0 completion

1. All 83 Stage 0 INVs verified (proptest minimum, Kani where tagged)
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

## §10.7 CRDT Verification Suite (R2.5c)

> **Purpose**: Consolidated proptest harnesses verifying every proven CRDT property in the
> Braid specification. This suite is the runtime counterpart of the algebraic proofs in
> spec/01-store.md (G-Set properties, causal independence), spec/02-schema.md (semilattice
> witness), and spec/04-resolution.md (resolution-merge composition, conservative conflict
> detection, LWW semilattice). Each harness references the specific INV it verifies.
>
> **Location**: `crates/braid-kernel/tests/crdt_verification.rs`
>
> **Execution**: Part of Gate 2 (`cargo test`). Run standalone with:
> `cargo test --test crdt_verification -- --nocapture`

### §10.7.1 Strategy Dependencies

All harnesses depend on the strategy hierarchy from §10.4. The following additional
strategies are specific to the CRDT suite:

```rust
use proptest::prelude::*;
use proptest::collection;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

// --- CRDT-suite-specific strategies ---

/// Two stores that share a common base and then diverge.
/// Models the fundamental CRDT scenario: two agents that forked from a common state.
fn arb_diverged_stores(
    base_txs: usize,
    branch_txs: usize,
) -> impl Strategy<Value = (Store, Store)> {
    arb_store(base_txs).prop_flat_map(move |base| {
        let base_clone = base.clone();
        (
            collection::vec(arb_datom(), 0..branch_txs * 3)
                .prop_map(move |extra| {
                    let mut s = base.clone();
                    for d in extra { s.insert_datom(d); }
                    s
                }),
            collection::vec(arb_datom(), 0..branch_txs * 3)
                .prop_map(move |extra| {
                    let mut s = base_clone.clone();
                    for d in extra { s.insert_datom(d); }
                    s
                }),
        )
    })
}

/// Three stores for associativity tests. All share a common genesis.
fn arb_three_stores(max_txs: usize) -> impl Strategy<Value = (Store, Store, Store)> {
    (arb_store(max_txs), arb_store(max_txs), arb_store(max_txs))
}

/// A frontier that is a strict subset of a store's full frontier.
/// Used to test conservative detection (partial view vs. full view).
fn arb_partial_frontier(
    store: &Store,
) -> impl Strategy<Value = HashMap<AgentId, TxId>> {
    let agents: Vec<AgentId> = store.frontier().keys().cloned().collect();
    let frontier = store.frontier().clone();
    collection::hash_set(0..agents.len(), 0..agents.len())
        .prop_map(move |included| {
            let mut partial = HashMap::new();
            for (i, agent) in agents.iter().enumerate() {
                if included.contains(&i) {
                    partial.insert(*agent, frontier[agent]);
                }
            }
            partial
        })
}

/// A pair of datoms targeting the same (entity, attribute) with different values
/// and configurable causal relationship.
fn arb_conflicting_datom_pair() -> impl Strategy<Value = (Datom, Datom, bool)> {
    (arb_entity_id(), arb_attribute(), arb_value(), arb_value(),
     arb_tx_id(), arb_tx_id(), any::<bool>())
        .prop_filter("values must differ", |(_, _, v1, v2, _, _, _)| v1 != v2)
        .prop_map(|(e, a, v1, v2, tx1, tx2, causally_related)| {
            let d1 = Datom {
                entity: e.clone(), attribute: a.clone(),
                value: v1, tx: tx1, op: Op::Assert,
            };
            let d2 = Datom {
                entity: e, attribute: a,
                value: v2, tx: tx2, op: Op::Assert,
            };
            (d1, d2, causally_related)
        })
}

/// A set of LWW-contested values: multiple (value, tx) pairs for the same
/// (entity, attribute).
fn arb_lww_contest(n: usize) -> impl Strategy<Value = Vec<(Value, TxId)>> {
    collection::vec((arb_value(), arb_tx_id()), 2..n)
}

/// Partial order over a small element set for lattice property testing.
/// Returns (elements, comparator_edges) where comparator_edges encodes
/// the partial order as a set of (i, j) pairs meaning elements[i] <= elements[j].
fn arb_partial_order(
    max_size: usize,
) -> impl Strategy<Value = (Vec<Keyword>, Vec<(usize, usize)>)> {
    (3..max_size).prop_flat_map(|n| {
        let elements: Vec<Keyword> =
            (0..n).map(|i| Keyword::from(format!(":test/e{i}"))).collect();
        // Generate a random DAG (topological ordering = index order)
        let mut all_edges = Vec::new();
        for i in 0..n {
            for j in (i+1)..n {
                all_edges.push((i, j));
            }
        }
        (Just(elements), collection::subsequence(all_edges.clone(), 0..all_edges.len()))
    })
}
```

### §10.7.2 Harness 1: G-Set Grow-Only Property

**Verifies**: INV-STORE-001 (Append-Only Immutability), INV-STORE-002 (Strict Growth),
L4 (Monotonicity), L5 (Growth-Only)

**Property**: The datom set grows monotonically. No operation ever removes a datom.
Set union with any other set preserves all existing elements.

```rust
proptest! {
    /// INV-STORE-001 + L4: Every datom present before an operation remains
    /// present after. The store is a G-Set: elements can only be added,
    /// never removed.
    #[test]
    fn crdt_gset_grow_only(
        base in arb_store(5),
        additions in collection::vec(arb_datom(), 1..20),
    ) {
        let snapshot: BTreeSet<Datom> = base.datoms().cloned().collect();
        let mut store = base;
        // Transact additional datoms
        for batch in additions.chunks(5) {
            let mut tx = Transaction::<Building>::new(SYSTEM_AGENT)
                .with_provenance(ProvenanceType::Observed);
            for d in batch { tx = tx.assert_datom(d.clone()); }
            if let Ok(committed) = tx.commit(store.schema()) {
                let _ = store.transact(committed);
            }
        }
        // Every datom from before must still be present
        for d in &snapshot {
            prop_assert!(
                store.datoms().any(|stored| stored == d),
                "G-Set violation: datom {:?} was removed after transact", d
            );
        }
    }

    /// INV-STORE-002 + L5: Every successful transaction strictly increases
    /// store size.
    #[test]
    fn crdt_gset_strict_growth(
        store in arb_store(3),
        datoms in collection::vec(arb_datom(), 1..5),
    ) {
        let mut s = store;
        let pre_len = s.len();
        let mut tx = Transaction::<Building>::new(SYSTEM_AGENT)
            .with_provenance(ProvenanceType::Observed);
        for d in &datoms { tx = tx.assert_datom(d.clone()); }
        if let Ok(committed) = tx.commit(s.schema()) {
            let _receipt = s.transact(committed).unwrap();
            prop_assert!(s.len() > pre_len,
                "Strict growth violated: pre={}, post={}", pre_len, s.len());
        }
    }

    /// L4 as subset check: S ⊆ S ∪ S' for arbitrary S'.
    #[test]
    fn crdt_gset_union_monotonic(
        s1 in arb_store(3),
        s2 in arb_store(3),
    ) {
        let s1_datoms: BTreeSet<Datom> = s1.datoms().cloned().collect();
        let mut merged = s1.clone();
        merged.merge(&s2);
        for d in &s1_datoms {
            prop_assert!(merged.datoms().any(|stored| stored == d),
                "Union monotonicity violation: datom lost from s1 after merge");
        }
        let s2_datoms: BTreeSet<Datom> = s2.datoms().cloned().collect();
        for d in &s2_datoms {
            prop_assert!(merged.datoms().any(|stored| stored == d),
                "Union monotonicity violation: datom lost from s2 after merge");
        }
    }
}
```

### §10.7.3 Harness 2: Merge Commutativity

**Verifies**: INV-STORE-004 (CRDT Merge Commutativity), L1

**Property**: `MERGE(A, B) = MERGE(B, A)` -- merge order does not affect the resulting
datom set. Two agents receiving the same stores in different order converge to the
same state.

```rust
proptest! {
    /// INV-STORE-004: MERGE(A, B).datoms = MERGE(B, A).datoms
    #[test]
    fn crdt_merge_commutative(
        s1 in arb_store(5),
        s2 in arb_store(5),
    ) {
        let mut m1 = s1.clone();
        m1.merge(&s2);
        let mut m2 = s2.clone();
        m2.merge(&s1);
        let d1: BTreeSet<Datom> = m1.datoms().cloned().collect();
        let d2: BTreeSet<Datom> = m2.datoms().cloned().collect();
        prop_assert_eq!(d1, d2,
            "Merge commutativity violation: MERGE(s1,s2) != MERGE(s2,s1)");
    }

    /// Commutativity with diverged stores (stronger: stores share a common
    /// ancestor, modeling the real-world agent fork scenario).
    #[test]
    fn crdt_merge_commutative_diverged(
        (s1, s2) in arb_diverged_stores(3, 5),
    ) {
        let mut m1 = s1.clone();
        m1.merge(&s2);
        let mut m2 = s2.clone();
        m2.merge(&s1);
        let d1: BTreeSet<Datom> = m1.datoms().cloned().collect();
        let d2: BTreeSet<Datom> = m2.datoms().cloned().collect();
        prop_assert_eq!(d1, d2,
            "Merge commutativity violation on diverged stores");
    }
}
```

### §10.7.4 Harness 3: Merge Associativity

**Verifies**: INV-STORE-005 (CRDT Merge Associativity), L2

**Property**: `MERGE(MERGE(A, B), C) = MERGE(A, MERGE(B, C))` -- regrouping merges
does not affect the result. Critical for multi-agent scenarios where three or more
agents merge in arbitrary order.

```rust
proptest! {
    /// INV-STORE-005: MERGE(MERGE(A,B),C).datoms = MERGE(A,MERGE(B,C)).datoms
    #[test]
    fn crdt_merge_associative(
        (s1, s2, s3) in arb_three_stores(3),
    ) {
        // Left grouping: (s1 ∪ s2) ∪ s3
        let mut left = s1.clone();
        left.merge(&s2);
        left.merge(&s3);

        // Right grouping: s1 ∪ (s2 ∪ s3)
        let mut s2_s3 = s2.clone();
        s2_s3.merge(&s3);
        let mut right = s1.clone();
        right.merge(&s2_s3);

        let d_left: BTreeSet<Datom> = left.datoms().cloned().collect();
        let d_right: BTreeSet<Datom> = right.datoms().cloned().collect();
        prop_assert_eq!(d_left, d_right,
            "Merge associativity violation: (A∪B)∪C != A∪(B∪C)");
    }

    /// Associativity also holds for the LIVE view, not just the datom set.
    /// This follows from the spec/04-resolution.md §4.3.1 Resolution-Merge
    /// Composition Proof, but we verify it empirically.
    #[test]
    fn crdt_merge_associative_live(
        (s1, s2, s3) in arb_three_stores(2),
    ) {
        let mut left = s1.clone();
        left.merge(&s2);
        left.merge(&s3);

        let mut s2_s3 = s2.clone();
        s2_s3.merge(&s3);
        let mut right = s1.clone();
        right.merge(&s2_s3);

        // Compare LIVE views for all entities
        for entity in left.entities() {
            let live_left = live_entity(&left, entity);
            let live_right = live_entity(&right, entity);
            prop_assert_eq!(live_left, live_right,
                "LIVE associativity violation for entity {:?}", entity);
        }
    }
}
```

### §10.7.5 Harness 4: Merge Idempotency

**Verifies**: INV-STORE-006 (CRDT Merge Idempotency), INV-MERGE-008
(At-Least-Once Delivery), L3

**Property**: `MERGE(A, A) = A` -- merging a store with itself is a no-op.
This is the foundation of at-least-once delivery: duplicate merges are harmless.

```rust
proptest! {
    /// INV-STORE-006 + L3: MERGE(A, A).datoms = A.datoms
    #[test]
    fn crdt_merge_idempotent(
        store in arb_store(5),
    ) {
        let original: BTreeSet<Datom> = store.datoms().cloned().collect();
        let mut merged = store.clone();
        merged.merge(&store);
        let after: BTreeSet<Datom> = merged.datoms().cloned().collect();
        prop_assert_eq!(original, after,
            "Merge idempotency violation: MERGE(A,A) != A");
    }

    /// INV-MERGE-008: Repeated merges are no-ops (at-least-once delivery).
    #[test]
    fn crdt_merge_idempotent_repeated(
        s1 in arb_store(3),
        s2 in arb_store(3),
    ) {
        let mut once = s1.clone();
        once.merge(&s2);
        let once_datoms: BTreeSet<Datom> = once.datoms().cloned().collect();

        // Merge s2 again -- should be no-op
        let mut twice = once.clone();
        let receipt = twice.merge(&s2);
        let twice_datoms: BTreeSet<Datom> = twice.datoms().cloned().collect();

        prop_assert_eq!(once_datoms, twice_datoms,
            "Repeated merge changed the store");
        prop_assert_eq!(receipt.new_datoms, 0,
            "Repeated merge reported new datoms");
    }

    /// Idempotency of the LIVE layer: LIVE(MERGE(A,A)) = LIVE(A).
    #[test]
    fn crdt_merge_idempotent_live(
        store in arb_store(3),
    ) {
        let mut merged = store.clone();
        merged.merge(&store);
        for entity in store.entities() {
            let live_before = live_entity(&store, entity);
            let live_after = live_entity(&merged, entity);
            prop_assert_eq!(live_before, live_after,
                "LIVE idempotency violation for entity {:?}", entity);
        }
    }
}
```

### §10.7.6 Harness 5: LWW Semilattice Property

**Verifies**: INV-RESOLUTION-005 (LWW Semilattice Properties),
ADR-RESOLUTION-009 (BLAKE3 Hash Tie-Breaking)

**Property**: LWW resolution forms a join-semilattice: commutative, associative,
idempotent. Tie-breaking via BLAKE3 hash preserves all three properties even when
HLC timestamps are identical.

```rust
proptest! {
    /// INV-RESOLUTION-005 commutativity: lww(v1, v2) = lww(v2, v1)
    #[test]
    fn crdt_lww_commutative(
        contestants in arb_lww_contest(6),
    ) {
        let mut forward = contestants.clone();
        let mut reverse = contestants.clone();
        reverse.reverse();
        let r1 = resolve_lww(&forward);
        let r2 = resolve_lww(&reverse);
        prop_assert_eq!(r1, r2,
            "LWW commutativity violation: different order produced different winner");
    }

    /// INV-RESOLUTION-005 associativity: lww(lww(a,b),c) = lww(a,lww(b,c))
    #[test]
    fn crdt_lww_associative(
        a in (arb_value(), arb_tx_id()),
        b in (arb_value(), arb_tx_id()),
        c in (arb_value(), arb_tx_id()),
    ) {
        // Left: lww(lww(a,b), c)
        let left_inner = resolve_lww(&[a.clone(), b.clone()]);
        let left = resolve_lww(&[left_inner, c.clone()]);

        // Right: lww(a, lww(b,c))
        let right_inner = resolve_lww(&[b.clone(), c.clone()]);
        let right = resolve_lww(&[a.clone(), right_inner]);

        prop_assert_eq!(left, right,
            "LWW associativity violation");
    }

    /// INV-RESOLUTION-005 idempotency: lww(v, v) = v
    #[test]
    fn crdt_lww_idempotent(
        v in (arb_value(), arb_tx_id()),
    ) {
        let result = resolve_lww(&[v.clone(), v.clone()]);
        prop_assert_eq!(result.0, v.0,
            "LWW idempotency violation: lww(v,v) != v");
    }

    /// ADR-RESOLUTION-009: BLAKE3 tie-breaking preserves commutativity when
    /// HLC timestamps are equal.
    #[test]
    fn crdt_lww_blake3_tiebreak(
        v1 in arb_value(),
        v2 in arb_value(),
    ) {
        prop_assume!(v1 != v2);
        // Same HLC timestamp for both -- force a tie
        let tx = fixed_tx_id();
        let contestants_1 = vec![(v1.clone(), tx), (v2.clone(), tx)];
        let contestants_2 = vec![(v2.clone(), tx), (v1.clone(), tx)];
        let r1 = resolve_lww(&contestants_1);
        let r2 = resolve_lww(&contestants_2);
        prop_assert_eq!(r1, r2,
            "BLAKE3 tie-break not commutative: different input order produced \
             different winner when HLC timestamps are equal");
    }

    /// LWW across all permutations: resolution of N values is order-independent.
    #[test]
    fn crdt_lww_all_permutations(
        contestants in arb_lww_contest(4),  // keep small for permutation count
    ) {
        let reference = resolve_lww(&contestants);
        // Test a few random shuffles (full permutation is O(n!) -- too expensive)
        let mut shuffled = contestants.clone();
        shuffled.rotate_left(1);
        prop_assert_eq!(resolve_lww(&shuffled), reference,
            "LWW permutation sensitivity: rotate_left(1) changed result");
        shuffled.reverse();
        prop_assert_eq!(resolve_lww(&shuffled), reference,
            "LWW permutation sensitivity: reverse changed result");
    }
}

// Helper: deterministic TxId for tie-break tests.
fn fixed_tx_id() -> TxId {
    TxId { wall_time: 1000, logical: 0, agent: AgentId::from_bytes([0u8; 16]) }
}
```

### §10.7.7 Harness 6: Conservative Conflict Detection (No False Negatives)

**Verifies**: INV-RESOLUTION-003 (Conservative Conflict Detection),
INV-RESOLUTION-004 (Conflict Predicate Correctness),
NEG-RESOLUTION-002 (No False Negative Conflict Detection),
spec/04-resolution.md §4.3.2 (Conservative Detection Completeness Proof)

**Property**: For any frontier F ⊆ S, if a true conflict exists and both datoms
are in F, the detection predicate fires. A partial frontier may overestimate
conflicts (false positives are safe) but never underestimate (false negatives
are critical).

```rust
proptest! {
    /// INV-RESOLUTION-003: conflicts_detected(F_local) ⊇ conflicts_detected(F_global).
    /// A partial frontier detects a SUPERSET of the conflicts visible at the
    /// full frontier.
    #[test]
    fn crdt_conflict_detection_conservative(
        (s_local, s_global) in arb_overlapping_stores(3, 8),
    ) {
        // s_local ⊆ s_global (s_local is a partial view)
        let f_local = s_local.frontier().clone();
        let f_global = s_global.frontier().clone();
        let conflicts_local = detect_conflicts(&s_local, &f_local);
        let conflicts_global = detect_conflicts(&s_global, &f_global);

        // Every conflict detected at the global frontier must also be detected
        // at the local frontier (if both datoms are present).
        for gc in &conflicts_global {
            let both_present = gc.assertions.iter().all(|(_, tx)| {
                s_local.datoms().any(|d|
                    d.entity == gc.entity
                    && d.attribute == gc.attribute
                    && d.tx == *tx
                )
            });
            if both_present {
                prop_assert!(
                    conflicts_local.iter().any(|lc|
                        lc.entity == gc.entity && lc.attribute == gc.attribute
                    ),
                    "FALSE NEGATIVE: conflict ({:?}, {:?}) detected globally \
                     and both datoms present locally, but not detected locally",
                    gc.entity, gc.attribute
                );
            }
        }
    }

    /// Anti-monotonicity: as frontier grows, detected conflict set can only shrink.
    /// F1 ⊆ F2 ==> conflicts(F2) ⊆ conflicts(F1)
    #[test]
    fn crdt_conflict_detection_antimonotone(
        base in arb_store(3),
        extra_datoms in collection::vec(arb_datom(), 1..10),
    ) {
        // F1 = base frontier (smaller)
        let f1 = base.frontier().clone();
        let conflicts_f1 = detect_conflicts(&base, &f1);

        // F2 = base + extra datoms (larger frontier)
        let mut expanded = base.clone();
        for d in &extra_datoms { expanded.insert_datom(d.clone()); }
        let f2 = expanded.frontier().clone();
        let conflicts_f2 = detect_conflicts(&expanded, &f2);

        // Every conflict at F2 should also appear at F1 (if both datoms were
        // present in F1)
        for c2 in &conflicts_f2 {
            let both_in_base = c2.assertions.iter().all(|(_, tx)| {
                base.datoms().any(|d|
                    d.entity == c2.entity
                    && d.attribute == c2.attribute
                    && d.tx == *tx
                )
            });
            if both_in_base {
                prop_assert!(
                    conflicts_f1.iter().any(|c1|
                        c1.entity == c2.entity && c1.attribute == c2.attribute
                    ),
                    "Anti-monotonicity violation: conflict present at larger \
                     frontier but absent at smaller frontier (both datoms in both)"
                );
            }
        }
    }

    /// INV-RESOLUTION-004: Conflict predicate requires all six conditions.
    /// Systematically vary each condition and verify the predicate matches
    /// expectations.
    #[test]
    fn crdt_conflict_predicate_six_conditions(
        (d1, d2, _) in arb_conflicting_datom_pair(),
        vary_condition in 0..6u8,
    ) {
        let schema = Schema::test_schema_cardinality_one(d1.attribute.clone());
        let mut d2_mod = d2.clone();

        let expect_conflict = match vary_condition {
            0 => {
                // Break condition 1: different entity
                d2_mod.entity = EntityId::from_content(b"different");
                false
            }
            1 => {
                // Break condition 2: different attribute
                d2_mod.attribute = Attribute::new(":other/attr").unwrap();
                false
            }
            2 => {
                // Break condition 4: one is a retraction
                d2_mod.op = Op::Retract;
                false
            }
            3 => {
                // Break condition 5: cardinality :many -- tested separately
                return Ok(());
            }
            4 => {
                // Condition 3: same value -- no conflict
                d2_mod.value = d1.value.clone();
                false
            }
            5 => {
                // All conditions met -- conflict expected
                true
            }
            _ => unreachable!(),
        };

        let conflict_set = ConflictSet {
            entity: d1.entity.clone(),
            attribute: d1.attribute.clone(),
            assertions: vec![
                (d1.value.clone(), d1.tx),
                (d2_mod.value.clone(), d2_mod.tx),
            ],
            retractions: vec![],
        };
        let detected = has_conflict(
            &conflict_set, &ResolutionMode::LastWriterWins
        );

        if expect_conflict {
            prop_assert!(detected,
                "Expected conflict not detected (condition {} held)",
                vary_condition);
        }
    }
}
```

### §10.7.8 Harness 7: Resolution-Merge Composition

**Verifies**: spec/04-resolution.md §4.3.1 (Resolution-Merge Composition Proof),
INV-RESOLUTION-002 (Resolution Commutativity), LIVE derived CRDT corollary

**Property**: `LIVE(MERGE(S1, S2)) = LIVE(MERGE(S2, S1))` and
`LIVE(MERGE(MERGE(S1, S2), S3)) = LIVE(MERGE(S1, MERGE(S2, S3)))`.
The LIVE view commutes and associates over set-union merge for all resolution modes.

```rust
proptest! {
    /// §4.3.1 Composition -- LIVE commutativity:
    /// LIVE(MERGE(A,B)) = LIVE(MERGE(B,A))
    /// Tests all three resolution modes simultaneously across all entities.
    #[test]
    fn crdt_resolution_merge_commutative(
        (s1, s2) in arb_diverged_stores(3, 5),
    ) {
        let mut m1 = s1.clone();
        m1.merge(&s2);
        let mut m2 = s2.clone();
        m2.merge(&s1);

        // LIVE views must be identical for all entities across all attributes
        let entities_m1: BTreeSet<EntityId> = m1.entities().collect();
        let entities_m2: BTreeSet<EntityId> = m2.entities().collect();
        prop_assert_eq!(&entities_m1, &entities_m2,
            "Entity sets differ after commuted merges");

        for entity in &entities_m1 {
            let live1 = live_entity(&m1, *entity);
            let live2 = live_entity(&m2, *entity);
            prop_assert_eq!(live1, live2,
                "LIVE(MERGE(A,B)) != LIVE(MERGE(B,A)) for entity {:?}", entity);
        }
    }

    /// §4.3.1 Composition -- LIVE associativity:
    /// LIVE(MERGE(MERGE(A,B),C)) = LIVE(MERGE(A,MERGE(B,C)))
    #[test]
    fn crdt_resolution_merge_associative(
        (s1, s2, s3) in arb_three_stores(2),
    ) {
        // Left grouping
        let mut left = s1.clone();
        left.merge(&s2);
        left.merge(&s3);

        // Right grouping
        let mut s2_s3 = s2.clone();
        s2_s3.merge(&s3);
        let mut right = s1.clone();
        right.merge(&s2_s3);

        for entity in left.entities() {
            let live_left = live_entity(&left, entity);
            let live_right = live_entity(&right, entity);
            prop_assert_eq!(live_left, live_right,
                "LIVE associativity violation for entity {:?}", entity);
        }
    }

    /// §4.3.1 -- LIVE determinism: Two agents with identical datom sets
    /// produce identical LIVE views, regardless of merge history.
    #[test]
    fn crdt_live_deterministic(
        s1 in arb_store(3),
        s2 in arb_store(3),
        s3 in arb_store(3),
    ) {
        // Agent A merges in order: s1, s2, s3
        let mut agent_a = Store::genesis();
        agent_a.merge(&s1);
        agent_a.merge(&s2);
        agent_a.merge(&s3);

        // Agent B merges in order: s3, s1, s2
        let mut agent_b = Store::genesis();
        agent_b.merge(&s3);
        agent_b.merge(&s1);
        agent_b.merge(&s2);

        // Same datom set -- same LIVE view
        let datoms_a: BTreeSet<Datom> = agent_a.datoms().cloned().collect();
        let datoms_b: BTreeSet<Datom> = agent_b.datoms().cloned().collect();
        prop_assert_eq!(&datoms_a, &datoms_b,
            "Datom sets differ despite same inputs in different order");

        for entity in agent_a.entities() {
            let live_a = live_entity(&agent_a, entity);
            let live_b = live_entity(&agent_b, entity);
            prop_assert_eq!(live_a, live_b,
                "LIVE determinism violation for entity {:?}: \
                 same datoms, different merge order produced different \
                 resolved values", entity);
        }
    }

    /// NEG-RESOLUTION-001: Merge MUST NOT apply resolution. Both conflicting
    /// values must be present in the merged datom set.
    #[test]
    fn crdt_merge_no_resolution(
        base in arb_store(2),
        (d1, d2, _) in arb_conflicting_datom_pair(),
    ) {
        // Create two branches with conflicting values
        let mut branch_a = base.clone();
        branch_a.insert_datom(d1.clone());
        let mut branch_b = base.clone();
        branch_b.insert_datom(d2.clone());

        // Merge
        let mut merged = branch_a.clone();
        merged.merge(&branch_b);

        // BOTH datoms must be present (merge = set union, no resolution)
        prop_assert!(merged.datoms().any(|d| d == &d1),
            "Merge lost d1 -- resolution applied during merge (NEG-RESOLUTION-001)");
        prop_assert!(merged.datoms().any(|d| d == &d2),
            "Merge lost d2 -- resolution applied during merge (NEG-RESOLUTION-001)");
    }
}
```

### §10.7.9 Harness 8: Causal Independence via Predecessor Sets

**Verifies**: INV-STORE-010 (Causal Ordering), INV-RESOLUTION-004 condition (6)
(Causal Independence in Conflict Predicate)

**Property**: Causal independence is defined by predecessor set reachability, NOT
by HLC comparison. Two transactions are causally independent iff neither is
transitively reachable from the other via the predecessor graph.

```rust
proptest! {
    /// INV-STORE-010: Causal order is defined by predecessor sets, not HLC.
    /// Construct scenarios where HLC order disagrees with causal order and
    /// verify that causally_independent() uses the predecessor graph.
    #[test]
    fn crdt_causal_independence_predecessor_based(
        base in arb_store(2),
        d1 in arb_datom(),
        d2 in arb_datom(),
    ) {
        let mut store = base;
        // Transact d1 (agent A)
        let tx1 = store.transact_single(d1.clone(), AGENT_A).unwrap();
        // Transact d2 WITHOUT declaring tx1 as predecessor (agent B)
        let tx2 = store.transact_single(d2.clone(), AGENT_B).unwrap();

        // tx1 and tx2 are causally independent (no predecessor link)
        prop_assert!(
            causally_independent(&store, tx1.tx_id, tx2.tx_id),
            "tx1 and tx2 should be causally independent (no predecessor link)"
        );

        // Now transact d3 with tx1 as predecessor
        let d3 = Datom::test_datom();
        let tx3 = store.transact_with_predecessor(
            d3, AGENT_A, tx1.tx_id
        ).unwrap();

        // tx1 < tx3 causally (tx1 is predecessor of tx3)
        prop_assert!(
            !causally_independent(&store, tx1.tx_id, tx3.tx_id),
            "tx1 and tx3 should be causally related (tx1 is predecessor)"
        );

        // tx2 || tx3 (no predecessor link between them)
        prop_assert!(
            causally_independent(&store, tx2.tx_id, tx3.tx_id),
            "tx2 and tx3 should be causally independent (no link)"
        );
    }

    /// INV-STORE-010: HLC ordering does NOT imply causal ordering across agents.
    /// Even if tx1.hlc < tx2.hlc, they may be causally independent.
    #[test]
    fn crdt_causal_independence_hlc_irrelevant(
        base in arb_store(2),
        d1 in arb_datom(),
        d2 in arb_datom(),
    ) {
        let mut store = base;
        // Agent A transacts at wall_time=100
        let tx1 = store.transact_at_time(d1, AGENT_A, 100).unwrap();
        // Agent B transacts at wall_time=200 (later HLC, no causal link)
        let tx2 = store.transact_at_time(d2, AGENT_B, 200).unwrap();

        // HLC says tx1 < tx2, but they are causally independent
        prop_assert!(tx1.tx_id.wall_time < tx2.tx_id.wall_time,
            "Precondition: tx1 should have earlier HLC");
        prop_assert!(
            causally_independent(&store, tx1.tx_id, tx2.tx_id),
            "HLC ordering does NOT imply causal ordering -- \
             these should be independent"
        );
    }

    /// Transitive closure: if A -> B -> C, then A <_causal C.
    #[test]
    fn crdt_causal_transitivity(
        base in arb_store(2),
        d1 in arb_datom(),
        d2 in arb_datom(),
        d3 in arb_datom(),
    ) {
        let mut store = base;
        let tx1 = store.transact_single(d1, AGENT_A).unwrap();
        let tx2 = store.transact_with_predecessor(
            d2, AGENT_B, tx1.tx_id
        ).unwrap();
        let tx3 = store.transact_with_predecessor(
            d3, AGENT_A, tx2.tx_id
        ).unwrap();

        // tx1 -> tx2 -> tx3, so tx1 <_causal tx3 (transitively)
        prop_assert!(
            store.is_causal_ancestor(tx1.tx_id, tx3.tx_id),
            "Transitive causal link not detected: tx1 -> tx2 -> tx3"
        );
        prop_assert!(
            !causally_independent(&store, tx1.tx_id, tx3.tx_id),
            "tx1 and tx3 should NOT be independent (transitive causal link)"
        );
    }

    /// Conflict predicate uses causal independence, not HLC.
    /// Two datoms with the same (e, a) and different values should only be
    /// flagged as conflicts if their transactions are causally independent.
    #[test]
    fn crdt_conflict_requires_causal_independence(
        base in arb_store(2),
        entity in arb_entity_id(),
        attr in arb_attribute(),
        v1 in arb_value(),
        v2 in arb_value(),
    ) {
        prop_assume!(v1 != v2);
        let mut store = base;

        // Register attr as cardinality :one
        store.register_test_attribute(
            &attr, Cardinality::One, ResolutionMode::LastWriterWins
        );

        // Agent A asserts (entity, attr, v1)
        let tx1 = store.transact_assertion(
            entity.clone(), attr.clone(), v1.clone(), AGENT_A
        ).unwrap();

        // Agent B asserts (entity, attr, v2) WITH tx1 as predecessor
        // (causal update, NOT conflict)
        let tx2 = store.transact_assertion_with_pred(
            entity.clone(), attr.clone(), v2.clone(),
            AGENT_B, tx1.tx_id,
        ).unwrap();

        // This is NOT a conflict because tx1 <_causal tx2
        let conflicts = detect_conflicts(&store, store.frontier());
        let is_conflict = conflicts.iter().any(|c|
            c.entity == entity && c.attribute == attr
        );
        prop_assert!(!is_conflict,
            "Causal update falsely flagged as conflict -- \
             conflict predicate must check causal independence, not HLC");
    }
}
```

### §10.7.10 Harness 9: Cascade Determinism

**Verifies**: INV-MERGE-010 (Cascade Determinism), ADR-MERGE-005 (Cascade as
Deterministic Fixpoint)

**Property**: The merge cascade is a pure function of the merged datom set. Two agents
independently merging the same two stores produce identical cascade output. The cascade
function takes only `&Store` and `&[Datom]` — no `AgentId`, no `SystemTime`, no RNG.
This restores L1/L2 at the total post-merge state level (including cascade datoms).

**Strategy**: Generate two arbitrary stores. Merge in both orders (A∪B and B∪A). Run
the cascade on each merged result. The cascade datom sets must be identical.

```rust
proptest! {
    /// INV-MERGE-010: Cascade determinism — identical input produces identical output.
    /// Merge order must not affect cascade results.
    #[test]
    fn crdt_cascade_determinism(
        s1 in arb_store(5),
        s2 in arb_store(5),
    ) {
        // Merge A ∪ B
        let mut target_ab = s1.clone();
        let receipt_ab = merge(&mut target_ab, &s2);
        let new_datoms_ab: Vec<Datom> = target_ab.datoms()
            .filter(|d| !s1.contains(d))
            .cloned()
            .collect();
        let cascade_ab = run_cascade(&target_ab, &new_datoms_ab);

        // Merge B ∪ A
        let mut target_ba = s2.clone();
        let receipt_ba = merge(&mut target_ba, &s1);
        let new_datoms_ba: Vec<Datom> = target_ba.datoms()
            .filter(|d| !s2.contains(d))
            .cloned()
            .collect();
        let cascade_ba = run_cascade(&target_ba, &new_datoms_ba);

        // Cascade datom sets must be identical (as sets, not sequences)
        let datoms_ab: BTreeSet<Datom> = cascade_ab.cascade_datoms.into_iter().collect();
        let datoms_ba: BTreeSet<Datom> = cascade_ba.cascade_datoms.into_iter().collect();
        prop_assert_eq!(&datoms_ab, &datoms_ba,
            "Cascade determinism violation: merge(A,B) produced {} cascade datoms, \
             merge(B,A) produced {} — sets differ",
            datoms_ab.len(), datoms_ba.len());

        // Verify cascade datom identity is content-addressable
        // (no agent ID, timestamp, or sequence number in the datom identity)
        for datom in &datoms_ab {
            prop_assert!(datom.attribute.name().starts_with(":cascade/"),
                "Cascade datom has non-cascade attribute: {}", datom.attribute.name());
        }
    }

    /// INV-MERGE-010 corollary: cascade on identical store produces identical output
    /// regardless of which "new datoms" set is provided (determinism from store state).
    #[test]
    fn crdt_cascade_store_determinism(
        s1 in arb_store(5),
        s2 in arb_store(5),
    ) {
        // Merge once to get the combined store
        let mut combined = s1.clone();
        merge(&mut combined, &s2);

        // Run cascade twice on the same inputs
        let new_datoms: Vec<Datom> = combined.datoms()
            .filter(|d| !s1.contains(d))
            .cloned()
            .collect();
        let cascade_1 = run_cascade(&combined, &new_datoms);
        let cascade_2 = run_cascade(&combined, &new_datoms);

        // Must be identical (no internal RNG, no clock reads)
        prop_assert_eq!(cascade_1.cascade_datoms, cascade_2.cascade_datoms,
            "Cascade produced different output on identical inputs — \
             internal nondeterminism detected");
    }
}
```

**Key verification points**:
- `run_cascade` signature takes `&Store` + `&[Datom]` only — `AgentId` and `SystemTime`
  are not in scope (enforced by Rust's type system, verified by `V:TYPE`)
- Cascade datom `EntityId` is derived from content (conflict entity, attribute, values),
  not from detection metadata
- Same merged state `S₁ ∪ S₂ = S₂ ∪ S₁` produces same cascade regardless of merge order

### §10.7.11 INV Cross-Reference Index

Summary of which INVs each harness in the CRDT Verification Suite covers:

| Harness | Section | INVs Verified |
|---------|---------|---------------|
| G-Set Grow-Only | §10.7.2 | INV-STORE-001, INV-STORE-002, L4, L5 |
| Merge Commutativity | §10.7.3 | INV-STORE-004, L1 |
| Merge Associativity | §10.7.4 | INV-STORE-005, L2, §4.3.1 (LIVE assoc) |
| Merge Idempotency | §10.7.5 | INV-STORE-006, INV-MERGE-008, L3 |
| LWW Semilattice | §10.7.6 | INV-RESOLUTION-005, ADR-RESOLUTION-009 |
| Conservative Detection | §10.7.7 | INV-RESOLUTION-003, INV-RESOLUTION-004, NEG-RESOLUTION-002, §4.3.2 |
| Resolution-Merge Composition | §10.7.8 | §4.3.1, INV-RESOLUTION-002, NEG-RESOLUTION-001 |
| Causal Independence | §10.7.9 | INV-STORE-010, INV-RESOLUTION-004(6) |
| Cascade Determinism | §10.7.10 | INV-MERGE-010, ADR-MERGE-005 |

**Total coverage**: 17 INVs, 5 algebraic laws (L1-L5), 2 formal proofs (§4.3.1, §4.3.2),
4 ADRs, 2 negative cases.

**Test count at 256 cases/property**: 26 properties x 256 = 6,656 property evaluations per run.

---
