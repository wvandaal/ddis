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
  Linear algebra: SVD of agent-entity matrix.
  QueryMode: Stratified (FFI to Rust linear algebra)
  Examples: spectral-authority, delegation-threshold

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
pub enum QueryExpr {
    Find {
        variables: Vec<Variable>,
        clauses: Vec<Clause>,
    },
    Pull {
        pattern: PullPattern,
        entity: EntityRef,
    },
}

pub enum Clause {
    /// Pattern match: [?e ?a ?v]
    Pattern(EntityRef, AttributeRef, ValueRef),
    /// Frontier scope: [:frontier ?f]
    Frontier(FrontierRef),
    /// Negation: (not [?e :attr ?v])
    Not(Box<Clause>),
    /// Aggregation: (aggregate ?var fn)
    Aggregate(Variable, AggregateFunc),
    /// FFI: call Rust function
    Ffi(FfiCall),
}

pub enum QueryMode {
    Monotonic,
    Stratified { frontier: Frontier },
    Barriered { barrier_id: BarrierId },
}

pub struct QueryResult {
    pub tuples: Vec<Vec<Value>>,
    pub mode: QueryMode,
    pub stratum: u8,
    pub provenance_tx: TxId,
}

impl Store {
    pub fn query(&mut self, expr: &QueryExpr, mode: QueryMode) -> Result<QueryResult, QueryError>;
}
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
**Verification**: `V:PROP`
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

