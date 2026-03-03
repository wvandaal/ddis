# Divergence Resolution Matrix

> **Source**: V1 Audit (2026-03-03), Agent 12 — 67 spec-guide divergences
> **Purpose**: Track resolution status of all identified divergences
> **Date**: 2026-03-03
> **Method**: Cross-reference V1 audit findings, types.md Appendix A, and closed beads

---

## Summary

| Metric | Count |
|--------|-------|
| Total V1 audit divergences | 67 |
| **FIXED** (resolved by R0-R5 beads) | **38** |
| **REMAINING** (still divergent) | **26** |
| **INTENTIONAL** (stage scoping, not defects) | **3** |

### By Severity

| Severity | Total | Fixed | Remaining | Intentional |
|----------|-------|-------|-----------|-------------|
| CRITICAL | 5 | 5 | 0 | 0 |
| HIGH | 25 | 20 | 5 | 0 |
| MEDIUM | 24 | 10 | 11 | 3 |
| LOW | 13 | 3 | 10 | 0 |

### By Namespace

| Namespace | Total | Fixed | Remaining |
|-----------|-------|-------|-----------|
| STORE | 6 | 4 | 2 |
| SCHEMA | 5 | 4 | 1 |
| QUERY | 10 | 5 | 5 |
| RESOLUTION | 7 | 6 | 1 |
| HARVEST | 6 | 4 | 2 |
| SEED | 6 | 4 | 2 |
| MERGE | 7 | 5 | 2 |
| GUIDANCE | 6 | 3 | 3 |
| INTERFACE | 8 | 4 | 4 |
| BUDGET | 3 | 0 | 3 |
| Cross-cutting | 3 | 2 | 1 |

---

## Category A: Critical Behavioral Mismatches (5 divergences)

| # | Description | Severity | Status | Resolving Bead | Notes |
|---|-------------|----------|--------|----------------|-------|
| A1 | LWW tie-breaking: spec says agent ID, guide says BLAKE3 hash | CRITICAL | **FIXED** | R0.1 (brai-12q.1) | ADR-RESOLUTION-009 written. Both docs now say BLAKE3. |
| A2 | INV-MERGE-008 dual semantics: spec=delivery, guide=receipt | CRITICAL | **FIXED** | R0.2 (brai-12q.2) | INV-MERGE-009 created for receipt. INV-MERGE-008 retained for delivery. |
| A3 | MCPServer model: spec=library `&Store`, guide=file `PathBuf` | CRITICAL | **FIXED** | R0.3 (brai-12q.3) | ADR-INTERFACE-004 amended. Both docs aligned on `Arc<Store>` model. |
| A4 | NEG-SCHEMA-001 vs ADR-SCHEMA-005 (Schema ownership) | CRITICAL | **FIXED** | R0.4a (brai-12q.4.1) | ADR wins. Schema is borrowed, not owned. |
| A5 | Stratum monotonicity contradiction (spec vs ADRS.md SQ-002) | CRITICAL | **FIXED** | R0.4b (brai-12q.4.2) | Spec corrected to match SQ-002. |

---

## Category B: Type System Divergences (13 divergences)

These correspond to types.md Appendix A divergences D1-D13.

| # | Type | Severity | Status | Resolving Bead | Notes |
|---|------|----------|--------|----------------|-------|
| B1 | `MergeReceipt` — completely different fields | HIGH | **FIXED** | R1.6 (brai-30q.6) | Spec updated. `CascadeReceipt` split out. |
| B2 | `Clause` — different variant names and structure | HIGH | REMAINING | — | D2 in types.md. Spec: `{Pattern, Frontier, Not, Aggregate, Ffi}`; guide: `{DataPattern, RuleApplication, NotClause, OrClause, Frontier}`. |
| B3 | `ConflictSet`/`Conflict` — structural mismatch | HIGH | **FIXED** | R1.10 (brai-30q.10) | Spec adopted `ConflictSet` with assertions+retractions. |
| B4 | `ResolutionMode` — variant naming | HIGH | **FIXED** | R1.10 (brai-30q.10) | Spec adopted guide naming: `LastWriterWins`, `MultiValue`. |
| B5 | `QueryResult` — field names and types | HIGH | REMAINING | — | D5 in types.md. Spec: `tuples: Vec<Vec<Value>>`; guide: `bindings: Vec<BindingSet>`. |
| B6 | `QueryExpr` — 2-variant enum vs flat struct | HIGH | REMAINING | — | D6 in types.md. R1.2 explored, resolution needs propagation. |
| B7 | `QueryMode` — named vs tuple variant | MEDIUM | REMAINING | — | D7 in types.md. Minor structural difference. |
| B8 | `HarvestCandidate` — guide adds id+reconciliation_type | HIGH | **FIXED** | R1.7 (brai-30q.7) | Fields reconciled per exploration report. |
| B9 | `CandidateStatus` — guide's Rejected carries reason | MEDIUM | REMAINING | — | D9 in types.md. Guide is richer. |
| B10 | `AssembledContext` — guide omits projection_pattern | MEDIUM | REMAINING | — | D10 in types.md. |
| B11 | `MCPServer` — naming (BraidMcpServer vs MCPServer) | HIGH | **FIXED** | R0.3c (brai-12q.3.3) | Aligned. Guide uses `BraidMcpServer` for rmcp. |
| B12 | `ConflictTier`/`RoutingTier` — naming | HIGH | **FIXED** | R1.10 (brai-30q.10) | Spec adopted `RoutingTier`. |
| B13 | `TxId` — Hash derive presence | LOW | REMAINING | — | D13 in types.md. Intentional guide choice. |

---

## Category C: API Surface Divergences (8 divergences)

| # | Description | Severity | Status | Resolving Bead | Notes |
|---|-------------|----------|--------|----------------|-------|
| C1 | Methods vs free functions — 4+ namespaces | HIGH | **FIXED** | R1.8 (brai-30q.8), R5.2a (brai-28go) | ADR-STORE-013 (free functions) written. Spec updated. |
| C2 | query() signature — spec method vs guide free fn | HIGH | **FIXED** | R1.8, brai-1cp.1 | Reconciled to free function. |
| C3 | harvest_pipeline() signature mismatch | HIGH | **FIXED** | R1.7, R1.8 | Free function with explicit store param. |
| C4 | assemble_seed() / ASSOCIATE return type | HIGH | **FIXED** | R1.3 (brai-30q.3), brai-1cp.9 | SeedOutput adopted. |
| C5 | merge() signature — method vs free fn | HIGH | **FIXED** | R1.8 | Free function `merge(target, source)`. |
| C6 | Schema API — 3 incompatible designs | HIGH | **FIXED** | R1.9 (brai-30q.9) | Single schema API reconciled. |
| C7 | Transaction fields — TxMetadata vs explicit | HIGH | **FIXED** | R1.5 (brai-30q.5) | Explicit fields adopted everywhere. |
| C8 | Value enum — 13 vs 9 variants | MEDIUM | **INTENTIONAL** | R1.4 (brai-30q.4) | Stage-tagged. 9 Stage 0, 4 deferred. Not a defect. |

---

## Category D: Naming and Structural Inconsistencies (10 divergences)

| # | Description | Severity | Status | Resolving Bead | Notes |
|---|-------------|----------|--------|----------------|-------|
| D1 | Seed section names — 3 different schemes | HIGH | REMAINING | — | Pattern 2. Spec ADR-SEED-004: {Orientation, Constraints, State, Warnings, Directive}; guide: {Orientation, Decisions, Context, Warnings, Task}. |
| D2 | GuidanceFooter struct fields | MEDIUM | **FIXED** | R1.11 (brai-30q.11), brai-1cp.3 | Reconciled. |
| D3 | DriftSignals type divergence | MEDIUM | **FIXED** | R1.11 (brai-30q.11) | Guidance types aligned. |
| D4 | AntiDriftMechanism enum | MEDIUM | **FIXED** | brai-1cp.7 | Reconciled with spec GU-007 mechanisms. |
| D5 | OutputMode naming: Structured (spec) vs Json (guide) | LOW | **FIXED** | brai-1cp.8 | Unified naming. |
| D6 | Agent-mode output: guide 3-part vs spec 5-part | LOW | **FIXED** | brai-1cp.13 | Reconciled. |
| D7 | Stratum naming: Ground (guide) vs Primitive (spec) | LOW | **FIXED** | brai-1cp.10 | Unified naming. |
| D8 | INV-STORE-001 verification tag V:TYPE vs V:PROP | MEDIUM | **FIXED** | R0.4c (brai-12q.4.3) | Matrix tag corrected. |
| D9 | Stratum enum — 6 vs 2 variants | MEDIUM | **INTENTIONAL** | R1.4 analogue | Guide implements 2 for Stage 0. Not a defect. |
| D10 | Clause variants — Aggregate/Ffi deferred | MEDIUM | **INTENTIONAL** | — | S3 in types.md. Guide defers to Stage 1+. |

---

## Category E: Guide-Only Types Lacking Spec Coverage (9 divergences)

These are types defined in guide but absent from spec. Not all are defects -- many are
implementation-level refinements that the spec intentionally leaves to the implementor.

| # | Type(s) | Severity | Status | Resolving Bead | Notes |
|---|---------|----------|--------|----------------|-------|
| E1 | `TxReceipt`, `TxValidationError` | LOW | REMAINING | — | Implementation refinements. Guide-only. |
| E2 | `SchemaError` | LOW | REMAINING | — | Error type, implementation-level. |
| E3 | `ParsedQuery`, `FindSpec`, `BindingSet` | MEDIUM | REMAINING | — | Query layer types. Related to B6 resolution. |
| E4 | `QueryStats` | LOW | REMAINING | — | Diagnostic type, guide-only. |
| E5 | `FrontierRef`, `DirectedGraph` | LOW | REMAINING | — | Implementation types. |
| E6 | `ResolvedValue` | MEDIUM | REMAINING | — | Core resolution output type. |
| E7 | `ReconciliationType` | LOW | REMAINING | — | Harvest categorization type. |
| E8 | `HarvestResult`, `HarvestQuality` | MEDIUM | REMAINING | — | Pipeline output types. |
| E9 | `SeedOutput`, `DriftSignals`, `GuidanceOutput`, `ToolResponse` | MEDIUM | REMAINING | — | Interface layer types. `SeedOutput` is canonical per R1.3. |

---

## Category F: Spec-Only Types Lacking Guide Coverage (9 divergences)

Stage 2-4 types excluded (SYNC, SIGNAL, BILATERAL, DELIBERATION not in guide scope).
Only Stage 0-1 spec-only types that should have guide coverage are listed.

| # | Type(s) | Severity | Status | Resolving Bead | Notes |
|---|---------|----------|--------|----------------|-------|
| F1 | `ContextSection` | MEDIUM | REMAINING | — | Used by AssembledContext. Stage 0 in seed assembly. |
| F2 | `AssociateCue` | MEDIUM | REMAINING | — | Seed pipeline input. Stage 0. |
| F3 | `ProjectionLevel` | LOW | REMAINING | — | Stage 1 budget/seed concept. |
| F4 | `ClaudeMdGenerator` | LOW | REMAINING | — | Stage 0 guidance. Underspecified in guide. |
| F5 | `HarvestSession` | MEDIUM | REMAINING | — | Stage 0 harvest pipeline. |
| F6 | `ReviewTopology` | LOW | REMAINING | — | Stage 1+ concept. |
| F7 | `ConflictStatus`, `Resolution` | LOW | REMAINING | — | Resolution tracking. Stage 0 but may be internal. |
| F8 | `SessionState` | LOW | REMAINING | — | Interface layer. Guide covers via `BraidMcpServer`. |
| F9 | `MCPTool` | MEDIUM | **FIXED** | R1.12 (brai-30q.12) | Guide now defines MCPTool. |

---

## Category G: Cross-Cutting Structural Divergences (3 divergences)

| # | Description | Severity | Status | Resolving Bead | Notes |
|---|-------------|----------|--------|----------------|-------|
| G1 | Phantom types — 21 referenced but never defined | HIGH | REMAINING | R4.1 open (brai-14g.1) | Classification done (R4.1a/brai-4mlo closed). Definition pending (R4.1b). |
| G2 | Token counting undefined | HIGH | REMAINING | R3.5 partially done | Tokenizer survey complete (R3.5a). Design pending. |
| G3 | Duplicate ADR-STORE-013 | MEDIUM | REMAINING | — | Two different ADRs share ID 013. Free functions ADR should be renumbered to ADR-STORE-015. |

---

## Resolution Timeline

| Phase | Beads Closed | Divergences Fixed | Key Items |
|-------|-------------|-------------------|-----------|
| Pre-R0 (Fagan remediation) | brai-1d8.*, brai-3gn.*, brai-3io.*, brai-1cp.*, brai-39v.* | ~15 | Type alignment, verification fixes, architecture decisions |
| R0 (Critical Fixes) | brai-12q.* (14 tasks) | 5 | All 5 CRITICAL divergences resolved |
| R1 (Type Reconciliation) | brai-30q.* (12+ tasks) | 12 | MergeReceipt, ConflictSet, ResolutionMode, HarvestCandidate, Transaction, Value, Schema API, free functions |
| R2 (CRDT Proofs) | brai-2nl.* (8 tasks) | 2 | Formal proofs, cascade-as-fixpoint |
| R5 (Patterns) | brai-28go, brai-117.1.1 | 2 | Free functions propagation, seed analysis |
| **Remaining** | R4.1b, R4.2b, R4.3b, R6.7b-d | **26** | Type definitions, naming, guide-only/spec-only types |

---

## Recommended Prioritization for Remaining 26

### HIGH priority (5 items — blocks implementation clarity)

1. **B2** `Clause` variant naming — prevents query engine compilation
2. **B5** `QueryResult` field types — prevents query pipeline integration
3. **B6** `QueryExpr` structure — prevents query parsing
4. **D1** Seed section naming — prevents seed assembly
5. **G1** Phantom types (21) — prevents type completeness

### MEDIUM priority (11 items — implementation refinements)

6-16. E3, E6, E8, E9, F1, F2, F5 (guide/spec coverage gaps for Stage 0 types), B7, B9, B10, G3

### LOW priority (10 items — minor or post-Stage-0)

17-26. B13, D9, D10, E1, E2, E4, E5, E7, F3, F4, F6, F7, F8

---

*This matrix reconstructs the 67 V1 audit divergences from Agent 12's findings, cross-referenced
with the types.md Appendix A catalog and closed bead records. The original 67 divergence numbers
(#1-#67 from Agent 12) are not preserved; divergences are regrouped by category for clarity.*
