# V1 Audit Triage — Exploration & Resolution Tracker

> **Audit source**: `V1_AUDIT_3-3-2026.md`
> **Beads tracking**: 8 epics (R0–R7), 122 tasks, 130 total beads
> **Status**: IN PROGRESS — exploration phase
> **Ready beads**: 41 (unblocked, ready for execution)
> **Dependency cycles**: 0
> **Parallelism**: R0, R1, R2, R3, R5 all run concurrently; R4 after R1; R6 after R1+R2; R7 after all

---

## Table of Contents

1. [User Decisions (Pre-Approved)](#user-decisions-pre-approved)
2. [Items Requiring User Review](#items-requiring-user-review)
3. [Exploration Reports](#exploration-reports)
   - [§A3: MCPServer Architectural Model](#a3-mcpserver-architectural-model)
   - [§B1: QueryExpr Type Resolution](#b1-queryexpr-type-resolution)
   - [§B2: SeedOutput Type Resolution](#b2-seedoutput-type-resolution)
   - [§B6: HarvestCandidate Field Resolution](#b6-harvestcandidate-field-resolution)
   - [§C1: Semilattice Proof](#c1-semilattice-proof)
   - [§C2: Cascade-as-Fixpoint](#c2-cascade-as-fixpoint)
   - [§C3: Causal Independence](#c3-causal-independence)
   - [§C4: Lattice Validation](#c4-lattice-validation)
4. [Research Reports](#research-reports)
   - [§D1: Stage 0 Scope Feasibility](#d1-stage-0-scope-feasibility)
   - [§D2: Datalog Implementation Guidance](#d2-datalog-implementation-guidance)
   - [§D3: Kani CI Feasibility](#d3-kani-ci-feasibility)
   - [§D4: K_agent Harvest Detection](#d4-k_agent-harvest-detection)
5. [Systemic Pattern Analysis](#systemic-pattern-analysis)
   - [§Pattern-2: Seed-as-Prompt Optimization](#pattern-2-seed-as-prompt-optimization)
   - [§Pattern-4: Token Counting Strategy](#pattern-4-token-counting-strategy)
   - [§Pattern-5: Spec Internal Contradictions](#pattern-5-spec-internal-contradictions)
6. [Cross-Cutting Plans](#cross-cutting-plans)
   - [§Bootstrap: Self-Bootstrap 82→100](#bootstrap-self-bootstrap-82100)
   - [§Verification: Pipeline Feasibility 97.8→100%](#verification-pipeline-feasibility-978100)
   - [§Traceability: Bilateral Convergence](#traceability-bilateral-convergence)
7. [Proposed Simplifications (USER REVIEW REQUIRED)](#proposed-simplifications-user-review-required)
8. [Failure Modes Discovered](#failure-modes-discovered)

---

## 1. User Decisions (Pre-Approved)

These decisions were made by the user in the triage prompt. No further review needed.

| ID | Decision | Rationale |
|----|----------|-----------|
| A1 | BLAKE3 hash for LWW tie-breaking | Deterministic, agent-independent, consistent with existing hash infrastructure |
| A2 | Spec canonical for INV-MERGE-008 | Delivery semantics keep INV-MERGE-008; guide's receipt recording gets new ID |
| B3 | Stage-tag Value enum (9 Stage 0, 4 deferred) | Comprehensively document everywhere, ensure consistency |
| B4 | Transaction explicit fields over TxMetadata bundle | Prefer higher specificity and precision |
| B5 | Guide's Stage 0 MergeReceipt fields | Prefer higher specificity and precision |
| P1 | Free functions over Store methods | Better modularity, Rust idioms, guide already consistent |

---

## 2. Items Requiring User Review

*Populated as exploration/research reports identify items needing user decision.*

| # | Item | Context | Options | Bead |
|---|------|---------|---------|------|
| | | | | |

---

## 3. Exploration Reports

### §A3: MCPServer Architectural Model

**Bead**: brai-12q.3 | **Status**: PENDING
**Subagents**: 2x Opus 4.6

*Report will be populated by exploration subagents.*

---

### §B1: QueryExpr Type Resolution

**Bead**: brai-30q.2 | **Status**: PENDING
**Subagents**: 2x Opus 4.6

*Report will be populated by exploration subagents.*

---

### §B2: SeedOutput Type Resolution

**Bead**: brai-30q.3 | **Status**: PENDING
**Subagents**: 2x Opus 4.6

*Report will be populated by exploration subagents.*

---

### §B6: HarvestCandidate Field Resolution

**Bead**: brai-30q.7 | **Status**: PENDING
**Subagents**: 2x Opus 4.6

*Report will be populated by exploration subagents.*

---

### §C1: Semilattice Proof

**Bead**: brai-2nl.1 | **Status**: PENDING
**Subagents**: 2x Opus 4.6

*Formal proof will be populated by exploration subagents.*

---

### §C2: Cascade-as-Fixpoint

**Bead**: brai-2nl.2 | **Status**: PENDING
**Subagents**: 2x Opus 4.6

*Formal specification and proof will be populated by exploration subagents.*

---

### §C3: Causal Independence

**Bead**: brai-2nl.3 | **Status**: PENDING
**Subagents**: 2x Opus 4.6

*Formal analysis will be populated by exploration subagents.*

---

### §C4: Lattice Validation

**Bead**: brai-2nl.4 | **Status**: PENDING
**Subagents**: 2x Opus 4.6

*Formal analysis will be populated by exploration subagents.*

---

## 4. Research Reports

### §D1: Stage 0 Scope Feasibility

**Bead**: brai-126.1 | **Status**: PENDING
**Subagent**: 1x Opus 4.6

*Full feasibility report will be populated by research subagent.*

---

### §D2: Datalog Implementation Guidance

**Bead**: brai-126.2 | **Status**: PENDING
**Subagent**: 1x Opus 4.6

*Implementation guidance report will be populated by research subagent.*

---

### §D3: Kani CI Feasibility

**Bead**: brai-126.3 | **Status**: PENDING
**Subagent**: 1x Opus 4.6

*Kani feasibility report will be populated by research subagent.*

---

### §D4: K_agent Harvest Detection

**Bead**: brai-126.4 | **Status**: PENDING
**Subagent**: 1x Opus 4.6

*Epistemological analysis will be populated by research subagent.*

---

## 5. Systemic Pattern Analysis

### §Pattern-2: Seed-as-Prompt Optimization

**Bead**: brai-117.1 | **Status**: PENDING
**Subagent**: 1x Opus 4.6 (spec-first-design + prompt-optimization skills)

*Optimization research will be populated by research subagent.*

---

### §Pattern-4: Token Counting Strategy

**Bead**: brai-126.5 | **Status**: PENDING
**Subagent**: 1x Opus 4.6

*Tokenizer comparison and design will be populated by research subagent.*

---

### §Pattern-5: Spec Internal Contradictions

**Bead**: brai-12q.4 | **Status**: PENDING

Three spec-internal contradictions requiring detailed exploration:

#### Contradiction 1: NEG-SCHEMA-001 vs ADR-SCHEMA-005

*Detailed analysis pending. Resolution: ADR wins.*

#### Contradiction 2: Stratum Monotonicity

*Detailed analysis pending. Exact divergence locations to be identified.*

#### Contradiction 3: INV-STORE-001 Verification Tag

*Detailed analysis pending. Matrix vs spec body tag mismatch.*

---

## 6. Cross-Cutting Plans

### §Bootstrap: Self-Bootstrap 82→100

**Bead**: brai-3ia.1 | **Status**: PENDING

*Comprehensive remediation plan pending.*

---

### §Verification: Pipeline Feasibility 97.8→100%

**Bead**: brai-3ia.2 | **Status**: PENDING

*Alternative verification strategies for 4 infeasible items pending.*

---

### §Traceability: Bilateral Convergence

**Bead**: brai-3ia.3 | **Status**: PENDING

*Traceability gap analysis and remediation plan pending.*

---

## 7. Proposed Simplifications (USER REVIEW REQUIRED)

**POLICY**: No Stage 0 scope cuts without explicit user consent. All proposed simplifications are listed here for review.

| # | Proposed Simplification | Rationale | Impact | User Decision |
|---|------------------------|-----------|--------|---------------|
| | | | | |

*Populated as research reports identify potential simplifications.*

---

## 8. Failure Modes Discovered

Failure modes identified during V1 audit, incorporated into FAILURE_MODES.md as FM-010 through FM-019.

| FM-ID | Description | DDIS Mechanism | Acceptance Criterion | Audit Source |
|-------|-------------|---------------|---------------------|--------------|
| FM-010 | Spec self-contradiction (NEG vs ADR) | 5-tier contradiction detection | 0 intra-spec contradictions | Pattern 5 #1 |
| FM-011 | Verification tag inconsistency (matrix vs body) | V-tags as datom attributes | 0 mismatches | Pattern 5 #3 |
| FM-012 | Type name divergence (spec vs guide) | Schema-as-data + bilateral scan | 0 cross-surface divergences | Category B (13 types) |
| FM-013 | Phantom types (referenced, never defined) | Schema validation (INV-SCHEMA-005) | 0 phantom types | Pattern 3 (21 types) |
| FM-014 | Free function vs method inconsistency | ADR-as-data + seed conventions | 0 placement mismatches | Pattern 1 |
| FM-015 | Seed section name divergence | Schema-as-data | All docs use same names | Pattern 2 |
| FM-016 | Token counting undefined dependency | Schema definition of tokenizer | All thresholds testable | Pattern 4 |
| FM-017 | Incomplete CRDT proofs (cascade gap) | Formal proof obligations | All properties proven | Category C |
| FM-018 | Stage 0 scope overcommitment | Guidance + M(t) scoring | Achievable within 4 weeks | Category D #1 |
| FM-019 | K_agent harvest epistemological overreach | Harvest heuristic w/ FP/FN | >=90% externalized capture | Category D #4 |

Existing FMs also cross-referenced with audit findings:
- **FM-005**: V1 Audit Category A item A2 (INV-MERGE-008)
- **FM-006**: V1 Audit 67 spec-guide divergences
- **FM-007**: V1 Audit Category A item A1 (LWW tie-breaking)
- **FM-008**: V1 Audit Agent 7 MAJOR/MINOR stale count findings
- **FM-009**: V1 Audit Agent 5 guidance ADR contradiction

### Wave 1 Non-Critical Findings Resolution

Full resolution of all 214 MAJOR/MINOR/NOTE findings: **`wave1-findings-resolution.md`**

| Status | Count | Percentage |
|--------|-------|------------|
| RESOLVED | 156 | 72.9% |
| DEFERRED | 42 | 19.6% |
| WONTFIX | 12 | 5.6% |
| TODO | 4 | 1.9% |

All 4 TODO items are blocked on R4 (Phantom & Missing Types) epic — specifically R4.2b (brai-2j88).

---

## 9. Bead Inventory — Complete Hierarchy

### Summary

| Metric | Value |
|--------|-------|
| Total beads | 130 |
| Epics | 8 (R0–R7) |
| Tasks + subtasks | 122 |
| P0 (critical) | 35 |
| P1 (high) | 84 |
| P2 (medium) | 11 |
| Ready (unblocked) | 20 |
| Blocked | 110 |
| Dependency cycles | 0 |

### Epic Dependency DAG (Revised)

```
R0 (Critical Fixes) ── narrow task deps only ──→ R1.6, R1.9b, R1.10, R1.12
R1 (Types)          ── parallel with R0 ──────→ R4 (Phantom Types) ──→ R6 ──→ R7
R2 (CRDT Proofs)    ── parallel with R0 ──────────────────────────→ R6 ──→ R7
R3 (Research)       ── parallel with all ──────────────────────────────────→ R7
R5 (Patterns)       ── parallel with R0 ──────────────────────────────────→ R7
```

**Key change from v1**: Removed R0→R1/R2/R5 epic-level deps. Most R1/R2/R5 tasks
are independent of R0 behavioral fixes. Only 4 narrow R1 tasks (R1.6, R1.9b, R1.10,
R1.12) genuinely depend on specific R0 outcomes. This increases parallelizable work
from 20 to 41 ready beads.

### R0 — Critical Behavioral Fixes (P0, 14 beads)

| Bead | Task | Deps |
|------|------|------|
| brai-12q | EPIC: R0 | — |
| brai-12q.1 | R0.1: Write ADR for LWW tie-breaking (BLAKE3) | — |
| brai-12q.1.1 | R0.1a: Audit all LWW/tie-breaking references | — |
| brai-12q.1.2 | R0.1b: Draft and apply ADR-RESOLUTION-NNN | R0.1a |
| brai-12q.2 | R0.2: Renumber INV-MERGE-008 | — |
| brai-12q.2.1 | R0.2a: Audit MERGE INV IDs | — |
| brai-12q.2.2 | R0.2b: Create new INV-MERGE-NNN | R0.2a |
| brai-12q.3 | R0.3: MCPServer architectural model | — |
| brai-12q.3.1 | R0.3a: Research MCP protocol lifecycle | — |
| brai-12q.3.2 | R0.3b: Analyze tradeoffs + recommendation | R0.3a |
| brai-12q.3.3 | R0.3c: Apply MCPServer decision to spec/guide | R0.3b |
| brai-12q.4 | R0.4: Fix 3 spec-internal contradictions | — |
| brai-12q.4.1 | R0.4a: NEG-SCHEMA-001 vs ADR-SCHEMA-005 | — |
| brai-12q.4.2 | R0.4b: Stratum monotonicity contradiction | — |
| brai-12q.4.3 | R0.4c: INV-STORE-001 verification tag mismatch | — |

### R1 — Type System Reconciliation (P0/P1, 21 beads)

| Bead | Task | Deps |
|------|------|------|
| brai-30q | EPIC: R1 | — |
| brai-30q.1 | R1.1: Create types.md | — |
| brai-30q.1.1 | R1.1a: Design types.md structure | — |
| brai-30q.1.2 | R1.1b: Populate with 135 types | R1.1a |
| brai-30q.2 | R1.2: QueryExpr type divergence | — |
| brai-30q.3 | R1.3: SeedOutput vs AssembledContext | — |
| brai-30q.4 | R1.4: Value enum (9+4 variants) | — |
| brai-3nlp | R1.4a: Audit Value enum references | — |
| brai-1qkl | R1.4b: Write canonical Value enum | R1.4a, R1.1 |
| brai-30q.5 | R1.5: Transaction explicit fields | — |
| brai-1cgz | R1.5a: Audit Transaction references | — |
| brai-1kb8 | R1.5b: Unify Transaction fields | R1.5a, R1.1 |
| brai-30q.6 | R1.6: MergeReceipt reconciliation | — |
| brai-30q.7 | R1.7: HarvestCandidate field resolution | — |
| brai-30q.8 | R1.8: Free functions ADR | — |
| brai-30q.9 | R1.9: Schema API reconciliation | — |
| brai-3829 | R1.9a: Catalog 3 Schema API designs | — |
| brai-dt4u | R1.9b: Reconcile Schema API | R1.9a |
| brai-30q.10 | R1.10: Resolution namespace types | — |
| brai-30q.11 | R1.11: Guidance namespace types | — |
| brai-30q.12 | R1.12: Interface namespace types | — |

### R2 — CRDT Formal Verification (P0/P1, 13 beads)

| Bead | Task | Deps |
|------|------|------|
| brai-2nl | EPIC: R2 | — |
| brai-2nl.7 | R2.0: cass/cm CRDT context retrieval | — |
| brai-2nl.1 | R2.1: Join-semilattice proof | R2.0 |
| brai-2nl.1.1 | R2.1a: Formal partial order statement | R2.0 |
| brai-2nl.1.2 | R2.1b: Write all proofs | R2.1a |
| brai-2nl.2 | R2.2: Cascade-as-fixpoint | R2.0 |
| brai-2nl.3 | R2.3: Causal independence | R2.0 |
| brai-2nl.4 | R2.4: Lattice witness requirement | R2.0 |
| brai-2nl.5 | R2.5: 5 unproven CRDT properties | R2.0 |
| brai-2nl.5.1 | R2.5a: LWW semilattice proof | R2.0 |
| brai-2nl.5.2 | R2.5b: Conservative detection proof | R2.0 |
| brai-2nl.5.3 | R2.5c: Design proptest harnesses | R2.0 |
| brai-2nl.6 | R2.6: TLA+ specification | R2.1–R2.4 |

### R3 — Scope, Feasibility & Research (P1, 20 beads)

| Bead | Task | Deps |
|------|------|------|
| brai-126 | EPIC: R3 | — |
| brai-126.1 | R3.1: Stage 0 scope feasibility (D1) | R3.1c |
| brai-3ipq | R3.1a: Formulate D1 questions | — |
| brai-18ic | R3.1b: Gather D1 source material | R3.1a |
| brai-328s | R3.1c: Write D1 feasibility report | R3.1b |
| brai-126.2 | R3.2: Datalog implementation (D2) | R3.2c |
| brai-3miz | R3.2a: Formulate D2 questions | — |
| brai-35ea | R3.2b: Evaluate Datalog crates | R3.2a |
| brai-293h | R3.2c: Write D2 guidance report | R3.2b |
| brai-126.3 | R3.3: Kani CI feasibility (D3) | R3.3c |
| brai-2cj4 | R3.3a: Formulate D3 questions | — |
| brai-3ddz | R3.3b: Benchmark Kani harnesses | R3.3a |
| brai-3tzi | R3.3c: Write D3 feasibility report | R3.3b |
| brai-126.4 | R3.4: K_agent harvest epistemology (D4) | R3.4c |
| brai-3j2y | R3.4a: Formulate D4 questions | — |
| brai-1jk3 | R3.4b: Analyze harvest mechanisms | R3.4a |
| brai-2j58 | R3.4c: Write D4 epistemological report | R3.4b |
| brai-126.5 | R3.5: Token counting strategy (P4) | R3.5c |
| brai-1myf | R3.5a: Survey Rust tokenizer crates | — |
| brai-1xk6 | R3.5b: Design tokenizer trait | R3.5a |
| brai-25uj | R3.5c: Write Pattern-4 report | R3.5b |

### R4 — Phantom & Missing Types (P1, 10 beads)

| Bead | Task | Deps |
|------|------|------|
| brai-14g | EPIC: R4 | R0, R1 |
| brai-14g.1 | R4.1: Triage 21 phantom types | R4.1b, R4.1c |
| brai-4mlo | R4.1a: Classify by stage/provenance | — |
| brai-2340 | R4.1b: Define Stage 0 phantoms | R4.1a, R1.1 |
| brai-33zu | R4.1c: Remove/tag orphans | R4.1a |
| brai-14g.2 | R4.2: 23 guide-only types to spec | R4.2b |
| brai-32vj | R4.2a: Audit guide-only types | — |
| brai-2j88 | R4.2b: Add to spec/types.md | R4.2a, R1.1 |
| brai-14g.3 | R4.3: 35 spec-only types to guide | R4.3b |
| brai-2a76 | R4.3a: Audit spec-only types | — |
| brai-8ebq | R4.3b: Add to guide/ | R4.3a, R1.1 |

### R5 — Systemic Pattern Resolution (P1, 6 beads)

| Bead | Task | Deps |
|------|------|------|
| brai-117 | EPIC: R5 | R0 |
| brai-117.1 | R5.1: Seed-as-Prompt optimization | — |
| brai-117.1.1 | R5.1a: Load skills + analyze seed | — |
| brai-117.1.2 | R5.1b: Unify seed section names | R5.1a |
| brai-117.2 | R5.2: Free functions propagation | R5.2b |
| brai-28go | R5.2a: Audit Store method references | — |
| brai-fg19 | R5.2b: Convert to free functions | R5.2a |

### R6 — Cross-Cutting Assessment Resolution (P1/P2, 34 beads)

| Bead | Task | Deps |
|------|------|------|
| brai-3ia | EPIC: R6 | R0, R1, R2, R5 |
| brai-3ia.1 | R6.1: Self-bootstrap 82→100 | — |
| brai-3ia.1.1 | R6.1a: Spec-to-datom worked example | — |
| brai-3ia.1.2 | R6.1b: Multi-level refinement schema | — |
| brai-3ia.1.3 | R6.1c: Stage 0 contradiction detection | — |
| brai-3ia.2 | R6.2: Verification 97.8→100% | — |
| brai-3ia.2.1 | R6.2a: Alt verification INV-QUERY-001 | — |
| brai-3ia.2.2 | R6.2b: Alt verification INV-QUERY-004 | — |
| brai-3ia.2.3 | R6.2c: Remaining 2 infeasible items | — |
| brai-3ia.3 | R6.3: 100% bilateral traceability | — |
| brai-3ia.3.1 | R6.3a: Backport 3 orphan ADRs | — |
| brai-3ia.3.2 | R6.3b: Scan for missing SEED.md traces | — |
| brai-3ia.3.3 | R6.3c: Formalize ~8% ADRS.md entries | — |
| brai-3ia.4 | R6.4: 67 spec-guide divergences | R0, R1, R2 |
| brai-3ia.4.1 | R6.4a: Verify SPEC-GAP markers | — |
| brai-3ia.5 | R6.5: FAILURE_MODES.md incorporation | — |
| brai-3ia.5.1 | R6.5a: Review existing FM entries | — |
| brai-3ia.5.2 | R6.5b: Write 10 new FM-NNN entries | R6.5a |
| brai-3ia.6 | R6.6: 214 non-critical Wave 1 findings | R6.4 |
| brai-3ia.6.1 | R6.6a: STORE+SCHEMA (56 items) | R6.4 |
| brai-3ia.6.2 | R6.6b: QUERY (23 items) | R6.4 |
| brai-3ia.6.3 | R6.6c: RESOLUTION+MERGE (23 items) | R6.4 |
| brai-3ia.6.4 | R6.6d: HARVEST+SEED (26 items) | R6.4 |
| brai-3ia.6.5 | R6.6e: GUIDANCE (23 items) | R6.4 |
| brai-3ia.6.6 | R6.6f: INTERFACE+BUDGET (28 items) | R6.4 |
| brai-3ia.6.7 | R6.6g: Architecture+Verification (35 items) | R6.4 |
| brai-3ia.7 | R6.7: Per-namespace guide alignment | R6.7d |
| brai-62ut | R6.7a: Rank 67 divergences by severity | R0 |
| brai-2xqw | R6.7b: Fix CRITICAL divergences (5) | R6.7a |
| brai-upx3 | R6.7c: Fix HIGH divergences (25) | R6.7b |
| brai-zhdn | R6.7d: Fix MEDIUM+LOW divergences (37) | R6.7c |

### R7 — Final Verification & Convergence (P1, 12 beads)

| Bead | Task | Deps |
|------|------|------|
| brai-3t9 | EPIC: R7 | R6 |
| brai-3t9.1 | R7.1: Multi-agent final verification | — |
| brai-3t9.1.1 | R7.1a: Per-namespace readiness (10 NS) | R6 |
| brai-1stk | R7.1b: Automated consistency scan | R6 |
| brai-1ga9 | R7.1c: Verify all 100% targets | R7.1a, R7.1b |
| brai-3t9.2 | R7.2: Verification matrix update | R7.2a |
| brai-nydw | R7.2a: Update spec/16-verification.md | R0 |
| brai-3t9.3 | R7.3: Cross-reference coherence | R7.3a |
| brai-2e9w | R7.3a: Full coherence scan | R6 |
| brai-3t9.4 | R7.4: Finalize triage document | R7.4a |
| brai-276t | R7.4a: Write resolution summary | R7.1, R7.2, R7.3 |

---

*This document is a living tracker. Sections are populated as exploration/research subagents complete their work. All items requiring user decision are surfaced in §2 and §7.*
