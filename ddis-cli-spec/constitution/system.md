---
module: system-constitution
domain: system
tier: 1
description: >
  System Constitution (Tier 1) — included in EVERY bundle.
  Contains executive summary, formal state space model, invariant/ADR/gate declarations,
  glossary, and cross-cutting concerns for the DDIS CLI tool.
ddis_version: "3.0"
tier_mode: two-tier
---

# DDIS CLI: Transactional Specification Management System

## Version 1.0 — A Self-Bootstrapping Application Specification

> Design goal: **A command-line tool that bridges human conversational exploration of system design with LLM-optimized formal specification, enabling transactional parsing, validation, search, and change intelligence over DDIS-conforming documents.**

> Core promise: Given any DDIS-conforming specification (monolith or modular), the CLI provides a 30-table indexed representation that supports round-trip parsing, 12+ mechanical validation checks, hybrid BM25/LSI/PageRank search with RRF fusion, 9-signal context bundles for LLM consumption, structural diffing, impact analysis, and an append-only operation log — all deterministic, all offline, all in a single SQLite file.

> Self-bootstrapping note (important):
> This specification IS validated by the tool it describes.
> The DDIS CLI parses, indexes, and validates this document.
> Where this document prescribes a behavior, the CLI implements that behavior — and can verify this document conforms to the standard the CLI enforces.
> This is not circular: the spec came first, the tool implements it, and then the tool validates the spec. Quality Gate APP-G-6 makes this mechanical.

---

# PART 0: EXECUTIVE BLUEPRINT

## 0.1 Executive Summary

The DDIS CLI is a Go command-line tool that makes DDIS-conforming specifications machine-queryable. It parses markdown specifications into a normalized 30-table SQLite index, then provides 13 commands for querying, validating, searching, diffing, and analyzing change impact across specification elements.

The primary purpose is to close the loop between three activities that currently require manual effort:

1. **Human exploration** — an engineer writes and evolves a specification through conversational iteration
2. **Formal indexing** — the CLI parses the spec into a structured, cross-referenced index
3. **LLM consumption** — the CLI assembles context bundles (9 intelligence signals) that give an LLM implementer exactly the information it needs for a given task

13 commands span four domains: *parsing* (`parse`, `render`, `seed`), *querying and validation* (`query`, `validate`, `diff`), *search and intelligence* (`search`, `context`, `impact`), and *lifecycle operations* (`log`, `tx begin`, `tx commit`, `tx rollback`).

### 0.1.1 Non-Negotiables (Engineering Contract)

These are not aspirations; they are the contract. A conforming implementation MUST satisfy all of them.

- **Round-trip fidelity is exact.** `parse` followed by `render` MUST produce byte-identical output for any valid DDIS specification. No whitespace normalization, no reordering, no "close enough." (APP-INV-001)

- **Validation is deterministic.** The same spec MUST produce the same validation report regardless of wall-clock time, random seed, or execution environment. (APP-INV-002)

- **Cross-references are resolved or reported.** Every cross-reference (`INV-NNN`, `ADR-NNN`, `§X.Y`) MUST resolve to an indexed element, or appear in the validation report as broken. No silent orphans. (APP-INV-003)

- **The oplog is append-only.** No record modification, no deletion, no rewriting. The operation log survives database recreation. (APP-INV-010)

- **Context bundles are self-contained.** An LLM receiving a context bundle MUST be able to implement the targeted subsystem without reaching for information outside the bundle. (APP-INV-005)

- **Search scores are mechanically derivable.** Every RRF fusion score MUST equal the correctly computed formula. No opaque relevance. (APP-INV-008)

## 0.2 Formal State Space Model

The system state is a 5-tuple:

```
S = (SpecFiles, Index, SearchState, OpLog, TxState)

where:
  SpecFiles   = MarkdownFile | (Manifest * ModuleFile*)
  Index       = SQLiteDB(30 tables)
  SearchState = FTSIndex * LSIModel * AuthorityScores
  OpLog       = JSONL(DiffRecord | ValidateRecord | TxRecord)*
  TxState     = Map(TxID -> {pending | committed | rolled_back})
```

### 0.2.1 State Transitions (13 Commands)

Each command is a transition function over the state space:

```
T_parse:       SpecFiles -> Index * SearchState
T_render:      Index -> SpecFiles                     (inverse of T_parse)
T_query:       Index * Target -> Fragment
T_validate:    Index -> Report(12+ checks)
T_diff:        Index * Index -> DeltaResult
T_impact:      Index * Target * Dir * Depth -> Graph
T_search:      Index * Query * Opts -> RankedResults
T_context:     Index * Target -> Bundle(9 signals)
T_tx_begin:    Index -> Index * TxState(pending)
T_tx_commit:   Index * TxState -> Index * OpLog
T_tx_rollback: Index * TxState -> Index
T_seed:        Index -> OpLog(genesis)
T_log:         OpLog * Filters -> FormattedRecords
```

### 0.2.2 Key Composition: T_context

The `context` command is the most complex transition because it composes nearly all others:

```
T_context = T_query
          . T_find_constraints
          . T_check_completeness
          . T_find_gaps
          . T_validate_local
          . T_tag_modes
          . T_lsi_similar
          . T_impact
          . T_oplog_recent
          . T_generate_guidance
```

This composition produces a bundle with 9 intelligence signals:
1. **Fragment** — the queried element's full content
2. **Constraints** — invariants and ADRs governing this element
3. **Completeness** — which required sub-elements are present/missing
4. **Gaps** — what the element lacks relative to DDIS requirements
5. **Local validation** — validation check results scoped to this element
6. **Mode tags** — which DDIS modes (meta-standard, domain-spec) apply
7. **LSI similar** — semantically related elements by cosine similarity
8. **Impact graph** — upstream/downstream dependency subgraph
9. **Guidance** — synthesized implementation guidance from all signals

### 0.2.3 Critical Invariant: Round-Trip

The parse-render round-trip is the system's foundational correctness property:

```
forall spec in ValidDDIS:
  render(parse(spec)) = spec          (byte-identical)

forall spec in ValidDDIS:
  parse(monolith(spec)) = parse(assemble(modules(spec)))  (equivalence)
```

Violation of the first property means the tool corrupts specifications. Violation of the second means monolith and modular forms are not interchangeable. Both are non-negotiable. (APP-INV-001, APP-INV-009)

### 0.2.4 The 4-Pass Parse Pipeline

Parsing is not a single function but a 4-pass pipeline, each pass producing a richer representation:

```
Pass 1 (Tree):     Markdown -> HeadingTree(sections, levels, bodies)
Pass 2 (Elements): HeadingTree -> TypedElements(invariants, ADRs, gates, ...)
Pass 3 (XRefs):    TypedElements -> CrossRefGraph(source, target, type)
Pass 4 (Resolve):  CrossRefGraph -> ResolvedIndex(validated references)
```

Each pass is independently testable. The pipeline is deterministic: same input always produces same output at every pass. (APP-ADR-009)

---

## 0.3 Invariant Registry (Declarations)

All 16 application invariants. Full definitions with formal expressions, violation scenarios, and validation methods are in the owning module. Each invariant starts at `Confidence: falsified`.

**APP-INV-001: Round-Trip Fidelity** (Owner: parse-pipeline)
Parse followed by render MUST produce byte-identical output for any valid DDIS specification.
Confidence: falsified
*Violation: parse a spec with trailing whitespace in a heading; render drops the whitespace.*

**APP-INV-002: Validation Determinism** (Owner: query-validation)
Validation results MUST be independent of wall-clock time, random number generator state, and check execution order.
Confidence: falsified
*Violation: a check uses `time.Now()` for staleness detection; results change across runs.*

**APP-INV-003: Cross-Reference Integrity** (Owner: query-validation)
Every resolved cross-reference MUST point to an existing indexed element.
Confidence: falsified
*Violation: an `INV-NNN` reference is indexed but the target invariant section was deleted.*

**APP-INV-004: Authority Monotonicity** (Owner: search-intelligence)
Adding a relevant cross-reference to a specification element can only increase that element's authority score, never decrease it.
Confidence: falsified
*Violation: adding an inbound link causes PageRank redistribution that lowers the target.*

**APP-INV-005: Context Self-Containment** (Owner: search-intelligence)
A context bundle MUST include all 9 intelligence signals such that an LLM can implement the targeted subsystem without external information.
Confidence: falsified
*Violation: the bundle omits the "constraints" signal; the LLM implements without checking invariants.*

**APP-INV-006: Transaction State Machine** (Owner: lifecycle-ops)
Transaction state transitions MUST follow only `pending -> committed` or `pending -> rolled_back`. No other transitions are valid.
Confidence: falsified
*Violation: a committed transaction is rolled back, reverting changes that downstream operations depend on.*

**APP-INV-007: Diff Completeness** (Owner: query-validation)
Structural diff MUST report every addition, removal, and modification between two specification indices. No silent drops.
Confidence: falsified
*Violation: a section body changes but the diff only compares headings; the modification is unreported.*

**APP-INV-008: RRF Fusion Correctness** (Owner: search-intelligence)
The RRF fusion score MUST equal the correctly computed `SUM(1/(K + rank_r(d)) * weight_r)` for all scoring signals r.
Confidence: falsified
*Violation: integer division truncates `1/(K + rank)` to zero for ranks > K.*

**APP-INV-009: Monolith-Modular Equivalence** (Owner: parse-pipeline)
Parsing a monolith specification MUST produce an index equivalent to parsing the assembled modular form of the same specification.
Confidence: falsified
*Violation: modular parsing assigns different section IDs because module boundaries introduce extra heading levels.*

**APP-INV-010: Oplog Append-Only** (Owner: lifecycle-ops)
The operation log MUST be append-only. No existing record may be modified or deleted after write.
Confidence: falsified
*Violation: `tx rollback` deletes the corresponding `tx begin` record from the oplog.*

**APP-INV-011: Check Composability** (Owner: query-validation)
Running a subset of validation checks MUST produce results identical to running all checks and filtering to that subset. Checks MUST NOT have inter-check dependencies.
Confidence: falsified
*Violation: Check 7 (cross-ref web) depends on Check 3 (element extraction) having populated a cache.*

**APP-INV-012: LSI Dimension Bound** (Owner: search-intelligence)
The LSI model's k-dimension parameter MUST be at most the document count, and all term vectors MUST have exactly k dimensions.
Confidence: falsified
*Violation: k is set to 100 but the spec has only 40 sections; SVD produces vectors with 40 dimensions but code assumes 100.*

**APP-INV-013: Impact Termination** (Owner: lifecycle-ops)
Impact analysis BFS MUST visit each node at most once. Cycles in the cross-reference graph MUST NOT cause infinite traversal.
Confidence: falsified
*Violation: INV-001 references ADR-001 which references INV-001; BFS loops indefinitely.*

**APP-INV-014: Glossary Expansion Bound** (Owner: search-intelligence)
Query expansion via glossary matching MUST add at most 5 terms to any single query.
Confidence: falsified
*Violation: a query term matches a glossary entry whose definition contains another glossary term, causing recursive expansion beyond 5.*

**APP-INV-015: Deterministic Hashing** (Owner: parse-pipeline)
Content hashes MUST be computed via SHA-256 with no salt, producing identical output for identical input across all platforms.
Confidence: falsified
*Violation: the hash function includes a timestamp or process ID in the input.*

**APP-INV-016: Implementation Traceability** (Owner: lifecycle-ops)
Every invariant that claims implementation status MUST have valid `Source`, `Tests`, and `Validates-via` file paths that exist and are non-empty.
Confidence: falsified
*Violation: an invariant's `Tests` path points to a file that was renamed; the path is stale.*

---

## 0.4 Invariant Confidence Levels

Invariants progress through confidence levels as evidence accumulates. Each level strictly subsumes the previous.

| Level | Meaning | Required Evidence |
|---|---|---|
| `falsified` | Has a concrete violation scenario; no automated verification yet | Written violation scenario (required for all APP-INVs at declaration) |
| `property-checked` | Go tests exercise the invariant on representative inputs | `Tests:` annotation points to passing `_test.go` files |
| `bounded-verified` | Property-based or fuzz tests explore the input space | Randomized/generated inputs with coverage metrics |
| `proven` | Mechanically verified correct (future goal) | Proof artifact or formal verification tool output |

Confidence levels are tracked per-invariant in the implementation. The `validate` command's Check 13 (APP-INV-016) verifies that claimed confidence levels have corresponding evidence at the declared paths.

---

## 0.5 ADR Registry (Declarations)

All 11 architecture decision records. Full specifications with Problem, Options, Decision, WHY NOT, Consequences, and Tests are in the implementing module.

**APP-ADR-001: Go over Rust** (Implements: parse-pipeline)
Decision: Go for CLI implementation. The workload is I/O-bound (SQLite reads/writes, file parsing), not CPU-bound. Go's fast compilation supports rapid iteration in the RALPH improvement loop. A pure-Go SQLite driver (`modernc.org/sqlite`) eliminates CGO complexity.
WHY NOT Rust: Longer compilation cycles slow the RALPH loop. The performance ceiling of Rust is unnecessary for a specification tool.

**APP-ADR-002: SQLite with Pure-Go Driver** (Implements: parse-pipeline)
Decision: Single-file SQLite database via `modernc.org/sqlite`. No external database server. The index is a derived artifact — deletable and recreatable from the spec files.
WHY NOT PostgreSQL: Unnecessary operational complexity for a single-user CLI tool.

**APP-ADR-003: BM25 + LSI + PageRank with RRF Fusion (K=60)** (Implements: search-intelligence)
Decision: Three orthogonal scoring signals fused via Reciprocal Rank Fusion with K=60. BM25 for lexical matching (via FTS5), LSI for semantic similarity, PageRank for structural authority. All signals are offline and deterministic.
WHY NOT embedding models: Requires a runtime dependency on an inference engine. Violates offline/deterministic constraint.

**APP-ADR-004: Cobra CLI Framework** (Implements: query-validation)
Decision: Cobra for command parsing and help generation. De facto standard for Go CLIs. Provides subcommand routing, flag parsing, shell completion.
WHY NOT bare `flag` package: No subcommand support without manual routing.

**APP-ADR-005: 30-Table Normalized Schema** (Implements: parse-pipeline)
Decision: Fully normalized relational schema with 30 tables. Cross-reference queries require joins but gain referential integrity. Tables include: sections, invariants, adrs, gates, glossary_terms, cross_refs, fts_content, lsi_vectors, authority_scores, and 21 supporting tables.
WHY NOT document store: Cross-reference graph queries require relational joins.

**APP-ADR-006: Context Bundles as Compound Intelligence (9 Signals)** (Implements: search-intelligence)
Decision: The `context` command assembles 9 distinct intelligence signals into a single bundle. Each signal is independently computable but jointly they provide complete implementation context for an LLM.
WHY NOT single-query output: Insufficient for LLM consumption without constraints, gaps, and impact context.

**APP-ADR-007: JSONL Oplog** (Implements: lifecycle-ops)
Decision: Append-only JSONL file for operation logging. Survives database recreation (the database is derived; the oplog is primary). Each line is a self-contained JSON record with timestamp, operation type, and payload.
WHY NOT SQLite table: The oplog must survive `parse --force` which recreates the database.

**APP-ADR-008: Surgical Edit over Full Rewrite in RALPH Apply** (Implements: lifecycle-ops)
Decision: The RALPH loop's Apply phase uses surgical edits (targeted modifications to specific sections) rather than regenerating entire specification files. This preserves human-authored content and reduces the blast radius of automated changes.
WHY NOT full rewrite: Risks losing nuance in human-authored prose; larger blast radius.

**APP-ADR-009: 4-Pass Parse Pipeline** (Implements: parse-pipeline)
Decision: Parsing proceeds in 4 sequential passes: tree construction, element extraction, cross-reference detection, reference resolution. Each pass produces an independently testable intermediate representation.
WHY NOT single-pass: Cross-references cannot be resolved until all elements are extracted. Single-pass requires backpatching, which is error-prone.

**APP-ADR-010: Monolith/Modular Polymorphism by Filename Detection** (Implements: parse-pipeline)
Decision: The parser detects monolith vs. modular input by checking for the presence of `manifest.yaml`. Monolith input is a single markdown file. Modular input is a manifest plus module files. Both produce the same index structure. (APP-INV-009)
WHY NOT explicit flag: User burden; easy to forget.

**APP-ADR-011: Structured Intent over Formal Derivation** (Implements: lifecycle-ops)
Decision: Invariant implementation traces use structured fields (`Source`, `Tests`, `Validates-via`) rather than formal proof derivations. This matches the current state of practice: Go tests, not theorem provers.
WHY NOT formal proofs: The Go ecosystem lacks mature formal verification tooling. Structured traces provide 80% of the traceability value at 10% of the cost.

---

## 0.6 Quality Gates (Declarations)

A conforming implementation is "done" when all quality gates pass. Gates are ordered by priority; a failing Gate N makes Gates N+1 through 6 irrelevant.

| Gate | Name | Validates | Check Type |
|------|------|-----------|------------|
| Gate | Name | Validates | Check Type |
| Gate-1 | Structural Conformance | All 13 commands accept expected inputs and produce expected output shapes | Mechanical (integration tests) |
| Gate-2 | Causal Chain | Every command traces through an APP-ADR or APP-INV to the formal state model (§0.2) | Sampling (5 commands) |
| Gate-3 | Decision Coverage | All design choices have corresponding APP-ADRs; no undocumented design decisions | Adversarial review |
| Gate-4 | Invariant Falsifiability | Each APP-INV has a concrete violation scenario and at least one test exercising it | Constructive (test audit) |
| Gate-5 | Cross-Reference Web | No orphan sections in the specification; every section has inbound or outbound references | Graph analysis (the CLI itself can check this) |
| Gate-6 | Self-Validation | The CLI successfully parses, indexes, and validates its own specification with zero errors | Mechanical (`ddis validate ddis-cli-spec/`) |

**Gate 1: Structural Conformance**
All 13 commands accept expected inputs and produce expected output shapes. Tested mechanically via integration tests.

**Gate 2: Causal Chain**
Every command traces through an APP-ADR or APP-INV to the formal state model (§0.2). Verified by sampling 5 commands.

**Gate 3: Decision Coverage**
All design choices have corresponding APP-ADRs; no undocumented design decisions. Verified by adversarial review.

**Gate 4: Invariant Falsifiability**
Each APP-INV has a concrete violation scenario and at least one test exercising it. Verified constructively via test audit.

**Gate 5: Cross-Reference Web**
No orphan sections in the specification; every section has inbound or outbound references. Verified by the CLI's own cross-reference density check.

**Gate 6: Self-Validation**
The CLI successfully parses, indexes, and validates its own specification with zero errors. This is the self-bootstrapping gate — it closes the loop: the spec describes the tool, the tool validates the spec. If the spec is invalid under its own tool's rules, either the spec or the tool has a bug — both must be fixed.

### Definition of Done (for this specification)

DDIS CLI Spec v1.0 is "done" when:
- All 6 quality gates pass
- All 16 APP-INVs are at least `property-checked` confidence
- The CLI parses and validates this spec with zero errors (APP-G-6)
- At least one non-trivial DDIS spec (the meta-standard itself) has been validated by the CLI

---

## 0.7 Glossary

Terms specific to the DDIS CLI. If a term is also defined in the DDIS standard, the CLI-specific definition here describes how the CLI represents or computes it.

| Term | Definition |
|---|---|
| **Authority Score** | PageRank-derived measure of a specification element's structural importance, computed from the cross-reference graph. Higher authority = more elements depend on this one. (APP-ADR-003) |
| **BM25** | Best Matching 25 — probabilistic lexical relevance scoring function. Implemented via SQLite FTS5 `bm25()`. One of three signals in the RRF fusion. |
| **Bundle** | See Context Bundle. |
| **Confidence Level** | One of four evidence levels for invariant implementation: `falsified`, `property-checked`, `bounded-verified`, `proven`. (§0.4) |
| **Context Bundle** | The output of the `context` command: a compound artifact containing 9 intelligence signals assembled for LLM consumption. (APP-ADR-006, APP-INV-005) |
| **Deep Context** | Reserved manifest field for future cross-module context that cannot be derived from the constitution alone. Currently null for all modules. |
| **Fragment** | A subset of the indexed specification returned by the `query` command. May be a single section, an invariant, an ADR, or a filtered projection. |
| **FTS5** | SQLite Full-Text Search extension version 5. Provides the BM25 scoring signal and powers the `search` command's lexical matching. |
| **Implementation Trace** | Structured evidence linking an invariant to its implementation: `Source` (Go file), `Tests` (test file), `Validates-via` (validation method). (APP-INV-016, APP-ADR-011) |
| **Impact Graph** | A directed subgraph of the cross-reference web showing upstream dependencies and downstream dependents of a target element, bounded by depth. Output of the `impact` command. |
| **Index** | The 30-table SQLite database produced by `parse`. Contains all typed elements, cross-references, search indices, and authority scores. A derived artifact — deletable and recreatable. (APP-ADR-005) |
| **LSI** | Latent Semantic Indexing — dimensionality reduction via truncated SVD on the term-document matrix. Produces semantic similarity scores independent of lexical overlap. (APP-ADR-003, APP-INV-012) |
| **OpLog** | Append-only JSONL file recording all CLI operations: parse, validate, diff, transaction begin/commit/rollback. Survives database recreation. (APP-ADR-007, APP-INV-010) |
| **PageRank** | Iterative authority scoring algorithm applied to the cross-reference graph. Elements with many high-authority inbound references score higher. (APP-ADR-003, APP-INV-004) |
| **RRF** | Reciprocal Rank Fusion — method for combining ranked lists from multiple scoring signals. Score = `SUM(1/(K + rank_r(d)) * weight_r)` with K=60. (APP-INV-008) |
| **Seed** | The `seed` command creates a genesis oplog record for a newly parsed specification. Establishes the baseline for subsequent diff and change tracking. |
| **Transaction** | An atomic unit of specification modification. States: `pending`, `committed`, `rolled_back`. State machine enforced by APP-INV-006. |

---

## 0.8 Section Map

Cross-reference lookup: which module file contains each section's full specification.

| Section Range | Module File | Notes |
|---|---|---|
| §0.1-§0.8, APP-INV/ADR/Gate declarations, Glossary | constitution/system.md | Cross-cutting: included in every bundle |
| 4-pass pipeline, schema design, round-trip, hashing, monolith/modular detection | modules/parse-pipeline.md | Owns: APP-INV-001, -009, -015. Implements: APP-ADR-001, -002, -005, -009, -010 |
| BM25/LSI/PageRank, RRF fusion, context bundles, glossary expansion, authority scoring | modules/search-intelligence.md | Owns: APP-INV-004, -005, -008, -012, -014. Implements: APP-ADR-003, -006 |
| 12+ validation checks, cross-ref resolution, structural diff, query projection | modules/query-validation.md | Owns: APP-INV-002, -003, -007, -011. Implements: APP-ADR-004 |
| Transaction state machine, oplog, impact BFS, implementation tracing, seed | modules/lifecycle-ops.md | Owns: APP-INV-006, -010, -013, -016. Implements: APP-ADR-007, -008, -011 |

---

## 0.9 Non-Goals

This specification explicitly does NOT attempt:

1. **Code generation from spec.** The CLI indexes and validates specifications. It does not generate Go, Rust, or any other implementation code from spec content. Code generation is a separate tool concern.

2. **Formal proof artifacts.** Invariant confidence levels include `proven` as a future goal, but this specification does not prescribe a formal verification toolchain. Structured implementation traces (APP-ADR-011) are the current ceiling.

3. **Modifying existing CLI commands.** This spec describes the 13 commands as implemented. The only addition is Check 13 (implementation traceability) to the validation suite, which extends rather than modifies the `validate` command.

4. **RALPH loop code changes.** The RALPH loop (`ddis_ralph_loop.sh`) is a consumer of the CLI, not a component of it. This spec does not prescribe changes to the RALPH loop's architecture.

5. **Cross-spec composability testing.** Validating references between two different DDIS-conforming specifications (e.g., `[OtherSpec]:INV-001`) is deferred. The CLI validates references within a single specification.

---

## Context Budget

> Authoritative values are in `manifest.yaml`. Replicated here for LLM orientation. If values diverge, the manifest is authoritative.

```
target_lines: 4000
hard_ceiling_lines: 5000
reasoning_reserve: 0.25
```

---

## Module Map

| Module | Domain | Contents |
|---|---|---|
| **parse-pipeline** | parsing | 4-pass parse pipeline (tree, elements, xrefs, resolve), 30-table schema design, render engine, monolith/modular detection, content hashing. APP-INV-001, -009, -015. APP-ADR-001, -002, -005, -009, -010. |
| **search-intelligence** | search | BM25/FTS5 integration, LSI model (truncated SVD), PageRank computation, RRF fusion (K=60), context bundle assembly (9 signals), glossary expansion. APP-INV-004, -005, -008, -012, -014. APP-ADR-003, -006. |
| **query-validation** | validation | Query projection, 12+ validation checks (composable), cross-reference resolution, structural diff, Cobra command routing. APP-INV-002, -003, -007, -011. APP-ADR-004. |
| **lifecycle-ops** | lifecycle | Transaction state machine (begin/commit/rollback), JSONL oplog (append-only), impact BFS with cycle protection, seed command, implementation traceability (Check 13). APP-INV-006, -010, -013, -016. APP-ADR-007, -008, -011. |
