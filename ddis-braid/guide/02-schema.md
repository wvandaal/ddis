# §2. SCHEMA — Build Plan

> **Spec reference**: [spec/02-schema.md](../spec/02-schema.md) — read FIRST
> **Stage 0 elements**: INV-SCHEMA-001–007 (7 INV; 006 progressive 0–4; 008 Stage 2), ADR-SCHEMA-001–004, NEG-SCHEMA-001–003
> **Dependencies**: STORE (§1 complete)
> **Cognitive mode**: Ontological — category theory, bootstrap, self-description

---

## §2.1 Module Structure

```
braid-kernel/src/
└── schema.rs     ← Schema, genesis, attribute registry, validation, layers
```

### Public API Surface

```rust
pub struct Schema { /* opaque */ }

impl Schema {
    /// Reconstruct schema from store datoms (the only constructor — enforces C3).
    /// Called internally by Store on load and after schema-modifying transactions.
    pub fn from_store(datoms: &BTreeSet<Datom>) -> Schema;

    /// Validate a datom against schema (attribute existence + value type match).
    pub fn validate_datom(&self, datom: &Datom) -> Result<(), SchemaValidationError>;

    /// Produce datoms for a new attribute definition (caller wraps in Transaction).
    pub fn new_attribute(&self, spec: AttributeSpec) -> Vec<Datom>;

    /// Look up attribute definition by attribute keyword.
    pub fn attribute(&self, ident: &Attribute) -> Option<&AttributeDef>;

    /// All known attributes.
    pub fn attributes(&self) -> impl Iterator<Item = (&Attribute, &AttributeDef)>;

    /// Resolution mode for an attribute.
    pub fn resolution_mode(&self, attr: &Attribute) -> ResolutionMode;
}

impl Store {
    /// Borrow the schema — zero cost, derived from store datoms on load.
    pub fn schema(&self) -> &Schema { &self.schema }
}

pub struct AttributeSpec {
    pub ident:           Attribute,
    pub value_type:      ValueType,
    pub cardinality:     Cardinality,
    pub doc:             String,
    pub resolution_mode: ResolutionMode,
    pub unique:          Option<Uniqueness>,
    pub is_component:    bool,
}

pub struct AttributeDef {
    pub entity:          EntityId,
    pub ident:           Attribute,
    pub value_type:      ValueType,
    pub cardinality:     Cardinality,
    pub resolution_mode: ResolutionMode,
    pub doc:             String,
    pub unique:          Option<Uniqueness>,
    pub is_component:    bool,
}

pub enum ValueType {
    String, Keyword, Boolean, Long, Double, Instant, Uuid, Ref, Bytes,
}

pub enum Cardinality { One, Many }
pub enum Uniqueness { Identity, Value }
```

### Genesis Constants

```rust
pub mod genesis {
    /// The 17 axiomatic meta-schema attributes.
    /// These are compile-time constants — the EntityId for each is derived from
    /// the keyword via blake3::hash(keyword.as_bytes()).
    pub const AXIOMATIC_ATTRIBUTES: [AttributeSpec; 17] = [
        // Layer 0 — Meta-Schema (9 attributes)
        attr(":db/ident",           ValueType::Keyword, One, "Attribute's keyword name"),
        attr(":db/valueType",       ValueType::Keyword, One, "Value type constraint"),
        attr(":db/cardinality",     ValueType::Keyword, One, ":one or :many"),
        attr(":db/doc",             ValueType::String,  One, "Documentation string"),
        attr(":db/unique",          ValueType::Keyword, One, ":identity or :value"),
        attr(":db/isComponent",     ValueType::Boolean, One, "Component lifecycle"),
        attr(":db/resolutionMode",  ValueType::Keyword, One, ":lww, :lattice, :multi"),
        attr(":db/latticeOrder",    ValueType::Ref,     One, "Ref to lattice definition"),
        attr(":db/lwwClock",        ValueType::Keyword, One, ":hlc, :wall, :agent-rank"),
        // Lattice definition (5 attributes)
        attr(":lattice/ident",      ValueType::Keyword, One, "Lattice name"),
        attr(":lattice/elements",   ValueType::Keyword, Many, "Set of lattice elements"),
        attr(":lattice/comparator", ValueType::String,  One, "Ordering function name"),
        attr(":lattice/bottom",     ValueType::Keyword, One, "Bottom element"),
        attr(":lattice/top",        ValueType::Keyword, One, "Top element"),
        // Transaction metadata (3 attributes)
        attr(":tx/time",            ValueType::Instant, One, "Wall-clock time"),
        attr(":tx/agent",           ValueType::Ref,     One, "Agent who transacted"),
        attr(":tx/provenance",      ValueType::Keyword, One, "Provenance type"),
    ];

    /// Produce the genesis datom set. Deterministic — same output every call.
    pub fn genesis_datoms() -> Vec<Datom> { /* ... */ }
}
```

---

## §2.2 Three-Box Decomposition

### Schema

**Black box** (contract):
- INV-SCHEMA-001: Schema-as-Data — schema is a subset of the store, not a separate DDL (C3).
- INV-SCHEMA-002: Genesis Completeness — genesis tx contains exactly 17 axiomatic attributes, self-contained.
- INV-SCHEMA-003: Schema Monotonicity — schema can only grow; attributes are never removed.
- INV-SCHEMA-004: Schema Validation on Transact — no undefined attribute or mistyped value enters the store.
- INV-SCHEMA-005: Meta-Schema Self-Description — axiomatic attributes describe themselves using only A₀.
- INV-SCHEMA-006: Six-Layer Schema Architecture — 6 layers with dependency ordering (Stage 0–4 progressive).
- INV-SCHEMA-007: Lattice Definition Completeness — every lattice-resolved attribute has a complete lattice definition.
- INV-SCHEMA-008: Diamond Lattice Signal Generation — incomparable lattice values produce error signal (Stage 2).

**State box** (internal design):
- `attrs: HashMap<Attribute, AttributeDef>` — in-memory attribute registry.
- Built from store datoms: scan for entities with `:db/ident` attribute.
- Schema is reconstructed on store load (not persisted separately).
- Schema grows only via `transact` of new schema datoms.

**Clear box** (implementation):
- `from_store(datoms)`: scan datoms for `(?, :db/ident, ?)` → extract all attribute-defining datoms →
  build `AttributeDef` per attribute → populate HashMap. Also extracts lattice definition entities.
- `validate_datom`: lookup `datom.attribute` → check value type matches `attr.value_type` →
  check cardinality constraint (`:one` means only one assertion per entity per attribute in LIVE view).
- `new_attribute`: generate EntityId from keyword (`EntityId::from_ident(keyword)`) →
  produce datoms for `:db/ident`, `:db/valueType`, `:db/cardinality`, etc.
- Genesis: iterate `AXIOMATIC_ATTRIBUTES` → call `new_attribute` for each → collect datoms.
  Use `TxId { wall_time: 0, logical: 0, agent: SYSTEM_AGENT }` for genesis tx.

### Six-Layer Schema Architecture (INV-SCHEMA-006)

**Black box** (contract):
- INV-SCHEMA-006: Schema organized into 6 layers with dependency ordering:
  Layer 0 (Meta-schema, 17 axiomatic) → Layer 1 (Agent & Provenance) → Layer 2 (DDIS Core) →
  Layer 3 (Discovery) → Layer 4 (Coordination) → Layer 5 (Workflow).
  Each layer depends only on layers below it. Stages 0–4 introduce layers progressively.

**State box** (internal design):
- Layer membership is determined by the attribute's `:db/doc` or a `:schema/layer` attribute.
- Stage 0 implements Layers 0–1 (meta-schema + agent/provenance). Layer 2 starts in Stage 0–1.
- Schema validation enforces layer ordering: Layer N attributes may only reference
  types defined in Layers 0..N.

**Clear box** (implementation):
```rust
pub enum SchemaLayer {
    MetaSchema,       // Layer 0: 17 axiomatic attributes
    AgentProvenance,  // Layer 1: agent, provenance types
    DdisCore,         // Layer 2: spec types, harvest, seed
    Discovery,        // Layer 3: search, exploration
    Coordination,     // Layer 4: deliberation, sync
    Workflow,         // Layer 5: task, workspace
}

impl Schema {
    /// Validate that attribute belongs to its declared layer
    /// and only references types from lower layers.
    pub fn validate_layer_ordering(&self) -> Vec<LayerViolation>;
}
```

### Lattice Definition Completeness (INV-SCHEMA-007)

**Black box** (contract):
- INV-SCHEMA-007: Every attribute with `:db/resolutionMode = :lattice` has a complete
  lattice definition: `:db/latticeOrder` → lattice entity with `:lattice/ident`,
  `:lattice/elements` (non-empty), `:lattice/comparator`, `:lattice/bottom`.

**State box** (internal design):
- Lattice definitions are entities in the store (C3).
- Genesis includes no lattice-resolved attributes — lattice definitions are added via schema evolution.
- Validation checks completeness at `transact` time: if an attribute sets `:lattice` mode,
  the referenced lattice entity must already exist with all required properties.

**Clear box** (implementation):
```rust
impl Schema {
    /// Validate that all lattice-resolved attributes have complete definitions.
    /// Lattice definitions are extracted from datoms during from_store() and stored
    /// internally — no Store reference needed (ADR-SCHEMA-005, Option C).
    pub fn validate_lattice_completeness(&self) -> Vec<LatticeDefError> {
        let mut errors = Vec::new();
        for (attr, def) in self.attributes() {
            if let ResolutionMode::Lattice { lattice_id } = def.resolution_mode {
                match self.lattice_def(lattice_id) {
                    None => { errors.push(MissingLatticeDef(attr.clone())); }
                    Some(lattice) => {
                        if lattice.ident.is_none() { errors.push(MissingIdent(attr.clone())); }
                        if lattice.elements.is_empty() { errors.push(EmptyElements(attr.clone())); }
                        if lattice.comparator.is_none() { errors.push(MissingComparator(attr.clone())); }
                        if lattice.bottom.is_none() { errors.push(MissingBottom(attr.clone())); }
                    }
                }
            }
        }
        errors
    }

    /// Look up a lattice definition by entity id (extracted during from_store).
    fn lattice_def(&self, id: EntityId) -> Option<&LatticeDef>;
}
```

**proptest strategy**: Generate schema with random lattice-resolved attributes.
For each, verify the lattice definition is complete (all 4 required properties present).

---

## §2.3 Type-Level Encoding

| INV | Compile-Time Guarantee | Mechanism |
|-----|----------------------|-----------|
| INV-SCHEMA-001 | Cannot construct schema outside store | `Schema::from_store` is the only constructor |
| INV-SCHEMA-003 | No `DROP` or `ALTER DELETE` on attributes | No `remove_attribute` method exists |

---

## §2.4 LLM-Facing Outputs

### Agent-Mode Output — Schema Evolution

```
[SCHEMA] Added attribute :task/status (Keyword, :one, resolution: lattice).
Schema: 18 attributes (17 axiomatic + 1 user-defined).
---
↳ Which schema layer does this attribute belong to? (See: INV-SCHEMA-006)
```

### Error Messages

- **Unknown attribute**: `Schema error: attribute {attr} not in schema — add via schema transaction — See: INV-SCHEMA-004`
- **Type mismatch**: `Schema error: {attr} expects {expected}, got {actual} — See: INV-SCHEMA-004`
- **Duplicate ident**: `Schema warning: attribute {attr} already exists — updating properties via append — See: INV-SCHEMA-003`

---

## §2.5 Verification

### Key Properties

```rust
proptest! {
    // INV-SCHEMA-001: Schema-as-Data (schema is subset of store)
    fn inv_schema_001() {
        let store = Store::genesis();
        let schema = store.schema();
        // Schema is derived from store datoms, not external source
        assert!(schema.attributes().count() > 0);
    }

    // INV-SCHEMA-002: Genesis Completeness (exactly 17 axiomatic attributes)
    fn inv_schema_002() {
        let store = Store::genesis();
        let schema = store.schema();
        assert_eq!(schema.attributes().count(), 17);
    }

    // INV-SCHEMA-005: Meta-Schema Self-Description
    fn inv_schema_005() {
        let store = Store::genesis();
        let schema = store.schema();
        for (attr, _def) in schema.attributes() {
            // Every axiomatic attribute can be looked up via itself
            assert!(schema.attribute(attr).is_some());
        }
    }

    // INV-SCHEMA-006: Six-Layer Architecture — layer ordering respected
    fn inv_schema_006(store in arb_schema(5)) {
        let schema = store.schema();
        let violations = schema.validate_layer_ordering();
        prop_assert!(violations.is_empty(),
            "Layer ordering violated: {:?}", violations);
    }

    // INV-SCHEMA-007: Lattice Definition Completeness
    fn inv_schema_007(store in arb_schema(3)) {
        let schema = store.schema();
        let errors = schema.validate_lattice_completeness();
        prop_assert!(errors.is_empty(),
            "Incomplete lattice definitions: {:?}", errors);
    }
}
```

### Kani Harnesses

INV-SCHEMA-001, 002, 004 have V:KANI tags. INV-SCHEMA-005 has V:PROP tag.

---

## §2.6 Implementation Checklist

- [ ] `Schema`, `AttributeSpec`, `AttributeDef`, `ValueType`, `Cardinality` types defined
- [ ] 17 axiomatic attributes as compile-time constants
- [ ] `genesis_datoms()` produces deterministic datom set
- [ ] `Schema::from_store()` reconstructs schema from datoms
- [ ] `Schema::validate_datom()` checks attribute existence and type
- [ ] `Schema::new_attribute()` produces correct datoms for new attributes
- [ ] Self-description test passes (schema describes itself)
- [ ] Integration with STORE: genesis → schema extraction → validation round-trip
- [ ] All proptest properties pass
- [ ] Kani harnesses pass

---
