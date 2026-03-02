# DDIS Project — Comprehensive Onboarding Guide

## What Is DDIS?

**Drift-Detecting Involutory Specification** — a methodology and toolchain for writing formal, machine-verifiable specifications that maintain a *bilateral* relationship with their implementations. The key insight: specification and implementation should be a two-way discourse, not a one-way decree. The spec validates the code, and the code feeds back into the spec.

The system is self-bootstrapping: DDIS is specified *in DDIS*, and the CLI tool that enforces the spec is itself *specified by* DDIS. There are three layers of specification, each eating its own tail.

---

## What Was Just Done (2026-03-01 Reorganization)

Two commits executed a planned cleanup:

### Commit `59b0058`: Project reorganization
**Problem**: The root directory had 13 non-obvious files — RALPH loop scripts, duplicate monolith specs, historical design docs — creating confusion about what was active vs. historical.

**What changed**:
- **Moved 22 files** into organized locations (RALPH scripts → `ralph/`, design docs → `docs/design/`, reference docs → `docs/reference/`, superseded audits → `.ddis/specs/audit-archive/`)
- **Removed 13 files**: byte-identical duplicates (`ddis_standard.md`, `ddis_final.md`), empty log files, orphaned beads archive from `ddis-evolution/`, an empty constitution placeholder
- **Net**: Root went from 13 non-obvious files → 4 clean files (README, AGENTS, CLAUDE, .gitignore) + 5 clearly-named directories
- **Zero functional impact**: `ddis validate`/`drift`/`coverage` all pass unchanged

### Commit `2846737`: Absorb audit chapters into spec
**Problem**: 9 chapters across 5 spec modules were incorrectly packaged as standalone "Cleanroom Audit" chapters during earlier audit rounds. These contained legitimate invariants/ADRs that belonged in the main spec structure.

**What changed**:
- Redistributed content from audit chapters into natural spec locations (e.g., event-sourcing audit chapters → schema/fold/diff/snapshot sections)
- 11 invariants and 5 ADRs preserved, 0 content lost
- Validation: 18/19 pass (Check 11 pre-existing warning), 100% coverage, 0 drift

---

## Directory Structure — The Complete Map

```
/data/projects/ddis/
│
├── README.md                              76 KB — Project overview, CLI reference, format spec
├── AGENTS.md                             887 B  — Agent instructions (canonical)
├── CLAUDE.md                             symlink → AGENTS.md
├── .gitignore                            862 B
├── PROPOSED_FILE_REORGANIZATION_PLAN.md   37 KB — Reorganization rationale (DRAFT status)
│
│   ┌──────────────────────────────────────────────────────────────┐
│   │  THE THREE SPECIFICATION LAYERS (most important to understand) │
│   └──────────────────────────────────────────────────────────────┘
│
├── ddis-modular/          ← LAYER 1: THE META-STANDARD
│   │                        "How to write a DDIS spec" — written in DDIS format
│   ├── manifest.yaml                8.1 KB — Top-level manifest (no parent_spec)
│   ├── manifest.ddis.db             2.5 MB — Indexed SQLite DB
│   ├── constitution/
│   │   └── system.md               36 KB  — State model, transitions, glossary
│   └── modules/
│       ├── core-standard.md         49 KB  — Core rules: INV-001..010, gates, causal chain
│       ├── element-specifications.md 54 KB — Spec element types: INV, ADR, gate, negspec
│       ├── modularization.md        42 KB  — Modularization protocol: INV-011..016
│       ├── guidance-operations.md   68 KB  — Discovery, context bundles: INV-017..021
│       └── drift-management.md      30 KB  — Drift detection: INV-021..023
│
├── ddis-cli-spec/         ← LAYER 2: THE CLI SPECIFICATION
│   │                        "What the ddis tool must do" — inherits from ddis-modular
│   ├── manifest.yaml               27 KB  — parent_spec → ddis-modular
│   ├── manifest.ddis.db            20 MB  — Full index (820 sections, 1356 xrefs)
│   ├── constitution/
│   │   └── system.md               92 KB  — 8-tuple state machine, 22 commands, all declarations
│   └── modules/                    9 domain modules, 97 INVs, 74 ADRs
│       ├── parse-pipeline.md        46 KB  — Parsing: APP-INV-001..004
│       ├── search-intelligence.md   56 KB  — Search: APP-INV-005..009
│       ├── query-validation.md      63 KB  — Validation: APP-INV-010..017
│       ├── code-bridge.md           76 KB  — Annotations/drift: APP-INV-018..027
│       ├── auto-prompting.md       135 KB  — LLM guidance: APP-INV-028..042
│       ├── lifecycle-ops.md         97 KB  — Bilateral cycle: APP-INV-041..055
│       ├── workspace-ops.md         54 KB  — Workspace: APP-INV-056..070
│       ├── event-sourcing.md        97 KB  — Events/fold/diff: APP-INV-071..097
│       └── triage-workflow.md       58 KB  — Fitness function, issue management
│
├── ddis-cli/              ← LAYER 3: THE IMPLEMENTATION
│   │                        Go CLI implementing the spec above
│   │                        238 .go files, ~62,500 LOC (including ~12,000 test LOC)
│   ├── cmd/ddis/main.go            — Entry point (delegates to internal/cli)
│   ├── bin/ddis                     — Compiled binary (24 MB)
│   ├── go.mod                       — Module: github.com/wvandaal/ddis
│   ├── Makefile                     — Build/test/install targets
│   │
│   ├── internal/cli/               ← COMMAND LAYER (54 files, ~10,000 LOC)
│   │   │  Every ddis subcommand has its own file:
│   │   ├── root.go                  — Cobra root command, --db flag, global wiring
│   │   ├── parse.go                 — markdown spec → SQLite index
│   │   ├── validate.go              — 19 mechanical validation checks
│   │   ├── coverage.go              — Completeness dashboard
│   │   ├── drift.go                 — Spec↔impl drift measurement
│   │   ├── crystallize.go           — Emit events to JSONL (event-only path)
│   │   ├── materialize.go           — Fold event log → SQLite
│   │   ├── project.go               — SQLite → markdown projection
│   │   ├── discover.go              — Conversational discovery + thread topology
│   │   ├── refine.go                — Iterative spec improvement
│   │   ├── absorb.go                — Merge impl patterns → spec
│   │   ├── witness.go               — Invariant proof receipts (4 levels)
│   │   ├── challenge.go             — 5-level stress testing
│   │   ├── contradict.go            — 5-tier contradiction (graph→SAT→Z3→LLM)
│   │   ├── search.go                — BM25 + LSI + PageRank via RRF
│   │   ├── context.go               — 9-signal LLM context bundle
│   │   ├── triage.go                — Automated triage with F(S) fitness
│   │   ├── next.go                  — Ranked next-action recommendation
│   │   ├── issue.go                 — File/triage/close/list spec issues
│   │   ├── scan.go                  — Extract code annotations
│   │   ├── snapshot.go              — Event stream snapshots
│   │   ├── bisect.go / blame.go / replay.go  — Event stream debugging
│   │   ├── history.go               — Unified timeline
│   │   ├── diff.go / impact.go / cascade.go  — Change analysis
│   │   ├── skeleton.go              — Template scaffold generator
│   │   ├── init.go / spec.go        — Workspace lifecycle
│   │   └── ... (patch, rename, bundle, implorder, checklist, progress, state, etc.)
│   │
│   ├── internal/storage/           ← DATA LAYER (30 SQLite tables)
│   │   ├── schema.go                — Table definitions
│   │   ├── db.go                    — Open/close, OpenExisting(), pragma setup
│   │   ├── insert.go                — All INSERT operations (upsert-aware)
│   │   ├── queries.go               — All SELECT queries (43 KB, domain-aware)
│   │   └── models.go                — Go structs for DB entities
│   │
│   ├── internal/parser/            ← SPEC PARSER (17 files)
│   │   │  Parses markdown → structured elements (INV, ADR, sections, gates, etc.)
│   │   ├── document.go / manifest.go / patterns.go
│   │   ├── invariants.go / adrs.go / sections.go / xref.go
│   │   └── gates.go / negspecs.go / examples.go / glossary.go / ...
│   │
│   ├── internal/consistency/       ← 5-TIER CONTRADICTION ENGINE
│   │   ├── graph.go                 — Tier 1: graph-based
│   │   ├── sat.go                   — Tier 3: SAT/DPLL propositional
│   │   ├── smt.go                   — Tier 5: Z3 subprocess (SMT-LIB2)
│   │   ├── heuristic.go / semantic.go  — Tier 4: heuristic+semantic
│   │   └── llm.go                   — Tier 6: LLM-as-judge pairwise
│   │
│   ├── internal/materialize/       ← EVENT-SOURCING ENGINE
│   │   ├── fold.go                  — FoldWithProcessors, stream processors
│   │   ├── diff.go                  — StructuralDiff + StateHash
│   │   ├── processors.go            — Built-in: validation, consistency, drift
│   │   └── snapshot.go              — Create/load/verify/prune snapshots
│   │
│   ├── internal/events/            — Event model, JSONL stream I/O
│   ├── internal/drift/             — Spec↔impl drift analysis + remediation
│   ├── internal/search/            — Hybrid search (BM25+LSI+PageRank, RRF fusion)
│   ├── internal/witness/           — Proof receipts (4-level: assertion→test→formal→review)
│   ├── internal/challenge/         — 5-level invariant stress testing
│   ├── internal/refine/            — Iterative spec improvement with LLM judge
│   ├── internal/absorb/            — Code→spec reconciliation
│   ├── internal/discover/          — Discovery session + thread management
│   ├── internal/triage/            — F(S) fitness function (6 quality signals)
│   ├── internal/llm/               — LLM provider abstraction (Anthropic, net/http)
│   └── ... (27 more packages: annotate, bundle, cascade, causal, coverage,
│            diff, discovery, exemplar, impact, implorder, oplog, parser,
│            process, progress, projector, query, renderer, skeleton,
│            checklist, state, workspace, autoprompt, validator)
│   │
│   └── tests/                      ← INTEGRATION & BEHAVIORAL TESTS
│       ├── invariant_behavioral_test.go  — 150 KB, covers all 97 invariants
│       ├── pipeline_integration_test.go  — E2E pipeline tests
│       ├── roundtrip_test.go             — Event→materialize→project round-trips
│       ├── triage_test.go                — Triage workflow tests
│       └── ... (23 more test files covering every domain)
│
│   ┌──────────────────────────────────────────────────────────────┐
│   │  SUPPORTING DIRECTORIES                                       │
│   └──────────────────────────────────────────────────────────────┘
│
├── ddis-evolution/        ← HISTORICAL ARCHIVE
│   │                        Version checkpoints and constitution history
│   ├── versions/
│   │   ├── ddis_v0.md .. ddis_final.md      — 4 monolith snapshots (166-209 KB)
│   │   └── v0/, v1/, v2/                    — Modular reconstructions of each version
│   ├── constitution_versions/
│   │   ├── constitution_v0.md               — Original flat constitution
│   │   └── constitution_v1.md               — First modular constitution
│   └── structural_assessment.json           — One-time modularization planning
│
├── docs/                  ← ORGANIZED DOCUMENTATION (new as of 2026-03-01)
│   ├── design/                              — Active architecture documents
│   │   ├── cli-plan.md                      — Original CLI implementation plan
│   │   ├── event-sourcing-architecture.md   — Event-sourcing blueprint
│   │   ├── event-stream-design-audit.md     — Architectural debt analysis
│   │   ├── implementation-prompt.md         — Next implementation directions
│   │   ├── feature-discovery-skill.md       — Feature discovery process
│   │   ├── tooling-exploration.md           — Tool gap analysis
│   │   └── workflow-witness-plan.md         — Methodology verification framework
│   └── reference/                           — Stable reference documents
│       ├── modularization-protocol.md       — Full modularization protocol
│       ├── evaluation-v17.md                — Evaluation rubric
│       └── progress-review-2026-02-24.md    — Strategic assessment
│
├── ralph/                 ← RALPH IMPROVEMENT LOOP TOOLCHAIN
│   │                        Recursive Automated LLM-Powered Helper
│   ├── ddis_ralph_loop.sh                   — 60 KB main automation driver
│   ├── ddis_assemble.sh                     — Assembles modular spec into monolith
│   ├── ddis_validate.sh                     — Validates assembled spec
│   ├── README.md                            — RALPH documentation
│   ├── improvement_strategy.md              — Convergence analysis
│   ├── kickoff_prompt.md                    — One-time kickoff artifact
│   └── judgments/
│       ├── judgment_v1.json                 — "stop_converged" verdict
│       └── judgment_v2.json                 — Confirmation judgment
│
├── ddis-braid/            ← PLACEHOLDER (empty, not yet populated)
│
│   ┌──────────────────────────────────────────────────────────────┐
│   │  DOT-DIRECTORIES (tooling state)                              │
│   └──────────────────────────────────────────────────────────────┘
│
├── .ddis/                 ← DDIS RUNTIME ARTIFACTS (dog-fooded)
│   ├── specs/                               — Reports, audits, strategy docs
│   │   ├── cleanroom-audit-2026-03-01.md    — Latest cleanroom audit
│   │   ├── cleanroom-audit-remediation-plan.md
│   │   ├── CLEANROOM_AUDIT_V2_2026-02-28.md — Authoritative V2 audit
│   │   ├── NEXT_STEPS_UNIVERSALITY_2026-02-28.md
│   │   ├── RECOMMENDATION_MCP_PROTOCOL_SERVER_2026-03-01.md
│   │   ├── UNIVERSALITY_FIELD_REPORT_2026-02-28.md
│   │   ├── GAP_ANALYSIS_2026-02-27.md
│   │   ├── CEREMONIAL_VS_LOADBEARING_TOOL_USAGE_2026-02-28.md
│   │   └── audit-archive/                   — Superseded audit rounds (R1, R2, R3)
│   └── events/                              — JSONL event streams
│       ├── stream-1.jsonl / stream-2.jsonl  — Primary/secondary event streams
│       └── threads.jsonl                    — Discovery thread index
│
├── .beads/                ← ISSUE TRACKING (br tool)
├── .cass/                 ← SESSION HISTORY + PLAYBOOK RULES
├── .claude/               ← CLAUDE CODE PROJECT SETTINGS
├── .ms/                   ← SKILL MANAGER (gitignored, has own internal git)
└── .vscode/               ← IDE SETTINGS
```

---

## Key Concepts for a New Developer

### The Bilateral Cycle
The four self-reinforcing loops that form the core of DDIS:
1. **`ddis discover`** — idea → spec (exploration, thread-based conversation)
2. **`ddis refine`** — spec → improved spec (iterative quality improvement)
3. **`ddis drift`** — spec ↔ impl (detect divergence, remediate)
4. **`ddis absorb`** — impl → spec (merge implementation patterns back)

### The Event-Sourcing Architecture
The CLI is built on an event-sourcing model:
- **`ddis crystallize`** emits events to JSONL streams
- **`ddis materialize`** folds event streams → SQLite index (the "projection")
- **`ddis project`** renders SQLite → markdown
- The older **`ddis parse`** path (markdown → SQLite directly) still works but is being deprecated in favor of the event-first pipeline

### Quality Metrics
- **F(S)** — Fitness function computed by `ddis triage` (6 normalized quality signals)
- **V(S)** — Drift score (0 = perfect alignment)
- **97 invariants** (APP-INV-001..097) — formal properties the CLI must maintain
- **74 ADRs** (APP-ADR-001..074) — architectural decisions with rationale
- **19 validation checks** — mechanical conformance checks
- **5-tier contradiction detection** — graph → SAT/DPLL → Z3/SMT → heuristic/semantic → LLM-as-judge
- **4-level witness hierarchy** — assertion → test-backed → formal → reviewed

### Where to Start if Refactoring

| Goal | Start Here |
|------|-----------|
| Understand the spec format | `ddis-modular/modules/core-standard.md` |
| Understand what the CLI does | `ddis-cli-spec/constitution/system.md` (state machine + 22 commands) |
| Understand the Go code structure | `ddis-cli/internal/cli/root.go` (Cobra wiring) → individual command files |
| Understand the data model | `ddis-cli/internal/storage/schema.go` (30 tables) |
| Understand the parser | `ddis-cli/internal/parser/document.go` + `patterns.go` |
| Run the tests | `cd ddis-cli && go test ./...` |
| Check current spec health | `ddis validate manifest.ddis.db --json` |
| See what needs work | `ddis triage manifest.ddis.db` |
| Find architectural debt | `docs/design/event-stream-design-audit.md` |
| See strategic direction | `.ddis/specs/NEXT_STEPS_UNIVERSALITY_2026-02-28.md` |

### Key Numbers
| Metric | Value |
|--------|-------|
| Go source files | 238 |
| Total Go LOC | ~62,500 |
| Test LOC | ~12,000 |
| Internal packages | 37 |
| CLI commands | ~45 |
| Spec invariants (CLI) | 97 |
| Spec ADRs (CLI) | 74 |
| Spec modules (CLI) | 9 |
| SQLite tables | 30 |
| Validation checks | 19 |
| Cross-references | 1,356 |
