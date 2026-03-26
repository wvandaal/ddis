# Braid Full Formal Audit Report — Session 047

> **Date**: 2026-03-26
> **Auditor**: Claude Opus 4.6 (1M context)
> **Scope**: Full codebase (~117K LOC), 22 spec files, 16 guide files, 2 crates
> **Method**: Fagan-class inspection with 5 parallel domain-specific audit agents + orchestrator synthesis
> **Store state at audit time**: 123,365 datoms, 10,629 entities, 11,264 transactions

---

## Executive Summary

Braid is an ambitious and architecturally sound project at its algebraic core. The datom
store, CRDT merge, content-addressable identity, and Datalog query engine form a genuine
G-Set CvRDT with formal verification (Kani + Stateright + proptest). The Transaction
typestate pattern is textbook Curry-Howard. The kernel enforces `#![forbid(unsafe_code)]`.
The crate boundary is clean — the kernel has zero IO dependencies.

**However, the project has a single dominant architectural defect that overshadows everything
else: the kernel violates its own C8 constraint (substrate independence) systematically.**
The DDIS methodology — its ontology (INV/ADR/NEG), its coherence model (Intent/Spec/Impl
trilateral), its entity categories, its F(S) weights, and its 100+ domain-specific schema
attributes — is hardcoded throughout 30 of 38 kernel source files. The project's stated
goal is that the kernel be a universal substrate for any epistemological policy. In its
current form, it is a DDIS-specific runtime that cannot serve any other domain without
major refactoring.

The second most critical issue is the F(S) monotonicity claim: the fitness function is
observably non-monotonic (any transaction adding unlinked entities can decrease F(S)),
making the "Lyapunov function" claim mathematically false. This was previously discovered
(Session 042) but not yet resolved.

The third critical area is performance architecture: the store cache is broken (hash
mismatch prevents regeneration), forcing every CLI command to parse 11,264 EDN files
from disk. Despite this, `braid status` completes in ~0.08s thanks to the release
binary's optimized code paths — but the O(N) full-datom-scan pattern appears 82 times
across 18 files and will become the dominant bottleneck as the store scales past 500K datoms.

**Quantitative summary:**

| Metric | Value |
|--------|-------|
| Total findings | 72 |
| P0-CRITICAL | 5 |
| P1-HIGH | 17 |
| P2-MEDIUM | 30 |
| P3-LOW | 20 |
| Invariants audited | 141 (across all namespaces) |
| Invariants fully implemented + tested | ~38% |
| Invariants partially implemented | ~26% |
| Invariants not yet implemented (future stage) | ~30% |
| C8-compliant kernel files | 18/38 (47%) |
| C8-non-compliant kernel files | 9/38 (24%) |
| C8-partially-compliant kernel files | 11/38 (29%) |
| Public types audited | 247 |
| unsafe blocks | 0 (forbid) |
| unwrap() in production code | 19 |
| Test functions in codebase | ~2,083 |
| Tests passing | 2,043 / 2,043 (0 failures, 1 ignored) |
| Compilation status | PASS (1 dead_code warning) |

---

## Phase 1: Checkpoint Answers

### 1. What are the three learning loops and where is each implemented?

- **Weight calibration (OBSERVER-4)**: Predicted vs actual outcomes adjust boundary weights.
  Implemented in `routing.rs` (hypothesis ledger, calibration pipeline) and `bilateral.rs`
  (`compute_fitness_from_policy` reads `PolicyConfig` weights). The hypothesis ledger records
  predictions at harvest time and matches against actual ΔF(S) on task close. **Status: LIVE
  but boundary weight adjustment (HL-CALIBRATE) is partial — calibration metrics are computed
  but weights don't auto-update in PolicyConfig datoms.**

- **Structure discovery (OBSERVER-5)**: Temporal coupling reveals hidden boundaries.
  Implemented in `concept.rs` (concept crystallization from observation clustering) and
  `topology.rs` (spectral coupling analysis). **Status: Concept crystallization is LIVE
  (156 observations clustered). Topology spectral analysis exists but doesn't feed back
  into boundary discovery.**

- **Ontology discovery (OBSERVER-6)**: Observation clustering reveals emergent knowledge categories.
  Implemented in `concept.rs` (sigmoid-gated crystallization) and `promote.rs` (promotion
  from observation to spec element). **Status: Crystallization LIVE, promotion LIVE, but
  no closed loop — promoted concepts don't automatically create new boundary definitions.**

**Assessment**: All three loops have implementations, but none fully close. Each produces
data that doesn't feed back into the coherence model's boundary definitions. This is the
"open loop" pattern (ADR-FOUNDATION-014) at the architectural level.

### 2. What is the observation-projection duality and how does the code realize it?

Observation (external reality → datoms) and projection (datoms → external artifacts like
code, tests, docs) are adjoint functors. The code realizes this through:
- **Observation**: `observe.rs` command, harvest pipeline (`harvest.rs`), extractor framework
  (`schema.rs` Layer 6 meta-extractor attributes)
- **Projection**: `seed.rs` (store → AGENTS.md), `inject.rs` (store → dynamic AGENTS.md sections),
  `compiler.rs` (store → generated test code), `agent_md.rs` (store → agent instructions)

The round-trip property (observe then project should approximate identity) is NOT tested.
No code verifies that `parse(render(spec_data)) ≈ spec_data` (the fixed-point condition
from ADR-FOUNDATION-006).

### 3. What is C8 and which kernel modules violate it?

C8 states the kernel must not contain logic specific to any methodology, including DDIS.
**9 of 38 kernel files are non-compliant:**

| File | LOC | Violation |
|------|-----|-----------|
| trilateral.rs | 2,074 | Hardcoded INTENT_ATTRS/SPEC_ATTRS/IMPL_ATTRS const arrays |
| schema.rs | 4,957 | Layers 1-4 hardcode 100+ DDIS-specific attributes |
| bilateral.rs | 4,769 | Hardcoded F(S) component weights |
| harvest.rs | 4,433 | Hardcoded entity category expectations |
| compiler.rs | 3,072 | Reads :spec/* attributes by name |
| spec_id.rs | 210 | Hardcodes INV/ADR/NEG element types only |
| bootstrap_hypotheses.rs | 564 | Filesystem I/O in kernel + DDIS-specific |
| store.rs (MaterializedViews) | 5,848 | observe_datom classifies by :spec/:task/:impl/:harvest namespaces |
| guidance.rs | 7,119 | SystemTime::now() + DDIS-flavored methodology |

Total: ~33,046 LOC (40% of kernel) fails the C8 test.

### 4. Which reconciliation types have working implementations?

| Divergence Type | Working? | Evidence |
|----------------|----------|----------|
| Epistemic (store vs agent) | YES | harvest.rs pipeline + gap detection |
| Structural (impl vs spec) | YES | bilateral.rs forward/backward scan |
| Consequential (current vs risk) | PARTIAL | guidance.rs M(t) + routing.rs R(t), but no uncertainty tensor |
| Aleatory (agent vs agent) | PARTIAL | merge.rs set union works; conflict detection checks 3/6 conditions |
| Logical (INV vs INV) | PARTIAL | compiler.rs pattern detection; no 5-tier contradiction engine |
| Axiological (impl vs goals) | PARTIAL | F(S) fitness function, but no GoalDrift signal |
| Temporal (frontier vs frontier) | NO | No sync barrier implementation |
| Procedural (behavior vs methodology) | PARTIAL | M(t) scoring, but no drift detection feedback loop |

### 5. What is the current F(S) score and what does each component measure?

F(S) = 0.62 (from `braid status` output). Seven components:

| Component | Weight | What it measures |
|-----------|--------|-----------------|
| V: Validation | 0.18 | Depth-weighted witness verification of spec elements |
| C: Coverage | 0.18 | Depth-weighted implementation coverage of spec elements |
| D: Drift | 0.18 | Complement of normalized divergence Φ (ISP gaps) |
| H: Harvest | 0.13 | Methodology adherence score M(t) |
| K: Contradiction | 0.13 | Complement of intra-transaction conflict ratio |
| I: Incompleteness | 0.08 | Complement of spec elements lacking falsification conditions |
| U: Uncertainty | 0.12 | Mean confidence across exploration entities |

**Critical note**: These weights are hardcoded constants in `bilateral.rs:80-92` — a C8 violation.
The `PolicyConfig` has `BoundaryDef` with weight fields, but `compute_fitness()` ignores them and
uses the hardcoded values.

### 6. Where are the open loops?

| Data Flow | Producer | Where It Terminates | What Should Consume It |
|-----------|----------|-------------------|----------------------|
| Signal datoms (:signal/*) | signal.rs | Stored but never read by any boundary | F(S) or M(t) should include signal coverage |
| Exploration confidence | concept.rs | Set once at creation, never updated | Calibration should update from outcomes |
| Bootstrap hypotheses | bootstrap_hypotheses.rs | Module never called from CLI | Should feed into hypothesis ledger |
| Proposal lifecycle | proposal.rs | Stored as datoms | No F(S) credit for accepted proposals |
| Individual divergence detections | signal.rs detect_* functions | Returned to caller, printed to stderr | Should be stored as datoms |
| Concept bridge gaps | concept.rs | Displayed in status | No guidance routing for bridge-gap closure |
| Witness challenge verdicts | witness.rs | Stored, updates depth | No feedback to challenge parameters (difficulty, frequency) |

---

## Phase 2: Spec-Implementation Fidelity Matrix

### Invariant Coverage Summary by Namespace

| Namespace | Total INVs | Fully Impl+Tested | Partial | Not Impl | Type-Encoded |
|-----------|-----------|-------------------|---------|----------|-------------|
| STORE (§1) | 16 | 8 (50%) | 5 (31%) | 3 (19%) | 2 (EntityId, Transaction typestate) |
| SCHEMA (§2) | 9 | 3 (33%) | 4 (44%) | 2 (22%) | 0 |
| QUERY (§3) | 24 | 13 (54%) | 4 (17%) | 7 (29%) | 1 (Stratum enum) |
| RESOLUTION (§4) | 8 | 3 (38%) | 3 (38%) | 2 (25%) | 1 (ResolutionMode enum) |
| HARVEST (§5) | 9 | 3 (33%) | 2 (22%) | 4 (44%) | 0 |
| SEED (§6) | 8 | 2 (25%) | 4 (50%) | 2 (25%) | 0 |
| MERGE (§7) | 10 | 4 (40%) | 1 (10%) | 5 (50%) | 0 |
| GUIDANCE (§12) | 7 | 2 (29%) | 2 (29%) | 3 (43%) | 0 |
| BILATERAL (§10) | 5 | 2 (40%) | 1 (20%) | 2 (40%) | 1 (CoherenceConditions) |
| TRILATERAL (§18) | 10 | 4 (40%) | 2 (20%) | 4 (40%) | 2 (AttrNamespace, LiveView) |
| WITNESS (§21) | 12 | 7 (58%) | 2 (17%) | 3 (25%) | 2 (WitnessVerdict, StaleReason) |
| **TOTAL** | **118** | **51 (43%)** | **30 (25%)** | **37 (31%)** | **9** |

### Constraint Compliance (C1–C8)

| Constraint | Status | Violations | Evidence |
|-----------|--------|------------|----------|
| C1: Append-only | **PASS** | 0 | BTreeSet only grows via insert(). No remove() exists. |
| C2: Identity by content | **PASS (with caveat)** | 1 | EntityId::ZERO sentinel is not content-derived (datom.rs:46). from_raw_bytes is pub(crate). |
| C3: Schema-as-data | **PASS** | 0 | Schema derived from datoms via from_datoms(). No external DDL. |
| C4: CRDT merge | **PASS** | 0 | BTreeSet union. Commutative, associative, idempotent by proptest. |
| C5: Traceability | **PARTIAL** | 2 | Some implementation behaviors lack spec traceability. Genesis attr count drift (spec says 19, impl has 57). |
| C6: Falsifiability | **PARTIAL** | — | 265 current-stage INVs lack L2+ witness verification. |
| C7: Self-bootstrap | **PASS** | 0 | Spec elements stored as datoms. System checks own specification. |
| C8: Substrate independence | **FAIL** | 1,192 | 30/38 kernel files contain methodology-specific logic. See Phase 1 Q3. |

---

## Phase 3: Performance Assessment

### Runtime Measurements

| Command | Time | Notes |
|---------|------|-------|
| `braid status -q` | 0.08s | Excellent. Was 97s → 3s → now 80ms. |
| `cargo build --release` | 4.3s (incremental) | Acceptable |
| `cargo check --all-targets` | 1m 57s (clean) | Expected for 117K LOC |

### Top 10 Performance Findings

| ID | Title | Severity | Complexity | Impact |
|----|-------|----------|-----------|--------|
| PERF-001 | Store cache broken (hash mismatch) | P0 | O(F * parse) where F=11,264 files | Every CLI command pays full load cost |
| PERF-002 | all_tasks() called 4x per status | P1 | O(4N) redundant scans | 4x wasted work on every status |
| PERF-005 | entities_matching_pattern() full scan | P1 | O(N * B) per fitness computation | 756K datom iterations per status call |
| PERF-008 | Datom cloned 5-7x into indexes | P1 | ~100-250 MB RAM for 108K datoms | 4-5x memory bloat |
| PERF-003 | live_projections() redundant with MaterializedViews | P2 | O(N) per call, 3 calls in bilateral | Redundant with O(1) data in views |
| PERF-004 | compute_beta_1() full scan + eigendecomposition | P2 | O(N) + O(E³) | On every coherence check |
| PERF-006 | concept_observation_coverage() full scan | P2 | O(N) instead of O(K) via index | Easy fix: use attribute_index |
| PERF-009 | Schema rebuild on merge unconditional | P2 | O(N) even when no schema datoms | Merge always rebuilds schema |
| PERF-007 | CC-2 existence check full scan | P3 | O(N) for single attribute | .any() on all datoms |
| PERF-010 | Frontier::at() full scan | P3 | O(N) instead of O(log N) | Time-travel queries |

### Scaling Thresholds

| Datom Count | Expected Status Time | Bottleneck |
|-------------|---------------------|-----------|
| 123K (current) | ~80ms | Acceptable |
| 500K | ~250ms-1s | O(N*B) fitness computation dominates |
| 1M | ~1-5s | 82 full-datom-scan call sites become visible |
| 5M | >10s without cache | Store load becomes the wall |

### Memory Estimate (108K datoms)

| Component | Estimated Size |
|-----------|---------------|
| Primary BTreeSet<Datom> | 20-50 MB |
| entity_index (cloned datoms) | 20-50 MB |
| attribute_index (cloned datoms) | 20-50 MB |
| avet_index (cloned datoms + keys) | 20-50 MB |
| Other indexes + views | 10-30 MB |
| **Total** | **~100-250 MB** |

With index-by-offset: **~25-55 MB** (4-5x reduction).

---

## Phase 4: Formal Audit — Fagan/IEEE Synthesis

### 4.1 Fagan Inspection: Subsystem Overview

| Subsystem | Understanding | Soundness | Assessment |
|-----------|--------------|-----------|------------|
| Datom types | Complete | SOUND | Excellent type design. EntityId, Attribute, Value well-constrained. |
| Store | Complete | SOUND (with caveats) | BTreeSet CvRDT correct. MaterializedViews ignores retractions. LIVE view hardcodes LWW. |
| Schema | Complete | UNSOUND-RECOVERABLE | Genesis count drifted. validate_evolution() not wired into transact. Layers 1-4 should be policy. |
| Query engine | Complete | SOUND | Naive (not semi-naive) evaluation. Stratification correct. CALM classification works. |
| Resolution | Near-complete | UNSOUND-RECOVERABLE | Conflict predicate checks 3/6 conditions. Lattice falls back to LWW. No causally_independent(). |
| Harvest | Complete | SOUND (with gaps) | Pipeline works. Missing: crystallization guard, FP/FN calibration. |
| Seed | Complete | SOUND | Budget compliance verified by proptest. Missing: demonstration density. |
| Merge | Complete | SOUND | Set union correct. Cascade steps 2-5 are documented stubs. |
| Guidance | Near-complete | UNSOUND-RECOVERABLE | Open-loop: injects footers but no drift detection feedback. SystemTime in kernel. |
| Bilateral | Complete | UNSOUND-RECOVERABLE | F(S) monotonicity falsified. CC-3 permanently defaults true. No residual documentation. |
| Trilateral | Complete | UNSOUND-RECOVERABLE | Phi uses entity overlap not links. Hardcoded ISP ontology. 6 compilation errors (now fixed). |
| Witness | Complete | SOUND | Triple-hash staleness, challenge pipeline, auto-task on refutation all work. |
| Topology | Partial | SOUND | Spectral partition + CALM classification work. Coupling analysis correct. |
| Signal | Partial | UNTESTED | 1,465 LOC of exported API. Only detect_all_divergence called externally. Open loop. |
| Concept | Partial | SOUND | Sigmoid-gated crystallization works. 156 observations clustered. |
| Policy | Complete | SOUND (but unwired) | Well-designed PolicyConfig + BoundaryDef. Ignored by the rest of the kernel. |

### 4.2 Critical Correctness Defects

```
F-CORR-001: No causally_independent() implementation
  Severity: P1-HIGH | Category: SOUNDNESS
  Evidence: Zero matches for "causally_independent|causal_ancestor" in entire kernel.
    resolution.rs:225-238 has_conflict() does not check causal independence.
  Impact: Conflict predicate is unsound. Sequential single-agent updates incorrectly
    flagged as conflicts. Multi-agent causal chains will be misclassified.
  Traces to: INV-STORE-010, INV-RESOLUTION-004 conditions 5-6
  Remediation: Implement is_causal_ancestor(store, tx1, tx2) via BFS over
    :tx/causal-predecessors. Wire into has_conflict() as condition 6.
  Confidence: 1.0
```

```
F-CORR-002: LIVE view hardcodes LWW for ALL attributes
  Severity: P1-HIGH | Category: CORRECTNESS
  Evidence: store.rs:1089-1100 index_datom() applies LWW regardless of schema-declared
    resolution mode. Multi-cardinality attributes silently lose all but latest value.
  Impact: store.live_value() returns incorrect results for non-LWW attributes.
    INV-STORE-012 violated.
  Traces to: INV-STORE-012, INV-RESOLUTION-001
  Remediation: Route LIVE view updates through schema.resolution_mode(attr).
  Confidence: 1.0
```

```
F-CORR-003: F(S) monotonicity claim is mathematically false
  Severity: P0-CRITICAL | Category: SOUNDNESS
  Evidence: bilateral.rs:1875 analyze_convergence() REPORTS trajectory but does not
    ENFORCE monotonicity. compute_fitness() recomputes from scratch — adding unlinked
    entities decreases F(S). Memory confirms: "F(S) monotonicity claim is false" (Session 042).
  Impact: INV-BILATERAL-001 and NEG-BILATERAL-001 are falsified. F(S) is NOT a Lyapunov
    function. The convergence thesis (ADR-FOUNDATION-014) lacks its formal foundation.
  Traces to: INV-BILATERAL-001, NEG-BILATERAL-001, ADR-FOUNDATION-014
  Remediation: Redefine: F(S) is monotonic under BILATERAL CYCLE operations only (not
    arbitrary transactions). Or: track F(S) regression and emit GoalDrift signal.
  Confidence: 0.95
```

```
F-CORR-004: MaterializedViews ignores retractions
  Severity: P1-HIGH | Category: CORRECTNESS
  Evidence: store.rs:648 observe_datom() returns immediately for Op::Retract.
    All accumulators (spec_count, coverage, task counts, ISP entity sets) are
    monotonically increasing even when entities are retracted.
  Impact: F(S) reports stale/inflated values. Materialized views diverge from
    batch computation over time as retractions accumulate.
  Traces to: C1 (retractions are first-class datoms), INV-STORE-017
  Remediation: Handle Op::Retract by decrementing relevant accumulators and
    removing entities from sets. ~50 LOC in observe_datom().
  Confidence: 0.95
```

```
F-CORR-005: partial_cmp().unwrap() on f64 in Cheeger computation
  Severity: P1-HIGH | Category: CORRECTNESS
  Evidence: query/graph.rs:2388,2427 — Fiedler vector values from eigendecomposition
    may produce NaN. partial_cmp returns None on NaN; unwrap() panics.
  Impact: Crashes topology compilation pipeline on ill-conditioned coupling matrices.
  Traces to: INV-TOPOLOGY-005
  Remediation: Use .unwrap_or(std::cmp::Ordering::Equal) or filter NaN.
  Confidence: 0.90
```

### 4.3 Critical Architectural Defects

```
ARCH-001: bootstrap_hypotheses.rs performs filesystem I/O in kernel
  Severity: P0-CRITICAL | Category: ARCHITECTURE
  Evidence: bootstrap_hypotheses.rs:11,133,193,224,326,362,412-443 —
    std::fs::read_dir, std::fs::write, std::fs::create_dir_all, std::fs::remove_file.
  Impact: Violates kernel's own doc: "no IO, no filesystem access" (lib.rs:8).
  Traces to: lib.rs preamble, C8
  Remediation: Move to CLI crate. Kernel receives filesystem scan results as data.
  Confidence: 1.0
```

```
ARCH-002: SystemTime::now() in 10+ kernel functions
  Severity: P0-CRITICAL | Category: ARCHITECTURE
  Evidence: guidance.rs:64,114,191,1229,1280,1344,1395,1476; task.rs:379,617
  Impact: Functions are non-deterministic. Violates lib.rs:8-9 determinism guarantee.
    Makes property-based testing unreliable.
  Traces to: lib.rs preamble
  Remediation: Accept `now: u64` parameter. CLI provides the clock.
  Confidence: 1.0
```

```
ARCH-003: guidance.rs is a 14K LOC re-export hub
  Severity: P2-MEDIUM | Category: ARCHITECTURE
  Evidence: guidance.rs:40-42 — pub use crate::context::*; pub use crate::methodology::*;
    pub use crate::routing::*; Combined effective API: 14,137 LOC from 4 modules.
  Impact: Massive namespace pollution. All symbols from 4 modules appear under
    braid_kernel::guidance. Dependency analysis misleading.
  Traces to: NEG-008 (no massive monolithic files)
  Remediation: Remove wildcard re-exports. Export each module independently.
  Confidence: 0.90
```

### 4.4 Type System Defects

| Finding | Severity | Issue |
|---------|----------|-------|
| Value::Keyword accepts unvalidated String | P2 | 392 call sites construct Value::Keyword with no format check |
| EntityId::ZERO sentinel violates C2 | P2 | Not content-derived; use Option<EntityId> instead |
| TxId has public fields bypassing HLC | P2 | Direct construction bypasses tick()/merge() monotonicity |
| SpecId stores element_type as bare String | P3 | Should be enum {Inv, Adr, Neg} with private fields |
| SchemaError::Inconsistency is catch-all | P3 | Single String variant for all schema violations |
| Bare String for TaskId, ShardId, etc. | P2 | Missing newtypes for structurally constrained strings |
| Attribute cloning in hot paths | P2 | 131 clone() calls in store.rs; intern attributes (~200 unique) |

### 4.5 Specification Defects

| Finding | Severity | Issue |
|---------|----------|-------|
| Genesis attr count drift: spec=19, impl=57 | P2 | INV-SCHEMA-002 falsification condition fails against current code |
| Schema monotonicity not enforced on transact | P2 | validate_evolution() exists but not called in transact path |
| Retraction precondition not checked | P2 | Can retract never-asserted datoms (INV-SCHEMA-004 L0 violated) |
| Harvest crystallization guard missing | P1 | NEG-HARVEST-003 (no premature crystallization) unguarded |
| Phi metric uses entity overlap not link-based traceability | P1 | Spec defines D_IS via :spec/traces-to links; code uses entity set membership |
| Test results not stored as datoms | P2 | Impl-Behavior boundary completely unmonitored |

### 4.6 IEEE Walkthrough: 5-Tier Contradiction Detection

| Tier | Found? | Details |
|------|--------|---------|
| 1. Direct contradiction | NO | No INV-A says X while INV-B says not-X found |
| 2. Implication contradiction | **YES** | INV-BILATERAL-001 (F(S) monotonic) contradicts observable behavior (F(S) can decrease). The invariant is aspirational, not enforced. |
| 3. Boundary contradiction | **YES** | C8 (substrate independence) contradicts the actual kernel implementation (40% of LOC is methodology-specific) |
| 4. Temporal contradiction | NO | No time-dependent conflicts found |
| 5. Axiological contradiction | **YES** | The project's true north ("universal substrate") conflicts with the implementation reality ("DDIS-specific runtime"). Every C8 violation is an axiological contradiction. |

---

## Phase 5: Axiological Synthesis

### 5.1 True North Alignment

**Is Braid actually infrastructure for organizational learning?**

Partially. The algebraic core (datom store, CRDT merge, Datalog queries) IS universal.
The content-addressable identity, append-only semantics, and per-attribute resolution
modes work for any domain. **But everything above the store layer is DDIS-specific.**
The coherence model assumes Intent/Spec/Impl. The schema assumes INV/ADR/NEG. The
harvest assumes :exploration/* entities. The guidance assumes a specific methodology.

A React project using Braid today would get: a working datom store, working queries,
working merge — but zero coherence checking, zero guidance, zero bilateral analysis,
and zero meaningful F(S) score. The value proposition of Braid (verifiable coherence)
is entirely locked inside the DDIS policy.

**Does the system actually close all loops?**

No. Seven open loops identified (see Phase 1 Q6). The most critical: signal datoms
are produced but never consumed by any boundary evaluation. Exploration confidence
is set once and never calibrated. Bootstrap hypotheses exist as dead code.

**Is the convergence thesis supported?**

Weakly. F(S) = 0.62 and has been at that level for multiple sessions. The hypothesis
ledger shows "mean error 0.521, trend: degrading" — the system's predictions are getting
WORSE, not better. The calibration loop exists but is not closing effectively.

### 5.2 Maturity Assessment (0-10)

| Dimension | Score | Rationale |
|-----------|-------|-----------|
| **Correctness** | **7** | Implemented features mostly work. Conflict predicate and LIVE view have specific bugs. No crashes in normal use. |
| **Completeness** | **5** | 43% of audited invariants fully implemented. Stage 0 ~90% complete, Stage 1 ~50%, Stage 2+ minimal. |
| **Performance** | **8** | 80ms status on 123K datoms is excellent. Cache broken but release binary compensates. Will degrade at 500K+. |
| **Architecture** | **6** | Clean algebraic core. Crate boundary good. But C8 violations are systemic and the policy manifest is unwired. |
| **Formal rigor** | **6** | forbid(unsafe_code), proptest, Stateright, Kani all used. But 265 INVs lack L2+ witnesses. F(S) monotonicity falsified. |
| **Axiological alignment** | **4** | The stated goal (universal substrate) and the reality (DDIS-specific runtime) are in significant tension. |

**Overall: 6.0 / 10** — A solid working prototype with genuine formal verification,
excellent performance, and clean core algebra, but with a systemic architectural defect
(C8) that must be resolved before the project can serve its stated mission.

### 5.3 The Single Highest-Leverage Change

**Wire the Policy Manifest into the kernel.**

`policy.rs` already defines `PolicyConfig`, `BoundaryDef`, and `CalibrationConfig` with
the right abstractions. The kernel already stores policy datoms. But every consumer
ignores them and uses hardcoded constants. The remediation:

1. `bilateral.rs`: Read F(S) weights from `PolicyConfig.boundaries[].weight` (not constants)
2. `trilateral.rs`: Read namespace partitions from policy boundaries (not INTENT_ATTRS/SPEC_ATTRS/IMPL_ATTRS)
3. `schema.rs`: Move layers 1-4 to a transactable `ddis.edn` manifest (only Layer 0 in kernel)
4. `store.rs MaterializedViews`: Configure observed attribute namespaces from policy
5. `harvest.rs`: Read expected entity profiles from policy
6. `spec_id.rs`: Read valid element types from policy

This is a single coherent refactor that resolves ~60% of all audit findings by addressing
the root cause (unwired policy manifest) rather than symptoms.

### 5.4 Top 5 Compounding Risks

1. **C8 violations compound** — Every new feature added to the kernel increases the DDIS
   coupling. Each session makes it harder to refactor toward substrate independence.

2. **F(S) monotonicity falsehood compounds** — Every session that treats F(S) as a Lyapunov
   function makes decisions based on a false mathematical claim. The longer the claim persists
   unremediated, the more design decisions are grounded in it.

3. **MaterializedViews retraction blindness compounds** — Every retraction makes the views
   more stale. After 10,000 retractions, the F(S) score will be meaningfully inflated.

4. **Store cache brokenness compounds** — As the store grows, the O(F*parse) startup cost
   increases linearly. At 50,000 transaction files, startup will exceed 10s.

5. **Open loops compound** — Data produced but not consumed creates silent technical debt.
   Signal datoms, exploration confidence, and bootstrap hypotheses all represent wasted
   computation that degrades the system's signal-to-noise ratio.

### 5.5 Premortem on This Audit

**What could I have missed?**
- Concurrency issues in LiveStore refresh (only code-analyzed, not stress-tested)
- Edge cases in the Datalog evaluator (no mutation testing)
- Interaction effects between subsystems (each agent audited independently)
- The `crates/braid/src/commands/` layer (26 files, ~34K LOC) received less scrutiny than the kernel

**Where might my assessment be wrong?**
- C8 severity: I classified it as P0-CRITICAL. It's possible that the project team views
  DDIS as the permanent and only policy, in which case C8 is aspirational rather than
  operational. If so, the severity downgrades to P2.
- F(S) monotonicity: The memory says this was already identified in Session 042. My finding
  may be duplicating known work.

**What biases shaped my findings?**
- Anchoring on C8: because C8 violations are pervasive, they dominated my analysis. Smaller
  but important findings (retraction handling, conflict predicate, LIVE view) may have
  received proportionally less attention.
- Confirmation bias toward documented issues: the MEMORY.md flagged several known issues
  which I verified rather than discovered independently.

---

## Appendix A: Complete Finding Registry

### P0-CRITICAL (5)

| ID | Title | Source |
|----|-------|--------|
| F-CORR-003 | F(S) monotonicity claim mathematically false | Lifecycle agent + orchestrator |
| ARCH-001 | bootstrap_hypotheses.rs performs filesystem I/O in kernel | Architecture agent |
| ARCH-002 | SystemTime::now() in 10+ kernel functions | Architecture agent |
| C8-001 | Hardcoded ISP trilateral ontology in kernel | Architecture agent + orchestrator |
| PERF-001 | Store cache broken (hash mismatch prevents regeneration) | Performance agent |

### P1-HIGH (17)

| ID | Title | Source |
|----|-------|--------|
| F-CORR-001 | No causally_independent() implementation | Foundation agent |
| F-CORR-002 | LIVE view hardcodes LWW for all attributes | Foundation agent |
| F-CORR-004 | MaterializedViews ignores retractions | Orchestrator + Foundation agent |
| F-CORR-005 | partial_cmp().unwrap() on f64 in Cheeger | Type System agent |
| F-SPEC-001 | Harvest crystallization guard (INV-HARVEST-006) missing | Lifecycle agent |
| F-SPEC-002 | Phi metric uses entity overlap not link-based traceability | Lifecycle agent |
| F-SPEC-003 | Guidance system is open-loop (no drift detection feedback) | Lifecycle agent |
| C8-002 | Hardcoded INV/ADR/NEG element types in spec_id.rs | Architecture agent |
| C8-003 | Schema layers 1-4 hardcode DDIS-specific attributes | Architecture agent |
| C8-004 | Harvest module assumes DDIS entity categories | Architecture agent |
| C8-005 | Bilateral hardcodes F(S) component weights | Architecture agent |
| C8-006 | Compiler module hardcodes spec attribute patterns | Architecture agent |
| PERF-002 | all_tasks() called 4x per status | Performance agent |
| PERF-005 | entities_matching_pattern() O(N*B) full scan | Performance agent |
| PERF-008 | Datom cloned 5-7x into indexes | Performance agent |
| F-CORR-006 | verify_semilattice() not implemented | Foundation agent |
| F-CORR-007 | Conflict predicate checks 3/6 required conditions | Foundation agent |

### P2-MEDIUM (30)

| ID | Title |
|----|-------|
| F-CORR-008 | Frontier durability (INV-STORE-009) not implemented at Store layer |
| F-CORR-009 | Read commands mutate shared store (INV-STORE-014 violation) |
| F-CORR-010 | Schema monotonicity not enforced on transact path |
| F-CORR-011 | Retraction validation incomplete (can retract never-asserted) |
| F-CORR-012 | Lattice resolution falls back to LWW |
| F-CORR-013 | Three-tier conflict routing not implemented |
| F-SPEC-004 | Genesis attr count drift (spec=19, impl=57) |
| F-SPEC-005 | CC-3 permanently defaults true, no staleness tracking |
| F-SPEC-006 | Residual documentation (INV-BILATERAL-004) unimplemented |
| F-SPEC-007 | Test results not stored as datoms |
| F-SPEC-008 | Merge cascade steps 2-5 are stubs |
| F-SPEC-009 | FP/FN calibration loop (INV-HARVEST-004) not wired |
| F-TYPE-001 | Value::Keyword accepts arbitrary unvalidated String |
| F-TYPE-002 | EntityId::ZERO sentinel violates content-addressing |
| F-TYPE-003 | TxId has public fields bypassing HLC invariants |
| F-TYPE-004 | Bare String for TaskId, ShardId, etc. |
| F-TYPE-005 | Attribute cloning in hot paths (131 clones in store.rs) |
| F-TYPE-006 | Value cloning in index maintenance |
| F-TYPE-007 | layout.rs unwrap on untrusted byte slice conversion |
| F-TYPE-008 | task.rs unwrap after is_none() with gap |
| ARCH-003 | guidance.rs reads source file in test code |
| ARCH-004 | Signal module: 1,465 LOC of dead API surface |
| ARCH-005 | generate_bootstrap_hypotheses exported but never called |
| ARCH-006 | guidance.rs is a 14K LOC re-export hub |
| ARCH-007 | Signal datoms produced but not consumed (open loop) |
| PERF-003 | live_projections() redundant with MaterializedViews |
| PERF-004 | compute_beta_1() full scan + eigendecomposition |
| PERF-006 | concept_observation_coverage() full scan (easy fix) |
| PERF-009 | Schema rebuild on merge unconditional |
| F-SPEC-010 | Seed demonstration density not implemented |

### P3-LOW (20)

| ID | Title |
|----|-------|
| F-CORR-014 | INV-STORE-002 can be vacuously satisfied |
| F-CORR-015 | No compile-time GENESIS_HASH constant |
| F-CORR-016 | CALM compliance is classification-only, not parse-time rejection |
| F-CORR-017 | expect() calls in production paths (4 instances) |
| F-TYPE-009 | SpecId stores element_type as String, not enum |
| F-TYPE-010 | BraidError string variants lose structure |
| F-TYPE-011 | SchemaError::Inconsistency is catch-all |
| F-TYPE-012 | Store lacks initialization state tracking (typestate) |
| F-TYPE-013 | HarvestCandidate lacks lifecycle state (typestate) |
| F-TYPE-014 | run_cycle with_spectral bool parameter |
| F-SPEC-011 | Merge commutativity: Store::merge() mutates target (intermediate inconsistency) |
| F-SPEC-012 | Witness cognitive independence architectural, not enforced |
| ARCH-008 | Exploration entities with no calibration feedback (open loop) |
| ARCH-009 | Seed module imports task module (upward dependency) |
| ARCH-010 | Deep dependency chains through re-export hub |
| PERF-007 | CC-2 existence check full scan |
| PERF-010 | Frontier::at() full scan (O(N) instead of O(log N)) |
| F-DATA-001 | 60 transaction files had hash/filename mismatches (fixed in 4e06be90) |
| F-DATA-002 | EDN parser required patch for dots in keyword namespaces |
| F-DATA-003 | Calibration code double-prefixed :config.scope keywords |

---

## Appendix B: Positive Findings

These are things the project does exceptionally well:

1. **`#![forbid(unsafe_code)]`** — Zero unsafe blocks. Gold standard for Rust safety.
2. **Transaction typestate** — Building → Committed → Applied enforced at compile time. Textbook Curry-Howard.
3. **Content-addressable EntityId** — Private inner field, pub(crate) from_raw_bytes. C2 enforced by construction.
4. **Clean crate boundary** — kernel has zero IO dependencies (blake3, ordered-float, serde only).
5. **Feature flags for stages** — Progressive disclosure via stage0/1/2/3 features.
6. **BTreeSet CvRDT** — Merge is commutative, associative, idempotent by construction. Verified by proptest.
7. **Error recovery hints** — Every error variant has a recovery_hint() method with actionable guidance.
8. **Three-box documentation** — Store, Schema, Query all have black box/state box/clear box docs.
9. **Formal verification** — Kani (bounded model checking), Stateright (protocol checking), proptest (property-based) all used.
10. **Attribute validation** — Namespace format `:ns/name` enforced at construction. ASCII-only guard prevents serialization corruption.

---

## Appendix C: Recommended Execution Plan

### WAVE 0: Blocking Defects (fix before anything else)

| Task | Finding | Files | Acceptance Criterion |
|------|---------|-------|---------------------|
| Move bootstrap_hypotheses.rs to CLI crate | ARCH-001 | bootstrap_hypotheses.rs, lib.rs | No std::fs in kernel; cargo check passes |
| Inject `now: u64` parameter replacing SystemTime::now() | ARCH-002 | guidance.rs, task.rs | grep SystemTime::now returns 0 in kernel |
| Fix store cache pipeline | PERF-001 | layout.rs | `braid status` uses store.bin on second run |
| Redefine F(S) monotonicity invariant | F-CORR-003 | spec/10-bilateral.md, bilateral.rs | INV-BILATERAL-001 states correct scope (bilateral ops only) |
| Handle retractions in MaterializedViews | F-CORR-004 | store.rs observe_datom() | Retraction decrements counters; proptest verifies batch=incremental |

### WAVE 1: Wire Policy Manifest (root cause of C8)

| Task | Finding | Files | Acceptance Criterion |
|------|---------|-------|---------------------|
| Read F(S) weights from PolicyConfig | C8-005 | bilateral.rs | Hardcoded W_* constants removed; weights from policy datoms |
| Parameterize trilateral namespace partitions | C8-001 | trilateral.rs | INTENT_ATTRS etc. read from policy; classify_attribute parameterized |
| Move schema layers 1-4 to ddis.edn manifest | C8-003 | schema.rs | kernel has only Layer 0 (:db/*); ddis.edn transacted at braid init |
| Configure MaterializedViews from policy | C8-001 | store.rs | observe_datom reads observed namespaces from policy boundaries |
| Parameterize spec_id element types | C8-002 | spec_id.rs | Valid types read from :policy/element-types; not hardcoded |
| Read harvest entity profiles from policy | C8-004 | harvest.rs | SPEC_EXPECTED etc. from policy datoms, not constants |

### WAVE 2: Soundness Recovery

| Task | Finding | Files | Acceptance Criterion |
|------|---------|-------|---------------------|
| Implement causally_independent() | F-CORR-001 | resolution.rs, store.rs | BFS over :tx/causal-predecessors; wired into has_conflict() |
| Fix LIVE view per-attribute resolution | F-CORR-002 | store.rs | live_view respects schema.resolution_mode(attr) |
| Guard partial_cmp NaN in Cheeger | F-CORR-005 | query/graph.rs | unwrap_or(Equal) or NaN filter before sort |
| Wire validate_evolution into transact | F-CORR-010 | store.rs, schema.rs | Schema evolution validated on every transaction |
| Add retraction existence check | F-CORR-011 | store.rs | Retracting non-asserted datom returns StoreError |
| Add crystallization guard to harvest | F-SPEC-001 | harvest.rs | High-weight candidates checked for stability before commit |
| Fix Phi to use link-based traceability | F-SPEC-002 | trilateral.rs | D_IS counts :spec/traces-to links, not entity overlap |
| Close guidance feedback loop | F-SPEC-003 | guidance.rs, signal.rs | Drift detection emits GoalDrift signal stored as datom |

### WAVE 3: Performance Hardening

| Task | Finding | Files | Acceptance Criterion |
|------|---------|-------|---------------------|
| Deduplicate all_tasks() calls | PERF-002 | task.rs, status.rs | Computed once per status; passed as parameter |
| Use attribute_index in entities_matching_pattern | PERF-005 | bilateral.rs | Range query on BTreeMap, not full datom scan |
| Intern Attribute strings | PERF-008 | datom.rs, store.rs | Attribute is Copy handle; intern table ~200 entries |
| Index-by-offset for secondary indexes | PERF-008 | store.rs | Vec<Datom> primary; indexes store usize offsets |
| Replace live_projections with MaterializedViews | PERF-003 | trilateral.rs, bilateral.rs | forward_scan/backward_scan use views.isp_* |

### WAVE 4: Verification Completeness

| Task | Finding | Files | Acceptance Criterion |
|------|---------|-------|---------------------|
| Update genesis attr count in spec | F-SPEC-004 | spec/02-schema.md | All "19" references updated to match implementation |
| Implement verify_semilattice() | F-CORR-006 | schema.rs | Lattice axioms checked on :db/resolutionMode = :lattice |
| Add test-result ingestion | F-SPEC-007 | new: test_import.rs | cargo test results → datoms; wired into F(S) |
| Implement conflict routing tiers | F-CORR-013 | resolution.rs | severity_of() + route_to_tier() for every conflict |
| Make Value::Keyword validated | F-TYPE-001 | datom.rs | Construction validates :ns/name format |
| Replace EntityId::ZERO with Option | F-TYPE-002 | datom.rs, schema.rs, resolution.rs | ResolutionMode::Lattice uses Option<EntityId> |

### Quality Gates Between Waves

| Gate | Criterion |
|------|-----------|
| WAVE 0 → 1 | cargo check + cargo test pass. No SystemTime in kernel. Cache works. |
| WAVE 1 → 2 | C8 compliance ≥ 80% (30/38 files). Policy manifest transacted at init. |
| WAVE 2 → 3 | All P0+P1 findings resolved. cargo clippy clean. F(S) invariant restated. |
| WAVE 3 → 4 | braid status < 200ms at 500K datoms. Memory < 100MB at 108K datoms. |
| WAVE 4 → DONE | All P2 findings resolved. Spec-impl coverage ≥ 70%. 0 contradictions. |
