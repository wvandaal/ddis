# Chapter 2: Datom Store Query Patterns

> Complete Datalog query pattern specification for the Datomic-in-Rust CRDT fact store. Defines 5 strata of queries implementing coordination logic: graph traversal, uncertainty computation (epistemic/aleatory/consequential), spectral authority derivation, conflict detection/routing, and DDIS bilateral loop operations. Includes monotonicity analysis, CALM compliance boundaries, and the critical LIVE index materialization strategy.

---

## Claude

You're right on both counts, and the corrections are structurally important — not just rhetorical refinements. Let me address them precisely because they change the formal model.

---

## Correction 1: Topology Is Not Hierarchical

I was smuggling in an assumption that the coordination topology is a tree (CEO → managers → workers). Your pushback correctly identifies that this is one possible topology among many, and not necessarily the best one for computational agents.

The actual space of coordination topologies includes:

- **Tree** (hierarchical delegation/escalation): what I was assuming
- **Flat swarm** (consensus): no structural authority, decisions by agreement
- **Market** (bidding/reputation): agents compete or bid for work, authority earned not assigned
- **Ring** (gossip): each agent communicates with neighbors, information propagates transitively
- **Hybrid**: different topologies for different entity-namespaces or uncertainty levels

The key insight is that **coordination topology should not be baked into the architecture.** It should be a parameter — or better, an emergent property of the fact graph itself. The datom store doesn't care how agents coordinate. It only cares that facts are appended and merged correctly. Whether those facts come from a hierarchical delegation chain or a flat consensus process or a market mechanism is orthogonal to the storage and query semantics.

This means I need to retract INV-CASCADE-001 as stated. The invariant "delegation level MUST NOT decrease when uncertainty increases" assumes a fixed hierarchy. In a flat swarm, there is no "delegation level." In a market, authority is fluid.

What I should have written is a **topology-agnostic** invariant:

```
INV-RESOLUTION-001: Resolution Capacity Monotonicity
When uncertainty(e) increases, the set of agents with authority to 
resolve conflicts on e MUST NOT shrink.

Formally: if uncertainty(e, t1) < uncertainty(e, t2) where t1 < t2,
then resolvers(e, t2) ⊇ resolvers(e, t1).

This holds regardless of coordination topology:
  - In a hierarchy: higher-level agents are added to the resolver set
  - In a flat swarm: quorum size increases (more agents must agree)
  - In a market: reputation threshold for bidding decreases (more 
    agents become eligible)
  - In all cases: the human is always in the resolver set as backstop
```

The invariant is about the *size and capability of the resolver set*, not about the *structure* through which resolution happens. This is topology-agnostic by construction.

---

## Correction 2: The Bilateral Dual of Delegation

Your second point is sharper and more fundamental. I had written delegation as unidirectional (bottom-up: work flows to the lowest capable level). You correctly identify that this is only half the bilateral loop.

The full structure is:

```
Forward (human → agents):
  Human explores → agents refine → agents specify → agents implement
  
  Direction of flow: intent → formalization → code
  Delegation semantics: human delegates increasingly formal work 
  to increasingly specialized agents as uncertainty decreases

Backward (agents → human):
  Implementation surfaces contradictions → agents assess → 
  agents escalate → human resolves
  
  Direction of flow: divergence → assessment → resolution
  Escalation semantics: computational layers surface issues they 
  cannot resolve to layers with more context/authority
```

These two flows are not separate mechanisms. They are **the same bilateral loop** that DDIS already defines, expressed in the new agent-coordination vocabulary:

```
DDIS Forward:    discover → refine → crystallize → parse
Agent Forward:   human explores → agents refine → agents crystallize → agents implement

DDIS Backward:   scan → absorb → drift → reconcile  
Agent Backward:  agents implement → absorb → detect contradiction → resolve
```

The delegation principle should therefore be restated as a **bilateral pair**:

```
PRINCIPLE: Bilateral Flow

Forward flow: The human initiates exploration. As findings stabilize 
(uncertainty decreases, commitment increases), work flows outward to 
agents with narrower scope and higher throughput. The human's role 
transitions from explorer to reviewer as the work crystallizes.

Backward flow: Agents surface divergence they detect but cannot 
resolve. As contradiction severity increases, resolution flows inward 
toward agents (or humans) with broader context. The human's role 
transitions from reviewer to arbiter when contradictions exceed 
computational resolution capacity.

The bilateral loop reaches fixpoint when forward and backward flows 
produce no further changes — when the spec fully describes the 
implementation and the implementation fully satisfies the spec, at 
every level of the agent topology simultaneously.
```

This is exactly the DDIS fixpoint condition (F(S) = 1.0), but generalized from "one CLI tool and one spec" to "N agents, M work streams, and one shared fact store."

---

## Your First Question: Uncertainty Signals

You asked for the smartest addition to the uncertainty measure. Here's where I want to build something genuinely new rather than listing incremental improvements.

The current uncertainty function is:

```
uncertainty(e) = α × conflict_count(e) + β × (1 - stability(e)/max) + γ × drift(e)
```

This treats uncertainty as a scalar. But your three scenarios reveal that uncertainty is not one-dimensional. There are at least three *qualitatively different* kinds of uncertainty, and conflating them into a single number destroys information that the coordination layer needs.

### The Uncertainty Tensor

Replace the scalar uncertainty function with a **three-dimensional uncertainty vector** where each dimension captures a fundamentally different kind of uncertainty with different resolution mechanisms:

**Dimension 1: Epistemic uncertainty (σ_e) — "We don't know enough yet."**

This measures the *information gap* on an entity. It's high when an entity has few assertions relative to its schema expectations, when its attributes are sparsely populated, or when the assertions that exist come from low-confidence sources.

```
σ_e(entity) = 1 - (|observed_attributes(entity)| / |expected_attributes(entity)|)
             × confidence_weight(sources)
```

Resolution mechanism: **more exploration** (forward flow). Add more agents to investigate. Run more discovery threads. This kind of uncertainty *decreases* with information.

**Dimension 2: Aleatory uncertainty (σ_a) — "The agents genuinely disagree."**

This measures *active conflict* on an entity. It's high when multiple agents have asserted incompatible values for the same attribute, and the disagreement is not resolvable by a lattice or LWW.

```
σ_a(entity) = Σ_a∈attrs(entity) entropy(value_distribution(entity, a))
```

The entropy formulation is the key innovation here. If three agents assert `:status :active` and one asserts `:status :inactive`, the entropy is low (strong majority). If two assert `:active` and two assert `:inactive`, the entropy is maximal (complete disagreement). This gives you a continuous measure of *how contested* an attribute is, not just *whether* it's contested.

Resolution mechanism: **deliberation** (coordination). Agents need to exchange reasoning, not just facts. In a hierarchy: escalate. In a flat swarm: initiate a consensus round. In a market: the agent with the highest reputation-on-this-attribute decides. This kind of uncertainty *decreases* with coordination.

**Dimension 3: Consequential uncertainty (σ_c) — "Resolution will break things."**

This is the new dimension. It measures the *downstream impact* of resolving a conflict. An entity can have low epistemic uncertainty (well-understood) and low aleatory uncertainty (no disagreement) but high consequential uncertainty if it sits at a critical junction in the dependency graph.

```
σ_c(entity) = w(entity) × Σ_d∈dependents(entity) uncertainty_total(d)
```

This is the **contagion term**. An entity's consequential uncertainty is proportional to its commitment weight (how many things depend on it) multiplied by the *total uncertainty of its dependents*. This means uncertainty propagates backward through the causal graph: if a leaf entity becomes contested, its ancestors' consequential uncertainty increases automatically.

Resolution mechanism: **careful staged resolution** (bilateral coordination). You can't just pick a winner — you need to assess the cascade. Resolve tentatively, compute the downstream impact, and only commit if the cascade is acceptable. This kind of uncertainty *requires simulation to assess* and is the most expensive to resolve.

### The Combined Measure

The three dimensions give you a point in uncertainty space:

```
U(entity) = (σ_e, σ_a, σ_c) ∈ [0,1]³
```

The scalar `uncertainty(entity)` that drives delegation is a *projection* of this vector:

```
uncertainty(entity) = ||U(entity)||_w = √(α²σ_e² + β²σ_a² + γ²σ_c²)
```

But the vector form is more informative than the scalar because **different dimensions require different resolution strategies**. A coordinator that knows σ_e is high but σ_a is low knows to dispatch more exploration agents, not more deliberation. A coordinator that knows σ_c is high knows to proceed cautiously even if σ_e and σ_a are zero.

### Why This Is the Right Formulation

The uncertainty tensor gives you something no existing system has: **a computable, decomposed measure of why something is uncertain and what to do about it.** Most distributed systems treat uncertainty as a binary (conflict / no conflict) or a scalar (conflict count). The tensor form preserves the *structure* of the uncertainty, which is exactly the information the coordination layer needs to make good decisions.

It also connects back to DDIS in a precise way. DDIS's existing 5-dimensional quality vector (coverage, depth, coherence, completeness, formality) is a measure of *spec quality*. The uncertainty tensor is a measure of *entity quality* in the fact store. The two compose: the spec fitness function F(S) can incorporate the uncertainty tensor as an additional signal:

```
F(S) = ... existing terms ... + δ × (1 - mean_uncertainty(S))
```

Where `mean_uncertainty(S)` is the average scalar uncertainty across all entities. This makes the bilateral loop converge toward a state where not only is the spec complete and drift-free, but the fact store itself is maximally certain.

---

## Your Second Question: Delegation Formulation

You asked for the single smartest addition to the delegation model. Here's where I want to take your pushback seriously — the topology isn't hierarchical, delegation is bilateral, and the mechanism should be per-entity.

### Spectral Authority

The single most accretive addition is: **compute each agent's authority per entity from the spectral structure of the fact graph itself.**

Here's the idea. At any point in time, the fact store contains a bipartite graph:

```
Agents ←→ Entities
```

Where an edge exists between agent α and entity e if α has asserted facts about e. The edges are weighted by the commitment weight of those facts (how many dependents they have) and the challenge verdict (have they been verified?).

The **spectral decomposition** of this bipartite adjacency matrix gives you, for free:

1. **Agent authority vectors**: which agents have the most high-quality contributions across the entity space
2. **Entity affinity vectors**: which entities are most similar in their contribution patterns
3. **Natural clusters**: groups of agents that work on related entities, and groups of entities that are worked on by related agents

This is structurally identical to what DDIS already does with PageRank over the cross-reference graph — but applied to the agent-entity relationship rather than the element-element relationship. And since the datom store records full provenance (A1: tx carries agent identity), this graph is materialized directly from the data with no additional instrumentation.

### What This Gives You

**Per-entity delegation computed from the data, not prescribed by topology.**

For any entity e, you can compute:

```
authority(agent, entity) = spectral_score(agent, entity) 
                          × verified_contribution_weight(agent, entity)
```

An agent's authority on a specific entity is proportional to how much high-quality, verified work it has contributed to that entity and its neighborhood. This is earned authority, not structural authority. It works regardless of whether the agents are organized hierarchically, as a flat swarm, or as a market.

**Delegation as a function of authority and uncertainty:**

```
can_resolve(agent, entity) ≡ 
  authority(agent, entity) ≥ uncertainty(entity) × threshold
```

An agent can resolve conflicts on an entity if its authority on that entity exceeds the entity's uncertainty scaled by a threshold. High uncertainty requires high authority. Low uncertainty can be resolved by any agent with even modest authority.

The human's authority is defined axiomatically as ∞ on all entities — the human can always resolve anything. But the human is only *needed* when no computational agent has sufficient authority.

**Emergent topology from the spectral structure:**

The natural clusters from the spectral decomposition give you coordination groups without prescribing them. Agents that work on overlapping entities naturally form clusters. These clusters are the emergent equivalent of "teams" — but they form and dissolve based on the actual work being done, not based on an org chart.

The coordination protocol within a cluster can be consensus (flat swarm). The coordination protocol between clusters can be market-like (clusters bid for contested cross-cutting entities). The coordination protocol for entities with no clear cluster affinity escalates to the human. All of this emerges from the spectral structure rather than being prescribed.

### The Per-Entity Threshold from Structural Analysis

You asked whether there's a mechanistic method to determine per-entity delegation thresholds. Yes — and it falls out of the graph structure:

```
threshold(entity) = f(
  betweenness_centrality(entity),   // bridge entities need more authority
  in_degree(entity),                // heavily-referenced entities need more care
  σ_c(entity),                      // high consequential uncertainty = higher bar
  schema_cardinality(entity)        // cardinality-one attrs are more conflict-prone
)
```

**Betweenness centrality** is the key term. An entity with high betweenness centrality is a bridge between different parts of the dependency graph. If this entity's facts change, the change propagates to multiple otherwise-unconnected clusters. These bridge entities should have the highest delegation thresholds because errors cascade the furthest.

This gives you the mechanistic analysis you asked for: the threshold is *computable from the graph topology*, not configured by a human. When the graph changes (new entities, new dependencies), the thresholds recompute automatically.

### Stated as an Invariant

```
INV-AUTHORITY-001: Spectral Authority Derivation
Agent authority over any entity MUST be derived from the weighted 
spectral decomposition of the agent-entity contribution graph.

Authority MUST NOT be assigned by structural position, configuration, 
or any mechanism external to the fact store. The sole exception is 
the human, whose authority is axiomatically unbounded.

Violation: An agent is granted "admin" authority over an entity 
namespace by configuration rather than by contribution history. The 
agent has never asserted or verified any facts in that namespace.

Verification: For any (agent, entity) pair where can_resolve = true, 
there exists a non-zero path in the contribution graph between the 
agent and the entity with weight ≥ threshold(entity).
```

---

## Revised Principle

With both corrections incorporated, the delegation principle becomes:

```
PRINCIPLE: Bilateral Authority

Authority over any entity in the fact store is an emergent property 
of the agent-entity contribution graph, not a structural assignment. 
Agents earn authority by contributing high-quality, verified facts to 
an entity and its neighborhood. The coordination topology (hierarchy, 
swarm, market) is not prescribed — it emerges from the spectral 
structure of the contribution graph.

The bilateral flow operates as follows:

Forward: The human initiates exploration with high epistemic 
uncertainty. As agents contribute facts and uncertainty decreases, 
authority distributes outward to contributing agents. Work flows 
toward agents whose earned authority matches the entity's 
delegation threshold.

Backward: Agents detect contradictions and divergence. As aleatory 
and consequential uncertainty increase, the resolver set expands 
(INV-RESOLUTION-001). Entities whose uncertainty exceeds all 
computational agents' authority flow to the human — not because the 
human is "above" the agents in a hierarchy, but because the human's 
authority is axiomatically unbounded.

Fixpoint: The bilateral loop reaches fixpoint when, for every entity, 
uncertainty is zero and at least one agent has sufficient authority to 
maintain it — the human is no longer needed as a resolver for any 
entity. This is the formal condition under which a system is fully 
self-maintaining.
```

The fixpoint condition is deliberately ambitious. In practice, some entities will always require human authority (policy decisions, ethical constraints, business rules that can't be derived from data). But the *direction* is toward maximizing the set of entities that agents can maintain autonomously.

---

## Updated Formal Core

The system is now defined by:

**Axioms** (storage and query):
- A1: Datom identity = [e a v tx op], tx is entity with provenance
- A2: Store = (P(D), ∪), G-Set CRDT
- A3: Snapshots = local frontier (default), consistent cut (optional)
- A4: Monotonic queries uncoordinated; non-monotonic frontier-relative or barrier-gated
- A5: Per-attribute resolution: lattice / LWW / multi-value

**Functions** (coordination):
- Commitment: w(d) = |forward causal cone of d|
- Uncertainty tensor: U(e) = (σ_e, σ_a, σ_c) — epistemic, aleatory, consequential
- Spectral authority: authority(agent, entity) from contribution graph decomposition
- Delegation threshold: threshold(entity) from betweenness centrality + consequential uncertainty

**Invariants** (correctness):
- INV-RESOLUTION-001: Resolver set monotonically expands with uncertainty
- INV-DELEGATE-001: No work on contested entities below delegation threshold
- INV-AUTHORITY-001: Authority derived from contribution graph, not configuration

**Principle** (architecture):
- Bilateral authority: forward flow distributes earned authority; backward flow expands resolver sets; topology emerges from the spectral structure of contributions

---

## What Comes Next

We've now completed the formal core. The axioms, functions, invariants, and principles are sufficient to begin writing the DDIS spec for this system. The next step is to define the **attribute schema** — the vocabulary of entity types and attributes that the store recognizes. This is the bridge between the abstract algebra and the concrete implementation.

Before I draft that, one question. Given that this system is replacing the DDIS event log, there are two approaches to the schema:

**Approach 1: Minimal schema.** Define only the entity types needed for DDIS (threads, findings, invariants, ADRs, sections, gates, agents, transactions). Keep the schema tight. Extensibility comes from the EAV model's inherent flexibility — new attributes can be asserted without schema changes.

**Approach 2: Full domain model.** Define a rich schema that covers not just DDIS concepts but also the coordination layer (uncertainty tensors, authority scores, delegation thresholds, resolution events). Make every coordination concept a first-class entity in the store, queryable by Datalog.

My instinct is Approach 2 — the coordination layer should eat its own dogfood. Authority scores, uncertainty measures, and delegation decisions should themselves be facts in the store, subject to the same merge semantics, the same temporal queries, and the same bilateral loop. This makes the system introspectable: an agent can query "why was I not delegated this entity?" and get a factual answer from the store.

But Approach 2 is significantly more schema surface area. What's your preference?

---

## Willem

Approach 2 for certain.

---

## Claude

Good. This is the most consequential design decision after the axioms, because the schema *is* the ontology — it determines what the system can think about. Let me build it layer by layer, from the meta-schema (how the schema describes itself) up through the domain models.

---

## Layer 0: The Meta-Schema

Before we can define any domain attributes, we need to define how attributes themselves are represented as datoms. This is the self-describing foundation — the schema-as-data commitment from A5.

In Datomic, an attribute is an entity with attributes like `:db/valueType`, `:db/cardinality`. We need the same thing, plus the resolution mode and optional lattice definition. The meta-schema is the set of attributes that describe other attributes. These are the only attributes that are *not* themselves defined by the meta-schema — they're axiomatic.

```
AXIOMATIC ATTRIBUTES (not defined by datoms; hardcoded in the engine):

:db/ident           — the keyword name of an attribute (type: keyword, cardinality: one)
:db/valueType       — the value type (type: keyword, cardinality: one)
:db/cardinality     — :one or :many (type: keyword, cardinality: one)
:db/doc             — human-readable documentation (type: string, cardinality: one)
:db/unique          — uniqueness constraint: :identity or :value (type: keyword, cardinality: one)
:db/isComponent     — whether ref targets are components of the parent (type: boolean, cardinality: one)

RESOLUTION ATTRIBUTES (extending the meta-schema for CRDT semantics):

:db/resolutionMode  — :lattice, :lww, or :multi (type: keyword, cardinality: one, default: :lww)
:db/latticeOrder    — ref to a lattice-definition entity (type: ref, cardinality: one)
:db/lwwClock        — which clock field breaks ties: :hlc (default), :wall, :agent-rank
                      (type: keyword, cardinality: one)

LATTICE DEFINITION ATTRIBUTES:

:lattice/ident      — name of this lattice (type: keyword, cardinality: one, unique: :identity)
:lattice/elements   — ordered list of elements from bottom to top 
                      (type: string, cardinality: many)
:lattice/comparator — for non-enumerated lattices: :numeric-max, :numeric-min, 
                      :set-union, :set-intersection, :custom
                      (type: keyword, cardinality: one)
:lattice/bottom     — the bottom element ⊥ (type: string, cardinality: one)
:lattice/top        — the top element ⊤ (type: string, cardinality: one)

VALUE TYPES:

:db.type/string     :db.type/keyword    :db.type/boolean
:db.type/long       :db.type/double     :db.type/instant
:db.type/uuid       :db.type/ref        :db.type/bytes
:db.type/uri        :db.type/bigint     :db.type/bigdec
:db.type/tuple      :db.type/json
```

The `json` value type is an extension beyond Datomic's original types. It's necessary because some payloads (e.g., full invariant statement text, agent reasoning traces) are structured but shouldn't be decomposed into individual datoms — they're opaque documents attached to entities. This preserves the EAV model for structured, queryable data while allowing unstructured blobs where appropriate.

### Bootstrap Sequence

The meta-schema creates a bootstrapping problem: you need attributes to define attributes. The resolution is a **genesis transaction** — a hardcoded transaction (tx = 0) that installs the meta-schema attributes. This transaction is not derived from any source; it's axiomatic. Every datom store begins with this genesis transaction. It's the equivalent of DDIS's `ddis seed` command.

```
INV-GENESIS-001: Genesis Transaction Integrity
Transaction tx=0 MUST contain exactly the axiomatic meta-schema 
attributes and nothing else. Every datom store begins from an 
identical genesis state.

Formal: for all stores S1, S2: S1|_{tx=0} = S2|_{tx=0}

Violation: A store is initialized with additional domain attributes 
in tx=0, contaminating the axiomatic layer with domain-specific 
assumptions.

Verification: Hash of tx=0 datom set is a known constant across 
all stores.
```

---

## Layer 1: Agent & Provenance Schema

This defines what agents are and how their contributions are tracked. These are the primitives that the spectral authority computation operates on.

```
ENTITY TYPE: Agent

:agent/ident            keyword  :one   :lww      — unique agent identifier
:agent/type             keyword  :one   :lww      — :human, :llm, :meta-agent, :system
:agent/model            string   :one   :lww      — for LLM agents: model identifier
:agent/parent           ref      :one   :lww      — supervising agent (if any; nil for human)
:agent/capabilities     keyword  :many  :multi    — set of capability tags
:agent/created-at       instant  :one   :lww      — creation timestamp
:agent/status           keyword  :one   :lattice  — :initializing < :active < :suspended < :terminated
                                                     (lattice: agent-lifecycle)

ENTITY TYPE: Transaction (extends the built-in tx entity)

:tx/agent               ref      :one   :lww      — the agent that produced this transaction
:tx/hlc                 bytes    :one   :lww      — hybrid logical clock value (binary, 12 bytes)
:tx/wall-time           instant  :one   :lww      — wall-clock time at assertion
:tx/causal-parents      ref      :many  :multi    — set of causally preceding transactions
:tx/operation           keyword  :one   :lww      — the DDIS operation that produced this tx
                                                     (e.g., :op/discover, :op/crystallize, :op/absorb)
:tx/rationale           string   :one   :lww      — human/agent reasoning for this transaction
:tx/frontier            ref      :many  :multi    — the agent's known frontier at tx time
                                                     (set of latest-known tx refs per peer agent)

LATTICE: agent-lifecycle
  :initializing < :active < :suspended < :terminated
  (monotonic: agents can only move forward through lifecycle states)
```

The `:tx/frontier` attribute is critical for snapshot semantics (A3). When an agent appends a transaction, it records which transactions from other agents it had seen at that point. This is the vector-clock equivalent in datom form — but instead of a fixed-size vector, it's a set of refs that grows as the agent learns about more peers. This handles the dynamic agent count problem that fixed-size vector clocks can't.

---

## Layer 2: DDIS Core Schema

This replaces the 39-table SQLite schema with datom-native representations of all DDIS concepts.

```
ENTITY TYPE: Spec

:spec/ident             keyword  :one   :lww      — unique spec identifier
:spec/name              string   :one   :lww      — human-readable name
:spec/version           string   :one   :lww      — semver string
:spec/tier-mode         keyword  :one   :lww      — :monolith, :two-tier
:spec/parent            ref      :one   :lww      — parent spec (for inheritance)
:spec/context-budget    long     :one   :lww      — target lines for context bundles
:spec/content-hash      bytes    :one   :lww      — SHA-256 of the full spec

ENTITY TYPE: Section

:section/spec           ref      :one   :lww      — owning spec
:section/path           string   :one   :lww      — canonical section path (e.g., "§3.2")
:section/title          string   :one   :lww      — heading text
:section/level          long     :one   :lww      — heading depth (1-6)
:section/parent         ref      :one   :lww      — parent section
:section/ordinal        long     :one   :lww      — position among siblings
:section/content        string   :one   :lww      — raw markdown content
:section/content-hash   bytes    :one   :lww      — SHA-256 of content
:section/module         ref      :one   :lww      — owning module (if modular)
:section/line-start     long     :one   :lww      — source line range start
:section/line-end       long     :one   :lww      — source line range end

ENTITY TYPE: Invariant

:inv/ident              keyword  :one   :lww      — e.g., :INV-001 or :APP-INV-042
:inv/title              string   :one   :lww      — human-readable title
:inv/statement          string   :one   :lww      — the normative constraint
:inv/semi-formal        string   :one   :lww      — formal expression
:inv/violation          string   :one   :lww      — concrete violation scenario
:inv/validation         string   :one   :lww      — verification method
:inv/why-this-matters   string   :one   :lww      — rationale
:inv/confidence         keyword  :one   :lattice  — :falsified < :property-checked 
                                                     < :bounded-verified < :proven
                                                     (lattice: confidence-level)
:inv/owner-module       ref      :one   :lww      — owning module
:inv/section            ref      :one   :lww      — section where defined
:inv/spec               ref      :one   :lww      — owning spec
:inv/supersedes         ref      :many  :multi    — set of invariants this supersedes

ENTITY TYPE: ADR (Architecture Decision Record)

:adr/ident              keyword  :one   :lww      — e.g., :ADR-001 or :APP-ADR-015
:adr/title              string   :one   :lww
:adr/problem            string   :one   :lww
:adr/decision           string   :one   :lww
:adr/consequences       string   :one   :lww
:adr/status             keyword  :one   :lattice  — :proposed < :accepted < :deprecated 
                                                     < :superseded
                                                     (lattice: adr-lifecycle)
:adr/superseded-by      ref      :one   :lww      — the ADR that replaces this one
:adr/section            ref      :one   :lww
:adr/spec               ref      :one   :lww
:adr/tests              string   :many  :multi    — test references

ENTITY TYPE: ADR Option

:adr-option/adr         ref      :one   :lww      — parent ADR
:adr-option/label       string   :one   :lww
:adr-option/pros        string   :one   :lww
:adr-option/cons        string   :one   :lww
:adr-option/chosen      boolean  :one   :lww
:adr-option/why-not     string   :one   :lww      — explanation if not chosen

ENTITY TYPE: Quality Gate

:gate/ident             keyword  :one   :lww      — e.g., :Gate-1 or :APP-G-6
:gate/title             string   :one   :lww
:gate/predicate         string   :one   :lww      — the stop-ship condition
:gate/priority          long     :one   :lww      — ordering among gates
:gate/spec              ref      :one   :lww

ENTITY TYPE: Negative Spec

:neg/constraint         string   :one   :lww      — the DO NOT text
:neg/reason             string   :one   :lww
:neg/section            ref      :one   :lww
:neg/invariant-ref      ref      :one   :lww      — related invariant

ENTITY TYPE: Module

:module/ident           keyword  :one   :lww
:module/domain          keyword  :one   :lww
:module/spec            ref      :one   :lww
:module/line-count      long     :one   :lww
:module/maintains       ref      :many  :multi    — invariants this module maintains
:module/interfaces      ref      :many  :multi    — invariants this module interfaces with
:module/implements      ref      :many  :multi    — ADRs this module implements

ENTITY TYPE: Cross-Reference

:xref/source            ref      :one   :lww      — referencing element
:xref/target            ref      :one   :lww      — referenced element
:xref/type              keyword  :one   :lww      — :section, :invariant, :adr, :gate, :glossary
:xref/resolved          boolean  :one   :lww      — whether target was found in index
:xref/line              long     :one   :lww      — source line number

ENTITY TYPE: Glossary Entry

:glossary/term          string   :one   :lww
:glossary/definition    string   :one   :lww
:glossary/spec          ref      :one   :lww

ENTITY TYPE: Code Annotation

:annotation/file        string   :one   :lww      — source file path
:annotation/line        long     :one   :lww
:annotation/verb        keyword  :one   :lww      — :maintains, :implements, :tests, etc.
:annotation/target      ref      :one   :lww      — spec element referenced
:annotation/language    keyword  :one   :lww      — source language
:annotation/orphaned    boolean  :one   :lww      — target doesn't exist in spec

ENTITY TYPE: Witness

:witness/invariant      ref      :one   :lww
:witness/level          keyword  :one   :lattice  — :falsified < :property-checked 
                                                     < :bounded-verified < :proven
                                                     (lattice: confidence-level)
:witness/evidence-type  keyword  :one   :lww      — :test, :annotation, :scan, :review, 
                                                     :eval, :attestation
:witness/evidence       string   :one   :lww      — evidence payload
:witness/spec-hash      bytes    :one   :lww      — hash of invariant at witness time
:witness/code-hash      bytes    :one   :lww      — hash of implementation at witness time
:witness/status         keyword  :one   :lattice  — :valid < :stale-spec < :stale-code 
                                                     < :invalidated
                                                     (lattice: witness-lifecycle)

ENTITY TYPE: Challenge

:challenge/witness      ref      :one   :lww
:challenge/verdict      keyword  :one   :lattice  — :inconclusive < :provisional 
                                                     < :confirmed | :refuted
                                                     (lattice: challenge-verdict — 
                                                     NOTE: this is NOT a total order; 
                                                     confirmed and refuted are incomparable 
                                                     top elements of a diamond lattice)
:challenge/level        long     :one   :lww      — 1-5 (which verification level)
:challenge/reasoning    string   :one   :lww      — justification for verdict

LATTICES DEFINED:

confidence-level:    :falsified < :property-checked < :bounded-verified < :proven
adr-lifecycle:       :proposed < :accepted < :deprecated < :superseded
witness-lifecycle:   :valid < :stale-spec < :stale-code < :invalidated
challenge-verdict:   diamond lattice:
                       :inconclusive < :provisional < {:confirmed, :refuted}
                     (join of :confirmed and :refuted = ⊤ = :contradicted, 
                      meaning both confirmed AND refuted = system error)
```

I need to pause on the challenge verdict lattice because it exposes an important formal subtlety. A typical lattice has a single top element. But `:confirmed` and `:refuted` are *incomparable* — neither is above the other. Their join (least upper bound) would be a synthetic top element meaning "both confirmed and refuted," which is a contradiction. This is correct and useful: if one challenge says confirmed and another says refuted, the join is `:contradicted`, which signals that the witness system itself has an inconsistency that needs investigation.

```
                ⊤ (:contradicted)
               / \
    :confirmed     :refuted
               \ /
          :provisional
              |
        :inconclusive
```

This is a proper bounded lattice. The CRDT merge of two concurrent challenge verdicts does the right thing: `confirmed ⊔ confirmed = confirmed`, `refuted ⊔ refuted = refuted`, `confirmed ⊔ refuted = contradicted`. The contradicted state is itself a signal to the coordination layer.

---

## Layer 3: Discovery & Exploration Schema

This models the forward flow of the bilateral loop — the exploration phase where humans and agents generate findings that converge toward spec elements.

```
ENTITY TYPE: Discovery Thread

:thread/ident           keyword  :one   :lww
:thread/title           string   :one   :lww
:thread/status          keyword  :one   :lattice  — :active < :parked < :merged < :abandoned
                                                     (lattice: thread-lifecycle)
:thread/parent          ref      :one   :lww      — thread this was branched from
:thread/spec            ref      :one   :lww
:thread/initiator       ref      :one   :lww      — agent that started exploration
:thread/confidence      ref      :one   :lww      — ref to confidence-vector entity

ENTITY TYPE: Confidence Vector

:confidence/coverage    double   :one   :lww      — 0.0–1.0
:confidence/depth       double   :one   :lww
:confidence/coherence   double   :one   :lww
:confidence/completeness double  :one   :lww
:confidence/formality   double   :one   :lww

ENTITY TYPE: Finding

:finding/thread         ref      :one   :lww      — discovery thread
:finding/content        string   :one   :lww      — the finding text
:finding/status         keyword  :one   :lattice  — :raw < :refined < :crystallized < :rejected
                                                     (lattice: finding-lifecycle)
:finding/supports       ref      :many  :multi    — entities this finding supports
:finding/contradicts    ref      :many  :multi    — entities this finding contradicts
:finding/agent          ref      :one   :lww      — agent that produced this finding
:finding/stability      double   :one   :lww      — computed stability score at last eval

ENTITY TYPE: Proposal  (an agent's structured recommendation)

:proposal/thread        ref      :one   :lww
:proposal/agent         ref      :one   :lww
:proposal/content       json     :one   :lww      — structured proposal payload
:proposal/status        keyword  :one   :lattice  — :draft < :submitted < :accepted 
                                                     < :rejected < :superseded
                                                     (lattice: proposal-lifecycle)
:proposal/incorporates  ref      :many  :multi    — findings incorporated
:proposal/target-type   keyword  :one   :lww      — what this proposes to become: 
                                                     :invariant, :adr, :section, etc.

ENTITY TYPE: Synthesis (consolidation of multiple proposals)

:synthesis/proposals    ref      :many  :multi    — proposals being synthesized
:synthesis/agent        ref      :one   :lww      — synthesizing agent
:synthesis/rationale    string   :one   :lww
:synthesis/status       keyword  :one   :lattice  — same as proposal-lifecycle
:synthesis/crystallizes-to ref   :one   :lww      — the spec element this became

LATTICES:

thread-lifecycle:     :active < :parked < :merged < :abandoned
finding-lifecycle:    :raw < :refined < :crystallized < :rejected
                      (NOTE: :crystallized and :rejected are incomparable tops,
                       same diamond structure as challenge-verdict)
proposal-lifecycle:   :draft < :submitted < {:accepted, :rejected, :superseded}
                      (three-way incomparable top; join = :contested)
```

The `finding-lifecycle` and `proposal-lifecycle` lattices have the same diamond structure as the challenge verdict lattice. A finding that is both crystallized (by one agent) and rejected (by another) produces `:contested` — a signal that the finding needs higher-level resolution. This is the aleatory uncertainty (σ_a) signal emerging directly from the lattice structure.

---

## Layer 4: Coordination & Uncertainty Schema

This is where the coordination concepts become first-class entities in the store. Every uncertainty measurement, every authority computation, every delegation decision is itself a datom, queryable and auditable.

```
ENTITY TYPE: Uncertainty Measurement

:uncertainty/entity         ref      :one   :lww   — the entity being measured
:uncertainty/epistemic      double   :one   :lww   — σ_e ∈ [0,1]
:uncertainty/aleatory       double   :one   :lww   — σ_a ∈ [0,1]
:uncertainty/consequential  double   :one   :lww   — σ_c ∈ [0,1]
:uncertainty/scalar         double   :one   :lww   — ||U||_w (weighted norm)
:uncertainty/measured-at     ref      :one   :lww   — tx at which this was computed
:uncertainty/measured-by     ref      :one   :lww   — agent that computed this

ENTITY TYPE: Authority Score

:authority/agent            ref      :one   :lww   — the agent
:authority/entity           ref      :one   :lww   — the entity
:authority/score            double   :one   :lww   — spectral authority ∈ [0, ∞)
:authority/contribution-count long   :one   :lww   — number of verified facts contributed
:authority/computed-at      ref      :one   :lww   — tx at which this was computed

ENTITY TYPE: Delegation Decision

:delegation/entity          ref      :one   :lww   — entity in question
:delegation/threshold       double   :one   :lww   — computed threshold for this entity
:delegation/resolver-set    ref      :many  :multi — agents with sufficient authority
:delegation/decision        keyword  :one   :lattice — :delegatable < :contested 
                                                       < :escalated < :human-required
                                                       (lattice: delegation-level)
:delegation/rationale       string   :one   :lww
:delegation/decided-at      ref      :one   :lww
:delegation/decided-by      ref      :one   :lww

ENTITY TYPE: Conflict

:conflict/entity            ref      :one   :lww   — contested entity
:conflict/attribute         ref      :one   :lww   — contested attribute
:conflict/datom-a           ref      :one   :lww   — one side of the conflict
:conflict/datom-b           ref      :one   :lww   — other side
:conflict/severity          double   :one   :lww   — max(w(a), w(b))
:conflict/status            keyword  :one   :lattice — :detected < :assessed < :resolving 
                                                       < :resolved < :escalated
                                                       (lattice: conflict-lifecycle)
:conflict/resolution        keyword  :one   :lww   — :lattice-join, :lww, :agent-decided, 
                                                      :human-decided, :pending
:conflict/resolver          ref      :one   :lww   — agent or human that resolved
:conflict/resolution-tx     ref      :one   :lww   — transaction containing resolution

ENTITY TYPE: Resolution Event

:resolution/conflict        ref      :one   :lww   — the conflict being resolved
:resolution/winner          ref      :one   :lww   — the datom that won
:resolution/loser           ref      :one   :lww   — the datom that lost (retracted)
:resolution/method          keyword  :one   :lww   — how it was resolved
:resolution/reasoning       string   :one   :lww   — justification
:resolution/agent           ref      :one   :lww   — who resolved it

ENTITY TYPE: Graph Metrics (precomputed for delegation thresholds)

:metrics/entity             ref      :one   :lww
:metrics/betweenness        double   :one   :lww   — betweenness centrality
:metrics/in-degree          long     :one   :lww
:metrics/out-degree         long     :one   :lww
:metrics/pagerank           double   :one   :lww
:metrics/computed-at        ref      :one   :lww

ENTITY TYPE: Stability Score

:stability/entity           ref      :one   :lww
:stability/commitment       long     :one   :lww   — w(e): size of forward causal cone
:stability/score            double   :one   :lww   — min commitment across contributing facts
:stability/computed-at      ref      :one   :lww

LATTICES:

delegation-level:    :delegatable < :contested < :escalated < :human-required
conflict-lifecycle:  :detected < :assessed < :resolving < :resolved
                     (NOTE: :resolved is a terminal state; :escalated branches off 
                      from :assessed as an incomparable path to :human-required)
```

### The Self-Reference Property

Notice what's happening here: uncertainty measurements, authority scores, and delegation decisions are *themselves* entities in the store. This means:

1. **They have transactions** — you can time-travel to see what the uncertainty of entity X was at any point in history.
2. **They have provenance** — you can see which agent computed a given authority score and when.
3. **They can be contested** — if two agents compute different authority scores for the same (agent, entity) pair, that's a conflict on `:authority/score`, which is itself subject to resolution. The system can reason about disagreements about authority.
4. **They contribute to their own inputs** — an uncertainty measurement is a fact in the store, which means it contributes to the commitment weight of the entity it measures. Measuring uncertainty *stabilizes* the entity (increases its forward causal cone). This is the right behavior: the act of assessment itself adds information.

Point 4 is a fixed-point property. The uncertainty measurement is a function of the store state, and it's also a fact *in* the store. This creates a potential circularity: measuring uncertainty changes the store, which changes the uncertainty. But because the store is append-only and measurement only adds facts (never removes them), and because the commitment function w(d) is monotonically non-decreasing, this converges. Each measurement can only *decrease* uncertainty (by adding information) or leave it unchanged. It cannot increase it. This is the same contraction property that guarantees DDIS's bilateral loop converges.

```
INV-MEASURE-001: Measurement Contraction
Computing and asserting an uncertainty measurement for entity e 
MUST NOT increase the scalar uncertainty of e.

Formally: let S' = S ∪ {measurement datoms for e}.
Then uncertainty(e, S') ≤ uncertainty(e, S).

This holds because:
  - σ_e (epistemic) can only decrease: we've added observed attributes
  - σ_a (aleatory) is unchanged: measurement doesn't assert conflicting values
  - σ_c (consequential) may increase slightly (new dependents) but is 
    bounded by the monotonic growth of w(e)

The net effect is non-increasing because the epistemic reduction 
dominates the consequential growth for well-weighted α, β, γ.

Violation: An uncertainty measurement computation triggers a 
cascade of recomputations that increases the measured entity's 
uncertainty above its pre-measurement level.

Verification: Assert pre-measurement and post-measurement uncertainty 
scalars; verify post ≤ pre.
```

Actually — I need to pause here and be honest. The claim that measurement is always contractive is *plausible* but not *proven*. The consequential uncertainty term σ_c depends on the uncertainty of dependents, and adding a measurement fact creates a new dependent. In pathological cases with tight feedback loops, this could oscillate. The correct statement is:

```
INV-MEASURE-001 (REVISED): Measurement Contraction (Conditional)
Computing and asserting an uncertainty measurement for entity e 
MUST NOT increase the scalar uncertainty of e, PROVIDED that the 
measurement entity is excluded from the consequential uncertainty 
computation of its own target.

Formally: σ_c(e) is computed over dependents(e) \ {measurements of e}.

This exclusion prevents the self-referential feedback loop while 
preserving the measurement's contribution to commitment weight.
```

This is the kind of subtlety that the ChatGPT conversation would have glossed over with "it's monotonic." The exclusion clause is small but critical — without it, the fixed-point property doesn't hold.

---

## Layer 5: Workflow & Task Schema

This models the implementation phase — tasks derived from the spec, their execution by agents, and the backward flow of absorption.

```
ENTITY TYPE: Task

:task/ident             keyword  :one   :lww
:task/title             string   :one   :lww
:task/spec-element      ref      :one   :lww      — the spec element this implements
:task/status            keyword  :one   :lattice  — :pending < :ready < :in-progress 
                                                     < :review < :done
                                                     (lattice: task-lifecycle)
:task/assigned-to       ref      :one   :lww      — agent working on it
:task/delegation-level  ref      :one   :lww      — delegation decision that authorized this
:task/depends-on        ref      :many  :multi    — task dependencies (from impl-order)
:task/blocks            ref      :many  :multi    — tasks blocked by this one
:task/priority          long     :one   :lattice  — numeric, higher = more important
                                                     (lattice: numeric-max)

ENTITY TYPE: Absorption Event (backward flow: impl → spec)

:absorption/source-file     string   :one   :lww   — implementation file
:absorption/source-pattern  string   :one   :lww   — what was detected
:absorption/target-element  ref      :one   :lww   — spec element affected
:absorption/type            keyword  :one   :lww   — :structural, :behavioral, :terminological
:absorption/drift-delta     double   :one   :lww   — drift change caused by this absorption
:absorption/agent           ref      :one   :lww

ENTITY TYPE: Drift Measurement

:drift/spec-element     ref      :one   :lww
:drift/score            double   :one   :lww      — 0.0 = perfect alignment, 1.0 = total divergence
:drift/category         keyword  :one   :lww      — :structural, :behavioral, :terminological
:drift/remediation      string   :one   :lww      — suggested fix
:drift/measured-at      ref      :one   :lww

ENTITY TYPE: Contradiction (detected by the 5-tier checker)

:contradiction/element-a    ref      :one   :lww
:contradiction/element-b    ref      :one   :lww
:contradiction/tier         long     :one   :lww      — 1-6 (which tier detected)
:contradiction/confidence   double   :one   :lww
:contradiction/type         keyword  :one   :lww      — :governance-overlap, :polarity-inversion,
                                                        :quantifier-conflict, :numeric-bound,
                                                        :semantic-tension, :sat-unsat
:contradiction/status       keyword  :one   :lattice  — same as conflict-lifecycle
:contradiction/resolution   string   :one   :lww

ENTITY TYPE: Fitness Snapshot

:fitness/spec               ref      :one   :lww
:fitness/score              double   :one   :lww      — F(S) ∈ [0,1]
:fitness/validation         double   :one   :lww      — V component
:fitness/coverage           double   :one   :lww      — C component
:fitness/drift              double   :one   :lww      — D component
:fitness/challenge-health   double   :one   :lww      — H component
:fitness/contradictions     double   :one   :lww      — K component
:fitness/issues             double   :one   :lww      — I component
:fitness/mean-uncertainty   double   :one   :lww      — new: from uncertainty tensor
:fitness/computed-at        ref      :one   :lww

LATTICE: task-lifecycle
  :pending < :ready < :in-progress < :review < :done
```

---

## Schema Summary

Let me count what we've defined:

| Layer | Entity Types | Attributes | Lattices |
|---|---|---|---|
| 0: Meta-schema | 1 (Lattice) | 17 axiomatic | — |
| 1: Agent & Provenance | 2 (Agent, Transaction) | 16 | 1 (agent-lifecycle) |
| 2: DDIS Core | 12 (Spec, Section, Invariant, ADR, ADR Option, Gate, Negative Spec, Module, Cross-Ref, Glossary, Annotation, Witness, Challenge) | 72 | 5 (confidence-level, adr-lifecycle, witness-lifecycle, challenge-verdict, task-lifecycle) |
| 3: Discovery | 4 (Thread, Confidence Vector, Finding, Proposal, Synthesis) | 28 | 3 (thread-lifecycle, finding-lifecycle, proposal-lifecycle) |
| 4: Coordination | 7 (Uncertainty, Authority, Delegation, Conflict, Resolution, Graph Metrics, Stability) | 35 | 2 (delegation-level, conflict-lifecycle) |
| 5: Workflow | 5 (Task, Absorption, Drift, Contradiction, Fitness) | 27 | 1 (task-lifecycle already counted) |
| **Total** | **31 entity types** | **~195 attributes** | **12 lattice definitions** |

---

## What This Means Architecturally

The schema is the system. With 195 attributes across 31 entity types, plus 12 lattice definitions, the datom store can represent:

1. **The complete state of a DDIS specification** — every section, invariant, ADR, gate, and cross-reference
2. **The full exploration history** — every discovery thread, finding, proposal, and synthesis
3. **The coordination state** — every uncertainty measurement, authority score, delegation decision, and conflict resolution
4. **The implementation state** — every task, absorption event, drift measurement, and fitness snapshot
5. **Its own meta-schema** — attribute definitions, lattice definitions, and resolution modes

All of this is queryable by the same Datalog engine. An agent can ask "what is the most uncertain entity in the spec that I have authority to work on?" and get a single query that joins across the coordination schema, the DDIS core schema, and the agent schema. That query was impossible with the JSONL event log because events are typed and siloed — you can't join across event streams without application code.

---

## Next Steps

The formal core (axioms, functions, invariants, principles) plus the schema gives us enough to write the DDIS spec for this system. The next layer is the **query patterns** — the specific Datalog queries that implement the coordination logic (uncertainty computation, authority derivation, delegation decisions, conflict detection). These are the operational core of the system — the code that agents actually run to coordinate.

Alternatively, if you want to start writing the spec now (using DDIS to specify this system), I can draft the `manifest.yaml` and constitution for the fact store spec, mapping everything we've defined into the DDIS format.

Which would you prefer — the query patterns first, or the DDIS spec skeleton first?

---

## Willem

query patterns

---

## Claude

This is the operational core. Every coordination decision in the system reduces to a query. I'll build from primitive patterns up to the full coordination logic, flagging monotonicity at each level because that determines whether the query is safe to run uncoordinated.

---

## The Query Language

First, the dialect. I'm using Datomic-style Datalog syntax because it's well-documented and maps directly to our schema, but with three extensions needed for the CRDT-aware context:

```
STANDARD DATOMIC CLAUSES:

[:find ?vars ...                    — projection (what to return)
 :in $ ?input-vars ...              — parameterization ($ = current db)
 :where                             — conjunction of pattern clauses
 [?e :attr ?v]                      — entity-attribute-value pattern
 [?e :attr ?v ?tx]                  — with transaction binding
 [?e :attr ?v ?tx ?op]              — with operation binding (assert/retract)
 [(pred ?x ?y)]                     — predicate expression
 [(fn ?x) ?result]                  — function expression
 (rule ?x ?y)                       — rule invocation
 (not [?e :attr ?v])                — negation (NON-MONOTONIC)
 (or [?e :attr ?v] [?e :attr2 ?v]) — disjunction
]

EXTENSIONS FOR CRDT CONTEXT:

[:frontier ?frontier-ref            — bind query to a specific frontier 
                                      (default: local frontier)
 :stability-min ?threshold          — only use facts with commitment ≥ threshold
 :barrier :required                 — assert that a sync barrier must be held
]

:frontier restricts the query to datoms visible at a given frontier 
(the down-set in the causal partial order). Without it, the query 
runs against the agent's full local knowledge.

:stability-min filters out low-commitment facts before evaluation. 
This is a pre-filter, not a post-filter — it affects which datoms 
participate in joins, not just which results are returned.

:barrier declares that this query requires a consistent cut. The 
engine MUST refuse to evaluate if no sync barrier is held. This is 
the static enforcement mechanism for the CALM theorem boundary.
```

### Evaluation Strategy

**Bottom-up semi-naive evaluation** (following datafrog's approach). Rules are evaluated iteratively: each round computes new tuples from the previous round's delta until fixpoint. This is well-suited to Rust because the core loop is tight and cache-friendly.

**Stratification for negation.** Queries containing `not` or aggregation are evaluated in strata. Within a stratum, all rules are monotonic. Between strata, the negation/aggregation is applied to the completed result of the previous stratum. This is standard and well-understood.

**The critical interaction with CRDTs:** Within a single stratum of monotonic rules, the query result is a monotonic function of the input datom set. This means:

1. Adding more datoms can only add to the result (never remove)
2. Two agents with overlapping but different datom sets get results that are both subsets of the result over the union
3. After merging, the result over the merged set is a superset of both pre-merge results

This is CALM compliance for the monotonic stratum. Non-monotonic strata break this property — which is why they're gated by `:stability-min` or `:barrier`.

---

## Stratum 0: Primitive Patterns (Monotonic)

These are the base-level queries that everything else builds on. All are monotonic and safe to run uncoordinated at any frontier.

### Current Value Resolution

The most fundamental query: "what is the current value of attribute `a` for entity `e`?" This must respect the resolution mode (A5).

```datalog
;; RULE: current-value
;; Resolves the "current" value of a cardinality-one attribute
;; according to its resolution mode.
;;
;; For :lww — latest assertion by HLC wins
;; For :lattice — lattice join of all concurrent assertions
;; For :multi — returns all values (caller must handle set)
;;
;; Retractions cancel specific prior assertions (A2: Op semantics)

;; Step 1: Compute the "live" assertions — those not retracted
;; MONOTONIC (filtering by set membership)

[:find ?e ?a ?v ?tx
 :where
 [?e ?a ?v ?tx :assert]
 (not-join [?tx-r]                    ;; NON-MONOTONIC — see note below
   [?e ?a ?v ?tx-r :retract]
   [(tx-after? ?tx-r ?tx)])]          ;; retraction is causally after assertion
```

**Monotonicity note:** This query contains `not-join`, which is non-monotonic. However, it's a *stratified* non-monotonicity: the retraction set is computed in a lower stratum (just the raw retract datoms), and the negation is applied in the next stratum. This is safe under stratified evaluation *if* the set of retract datoms is stable at the agent's frontier.

This is the fundamental tension in the system: even the most basic "current value" query involves negation (checking that a value hasn't been retracted). We have three options, and I want to present them honestly because this is a foundational choice:

**Option A: Accept stratified negation as safe for local-frontier queries.** The reasoning is that an agent's local frontier is fixed at query time. The set of retract datoms it knows about won't change during evaluation. So stratified negation over a fixed frontier is deterministic and consistent, even if another agent with a different frontier would get a different answer.

**Option B: Separate the "live set" computation into a materialized view.** Maintain a separate index of `[e a v]` triples that represent the current live state, updated incrementally as new datoms arrive. Queries over the live view are monotonic (just lookups). The non-monotonicity is pushed into the materialization step.

**Option C: Don't use negation at all. Use the assertion/retraction history directly.** Define "current value" as a fold over the datom history for `[e a]`, applying assert/retract operations in causal order. This is a function, not a query — it's evaluated imperatively rather than declaratively.

**My recommendation: Option B.** It's the cleanest separation of concerns. The materialized live-set is updated by a deterministic incremental process (analogous to DDIS's `materialize` fold), and all Datalog queries run over the live set without negation. This makes the entire query stratum monotonic, which means full CALM compliance for all coordination queries.

The materialized live-set becomes a fifth index alongside EAVT, AEVT, VAET, AVET:

```
EAVT  — all datoms, sorted by entity-attribute-value-tx
AEVT  — all datoms, sorted by attribute-entity-value-tx
VAET  — ref datoms, sorted by value-attribute-entity-tx
AVET  — unique/indexed datoms, sorted by attribute-value-entity-tx
LIVE  — current live state: [e a v] after applying all assert/retract ops
        Resolution-mode-aware: for :lww, one value per [e a];
        for :lattice, the lattice join; for :multi, the full set
```

With this, the "current value" query becomes:

```datalog
;; RULE: current-value (over LIVE index — fully monotonic)
;; No negation required. The LIVE index is the materialized result
;; of applying all assert/retract operations.

[:find ?e ?a ?v
 :in $ ?target-entity ?target-attr
 :where
 [?target-entity ?target-attr ?v]]    ;; implicit: queries LIVE index
```

I'll proceed with Option B for all subsequent queries.

**Invariant:**

```
INV-LIVE-001: Live Index Consistency
The LIVE index MUST be the deterministic result of applying all 
assert and retract datoms in causal order with the declared 
resolution mode per attribute.

Formally: LIVE(S) = fold(causal-sort(S), apply-resolution)

Where apply-resolution for each [e a]:
  :lww    → value from the assertion with the greatest HLC
  :lattice → ⊔ over all unretracted assertions
  :multi  → set of all unretracted values

Violation: The LIVE index shows [bob :status :active] but the 
datom history contains a retraction of this value with no 
subsequent re-assertion.

Verification: Full recomputation of LIVE from raw datoms; 
compare to incremental result.
```

---

## Stratum 1: Graph Traversal Rules (Monotonic)

These rules compute transitive relationships over the datom graph. They're recursive Datalog — the core of what makes Datalog more expressive than SQL for graph queries.

### Causal Ancestry

```datalog
;; RULE: causal-ancestor
;; Transitive closure of the causal-parents relation on transactions.
;; ?ancestor causally precedes ?descendant.

(causal-ancestor ?ancestor ?descendant) :-
  [?descendant :tx/causal-parents ?ancestor].

(causal-ancestor ?ancestor ?descendant) :-
  [?descendant :tx/causal-parents ?mid],
  (causal-ancestor ?ancestor ?mid).

;; MONOTONIC: adding new transactions can only add new ancestry paths.
;; Semi-naive evaluation: each round discovers ancestors one hop further.
;; Terminates because the causal graph is a DAG (no cycles by construction).
```

### Forward Causal Cone (for commitment function w)

```datalog
;; RULE: causal-descendant
;; The inverse of causal-ancestor. ?descendant depends on ?ancestor.

(causal-descendant ?descendant ?ancestor) :-
  [?descendant :tx/causal-parents ?ancestor].

(causal-descendant ?descendant ?ancestor) :-
  [?descendant :tx/causal-parents ?mid],
  (causal-descendant ?mid ?ancestor).

;; RULE: datom-dependents
;; All datoms that transitively depend on datom ?d
;; (i.e., were asserted in transactions that causally follow ?d's tx)

[:find (count ?dep-datom)
 :in $ ?target-datom
 :where
 [?target-datom _ _ ?target-tx]       ;; get the tx of the target datom
 (causal-descendant ?dep-tx ?target-tx)
 [?dep-datom _ _ ?dep-tx]]            ;; all datoms in descendant txs

;; This count IS the commitment function w(d).
;; MONOTONIC in the datom count (adding datoms can only increase w).
;; NON-MONOTONIC in the count aggregation — but the count can only 
;; increase, so it's monotonically non-decreasing as a function of 
;; store growth. This makes it safe for the stability computation.
```

### Entity Dependency Graph

```datalog
;; RULE: entity-depends-on
;; Entity ?a depends on entity ?b if ?a has a ref attribute pointing to ?b,
;; OR if ?a was asserted in a transaction that causally follows a 
;; transaction asserting ?b.

(entity-depends-on ?a ?b) :-
  [?a ?attr ?b],
  [?attr :db/valueType :db.type/ref].

(entity-depends-on ?a ?b) :-
  [?a _ _ ?tx-a],
  [?b _ _ ?tx-b],
  (causal-ancestor ?tx-b ?tx-a),
  [(!= ?a ?b)].

;; Transitive closure
(entity-depends-on-transitive ?a ?b) :-
  (entity-depends-on ?a ?b).

(entity-depends-on-transitive ?a ?b) :-
  (entity-depends-on ?a ?mid),
  (entity-depends-on-transitive ?mid ?b).
```

### Cross-Reference Reachability (for DDIS contradiction detection)

```datalog
;; RULE: reach-set
;; The set of all entities reachable from ?source via cross-references.
;; Used for Jaccard similarity in Tier 2 contradiction detection.

(reachable ?source ?target) :-
  [?xref :xref/source ?source],
  [?xref :xref/target ?target],
  [?xref :xref/resolved true].

(reachable ?source ?target) :-
  [?xref :xref/source ?source],
  [?xref :xref/target ?mid],
  [?xref :xref/resolved true],
  (reachable ?mid ?target).
```

---

## Stratum 2: Uncertainty Computation (Mixed Monotonicity)

This is the core coordination logic. Each dimension of the uncertainty tensor is a separate computation with different monotonicity properties.

### Epistemic Uncertainty (σ_e)

"How much do we know about this entity?"

```datalog
;; RULE: schema-expected-attributes
;; For a given entity type, what attributes are expected?
;; This is derived from the schema: all attributes whose :db/doc
;; mentions this entity type, or that appear on exemplar entities
;; of this type.

;; For now, we use a simpler heuristic: count attributes that 
;; have been observed on ANY entity of the same type, and compare
;; to how many this specific entity has.

;; Step 1: Determine entity type (by convention: :entity/type attr)
;; Step 2: Count distinct attributes observed on peers of same type  
;; Step 3: Count distinct attributes on target entity
;; Step 4: Ratio = observed / expected

;; QUERY: epistemic-uncertainty
[:find ?entity (- 1.0 (/ ?observed ?expected))
 :in $ ?entity
 :where
 [?entity :entity/type ?type]
 
 ;; Count attributes on this entity
 [(ground ?entity) ?e]
 [?e ?any-attr _]
 [(count-distinct ?any-attr) ?observed]
 
 ;; Count attributes observed across all entities of this type
 [?peer :entity/type ?type]
 [?peer ?peer-attr _]
 [(count-distinct ?peer-attr) ?expected]]
```

**Monotonicity:** Adding more datoms about the entity *decreases* σ_e (more observed attributes → ratio increases → uncertainty decreases). Adding more datoms about peer entities *increases* σ_e (more expected attributes → ratio decreases). In practice, the expected set stabilizes quickly (schema is defined early), so σ_e is dominated by the monotonically decreasing numerator term. This is *eventually* monotonically non-increasing but not strictly so.

**Assessment:** Safe to run uncoordinated for practical purposes. The worst case is that an agent overestimates epistemic uncertainty (hasn't seen schema datoms from other agents yet), which causes it to *under*-delegate. This is the conservative failure mode.

### Aleatory Uncertainty (σ_a)

"How much do agents disagree about this entity?"

This is the entropy computation over concurrent values for cardinality-one attributes. This is the most complex query because it requires grouping concurrent assertions and computing Shannon entropy.

```datalog
;; RULE: concurrent-values
;; For a given entity-attribute pair, find all values that are 
;; "concurrent" — asserted in causally independent transactions.
;;
;; Two assertions are concurrent if neither's tx is a causal 
;; ancestor of the other's.

;; Step 1: Get all live assertions for [e a]
;; Step 2: For each pair, check causal independence
;; Step 3: Group concurrent values

;; QUERY: concurrent-assertions
;; Returns all live values for a given [e a] with their asserting agents
[:find ?entity ?attr ?value ?agent
 :in $ ?entity ?attr
 :where
 [?entity ?attr ?value ?tx :assert]
 [?tx :tx/agent ?agent]
 ;; Exclude retracted values (LIVE index handles this)
 ]

;; QUERY: aleatory-uncertainty
;; Computes entropy over the value distribution for each attr of an entity
;; 
;; This CANNOT be expressed in pure Datalog because entropy requires:
;; 1. Grouping (group by value, count agents per value)
;; 2. Division (proportion = count / total)
;; 3. Logarithm (entropy = -Σ p log p)
;;
;; These are aggregate functions that break the relational algebra.
;; 
;; DECISION: σ_a is computed as a DERIVED FUNCTION, not a pure query.
;; The Datalog engine provides the input (concurrent-assertions), 
;; and a Rust function computes the entropy.

fn aleatory_uncertainty(entity: EntityId, store: &Store) -> f64 {
    let mut total_entropy = 0.0;
    let mut attr_count = 0;
    
    for attr in store.attributes_of(entity) {
        if store.cardinality(attr) != Cardinality::One { continue; }
        
        // Get all live values with agent counts
        let values: HashMap<Value, usize> = store.query(
            "[:find ?value (count ?agent)
              :in $ ?entity ?attr
              :where
              [?entity ?attr ?value ?tx]
              [?tx :tx/agent ?agent]]",
            entity, attr
        );
        
        if values.len() <= 1 { continue; } // no disagreement
        
        let total: f64 = values.values().sum::<usize>() as f64;
        let entropy: f64 = values.values()
            .map(|&c| {
                let p = c as f64 / total;
                -p * p.ln()
            })
            .sum();
        
        // Normalize to [0,1] by dividing by max possible entropy
        let max_entropy = (values.len() as f64).ln();
        total_entropy += entropy / max_entropy;
        attr_count += 1;
    }
    
    if attr_count == 0 { 0.0 } else { total_entropy / attr_count as f64 }
}
```

**Monotonicity:** σ_a is NOT monotonically non-increasing. Adding a new concurrent assertion can *increase* entropy (more disagreement). This is correct — aleatory uncertainty genuinely increases when agents disagree more. It's the one dimension that can grow, and that growth is the signal to the coordination layer that deliberation is needed.

**Assessment:** Safe to run uncoordinated because overestimation (seeing disagreement that might be resolved elsewhere) causes conservative behavior (escalation), and underestimation (not yet seeing a conflicting assertion) is self-correcting on the next merge.

### Consequential Uncertainty (σ_c)

"How much would it hurt to get this entity wrong?"

```datalog
;; RULE: consequential-uncertainty
;; σ_c(e) = w(e) × Σ_{d ∈ dependents(e)} uncertainty_scalar(d)
;;
;; This is recursive: consequential uncertainty depends on the 
;; uncertainty of dependents, which may themselves have consequential
;; uncertainty depending on their dependents.
;;
;; To avoid infinite recursion, we compute this bottom-up:
;; leaf entities (no dependents) have σ_c = 0.
;; Inner entities accumulate from leaves upward.
;;
;; This is a MONOTONIC DATALOG computation if we treat it as a 
;; fixed-point over the dependency DAG.

;; Step 1: Leaf entities (no outgoing entity-depends-on edges)
(consequential-base ?entity 0.0) :-
  [?entity :entity/type _],
  (not (entity-depends-on _ ?entity)).  ;; NON-MONOTONIC: negation

;; Step 2: Recursive accumulation
;; NOTE: This requires stratified negation (for the leaf base case)
;; followed by monotonic fixed-point computation.

;; DECISION: Like σ_a, this is a DERIVED FUNCTION, not pure Datalog.
;; The dependency graph is extracted by Datalog; the accumulation 
;; is computed by a Rust function doing bottom-up traversal.

fn consequential_uncertainty(
    entity: EntityId, 
    store: &Store,
    cache: &mut HashMap<EntityId, f64>,  // memoization
) -> f64 {
    if let Some(&cached) = cache.get(&entity) {
        return cached;
    }
    
    let commitment = store.commitment_weight(entity);
    
    let dependents: Vec<EntityId> = store.query(
        "[:find ?dep
          :in $ ?entity
          :where
          (entity-depends-on ?dep ?entity)]",
        entity
    );
    
    if dependents.is_empty() {
        cache.insert(entity, 0.0);
        return 0.0;
    }
    
    // CRITICAL: exclude uncertainty measurements of THIS entity
    // from the dependent set (INV-MEASURE-001 exclusion clause)
    let dependents: Vec<EntityId> = dependents.into_iter()
        .filter(|d| !is_uncertainty_measurement_of(store, *d, entity))
        .collect();
    
    let dep_uncertainty_sum: f64 = dependents.iter()
        .map(|d| scalar_uncertainty(store, *d, cache))
        .sum();
    
    let sigma_c = (commitment as f64) * dep_uncertainty_sum 
                  / (dependents.len() as f64 + 1.0);  // +1 smoothing
    let sigma_c = sigma_c.min(1.0);  // clamp to [0,1]
    
    cache.insert(entity, sigma_c);
    sigma_c
}
```

**The INV-MEASURE-001 exclusion is enforced here.** Line 19 of the function filters out uncertainty measurement entities that target the entity being measured. Without this filter, the function would diverge in the presence of self-referential measurements.

**Monotonicity:** σ_c is monotonically non-decreasing with respect to the dependency graph size (more dependents → higher σ_c) and monotonically non-increasing with respect to dependent resolution (resolved dependents have lower uncertainty → lower σ_c). The net direction depends on whether the system is growing (adding entities) or converging (resolving uncertainties). In the bilateral loop, the system first grows (forward flow) then converges (backward flow), so σ_c first increases then decreases. This is the expected behavior.

### Combined Uncertainty Scalar

```rust
fn scalar_uncertainty(
    store: &Store, 
    entity: EntityId,
    cache: &mut HashMap<EntityId, f64>,
) -> f64 {
    let sigma_e = epistemic_uncertainty(entity, store);
    let sigma_a = aleatory_uncertainty(entity, store);
    let sigma_c = consequential_uncertainty(entity, store, cache);
    
    // Weighted L2 norm
    // α, β, γ are system parameters stored as datoms in the meta-schema
    let alpha = store.system_param(":uncertainty/alpha"); // default 0.4
    let beta  = store.system_param(":uncertainty/beta");  // default 0.4
    let gamma = store.system_param(":uncertainty/gamma"); // default 0.2
    
    (alpha * sigma_e.powi(2) 
     + beta * sigma_a.powi(2) 
     + gamma * sigma_c.powi(2))
    .sqrt()
}
```

The default weights emphasize epistemic and aleatory uncertainty equally (0.4 each) with consequential uncertainty weighted lower (0.2). Rationale: epistemic and aleatory uncertainty are *actionable* — you can reduce them by exploring more or deliberating more. Consequential uncertainty is *structural* — it depends on the graph topology, which changes slowly. Overweighting it would cause the system to be overly cautious about well-understood entities that happen to be heavily depended upon.

These weights are themselves facts in the store and can be adjusted per deployment.

---

## Stratum 3: Authority Computation

The spectral authority derivation requires building the agent-entity contribution graph and computing its decomposition. This is not expressible in Datalog — it's linear algebra. But the *inputs* are Datalog queries, and the *outputs* are datoms in the store.

### Building the Contribution Graph

```datalog
;; QUERY: contribution-edges
;; All (agent, entity, weight) triples where the agent has contributed
;; to the entity through verified assertions.

[:find ?agent ?entity (sum ?weight)
 :where
 [?datom _ _ ?tx :assert]
 [?tx :tx/agent ?agent]
 [?datom :entity/type _]              ;; ?datom is about an entity
 
 ;; Weight = 1 for unverified, 2 for witnessed, 3 for challenged-confirmed
 ;; Computed via a helper rule:
 (contribution-weight ?datom ?weight)]

;; RULE: contribution-weight
;; Weight of a contribution based on its verification status

(contribution-weight ?datom 3) :-
  [?datom _ _ ?tx],
  [?witness :witness/invariant ?inv],
  [?challenge :challenge/witness ?witness],
  [?challenge :challenge/verdict :confirmed],
  ;; the datom is about an entity maintained by this invariant
  [?inv :inv/owner-module ?mod],
  [?datom :entity/type ?type].         ;; simplified; real version checks 
                                        ;; module-entity relationship

(contribution-weight ?datom 2) :-
  [?datom _ _ ?tx],
  [?witness :witness/invariant ?inv],
  [?witness :witness/status :valid],
  (not [?challenge :challenge/witness ?witness]
       [?challenge :challenge/verdict :confirmed]).

(contribution-weight ?datom 1) :-
  [?datom _ _ ?tx],
  (not [?witness :witness/invariant _]  ;; no witness covers this
       (contribution-weight ?datom 2)),
  (not (contribution-weight ?datom 3)).
```

### Spectral Decomposition

```rust
/// Build the bipartite adjacency matrix and compute its SVD.
/// Returns authority scores per (agent, entity) pair.
fn compute_spectral_authority(store: &Store) -> Vec<AuthorityScore> {
    // Step 1: Get contribution edges from Datalog
    let edges: Vec<(AgentId, EntityId, f64)> = store.query(
        "[:find ?agent ?entity ?weight
          :where (contribution-edge ?agent ?entity ?weight)]"
    );
    
    // Step 2: Build bipartite adjacency matrix
    // Rows = agents, Columns = entities, Values = contribution weights
    let (agent_idx, entity_idx, matrix) = build_adjacency_matrix(&edges);
    
    // Step 3: SVD decomposition
    // A ≈ U_k × Σ_k × V_k^T
    // k = min(50, agent_count, entity_count)
    let k = 50.min(agent_idx.len()).min(entity_idx.len());
    let svd = truncated_svd(&matrix, k);
    
    // Step 4: Authority scores
    // For agent i on entity j:
    //   authority(i,j) = Σ_d (U[i,d] × Σ[d] × V[j,d])
    // This is the reconstruction of the original matrix through the 
    // low-rank approximation — it captures the "structural" contribution
    // pattern, not just the raw edge weight.
    let mut scores = Vec::new();
    for (agent, i) in &agent_idx {
        for (entity, j) in &entity_idx {
            let score: f64 = (0..k)
                .map(|d| svd.u[(*i, d)] * svd.sigma[d] * svd.vt[(d, *j)])
                .sum();
            
            if score > 0.0 {
                scores.push(AuthorityScore {
                    agent: *agent,
                    entity: *entity,
                    score,
                    contribution_count: direct_contribution_count(
                        &edges, *agent, *entity
                    ),
                });
            }
        }
    }
    
    scores
}
```

### Why Spectral and Not Just Raw Contribution Count

The spectral decomposition captures something that raw contribution counting misses: **transitive authority.** If agent α has contributed heavily to entities A, B, and C, and entities A, B, and C are all closely related to entity D (through cross-references or shared attributes), then agent α has authority over entity D *even if it has never directly touched D*. The SVD captures this by projecting agents and entities into a shared latent space where proximity reflects structural similarity.

This is the same mathematical machinery DDIS already uses for LSI search (SVD over the term-document matrix). Applied to the agent-entity matrix instead, it gives you authority instead of relevance. The analogy is precise: LSI finds "this document is relevant to this query because it's similar to other documents that match" — spectral authority finds "this agent has authority on this entity because it has contributed to structurally similar entities."

### Delegation Threshold Computation

```datalog
;; QUERY: delegation-threshold
;; The minimum authority required to work on a given entity.
;; Derived from graph metrics and consequential uncertainty.

[:find ?entity ?threshold
 :in $ ?entity
 :where
 [?metrics :metrics/entity ?entity]
 [?metrics :metrics/betweenness ?betweenness]
 [?metrics :metrics/in-degree ?in-degree]
 [?unc :uncertainty/entity ?entity]
 [?unc :uncertainty/consequential ?sigma-c]
 
 ;; Schema cardinality factor: entities with more cardinality-one
 ;; attributes are more conflict-prone
 [(entity-conflict-surface ?entity) ?surface]
 
 ;; Threshold formula:
 ;; threshold = normalize(
 ;;   w1 * betweenness + w2 * in-degree + w3 * σ_c + w4 * surface
 ;; )
 [(* 0.3 ?betweenness) ?t1]
 [(* 0.2 ?in-degree) ?t2]
 [(* 0.3 ?sigma-c) ?t3]
 [(* 0.2 ?surface) ?t4]
 [(+ ?t1 ?t2 ?t3 ?t4) ?threshold]]

;; RULE: entity-conflict-surface
;; Fraction of an entity's attributes that are cardinality-one
;; (and therefore susceptible to conflict)

[(entity-conflict-surface ?entity) ?surface] :-
  [?entity _ _ ?tx],
  [(count-attrs ?entity :one) ?one-count],
  [(count-attrs ?entity :all) ?all-count],
  [(/ ?one-count ?all-count) ?surface].
```

### The Delegation Decision

```datalog
;; QUERY: can-resolve
;; Can agent ?agent resolve conflicts on entity ?entity?

[:find ?agent ?entity
 :in $ ?agent ?entity
 :where
 [?auth :authority/agent ?agent]
 [?auth :authority/entity ?entity]
 [?auth :authority/score ?score]
 
 [?deleg :delegation/entity ?entity]
 [?deleg :delegation/threshold ?threshold]
 
 [(>= ?score ?threshold)]]

;; QUERY: delegation-decision
;; What is the delegation status of entity ?entity?

[:find ?entity ?decision
 :in $ ?entity
 :where
 [?unc :uncertainty/entity ?entity]
 [?unc :uncertainty/scalar ?u-scalar]
 
 ;; Count agents that can resolve
 [(resolver-count ?entity) ?rc]
 
 ;; Decision logic:
 ;; rc > 0 AND u_scalar < 0.2  → :delegatable
 ;; rc > 0 AND u_scalar >= 0.2 → :contested  
 ;; rc = 0 AND u_scalar < 0.5  → :escalated
 ;; rc = 0 AND u_scalar >= 0.5 → :human-required
 [(delegation-classify ?rc ?u-scalar) ?decision]]
```

The threshold values (0.2, 0.5) are configurable system parameters stored as datoms.

---

## Stratum 4: Conflict Detection and Routing

### Structural Conflict Detection

```datalog
;; QUERY: detect-conflicts
;; Find all entity-attribute pairs with concurrent conflicting values.
;; This is the core conflict detection query.

;; Two datoms conflict iff:
;; 1. Same entity and attribute
;; 2. Different values
;; 3. Both currently live (in LIVE index)
;; 4. Attribute has cardinality :one
;; 5. Neither tx causally precedes the other (concurrent)

[:find ?entity ?attr ?val-a ?tx-a ?val-b ?tx-b
 :where
 [?entity ?attr ?val-a ?tx-a :assert]
 [?entity ?attr ?val-b ?tx-b :assert]
 [(!= ?val-a ?val-b)]
 [(!= ?tx-a ?tx-b)]
 
 ;; Cardinality check
 [?attr :db/cardinality :db.cardinality/one]
 
 ;; Concurrency check: neither is ancestor of the other
 (not (causal-ancestor ?tx-a ?tx-b))
 (not (causal-ancestor ?tx-b ?tx-a))
 
 ;; Both are still live (not retracted)
 ;; (Handled by querying against LIVE index)
 ]

;; MONOTONICITY: This query contains negation (not causal-ancestor).
;; However, causal-ancestor is MONOTONICALLY GROWING — adding new 
;; causal links can only ADD ancestry paths, which means the negation 
;; can only REMOVE pairs from the conflict set (a previously-concurrent 
;; pair might become causally ordered after learning about intermediate 
;; transactions). This means the conflict set is MONOTONICALLY 
;; NON-INCREASING with respect to causal information.
;;
;; In practice: an agent might detect a "false conflict" because it 
;; hasn't yet learned about the causal path between two transactions. 
;; After merging, the false conflict disappears. This is the 
;; conservative failure mode: over-detection of conflicts, never 
;; under-detection.
```

This monotonicity property is important enough to state as an invariant:

```
INV-CONFLICT-CONSERVATIVE-001: Conservative Conflict Detection
The set of detected conflicts at any local frontier MUST be a 
superset of the conflicts that would be detected at the global 
frontier (union of all agents' knowledge).

Formally: conflicts(frontier_local) ⊇ conflicts(frontier_global)

This holds because learning about new causal paths can only 
resolve apparent concurrency (turning concurrent pairs into 
causally-ordered pairs), never create new concurrency.

Consequence: An agent may waste effort resolving a conflict that 
doesn't actually exist (another agent's assertion was causally 
prior but the causal link hasn't propagated yet). This is safe — 
the resolution produces a redundant datom that is harmlessly 
deduplicated on merge. The system never MISSES a real conflict.
```

### Conflict Severity and Routing

```rust
/// Route a detected conflict to the appropriate resolution mechanism.
fn route_conflict(
    store: &Store,
    entity: EntityId,
    attr: AttrId,
    datom_a: DatomId,
    datom_b: DatomId,
) -> ResolutionRoute {
    // Step 1: Check if the attribute has a lattice
    if let Some(lattice) = store.lattice_for(attr) {
        let val_a = store.value_of(datom_a);
        let val_b = store.value_of(datom_b);
        
        if let Some(joined) = lattice.join(val_a, val_b) {
            // Lattice resolves it deterministically
            return ResolutionRoute::Automatic {
                method: ResolutionMethod::LatticeJoin,
                winner: joined,
                confidence: 1.0,
            };
        }
        // Values are incomparable in the lattice — fall through
    }
    
    // Step 2: Check if LWW resolves it
    let hlc_a = store.hlc_of(datom_a);
    let hlc_b = store.hlc_of(datom_b);
    if hlc_a != hlc_b {
        let winner = if hlc_a > hlc_b { datom_a } else { datom_b };
        // LWW resolves it, but with lower confidence
        return ResolutionRoute::Automatic {
            method: ResolutionMethod::LWW,
            winner: store.value_of(winner),
            confidence: 0.7, // LWW is deterministic but arbitrary
        };
    }
    
    // Step 3: Compute severity for escalation
    let w_a = store.commitment_weight(datom_a);
    let w_b = store.commitment_weight(datom_b);
    let severity = w_a.max(w_b) as f64;
    
    // Step 4: Find agents with authority to resolve
    let resolvers: Vec<AgentId> = store.query(
        "[:find ?agent
          :in $ ?entity
          :where (can-resolve ?agent ?entity)]",
        entity
    );
    
    if resolvers.is_empty() {
        return ResolutionRoute::Human {
            severity,
            reason: "No agent has sufficient authority".into(),
        };
    }
    
    // Step 5: Route based on severity thresholds
    let threshold_auto = store.system_param(":conflict/threshold-auto");
    let threshold_agent = store.system_param(":conflict/threshold-agent");
    
    if severity < threshold_auto {
        // Low severity: best-authority agent resolves automatically
        let best = resolvers.into_iter()
            .max_by_key(|a| store.authority(*a, entity))
            .unwrap();
        ResolutionRoute::AgentAutomatic {
            agent: best,
            severity,
        }
    } else if severity < threshold_agent {
        // Medium severity: agent resolves with human notification
        let best = resolvers.into_iter()
            .max_by_key(|a| store.authority(*a, entity))
            .unwrap();
        ResolutionRoute::AgentWithNotification {
            agent: best,
            severity,
            notify: store.human_agent(),
        }
    } else {
        // High severity: human decides
        ResolutionRoute::Human {
            severity,
            reason: format!(
                "Severity {} exceeds agent threshold {}", 
                severity, threshold_agent
            ),
        }
    }
}
```

---

## Stratum 5: DDIS Bilateral Loop Queries

These implement the specific DDIS operations over the datom store. They replace the current JSONL-fold-based implementations.

### Fitness Function

```datalog
;; QUERY: spec-fitness
;; Computes F(S) for a given spec entity.
;; Each component is a separate sub-query.

;; V: Validation score
[:find (/ (count ?passing) (count ?check))
 :in $ ?spec
 :where
 [?check :validation-check/spec ?spec]
 [?check :validation-check/status ?status]
 [(= ?status :passing) ?passing-bool]
 [(true? ?passing-bool)]
 ;; bind ?passing from ?check where status = :passing
 ]

;; C: Coverage score  
[:find (/ (count ?complete) (count ?element))
 :in $ ?spec
 :where
 [?element :inv/spec ?spec]
 ;; An invariant is "complete" if it has all 6 components
 [?element :inv/title _]
 [?element :inv/statement _]
 [?element :inv/semi-formal _]
 [?element :inv/violation _]
 [?element :inv/validation _]
 [?element :inv/why-this-matters _]]

;; D: Drift score
[:find (avg ?drift-score)
 :in $ ?spec
 :where
 [?drift :drift/spec-element ?elem]
 [?elem :inv/spec ?spec]
 [?drift :drift/score ?drift-score]]

;; H: Challenge health
[:find (/ (count ?confirmed) (count ?witness))
 :in $ ?spec
 :where
 [?witness :witness/invariant ?inv]
 [?inv :inv/spec ?spec]
 [?witness :witness/status :valid]
 [?challenge :challenge/witness ?witness]
 [?challenge :challenge/verdict :confirmed]]

;; K: Contradiction density
[:find (count ?contradiction)
 :in $ ?spec
 :where
 [?contradiction :contradiction/element-a ?a]
 [?a :inv/spec ?spec]
 [?contradiction :contradiction/status ?status]
 [(!= ?status :resolved)]]

;; U: Mean uncertainty (NEW — extends the fitness function)
[:find (avg ?u-scalar)
 :in $ ?spec
 :where
 [?entity _ _ _]
 [?entity :inv/spec ?spec]  ;; or more broadly, any entity in this spec
 [?unc :uncertainty/entity ?entity]
 [?unc :uncertainty/scalar ?u-scalar]]
```

The combined fitness function, now with seven terms:

```
F(S) = 0.18×V + 0.18×C + 0.18×(1-D) + 0.13×H + 0.13×(1-K) + 0.08×(1-I) + 0.12×(1-U)

where U = mean uncertainty across all spec entities
```

The weights are redistributed from the original 6-signal function to accommodate the uncertainty term. The uncertainty signal gets weight 0.12, reflecting its importance as a coordination-layer metric. The other weights are slightly reduced proportionally.

### Crystallization Readiness

This is the query that answers "is this discovery finding ready to become a spec element?"

```datalog
;; QUERY: crystallization-candidates
;; Findings whose stability exceeds the crystallization threshold
;; AND whose parent thread has sufficient confidence.

[:find ?finding ?thread ?stability
 :where
 [?finding :finding/status :refined]           ;; must be refined, not raw
 [?finding :finding/thread ?thread]
 [?thread :thread/status :active]
 
 ;; Stability check
 [?stab :stability/entity ?finding]
 [?stab :stability/score ?stability]
 [(>= ?stability 0.7)]                         ;; configurable threshold
 
 ;; Confidence check on parent thread
 [?thread :thread/confidence ?conf]
 [?conf :confidence/depth ?depth]
 [?conf :confidence/coherence ?coherence]
 [(>= ?depth 0.6)]
 [(>= ?coherence 0.6)]
 
 ;; No unresolved conflicts on this finding
 (not [?conflict :conflict/entity ?finding]
      [?conflict :conflict/status ?cs]
      [(!= ?cs :resolved)])]
```

This query has a `not` clause (no unresolved conflicts), making it non-monotonic. An agent that hasn't yet learned about a conflict on this finding might incorrectly conclude it's ready to crystallize. This is the *dangerous* failure mode — premature crystallization.

The defense is the `:stability-min` guard:

```datalog
;; SAFE VARIANT: only consider findings whose contributing facts 
;; all have commitment weight ≥ threshold

[:find ?finding ?thread ?stability
 :stability-min 3                              ;; at least 3 dependents
 :where
 ... same as above ...]
```

The `:stability-min 3` filter ensures that only findings built on well-established facts are considered. A finding whose supporting facts have few dependents is too provisional to crystallize even if it appears conflict-free.

### Delegation Routing for Implementation Tasks

```datalog
;; QUERY: ready-tasks
;; Tasks that are ready for delegation: all dependencies satisfied,
;; target spec element is delegatable, and an agent with authority exists.

[:find ?task ?spec-element ?agent ?authority-score
 :where
 [?task :task/status :pending]
 [?task :task/spec-element ?spec-element]
 
 ;; All dependencies are done
 (not [?task :task/depends-on ?dep]
      [?dep :task/status ?dep-status]
      [(!= ?dep-status :done)])
 
 ;; Spec element is delegatable
 [?deleg :delegation/entity ?spec-element]
 [?deleg :delegation/decision :delegatable]
 
 ;; Find the best agent
 [?auth :authority/entity ?spec-element]
 [?auth :authority/agent ?agent]
 [?auth :authority/score ?authority-score]
 [?agent :agent/status :active]]

;; ORDER BY ?authority-score DESC — best agent first
```

### The Backward Flow: Absorption Detection

```datalog
;; QUERY: drift-candidates
;; Spec elements whose implementation annotations suggest divergence.
;; This is the trigger for the backward flow.

[:find ?inv ?annotation ?verb
 :where
 [?annotation :annotation/target ?inv]
 [?annotation :annotation/verb ?verb]
 [?annotation :annotation/orphaned false]
 
 ;; The annotation's source file has been modified more recently
 ;; than the invariant's last update
 [?annotation :annotation/file ?file]
 [?annotation _ _ ?ann-tx]
 [?inv :inv/content-hash _ ?inv-tx]
 
 ;; ann-tx is causally after inv-tx
 (causal-ancestor ?inv-tx ?ann-tx)
 
 ;; No absorption has been recorded since the annotation
 (not [?abs :absorption/target-element ?inv]
      [?abs _ _ ?abs-tx]
      (causal-ancestor ?ann-tx ?abs-tx))]
```

---

## Query Pattern Index

For reference, here's the complete set of query patterns we've defined, classified by stratum and monotonicity:

| Stratum | Query | Monotonic | Sync Barrier | Purpose |
|---|---|---|---|---|
| 0 | current-value | ✅ (via LIVE index) | No | Resolve current attribute value |
| 1 | causal-ancestor | ✅ | No | Transitive causal ordering |
| 1 | causal-descendant | ✅ | No | Forward causal cone |
| 1 | entity-depends-on | ✅ | No | Entity dependency graph |
| 1 | reachable | ✅ | No | Cross-reference reachability |
| 2 | epistemic-uncertainty | ⚠️ eventually | No | σ_e computation |
| 2 | aleatory-uncertainty | ❌ | No (conservative) | σ_a computation |
| 2 | consequential-uncertainty | ⚠️ mixed | No (conservative) | σ_c computation |
| 3 | contribution-edges | ✅ | No | Agent-entity contribution graph |
| 3 | spectral-authority | N/A (linear algebra) | No | Authority scores |
| 3 | delegation-threshold | ✅ | No | Per-entity threshold |
| 3 | can-resolve | ✅ | No | Authority ≥ threshold check |
| 3 | delegation-decision | ⚠️ aggregation | No | Delegation classification |
| 4 | detect-conflicts | ⚠️ conservative | No | Find concurrent contradictions |
| 4 | route-conflict | N/A (imperative) | No | Severity → resolution path |
| 5 | spec-fitness | ❌ aggregation | Optional | F(S) computation |
| 5 | crystallization-candidates | ❌ negation | Recommended | Ready-to-crystallize findings |
| 5 | ready-tasks | ❌ negation | Recommended | Delegatable implementation tasks |
| 5 | drift-candidates | ❌ negation | Recommended | Backward-flow triggers |

The pattern is clear: Strata 0–3 are fully or conservatively monotonic, safe to run at any frontier without coordination. Strata 4–5 contain non-monotonic queries that benefit from sync barriers for correctness-critical decisions (crystallization, delegation, fitness measurement) but can run uncoordinated for advisory purposes (showing the human what *might* be ready).

This maps to exactly the 4B/4C hybrid you chose in Question 4: monotonic queries run freely, non-monotonic queries are stability-gated for critical decisions.

---

## One Structural Observation

Looking at the full query set, a pattern emerges that I didn't anticipate. The queries naturally partition into two categories that correspond to the two directions of the bilateral loop:

**Forward-flow queries** (human → spec → implementation): epistemic uncertainty, crystallization candidates, delegation decisions, ready tasks. These are *planning* queries — "what should we do next?"

**Backward-flow queries** (implementation → spec → human): conflict detection, drift candidates, aleatory uncertainty, absorption triggers. These are *assessment* queries — "what went wrong or changed?"

The commitment function w(d) and consequential uncertainty σ_c sit at the intersection — they're used by both flows. The spectral authority computation is the bridge: it's updated by backward-flow observations (agent contributions) and consumed by forward-flow decisions (delegation).

This bilateral structure in the query layer mirrors the bilateral structure in the DDIS workflow. It's not something I designed in — it fell out of the formalization. That's usually a sign the model is capturing real structure.

---

## Next Step

With the schema (Layer 0–5) and query patterns (Strata 0–5) defined, we have the full formal core of the system. The next step is to either:

**A. Draft the DDIS manifest and constitution for this spec.** Map everything we've defined into DDIS-conforming sections with invariants, ADRs, and quality gates. This would be the actual spec document that a DDIS tool would validate.

**B. Define the storage engine interface.** The Rust trait boundaries for the datom store, index layer, and query engine — the API surface that the implementation would code against.

**C. Define the agent protocol.** How agents actually interact with the store — the transaction format, the merge protocol, the sync barrier mechanism, and the frontier exchange format.

My instinct is C — the agent protocol is the missing piece between "the store exists and has queries" and "agents actually use it to coordinate." But your call.

---
