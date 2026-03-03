> **Namespace**: SCHEMA | **Wave**: 1 (Foundation) | **Stage**: 0
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §2. SCHEMA — Schema-as-Data

### §2.0 Overview

Schema in Braid is not a separate DDL or configuration file — it is data in the store
itself. The schema is a set of datoms that describe what attributes exist, what types
they expect, and how they behave during conflict resolution. Schema evolution is a
transaction, not a migration.

**Traces to**: SEED.md §4, C3
**ADRS.md sources**: FD-002, FD-008, SR-008, SR-009, SR-010, PO-012

---

### §2.1 Level 0: Algebraic Specification

#### Meta-Schema Recursion

```
The schema S_schema ⊂ S is a subset of datoms in the store.
Schema datoms describe attributes; attributes describe datoms.

Self-reference: the meta-schema attributes describe themselves.
  e.g., :db/valueType has valueType :db.type/keyword
        :db/cardinality has cardinality :db.cardinality/one

Formally: Let A₀ = {a₁, ..., a₁₇} be the 17 axiomatic meta-schema attributes.
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

For each of the 17 axiomatic attributes aᵢ:
  (aᵢ, :db/ident,        <keyword>,     tx₀, Assert)
  (aᵢ, :db/valueType,    <type>,        tx₀, Assert)
  (aᵢ, :db/cardinality,  <cardinality>, tx₀, Assert)
  (aᵢ, :db/doc,          <description>, tx₀, Assert)
  ... (additional properties as needed)

tx₀ has no causal predecessors.
tx₀ is the root of the causal graph.
```

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
  :tx/agent           — Ref        :one    — agent who transacted
  :tx/provenance      — Keyword    :one    — provenance type
```

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
/// The 17 axiomatic attributes — hardcoded in the engine.
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

pub struct Schema {
    store: Store,  // schema IS the store (filtered to schema datoms)
}

impl Schema {
    /// Look up attribute definition.
    pub fn attribute(&self, ident: &Keyword) -> Option<AttributeDef>;

    /// Validate a value against an attribute's type.
    pub fn validate_value(&self, attr: &Attribute, value: &Value) -> Result<(), SchemaError>;

    /// Add a new attribute (returns a Transaction).
    pub fn define_attribute(&self, spec: AttributeSpec) -> Transaction<Building>;

    /// All known attributes.
    pub fn attributes(&self) -> impl Iterator<Item = AttributeDef>;
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
    pub fn attribute(&self, ident: &Keyword) -> Option<&AttributeDef> { /* ... */ }
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
∀ aᵢ ∈ A₀ (the 17 axiomatic attributes):
  ∃ datoms in GENESIS() defining aᵢ
  AND those datoms use only attributes from A₀
  (the meta-schema is self-contained)
```

#### Level 1 (State Invariant)
The genesis transaction contains exactly the 17 axiomatic attribute definitions.
Each attribute is fully specified (ident, valueType, cardinality at minimum).
No non-meta-schema datoms exist in genesis.

#### Level 2 (Implementation Contract)
```rust
fn genesis() -> Store {
    let mut store = Store::empty();
    let tx = Transaction::<Building>::new(SYSTEM_AGENT)
        .with_provenance(ProvenanceType::Observed);
    // Assert exactly 17 attributes...
    // Assert each attribute's ident, valueType, cardinality, doc
    let tx = tx.commit_genesis();  // special: bypasses schema validation (bootstrap)
    store.apply_genesis(tx);
    assert_eq!(store.schema().attributes().count(), 17);
    store
}
```

**Falsification**: A genesis store where `schema.attributes().count() != 17`, or where
any axiomatic attribute lacks a complete definition.

---

### INV-SCHEMA-003: Schema Monotonicity

**Traces to**: SEED §4, C1, C3
**Verification**: `V:PROP`
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
**Verification**: `V:PROP`, `V:KANI`
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
itself one of the 17 axiomatic attributes.

**Falsification**: Any axiomatic attribute whose definition requires an attribute outside A₀.

---

### INV-SCHEMA-006: Six-Layer Schema Architecture

**Traces to**: ADRS SR-009
**Verification**: `V:PROP`
**Stage**: 0–4 (progressive)

#### Level 0 (Algebraic Law)
```
Schema is organized into 6 layers:
  Layer 0: Meta-schema (17 axiomatic attributes)        — Stage 0
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
**Verification**: `V:PROP`
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
```

#### Level 1 (State Invariant)
Every lattice-resolved attribute has a complete lattice definition.

**Falsification**: An attribute declared as `:lattice` resolution mode with no corresponding
lattice definition, or a lattice definition missing required properties.

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
A) **17 hardcoded meta-schema attributes** — the minimum set that can describe everything else.
B) **Empty genesis** — all attributes added post-genesis by user transactions.
C) **Full domain schema in genesis** — all 195+ attributes hardcoded.

#### Decision
**Option A.** Exactly 17 attributes are hardcoded in the engine (not defined by datoms that
reference themselves — that would be circular). Everything else is defined by datoms using
these 17. This is the only place where "code knows about schema" — all other schema is data.

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

---

### ADR-SCHEMA-005: Owned Schema with Borrow API

**Traces to**: C3, INV-SCHEMA-001
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

---

### §2.6 Negative Cases

### NEG-SCHEMA-001: No External Schema

**Traces to**: C3
**Verification**: `V:TYPE`

**Safety property**: `□ ¬(∃ schema definition outside the datom store)`
No YAML config, no CREATE TABLE, no schema.json.

**Formal statement**: The only source of truth for "what attributes exist" is
`store.query([:find ?a :where [?a :db/ident ?name]])`.

**Rust type-level enforcement**: `Schema` wraps a `&Store` reference. No `Schema::from_file()`.

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

