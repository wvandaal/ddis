# Stage 0/1 Readiness and Next Steps -- Stage 0/1 Synthesis Audit
> Wave 2 Cross-Cutting Synthesis | Date: 2026-03-17 | Agent: Opus 4.6 | Methodology: Cross-domain synthesis of 124 Wave 1 findings

## 1. Stage 0 Scorecard

The Stage 0 scope comprises 83 invariants across 11 namespaces. The success criterion from SEED.md section 10 is: "Work 25 turns, harvest, start fresh with seed -- new session picks up without manual re-explanation."

| Namespace | Stage 0 INVs | Completion % | Key Gaps |
|-----------|-------------|-------------|----------|
| **STORE** (13) | INV-STORE-001-012, 014 | **85%** | INV-STORE-009 (frontier durability) not enforced; VAET/AVET indexes missing; `as_of` not implemented; `EntityId::from_raw_bytes` is `pub` not `pub(crate)` (INV-STORE-003 safety hole) |
| **LAYOUT** (11) | INV-LAYOUT-001-011 | **75%** | Serializer does not sort datoms before hashing (INV-LAYOUT-011 canonical serialization violated -- different insertion orders produce different hashes, breaking content-addressed identity); VAET/AVET indexes not implemented in rebuild |
| **SCHEMA** (7) | INV-SCHEMA-001-007 | **85%** | Genesis attribute count three-way contradiction (17/18/19); schema validation lacks full cardinality and retraction checks (INV-SCHEMA-004 partial); lattice `ResolutionMode` is unit variant (missing `lattice_id` field for INV-SCHEMA-007) |
| **QUERY** (10) | INV-QUERY-001-002, 005-007, 012-014, 017, 021 | **60%** | Evaluator is single-pass nested-loop join, NOT semi-naive fixpoint (INV-QUERY-001 docstring claims semi-naive but implementation is not); no Datalog text parser exists; `QueryMode` enum absent; Clause AST missing 3/5 variants; graph algorithms operate on `DiGraph` not `Store` |
| **RESOLUTION** (8) | INV-RESOLUTION-001-008 | **70%** | `has_conflict` missing causal independence check (false positive conflicts, INV-RESOLUTION-004 partial); lattice resolution silently falls back to LWW (INV-RESOLUTION-006); `conflict_to_datoms` uses unregistered attributes |
| **HARVEST** (5) | INV-HARVEST-001-003, 005, 007 | **80%** | Warning thresholds diverge 3x from spec values (INV-HARVEST-005); NEG-HARVEST-001 (no unharvested session termination) cannot be enforced -- no session termination detection mechanism |
| **SEED** (6) | INV-SEED-001-006 | **75%** | INV-SEED-006 (intention anchoring) not implemented; dynamic CLAUDE.md typestate pipeline missing (generates string but no incremental/typestate build) |
| **MERGE** (5) | INV-MERGE-001-002, 008-010 | **30%** | INV-MERGE-002 (cascade completeness) completely absent -- the critical finding. Code maps INV-MERGE-002 to "frontier monotonicity" but spec defines it as "5-step cascade." No `CascadeReceipt` exists. Merge is bare set union with no cascade steps producing datoms. INV-MERGE-010 (cascade determinism) also absent. |
| **GUIDANCE** (6) | INV-GUIDANCE-001-002, 007-010 | **65%** | `GuidanceTopology` comonadic structure declared in types but not wired; no guidance footer in MCP responses; 7/8 signal types commented out in dispatch |
| **INTERFACE** (6) | INV-INTERFACE-001-003, 008-010 | **60%** | MCP reloads store per call (no `ArcSwap`, INV-INTERFACE-002 "persistent process" violated); budget manager not wired into CLI dispatch; INV-INTERFACE-009 (error recovery) partially implemented |
| **TRILATERAL** (6) | INV-TRILATERAL-001-003, 005-007 | **80%** | 2.5 of 8 reconciliation taxonomy types detected (INV-TRILATERAL-003 partial); Bilateral `Boundary` enum has 2 variants not 4 |

**Weighted Stage 0 Completion: approximately 68%**

This is calculated by weighting each namespace's completion by its INV count relative to the 83 total.

---

## 2. Stage 0 Close Blockers

These are the minimum fixes required to declare Stage 0 complete, ranked by severity.

**BLOCKER 1 (Critical): Merge cascade is completely absent.**
INV-MERGE-002 is a Stage 0 deliverable requiring 5 cascade steps (conflict detection, cache invalidation, projection staleness, uncertainty update, subscription notification), each producing datoms. The code has none of this. The spec even has ADR-MERGE-007 addressing exactly this gap (stub datoms at Stage 0). The current merge is bare set union with no cascade at all. This is the single largest gap between spec and implementation.

**BLOCKER 2 (High): Serializer does not sort datoms before hashing.**
INV-LAYOUT-011 requires canonical serialization. The `serialize_tx` function iterates `tx.datoms` in insertion order. If two agents construct the same transaction with datoms in different orders, they get different content hashes, breaking content-addressed identity (INV-LAYOUT-001, INV-STORE-003). This is a correctness bug that undermines the fundamental CRDT merge property.

**BLOCKER 3 (High): Generated coherence test file has syntax error.**
`crates/braid-kernel/tests/generated_coherence_tests.rs` (932 lines) has an unclosed delimiter, preventing `cargo test --all-targets` from compiling. Gate 2 (test) is broken at the CI level.

**BLOCKER 4 (High): Genesis attribute count three-way contradiction.**
The `GENESIS_ATTR_COUNT` constant is 19, but different parts of the code reference 17 or 18 axiomatic attributes. This is an internal spec contradiction (Tier 1: exact contradiction) and is precisely the kind of divergence DDIS is designed to detect.

**BLOCKER 5 (High): `EntityId::from_raw_bytes` is `pub`.**
The function is documented "for deserialization only" but is a public API escape hatch. Any external code can construct arbitrary `EntityId` values bypassing content-addressed identity (INV-STORE-003). Must be `pub(crate)`.

**BLOCKER 6 (Medium): Datalog evaluator is not semi-naive.**
The evaluator file header claims "Semi-naive fixpoint Datalog evaluator" but the implementation is a single-pass nested-loop join. INV-QUERY-001 (CALM compliance, semi-naive fixpoint convergence) is not met. For Stage 0 scope, the evaluator works for non-recursive queries, but the documentation is inaccurate and recursive query support is absent.

---

## 3. Stage 1 Blockers

Stage 1 adds 26 invariants for budget-aware output, guidance injection, bilateral loop, and advanced graph metrics. The following must be resolved before Stage 1 can begin:

**B1: All Stage 0 Close Blockers above must be resolved first.** Stage 1 builds on a working Stage 0 foundation.

**B2: Bilateral Boundary enum has 2 variants, spec requires 4.** Stage 1 introduces INV-BILATERAL-001-002, 004-005 which require the full four-boundary model (Intent-Spec, Spec-Impl, Impl-Behavior, and cross-boundary). The current enum has only `IntentSpec` and `SpecImpl`.

**B3: Signal system is minimal (1 of 8 types).** Stage 1 introduces INV-SIGNAL-002 (Confusion signal). While Confusion is technically the only Stage 1 signal, the signal infrastructure (subscription, dispatch, signal-as-datom) must be functional. Currently 7/8 signal types are commented out and the subscription system is entirely unimplemented.

**B4: Budget manager exists but is not wired into CLI dispatch.** Stage 1 is explicitly "Budget-Aware Output." The budget module has 1,121 LOC of implementation but is orphaned -- no command actually uses it. This must be integrated before Stage 1 budget invariants (INV-BUDGET-001-006) can be addressed.

**B5: MCP store reload per call.** Stage 1 requires continuous Q(t) tracking. If the store reloads on every MCP call, Q(t) state is lost between calls. The persistent process requirement (INV-INTERFACE-002) needs `ArcSwap` or equivalent.

**B6: 10 SYNC elements falsely witnessed.** The `verify_frontier_advancement` function in merge.rs has doc comments claiming to verify INV-SYNC-001 through INV-SYNC-005 plus ADR-SYNC-001-003, NEG-SYNC-001-002. SYNC is a Stage 3 namespace. A simple frontier comparison function does not implement sync barriers. These false witness claims must be removed to avoid poisoning coherence metrics.

---

## 4. Prioritized Action List (Top 20)

Ranked by (impact on coherence) x (1 / effort). Impact is scored 1-10.

| # | Action | Findings Addressed | Effort | Impact | Files |
|---|--------|-------------------|--------|--------|-------|
| **1** | Fix generated coherence test syntax error (unclosed delimiter) | Test suite broken | S | 9 | `crates/braid-kernel/tests/generated_coherence_tests.rs` |
| **2** | Restrict `EntityId::from_raw_bytes` to `pub(crate)` | INV-STORE-003 safety hole | S | 8 | `crates/braid-kernel/src/datom.rs` |
| **3** | Sort datoms before serialization in `serialize_tx` | INV-LAYOUT-011 canonical serialization, content-addressed identity | S | 9 | `crates/braid-kernel/src/layout.rs` |
| **4** | Resolve genesis attribute count contradiction (pick 19, fix all references) | Three-way contradiction (17/18/19) | S | 7 | `crates/braid-kernel/src/schema.rs`, `crates/braid-kernel/src/store.rs` |
| **5** | Remove false SYNC witness claims from merge.rs | 10 SYNC elements falsely witnessed | S | 8 | `crates/braid-kernel/src/merge.rs` |
| **6** | Implement merge cascade stub datoms (ADR-MERGE-007 pattern) | INV-MERGE-002 cascade absent, INV-MERGE-010 cascade determinism | M | 10 | `crates/braid-kernel/src/merge.rs`, `crates/braid-kernel/src/store.rs` |
| **7** | Fix `has_conflict` to include causal independence check | INV-RESOLUTION-004 false positive conflicts | M | 7 | `crates/braid-kernel/src/resolution.rs` |
| **8** | Add `lattice_id` field to lattice `ResolutionMode` variant | INV-SCHEMA-007, lattice resolution silently falls back to LWW | S | 6 | `crates/braid-kernel/src/schema.rs`, `crates/braid-kernel/src/resolution.rs` |
| **9** | Register attributes used by `conflict_to_datoms` in genesis schema | conflict_to_datoms uses unregistered attributes | S | 6 | `crates/braid-kernel/src/resolution.rs`, `crates/braid-kernel/src/schema.rs` |
| **10** | Wire budget manager into CLI dispatch (at minimum, `braid status` shows Q(t)) | Budget manager not wired | M | 7 | `crates/braid/src/commands/mod.rs`, `crates/braid/src/commands/status.rs` |
| **11** | Fix evaluator documentation: remove "semi-naive" claim or implement fixpoint | INV-QUERY-001 docstring false claim | S | 5 | `crates/braid-kernel/src/query/evaluator.rs` |
| **12** | Add `Boundary::ImplBehavior` and `Boundary::IntentImpl` to bilateral enum | Bilateral Boundary enum has 2 not 4 variants | S | 5 | `crates/braid-kernel/src/bilateral.rs` |
| **13** | Implement INV-SEED-006 intention anchoring (anchor task reference in seed output) | Seed does not anchor against task drift | M | 6 | `crates/braid-kernel/src/seed.rs` |
| **14** | Add guidance footer injection to MCP tool responses | No guidance footer in MCP responses | M | 6 | `crates/braid/src/mcp.rs` |
| **15** | Align harvest warning thresholds with spec values | Warning thresholds diverge 3x | S | 4 | `crates/braid-kernel/src/harvest.rs` |
| **16** | Fix INV-MERGE-002 Kani proof to verify cascade completeness (not just frontier) | Kani proves wrong property for INV-MERGE-002 | M | 7 | `crates/braid-kernel/src/kani_proofs.rs` |
| **17** | Create agent entities for non-genesis agents during transact | Agent entities never created | M | 5 | `crates/braid-kernel/src/store.rs` |
| **18** | Implement `Store::as_of(tx_id)` for temporal queries | Store::as_of missing | M | 5 | `crates/braid-kernel/src/store.rs` |
| **19** | Fix `DeliberationStatus` to not derive `Ord` (spec says "do NOT") | Spec violation on ordering | S | 3 | `crates/braid-kernel/src/deliberation.rs` |
| **20** | Implement INV-STORE-009 frontier durability (persist frontier to layout) | Frontier durability not enforced | L | 6 | `crates/braid-kernel/src/store.rs`, `crates/braid-kernel/src/layout.rs`, `crates/braid/src/layout.rs` |

---

## 5. Spec Hygiene Actions

These are fixes needed in specification and guide documents, not implementation code.

**SH-1: Reconcile INV-MERGE-002 definition between code and spec.** The code maps INV-MERGE-002 to "frontier monotonicity." The spec defines it as "Merge Cascade Completeness" (5-step cascade). Either the code's understanding or the spec's numbering must be reconciled. The spec version is authoritative.

**SH-2: TOPOLOGY namespace is correctly Stage 3.** The audit finding "TOPOLOGY namespace entirely unimplemented (16 INVs)" is not a gap -- it is correctly deferred. However, `spec/README.md` should explicitly note this to avoid future audit confusion.

**SH-3: SYNC stage assignment clarity.** INV-SYNC-001 through INV-SYNC-005 are Stage 3, but `merge.rs` includes frontier verification claiming to witness SYNC invariants. The spec should add a note in `spec/08-sync.md` that Stage 0 frontier comparison is a precursor, not a witness of SYNC invariants.

**SH-4: Guide type catalog divergence.** `docs/guide/types.md` specifies `CascadeReceipt` as a merge return type but it does not exist in the implementation. The guide's `merge_stores` signature returns `(MergeReceipt, CascadeReceipt)` but the actual implementation returns only `MergeReceipt`.

**SH-5: Cross-reference index completeness.** `spec/17-crossref.md` excludes 47 spec elements from the cross-reference index. These should be catalogued to enable bilateral coherence checking.

**SH-6: Verification matrix should flag false witnesses.** `spec/16-verification.md` has no mechanism for recording "this invariant is claimed as witnessed but the witness is incorrect." A "witness quality" column would prevent the SYNC false-witness problem from recurring.

**SH-7: Stage 0 sub-staging should be made explicit in SEED.md.** The guide's Stage 0a/0b split is a recommendation in the guide only. SEED.md section 10 describes Stage 0 as monolithic. The two documents should be aligned.

---

## Summary Assessment

**Stage 0 is approximately 68% complete.** The core store, schema, harvest, seed, and trilateral namespaces are substantially implemented (75-85%). The critical gap is the MERGE namespace at 30% -- specifically the complete absence of merge cascade (INV-MERGE-002), which the spec treats as a Stage 0 deliverable. The QUERY and INTERFACE namespaces are at 60%, with the evaluator's false "semi-naive" claim and the MCP store reload issue being the main concerns.

The **functional success criterion** ("work 25 turns, harvest, seed, new session picks up") is partially met -- the braid-seed section of CLAUDE.md shows this works in practice (25 harvests, 43 observations recorded). However, the **formal success criterion** (83 invariants satisfied) has significant gaps.

791 tests pass but `cargo test --all-targets` does not compile due to the generated test syntax error. This means CI is broken.

**The single most important action for the project right now is:** Fix the merge cascade (Action #6). INV-MERGE-002 is the highest-impact Stage 0 invariant that is completely unimplemented. The spec already has ADR-MERGE-007 defining exactly how to do this with stub datoms. Without cascade, merge is a bare set union with no audit trail, no conflict detection record, and no post-merge coherence restoration -- which means the CRDT merge property is mathematically correct but operationally incomplete. Every downstream system (guidance recalculation, trilateral metrics, bilateral convergence monitoring) depends on cascade producing datoms that record what changed during merge. However, before tackling that M-sized effort, first spend 15 minutes on Actions #1 (fix generated test syntax) and #2-3 (restrict `from_raw_bytes`, sort datoms before hashing) -- these are S-sized fixes with outsized impact on correctness and CI health.
