# Cleanroom Audit — Round 3

**Thread**: `t-cleanroom-r3-2026-02-28`
**Date**: 2026-02-28
**Method**: 3 parallel deep-dive agents (F-09, F-13, F-14), formal spec-first approach

## Scope

Resolved all 3 deferred items from Round 2:
- F-09: ADR options round-trip fidelity (event-sourcing pipeline)
- F-13: Cascade relationship completeness (code-bridge)
- F-14: Empty dependency graphs in implorder/progress (workspace-ops)

## New Spec Elements

| ID | Type | Title | Module |
|----|------|-------|--------|
| APP-INV-110 | Invariant | ADR Options Round-Trip Fidelity | event-sourcing |
| APP-INV-111 | Invariant | Cascade Relationship Completeness | code-bridge |
| APP-INV-112 | Invariant | Module-Level Dependency Ordering | workspace-ops |
| APP-ADR-079 | ADR | ADR Options Serialization in Event Payloads | event-sourcing |
| APP-ADR-080 | ADR | Module-Level DAG with SCC Cycle Breaking | workspace-ops |

## Implementation Details

### F-09: ADR Options Round-Trip Fidelity

**Root cause**: Import command at `importcmd.go:131` queries only `adrs` table; never JOINs `adr_options`. Materializer's `InsertADR` doesn't parse the Options field. Round-trip drops all option data.

**Fix**:
- `importcmd.go`: Added `id` to ADR query, call `storage.GetADROptions()` per ADR, serialize as JSON in `ADRPayload.Options`
- `materialize.go`: `InsertADR` detects JSON array in Options field, deserializes, calls `InsertADROption()` per entry

### F-13: Cascade Relationship Completeness

**Root cause**: `cascade.go:100` had `r.RelType == "maintains"` in exclusion condition. Semantically backwards: the owning module (maintains) was excluded while consumers (interfaces, implements) were included.

**Fix**: Removed the `maintains` exclusion. All 4 relationship types now included. Added `Role` field to `AffectedModule` struct: "owner" (maintains), "consumer" (interfaces/implements), "peer" (adjacent).

### F-14: Module-Level Dependency DAG

**Root cause**: Both `implorder` and `progress` attempted invariant-level dependency graphs, but creating edges from interface relationships caused cycles (bidirectional module interfaces). The workaround was to emit no edges, resulting in a single flat phase.

**Fix**: Replaced invariant-level approach with module-level DAG:
1. Build directed graph from `adjacent` declarations (A lists B → A depends on B)
2. Compute SCCs via Tarjan's algorithm (absorbs bidirectional cycles)
3. Condense SCCs into a DAG
4. Topological sort condensation via Kahn's algorithm
5. Assign invariants to phases based on maintaining module's SCC position
6. Within each phase, sort by authority score (PageRank)

**Result**: 3 phases instead of 1. Phase 0 = foundation (parsing, validation, search, workspace: 43 elements), Phase 1 = dependent (auto-prompting, code-bridge, lifecycle, triage: 34 elements), Phase 2 = event-sourcing (35 elements).

## Quality Gates (Final)

- **Build**: clean (go build ./...)
- **Vet**: clean (go vet ./...)
- **Tests**: all passing (610+ tests)
- **Validation**: 18/19 (Check 11 proportional weight — pre-existing)
- **Coverage**: 98% (109/112 INV, 80/80 ADR)
- **Drift**: 0
- **Witnesses**: 109/109 valid
- **Challenges**: 109/109 confirmed, 0 refuted
