# Braid

> Braid is a runtime for formal epistemology — not a software tool.
> It replaces the Go CLI (`../ddis-cli/`) with a system built from
> first principles on `SEED.md`. The Go CLI and its specs are reference
> material — not the codebase you extend.

---

## True North

DDIS is a formal epistemology — how shared knowledge grows and stays coherent
across observers, time, and domains. Braid provides the universal substrate:
append-only datom store, CRDT merge, boundary registry, fitness gradient,
extractor framework, and harvest/seed lifecycle.

**DDIS methodology is braid's first APPLICATION, not its identity** (C8).
The INV/ADR/NEG ontology, 7-component F(S), and witness protocol are one
epistemological policy among many. Others (research, compliance, manufacturing)
run on the same substrate via different policy manifests. The policy is datoms —
at `braid init`, a manifest is transacted and the kernel configures itself.

**One atomic operation at every level**: observe reality, compare to model,
reduce discrepancy. The 8-type divergence taxonomy (SEED.md §6) classifies
every way a model diverges from reality. Three learning loops close the system:
weight calibration (OBSERVER-4), structure discovery (OBSERVER-5), ontology
discovery (OBSERVER-6).

**The steering principle**: Braid is a navigation system for the LLM's pre-existing
knowledge manifold, not a knowledge store. The datom store contains trajectory (where
the agent has been), steering (where to go next), and calibration (steering accuracy).
The LLM builds an in-context model of braid from every token it encounters — commands,
flags, help text, outputs, errors, schema attributes, silences, cross-command patterns.
That model's quality bounds the agent's behavior. Braid is not a tool the LLM uses;
it is a language the LLM speaks. The API must be a coherent formal language that an
in-context learner converges on within 3-5 interactions. Inconsistencies (e.g.
`braid task list` vs `braid observe --list`) are parasitic constraints that degrade
reasoning across all subsequent turns. Command names, error messages, output structure,
and vocabulary must be grammatically, semantically, and structurally uniform across
the entire surface. A CLI response that returns only a receipt wastes the most powerful
steering moment in the interaction.
(Full theory: `docs/design/STEERING_MANIFOLD.md`)

**Substrate test**: "Would this make sense for a React project? A research lab?
A compliance team?" If no, it belongs in the policy layer, not the kernel.
Every change must close a loop, not open one (ADR-FOUNDATION-014).

---

## The One Rule

**Every session must leave the project in a better state than it found it.**

No speculative scaffolding, no aspirational stubs, no "TODO: implement later."
Every artifact committed must be complete within its scope, tested if code,
traceable to `SEED.md`. Reduce scope before reducing quality.

---

## Core Abstractions

| Term | Definition |
|---|---|
| **Datom** | `[entity, attribute, value, transaction, operation]` — atomic fact |
| **Store** | `(P(D), ∪)` — grow-only set of datoms. Merge = set union |
| **Transaction** | Entity carrying provenance (who, when, why, causal predecessors) |
| **Resolution** | Per-attribute: lattice-resolved, last-writer-wins, or multi-value |
| **Frontier** | All datoms known to a specific agent at a specific point |
| **Harvest** | End-of-session extraction of un-transacted knowledge into store |
| **Seed** | Start-of-session assembly of relevant knowledge from store |
| **Guidance** | Methodology pointer injected into tool responses |
| **F(S)** | Fitness: coverage, depth, coherence, completeness, formality. Target: 1.0 |

---

## Hard Constraints

Non-negotiable. Violating any is a defect regardless of other merits.

**C1: Append-only store.** Never delete or mutate. Retractions are new datoms (`op=retract`).

**C2: Identity by content.** Datom = `[e, a, v, tx, op]`. Same fact = same datom.

**C3: Schema-as-data.** Schema defined as datoms. Evolution is a transaction, not a migration.

**C4: CRDT merge = set union.** No heuristics at merge time. Conflict resolution is query-layer.

**C5: Traceability.** Every artifact traces to a spec element traces to a SEED.md goal. Orphans are defects.

**C6: Falsifiability.** Every invariant needs "violated if..." — no falsification condition = not an invariant.

**C7: Self-bootstrap.** Spec elements are the system's first data. The system's first coherence check is its own spec.

**C8: Substrate independence.** The kernel (`braid-kernel`) must not contain logic specific to
any language, test framework, spec format, or methodology — including DDIS itself. All
domain-specific behavior enters through plugin/extractor: registered as datoms, discovered
by query, invoked at runtime. **Test**: "would this make sense for a React project with Jest
and Jira?" If no, it belongs in application-layer, not kernel. **Falsification**: any kernel
function that imports, parses, or assumes a specific ecosystem or methodology.

**C9: Parameter substrate independence.** (INV-FOUNDATION-015) No hardcoded domain-specific
*values* — thresholds, weights, sizes, intervals. C8 = no hardcoded logic; C9 = no hardcoded
numbers. Every parameter follows: bootstrap default (compile-time fallback) → config override
(`:config/*` datom via `get_config`) → self-calibration (OBSERVER-4 learning loop).
**Falsification**: any kernel constant that fails the substrate test and lacks a `get_config`
override path.

**C10: CLI coherence.** Every command, flag, output, error, and help text is a steering
event on the LLM's knowledge manifold. The API must be a learnable formal language
(ADR-CLI-001). Enforcement is braid's own coherence machinery: INV-CLI-001..006 are
datoms in the store, verified by `braid witness`, measured by `braid bilateral`, surfaced
by `braid status`. The `ResourceCommand` trait makes grammar violations uncompilable,
`SteeringError` makes bare errors uncompilable, `CreateResponse` makes receipt-only
responses uncompilable. New CLI surface requires crystallizing the spec element FIRST
(`braid spec create`), then implementing against it, then witnessing it (`braid witness`).
**Falsification**: any INV-CLI invariant without an L2+ witness; any `braid bilateral`
drift on the CLI coherence boundary.
(Theory: `docs/design/STEERING_MANIFOLD.md`)

---

## Critical Failure Modes

**NEG-001: No aspirational stubs.** `unimplemented!()` or `// TODO` = waste. Implement fully
or don't create the file. Partial implementations that compile but don't work are worse than
no implementation.

**NEG-002: No relitigating settled decisions.** Check `docs/design/ADRS.md` before proposing
alternatives to any design choice. ADRs exist to prevent re-derivation. Only revisit on
formal contradiction with another ADR or invariant.

**NEG-009: No "software tool" framing.** Features must generalize beyond software projects —
research labs, compliance teams, manufacturing. DDIS ontology (INV/ADR/NEG) is one policy;
don't hardcode it as the only one.

**NEG-010: No open loops.** Every datum the system creates or receives must connect to a
boundary evaluation, gradient computation, or calibration measurement. Open loops caused
every problem identified in Session 033. (ADR-FOUNDATION-014)

---

## Specification Methodology

Every spec element requires: **ID** (`INV-{NS}-{NNN}` / `ADR-{NS}-{NNN}` / `NEG-{NS}-{NNN}`),
**type**, **traceability** to SEED.md, **falsification condition**, and **uncertainty marker**
where applicable (confidence 0.0–1.0, plus what would resolve it).

```
### INV-STORE-001: Append-Only Immutability
**Traces to**: SEED.md §4 (Design Commitment #2)
**Type**: Invariant
**Statement**: The datom store never deletes or mutates an existing datom.
**Falsification**: Any operation that removes or modifies a datom's [e,a,v,tx,op] tuple.
**Verification**: After any operation sequence, datom count is monotonically non-decreasing
and all previously-observed tuples remain present.
```

---

## Session Protocol

**Start**: Read this file (especially the braid-seed section below for prior session context).
Or run `braid seed --task "your task"` for fresh context assembly.
Trace task to bedrock vision, convergence thesis (ADR-FOUNDATION-014), or specific INV/ADR.
Ask: "Does this close a loop or open one?" and "Would this work for a non-DDIS project?"

**During**: `braid observe "insight" --confidence 0.8` to capture knowledge.
Record failure modes in `docs/design/FAILURE_MODES.md` with FM-NNN IDs.
Document every design decision as an ADR.

**End**:
```bash
braid harvest --task "<task>" --commit   # Persist session knowledge
braid seed --inject AGENTS.md           # Refresh seed for next session
cargo check --all-targets && cargo clippy --all-targets -- -D warnings
cargo fmt --check && cargo test          # Quality gates
# Then: git add, git commit, git push
```

**Verification**: All new files complete (no stubs/TODOs). All spec elements have IDs,
traceability, falsification. No C1–C9 violations. No domain-specific logic in kernel
without going through policy/extractor pattern.

---

## Agent Launch Protocol (INV-TOPOLOGY-003)

Prevents build-lock contention and file overwrites (learned from Session 025).

1. **Commit before launch.** Agents' `cargo fmt` will overwrite uncommitted edits.
2. **Disjoint file sets.** Each agent edits DIFFERENT files. Same file = serialize, not parallelize.
3. **Agents don't build.** Edit only — no `cargo fmt/clippy/test`. Orchestrator verifies after.
4. **No worktrees.** Launch without `isolation: "worktree"`.
5. **Use `-q` flag.** `braid ... -q` suppresses guidance footer. Never `2>/dev/null`.

---

## Codebase Orientation

| Path | Purpose |
|---|---|
| `SEED.md` | Foundational design document — read after this file |
| `spec/README.md` | Master index for 21 namespace specifications |
| `crates/braid-kernel/` | Core: datom store, schema, query, harvest/seed, merge, guidance |
| `crates/braid/` | CLI: init, status, transact, query, harvest, seed, guidance, merge, log |
| `docs/design/ADRS.md` | Settled design decisions with rationale — check before relitigating |
| `docs/design/FAILURE_MODES.md` | Agentic failure mode catalog with acceptance criteria |
| `docs/guide/README.md` | Implementation guide index by namespace |
| `docs/audits/GAP_ANALYSIS.md` | Go CLI vs Braid mapping (ALIGNED/DIVERGENT/EXTRA/BROKEN/MISSING) |
| `docs/history/transcripts/journal.md` | 7 design session transcripts — read surgically, not whole |

**Sibling dirs** (reference only — do not port Go to Rust):
`../ddis-modular/` (meta-standard), `../ddis-cli-spec/` (97 INVs, 74 ADRs),
`../ddis-cli/` (Go impl, ~62.5K LOC). Consult when spec is ambiguous.

---

## Build Notes

`CARGO_TARGET_DIR=/tmp/cargo-target` — must `cp` binary after `cargo build --release`.

Stage 1 is ~85% complete. Stage 0 (spec, guide, gap analysis, kernel, CLI) is done.
Details in SEED.md §10.

---

## Dynamic Store Context

> Auto-generated by `braid seed --inject AGENTS.md`.
> Regenerate: `braid seed --inject AGENTS.md --task "your task"`

<braid-methodology>
<!-- Generated by braid. Do not edit manually. Regenerate: braid seed --inject AGENTS.md -->
<!-- Updated: 1774640198 | Store: 15346 datoms -->

## Methodology Gaps
- 171 current-stage INVs untested → add L2+ witness

## Ceremony Protocol (k*=0.7)
Standard: observe + execute → retroactive crystallize
For known-category bug fixes: execute-first OK if provenance chain exists after commit.

## Session Constraints
- Harvest/Seed lifecycle: NOT YET IMPLEMENTED (spec only)
- R(t) task routing: NOT YET IMPLEMENTED (spec only)

</braid-methodology>

<braid-seed>
<!-- Generated by braid. Do not edit manually. Regenerate: braid seed --inject AGENTS.md -->
<!-- Updated: 1774640198 | Store: 15346 datoms, 2000 entities -->

### Session Context
Braid runtime: observe, harvest, seed. See AGENTS.md for details.
Braid store (append-only, CRDT merge). 15346 datoms, 2000 entities.
Knowledge: 0 harvests, 0 observations, 0 decisions captured.
Spec: 380 elements, 24 namespaces — BILATERAL(5/10/2) BOOTSTRAP(0/0/1) BUDGET(9/7/2) COHERENCE(1/0) DELIBERATION(6/4/3) FOUNDATION(0/6) GUIDANCE(14/9/3) HARVEST(9/7/3) INTERFACE(10/10/4) LAYOUT(11/7/5) MERGE(10/7/3) QUERY(24/13/4) REFLEXIVE(7/1/3) RESOLUTION(8/13/3) SCHEMA(9/8/3) SEED(8/7/2) SIGNAL(6/5/3) STORE(16/21/5) SYNC(5/3/2) TOPOLOGY(1/1/1) TRILATERAL(10/6/4) UNCERTAINTY(0/4) VERIFICATION(0/1) WITNESS(2/1/2)

### Active Constraints
- ADR-BILATERAL-001 — Fitness Function Weights
- ADR-BILATERAL-002 — Divergence Metric as Weighted Boundary Sum
- ADR-BILATERAL-003 — Intent Validation as Periodic Session
- ADR-BILATERAL-004 — Bilateral Authority Principle
- ADR-BILATERAL-005 — Reconciliation Taxonomy — Detect-Classify-Resolve

### Recent Entities
- Project context: new project (no prior sessions)
Core: datom [e,a,v,tx,op]. Store = grow-only set (CRDT merge = set union). 45 distinct attributes in use.
CLI: braid {init, status, transact, query, harvest, seed, observe, guidance, merge, log, schema}.
Workflow: observe → harvest → seed. Use braid observe to capture insights, braid harvest --commit at session end.
- :spec/inv-layout-004 — Merge as Directory Union
- :spec/adr-harvest-007 — Turn-Count Proxy for Context Budget at Stage 0
- :spec/adr-store-016 — ArcSwap MVCC Concurrency Model
- :spec/inv-harvest-005 — Proactive Warning
- :spec/inv-merge-001 — Merge Is Set Union
- :spec/neg-layout-005 — No Index as Source of Truth
- :spec/inv-layout-001 — Content-Addressed File Identity
- :spec/inv-store-003 — Content-Addressable Identity
- :spec/adr-schema-005 (10 attrs)
- :spec/inv-layout-006 (11 attrs)
- ... and 21 more
- :witness/fbw.9fe1d34801951c82 (12 attrs)
- :witness/fbw.cb69e698106e228f (14 attrs)
- :witness/fbw.267cfc9dbe2d7540 (12 attrs)
- :witness/fbw.c55978e8baf66940 (16 attrs)
- :witness/fbw.5194ed9e1214f0a4 (12 attrs)
- :witness/fbw.f2e31799f9c2fe4e (12 attrs)
- :witness/fbw.83d1acf12189e201 (16 attrs)
- :witness/fbw.f1f0e1f1e5b5ee81 (14 attrs)
- :witness/fbw.42d346c21da8e238 (15 attrs)
- :witness/fbw.dd5aa77185b5de84 (13 attrs)
- ... and 5 more

### Next Actions


Protocol: observe → status → observe → harvest --commit | seed --inject AGENTS.md

### Quick Reference
```bash
braid status                           # Dashboard + next action
braid observe "..." --confidence 0.7    # Capture knowledge
braid harvest --commit                 # End-of-session extraction
braid seed --inject AGENTS.md          # Refresh this section
```
</braid-seed>
