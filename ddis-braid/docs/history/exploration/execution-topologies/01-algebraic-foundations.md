# 01 — Algebraic Foundations

> **Summary:** The topology framework has a clean algebraic structure: coordination state
> forms a sheaf over the agent space, CALM compliance stratifies transitions into
> barrier-free (monotonic) and barrier-required (non-monotonic), topologies form a
> join-semilattice for conflict resolution, and the harvest functor is an adjunction
> between the conversation category and the datom category.

---

## 1. The Sheaf Structure of Coordination State

### 1.1 Definition

A **sheaf** on the agent space assigns coordination state to groups of agents consistently.
In the topology framework, this means:

1. **Local sections**: Each agent alpha's frontier F_alpha is a local view of coordination
   state. Agent alpha knows its own tasks, its merge channels, the coupling scores it has
   computed, and the topology decisions it has received.

2. **Restriction maps**: When agents alpha and beta communicate (merge), their overlapping
   knowledge must be consistent. If alpha asserts [task:1 :task/status :completed tx:100]
   and beta has already received this datom via a different merge path, the datom is
   identical (content-addressed identity, C2). No reconciliation needed.

3. **Gluing axiom**: Local views that agree on overlaps can be uniquely combined into a
   global view. This is guaranteed by set union (MERGE): the global state is the union
   of all local states.

4. **Global section**: A consistent global view of coordination state exists only after
   a sync barrier (INV-SYNC-001) — the consistent cut where all agents have exchanged
   frontiers and non-monotonic queries become valid.

### 1.2 Why CRDT Laws Give Sheaf Conditions for Free

The datom store's algebraic laws (L1-L5 from spec/01-store.md) guarantee the sheaf
conditions automatically:

| CRDT Law | Sheaf Condition It Guarantees |
|----------|-------------------------------|
| L1 (Commutativity): S1 union S2 = S2 union S1 | Restriction maps commute: the order in which agents merge doesn't affect the result |
| L2 (Associativity): (S1 union S2) union S3 = S1 union (S2 union S3) | Gluing is unique: combining three agents' views in any grouping produces the same global view |
| L3 (Idempotency): S union S = S | Sections are consistent: re-merging adds nothing |
| L4 (Monotonicity): S subset-of S union S' | Sections only refine: information only grows |
| L5 (Growth-only): |S(t+1)| >= |S(t)| | Coordination state never shrinks |

### 1.3 Different Topologies as Different Grothendieck Topologies

In the categorical framework, different coordination topologies correspond to different
**Grothendieck topologies** on the same base space (the agent set).

The base space is the set of agents: {alpha, beta, gamma, ...}

A Grothendieck topology J specifies, for each agent, which collections of merge partners
constitute a "covering" — i.e., enough information to reconstruct the global state.

| Coordination Topology | Corresponding Grothendieck Topology |
|----------------------|-------------------------------------|
| Mesh | Every pair is a covering family. Full coverage from any single merge partner (diameter 1). |
| Star(hub) | {hub} is the only covering for non-hub agents. Hub is covered by all spokes. |
| Pipeline(order) | Each agent is covered by its predecessor. Global coverage requires the full chain. |
| Hierarchy(root, levels) | Each agent is covered by its parent. Global coverage requires path to root. |

The **refinement ordering** on Grothendieck topologies gives a partial order on coordination
topologies: mesh is the finest (most covering families), star is coarser, pipeline is coarsest.

---

## 2. CALM Compliance for Coordination

### 2.1 The CALM Theorem Applied to Topology

The CALM theorem (Consistency As Logical Monotonicity) states:
- **Monotonic programs** can be computed without coordination (no barriers needed)
- **Non-monotonic programs** require coordination to produce consistent results

Applied to topology operations:

**Monotonic coordination operations** (safe without barriers):
- Adding a task to the work queue
- Asserting an observation ("I see coupling between store and query modules")
- Claiming a task (asserting assignment)
- Adding a merge channel (new communication link)
- Increasing merge frequency on existing channel
- Adding an agent to a cluster

These only ADD facts. The result is the same regardless of when each agent learns about them.

**Non-monotonic coordination operations** (require barriers):
- Deciding "all tasks are complete" (absence of incomplete tasks)
- Selecting optimal topology from complete coupling data (requires knowing ALL coupling scores)
- Removing a merge channel (negation: channel no longer exists)
- Removing an agent from a cluster (negation: agent no longer member)
- Decreasing merge frequency (comparing to threshold: frequency < old_frequency)

### 2.2 Stratification of Topology Operations

Following the six-stratum classification in spec/03-query.md:

| Stratum | Topology Operations | Coordination Required |
|---------|--------------------|-----------------------|
| 0 (Selection) | Query current topology, list agents, check merge frequency | None |
| 1 (Join) | Compute coupling scores between agent pairs, traverse dependency graph | None |
| 2 (Aggregation) | Compute mean staleness, total merge overhead, utilization balance | Frontier barrier |
| 3 (Negation) | "Which channels are NOT active?", "Which agents have NOT acknowledged?" | Frontier barrier |
| 4 (Recursion) | Transitive coupling (A coupled to B, B coupled to C, therefore A coupled to C) | Frontier barrier |
| 5 (Negation over Recursion) | "No circular dependencies in topology", "All agents reachable" | Full sync barrier |

---

## 3. The Lattice of Topologies

### 3.1 Partial Order on Topologies

Different topologies form a partial order based on **coordination intensity** — the amount
of inter-agent communication they require:

```
Solo <= Star <= Mesh        (mesh is the most connected)
Solo <= Pipeline <= Ring <= Mesh
Solo <= Hierarchy <= Mesh
Hybrid = colimit of component topologies in the category
```

The ordering relation: T1 <= T2 if every merge channel in T1 is also in T2 (T2 has at
least as much communication as T1).

### 3.2 Join-Semilattice Structure

The partial order forms a **join-semilattice**: given any two topologies T1, T2, their
join T1 join T2 is the coarsest topology that refines both.

```
T1 join T2 = topology with channels = channels(T1) union channels(T2)
```

The join always exists because we can always take the union of channel sets. The result
is a valid topology (connected graph, if both inputs were connected and share at least
one agent).

### 3.3 Resolution Mode for Topology Decisions

The lattice structure enables using `resolutionMode = :lattice` for topology attributes:

When two agents independently propose different topologies (concurrent assertions), the
LIVE index resolves to their **join** — the topology that satisfies both requirements.

```
Agent alpha proposes: Star(alpha) for cluster A
Agent beta proposes:  Mesh for cluster A
Resolution (join):   Mesh for cluster A (mesh >= star)
```

This eliminates topology oscillation: instead of LWW (which could flip-flop between
competing proposals), the lattice join always moves toward more connection, which is the
safe default. Explicit reduction in coordination intensity requires an explicit retraction
(which triggers deliberation if commitment weight is high enough).

### 3.4 Meet Operation (Intersection)

The meet T1 meet T2 = topology with channels = channels(T1) intersect channels(T2).

The meet is useful for computing the "minimum viable topology" — the least coordination
needed to satisfy both T1 and T2's requirements. Used during relaxation: start from mesh
(join of all), relax toward the meet with the coupling-optimal topology.

---

## 4. The Harvest Functor as Adjunction

### 4.1 The Two Categories

**Category C (Conversations)**: Objects are conversation logs (JSONL streams, Agent Mail
threads, session transcripts). Morphisms are log extensions (appending messages).

**Category D (Datoms)**: Objects are datom stores (sets of datoms). Morphisms are
transactions (adding datoms via TRANSACT).

### 4.2 The Adjunction F -| U

The **free functor** F: C -> D "harvests" conversations into datoms:
```
F(conversation_log) = {datoms extracted by harvest pipeline}
```

The **forgetful functor** U: D -> C "recalls" datoms as natural language:
```
U(datom_store) = {rendered descriptions of datoms suitable for conversation}
```

The adjunction F -| U means:
- Every conversation can be harvested (F applies to all logs)
- Every datom can be recalled as conversation (U applies to all stores)
- The unit eta: Id -> UF measures information added by harvest (provenance, schema, resolution mode)
- The counit epsilon: FU -> Id measures structure lost by forgetting (the datom's type information, relationships, resolution mode are lost when rendered as prose)

### 4.3 The Epistemic Gap as Unit Measurement

The epistemic gap Delta(t) = K_agent(t) \ K_store(t) is precisely the measure of the
unit eta: how much the agent knows that hasn't been harvested.

For coordination, Delta_coord(t) = coordination knowledge in conversation substrate that
hasn't been harvested into datoms. Includes: informal topology agreements, verbal task
claims, undocumented coupling observations.

Perfect coordination harvest: Delta_coord(t) = empty set at session end.

---

## 5. Resolution Modes for Coordination Attributes

Each coordination attribute uses the resolution mode that matches its semantics:

| Attribute | Resolution Mode | Rationale |
|-----------|----------------|-----------|
| :task/status | LWW (Last Writer Wins) | Task status is a state machine; latest transition is current state |
| :task/assignee | LWW | Latest claim wins; prevents double-assignment race condition |
| :task/priority | Lattice (max) | Priority can only increase (escalation is monotonic) |
| :reservation/paths | Multi-value | All reservations visible; enforcement is advisory |
| :topology/type | Lattice (join) | Concurrent proposals resolve to more coordination (safe default) |
| :topology/merge-frequency | Lattice (max) | Concurrent proposals resolve to higher frequency (safe default) |
| :channel/active | LWW | Latest assertion wins; channel is either active or not |
| :outcome/quality | LWW | Latest measurement supersedes previous |
| :coupling/weight | Lattice (max) | Concurrent coupling observations resolve to higher coupling (conservative) |
| :agent/capabilities | Multi-value | All capabilities visible; agent may have many skills |

### 5.1 Why Conservative Resolution is Correct

For coordination attributes, the conservative direction is always "more coordination":
- Higher coupling weight -> tighter merge schedule
- Higher merge frequency -> more communication
- Topology join -> more channels

Over-coordination wastes merge overhead but never loses data. Under-coordination risks
conflicts, stale state, and knowledge loss. Since the cost function is asymmetric
(conflict_cost >> merge_cost), conservative resolution minimizes maximum regret.

The system relaxes from conservative defaults via explicit retractions, which:
- Carry provenance (who decided to relax, and why)
- Trigger commitment weight check (high commitment -> deliberation)
- Are recorded as datoms (full history of relaxation decisions)

---

## 6. Composition with Existing Braid Algebra

### 6.1 The Topology Framework Uses, Does Not Replace, Existing Algebra

The topology framework is a new _data domain_ on the existing algebraic substrate:

| Braid Component | How Topology Uses It |
|----------------|---------------------|
| Datom store (P(D), union) | Coordination entities are datoms. Same store, same laws. |
| CRDT merge (set union) | Merge topology determines information flow patterns. |
| Per-attribute resolution | Coordination attributes use appropriate resolution modes (see table above). |
| Frontier-relative queries | Each agent queries its local coordination state. |
| Sync barriers (SYNC S8) | Non-monotonic topology decisions use existing barrier protocol. |
| Harvest pipeline (HARVEST S5) | Extended to ingest external coordination logs. |
| Seed assembly (SEED S6) | Extended to project coordination context. |
| R(t) routing function (GUIDANCE S12) | Extended from work-item routing to topology selection. |
| Bilateral loop (BILATERAL S10) | Extended to topology drift detection via F(T). |
| Signal system (SIGNAL S9) | New signal types for topology events (drift, conflict, scaling). |
| Deliberation (DELIBERATION S11) | Topology decisions with high commitment weight use deliberation. |

### 6.2 What's New vs. What's Extended

**New concepts** (not present in current spec):
- Coupling model (five mechanisms, composite signal, learnable weights)
- Coordination topology T = (G, Phi, Sigma, Pi)
- Topology fitness function F(T)
- Scaling authority function A(d) = R(1-C)T
- Cold-start bootstrap algorithm

**Extended concepts** (present in spec, extended to new domain):
- Harvest: now ingests external coordination logs (not just conversation context)
- Seed: now projects coordination context (not just knowledge context)
- R(t): now routes topology selection (not just work-item routing)
- F(S): composed with F(T) into F_total (joint specification + topology fitness)
- Bilateral loop: now includes topology drift detection
- Reconciliation taxonomy: all eight types now explicitly applied to coordination

---

## 7. The Quadrilateral Convergence Model

### 7.1 From Bilateral to Trilateral to Quadrilateral

The Braid spec progression:

- **Bilateral** (spec/10-bilateral.md): Spec <-> Impl convergence
- **Trilateral** (spec/18-trilateral.md): Intent <-> Spec <-> Impl convergence

The topology framework adds a fourth vertex:

```
        Intent
       /      \
      /        \
   Spec ---- Impl
      \        /
       \      /
      Topology
```

### 7.2 Four Bilateral Loops

| Loop | Boundary | Detection | Resolution |
|------|----------|-----------|------------|
| Intent <-> Spec | "Does the spec capture what we want?" | Goal-drift signal | Human review + ADR revision |
| Spec <-> Impl | "Does the impl match the spec?" | Bilateral scan, annotation check | Guided reimplementation |
| Impl <-> Topology | "Is the topology right for how we're building?" | F(T) measurement, coordination drift | Topology adjustment via transition protocol |
| Topology <-> Intent | "Is our coordination approach aligned with our goals?" | Strategic review, outcome quality | Scaling policy revision, human input |

### 7.3 Total System Fitness

```
F_total = lambda * F(S) + (1 - lambda) * F(T)

where lambda = relative importance of spec quality vs coordination quality
```

lambda varies by phase:
- Specification production: lambda ~ 0.8 (spec quality dominates)
- Multi-agent implementation: lambda ~ 0.5 (both matter equally)
- Pure coordination tasks: lambda ~ 0.2 (topology fitness dominates)

lambda is itself a datom, adjustable per phase:
```
[fitness:total :fitness/spec-weight 0.80 tx:genesis assert]  ;; spec phase
[fitness:total :fitness/spec-weight 0.50 tx:impl-start assert]  ;; implementation
```

F_total = 1.0 is the fixpoint where all four vertices are coherent.

---

## 8. Summary of Algebraic Properties

| Property | Structure | Why It Matters |
|----------|-----------|---------------|
| Coordination state is a sheaf | Sheaf on agent space | Local views automatically consistent; global view via sync |
| CALM stratifies transitions | Monotonic/non-monotonic classification | Knows which topology changes need barriers and which don't |
| Topologies form a lattice | Join-semilattice over channel sets | Conflict resolution for competing topology proposals |
| Harvest is an adjunction | F -| U between Conversation and Datom categories | Clean separation of ephemeral substrate from durable store |
| Resolution modes match semantics | Per-attribute in coordination domain | Task status uses LWW; topology uses lattice join; reservations use multi-value |
| Topology framework composes | New data domain on existing algebra | No new algebraic machinery needed; just new entities and attributes |
| Quadrilateral convergence | Four bilateral loops with F_total | Joint optimization of spec quality and coordination effectiveness |

---

*Next: `02-coupling-model.md` — the five coupling mechanisms that drive topology selection.*
