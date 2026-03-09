# 11 — Topology as Compilation

> **Summary:** The specification dependency graph IS a program encoding which work is coupled,
> which can proceed in parallel, and which requires sequential ordering. In Braid — uniquely
> among multi-agent frameworks — this program is queryable data in the same store as the
> coordination state. This means the optimal coordination topology can be **derived from
> structure** (compiled) rather than **discovered from execution** (interpreted). The bilateral
> feedback loop becomes profile-guided optimization (PGO), refining structural predictions
> with observed outcomes. This eliminates the cold-start problem, inverts the learning loop
> from discovery to verification, and unifies task decomposition with topology selection.

---

## 1. The Core Insight

### 1.1 The Reactive Paradigm (Current Framework, Documents 00–10)

The topology framework as designed in documents 00–10 is fundamentally **reactive**:

```
Execute → Observe outcomes → Compute coupling → Adjust topology → Measure F(T) → Repeat
```

This is the standard approach in distributed systems and multi-agent coordination. Every
existing framework (AutoGen, CrewAI, LangGraph, Swarm, etc.) works this way: coordination
patterns are **discovered** at runtime through trial, error, and feedback.

The reactive paradigm has inherent costs:
- **Cold-start penalty**: No prior data → default to maximum coordination → 3-10 sessions to converge
- **Discovery waste**: Exploration phase produces suboptimal coordination, damaging early outcomes
- **Lag**: Coupling changes are detected AFTER they cause problems (conflicts, staleness)
- **Asymptotic**: Converges toward optimal but may never reach it (noise, non-stationarity)

### 1.2 The Compilation Paradigm (This Document)

Braid has something no other multi-agent framework has: **the specification is data in the
same store as the coordination state.** Spec elements are datoms. Dependencies between spec
elements are datoms. The coupling structure of the work is ALREADY ENCODED in the store
before any agent begins working.

This enables a qualitatively different paradigm:

```
Analyze spec structure → Derive coupling → Compile topology → Execute → Compare prediction
to outcome → Refine compiler → Repeat
```

The specification is the **source language**. The topology is the **compiled execution plan**.
The bilateral feedback loop is **profile-guided optimization** (PGO).

### 1.3 Why This Is Uniquely Braid

Other multi-agent frameworks cannot do this because:

1. **They don't have specifications as queryable data.** Their task descriptions are strings,
   not structured datoms with typed dependencies. You can't query the coupling structure of
   a natural-language task list.

2. **They don't have a unified store.** Their coordination state lives in separate databases,
   message queues, or in-memory structures. There's no single algebra connecting task structure
   to coordination topology.

3. **They don't self-bootstrap.** Their coordination mechanisms are hardcoded, not specified
   in the same formalism as the work they coordinate. Braid's topology decisions are datoms
   that can be queried, versioned, and optimized by the same system that produces them.

This is the payoff of constraint C7 (self-bootstrap) that the original SEED.md didn't
anticipate: the specification bootstrapping the system isn't just philosophically elegant —
it's the key that unlocks **compile-time topology optimization**.

---

## 2. The Compiler Architecture

### 2.1 The Compilation Pipeline

```
                    SPECIFICATION (datoms)
                           |
                    +------v------+
                    |  FRONT-END  |  Parse spec dependency graph
                    |             |  into coupling IR
                    +------+------+
                           |
                    +------v------+
                    |  MIDDLE-END |  Optimize: partition, merge,
                    |             |  critical path, CALM classify
                    +------+------+
                           |
                    +------v------+
                    |  BACK-END   |  Emit topology T=(G,Phi,Sigma,Pi)
                    |             |  with full configuration
                    +------+------+
                           |
                    COMPILED TOPOLOGY
                           |
                    +------v------+
                    |  EXECUTION  |  Agents work under compiled topology
                    +------+------+
                           |
                    +------v------+
                    |  PROFILING  |  Harvest outcomes, measure F(T),
                    |             |  compare predicted vs observed
                    +------+------+
                           |
                    +------v------+
                    |     PGO     |  Feed prediction deltas back
                    |             |  to improve next compilation
                    +------+------+
                           |
                    (next compilation pass)
```

### 2.2 Front-End: Spec Graph → Coupling IR

The front-end parses the specification dependency graph into a **coupling intermediate
representation** (Coupling IR). Unlike the heuristic five-mechanism coupling model
(document 02), this is **exact structural coupling** derived from datom relationships.

#### 2.2.1 Spec Dependency Extraction

The specification contains typed relationships between elements:

```
;; Invariant depends on another invariant
[inv:STORE-001 :inv/depends-on inv:STORE-004 tx:100 assert]

;; ADR references an invariant
[adr:STORE-008 :adr/affects inv:STORE-001 tx:101 assert]

;; Invariant constrains a schema attribute
[inv:SCHEMA-003 :inv/constrains attr:db/valueType tx:102 assert]

;; Negative case bounds an invariant
[neg:MUTATION-001 :neg/tests inv:STORE-001 tx:103 assert]

;; Section covers a namespace
[sec:harvest-pipeline :sec/namespace :harvest tx:104 assert]
```

These relationships form a **directed graph** where:
- **Nodes** are spec elements (invariants, ADRs, negative cases, sections, schema attributes)
- **Edges** are typed dependencies (depends-on, affects, constrains, tests, covers)

#### 2.2.2 Task-to-Spec Binding

Each task is bound to the spec elements it touches:

```
[task:t1 :task/touches inv:STORE-001 tx:200 assert]
[task:t1 :task/touches inv:STORE-004 tx:200 assert]
[task:t2 :task/touches inv:STORE-001 tx:201 assert]
[task:t2 :task/touches adr:STORE-008 tx:201 assert]
```

#### 2.2.3 Structural Coupling Query

The coupling between two tasks is the number of spec elements they share, weighted by
dependency depth:

```datalog
;; Direct coupling: shared spec elements
[:find ?t1 ?t2 (count ?shared)
 :where
 [?t1 :task/touches ?shared]
 [?t2 :task/touches ?shared]
 [(!= ?t1 ?t2)]]

;; Transitive coupling: spec elements within distance k
[:find ?t1 ?t2 (count ?reachable)
 :where
 [?t1 :task/touches ?e1]
 [?t2 :task/touches ?e2]
 [(!= ?t1 ?t2)]
 (reachable-within ?e1 ?e2 2 ?reachable)]  ;; k=2 hops in spec graph
```

#### 2.2.4 Coupling IR Definition

The Coupling IR is a weighted graph:

```
CouplingIR = {
  nodes: Set<TaskId>,
  edges: Map<(TaskId, TaskId), CouplingWeight>,
  annotations: Map<(TaskId, TaskId), CouplingAnnotation>
}

CouplingWeight = {
  structural: f64,        // From spec dependency graph (exact)
  historical: f64,        // From PGO data (learned, initially 0)
  combined: f64,          // Weighted combination
  confidence: f64,        // How much to trust this prediction
}

CouplingAnnotation = {
  shared_elements: Set<SpecElementId>,  // WHY they're coupled
  dependency_type: DependencyType,      // WHAT kind of coupling
  monotonic: bool,                      // CALM classification
}
```

The key innovation: the Coupling IR carries **annotations** explaining WHY tasks are coupled
(which spec elements they share) and WHAT kind of coupling it is (monotonic or non-monotonic).
This enables the middle-end to make CALM-aware optimization decisions.

#### 2.2.5 Structural vs Heuristic Coupling

The five coupling mechanisms from document 02 are **heuristic approximations** of the
structural coupling:

| Mechanism | Relationship to Structural Coupling |
|-----------|-------------------------------------|
| A: File paths | Proxy for spec element co-location |
| B: Shared invariants | Direct subset of structural coupling |
| C: Schema attributes | Direct subset of structural coupling |
| D: Causal transactions | Runtime refinement of structural prediction |
| E: Historical patterns | PGO data for structural prediction |

In the compilation paradigm:
- **Mechanisms B and C** are computed by the front-end (structural, exact)
- **Mechanism A** is a fallback when spec bindings are incomplete
- **Mechanisms D and E** are PGO data that refine structural predictions

The five-mechanism model from document 02 doesn't go away — it becomes the **fallback for
uncompilable work** (tasks without spec bindings) and the **PGO signal** for compiled work.

---

### 2.3 Middle-End: Optimization Passes

The middle-end transforms the Coupling IR into an optimized execution plan. Like a compiler's
middle-end, it applies a sequence of optimization passes, each preserving correctness while
improving performance.

#### 2.3.1 Pass 1: CALM Classification

Classify every edge in the Coupling IR as monotonic or non-monotonic:

```
For each edge (t1, t2) with shared spec elements S:
  if all elements in S use monotonic resolution modes
     (lattice-resolved, multi-value, or read-only):
    mark edge as MONOTONIC
  else if any element in S uses non-monotonic resolution
     (LWW with potential override, retraction-dependent):
    mark edge as NON-MONOTONIC
```

Monotonic edges are safe for parallel execution without barriers (CALM theorem).
Non-monotonic edges require ordering constraints or barrier synchronization.

This is the compilation analog of **dependency analysis** in a traditional compiler:
monotonic edges are like independent instructions that can be reordered; non-monotonic
edges are like data dependencies that enforce ordering.

#### 2.3.2 Pass 2: Critical Path Analysis

Compute the critical path through the task dependency DAG, considering only non-monotonic
edges (monotonic edges don't create sequencing constraints):

```
CRITICAL_PATH(CouplingIR) -> List<TaskId>:
  // Build the dependency DAG from non-monotonic edges only
  dag = CouplingIR.edges
    .filter(|e| !e.monotonic)
    .to_dag()

  // Longest path through the DAG (= minimum completion time)
  return longest_path(dag)
```

Tasks on the critical path determine the minimum completion time. The topology should
prioritize fast communication along critical-path edges.

#### 2.3.3 Pass 3: Spectral Partitioning

Partition tasks into clusters using the same spectral method as document 06, but now
operating on **exact structural coupling** instead of heuristic estimates:

```
PARTITION(CouplingIR, n_agents) -> Map<ClusterId, Set<TaskId>>:
  // Build Laplacian from coupling weights
  L = laplacian(CouplingIR.edges)

  // Number of clusters = min(n_agents, spectral_gap_clusters(L))
  k = min(n_agents, count_near_zero_eigenvalues(L))

  // Partition using Fiedler vector (k=2) or k-way spectral (k>2)
  return spectral_partition(L, k)
```

The improvement over document 06: partitioning on structural coupling means clusters
correspond to **semantically coherent specification namespaces**, not just file proximity.
An agent assigned to a cluster gets a coherent area of the spec to work on.

#### 2.3.4 Pass 4: Cluster Merging (Target Architecture Fit)

If there are more clusters than agents, merge clusters to fit:

```
MERGE_CLUSTERS(clusters, n_agents) -> Map<ClusterId, Set<TaskId>>:
  while |clusters| > n_agents:
    // Find the two clusters with highest inter-cluster coupling
    (c1, c2) = argmax over pairs: inter_coupling(c1, c2)
    // Merge them
    clusters = clusters.merge(c1, c2)
  return clusters
```

This is analogous to **register allocation** in a compiler: more clusters than agents is
like more live variables than registers. The compiler must spill (merge clusters) while
minimizing the cost (inter-cluster coupling after merge).

#### 2.3.5 Pass 5: Communication Scheduling

For each pair of clusters, determine the optimal merge frequency based on cross-cluster
coupling and CALM classification:

```
SCHEDULE(clusters, CouplingIR) -> Map<(ClusterId, ClusterId), MergeFrequency>:
  for each pair (c1, c2):
    cross_coupling = sum of CouplingIR.edges between c1 and c2
    has_nonmonotonic = any edge between c1 and c2 is NON-MONOTONIC

    if has_nonmonotonic:
      // Non-monotonic cross-cluster dependency → high frequency + barriers
      frequency = :high
      barrier = true
    else if cross_coupling > 0.5:
      frequency = :high
      barrier = false
    else if cross_coupling > 0.2:
      frequency = :medium
      barrier = false
    else if cross_coupling > 0:
      frequency = :low
      barrier = false
    else:
      frequency = :none    // No coupling → no communication needed
      barrier = false
```

This is analogous to **instruction scheduling** in a compiler: arrange operations to
maximize pipeline utilization (parallel work) while respecting data dependencies
(non-monotonic edges).

#### 2.3.6 Pass 6: Barrier Placement

Place synchronization barriers at the minimum necessary points:

```
PLACE_BARRIERS(schedule, CouplingIR) -> Set<BarrierPoint>:
  barriers = {}
  for each non-monotonic edge (t1, t2) crossing cluster boundaries:
    // t2 cannot begin until t1's output is merged
    barriers.add(BarrierPoint {
      after: t1,
      before: t2,
      reason: CouplingIR.annotation(t1, t2).shared_elements
    })
  return barriers
```

This is analogous to **memory fence placement** in a compiler: minimize synchronization
points while ensuring correctness for non-monotonic operations.

---

### 2.4 Back-End: Topology Emission

The back-end takes the optimized execution plan and emits a complete topology
T = (G, Phi, Sigma, Pi):

#### 2.4.1 Agent Graph G

```
For each cluster c with assigned agent alpha:
  emit [agent:alpha :agent/cluster cluster:c tx:compile assert]
  emit [agent:alpha :agent/status :active tx:compile assert]

For each pair of clusters (c1, c2) with frequency > :none:
  emit [channel:c1-c2 :channel/from agent:alpha1 tx:compile assert]
  emit [channel:c1-c2 :channel/to agent:alpha2 tx:compile assert]
  emit [channel:c1-c2 :channel/frequency schedule(c1,c2) tx:compile assert]
  if schedule(c1,c2).barrier:
    emit [channel:c1-c2 :channel/barrier true tx:compile assert]
```

#### 2.4.2 Merge Policy Phi

```
For each channel:
  frequency from communication schedule (Pass 5)
  barrier placement from Pass 6
```

#### 2.4.3 Assignment Policy Pi

```
For each task t in cluster c:
  assigned_agent = agent assigned to cluster c
  emit [task:t :task/assigned-to agent:alpha tx:compile assert]

Critical path tasks get priority scheduling:
  if t in critical_path:
    emit [task:t :task/priority :critical tx:compile assert]
```

#### 2.4.4 Scaling Policy Sigma

```
Derived from cluster structure:
  if any cluster has more tasks than capacity threshold:
    emit scaling recommendation (add agent to cluster)
  if any cluster has fewer tasks than minimum threshold:
    emit scaling recommendation (merge cluster with neighbor)
```

#### 2.4.5 Compilation Metadata

The compiled topology carries metadata that enables PGO:

```
[compilation:c1 :compilation/timestamp now tx:compile assert]
[compilation:c1 :compilation/spec-hash (hash of spec dependency graph) tx:compile assert]
[compilation:c1 :compilation/predicted-coupling coupling-matrix tx:compile assert]
[compilation:c1 :compilation/predicted-f-t estimated-fitness tx:compile assert]
[compilation:c1 :compilation/passes ["calm","critical-path","spectral","merge","schedule","barriers"] tx:compile assert]
```

---

### 2.5 PGO: Profile-Guided Optimization

After execution under the compiled topology, the system harvests outcomes and compares
predictions to observations. The delta between predicted and observed coupling is the
**compiler's prediction error** — the signal that drives compiler improvement.

#### 2.5.1 Prediction vs Observation Comparison

```
For each pair of agents (alpha, beta):
  predicted_coupling = compilation.predicted_coupling[alpha][beta]
  observed_coupling = actual datom flow between alpha and beta during execution

  prediction_error = |predicted_coupling - observed_coupling|

  emit [pgo:p1 :pgo/agent-pair (alpha, beta) tx:harvest assert]
  emit [pgo:p1 :pgo/predicted predicted_coupling tx:harvest assert]
  emit [pgo:p1 :pgo/observed observed_coupling tx:harvest assert]
  emit [pgo:p1 :pgo/error prediction_error tx:harvest assert]
```

#### 2.5.2 Error Analysis

The prediction errors cluster into diagnostic categories:

| Error Pattern | Diagnosis | Compiler Improvement |
|---------------|-----------|---------------------|
| Predicted high, observed low | Spec elements look coupled but aren't in practice | Reduce weight of structural coupling for this dependency type |
| Predicted low, observed high | Runtime coupling not visible in spec graph | Add heuristic mechanism A/D/E as supplementary signal |
| Predicted correctly | Structural coupling is accurate | Increase confidence in spec-derived coupling |
| Uniformly high error | Spec bindings incomplete or stale | Flag spec coverage gap; fall back to reactive mode |

#### 2.5.3 Compiler Learning Loop

The PGO data feeds back into the front-end's coupling weight computation:

```
CouplingWeight.combined =
  alpha * structural_coupling +         // From spec graph
  beta * historical_coupling +          // From PGO
  (1 - alpha - beta) * heuristic_coupling  // From mechanisms A/D/E

Where:
  alpha starts at 0.8 (high trust in spec structure)
  beta starts at 0.0 (no PGO data yet)
  After N sessions: alpha and beta adjust based on prediction accuracy
```

Over time:
- If structural coupling predicts well → alpha stays high
- If structural coupling predicts poorly → alpha decreases, beta/heuristic increase
- If PGO data accumulates → beta increases

This is exactly analogous to how GCC's PGO works: the compiler starts with static analysis
(structural coupling), collects runtime profiles (observed coupling), and uses the profiles
to improve subsequent compilations.

---

## 3. The Compilation/Interpretation Spectrum

Not all work is equally compilable. The compilation paradigm exists on a spectrum with
pure interpretation (reactive adjustment) at one end and full ahead-of-time compilation
at the other.

### 3.1 Compilability Conditions

Work is **compilable** when:
1. Tasks have explicit spec bindings (:task/touches relationships)
2. Spec elements have typed dependencies (:inv/depends-on, :adr/affects, etc.)
3. The spec dependency graph is reasonably complete (no major gaps)

Work is **uncompilable** when:
1. Tasks are ad-hoc (no spec bindings — e.g., "fix this bug")
2. Spec is incomplete or uncertain (high UNC-* markers)
3. Novel work with no structural precedent

### 3.2 The Hybrid: JIT + AOT

The optimal strategy is hybrid, like modern language runtimes (Java HotSpot, V8):

```
For each task batch:
  compilable_tasks = tasks with spec bindings and complete dependency info
  uncompilable_tasks = tasks without sufficient structural information

  // AOT: compile what we can predict
  compiled_topology = COMPILE(compilable_tasks)

  // JIT: interpret what we can't
  reactive_topology = COLD_START(uncompilable_tasks)  // From document 06

  // Merge: hybrid topology covering both
  final_topology = MERGE_TOPOLOGIES(compiled_topology, reactive_topology)
```

The merge is safe because both topologies are elements of the topology lattice
(document 01, section 5) and the join operation preserves coordination safety.

### 3.3 Staged Introduction

| Stage | Compilation Level | Mechanism |
|-------|-------------------|-----------|
| Stage 0 (pre-spec) | Pure interpretation | Cold-start algorithm (doc 06) only |
| Stage 0a (spec exists, not yet datoms) | Static analysis only | Parse spec files, extract dependencies heuristically |
| Stage 0b (spec as datoms) | Basic AOT | Front-end operates on datom queries; full pipeline |
| Stage 3 (multi-agent with history) | AOT + PGO | Structural coupling refined by historical observations |
| Stage 4 (advanced) | Full JIT + AOT hybrid | Adaptive compilation with mid-session recompilation |

The critical transition is **Stage 0a → Stage 0b**: once the specification elements are
datoms in the store, the front-end's Datalog queries become exact. Before that, the
front-end must parse markdown files and extract dependencies heuristically — still better
than no compilation, but less precise.

---

## 4. Formal Properties

### 4.1 Compilation Correctness

**Statement:** A compiled topology never under-coordinates relative to what the spec structure
requires.

**Formal:** For any compiled topology T_compiled derived from spec graph G_spec:
  For all pairs of tasks (t1, t2) with a non-monotonic dependency path in G_spec:
    exists a channel in T_compiled connecting the agents assigned to t1 and t2
    with frequency >= :medium and barrier = true.

**Rationale:** The front-end extracts ALL dependencies from the spec graph. Pass 1 (CALM
classification) identifies all non-monotonic edges. Pass 5 (communication scheduling) assigns
high frequency to non-monotonic cross-cluster edges. Pass 6 (barrier placement) adds barriers.
No non-monotonic dependency can be missed because the extraction is exhaustive over the
datom store.

**Falsification:** A non-monotonic dependency between tasks t1 and t2 exists in the spec graph,
but the compiled topology has no barrier-protected channel between their assigned agents.

**Verification:** V:PROP — Generate random spec graphs with known non-monotonic edges. Compile.
Assert every non-monotonic edge has a corresponding barrier-protected channel.

### 4.2 Compilation Dominance

**Statement:** Compiled topology produces weakly better F(T) than cold-start topology for
any spec-bearing project, from the first session.

**Formal:** For any spec graph G_spec and task set T:
  F(T_compiled(G_spec, T)) >= F(T_coldstart(T)) - epsilon

Where epsilon = 0.05 (noise margin for F(T) measurement).

**Argument:** The cold-start algorithm (document 06) starts from mesh (maximum coordination)
and relaxes. The compiled topology starts from the structurally optimal point. Since:
1. The compiled topology has exact coupling information (from spec graph)
2. The cold-start has no coupling information (heuristic at best)
3. Exact information always produces equal or better optimization than no information

The compiled topology weakly dominates cold-start.

**Caveat:** This assumes the spec graph accurately represents actual coupling. If the spec
graph is severely wrong (stale, incomplete, or structurally misleading), compilation may
produce worse results than cold-start. This is why the PGO loop measures prediction error
and adjusts confidence.

**Falsification:** A spec-bearing project where cold-start consistently outperforms compilation
(F(T_coldstart) > F(T_compiled) + epsilon for 3+ sessions).

**Verification:** V:PROP — Compare F(T) under compiled vs cold-start topologies across
simulated sessions with varying spec graph accuracy.

### 4.3 PGO Convergence

**Statement:** With PGO data from N sessions, the compiler's prediction error decreases
monotonically (within noise margin).

**Formal:** Let E(n) = mean prediction error at session n.
  For all n > warm-up period (3 sessions):
    E(n+1) <= E(n) + epsilon (epsilon = 0.02)

**Argument:** PGO adjusts coupling weights (alpha, beta) based on observed prediction accuracy.
Each session provides new data points. The weight adjustment is gradient descent on prediction
error. Under standard SGD convergence conditions (bounded gradients, decreasing learning rate),
the error decreases monotonically.

**Falsification:** Prediction error increases for 3 consecutive sessions after the warm-up period.

**Verification:** V:PROP — Track prediction error across simulated sessions. Assert monotonic
decrease after warm-up.

### 4.4 Hybrid Safety

**Statement:** The hybrid topology (merged AOT + JIT) preserves all safety invariants from
both the compilation and reactive frameworks.

**Formal:** For any hybrid topology T_hybrid = MERGE(T_compiled, T_reactive):
  INV-TOPO-TRANS-001 through 004 hold for T_hybrid
  INV-TOPO-COLD-001 and 002 hold for the reactive component
  Compilation Correctness (4.1) holds for the compiled component

**Argument:** MERGE_TOPOLOGIES uses the lattice join operation (document 01, section 5).
The join of two topologies that individually satisfy safety invariants also satisfies them,
because:
1. Channel sets only grow under join (monotonicity, INV-TOPO-TRANS-001)
2. Connectivity is preserved under join (adding channels can't disconnect, INV-TOPO-TRANS-003)
3. Grace periods take the maximum of both components (safety preserved, INV-TOPO-TRANS-002)

**Falsification:** A safety invariant is violated in the hybrid topology that was satisfied
in both component topologies.

**Verification:** V:PROP — Generate random compiled and reactive topologies, merge, assert
all safety invariants hold.

---

## 5. The Deeper Implications

### 5.1 Task Decomposition Is Topology Selection

In the reactive paradigm, task decomposition and topology selection are separate problems:
1. Decompose work into tasks (human or agent decision)
2. Select topology for the given tasks (topology framework)

In the compilation paradigm, they're the **same problem**. The spec dependency graph
determines both the natural task boundaries AND the natural cluster boundaries. The compiler
simultaneously decides:
- How to decompose the spec graph into task-sized chunks
- How to assign those chunks to agents
- How to coordinate between agents

This means the compiler can do something the reactive framework can't: **restructure
the work to fit the available agents**. If you have 3 agents but the spec graph has
5 natural clusters, the compiler doesn't just merge clusters — it can suggest a different
task decomposition that produces 3 clusters with lower cross-cluster coupling.

```
DECOMPOSE_AND_COMPILE(spec_graph, n_agents) -> (TaskDecomposition, Topology):
  // Try multiple decomposition strategies
  candidates = [
    decompose_by_namespace(spec_graph),
    decompose_by_dependency_depth(spec_graph),
    decompose_by_monotonicity_boundary(spec_graph),
    decompose_by_min_cut(spec_graph, n_agents),
  ]

  // For each, compile a topology and estimate F(T)
  for each candidate in candidates:
    topology = COMPILE(candidate, n_agents)
    estimated_f_t = ESTIMATE_FITNESS(topology, candidate)

  // Return the (decomposition, topology) pair with best estimated F(T)
  return argmax(candidates, by=estimated_f_t)
```

### 5.2 The Compiler Knows What The Agent Should Think About

Because the compiler understands the spec dependency graph, it can generate not just
topology but **cognitive context** for each agent. The seed assembly (spec/05-harvest.md)
must project relevant knowledge into each agent's context window. The compiler knows
exactly which spec elements each agent needs:

```
For agent alpha assigned to cluster c:
  primary_context = spec elements touched by tasks in c
  coupling_context = spec elements in adjacent clusters (cross-cluster dependencies)
  boundary_context = interfaces between clusters (merge points)

  seed(alpha) = project(primary_context + coupling_context + boundary_context)
```

This is **link-time optimization (LTO)** for cognition: the compiler sees across cluster
boundaries and can include cross-cluster context that the agent wouldn't know to ask for.

### 5.3 The Specification Tells You When To Recompile

In traditional compilation, the programmer decides when to recompile. In Braid, the system
knows when recompilation is needed because the **spec is versioned in the store**:

```
SHOULD_RECOMPILE(current_compilation, current_spec_state) -> bool:
  spec_hash_at_compile = current_compilation.spec_hash
  spec_hash_now = hash(current_spec_state)

  if spec_hash_at_compile != spec_hash_now:
    // Spec has changed since last compilation
    changed_elements = diff(spec_at_compile, spec_now)

    // Check if changes affect the coupling structure
    coupling_affected = any element in changed_elements
      is referenced by CouplingIR.annotations

    return coupling_affected  // Only recompile if coupling structure changed
  else:
    return false
```

Recompilation is triggered by **structural changes to the spec**, not by time or by
topology drift. This is analogous to incremental compilation: only recompile the modules
whose source code changed.

### 5.4 Compilation As Verification

The compilation process is itself a **verification step** for the specification. If the
compiler cannot produce a valid topology, it means the spec has structural problems:

| Compilation Failure | Spec Diagnosis |
|---------------------|----------------|
| Cyclic non-monotonic dependencies | Spec has contradictory invariants that can't be ordered |
| Disconnected spec graph | Spec is missing dependency declarations between related elements |
| Extreme imbalance in cluster sizes | Spec namespaces are poorly decomposed |
| No valid partition for available agent count | Spec coupling is too tight for the available parallelism |
| High predicted conflict rate | Spec has too many LWW/override resolution modes in adjacent elements |

This means the compiler serves as an early warning system: specification problems are
detected at compile time (before agents start working), not at runtime (after conflicts
have already happened).

This is the deepest connection to DDIS's core mission: **maintaining verifiable coherence**.
The compiler verifies that the specification is coherent enough to support parallel
execution. If it isn't, the spec needs work before implementation begins.

---

## 6. Worked Example: Compiling Braid Stage 0

### 6.1 Spec Input

The Braid specification (spec/) has 14 namespace files with typed dependencies. For Stage 0,
the relevant spec elements include:

```
Namespace: STORE    -> INV-STORE-001..009, ADR-STORE-001..009
Namespace: QUERY    -> INV-QUERY-001..005, ADR-QUERY-001..003
Namespace: HARVEST  -> INV-HARVEST-001..004, ADR-HARVEST-001..002
Namespace: SEED     -> INV-SEED-001..003, ADR-SEED-001..003
Namespace: SCHEMA   -> INV-SCHEMA-001..005, ADR-SCHEMA-001..004
```

Dependency structure (simplified):
```
STORE <-- QUERY (query depends on store)
STORE <-- SCHEMA (schema depends on store)
STORE <-- HARVEST (harvest depends on store)
QUERY <-- HARVEST (harvest uses queries to extract)
QUERY <-- SEED (seed uses queries to project)
HARVEST <-- SEED (seed depends on harvest output)
```

### 6.2 Front-End Output

Coupling IR with 5 clusters (one per namespace), structural coupling:

```
STORE-QUERY:    0.85 (QUERY depends heavily on STORE internals)
STORE-SCHEMA:   0.70 (SCHEMA defines what STORE accepts)
STORE-HARVEST:  0.45 (HARVEST reads from STORE)
QUERY-HARVEST:  0.55 (HARVEST uses QUERY engine)
QUERY-SEED:     0.60 (SEED uses QUERY for projection)
HARVEST-SEED:   0.50 (SEED reads HARVEST output)
SCHEMA-QUERY:   0.30 (QUERY validates against SCHEMA)
SCHEMA-HARVEST: 0.10 (weak — HARVEST mostly schema-independent)
SCHEMA-SEED:    0.10 (weak — SEED mostly schema-independent)
```

CALM classification:
- STORE-QUERY: monotonic (queries don't mutate)
- STORE-SCHEMA: NON-MONOTONIC (schema changes affect store validation)
- STORE-HARVEST: monotonic (harvest only reads + appends)
- All others: monotonic

### 6.3 Middle-End Optimization (3 Agents Available)

**Pass 1 (CALM):** One non-monotonic edge: STORE-SCHEMA. Barrier required.

**Pass 2 (Critical Path):** STORE → SCHEMA (barrier) → QUERY → HARVEST → SEED.
Minimum completion time determined by the STORE-SCHEMA barrier.

**Pass 3 (Spectral Partition):** 5 namespaces, 3 agents → need to merge.
Natural clusters from spectral analysis:
- Cluster A: {STORE, SCHEMA} — coupling 0.70, non-monotonic dependency
- Cluster B: {QUERY} — high coupling to both A and C
- Cluster C: {HARVEST, SEED} — coupling 0.50

**Pass 4 (Cluster Merge):** Already 3 clusters for 3 agents. No merge needed.

**Pass 5 (Communication Schedule):**
```
A-B: coupling 0.85+0.30 = high → frequency :high, no barrier (monotonic)
B-C: coupling 0.55+0.60 = high → frequency :high, no barrier (monotonic)
A-C: coupling 0.45+0.10+0.10 = medium → frequency :medium, no barrier (monotonic)
```

**Pass 6 (Barrier Placement):**
```
Within Cluster A: barrier between SCHEMA changes and STORE validation
(This is intra-cluster, handled by the agent assigned to cluster A)
```

### 6.4 Back-End Output

```
Compiled Topology:
  Agent alpha → Cluster A (STORE + SCHEMA)
    - Intra-cluster: barrier between schema transactions and store validation
    - Primary context: INV-STORE-*, INV-SCHEMA-*, ADR-STORE-*, ADR-SCHEMA-*

  Agent beta → Cluster B (QUERY)
    - High-frequency merge with alpha (STORE dependencies)
    - High-frequency merge with gamma (HARVEST/SEED dependencies)
    - Primary context: INV-QUERY-*, ADR-QUERY-*
    - Coupling context: INV-STORE-001..004, INV-HARVEST-001, INV-SEED-001

  Agent gamma → Cluster C (HARVEST + SEED)
    - High-frequency merge with beta (QUERY dependencies)
    - Medium-frequency merge with alpha (STORE read access)
    - Primary context: INV-HARVEST-*, INV-SEED-*, ADR-HARVEST-*, ADR-SEED-*
    - Coupling context: INV-QUERY-001..003, INV-STORE-001

Predicted F(T): 0.86
  D1 (throughput): estimated 0.90 (good parallelism, critical path is short)
  D2 (conflicts): estimated 0.02 (only STORE-SCHEMA is non-monotonic, handled by barrier)
  D3 (staleness): estimated 0.05 (high-frequency merges on coupled channels)
  D4 (overhead): estimated 0.10 (medium — 3 high-freq + 1 medium-freq channel)
  D5 (balance): estimated 0.82 (Cluster A slightly larger)
  D6 (blocking): estimated 0.03 (one intra-cluster barrier)
  D7 (knowledge loss): estimated 0.02 (good coupling context in seeds)
```

### 6.5 Post-Session PGO

After executing under the compiled topology:

```
Observed F(T): 0.84

Prediction errors:
  alpha-beta coupling: predicted 0.85, observed 0.78 (beta needed STORE less than expected)
  beta-gamma coupling: predicted 0.60, observed 0.72 (gamma needed QUERY more than expected)
  alpha-gamma coupling: predicted 0.45, observed 0.42 (accurate)

PGO adjustment for next compilation:
  Decrease STORE-QUERY structural weight by 0.07 (spec says dependent but practice shows loose)
  Increase QUERY-SEED structural weight by 0.12 (seed projection queries are more complex than spec suggests)
  Maintain STORE-HARVEST structural weight (accurate prediction)

Next compilation:
  alpha-beta channel: reduce from :high to :high (still above threshold, no change)
  beta-gamma channel: already :high (increase confirmed, no change needed)
  alpha-gamma channel: maintain :medium (accurate)

  Prediction accuracy: 93% (good — structural coupling is largely correct for this project)
```

---

## 7. Relationship to Documents 00–10

### 7.1 What Changes

| Document | Impact of Compilation Paradigm |
|----------|-------------------------------|
| 00 (Thesis) | **Extended.** Topology emerges from datom store AND from spec structure. Two sources: structural (compile-time) + observational (runtime) |
| 01 (Algebraic Foundations) | **Unchanged.** All algebraic properties still hold. Compilation is an optimization, not a structural change |
| 02 (Coupling Model) | **Reframed.** Five mechanisms become fallback/PGO signals. Structural coupling from spec graph becomes primary |
| 03 (Topology Definition) | **Unchanged.** T=(G,Phi,Sigma,Pi) is still the output. Compilation is a new way to compute it |
| 04 (Transition Protocol) | **Unchanged.** Compiled topologies still transition using CALM-stratified protocol |
| 05 (Scaling Authority) | **Extended.** Compiler can recommend scaling decisions based on structural analysis |
| 06 (Cold-Start) | **Superseded for spec-bearing projects.** Cold-start becomes fallback for uncompilable work |
| 07 (Fitness Function) | **Extended.** F(T) now has two components: predicted (from compiler) and observed (from execution) |
| 08 (Open Questions) | **Partially resolved.** Signal system (Q1) and observability (Q4) enriched by compilation metadata |
| 09 (Invariants) | **Extended.** Three new invariants (Compilation Correctness, Dominance, PGO Convergence) |
| 10 (Design Decisions) | **Extended.** One new ADR (TD-COMPILE-001) |

### 7.2 What Doesn't Change

The compilation paradigm is **additive**, not disruptive. Everything in documents 00–10
remains valid and necessary:

- The reactive framework handles uncompilable work
- The algebraic foundations underpin both paradigms
- The transition protocol governs topology changes regardless of how they're computed
- The fitness function measures outcomes regardless of how the topology was derived
- The scaling authority governs decisions regardless of whether they come from compilation or observation

Compilation is a new LAYER on top of the existing framework, not a replacement.

---

## 8. New Design Decision

### TD-COMPILE-001: Specification-Predictive Compilation of Coordination Topology

**Problem:** How should the optimal coordination topology be determined — by observing
execution outcomes (reactive) or by analyzing specification structure (predictive)?

**Options:**
1. **Pure reactive.** Observe outcomes, compute coupling, adjust topology.
   - Pro: Works with any task type, no spec required
   - Con: Cold-start penalty, discovery waste, convergence lag
2. **Pure compilation.** Derive topology entirely from spec structure.
   - Pro: Optimal from session 1, no cold-start
   - Con: Requires complete spec bindings; fails on ad-hoc work
3. **Hybrid AOT + JIT.** Compile what can be predicted from spec structure;
   interpret (reactive adjust) what can't. PGO refines structural predictions.
   - Pro: Best of both worlds; degrades gracefully; improves over time
   - Con: More complex; two paradigms to maintain

**Decision:** Option 3 — Hybrid compilation with PGO.

**Rationale:** The compilation paradigm is uniquely enabled by Braid's self-bootstrap
property (C7). No other multi-agent framework has queryable specifications in the same
store as coordination state. Failing to exploit this structural advantage would leave
performance on the table. The hybrid approach ensures graceful degradation for uncompilable
work while providing optimal coordination for spec-bearing work from the first session.

**Consequences:**
- Compilation pipeline added to topology framework (front-end, middle-end, back-end)
- Five-mechanism coupling model (document 02) becomes PGO/fallback signal
- Cold-start algorithm (document 06) becomes fallback for uncompilable work
- PGO loop added to bilateral feedback mechanism
- Compilation metadata stored as datoms for auditability and learning
- Spec graph accuracy becomes measurable (prediction error metric)
- Compiler serves as spec verification tool (structural problems detected pre-execution)

---

## 9. New Invariants

### INV-TOPO-COMPILE-001: Compilation Correctness

**Traces to:** CALM theorem; spec/04-resolution.md (resolution modes)
**Type:** Safety invariant
**Statement:** A compiled topology never under-coordinates relative to what the spec
structure requires. Every non-monotonic dependency in the spec graph has a corresponding
barrier-protected channel in the compiled topology.
**Formal:** For all non-monotonic dependency paths (t1 →* t2) in the spec graph:
  exists channel C in T_compiled connecting agents of t1 and t2
  with C.barrier = true and C.frequency >= :medium.
**Falsification:** A non-monotonic spec dependency exists without a barrier-protected channel
in the compiled topology.
**Verification:** V:PROP — Generate random spec graphs with known non-monotonic edges.
Compile. Assert every non-monotonic edge has barrier-protected channel.

### INV-TOPO-COMPILE-002: Compilation Dominance (Conditional)

**Traces to:** Information theory (more information → better optimization)
**Type:** Progress invariant (conditional)
**Statement:** For spec-bearing projects with accurate spec bindings, compiled topology
produces weakly better F(T) than cold-start from session 1.
**Formal:** Given spec accuracy > 0.7 (measured by PGO prediction error < 0.3):
  F(T_compiled) >= F(T_coldstart) - 0.05
**Falsification:** A spec-bearing project with accuracy > 0.7 where cold-start consistently
outperforms compilation (3+ sessions).
**Verification:** V:PROP — Compare F(T) under both paradigms across simulated projects with
varying spec accuracy.

### INV-TOPO-COMPILE-003: PGO Convergence

**Traces to:** spec/10-bilateral.md (bilateral convergence); SGD convergence theory
**Type:** Progress invariant
**Statement:** Compilation prediction error decreases monotonically (within noise margin)
after warm-up period with PGO feedback.
**Formal:** Let E(n) = mean prediction error at session n.
  For n > 3 (warm-up): E(n+1) <= E(n) + 0.02
**Falsification:** Prediction error increases for 3 consecutive sessions after warm-up.
**Verification:** V:PROP — Track prediction error across simulated sessions with PGO
feedback. Assert monotonic decrease after warm-up.

---

## 10. The Compiler Analogy — Complete Mapping

| Compiler Concept | Topology Concept | Why It Matters |
|---|---|---|
| Source language | Specification (datoms) | Spec is queryable, typed, versioned |
| Lexing / parsing | Spec dependency extraction | Datom queries, not text parsing |
| AST / IR | Coupling IR | Annotated graph with CALM classification |
| Type checking | CALM classification | Monotonic vs non-monotonic edge typing |
| Dependency analysis | Coupling weight computation | Structural + historical (PGO) |
| Dead code elimination | Irrelevant spec filtering | Only compile spec elements touched by current tasks |
| Loop optimization | Merge frequency tuning | Minimize overhead while maintaining coupling coverage |
| Register allocation | Cluster-to-agent assignment | More clusters than agents → merge (spill) |
| Instruction scheduling | Communication scheduling | Maximize parallelism, respect data dependencies |
| Memory fence placement | Barrier placement | Minimal barriers for non-monotonic edges |
| Link-time optimization | Cross-cluster context injection | Seed assembly uses compiler's cross-boundary knowledge |
| Target architecture | Agent capabilities + count | Compilation adapts to available resources |
| Object code | Compiled topology T=(G,Phi,Sigma,Pi) | Complete executable coordination plan |
| Runtime | Multi-agent execution | Agents work under compiled topology |
| Profiling | F(T) measurement + outcome harvesting | Compare predicted to observed performance |
| PGO | Bilateral feedback on coupling weights | Structural predictions refined by runtime observations |
| Incremental compilation | Spec-change-triggered recompilation | Only recompile when coupling structure changes |
| JIT compilation | Mid-session reactive adjustment | Handle unpredicted coupling at runtime |
| Compiler warnings | Spec structural diagnostics | Detect spec problems before agents start working |
| Compilation error | Uncompilable spec structure | Cyclic non-monotonic deps, disconnected graph |

---

## 11. Traceability

| Concept | Traces to |
|---------|-----------|
| Spec as source language | SEED.md §4 (specification formalism); C7 (self-bootstrap) |
| Coupling IR from spec graph | spec/01-store.md (datom relationships); spec/11-schema.md (typed attributes) |
| CALM classification pass | spec/03-query.md (CALM compliance); 04-transition-protocol.md |
| Spectral partitioning | 06-cold-start.md (same algorithm, better input data) |
| PGO via bilateral feedback | spec/10-bilateral.md (convergence loop); 07-fitness-function.md |
| Compilation as verification | spec/16-verification.md (verification pipeline) |
| Hybrid AOT + JIT | 06-cold-start.md (JIT fallback); this document (AOT primary) |
| Task decomposition duality | 03-topology-definition.md (assignment policy Pi) |
| Cognitive context from compiler | spec/06-seed.md (seed assembly); spec/13-budget.md (attention budget) |
| Spec-change-triggered recompile | spec/09-signal.md (divergence signals) |

---

## 12. Summary

The topology-as-compilation paradigm is the key insight that differentiates Braid's
coordination framework from every other multi-agent system. It is enabled by one unique
property: **the specification is queryable data in the same store as the coordination
state** (constraint C7, self-bootstrap).

The compilation pipeline — front-end (spec graph → coupling IR), middle-end (optimization
passes), back-end (topology emission) — produces a structurally optimal coordination plan
before any agent begins working. The PGO loop (harvest outcomes → prediction error →
weight adjustment) refines structural predictions with observed reality, converging toward
a compiler that accurately predicts coordination requirements from specification structure.

The reactive framework from documents 00–10 remains fully intact as the JIT layer:
handling uncompilable work, adjusting at runtime when predictions are wrong, and providing
the PGO data that improves compilation. The two paradigms compose cleanly because they
share the same algebraic foundation (lattice of topologies, CALM stratification, fitness
function).

The deepest implication: **the compiler serves as a specification verifier**. If the spec
can't be compiled into a valid topology, the spec has structural problems that would cause
coordination failures at runtime. This means coordination failures are caught at compile
time, not at runtime — precisely the shift from "fight problems as they occur" to
"prevent problems structurally" that DDIS was designed to achieve.

---

*This document extends the execution-topologies exploration with the compilation paradigm.
It should be read after documents 00–10, as it builds on all of them while changing none.
The compilation paradigm is the capstone that makes the topology framework qualitatively
different from existing multi-agent coordination approaches.*
