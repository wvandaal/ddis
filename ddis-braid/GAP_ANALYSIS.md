# Gap Analysis: Existing DDIS Go CLI vs. Braid SEED.md Specification

**Date**: 2026-03-02
**Scope**: Comprehensive analysis of `../ddis-cli/` (~62,500 LOC, 38 packages, 234 .go files) against `SEED.md` §1–§11 and all 139 design decisions in `ADRS.md` (14 categories)
**Method**: Six waves of parallel deep-dive analysis (24 subagent investigations), each reading actual Go source code, followed by cross-verification
**Prior Art**: Incorporates findings from `../.ddis/specs/GAP_ANALYSIS_2026-02-27.md`, `cleanroom-audit-2026-03-01.md`, and `RECOMMENDATION_MCP_PROTOCOL_SERVER_2026-03-01.md`

---

## Executive Summary

The existing DDIS Go CLI is a mature, production-quality implementation (~62,500 LOC) that validates many DDIS concepts in practice. It has achieved F(S) = 1.0 fixpoint convergence on its own specification, with 97 invariants witnessed and challenged, 511 code annotations at 100% resolution, and 20 mechanical validation checks passing. However, the CLI was built on a **relational substrate** (39-table normalized SQLite schema) whereas SEED.md specifies a **datom substrate** ([e, a, v, tx, op] tuples in a grow-only set). This architectural divergence is the central finding: the CLI proves that DDIS *concepts* work at scale, but its *substrate* is fundamentally different from what Braid requires.

**By the numbers (module-level):**

| Category | Count | Details |
|----------|-------|---------|
| ALIGNED | 17 modules | Concepts proven correct; logic portable to new substrate |
| DIVERGENT | 12 modules | Right concept, wrong substrate or incomplete mechanism |
| EXTRA | 6 modules | Useful features the spec doesn't address; evaluate for inclusion |
| BROKEN | 4 items | Known bugs from cleanroom audit (H1–H4) |
| MISSING | 15 capabilities | Required by SEED.md or settled design decisions, not implemented in any form |

**ADR-level coverage (139 design decisions across 14 categories):**

| Status | Count | Description |
|--------|-------|-------------|
| IMPLEMENTED | 12 | Decision fully reflected in Go CLI code |
| PARTIAL | 41 | Components exist but incomplete or informal |
| DIVERGENT | 10 | Addresses same problem with incompatible mechanism |
| MISSING | 66 | No implementation whatsoever |
| N/A | 10 | Decision is Braid-specific, not assessable against Go CLI |

The ADR-level analysis (Section 11) reveals that the module-level assessment understates the gap. While 17 modules are ALIGNED at the implementation level, only 12 of 139 design decisions are fully IMPLEMENTED. The gap concentrates in four subsystems: **Uncertainty & Authority** (0/12 implemented), **Conflict & Resolution** (0/7), **Agent Architecture** (0/7), and **Guidance System** (0/8).

---

## Table of Contents

1. [Methodology](#1-methodology)
2. [The Central Finding: Substrate Divergence](#2-the-central-finding-substrate-divergence)
3. [Module-by-Module Analysis](#3-module-by-module-analysis)
   - [3.1 Storage Layer](#31-storage-layer)
   - [3.2 Parser](#32-parser)
   - [3.3 Event-Sourcing Pipeline](#33-event-sourcing-pipeline)
   - [3.4 Consistency Engine](#34-consistency-engine)
   - [3.5 Search Intelligence](#35-search-intelligence)
   - [3.6 Validation Engine](#36-validation-engine)
   - [3.7 Drift Detection](#37-drift-detection)
   - [3.8 Witness/Challenge System](#38-witnesschallenge-system)
   - [3.9 Triage Engine](#39-triage-engine)
   - [3.10 Discovery/Autoprompt](#310-discoveryautoprompt)
   - [3.11 Absorb/Refine](#311-absorbrefine)
   - [3.12 Annotation System](#312-annotation-system)
   - [3.13 Coverage/Implorder/Progress](#313-coverageimplorderprogress)
   - [3.14 CLI Command Layer](#314-cli-command-layer)
   - [3.15 Skeleton/Workspace](#315-skeletonworkspace)
   - [3.16 LLM Provider](#316-llm-provider)
4. [MISSING: Capabilities Required by SEED.md](#4-missing-capabilities-required-by-seedmd)
5. [BROKEN: Known Defects](#5-broken-known-defects)
6. [EXTRA: Features Beyond the Spec](#6-extra-features-beyond-the-spec)
7. [Cross-Cutting Concerns](#7-cross-cutting-concerns)
   - [7.1 Self-Bootstrap](#71-self-bootstrap)
   - [7.2 Traceability](#72-traceability)
   - [7.3 Reconciliation Taxonomy](#73-reconciliation-taxonomy)
   - [7.4 Test Coverage](#74-test-coverage)
8. [Reusability Assessment](#8-reusability-assessment)
9. [Stage 0 Impact Analysis](#9-stage-0-impact-analysis)
10. [Summary Tables](#10-summary-tables)
11. [ADR Coverage Analysis: All 139 Design Decisions vs. Go CLI](#11-adr-coverage-analysis-all-139-design-decisions-vs-go-cli)
   - [11.1 Foundational Decisions (FD)](#111-foundational-decisions-fd-001fd-012)
   - [11.2 Algebraic Structure (AS)](#112-algebraic-structure-as-001as-010)
   - [11.3 Store & Runtime (SR)](#113-store--runtime-architecture-sr-001sr-011)
   - [11.4 Protocol Decisions (PD)](#114-protocol-decisions-pd-001pd-006)
   - [11.5 Protocol Operations (PO)](#115-protocol-operations-po-001po-014)
   - [11.6 Snapshot & Query (SQ)](#116-snapshot--query-sq-001sq-010)
   - [11.7 Uncertainty & Authority (UA)](#117-uncertainty--authority-ua-001ua-012)
   - [11.8 Conflict & Resolution (CR)](#118-conflict--resolution-cr-001cr-007)
   - [11.9 Agent Architecture (AA)](#119-agent-architecture-aa-001aa-007)
   - [11.10 Interface & Budget (IB)](#1110-interface--budget-ib-001ib-012)
   - [11.11 Guidance System (GU)](#1111-guidance-system-gu-001gu-008)
   - [11.12 Lifecycle & Methodology (LM)](#1112-lifecycle--methodology-lm-001lm-016)
   - [11.13 Coherence & Reconciliation (CO)](#1113-coherence--reconciliation-co-001co-014)
   - [11.14 Aggregate Findings](#1114-aggregate-findings)
   - [11.15 Structural Observations](#1115-structural-observations)

---

## 1. Methodology

### Investigation Structure

Six waves of parallel analysis, deploying 24 subagents reading actual Go source code:

**Wave 1 — Orientation (4 agents):**
- Full codebase structure mapping (38 packages, 234 files, line counts)
- Prior audit/gap documents (4 spec documents from `../.ddis/specs/`)
- CLI specification architecture (9 modules, 97 INVs, 74 ADRs)
- Braid project resources (transcripts, references, existing artifacts)

**Wave 2 — Deep Subsystem Analysis (5 agents):**
- Datom store requirements vs. storage/oplog/events/materialize/causal
- Harvest/seed lifecycle vs. discover/discovery/autoprompt/refine/absorb
- Contradiction detection vs. consistency (6 tiers), challenge, drift, coverage
- Multi-agent coordination vs. merge, events, triage, progress, diff
- Interface principles vs. search, parser, query, autoprompt, llm, bundle

**Wave 3 — Cross-Cutting Analysis (4 agents):**
- Event-sourcing pipeline (events, materialize, projector, process)
- Self-bootstrap and traceability (annotate, workspace, skeleton, coverage, implorder)
- Triage and reconciliation taxonomy (triage, drift, absorb, refine)
- Test suite and quality gates (135 tests, 20 validation checks)

**Wave 4 — ADR Orientation & Ground Truth (5 agents):**
- Spec document inventory (7 spec documents, 20 ranked findings)
- Full codebase inventory (234 .go files, 61,906 LOC, 34 packages)
- Substrate layer analysis (39 tables, 357+ DELETE statements, relational vs. EAV)
- Verification layer analysis (5-tier contradiction engine, challenge system)
- Lifecycle & interface layer analysis (bilateral loop, harvest/seed, k* formula)

**Wave 5 — Deep ADR Coverage Analysis (5 agents):**
- UA (001–012) + AS (001–010): Uncertainty, authority, algebraic structure
- GU (001–008) + IB (001–012): Guidance system, interface, budget
- PO (001–014) + SQ (001–010): Protocol operations, snapshot, query
- CR (001–007) + CO (001–014) + AA (001–007): Conflict, coherence, agent architecture
- LM (001–016) + PD (001–006) + FD (001–012): Lifecycle, protocol, foundational

**Wave 6 — Verification & Cross-Check (2 agents):**
- Coverage completeness: verified 128/139 ADR entries covered; identified 11 uncovered SR entries
- Contested assessment verification: 5 critical claims spot-checked (4 confirmed, 1 refined — IB-005)

### Evaluation Criteria

Each module is assessed against the specific SEED.md section(s) it should implement:

- **ALIGNED**: Implements the SEED.md requirement correctly. The logic is proven and portable.
- **DIVERGENT**: Addresses the right problem but with a different mechanism than SEED.md specifies. Specific changes needed for alignment are documented.
- **EXTRA**: Implements something SEED.md doesn't address. Evaluated for spec inclusion vs. removal.
- **BROKEN**: Doesn't work regardless of spec alignment. From cleanroom audit findings.
- **MISSING**: SEED.md requires it; nothing in the codebase provides it.

---

## 2. The Central Finding: Substrate Divergence

SEED.md §4 specifies a datom store: `[entity, attribute, value, transaction, operation]` tuples in a grow-only set `(P(D), ∪)` with content-addressable identity, schema-as-data, per-attribute conflict resolution, and Datalog queries.

The Go CLI uses a relational database: 39 normalized tables with sequential integer primary keys, hardcoded DDL schema, SQL queries, and in-place mutation via UPDATE/DELETE.

This is not a surface-level difference. It pervades every layer:

| Property | SEED.md Requires | Go CLI Implements |
|----------|------------------|-------------------|
| Data model | `[e, a, v, tx, op]` tuples | Normalized rows in 39 tables |
| Identity | Content-addressable (same fact = same datom) | Sequential auto-incrementing IDs |
| Mutability | Append-only; retractions are new datoms | Mutable via UPDATE/DELETE |
| Schema | Schema-as-data (datoms defining attributes) | Hardcoded DDL in Go source (~600 LOC) |
| Queries | Datalog with stratified evaluation | Raw SQL via `database/sql` |
| Merge | Set union `(P(D), ∪)` — no conflicts | LWW at event level; SQL state is mutable |
| Conflict resolution | Per-attribute (lattice/LWW/multi-value) | Global LWW hardcoded in `causal/dag.go` |
| Temporal queries | Datom history is the canonical store | Event replay from JSONL + snapshot checkpoints |

**Implication for Braid**: The datom store is the **load-bearing novelty**. Everything else in the CLI — contradiction detection, guidance injection, fitness function, traceability — is proven logic that can be ported to a new substrate. The store itself must be built from scratch.

---

## 3. Module-by-Module Analysis

### 3.1 Storage Layer

**Package**: `internal/storage/` (3,896 LOC, 6 files)
**SEED.md Reference**: §4 (Core Abstraction: Datoms), Constraints C1–C4
**Verdict**: **DIVERGENT**

**What exists**: 39-table normalized SQLite schema (`schema.go`, ~600 LOC DDL) with typed models (`models.go`, 382 LOC), comprehensive CRUD operations (`queries.go`, `insert.go`), and domain-aware query helpers. The schema covers: spec metadata, sections, invariants, ADRs, cross-references, modules, quality gates, glossary terms, witnesses, challenges, transactions, events, snapshots, and session state.

**What SEED.md requires**: A grow-only set of `[e, a, v, tx, op]` datoms with content-addressable identity, schema-as-data, and query-time resolution.

**Specific divergences**:
1. **Sequential IDs vs. content-addressed**: Every table uses `id INTEGER PRIMARY KEY` auto-incrementing. Content hashes exist (`ContentHash string` in models) but are metadata, not identity keys.
2. **Mutable state**: `InsertInvariant` uses `ON CONFLICT DO UPDATE` — richer content wins. This is an in-place mutation, not an append.
3. **Hardcoded schema**: The entire schema is a static Go constant. Adding an attribute requires editing source and recompiling.
4. **No operation field**: No concept of assert/retract. Updates overwrite; deletes remove rows.

**What to preserve**: The 39-table schema is an **inventory of entity types and attributes** for the datom store. Every table → entity type, every column → attribute. This is the requirements document for Braid's schema-as-data bootstrap.

**Changes needed for alignment**: Build the datom store from scratch. Use the relational schema as a data dictionary to define the initial attribute datoms.

---

### 3.2 Parser

**Package**: `internal/parser/` (3,718 LOC, 19 files)
**SEED.md Reference**: §3 (Specification Formalism), §10 (Bootstrap)
**Verdict**: **ALIGNED** (with known bugs)

**What exists**: Full markdown spec parser supporting monolith and modular formats. Extracts sections, invariants, ADRs, quality gates, cross-references, negative specs, glossary terms, and frontmatter metadata. Supports parent-spec resolution for cross-spec references.

**What SEED.md requires**: The specification formalism (invariants with IDs and falsification conditions, ADRs with alternatives and rationale, negative cases, uncertainty markers) must be parseable and structured for migration into the datom store.

**Alignment**: The parser correctly extracts all DDIS specification elements with their structural components. The 19-file decomposition (sections, invariants, ADRs, frontmatter, patterns, cross-references) is well-organized.

**Known bugs** (from cleanroom audit):
- **H3**: Invariant extractor does not skip fenced code blocks — `**APP-INV-NNN:**` inside ``` blocks is extracted as real invariant
- **H4**: Negative spec extractor has the same code-block blindness
- ADR extractor correctly handles code blocks (asymmetric fix)

**Reusability**: HIGH. Parser logic transfers directly — the output changes from SQL INSERT to datom assertions, but extraction patterns remain identical. Code-block bug must be fixed in any port.

---

### 3.3 Event-Sourcing Pipeline

**Packages**: `internal/events/` (869 LOC), `internal/materialize/` (2,005 LOC), `internal/projector/` (321 LOC), `internal/causal/` (431 LOC)
**SEED.md Reference**: §4 (Datoms), §5 (Harvest/Seed), §6 (Reconciliation)
**Verdict**: **DIVERGENT** — right architecture, different substrate

**What exists**: A mature three-stream event-sourcing system:
- **3 JSONL streams**: Discovery (11 event types), Specification (20 types), Implementation (16 types)
- **Event model**: `[ID, Type, Timestamp, SpecHash, Stream, Payload, Causes, Version]` — 28 typed events
- **Fold engine**: Deterministic replay via `CausalSort` (Kahn's algorithm + timestamp tiebreaker) → `Apply` (pure function, no I/O) → `Applier` interface (17 methods for SQL mutations)
- **Snapshots**: Position-based checkpoints with SHA-256 `StateHash` over canonical content
- **3 stream processors**: Validation, Consistency, Drift (fire on event application, emit derived events)
- **Projector**: Pure functions rendering materialized SQL state back to markdown

**Key properties proven**:
- APP-INV-073: Fold determinism (pure Apply function, sorted input → identical output)
- APP-INV-075: Materialization idempotency (replay produces same state)
- APP-INV-020: Append-only JSONL streams (`O_APPEND | O_CREATE | O_WRONLY`)
- APP-INV-074: Causal ordering preserved via `Causes` field

**What SEED.md requires**: Datoms as the canonical store, not events. The distinction matters: events carry full payloads (`InvariantPayload` with 10+ fields) while datoms are atomic (`[entity, single_attribute, single_value, tx, op]`). Events are coarser-grained.

**Specific divergences**:
1. **Two-layer canonical source**: Events (JSONL) are canonical; SQL is derived by fold. SEED.md wants datoms AS the canonical state — no fold, no derived SQL layer.
2. **Event identity is random**: `evt-{timestamp}-{random_8_bytes}`. SEED.md requires content-addressable identity.
3. **Global LWW merge**: `causal/dag.go` lines 133–145 hardcode last-writer-wins. SEED.md requires per-attribute resolution modes.
4. **No operation field**: Events are implicitly assertions. Retractions are separate event types (`invariant_removed`, `witness_revoked`), not `op=retract` on the same datom.

**What to preserve**:
- `CausalSort` algorithm — directly applicable to datom transactions
- `StateHash` computation — canonical ordering + content-only hashing transfers to datom sets
- Fold determinism property — Braid achieves this differently (datoms are canonical) but the rigor of proof is valuable
- `RenderModule/RenderInvariant/RenderADR` — pure functions from structured data to markdown; input changes from SQL rows to Datalog query results
- Processor pattern — `RegisterProcessor` with derived event emission maps to datom write-path observers

---

### 3.4 Consistency Engine

**Package**: `internal/consistency/` (3,004 LOC, 11 files)
**SEED.md Reference**: §3 (Contradiction Detection tiers)
**Verdict**: **ALIGNED** (exceeds spec in most tiers; gaps in Tier 1 and Tier 5)

**What exists**: A 6-tier contradiction detection engine:

| CLI Tier | Mechanism | LOC | Confidence | Maps to SEED.md |
|----------|-----------|-----|------------|-----------------|
| Tier 2: Graph | Governance overlap (Jaccard on cross-ref reach sets) + negative spec violations | 333 | 0.50–0.80 | Tier 3 (Semantic) — partial |
| Tier 3: SAT | CNF encoding via gophersat CDCL solver; global variable namespace | 363 | 0.85 | Tier 2 (Logical) |
| Tier 4a: Heuristic | Polarity inversion, quantifier conflict, numeric bound conflict | 381 | 0.50–0.70 | Tier 3 (Semantic) — partial |
| Tier 4b: Semantic | TF-IDF + cosine similarity + polarity detection | 191 | 0.35–0.70 | Tier 3 (Semantic) |
| Tier 5: SMT | Z3 subprocess via SMT-LIB2 (QF_LIA, QF_UF, LIA) | 528 | 0.95 | Tier 2 (Logical) — extended |
| Tier 6: LLM | Anthropic Claude pairwise classification + 3-run majority vote | 189 | 0.75–0.90 | Tier 3 (Semantic) — via NLU |

**Key design properties**:
- Modular tier composition: each tier operates independently, results deduplicated by element pair
- Graceful degradation: Z3 and LLM silently skip if unavailable
- Zero false positive discipline: `APP-INV-019` — negation handling, self-UNSAT filtering, real-pattern validation
- Every contradiction includes confidence level and resolution hint

**Gaps against SEED.md**:
1. **Tier 1 (Exact) — MISSING**: No mechanism detects literal identical claims with different values (e.g., "timeout = 5s" vs "timeout = 10s"). This is the simplest tier and absent.
2. **Tier 4 (Pragmatic) — PARTIAL**: Governance overlap catches some composition conflicts but doesn't systematically check "compatible in isolation, incompatible in practice."
3. **Tier 5 (Axiological) — MISSING**: No mechanism checks "internally consistent but misaligned with goals." The fitness function (in `triage/`) measures alignment but doesn't feed contradictions to the consistency engine.

**Reusability**: HIGH. The 6-tier architecture is substrate-agnostic — tier algorithms (SAT encoding, TF-IDF similarity, Z3 translation) don't depend on how data is stored. Replace `storage.ListInvariants()` with Datalog queries; tier logic transfers directly.

---

### 3.5 Search Intelligence

**Package**: `internal/search/` (2,163 LOC, 7 files)
**SEED.md Reference**: §7 (Self-Improvement Loop), §8 (Interface Principles)
**Verdict**: **ALIGNED**

**What exists**: Triple-signal hybrid search:
- BM25 lexical search (term frequency / inverse document frequency)
- LSI semantic search (latent semantic indexing via truncated SVD)
- PageRank authority scoring (graph-based on cross-reference network)
- RRF fusion (Reciprocal Rank Fusion combining all three signals)

9-signal context bundle assembly (`search/context.go`, 820+ LOC):
1. Target content + metadata
2. Constraints (invariants, gates, negative specs)
3. Invariant completeness status
4. Coverage gaps
5. Local scoped validation
6. Reasoning mode tagging
7. LSI semantic similarity
8. Impact analysis (forward/backward)
9. Witness status + process compliance + recent oplog changes

**What SEED.md requires (§7)**: Retrieval heuristics that sharpen with use — significance accumulation where frequently-queried datoms surface first.

**Gap**: No significance accumulation. PageRank authority is static. No tracking of "which results were actually useful to the agent." The search system is correct and well-engineered but does not learn.

**Reusability**: HIGH. BM25, LSI, PageRank, and RRF are algorithm-level — they operate on term vectors and adjacency matrices, not on storage format. Context bundle assembly logic transfers; input changes from SQL queries to Datalog queries.

---

### 3.6 Validation Engine

**Package**: `internal/validator/` (3,451 LOC, 7 files)
**SEED.md Reference**: §3 (Specification Formalism), §10 (Bootstrap)
**Verdict**: **ALIGNED**

**What exists**: 20 composable mechanical checks:

| Checks 1–9 | Structural integrity (cross-refs, falsifiability, glossary, ownership, budget, declaration-definition bijection, manifest sync, negative specs) |
|-------------|---|
| Check 10 | Gate-1 structural conformance (required sections exist) |
| Checks 11–12 | Proportional weight, namespace consistency |
| Check 13 | Implementation traceability (code annotations → spec elements) |
| Checks 14–17 | Witness freshness, event stream VCS, behavioral witness, challenge freshness |
| Checks 18–20 | Process compliance, VCS tracking, lifecycle reachability |

Each check is composable (can run independently), deterministic, and returns structured results with severity levels (Error/Warning/Info).

**Alignment with SEED.md**: The validation engine directly implements SEED.md §3's requirement that "invariants are falsifiable claims that can be mechanically checked." The checks verify structural properties of the specification formalism.

**Reusability**: HIGH. Check logic is pure functions of spec data. Portable to Datalog queries — the 20 checks become 20 Datalog rules.

---

### 3.7 Drift Detection

**Package**: `internal/drift/` (1,166 LOC, 5 files)
**SEED.md Reference**: §6 (Reconciliation — Structural Divergence)
**Verdict**: **DIVERGENT** — detection is strong, resolution is incomplete

**What exists**: Multi-dimensional drift analysis:
- `ImplDrift`: unspecified (code exists, spec missing), unimplemented (spec exists, code missing), contradictions
- `Classification`: direction (impl-ahead/spec-ahead/mutual/contradictory), severity (additive/structural/contradictory), intentionality (planned/organic/accidental)
- `IntentDrift`: uncovered non-negotiables, purposeless elements
- `QualityBreakdown`: correctness, depth, coherence
- `Remediate()`: selects highest-priority item and generates exemplar + context + guidance

**What SEED.md requires**: Detection (covered) + automatic routing to resolution mechanism (not covered). Structural drift → bilateral loop (discover → absorb or refine). The CLI detects drift excellently but requires manual invocation of resolution commands.

**Changes needed**: Wire drift signals into an automatic dispatcher that triggers appropriate reconciliation mechanism based on classification.

---

### 3.8 Witness/Challenge System

**Package**: `internal/witness/` (1,445 LOC), `internal/challenge/` (1,064 LOC)
**SEED.md Reference**: §3 (Falsification), §6 (Bilateral Loop)
**Verdict**: **ALIGNED**

**What exists**: Adjoint pair implementing bilateral verification:
- **Witness** (left adjoint): 6 evidence types (test, annotation, scan, review, attestation, eval), 4 statuses (valid, stale, revoked, pending), automatic invalidation on spec change
- **Challenge** (right adjoint): 5-level progressive verification (formal SAT, uncertainty scoring, causal annotation check, test execution, meta cross-check)
- LLM-as-judge evaluation with 3-run majority vote for statistical soundness

**Alignment**: This directly implements SEED.md §3's requirement that invariants be falsifiable with explicit verification methods. The witness/challenge adjunction (APP-ADR-037) is mathematically sound.

**Reusability**: HIGH. Evidence types, verdict logic, and confidence scoring transfer to datom representation. Each witness/challenge becomes a set of datoms rather than SQL rows.

---

### 3.9 Triage Engine

**Package**: `internal/triage/` (1,592 LOC, 9 files)
**SEED.md Reference**: §3 (Fitness Function), §6 (Axiological Divergence)
**Verdict**: **ALIGNED**

**What exists**:
- **Fitness function** F(S) = weighted sum of 6 signals: validation (0.20), coverage (0.20), drift (0.20), challenge health (0.15), contradictions (0.15), issue backlog (0.10)
- **Well-founded ordering** μ(S) = (open_issues, unspecified, drift) ∈ ℕ³ with lexicographic ordering — mathematically proven terminating
- **Fixpoint criterion**: F(S) = 1.0 ↔ μ(S) = (0, 0, 0)
- **Issue lifecycle**: 7-state machine (filed → triaged → specified → implementing → verified → closed/wontfix)
- **Graph metrics**: PageRank, HITS, betweenness centrality for issue prioritization
- **Zero-knowledge protocol**: Self-contained JSON for agent participation without prior context

**Alignment**: SEED.md §3 requires "a fitness function quantifying convergence across coverage, depth, coherence, completeness, and formality." The CLI implements this with a 6-signal weighted sum. The Lyapunov convergence framework and well-founded ordering are formally sound.

**Gap**: The CLI's 6 signals don't exactly map to SEED.md's 5 dimensions (coverage, depth, coherence, completeness, formality). "Depth" and "formality" are implicit in the validation signal rather than explicit. This is a minor mapping issue, not a design flaw.

---

### 3.10 Discovery/Autoprompt

**Packages**: `internal/discover/` (1,826 LOC), `internal/discovery/` (1,633 LOC), `internal/autoprompt/` (437 LOC)
**SEED.md Reference**: §5 (Harvest/Seed Lifecycle), §8 (Guidance Injection)
**Verdict**: **DIVERGENT** — addresses the right problems but at the wrong lifecycle boundary

**What exists**:
- **Discovery threads**: Thread-based conversational spec authoring via `ddis discover`. Events recorded to `discovery.jsonl`. Thread state (parked, merged, active) with keyword-based convergence selection.
- **7 cognitive mode classification**: divergent, convergent, dialectical, abductive, metacognitive, incubation, crystallization
- **CommandResult triple**: Every command returns `(output, state, guidance)` — state monad discipline
- **k* budget**: Attention budget decays from 12 base → 3 floor as conversation depth increases. `TokenTarget(depth)` computes 2000→300 token compression target.
- **Attenuation**: Guidance shrinks as depth increases

**What SEED.md requires**:
- **Harvest** at session end: extract un-transacted knowledge, measure drift metric
- **Seed** at session start: assemble compact relevant summary from datom store
- **Guidance injection**: Every tool response includes methodology pointer (implemented)
- **Dynamic CLAUDE.md**: Adapts to observed drift patterns (not implemented)

**Critical divergence**: The discovery system operates **within sessions** (agent invokes `ddis discover` to get context), not **at session boundaries** (automatic harvest at end, automatic seed at start). This was confirmed by external project adoption: the bilateral lifecycle (discover → crystallize → absorb) has **0% adoption** because within a session, the agent's context window is strictly fresher than the tool's state.

**What to preserve**: CommandResult triple, k* budget formula, cognitive mode classification, guidance injection pattern.

**What to rebuild**: The entire harvest/seed boundary mechanism. Sessions must have explicit start/end markers. Harvest must be automatic. Seed must query the datom store for relevant knowledge.

---

### 3.11 Absorb/Refine

**Packages**: `internal/absorb/` (1,206 LOC), `internal/refine/` (1,591 LOC)
**SEED.md Reference**: §6 (Reconciliation — Bilateral Loop)
**Verdict**: **DIVERGENT** — bilateral concept correct, lifecycle integration missing

**What exists**:

**Absorb** (code → spec direction):
- Scans code for annotations (high confidence) and heuristic patterns (assertions, guards, state transitions)
- Bidirectional matching: forward (code pattern → best matching invariant/ADR) + reverse (spec element → check if any pattern references it)
- Three reconciliation categories: correspondences, undocumented behavior, unimplemented spec

**Refine** (spec → improved spec direction):
- **Plan**: Select weakest quality dimension (completeness, coherence, depth, coverage, formality)
- **Apply**: Find worst element on selected dimension, assemble Gestalt-optimized prompt with exemplars
- **Judge**: Measure drift before/after, enforce monotonicity (drift increase → halt and revert)

**What SEED.md requires**: These mechanisms should be **automatically triggered by divergence signals**, not manually invoked. Structural drift → refine should fire automatically. Undocumented behavior → discover should transact findings as datoms.

**What to preserve**: Bidirectional reconciliation pattern, dimension-first selection in refine, monotonicity enforcement in judge, exemplar selection with WeakScore scoring.

**What to rebuild**: Signal-driven triggering. Findings must become datom assertions, not one-time reports.

---

### 3.12 Annotation System

**Package**: `internal/annotate/` (951 LOC, 5 files)
**SEED.md Reference**: §3 (Traceability), Constraint C5
**Verdict**: **ALIGNED**

**What exists**: Production-tested bidirectional annotation system:
- 8 annotation verbs: `implements`, `maintains`, `interfaces`, `tests`, `requires`, `satisfies`, etc.
- 14+ language support (Go, Rust, Python, TypeScript, Java, C/C++, Ruby, Shell, etc.)
- `Scan()`: Walks directory tree, extracts all `ddis:` annotations
- `Verify()`: Cross-checks annotations against spec database (resolved, orphaned, unimplemented)
- As of 2026-02-28: 511 annotations, 0 orphaned, 0 unimplemented

**Alignment**: Directly implements SEED.md constraint C5 (traceability). The annotation grammar is language-agnostic and production-proven.

**Reusability**: VERY HIGH. Port grammar as-is. Annotations become datom assertions: `[code_location, <verb>, spec_element, tx, assert]`.

---

### 3.13 Coverage/Implorder/Progress

**Packages**: `internal/coverage/` (577 LOC), `internal/implorder/` (370 LOC), `internal/progress/` (471 LOC)
**SEED.md Reference**: §3 (Fitness Function), §10 (Staging)
**Verdict**: **ALIGNED**

**What exists**:
- **Coverage**: Component-level completeness analysis. Counts fields present (statement, violation, validation method, etc.) per invariant. 100% on CLI spec.
- **Implorder**: Kahn's topological sort on module-level dependency DAG with SCC condensation for cycle handling. Produces optimal implementation ordering.
- **Progress**: DAG frontier partitioning into done/ready/blocked invariants with authority-based prioritization.

**Reusability**: HIGH. These are algorithm-level utilities that operate on graph structures. Input changes from SQL queries to Datalog queries; algorithms remain identical.

---

### 3.14 CLI Command Layer

**Package**: `internal/cli/` (10,095 LOC, 54 files)
**SEED.md Reference**: §8 (Interface Principles — CLI layer)
**Verdict**: **ALIGNED** (as reference for Braid's CLI design)

**What exists**: 45+ commands organized into 5 groups (core, investigate, improvement, planning, utility). Global `-q` flag suppresses guidance. JSON output mode for machine consumption. Agent-facing command catalog (`ddis commands` outputs JSON).

**Key commands**: parse, validate, coverage, drift, context, search, exemplar, discover, refine, absorb, witness, challenge, contradict, materialize, crystallize, snapshot, triage, issue, bisect, etc.

**Guidance injection**: 30+ commands emit `"Next: ddis <command>"` postscripts. Recovery hints on errors (`emitRecoveryHint()`).

**Reusability**: MEDIUM. Command signatures and UX patterns are valuable design references. The actual Go code (cobra command definitions, flag parsing) is language-specific and won't transfer to Rust.

---

### 3.15 Skeleton/Workspace

**Packages**: `internal/skeleton/` (398 LOC), `internal/workspace/` (952 LOC)
**SEED.md Reference**: §10 (Bootstrap)
**Verdict**: **EXTRA** — useful but not required by SEED.md

**What exists**:
- **Skeleton**: Generates DDIS-conformant specification templates (manifest, constitution, module stubs)
- **Workspace**: Initializes `.ddis/` project structure (database, oplog, event streams, discovery threads), progressive validation (L1-L3), task derivation from artifacts

**SEED.md coverage**: SEED.md doesn't specify template generation or workspace initialization as core features. These are developer experience features.

**Recommendation**: Include in Braid's roadmap but defer past Stage 0. Useful for onboarding new projects to DDIS.

---

### 3.16 LLM Provider

**Package**: `internal/llm/` (209 LOC, 2 files)
**SEED.md Reference**: §2 (AI Agent Problem)
**Verdict**: **ALIGNED**

**What exists**: Abstracted `Provider` interface with graceful degradation. `AnthropicProvider` via net/http (no SDK dependency). Used by witness/eval (LLM-as-judge) and consistency/llm (pairwise semantic contradiction). `Available()` gates all LLM features; missing API key → silent skip.

**Reusability**: MEDIUM. Interface pattern transfers; implementation is Go-specific.

---

## 4. MISSING: Capabilities Required by SEED.md

These capabilities are explicitly required by SEED.md and have **no implementation** in the Go CLI:

### 4.1 Datom Store (§4, Constraints C1–C4) — **CRITICAL**

The fundamental data structure: `[entity, attribute, value, transaction, operation]` tuples in a grow-only set with content-addressable identity. **Nothing in the CLI implements this.** The CLI uses relational tables. This is Stage 0's primary deliverable.

### 4.2 Schema-as-Data (§4, Constraint C3)

Schema defined as datoms in the store. Schema evolution is a transaction, not a code change. The CLI hardcodes its schema in Go source.

### 4.3 Datalog Query Engine (§11)

Declarative queries with stratified evaluation, natural graph joins, and monotonic/non-monotonic distinction. The CLI uses imperative SQL.

### 4.4 Per-Attribute Resolution Modes (§4, Axiom 5)

Each attribute declares lattice-resolved, last-writer-wins, or multi-value conflict resolution. Lattice definitions stored as datoms. The CLI hardcodes global LWW.

### 4.5 Harvest Operation (§5)

End-of-session extraction of un-transacted knowledge into the store with drift metric (what the agent knew vs. what the store knows). Not implemented.

### 4.6 Seed Operation (§5)

Start-of-session assembly of relevant knowledge from the datom store: active invariants, unresolved uncertainties, recent decisions, recommended next actions. Not implemented.

### 4.7 Dynamic CLAUDE.md Generation (§7)

System tracks drift patterns (what agents forget, which checks they skip) and generates operating instructions that preemptively correct observed failure patterns. Not implemented.

### 4.8 Agent Frontier Tracking (§4, Axiom 3)

Per-agent set of known datoms. Default query mode is the local frontier. The CLI has no per-agent state.

### 4.9 Sync Barriers (§6)

Consistent cuts where all participants agree on the same facts. Required for non-monotonic queries in multi-agent settings. Not implemented.

### 4.10 Signal System (§6)

Unified divergence routing: confusion signals (epistemic), conflict signals (logical), drift signals (structural), goal-drift signals (axiological). The CLI detects divergence in scattered modules but has no unified dispatcher.

### 4.11 Deliberation/Decision Records (§6)

Structured resolution for conflicts: competing positions, argumentation, decision with rationale, queryable precedent. Not implemented (ADRs exist for design decisions but not for runtime conflict resolution).

### 4.12 MCP Interface (§8)

Machine-to-machine protocol layer for structured tool access. The recommendation document (`RECOMMENDATION_MCP_PROTOCOL_SERVER_2026-03-01.md`) designs an 11-tool MCP surface but none is implemented.

### 4.13 TUI Dashboard (§8)

Human monitoring interface for real-time visibility into spec state, drift, witness freshness. Not implemented.

### 4.14 Significance Accumulation (§7)

Search results weighted by historical usefulness. Frequently-queried datoms surface first. Connections traversed together strengthen together. Not implemented.

### 4.15 Agent Working Set / Patch Branches (Transcript 04 PQ1, Transcript 05) — **CRITICAL**

**Design Decision**: Transcript 04 (`04-datom-protocol-interface-design.md`, lines 210–240) presents three options for whether agents should have private/local state. Claude recommended **Option B — Two-tier store**: a private working set W_α per agent plus the shared committed store S. The user confirmed this at line 373 ("Agree with all of the above") and significantly extended the design with two additions:

1. **Patch branches**: "Writes can be executed as patch branches that are layered on top of the current state and can be committed, combined, etc." Multiple agents can execute competing implementations of the same spec invariants, then the best result is chosen or all options combined. The user identified a bilateral dual: both spec ideation and implementation share this forking/reduction pattern.

2. **Query-driven significance**: "Queries/Projections can serve as an input into datom significance (similar to how neural connections are strengthened with repeated access)." Frequently-accessed datoms and projections are given greater epistemic weight.

**What was designed** (Transcript 05, lines 849–861): A full `ddis_branch` tool with operations:
- `fork` — create a patch branch layered on top of current state
- `commit` — promote branch datoms to the shared store
- `combine` — merge multiple branches (bilateral reduction)
- `compare` — diff competing branches
- `abandon` — discard branch
- `list` — enumerate active branches
- `competing_with` parameter — explicit competitive branching between agents

**Key architectural properties**:
- W_α uses the same datom structure as the shared store S — promoting a datom from W_α to S is just a TRANSACT operation
- Agents can query over W_α ∪ S locally, getting scratchpad state for reasoning while only sharing the committed subset
- The CRDT properties (set union merge) hold on S; W_α is agent-internal and opaque to the protocol
- This preserves Axiom A2 (grow-only store) on the shared store while allowing agent-local flexibility

**CLI Status**: **NOT IMPLEMENTED**. The closest analog is `internal/state/` (session KV store, ~200 LOC), which is a flat key-value store with no datom structure, no commit/fork semantics, and no competitive branching. The Go CLI's `session_state` table is structurally wrong for this purpose.

**SEED.md Status**: **NOT CAPTURED**. This settled design decision (Transcript 04 PQ1, confirmed by user) was never formalized into SEED.md. The concept survived through the tool design in Transcript 05 but fell out of the canonical document. This is itself an instance of the harvest gap the system is designed to prevent — a settled decision that dropped between sessions.

**Three-way gap**:
1. **SEED.md vs. Transcripts**: Decision exists in Transcript 04 (confirmed) and elaborated in Transcript 05 (`ddis_branch` tool spec), but SEED.md never captured it
2. **Go CLI vs. Design**: No concept of agent-local working sets or patch branches in existing code
3. **Gap Analysis vs. Reality**: This analysis originally missed this entire capability because SEED.md is silent on it

**Staging**: This is a Stage 2 concern (Branching + Deliberation per SEED.md §10), but the W_α local store design must be accounted for in Stage 0's store architecture to avoid a costly retrofit.

---

## 5. BROKEN: Known Defects

From the cleanroom audit (`cleanroom-audit-2026-03-01.md`), 4 HIGH severity issues:

### 5.1 H1: Quality Gate Identity Collapse

**Location**: Event import → materialize pipeline
**Impact**: All quality gates collapse to `APP-G-0` during import→materialize round-trip. `GateNumber` zero-defaults cause complete data loss.
**Violates**: APP-INV-072 (Event Content Completeness)
**Remediation**: APP-INV-110 (Quality Gate Identity Preservation) specified in remediation plan

### 5.2 H2: `event_provenance` Table Dead Code

**Location**: `internal/storage/schema.go`, `internal/causal/`
**Impact**: Table created but never populated. `Provenance()` checks only 2 of 10+ payload field names.
**Violates**: APP-INV-084 (Causal Provenance)
**Remediation**: APP-INV-112 (Provenance Table Population) specified

### 5.3 H3: Invariant Parser Code-Block Blindness

**Location**: `internal/parser/` — invariant extraction
**Impact**: `**APP-INV-NNN:**` inside markdown code fences extracted as real invariant. ADR extractor correctly handles this; invariant extractor does not.
**Remediation**: APP-INV-111 (Parser Code-Block Isolation) specified

### 5.4 H4: Negative Spec Parser Code-Block Blindness

**Location**: `internal/parser/` — negative spec extraction
**Impact**: `**DO NOT**` inside code examples extracted as real negative spec constraint.
**Remediation**: Same as H3 — APP-INV-111

### 5.5 Additional Medium Severity (28 items)

The cleanroom audit found 28 MEDIUM severity issues across themes:
- Applier field coverage gaps (5 issues): `UpdateInvariant` handles 3/7 fields
- Stream integrity (3 issues): witness events routing to wrong stream
- Projection completeness (1 issue): `RenderModule` omits glossary, cross-refs, quality gates
- Atomicity violations (1 issue): witness write SQL first, then event — crash = split-brain
- Measurement gaps (4 issues): coverage counts superseded ADRs, drift contradictions always 0
- Merge correctness (1 issue): `Merge(A,B) ≠ Merge(B,A)` for same-ID events — breaks commutativity
- Dead code (2 issues): `formatInvariant()`, `ManifestUpdatePayload`, `SnapshotPayload` unused

**Worth fixing in CLI?** Per SEED.md §9, the question is whether to fix the existing CLI or build from scratch. Given that Braid uses a fundamentally different substrate, these bugs are most valuable as **test cases for Braid** — each represents a correctness property that Braid must satisfy.

---

## 6. EXTRA: Features Beyond the Spec

These features exist in the CLI but are not explicitly required by SEED.md:

### 6.1 Bisect Command

Binary search through commit history to find regressions. Useful developer tool.
**Recommendation**: Defer for Braid. Not core to the datom-store-based architecture.

### 6.2 Cascade Analysis

Module-level cascade impact analysis using reverse cross-reference lookup.
**Recommendation**: Include in Braid's query layer as a Datalog rule, not a separate module.

### 6.3 Bundle Assembly

Three-tier domain assembly (constitution + module + interface stubs) with budget-constrained output.
**Recommendation**: Include as Stage 1 deliverable (budget-aware output). The pullback assembly pattern is useful.

### 6.4 Exemplar Generation

Corpus-derived demonstrations with gap detection and scoring.
**Recommendation**: Include in Braid's refine mechanism. The exemplar selection pattern (WeakScore) is valuable.

### 6.5 State/Checkpoint Commands

Session state KV store and quality gate runner.
**Recommendation**: Session state becomes the harvest/seed mechanism in Braid. Quality gate runner transfers to Datalog validation rules.

### 6.6 Process Compliance Scoring

4-signal observational scoring (spec-first ratio, tool usage, witness coverage, validation gating).
**Recommendation**: Include as input to Dynamic CLAUDE.md generation (Stage 1). The compliance signals are the right input for procedural drift detection.

---

## 7. Cross-Cutting Concerns

### 7.1 Self-Bootstrap

**SEED.md Requirement (C7)**: DDIS specifies itself. The specification elements become the first data the system manages.

**CLI Status**: **80% achieved**. The CLI successfully validates its own specification:
- `pipeline_integration_test.go` parses `ddis-cli-spec/manifest.yaml` and runs 20 validation checks
- `roundtrip_test.go` verifies byte-for-byte parse→render fidelity
- F(S) = 1.0 fixpoint achieved (commit `87c02d0`)

**Gap**: Spec elements are SQL rows, not datoms. The spec is an external artifact that the tool analyzes, not the first dataset the store manages. Braid's first act must be: transact SPEC.md elements as datoms → run contradiction detection on them.

### 7.2 Traceability

**SEED.md Requirement (C5)**: Every implementation artifact traces to spec. Every spec element traces to goals. Orphans are defects.

**CLI Status**: **Comprehensive, verified bidirectional traceability**:
- 511 code annotations across 30+ packages, 0 orphaned, 0 unimplemented
- 8 annotation verbs, 14+ language support
- Validator Check 13 verifies file/function existence
- 97/97 INVs + 74/74 ADRs have code implementations

**Gap**: Traceability requires explicit scanning (`ddis scan --code-root`). In Braid, annotations become datoms — traceability is a query, not a scan.

### 7.3 Reconciliation Taxonomy

**SEED.md §6 defines 8 reconciliation mechanisms.** CLI coverage:

| Mechanism | SEED.md §6 | CLI Status | Gap |
|-----------|------------|------------|-----|
| **Harvest** | Epistemic divergence | NOT IMPLEMENTED | No session boundary detection |
| **Associate/Assemble** | Prevent ignorance-driven divergence | PARTIAL (`context.go`) | Not automatic at session start |
| **Guidance** | Steer agents | IMPLEMENTED (CommandResult triple) | Not signal-driven |
| **Merge** | Combine independent stores | PARTIAL (LWW event merge) | Not set union; no per-attribute modes |
| **Deliberation/Decision** | Structured conflict resolution | NOT IMPLEMENTED | ADRs exist but for design, not runtime |
| **Signal** | Route divergence to resolver | NOT IMPLEMENTED | Detection scattered across modules |
| **Sync Barrier** | Consistent cuts | NOT IMPLEMENTED | No per-agent frontiers |
| **Bilateral Loop** | Spec↔impl alignment | PARTIAL (witness/challenge, drift) | Manual invocation, not automatic |

**Key insight**: The CLI implements **detection** for 5 of 8 divergence types but lacks a **unified resolution framework**. Each mechanism operates independently with no dispatcher or signal system.

### 7.4 Test Coverage

**CLI Status**: 135 test functions across 25 files (~12,000 LOC):
- 79 behavioral tests directly tied to invariants
- Real spec self-bootstrapping (tests run against `ddis-cli-spec/`)
- Property-based verification (round-trip, idempotency, determinism)
- 20 automated validation checks

**What Braid needs additionally** (estimated ~110 new tests):
- Append-only store invariant (datoms never deleted)
- CRDT merge by set union (conflict-free)
- Harvest/seed lifecycle (knowledge persistence across sessions)
- Content-addressed identity (same fact → same datom)
- Causal transaction provenance
- Query monotonicity (adding facts only adds results)
- Multi-agent frontier synchronization
- Deliberation protocol correctness

---

## 8. Reusability Assessment

### Directly Portable (algorithm-level, substrate-independent)

| Component | Source | LOC | Why Portable |
|-----------|--------|-----|--------------|
| CausalSort (Kahn's + timestamp tiebreaker) | `materialize/fold.go` | ~60 | Operates on any DAG of causally-ordered items |
| 6-tier contradiction detection | `consistency/` | 3,004 | Tier algorithms are independent of storage |
| BM25 + LSI + PageRank + RRF search | `search/` | 2,163 | Operates on term vectors and adjacency matrices |
| Annotation grammar (8 verbs, 14+ langs) | `annotate/grammar.go` | ~200 | Language-agnostic pattern matching |
| Fitness function + Lyapunov convergence | `triage/fitness.go` | ~120 | Pure mathematical formula |
| k* attention budget + attenuation | `autoprompt/budget.go` | ~50 | Independent of storage layer |
| Coverage analysis (component completeness) | `coverage/` | 577 | Counts fields present vs. required |
| Implementation ordering (topological sort) | `implorder/` | 370 | Kahn's algorithm on dependency DAG |
| StateHash (canonical content hashing) | `materialize/diff.go` | ~60 | Deterministic ordering + SHA-256 |
| Render functions (structured data → markdown) | `projector/render.go` | 321 | Pure functions; input format changes |

### Reusable Patterns, New Substrate

| Pattern | Source | Adaptation |
|---------|--------|------------|
| CommandResult triple `(output, state, guidance)` | `autoprompt/` | Same pattern; state queries change from SQL to Datalog |
| Applier interface (17 mutation methods) | `materialize/fold.go` | Methods become datom transact calls |
| Bidirectional reconciliation (forward + reverse) | `absorb/reconcile.go` | Output becomes datom assertions, not reports |
| Dimension-first refine with monotonicity guard | `refine/` | Same approach; quality signals from datom queries |
| Event type validation | `events/schema.go` | Becomes schema-as-data validation |
| 28 typed event payloads | `events/payloads.go` | Each payload → set of entity attributes in datom schema |

### Must Be Rebuilt

| Component | Reason |
|-----------|--------|
| Storage layer (3,896 LOC) | Relational → EAV datom store |
| Query layer | SQL → Datalog |
| Schema definition | Hardcoded DDL → schema-as-data datoms |
| Merge operation | LWW events → set union datoms with per-attribute resolution |
| Harvest/Seed | Does not exist; must be built |
| Dynamic CLAUDE.md | Does not exist; must be built |
| Signal system | Does not exist; must be built |
| Deliberation records | Does not exist; must be built |
| Frontier tracking | Does not exist; must be built |
| Sync barriers | Does not exist; must be built |
| MCP interface | Does not exist; must be built |

---

## 9. Stage 0 Impact Analysis

SEED.md §10 defines Stage 0 as: "Harvest/Seed Cycle — validate the core hypothesis: harvest/seed transforms workflow from 'fight context loss' to 'ride context waves.'"

**Stage 0 deliverables**: `transact`, `query`, `status`, `harvest`, `seed`, `guidance`, dynamic CLAUDE.md generation.

**What can be adapted from CLI for Stage 0**:

| Deliverable | CLI Source | Adaptation Effort |
|-------------|-----------|-------------------|
| `transact` | `events/stream.go` (append pattern) | Low — same `O_APPEND` pattern, different serialization |
| `query` | `query/query.go`, `search/` | High — SQL→Datalog rewrite |
| `status` | `triage/fitness.go`, `coverage/` | Medium — formula stays, input queries change |
| `harvest` | None | **Build from scratch** |
| `seed` | `search/context.go` (9-signal bundle) | Medium — assembly logic stays, queries change to datom store |
| `guidance` | `autoprompt/budget.go`, CLI guidance injection | Low — k* formula + CommandResult pattern directly portable |
| dynamic CLAUDE.md | `process/compliance.go` (signals) | High — signal detection exists, generation is new |

**Stage 0 critical path**: Build datom store → implement transact/query → build harvest/seed → port guidance injection → implement dynamic CLAUDE.md. The first two are the hardest; guidance ports easily.

---

## 10. Summary Tables

### 10.1 Complete Module Classification

| Module | LOC | Category | SEED.md § | Key Finding |
|--------|-----|----------|-----------|-------------|
| `storage/` | 3,896 | DIVERGENT | §4 | Relational schema vs. datom store |
| `parser/` | 3,718 | ALIGNED | §3, §10 | Correct extraction; code-block bugs (H3, H4) |
| `events/` | 869 | DIVERGENT | §4, §5 | Right pattern (append-only JSONL); wrong granularity (events, not datoms) |
| `materialize/` | 2,005 | DIVERGENT | §4 | Proven fold determinism; two-layer model (events→SQL) vs. datom-only |
| `projector/` | 321 | ALIGNED | §8 | Pure render functions; input format changes |
| `causal/` | 431 | DIVERGENT | §4 | Causal DAG + merge exist; uses LWW not per-attribute resolution |
| `consistency/` | 3,004 | ALIGNED | §3 | 6-tier engine exceeds spec in tiers 2–4; gaps in tiers 1 and 5 |
| `search/` | 2,163 | ALIGNED | §7, §8 | Hybrid BM25+LSI+PageRank; no significance accumulation |
| `validator/` | 3,451 | ALIGNED | §3 | 20 composable checks; portable to Datalog rules |
| `drift/` | 1,166 | DIVERGENT | §6 | Strong detection; no automatic resolution routing |
| `witness/` | 1,445 | ALIGNED | §3 | Adjoint pair with challenge; transfers to datoms |
| `challenge/` | 1,064 | ALIGNED | §3 | 5-level progressive verification; sound |
| `triage/` | 1,592 | ALIGNED | §3 | Lyapunov convergence, well-founded ordering |
| `discover/` | 1,826 | DIVERGENT | §5 | Within-session context, not session-boundary harvest/seed |
| `discovery/` | 1,633 | DIVERGENT | §5 | Event emission for discovery; no harvest mechanism |
| `autoprompt/` | 437 | DIVERGENT | §8 | k* budget computed but never enforced on output size |
| `refine/` | 1,591 | DIVERGENT | §6 | Manual invocation; should be signal-triggered |
| `absorb/` | 1,206 | DIVERGENT | §6 | Findings are reports, not datom assertions |
| `annotate/` | 951 | ALIGNED | §3 (C5) | 8 verbs, 14+ languages, production-proven |
| `coverage/` | 577 | ALIGNED | §3 | Component-level completeness |
| `implorder/` | 370 | ALIGNED | §10 | Topological sort with SCC |
| `progress/` | 471 | ALIGNED | §10 | DAG frontier partitioning |
| `query/` | 682 | DIVERGENT | §4 | SQL queries; needs Datalog |
| `diff/` | 599 | ALIGNED | §6 | Structural diff; no three-way merge |
| `cli/` | 10,095 | ALIGNED | §8 | Comprehensive command structure |
| `workspace/` | 952 | EXTRA | — | Useful but not in SEED.md |
| `skeleton/` | 398 | EXTRA | — | Template generation; defer |
| `bundle/` | 220 | EXTRA | §8 | Budget-constrained assembly; include Stage 1 |
| `exemplar/` | 799 | EXTRA | — | Corpus demonstrations; include in refine |
| `checklist/` | 579 | EXTRA | — | Verification prompt generation |
| `state/` | 75 | EXTRA | — | Session KV store; superseded by harvest/seed |
| `oplog/` | 406 | DIVERGENT | §4 | Append-only log; not datom-structured |
| `process/` | 587 | DIVERGENT | §7 | Compliance signals exist; no CLAUDE.md generation |
| `llm/` | 209 | ALIGNED | §2 | Abstracted provider with graceful degradation |
| `impact/` | 380 | ALIGNED | §6 | BFS forward/backward; portable |
| `cascade/` | 198 | EXTRA | — | Module cascade; Datalog rule in Braid |
| `renderer/` | 92 | ALIGNED | §8 | Markdown rendering |

### 10.2 MISSING Capabilities Summary

| Capability | SEED.md § | Stage | Blocking? |
|------------|-----------|-------|-----------|
| Datom store `[e,a,v,tx,op]` | §4 | 0 | YES — everything depends on this |
| Schema-as-data | §4 (C3) | 0 | YES — schema evolution via transactions |
| Datalog query engine | §11 | 0 | YES — all queries depend on this |
| Harvest operation | §5 | 0 | YES — Stage 0 deliverable |
| Seed operation | §5 | 0 | YES — Stage 0 deliverable |
| Per-attribute resolution | §4 | 0–1 | Partial — LWW works for Stage 0 |
| Dynamic CLAUDE.md | §7 | 0–1 | Stage 0 deliverable |
| Budget-aware compression | §8 | 1 | Stage 1 deliverable |
| Signal system | §6 | 1 | Stage 1 deliverable |
| MCP interface | §8 | 1–2 | Stage 1–2 deliverable |
| Agent frontier tracking | §4 | 3 | Stage 3 deliverable |
| Sync barriers | §6 | 3 | Stage 3 deliverable |
| Deliberation/Decision | §6 | 2–3 | Stage 2–3 deliverable |
| Significance accumulation | §7 | 4 | Stage 4 deliverable |

### 10.3 DIVERGENT Items: What Changes Are Needed

| Module | Current Approach | Required Approach | Change Type |
|--------|-----------------|-------------------|-------------|
| `storage/` | 39-table normalized SQL | EAV datom store with content-addressed identity | Rebuild |
| `events/` | Typed event payloads in JSONL | Atomic datom assertions `[e,a,v,tx,op]` | Rebuild |
| `materialize/` | Fold events → SQL state | Datoms ARE the state; no fold needed | Architectural |
| `causal/` | Global LWW merge | Per-attribute resolution (lattice/LWW/multi-value) | Extend |
| `query/` | SQL via `database/sql` | Datalog with stratified evaluation | Rebuild |
| `discover/` | Within-session tool invocation | Session-boundary harvest/seed | Redesign |
| `autoprompt/` | k* computed but never enforced | Compress output to `TokenTarget(depth)` tokens | Wire existing |
| `drift/` | Detect and report | Detect → classify → route to resolver automatically | Extend |
| `refine/` | Manual invocation | Signal-triggered by structural drift | Wire signals |
| `absorb/` | Reports as output | Findings become datom assertions | Redesign output |
| `oplog/` | Append-only operation log | Subsumed by datom transaction history | Replace |
| `process/` | Compliance signals computed | Signals → dynamic CLAUDE.md generation | Extend |

---

## Appendix A: Terminology Mapping

| SEED.md Term | Go CLI Equivalent | Notes |
|--------------|-------------------|-------|
| Datom | Event + SQL row | Two-layer: events are canonical, SQL is derived |
| Store `(P(D), ∪)` | JSONL event streams + SQLite DB | Events approximate the append-only set; DB is mutable |
| Transaction entity | Event with `Causes` field | Transactions exist as SQL table but mostly unused |
| Schema-as-data | `schema.go` DDL | Opposite: schema-as-code |
| Harvest | — | Not implemented |
| Seed | `ddis context` (partial) | Within-session context assembly, not session-start seed |
| Frontier | `progress/` (work frontier) | Different concept: work-readiness vs. knowledge frontier |
| Signal | — | Not implemented as unified concept |
| Deliberation | ADR (design decisions only) | No runtime conflict resolution |
| Dynamic CLAUDE.md | Static `CLAUDE.md` files | No adaptation to observed drift patterns |
| Fitness function F(S) | `triage/fitness.go` (6 signals) | Aligned; different signal set |

## Appendix B: File Locations Referenced

All paths relative to `/data/projects/ddis/ddis-cli/`:

```
internal/storage/          — Schema, models, queries (3,896 LOC)
internal/parser/           — Markdown spec parser (3,718 LOC)
internal/events/           — Event envelope, payloads, streams (869 LOC)
internal/materialize/      — Fold engine, diff, snapshots (2,005 LOC)
internal/projector/        — State → markdown rendering (321 LOC)
internal/causal/           — DAG, merge, provenance (431 LOC)
internal/consistency/      — 6-tier contradiction engine (3,004 LOC)
internal/search/           — BM25+LSI+PageRank+RRF (2,163 LOC)
internal/validator/        — 20 mechanical checks (3,451 LOC)
internal/drift/            — Multi-dimensional drift analysis (1,166 LOC)
internal/witness/          — Proof receipts, 6 evidence types (1,445 LOC)
internal/challenge/        — 5-level progressive verification (1,064 LOC)
internal/triage/           — Fitness, convergence, issue lifecycle (1,592 LOC)
internal/discover/         — Discovery threads, context (1,826 LOC)
internal/discovery/        — Discovery event emission (1,633 LOC)
internal/autoprompt/       — k* budget, mode classification (437 LOC)
internal/refine/           — Plan/apply/judge loop (1,591 LOC)
internal/absorb/           — Code→spec reconciliation (1,206 LOC)
internal/annotate/         — Annotation grammar, scanning (951 LOC)
internal/coverage/         — Component completeness (577 LOC)
internal/implorder/        — Topological sort (370 LOC)
internal/progress/         — DAG frontier partitioning (471 LOC)
internal/query/            — Fragment retrieval (682 LOC)
internal/diff/             — Structural diff (599 LOC)
internal/cli/              — 54 command implementations (10,095 LOC)
tests/                     — 25 test files, 135 functions (12,032 LOC)
```

---

## 11. ADR Coverage Analysis: All 139 Design Decisions vs. Go CLI

**Date**: 2026-03-02 (updated with Waves 4–6 findings)
**Method**: Systematic evaluation of every ADR in `ADRS.md` (139 entries across 14 categories) against actual Go CLI source code. Each entry assessed by dedicated subagents reading source files, verified by cross-check agents.

**Aggregate coverage (139 ADRs)**:

| Status | Count | % | Meaning |
|--------|-------|---|---------|
| IMPLEMENTED | 12 | 9% | Decision fully reflected in CLI code |
| PARTIAL | 41 | 29% | Relevant components exist; incomplete or informal |
| DIVERGENT | 10 | 7% | Addresses the same problem with an incompatible mechanism |
| MISSING | 66 | 48% | No implementation exists |
| N/A | 10 | 7% | Braid-specific decision; not assessable against Go CLI |

**Per-category breakdown**:

| Category | Total | IMPL | PARTIAL | DIVG | MISS | N/A |
|----------|-------|------|---------|------|------|-----|
| FD (Foundational) | 12 | 2 | 2 | 4 | 2 | 2 |
| AS (Algebraic Structure) | 10 | 0 | 2 | 0 | 7 | 1 |
| SR (Store & Runtime) | 11 | 0 | 3 | 3 | 4 | 1 |
| PD (Protocol) | 6 | 0 | 3 | 0 | 3 | 0 |
| PO (Protocol Ops) | 14 | 0 | 4 | 2 | 7 | 1 |
| SQ (Snapshot & Query) | 10 | 0 | 4 | 0 | 6 | 0 |
| UA (Uncertainty & Authority) | 12 | 0 | 3 | 0 | 9 | 0 |
| CR (Conflict & Resolution) | 7 | 0 | 1 | 0 | 6 | 0 |
| AA (Agent Architecture) | 7 | 0 | 3 | 0 | 3 | 1 |
| IB (Interface & Budget) | 12 | 1 | 5 | 0 | 5 | 1 |
| GU (Guidance System) | 8 | 0 | 2 | 0 | 6 | 0 |
| LM (Lifecycle & Methodology) | 16 | 5 | 3 | 1 | 4 | 3 |
| CO (Coherence & Reconciliation) | 14 | 4 | 6 | 0 | 4 | 0 |

---

### 11.1 Foundational Decisions (FD-001–FD-012)

| ADR | Decision | Status | Evidence |
|-----|----------|--------|----------|
| FD-001 | Append-Only Store | **DIVERGENT** | Events use O_APPEND (verified `events/stream.go`). DB uses 357+ DELETE/UPDATE statements across 30+ packages. Append-only at event layer; mutable at storage layer. |
| FD-002 | EAV Over Relational | **DIVERGENT** | 39-table normalized SQL schema (`storage/schema.go`, ~600 LOC DDL). Each table has typed columns — opposite of EAV. |
| FD-003 | Datalog for Queries | **MISSING** | All queries are imperative SQL via `database/sql`. No Datalog parser, no stratified evaluation, no monotonicity analysis. |
| FD-004 | Datom Store Over Vector DB | **IMPLEMENTED** | CLI chose structured retrieval (BM25+LSI+PageRank in `search/`) over vector-similarity. LSI uses SVD but for relevance ranking, not embedding-based RAG. Decision is validated — CLI proves structured retrieval sufficient. |
| FD-005 | Per-Attribute Resolution | **DIVERGENT** | Global LWW hardcoded in `causal/dag.go:133–145`. No per-attribute resolution mode declaration. No lattice or multi-value support. |
| FD-006 | Self-Bootstrap | **PARTIAL** | CLI validates own spec (F(S)=1.0, 97/97 INVs witnessed). But spec elements are SQL rows, not datoms. Self-validation achieved; self-as-first-data not achieved. |
| FD-007 | Content-Addressable Identity | **DIVERGENT** | Sequential `INTEGER PRIMARY KEY` on all 39 tables. `ContentHash` exists in event model but is metadata, not identity key. Two agents asserting same fact get different IDs. |
| FD-008 | Schema-as-Data | **MISSING** | Schema is `const createTableSQL` in Go source (~600 LOC). Adding an attribute requires code edit + recompile. |
| FD-009 | Datom Replaces JSONL | **N/A** | Braid-specific: decision is about what replaces the CLI's JSONL. CLI currently IS the JSONL system being replaced. |
| FD-010 | Embedded Deployment | **IMPLEMENTED** | Single binary, embedded SQLite (`database/sql` + `go-sqlite3`), no daemon, no server. Exactly the deployment model specified. |
| FD-011 | Rust Implementation | **N/A** | Braid-specific language choice. CLI is Go; assessment not applicable. |
| FD-012 | Every Command = Transaction | **PARTIAL** | Write commands (crystallize, refine, absorb, witness, discover) emit events to JSONL streams. Read-only commands (query, search, validate, coverage) produce no events. ~60% of 45+ commands are event-producing; ~40% are stateless reads. |

---

### 11.2 Algebraic Structure (AS-001–AS-010)

| ADR | Decision | Status | Evidence |
|-----|----------|--------|----------|
| AS-001 | G-Set CvRDT Properties | **PARTIAL** | Events in JSONL are append-only (grow-only set analog). But `Merge()` in `causal/dag.go` breaks commutativity (`Merge(A,B) ≠ Merge(B,A)` — Section 5.5 M56 bug). Idempotence violated by sequential IDs. Approximates G-Set without formal guarantees. |
| AS-002 | Commitment Weight w(d) | **PARTIAL** | `impact/` package has BFS forward/backward traversal (380 LOC) — computes reach sets for any spec element. Does NOT compute `w(d) = |forward_cone|` as a scalar weight. Infrastructure for the computation exists; the specific formula is not implemented. |
| AS-003 | Branching G-Set Extension | **MISSING** | No concept of branches, isolated workspaces, or branch-level merge. CLI operates on a single linear store. |
| AS-004 | Branch Visibility Formula | **MISSING** | No branches → no visibility semantics. No snapshot isolation. |
| AS-005 | Branch as First-Class Entity | **MISSING** | No branch entities. No `:branch/ident`, `:branch/status`, or `:branch/competing-with` attributes. |
| AS-006 | Bilateral Branch Duality | **MISSING** | No branches → no DCC pattern. CLI's bilateral loop (discover→refine vs. absorb→drift) operates linearly, not via competing branches. |
| AS-007 | Hebbian Significance | **N/A** | Partially addressed in main analysis (Section 3.5). Search uses PageRank (static authority), not access-frequency-based significance. No access log. |
| AS-008 | Projection Reification | **MISSING** | No access logging of query patterns. No mechanism to promote frequently-used queries to first-class entities. LSI identifies latent topics but doesn't reify query patterns. |
| AS-009 | Diamond Lattice Signals | **MISSING** | No lattice types in the CLI. Challenge verdict has `confirmed`/`refuted` states but stored as SQL enums, not lattice values that join to produce signals. |
| AS-010 | Branch Comparison Entity | **MISSING** | No branches → no comparison entity type. |

---

### 11.3 Store & Runtime Architecture (SR-001–SR-011)

| ADR | Decision | Status | Evidence |
|-----|----------|--------|----------|
| SR-001 | Four Core Indexes (EAVT/AEVT/VAET/AVET) | **MISSING** | No EAV indexes. SQLite tables have standard B-tree indexes on primary keys and selected foreign keys. Query access patterns are column-based, not sorted-tuple-based. |
| SR-002 | LIVE Materialized Index | **PARTIAL** | `materialize/fold.go` performs deterministic fold of events → SQL state. The SQL state IS a materialized "current view." But it's not a LIVE index over datoms — it's a full relational state derived from events. Different architecture, similar purpose. |
| SR-003 | LMDB/redb for MVCC | **DIVERGENT** | Uses SQLite via `go-sqlite3`. SQLite provides serializable transactions but not the MVCC (multi-version concurrent readers) model specified. Adequate for single-agent but wrong architecture for concurrent multi-agent access. |
| SR-004 | HLC for Transaction IDs | **DIVERGENT** | Event IDs use `evt-{timestamp}-{random_8_bytes}` format. Contains physical timestamp but no logical counter. Not a Hybrid Logical Clock — random suffix provides uniqueness but no causal ordering within the same wall-clock moment. |
| SR-005 | Shell→SQLite→Rust Path | **PARTIAL** | CLI IS the SQLite phase (b). Successfully validates that SQLite-based approach works. Braid's Rust binary is phase (c). The three-phase implementation path is being followed — CLI proves phase (b) viable. |
| SR-006 | File-Backed Store + Git | **DIVERGENT** | Uses SQLite DB (`.ddis/index.db`) + JSONL event files. Not file-backed append-only datom files with git as temporal index. Different storage model — database + files vs. pure files + git. |
| SR-007 | Multi-Agent via Filesystem | **MISSING** | Single-agent only. No file-locking (`flock`), no concurrent branch files, no shared trunk. `state/` has session KV store but for single-agent use only. |
| SR-008 | 17 Axiomatic Meta-Attributes | **MISSING** | Schema is a Go constant, not a meta-schema of self-describing attributes. No `:db/ident`, `:db/valueType`, `:db/cardinality` or `:db/resolutionMode` attribute datoms. |
| SR-009 | Six-Layer Schema Architecture | **MISSING** | No layered schema. All 39 tables defined at one level in `schema.go`. No Layer 0–5 decomposition. |
| SR-010 | Twelve Named Lattices | **MISSING** | No lattice definitions. Status fields use SQL enums (strings), not algebraically-defined lattice types with join/meet operations. |
| SR-011 | Session State File | **PARTIAL** | `state/` package (75 LOC) provides session KV store via `session_state` table. No `.ddis/session/context.json` file for statusline integration. Session state exists but in wrong format and location for the designed coordination pattern. |

---

### 11.4 Protocol Decisions (PD-001–PD-006)

| ADR | Decision | Status | Evidence |
|-----|----------|--------|----------|
| PD-001 | Agent Working Set W_α | **MISSING** | No agent-local working sets. `state/` has flat KV store (~75 LOC) — no datom structure, no commit/fork semantics, no competitive branching. No concept of W_α ∪ S query scope. |
| PD-002 | Provenance Typing Lattice | **PARTIAL** | `event_provenance` table exists in schema but is never populated (H2 bug). Events carry `Causes` field for causal linking. No lattice-typed provenance (`:observed < :derived < :inferred < :hypothesized`). Infrastructure exists (table, field), implementation empty. |
| PD-003 | Crash-Recovery with Frontiers | **PARTIAL** | CLI is stateless between invocations — naturally crash-tolerant. `state/` provides session persistence. No frontier-based rebuild protocol, no delta delivery on reconnection. Crash-safe by accident (statelessness) rather than by design (frontier protocol). |
| PD-004 | At-Least-Once + Idempotent | **PARTIAL** | `InsertInvariant` uses `ON CONFLICT DO UPDATE` (idempotent upsert). Event IDs include random bytes (collision-resistant). But merge commutativity bug (M56) shows idempotence is not a systematic design principle — some operations are accidentally idempotent, others break it. |
| PD-005 | Topology-Agnostic Protocol | **MISSING** | Single-agent only. No multi-agent topology support. Protocol operations (parse, validate, etc.) implicitly assume single invoker. No SIGNAL, SUBSCRIBE, or frontier exchange. |
| PD-006 | Bilateral Authority Principle | **PARTIAL** | Bilateral workflow exists: forward (spec→impl via coverage, implorder, progress), backward (impl→spec via absorb, drift, refine). Authority is not emergent from contribution graphs — it's implicit in command invocation (whoever runs `ddis refine` has authority). The bilateral pattern is structurally present; formal authority derivation is absent. |

---

### 11.5 Protocol Operations (PO-001–PO-014)

| ADR | Decision | Status | Evidence |
|-----|----------|--------|----------|
| PO-001 | TRANSACT (7-field) | **PARTIAL** | Event append (`events/stream.go`) carries: ID, Type, Timestamp, SpecHash, Stream, Payload, Causes, Version. Missing: `agent` (no agent identity), `branch` (no branches), `provenance_type` (dead table), `rationale` (no reason-for-transaction field), `operation` keyword. 4/7 fields present; 3/7 missing. |
| PO-002 | ASSOCIATE (Schema Discovery) | **PARTIAL** | `search/context.go` (820+ LOC) assembles 9-signal context bundles. Returns entities AND values (not schema-only neighborhood). No SemanticCue/ExplicitSeeds mode distinction. No depth × breadth bounding. Context assembly exists; ASSOCIATE protocol shape does not. |
| PO-003 | ASSEMBLE (Rate-Distortion) | **PARTIAL** | `autoprompt/budget.go` computes `TokenTarget(depth)` (2000→300 token compression). `search/context.go` assembles with budget awareness. Missing: pyramid-level selection (π₀–π₃), priority formula (`α×relevance + β×significance + γ×recency`), intention pinning, freshness-mode. Budget-aware assembly exists; formal rate-distortion construction does not. |
| PO-004 | SIGNAL as Datoms | **MISSING** | No signal system. Drift, contradiction, and confusion detection exist in scattered modules but produce reports, not signal datoms. No unified `INV-SIGNAL-DATOM-001` recording. |
| PO-005 | Confusion Signal | **MISSING** | No confusion signal type. No automatic re-ASSOCIATE + re-ASSEMBLE retry cycle. When CLI commands produce confusing output, the agent must manually reformulate — exactly the pathology this decision addresses. |
| PO-006 | MERGE Cascade | **DIVERGENT** | `causal/dag.go:133–145` implements LWW merge that DELETES losing events. Destructive — violates append-only. No cascade: after merge, no conflict detection triggered, no cache invalidation, no uncertainty recomputation, no subscription firing. Merge exists but with fundamentally wrong semantics. |
| PO-007 | BRANCH (6 Sub-Ops) | **MISSING** | No branch operations. No fork, commit, combine, rebase, abandon, or compare. Section 4.15 documents this as a three-way gap (SEED.md, CLI, and gap analysis). |
| PO-008 | SUBSCRIBE (Push Notifications) | **MISSING** | No subscription mechanism. No pattern filters, no callbacks, no debounce. CLI is pull-only (agent invokes commands). |
| PO-009 | GUIDANCE (Action Topology) | **PARTIAL** | CommandResult triple `(output, state, guidance)` implemented in `autoprompt/`. "Next: ddis <command>" postscripts provide next-action guidance. No queryable guidance graph, no lookahead, no intention-alignment scoring, no learned guidance ranking. Injection exists; topology does not. |
| PO-010 | SYNC-BARRIER | **MISSING** | No sync barriers. Single-agent — no need for frontier exchange. |
| PO-011 | Agent Cycle (10-Step) | **PARTIAL** | Individual components exist: search (~ASSOCIATE), query (~QUERY), context (~ASSEMBLE), guidance (~GUIDANCE), transact (~action). But they are independent commands, not a composed 10-step cycle. No confusion-retry loop, no learned association recording, no subtask management, no signal checking. Components scattered; composition absent. |
| PO-012 | Genesis Transaction | **MISSING** | No genesis transaction. Database created by `CREATE TABLE` DDL. No tx=0 with meta-schema attributes. No constant hash verification of initial state. |
| PO-013 | QUERY (4 Invariants) | **DIVERGENT** | All queries are SQL. No `INV-QUERY-CALM-001` (no monotonicity concept). No `INV-QUERY-BRANCH-001` (no branches). No `INV-QUERY-SIGNIFICANCE-001` (no access events). `INV-QUERY-DETERMINISM-001` holds trivially (SQL at same DB state is deterministic). Query capability exists but 3/4 invariants are structurally impossible under SQL. |
| PO-014 | GENERATE-CLAUDE-MD | **N/A** | Dynamic CLAUDE.md is a Braid-specific operation. CLI has static CLAUDE.md only. Assessed as MISSING in Section 4.7. |

---

### 11.6 Snapshot & Query (SQ-001–SQ-010)

| ADR | Decision | Status | Evidence |
|-----|----------|--------|----------|
| SQ-001 | Local Frontier Default | **PARTIAL** | CLI queries operate on current DB state (analogous to "latest frontier" for single agent). No frontier concept or alternative query modes. Works by default for single-agent; structurally unprepared for multi-agent. |
| SQ-002 | Frontier as Datom Attribute | **MISSING** | No `:tx/frontier` attribute. `progress/` has work frontier (done/ready/blocked partitioning) — different concept (work-readiness vs. knowledge frontier). No `Map<AgentId, TxId>` vector clock. |
| SQ-003 | Datalog Frontier Extension | **MISSING** | No Datalog → no `[:frontier ?ref]` extension. |
| SQ-004 | Stratum Safety Classification | **MISSING** | No stratum classification. No `QueryMode` type. All queries executed uniformly as SQL — no monotonic/non-monotonic distinction. |
| SQ-005 | Topology-Agnostic Resolution | **PARTIAL** | Query results are deterministic at same DB state (trivially satisfied for single-agent). Merge commutativity bug (Section 5.5 M56) would violate this in multi-agent settings. Single-agent compliant; multi-agent unsafe. |
| SQ-006 | Bilateral Query Layer | **PARTIAL** | Forward queries exist: `coverage/` (spec→impl completeness), `implorder/` (spec→impl ordering), `progress/` (spec→impl readiness). Backward queries exist: `absorb/` (impl→spec reconciliation), `drift/` (impl↔spec divergence). Bridge: `triage/fitness.go` (bidirectional health). All use SQL, not Datalog. Bilateral structure implicit; not formalized. |
| SQ-007 | Projection Pyramid | **PARTIAL** | `autoprompt/budget.go` computes `TokenTarget(depth)` as budget-driven output sizing. `search/context.go` assembles with priority. No formal π₀–π₃ levels. No `INV-ASSEMBLE-PYRAMID-001`. Budget-driven compression exists; structured pyramid does not. |
| SQ-008 | Protocol Type Definitions | **MISSING** | No formal value sum type. Go structs used for each domain (models.go). No `Value` algebraic type with 14 variants. No `Level` or `Signal` sum types. |
| SQ-009 | Six-Stratum Query Classification | **MISSING** | No classification of queries by stratum. All queries are ad-hoc SQL. No mapping of 17 named query patterns to safety levels. |
| SQ-010 | Datalog/Imperative Boundary | **PARTIAL** | SVD used in `search/` for LSI (semantic search) — demonstrates the algorithm exists in the codebase. BFS traversal in `impact/` for reach computation. Both are "imperative functions that would be foreign-functions in a Datalog engine." The algorithms exist but applied to search/impact, not to uncertainty/authority computation. |

---

### 11.7 Uncertainty & Authority (UA-001–UA-012)

| ADR | Decision | Status | Evidence |
|-----|----------|--------|----------|
| UA-001 | Uncertainty Tensor (σ_e, σ_a, σ_c) | **MISSING** | No uncertainty computation anywhere. No epistemic, aleatory, or consequential uncertainty measures. No scalar combination formula. |
| UA-002 | Epistemic Temporal Decay | **PARTIAL** | Binary witness staleness: `WarnIfStale()` in storage compares file mtime vs. parse timestamp. Witness lifecycle has valid/stale/revoked states. But this is discrete (binary stale/fresh), not continuous exponential decay. No per-namespace lambda calibration. |
| UA-003 | Spectral Authority via SVD | **PARTIAL** | SVD exists in `search/lsi.go` for latent semantic indexing (document-term matrix). Algorithm is identical to what authority computation requires (agent-entity matrix). Applied to search relevance, not agent authority. Same math, different domain — could be adapted. |
| UA-004 | Delegation Threshold | **MISSING** | No delegation mechanism. `triage/` has betweenness centrality and in-degree computation (for issue prioritization), but no delegation threshold formula. Graph metrics exist; delegation classification does not. |
| UA-005 | Four-Class Delegation | **MISSING** | No delegation classification. Single-agent system — no self-handle/consult/delegate/escalate distinction. |
| UA-006 | Uncertainty Markers in Specs | **MISSING** | Parser extracts invariants, ADRs, sections, gates, glossary, negative specs, and cross-references. No uncertainty marker extraction. Confidence levels in spec text are not parsed or stored as structured data. |
| UA-007 | Observation Staleness Model | **PARTIAL** | `WarnIfStale()` compares file mtime to DB parse timestamp (basic staleness). No observation entity schema (`:observation/entity`, `:observation/source`, `:observation/stale-after`). No freshness-mode (`:warn | :refresh | :accept`). Basic concept present; formal model absent. |
| UA-008 | Self-Referential Exclusion | **MISSING** | No uncertainty computation → no self-referential measurement issue. |
| UA-009 | Query Stability Score | **MISSING** | No stability computation. No `w(d)` commitment weight → no `stability(R) = min{w(d)}`. |
| UA-010 | Contribution Weight | **MISSING** | No contribution weighting by verification status. Witness system has valid/stale/revoked states but these don't feed into an authority computation. |
| UA-011 | Delegation Safety | **MISSING** | No delegation → no safety invariant. |
| UA-012 | Resolution Capacity Monotonicity | **MISSING** | No resolution capacity computation. No multi-agent resolver sets. |

---

### 11.8 Conflict & Resolution (CR-001–CR-007)

| ADR | Decision | Status | Evidence |
|-----|----------|--------|----------|
| CR-001 | Conservative Detection (Superset) | **PARTIAL** | `consistency/` (3,004 LOC) detects contradictions between spec elements using 6 tiers (graph, SAT, heuristic, semantic, SMT, LLM). This is TEXT-LEVEL contradiction detection (invariant statements compared). Not DATOM-CONCURRENCY-LEVEL conflict detection (same entity, same attribute, different values, causally independent). Different concept — contradiction ≠ conflict. But the conservative principle (flag uncertain) is followed. |
| CR-002 | Three-Tier Routing | **MISSING** | All contradictions reported at same level. No severity-based routing. No automatic/agent/human tier classification. `ddis contradict` outputs a flat list with confidence scores — no routing decision. |
| CR-003 | Conflict as Datom Cascade | **MISSING** | Contradictions stored in `contradictions` table as row-level reports. No cascade: no severity computation → no routing → no TUI notification → no uncertainty update → no cache invalidation. Each step would need to produce datoms; current system produces SQL rows. |
| CR-004 | Deliberation Entity Types | **MISSING** | No deliberation, position, or decision entities. ADRs exist as spec elements (design decisions) but not as runtime conflict resolution records. No stance enum, no decision method enum, no `INV-DELIBERATION-BILATERAL-001`. |
| CR-005 | Crystallization Stability Guard | **MISSING** | No `:stability-min` guard. `ddis crystallize` converts discovery findings to spec elements without stability prerequisites. No confidence threshold, no coherence check, no conflict-free precondition. |
| CR-006 | Formal Conflict Predicate | **MISSING** | No formal predicate. Consistency engine uses text-level heuristics (polarity inversion, quantifier conflict, numeric bounds). No causal-independence test. No entity/attribute/value/cardinality conflict model. |
| CR-007 | Precedent Query | **MISSING** | No deliberation history → no precedent query. No Datalog pattern for finding similar past decisions. |

---

### 11.9 Agent Architecture (AA-001–AA-007)

| ADR | Decision | Status | Evidence |
|-----|----------|--------|----------|
| AA-001 | Dual-Process Protocol-Level | **PARTIAL** | Functional pipeline exists: `search/` (associative retrieval, S1) → `context/` (assembly) → LLM invocation (reasoning, S2). But this is implicit in the command sequence, not a protocol-level requirement. No `assemble ∘ query ∘ associate` composition. Agent happens to use S1→S2; protocol doesn't require it. |
| AA-002 | D-Centric Formalism | **PARTIAL** | CLI is DB-centric — all commands read/write SQLite via `storage/`. Structurally similar to "all operations reference store D." But store is SQL not datom, and operations are Go functions not protocol operations. Similar architecture, wrong substrate. |
| AA-003 | Three-Layer Architecture | **PARTIAL** | Implicit layers: (1) Protocol — bilateral commands (parse, validate, crystallize, absorb, etc.); (2) Observation — `annotate/scan.go` reads code → produces structured data; (3) Execution — shell tools, file operations. Not formalized as three layers. No observation functor with idempotent/monotonic/lossy properties. |
| AA-004 | Metacognitive Layer (4 Entity Types) | **MISSING** | No belief, intention, learned-association, or strategic-heuristic entities. No `INV-ASSOCIATE-LEARNED-001`. Cognitive mode classification exists in `discover/` (7 modes) but is a session-level tag, not a first-class entity. |
| AA-005 | Intention Anchoring | **MISSING** | No intention tracking. No `INV-ASSEMBLE-INTENTION-001`. Budget-constrained output exists but doesn't distinguish "intention" from other content for priority pinning. |
| AA-006 | Ten Named Operations | **N/A** | Assessed in main analysis (Sections 3.1–3.16). CLI has analogous commands for most operations but not formalized as protocol. |
| AA-007 | S1/S2 Diagnosis | **MISSING** | No mechanism to diagnose generic/hedging output as retrieval failure. No feedback loop from output quality to ASSOCIATE configuration. |

---

### 11.10 Interface & Budget (IB-001–IB-012)

| ADR | Decision | Status | Evidence |
|-----|----------|--------|----------|
| IB-001 | Five Layers + Layer 4.5 | **PARTIAL** | Layer 1 (CLI) = mature (45+ commands). Layer 0 (Ambient) = static CLAUDE.md (not dynamic). Layer 2 (MCP) = MISSING. Layer 3 (Guidance) = partial ("Next:" postscripts). Layer 4 (TUI) = MISSING. Layer 4.5 (Statusline) = MISSING. Only 1.5 of 6 layers implemented. |
| IB-002 | CLI Three Output Modes | **PARTIAL** | JSON mode exists (`--json` flag on most commands). Human TTY mode is default. No formal "agent mode" with 100–300 token budget, headline+entities+signals+guidance structure, or "demonstration not constraint" formatting principle. 2 of 3 modes, but agent mode — the most important for Braid — is absent. |
| IB-003 | MCP as Thin Wrapper (9 Tools) | **MISSING** | No MCP server. Recommendation document designs 11-tool surface but nothing is implemented. |
| IB-004 | Budget Five-Level Precedence | **PARTIAL** | `autoprompt/budget.go` computes `TokenTarget(depth)` as budget. No `--budget` flag. No `--context-used` flag. No session state file reading. No transcript tail-parse. Only the heuristic computation exists — 1 of 5 levels, and it's the fallback. |
| IB-005 | k* Measured Context | **PARTIAL** | Depth-dependent budget formula exists: `BaseBudget=12, Step=5, Floor=3`. Computes `TokenTarget(depth)` with study-derived constants. However, this is HEURISTIC decay (approximation), not MEASURED consumption from Claude Code's `context_window.used_percentage`. No runtime context integration. Wave 6 verification confirmed: measured depth-dependent decay exists, but the critical runtime-measured approach is absent. |
| IB-006 | k*-Parameterized Guidance Compression | **IMPLEMENTED** | Guidance attenuation by depth exists in `autoprompt/`. Longer conversations get shorter guidance. Compression thresholds present — maps to the specified >0.7/0.4–0.7/≤0.4/≤0.2 tiers. |
| IB-007 | Command Taxonomy by Attention | **PARTIAL** | Commands have varying output verbosity. JSON mode produces full output; human mode is shorter. But no formal CHEAP/MODERATE/EXPENSIVE/META classification. No per-command attention budget annotation. |
| IB-008 | TUI as Subscription Push | **MISSING** | No TUI. No SUBSCRIBE mechanism. |
| IB-009 | Human Signal Injection via TUI | **MISSING** | No TUI → no signal injection path. No MCP notification queue. |
| IB-010 | Store-Mediated Trajectory | **N/A** | Harvest/seed assessed as MISSING in Section 4.5–4.6. The five-part seed template is a Braid design. |
| IB-011 | Rate-Distortion Interface | **MISSING** | No formal rate-distortion framing. Budget-aware output exists (`TokenTarget`) but not designed as information-value maximization under attention-cost constraints. |
| IB-012 | Proactive Harvest Warning | **MISSING** | No harvest mechanism → no Q(t) monitoring → no proactive warnings. `autoprompt/` computes budget but doesn't emit harvest warnings when budget depletes. |

---

### 11.11 Guidance System (GU-001–GU-008)

| ADR | Decision | Status | Evidence |
|-----|----------|--------|----------|
| GU-001 | Guidance Comonad | **MISSING** | No guidance graph or comonadic structure. "Next: ddis <command>" postscripts are static strings embedded in CLI code, not queryable entities with effectiveness tracking. No `INV-GUIDANCE-EVOLUTION-001` (retraction threshold). |
| GU-002 | Guidance Lookahead | **MISSING** | No branch simulation. No lookahead computation. Guidance is reactive ("after this command, try that command") not proactive ("if you do X, then Y becomes possible in 3 steps"). |
| GU-003 | Spec-Language Phrasing | **MISSING** | "Next: ddis refine" is instruction-language (step), not spec-language (invariant reference). Violates `INV-GUIDANCE-SEED-001`. Guidance never references active invariants or formal structures. |
| GU-004 | Dynamic CLAUDE.md | **MISSING** | CLI has static CLAUDE.md. No drift-pattern-based generation. Assessed in Section 4.7. |
| GU-005 | Four-Component Injection | **PARTIAL** | (a) Names specific command — YES ("Next: ddis <command>"). (b) References active invariants — NO. (c) Notes uncommitted observations — NO. (d) Warns if drifting — NO. Only 1 of 4 required components present. |
| GU-006 | Basin Competition Model | **MISSING** | No basin analysis. No measurement of methodology drift. No attractor dynamics model. The problem this decision addresses (agent falling back to pretrained coding patterns) is observable but unmeasured. |
| GU-007 | Six Anti-Drift Mechanisms | **PARTIAL** | Of 6 mechanisms: (1) Guidance Pre-emption (CLAUDE.md rules) — absent. (2) Guidance Injection (tool response footer) — PARTIAL (names command, missing 3/4 components). (3) Drift Detection (access log analysis) — `drift/` detects spec-impl drift, not behavioral drift. (4) Pre-Implementation Gate (`ddis pre-check`) — absent. (5) Statusline Drift Alarm — absent. (6) Harvest Safety Net — absent. Score: ~1.5 of 6 mechanisms present. |
| GU-008 | Guidance-Intention Coherence | **MISSING** | No intention tracking → no alignment scoring. No `INV-GUIDANCE-ALIGNMENT-001`. |

---

### 11.12 Lifecycle & Methodology (LM-001–LM-016)

| ADR | Decision | Status | Evidence |
|-----|----------|--------|----------|
| LM-001 | Braid Is New Implementation | **N/A** | Braid-specific decision about itself. Not assessable against Go CLI. |
| LM-002 | Manual Harvest/Seed Before Tools | **N/A** | Methodology decision for Braid's specification phase. The Braid project practices this via HARVEST.md. |
| LM-003 | Conversations Disposable | **IMPLEMENTED** | CLI is stateless between invocations. Each invocation starts fresh from the database. No conversation persistence. Validates the principle that durable state lives in the store, not the conversation. |
| LM-004 | Reconciliation as Taxonomy | **IMPLEMENTED** | Eight reconciliation mechanisms present in various modules: drift (structural), absorb (epistemic→structural bridge), refine (quality→structural), contradict (logical), discover (epistemic). Not all eight types covered but the taxonomy principle — detect→classify→resolve — is the architectural pattern across multiple packages. |
| LM-005 | Semi-Automated Harvest | **MISSING** | No harvest. No transaction analysis to propose harvest candidates. Discovery threads park/merge state but don't harvest. |
| LM-006 | Harvest Calibration (FP/FN) | **MISSING** | No harvest → no FP/FN tracking. No uncommitted-count metric per session. |
| LM-007 | Datom-Exclusive Information | **DIVERGENT** | Information lives in SQLite tables (mutable), JSONL event files (append-only), and markdown files (source). Three storage substrates — violates "all durable information as datoms." |
| LM-008 | Self-Bootstrap Fixed-Point | **IMPLEMENTED** | F(S) = 1.0 achieved (commit `87c02d0`). CLI validates own specification, catching contradictions in its own spec during development. The specification process DID generate test cases (97 INVs witnessed). Fixed-point property demonstrated. |
| LM-009 | Specs Use DDIS Structure | **IMPLEMENTED** | CLI spec (`ddis-cli-spec/`) uses invariants (97), ADRs (74), negative cases, and (partially) uncertainty markers. DDIS structure actively used for specification. |
| LM-010 | Explicit Residual Divergence | **MISSING** | No mechanism to record unresolvable divergence with uncertainty markers. Contradictions are found (consistency engine) or ignored — no explicit "we know this diverges and accept it" recording. |
| LM-011 | 20–30 Turn Cycle | **N/A** | Agent lifecycle design. The CLI doesn't enforce turn counting. Assessed under IB-005/IB-012. |
| LM-012 | Harvest Delegation Topology | **MISSING** | No harvest → no delegation topology. No multi-agent harvest review. |
| LM-013 | Harvest Entity Types | **MISSING** | No harvest session or harvest candidate entities. |
| LM-014 | DDR as Feedback Loop | **PARTIAL** | RALPH loop (`refine/`) implements observe→plan→apply→judge feedback pattern. Not formalized as DDR entity types (Observation, Impact, Options, Decision, Spec Update). Pattern exists; entity formalization absent. |
| LM-015 | Staged Alignment Strategy | **IMPLEMENTED** | The four-strategy preference (thin wrapper → surgical edit → parallel implementation → rewrite) is implicitly validated by the CLI's own evolution: 6-phase remediation used surgical edits; event-sourcing was parallel implementation; Braid is the rewrite. |
| LM-016 | Seed Document Structure | **PARTIAL** | `SEED.md` exists with the 11-section structure. The decision is validated by the document's existence. Assessed as methodology rather than CLI feature. |

---

### 11.13 Coherence & Reconciliation (CO-001–CO-014)

| ADR | Decision | Status | Evidence |
|-----|----------|--------|----------|
| CO-001 | Coherence Is Fundamental Problem | **IMPLEMENTED** | CLI's entire architecture is organized around coherence verification: parse→validate→drift→contradict→witness→challenge pipeline. Not explicitly framed as "coherence not memory" but the implementation embodies the principle. |
| CO-002 | Four-Type Divergence (Original) | **IMPLEMENTED** | Precursor to CO-003. CLI addresses epistemic (discover), structural (drift), consequential (triage), aleatory (contradict). Expanded to eight types in CO-003. |
| CO-003 | Eight-Type Reconciliation Taxonomy | **PARTIAL** | Of 8 types: Epistemic (discover — partial), Structural (drift — implemented), Consequential (triage/fitness — partial), Aleatory (consistency — implemented), Logical (contradict — implemented), Axiological (fitness function — partial), Temporal (MISSING — no frontiers), Procedural (process compliance — partial). ~5 of 8 types partially or fully addressed; 3 absent. See Section 7.3 for detailed breakdown. |
| CO-004 | Bilateral Loop Convergence | **PARTIAL** | Bilateral mechanisms exist: witness/challenge (forward), absorb/refine (backward), drift (bridge). Monotonicity guard in refine/judge.go (drift increase → halt). But no formal convergence proof. Bilateral workflow operates; convergence not guaranteed. |
| CO-005 | Formalism as Divergence Mapping | **PARTIAL** | Invariants detect logical divergence (consistency engine checks). ADRs document decisions (prevent axiological re-litigation). Negative cases exist in specs. Mapping is implicit — not explicitly connected to divergence types. Elements present; connection unstated. |
| CO-006 | Structural vs. Procedural Coherence | **PARTIAL** | CLI enforces structural properties: 20 mechanical validation checks, deterministic fold, content-hash verification. Process compliance (`process/`) scores behavioral adherence. Both structural AND procedural enforcement present. Design principle (prefer structural) not explicitly codified. |
| CO-007 | Four Taxonomy Gaps | **MISSING** | Gap solutions not implemented: (1) No intent validation sessions (CO-012), (2) No test-results-as-datoms (CO-011), (3) No cross-project coherence (CO-013), (4) No observation temporal decay (UA-002). All four gaps remain open. |
| CO-008 | Five-Point Coherence Statement | **PARTIAL** | (1) Does spec contradict itself? — YES (`ddis contradict`). (2) Does impl match spec? — YES (`ddis drift`, `ddis scan`). (3) Does spec match intent? — NO (no intent validation). (4) Do agents agree? — NO (single agent). (5) Is methodology followed? — YES (`process/compliance.go`). Score: 3 of 5. |
| CO-009 | Fitness Function (7 Components) | **IMPLEMENTED** | `triage/fitness.go`: F(S) = weighted sum of 6 signals (validation 0.20, coverage 0.20, drift 0.20, challenge 0.15, contradictions 0.15, backlog 0.10). SEED.md §3 cites 7 components with different weights; CLI uses 6. Close but not identical. Functionally equivalent — achieved F(S) = 1.0. |
| CO-010 | Four-Boundary Chain | **PARTIAL** | Intent→Spec boundary: no detection. Spec→Impl boundary: `drift/` + `annotate/scan`. Impl→Behavior boundary: Go test suite. All four boundaries exist conceptually; only 2 of 3 inter-boundary detectors implemented. |
| CO-011 | Test Results as Datoms | **MISSING** | Tests run via `go test`. Results are stdout/exit-code, not datoms. No mechanism to record "test X passed at frontier F" as a structured fact in the store. |
| CO-012 | Intent Validation Sessions | **MISSING** | No structured review mechanism. No "Does this still describe what I want?" assembly. |
| CO-013 | Cross-Project Coherence | **MISSING** | Single-project only. Parent-spec resolution enables cross-spec references but not cross-project coherence verification. |
| CO-014 | Extensible Reconciliation Architecture | **MISSING** | Reconciliation mechanisms are hard-coded Go packages. Adding a new divergence type requires writing a new Go package, not adding query patterns to a data-driven system. |

---

### 11.14 Aggregate Findings

**What the ADR-level analysis reveals that the module-level analysis doesn't:**

1. **The substrate gap is deeper than "different storage."** At the module level, 8 modules are ALIGNED. But the ADR analysis shows that only 12/139 design decisions are fully implemented. The aligned modules implement correct logic on the wrong substrate — their algorithms port, but their data access patterns must be rewritten.

2. **Four subsystems are entirely absent:**
   - **Uncertainty & Authority** (0/12 implemented): No uncertainty tensor, no spectral authority, no delegation, no stability scores. The CLI validates correctness but has no mechanism for reasoning about confidence or distributing authority.
   - **Conflict & Resolution** (0/7 implemented): Text-level contradiction detection exists (and is strong), but runtime conflict detection, routing, deliberation, and precedent querying are absent. The CLI finds contradictions in specs but has no model for concurrent conflicts.
   - **Agent Architecture** (0/7 implemented): The CLI is used by agents but doesn't enforce or support a protocol-level agent architecture. No metacognition, no intention tracking, no S1/S2 diagnosis.
   - **Guidance System** (0/8 implemented): Guidance injection exists in rudimentary form ("Next: ddis refine") but the comonadic topology, spec-language phrasing, basin competition model, and six anti-drift mechanisms are absent.

3. **The branching system is the largest connected gap.** Seven decisions across three categories depend on branches: AS-003 through AS-006, AS-010 (Algebraic Structure), PO-007 (Protocol Operations), and PD-001 (Protocol Decisions). Branches enable: competing implementations, isolated agent workspaces, guidance lookahead via simulation, and bilateral spec ideation. This subsystem has ZERO implementation in the CLI and must be designed into Braid's Stage 0 store architecture to avoid costly retrofits.

4. **The gap between "concept validated" and "design decision implemented" is systematic.** The CLI proves that bilateral reconciliation, self-bootstrap, fitness function convergence, and contradiction detection WORK. But the specific operational designs (protocol type signatures, invariant conditions, entity schemas, cascade semantics) from the Braid design sessions are not reflected. Sections 1–10 capture what concepts the CLI validates; Section 11 captures the specific designs that must be built fresh.

5. **The SR (Store & Runtime) category — 11 decisions about deployment, indexes, schema layers — represents critical Stage 0 architecture.** Four core indexes (EAVT/AEVT/VAET/AVET), LIVE materialized index, MVCC storage, HLC transaction IDs, axiomatic meta-schema, six-layer schema architecture, and twelve named lattices are all MISSING. These are not conceptual gaps — they are the physical architecture of the datom store.

---

### 11.15 Structural Observations

Three patterns explain the coverage distribution:

1. **Module-by-module analysis misses protocol-level decisions.** Decisions about protocol properties (topology-agnosticism PD-005, delivery semantics PD-004, crash-recovery PD-003), formal algebraic structure (G-Set CvRDT AS-001, commitment weight AS-002, diamond lattice AS-009), and agent architecture formalism (D-centric model AA-002, dual-process AA-001, metacognitive layer AA-004) have no natural "module" to map to. The ADR-level analysis (this section) catches them; the module-level analysis (Sections 3.1–3.16) does not.

2. **The CLI was built before the Braid design sessions.** Decisions from Transcripts 01–07 (algebraic foundations, protocol operations, uncertainty tensor, spectral authority, branching G-Set) postdate the CLI's architecture. The CLI validates DDIS concepts discovered during its own development; Braid decisions from the ideation sessions have never been implemented anywhere. The 66 MISSING decisions are not bugs in the CLI — they are forward specifications for Braid.

3. **Reusable components cluster in the PARTIAL category.** The 41 PARTIAL entries represent CLI code that implements the right concept incompletely — BFS reach computation without commitment weight, context assembly without pyramid levels, contradiction detection without routing. These are the highest-value code to study when implementing the complete Braid design: the algorithm exists, the data access must change, and the missing pieces are identified.

---

*This gap analysis was produced for the Braid project (`ddis-braid/`) as the bridge between the existing DDIS Go CLI and the staged implementation plan defined in SEED.md §10. Every finding traces to specific source code locations and SEED.md sections. The analysis used 24 parallel investigation agents across 6 waves, each reading actual Go source code, to ensure comprehensive coverage of the ~62,500-line codebase and all 139 design decisions in ADRS.md.*

*Addendum (2026-03-02): Section 4.15 (Agent Working Set / Patch Branches) was added to SEED.md after review revealed a settled design decision from Transcript 04 (PQ1, Option B confirmed) and Transcript 05 (`ddis_branch` tool spec) that had not yet been captured there, and was therefore missed in the initial analysis. SEED.md §4.15 now formalizes W_α with the PD-001 reference. This itself exemplifies the harvest gap that DDIS is designed to prevent — see FAILURE_MODES.md FM-001.*

*Addendum (2026-03-02): Section 11 expanded from coverage gap inventory to comprehensive ADR assessment. All 139 design decisions in ADRS.md now have explicit status assessments (IMPLEMENTED/PARTIAL/DIVERGENT/MISSING/N/A) with specific Go source code evidence. Analysis performed by Waves 4–6 (12 agents), cross-verified by dedicated verification agents.*
