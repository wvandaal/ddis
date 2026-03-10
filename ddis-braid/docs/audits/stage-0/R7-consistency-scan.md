# R7.1b Automated Consistency Scan

> **Date**: 2026-03-03
> **Scope**: `spec/*.md`, `docs/guide/*.md`
> **Method**: Automated grep/count/cross-reference checks
> **Result**: **ALL 7 CHECKS PASS**

---

## Check 1: INV Count

**Target**: 124 unique `### INV-*-NNN:` headings across `spec/*.md`
**Actual**: **124** -- PASS

### Per-Namespace Breakdown

| Namespace | INV Count | File |
|-----------|-----------|------|
| STORE | 14 | `01-store.md` |
| SCHEMA | 8 | `02-schema.md` |
| QUERY | 21 | `03-query.md` |
| RESOLUTION | 8 | `04-resolution.md` |
| HARVEST | 8 | `05-harvest.md` |
| SEED | 8 | `06-seed.md` |
| MERGE | 9 | `07-merge.md` |
| SYNC | 5 | `08-sync.md` |
| SIGNAL | 6 | `09-signal.md` |
| BILATERAL | 5 | `10-bilateral.md` |
| DELIBERATION | 6 | `11-deliberation.md` |
| GUIDANCE | 11 | `12-guidance.md` |
| BUDGET | 6 | `13-budget.md` |
| INTERFACE | 9 | `14-interface.md` |
| **Total** | **124** | |

---

## Check 2: ADR Count

**Target**: 72 unique `### ADR-*-NNN:` headings across `spec/*.md`
**Actual**: **72** -- PASS

### Per-Namespace Breakdown

| Namespace | ADR Count | File |
|-----------|-----------|------|
| STORE | 15 | `01-store.md` |
| SCHEMA | 5 | `02-schema.md` |
| QUERY | 9 | `03-query.md` |
| RESOLUTION | 6 | `04-resolution.md` |
| HARVEST | 4 | `05-harvest.md` |
| SEED | 4 | `06-seed.md` |
| MERGE | 4 | `07-merge.md` |
| SYNC | 3 | `08-sync.md` |
| SIGNAL | 3 | `09-signal.md` |
| BILATERAL | 3 | `10-bilateral.md` |
| DELIBERATION | 4 | `11-deliberation.md` |
| GUIDANCE | 5 | `12-guidance.md` |
| BUDGET | 3 | `13-budget.md` |
| INTERFACE | 4 | `14-interface.md` |
| **Total** | **72** | |

### Numbering Note

ADR-RESOLUTION has a numbering gap: IDs jump from ADR-RESOLUTION-005 to ADR-RESOLUTION-009.
ADR-RESOLUTION-006, -007, -008 do not exist. This is a numbering discontinuity, not missing
content -- ADR-RESOLUTION-009 (BLAKE3 Hash Tie-Breaking for LWW) is a well-formed, complete
entry referenced extensively throughout `04-resolution.md` (6 references). The gap likely
reflects renumbering during spec development. All other namespaces have contiguous numbering.

---

## Check 3: NEG Count

**Target**: 42 unique `### NEG-*-NNN:` headings across `spec/*.md`
**Actual**: **42** -- PASS

### Per-Namespace Breakdown

| Namespace | NEG Count | File |
|-----------|-----------|------|
| STORE | 5 | `01-store.md` |
| SCHEMA | 3 | `02-schema.md` |
| QUERY | 4 | `03-query.md` |
| RESOLUTION | 3 | `04-resolution.md` |
| HARVEST | 3 | `05-harvest.md` |
| SEED | 2 | `06-seed.md` |
| MERGE | 3 | `07-merge.md` |
| SYNC | 2 | `08-sync.md` |
| SIGNAL | 3 | `09-signal.md` |
| BILATERAL | 2 | `10-bilateral.md` |
| DELIBERATION | 3 | `11-deliberation.md` |
| GUIDANCE | 3 | `12-guidance.md` |
| BUDGET | 2 | `13-budget.md` |
| INTERFACE | 4 | `14-interface.md` |
| **Total** | **42** | |

---

## Check 4: No SPEC-GAP Markers

**Command**: `grep -rn "SPEC-GAP" docs/guide/`
**Expected**: No results
**Actual**: No results -- **PASS**

Also verified: no SPEC-GAP markers exist anywhere in `spec/` either.

---

## Check 5: No Stale DIVERGENCE Markers

**Command**: `grep "DIVERGENCE" docs/guide/types.md`
**Expected**: Only the convention line
**Actual**: Single match at line 10:

```
> **Convention**: `[AGREE]` = spec and guide definitions match. `[DIVERGENCE]` = mismatch
```

This is the convention declaration only, not a stale divergence marker -- **PASS**

---

## Check 6: Duplicate ID Check

**Method**: Extract all `### INV-*-NNN:`, `### ADR-*-NNN:`, `### NEG-*-NNN:` headings,
strip trailing text, sort, check for duplicates via `uniq -d`.

**Results**:
- Duplicate INVs: **0** -- PASS
- Duplicate ADRs: **0** -- PASS
- Duplicate NEGs: **0** -- PASS

All 238 element IDs (124 INV + 72 ADR + 42 NEG) are unique across the entire spec corpus.

---

## Check 7: Cross-Reference Spot Check

**Method**: Selected 10 INV IDs from `spec/16-verification.md` (verification matrix), one
from each of 10 different namespaces, and verified each exists as a `### INV-*-NNN:` heading
in its namespace file.

| INV ID | Namespace File | Result |
|--------|---------------|--------|
| INV-STORE-003 | `01-store.md` | PASS |
| INV-SCHEMA-006 | `02-schema.md` | PASS |
| INV-QUERY-015 | `03-query.md` | PASS |
| INV-RESOLUTION-005 | `04-resolution.md` | PASS |
| INV-HARVEST-004 | `05-harvest.md` | PASS |
| INV-SEED-007 | `06-seed.md` | PASS |
| INV-MERGE-008 | `07-merge.md` | PASS |
| INV-GUIDANCE-010 | `12-guidance.md` | PASS |
| INV-BILATERAL-003 | `10-bilateral.md` | PASS |
| INV-BUDGET-004 | `13-budget.md` | PASS |

**10/10 PASS** -- all sampled cross-references resolve correctly.

### Bonus: Verification Matrix Completeness

As an additional check beyond the 10-sample spot check, verified bidirectional completeness
between the verification matrix (`spec/16-verification.md`) and the namespace definition files:

- INVs defined in namespace files: **124**
- INVs listed in verification matrix: **124**
- Missing from verification matrix: **0**
- Phantom entries in verification matrix: **0**

The verification matrix is **100% complete** -- every INV defined in a namespace file has a
corresponding entry in the verification matrix, and vice versa.

---

## Summary

| # | Check | Target | Actual | Status |
|---|-------|--------|--------|--------|
| 1 | INV count | 124 | 124 | PASS |
| 2 | ADR count | 72 | 72 | PASS |
| 3 | NEG count | 42 | 42 | PASS |
| 4 | No SPEC-GAP markers in docs/guide/ | 0 | 0 | PASS |
| 5 | No stale DIVERGENCE markers | convention only | convention only | PASS |
| 6 | No duplicate IDs | 0 duplicates | 0 duplicates | PASS |
| 7 | Cross-ref spot check (10 IDs) | 10/10 resolve | 10/10 resolve | PASS |

**Overall: 7/7 checks pass. The specification is internally consistent.**

### Grand Totals

- **238 total specification elements** (124 INV + 72 ADR + 42 NEG)
- **14 namespaces**, each with its own file in `spec/`
- **0 duplicates** across the entire corpus
- **100% verification matrix coverage** (124/124 INVs mapped)
- **1 numbering discontinuity** (ADR-RESOLUTION-006/007/008 gap) -- cosmetic only, no missing content

### Observations

1. QUERY is the largest namespace (21 INV, 9 ADR, 4 NEG = 34 elements), tied with STORE
   (14 INV, 15 ADR, 5 NEG = 34 elements). This reflects the central role of Datalog query
   and the append-only store in the architecture.

2. SYNC is the smallest namespace (5 INV, 3 ADR, 2 NEG = 10 elements), tied with BILATERAL
   (5 INV, 3 ADR, 2 NEG = 10 elements). These are higher-level coordination concerns that
   compose primitives from other namespaces.

3. The ADR-RESOLUTION numbering gap (006-008) is the only numbering irregularity across all
   238 elements. INV and NEG numbering is fully contiguous within every namespace.
