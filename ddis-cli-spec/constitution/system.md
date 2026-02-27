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

> Core promise: Given any DDIS-conforming specification (monolith or modular), the CLI provides a 39-table indexed representation that supports round-trip parsing, 14 mechanical validation checks, hybrid BM25/LSI/PageRank search with RRF fusion, 9-signal context bundles for LLM consumption, structural diffing, impact analysis, and an append-only operation log — all deterministic, all offline, all in a single SQLite file.

> Self-bootstrapping note (important):
> This specification IS validated by the tool it describes.
> The DDIS CLI parses, indexes, and validates this document.
> Where this document prescribes a behavior, the CLI implements that behavior — and can verify this document conforms to the standard the CLI enforces.
> This is not circular: the spec came first, the tool implements it, and then the tool validates the spec. Quality Gate APP-G-6 makes this mechanical.

---

# PART 0: EXECUTIVE BLUEPRINT

## 0.1 Executive Summary

The DDIS CLI is a Go command-line tool that makes DDIS-conforming specifications machine-queryable and machine-improvable. It parses markdown specifications into a normalized 39-table SQLite index, then provides commands for querying, validating, searching, diffing, analyzing change impact, detecting contradictions, bridging spec to code, and orchestrating bilateral specification workflows where humans think and LLMs formalize.

The primary purpose is to close the loop between four activities that currently require manual effort:

1. **Human exploration** — an engineer discovers and evolves a specification through conversational iteration (`ddis discover`)
2. **Formal indexing** — the CLI parses the spec into a structured, cross-referenced index (`ddis parse`, `ddis validate`)
3. **LLM consumption** — the CLI assembles context bundles that give an LLM implementer exactly the information it needs (`ddis context`, `ddis search`)
4. **Bilateral feedback** — implementation speaks back into the spec via code annotation scanning and automated absorption (`ddis scan`, `ddis absorb`, `ddis drift`)

Commands span seven domains: *parsing* (`parse`, `render`, `seed`), *querying and validation* (`query`, `validate`, `diff`), *search and intelligence* (`search`, `context`, `impact`, `exemplar`), *lifecycle operations* (`log`, `tx`, `coverage`, `skeleton`, `checkpoint`, `cascade`, `bundle`, `impl-order`, `checklist`, `progress`, `state`, `drift`), *code bridge* (`scan`, `history`), *auto-prompting* (`discover`, `refine`, `absorb`), and *workspace* (`init`, `spec`, `tasks`).

### 0.1.1 Non-Negotiables (Engineering Contract)

These are not aspirations; they are the contract. A conforming implementation MUST satisfy all of them.

- **Round-trip fidelity is exact.** `parse` followed by `render` MUST produce byte-identical output for any valid DDIS specification. No whitespace normalization, no reordering, no "close enough." (APP-INV-001)

- **Validation is deterministic.** The same spec MUST produce the same validation report regardless of wall-clock time, random seed, or execution environment. (APP-INV-002)

- **Cross-references are resolved or reported.** Every cross-reference (`INV-NNN`, `ADR-NNN`, `§X.Y`) MUST resolve to an indexed element, or appear in the validation report as broken. No silent orphans. (APP-INV-003)

- **The oplog is append-only.** No record modification, no deletion, no rewriting. The operation log survives database recreation. (APP-INV-010)

- **Context bundles are self-contained.** An LLM receiving a context bundle MUST be able to implement the targeted subsystem without reaching for information outside the bundle. (APP-INV-005)

- **Search scores are mechanically derivable.** Every RRF fusion score MUST equal the correctly computed formula. No opaque relevance. (APP-INV-008)

## 0.2 Formal State Space Model

The system state is an 8-tuple:

```
S = (SpecFiles, Index, SearchState, OpLog, TxState, EventStreams, DiscoveryState, Workspace)

where:
  SpecFiles      = MarkdownFile | (Manifest * ModuleFile*)
  Index          = SQLiteDB(39 tables)
  SearchState    = FTSIndex * LSIModel * AuthorityScores
  OpLog          = JSONL(DiffRecord | ValidateRecord | TxRecord)*
  TxState        = Map(TxID -> {pending | committed | rolled_back})
  EventStreams   = (DiscoveryJSONL * SpecJSONL * ImplJSONL)    -- three-stream event sourcing
  DiscoveryState = ThreadTopology * ArtifactMap * ConfidenceVector * OpenQuestions
  Workspace      = Map(SpecID -> {manifest_path, parent_spec, related_specs, drift_score})
```

### 0.2.1 State Transitions (30 Commands)

Each command is a transition function over the state space:

```
-- Parsing domain
T_parse:       SpecFiles -> Index * SearchState
T_render:      Index -> SpecFiles                     (inverse of T_parse)
T_seed:        Index -> OpLog(genesis)

-- Query and validation domain
T_query:       Index * Target -> Fragment
T_validate:    Index * Opts -> Report(14 checks)
T_diff:        Index * Index -> DeltaResult
T_exemplar:    Index * Target -> ExemplarResult

-- Search and intelligence domain
T_search:      Index * Query * Opts -> RankedResults
T_context:     Index * Target -> Bundle(9 signals)
T_impact:      Index * Target * Dir * Depth -> Graph

-- Lifecycle domain
T_tx_begin:    Index -> Index * TxState(pending)
T_tx_commit:   Index * TxState -> Index * OpLog
T_tx_rollback: Index * TxState -> Index
T_log:         OpLog * Filters -> FormattedRecords
T_coverage:    Index -> CoverageReport
T_skeleton:    SkeletonOpts -> SpecFiles
T_checkpoint:  Index -> GateResults
T_cascade:     Index * ModuleID -> CascadeResult
T_bundle:      Index * Domain -> BundledSpec
T_implorder:   Index -> TopologicalOrder
T_checklist:   Index * Target -> ChecklistResult
T_progress:    Index -> FrontierReport(done/ready/blocked)
T_state:       Index * Key * Value? -> Value | void
T_drift:       Index * CodeRoot? -> DriftReport

-- Code bridge domain
T_scan:        CodeRoot * Index -> ScanResult * EventStreams'
T_contradict:  Index * Opts -> ContradictionReport
T_history:     EventStreams * Filters -> UnifiedTimeline

-- Auto-prompting domain (state monad: returns CommandResult)
T_next:        Workspace -> CommandResult
T_discover:    Index * DiscoveryState -> CommandResult * DiscoveryState'
T_refine:      Index * OpLog -> CommandResult * Index'
T_absorb:      CodeRoot * Index -> CommandResult * SpecFiles'

-- Workspace domain
T_init:        EmptyDir -> SpecFiles * Index * EventStreams * Workspace
T_spec:        Workspace * ManifestPath -> Workspace'
T_tasks:       DiscoveryState * Index -> TaskList

-- Witness domain
T_witness:     Index * InvariantID -> WitnessReceipt
T_challenge:   Index * InvariantID * CodeRoot? -> ChallengeResult
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

All 56 application invariants. Full definitions with formal expressions, violation scenarios, and validation methods are in the owning module. Each invariant starts at `Confidence: falsified`.

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

**APP-INV-017: Annotation Portability** (Owner: code-bridge)
The annotation grammar is parseable in any programming language that supports single-line comments. No language-specific AST parsers required.
Confidence: falsified
*Violation: scanner misses Python `#`-style comments; annotations silently disappear from scan results.*

**APP-INV-018: Scan-Spec Correspondence** (Owner: code-bridge)
When `ddis scan --verify` is run with a spec database, every annotation target must resolve to an existing spec element. Orphaned and unimplemented elements reported.
Confidence: falsified
*Violation: `// ddis:maintains INV-XYZ` stored without validation; link is orphaned but not reported.*

**APP-INV-019: Contradiction Graph Soundness** (Owner: code-bridge)
Every reported contradiction represents a genuine logical conflict. False positives are not acceptable at any tier.
Confidence: falsified
*Violation: detector flags structural redundancy (INV-018) as contradicting signal-to-noise (INV-007); it's not a real conflict.*

**APP-INV-020: Event Stream Append-Only** (Owner: code-bridge)
JSONL event streams are strictly append-only with monotonically increasing timestamps. No modification, deletion, or reordering after write.
Confidence: falsified
*Violation: `ddis parse --force` recreates the DB and loses all historical events.*

**APP-INV-021: Consistency Encoding Fidelity** (Owner: code-bridge)
Propositional encodings generated from `semi_formal` fields faithfully represent the logical content. UNSAT results from SAT solving correspond to genuine inconsistency. (Updated: Z3 CGo dependency superseded by pure-Go gophersat per APP-ADR-034.)
Confidence: falsified
*Violation: two unrelated invariants mapped to the same propositional variable; false contradiction reported.*

**APP-INV-022: Refinement Drift Monotonicity** (Owner: auto-prompting)
Each iteration of `ddis refine` must produce a measurable drift reduction. Drift monotonically decreases; regression halts the loop. Extends INV-022 from parent spec.
Confidence: falsified
*Violation: new invariant introduces unresolved cross-ref; drift increases but loop continues.*

**APP-INV-023: Prompt Self-Containment** (Owner: auto-prompting)
Every generated prompt contains all context needed for the LLM to act. No implicit dependencies on prior turns or environment.
Confidence: falsified
*Violation: prompt references "the invariant from the previous iteration" without including its text.*

**APP-INV-024: Ambiguity Surfacing** (Owner: auto-prompting)
When the refine loop detects unresolved design decisions, these ambiguities are surfaced to the user. The loop does not resolve ambiguities autonomously.
Confidence: falsified
*Violation: system silently resolves tension between INV-007 and INV-018 without user input.*

**APP-INV-025: Discovery Provenance Chain** (Owner: auto-prompting)
Every crystallized artifact has a complete provenance chain in the event stream from root question/finding to crystallization.
Confidence: falsified
*Violation: ADR exists in spec but has no provenance in discovery JSONL; invisible to task generation.*

**APP-INV-026: Classification Non-Prescriptive** (Owner: auto-prompting)
The cognitive mode classification layer observes and tags — it never prescribes, directs, or constrains the user's thinking.
Confidence: falsified
*Violation: after 5 divergent events, system suggests "Consider narrowing your focus" — a prescription.*

**APP-INV-027: Thread Topology Primacy** (Owner: auto-prompting)
Inquiry threads are the primary organizational unit, not sessions. A single thread may span sessions, LLMs, and humans.
Confidence: falsified
*Violation: events scoped by session; caching exploration split across sessions with interleaved authentication events.*

**APP-INV-028: Spec-as-Trunk** (Owner: auto-prompting)
Every discovery thread branches from the specification and crystallizes back into it. No orphan threads that bypass spec integration.
Confidence: falsified
*Violation: thread marked "merged" but no artifacts written to spec; decisions invisible to downstream tools.*

**APP-INV-029: Convergent Thread Selection** (Owner: auto-prompting)
Thread attachment is inferred from conversation content, never forced. User override via `--thread` always available but never required.
Confidence: falsified
*Violation: exact keyword matching misses related thread about "TTL-based expiration" when user discusses "cache invalidation."*

**APP-INV-030: Contributor Topology Graceful Degradation** (Owner: auto-prompting)
Contributor topology degrades gracefully: multi-author → temporal self-disagreement → skip. No core feature depends on git blame.
Confidence: falsified
*Violation: `ddis discover` crashes with "fatal: not a git repository" when git unavailable.*

**APP-INV-031: Absorbed Artifacts Validate** (Owner: auto-prompting)
Every artifact produced by `ddis absorb` passes `ddis validate`. No syntactically invalid spec output.
Confidence: falsified
*Violation: absorbed invariant missing `Violation scenario:` component; validate Check 2 fails.*

**APP-INV-032: Symmetric Reconciliation** (Owner: auto-prompting)
`ddis absorb --against` reports gaps in both directions: undocumented behavior AND unimplemented specification. Neither direction privileged.
Confidence: falsified
*Violation: reconciliation only reports what code does that spec doesn't mention; missed 5 unimplemented spec invariants.*

**APP-INV-033: Absorption Format Parity** (Owner: auto-prompting)
Absorbed specs are structurally indistinguishable from hand-written specs. Only provenance metadata differs.
Confidence: falsified
*Violation: absorbed invariants have only statements, no violation scenarios; visible quality gap.*

**APP-INV-034: State Monad Universality** (Owner: auto-prompting)
Every auto-prompting command returns `(output, state, guidance)`. No command produces output without guidance for the LLM interpreter.
Confidence: falsified
*Violation: `ddis refine audit` returns audit report but no guidance; LLM interpreter has no next-step hints.*

**APP-INV-035: Guidance Attenuation** (Owner: auto-prompting)
First invocation returns heavy guidance; subsequent invocations return light deltas. k\* guard prevents overprompting.
Confidence: falsified
*Violation: every invocation dumps full translation framework; by invocation 10, context is 40% guidance.*

**APP-INV-036: Human Format Transparency** (Owner: auto-prompting)
The human never needs to learn the spec format to use discovery. LLMs author specs; humans confirm crystallization.
Confidence: falsified
*Violation: system prompts user to "write the invariant in the following format: `**INV-NNN: Title** ...`"*

**APP-INV-037: Workspace Isolation** (Owner: workspace-ops)
Each spec in a multi-spec workspace is independently parseable. Removing one spec does not prevent parsing of others.
Confidence: falsified
*Violation: removing `data-spec` crashes `ddis parse api-spec/` because cross-spec resolution is not fault-tolerant.*

**APP-INV-038: Cross-Spec Reference Integrity** (Owner: workspace-ops)
Cross-spec references resolve correctly. Changed elements in referenced specs are flagged for review.
Confidence: falsified
*Violation: referenced INV-006 in meta-spec is amended but referencing API spec has no stale-reference warning.*

**APP-INV-039: Task Derivation Completeness** (Owner: workspace-ops)
Every artifact in a discovery artifact map generates appropriate tasks per the mechanical derivation rules. No artifact silently skipped.
Confidence: falsified
*Violation: invariant→task rule generates constraint task but not property test task; 3 invariants produce 3 tasks instead of 6.*

**APP-INV-040: Progressive Validation Monotonicity** (Owner: workspace-ops)
Validation maturity levels are strictly ordered: Level 1 ⊂ Level 2 ⊂ Level 3. A spec passing Level N passes all levels below N.
Confidence: falsified
*Violation: spec passes Level 2 (has invariants) but fails Level 1 (no overview); monotonicity broken.*

**APP-INV-041: Witness Auto-Invalidation** (Owner: lifecycle-ops)
When a spec is re-parsed and an invariant's content_hash changes, any witness with mismatched spec_hash is automatically set to stale_spec.
Confidence: falsified
*Violation: Invariant modified, re-parsed, but witness still shows valid with old hash.*

**APP-INV-042: Guidance Emission** (Owner: auto-prompting)
Every data command with non-empty findings emits at least one guidance hint.
Confidence: falsified
*Violation: `ddis validate` reports 5 failures but emits no guidance; the LLM has no next-step suggestion.*

**APP-INV-043: Invariant Statement Inline** (Owner: query-validation)
Every validation finding includes governing invariant statement inline.
Confidence: falsified
*Violation: A validation report shows "[FAIL] Check 3" but no invariant text; the LLM must run ddis context just to understand what was violated.*

**APP-INV-044: Warning Collapse** (Owner: query-validation)
No check produces >10 warning lines in text mode.
Confidence: falsified
*Violation: Check 3 produces 88 cross-reference density warnings, burying the actual failure diagnosis in noise.*

**APP-INV-045: Universal Auto-Discovery** (Owner: auto-prompting)
Every DB-reading command supports auto-discovery.
Confidence: falsified
*Violation: `ddis validate` requires an explicit path argument even when a manifest.ddis.db exists in the current directory.*

**APP-INV-046: Error Recovery Guidance** (Owner: auto-prompting)
Every error includes at least one recovery hint.
Confidence: falsified
*Violation: `ddis validate` fails with "no such table: spec_index" and no hint to run ddis parse first.*

**APP-INV-047: Frontmatter-Manifest Bijection** (Owner: query-validation)
Module frontmatter declarations (maintains, interfaces, implements, negative_specs) MUST be a bijection with manifest.yaml entries. Every frontmatter field matches its manifest counterpart exactly.
Confidence: falsified
*Violation: code-bridge.md frontmatter lists APP-INV-002 in interfaces but manifest.yaml omits it; parser silently uses one or the other depending on code path, producing inconsistent cross-reference graphs.*

**APP-INV-048: Behavioral Witness Ground Truth** (Owner: query-validation)
Behavioral tests annotated with ddis:tests produce ground truth witness evidence for invariant challenge verification.
Confidence: falsified

**APP-INV-049: Event Stream VCS Primacy** (Owner: lifecycle-ops)
Event streams are append-only JSONL files under VCS control, not database tables.
Confidence: falsified

**APP-INV-050: Challenge-Witness Adjunction Fidelity** (Owner: lifecycle-ops)
For every invariant with a valid witness, challenge must return a verdict. If refuted, witness is automatically invalidated.
Confidence: falsified

**APP-INV-051: Challenge-Informed Navigation** (Owner: lifecycle-ops)
The ddis next command must query challenge_results and incorporate verdict distribution into its recommendation.
Confidence: falsified

**APP-INV-052: Challenge-Driven Task Derivation** (Owner: lifecycle-ops)
The ddis tasks command must derive actionable work items from challenge verdicts.
Confidence: falsified

**APP-INV-053: Event Stream Completeness** (Owner: lifecycle-ops)
Every state-mutating CLI command must emit a typed event to the appropriate event stream.
Confidence: falsified

**APP-INV-054: LLM Provider Graceful Degradation** (Owner: code-bridge)
The LLM provider must degrade gracefully when no API key is configured.
Confidence: falsified

**APP-INV-055: Eval Evidence Statistical Soundness** (Owner: code-bridge)
The eval witness evidence type must use majority vote to produce statistically sound confidence scores.
Confidence: falsified

**APP-INV-056: Process Compliance Observability** (Owner: auto-prompting)
For every feature modification scope, the system computes a process compliance score PC in [0.0, 1.0] from available data sources. Missing data sources degrade to neutral (0.5), never block. Check 18 is warning-only.
Confidence: falsified
*Violation: agent writes code before spec, never uses ddis discover, but no signal fires and the context bundle omits any process compliance data.*

**APP-INV-057: External Tool Graceful Degradation** (Owner: workspace-ops)
When a ddis command depends on an external tool (e.g., gh), absence or failure of that tool must produce a clear, actionable error with recovery guidance — never a panic, silent failure, or cryptic exec error.
Confidence: falsified
*Violation: user runs ddis issue without gh installed and gets raw exec error instead of install guidance.*

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

All 43 architecture decision records. Full specifications with Problem, Options, Decision, WHY NOT, Consequences, and Tests are in the implementing module.

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

**APP-ADR-005: 39-Table Normalized Schema** (Implements: parse-pipeline)
Decision: Fully normalized relational schema with 39 tables. Cross-reference queries require joins but gain referential integrity. Tables include: spec_index, source_files, sections, invariants, adrs, quality_gates, glossary_terms, cross_references, modules, module_relationships, state_machine_cells, negative_specs, fts_content, lsi_vectors, search_model, search_authority, session_state, code_annotations, invariant_witnesses, and 20 supporting tables (oplog, transactions, implementation_map, context_signals, and indexes).
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

**APP-ADR-012: Annotations over Code Manifest** (Implements: code-bridge)
Decision: Inline `// ddis:maintains INV-006` annotations in source code, not a centralized `code_manifest.yaml`. Annotations travel with the code, are portable across all languages via comment syntax, and make traceability inspectable at point of implementation.
WHY NOT code manifest: A manifest is a declaration of intent, not proof of implementation. Manifest files drift from code independently.

**APP-ADR-013: Z3 as Required Dependency** (Implements: code-bridge) — **SUPERSEDED by APP-ADR-034**
Decision: ~~Z3 SMT solver is a required dependency (CGO via `go-z3`).~~ Superseded: all Go Z3 bindings require CGo, violating single-binary distribution. Replaced by tiered pure-Go consistency (APP-ADR-034): graph + gophersat SAT + heuristic NLP + LSI.
WHY SUPERSEDED: CGo breaks `curl | bash` distribution, mitchellh/go-z3 is archived, and the tiered pure-Go approach achieves ~80% detection at zero CGo cost.

**APP-ADR-014: Tiered Contradiction Detection** (Implements: code-bridge)
Decision: Four tiers running sequentially. Tier 1: existing structural validation (14 checks). Tier 2: graph-based contradiction detection (typed cross-ref edges, cycle detection, governance overlap via existing BFS/PageRank). Tier 3: SAT via gophersat (semi-formal → propositional encoding, UNSAT = contradiction, MUS extraction for conflict localization). Tier 4: heuristic NLP (polarity/quantifier/numeric rules) + existing LSI for semantic tension detection. All tier results merged with deduplication.
WHY NOT Z3: CGo violates single-binary distribution (APP-ADR-034). WHY NOT graph-only: Misses arithmetic contradictions. WHY NOT SAT-only: Semi-formal parsing covers ~80% of expressions; remaining fall to Tier 4 heuristics.

**APP-ADR-015: Three-Stream Event Sourcing** (Implements: code-bridge)
Decision: Three JSONL event streams: Stream 1 (discovery), Stream 2 (spec parse/validate/drift), Stream 3 (implementation). Cross-stream references via shared artifact IDs. Streams never write to each other.
WHY NOT single stream: Different domains have different schemas, consumers, and lifecycle semantics.

**APP-ADR-016: Auto-Prompting over Manual Prompting** (Implements: auto-prompting)
Decision: CLI generates context-rich prompts from spec state, drift data, and exemplars. Users can override with `--prompt-only`. Auto-prompting is the default; manual is the escape hatch.
WHY NOT manual-only: Requires deep familiarity with spec format and Gestalt optimization principles.

**APP-ADR-017: Gestalt Theory Integration** (Implements: auto-prompting)
Decision: Structural principles applied to prompt generation: demonstrations > constraints, spec-first framing, DoF separation per iteration, k\* overprompting guard. Embedded in logic, not user-facing config.
WHY NOT ignore Gestalt: Empirical evidence shows +3-4 quality points from spec-first framing alone.

**APP-ADR-018: Observation over Prescription** (Implements: auto-prompting)
Decision: Cognitive mode classification observes and tags, never prescribes. Labels inform prompt generation (mode-appropriate DoF) but are never directives.
WHY NOT prescriptive: Prescription destroys naturalism. The user's cognitive autonomy is non-negotiable.

**APP-ADR-019: Threads over Sessions** (Implements: auto-prompting)
Decision: Inquiry threads are the primary scoping unit. Sessions are substrate metadata. A thread may span sessions, LLMs, and humans.
WHY NOT sessions: Sessions are accidents of tooling. Cognitive coherence doesn't respect context window limits.

**APP-ADR-020: Conversational over Procedural** (Implements: auto-prompting)
Decision: Single `ddis discover` command. System loads context, converges on thread and mode during conversation. Old subcommands (explore, decide, risks) become internal classification events. Experience feels like resuming a conversation.
WHY NOT procedural: Forcing users to declare modes violates observation-over-prescription (APP-ADR-018).

**APP-ADR-021: Contributor Topology via Git Blame** (Implements: auto-prompting)
Decision: Use `git blame --porcelain` for per-section authorship. Surface cross-pollination opportunities and silent mental model disagreements. Graceful degradation: multi-author → temporal self-disagreement → skip.
WHY NOT ignore contributors: Structural validation misses epistemic incoherence — different mental models that pass all structural checks.

**APP-ADR-022: State Monad Architecture** (Implements: auto-prompting)
Decision: CLI returns `(output, state, guidance)` — `CommandResult`. LLM is interpreter; human is input stream. CLI stays pure (no LLM dependency). Each interaction is inspectable via `--prompt-only`.
WHY NOT prompt-only: Without structured feedback, LLM loses context between invocations. WHY NOT full agent: Makes CLI non-deterministic and provider-dependent.

**APP-ADR-023: LLMs as Primary Spec Authors** (Implements: auto-prompting)
Decision: The rigorous format (4-component invariants, 5-subsection ADRs) is the API contract between LLM author and mechanical validator. Humans review; LLMs write. User-friendly = better conversation, not simpler format.
WHY NOT human-first: Every spec in this project was authored by LLMs. The format serves the validator.

**APP-ADR-024: Bilateral Specification / The Inverse Principle** (Implements: auto-prompting)
Decision: Every forward operation has an inverse. `discover` (idea→spec) ↔ `absorb` (impl→spec). Four-loop cycle replaces three-loop triad. In category theory: each pair is an adjunction; drift measures round-trip divergence from identity.
WHY NOT unidirectional: The code has knowledge the spec doesn't capture. `absorb` gives the code voice.

**APP-ADR-025: Heuristic Scan over AST Parsing** (Implements: auto-prompting)
Decision: Regex + LLM analysis for `ddis absorb` code extraction. No language-specific AST parsers. Builds on the annotation scanner (APP-ADR-012) with additional heuristic pattern detection.
WHY NOT AST parsing: Violates portability principle (APP-INV-017). The annotation system already solves cross-language extraction.

**APP-ADR-026: Full Workspace Init at Phase 12** (Implements: workspace-ops)
Decision: `ddis init` creates complete scaffolding in one command: manifest, DB, event streams, discovery directory, `.gitignore`. `--workspace` adds multi-spec infrastructure.
WHY NOT partial init: Incomplete scaffolding requires retroactive extension. One comprehensive init is cleaner.

**APP-ADR-027: Peer Spec Relationships** (Implements: workspace-ops)
Decision: Manifest gains `related_specs` array alongside `parent_spec`. Cross-spec references resolve: local → parent → related. Diamond dependencies supported.
WHY NOT parent-child only: CLI spec and meta-spec are already peer domains. Forcing one as "parent" is semantically incorrect.

**APP-ADR-028: Progressive Validation over Binary Pass/Fail** (Implements: workspace-ops)
Decision: Checks grouped into Level 1 (Seed), Level 2 (Growing), Level 3 (Complete). Validation says "here's where you are and what's next," never "FAIL." `--level N` for CI gating.
WHY NOT binary: A freshly-initialized spec failing 11 of 12 checks is discouraging and conveys no useful information.

**APP-ADR-029: Beads-Compatible Task Output** (Implements: workspace-ops)
Decision: `ddis tasks` defaults to beads JSONL for `br import`. Also supports JSON and markdown. Task dependencies derived from `implementation_map.phases`.
WHY NOT JSON only: Beads is the project's issue tracker. Beads-compatible output eliminates manual conversion.

**APP-ADR-030: Persistent Witnesses over Ephemeral Done Flags** (Implements: lifecycle-ops)
Decision: Witnesses persist in the `invariant_witnesses` table with auto-invalidation on spec change via content_hash comparison. `ddis progress` loads witnesses by default. The `--done` flag remains as an additive override.
WHY NOT ephemeral done flags: No auto-invalidation, no per-invariant evidence tracking, no cross-agent visibility.

**APP-ADR-031: Navigational Guidance as Postscript** (Implements: auto-prompting)
Decision: Navigational guidance emitted as postscript, not inline. Data output comes first, guidance follows as a clearly separated block after the primary output.
WHY NOT inline guidance: Interleaving guidance with data output breaks the Gestalt principle of figure-ground separation. The LLM cannot parse data and guidance when they are mixed.

**APP-ADR-032: Gestalt-Optimized CLI Output** (Implements: query-validation)
Decision: Validation output uses Gestalt principles: failures-first, spec framing, warning collapse. Every failing check includes the governing invariant statement inline. Warnings beyond 5 collapsed to count plus top-5 summary.
WHY NOT verbose flag: The useful information should be in the default output. Requiring a flag to see the governing invariant defeats the purpose of LLM-friendly output.

**APP-ADR-033: ddis next as Universal Entry Point** (Implements: auto-prompting)
Decision: Bare `ddis` invocation delegates to `ddis next` meta-command. The meta-command inspects workspace state and suggests the single most useful next action.
WHY NOT help text: Help text lists all commands equally. `ddis next` is opinionated: it reads the current state and recommends one action.

**APP-ADR-034: Pure-Go Tiered Consistency over Z3** (Implements: code-bridge) — **Supersedes APP-ADR-013**
Decision: Replace the Z3 CGo dependency with a pure-Go tiered consistency checking architecture. Tier 1: existing structural validation (14 checks). Tier 2: graph-based contradiction detection via existing BFS + SQLite CTEs (typed cross-ref edges, cycle detection, governance overlap). Tier 3: propositional satisfiability via gophersat (pure Go, MIT, v1.4) for encoding invariant pairs as SAT clauses. Tier 4: heuristic NLP (polarity/quantifier/numeric rules) + existing LSI for semantic tension detection.
WHY NOT Z3: All Go Z3 bindings require CGo, which violates single-binary distribution (APP-ADR-024). The most mature binding (mitchellh/go-z3) is archived. 10-15MB binary size increase plus `libz3-dev` build requirement is unacceptable for a `curl | bash` installable tool. The tiered pure-Go approach achieves ~80% of Z3's detection power at zero CGo cost, and gophersat provides SAT solving with UNSAT core extraction (MUS) for the remaining propositional contradictions.

**APP-ADR-035: Frontmatter-Manifest Cross-Validation** (Implements: query-validation)
Decision: Add a validation check that verifies module frontmatter (maintains, interfaces, implements, negative_specs) is a bijection with the corresponding manifest.yaml entries. Discrepancies are reported as validation errors, not warnings.
WHY NOT trust one source: Both frontmatter and manifest are read by different code paths (parser reads frontmatter, manifest reader reads manifest). Silent divergence causes inconsistent cross-reference resolution depending on which code path runs first.

**APP-ADR-037: Challenge as Right Adjoint of Witness** (Implements: lifecycle-ops)
Decision: Challenge is a separate command with 5-level mechanical verification. Witness records claims, challenge verifies them via formal SAT, uncertainty scoring, causal annotation lookup, practical test execution, and meta keyword overlap.

**APP-ADR-038: Z3 Subprocess as Tier 5 SMT Consistency** (Implements: code-bridge) — **Supersedes APP-ADR-034 partially**
Decision: Z3 invoked as subprocess via SMT-LIB2 stdin/stdout for arithmetic, quantifier, and uninterpreted function contradictions. Graceful degradation when Z3 not in PATH. Preserves single binary. APP-ADR-034 retained for fast propositional path.

**APP-ADR-039: Evidence Accumulation Verdicts** (Implements: lifecycle-ops)
Decision: Dempster-Shafer inspired multi-signal scoring replaces binary test-found confirmation. Independent signals compound: base confidence, package spread, implements verb, annotation volume, semi-formal consistency, keyword overlap. Threshold 0.85 for confirmed.

**APP-ADR-040: LLM-as-Judge Semantic Contradictions** (Implements: code-bridge)
Decision: Anthropic Claude API via Provider interface. Pairwise invariant comparison with majority vote (3 runs, 2/3 agreement). Tier 6 in consistency checker. Graceful degradation.

**APP-ADR-041: Challenge-Feedback Loop Closes Bilateral Lifecycle** (Implements: lifecycle-ops)
Decision: Targeted 4-phase closure of 4 missing morphisms. Challenge verdicts flow to navigation and task derivation. Event stream wiring for 5 command types. LLM provider abstraction.

**APP-ADR-042: Tier 6 LLM-as-Judge Semantic Contradiction Detection** (Implements: code-bridge)
Decision: LLM-as-judge for invariant pairs that failed Tiers 3-5 parsing. Majority vote with 3 runs, 2/3 agreement. Contradictory pairs yield confidence 0.80 (2/3) or 0.95 (3/3). Requires Provider.Available().

**APP-ADR-043: Observational Process Compliance over Prescriptive Gates** (Implements: auto-prompting)
Decision: Process compliance is OBSERVED and REPORTED through existing information channels (context bundles, validation, drift), never ENFORCED through gates or blocks. Follows APP-INV-026 and APP-ADR-018.
WHY NOT prescriptive gates: Coercive hooks create adversarial dynamics; agents learn to circumvent rather than internalize.

**APP-ADR-044: External Issue Tracker Integration via gh CLI** (Implements: workspace-ops)
Decision: Wrap gh CLI for issue filing. ddis issue shells out to gh issue create. Auth delegated to gh. Graceful degradation via APP-INV-057 when gh is absent.

---

## 0.6 Quality Gates (Declarations)

A conforming implementation is "done" when all quality gates pass. Gates are ordered by priority; a failing Gate N makes Gates N+1 through 6 irrelevant.

| Gate | Name | Validates | Check Type |
|------|------|-----------|------------|
| Gate-1 | Structural Conformance | All 30 commands accept expected inputs and produce expected output shapes | Mechanical (integration tests) |
| Gate-2 | Causal Chain | Every command traces through an APP-ADR or APP-INV to the formal state model (§0.2) | Sampling (5 commands) |
| Gate-3 | Decision Coverage | All design choices have corresponding APP-ADRs; no undocumented design decisions | Adversarial review |
| Gate-4 | Invariant Falsifiability | Each APP-INV has a concrete violation scenario and at least one test exercising it | Constructive (test audit) |
| Gate-5 | Cross-Reference Web | No orphan sections in the specification; every section has inbound or outbound references | Graph analysis (the CLI itself can check this) |
| Gate-6 | Self-Validation | The CLI successfully parses, indexes, and validates its own specification with zero errors | Mechanical (`ddis validate ddis-cli-spec/`) |

**Gate 1: Structural Conformance**
All 30 commands accept expected inputs and produce expected output shapes. Tested mechanically via integration tests. The 14 validator checks map to gates as follows: Checks 1, 3, 7, 8, 10 (structural conformance → Gate-1), Checks 2, 4, 5 (falsifiability and depth → Gate-4), Checks 6, 9 (cross-reference web → Gate-5), Check 11 (proportional weight), Check 12 (ADR completeness → Gate-3), Check 13 (traceability → Gate-2), Check 14 (witness freshness → Gate-6).

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

DDIS CLI Spec v3.0 is "done" when:
- All 6 quality gates pass
- All 49 APP-INVs are at least `property-checked` confidence
- The CLI parses and validates this spec with zero errors (APP-G-6)
- At least one non-trivial DDIS spec (the meta-standard itself) has been validated by the CLI
- The bilateral lifecycle (`discover` → `refine` → `drift` → `absorb`) operates on the CLI's own spec (self-bootstrapping)

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
| **Index** | The 39-table SQLite database produced by `parse`. Contains all typed elements, cross-references, search indices, and authority scores. A derived artifact — deletable and recreatable. (APP-ADR-005) |
| **LSI** | Latent Semantic Indexing — dimensionality reduction via truncated SVD on the term-document matrix. Produces semantic similarity scores independent of lexical overlap. (APP-ADR-003, APP-INV-012) |
| **OpLog** | Append-only JSONL file recording all CLI operations: parse, validate, diff, transaction begin/commit/rollback. Survives database recreation. (APP-ADR-007, APP-INV-010) |
| **PageRank** | Iterative authority scoring algorithm applied to the cross-reference graph. Elements with many high-authority inbound references score higher. (APP-ADR-003, APP-INV-004) |
| **RRF** | Reciprocal Rank Fusion — method for combining ranked lists from multiple scoring signals. Score = `SUM(1/(K + rank_r(d)) * weight_r)` with K=60. (APP-INV-008) |
| **Absorption** | The process of deriving a DDIS specification from existing implementation code. Inverse of discovery. Implemented by `ddis absorb`. (APP-ADR-024) |
| **Annotation** | A `// ddis:<verb> <target>` comment embedded in source code declaring spec traceability. Portable across all languages with comment syntax. (APP-ADR-012, APP-INV-017) |
| **Bilateral Specification** | The principle that specification flows in both directions: human intent → spec (discover/refine) AND implementation → spec (absorb/drift). Neither direction is privileged. (APP-ADR-024) |
| **Cognitive Mode** | One of seven modes of human thinking observed during discovery: divergent, convergent, dialectical, abductive, metacognitive, incubation, crystallization. Classified observationally, never prescriptively. (APP-ADR-018) |
| **CommandResult** | The universal return type for auto-prompting commands: `(output, state, guidance)`. The state monad's output. (APP-ADR-022, APP-INV-034) |
| **Contributor Topology** | The authorship structure of a spec, extracted from git blame. Reveals where multiple contributors' mental models overlap or silently conflict. (APP-ADR-021, APP-INV-030) |
| **Contradiction** | A logical conflict between spec elements (e.g., quantifier conflict, negation pair, negative spec violation). Detected by Tier 1 (structural), Tier 2 (graph), Tier 3 (SAT via gophersat), Tier 4 (heuristic NLP + LSI). (APP-ADR-034, APP-INV-019) |
| **Crystallization** | The act of committing a discovery insight into a durable spec artifact (invariant, ADR, glossary entry, negative spec). The only explicit user act in discovery. (APP-INV-028) |
| **Discovery** | The process of transforming nebulous feature ideas into DDIS-conforming spec artifacts through conversational exploration. Implemented by `ddis discover`. (APP-ADR-020) |
| **Event Stream** | An append-only JSONL file recording lifecycle events. Three streams: discovery (Stream 1), spec (Stream 2), implementation (Stream 3). (APP-ADR-015, APP-INV-020) |
| **Guidance** | The LLM-facing component of a `CommandResult`: observed mode, DoF hint, suggested next actions, relevant context, and translation hint. Attenuates over conversation depth. (APP-INV-035) |
| **k\* Guard** | The overprompting threshold from LLM Gestalt Theory. Prompt size must not exceed this budget, which decreases as conversation depth increases. (APP-INV-035, APP-ADR-017) |
| **Progressive Validation** | Validation grouped into maturity levels: Level 1 (Seed), Level 2 (Growing), Level 3 (Complete). Reports current level and next steps. Never says "FAIL." (APP-ADR-028, APP-INV-040) |
| **Reconciliation** | The `--against` mode of `ddis absorb`: comparing absorbed draft against existing spec to find gaps in both directions (undocumented behavior + unimplemented specification). (APP-INV-032) |
| **Seed** | The `seed` command creates a genesis oplog record for a newly parsed specification. Establishes the baseline for subsequent diff and change tracking. |
| **State Monad** | The CLI's interaction pattern: each command takes state and returns `(output, new_state, guidance)`. The LLM is the interpreter; the human is the input stream. (APP-ADR-022) |
| **Thread** | An inquiry thread — a directed line of investigation in discovery. Primary scoping unit for events. May span sessions, LLMs, and humans. Lifecycle: branch, merge, park, resume, fork, converge. (APP-ADR-019, APP-INV-027) |
| **Transaction** | An atomic unit of specification modification. States: `pending`, `committed`, `rolled_back`. State machine enforced by APP-INV-006. |
| **Workspace** | A multi-spec project managed by `ddis init --workspace`. Contains multiple specs with parent, peer, and diamond dependency relationships. (APP-ADR-026, APP-INV-037) |

---

## 0.8 Section Map

Cross-reference lookup: which module file contains each section's full specification.

| Section Range | Module File | Notes |
|---|---|---|
| §0.1-§0.9, APP-INV/ADR/Gate declarations, Glossary | constitution/system.md | Cross-cutting: included in every bundle |
| 4-pass pipeline, schema design, round-trip, hashing, monolith/modular detection | modules/parse-pipeline.md | Owns: APP-INV-001, -009, -015. Implements: APP-ADR-001, -002, -005, -009, -010 |
| BM25/LSI/PageRank, RRF fusion, context bundles, glossary expansion, authority scoring | modules/search-intelligence.md | Owns: APP-INV-004, -005, -008, -012, -014. Implements: APP-ADR-003, -006 |
| 16 validation checks, cross-ref resolution, structural diff, query projection | modules/query-validation.md | Owns: APP-INV-002, -003, -007, -011, -043, -044, -047, -049. Implements: APP-ADR-004, -032, -035, -036 |
| Transaction state machine, oplog, impact BFS, implementation tracing, seed, challenge adjunction, evidence accumulation | modules/lifecycle-ops.md | Owns: APP-INV-006, -010, -013, -016, -041, -050, -051, -052, -053. Implements: APP-ADR-007, -008, -011, -030, -037, -039, -041 |
| Annotations, scan, contradiction detection (graph + SAT + heuristic + LSI + SMT + LLM), event sourcing, LLM provider | modules/code-bridge.md | Owns: APP-INV-017, -018, -019, -020, -021, -054, -055. Implements: APP-ADR-012, -014, -015, -034, -038, -040, -042 |
| State monad, discover, refine, absorb loops, contributor topology, thread management | modules/auto-prompting.md | Owns: APP-INV-022–036, -042, -045, -046. Implements: APP-ADR-016–025, -031, -033 |
| Workspace init, multi-domain composition, cross-spec drift, task generation, progressive validation | modules/workspace-ops.md | Owns: APP-INV-037, -038, -039, -040. Implements: APP-ADR-026, -027, -028, -029 |

---

## 0.9 Non-Goals

This specification explicitly does NOT attempt:

1. **Code generation from spec.** The CLI indexes and validates specifications. It does not generate Go, Rust, or any other implementation code from spec content. Code generation is a separate tool concern.

2. **Formal proof artifacts.** Invariant confidence levels include `proven` as a future goal, but this specification does not prescribe a formal verification toolchain beyond tiered pure-Go consistency checking for contradiction detection (APP-ADR-034, superseding APP-ADR-013). Structured implementation traces (APP-ADR-011) remain the ceiling for invariant confidence.

3. **Embedding an LLM runtime.** The CLI is a state monad (APP-ADR-022): it produces prompts and guidance, but does not execute them. The LLM interpreter is external. This preserves the CLI's purity, determinism, and provider-independence.

4. **MCP server interface.** The CLI is shell-first. An MCP wrapper may be added later but is not specified here (PLAN-ADR-003).

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
| **parse-pipeline** | parsing | 4-pass parse pipeline (tree, elements, xrefs, resolve), 39-table schema design, render engine, monolith/modular detection, content hashing. APP-INV-001, -009, -015. APP-ADR-001, -002, -005, -009, -010. |
| **search-intelligence** | search | BM25/FTS5 integration, LSI model (truncated SVD), PageRank computation, RRF fusion (K=60), context bundle assembly (9 signals), glossary expansion. APP-INV-004, -005, -008, -012, -014. APP-ADR-003, -006. |
| **query-validation** | validation | Query projection, 16 validation checks (composable), cross-reference resolution, structural diff, Cobra command routing. APP-INV-002, -003, -007, -011, -043, -044, -047, -049. APP-ADR-004, -032, -035, -036. |
| **lifecycle-ops** | lifecycle | Transaction state machine (begin/commit/rollback), JSONL oplog (append-only), impact BFS with cycle protection, seed command, implementation traceability (Check 13). APP-INV-006, -010, -013, -016, -041. APP-ADR-007, -008, -011, -030. |
| **code-bridge** | bridge | Cross-language annotation scanner (`ddis scan`), tiered contradiction detection (graph + SAT + heuristic + LSI), three-stream event sourcing (`ddis history`), spec-code drift types. APP-INV-017–021. APP-ADR-012, -014, -015, -034. |
| **auto-prompting** | autoprompt | Bilateral specification lifecycle: `ddis discover` (idea→spec), `ddis refine` (spec improvement), `ddis absorb` (impl→spec). State monad architecture, thread-scoped discovery, cognitive mode classification, contributor topology, Gestalt-optimized prompt generation. APP-INV-022–036, -042, -045, -046. APP-ADR-016–025, -031, -033. |
| **workspace-ops** | workspace | Workspace initialization (`ddis init`), multi-domain composition (`ddis spec add/list`), cross-spec drift, mechanical task generation (`ddis tasks`), progressive validation (Level 1/2/3 maturity tiers). APP-INV-037–040. APP-ADR-026–029. |
