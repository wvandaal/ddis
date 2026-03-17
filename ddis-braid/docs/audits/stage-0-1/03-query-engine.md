# Query Engine — Stage 0/1 Audit
> Wave 1 Domain Audit | Date: 2026-03-17 | Agent: Opus 4.6 | Methodology: Fagan Inspection + IEEE Walkthrough

## Domain Inventory

### QUERY Namespace (spec/03-query.md)
- **INVs**: INV-QUERY-001 through INV-QUERY-024 (24 invariants)
- **ADRs**: ADR-QUERY-001 through ADR-QUERY-013 (13 ADRs)
- **NEGs**: NEG-QUERY-001 through NEG-QUERY-004 (4 negative cases)

---

## Quantitative Summary

### INVs (24 total in spec)

| ID | Stage | Status | Evidence |
|---|---|---|---|
| INV-QUERY-001 | 0 | PARTIALLY IMPLEMENTED | evaluator.rs:1-17, but no true semi-naive fixpoint loop |
| INV-QUERY-002 | 0 | IMPLEMENTED | evaluator.rs:60-61, proptest at evaluator.rs:870-920 |
| INV-QUERY-003 | 1 | UNIMPLEMENTED (deferred) | Stage 1, no access log exists |
| INV-QUERY-004 | 2 | UNIMPLEMENTED (deferred) | Stage 2, no branch visibility |
| INV-QUERY-005 | 0 | IMPLEMENTED | stratum.rs:46-48, 95-105 |
| INV-QUERY-006 | 0 | PARTIALLY IMPLEMENTED | No rule system in AST, no head-variable safety check |
| INV-QUERY-007 | 0 | IMPLEMENTED | evaluator.rs:49-76, Frontier scoping works |
| INV-QUERY-008 | 1 | UNIMPLEMENTED (deferred) | Stage 1, no FFI mechanism |
| INV-QUERY-009 | 1 | UNIMPLEMENTED (deferred) | Stage 1 |
| INV-QUERY-010 | 3 | UNIMPLEMENTED (deferred) | Stage 3 |
| INV-QUERY-011 | 2 | UNIMPLEMENTED (deferred) | Stage 2 |
| INV-QUERY-012 | 0 | DIVERGENT | graph.rs:93-138 operates on DiGraph, not Store+entity_type+dep_attr |
| INV-QUERY-013 | 0 | DIVERGENT | graph.rs:141-209, same API mismatch as INV-QUERY-012 |
| INV-QUERY-014 | 0 | DIVERGENT | graph.rs:215-247, API mismatch + no epsilon-convergence/max_iterations config |
| INV-QUERY-015 | 1 | IMPLEMENTED (ahead of stage) | graph.rs:573-650 |
| INV-QUERY-016 | 1 | IMPLEMENTED (ahead of stage) | graph.rs:2619-2706 |
| INV-QUERY-017 | 0 | DIVERGENT | graph.rs:252-285, API mismatch + missing slack/forward-backward pass |
| INV-QUERY-018 | 1 | IMPLEMENTED (ahead of stage) | graph.rs:2722-2785 |
| INV-QUERY-019 | 2 | UNIMPLEMENTED | No eigenvector_centrality function exists |
| INV-QUERY-020 | 2 | UNIMPLEMENTED | No articulation_points function exists |
| INV-QUERY-021 | 0 | DIVERGENT | graph.rs:941-948 (density only, no avg_degree/clustering/components) |
| INV-QUERY-022 | 0 | IMPLEMENTED | graph.rs:1327-1358, graph.rs:1224-1320 |
| INV-QUERY-023 | 0 | IMPLEMENTED | graph.rs:504-508 |
| INV-QUERY-024 | 0 | IMPLEMENTED | graph.rs:523-562 |

**Stage 0 INVs: 14 total. Fully conformant: 5. Partially conformant: 2. Divergent: 5. Unimplemented: 2 (INV-QUERY-019, 020 are Stage 2, not Stage 0 blockers).**

### ADRs (13 total in spec)

| ID | Reflected in Code | Notes |
|---|---|---|
| ADR-QUERY-001 | YES | Datalog over SQL -- evaluator.rs |
| ADR-QUERY-002 | DIVERGENT | Claims semi-naive but implementation is naive nested-loop |
| ADR-QUERY-003 | PARTIALLY | Six strata defined in stratum.rs:27-39 but classifier returns S0/S1 only |
| ADR-QUERY-004 | NO | No FFI mechanism exists |
| ADR-QUERY-005 | YES | Local frontier as default, evaluator.rs:45-47 |
| ADR-QUERY-006 | NO | No `:tx/frontier` as datom attribute; frontier is a separate struct |
| ADR-QUERY-007 | NO | No projection pyramid |
| ADR-QUERY-008 | NO | No bilateral query layer |
| ADR-QUERY-009 | YES | Full graph engine in kernel, graph.rs |
| ADR-QUERY-010 | NO | No three-layer composition |
| ADR-QUERY-011 | NO | No query stability score |
| ADR-QUERY-012 | YES | Spectral operations implemented in graph.rs |
| ADR-QUERY-013 | YES | Hodge-theoretic coherence via edge Laplacian in graph.rs |

**ADRs: 13 total. Reflected in code: 6. Drifted/not reflected: 7.**

### NEGs (4 total in spec)

| ID | Enforced | Notes |
|---|---|---|
| NEG-QUERY-001 | PARTIALLY | Stratum classification prevents S2+ at eval time, but no parse-time rejection since no parser exists |
| NEG-QUERY-002 | YES | Queries are pure reads; evaluator has no &mut Store |
| NEG-QUERY-003 | PARTIALLY | Nested-loop join always terminates, but no rules/recursion to test fixpoint termination |
| NEG-QUERY-004 | YES | No access log exists, so no access events anywhere |

**NEGs: 4 total. Fully enforced: 2. Partially enforced: 2. Reachable: 0.**

---

## Findings

### FINDING-001: The evaluator is NOT semi-naive; it is a naive nested-loop join
Severity: HIGH
Type: DIVERGENCE
Sources: spec/03-query.md:47-55 (Semi-Naive Evaluation algebraic spec) vs crates/braid-kernel/src/query/evaluator.rs:72-114
Evidence: The spec defines semi-naive evaluation with a delta iteration: "on each iteration, only derive facts using at least one NEW fact from the previous iteration." The implementation at evaluator.rs:72-114 performs a sequential nested-loop join over clauses -- it iterates through where_clauses once, feeding bindings forward. There is no fixpoint loop, no delta computation, no iteration counter, and no convergence check. The module header at evaluator.rs:1 says "Semi-naive fixpoint Datalog evaluator" but the code contains zero instances of the words "iteration", "fixpoint", "delta", or "convergence" in executable code. This is a single-pass nested-loop join, which is correct for conjunctive queries over ground data but is NOT semi-naive evaluation as specified.
Impact: Any recursive Datalog program (rules referencing themselves) would not be evaluated. The spec's INV-QUERY-001 (Knaster-Tarski fixpoint) and INV-QUERY-006 (semi-naive termination) are structurally impossible to test because the evaluator cannot handle recursion.

### FINDING-002: No Datalog parser exists; queries are programmatic-only
Severity: HIGH
Type: GAP
Sources: docs/guide/03-query.md:256-272 (parser spec with [:find ... :where ...] syntax) vs Glob result showing no parser.rs file
Evidence: The guide specifies a parser module (`parser.rs`) that converts string input `[:find ?vars :where [clauses]]` into `ParsedQuery`. No such file exists. The `QueryExpr` struct in clause.rs:10-16 must be constructed programmatically in Rust. The CLI command `braid query '[:find ...]'` specified in the guide cannot work without a parser. The `ParsedQuery` struct from the guide (with `find_spec`, `where_clauses`, `rules`, `inputs` fields) does not exist -- instead there is `QueryExpr` with only `find` and `where_clauses`.
Impact: The primary user-facing interaction point for queries (string-based Datalog) is missing. All query construction requires Rust API calls. This blocks the self-bootstrap scenario where `braid query` is used from the CLI.

### FINDING-003: QueryMode enum does not exist in the codebase
Severity: HIGH
Type: UNIMPLEMENTED
Sources: spec/03-query.md:410-414 (Stratum Safety with QueryMode::Monotonic, Stratified, Barriered) and guide at docs/guide/03-query.md:59-61 vs Grep for "QueryMode" returning no matches
Evidence: The spec defines `QueryMode` with at least `Monotonic`, `Stratified`, and `Barriered` variants. The guide specifies `QueryMode` as a field on `QueryResult`. Neither type exists in the codebase. The stratum classifier in stratum.rs:95-105 returns `Stratum` but there is no mode-stratum compatibility check because there is no mode to check against. INV-QUERY-005 (mode-stratum compatibility) is nominally satisfied only because the classifier always returns S0/S1 and `check_stage0` rejects S2+, but the mechanism specified (matching mode against stratum) does not exist.
Impact: The spec's safety property that "monotonic mode rejects S2+ queries" is not enforced through the specified mechanism. There is no way to request a specific query mode.

### FINDING-004: Graph algorithms operate on DiGraph, not Store+attributes as specified
Severity: MEDIUM
Type: DIVERGENCE
Sources: spec/03-query.md:591-600 (INV-QUERY-012 L2 contract: `topo_sort(store, entity_type, dep_attr)`) vs graph.rs:93 (`pub fn topo_sort(graph: &DiGraph)`)
Evidence: All spec Level 2 contracts for graph algorithms (INV-QUERY-012 through INV-QUERY-021) take `(store: &Store, entity_type: &Attribute, dep_attr: &Attribute)` as parameters. The actual implementations take `&DiGraph` (a pre-constructed directed graph). The spec's `extract_subgraph` function that converts store+attributes into a graph is not part of the graph module -- the caller must construct a `DiGraph` manually. The `DiGraph` uses `String` node labels instead of `EntityId`. The guide at docs/guide/03-query.md:94-130 also specifies the Store-based API.
Impact: Graph algorithms cannot be called directly from the query engine against store data. Every consumer must write its own subgraph extraction logic. The specified integration pattern (query engine provides graph operations over store data) is broken.

### FINDING-005: PageRank does not use configurable epsilon/damping/max_iterations
Severity: MEDIUM
Type: DIVERGENCE
Sources: spec/03-query.md:688-704 (INV-QUERY-014 L2: `PageRankConfig { damping, epsilon, max_iterations }`) vs graph.rs:215-247
Evidence: The spec requires `pagerank(store, entity_type, dep_attr, config: &PageRankConfig)` with configurable damping (default 0.85), epsilon convergence (1e-6), and max_iterations (100). The implementation at graph.rs:215 has signature `pub fn pagerank(graph: &DiGraph, iterations: usize)` -- hardcoded damping of 0.85 (line 221), no epsilon convergence check, and caller-specified iteration count instead of a safety-bounded maximum. The guide at docs/guide/03-query.md:553-559 specifies the PageRankConfig proptest. The falsification condition ("scores do not sum to 1.0") is actually violated for graphs with dangling nodes, as acknowledged in the test at graph.rs:3409-3419.
Impact: The normalization property (scores sum to 1.0) stated in the spec's INV-QUERY-014 is violated for any graph with dangling nodes (nodes with no outgoing edges). The proptest at graph.rs:3402-3427 explicitly handles this by weakening the assertion.

### FINDING-006: Critical path analysis missing forward/backward pass, slack, earliest/latest start
Severity: MEDIUM
Type: DIVERGENCE
Sources: spec/03-query.md:842-860 (INV-QUERY-017 L2: `CriticalPathResult { path, total_weight, slack, earliest_start, latest_start }`) vs graph.rs:252-285
Evidence: The spec requires a `CriticalPathResult` with per-vertex slack, earliest_start, and latest_start maps. The implementation returns `Option<(usize, Vec<String>)>` -- just the path length and the path itself. There is no slack computation, no earliest_start/latest_start, no weight support (uniform weight hardcoded), and no forward/backward pass. The falsification condition "a non-critical-path vertex has zero slack" cannot be tested because slack is not computed.
Impact: The critical path implementation is a longest-path computation, not a full critical path analysis. Downstream consumers (like R(t) routing in INV-GUIDANCE-010) that need slack values cannot use this.

### FINDING-007: Graph density implementation lacks avg_degree, clustering, and component count
Severity: MEDIUM
Type: DIVERGENCE
Sources: spec/03-query.md:1040-1058 (INV-QUERY-021 L2: `GraphDensityMetrics { vertex_count, edge_count, density, avg_degree, avg_clustering, components }`) vs graph.rs:941-948
Evidence: The spec requires a `GraphDensityMetrics` struct with six fields including avg_degree, avg_clustering, and component count. The implementation is a single function `pub fn density(graph: &DiGraph) -> f64` at graph.rs:941 that returns only the density scalar. No `GraphDensityMetrics` struct exists. Average degree, clustering coefficient, and connected component count are not computed.
Impact: The `braid status` command and M(t) methodology adherence score (INV-GUIDANCE-008) that consume graph density metrics will not have the full health signal.

### FINDING-008: Clause AST missing spec-required variants (RuleApplication, OrClause, NotClause, Frontier)
Severity: MEDIUM
Type: DIVERGENCE
Sources: docs/guide/03-query.md:74-83 (Clause enum with 5 variants) vs clause.rs:28-39 (Clause enum with 2 variants)
Evidence: The guide specifies `Clause` with variants: `DataPattern`, `RuleApplication`, `NotClause`, `OrClause`, `Frontier`. The implementation has only `Pattern` and `Predicate`. Neither rules, negation, or-clauses, nor frontier clauses exist in the AST. The guide explicitly marks some as "Stage 1+" but `Frontier(FrontierRef)` is specified for Stage 0 (INV-QUERY-007). Frontier scoping is implemented via the `evaluate_with_frontier` function parameter, not as a clause in the query.
Impact: Rules (recursive Datalog) and or-clauses cannot be expressed. Frontier scoping works but through a different mechanism than specified (function parameter vs. clause in the query). The query language is significantly less expressive than specified.

### FINDING-009: QueryResult type diverges from spec/guide
Severity: LOW
Type: DIVERGENCE
Sources: docs/guide/03-query.md:55-62 (QueryResult with bindings: Vec<BindingSet>, stratum, mode, provenance_tx) vs evaluator.rs:34-40 (QueryResult::Rel/Scalar enum)
Evidence: The guide specifies `QueryResult { bindings: Vec<BindingSet>, stratum: Stratum, mode: QueryMode, provenance_tx: TxId }` where `BindingSet` preserves variable names. The implementation uses `QueryResult::Rel(Vec<Vec<Value>>)` and `QueryResult::Scalar(Option<Value>)`. Variable names are lost in the projection step (evaluator.rs:97-113). There is no stratum, mode, or provenance_tx in the result. The `Binding` type (HashMap<String, Value>) exists internally but is not exposed in results.
Impact: Downstream consumers cannot inspect which stratum a query executed at, cannot verify the mode, and cannot trace query provenance. Variable names are lost, making result interpretation harder for LLM agents.

### FINDING-010: INV-QUERY-007 frontier is not stored as `:tx/frontier` attribute datom
Severity: MEDIUM
Type: DIVERGENCE
Sources: spec/03-query.md:455-461 (INV-QUERY-007: "Frontier is stored as :tx/frontier attribute") and ADR-QUERY-006 ("Frontier as Datom Attribute") vs store.rs:244 (Frontier is a HashMap<AgentId, TxId> struct)
Evidence: The spec and ADR-QUERY-006 both require that frontier data be stored as datoms with the `:tx/frontier` attribute, queryable via "the same Datalog engine as any other data." Grep for ":tx/frontier" returns zero matches in the codebase. The `Frontier` struct at store.rs:244 is an in-memory `HashMap<AgentId, TxId>` that is not persisted as datoms. The `evaluate_with_frontier` function accepts a `Frontier` parameter, but frontier information is not queryable via Datalog. A query like `[:find ?agent ?tx :where [_ :tx/frontier ?agent ?tx]]` cannot work.
Impact: The spec's commitment that "What does agent X know?" is answerable as an ordinary Datalog query (INV-QUERY-007 Level 0) is not met. Frontier is a first-class parameter but not first-class data.

### FINDING-011: Aggregation exists as post-processing but not as Datalog-integrated operation
Severity: LOW
Type: MISALIGNMENT
Sources: spec/03-query.md:181-199 (Stratum 3 = Aggregation) vs aggregate.rs:65-119
Evidence: The spec defines aggregation as Stratum 3, requiring stratified evaluation within the Datalog engine. The implementation in aggregate.rs provides `aggregate()` as a post-processing function applied to `QueryResult` after evaluation. This is correct at Stage 0 (where S3 is deferred), but the implementation approach (post-processing) diverges from the spec's approach (in-engine aggregation). The guide acknowledges this at guide:81 with "Stage 1+: Aggregate(Variable, AggregateFunc)".
Impact: Low for Stage 0. The post-processing approach works correctly for simple cases. However, it cannot handle aggregation within recursive rules (which the spec's Stratum 3/4 requires).

### FINDING-012: INV numbers in code comments contradict spec assignments
Severity: LOW
Type: MISALIGNMENT
Sources: mod.rs:9-28 (INV-QUERY doc comments) vs spec/03-query.md:301-1253 (INV definitions)
Evidence: Several INV numbers in mod.rs comments do not match the spec's definitions:
- mod.rs:9 says "INV-QUERY-001: Semi-naive fixpoint convergence" but spec says "INV-QUERY-001: CALM Compliance"
- mod.rs:10 says "INV-QUERY-002: CALM compliance for S0/S1" but spec says "INV-QUERY-002: Query Determinism"
- mod.rs:12 says "INV-QUERY-004: Branch visibility" (which is correct)
- mod.rs:14 says "INV-QUERY-006: Entity-centric view via index scan" but spec says "INV-QUERY-006: Semi-Naive Termination"
- graph.rs:15 says "INV-QUERY-017: All graph algorithms are deterministic" but spec says "INV-QUERY-017: Critical Path Analysis"
The spec and code disagree on what several INV numbers mean.
Impact: Traceability from code to spec is unreliable. A developer checking INV-QUERY-001 compliance in the code would be verifying a different property than the spec defines.

### FINDING-013: Tarjan SCC result type diverges from spec
Severity: LOW
Type: DIVERGENCE
Sources: spec/03-query.md:639-651 (SCCResult with components, condensation, has_cycles) vs graph.rs:141 (`pub fn scc(graph: &DiGraph) -> Vec<Vec<String>>`)
Evidence: The spec defines `SCCResult { components, condensation, has_cycles }` with a condensation DAG. The implementation returns `Vec<Vec<String>>` -- just the component lists without the condensation DAG or the `has_cycles` flag. The `topo_sort` function detects cycles implicitly (returns None) rather than using SCCResult.
Impact: Downstream consumers that need the condensation DAG (e.g., for operating on the SCC-contracted graph) cannot use the current API.

### FINDING-014: FindSpec missing Collection and Tuple variants
Severity: LOW
Type: DIVERGENCE
Sources: docs/guide/03-query.md:64-71 (FindSpec with Relation, Scalar, Collection, Tuple) vs clause.rs:20-25 (FindSpec with Rel, Scalar only)
Evidence: The guide specifies four Datomic-style find forms: `Relation(Vec<Variable>)`, `Scalar(Variable)`, `Collection(Variable)` (`[:find [?x ...]]`), and `Tuple(Vec<Variable>)` (`[:find [?x ?y]]`). The implementation has only `Rel(Vec<String>)` and `Scalar(String)`. The Collection and Tuple forms are missing.
Impact: Queries that need to return a flat collection of values or a single tuple cannot express this in the find spec. All multi-value queries must use Rel and post-process.

### FINDING-015: The Predicate clause type exists in code but not in spec or guide AST
Severity: INFO
Type: MISALIGNMENT
Sources: clause.rs:33-38 (Clause::Predicate { op, args }) vs guide/03-query.md:74-83 (no Predicate variant)
Evidence: The implementation includes `Clause::Predicate` with comparison operators (=, !=, >, <, >=, <=). Neither the spec nor the guide mention a Predicate clause variant in the AST definition. The stratum classifier correctly classifies predicate filters as monotone (S1). This is a useful extension but is undocumented in the spec.
Impact: The spec does not account for this clause type in its stratum classification rules or falsification conditions. The implementation is ahead of the spec here.

---

## Domain Health Assessment

**Strongest aspect**: The graph algorithm suite in `graph.rs` is the most impressive part of this domain. At ~4000 LOC, it provides topological sort, Tarjan SCC, PageRank, betweenness centrality, HITS, k-core decomposition, persistent homology, Fiedler vector, spectral decomposition (both dense Jacobi and sparse Lanczos), Cheeger inequality, Ollivier-Ricci curvature, sheaf cohomology, and heat kernel trace. Many of these are ahead of their specified stage. The implementations are mathematically sound, deterministic (BTreeMap ordering throughout), and well-tested with both unit tests and property-based tests. The spectral graph theory work (INV-QUERY-022 through 024) is particularly complete.

**Most concerning gap**: The Datalog evaluator is structurally unable to fulfill its spec. It claims to be "semi-naive fixpoint" (INV-QUERY-001, ADR-QUERY-002) but is a single-pass nested-loop join. There is no fixpoint loop, no delta computation, no rule system, no recursion support, and no parser. The `QueryMode` type that the spec builds its safety properties around (INV-QUERY-005, NEG-QUERY-001) does not exist. The frontier is not stored as datom data (INV-QUERY-007, ADR-QUERY-006). These are not missing features at the margins -- they are the fundamental architectural commitments of the spec's query engine design. The evaluator works correctly for its narrow scope (conjunctive queries over ground data with variable joins), but it is a different system than what the spec describes.
