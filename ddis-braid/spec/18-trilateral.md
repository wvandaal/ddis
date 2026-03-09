> **Namespace**: TRILATERAL | **Wave**: 4 (Integration) | **Stage**: 0
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §18. TRILATERAL — Trilateral Coherence Model

### §18.0 Overview

The trilateral coherence model extends the bilateral loop (BILATERAL namespace)
to cover the full Intent ↔ Specification ↔ Implementation triangle. Where the
bilateral loop formalizes the Specification ↔ Implementation boundary as an
adjunction, the trilateral model adds the Intent ↔ Specification and Intent ↔
Implementation boundaries, unifying all three under a single datom store with
three LIVE materialized views.

**Key insight**: Intent, Specification, and Implementation are not separate
document stores requiring periodic synchronization — they are three LIVE
materialized views over the single datom store (extending INV-STORE-012's
LIVE index concept to all three coherence boundaries).

**Traces to**: SEED.md §1 (coherence verification), §2 (divergence problem),
§3 (specification formalism), §5 (harvest/seed lifecycle), §6 (reconciliation
mechanisms)
**Source**: `exploration/coherence-convergence/TRILATERAL_COHERENCE_MODEL.md` §19
**ADRS.md sources**: CO-008, CO-010, IB-010

**Forward reference — Quadrilateral extension (Stage 3):** The trilateral
model (Intent ↔ Spec ↔ Impl) extends to a quadrilateral (Intent ↔ Spec ↔
Impl ↔ Topology) when the topology coordination layer is implemented at
Stage 3. The fourth vertex adds TOPO_ATTRS and two new boundaries (T↔I,
T↔P) to the divergence metric Φ. The generalization is parameterized over
N vertices (ADR-TRILATERAL-004), so the trilateral case is a specialization
of the N-lateral model with |boundaries| = 2.

---

### §18.1 Level 0: Algebraic Specification

#### Three LIVE Projections

```
Given:
  Store S ∈ P(D)                    — the single datom store (STORE namespace)
  INTENT_ATTRS ⊂ Keyword            — attribute namespace for intent facts
  SPEC_ATTRS ⊂ Keyword              — attribute namespace for spec facts
  IMPL_ATTRS ⊂ Keyword              — attribute namespace for impl facts

  INTENT_ATTRS ∩ SPEC_ATTRS = ∅
  SPEC_ATTRS ∩ IMPL_ATTRS = ∅
  INTENT_ATTRS ∩ IMPL_ATTRS = ∅

Three LIVE projections:
  LIVE_I(S) = project(S, {d ∈ S | d.a ∈ INTENT_ATTRS})
  LIVE_S(S) = project(S, {d ∈ S | d.a ∈ SPEC_ATTRS})
  LIVE_P(S) = project(S, {d ∈ S | d.a ∈ IMPL_ATTRS})

  where project applies resolution per INV-STORE-012 (LIVE index).
```

Each projection is a monotone function from the store semilattice:
```
S₁ ⊆ S₂ ⟹ LIVE_X(S₁) ⊆ LIVE_X(S₂)   for X ∈ {I, S, P}
```

#### Divergence as Live Metric

```
Φ(S) = w₁ × D_IS(S) + w₂ × D_SP(S)

where:
  D_IS(S) = |{e ∈ LIVE_I(S) | ¬∃ link: link.a = :spec/traces-to ∧ link.v = e}|
           + |{e ∈ LIVE_S(S) | ¬∃ link: link.a = :spec/traces-to ∧ link.e ∈ LIVE_I(S) ∧ link.v = e}|

  D_SP(S) = |{e ∈ LIVE_S(S) | ¬∃ link: link.a = :spec/implements ∧ link.v = e}|
           + |{e ∈ LIVE_P(S) | ¬∃ link: link.a = :spec/implements ∧ link.e = e}|

  Type semantics for cross-boundary links:
  - D_IS uses String-valued :spec/traces-to links (spec/02-schema.md line 143):
    an intent entity is "linked" if ANY spec entity has a :spec/traces-to String
    value referencing it (e.g., "SEED §4 Axiom 2"). String presence, not Ref
    entity resolution — traceability to SEED.md sections is inherently textual.
  - D_SP uses Ref-valued :spec/implements links (spec/02-schema.md line 191):
    a spec entity is "implemented" if ANY impl entity has a :spec/implements Ref
    pointing to its entity ID. Ref resolution, not String matching.

  w₁, w₂ = boundary weights (configurable as datoms, default: w₁ = w₂ = 0.5)
```

Φ is a live counter computed from the store at any instant. Every transaction
that adds a `:spec/traces-to` or `:spec/implements` link decreases Φ. Every transaction
that adds an unlinked intent decision or unlinked code function increases Φ.

Generalized form (ADR-TRILATERAL-004):
```
  Φ(S) = Σᵢ wᵢ × Dᵢ(S)  where i ∈ boundaries(S)
  boundaries(S₀) = {IS, SP}  (trilateral initial state)
```
The existing two-term formula is a specialization with |boundaries| = 2.
Adding a vertex (e.g., Topology at Stage 3) registers new boundaries and
their divergence functions; the formula structure is unchanged.

#### Formality Gradient

```
formality_level(e, S) =
  0  if  e has no outgoing cross-boundary links
  1  if  e has :intent/noted (acknowledged but not formalized)
  2  if  e has :spec/id ∧ :spec/type ∧ :spec/statement
  3  if  e has L2 + :spec/falsification ∧ :spec/traces-to
  4  if  e has L3 + :spec/witnessed ∧ :spec/challenged

∀ store growth S → S' (S ⊂ S'):
  formality_level(e, S') ≥ formality_level(e, S)
  (adding links can only increase formality, never decrease it)
```

**Laws**:
- **L1 (Projection monotonicity)**: `S₁ ⊆ S₂ ⟹ LIVE_X(S₁) ⊆ LIVE_X(S₂)` for X ∈ {I, S, P}
- **L2 (Convergence monotonicity)**: Adding `:spec/traces-to` or `:spec/implements` links never increases Φ
- **L3 (Attribute partition)**: `INTENT_ATTRS ∩ SPEC_ATTRS = SPEC_ATTRS ∩ IMPL_ATTRS = INTENT_ATTRS ∩ IMPL_ATTRS = ∅`
- **L4 (Formality monotonicity)**: Formality level is monotonically non-decreasing under store growth

Forward reference — LIVE_T Projection (Stage 3):
```
  LIVE_T: The topology projection, computed over TOPO_ATTRS.
  Defined when the topology coordination layer (exploration docs 00-11) is implemented.
  LIVE_T is structurally identical to LIVE_I, LIVE_S, LIVE_P — a partition of the
  attribute namespace with a divergence function measuring gap across its boundaries.
```
When LIVE_T is added, L1 extends to X ∈ {I, S, P, T} and L3 extends to
include TOPO_ATTRS in the pairwise disjointness constraint. See
INV-TRILATERAL-005 for the reserved TOPO_ATTRS namespace and
ADR-TRILATERAL-004 for the N-lateral generalization.

---

### §18.2 Level 1: State Machine Specification

**State**: `Σ_trilateral = (live_i: LiveView, live_s: LiveView, live_p: LiveView, phi: f64, formality_map: Map<EntityId, u8>)`

**Transitions**:

```
TRANSACT_INTENT(Σ, datoms) → Σ' where:
  PRE:  ∀ d ∈ datoms: d.a ∈ INTENT_ATTRS
  POST: Σ'.live_i updated incrementally
  POST: Σ'.phi recomputed (may increase — unlinked intent)

TRANSACT_SPEC(Σ, datoms) → Σ' where:
  PRE:  ∀ d ∈ datoms: d.a ∈ SPEC_ATTRS
  POST: Σ'.live_s updated incrementally
  POST: Σ'.phi recomputed (may increase — unlinked spec)

TRANSACT_IMPL(Σ, datoms) → Σ' where:
  PRE:  ∀ d ∈ datoms: d.a ∈ IMPL_ATTRS
  POST: Σ'.live_p updated incrementally
  POST: Σ'.phi recomputed (may increase — unlinked impl)

LINK(Σ, source, target, link_type) → Σ' where:
  PRE:  link_type ∈ {:spec/traces-to, :spec/implements}
  POST: Σ'.phi ≤ Σ.phi (link addition never increases Φ)
  POST: formality_level(source, S') ≥ formality_level(source, S)

COMPUTE_PHI(Σ) → Σ' where:
  POST: Σ'.phi = w₁ × D_IS(S) + w₂ × D_SP(S)
  POST: phi computable from store alone (no external state)
```

---

### §18.3 Level 2: Implementation Contract

```rust
/// Attribute namespace partitions — pairwise disjoint.
pub const INTENT_ATTRS: &[&str] = &[
    "intent/decision", "intent/rationale", "intent/source",
    "intent/goal", "intent/constraint", "intent/preference",
    "intent/noted",
];

pub const SPEC_ATTRS: &[&str] = &[
    "spec/id", "spec/type", "spec/statement", "spec/falsification",
    "spec/traces-to", "spec/stage", "spec/verification",
    "spec/witnessed", "spec/challenged",
];

pub const IMPL_ATTRS: &[&str] = &[
    "impl/signature", "impl/implements", "impl/file",
    "impl/module", "impl/test-result", "impl/coverage",
];

/// Compute total divergence Φ from the store.
/// Pure function — no external state required (INV-TRILATERAL-002).
pub fn compute_phi(store: &Store, w_is: f64, w_sp: f64) -> f64 {
    let d_is = count_unlinked_intent(store) + count_untraced_spec(store);
    let d_sp = count_unimplemented_spec(store) + count_unlinked_impl(store);
    w_is * d_is as f64 + w_sp * d_sp as f64
}

/// Compute formality level for an entity.
pub fn formality_level(store: &Store, entity: EntityId) -> u8 {
    // Level 0–4 based on link structure (§18.1)
    ...
}
```

---

### §18.4 Invariants

### INV-TRILATERAL-001: Three LIVE Projections

**Traces to**: SEED §1, §4, INV-STORE-012
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
LIVE_I, LIVE_S, LIVE_P are monotone functions of the store semilattice (P(D), ∪):
  S₁ ⊆ S₂ ⟹ LIVE_X(S₁) ⊆ LIVE_X(S₂)   for X ∈ {I, S, P}

Each is a projection over a disjoint attribute namespace:
  LIVE_X(S) = LIVE(project(S, X_ATTRS))
  where LIVE is the INV-STORE-012 materialized view operation.
```

#### Level 1 (State Invariant)
The three LIVE views update incrementally with every transaction. Each view
contains exactly those datoms whose attribute falls within its namespace
partition, with resolution applied per INV-STORE-012.

**Falsification**: A datom with attribute in INTENT_ATTRS appears in LIVE_S
or LIVE_P, or a transaction adding datoms to the store does not update the
relevant LIVE view.

---

### INV-TRILATERAL-002: Divergence as Live Metric

**Traces to**: SEED §2, §6, ADRS CO-010
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Φ(S) = w₁ × D_IS(S) + w₂ × D_SP(S)

Φ is computable from the store alone — no external state required.
Φ ≥ 0 (by construction: divergence counts are non-negative).
Φ = 0 iff all intent datoms have :spec/traces-to links to spec datoms AND
         all spec datoms have :spec/implements links to impl datoms (and vice versa).
```

#### Level 1 (State Invariant)
Φ is recomputed after every transaction. The COMPUTE_PHI transition reads
only from the store — no session state, no external files, no configuration
beyond the boundary weights (which are themselves datoms in the store).

**Falsification**: Φ computation requires state not in the store, or Φ is
negative, or Φ = 0 while unlinked datoms exist across boundaries.

---

### INV-TRILATERAL-003: Formality Gradient

**Traces to**: SEED §3 (specification formalism)
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
formality_level : EntityId × Store → {0, 1, 2, 3, 4}

∀ store growth S ⊂ S':
  formality_level(e, S') ≥ formality_level(e, S)
  (monotonically non-decreasing under store growth)
```

#### Level 1 (State Invariant)
Formality level is computed from link structure, not from metadata or labels.
A datom starts at Level 0 (no links) and progresses through Levels 1–4 as
cross-boundary links, falsification conditions, and verification evidence
are added. Because the store is append-only (C1), removing links is impossible
without retraction, and retraction of a link would create a new datom rather
than deleting the old one — the formality level as measured from the full
history never decreases.

**Falsification**: An entity's formality level decreases between two store
states where S ⊂ S' (the second state is a superset of the first).

---

### INV-TRILATERAL-004: Convergence Monotonicity

**Traces to**: SEED §6, INV-BILATERAL-001
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
∀ LINK operations adding :spec/traces-to or :spec/implements links:
  Φ(S') ≤ Φ(S)
  (adding convergence links never increases total divergence)
```

#### Level 1 (State Invariant)
The LINK transition can only decrease Φ or leave it unchanged. Adding a
`:spec/traces-to` link connects an intent datom to a spec datom, reducing D_IS.
Adding an `:spec/implements` link connects an impl datom to a spec datom, reducing
D_SP. Neither operation can increase divergence at any boundary.

This extends INV-BILATERAL-001 (monotonic convergence of the bilateral fitness
function) to the full trilateral model.

**Falsification**: A LINK operation that increases Φ — i.e., adding a
`:spec/traces-to` or `:spec/implements` link results in higher total divergence.

---

### INV-TRILATERAL-005: Attribute Namespace Partitioning

**Traces to**: INV-SCHEMA-001, C3 (schema-as-data)
**Verification**: `V:TYPE`, `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
INTENT_ATTRS ∩ SPEC_ATTRS = ∅
SPEC_ATTRS ∩ IMPL_ATTRS = ∅
INTENT_ATTRS ∩ IMPL_ATTRS = ∅

∀ datoms d ∈ S: d.a ∈ INTENT_ATTRS ⊔ SPEC_ATTRS ⊔ IMPL_ATTRS ⊔ META_ATTRS
  (every attribute belongs to exactly one namespace;
   META_ATTRS covers cross-cutting attributes like :db/ident, :tx/provenance)
```

#### Level 1 (State Invariant)
The attribute namespace partition is enforced at schema level (INV-SCHEMA-001).
Each attribute is declared with a namespace prefix (`:intent/`, `:spec/`,
`:impl/`) that determines which LIVE view contains its datoms. The partition
is defined as schema datoms in the store (C3: schema-as-data), making it
queryable and evolvable via transactions.

#### Level 2 (Implementation Contract)
```rust
/// Compile-time verification via type system.
pub enum AttrNamespace {
    Intent,
    Spec,
    Impl,
    Meta,  // cross-cutting: :db/*, :tx/*
}

pub fn classify_attribute(attr: &Attribute) -> AttrNamespace {
    match attr.namespace() {
        "intent" => AttrNamespace::Intent,
        "spec"   => AttrNamespace::Spec,
        "impl"   => AttrNamespace::Impl,
        _        => AttrNamespace::Meta,
    }
}
```

Forward reference — Topology namespace (Stage 3):
```
  TOPO_ATTRS ∩ INTENT_ATTRS = ∅
  TOPO_ATTRS ∩ SPEC_ATTRS   = ∅
  TOPO_ATTRS ∩ IMPL_ATTRS   = ∅

  TOPO_ATTRS will include: :topo/type, :topo/agents, :topo/channels,
    :topo/fitness, :topo/coupling, :topo/compiled-from, etc.
```
See ADR-TRILATERAL-004 (N-Lateral Extensibility) for how the attribute
namespace partition generalizes to N vertices.

**Falsification**: Two attributes from different namespace partitions share
the same keyword, or a datom appears in more than one LIVE view.

---

### INV-TRILATERAL-006: Divergence as Datalog Program

**Traces to**: INV-QUERY-001 (CALM-compliant monotonic reads)
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Φ is expressible as a Stratum 5 Datalog query (INV-QUERY-005):

  -- Unlinked intent (no :spec/traces-to from spec)
  unlinked_intent(E) :- intent_entity(E), not traced_from_spec(E).
  traced_from_spec(E) :- [_, :spec/traces-to, E, _, :assert].

  -- Unlinked spec (no :spec/implements from impl)
  unlinked_spec(E) :- spec_entity(E), not implemented_by(E).
  implemented_by(E) :- [_, :spec/implements, E, _, :assert].

  -- Divergence counts
  d_is = count(unlinked_intent) + count(untraced_spec).
  d_sp = count(unlinked_spec) + count(unlinked_impl).

  phi = w1 * d_is + w2 * d_sp.
```

#### Level 1 (State Invariant)
Because Φ is a Datalog program over the store, it inherits the CALM
compliance guarantees of the query engine (INV-QUERY-001): the computation
is monotone (adding datoms can only change counts upward), deterministic
(same store produces same Φ), and incrementalizable (delta-maintenance via
semi-naive evaluation).

**Falsification**: The Φ computation cannot be expressed as a Datalog program,
or the computation produces different results when evaluated incrementally
vs. from scratch.

---

### INV-TRILATERAL-007: Unified Store Self-Bootstrap

**Traces to**: C7 (self-bootstrap), SEED §1
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
The TRILATERAL spec elements (INV-TRILATERAL-001..010, ADR-TRILATERAL-001..006,
NEG-TRILATERAL-001..004) are datoms in the store.

∀ trilateral elements T: T ∈ LIVE_S(S)
  (all trilateral spec elements appear in the spec LIVE view)
```

#### Level 1 (State Invariant)
The trilateral coherence model specifies itself. Its invariants, ADRs, and
negative cases are transacted into the store as spec-namespace datoms during
the self-bootstrap process (spec/01-store.md, C7). This enables the system
to verify its own trilateral coherence — Φ includes the trilateral spec
elements themselves as entities requiring `:spec/implements` links.

**Falsification**: Any trilateral spec element that is not present as a datom
in the store after self-bootstrap, or that appears in LIVE_I or LIVE_P
instead of LIVE_S.

---

### INV-TRILATERAL-008: ISP Specification Bypass Detection

**Traces to**: SEED §2 (divergence problem), SEED §6 (reconciliation),
  INV-QUERY-023 (edge Laplacian), exploration/sheaf-coherence/00-sheaf-cohomology.md §5
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
The ISP triangle is the smallest non-trivial coherence cycle:

  I ——(:spec/traces-to)—— S ——(:spec/implements)—— P
   \                                                /
    \————————(:impl/source-intent)————————————————/

A specification bypass occurs when an implementation P derives directly from
intent I without following the specification S's interpretation.

Define the ISP agreement predicate for entity e:
  agree_IS(e) ≡ (interpretation of e in LIVE_I matches LIVE_S)
  agree_SP(e) ≡ (interpretation of e in LIVE_S matches LIVE_P)
  agree_IP(e) ≡ (interpretation of e in LIVE_I matches LIVE_P)

ISP coherence for entity e:
  ISP_coherent(e) ≡ agree_IS(e) ∧ agree_SP(e) → agree_IP(e)

Specification bypass:
  bypass(e) ≡ agree_IP(e) ∧ ¬agree_SP(e)
  (impl matches intent but contradicts spec — spec was circumvented)
```

#### Level 1 (State Invariant)
The coherence checker examines every entity e that has links in all three
boundaries (I↔S, S↔P, I↔P). For each such entity, it computes the three
agreement predicates by comparing resolved attribute values across LIVE views.

When `bypass(e)` holds, the checker emits a diagnostic identifying:
- The entity e
- The attribute(s) where S and P disagree
- The I→P link that created the bypass path

In Stage 0 (single agent), this reduces to checking whether the implementing
agent's code follows the spec or directly interprets the seed. The ISP check
is O(|entities|) — a linear scan over trilateral-linked entities.

#### Level 2 (Implementation Contract)
```rust
/// Check ISP coherence for a single entity across three LIVE views.
pub fn isp_check(
    entity: EntityId,
    live_i: &LiveView,
    live_s: &LiveView,
    live_p: &LiveView,
    attrs: &[Attribute],
) -> IspResult {
    let mut bypasses = Vec::new();
    for attr in attrs {
        let v_i = live_i.resolve(entity, attr);
        let v_s = live_s.resolve(entity, attr);
        let v_p = live_p.resolve(entity, attr);
        let agree_is = v_i == v_s;
        let agree_sp = v_s == v_p;
        let agree_ip = v_i == v_p;
        if agree_ip && !agree_sp {
            bypasses.push(Bypass { entity, attr: attr.clone(), v_s, v_p });
        }
    }
    IspResult { entity, bypasses }
}
```

**Falsification**: An entity e exists where `bypass(e)` holds (impl follows intent,
contradicts spec) but the checker reports no bypass. Equivalently: any false negative
on the ISP agreement check.

**proptest strategy**: For randomly generated ISP triples (v_i, v_s, v_p) over a
small value domain (|V| ≤ 4), verify that `bypass` is detected iff
`v_i == v_p && v_s != v_p`.

---

### INV-TRILATERAL-009: Coherence Completeness — (Φ, β₁) Duality

**Traces to**: INV-TRILATERAL-001 (Φ definition), INV-QUERY-024 (β₁ computation),
  SEED §2 (divergence problem), exploration/sheaf-coherence/00-sheaf-cohomology.md §6
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
The coherence state of a store S is the pair (Φ(S), β₁(S)) where:

  Φ(S) = |D_IS| + |D_SP| + |D_IP|    (INV-TRILATERAL-001, gap count)
  β₁(S) = dim(ker(L₁(G_S)))           (INV-QUERY-024, cycle count)

The four coherence quadrants:

  Φ > 0, β₁ = 0: Gaps exist but are independent.
                  Resolution: fix any gap in any order (monotonic).
  Φ = 0, β₁ > 0: All links exist but form contradictory cycles.
                  Resolution: coordinated multi-boundary fix required.
  Φ > 0, β₁ > 0: Gaps AND cycles. Fix cycles first (they constrain gap resolution order).
  Φ = 0, β₁ = 0: Fully coherent. Target state.

Completeness theorem:
  Φ(S) = 0 ∧ β₁(S) = 0 ↔ S is coherent
  (no gaps and no cyclic contradictions ↔ full coherence)

Monotonicity (gap resolution):
  Adding a `:spec/traces-to` or `:spec/implements` link ℓ:
    Φ(S ∪ {ℓ}) ≤ Φ(S)         (Φ is monotonically non-increasing — INV-TRILATERAL-004)
    β₁(S ∪ {ℓ}) ≶ β₁(S)       (β₁ can increase or decrease — CAUTION)

  Adding ℓ may complete an open path into a closed cycle with disagreements,
  increasing β₁. The guidance system MUST compute β₁ before and after proposed
  link additions (INV-TRILATERAL-008 provides the diagnostic).
```

#### Level 1 (State Invariant)
The `braid coherence` command reports both Φ and β₁. Neither metric alone is
sufficient: Φ misses the case where all links exist but are contradictory
(Φ = 0, β₁ > 0), and β₁ misses isolated gaps that don't form cycles (Φ > 0,
β₁ = 0). The pair (Φ, β₁) is the minimal complete characterization of
store coherence with respect to the trilateral model.

The coherence check returns `COHERENT` only when both Φ = 0 AND β₁ = 0.
Any other state includes a diagnostic explaining which quadrant the store
occupies and what resolution strategy applies.

#### Level 2 (Implementation Contract)
```rust
#[derive(Debug, PartialEq, Eq)]
pub enum CoherenceQuadrant {
    Coherent,              // Φ = 0, β₁ = 0
    GapsOnly,              // Φ > 0, β₁ = 0
    CyclesOnly,            // Φ = 0, β₁ > 0
    GapsAndCycles,         // Φ > 0, β₁ > 0
}

pub fn coherence_state(phi: usize, beta_1: usize) -> CoherenceQuadrant {
    match (phi > 0, beta_1 > 0) {
        (false, false) => CoherenceQuadrant::Coherent,
        (true,  false) => CoherenceQuadrant::GapsOnly,
        (false, true)  => CoherenceQuadrant::CyclesOnly,
        (true,  true)  => CoherenceQuadrant::GapsAndCycles,
    }
}

/// Full coherence check returning (Φ, β₁, quadrant, diagnostics).
pub fn check_coherence(store: &Store) -> CoherenceReport {
    let phi = compute_phi(store);        // INV-TRILATERAL-001
    let beta_1 = compute_beta_1(store);  // INV-QUERY-024
    let quadrant = coherence_state(phi, beta_1);
    CoherenceReport { phi, beta_1, quadrant, /* diagnostics */ }
}
```

**Falsification**: The system reports `CoherenceQuadrant::Coherent` when Φ > 0 OR
β₁ > 0. Equivalently: a store with an unresolved gap or an unresolved cyclic
contradiction is classified as coherent.

**proptest strategy**: For random store states with known Φ and β₁ values,
verify that the quadrant classification matches the (Φ > 0, β₁ > 0) truth table
exhaustively over all four combinations.

---

### INV-TRILATERAL-010: Persistent Cohomology over Transaction Filtration

**Traces to**: C1 (append-only), INV-QUERY-024 (β₁),
  SEED §7 (self-improvement loop), exploration/sheaf-coherence/02-persistent-diagnostics.md §1
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
The append-only store (C1) induces a monotone filtration:

  ∅ = S₀ ⊂ S₁ ⊂ S₂ ⊂ ... ⊂ Sₜ

where Sᵢ = store after transaction i, and Sᵢ ⊂ Sᵢ₊₁ (inclusion by set growth).

The first cohomology at each step gives a persistence module:

  M = {H¹(Sᵢ), φᵢ : H¹(Sᵢ) → H¹(Sᵢ₊₁)}

where φᵢ is induced by the inclusion Sᵢ ↪ Sᵢ₊₁.

By the structure theorem for persistence modules (over a PID):

  M ≅ ⊕ⱼ k[bⱼ, dⱼ)

Each interval [bⱼ, dⱼ) represents an incoherence cycle born at transaction bⱼ
and resolved at transaction dⱼ. The multiset {(bⱼ, dⱼ)} is the persistence diagram.

Stability (Bottleneck Distance Theorem):
  d_B(PD(Sᵢ), PD(Sᵢ₊₁)) ≤ 1
  (a single transaction changes the persistence diagram by at most one birth or death)

Derived metrics:
  P_max(t)    = max{dⱼ - bⱼ : bⱼ ≤ t < dⱼ}   — age of longest-lived cycle
  N_active(t) = |{j : bⱼ ≤ t < dⱼ}|            — count of unresolved cycles
  R_net(t)    = R_birth(t) - R_death(t)          — accumulation rate
```

#### Level 1 (State Invariant)
The persistence tracker maintains a running record of H¹ generator births and
deaths across transaction history. On each transaction, it incrementally updates
the edge Laplacian (only edges affected by the new datoms change), recomputes
β₁, and detects births (new kernel vectors) and deaths (kernel vectors that
left the kernel).

A generator that persists beyond the **chronic threshold** (configurable,
default: 3× median resolved persistence) triggers a `SIGNAL_H1_CHRONIC`
signal, elevating the incoherence from routine work-in-progress to a
structural problem requiring deliberation.

The chronic threshold is stored as a datom (schema-as-data, C3):
```
(config:cohomology, :config/chronic-threshold-multiplier, 3.0, tx:config, assert)
```

#### Level 2 (Implementation Contract)
```rust
/// A single persistence interval: an incoherence cycle's lifetime.
#[derive(Debug, Clone)]
pub struct PersistenceInterval {
    pub birth_tx: TxId,
    pub death_tx: Option<TxId>,  // None = still alive
    pub generator: DVector<f64>, // harmonic representative at birth
}

/// Incremental persistence tracker across transaction history.
pub struct PersistenceTracker {
    intervals: Vec<PersistenceInterval>,
    prev_kernel: Vec<DVector<f64>>,
    chronic_multiplier: f64,  // default 3.0
}

impl PersistenceTracker {
    /// Update after a new transaction. Returns births and deaths.
    pub fn update(
        &mut self,
        tx: TxId,
        new_kernel: Vec<DVector<f64>>,
    ) -> PersistenceUpdate {
        let births = self.detect_births(&new_kernel);
        let deaths = self.detect_deaths(&new_kernel, tx);
        self.prev_kernel = new_kernel;
        PersistenceUpdate { births, deaths }
    }

    /// Current persistence diagram.
    pub fn diagram(&self) -> Vec<(TxId, Option<TxId>)> {
        self.intervals.iter()
            .map(|iv| (iv.birth_tx, iv.death_tx))
            .collect()
    }
}
```

**Falsification**: A generator that is provably resolved (the corresponding
eigenvector leaves ker(L₁)) is not recorded as dead, OR a generator that
persists beyond the chronic threshold does not trigger the chronic signal.

**proptest strategy**: For a sequence of random store mutations (n ≤ 20
transactions), verify that |births| - |deaths| = β₁(Sₜ) - β₁(S₀) at every
step (the persistence accounting identity).

---

### §18.5 ADRs

### ADR-TRILATERAL-001: Unified Store with Three LIVE Views

**Traces to**: SEED §4, INV-STORE-012, TRILATERAL_COHERENCE_MODEL.md §19, ADRS IB-010, ADRS CO-008, ADRS CO-010
**Stage**: 0

#### Problem
How should the three coherence states (Intent, Specification, Implementation)
be represented and synchronized?

#### Options
A) **Three separate stores with periodic sync** — each state has its own ground
   truth (JSONL logs, spec files, code files) connected by six functors and
   synchronized through periodic 5-step convergence cycles. Divergence
   accumulates unobserved between cycles.
B) **Unified datom store with three LIVE views** — one datom store, three
   monotone projection functions defined by attribute namespace partition.
   Divergence is a live metric, convergence is a property, not an action.

#### Decision
**Option B.** I, S, P are three LIVE materialized views over the single datom
store. Every input — conversation message, spec element, code assertion — is
a datom transacted into the same store. The views update incrementally,
automatically, continuously.

The LIVE index from INV-STORE-012 already solves the store/query boundary with
`LIVE(S) = fold(causal_sort(S), apply_resolution)`. This proposal extends the
same concept to all three coherence boundaries.

#### Formal Justification
Option A (three stores, periodic sync) has the same architectural weakness as
pre-CI integration: batch synchronization where divergence accumulates
unobserved between sync points. Option B eliminates the sync problem by
construction — there is nothing to synchronize because there is only one store.

The six functors from the TRILATERAL_COHERENCE_MODEL become projections from a
single source. The adjunctions become an implementation detail of how the
projection functions compose, not a user-facing concept.

#### Consequences
- Divergence (Φ) is a live counter, not a periodic measurement
- No "sync" operation needed — coherence is a continuous property
- Every input channel (conversation, spec edit, code commit) feeds the same store
- Cross-boundary queries are native Datalog programs (not cross-system joins)
- The convergence cycle (BILATERAL §10) becomes the strategy for reducing Φ
  when the live metric indicates drift, not the only mechanism for detecting drift

#### Falsification
The unified-store approach is wrong if: (1) the single store becomes a
performance bottleneck compared to three specialized stores, (2) attribute
namespace partitioning is insufficient to separate the three views cleanly
(cross-cutting concerns that belong to no single namespace), or (3) the
overhead of datomizing every input exceeds the value of continuous Φ
measurement.

---

### ADR-TRILATERAL-002: EDNL as Interchange Format

**Traces to**: ADRS FD-001, ADR-LAYOUT-001 (LAYOUT namespace), SEED §4
**Stage**: 0

#### Problem
What interchange format should `braid transact --file` accept for bulk datom
ingestion, including self-bootstrap of spec elements?

#### Options
A) **JSONL** (JSON Lines) — one JSON object per line. Universal tooling support,
   but semantically poor for Clojure-heritage datoms (no keywords, no tagged
   literals, no sets).
B) **EDNL** (EDN per line) — one EDN value per line. Native representation for
   datoms (keywords, tagged literals, maps). Aligns with the datom's Datomic
   heritage. Requires an EDN parser (UNC-LAYOUT-002).
C) **Custom binary format** — compact but opaque. Poor debuggability.

#### Decision
**Option B.** EDNL for interchange (`braid transact --file`). Per-transaction
`.edn` files on disk (per LAYOUT namespace, ADR-LAYOUT-001). The EDN format
is the natural representation for datoms: keywords for attributes (`:spec/type`),
tagged literals for content-addressed IDs (`#blake3 "..."`), maps for datom
tuples.

On-disk storage uses per-transaction content-addressed `.edn` files in
`.braid/txns/{hash[0..2]}/{full_blake3_hex}.edn` (per LAYOUT namespace).
The EDNL format is used for streaming interchange (pipe-friendly, one datom
per line).

#### Formal Justification
JSONL (Option A) requires encoding keywords as strings (`"spec/type"` instead
of `:spec/type`), losing the semantic distinction between keywords and strings
that the datom model relies on. Tagged literals (`#blake3`, `#hlc`) have no
JSON equivalent — they would need ad-hoc encoding conventions. EDNL preserves
the full semantic structure of datoms natively.

#### Consequences
- Stage 0 requires an EDN parser as the first deliverable (UNC-LAYOUT-002)
- Custom parser (~500 lines) aligns with cleanroom philosophy
- INV-LAYOUT-011 (canonical serialization) provides the formal parser contract
- `serde_json` is still needed for CLI JSON output mode and MCP JSON-RPC — this
  ADR covers store interchange, not all serialization
- Self-bootstrap files: `spec-schema.ednl`, `spec-bootstrap.ednl`

#### Falsification
EDNL is wrong if: (1) the EDN parser introduces more bugs than JSONL's
well-tested ecosystem would have, (2) tooling friction (no EDN support in
standard Unix tools) outweighs semantic benefits, or (3) the custom parser
exceeds 1000 lines (indicating EDN is more complex than assumed).

---

### ADR-TRILATERAL-003: Hooks for Invisible Convergence

**Traces to**: SEED §5 (harvest/seed lifecycle), TRILATERAL_COHERENCE_MODEL.md §19, ADRS IB-010
**Stage**: 1

#### Problem
How should inputs from conversation, spec edits, and code changes be
automatically datomized into the store without manual annotation?

#### Options
A) **Manual annotation** — developers explicitly tag every function, conversation
   decision, and spec element with cross-boundary links.
B) **Batch processing** — periodic scripts scan conversation logs, spec files,
   and code for new content and datomize it.
C) **Event-driven hooks** — file watchers, Claude Code hooks, and git hooks
   automatically datomize inputs as they occur. Invisible to the developer.

#### Decision
**Option C, deferred to Stage 1.** Hooks provide the "invisible convergence"
property: the developer sees nothing; the LIVE views simply update. At Stage 0,
datomization is explicit (`braid transact --file`). At Stage 1, hooks automate:

- **Conversation → datoms**: Claude Code statusline hook extracts decisions
  from conversation context, transacts as intent datoms
- **Spec → datoms**: File watcher on `spec/` parses structured elements,
  transacts as spec datoms (extending existing `ddis parse` concept)
- **Code → datoms**: File watcher on `src/` extracts module/function structure,
  transacts as impl datoms. `// braid:spec/implements INV-X-NNN` annotations provide
  explicit links; LLM-assisted inference fills gaps

Stage 0 requires only explicit datomization — hooks are not a Stage 0
deliverable.

#### Formal Justification
Manual annotation (Option A) imposes overhead proportional to output — every
line of code requires a tag. This is the "process obligation" that SEED.md §2
identifies as the root cause of methodology decay. Batch processing (Option B)
reintroduces the periodic-sync problem that the unified store eliminates.
Hooks (Option C) make convergence a side effect of doing work — the developer
writes code, and the system datomizes it automatically.

Deferring hooks to Stage 1 respects the staged roadmap: Stage 0 validates the
core store/query/harvest/seed cycle; Stage 1 adds the automation that makes
the cycle invisible.

#### Consequences
- Stage 0 is manual: `braid transact --file spec-bootstrap.ednl`
- Stage 1 adds hooks: convergence becomes invisible
- Hook implementation depends on Claude Code's statusline/hook system
- The formality gradient (INV-TRILATERAL-003) handles partial datomization
  gracefully — not everything needs full links from day one

#### Falsification
Hooks are wrong if: (1) the overhead of continuous datomization degrades
interactive performance perceptibly (>100ms latency added), (2) automated
extraction (especially LLM-assisted) produces more noise than signal
(false links that increase rather than decrease confusion), or (3) developers
bypass the hook system because manual annotation is more reliable.

---

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

```
  Φ(S) = Σᵢ wᵢ × Dᵢ(S)
```

where i ranges over all adjacent boundary pairs. The trilateral case has 2 boundaries
(IS, SP). The quadrilateral adds 2 more (PT, TI — topology↔impl, topology↔intent).
The N-lateral case has N boundaries for an N-gon.

Each boundary requires:
  1. A LIVE projection (partition of attribute namespace)
  2. A divergence function Dᵢ computing gap across the boundary
  3. A weight wᵢ (stored as a datom, tunable)

#### Consequences
- Φ(S) formula generalized to weighted sum over N boundaries
- INV-TRILATERAL-005 phrasing encompasses "all N projections are pairwise disjoint"
- Adding a vertex requires: define ATTRS, implement Dᵢ, register wᵢ
- No existing code changes — current 3-vertex case is a specialization
- Stage 3 adds the 4th vertex (topology) via this extension mechanism

#### Falsification
This decision is wrong if: the N-lateral generalization introduces overhead or
complexity for the trilateral case that makes the 3-vertex implementation worse
than a hardcoded trilateral.

---

### ADR-TRILATERAL-005: Cohomological Complement to Divergence Metric

**Traces to**: INV-TRILATERAL-009 (coherence duality), INV-TRILATERAL-001 (Φ),
  INV-QUERY-024 (β₁), exploration/sheaf-coherence/00-sheaf-cohomology.md §6
**Stage**: 0

#### Problem
The divergence metric Φ counts unlinked boundaries (missing edges in the
coherence graph). A store can achieve Φ = 0 while having contradictory
information flowing around a cycle of links (all links present, but mutually
inconsistent). Should the trilateral model incorporate a topological invariant
to detect this blind spot?

#### Options
A) **Add β₁ (first Betti number) as a formal complement to Φ** — Compute
   H¹ of the coherence graph via the edge Laplacian and report (Φ, β₁)
   as the coherence state
   - Pro: Mathematically complete — (Φ, β₁) = (0, 0) is necessary and sufficient
     for coherence (no gaps, no cycles)
   - Pro: Reuses existing nalgebra infrastructure (ADR-QUERY-012, INV-QUERY-023)
   - Pro: Provides actionable diagnostics: harmonic representatives locate the
     exact cycle and recommend where to intervene
   - Con: Adds a linear algebra computation to every coherence check

B) **Rely on Φ alone with ad-hoc cycle detection** — Keep Φ as the only
   metric, add a separate cycle-detection pass as a heuristic warning
   - Pro: Simpler mental model (one number)
   - Con: Cycle detection without cohomology cannot determine independence
     of cycles, leading to over- or under-counting
   - Con: No formal completeness guarantee

C) **Defer to contradiction engine** — Use the existing 5-tier contradiction
   detector (from the Go CLI) to find pairwise semantic contradictions
   - Pro: Leverages existing infrastructure
   - Con: Pairwise contradiction detection misses *cyclic* contradictions
     where each pair is locally consistent but the cycle is globally incoherent
   - Con: O(n²) pairwise comparisons vs O(|E|²) for L₁ eigendecomposition

#### Decision
**Option A.** The pair (Φ, β₁) is the minimal complete coherence characterization.
Φ detects zeroth-order problems (missing links); β₁ detects first-order problems
(contradictory cycles). Neither subsumes the other — the four quadrants of
(Φ > 0, β₁ > 0) have distinct resolution strategies (INV-TRILATERAL-009).

The computation cost is negligible: for Stage 0 (single agent), β₁ = 0 by
construction (no multi-agent cycles), so the ISP triangle check
(INV-TRILATERAL-008) is the effective implementation. For Stage 2+, the edge
Laplacian eigendecomposition is O(m²) where m = |E| ≤ 45 (for n ≤ 10 agents),
completing in microseconds.

#### Consequences
- `braid coherence` reports (Φ, β₁, quadrant) instead of Φ alone
- The fitness function F(S) extends to include a β₁ term (see INV-TRILATERAL-009)
- Guidance injection includes cycle diagnostics when β₁ > 0
- The self-bootstrap coherence check (INV-TRILATERAL-007) verifies β₁ = 0
  for the spec's own trilateral structure

#### Falsification
This decision is wrong if: the (Φ, β₁) pair fails to detect a class of
coherence failures that exists in practice — i.e., a store state that is
incoherent but has Φ = 0 ∧ β₁ = 0. By the completeness theorem
(INV-TRILATERAL-009 Level 0), this cannot happen for the coherence graph
model, but it could happen if the coherence graph fails to capture some
aspect of store coherence (e.g., higher-order incoherence requiring H²).

---

### ADR-TRILATERAL-006: F₂ Coefficients for Initial Cohomology

**Traces to**: INV-QUERY-023, INV-QUERY-024,
  exploration/sheaf-coherence/00-sheaf-cohomology.md §3
**Stage**: 0

#### Problem
The edge Laplacian (INV-QUERY-023) works over ℝ, using real-valued eigenvectors.
The underlying coherence question is binary: for each edge (agent pair) and
attribute, do the two agents agree (0) or disagree (1)? What coefficient field
should the cohomology computation use?

#### Options
A) **ℝ (real numbers) via eigendecomposition** — Compute L₁ over ℝ,
   detect zero eigenvalues with floating-point tolerance ε = 1e-10
   - Pro: Reuses nalgebra's `symmetric_eigenvalues()` — no new code
   - Pro: Harmonic representatives are real vectors, enabling weighted
     energy computations for resolution-mode-aware diagnostics
   - Con: Floating-point tolerance introduces potential for false
     positives/negatives near the ε boundary

B) **F₂ (binary field) via rank computation** — Compute β₁ = dim(ker(∂₁))
   - dim(im(∂₀)) using exact arithmetic over GF(2)
   - Pro: Exact computation, no floating-point issues
   - Con: Requires implementing GF(2) matrix operations (Smith normal form
     or Gaussian elimination mod 2) — new code
   - Con: Loses the weighted Hodge decomposition (weights are meaningless
     over F₂)

C) **Dual approach** — F₂ for β₁ count (exact), ℝ for harmonic representatives
   (diagnostic)
   - Pro: Best of both — exact count with rich diagnostics
   - Con: Two separate computations; complexity not justified at Stage 0

#### Decision
**Option A.** Real coefficients via the existing eigendecomposition, with
tolerance ε = 1e-10. The nalgebra eigenvalue solver is numerically stable for
the class of matrices that arise (symmetric, positive semi-definite, small
dimension m ≤ 45). The real-valued computation also gives the weighted Hodge
decomposition for free, which is essential for resolution-mode-aware diagnostics
(INV-QUERY-023 Level 1).

The tolerance ε = 1e-10 is well above machine epsilon (~1e-16) and well below
any meaningful eigenvalue gap (the smallest non-zero eigenvalue of L₁ for a
connected graph is the 1-form spectral gap μ₁ > 0, typically O(1/|E|)).
UNC-TRILATERAL-003 tracks the open question of whether F₂ or a dual approach
is needed for large-scale deployments.

#### Consequences
- β₁ computation is `eigenvalues.filter(|v| v.abs() < 1e-10).count()`
- Harmonic representatives are the corresponding eigenvectors
- Weighted edge Laplacian L₁(W) gives resolution-mode-aware β₁
- The ε = 1e-10 tolerance is a configurable constant, not hardcoded
  in business logic (stored as config datom, C3)

#### Falsification
This decision is wrong if: the floating-point eigenvalue computation produces
a false zero (eigenvalue < ε that is actually non-zero) or a false non-zero
(eigenvalue > ε that is actually zero) for any coherence graph arising from
a real DDIS store. This would be observable as a disagreement between the
real-valued β₁ and the F₂ β₁ for the same graph.

---

### §18.6 Negative Cases

### NEG-TRILATERAL-001: No Cross-View Contamination

**Traces to**: INV-TRILATERAL-005
**Verification**: `V:TYPE`, `V:PROP`

**Safety property**: `□ ¬(∃ datom d: d ∈ LIVE_X(S) ∧ d.a ∉ X_ATTRS)` for X ∈ {I, S, P}

No datom appears in a LIVE view unless its attribute belongs to that view's
namespace partition. The three views are disjoint by construction — a spec
datom never contaminates the intent view, an impl datom never contaminates
the spec view.

Cross-cutting attributes (`:db/*`, `:tx/*`) belong to META_ATTRS and appear
in none of the three LIVE views. They are queryable directly from the store.

**proptest strategy**: Generate random datoms with random attributes. Verify
that each datom appears in exactly one LIVE view (or none, if META_ATTRS).
No datom appears in two or more views.

---

### NEG-TRILATERAL-002: No External State for Divergence

**Traces to**: INV-TRILATERAL-002, NEG-INTERFACE-001
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ external_state E: Φ computation reads E ∧ E ∉ S)`

Φ is a pure function of the store. No external files, no session state, no
configuration outside the store contributes to the divergence computation.
Boundary weights (w₁, w₂) are datoms in the store (schema-as-data, C3).

This ensures that two agents with the same store compute the same Φ — there
is no "my divergence is different from yours" based on local state.

**proptest strategy**: Compute Φ from a store. Serialize and deserialize the
store. Recompute Φ. Values must be identical. No external file reads during
Φ computation.

---

### NEG-TRILATERAL-003: No Divergence Increase from Convergence Operations

**Traces to**: INV-TRILATERAL-004, INV-BILATERAL-001
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ LINK operation: Φ(S') > Φ(S))`

Adding a `:spec/traces-to` or `:spec/implements` link cannot increase Φ. These links
can only connect previously unlinked datoms (reducing divergence) or be
redundant (no change to divergence). No link addition creates new divergence.

Note: TRANSACT operations for new intent/spec/impl datoms CAN increase Φ
(new work creates new divergence). This negative case applies only to
convergence operations (link additions), not to all transactions.

**proptest strategy**: Generate random stores with random unlinked datoms.
Add random `:spec/traces-to` and `:spec/implements` links. Verify Φ never increases
after any link addition.

---

### NEG-TRILATERAL-004: No Φ-Only Coherence Declaration

**Traces to**: INV-TRILATERAL-009 (coherence duality), ADR-TRILATERAL-005
**Verification**: `V:TYPE`

**Safety property**: `□ ¬(coherence_check returns COHERENT when β₁ > 0)`

The system must never declare a store "coherent" based solely on Φ = 0
without also verifying β₁ = 0. The state Φ = 0, β₁ > 0 means all links
exist but contain contradictory cycles — this is an incoherent store that
would pass a Φ-only check.

This is enforced at the type level: the `CoherenceReport` struct requires
both `phi` and `beta_1` fields, and `CoherenceQuadrant::Coherent` is only
constructible when both are zero (INV-TRILATERAL-009 Level 2). There is
no code path that evaluates coherence from Φ alone.

**Violation condition**: Any code path, CLI command, or API endpoint that
reports "coherent" or equivalent without computing β₁. Any conditional
that checks `phi == 0` without also checking `beta_1 == 0`.

---

### §18.7 Uncertainty Register

### UNC-TRILATERAL-001: Boundary Weight Calibration

**Confidence**: 0.5
**What would resolve it**: Empirical data from Stage 0/1 usage showing which
boundary (I↔S vs S↔P) produces more actionable divergence signals. Adjust
w₁, w₂ based on observed false-positive rates per boundary.

### UNC-TRILATERAL-002: Attribute Namespace Completeness

**Confidence**: 0.7
**What would resolve it**: Stage 0 implementation revealing cross-cutting
attributes that do not fit cleanly into INTENT/SPEC/IMPL/META partitions.
The current partition may need extension for coordination-specific attributes
(e.g., `:deliberation/*`, `:signal/*`).

---

### UNC-TRILATERAL-003: Coefficient Generalization (ℝ vs F₂)

**Confidence**: 0.8
**What would resolve it**: Stage 2+ deployment with ≥ 3 agents producing a
coherence graph where the real-valued β₁ (via eigendecomposition with ε = 1e-10)
disagrees with the exact F₂ β₁ (via mod-2 rank computation). If no disagreement
is observed across 1000+ coherence checks, the ℝ approach is validated. If
disagreement occurs, the dual approach (Option C from ADR-TRILATERAL-006)
must be implemented.

Current assessment: for the graph sizes arising in DDIS (n ≤ 10 agents,
m ≤ 45 edges), the eigenvalue gap between zero and non-zero eigenvalues is
large enough that ε = 1e-10 is safe. This uncertainty is relevant only if
DDIS scales to significantly larger agent populations.

---

### UNC-TRILATERAL-004: Sheaf Laplacian Refinement

**Confidence**: 0.6
**What would resolve it**: Stage 2+ implementation comparing the standard
weighted edge Laplacian L₁(W) (INV-QUERY-023) against the full sheaf Laplacian
L_F (Hansen-Ghrist construction) for detecting resolution-mode-dependent
convergence bottlenecks.

The sheaf Laplacian uses per-edge restriction maps derived from resolution modes
(LWW → binary agreement, lattice → lattice distance, multi-value → Jaccard
distance), giving a tighter convergence bound (sheaf spectral gap μ_F ≤ μ₁).
If μ_F << μ₁ in practice, the sheaf Laplacian reveals semantic bottlenecks
invisible to the standard construction — this would justify upgrading from
L₁(W) to L_F. Until empirical evidence exists, the weighted edge Laplacian
(simpler, well-understood) is the correct choice.

---

### §18.8 Relationship to Existing Namespaces

- **Extends BILATERAL** (§10): bilateral loop → trilateral coherence. The
  bilateral fitness function F(S) operates on D_SP; the trilateral Φ adds D_IS.
  AUTO_CYCLE/FULL_CYCLE from INV-BILATERAL-002 operate within the trilateral
  framework.
- **Depends on STORE** (§1): INV-STORE-012 LIVE index is the foundation for all
  three LIVE projections. The append-only store (C1) guarantees formality
  monotonicity.
- **Depends on SCHEMA** (§2): Attribute namespace partitioning relies on the
  schema system to enforce that each attribute belongs to exactly one namespace.
- **Depends on QUERY** (§3): Φ as Datalog program (INV-TRILATERAL-006) depends
  on INV-QUERY-001 (CALM-compliant monotonic reads) for correctness guarantees.
  β₁ computation (INV-TRILATERAL-009) depends on INV-QUERY-023 (edge Laplacian)
  and INV-QUERY-024 (Betti number extraction) for the spectral computation.
- **Depends on LAYOUT** (§1b): Per-transaction content-addressed `.edn` files
  provide the VCS-compatible storage structure that makes Φ computation possible
  from the file system.
- **Informs HARVEST** (§5): Harvest is the primary mechanism for populating
  LIVE_I — extracting intent from conversations into store datoms.
- **Informs SEED** (§6): Seed assembly queries LIVE_I, LIVE_S, and LIVE_P to
  build context-aware output.
