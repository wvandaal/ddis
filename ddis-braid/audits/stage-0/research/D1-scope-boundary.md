# D1 — Stage 0 Scope Boundary Analysis

> **Thread**: R3.1a — Stage 0 vs Stage 1 boundary validation
> **Date**: 2026-03-03
> **Source**: spec/17-crossref.md Appendix C, spec/16-verification.md, all referenced INVs

---

## Research Questions

1. What exactly is in Stage 0 vs Stage 1?
2. Are there any invariants tagged Stage 0 that should be deferred to Stage 1?
3. Are there any Stage 1 items that are actually required for Stage 0 to function?
4. Is the 62-INV Stage 0 scope achievable in 1-2 weeks?

---

## Findings

### Stage 0 Composition (62 INV from Appendix C)

| Namespace | Count | Elements |
|-----------|-------|----------|
| STORE | 13 | INV-STORE-001-012, 014 |
| SCHEMA | 7 | INV-SCHEMA-001-007 (006 progressive, 008 deferred) |
| QUERY | 10 | INV-QUERY-001-002, 005-007, 012-014, 017, 021 |
| RESOLUTION | 8 | INV-RESOLUTION-001-008 (all) |
| HARVEST | 5 | INV-HARVEST-001-003, 005, 007 |
| SEED | 4 | INV-SEED-001-004 |
| MERGE | 3 | INV-MERGE-001-002, 008 |
| GUIDANCE | 6 | INV-GUIDANCE-001-002, 007-010 |
| INTERFACE | 5 | INV-INTERFACE-001-003, 008-009 |

**Note**: Appendix C says "62 INV, core" in the header (section 17.3) but the appendix B
statistics table says 61 Stage 0 INVs. The detailed count from Appendix C enumerates:
13 + 7 + 10 + 8 + 5 + 4 + 3 + 6 + 5 = **61 INV**. The discrepancy is minor (possibly
INV-MERGE-009 was moved in/out during editing). The operational number is **61**.

### Items That Should Be Questioned for Stage 0 Inclusion

#### 1. Graph Engine Algorithms (QUERY-012 through 017, 021) — BORDERLINE

The spec places 7 graph algorithms in Stage 0:
- INV-QUERY-012: Topological sort
- INV-QUERY-013: Cycle detection (Tarjan SCC)
- INV-QUERY-014: PageRank
- INV-QUERY-017: Critical path analysis
- INV-QUERY-021: Graph density metrics

These are justified by INV-GUIDANCE-009 (task derivation) and INV-GUIDANCE-010
(R(t) work routing), both Stage 0. The reasoning is that guidance needs graph
metrics to route work. However:

**Assessment**: PageRank and critical path are reasonable for Stage 0 — they drive
task prioritization which is essential for the guidance system. Topological sort and
cycle detection are prerequisites. Graph density is cheap to compute and useful for
`braid status`. The chain is: GUIDANCE-009/010 need graph metrics; graph metrics
need graph algorithms. This is load-bearing — keep in Stage 0.

#### 2. Full RESOLUTION Namespace (8 INV) — REVIEW RECOMMENDED

All 8 resolution invariants are Stage 0, including:
- INV-RESOLUTION-003: Conservative conflict detection (V:MODEL via stateright)
- INV-RESOLUTION-007: Resolution chain logging (V:MODEL + V:KANI)
- INV-RESOLUTION-008: Transitive conflict propagation (V:MODEL)

Three of these require stateright model checking (Gate 6 in CI).

**Assessment**: In a single-agent Stage 0, conflicts are rare (only one agent
writing). The resolution machinery is needed for merge correctness (INV-MERGE-001
depends on it via the dependency graph), but the multi-agent conflict scenarios
are Stage 3. Consider: **implement the type-level resolution mode selection
(INV-RESOLUTION-001, V:TYPE) and the basic per-attribute routing (INV-RESOLUTION-002)
in Stage 0, but defer the model-checked properties (003, 007, 008) to Stage 1.**
This would reduce Stage 0 by 3 INVs and remove the stateright dependency from
Stage 0. However, this conflicts with the spec's explicit inclusion — proceeding
with full RESOLUTION may be the safer choice to avoid late integration surprises.

#### 3. INV-GUIDANCE-007 through 010 — AMBITIOUS

These are the most complex Stage 0 items:
- INV-GUIDANCE-007: Dynamic CLAUDE.md generation
- INV-GUIDANCE-008: M(t) methodology adherence score
- INV-GUIDANCE-009: Task derivation from graph metrics
- INV-GUIDANCE-010: R(t) work routing

Dynamic CLAUDE.md (007) is the Stage 0 success criterion showcase. M(t) (008)
requires five independent sub-metrics. Task derivation (009) requires the full
graph engine. Work routing (010) requires PageRank, betweenness (Stage 1), and
critical path.

**Assessment**: INV-GUIDANCE-010 references betweenness centrality (INV-QUERY-015),
which is Stage 1. This is a **cross-stage dependency**. R(t) at Stage 0 should
use only Stage 0 graph metrics (PageRank + critical path + topo sort), with
betweenness added at Stage 1. The spec already handles this by listing QUERY-015
as Stage 1, but the GUIDANCE-010 description should clarify that R(t) degrades
gracefully without betweenness.

### Items That Should Move from Stage 1 to Stage 0

#### 1. INV-HARVEST-005 Stage 0 Simplification — ALREADY HANDLED

The spec correctly notes: "Stage 0 simplification: At Stage 0, Q(t) is not yet
available (BUDGET is Stage 1). The Stage 0 implementation uses a turn-count
heuristic as a proxy." This is well-designed progressive refinement.

#### 2. INV-QUERY-003 (Significance Tracking) — WORTH CONSIDERING

Currently Stage 1. Significance tracking feeds ASSOCIATE (SEED namespace), which
is Stage 0. Without significance tracking, ASSOCIATE must use a simpler heuristic
(e.g., recency-only). The spec's SEED-001 through 004 don't explicitly require
significance — ASSOCIATE can use structural graph traversal alone at Stage 0.

**Assessment**: Leave at Stage 1. ASSOCIATE works without significance via
structural graph expansion at Stage 0.

### Scope Feasibility Assessment

The Stage 0 scope is **61 invariants across 9 namespaces**. This is large.

**Complexity tiers within Stage 0**:
- **Tier 1 (foundation, ~2 weeks)**: STORE (13), SCHEMA (7), basic QUERY (5 non-graph: 001, 002, 005, 006, 007) = 25 INV
- **Tier 2 (query + resolution, ~1 week)**: QUERY graph (5: 012-014, 017, 021), RESOLUTION (8) = 13 INV
- **Tier 3 (lifecycle, ~1 week)**: HARVEST (5), SEED (4), MERGE (3) = 12 INV
- **Tier 4 (intelligence + interface, ~1 week)**: GUIDANCE (6), INTERFACE (5) = 11 INV

**Total estimated**: 5 weeks for a single agent, 2-3 weeks with parallel agents.

The SEED.md target of "1-2 weeks" for Stage 0 is aggressive for 61 INVs.
A phased approach within Stage 0 is recommended:
- **Stage 0a** (Tier 1+2): Store, Schema, Query, Resolution = 38 INV (foundation)
- **Stage 0b** (Tier 3+4): Harvest, Seed, Merge, Guidance, Interface = 23 INV (lifecycle)

This lets us validate the core store hypothesis before building the lifecycle layer.

---

## Recommendations

1. **Accept the 61-INV scope** but plan for sub-staging (0a/0b).
2. **Clarify INV-GUIDANCE-010** graceful degradation without Stage 1 graph metrics.
3. **Consider deferring V:MODEL resolution proofs** (003, 007, 008) to Stage 1 while
   keeping the implementation in Stage 0 — verify with proptest initially, add
   stateright proofs later.
4. **The 1-2 week timeline is realistic** only if interpreted as Stage 0a (store +
   query foundation). Full Stage 0 including lifecycle and guidance is 3-5 weeks.
5. **Cross-stage dependency**: INV-MERGE-009 is listed in Appendix C as Stage 0 in
   some readings but not in the detailed breakdown. Verify its stage assignment.

---

## Stage 0 Element Count Verification

Cross-referencing Appendix C (section 17.3) against section 16.1 verification matrix:

| Element | Appendix C Stage | Matrix Stage | Match? |
|---------|-----------------|--------------|--------|
| INV-MERGE-008 | 0 (Appendix C) | 0 (Matrix) | YES |
| INV-MERGE-009 | Not listed in Appendix C | 0 (Matrix) | DISCREPANCY |

INV-MERGE-009 appears in the verification matrix as Stage 0 but is NOT in
Appendix C's detailed element list. This accounts for the 61 vs 62 discrepancy.
INV-MERGE-009 should be added to Appendix C or its stage should be clarified.
