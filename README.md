# DDIS — Discourse-Driven Implementation Specification

A formal standard and CLI toolchain for writing software specifications that are simultaneously human-readable, LLM-consumable, and machine-validatable — where specification is a bilateral feedback loop, not a one-way decree.

## The Problem

Software specifications fail in three predictable ways.

**Informal docs rot.** Most teams write specifications as markdown documents, wiki pages, or design docs. These documents capture intent at a point in time but have no mechanism for detecting when the code diverges. Within weeks, the specification describes a system that no longer exists. Engineers learn to distrust the docs and fall back on oral tradition, code archaeology, and asking the original author. The specification becomes shelfware.

**Formal specs go unread.** Formal methods — TLA+, Alloy, Z — produce machine-checkable specifications, but few practicing engineers can write or read them fluently. The specification becomes a parallel artifact maintained by specialists, disconnected from the implementation decisions that determine correctness. The formalism verifies a model of the system, not the system itself.

**The LLM gap.** Large language models are increasingly capable implementers, but they consume specifications under constraints fundamentally different from human readers: fixed context windows, no random access, a tendency to hallucinate plausible details that fill gaps in the spec, and no ability to ask clarifying questions. Specifications written for humans fail silently when consumed by LLMs — the model produces confident, plausible, subtly wrong implementations because the spec left room for interpretation. No existing specification standard addresses this failure mode.

DDIS is a response to all three. It is a markdown-based specification format with embedded formal structure — invariants, architecture decision records, quality gates, negative specifications, verification prompts — that a deterministic CLI tool can parse into a queryable SQLite index, validate mechanically, measure for drift against implementation, and assemble into context bundles optimized for LLM consumption.

## The DDIS Approach

The core insight is **bilateral specification**: specification is not a one-way flow from human intent to code. It is a continuous feedback loop between two directions.

In the **forward direction**, human exploration becomes formal spec. An engineer discovers design insights through conversation (`ddis discover`), refines them into spec-quality prose (`ddis refine`), and crystallizes them into the canonical event log (`ddis crystallize`). The tool parses the result into a queryable index (`ddis parse`) and validates it against the standard's own rules (`ddis validate`).

In the **backward direction**, implementation reality speaks back into the spec. Code annotations are scanned for spec-element references (`ddis scan`). Implementation patterns are absorbed into the spec (`ddis absorb`). Divergence between spec and code is quantified as drift (`ddis drift`). Drift reconciliation is monotonically decreasing (INV-022): every reconciliation step reduces the gap, never increases it.

These two directions form a closed loop. The system converges to a **fixpoint** — a state where the spec fully describes the implementation, the implementation fully satisfies the spec, all invariants are witnessed, all quality gates pass, and drift is zero. This fixpoint is not aspirational. The DDIS project has achieved it on its own specification: F(S) = 1.0, 97/97 invariants witnessed, 19/19 validation checks passing, 0 drift. The spec describes the tool; the tool validates the spec; both agree completely.

The formal model underlying this loop is a **spec fitness function** F(S) in [0,1], a weighted combination of six normalized quality signals — validation score, coverage, drift, challenge verdicts, contradiction count, and open issues. The Lyapunov complement V(S) = 1 - F(S) provides convergence evidence: V(S) = 0 if and only if the spec has reached fixpoint. Each triage step is a contractive endofunctor on the spec state space, guaranteed to decrease the triage measure or reach the fixed point.

DDIS is **self-bootstrapping**. The DDIS standard is written in DDIS format. The CLI tool parses, indexes, and validates the standard that defines it. The CLI application specification is itself a DDIS-conforming document validated by the tool it describes. This is not circular — the spec came first, the tool implements it, and then the tool validates the spec. If the spec is invalid under its own tool's rules, either the spec or the tool has a bug, and both must be fixed.

## Key Concepts

- **Causal Chain** (INV-001). Every implementation section traces back through an architecture decision record, through an invariant, to the formal model. If a section's ancestry cannot be traced to first principles, the section is unjustified. The CLI mechanically verifies this chain — no orphaned sections survive validation.

- **Bilateral Specification**. The forward cycle (`discover` -> `refine` -> `crystallize` -> `parse`) moves human intent into formal spec. The backward cycle (`scan` -> `absorb` -> `drift` -> reconcile) moves implementation reality into the spec. Neither direction is privileged. The spec is the meeting point, not the origin.

- **Quality Gates**. Stop-ship predicates ordered by priority. The DDIS meta-standard defines 7 gates (structural conformance, causal chain integrity, decision coverage, invariant falsifiability, cross-reference web, implementation readiness, LLM implementation readiness) plus 5 conditional modularization gates. The CLI application spec defines 6 gates, where Gate-6 is self-validation: the CLI must parse and validate its own specification with zero errors.

- **Negative Specifications** (INV-017). Every implementation chapter states what the subsystem must NOT do, not merely what it must do. This is the primary defense against LLM hallucination. An LLM that encounters only positive specifications will fill gaps with plausible but unauthorized behaviors. Explicit `DO NOT` constraints per subsystem prevent this. Example from the event-sourcing module: "Must NOT read or write markdown as canonical source of truth."

- **Modularization** (INV-011 through INV-016). Specifications exceeding 2,500 lines decompose into a two-tier structure: a system constitution (included in every bundle) containing declarations, and per-domain modules containing full definitions. Each bundle (constitution + one module) fits within an LLM context window. Cross-module references go through the constitution only — no direct module-to-module coupling.

- **Event Sourcing** (APP-INV-071). The JSONL event log is the canonical source of truth. The SQLite index and rendered markdown are derived views, produced by deterministic fold (`ddis materialize`) and pure projection (`ddis project`). Content-mutating commands write only to the event log. This enables temporal queries, bisection to find defect-introducing events, snapshot optimization, and CRDT-style merge for concurrent edits.

- **Context Bundles** (`ddis context`). 9-signal information packets assembled for LLM consumption: the queried fragment, governing constraints (invariants and ADRs), completeness assessment, gap analysis, local validation results, mode tags, LSI-similar elements, impact graph, and synthesized guidance. A bundle is self-contained — an LLM receiving it can implement the targeted subsystem without reaching for information outside the bundle (APP-INV-005).

- **Invariant Witnesses**. A 4-level confidence hierarchy for invariant verification: Level 0 (falsified) -> Level 1 (property-checked via automated heuristics) -> Level 2 (bounded-verified via test evidence) -> Level 3 (proven via formal methods or exhaustive checking). The `ddis witness` command records proof receipts; `ddis challenge` stress-tests them with a 5-level verification pipeline including SAT solving, Z3 SMT checking, and LLM-as-judge evaluation.

- **Drift**. Quantified divergence between spec and implementation. The `ddis drift` command classifies drift by category (structural, behavioral, terminological), measures it numerically, and generates remediation plans. Reconciliation is monotonically non-increasing (INV-022): every reconciliation step preserves existing correspondences and reduces the drift score.

- **Contradiction Detection** (`ddis contradict`). A 5-tier consistency checker: Tier 1 (graph-based, checks contradiction edges), Tier 2 (predicate analysis), Tier 3 (SAT/DPLL propositional encoding), Tier 4 (heuristic + semantic analysis), Tier 5 (Z3 SMT solver via SMT-LIB2). Each tier is independently useful; higher tiers activate when lower ones are inconclusive.

## Quick Start

```bash
# Install
go install github.com/wvandaal/ddis@latest

# Initialize a new DDIS workspace
ddis init my-project

# Parse a specification into a queryable index
ddis parse manifest.yaml -o spec.db

# Validate against all mechanical checks
ddis validate spec.db

# Check coverage and drift
ddis coverage spec.db
ddis drift spec.db

# Search the spec and get LLM-optimized context
ddis search spec.db "authentication"
ddis context spec.db "§3.2"

# Start a discovery session
ddis discover --spec spec.db

# See what to do next
ddis next
```

Running bare `ddis` with no arguments is equivalent to `ddis next` — it computes current spec fitness and recommends the highest-impact next action.

## The Bilateral Loop

```
                    FORWARD (human → spec)
                    ─────────────────────►

    ┌──────────┐    ┌──────────┐    ┌─────────────┐    ┌──────────┐
    │ discover │───►│  refine  │───►│ crystallize  │───►│  parse   │
    │          │    │          │    │              │    │          │
    │ explore  │    │ formalize│    │ emit events  │    │ index +  │
    │ intent   │    │ language │    │ to JSONL log │    │ validate │
    └──────────┘    └──────────┘    └─────────────┘    └────┬─────┘
         ▲                                                   │
         │                    ┌──────────┐                   │
         │                    │   SPEC   │                   │
         │                    │  (SQLite │◄──────────────────┘
         │                    │   index) │
         │                    └────┬─────┘
         │                         │
    ┌────┴─────┐    ┌──────────┐  │    ┌──────────┐
    │  drift   │◄───│  absorb  │◄─┘───►│   scan   │
    │          │    │          │       │          │
    │ quantify │    │ merge    │       │ extract  │
    │ diverge  │    │ back     │       │ code     │
    └──────────┘    └──────────┘       │ annots   │
                                       └──────────┘
                    ◄─────────────────────
                    BACKWARD (impl → spec)
```

The forward cycle transforms human exploration into indexed, validated specification. The backward cycle transforms implementation evidence into spec updates. The spec (SQLite index) sits at the center. Convergence occurs when both directions produce no further changes — the fixpoint where F(S) = 1.0.

## CLI Command Reference

### Core Workflow

| Command | Description |
|---------|-------------|
| `next` | Compute current fitness and recommend next action |
| `parse` | Parse markdown spec into SQLite index |
| `validate` | Run mechanical validation checks against the index |
| `coverage` | Measure completeness across all spec domains |
| `drift` | Quantify and classify spec-implementation divergence |
| `spec` | Register or list specs in a multi-spec workspace |
| `issue` | File, triage, close, and list spec issues |
| `triage` | Automated triage: fitness, ranked work, protocol output |
| `materialize` | Fold event log into SQLite index (event-sourcing path) |
| `project` | Render SQLite index to markdown (projection) |

### Investigation

| Command | Description |
|---------|-------------|
| `context` | Assemble 9-signal context bundle for LLM consumption |
| `search` | Hybrid BM25 + LSI + PageRank search with RRF fusion |
| `query` | Retrieve spec fragments by section path, INV, ADR, or gate |
| `exemplar` | Corpus-derived demonstrations for a spec element |
| `impact` | BFS forward/backward impact analysis over cross-ref graph |
| `cascade` | Module cascade analysis for change propagation |
| `contradict` | 5-tier contradiction detection (graph, SAT, Z3, LLM) |
| `history` | Unified timeline across event streams |
| `bisect` | Binary search for defect-introducing event |
| `blame` | Trace spec element to originating events |

### Improvement

| Command | Description |
|---------|-------------|
| `discover` | Conversational specification discovery with thread topology |
| `refine` | Iterative spec improvement with drift-monotonic guarantee |
| `absorb` | Merge implementation patterns back into spec |
| `witness` | Record invariant proof receipts at 4 confidence levels |
| `challenge` | Stress-test witnesses with 5-level verification pipeline |
| `scan` | Extract code annotations referencing spec elements |

### Planning

| Command | Description |
|---------|-------------|
| `progress` | DAG frontier report: done, ready, and blocked elements |
| `impl-order` | Topological sort of implementation dependencies (Kahn's) |
| `checklist` | Generate verification checklist from spec element |
| `bundle` | Three-tier domain assembly (constitution + domain + stubs) |
| `skeleton` | Generate DDIS-conformant spec template |
| `diff` | Structural diff between two spec indices |
| `tasks` | Derive implementation tasks from spec artifact map |

### Utility

| Command | Description |
|---------|-------------|
| `render` | Render SQLite index back to markdown (round-trip) |
| `seed` | Initialize oplog with genesis record |
| `log` | Browse operation log with filtering |
| `tx` | Transaction lifecycle (begin, commit, rollback) |
| `state` | Session state key-value store |
| `checkpoint` | Run quality gate checks |
| `init` | Initialize a new DDIS workspace |
| `patch` | Surgical spec editing |
| `manifest` | Manifest sync and scaffolding |
| `import` | Convert markdown to event stream |
| `replay` | Materialize to a specific event position |
| `rename` | Rename spec elements across files and index |
| `snapshot` | Create, list, verify, and prune event snapshots |

## Specification Format

A DDIS specification is a collection of markdown files organized by a `manifest.yaml`. The manifest declares the module structure, invariant registry, and context budget.

### Manifest

```yaml
ddis_version: "3.0"
spec_name: "My System Specification"
tier_mode: "two-tier"
parent_spec: "../ddis-modular/manifest.yaml"  # optional: inherit from meta-standard

context_budget:
  target_lines: 4000
  hard_ceiling_lines: 5000
  reasoning_reserve: 0.25

constitution:
  system: "constitution/system.md"

modules:
  parse-pipeline:
    file: "modules/parse-pipeline.md"
    domain: parsing
    maintains: [APP-INV-001, APP-INV-009]
    interfaces: [APP-INV-002, APP-INV-003]
    implements: [APP-ADR-001, APP-ADR-002]
    negative_specs:
      - "Must NOT silently drop markdown content during parsing"
      - "Must NOT assume heading hierarchy is always well-formed"
```

### Invariant Definition

Invariants are numbered, falsifiable constraints. Each has a violation scenario (a concrete way it could fail) and a verification method.

```markdown
**APP-INV-001: Round-Trip Fidelity** (Owner: parse-pipeline)
Parse followed by render MUST produce byte-identical output for any valid
DDIS specification.
Confidence: bounded-verified

*Violation: parse a spec with trailing whitespace in a heading; render
drops the whitespace.*

Formal expression:
  forall spec in ValidDDIS: render(parse(spec)) = spec

- Source: `internal/parser/parser.go`
- Tests: `tests/roundtrip_test.go`
```

### Architecture Decision Record

ADRs capture design choices where alternatives exist. Each records what was decided, what was rejected, and why.

```markdown
### APP-ADR-001: Monolith-First Parsing

**Problem:** Should the parser handle monolith specs, modular specs, or both?

**Options:**
1. Monolith only — single file, no manifest
2. Modular only — always require manifest.yaml
3. Both — detect format from input, share parsing core

**Decision:** Option 3. The parser accepts both formats. A manifest.yaml
input triggers modular parsing; a .md input triggers monolith parsing.
The underlying 4-pass pipeline is shared.

**Consequences:** Two code paths must produce equivalent indices
(APP-INV-009). Testing requires fixtures for both formats.

**Tests:** `tests/parser_test.go::TestMonolithModularEquivalence`
```

### Negative Specifications

Every module declares explicit constraints on what the subsystem must NOT do. These appear both in the manifest and inline in the spec body.

```markdown
**DO NOT** return search results without a computable score derivation.
Every result must include the RRF formula components that produced its rank.

**DO NOT** build context bundles that reference elements outside the spec
index. Bundles are self-contained by construction, not convention.
```

## Architecture

The CLI is implemented in Go (~62,000 LOC) as a single statically-linked binary with no runtime dependencies beyond the filesystem.

```
ddis-cli/
├── cmd/ddis/main.go          # Entry point
├── internal/
│   ├── cli/                   # 48 cobra command implementations
│   ├── parser/                # 4-pass markdown-to-index pipeline
│   ├── storage/               # SQLite storage layer (39 tables)
│   ├── search/                # BM25 + LSI + PageRank with RRF fusion
│   ├── validator/             # 19 mechanical validation checks
│   ├── events/                # JSONL event stream read/write
│   ├── materialize/           # Event fold, structural diff, snapshots
│   ├── projector/             # SQLite-to-markdown projection
│   ├── consistency/           # 5-tier contradiction checker (graph, SAT, Z3, LLM)
│   ├── drift/                 # Drift detection, classification, remediation
│   ├── witness/               # Invariant proof receipts, 4-level confidence
│   ├── challenge/             # 5-level witness verification pipeline
│   ├── discovery/             # Thread topology, artifact maps
│   ├── autoprompt/            # Guidance generation, mode classification
│   ├── llm/                   # LLM provider abstraction (Anthropic via net/http)
│   ├── triage/                # Fitness function, ranked work, convergence
│   ├── coverage/              # Completeness metrics across domains
│   ├── diff/                  # Structural diffing with composite keys
│   ├── impact/                # BFS forward/backward impact analysis
│   ├── bundle/                # Three-tier domain assembly (pullback)
│   ├── cascade/               # Module cascade via reverse ref lookup
│   ├── implorder/             # Kahn's topological sort
│   ├── skeleton/              # Go text/template scaffold generator
│   ├── annotate/              # Code annotation extraction
│   ├── absorb/                # Spec absorption from implementation
│   ├── refine/                # Iterative spec refinement
│   ├── discover/              # Discovery session management
│   ├── causal/                # Causal chain verification
│   ├── process/               # Stream processors (validation, consistency, drift)
│   ├── workspace/             # Multi-spec workspace management
│   ├── state/                 # Session state CRUD
│   ├── oplog/                 # Append-only operation log
│   ├── checklist/             # Verification checklist generation
│   ├── progress/              # DAG frontier partitioning
│   ├── exemplar/              # Corpus-derived demonstrations
│   └── query/                 # Fragment retrieval
└── tests/                     # 755 test functions across 55 test files
```

Key architectural decisions:

- **SQLite as the index** (APP-ADR-002). A single-file, zero-configuration, ACID-compliant database. No server, no network, no configuration. The 39-table schema normalizes the specification into queryable relations while preserving the original document structure for round-trip rendering.

- **JSONL event streams** (APP-ADR-058). Content-mutating operations append events to `.jsonl` files. The SQLite index is a materialized view derived by deterministic fold. This enables temporal queries (`ddis replay`), defect bisection (`ddis bisect`), provenance tracing (`ddis blame`), and snapshot-accelerated recovery.

- **Hybrid search with RRF fusion** (APP-ADR-003). Three signals — BM25 lexical relevance (via SQLite FTS5), LSI semantic similarity (via gonum SVD), and PageRank structural authority (from the cross-reference graph) — are combined via Reciprocal Rank Fusion. Every score is mechanically derivable from the formula: `SUM(1/(K + rank_r(d)) * weight_r)`.

- **5-tier contradiction checker** (APP-ADR-038). Tier 1: graph-based (contradiction edges in the cross-reference graph). Tier 2: predicate analysis. Tier 3: SAT/DPLL propositional encoding via gophersat. Tier 4: heuristic + semantic analysis. Tier 5: Z3 SMT solver via SMT-LIB2 subprocess. Higher tiers activate when lower tiers are inconclusive. Z3 is optional — the tool degrades gracefully if it is not installed.

- **Pure Go, no CGo for SQLite** (via modernc.org/sqlite). The binary compiles and runs on any platform Go targets without requiring a C compiler or system SQLite installation.

## Self-Bootstrapping

DDIS validates itself at two levels.

**The meta-standard validates itself.** The DDIS standard (`ddis-modular/`) is written in DDIS format: a manifest with a constitution and 5 domain modules, 23 invariants, 14 ADRs, 7 quality gates, and negative specifications per module. The CLI parses and validates it.

**The CLI spec validates itself.** The CLI application specification (`ddis-cli-spec/`) is a DDIS-conforming document with a constitution and 9 domain modules, 97+ invariants, 74+ ADRs, and 6 quality gates. The CLI it describes parses and validates it. Quality Gate APP-G-6 makes this mechanical: the tool must validate its own specification with zero errors.

The project achieved fixpoint on 2026-02-27:

| Metric | Value |
|--------|-------|
| Spec fitness F(S) | 1.0 |
| Lyapunov complement V(S) | 0.0 |
| Triage measure | (0, 0, 0) |
| Validation checks | 19/19 passing |
| Coverage | 100% (97 INV, 74 ADR, all 10 domains) |
| Drift | 0 |
| Invariant witnesses | 97/97 witnessed (Level 2+) |
| Challenges | 97/97 confirmed (5-level verification) |
| Code annotations | 511 across 30+ packages, 0 orphaned |
| Cross-references | 1,356 resolved, 0 unresolved |

## The Formal Model

The CLI system state is an 8-tuple drawn from the constitution (ddis-cli-spec/constitution/system.md, Section 0.2):

```
S = (SpecFiles, Index, SearchState, OpLog, TxState, EventStreams, DiscoveryState, Workspace)

where:
  SpecFiles      = MarkdownFile | (Manifest * ModuleFile*)
  Index          = SQLiteDB(39 tables)
  SearchState    = FTSIndex * LSIModel * AuthorityScores
  OpLog          = JSONL(DiffRecord | ValidateRecord | TxRecord)*
  TxState        = Map(TxID -> {pending | committed | rolled_back})
  EventStreams   = (DiscoveryJSONL * SpecJSONL * ImplJSONL)
  DiscoveryState = ThreadTopology * ArtifactMap * ConfidenceVector * OpenQuestions
  Workspace      = Map(SpecID -> {manifest_path, parent_spec, related_specs, drift_score})
```

Each of the 48 CLI commands is a transition function over this state space. The specification defines all transitions formally — for example:

```
T_parse:       SpecFiles -> Index * SearchState
T_render:      Index -> SpecFiles                     (inverse of T_parse)
T_context:     Index * Target -> Bundle(9 signals)
T_materialize: EventStreams * Snapshot? -> Index       (fold events into SQLite)
T_drift:       Index * CodeRoot? -> DriftReport
```

The **fitness function** F(S) combines six signals into a single scalar:

```
F(S) = 0.20*V(S) + 0.20*C(S) + 0.20*(1-D(S)) + 0.15*H(S) + 0.15*(1-K(S)) + 0.10*(1-I(S))

where:
  V(S) = validation score (fraction of checks passing)
  C(S) = coverage score (fraction of elements complete)
  D(S) = drift score (normalized divergence)
  H(S) = challenge health (fraction of witnesses confirmed)
  K(S) = contradiction density (normalized conflict count)
  I(S) = issue density (normalized open issue count)
```

F(S) = 1.0 if and only if all validation checks pass, coverage is complete, drift is zero, all challenges are confirmed, no contradictions exist, and no issues are open. The Lyapunov complement V(S) = 1 - F(S) provides convergence evidence for the triage endofunctor.

## Project Structure

```
ddis/
├── ddis-modular/                  # The DDIS meta-standard (v3.0)
│   ├── manifest.yaml              # 5 modules, 23 invariants, 14 ADRs
│   ├── constitution/
│   │   └── system.md              # System constitution (tier-1)
│   └── modules/
│       ├── core-standard.md       # INV-001–010, INV-017–020
│       ├── element-specifications.md
│       ├── modularization.md      # INV-011–016
│       ├── guidance-operations.md
│       └── drift-management.md    # INV-021–023
│
├── ddis-cli-spec/                 # CLI application spec (v1.0)
│   ├── manifest.yaml              # 9 modules, parent_spec → ddis-modular
│   ├── constitution/
│   │   └── system.md              # 97+ invariants, 74+ ADRs, 6 gates
│   └── modules/
│       ├── parse-pipeline.md      # Parsing domain
│       ├── search-intelligence.md # Search domain
│       ├── query-validation.md    # Validation domain
│       ├── lifecycle-ops.md       # Lifecycle domain
│       ├── code-bridge.md         # Code bridge domain
│       ├── auto-prompting.md      # Auto-prompting domain
│       ├── workspace-ops.md       # Workspace domain
│       ├── triage-workflow.md     # Triage domain
│       └── event-sourcing.md      # Event sourcing domain
│
├── ddis-cli/                      # CLI implementation (Go)
│   ├── cmd/ddis/main.go           # Entry point
│   ├── internal/cli/              # 48 command implementations
│   ├── internal/                  # 35 core packages
│   └── tests/                     # 755 test functions
│
└── AGENTS.md                      # Agent guidelines
```

**Language:** Go 1.24 | **Module:** `github.com/wvandaal/ddis` | **Dependencies:** cobra (CLI), gonum (linear algebra for LSI), modernc.org/sqlite (pure-Go SQLite), gophersat (SAT solver), go-yaml (YAML parsing)

## Project Status

DDIS is a working research system that has achieved fixpoint on its own specification. It is not production software in the conventional sense — it is a tool for writing and validating specifications, and its primary user to date is itself.

| Component | Status |
|-----------|--------|
| Meta-standard | v3.0 — 5 modules, 23 invariants, 7+5 quality gates |
| CLI specification | v1.0 — 9 modules, 97+ invariants, 74+ ADRs, 6 quality gates |
| CLI implementation | ~62,000 LOC Go, 48 commands, 755 test functions |
| Self-bootstrap | Fixpoint achieved: F(S) = 1.0, V(S) = 0.0 |
| Event sourcing | Full pipeline: import, materialize, project, snapshot, bisect, blame, replay |
| Contradiction checker | 5 tiers operational (graph, predicate, SAT, heuristic, Z3) |
| LLM integration | Provider abstraction, LLM-as-judge for Tier 6 contradiction, eval evidence |

Active development areas: universality adoption across downstream specs, event-sourcing pipeline hardening, and multi-agent triage workflows.

## Contributing

See `AGENTS.md` for coding guidelines. The self-bootstrapping requirement applies to all contributions: changes to the specification must pass the CLI's own validation, and changes to the CLI must not regress the specification's fitness score. Run `ddis validate` and `ddis drift` before committing.

The repository lives at [github.com/wvandaal/ddis](https://github.com/wvandaal/ddis).
