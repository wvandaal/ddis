//! Schema-as-data: attribute definitions stored as datoms (C3).
//!
//! The schema is derived from the store, not stored separately. Schema
//! evolution is a transaction, not a migration. The 17 axiomatic meta-schema
//! attributes describe themselves (INV-SCHEMA-005).
//!
//! # Invariants
//!
//! - **INV-SCHEMA-001**: Schema is a subset of the store, not separate DDL.
//! - **INV-SCHEMA-002**: Genesis contains exactly 17 axiomatic attributes.
//! - **INV-SCHEMA-003**: Schema can only grow (monotonicity).
//! - **INV-SCHEMA-004**: Every transacted datom is validated against schema.
//! - **INV-SCHEMA-005**: Axiomatic attributes describe themselves using A₀.

use std::collections::{BTreeSet, HashMap};

use crate::datom::{Attribute, Datom, EntityId, Op, TxId, Value};
use crate::error::StoreError;

// ---------------------------------------------------------------------------
// Schema types
// ---------------------------------------------------------------------------

/// Value type constraint for an attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueType {
    /// UTF-8 string.
    String,
    /// Keyword (`:ns/name`).
    Keyword,
    /// Boolean.
    Boolean,
    /// 64-bit signed integer.
    Long,
    /// 64-bit float.
    Double,
    /// Milliseconds since epoch.
    Instant,
    /// 128-bit UUID.
    Uuid,
    /// Reference to another entity.
    Ref,
    /// Opaque bytes.
    Bytes,
}

impl ValueType {
    /// Parse from a keyword string (e.g., `:db.type/string`).
    pub fn from_keyword(kw: &str) -> Option<Self> {
        match kw {
            ":db.type/string" => Some(ValueType::String),
            ":db.type/keyword" => Some(ValueType::Keyword),
            ":db.type/boolean" => Some(ValueType::Boolean),
            ":db.type/long" => Some(ValueType::Long),
            ":db.type/double" => Some(ValueType::Double),
            ":db.type/instant" => Some(ValueType::Instant),
            ":db.type/uuid" => Some(ValueType::Uuid),
            ":db.type/ref" => Some(ValueType::Ref),
            ":db.type/bytes" => Some(ValueType::Bytes),
            _ => None,
        }
    }

    /// Convert to keyword string.
    pub fn as_keyword(&self) -> &'static str {
        match self {
            ValueType::String => ":db.type/string",
            ValueType::Keyword => ":db.type/keyword",
            ValueType::Boolean => ":db.type/boolean",
            ValueType::Long => ":db.type/long",
            ValueType::Double => ":db.type/double",
            ValueType::Instant => ":db.type/instant",
            ValueType::Uuid => ":db.type/uuid",
            ValueType::Ref => ":db.type/ref",
            ValueType::Bytes => ":db.type/bytes",
        }
    }

    /// Check if a value matches this type constraint.
    pub fn matches(&self, value: &Value) -> bool {
        matches!(
            (self, value),
            (ValueType::String, Value::String(_))
                | (ValueType::Keyword, Value::Keyword(_))
                | (ValueType::Boolean, Value::Boolean(_))
                | (ValueType::Long, Value::Long(_))
                | (ValueType::Double, Value::Double(_))
                | (ValueType::Instant, Value::Instant(_))
                | (ValueType::Uuid, Value::Uuid(_))
                | (ValueType::Ref, Value::Ref(_))
                | (ValueType::Bytes, Value::Bytes(_))
        )
    }
}

/// Attribute cardinality.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cardinality {
    /// Exactly one value per entity per attribute.
    One,
    /// Multiple values per entity per attribute.
    Many,
}

impl Cardinality {
    /// Parse from a keyword string.
    pub fn from_keyword(kw: &str) -> Option<Self> {
        match kw {
            ":db.cardinality/one" => Some(Cardinality::One),
            ":db.cardinality/many" => Some(Cardinality::Many),
            _ => None,
        }
    }

    /// Convert to keyword string.
    pub fn as_keyword(&self) -> &'static str {
        match self {
            Cardinality::One => ":db.cardinality/one",
            Cardinality::Many => ":db.cardinality/many",
        }
    }
}

/// Uniqueness constraint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Uniqueness {
    /// Unique identity — upsert semantics.
    Identity,
    /// Unique value — reject duplicate values.
    Value,
}

impl Uniqueness {
    /// Parse from a keyword string.
    pub fn from_keyword(kw: &str) -> Option<Self> {
        match kw {
            ":db.unique/identity" => Some(Uniqueness::Identity),
            ":db.unique/value" => Some(Uniqueness::Value),
            _ => None,
        }
    }
}

/// Conflict resolution mode for an attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResolutionMode {
    /// Last-writer-wins with HLC + BLAKE3 tiebreaker.
    Lww,
    /// User-defined lattice join.
    Lattice,
    /// Multi-value (set union — keep all).
    Multi,
}

impl ResolutionMode {
    /// Parse from a keyword string.
    pub fn from_keyword(kw: &str) -> Option<Self> {
        match kw {
            ":resolution/lww" => Some(ResolutionMode::Lww),
            ":resolution/lattice" => Some(ResolutionMode::Lattice),
            ":resolution/multi" => Some(ResolutionMode::Multi),
            _ => None,
        }
    }

    /// Convert to keyword string.
    pub fn as_keyword(&self) -> &'static str {
        match self {
            ResolutionMode::Lww => ":resolution/lww",
            ResolutionMode::Lattice => ":resolution/lattice",
            ResolutionMode::Multi => ":resolution/multi",
        }
    }
}

// ---------------------------------------------------------------------------
// AttributeSpec + AttributeDef
// ---------------------------------------------------------------------------

/// Specification for creating a new attribute.
#[derive(Clone, Debug)]
pub struct AttributeSpec {
    /// The attribute keyword (e.g., `:db/ident`).
    pub ident: Attribute,
    /// Value type constraint.
    pub value_type: ValueType,
    /// Cardinality (one or many).
    pub cardinality: Cardinality,
    /// Documentation string.
    pub doc: String,
    /// Conflict resolution mode (default: LWW).
    pub resolution_mode: ResolutionMode,
    /// Uniqueness constraint (if any).
    pub unique: Option<Uniqueness>,
    /// Component lifecycle flag.
    pub is_component: bool,
}

/// A fully resolved attribute definition in the schema.
#[derive(Clone, Debug)]
pub struct AttributeDef {
    /// Entity ID of this attribute.
    pub entity: EntityId,
    /// The attribute keyword.
    pub ident: Attribute,
    /// Value type constraint.
    pub value_type: ValueType,
    /// Cardinality.
    pub cardinality: Cardinality,
    /// Conflict resolution mode.
    pub resolution_mode: ResolutionMode,
    /// Documentation.
    pub doc: String,
    /// Uniqueness constraint.
    pub unique: Option<Uniqueness>,
    /// Component lifecycle.
    pub is_component: bool,
}

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

/// The schema — derived from store datoms, never stored separately.
///
/// Reconstructed via `Schema::from_datoms()` on store load and after
/// schema-modifying transactions.
#[derive(Clone, Debug)]
pub struct Schema {
    attrs: HashMap<Attribute, AttributeDef>,
}

impl Schema {
    /// Reconstruct schema from a datom set.
    ///
    /// Scans for entities with `:db/ident` → extracts attribute definitions.
    pub fn from_datoms(datoms: &BTreeSet<Datom>) -> Self {
        let mut attr_entities: HashMap<EntityId, HashMap<String, Value>> = HashMap::new();

        // Collect all datoms for entities that have :db/ident
        for datom in datoms {
            if datom.op == Op::Assert && datom.attribute.namespace() == "db" {
                attr_entities
                    .entry(datom.entity)
                    .or_default()
                    .insert(datom.attribute.as_str().to_string(), datom.value.clone());
            }
        }

        let mut attrs = HashMap::new();

        for (entity_id, fields) in &attr_entities {
            // Only process entities that have :db/ident
            let ident = match fields.get(":db/ident") {
                Some(Value::Keyword(kw)) => match Attribute::new(kw) {
                    Ok(a) => a,
                    Err(_) => continue,
                },
                _ => continue,
            };

            let value_type = fields
                .get(":db/valueType")
                .and_then(|v| match v {
                    Value::Keyword(kw) => ValueType::from_keyword(kw),
                    _ => None,
                })
                .unwrap_or(ValueType::String);

            let cardinality = fields
                .get(":db/cardinality")
                .and_then(|v| match v {
                    Value::Keyword(kw) => Cardinality::from_keyword(kw),
                    _ => None,
                })
                .unwrap_or(Cardinality::One);

            let resolution_mode = fields
                .get(":db/resolutionMode")
                .and_then(|v| match v {
                    Value::Keyword(kw) => ResolutionMode::from_keyword(kw),
                    _ => None,
                })
                .unwrap_or(ResolutionMode::Lww);

            let doc = fields
                .get(":db/doc")
                .and_then(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();

            let unique = fields.get(":db/unique").and_then(|v| match v {
                Value::Keyword(kw) => Uniqueness::from_keyword(kw),
                _ => None,
            });

            let is_component = fields
                .get(":db/isComponent")
                .and_then(|v| match v {
                    Value::Boolean(b) => Some(*b),
                    _ => None,
                })
                .unwrap_or(false);

            attrs.insert(
                ident.clone(),
                AttributeDef {
                    entity: *entity_id,
                    ident,
                    value_type,
                    cardinality,
                    resolution_mode,
                    doc,
                    unique,
                    is_component,
                },
            );
        }

        Schema { attrs }
    }

    /// Look up an attribute definition by keyword.
    pub fn attribute(&self, ident: &Attribute) -> Option<&AttributeDef> {
        self.attrs.get(ident)
    }

    /// All known attributes.
    pub fn attributes(&self) -> impl Iterator<Item = (&Attribute, &AttributeDef)> {
        self.attrs.iter()
    }

    /// Number of attributes in the schema.
    pub fn len(&self) -> usize {
        self.attrs.len()
    }

    /// Whether the schema has no attributes.
    pub fn is_empty(&self) -> bool {
        self.attrs.is_empty()
    }

    /// Resolution mode for an attribute (defaults to LWW if unknown).
    pub fn resolution_mode(&self, attr: &Attribute) -> ResolutionMode {
        self.attrs
            .get(attr)
            .map(|def| def.resolution_mode)
            .unwrap_or(ResolutionMode::Lww)
    }

    /// Validate a datom against the schema (INV-SCHEMA-004).
    pub fn validate_datom(&self, datom: &Datom) -> Result<(), StoreError> {
        match self.attrs.get(&datom.attribute) {
            None => Err(StoreError::UnknownAttribute(datom.attribute.clone())),
            Some(def) => {
                if !def.value_type.matches(&datom.value) {
                    Err(StoreError::SchemaViolation {
                        attr: datom.attribute.clone(),
                        expected: def.value_type.as_keyword().to_string(),
                        got: datom.value.type_name().to_string(),
                    })
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Validate schema evolution: new schema must be a monotone extension (INV-SCHEMA-003).
    ///
    /// Rules:
    /// - No attribute may be removed
    /// - Value type may not change
    /// - Cardinality may not narrow (Many → One is forbidden; One → Many is allowed)
    ///
    /// Returns a list of violations (empty = valid evolution).
    pub fn validate_evolution(&self, new_schema: &Schema) -> Vec<SchemaEvolutionError> {
        let mut errors = Vec::new();

        for (attr, old_def) in &self.attrs {
            match new_schema.attrs.get(attr) {
                None => {
                    errors.push(SchemaEvolutionError::AttributeRemoved(attr.clone()));
                }
                Some(new_def) => {
                    if new_def.value_type != old_def.value_type {
                        errors.push(SchemaEvolutionError::ValueTypeChanged {
                            attr: attr.clone(),
                            old: old_def.value_type,
                            new: new_def.value_type,
                        });
                    }
                    if old_def.cardinality == Cardinality::Many
                        && new_def.cardinality == Cardinality::One
                    {
                        errors.push(SchemaEvolutionError::CardinalityNarrowed(attr.clone()));
                    }
                }
            }
        }

        errors
    }

    /// Check if this schema is a superset of another (for merge compatibility).
    pub fn is_superset_of(&self, other: &Schema) -> bool {
        for attr in other.attrs.keys() {
            if !self.attrs.contains_key(attr) {
                return false;
            }
        }
        true
    }
}

/// Error in schema evolution (INV-SCHEMA-003 violation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaEvolutionError {
    /// An existing attribute was removed (forbidden).
    AttributeRemoved(Attribute),
    /// An attribute's value type was changed (forbidden).
    ValueTypeChanged {
        /// The attribute.
        attr: Attribute,
        /// The old type.
        old: ValueType,
        /// The new type.
        new: ValueType,
    },
    /// Cardinality narrowed from Many to One (forbidden).
    CardinalityNarrowed(Attribute),
}

// ---------------------------------------------------------------------------
// Genesis
// ---------------------------------------------------------------------------

/// Produce the genesis datom set — the 17 axiomatic meta-schema attributes.
///
/// Deterministic: same output every call (INV-STORE-008, INV-SCHEMA-002).
pub fn genesis_datoms(genesis_tx: TxId) -> Vec<Datom> {
    let specs = axiomatic_attributes();
    let mut datoms = Vec::new();

    for spec in &specs {
        let entity = EntityId::from_ident(spec.ident.as_str());

        // :db/ident
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(spec.ident.as_str().to_string()),
            genesis_tx,
            Op::Assert,
        ));

        // :db/valueType
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/valueType"),
            Value::Keyword(spec.value_type.as_keyword().to_string()),
            genesis_tx,
            Op::Assert,
        ));

        // :db/cardinality
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/cardinality"),
            Value::Keyword(spec.cardinality.as_keyword().to_string()),
            genesis_tx,
            Op::Assert,
        ));

        // :db/doc
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String(spec.doc.clone()),
            genesis_tx,
            Op::Assert,
        ));

        // :db/resolutionMode
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/resolutionMode"),
            Value::Keyword(spec.resolution_mode.as_keyword().to_string()),
            genesis_tx,
            Op::Assert,
        ));
    }

    datoms
}

/// The 17 axiomatic meta-schema attributes (INV-SCHEMA-002).
fn axiomatic_attributes() -> Vec<AttributeSpec> {
    vec![
        // Layer 0 — Meta-Schema (9 attributes)
        attr(
            ":db/ident",
            ValueType::Keyword,
            Cardinality::One,
            "Attribute's keyword name",
        ),
        attr(
            ":db/valueType",
            ValueType::Keyword,
            Cardinality::One,
            "Value type constraint",
        ),
        attr(
            ":db/cardinality",
            ValueType::Keyword,
            Cardinality::One,
            ":one or :many",
        ),
        attr(
            ":db/doc",
            ValueType::String,
            Cardinality::One,
            "Documentation string",
        ),
        attr(
            ":db/unique",
            ValueType::Keyword,
            Cardinality::One,
            ":identity or :value",
        ),
        attr(
            ":db/isComponent",
            ValueType::Boolean,
            Cardinality::One,
            "Component lifecycle",
        ),
        attr(
            ":db/resolutionMode",
            ValueType::Keyword,
            Cardinality::One,
            ":lww, :lattice, :multi",
        ),
        attr(
            ":db/latticeOrder",
            ValueType::Ref,
            Cardinality::One,
            "Ref to lattice definition",
        ),
        attr(
            ":db/lwwClock",
            ValueType::Keyword,
            Cardinality::One,
            ":hlc, :wall, :agent-rank",
        ),
        // Lattice definitions (5 attributes)
        attr(
            ":lattice/ident",
            ValueType::Keyword,
            Cardinality::One,
            "Lattice name",
        ),
        attr(
            ":lattice/elements",
            ValueType::Keyword,
            Cardinality::Many,
            "Set of lattice elements",
        ),
        attr(
            ":lattice/comparator",
            ValueType::String,
            Cardinality::One,
            "Ordering function name",
        ),
        attr(
            ":lattice/bottom",
            ValueType::Keyword,
            Cardinality::One,
            "Bottom element",
        ),
        attr(
            ":lattice/top",
            ValueType::Keyword,
            Cardinality::One,
            "Top element",
        ),
        // Transaction metadata (3 attributes)
        attr(
            ":tx/time",
            ValueType::Instant,
            Cardinality::One,
            "Wall-clock time",
        ),
        attr(
            ":tx/agent",
            ValueType::Ref,
            Cardinality::One,
            "Agent who transacted",
        ),
        attr(
            ":tx/provenance",
            ValueType::Keyword,
            Cardinality::One,
            "Provenance type",
        ),
    ]
}

fn attr(ident: &str, value_type: ValueType, cardinality: Cardinality, doc: &str) -> AttributeSpec {
    AttributeSpec {
        ident: Attribute::from_keyword(ident),
        value_type,
        cardinality,
        doc: doc.to_string(),
        resolution_mode: ResolutionMode::Lww,
        unique: None,
        is_component: false,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::AgentId;

    #[test]
    fn genesis_produces_17_attributes() {
        let specs = axiomatic_attributes();
        assert_eq!(
            specs.len(),
            17,
            "INV-SCHEMA-002: exactly 17 axiomatic attributes"
        );
    }

    #[test]
    fn genesis_datoms_are_deterministic() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(0, 0, agent);
        let d1 = genesis_datoms(tx);
        let d2 = genesis_datoms(tx);
        assert_eq!(d1, d2, "INV-STORE-008: genesis is deterministic");
    }

    #[test]
    fn schema_from_genesis_has_17_attributes() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(0, 0, agent);
        let datoms: BTreeSet<Datom> = genesis_datoms(tx).into_iter().collect();
        let schema = Schema::from_datoms(&datoms);
        assert_eq!(schema.len(), 17);
    }

    #[test]
    fn schema_validates_correct_datom() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(0, 0, agent);
        let datoms: BTreeSet<Datom> = genesis_datoms(tx).into_iter().collect();
        let schema = Schema::from_datoms(&datoms);

        let valid = Datom::new(
            EntityId::from_ident(":test/attr"),
            Attribute::from_keyword(":db/doc"),
            Value::String("hello".into()),
            tx,
            Op::Assert,
        );
        assert!(schema.validate_datom(&valid).is_ok());
    }

    #[test]
    fn schema_rejects_type_mismatch() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(0, 0, agent);
        let datoms: BTreeSet<Datom> = genesis_datoms(tx).into_iter().collect();
        let schema = Schema::from_datoms(&datoms);

        let bad = Datom::new(
            EntityId::from_ident(":test/attr"),
            Attribute::from_keyword(":db/doc"), // expects String
            Value::Long(42),                    // got Long
            tx,
            Op::Assert,
        );
        assert!(schema.validate_datom(&bad).is_err());
    }

    #[test]
    fn schema_rejects_unknown_attribute() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(0, 0, agent);
        let datoms: BTreeSet<Datom> = genesis_datoms(tx).into_iter().collect();
        let schema = Schema::from_datoms(&datoms);

        let unknown = Datom::new(
            EntityId::from_ident(":test/attr"),
            Attribute::from_keyword(":nonexistent/attr"),
            Value::String("x".into()),
            tx,
            Op::Assert,
        );
        assert!(schema.validate_datom(&unknown).is_err());
    }

    #[test]
    fn value_type_matches() {
        assert!(ValueType::String.matches(&Value::String("hi".into())));
        assert!(!ValueType::String.matches(&Value::Long(1)));
        assert!(ValueType::Ref.matches(&Value::Ref(EntityId::from_content(b"x"))));
    }

    #[test]
    fn evolution_reflexive() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(0, 0, agent);
        let datoms: BTreeSet<Datom> = genesis_datoms(tx).into_iter().collect();
        let schema = Schema::from_datoms(&datoms);
        let errors = schema.validate_evolution(&schema);
        assert!(errors.is_empty(), "evolution(S, S) must be valid");
    }

    #[test]
    fn evolution_detects_attribute_removal() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(0, 0, agent);
        let datoms: BTreeSet<Datom> = genesis_datoms(tx).into_iter().collect();
        let full_schema = Schema::from_datoms(&datoms);
        let empty_schema = Schema {
            attrs: HashMap::new(),
        };
        let errors = full_schema.validate_evolution(&empty_schema);
        assert_eq!(
            errors.len(),
            17,
            "all 17 attributes should be flagged as removed"
        );
        assert!(errors
            .iter()
            .all(|e| matches!(e, SchemaEvolutionError::AttributeRemoved(_))));
    }

    #[test]
    fn evolution_allows_new_attributes() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(0, 0, agent);
        let datoms: BTreeSet<Datom> = genesis_datoms(tx).into_iter().collect();
        let old_schema = Schema::from_datoms(&datoms);

        // Add a new attribute to the datom set
        let mut new_datoms = datoms.clone();
        let new_entity = EntityId::from_ident(":custom/attr");
        new_datoms.insert(Datom::new(
            new_entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":custom/attr".into()),
            tx,
            Op::Assert,
        ));
        new_datoms.insert(Datom::new(
            new_entity,
            Attribute::from_keyword(":db/valueType"),
            Value::Keyword(":db.type/string".into()),
            tx,
            Op::Assert,
        ));
        new_datoms.insert(Datom::new(
            new_entity,
            Attribute::from_keyword(":db/cardinality"),
            Value::Keyword(":db.cardinality/one".into()),
            tx,
            Op::Assert,
        ));
        new_datoms.insert(Datom::new(
            new_entity,
            Attribute::from_keyword(":db/doc"),
            Value::String("Custom attribute".into()),
            tx,
            Op::Assert,
        ));
        new_datoms.insert(Datom::new(
            new_entity,
            Attribute::from_keyword(":db/resolutionMode"),
            Value::Keyword(":resolution/lww".into()),
            tx,
            Op::Assert,
        ));
        let new_schema = Schema::from_datoms(&new_datoms);

        assert_eq!(new_schema.len(), 18);
        let errors = old_schema.validate_evolution(&new_schema);
        assert!(errors.is_empty(), "adding attributes is valid evolution");
    }

    #[test]
    fn is_superset_of() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(0, 0, agent);
        let datoms: BTreeSet<Datom> = genesis_datoms(tx).into_iter().collect();
        let schema = Schema::from_datoms(&datoms);
        let empty = Schema {
            attrs: HashMap::new(),
        };

        assert!(schema.is_superset_of(&empty));
        assert!(schema.is_superset_of(&schema));
        assert!(!empty.is_superset_of(&schema));
    }
}
