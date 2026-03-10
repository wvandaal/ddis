# 09 — Invariants Catalog

> **Summary:** Complete catalog of all topology invariants defined in this exploration.
> Each invariant has an ID, formal statement, falsification condition, verification method,
> and traceability to SEED.md or existing spec elements.

---

## Naming Convention

```
INV-TOPO-{COMPONENT}-{NNN}

Components:
  TRANS = Transition protocol (04-transition-protocol.md)
  COLD  = Cold-start bootstrap (06-cold-start.md)
  FIT   = Fitness function (07-fitness-function.md)
```

When these invariants are distilled into the Braid spec, they may be renamed to fit
the existing namespace scheme (e.g., INV-MERGE-*, INV-SYNC-*, INV-GUIDANCE-*) depending
on which spec namespace they naturally extend.

---

## Transition Protocol Invariants

### INV-TOPO-TRANS-001: Monotonic Safety

**Traces to:** spec/01-store.md L4 (monotonicity); CALM theorem
**Type:** Safety invariant
**Statement:** A Tier M (monotonic) transition never decreases the channel count.
**Formal:** For any Tier M transition T_old -> T_new:
  |channels(T_new)| >= |channels(T_old)|
  AND channels(T_old) is a subset of channels(T_new)
**Falsification:** |channels(T_new)| < |channels(T_old)| and the transition was classified
as Tier M.
**Verification:**
  V:TYPE — Type system enforces that the MonotonicTransition type constructor only accepts
  channel additions (AddChannel, IncreaseFrequency, AddAgent). The type does not expose
  RemoveChannel or DecreaseFrequency operations.
  V:PROP — Proptest: generate random Tier M transitions, assert channel set only grows.

---

### INV-TOPO-TRANS-002: Grace Period Completeness

**Traces to:** spec/08-sync.md (sync barriers); spec/01-store.md C1 (append-only, no data loss)
**Type:** Safety invariant
**Statement:** No in-flight merge data is lost during a Tier NM transition.
**Formal:** For any datom d that was in-transit on channel C at time t_grace_start:
  d is received by the target agent before t_grace_end.
**Falsification:** A datom d was in-transit on a channel that was deactivated before d
was received by the target agent.
**Verification:**
  V:PROP — Property test: inject a datom on channel C immediately before grace period
  start. Assert that the datom appears in the target agent's store before grace period end.
  Parameterize over: channel latency, datom size, grace period duration.

---

### INV-TOPO-TRANS-003: Connectivity Preservation

**Traces to:** Graph theory (connected graph); coordination liveness
**Type:** Safety invariant
**Statement:** The agent graph is connected after every enacted topology transition.
**Formal:** For any enacted transition resulting in topology T_new:
  For all pairs of active agents (alpha, beta):
    exists path from alpha to beta through active channels in T_new.
**Falsification:** After an enacted transition, there exist active agents alpha, beta
with no path between them in the agent graph under T_new.
**Verification:**
  V:PROP — After every simulated transition, run BFS/DFS connectivity check on the
  agent graph. Assert all active agents are in the same connected component.

---

### INV-TOPO-TRANS-004: Rollback Safety

**Traces to:** spec/01-store.md L4 (monotonicity); CALM theorem
**Type:** Safety invariant
**Statement:** A rollback is always a monotonic transition (re-adding removed channels).
**Formal:** For any rollback of transition tau that reverts T_new to T_old:
  channels(T_old) is a subset of channels(rollback_result)
**Falsification:** A rollback removes channels that existed in T_old.
**Verification:**
  V:TYPE — Rollback function signature takes T_old as input and produces a topology
  where channels(result) is a superset of channels(T_old). The type prevents removing
  channels during rollback.
  V:PROP — Proptest: generate random Tier NM transitions, simulate rollback, assert
  channel set includes all of T_old's channels.

---

## Cold-Start Invariants

### INV-TOPO-COLD-001: Monotonic Relaxation

**Traces to:** Minimax strategy (asymmetric cost function); 06-cold-start.md theorem
**Type:** Progress invariant
**Statement:** The coordination intensity of the topology never increases without evidence
of under-coordination.
**Formal:** Let intensity(T) = sum over all channels of merge_frequency_rank(channel).
  If conflict_rate = 0 AND quality >= quality_prev - epsilon:
    intensity(T_next) <= intensity(T_current)
**Falsification:** Merge frequency increases or topology structure tightens (more channels
added, higher frequency assigned) when conflict_rate = 0 and quality is stable or improving.
**Verification:**
  V:PROP — Track coordination_intensity metric across simulated sessions. Assert
  monotonically non-increasing except when conflict events occur (D2 > 0) or quality
  drops (D1 decreases beyond noise margin).

---

### INV-TOPO-COLD-002: Cold-Start Safety

**Traces to:** Minimax dominance theorem (06-cold-start.md)
**Type:** Safety invariant
**Statement:** The cold-start topology provides coordination intensity >= the
coupling-optimal topology for the actual task coupling.
**Formal:** For any coupling matrix C:
  intensity(COLD_START(C)) >= intensity(OPTIMAL(C))
  where OPTIMAL(C) is the topology minimizing total cost (merge overhead + conflict cost)
  for the given coupling.
**Falsification:** There exists a coupling matrix C where
  intensity(COLD_START(C)) < intensity(OPTIMAL(C)).
**Verification:**
  V:PROP — Generate random coupling matrices C (various sizes, densities). Compute
  COLD_START topology and OPTIMAL topology (brute-force for small n). Assert
  COLD_START intensity >= OPTIMAL intensity for all generated C.

---

## Fitness Function Invariants

### INV-TOPO-FIT-001: Monotonic Improvement Under Bilateral Loop

**Traces to:** spec/10-bilateral.md (bilateral convergence); SEED.md S7 (self-improvement loop)
**Type:** Progress invariant
**Statement:** When the bilateral loop recommends a topology change and the change is
enacted, F(T') >= F(T) - epsilon (within noise margin epsilon = 0.02).
**Formal:** For any enacted topology change recommended by the bilateral loop:
  F(T_after) >= F(T_before) - 0.02
**Falsification:** An enacted topology change decreases F(T) by more than epsilon (0.02)
across the measurement window.
**Verification:**
  V:PROP — Track F(T) before and after each enacted bilateral-loop-recommended change.
  Assert F(T_after) >= F(T_before) - epsilon. Allow for measurement noise via statistical
  test (95% confidence interval includes improvement).
**Note:** Three consecutive decreases beyond epsilon trigger Signal::TopologyDrift. This
is a probabilistic guarantee — individual changes may decrease due to noise.

---

### INV-TOPO-FIT-002: Independence of F(T) and F(S)

**Traces to:** spec/10-bilateral.md (F(S)); orthogonality of spec quality and coordination
**Type:** Structural invariant
**Statement:** F(T) and F(S) are independent: topology changes do not affect specification
convergence, and specification changes do not affect coordination effectiveness.
**Formal:** For any topology change delta_T:
  |F(S_after) - F(S_before)| < epsilon_independence (0.05)
  AND for any spec change delta_S:
  |F(T_after) - F(T_before)| < epsilon_independence (0.05)
**Falsification:** A topology change causes F(S) to decrease by more than 0.05, or a
spec change causes F(T) to decrease by more than 0.05.
**Verification:**
  V:PROP — Measure both F(S) and F(T) across topology and spec changes. Compute
  Pearson correlation coefficient. Assert |r| < 0.3 (weak or no correlation).
**Note:** If correlation is found, it indicates a confound requiring investigation
(e.g., bad topology causing agents to produce poor spec work, or spec complexity
requiring topology adjustment).

---

## Invariant Dependency Graph

```
INV-TOPO-COLD-002 (cold-start safety)
  |
  v
INV-TOPO-COLD-001 (monotonic relaxation)
  |
  v
INV-TOPO-FIT-001 (F(T) improvement)
  |
  v
INV-TOPO-FIT-002 (independence)

INV-TOPO-TRANS-001 (monotonic safety)
  |
  v
INV-TOPO-TRANS-004 (rollback safety)
  |
  v
INV-TOPO-TRANS-003 (connectivity)
  |
  v
INV-TOPO-TRANS-002 (grace period)
```

Two independent chains:
1. **Convergence chain:** Cold-start safety -> Monotonic relaxation -> F(T) improvement -> Independence
2. **Transition chain:** Monotonic safety -> Rollback safety -> Connectivity -> Grace period

The convergence chain ensures the topology improves over time.
The transition chain ensures topology changes don't cause data loss.

---

## Cross-Reference to Existing Braid Invariants

| Topology Invariant | Depends On | Why |
|--------------------|-----------|-----|
| INV-TOPO-TRANS-001 | INV-STORE-004 (L4 monotonicity) | Channel monotonicity mirrors store monotonicity |
| INV-TOPO-TRANS-002 | INV-STORE-001 (append-only, C1) | No data loss is a consequence of append-only |
| INV-TOPO-TRANS-003 | INV-QUERY-* (graph algorithms) | Connectivity check uses BFS/DFS from query engine |
| INV-TOPO-COLD-001 | INV-HARVEST-001 (epistemic gap) | Relaxation requires evidence = harvested outcomes |
| INV-TOPO-FIT-001 | INV-BILATERAL-001 (convergence) | Topology bilateral loop mirrors spec bilateral loop |
| INV-TOPO-FIT-002 | INV-BILATERAL-001 (convergence) | Independence ensures loops don't interfere |

---

*Next: `10-design-decisions.md` — all ADRs from this exploration.*
