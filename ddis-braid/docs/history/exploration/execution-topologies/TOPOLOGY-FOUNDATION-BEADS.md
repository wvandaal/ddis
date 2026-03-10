# Topology-Ready Foundations — Formal Design Proposal

> **Status**: APPROVED — All open questions resolved. Implementation proceeds F1→F2→F3→F4→F5→F6.
> **Traces to**: exploration/execution-topologies/ (documents 00–11)
> **Scope**: Spec and guide additions for Stage 0 that enable topology framework (Stage 3)
> **Principle**: Zero topology code in Stage 0, zero topology debt in Stage 3.

---

## Dependency Graph (Read First)

```
TOPO-EPIC (epic)
├── F1: Self-Bootstrap Dependency Graph [CRITICAL]
│   ├── F1-A: Audit existing :spec/depends-on coverage
│   ├── F1-B: Add typed relationship attributes (:spec/affects, :spec/constrains, :spec/tests)
│   ├── F1-C: INV-SCHEMA-009 — Spec Dependency Graph Completeness
│   ├── F1-D: ADR-SCHEMA-007 — Typed vs Untyped Spec Relationships
│   ├── F1-E: NEG-BOOTSTRAP-001 — Content-only bootstrap
│   ├── F1-F: Update bootstrap worked example to show dependency edges
│   └── F1-G: Update bootstrap EDNL template with dependency population
│
├── F2: Resolution Mode Extensibility [HIGH]
│   ├── F2-A: Audit lattice registration mechanism for custom topology lattices
│   ├── F2-B: Define topology-lifecycle lattice (13th named lattice)
│   ├── F2-C: Define priority-max lattice refinement for topology priority
│   ├── F2-D: ADR-SCHEMA-008 — Pre-register coordination lattices at Layer 0
│   └── F2-E: Forward-reference Layer 4 resolution mode requirements
│
├── F3: Agent Entity First-Class [HIGH]
│   ├── F3-A: ADR-STORE-020 — Agent entity ID scheme
│   ├── F3-B: Define minimal Layer 1 agent schema (extend existing 2+16)
│   ├── F3-C: INV-STORE-015 — Agent entity completeness
│   ├── F3-D: Verify :tx/agent is Ref to agent entity (not bare keyword)
│   ├── F3-E: System agent entity in genesis transaction
│   └── F3-F: Forward-reference extensibility for :agent/capabilities, :agent/trust
│
├── F4: Frontier Queryable Data Model [HIGH]
│   │   (depends on F3)
│   ├── F4-A: ADR-STORE-021 — Frontier representation (TxId set vs vector clock)
│   ├── F4-B: Define :agent/frontier attribute in Layer 1
│   ├── F4-C: INV-STORE-016 — Frontier computability
│   ├── F4-D: Frontier-relative staleness as Datalog query
│   └── F4-E: Verify single-agent degenerate case (frontier = all txs)
│
├── F5: Spectral Computation in Query Engine [MEDIUM]
│   ├── F5-A: Add Laplacian computation to Stratum 3 capability list
│   ├── F5-B: Add eigendecomposition to Stratum 3 capability list
│   ├── F5-C: ADR-QUERY-012 — Spectral operations via nalgebra FFI
│   └── F5-D: INV-QUERY-022 — Spectral computation correctness
│
└── F6: Trilateral → Quadrilateral Extension Point [LOW]
    ├── F6-A: Reserve TOPO_ATTRS namespace in attribute partition
    ├── F6-B: Add forward reference in spec/18-trilateral.md
    ├── F6-C: ADR-TRILATERAL-004 — N-lateral extensibility
    ├── F6-D: Ensure Φ(S) formula is algebraically extensible
    └── F6-E: Define LIVE_T projection extension point
```

---

## F1: Self-Bootstrap Dependency Graph [CRITICAL]

### F1-A: Audit Existing `:spec/depends-on` Coverage

**Finding**: `:spec/depends-on` (Ref, :many, :multi) already exists in Layer 2
(spec/02-schema.md line 144). The bootstrap path (docs/guide/00-architecture.md line 666)
explicitly states "Cross-references (`:spec/traces-to`, `:spec/depends-on`) become ref datoms."

**Gap**: The worked example (docs/guide/11-worked-examples.md, Step 3) transacts INV-STORE-001
WITHOUT any `:spec/depends-on` refs. The bootstrap demonstrates content but not structure.
This means an implementing agent would produce a flat collection of spec elements with no
queryable dependency graph — making the compilation front-end (doc 11) impossible.

**Task**: Verify which spec elements currently have `:spec/depends-on` relationships
defined in the spec prose, and count the gap.

---

### F1-B: Add Typed Relationship Attributes

**Rationale**: `:spec/depends-on` captures one type of relationship (X depends on Y).
The compilation front-end (exploration/11-topology-as-compilation.md §2.2) needs typed
relationships to distinguish:

- **depends-on**: X requires Y to be satisfied (sequencing constraint)
- **affects**: X changes the interpretation of Y (impact relationship)
- **constrains**: X bounds the solution space of Y (negative case → invariant)
- **tests**: X verifies Y (verification relationship)

These are structurally different for CALM classification (Pass 1 in the compilation
middle-end). A `depends-on` edge may be monotonic or non-monotonic depending on the
resolution modes of the shared attributes. An `affects` edge is always non-monotonic
(it changes interpretation). A `constrains` edge is monotonic (it only narrows).

**Proposed attributes** (Layer 2 additions):

```
:spec/affects          — Ref        :many   multi  — X affects interpretation of Y
:spec/constrains       — Ref        :many   multi  — X bounds solution space of Y
:spec/tests            — Ref        :many   multi  — X verifies Y
```

**Why these are Layer 2, not Layer 4**: These describe relationships between spec elements,
which are Layer 2 entities. They do not reference coordination entities (Layer 4). The
dependency ordering constraint (INV-SCHEMA-006: Layer N references only Layers 0..N) is
satisfied.

**DDIS Spec Element**:

```
### INV-SCHEMA-009: Spec Dependency Graph Completeness

**Traces to**: SEED.md §4 (C7: self-bootstrap), exploration/11-topology-as-compilation.md §2.2
**Type**: Invariant
**Stage**: 0

#### Level 0 (Algebraic Law)
For every spec element e with a prose dependency on spec element e':
  ∃ datom (e, :spec/depends-on | :spec/affects | :spec/constrains | :spec/tests, e', tx, assert)

The spec dependency graph G_spec = (V, E) where:
  V = {e | ∃ (e, :spec/type, _, _, assert) ∈ S}
  E = {(e, e', type) | ∃ (e, :spec/{type}, e', _, assert) ∈ S
       where type ∈ {depends-on, affects, constrains, tests}}

G_spec must be a connected graph (excluding uncertainty elements).

#### Level 1 (State Invariant)
After self-bootstrap (Phase 2), G_spec has:
  |V| = total spec elements transacted
  |E| ≥ |V| - 1  (at minimum, a spanning tree)

#### Level 2 (Implementation Contract)
The bootstrap EDNL file generator must extract dependency relationships from
spec prose (explicit cross-references like "Depends on INV-STORE-001") and
emit corresponding :spec/depends-on ref datoms.

**Falsification**: A spec element with a prose dependency ("Depends on X" or
"Traces to X" where X is another spec element) that has no corresponding
:spec/depends-on, :spec/affects, :spec/constrains, or :spec/tests ref datom.

**Verification**: V:PROP — After bootstrap, query for all spec elements. For each,
check that every cross-reference in its prose has a corresponding ref datom.

**proptest strategy**: Generate random spec element sets with known dependency
graphs. Bootstrap them. Assert the dependency graph is recovered exactly.
```

---

### F1-D: ADR-SCHEMA-007 — Typed vs Untyped Spec Relationships

```
### ADR-SCHEMA-007: Typed Spec Element Relationships

**Traces to**: SEED.md §4 (C7), exploration/11-topology-as-compilation.md §2.2
**Stage**: 0

#### Problem
Spec elements have relationships to each other. Currently only :spec/depends-on
exists. Should relationships be typed (separate attributes per relationship type)
or untyped (single :spec/depends-on for all relationships)?

#### Options
A) **Single untyped attribute** (:spec/depends-on for everything)
   - Pro: Simpler schema, fewer attributes
   - Con: Cannot distinguish sequencing constraints from impact relationships
     in Datalog queries without additional annotation
   - Con: CALM classification (monotonic vs non-monotonic edges) requires
     relationship type information

B) **Four typed relationship attributes** (:spec/depends-on, :spec/affects,
   :spec/constrains, :spec/tests)
   - Pro: Typed relationships are directly queryable in Datalog
   - Pro: CALM classification can use relationship type as input
   - Pro: Compilation front-end can build annotated Coupling IR
   - Con: More attributes (4 vs 1) in Layer 2 schema

C) **Relationship entity with type attribute** (separate entity per relationship)
   - Pro: Maximum extensibility (new relationship types without schema changes)
   - Con: Three datoms per relationship instead of one (entity overhead)
   - Con: Queries become more complex (join through relationship entity)

#### Decision
**Option B.** Four typed attributes. The relationship types are structurally
different for CALM classification:
  - depends-on: may be monotonic or non-monotonic
  - affects: always non-monotonic (changes interpretation)
  - constrains: always monotonic (narrows solution space)
  - tests: always monotonic (verification is additive)

The type information is needed at query time for compilation (doc 11, Pass 1).
Encoding it in the attribute name avoids an extra join. Four attributes is a
small addition to the 23 existing Layer 2 spec attributes.

#### Consequences
- Three new Layer 2 attributes: :spec/affects, :spec/constrains, :spec/tests
- :spec/depends-on already exists (no change needed)
- Bootstrap EDNL generator must emit typed relationships
- Compilation front-end queries become simple attribute pattern matches
- CALM classification for the compilation middle-end gets relationship type for free
```

---

### F1-E: NEG-BOOTSTRAP-001 — Content-Only Bootstrap

```
### NEG-BOOTSTRAP-001: Content-Only Bootstrap Produces Flat Store

**Traces to**: SEED.md §4 (C7), exploration/11-topology-as-compilation.md §2.2
**Type**: Negative case

**Statement**: A self-bootstrap that transacts spec element content (id, type,
statement, falsification) but NOT dependency relationships (:spec/depends-on,
:spec/affects, :spec/constrains, :spec/tests) produces a store where:
  - Spec elements exist as isolated entities
  - No dependency graph is queryable
  - The compilation front-end (doc 11 §2.2) has no input data
  - Contradiction detection cannot trace dependency chains
  - Impact analysis queries return empty results

**Violation condition**: After bootstrap, the query
  [:find (count ?e) :where [?e :spec/depends-on _]]
returns 0 while spec elements with prose dependencies exist.

**Safety property**: □(bootstrap_complete → dependency_edges > 0)
```

---

### F1-F: Update Bootstrap Worked Example

**Task**: Extend docs/guide/11-worked-examples.md Step 3 to show `:spec/depends-on`
ref datoms alongside content datoms. The worked example currently shows
INV-STORE-001 without dependencies. It should also show a second invariant
(e.g., INV-STORE-004) with an explicit `:spec/depends-on` reference back to
INV-STORE-001.

**Acceptance criteria**: The worked example demonstrates that after bootstrap:
1. Content is queryable: `[:find ?id :where [?e :spec/type "invariant"] [?e :spec/id ?id]]`
2. Dependencies are queryable: `[:find ?from ?to :where [?e :spec/id ?from] [?e :spec/depends-on ?d] [?d :spec/id ?to]]`
3. Transitive closure works: reachable spec elements from any root

---

### F1-G: Update Bootstrap EDNL Template

**Task**: Update docs/guide/00-architecture.md §0.3b Phase 2 to specify that the
bootstrap EDNL file MUST include dependency relationships extracted from spec
prose. Add a sentence: "The bootstrap EDNL generator parses cross-references
from spec markdown and emits corresponding :spec/depends-on, :spec/affects,
:spec/constrains, and :spec/tests ref datoms."

**Acceptance criteria**: Phase 2 description is unambiguous that dependency
edges are required, not optional.

---

## F2: Resolution Mode Extensibility [HIGH]

### F2-A: Audit Lattice Registration

**Finding**: ADR-SCHEMA-004 defines 12 named lattices (spec/02-schema.md lines
755–778). Lattices are datoms (`:lattice/ident`, `:lattice/elements`,
`:lattice/comparator`, `:lattice/bottom`, `:lattice/top`). Custom lattices
can be registered by transacting new lattice entities.

**Question**: Can the current lattice definition mechanism express:
1. The **topology lattice** (Solo ≤ Star ≤ Mesh, join toward more coordination)?
2. The **priority-max lattice** (numeric max)?
3. The **trust-level lattice** (bounded double with sigmoid accumulation)?

Priority-max is already lattice #12 (`numeric-max`). The topology lattice needs
a custom comparator (channel-set-inclusion). Trust-level may need a non-standard
lattice (bounded real interval [0, 1] with max join).

**Task**: Verify that the lattice definition mechanism is expressive enough for
coordination lattices. If the `:lattice/comparator` is a named function (string),
the topology comparator must be registerable.

---

### F2-B: Define Topology-Lifecycle Lattice (13th Named Lattice)

**Proposed lattice**:

```
topology-lifecycle lattice:
  elements: {:proposed, :compiled, :enacted, :superseded, :rolled-back}
  ordering: :proposed < :compiled < :enacted
            :proposed < :rolled-back  (terminal)
            :enacted < :superseded    (terminal)
  bottom: :proposed
  top: none (two terminal states)
  comparator: topology-lifecycle-join

Join semantics:
  :proposed ⊔ :compiled = :compiled  (progress)
  :proposed ⊔ :enacted  = :enacted   (skip compilation)
  :compiled ⊔ :enacted  = :enacted   (progress)
  :enacted  ⊔ :superseded = :superseded (new topology replaces)
```

**DDIS Spec Element**:

```
Update to ADR-SCHEMA-004: Add 13th lattice (topology-lifecycle)

Rationale: Topology entities (exploration/03-topology-definition.md) have a
lifecycle that must be lattice-resolved for CRDT correctness. Concurrent
assertions about topology status must join toward the most advanced state.
This is the same pattern as the existing 11 lifecycle lattices.

Stage: 2–3 (but lattice DEFINITION is data, so it can be registered at any
stage without code changes — this is the power of schema-as-data, C3)
```

---

### F2-D: ADR-SCHEMA-008 — Pre-register Coordination Lattices

```
### ADR-SCHEMA-008: Coordination Lattice Pre-Registration

**Traces to**: exploration/01-algebraic-foundations.md §5 (lattice of topologies)
**Stage**: 2–3 (registration); 0 (mechanism verification)

#### Problem
The topology framework (exploration docs 00–11) requires lattice resolution modes
for coordination attributes. Should these lattices be registered in Stage 0 or
deferred to Stage 2–3?

#### Options
A) **Register all coordination lattices in Stage 0 genesis**
   - Pro: Available from day one
   - Con: Adds ~5 lattices to genesis that aren't used until Stage 2–3
   - Con: Increases genesis datom count

B) **Register coordination lattices in Stage 2–3 via schema extension transactions**
   - Pro: No Stage 0 changes; follows schema-as-data (C3)
   - Con: Must verify the mechanism works (registering new lattices post-genesis)

C) **Verify mechanism in Stage 0; register in Stage 2–3**
   - Pro: Validates extensibility without adding unused data
   - Pro: Stage 0 test suite includes "register custom lattice" proptest
   - Con: Slightly more testing

#### Decision
**Option C.** The lattice registration mechanism must be verified working in
Stage 0 (a proptest that registers a custom lattice, transacts a datum using it,
and verifies resolution). Actual coordination lattices are registered when needed.

#### Consequences
- Stage 0 proptest: `custom_lattice_registration_and_resolution`
- No genesis changes
- Coordination lattice definitions documented here as forward reference
- Stage 2–3 implementation simply transacts lattice entities
```

---

### F2-E: Forward-Reference Layer 4 Resolution Modes

**Task**: Add a forward-reference section to spec/02-schema.md after the 12
named lattices (ADR-SCHEMA-004) noting that Layer 4 (Coordination) will require
additional lattices. List them as forward references:

```
Forward reference — Layer 4 Coordination Lattices (Stage 2–3):
  13. topology-lifecycle: :proposed < :compiled < :enacted
  14. channel-frequency: :none < :on-demand < :low < :medium < :high < :continuous
  15. coordination-intensity: numeric ordering (merge overhead metric)
  16. trust-level: bounded-real [0.0, 1.0] with max join

These follow the same pattern as lattices 1–12 and are registered via the
standard lattice entity mechanism (INV-SCHEMA-007).
```

**Acceptance criteria**: The forward reference exists in the spec, is clearly
marked as Stage 2–3, and does not create any Stage 0 implementation obligation.

---

## F3: Agent Entity First-Class [HIGH]

### F3-A: ADR-STORE-020 — Agent Entity ID Scheme

```
### ADR-STORE-020: Agent Entity Identification

**Traces to**: SEED.md §4 (Axiom 1: identity), exploration/03-topology-definition.md §1
**Stage**: 0

#### Problem
Agents are referenced by :tx/agent (Ref) in every transaction. What is the
EntityId of an agent entity? Content-addressed identity (INV-STORE-002) requires
that the EntityId be derived from content. But what content identifies an agent?

#### Options
A) **Hash of agent name string** — EntityId = BLAKE3("agent:" + name)
   - Pro: Simple, deterministic, human-readable name
   - Con: Name collision across deployments; renaming destroys identity

B) **Hash of (program, model, instance-salt)** — EntityId = BLAKE3(program + model + salt)
   - Pro: Distinguishes agent types (claude-code/opus-4.6 vs codex/o3)
   - Con: instance-salt must be managed; same model in different instances
     gets different IDs (which may be desired or not)

C) **Hash of agent configuration datoms** — EntityId = BLAKE3(canonical serialization
   of the agent's initial assertion datoms)
   - Pro: Consistent with INV-STORE-002 (content-addressed)
   - Pro: Agent identity IS its first assertion set
   - Con: Slightly more complex; requires canonical serialization order

D) **Hash of (program, model, deployment-context)** — EntityId = BLAKE3(program + model + project-path)
   - Pro: Same agent type in same project = same entity (natural dedup)
   - Pro: Different deployments get different entities (isolation)
   - Con: Moving project path changes agent identity

#### Decision
**Option D.** Agent identity is BLAKE3(program + model + deployment-context).

Rationale:
- Content-addressed (INV-STORE-002 compliant)
- Deterministic (same agent type in same deployment = same EntityId)
- Natural deduplication (two sessions of claude-code/opus-4.6 on ddis-braid
  reference the same agent entity)
- Deployment isolation (same agent type on different projects = different entities)
- Aligns with how :tx/agent is used: the agent IS the (program, model, context)
  tuple, not the ephemeral session

Consequences:
- Agent entities created lazily on first transaction from that (program, model, context)
- Genesis creates SYSTEM_AGENT with fixed EntityId = BLAKE3("system" + "braid" + "genesis")
- :agent/ident attribute stores human-readable name
- :agent/program and :agent/model attributes store the ID components
- Renaming doesn't change EntityId (hash is from canonical components)
```

---

### F3-B: Define Minimal Layer 1 Agent Schema

**Finding**: Layer 1 currently specifies "2 types, 16 attributes" (spec/02-schema.md
line 306) but the actual attributes listed are only `:tx/time`, `:tx/agent`,
`:tx/provenance`. The remaining Layer 1 attributes are referenced in ADRS SR-008
but not enumerated in the schema spec.

**Task**: Ensure the Layer 1 schema explicitly includes agent entity attributes:

```
Agent entity attributes (Layer 1):
  :agent/ident          — Keyword    :one    lww    — Human-readable agent name
  :agent/program        — Keyword    :one    lww    — Harness (claude-code, codex, gemini, human)
  :agent/model          — Keyword    :one    lww    — LLM model (opus-4.6, sonnet-4.6, o3, human)

Forward reference — Layer 4 extensions (Stage 2–3):
  :agent/capabilities   — Keyword    :many   multi  — Domain competencies
  :agent/frontier       — Ref        :many   multi  — Set of latest known TxIds
  :agent/trust          — Double     :one    lww    — Trust score T ∈ [0, 1]
  :agent/status         — Keyword    :one    lattice — agent-lifecycle lattice
```

**DDIS Spec Element**:

```
### INV-STORE-015: Agent Entity Completeness

**Traces to**: SEED.md §4 (Axiom 1), exploration/03-topology-definition.md §1
**Type**: Invariant
**Stage**: 0

#### Level 0 (Algebraic Law)
Every :tx/agent Ref in the store points to a valid agent entity:
  ∀ d = (_, :tx/agent, agent_ref, _, _) ∈ S:
    ∃ d' = (agent_ref, :agent/ident, _, _, assert) ∈ S

#### Level 1 (State Invariant)
After genesis, at least SYSTEM_AGENT exists:
  |{e | (e, :agent/ident, _, _, assert) ∈ S₀}| ≥ 1

#### Level 2 (Implementation Contract)
Store::transact() ensures that if :tx/agent references an EntityId not yet in
the store, the transaction ALSO includes assertion datoms creating that agent entity
with at minimum :agent/ident, :agent/program, :agent/model.

**Falsification**: A :tx/agent Ref that does not resolve to an entity with
:agent/ident in the store.

**Verification**: V:PROP — After any sequence of transactions, every :tx/agent
ref resolves to an agent entity. V:KANI — transact postcondition includes
agent entity existence.

**proptest strategy**: Generate random transaction sequences with varying agent
IDs. Assert every :tx/agent ref resolves.
```

---

### F3-D: Verify :tx/agent Is Ref to Agent Entity

**Finding**: spec/02-schema.md line 124 defines `:tx/agent — Ref :one` and
spec/01-store.md line 179 defines `agent: AgentId` in the Transaction algebra.
The Ref type points to an EntityId.

**Question**: Is AgentId a type alias for EntityId? Or is it a separate type?

**Task**: Verify that AgentId IS EntityId (content-addressed) in the spec.
If AgentId is currently a separate type (e.g., a string name), this must be
changed to EntityId for INV-STORE-015 and the topology framework to work.

**Acceptance criteria**: The spec is unambiguous that :tx/agent is a Ref to
an agent entity, not a bare keyword or string.

---

### F3-E: System Agent in Genesis

**Task**: Verify or add that the genesis transaction creates a SYSTEM_AGENT entity.
Currently genesis (spec/01-store.md lines 261-268) creates "17 axiomatic attributes"
but does not mention creating an agent entity.

**Proposed addition to genesis**:

```
GENESIS() → S₀

POST:
  S₀.datoms = {meta_schema_datoms} ∪ {system_agent_datoms}
  where system_agent_datoms = {
    (SYSTEM_AGENT, :agent/ident, :system, tx_0, assert),
    (SYSTEM_AGENT, :agent/program, :braid, tx_0, assert),
    (SYSTEM_AGENT, :agent/model, :system, tx_0, assert),
  }
  SYSTEM_AGENT = BLAKE3("system" + "braid" + "genesis")
```

**Rationale**: tx_0 (genesis) needs a :tx/agent. That agent must be a valid entity
(INV-STORE-015). Therefore genesis must create SYSTEM_AGENT.

---

## F4: Frontier Queryable Data Model [HIGH]

### F4-A: ADR-STORE-021 — Frontier Representation

```
### ADR-STORE-021: Frontier Representation

**Traces to**: SEED.md §4 (Axiom 3: snapshots/frontiers),
  exploration/07-fitness-function.md §3.3 (D3 staleness)
**Stage**: 0 (data model); 3 (multi-agent usage)

#### Problem
An agent's frontier is the set of datoms it knows about. How should this be
represented in the store for queryability?

#### Options
A) **Set of TxIds** — frontier(α) = {tx₁, tx₂, ..., txₙ} = all transactions α has seen.
   visible(α) = {d ∈ S | d.tx ∈ frontier(α)}
   - Pro: Exact; every datom is attributable to a transaction
   - Pro: Staleness = |{tx ∈ S | tx ∉ frontier(α)}| / |{tx ∈ S}|
   - Con: Large frontier for long-running agents (grows with transaction count)

B) **High-water-mark TxId** — frontier(α) = max(tx seen by α)
   visible(α) = {d ∈ S | d.tx ≤ frontier(α)} (using HLC ordering)
   - Pro: Compact (single value)
   - Con: Assumes total ordering of transactions; not true in concurrent multi-agent
     (agent A's tx₅ is incomparable with agent B's tx₃ under HLC partial order)

C) **Per-agent high-water-mark (vector clock style)** —
   frontier(α) = {(β, max_tx_β) | β ∈ agents, max_tx_β = latest tx from β seen by α}
   - Pro: Compact (one entry per agent)
   - Pro: Comparison is pointwise: α is ahead of β on agent γ iff frontier(α)[γ] ≥ frontier(β)[γ]
   - Pro: Staleness = number of entries where frontier(α)[β] < latest(β)
   - Con: Requires knowing the set of all agents (grows with agent count, not tx count)

#### Decision
**Option C.** Per-agent vector clock. Each agent's frontier is a map from agent
IDs to the latest transaction from that agent that the current agent has seen.

Rationale:
- Compact (one entry per agent, not per transaction)
- Directly supports staleness computation (D3 in F(T))
- Comparison is efficient (pointwise max)
- Merge is pointwise max (CRDT: vector clocks form a join-semilattice)
- Consistent with HLC causality (if α saw β's tx₅, α also saw β's tx₁..tx₄)
- In single-agent Stage 0, frontier = {(self, latest_tx)} — trivially degenerate

Consequences:
- :agent/frontier stores per-agent-latest-tx pairs
- Frontier merge: pointwise max of vector clock entries (join-semilattice)
- Staleness query: count agents where frontier[agent] < that agent's latest tx
- Single-agent: frontier always up-to-date (staleness = 0)
```

---

### F4-B: Define :agent/frontier Attribute

**Proposed Layer 1 attribute** (forward reference, populated Stage 3):

```
:agent/frontier     — Ref        :many   multi  — Per-agent latest TxId entries
```

**Issue**: A vector clock is a map (AgentId → TxId), not a flat set of Refs.
Options for encoding:
1. **Compound entity**: Each frontier entry is a separate entity
   `(entry, :frontier/agent, agent_ref, tx, assert)` +
   `(entry, :frontier/tx, tx_ref, tx, assert)`
2. **Tuple value**: `:agent/frontier` of type `Tuple [Ref, Ref]` = (agent, tx) pairs
3. **JSON value**: `:agent/frontier` of type `Json` = serialized vector clock

**Recommendation**: Option 1 (compound entity) is most consistent with the datom model.
Each frontier entry is a first-class entity, queryable via Datalog joins.
Resolution mode: LWW per (agent, frontier-agent) pair — latest observation wins.

**DDIS Spec Element**:

```
### INV-STORE-016: Frontier Computability

**Traces to**: SEED.md §4 (Axiom 3), exploration/07-fitness-function.md §3.3
**Type**: Invariant
**Stage**: 0 (definition); 3 (multi-agent usage)

#### Level 0 (Algebraic Law)
For any agent α and store S:
  staleness(α, S) = |{β ∈ agents(S) | frontier(α)[β] < latest(β, S)}| / |agents(S)|

staleness(α, S) ∈ [0, 1]
staleness(α, S) = 0 ⟺ α is fully up-to-date with all agents

#### Level 1 (State Invariant)
In single-agent mode (Stage 0):
  |agents(S)| = 1
  frontier(self)[self] = latest(self, S)  — always
  staleness(self, S) = 0                  — always

#### Level 2 (Implementation Contract)
staleness(α, S) is computable via a Datalog query at Stratum 0 (monotonic,
coordination-free).

**Falsification**: staleness(α, S) requires a non-monotonic query (Stratum 2+)
or cannot be expressed in Datalog.

**Verification**: V:PROP — In single-agent mode, staleness is always 0.
V:PROP — In simulated multi-agent mode, staleness correctly reflects
the gap between an agent's frontier and the global state.
```

---

## F5: Spectral Computation in Query Engine [MEDIUM]

### F5-A: Add Laplacian Computation

**Finding**: spec/03-query.md Stratum 3 specifies linear algebra FFI (SVD for
spectral authority). The topology framework additionally needs:

1. **Graph Laplacian**: L = D - A (degree matrix minus adjacency matrix)
2. **Fiedler vector**: second-smallest eigenvector of L (for spectral bisection)
3. **Spectral gap**: eigenvalue gap indicating cluster count

These are standard operations in the same linear algebra FFI (nalgebra or equivalent).

**DDIS Spec Element**:

```
### INV-QUERY-022: Spectral Computation Correctness

**Traces to**: exploration/06-cold-start.md §5.1 (spectral partitioning),
  exploration/11-topology-as-compilation.md §2.3.3 (Pass 3)
**Type**: Invariant
**Stage**: 3 (usage); 0 (FFI foundation)

#### Level 0 (Algebraic Law)
For any symmetric adjacency matrix A:
  L = D - A  where D = diag(row_sums(A))
  eigenvalues(L) are real and non-negative (L is positive semi-definite)
  eigenvalue_0 = 0 (always, for connected graphs)
  Fiedler vector = eigenvector corresponding to eigenvalue_1

#### Level 1 (State Invariant)
The number of zero eigenvalues of L equals the number of connected components
in the graph defined by A.

#### Level 2 (Implementation Contract)
pub fn graph_laplacian(adjacency: &Matrix) -> Matrix;
pub fn fiedler_vector(laplacian: &Matrix) -> Vector;
pub fn spectral_partition(adjacency: &Matrix, k: usize) -> Vec<Vec<NodeId>>;

All return deterministic results for the same input.

**Falsification**: spectral_partition returns a partition where two nodes in
the same cluster have no path between them in the original graph.

**Verification**: V:PROP — For random adjacency matrices, verify:
  1. Laplacian has non-negative eigenvalues
  2. Number of zero eigenvalues = number of connected components
  3. Spectral partition preserves intra-cluster connectivity
```

---

### F5-C: ADR-QUERY-012 — Spectral Operations via nalgebra FFI

```
### ADR-QUERY-012: Spectral Graph Operations

**Traces to**: exploration/06-cold-start.md §5, exploration/11-topology-as-compilation.md §2.3
**Stage**: 0 (FFI infrastructure); 3 (spectral partitioning usage)

#### Problem
The topology framework requires spectral graph operations (Laplacian, eigendecomposition,
Fiedler vector) for cluster identification. How should these be implemented?

#### Options
A) **nalgebra FFI** — Use nalgebra crate's eigendecomposition
   - Pro: Mature, well-tested, pure Rust, no C dependency
   - Pro: Already planned for Stratum 3 SVD (spectral authority)
   - Con: nalgebra eigendecomposition is O(n³); acceptable for n ≤ 50

B) **Custom implementation** — Hand-roll power iteration for Fiedler vector
   - Pro: Optimized for the specific use case (only need 2nd eigenvector)
   - Con: Reinventing tested linear algebra; error-prone

C) **lapack-sys FFI** — Call LAPACK's dsyev via FFI
   - Pro: Maximum performance
   - Con: C dependency; complicates build; overkill for n ≤ 50

#### Decision
**Option A.** nalgebra for all linear algebra operations (SVD, eigendecomposition,
Laplacian computation). The Stratum 3 FFI boundary already exists for spectral
authority. Spectral partitioning reuses the same infrastructure.

Consequences:
- nalgebra dependency justified by two use cases (authority + topology)
- graph_laplacian, fiedler_vector, spectral_partition added to query/graph.rs
- Stratum 3 capability list extended to include spectral operations
- Stage 0 builds the FFI; Stage 3 uses it for topology
```

---

## F6: Trilateral → Quadrilateral Extension Point [LOW]

### F6-A: Reserve TOPO_ATTRS Namespace

**Finding**: spec/18-trilateral.md defines three disjoint attribute namespaces:
INTENT_ATTRS, SPEC_ATTRS, IMPL_ATTRS (lines 143–160). INV-TRILATERAL-005
requires these to be pairwise disjoint.

**Task**: Add a forward-reference reserving the TOPO_ATTRS namespace for
topology-specific attributes. Stage 0 does not populate this namespace.

```
Forward reference — Topology namespace (Stage 3):
  TOPO_ATTRS ∩ INTENT_ATTRS = ∅
  TOPO_ATTRS ∩ SPEC_ATTRS   = ∅
  TOPO_ATTRS ∩ IMPL_ATTRS   = ∅

  TOPO_ATTRS will include: :topo/type, :topo/agents, :topo/channels,
    :topo/fitness, :topo/coupling, :topo/compiled-from, etc.
```

---

### F6-C: ADR-TRILATERAL-004 — N-lateral Extensibility

```
### ADR-TRILATERAL-004: N-Lateral Extensibility

**Traces to**: exploration/01-algebraic-foundations.md §7 (quadrilateral),
  exploration/07-fitness-function.md §7 (F_total composition)
**Stage**: 0 (design); 3 (topology vertex implementation)

#### Problem
The trilateral model (Intent ↔ Spec ↔ Impl) has three vertices. The topology
exploration (doc 01 §7, doc 07 §7.3) proposes a fourth vertex (Topology)
creating a quadrilateral. Should the model be designed for exactly four vertices,
or for N vertices?

#### Options
A) **Hardcode quadrilateral** (4 vertices: Intent, Spec, Impl, Topology)
   - Pro: Simple; matches current needs
   - Con: If a fifth vertex emerges (e.g., Deployment, User), requires redesign

B) **N-lateral model** (parameterized over vertex count)
   - Pro: Extensible; adding vertices doesn't change the algebra
   - Con: Slightly more abstract

#### Decision
**Option B.** The divergence metric generalizes to N vertices:

  Φ(S) = Σᵢ wᵢ × D_i(S)

where i ranges over all adjacent boundary pairs. The trilateral case has 2 boundaries
(IS, SP). The quadrilateral adds 2 more (PT, TI — topology↔impl, topology↔intent).
The N-lateral case has N boundaries for an N-gon.

Each boundary requires:
  1. A LIVE projection (partition of attribute namespace)
  2. A divergence function D_i computing gap across the boundary
  3. A weight w_i (stored as a datom, tunable)

The existing INV-TRILATERAL-005 (namespace partitioning) generalizes naturally:
  all LIVE projections are pairwise disjoint.

#### Consequences
- INV-TRILATERAL-005 phrasing updated to "all N projections are pairwise disjoint"
- Φ(S) formula generalized to weighted sum over N-1 boundaries
- Adding a vertex requires: define ATTRS, implement D_i, register w_i
- No existing code changes — current 3-vertex case is a specialization
- Stage 3 adds the 4th vertex (topology) via this extension mechanism
```

---

### F6-D: Ensure Φ(S) Is Algebraically Extensible

**Finding**: spec/18-trilateral.md defines Φ(S) = w₁ × D_IS(S) + w₂ × D_SP(S)
with exactly two terms.

**Task**: Rewrite the algebraic law (Level 0) to use summation notation:

```
Current:  Φ(S) = w₁ × D_IS(S) + w₂ × D_SP(S)
Proposed: Φ(S) = Σᵢ wᵢ × Dᵢ(S)  where i ∈ boundaries(S)
          boundaries(S₀) = {IS, SP}  (trilateral initial state)
```

This is a non-breaking change: the existing formula is a specialization with
|boundaries| = 2. The generalization allows adding boundaries without modifying
the formula.

**Acceptance criteria**: Level 0 algebraic law uses summation notation. Level 1
state invariant verifies |boundaries| ≥ 2. Level 2 implementation uses a loop
over boundary definitions, not hardcoded IS/SP.

---

## Verification Matrix

| Foundation | New INVs | New ADRs | New NEGs | Spec Files Touched | Guide Files Touched |
|------------|----------|----------|----------|-------------------|-------------------|
| F1 | INV-SCHEMA-009 | ADR-SCHEMA-007 | NEG-BOOTSTRAP-001 | 02-schema.md | 00-architecture.md, 11-worked-examples.md |
| F2 | — | ADR-SCHEMA-008 | — | 02-schema.md | — |
| F3 | INV-STORE-015 | ADR-STORE-020 | — | 01-store.md, 02-schema.md | 00-architecture.md |
| F4 | INV-STORE-016 | ADR-STORE-021 | — | 01-store.md | — |
| F5 | INV-QUERY-022 | ADR-QUERY-012 | — | 03-query.md | 00-architecture.md |
| F6 | — | ADR-TRILATERAL-004 | — | 18-trilateral.md | 13-trilateral.md |
| **Total** | **4** | **6** | **1** | **5 spec files** | **3 guide files** |

---

## Resolved Questions

### OQ-1: Agent Entity ID — RESOLVED
**Decision**: BLAKE3(program + model + session_context). Each agent instance is
a distinct entity. Session context disambiguates concurrent sessions of the same
agent type on the same project.

### OQ-2: Layer 1 vs Layer 4 for Agent Attributes — RESOLVED
**Decision**: **Layer 1** (Stage 0). Agent attributes (:agent/ident, :agent/program,
:agent/model) are provenance infrastructure, not coordination logic. Every transaction
has :tx/agent (Ref). Making it reference a structured entity from day one avoids a
future migration from unstructured strings to refs. The schema cost is 3 attributes.

### OQ-3: Frontier Encoding — RESOLVED
**Decision**: **Compound entity** (Option A). Each frontier entry is a first-class
entity with :frontier/agent (Ref) and :frontier/tx (Ref). This is the most
DDIS-native encoding — frontiers are facts, facts are datoms, datoms are queryable
via standard Datalog joins. Entity proliferation is bounded (one per agent per
session). Resolution: LWW per (agent, frontier-agent) pair.

### OQ-4: ID Conflicts — RESOLVED
All IDs in the body text updated to corrected, non-conflicting values:
INV-QUERY-022, ADR-STORE-020, ADR-STORE-021, ADR-QUERY-012.

### OQ-5: Attribute Naming — RESOLVED
**Decision**: **`:spec/affects`**. Active voice, consistent with existing naming
pattern (:spec/traces-to, :spec/depends-on). "Impacts" implies severity judgment
that doesn't belong in a structural relationship.
