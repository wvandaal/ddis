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
│   ├── HARVEST.md                   ← Manual harvest log (session summaries, decisions)
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

**1. Orient.** Read this file. Check `docs/HARVEST.md` for the latest session summary and pending
decisions. Check `docs/design/FAILURE_MODES.md` for open failure modes that may intersect your task.
If a specific task was assigned, locate it.

**2. Plan.** Before writing any code or spec, state what you intend to do and why. Trace the
work back to a section in `SEED.md` or a specific invariant/ADR. If you cannot trace it,
question whether the work should be done.

**3. Execute.** Do the work. Follow the constraints below. Test everything testable. Document
every design decision as an ADR. When you discover a process failure, methodology gap, or
unexpected divergence, record it immediately in `docs/design/FAILURE_MODES.md` with an FM-NNN ID.

**4. Harvest.** Before the session ends, append to `docs/HARVEST.md`:
   - What was accomplished (with file paths)
   - Decisions made (with rationale)
   - Open questions discovered
   - Failure modes discovered (reference FM-NNN IDs from `docs/design/FAILURE_MODES.md`)
   - Recommended next action for the following session

### The Manual Harvest/Seed Discipline

Until the datom store exists and automates this, **practice the methodology by hand**:

- At session start: read `docs/HARVEST.md` to load prior session state (this is your "seed")
- During work: note decisions, discoveries, and uncertainties as you go
- At session end: write a harvest entry in `docs/HARVEST.md` (this is your "harvest")

This is not bookkeeping — it is the methodology itself. The tools will automate it later.
Methodology precedes tooling.

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

**NEG-006: Do not skip the harvest.** Every session must end with a harvest entry in
`docs/HARVEST.md`. A session without a harvest is knowledge lost. This is the single most
important discipline until the tooling automates it.

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
- [ ] Read this CLAUDE.md
- [ ] Read latest entry in docs/HARVEST.md
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
- [ ] Harvest entry appended to docs/HARVEST.md with:
  - What was accomplished
  - Decisions made with rationale
  - Open questions discovered
  - Failure modes discovered (reference FM-NNN IDs)
  - Recommended next action

</session_checklist>

---

## Dynamic Store Context

> This section is auto-generated by `braid seed --inject CLAUDE.md`.
> It provides real-time context from the braid datom store.
> Regenerate: `braid seed --inject CLAUDE.md --task "your task"`

<braid-seed>
<!-- Generated by braid. Do not edit manually. Regenerate: braid seed --inject AGENTS.md -->
<!-- Updated: 1773245094 | Store: 3126 datoms, 755 entities -->

### Session Context
Store: 3126 datoms, 755 entities (+14 since last harvest)
Codebase: 40954 LOC across 49 .rs files
Key files:
  ddis-braid/crates/braid-kernel/src/query/graph.rs (3984 LOC)
  ddis-braid/crates/braid-kernel/src/harvest.rs (3146 LOC)
  ddis-braid/crates/braid-kernel/src/seed.rs (3095 LOC)
  ddis-braid/crates/braid-kernel/src/schema.rs (2347 LOC)
  ddis-braid/crates/braid-kernel/src/guidance.rs (2330 LOC)
  ddis-braid/crates/braid-kernel/src/bilateral.rs (2042 LOC)
  ddis-braid/crates/braid-kernel/src/trilateral.rs (1857 LOC)
  ddis-braid/crates/braid/src/commands/mod.rs (1399 LOC)
  ddis-braid/crates/braid-kernel/tests/cross_namespace.rs (1316 LOC)
  ddis-braid/crates/braid-kernel/src/store.rs (1217 LOC)
Last session: Session 016: Seed quality deep-fix — codebase snapshot for project comprehension (1 txns, 1 observations)
Accomplished:
  - Codebase snapshot feature: harvest now captures LOC by file, key file map, and test count. Seed renders this as a project map in Orientation — incoming agents immediately know where things are.
Decided:
  - Session 016 polish: synthesis directive now carries forward the actual task instead of a monitoring command. Prior session accomplishments strip legacy 'Session entity:' prefix. Accomplishments split compound entries. Git summary includes top modified files for codebase orientation.
  - Codebase snapshot feature: harvest now captures LOC by file, key file map, and test count. Seed renders this as a project map in Orientation — incoming agents immediately know where things are.
  - Session 016: Deep-fix wave — RC1 UTF-8, RC2 entity body text, RC3 char-safe truncation + budget allocator, RC4 synthesis directive placement, E1 noise filter, E4 enriched rationale. E2 session metrics, E3 question carry-forward, E5 prior session depth, E6 semantic grouping.
Changes:
  branch=main, 1 commits, 6 files (+157/-13)
    c4c51ab Session 016: Polish — synthesis directive task carry-forward, hot files, clean display
  Hot files:
    ddis-braid/crates/braid-kernel/src/seed.rs (58 lines changed)
Recommended:
  Next session task: continue: Session 016: Seed quality deep-fix — codebase snapshot for project comprehension
Prior session: Session 016: Seed quality deep-fix — make harvest/seed replace HARVEST.md
  Done: Session 016 polish: synthesis directive now carries forward the actual task instead of a monitoring command. Prior session accomplishments strip legacy 'Session entity:' prefix. Accomplishments split ...
  Decided: Session 016 polish: synthesis directive now carries forward the actual task instead of a monitoring command. Prior session accomplishments strip legacy 'Session entity:' prefix. Accomplishments split ...; Seed Orientation redesigned as narrative briefing: accomplishments, decisions with rationale, open questions, git changes, prior session summary, synthesis directive carry-forward; Session 015: Fixed 10 harvest/seed defects across 6 waves. Category matching (B1), narrative persistence (A1), task-aware constraints (B2/B3), real M(t) (C1), phantom commands (C2), CC-2 suppression (...
Earlier: Session 016: Seed quality deep-fix — make harvest/seed replace HARVEST.md
  Done: Session 016: Deep-fix wave — RC1 UTF-8, RC2 entity body text, RC3 char-safe truncation + budget allocator, RC4 synthesis directive placement, E1 noise filter, E4 enriched rationale. E2 session metrics...
  Decided: Seed Orientation redesigned as narrative briefing: accomplishments, decisions with rationale, open questions, git changes, prior session summary, synthesis directive carry-forward; Session 015: Fixed 10 harvest/seed defects across 6 waves. Category matching (B1), narrative persistence (A1), task-aware constraints (B2/B3), real M(t) (C1), phantom commands (C2), CC-2 suppression (...; Session 016: Deep-fix wave — RC1 UTF-8, RC2 entity body text, RC3 char-safe truncation + budget allocator, RC4 synthesis directive placement, E1 noise filter, E4 enriched rationale. E2 session metrics...

### Active Constraints
- [?] INV-SEED-001 — Seed as Store Projection
- [?] INV-SEED-005 — Demonstration Density
- [?] INV-INTERFACE-009 — Error Recovery Protocol Completeness
- [?] ADR-BILATERAL-003 — Intent Validation as Periodic Session
- [?] ADR-BILATERAL-005 — Reconciliation Taxonomy — Detect-Classify-Resolve
- [?] ADR-BILATERAL-009 — Cross-Project Coherence Deferred
- [?] ADR-FOUNDATION-002 — Manual Harvest/Seed Before Tooling
- [?] ADR-INTERFACE-009 — Staged Alignment Strategy for Existing Codebase
- [?] ADR-QUERY-007 — Projection Pyramid
- [?] ADR-QUERY-010 — Agent-Store Composition — Three Layers

### Recent Entities
- :harvest/session-braid-user-1773245076 — Harvest session for task: Session 016: Seed quality deep-fix — codebase snapshot for project comprehension
- :harvest/session-braid-user-1773244856 — Harvest session for task: Session 016: Seed quality deep-fix — make harvest/seed replace HARVEST.md

### Next Actions

From last harvest:
  Next session task: continue: Session 016: Seed quality deep-fix — codebase snapshot for project comprehension

Store diagnostics:
  1. Connect — Divergence Φ=240.6 with 83 cycles. Structural remediation needed.
     run: braid status --deep --full
     ref: INV-TRILATERAL-001, INV-TRILATERAL-004
  2. Observe — 83 cycles in entity graph. May indicate circular dependencies.
     run: braid status --deep
     ref: INV-TRILATERAL-003
  3. Investigate — 19 observation(s) have staleness > 0.8. Review or re-observe.
     run: braid query --datalog '[:find ?e ?body :where [?e :exploration/body ?body] [?e :exploration/source "braid:observe"]]'
     ref: ADR-HARVEST-005

Quick reference:
  braid status                     # Dashboard + next action
  braid observe "..." --confidence 0.7  # Capture knowledge
  braid harvest --commit           # End-of-session extraction

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
