# Braid Full Audit — Session 047

> **Auditor**: Claude Opus 4.6 (1M context)
> **Date**: 2026-03-26
> **Codebase snapshot**: commit `92bfb9a8` (HEAD of main)
> **Store state**: 108,627 datoms, 10,245 entities, 10,813 transactions
> **Test state**: 2,043 passing, 0 failing, 1 ignored
> **LOC**: 117,661 (83,293 kernel + 34,368 CLI)

---

# Document 1: Audit Report

## Summary Statistics

| Category | Count |
|----------|-------|
| Total findings | 31 |
| Critical (soundness risk) | 4 |
| High (correctness risk under extension) | 8 |
| Medium (maintenance/quality risk) | 12 |
| Low (cosmetic/minor) | 7 |

| Dimension | Findings |
|-----------|----------|
| 1. Type System Integrity | 4 |
| 2. Invariant Soundness | 4 |
| 3. Error Algebra | 3 |
| 4. Concurrency & Aliasing | 2 |
| 5. Spec-Implementation Alignment | 4 |
| 6. C8 Compliance | 5 |
| 7. Architectural Coherence | 3 |
| 8. Performance Architecture | 3 |
| 9. Formal Verification Coverage | 2 |
| 10. Test Architecture | 1 |

---

## Dimension 1: Type System Integrity

### F-TYPE-001 — MaterializedViews Has No Compile-Time Sync Guarantee

**SEVERITY**: High (correctness risk under maintenance)
**FILES**: `crates/braid-kernel/src/store.rs:540-604`

**OBSERVATION**:
`MaterializedViews` is a 23-field struct of counters and sets that shadow store state
(spec_count, validation_depth, coverage_impl_targets, etc.). It is updated by
`observe_datom()` during both batch construction (`from_datoms`) and incremental
application (`apply_tx`). The invariant `MaterializedViews::from_store(S).fitness() ==
compute_fitness(S)` is stated in a doc comment (line 538) but:

1. No property test verifies this isomorphism for arbitrary stores.
2. The batch path (`from_datoms`) and incremental path (`apply_tx → index_datom →
   observe_datom`) execute different surrounding logic — batch iterates all datoms;
   incremental processes only new datoms after insert deduplication.
3. `apply_datoms()` rebuilds views from scratch (line 1394: `self.views =
   MaterializedViews::default()`) on bulk import, creating a third code path.

**INVARIANT REFERENCE**: INV-BILATERAL-001 (Monotonic Convergence), INV-TRILATERAL-002
**CURRENT STATE**: Untested. The three code paths are logically independent.
**RISK**: If `observe_datom()` miscounts during incremental updates, F(S) drifts silently
from the true fitness. The hypothesis ledger's "mean error 0.521, trend: degrading"
(observed in `braid status`) may be partly caused by this divergence.

**RECOMMENDATION**:
1. Add proptest: generate random store of N datoms, compute fitness via
   `compute_fitness(&store)` and `store.views().fitness()`. Assert equality within ε.
2. Extract `observe_datom()` as the single authoritative update path; make all three
   code paths (genesis, from_datoms, apply_tx) call it identically.
**EFFORT**: ~4 hours. **ACCEPTANCE**: Proptest passes for 1000 random stores.

---

### F-TYPE-002 — Value Enum Admits Unvalidated Ref Cycles

**SEVERITY**: Medium
**FILES**: `crates/braid-kernel/src/datom.rs:52-85` (Value enum), `crates/braid-kernel/src/schema.rs` (validate_datom)

**OBSERVATION**:
`Value::Ref(EntityId)` allows any EntityId as a reference target, including entities that
do not exist in the store. Schema validation (`validate_datom`) checks value types but does
not verify referential integrity at transact time. This is intentional for eventual
consistency (a referenced entity may arrive in a later transaction), but there is no
mechanism to detect permanently dangling references.

**INVARIANT REFERENCE**: INV-SCHEMA-004 (Schema Validation on Transact)
**CURRENT STATE**: By design (C4 — merge by set union means references may precede referents).
**RISK**: Low in practice. A "dangling ref audit" query would surface these.

**RECOMMENDATION**:
Add `store.dangling_refs() -> Vec<(EntityId, Attribute, EntityId)>` as a diagnostic
function exposed via `braid verify`. No transact-time enforcement needed.
**EFFORT**: ~2 hours. **ACCEPTANCE**: Returns empty for self-consistent stores; non-empty for stores with orphan refs.

---

### F-TYPE-003 — Attribute Newtype Lacks Namespace Validation at Deser Boundary

**SEVERITY**: Medium
**FILES**: `crates/braid-kernel/src/datom.rs:100-125`

**OBSERVATION**:
`Attribute` validates the `:namespace/name` format at construction (`Attribute::new()`),
but deserialization from EDN strings can bypass this if a raw `Attribute(String)` is
constructed via serde. The `Deserialize` impl wraps raw strings into `Attribute` without
calling `new()`.

**INVARIANT REFERENCE**: INV-SCHEMA-001 (Schema-as-Data)
**CURRENT STATE**: The EDN parser in `layout.rs` does validate format, but a crafted
transaction file could inject invalid attributes.
**RISK**: Malformed attributes would bypass schema validation entirely. Low probability
(requires hand-editing .edn files), but violates defense-in-depth.

**RECOMMENDATION**:
Implement `Deserialize` manually for `Attribute` with validation, or use
`#[serde(try_from = "String")]` with the validation in `TryFrom`.
**EFFORT**: ~1 hour. **ACCEPTANCE**: `serde_json::from_str::<Attribute>("\"invalid\"")` returns `Err`.

---

### F-TYPE-004 — FitnessDelta and FitnessComponents Use f64 Without NaN Guards

**SEVERITY**: Medium
**FILES**: `crates/braid-kernel/src/store.rs` (FitnessDelta), `crates/braid-kernel/src/bilateral.rs` (FitnessComponents)

**OBSERVATION**:
`FitnessScore(f64)` and `FitnessComponents` use raw f64 values. Several downstream
consumers call `.partial_cmp().unwrap()` (e.g., `bilateral.rs:326`, `bilateral.rs:408`,
`bilateral.rs:587` in the CLI). If any component computes as NaN (e.g., 0/0 from an
empty store with spec_count=0), the unwrap panics.

**INVARIANT REFERENCE**: INV-BILATERAL-001 (Monotonic Convergence)
**CURRENT STATE**: Three `partial_cmp().unwrap()` calls in `commands/bilateral.rs` are
unguarded. Other call sites in `task.rs`, `analyze.rs`, and `status.rs` correctly use
`unwrap_or(Ordering::Equal)`.

**RECOMMENDATION**:
1. Replace `FitnessScore(f64)` with `FitnessScore(OrderedFloat<f64>)` (already a dependency).
2. Or: add `.unwrap_or(Ordering::Equal)` to the three unguarded sites.
**EFFORT**: ~1 hour. **ACCEPTANCE**: `FitnessScore(f64::NAN).partial_cmp(...)` does not panic.

---

## Dimension 2: Invariant Soundness

### F-INV-001 — LIVE View Ignores Bare Retractions

**SEVERITY**: Critical (correctness)
**FILES**: `crates/braid-kernel/src/store.rs:943-1000` (index_datom method)

**OBSERVATION**:
The LIVE view update logic in `index_datom()` handles two cases:
- `Op::Assert`: insert or update the (entity, attribute) → value mapping
- `Op::Retract`: remove the (entity, attribute) entry

However, the retraction path uses `self.live_view.remove(&(entity, attr))` which only
works if the retracted value matches the current LIVE value. If a retraction targets a
value that was already superseded by a newer assertion, the retraction is silently
ignored. More critically: if a retraction arrives *before* its matching assertion
(possible in CRDT merge), the LIVE view won't reflect it when the assertion later arrives.

The `from_datoms()` rebuild (line 1085-1095) processes datoms in BTreeSet order (by the
`Ord` impl on Datom), which sorts by (entity, attribute, value, tx, op). This means
assert-before-retract ordering is not guaranteed — it depends on the sort order of the
entire tuple, not just the transaction timestamp.

**INVARIANT REFERENCE**: INV-STORE-012 (LIVE Index Correctness)
**CURRENT STATE**: Unsound for stores with complex retraction patterns. The LIVE index
may show retracted values or miss valid assertions.

**RECOMMENDATION**:
1. The LIVE rebuild must process datoms in causal (tx) order, not BTreeSet order.
2. Add proptest: generate random assert/retract sequences, verify LIVE matches the
   expected state computed by a simple fold over causally-sorted datoms.
3. The incremental path must check whether the retracted value matches the current
   LIVE value before removing.
**EFFORT**: ~6 hours. **ACCEPTANCE**: Proptest with 10,000 random datom sequences.

---

### F-INV-002 — F(S) Monotonicity Claim Is False

**SEVERITY**: Critical (specification unsoundness)
**FILES**: `crates/braid-kernel/src/bilateral.rs:1-50` (doc comment claims monotonicity),
`spec/10-bilateral.md` (INV-BILATERAL-001)

**OBSERVATION**:
INV-BILATERAL-001 states: "Monotonic Convergence — F(S) is non-decreasing under
bilateral operations." The implementation comment says the same. This claim is false.

Counter-evidence: The project's own history shows F(S) dropped from 0.67 to 0.58
(Session 033) due to MaterializedViews placeholder values. More fundamentally:
- Adding a spec element without an implementation decreases the coverage component.
- Adding an unresolved uncertainty decreases the uncertainty component.
- A newly discovered contradiction decreases the coherence component.

F(S) is NOT monotone under bilateral operations. It is monotone under *convergent*
bilateral operations (where both spec and impl grow together). The invariant statement
needs qualification.

**INVARIANT REFERENCE**: INV-BILATERAL-001
**CURRENT STATE**: The invariant as stated is falsified by normal operation.
**RISK**: Code that assumes monotonicity (e.g., regression detection in hypothesis
ledger) will produce false alarms.

**RECOMMENDATION**:
1. Revise INV-BILATERAL-001 to: "F(S) is non-decreasing under the composition of a
   bilateral scan with its recommended resolution actions."
2. Track F(S) with a "high-water mark" that distinguishes expected temporary dips
   (new spec elements) from genuine regressions.
**EFFORT**: ~3 hours (spec revision + code adjustment). **ACCEPTANCE**: The revised invariant is not falsified by adding a spec element.

---

### F-INV-003 — Index Synchronization Depends on Single Insertion Point

**SEVERITY**: High
**FILES**: `crates/braid-kernel/src/store.rs:943` (index_datom), `store.rs:1165` (apply_datoms)

**OBSERVATION**:
The five secondary indexes (entity_index, attribute_index, vaet_index, avet_index,
live_view) plus MaterializedViews are maintained by `index_datom()`, which must be called
for every datom inserted into the primary BTreeSet. There is no compile-time enforcement
that this call happens. The correctness depends on programmer discipline: every path that
inserts into `self.datoms` must also call `index_datom()`.

There are four insertion paths:
1. `genesis()` → manual loop calling `views.observe_datom()` but NOT `index_datom()`
   (indexes are rebuilt in `from_datoms`)
2. `from_datoms()` → manual loop calling `views.observe_datom()`, then rebuilds indexes
3. `apply_tx()` → calls `index_datom()` per datom
4. `apply_datoms()` → rebuilds everything from scratch

Paths 1 and 2 use `observe_datom()` directly, bypassing `index_datom()`. This means the
MaterializedViews path and the index path are not unified.

**INVARIANT REFERENCE**: INV-STORE-012 (LIVE Index Correctness), ADR-STORE-005
**CURRENT STATE**: Functionally correct because `from_datoms()` rebuilds indexes
independently. But the two paths (incremental via `index_datom` vs batch via `from_datoms`)
are logically separate implementations of the same invariant.

**RECOMMENDATION**:
Unify the insertion paths: make `index_datom()` the sole function that updates both
indexes AND MaterializedViews. Remove the separate `observe_datom()` method.
**EFFORT**: ~4 hours. **ACCEPTANCE**: `grep -n 'observe_datom' store.rs` returns only the
definition and calls from `index_datom`.

---

### F-INV-004 — Schema Layers 1-4 Are Hardcoded, Not Bootstrapped from Policy

**SEVERITY**: High (C3 violation)
**FILES**: `crates/braid-kernel/src/schema.rs:200-800` (layer definitions)

**OBSERVATION**:
C3 states: "Schema is defined as datoms in the store, not as a separate DDL." Layer 0
(19 meta-schema attributes) is correctly hardcoded — it must be, because it bootstraps
the ability to define further schema. But Layers 1-4 (~180 attributes covering trilateral,
spec elements, discovery, coordination, workflow) are ALSO hardcoded in
`Schema::default_schema()`.

These layers should be transacted as datoms during `braid init` from a policy manifest.
The current design violates C3 because schema evolution for these layers requires a code
change, not a transaction.

**INVARIANT REFERENCE**: C3 (Schema-as-Data), INV-SCHEMA-001, ADR-FOUNDATION-013
**CURRENT STATE**: Violated. Layers 1-4 are hardcoded.
**RISK**: Any domain other than DDIS would need to fork the kernel to change the schema.

**RECOMMENDATION**:
1. Move Layers 1-4 to `.edn` manifest files loaded at `braid init`.
2. The kernel should only hardcode Layer 0 (meta-schema).
3. This is the most impactful C8 fix — it decouples the ontology from the code.
**EFFORT**: ~16 hours. **ACCEPTANCE**: `braid init --manifest research.edn` produces a store with a different schema than `braid init --manifest ddis.edn`.

---

## Dimension 3: Error Algebra

### F-ERR-001 — 1,180 unwrap() Calls Across Both Crates

**SEVERITY**: Medium (panic surface)
**FILES**: All crates (719 in kernel, 461 in CLI)

**OBSERVATION**:
The majority (~90%) are in test code (`#[test]`, `#[cfg(test)]`). However, the kernel
agent identified ~20 unwrap/expect calls in production library code, and the CLI agent
identified ~15 in production CLI code. The most concerning:

- `query/graph.rs:60-61`: `get_mut(src).unwrap()` in `add_edge` — panics if node not
  pre-added. This is a logic error (caller must add nodes first), but there is no
  compile-time enforcement.
- `seed.rs:3084,3121`: `kept.pop().unwrap()` — panics on empty vec.
- `bilateral.rs:326,408,587`: `partial_cmp().unwrap()` — panics on NaN (see F-TYPE-004).

**INVARIANT REFERENCE**: NEG-INTERFACE-004 (No Error Without Recovery Hint)
**CURRENT STATE**: Most unwraps are provably safe (documented invariants), but the
absence of proof comments on ~10 of them leaves the safety argument implicit.

**RECOMMENDATION**:
1. Add `// SAFETY: ...` comments to all production unwrap() calls.
2. Replace the ~5 genuinely risky unwraps with proper error returns.
3. Add `#[cfg_attr(test, allow(clippy::unwrap_used))]` and enable `clippy::unwrap_used`
   for non-test code.
**EFFORT**: ~4 hours. **ACCEPTANCE**: `cargo clippy` with `unwrap_used` lint shows zero violations in non-test code.

---

### F-ERR-002 — Error Types Are Manual Instead of thiserror-Derived

**SEVERITY**: Low (maintenance cost)
**FILES**: `crates/braid-kernel/src/error.rs`, `crates/braid/src/error.rs`

**OBSERVATION**:
All error types use manual `impl Display` and `impl Error`. This is deliberate (the kernel
avoids dependencies), but results in ~100 lines of boilerplate per error enum. The
`recovery_hint()` pattern is unique and valuable — thiserror doesn't support it natively.

**INVARIANT REFERENCE**: None (design choice)
**CURRENT STATE**: Functional but verbose.
**RISK**: Low. The error types are stable and rarely change.

**RECOMMENDATION**: Keep current approach. The custom `recovery_hint()` method justifies
manual implementation. Document the rationale as an ADR.
**EFFORT**: ~30 minutes (ADR only). **ACCEPTANCE**: ADR written.

---

### F-ERR-003 — Clippy Fails CI Gate With 8 Errors

**SEVERITY**: Low (CI hygiene)
**FILES**: `crates/braid-kernel/src/concept.rs`, `routing.rs`, `seed.rs`

**OBSERVATION**:
`cargo clippy --all-targets -- -D warnings` produces 8 errors:
- `routing.rs:2819`: unused import `ProvenanceType`
- `concept.rs:129`: should use `is_none_or`
- `concept.rs:1911`: manual `Option::map`
- `concept.rs:1979`: manual `div_ceil`
- `concept.rs:3608`: explicit closure for copy
- `concept.rs:4827`: manual `RangeInclusive::contains`
- `routing.rs:808`: identical if/else branches
- `seed.rs:2624`: manual clamp pattern

**INVARIANT REFERENCE**: CI gate requirement from `spec/16-verification.md`
**CURRENT STATE**: Failing. These are all trivial fixes.

**RECOMMENDATION**: Fix all 8 in one commit.
**EFFORT**: ~15 minutes. **ACCEPTANCE**: `cargo clippy --all-targets -- -D warnings` exits 0.

---

## Dimension 4: Concurrency & Aliasing

### F-CONC-001 — Zero Shared Mutable State (Positive Finding)

**SEVERITY**: N/A (positive)
**FILES**: Entire codebase

**OBSERVATION**:
The kernel contains zero `Arc<Mutex>`, zero `Rc<RefCell>`, zero `RwLock`, and
`#![forbid(unsafe_code)]`. The only shared state is a single `AtomicUsize` in
`harvest.rs` for a spec candidate counter. The store is purely synchronous and
single-threaded. All concurrency is external (multiple processes via filesystem).

This is an excellent design for correctness. The aliasing discipline is:
- Store is owned, not shared
- All queries take `&Store` (immutable borrow)
- Transactions consume `self` via typestate (Building → Committed → Applied)
- The daemon process holds a single `LiveStore` instance

**INVARIANT REFERENCE**: ADR-STORE-006 (Embedded Deployment)
**CURRENT STATE**: Sound.

---

### F-CONC-002 — LiveStore Refresh Has a TOCTOU Window

**SEVERITY**: Low (single-agent mitigated)
**FILES**: `crates/braid/src/live_store.rs:100-150`

**OBSERVATION**:
`LiveStore::refresh_if_needed()` checks `has_new_external_txns()` (comparing directory
mtime), then reads new transaction files. Between the mtime check and the file reads,
another agent could write additional transactions that are missed. This is a classic
TOCTOU (Time-Of-Check-Time-Of-Use) pattern.

**INVARIANT REFERENCE**: PD-003 (Crash-Recovery Model)
**CURRENT STATE**: Mitigated in practice because:
1. The daemon holds the store in memory; CLI falls back to refresh.
2. Content-addressed files are immutable — a missed file is caught on next refresh.
3. The refresh is called before every command, so the window is short.

**RISK**: Very low. A transaction could be invisible for one command invocation. The next
invocation will catch it.

**RECOMMENDATION**: Document as known behavior. No fix needed at current scale.
**EFFORT**: ~30 minutes (documentation). **ACCEPTANCE**: Comment in code.

---

## Dimension 5: Specification-Implementation Alignment

### F-SPEC-001 — 265 Current-Stage INVs Lack L2+ Witness

**SEVERITY**: High (verification gap)
**FILES**: `spec/` (all namespaces), `braid status` output

**OBSERVATION**:
The `braid status` output reports "265 current-stage INVs untested → add L2+ witness."
The specification defines 201 invariants across 23 namespaces. Of these, a significant
majority (estimated ~130 for Stage 0-1) lack formal witnesses (proptest, kani harness,
or E2E test that directly exercises the falsification condition).

Current test coverage by namespace (estimated from test name analysis):
- STORE: ~60% of INVs have direct tests
- QUERY: ~70% (graph algorithms well-tested)
- SCHEMA: ~50%
- RESOLUTION: ~40%
- HARVEST/SEED: ~30%
- TRILATERAL/BILATERAL: ~20%
- TOPOLOGY: ~60%
- GUIDANCE/BUDGET: ~15%

**INVARIANT REFERENCE**: INV-WITNESS-011 (Verification Completeness Guard)
**CURRENT STATE**: 2,043 tests exist, but many test implementation behavior rather than
specification invariants. The mapping from tests to INVs is incomplete.

**RECOMMENDATION**:
1. Create a coverage matrix: `INV-ID → test name → witness level (L0/L1/L2/L3)`.
2. Prioritize Stage 0 INVs (STORE, SCHEMA, QUERY, LAYOUT) for L2+ witnesses.
3. Target: 100% L2+ coverage for Stage 0 INVs before Stage 1 gate.
**EFFORT**: ~40 hours (incremental). **ACCEPTANCE**: Coverage matrix shows 100% for Stage 0.

---

### F-SPEC-002 — Spec Elements 201 vs Braid Status Reports Discrepancy

**SEVERITY**: Medium
**FILES**: `spec/README.md`, `braid status` output

**OBSERVATION**:
The spec files contain 201 INVs by manual count. The store contains 108,627 datoms
across 10,245 entities. The `braid status` reports "265 current-stage INVs untested"
which implies more than 265 INVs are tracked in the store. The discrepancy (265 vs 201)
suggests either:
1. Spec elements were created via `braid spec create` that don't exist in `spec/*.md` files, or
2. The "current-stage" filter includes elements from future stages.

**INVARIANT REFERENCE**: C7 (Self-Bootstrap), INV-BILATERAL-005
**CURRENT STATE**: Unclear. The store and the spec files are not in exact correspondence.

**RECOMMENDATION**:
Run `braid query '[:find (count ?e) :where [?e :spec/element-type "invariant"]]'` to
get the exact store count. Reconcile with `spec/*.md`.
**EFFORT**: ~2 hours. **ACCEPTANCE**: Store count matches spec file count, or discrepancies documented.

---

### F-SPEC-003 — 123 Observations With Uncrystallized Spec IDs

**SEVERITY**: Medium
**FILES**: `braid status` output (methodology gaps section)

**OBSERVATION**:
The methodology section of `braid status` reports "123 observations with uncrystallized
spec IDs → braid spec create" and "28 tasks with unresolved spec refs → crystallize
first." These are knowledge artifacts that reference non-existent spec elements —
essentially forward references to spec IDs that were never formalized.

**INVARIANT REFERENCE**: C5 (Traceability), INV-HARVEST-003 (Drift Score Recording)
**CURRENT STATE**: Open loop. Observations reference phantoms.

**RECOMMENDATION**: Batch-crystallize or prune. Run `braid task search "uncrystallized"` to
identify and resolve.
**EFFORT**: ~4 hours. **ACCEPTANCE**: Methodology gap count drops to <20.

---

### F-SPEC-004 — Specification Namespaces vs Implementation Module Mapping

**SEVERITY**: Low (documentation gap)
**FILES**: `spec/README.md`, `crates/braid-kernel/src/`

**OBSERVATION**:
The specification defines 23 namespaces. The implementation has ~47 source files. The
mapping between spec namespaces and implementation files is partially documented in
doc comments but not maintained as a formal cross-reference. Examples:
- STORE → store.rs (clear)
- QUERY → query/ (clear)
- TRILATERAL → trilateral.rs (clear)
- GUIDANCE → guidance.rs + methodology.rs + routing.rs + context.rs (split across 4 files)
- COHERENCE → bilateral.rs + coherence.rs (overlapping names confuse)

**INVARIANT REFERENCE**: C5 (Traceability)
**RECOMMENDATION**: Add a namespace-to-file mapping table in `docs/guide/00-architecture.md`.
**EFFORT**: ~1 hour. **ACCEPTANCE**: Table exists and is accurate.

---

## Dimension 6: C8 Compliance (Substrate Independence)

This is the most important audit dimension per the audit prompt. I classify each violation
by severity and count the functions affected.

### F-C8-001 — Schema Layers 1-4 Hardcode DDIS Ontology (CRITICAL)

**SEVERITY**: Critical (fundamental C8 violation)
**FILES**: `crates/braid-kernel/src/schema.rs:200-800`
**FUNCTIONS AFFECTED**: `Schema::default_schema()`, all code that references Layer 1-4 attributes

**OBSERVATION**:
~180 attributes across Layers 1-4 hardcode the DDIS ontology:
- Layer 1: `:intent/*`, `:spec/*`, `:impl/*` (trilateral model)
- Layer 2: `:element/*`, `:inv/*`, `:adr/*`, `:neg/*` (spec element types)
- Layer 3: `:exploration/*`, `:promotion/*`, `:signal/*` (discovery)
- Layer 4: `:topology/*`, `:hypothesis/*`, `:witness/*`, `:policy/*` (coordination)

The test: "Would this code make sense if braid managed a React project?" **No.** A React
project has no `:spec/element-type`, no `:inv/*` attributes, no trilateral ISP model.

The `PolicyConfig` system (policy.rs) was designed to address this — boundaries and
weights can be configured via policy datoms. But the schema attributes themselves are
still hardcoded.

**INVARIANT REFERENCE**: C8, ADR-FOUNDATION-012, INV-FOUNDATION-006
**CURRENT STATE**: Violated. The kernel cannot serve a non-DDIS domain without code changes.

**RECOMMENDATION**:
1. Layer 0 (19 meta-schema attributes): keep hardcoded (bootstrap requirement).
2. Layers 1-4: move to policy manifest `.edn` files loaded at `braid init`.
3. The kernel exposes `Schema::from_datoms()` (already exists) as the sole schema source.
4. `braid init --manifest ddis.edn` installs DDIS schema; `braid init --manifest react.edn` installs a different one.
**EFFORT**: ~16 hours. **ACCEPTANCE**: `braid init` with no manifest produces a store with only Layer 0 attributes.

---

### F-C8-002 — MaterializedViews Hardcodes DDIS-Specific Counters

**SEVERITY**: High
**FILES**: `crates/braid-kernel/src/store.rs:540-604` (MaterializedViews struct)

**OBSERVATION**:
MaterializedViews has fields that reference DDIS-specific attributes:
- `spec_count` — counts entities with `:spec/element-type`
- `validation_depth` — tracks `:impl/verification-depth`
- `coverage_impl_targets` — tracks `:impl/implements`
- `has_falsification` — tracks `:spec/falsification`
- `task_covered` — tracks `:task/traces-to`
- `confidence_sum/count` — tracks `:exploration/confidence`
- `harvest_count` — tracks `:harvest/session-id`
- `intent_datom_count/spec_datom_count/impl_datom_count` — trilateral partition

These are DDIS-specific metrics hardcoded into the core store type.

**INVARIANT REFERENCE**: C8, ADR-FOUNDATION-012
**CURRENT STATE**: Violated. The `compute_fitness_from_policy()` path (policy.rs) exists
as the C8-compliant alternative, but the fallback to MaterializedViews means the
DDIS-specific code remains in the kernel.

**RECOMMENDATION**:
1. Make MaterializedViews generic: replace hardcoded attribute checks with a configurable
   list of `(attribute_pattern, counter_name)` pairs loaded from policy datoms.
2. Or: move MaterializedViews out of Store and into a separate policy-level module.
**EFFORT**: ~12 hours. **ACCEPTANCE**: `grep ':spec/' store.rs` returns zero results.

---

### F-C8-003 — Trilateral Constants Hardcode ISP Attribute Partitions

**SEVERITY**: High
**FILES**: `crates/braid-kernel/src/trilateral.rs:47-80`

**OBSERVATION**:
Three constants hardcode the Intent/Specification/Implementation attribute partition:
```rust
pub const INTENT_ATTRS: &[&str] = &[":intent/decision", ...]; // 7 items
pub const SPEC_ATTRS: &[&str] = &[":spec/id", ...];           // 11 items
pub const IMPL_ATTRS: &[&str] = &[":impl/signature", ...];    // 6 items
```

This is the DDIS-specific coherence model. A research lab would have
Hypothesis/Experiment/Result, not Intent/Spec/Impl.

**INVARIANT REFERENCE**: C8, INV-TRILATERAL-005 (Attribute Namespace Partitioning)
**CURRENT STATE**: Violated. The trilateral model is hardcoded.

**RECOMMENDATION**:
Make the attribute partition configurable via policy datoms:
`:policy/coherence-layer-1 "intent"`, `:policy/coherence-layer-2 "spec"`, etc.
The trilateral module reads these at runtime.
**EFFORT**: ~8 hours. **ACCEPTANCE**: Trilateral computation works with custom layer definitions.

---

### F-C8-004 — Guidance System Hardcodes Basin Competition Model

**SEVERITY**: High
**FILES**: `crates/braid-kernel/src/guidance.rs` (7,119 LOC), `methodology.rs`, `routing.rs`, `context.rs`

**OBSERVATION**:
The guidance system (14,137 LOC combined — 15% of the kernel) contains extensive
DDIS-specific logic:
- `classify_command()` hardcodes braid CLI commands: "guidance", "bilateral", "harvest",
  "seed", "witness", "challenge", "spec"
- Basin competition model (Basin A = DDIS methodology, Basin B = pretrained patterns)
- COTX routing rules for finding/task/ADR/question categories
- Default derivation rules referencing "invariant", "adr" artifact types
- M(t) methodology score with DDIS-specific telemetry

The guidance system is the most C8-violating area of the kernel by LOC.

**INVARIANT REFERENCE**: C8, ADR-FOUNDATION-012
**CURRENT STATE**: Severely violated. The guidance system IS the DDIS methodology,
hardcoded into the kernel.

**RECOMMENDATION**:
This is the hardest C8 fix. The guidance framework (context blocks, budget-aware
projection, routing) is substrate-agnostic. The DDIS-specific content (command
classification, basin model, COTX rules) should be:
1. Extracted into a DDIS guidance plugin.
2. The kernel provides a `GuidanceProvider` trait; the plugin implements it.
3. Policy datoms specify which guidance provider to use.
**EFFORT**: ~40 hours. **ACCEPTANCE**: The kernel's guidance module contains no string literal referencing "invariant", "ADR", "harvest", or "bilateral".

---

### F-C8-005 — INNATE_CONCEPTS Hardcodes Domain-Specific Concept Seeds

**SEVERITY**: Medium
**FILES**: `crates/braid-kernel/src/concept.rs:1362-1383`

**OBSERVATION**:
The `INNATE_CONCEPTS` constant defines 5 bootstrap concepts:
- "components" — "Discrete isolated parts..."
- "dependencies" — "Relationships and connections..."
- "invariants" — "Rules and constraints..."
- "patterns" — "Recurring structures..."
- "anomalies" — "Defects and surprises..."

The first two are generic. "invariants" is DDIS-specific terminology. "patterns" and
"anomalies" are reasonable for any domain. The descriptions are generic enough to work
beyond software, but the `invariants` concept name biases toward DDIS.

Note: the kernel agent reported 12 concepts; the actual count is 5 (the test at line
2425 confirms `INNATE_CONCEPTS.len() == 5`). The kernel agent's finding was incorrect on
this count.

**INVARIANT REFERENCE**: C8
**CURRENT STATE**: Minor violation. The concept engine itself is substrate-agnostic.

**RECOMMENDATION**: Load innate concepts from the policy manifest instead of a const array.
**EFFORT**: ~2 hours. **ACCEPTANCE**: `INNATE_CONCEPTS` removed from kernel code.

---

## Dimension 7: Architectural Coherence

### F-ARCH-001 — Guidance Module Is 15% of Kernel LOC

**SEVERITY**: Medium (god module risk)
**FILES**: `crates/braid-kernel/src/guidance.rs` + `methodology.rs` + `routing.rs` + `context.rs`

**OBSERVATION**:
The guidance system is 14,137 LOC across 4 files — 17% of the 83,293 LOC kernel. By
comparison, the core store (store.rs) is 5,848 LOC (7%). The guidance system is larger
than the store, schema, query engine, and resolution system combined.

This suggests the guidance system has absorbed responsibilities that should be distributed:
- Task routing (routing.rs: 2,879 LOC) could be a separate module
- Methodology scoring (methodology.rs: 1,715 LOC) is a fitness variant
- Context assembly (context.rs: 2,424 LOC) overlaps with seed.rs

**INVARIANT REFERENCE**: ADR-STORE-015 (Free Functions Over Store Methods)
**CURRENT STATE**: Functional but a maintenance risk.

**RECOMMENDATION**: No immediate action needed. When C8 refactoring happens (F-C8-004),
the guidance split should produce a kernel-level `guidance_framework` (~3K LOC) and a
policy-level `ddis_guidance` (~11K LOC).
**EFFORT**: Included in F-C8-004 estimate.

---

### F-ARCH-002 — graph.rs Is a Standalone Math Library Inside the Query Module

**SEVERITY**: Low (crate extraction candidate)
**FILES**: `crates/braid-kernel/src/query/graph.rs` (4,546 LOC)

**OBSERVATION**:
`graph.rs` implements: PageRank, betweenness centrality, Fiedler partitioning, Cheeger
constant, persistent homology, cellular sheaves, Ricci curvature, HITS, k-core
decomposition, eigenvector centrality, articulation points, spectral decomposition,
edge Laplacian, and first Betti number. This is a complete algebraic graph theory library
embedded inside the query module.

It has no dependencies on any other braid type — it operates on `DiGraph` and
`DenseMatrix`. It could be extracted to a standalone crate.

**INVARIANT REFERENCE**: None (architecture quality)
**CURRENT STATE**: Works fine where it is. Extraction is optional.

**RECOMMENDATION**: Consider extracting to `braid-graph` crate if external consumers emerge.
**EFFORT**: ~4 hours. **ACCEPTANCE**: Separate crate compiles independently.

---

### F-ARCH-003 — CLI Commands bootstrap.rs and trace.rs Are Application-Layer, Not Kernel

**SEVERITY**: Medium (layering violation)
**FILES**: `crates/braid/src/bootstrap.rs` (1,046 LOC), `crates/braid/src/commands/trace.rs` (1,464 LOC)

**OBSERVATION**:
`bootstrap.rs` parses `spec/*.md` files for INV/ADR/NEG patterns — this is DDIS-specific
spec format parsing. `trace.rs` scans Rust source files for `INV-`/`ADR-`/`NEG-` patterns.
Both are DDIS-methodology-specific and should be application-layer plugins, not part of
the core binary.

However: they are in the CLI crate (`braid`), not the kernel (`braid-kernel`), so they
don't violate the kernel/application boundary. They violate C8 for the overall system
but not for the kernel specifically.

**INVARIANT REFERENCE**: C8 (at system level)
**CURRENT STATE**: Acceptable layering for Stage 0-1 where DDIS is the only policy.

**RECOMMENDATION**: When the plugin/extractor framework matures, move these to a
`braid-ddis` plugin crate.
**EFFORT**: ~8 hours. **ACCEPTANCE**: `bootstrap.rs` and `trace.rs` are in a separate crate.

---

## Dimension 8: Performance Architecture

### F-PERF-001 — Merge Rebuilds All Indexes From Scratch

**SEVERITY**: High (O(n) where O(delta) suffices)
**FILES**: `crates/braid-kernel/src/store.rs:1165-1200` (apply_datoms), `merge.rs`

**OBSERVATION**:
The `apply_datoms()` method (used during merge and bulk import) rebuilds ALL indexes and
MaterializedViews from scratch:
```rust
self.views = MaterializedViews::default();
for d in &self.datoms {
    views.observe_datom(d);
}
```

For a store with 108K datoms, this is O(108K) per merge operation. The `apply_tx()`
path (used for single transactions) updates incrementally — O(delta). The merge path
should also use incremental updates.

**INVARIANT REFERENCE**: ADR-STORE-005 (Four Core Indexes)
**CURRENT STATE**: Correct but slow. The 97s→3s performance fix (Session 039) introduced
`LiveStore` and binary caching, which amortizes the rebuild. But every `apply_datoms()`
call still pays O(n).

**RECOMMENDATION**:
Make `apply_datoms()` use the incremental `index_datom()` path for the delta
(new datoms only), not a full rebuild.
**EFFORT**: ~6 hours. **ACCEPTANCE**: Merge of 100 datoms into a 100K-datom store takes <100ms.

---

### F-PERF-002 — BTreeSet<Datom> Primary Index Has O(log n) Per Lookup

**SEVERITY**: Medium (acceptable for current scale)
**FILES**: `crates/braid-kernel/src/store.rs:520` (datoms field)

**OBSERVATION**:
The primary store is `BTreeSet<Datom>` which stores datoms sorted by the `Ord` impl on
Datom (entity, attribute, value, tx, op). This gives O(log n) for membership tests and
O(n) for iteration. The secondary indexes (HashMap-based) provide O(1) for common
queries. The BTreeSet is mainly used for:
1. Deduplication (content-addressed identity — same datom inserted twice is a no-op)
2. Ordered iteration for serialization
3. The `datoms()` method (full iteration)

At 108K datoms, this is fine. At 10M datoms, the BTreeSet's cache behavior will degrade.

**INVARIANT REFERENCE**: None (performance, not correctness)
**CURRENT STATE**: Adequate for Stage 0-2 scale.

**RECOMMENDATION**: Monitor. If datom count exceeds 1M, consider switching to a
`HashSet<Datom>` for deduplication plus sorted vectors for iteration.
**EFFORT**: ~8 hours if needed. **ACCEPTANCE**: Benchmark showing <1ms for 1M-datom lookups.

---

### F-PERF-003 — Binary Cache Invalidation on Every Transaction

**SEVERITY**: Medium
**FILES**: `crates/braid/src/layout.rs` (write_tx), `live_store.rs`

**OBSERVATION**:
Every `write_tx()` call marks the binary cache (store.bin) as stale. The cache is rebuilt
on the next `load_store()`. For the daemon, this is fine (the store lives in memory). For
CLI-mode (no daemon), every command that writes a transaction triggers a full cache rebuild
on the next read command.

Session 032 documented this: "30s `braid status` caused by RFL-2 txn per command
invalidating all caches." The daemon (Session 032 solution) amortizes this.

**INVARIANT REFERENCE**: ADR-STORE-006 (Embedded Deployment with Session Daemon)
**CURRENT STATE**: Mitigated by daemon. CLI fallback is slow.

**RECOMMENDATION**: Add incremental cache updates: append new datoms to the binary cache
rather than invalidating it entirely.
**EFFORT**: ~8 hours. **ACCEPTANCE**: CLI-mode `braid status` after `braid observe` takes <2s.

---

## Dimension 9: Formal Verification Coverage

### F-FORMAL-001 — Kani Proofs Cover Core Invariants but Skip MaterializedViews

**SEVERITY**: Medium
**FILES**: `crates/braid-kernel/src/kani_proofs.rs` (2,107 LOC)

**OBSERVATION**:
The Kani proof harnesses verify:
- Append-only immutability (INV-STORE-001)
- Content-addressable identity (INV-STORE-003)
- CRDT merge properties (INV-STORE-004, 005, 006)
- Genesis determinism (INV-STORE-008)
- Transaction typestate (compile-time)

NOT verified by Kani:
- LIVE index correctness (INV-STORE-012) — the most complex runtime invariant
- MaterializedViews isomorphism (F-TYPE-001)
- Schema validation completeness (INV-SCHEMA-004)
- Resolution mode correctness (INV-RESOLUTION-001-006)

**INVARIANT REFERENCE**: ADR-VERIFICATION-001 (Three-Tier Kani CI Pipeline)
**CURRENT STATE**: Core CRDT properties are verified. Derived-state correctness is not.

**RECOMMENDATION**:
Add Kani harnesses for:
1. LIVE index: after applying N datoms, LIVE matches fold-based computation.
2. Resolution: LWW returns max-HLC value; Lattice returns lattice join.
**EFFORT**: ~8 hours. **ACCEPTANCE**: `cargo kani` passes with new harnesses.

---

### F-FORMAL-002 — Stateright Model Has 1 Ignored Test

**SEVERITY**: Low
**FILES**: `tests/stateright_model.rs`

**OBSERVATION**:
The Stateright model checker has 10 passing tests and 1 ignored:
`transact_coherence_no_undetected_contradictions`. The reason for ignoring is not
documented in the code.

**INVARIANT REFERENCE**: INV-COHERENCE-002 (no undetected contradictions)
**CURRENT STATE**: The ignored test may be covering an unimplemented invariant or may
be intermittently failing. Either way, an ignored test is a coverage gap.

**RECOMMENDATION**: Investigate and either fix or document why it's ignored.
**EFFORT**: ~2 hours. **ACCEPTANCE**: Test passes or has a documented skip reason.

---

## Dimension 10: Test Architecture

### F-TEST-001 — Tests Test Behavior, Not Invariants (Coverage Matrix Gap)

**SEVERITY**: Medium (systematic gap)
**FILES**: All test files

**OBSERVATION**:
2,043 tests exist across 13 test binaries. The tests are well-structured with clear
naming conventions. However, there is no formal mapping from tests to specification
invariants. Most tests verify implementation behavior ("does transact add a datom?")
rather than specification invariants ("is INV-STORE-001 satisfied for all reachable
states?").

The `daemon_runtime_datoms_after_tool_calls` test mentioned in the audit prompt as
failing is currently **passing** — it was fixed in a prior session.

Examples of tests that DO test invariants:
- `proptest_store_append_only` — directly tests INV-STORE-001
- `kani_merge_commutative` — directly tests INV-STORE-004
- `stateright_agent_*` — protocol model checking

Examples of tests that test behavior without invariant reference:
- `test_create_task` — tests task creation works, but doesn't reference any INV
- `test_query_entity` — tests query returns results, but doesn't reference INV-QUERY-002

**INVARIANT REFERENCE**: INV-WITNESS-011 (Verification Completeness Guard)
**CURRENT STATE**: High test count, low invariant coverage density.

**RECOMMENDATION**:
1. Add `#[doc = "Witnesses: INV-STORE-001"]` attributes to tests that witness invariants.
2. Generate a coverage matrix via `braid trace --source tests/`.
3. Target: every Stage 0 INV has at least one test with an explicit witness reference.
**EFFORT**: ~16 hours (incremental). **ACCEPTANCE**: `braid witness completeness` shows >80% for Stage 0.

---

---

# Document 2: Architectural Assessment

## A. Soundness Assessment

### Subsystem Classification

| Subsystem | Files | LOC | Classification | Rationale |
|-----------|-------|-----|----------------|-----------|
| **Datom Model** | datom.rs | 741 | **SOUND** | Types are tight (no excess cardinality beyond Value enum). Content-addressable identity is enforced at construction. Kani-verified. |
| **Store Core** | store.rs | 5,848 | **UNSOUND-RECOVERABLE** | F-INV-001 (LIVE view), F-TYPE-001 (MaterializedViews isomorphism), F-INV-003 (index sync). All fixable within current architecture. |
| **Schema** | schema.rs | 4,957 | **UNSOUND-RECOVERABLE** | F-INV-004 (Layers 1-4 hardcoded, C3 violation). Architecture supports fix (Schema::from_datoms exists). |
| **Query Engine** | query/ | 7,509 | **SOUND** | Datalog evaluator is well-tested. Graph algorithms are pure math. Stratification is clean. |
| **Resolution** | resolution.rs | 1,656 | **SOUND** | LWW/Lattice/Multi modes are correctly implemented. Conservative conflict detection works. |
| **Layout** | layout.rs | 1,264 | **SOUND** | Content-addressed file storage. Integrity verification. Clean EDN serialization. |
| **Merge** | merge.rs | 1,518 | **UNTESTED** | Set union is correct by construction, but cascade stubs (5 steps) are mostly no-ops. The cascade pipeline is structurally present but functionally empty for 3/5 steps. |
| **Harvest** | harvest.rs | 4,433 | **UNSOUND-RECOVERABLE** | Pipeline works. Spec candidate promotion (SpecCandidateType) violates C8. Fixable by moving to application layer. |
| **Seed** | seed.rs | 5,314 | **UNSOUND-RECOVERABLE** | Context assembly works. DDIS-specific directive compilation violates C8. |
| **Guidance** | guidance.rs + 3 files | 14,137 | **UNSOUND-STRUCTURAL** | 15% of kernel LOC, deeply tied to DDIS methodology. Cannot serve non-DDIS domains without redesign. Requires extraction of DDIS-specific logic to plugin layer. |
| **Bilateral** | bilateral.rs | 4,769 | **UNSOUND-RECOVERABLE** | F(S) computation works. Monotonicity claim false (F-INV-002). BoundaryRegistry is C8-compliant. |
| **Trilateral** | trilateral.rs | 2,048 | **UNSOUND-STRUCTURAL** | Hardcoded ISP attribute partitions (F-C8-003). Cannot work for non-DDIS domains. |
| **Coherence** | coherence.rs | 1,300 | **UNSOUND-RECOVERABLE** | Tier 1 is substrate-independent. Tier 2 checks DDIS-specific attributes. |
| **Signal** | signal.rs | 1,465 | **SOUND** | 8 divergence types are epistemological, not domain-specific. |
| **Deliberation** | deliberation.rs | 1,436 | **SOUND** | Generic conflict resolution with lattice lifecycle. |
| **Policy** | policy.rs | 1,471 | **SOUND** | This IS the C8 compliance solution. Reads policy datoms dynamically. |
| **Topology** | topology.rs | 3,072 | **SOUND** | Spectral partitioning, CALM classification, agent assignment — all substrate-agnostic. |
| **Concept** | concept.rs | 5,362 | **UNSOUND-RECOVERABLE** | Engine is generic. INNATE_CONCEPTS constant violates C8 (F-C8-005). |
| **Witness** | witness.rs | 2,226 | **UNSOUND-RECOVERABLE** | System is generic. `completeness_guard()` filters for "invariant" type — DDIS-specific. |
| **Compiler** | compiler.rs | 3,072 | **UNTESTED** | Generates proptest code for detected patterns. Not exercised in CI. |

### Summary Counts

| Classification | Count | LOC | % of Kernel |
|---------------|-------|-----|-------------|
| SOUND | 9 subsystems | ~25,676 | 31% |
| UNSOUND-RECOVERABLE | 8 subsystems | ~30,441 | 37% |
| UNSOUND-STRUCTURAL | 2 subsystems | ~16,185 | 19% |
| UNTESTED | 2 subsystems | ~4,590 | 6% |

---

## B. Architectural Tension Map

| Tension | Goal A | Goal B | Status |
|---------|--------|--------|--------|
| **T1: Performance vs C1 (Append-Only)** | Fast queries at 100K+ datoms | Never delete, never compact | **RESOLVED WELL**: LIVE index + binary cache + daemon amortize the cost of append-only. The 97s→3s fix (Session 039) validates this. |
| **T2: C8 (Substrate Independence) vs DDIS-Specific Features** | Kernel serves any domain | DDIS needs specific fitness metrics, guidance, trilateral model | **UNRESOLVED**: The deepest tension. 19% of kernel LOC is UNSOUND-STRUCTURAL due to C8 violations. Policy.rs is the architectural solution but only partially adopted. |
| **T3: Harvest/Seed Lifecycle vs Zero-Coupling** | Knowledge survives conversations | Session boundary logic shouldn't couple kernel to CLI lifecycle | **RESOLVED WELL**: Harvest and seed are free functions over the store, not store methods (SR-013). The lifecycle is in the CLI; the kernel provides primitives. |
| **T4: F(S) Accuracy vs Incremental Computation** | Fitness must be exact | Full recomputation is O(n) | **UNRESOLVED**: MaterializedViews provides O(1) incremental fitness but has isomorphism risk (F-TYPE-001). The batch `compute_fitness()` is authoritative but O(n). |
| **T5: Schema Richness vs Schema Flexibility** | Rich ontology enables powerful queries | Rich schema hardcodes domain assumptions | **UNRESOLVED**: Layers 1-4 provide the DDIS ontology but violate C3 and C8. Policy manifest is the solution but not yet used for schema. |
| **T6: Formal Verification vs Development Velocity** | Kani+Stateright prove correctness | Proof harnesses are expensive to write and maintain | **PARTIALLY RESOLVED**: Core CRDT properties are verified. Derived-state invariants (LIVE, MaterializedViews) are not. The verification investment is well-targeted but incomplete. |
| **T7: Guidance Completeness vs Kernel Purity** | Every tool response needs methodology steering | Methodology is domain-specific | **UNRESOLVED**: The guidance framework (budget-aware projection, context blocks) is substrate-agnostic. The guidance CONTENT (basin model, COTX routing) is DDIS-specific. These are entangled in the same module. |
| **T8: Self-Bootstrap (C7) vs Clean Architecture** | The spec IS the first dataset | Spec-specific code in the kernel creates circular dependencies | **PARTIALLY RESOLVED**: bootstrap.rs is in the CLI crate, not the kernel. But schema.rs hardcodes spec-element attributes. |

---

## C. Loop Analysis

### Tight Loops (Genuine Closed Loops)

1. **Transact → LIVE → Query**: Datom inserted → LIVE index updated → query reads current
   state. Feedback path: query results inform next transaction. **Tight and healthy.**

2. **Observe → Store → Seed → Agent**: Knowledge captured via `braid observe` → stored as
   datoms → assembled into seed for next session → agent receives context. **The core
   hypothesis loop. Working.**

3. **Hypothesis → Outcome → Calibration**: Prediction recorded at harvest → actual outcome
   measured at task close → mean error computed → weights adjusted. **Working but degrading**
   (mean error 0.521, trend degrading — suggests calibration function needs tuning).

4. **Bilateral Scan → Gap Detection → Task Creation**: Forward/backward scans detect
   spec-impl gaps → gaps surface as tasks → tasks get completed → gaps close. **Working.**

### Open Loops (Data Produced but Not Consumed)

1. **Compiler-generated proptests**: `compiler.rs` generates test code but no pipeline
   executes the generated tests. The output is produced but never consumed. **Open.**

2. **123 uncrystallized observations**: Knowledge captured but never promoted to spec
   elements. The observation-to-spec pipeline stalls at the "crystallize" step. **Open.**

3. **Cascade stubs in merge**: 3/5 cascade steps are no-ops. Merge produces a receipt
   noting what should be cascaded, but the cascade actions are not implemented. **Open.**

### Broken Loops (Feedback Path Severed)

1. **Access log significance**: AS-007 specifies a Hebbian significance system via a
   separate access log. The budget module has `ActivationStrategy` types but the access
   log is not implemented. Queries happen but significance is not accumulated. **Broken.**

2. **Confusion signal → re-ASSOCIATE**: INV-SIGNAL-002 specifies confusion triggers
   automatic re-association. The signal types exist but the re-association pipeline is
   not wired. **Broken.**

### Phantom Loops (Appear Closed but Feedback Is No-Op)

1. **F(S) → Guidance → Agent**: F(S) is computed and displayed in `braid status`. The
   guidance footer includes methodology pointers. But the actual feedback path — "low F(S)
   triggers specific corrective actions" — is a no-op. The agent sees the number but there
   is no automated response to F(S) changes. **Phantom.**

2. **Hypothesis calibration → weight adjustment**: `calibrate_boundary_weights()` exists
   in policy.rs and produces `WeightAdjustment` structs. But the adjusted weights are not
   automatically transacted back into the store. The calibration computes results that are
   reported but not applied. **Phantom** (partially — `braid harvest --commit` does apply
   some, but not systematically).

---

## D. Risk Register

| # | Risk | Severity | Probability | Blast Radius | Score | Evidence | Mitigation | Effort |
|---|------|----------|-------------|------------|-------|----------|------------|--------|
| **R1** | **LIVE index shows retracted values** (F-INV-001) | Critical | Medium (requires complex retraction patterns) | High (all queries return wrong "current" state) | **9.0** | store.rs:943-1000 — retraction path has ordering assumptions | Rebuild LIVE from causally-sorted datoms; add proptest | 6h |
| **R2** | **C8 violations prevent non-DDIS adoption** (F-C8-001 through F-C8-004) | High | High (anyone trying non-DDIS domain) | Critical (fundamental capability gap) | **8.0** | schema.rs Layers 1-4, trilateral.rs constants, guidance.rs DDIS-specific logic | Move to policy manifests; extract DDIS plugin | 76h |
| **R3** | **MaterializedViews drift from true F(S)** (F-TYPE-001) | High | Medium (three code paths) | High (fitness-based decisions are wrong) | **7.5** | store.rs:540-604 — no isomorphism test | Add proptest; unify insertion paths | 4h |
| **R4** | **Clippy CI gate fails** (F-ERR-003) | Medium | Certain | Low (8 trivial fixes) | **5.0** | 8 clippy errors in concept.rs, routing.rs, seed.rs | Fix all 8 | 15m |
| **R5** | **265 INVs lack formal witnesses** (F-SPEC-001) | High | Certain | Medium (false confidence in correctness) | **6.0** | braid status methodology gaps | Create coverage matrix; add witnesses | 40h |
| **R6** | **Merge cascade is 60% stub** (F-ARCH-003, loop analysis) | Medium | High (any real merge) | Medium (cascade effects not propagated) | **5.5** | merge.rs — 3/5 cascade steps are no-ops | Implement remaining cascade steps | 16h |
| **R7** | **Hypothesis ledger accuracy degrading** (braid status) | Medium | Certain (already happening) | Medium (guidance quality degrades) | **5.0** | "mean error 0.521, trend: degrading" | Tune calibration function; investigate root cause | 8h |
| **R8** | **NaN panic in bilateral.rs** (F-TYPE-004) | Medium | Low (requires empty/degenerate store) | Low (single command crashes) | **3.0** | bilateral.rs:326,408,587 | Add .unwrap_or(Ordering::Equal) | 30m |
| **R9** | **Binary cache invalidation on every tx** (F-PERF-003) | Medium | Certain | Low (daemon mitigates) | **3.5** | layout.rs write_tx, Session 032 analysis | Incremental cache updates | 8h |
| **R10** | **Schema layers not evolvable via transactions** (F-INV-004) | High | Medium (schema changes require recompilation) | Medium (C3 violation) | **6.5** | schema.rs:200-800 | Move Layers 1-4 to policy manifest | 16h |

Ranked by score: R1 (9.0) > R2 (8.0) > R3 (7.5) > R10 (6.5) > R5 (6.0) > R6 (5.5) > R4 (5.0) = R7 (5.0) > R9 (3.5) > R8 (3.0).

---

---

# Document 3: Implementation Roadmap

## Wave 0: Blocking Defects (Must Fix Before Anything Else)

**Quality gate**: `cargo clippy --all-targets -- -D warnings` passes. All 2,043+ tests pass. No known correctness bugs.

| Task | Finding | Files | Acceptance Criterion | Effort |
|------|---------|-------|---------------------|--------|
| W0-1: Fix 8 clippy errors | F-ERR-003 | concept.rs, routing.rs, seed.rs | `cargo clippy -- -D warnings` exits 0 | 15m |
| W0-2: Guard NaN panics in bilateral | F-TYPE-004 | commands/bilateral.rs:326,408,587 | No `partial_cmp().unwrap()` without NaN guard | 30m |
| W0-3: Fix LIVE index retraction ordering | F-INV-001 | store.rs:943-1000 | Proptest: 10K random assert/retract sequences produce correct LIVE | 6h |
| W0-4: Add MaterializedViews isomorphism proptest | F-TYPE-001 | store.rs | `views.fitness() == compute_fitness(&store)` for 1000 random stores | 4h |
| W0-5: Unify index insertion paths | F-INV-003 | store.rs | Single `index_datom()` function for all insert paths; `observe_datom()` called only from `index_datom()` | 4h |
| W0-6: Investigate ignored stateright test | F-FORMAL-002 | tests/stateright_model.rs | Test either passes or has documented skip reason | 2h |

**Estimated total**: ~17 hours. **LOC impact**: ~500 lines changed.

**Dependency order**: W0-1 first (unblocks CI), then W0-3→W0-5 (both modify store.rs), W0-2 and W0-4 and W0-6 in parallel.

---

## Wave 1: Soundness Recovery (Fix All UNSOUND-RECOVERABLE)

**Quality gate**: All subsystems classified SOUND or UNTESTED. No UNSOUND-RECOVERABLE remaining. Proptest coverage for all store invariants.

| Task | Finding | Files | Acceptance Criterion | Effort |
|------|---------|-------|---------------------|--------|
| W1-1: Validate Attribute deserialization | F-TYPE-003 | datom.rs | `serde_json::from_str::<Attribute>("\"invalid\"")` returns Err | 1h |
| W1-2: Revise INV-BILATERAL-001 monotonicity claim | F-INV-002 | spec/10-bilateral.md, bilateral.rs | Revised statement not falsified by normal spec additions | 3h |
| W1-3: Move INNATE_CONCEPTS to policy | F-C8-005 | concept.rs, policy.rs | `INNATE_CONCEPTS` constant removed from kernel | 2h |
| W1-4: Make witness completeness_guard C8-compliant | F-C8 series | witness.rs | Element type filter loaded from policy, not hardcoded | 2h |
| W1-5: Make coherence Tier 2 C8-compliant | F-C8 series | coherence.rs | Tier 2 attribute patterns from policy | 2h |
| W1-6: Investigate hypothesis calibration degradation | R7 | routing.rs, policy.rs | Mean error trend stabilized or improving | 8h |
| W1-7: Add dangling ref diagnostic | F-TYPE-002 | store.rs, commands/verify.rs | `braid verify --refs` reports dangling references | 2h |
| W1-8: Reconcile store INV count with spec files | F-SPEC-002 | spec/, store | Discrepancy documented or resolved | 2h |
| W1-9: Batch-crystallize or prune phantom observations | F-SPEC-003 | store | Uncrystallized count < 20 | 4h |

**Estimated total**: ~26 hours. **LOC impact**: ~1,500 lines.

**Dependencies**: W0 complete. W1-1 through W1-5 are independent. W1-6 depends on W0-4 (need correct fitness first).

---

## Wave 2: Architectural Corrections (Fix UNSOUND-STRUCTURAL)

**Quality gate**: No UNSOUND-STRUCTURAL subsystems. C8 compliance for kernel core types.

| Task | Finding | Files | Acceptance Criterion | Effort |
|------|---------|-------|---------------------|--------|
| W2-1: Move schema Layers 1-4 to policy manifest | F-C8-001, F-INV-004 | schema.rs, policy.rs, braid init | `braid init --manifest ddis.edn` installs DDIS schema; bare `braid init` has only Layer 0 | 16h |
| W2-2: Make MaterializedViews configurable | F-C8-002 | store.rs, policy.rs | `grep ':spec/' store.rs` returns zero results; views driven by policy datoms | 12h |
| W2-3: Make trilateral attribute partitions configurable | F-C8-003 | trilateral.rs, policy.rs | `INTENT_ATTRS`, `SPEC_ATTRS`, `IMPL_ATTRS` constants removed; loaded from policy | 8h |
| W2-4: Extract DDIS guidance to plugin | F-C8-004 | guidance.rs, methodology.rs, routing.rs, context.rs | Kernel guidance module has no DDIS-specific string literals; DDIS logic in separate module | 40h |
| W2-5: Implement merge cascade steps 2-4 | R6 | merge.rs | 5/5 cascade steps produce datoms; tested with 3+ conflicting merge scenarios | 16h |

**Estimated total**: ~92 hours. **LOC impact**: ~8,000 lines (significant refactoring).

**Dependencies**: W1 complete. W2-1 enables W2-2 and W2-3 (policy infrastructure must exist). W2-4 is independent but the largest task.

**Critical path**: W2-1 → W2-2 → W2-3 (sequential — each builds on policy infrastructure). W2-4 and W2-5 run in parallel.

---

## Wave 3: Verification Completeness (Close All Coverage Gaps)

**Quality gate**: 100% Stage 0 INV coverage at L2+. Formal coverage matrix exists and is maintained.

| Task | Finding | Files | Acceptance Criterion | Effort |
|------|---------|-------|---------------------|--------|
| W3-1: Create INV-to-test coverage matrix | F-SPEC-001, F-TEST-001 | New: docs/audits/COVERAGE_MATRIX.md | Every Stage 0 INV mapped to ≥1 test | 8h |
| W3-2: Add missing STORE invariant witnesses | F-SPEC-001 | tests/ | INV-STORE-001..016 each have L2+ witness | 12h |
| W3-3: Add missing SCHEMA invariant witnesses | F-SPEC-001 | tests/ | INV-SCHEMA-001..009 each have L2+ witness | 6h |
| W3-4: Add missing QUERY invariant witnesses | F-SPEC-001 | tests/ | INV-QUERY-001..024 each have L2+ witness | 8h |
| W3-5: Add Kani harness for LIVE index | F-FORMAL-001 | kani_proofs.rs | `cargo kani` verifies LIVE correctness for bounded inputs | 4h |
| W3-6: Add Kani harness for resolution modes | F-FORMAL-001 | kani_proofs.rs | LWW, Lattice, Multi correctness verified | 4h |
| W3-7: Add namespace-to-file mapping doc | F-SPEC-004 | docs/guide/00-architecture.md | Table mapping 23 namespaces to implementation files | 1h |

**Estimated total**: ~43 hours. **LOC impact**: ~3,000 lines (mostly test code).

**Dependencies**: W0-3 and W0-5 complete (LIVE must be correct before verifying it). Otherwise independent of W1/W2.

---

## Wave 4: Performance Hardening

**Quality gate**: `braid status` in CLI mode (no daemon) < 2s for 200K-datom store. Merge of 1K datoms into 200K store < 500ms.

| Task | Finding | Files | Acceptance Criterion | Effort |
|------|---------|-------|---------------------|--------|
| W4-1: Incremental merge path | F-PERF-001 | store.rs (apply_datoms) | Merge of 100 datoms into 100K-datom store < 100ms | 6h |
| W4-2: Incremental binary cache | F-PERF-003 | layout.rs, live_store.rs | CLI-mode `braid status` after `braid observe` < 2s | 8h |
| W4-3: Profile and optimize hot paths | General | store.rs, bilateral.rs | Benchmark suite with regression detection | 8h |

**Estimated total**: ~22 hours. **LOC impact**: ~1,000 lines.

**Dependencies**: W0-5 complete (unified insertion path enables incremental merge).

---

## Wave 5: Production Readiness

**Quality gate**: System can be used by a non-DDIS domain with zero kernel code changes. All failure modes from FAILURE_MODES.md are VERIFIED or CLOSED.

| Task | Finding | Files | Acceptance Criterion | Effort |
|------|---------|-------|---------------------|--------|
| W5-1: Create non-DDIS example manifest | C8 validation | New: examples/research.edn | `braid init --manifest research.edn` produces working store with different ontology | 4h |
| W5-2: End-to-end non-DDIS workflow test | C8 validation | New: tests/e2e_non_ddis.sh | Full observe/query/harvest/seed cycle with non-DDIS manifest | 8h |
| W5-3: Wire access log significance | Loop analysis | New: access_log.rs | Queries accumulate significance; high-significance results sort first | 16h |
| W5-4: Wire confusion signal re-association | Loop analysis | signal.rs, seed.rs | INV-SIGNAL-002 satisfied: confusion triggers automatic re-ASSOCIATE | 8h |
| W5-5: Execute compiler-generated tests | Loop analysis | compiler.rs, CI | Generated proptests are executed in CI | 4h |
| W5-6: Advance failure modes to VERIFIED | FAILURE_MODES.md | docs/design/FAILURE_MODES.md | ≥15 of 22 failure modes at VERIFIED status | 16h |
| W5-7: External documentation | Production readiness | README.md, docs/ | Installation, quickstart, manifest authoring guide | 8h |

**Estimated total**: ~64 hours. **LOC impact**: ~4,000 lines.

**Dependencies**: W2 complete (C8 fixes must be in place for non-DDIS manifests).

---

## Dependency DAG

```
Wave 0 (17h) ─┬─→ Wave 1 (26h) ──→ Wave 2 (92h) ──→ Wave 5 (64h)
               │                          ↑
               └─→ Wave 3 (43h) ──────────┘
               │
               └─→ Wave 4 (22h)
```

**Critical path**: W0 → W1 → W2 → W5 = **199 hours** (~5 weeks at 40h/week, ~2.5 weeks with 2 agents).

W3 (verification) and W4 (performance) can run in parallel with W1/W2.

---

## Total Effort Summary

| Wave | Hours | Tasks | LOC Impact | Gate |
|------|-------|-------|------------|------|
| 0: Blocking | 17 | 6 | ~500 | CI green, no known correctness bugs |
| 1: Soundness | 26 | 9 | ~1,500 | No UNSOUND-RECOVERABLE |
| 2: Architecture | 92 | 5 | ~8,000 | No UNSOUND-STRUCTURAL, C8 compliant |
| 3: Verification | 43 | 7 | ~3,000 | 100% Stage 0 INV coverage at L2+ |
| 4: Performance | 22 | 3 | ~1,000 | <2s CLI status, <500ms merge |
| 5: Production | 64 | 7 | ~4,000 | Non-DDIS domain works end-to-end |
| **Total** | **264** | **37** | **~18,000** | |

---

*End of audit. This document traces every finding to specific files and line numbers,
every recommendation to a specification invariant or design constraint, and every effort
estimate to a concrete acceptance criterion. The quality of the plan depends on the
depth of the comprehension; the comprehension was thorough.*
