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

## The Parser Pipeline

`ddis parse` transforms markdown specifications into a queryable SQLite index through a sequential 4-pass architecture.

**Pass 1: Section Tree Building.** A stack-based parser scans for markdown headings (`#` through `######`) and builds a hierarchical section tree. Each heading is normalized into a canonical path: `PART N` becomes `PART-<roman>`, `§N.M` stays as-is, `Chapter N` becomes `Chapter-<n>`, `Appendix X` becomes `Appendix-<letter>`, and everything else is slugified into lowercase dash-separated tokens. Duplicate paths are disambiguated with suffixes (`path~2`, `path~3`). The output is a flat list of section nodes with parent pointers and line ranges.

**Pass 2: Element Extraction.** Twelve parallel recognizers scan the line array for structured elements:

| Recognizer | Pattern | Output |
|------------|---------|--------|
| Invariants | `**INV-NNN: Title**` blocks | 6-component struct (title, statement, semi-formal, violation, validation, why-this-matters) |
| ADRs | `### ADR-NNN: Title` + subheadings | Problem, options with pros/cons, decision, consequences, tests |
| Quality Gates | `**Gate N: Title**` | Gate ID, title, predicate |
| Negative Specs | `**DO NOT** constraint` | Constraint text, reason, invariant ref |
| Verification Prompts | `### Verification Prompt for [Chapter]` | Positive/negative/integration checks |
| Glossary Entries | `\| **Term** \| Definition \|` | Term-definition pairs |
| State Machines | Tables with state/event/transition columns | State, event, transition, guard |
| Performance Budgets | Tables with metric/target columns | Operation, target, measurement method |
| Worked Examples | `#### Worked Example` blocks | Title, content |
| Meta-Instructions | `> **META-INSTRUCTION**: directive` | Directive, reason |
| WHY NOT | `// WHY NOT alternative? explanation` | Alternative, explanation, ADR ref |
| Comparisons | Suboptimal / chosen blocks | Suboptimal vs chosen approach with reasons |

Each recognizer is a finite state machine. The invariant parser, for example, transitions through `idle → headerSeen → statementSeen → inCodeBlock → codeDone → idle`, capturing each component as it appears. The ADR parser similarly transitions through `idle → headerSeen → inProblem → inOptions → inDecision → inConsequences → inTests → idle`, extracting structured fields from subsection content.

**Pass 3: Cross-Reference Extraction.** Every non-code-block line is scanned for four reference types: section refs (`§N.M`), invariant refs (`INV-NNN` or `APP-INV-NNN`), ADR refs (`ADR-NNN` or `APP-ADR-NNN`), and gate refs (`Gate N`). References appearing on definition lines (the line that *defines* the element) are excluded to avoid self-references. Each extracted reference is inserted into the `cross_references` table with `resolved=0`.

**Pass 4: Cross-Reference Resolution.** Each unresolved reference is matched against the current spec's index. If not found locally and the spec declares a `parent_spec` in its manifest, resolution falls back to the parent spec's index. This enables CLI spec references like `INV-006` to resolve against the meta-standard. Successfully matched references are marked `resolved=1`. Unresolved references after this pass are reported by validation check 1 (cross-reference integrity).

A final formatting pass records blank lines and horizontal rules for round-trip fidelity — `ddis render` can reconstruct the original markdown byte-for-byte from the index (APP-INV-001).

## The Search Engine

DDIS implements a deterministic hybrid search that fuses three independent relevance signals via Reciprocal Rank Fusion (RRF). Every score is mechanically derivable — no neural models, no API dependencies, no nondeterminism.

### BM25 (Lexical Relevance)

SQLite FTS5 provides BM25F full-text search over element type, ID, title, and content. Query sanitization strips FTS5 special characters, wraps element IDs (`INV-006`, `ADR-003`, `§0.5`) in quotes for exact matching, and defaults to AND conjunction. BM25F uses k1 = 1.2 (term frequency saturation) and b = 0.75 (field length normalization).

### LSI (Semantic Similarity)

Latent Semantic Indexing captures meaning beyond lexical overlap by projecting documents into a reduced-dimension concept space via Singular Value Decomposition:

```
1. Build TF-IDF matrix A (m terms × n documents)
     tf[i,j] = 1 + log(count)          (sublinear term frequency)
     idf[i]  = log(N / doc_freq)       (inverse document frequency)
     A[i,j]  = tf[i,j] × idf[i]

2. Compute truncated SVD
     A ≈ U_k × Σ_k × V_kᵀ
     k = min(50, doc_count, vocab_size)  (APP-INV-012: dimension bound)

3. Document vectors: d_j = V_k[j] × Σ_k  (scaled right singular vectors)

4. Query projection: q_k = q_tfidf × U_k  (project into latent space)

5. Ranking: cosine(q_k, d_j) for each document j
```

The dimension bound k = 50 keeps computation tractable while capturing semantic relationships invisible to keyword matching. For a typical spec with 500 documents and 5000 vocabulary terms, the LSI model occupies ~750KB.

### PageRank (Structural Authority)

The cross-reference graph (nodes = spec elements, edges = resolved cross-references) is analyzed with power iteration PageRank:

```
Parameters: damping = 0.85, max_iterations = 100, convergence = 1e-6

PR[v] = (1 - d)/|V|  +  d × Σ_{u→v} PR[u] / out_degree[u]

Dangling nodes (out_degree = 0) redistribute mass uniformly.
Typical convergence: 20-40 iterations.
```

Authority measures structural importance, not relevance — a heavily-referenced invariant is important regardless of whether it matches the query terms. This signal is weighted at 0.5× relative to BM25 and LSI.

### RRF Fusion

The three ranked lists are combined with Reciprocal Rank Fusion:

```
score(doc) = Σ_r  weight_r / (K + rank_r(doc))

where:
  K = 60                        (standard RRF parameter)
  weight = {BM25: 1.0, LSI: 1.0, Authority: 0.5}

final_score = score × type_boost
  type_boost = {invariant: 1.2, adr: 1.1, gate: 1.1, section: 1.0,
                negative_spec: 0.9, glossary: 0.8}
```

**Worked example.** A document ranked #1 by BM25, #3 by LSI, #5 by PageRank:

```
raw   = 1.0/(60+1) + 1.0/(60+3) + 0.5/(60+5)
      = 0.01639 + 0.01587 + 0.00769
      = 0.03995

If element type is "invariant":
  final = 0.03995 × 1.2 = 0.0479
```

**Query expansion.** If the query contains a glossary term, significant words (>4 characters) from its definition are added as expansion terms (max 5). This bridges vocabulary gaps without a neural embedding model.

## The Validation System

`ddis validate` runs 20 mechanical checks against the indexed specification. Each check is tied to a formal invariant, produces a severity (error or warning), and includes a human-readable explanation on failure.

| # | Check | Invariant | What It Verifies |
|---|-------|-----------|------------------|
| 1 | Cross-reference integrity | APP-INV-003 | Every `§`, `INV-`, `ADR-`, `Gate` reference resolves to an existing element |
| 2 | Invariant falsifiability | INV-003 | Each invariant has title, statement, violation scenario, and validation method |
| 3 | Cross-reference density | INV-006 | Every non-trivial section has at least 1 inbound reference |
| 4 | Glossary completeness | INV-009 | Terms appearing 3+ times in the spec have glossary definitions |
| 5 | Invariant ownership | INV-013 | Each invariant is maintained by exactly one module |
| 6 | Bundle budget | INV-014 | Module line counts fit within the manifest's context budget |
| 7 | Declaration-definition bijection | INV-015 | Constitution registry entries match actual invariant definitions |
| 8 | Manifest sync | INV-016 | Manifest module names match module declarations in spec body |
| 9 | Negative spec coverage | INV-017 | Each implementation chapter has at least 3 DO NOT constraints |
| 10 | Gate-1 structural conformance | Gate-1 | Required sections exist (overview, invariant registry, ADR index) |
| 11 | Proportional chapter weight | — | Chapter line counts are within 20% of the mean |
| 12 | Namespace consistency | — | Parent invariants use `INV-NNN`, child uses `APP-INV-NNN` consistently |
| 13 | Implementation traceability | APP-INV-041 | No orphaned code annotations (annotations referencing nonexistent spec elements) |
| 14 | Witness freshness | APP-INV-041 | No stale witnesses (spec content changed since witness was recorded) |
| 15 | Event stream VCS tracking | APP-INV-048 | Event JSONL files are tracked in git |
| 16 | Behavioral witness validity | APP-INV-049 | Test-type witnesses originated from actual test execution |
| 17 | Challenge freshness | APP-INV-050 | Every valid witness has a corresponding challenge verdict |
| 18 | Process compliance | APP-INV-056 | Methodology adherence score (annotations, witnesses, tests, reviews) |
| 19 | VCS tracking | APP-ADR-048 | All spec source files are tracked in git |
| 20 | Lifecycle reachability | APP-INV-062 | Every state in state machine tables has a forward path to a terminal state |

Checks 1–12 apply to any DDIS specification. Checks 13–20 apply when the spec has a linked implementation (code annotations, witnesses, event streams). The `--level` flag selects which tiers run: level 1 (structural), level 2 (+ code bridge), level 3 (all checks).

## The Contradiction Checker

`ddis contradict` implements a 5-tier consistency analysis pipeline. Each tier uses a different technique with increasing computational cost and reasoning power. Higher tiers activate when lower tiers are inconclusive. Contradictions are deduplicated across tiers — the highest-confidence result wins for each pair.

### Tier 2: Graph Patterns

For each pair of invariants from different modules, compute the Jaccard similarity of their cross-reference reach sets (the set of elements they reference). If Jaccard > 0.6 and the invariant statements contain opposing polarity markers (`must` vs `must not`, `always` vs `never`, `enable` vs `disable`, `minimize` vs `maximize`), report a `GovernanceOverlap` contradiction. Also detects negative-spec violations: invariant statements that imply behavior explicitly forbidden by a `DO NOT` constraint. Confidence: 0.5–0.8. Complexity: O(N2 x reach).

### Tier 3: SAT/CDCL

Translates semi-formal invariant expressions into propositional CNF (Conjunctive Normal Form) via pattern-based rules:

```
"FOR ALL x IN S: body"  →  body parsed as conjunction
"A IMPLIES B"            →  clause (¬A ∨ B)
"A AND B AND C"          →  three unit clauses (A), (B), (C)
"P(x) = true"            →  unit clause (P_x)
"x.prop = val"           →  atom (x_prop_eq_val)
```

A global variable namespace ensures consistent encoding across all invariants — the same variable name always maps to the same integer ID. Pairwise satisfiability is checked using gophersat's CDCL (Conflict-Driven Clause Learning) solver. If the conjunction of two invariants' clauses is unsatisfiable, they contradict. Confidence: 0.85.

### Tier 4: Heuristic + Semantic

Three pattern-based detectors run in parallel:

- **Polarity inversion**: positive directive of invariant A matches negative directive of B (>50% word overlap)
- **Quantifier conflict**: `FOR ALL` vs `EXISTS` with >60% subject overlap
- **Numeric bound conflict**: incompatible arithmetic constraints (`x >= 10` vs `x < 5`)

Plus TF-IDF cosine similarity: invariant pairs with >0.8 semantic overlap are flagged as `SemanticTension`. Confidence: 0.5–0.7.

### Tier 5: Z3 SMT Solver

Translates semi-formal expressions into SMT-LIB2 format and invokes Z3 as a subprocess (`z3 -in -smt2`, 30-second timeout per pair). Supports four theories:

| Theory | When Selected | Handles |
|--------|---------------|---------|
| `QF_UF` | Default | Uninterpreted functions, predicates |
| `QF_LIA` | Arithmetic constants detected | Linear integer arithmetic |
| `LIA` | Quantifiers detected | Quantified linear arithmetic |
| `ALL` | Mixed patterns | Combined theories |

A validity filter rejects translations with no real logical patterns (only fallback boolean atoms from natural language fragments), preventing false positives. Self-UNSAT invariants (encoding artifacts) are pre-filtered before pairwise checking. Z3 is optional — if not installed, Tier 5 is silently skipped. Confidence: 0.95.

### Tier 6: LLM-as-Judge

Pairwise semantic contradiction analysis via Claude. Three independent invocations with majority vote: 3/3 agreement yields confidence 0.9, 2/3 yields 0.75. Requires an API key; silently skipped if unavailable. Used as the final arbiter when formal tiers are inconclusive but semantic tension is high.

## Event Sourcing in Depth

DDIS implements event sourcing as both a specification strategy and a CLI architecture. The JSONL event log is the canonical source of truth (APP-INV-071). The SQLite index is a materialized view. Rendered markdown is a projection. Both are deterministically derived from the log.

### The Event Schema

Events are JSON objects in append-only `.jsonl` files across three streams:

| Stream | Purpose | Event Types (examples) |
|--------|---------|------------------------|
| Stream 1 (Discovery) | Exploratory discourse | `question_opened`, `finding_recorded`, `decision_crystallized`, `thread_branched`, `mode_observed` |
| Stream 2 (Specification) | Content mutations | `spec_parsed`, `invariant_crystallized`, `adr_crystallized`, `cross_ref_added`, `drift_measured` |
| Stream 3 (Implementation) | Verification lifecycle | `issue_triaged`, `witness_recorded`, `challenge_completed`, `annotation_scanned` |

Every event carries a unique ID, timestamp, stream number, version, type-specific payload, and a `causes` array encoding causal predecessors — forming a partial order over the event history.

### The Fold

Materialization is a pure fold over the event sequence:

```
fold(ε) = empty_database
fold(e₁ · e₂ · ... · eₙ) = apply(...apply(apply(∅, e₁), e₂)..., eₙ)
```

**Causal sort** (Kahn's algorithm with timestamp tiebreaker) topologically orders events by their `causes` relation before replay. The `apply` function is a pure dispatcher — each of the 42+ event types maps to a specific SQL mutation. No randomness, no timestamps in logic, no environment dependencies (APP-INV-073: fold determinism).

**Consequence: temporal queries.** Because fold is deterministic, `fold(log[0:t])` gives the exact spec state at any point in time. `ddis bisect` binary-searches the event log to find the event that introduced a defect. `ddis blame` traces any element back to its originating crystallization event.

### Stream Processors

After each content event, registered processors can emit derived events:

| Processor | Trigger Events | Output |
|-----------|----------------|--------|
| Validation | `invariant_crystallized`, `adr_crystallized` | Completeness findings (missing components) |
| Consistency | `cross_ref_added` | Contradiction signals |
| Drift | `invariant_crystallized`, `invariant_updated` | Drift check triggers |

Derived events carry `derived_by` (processor name) and the triggering event ID in `causes`. Processor failures are non-fatal (APP-INV-091) — a broken processor cannot corrupt the fold.

### Snapshots

Snapshots accelerate incremental replay by storing a materialized state hash at a known event position:

```
CreateSnapshot:  (position, SHA-256(state)) → snapshots table
FoldFrom(snap):  replay only events after snap.position
VerifySnapshot:  recompute hash, compare to stored value
PruneSnapshots:  keep last N, delete older
```

Corrupted snapshots trigger graceful fallback to full replay from the empty state (APP-INV-095).

### CRDT Convergence

Independent events (no causal path between them) commute under fold:

```
¬(e₁ < e₂) ∧ ¬(e₂ < e₁) ⟹ apply(apply(s, e₁), e₂) = apply(apply(s, e₂), e₁)
```

This commutativity (APP-INV-081) enables multi-agent merge: given independent event subsequences from different agents, `merge(A, B) = merge(B, A)`. The event log forms a join-semilattice over the causal partial order.

### The Architectural Inversion

The event-sourcing pipeline inverts the traditional flow:

```
Traditional:  markdown → parse → SQLite       (markdown is canonical)
Event-first:  JSONL → materialize → SQLite → project → markdown  (JSONL is canonical)
```

The transition is non-breaking because of import equivalence (APP-INV-078): `project(materialize(import(md)))` is structurally equivalent to `parse(md)`. Existing markdown specs are bridged via `ddis import`, which emits synthetic events. The `parse` command becomes a convenience alias for import + materialize.

## The Bilateral Lifecycle

The bilateral specification lifecycle is the operational core of DDIS — four interlocking loops that converge spec and implementation toward agreement.

### Discovery Threads

`ddis discover` manages persistent, cross-session inquiry threads. Each thread is a directed line of investigation with an ID, status (`active`, `parked`, `merged`), a 5-dimensional confidence vector, and an event stream stored in `.ddis/events/{thread_id}.jsonl`.

**Thread convergence** selects or creates a thread based on content: Jaccard-like keyword overlap with a recency boost (+0.1 if last event <24 hours). Score >= 0.4 reuses the existing thread; below that, a new thread is created.

Thread event streams are folded into a deterministic `DiscoveryState` — an artifact map, findings set, open questions, and thread topology. This fold is the same pure-function pattern used by event sourcing: no side effects, replay-safe, deterministic.

### The State Monad

Every auto-prompting command returns a triple:

```
CommandResult {
    Output:   string         // human-readable markdown
    State:    StateSnapshot  // machine-readable quality metrics
    Guidance: Guidance       // soft suggestions for next action
}
```

The CLI is the interpreter; the LLM is the reasoner; the human is the input stream. `State` tells the LLM where things stand. `Guidance` suggests what to do next — but neither is mandatory. The LLM can override both.

**StateSnapshot** captures multidimensional quality:

| Dimension | Range | What It Measures |
|-----------|-------|------------------|
| Coverage | 0–10 | Presence of sections, invariants, ADRs, gates |
| Depth | 0–10 | Invariant component completeness (all 5 sub-components) |
| Coherence | 0–10 | Cross-reference resolution ratio |
| Completeness | 0–10 | Element type diversity (7 types present) |
| Formality | 0–10 | Semi-formal expressions present vs total invariants |

The **limiting factor** is the lowest-scoring dimension, prioritized by: completeness > coherence > depth > coverage > formality.

### The k* Attention Budget

Guidance size decreases monotonically with conversation depth to prevent overprompting:

```
k*(depth) = max(3, 12 - floor(depth / 5))

Depth  0–4:   k*=12  →  ~2000 tokens  (full framework)
Depth  5–9:   k*=10  →  ~1500 tokens  (mode + context)
Depth 10–19:  k*=8   →  ~1200 tokens  (focused)
Depth 20–34:  k*=6   →  ~800 tokens   (light)
Depth 35–44:  k*=4   →  ~500 tokens   (minimal)
Depth 45+:    k*=3   →  ~300 tokens   (nudge only)
```

The attenuation factor `1 - (k*/12)` scales guidance from 0% reduction at depth 0 to 75% at depth 45+. Heavy initial framing builds context; subsequent invocations refine with increasingly light touch. This prevents the common failure mode where verbose instructions crowd out the LLM's own reasoning space.

### Cognitive Mode Classification

The system observes (never prescribes) seven cognitive modes:

| Mode | DoF Hint | Signal |
|------|----------|--------|
| Divergent | very high | Exploring the design space |
| Convergent | low | Narrowing toward solution |
| Dialectical | mid | Balancing trade-offs |
| Abductive | mid | Inferring from examples |
| Metacognitive | mid | Reflecting on process |
| Incubation | very low | Letting ideas settle |
| Crystallization | low | Formalizing into spec |

Classification is non-prescriptive (APP-INV-026): mode names appear in machine state (`guidance.observed_mode`), never in user-facing output. The system observes what cognitive mode the conversation is in and adjusts degrees-of-freedom hints accordingly — it never tells the user to change modes.

## The Witness and Challenge System

Invariant witnesses provide graduated evidence that an invariant holds in the implementation.

### Four Proof Levels

| Level | Name | Evidence Required | Confidence |
|-------|------|-------------------|------------|
| 0 | Falsified | No evidence recorded | — |
| 1 | Property-checked | Automated heuristic analysis | Low |
| 2 | Bounded-verified | Test execution evidence with code hash | Medium |
| 3 | Proven | Formal proof or exhaustive checking | High |

**Recording a witness** (`ddis witness`) captures a snapshot of the current spec hash and (optionally) code hash, along with evidence type (`test`, `annotation`, `scan`, `review`, `eval`, `attestation`) and the evidence payload.

**LLM evaluation** (`eval` evidence type) runs 3 independent invocations asking "Does this invariant hold?" Majority vote determines the verdict: 3/3 agreement yields confidence 0.95, 2/3 yields 0.75, no majority rejects the witness.

### Staleness Detection

Witnesses auto-invalidate when the spec changes (APP-INV-041): if the invariant's `content_hash` no longer matches the witness's `spec_hash`, the status becomes `stale_spec`. Code hash mismatches produce `stale_code`. Stale witnesses are excluded from coverage and fitness calculations until re-verified.

### The Challenge Pipeline

`ddis challenge` stress-tests witnesses through a 5-level verification:

1. **Formal** — Does the evidence logically entail the invariant?
2. **Uncertainty** — Are edge cases and boundary conditions covered?
3. **Causal** — Does the test actually exercise the invariant's causal chain?
4. **Practical** — Does the evidence survive realistic conditions?
5. **Meta** — Is the evidence methodology itself sound?

Verdicts: `confirmed`, `provisional`, `refuted`, `inconclusive`. A `refuted` verdict automatically invalidates the witness (APP-INV-050).

### Evidence Chain

Issue closure requires a complete evidence chain for all affected invariants: valid witness + confirmed challenge for each. `VerifyEvidenceChain` gates issue closure — missing witnesses, stale witnesses, missing challenges, or non-confirmed verdicts block closure and emit specific remedy commands.

## The Context Bundle

`ddis context` assembles a 9-signal information packet for a given spec element, designed to give an LLM everything it needs to implement that element correctly.

| # | Signal | Source | Purpose |
|---|--------|--------|---------|
| 1 | Fragment | `ddis query` | The target element's full content |
| 2 | Constraints | Invariants, ADRs, gates, negative specs | What governs this element |
| 3 | Completeness | 5-component check per invariant | What's present vs missing |
| 4 | Gaps | Coverage analysis | Unspecified behaviors requiring attention |
| 5 | Local validation | Scoped checks on target region | What validation issues exist locally |
| 6 | Reasoning modes | Mode tags on related elements | Formal / causal / practical / meta classification |
| 7 | Related elements | LSI cosine similarity | Semantically similar items across the spec |
| 8 | Impact radius | BFS depth-2 forward + backward | What depends on this / what this depends on |
| 9 | Guidance | Synthesis of all signals | What to do and what to avoid |

Every signal field is present in every bundle, even when empty (APP-INV-005). Missing signals are invisible to an LLM — it cannot reason about what it does not see. An empty field explicitly communicates "nothing here" rather than leaving a gap for hallucination.

The bundle is self-contained by construction: an LLM receiving the bundle for a module can implement that module's subsystem without reaching for information outside the bundle (APP-INV-011). Impact analysis uses BFS over the cross-reference graph with a configurable depth bound (default 2) to map the dependency neighborhood without exploding into the full graph.

## The Triage Engine

The triage system computes spec fitness, ranks deficiencies by impact, and recommends the highest-value next action.

### The Fitness Function

F(S) is a weighted sum of six normalized quality signals:

| Signal | Weight | Measures | Perfect |
|--------|--------|----------|---------|
| V (validation) | 0.20 | Fraction of checks passing | 1.0 |
| C (coverage) | 0.20 | Fraction of spec elements complete | 1.0 |
| D (drift) | 0.20 | Normalized spec-impl divergence | 0.0 |
| H (challenge health) | 0.15 | Fraction of witnesses confirmed | 1.0 |
| K (contradictions) | 0.15 | Normalized conflict count | 0.0 |
| I (issues) | 0.10 | Normalized open issue count | 0.0 |

Weights are fixed (not configurable) to prevent gaming. Foundation signals (validation, coverage, drift) each get 0.20. Verification signals (challenges, contradictions) get 0.15. Process signals (issues) get 0.10.

### Deficiency Ranking

For each signal below perfect, the engine computes the potential fitness gain: `delta_F = gap x weight`, then sorts descending. This implements **steepest descent** — the recommended next action always targets the largest available fitness improvement. The output is a ranked list of concrete commands: "run `ddis validate` (estimated +0.15 F(S))", "run `ddis coverage` (estimated +0.12 F(S))".

### Convergence Guarantee

The triage measure mu(S) = (open_issues, unspecified, drift) with **lexicographic well-founded ordering** provides a convergence proof: each triage step either decreases mu lexicographically or has reached the fixpoint. Since the natural numbers cubed with lex ordering is well-founded, the process terminates. This is not a heuristic — it is a mathematical guarantee that the bilateral loop converges.

### Issue State Machine

Issues follow an event-sourced state machine:

```
filed → triaged → specified → implementing → verified → closed
                                                ↓
                                             wontfix
```

Each transition emits a Stream 3 event. The `triaged` transition must carry a discovery `thread_id` (APP-INV-063), linking every issue to its specification context. State is derived by replaying events — no mutable status field, no lost updates.

## The Code-Spec Bridge

The annotation system bridges source code and specification through structured comments that the CLI can scan, verify, and cross-reference.

### Annotation Grammar

```
// ddis:<verb> <target> [qualifier]
```

**Verbs** define the relationship:

| Verb | Meaning | Example |
|------|---------|---------|
| `maintains` | This code maintains this invariant | `// ddis:maintains APP-INV-042` |
| `implements` | This code implements this decision | `// ddis:implements APP-ADR-015` |
| `tests` | This test covers this element | `// ddis:tests INV-006` |
| `interfaces` | This code interfaces with this element | `// ddis:interfaces APP-INV-003` |
| `validates-via` | This validates using this method | `// ddis:validates-via ADR-025` |
| `postcondition` | This establishes this postcondition | `// ddis:postcondition APP-INV-010` |
| `relates-to` | General relationship | `// ddis:relates-to Gate-1` |
| `satisfies` | This satisfies this requirement | `// ddis:satisfies APP-INV-055` |

**Targets** are spec element IDs: `INV-NNN`, `APP-INV-NNN`, `ADR-NNN`, `APP-ADR-NNN`, `Gate-N`, `§N.M`, or `@custom-id`.

### Language Support

The scanner recognizes comment syntax for 15+ language families: `//` (Go, Rust, TypeScript, Java, C++, C#, Swift, Kotlin), `#` (Python, Ruby, Shell, YAML, TOML), `--` (SQL, Lua, Haskell), `;` (Lisp, Clojure, Assembly), `%` (LaTeX, Erlang), and `<!--` (HTML, XML, Markdown).

### Bidirectional Verification

`ddis scan` performs cross-verification in both directions:

- **Orphaned annotations**: code references a spec element that doesn't exist in the index — the annotation is stale or the spec changed under it.
- **Unimplemented elements**: spec elements with no code annotations — either the code isn't annotated or the element isn't implemented yet.

Both directions are reported. Validation check 13 (implementation traceability) fails if orphaned annotations exist. The DDIS CLI itself carries 511 annotations across 30+ packages with 0 orphaned.

## The SQLite Schema

The 38-table schema (plus 1 virtual FTS5 table) organizes into eight functional groups:

**Spec Management** (3 tables). `spec_index` stores parsed specification metadata (path, name, version, content hash, source type, parent spec foreign key). `source_files` tracks individual files with role classification (monolith, manifest, constitution, module). `manifest` holds parsed manifest.yaml content (tier mode, context budget, raw YAML).

**Document Structure** (1 table). `sections` stores the hierarchical section tree with self-referencing `parent_id`, canonical section paths, heading levels, line ranges, content hashes, and full raw text for round-trip rendering.

**Spec Elements** (6 tables). `invariants` stores 6-component definitions. `adrs` stores architecture decision records with status tracking and supersession chains. `adr_options` normalizes option analysis (label, pros, cons, chosen flag, why-not). `quality_gates` stores gate predicates. `negative_specs` stores DO NOT constraints linked to sections. `glossary_entries` stores term-definition pairs.

**Verification** (5 tables). `verification_prompts` and `verification_checks` store per-chapter verification criteria (positive, negative, integration). `invariant_witnesses` stores proof receipts with evidence type, hash snapshots, and staleness tracking. `challenge_results` stores 5-level challenge verdicts. `invariant_registry` stores the constitution-level registry with ownership.

**Structural Elements** (8 tables). `meta_instructions`, `worked_examples`, `why_not_annotations`, `comparison_blocks` store extracted structural content. `performance_budgets` and `budget_entries` store performance constraint tables. `state_machines` and `state_machine_cells` store extracted state/event/transition specifications.

**Cross-Reference Graph** (1 table). `cross_references` stores directed edges with source, target, reference type (section, invariant, adr, gate, glossary), and resolution status. Indexed on both source and target for bidirectional traversal.

**Modular Structure** (3 tables). `modules` stores module definitions with domain and line count. `module_relationships` stores inter-module edges (maintains, interfaces, implements, adjacent). `module_negative_specs` stores module-level DO NOT constraints from the manifest.

**Transactions** (2 tables). `transactions` tracks spec modification transactions with a state machine (pending → committed | rolled_back). `tx_operations` stores individual operations within transactions, ordered by ordinal.

**Search** (4 tables). `fts_index` is a virtual FTS5 table for BM25. `search_vectors` stores LSI document vectors as binary blobs. `search_authority` stores PageRank scores. `search_model` stores model metadata (dimensions, term count, doc count).

**Session and Bridge** (5 tables). `session_state` provides key-value storage for authoring sessions. `code_annotations` stores extracted ddis: annotations from source code. `snapshots` stores materialization checkpoints. `event_provenance` tracks element-to-event provenance. `formatting_hints` stores blank lines and horizontal rules for round-trip fidelity.

## Design Principles

These principles explain *why* DDIS works the way it does.

### Determinism as Cornerstone

Multiple invariants enforce determinism across the system: validation produces identical results regardless of clock, RNG, or execution order (APP-INV-002). Hashing uses SHA-256 with no salt (APP-INV-015). Event fold is a pure function — same events always produce identical state (APP-INV-073). The end-to-end pipeline (import, materialize, project) produces byte-identical output across runs (APP-INV-097).

Determinism is not a convenience — it is what makes the rest of the system possible. Temporal queries require that `fold(log[0:t])` always produces the same state. CRDT merge requires that independent events commute. Bisection requires that replaying events up to position N always reproduces the defect. Multi-agent collaboration requires that concurrent folds converge. Remove determinism and the bilateral loop oscillates instead of converging.

### Monotonicity as Progress Guarantee

Four key monotonicity invariants prevent oscillation:

- **INV-022 / APP-INV-022**: Drift reconciliation only reduces drift, never increases it
- **APP-INV-004**: Adding a relevant cross-reference can only increase authority scores
- **APP-INV-040**: Progressive validation is monotonically inclusive (Level 1 checks are a subset of Level 2, which are a subset of Level 3)
- **APP-INV-094**: Snapshot positions are monotonically non-decreasing

Without monotonicity, a system could enter cycles where reconciliation increases drift, which triggers more reconciliation that increases drift further. Monotonicity turns "the system improves" from a hope into a mathematical fact.

### LLM-Awareness as First-Class Concern

DDIS is not "AI-powered" — the CLI is fully deterministic. But the specification format is explicitly designed for LLM consumption:

1. **Negative specifications** (INV-017): LLMs hallucinate plausible behavior to fill gaps. Explicit DO NOT constraints block the most likely hallucination patterns per subsystem — at least 3 per chapter.
2. **Structural redundancy** (INV-018): LLMs cannot "flip back" to earlier definitions the way humans can. Invariants are restated at point of use, not merely cited by ID. A reference 2,000 lines from its definition is functionally invisible.
3. **Context bundles**: 9-signal packets that fit within context windows, contain everything needed for implementation, and leave no gaps for hallucination to fill.
4. **Modular decomposition**: Two-tier constitution + per-domain modules. Each bundle (constitution + one module) fits the context budget (target 4,000 lines, ceiling 5,000).
5. **Verification prompts**: Self-check criteria the LLM can run against its own output before committing.
6. **Fixed document structure**: Variable organization forces the LLM to learn the scheme in addition to the content. Fixed structure reduces cognitive overhead and makes pattern matching reliable.

### The Inverse Principle

Every forward operation has a dual:

```
discover  ⊣  absorb       (idea ↔ implementation)
parse     ⊣  render       (markdown ↔ index)
witness   ⊣  challenge    (attest ↔ verify)
refine    ⊣  drift        (improve spec ↔ measure divergence)
tasks     ⊣  traceability (spec → work items ↔ work items → spec)
```

Each adjunction has a unit measuring round-trip divergence from identity. For parse/render, the unit is byte-level: `render(parse(spec)) = spec` (APP-INV-001). Drift is the distance of the absorb/discover round-trip from identity. The bilateral loop is complete when all units are within tolerance — when every round-trip is (approximately) the identity function.

### Self-Containment

The spec plus the implementer's general competence plus public references must be sufficient for a correct implementation (INV-008). Every gap in the spec is a hallucination site. Corollary: no implicit context. Every assumption, every constraint, every prohibition is stated explicitly. If something matters, it is written down. If it is not written down, an LLM will invent its own version.

### Composition Over Mutation

- Modules compose via the constitution, not code inclusion
- Commands are pure functions: `state → (output, new_state, guidance)`
- Event streams are appended, never mutated
- Operations produce new artifacts rather than modifying originals in place

This is not incidental style — composition is what makes the bilateral loop possible. If operations mutated state in place, temporal queries would be impossible (no prior state to query). Bisection would be impossible (no way to reproduce intermediate states). CRDT merge would be impossible (mutations don't commute). The append-only, composition-first architecture is a direct consequence of the determinism and convergence requirements.

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
