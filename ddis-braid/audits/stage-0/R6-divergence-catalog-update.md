# R6.4 Divergence Catalog Update

> **Date**: 2026-03-03
> **Task**: R6.4 — Resolve SPEC-GAP Markers and Coordinate Divergence Catalog
> **Status**: Complete

---

## 1. SPEC-GAP Markers

**Status**: Zero remaining.

All 4 SPEC-GAP markers identified during guide production (Session 005) were replaced
with formal invariants in Session 006:

| Original SPEC-GAP | Resolved As |
|--------------------|-------------|
| Tool description quality metric | INV-INTERFACE-008 |
| Error message recovery-hint completeness | INV-INTERFACE-009 |
| Dynamic CLAUDE.md as formally optimized prompt | INV-GUIDANCE-007 augmentation |
| Token efficiency as testable property | INV-BUDGET-006 |

Verified via `grep -rn 'SPEC-GAP' guide/ spec/` -- zero hits in guide/ and spec/.

---

## 2. Type Divergence Catalog (guide/types.md)

### V1 Audit Baseline

The V1 audit (14 Fagan inspection subagents) identified:
- **67 spec-guide divergences** (5 CRITICAL) across all categories
- **13 type-level divergences** (D1-D13) tracked in guide/types.md Appendix A
- **34 additional type divergences** (phantom types, missing types, field mismatches)
- **3 intentional stage-scoping items** (S1-S3)

### Current State (Post R0-R6)

**All 13 type divergences resolved:**

| ID | Type | Resolved By | Nature |
|----|------|-------------|--------|
| D1 | `MergeReceipt` | R1.6 | Spec adopted guide's split (MergeReceipt + CascadeReceipt) |
| D2 | `Clause` | R6.7b | Spec adopted Datalog-standard naming from guide |
| D3 | `ConflictSet` | R1.10 | Spec adopted guide's assertions + retractions model |
| D4 | `ResolutionMode` | R1.10 | Spec adopted guide naming (LastWriterWins, MultiValue) |
| D5 | `QueryResult` | R6.7b | Spec adopted bindings + Stratum enum from guide |
| D6 | `QueryExpr` | R6.7b | Spec adopted Find(ParsedQuery) from guide |
| D7 | `QueryMode` | R6.7b | Spec adopted tuple variant form from guide |
| D8 | `HarvestCandidate` | R6.7b | Spec added id + reconciliation_type from guide |
| D9 | `CandidateStatus` | R4.1b | Spec added Rejected(String) from guide |
| D10 | `AssembledContext` | R6.7b | Guide added projection_pattern from spec |
| D11 | `MCPServer` | R0.3c | Aligned on struct fields; guide BraidMcpServer for rmcp |
| D12 | `ConflictTier` | R1.10 | Spec adopted RoutingTier from guide |
| D13 | `TxId` | R6.7d | Intentional -- Hash derive only needed on AgentId |

**Stage-scoping items** (S1-S3) remain as intentional, not defects:
- S1: Value enum (14 spec vs 9 guide -- 5 deferred to later stages)
- S2: Stratum enum (6 spec vs 2 guide -- 4 deferred to Stage 1+)
- S3: Clause variants (Aggregate, Ffi deferred to Stage 1+)

**Guide-only types reduced**: from 18 to 12 (ParsedQuery, FindSpec, BindingSet,
TxReceipt, TxValidationError, SchemaError moved to AGREE).

**Spec-only types reduced**: from 37 to 33 (Resolution, HarvestSession,
ReviewTopology moved to AGREE; MCPTool moved to AGREE).

---

## 3. Broader Divergence Resolution (67 V1 Audit Items)

The V1 audit's 67 divergences fell into 5 systemic patterns. Resolution status:

| Pattern | Items | Status |
|---------|-------|--------|
| P1: Methods vs free functions (4 namespaces) | ~12 | **RESOLVED (R5, SR-013/ADR-STORE-015/ADR-ARCHITECTURE-001)**: Free functions adopted project-wide |
| P2: Seed section naming (3 different sets) | ~4 | **RESOLVED (R5.1b)**: Spec adopted guide naming |
| P3: Phantom types (21 undefined types) | ~21 | **RESOLVED (R4)**: Defined, removed, or tagged as non-Stage-0 |
| P4: Token counting undefined | ~3 | **RESOLVED (R3/D5)**: Tokenizer survey + ADR |
| P5: Spec internal contradictions (3 items) | ~3 | **RESOLVED (R0.4)**: NEG-SCHEMA-001, stratum, verification tags fixed |
| Type divergences (D1-D13) | 13 | **RESOLVED**: All 13 reconciled (see section 2) |
| Other (naming, behavioral, scope) | ~11 | **Partially resolved** by R1-R5 work; residual items are Stage 1+ or documentation-only |

**Estimated resolution**: ~60 of 67 divergences resolved (90%). Remaining ~7 are
low-severity items that are either (a) intentional stage-scoping accepted as non-defects,
or (b) documentation refinements not blocking Stage 0 implementation.

---

## 4. New Divergences Introduced by R0-R5

Two new divergences were found and fixed during this R6.4 audit:

### 4.1 Stale SEED INV References in guide/12-stages-1-4.md (FIXED)

The R5.1b agent renumbered SEED INVs (added INV-SEED-004/005, shifted old 004-006 to
006-008) and updated spec/17-crossref.md correctly, but guide/12-stages-1-4.md retained
stale `INV-SEED-005-006` references instead of the correct `INV-SEED-007-008`.

**Fix applied**: Two occurrences at lines 26 and 168 updated to `INV-SEED-007-008`.

### 4.2 Verification Matrix Summary Was Stale (FIXED by another agent)

The R0-R5 agent added 3 new INVs (INV-SEED-004, INV-SEED-005, INV-MERGE-009) and
added the matrix rows, but the summary at section 16.6 was only updated from 121 to 122
(should be 124). Another agent (R6.7) corrected this to the accurate counts:
- Total: 124 (was 122)
- Stage 0: 64 (was 62)
- V:KANI: 41 (was 39)
- V:TYPE: 10 (was 12)

### 4.3 ADR-ARCHITECTURE-001 Missing from ADRS.md (FIXED by another agent)

ADR-ARCHITECTURE-001 (free functions over Store methods) was defined in
guide/00-architecture.md and referenced in 7+ spec files, but was not tracked in
ADRS.md. Another agent added it as SR-013 with cross-references to both
ADR-STORE-015 (spec) and ADR-ARCHITECTURE-001 (guide).

---

## 5. Summary Statistics

| Metric | V1 Audit | Current | Delta |
|--------|----------|---------|-------|
| Total spec-guide divergences | 67 | ~7 | -60 (90% resolved) |
| Type divergences (D1-D13) | 13 (4 critical) | 0 | -13 (100% resolved) |
| SPEC-GAP markers | 4 | 0 | -4 (100% resolved) |
| Phantom types | 21 | 0 | -21 (100% resolved) |
| Spec internal contradictions | 3 | 0 | -3 (100% resolved) |
| New divergences introduced | — | 2 (both fixed) | — |
| Total INVs | 121 | 124 | +3 |
| Total spec elements | 233 | 238 | +5 |

---

## 6. Conclusion

The divergence catalog is effectively resolved. All critical and high-severity
divergences have been reconciled. The remaining ~7 items are intentional stage-scoping
or documentation-level refinements that do not block Stage 0 implementation.

No outstanding R6.4 work remains. The types.md canonical catalog is current and
internally consistent (all body-level status tags match the appendix summary).
