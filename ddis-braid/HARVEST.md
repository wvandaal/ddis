# HARVEST.md — Session Log

> This file is the manual harvest/seed mechanism. Every session appends an entry.
> Read the latest entry at session start (your "seed"). Write a new entry at session end (your "harvest").
> When the datom store exists, this file becomes unnecessary — the harvest/seed cycle is automated.

---

## Session 001 — 2026-03-01/02 (Pre-Braid: Design Foundation)

**Platform**: Claude.ai (multi-session conversation)
**Duration**: ~7 design sessions across several hours

### What Was Accomplished

- Produced `SEED.md` — the 11-section foundational design document covering:
  - Divergence as the fundamental problem (not just AI memory)
  - Specification formalism (invariants, ADRs, negative cases, uncertainty markers)
  - Datom abstraction with 5 algebraic axioms
  - Harvest/seed lifecycle
  - Reconciliation taxonomy (8 divergence types mapped to detection/resolution mechanisms)
  - Self-improvement loop (graph densification, adaptive instructions, retrieval sharpening)
  - Interface principles (budget-aware output, guidance injection, five layers)
  - Staged roadmap (Stage 0–4)
  - Design rationale (7 "why" entries including self-bootstrap)

- Produced `CLAUDE.md` — LLM-optimized operating instructions for all braid sessions

- Produced `onboarding.md` — comprehensive guide to the existing DDIS Go CLI

- Established the self-bootstrap commitment: DDIS specifies itself using DDIS methodology

### Decisions Made

| Decision | Rationale |
|---|---|
| Braid is a new implementation, not a patch of ddis-cli | The specification has diverged enough from the existing Go implementation that adaptation would be more costly than rebuild on clean foundations |
| DDIS specifies itself | Integrity (can't spec coherence system incoherently), bootstrapping (spec elements are first data), validation (if DDIS can't spec DDIS, it can't spec anything) |
| Manual harvest/seed before tools exist | Methodology precedes tooling; tools automate established practice |
| Reconciliation mechanisms are a unified taxonomy | All protocol operations are instances of: detect divergence → classify → resolve to coherence |
| Uncertainty markers are first-class | Prevents aspirational prose from being implemented as axioms |

### Open Questions

1. **Implementation language**: SEED.md says "existing Rust implementation" but the current CLI is Go. Decision needed: Rust (as originally designed) or Go (for continuity)?
2. **Section 9 of SEED.md is incomplete**: Needs the codebase description filled in by Willem
3. **Datom serialization format**: Not yet specified. JSONL? Protobuf? Custom binary?
4. **SQLite vs. custom storage**: The existing CLI uses SQLite extensively. Does braid?
5. **Temporal decay of facts**: Discussed but not formalized. λ parameter per attribute namespace.

### Recommended Next Action

**Produce SPEC.md** — the DDIS-structured specification. Work through SEED.md section by section,
extracting every implicit claim into formal invariants, ADRs, and negative cases. This is Step 2
in the concrete roadmap (SEED.md §10). Estimated: 2–4 hours across multiple Claude Code sessions.

---

## Session 002 — 2026-03-02 (Gap Analysis + Failure Mode Discovery)

**Platform**: Claude Code (Opus 4.6)
**Duration**: ~2 hours across one continuous session (context compaction occurred mid-session)

### What Was Accomplished

1. **Produced `GAP_ANALYSIS.md`** (883 lines → 920+ lines after addendum)
   - Comprehensive analysis of the existing Go CLI (~62,500 LOC, 38 packages) against SEED.md §1–§11
   - Three waves of parallel investigation using 12 subagent deep-dives, each reading actual Go source
   - Central finding: **substrate divergence** — CLI uses relational (39-table SQLite) vs. SEED.md's datom store (EAV)
   - Categorized: 8 ALIGNED, 12 DIVERGENT, 6 EXTRA, 4 BROKEN, 15 MISSING (originally 14, +1 after FM-001)
   - Incorporated prior art: `GAP_ANALYSIS_2026-02-27.md`, `cleanroom-audit-2026-03-01.md`, `RECOMMENDATION_MCP_PROTOCOL_SERVER_2026-03-01.md`

2. **Frontier tracking provenance investigation**
   - User challenged: "where does frontier tracking come from?"
   - Traced to exact origin: `transcripts/01-datomic-rust-crdt-spec-foundation.md:328-337`
   - Claude introduced the concept as a consequence of multi-writer partial orders (antichains vs. total order points)
   - Three options presented (3A/3B/3C), user chose 3C at line 645
   - Formalized as Axiom A3, elaborated in Transcript 02 with `:tx/frontier` attribute (line 471) and Datalog query syntax `[:frontier ?frontier-ref]` (line 1004)

3. **Private datoms / working set discovery (FM-001)**
   - User asked about "private datoms" and "scratchpad state" — not in gap analysis
   - Found settled decision in `transcripts/04-datom-protocol-interface-design.md:210-240`
   - Option B (two-tier: W_α + S) recommended by Claude, confirmed by user at line 373
   - User extended with "patch branches" concept and query-driven significance
   - Full `ddis_branch` tool designed in `transcripts/05:849-861`
   - **None of this appears in SEED.md** — a harvest gap in the design session harvest

4. **Produced `FAILURE_MODES.md`** — Bootstrap failure mode registry
   - Designed as primary mechanism for recording, triaging, and resolving failure modes
   - 4 initial failure modes catalogued (FM-001 through FM-004)
   - Includes severity levels (S0–S3), divergence type classification, lifecycle states
   - Traces to SEED.md §6 reconciliation taxonomy

5. **Updated `GAP_ANALYSIS.md`** with §4.15 (Agent Working Set / Patch Branches)
   - New MISSING category item covering the W_α / patch branch design
   - Updated executive summary (14 → 15 MISSING capabilities)
   - Added addendum noting the transcript-sourced finding

### Decisions Made

| Decision | Rationale |
|---|---|
| Gap analysis anchored on SEED.md as primary source | SEED.md is the canonical seed document; transcripts are supporting rationale. But this created anchoring bias (FM-003) |
| FAILURE_MODES.md uses DDIS reconciliation taxonomy | Dog-fooding: the failure mode registry classifies divergence using the same taxonomy the system will implement |
| FM-004 rated S0 (Structural) | Missing design decisions in SEED.md is a structural divergence — everything downstream (SPEC.md, implementation) inherits the gap |
| W_α architecture is Stage 2 but must inform Stage 0 store design | The working set uses the same datom structure as the shared store; the store must be designed to support this from the start |

### Open Questions

1. **Transcript→SEED.md reconciliation**: How many additional confirmed decisions from Transcripts 01–07 are missing from SEED.md? FM-004 identifies 4 from Transcript 04 alone. A systematic audit is needed.
2. **SEED.md update scope**: Should SEED.md be updated now (before SPEC.md) or should the transcript audit feed directly into SPEC.md production? Risk: if SEED.md stays incomplete, SPEC.md inherits the gaps.
3. **Implementation language**: Still unresolved from Session 001. SEED.md references "Rust" (§9) but the existing codebase is Go. The gap analysis implicitly assumes Rust (per SEED.md).
4. **FM-003 resolution**: What should the standard methodology be for future gap analyses? "SEED.md + transcript audit" or "SEED.md only with transcript spot-checks"?

### Failure Modes Discovered

| ID | Severity | Description |
|----|----------|-------------|
| FM-001 | S1 | Harvest gap: W_α / patch branch design dropped from SEED.md |
| FM-002 | S1 | Frontier tracking attribution: analyst label, not design session term |
| FM-003 | S2 | Gap analysis anchoring bias: SEED.md-only methodology misses transcript-only decisions |
| FM-004 | S0 | SEED.md incomplete: at least 4 confirmed decisions from Transcript 04 not captured |

### Files Created/Modified

| File | Action | Lines |
|------|--------|-------|
| `GAP_ANALYSIS.md` | CREATED then MODIFIED | 920+ |
| `FAILURE_MODES.md` | CREATED | ~230 |
| `HARVEST.md` | MODIFIED (this entry) | +80 |
| `AGENTS.md` (via `CLAUDE.md` symlink) | MODIFIED | +30 (project structure, source docs, session lifecycle, checklist, transcript index) |
| `SEED.md` | MODIFIED | +20 (§4 Protocol-Level Design Decisions, §10 Failure Mode Registry) |

### Recommended Next Action

**Resolve FM-004 (S0): Transcript→SEED.md reconciliation.** This is the highest-severity open
failure mode. Walk all 7 transcripts systematically, extract every confirmed design decision,
and verify each appears in SEED.md. Update SEED.md with missing decisions. This must happen
before SPEC.md production — otherwise the specification will inherit the gaps.

Secondary: **Produce SPEC.md** (carried forward from Session 001). With SEED.md complete and
the gap analysis available, the SPEC.md production will be on a sound foundation.

---

## Session 003 — 2026-03-02 (SEED.md §9 Gap Analysis: Cleanroom Codebase Evaluation)

**Platform**: Claude Code (Opus 4.6)
**Duration**: ~1 hour, single continuous session

### What Was Accomplished

1. **Verified AGENTS.md document references** (pre-task sanity check)
   - Checked every file and directory referenced in the project structure section
   - All 7 transcript `.md` files, all 7 `.txt` files, all 3 reference docs, journal files, SEED.md sections, sibling directories — all present and matching descriptions
   - Found 2 minor discrepancies:
     - AGENTS.md `<h1>` says `# CLAUDE.md — ddis-braid` but canonical filename is `AGENTS.md` (ACFS symlink convention)
     - `references/AGENTIC_SYSTEMS_FORMAL_ANALYSIS.md` described as "Mar 2026" but content dated Feb 28, 2026
   - User fixed the date discrepancy in AGENTS.md during the session

2. **Performed comprehensive cleanroom gap analysis** of Go CLI against Braid SEED.md
   - Launched 4 parallel research agents (total ~12 minutes wall time):
     - **Agent 1** — Read all 8 documents in `../.ddis/specs/` (audit findings, remediation plan, MCP recommendation, ceremonial usage analysis, universality field report, prior gap analysis, next steps)
     - **Agent 2** — Full inventory of all 36 `internal/` packages in Go CLI: read actual Go source, catalogued key types/functions, LOC per package, DDIS concept mapping (144K tokens, 105 tool uses, ~6 min)
     - **Agent 3** — Full analysis of CLI spec (`ddis-cli-spec/`): manifest structure, 9 modules, all invariant/ADR counts, concept-by-concept mapping to Braid SEED.md
     - **Agent 4** — Meta-standard analysis (`ddis-modular/`), docs directory, DATOMIC_IN_RUST.md reference, HARVEST.md prior context

3. **Updated SEED.md §9** — Replaced 12-line placeholder with ~140-line comprehensive gap analysis
   - §9.1: Corrected factual record (Go not Rust, 62,500 LOC, 36 packages, 39-table SQLite, 112 INVs, 82 ADRs, fixpoint and audit timeline)
   - §9.2: Five-category classification with per-module assessments:
     - **8 ALIGNED**: contradiction detection (6-tier), guidance injection (state monad + k*), budget-aware output, bilateral loop (4 adjunction pairs), fitness function (Lyapunov), witness/challenge, validation engine (20 checks), search intelligence (BM25+LSI+PageRank+RRF)
     - **5 DIVERGENT**: storage (39-table SQL → datom store), event sourcing (3-stream JSONL fold → datom set), CRDT merge (causal DAG → set union), parser (SQL inserts → datom assertions), discovery threads (within-session JSONL → session-boundary harvest/seed)
     - **14 EXTRA**: triage, skeleton, task derivation, exemplar, process compliance, refinement, impact/cascade, coverage, impl ordering, diff, annotations, oplog, renderer, GitHub integration — each assigned a target stage
     - **7 BROKEN**: quality gate identity collapse, dead provenance, parser code-block vulnerability, non-atomic dual-write, applier field gaps, measurement hardcoding, bilateral lifecycle non-adoption (0%) — each mapped to Braid constraint that prevents it
     - **14 MISSING**: datom store, Datalog engine, schema-as-data, per-attribute resolution, harvest, seed, dynamic CLAUDE.md, agent frontiers, sync barriers, signal system, deliberation/decision, MCP interface, TUI, knowledge graph densification + adaptive retrieval — each assigned priority stage
   - §9.3: Five-step implementation strategy and critical risk identification (datom store + Datalog are load-bearing novelties)

### Decisions Made

| Decision | Rationale |
|---|---|
| Gap analysis written directly into SEED.md §9, not a separate file | SEED.md §9 was explicitly designed as the gap analysis location ("Fill in the codebase description in section 9"). Keeping it in-document ensures the seed is self-contained. |
| Corrected "Rust" to "Go" in §9.1 | The existing codebase is Go (~62,500 LOC). SEED.md §9 previously said "existing Rust implementation" which was factually wrong. (Resolves Session 001 open question #1 partially — the *existing* implementation is Go. The *target* implementation language for Braid remains unresolved.) |
| Central thesis: "substrate divergence" | The Go CLI has strong behavioral coverage (concepts) on a fundamentally different substrate (relational SQL + JSONL). This frames the implementation strategy: build the substrate first, then port the behavioral concepts. |
| Every BROKEN finding mapped to a Braid constraint | Demonstrates that the datom store design structurally prevents each Go CLI defect. This is not accidental — it validates that SEED.md constraints C1–C7 address real failure modes. |
| EXTRA modules assigned to specific stages | Prevents scope creep in Stage 0. Most EXTRA modules become trivial Datalog queries once the store exists, so deferral is low-risk. |
| Code annotation system (`internal/annotate/`) included in Stage 0 | Constraint C5 (traceability) is non-negotiable. The annotation grammar and scan logic are portable. Other EXTRA modules are deferred. |

### Key Findings

1. **The Go CLI spec has grown beyond the counts recorded in AGENTS.md and prior sessions**: 112 invariants (not 97) and 82 ADRs (not 74). The spec grew during the event-sourcing expansion (APP-INV-071–097) and the cleanroom audit remediation (APP-INV-098–112).

2. **The bilateral lifecycle non-adoption finding (0% across 2 external projects) is the strongest empirical validation of Braid's harvest/seed design.** The ceremonial-vs-loadbearing analysis explains *why* (information-theoretic redundancy of within-session tool consultation). Braid's session-boundary architecture (seed at start, harvest at end) is the correct response.

3. **The 39-table SQLite schema is a detailed requirements document** for the Braid datom store. Each table → entity types, each column → attributes. The DDL is not portable; the data model it describes is.

4. **The MCP protocol server recommendation** (`../.ddis/specs/RECOMMENDATION_MCP_PROTOCOL_SERVER_2026-03-01.md`) is essentially a subset of Braid's Stage 3. Its diagnosis — that the bilateral lifecycle fails because the write interface is human-optimized while the primary consumer is AI agents — is directly actionable for Braid's interface design.

5. **The cleanroom audit's 52 findings are a test suite for Braid's design.** Every HIGH/MEDIUM finding maps to a Braid constraint that would prevent it. If Braid's datom store works correctly, these defects are structurally impossible.

### Open Questions

1. **Implementation language for Braid**: Still unresolved. SEED.md now correctly describes the existing codebase as Go. The target language for Braid is undecided. `references/DATOMIC_IN_RUST.md` explores Rust; `transcripts/01` designs the datom store formally; the existing CLI is Go. (Carried from Session 001, partially addressed.)
2. **Relationship between SEED.md §9 and the standalone GAP_ANALYSIS.md from Session 002**: Both now contain gap analyses. SEED.md §9 is the canonical location (per SEED.md §10 step 4: "Gap analysis"). The standalone `GAP_ANALYSIS.md` has additional detail (920+ lines) and the FM-001 working set addendum. Reconciliation needed.
3. **FM-004 (S0) still unresolved**: Transcript→SEED.md reconciliation has not been performed. At least 4 confirmed design decisions from Transcript 04 are not captured in SEED.md. This is the highest-severity open failure mode from Session 002.
4. **Datom serialization format**: Still unresolved (carried from Session 001).
5. **SQLite as storage backend for datom store**: Still unresolved. SQLite could serve as the physical storage layer for datoms while Datalog provides the query layer. The Go CLI's extensive SQLite usage proves the technology works at this scale. (Carried from Session 001.)

### Files Modified

| File | Action | Details |
|------|--------|---------|
| `SEED.md` | MODIFIED | §9 replaced: 12-line placeholder → ~140-line comprehensive gap analysis (§9.1 codebase overview, §9.2 five-category classification, §9.3 implementation strategy) |
| `HARVEST.md` | MODIFIED | This entry appended |

### Recommended Next Action

**Resolve FM-004 (S0): Transcript→SEED.md reconciliation** (carried from Session 002). This remains the highest-severity open failure mode. The gap analysis in §9 is now complete, but if SEED.md sections 1–8 are missing confirmed design decisions from the transcripts, then the gap analysis itself is built on an incomplete foundation. Walk all 7 transcripts, extract confirmed decisions, verify each appears in SEED.md.

Secondary: **Produce SPEC.md** (carried from Sessions 001 and 002). With §9 now complete, the SPEC.md production has a sound foundation — but only if FM-004 is resolved first.

---

## Session 011 — 2026-03-02 (SPEC.md Wave 4: Integration Sections — Complete)

**Platform**: Claude Code (Opus 4.6)
**Duration**: ~45 minutes, single session (continuation of Session 010)

### What Was Accomplished

1. **Produced `SPEC.md` Wave 4 — Integration sections** (8,157 total lines)
   - **§15 Uncertainty Register** (10 entries): 2 explicit markers from Wave 3 (UNC-BILATERAL-001/002 — fitness function weights, boundary weights), plus 8 implicit uncertainties identified by systematic analysis: content-addressable collision rate, HLC clock skew tolerance, Datalog evaluation performance, harvest warning thresholds, basin competition crossover point, crystallization stability threshold, 17-attribute sufficiency, resolution mode ergonomics. Each entry includes confidence level, stage affected, impact analysis, resolution criteria, and what breaks if wrong.
   - **§16 Verification Plan**: Complete 104-row per-invariant verification matrix (14 namespace tables mapping each INV to primary/secondary V:TAG, tool, CI gate, and stage), 5-gate CI pipeline specification (compile→test→kani→model→miri), typestate encoding catalog (9 patterns), deductive verification candidates (5 INVs recommended for Verus/Creusot post-Stage 2), verification statistics summary.
   - **§17 Cross-Reference Index**: Namespace→SEED.md→ADRS.md mapping (14 rows), invariant dependency graph (key chains from STORE foundations through INTERFACE), dependency depth analysis (5 levels confirming implementation order), stage mapping (Stages 0–4 with INV counts and success criteria), hard constraint traceability (C1–C7 → specific INVs), failure mode traceability (FM-001–004 → specific INVs and ADRs).

2. **Updated appendices to final form**: Element count summary marked "(Complete)", verification statistics expanded with Stage 0 INV counts and uncertainty metrics, Stage 0 element catalog refined with corrected namespace coverage.

3. **SPEC.md is now complete**: All 17 sections (§0–§17) across 4 waves, 208 specification elements (104 INV + 63 ADR + 41 NEG), 10 uncertainty markers, 8,157 lines.

### Decisions Made

| Decision | Rationale |
|---|---|
| 10 uncertainty entries (2 explicit + 8 implicit) | Explicit markers from Waves 1–3 are supplemented by systematic analysis of assumptions that have not been validated by implementation. Focus on Stage 0 blockers. |
| 5-gate CI pipeline (compile→test→kani→model→miri) | Progressive verification: cheap gates run on every commit, expensive gates on PRs or nightly. Matches Rust formal verification ecosystem capabilities. |
| 5 deductive verification candidates deferred post-Stage 2 | CRDT laws and merge preservation are high-value targets for Verus/Creusot proofs, but the cost is only justified after implementation stabilizes. |
| Dependency graph confirms implementation order | Longest chain (depth 4) runs STORE→MERGE→SYNC→BILATERAL→GUIDANCE→INTERFACE, validating the Wave 1→2→3 production order matches implementation dependency order. |

### Open Questions

1. **SPEC.md modularization**: At 8,157 lines, the file significantly exceeds NEG-008 (no massive monolithic files). Splitting strategy needed before beginning IMPLEMENTATION_GUIDE.md.
2. **Three high-urgency uncertainties**: UNC-HARVEST-001 (warning thresholds), UNC-GUIDANCE-001 (basin crossover), UNC-SCHEMA-001 (17-attribute sufficiency) — all Stage 0 blockers that must be resolved during initial implementation.

### Failure Modes

No new failure modes discovered. The uncertainty register (§15) systematically identifies where the specification might be wrong — this is the FM-004 (cascading incompleteness) countermeasure applied to the specification itself.

### Files Created/Modified

| File | Action | Details |
|------|--------|---------|
| `SPEC.md` | MODIFIED | 7,445 → 8,157 lines, +3 integration sections (§15–§17), appendices finalized |
| `HARVEST.md` | MODIFIED | This entry appended |

### Recommended Next Action

**SPEC.md is complete.** Next priorities:
1. **IMPLEMENTATION_GUIDE.md** — Stage 0 deliverables with exact CLI command signatures, file formats, CLAUDE.md template, success criteria. The implementing agent's operating manual.
2. **SPEC.md modularization** — Split into per-namespace or per-wave files if the implementing agent needs more manageable units.

---

## Session 010 — 2026-03-02 (SPEC.md Wave 3: Intelligence Specification)

**Platform**: Claude Code (Opus 4.6)
**Duration**: ~1 hour, single session (continuation of Session 009)

### What Was Accomplished

1. **Produced `SPEC.md` Wave 3 — Intelligence namespaces** (7,445 total lines, 208 cumulative elements)
   - **§9 SIGNAL** (6 INV, 3 ADR, 3 NEG = 12 elements): Signal as typed divergence event, eight signal types mapping to reconciliation taxonomy, dispatch function, confusion→re-association pipeline, subscription completeness, severity-ordered routing (three-tier cascade), diamond lattice signal generation (AS-009), taxonomy completeness check
   - **§10 BILATERAL** (5 INV, 3 ADR, 2 NEG = 10 elements): Bilateral loop as adjunction (forward ⊣ backward), divergence measure over four-boundary chain, fitness function F(S) with seven components (CO-009), monotonic convergence property, five-point coherence statement (C1–C5), bilateral symmetry via same Datalog apparatus, residual documentation requirement, test results as datoms
   - **§11 DELIBERATION** (6 INV, 4 ADR, 3 NEG = 13 elements): Convergence to decided/stalled, crystallization stability guard (CR-005 — six conditions including stability_min=0.7), precedent queryability (case law system), bilateral deliberation symmetry, commitment weight integration (AS-002), competing branch resolution (winner committed, losers abandoned), three entity types, five decision methods, precedent as case law
   - **§12 GUIDANCE** (7 INV, 4 ADR, 3 NEG = 14 elements): Comonadic topology (GU-001), basin competition model P(Basin_A) vs P(Basin_B), six anti-drift mechanisms as energy injection, continuous injection (every response has footer), spec-language phrasing, intention-action coherence, drift detection responsiveness (5-command transact gap), learned guidance effectiveness tracking (pruned below 0.3), lookahead via branch simulation, dynamic CLAUDE.md improvement
   - **§13 BUDGET** (5 INV, 3 ADR, 2 NEG = 10 elements): k*_eff as monotonically decreasing resource, Q(t) formula with piecewise attention decay, five-level output precedence, projection pyramid (π₀–π₃), output budget as hard cap, precedence-ordered truncation, quality-adjusted degradation, guidance compression by budget, command attention profiles, rate-distortion framework
   - **§14 INTERFACE** (7 INV, 3 ADR, 3 NEG = 13 elements): Five layers plus Layer 4.5 statusline bridge, three CLI output modes (structured/agent/human), MCP as thin wrapper with nine tools, statusline zero-cost to agent, TUI subscription liveness, human signal injection, proactive harvest warning thresholds

2. **Updated appendices**: Element count summary (208 total), verification coverage matrix, Stage 0 element catalog — all updated to include Wave 3 data including new Stage 0 elements from GUIDANCE and INTERFACE namespaces.

3. **Cross-namespace consistency**: Wave 3 namespaces reference Wave 1 types (Datom, EntityId, Store, QueryExpr), Wave 2 mechanisms (harvest pipeline, seed assembly, merge cascade, sync barriers), and each other (SIGNAL↔BILATERAL, DELIBERATION↔SIGNAL, GUIDANCE→BUDGET, INTERFACE→GUIDANCE).

### Decisions Made

| Decision | Rationale |
|---|---|
| Eight signal types with surjective taxonomy mapping | Some divergence types (Temporal, Procedural) are detected by specialized mechanisms and surfaced through existing signal types. Bijection would force artificial types. |
| Fitness function weights as uncertainty | CO-009 weights (V=0.18, C=0.18, D=0.18, H=0.13, K=0.13, I=0.08, U=0.12) are theoretical. Marked UNC-BILATERAL-001 with confidence 0.6 pending empirical calibration. |
| Basin competition as central failure model | Understanding agent methodology drift as dynamical systems (two attractors) rather than memory problem is prerequisite to effective countermeasures. Six anti-drift mechanisms are energy injections. |
| Crystallization guard over immediate commit | Premature crystallization is S0-severity (silently wrong artifacts). Stability guard with six conditions directly addresses FM-004 (cascading incompleteness). |
| MCP as thin wrapper, CLI does all computation | Single-implementation principle: all logic lives in CLI binary, MCP only adds session state and notifications. Prevents duplication bugs. |

### Open Questions

1. **SPEC.md modularization**: At 7,445 lines with 14/14 namespaces done, the file now exceeds NEG-008 (no massive monolithic files). Must modularize before Wave 4 integration sections.
2. **Fitness function weight calibration**: UNC-BILATERAL-001 (confidence 0.6) and UNC-BILATERAL-002 (confidence 0.5) need empirical data from Stage 0 usage.
3. **Learned guidance effectiveness measurement**: The 0.3 threshold for pruning (INV-GUIDANCE-005) is theoretical. Needs calibration.
4. **Wave 4 integration**: §15 Uncertainty Register, §16 Verification Plan, §17 Cross-Reference Index remain.

### Failure Modes

No new failure modes discovered. Wave 3 namespaces directly address:
- FM-001 (knowledge loss) — GUIDANCE continuous injection prevents methodology drift that leads to unharvested work
- FM-002 (provenance fabrication) — SIGNAL routes provenance-typed events through three-tier cascade
- FM-003 (anchoring bias) — BILATERAL bilateral symmetry ensures both directions are checked with same apparatus
- FM-004 (cascading incompleteness) — DELIBERATION stability guard prevents premature crystallization

### Files Created/Modified

| File | Action | Details |
|------|--------|---------|
| `SPEC.md` | MODIFIED | 5,083 → 7,445 lines, +72 elements (36 INV, 20 ADR, 16 NEG), 6 new namespaces (SIGNAL, BILATERAL, DELIBERATION, GUIDANCE, BUDGET, INTERFACE) |
| `HARVEST.md` | MODIFIED | This entry appended |

### Recommended Next Action

1. **Plan SPEC.md modularization** — at 7,445 lines, splitting is necessary before adding Wave 4 integration sections
2. **Produce Wave 4** — §15 Uncertainty Register (collect all UNC-* markers), §16 Verification Plan (per-invariant verification matrix), §17 Cross-Reference Index (namespace→SEED→ADRS mappings)
3. **Begin IMPLEMENTATION_GUIDE.md** — Stage 0 deliverables with exact CLI command signatures, file formats, success criteria

---

## Session 008 — 2026-03-02 (SPEC.md Wave 1: Foundation Specification)

**Platform**: Claude Code (Opus 4.6)
**Duration**: ~1.5 hours, single session

### What Was Accomplished

1. **Produced `SPEC.md` Wave 1 — Foundation namespaces** (3,173 lines, 85 elements)
   - **§0 Preamble**: Scope, conventions (element ID format, three-level refinement, verification tags, traceability notation, stage assignment), namespace index (14 namespaces across 4 waves), hard constraints (C1–C7)
   - **§1 STORE** (14 INV, 12 ADR, 5 NEG = 31 elements): G-Set CvRDT algebra with 5 laws (L1–L5), typestate Transaction lifecycle, CRDT merge properties (commutativity/associativity/idempotency), genesis determinism, frontier durability, HLC monotonicity, LIVE index correctness, working set isolation, every-command-as-transaction
   - **§2 SCHEMA** (8 INV, 4 ADR, 3 NEG = 15 elements): Meta-schema recursion (17 axiomatic attributes as fixed point), genesis completeness, schema monotonicity, validation on transact, self-description, six-layer architecture, lattice definition completeness, diamond signal generation
   - **§3 QUERY** (11 INV, 8 ADR, 4 NEG = 23 elements): Datalog fixpoint semantics, CALM compliance, semi-naive evaluation, six-stratum classification, query determinism, significance tracking (access log separation), branch visibility, stratum safety, FFI boundary purity, bilateral symmetry, topology-agnostic results, projection reification
   - **§4 RESOLUTION** (8 INV, 5 ADR, 3 NEG = 16 elements): Per-attribute resolution as semilattice, conflict predicate (six-condition with causal independence), three-tier routing, conservative detection (no false negatives), LWW/lattice/multi commutativity, conflict entity datom trail
   - **Appendices**: Element count summary, verification coverage matrix, Stage 0 element catalog

2. **Methodology**: Three-level cleanroom refinement (Mills) applied throughout:
   - Level 0: Algebraic laws (mathematical objects, operations, proofs)
   - Level 1: State machine (state, transitions, pre/postconditions)
   - Level 2: Implementation contract (Rust types, typestate patterns, Kani annotations)

3. **Verification audit** of completed SPEC.md:
   - 85/85 elements have traceability (100%)
   - 41/41 INVs have falsification conditions (100%)
   - 41/41 INVs have V:PROP minimum verification (100%)
   - 22 INVs have V:KANI, 5 have V:MODEL, 9 have V:TYPE
   - 0 contradictions found across all 4 namespaces
   - Refinement chains verified: L1 preserves L0, L2 preserves L1

### Decisions Made

| Decision | Rationale |
|---|---|
| Cleanroom three-level refinement (Mills) | Bridges algebraic foundations to implementable Rust code. Each level is verified against the level above it. Level 0 = what, Level 1 = how (abstractly), Level 2 = how (concretely). |
| Verification matrix with 7 tags | V:TYPE/V:PROP/V:KANI/V:CONTRACT/V:MODEL/V:DEDUCTIVE/V:MIRI — minimum V:PROP for all, V:KANI for critical, V:MODEL for protocol. Matches Rust formal methods ecosystem. |
| STORE namespace fully specified to Level 2 | STORE is the load-bearing novelty. Full Rust types, typestate Transaction lifecycle, Kani annotations. Other namespaces have Level 2 where implementation contracts are clear. |
| QUERY/RESOLUTION Level 2 intentionally declarative | These namespaces specify engine behavior, not direct Rust code. Level 2 uses Datalog formalization and query engine API rather than low-level Rust. |
| Single monolithic SPEC.md for Wave 1 | All 4 foundation namespaces in one file preserves cross-namespace references (STORE types used by SCHEMA, QUERY, RESOLUTION). Will evaluate modularization as Wave 2–3 are added. |

### Key Design Elements Produced

1. **Typestate Transaction lifecycle** (INV-STORE-001..002, §1.3): Building → Committed → Applied enforced at compile time. Prevents applying uncommitted transactions (type error).
2. **CRDT laws as invariants** (INV-STORE-004..007): L1–L4 (commutativity, associativity, idempotency, monotonicity) with proptest and Kani verification strategies.
3. **Conflict predicate** (INV-RESOLUTION-004): Six conditions including causal independence — the critical distinction between "update" and "conflict."
4. **Conservative conflict detection** (INV-RESOLUTION-003): `conflicts(local) ⊇ conflicts(global)` — proven safe by monotonicity of causal-ancestor relation.
5. **CALM compliance** (INV-QUERY-001): Monotonic queries run without coordination; non-monotonic queries require frontier/barrier. Parse-time enforcement.
6. **Access log separation** (INV-QUERY-003, NEG-QUERY-004): Significance tracking in separate log, not main store. Prevents unbounded positive feedback loops.

### Open Questions

1. **Level 2 completeness for QUERY/RESOLUTION**: The verification audit notes 19/41 INVs have full Level 2 Rust contracts. The remaining 22 have Level 2 as "engine behavior" — should these be formalized further?
2. **Wave 2 dependencies on Wave 1**: HARVEST/SEED/MERGE/SYNC depend on STORE types and QUERY engine. Cross-namespace type references need careful management.
3. **Datom serialization format**: Still unresolved. Does not block SPEC.md but blocks implementation.
4. **SPEC.md modularization**: At 3,173 lines with only 4 of 14 namespaces, the file will exceed NEG-008 (no massive monolithic files) by Wave 3. Plan for splitting needed.

### Failure Modes

No new failure modes discovered. The cleanroom methodology (three-level refinement with verification tags) addresses FM-003 (anchoring bias) by forcing algebraic grounding before implementation detail.

### Files Created/Modified

| File | Action | Details |
|------|--------|---------|
| `SPEC.md` | CREATED | 3,173 lines, 85 elements (41 INV, 29 ADR, 15 NEG), 4 namespaces (STORE, SCHEMA, QUERY, RESOLUTION) |
| `HARVEST.md` | MODIFIED | This entry appended |

### Recommended Next Action

**Produce SPEC.md Wave 2 — Lifecycle namespaces** (HARVEST, SEED, MERGE, SYNC). These depend on Wave 1 definitions (STORE types, SCHEMA attributes, QUERY engine). Same three-level refinement methodology. Estimated: ~40 INV, ~20 ADR, ~10 NEG across 4 namespaces.

Secondary: **Plan SPEC.md modularization** before Wave 3 pushes the file past manageable size.

---

## Session 009 — 2026-03-02 (SPEC.md Wave 2: Lifecycle Specification)

**Platform**: Claude Code (Opus 4.6)
**Duration**: ~1 hour, single session (continuation of Session 008)

### What Was Accomplished

1. **Produced `SPEC.md` Wave 2 — Lifecycle namespaces** (5,083 total lines, 136 cumulative elements)
   - **§5 HARVEST** (8 INV, 4 ADR, 3 NEG = 15 elements): Epistemic gap algebra (set difference: agent_knowledge \ store), harvest pipeline (detect→propose→review→commit→record), proactive warning system at Q(t) thresholds, crystallization stability guard (no harvest during open deliberation), FP/FN calibration metrics (LM-006), bounded conversation lifecycle, delegation topologies (single-agent, review-agent, committee)
   - **§6 SEED** (6 INV, 3 ADR, 2 NEG = 11 elements): Seed as projection (assemble ∘ query ∘ associate), priority scoring formula (α×relevance + β×significance + γ×recency), dynamic CLAUDE.md generation (7-step process with 3-concern collapse), rate-distortion assembly (projection pyramid π₀–π₃), intention anchoring with task_context vector, budget-monotonic truncation
   - **§7 MERGE** (8 INV, 4 ADR, 3 NEG = 15 elements): Core set-union merge (L1–L5 from STORE), 5-step merge cascade (copy→detect→surface→record→update), branching G-Set extension with 5 properties (P1 inclusion through P5 growth preservation), 6 branch sub-operations, competing branch lock, working set isolation (W_α ∩ W_β = ∅), bilateral branch duality, at-least-once idempotent delivery
   - **§8 SYNC** (5 INV, 3 ADR, 2 NEG = 10 elements): Consistent cut algebra (intersection of frontiers), barrier protocol (initiate→exchange→resolve), topology-dependent implementation (P2P direct, hub-spoke via central), barrier timeout safety (no stuck agents), topology-independent results, barrier entity provenance trail

2. **Updated appendices**: Element count summary (136 total), verification coverage matrix, Stage 0 element catalog — all updated to include Wave 2 data.

3. **Cross-namespace consistency maintained**: Wave 2 namespaces reference STORE types (Datom, EntityId, TxId, Store), SCHEMA attributes (`:db/attr.*`), QUERY engine (frontier-scoped queries), and RESOLUTION modes — all defined in Wave 1.

### Decisions Made

| Decision | Rationale |
|---|---|
| Harvest as epistemic gap detection, not summarization | FM-001 shows ~47% miss rate with manual summarization. The set-difference formulation (agent_knowledge \ store) makes gaps structurally detectable rather than relying on agent recall. |
| Dynamic CLAUDE.md generation in SEED namespace | The three-concern collapse (ambient awareness + guidance + trajectory) is the concrete implementation of seed assembly. CLAUDE.md is the output artifact, not a separate system. |
| Branching G-Set as formal extension to STORE algebra | Working set isolation (PD-001) requires branch semantics. Extending the G-Set with (S, B, ⊑, commit, combine) preserves all 5 CRDT laws while adding branch operations. |
| Sync barriers as consistent cuts | Consistent cut theory from distributed systems gives a clean algebra. The barrier is the set of datoms visible to all participants — intersection, not union. |
| Competing branch lock for multi-agent merge | When agents fork competing approaches (deliberation), only one merges. The losing branch remains readable for provenance but is not committed. Prevents the "merge everything" failure mode. |

### Open Questions

1. **SPEC.md modularization**: At 5,083 lines with 8 of 14 namespaces, the file will exceed NEG-008 (no massive monolithic files) during Wave 3. Splitting strategy needed before proceeding.
2. **Harvest proactive warning thresholds**: The Q(t) formula is specified but the concrete thresholds (SEED.md doesn't provide numbers) are marked as uncertainty (UNC-HARVEST-001). Need empirical calibration.
3. **Dynamic CLAUDE.md generation specifics**: The 7-step generation process is specified but the template format for the three-concern collapse is an implementation detail deferred to Stage 0.
4. **Merge cascade crash recovery**: ADR-MERGE-003 specifies WAL-based crash recovery, but the interaction with the append-only store invariant (INV-STORE-001) during partial merges needs deeper analysis.

### Failure Modes

No new failure modes discovered. Wave 2 namespaces directly address FM-001 (knowledge loss — harvest gap detection), FM-003 (anchoring bias — seed assembles from full store), and FM-004 (cascading incompleteness — bilateral branch duality).

### Files Created/Modified

| File | Action | Details |
|------|--------|---------|
| `SPEC.md` | MODIFIED | 3,173 → 5,083 lines, +51 elements (27 INV, 14 ADR, 10 NEG), 4 new namespaces (HARVEST, SEED, MERGE, SYNC) |
| `HARVEST.md` | MODIFIED | This entry appended |

### Recommended Next Action

**Plan SPEC.md modularization** before Wave 3. At 5,083 lines, adding 6 more namespaces (SIGNAL, BILATERAL, DELIBERATION, GUIDANCE, BUDGET, INTERFACE) would push past 8,000 lines. Options:
- Split into per-namespace files under `spec/` with a root SPEC.md index
- Split into per-wave files (SPEC-foundation.md, SPEC-lifecycle.md, SPEC-intelligence.md)
- Keep monolithic but with clear section markers for tooling

Then **produce SPEC.md Wave 3 — Intelligence namespaces** (SIGNAL, BILATERAL, DELIBERATION, GUIDANCE, BUDGET, INTERFACE). Estimated: ~30 INV, ~15 ADR, ~10 NEG across 6 namespaces.

---

## Session 004 — 2026-03-02 (ADR→Gap Analysis Cross-Reference Audit)

**Platform**: Claude Code (Opus 4.6)
**Duration**: ~30 minutes, single continuous session

### What Was Accomplished

1. **Systematic ADR→GAP_ANALYSIS.md cross-reference audit**
   - Cross-referenced all **125 ADRs** across 13 categories in `ADRS.md` against `GAP_ANALYSIS.md` and SEED.md §9
   - Used 5 parallel agents: 1 context reader (SEED.md §9 + FAILURE_MODES.md) + 4 analysis agents (one per ADR group)
   - Each agent read both `ADRS.md` and `GAP_ANALYSIS.md` in full, then assessed each ADR individually

2. **Coverage findings**:
   - **~33 ADRs fully covered** (26%) — primarily foundational substrate decisions (FD-001–009) and high-level MISSING capabilities
   - **~34 ADRs partially covered** (27%) — concept area touched but specific design decision not evaluated
   - **~58 ADRs not covered at all** (46%) — entirely absent from gap analysis

3. **Worst coverage gaps by category**:
   | Category | Total | Fully Covered | Gap |
   |----------|-------|---------------|-----|
   | Uncertainty & Authority (UA) | 12 | 0 | Entire subsystem absent — tensor, spectral authority, delegation, staleness |
   | Guidance System (GU) | 8 | 1 | Comonadic structure, lookahead, basin competition, spec-language all missing |
   | Agent Architecture (AA) | 7 | 0 | D-centric formalism, metacognitive layer, intention anchoring all missing |
   | Snapshot & Query (SQ) | 10 | 1 | Stratum classification, projection pyramid, bilateral query structure, FFI boundary |
   | Conflict & Resolution (CR) | 7 | 0 | Conservative detection invariant, routing tiers, formal predicate, precedent query |

4. **Added Section 11 to `GAP_ANALYSIS.md`** (~150 lines, 12 subsections + structural observations)
   - §11.1 Foundational Decisions (3 items: FD-004, FD-010, FD-012)
   - §11.2 Algebraic Structure (8 items: AS-001, AS-002, AS-004–006, AS-008–010)
   - §11.3 Protocol Decisions (5 items: PD-002–006)
   - §11.4 Protocol Operations (10 items: PO-001–003, PO-005–006, PO-008–009, PO-011–013)
   - §11.5 Snapshot & Query (9 items: SQ-002–010)
   - §11.6 Uncertainty & Authority (12 items: UA-001–012 — complete section)
   - §11.7 Conflict & Resolution (7 items: CR-001–007)
   - §11.8 Agent Architecture (6 items: AA-001–005, AA-007)
   - §11.9 Interface & Budget (8 items: IB-001–002, IB-004–005, IB-007, IB-009, IB-011–012)
   - §11.10 Guidance System (7 items: GU-001–003, GU-005–008)
   - §11.11 Lifecycle & Methodology (6 items: LM-005–006, LM-010, LM-012–014)
   - §11.12 Coherence & Reconciliation (9 items: CO-004–008, CO-011–014)
   - §11.13 Structural Observations (3 explanatory patterns)
   - Updated Table of Contents to include Section 11
   - Added closing addendum documenting the audit

5. **Three structural patterns identified** explaining why the original analysis missed these items:
   - **Module-by-module methodology misses protocol-level decisions** — Decisions about protocol properties (topology-agnosticism, delivery semantics, crash-recovery), formal algebraic structure (G-Set CvRDT, commitment weight, diamond lattice), and agent architecture (D-centric model, metacognitive layer) have no natural "module" to map to
   - **Capability gaps are identified but operational specifications are not** — The analysis says "signal system MISSING" but never evaluates the 8 specific signal types, their type signatures, or their invariants
   - **The entire Uncertainty & Authority subsystem is absent** — UA-001 through UA-012 form a coherent subsystem (tensor, decay, spectral authority, delegation, staleness) that the Go CLI predates and the gap analysis does not address

### Decisions Made

| Decision | Rationale |
|---|---|
| Include all NO and significant PARTIAL items in Section 11 | Conservative: better to flag an item for future analysis than to miss it. Avoids the anchoring bias identified in FM-003. |
| Organized by ADRS.md category, not by gap severity | Preserves traceability — each Section 11 subsection maps directly to an ADRS.md category, making future gap analysis of specific ADRs easy to locate. |
| Did not add items already adequately covered | User explicitly requested no redundant entries. Items with full YES coverage (e.g., FD-001–003, FD-005–009, AS-003, PD-001, PO-004, PO-007, PO-010, PO-014, SQ-001, LM-001–004, LM-007–009, CO-001–003, CO-009) were excluded. |
| PARTIAL items included only when the uncovered aspect represents a genuinely distinct design decision | For example, PO-013 (QUERY) is PARTIAL — the SQL-vs-Datalog gap is covered, but the 4 specific invariants are not. Since those invariants define properties the implementation MUST have, they warrant their own entry. |

### Key Findings

1. **The gap analysis has a systematic blind spot for "how" decisions.** It answers "does the CLI have X?" (YES/NO) but not "does X satisfy the specific properties specified in the ADR?" For example, the analysis says merge exists but is LWW-based (correct), but never evaluates whether the merge satisfies CvRDT commutativity/associativity/idempotence — and the commutativity bug in Section 5.5 proves it doesn't.

2. **FM-003 (anchoring bias) is now demonstrated at scale.** The original gap analysis anchored on SEED.md and evaluated the CLI module-by-module. ADRS.md contains 125 design decisions harvested from the transcripts — 58 of these are invisible to the gap analysis. This is a 46% miss rate, confirming that the SEED.md→transcript compression ratio (10:1) causes significant information loss.

3. **The relationship between ADRS.md and GAP_ANALYSIS.md is now well-defined.** ADRS.md is the complete index of design decisions; GAP_ANALYSIS.md evaluates the CLI against those decisions. Section 11 bridges the gap by cataloging which ADRs still need evaluation. Future gap analysis work should iterate through Section 11 items.

4. **FM-004 is now better scoped.** The 58 uncovered ADRs are not all missing from SEED.md — many are in the transcripts but captured in ADRS.md. The resolution path is clearer: (a) complete the ADRS.md→SEED.md reconciliation, then (b) complete the ADRS.md→GAP_ANALYSIS.md evaluation (Section 11 items).

### Open Questions

1. **FM-004 (S0) still unresolved** — Transcript→SEED.md reconciliation has not been performed. ADRS.md now serves as the comprehensive index of transcript decisions, which makes the reconciliation more tractable: compare ADRS.md against SEED.md rather than re-reading all 7 transcripts. (Carried from Sessions 002, 003.)
2. **Should Section 11 items be evaluated in bulk or incrementally?** Bulk: one session evaluates all 88 items against the Go CLI source code. Incremental: evaluate items as they become relevant during SPEC.md production. Incremental is more efficient but risks missing cross-cutting gaps.
3. **Implementation language for Braid** — Still unresolved. (Carried from Sessions 001, 002, 003.)
4. **Datom serialization format** — Still unresolved. (Carried from Session 001.)
5. **Relationship between SEED.md §9 and standalone GAP_ANALYSIS.md** — Still needs reconciliation. (Carried from Session 003.)

### Failure Modes Discovered

None new this session. FM-003 (anchoring bias) is now quantitatively confirmed: 46% miss rate across 125 ADRs.

### Files Created/Modified

| File | Action | Details |
|------|--------|---------|
| `GAP_ANALYSIS.md` | MODIFIED | Added Section 11 (ADR Coverage Gaps, ~150 lines, 12 subsections), updated Table of Contents, added closing addendum. File grew from ~920 to ~1071 lines. |
| `HARVEST.md` | MODIFIED | This entry appended |

### Recommended Next Action

**Resolve FM-004 (S0): ADRS.md→SEED.md reconciliation.** With ADRS.md now serving as the comprehensive design decision index (125 entries), the reconciliation is tractable: compare each ADRS.md entry against SEED.md and identify which confirmed decisions are missing from the seed. This is the highest-severity open failure mode and blocks SPEC.md production.

Secondary: **Evaluate Section 11 items against Go CLI source.** For each of the 88 ADR coverage gaps catalogued in GAP_ANALYSIS.md §11, perform a proper gap analysis of the Go CLI's status. This can be parallelized by category (one agent per subsection). Priority categories: Uncertainty & Authority (0/12 covered, forms a coherent subsystem) and Conflict & Resolution (0/7 covered, critical for correctness).

Tertiary: **Produce SPEC.md** (carried from Sessions 001–003). Now depends on both FM-004 resolution and the Section 11 evaluation.

---

## Session 005 — 2026-03-02 (Comprehensive ADR Coverage Analysis + GAP_ANALYSIS.md Completion)

**Platform**: Claude Code (Opus 4.6)
**Duration**: ~3 hours across two context windows (compaction occurred mid-session)

### What Was Accomplished

1. **Completed comprehensive ADR coverage analysis** (Waves 4–6, 12 agents total)
   - **Wave 4 — Orientation & Ground Truth** (5 agents):
     - Spec document inventory (7 docs, 20 ranked findings)
     - Full codebase inventory (234 .go files, 61,906 LOC, 34 packages)
     - Substrate layer deep-dive (39 tables, 357+ DELETE statements confirmed)
     - Verification layer deep-dive (5-tier contradiction engine mapped)
     - Lifecycle & interface deep-dive (bilateral loop, harvest/seed gap, k* formula)
   - **Wave 5 — Deep Per-ADR Analysis** (5 agents):
     - UA (001–012) + AS (001–010): 0 IMPLEMENTED, 5 PARTIAL, 0 DIVERGENT, 15 MISSING
     - GU (001–008) + IB (001–012): 1 IMPLEMENTED, 7 PARTIAL, 0 DIVERGENT, 11 MISSING
     - PO (001–014) + SQ (001–010): 0 IMPLEMENTED, 8 PARTIAL, 2 DIVERGENT, 13 MISSING
     - CR (001–007) + CO (001–014) + AA (001–007): 4 IMPLEMENTED, 10 PARTIAL, 0 DIVERGENT, 13 MISSING
     - LM (001–016) + PD (001–006) + FD (001–012): 7 IMPLEMENTED, 8 PARTIAL, 5 DIVERGENT, 6 MISSING
   - **Wave 6 — Verification** (2 agents):
     - Coverage completeness: 128/139 ADR entries covered; 11 new SR entries identified
     - Spot-check of 5 contested findings: FD-001 (O_APPEND confirmed), CO-009 (6 signals confirmed), PO-006 (LWW merge deletes confirmed), AA-006 (guidance struct confirmed), IB-005 (corrected: depth-dependent decay exists but no runtime context integration)

2. **Updated `GAP_ANALYSIS.md`** — comprehensive rewrite of Section 11
   - Header: Updated scope (139 ADRs, 14 categories), method (6 waves, 24 agents)
   - Executive Summary: Added ADR-level coverage table (12 IMPL, 41 PARTIAL, 10 DIVG, 66 MISS, 10 N/A)
   - Methodology: Added Waves 4–6 documentation
   - Section 11: **Complete rewrite** — from gap inventory to comprehensive status assessment:
     - §11.1–§11.13: Per-category tables with IMPLEMENTED/PARTIAL/DIVERGENT/MISSING/N/A status for every ADR, with specific Go source file evidence
     - §11.14: Aggregate findings (5 key insights from ADR-level analysis)
     - §11.15: Updated structural observations (3 patterns)
   - Table of Contents: Updated with 15 Section 11 subsections
   - File grew from 1,072 to 1,264 lines

3. **Produced `ADRS.md`** in previous session portion — 139 entries across 14 categories
   - Exhaustive 3-pass extraction from all 7 transcripts (374 decision points, 0 gaps)
   - Added SR (Store & Runtime) category with 11 entries discovered by Wave 6 coverage check
   - Categories: FD(12), AS(10), SR(11), PD(6), PO(14), SQ(10), UA(12), CR(7), AA(7), IB(12), GU(8), LM(16), CO(14)

4. **Updated `SEED.md` §9** to establish `GAP_ANALYSIS.md` as canonical gap analysis document

### Decisions Made

| Decision | Rationale |
|---|---|
| Assess every ADR entry, including N/A items | Completeness — even Braid-specific decisions (FD-009, FD-011, LM-001, LM-002, LM-011) benefit from explicit N/A annotation explaining why. |
| Organize Section 11 by category with tables | Tables enable rapid scanning. Per-category structure maps directly to ADRS.md categories for traceability (C5). |
| Wave 6 verification with spot-checks | Defense against propagation of agent errors. Verified the most consequential claims (append-only, fitness signals, merge semantics). One correction discovered (IB-005). |
| Include SR category as newly discovered | Wave 6 identified 11 Store & Runtime entries in ADRS.md that no Wave 5 agent covered. Making this explicit prevents false completeness claims. |
| Use "Evidence" column with specific file paths | Grounds every assessment in verifiable source code locations. Prevents abstract assessments disconnected from reality. |

### Key Findings

1. **ADR-level analysis reveals module-level assessment understates the gap.** 8 modules ALIGNED at module level, but only 12/139 (9%) design decisions IMPLEMENTED at ADR level. The aligned modules implement correct logic on wrong substrate.

2. **Four subsystems are completely absent**: Uncertainty & Authority (0/12), Conflict & Resolution (0/7), Agent Architecture (0/7), Guidance System (0/8). Combined: 34 design decisions with zero implementation.

3. **The branching system is the largest connected gap** — 7 decisions across 3 categories (AS-003–006, AS-010, PO-007, PD-001) with zero implementation. Must be designed into Stage 0 store.

4. **41 PARTIAL entries are the highest-value reference code** — algorithms exist, data access must change, missing pieces identified. These represent ~30% of the codebase that can inform Braid implementation.

5. **IB-005 correction**: Wave 2 claimed "only heuristic fallback." Wave 6 verified depth-dependent decay EXISTS (BaseBudget=12, Step=5, Floor=3 in autoprompt/budget.go) but runtime-measured k* (reading context_window.used_percentage) is genuinely MISSING.

### Open Questions

1. **FM-004 (S0)**: ADRS.md now serves as the comprehensive index (139 entries). The ADRS.md→SEED.md reconciliation is tractable but has not been performed. Carried from Sessions 002–004.
2. **SPEC.md production**: Now has complete foundation (SEED.md, ADRS.md, GAP_ANALYSIS.md). Ready to begin.
3. **Implementation language**: Unresolved. Carried from Sessions 001–004.
4. **Datom serialization format**: Unresolved. Carried from Session 001.
5. **Relationship between SEED.md §9 and GAP_ANALYSIS.md**: §9 edited to reference GAP_ANALYSIS.md as canonical. Reconciliation partially resolved.

### Failure Modes Discovered

None new. FM-003 (anchoring bias) is now definitively addressed — the ADR-level analysis eliminates the 46% miss rate identified in Session 004.

### Files Created/Modified

| File | Action | Details |
|------|--------|---------|
| `ADRS.md` | CREATED | 139 entries across 14 categories, ~950 lines. Exhaustive transcript extraction. |
| `GAP_ANALYSIS.md` | MODIFIED | Section 11 rewritten (gap inventory → comprehensive assessment), header/exec summary/methodology updated. 1,072 → 1,264 lines. |
| `SEED.md` | MODIFIED | §9 reference to GAP_ANALYSIS.md added |
| `HARVEST.md` | MODIFIED | This entry appended |

### Recommended Next Action

**Produce SPEC.md.** All prerequisites are now met:
- `SEED.md` — foundational design document (11 sections)
- `ADRS.md` — complete design decision index (139 entries, 14 categories)
- `GAP_ANALYSIS.md` — comprehensive codebase evaluation with per-ADR assessments (1,264 lines)
- `FAILURE_MODES.md` — bootstrap failure mode registry (4 FMs)

The SPEC.md production should work through SEED.md section by section, formalizing each implicit claim as an invariant with ID and falsification condition, recording each choice as an ADR, and using ADRS.md and GAP_ANALYSIS.md Section 11 as the source for operational specifications.

Secondary: **Resolve FM-004** (S0) — ADRS.md→SEED.md reconciliation. This can now be done incrementally during SPEC.md production rather than as a separate pass.

---

## Session 006 — 2026-03-02 (FM-004 Resolution: ADRS.md→SEED.md Reconciliation)

**Platform**: Claude Code (Opus 4.6)
**Duration**: ~45 minutes across two context windows (compaction occurred mid-session)

### What Was Accomplished

1. **Resolved FM-004 (S0 — highest-severity open failure mode)**
   - Launched 5 parallel reconciliation agents comparing all 125 ADRS.md entries against SEED.md
   - Agent 1: FD + AS + SR (33 ADRs) — found 9 HIGH items
   - Agent 2: PD + PO (20 ADRs) — found 8 HIGH items
   - Agent 3: SQ + UA + CR (29 ADRs) — found 19 HIGH items
   - Agent 4: AA + IB + GU (27 ADRs) — found 15 HIGH items
   - Agent 5: LM + CO (16 ADRs) — found 8 HIGH items
   - **Total: 59 HIGH items** requiring SEED.md additions

2. **Applied 7 edits to SEED.md**, adding ~32 lines (305→337, +10.5% growth):
   - §3: Added certainty + commitment dimensions to fitness function formula (CO-009)
   - §4: Replaced 2-line "Protocol-Level Design Decisions" paragraph with 4 new subsections:
     - "Implementation Architecture" — embedded, file-backed, 3-layer, 4+1 indexes, HLC, schema bootstrap (FD-010, SR-001/002/004/006/007/008, PO-012, AA-003)
     - "Query Engine" — Datalog dialect, CALM, 6 strata, frontier-as-datom, FFI boundary (FD-003, SQ-002/003/004/009/010)
     - "Agent Working Set and Recovery" — W_α two-tier, crash-recovery, TRANSACT signature, conflict predicate (PD-001/003, PO-001, CR-006)
     - Forward-reference paragraph for LOW items (PD-002/004, PO-005/006/007/011)
   - §5: Added "Operational Parameters" — 20-30 turn lifecycle, semi-automated harvest, FP/FN calibration, staleness, warnings, delegation topologies, crystallization guard (LM-005/006/011/012, UA-007, IB-012, CR-005)
   - §6: Added "Uncertainty, Authority, and Conflict" — tensor, decay, spectral authority, delegation, conservative detection, 3-tier routing, deliberation entities, dual-process, CLAUDE.md collapse, signal injection, test-as-datoms, taxonomy gaps (UA-001/002/003/005/009/012, CR-001/002/004, AA-001, GU-004, IB-009, CO-007/011)
   - §7: Added "Feedback Loop Architecture" — basin competition, 6 anti-drift mechanisms, metacognitive entities, access log separation (GU-006/007, AA-004, AS-007)
   - §8: Added Layer 4.5 + "Budget and Output Architecture" — hard invariant with 5-level precedence, k* measurement, projection pyramid, 3 output modes, 4-part footer, intention anchoring, bilateral query layer (IB-001/002/004/005/006, SQ-006/007, GU-005, AA-005)
   - §10: Added "Bootstrap Specifics" — every-command-is-transaction, genesis tx, branch ops, 10-step agent cycle, DDR feedback loop (FD-012, PO-007/011/012, LM-014)

3. **Reframed FAILURE_MODES.md** — complete rewrite per user directive:
   - **Old framing**: Task tracker for ad-hoc manual fixes (OPEN → RESOLVED when we patched SEED.md)
   - **New framing**: Agentic failure mode catalog — test cases and acceptance criteria for evaluating DDIS/Braid
   - **New lifecycle**: `OBSERVED → MAPPED → TESTABLE → VERIFIED` (tracks whether the methodology addresses the failure class)
   - Each FM now has: what happened, why it's hard for agents, root cause, DDIS/Braid mechanism, measurable SLA
   - Coverage summary: all 4 FMs map to mechanisms; target SLAs defined (≥99% for FM-001/004, 100% for FM-002, ≥95% for FM-003)
   - Current manual rates documented for baseline comparison (~47% miss rate for FM-001/003/004)
   - Updated SEED.md §10 and AGENTS.md references to match new framing

### Decisions Made

| Decision | Rationale |
|---|---|
| Concise additions with ADRS.md forward references | User explicitly requested keeping SEED.md from growing too large. 32 lines of growth to capture 59 items = high information density. LOW items get forward references, not full text. |
| Group by SEED.md section, not by ADR category | Natural reading order. Each section gets the information it needs. Avoids redundancy. |
| Three-classification system (CAPTURED/ABSTRACTED/MISSING → HIGH/LOW) | HIGH = "agent would build the wrong thing without this." LOW = "important detail but concept is captured at seed level." This filters 125 entries down to 59 actionable ones. |
| FAILURE_MODES.md is a test catalog, not a task tracker | User directive: the document's purpose is acceptance criteria for DDIS/Braid, not tracking manual fixes. "RESOLVED because we edited SEED.md" misses the point — the question is whether the methodology prevents the failure class. |
| Basin competition model (GU-006) highlighted in §7 | Identified by Agent 4 as "the single most consequential gap" — it explains WHY all the anti-drift mechanisms exist. |

### Key Findings

1. **59 of 125 ADRs (47%) contained information that would cause an agent to build the wrong thing** if reading SEED.md alone. This validates the S0 severity classification of FM-004.

2. **The worst gaps were in §6 (Reconciliation)** — the entire Uncertainty & Authority subsystem (12 ADRs) and Conflict Resolution specifics (7 ADRs) were absent.

3. **§4 was the most critical section for additions** — zero information about indexes, HLC, schema bootstrap, Datalog dialect, query classification, or the TRANSACT signature.

4. **The reconciliation approach (ADRS.md→SEED.md rather than transcripts→SEED.md) was correct.** Comparing document-to-document was tractable; reading all 7 transcripts would have been 5-10x more expensive.

### Open Questions

1. **Implementation language for Braid** — Still unresolved. (Carried from Sessions 001–005.)
2. **Datom serialization format** — Still unresolved. (Carried from Session 001.)
3. **GAP_ANALYSIS.md Section 11 evaluation** — 88 ADR coverage gaps catalogued but not yet evaluated against Go CLI source. (Carried from Session 004.)

### Failure Modes

All 4 FMs now have DDIS/Braid mechanisms identified and acceptance criteria defined:

| FM | Status | Target SLA | Current Manual Rate |
|----|--------|------------|---------------------|
| FM-001 (knowledge loss) | TESTABLE | ≥99% decision capture | ~53% |
| FM-002 (provenance fabrication) | TESTABLE | 100% verifiable provenance | Unknown |
| FM-003 (anchoring bias) | MAPPED | ≥95% analysis coverage | ~54% |
| FM-004 (cascading incompleteness) | TESTABLE | ≥99% completeness detection | ~53% |

### Files Modified

| File | Action | Details |
|------|--------|---------|
| `SEED.md` | MODIFIED | 8 edits total: 7 adding subsections in §§3,4,5,6,7,8,10 (305→338 lines); 1 updating §10 FAILURE_MODES.md reference |
| `FAILURE_MODES.md` | REWRITTEN | Complete rewrite: task tracker → test case catalog. New lifecycle, acceptance criteria, SLA targets. |
| `AGENTS.md` | MODIFIED | Updated FAILURE_MODES.md description in project structure and source documents sections |
| `HARVEST.md` | MODIFIED | This entry appended |

### Recommended Next Action

**Produce SPEC.md** — the DDIS-structured specification. FM-004 is resolved. SEED.md is now comprehensive (337 lines, all 125 ADRS.md entries reconciled). Work through SEED.md §§1–11 section by section, formalizing each claim as invariants (with IDs and falsification conditions), ADRs (with alternatives and rationale), and negative cases.

Secondary: **Evaluate GAP_ANALYSIS.md Section 11 items** against Go CLI source. Parallelizable by category.

Tertiary: **Decide implementation language** (Rust vs Go). Blocks Stage 0 but not SPEC.md.

---

## Session 007 — 2026-03-02 (GAP_ANALYSIS.md Finalization + Carry-Forward Resolution)

**Platform**: Claude Code (Opus 4.6)
**Duration**: ~20 minutes, single session (continuation from Session 005 after context compaction)

### What Was Accomplished

1. **Completed GAP_ANALYSIS.md Section 11 rewrite** (Task 10 from Session 005)
   - Rewrote Section 11 from gap inventory ("58 ADRs not assessed") to comprehensive status assessment for all 139 ADRs
   - 15 subsections: §11.1–§11.13 per-category tables, §11.14 aggregate findings, §11.15 structural observations
   - Every ADR has IMPLEMENTED/PARTIAL/DIVERGENT/MISSING/N/A status with specific Go source file evidence
   - Updated header (6 waves, 24 agents), executive summary (ADR-level table), methodology (Waves 4–6), Table of Contents
   - File: 1,072 → 1,265 lines

2. **Resolved all carry-forward open questions**:
   - **Implementation language**: User confirmed **Rust**. Updated AGENTS.md project structure (`language TBD, likely Rust` → `Rust`). Aligns with FD-011 in ADRS.md.
   - **GAP_ANALYSIS.md Section 11 evaluation**: Already completed in this session (Session 006 harvest carried it forward from Session 004 state, not knowing Session 005 resolved it).
   - **Datom serialization format**: Remains unresolved but does not block SPEC.md.

3. **Appended Session 005 harvest entry** to HARVEST.md

### Decisions Made

| Decision | Rationale |
|---|---|
| Rust as implementation language | User decision. Aligns with FD-011, transcript history (user confirmed at 04:2397), and DATOMIC_IN_RUST.md reference material. |
| Section 11 uses per-ADR tables with Evidence column | Grounds every assessment in verifiable source code. Enables future agents to validate or update assessments by checking the cited files. |

### Open Questions

1. **Datom serialization format** — Unresolved. Carried from Session 001. Does not block SPEC.md.

### Files Modified

| File | Action | Details |
|------|--------|---------|
| `GAP_ANALYSIS.md` | MODIFIED | Section 11 rewritten, header/exec summary/methodology/TOC updated. 1,072 → 1,265 lines. |
| `AGENTS.md` | MODIFIED | Project structure: `language TBD, likely Rust` → `Rust` |
| `HARVEST.md` | MODIFIED | Sessions 005 + 007 entries appended |

### Recommended Next Action

**Produce SPEC.md.** All blockers are resolved:
- SEED.md complete (338 lines, FM-004 resolved)
- ADRS.md complete (139 entries, 14 categories)
- GAP_ANALYSIS.md complete (1,265 lines, all 139 ADRs assessed)
- Implementation language decided (Rust)
- All failure modes resolved or documented

Work through SEED.md §§1–11, formalizing claims as invariants (INV-{NS}-{NNN}), design choices as ADRs (ADR-{NS}-{NNN}), and bounds as negative cases (NEG-{NS}-{NNN}). Use ADRS.md as the operational specification source. Namespaces: STORE, QUERY, HARVEST, SEED, GUIDANCE, MERGE, DELIBERATION, SIGNAL, SYNC, BILATERAL, SCHEMA, RESOLUTION, BUDGET, INTERFACE.

---

## Session 008 — 2026-03-02: SPEC.md Modularization

### Task

Modularize SPEC.md (8,157 lines) into `spec/` directory with one file per namespace. Prerequisite for IMPLEMENTATION_GUIDE.md production — enables per-namespace context loading to prevent FM-001/FM-003 failure modes.

### What Was Accomplished

| File | Action | Details |
|------|--------|---------|
| `spec/` directory | CREATED | 19 files: README.md + 00-preamble.md + 14 namespace files + 3 integration files |
| `spec/00-preamble.md` | CREATED | Lines 1–137 of SPEC.md (title block + §0 shared definitions) |
| `spec/01-store.md` – `spec/14-interface.md` | CREATED | 14 namespace sections, exact content with compact navigation headers |
| `spec/15-uncertainty.md` – `spec/17-crossref.md` | CREATED | Integration sections + Appendices A–C |
| `spec/README.md` | CREATED | Master index with wave grouping, reading order, links |
| `SPEC.md` | MODIFIED | Replaced with thin stub pointing to `spec/` |
| `CLAUDE.md` | MODIFIED | Updated project structure, source doc refs, and task guidance to reference `spec/` |
| `HARVEST.md` | MODIFIED | Session 008 entry appended |

### Verification

- **Content integrity**: Concatenating all spec files (stripping 3-line headers) produces byte-for-byte identical output to original SPEC.md
- **Element counts preserved**: 310 INV refs, 85 ADR refs, 54 NEG refs — all match original
- **Line count**: 8,208 total = 8,157 original + 51 added header lines (17 files × 3 lines)
- **NEG-008 resolved**: No file exceeds 1,175 lines (STORE is the largest)

### Decisions Made

| Decision | Rationale |
|---|---|
| Flat `spec/` directory (no subdirectories) | 18 files is manageable; nested dirs would complicate relative links |
| Compact 2-line navigation header per namespace file | Provides wave/stage context and preamble link without adding noise |
| §17 + Appendices A–C in single file | These are cross-cutting reference tables that belong together |
| SPEC.md retained as stub (not deleted) | RULE NUMBER 1: no file deletion |
| §0 title block (lines 1–19) included in preamble | Title block is document-level metadata; preamble is the natural home |

### Open Questions

None introduced. No content was modified.

### Recommended Next Action

**Produce IMPLEMENTATION_GUIDE.md.** The specification is now modularized for per-namespace loading. Work through Stage 0 namespaces first (STORE, SCHEMA, QUERY, HARVEST, SEED, GUIDANCE, INTERFACE), loading one `spec/` file at a time to ensure full attention per namespace.

---

## Session 009 — 2026-03-02: IMPLEMENTATION_GUIDE.md Production (Modularized as `guide/`)

### Task

Produce the implementation guide — the definitive build plan for the implementing agent. Modularized as `guide/` (13 files), mirroring the `spec/` pattern. Grounded in formal methods, cleanroom software engineering, and prompt-optimization methodology.

Traces to: SEED.md §10 (Concrete Next Step 3), CLAUDE.md task-specific guidance.

### What Was Accomplished

| File | Lines | Content |
|------|-------|---------|
| `guide/README.md` | ~130 | Master index, build order, cognitive phase protocol, spec cross-reference |
| `guide/00-architecture.md` | ~600 | Crate workspace layout, core type catalog (Datom through Store), Cargo.toml files, file formats (JSONL, redb, seed template, dynamic CLAUDE.md), CLI command signatures (clap derive structs), MCP tool definitions (9 tools with JSON Schema), LLM-native interface design (output algebra, error protocol, guidance footer design, token targets), SPEC-GAP markers, uncertainty resolution protocol |
| `guide/01-store.md` | ~280 | STORE build plan — module structure, public API, three-box decomposition (Datom, Store, Transaction typestate), type-level encoding, LLM-facing outputs, proptest strategies, Kani harnesses, implementation checklist |
| `guide/02-schema.md` | ~230 | SCHEMA build plan — genesis constants (17 axiomatic attributes), Schema type, self-description verification |
| `guide/03-query.md` | ~260 | QUERY build plan — Datalog parser, semi-naive evaluator, stratum classification, CALM compliance |
| `guide/04-resolution.md` | ~220 | RESOLUTION build plan — three resolution modes, conflict predicate, LIVE index |
| `guide/05-harvest.md` | ~250 | HARVEST build plan — epistemic gap detection, five-stage pipeline, quality metrics, candidate presentation |
| `guide/06-seed.md` | ~270 | SEED build plan — associate/assemble/compress, dynamic CLAUDE.md, five-part trajectory seed as prompt component |
| `guide/07-merge-basic.md` | ~180 | MERGE Stage 0 subset — INV-MERGE-001/008 only, pure set union |
| `guide/08-guidance.md` | ~230 | GUIDANCE build plan — drift detection, six anti-drift mechanisms, footer selection algorithm, navigative language |
| `guide/09-interface.md` | ~260 | INTERFACE build plan — output mode dispatch, MCP server, persistence bridge, error protocol |
| `guide/10-verification.md` | ~280 | Tiered verification pipeline (Gates 1–5), CI configuration (GitHub Actions YAML), coverage matrix, proptest configuration, quality gate protocol |
| `guide/11-worked-examples.md` | ~520 | Self-bootstrap demo (genesis → schema → spec transact → query), harvest/seed session transcript (10-turn lifecycle), 5 Datalog queries, 3 error recovery demos, MCP round-trip demo |
| `guide/12-stages-1-4.md` | ~190 | Stage 1–4 roadmap, extension points per stage, INV activation table |
| `IMPLEMENTATION_GUIDE.md` | ~20 | Stub pointer to `guide/` (same pattern as SPEC.md → spec/) |
| `CLAUDE.md` | MODIFIED | Updated project structure to include `guide/` directory |

**Totals**: 13 guide files + 1 stub pointer. ~3,900 lines across all guide files.

### Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Modularized as `guide/` (not monolithic) | NEG-008 (no massive files); enables per-namespace loading during implementation |
| `guide/NN-*.md` numbering mirrors `spec/NN-*.md` | Implementing agent loads `spec/03-query.md` alongside `guide/03-query.md` — mental mapping |
| Three-box decomposition (black/state/clear) per core type | Cleanroom methodology (Mills); each box is independently verifiable |
| Exact Rust type signatures (not prose) | Implementing agent needs precise contracts, not descriptions |
| LLM-native design as explicit §0.6 section | Core structural invariant of Braid — every output is an optimized prompt |
| Worked examples as the longest section | Per prompt-optimization: demonstrations encode what constraints cannot |
| `[SPEC-GAP]` markers for spec augmentation | Four potential spec additions identified during guide production |
| Build order: STORE → SCHEMA → QUERY → RESOLUTION → MERGE → HARVEST → SEED → GUIDANCE → INTERFACE | Follows invariant dependency graph from spec/17-crossref.md §17.2 |

### SPEC-GAP Markers Identified

Four potential specification additions flagged for follow-up:
1. `[SPEC-GAP]` Tool description quality metric (INV-INTERFACE-008 candidate)
2. `[SPEC-GAP]` Error message recovery-hint completeness (INV-INTERFACE-009 candidate)
3. `[SPEC-GAP]` Dynamic CLAUDE.md as formally optimized prompt (INV-GUIDANCE-007 augmentation)
4. `[SPEC-GAP]` Token efficiency as testable property (INV-BUDGET-006 candidate)

### Failure Modes Observed

None triggered. NEG-001 (no stubs) — all files complete. NEG-005 (no unstructured prose) — all guide sections use structured format. NEG-008 (no massive files) — largest file ~600 lines.

### Open Questions

- UNC-SCHEMA-001 (17 axiomatic attributes): guide specifies genesis constants but verification requires implementation. Flagged in guide/02-schema.md.
- Should `braid-kernel` use `edition = "2024"` or `"2021"`? (2024 may not be stable yet — implement should verify)

### Recommended Next Action

**Begin Stage 0 implementation.** The implementing agent's workflow:
1. Read `guide/README.md` (build order, cognitive protocol)
2. For each namespace in order: read `spec/NN-*.md` then `guide/NN-*.md`
3. Implement following three-box decomposition and verification checklist
4. First act: `Store::genesis()` + spec element self-bootstrap (guide/11-worked-examples.md §11.1)

---

## Session 006 — 2026-03-02 (Close 4 SPEC-GAP Markers with Formal Invariants)

**Platform**: Claude Code (Opus 4.6)
**Duration**: Single focused session

### What Was Accomplished

Closed all 4 `[SPEC-GAP]` markers identified during implementation guide production.
Three new invariants added, one augmented, one negative case added. All with full
three-level refinement (L0 algebraic → L1 state machine → L2 implementation contract),
falsification conditions, verification tags, and traceability.

**Files modified** (6 total):

1. **`spec/14-interface.md`** — Added INV-INTERFACE-008 (MCP Tool Description Quality),
   INV-INTERFACE-009 (Error Recovery Protocol Completeness), NEG-INTERFACE-004
   (No Error Without Recovery Hint)

2. **`spec/12-guidance.md`** — Augmented INV-GUIDANCE-007 from "Dynamic CLAUDE.md
   Improvement" to "Dynamic CLAUDE.md as Optimized Prompt" — added k* constraint budget,
   ambient/active partition (≤80 tokens), demonstration density ≥1.0, typestate generation
   pipeline (MeasureDrift → DiagnoseDrift → SelectCorrections → ValidateBudget → Emit),
   Level 2 implementation contract

3. **`spec/13-budget.md`** — Added INV-BUDGET-006 (Token Efficiency as Testable Property)
   with density monotonicity, mode-specific ceilings (agent ≤300, guidance ≤50, error ≤100),
   rate-distortion bound

4. **`spec/16-verification.md`** — Updated verification matrix (BUDGET 5→6, INTERFACE 7→9),
   gate coverage (104→107 proptest, 42→44 kani), all statistics

5. **`spec/17-crossref.md`** — Updated Appendix A (107 INV, 42 NEG, 212 total elements),
   Appendix B (all percentages), Appendix C (Stage 0: 62→64, includes INV-INTERFACE-008–009,
   NEG-INTERFACE-004), dependency graph (3 new edges), Stage 1 count (17→18)

6. **`guide/00-architecture.md`** — Replaced 4 `[SPEC-GAP]` markers with "Resolved Spec Gaps"
   section referencing the now-defined invariants

### Decisions Made

| Decision | Rationale |
|---|---|
| INV-INTERFACE-008 at Stage 0 | MCP tool descriptions are needed from the first usable build; quality gates should be in place from day one |
| INV-INTERFACE-009 at Stage 0 | Errors are produced from the earliest stage; recovery protocol prevents agents from hitting dead ends |
| INV-BUDGET-006 at Stage 1 | Token efficiency depends on the budget manager (all BUDGET INVs are Stage 1); density monotonicity requires the projection pyramid to be implemented first |
| NEG-INTERFACE-004 as separate negative case | The safety property `□(error_emitted → recovery_hint_present)` deserves its own proptest strategy independent of INV-INTERFACE-009's structural requirements |
| Augment INV-GUIDANCE-007 in place rather than creating new INV | The CLAUDE.md generation invariant is the same conceptual element — augmentation preserves ID stability and traceability |

### Verification Results

| Check | Result |
|---|---|
| Unique INV definitions | 107 (across 14 spec files) |
| Unique NEG definitions | 42 (across 14 spec files) |
| SPEC-GAP markers remaining | 0 (in guide/) |
| Falsification conditions present | All new INVs have them |
| Stage 0 count | 64 (updated in 17-crossref.md) |

### Failure Modes Observed

None triggered. All new spec elements have IDs, types, traceability, falsification conditions,
and three-level refinement. No stubs (NEG-001), no unstructured prose (NEG-005).

### Open Questions

None new. All gaps cleanly resolved.

### Recommended Next Action

**Begin Stage 0 implementation.** The specification is now complete enough for Stage 0:
64 invariants with full verification strategies, 0 SPEC-GAP markers, all cross-references
consistent. The implementing agent should follow the workflow in Session 005's recommendation.

