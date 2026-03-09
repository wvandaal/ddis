# 04 — Topology Transition Protocol

> **Summary:** CALM compliance stratifies topology transitions into two tiers. Monotonic
> transitions (adding channels/agents, increasing merge frequency) are safe without
> coordination barriers. Non-monotonic transitions (removing channels/agents, reducing
> merge frequency) require sync barriers to prevent data loss. The commitment weight
> determines grace period duration and timeout behavior.

---

## 1. The Key Insight: Topology Independence of Ordering

Before designing a protocol, we establish what the CRDT algebra already guarantees:

### 1.1 Theorem: Topology Independence of Ordering

**Statement:** For any two topologies T1, T2 over the same agent set, if all datoms
eventually propagate (liveness), then the LIVE index converges to the same state
regardless of which topology was used during propagation.

**Proof sketch:** LIVE index is computed from the datom set S via per-attribute resolution
modes. Resolution modes are deterministic functions of S (LWW uses max TxId, lattice
uses join, multi-value uses set). Since MERGE is set union (L1-L3), the final S is the
same regardless of merge order or channel topology. Therefore LIVE(S) is the same. QED

### 1.2 Corollary: Topology Transitions Cannot Cause Data Corruption

Because causal ordering is encoded in the datom (TxId = HLC with wall_time, logical counter,
agent_id), not in the merge topology, a topology transition cannot violate causal ordering.
The topology determines WHEN agents learn facts, not WHAT ORDER they learn them. The ordering
is always reconstructable from the TxId.

### 1.3 What CAN Go Wrong

The only risks during a topology transition are:
1. **Temporary inconsistency**: Agents on different topologies see different subsets of S
2. **Stale state**: An agent loses its high-frequency merge channel and temporarily falls behind
3. **Orphaning**: An agent loses ALL channels — violates connectivity invariant

None of these cause data loss or corruption. They affect availability and freshness, not safety.

---

## 2. CALM-Stratified Transition Protocol

### 2.1 Classification

CALM theorem: monotonic operations safe without coordination; non-monotonic require barriers.

**Monotonic topology transitions** (add capacity):
- Adding a communication channel between two agents
- Adding an agent to a cluster
- Increasing merge frequency on an existing channel
- Adding a new cluster to a hybrid topology

Property: channels(T_new) is a superset of channels(T_old). No agent loses any channel.

**Non-monotonic topology transitions** (remove capacity):
- Removing a communication channel
- Removing an agent from a cluster
- Decreasing merge frequency
- Removing a cluster from a hybrid topology
- Restructuring (e.g., switching from mesh to star)

Property: channels(T_new) is NOT a superset of channels(T_old). Some agent loses a channel.

---

## 3. Tier M: Monotonic Transitions (No Barrier Required)

### 3.1 Protocol

```
1. ASSERT: Topology change transacted as datom
   [topo-change:tau :topo/type :add-channel tx:t assert]
   [topo-change:tau :topo/channel channel:new tx:t assert]
   [topo-change:tau :topo/status :enacted tx:t assert]

2. PROPAGATE: Datom flows through EXISTING channels (old topology)
   Guaranteed by liveness of current topology.

3. ADOPT: Each agent, upon receiving the datom, activates the new channel.
   Agent checks: "does this change add capacity for me?"
   If yes: activate immediately.
   Adoption is idempotent (receiving the same change twice = no-op).

4. LIVE: New topology is fully active once all agents have received the datom.
   No explicit confirmation needed.
   Convergence guaranteed by CRDT liveness.
```

### 3.2 Formal Properties

- **Safety:** No datom is lost (old channels still active during propagation)
- **Liveness:** All agents eventually adopt (old topology propagates the change)
- **Monotonicity:** channels(T_new) is superset of channels(T_old) throughout transition
- **Convergence:** After propagation delay, channels(agent) = channels(T_new) for all agents

### 3.3 Latency Analysis

Propagation delay is bounded by the diameter of the old topology times the merge frequency:

| Old Topology | Diameter | At :high merge (1 min) | At :medium (5 min) |
|-------------|----------|----------------------|-------------------|
| Mesh | 1 | 1 minute | 5 minutes |
| Star | 2 | 2 minutes | 10 minutes |
| Pipeline(n) | n | n minutes | 5n minutes |
| Hierarchy(h) | 2h | 2h minutes | 10h minutes |

---

## 4. Tier NM: Non-Monotonic Transitions (Barrier Required)

### 4.1 Protocol

```
1. PROPOSE: Topology change transacted with status :proposed
   [topo-change:tau :topo/status :proposed tx:t1 assert]
   [topo-change:tau :topo/old-topology topo:current tx:t1 assert]
   [topo-change:tau :topo/new-topology topo:next tx:t1 assert]
   [topo-change:tau :topo/grace-period-ms <computed> tx:t1 assert]
   [topo-change:tau :topo/barrier-timeout-ms <computed> tx:t1 assert]

2. PROPAGATE: Proposal flows through current topology (all channels still active).
   This is a monotonic operation (asserting a datom) so it propagates normally.

3. ACKNOWLEDGE: Each affected agent asserts acknowledgment.
   [topo-ack:a :ack/change topo-change:tau tx:t2 assert]
   [topo-ack:a :ack/agent agent:self tx:t2 assert]

   "Affected" = any agent that will lose a channel or be removed.
   Acknowledgment means: "I have received the proposal and am prepared to transition."

4. BARRIER: Initiator queries for complete acknowledgment.

   [:find (count ?ack)
    :where
    [?ack :ack/change topo-change:tau]
    [?ack :ack/agent ?a]
    [(affected-agents topo-change:tau) ?affected]
    [(contains ?affected ?a)]]

   This is a NON-MONOTONIC query (absence of acknowledgment matters).
   Requires sync barrier (INV-SYNC-001).
   All agents must reach consistent cut before query is valid.

5. GRACE PERIOD: Old and new channels both active for duration delta.
   delta = max(in_flight_merge_time, configurable minimum)
   During grace period: agents on old topology can flush pending merges.
   Safety: no in-flight data lost.

6. ENACT: After grace period, status transitions to :enacted.
   [topo-change:tau :topo/status :enacted tx:t3 assert]
   Agents deactivate old channels, activate new channels.
   Simultaneous (within one merge cycle of receiving :enacted datom).

7. VERIFY: Post-transition connectivity check.
   Datalog query: is the agent graph under T_new connected?

   [:find ?disconnected
    :where
    [?a :agent/status :active]
    (not
      [:find ?path
       :where
       [(reachable ?a ?any-other-agent :channel/active true) ?path]])]

   If disconnected: ROLLBACK (re-assert old topology, retract change).
```

### 4.2 Formal Properties

- **Safety:** No datom is lost (grace period ensures in-flight merges complete)
- **Liveness:** Barrier completes in bounded time (timeout -> rollback)
- **Atomicity:** All agents switch within one merge cycle of :enacted
- **Connectivity:** Post-transition graph verified connected
- **Reversibility:** Rollback is a monotonic transition (re-adding channels) -> always safe

### 4.3 Timeout Behavior

If barrier doesn't complete within barrier-timeout-ms:
- Change status -> :timed-out
- Old topology remains active
- Signal::TopologyTransitionFailed fired
- Guidance recommends investigating which agent(s) didn't acknowledge
- No automatic retry (human investigation may be needed)

### 4.4 Grace Period and Timeout Computation

Both are functions of commitment weight:

```
grace_period(transition) = base_grace * commitment_weight(transition)
barrier_timeout(transition) = base_timeout * commitment_weight(transition)
```

Defaults:
```
[transition:defaults :transition/base-grace-ms 5000 tx:genesis assert]      ;; 5 seconds
[transition:defaults :transition/base-timeout-ms 300000 tx:genesis assert]  ;; 5 minutes
```

High-commitment transitions (removing agent with many dependents):
  commitment_weight ~ 0.7 -> grace = 3.5s, timeout = 3.5 minutes

Low-commitment transitions (reducing merge frequency on lightly-used channel):
  commitment_weight ~ 0.1 -> grace = 0.5s, timeout = 30s

---

## 5. Tier Selection: Automatic via Algebraic Property

The choice between Tier M and Tier NM is not manual — it is determined by the transition's
algebraic properties:

```
tier(transition) =
  if channels(T_new) is superset of channels(T_old):
    Tier_M          -- monotonic: adding capacity
  else:
    Tier_NM         -- non-monotonic: removing capacity
```

This is a mechanical check: compare the channel sets before and after. No human judgment needed.

Within Tier NM, the commitment weight determines operational parameters (grace period, timeout).

---

## 6. State Machine

```
                    +---------------------------------------------+
                    |              Tier M path                     |
                    |  (monotonic: channels only grow)             |
                    |                                             |
    PROPOSED ------>|  PROPAGATING --> ADOPTED (immediate)        |
        |           |                                             |
        |           +---------------------------------------------+
        |
        |           +---------------------------------------------+
        |           |              Tier NM path                    |
        +---------->|                                             |
                    |  PROPAGATING --> ACKNOWLEDGING               |
                    |                      |                       |
                    |                      v                       |
                    |                  BARRIER_MET --> GRACE       |
                    |                      |            |          |
                    |                      |            v          |
                    |                  TIMED_OUT    ENACTED        |
                    |                      |            |          |
                    |                      v            v          |
                    |                  ROLLED_BACK   VERIFIED      |
                    |                                   |          |
                    |                               FAILED?        |
                    |                              /       \       |
                    |                         ROLLBACK   ACTIVE    |
                    +---------------------------------------------+
```

### 6.1 State Definitions

| State | Meaning | Entry Condition |
|-------|---------|-----------------|
| PROPOSED | Change datom asserted | Transaction accepted |
| PROPAGATING | Change datom flowing through old topology | Immediately after PROPOSED |
| ADOPTED (Tier M) | All agents have activated new channels | All agents received change datom |
| ACKNOWLEDGING (Tier NM) | Waiting for affected agents to acknowledge | After propagation |
| BARRIER_MET | All acknowledgments received | Non-monotonic query satisfied |
| TIMED_OUT | Barrier didn't complete in time | Timeout reached |
| GRACE | Old and new channels both active | After BARRIER_MET, for grace period |
| ENACTED | New topology active, old channels deactivated | After grace period |
| VERIFIED | Post-transition connectivity confirmed | BFS/DFS on agent graph |
| ACTIVE | Transition complete, new topology is current | After VERIFIED |
| ROLLED_BACK | Transition reverted to old topology | After TIMED_OUT or FAILED |

### 6.2 Rollback Protocol

Rollback from any Tier NM state:
1. Assert retraction of topology change: [topo-change:tau :topo/status :rolled-back tx:t assert]
2. Re-assert old topology channels (this is a Tier M operation — monotonic, instant)
3. Record rollback reason as datom (provenance for learning)

Rollback is always safe because it is a monotonic transition (re-adding removed channels).

---

## 7. Invariants

### INV-TOPO-TRANS-001: Monotonic Safety
**Statement:** A Tier M transition never decreases channel count.
**Falsification:** |channels(T_new)| < |channels(T_old)| and tier = M.
**Verification:** V:TYPE (type system enforces monotonic transition type only accepts
channel additions). V:PROP (proptest: for any Tier M transition, assert channel set
grows or stays the same).

### INV-TOPO-TRANS-002: Grace Period Completeness
**Statement:** No in-flight merge is lost during a Tier NM transition.
**Falsification:** A datom was in-transit on a channel that was deactivated before the
datom was received.
**Verification:** V:PROP (property test: inject datom just before grace period start,
verify it arrives before grace period end).

### INV-TOPO-TRANS-003: Connectivity Preservation
**Statement:** The agent graph is connected after every enacted transition.
**Falsification:** There exist agents alpha, beta with no path between them in T_new.
**Verification:** V:PROP (BFS/DFS connectivity check on agent graph after every transition).

### INV-TOPO-TRANS-004: Rollback Safety
**Statement:** A rollback is always a monotonic transition (re-adding removed channels).
**Falsification:** Rollback removes channels that existed in T_old.
**Verification:** V:TYPE (rollback function signature guarantees channels(rollback) is
superset of channels(T_old)). V:PROP (proptest: for any rollback, assert channel set
includes all of T_old's channels).

---

## 8. Traceability

| Concept | Traces to |
|---------|-----------|
| Topology Independence of Ordering theorem | spec/01-store.md L1-L5 (CRDT laws) |
| CALM stratification of transitions | spec/03-query.md (CALM compliance, six strata) |
| Sync barrier for non-monotonic transitions | spec/08-sync.md (INV-SYNC-001 through 005) |
| Commitment weight for grace period | ADRS.md AS-002 (continuous commitment weight) |
| Rollback as monotonic transition | spec/01-store.md (monotonicity guarantees safety) |
| Connectivity verification | spec/03-query.md query/graph.rs (BFS/DFS, connected components) |
| Signal::TopologyTransitionFailed | spec/09-signal.md (signal types and routing) |

---

*Next: `05-scaling-authority.md` — who decides when to scale, and how trust is earned.*
