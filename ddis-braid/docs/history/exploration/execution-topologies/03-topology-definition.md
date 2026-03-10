# 03 — Topology Definition: T = (G, Phi, Sigma, Pi)

> **Summary:** A coordination topology is a 4-tuple T = (G, Phi, Sigma, Pi) where G is the
> agent graph, Phi is the merge policy, Sigma is the scaling policy, and Pi is the assignment
> policy. All four components are represented as datoms, queryable via Datalog, and evolvable
> through the bilateral learning loop.

---

## 1. Formal Definition

A **coordination topology** T = (G, Phi, Sigma, Pi) where:

| Component | Domain | Determines |
|-----------|--------|-----------|
| **G** (Agent graph) | Vertices = agents, Edges = channels | Which agents exist and which can communicate |
| **Phi** (Merge policy) | G x CouplingMatrix -> MergeFrequency | Which channels are active and at what merge frequency |
| **Sigma** (Scaling policy) | SystemState -> ScalingAction | When to add/remove agents, how to partition into clusters |
| **Pi** (Assignment policy) | Agents x Tasks -> AssignmentScore | Which agent should work on which task |

---

## 2. G: The Agent Graph

### 2.1 Agents as Entities

Each agent is an entity in the datom store:

```
[agent:alpha :agent/program "claude-code" tx:1 assert]
[agent:alpha :agent/model "opus-4.6" tx:1 assert]
[agent:alpha :agent/capabilities #{:store :query :schema} tx:1 assert]
[agent:alpha :agent/status :active tx:1 assert]
[agent:alpha :agent/cluster cluster:A tx:5 assert]
[agent:alpha :agent/frontier frontier:alpha-100 tx:100 assert]
```

Agent attributes:

| Attribute | Type | Resolution | Purpose |
|-----------|------|-----------|---------|
| :agent/program | Keyword | LWW | Which AI system (claude-code, codex, gemini) |
| :agent/model | Keyword | LWW | Which model (opus-4.6, sonnet-4.6) |
| :agent/capabilities | Multi-value set | Multi | Skills and domain competencies |
| :agent/status | Keyword | LWW | :active, :idle, :terminating, :terminated |
| :agent/cluster | Ref | LWW | Current cluster membership |
| :agent/frontier | Ref | LWW | Current knowledge frontier |
| :agent/session-start | Instant | LWW | When agent's current session began |
| :agent/task-count | Long | LWW | Number of tasks completed this session |

### 2.2 Channels as Entities

Communication channels between agents:

```
[channel:ab :channel/from agent:alpha tx:2 assert]
[channel:ab :channel/to agent:beta tx:2 assert]
[channel:ab :channel/type :peer tx:2 assert]
[channel:ab :channel/merge-frequency :high tx:2 assert]
[channel:ab :channel/active true tx:2 assert]
```

Channel attributes:

| Attribute | Type | Resolution | Purpose |
|-----------|------|-----------|---------|
| :channel/from | Ref | LWW | Source agent |
| :channel/to | Ref | LWW | Target agent |
| :channel/type | Keyword | LWW | :peer, :supervisory, :pipeline, :broadcast |
| :channel/merge-frequency | Keyword | Lattice(max) | :realtime, :high, :medium, :low, :session |
| :channel/active | Boolean | LWW | Whether channel is currently active |
| :channel/coupling-score | Double | Lattice(max) | Coupling that motivated this channel |
| :channel/created-at | Instant | LWW | When channel was established |

### 2.3 Named Topology Patterns

Topology patterns are recognizable graph structures, expressible as Datalog predicates:

**Mesh:**
```
mesh(agents) <=> forall a, b in agents where a != b: exists channel(a, b)
```
All pairs connected. Diameter 1. O(n^2) channels.

**Star:**
```
star(hub, agents) <=> forall a in agents where a != hub:
  exists channel(a, hub) AND NOT exists b != hub: channel(a, b)
```
One hub, all others connect only to hub. Diameter 2. O(n) channels.

**Pipeline:**
```
pipeline(agents, order) <=> forall i in 0..|agents|-1:
  exists channel(agents[i], agents[i+1])
```
Linear chain. Diameter n. O(n) channels.

**Ring:**
```
ring(agents, order) <=> pipeline(agents, order) AND
  exists channel(agents[|agents|-1], agents[0])
```
Pipeline with wrap-around. Diameter n/2. O(n) channels.

**Hierarchy:**
```
hierarchy(root, levels) <=> forall a != root:
  exists unique parent(a) AND parent(a) in level(a) - 1
```
Tree structure. Diameter 2 * height. O(n) channels.

**Hybrid:**
```
hybrid(clusters, bridge, inter_topo) <=>
  forall cluster k: valid_topology(k.topology, k.agents)
  AND forall pair (k1, k2): connected_through(bridge, k1, k2) via inter_topo
```
Composition of sub-topologies with bridge agent(s).

---

## 3. Phi: Merge Policy

### 3.1 Coupling to Merge Frequency Mapping

The merge policy maps coupling scores to merge frequencies:

```
merge_frequency(alpha, beta) = f(coupling(tasks_of(alpha), tasks_of(beta)))

f(c) =
  | c > 0.7              -> :realtime    (merge every transaction)
  | c in [0.5, 0.7]      -> :high        (merge every minute)
  | c in [0.3, 0.5]      -> :medium      (merge every 5 minutes)
  | c in [0.1, 0.3]      -> :low         (merge every 15 minutes)
  | c < 0.1              -> :session     (merge at session boundaries only)
  | c = 0                -> :trunk-only  (no direct merge, only through shared trunk)
```

### 3.2 Merge Frequency Semantics

| Frequency | Period | Consistency | Overhead | Use When |
|-----------|--------|-------------|----------|----------|
| :realtime | Every tx | Near-linearizable | High | Two agents editing the same file section |
| :high | ~1 min | Bounded staleness (1 min) | Medium-high | Same module, different files |
| :medium | ~5 min | Bounded staleness (5 min) | Medium | Related modules, moderate coupling |
| :low | ~15 min | Bounded staleness (15 min) | Low | Loosely related work |
| :session | End of session | Eventual | Minimal | Independent work streams |
| :trunk-only | Next trunk sync | Weak | None | No direct relationship |

### 3.3 Merge Frequency as Datom

The merge frequency is stored per-channel:

```
[channel:ab :channel/merge-frequency :high tx:10 assert]
```

Resolution mode is Lattice(max) — concurrent proposals resolve to higher frequency
(the safe/conservative direction). See 01-algebraic-foundations.md section 3.3.

### 3.4 Dynamic Adjustment

The merge frequency adjusts based on the bilateral loop (07-fitness-function.md):

- If D2 (conflict rate) increases for this channel: increase merge frequency
- If D4 (merge overhead) increases for this channel: decrease merge frequency
- The adjustment is a Tier M (monotonic, increase) or Tier NM (non-monotonic, decrease) transition

---

## 4. Sigma: Scaling Policy

### 4.1 Scale Up Conditions

```
scale_up <=>
  (critical_path_length > threshold AND parallelizable_tasks > active_agents)
  OR (agent_utilization > 0.9 for > 10 minutes)
  OR (unblocked_ready_tasks > active_agents * 1.5)
```

Each condition has a configurable threshold stored as a datom:

```
[scaling:thresholds :scaling/utilization-high 0.9 tx:genesis assert]
[scaling:thresholds :scaling/utilization-duration-ms 600000 tx:genesis assert]
[scaling:thresholds :scaling/ready-task-ratio 1.5 tx:genesis assert]
```

### 4.2 Scale Down Conditions

```
scale_down <=>
  (agent_utilization < 0.2 for > 15 minutes)
  OR (remaining_tasks <= active_agents - 1)
  OR (coupling_within_cluster > coupling_between_clusters -> merge clusters)
```

Scale-down is non-monotonic (removing agents). Subject to the authority function
(05-scaling-authority.md): A(d) = R(1-C)T determines whether autonomous or human-approved.

### 4.3 Partitioning Algorithm

When scaling requires redistribution, partition agents into clusters:

```
partition(agents, tasks) =
  minimize inter_cluster_coupling(partition)
  subject to:
    forall cluster: |tasks(cluster)| >= 1
    forall cluster: |agents(cluster)| >= 1
    forall cluster: coupling_within(cluster) > coupling_threshold
```

This is graph partitioning on the coupling graph. Use spectral partitioning
(eigenvector of graph Laplacian) for the general case.

Note: Revisit spectral partitioning if agent counts consistently <= 8, where simpler
algorithms (thresholded connected components, brute-force partition enumeration) may
be sufficient and more interpretable. For n <= 20, spectral decomposition is O(n^3) =
microseconds, so performance is not a concern. The question is interpretability.

### 4.4 Scaling Actions as Datoms

Every scaling decision is recorded:

```
[scaling:s1 :scaling/action :add-agent tx:50 assert]
[scaling:s1 :scaling/rationale "3 unblocked tasks, 2 agents at 95% util" tx:50 assert]
[scaling:s1 :scaling/authority-score 0.42 tx:50 assert]
[scaling:s1 :scaling/status :recommended tx:50 assert]
[scaling:s1 :scaling/outcome-quality nil tx:50 assert]  ;; filled after harvest
```

---

## 5. Pi: Assignment Policy

### 5.1 Assignment Score Function

```
assignment_score(agent, task) =
  alpha * capability_match(agent, task)
  + beta * coupling_to_current(agent, task)
  + gamma * critical_path_weight(task)
  + delta * historical_success(agent, task_type)
  + epsilon * locality(agent, task)
```

### 5.2 Signal Definitions

**capability_match(agent, task):**
Does the agent have the skills this task requires?

```
capability_match = |agent.capabilities intersect task.required_capabilities|
                 / |task.required_capabilities|
```

If 0: agent cannot do this task. If 1: agent has all required capabilities.

**coupling_to_current(agent, task):**
Is this task related to what the agent is already working on?

```
coupling_to_current = max over T_current in agent.current_tasks:
  coupling(task, T_current)
```

High coupling to current work means the agent already has the context loaded.

**critical_path_weight(task):**
How important is this task to overall progress?

```
critical_path_weight = 1.0 if task is on critical path
                     = 0.5 if task is within 1 hop of critical path
                     = 0.0 otherwise
```

Computed from the task dependency DAG using longest-path algorithm (already specified
in query/graph.rs).

**historical_success(agent, task_type):**
Has this agent succeeded at similar tasks before?

```
historical_success = (completed_well) / (total_assigned) for this agent + task_type
```

Where "completed_well" means the task was completed without rework, with quality score
above threshold. Requires outcome data from previous sessions (harvested).

**locality(agent, task):**
Is the task's data in the agent's current frontier?

```
locality = |relevant_datoms(task) intersect visible(agent)| / |relevant_datoms(task)|
```

If high: agent already has the context. If low: agent needs to merge/sync first.

### 5.3 Weight Defaults

```
[assignment:weights :assignment/capability    0.30 tx:genesis assert]
[assignment:weights :assignment/coupling      0.25 tx:genesis assert]
[assignment:weights :assignment/critical-path 0.20 tx:genesis assert]
[assignment:weights :assignment/historical    0.15 tx:genesis assert]
[assignment:weights :assignment/locality      0.10 tx:genesis assert]
```

Like coupling weights, these are datoms that evolve through the bilateral learning loop.

---

## 6. Hybrid Topologies

### 6.1 Definition

A hybrid topology partitions the agent space into clusters with different sub-topologies:

```
Hybrid(T) = {(cluster_1, T_1), (cluster_2, T_2), ..., (inter_cluster, T_bridge)}
```

Each cluster has:
- A set of agents
- A sub-topology (mesh, star, pipeline, etc.)
- A merge frequency (uniform within cluster)
- A task domain (set of tasks assigned to cluster)

The inter-cluster topology connects clusters through bridge agent(s).

### 6.2 Datom Schema for Hybrid Topologies

```
;; Hybrid topology entity
[hybrid:h1 :hybrid/rationale "Stage 0 build order" tx:100 assert]
[hybrid:h1 :hybrid/created-at 1741500000000 tx:100 assert]

;; Cluster A
[cluster:A :cluster/hybrid hybrid:h1 tx:100 assert]
[cluster:A :cluster/topology :peer-mesh tx:100 assert]
[cluster:A :cluster/agents #{agent:alpha agent:beta} tx:100 assert]
[cluster:A :cluster/merge-frequency :realtime tx:100 assert]
[cluster:A :cluster/task-domain :store tx:100 assert]
[cluster:A :cluster/rationale "store module coupling > 0.8" tx:100 assert]

;; Cluster B
[cluster:B :cluster/hybrid hybrid:h1 tx:100 assert]
[cluster:B :cluster/topology :peer-mesh tx:100 assert]
[cluster:B :cluster/agents #{agent:gamma agent:delta} tx:100 assert]
[cluster:B :cluster/merge-frequency :periodic tx:100 assert]
[cluster:B :cluster/task-domain :query tx:100 assert]

;; Cluster C (solo)
[cluster:C :cluster/hybrid hybrid:h1 tx:100 assert]
[cluster:C :cluster/topology :solo tx:100 assert]
[cluster:C :cluster/agents #{agent:epsilon} tx:100 assert]
[cluster:C :cluster/task-domain :harvest tx:100 assert]

;; Inter-cluster bridge
[hybrid:h1 :hybrid/bridge agent:zeta tx:100 assert]
[hybrid:h1 :hybrid/inter-topology :star tx:100 assert]
[hybrid:h1 :hybrid/inter-frequency :session tx:100 assert]
```

### 6.3 Worked Example: 6 Agents on Braid Stage 0

Using the coupling matrix from 02-coupling-model.md section 6:

```
Cluster A (store kernel): agents {alpha, beta}
  Sub-topology: peer mesh (|cluster| = 2)
  Merge frequency: :realtime (coupling 0.40 > 0.3)
  Rationale: store.rs shared file, INV-STORE -> INV-LAYOUT dependency

Cluster B (query engine): agents {gamma, delta}
  Sub-topology: peer mesh (|cluster| = 2)
  Merge frequency: :high (coupling 0.34 > 0.3)
  Rationale: schema.rs shared file, INV-SCHEMA -> INV-QUERY dependency

Agent epsilon (harvest): solo
  No cluster peer (coupling to all others < 0.3)
  Merge: :session with trunk

Agent zeta (interface): solo + bridge role
  Highest inter-cluster coupling (interface touches all modules)
  Bridge: merges with cluster A (:low) and cluster B (:low) and epsilon (:session)

Inter-cluster: star(zeta), :low frequency
```

### 6.4 Dynamic Evolution

As tasks complete and coupling changes, the topology adapts:

1. Cluster A finishes store kernel -> alpha reassigned to cluster B (query needs help)
2. Coupling between clusters A and B drops -> inter-merge frequency drops to :session
3. New tasks in harvest module -> epsilon's cluster grows, possibly gets a partner

Each adaptation follows the transition protocol (04-transition-protocol.md):
- Adding alpha to cluster B: Tier M (monotonic, adding member) -> instant
- Reducing inter-cluster frequency: Tier NM (non-monotonic, reducing communication) -> barrier

---

## 7. Datom Schema Summary

### 7.1 Entity Types

| Entity Type | Key Attributes | Stage |
|-------------|---------------|-------|
| Agent | program, model, capabilities, status, cluster, frontier | 0 |
| Channel | from, to, type, merge-frequency, active, coupling-score | 0 |
| Cluster | hybrid, topology, agents, merge-frequency, task-domain, rationale | 3 |
| Hybrid | rationale, bridge, inter-topology, inter-frequency, created-at | 3 |
| Task | title, status, assignee, depends-on, touches-file, implements | 0 |
| Reservation | agent, path, exclusive, expires-at | 3 |
| Outcome | topology, quality, conflict-count, merge-overhead-ms, session | 3 |
| Pattern | task-type, optimal-topology, confidence, sample-count | 3 |
| Scaling | action, rationale, authority-score, status, outcome-quality | 3 |

### 7.2 Schema Layer

These entities belong to Schema Layer 5 (Workflow & Task) as defined in spec/02-schema.md.
They extend the existing 31 entity types and ~195 attributes with coordination-specific
entities and attributes.

The schema definitions are themselves datoms (C3: schema-as-data). Adding coordination
attributes is a transaction, not a migration:

```
;; Define :topology/type attribute
[attr:topo-type :db/ident :topology/type tx:schema assert]
[attr:topo-type :db/valueType :db.type/keyword tx:schema assert]
[attr:topo-type :db/cardinality :db.cardinality/one tx:schema assert]
[attr:topo-type :db/resolutionMode :db.resolution/lattice tx:schema assert]
[attr:topo-type :db/doc "Named topology pattern" tx:schema assert]
```

---

## 8. Topology Queries (Datalog)

### 8.1 Current Topology Structure

```datalog
;; All active channels with merge frequencies
[:find ?from ?to ?frequency
 :where
 [?ch :channel/from ?from]
 [?ch :channel/to ?to]
 [?ch :channel/active true]
 [?ch :channel/merge-frequency ?frequency]]
```

### 8.2 Cluster Membership

```datalog
;; Which agents are in which clusters
[:find ?agent ?cluster ?topology
 :where
 [?cluster :cluster/agents ?agents]
 [(contains ?agents ?agent)]
 [?cluster :cluster/topology ?topology]]
```

### 8.3 Recommended Merge Frequency

```datalog
;; Compute recommended merge frequency from coupling
[:find ?agent-a ?agent-b ?recommended-freq
 :where
 [?t1 :task/assignee ?agent-a]
 [?t2 :task/assignee ?agent-b]
 [(!= ?agent-a ?agent-b)]
 [(coupling ?t1 ?t2) ?c]
 [(merge-frequency-for-coupling ?c) ?recommended-freq]]
```

### 8.4 Agents That Should Be in the Same Cluster

```datalog
;; Agents with coupling above threshold should be co-located
[:find ?agent-a ?agent-b ?coupling
 :where
 [?t1 :task/assignee ?agent-a]
 [?t2 :task/assignee ?agent-b]
 [(!= ?agent-a ?agent-b)]
 [(coupling ?t1 ?t2) ?coupling]
 [(> ?coupling 0.3)]]
```

---

## 9. Traceability

| Concept | Traces to |
|---------|-----------|
| Agent as entity in store | spec/01-store.md (everything is a datom) |
| Channel as entity | spec/01-store.md |
| Merge frequency determines information flow | SEED.md S4 (CRDT merge by set union) |
| Coupling-driven frequency selection | INS-005 (task-level regime routing) |
| Hybrid topology with clusters | INS-022 (four topology patterns, regime-dependent) |
| Graph partitioning for clustering | spec/03-query.md query/graph.rs (graph algorithms) |
| Assignment policy with graph metrics | bv (PageRank, betweenness, critical path) |
| Schema Layer 5 for coordination | spec/02-schema.md (six-layer schema architecture) |
| Dynamic evolution via bilateral loop | spec/10-bilateral.md |

---

*Next: `04-transition-protocol.md` — how to safely change from one topology to another.*
