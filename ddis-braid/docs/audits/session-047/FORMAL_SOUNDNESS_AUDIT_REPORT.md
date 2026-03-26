# Braid Full Audit Report

> **Date**: 2026-03-26
> **Auditor**: Claude Opus 4.6 (1M context)
> **Scope**: Soundness, Architecture, Coherence, Accretive Path
> **Store**: 108,523 datoms, 10,241 entities, 2,040 tests passing
> **Codebase**: ~175K LOC Rust (91.6K kernel + 24.8K CLI + tests)

---

## 1. Executive Summary

**Project health: STRUCTURALLY SOUND, OPERATIONALLY INCOMPLETE.**

The algebraic foundation is proven correct — CRDT properties (commutativity,
associativity, idempotency, monotonicity) are verified at three levels (Kani
bounded model checking, proptest property testing, Stateright exhaustive model
checking). The type system enforces critical invariants (transaction typestate,
append-only C1, `#![forbid(unsafe_code)]`). The specification layer is mature
(216 invariants across 20 namespaces, 80.6% implemented, 66.2% tested).

**The single most important finding**: The coherence gate — the system's primary
mechanism for preventing contradictions from entering the store — is not wired
into the main write path. Every CLI command (`observe`, `write assert`, `task
create`, `harvest`, `spec create`) bypasses coherence checking entirely. The
gate protects only proposal acceptance, a rarely-used path. This means 99%+ of
store mutations have no contradiction prevention.

**The single most important recommendation**: Wire `transact_with_coherence()`
into `LiveStore::write_tx()` before any other work. This is a ~20-line change
that closes the most critical soundness gap in the system. Without it, every
other formal guarantee is undermined because the store can accumulate
contradictions unchecked.

---

## 2. Soundness Findings

### 2.1 Formal Guarantee Verification Table

| Guarantee | Claim Source | Verification | Status | Risk |
|-----------|-------------|--------------|--------|------|
| Merge commutativity | INV-STORE-004, C4 | Kani + proptest + Stateright | **PROVEN** | LOW |
| Merge associativity | INV-STORE-005, C4 | Kani + proptest + Stateright | **PROVEN** | LOW |
| Merge idempotency | INV-STORE-006, C4 | Kani + proptest + Stateright | **PROVEN** | LOW |
| Append-only (C1) | INV-STORE-001 | Kani + Stateright + code audit (0 .remove() on datoms) | **PROVEN** | NONE |
| Content-addressed EntityId | INV-STORE-003, C2 | Kani (2 harnesses) + proptest | **PROVEN** | NONE |
| Transaction typestate | ADR-STORE-001 | Type system (sealed trait, consume-self, no Clone) | **TYPE-ENFORCED** | NONE |
| Genesis determinism | INV-STORE-008 | Kani + unit test | **PROVEN** | NONE |
| Schema semilattice | INV-SCHEMA-002 | Kani (merge superset) | **PROVEN** | LOW |
| Query CALM compliance | INV-QUERY-001 | Kani (bounded, 1 query pattern) | **PROVEN** (narrow) | LOW |
| Query determinism | INV-QUERY-002 | Kani (bounded, 1 query pattern) | **PROVEN** (narrow) | LOW |
| F(S) monotonicity | INV-BILATERAL-001 L1 | Code analysis | **FALSE** | CRITICAL |
| Coherence gate prevents contradictions | INV-COHERENCE-* | Code analysis | **UNWIRED** | CRITICAL |
| Fixpoint convergence (Datalog) | evaluator.rs header | Code analysis | **DOES NOT EXIST** | LOW* |
| Semi-naive evaluation | evaluator.rs header | Code analysis | **DOES NOT EXIST** | LOW* |
| Schema layer enforcement | INV-SCHEMA-006 | Code analysis | **CONVENTION ONLY** | LOW |
| NaN handling in Value::Double | (implicit) | Proptest explicitly filters NaN | **UNVERIFIED** | MEDIUM |
| Hypothesis calibration convergence | OBSERVER-4 | Empirical (error=0.521, degrading) | **DEFECTIVE** | HIGH |

*LOW because no recursive rules are supported, making claims vacuously true.
The risk escalates to HIGH if/when recursive rules are added without implementing fixpoint.

### 2.2 Critical Unsoundnesses

**CRITICAL-1: Coherence gate not wired into write path.**
The coherence gate (`coherence.rs`, 1,300 LOC, 18 tests) exists as well-tested
library code but is architecturally disconnected from the CLI write path.
`LiveStore::write_tx()` calls `store.apply_datoms()` directly, bypassing both
schema validation and coherence checking. The only production caller of
`transact_with_coherence()` is `proposal.rs:468`.

**CRITICAL-2: F(S) is not monotonic.**
The specification claims `F(S(t+1)) >= F(S(t))` (Law L1). This is false:
- Adding an unwitnessed spec element lowers V (validation) and C (coverage)
- Adding a low-confidence exploration lowers U (uncertainty complement)
- The hypothesis ledger measures ΔF(S) that empirically goes negative
- The Lyapunov analysis reports non-monotonicity without enforcing it

**HIGH-1: Hypothesis ledger dimensional mismatch.**
Predicted values are normalized R(t) importance scores in [0,1]. Actual values
are raw F(S) deltas (typically near 0 for individual task closes). Mean error
converges toward ~0.5 because the quantities being compared have different
dimensions. The "degrading trend" (error 0.521) is structural, not a
calibration failure — the acquisition function is comparing apples to oranges.

### 2.3 Verification Gaps Ranked by Risk

| Gap | Risk | Mitigation |
|-----|------|------------|
| Coherence gate bypass | CRITICAL | Wire into LiveStore::write_tx() |
| F(S) non-monotonicity | CRITICAL | Redesign as monotone-by-construction or document as non-monotone |
| Hypothesis dimensional mismatch | HIGH | Normalize both to same scale or change predicted to ΔF(S) estimates |
| Intra-transaction contradictions not detected | MEDIUM | Add within-tx duplicate (e,a) check in tier1 |
| NaN in Value::Double | MEDIUM | Add schema-level NaN rejection or proptest NaN coverage |
| Kani proofs use only Value::Long | MEDIUM | Extend harnesses to cover all 9 Value variants |
| Query AVET index never used | LOW | Wire into evaluator (performance, not correctness) |
| Wasserstein-1 greedy heuristic | MEDIUM | Document approximation or implement exact transport |
| Schema evolution (validate_evolution) dead code | LOW | Wire into transact path for schema-modifying transactions |

---

## 3. Architectural Findings

### 3.1 C8 Violations Inventory

**19 violations found** (10 HARD, 4 SOFT, 5 ACCEPTABLE).

| Priority | Violation | File | Severity |
|----------|-----------|------|----------|
| 1 | Schema L1+L2 (60 attrs) hardcoded as DDIS ontology | schema.rs:1134-1522 | HARD |
| 2 | INV/ADR/NEG prefixes hardcoded in 3 modules | spec_id.rs, task.rs, methodology.rs | HARD |
| 3 | MaterializedViews ISP namespace classification hardcoded | store.rs:642-764 | HARD |
| 4 | SpecCandidateType enum hardcodes DDIS types | harvest.rs:1628-1713 | HARD |
| 5 | Trace scanner depth classification is Rust-specific | trace.rs:120-213 | HARD |
| 6 | "cargo fmt, cargo clippy, cargo test" in kernel | topology.rs:797 | HARD |
| 7 | Boundary enum hardcodes IntentSpec/SpecImpl | bilateral.rs:101-108 | HARD |
| 8 | Crystallization candidates assume DDIS formalization attrs | methodology.rs:884-944 | HARD |
| 9 | Logical divergence detector assumes invariant entities | signal.rs:515-570 | SOFT |
| 10 | Cargo quality gate commands in seed output | seed.rs:2770 | SOFT |

**Highest-impact fix**: Move schema Layers 1-2 to the DDIS policy manifest (V1+V2).
This cascades through V3 (MaterializedViews classification) and V7 (dynamic
boundaries). The kernel's schema should contain only Layer 0 (meta-schema:
`:db/*`, `:tx/*`, `:lattice/*`). All domain-specific attributes enter through
the policy manifest at `braid init` time.

### 3.2 Module Health Assessment

**Circular dependencies (4 found):**

| Cycle | Severity | Fix |
|-------|----------|-----|
| store ↔ merge | HIGH | Extract shared types to `types.rs` |
| store ↔ bilateral (inline) | MEDIUM | Extract FitnessScore/Components to `fitness.rs` |
| routing ↔ context | MEDIUM | Merge into one module or extract shared types |
| proptest_strategies ↔ bilateral | LOW | Move arb_fitness_* into bilateral test module |

**God modules (> 4,000 LOC):**

| Module | LOC | Decomposition |
|--------|-----|---------------|
| guidance.rs | 7,119 | Extract telemetry/scoring subsystem |
| store.rs | 5,944 | Extract StatusSnapshot + FitnessGradient |
| concept.rs | 5,362 | Split extraction vs linking |
| seed.rs | 5,314 | Split assembly vs formatting |
| schema.rs | 4,957 | Move L1-L5 to policy (C8 fix eliminates ~2K lines) |
| bilateral.rs | 4,769 | Split F(S) components vs cycle orchestration |
| harvest.rs | 4,433 | Extract text analysis heuristics |
| query/graph.rs | 4,546 | Strong candidate for separate `braid-graph` crate |

**Fan-in hotspots**: `datom` (41 importers), `store` (31), `schema` (17).
Any API change to these modules cascades widely. Their interfaces should be frozen.

### 3.3 Performance Architecture Assessment

**~20 full O(N) datom scans in a single `braid status` call:**

| Location | Function | Could Use Index? |
|----------|----------|-----------------|
| bilateral.rs:833 | entities_matching_pattern (×7 boundaries) | Yes — namespace-prefix on attribute_index |
| bilateral.rs:1170 | compute_contradiction_complement | Yes — MaterializedViews |
| trilateral.rs:394 | compute_beta_1 | Partially — VAET index |
| task.rs:979 | all_tasks (called 2×) | Yes — attribute_datoms(":task/id") |
| methodology.rs:433,515,535,768 | 4 telemetry scans | Yes — attribute_index |

At 108K datoms, ~20 scans = ~2.16M datom iterations per status call.

**merge() rebuilds ALL indexes from scratch** even for 0 new datoms. An
incremental strategy inserting only the delta would be O(M log N) instead of
O((N+M) log(N+M)).

**transact() allocates O(E) HashSet on every call** (line 1334) for
pre_existing entity tracking. With 10K entities, this is ~320KB per transaction.

### 3.4 Learning Loop Closure Status

| Loop | Observation | Analysis | Feedback | Status |
|------|------------|----------|----------|--------|
| **1. Calibration** (OBSERVER-4) | LIVE: hypotheses at harvest | LIVE: outcomes at task close | COMPUTED BUT NOT TRANSACTED | **PARTIALLY CLOSED** |
| **2. Structure** (OBSERVER-5) | NOT IMPLEMENTED | NOT IMPLEMENTED | NOT IMPLEMENTED | **OPEN** |
| **3. Ontology** (OBSERVER-6) | LIVE: embed + cluster at observe/harvest | LIVE: agglomerative crystallization | SHALLOW: routing dampening only | **PARTIALLY CLOSED** |

**The last-mile problem**: Both Loop 1 and Loop 3 compute feedback signals but
never transact them back. `apply_weight_adjustments()` generates datoms but is
never called from the CLI. Concepts affect routing priority but never evolve
into boundaries or schema modifications.

---

## 4. Coherence Matrix

### 4.1 Spec → Implementation Coverage

```
| Namespace     | INVs | Impl | Kani | Prop | Tested | Untested | Coverage |
|---------------|------|------|------|------|--------|----------|----------|
| STORE         |   16 |   16 |    6 |   14 |     16 |        0 |    100%  |
| LAYOUT        |   11 |   11 |    3 |    5 |      7 |        4 |     64%  |
| SCHEMA        |    9 |    9 |    3 |    4 |      8 |        1 |     89%  |
| QUERY         |   24 |   24 |    2 |   10 |     20 |        4 |     83%  |
| RESOLUTION    |    8 |    8 |    4 |    3 |      8 |        0 |    100%  |
| HARVEST       |    9 |    9 |    2 |    6 |      7 |        2 |     78%  |
| SEED          |    8 |    8 |    2 |    4 |      7 |        1 |     88%  |
| MERGE         |   10 |   10 |    2 |    4 |      7 |        3 |     70%  |
| SYNC          |    5 |    2 |    0 |    0 |      0 |        5 |      0%  |
| SIGNAL        |    6 |    6 |    0 |    1 |      1 |        5 |     17%  |
| BILATERAL     |    5 |    5 |    0 |    5 |      5 |        0 |    100%  |
| DELIBERATION  |    6 |    6 |    0 |    0 |      4 |        2 |     67%  |
| GUIDANCE      |   24 |   20 |    1 |    3 |     17 |        7 |     71%  |
| BUDGET        |    9 |    9 |    2 |    1 |      8 |        1 |     89%  |
| INTERFACE     |   10 |    1 |    0 |    0 |      2 |        8 |     20%  |
| TRILATERAL    |   10 |   10 |    0 |    6 |     10 |        0 |    100%  |
| TOPOLOGY      |   16 |    5 |    0 |    0 |      6 |       10 |     38%  |
| COHERENCE     |   13 |    1 |    0 |    0 |      1 |       12 |      8%  |
| WITNESS       |   12 |   11 |    0 |    4 |      8 |        4 |     67%  |
| REFLEXIVE     |    5 |    3 |    0 |    0 |      1 |        4 |     20%  |
|---------------|------|------|------|------|--------|----------|----------|
| **TOTAL**     |  216 |  174 |   27 |   70 |    143 |       73 |   66.2%  |
```

### 4.2 Reverse Traceability (Implementation → Spec)

**42 invariants exist in code but not in spec/**:
- INV-TASK-001..006 (task management — 70 tests, no formal spec)
- INV-FOUNDATION-006..014 (bedrock principles — CLAUDE.md only, not in spec/)
- INV-QUERY-025..034 (extended graph algorithms — 90 tests, no spec)
- INV-EMBEDDING-001..004, INV-PROMOTE-001..004, INV-HARVEST-010..012

**30 functions > 50 lines lack any spec reference**, including:
- `build_orientation()` (515 lines, seed.rs) — largest untraced function
- `compute_routing_from_store_inner()` (260 lines, routing.rs)
- `synthesize_narrative()` (256 lines, harvest.rs)

### 4.3 Top 10 Coherence Gaps by Criticality

| Rank | Gap | Impact | Effort |
|------|-----|--------|--------|
| 1 | Coherence gate unwired from write path | Contradictions enter store unchecked | ~20 lines |
| 2 | F(S) monotonicity claim is false | Convergence guarantees are invalid | Design change |
| 3 | Hypothesis dimensional mismatch | Calibration loop feeds back noise | Design change |
| 4 | SYNC namespace: 0% tested (0/5) | Multi-agent coordination unverified | 5 test suites |
| 5 | COHERENCE namespace: 8% tested (1/13) | Density matrix theory unverified | 12 test suites |
| 6 | 42 reverse-gap invariants not in spec | Implementation drifts from spec | 42 spec elements |
| 7 | INV-TASK-* not formally specified | Core workflow lacks formal guarantees | 6 spec elements |
| 8 | Schema L1-L2 hardcoded (C8 violation) | Kernel not substrate-independent | Architecture change |
| 9 | 3 F(S) computation paths disagree | Measurement fragility | Unify or eliminate |
| 10 | Loop 2 (Structure Discovery) entirely open | System cannot discover hidden boundaries | New subsystem |

---

## 5. Accretive Action Plan

### Risk-Adjusted Prioritization

```
accretive_value = (convergence_impact × loop_closure_factor) / (effort × risk)
```

| Rank | Action | Conv. Impact | Loop? | Effort | Risk | Score | Sessions |
|------|--------|-------------|-------|--------|------|-------|----------|
| **1** | Wire coherence gate into write path | 0.9 | No | 0.1 | 0.1 | **90.0** | 0.5 |
| **2** | Fix hypothesis dimensional mismatch | 0.7 | 2× (Loop 1) | 0.3 | 0.2 | **23.3** | 1 |
| **3** | Close Loop 1: call apply_weight_adjustments | 0.5 | 2× (Loop 1) | 0.2 | 0.1 | **50.0** | 0.5 |
| **4** | Eliminate full-scan bottlenecks in braid status | 0.4 | No | 0.4 | 0.1 | **10.0** | 2 |
| **5** | Redesign F(S) as monotone-by-construction | 0.8 | 2× (Loop 1) | 0.8 | 0.4 | **5.0** | 3 |

**Recommended execution order**: 1 → 3 → 2 → 4 → 5.

Action 1 is a ~20-line change with 90× return on effort. Action 3 is similarly
trivial (call an existing function from the CLI). Together, 1+3 in a single
session would close the biggest soundness gap and the biggest learning loop gap.

Action 2 requires redesigning how predictions are recorded (change from R(t)
importance to estimated ΔF(S)), but the analysis code already exists. Action 4
is mechanical (replace full scans with index lookups). Action 5 is the deepest
change — rethinking F(S) to be monotone — and may require accepting that F(S)
is informational rather than a Lyapunov function.

---

## 6. Open Loops Inventory

| Subsystem | Produces | Feeds Back Into | Status |
|-----------|----------|----------------|--------|
| Hypothesis ledger | Predictions + outcomes | Nothing (display only) | **OPEN** |
| Weight calibration | Adjustment recommendations | Nothing (not transacted) | **OPEN** |
| Concept crystallization | Concept entities | Routing dampening only | **PARTIALLY OPEN** |
| Temporal coupling | Nothing | Nothing | **FULLY OPEN** |
| Signal system | Signal datoms | Nothing (no subscription dispatch) | **OPEN** |
| Coherence gate | Violation reports | Nothing (not called) | **OPEN** |
| MaterializedViews | Incremental accumulators | F(S) fallback path only | **PARTIALLY OPEN** |
| Witness system | FBW entities | Bilateral depth scoring | **CLOSED** |
| Bilateral cycle | Convergence analysis | braid status display | **PARTIALLY OPEN** |
| Trace scanner | Link entities + depth | F(S) coverage component | **CLOSED** |

**Recommended closure mechanisms:**
1. **Hypothesis → calibration**: Call `apply_weight_adjustments()` from `braid harvest --commit`
2. **Coherence gate → write path**: Wire into `LiveStore::write_tx()`
3. **Concepts → ontology**: When concept variance < threshold for N sessions, propose new boundary
4. **Signals → resolution**: Implement signal subscription dispatch (SIGNAL-002)
5. **MaterializedViews → F(S)**: Use views accumulators as primary path, eliminate full scans

---

## 7. Formal Verification Roadmap

### Current Coverage

| Tier | Count | What |
|------|-------|------|
| **Kani proofs** | 22+ harnesses | CRDT algebra, content identity, schema genesis, query CALM/determinism |
| **Stateright models** | 3 models | Multi-agent merge (2-3 agents), algebraic properties, frontier monotonicity |
| **Proptest properties** | 70 INVs covered | Store merge, schema validation, bilateral, trilateral, harvest, witness |
| **Unit tests** | 2,040 total | All passing, covering 143/216 specified invariants |

### Recommended Additions (Ranked by Risk × Impact)

| Priority | Addition | Type | Covers | Impact |
|----------|----------|------|--------|--------|
| 1 | Coherence gate integration test | Unit | Verify write path calls coherence | CRITICAL |
| 2 | Kani: extend all harnesses to 9 Value types | Kani | CRDT + identity across all types | HIGH |
| 3 | Proptest: NaN/Inf in Value::Double | Proptest | Serialization determinism, index behavior | MEDIUM |
| 4 | Kani: schema retraction safety | Kani | Meta-schema attribute retraction has no effect | MEDIUM |
| 5 | Proptest: intra-tx contradiction detection | Proptest | Two assertions for same (e,a) in one tx | MEDIUM |
| 6 | Stateright: concurrent transact on same store | Stateright | Multi-thread safety (currently &mut self) | LOW |
| 7 | Proptest: F(S) direction under valid operations | Proptest | Document non-monotonicity empirically | HIGH |
| 8 | Kani: stratum boundary enforcement | Kani | S0/S1 queries cannot trigger non-monotonic eval | LOW |
| 9 | E2E: hypothesis prediction accuracy | Integration | Record + close + verify error direction | HIGH |
| 10 | Proptest: query index vs full-scan equivalence | Proptest | Index selection produces identical results | LOW |

### Specific Harness Designs

**Priority 1 — Coherence integration test:**
```rust
#[test]
fn write_path_rejects_contradiction() {
    let store = test_store_with_entity_attr_value();
    let tx = build_contradicting_tx(); // same (e,a), different v
    let result = live_store.write_tx(&tx);
    assert!(result.is_err()); // CURRENTLY WOULD PASS (no check)
}
```

**Priority 2 — Kani Value-type coverage:**
```rust
#[kani::proof]
#[kani::unwind(3)]
fn prove_merge_commutativity_all_value_types() {
    let vtype: u8 = kani::any();
    kani::assume(vtype < 9);
    let val = symbolic_value(vtype); // dispatch to each variant
    // ... existing commutativity proof with polymorphic values
}
```

---

## 8. Risks and Concerns

### Architectural Risks (Major Rework Potential)

1. **C8 migration scope**. Moving schema L1-L2 to policy manifests cascades
   through MaterializedViews, bilateral boundaries, methodology scoring,
   harvest candidate classification, and signal detection. This is ~15 modules
   and could require 5-10 sessions of careful surgery. The risk is not that it
   can't be done, but that partial migration leaves the system in an
   inconsistent state where some code reads from policy and some from hardcoded
   schema.

2. **F(S) redesign**. If F(S) is redesigned as monotone-by-construction
   (e.g., tracking "best achieved" rather than "current state"), all existing
   F(S) values in the store become stale. The hypothesis ledger's error history
   becomes meaningless. The bilateral convergence analysis needs rebaselining.
   Consider whether F(S) should remain a snapshot metric (accept non-monotonicity,
   document it) or become a high-water mark (monotone but potentially misleading).

3. **Query engine evolution**. The evaluator header claims fixpoint/semi-naive
   but implements single-pass conjunctive queries. If recursive rules are ever
   needed (e.g., transitive closure for dependency chains), the entire evaluator
   architecture must change. The risk is that the current API contract implies
   capabilities that don't exist, leading to surprise breakage when expectations
   meet reality.

### Convergence Risks (Learning Loop Failure)

1. **Calibration noise**. The hypothesis ledger has been accumulating ~0.5 error
   entries for months. If/when the dimensional mismatch is fixed, all historical
   data becomes invalid. The system needs a "calibration epoch" mechanism to
   discard pre-fix hypotheses without violating C1 (append-only).

2. **Concept collapse persistence**. DOGFOOD-2 scored 3.48/10. The hash embedder
   produces deterministic but semantically poor embeddings. Until real embeddings
   (Model2Vec) are enabled by default, the ontology loop produces low-quality
   concepts that pollute the routing signal. This is a **fixable implementation
   issue** (the `embeddings` feature flag exists, Model2Vec is implemented), not
   an architectural problem.

3. **Loop 2 (Structure) is entirely absent**. The system cannot discover hidden
   boundaries from usage patterns. This means the boundary set is static —
   defined at init time and never refined by experience. For the self-calibrating
   vision to work, boundary discovery must be implemented. This is the deepest
   missing piece.

### Timeline Risks

1. **Stage 0 completion criteria** require "work 25 turns, harvest, start fresh
   with seed — new session picks up without manual re-explanation." The coherence
   gate gap and F(S) non-monotonicity are not blockers for this criterion, but
   the hypothesis degradation might create confusing guidance that undermines
   the seed quality.

2. **The 73 untested invariants** across SYNC (5), COHERENCE (12), INTERFACE (8),
   TOPOLOGY (10), SIGNAL (5), and others represent a verification debt that grows
   as the store evolves. Each untested invariant is a potential soundness gap
   that compounds over time.

3. **Performance at scale**. At 108K datoms, `braid status` runs in ~4s with ~20
   full scans. At 1M datoms (projected within 20 sessions at current growth),
   it would take ~40s without the scan optimizations recommended in Section 5,
   Action 4. The merge() full-index-rebuild is an even steeper cliff.

---

## Appendix A: Methodology

This audit followed the 5-phase structure defined in `docs/audits/FULL_AUDIT_PROMPT.md`:
- Phase 1: 6 parallel soundness investigations (CRDT, typestate, coherence gate, query, F(S)+schema, C8)
- Phase 2: 2 parallel architecture investigations (module coupling + type algebra, performance + learning loops)
- Phase 3: 1 comprehensive coverage matrix investigation
- Phase 4: Synthesis from findings (not delegated)
- Phase 5: This report (not delegated)

Total: 9 investigation agents, each reading 5-15 source files, producing 3,000-5,000 word findings.
Every claim traces to a file:line reference provided by the investigation agents.

## Appendix B: File Reference Index

| File | Key Findings |
|------|-------------|
| store.rs:1324 | transact() — O(k log N), O(E) HashSet alloc per tx |
| store.rs:1452 | merge() — O(N+M) full index rebuild |
| store.rs:642-764 | MaterializedViews ISP hardcoding (C8 V3) |
| coherence.rs:476 | transact_with_coherence() — exists but unwired |
| bilateral.rs:11-17 | F(S) monotonicity claim — FALSE |
| bilateral.rs:80-92 | Hardcoded F(S) weights |
| routing.rs:1198 | Hypothesis prediction recording — dimensional mismatch |
| task.rs:1176 | Hypothesis outcome measurement — raw ΔF(S) |
| policy.rs:394 | calibrate_boundary_weights() — never called from CLI |
| policy.rs:554 | apply_weight_adjustments() — generates datoms, never transacted |
| schema.rs:1134-1522 | L1+L2 hardcoded DDIS ontology (C8 V1+V2) |
| spec_id.rs:53 | INV/ADR/NEG prefixes hardcoded (C8 V4) |
| evaluator.rs:74-141 | Single-pass evaluator, NOT fixpoint |
| kani_proofs.rs:141-288 | CRDT proofs (commutativity, associativity, idempotency) |
| concept.rs:285 | crystallize_concepts() — O(n^2) agglomerative clustering |
| query/graph.rs:769-797 | Wasserstein-1 greedy heuristic (not optimal transport) |
| live_store.rs:183 | write_tx() — bypasses coherence entirely |
