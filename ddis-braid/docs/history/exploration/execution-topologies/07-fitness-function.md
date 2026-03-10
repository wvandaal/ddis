# 07 — Topology Fitness Function: F(T)

> **Summary:** F(T) measures coordination effectiveness across seven dimensions: throughput,
> conflict rate, staleness, merge overhead, balance, blocking time, and knowledge loss. It
> drives the bilateral convergence loop for topology and composes with F(S) (specification
> fitness) into a total system fitness F_total. The diagnostic mapping from dimension
> degradation to corrective action enables automated topology optimization.

---

## 1. What F(T) Measures

A topology exists to solve one problem: **enable multiple agents to make progress on
interdependent work without interfering with each other.** This decomposes into two
forces in tension:

**Coordination benefit:** Agents who need each other's outputs receive them quickly.
No agent is blocked waiting for facts that exist elsewhere in the system.

**Coordination cost:** Every merge, message, and sync barrier consumes attention budget,
compute, and wall-clock time. Agents spend time coordinating instead of working.

The optimal topology **maximizes benefit while minimizing cost.** This is a rate-distortion
problem (same structure as the seed assembly budget in spec/13-budget.md): maximize
information flow under bounded coordination budget.

---

## 2. The Seven Dimensions

### 2.1 Formula

```
F(T) = w1*D1 + w2*(1-D2) + w3*(1-D3) + w4*(1-D4) + w5*D5 + w6*(1-D6) + w7*(1-D7)
```

For dimensions where 0 is optimal (conflicts, staleness, overhead, blocking, knowledge
loss), we use (1 - Di) so that F(T) -> 1.0 is universally optimal.

### 2.2 Dimension Definitions

| # | Dimension | Symbol | Measures | Range | Optimal |
|---|-----------|--------|----------|-------|---------|
| 1 | Throughput | D1 | Tasks completed per unit time | [0, 1] | 1 = max observed |
| 2 | Conflict rate | D2 | Fraction of merges producing conflicts | [0, 1] | 0 = no conflicts |
| 3 | Staleness | D3 | Mean age of stale facts across agents | [0, 1] | 0 = fully current |
| 4 | Merge overhead | D4 | Fraction of time merging vs working | [0, 1] | 0 = no overhead |
| 5 | Balance | D5 | Uniformity of agent utilization | [0, 1] | 1 = perfectly balanced |
| 6 | Blocking time | D6 | Fraction of time blocked on barriers/merges | [0, 1] | 0 = no blocking |
| 7 | Knowledge loss | D7 | Epistemic gap at session end (pre-harvest) | [0, 1] | 0 = all harvested |

### 2.3 Default Weights

```
[fitness:topo :fitness/throughput-weight     0.25 tx:genesis assert]
[fitness:topo :fitness/conflict-weight       0.20 tx:genesis assert]
[fitness:topo :fitness/staleness-weight      0.10 tx:genesis assert]
[fitness:topo :fitness/overhead-weight       0.15 tx:genesis assert]
[fitness:topo :fitness/balance-weight        0.10 tx:genesis assert]
[fitness:topo :fitness/blocking-weight       0.10 tx:genesis assert]
[fitness:topo :fitness/knowledge-loss-weight 0.10 tx:genesis assert]
```

Rationale:
- **Throughput** (0.25) dominates because getting work done is the primary goal
- **Conflict rate** (0.20) is second because conflicts cause rework (high per-occurrence cost)
- **Merge overhead** (0.15) is third because it's the direct cost of coordination
- **The rest** (0.10 each) are important but secondary signals

---

## 3. How Each Dimension Is Computed

### 3.1 D1 — Throughput

```
D1(T, window) = tasks_completed(window) / max_historical_throughput
```

Normalized against the maximum throughput ever observed (across all topologies and sessions).
D1 = 1.0 means "as good as we've ever done."

Queryable:
```datalog
[:find (count ?t)
 :where
 [?t :task/status :completed]
 [?t :task/completed-at ?ts]
 [(> ?ts window-start)]]
```

### 3.2 D2 — Conflict Rate

```
D2(T, window) = merge_conflicts(window) / total_merges(window)
```

A conflict is a datom entity (spec/04-resolution.md, three-tier conflict routing).
Every merge that produces at least one Conflict entity increments the numerator.

Queryable:
```datalog
[:find (count ?c)
 :where
 [?c :conflict/detected-at ?ts]
 [(> ?ts window-start)]]
```

### 3.3 D3 — Staleness

```
D3(T) = mean over agents alpha of: staleness(alpha)

staleness(alpha) = |{d in S | d not in visible(alpha)}| / |S|
                 = fraction of total datoms not yet in alpha's frontier
```

Uses the frontier-relative query mechanism (SQ-001). The gap between each agent's
frontier and the global store is its staleness.

### 3.4 D4 — Merge Overhead

```
D4(T, window) = total_merge_time(window) / total_agent_time(window)
```

Measurable from transaction timestamps: merge operations produce datoms with timing metadata.

```
[merge:m1 :merge/duration-ms 45 tx:100 assert]
[merge:m1 :merge/datoms-transferred 12 tx:100 assert]
[merge:m1 :merge/channel channel:ab tx:100 assert]
```

### 3.5 D5 — Balance (Utilization Uniformity)

```
D5(T) = 1 - coefficient_of_variation(utilizations)
       = 1 - (sigma / mu) of per-agent task completion counts
```

If all agents complete equal work: D5 = 1.0.
If one agent does everything: D5 -> 0.0.

Unbalanced utilization means the assignment policy Pi is suboptimal or the merge topology
Phi is creating bottlenecks (hub agent in star is overloaded).

### 3.6 D6 — Blocking Time

```
D6(T, window) = total_blocking_time(window) / total_agent_time(window)
```

Blocking events include:
- Waiting for sync barrier (spec/08-sync.md)
- Waiting for merge with busy agent
- Waiting for file reservation release
- Waiting for deliberation resolution
- Waiting for topology transition acknowledgment

Each event is a datom:
```
[block:b1 :block/agent agent:alpha tx:100 assert]
[block:b1 :block/type :sync-barrier tx:100 assert]
[block:b1 :block/duration-ms 1500 tx:100 assert]
```

### 3.7 D7 — Knowledge Loss

```
D7(T) = mean over agents alpha of: |Delta(alpha)| / |K_alpha|

Where Delta(alpha) = K_alpha \ K_store = agent's epistemic gap
      K_alpha = total knowledge agent holds
```

Reuses the harvest quality metric from spec/05-harvest.md (INV-HARVEST-003). A topology
that fragments knowledge or makes harvest difficult will have high D7.

---

## 4. The D2 vs D4 Tension

The conflict rate (D2) and merge overhead (D4) are in direct tension. This is the
**fundamental tradeoff** of coordination topology:

```
More merging -> D2 down (fewer conflicts) but D4 up (more overhead)
Less merging -> D4 down (less overhead) but D2 up (more conflicts)
```

The optimal point is where marginal cost of additional merge = marginal cost of conflict:

```
d(conflict_cost)/d(merge_frequency) = d(merge_cost)/d(merge_frequency)
```

Since conflict_cost >> merge_cost (a conflict causes rework; a merge is cheap computation),
the optimal point is biased toward more merging. This is why the cold-start algorithm
(06-cold-start.md) defaults to mesh (maximum merging) — the cost curve is asymmetric.

As the system learns the actual cost curves (from harvested outcomes), it finds the precise
optimal merge frequency per channel. The coupling weights encode this learning.

---

## 5. The Bilateral Loop for Topology

### 5.1 Loop Structure

```
                    +-------------------+
                    |   F(T) = 0.72     | <- current fitness
                    +--------+----------+
                             |
                   +---------v-----------+
                   |  DETECT DRIFT        |
                   |  delta_F = F(T)-F(T')|
                   |  Which Di dropped?   |
                   +---------+-----------+
                             |
               +-------------+-------------+
               v             v             v
         D2 increased   D4 increased   D5 decreased
         (conflicts)    (overhead)     (imbalance)
               |             |             |
               v             v             v
         TIGHTEN Phi    RELAX Phi     REBALANCE Pi
         (more merge)   (less merge)  (reassign tasks)
               |             |             |
               +-------------+-------------+
                             |
                   +---------v-----------+
                   |  PROPOSE CHANGE      |
                   |  (topology datom)    |
                   +---------+-----------+
                             |
                   +---------v-----------+
                   |  TRANSITION (04)     |
                   |  Tier M or NM        |
                   +---------+-----------+
                             |
                   +---------v-----------+
                   |  MEASURE F(T')       |
                   |  Did fitness improve?|
                   +---------+-----------+
                             |
                     +-------v-------+
                     |   HARVEST      |
                     |   outcome      |
                     |   datoms       |
                     +---------------+
```

### 5.2 Diagnostic Mapping

The mapping from dimension degradation to corrective action:

| Dimension Dropping | Root Cause | Recommended Adjustment | Component |
|--------------------|-----------|----------------------|-----------|
| D1 (throughput) down | Agents idle or blocked | Scale up (Sigma) or unblock critical path | Sigma, Pi |
| D2 (conflicts) up | Coupled tasks on different agents | Tighten merge frequency (Phi) or co-locate tasks (Pi) | Phi, Pi |
| D3 (staleness) up | Merge frequency too low | Increase merge frequency (Phi) | Phi |
| D4 (overhead) up | Merge frequency too high | Decrease merge frequency (Phi) or change topology type | Phi, G |
| D5 (balance) down | Uneven task assignment | Rebalance assignment (Pi) | Pi |
| D6 (blocking) up | Too many sync barriers | Restructure to reduce non-monotonic dependencies | G |
| D7 (knowledge loss) up | Harvest not keeping up | Trigger harvest earlier; adjust topology for harvest access | Phi |

### 5.3 Automated vs Manual Adjustment

The diagnostic mapping can be applied automatically for Tier 1 adjustments (low commitment
weight) or surfaced as recommendations for Tier 2/3 (higher commitment weight).

The authority function (05-scaling-authority.md) governs which adjustments are autonomous.

---

## 6. F(T) Convergence Properties

### 6.1 INV-TOPO-FIT-001: Monotonic Improvement Under Bilateral Loop

**Statement:** When the bilateral loop recommends a topology change and the change is
enacted, F(T') >= F(T) - epsilon (within noise margin epsilon = 0.02).

**Falsification:** An enacted topology change decreases F(T) by more than epsilon across
the measurement window.

**Verification:** V:PROP (track F(T) before and after each change; assert improvement
within margin)

**Note:** This is a probabilistic guarantee. Individual changes may decrease F(T) due to
measurement noise, but the trend must be non-decreasing. Three consecutive decreases
trigger Signal::TopologyDrift.

### 6.2 INV-TOPO-FIT-002: Independence of F(T) and F(S)

**Statement:** The topology fitness F(T) and specification fitness F(S) are independent:
changes to topology do not affect specification convergence, and vice versa.

**Falsification:** A topology change causes F(S) to decrease, or a spec change causes
F(T) to decrease.

**Verification:** V:PROP (measure both F(S) and F(T) across changes; assert independence
via correlation test)

**Rationale:** F(T) measures coordination effectiveness. F(S) measures specification
quality. They operate on orthogonal concerns. If they correlate, it indicates a confound
that needs investigation (e.g., bad topology causing agents to produce bad spec work).

---

## 7. Composition with F(S): Total System Fitness

### 7.1 Formula

```
F_total = lambda * F(S) + (1 - lambda) * F(T)
```

Where lambda = relative importance of spec quality vs coordination quality.

### 7.2 Lambda by Phase

| Phase | lambda | Rationale |
|-------|--------|-----------|
| Specification production | 0.80 | Spec quality is primary goal |
| Single-agent implementation | 0.90 | Coordination irrelevant (one agent) |
| Multi-agent implementation | 0.50 | Both matter equally |
| Pure coordination tasks | 0.20 | Topology fitness dominates |
| Agent onboarding/scaling | 0.30 | Coordination effectiveness critical |

Lambda as datom:
```
[fitness:total :fitness/spec-weight 0.80 tx:genesis assert]
```

### 7.3 The Quadrilateral Model

F_total drives convergence across all four vertices of the quadrilateral:

```
        Intent
       /      \
      /        \
   Spec ---- Impl
      \        /
       \      /
      Topology
```

| Loop | Fitness Component | Detection Mechanism |
|------|-------------------|---------------------|
| Intent <-> Spec | F(S) | Goal-drift signal |
| Spec <-> Impl | F(S) | Bilateral scan, annotation check |
| Impl <-> Topology | F(T) | F(T) measurement, coordination drift |
| Topology <-> Intent | F(T) | Strategic review, outcome quality |

F_total = 1.0 is the fixpoint where all four vertices are coherent.

---

## 8. F(T) as Datom

Every F(T) measurement is a datom in the store:

```
[fitness-measurement:f1 :fitness/type :topology tx:200 assert]
[fitness-measurement:f1 :fitness/score 0.72 tx:200 assert]
[fitness-measurement:f1 :fitness/d1-throughput 0.88 tx:200 assert]
[fitness-measurement:f1 :fitness/d2-conflicts 0.05 tx:200 assert]
[fitness-measurement:f1 :fitness/d3-staleness 0.08 tx:200 assert]
[fitness-measurement:f1 :fitness/d4-overhead 0.12 tx:200 assert]
[fitness-measurement:f1 :fitness/d5-balance 0.85 tx:200 assert]
[fitness-measurement:f1 :fitness/d6-blocking 0.03 tx:200 assert]
[fitness-measurement:f1 :fitness/d7-knowledge-loss 0.02 tx:200 assert]
[fitness-measurement:f1 :fitness/topology topo:current tx:200 assert]
[fitness-measurement:f1 :fitness/session session:current tx:200 assert]
[fitness-measurement:f1 :fitness/measured-at 1741500000000 tx:200 assert]
```

This enables:
- Historical comparison: "how has F(T) trended over sessions?"
- Topology correlation: "which topologies produced the highest F(T)?"
- Dimension analysis: "which dimension is the current bottleneck?"
- Learning: "does relaxing merge frequency improve or degrade F(T)?"

---

## 9. Traceability

| Concept | Traces to |
|---------|-----------|
| F(T) parallels F(S) | spec/10-bilateral.md (fitness function for specification) |
| Rate-distortion framing | spec/13-budget.md (attention budget, projection pyramid) |
| Seven-dimension decomposition | Analogous to F(S)'s six dimensions (validation, coverage, drift, etc.) |
| Bilateral loop for topology | spec/10-bilateral.md (bilateral convergence loop) |
| Diagnostic mapping | spec/12-guidance.md (R(t) routing, six anti-drift mechanisms) |
| Signal::TopologyDrift | spec/09-signal.md (signal types and routing) |
| Quadrilateral model | spec/18-trilateral.md (extended from trilateral to quadrilateral) |
| Outcome harvesting | SEED.md S5 (harvest/seed lifecycle) |

---

*Next: `08-open-questions.md` — what remains uncertain and what would resolve it.*
