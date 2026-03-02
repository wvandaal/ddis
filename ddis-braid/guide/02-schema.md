# §2. SCHEMA — Build Plan

> **Spec reference**: [spec/02-schema.md](../spec/02-schema.md) — read FIRST
> **Stage 0 elements**: INV-SCHEMA-001–008 (all 8), ADR-SCHEMA-001–004, NEG-SCHEMA-001–003
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
    /// Build the schema from the datom set (extract schema-defining datoms).
    pub fn from_store(store: &Store) -> Self;

    /// Validate an attribute exists and value type matches.
    pub fn validate_datom(&self, datom: &Datom) -> Result<(), SchemaValidationError>;

    /// Register a new attribute (produces datoms for the transaction).
    pub fn new_attribute(&self, spec: AttributeSpec) -> Vec<Datom>;

    /// Lookup attribute definition.
    pub fn get(&self, attr: &Attribute) -> Option<&AttributeDef>;

    /// All known attributes.
    pub fn attributes(&self) -> impl Iterator<Item = (&Attribute, &AttributeDef)>;

    /// Resolution mode for an attribute.
    pub fn resolution_mode(&self, attr: &Attribute) -> ResolutionMode;
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
- INV-SCHEMA-001: genesis installs exactly 17 axiomatic attributes, is deterministic.
- INV-SCHEMA-002: meta-schema is self-describing (every axiomatic attribute is described using axiomatic attributes).
- INV-SCHEMA-003: schema is data in the store, not external DDL (C3).
- INV-SCHEMA-004: schema evolution is monotonic — new attributes only, no removal.
- INV-SCHEMA-005: every datom's attribute exists in the schema at transact time.
- INV-SCHEMA-006: every datom's value type matches the schema-declared type.
- INV-SCHEMA-007: resolution mode is declared per-attribute.
- INV-SCHEMA-008: the schema can describe itself (self-bootstrap test).

**State box** (internal design):
- `attrs: HashMap<Attribute, AttributeDef>` — in-memory attribute registry.
- Built from store datoms: scan for entities with `:db/ident` attribute.
- Schema is reconstructed on store load (not persisted separately).
- Schema grows only via `transact` of new schema datoms.

**Clear box** (implementation):
- `from_store`: query store for `(?, :db/ident, ?)` → extract all attribute-defining datoms →
  build `AttributeDef` per attribute → populate HashMap.
- `validate_datom`: lookup `datom.attribute` → check value type matches `attr.value_type` →
  check cardinality constraint (`:one` means only one assertion per entity per attribute in LIVE view).
- `new_attribute`: generate EntityId from keyword (`EntityId::from_ident(keyword)`) →
  produce datoms for `:db/ident`, `:db/valueType`, `:db/cardinality`, etc.
- Genesis: iterate `AXIOMATIC_ATTRIBUTES` → call `new_attribute` for each → collect datoms.
  Use `TxId { wall_time: 0, logical: 0, agent: SYSTEM_AGENT }` for genesis tx.

---

## §2.3 Type-Level Encoding

| INV | Compile-Time Guarantee | Mechanism |
|-----|----------------------|-----------|
| INV-SCHEMA-003 | Cannot construct schema outside store | `Schema::from_store` is the only constructor |
| INV-SCHEMA-004 | No `DROP` or `ALTER DELETE` on attributes | No `remove_attribute` method exists |

---

## §2.4 LLM-Facing Outputs

### Agent-Mode Output — Schema Evolution

```
[SCHEMA] Added attribute :task/status (Keyword, :one, resolution: lattice).
Schema: 18 attributes (17 axiomatic + 1 user-defined).
---
↳ Which schema layer does this attribute belong to? (See: INV-SCHEMA-001)
```

### Error Messages

- **Unknown attribute**: `Schema error: attribute {attr} not in schema — add via schema transaction — See: INV-SCHEMA-005`
- **Type mismatch**: `Schema error: {attr} expects {expected}, got {actual} — See: INV-SCHEMA-006`
- **Duplicate ident**: `Schema warning: attribute {attr} already exists — updating properties via append — See: INV-SCHEMA-004`

---

## §2.5 Verification

### Key Properties

```rust
proptest! {
    // INV-SCHEMA-001: Genesis produces 17 attributes
    fn inv_schema_001() {
        let store = Store::genesis();
        let schema = Schema::from_store(&store);
        assert_eq!(schema.attributes().count(), 17);
    }

    // INV-SCHEMA-002: Self-description
    fn inv_schema_002() {
        let store = Store::genesis();
        let schema = Schema::from_store(&store);
        for (attr, def) in schema.attributes() {
            // Every axiomatic attribute can be looked up via itself
            assert!(schema.get(attr).is_some());
        }
    }

    // INV-SCHEMA-008: Genesis is deterministic
    fn inv_schema_008() {
        let s1 = Store::genesis();
        let s2 = Store::genesis();
        assert_eq!(s1.datoms().collect::<Vec<_>>(), s2.datoms().collect::<Vec<_>>());
    }
}
```

### Kani Harnesses

INV-SCHEMA-001, 002, 004 have V:KANI tags.

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
