# §3. QUERY — Build Plan

> **Spec reference**: [spec/03-query.md](../spec/03-query.md) — read FIRST
> **Stage 0 elements**: INV-QUERY-001–002, 005–007, 012–014, 017, 021 (10 INV), ADR-QUERY-001–003, 005–006, 009, NEG-QUERY-001–004
> **Dependencies**: STORE (§1), SCHEMA (§2)
> **Cognitive mode**: Language-theoretic — Datalog semantics, CALM theorem, fixpoint evaluation

---

## §3.1 Module Structure

```
braid-kernel/src/query/
├── mod.rs          ← Query engine entry: parse → classify → evaluate → return
├── parser.rs       ← Datalog parser ([:find ... :where ...] syntax)
├── clause.rs       ← Clause, Pattern, Binding, BindingSet
├── evaluator.rs    ← Semi-naive bottom-up fixpoint evaluation
├── strata.rs       ← Stratum classification (0–5), CALM analysis
└── graph.rs        ← Graph engine: topo sort, SCC, PageRank, critical path, density
```

### Public API Surface

```rust
/// Top-level query expression (spec/03-query.md §3.3 Level 2).
/// Both spec and guide define Find(ParsedQuery) and Pull as the two query
/// modes (R6.7b alignment). ParsedQuery is the rich Datalog AST.
pub enum QueryExpr {
    Find(ParsedQuery),
    Pull {
        pattern: PullPattern,
        entity: EntityRef,
    },
}

/// Execute a Datalog query against the store.
pub fn query(store: &Store, expr: &QueryExpr, mode: QueryMode) -> Result<QueryResult, QueryError>;

/// Parse without executing (for validation).
pub fn parse(q: &str) -> Result<ParsedQuery, ParseError>;

/// Classify the stratum of a parsed query.
pub fn classify_stratum(q: &ParsedQuery) -> Stratum;

/// Parsed Datalog query — the rich AST for a Find expression.
/// This is the internal representation that QueryExpr::Find wraps.
pub struct ParsedQuery {
    pub find_spec:  FindSpec,
    pub where_clauses: Vec<Clause>,
    pub rules:      Vec<Rule>,
    pub inputs:     Vec<Input>,
}

/// Query result — uses BindingSet (variable->value maps).
/// Both spec and guide use BindingSet to preserve variable names for
/// downstream consumers (R6.7b alignment).
pub struct QueryResult {
    pub bindings: Vec<BindingSet>,
    pub stratum:  Stratum,
    pub mode:     QueryMode,     // Which mode was used (INV-QUERY-005)
    pub provenance_tx: TxId,     // Audit trail — transaction context of evaluation
}

/// Datomic-style find specification — all four Datomic find forms.
/// Now defined in both spec and guide (R6.7b alignment).
pub enum FindSpec {
    Relation(Vec<Variable>),    // [:find ?x ?y]
    Scalar(Variable),           // [:find ?x .]
    Collection(Variable),       // [:find [?x ...]]
    Tuple(Vec<Variable>),       // [:find [?x ?y]]
}

/// Clause types for Stage 0 (spec and guide now agree per R6.7b).
/// Aggregate and Ffi variants are deferred to Stage 1+ (Strata 2-5).
pub enum Clause {
    DataPattern(EntityPattern, AttributePattern, ValuePattern),
    RuleApplication(RuleName, Vec<Term>),
    NotClause(Box<Clause>),       // Stratum 2+ only
    OrClause(Vec<Vec<Clause>>),
    Frontier(FrontierRef),        // Frontier scope (INV-QUERY-007)
    // Stage 1+: Aggregate(Variable, AggregateFunc)
    // Stage 1+: Ffi(FfiCall)
}

pub enum Stratum {
    S0_Primitive,           // Pure data lookup, no joins
    S1_MonotonicJoin,       // Joins, recursion, no negation
    S2_StratifiedNegation,  // Negation (Stage 1+, rejected in monotonic mode)
    // S3_Aggregation, S4_Ffi, S5_External deferred to Stage 2+
}

// --- Graph Engine (INV-QUERY-012–021) ---

/// Topological sort result: entities in dependency order with depth.
pub fn topo_sort(
    store: &Store,
    entity_type: &Attribute,
    dep_attr: &Attribute,
) -> Result<Vec<(EntityId, usize)>, GraphError>;

/// Strongly connected components via Tarjan's algorithm.
pub fn tarjan_scc(
    store: &Store,
    entity_type: &Attribute,
    dep_attr: &Attribute,
) -> SCCResult;

/// PageRank scores over the dependency graph.
pub fn pagerank(
    store: &Store,
    entity_type: &Attribute,
    dep_attr: &Attribute,
    config: &PageRankConfig,
) -> Vec<(EntityId, f64)>;

/// Critical path through a weighted DAG.
pub fn critical_path(
    store: &Store,
    entity_type: &Attribute,
    dep_attr: &Attribute,
    weight_attr: Option<&Attribute>,
) -> Result<CriticalPathResult, GraphError>;

/// Graph density and derived health metrics.
pub fn graph_density(
    store: &Store,
    entity_type: &Attribute,
    dep_attr: &Attribute,
) -> GraphDensityMetrics;
```

---

## §3.1a ADR: Custom Datalog Evaluator (from D2-datalog-engines.md)

### ADR-IMPL-QUERY-001: Custom Datalog Engine Over Existing Crates

**Traces to**: SEED.md §4 (Axiom 4: Datalog), ADR-QUERY-002, INV-QUERY-006
**Stage**: 0
**Source**: D2-datalog-engines.md (research report, 2026-03-03)

#### Problem

Braid requires a Datalog engine that supports: runtime query construction (`braid query '[:find ...]'`),
semi-naive bottom-up evaluation, stratified negation (Stage 2+), EAV triple pattern matching,
frontier-scoped evaluation, and graph algorithms as kernel operations. Which engine to use?

#### Options Evaluated

| Feature | Datafrog | Crepe | Ascent | DDlog | Custom |
|---------|----------|-------|--------|-------|--------|
| Semi-naive | YES | YES | Likely | YES | Build |
| Stratified negation | Manual | YES | YES | YES | Build |
| Aggregation | NO | NO | YES | YES | Build |
| FFI | Trivial | Limited | NO | YES | Build |
| **Runtime queries** | YES | **NO** | **NO** | Partial | YES |
| Active maintenance | Low | Moderate | Active | DEAD | N/A |
| EAV pattern matching | Manual | NO | NO | NO | Build |
| Frontier scoping | NO | NO | NO | NO | Build |

#### Decision

**Custom Datalog evaluator.** Crepe and Ascent are disqualified because they are compile-time
macro systems with no runtime query construction. DDlog is archived. Datafrog provides useful
low-level primitives (leapjoin, semi-naive iteration) but at ~1500 LOC it may be simpler to
reimplement directly rather than take a lightly-maintained dependency.

#### Implementation Estimate

| Component | LOC Estimate | Complexity |
|-----------|-------------|------------|
| Query parser (Datomic-style) | 500-800 | Medium |
| Semi-naive evaluator | 400-600 | High |
| Stratum classifier | 200-300 | Medium |
| EAV index integration | 300-500 | Medium |
| Frontier scoping | 100-200 | Low |
| Graph algorithms (6 for Stage 0) | 600-1000 | Medium |
| **Total** | **2100-3400** | |

The evaluator is the critical-path item for Stage 0 -- everything else (harvest, seed,
guidance) depends on a functioning query layer.

#### Consequences

- No external Datalog dependency. The query engine is fully owned code.
- Parser implements Datomic-style `[:find ... :where ...]` syntax directly.
- Semi-naive evaluation operates over the store's EAVT/AEVT/AVET/VAET indexes.
- Only Strata 0-1 (monotonic) are needed at Stage 0. Strata 2-5 can be stubbed.
- Consider Datafrog as a future optimization substrate if the custom evaluator proves
  insufficient for large-scale workloads, but do not take the dependency at Stage 0.

#### Falsification

Evidence this decision is wrong would be: an existing crate that supports runtime query
construction, EAV pattern matching, frontier scoping, AND is actively maintained. If such
a crate emerges, re-evaluate. As of March 2026, none qualifies.

---

### Error Types

```rust
/// Errors during query parsing.
pub enum ParseError {
    /// Unexpected token at the given byte offset.
    UnexpectedToken { offset: usize, expected: String, found: String },
    /// Unbound variable in find spec (not bound in any where clause).
    UnboundVariable(String),
    /// Unsafe rule: head variable not bound in body (INV-QUERY-006).
    UnsafeRule { rule_name: String, unbound_var: String },
    /// Empty query (no find spec or no where clauses).
    EmptyQuery,
}

/// Errors during query evaluation.
pub enum QueryError {
    /// Query requires a higher stratum than the requested mode allows (INV-QUERY-005).
    StratumViolation { query_stratum: Stratum, mode: QueryMode },
    /// Parse failure (wraps ParseError).
    Parse(ParseError),
    /// Schema error: attribute referenced in query does not exist.
    UnknownAttribute(Attribute),
    /// Semi-naive evaluation did not converge (should not occur for safe Datalog).
    EvaluationTimeout { iterations: u32 },
}

/// Errors from graph algorithm execution.
pub enum GraphError {
    /// Graph contains cycles — includes SCC decomposition for diagnostics.
    CycleDetected(SCCResult),
    /// No entities match the entity_type filter.
    EmptyGraph,
    /// PageRank or eigenvector did not converge within max iterations.
    NonConvergence(u32),
}
```

---

## §3.2 Three-Box Decomposition

### Query Engine

**Black box** (contract):
- INV-QUERY-001: CALM compliance — monotonic queries produce monotonic results. Adding datoms
  to the store can only add results, never remove them.
- INV-QUERY-002: Query Determinism — identical expressions at identical frontiers return identical
  results; query results are a pure function of expression and visible datom set.
- INV-QUERY-005: Mode-stratum compatibility — monotonic mode rejects Stratum 2+ queries.
- INV-QUERY-006: Semi-Naive Termination — evaluation terminates for all valid (safe) Datalog
  queries; parser rejects unsafe programs with unbound head variables.
- INV-QUERY-007: Frontier as Queryable Data — frontier stored as `:tx/frontier` attribute,
  queryable via the same Datalog engine as any other data. No special-case API.

**State box** (internal design):
- Parser: convert string → `ParsedQuery`. Datomic-style syntax: `[:find ?vars :where [clauses]]`.
- Stratum classifier: walk the AST → classify each clause → overall stratum is max of all clauses.
- Evaluator: semi-naive bottom-up fixpoint.
  - Initialize working set from data patterns (index lookups).
  - Iterate: apply rules, compute new bindings (delta), add to working set.
  - Terminate when delta is empty (fixpoint reached).
- For Stage 0: only Stratum 0 (primitive) and Stratum 1 (monotonic join) supported.

**Clear box** (implementation):
- Parser: pest grammar or nom combinators. The syntax is:
  ```
  query = "[:find" find-spec ":where" clause+ "]"
  find-spec = var+ | var "." | "[" var "..." "]" | "[" var+ "]"
  clause = "[" pattern pattern pattern "]"
  pattern = var | literal | "_"
  var = "?" ident
  ```
- Index lookup: `[?e :db/ident ?name]` → scan AEVT index for attribute `:db/ident`.
- Join: Nested loop join for Stage 0. Hash join optimization deferred to Stage 1.
- Semi-naive delta: track new bindings per iteration → only join new bindings against old.
- **Access log interaction**: Semi-naive evaluation (INV-QUERY-003) operates within a single
  query execution. The access log (INV-STORE-009) records query datom reads but does not
  affect evaluation strategy — the query engine sees a snapshot, not a live stream.

### Stratum Classification

**Black box**: Given a parsed query, classify it into a stratum.
- S0: No joins, no rules — pure index lookup.
- S1: Joins and/or recursive rules, but no negation/aggregation.
- S2+: Deferred. Query with negation in monotonic mode → `QueryError::StratumViolation`.

**Clear box** — stratum assignment algorithm:

```rust
pub fn classify_stratum(q: &ParsedQuery) -> Stratum {
    let mut max_stratum = Stratum::S0_Primitive;

    for clause in &q.where_clauses {
        let clause_stratum = match clause {
            // Single data pattern with no shared variables with other clauses → S0
            Clause::DataPattern(..) => {
                if shares_variables_with_other_clauses(clause, &q.where_clauses) {
                    Stratum::S1_MonotonicJoin  // Join required
                } else {
                    Stratum::S0_Primitive      // Pure index lookup
                }
            }
            // Rule application always requires fixpoint evaluation → S1+
            Clause::RuleApplication(..) => Stratum::S1_MonotonicJoin,
            // Or-clause with joins → S1
            Clause::OrClause(..) => Stratum::S1_MonotonicJoin,
            // Negation → S2+ (deferred, rejected in monotonic mode)
            Clause::NotClause(..) => Stratum::S2_StratifiedNegation,
            // Frontier clause does not affect stratum
            Clause::Frontier(..) => Stratum::S0_Primitive,
        };
        max_stratum = max_stratum.max(clause_stratum);
    }

    // Recursive rules bump to at least S1
    if !q.rules.is_empty() {
        max_stratum = max_stratum.max(Stratum::S1_MonotonicJoin);
    }

    max_stratum
}
```

The overall stratum is the maximum across all clauses. Mode-stratum compatibility
(INV-QUERY-005) is checked at the `query()` entry point: if `mode == Monotonic` and
`stratum >= S2`, return `Err(QueryError::StratumViolation { .. })`.

---

## §3.2a Type-Level Encoding (Datalog)

| INV | Compile-Time Guarantee | Mechanism |
|-----|----------------------|-----------|
| INV-QUERY-005 | `QueryMode::Monotonic` rejects Stratum 2+ | `match (mode, stratum)` at eval entry |
| INV-QUERY-006 | Unsafe programs rejected at parse time | Parser rejects unbound head variables; evaluation always terminates |
| INV-QUERY-007 | Frontier queryable via Datalog | `:tx/frontier` as regular attribute; `[:frontier ?f]` clause |

---

## §3.3 Graph Engine

### Graph Algorithm Suite (INV-QUERY-012–014, 017, 021)

**Black box** (contract):
- INV-QUERY-012: Topological sort via Kahn's algorithm. Deterministic tie-breaking by EntityId.
  Returns `Err(CycleDetected(scc))` if the graph contains cycles.
- INV-QUERY-013: Tarjan SCC — partition, maximality, DAG condensation. Precondition for
  topological sort and cycle detection across the system.
- INV-QUERY-014: PageRank with configurable damping (default 0.85), ε-convergence (1e-6),
  max 100 iterations. Scores sum to 1.0. Monotonic: adding edges increases scores.
- INV-QUERY-017: Critical path analysis. Forward/backward pass, slack computation.
  Requires DAG (uses topo_sort). Default weight 1.0 per vertex; custom via weight attribute.
- INV-QUERY-021: Graph density metrics — density, avg degree, avg clustering, component count.
  Health signal for the dependency graph. Feeds `braid status` and M(t) (INV-GUIDANCE-008).

All graph algorithms share a common subgraph extraction pattern: given `entity_type` and
`dep_attr`, extract the directed graph from the store's reference-valued datoms.

**State box** (internal design):
- Common graph extraction: scan AEVT index for `dep_attr` → build adjacency list
  `HashMap<EntityId, Vec<EntityId>>`. Filter by `entity_type` attribute.
- Algorithms operate on the in-memory adjacency list, not raw datoms.
- Results are returned as typed structs, not raw binding sets — this is a higher-level
  API than Datalog queries, consuming the same store data.
- All algorithms are pure functions: no side effects, no mutation, deterministic.

**Clear box** (implementation):
```rust
/// Shared graph extraction from store datoms.
fn extract_subgraph(
    store: &Store,
    entity_type: &Attribute,
    dep_attr: &Attribute,
) -> DirectedGraph {
    // 1. Query store for entities matching entity_type
    // 2. For each entity, collect dep_attr Ref values → edges
    // 3. Build adjacency list
    DirectedGraph {
        vertices: BTreeSet<EntityId>,
        adj: HashMap<EntityId, Vec<EntityId>>,
        in_degree: HashMap<EntityId, usize>,
    }
}

/// Kahn's algorithm (INV-QUERY-012)
pub fn topo_sort(/* ... */) -> Result<Vec<(EntityId, usize)>, GraphError> {
    let graph = extract_subgraph(store, entity_type, dep_attr);
    let scc = tarjan_scc_internal(&graph);
    if scc.has_cycles {
        return Err(GraphError::CycleDetected(scc));
    }
    // Initialize queue with in_degree=0 vertices, sorted by EntityId
    // Process: dequeue, emit, decrement neighbors, enqueue new zero-degree
    // Track depth as max(predecessor_depths) + 1
}

/// Tarjan's SCC (INV-QUERY-013)
fn tarjan_scc_internal(graph: &DirectedGraph) -> SCCResult {
    // Single DFS pass with index + lowlink stack
    // O(V + E)
    // Returns components in reverse topological order
}

/// Power iteration PageRank (INV-QUERY-014)
pub fn pagerank(/* ... */) -> Vec<(EntityId, f64)> {
    let graph = extract_subgraph(store, entity_type, dep_attr);
    // Initialize: PR(v) = 1/|V| for all v
    // Iterate: PR'(v) = (1-d)/|V| + d × Σ PR(u)/out(u)
    // Converge when max|PR' - PR| < epsilon
    // Sort by descending rank
}

/// Forward/backward pass critical path (INV-QUERY-017)
pub fn critical_path(/* ... */) -> Result<CriticalPathResult, GraphError> {
    let topo = topo_sort(store, entity_type, dep_attr)?;
    // Forward pass: earliest_start[v] = max(earliest_start[dep] + w[dep])
    // total = max(earliest_start[v] + w[v]) for all sinks
    // Backward pass: latest_start[v] = min(latest_start[succ] - w[v])
    // Slack: latest_start[v] - earliest_start[v]
    // Critical path: vertices with slack = 0
}

/// Density and clustering (INV-QUERY-021)
pub fn graph_density(/* ... */) -> GraphDensityMetrics {
    let graph = extract_subgraph(store, entity_type, dep_attr);
    // density = |E| / (|V| × (|V| - 1))
    // avg_degree = 2|E| / |V|
    // clustering: for each v, count triangles, divide by deg(v)×(deg(v)-1)/2
    // components: BFS/DFS to count weakly connected components
}
```

### Type-Level Encoding (Graph)

| INV | Compile-Time Guarantee | Mechanism |
|-----|----------------------|-----------|
| INV-QUERY-012 | Cycle detection before sort | `Result<_, GraphError::CycleDetected>` forces caller to handle cycles |
| INV-QUERY-013 | SCCResult always complete | `has_cycles` flag + non-empty `components` |
| INV-QUERY-017 | DAG precondition enforced | `critical_path` calls `topo_sort` internally — cycle = error propagation |

---

## §3.4 LLM-Facing Outputs (All Queries)

### Agent-Mode Output — `braid query`

```
[QUERY] 5 results (Stratum 1, monotonic). Attributes: ?id, ?type.
  INV-STORE-001  invariant
  INV-STORE-002  invariant
  INV-STORE-003  invariant
  INV-STORE-004  invariant
  INV-STORE-005  invariant
---
↳ All results are stable (CALM monotonic — adding datoms can only add results).
  Explore: `braid query '[:find ?id ?dep :where [?e :spec/id ?id] [?e :spec/depends-on ?d] [?d :spec/id ?dep]]'`
```

### Agent-Mode Output — Graph Algorithms

```
[QUERY] Topological sort: 61 entities, max depth 7. 0 cycles.
  Depth 0: INV-STORE-001, INV-STORE-002, INV-STORE-003 (+5 more)
  Depth 1: INV-SCHEMA-001, INV-SCHEMA-002 (+3 more)
  ...
  Depth 7: INV-INTERFACE-003
---
↳ Critical path has 8 entities. Next on path: INV-STORE-004 (PageRank: 0.043, slack: 0.0)
```

```
[QUERY] PageRank: top-5 by authority.
  INV-STORE-004  0.043  (CRDT commutativity — 12 dependents)
  INV-QUERY-001  0.038  (CALM compliance — 8 dependents)
  INV-SCHEMA-001 0.035  (genesis completeness — 7 dependents)
  INV-STORE-001  0.031  (append-only — 6 dependents)
  INV-MERGE-001  0.028  (set-union — 5 dependents)
---
↳ High-authority entities should be implemented first. See INV-GUIDANCE-010 (R(t) routing).
```

### Error Messages

- **Parse error**: `Query error: unexpected token at position {N} — expected {expected} — See Datalog syntax in spec/03-query.md §3.3`
- **Stratum violation**: `Query error: negation requires stratified mode — use --mode stratified — See: INV-QUERY-005`
- **No results**: `[QUERY] 0 results. Verify attribute names match schema: braid query '[:find ?a :where [_ :db/ident ?a]]'`
- **Cycle detected**: `Graph error: cycle detected in {N} SCCs — resolve circular dependencies before topological sort — See: INV-QUERY-013`
- **Non-convergence**: `Graph error: PageRank did not converge in {N} iterations — graph may be disconnected — See: INV-QUERY-014`

---

## §3.5 Verification

### Key Properties

```rust
proptest! {
    // INV-QUERY-001: CALM monotonicity
    fn inv_query_001(store in arb_store(5), extra_datoms in arb_datoms(3), query in arb_monotonic_query()) {
        let r1 = query(&store, &query).bindings;
        let mut bigger_store = store.clone();
        bigger_store.add_datoms(extra_datoms);
        let r2 = query(&bigger_store, &query).bindings;
        // r1 ⊆ r2: every result in the smaller store appears in the bigger store
        for binding in &r1 {
            prop_assert!(r2.contains(binding));
        }
    }

    // INV-QUERY-002: Query Determinism (same expression + same frontier = same result)
    fn inv_query_002(store in arb_store(10), query in arb_monotonic_query()) {
        let r1 = query(&store, &query).bindings;
        let r2 = query(&store, &query).bindings;
        prop_assert_eq!(r1, r2);
    }

    // INV-QUERY-012: Topological sort respects all edges
    fn inv_query_012(store in arb_store_with_refs(10)) {
        let attr_type = Attribute::new(":spec/type").unwrap();
        let attr_dep = Attribute::new(":spec/depends-on").unwrap();
        if let Ok(order) = topo_sort(&store, &attr_type, &attr_dep) {
            let positions: HashMap<_, _> = order.iter().enumerate()
                .map(|(i, (eid, _))| (*eid, i)).collect();
            // For every edge u→v, u must appear before v
            for (eid, deps) in extract_edges(&store, &attr_type, &attr_dep) {
                for dep in deps {
                    if let (Some(&pos_u), Some(&pos_v)) = (positions.get(&dep), positions.get(&eid)) {
                        prop_assert!(pos_u < pos_v);
                    }
                }
            }
        }
    }

    // INV-QUERY-013: Tarjan SCC is a partition
    fn inv_query_013(store in arb_store_with_refs(10)) {
        let attr_type = Attribute::new(":spec/type").unwrap();
        let attr_dep = Attribute::new(":spec/depends-on").unwrap();
        let scc = tarjan_scc(&store, &attr_type, &attr_dep);
        // Every vertex in exactly one component
        let all_verts: Vec<_> = scc.components.iter().flatten().collect();
        let unique: BTreeSet<_> = all_verts.iter().collect();
        prop_assert_eq!(all_verts.len(), unique.len());
    }

    // INV-QUERY-014: PageRank normalization
    fn inv_query_014(store in arb_store_with_refs(5)) {
        let attr_type = Attribute::new(":spec/type").unwrap();
        let attr_dep = Attribute::new(":spec/depends-on").unwrap();
        let config = PageRankConfig::default();
        let ranks = pagerank(&store, &attr_type, &attr_dep, &config);
        if !ranks.is_empty() {
            let sum: f64 = ranks.iter().map(|(_, r)| r).sum();
            prop_assert!((sum - 1.0).abs() < 1e-4);
        }
    }

    // INV-QUERY-017: Critical path vertices have zero slack
    fn inv_query_017(store in arb_dag_store(5)) {
        let attr_type = Attribute::new(":spec/type").unwrap();
        let attr_dep = Attribute::new(":spec/depends-on").unwrap();
        if let Ok(result) = critical_path(&store, &attr_type, &attr_dep, None) {
            for eid in &result.path {
                let slack = result.slack.get(eid).copied().unwrap_or(f64::MAX);
                prop_assert!((slack - 0.0).abs() < 1e-10);
            }
        }
    }

    // INV-QUERY-021: Density in [0, 1]
    fn inv_query_021(store in arb_store_with_refs(10)) {
        let attr_type = Attribute::new(":spec/type").unwrap();
        let attr_dep = Attribute::new(":spec/depends-on").unwrap();
        let metrics = graph_density(&store, &attr_type, &attr_dep);
        prop_assert!(metrics.density >= 0.0 && metrics.density <= 1.0);
        prop_assert!(metrics.avg_clustering >= 0.0 && metrics.avg_clustering <= 1.0);
    }
}
```

---

## §3.6 Implementation Checklist

### Datalog Engine
- [ ] Datalog parser handles `[:find ... :where ...]` syntax
- [ ] Stratum classifier distinguishes S0 (primitive) from S1 (monotonic join)
- [ ] Semi-naive evaluator reaches fixpoint
- [ ] Index lookups use EAVT/AEVT/VAET/AVET correctly
- [ ] Mode-stratum compatibility enforced (monotonic rejects S2+)
- [ ] CALM monotonicity holds — INV-QUERY-001 (proptest)
- [ ] Query determinism holds — INV-QUERY-002 (proptest)
- [ ] Semi-naive termination holds — INV-QUERY-006 (proptest)
- [ ] Frontier queryable as data — INV-QUERY-007 (`:tx/frontier` attribute)

### Graph Engine
- [ ] `extract_subgraph()` — shared graph extraction from store datoms
- [ ] `topo_sort()` — Kahn's algorithm, EntityId tie-breaking (INV-QUERY-012)
- [ ] `tarjan_scc()` — single DFS, components + condensation DAG (INV-QUERY-013)
- [ ] `pagerank()` — power iteration, configurable damping/ε/max_iter (INV-QUERY-014)
- [ ] `critical_path()` — forward/backward pass, slack computation (INV-QUERY-017)
- [ ] `graph_density()` — density, avg degree, clustering, component count (INV-QUERY-021)
- [ ] Cycle detection integrates with topo_sort (CycleDetected error)
- [ ] All graph algorithms are pure functions (no IO, no mutation, deterministic)
- [ ] Graph results consumed by R(t) routing (INV-GUIDANCE-010)

### Integration
- [ ] Integration: genesis → schema query → spec-element query round-trip
- [ ] Integration: topo_sort → critical_path → pagerank pipeline
- [ ] Error messages follow protocol (what + why + recovery + ref)

---
