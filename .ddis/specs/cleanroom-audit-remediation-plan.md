# Cleanroom Audit Remediation Plan — Spec-First, Then Implement

**Date**: 2026-03-01
**Companion document**: `cleanroom-audit-2026-03-01.md` (findings report)
**Method**: Classify findings into (A) violations of existing invariants and (B) gaps needing new spec elements. Crystallize new spec elements first. Validate. Then implement.

---

## Context

A deep cleanroom audit of the ddis-cli codebase (5 parallel agents, ~60 findings) revealed **4 HIGH**, **28 MEDIUM**, and **20+ LOW** severity issues across event pipeline, storage, parser, validator, and CLI domains. Key themes: applier field coverage gaps causing silent data loss, parser code-block vulnerability, dead code/tables, import quality gate data collapse, projector content type gaps, non-atomic dual-write, and drift/triage contradiction integration gap.

**Approach**: Classify findings into (A) violations of existing invariants (implementation fix only) and (B) gaps needing new spec elements. Crystallize 8 new invariants + 4 new ADRs via DDIS first. Validate to 0 errors. Then implement all fixes.

**Current state**: 105 APP-INV invariants, 76 APP-ADRs, 19/19 validation, 100% coverage, 0 drift.

---

## Part A: Specification (8 New Invariants, 4 New ADRs)

### Findings Covered by EXISTING Invariants (Impl Fix Only, No New Spec)

| Finding | Existing INV | Fix |
|---------|-------------|-----|
| M8 (crystallize empty spec hash) | APP-INV-072 | Pass `specHashFromDB()` in crystallize emitEvent calls |
| M12 (projector missing types) | APP-INV-076/077 | Add glossary, cross-ref, quality gate, section rendering to projector |
| M14 (merge commutativity bias) | APP-INV-081 | Use deterministic tiebreaker (e.g., content hash) for same-ID events |
| M20 (annotation regex false positives) | APP-INV-017 | Anchor regex to start-of-comment or add negative lookbehind |
| M22 (thread JSONL duplicates) | APP-INV-020 | Dedup on thread ID before append |
| M24 (Check 20 initial state assumption) | APP-INV-062 | Find initial state by graph analysis (0 in-degree), not first row |
| M25 (SAT sanitize collisions) | APP-INV-021 | Preserve parenthesized structure in variable names |

### New Invariants (APP-INV-106..113)

#### APP-INV-106: Applier Field Coverage Completeness
- **Module**: event-sourcing
- **Addresses**: M1, M2, M3, M4, M5
- **Property**: `forall event type T with Applier method A: fields_written(A) = content_fields(T.Payload)`
- **Statement**: Every non-metadata field in a content-bearing event payload must be mapped to a SQL column by the corresponding Applier method. No field may be silently dropped.
- **Semi-formal**: `|content_fields(P) \ handled_fields(A)| = 0`
- **Violation**: `UpdateInvariant` receives `NewValues["violation_scenario"]`, switch has no case, field silently ignored. DB state diverges from event log.
- **Validation**: Reflection test enumerating payload struct fields vs applier switch cases. Round-trip test: update all fields via event, materialize, verify all present.

#### APP-INV-107: Stream-Type Affinity Enforcement
- **Module**: event-sourcing
- **Addresses**: M6, M7
- **Property**: `forall emitted events E: E.Stream = canonical_stream(E.Type)`
- **Statement**: Every event must be emitted to the stream designated for its type in the schema. Derived events inherit stream from their type, not from the triggering event.
- **Semi-formal**: `E.Stream = streamEventTypes[E.Type]`
- **Violation**: Derived `implementation_finding` events inherit `trigger.Stream` (Stream 2) instead of Stream 3.
- **Validation**: Static analysis of all `emitEvent` / `NewEvent` call sites. `ValidateEvent` enforced on all emit paths.

#### APP-INV-108: Measurement Signal Completeness
- **Module**: triage-workflow (interfaces event-sourcing)
- **Addresses**: M17, M18, M19, M28
- **Property**: `forall fitness signals S: S is computed from DB state, not hardcoded constants`
- **Statement**: Coverage denominator excludes superseded ADRs. Drift contradictions reflects actual contradiction count. Triage fitness contradictions matches drift output.
- **Semi-formal**: `coverage.denom = count(adrs WHERE status != 'superseded')`, `drift.contradictions = count(detected_contradictions)`, `fitness.contradictions = drift.contradictions`
- **Violation**: Coverage counts 2 superseded ADRs in denominator. Drift reports 0 contradictions despite `ddis contradict` finding real ones.
- **Validation**: Insert superseded ADR, verify excluded from coverage. Run `ddis contradict`, verify count matches drift output.

#### APP-INV-109: Event-SQL Write Atomicity
- **Module**: event-sourcing
- **Addresses**: M13
- **Property**: `forall commands C: C writes ONLY to event log (single-write) OR both event+SQL atomically`
- **Statement**: No state where event exists but SQL missing, or vice versa. During Phase B migration, commands that write both must ensure crash-safety.
- **Semi-formal**: `(event_emitted(C) AND NOT sql_written(C)) = false`, `(sql_written(C) AND NOT event_emitted(C)) = false`
- **Violation**: `witness` writes SQL first, then emits event. Crash between them creates split-brain.
- **Validation**: Inject event-emission failure, verify SQL write rolled back or not committed until event succeeds.

#### APP-INV-110: Quality Gate Identity Preservation
- **Module**: event-sourcing
- **Addresses**: H1
- **Property**: `forall quality gate events E: E.payload.gate_id preserves original gate identifier`
- **Statement**: Import must carry the original gate_id in the payload. The applier must use the payload's gate_id, not derive it from a zero-default integer. `GateNumber=0` is rejected.
- **Semi-formal**: `import(gate).payload.gate_id = gate.gate_id`, `GateNumber > 0 OR gate_id explicitly set`
- **Violation**: Import reads `gate_id` from DB, discards it, creates payload with `GateNumber=0`. Applier generates `"APP-G-0"` for all gates. All gates collapse via `INSERT OR REPLACE`.
- **Validation**: Import spec with 3 gates, materialize, verify 3 distinct rows. Add `gate_id` string field to `QualityGatePayload`.

#### APP-INV-111: Parser Code-Block Isolation
- **Module**: parse-pipeline
- **Addresses**: H3, H4
- **Property**: `forall parser extractors F: F does not extract elements from within fenced code blocks`
- **Statement**: Invariant, ADR, negative spec, and cross-ref extraction must skip content inside markdown code fences. The ADR extractor already does this correctly; invariant and negative spec extractors do not.
- **Semi-formal**: `forall extracted E: E.line_range INTERSECT code_block_ranges = empty`
- **Violation**: `**APP-INV-NNN: Example**` inside a code block extracted as real invariant. `**DO NOT**` in code example extracted as real negative spec.
- **Validation**: Parse test spec with invariant headers and DO NOT patterns inside code blocks. Verify zero extractions from code regions.

#### APP-INV-112: Provenance Table Population
- **Module**: event-sourcing
- **Addresses**: H2, M11
- **Property**: `forall content events applied during fold: event_provenance row exists linking event to element`
- **Statement**: The `event_provenance` table must be populated during materialization fold. The `Provenance()` function must check all payload identifier field names, not just `id` and `invariant_id`.
- **Semi-formal**: `|materialized_elements| <= |event_provenance_rows|`, `Provenance(elementID).length > 0 for all elements`
- **Violation**: Table created but never INSERT'd. `Provenance()` only checks 2 of 10+ field names.
- **Validation**: Materialize test stream, verify non-empty provenance table. Query provenance for sections, glossary terms, modules — verify results.

#### APP-INV-113: Validator Strictness Proportionality
- **Module**: query-validation
- **Addresses**: M15
- **Property**: `forall validator checks C with coverage metrics: severity proportional to gap, not binary`
- **Statement**: Check 17 (challenge freshness) should use threshold-based failure (e.g., >10% unchallenged = FAIL), not single-item failure. 1 unchallenged out of 97 = WARNING, not FAIL.
- **Semi-formal**: `check_17_fail iff unchallenged_ratio > threshold (default 0.10)`
- **Violation**: 1 unchallenged witness out of 97 fails entire validation, blocking fixpoint convergence.
- **Validation**: 96/97 challenged = WARNING (pass). 50/97 challenged = FAIL.

### New ADRs (APP-ADR-077..080)

#### APP-ADR-077: Applier Update Method Completeness Enforcement
- **Module**: event-sourcing
- **Problem**: Update applier methods handle a subset of payload fields (3/7 for invariants, 2/7 for ADRs). New fields silently dropped.
- **Decision**: Exhaustive switch with reflection-based test enforcement. Each applier method must handle all non-metadata payload fields. `TestApplierFieldCoverage` test uses reflection to verify.
- **Why not generic field-map**: SQL injection risk, harder to audit.
- **Consequences**: Every payload struct addition requires corresponding applier case. Test catches gaps at compile/test time.

#### APP-ADR-078: Materialize Reads All Streams
- **Module**: event-sourcing
- **Problem**: Witness/challenge events are Stream 3 types but materialize only reads Stream 2. Events invisible to fold.
- **Decision**: `runMaterializeInternal` reads from all three stream files, merging events. `isContentEvent` remains the filter for what gets folded. Stream membership = storage/VCS concern, not fold concern.
- **Why not move types to Stream 2**: Witnesses are implementation evidence, not spec content.
- **Consequences**: `runMaterializeInternal` concatenates events from all stream files before fold.

#### APP-ADR-079: Dead Code Pruning Policy
- **Module**: event-sourcing
- **Problem**: `formatInvariant`/`formatADR` (60 lines), `ManifestUpdatePayload`/`SnapshotPayload` are defined but never used.
- **Decision**: Remove dead code immediately. Per project policy: no tech debt, no forward-compatibility shims.
- **Consequences**: Dead functions/types removed. If Phase C needs them, re-create from spec.

#### APP-ADR-080: Measurement Signal Wiring Strategy
- **Module**: triage-workflow
- **Problem**: Coverage counts superseded ADRs, drift contradictions always 0, triage fitness inflated.
- **Decision**: Progressive wiring. Coverage adds `status != 'superseded'` filter. Drift queries contradiction results. Triage reads from drift. `drift --code` deferred with tracking issue.
- **Consequences**: Honest measurement signals enable meaningful fixpoint convergence.

---

## Part B: Implementation (After Spec Validation)

### Phase 1: Spec Crystallization + Validation
1. Crystallize APP-INV-106..113 (8 invariants) via `ddis discover crystallize`
2. Crystallize APP-ADR-077..080 (4 ADRs) via `ddis discover crystallize`
3. Update event-sourcing module frontmatter (maintains, implements)
4. Update parse-pipeline module frontmatter (APP-INV-111)
5. Update triage-workflow module (APP-INV-108, APP-ADR-080)
6. Update query-validation module (APP-INV-113)
7. Re-parse: `ddis parse ddis-cli-spec/manifest.yaml`
8. Validate: 19/19 checks, 100% coverage, 0 drift

### Phase 2: HIGH Severity Fixes (H1-H4)

| File | Change | Finding |
|------|--------|---------|
| `events/payloads.go` | Add `GateID string` field to `QualityGatePayload` | H1 |
| `cli/importcmd.go:246` | Set `p.GateID = gateID` (the variable already scanned from DB) | H1 |
| `cli/materialize.go:370` | Use `p.GateID` instead of `fmt.Sprintf("APP-G-%d", p.GateNumber)` | H1 |
| `parser/invariants.go:30-56` | Add code-block tracking in idle state (copy pattern from adrs.go:72-93) | H3 |
| `parser/negspecs.go:10-63` | Add code-block tracking (same pattern) | H4 |

### Phase 3: Applier Field Coverage (M1-M5)

| File | Change | Finding |
|------|--------|---------|
| `cli/materialize.go:266-282` | Add `violation_scenario`, `validation_method`, `why_this_matters` cases to `UpdateInvariant` | M1 |
| `cli/materialize.go:297-311` | Add `problem`, `options`, `consequences`, `tests`, `status` cases to `UpdateADR` | M2 |
| `cli/materialize.go:332-336` | Write `p.Score`, `p.Detail` to DB; use payload `challenged_by` | M3 |
| `cli/materialize.go:289-295` | Write `p.Options` to `chosen_option` column | M4 |
| `events/payloads.go` | Add `RefType string` field to `CrossRefPayload` | M5 |
| `cli/materialize.go:353-358` | Use `p.RefType` instead of hardcoded `'section'` | M5 |
| `cli/importcmd.go:195` | Set `p.RefType` from DB `ref_type` column | M5 |

### Phase 4: Stream + Provenance + Atomicity (M6-M8, M11, M13, H2)

| File | Change | Finding |
|------|--------|---------|
| `cli/materialize.go` | Read all 3 stream files, merge events before fold | M6, ADR-078 |
| `materialize/processors.go:109` | Use `events.TypeImplementationFinding` constant; set correct stream | M7 |
| `cli/crystallize.go:132,144,155` | Pass `specHashFromDB()` instead of `""` | M8 |
| `causal/dag.go:77-96` | Add checks for `path`, `term`, `pattern`, `name`, `source`, `gate_id` | M11 |
| `cli/materialize.go` (applier) | Populate `event_provenance` table on each `Apply()` | H2 |
| `cli/witness.go:147-168` | Emit event FIRST, then write SQL (or wrap in single error check) | M13 |

### Phase 5: Measurement Wiring (M17-M19, M28)

| File | Change | Finding |
|------|--------|---------|
| `coverage/coverage.go:86` | Add `WHERE status IS NULL OR status != 'superseded'` to ADR query | M17 |
| `drift/drift.go:187` | Query `challenge_results` or consistency tables for contradiction count | M18 |
| `triage/triage.go:274` | Read contradictions from drift result, not hardcoded 0 | M28 |
| `cli/drift.go` | Add `--code` flag stub that returns "not implemented" with recovery hint | M19 |

### Phase 6: Dead Code Removal + Validator Fix + Minor Fixes (M9-M10, M15, M20-M27)

| File | Change | Finding |
|------|--------|---------|
| `cli/crystallize.go:223-281` | Remove `formatInvariant()` and `formatADR()` | M9 |
| `events/payloads.go:125,132` | Remove `ManifestUpdatePayload` and `SnapshotPayload` | M10 |
| `events/schema.go:48-49` | Remove `TypeManifestUpdated` and `TypeSnapshotCreated` | M10 |
| `validator/checks.go:1330` | Change Check 17 to threshold-based (>10% = FAIL, else WARNING) | M15 |
| `annotate/grammar.go:13` | Tighten regex or add start-of-line/comment anchor | M20 |
| `cli/next.go:305` | Fix modeHint to read correct payload field or remove dead code | M21 |
| `discover/thread.go:147` | Dedup by thread ID before append | M22 |
| `validator/reachability.go:199` | Find initial state by 0 in-degree, not first row | M24 |
| `consistency/sat.go:347` | Preserve structure in sanitized variable names | M25 |
| `consistency/smt.go:325-327` | Restore multi-argument function arity in SMT-LIB2 translation | M26 |
| `consistency/llm.go:168` | Reduce LLM confidence from 0.95 to 0.85 (reflect non-determinism) | M27 |
| `cli/root.go` | Add `snapshotCmd.GroupID = "utility"`, `versionCmd.GroupID = "utility"` | L1, L2 |
| `projector/render.go` | Add glossary, cross-ref, quality gate, section rendering | M12 |
| `projector/render.go:134` | Replace deprecated `strings.Title` with `cases.Title` | L3 |
| `causal/dag.go:105-113` | Deterministic tiebreaker for same-ID merge conflicts | M14 |
| `storage/storage_test.go:105-140` | Add missing 4 tables to expected list | L14 |
| `storage/models.go:319,335` | Fix stale comments on EvidenceType and Verdict | L15, L16 |

### Phase 7: Tests + Full Verification

**New behavioral tests** (in `tests/invariant_behavioral_test.go`):
- `TestAPPINV106_ApplierFieldCoverageCompleteness` — reflection-based field coverage check
- `TestAPPINV107_StreamTypeAffinityEnforcement` — emit event, verify stream matches schema
- `TestAPPINV108_MeasurementSignalCompleteness` — superseded ADR excluded from coverage
- `TestAPPINV109_EventSQLWriteAtomicity` — event-first, SQL-second ordering
- `TestAPPINV110_QualityGateIdentityPreservation` — import 3 gates, materialize, verify 3 rows
- `TestAPPINV111_ParserCodeBlockIsolation` — INV header in code block not extracted
- `TestAPPINV112_ProvenanceTablePopulation` — fold populates event_provenance
- `TestAPPINV113_ValidatorStrictnessProportionality` — 1/97 unchallenged = warning not fail

**Parser tests** (in `internal/parser/`):
- Test invariant extraction skips code blocks
- Test negative spec extraction skips code blocks

**Full verification**:
```bash
go test ./...                                # all pass
go vet ./...                                 # clean
ddis parse ddis-cli-spec/manifest.yaml       # re-parse
ddis validate manifest.ddis.db               # 19/19
ddis coverage manifest.ddis.db               # 100%
ddis drift manifest.ddis.db --report         # 0
ddis scan ./ddis-cli --spec manifest.ddis.db --verify  # 0 orphaned, 0 unimplemented
ddis witness APP-INV-106..113 manifest.ddis.db  # all witnessed
ddis challenge --all manifest.ddis.db        # all confirmed
```

---

## Finding-to-Phase Traceability Matrix

| Finding | Severity | Phase | New Spec Element |
|---------|----------|-------|-----------------|
| H1 | HIGH | 2 | APP-INV-110 |
| H2 | HIGH | 4 | APP-INV-112 |
| H3 | HIGH | 2 | APP-INV-111 |
| H4 | HIGH | 2 | APP-INV-111 |
| M1 | MEDIUM | 3 | APP-INV-106, APP-ADR-077 |
| M2 | MEDIUM | 3 | APP-INV-106, APP-ADR-077 |
| M3 | MEDIUM | 3 | APP-INV-106 |
| M4 | MEDIUM | 3 | APP-INV-106 |
| M5 | MEDIUM | 3 | APP-INV-106 |
| M6 | MEDIUM | 4 | APP-INV-107, APP-ADR-078 |
| M7 | MEDIUM | 4 | APP-INV-107 |
| M8 | MEDIUM | 4 | (existing APP-INV-072) |
| M9 | MEDIUM | 6 | APP-ADR-079 |
| M10 | MEDIUM | 6 | APP-ADR-079 |
| M11 | MEDIUM | 4 | APP-INV-112 |
| M12 | MEDIUM | 6 | (existing APP-INV-076/077) |
| M13 | MEDIUM | 4 | APP-INV-109 |
| M14 | MEDIUM | 6 | (existing APP-INV-081) |
| M15 | MEDIUM | 6 | APP-INV-113 |
| M17 | MEDIUM | 5 | APP-INV-108, APP-ADR-080 |
| M18 | MEDIUM | 5 | APP-INV-108, APP-ADR-080 |
| M19 | MEDIUM | 5 | APP-INV-108 |
| M20 | MEDIUM | 6 | (existing APP-INV-017) |
| M21 | MEDIUM | 6 | — |
| M22 | MEDIUM | 6 | (existing APP-INV-020) |
| M23 | MEDIUM | 6 | — |
| M24 | MEDIUM | 6 | (existing APP-INV-062) |
| M25 | MEDIUM | 6 | (existing APP-INV-021) |
| M26 | MEDIUM | 6 | — |
| M27 | MEDIUM | 6 | — |
| M28 | MEDIUM | 5 | APP-INV-108, APP-ADR-080 |
| L1-L22 | LOW | 6 | — |

---

## Expected Outcome

| Metric | Before | After |
|--------|--------|-------|
| Invariants | 105 | 113 (+8) |
| ADRs | 76 | 80 (+4) |
| HIGH severity bugs | 4 | 0 |
| MEDIUM severity bugs | 28 | 0 |
| Dead code items | 6 | 0 |
| Applier field coverage | 40-50% | 100% |
| Quality gate round-trip | BROKEN (all collapse to APP-G-0) | CORRECT |
| Parser code-block safety | 2/4 extractors safe | 4/4 extractors safe |
| Provenance table | EMPTY | POPULATED |
| Measurement signal accuracy | 3 hardcoded values | all from DB |
| Findings addressed | 0/52 | 52/52 |
