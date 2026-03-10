# 02 — The Coupling Model

> **Summary:** Task coupling — the degree to which progress on one task affects another — is
> measured through five independent mechanisms (file paths, invariants, schema, causal
> dependencies, historical patterns). These compose into a weighted signal with learnable
> weights stored as datoms, introduced progressively across implementation stages.

---

## 1. What Is Task Coupling?

At the most abstract level: **two tasks are coupled if progress on one affects the probability
of success on the other.** This can happen through several independent mechanisms.

Coupling is the primary input to topology selection: high coupling between two agents' tasks
means they need frequent merging (tight coordination). Low coupling means infrequent merging
suffices (loose coordination).

The coupling function:
```
coupling : Task x Task -> [0.0, 1.0]
```

Where 0.0 = completely independent (no coordination needed) and 1.0 = maximally coupled
(essentially the same task; must be done by one agent or with real-time sync).

---

## 2. The Five Coupling Mechanisms

### 2.1 Mechanism A: Shared File Paths

**Definition:** Two tasks are file-coupled if they modify overlapping sets of files.

```
file_coupling(T1, T2) = |files(T1) intersect files(T2)| / |files(T1) union files(T2)|
```

This is the Jaccard similarity coefficient of the file sets.

**Concrete example:**
```
T1: "Implement append-only transact" -> touches {src/store.rs, src/datom.rs}
T2: "Implement LIVE index update"    -> touches {src/store.rs, src/resolution.rs}

file_coupling = |{src/store.rs}| / |{src/store.rs, src/datom.rs, src/resolution.rs}|
              = 1/3 = 0.33
```

**Tradeoffs:**

| Strength | Weakness |
|----------|----------|
| Easiest to compute (string intersection) | Coarse granularity (two tasks touching different sections of same file register as coupled) |
| Directly predicts merge conflicts (primary coordination concern) | Doesn't capture semantic coupling (function signature change in file A breaks caller in file B — no file overlap) |
| Available before work starts (from file reservations or task descriptions) | Relies on agents accurately declaring which files they'll touch |
| Language-agnostic | Over-counts for large files, under-counts for small modules |

**When it dominates:** Early stages before rich invariant/schema data exists. Also dominates
when the primary coordination risk is merge conflicts (which is usually the case in practice).

**Source availability:** Always available. Can be computed from:
- File reservation declarations (Agent Mail / coordination substrate)
- Task descriptions (natural language extraction)
- Historical modification patterns (git log analysis)

---

### 2.2 Mechanism B: Shared Invariants

**Definition:** Two tasks are invariant-coupled if they implement invariants that have
dependencies in the invariant dependency graph (spec/17-crossref.md).

```
invariant_coupling(T1, T2) = max over pairs (i1, i2) of:
  1.0 / (1.0 + shortest_path_length(i1, i2, inv_dependency_graph))
  where i1 in invariants(T1), i2 in invariants(T2)
```

Directly connected invariants have coupling 0.5 (path length 1). Same invariant has
coupling 1.0 (path length 0). Invariants 3 hops apart have coupling 0.25.

**Concrete example:**
```
T1: "Verify append-only holds after merge"
    -> implements INV-STORE-001, INV-MERGE-001

T2: "Implement LIVE index rebuild"
    -> implements INV-STORE-010, INV-STORE-011

Invariant dependency: INV-STORE-010 depends on INV-STORE-001 (path length 1)
invariant_coupling = 1.0 / (1.0 + 1) = 0.50
```

**Tradeoffs:**

| Strength | Weakness |
|----------|----------|
| Captures semantic coupling that file paths miss | Requires spec-level traceability (tasks linked to invariants, requires C5) |
| Leverages existing invariant dependency graph | Indirect coupling (related invariants may not create actual coordination needs) |
| More stable than file paths (invariants don't change during implementation) | Only available once spec is mature enough to have dependency graph |
| Precisely captures "breaking someone else's proof" risk | |

**When it dominates:** Specification-heavy phases where the primary risk is violating someone
else's invariant, not merge conflicts.

**Source availability:** Available once spec elements are datoms (Stage 0b+). Computed via
Datalog query over the invariant dependency graph:

```datalog
[:find ?inv1 ?inv2 ?path-length
 :where
 [?t1 :task/implements ?inv1]
 [?t2 :task/implements ?inv2]
 [(shortest-path ?inv1 ?inv2 :inv/depends-on) ?path-length]]
```

---

### 2.3 Mechanism C: Shared Schema Attributes

**Definition:** Two tasks are schema-coupled if one modifies a schema attribute that the
other reads or queries.

```
schema_coupling(T1, T2) =
  if T1 modifies schema AND T2 queries modified attribute: 1.0
  if T1 modifies schema AND T2 queries same entity type:   0.5
  if both read same attribute (no modification):            0.0
  else:                                                     0.0
```

**Concrete example:**
```
T1: "Add resolution mode to :task/status"
    -> modifies schema entity for :task/status (changing valueType from Keyword to Ref)

T2: "Query all in-progress tasks"
    -> [:find ?t :where [?t :task/status :in-progress]]

Schema coupling = 1.0 (T2's query breaks if T1 changes :task/status type)
```

**Tradeoffs:**

| Strength | Weakness |
|----------|----------|
| Captures deepest coupling type (data model changes affect everything) | Only relevant for schema-modifying tasks (rare) |
| Schema is in the store -> queryable via Datalog | May overweight read-only access (querying != modifying) |
| Highest-impact coupling: schema errors propagate everywhere | Narrow applicability |

**When it dominates:** Schema evolution tasks. Rare but critical when it occurs.

**Source availability:** Available once schema-as-data is implemented (Stage 0b+). Computed by
comparing task descriptions against schema modification operations.

---

### 2.4 Mechanism D: Causal Transaction Dependencies

**Definition:** Two tasks are causally coupled if one task's transactions are causal
predecessors of the other's.

```
causal_coupling(T1, T2) =
  |{tx in T2.transactions : exists tx' in T1.transactions where tx' in tx.causal_predecessors}|
  / |T2.transactions|
```

Fraction of T2's transactions that causally depend on T1's output.

**Concrete example:**
```
T1 asserts: [task:a1 :task/schema-version 2 tx:100 assert]
T2 reads task:a1 and assumes schema-version=2: tx:105.causal_predecessors = [tx:100]

causal_coupling = 1.0 (T2 literally depends on T1's output)
```

**Tradeoffs:**

| Strength | Weakness |
|----------|----------|
| Most precise signal (actual data flow, not proximity) | Only available during/after execution (can't predict before work starts) |
| Already part of datom model (causal predecessors in every transaction) | May capture false dependencies (T2 reads T1's output but doesn't semantically depend on it) |
| Captures temporal ordering: T2 cannot progress until T1's assertions propagate | Requires agents to correctly record causal predecessors |

**When it dominates:** During execution, for real-time topology adjustment. Not useful for
up-front planning.

**Source availability:** Available once multi-agent transactions are recorded (Stage 3).
Computed directly from TxData.causal_predecessors:

```datalog
[:find ?t1 ?t2 ?coupling
 :where
 [?tx2 :tx/task ?t2]
 [?tx2 :tx/causal-predecessors ?preds]
 [?tx1 :tx/task ?t1]
 [(contains ?preds ?tx1)]
 [(count-ratio ?preds ?tx1-count ?tx2-total) ?coupling]]
```

---

### 2.5 Mechanism E: Historical Co-modification Patterns

**Definition:** Two task types are historically coupled if concurrent work on both has
historically resulted in coordination difficulties (conflicts, quality drops, rework).

```
historical_coupling(T1, T2) =
  mean over past sessions where type(T1') = type(T1) and type(T2') = type(T2):
    (quality_when_sequential - quality_when_concurrent) / quality_when_sequential
```

The quality differential between sequential and concurrent execution for similar task types.

**Concrete example:**
```
Session 1: store + query tasks concurrent -> 3 merge conflicts, quality 0.72
Session 2: store + query tasks concurrent -> 2 merge conflicts, quality 0.75
Session 3: store + query tasks sequential -> 0 conflicts, quality 0.94

historical_coupling(store, query) = (0.94 - 0.735) / 0.94 = 0.218
```

**Tradeoffs:**

| Strength | Weakness |
|----------|----------|
| Captures coupling no static analysis can predict (emergent from dev patterns, agent behaviors, codebase complexity) | Cold-start problem (no data in first session) |
| Improves over time (more sessions -> better predictions) | May overfit to past patterns that don't generalize to new contexts |
| Accounts for factors invisible to other signals (agent skill, time-of-day, codebase complexity) | Requires outcome measurement (need to know if coordination succeeded or failed) |

**When it dominates:** After 3+ sessions with outcome data. Becomes the most reliable
signal long-term because it captures the actual experienced coupling, not just predicted.

**Source availability:** Available once coordination outcomes are harvested (Stage 3+, but
can begin collecting data earlier). Computed from harvested outcome datoms:

```datalog
[:find ?task-type-1 ?task-type-2 ?coupling
 :where
 [?outcome :outcome/task-type-1 ?task-type-1]
 [?outcome :outcome/task-type-2 ?task-type-2]
 [?outcome :outcome/concurrent true]
 [?outcome :outcome/quality ?q-concurrent]
 [?seq :outcome/task-type-1 ?task-type-1]
 [?seq :outcome/task-type-2 ?task-type-2]
 [?seq :outcome/concurrent false]
 [?seq :outcome/quality ?q-sequential]
 [(- ?q-sequential ?q-concurrent) ?delta]
 [(/ ?delta ?q-sequential) ?coupling]]
```

---

## 3. Composite Coupling Signal

### 3.1 Formula

No single coupling mechanism dominates across all contexts. The composite signal combines
all five with learned weights:

```
coupling(T1, T2) = w_f * file_coupling(T1, T2)
                 + w_i * invariant_coupling(T1, T2)
                 + w_s * schema_coupling(T1, T2)
                 + w_c * causal_coupling(T1, T2)
                 + w_h * historical_coupling(T1, T2)
```

where w_f + w_i + w_s + w_c + w_h = 1.0 and all w >= 0.

### 3.2 Weights as Datoms

The weights are stored in the datom store and updated through the bilateral learning loop:

```
[weights:coupling :coupling/file-weight       0.40 tx:genesis assert]
[weights:coupling :coupling/invariant-weight   0.25 tx:genesis assert]
[weights:coupling :coupling/schema-weight      0.10 tx:genesis assert]
[weights:coupling :coupling/causal-weight      0.15 tx:genesis assert]
[weights:coupling :coupling/historical-weight  0.10 tx:genesis assert]
```

### 3.3 Weight Learning

At each harvest, compare predicted coupling (composite score) against observed coupling
(from actual conflict/overhead data). Adjust weights to minimize prediction error:

```
error(session) = |predicted_coupling - observed_coupling|

For each weight w_i:
  if mechanism i's signal positively correlated with observed coupling:
    w_i += learning_rate * correlation_strength
  else:
    w_i -= learning_rate * |correlation_strength|

Normalize: w_i = w_i / sum(w)
```

The learning rate and loss function are also datoms:

```
[learning:coupling :learning/rate 0.05 tx:genesis assert]
[learning:coupling :learning/loss-function :mse tx:genesis assert]
```

### 3.4 Algebraic Property: Semilattice Preservation

The composite coupling score preserves the semilattice structure of individual signals:

Each individual coupling signal is a join-semilattice:
- More data -> coupling can only increase or stay the same (monotonically)
- file_coupling(T1, T2) only increases as more shared files are discovered
- invariant_coupling(T1, T2) only increases as more dependency paths are found
- historical_coupling(T1, T2) only increases as more negative outcomes are observed

The weighted combination of join-semilattices is itself a join-semilattice (weighted join).

This means the composite coupling function composes correctly with CRDT merge:
- When two agents merge their coupling observations, the composite score converges to
  the same value regardless of merge order (commutativity)
- Coupling scores can only grow through observation (monotonicity)
- Explicit coupling reduction requires retraction (non-monotonic, requires barrier)

---

## 4. Staged Introduction

Each stage adds coupling signals as they become available:

| Stage | Available Signals | Default Weights | Rationale |
|-------|------------------|-----------------|-----------|
| 0 | File paths only | [1.0, 0, 0, 0, 0] | Only file information available from task descriptions |
| 0b | + Invariant coupling | [0.50, 0.35, 0, 0, 0.15] | Spec elements become datoms; dependency graph queryable |
| 1 | + Schema coupling | [0.40, 0.25, 0.10, 0, 0.25] | Schema-as-data complete; attribute modifications detectable |
| 3 | + Causal + Historical | [0.25, 0.20, 0.10, 0.15, 0.30] | Multi-agent transactions recorded; outcome data harvested |

The weight vector evolves through harvest/seed. The convergence trajectory is queryable:

```datalog
[:find ?stage ?weights ?prediction-accuracy
 :where
 [?w :coupling-weights/stage ?stage]
 [?w :coupling-weights/vector ?weights]
 [?w :coupling-weights/accuracy ?prediction-accuracy]]
```

Note: Even in Stage 0 with only file coupling, the topology selection is useful. The cold-start
algorithm (06-cold-start.md) uses file coupling as its primary signal.

---

## 5. Coupling Matrix and Graph

### 5.1 The Coupling Matrix

For n agents with assigned tasks, the coupling matrix C is an n x n symmetric matrix:

```
C[i][j] = coupling(tasks_of(agent_i), tasks_of(agent_j))
```

where coupling between agents is the maximum coupling between any pair of their assigned tasks:

```
coupling(tasks_of(alpha), tasks_of(beta)) =
  max over (T_a in tasks(alpha), T_b in tasks(beta)) of: coupling(T_a, T_b)
```

Using max (not mean) because even one tightly coupled pair of tasks requires coordination
between the agents.

### 5.2 The Coupling Graph

The coupling matrix induces a weighted graph:
- Vertices: agents
- Edges: pairs with coupling > 0
- Edge weight: coupling score

This graph is the primary input to:
- Topology selection (03-topology-definition.md, Phi component)
- Graph partitioning for hybrid topologies (spectral partition of coupling graph)
- Cold-start algorithm (06-cold-start.md)

### 5.3 Coupling Threshold

The coupling threshold theta determines which edges are "significant":

```
For coupling(alpha, beta) > theta: agents need explicit coordination
For coupling(alpha, beta) <= theta: agents can work independently

Default: theta = 0.3 (datom, tunable)
```

The threshold is used for:
- Graph partitioning: connected components of G_theta = {edges with weight > theta}
- Merge frequency mapping: coupling -> frequency (see 03-topology-definition.md)
- Cold-start clustering: spectral_partition(C, threshold=theta)

---

## 6. Worked Example: Braid Stage 0

Six agents working on Braid Stage 0:

```
Agent alpha: Store kernel (src/store.rs, src/datom.rs)
Agent beta:  Layout (src/layout.rs, src/store.rs)
Agent gamma: Schema (src/schema.rs)
Agent delta: Query engine (src/query/*.rs, src/schema.rs)
Agent epsilon: Harvest pipeline (src/harvest.rs, src/seed.rs)
Agent zeta: Interface (src/commands/*.rs)
```

### 6.1 File Coupling Matrix

```
         alpha  beta  gamma  delta  epsilon  zeta
alpha     ---   0.33  0.00   0.00   0.00    0.00
beta     0.33   ---   0.00   0.00   0.00    0.00
gamma    0.00  0.00   ---    0.33   0.00    0.00
delta    0.00  0.00  0.33    ---    0.00    0.00
epsilon  0.00  0.00  0.00   0.00    ---     0.00
zeta     0.00  0.00  0.00   0.00   0.00     ---
```

(alpha and beta share store.rs; gamma and delta share schema.rs)

### 6.2 Invariant Coupling (if available)

```
         alpha  beta  gamma  delta  epsilon  zeta
alpha     ---   0.50  0.25   0.17   0.10    0.05
beta     0.50   ---   0.25   0.17   0.10    0.05
gamma    0.25  0.25   ---    0.50   0.17    0.10
delta    0.17  0.17  0.50    ---    0.10    0.05
epsilon  0.10  0.10  0.17   0.10    ---     0.25
zeta     0.05  0.05  0.10   0.05   0.25     ---
```

(INV-STORE -> INV-LAYOUT dependency; INV-SCHEMA -> INV-QUERY dependency)

### 6.3 Composite Coupling (Stage 0b weights: [0.50, 0.35, 0, 0, 0.15])

```
         alpha  beta  gamma  delta  epsilon  zeta
alpha     ---   0.40  0.09   0.06   0.04    0.02
beta     0.40   ---   0.09   0.06   0.04    0.02
gamma    0.09  0.09   ---    0.34   0.06    0.04
delta    0.06  0.06  0.34    ---    0.04    0.02
epsilon  0.04  0.04  0.06   0.04    ---     0.09
zeta     0.02  0.02  0.04   0.02   0.09     ---
```

### 6.4 Resulting Topology (with threshold 0.3)

Coupling clusters (connected components above 0.3):
- Cluster A: {alpha, beta} (coupling 0.40)
- Cluster B: {gamma, delta} (coupling 0.34)
- Independent: {epsilon}, {zeta}

Recommended topology:
- Cluster A: Mesh(alpha, beta) — |cluster| <= 3
- Cluster B: Mesh(gamma, delta) — |cluster| <= 3
- epsilon: Solo
- zeta: Solo
- Inter-cluster: Star with bridge agent (highest inter-cluster coupling)

This matches intuition: store/layout are tightly coupled, schema/query are tightly coupled,
harvest and interface are relatively independent.

---

## 7. Traceability

| Concept | Traces to |
|---------|-----------|
| Coupling as input to topology selection | INS-005 (task-level regime routing) |
| File coupling as primary signal | Practical observation: merge conflicts are the primary coordination cost |
| Invariant coupling via dependency graph | spec/17-crossref.md (invariant dependency chains) |
| Schema coupling | spec/02-schema.md (schema-as-data, C3) |
| Causal coupling | spec/01-store.md INV-STORE-014 (TxData.causal_predecessors) |
| Historical coupling via harvest | SEED.md S5 (harvest/seed lifecycle), S7 (self-improvement loop) |
| Composite signal with learnable weights | SEED.md S7 (fitness function, bilateral feedback) |
| Semilattice preservation | spec/01-store.md (G-Set CvRDT laws L1-L5) |

---

*Next: `03-topology-definition.md` — the formal definition T = (G, Phi, Sigma, Pi).*
