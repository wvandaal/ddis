# Cleanroom Audit — Round 2

**Thread**: `t-cleanroom-r2-2026-02-28`
**Date**: 2026-02-28
**Method**: 6 parallel deep-dive agents, manual verification of all findings

## Verified Findings

### CRITICAL (P0)

**F-07: FK Constraint — INSERT OR REPLACE on witnesses orphans challenge FK refs**
- File: `internal/storage/insert.go:510-531`
- Root cause: `INSERT OR REPLACE INTO invariant_witnesses` deletes the old row (destroying its ID)
  then inserts a new row with a new auto-increment ID. `challenge_results.witness_id` references
  `invariant_witnesses(id)` without CASCADE rules. The pre-clear at line 512 deletes challenges
  first, but the FK can still fail when `InsertChallengeResult` runs because the witness ID
  in the caller's struct may be stale.
- Fix: Change to `INSERT ... ON CONFLICT(spec_id, invariant_id) DO UPDATE` which preserves the
  row ID, or add `ON DELETE CASCADE` to the FK constraint.

**F-08: Module relationships not applied during materialize**
- File: `internal/cli/materialize.go:339-343`
- Root cause: `InsertModule()` receives `ModulePayload` with `Maintains`, `Interfaces`,
  `Implements`, `Adjacent` arrays but only stores `module_name` and `domain`.
  The `module_relationships` table is never populated.
- Impact: Projector can't filter invariants/ADRs by module ownership. Round-trip fidelity
  drops to ~50% for modules. Breaks APP-INV-078 (Import Equivalence) and APP-INV-087.
- Fix: After INSERT into modules, iterate payload relationship arrays and INSERT into
  `module_relationships` table.

### HIGH (P1)

**F-09: ADR options table never imported/materialized/projected**
- Files: `internal/cli/importcmd.go:131`, `internal/events/payloads.go`, fold.go, project.go
- Root cause: Import query selects only 6 ADR fields; no JOIN on `adr_options` table.
  No event type exists for ADR options. Materialize can't populate `adr_options`.
  Projector doesn't render options.
- Impact: Round-trip fidelity ~67% for ADRs. Option rationales (pros, cons, why_not) are lost.
- Fix: Add `Options` field to ADRPayload (already exists as string); populate during import
  by concatenating option labels. Full structured options require a new event type.
- Deferred: Structured ADR options events (new event type) — future work.

**F-10: Drift contradictions counter never incremented**
- File: `internal/drift/drift.go:187`
- Root cause: `contradictions := 0` declared but never incremented. The drift formula
  includes `2*contradictions` but the value is always 0.
- Impact: Drift is never classified as "contradictory". The 2x penalty for contradictions
  is dead code.
- Fix: Integrate contradiction detection from `internal/consistency/` package. After
  unimplemented/unspecified detection (5a/5b), run consistency check and count results.

**F-11: --from-snapshot flag defined but never used**
- File: `internal/cli/materialize.go:52,77`
- Root cause: `materializeFromSnap` flag is defined but `runMaterialize()` never reads it.
  `runMaterializeInternal()` always creates a fresh DB and does full fold.
- Impact: Snapshot optimization is unavailable from CLI. `FoldFrom()` exists but is
  unreachable.
- Fix: Pass flag to `runMaterializeInternal()`, load latest snapshot, use `FoldFrom()`.

**F-12: Integer division in refine confidence scoring**
- File: `internal/refine/audit.go:163,171,179,196`
- Root cause: `10 * sid.CompleteElements / sid.TotalElements` uses Go integer division.
  For 7/10 = 0.7, the expression becomes `10 * 7 / 10 = 70 / 10 = 7` (correct).
  But for 1/3 = 0.33, it becomes `10 * 1 / 3 = 10 / 3 = 3` instead of 3.33.
- Impact: RALPH refinement dimension selection uses imprecise confidence scores.
  The wrong dimension may be selected for improvement.
- Fix: Reorder to `(10 * numerator + denominator/2) / denominator` for proper rounding,
  or use float intermediate.

### MEDIUM (P2)

**F-13: Cascade excludes maintains relationships**
- File: `internal/cascade/cascade.go:100`
- Root cause: `if r.RelType == "maintains" { continue }` — owner module is excluded.
- Impact: When an invariant changes, the module that maintains it is NOT flagged as affected.
- Fix: Remove the exclusion or add a separate "owner" category to the output.

**F-14: implorder/progress have empty dependency graphs**
- Files: `internal/implorder/implorder.go:125`, `internal/progress/progress.go:119`
- Root cause: `edgeSet` and `deps` maps are initialized but never populated. Comments explain
  interface relationships cause cycles. All invariants have in-degree 0.
- Impact: "Implementation order" is actually authority ranking. Labels are misleading.
- Fix: Populate edges from maintains→implements chains (avoiding cycles), or retitle
  the output as "authority ranking" rather than "dependency order".

### FALSE POSITIVES (Rejected)

- **VerifyEvidenceChain nil-slice panic**: In Go, `range nil` for a slice is safe (no iteration).
  The code at evidence.go:40 uses `for i := range witnesses {}` which is a no-op when
  `witnesses == nil`. Not a bug.

## New Spec Elements

| ID | Type | Title | Module |
|----|------|-------|--------|
| APP-INV-106 | Invariant | Drift contradiction integration | lifecycle-ops |
| APP-INV-107 | Invariant | Witness ID stability under upsert | lifecycle-ops |
| APP-INV-108 | Invariant | Module relationship materialization completeness | event-sourcing |
| APP-INV-109 | Invariant | Refine confidence floating-point fidelity | auto-prompting |
| APP-ADR-077 | ADR | ON CONFLICT DO UPDATE for witness upsert | lifecycle-ops |
| APP-ADR-078 | ADR | Snapshot-accelerated fold CLI integration | event-sourcing |

Note: IDs 103-105 were initially assigned but collided with existing spec elements
(APP-INV-103: Witness Lifecycle Completeness, APP-INV-104: Task Witness Enrichment,
APP-INV-105: CI Witness Gate). Re-assigned to 107-109. Stale events for the colliding
IDs exist in the stream but will be overridden by the authoritative definitions.

## Implementation Status

### Fixes Applied

| Fix | Status | Commit |
|-----|--------|--------|
| F-07: InsertWitness ON CONFLICT DO UPDATE | DONE | storage/insert.go |
| F-08: Module relationships in InsertModule | DONE | cli/materialize.go |
| F-10: Drift contradiction integration | DONE | drift/drift.go |
| F-11: --from-snapshot wired to FoldFrom | DONE | cli/materialize.go |
| F-12: roundedDiv for integer division | DONE | refine/audit.go |
| Migration 3: Stale FK reference in challenge_results | DONE | storage/db.go |
| F-09: ADR options (deferred) | DEFERRED | Needs new event type |
| F-13: Cascade maintains exclusion | DEFERRED | Medium severity |
| F-14: Empty dependency graphs | DEFERRED | Medium severity |

### Migration 3: challenge_results FK to _witnesses_old

SQLite 3.25+ auto-updates FK references during ALTER TABLE RENAME. When Migration 2
renamed `invariant_witnesses` → `_witnesses_old`, SQLite automatically rewrote the FK in
`challenge_results` to point at `_witnesses_old` instead of `invariant_witnesses`. This
caused all challenge insertions to fail with FOREIGN KEY constraint errors. Migration 3
detects the stale reference, drops and recreates `challenge_results`, and cleans up the
`_witnesses_old` artifact table.

### Quality Gates (Final)

- **Build**: clean (go build ./...)
- **Vet**: clean (go vet ./...)
- **Tests**: 610+ tests, all passing
- **Validation**: 18/19 (Check 11 proportional weight — pre-existing)
- **Coverage**: 100% (109/109 INV, 78/78 ADR)
- **Drift**: 0
- **Witnesses**: 106/106 valid
- **Challenges**: 106/106 confirmed, 0 refuted
