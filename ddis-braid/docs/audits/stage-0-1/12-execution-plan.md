# Stage 0/1 Execution Plan -- Definitive Edition

> Last updated: 2026-03-17 | Total tasks: ~220 | Store: 11,100+ datoms
> This document is the single source of truth for closing Stage 0 and completing Stage 1.
> It supersedes all prior task lists, audit action items, and session notes.
> Any agent can pick this up and execute without additional context.

---

## 1. Current State Assessment

### 1.1 Phase 0 Fixes Already Completed (2026-03-17)

Five critical correctness fixes were completed during the audit triage phase:

| Fix | What It Addressed | Verification |
|-----|-------------------|--------------|
| ADR-COHERENCE-001 retract-then-assert | Status transitions now use retract-then-assert instead of plain assert, preventing ghost datoms from accumulating. Fixes a fundamental append-only store correctness issue. | `cargo test` -- store monotonicity properties still hold |
| GENESIS_ATTR_COUNT + LAYER_1_COUNT constants | Eliminated the three-way genesis attribute count contradiction (17/18/19). The codebase now uses named constants derived from the actual genesis schema. | `schema_store_query` integration test passes |
| Wave gate tests | Full closed-loop validation: generated coherence tests compile and execute correctly against the actual store. | `cargo test --test wave_gates` -- all pass |
| 127 proptest functions via Coherence Compiler | Spec elements generate meaningful property tests via the compiler pipeline. Function names are valid Rust identifiers. Tautological no-ops replaced with real checks. | `cargo test --test generated_coherence_tests` -- 127 pass |
| Self-bootstrap + verify CLI | The compiler generates its own tests and the full E2E pipeline (parse spec -> transact -> compile -> test) completes end-to-end. | `cargo test --test e2e_distillation` -- passes |

**Current build health**: `cargo check --all-targets` **GREEN**. `cargo test --lib --bins` **972 pass, 0 fail**. Integration tests pass (genesis count resolved). Generated coherence tests compile and pass.

### 1.2 What Stage 0 Means (from SEED.md Section 10)

**Stage 0: Harvest/Seed Cycle** -- Validate the core hypothesis: harvest/seed transforms workflow from "fight context loss" to "ride context waves."

**Deliverables** (all must be operational):
- `braid init` -- bootstrap a new store with genesis schema
- `braid transact` -- assert/retract datoms with causal predecessors
- `braid query` -- Datalog queries with entity/attribute filters
- `braid status` -- dashboard with F(S), M(t), tasks, next action
- `braid harvest` -- end-of-session knowledge extraction pipeline
- `braid seed` -- start-of-session context assembly with `--inject AGENTS.md`
- `braid guidance` -- methodology steering (integrated into status/bilateral)
- Dynamic CLAUDE.md generation -- `agent_md.rs` generates context-aware AGENTS.md sections
- MCP server -- 6 tools over JSON-RPC stdio
- Self-bootstrap -- spec elements transacted as datoms (first act)

**Success criterion**: "Work 25 turns, harvest, start fresh with seed -- new session picks up without manual re-explanation."

**Current status**: Functionally PARTIAL. 25 harvests, 43 observations, sessions 016-021 show continuity. Multi-session degradation noted for sessions N-4 and earlier. 62/83 Stage 0 INVs implemented (75%). 11 divergent (code proves different property than spec). 10 unimplemented.

### 1.3 What Stage 1 Means

**Stage 1: Budget-Aware Output + Guidance Injection** -- Context-aware tools with graceful degradation. Every tool output adapts to remaining attention budget. Guidance injection measurably reduces drift.

**26 additional INVs**: BUDGET-001-006, GUIDANCE-003-004, BILATERAL-001-002/004-005, INTERFACE-004/007, QUERY-003/008-009/015-016/018, SIGNAL-002, HARVEST-004/006, SEED-007-008, TRILATERAL-004.

**Key capabilities**:
- Q(t) measurement -- continuous attention budget tracking
- Output precedence -- prioritize high-value content when budget is low
- Guidance compression -- adapt footers to remaining budget
- Bilateral F(S) loop -- convergence monitoring
- Confusion signal -- first signal type for agent confusion detection
- CLAUDE.md quality tracking -- relevance and improvement metrics
- Advanced graph metrics -- betweenness, HITS, k-Core

### 1.4 Current Completion Metrics

| Metric | Value | Target |
|--------|-------|--------|
| Stage 0 INV coverage | 62/83 = 75% | >= 85% (71/83) |
| Stage 0 INVs divergent | 11/83 = 13% | 0% |
| Stage 0 INVs unimplemented | 10/83 = 12% | 0% |
| Stage 0 CLI completeness | 9/10 = 90% | 100% |
| Stage 1 pre-implemented | 12/26 = 46% | 100% |
| Stage 1 partially implemented | 6/26 = 23% | 100% |
| Stage 1 not started | 8/26 = 31% | 0% |
| Total test functions | 972 compiling + 127 generated | >= 1,200 |
| INV test coverage | 112/163 = 68.7% | >= 80% (130/163) |
| False witnesses | 6 remaining (5 SYNC + 1 Kani) | 0 |
| F(S) bilateral fitness | Estimated ~0.72 | >= 0.85 (S0), >= 0.90 (S1) |
| Proptest blocks | 143 across 23 files | 160+ |
| Kani harnesses | 36 | 36+ (fix ID mismatch) |
| Fuzzing targets | 0 | >= 1 (EDN parser) |
| MIRI jobs | 0 | 1 CI job |
| Store datoms | 9,314 | 11,000+ |
| Spec elements in store | 358 across 22 namespaces | 358+ |

---

## 2. Axiological Grounding

### 2.1 The Three Systemic Patterns to Address

These patterns were identified by the 11-agent audit and represent the most significant structural issues. Every wave in this plan explicitly addresses at least one.

**Pattern 1: Priority Inversion -- Infrastructure Over Purpose**

The implementation invested heavily in mathematical infrastructure (~4,000 LOC in graph.rs: spectral graph theory, persistent homology, Ollivier-Ricci curvature, sheaf cohomology, Renyi entropy) while leaving purpose-layer mechanisms stubbed or commented out. The reconciliation taxonomy -- SEED.md Section 6's organizing principle defining 8 divergence types -- has only 2.5/8 types with functional detection and resolution. The signal system has 7/8 types commented out.

*Plan response*: Waves B (merge cascade), C (signal infrastructure, budget wiring), and G5 (divergence detection) directly close this gap. The cascade produces datoms that feed signal routing, which feeds divergence detection. The chain is: Wave B -> Wave C -> Wave G5.

**Pattern 2: Library-Not-Wired -- Dead Code at Integration Boundaries**

Multiple subsystems are fully implemented at the kernel level but not integrated into execution paths: BudgetManager (40+ tests, never called from CLI), guidance footer (works but not in MCP path), conflict_to_datoms (uses unregistered attributes), calibrate_harvest (computes precision/recall but never called). Each module satisfies its local spec in isolation, but the composition has not been verified.

*Plan response*: Wave C is dedicated entirely to wiring. C1 wires budget into CLI. C1.2 implements ArcSwap for MCP persistence. C1.4 adds guidance footer to MCP. C4.1 wires R(t) routing. Each wiring task has an integration test that verifies the composition, not just the component.

**Pattern 3: False Witness Inflation -- Coverage Theater**

83 false witnesses were identified. The Phase 0 fixes resolved 77 (the non-compiling generated coherence tests). Remaining: 5 SYNC false annotations in merge.rs (verify_frontier_advancement claims INV-SYNC-001 through INV-SYNC-005 but implements none of the barrier semantics) and 1 Kani ID mismatch (prove_frontier_monotonicity claims INV-MERGE-002 but verifies a different property).

*Plan response*: Wave A tasks A3 (remove false SYNC claims) and A7 (fix Kani proof ID) close this entirely. Wave F adds real tests for the untested invariants.

### 2.2 The Four Axiological Drift Patterns

These represent deeper misalignments between the implementation and its stated purpose.

**Drift 1: Reconciliation Taxonomy Inversion**

The taxonomy of 8 divergence types is the system's raison d'etre (SEED.md Section 6). Only 2.5/8 types have functional detection and resolution. The signal system's 7 commented-out variants are the visible symptom.

*Addressed by*: Wave C2 (signal infrastructure), Wave G5 (divergence detection for 6 missing types), Wave G4 (conflict/resolution pipeline). Target: 5/8 types functional by Stage 1 close.

**Drift 2: The Datalog Promise**

The entire rationale for choosing Datalog over SQL (FD-003) was transitive closure queries for traceability chains (goal -> INV -> impl -> test). The evaluator is a single-pass nested-loop join that cannot compute transitive closure. The design rationale is orphaned from the implementation.

*Addressed by*: Wave C3.3 (Datalog text parser), Wave G5.3/G5.4 (semi-naive fixpoint evaluation with delta tracking). Stage 1 target: recursive queries operational for traceability chains.

**Drift 3: Lattice Resolution Collapse**

Per-attribute resolution (FD-005) was justified by "different attributes need different semantics." Yet `ResolutionMode::Lattice` silently falls back to LWW at resolution.rs:159. Every lattice-resolved attribute exhibits LWW behavior, defeating the diamond lattice structures designed to produce coordination signals.

*Addressed by*: Wave A5 (add lattice_id field), Wave G1 (full lattice resolution pipeline: LatticeDef, lub(), diamond lattice error signal). Target: lattice resolution fully functional, no LWW fallback.

**Drift 4: Axiological Self-Blindness**

The divergence type named "axiological" -- implementation diverging from goals -- has the weakest implementation. GoalDrift signal is commented out. The system designed to detect all forms of divergence cannot detect its own axiological drift.

*Addressed by*: Wave G5.6 (META: Axiological Self-Blindness), Wave G5.10 (GoalDrift signal + F(S)). Target: GoalDrift signal functional, F(S) includes axiological component.

### 2.3 The 8 Divergence Types and Their Coverage Target

| Divergence Type | Boundary | Current Status | After Wave A-B | After Wave C-G | Target |
|-----------------|----------|---------------|----------------|----------------|--------|
| **Epistemic** | Store vs. Agent | OPERATIONAL | OPERATIONAL | OPERATIONAL | 100% |
| **Structural** | Impl vs. Spec | OPERATIONAL | OPERATIONAL | OPERATIONAL | 100% |
| **Procedural** | Agent vs. Method | PARTIAL (no access log) | PARTIAL | Detection + access log | 80% |
| **Logical** | INV vs. INV | WEAK (2/5 tiers) | WEAK (2/5) | 3-4/5 tiers | 70% |
| **Consequential** | State vs. Risk | ABSENT | ABSENT | Detection via uncertainty tensor | 60% |
| **Aleatory** | Agent vs. Agent | ABSENT (types exist) | Types + cascade | Detection + deliberation dispatch | 70% |
| **Axiological** | Impl vs. Goals | ABSENT | ABSENT | GoalDrift signal + F(S) | 60% |
| **Temporal** | Frontier vs. Frontier | ABSENT | ABSENT | Deferred to Stage 2-3 | 0% (DEFERRED) |

**Stage 1 target**: At least 5/8 divergence types with functional detection mechanisms. Temporal deferred to Stage 3 per spec.

---

## 3. Execution Waves

Tasks are grouped into waves that respect dependency ordering. Within each wave, tasks are parallelizable unless explicitly chained. Waves must execute in order: A before B, B before C, etc.

### Effort Estimation Key

| Symbol | Meaning | Approximate Time |
|--------|---------|------------------|
| S | Small | < 30 minutes |
| M | Medium | 30 minutes - 2 hours |
| L | Large | 2+ hours or multi-session |

---

### Wave A: Critical Correctness (P0) -- PARTIALLY COMPLETE

**Goal**: All P0 bugs fixed. Green CI. Correct algebraic properties. Zero false witnesses.
**Status**: 5 of ~17 P0 tasks completed in Phase 0 (genesis count, generated tests, compiler). ~12 remaining.
**Effort**: 2-3 agent sessions.
**Success criterion**: `cargo test --all-targets` all green. No false witnesses. Content-addressed identity holds for all code paths. All Kani harnesses verify the property they claim.

#### A1: Sort datoms before hashing in serialize_tx

| Field | Value |
|-------|-------|
| **Task ID** | t-5346 |
| **Findings** | Audit 01-store-layout.md FINDING-002; Audit 11-readiness BLOCKER 2 |
| **Spec traces** | INV-LAYOUT-011 (Canonical Serialization), INV-LAYOUT-001 (Content-Addressed File Identity), INV-STORE-003 (Content-Addressable Identity) |
| **Files** | `crates/braid-kernel/src/layout.rs` -- `serialize_tx()` function |
| **Implementation** | Sort `tx.datoms` by the Datom's `Ord` implementation before iterating for EDN serialization. Two agents constructing the same transaction with datoms in different insertion orders must produce identical content hashes. Add `datoms.sort()` or collect into a `BTreeSet` before serialization. |
| **Test** | Wave F task F8 (proptest for canonical serialization order independence). Also add a unit test: construct same datoms in two different orders, serialize both, assert hash equality. |
| **Dependencies** | None (independent) |
| **Effort** | S |

#### A2: Restrict EntityId::from_raw_bytes to pub(crate)

| Field | Value |
|-------|-------|
| **Task ID** | t-1b91 |
| **Findings** | Audit 01-store-layout.md FINDING-001; Audit 11-readiness BLOCKER 5 |
| **Spec traces** | INV-STORE-003 (Content-Addressable Identity), ADR-STORE-014 (Private Inner Field) |
| **Files** | `crates/braid-kernel/src/datom.rs:66-68` -- `EntityId::from_raw_bytes` |
| **Implementation** | Change `pub fn from_raw_bytes` to `pub(crate) fn from_raw_bytes`. The function is documented "for deserialization only" but its `pub` visibility allows any external crate to construct arbitrary EntityId values bypassing content-addressed identity. Kani proofs and proptest strategies use it internally (both within the crate), so `pub(crate)` preserves all test usage. |
| **Test** | Compile-time enforcement: external crate usage would fail to compile. Add a negative compile test if Rust trybuild is available, or document the restriction. |
| **Dependencies** | None (independent) |
| **Effort** | S |

#### A3: Remove false SYNC witness claims from merge.rs

| Field | Value |
|-------|-------|
| **Task ID** | t-61ff |
| **Findings** | Audit 10-verification.md Section 4 Category A; Audit 11-readiness BLOCKER 6 |
| **Spec traces** | INV-SYNC-001 through INV-SYNC-005, ADR-SYNC-001-003, NEG-SYNC-001-002 (all Stage 3) |
| **Files** | `crates/braid-kernel/src/merge.rs:100-122` -- `verify_frontier_advancement()` doc comments |
| **Implementation** | Remove all SYNC-related witness annotations from the `verify_frontier_advancement()` doc comments. This function is a 12-line pointwise comparison checking `post >= pre`. It implements none of the barrier semantics, timeout safety, topology independence, entity provenance, or non-monotonic query gating that SYNC invariants require. SYNC is Stage 3. The function can be annotated with what it actually verifies: frontier monotonicity (related to INV-STORE-007 or a general merge property). |
| **Test** | Grep codebase for `INV-SYNC` annotations; verify count drops from 5 to 0 outside spec/ files. |
| **Dependencies** | None (independent) |
| **Effort** | S |

#### A4: Register :resolution/* attributes in genesis schema

| Field | Value |
|-------|-------|
| **Task ID** | t-7e8a |
| **Findings** | Audit 02-schema-resolution.md; `conflict_to_datoms` uses unregistered attributes |
| **Spec traces** | INV-SCHEMA-004 (Schema Validation on Transact), INV-RESOLUTION-008 (Conflict Entity Datom Trail) |
| **Files** | `crates/braid-kernel/src/schema.rs` (genesis attributes), `crates/braid-kernel/src/resolution.rs` (conflict_to_datoms) |
| **Implementation** | Add `:resolution/conflict-id`, `:resolution/attribute`, `:resolution/conflicting-values`, `:resolution/mode`, `:resolution/status` (and any other attributes used by `conflict_to_datoms()`) to the genesis schema's axiomatic attribute list. Update `GENESIS_ATTR_COUNT` and `LAYER_1_COUNT` constants accordingly. Schema validation on transact (INV-SCHEMA-004) will then accept datoms produced by `conflict_to_datoms`. |
| **Test** | Call `conflict_to_datoms` on a test store, transact the result, verify no schema validation error. |
| **Dependencies** | None (independent, but A5 should be done in the same session) |
| **Effort** | S |

#### A5: Add lattice_id field to ResolutionMode::Lattice

| Field | Value |
|-------|-------|
| **Task ID** | t-70b0 |
| **Findings** | Audit 02-schema-resolution.md; lattice resolution requires identifying which lattice definition to use |
| **Spec traces** | INV-SCHEMA-007 (Lattice Definition Completeness) |
| **Files** | `crates/braid-kernel/src/schema.rs` (ResolutionMode enum), `crates/braid-kernel/src/resolution.rs` (resolve function) |
| **Implementation** | Change `ResolutionMode::Lattice` from a unit variant to `ResolutionMode::Lattice { lattice_id: EntityId }`. This field identifies which lattice definition (stored as datoms with `:lattice/*` attributes) governs resolution for this attribute. Update all match arms. The resolve() function should use lattice_id to look up the LatticeDef. For now, the LWW fallback remains but is explicitly gated on `lattice_id` lookup failure (not a silent default). |
| **Test** | Unit test: construct a schema with `ResolutionMode::Lattice { lattice_id }`, verify the lattice_id is preserved through schema round-trip (datoms -> Schema -> datoms). |
| **Dependencies** | None (independent, complements A4) |
| **Effort** | S |

#### A6: Fix has_conflict causal independence check

| Field | Value |
|-------|-------|
| **Task ID** | t-152b |
| **Findings** | Audit 02-schema-resolution.md; INV-RESOLUTION-004 partial |
| **Spec traces** | INV-RESOLUTION-004 (Conflict Predicate Soundness) |
| **Files** | `crates/braid-kernel/src/resolution.rs` -- `has_conflict()` function |
| **Implementation** | The conflict predicate currently checks 6 conditions (same entity, same attribute, different values, both asserted, no retraction, different transactions) but is missing the causal independence check: two datoms conflict only if neither transaction is a causal ancestor of the other. If tx_a is a causal ancestor of tx_b, then tx_b supersedes tx_a (not a conflict). Implement `is_causal_ancestor(store, tx_a, tx_b) -> bool` using a BFS walk through `:tx/causal-predecessors` datoms (see B1.2), then add `!is_causal_ancestor(store, d1.tx, d2.tx) && !is_causal_ancestor(store, d2.tx, d1.tx)` to the conflict predicate. |
| **Test** | Proptest: generate two transactions where one is a causal predecessor of the other, asserting different values for the same entity+attribute. `has_conflict` should return false. Generate two causally independent transactions, `has_conflict` should return true. |
| **Dependencies** | A4 (needs `:resolution/*` attributes registered). Also benefits from B1.2 (BFS walk implementation), but the causal ancestor check can be implemented independently with a simpler version first. |
| **Effort** | M |

#### A7: Fix INV-MERGE-002 Kani proof to verify cascade (not frontier monotonicity)

| Field | Value |
|-------|-------|
| **Task ID** | t-8ef5 |
| **Findings** | Audit 10-verification.md Section 4 Category C; Audit 05-merge-sync.md FINDING-001 |
| **Spec traces** | INV-MERGE-002 (Merge Cascade Completeness) |
| **Files** | `crates/braid-kernel/src/kani_proofs.rs:442` -- `prove_frontier_monotonicity()` |
| **Implementation** | The Kani proof at line 442 claims to verify INV-MERGE-002 but actually verifies frontier monotonicity (a valid but different property). Two fixes: (1) Re-label the existing proof from "INV-MERGE-002" to "INV-STORE-007" or "MERGE-MONOTONICITY" since frontier monotonicity is what it actually tests. (2) After Wave B implements merge cascade, add a new Kani harness `prove_merge_cascade_completeness` that verifies the 5-step cascade produces datoms for each step. Also update the module header at line 16 which says "INV-MERGE-002: Frontier monotonicity." The Stateright model (stateright_model.rs:15) has the same mislabel and should be corrected. |
| **Test** | Kani harness compiles and passes. `grep -r "INV-MERGE-002" crates/` shows only correct references. |
| **Dependencies** | Relabeling can happen immediately. The new cascade proof depends on B1 (merge cascade implementation). Split into two sub-tasks: A7a (relabel, S) and A7b (new cascade proof, M, after B1). |
| **Effort** | M (total: S for relabel + M for new proof) |

#### A8: Fix function name generation in coherence compiler

| Field | Value |
|-------|-------|
| **Task ID** | t-c5a6 |
| **Findings** | Audit 10-verification.md Section 4 Category B; VERIFY-SUB |
| **Spec traces** | INV-TRILATERAL-001 (Coherence Verification), generated test infrastructure |
| **Files** | `crates/braid-kernel/src/compiler.rs` -- `emit_test_module()` function |
| **Implementation** | **COMPLETED in Phase 0.** Function names now use valid Rust identifiers (underscores instead of colons/slashes). The `fn generated_:spec/inv_store_001_monotonicity` pattern is replaced with `fn generated_spec_inv_store_001_monotonicity`. Verified by successful compilation of 127 generated tests. |
| **Test** | `cargo test --test generated_coherence_tests` -- all 127 pass |
| **Dependencies** | None (complete) |
| **Effort** | DONE |

#### A9: Implement predicate/compute_metric helper functions

| Field | Value |
|-------|-------|
| **Task ID** | t-5e62 |
| **Findings** | Audit 10-verification.md; generated tests called undefined functions |
| **Spec traces** | Generated test infrastructure |
| **Files** | `crates/braid-kernel/tests/generated_coherence_tests.rs` (or helper module) |
| **Implementation** | **COMPLETED in Phase 0.** The tautological templates that called undefined `predicate()` and `compute_metric()` functions were replaced with meaningful checks that exercise actual store/schema/query operations. The compiler now emits tests that use real Store, Schema, and Datom operations. |
| **Test** | All 127 generated tests compile and pass |
| **Dependencies** | None (complete) |
| **Effort** | DONE |

#### A10: Replace tautological no-op templates with meaningful checks

| Field | Value |
|-------|-------|
| **Task ID** | t-9755 |
| **Findings** | Audit 10-verification.md; 18 tautological monotonicity tests, 6 tautological immutability tests |
| **Spec traces** | Generated test infrastructure |
| **Files** | `crates/braid-kernel/src/compiler.rs` -- template generation |
| **Implementation** | **COMPLETED in Phase 0.** Templates now generate tests that perform actual mutations (transact datoms, merge stores, resolve conflicts) and check the property after the mutation. Monotonicity tests transact and verify count increases. Immutability tests snapshot, transact new datoms, and verify old datoms unchanged. |
| **Test** | Generated tests pass with non-trivial assertions |
| **Dependencies** | None (complete) |
| **Effort** | DONE |

#### A11: Fix code generator for coherence tests

| Field | Value |
|-------|-------|
| **Task ID** | t-b2f0 |
| **Findings** | Meta-task for A8-A10 |
| **Spec traces** | Generated test infrastructure |
| **Files** | `crates/braid-kernel/src/compiler.rs` |
| **Implementation** | **COMPLETED in Phase 0.** The compiler pipeline now produces compilable, semantically meaningful tests. The full E2E pipeline (parse spec -> transact elements -> compile -> emit tests -> test) completes successfully. |
| **Test** | `cargo test --test e2e_distillation` -- end-to-end pipeline test passes |
| **Dependencies** | None (complete) |
| **Effort** | DONE |

#### A12: Fix MergeReceipt to include duplicate_datoms and frontier_delta

| Field | Value |
|-------|-------|
| **Task ID** | t-2cc1 |
| **Findings** | Audit 05-merge-sync.md FINDING-002 |
| **Spec traces** | INV-MERGE-009 (Merge Receipt Completeness) |
| **Files** | `crates/braid-kernel/src/store.rs:418-423` -- MergeReceipt struct, `Store::merge()` method |
| **Implementation** | The spec defines `MergeReceipt { new_datoms, duplicate_datoms, frontier_delta }`. The implementation has `MergeReceipt { new_datoms, total_datoms }`. Add `duplicate_datoms: usize` and `frontier_delta: HashMap<AgentId, (Option<TxId>, TxId)>` fields. In `Store::merge()`, compute duplicate_datoms as `other.datoms.len() - new_datoms` and frontier_delta by comparing pre-merge and post-merge frontier for each agent. Remove the non-spec `total_datoms` field (or keep it as a convenience alongside the spec fields). |
| **Test** | Unit test: merge two stores with known overlap, verify `duplicate_datoms` equals expected count. Verify `frontier_delta` maps each agent to (old_frontier, new_frontier). |
| **Dependencies** | None (independent). Feeds into B1.1 (CascadeReceipt extends MergeReceipt). |
| **Effort** | S |

**Wave A Summary**: 12 tasks total. 5 DONE (A8-A11 from Phase 0). 7 remaining (A1-A7, A12). A1, A2, A3, A4, A5, A12 are independent and can be parallelized. A6 depends on A4. A7 splits into immediate relabel (A7a) and deferred proof (A7b after Wave B).

---

### Wave B: Stage 0 Close (P1 S0-CLOSE) -- NOT STARTED

**Goal**: All 83 Stage 0 INVs meaningfully addressed. Coverage from 75% to 90%+. Merge cascade functional. All BLOCKER items resolved.
**Effort**: 4-6 agent sessions.
**Success criterion**: `braid bilateral` shows F(S) >= 0.85 for Stage 0 namespaces. No BLOCKER items remain. 71+ of 83 Stage 0 INVs implemented.
**Prerequisite**: Wave A complete (CI green, correct algebraic properties).

#### B1: Merge Cascade -- The Single Most Important Action

The merge cascade (INV-MERGE-002) is the critical Stage 0 gap. The spec requires 5 cascade steps (conflict detection, cache invalidation, projection staleness, uncertainty update, subscription notification), each producing datoms. ADR-MERGE-007 specifies a Stage 0 stub-datom approach: each step produces a marker datom rather than executing the full logic.

##### B1.1: Implement CascadeReceipt struct

| Field | Value |
|-------|-------|
| **Task ID** | t-f869 |
| **Spec traces** | INV-MERGE-002, INV-MERGE-010, ADR-MERGE-007 |
| **Files** | `crates/braid-kernel/src/merge.rs` or `crates/braid-kernel/src/store.rs` |
| **Implementation** | Define `CascadeReceipt { steps: Vec<CascadeStep>, total_datoms_produced: usize }` and `CascadeStep { step_type: CascadeStepType, datoms_produced: Vec<Datom>, entity_id: EntityId }` with `CascadeStepType` enum having 5 variants: `ConflictDetection`, `CacheInvalidation`, `ProjectionStaleness`, `UncertaintyUpdate`, `SubscriptionNotification`. Each step records the datoms it produced. |
| **Test** | Type-level: CascadeReceipt can be constructed. Unit: verify 5 step types cover the spec. |
| **Dependencies** | A12 (MergeReceipt fixed first) |
| **Effort** | S |

##### B1.2: Implement is_causal_ancestor() BFS walk

| Field | Value |
|-------|-------|
| **Task ID** | t-eb7f |
| **Spec traces** | INV-RESOLUTION-004 (Conflict Predicate Soundness), INV-MERGE-002 (cascade step 1) |
| **Files** | `crates/braid-kernel/src/merge.rs` or `crates/braid-kernel/src/store.rs` |
| **Implementation** | Implement `is_causal_ancestor(store: &Store, ancestor: TxId, descendant: TxId) -> bool` using a BFS walk through `:tx/causal-predecessors` datoms. Starting from `descendant`, walk backwards through causal predecessor links. Return true if `ancestor` is encountered. Use a `HashSet<TxId>` for visited to prevent cycles. Limit depth to prevent pathological cases (configurable, default 1000). |
| **Test** | Unit: chain of 3 transactions A -> B -> C. is_causal_ancestor(A, C) == true. is_causal_ancestor(C, A) == false. Two independent transactions: is_causal_ancestor returns false in both directions. |
| **Dependencies** | None (independent, uses existing store query capabilities) |
| **Effort** | M |

##### B1.3: Implement cascade step 1 -- conflict detection after merge

| Field | Value |
|-------|-------|
| **Task ID** | t-b70c |
| **Spec traces** | INV-MERGE-002 step 1, INV-RESOLUTION-004 |
| **Files** | `crates/braid-kernel/src/merge.rs` |
| **Implementation** | After the set-union merge, scan the newly-added datoms for conflicts using `has_conflict()` (fixed in A6 with causal independence check). For each conflict found, produce a datom recording the conflict: `[:conflict/entity e, :conflict/attribute a, :conflict/tx-a tx1, :conflict/tx-b tx2, :conflict/detected-at merge_tx]`. Collect all conflict datoms into `CascadeStep { step_type: ConflictDetection, datoms_produced: conflict_datoms }`. |
| **Test** | Merge two stores with a real conflict (same entity, same attribute, different values, causally independent). Verify CascadeStep::ConflictDetection contains the conflict datom. Merge two stores with no conflict, verify step produces zero datoms. |
| **Dependencies** | B1.1 (CascadeReceipt struct), B1.2 (is_causal_ancestor for conflict check), A6 (fixed has_conflict) |
| **Effort** | M |

##### B1.4: Implement cascade steps 2-5 stub datoms (ADR-MERGE-007)

| Field | Value |
|-------|-------|
| **Task ID** | t-2dbb |
| **Spec traces** | INV-MERGE-002 steps 2-5, ADR-MERGE-007 (stub datoms at Stage 0) |
| **Files** | `crates/braid-kernel/src/merge.rs` |
| **Implementation** | Per ADR-MERGE-007, Stage 0 cascade steps 2-5 produce marker datoms recording that the step ran but not executing full logic. For each of cache invalidation (step 2), projection staleness (step 3), uncertainty update (step 4), and subscription notification (step 5): create a datom `[:cascade/step-type <type>, :cascade/status :stub, :cascade/merge-tx merge_tx]`. Each step becomes a `CascadeStep` in the receipt. The stubs are replaced with real logic in Stage 1-2. |
| **Test** | After any merge, the CascadeReceipt has exactly 5 steps. Steps 2-5 each produce exactly 1 stub datom with `:cascade/status :stub`. |
| **Dependencies** | B1.3 (step 1 establishes the cascade pattern) |
| **Effort** | M |

##### B1.5: Wire cascade into Store::merge return type

| Field | Value |
|-------|-------|
| **Task ID** | t-6094 |
| **Spec traces** | INV-MERGE-002, INV-MERGE-009, INV-MERGE-010 |
| **Files** | `crates/braid-kernel/src/store.rs` -- `Store::merge()` signature and callers |
| **Implementation** | Change `Store::merge()` return type from `MergeReceipt` to `(MergeReceipt, CascadeReceipt)`. Update all callers (merge command in `crates/braid/src/commands/mod.rs`, test files, stateright models). The cascade runs after the set-union and before the function returns. Register all `:cascade/*` and `:conflict/*` attributes in the genesis schema (extends A4). |
| **Test** | Integration test: merge two stores, destructure the return into (merge_receipt, cascade_receipt). Verify cascade_receipt.steps.len() == 5. |
| **Dependencies** | B1.4 (all cascade steps implemented) |
| **Effort** | M |

##### B1.6: Full merge cascade integration

| Field | Value |
|-------|-------|
| **Task ID** | t-0246 |
| **Spec traces** | INV-MERGE-002 (Merge Cascade Completeness) |
| **Files** | `crates/braid-kernel/src/merge.rs`, `crates/braid-kernel/src/store.rs` |
| **Implementation** | End-to-end integration: the cascade datoms are transacted into the merged store (not just returned). This means the merge operation itself produces datoms that become part of the store's permanent record. Verify that cascade datoms are content-addressed (same merge of same stores produces same cascade datoms -- idempotent). The cascade transaction should reference the merge transaction as a causal predecessor. |
| **Test** | Proptest: merge two random stores, verify cascade datoms in the resulting store. Merge again (idempotent), verify no duplicate cascade datoms. Kani harness (A7b): verify all 5 steps produce datoms. |
| **Dependencies** | B1.5 (cascade wired into return type) |
| **Effort** | L |

##### B1.7: Implement INV-MERGE-010 cascade determinism receipt

| Field | Value |
|-------|-------|
| **Task ID** | t-2a09 |
| **Spec traces** | INV-MERGE-010 (Cascade Determinism) |
| **Files** | `crates/braid-kernel/src/merge.rs` |
| **Implementation** | The CascadeReceipt must capture the conflict set so that the same conflicts always produce the same cascade steps (deterministic). Add a `conflicts: Vec<ConflictSet>` field to CascadeReceipt. Hash the receipt to produce a receipt_id. Two merges of the same stores must produce the same receipt_id. |
| **Test** | Proptest: merge stores A and B, get receipt_1. Merge A and B again, get receipt_2. Assert receipt_1.receipt_id == receipt_2.receipt_id. |
| **Dependencies** | B1.6 (full cascade functional) |
| **Effort** | M |

**B1 chain**: B1.1 -> B1.3 -> B1.4 -> B1.5 -> B1.6 -> B1.7. B1.2 feeds into B1.3 independently.

#### B2: Resolution + Schema

##### B2.1: Implement INV-RESOLUTION-003 convergence verification

| Field | Value |
|-------|-------|
| **Task ID** | t-a7be |
| **Spec traces** | INV-RESOLUTION-003 (Convergence: identical stores yield identical resolved values) |
| **Files** | `crates/braid-kernel/src/resolution.rs` |
| **Implementation** | Add a `verify_convergence(store_a: &Store, store_b: &Store, attr: &Attribute) -> bool` function that checks: if store_a.datoms == store_b.datoms, then resolve(store_a, entity, attr) == resolve(store_b, entity, attr) for all entities. This is the CRDT convergence property applied to resolution. The function should be callable from tests and from the bilateral scan. |
| **Test** | Proptest: generate random store, clone it, resolve same attribute on both, assert identical results. Second proptest: generate two stores, merge both ways (A into B, B into A), verify convergence. |
| **Dependencies** | A4 (registered resolution attributes), A6 (fixed conflict predicate) |
| **Effort** | M |

##### B2.2: Fix conflict_to_datoms to use registered attributes

| Field | Value |
|-------|-------|
| **Task ID** | t-551e |
| **Spec traces** | INV-RESOLUTION-008 (Conflict Entity Datom Trail) |
| **Files** | `crates/braid-kernel/src/resolution.rs` -- `conflict_to_datoms()` |
| **Implementation** | After A4 registers `:resolution/*` attributes in the genesis schema, update `conflict_to_datoms()` to use exactly the registered attribute names. Verify that the datoms produced by this function pass schema validation when transacted. The function should produce datoms that can be queried using standard attribute-based queries. |
| **Test** | Unit test: produce conflict datoms, transact them into a store with genesis schema, verify no schema validation error. |
| **Dependencies** | A4 (attributes must be registered first) |
| **Effort** | S |

##### B2.3: Validate lattice definitions for all 4 required properties

| Field | Value |
|-------|-------|
| **Task ID** | t-1e2a |
| **Spec traces** | INV-SCHEMA-007 (Lattice Definition Completeness) |
| **Files** | `crates/braid-kernel/src/schema.rs` |
| **Implementation** | Each lattice definition stored as datoms must satisfy 4 properties: (1) `join(a, bottom) = a` (identity), (2) `join(a, a) = a` (idempotence), (3) `join(a, b) = join(b, a)` (commutativity), (4) `join(a, join(b, c)) = join(join(a, b), c)` (associativity). Add `validate_lattice(lattice_id: EntityId) -> Result<(), SchemaError>` that checks these properties against the stored lattice elements. Run during schema construction from datoms. |
| **Test** | Create a well-formed Severity lattice (Low/Medium/High/Critical) and verify it passes all 4 checks. Create a malformed lattice (non-associative join) and verify validation rejects it. |
| **Dependencies** | A5 (lattice_id field in ResolutionMode) |
| **Effort** | M |

#### B3: Lifecycle + Interface

##### B3.1: Implement INV-SEED-006 intention anchoring

| Field | Value |
|-------|-------|
| **Task ID** | t-acf0 |
| **Spec traces** | INV-SEED-006 (Intention Anchoring) |
| **Files** | `crates/braid-kernel/src/seed.rs` -- `assemble_seed()` |
| **Implementation** | When `assemble_seed()` is called with a task parameter, the task string should be anchored in the Directive section of the seed output. The seed should include the task intention prominently (not buried in metadata) and use it to influence entity ranking during assembly. Entities relevant to the task should rank higher. The task intention should persist across seed sections, preventing task drift during long sessions. |
| **Test** | Wave F task F3: call assemble_seed with task="implement merge cascade", verify the Directive section contains the task string, verify merge-related spec elements rank higher than unrelated elements. |
| **Dependencies** | None |
| **Effort** | M |

##### B3.2: Fix seed budget check -- not tautological

| Field | Value |
|-------|-------|
| **Task ID** | t-ee9a |
| **Spec traces** | NEG-SEED-002 (Seed Budget Not Tautological) |
| **Files** | `crates/braid-kernel/src/seed.rs` -- `verify_seed()` |
| **Implementation** | The seed budget check currently compares the seed's token count against a budget that is always larger than the seed. Replace with a meaningful check: the seed's total token count should not exceed the configured budget (default: portion of context window). The budget should be parameterized, not a tautologically-large constant. |
| **Test** | Set a small budget (e.g., 500 tokens). Generate a seed that would exceed it. Verify `verify_seed` returns an error or truncates. |
| **Dependencies** | None |
| **Effort** | S |

##### B3.3: Align harvest warning thresholds with spec

| Field | Value |
|-------|-------|
| **Task ID** | t-ca63 |
| **Spec traces** | INV-HARVEST-005 (Proactive Warning) |
| **Files** | `crates/braid-kernel/src/harvest.rs` |
| **Implementation** | The current warning thresholds diverge 3x from spec values. Align the thresholds in `compute_harvest_urgency()` or equivalent with the values specified in spec/05-harvest.md. The spec defines specific Q(t) thresholds that trigger warnings; the implementation should use those exact values. |
| **Test** | Unit test: simulate Q(t) crossing each threshold, verify correct warning level is emitted. |
| **Dependencies** | None |
| **Effort** | S |

##### B3.4: Fix DeliberationStatus -- do NOT derive Ord

| Field | Value |
|-------|-------|
| **Task ID** | t-77a9 |
| **Spec traces** | Spec says "do NOT derive Ord" for DeliberationStatus |
| **Files** | `crates/braid-kernel/src/deliberation.rs` |
| **Implementation** | Remove `Ord` and `PartialOrd` derives from `DeliberationStatus` enum. The deliberation lifecycle (Propose -> Discuss -> Stabilize -> Crystallize) is not a total order -- some transitions are not comparable. Use explicit transition methods instead. |
| **Test** | Compile-time: code that relies on DeliberationStatus ordering should produce a compile error (or be updated to use explicit comparisons). |
| **Dependencies** | None |
| **Effort** | S |

##### B3.5: Implement deliberation stability guard

| Field | Value |
|-------|-------|
| **Task ID** | t-0d9c |
| **Spec traces** | INV-DELIBERATION-002 (Stability Guard) |
| **Files** | `crates/braid-kernel/src/deliberation.rs` |
| **Implementation** | Block premature crystallization: a deliberation cannot transition to Crystallized state unless it has been in Stabilize state for a minimum number of cycles (configurable, default 3). Track a `stability_counter` on deliberation entities. Each cycle in Stabilize increments the counter. Crystallization requires counter >= threshold. |
| **Test** | Unit test: create deliberation, try to crystallize immediately (should fail). Move to Stabilize, increment 3 times, crystallize (should succeed). |
| **Dependencies** | B3.4 (Ord removed first, so transition logic is explicit) |
| **Effort** | M |

##### B3.6: Create agent entities for non-genesis agents

| Field | Value |
|-------|-------|
| **Task ID** | t-403b |
| **Spec traces** | INV-STORE-015 (Agent Entity Creation) |
| **Files** | `crates/braid-kernel/src/store.rs` -- `transact()` |
| **Implementation** | When a transaction is submitted by an agent that doesn't yet have an entity in the store, automatically create an agent entity with `:agent/name`, `:agent/created-at`, and `:agent/first-tx` attributes. This should happen inside `transact()` before the main transaction datoms are committed. The genesis agent already has an entity; this handles subsequent agents. See B4.1 (ensure_agent_entity sub-task). |
| **Test** | Transact with a new agent identifier, verify the store contains an agent entity for that identifier. Transact again with the same agent, verify no duplicate entity. |
| **Dependencies** | None |
| **Effort** | M |

##### B3.7: Fix causal predecessor validation error variant

| Field | Value |
|-------|-------|
| **Task ID** | t-a581 |
| **Spec traces** | INV-STORE-010 (Causal Predecessor Validation) |
| **Files** | `crates/braid-kernel/src/store.rs:156-162` |
| **Implementation** | Currently returns `StoreError::DuplicateTransaction(format!("causal predecessor not found: {:?}", pred))`. The error message says "predecessor not found" but the variant says "DuplicateTransaction" -- wrong variant. Add a new error variant `StoreError::MissingCausalPredecessor(TxId)` and use it instead. |
| **Test** | Transact with a non-existent causal predecessor. Verify the error type is `MissingCausalPredecessor`, not `DuplicateTransaction`. |
| **Dependencies** | None |
| **Effort** | S |

##### B3.8: Implement Store::as_of for temporal queries

| Field | Value |
|-------|-------|
| **Task ID** | t-2935 |
| **Spec traces** | INV-STORE-005 (Temporal Queries) |
| **Files** | `crates/braid-kernel/src/store.rs` |
| **Implementation** | Implement `Store::as_of(&self, frontier: &Frontier) -> SnapshotView` (see B4.2 and B4.3 for sub-tasks). The SnapshotView filters the store's datoms to only those with tx_id <= the frontier's TxId for the relevant agent. Supports time-travel queries: "what was the store state at transaction T?" The Frontier::at() method already exists as a building block. |
| **Test** | Unit test: transact 3 times. as_of(tx_1) should show only datoms from tx_0 and tx_1. as_of(tx_2) should show all. |
| **Dependencies** | B4.2, B4.3 (sub-tasks for implementation details) |
| **Effort** | M |

##### B3.9: Fix layout read_tx to verify content hash on read

| Field | Value |
|-------|-------|
| **Task ID** | t-f684 |
| **Spec traces** | INV-LAYOUT-005 (Content Hash Verification on Read) |
| **Files** | `crates/braid/src/layout.rs` -- `DiskLayout::read_tx()` |
| **Implementation** | When reading a transaction file from disk, re-compute the BLAKE3 hash of the file contents and compare against the filename (which is the hash). If they don't match, return an error. This detects bit-rot and tampering. See B4.4 for sub-task. |
| **Test** | Write a transaction file, corrupt one byte, attempt to read. Verify content hash mismatch error. |
| **Dependencies** | B4.4 (BLAKE3 verification implementation) |
| **Effort** | M |

##### B3.10: Implement INV-INTERFACE-010 anti-drift injection in MCP responses

| Field | Value |
|-------|-------|
| **Task ID** | t-2033 |
| **Spec traces** | INV-INTERFACE-010 (Anti-Drift Injection), INV-GUIDANCE-001 (Every Tool Response), NEG-GUIDANCE-001 |
| **Files** | `crates/braid/src/mcp.rs` |
| **Implementation** | Call `build_command_footer()` from the MCP tool response path, not just the CLI path. Every MCP tool response should include the guidance footer (M(t) score, next action, methodology reminder). This closes the "anti-drift dead zone" identified in the axiological audit. The footer should be appended to the tool response text in a structured section. |
| **Test** | Integration test: call MCP tool_status, verify response contains guidance footer. Call each MCP tool, verify footer presence. |
| **Dependencies** | None (build_command_footer already exists, just needs to be called from MCP path) |
| **Effort** | M |

##### B3.11: Integration-test INV-LAYOUT-010 concurrent write safety

| Field | Value |
|-------|-------|
| **Task ID** | t-2c4c |
| **Spec traces** | INV-LAYOUT-010 (Concurrent Write Safety) |
| **Files** | `crates/braid-kernel/tests/` or `crates/braid/tests/` |
| **Implementation** | The O_CREAT|O_EXCL logic exists but isn't integration-tested. Write a test that spawns 2-3 threads, each attempting to write the same transaction file simultaneously. Verify that exactly one succeeds and others get the expected error. |
| **Test** | Multi-threaded test using std::thread::scope. |
| **Dependencies** | None |
| **Effort** | M |

#### B4: Supporting S0 Sub-Tasks

##### B4.1: Add ensure_agent_entity() auto-creation in transact()

| Field | Value |
|-------|-------|
| **Task ID** | t-df46 |
| **Spec traces** | INV-STORE-015 (supports B3.6) |
| **Files** | `crates/braid-kernel/src/store.rs` |
| **Implementation** | Helper function `ensure_agent_entity(store, agent_id) -> Option<Vec<Datom>>` that checks if an entity with `:agent/id = agent_id` exists, and if not, produces the datoms to create one. Called from transact() before main transaction processing. |
| **Test** | Covered by B3.6 tests. |
| **Dependencies** | None |
| **Effort** | M |

##### B4.2: Implement Store::as_of using Frontier::at

| Field | Value |
|-------|-------|
| **Task ID** | t-27f5 |
| **Spec traces** | INV-STORE-005 (supports B3.8) |
| **Files** | `crates/braid-kernel/src/store.rs` |
| **Implementation** | The `as_of` method filters datoms using `Frontier::at(tx_id)` to determine which datoms are visible at a given point in time. Returns a SnapshotView (B4.3). |
| **Test** | Covered by B3.8 tests. |
| **Dependencies** | B4.3 (SnapshotView struct) |
| **Effort** | M |

##### B4.3: Implement SnapshotView struct

| Field | Value |
|-------|-------|
| **Task ID** | t-74be |
| **Spec traces** | INV-STORE-005 (supports B3.8) |
| **Files** | `crates/braid-kernel/src/store.rs` |
| **Implementation** | `SnapshotView<'a>` with `current()`, `len()`, `datoms()` methods. Holds a reference to the store and a frontier, lazily filtering datoms. |
| **Test** | Covered by B3.8 tests. |
| **Dependencies** | None |
| **Effort** | M |

##### B4.4: Add BLAKE3 verification in DiskLayout::read_tx()

| Field | Value |
|-------|-------|
| **Task ID** | t-f671 |
| **Spec traces** | INV-LAYOUT-005 (supports B3.9) |
| **Files** | `crates/braid/src/layout.rs` |
| **Implementation** | After reading file bytes, compute BLAKE3 hash. Compare against filename. Error on mismatch. |
| **Test** | Covered by B3.9 tests. |
| **Dependencies** | None |
| **Effort** | M |

##### B4.5: Implement pi_0 pinning for active intentions

| Field | Value |
|-------|-------|
| **Task ID** | t-5b3a |
| **Spec traces** | INV-SEED-006 (supports B3.1) |
| **Files** | `crates/braid-kernel/src/seed.rs` |
| **Implementation** | Active task intentions get priority slot in seed assembly, pinned at position 0. |
| **Test** | Covered by B3.1 and F3. |
| **Dependencies** | B3.1 |
| **Effort** | M |

##### B4.6: Implement intention entity querying from store

| Field | Value |
|-------|-------|
| **Task ID** | t-5910 |
| **Spec traces** | INV-SEED-006, INV-GUIDANCE-009 |
| **Files** | `crates/braid-kernel/src/seed.rs` |
| **Implementation** | Query the store for entities with `:task/status :open` to find active intentions for seed anchoring. |
| **Test** | Create tasks, query them, verify active ones are returned. |
| **Dependencies** | None |
| **Effort** | M |

##### B4.7: Implement BudgetExhaustedByIntentions signal

| Field | Value |
|-------|-------|
| **Task ID** | t-4a9e |
| **Spec traces** | INV-SIGNAL-006 (signal types) |
| **Files** | `crates/braid-kernel/src/signal.rs` |
| **Implementation** | Emit a signal when too many pinned intentions consume the seed budget. |
| **Test** | Pin many intentions, verify signal emitted when budget exceeded. |
| **Dependencies** | B4.5 (pi_0 pinning) |
| **Effort** | S |

##### B4.8: Remove total_tokens clamping -- let verify_seed detect overflows

| Field | Value |
|-------|-------|
| **Task ID** | t-7625 |
| **Spec traces** | NEG-SEED-002 |
| **Files** | `crates/braid-kernel/src/seed.rs` |
| **Implementation** | Remove the artificial clamping of total_tokens. Let the seed grow naturally and have verify_seed catch budget overflows. |
| **Test** | Verify large seed triggers verify_seed error, not silent clamping. |
| **Dependencies** | B3.2 (seed budget check fixed) |
| **Effort** | S |

##### B4.9: Fix :impl/implements to Cardinality::Many with multi resolution

| Field | Value |
|-------|-------|
| **Task ID** | t-16fe |
| **Spec traces** | INV-SCHEMA-004 |
| **Files** | `crates/braid-kernel/src/schema.rs` |
| **Implementation** | The `:impl/implements` attribute should have `Cardinality::Many` since an implementation file can implement multiple spec elements. Update the genesis schema definition. |
| **Test** | Transact multiple `:impl/implements` values for the same entity. Verify all are stored. |
| **Dependencies** | None |
| **Effort** | S |

##### B4.10: Add weight and reconciliation_type to HarvestCandidate

| Field | Value |
|-------|-------|
| **Task ID** | t-90ca |
| **Spec traces** | INV-HARVEST-003, reconciliation taxonomy |
| **Files** | `crates/braid-kernel/src/harvest.rs` |
| **Implementation** | Each HarvestCandidate should carry a weight (priority) and a reconciliation_type (which of the 8 divergence types this candidate addresses). Allows prioritized harvest and taxonomy-aware classification. |
| **Test** | Create candidates with different weights. Verify they sort correctly. Verify reconciliation_type is preserved. |
| **Dependencies** | None |
| **Effort** | S |

##### B4.11: Fix evaluator doc -- remove false semi-naive claim

| Field | Value |
|-------|-------|
| **Task ID** | t-3a78 |
| **Spec traces** | INV-QUERY-001 (docstring accuracy) |
| **Files** | `crates/braid-kernel/src/query/evaluator.rs:1` |
| **Implementation** | Remove "Semi-naive fixpoint" from the module-level doc comment. Replace with accurate description: "Single-pass sequential join evaluator for non-recursive Datalog queries." Add a note: "Semi-naive fixpoint evaluation with delta tracking is planned for Stage 1+ (see INV-QUERY-001)." |
| **Test** | Doc review. |
| **Dependencies** | None |
| **Effort** | S |

#### B5: S0 Docs/Spec

##### B5.1: Reconcile INV-MERGE-002 definition between code and spec

| Field | Value |
|-------|-------|
| **Task ID** | t-fdb0 |
| **Spec traces** | INV-MERGE-002, SH-1 |
| **Files** | `crates/braid-kernel/src/merge.rs` comments, `spec/07-merge.md` |
| **Implementation** | Update merge.rs module header and all code comments to use the spec's definition of INV-MERGE-002 (Merge Cascade Completeness), not "frontier monotonicity." |
| **Test** | Grep for INV-MERGE-002 across codebase, verify all references match spec definition. |
| **Dependencies** | A7 (Kani proof relabeled) |
| **Effort** | S |

##### B5.2: Fix verify_seed max_results from hardcoded 50

| Field | Value |
|-------|-------|
| **Task ID** | t-b7f3 |
| **Spec traces** | NEG-SEED-002 |
| **Files** | `crates/braid-kernel/src/seed.rs` |
| **Implementation** | Make max_results configurable instead of hardcoded to 50. |
| **Test** | Pass max_results=10, verify seed contains at most 10 entities. |
| **Dependencies** | None |
| **Effort** | S |

##### B5.3: Add reason String to CandidateStatus::Rejected variant

| Field | Value |
|-------|-------|
| **Task ID** | t-74ff |
| **Spec traces** | INV-HARVEST-003 |
| **Files** | `crates/braid-kernel/src/harvest.rs` |
| **Implementation** | Change `CandidateStatus::Rejected` to `CandidateStatus::Rejected { reason: String }` for debugging and audit trail. |
| **Test** | Reject a candidate, verify reason is preserved and queryable. |
| **Dependencies** | None |
| **Effort** | S |

**Wave B Summary**: 7 cascade tasks (B1.1-B1.7), 3 resolution tasks (B2.1-B2.3), 11 lifecycle tasks (B3.1-B3.11), 11 sub-tasks (B4.1-B4.11), 3 docs tasks (B5.1-B5.3). Total: 35 tasks. Critical path: B1 cascade chain.

---

### Wave C: Stage 1 Prep Infrastructure -- NOT STARTED

**Goal**: Wire existing kernel libraries into CLI, establish signal/budget/query infrastructure.
**Effort**: 3-4 agent sessions.
**Success criterion**: Budget manager wired into CLI dispatch. MCP persistent via ArcSwap. Signal system functional. Query modes operational.
**Prerequisite**: Wave B complete.

#### C1: Budget + Interface Wiring

##### C1.1: Wire BudgetManager into CLI dispatch

| Field | Value |
|-------|-------|
| **Task ID** | t-b66e |
| **Spec traces** | INV-BUDGET-001 (Hard Cap), INV-BUDGET-002 (Q(t) Measurement) |
| **Files** | `crates/braid/src/commands/mod.rs`, `crates/braid/src/main.rs` |
| **Implementation** | BudgetManager (40+ tests, 1,121 LOC in budget.rs) is fully implemented but never called from CLI dispatch. Wire it: (1) create BudgetManager at CLI startup, (2) call `allocate()` before each command, (3) call `enforce_ceiling()` on command output before rendering. This single change satisfies INV-BUDGET-001-006 at the integration level. The budget ctx already exists in main.rs but is only used for footer compression. |
| **Test** | Integration test: run braid status with a very small budget, verify output is truncated. Run with large budget, verify full output. |
| **Dependencies** | None |
| **Effort** | M |

##### C1.2: Fix MCP store reload -- implement ArcSwap persistence

| Field | Value |
|-------|-------|
| **Task ID** | t-f637 |
| **Spec traces** | INV-INTERFACE-002 (MCP Thin Wrapper / Persistent Process) |
| **Files** | `crates/braid/src/mcp.rs` -- `serve()`, all tool handlers |
| **Implementation** | Currently `tool_status` calls `layout.load_store()?` on every MCP tool call (disk I/O per call). Replace with `ArcSwap<Store>`: load store once at MCP server startup, hold in `ArcSwap`. Tool handlers read via `store.load()` (lock-free). Write operations (transact, harvest) swap atomically via `store.store(new_store)`. This provides snapshot isolation and eliminates per-call disk I/O. |
| **Test** | Start MCP server, call tool_status twice, verify the second call does not re-read from disk (measure via store identity or timing). |
| **Dependencies** | None |
| **Effort** | M |

##### C1.3: Convert remaining from_human() commands to native output

| Field | Value |
|-------|-------|
| **Task ID** | t-7761 |
| **Spec traces** | INV-INTERFACE-001 (CLI Three-Mode Output) |
| **Files** | `crates/braid/src/commands/*.rs` |
| **Implementation** | Some commands still use the `from_human()` bridge to convert human-readable text to CommandOutput. Convert each to native CommandOutput with structured JSON, AgentOutput with context/content/footer, and human text. Session 021 converted 9 commands; identify and convert any remaining. |
| **Test** | Run each command with `--mode json`, verify structured JSON output. Run with `--mode agent`, verify three-part output. |
| **Dependencies** | None |
| **Effort** | L |

##### C1.4: Add guidance footer to MCP tool responses

| Field | Value |
|-------|-------|
| **Task ID** | t-c399 |
| **Spec traces** | INV-GUIDANCE-001 (Every Tool Response Includes Footer), NEG-GUIDANCE-001 |
| **Files** | `crates/braid/src/mcp.rs` |
| **Implementation** | Call `build_command_footer()` from the MCP tool response path. This is the same function used by CLI. Append the footer to every MCP tool response text. This closes the anti-drift dead zone through MCP. Note: B3.10 addresses the same issue from the INV-INTERFACE-010 perspective. This task and B3.10 may be merged during execution. |
| **Test** | Call each MCP tool, verify response contains M(t) footer and next action. |
| **Dependencies** | B3.10 (may be same implementation) |
| **Effort** | M |

##### C1.5: Implement MCP braid_guidance tool

| Field | Value |
|-------|-------|
| **Task ID** | t-6baa |
| **Spec traces** | INV-INTERFACE-003 (Fixed Tool Count -- spec says 6 tools including braid_guidance) |
| **Files** | `crates/braid/src/mcp.rs` |
| **Implementation** | The spec defines 6 MCP tools including `braid_guidance`. The current implementation has `braid_observe` instead. Add `braid_guidance` tool that returns methodology steering: current M(t) score, recommended next action, relevant INVs, drift indicators. Keep `braid_observe` if useful, but add `braid_guidance` per spec. |
| **Test** | MCP tool definition list includes `braid_guidance`. Calling it returns structured guidance output. |
| **Dependencies** | C1.2 (ArcSwap for store access) |
| **Effort** | M |

#### C2: Signal Infrastructure

##### C2.1: Uncomment and implement SignalType variants

| Field | Value |
|-------|-------|
| **Task ID** | t-4417 |
| **Spec traces** | INV-SIGNAL-006 (Signal Subscription Completeness) |
| **Files** | `crates/braid-kernel/src/signal.rs` |
| **Implementation** | Currently 7/8 signal types are commented out. Uncomment `Contradiction`, `GoalDrift`, `QualityDecay`, `Conflict`, `SchemaViolation`, `HarvestUrgency`, `FrontierStale` signal types. For each, implement basic emit/detect logic. The dispatch function should match on all 8 variants (not just Confusion). Each signal becomes a datom when emitted. |
| **Test** | Unit test: emit each signal type, verify it becomes a datom in the store. Integration test: verify dispatch routes each type. |
| **Dependencies** | None |
| **Effort** | M |

##### C2.2: Implement signal subscription system

| Field | Value |
|-------|-------|
| **Task ID** | t-1fc1 |
| **Spec traces** | INV-SIGNAL-003 (Signal Subscription) |
| **Files** | `crates/braid-kernel/src/signal.rs` |
| **Implementation** | Implement `subscribe(signal_type, handler)` and `dispatch(signal)` pattern. Handlers can be closures or function pointers. When a signal is emitted, all subscribers for that signal type are notified. Start with synchronous dispatch (Stage 3 adds async). |
| **Test** | Subscribe to Confusion signal. Emit Confusion signal. Verify handler was called. Subscribe to GoalDrift. Emit Confusion. Verify GoalDrift handler was NOT called. |
| **Dependencies** | C2.1 (signal types exist) |
| **Effort** | M |

##### C2.3: Add target: EntityId field to Signal struct

| Field | Value |
|-------|-------|
| **Task ID** | t-0945 |
| **Spec traces** | INV-SIGNAL-001 (Signal Entity Structure) |
| **Files** | `crates/braid-kernel/src/signal.rs` |
| **Implementation** | Add `target: EntityId` to Signal struct. Signals should be directed at specific entities (the entity that triggered the signal). |
| **Test** | Emit signal with target. Verify target is preserved in the signal datom. |
| **Dependencies** | None |
| **Effort** | S |

#### C3: Query Infrastructure

##### C3.1: Add QueryMode enum and mode-stratum compatibility

| Field | Value |
|-------|-------|
| **Task ID** | t-8325 |
| **Spec traces** | INV-QUERY-005 (Query Mode Stratum Compatibility) |
| **Files** | `crates/braid-kernel/src/query/` |
| **Implementation** | Define `QueryMode { Monotonic, Stratified(Frontier), Recursive }`. Add mode parameter to evaluate(). Monotonic (current behavior). Stratified adds frontier filtering. Recursive (Stage 1+) adds fixpoint. Each mode restricts which strata are allowed. |
| **Test** | Evaluate a monotonic query in Monotonic mode (passes). Try to evaluate a non-monotonic query in Monotonic mode (error). |
| **Dependencies** | None |
| **Effort** | M |

##### C3.2: Add missing Clause AST variants -- Rules, OrClause, Frontier

| Field | Value |
|-------|-------|
| **Task ID** | t-e8d4 |
| **Spec traces** | INV-QUERY-006 (Clause Completeness) |
| **Files** | `crates/braid-kernel/src/query/` |
| **Implementation** | The Clause enum is missing 3 of 5 required variants: `Rules` (named rule definitions), `OrClause` (disjunction), `Frontier` (frontier-scoped clause). Add these variants. They can initially return errors in the evaluator (not yet implemented), but the AST must be complete so that the parser (C3.3) and programmatic query builder can construct full Datalog expressions. |
| **Test** | Construct a QueryExpr with each new variant. Verify it serializes/deserializes correctly. |
| **Dependencies** | None |
| **Effort** | M |

##### C3.3: Implement Datalog text parser

| Field | Value |
|-------|-------|
| **Task ID** | t-5faf |
| **Spec traces** | INV-QUERY-002 (Datalog Text Syntax) |
| **Files** | `crates/braid-kernel/src/query/parser.rs` (new file) |
| **Implementation** | Parse Datalog text syntax into QueryExpr AST. Currently all queries are built programmatically. A text parser enables: (1) braid query CLI with text input, (2) MCP queries in text form, (3) spec-embedded Datalog expressions. Parser should handle: find-clauses, where-clauses, variables (?x), constants, attribute keywords (:attr/name). Use nom or pest for parsing. |
| **Test** | Parse `[:find ?e :where [?e :spec/type :invariant]]` into a QueryExpr. Verify it produces the same result as the programmatic equivalent. Parse several complex queries from spec examples. |
| **Dependencies** | C3.2 (all Clause variants exist for parser to target) |
| **Effort** | L |

##### C3.4: Store frontier as :tx/frontier datom attribute

| Field | Value |
|-------|-------|
| **Task ID** | t-49e6 |
| **Spec traces** | INV-QUERY-007 (Frontier as Datom Attribute), ADR-QUERY-006 |
| **Files** | `crates/braid-kernel/src/store.rs` |
| **Implementation** | Per ADR-QUERY-006, the frontier should be stored as datoms (`:tx/frontier agent_id frontier_value`) rather than only as in-memory state. This makes the frontier queryable via Datalog and survivable across store serialization/deserialization without reconstruction. Register `:tx/frontier` in genesis schema. On every transact, emit a frontier datom. |
| **Test** | Transact 3 times. Query for `[:find ?f :where [?tx :tx/frontier ?f]]`. Verify 3 frontier datoms. |
| **Dependencies** | None |
| **Effort** | M |

#### C4: Other S1-PREP

##### C4.1: Wire R(t) routing to real store tasks

| Field | Value |
|-------|-------|
| **Task ID** | t-f2f3 |
| **Spec traces** | INV-GUIDANCE-010 (R(t) Task Routing) |
| **Files** | `crates/braid-kernel/src/guidance.rs` |
| **Implementation** | R(t) currently derives task recommendations but doesn't query real store-persisted tasks. Wire `derive_tasks()` to query entities with `:task/status :open` from the store, incorporating graph centrality metrics (PageRank, critical path) to rank them. |
| **Test** | Create tasks in store with dependencies. Verify R(t) suggests the highest-centrality unblocked task. |
| **Dependencies** | None |
| **Effort** | M |

##### C4.2: Implement commitment weight for decisions

| Field | Value |
|-------|-------|
| **Task ID** | t-d558 |
| **Spec traces** | INV-DELIBERATION-005 (Commitment Weight) |
| **Files** | `crates/braid-kernel/src/deliberation.rs` |
| **Implementation** | Each decision (crystallized deliberation) should carry a commitment weight: how many downstream artifacts depend on it. Higher weight = harder to reverse. Compute from the dependency graph. |
| **Test** | Crystallize a decision. Add 3 implementations that reference it. Verify commitment weight = 3. |
| **Dependencies** | None |
| **Effort** | M |

##### C4.3: Implement three-tier conflict routing

| Field | Value |
|-------|-------|
| **Task ID** | t-0800 |
| **Spec traces** | INV-RESOLUTION-007 (Three-Tier Routing) |
| **Files** | `crates/braid-kernel/src/resolution.rs` |
| **Implementation** | Conflicts should be routed through three tiers: (1) automatic resolution (LWW, lattice, multi-value), (2) signal emission for unresolvable conflicts, (3) deliberation for conflicts requiring human/agent judgment. The routing is based on the conflict's resolution mode and severity. Currently tier 1 exists; tier 2 emits no signal; tier 3 is a stub. Wire tiers 2 and 3. |
| **Test** | Create a conflict resolvable by LWW (tier 1). Create an incomparable lattice conflict (tier 2 -- signal emitted). Create a conflict requiring deliberation (tier 3 -- deliberation entity created). |
| **Dependencies** | C2.1 (signals for tier 2), B3.5 (deliberation for tier 3) |
| **Effort** | M |

**Wave C Summary**: 5 budget/interface tasks (C1.1-C1.5), 3 signal tasks (C2.1-C2.3), 4 query tasks (C3.1-C3.4), 3 prep tasks (C4.1-C4.3). Total: 15 tasks. Critical path: C1.1 (budget wiring) and C3.3 (Datalog parser, L-sized).

---

### Wave D: Query Engine Maturity -- NOT STARTED

**Goal**: Full query capabilities, graph algorithms integrated with Store, signal pipeline operational.
**Effort**: 3-4 agent sessions.
**Success criterion**: Graph algorithms query Store directly (not DiGraph). VAET/AVET indexes implemented. QueryResult aligned with spec. Access log operational.
**Prerequisite**: Wave C complete (QueryMode, AST variants, signal infra).

| # | Task ID | Title | Spec Traces | Files | Implementation | Dependencies | Effort |
|---|---------|-------|-------------|-------|---------------|--------------|--------|
| D1 | t-ae29 | Implement graph algorithm Store integration | INV-QUERY-012 | query/graph.rs, store.rs | Graph algorithms currently operate on `DiGraph`, not directly on Store. Implement `extract_subgraph(store, entity_type, dep_attr) -> DiGraph` (D6) and then wrap each algorithm with a Store-aware entry point: `pagerank_from_store(store, entity_type, dep_attr)`, `scc_from_store(...)`, etc. | D6 | M |
| D2 | t-5a39 | Implement HITS and k-Core algorithms | INV-QUERY-015, INV-QUERY-016 | query/graph.rs | HITS: iterative hub/authority scoring. k-Core: iterative shell removal. Both operate on DiGraph. Add proptest for convergence. | None | M |
| D3 | t-64be | Full critical path with slack/earliest/latest | INV-QUERY-017 | query/graph.rs | Current critical_path returns only the path. Add `CriticalPathResult { path, earliest_start, latest_start, slack }` for each node. | None | M |
| D4 | t-d557 | Add PageRankConfig with configurable parameters | INV-QUERY-014 | query/graph.rs | Make damping (default 0.85), epsilon (default 1e-6), and max_iterations (default 100) configurable via `PageRankConfig` struct. | None | S |
| D5 | t-4569 | Implement GraphDensityMetrics struct | INV-QUERY-021 | query/graph.rs | 6 fields: nodes, edges, density, avg_degree, max_degree, components. | None | S |
| D6 | t-eafd | Implement extract_subgraph | INV-QUERY-012 | query/graph.rs, store.rs | `extract_subgraph(store, entity_type_attr, entity_type_val, dep_attr) -> DiGraph`. Query store for all entities matching type, build edges from dep_attr references. | C3.4 (frontier as datom helps) | M |
| D7 | t-c12b | Extend scc() to return SCCResult | INV-QUERY-017 | query/graph.rs | `SCCResult { components, condensation_dag, has_cycles }`. Condensation DAG is a new DiGraph where each SCC is a single node. | None | M |
| D8 | t-8ef2 | Align QueryResult with spec | INV-QUERY-005 | query/ | Add `bindings: Vec<Binding>`, `stratum: usize`, `mode: QueryMode`, `provenance_tx: TxId` fields to QueryResult per spec. | C3.1 (QueryMode) | M |
| D9 | t-cec8 | Add Collection and Tuple to FindSpec | INV-QUERY-006 | query/ | FindSpec currently has `Scalar` only. Add `Collection` (Vec of results) and `Tuple` (single row) variants. | None | S |
| D10 | t-b764 | Implement access log + Hebbian significance | INV-QUERY-008, INV-QUERY-009 | query/, store.rs | Record every query's accessed entities. Track access frequency. Hebbian rule: entities accessed together strengthen mutual significance. Store significance as datoms. | C3.4 | L |
| D11 | t-03cf | Implement VAET and AVET indexes | INV-STORE-004 | store.rs | VAET: `BTreeMap<(Value, Attribute, EntityId, TxId)>` for reverse-reference lookups. AVET: `BTreeMap<(Attribute, Value, EntityId, TxId)>` for unique/range queries. Maintain on transact and merge. | None | L |
| D12 | t-5b1e | Implement AVET index | INV-STORE-004 | store.rs | Sub-task of D11. `BTreeMap<Attribute, BTreeMap<Value, Vec<Datom>>>`. | None | M |
| D13 | t-9ff3 | Implement VAET index | INV-STORE-004 | store.rs | Sub-task of D11. `BTreeMap<Value, BTreeMap<Attribute, Vec<Datom>>>` (for reference values). | None | M |

**Wave D Summary**: 13 tasks. D11 is the L-sized work item. D2-D5 and D7, D9 are independent and parallelizable.

---

### Wave E: Spec Hygiene -- NOT STARTED

**Goal**: Specification-implementation alignment, cross-reference completeness, docstring accuracy.
**Effort**: 2-3 agent sessions. Can run in parallel with Waves D/F.
**Prerequisite**: Wave A complete (false witnesses removed).

#### E1: High Priority

| # | Task ID | Title | SH Ref | Spec Traces | Files | Effort |
|---|---------|-------|--------|-------------|-------|--------|
| E1.1 | t-fdb0 | Reconcile INV-MERGE-002 between code and spec | SH-1 | INV-MERGE-002 | merge.rs, spec/07-merge.md | S |
| E1.2 | t-645d | Add SYNC stage clarity note | SH-3 | INV-SYNC-* | spec/08-sync.md | S |
| E1.3 | t-3c97 | Fix guide type catalog CascadeReceipt | SH-4 | INV-MERGE-002 | docs/guide/types.md | S |
| E1.4 | t-b5da | Add witness quality column to verification | SH-6 | INV-TRILATERAL-001 | spec/16-verification.md | M |
| E1.5 | t-4be5 | Fix INV-MERGE-009/010 ID swap | -- | INV-MERGE-009, INV-MERGE-010 | merge.rs, spec/07-merge.md | S |
| E1.6 | t-cb07 | Reconcile INV-QUERY numbers | -- | INV-QUERY-* | query/*.rs, spec/03-query.md | S |

#### E2: Medium Priority

| # | Task ID | Title | Files | Effort |
|---|---------|-------|-------|--------|
| E2.1 | t-fdf1 | Catalogue 47 excluded elements in crossref | spec/17-crossref.md | M |
| E2.2 | t-7699 | Add TOPOLOGY/COHERENCE to crossref | spec/17-crossref.md | S |
| E2.3 | t-0470 | Add TOPOLOGY/COHERENCE to spec README | spec/README.md | S |
| E2.4 | t-dfc7 | Add all 22 namespaces to preamble | spec/00-preamble.md | S |
| E2.5 | t-b16c | Add TOPOLOGY/COHERENCE ADRs to ADRS.md | docs/design/ADRS.md | S |
| E2.6 | t-61f7 | Fix TRILATERAL count 7->10 | spec/17-crossref.md | S |
| E2.7 | t-6623 | Fix merge_stores docstring cascade steps | merge.rs | S |
| E2.8 | t-aa96 | Reconcile drift_score semantics | spec, bilateral.rs | S |
| E2.9 | t-b42a | Align SEED.md thresholds with Q(t) | SEED.md, spec | S |
| E2.10 | t-0f32 | Document attention_decay continuity fix as ADR | docs/design/ADRS.md | S |

#### E3: Low Priority

| # | Task ID | Title | Files | Effort |
|---|---------|-------|-------|--------|
| E3.1 | t-420f | Add Stage 3 deferral note to TOPOLOGY | spec/ | S |
| E3.2 | t-f18f | Align SEED.md Stage 0 with 0a/0b sub-staging | SEED.md | S |
| E3.3 | t-051f | Remove INV-STORE-013 from transact() doc | store.rs | S |
| E3.4 | t-050e | Rename from_datoms -> from_store in spec/guide | spec, guide | S |
| E3.5 | t-a0f6 | Update guide persistence.rs reference | guide | S |
| E3.6 | t-e8be | Fix merge.rs INV-MERGE-008 docstring | merge.rs | S |
| E3.7 | t-cc9b | Fix ADR-MERGE-004 docstring | merge.rs | S |
| E3.8 | t-1396 | Document Clause::Predicate in query spec | spec/03-query.md | S |
| E3.9 | t-3e1f | Update seed spec to reference agent_md | spec/06-seed.md | S |
| E3.10 | t-dcb2 | Reconcile SeedOutput struct | spec, guide | S |
| E3.11 | t-d380 | Add INV-SCHEMA-009 to guide checklist | guide | S |
| E3.12 | t-3f0f | Align guide confidence ranges with Fisher-Rao | guide | S |
| E3.13 | t-3abc | Document Rust merge adaptation | guide, spec | S |

**Wave E Summary**: 29 tasks. All S-sized except E1.4 and E2.1 (M). No code dependencies -- pure documentation. Can be parallelized across agents or batched in a single docs session.

---

### Wave F: Verification Coverage -- NOT STARTED

**Goal**: Close the 31.3% untested INV gap. Fix generated tests. Add missing verification tiers (T3 fuzzing, T4 MIRI). Achieve 80%+ INV coverage.
**Effort**: 2-3 agent sessions. Can run in parallel with Waves D/E.
**Prerequisite**: Wave A items A8-A11 complete (coherence compiler fixed).

| # | Task ID | Title | Spec Traces | Files | Implementation | Dependencies | Effort |
|---|---------|-------|-------------|-------|---------------|--------------|--------|
| F1 | t-6ee6 | INV-SEED-005 + INV-SEED-006 integration test | INV-SEED-005, INV-SEED-006 | tests/ | Create store with 10+ spec elements and 5+ constraints. Call assemble_seed(). Verify: (a) demonstrations embedded for constraint clusters >= 2, (b) task intention in Directive section. | B3.1 (intention anchoring) | M |
| F2 | t-9f78 | INV-SEED-005 demonstration density test | INV-SEED-005 | tests/ | Verify seed output includes worked examples for constraint clusters. Currently the output never includes examples. | B3.1 | M |
| F3 | t-3faf | INV-SEED-006 intention anchoring test | INV-SEED-006 | tests/ | Call assemble_seed with task string. Verify task appears in Directive. Verify task-relevant entities rank higher. | B3.1 | M |
| F4 | t-a267 | INV-INTERFACE-002 MCP thin wrapper test | INV-INTERFACE-002 | tests/ | Start MCP server, call each tool, verify response structure matches spec. Verify store is NOT reloaded per call after C1.2. | C1.2 | M |
| F5 | t-7ea3 | INV-INTERFACE-005 TUI subscription liveness | INV-INTERFACE-005 | tests/ | Stage 4 test stub. Minimal: verify SignalType subscription fires for TUI events. | C2.2 | S |
| F6 | t-d9d5 | INV-INTERFACE-006 human signal injection | INV-INTERFACE-006 | tests/ | Stage 3 test stub. Minimal: verify external signal injection path exists. | C2.1 | S |
| F7 | t-4713 | Commit proptest regression files to VCS | -- | .proptest-regressions/ | Commit all `.proptest-regressions` files so regressions are not lost across checkouts. | None | S |
| F8 | t-1890 | Proptest for canonical serialization order independence | INV-LAYOUT-011 | tests/ | Generate same datoms in two random orders. Serialize both. Assert identical hashes. | A1 (sort fix) | M |
| F9 | t-27e0 | Store::merge associativity proptest | INV-MERGE-001, INV-STORE-004 | tests/ | `merge(merge(A, B), C) == merge(A, merge(B, C))` for random stores. | None | M |
| F10 | t-e0b0 | Fuzz target for EDN deserializer (T3) | T3 coverage | fuzz/ (new dir) | Create a `cargo fuzz` target for the EDN parser. Feed random bytes, verify no panics. This fills the T3 gap. | None | M |
| F11 | t-f74e | MIRI CI job (T4) | T4 coverage | .github/workflows/ | Add a MIRI job to CI. The codebase uses `#![forbid(unsafe_code)]` so this is lower risk, but MIRI catches UB in dependencies too. | None | M |
| F12 | t-3aef | SYNC namespace integration tests | INV-SYNC-* | tests/ | Stage 3 content. Minimal: create two agents with divergent frontiers. Test that frontier comparison works. Do NOT claim to witness full SYNC invariants (they require barrier semantics not yet implemented). | C2.1 | L |

**Wave F Summary**: 12 tasks. F7 and F8-F9 are quick wins. F10-F11 add missing verification tiers. F12 is L-sized (Stage 3 content preview).

---

### Wave G: Stage 1 Full Implementation -- NOT STARTED

**Goal**: Full Stage 1 functionality: budget-aware output, guidance injection, bilateral loop, lattice resolution, divergence detection for 5/8 types.
**Effort**: 6-8 agent sessions.
**Prerequisite**: Waves B + C complete. Can overlap with Waves D/E/F.

#### G1: Lattice Resolution Pipeline (addresses Drift 3: Lattice Resolution Collapse)

| # | Task ID | Title | Spec Traces | Files | Implementation | Dependencies | Effort |
|---|---------|-------|-------------|-------|---------------|--------------|--------|
| G1.1 | t-da96 | LatticeDef struct with join/bottom/top | INV-RESOLUTION-006, INV-SCHEMA-007 | resolution.rs | Define `LatticeDef { elements: Vec<Value>, join: HashMap<(Value,Value), Value>, bottom: Value, top: Value }`. Stored as datoms with `:lattice/*` attributes. | A5 (lattice_id) | M |
| G1.2 | t-1dfe | lattice_def() method on Schema | INV-SCHEMA-007 | schema.rs | `Schema::lattice_def(lattice_id: EntityId) -> Option<LatticeDef>`. Reads from `:lattice/*` datoms in store. | G1.1 | S |
| G1.3 | t-194c | lattice lub() computation in resolve() | INV-RESOLUTION-006 | resolution.rs | `lub(lattice: &LatticeDef, a: &Value, b: &Value) -> Result<Value, LatticeError>`. Look up join(a, b) in the lattice definition. Return LatticeError::Incomparable if not in join table. | G1.2 | M |
| G1.4 | t-72f2 | Replace LWW fallback with real lattice resolution | INV-RESOLUTION-006 | resolution.rs:159 | Remove the "Stage 0: lattice resolution falls back to LWW" line. Replace with call to lub(). If lattice_def lookup fails, return an explicit error (not silent LWW). | G1.3 | M |
| G1.5 | t-f08b | Diamond lattice error signal for incomparable values | INV-SIGNAL-005 | signal.rs, resolution.rs | When lub() returns Incomparable, emit a `SignalType::Conflict` with details about the incomparable values. This triggers three-tier routing (C4.3). | G1.4, C2.1 | M |
| G1.6 | t-d946 | Discrete Severity lattice | INV-SCHEMA-007 | schema.rs | Implement the Severity lattice (Low < Medium < High < Critical) as datoms. This is the first concrete lattice, serving as both a useful type and a demonstration. | G1.1 | S |
| G1.7 | t-194f | Fix Schema::from_datoms for :lattice/* namespace | INV-SCHEMA-007 | schema.rs | Ensure Schema::from_datoms extracts lattice definitions from `:lattice/*` datoms. Currently may skip them. | G1.1 | S |
| G1.8 | t-d684 | LwwClock per-attribute clock selection | INV-RESOLUTION-006 | resolution.rs | Allow different LWW attributes to use different clock sources (wall clock, HLC, vector clock). Per-attribute configuration. | None | M |

**G1 chain**: G1.1 -> G1.2 -> G1.3 -> G1.4 -> G1.5. G1.6, G1.7 independent. G1.8 independent.

#### G2: Budget + Guidance + Interface

| # | Task ID | Title | Spec Traces | Files | Dependencies | Effort |
|---|---------|-------|-------------|-------|--------------|--------|
| G2.1 | t-c016 | M(t) fifth component -- guidance_compliance | INV-GUIDANCE-008 | guidance.rs | C1.1 | M |
| G2.2 | t-55da | Dynamic CLAUDE.md typestate pipeline | INV-GUIDANCE-007 | agent_md.rs | C1.1 | L |
| G2.3 | t-54a0 | GuidanceTopology comonadic structure | ADR-GUIDANCE-001 | guidance.rs | C1.1, C3.3 | L |
| G2.4 | t-e7cd | Four-part guidance footer structure | INV-GUIDANCE-001 | guidance.rs | C1.4 | M |
| G2.5 | t-d568 | T(t) topology fitness metric | INV-GUIDANCE-011 | guidance.rs | G2.3 | M |
| G2.6 | t-98e2 | Error RecoveryAction enum | INV-INTERFACE-009 | commands/ | None | M |
| G2.7 | t-bf74 | Convert task commands to CommandOutput | INV-INTERFACE-001 | commands/ | C1.3 | M |
| G2.8 | t-a0df | MCP task management tools | INV-INTERFACE-003 | mcp.rs | C1.2 | M |
| G2.9 | t-c60f | Layer 4.5 statusline bridge | INV-INTERFACE-004 | commands/ | C1.1 | M |
| G2.10 | t-6ffd | Statusline bridge | INV-INTERFACE-004 | commands/ | G2.9 | M |
| G2.11 | t-f249 | Q(t)-based harvest heuristic in status | INV-BUDGET-002 | commands/status.rs | C1.1 | M |
| G2.12 | t-0623 | Proactive harvest warning | INV-INTERFACE-007 | commands/ | C1.1, C2.1 | M |
| G2.13 | t-203d | Persist derived tasks as datoms | INV-GUIDANCE-009 | guidance.rs | C4.1 | M |

#### G3: Bilateral + Trilateral + Harvest/Seed

| # | Task ID | Title | Spec Traces | Files | Dependencies | Effort |
|---|---------|-------|-------------|-------|--------------|--------|
| G3.1 | t-ceaf | Bilateral Boundary four-variant enum | INV-BILATERAL-003 | bilateral.rs | None | S |
| G3.2 | t-792a | Bilateral scan all 4 boundaries | INV-BILATERAL-001 | bilateral.rs | G3.1 | M |
| G3.3 | t-0c30 | CC-3 staleness detection | INV-BILATERAL-004 | bilateral.rs | G3.2 | M |
| G3.4 | t-e6d7 | Session termination detection | NEG-HARVEST-001 | harvest.rs | C2.1 | M |
| G3.5 | t-0df4 | Harvest FP/FN calibration infrastructure | INV-HARVEST-004, INV-HARVEST-006 | harvest.rs | None | M |
| G3.6 | t-78db | Wire calibrate_harvest() into pipeline | INV-HARVEST-004 | harvest.rs | G3.5 | M |
| G3.7 | t-1a5c | Observation staleness tracking | -- | harvest.rs | None | M |
| G3.8 | t-ebcc | Seed relevance/improvement tracking | INV-SEED-007, INV-SEED-008 | seed.rs | None | M |
| G3.9 | t-7aaa | Constraint cluster detection in seed | INV-SEED-005 | seed.rs | None | M |
| G3.10 | t-8c2c | 7-step session lifecycle state machine | INV-HARVEST-007, NEG-HARVEST-001 | session.rs (new) | G3.4 | L |

#### G4: Conflict + Deliberation + Resolution

| # | Task ID | Title | Spec Traces | Files | Dependencies | Effort |
|---|---------|-------|-------------|-------|--------------|--------|
| G4.1 | t-bbc1 | Store-wide detect_conflicts() | INV-RESOLUTION-004 | resolution.rs | B2.1 | M |
| G4.2 | t-3754 | Full conflict lifecycle pipeline | INV-RESOLUTION-008 | resolution.rs | G4.1, B2.2 | L |
| G4.3 | t-8c21 | Wire merge to deliberation dispatch | -- | merge.rs, deliberation.rs | B3.5, C2.1 | M |
| G4.4 | t-bec2 | INV-MERGE-009 cascade logic | INV-MERGE-009 | merge.rs | B1.6 | M |
| G4.5 | t-c1e1 | Frontier durability | INV-STORE-009 | store.rs, layout.rs | None | M |
| G4.6 | t-6fd9 | LIVE index as materialized view | INV-STORE-012 | store.rs | D11 (VAET/AVET) | L |
| G4.7 | t-d124 | Read command provenance recording | INV-STORE-014 | store.rs | C3.4 | M |
| G4.8 | t-841c | SchemaValidationError enum | INV-SCHEMA-004 | schema.rs | None | M |
| G4.9 | t-4f59 | Schema::validate_layer_ordering() | INV-SCHEMA-006 | schema.rs | G4.8 | M |
| G4.10 | t-20b8 | Schema::new_attribute() public API | INV-SCHEMA-004 | schema.rs | G4.8 | M |
| G4.11 | t-2f59 | Schema validation for cardinality/retraction | INV-SCHEMA-004 | schema.rs | G4.8 | M |

#### G5: Divergence Detection (META tasks)

| # | Task ID | Title | META | Spec Traces | Dependencies | Effort |
|---|---------|-------|------|-------------|--------------|--------|
| G5.1 | t-b547 | META: Reconciliation Taxonomy Inversion | META-1 | All 8 divergence types | C2.1, G4.1 | L |
| G5.2 | t-1cea | Detection for 6 missing taxonomy types | META-1 | INV-TRILATERAL-003 | G5.1 | L |
| G5.3 | t-d2d6 | META: Datalog Promise | META-2 | FD-003, INV-QUERY-001 | C3.3 | L |
| G5.4 | t-dd14 | Semi-naive fixpoint with delta tracking | META-2 | INV-QUERY-001 | G5.3 | L |
| G5.5 | t-adff | META: Lattice Resolution Collapse | META-3 | FD-005, INV-RESOLUTION-006 | G1.4 | L |
| G5.6 | t-f2b1 | META: Axiological Self-Blindness | META-4 | SEED.md Section 6 | G5.10, G3.2 | L |
| G5.7 | t-3171 | Contradiction detection Tiers 3-5 | INV-TRILATERAL-003 | coherence.rs | G5.1 | L |
| G5.8 | t-49ae | Consequential divergence detection | SEED.md Section 6 | signal.rs | C2.1 | M |
| G5.9 | t-1417 | Aleatory divergence detection | SEED.md Section 6 | signal.rs | C2.1, G4.3 | M |
| G5.10 | t-ff0c | Axiological divergence detection | SEED.md Section 6 | signal.rs, bilateral.rs | C2.1, G3.2 | M |

#### G6: Failure Mode Mitigations

| # | Task ID | Title | FM | Spec Traces | Dependencies | Effort |
|---|---------|-------|-----|-------------|--------------|--------|
| G6.1 | t-a0cf | FM-002 provenance structural audit | FM-002 | INV-STORE-014 | None | M |
| G6.2 | t-25bc | FM-006 per-projection frontier staleness | FM-006 | INV-STORE-009 | G4.5 | M |
| G6.3 | t-8063 | FM-007 bilateral scan on ADR changes | FM-007 | INV-BILATERAL-001 | G3.2 | M |
| G6.4 | t-640e | FM-009 semantic contradiction detection | FM-009 | INV-TRILATERAL-003 | G5.7 | M |
| G6.5 | t-4d88 | FM-010 NEG-vs-ADR scope overlap detection | FM-010 | -- | G5.7 | M |
| G6.6 | t-54a9 | FM-011 verification matrix reconciliation | FM-011 | -- | G3.2 | M |
| G6.7 | t-5d4e | FM-012 cross-document type verification | FM-012 | -- | G3.2 | M |
| G6.8 | t-09cc | FM-013 phantom type verification | FM-013 | INV-SCHEMA-004 | G4.8 | M |
| G6.9 | t-4af1 | FM-020 DECIDE/EXPLORE classification | FM-020 | -- | G5.6 | M |

#### G7: Miscellaneous S1

| # | Task ID | Title | Effort |
|---|---------|-------|--------|
| G7.1 | t-2e60 | Fix task ID collision risk -- expand from 4 hex to 8 | S |

**Wave G Summary**: 8 lattice tasks (G1), 13 budget/guidance tasks (G2), 10 bilateral/harvest tasks (G3), 11 conflict/resolution tasks (G4), 10 divergence detection tasks (G5), 9 failure mode tasks (G6), 1 misc (G7). Total: 62 tasks.

---

### Wave H: Stage 2 (Deferred)

These tasks are explicitly Stage 2+ and should NOT be started until Stage 1 is substantially complete.

| # | Task ID | Title | Stage |
|---|---------|-------|-------|
| H1 | t-50ea | Temporal divergence detection (cross-agent frontier comparison) | S2 |
| H2 | t-69b7 | Align branch.rs with Branching G-Set model | S2 |
| H3 | t-b54b | Coherence density matrix with agreement functions | S2 |
| H4 | t-3c9a | In-engine aggregation for Stratum 3 | S2 |
| H5 | t-dd14 | Semi-naive fixpoint with delta tracking (if not completed in G5.4) | S2 |

---

## 4. Dependency Graph

### 4.1 Critical Path

The longest dependency chain determines the minimum time to Stage 1 completion:

```
A4 (register attrs) -> A6 (causal independence) -> B1.3 (cascade step 1)
  -> B1.4 (cascade steps 2-5) -> B1.5 (wire return type)
  -> B1.6 (full cascade integration) -> B1.7 (determinism)
  -> G4.4 (INV-MERGE-009 cascade logic)
  -> G5.1 (taxonomy detection)
  -> G5.2 (6 missing types)
```

This chain is 10 tasks deep. At ~1 task per hour average, this is 10-15 hours of sequential work minimum. Everything else can be parallelized around it.

### 4.2 Key Dependency Chains

**Chain 1: Merge Cascade** (the critical path)
```
A12 (MergeReceipt fix) -> B1.1 (CascadeReceipt) -> B1.3 (step 1) -> B1.4 (steps 2-5) -> B1.5 (wire) -> B1.6 (integrate) -> B1.7 (determinism) -> G4.4 (S1 cascade logic)
```
*Why this ordering*: Each step depends on the struct/function from the prior step. The cascade must exist before it can be extended.

**Chain 2: Lattice Resolution**
```
A5 (lattice_id field) -> G1.1 (LatticeDef) -> G1.2 (Schema method) -> G1.3 (lub computation) -> G1.4 (replace LWW fallback) -> G1.5 (error signal)
```
*Why this ordering*: The lattice_id field must exist in the enum before LatticeDef can be looked up. lub() must exist before resolve() can call it. The signal requires the error path.

**Chain 3: Signal Pipeline**
```
C2.1 (uncomment types) -> C2.2 (subscription) -> G5.8-G5.10 (divergence detection) -> G5.1 (taxonomy completeness)
```
*Why this ordering*: Signal types must exist before subscription can route them. Divergence detection emits signals that must be dispatchable.

**Chain 4: Query Engine**
```
C3.1 (QueryMode) -> C3.2 (AST variants) -> C3.3 (text parser) -> G5.3 (Datalog promise) -> G5.4 (semi-naive fixpoint)
```
*Why this ordering*: The parser targets the AST. The AST needs all variant types. Semi-naive evaluation requires the parser to express recursive queries.

**Chain 5: Budget Integration**
```
C1.1 (wire budget to CLI) -> G2.11 (Q(t)-based harvest heuristic) -> G2.12 (proactive warning)
```
*Why this ordering*: Budget measurement must be active before Q(t) can drive heuristics.

**Chain 6: Bilateral Completeness**
```
G3.1 (4-variant Boundary) -> G3.2 (scan all 4) -> G3.3 (CC-3 staleness) -> G5.6 (axiological self-blindness)
```
*Why this ordering*: The bilateral scan must cover all boundaries before staleness detection can operate on them. Axiological self-blindness detection requires the bilateral scan to report axiological drift.

**Chain 7: Deliberation Pipeline**
```
B3.4 (remove Ord) -> B3.5 (stability guard) -> G4.3 (merge-to-deliberation) -> C4.3 (three-tier routing)
```
*Why this ordering*: Explicit transitions (no Ord) before guards. Guards before dispatch. Dispatch before routing.

### 4.3 Parallelization Opportunities

**Within Wave A**: A1, A2, A3, A4, A5, A12 are all independent. Assign to separate agents or execute in rapid succession.

**Within Wave B**: B1 (cascade), B2 (resolution), B3 (lifecycle), B4 (sub-tasks), B5 (docs) are largely independent tracks. B2 and B3 can run in parallel with B1. B5 can run in parallel with everything.

**Waves D, E, F are parallel with each other**: D (query), E (spec hygiene), F (verification) have no cross-dependencies. Three agents can work simultaneously.

**Waves D/E/F parallel with early G tasks**: G1 (lattice) depends only on A5 (Wave A). G3.1 (boundary enum) has no dependencies. These can start as soon as their Wave A/B prerequisites are met, without waiting for all of D/E/F.

**Wave E is fully parallelizable**: All 29 spec hygiene tasks are independent documentation changes. Multiple agents can work these simultaneously.

**G sub-waves are largely parallel**: G1 (lattice), G2 (budget/guidance), G3 (bilateral/harvest), G4 (conflict/resolution), G6 (FM mitigations) depend on different Wave C outputs and can run concurrently. G5 (divergence detection) depends on several other G sub-waves.

---

## 5. Success Criteria

### 5.1 Stage 0 Close Criteria

All of the following must be true to declare Stage 0 complete:

1. **CI is green**: `cargo check --all-targets` and `cargo test --all-targets` both pass with 0 failures. No compilation errors, no test failures.

2. **All P0 bugs resolved**: The remaining 7 P0 tasks from Wave A are closed (A1-A7, A12). All 5 Phase 0 fixes verified.

3. **All S0-CLOSE tasks resolved**: All 24 S0-CLOSE stage tasks closed, plus 10 S0-SUB supporting tasks.

4. **INV coverage >= 85%**: At least 71 of 83 Stage 0 INVs meaningfully implemented (currently 62/83 = 75%). "Meaningfully implemented" means the code satisfies the invariant's stated property -- not just annotated as such.

5. **No false witnesses**: All SYNC witness claims removed from merge.rs. Kani proof INV-MERGE-002 correctly labeled. Generated coherence tests compile and are semantically meaningful (already achieved in Phase 0). Zero instances where code claims to verify an INV but actually verifies a different property.

6. **F(S) >= 0.85**: `braid bilateral` reports fitness function at or above 0.85 for Stage 0 namespaces. All 7 F(S) weights remain at spec values (V=0.18, C=0.18, D=0.18, H=0.13, K=0.13, I=0.08, U=0.12).

7. **Merge cascade functional**: INV-MERGE-002 implemented with CascadeReceipt producing datoms for each of 5 cascade steps. ADR-MERGE-007 stub pattern used for steps 2-5 at Stage 0. Step 1 (conflict detection) is real.

8. **Harvest/seed round-trip validated**: Successfully work 25+ turns, harvest, start fresh with seed, new session picks up without manual re-explanation. Multi-session continuity at least shows last 2-3 sessions with diminishing detail for earlier sessions.

### 5.2 Stage 1 Complete Criteria

1. **All 26 Stage 1 INVs addressed**: BUDGET-001-006, GUIDANCE-003-004, BILATERAL-001-002/004-005, INTERFACE-004/007, QUERY-003/008-009/015-016/018, SIGNAL-002, HARVEST-004/006, SEED-007-008, TRILATERAL-004. Each either fully implemented or documented with a scope limitation rationale.

2. **Budget-aware output operational**: Every CLI command output passes through `enforce_ceiling()` with mode-aware truncation. Q(t) is measured continuously. Output precedence ordering is functional. BudgetManager integrated into CLI dispatch loop.

3. **Guidance injection in MCP**: MCP tool responses include methodology footer with M(t) score, next action, and drift indicators. No guidance-free dead zone in any interface path.

4. **Lattice resolution functional**: `resolve()` uses actual `lub()` computation, not LWW fallback. At least one concrete lattice (Severity: Low/Medium/High/Critical) operational. Diamond lattice error signal emits on incomparable values.

5. **Signal system active**: At least 4 of 8 signal types functional with working dispatch and subscription: Confusion, Contradiction, GoalDrift, and one domain signal (Conflict or HarvestUrgency).

6. **Divergence detection for 5+ types**: At least 5 of 8 divergence types have functional detection mechanisms: Epistemic (harvest gap), Structural (bilateral scan), Procedural (M(t) + access log), plus at least 2 of {Consequential, Aleatory, Axiological, Logical}. Temporal is deferred to Stage 3.

7. **Bilateral F(S) >= 0.90**: `braid bilateral` reports fitness function at or above 0.90 including Stage 1 namespaces. F(S) is monotonically improving across sessions.

8. **Test coverage >= 80%**: At least 130 of 163 INVs have dedicated tests. No false witnesses.

9. **All 4 META axiological gaps remediated**: Reconciliation Taxonomy Inversion (META-1), Datalog Promise (META-2), Lattice Resolution Collapse (META-3), and Axiological Self-Blindness (META-4) each have either a full implementation or a documented scope limitation with a plan for the next stage.

### 5.3 Verification Criteria (Zero-Defect Target)

| Tier | Requirement | Current | Target |
|------|-------------|---------|--------|
| T0 | `cargo clippy --all-targets -- -D warnings` clean | GREEN | GREEN |
| T1 | All unit tests passing | 972 pass | 1,100+ pass |
| T2 | All proptests passing | 143 blocks | 160+ blocks |
| T3 | Fuzz targets exist for EDN parser | 0 targets | >= 1 target |
| T4 | MIRI CI job configured | Not configured | Configured (if applicable given `forbid(unsafe_code)`) |
| T5 | All Kani harnesses passing with correct INV targets | 36 harnesses, 1 mislabeled | 36+ harnesses, 0 mislabeled |
| T6 | Stateright models passing | 11 tests | 11+ tests |
| T7 | Generated coherence tests compile and meaningful | 127 passing | 127+ passing, 0 tautological |
| -- | No false witnesses in annotation scan | 6 remaining | 0 |
| -- | All integration tests passing | 11 pass, 0 fail | 15+ pass, 0 fail |

---

## 6. Risk Register

| # | Risk | Probability | Impact | Mitigation |
|---|------|-------------|--------|------------|
| R1 | **Merge cascade complexity exceeds estimate.** The 5-step cascade is the single largest unimplemented feature. Step 1 (conflict detection) requires BFS walk, causal independence check, and datom generation. Steps 2-5 are stubs at Stage 0 but must be designed for real logic at Stage 1. | Medium | High | ADR-MERGE-007 explicitly authorizes stub datoms at Stage 0. Implement step 1 fully, steps 2-5 as stubs. This bounds the M-sized effort. If cascade takes longer, reduce scope to steps 1+2 and defer 3-5 entirely. |
| R2 | **Datalog text parser (C3.3) is under-estimated.** Parsing Datalog with variables, constants, keywords, rules, aggregates, and negation is a non-trivial language task. The L estimate may not be sufficient. | Medium | Medium | Start with a minimal parser (find-clauses and where-clauses only). Rules, OrClause, and aggregation can be deferred to a second parser iteration. Use nom for structured parsing, not regex. |
| R3 | **Schema changes cascade into test failures.** Several Wave A/B tasks modify the genesis schema (A4, A5, B4.9). Each schema change propagates to all tests that construct stores with genesis(). | Medium | Medium | Batch schema changes into a single session. Update `GENESIS_ATTR_COUNT` and `LAYER_1_COUNT` constants once. Run `cargo test` after each schema change to catch failures immediately. |
| R4 | **Semi-naive fixpoint evaluation (G5.4) is architecturally disruptive.** Replacing the evaluator's core algorithm from single-pass join to delta-tracking fixpoint may require restructuring the entire query module. | Medium | High | Implement fixpoint as a separate evaluator path (`evaluate_recursive`) rather than replacing `evaluate`. Monotonic queries continue to use the fast single-pass path. Recursive queries use the new path. Feature-flag if needed. |
| R5 | **False witness patterns recur.** The audit found 83 false witnesses. New code may introduce new ones, especially when agents add witness annotations to partially-implemented features. | Low | Medium | Establish a witness validation rule: an annotation claiming `INV-X` must have a test that specifically exercises the falsification condition of INV-X. Add a CI check that cross-references annotations against test presence. |

---

## 7. Session Planning Guide

### 7.1 Task Management Workflow

Use the `braid` CLI for task management during sessions:

```bash
# At session start: see what's ready
braid status                    # Dashboard with F(S), M(t), tasks, next action

# During work: observe decisions and questions
braid observe "Merge cascade step 1 uses BFS walk for causal ancestors" --category design-decision --confidence 0.9

# At session end: harvest and refresh
braid harvest --commit          # Extract session knowledge into store
braid seed --inject AGENTS.md   # Refresh AGENTS.md braid-seed section for next session
```

### 7.2 Tasks Per Session

A single agent session can typically handle:

| Wave | Tasks per session | Rationale |
|------|-------------------|-----------|
| Wave A | 4-6 S-sized tasks | Quick fixes, independent, high-value |
| Wave B | 2-3 M-sized tasks or 1 L-sized task | Sequential cascade chain limits parallelism |
| Wave C | 2-3 tasks | Integration work requires careful testing |
| Wave D | 3-4 tasks | Algorithm implementation is focused work |
| Wave E | 8-12 S-sized tasks | Documentation changes, fast, parallelizable |
| Wave F | 2-3 tasks | Tests require understanding the invariant deeply |
| Wave G | 1-2 L-sized or 3-4 M-sized tasks | Complex feature implementation |

### 7.3 When to Harvest

Harvest after:
- Every session (mandatory, NEG-HARVEST-001)
- Every significant design decision
- Every completed wave
- Before switching between waves

### 7.4 Progress Verification

After each wave or significant milestone:

```bash
# Verify code health
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets -- -D warnings

# Verify specification alignment
braid bilateral               # F(S) should be monotonically increasing

# Verify store health
braid status                  # Check datom count, entity count, task status
```

### 7.5 Recommended Session Sequence

For a single agent working sequentially:

1. **Session 1**: Wave A remaining (A1-A7, A12) -- target CI green, zero false witnesses
2. **Session 2**: Wave B -- B1.1-B1.4 (cascade struct + steps 1-4)
3. **Session 3**: Wave B -- B1.5-B1.7 (wire cascade, determinism) + B2 (resolution)
4. **Session 4**: Wave B -- B3 (lifecycle, all 11 tasks)
5. **Session 5**: Wave B -- B4-B5 (sub-tasks, docs) + Wave E (spec hygiene batch)
6. **Session 6**: Wave C -- C1 (budget+interface wiring) + C2 (signal infra)
7. **Session 7**: Wave C -- C3 (query infra) + C4 (other prep)
8. **Session 8**: Wave D -- D1-D7 (graph algorithms, subgraph, QueryResult)
9. **Session 9**: Wave D -- D8-D13 (VAET/AVET, access log)
10. **Session 10**: Wave F -- F1-F8 (tests for untested INVs, proptest additions)
11. **Session 11**: Wave F -- F9-F12 (fuzz, MIRI, SYNC tests) + Wave G7 (misc)
12. **Session 12**: Wave G -- G1 (lattice resolution pipeline, 8 tasks)
13. **Session 13**: Wave G -- G2 (budget/guidance, 13 tasks)
14. **Session 14**: Wave G -- G3 (bilateral/harvest/seed, 10 tasks)
15. **Session 15**: Wave G -- G4 (conflict/resolution, 11 tasks)
16. **Session 16**: Wave G -- G5 (divergence detection, 10 tasks)
17. **Session 17**: Wave G -- G6 (failure mode mitigations, 9 tasks)
18. **Session 18**: Final integration testing, Stage 0 close verification
19. **Session 19**: Stage 1 close verification, F(S) >= 0.90 confirmation
20. **Session 20**: Cleanup, documentation, handoff to Stage 2

For multiple agents working in parallel, sessions 8-11 (Waves D/E/F) can be assigned to separate agents, reducing the timeline to ~15 sessions.

---

## 8. Task Counts Summary

| Wave | Tasks | S | M | L | DONE | Remaining |
|------|------:|--:|--:|--:|-----:|----------:|
| A | 12 | 7 | 3 | 0 | 5 | 7 |
| B | 35 | 13 | 18 | 2 | 0 | 35 |
| C | 15 | 2 | 10 | 2 | 0 | 15 |
| D | 13 | 3 | 8 | 2 | 0 | 13 |
| E | 29 | 27 | 2 | 0 | 0 | 29 |
| F | 12 | 3 | 7 | 1 | 0 | 12 |
| G | 62 | 5 | 38 | 12 | 0 | 62 |
| H | 5 | 0 | 0 | 5 | 0 | 5 (deferred) |
| **Total** | **183** | **60** | **86** | **24** | **5** | **173** |

**Estimated total effort**: 20-25 sessions (single agent) or 15-18 sessions (2-3 parallel agents).
- Waves A+B (Stage 0 close): 5-7 sessions
- Waves C-F (S1 prep + parallel tracks): 5-8 sessions
- Wave G (Stage 1 implementation): 6-8 sessions
- Integration + verification: 2-3 sessions

---

*This execution plan is itself a DDIS artifact: every task traces to a spec element, every wave addresses an identified divergence pattern, and every success criterion is mechanically verifiable. When the braid task management system is operational, these tasks become datoms in the store, queryable and trackable through the harvest/seed lifecycle. Until then, this document is the canonical task registry.*
