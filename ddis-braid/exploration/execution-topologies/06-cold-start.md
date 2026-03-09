# 06 — Cold-Start Bootstrap

> **Summary:** The cold-start problem — selecting a topology with no prior coordination
> history — is solved by monotonic relaxation from maximum coordination (mesh). Start
> with the safest topology, measure outcomes, relax coordination intensity where evidence
> shows it's unnecessary. This is a minimax strategy that minimizes maximum possible loss.

---

## 1. The Exploration-Exploitation Tradeoff

Two possible strategies for cold-start:

**Strategy A (Explore-first):** Start with a random topology, measure outcomes, converge
toward optimal. Problem: the exploration phase may produce bad coordination, damaging
outcomes and wasting work.

**Strategy B (Conservative-first):** Start with the maximum-coordination topology (mesh),
measure coupling, relax coordination where the data shows it's unnecessary. Never
under-coordinates, only over-coordinates.

---

## 2. Theorem: Monotonic Relaxation Dominance

**Statement:** For any task distribution with unknown coupling, starting from mesh and
relaxing to the coupling-optimal topology produces weakly better expected outcomes than
any explore-first strategy.

**Argument:**

Over-coordination (mesh when star suffices) costs only merge overhead — O(n^2) merges
instead of O(n). This cost is:
- Bounded (known upper bound from merge operation cost)
- Predictable (proportional to agent count squared)
- Non-destructive (no data loss, no conflicts, no rework)

Under-coordination (star when mesh is needed) costs data staleness and merge conflicts,
which can cause rework and knowledge loss. This cost is:
- Unbounded (cascading failures from stale state)
- Unpredictable (depends on the specific coupling pattern)
- Potentially destructive (conflicts cause rework, stale state causes wrong decisions)

Since the cost of over-coordination is bounded and predictable, while the cost of
under-coordination is unbounded and unpredictable, the conservative strategy has
lower maximum regret.

This is a **minimax** argument: minimize the maximum possible loss. It is the appropriate
framework because we have no prior data to estimate expected loss.

**Corollary:** The cold-start topology is a conservative upper bound on coordination
intensity. The system can only RELAX from this point, never TIGHTEN beyond it (unless
evidence of under-coordination appears — conflicts, quality drops). This is the
monotonic relaxation principle.

---

## 3. The COLD_START Algorithm

### 3.1 Algorithm (Pseudocode)

```
COLD_START(agents: Set<Agent>, tasks: Set<Task>) -> Topology:

  n <- |agents|

  -- Phase 1: Compute initial coupling from available signals --

  C <- n x n zero matrix                    // coupling matrix
  for each pair (Ti, Tj) in tasks:
    C[assigned(Ti)][assigned(Tj)] += file_coupling(Ti, Tj)

  // If spec elements are datoms (Stage 0b+), add invariant coupling
  if store.has_schema(:inv/depends-on):
    for each pair (Ti, Tj):
      C[assigned(Ti)][assigned(Tj)] += 0.6 * invariant_coupling(Ti, Tj)

  -- Phase 2: Select topology based on agent count + coupling --

  if n <= 3:
    return Mesh(agents)
    // Rationale: mesh overhead for <= 3 is O(6) merges — trivial.
    // No topology can outperform mesh at this scale.

  // Partition tasks into coupling clusters
  clusters <- spectral_partition(C, threshold=0.3)

  if |clusters| = 1:
    // All tasks are coupled — single cluster
    if n <= 5:
      return Mesh(agents)
      // Rationale: mesh overhead for <= 5 is O(20) — still manageable.
    else:
      hub <- agent with max sum(C[hub][*])    // most-coupled agent
      return Star(hub, agents)
      // Rationale: star gives O(n) merges while maintaining
      // diameter 2 (any agent reaches any other in 2 hops).

  else:
    // Multiple coupling clusters — hybrid topology
    sub_topologies <- {}
    for each cluster k in clusters:
      agents_k <- agents assigned to tasks in k
      if |agents_k| <= 3:
        sub_topologies[k] <- Mesh(agents_k)
      else:
        hub_k <- agent with max coupling within k
        sub_topologies[k] <- Star(hub_k, agents_k)

    // Bridge: agent with highest inter-cluster coupling
    bridge <- agent maximizing sum over cross-cluster edges of C[bridge][*]

    return Hybrid(sub_topologies, bridge, inter_frequency=:periodic)

  -- Phase 3: Schedule relaxation check --

  // After first session completes, harvest coordination outcomes
  // and recompute topology with historical coupling signal
  schedule_recompute(after=session_end)
```

### 3.2 Agent Count Thresholds

These thresholds are derived from the Swarm Kernel Architecture research (three regime zones):

| Agent Count | Regime | Default Topology | Rationale |
|-------------|--------|-----------------|-----------|
| 1 | Solo | None (no coordination needed) | Single agent, no merge partners |
| 2-3 | Solo/Team boundary | Mesh | O(6) overhead is trivial; full coordination |
| 4-5 | Small Team | Mesh or Star (coupling-dependent) | Mesh if coupling > 0.3; Star if coupling < 0.3 |
| 6-12 | Team | Hybrid | Clusters of 3-5 in mesh, bridge agents between |
| 13+ | Fleet | Hierarchical or Hybrid with hierarchy | Mesh overhead O(n^2) becomes prohibitive |

### 3.3 Why Mesh Is the Safe Default for Small Groups

For n agents, mesh has O(n^2) merge channels. The overhead per channel is bounded by
the merge operation cost (set union, content-addressed deduplication).

At small n:
- n=2: 2 channels, 2 merges per cycle
- n=3: 6 channels, 6 merges per cycle
- n=5: 20 channels, 20 merges per cycle

For Braid's datom store (in-memory BTreeSet, content-addressed deduplication), each merge
is O(|delta|) where |delta| is the number of new datoms since last merge. For typical
workloads, |delta| ~ 10-50 datoms per minute, and merge is microseconds.

The overhead of 20 merges at microseconds each is negligible compared to the cost of
one merge conflict (which can cause hours of rework).

---

## 4. Convergence Trajectory

### 4.1 Worked Example: 4-Session Convergence

**Session 1: Mesh (cold-start, no history)**
```
Topology: Mesh(alpha, beta, gamma, delta)
Merge frequency: :high (all channels)
Observed:
  conflict_count: 0
  merge_overhead: 340ms total per cycle
  quality: 0.88
  staleness: 0.02 (very fresh)
  balance: 0.85
```
Harvest: "zero conflicts suggests mesh is overkill for some pairs"

**Session 2: Mesh with reduced frequency on low-coupling pairs**
```
Topology: Mesh, but alpha-gamma and beta-delta channels reduced to :medium
Rationale: coupling(alpha, gamma) = 0.15, coupling(beta, delta) = 0.10
Observed:
  conflict_count: 0
  merge_overhead: 180ms (47% reduction)
  quality: 0.90 (improved — less merge interruption)
  staleness: 0.05 (acceptable)
  balance: 0.87
```
Harvest: "still zero conflicts after relaxation; can relax further"

**Session 3: Hybrid (store+query cluster in mesh, interface solo)**
```
Topology: Hybrid({alpha, beta}: mesh, {gamma, delta}: mesh, epsilon: solo)
Rationale: coupling analysis shows two clear clusters
Observed:
  conflict_count: 1 (epsilon needed a fact from gamma's cluster)
  merge_overhead: 95ms (72% reduction from session 1)
  quality: 0.91
  staleness: 0.08
  balance: 0.90
```
Harvest: "one conflict was non-critical; hybrid topology validated"

**Session 4: Same hybrid, tuned merge frequencies**
```
Topology: Same hybrid, inter-cluster frequency adjusted to :medium (was :low)
Rationale: epsilon's conflict in session 3 suggests slightly more inter-cluster communication
Observed:
  conflict_count: 0
  merge_overhead: 72ms
  quality: 0.93
  staleness: 0.05
  balance: 0.92
```
Approaching optimal topology. F(T) improving monotonically.

### 4.2 Convergence Rate

The convergence rate depends on outcome variance:
- **Low-variance projects** (consistent coupling patterns): converge in 3-4 sessions
- **High-variance projects** (coupling shifts per task type): 8-10 sessions
- **Novel projects** (no similar historical data): 5-7 sessions

In all cases, convergence is monotonic: F(T) never decreases by more than noise margin
epsilon between sessions (INV-TOPO-FIT-001).

---

## 5. Spectral Partitioning

### 5.1 Method

The coupling matrix C is treated as an adjacency matrix of a weighted graph. Spectral
partitioning uses the eigenvectors of the graph Laplacian to identify natural clusters:

```
L = D - C            // Graph Laplacian (D = degree matrix, C = coupling matrix)
eigenvalues, eigenvectors = eigen(L)
// Number of clusters = number of eigenvalues near zero
// Cluster assignment from signs of second-smallest eigenvector (Fiedler vector)
```

### 5.2 Threshold Selection

The coupling threshold theta = 0.3 determines which edges are "significant" before
partitioning. Edges with coupling < theta are zeroed in C before computing the Laplacian.

This threshold is a datom:
```
[partitioning:config :partitioning/threshold 0.3 tx:genesis assert]
```

### 5.3 Revisit Conditions

Revisit spectral partitioning if:
- **(a)** Agent counts consistently <= 8: simpler algorithms (connected components on
  thresholded coupling graph, or brute-force enumeration of partitions) may be sufficient
  and more interpretable.
- **(b)** The coupling matrix is very sparse (most pairs have coupling ~ 0): thresholded
  connected components degenerates to the same result but runs faster.
- **(c)** Real-time repartitioning is needed during sessions: spectral decomposition's
  O(n^3) may be too slow (though for n <= 20 this is microseconds).
- **(d)** Interpretability is more important than optimality: spectral partitioning is
  hard to explain to humans; connected components is intuitive.

For now, spectral partitioning is the mathematically principled choice and handles all
regimes correctly.

---

## 6. Invariants

### INV-TOPO-COLD-001: Monotonic Relaxation
**Statement:** The coordination intensity of the topology never increases without
evidence of under-coordination (conflict_rate > 0 or quality_drop > epsilon).

**Falsification:** Merge frequency increases or topology structure tightens when
conflict_rate = 0 and quality is stable or improving.

**Verification:** V:PROP (track coordination_intensity metric across sessions; assert
monotonically non-increasing except after conflict events)

**Rationale:** Monotonic relaxation guarantees safe convergence. The only way to increase
coordination is if the system observes evidence that current coordination is insufficient.

### INV-TOPO-COLD-002: Cold-Start Safety
**Statement:** The cold-start topology provides coordination intensity >= the
coupling-optimal topology for the actual task coupling.

**Falsification:** The cold-start topology produces coordination intensity strictly
less than what the coupling matrix requires.

**Verification:** V:PROP (for any random coupling matrix C, verify
cold_start_topology(C).intensity >= optimal_topology(C).intensity)

**Rationale:** The cold-start topology is an upper bound. It can never under-coordinate
because it starts from maximum coordination (mesh for small n) or the maximum supported
by the coupling structure (hybrid with conservative thresholds for large n).

---

## 7. Traceability

| Concept | Traces to |
|---------|-----------|
| Minimax strategy (minimize max regret) | First principles: asymmetric cost function (conflict >> overhead) |
| Monotonic relaxation | spec/01-store.md L4 (monotonicity of store); analogous principle for topology |
| Agent count thresholds (Solo/Team/Fleet) | Swarm Kernel Architecture (three regime zones) |
| Spectral partitioning | Standard graph theory; used in query/graph.rs |
| Coupling-driven topology selection | INS-005 (task-level regime routing outperforms fixed topology) |
| Session-over-session convergence | SEED.md S7 (self-improvement loop), S5 (harvest/seed) |
| Historical coupling accumulation | 02-coupling-model.md Mechanism E |

---

*Next: `07-fitness-function.md` — how to measure whether the current topology is working.*
