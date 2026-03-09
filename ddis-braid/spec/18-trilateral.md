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
The TRILATERAL spec elements (INV-TRILATERAL-001..007, ADR-TRILATERAL-001..003,
NEG-TRILATERAL-001..003) are datoms in the store.

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
- **Depends on LAYOUT** (§1b): Per-transaction content-addressed `.edn` files
  provide the VCS-compatible storage structure that makes Φ computation possible
  from the file system.
- **Informs HARVEST** (§5): Harvest is the primary mechanism for populating
  LIVE_I — extracting intent from conversations into store datoms.
- **Informs SEED** (§6): Seed assembly queries LIVE_I, LIVE_S, and LIVE_P to
  build context-aware output.
