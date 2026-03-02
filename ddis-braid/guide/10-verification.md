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

### Gate 2: Test (`cargo test`)

**Checks**: All `V:PROP` invariants via proptest properties. Every one of the 104 invariants
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

**Coverage**: 42 invariants (40.4%) with critical-path verification:
- All STORE CRDT laws (INV-STORE-004–008)
- All SCHEMA bootstrap properties (INV-SCHEMA-001–002, 004)
- All RESOLUTION algebraic laws (INV-RESOLUTION-002, 004–006)
- HARVEST gap detection (INV-HARVEST-001, 006)
- SEED idempotence (INV-SEED-002–003)
- MERGE preservation (INV-MERGE-001)

**Configuration**:
```rust
// Kani harness template
#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    #[kani::unwind(8)]  // Bound: up to 8 datoms per transaction
    fn inv_store_001_append_only() {
        let store = kani::any::<Store>();
        let tx = kani::any::<Transaction<Committed>>();
        let pre_len = store.len();
        let result = store.transact(tx);
        if let Ok(new_store) = result {
            kani::assert(new_store.len() >= pre_len);
        }
    }
}
```

**Solver bounds**: `#[kani::unwind(8)]` for most harnesses. Increase to 16 for
CRDT commutativity/associativity proofs (three-store merge scenarios).

### Gate 4: Model Checking (`cargo test --features stateright`)

**Checks**: `V:MODEL` invariants — protocol safety and liveness over all reachable states.

**Time**: <30 minutes.

**Coverage**: 15 invariants (14.4%) for protocol-level properties:
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

The verification matrix maps each Stage 0 INV (62 total) to its verification methods.
This template is filled as implementation proceeds:

| INV | V:TYPE | V:PROP | V:KANI | V:MODEL | Status |
|-----|--------|--------|--------|---------|--------|
| INV-STORE-001 | ✓ (typestate) | ☐ | ☐ | — | ☐ |
| INV-STORE-002 | ✓ (newtype) | ☐ | ☐ | — | ☐ |
| INV-STORE-003 | ✓ (Hash/Eq) | ☐ | — | — | ☐ |
| INV-STORE-004 | — | ☐ | ☐ | ☐ | ☐ |
| ... | | | | | |

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

1. All 62 Stage 0 INVs verified (proptest minimum, Kani where tagged)
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
