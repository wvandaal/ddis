# Braid

> **Identity**: You are working on **Braid**, the next-generation DDIS implementation.
> Braid replaces the existing Go CLI (`../ddis-cli/`, ~62,500 LOC) with a system
> built from first principles on the foundations established in `SEED.md`.
> The existing Go CLI and its specifications (`../ddis-modular/`, `../ddis-cli-spec/`)
> are reference material — not the codebase you are extending.

---

<purpose>

## What This Project Is

DDIS (Decision-Driven Implementation Specification) is a specification standard, protocol, and
knowledge substrate whose purpose is to maintain **verifiable coherence** between intent,
specification, implementation, and observed behavior — across people, AI agents, and time.

The fundamental problem DDIS solves is **divergence**: the inevitable drift between what you want,
what you wrote down, how you said to build it, and what actually got built. DDIS makes coherence
a structural property of the system rather than a process obligation that decays under pressure.

Braid is the implementation of this vision. It is built on an append-only datom store with
CRDT merge semantics, a harvest/seed lifecycle that makes conversations disposable while
knowledge remains durable, and a specification formalism (invariants, ADRs, negative cases,
uncertainty markers) that enables automated coherence verification at every boundary.

**DDIS specifies itself.** The specification elements (invariants, ADRs, negative cases) become
the first dataset the system manages. The specification is both the plan and the first data.

</purpose>

<primary_directive>

## The One Rule

**Every session must leave the project in a better state than it found it.**

This means: no speculative scaffolding, no aspirational stubs, no "TODO: implement later."
Every artifact committed must be complete within its scope, tested if it's code, and traceable
to a goal in `SEED.md`. If the scope is too large for one session, reduce the scope — do not
reduce the quality.

</primary_directive>

---

<context_and_orientation>

## Project Structure

```
ddis-braid/                                     ← YOU ARE HERE
├── CLAUDE.md                                   ← This file (read first, every session)
├── SEED.md                                     ← The foundational design document (read second)
├── spec/                            ← Modularized specification (one file per namespace)
│   ├── README.md                    ← Master index with wave grouping and reading order
│   ├── 00-preamble.md               ← Shared definitions (conventions, verification tags, constraints)
│   ├── 01-store.md – 14-interface.md  ← 14 namespace specifications (§1–§14)
│   └── 15-uncertainty.md – 17-crossref.md  ← Integration sections (§15–§17 + Appendices)
├── crates/                          ← Implementation (Rust)
│   ├── braid-kernel/                ← Core datom store, schema, query engine
│   └── braid/                       ← CLI and higher-level APIs
├── docs/                            ← All documentation
│   ├── HARVEST.md                   ← [ARCHIVED → docs/history/] Replaced by braid seed
│   ├── design/                      ← Design documents
│   │   ├── ADRS.md                  ← Design decision index (all settled choices with rationale)
│   │   ├── FAILURE_MODES.md         ← Agentic failure mode catalog (test cases + acceptance criteria)
│   │   └── DEFECT_SPEC.md           ← Defect specification
│   ├── audits/                      ← Audit artifacts
│   │   └── GAP_ANALYSIS.md          ← Existing code vs. specification comparison
│   ├── guide/                       ← Modularized implementation guide (one file per namespace)
│   │   ├── README.md                ← Master index, build order, cognitive phase protocol
│   │   ├── 00-architecture.md       ← Crate layout, type catalog, CLI/MCP specs, LLM-native design
│   │   ├── 01-store.md – 09-interface.md  ← Per-namespace build plans (§1–§9)
│   │   ├── 10-verification.md       ← Verification pipeline, CI gates, coverage matrix
│   │   ├── 11-worked-examples.md    ← Self-bootstrap demo, session transcripts, Datalog queries
│   │   └── 12-stages-1-4.md         ← Future roadmap, extension points
│   └── history/                     ← Historical reference material
│       ├── onboarding.md            ← Comprehensive guide to the existing DDIS project
│       ├── references/              ← Reference docs & conversation history from related discussions
│       │   ├── AGENTIC_SYSTEMS_FORMAL_ANALYSIS.md
│       │   ├── BRAID_IDEATION_TRANSCRIPT.md
│       │   └── DATOMIC_IN_RUST.md
│       └── transcripts/             ← Conversation history from design sessions
│           ├── journal.md           ← Index of all transcripts with summaries
│           ├── journal.txt          ← Index of all transcripts with summaries (Raw TXT with JSON)
│           ├── *.txt                ← Individual session transcripts (7 chapters, raw TXT with JSON)
│           └── *.md                 ← Individual session transcripts (7 chapters)
```

### Sibling Directories (Reference Material)

```
../ddis-modular/                     ← LAYER 1: The meta-standard ("how to write a DDIS spec")
../ddis-cli-spec/                    ← LAYER 2: The CLI specification (97 INVs, 74 ADRs, 9 modules)
../ddis-cli/                         ← LAYER 3: The Go implementation (~62,500 LOC, 238 .go files)
../ddis-evolution/                   ← Historical archive (version checkpoints)
../docs/                             ← Design documents and reference material
../ralph/                            ← RALPH improvement loop toolchain
../.ddis/                            ← Runtime artifacts (audits, event streams)
```

</context_and_orientation>

<source_documents>

## Source Documents — Reading Order

Read these in order when you need to understand the design:

1. **`SEED.md`** — The foundational document. 11 sections covering: what DDIS is, the divergence
   problem, specification formalism, datom abstraction, harvest/seed lifecycle, reconciliation
   mechanisms, self-improvement loop, interface principles, existing codebase, staged roadmap,
   and design rationale. Everything traces back to this document.

2. **`docs/history/transcripts/journal.txt`** — Index of 7 design session transcripts with summaries. These
   contain the full reasoning behind every decision in the seed. Read individual transcripts
   only when you need the detailed rationale for a specific design choice.

3. **`docs/history/onboarding.md`** — Comprehensive guide to the existing DDIS project (the Go CLI). Covers
   the three-layer spec architecture, directory structure, bilateral cycle, event-sourcing model,
   and quality metrics. Use this to understand what exists, not what to build.

4. **`../ddis-cli-spec/`** — The existing CLI specification. 97 invariants, 74 ADRs across 9
   modules. Reference for understanding how DDIS specifications are structured in practice.

5. **`docs/design/FAILURE_MODES.md`** — Agentic failure mode catalog. Documents real failures observed
   when using AI agents for complex work (knowledge loss, provenance fabrication, anchoring
   bias, cascading incompleteness). Each entry maps to a DDIS/Braid mechanism and defines
   an acceptance criterion with SLA target. These are test cases for evaluating whether the
   methodology works — not a task tracker for ad-hoc fixes.

6. **`docs/audits/GAP_ANALYSIS.md`** — Comprehensive analysis of the Go CLI (~62,500 LOC) against SEED.md.
   Categorizes 38 packages as ALIGNED/DIVERGENT/EXTRA/BROKEN/MISSING. Use when understanding
   what exists in the Go CLI and how it maps to Braid's design.

7. **`docs/design/ADRS.md`** — Design decision index. All settled choices from design transcripts and
   sessions, with rationale, alternatives rejected, and transcript references. Lightweight
   precursor to formal ADR elements in `spec/`. Check before relitigating any decision.

</source_documents>

---

<methodology>

## How to Work

### Session Lifecycle

Every session follows this pattern:

**1. Orient.** Read this file. Read the braid-seed section below for prior session context,
or run `braid seed --task "your task"` for a fresh context assembly.
Check `docs/design/FAILURE_MODES.md` for open failure modes that may intersect your task.
If a specific task was assigned, locate it.

**2. Plan.** Before writing any code or spec, state what you intend to do and why. Trace the
work back to a section in `SEED.md` or a specific invariant/ADR. If you cannot trace it,
question whether the work should be done.

**3. Execute.** Do the work. Follow the constraints below. Test everything testable. Document
every design decision as an ADR. When you discover a process failure, methodology gap, or
unexpected divergence, record it immediately in `docs/design/FAILURE_MODES.md` with an FM-NNN ID.

**4. Harvest.** Before the session ends, run `braid harvest --commit` to extract session
knowledge into the store. Then run `braid seed --inject AGENTS.md` to refresh the seed
section for the next session. The harvest captures accomplishments, decisions, open questions,
and git context automatically.

### The Harvest/Seed Discipline

The datom store now automates the harvest/seed lifecycle:

- At session start: read the braid-seed section in this file (auto-injected by `braid seed --inject AGENTS.md`)
- During work: run `braid observe "your insight" --confidence 0.8` to capture knowledge
- At session end: run `braid harvest --commit` then `braid seed --inject AGENTS.md`

The store replaces `docs/HARVEST.md` (archived at `docs/history/HARVEST.md.archived`).
Knowledge is captured as datoms, not as prose in a log file.

### Specification Methodology (DDIS-on-DDIS)

When writing specification elements, every element must have:

- **An ID** following the pattern: `INV-{NAMESPACE}-{NNN}`, `ADR-{NAMESPACE}-{NNN}`,
  `NEG-{NAMESPACE}-{NNN}` (e.g., `INV-STORE-001`, `ADR-EAV-001`, `NEG-MUTATION-001`)
- **A type**: invariant, ADR, negative case, section, goal, or uncertainty
- **Traceability**: explicit reference to which `SEED.md` section motivates this element
- **Falsification condition** (invariants and negative cases): "This is violated if..."
- **Uncertainty marker** (where applicable): confidence level (0.0–1.0) and what would resolve it

```
### INV-STORE-001: Append-Only Immutability

**Traces to**: SEED.md §4 (Design Commitment #2)
**Type**: Invariant
**Statement**: The datom store never deletes or mutates an existing datom. All state changes
are new assertions (including retractions, which are datoms with op=retract).
**Falsification**: Any operation that removes a datom from the store, or modifies the [e,a,v,tx,op]
tuple of an existing datom in place, violates this invariant.
**Verification**: Unit test asserting that after any sequence of operations, the count of datoms
is monotonically non-decreasing and all previously-observed datom tuples remain present.
```

</methodology>

---

<staged_roadmap>

## The Staged Roadmap

Work proceeds in stages. **Stage 0 is complete.** The specification (`spec/`), implementation
guide (`docs/guide/`), and gap analysis (`docs/audits/GAP_ANALYSIS.md`) are all produced.
The kernel implementation has 288 passing tests, formal verification (Kani + Stateright),
and full CI pipelines. **Current focus is Stage 1.**

### Completed: Specification Production + Stage 0 Implementation

The following are complete and serve as the foundation for all future stages:

1. **`spec/`** — 21 namespace specification files with invariants, ADRs, negative cases,
   uncertainty markers. See `spec/README.md` for the master index.

2. **`docs/guide/`** — Stage 0–4 deliverables, CLI command specs, file formats, success criteria.

3. **`docs/audits/GAP_ANALYSIS.md`** — Every existing module in `../ddis-cli/` categorized as
   ALIGNED, DIVERGENT, EXTRA, BROKEN, or MISSING relative to the specification.

4. **`crates/braid-kernel/`** — 17,912 LOC Rust: datom store, schema, query engine (Datalog
   with stratification), harvest/seed lifecycle, merge with CRDT semantics, resolution modes,
   trilateral coherence, guidance system, dynamic CLAUDE.md generation. 288 tests passing.

5. **`crates/braid/`** — CLI binary with 10 commands: init, status, transact, query, harvest,
   seed, guidance, merge, log, generate.

### Stage 0: Harvest/Seed Cycle (Target: 1–2 weeks implementation)

Validate the core hypothesis: harvest/seed transforms workflow from "fight context loss"
to "ride context waves."

**Deliverables**: `transact`, `query`, `status`, `harvest`, `seed`, `guidance`, dynamic
CLAUDE.md generation.

**Success criterion**: Work 25 turns, harvest, start fresh with seed — new session picks up
without manual re-explanation.

**First act**: Migrate the specification elements from `spec/` into the store as datoms.

### Stage 1: Budget-Aware Output + Guidance Injection
### Stage 2: Branching + Deliberation
### Stage 3: Multi-Agent Coordination
### Stage 4: Advanced Intelligence

(Details in SEED.md §10)

</staged_roadmap>

---

<constraints>

## Hard Constraints

These are non-negotiable. Violating any of these is a defect regardless of other merits.

**C1: Append-only store.** The datom store never deletes or mutates. Retractions are new datoms
with `op=retract`. No exceptions, no "temporary" mutations, no "cleanup" deletions.

**C2: Identity by content.** A datom is `[e, a, v, tx, op]`. Two agents asserting the same fact
independently produce one datom. Content-addressable identity, not sequential IDs.

**C3: Schema-as-data.** The schema is defined as datoms in the store, not as a separate DDL or
config file. Schema evolution is a transaction, not a migration.

**C4: CRDT merge by set union.** Merging two stores is the mathematical set union of their datom
sets. No heuristics, no conflict resolution at merge time. Conflict resolution is a query-layer
concern using per-attribute resolution modes.

**C5: Traceability.** Every implementation artifact must trace to at least one specification
element. Every specification element must trace to at least one goal in SEED.md. Orphans in
either direction are defects.

**C6: Falsifiability.** Every invariant must have an explicit falsification condition. "This
invariant is violated if..." is required, not optional. An invariant without a falsification
condition is not an invariant — it is a wish.

**C7: Self-bootstrap.** DDIS specifies itself. The specification elements are the first data
the system manages. The system's first act of coherence verification is checking its own
specification for contradictions.

</constraints>

<negative_cases>

## What NOT To Do

These are the failure modes observed in prior sessions and in LLM coding patterns generally.
Each is phrased as a falsifiable negative case.

**NEG-001: Do not generate aspirational stubs.** If you write a function signature with
`// TODO: implement` or `unimplemented!()`, you have produced waste. Either implement it
fully or don't create the file. Partial implementations that compile but don't work are
worse than no implementation.

**NEG-002: Do not relitigate settled decisions.** If an ADR exists for a design choice
(e.g., "Why EAV instead of relational?" — SEED.md §11), do not propose alternatives unless
you have found a contradiction with another ADR or invariant. ADRs exist precisely to prevent
this pattern.

**NEG-003: Do not optimize prematurely.** The first priority is correctness against the
specification. Performance optimization comes after the invariants pass. An efficient
implementation that violates an invariant is a defect.

**NEG-004: Do not conflate braid with ddis-cli.** The existing Go CLI is reference material.
You are not patching it, extending it, or migrating it. Braid is a new implementation built
from the specification. Code from ddis-cli may inform design, but copying it wholesale
without verifying alignment with the specification is prohibited.

**NEG-005: Do not write specification prose without structure.** Every claim about what the
system should do must be either an invariant (with ID and falsification condition), an ADR
(with alternatives and rationale), or a negative case (with violation condition). Unstructured
prose that reads like a requirements document is not a DDIS specification.

**NEG-006: Do not skip the harvest.** Every session must end with `braid harvest --commit`.
A session without a harvest is knowledge lost. Run `braid seed --inject AGENTS.md` after
harvesting to refresh context for the next session.

**NEG-007: Do not treat uncertainty as a defect.** Where the specification isn't sure yet,
mark it with a confidence level and what would resolve it. Do not write aspirational prose
that reads like settled commitment. Agents implementing uncertain claims as axioms is a
critical failure mode.

**NEG-008: Do not produce massive monolithic files.** Specifications, code, and documentation
should be modular. A 10,000-line file is a sign that decomposition was skipped. Prefer
many small, focused files over few large ones.

</negative_cases>

---

<reconciliation_taxonomy>

## The Reconciliation Framework

All protocol operations in DDIS are instances of one fundamental operation:
**detect divergence → classify it → resolve it back to coherence.**

When designing or implementing any feature, identify which divergence type it addresses:

| Divergence Type | Boundary | Detection | Resolution |
|---|---|---|---|
| Epistemic | Store vs. agent knowledge | Harvest gap detection | Harvest (promote to datoms) |
| Structural | Implementation vs. spec | Bilateral scan / drift | Associate + guided reimplementation |
| Consequential | Current state vs. future risk | Uncertainty tensor | Guidance (redirect before action) |
| Aleatory | Agent vs. agent | Merge conflict detection | Deliberation + Decision |
| Logical | Invariant vs. invariant | Contradiction detection (5-tier) | Deliberation + ADR |
| Axiological | Implementation vs. goals | Fitness function, goal-drift signal | Human review + ADR revision |
| Temporal | Agent frontier vs. agent frontier | Frontier comparison | Sync barrier |
| Procedural | Agent behavior vs. methodology | Drift detection (access log) | Dynamic CLAUDE.md |

If you are building something that doesn't map to this taxonomy, either you've found a new
divergence type (document it) or you're building something that doesn't belong.

</reconciliation_taxonomy>

<core_abstractions>

## Core Abstractions — Quick Reference

**Datom**: `[entity, attribute, value, transaction, operation]` — an atomic fact.
The entire data model. Nothing else.

**Store**: `(P(D), ∪)` — a grow-only set of datoms. Merges are set union. Never shrinks.

**Transaction**: An entity in the store carrying provenance (who, when, why, causal predecessors).

**Resolution modes** (per-attribute): lattice-resolved, last-writer-wins, or multi-value.

**Frontier**: The set of all datoms known to a specific agent at a specific point.

**Harvest**: End-of-session extraction of un-transacted knowledge into the store.

**Seed**: Start-of-session assembly of relevant knowledge from the store.

**Guidance**: Brief methodology pointer injected into every tool response.

**Fitness function F(S)**: Quantified convergence across coverage, depth, coherence,
completeness, and formality. Target: F(S) → 1.0.

</core_abstractions>

---

<design_decisions>

## Key Design Decisions

These are settled. Do not revisit without finding a formal contradiction (NEG-002).

The full design decision index is in **`docs/design/ADRS.md`**, organized into:
- **Foundational Decisions (FD-001–008)**: Append-only store, EAV, Datalog, content-addressable identity, schema-as-data, per-attribute resolution, self-bootstrap
- **Protocol Decisions (PD-001–004)**: Agent working set W_α / patch branches, provenance typing lattice, crash-recovery model, at-least-once delivery
- **Snapshot & Query Decisions (SQ-001–004)**: Local frontier default, frontier as datom attribute, Datalog frontier extension, stratum safety classification
- **Lifecycle Decisions (LD-001–004)**: Braid as new implementation, manual harvest/seed, disposable conversations, reconciliation taxonomy

Each entry includes rationale, alternatives rejected, and transcript references. Consult
`docs/design/ADRS.md` for the full record before proposing changes to any settled decision.

</design_decisions>

<session_checklist>

## Session Checklist

Use this at the start and end of every session.

### Start of Session
- [ ] Read this AGENTS.md (especially the braid-seed section for prior session context)
- [ ] Or run `braid seed --task "your task"` for fresh context assembly
- [ ] Check `docs/design/FAILURE_MODES.md` for open failure modes that intersect your task
- [ ] Identify the specific task for this session
- [ ] Trace the task to SEED.md section(s)
- [ ] State the plan before executing

### End of Session
- [ ] All new files are complete within their scope (no stubs, no TODOs)
- [ ] All specification elements have IDs, types, traceability, and falsification conditions
- [ ] No hard constraints (C1–C7) violated
- [ ] No negative cases (NEG-001–NEG-008) triggered
- [ ] Any new failure modes discovered during the session recorded in `docs/design/FAILURE_MODES.md`
- [ ] Run `braid harvest --commit` to persist session knowledge
- [ ] Run `braid seed --inject AGENTS.md` to refresh seed for next session

</session_checklist>

---

## Dynamic Store Context

> This section is auto-generated by `braid seed --inject CLAUDE.md`.
> It provides real-time context from the braid datom store.
> Regenerate: `braid seed --inject CLAUDE.md --task "your task"`

<braid-seed>
<!-- Generated by braid. Do not edit manually. Regenerate: braid seed --inject AGENTS.md -->
<!-- Updated: 1773268560 | Store: 4902 datoms, 967 entities -->

### Session Context
Braid: append-only datom store (CRDT merge, content-addressed). 4902 datoms, 967 entities. Codebase: 42980 LOC across 51 .rs files
Key files:
  ddis-braid/crates/braid-kernel/src/query/graph.rs (3984 LOC)
  ddis-braid/crates/braid-kernel/src/seed.rs (3647 LOC)
  ddis-braid/crates/braid-kernel/src/harvest.rs (3246 LOC)
  ddis-braid/crates/braid-kernel/src/guidance.rs (2398 LOC)
  ddis-braid/crates/braid-kernel/src/schema.rs (2347 LOC)
Goal: harvest/seed replaces HARVEST.md. Status: 21 harvests, 33 observations, 21 decisions captured.
Spec: 354 elements, 20 namespaces — BILATERAL(5/10/2) BOOTSTRAP(0/0/1) BUDGET(6/4/2) DELIBERATION(6/4/3) FOUNDATION(0/6) GUIDANCE(11/9/3) HARVEST(9/7/3) INTERFACE(12/11/4) LAYOUT(11/7/5) MERGE(10/7/3) QUERY(24/13/4) RESOLUTION(8/13/3) SCHEMA(9/8/3) SEED(8/7/2) SIGNAL(6/5/3) STORE(16/21/5) SYNC(5/3/2) TRILATERAL(10/6/4) UNCERTAINTY(0/4) VERIFICATION(0/1)
Last session: Session 020: Phase A — braid trace + witness + M(t) fix (3 txns, 1 observations, +1575 datoms, +193 entities)
  - Session entity: :impl/braid-kernel.src.agent_md.inv-store-001
  - Session entity: :impl/braid-kernel.src.merge.inv-merge-010
  - Session entity: :impl/braid-kernel.src.seed.inv-seed-004
  - Session entity: :impl/braid-kernel.src.schema.inv-schema-002
  - Session entity: :impl/braid-kernel.src.query.mod.inv-query-014
  Decided: Session 020 Phase A complete: braid trace scanner (350 LOC), M(t) floor clamp, witness marking. Results: 52 files scanned, 456 refs found, 191 resolved to 111/354 spec entities (28% coverage). 12 spec elements witnessed from test files. F(S) 0.2965 t...
  Changes: branch=main, 2 commits, 10 files (+174/-66)
Prior: Session 019: P1.1 budget pipeline + P3.1 session lifecycle + P4.2 bilateral CLI — Dogfood feedback: (1) Session start/end feels like a major quality-of-life impro...
Prior: P3.1 session lifecycle commands — Session lifecycle commands implemented. braid session start: inject seed + show ...

### Active Constraints
- [?] INV-INTERFACE-009 — Error Recovery Protocol Completeness
- [?] INV-SEED-001 — Seed as Store Projection
- [?] INV-SEED-005 — Demonstration Density
- [?] ADR-BILATERAL-003 — Intent Validation as Periodic Session
- [?] ADR-STORE-012 — Three-Phase Implementation Path
- [?] INV-GUIDANCE-009 — Task Derivation Completeness
- [?] INV-MERGE-002 — Merge Cascade Completeness
- [?] INV-MERGE-009 — Merge Receipt Completeness
- [?] INV-RESOLUTION-007 — Three-Tier Routing Completeness
- [?] INV-SCHEMA-002 — Genesis Completeness

### Recent Entities
- Project context: Stage 0
Core: datom [e,a,v,tx,op]. Store = grow-only set (CRDT merge = set union). 58 distinct attributes in use.
Crates: braid-kernel (store, schema, query, harvest, seed, guidance, merge), braid (CLI).
CLI: braid {init, status, transact, query, harvest, seed, observe, guidance, merge, log, schema}.
Patterns: Store::genesis() for tests. BraidError for errors. EntityId::from_ident(). Value::{String,Keyword,Long,Double}.
Quality: cargo check && cargo clippy --all-targets -- -D warnings && cargo fmt --check && cargo test.
Key attrs: :impl/file(382), :impl/implements(382), :impl/module(382), :exploration/body(33), :exploration/category(33), :exploration/confidence(33), :exploration/content-hash(33), :exploration/maturity(33), :exploration/source(33), :harvest/agent(21), :harvest/candidate-count(21), :harvest/drift-score(21), :exploration/tags(18), :harvest/store-datom-count(12), :harvest/store-entity-count(12)
- Session history (oldest → newest):
  Session 015: Close the harvest/seed gap — make HARVEST.md unnecessary → Seed Orientation redesigned as narrative briefing
  Session 016: Seed quality deep-fix — make harvest/seed replace HARVEST... → Session 016: Deep-fix wave — RC1 UTF-8, RC2 entity body text...
  Session 017: Seed optimization deep dive — prompt-optimization theory ... → Prompt-optimization applied to seed: seed output IS turn 1 o...
  Session 018: Harvest classification fix + directive enrichment → Fix: harvest classify_profile now reads exploration/category...
  Validation session 1 (P1.1) dogfood experience: Seed provided adequate... → Validation session 1 (P1.1) dogfood experience: Seed provide...
  P3.1 session lifecycle commands → Session lifecycle commands implemented. braid session start:...
- Key observations:
  - Validation session 1 (P1.1) dogfood experience: Seed provided adequate orientation for budget work. Key gap: seed doesn't show the attribute vocabular...
  - Session 018 wave 2: inject path cleaned (removed **** empty-label noise), directive dedup (synthesis OR excerpt, not both), completeness gaps reclassi...
  - Directive section improved: decisions carry forward with NEG-002 anchor, telemetry noise filtered (divergence/cycles/staleness), open questions shown ...
  - Fix: harvest classify_profile now reads exploration/category values to correctly classify open-question/design-decision observations as Uncertainty/De...
  - Session 017: Seed optimization deep dive — project context cheat sheet, spec landscape in Orientation, observation deduplication, dynamic observation ...
  - Codebase snapshot feature: harvest now captures LOC by file, key file map, and test count. Seed renders this as a project map in Orientation — incomin...
- :spec/inv-store-003 — Content-Addressable Identity
- :spec/inv-store-001 — Append-Only Immutability
- :spec/inv-merge-001 — Merge Is Set Union
- :spec/inv-store-002 — Strict Transaction Growth
- :spec/inv-harvest-002 — Harvest Provenance Trail
- :spec/inv-schema-001 — Schema-as-Data
- :spec/inv-store-004 — CRDT Merge Commutativity
- :spec/inv-merge-002 — Merge Cascade Completeness
- :spec/inv-schema-002 — Genesis Completeness
- :spec/inv-interface-011 — "CLI Surface as Optimized Prompt"
- ... and 5 more
- :observation/session-020-phase-a-complete-braid-trace-scanner-350-loc — Session 020 Phase A complete: braid trace scanner (350 LOC), M(t) floor clamp, witness marking. Results: 52 files scanned, 456 refs found, 191 resolved to 111/354 spec entities (28% coverage). 12 spec...
- :observation/session-019-four-pillar-s0s1-transition-plan-executed-pha — Session 019: Four-Pillar S0→S1 transition plan executed. Phase 0 complete: 25 beads created with full dependency graph, P3.2 implemented (--commit bypasses crystallization guard at Stage 0, --guard fl...
- :observation/session-012-self-bootstrap-complete-c7-injection-engine — Session 012: Self-bootstrap complete (C7). Injection engine (inject.rs, 500+ LOC, 23 tests, lens laws verified). CLI braid seed --inject CLAUDE.md. First live injection: 560 tokens, 2784 datoms, 680 e...
- :observation/session-lifecycle-commands-implemented-braid-session-start — Session lifecycle commands implemented. braid session start: inject seed + show actionable summary with auto-task continuation. braid session end: harvest + re-inject + git guidance. 150 LOC in sessio...
- :observation/how-should-seed-handle-multi-session-continuity-current-sh — How should seed handle multi-session continuity? Current: shows last 2-3 sessions. Gap: agent loses context from session N-4 and earlier. Possible: session chain summary with diminishing detail.
- :observation/prompt-optimization-applied-to-seed-seed-output-is-turn-1-o — Prompt-optimization applied to seed: seed output IS turn 1 of every conversation — it determines the trajectory basin. Three principles: (1) spec-language activates design-level reasoning substrate (B...
- :observation/checking-phase-a-bug-datalog-evaluator — checking phase A bug: datalog evaluator
- :observation/ux-overhaul-session-011-command-consolidation-1710-complet (11 attrs)
- :observation/phase-b-complete-harvest-warnings-staleness-model-crystal (10 attrs)
- :observation/phase-a-complete-datalog-bug-fixed-observe-command-adapti (10 attrs)
- ... and 3 more

### Open Questions
- [?] How should seed handle multi-session continuity? Current: shows last 2-3 sessions. Gap: agent loses context from session N-4 and earlier. Possible: session chain summary with diminishing detail.

### Next Actions

Decisions (settled, do not relitigate):
  - Session 020 Phase A complete: braid trace scanner (350 LOC), M(t) floor clamp, witness marking. Results: 52 files scanned, 456 refs found, 191 resolved to 111/354 spec entities (28% coverage). 12 spec...

Protocol: observe decisions/questions during work, harvest before ending session.
Quick: braid status | braid observe "..." --category design-decision | braid harvest --commit

### Quick Reference
```bash
braid status                           # Dashboard + next action
braid observe "..." --confidence 0.7    # Capture knowledge
braid harvest --commit                 # End-of-session extraction
braid seed --inject AGENTS.md          # Refresh this section
```
</braid-seed>

---

<transcript_usage>

## Using the Transcripts

The `docs/history/transcripts/` directory contains the complete reasoning history behind every design
decision. The transcripts are large — do not read them whole. Use them surgically:

1. Check `docs/history/transcripts/journal.md` for the summary index
2. Identify which transcript likely contains the reasoning you need
3. Read only the relevant portions
4. For more surgical, fine-grain precision, you can parse the contents of the corresponding `*.txt` files in `docs/history/transcripts/`, each of which corresponds to the equivalently named `*.md` file.

### Transcript Index (from journal.txt)

| Transcript | Topic |
|---|---|
| `01-datomic-rust-crdt-spec-foundation.md` | Algebraic foundations, five axioms, uncertainty tensor, spectral authority |
| `02-datom-store-query-patterns.md` | Datalog query strata, monotonicity analysis, CALM compliance, LIVE indexing |
| `03-agent-protocol-convergence-analysis.md` | Dual-process architecture, multi-agent coordination, protocol gaps |
| `04-datom-protocol-interface-design.md` | Protocol operations, five-layer interface, attention budget decay, **PQ1–PQ4 design decisions** (private datoms/W_α, provenance typing, crash-recovery, delivery semantics) |
| `05-ddis-implementation-roadmap-dynamic-claude-md.md` | Staging model, change management, dynamic CLAUDE.md innovation |
| `06-ddis-seed-document-coherence-verification.md` | Shift from "memory problem" to "coherence verification" |
| `07-ddis-seed-document-finalization.md` | Self-bootstrap commitment, reconciliation taxonomy, final seed |

</transcript_usage>

<existing_codebase_relationship>

## Relationship to Existing Codebase

The existing Go CLI (`../ddis-cli/`) is a 62,500 LOC implementation with:
- 97 invariants (APP-INV-001..097)
- 74 ADRs (APP-ADR-001..074)
- 30 SQLite tables
- 19 validation checks
- 5-tier contradiction engine
- Event-sourcing pipeline (crystallize → materialize → project)
- Bilateral cycle (discover → refine → drift → absorb)

**How to use it**: The existing implementation validates that DDIS concepts work in practice.
Its architecture (parser, storage, events, consistency, materialize, drift, search, witness,
challenge, refine, absorb, discover, triage) shows which abstractions survived contact with
reality. Consult it when the specification leaves something ambiguous.

**How NOT to use it**: Do not port Go code to Rust line-by-line. Do not assume its architecture
is correct for braid. The gap analysis (docs/audits/GAP_ANALYSIS.md) will determine which modules align
with the new specification and which diverge. Let the specification lead, not the legacy code.

</existing_codebase_relationship>

---

<guidance_for_specific_tasks>

## Task-Specific Guidance

### If your task is "Write/Edit the specification" (spec/)

Work through SEED.md section by section. Each namespace has its own file in `spec/`
(see `spec/README.md` for the index). For each section:
1. Extract every implicit claim about system behavior
2. Formalize each claim as an invariant with ID and falsification condition
3. Record every design choice as an ADR with alternatives and rationale
4. Identify bounds on the solution space as negative cases
5. Mark uncertain claims with confidence levels
6. Cross-reference: does any new invariant contradict an existing one?

Organize by namespace: STORE, QUERY, HARVEST, SEED, GUIDANCE, MERGE, DELIBERATION, SIGNAL,
SYNC, BILATERAL, SCHEMA, RESOLUTION, BUDGET, INTERFACE.

The transcripts contain deeper reasoning than the seed. When the seed states a conclusion
without full rationale, check the transcripts for the argument.

### If your task is "Write the implementation guide" (docs/guide/)

Write for the agent that will build the system. Include:
- Stage 0 deliverables with exact CLI command signatures and expected behaviors
- File format specifications (datom serialization, event log format, seed format)
- CLAUDE.md template that the dynamic generator will produce
- Success criteria that are mechanically verifiable, not subjective
- Worked examples: "here is a session transcript showing harvest/seed in action"

### If your task is "Write docs/audits/GAP_ANALYSIS.md"

For each internal package in `../ddis-cli/internal/`:
1. Read the package's Go source
2. Identify what specification elements it implements
3. Categorize: ALIGNED (keep), DIVERGENT (adapt), EXTRA (evaluate), BROKEN (fix), MISSING (build)
4. Note which braid specification elements have no corresponding existing code

### If your task is "Implement Stage 0"

Read `docs/guide/README.md` first. Then:
1. Set up the project structure (Cargo workspace if Rust)
2. Implement the datom store (append-only, content-addressed)
3. Implement transact and query
4. Implement harvest and seed
5. Implement the guidance injection system
6. Implement dynamic CLAUDE.md generation
7. **First act**: transact the specification elements from `spec/` as datoms
8. Verify: the system can check its own specification for contradictions

</guidance_for_specific_tasks>

---

*This document is itself a rough instance of the DDIS methodology: it steers agent behavior through
structured constraints (invariants, negative cases, falsification conditions) rather than through
aspirational process obligations. When the dynamic CLAUDE.md generator exists, this static
document will be replaced by a version that adapts to observed drift patterns. Until then, it
is the seed that keeps every session aligned.*


<!-- bv-agent-instructions-v1 -->

---

## Beads Workflow Integration

This project uses [beads_viewer](https://github.com/Dicklesworthstone/beads_viewer) for issue tracking. Issues are stored in `.beads/` and tracked in git.

### Essential Commands

```bash
# View issues (launches TUI - avoid in automated sessions)
bv

# CLI commands for agents (use these instead)
bd ready              # Show issues ready to work (no blockers)
bd list --status=open # All open issues
bd show <id>          # Full issue details with dependencies
bd create --title="..." --type=task --priority=2
bd update <id> --status=in_progress
bd close <id> --reason="Completed"
bd close <id1> <id2>  # Close multiple issues at once
bd sync               # Commit and push changes
```

### Workflow Pattern

1. **Start**: Run `bd ready` to find actionable work
2. **Claim**: Use `bd update <id> --status=in_progress`
3. **Work**: Implement the task
4. **Complete**: Use `bd close <id>`
5. **Sync**: Always run `bd sync` at session end

### Key Concepts

- **Dependencies**: Issues can block other issues. `bd ready` shows only unblocked work.
- **Priority**: P0=critical, P1=high, P2=medium, P3=low, P4=backlog (use numbers, not words)
- **Types**: task, bug, feature, epic, question, docs
- **Blocking**: `bd dep add <issue> <depends-on>` to add dependencies

### Session Protocol

**Before ending any session, run this checklist:**

```bash
git status              # Check what changed
git add <files>         # Stage code changes
bd sync                 # Commit beads changes
git commit -m "..."     # Commit code
bd sync                 # Commit any new beads changes
git push                # Push to remote
```

### Best Practices

- Check `bd ready` at session start to find available work
- Update status as you work (in_progress → closed)
- Create new issues with `bd create` when you discover tasks
- Use descriptive titles and set appropriate priority/type
- Always `bd sync` before ending session

<!-- end-bv-agent-instructions -->
