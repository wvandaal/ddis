# Braid Stage 0/1 Comprehensive Audit — Executive Summary

> **Date**: 2026-03-17
> **Methodology**: 11-agent parallel formal audit (Opus 4.6, max effort)
> **Wave 1**: 7 domain-focused Fagan inspection agents (Store, Schema, Query, Harvest/Seed, Merge/Sync, Guidance/Budget/Interface, Bilateral/Signal/Cross-cutting)
> **Wave 2**: 4 cross-cutting synthesis agents (Completeness, Axiological Alignment, Verification Matrix, Readiness/Next Steps)
> **Scope**: All 22 spec namespaces, all guide files, all implementation code (~72,612 LOC, 72 .rs files)
> **Total findings**: 124 domain findings + 4 synthesis reports

---

## 1. Quantitative Dashboard

| Metric | Value | Assessment |
|--------|-------|------------|
| Total Rust LOC | 72,612 across 72 files | Substantial — possibly over-engineered for Stage 0 scope |
| Library tests passing | 791 (unit), 972 (unit+integration excl. broken file) | **GREEN** |
| `cargo check --all-targets` | **FAIL** — generated_coherence_tests.rs:933 unclosed delimiter | **RED** — CI broken |
| Integration test failure | 1 (`schema_store_query`: assertion `19 != 18`) | Genesis count contradiction |
| Spec INVs (total) | 163 across 16 namespaces (+29 in TOPOLOGY/COHERENCE uncatalogued) | — |
| Stage 0 INVs | 83 | — |
| Stage 0 INVs implemented | 62 (75%) | **AMBER** — functional but incomplete |
| Stage 0 INVs divergent | 11 (13%) | Code proves different property than spec requires |
| Stage 0 INVs unimplemented | 10 (12%) | Including critical merge cascade |
| Stage 1 INVs pre-implemented | 12/26 (46%) | Budget and bilateral ahead of schedule |
| Proptest blocks | 143 across 23 files | **Excellent** coverage strategy |
| Kani BMC harnesses | 36 | Core algebraic properties verified |
| Stateright model tests | 11 | Concurrent interleaving verification |
| Fuzzing targets | 0 | **Missing** — T3 absent |
| MIRI coverage | 0 | **Missing** — T4 absent |
| False witnesses | 83 (77 non-compiling + 5 SYNC + 1 Kani ID mismatch) | **CRITICAL** — inflates perceived coverage |
| Datoms in store | 9,314 | Self-bootstrap functioning |
| Spec elements as datoms | 358 across 22 namespaces | Self-bootstrap complete |
| Harvest sessions completed | 25 | Lifecycle operating |
| Observations captured | 43 | Knowledge accumulating |

---

## 2. The Three Systemic Patterns

### Pattern 1: Priority Inversion — Infrastructure Over Purpose

The implementation invested heavily in mathematical infrastructure (spectral graph theory, persistent homology, Ollivier-Ricci curvature, sheaf cohomology, Renyi entropy — ~4,000 LOC in `graph.rs` alone) while leaving the **purpose-layer mechanisms** that implement DDIS's core value proposition stubbed or commented out.

The reconciliation taxonomy — SEED.md §6's organizing principle — defines 8 divergence types. Each should have a detection mechanism and a resolution pathway. Current status:

| Divergence Type | Detection | Resolution | Status |
|-----------------|-----------|------------|--------|
| Epistemic (Store ↔ Agent) | Harvest gap detection | Harvest pipeline | **OPERATIONAL** |
| Structural (Impl ↔ Spec) | Bilateral scan, Φ metric | Associate + guidance | **OPERATIONAL** |
| Procedural (Agent ↔ Methodology) | M(t) methodology score | Dynamic CLAUDE.md | **PARTIAL** — no access log |
| Logical (INV ↔ INV) | Coherence gate Tiers 1-2 | — | **WEAK** — 2/5 tiers |
| Consequential (State ↔ Risk) | — | — | **ABSENT** — no uncertainty tensor |
| Aleatory (Agent ↔ Agent) | — | — | **ABSENT** — types exist, pipeline unwired |
| Axiological (Impl ↔ Goals) | — | — | **ABSENT** — GoalDrift signal commented out |
| Temporal (Frontier ↔ Frontier) | — | — | **ABSENT** — no cross-agent comparison |

**Formal diagnosis**: The system satisfies the *substrate axioms* (C1-C4: append-only, content-addressed, schema-as-data, CRDT merge) but does not yet satisfy the *protocol completeness properties* that make those axioms useful for coherence verification. In algebraic terms: the carrier set and binary operation are defined, but the homomorphisms that map store state to coherence verdicts are incomplete.

### Pattern 2: "Library But Not Wired" — Dead Code at Integration Boundaries

Multiple subsystems are fully implemented at the kernel level but not integrated into the actual execution path:

| Subsystem | Library Status | Integration Status | Gap |
|-----------|---------------|-------------------|-----|
| BudgetManager | 40+ tests, Q(t), precedence, ceiling | `allocate()`/`enforce_ceiling()` never called from CLI dispatch | INV-BUDGET-001 passes unit tests, fails in production |
| Guidance footer | `build_command_footer()` works | Not called from MCP path | Anti-drift dead zone through MCP |
| `conflict_to_datoms` | Produces well-formed datoms | Uses `:resolution/*` attrs not in schema | Schema validation would reject its output |
| Coherence gate | Tiers 1-2 operational | Only at transact time, not bilateral scan | Post-merge contradictions undetected |
| `calibrate_harvest()` | Computes precision/recall/F1/MCC | Never called from harvest pipeline or CLI | FP/FN calibration is dead code |

**Formal diagnosis**: The system exhibits a *structural composition gap*. Each module satisfies its local specification in isolation, but the composition — `Module_A ∘ Module_B` — has not been verified. In Hoare logic terms: `{P_A} A {Q_A}` and `{P_B} B {Q_B}` hold, but `{P_A} A;B {Q_B}` requires `Q_A ⊆ P_B`, and this subsumption has not been checked at the integration level.

### Pattern 3: False Witness Inflation — Coverage Theater

**83 false witnesses** create a significant gap between perceived and actual verification coverage:

1. **77 non-compiling generated tests** (`generated_coherence_tests.rs`): Contains colons/slashes in function names (`fn generated_:spec/inv_store_001_monotonicity`), calls to undefined functions (`predicate()`, `compute_metric()`), 18 tautological no-ops (`let before = x; let after = x; assert!(before <= after)`). Claims coverage of 44 spec elements, provides zero.

2. **5 SYNC false annotations**: `verify_frontier_advancement()` in merge.rs:100-122 is annotated with INV-SYNC-001 through INV-SYNC-005, ADR-SYNC-001-003, NEG-SYNC-001-002. The function is a 12-line pointwise comparison that checks `post >= pre`. It implements none of the barrier semantics, timeout safety, topology independence, entity provenance, or non-monotonic query gating that SYNC invariants require.

3. **1 Kani ID mismatch**: `prove_frontier_monotonicity()` in kani_proofs.rs:442 claims to verify INV-MERGE-002. The spec defines INV-MERGE-002 as "Merge Cascade Completeness" (5-step cascade). The proof verifies frontier monotonicity — a valid but different property.

**Formal diagnosis**: The witness relation `W: Test → INV` is not injective (multiple tests claim the same INV) and not faithful (the claimed INV is not what the test actually verifies). Any automated coverage analysis that trusts the witness annotations will over-report by ~35%.

---

## 3. Axiological Alignment Assessment

**Verdict: PARTIALLY_ALIGNED**

The implementation preserves the **substrate commitments** (append-only datom store, content-addressed identity, schema-as-data, set-union merge, self-bootstrap) with high fidelity. These are the load-bearing novelties that distinguish Braid from conventional architectures.

The implementation **does not yet deliver** on the **central promise**: verifiable coherence across all divergence types. The system is more accurately described as "a datom store with harvest/seed lifecycle and partial coherence checking" than "a system that maintains verifiable coherence between intent, specification, implementation, and observed behavior."

### Four Axiological Drift Patterns

**Drift 1: Reconciliation Taxonomy Inversion** — The taxonomy of 8 divergence types is the system's raison d'etre, but only 2.5/8 types have functional detection and resolution. The signal system has 7/8 types commented out.

**Drift 2: The Datalog Promise** — The entire rationale for choosing Datalog over SQL was transitive closure queries for traceability chains. The evaluator cannot compute transitive closure. Design rationale FD-003 is currently orphaned from the implementation.

**Drift 3: Lattice Resolution Collapse** — Per-attribute resolution (FD-005) was justified by "different attributes need different semantics." Yet `ResolutionMode::Lattice` silently falls back to LWW. Every lattice-resolved attribute exhibits LWW behavior.

**Drift 4: Axiological Self-Blindness** — The divergence type named "axiological" has the weakest implementation. `GoalDrift` signal is commented out. The system designed to detect all forms of divergence cannot detect its own axiological drift.

### Critical Distinction

> The gaps are not quality failures — the code that exists is well-structured, well-tested, and architecturally sound. The gaps are **priority inversions**: significant effort went into sophisticated mathematical machinery while the purpose-layer mechanisms remain stubbed. The implementation built excellent infrastructure for a coherence verification system while leaving the coherence verification itself incomplete.

---

## 4. Stage 0 Completion Assessment

**Overall: 72% complete (68% by INV coverage, 90% CLI deliverables, self-bootstrap functioning)**

### Success Criterion Status

SEED.md §10: *"Work 25 turns, harvest, start fresh with seed — new session picks up without manual re-explanation."*

**Functionally PARTIAL** — The braid-seed section in CLAUDE.md shows 25 harvests and 43 observations with session continuity across sessions 016-021. The mechanism works. However, the open question ("How should seed handle multi-session continuity?") indicates degradation for sessions N-4 and earlier.

**Formally INCOMPLETE** — 62/83 Stage 0 INVs are implemented. 11 are divergent (code proves a different property). 10 are unimplemented.

### Stage 0 Close Blockers

| # | Blocker | Severity | Effort | Addresses |
|---|---------|----------|--------|-----------|
| B1 | Merge cascade completely absent | CRITICAL | M | INV-MERGE-002, INV-MERGE-010, NEG-MERGE-002 |
| B2 | Serializer doesn't sort datoms before hashing | HIGH | S | INV-LAYOUT-011, INV-LAYOUT-001, INV-STORE-003 |
| B3 | Generated test file has syntax error (CI broken) | HIGH | S | Build health |
| B4 | Genesis attribute count three-way contradiction (17/18/19) | HIGH | S | INV-SCHEMA-002, INV-STORE-008 |
| B5 | `EntityId::from_raw_bytes` is `pub` (safety hole) | HIGH | S | INV-STORE-003, ADR-STORE-014 |
| B6 | Evaluator doc claims "semi-naive" but isn't | HIGH | S | INV-QUERY-001 docstring accuracy |

---

## 5. Stage 1 Readiness

**Stage 1 readiness: 46% pre-implemented, requires ~3-4 weeks of wiring + gap closure**

Stage 1 adds 26 INVs for budget-aware output, guidance injection, bilateral loop, and advanced graph metrics. Key blockers before Stage 1 can begin:

1. All Stage 0 close blockers resolved
2. Budget manager wired into CLI dispatch (exists as library, not integrated)
3. Signal infrastructure functional (subscription, dispatch beyond Confusion)
4. MCP store persistence (currently reloads from disk per call)
5. False SYNC witness annotations removed

---

## 6. Verification Health

**Verdict: ADEQUATE (with caveats)**

The core algebraic invariants have excellent multi-tier coverage. The upper-layer and lifecycle namespaces are thin.

| Tier | Tool | Count | Assessment |
|------|------|-------|------------|
| T0 | clippy -D warnings | CI enforced | GREEN |
| T1 | Unit tests | 972 | GREEN |
| T2 | Proptest | 143 blocks | GREEN — excellent strategy hierarchy |
| T3 | Fuzzing | 0 | **RED** — missing entirely |
| T4 | MIRI | 0 | **RED** — missing entirely |
| T5 | Kani BMC | 36 harnesses | GREEN — 3-tier CI (daily/nightly/weekly) |
| T6 | Stateright | 11 tests | GREEN — exhaustive BFS over interleavings |

**INV coverage: 112/163 tested (68.7%), 51 untested (31.3%)**

---

## 7. Prioritized Execution Plan (Top 20)

Ranked by `(impact on coherence × axiological alignment) / effort`. Actions 1-5 are each ~15 minutes. Action 6 is the highest-impact structural fix.

| # | Action | Effort | Impact | Files |
|---|--------|--------|--------|-------|
| 1 | Fix generated_coherence_tests.rs syntax error | S | 9/10 | tests/generated_coherence_tests.rs |
| 2 | Sort datoms before serialization in `serialize_tx` | S | 9/10 | braid-kernel/src/layout.rs |
| 3 | Restrict `EntityId::from_raw_bytes` to `pub(crate)` | S | 8/10 | braid-kernel/src/datom.rs |
| 4 | Reconcile genesis count (pick 19, update all refs) | S | 7/10 | schema.rs, spec/01-store.md, spec/02-schema.md, guides |
| 5 | Remove 10 false SYNC witness annotations | S | 8/10 | braid-kernel/src/merge.rs |
| 6 | Implement merge cascade stub datoms (ADR-MERGE-007) | M | 10/10 | merge.rs, store.rs |
| 7 | Fix `has_conflict` causal independence check | M | 7/10 | resolution.rs |
| 8 | Add `lattice_id` to `ResolutionMode::Lattice` | S | 6/10 | schema.rs, resolution.rs |
| 9 | Register `:resolution/*` attrs in genesis schema | S | 6/10 | resolution.rs, schema.rs |
| 10 | Wire BudgetManager into CLI dispatch | M | 7/10 | commands/mod.rs, commands/status.rs |
| 11 | Fix evaluator docs (remove "semi-naive" claim) | S | 5/10 | query/evaluator.rs |
| 12 | Add `Boundary::ImplBehavior`/`IntentImpl` to bilateral | S | 5/10 | bilateral.rs |
| 13 | Implement INV-SEED-006 intention anchoring | M | 6/10 | seed.rs |
| 14 | Add guidance footer to MCP tool responses | M | 6/10 | mcp.rs |
| 15 | Align harvest warning thresholds with spec | S | 4/10 | harvest.rs |
| 16 | Fix INV-MERGE-002 Kani proof target | M | 7/10 | kani_proofs.rs |
| 17 | Create agent entities for non-genesis agents | M | 5/10 | store.rs |
| 18 | Implement `Store::as_of(tx_id)` | M | 5/10 | store.rs |
| 19 | Fix `DeliberationStatus` Ord derivation | S | 3/10 | deliberation.rs |
| 20 | Implement INV-STORE-009 frontier durability | L | 6/10 | store.rs, layout.rs |

---

## 8. What's Strong — The Foundation That Works

The audit is not all gaps. Several aspects are genuinely excellent and provide a solid foundation:

- **CRDT Algebra (INV-STORE-004-007, INV-MERGE-001)**: Set-union merge verified at 4 tiers (unit, proptest, Kani, Stateright). Commutativity, associativity, idempotency, monotonicity all proven. This is the strongest verification in the project.

- **Trilateral Coherence (INV-TRILATERAL-001-009)**: 8/10 INVs implemented with 127 generated proptests. Best-implemented cross-cutting module. Von Neumann entropy, beta_1 homology, formality gradient all working.

- **F(S) Fitness Function**: All 7 weights match spec exactly (V=0.18, C=0.18, D=0.18, H=0.13, K=0.13, I=0.08, U=0.12), with depth-weighted extensions exceeding spec.

- **Self-Bootstrap**: 358 spec elements transacted as 9,314 datoms. The system genuinely manages its own specification. The bootstrap is not aspirational — it is operational.

- **Harvest/Seed Round-Trip**: 25 harvests, 43 observations captured. Fisher-Rao information-geometric scoring model. Sessions show cross-session continuity.

- **Content-Addressed Layout**: EDN serialization, BLAKE3 hashing, per-transaction files, git coordination. Layout module is clean kernel/IO split.

- **Budget Subsystem** (library level): BudgetManager, Q(t), precedence ordering, attention decay with continuity fix — well-architected and proptest-covered. Needs wiring into CLI.

---

## 9. Formal Methods Assessment

Evaluated against the rust-formal-engineering skill's verification tiers and cleanroom protocol:

### Cleanroom Three-Box Decomposition

| Component | Black Box (Spec) | State Box (Design) | Clear Box (Code) | Verification |
|-----------|-----------------|--------------------|--------------------|-------------|
| Datom Store | spec/01-store.md (16 INV) | docs/guide/01-store.md | store.rs | T1+T2+T5+T6 — **STRONG** |
| Schema | spec/02-schema.md (9 INV) | docs/guide/02-schema.md | schema.rs | T1+T2+T5 — **ADEQUATE** |
| Query Engine | spec/03-query.md (24 INV) | docs/guide/03-query.md | query/*.rs | T1+T2 — **WEAK** (spec claims ≠ reality) |
| Harvest | spec/05-harvest.md (9 INV) | docs/guide/05-harvest.md | harvest.rs | T1+T2+T5 — **ADEQUATE** |
| Seed | spec/06-seed.md (8 INV) | docs/guide/06-seed.md | seed.rs | T1+T2 — **WEAK** (key INVs untested) |
| Merge | spec/07-merge.md (10 INV) | docs/guide/07-merge-basic.md | merge.rs | T1+T2+T5+T6 core only — **WEAK** (cascade absent) |
| Resolution | spec/04-resolution.md (8 INV) | docs/guide/04-resolution.md | resolution.rs | T1+T2+T5 — **ADEQUATE** |
| Guidance | spec/12-guidance.md (11 INV) | docs/guide/08-guidance.md | guidance.rs | T1+T2 — **ADEQUATE** |
| Budget | spec/13-budget.md (6 INV) | docs/guide/10b-budget.md | budget.rs | T1+T2+T5 — **STRONG** (library) |
| Interface | spec/14-interface.md (10 INV) | docs/guide/09-interface.md | commands/*.rs, mcp.rs | T1 — **WEAK** |
| Bilateral | spec/10-bilateral.md (5 INV) | — | bilateral.rs | T1+T2+T6 — **ADEQUATE** |
| Trilateral | spec/18-trilateral.md (10 INV) | docs/guide/13-trilateral.md | trilateral.rs | T1+T2 — **STRONG** |

### Type-Level Safety Assessment

| Property | Rust Mechanism | Status |
|----------|---------------|--------|
| Append-only (C1) | `BTreeSet` — no remove method | **ENFORCED** by type system |
| Content-addressed (C2) | `EntityId` newtype | **PARTIALLY ENFORCED** — `from_raw_bytes` is `pub` |
| Schema-as-data (C3) | `Schema::from_datoms` sole constructor | **ENFORCED** |
| CRDT merge (C4) | `BTreeSet::extend` | **ENFORCED** by type system |
| Transaction typestate | `Transaction<Building>` → `Transaction<Committed>` | **ENFORCED** at compile time |
| No merge-time resolution | `Store::merge(&mut self, &Store)` — no Schema param | **ENFORCED** at type level (NEG-RESOLUTION-001) |

### Gaps vs. Cleanroom Zero-Defect Standard

The project does NOT yet meet cleanroom zero-defect criteria because:
1. **No fuzzing** (T3) — parsers and deserializers untested against arbitrary input
2. **No MIRI** (T4) — no undefined behavior detection (the codebase is `#![forbid(unsafe_code)]` so this is lower risk)
3. **False witnesses** corrupt the verification chain — cleanroom requires every test to verify exactly what it claims
4. **Composition verification absent** — modules verified in isolation but not at integration boundaries

---

## 10. Audit File Index

| File | Contents |
|------|----------|
| `00-executive-summary.md` | This document |
| `01-store-layout.md` | W1-A: Store + Layout domain audit (15 findings) |
| `02-schema-resolution.md` | W1-B: Schema + Resolution domain audit (21 findings) |
| `03-query-engine.md` | W1-C: Query Engine domain audit (15 findings) |
| `04-harvest-seed.md` | W1-D: Harvest + Seed domain audit (18 findings) |
| `05-merge-sync.md` | W1-E: Merge + Sync domain audit (15 findings) |
| `06-guidance-budget-interface.md` | W1-F: Guidance + Budget + Interface domain audit (16 findings) |
| `07-bilateral-signal-crosscutting.md` | W1-G: Bilateral + Signal + Cross-cutting audit (24 findings) |
| `08-implementation-completeness.md` | W2-A: Stage 0/1 implementation completeness synthesis |
| `09-axiological-alignment.md` | W2-B: Axiological alignment assessment |
| `10-verification-coverage.md` | W2-F: Test and verification coverage matrix |
| `11-readiness-next-steps.md` | W2-G: Stage 0/1 readiness assessment and prioritized action plan |

---

*This audit was conducted by 11 Opus 4.6 agents reading every spec file, guide file, and implementation file in the project. All findings are sourced to specific `file:line` locations and spec element IDs. Full agent transcripts are preserved in task output files. The audit methodology combined Fagan Inspection (systematic defect detection with source tracing), IEEE 1028 Walkthrough (cross-referencing between documents), and formal verification assessment (cleanroom three-box decomposition, multi-tier verification coverage).*
