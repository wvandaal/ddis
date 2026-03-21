> **DEPRECATED**: This file is bootstrap scaffolding. The canonical source of truth is the braid datom store. Use `braid spec show` and `braid query` to access spec elements. See ADR-STORE-019.

---

> **Namespace**: SCHEMA | **Wave**: 1 (Foundation) | **Stage**: 0
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §2. SCHEMA — Schema-as-Data

### §2.0 Overview

Schema in Braid is not a separate DDL or configuration file — it is data in the store
itself. The schema is a set of datoms that describe what attributes exist, what types
they expect, and how they behave during conflict resolution. Schema evolution is a
transaction, not a migration.

**Traces to**: SEED.md §4, C3
**docs/design/ADRS.md sources**: FD-002, FD-008, SR-008, SR-009, SR-010, PO-012

---

### §2.1 Level 0: Algebraic Specification

#### Meta-Schema Recursion

```
The schema S_schema ⊂ S is a subset of datoms in the store.
Schema datoms describe attributes; attributes describe datoms.

Self-reference: the meta-schema attributes describe themselves.
  e.g., :db/valueType has valueType :db.type/keyword
        :db/cardinality has cardinality :db.cardinality/one

Formally: Let A₀ = {a₁, ..., a₁₉} be the 19 axiomatic meta-schema attributes.
∀ aᵢ ∈ A₀: ∃ datoms in S₀ (genesis) that define aᵢ using A₀ itself.
The meta-schema is the fixed point of "attributes that describe attributes."
```

#### Schema as Monotonic Extension

```
Schema evolution is store growth:
  schema(S) ⊆ schema(S')   whenever S ⊆ S'

New attributes are added by asserting new datoms. Existing attributes are never removed
(C1 — append-only). Attribute properties can be "changed" by asserting new values and
retracting old ones, but the history of every schema change is preserved.
```

#### Attribute Algebra

```
Attribute a is fully specified by:
  :db/ident        — keyword name (e.g., :task/status)
  :db/valueType    — the value domain (one of the 14 value types)
  :db/cardinality  — :one | :many
  :db/resolutionMode — :lww | :lattice | :multi  (per-attribute conflict resolution)
  :db/doc          — documentation string

Optional:
  :db/unique       — :identity | :value (uniqueness constraint)
  :db/isComponent  — boolean (component entity lifecycle)
  :db/latticeOrder — ref to lattice definition (if resolutionMode = :lattice)
  :db/lwwClock     — :hlc | :wall | :agent-rank (if resolutionMode = :lww)
```

---

### §2.2 Level 1: State Machine Specification

#### Genesis Transaction

```
GENESIS() → S₀ containing exactly:

For each of the 19 axiomatic attributes aᵢ:
  (aᵢ, :db/ident,        <keyword>,     tx₀, Assert)
  (aᵢ, :db/valueType,    <type>,        tx₀, Assert)
  (aᵢ, :db/cardinality,  <cardinality>, tx₀, Assert)
  (aᵢ, :db/doc,          <description>, tx₀, Assert)
  ... (additional properties as needed)

tx₀ has no causal predecessors.
tx₀ is the root of the causal graph.
```

#### M(t) Default Weight Bootstrap Datoms

The M(t) methodology adherence weights (INV-GUIDANCE-008) are installed as genesis
datoms so that the guidance system has defined weights from the first transaction.
These defaults are overridable via subsequent transactions (schema-as-data, C3).

```clojure
[:guidance/m-weight :guidance.weight/coverage        0.25]
[:guidance/m-weight :guidance.weight/harvest-rate     0.20]
[:guidance/m-weight :guidance.weight/contradiction    0.15]
[:guidance/m-weight :guidance.weight/traceability     0.25]
[:guidance/m-weight :guidance.weight/formality        0.15]
```

These five weights sum to 1.0 and correspond to the five independently measurable
components of the methodology score. See `docs/guide/08-guidance.md` and
`spec/12-guidance.md` (INV-GUIDANCE-008) for the full M(t) computation.

#### The 17 Axiomatic Attributes

```
Layer 0 — Meta-Schema (self-describing):
  :db/ident           — Keyword    :one    — attribute's keyword name
  :db/valueType       — Keyword    :one    — value type constraint
  :db/cardinality     — Keyword    :one    — :one or :many
  :db/doc             — String     :one    — documentation
  :db/unique          — Keyword    :one    — :identity or :value
  :db/isComponent     — Boolean    :one    — component lifecycle binding
  :db/resolutionMode  — Keyword    :one    — :lww, :lattice, or :multi
  :db/latticeOrder    — Ref        :one    — ref to lattice definition entity
  :db/lwwClock        — Keyword    :one    — :hlc, :wall, or :agent-rank

Lattice definition attributes:
  :lattice/ident      — Keyword    :one    — lattice name
  :lattice/elements   — Keyword    :many   — set of lattice elements
  :lattice/comparator — String     :one    — ordering function name
  :lattice/bottom     — Keyword    :one    — bottom element
  :lattice/top        — Keyword    :one    — top element (if bounded)

Transaction metadata:
  :tx/time            — Instant    :one    — wall-clock time
  :tx/agent           — Ref        :one    — agent who transacted (→ agent entity)
  :tx/provenance      — Keyword    :one    — provenance type

Agent entity attributes (Layer 1 — provenance infrastructure, ADR-STORE-020):
  :agent/ident        — Keyword    :one    lww    — human-readable agent name
  :agent/program      — Keyword    :one    lww    — harness (claude-code, codex, gemini, human)
  :agent/model        — Keyword    :one    lww    — LLM model (opus-4.6, sonnet-4.6, o3, human)
  :agent/session-id   — String     :one    lww    — session disambiguation token

  Forward reference — Layer 4 extensions (Stage 2–3):
  :agent/capabilities — Keyword    :many   multi  — domain competencies
  :agent/trust        — Double     :one    lww    — trust score T ∈ [0, 1]
  :agent/status       — Keyword    :one    lattice — agent-lifecycle lattice
```

#### Layer 2 Spec-Element Attributes (Self-Bootstrap)

The following attributes define how specification elements are represented as datoms.
These belong to Layer 2 (DDIS Core) and are installed after genesis + Layer 1 (Agent &
Provenance). They enable the self-bootstrap commitment (C7): the specification is the
first dataset the system manages.

```
Core identity (required for all spec elements):
  :spec/id              — String     :one    lww    — Element ID (e.g., "INV-STORE-001")
  :spec/type            — Keyword    :one    lww    — invariant | adr | negative-case | uncertainty
  :spec/namespace       — Keyword    :one    lww    — STORE | SCHEMA | QUERY | RESOLUTION | ...
  :spec/statement       — String     :one    lww    — Primary statement text

Traceability:
  :spec/traces-to       — String     :many   multi  — References (e.g., "SEED §4 Axiom 2", "C1")
  :spec/depends-on      — Ref        :many   multi  — X requires Y (sequencing constraint)
  :spec/affects         — Ref        :many   multi  — X changes interpretation of Y (always non-monotonic)
  :spec/constrains      — Ref        :many   multi  — X bounds solution space of Y (always monotonic)
  :spec/tests           — Ref        :many   multi  — X verifies Y (always monotonic)

Verification & falsification:
  :spec/falsification   — String     :one    lww    — Violation condition text
  :spec/verification    — Keyword    :many   multi  — V:TYPE, V:PROP, V:KANI, V:MODEL, ...
  :spec/stage           — Long       :one    lww    — Implementation stage (0, 1, 2, 3, 4)
  :spec/witnessed       — Boolean    :one    lww    — Invariant has been witnessed (test evidence)
  :spec/challenged      — Boolean    :one    lww    — Invariant has been challenged (adversarial verification)

Three-level refinement fidelity (Mills cleanroom):
  :spec/level-0         — String     :one    lww    — Algebraic law text (Level 0)
  :spec/level-1         — String     :one    lww    — State machine invariant text (Level 1)
  :spec/level-2         — String     :one    lww    — Implementation contract text (Level 2)

ADR-specific:
  :spec/adr-problem     — String     :one    lww    — Problem statement
  :spec/adr-options     — String     :many   multi  — Option descriptions (one per option)
  :spec/adr-decision    — String     :one    lww    — Chosen option with rationale
  :spec/adr-alternatives— String     :many   multi  — Rejected alternatives with reasons

Negative case-specific:
  :spec/neg-violation   — String     :one    lww    — What constitutes a violation
  :spec/neg-safety      — String     :one    lww    — Safety property in temporal logic

Uncertainty:
  :spec/confidence      — Double     :one    lww    — Confidence level (0.0–1.0)
  :spec/resolves-when   — String     :one    lww    — What would resolve the uncertainty

Proptest / verification detail:
  :spec/proptest        — String     :one    lww    — Proptest strategy description
```

Trilateral coherence attributes (Layer 2 — required for C7 self-verification):

```
Intent namespace:
  :intent/noted         — Boolean    :one    lww    — Entity acknowledged, not yet formalized
  :intent/decision      — String     :one    lww    — Decision text captured from conversation
  :intent/rationale     — String     :one    lww    — Reasoning behind a decision
  :intent/source        — String     :one    lww    — Origin context (conversation ID, user directive)
  :intent/goal          — String     :one    lww    — Goal statement this entity represents
  :intent/constraint    — String     :one    lww    — Constraint or boundary condition
  :intent/preference    — String     :one    lww    — Non-binding preference or heuristic

Implementation namespace:
  :impl/module          — String     :one    lww    — Source module path
  :impl/file            — String     :one    lww    — Source file path
  :impl/signature       — String     :one    lww    — Function/type signature
  :impl/implements      — Ref        :many   multi  — Impl entity → spec entity (typed link)
  :impl/test-result     — Keyword    :one    lattice — Test result (:untested < :failing < :passing)
  :impl/coverage        — Double     :one    lww    — Test coverage fraction [0.0, 1.0]

Cross-boundary link:
  :spec/implements      — Ref        :many   multi  — Impl entity → spec entity it implements
```

This gives 26 spec-element attributes (23 original + 3 typed relationship attributes:
`:spec/affects`, `:spec/constrains`, `:spec/tests`) plus 14 trilateral coherence
attributes (40 total in Layer 2). Combined with the 19 axiomatic attributes and Layer 1 agent/provenance
attributes, the Stage 0 schema is sufficient to represent all specification elements in
the datom store with full fidelity across all three refinement levels and all element
types (invariant, ADR, negative case, uncertainty).

#### Schema Evolution as Transaction

```
ADD-ATTRIBUTE(S, attr_spec) → S'

PRE:
  attr_spec contains at minimum: :db/ident, :db/valueType, :db/cardinality
  No existing attribute has the same :db/ident (unless this is a schema update)

POST:
  S'.datoms = S.datoms ∪ {datoms defining the new attribute}
  schema(S') ⊃ schema(S)

SCHEMA-UPDATE(S, attr_ident, property, new_value) → S'

PRE:
  attr_ident exists in schema(S)
  property is a valid meta-schema attribute
  new_value is compatible with the meta-schema attribute's type

POST:
  S'.datoms = S.datoms ∪ {(attr_entity, property, new_value, tx, Assert)}
  The old value is NOT removed (append-only). LIVE index resolves to new value.
```

---

### §2.3 Level 2: Interface Specification

```rust
/// The 19 axiomatic attributes — hardcoded in the engine.
pub mod meta_schema {
    pub const DB_IDENT: Attribute = Attribute::from_keyword(":db/ident");
    pub const DB_VALUE_TYPE: Attribute = Attribute::from_keyword(":db/valueType");
    pub const DB_CARDINALITY: Attribute = Attribute::from_keyword(":db/cardinality");
    pub const DB_DOC: Attribute = Attribute::from_keyword(":db/doc");
    pub const DB_UNIQUE: Attribute = Attribute::from_keyword(":db/unique");
    pub const DB_IS_COMPONENT: Attribute = Attribute::from_keyword(":db/isComponent");
    pub const DB_RESOLUTION_MODE: Attribute = Attribute::from_keyword(":db/resolutionMode");
    pub const DB_LATTICE_ORDER: Attribute = Attribute::from_keyword(":db/latticeOrder");
    pub const DB_LWW_CLOCK: Attribute = Attribute::from_keyword(":db/lwwClock");
    pub const LATTICE_IDENT: Attribute = Attribute::from_keyword(":lattice/ident");
    pub const LATTICE_ELEMENTS: Attribute = Attribute::from_keyword(":lattice/elements");
    pub const LATTICE_COMPARATOR: Attribute = Attribute::from_keyword(":lattice/comparator");
    pub const LATTICE_BOTTOM: Attribute = Attribute::from_keyword(":lattice/bottom");
    pub const LATTICE_TOP: Attribute = Attribute::from_keyword(":lattice/top");
    pub const TX_TIME: Attribute = Attribute::from_keyword(":tx/time");
    pub const TX_AGENT: Attribute = Attribute::from_keyword(":tx/agent");
    pub const TX_PROVENANCE: Attribute = Attribute::from_keyword(":tx/provenance");
}

/// Schema is owned by Store internally, derived from schema datoms (ADR-SCHEMA-005, Option C).
/// Constructed via Schema::from_store() on load and after schema-modifying transactions.
/// Exposed via store.schema() -> &Schema (zero-cost borrow).
pub struct Schema { /* fields extracted from schema datoms */ }

impl Schema {
    /// Reconstruct schema from store datoms (the only constructor — enforces C3).
    pub fn from_store(datoms: &BTreeSet<Datom>) -> Schema;

    /// Look up attribute definition by attribute keyword.
    pub fn attribute(&self, ident: &Attribute) -> Option<&AttributeDef>;

    /// Validate a datom against schema (attribute existence + value type match).
    pub fn validate_datom(&self, datom: &Datom) -> Result<(), SchemaValidationError>;

    /// Produce datoms for a new attribute definition (caller wraps in Transaction).
    pub fn new_attribute(&self, spec: AttributeSpec) -> Vec<Datom>;

    /// All known attributes.
    pub fn attributes(&self) -> impl Iterator<Item = (&Attribute, &AttributeDef)>;

    /// Resolution mode for an attribute.
    pub fn resolution_mode(&self, attr: &Attribute) -> ResolutionMode;

    /// LWW clock selection for an attribute (when resolution mode = LWW).
    pub fn lww_clock(&self, attr: &Attribute) -> LwwClock;
}

/// Clock selection for LWW resolution (INV-RESOLUTION-005).
/// Stored as :db/lwwClock keyword on the attribute entity. Determines which
/// ordering function is used when two assertions have equal HLC timestamps.
pub enum LwwClock {
    /// Hybrid Logical Clock ordering (default). Most precise.
    Hlc,
    /// Wall-clock ordering. Less precise but simpler.
    Wall,
    /// Agent rank ordering. Deterministic hierarchy among agents.
    AgentRank,
}

impl Store {
    /// Borrow the schema — zero cost, derived from store datoms on load.
    pub fn schema(&self) -> &Schema { &self.schema }
}

/// Six-layer schema architecture (INV-SCHEMA-006).
/// Each layer depends only on layers below it; Stage 0 installs Layers 0–1.
pub enum SchemaLayer {
    MetaSchema,       // Layer 0: 19 axiomatic attributes (9 :db/*, 5 :lattice/*, 5 :tx/*)
    AgentProvenance,  // Layer 1: 3 types, 20 attributes (agent entity + tx metadata + provenance)
    DdisCore,         // Layer 2: 12 types, 72 attributes (spec elements, observations)
    Discovery,        // Layer 3: 5 types, 28 attributes (threads, findings)
    Coordination,     // Layer 4: 7 types, 35 attributes (deliberation, sync)
    Workflow,         // Layer 5: 5 types, 27 attributes (tasks, workspace)
}

/// Errors from datom-level schema validation (INV-SCHEMA-004).
/// Returned by Schema::validate_datom() when a datom fails type checking.
pub enum SchemaValidationError {
    /// Datom references an attribute not defined in the schema.
    UnknownAttribute(Attribute),
    /// Datom's value type does not match the attribute's declared :db/valueType.
    TypeMismatch { attr: Attribute, expected: ValueType, got: ValueType },
    /// Datom violates the attribute's cardinality constraint (:one vs :many).
    CardinalityViolation { attr: Attribute, cardinality: Cardinality },
    /// Datom's value is not a valid element of the attribute's declared lattice.
    InvalidLatticeValue { attr: Attribute, value: Value, lattice: String },
    /// Retraction targets an entity-attribute pair with no prior assertion.
    InvalidRetraction { entity: EntityId, attr: Attribute },
}

/// Errors from schema operations (attribute definition, schema evolution).
/// Distinct from SchemaValidationError (which covers datom-level type checking
/// in INV-SCHEMA-004); SchemaError covers schema definition operations.
pub enum SchemaError {
    /// Attempt to define an attribute with an :db/ident that already exists.
    DuplicateAttribute(Attribute),
    /// Attribute has invalid cardinality value (not :one or :many).
    InvalidCardinality,
    /// Schema layer dependency violation (NEG-SCHEMA-003): attribute in Layer N
    /// references entity type from Layer M where M > N.
    LayerDependencyViolation { attr: Attribute, attr_layer: SchemaLayer, ref_layer: SchemaLayer },
}
```

#### CLI Commands

```
braid schema                          # List all attributes with types
braid schema add --ident :task/status --type keyword --cardinality one --resolution lattice
braid schema show :task/status        # Show attribute definition and history
```

---

### §2.4 Invariants

### INV-SCHEMA-001: Schema-as-Data

**Traces to**: SEED §4, C3, ADRS FD-008
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
schema(S) ⊂ S
  (the schema is a subset of the store, not a separate structure)
∀ attribute definitions: they are datoms in the store
```

#### Level 1 (State Invariant)
There is no schema file, DDL, or configuration outside the store.
All attribute definitions are queryable via the same query engine as any other datoms.

#### Level 2 (Implementation Contract)
```rust
// Schema is derived from store datoms on load, owned by Store (ADR-SCHEMA-005).
pub struct Schema { /* fields extracted from schema datoms */ }

impl Schema {
    /// Reconstruct schema from store datoms (the only constructor).
    pub fn from_store(datoms: &BTreeSet<Datom>) -> Schema { /* ... */ }
    pub fn attribute(&self, ident: &Attribute) -> Option<&AttributeDef> { /* ... */ }
}

impl Store {
    /// Borrow the schema — zero cost, derived from store datoms on load.
    pub fn schema(&self) -> &Schema { &self.schema }
}
```

**Falsification**: Any attribute definition that exists outside the datom store (e.g., in a
config file, hardcoded enum, or separate database table).

---

### INV-SCHEMA-002: Genesis Completeness

**Traces to**: SEED §10, ADRS PO-012, SR-008
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ aᵢ ∈ A₀ (the 19 axiomatic attributes):
  ∃ datoms in GENESIS() defining aᵢ
  AND those datoms use only attributes from A₀
  (the meta-schema is self-contained)
```

#### Level 1 (State Invariant)
The genesis transaction contains exactly the 19 axiomatic attribute definitions.
Each attribute is fully specified (ident, valueType, cardinality at minimum).
No non-meta-schema datoms exist in genesis.

#### Level 2 (Implementation Contract)
```rust
fn genesis() -> Store {
    let mut store = Store::empty();
    let tx = Transaction::<Building>::new(SYSTEM_AGENT)
        .with_provenance(ProvenanceType::Observed);
    // Assert exactly 19 attributes...
    // Assert each attribute's ident, valueType, cardinality, doc
    let tx = tx.commit_genesis();  // special: bypasses schema validation (bootstrap)
    store.apply_genesis(tx);
    assert_eq!(store.schema().attributes().count(), 19);
    store
}
```

**Falsification**: A genesis store where `schema.attributes().count() != 19`, or where
any axiomatic attribute lacks a complete definition.

---

### INV-SCHEMA-003: Schema Monotonicity

**Traces to**: SEED §4, C1, C3
**Verification**: `V:TYPE`, `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ S ⊆ S': schema(S) ⊆ schema(S')
  (schema can only grow; attributes are never removed)
```

#### Level 1 (State Invariant)
Once an attribute is defined, it is permanently part of the schema. Its properties
may be updated (via new datoms), but the attribute identity persists forever.

**Falsification**: An operation that removes an attribute from the schema.

---

### INV-SCHEMA-004: Schema Validation on Transact

**Traces to**: SEED §4, ADRS PO-001
**Verification**: `V:TYPE`, `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ TRANSACT(S, T):
  ∀ d ∈ T.datoms:
    d.a ∈ schema(S)                              — attribute must exist
    typeof(d.v) = schema(S).valueType(d.a)       — value type must match
    (d.op = Retract ⟹ ∃ d' ∈ S: d'.e = d.e ∧ d'.a = d.a ∧ d'.op = Assert)
      — can only retract what was asserted
```

#### Level 1 (State Invariant)
No datom with an undefined attribute or mistyped value enters the store.
Retractions require a prior assertion of the same entity-attribute pair.

#### Level 2 (Implementation Contract)
```rust
impl Transaction<Building> {
    pub fn commit(self, schema: &Schema) -> Result<Transaction<Committed>, TxValidationError> {
        for datom in &self.datoms {
            let attr_def = schema.attribute(&datom.attribute)
                .ok_or(TxValidationError::UnknownAttribute(datom.attribute.clone()))?;
            attr_def.validate_value(&datom.value)?;
        }
        Ok(Transaction { _state: PhantomData::<Committed>, ..self })
    }
}
```

**Falsification**: A datom with attribute `:foo/bar` entering the store when no attribute
`:foo/bar` is defined in the schema.

---

### INV-SCHEMA-005: Meta-Schema Self-Description

**Traces to**: ADRS SR-008
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ aᵢ ∈ A₀: aᵢ is described by datoms that use only attributes from A₀
  (the meta-schema is a fixed point: it describes itself using itself)
```

#### Level 1 (State Invariant)
The `:db/ident` attribute has a datom `(:db/ident, :db/valueType, :db.type/keyword, tx₀, Assert)`.
This datom describes `:db/ident`'s value type using the `:db/valueType` attribute, which is
itself one of the 19 axiomatic attributes.

**Falsification**: Any axiomatic attribute whose definition requires an attribute outside A₀.

---

### INV-SCHEMA-006: Six-Layer Schema Architecture

**Traces to**: ADRS SR-009
**Verification**: `V:PROP`
**Stage**: 0–4 (progressive)

#### Level 0 (Algebraic Law)
```
Schema is organized into 6 layers:
  Layer 0: Meta-schema (19 axiomatic attributes)        — Stage 0
  Layer 1: Agent & Provenance (2 types, 16 attributes)  — Stage 0
  Layer 2: DDIS Core (12 types, 72 attributes)          — Stage 0–1
  Layer 3: Discovery & Exploration (5 types, 28 attrs)  — Stage 1–2
  Layer 4: Coordination (7 types, 35 attributes)        — Stage 2–3
  Layer 5: Workflow & Task (5 types, 27 attributes)     — Stage 3–4

Each layer depends only on layers below it.
```

#### Level 1 (State Invariant)
Attributes in Layer N reference only entity types defined in Layers 0..N.

**Falsification**: A Layer 1 attribute that references a Layer 3 entity type.

---

### INV-SCHEMA-007: Lattice Definition Completeness

**Traces to**: ADRS SR-010
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ attributes a with :db/resolutionMode = :lattice:
  ∃ lattice entity L such that:
    a.:db/latticeOrder = L
    L.:lattice/ident is defined
    L.:lattice/elements is non-empty
    L.:lattice/comparator names a valid ordering function
    L.:lattice/bottom ∈ L.:lattice/elements

  AND the comparator defines a valid join-semilattice over the declared elements:

  Semilattice witness requirement (R2.4):
    Let E = L.:lattice/elements and ≤ = L.:lattice/comparator. Then:

    (1) Reflexivity:     ∀ x ∈ E: x ≤ x
    (2) Antisymmetry:    ∀ x, y ∈ E: (x ≤ y ∧ y ≤ x) ⟹ x = y
    (3) Transitivity:    ∀ x, y, z ∈ E: (x ≤ y ∧ y ≤ z) ⟹ x ≤ z
    (4) Join existence:  ∀ x, y ∈ E: ∃ j ∈ E:
                           j ≥ x ∧ j ≥ y                          (upper bound)
                           ∧ (∀ u ∈ E: u ≥ x ∧ u ≥ y ⟹ u ≥ j)   (least)
    (5) Join uniqueness: The j in (4) is unique for each pair (x, y)
    (6) Bottom validity: ∀ x ∈ E: L.:lattice/bottom ≤ x

  These properties MUST be verified at schema-registration time (when the lattice
  definition is transacted). For finite element sets, verification is exhaustive
  over all pairs/triples — O(|E|²) for (1)-(2) and (4)-(5), O(|E|³) for (3).
  For the 12 named lattices (ADR-SCHEMA-004), |E| ≤ 10, so this is trivially fast.
```

#### Level 1 (State Invariant)
Every lattice-resolved attribute has a complete lattice definition. The lattice
definition is not merely syntactically complete (has elements, comparator, bottom)
but semantically valid: the comparator defines a partial order with unique least
upper bounds for all pairs.

A user-defined lattice that fails the semilattice witness check at registration time
is rejected. The transaction installing the lattice definition does not commit.
This prevents downstream violations of INV-RESOLUTION-002 (Resolution Commutativity)
and INV-RESOLUTION-006 (Lattice Join Correctness), which depend on the algebraic
properties guaranteed by the semilattice structure.

#### Level 2 (Implementation Contract)
```rust
/// Verify that a lattice definition forms a valid join-semilattice.
/// Called during schema validation when a lattice entity is transacted.
fn verify_semilattice(elements: &[Keyword], comparator: &dyn Fn(&Keyword, &Keyword) -> bool,
                      bottom: &Keyword) -> Result<(), LatticeValidationError> {
    // (1) Reflexivity
    for x in elements {
        if !comparator(x, x) {
            return Err(LatticeValidationError::NotReflexive(x.clone()));
        }
    }
    // (2) Antisymmetry
    for x in elements {
        for y in elements {
            if x != y && comparator(x, y) && comparator(y, x) {
                return Err(LatticeValidationError::NotAntisymmetric(x.clone(), y.clone()));
            }
        }
    }
    // (3) Transitivity
    for x in elements {
        for y in elements {
            for z in elements {
                if comparator(x, y) && comparator(y, z) && !comparator(x, z) {
                    return Err(LatticeValidationError::NotTransitive(
                        x.clone(), y.clone(), z.clone()));
                }
            }
        }
    }
    // (4)+(5) Join existence and uniqueness
    for x in elements {
        for y in elements {
            let upper_bounds: Vec<_> = elements.iter()
                .filter(|u| comparator(x, u) && comparator(y, u))
                .collect();
            if upper_bounds.is_empty() {
                return Err(LatticeValidationError::NoJoin(x.clone(), y.clone()));
            }
            let least = upper_bounds.iter()
                .filter(|u| upper_bounds.iter().all(|w| comparator(u, w)))
                .collect::<Vec<_>>();
            if least.len() != 1 {
                return Err(LatticeValidationError::NonUniqueJoin(x.clone(), y.clone()));
            }
        }
    }
    // (6) Bottom validity
    for x in elements {
        if !comparator(bottom, x) {
            return Err(LatticeValidationError::InvalidBottom(bottom.clone(), x.clone()));
        }
    }
    Ok(())
}
```

**Falsification**: An attribute declared as `:lattice` resolution mode with no corresponding
lattice definition, or a lattice definition missing required properties. Additionally:
a lattice definition whose comparator violates reflexivity, antisymmetry, or transitivity;
or a lattice definition where some pair of elements has no least upper bound or has multiple
incomparable upper bounds (violating the semilattice requirement). Any such lattice that
is accepted by the schema validation without triggering an error violates this invariant.

**proptest strategy**: Generate random partial orders over small element sets (|E| ≤ 8).
For valid semilattices, verify `verify_semilattice` accepts. For partial orders that
violate any of the six properties, verify `verify_semilattice` rejects with the
correct error variant.

---

### INV-SCHEMA-008: Diamond Lattice Signal Generation

**Traces to**: ADRS AS-009, SR-010
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
For lattices with diamond structure (two incomparable top elements):
  join(a, b) where a ⊥ b = error_signal_element

Example: challenge-verdict lattice
  :confirmed ⊥ :refuted (incomparable)
  join(:confirmed, :refuted) = :contradicted (error signal)
```

#### Level 1 (State Invariant)
When concurrent assertions produce incomparable lattice values, the join operation
produces a first-class error signal (the top of the diamond), which triggers the
coordination layer's conflict detection.

**Falsification**: Two incomparable lattice values that silently merge without producing
a coordination signal.

---

### INV-SCHEMA-009: Spec Dependency Graph Completeness

**Traces to**: SEED §4 (C7: self-bootstrap), exploration/11-topology-as-compilation.md §2.2
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
For every spec element e with a prose dependency on spec element e':
  ∃ datom (e, :spec/depends-on | :spec/affects | :spec/constrains | :spec/tests, e', tx, assert)

The spec dependency graph G_spec = (V, E) where:
  V = {e | ∃ (e, :spec/type, _, _, assert) ∈ S}
  E = {(e, e', type) | ∃ (e, :spec/{type}, e', _, assert) ∈ S
       where type ∈ {depends-on, affects, constrains, tests}}

G_spec must be a connected graph (excluding uncertainty elements).
```

#### Level 1 (State Invariant)
After self-bootstrap (Phase 2), G_spec has:
  |V| = total spec elements transacted
  |E| >= |V| - 1  (at minimum, a spanning tree)

#### Level 2 (Implementation Contract)
The bootstrap EDNL file generator must extract dependency relationships from
spec prose (explicit cross-references like "Depends on INV-STORE-001") and
emit corresponding `:spec/depends-on` ref datoms.

**Falsification**: A spec element with a prose dependency ("Depends on X" or
"Traces to X" where X is another spec element) that has no corresponding
`:spec/depends-on`, `:spec/affects`, `:spec/constrains`, or `:spec/tests` ref datom.

**proptest strategy**: Generate random spec element sets with known dependency
graphs. Bootstrap them. Assert the dependency graph is recovered exactly.

---

### §2.5 ADRs

### ADR-SCHEMA-001: Schema-as-Data Over DDL

**Traces to**: SEED §4, C3, ADRS FD-008
**Stage**: 0

#### Problem
Where does the schema live?

#### Options
A) **Schema as datoms in the store** — self-describing, queryable, evolvable by transaction.
B) **Separate DDL file** — traditional approach (the Go CLI uses 39 CREATE TABLE statements).
C) **Hardcoded in source** — enums and structs in Rust source code.

#### Decision
**Option A.** The schema is datoms. Schema evolution is a transaction. Schema queries use the
same engine as data queries.

#### Formal Justification
Option A preserves C3 and C7 (self-bootstrap). The schema is the first data the system
manages — it describes itself. Options B and C create a separate truth source that can
diverge from the store.

---

### ADR-SCHEMA-002: 17 Axiomatic Attributes

**Traces to**: ADRS SR-008
**Stage**: 0

#### Problem
How does the schema bootstrap itself?

#### Options
A) **19 hardcoded meta-schema attributes** — the minimum set that can describe everything else.
B) **Empty genesis** — all attributes added post-genesis by user transactions.
C) **Full domain schema in genesis** — all 195+ attributes hardcoded.

#### Decision
**Option A.** Exactly 19 attributes are hardcoded in the engine (not defined by datoms that
reference themselves — that would be circular). Everything else is defined by datoms using
these 19. This is the only place where "code knows about schema" — all other schema is data.

#### Formal Justification
Option B has a chicken-and-egg problem: you can't define `:db/ident` as a datom before
`:db/ident` exists. Option C defeats the purpose of schema-as-data. Option A is the
minimal fixed point.

---

### ADR-SCHEMA-003: Six-Layer Architecture

**Traces to**: ADRS SR-009
**Stage**: 0

#### Problem
How should the ~195+ attributes be organized?

#### Options
A) **Six layers with dependency ordering** — each layer depends only on layers below it.
B) **Flat namespace** — all attributes at one level.
C) **Module-per-entity-type** — each entity type is an independent module.

#### Decision
**Option A.** Six layers enable incremental implementation. Stage 0 installs Layers 0–1
(meta-schema + agent/provenance). Each subsequent stage adds the next layer. The dependency
ordering ensures Layer N attributes can be fully defined using only Layer 0..N-1 entity types.

---

### ADR-SCHEMA-004: Twelve Named Lattices

**Traces to**: ADRS SR-010
**Stage**: 0–2

#### Problem
How many lattice definitions does the system need?

#### Decision
Twelve lattices, several with non-trivial diamond structure:
1. agent-lifecycle
2. confidence-level
3. adr-lifecycle
4. witness-lifecycle
5. challenge-verdict (diamond: `:confirmed`/`:refuted` → `:contradicted`)
6. thread-lifecycle
7. finding-lifecycle (diamond)
8. proposal-lifecycle (three-way incomparable → `:contested`)
9. delegation-level
10. conflict-lifecycle
11. task-lifecycle
12. numeric-max

The diamond patterns connect lattice algebra to coordination (INV-SCHEMA-008).

#### Forward Reference — Layer 4 Coordination Lattices (Stage 2–3)

The topology framework (exploration docs 00–11, ADR-SCHEMA-008) requires additional
lattices for coordination attributes. These follow the same pattern as lattices 1–12
and are registered via the standard lattice entity mechanism (INV-SCHEMA-007):

```
13. topology-lifecycle:       :proposed < :compiled < :enacted; terminals: :superseded, :rolled-back
14. channel-frequency:        :none < :on-demand < :low < :medium < :high < :continuous
15. coordination-intensity:   numeric ordering (merge overhead metric)
16. trust-level:              bounded-real [0.0, 1.0] with max join
```

These lattices are NOT installed at genesis. They are registered as schema extension
transactions when the coordination layer (Stage 2–3) is activated. The lattice
registration mechanism is verified working at Stage 0 (ADR-SCHEMA-008, Option C).

---

### ADR-SCHEMA-005: Owned Schema with Borrow API

**Traces to**: C3, INV-SCHEMA-001, ADRS SR-012
**Stage**: 0

#### Problem
What is Schema's ownership model relative to Store?

#### Options
A) **Borrow** — `Schema<'a> { store: &'a Store }`. Schema borrows Store, queries on
   demand. Pure C3 expression but lifetime-infectious in Rust.
B) **Copy** — `Schema { /* copied data */ }`. Independent of Store lifetime, but the
   copy can diverge from the store after construction.
C) **Owned internally** — Store owns a `Schema` field, derived from schema datoms on
   load. Exposed via `store.schema() -> &Schema`.

#### Decision
**Option C.** Store owns Schema internally, constructed via `Schema::from_store(datoms)`
on load and after schema-modifying transactions. The API exposes `&Schema` via borrow
(zero-cost, no allocation). This avoids lifetime infection (Option A), prevents
divergence (Option B), and maintains C3 because Schema is always derived from datoms.

#### Consequences
- `Schema` struct has no lifetime parameter — can be stored in any context
- `Schema::from_store()` is the sole constructor (enforces C3)
- Store rebuilds Schema when schema datoms change (after transact of schema attributes)
- `store.schema()` returns `&Schema` — borrow semantics, zero cost

#### Stage 3 Concurrency Analysis

At Stage 3 (multi-agent coordination), the Store is accessed concurrently via
`ArcSwap<Store>` (ADR-STORE-016). Because Schema is owned by Store (Option C), Schema
is part of each MVCC snapshot — readers always see a Schema consistent with the datoms
in that snapshot. This makes schema-datom consistency a **structural property** of the
concurrency model, not a coordination obligation.

**Option B rejection (independent Schema snapshots)**: Three consistency hazards rule
out decoupling Schema from Store's MVCC lifecycle:

1. **Stale schema + new datoms**: A reader holds an old Schema snapshot while the Store
   swaps to a new version containing newly-defined attributes. The reader encounters datoms
   whose attributes are unknown to its Schema — validation fails or silently misinterprets
   values.
2. **New schema + old datoms**: A reader obtains a new Schema snapshot but reads from an
   older Store. Attributes declared in the Schema may have no datoms yet, causing queries
   to return phantom empty results where the reader expects data.
3. **Resolution mode mismatch during merge**: If Schema is versioned independently, a merge
   operation could use one Schema's resolution modes while operating on datoms that were
   asserted under a different Schema version. The resolution mode for a given attribute
   might differ between versions (e.g., an attribute changed from LWW to multi-value),
   producing silently incorrect merge results.

Under Option C + MVCC, all three hazards are structurally impossible: the Schema and datoms
are the same snapshot. See also ADR-INTERFACE-004 (MCP server architecture) for the
`ArcSwap<Store>` initialization model.

---

### ADR-SCHEMA-006: Value Type Union

**Traces to**: SEED §4, ADRS SQ-008
**Stage**: 0

#### Problem
What is the complete set of value types that the datom store supports? The `Value` type
in the `[e, a, v, tx, op]` tuple must cover all data that agents, specifications, and
the system itself need to represent — without being so open-ended that schema validation
becomes meaningless.

#### Options
A) **Minimal type set** — String, Long, Boolean, Ref. All complex data serialized to
   String. Simple but loses type safety: a String containing JSON is not distinguishable
   from a String containing prose without parsing the content.
B) **Datomic-compatible type union** — 14 types matching Datomic's value types: String,
   Keyword, Boolean, Long, Double, Instant, UUID, Ref, Bytes, URI, BigInt, BigDec,
   Tuple, Json. Proven at scale with well-understood storage and indexing characteristics.
C) **Extensible type system** — User-defined value types registered at runtime. Maximum
   flexibility but complicates schema validation, storage, and merge semantics.

#### Decision
**Option B.** The complete Value type union is:

```
Value = String              — UTF-8 text (spec statements, descriptions, rationale)
      | Keyword             — namespaced identifier (:db/ident, :task/status)
      | Boolean             — true | false
      | Long                — 64-bit signed integer (counts, indices, stage numbers)
      | Double              — 64-bit IEEE 754 (confidence levels, scores, weights)
      | Instant             — UTC timestamp with nanosecond precision (wall-clock times)
      | UUID                — 128-bit universally unique identifier
      | Ref EntityId        — reference to another entity (relationships, dependencies)
      | Bytes               — raw byte array (hashes, binary content)
      | URI                 — RFC 3986 URI (external references, documentation links)
      | BigInt              — arbitrary precision integer (future-proofing for large counts)
      | BigDec              — arbitrary precision decimal (financial, scientific)
      | Tuple [Value]       — ordered heterogeneous sequence (composite keys, coordinates)
      | Json String         — JSON-encoded string (complex payloads, comparison scores)
```

Additionally, two domain-specific sum types used in the protocol:

```
Level = 0 | 1 | 2 | 3
  — refinement level (Level 0 algebraic law through Level 3 test evidence)

SignalType = Confusion | Conflict | UncertaintySpike | ResolutionProposal
           | DelegationRequest | GoalDrift | BranchReady | DeliberationTurn
  — divergence signal classification (INV-SIGNAL-001)
```

#### Formal Justification
The 14 value types are the minimum set needed to represent all protocol-level data without
resorting to String-encoded structured data:
- **Keyword** is essential for schema (attribute idents), entity types, and enum values
  without encoding them as arbitrary strings
- **Ref** enables the entity reference graph that Datalog traverses (joins on entity IDs)
- **Instant** enables time-travel queries (as_of, since) with nanosecond precision
- **Tuple** enables composite values (e.g., `[confidence, evidence_count]` for uncertainty
  tensors) without requiring separate entities for every compound value
- **Json** is the escape hatch for genuinely complex payloads (branch comparison scores,
  diagnostic reports) that would require dozens of attributes to flatten

The Level and SignalType sum types are protocol enums stored as Keywords but given
distinct Rust types for compile-time exhaustiveness checking.

#### Consequences
- `Value` is a Rust enum with 14 variants — pattern matching covers all cases
- Schema validation checks that a datom's value matches its attribute's declared type
- Each value type has a canonical serialization to EDNL (ADR-STORE-007)
- Indexing (AVET) operates on the serialized form — ordering is type-aware
- Merge deduplication compares values by content, not by reference (C2)

#### Falsification
This decision is wrong if: a protocol-level datum cannot be represented by any of the 14
value types without lossy transformation (e.g., a geometric shape, a graph structure, or
a probabilistic distribution that loses semantic information when encoded as Json or Bytes).

---

### ADR-SCHEMA-007: Typed Spec Element Relationships

**Traces to**: SEED §4 (C7), exploration/11-topology-as-compilation.md §2.2
**Stage**: 0

#### Problem
Spec elements have relationships to each other. Currently only `:spec/depends-on`
exists. Should relationships be typed (separate attributes per relationship type)
or untyped (single `:spec/depends-on` for all relationships)?

#### Options
A) **Single untyped attribute** (`:spec/depends-on` for everything)
   - Pro: Simpler schema, fewer attributes
   - Con: Cannot distinguish sequencing constraints from impact relationships
     in Datalog queries without additional annotation
   - Con: CALM classification (monotonic vs non-monotonic edges) requires
     relationship type information

B) **Four typed relationship attributes** (`:spec/depends-on`, `:spec/affects`,
   `:spec/constrains`, `:spec/tests`)
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
  - `:spec/depends-on`: may be monotonic or non-monotonic
  - `:spec/affects`: always non-monotonic (changes interpretation)
  - `:spec/constrains`: always monotonic (narrows solution space)
  - `:spec/tests`: always monotonic (verification is additive)

The type information is needed at query time for compilation (exploration doc 11,
Pass 1). Encoding it in the attribute name avoids an extra join. Four attributes
is a small addition to the existing Layer 2 spec attributes.

#### Consequences
- Three new Layer 2 attributes: `:spec/affects`, `:spec/constrains`, `:spec/tests`
- `:spec/depends-on` already exists (no change needed)
- Bootstrap EDNL generator must emit typed relationships
- Compilation front-end queries become simple attribute pattern matches
- CALM classification for the compilation middle-end gets relationship type for free

---

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
- Coordination lattice definitions documented as forward references
- Stage 2–3 implementation simply transacts lattice entities

---

### §2.6 Negative Cases

### NEG-SCHEMA-001: No External Schema

**Traces to**: C3
**Verification**: `V:TYPE`

**Safety property**: `□ ¬(∃ schema definition outside the datom store)`
No YAML config, no CREATE TABLE, no schema.json.

**Formal statement**: The only source of truth for "what attributes exist" is
`store.query([:find ?a :where [?a :db/ident ?name]])`.

**Rust type-level enforcement**: `Schema` is owned by `Store` internally (ADR-SCHEMA-005,
Option C). `Schema::from_store(datoms)` is the sole constructor — no `Schema::from_file()`,
`Schema::from_yaml()`, or `Schema::new()`. The type has no lifetime parameter; schema is
derived from store datoms on load and after schema-modifying transactions.

---

### NEG-SCHEMA-002: No Schema Deletion

**Traces to**: C1, C3
**Verification**: `V:TYPE`, `V:PROP`

**Safety property**: `□ ¬(∃ operation that removes an attribute from the schema)`
Attributes can be deprecated (via new datoms marking them deprecated), but never deleted.

**Formal statement**: `∀ t, t' where t < t': attributes(S(t)) ⊆ attributes(S(t'))`

---

### NEG-SCHEMA-003: No Circular Layer Dependencies

**Traces to**: ADRS SR-009
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ attribute in Layer N referencing entity type from Layer M where M > N)`

**proptest strategy**: For each attribute, verify all referenced entity types are from
the same or lower layer.

---

### NEG-BOOTSTRAP-001: Content-Only Bootstrap Produces Flat Store

**Traces to**: SEED §4 (C7), exploration/11-topology-as-compilation.md §2.2
**Verification**: `V:PROP`

**Statement**: A self-bootstrap that transacts spec element content (id, type,
statement, falsification) but NOT dependency relationships (`:spec/depends-on`,
`:spec/affects`, `:spec/constrains`, `:spec/tests`) produces a store where:
  - Spec elements exist as isolated entities
  - No dependency graph is queryable
  - The compilation front-end (exploration doc 11 §2.2) has no input data
  - Contradiction detection cannot trace dependency chains
  - Impact analysis queries return empty results

**Safety property**: `□(bootstrap_complete → dependency_edges > 0)`

**Violation condition**: After bootstrap, the query
  `[:find (count ?e) :where [?e :spec/depends-on _]]`
returns 0 while spec elements with prose dependencies exist.

---

