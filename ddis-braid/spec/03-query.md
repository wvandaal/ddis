> **Namespace**: QUERY | **Wave**: 1 (Foundation) | **Stage**: 0
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §3. QUERY — Datalog Query Engine

### §3.0 Overview

Queries in Braid use a Datomic-style Datalog dialect with semi-naive bottom-up evaluation.
The query engine classifies queries into six strata of increasing power and cost, with
CALM compliance determining which queries can run without coordination.

**Traces to**: SEED.md §4
**ADRS.md sources**: FD-003, SQ-001–010, PO-013, AA-001

---

### §3.1 Level 0: Algebraic Specification

#### Datalog Fixpoint

```
A Datalog program P over database D (the datom store) computes the minimal fixpoint:
  T_P(I) = I ∪ { head(r) | r ∈ P, body(r) ⊆ I }
  fixpoint(P, D) = lfp(T_P, D) = T_P^ω(D)

The fixpoint exists and is unique (by Knaster-Tarski, since T_P is monotone on
the lattice of interpretations ordered by subset inclusion).
```

#### CALM Theorem Compliance

```
CALM (Consistency As Logical Monotonicity):
  A program has a consistent, coordination-free distributed implementation
  iff it is monotone.

Monotone query: adding facts can only add results (never remove them).
  ∀ D ⊆ D': Q(D) ⊆ Q(D')

Non-monotone operations: negation, aggregation, set difference.
  These are frontier-relative: result depends on what is NOT in the store,
  which varies by agent frontier.
```

#### Semi-Naive Evaluation

```
Standard naive evaluation: iterate T_P until fixpoint.
Semi-naive optimization: on each iteration, only derive facts using at least
one NEW fact from the previous iteration.

ΔT_P^(i+1) = T_P(I^i ∪ ΔI^i) \ I^i
I^(i+1) = I^i ∪ ΔT_P^(i+1)

Terminates when ΔT_P^(i+1) = ∅.
```

#### Query Modes

```
QueryMode = Monotonic         — runs at any frontier without coordination
          | Stratified(FId)   — non-monotonic, evaluated at specific frontier
          | Barriered(BId)    — requires sync barrier for correctness

∀ queries Q:
  is_monotonic(Q) ⟹ mode(Q) = Monotonic
  has_negation(Q) ∨ has_aggregation(Q) ⟹ mode(Q) ∈ {Stratified, Barriered}
```

---

### §3.2 Level 1: State Machine Specification

#### Six-Stratum Classification

```
Stratum 0 — Primitive (monotonic):
  Current-value over LIVE index. No joins beyond entity lookup.
  QueryMode: Monotonic
  Examples: current-value, entity-attributes, type-instances

Stratum 1 — Graph Traversal (monotonic):
  Multi-hop joins following references. Transitive closure.
  QueryMode: Monotonic
  Examples: causal-ancestor, depends-on, cross-ref reachability

Stratum 2 — Uncertainty (mixed):
  Epistemic (count-distinct aggregation), aleatory (entropy — FFI),
  consequential (DAG traversal — FFI).
  QueryMode: Stratified
  Examples: epistemic-uncertainty, aleatory-uncertainty, consequential-risk

Stratum 3 — Authority (not pure Datalog):
  Linear algebra: SVD of agent-entity matrix, Laplacian eigendecomposition,
  Fiedler vector, spectral partitioning.
  QueryMode: Stratified (FFI to Rust linear algebra)
  Examples: spectral-authority, delegation-threshold, spectral-partition

Stratum 4 — Conflict Detection (conservatively monotonic):
  Concurrent assertion detection on cardinality-one attributes.
  QueryMode: Monotonic (conservative — may overcount)
  Examples: detect-conflicts, route-conflict

Stratum 5 — Bilateral Loop (non-monotonic):
  Fitness computation, crystallization readiness, drift measurement.
  QueryMode: Barriered (for correctness-critical decisions)
  Examples: spec-fitness, crystallization-candidates, drift-candidates
```

#### Query Evaluation Pipeline

```
QUERY(S, expression, frontier, mode) → QueryResult

PRE:
  expression is a valid Datalog program
  if mode = Monotonic: expression contains no negation/aggregation
  if mode = Barriered(id): barrier id is resolved

PIPELINE:
  1. Parse expression → AST
  2. Classify monotonicity → reject Monotonic mode if non-monotonic
  3. Determine stratum
  4. Select data source:
     - Monotonic: any available frontier (default: local)
     - Stratified: specified frontier
     - Barriered: barrier's consistent cut
  5. Evaluate via semi-naive bottom-up with FFI for derived functions
  6. Record query provenance as transaction (INV-STORE-014)
  7. Generate access event in access log (INV-QUERY-003)

POST:
  result is the minimal fixpoint of the program over the selected data
  provenance transaction recorded
  access event generated
```

#### Frontier-Scoped Evaluation

```
A query at frontier F sees exactly:
  visible(F) = {d ∈ S | d.tx ≤ max(F[d.tx.agent])}

Frontier is itself a datom attribute (:tx/frontier), enabling:
  [:find ?agent ?tx :where [?tx :tx/frontier ?f] [?f :frontier/agent ?agent]]
```

---

### §3.3 Level 2: Interface Specification

```rust
/// Datalog query expression.
/// Find wraps ParsedQuery — the richer AST produced by the query parser.
/// ParsedQuery subsumes the simpler {variables, clauses} representation
/// with support for all four Datomic find forms, rules, and inputs.
pub enum QueryExpr {
    Find(ParsedQuery),
    Pull {
        pattern: PullPattern,
        entity: EntityRef,
    },
}

/// Parsed Datalog query — the rich AST for a Find expression.
pub struct ParsedQuery {
    pub find_spec:     FindSpec,
    pub where_clauses: Vec<Clause>,
    pub rules:         Vec<Rule>,
    pub inputs:        Vec<Input>,
}

/// Datomic-style find specification — the four standard find forms.
pub enum FindSpec {
    Relation(Vec<Variable>),    // [:find ?x ?y]
    Scalar(Variable),           // [:find ?x .]
    Collection(Variable),       // [:find [?x ...]]
    Tuple(Vec<Variable>),       // [:find [?x ?y]]
}

/// Clause types for Datalog queries.
/// Stage 0 variants: DataPattern, RuleApplication, NotClause, OrClause, Frontier.
/// Stage 1+ variants (deferred): Aggregate, Ffi.
pub enum Clause {
    /// Data pattern match: [?e :attr ?v]
    DataPattern(EntityPattern, AttributePattern, ValuePattern),
    /// Rule application: (rule-name ?x ?y)
    RuleApplication(RuleName, Vec<Term>),
    /// Negation: (not [?e :attr ?v]). Stratum 2+ only.
    NotClause(Box<Clause>),
    /// Disjunction: (or [...] [...])
    OrClause(Vec<Vec<Clause>>),
    /// Frontier scope: [:frontier ?f]
    Frontier(FrontierRef),
    // Stage 1+: Aggregate(Variable, AggregateFunc)
    // Stage 1+: Ffi(FfiCall)
}

pub enum QueryMode {
    Monotonic,
    Stratified(Frontier),
    Barriered(BarrierId),
}

pub type BindingSet = HashMap<Variable, Value>;

pub struct QueryResult {
    pub bindings: Vec<BindingSet>,
    pub stratum:  Stratum,
    pub mode:     QueryMode,
    pub provenance_tx: TxId,
}

/// Six-stratum classification (ADR-QUERY-003, INV-QUERY-005).
/// Determines the query's safety level and required coordination mode.
pub enum Stratum {
    /// Stratum 0: Current-value over LIVE index. No joins. Monotonic.
    S0_Primitive,
    /// Stratum 1: Multi-hop joins, transitive closure. Monotonic.
    S1_MonotonicJoin,
    /// Stratum 2: Epistemic/aleatory/consequential uncertainty. Mixed (Stratified).
    S2_Uncertainty,
    /// Stratum 3: Linear algebra (SVD, spectral authority, Laplacian, Fiedler vector, spectral partitioning). Stratified (FFI).
    S3_Authority,
    /// Stratum 4: Concurrent assertion detection. Conservatively monotonic.
    S4_ConflictDetection,
    /// Stratum 5: Fitness, crystallization readiness, drift. Barriered.
    S5_BilateralLoop,
}

/// Errors from query parsing, classification, or evaluation.
/// Return type of the top-level query() function.
pub enum QueryError {
    /// Query expression failed to parse.
    ParseError(String),
    /// Non-monotonic constructs (negation, aggregation) in Monotonic mode (INV-QUERY-001).
    NonMonotonicInMonotonicMode,
    /// Unsafe Datalog program: unbound head variable (INV-QUERY-006).
    UnsafeProgram { variable: Variable },
    /// Barrier required but not resolved (INV-QUERY-005: Stratum 5 requires Barriered mode).
    BarrierNotResolved(BarrierId),
    /// FFI derived function error.
    FfiError { function: String, message: String },
    /// Graph algorithm error (delegates to GraphError).
    Graph(GraphError),
}

/// Errors from graph algorithms (INV-QUERY-012–021).
/// Used by topo_sort, critical_path, and other graph queries.
pub enum GraphError {
    /// Cycle detected during topological sort (INV-QUERY-012, INV-QUERY-013).
    /// Contains the strongly connected components via Tarjan's algorithm.
    CycleDetected(SCCResult),
    /// Graph has no vertices for the specified entity type and dependency attribute.
    EmptyGraph,
    /// Power iteration (PageRank, eigenvector centrality, HITS) did not converge
    /// within the maximum iteration bound.
    NonConvergence(u32),
}

// Free function (ADR-ARCHITECTURE-001): query is a complex operation spanning
// the QUERY namespace (stratum classification, FFI, access log). Takes &Store
// rather than &mut self; provenance recording (INV-STORE-014) is a separate
// transact call by the caller.
pub fn query(store: &Store, expr: &QueryExpr, mode: QueryMode) -> Result<QueryResult, QueryError>;
```

#### FFI Boundary

```rust
/// Derived functions that cannot be expressed in pure Datalog.
pub trait DerivedFunction {
    fn name(&self) -> &str;
    fn evaluate(&self, inputs: &[Value]) -> Result<Value, FfiError>;
}

/// Three core derived functions:
/// 1. σ_a (aleatory uncertainty) — requires entropy computation
/// 2. σ_c (consequential uncertainty) — requires bottom-up DAG traversal
/// 3. spectral_authority — requires SVD (linear algebra)
pub fn register_derived_functions(engine: &mut QueryEngine) {
    engine.register_ffi("aleatory_uncertainty", AleatoryUncertainty);
    engine.register_ffi("consequential_uncertainty", ConsequentialUncertainty);
    engine.register_ffi("spectral_authority", SpectralAuthority);
}
```

#### CLI Commands

```
braid query '[:find ?e ?name :where [?e :db/ident ?name]]'
braid query --file query.edn
braid query --mode monotonic '[:find ...]'    # Reject if non-monotonic
braid query --frontier agent-1 '[:find ...]'  # Query at specific frontier
```

---

### §3.4 Invariants

### INV-QUERY-001: CALM Compliance

**Traces to**: SEED §4 Axiom 4, ADRS FD-003, PO-013
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ monotonic queries Q, ∀ D ⊆ D':
  Q(D) ⊆ Q(D')
  (adding facts can only add results, never remove them)
```

#### Level 1 (State Invariant)
Queries declared as `Monotonic` mode MUST NOT contain negation or aggregation.
The query parser rejects non-monotonic constructs in Monotonic mode at parse time.

#### Level 2 (Implementation Contract)
```rust
impl QueryParser {
    pub fn parse(&self, expr: &str, mode: QueryMode) -> Result<QueryAst, QueryError> {
        let ast = self.parse_inner(expr)?;
        if mode == QueryMode::Monotonic && ast.has_negation_or_aggregation() {
            return Err(QueryError::NonMonotonicInMonotonicMode);
        }
        Ok(ast)
    }
}
```

**Falsification**: A query in Monotonic mode that contains negation or aggregation and
is not rejected at parse time.

---

### INV-QUERY-002: Query Determinism

**Traces to**: ADRS PO-013
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ queries Q, ∀ frontiers F:
  Q(S, F) at time t₁ = Q(S, F) at time t₂
  (identical expressions at identical frontiers return identical results)
```

#### Level 1 (State Invariant)
Query results are a pure function of the expression and the visible datom set.
No external randomness, no time-of-day dependency, no ordering dependency.

**Falsification**: Two evaluations of the same query at the same frontier returning
different results.

---

### INV-QUERY-003: Query Significance Tracking

**Traces to**: ADRS AS-007, PO-013
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
∀ queries Q executed against store S:
  an access event is recorded in the ACCESS LOG (separate from S)
  significance(d) = Σ decay(now - t) × query_weight(q) over queries returning d
```

#### Level 1 (State Invariant)
Every query generates an access event in the access log, NOT in the main store.
The access log feeds significance computation for ASSOCIATE.

**Falsification**: A query that completes without generating an access event, or
an access event recorded in the main store (violating AS-007's separation requirement).

---

### INV-QUERY-004: Branch Visibility

**Traces to**: ADRS AS-004
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
visible(branch b) = {d ∈ trunk | d.tx ≤ b.base_tx} ∪ {d | d.tx.branch = b}

Trunk commits after the fork point are NOT visible unless the branch rebases.
```

#### Level 1 (State Invariant)
A query against branch b sees exactly the trunk datoms at the fork point plus
the branch's own datoms. Snapshot isolation.

**Falsification**: A branch query that sees trunk datoms with tx > branch.base_tx
without an explicit rebase operation.

---

### INV-QUERY-005: Stratum Safety

**Traces to**: ADRS SQ-004, SQ-009
**Verification**: `V:TYPE`, `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ queries Q with stratum(Q) ∈ {0, 1}:         mode(Q) = Monotonic
∀ queries Q with stratum(Q) ∈ {2, 3}:         mode(Q) = Stratified
∀ queries Q with stratum(Q) = 4:              mode(Q) = Monotonic (conservative)
∀ queries Q with stratum(Q) = 5:              mode(Q) = Barriered (for critical decisions)
```

#### Level 1 (State Invariant)
The query engine classifies every query into a stratum and enforces the corresponding
mode constraint.

**Falsification**: A stratum 5 query executing in Monotonic mode.

---

### INV-QUERY-006: Semi-Naive Termination

**Traces to**: ADRS FD-003
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Semi-naive evaluation terminates iff the Datalog program is safe
(every variable in the head appears in a positive body literal).
Braid restricts to safe Datalog programs.

Termination: ΔT_P^(i+1) = ∅ after finitely many iterations
(because the Herbrand base is finite for a finite store).
```

#### Level 1 (State Invariant)
The parser rejects unsafe Datalog programs (unbound head variables).
Evaluation always terminates.

**Falsification**: A query that runs indefinitely (non-terminating fixpoint computation).

---

### INV-QUERY-007: Frontier as Queryable Data

**Traces to**: ADRS SQ-002, SQ-003
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Frontier is stored as :tx/frontier attribute.
Frontier = Map<AgentId, TxId> (vector-clock equivalent).

The Datalog extension [:frontier ?f] enables:
  "What does agent X know?" as an ordinary Datalog query.
```

#### Level 1 (State Invariant)
Frontier information is queryable via the same query engine as any other data.
No special-case API for frontier queries.

**Falsification**: Frontier data that is accessible only through a non-Datalog API.

---

### INV-QUERY-008: FFI Boundary Purity

**Traces to**: ADRS SQ-010
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
∀ derived functions f registered via FFI:
  f is a pure function: f(inputs) = f(inputs) always
  f has no side effects on the store
  Datalog provides the input query; f computes the result
```

#### Level 1 (State Invariant)
Three core computations are FFI: σ_a (entropy), σ_c (DAG traversal), spectral authority (SVD).
Each is a pure function from datom inputs to computed value.

**Falsification**: A derived function that modifies the store or returns different
results for identical inputs.

---

### INV-QUERY-009: Bilateral Query Symmetry

**Traces to**: ADRS SQ-006
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
The query layer is bilateral:
  Forward queries: spec → implementation status
  Backward queries: implementation → spec alignment

Both directions use the same Datalog apparatus. No asymmetric special-casing.
```

#### Level 1 (State Invariant)
For every forward query "does implementation X satisfy spec Y?" there is a symmetric
backward query "does spec Y accurately describe implementation X?"

**Falsification**: A forward query with no backward counterpart, or vice versa.

---

### INV-QUERY-010: Topology-Agnostic Results

**Traces to**: ADRS SQ-005
**Verification**: `V:MODEL`
**Stage**: 3

#### Level 0 (Algebraic Law)
```
∀ queries Q, ∀ dissemination topologies T₁, T₂:
  if all agents have received the same datom set:
    Q_T₁(S) = Q_T₂(S)
  (query results are independent of how datoms were distributed)
```

#### Level 1 (State Invariant)
Query results depend only on the datom set, not on the topology
(star, ring, mesh, hierarchy) used to distribute datoms.

**Falsification**: Two identical stores, assembled via different topologies, producing
different query results for the same expression.

---

### INV-QUERY-011: Projection Reification

**Traces to**: ADRS AS-008
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
∀ projection patterns P with access_count(P) > reification_threshold (default 3):
  P is stored as a first-class entity with significance score
  P is discoverable via ASSOCIATE
```

#### Level 1 (State Invariant)
Useful query patterns are promoted to entities, enabling the system to learn
"good ways to look at data."

**Falsification**: A projection pattern accessed 10+ times that is not stored as an entity.

---

### INV-QUERY-012: Topological Sort

**Traces to**: ADRS SQ-004, FD-003
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Given a directed acyclic graph G = (V, E) derived from datom references
(entity → entity via Ref-valued attributes):

topo_sort(G) produces a linear extension L = [v₁, v₂, ..., vₙ] such that:
  ∀ (u, v) ∈ E: index(u, L) < index(v, L)

If G contains a cycle: topo_sort(G) = Err(CycleDetected(scc))
```

The sort is deterministic: ties broken by EntityId lexicographic order.
This ensures reproducible implementation ordering across sessions and agents.

#### Level 1 (State Invariant)
The query engine provides `topo_sort(entity_type, dependency_attr)` as a
built-in graph query. It operates on the materialized reference graph
extracted from the store's EAVT index for the specified attribute.

Pipeline: extract subgraph → detect cycles (INV-QUERY-013) → if DAG, sort
via Kahn's algorithm → return ordered entity list with depth annotations.

#### Level 2 (Implementation Contract)
```rust
pub fn topo_sort(
    store: &Store,
    entity_type: &Attribute,    // e.g., :task/type
    dep_attr: &Attribute,       // e.g., :task/depends-on
) -> Result<Vec<(EntityId, usize)>, GraphError> {
    // Returns (entity, depth) pairs in topological order
    // Kahn's algorithm: O(V + E)
    // CycleDetected error includes the SCC via Tarjan (INV-QUERY-013)
}
```

**Falsification**: `topo_sort` returns an ordering where a dependency appears
after its dependent, OR returns `Ok` for a graph containing a cycle, OR
produces different orderings for the same graph across invocations.

---

### INV-QUERY-013: Cycle Detection via Tarjan SCC

**Traces to**: ADRS SQ-004, FD-003
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
tarjan_scc(G) decomposes G into strongly connected components:
  G = SCC₁ ∪ SCC₂ ∪ ... ∪ SCCₖ

Properties:
  1. Partition: every vertex belongs to exactly one SCC
  2. Maximality: no SCC can be extended with additional vertices
  3. DAG condensation: the graph of SCCs is always a DAG

∀ SCC with |SCC| > 1: SCC represents a circular dependency (error condition)
∀ SCC with |SCC| = 1: trivial SCC (no self-cycle unless self-referencing)
```

#### Level 1 (State Invariant)
Cycle detection is a precondition for topological sort, task derivation,
and schema layer validation. When cycles are detected, they are reported
as `GraphError::CycleDetected(Vec<Vec<EntityId>>)` containing all
non-trivial SCCs.

The condensation DAG (SCCs as vertices, inter-SCC edges) is also returned
for downstream algorithms that can operate on the condensed graph.

#### Level 2 (Implementation Contract)
```rust
pub struct SCCResult {
    pub components: Vec<Vec<EntityId>>,  // SCCs in reverse topological order
    pub condensation: Vec<Vec<usize>>,   // DAG adjacency list over SCC indices
    pub has_cycles: bool,                // true if any |SCC| > 1
}

pub fn tarjan_scc(
    store: &Store,
    entity_type: &Attribute,
    dep_attr: &Attribute,
) -> SCCResult {
    // Tarjan's algorithm: O(V + E), single DFS pass
}
```

**Falsification**: A vertex appears in two SCCs (partition violation), OR
a non-trivial SCC is not detected, OR the condensation contains a cycle.

---

### INV-QUERY-014: PageRank Scoring

**Traces to**: ADRS SQ-004
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
PageRank computes importance scores over the datom reference graph:

PR(v) = (1 - d)/|V| + d × Σ_{u→v} PR(u)/out_degree(u)

where d ∈ (0,1) is the damping factor (default: 0.85).

Properties:
  1. Normalization: Σ PR(v) = 1
  2. Convergence: power iteration converges for any connected graph
  3. Monotonicity: adding an edge u→v can only increase PR(v) (monotone query)
  4. Determinism: PR(G) at frontier F is a pure function of G and d
```

#### Level 1 (State Invariant)
PageRank is a Stratum 1 query (graph traversal, monotonic). It operates on
the reference subgraph for a given entity type and dependency attribute.
The result is a `Vec<(EntityId, f64)>` sorted by descending rank.

Convergence criterion: `max|PR^(i+1)(v) - PR^(i)(v)| < ε` (default ε = 1e-6).
Maximum iterations: 100 (safety bound).

#### Level 2 (Implementation Contract)
```rust
pub struct PageRankConfig {
    pub damping: f64,         // default: 0.85
    pub epsilon: f64,         // convergence: 1e-6
    pub max_iterations: u32,  // safety bound: 100
}

pub fn pagerank(
    store: &Store,
    entity_type: &Attribute,
    dep_attr: &Attribute,
    config: &PageRankConfig,
) -> Vec<(EntityId, f64)> {
    // Power iteration: O(iterations × (V + E))
    // Returns entities sorted by descending rank
}
```

**Falsification**: PageRank scores do not sum to 1.0 (within ε), OR
the same graph produces different scores across invocations, OR
power iteration fails to converge within `max_iterations`.

---

### INV-QUERY-015: Betweenness Centrality

**Traces to**: ADRS SQ-004
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
Betweenness centrality measures how often a vertex lies on shortest paths:

BC(v) = Σ_{s≠v≠t} σ_st(v) / σ_st

where σ_st = number of shortest paths from s to t,
      σ_st(v) = number of those passing through v.

Properties:
  1. Range: BC(v) ∈ [0, (|V|-1)(|V|-2)/2] (unnormalized)
  2. Normalized: BC_norm(v) = BC(v) / ((|V|-1)(|V|-2)/2) ∈ [0, 1]
  3. Bottleneck identification: high BC ⟹ vertex is a critical dependency
```

#### Level 1 (State Invariant)
Betweenness centrality is a Stratum 1 query. It identifies bottleneck
entities in the dependency graph — entities whose removal would maximally
disrupt connectivity. Used by R(t) work routing (INV-GUIDANCE-010) to
prioritize unblocking high-betweenness tasks.

#### Level 2 (Implementation Contract)
```rust
pub fn betweenness_centrality(
    store: &Store,
    entity_type: &Attribute,
    dep_attr: &Attribute,
    normalized: bool,
) -> Vec<(EntityId, f64)> {
    // Brandes' algorithm: O(V × E)
    // Returns entities sorted by descending centrality
}
```

**Falsification**: Normalized betweenness score falls outside [0, 1], OR
a vertex on every shortest path between two components has BC = 0.

---

### INV-QUERY-016: HITS Hub/Authority Scoring

**Traces to**: ADRS SQ-004, SQ-006
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
HITS computes dual scores — hub (outgoing links) and authority (incoming links):

auth(v) = Σ_{u→v} hub(u)
hub(v)  = Σ_{v→w} auth(w)

Converges via alternating power iteration with normalization.

Properties:
  1. Convergence: guaranteed for connected components
  2. Duality: hubs aggregate, authorities are depended upon
  3. Orthogonality to PageRank: HITS distinguishes aggregators from authorities
```

#### Level 1 (State Invariant)
HITS is a Stratum 1 query. The hub/authority duality maps to the bilateral
query layer (ADR-QUERY-008): authorities correspond to deeply specified
entities, hubs correspond to integration points that reference many authorities.

Used to bootstrap spectral authority (Stratum 3) at Stage 1 before
the full SVD-based computation is available.

#### Level 2 (Implementation Contract)
```rust
pub struct HITSResult {
    pub authorities: Vec<(EntityId, f64)>,  // sorted descending
    pub hubs: Vec<(EntityId, f64)>,         // sorted descending
    pub iterations: u32,                    // actual convergence iterations
}

pub fn hits(
    store: &Store,
    entity_type: &Attribute,
    dep_attr: &Attribute,
    max_iterations: u32,  // default: 100
    epsilon: f64,         // convergence: 1e-6
) -> HITSResult {
    // Alternating power iteration: O(iterations × (V + E))
}
```

**Falsification**: Authority scores not normalized (Σ auth² ≠ 1), OR
hub scores not normalized (Σ hub² ≠ 1), OR a vertex with only incoming
edges has a non-zero hub score.

---

### INV-QUERY-017: Critical Path Analysis

**Traces to**: ADRS SQ-004
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Given a weighted DAG G = (V, E, w) where w: V → ℝ⁺ (vertex weights = effort):

critical_path(G) = argmax over all source-to-sink paths P: Σ_{v ∈ P} w(v)

Properties:
  1. Existence: every DAG has at least one critical path
  2. Uniqueness: if vertex weights are distinct, the critical path is unique
  3. Optimality: reducing total project time requires reducing a critical-path task
  4. Slack: for non-critical vertices, slack(v) = latest_start(v) - earliest_start(v)
```

Vertex weights default to 1.0 (uniform effort). Custom weights can be
stored as datom attributes (e.g., `:task/effort`).

#### Level 1 (State Invariant)
Critical path is a Stratum 1 query (graph traversal on DAG, monotonic).
Requires topological sort (INV-QUERY-012) as a prerequisite.

Forward pass: compute earliest start times.
Backward pass: compute latest start times.
Critical path: vertices where earliest_start = latest_start (zero slack).

#### Level 2 (Implementation Contract)
```rust
pub struct CriticalPathResult {
    pub path: Vec<EntityId>,           // critical path vertices
    pub total_weight: f64,             // critical path length
    pub slack: HashMap<EntityId, f64>, // slack per vertex (0.0 = critical)
    pub earliest_start: HashMap<EntityId, f64>,
    pub latest_start: HashMap<EntityId, f64>,
}

pub fn critical_path(
    store: &Store,
    entity_type: &Attribute,
    dep_attr: &Attribute,
    weight_attr: Option<&Attribute>,  // None = uniform weight 1.0
) -> Result<CriticalPathResult, GraphError> {
    // Requires DAG (errors on cycle). O(V + E)
}
```

**Falsification**: A non-critical-path vertex has zero slack, OR
the critical path length is not the maximum path weight, OR
the algorithm succeeds on a graph containing cycles.

---

### INV-QUERY-018: k-Core Decomposition

**Traces to**: ADRS SQ-004
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
The k-core of G is the maximal subgraph where every vertex has degree ≥ k.

core_number(v) = max k such that v ∈ k-core(G)

Properties:
  1. Nesting: k₂ > k₁ ⟹ k₂-core ⊆ k₁-core
  2. Uniqueness: for each k, the k-core is unique
  3. Monotonicity: adding edges can only increase core numbers
  4. Density signal: high core number ⟹ tightly coupled component
```

#### Level 1 (State Invariant)
k-Core decomposition is a Stratum 1 query (monotonic). It identifies
tightly coupled clusters in the dependency graph — regions where entities
are heavily interdependent. High-core regions may indicate:
- Specification areas needing atomic implementation (can't do one without the others)
- Potential module boundaries (high internal cohesion)
- Risk concentrations (failure cascades within high-core regions)

#### Level 2 (Implementation Contract)
```rust
pub fn k_core_decomposition(
    store: &Store,
    entity_type: &Attribute,
    dep_attr: &Attribute,
) -> HashMap<EntityId, u32> {
    // Iterative peeling: O(V + E)
    // Returns core number for each entity
}
```

**Falsification**: A vertex in the k-core has degree < k within that subgraph,
OR k₂-core ⊄ k₁-core for k₂ > k₁ (nesting violation).

---

### INV-QUERY-019: Eigenvector Centrality

**Traces to**: ADRS SQ-004
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
Eigenvector centrality is the dominant eigenvector of the adjacency matrix A:

A × x = λ × x, where λ is the largest eigenvalue

EC(v) = x_v / max(x)

Properties:
  1. Non-negative (Perron-Frobenius for non-negative matrices)
  2. Recursive influence: high EC ⟹ connected to other high-EC vertices
  3. Convergence: power iteration converges for connected graphs
  4. Relationship to PageRank: PageRank ≈ damped eigenvector centrality
```

#### Level 1 (State Invariant)
Eigenvector centrality is a Stratum 1 query. It provides refined authority
scoring beyond PageRank by capturing recursive influence without damping.
Available at Stage 2 when branch-level analysis makes refined scoring valuable.

#### Level 2 (Implementation Contract)
```rust
pub fn eigenvector_centrality(
    store: &Store,
    entity_type: &Attribute,
    dep_attr: &Attribute,
    epsilon: f64,         // convergence: 1e-6
    max_iterations: u32,  // safety: 100
) -> Vec<(EntityId, f64)> {
    // Power iteration on adjacency matrix: O(iterations × (V + E))
}
```

**Falsification**: Any centrality score is negative (Perron-Frobenius violation),
OR scores are not normalized to [0, 1], OR the same graph produces
different centrality rankings across invocations.

---

### INV-QUERY-020: Articulation Points

**Traces to**: ADRS SQ-004
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
An articulation point (cut vertex) v is a vertex whose removal disconnects G:

articulation_point(v) ⟺ components(G \ {v}) > components(G)

A bridge (cut edge) (u,v) is an edge whose removal disconnects G:

bridge(u,v) ⟺ components(G \ {(u,v)}) > components(G)

Properties:
  1. Every bridge endpoint is an articulation point (in undirected graphs)
  2. Biconnected components partition edges, overlapping only at articulation points
  3. Articulation points represent single points of failure in the dependency graph
```

#### Level 1 (State Invariant)
Articulation point detection is a Stratum 1 query. It identifies entities
that are single points of failure — if the entity's specification or
implementation fails, it disconnects the dependency graph. Used for:
- Risk assessment (single-point-of-failure detection)
- Implementation priority (articulation points should be implemented first)
- Redundancy planning (add alternative paths around articulation points)

#### Level 2 (Implementation Contract)
```rust
pub struct ArticulationResult {
    pub articulation_points: Vec<EntityId>,
    pub bridges: Vec<(EntityId, EntityId)>,
    pub biconnected_components: Vec<Vec<EntityId>>,
}

pub fn articulation_points(
    store: &Store,
    entity_type: &Attribute,
    dep_attr: &Attribute,
) -> ArticulationResult {
    // DFS with low-link values: O(V + E)
}
```

**Falsification**: A vertex identified as an articulation point whose removal
does not disconnect the graph, OR a non-articulation vertex whose removal
does disconnect it.

---

### INV-QUERY-021: Graph Density Metrics

**Traces to**: ADRS SQ-004
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Graph density and derived health metrics:

density(G) = |E| / (|V| × (|V| - 1))  for directed graphs, ∈ [0, 1]

Derived metrics:
  avg_degree(G) = 2|E| / |V|
  clustering_coefficient(v) = 2 × triangles(v) / (deg(v) × (deg(v) - 1))
  avg_clustering(G) = Σ clustering_coefficient(v) / |V|

Properties:
  1. density ∈ [0, 1], with 0 = no edges, 1 = complete graph
  2. Monotonicity: adding edges increases density (monotone query)
  3. Health signal: very high density (>0.5) indicates over-coupling;
     very low density (<0.05) indicates under-specification
```

#### Level 1 (State Invariant)
Graph density is a Stratum 0 query (primitive, monotonic — edge counting).
It provides a store-level health metric for the dependency graph.
Reported by `braid status` and incorporated into the M(t) methodology
adherence score (INV-GUIDANCE-008).

#### Level 2 (Implementation Contract)
```rust
pub struct GraphDensityMetrics {
    pub vertex_count: usize,
    pub edge_count: usize,
    pub density: f64,
    pub avg_degree: f64,
    pub avg_clustering: f64,
    pub components: usize,         // number of weakly connected components
}

pub fn graph_density(
    store: &Store,
    entity_type: &Attribute,
    dep_attr: &Attribute,
) -> GraphDensityMetrics {
    // O(V + E) for density, O(V × E) for clustering coefficients
}
```

**Falsification**: Density falls outside [0, 1], OR vertex/edge counts
disagree with the store's datom count for the specified attribute.

---

### INV-QUERY-022: Spectral Computation Correctness

**Traces to**: exploration/06-cold-start.md §5.1 (spectral partitioning),
  exploration/11-topology-as-compilation.md §2.3.3 (Pass 3)
**Verification**: `V:PROP`
**Stage**: 3 (usage); 0 (FFI foundation)

#### Level 0 (Algebraic Law)
```
For any symmetric adjacency matrix A:
  L = D - A  where D = diag(row_sums(A))
  eigenvalues(L) are real and non-negative (L is positive semi-definite)
  eigenvalue_0 = 0 (always, for connected graphs)
  Fiedler vector = eigenvector corresponding to eigenvalue_1

The number of zero eigenvalues of L equals the number of connected components
in the graph defined by A.
```

#### Level 1 (State Invariant)
Spectral partitioning preserves intra-cluster connectivity: for any partition
produced by `spectral_partition(A, k)`, all nodes within each cluster are
connected via edges in the original adjacency matrix A.

#### Level 2 (Implementation Contract)
```rust
pub fn graph_laplacian(adjacency: &Matrix) -> Matrix;
pub fn fiedler_vector(laplacian: &Matrix) -> Vector;
pub fn spectral_partition(adjacency: &Matrix, k: usize) -> Vec<Vec<NodeId>>;

// All return deterministic results for the same input.
```

**Falsification**: `spectral_partition` returns a partition where two nodes in
the same cluster have no path between them in the original graph.

**proptest strategy**: For random adjacency matrices, verify:
  1. Laplacian has non-negative eigenvalues
  2. Number of zero eigenvalues = number of connected components
  3. Spectral partition preserves intra-cluster connectivity

---

### §3.5 ADRs

### ADR-QUERY-001: Datalog Over SQL

**Traces to**: SEED §4, §11, ADRS FD-003
**Stage**: 0

#### Problem
What query language should the datom store use?

#### Options
A) **Datalog** — declarative, natural graph joins, stratified evaluation maps to
   monotonic/non-monotonic distinction. CALM-compliant.
B) **SQL** — familiar but poor graph traversal. Requires recursive CTEs for transitive closure.
C) **Custom query language** — maximum flexibility but wheel reinvention.
D) **GraphQL** — web-oriented, not designed for formal verification.

#### Decision
**Option A.** Datalog's join semantics naturally express traceability queries
(goal → invariant → implementation → test). Stratified evaluation maps cleanly to
the monotonic/non-monotonic distinction (CALM theorem). Semi-naive evaluation avoids
redundant derivation.

#### Formal Justification
EAV triples are Datalog's native data model. The [entity, attribute, value] triple maps
directly to a Datalog fact `attr(entity, value)`. This eliminates the impedance mismatch
that SQL creates with EAV data.

---

### ADR-QUERY-002: Semi-Naive Bottom-Up Evaluation

**Traces to**: ADRS FD-003
**Stage**: 0

#### Problem
What evaluation strategy for Datalog?

#### Options
A) **Naive bottom-up** — iterate T_P until fixpoint. Correct but redundant.
B) **Semi-naive bottom-up** — only use new facts in each iteration. More efficient.
C) **Top-down (SLD resolution)** — goal-directed. Worse for materialized views.

#### Decision
**Option B.** Semi-naive avoids redundant derivation while maintaining bottom-up's
advantage for materialized views and incremental computation.

---

### ADR-QUERY-003: Six-Stratum Classification

**Traces to**: ADRS SQ-004, SQ-009
**Stage**: 0

#### Problem
How to organize query patterns by safety and cost?

#### Decision
Six strata: Stratum 0 (primitive, monotonic), Stratum 1 (graph traversal, monotonic),
Stratum 2 (uncertainty, mixed), Stratum 3 (authority, FFI), Stratum 4 (conflict detection,
conservatively monotonic), Stratum 5 (bilateral loop, non-monotonic).

The classification enables systematic safety analysis: Strata 0–1 are always safe.
Stratum 4 is safe but conservative (may overcount). Strata 2–3 and 5 require specific
frontier or barrier guarantees.

---

### ADR-QUERY-004: FFI for Derived Functions

**Traces to**: ADRS SQ-010
**Stage**: 1

#### Problem
Three core computations cannot be expressed in pure Datalog: σ_a (entropy), σ_c (DAG
traversal with memoization), spectral authority (SVD). How to handle this?

#### Options
A) **Extend Datalog** — add aggregation, recursion, linear algebra to the query language.
B) **FFI mechanism** — Datalog provides input data; Rust function computes result.
C) **Out-of-band computation** — separate process computes, results stored as datoms.

#### Decision
**Option B.** The FFI boundary cleanly separates declarative queries (Datalog's strength)
from imperative computation (Rust's strength). The derived function is pure — same inputs,
same output.

#### Formal Justification
Major architectural implication: three of four core coordination computations (σ_a, σ_c,
spectral authority) are derived functions. Option A would bloat the query language beyond
Datalog's well-understood theoretical properties. Option B preserves Datalog's properties
while enabling necessary computation.

---

### ADR-QUERY-005: Local Frontier as Default

**Traces to**: ADRS SQ-001
**Stage**: 0

#### Problem
What is the default query scope?

#### Options
A) **Local frontier only** — each agent sees only what it knows. No coordination.
B) **Consistent cut only** — all queries require sync barrier. Expensive.
C) **Local frontier default, consistent cut via optional sync barrier** — flexible.

#### Decision
**Option C.** Monotonic queries (Strata 0–1) are safe at any frontier, so local is fine.
Non-monotonic queries (Strata 2–5) may need a sync barrier for correctness-critical decisions,
but many non-monotonic queries produce useful approximate results at local frontier.

**Enforcement**: INV-QUERY-004 ensures branch visibility defaults to local frontier. No separate invariant needed — the ADR decision is structurally enforced.

---

### ADR-QUERY-006: Frontier as Datom Attribute

**Traces to**: ADRS SQ-002, SQ-003
**Stage**: 0

#### Problem
Where is frontier information stored?

#### Options
A) **External metadata** — frontier in a separate data structure, not queryable via Datalog.
B) **Datom attribute** — `:tx/frontier` is a regular attribute, queryable like any other data.

#### Decision
**Option B.** Frontier as a datom attribute enables Datalog frontier clauses:
`[:frontier ?f]` queries "what does agent X know?" as ordinary data. No special-case API.

#### Formal Justification
Preserves FD-012 (every command is a transaction) — frontier updates are transactions.
Preserves schema-as-data (C3) — frontier structure is described by schema attributes.

---

### ADR-QUERY-007: Projection Pyramid

**Traces to**: ADRS SQ-007
**Stage**: 1

#### Problem
How to compress query results for budget-aware output?

#### Decision
Four-level projection pyramid:
- π₀: full datoms (>2000 tokens available)
- π₁: entity summaries (500–2000 tokens)
- π₂: type summaries (200–500 tokens)
- π₃: store summary (≤200 tokens — single-line status)

Selection is budget-driven: at high k*, full detail; at low k*, compressed pointers.

---

### ADR-QUERY-008: Bilateral Query Layer

**Traces to**: ADRS SQ-006
**Stage**: 1

#### Problem
How to structure the query layer for bilateral verification?

#### Decision
Queries naturally partition into:
- **Forward-flow** (planning): epistemic uncertainty, crystallization candidates,
  delegation, ready tasks
- **Backward-flow** (assessment): conflict detection, drift candidates, aleatory
  uncertainty, absorption triggers
- **Bridge** (both): commitment weight, consequential uncertainty, spectral authority

Spectral authority is the explicit bridge — updated by backward-flow observations,
consumed by forward-flow decisions.

---

### ADR-QUERY-009: Full Graph Engine in Kernel

**Traces to**: ADRS SQ-004, FD-003
**Stage**: 0

#### Problem
Should graph algorithms (PageRank, betweenness, critical path, etc.) be
external tools or part of the kernel query engine?

#### Options
A) **External tools** — graph algorithms as separate binaries, called from CLI.
B) **FFI derived functions** — graph algorithms as Datalog FFI functions.
C) **Full kernel integration** — graph algorithms as first-class query operations
   alongside Datalog, with results stored as datoms.

#### Decision
**Option C.** The graph algorithms are the foundation of task derivation
(INV-GUIDANCE-009), work routing (INV-GUIDANCE-010), and topology fitness
(INV-GUIDANCE-011). Externalizing them would break the CRDT merge properties —
graph results must be datoms to merge across agents. FFI (Option B) would work
but forces unnatural Datalog encoding of results.

#### Formal Justification
Graph metrics over the datom reference graph are monotonic (adding edges/vertices
can only change scores, never invalidate the computation). This makes them
CALM-compliant (INV-QUERY-001). Results are stored as datoms with `:graph/*`
attributes, making them queryable, mergeable, and traceable. The ten algorithms
(INV-QUERY-012–021) cover the complete analysis needs identified in the ddis CLI
tasking system and the Beads graph triage engine.

---

### ADR-QUERY-010: Agent-Store Composition — Three Layers

**Traces to**: SEED §4, ADRS AA-001
**Stage**: 0

#### Problem
How does the agent interact with the datom store? LLM agents face a fundamental tension:
they have powerful associative reasoning (System 2) but no native structured retrieval
(System 1). Context assembly — getting the right information into the LLM's window — is
the bottleneck. Should context assembly be an application-level concern (the agent builds
its own prompts) or a protocol-level requirement (the system provides structured retrieval)?

#### Options
A) **Application-level context assembly** — The agent directly queries the store and
   formats results into its context window. Simple but leads to the "flat-buffer pathology":
   agents dump everything they can find into the context, losing structure, exceeding
   budgets, and missing relevant information that was not in the initial query.
B) **Protocol-level three-layer composition** — Context assembly is a first-class pipeline
   with three stages: ASSOCIATE (semantic retrieval), QUERY (structured Datalog), and
   ASSEMBLE (budget-aware projection). The pipeline is `assemble . query . associate :
   SemanticCue -> BudgetedContext`. Each stage has defined inputs, outputs, and invariants.
C) **Monolithic retrieval function** — A single `get_context(task)` function that handles
   all retrieval internally. Hides complexity but cannot be composed, tested, or tuned
   at each stage independently.

#### Decision
**Option B.** The three-layer composition `assemble . query . associate` is a protocol-
level requirement, not an application-level convenience. Each layer maps to a distinct
cognitive function:

```
associate : SemanticCue -> Set<EntityId>     — System 1: what might be relevant?
query     : Set<EntityId> -> Set<Datom>      — Structured: what exactly do I know?
assemble  : Set<Datom> -> BudgetedContext    — Projection: what fits in the budget?
```

#### Formal Justification
The dual-process architecture (System 1 associative + System 2 reasoning) is not a metaphor
— it is a structural requirement. The agent's LLM reasoning (System 2) operates on whatever
context it receives. If context assembly is ad hoc (Option A), the reasoning quality is
bounded by the ad hoc retrieval quality. By making the three-layer pipeline protocol-level,
each layer can be independently verified:
- ASSOCIATE can be evaluated by recall (did it find relevant entities?)
- QUERY can be verified by Datalog correctness (did it return accurate facts?)
- ASSEMBLE can be evaluated by budget compliance (did it fit within k* tokens?)

Option A conflates these concerns; Option C hides them. Option B makes each verifiable.

#### Consequences
- The QUERY namespace provides Datalog evaluation (this file)
- The ASSOCIATE function (Stratum 3) is a Datalog FFI function that bridges semantic
  similarity to structured retrieval
- The ASSEMBLE function uses the projection pyramid (ADR-QUERY-007) for budget-aware output
- Confusion signals (INV-SIGNAL-002) trigger re-association within the same agent cycle
- The pipeline is composable: each stage can be replaced or augmented independently

#### Falsification
This decision is wrong if: application-level context assembly (Option A) achieves equal or
better context quality (measured by agent task completion rate) compared to the three-layer
pipeline, making the protocol-level requirement unnecessary overhead.

---

### ADR-QUERY-011: Query Stability Score

**Traces to**: SEED §6, ADRS UA-009
**Stage**: 1

#### Problem
An agent queries the store and receives results. The agent may then make decisions based
on those results, including irrevocable decisions (committing code, publishing specifications,
resolving conflicts). How does the agent know whether the query results are stable enough
to act on? A result derived from recently-asserted, unverified facts may change when new
information arrives. A result derived from well-established, multiply-confirmed facts is
unlikely to change.

#### Options
A) **No stability information** — All query results are treated equally. The agent has
   no way to distinguish high-confidence from low-confidence results without manual
   inspection of provenance.
B) **Stability score per result** — Each query result carries a stability score derived
   from the commitment weights of the contributing facts. The agent can compare stability
   against a threshold before making irrevocable decisions.
C) **Mandatory sync barrier for all decisions** — Every decision requires a sync barrier
   to ensure the agent has seen all available information. Correct but expensive and
   unnecessary for decisions based on well-established facts.

#### Decision
**Option B.** Every query result has a computable stability score:

```
stability(R) = min{w(d) : d in F and d contributed to R}

where:
  R = the query result
  F = the set of facts (datoms) that contributed to R
  w(d) = commitment weight of datom d
       = f(provenance_type, verification_status, age, agent_authority)
```

The stability score is the minimum commitment weight among all contributing facts.
A result is only as stable as its weakest contributing fact. An agent can compare
`stability(R) >= threshold` before making irrevocable decisions.

#### Formal Justification
The stability score is the lattice meet (minimum) of commitment weights over contributing
facts. This is conservative by construction: a single low-confidence fact pulls down the
entire result's stability. This prevents the failure mode where a high-confidence result
masks a low-confidence dependency (e.g., "Task X is ready" derived from high-confidence
dependency analysis but low-confidence effort estimates).

The threshold mechanism provides a tunable safety margin. For irrevocable decisions
(code deployment, specification finalization), a high threshold requires all contributing
facts to be well-established. For exploratory queries (association, browsing), a low or
zero threshold allows acting on preliminary information.

This is distinct from the crystallization stability threshold (CR-005), which governs
when observations are promoted to stable specification elements. Query stability measures
the safety of acting on any query result, not just promoting datoms.

#### Consequences
- The query engine tracks which datoms contributed to each result (data lineage)
- Stability computation is O(|contributing datoms|) per result — lightweight
- Agents can set per-decision stability thresholds in their policy function (pi)
- A result with stability = 0 (contributed by a hypothesized fact) should never be used
  for irrevocable decisions without a sync barrier
- Stability scores are stored as datoms when query provenance is enabled (INV-STORE-014)

#### Falsification
An agent makes an irrevocable decision (e.g., committing a specification change) based on
a query result with stability = 0, and the result later changes when new information
arrives, causing the decision to be wrong. This is the exact scenario the stability score
is designed to prevent.

---

### ADR-QUERY-012: Spectral Graph Operations via nalgebra

**Traces to**: exploration/06-cold-start.md §5, exploration/11-topology-as-compilation.md §2.3
**Stage**: 0 (FFI infrastructure); 3 (spectral partitioning usage)

#### Problem
The topology framework requires spectral graph operations (Laplacian, eigendecomposition,
Fiedler vector) for cluster identification in the compilation middle-end (Pass 3:
spectral partitioning). How should these be implemented?

#### Options
A) **nalgebra FFI** — Use nalgebra crate's eigendecomposition
   - Pro: Mature, well-tested, pure Rust, no C dependency
   - Pro: Already planned for Stratum 3 SVD (spectral authority)
   - Con: nalgebra eigendecomposition is O(n^3); acceptable for n <= 50

B) **Custom implementation** — Hand-roll power iteration for Fiedler vector
   - Pro: Optimized for the specific use case (only need 2nd eigenvector)
   - Con: Reinventing tested linear algebra; error-prone

C) **lapack-sys FFI** — Call LAPACK's dsyev via FFI
   - Pro: Maximum performance
   - Con: C dependency; complicates build; overkill for n <= 50

#### Decision
**Option A.** nalgebra for all linear algebra operations (SVD, eigendecomposition,
Laplacian computation). The Stratum 3 FFI boundary already exists for spectral
authority. Spectral partitioning reuses the same infrastructure.

#### Consequences
- nalgebra dependency justified by two use cases (authority + topology)
- `graph_laplacian`, `fiedler_vector`, `spectral_partition` added to query engine
- Stratum 3 capability list extended to include spectral graph operations
- Stage 0 builds the FFI; Stage 3 uses it for topology

#### Falsification
This decision is wrong if: the nalgebra eigendecomposition is numerically unstable
for the class of graph Laplacians that arise from spec dependency graphs (typically
sparse, small n <= 50, symmetric positive semi-definite).

---

### §3.6 Negative Cases

### NEG-QUERY-001: No Non-Monotonic Queries in Monotonic Mode

**Traces to**: ADRS PO-013, SQ-004
**Verification**: `V:TYPE`, `V:PROP`

**Safety property**: `□ ¬(∃ query Q in Monotonic mode containing negation or aggregation)`

**Rust type-level enforcement**: The `QueryMode::Monotonic` variant triggers a parse-time
check that rejects negation/aggregation constructs.

---

### NEG-QUERY-002: No Query Side Effects

**Traces to**: ADRS SQ-010
**Verification**: `V:TYPE`

**Safety property**: `□ ¬(∃ query evaluation that modifies the datom set)`
Queries are read-only over the datom set. The only write is the provenance transaction
(INV-STORE-014) and the access log event (INV-QUERY-003).

**Formal statement**: FFI derived functions have signature `fn(&[Value]) -> Value` —
no `&mut Store` parameter.

---

### NEG-QUERY-003: No Unbounded Query Evaluation

**Traces to**: ADRS FD-003
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ query that runs indefinitely)`
All accepted Datalog programs are safe (every head variable appears in a positive body
literal) and operate over a finite Herbrand base.

**proptest strategy**: Generate random safe Datalog programs over random stores.
Verify all evaluations terminate within a bounded number of iterations.

---

### NEG-QUERY-004: No Access Events in Main Store

**Traces to**: ADRS AS-007
**Verification**: `V:TYPE`, `V:PROP`

**Safety property**: `□ ¬(∃ access event stored as a datom in the main store)`
Access events go to the ACCESS LOG, never to the main datom store.

**Formal statement**: The access log is a separate append-only structure. Storing access
events as datoms would create unbounded positive feedback (querying generates events,
events are queryable, queries generate more events...).

---

