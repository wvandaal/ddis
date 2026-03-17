# Merge + Sync — Stage 0/1 Audit
> Wave 1 Domain Audit | Date: 2026-03-17 | Agent: Opus 4.6 | Methodology: Fagan Inspection + IEEE Walkthrough

## Domain Inventory

**Spec files audited**: `spec/07-merge.md`, `spec/08-sync.md`
**Guide audited**: `docs/guide/07-merge-basic.md`
**Code audited**: `crates/braid-kernel/src/merge.rs`, `crates/braid-kernel/src/store.rs` (Store::merge, MergeReceipt, Frontier), `crates/braid-kernel/src/resolution.rs`, `crates/braid-kernel/src/branch.rs`, `crates/braid-kernel/src/agent_store.rs`, `crates/braid-kernel/src/kani_proofs.rs`, `crates/braid-kernel/tests/stateright_model.rs`, `crates/braid-kernel/tests/multi_agent_stress.rs`, `crates/braid/src/commands/mod.rs` (merge command)
**SEED.md sections**: Section 4 (Axioms) and Section 6 (Reconciliation Mechanisms)

### MERGE Namespace (spec/07-merge.md)
- **INVs**: INV-MERGE-001 through INV-MERGE-010 (10 invariants)
- **ADRs**: ADR-MERGE-001 through ADR-MERGE-007 (7 ADRs)
- **NEGs**: NEG-MERGE-001 through NEG-MERGE-003 (3 negative cases)

### SYNC Namespace (spec/08-sync.md)
- **INVs**: INV-SYNC-001 through INV-SYNC-005 (5 invariants)
- **ADRs**: ADR-SYNC-001 through ADR-SYNC-003 (3 ADRs)
- **NEGs**: NEG-SYNC-001, NEG-SYNC-002 (2 negative cases)

---

## Findings

### FINDING-001: INV-MERGE-002 identity collision -- spec says "Merge Cascade Completeness" but code says "Frontier Monotonicity"

**Severity**: HIGH
**Type**: DIVERGENCE
**Sources**: `spec/07-merge.md:264-286` (INV-MERGE-002: "Merge Cascade Completeness") vs `crates/braid-kernel/src/merge.rs:10` ("INV-MERGE-002: Merge preserves causal order (frontier pointwise max)") and `crates/braid-kernel/src/kani_proofs.rs:16,427-490` ("INV-MERGE-002: Frontier monotonicity")
**Evidence**: The spec defines INV-MERGE-002 as:
> "all 5 cascade steps execute: (1) conflict detection, (2) cache invalidation, (3) projection staleness, (4) uncertainty update, (5) subscription notification. all cascade steps produce datoms"

The code's merge.rs module header (line 10) defines it as:
> "INV-MERGE-002: Merge preserves causal order (frontier pointwise max)"

The Kani proof `prove_frontier_monotonicity` at kani_proofs.rs:442 also labels itself INV-MERGE-002 and verifies frontier monotonicity, not cascade completeness.

The Stateright model (stateright_model.rs:15) also references "INV-MERGE-002: Frontier monotonicity -- frontiers never shrink."

**Impact**: The formal verification artifacts (Kani proofs, Stateright models) verify a different invariant than what the spec requires under that ID. The actual spec requirement (cascade completeness with 5-step datom trail) has no formal verification at all. This is the most serious traceability failure in the domain.

---

### FINDING-002: MergeReceipt struct is missing spec-required fields

**Severity**: HIGH
**Type**: DIVERGENCE
**Sources**: `spec/07-merge.md:187-190,446-451` (INV-MERGE-009 L2 interface) vs `crates/braid-kernel/src/store.rs:418-423` (actual MergeReceipt)
**Evidence**: The spec's L2 interface (spec/07-merge.md:187-190) defines:
```rust
pub struct MergeReceipt {
    pub new_datoms:      usize,
    pub duplicate_datoms: usize,
    pub frontier_delta:  HashMap<AgentId, (Option<TxId>, TxId)>,
}
```

The actual implementation (store.rs:418-423):
```rust
pub struct MergeReceipt {
    pub new_datoms: usize,
    pub total_datoms: usize,
}
```

The `duplicate_datoms` and `frontier_delta` fields are entirely absent. There is a `total_datoms` field that the spec does not define. A codebase-wide grep for `frontier_delta` and `duplicate_datoms` returns zero matches.

**Impact**: INV-MERGE-009 (Merge Receipt Completeness) is violated at the interface level. The receipt does not record how many datoms were deduplicated, nor does it record per-agent frontier changes. Post-merge auditing cannot determine which agents' frontiers advanced.

---

### FINDING-003: Merge cascade is completely absent -- no CascadeReceipt, no 5-step cascade

**Severity**: CRITICAL
**Type**: UNIMPLEMENTED
**Sources**: `spec/07-merge.md:84-96,192-201,264-286` (cascade specification) and `docs/guide/07-merge-basic.md:116-228` (cascade implementation plan) vs `crates/braid-kernel/src/store.rs:651-698` (actual Store::merge)
**Evidence**: The spec requires a 5-step cascade after every merge (INV-MERGE-002), producing a CascadeReceipt. The guide provides detailed pseudocode for `run_cascade()` and `cascade_stub()` functions.

The actual Store::merge implementation (store.rs:651-698) performs:
1. Set union (BTreeSet insert loop)
2. Frontier pointwise max
3. Clock advancement
4. Schema rebuild
5. Index rebuild

No cascade steps execute. No CascadeReceipt is produced. No stub datoms are generated. There is no `run_cascade` function anywhere in the codebase. The `merge_stores` free function in merge.rs simply delegates to `target.merge(source)` which returns only a MergeReceipt.

The guide (07-merge-basic.md) specifies that even at Stage 0, ADR-MERGE-007 requires stub datoms for steps 2-5 and full conflict detection for step 1. None of this exists.

A codebase-wide grep for `CascadeReceipt` returns zero matches in Rust source files.

**Impact**: INV-MERGE-002 (Merge Cascade Completeness) is completely unimplemented. NEG-MERGE-002 ("No Merge Without Cascade") is violated on every merge operation. The audit trail for post-merge effects is entirely absent.

---

### FINDING-004: INV-MERGE-009 and INV-MERGE-010 doc-header descriptions swapped vs spec

**Severity**: MEDIUM
**Type**: MISALIGNMENT
**Sources**: `spec/07-merge.md:425-461` (INV-MERGE-009: Merge Receipt Completeness) and `spec/07-merge.md:465-530` (INV-MERGE-010: Cascade Determinism) vs `crates/braid-kernel/src/merge.rs:17-18`
**Evidence**: The merge.rs module header (lines 17-18) states:
```
//! - **INV-MERGE-009**: Cascade: schema rebuild -> resolution recompute -> LIVE invalidation.
//! - **INV-MERGE-010**: MergeReceipt captures new datom count and conflict set.
```

The spec defines the opposite:
- INV-MERGE-009 = "Merge Receipt Completeness" (receipt fields)
- INV-MERGE-010 = "Cascade Determinism" (cascade as pure function)

The descriptions are transposed. The merge_stores function docstring (line 48) labels the cascade "INV-MERGE-009" consistent with the swapped interpretation.

**Impact**: Any developer or agent tracing invariant IDs from code to spec will find contradictory definitions. Test witness annotations referencing these IDs are misleading.

---

### FINDING-005: Conflict predicate does not check cardinality or causal independence

**Severity**: MEDIUM
**Type**: DIVERGENCE
**Sources**: `spec/07-merge.md:336-339` (Cascade Step 1 conflict detection) and `resolution.rs:195-229` (has_conflict implementation) and INV-RESOLUTION-004 (six-condition predicate)
**Evidence**: The spec (07-merge.md:336-339) and INV-RESOLUTION-004 require all six conditions for conflict:
1. Same entity
2. Same attribute
3. Different values
4. Both assertions
5. Attribute has cardinality :one
6. Causally independent

The `has_conflict` function (resolution.rs:216-229) only checks:
- Multi-value mode (returns false) -- approximation of condition 5
- Number of active assertions > 1
- Different values exist

It does NOT check cardinality from schema (condition 5 is partially addressed by checking resolution mode, but ResolutionMode::Lww does not imply cardinality :one -- an attribute could be :many with LWW resolution). It does NOT check causal independence (condition 6) -- there is no causal ordering check at all.

**Impact**: The conflict predicate may produce false positives for causally ordered assertions from the same agent chain. It may also detect "conflicts" on cardinality-many attributes under LWW mode, which the spec says cannot conflict.

---

### FINDING-006: Merge function is a Store method, not a free function as specified

**Severity**: MEDIUM
**Type**: DIVERGENCE
**Sources**: `spec/07-merge.md:207-209` ("Free functions (ADR-ARCHITECTURE-001)") and `docs/guide/07-merge-basic.md:44-49` (notes the Rust adaptation) vs `crates/braid-kernel/src/store.rs:651` (Store::merge method)
**Evidence**: The spec (07-merge.md:207-209) requires:
```rust
pub fn merge(target: &mut Store, source: &Store) -> (MergeReceipt, CascadeReceipt);
```

The guide acknowledges the divergence (07-merge-basic.md:44-49) and explains it as a Rust adaptation. The actual implementation lives as `Store::merge(&mut self, other: &Store) -> MergeReceipt` at store.rs:651.

The `merge_stores` free function in merge.rs:60 exists but is a thin wrapper that just calls `target.merge(source)`. The return type is `MergeReceipt`, not `(MergeReceipt, CascadeReceipt)`.

**Impact**: The function does not return a CascadeReceipt (see FINDING-003). The guide documents this as an acceptable Rust adaptation, but the return type divergence means the spec's L2 interface contract is not met.

---

### FINDING-007: INV-MERGE-008 code definition diverges from spec

**Severity**: LOW
**Type**: MISALIGNMENT
**Sources**: `spec/07-merge.md:404-422` (INV-MERGE-008: "At-Least-Once Idempotent Delivery") vs `crates/braid-kernel/src/merge.rs:16` ("INV-MERGE-008: Deduplication by content identity")
**Evidence**: The spec defines INV-MERGE-008 as:
> "MERGE(MERGE(S, R), R) = MERGE(S, R) (duplicate delivery produces same result -- idempotency from L3)"

The code labels it as "Deduplication by content identity" which is a related but different concept (INV-STORE-003).

The actual behavior is correct -- BTreeSet insertion does deduplicate by content identity, and the property test `merge_stores_is_idempotent` (merge.rs:258-269) verifies idempotency. But the documentation description is imprecise.

**Impact**: Low -- the behavior is correct, but the documentation is misleading about what the invariant actually requires.

---

### FINDING-008: Sync namespace is entirely unimplemented (all 5 INVs, 3 ADRs, 2 NEGs)

**Severity**: INFO
**Type**: UNIMPLEMENTED
**Sources**: `spec/08-sync.md` (entire file, Stage 3) vs codebase
**Evidence**: The spec declares the SYNC namespace as Stage 3. All elements are deferred:
- INV-SYNC-001 through INV-SYNC-005: no implementation
- ADR-SYNC-001 through ADR-SYNC-003: no implementation
- NEG-SYNC-001, NEG-SYNC-002: no implementation

There is no Barrier struct, no sync_barrier function, no barrier_participate function, no query_at_barrier function anywhere in the codebase. No `braid sync` CLI command exists.

The only sync-related code is a comment block in merge.rs:100-109 listing all SYNC invariants/ADRs/NEGs as doc annotations on the `verify_frontier_advancement` function, which is a frontier comparison utility -- not a sync barrier implementation.

**Impact**: Expected at Stage 0. The annotations in merge.rs:100-109 create a misleading traceability signal -- they claim to witness INV-SYNC-001 through INV-SYNC-005 via a simple frontier comparison function, but these invariants require actual barrier protocols with participant exchange and timeout semantics.

---

### FINDING-009: verify_frontier_advancement falsely annotated as implementing SYNC invariants

**Severity**: MEDIUM
**Type**: STALE
**Sources**: `crates/braid-kernel/src/merge.rs:100-109` vs `spec/08-sync.md:163-276`
**Evidence**: The function `verify_frontier_advancement` (merge.rs:110-122) is annotated with:
```
/// INV-SYNC-001: Barrier produces consistent cut
/// INV-SYNC-002: Barrier timeout safety
/// INV-SYNC-003: Barrier is topology-independent
/// INV-SYNC-004: Barrier entity provenance
/// INV-SYNC-005: Non-monotonic queries require barrier
/// ADR-SYNC-001, ADR-SYNC-002, ADR-SYNC-003
/// NEG-SYNC-001, NEG-SYNC-002
```

The function body simply checks that post-frontier >= pre-frontier for all agents:
```rust
pub fn verify_frontier_advancement(pre: &Frontier, post: &Frontier) -> bool {
    for (agent, pre_tx) in pre {
        match post.get(agent) {
            Some(post_tx) => { if post_tx < pre_tx { return false; } }
            None => return false,
        }
    }
    true
}
```

This checks monotonicity of a single frontier pair. It does not implement barriers, timeouts, participant exchange, consistent cuts, topology independence, or non-monotonic query gating. All 10 annotations are false witnesses.

**Impact**: If automated witness scanning is used to assess coverage, all 10 SYNC elements would appear "witnessed" when they are not implemented. This corrupts bilateral coverage metrics.

---

### FINDING-010: ADR-MERGE-004 code description says "prefer-left, prefer-right" but spec says "Union, SelectiveUnion, ConflictToDeliberation"

**Severity**: LOW
**Type**: MISALIGNMENT
**Sources**: `spec/07-merge.md:599-613` (ADR-MERGE-004: Three Combine Strategies) vs `crates/braid-kernel/src/merge.rs:25`
**Evidence**: merge.rs line 25 states:
> "ADR-MERGE-004: Three combine strategies (union, prefer-left, prefer-right)."

The spec (07-merge.md:599-613) defines the three strategies as:
> "Union -- b1.datoms U b2.datoms; SelectiveUnion -- agent-curated subset; ConflictToDeliberation -- conflicts -> Deliberation entity"

"prefer-left" and "prefer-right" are not options in the spec.

**Impact**: Low -- Stage 2 feature, not yet implemented. But the docstring will mislead implementers when the time comes.

---

### FINDING-011: Branch module implements filter-based branching, not spec's BranchingGSet model

**Severity**: MEDIUM
**Type**: DIVERGENCE
**Sources**: `spec/07-merge.md:33-49` (Branching G-Set algebraic model) and `spec/07-merge.md:98-131` (six branch sub-operations) vs `crates/braid-kernel/src/branch.rs` (filter-based implementation)
**Evidence**: The spec defines branches as a Branching G-Set extension with properties P1-P5, where each branch is a G-Set over datoms with its own datom set (`branch.datoms: BTreeSet<Datom>`). The spec defines six operations: FORK, COMMIT, COMBINE, REBASE, ABANDON, COMPARE.

The implementation (branch.rs) uses a fundamentally different model: branches are **filters on transaction tags**. A branch is defined as:
```
branch(name) = { d in S : d.tx has :tx/branch = name }
```

This means all branch datoms live in the single shared store. The branch is not a separate G-Set but a view. The `merge_branch` function (branch.rs:103-145) re-asserts source datoms under a new transaction tagged with the target branch, which creates duplicate datoms with different tx IDs -- not a set union.

Only 3 of the 6 spec operations are implemented: create (FORK partial), merge (COMMIT partial), and prune (ABANDON). COMBINE, REBASE, and COMPARE (as a BranchComparison entity) are absent. The `compare_branches` function (branch.rs:155-186) returns symmetric difference, not a BranchComparison entity as spec requires.

**Impact**: Stage 2 items, but the foundational branch model diverges from the spec's algebraic formulation. The filter-based model cannot satisfy P2 (branch isolation) as strictly as the G-Set model because all datoms coexist in one store. The branch.rs module notes this is Stage 2 and deferred, which is appropriate for timing, but the architectural approach diverges.

---

### FINDING-012: No associativity test for merge in merge.rs

**Severity**: LOW
**Type**: GAP
**Sources**: `spec/07-merge.md:28` (L2: associativity) vs `crates/braid-kernel/src/merge.rs` (test section)
**Evidence**: The spec's L2 requires: `MERGE(MERGE(S1, S2), S3) = MERGE(S1, MERGE(S2, S3))`.

The merge.rs tests verify commutativity and idempotency but NOT associativity. The Kani proof `prove_merge_associativity` (kani_proofs.rs:194) verifies associativity at the BTreeSet level, and the Stateright model verifies it under concurrent interleavings. However, the unit/proptest suite in merge.rs itself has no associativity test, meaning the test gap exists at the integration level (Store::merge on actual stores, not bare BTreeSets).

**Impact**: Low -- the property is verified by Kani and Stateright. But the absence from the primary test file means local `cargo test` does not exercise associativity for Store::merge specifically.

---

### FINDING-013: Cascade step ordering inconsistency between spec, guide, and ADR-MERGE-007

**Severity**: LOW
**Type**: MISALIGNMENT
**Sources**: `spec/07-merge.md:84-96` (CASCADE definition), `spec/07-merge.md:809-817` (ADR-MERGE-007 stub names), `docs/guide/07-merge-basic.md:120-129` (guide step ordering)
**Evidence**: The guide (07-merge-basic.md:126-129) explicitly notes:
> "ADR-MERGE-007 stub attribute names diverge (e.g., step 3 = :cascade/secondary-conflicts, step 5 = :cascade/projection-staleness). This guide follows the L0 definition, which is authoritative."

The spec's CASCADE definition (07-merge.md:84-96) lists:
1. DETECT CONFLICTS
2. INVALIDATE CACHES
3. MARK STALE PROJECTIONS
4. RECOMPUTE UNCERTAINTY
5. FIRE SUBSCRIPTIONS

ADR-MERGE-007 (07-merge.md:809-817) lists stub names that swap steps 3 and 5:
```
Step 2: :cascade/cache-invalidation
Step 3: :cascade/secondary-conflicts
Step 4: :cascade/uncertainty-delta
Step 5: :cascade/projection-staleness
```

The guide correctly identifies this divergence and states the L0 definition is authoritative. This is documented but unresolved.

**Impact**: Low -- the cascade is not implemented (FINDING-003), so the naming inconsistency has no runtime effect. But when cascade is implemented, the implementer must choose between spec L0 names and ADR-MERGE-007 names.

---

### FINDING-014: Generated coherence test for INV-SYNC-001 is a no-op

**Severity**: LOW
**Type**: STALE
**Sources**: `crates/braid-kernel/tests/generated_coherence_tests.rs:703-711`
**Evidence**: The generated test at line 703-711:
```rust
fn generated_:spec/inv_sync_001_monotonicity(store in arb_store(3)) {
    let before = store.datom_count();
    let after = store.datom_count();
    prop_assert!(before <= after);
}
```

This test reads `datom_count()` twice on the same unchanged store. `before` always equals `after`. The test is a tautology that cannot fail.

**Impact**: Low -- this is an auto-generated placeholder. But it inflates test counts for the SYNC namespace without testing anything.

---

### FINDING-015: Merge cascade description in merge_stores docstring diverges from spec cascade steps

**Severity**: MEDIUM
**Type**: DIVERGENCE
**Sources**: `spec/07-merge.md:84-96` (5 cascade steps) vs `crates/braid-kernel/src/merge.rs:50-57` (docstring cascade description)
**Evidence**: The merge.rs docstring for merge_stores (lines 50-57) describes the cascade as:
```
/// 1. Schema rebuild from merged datoms
/// 2. Resolution recompute for conflicting (entity, attribute) pairs
/// 3. LIVE index invalidation
/// 4. Guidance recalculation (deferred to guidance module)
/// 5. Trilateral metrics update (deferred to trilateral module)
```

The spec's cascade (07-merge.md:84-96) defines:
```
1. DETECT CONFLICTS
2. INVALIDATE CACHES
3. MARK STALE PROJECTIONS
4. RECOMPUTE UNCERTAINTY
5. FIRE SUBSCRIPTIONS
```

These are entirely different step sets. The code's "cascade" (schema rebuild, resolution recompute, LIVE invalidation) corresponds more to Store infrastructure maintenance than to the spec's cascade semantics. Steps 4-5 in the code docstring (guidance, trilateral) are different modules entirely, not cascade steps.

**Impact**: The merge_stores function claims to implement the 5-step cascade but actually describes a different set of operations. This misleads anyone reading the code to understand the cascade as spec-defined.

---

## Quantitative Summary

### MERGE Namespace (spec/07-merge.md)

| Category | Total | Implemented | Unimplemented | Divergent |
|----------|-------|-------------|---------------|-----------|
| INVs | 10 | 3 | 3 | 4 |
| ADRs | 7 | 1 | 4 | 2 |
| NEGs | 3 | 1 | 1 | 1 |

**INV Detail**:
- INV-MERGE-001 (Set Union): **Implemented** -- Store::merge at store.rs:651 does BTreeSet union. Verified by proptest, Kani, Stateright.
- INV-MERGE-002 (Cascade Completeness): **Unimplemented** -- no cascade exists. Also, the ID is used in code for a different invariant (frontier monotonicity). See FINDING-001, FINDING-003.
- INV-MERGE-003 (Branch Isolation): **Implemented** -- AgentStore provides W_alpha isolation (agent_store.rs). Branch filter in branch.rs provides tag-based isolation. Tested.
- INV-MERGE-004 (Competing Branch Lock): **Unimplemented** -- Stage 2, expected.
- INV-MERGE-005 (Branch Commit Monotonicity): **Divergent** -- The branch commit model differs from spec (filter-based vs G-Set). Monotonicity holds by construction but through a different mechanism.
- INV-MERGE-006 (Branch as First-Class Entity): **Implemented** -- branch.rs creates branch entities with :branch/* attributes. Tested.
- INV-MERGE-007 (Bilateral Branch Duality): **Unimplemented** -- Stage 2, expected.
- INV-MERGE-008 (Idempotent Delivery): **Implemented** -- BTreeSet dedup provides idempotency. Proptest verifies. Docstring mislabeled (FINDING-007).
- INV-MERGE-009 (Receipt Completeness): **Divergent** -- MergeReceipt missing duplicate_datoms and frontier_delta fields. See FINDING-002.
- INV-MERGE-010 (Cascade Determinism): **Divergent** -- No cascade to be deterministic. ID mislabeled in code as "MergeReceipt captures..." See FINDING-004.

**ADR Detail**:
- ADR-MERGE-001 (Set Union Over Heuristic): **Reflected** -- merge is pure set union.
- ADR-MERGE-002 (Branching G-Set): **Divergent** -- branch.rs uses filter model, not G-Set. See FINDING-011.
- ADR-MERGE-003 (Competing Branch Lock): **Unimplemented** -- Stage 2.
- ADR-MERGE-004 (Three Combine Strategies): **Unimplemented + Mislabeled** -- Stage 2. Code docstring says "prefer-left/prefer-right." See FINDING-010.
- ADR-MERGE-005 (Cascade as Deterministic Layer): **Unimplemented** -- No cascade layer exists.
- ADR-MERGE-006 (Branch Comparison Entity): **Unimplemented** -- Stage 2. compare_branches returns tuples, not BranchComparison entity.
- ADR-MERGE-007 (Cascade Stub Datoms): **Unimplemented** -- No stub datoms produced. See FINDING-003.

**NEG Detail**:
- NEG-MERGE-001 (No Data Loss): **Enforced** -- BTreeSet union preserves all datoms. Verified by proptest, Kani, Stateright.
- NEG-MERGE-002 (No Merge Without Cascade): **Violated** -- Every merge completes without cascade. See FINDING-003.
- NEG-MERGE-003 (No Working Set Leak): **Enforced** -- AgentStore isolates W_alpha. Tested with proptest.

### SYNC Namespace (spec/08-sync.md)

| Category | Total | Implemented | Unimplemented | Divergent |
|----------|-------|-------------|---------------|-----------|
| INVs | 5 | 0 | 5 | 0 |
| ADRs | 3 | 0 | 3 | 0 |
| NEGs | 2 | 0 | 2 | 0 |

All SYNC elements are Stage 3 and unimplemented, as expected. However, 10 of them are falsely annotated as witnessed by `verify_frontier_advancement` (FINDING-009).

### SEED.md Traceability

- SEED.md Section 4 Axiom 2 ("Store as grow-only set, CRDT merge = set union"): **Implemented**. Store::merge is set union. Verified.
- SEED.md Section 4 Axiom 3 ("sync barriers establish consistent cuts"): **Unimplemented**. Stage 3, expected.
- SEED.md Section 6 ("Merge combines knowledge... reveals where agents disagree"): **Partially implemented**. Merge works as set union. Conflict detection exists as a separate post-merge call (`detect_merge_conflicts`), but is NOT automatically invoked during merge. The spec requires it as cascade step 1.
- SEED.md Section 6 ("Sync Barrier establishes a consistent cut"): **Unimplemented**. Stage 3, expected.

---

## Domain Health Assessment

**Strongest aspect**: The core merge-as-set-union property (INV-MERGE-001) is the best-verified property in the domain. It has unit tests, property-based tests (proptest), bounded model checking (Kani), and exhaustive state exploration (Stateright). Commutativity, idempotency, and monotonicity are all verified across multiple verification modalities. The W_alpha working set isolation (INV-MERGE-003, NEG-MERGE-003) is also well-implemented with comprehensive tests.

**Most concerning gap**: The merge cascade (INV-MERGE-002, ADR-MERGE-005, ADR-MERGE-007) is completely absent, yet the spec and guide treat it as a Stage 0 deliverable with extensive specification including stub datom requirements. This is compounded by the INV-MERGE-002 identity collision (FINDING-001), which means the Kani and Stateright proofs that claim to verify this invariant actually verify a different property. The net effect is that the single most specified sub-feature of Stage 0 merge (the 5-step cascade with deterministic datom trail) is simultaneously the most unimplemented. Additionally, 10 SYNC spec elements are falsely annotated as witnessed, which could corrupt automated coverage metrics.
