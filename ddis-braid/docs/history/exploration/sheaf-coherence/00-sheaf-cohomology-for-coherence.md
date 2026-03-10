# 00 — Sheaf Cohomology as a Formal Verification Framework for Coherence

> **Status**: EXPLORATION
> **Date**: 2026-03-09
> **Traces to**: SEED.md §1 (coherence verification), §2 (divergence problem), §4 (datom algebra), §6 (reconciliation)
> **Builds on**: exploration/execution-topologies/01-algebraic-foundations.md (sheaf structure),
>   spec/18-trilateral.md (divergence metric Φ), spec/01-store.md (CRDT laws L1-L5)
>
> **Thesis**: The current divergence metric Φ measures the *magnitude* of incoherence (a scalar).
> Sheaf cohomology measures the *topology* of incoherence (a graded group). This distinction
> is the difference between "you have 47 problems" and "you have 3 independent structural
> contradictions, one of which cannot be resolved without coordinated changes across 5 entities."

---

## Table of Contents

1. [Why Cohomology, Not Just Counting](#1-why-cohomology)
2. [The Coherence Sheaf: Formal Construction](#2-coherence-sheaf)
3. [Čech Cohomology over the Agent Graph](#3-cech-cohomology)
4. [The ISP Triangle: Specification Bypass as H¹](#4-isp-triangle)
5. [Persistent Cohomology: The Coherence EKG](#5-persistent-cohomology)
6. [Computable Invariants and Complexity](#6-computability)
7. [Connection to Existing Braid Algebra](#7-braid-algebra)
8. [Implications for Guidance and Diagnostics](#8-guidance)
9. [Implementation Path](#9-implementation)
10. [What Is Genuinely Novel Here](#10-novelty)
11. [Open Questions](#11-open-questions)

---

## 1. Why Cohomology, Not Just Counting

<a name="1-why-cohomology"></a>

The trilateral coherence model (spec/18-trilateral.md) defines divergence as:

```
Φ(S) = w₁ × D_IS(S) + w₂ × D_SP(S)
```

This is a **scalar** — a single number measuring total gap across boundaries. It answers
"how much incoherence exists?" But it cannot answer:

- **Are the gaps independent or entangled?** Ten unlinked datoms could be ten isolated
  oversights (trivial to fix one at a time) or one structural contradiction that manifests
  across ten entities (requires coordinated resolution).

- **Can pairwise fixes resolve the problem?** Some incoherence patterns have the property
  that fixing any individual gap doesn't reduce total incoherence — the fix at one boundary
  creates a new gap at another. This is a *cohomological obstruction*.

- **What is the minimum coordinated change required?** When H¹ ≠ 0, there exists no
  sequence of local (pairwise) fixes that resolves the incoherence. The system must
  identify the *cycle* and resolve it as a unit.

**Analogy**: Φ is like measuring voltage. Cohomology is like measuring the topology of the
circuit. A voltage reading tells you there's a potential difference; circuit topology tells
you whether the current flows in a loop (which requires cutting the loop, not just adding
resistance at one point).

### The Fundamental Limitation of Scalar Metrics

Any scalar divergence metric has a fundamental blind spot: it cannot distinguish between
these two situations:

**Situation A** (independent gaps):
```
Intent₁ ←(gap)→ Spec₁       Intent₂ ←(gap)→ Spec₂
Spec₁   ←(ok) → Impl₁       Spec₂   ←(ok) → Impl₂
```
Φ = 2. Fix each gap independently. Resolution effort: O(n).

**Situation B** (cyclic obstruction):
```
Intent₁ ←(gap)→ Spec₁       Spec₁ ←(ok)→ Impl₁
Impl₁   ←(gap)→ Intent₁     (Impl₁ contradicts Intent₁ through Spec₁)
```
Φ = 2. But fixing the I→S gap may break the S→P link (because the spec needs to change
to match the intent, but the implementation was built against the old spec). Resolution
effort: O(n²) or worse — requires coordinated change.

H¹ = 0 for Situation A (no cycles). H¹ ≠ 0 for Situation B (cycle). The cohomology
*sees the structure that the scalar misses*.

---

## 2. The Coherence Sheaf: Formal Construction

<a name="2-coherence-sheaf"></a>

### 2.1 Setup: The Coherence Space

We work in two parallel settings:

**Setting 1: Multi-Agent Coherence** (agent frontiers)
- **Base space**: The agent communication graph G = (V, E) where V = agents, E = merge channels
- **Open cover**: For each agent α, the merge neighborhood N(α) = {α} ∪ {β : (α,β) ∈ E}
- **Sheaf**: The resolved-view presheaf V assigning to each agent its LIVE-resolved frontier

**Setting 2: Trilateral Coherence** (ISP boundaries)
- **Base space**: The coherence graph K = ({I, S, P}, {IS, SP}) — a path graph
  - Extended to K⁺ = ({I, S, P}, {IS, SP, IP}) when direct I→P links exist (cycle)
- **Open cover**: The three vertex stars {I ∪ IS, S ∪ IS ∪ SP, P ∪ SP}
- **Sheaf**: The boundary-view presheaf B assigning to each vertex its LIVE projection

Both settings share the same algebraic substrate: the datom store (P(D), ∪) with CRDT laws L1-L5.

### 2.2 The Resolved-View Presheaf (Multi-Agent)

For each agent α ∈ V, define:

```
V(α) = LIVE(F_α)
      = fold(causal_sort(F_α), apply_resolution)
```

where F_α is α's frontier (the set of all datoms α has received) and LIVE is the
INV-STORE-012 materialized view operation.

**Key property**: V is a deterministic pure function of F_α. Same frontier → same view.

For each edge (α, β) ∈ E, define the restriction:

```
ρ_αβ : V(α) → V(N(α) ∩ N(β))
```

The restriction computes the resolved view of the datoms that both α and β could
potentially see — the intersection of their merge neighborhoods.

### 2.3 Why CRDT Laws Give Sheaf Conditions

The topology exploration (01-algebraic-foundations.md §1.2) already proved that L1-L5
guarantee the sheaf conditions. Recapping with the precise connection:

| CRDT Law | Sheaf Axiom | Consequence |
|----------|-------------|-------------|
| L1 (Commutativity) | Restriction maps commute | Merge order doesn't affect resolved view |
| L2 (Associativity) | Gluing is unique | Three-way merge same regardless of grouping |
| L3 (Idempotency) | Sections consistent on overlaps | Re-merging is a no-op |
| L4 (Monotonicity) | Sections only refine | Information only grows |
| L5 (Growth-only) | Presheaf is flasque in the limit | Full convergence is reachable |

**Critical precision**: The sheaf conditions hold for the *datom set* (raw facts). They
do NOT automatically hold for the *resolved view* when using LWW resolution, because LWW
is order-dependent on tie-breaking. However:
- Content-addressed identity (C2) ensures that datoms are unique
- HLC timestamps ensure total ordering for LWW
- Given the same frontier, LWW resolution is deterministic

So V is a sheaf on the base space induced by the agent graph, with the caveat that
"sections" are resolved views, not raw datom sets.

### 2.4 The Boundary-View Presheaf (Trilateral)

For each vertex X ∈ {I, S, P}, define:

```
B(X) = LIVE_X(S)
     = project(S, {d ∈ S | d.a ∈ X_ATTRS})
```

where LIVE_X is the trilateral projection from spec/18-trilateral.md §18.1.

For each edge (X, Y) ∈ {IS, SP}, define the cross-boundary comparison:

```
σ_XY : B(X) × B(Y) → Δ_XY
```

where Δ_XY is the set of "boundary discrepancies" — entities in X without corresponding
links to Y, or vice versa. This is precisely D_XY(S) from the Φ formula.

The presheaf B is NOT a sheaf in the standard sense (the three views are disjoint by
INV-TRILATERAL-005, so there is no "overlap" in the traditional sense). Instead, the
cross-boundary links (:spec/traces-to, :spec/implements) play the role of the gluing
data. The cohomology we compute is the obstruction to *completing* these links into a
globally coherent assignment.

---

## 3. Čech Cohomology over the Agent Graph

<a name="3-cech-cohomology"></a>

### 3.1 The Cochain Complex

Given the agent communication graph G = (V, E) with |V| = n agents and |E| = m edges,
and the attribute space A (the set of all attributes in the schema), we construct:

**Cochain groups** (with coefficients in F₂ = {0, 1}, tracking agreement/disagreement):

```
C⁰(G, V) = Π_{α ∈ V} V(α)
          = the product of all agent views
          Dimension: n × |A|   (n agents, |A| attributes to resolve)

C¹(G, V) = Π_{(α,β) ∈ E} Δ(α, β)
          = the product of all pairwise disagreements
          Dimension: m × |A|

C²(G, V) = Π_{(α,β,γ) ∈ Triangles(G)} Δ(α, β, γ)
          = the product of all triple disagreements
          Dimension: t × |A|   (t = number of triangles in G)
```

where Δ(α, β) is the set of attributes on which α and β disagree after resolving
their respective frontiers:

```
Δ(α, β) = {a ∈ A | resolve(F_α, a) ≠ resolve(F_β, a)}
```

**Coboundary maps**:

```
δ⁰: C⁰ → C¹
(δ⁰v)_{αβ} = Δ(α, β)
            = {a ∈ A | v_α(a) ≠ v_β(a)}
            (attribute-wise comparison between adjacent agents)

δ¹: C¹ → C²
(δ¹d)_{αβγ} = d_{βγ} ⊕ d_{αγ} ⊕ d_{αβ}
             (symmetric difference of pairwise disagreements around a triangle)
```

### 3.2 Cohomology Groups

```
H⁰(G, V) = ker(δ⁰)
          = {v ∈ C⁰ | all adjacent agents agree on all attributes}
          = the globally consistent view (if it exists)

H¹(G, V) = ker(δ¹) / im(δ⁰)
          = pairwise disagreements that DON'T come from a simple
            "each agent has a different version" pattern
          = CYCLIC INCOHERENCE — disagreements that form loops

H²(G, V) = C² / im(δ¹)
          = obstructions to deforming one coherent state into another
          = STRUCTURAL RIGIDITY of the knowledge graph
```

### 3.3 Interpretation for DDIS

**H⁰ = 0** (trivial global section): No single resolved view is consistent across all
agents. This is the normal state during active work — agents have different frontiers.

**H⁰ ≠ 0** (nontrivial global section): All agents agree. This happens after a full sync
barrier (INV-SYNC-001). The *existence* of H⁰ after sync is guaranteed by CRDT convergence;
the *dimension* of H⁰ tells us how much of the resolved state is globally consistent.

**H¹ = 0**: All disagreements are "tree-like" — they can be resolved by sequential pairwise
merges in any order. The merge topology doesn't matter.

**H¹ ≠ 0**: There exist *cyclic disagreements* — agents α, β, γ where:
- α and β agree on attribute a₁
- β and γ agree on attribute a₂
- But γ and α disagree on a₃ in a way that's *not* a simple "different version" issue

This means: resolving the α-β disagreement may create a new β-γ disagreement, which
when resolved creates a new γ-α disagreement. **The incoherence circulates.**

**rank(H¹)** = the number of independent incoherence cycles. This is bounded by the
circuit rank of G: β₁(G) = m - n + 1 (number of independent cycles in the graph).

### 3.4 Worked Example: Three Agents, One Cycle

```
Agents: {α, β, γ}
Edges: {(α,β), (β,γ), (γ,α)}    — a triangle (cycle of length 3)
Attribute: :task/status for entity task:1

Frontiers:
  F_α = {(task:1, :task/status, :in-progress, tx:10, assert)}
  F_β = {(task:1, :task/status, :completed,   tx:12, assert)}
  F_γ = {(task:1, :task/status, :in-progress, tx:11, assert)}

LWW Resolution (highest tx wins):
  V(α)(task:1, :task/status) = :in-progress  (tx:10 is α's latest)
  V(β)(task:1, :task/status) = :completed    (tx:12 is β's latest)
  V(γ)(task:1, :task/status) = :in-progress  (tx:11 is γ's latest)
```

**Pairwise disagreements**:
```
Δ(α,β) = {:task/status}   (α says in-progress, β says completed)
Δ(β,γ) = {:task/status}   (β says completed, γ says in-progress)
Δ(γ,α) = {}               (γ and α both say in-progress, BUT different tx)
```

Wait — γ and α actually agree on the resolved value (:in-progress) even though they
have different underlying datoms. So:

```
δ⁰: v ↦ (Δ(α,β)=1, Δ(β,γ)=1, Δ(γ,α)=0)   in F₂
δ¹: d ↦ d_{βγ} ⊕ d_{γα} ⊕ d_{αβ} = 1 ⊕ 0 ⊕ 1 = 0   in F₂
```

Since δ¹(d) = 0, this 1-cochain is a cocycle. Is it a coboundary?

The image of δ⁰ consists of all vectors (Δ(α,β), Δ(β,γ), Δ(γ,α)) obtainable from
assigning each agent a single "correct" value and comparing. But no single assignment
produces the pattern (1, 1, 0): if we assign everyone the same value, all disagreements
vanish; if we assign α and γ one value and β another, we get (1, 1, 0) — which IS in the
image of δ⁰.

So H¹ = 0 for this example. The disagreement is "tree-like" even on a cycle, because
the cycle doesn't actually carry a nontrivial cocycle.

**When does H¹ ≠ 0?** When the pairwise disagreements are INCONSISTENT around a cycle:

```
Δ(α,β) = 1, Δ(β,γ) = 1, Δ(γ,α) = 1    — odd cycle
δ¹(d) = 1 ⊕ 1 ⊕ 1 = 1 ≠ 0             — NOT a cocycle
```

Actually in F₂, this fails the cocycle condition. Let me reconsider.

For F₂-coefficients on a triangle, H¹ ≅ F₂ when the cycle carries a nontrivial class.
The nontrivial cocycle is (1, 1, 1) — all three edges disagree. This represents a situation
where every pair of agents disagrees, and the disagreements form a consistent cycle (no
pair's disagreement is "explained" by a global assignment of correct values).

This happens when three agents each have a *different* resolved value for the same attribute,
and no single value is consistent with all pairwise views:

```
V(α)(a) = x,  V(β)(a) = y,  V(γ)(a) = z   where x ≠ y ≠ z ≠ x
```

In DDIS with LWW resolution, this CAN'T happen (LWW produces total order → at most 2
distinct values among any 3 agents). But with **multi-value** resolution or **lattice**
resolution, it CAN:

```
Multi-value resolution:
  V(α)(tags) = {A, B}
  V(β)(tags) = {B, C}
  V(γ)(tags) = {C, A}

  Each pair shares one tag but not the other.
  No single assignment is consistent with all three views.
  H¹ ≠ 0 — cyclic incoherence.
```

This is the precise mathematical detection of "circular disagreement" in multi-value
attributes — exactly the scenario where the deliberation protocol (DELIBERATION namespace)
is required.

---

## 4. The ISP Triangle: Specification Bypass as H¹

<a name="4-isp-triangle"></a>

### 4.1 The Core Insight

The most powerful application of sheaf cohomology in DDIS is not multi-agent coordination
(where CRDT laws make H¹ relatively well-behaved) but the **trilateral coherence model**.

Consider the coherence graph K = ({I, S, P}, edges):

**Ideal case** (all paths through spec):
```
K_tree = I ——— S ——— P       (path graph, no cycle)
```
H¹(K_tree) = 0 always. Pairwise coherence implies global coherence.
This means: if I↔S is coherent AND S↔P is coherent, then I↔S↔P is coherent.

**Reality** (direct implementation from intent):
```
K_cycle = I ——— S
           \   /
            P         (triangle, one cycle)
```
H¹(K_cycle) CAN BE ≠ 0. This is the **specification bypass** failure mode.

### 4.2 Formalizing Specification Bypass

An agent reads the intent (SEED.md, conversation, goals) and directly implements code
without first formalizing the intent as specification elements. This creates the I→P link.

If the spec also exists (written earlier, or by another agent), we have:
- I → S: intent formalized as spec (`:spec/traces-to` links)
- S → P: spec implemented as code (`:spec/implements` links)
- I → P: intent directly implemented as code (implicit, no formal link)

**The cohomological obstruction**: The code (P) was implemented from the intent (I) as
the agent understood it. But the spec (S) was also written from the intent, possibly by
a different agent or at a different time. If the spec's interpretation of the intent
differs from the code's interpretation, we have:

```
I →(traces-to)→ S →(implements)→ P₁    (spec-mediated path)
I →(directly-implements)→ P₂           (bypass path)

H¹ ≠ 0  iff  P₁ ≠ P₂
```

This is **exactly** when the spec is out of sync with both the intent and the implementation
in a way that no pairwise fix resolves: fixing I↔S may break S↔P, fixing S↔P may break
I↔S, and the I→P direct link constrains both.

### 4.3 Detection via Store Queries

The cohomological obstruction is detectable as a Datalog query:

```datalog
% Entities linked through both paths
dual_path(Intent, Spec, Impl) :-
    [Spec, :spec/traces-to, Intent, _, :assert],
    [Impl, :spec/implements, Spec, _, :assert],
    [Impl, :impl/source-intent, Intent, _, :assert].

% The ISP triangle has a cycle: check if the two paths agree
% This requires comparing the "effective spec" (what the impl actually follows)
% with the "formal spec" (what :spec/implements points to)
bypass_conflict(Intent, Spec, Impl) :-
    dual_path(Intent, Spec, Impl),
    [Spec, :spec/statement, SpecStatement, _, :assert],
    [Impl, :impl/behavior, ImplBehavior, _, :assert],
    not consistent(SpecStatement, ImplBehavior).
```

When `bypass_conflict` has solutions, H¹ ≠ 0 on the ISP triangle.

### 4.4 Why This Matters More Than Φ

The current Φ metric would see:
- D_IS: intent entities without `:spec/traces-to` → maybe 0 (spec exists!)
- D_SP: spec entities without `:spec/implements` → maybe 0 (impl exists!)
- **Φ = 0 even though the system is incoherent**

This is the critical blind spot: Φ measures link *existence*, not link *consistency*.
Sheaf cohomology measures consistency of the path around the cycle. Two different paths
from I to P (through S, and directly) must produce compatible results — if they don't,
H¹ ≠ 0, and no pairwise metric detects it.

**This is a genuine capability gap that cohomology fills.**

---

## 5. Persistent Cohomology: The Coherence EKG

<a name="5-persistent-cohomology"></a>

### 5.1 Cohomology Over Time

The datom store grows monotonically (C1, L5). At each transaction tx_i, we can compute:

```
H¹(tx_i) = the first cohomology group at transaction i
```

As the store grows, H¹ changes:
- **Birth**: A new H¹ generator appears when a cycle of inconsistency is created
  (e.g., an agent implements from intent without going through spec)
- **Death**: An H¹ generator disappears when the inconsistency is resolved
  (e.g., the spec is updated to match the implementation, or the implementation
  is corrected to match the spec)

### 5.2 The Persistence Diagram

The **persistence diagram** PD = {(birth_i, death_i)} records when each incoherence
cycle was born and when it died:

```
                  death
                    │
                    │           x (short-lived: typo fixed in next session)
                    │     x (medium: spec rewrite resolved in 3 sessions)
                    │
                    │
                    │                              x (LONG-LIVED: structural problem)
                    │
                    └───────────────────────────── birth
```

**Interpretation**:
- Points near the diagonal (short-lived): routine incoherence, work in progress. Normal.
- Points far from the diagonal (long-lived): **structural design problems**. These are
  incoherence cycles that persist across many transactions, many sessions, many agents.
  They represent fundamental contradictions in the system's design.

### 5.3 The Bottleneck Distance as Design Quality Metric

Given two persistence diagrams PD₁ (before a change) and PD₂ (after), the
**bottleneck distance** d_B(PD₁, PD₂) measures how much the "shape of incoherence"
changed:

```
d_B(PD₁, PD₂) = inf_γ sup_x ||x - γ(x)||_∞
```

where γ ranges over bijections between PD₁ and PD₂ (with diagonal points as padding).

**Use case**: After a major refactoring or spec revision, compute d_B to measure
whether the refactoring actually resolved structural problems (d_B large) or just
shuffled them around (d_B small, same persistence diagram up to translation).

### 5.4 Betti Numbers as Dashboard Metrics

The Betti numbers β_k = rank(H^k) give a concise summary:

```
β₀ = number of connected coherence components
     (should be 1 for a healthy project — everything connected)

β₁ = number of independent incoherence cycles
     (should be 0 for a converged project; during work, tracks structural problems)

β₂ = number of "voids" in the coherence structure
     (obstructions to filling in missing relationships; rare but diagnostic)
```

A **coherence dashboard**:
```
┌─────────────────────────────────────────────┐
│  COHERENCE STATUS                           │
│                                             │
│  Φ (divergence):     47  (▼ from 52)        │
│  β₀ (components):    1   (connected ✓)      │
│  β₁ (cycles):        3   (▲ from 2)         │
│  H¹ generators:                             │
│    [1] INV-STORE-001 ↔ impl/store.rs ↔ SEED §4  (age: 12 txns)  │
│    [2] ADR-QUERY-005 ↔ impl/query.rs ↔ SEED §3  (age: 47 txns)  ←── STRUCTURAL │
│    [3] INV-HARVEST-003 ↔ impl/harvest.rs ↔ SEED §5  (age: 2 txns)  │
│                                             │
│  Recommendation: Generator [2] is long-lived│
│  (47 txns). This suggests ADR-QUERY-005's   │
│  decision conflicts with the implementation │
│  AND the original intent. Coordinated       │
│  resolution required — pairwise fixes will  │
│  not converge.                              │
└─────────────────────────────────────────────┘
```

---

## 6. Computable Invariants and Complexity

<a name="6-computability"></a>

### 6.1 Finite Computability

For a finite graph G with n vertices, m edges, and t triangles, Čech cohomology is
computed by linear algebra over F₂:

```
δ⁰: F₂^(n×|A|) → F₂^(m×|A|)     — incidence matrix ⊗ identity
δ¹: F₂^(m×|A|) → F₂^(t×|A|)     — triangle boundary matrix ⊗ identity

H⁰ = ker(δ⁰)                     — Gaussian elimination: O(n²m|A|)
H¹ = ker(δ¹) / im(δ⁰)           — rank computation: O(m²t|A|)
```

For typical DDIS deployments:
- n ≤ 10 agents (even in ambitious multi-agent setups)
- m ≤ 45 edges (complete graph on 10 agents)
- t ≤ 120 triangles
- |A| ≤ 100 attributes (schema Layer 0-2)

**Total cost**: O(10⁴) operations — trivially fast. Microseconds on modern hardware.

### 6.2 Persistent Cohomology Complexity

Standard persistence algorithm: O(n³) where n = number of simplices.

For our application: n = |V| + |E| + |Triangles| ≤ 175, so O(175³) ≈ 5 × 10⁶ — still trivially fast.

The persistence computation needs to run at each transaction (to track birth/death).
With incremental updates (only recompute affected simplices), the amortized cost per
transaction is much lower.

### 6.3 Over Non-Binary Coefficients

Using F₂ coefficients captures agreement/disagreement but loses the *degree* of
disagreement. For richer information:

**Z-coefficients** (integer cohomology): Track the number of distinct values in
disagreement, not just whether disagreement exists. H¹(G; Z) detects "how badly"
the cycle disagrees.

**R-coefficients** (real cohomology): For continuous attributes (like divergence
scores, confidence levels), use real-valued cochains. H¹(G; R) gives a continuous
measure of cyclic incoherence.

The computation is identical (linear algebra), just over different fields. The
complexity doesn't change.

### 6.4 Relationship to Graph Theory

For a connected graph G with β₁(G) = m - n + 1 cycle rank:

```
rank(H¹(G; F₂)) ≤ β₁(G)
```

Equality holds when every cycle in G carries a nontrivial disagreement. The ratio
rank(H¹) / β₁ measures "what fraction of the communication graph's cycles are
incoherent."

For a tree (β₁ = 0): H¹ = 0 always. Trees can't have cyclic incoherence.
For a complete graph on n agents: β₁ = n(n-1)/2 - n + 1 = (n-1)(n-2)/2.

**Design implication**: Tree-topology communication (star, pipeline, hierarchy) has
H¹ = 0 by construction. Only mesh and ring topologies can produce cyclic incoherence.
This gives a topology-aware coherence guarantee: if your agent graph is acyclic,
pairwise coherence checks are sufficient.

---

## 7. Connection to Existing Braid Algebra

<a name="7-braid-algebra"></a>

### 7.1 Relationship to Φ

The divergence metric Φ and the cohomology H¹ are complementary, not competing:

| Aspect | Φ (Divergence) | H¹ (Cohomology) |
|--------|----------------|-----------------|
| Type | Scalar (f64) | Graded group (vector space) |
| Measures | Magnitude of gaps | Topology of gaps |
| Detects | Missing links | Inconsistent cycles |
| Blind spot | Cyclic incoherence | Isolated gaps |
| Computational cost | O(|S|) — linear scan | O(n²m) — matrix rank |
| Update model | Incremental (per transaction) | Incremental (per transaction) |

The combined diagnostic:
```
(Φ, H¹) together characterize coherence completely:
  Φ = 0, H¹ = 0  →  fully coherent (ideal)
  Φ > 0, H¹ = 0  →  gaps exist but are all pairwise-resolvable (routine work)
  Φ = 0, H¹ ≠ 0  →  all links exist but form inconsistent cycles (DANGEROUS — looks coherent but isn't)
  Φ > 0, H¹ ≠ 0  →  gaps exist AND some are structurally entangled (requires coordinated resolution)
```

The (Φ = 0, H¹ ≠ 0) case is the critical one: it's invisible to Φ but detected by H¹.

### 7.2 Relationship to Spectral Analysis

The spectral graph operations (INV-QUERY-022, ADR-QUERY-012) in spec/03-query.md compute
the Laplacian and Fiedler vector of the dependency graph. The connection to cohomology:

```
The graph Laplacian L = D - A is the matrix representation of δ⁰ᵀδ⁰ (the Hodge Laplacian).

Eigenvalues of L:
  λ₀ = 0                    (multiplicity = β₀ = number of connected components)
  λ₁ (algebraic connectivity, Fiedler value)  — how well-connected the graph is
  ...
  λₙ₋₁                      (spectral radius)

The relationship:
  H⁰ ≅ ker(L)               — zero eigenspace
  rank(H¹) = nullity of the 1-form Laplacian L₁ = δ₁ᵀδ₁ + δ₀δ₀ᵀ
```

The spectral analysis already specified in the query engine gives us H⁰ for free (it's the
kernel of the Laplacian). To get H¹, we need the **1-form Laplacian** — a natural extension
of the existing spectral computation.

### 7.3 Relationship to CALM Compliance

CALM (INV-QUERY-001) classifies operations as monotonic or non-monotonic. The cohomological
perspective adds precision:

- **Monotonic operations** (Strata 0-1) can create new H¹ generators (adding datoms can
  create new disagreements) but cannot destroy existing ones (no deletion).
- **Non-monotonic operations** (Strata 2+, requiring barriers) can destroy H¹ generators
  (negation can remove the source of disagreement).

**Therefore**: H¹ is monotonically non-decreasing under monotonic operations. Resolving
cyclic incoherence REQUIRES non-monotonic operations (negation, aggregation). This gives
a formal justification for the sync barrier requirement: cyclic incoherence can only be
resolved through coordinated non-monotonic operations.

### 7.4 Relationship to the Reconciliation Taxonomy

The reconciliation taxonomy (CLAUDE.md §Reconciliation Framework) maps divergence types
to detection and resolution mechanisms. Cohomology refines this:

| Divergence Type | Φ Detects? | H¹ Detects? | Resolution Implication |
|-----------------|------------|-------------|----------------------|
| Epistemic (store vs. agent knowledge) | Yes | No (no cycle) | Pairwise harvest suffices |
| Structural (impl vs. spec) | Yes | **Yes** (ISP cycle) | May need coordinated fix |
| Consequential (current vs. future risk) | No | No | Guidance, not cohomology |
| Aleatory (agent vs. agent) | Yes | **Yes** (agent cycle) | Deliberation required if H¹ ≠ 0 |
| Logical (invariant vs. invariant) | No | **Yes** (spec cycle) | Contradiction = nontrivial H¹ on spec graph |
| Axiological (impl vs. goals) | Yes | **Yes** (ISP cycle) | Goal-drift is H¹ on the I-P boundary |

The key additions: **Logical divergence** (contradictions between invariants) is detectable
as H¹ on the spec dependency graph (INV-SCHEMA-009). If invariant A depends on B and B
depends on C and C contradicts A, that's a cycle with nontrivial H¹. The existing 5-tier
contradiction engine checks pairwise consistency but not cyclic consistency — H¹ fills this gap.

---

## 8. Implications for Guidance and Diagnostics

<a name="8-guidance"></a>

### 8.1 H¹-Aware Guidance Injection

The guidance system (GUIDANCE namespace, INV-GUIDANCE-001) currently injects methodology
pointers based on context and recent activity. With H¹ awareness:

```
If H¹ ≠ 0:
  guidance_priority = "STRUCTURAL INCOHERENCE DETECTED"
  guidance_detail = format_h1_generators(H¹)
  guidance_action = "Resolve cycle [generator list] before adding new content.
                     Pairwise fixes will not converge. Coordinate across:
                     {entities in cycle}"
```

This is a qualitative upgrade: instead of "your Φ is 47, consider adding links," the
system says "you have a cyclic contradiction between these specific entities, and fixing
them one at a time will create new problems."

### 8.2 Deliberation Triggers

The deliberation protocol (INV-DELIBERATION-001) currently triggers on commitment weight.
H¹ adds a second trigger:

```
deliberation_required(Decision) :-
    commitment_weight(Decision) > threshold,    % existing trigger
    OR
    affects_h1_generator(Decision, Generator),  % new trigger
    persistent(Generator, Age),
    Age > threshold_age.
```

If a decision affects an entity that participates in a long-lived H¹ generator, the
decision should go through deliberation regardless of its commitment weight — because
it might resolve (or worsen) a structural problem.

### 8.3 Merge Priority from Cohomological Analysis

When multiple merge channels exist, the topology framework (exploration/execution-topologies/)
must decide merge priority. H¹ adds a natural criterion:

```
merge_priority(α, β) =
    base_priority(α, β)                   % existing coupling-based priority
    + h1_boost * count_generators(α, β)   % boost if this merge could resolve H¹ generators

count_generators(α, β) = |{g ∈ generators(H¹) | edge (α,β) ∈ support(g)}|
```

Merges that could resolve cyclic incoherence get priority over merges that only reduce Φ.

---

## 9. Implementation Path

<a name="9-implementation"></a>

### 9.1 Stage 0: Foundation Data Structures

The cohomology computation requires no new store operations — it's a pure query over
existing data:

```rust
/// The coherence complex for a given store and agent set.
pub struct CoherenceComplex {
    /// Agents and their frontiers
    agents: Vec<(AgentId, Frontier)>,
    /// Communication graph (merge channels)
    edges: Vec<(AgentId, AgentId)>,
    /// Triangles (computed from edges)
    triangles: Vec<(AgentId, AgentId, AgentId)>,
}

/// Cohomology computation result
pub struct CohomologyResult {
    /// β₀: number of connected components
    betti_0: usize,
    /// β₁: number of independent incoherence cycles
    betti_1: usize,
    /// H¹ generators: each is a cycle of agent-attribute disagreements
    h1_generators: Vec<CycleGenerator>,
    /// Persistence data (if historical computation enabled)
    persistence: Option<PersistenceDiagram>,
}

/// A generator of H¹ — an incoherence cycle
pub struct CycleGenerator {
    /// The agents forming the cycle
    cycle: Vec<AgentId>,
    /// The attributes that disagree around the cycle
    disagreeing_attributes: Vec<Attribute>,
    /// The entities involved
    entities: Vec<EntityId>,
    /// Birth transaction (when this cycle appeared)
    birth_tx: TxId,
}
```

### 9.2 Stage 0: ISP Triangle Computation

The ISP triangle is the simplest and most valuable application. It requires only three
"agents" (Intent, Spec, Impl views) and their pairwise comparisons:

```rust
/// Compute H¹ for the ISP triangle.
/// Returns None if the coherence graph is acyclic (no I→P bypass links).
pub fn isp_cohomology(store: &Store) -> Option<Vec<BypassConflict>> {
    // 1. Find entities with both spec-mediated and direct I→P paths
    let dual_path_entities = query_dual_paths(store);

    // 2. For each, compare the spec-mediated interpretation with the direct interpretation
    let conflicts: Vec<BypassConflict> = dual_path_entities
        .iter()
        .filter(|e| spec_mediated_view(store, e) != direct_view(store, e))
        .collect();

    if conflicts.is_empty() { None } else { Some(conflicts) }
}
```

### 9.3 Stage 1: Full Agent Cohomology

When multi-agent coordination is added, extend to the full agent graph:

```rust
/// Compute full Čech cohomology over the agent communication graph.
pub fn agent_cohomology(store: &Store, agents: &[AgentId]) -> CohomologyResult {
    let complex = build_complex(store, agents);
    let delta_0 = build_coboundary_0(&complex);  // incidence matrix
    let delta_1 = build_coboundary_1(&complex);  // triangle boundary matrix

    let h0 = kernel_rank(&delta_0);
    let h1 = kernel_rank(&delta_1) - image_rank(&delta_0);

    let generators = if h1 > 0 {
        extract_generators(&delta_0, &delta_1, &complex)
    } else {
        vec![]
    };

    CohomologyResult { betti_0: h0, betti_1: h1, h1_generators: generators, persistence: None }
}
```

### 9.4 Stage 2: Persistent Cohomology

Add temporal tracking:

```rust
/// Track cohomology over transaction history.
pub fn persistent_cohomology(
    store: &Store,
    agents: &[AgentId],
    from_tx: TxId,
    to_tx: TxId,
) -> PersistenceDiagram {
    let mut diagram = PersistenceDiagram::new();
    let mut prev_generators: HashSet<GeneratorSignature> = HashSet::new();

    for tx in store.transactions_in_range(from_tx, to_tx) {
        let result = agent_cohomology(store.as_of(tx), agents);
        let curr_generators: HashSet<GeneratorSignature> = result.h1_generators
            .iter()
            .map(|g| g.signature())
            .collect();

        // Births: generators in curr but not in prev
        for g in curr_generators.difference(&prev_generators) {
            diagram.record_birth(g.clone(), tx);
        }

        // Deaths: generators in prev but not in curr
        for g in prev_generators.difference(&curr_generators) {
            diagram.record_death(g.clone(), tx);
        }

        prev_generators = curr_generators;
    }

    diagram
}
```

### 9.5 Dependency on Existing Components

| Component | Dependency | Status |
|-----------|-----------|--------|
| Datom store (P(D), ∪) | Provides the data | Stage 0 core |
| LIVE resolution (INV-STORE-012) | Computes resolved views | Stage 0 core |
| Frontier model (ADR-STORE-021) | Per-agent frontier tracking | Stage 0 (just added in topology foundations) |
| Agent entities (ADR-STORE-020) | Agent identification | Stage 0 (just added) |
| nalgebra (ADR-QUERY-012) | Linear algebra for rank computation | Stage 0 (spec'd) |
| Spec dependency graph (INV-SCHEMA-009) | ISP cycle detection | Stage 0 (just added) |
| Spectral computation (INV-QUERY-022) | Laplacian ↔ H⁰ connection | Stage 0 (just added) |

Everything needed is already in the Stage 0 spec. The cohomology computation is a pure
function over existing data structures — no new store operations, no new data model changes.

---

## 10. What Is Genuinely Novel Here

<a name="10-novelty"></a>

### 10.1 Prior Art

**Sheaf theory in distributed systems**: Goguen and Burstall (institutions, specifications)
used sheaves to model software specifications in the 1980s-90s. Their work is foundational
but abstract — no computational framework, no persistence, no connection to CRDTs.

**Persistent homology in data analysis**: TDA (topological data analysis) uses persistent
homology to study the "shape" of data clouds. The computational framework (persistence
diagrams, bottleneck distance) comes from this field. But TDA operates on point clouds
in metric spaces, not on knowledge graphs with semantic structure.

**Cohomological methods in database theory**: Abramsky, Barbosa et al. use sheaf-theoretic
semantics for databases (contextuality, non-locality). Their work connects quantum
foundations to database theory but doesn't address coherence verification or CRDTs.

**CRDT verification**: Formal verification of CRDTs (Shapiro, Preguiça et al.) proves
convergence properties but doesn't use cohomological tools — they use operational
semantics and simulation proofs.

### 10.2 What's New in This Proposal

1. **Sheaf cohomology as a coherence diagnostic for multi-agent AI systems**: No prior work
   applies Čech cohomology to the specific problem of measuring coherence between AI agents
   operating on a shared CRDT store. The connection between CRDT laws and sheaf conditions
   (§2.3) is straightforward but, to our knowledge, not previously formalized.

2. **The ISP triangle obstruction** (§4): Specification bypass as H¹ on the intent-spec-impl
   triangle is a new characterization. The key insight — that Φ = 0 can coexist with H¹ ≠ 0,
   meaning "all links exist but are inconsistent" — is a genuine blind spot in existing
   divergence metrics that cohomology uniquely addresses.

3. **Persistent cohomology as project health diagnostic** (§5): Tracking the birth-death
   of incoherence cycles over transaction history gives a "coherence EKG" with no prior
   analog. The distinction between short-lived (work in progress) and long-lived (structural
   problems) incoherence cycles is a novel diagnostic.

4. **CALM-cohomology connection** (§7.3): The observation that H¹ is monotonically
   non-decreasing under monotonic operations, and that resolving cyclic incoherence requires
   non-monotonic operations, gives a formal justification for sync barriers based on
   cohomological structure rather than just operational classification.

### 10.3 What This Is NOT

- It is NOT a replacement for Φ. Φ and H¹ are complementary diagnostics.
- It is NOT heavyweight mathematics applied for its own sake. The computation is O(n⁴)
  for n ≤ 10, which is microseconds. The conceptual overhead is higher than the
  computational overhead, but the conceptual insight (cyclic vs. acyclic incoherence)
  is genuinely useful.
- It is NOT required for Stage 0 to function. Stage 0 works fine with just Φ. Cohomology
  is a Stage 0 *extension* that becomes critical at Stage 2-3 (multi-agent, complex
  spec graphs).

---

## 11. Open Questions

<a name="11-open-questions"></a>

### OQ-1: Coefficient Choice for Non-Binary Disagreements

F₂ coefficients (agree/disagree) are the simplest and most natural for the initial
implementation. But some applications want richer coefficients:

- **Z coefficients**: Count the number of distinct resolved values in a cycle
- **R coefficients**: Weight disagreements by semantic distance (how different are the values?)
- **Module coefficients**: Use the resolution lattice itself as the coefficient module

**Recommendation**: Start with F₂. Generalize to Z or R only if F₂ proves too coarse
in practice.

### OQ-2: Simplicial vs. Cubical Complex

The Čech complex (simplicial) is the standard construction. An alternative is the
**cubical complex** — treating each attribute as an independent binary dimension
(agree/disagree on each attribute). The cubical complex may be more natural for the
DDIS setting where attributes are independent.

**Recommendation**: Defer. Simplicial is well-understood and sufficient for n ≤ 10.

### OQ-3: H² and Higher Cohomology

H² detects "voids" — obstructions to deforming one coherent state into another. For
DDIS, this would mean: even if H¹ = 0 (no cyclic incoherence), H² ≠ 0 means the
*path* from current state to fully coherent state has topological obstructions (you
can't continuously deform the current state into a coherent one without passing through
a worse intermediate state).

**Recommendation**: Explore in Stage 3. H⁰ and H¹ are the immediately useful invariants.

### OQ-4: Localization and Relative Cohomology

For large projects, computing global H¹ may be less useful than **relative cohomology**
H¹(G, G₀) where G₀ is the "stable" subgraph and G \ G₀ is the "active work" subgraph.
This would filter out long-resolved incoherence and focus on recent changes.

**Recommendation**: Natural extension. Implement when the persistence diagram shows that
most H¹ generators are long-dead and only a few are active.

### OQ-5: Integration with Formal Verification Tools

The cohomology computation could be expressed as a Kani harness (verify that H¹ = 0
after specific merge sequences) or as a stateright model (verify that all reachable
states have bounded H¹ rank). This would upgrade cohomological diagnostics from
"measured at runtime" to "verified by construction."

**Recommendation**: Stage 2. First validate that the cohomological diagnostics are
useful in practice (Stage 0-1), then invest in formal verification of the diagnostics
themselves.

---

*The fundamental claim of this exploration is simple: counting gaps (Φ) tells you how far
you are from coherence. Computing cohomology (H¹) tells you whether you can GET to
coherence by fixing things one at a time, or whether you need coordinated resolution.
The difference between these two questions is the difference between routine maintenance
and structural redesign. DDIS should answer both.*
