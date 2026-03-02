# Cleanroom Audit Report: ddis-cli Codebase

**Date**: 2026-03-01
**Method**: 5 parallel deep-audit agents, each targeting a distinct domain
**Scope**: All code in `ddis-cli/` (~35K+ LOC Go, 44 commands, 27 test packages)
**Domains**: Event pipeline, Storage, Causal/Projector/Witness, Validator/Consistency/Parser, CLI wiring/Coverage/Drift

---

## Executive Summary

| Severity | Count | Status |
|----------|-------|--------|
| HIGH (data loss / correctness) | 4 | Spec remediation planned |
| MEDIUM (silent data loss / logic gaps) | 28 | Spec remediation planned |
| LOW (cosmetic / minor fragility) | 20+ | Bundled with MEDIUM fixes |

**Key themes**:
1. Applier field coverage gaps causing silent data loss on update events
2. Parser code-block vulnerability in idle state (invariants + negative specs)
3. Dead code/tables (`event_provenance`, `formatInvariant`, `ManifestUpdatePayload`)
4. Import quality gate data collapse bug (all gates -> APP-G-0)
5. Projector content type coverage gap (missing 4 of 8 types)
6. Non-atomic dual-write pattern (SQL then event, crash = split-brain)
7. Drift/triage contradiction integration gap (always hardcoded 0)

---

## HIGH Severity Findings (4)

### H1: Import Quality Gate Collapse — ALL Gates to APP-G-0

**Location**: `cli/importcmd.go:246`, `cli/materialize.go:370`, `events/payloads.go:96`

**Root cause**: Import reads `gate_id` from DB (`SELECT gate_id, title, predicate FROM quality_gates`) but creates payload with only `Title` and `Predicate`:
```go
p := events.QualityGatePayload{Title: title, Predicate: predicate}
```
`QualityGatePayload.GateNumber` (an `int`) defaults to Go zero value `0`. The applier then generates:
```go
gateID := fmt.Sprintf("APP-G-%d", p.GateNumber)  // always "APP-G-0"
```
All gates collapse to a single row via `INSERT OR REPLACE` on `UNIQUE(spec_id, gate_id)`.

**Impact**: Complete data loss of quality gate identity during import->materialize round-trip. Only the last gate survives.

**Violates**: APP-INV-072 (Event Content Completeness), APP-INV-078 (Import Equivalence)

---

### H2: `event_provenance` Table is Dead Code

**Location**: `storage/schema.go:489-499`, entire codebase

**Evidence**:
- Table created in schema with 2 indexes (`idx_provenance_element`, `idx_provenance_event`)
- Cleaned up in `deleteSpecData` (`insert.go:713`)
- **No INSERT function exists anywhere**
- **No SELECT function exists anywhere**
- No model struct exists

**Impact**: APP-INV-084 (Causal Provenance) is unimplemented at the storage layer. The `Provenance()` function in `causal/dag.go` does a linear scan of the event log instead, but only checks 2 of 10+ payload field names (see M11).

---

### H3: Invariant Parser Has No Code-Block Tracking in Idle State

**Location**: `parser/invariants.go:30-56`

**Evidence**: The invariant extractor's `idle` state directly matches `InvHeaderRe` on every line without checking if the line is inside a fenced code block:
```go
case idle:
    if m := InvHeaderRe.FindStringSubmatch(trimmed); m != nil {
        state = headerSeen  // No code-block check!
```

**Contrast**: The ADR extractor (`adrs.go:72-93`) has proper code-block tracking with `inCodeBlock` flag and `codeFence` string, explicitly documented as needed.

**Impact**: `**APP-INV-NNN: Example Title**` inside markdown code fences is extracted as a real invariant. Inflates invariant counts, corrupts coverage, confuses drift.

---

### H4: Negative Spec Parser Has No Code-Block Tracking

**Location**: `parser/negspecs.go:10-63`

**Evidence**: Iterates all lines looking for `NegSpecRe` matches (`**DO NOT**`) with zero code-block awareness. No `inCodeBlock` flag, no fence tracking.

**Impact**: `**DO NOT** use raw text` inside a code example is extracted as a real negative spec.

---

## MEDIUM Severity Findings (28)

### Theme A: Applier Field Coverage Gaps

| ID | Location | Issue |
|----|----------|-------|
| M1 | `materialize.go:266-282` | `UpdateInvariant` handles only `title`, `statement`, `semi_formal` (3/7). Missing: `violation_scenario`, `validation_method`, `why_this_matters` |
| M2 | `materialize.go:297-311` | `UpdateADR` handles only `title`, `decision` (2/7). Missing: `problem`, `options`, `consequences`, `tests`, `status` |
| M3 | `materialize.go:332-336` | `InsertChallenge` drops `Score`, `Detail` from `ChallengePayload`; hardcodes `challenged_by='system'` |
| M4 | `materialize.go:289-295` | `InsertADR` ignores `Options` field; `chosen_option` column never populated via event path |
| M5 | `materialize.go:353-358` | `InsertCrossRef` hardcodes `ref_type='section'`; `CrossRefPayload` lacks `RefType` field |

**Common pattern**: The switch statements in update methods silently fall through for unhandled fields. No error, no log — the field update is simply lost.

### Theme B: Stream Integrity

| ID | Location | Issue |
|----|----------|-------|
| M6 | `importcmd.go:268` | Witness events (Stream 3 type per schema) may be written to wrong stream file |
| M7 | `processors.go:109` | Derived events use `trigger.Stream` (typically Stream 2) but `implementation_finding` is a Stream 3 type. Also uses string literal instead of constant. |
| M8 | `crystallize.go:132,144,155` | All three `emitEvent` calls pass `""` as specHash — no provenance tracking |

### Theme C: Projection Completeness

| ID | Location | Issue |
|----|----------|-------|
| M12 | `projector/render.go` | `RenderModule()` renders invariants, ADRs, negative specs but omits: glossary terms, cross-references, quality gates, sections. `Sections` field exists in `ModuleSpec` but `RenderModule` never iterates it. |

### Theme D: Provenance/Blame Completeness

| ID | Location | Issue |
|----|----------|-------|
| M11 | `causal/dag.go:77-96` | `Provenance()` only checks `payload["id"]` and `payload["invariant_id"]`. Misses: `"path"` (sections), `"term"` (glossary), `"pattern"` (neg specs), `"name"` (modules), `"source"` (cross-refs), `"gate_id"` (quality gates). `ddis blame` returns empty for 6 of 8 element types. |

### Theme E: Atomicity

| ID | Location | Issue |
|----|----------|-------|
| M13 | `cli/witness.go:147-168` | Witness/challenge write SQL first, then emit event. Crash between = SQL has witness but event log doesn't. Violates APP-INV-071 (Log Canonicality). Same pattern in `cli/challenge.go:130-141`. |

### Theme F: Measurement Completeness

| ID | Location | Issue |
|----|----------|-------|
| M17 | `coverage/coverage.go:86` | `storage.ListADRs()` returns ALL ADRs including `status='superseded'`. Superseded ADRs inflate denominator, preventing 100% coverage. |
| M18 | `drift/drift.go:187` | `contradictions := 0` — never incremented. `ddis contradict` results never feed into drift. |
| M19 | `drift.go:46-48` | `ddis drift --code` specified in spec (`code-bridge.md:318`) but not implemented. |
| M28 | `triage/triage.go:274` | `signals.Contradictions = 0.0` — hardcoded, inflating F(S) by 15% (contradiction weight). |

### Theme G: Merge Correctness

| ID | Location | Issue |
|----|----------|-------|
| M14 | `causal/dag.go:105-113` | Same-ID events with different content: `Merge(A,B)` keeps A's version, `Merge(B,A)` keeps B's. Breaks commutativity required by semilattice (APP-INV-081). Test `TestMerge_Commutativity` uses disjoint IDs, doesn't exercise this case. |

### Theme H: Validator Strictness

| ID | Location | Issue |
|----|----------|-------|
| M15 | `validator/checks.go:1330-1332` | Check 17 (Challenge Freshness): ANY unchallenged witness fails entire validation. 1/97 unchallenged = FAIL. Too strict for iterative development. |

### Theme I: Dead Code

| ID | Location | Issue |
|----|----------|-------|
| M9 | `crystallize.go:223-281` | `formatInvariant()` and `formatADR()` — 60 lines, never called after event-only path |
| M10 | `payloads.go:125,132` | `ManifestUpdatePayload` and `SnapshotPayload` defined, never instantiated. `TypeManifestUpdated` and `TypeSnapshotCreated` in schema but never emitted. |

### Theme J: CLI/UX + Minor Logic

| ID | Location | Issue |
|----|----------|-------|
| M20 | `annotate/grammar.go:13` | Annotation regex matches `ddis:maintains` anywhere in comment, including prose about annotations |
| M21 | `cli/next.go:305` | `modeHint` reads `payload["mode"]` which is never written by `discover.go:159` |
| M22 | `discover/thread.go:147` | `SaveThread` uses `O_APPEND` — duplicate thread entries accumulate |
| M23 | `crystallize.go:132` | crystallize ignores `--events` flag, always writes to `.ddis/events/` |
| M24 | `validator/reachability.go:199` | `initialState := transitions[0].fromState` — fragile if table unordered |
| M25 | `consistency/sat.go:347-363` | `sanitize()` strips structure — `render(spec, output)` = `render(spec_output)` — false collisions |
| M26 | `consistency/smt.go:325-327` | All functions declared arity 1 — loses multi-argument semantics |
| M27 | `consistency/llm.go:168` | LLM 3/3 confidence (0.95) equals SMT (0.95) despite non-determinism |

---

## LOW Severity Findings (20+)

| ID | Location | Issue |
|----|----------|-------|
| L1 | `root.go:199` | `snapshotCmd` missing GroupID assignment |
| L2 | `root.go:189` | `versionCmd` missing GroupID assignment |
| L3 | `projector/render.go:134` | `strings.Title` deprecated since Go 1.18 |
| L4 | `projector/render.go:61` | Projector uses bold format for invariants; parser expects heading as canonical |
| L5 | `witness/eval.go:125` | `RecordEval` never sets `CodeHash` |
| L6 | `witness/eval.go:144` | LLM responses with preamble classify as "inconclusive" |
| L7 | `cli/bisect.go:36` | `validation-fail` predicate documented but not implemented |
| L8 | `cli/bisect.go:97` | Hand-crafted JSON output vulnerable to injection |
| L9 | `cli/replay.go:103` | Silently creates `replay.db` without checking for existing file |
| L10 | `cli/witness.go:88` | Argument parsing inconsistency between list/check and record modes |
| L11 | `storage/db.go:50` | PRAGMA `foreign_keys=ON` not guaranteed on pooled connections |
| L12 | `storage/insert.go:448` | `InsertSearchVector` function never called; `search_vectors` table always empty |
| L13 | `storage/queries.go:68` | `GetSpecIndex` omits `parent_spec_id` column |
| L14 | `storage/storage_test.go:105` | Test table list missing 4 tables: `invariant_witnesses`, `challenge_results`, `snapshots`, `event_provenance` |
| L15 | `storage/models.go:319` | Stale comment on `EvidenceType` (missing `'eval'`) |
| L16 | `storage/models.go:335` | Stale comment on `Verdict` (missing `'provisional'`) |
| L17 | Write-only tables | `verification_checks`, `formatting_hints`, `budget_entries`, `state_machine_cells`, `meta_instructions`, `code_annotations` — all INSERT, no SELECT |
| L18 | `parser/sections.go:42` | `~N` path dedup suffix can break cross-ref resolution |
| L19 | `consistency/semantic.go:18` | Comment typo: "analyzeSemanticmantic" |
| L20 | `validator/checks.go:76-127` | Checks 2, 3, 4 never set `result.Passed = false` — purely advisory |
| L21 | `validator/checks.go:593` | Check 7 asymmetric failure — defined-but-not-registered = warning, registered-but-not-defined = error |
| L22 | `cli/next.go:183` | `StateTriaged` with no ThreadID silently falls through |

---

## Remediation Plan

See `.ddis/specs/cleanroom-audit-remediation-plan.md` for the full spec-first remediation plan.

### Spec Changes (Part A)

**8 new invariants** (APP-INV-106..113):
| ID | Module | Property |
|----|--------|----------|
| APP-INV-106 | event-sourcing | Applier field coverage completeness |
| APP-INV-107 | event-sourcing | Stream-type affinity enforcement |
| APP-INV-108 | triage-workflow | Measurement signal completeness |
| APP-INV-109 | event-sourcing | Event-SQL write atomicity |
| APP-INV-110 | event-sourcing | Quality gate identity preservation |
| APP-INV-111 | parse-pipeline | Parser code-block isolation |
| APP-INV-112 | event-sourcing | Provenance table population |
| APP-INV-113 | query-validation | Validator strictness proportionality |

**4 new ADRs** (APP-ADR-077..080):
| ID | Module | Decision |
|----|--------|----------|
| APP-ADR-077 | event-sourcing | Applier update method completeness enforcement (exhaustive switch + reflection test) |
| APP-ADR-078 | event-sourcing | Materialize reads all streams (stream membership = storage concern, not fold concern) |
| APP-ADR-079 | event-sourcing | Dead code pruning policy (remove immediately, no forward-compat shims) |
| APP-ADR-080 | triage-workflow | Measurement signal wiring strategy (progressive: wire existing, defer unbuilt) |

### Existing Invariant Violations (Impl Fix Only)

| Finding | Violated INV | Fix |
|---------|-------------|-----|
| M8 | APP-INV-072 | Pass `specHashFromDB()` in crystallize |
| M12 | APP-INV-076/077 | Add missing content types to projector |
| M14 | APP-INV-081 | Deterministic tiebreaker for same-ID merge |
| M20 | APP-INV-017 | Anchor annotation regex |
| M22 | APP-INV-020 | Thread dedup on write |
| M24 | APP-INV-062 | Find initial state by 0 in-degree |
| M25 | APP-INV-021 | Preserve structure in SAT sanitize |

### Implementation Phases (Part B)

1. **Spec crystallization + validation** (DDIS-first)
2. **HIGH severity fixes** (H1-H4: quality gate collapse, parser code-block)
3. **Applier field coverage** (M1-M5: exhaustive switch statements)
4. **Stream + provenance + atomicity** (M6-M8, M11, M13, H2)
5. **Measurement wiring** (M17-M19, M28: coverage/drift/triage)
6. **Dead code removal + validator fix + minor fixes** (M9-M10, M15, M20-M25)
7. **Tests + full verification** (8 behavioral tests, parser tests, witness/challenge all 113 invariants)

### Expected Outcome

| Metric | Before | After |
|--------|--------|-------|
| Invariants | 105 | 113 (+8) |
| ADRs | 76 | 80 (+4) |
| HIGH severity bugs | 4 | 0 |
| MEDIUM severity bugs | 28 | 0 |
| Dead code items | 6 | 0 |
| Applier field coverage | ~50% | 100% |
| Quality gate round-trip | BROKEN | CORRECT |
| Parser code-block safety | 2/4 extractors | 4/4 extractors |
| Provenance table | EMPTY | POPULATED |
| Measurement signals | 3 hardcoded | all from DB |
