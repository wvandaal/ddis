# D3 — Kani CI Feasibility

> **Thread**: R3.3a — Can Kani run in CI within reasonable time?
> **Date**: 2026-03-03
> **Sources**: spec/16-verification.md, Kani documentation, kani-verifier-blog

---

## Research Questions

1. Can Kani run in CI within reasonable time (< 15 minutes)?
2. How many invariants need Kani verification?
3. What unwind depths are needed for Braid's data structures?
4. What solver should we use?
5. How should we stage Kani verification in CI?

---

## Kani Overview

Kani is a bounded model checker for Rust that compiles Rust code to CBMC
(C Bounded Model Checker) intermediate representation, then uses SAT/SMT solvers
to exhaustively verify properties within bounded execution depth.

Key characteristics:
- Bit-precise: models exact Rust semantics including overflow, pointer arithmetic
- Bounded: loops must be unwound to a finite depth (`#[kani::unwind(N)]`)
- Solver-sensitive: verification time varies drastically by solver choice
- CI-friendly: official GitHub Action available (`model-checking/kani-github-action`)

---

## Braid's Kani Requirements (from spec/16-verification.md)

### 38 Invariants with V:KANI Tag

| Namespace | Count | Invariants |
|-----------|-------|------------|
| STORE | 10 | 001-008, 010, 012 |
| SCHEMA | 3 | 001, 002, 004 |
| QUERY | 5 | 001, 004, 012, 013, 017 |
| RESOLUTION | 5 | 002, 004, 005, 006, 007 |
| HARVEST | 2 | 001, 006 |
| SEED | 2 | 002, 003 |
| MERGE | 5 | 001, 003, 004, 005, 008 |
| SIGNAL | 3 | 001, 003, 005 |
| DELIBERATION | 2 | 002, 005 |
| GUIDANCE | 1 | 006 |
| BUDGET | 3 | 001, 003, 006 |

### Stage Distribution

| Stage | Kani INVs | Notes |
|-------|-----------|-------|
| 0 | 24 | STORE(10), SCHEMA(3), QUERY(3), RESOLUTION(5), HARVEST(1), SEED(2), MERGE(1) |
| 1 | 5 | HARVEST(1), BUDGET(3), MERGE(1 — via 008 already in Stage 0) |
| 2 | 6 | QUERY(1), MERGE(3), DELIBERATION(2), SIGNAL(1), GUIDANCE(1) |
| 3 | 3 | SIGNAL(2), MERGE(1) |

**Critical**: 24 of 38 Kani invariants are in Stage 0. This is the primary CI concern.

---

## Time Budget Analysis

### Benchmark Data (from Kani blog, s2n-quic verification)

Per-harness verification times vary enormously by solver:

| Harness type | MiniSat | CaDiCaL | Kissat |
|-------------|---------|---------|--------|
| Simple property (small state) | 2-10s | 1-5s | 1-5s |
| Medium (loops, collections) | 60-300s | 10-60s | 10-60s |
| Complex (nested loops, BTreeMap) | 1000s+ | 60-300s | 60-300s |
| Very complex (unbounded) | TIMEOUT | TIMEOUT | TIMEOUT |

Solver choice matters enormously: the s2n-quic benchmarks showed **200x speedups**
switching from MiniSat to CaDiCaL/Kissat for specific harnesses.

### Estimated Braid Harness Complexity

| Category | Example INVs | Estimated Time (CaDiCaL) | Count |
|----------|-------------|-------------------------|-------|
| Trivial (type-level) | STORE-001, STORE-003 | 2-5s | 5 |
| Simple (set operations) | STORE-004-006, MERGE-001 | 5-30s | 8 |
| Medium (hash/index) | STORE-002, 010, 012 | 30-120s | 7 |
| Complex (query eval) | QUERY-001, 012, 013 | 120-600s | 4 |
| **Total (24 Stage 0)** | | **~20-50 min** | **24** |

### CI Time Budget

The spec allocates (section 16.2):
- Gate 5 (Kani): **< 15 minutes**
- Gate 6 (stateright): **< 30 minutes**

**Assessment**: 15 minutes for 24 harnesses is **tight but feasible** if:
1. Solver is optimized per-harness (`#[kani::solver(kissat)]` vs `#[kani::solver(cadical)]`)
2. Unwind bounds are tight (not over-approximated)
3. Harnesses are parallelized (Kani supports `--jobs N`)
4. Complex harnesses are moved to nightly CI

---

## Unwind Depth Analysis

Braid's core data structures and their loop bounds:

| Structure | Operations | Estimated Unwind |
|-----------|-----------|-----------------|
| BTreeSet<Datom> (store) | insert, contains, union | 3-5 (bounded by test store size) |
| Vec<Datom> (transaction) | iterate, push | 3-5 |
| HashMap<Attribute, Resolution> | lookup, insert | 2-3 |
| HLC timestamp comparison | no loops | 0 |
| Semi-naive fixpoint | iterate until convergence | 5-10 (bounded by query depth) |
| Tarjan SCC (DFS) | recursive DFS | 5-10 (bounded by graph size) |
| PageRank iteration | power iteration | 10-20 (bounded by max_iterations) |

**Key insight**: All Kani harnesses must use **bounded test stores** (e.g., 3-5 datoms).
Kani cannot verify properties over unbounded collections. The proptest framework
covers the unbounded case; Kani covers the exhaustive bounded case.

Recommended default: `#[kani::unwind(8)]` for most harnesses, with per-harness
overrides for graph algorithms (`#[kani::unwind(12)]`).

---

## Solver Recommendations

Based on s2n-quic benchmark data and Braid's property types:

| Property Type | Recommended Solver | Reasoning |
|---------------|-------------------|-----------|
| Set algebra (L1-L5) | CaDiCaL | Good for equality reasoning |
| Content addressing (hashing) | Kissat | Better for bit-level operations |
| Graph algorithms | CaDiCaL | Good for structural properties |
| Resolution mode (enum matching) | MiniSat (fast) | Simple boolean constraints |

Configure per-harness:
```rust
#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(8)]
fn verify_store_commutativity() {
    let s1: Store = kani::any();
    let s2: Store = kani::any();
    assert_eq!(s1.merge(&s2), s2.merge(&s1));
}
```

---

## CI Pipeline Design

### Recommended Gate 5 Structure

```yaml
# Gate 5a: Fast Kani (every PR) — target < 5 min
- Trivial + simple harnesses (13 harnesses)
- Solver: cadical (default)
- Unwind: 5

# Gate 5b: Full Kani (nightly) — target < 30 min
- All 24 Stage 0 harnesses
- Per-harness solver selection
- Unwind: 8-12

# Gate 5c: Extended Kani (weekly) — target < 2 hours
- All harnesses with higher unwind bounds
- Regression testing for new harnesses
```

This three-tier approach keeps PR verification fast while still running
comprehensive verification on a schedule.

### GitHub Action Configuration

```yaml
- name: Kani Verification (Fast)
  uses: model-checking/kani-github-action@v1
  with:
    command: cargo kani
    args: >-
      --harness "verify_store_*"
      --harness "verify_schema_*"
      --output-format terse
      --jobs 4
```

---

## Feasibility Assessment

| Question | Answer |
|----------|--------|
| Can 24 harnesses run in 15 min? | YES, with tier split (fast tier in 5 min, full in 30 min nightly) |
| Blocking issue? | None. Kani is mature, has GitHub Action, CaDiCaL solver is fast. |
| Main risk | Query evaluation harnesses (INV-QUERY-012, 013, 017) may be slow due to graph algorithm complexity. Move these to nightly if > 2 min each. |
| Unwind depth | 8 default, 12 for graph algorithms. Must use bounded test stores (3-5 datoms). |
| Alternative | If Kani proves too slow for specific harnesses, those can be downgraded to proptest-only with a tracked exception. |

---

## Recommendations

1. **Start with CaDiCaL** as default solver; benchmark each harness and switch to
   Kissat where it is faster.
2. **Use three CI tiers**: fast (every PR), full (nightly), extended (weekly).
3. **Bound all test stores to 3-5 datoms** for Kani harnesses. Proptest covers larger sizes.
4. **Track verification time per harness** from the start. Budget each harness < 60s
   for the fast tier.
5. **The 15-minute gate is achievable** for Stage 0 if the fast tier contains only
   the trivial+simple harnesses (~13 of 24).
6. **Consider `#[kani::stub]`** for complex subsystems (e.g., stub the hash function
   with a simpler version for CRDT algebra proofs).
