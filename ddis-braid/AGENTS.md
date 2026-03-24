# Braid

> **Identity**: You are working on **Braid**, the next-generation DDIS implementation.
> Braid replaces the existing Go CLI (`../ddis-cli/`, ~62,500 LOC) with a system
> built from first principles on the foundations established in `SEED.md`.
> The existing Go CLI and its specifications (`../ddis-modular/`, `../ddis-cli-spec/`)
> are reference material — not the codebase you are extending.

---

<purpose>

## What This Project Is

**DDIS is a formal epistemology** — a mathematical framework for how shared knowledge grows,
becomes coherent, and stays coherent across observers, across time, and across domains.

**Braid is a runtime for formal epistemology.** It provides the universal substrate: an
append-only datom store (minimum viable unit of shared belief), CRDT merge (intersubjectivity),
configurable boundary registry (coherence measurement), fitness gradient (optimal inquiry),
extractor framework (domain-specific plugin interface), and harvest/seed lifecycle (cumulative
knowledge across mortal observers).

**DDIS methodology is the first APPLICATION on the braid substrate.** The INV/ADR/NEG ontology,
7-component F(S), witness challenge protocol, and harvest/seed discipline are one epistemological
policy among many. Other policies (scientific research, regulatory compliance, product
development) can run on the same substrate by providing a different policy manifest. DDIS is
braid's first customer, not its identity. (See C8, ADR-FOUNDATION-012.)

**The atomic operation at every level**: observe reality, compare to model, reduce the
discrepancy. This single operation — applied at the level of datoms, boundaries, gradients,
calibration, and policy merge — constitutes a complete learning system. The 8-type divergence
taxonomy (SEED.md §6) enumerates every way a model can diverge from reality. Detecting and
reducing all 8 types converges the model toward truth. (See ADR-FOUNDATION-014.)

**Three learning loops close the system**: weight calibration (OBSERVER-4: predicted vs actual
outcomes adjust boundary weights), structure discovery (OBSERVER-5: temporal coupling reveals
hidden boundaries), ontology discovery (OBSERVER-6: observation clustering reveals emergent
knowledge categories). Together they converge on the optimal coherence model for any domain.

**The bootstrap doesn't need to be optimal.** It needs to be good enough to start the
convergence loop. DDIS is a good initial policy for software development. The calibration
loop discovers if it's wrong and adjusts. The system finds its own optimum empirically.

**The policy is datoms.** At `braid init`, a manifest is transacted: claim types, evidence
types, boundary definitions with weights, anomaly detectors. The kernel reads it and
configures itself. Calibrated policies are transferable via CRDT merge — two teams that
independently learned about their domain can combine their learning through set union.
(See ADR-FOUNDATION-013, INV-FOUNDATION-007.)

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

**0. True North Check.** Before anything else, verify alignment with the bedrock vision:
- Braid is infrastructure for organizational learning (not a software tool)
- The kernel is substrate (universal); DDIS is application (replaceable) — C8
- Every change must close a loop, not open one (ADR-FOUNDATION-014)
- Ask: "Would this make sense for a React project? A research lab? A compliance team?"
- If the answer is no, the change belongs in the policy layer, not the kernel
- If uncertain, read `bedrock-vision.md` in the memory files

**1. Orient.** Read this file. Read the braid-seed section below for prior session context,
or run `braid seed --task "your task"` for a fresh context assembly.
Check `docs/design/FAILURE_MODES.md` for open failure modes that may intersect your task.
If a specific task was assigned, locate it.

**2. Plan.** Before writing any code or spec, state what you intend to do and why. Trace the
work back to the bedrock vision, the convergence thesis (ADR-FOUNDATION-014), or a specific
invariant/ADR. If you cannot trace it, question whether the work should be done.

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

### Stage 1: Budget-Aware Output + Guidance Injection — **85% complete**
### Stage 2: Branching + Deliberation — **50% complete**
### Stage 3: Multi-Agent Coordination — **28% complete**
### Stage 4: Advanced Intelligence — **8% complete**

(Details in SEED.md §10)

### Architectural North Star: Epistemology Runtime (Session 033)

Three proposals implement the substrate/application separation (C8, ADR-FOUNDATION-012):

**Proposal 1: Policy Manifest** (NEXT — prerequisite for everything)
- F(S) weights, boundary definitions, anomaly thresholds become datoms
- `braid init` transacts a policy manifest; kernel reads it dynamically
- BoundaryRegistry becomes the primary fitness path
- DDIS manifest is one configuration, not the only one
- See: ADR-FOUNDATION-013, INV-FOUNDATION-007

**Proposal 2: LLM-Bootstrapped Initialization** (requires Proposal 1)
- `braid init` converses with user about their domain
- LLM compiles natural language → policy manifest datoms
- The conversation IS the first harvest
- Community manifests: ddis.edn, research.edn, compliance.edn

**Proposal 3: Active Observer Daemon** (requires Proposal 1 + CE-7)
- Daemon gains policy-defined watchers (filesystem, git, test runner, MCP)
- Auto-generates datoms through extractor pipeline + COTX routing
- Boundary checks run continuously; interrupts only on anomaly
- Basin A wins by being autonomous, not just invisible

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

**C8: Substrate independence.** The braid kernel (`braid-kernel`) must not contain logic
specific to any programming language, test framework, specification format, or project
methodology — including DDIS itself. All domain-specific behavior enters through the
plugin/extractor pattern: registered as datoms, discovered by query, invoked at runtime.
The DDIS methodology (INV/ADR/NEG ontology, 7-component F(S), witness challenge protocol,
harvest/seed lifecycle) is the first *application* on the braid substrate, not part of the
substrate itself. The test: "would this code make sense if braid managed a React project
with Jest tests and Jira tickets?" If no, it belongs in an application-layer plugin, not
the kernel. Falsification: any kernel function that imports, parses, or assumes a specific
language ecosystem or methodology violates this constraint.

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

**NEG-009: Do not regress to "software tool" framing.** Braid is infrastructure for
organizational learning, not a software development tool. If you find yourself adding
features "for software projects" without considering whether they generalize to research
labs, compliance teams, or manufacturing — stop. The architecture is universal. Every
feature should work for any domain that has claims, evidence, and alignment boundaries.
The DDIS methodology (INV/ADR/NEG) is one policy; do not hardcode it as the only one.

**NEG-010: Do not open loops.** Every piece of information the system creates or receives
must feed back into the coherence model. If you add a feature that produces data but
doesn't connect that data to a boundary evaluation, a gradient computation, or a
calibration measurement — you've opened a loop. Open loops are the root cause of every
problem identified in the Session 033 review. Close them. (ADR-FOUNDATION-014)

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
- [ ] **TRUE NORTH CHECK**: Braid is infrastructure for organizational learning.
      The kernel is substrate (universal). DDIS is application (replaceable via C8).
      Every change must close a loop. Read `bedrock-vision.md` if uncertain.
- [ ] Read this AGENTS.md (especially the braid-seed section for prior session context)
- [ ] Or run `braid seed --task "your task"` for fresh context assembly
- [ ] Identify the specific task for this session
- [ ] Trace the task to the bedrock vision, convergence thesis, or specific INV/ADR
- [ ] Ask: "Does this close a loop or open one?" (ADR-FOUNDATION-014)
- [ ] Ask: "Would this work for a React project?" (INV-FOUNDATION-006 / C8)
- [ ] State the plan before executing

### End of Session
- [ ] All new files are complete within their scope (no stubs, no TODOs)
- [ ] All specification elements have IDs, types, traceability, and falsification conditions
- [ ] No hard constraints (C1–C8) violated — **especially C8 (substrate independence)**
- [ ] No negative cases (NEG-001–NEG-008) triggered
- [ ] No domain-specific logic added to kernel without going through policy/extractor pattern
- [ ] Run `braid harvest --commit` to persist session knowledge
- [ ] Run `braid seed --inject AGENTS.md` to refresh seed for next session

</session_checklist>

---

<agent_launch_protocol>

## Agent Launch Protocol (INV-TOPOLOGY-003)

When launching parallel subagents, follow these rules to prevent build-lock
contention and file overwrites (learned from Session 025, 3 agent conflicts):

**Rule 1: Commit before launch.** Always `git commit` your changes before
launching agents. Agents' `cargo fmt` runs will overwrite uncommitted edits.

**Rule 2: Disjoint file sets.** Each agent edits DIFFERENT files. If two tasks
touch the same file, serialize them (don't parallelize). Check task descriptions
for `FILE:` markers to determine file sets.

**Rule 3: Agents don't build.** Agents EDIT ONLY — no `cargo fmt`, `cargo clippy`,
`cargo test`. The orchestrator (you) verifies once after all agents complete.
If you must have agents verify, give each a separate `CARGO_TARGET_DIR`.

**Rule 4: No worktrees.** This project does not use git worktrees. Launch agents
without `isolation: "worktree"`.

**Rule 5: Use `-q` flag.** Agents should use `braid ... -q` to suppress the
guidance footer without hiding errors. Never use `2>/dev/null`.

</agent_launch_protocol>

---

## Dynamic Store Context

> This section is auto-generated by `braid seed --inject CLAUDE.md`.
> It provides real-time context from the braid datom store.
> Regenerate: `braid seed --inject CLAUDE.md --task "your task"`

<braid-methodology>
<!-- Generated by braid. Do not edit manually. Regenerate: braid seed --inject AGENTS.md -->
<!-- Updated: 1774316928 | Store: 72865 datoms -->

## Methodology Gaps
- 74 observations with uncrystallized spec IDs → braid spec create
- 15 tasks with unresolved spec refs → crystallize first
- 243 current-stage INVs untested → add L2+ witness
- concentration: 4 traces in BILATERAL — Review: braid task search INV-BILATERAL
- concentration: 4 traces in FOUNDATION — Review: braid task search INV-FOUNDATION
- concentration: 4 traces in STORE — Review: braid task search INV-STORE

## Ceremony Protocol (k*=1.0)
Standard: observe + execute → retroactive crystallize
For known-category bug fixes: execute-first OK if provenance chain exists after commit.

## Next Actions (R(t) pre-computed)
1. "RDI-5: Precedent query — find_resolution_precedent(store, " (impact=0.72) → braid go t-d9536d87
2. "FEGH-1: generate_bridge_hypotheses — the free energy gradi" (impact=0.28) → braid go t-e656e270
3. "ATT-2-IMPL: Hebbian feedback from verbose-request detection." (impact=0.28) → braid go t-065fdbaa

</braid-methodology>

<braid-seed>
<!-- Generated by braid. Do not edit manually. Regenerate: braid seed --inject AGENTS.md -->
<!-- Updated: 1774316928 | Store: 72865 datoms, 7144 entities -->

### Session Context

### Next Actions


Next actions:
  1. Work — R(t) top: "RDI-5: Precedent query — find_resolution_precedent(store, attribute) -> Vec<Decision> for case law retrieval. BACKGROUND: The case law system enables querying past resolution outcomes for a given attribute. Uses AVET index on :resolution/attribute for O(1) lookup, then follows :resolution/decision Ref to load Decision entities. Results sorted by stability_score descending, then tx descending. The function is dual-purpose: (1) inform human reviewers with conflict history, (2) provide data for mode escalation signals. APPROACH: (1) Implement find_resolution_precedent(store, attribute) -> Vec<(EntityId, DecisionMethod, Value, TxId)>. Uses store.avet_lookup(:resolution/attribute, attribute_keyword) to find all resolution entities, then follows :resolution/decision Ref to load Decision details. (2) Add precedent_summary(store, attribute) -> PrecedentSummary struct with total_count, mode_distribution: BTreeMap<DecisionMethod, usize>, override_count, last_resolution_tx. (3) Wire into guidance: when R(t) computes impact for a task, attributes with high conflict history get a contention_boost. ACCEPTANCE: (A) Empty store returns empty vec. (B) After 5 resolutions of same attribute, returns all 5 sorted correctly. (C) PrecedentSummary mode_distribution sums to total_count. (D) AVET index is used (not linear scan). DEPENDS-ON: RDI-4. FILE: crates/braid-kernel/src/resolution.rs" (impact=0.72) — t-d9536d87
     run: braid go t-d9536d87

Protocol: observe → status → observe → harvest --commit | seed --inject AGENTS.md

Active intentions (2 tasks):
  [t-d9536d87] RDI-5: Precedent query — find_resolution_precedent(store, attribute) -> Vec<Decision> for case law retrieval. BACKGROUND: The case law system enables querying past resolution outcomes for a given attribute. Uses AVET index on :resolution/attribute for O(1) lookup, then follows :resolution/decision Ref to load Decision entities. Results sorted by stability_score descending, then tx descending. The function is dual-purpose: (1) inform human reviewers with conflict history, (2) provide data for mode escalation signals. APPROACH: (1) Implement find_resolution_precedent(store, attribute) -> Vec<(EntityId, DecisionMethod, Value, TxId)>. Uses store.avet_lookup(:resolution/attribute, attribute_keyword) to find all resolution entities, then follows :resolution/decision Ref to load Decision details. (2) Add precedent_summary(store, attribute) -> PrecedentSummary struct with total_count, mode_distribution: BTreeMap<DecisionMethod, usize>, override_count, last_resolution_tx. (3) Wire into guidance: when R(t) computes impact for a task, attributes with high conflict history get a contention_boost. ACCEPTANCE: (A) Empty store returns empty vec. (B) After 5 resolutions of same attribute, returns all 5 sorted correctly. (C) PrecedentSummary mode_distribution sums to total_count. (D) AVET index is used (not linear scan). DEPENDS-ON: RDI-4. FILE: crates/braid-kernel/src/resolution.rs (task, P2/medium, in-progress)
    Depends on: RDI-4: Wire resolution-to-decision into resolve_with_trail + cascade_full. BACKGROUND: resolve_with_trail(conflict, schema) -> ResolutionRecord already produces resolution datoms via conflict_to_datoms(). The bridge requires calling resolution_to_decision() and appending the Decision datoms to the same transaction. cascade_full() in merge.rs orchestrates the post-merge cascade; step 1 calls detect_conflicts + resolve, which should now also produce Decision entities. The key invariant: the Resolution datoms and Decision datoms MUST be in the same transaction (atomicity). APPROACH: (1) Extend resolve_with_trail to also return Decision datoms via resolution_to_decision(). (2) Extend conflict_to_datoms to include Decision+Position datoms. (3) Wire :resolution/decision Ref from the resolution entity to the Decision entity. (4) In cascade_full(), ensure the Decision datoms are included in the cascade output. ACCEPTANCE: (A) resolve_with_trail output includes Decision datoms. (B) Resolution entity has :resolution/decision Ref to Decision entity. (C) cascade_full includes Decision datoms in output. (D) All datoms in same tx (atomicity). DEPENDS-ON: RDI-3. FILE: crates/braid-kernel/src/resolution.rs + crates/braid-kernel/src/merge.rs (t-e401eaa4)
    Blocks: RDI-6: Mode escalation signals — detect override patterns and recommend resolution mode changes. BACKGROUND: When humans or higher-authority agents override automated resolutions, the pattern indicates the current mode is inappropriate. The escalation_signal function analyzes the precedent base for a given attribute and fires when the override rate exceeds 0.3. This is the self-tuning feedback loop: resolution→Decision, override→counter-Decision, detection→escalation signal→guidance gap→mode change recommendation. APPROACH: (1) Add :resolution/override-count (Long, One) to schema. (2) Implement escalation_signal(store, attribute) -> Option<EscalationSignal>. EscalationSignal: attribute, current_mode, recommended_mode, override_count, total_count, confidence. (3) Recommendation logic: override_rate > 0.3 AND current=LWW → Lattice; override_rate > 0.5 AND current=Lattice → Deliberation; override_rate < 0.1 → downgrade to LWW. (4) Wire into methodology_gaps() as a new gap category: resolution_escalation_count. (5) Wire into braid status: show 'resolution: N attributes need mode review' when escalation signals exist. ACCEPTANCE: (A) No overrides → no signal. (B) 4/10 overrides on LWW attribute → EscalationSignal with recommended=Lattice. (C) 6/10 overrides on Lattice → recommended=Deliberation. (D) 0/10 overrides → signal to downgrade. (E) Shows in braid status. DEPENDS-ON: RDI-5. FILE: crates/braid-kernel/src/resolution.rs + crates/braid-kernel/src/guidance.rs + crates/braid/src/commands/status.rs (t-c9645957)
  [t-eeaf0160] TOPO-CALM: Implement CALM classification of task phases — Tier M (parallel) vs Tier NM (sequential barrier) (ADR-TOPOLOGY-002, INV-TOPOLOGY-006). BACKGROUND: The CALM theorem (Consistency As Logical Monotonicity) partitions operations: monotonic operations can execute without coordination (Tier M), non-monotonic operations require sync barriers (Tier NM). In the density matrix framework: Tier M = near-zero eigenvalues (independent work), Tier NM = strong eigenvalues (coupled work). For our agent coordination: editing code is Tier M (parallel, each agent edits different files), verification is Tier NM (cargo test/clippy requires all edits complete, sequential barrier). The phase plan is: Phase 1 (Tier M) = all agents edit in parallel, Phase 2 (Tier NM) = barrier (git commit + merge + cargo test), Phase 3 (Tier M) = continue with updated coupling. APPROACH: (1) Define CalmTier enum: MonotonicParallel, NonMonotonicBarrier. (2) classify_task_phase(task: &TaskSummary) -> CalmTier. Tasks involving only file edits = Tier M. Tasks involving verification, merge, schema change = Tier NM. (3) phase_plan(partition: &[Vec<EntityId>], tiers: &BTreeMap<EntityId, CalmTier>) -> Vec<Phase>. Groups consecutive Tier M tasks into parallel phases, inserts barriers before Tier NM tasks. ACCEPTANCE: (A) Pure edit tasks classified as Tier M. (B) Tasks with spec-refs to INV-MERGE-* classified as Tier NM. (C) Phase plan alternates M and NM phases. DEPENDS-ON: t-21af64fc (TOPO-SPECTRAL). FILE: crates/braid-kernel/src/topology.rs (task, P2/medium, closed)
    Depends on: TOPO-SPECTRAL: Implement spectral topology selection — Fiedler partition, Cheeger quality, topology pattern classification (INV-TOPOLOGY-005, ADR-TOPOLOGY-004 middle-end). BACKGROUND: The eigenstructure of rho_C directly determines the optimal topology pattern. The spectral gap Delta = lambda_1 - lambda_2 indicates how dominant the primary coupling cluster is. The Fiedler vector (2nd eigenvector of graph Laplacian L = D - C) gives the optimal binary partition. Recursive Fiedler partitioning gives the full cluster hierarchy. The Cheeger constant h(G) >= lambda_2/2 bounds partition quality. We already have fiedler(), cheeger(), symmetric_eigen_decomposition() in query/graph.rs. APPROACH: (1) Define TopologyPattern enum: Solo, Mesh, Star(hub), Pipeline, Hybrid(clusters). (2) select_topology(analysis: &CouplingAnalysis, agent_count: usize) -> TopologyPattern. Uses intrinsic spectral metrics: if p > 0.8 -> Parallel, if 0.3 < p <= 0.8 -> Hybrid (Fiedler partition), if p <= 0.3 -> Sequential/Star. (3) spectral_partition(coupling: &DenseMatrix, k: usize) -> Vec<Vec<usize>>. Recursive Fiedler bisection into k groups. (4) partition_quality(partition, coupling) -> f64 using Cheeger-like metric. ACCEPTANCE: (A) Identity coupling matrix -> Solo topology. (B) Block-diagonal coupling -> Hybrid with matching cluster count. (C) Fully coupled -> Mesh. (D) select_topology is deterministic (INV-TOPOLOGY-005). DEPENDS-ON: t-3f8df462 (TOPO-DENSITY). FILE: crates/braid-kernel/src/topology.rs (t-21af64fc)
    Blocks: TOPO-PLAN: Implement TopologyPlan struct and compute_plan() — the full compilation back-end (ADR-TOPOLOGY-004 back-end, INV-TOPOLOGY-005). BACKGROUND: This is the back-end of the topology compilation pipeline. It assembles all previous computations (coupling density matrix, spectral partition, CALM classification) into a complete execution plan. The plan includes per-agent task assignments, file sets (verified disjoint), seed commands, CARGO_TARGET_DIR assignments, phase ordering, and estimated F(T). The plan is a pure data structure — no IO, no agent management. It can be serialized to JSON for external orchestrators or displayed as human-readable text. APPROACH: (1) TopologyPlan struct with: agent_count, agents: Vec<AgentAssignment>, phases: Vec<Phase>, coupling_analysis: CouplingAnalysis, estimated_ft: f64. (2) AgentAssignment struct: agent_id: String, tasks: Vec<EntityId>, files: BTreeSet<String>, seed_command: String, cargo_target_dir: String, estimated_impact: f64. (3) compute_plan(store: &Store, agent_count: usize) -> TopologyPlan. Orchestrates: ready_task_files -> composite_coupling -> density_matrix -> spectral_partition -> calm_classify -> balance_assignment -> emit_plan. (4) balance_assignment: greedy bin-packing assigns highest-impact unassigned task to agent with lowest cumulative impact, subject to coupling constraints (coupled tasks to same agent). Uses R(t) impact scores from compute_routing. ACCEPTANCE: (A) All tasks assigned to exactly one agent. (B) File sets across agents are disjoint (no shared files). (C) Coupled tasks (rho_C > 0) assigned to same agent. (D) Impact variance across agents < 0.5 * mean_impact (balanced). (E) Plan is deterministic (INV-TOPOLOGY-005). DEPENDS-ON: t-eeaf0160 (TOPO-CALM), t-21af64fc (TOPO-SPECTRAL). FILE: crates/braid-kernel/src/topology.rs (t-57c4333f)
[Note: Intention context exhausts budget — other sections compressed]

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
