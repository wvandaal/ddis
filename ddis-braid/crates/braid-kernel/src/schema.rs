//! Schema-as-data: attribute definitions stored as datoms (C3).
//!
//! The schema is derived from the store, not stored separately. Schema
//! evolution is a transaction, not a migration. The 18 axiomatic meta-schema
//! attributes describe themselves (INV-SCHEMA-005).
//!
//! # Six-Layer Schema Architecture (INV-SCHEMA-006)
//!
//! - **Layer 0** (Meta-Schema): 18 axiomatic attributes — `:db/*`, `:lattice/*`, `:tx/*`.
//! - **Layer 1** (Trilateral): 24 domain attributes — `:intent/*`, `:spec/*`, `:impl/*`.
//! - **Layer 2** (Specification Elements): 36 rich-metadata attributes —
//!   `:element/*`, `:inv/*`, `:adr/*`, `:neg/*`, `:dep/*`, `:session/*`,
//!   `:methodology/*`, `:coherence/*`.
//! - **Layer 3** (Discovery/Exploration): 20 attributes — `:exploration/*`, `:promotion/*`.
//! - **Layers 4–5**: Coordination, Workflow (future stages).
//!
//! Each layer depends only on layers below it. Layer 0 is installed at genesis.
//! Layers 1–2 are installed via schema-evolution transactions.
//!
//! # Invariants
//!
//! - **INV-SCHEMA-001**: Schema is a subset of the store, not separate DDL.
//! - **INV-SCHEMA-002**: Genesis contains exactly 18 axiomatic attributes.
//! - **INV-SCHEMA-003**: Schema can only grow (monotonicity).
//! - **INV-SCHEMA-004**: Every transacted datom is validated against schema.
//! - **INV-SCHEMA-005**: Axiomatic attributes describe themselves using A₀.
//! - **INV-SCHEMA-006**: Six-layer architecture with dependency ordering.

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

/// Produce the genesis datom set — the 18 axiomatic meta-schema attributes.
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

/// The 18 axiomatic meta-schema attributes (INV-SCHEMA-002).
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
        // Transaction metadata (4 attributes)
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
        attr(
            ":tx/rationale",
            ValueType::String,
            Cardinality::One,
            "Human-readable rationale for the transaction",
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

/// Build an `AttributeSpec` with multi-value resolution mode.
fn attr_multi(
    ident: &str,
    value_type: ValueType,
    cardinality: Cardinality,
    doc: &str,
) -> AttributeSpec {
    AttributeSpec {
        ident: Attribute::from_keyword(ident),
        value_type,
        cardinality,
        doc: doc.to_string(),
        resolution_mode: ResolutionMode::Multi,
        unique: None,
        is_component: false,
    }
}

/// Build an `AttributeSpec` with a uniqueness constraint.
fn attr_unique(
    ident: &str,
    value_type: ValueType,
    cardinality: Cardinality,
    doc: &str,
    unique: Uniqueness,
) -> AttributeSpec {
    AttributeSpec {
        ident: Attribute::from_keyword(ident),
        value_type,
        cardinality,
        doc: doc.to_string(),
        resolution_mode: ResolutionMode::Lww,
        unique: Some(unique),
        is_component: false,
    }
}

// ---------------------------------------------------------------------------
// Layer 1 — Trilateral Domain Attributes (INV-SCHEMA-006)
// ---------------------------------------------------------------------------

/// The 24 Layer 1 (Trilateral) attributes: Intent (7) + Spec (11) + Impl (6).
///
/// These are the domain attributes used by the trilateral coherence model
/// (Intent <-> Specification <-> Implementation). They depend only on Layer 0
/// value types (String, Keyword, Long, Boolean, Ref).
///
/// Layer 1 is installed as a schema-evolution transaction after genesis.
pub fn layer_1_attributes() -> Vec<AttributeSpec> {
    vec![
        // --- Intent layer (7 attributes) ---
        attr(
            ":intent/decision",
            ValueType::String,
            Cardinality::One,
            "Design decision captured from intent",
        ),
        attr(
            ":intent/rationale",
            ValueType::String,
            Cardinality::One,
            "Rationale behind a design decision",
        ),
        attr(
            ":intent/source",
            ValueType::String,
            Cardinality::One,
            "Source reference for intent (transcript, conversation)",
        ),
        attr(
            ":intent/goal",
            ValueType::String,
            Cardinality::One,
            "Goal or objective driving this entity",
        ),
        attr(
            ":intent/constraint",
            ValueType::String,
            Cardinality::One,
            "Constraint or hard requirement on the design",
        ),
        attr(
            ":intent/preference",
            ValueType::String,
            Cardinality::One,
            "Soft preference for design direction",
        ),
        attr(
            ":intent/noted",
            ValueType::String,
            Cardinality::One,
            "Observation noted during intent capture",
        ),
        // --- Specification layer (11 attributes) ---
        attr(
            ":spec/id",
            ValueType::String,
            Cardinality::One,
            "Specification element ID (e.g., INV-STORE-001)",
        ),
        attr(
            ":spec/element-type",
            ValueType::Keyword,
            Cardinality::One,
            "Type of spec element (:spec.element/invariant, :spec.element/adr, etc.)",
        ),
        attr(
            ":spec/namespace",
            ValueType::Keyword,
            Cardinality::One,
            "Namespace of spec element (:spec.ns/store, :spec.ns/schema, etc.)",
        ),
        attr(
            ":spec/source-file",
            ValueType::String,
            Cardinality::One,
            "Source markdown file for the spec element",
        ),
        attr(
            ":spec/stage",
            ValueType::Long,
            Cardinality::One,
            "Implementation stage (0, 1, 2, ...) when this element becomes relevant",
        ),
        attr(
            ":spec/statement",
            ValueType::String,
            Cardinality::One,
            "Formal statement text of the invariant or spec element",
        ),
        attr(
            ":spec/falsification",
            ValueType::String,
            Cardinality::One,
            "Falsification condition: how to violate this invariant",
        ),
        attr(
            ":spec/traces-to",
            ValueType::String,
            Cardinality::One,
            "SEED.md section reference that motivates this element",
        ),
        attr(
            ":spec/verification",
            ValueType::String,
            Cardinality::One,
            "Verification method (V:PROP, V:KANI, V:TYPE, V:MODEL)",
        ),
        attr(
            ":spec/witnessed",
            ValueType::Boolean,
            Cardinality::One,
            "Whether this element has test evidence",
        ),
        attr(
            ":spec/challenged",
            ValueType::Boolean,
            Cardinality::One,
            "Whether this element has been formally challenged",
        ),
        // --- Implementation layer (6 attributes) ---
        attr(
            ":impl/signature",
            ValueType::String,
            Cardinality::One,
            "Function or type signature implementing a spec element",
        ),
        attr(
            ":impl/implements",
            ValueType::Ref,
            Cardinality::One,
            "Ref to the spec element this code implements",
        ),
        attr(
            ":impl/file",
            ValueType::String,
            Cardinality::One,
            "Source file path containing the implementation",
        ),
        attr(
            ":impl/module",
            ValueType::String,
            Cardinality::One,
            "Module or crate containing the implementation",
        ),
        attr(
            ":impl/test-result",
            ValueType::Keyword,
            Cardinality::One,
            "Test result status (:pass, :fail, :skip)",
        ),
        attr(
            ":impl/coverage",
            ValueType::Double,
            Cardinality::One,
            "Code coverage ratio (0.0-1.0) for this implementation",
        ),
    ]
}

/// Produce datoms for all Layer 1 attributes.
///
/// These should be transacted as a schema-evolution transaction after genesis.
/// Uses the same datom structure as `genesis_datoms` — each attribute becomes an
/// entity with `:db/ident`, `:db/valueType`, `:db/cardinality`, `:db/doc`,
/// and `:db/resolutionMode` datoms.
pub fn layer_1_datoms(tx: TxId) -> Vec<Datom> {
    schema_datoms_from_specs(&layer_1_attributes(), tx)
}

// ---------------------------------------------------------------------------
// Layer 2 — Specification Element Attributes (INV-SCHEMA-006)
// ---------------------------------------------------------------------------

/// Number of Layer 2 specification element attributes.
pub const LAYER_2_COUNT: usize = 36;

/// The 36 Layer 2 (Specification Element) attributes.
///
/// These provide rich metadata for first-class specification elements (INV, ADR,
/// NEG) stored as datoms. They depend only on Layer 0 value types.
///
/// Organized into 8 groups:
/// - Core Element (8): identity and common metadata
/// - Invariant-Specific (4): formal verification properties
/// - ADR-Specific (5): decision record structure
/// - Negative Case-Specific (3): violation/detection/mitigation
/// - Cross-Reference (3): dependency edges between elements
/// - Session/Provenance (4): session lifecycle metadata
/// - Methodology (4): methodology score tracking
/// - Coherence (5): divergence metrics
pub fn layer_2_attributes() -> Vec<AttributeSpec> {
    vec![
        // =================================================================
        // Core Element Attributes (8) — All spec elements
        // =================================================================
        attr_unique(
            ":element/id",
            ValueType::String,
            Cardinality::One,
            "Specification element ID (e.g., INV-STORE-001, ADR-SEED-002)",
            Uniqueness::Identity,
        ),
        attr(
            ":element/type",
            ValueType::Keyword,
            Cardinality::One,
            "Element type: :element.type/invariant, :element.type/adr, :element.type/negative-case, :element.type/uncertainty, :element.type/section, :element.type/goal",
        ),
        attr(
            ":element/title",
            ValueType::String,
            Cardinality::One,
            "Human-readable title of the specification element",
        ),
        attr(
            ":element/body",
            ValueType::String,
            Cardinality::One,
            "Full body text of the specification element",
        ),
        attr(
            ":element/namespace",
            ValueType::Keyword,
            Cardinality::One,
            "Namespace: :element.ns/store, :element.ns/query, :element.ns/harvest, etc.",
        ),
        attr_multi(
            ":element/traces-to",
            ValueType::String,
            Cardinality::Many,
            "SEED.md section(s) that motivate this element",
        ),
        attr(
            ":element/status",
            ValueType::Keyword,
            Cardinality::One,
            "Lifecycle status: :element.status/active, :element.status/superseded, :element.status/proposed, :element.status/deprecated",
        ),
        attr(
            ":element/confidence",
            ValueType::Double,
            Cardinality::One,
            "Uncertainty confidence level (0.0-1.0); 1.0 = fully certain",
        ),
        // =================================================================
        // Invariant-Specific Attributes (4)
        // =================================================================
        attr(
            ":inv/statement",
            ValueType::String,
            Cardinality::One,
            "Formal invariant statement text",
        ),
        attr(
            ":inv/falsification",
            ValueType::String,
            Cardinality::One,
            "How to violate this invariant (falsification condition)",
        ),
        attr(
            ":inv/verification",
            ValueType::String,
            Cardinality::One,
            "How to verify this invariant holds (test strategy)",
        ),
        attr(
            ":inv/property-type",
            ValueType::Keyword,
            Cardinality::One,
            "Property classification: :inv.prop/safety, :inv.prop/liveness, :inv.prop/monotonicity, :inv.prop/convergence",
        ),
        // =================================================================
        // ADR-Specific Attributes (5)
        // =================================================================
        attr(
            ":adr/problem",
            ValueType::String,
            Cardinality::One,
            "Problem statement that motivated this decision",
        ),
        attr(
            ":adr/decision",
            ValueType::String,
            Cardinality::One,
            "The decision that was made",
        ),
        attr_multi(
            ":adr/alternatives",
            ValueType::String,
            Cardinality::Many,
            "Alternatives that were considered and rejected",
        ),
        attr(
            ":adr/consequences",
            ValueType::String,
            Cardinality::One,
            "Consequences and implications of the decision",
        ),
        attr(
            ":adr/superseded-by",
            ValueType::String,
            Cardinality::One,
            "ID of the ADR that supersedes this one (if superseded)",
        ),
        // =================================================================
        // Negative Case-Specific Attributes (3)
        // =================================================================
        attr(
            ":neg/violation",
            ValueType::String,
            Cardinality::One,
            "Description of what a violation looks like",
        ),
        attr(
            ":neg/detection",
            ValueType::String,
            Cardinality::One,
            "How to detect this violation",
        ),
        attr(
            ":neg/mitigation",
            ValueType::String,
            Cardinality::One,
            "How to prevent or fix the violation",
        ),
        // =================================================================
        // Cross-Reference Attributes (3) — dependency edges
        // =================================================================
        attr(
            ":dep/from",
            ValueType::Ref,
            Cardinality::One,
            "Source entity of a dependency edge",
        ),
        attr(
            ":dep/to",
            ValueType::Ref,
            Cardinality::One,
            "Target entity of a dependency edge",
        ),
        attr(
            ":dep/type",
            ValueType::Keyword,
            Cardinality::One,
            "Dependency type: :dep.type/requires, :dep.type/refines, :dep.type/contradicts, :dep.type/supersedes, :dep.type/traces-to, :dep.type/references",
        ),
        // =================================================================
        // Session/Provenance Attributes (4)
        // =================================================================
        attr(
            ":session/agent",
            ValueType::Ref,
            Cardinality::One,
            "Agent that created or owns this session",
        ),
        attr(
            ":session/task",
            ValueType::String,
            Cardinality::One,
            "Task description for this session",
        ),
        attr(
            ":session/start-tx",
            ValueType::Ref,
            Cardinality::One,
            "Transaction ID marking session start",
        ),
        attr(
            ":session/harvest-quality",
            ValueType::Double,
            Cardinality::One,
            "Quality score of the session harvest (0.0-1.0)",
        ),
        // =================================================================
        // Methodology Attributes (4)
        // =================================================================
        attr(
            ":methodology/score",
            ValueType::Double,
            Cardinality::One,
            "Methodology adherence score M(t) (0.0-1.0)",
        ),
        attr(
            ":methodology/trend",
            ValueType::Keyword,
            Cardinality::One,
            "Methodology trend: :methodology.trend/up, :methodology.trend/down, :methodology.trend/stable",
        ),
        attr(
            ":methodology/harvest-count",
            ValueType::Long,
            Cardinality::One,
            "Number of harvests completed in this session",
        ),
        attr(
            ":methodology/turn-count",
            ValueType::Long,
            Cardinality::One,
            "Number of agent turns in this session",
        ),
        // =================================================================
        // Coherence Attributes (5) — divergence metrics
        // =================================================================
        attr(
            ":coherence/phi",
            ValueType::Double,
            Cardinality::One,
            "Divergence measure Phi across ISP triangle",
        ),
        attr(
            ":coherence/quadrant",
            ValueType::Keyword,
            Cardinality::One,
            "Coherence quadrant classification",
        ),
        attr(
            ":coherence/d-is",
            ValueType::Double,
            Cardinality::One,
            "Intent-Specification divergence component",
        ),
        attr(
            ":coherence/d-sp",
            ValueType::Double,
            Cardinality::One,
            "Specification-Implementation divergence component",
        ),
        attr(
            ":coherence/beta-1",
            ValueType::Long,
            Cardinality::One,
            "First Betti number (structural cycle count in dependency graph)",
        ),
    ]
}

/// Produce datoms for all Layer 2 attributes.
///
/// These should be transacted as a schema-evolution transaction after Layer 1.
/// Depends only on Layer 0 value types (INV-SCHEMA-006 layer ordering).
pub fn layer_2_datoms(tx: TxId) -> Vec<Datom> {
    schema_datoms_from_specs(&layer_2_attributes(), tx)
}

/// Produce all domain schema datoms (Layer 1 + Layer 2).
///
/// Convenience function that returns the full set of domain-level schema datoms.
/// Should be transacted after genesis to register all domain attributes.
pub fn domain_schema_datoms(tx: TxId) -> Vec<Datom> {
    let mut datoms = layer_1_datoms(tx);
    datoms.extend(layer_2_datoms(tx));
    datoms
}

// ---------------------------------------------------------------------------
// Layer 3 — Discovery/Exploration Attributes (INV-SCHEMA-006)
// ---------------------------------------------------------------------------

/// Number of Layer 3 exploration/discovery attributes.
pub const LAYER_3_COUNT: usize = 20;

/// The 20 Layer 3 (Discovery/Exploration) attributes.
///
/// These capture the lifecycle of exploratory knowledge — from initial
/// discovery through promotion to formal specification elements. They enable
/// the store-first specification pipeline where exploration entities gain
/// `:spec/*` attributes via `braid promote` rather than being re-entered
/// from markdown.
///
/// Organized into 3 groups:
/// - Exploration Identity (8): source, category, confidence, maturity
/// - Promotion Lifecycle (7): promotion status, target element, verification
/// - Exploration Cross-Reference (5): links between exploration entities
///
/// Depends only on Layer 0 value types (INV-SCHEMA-006 layer ordering).
pub fn layer_3_attributes() -> Vec<AttributeSpec> {
    vec![
        // =================================================================
        // Exploration Identity Attributes (8) — where knowledge came from
        // =================================================================
        attr_unique(
            ":exploration/id",
            ValueType::String,
            Cardinality::One,
            "Exploration entity ID (e.g., EXPL-TOPO-001, EXPL-GEOM-002)",
            Uniqueness::Identity,
        ),
        attr(
            ":exploration/source",
            ValueType::String,
            Cardinality::One,
            "Source document path or session ID where this knowledge originated",
        ),
        attr(
            ":exploration/category",
            ValueType::Keyword,
            Cardinality::One,
            "Knowledge category: :exploration.cat/theorem, :exploration.cat/conjecture, :exploration.cat/definition, :exploration.cat/algorithm, :exploration.cat/design-decision, :exploration.cat/open-question",
        ),
        attr(
            ":exploration/confidence",
            ValueType::Double,
            Cardinality::One,
            "Epistemic confidence in this exploration entity (0.0-1.0)",
        ),
        attr(
            ":exploration/maturity",
            ValueType::Keyword,
            Cardinality::One,
            "Maturity level: :exploration.maturity/sketch, :exploration.maturity/draft, :exploration.maturity/reviewed, :exploration.maturity/proven",
        ),
        attr(
            ":exploration/body",
            ValueType::String,
            Cardinality::One,
            "Full text content of the exploration entity",
        ),
        attr(
            ":exploration/title",
            ValueType::String,
            Cardinality::One,
            "Human-readable title of the exploration entity",
        ),
        attr_multi(
            ":exploration/tags",
            ValueType::Keyword,
            Cardinality::Many,
            "Taxonomy tags for discovery and filtering",
        ),
        // =================================================================
        // Promotion Lifecycle Attributes (7) — store-first pipeline
        // =================================================================
        attr(
            ":promotion/status",
            ValueType::Keyword,
            Cardinality::One,
            "Promotion status: :promotion.status/unpromoted, :promotion.status/candidate, :promotion.status/promoted, :promotion.status/rejected",
        ),
        attr(
            ":promotion/target-element",
            ValueType::String,
            Cardinality::One,
            "Target spec element ID after promotion (e.g., INV-TOPOLOGY-001)",
        ),
        attr(
            ":promotion/target-namespace",
            ValueType::Keyword,
            Cardinality::One,
            "Target spec namespace: :element.ns/topology, :element.ns/coherence, etc.",
        ),
        attr(
            ":promotion/target-type",
            ValueType::Keyword,
            Cardinality::One,
            "Target element type: :element.type/invariant, :element.type/adr, :element.type/negative-case",
        ),
        attr(
            ":promotion/promoted-tx",
            ValueType::Ref,
            Cardinality::One,
            "Transaction ID of the promotion event",
        ),
        attr(
            ":promotion/phi-before",
            ValueType::Double,
            Cardinality::One,
            "Divergence Phi on exploration-spec boundary before promotion",
        ),
        attr(
            ":promotion/phi-after",
            ValueType::Double,
            Cardinality::One,
            "Divergence Phi on exploration-spec boundary after promotion (target: 0.0)",
        ),
        // =================================================================
        // Exploration Cross-Reference Attributes (5)
        // =================================================================
        attr(
            ":exploration/depends-on",
            ValueType::Ref,
            Cardinality::One,
            "Reference to another exploration entity this one depends on",
        ),
        attr(
            ":exploration/refines",
            ValueType::Ref,
            Cardinality::One,
            "Reference to an exploration entity this one refines or supersedes",
        ),
        attr(
            ":exploration/related-spec",
            ValueType::Ref,
            Cardinality::One,
            "Reference to a spec element this exploration entity relates to",
        ),
        attr(
            ":exploration/source-session",
            ValueType::Ref,
            Cardinality::One,
            "Reference to the session entity where this was discovered",
        ),
        attr(
            ":exploration/evidence",
            ValueType::String,
            Cardinality::One,
            "Evidence supporting this exploration entity (proof sketch, test results, etc.)",
        ),
    ]
}

/// Produce datoms for all Layer 3 attributes.
///
/// These should be transacted as a schema-evolution transaction after Layer 2.
/// Depends only on Layer 0 value types (INV-SCHEMA-006 layer ordering).
pub fn layer_3_datoms(tx: TxId) -> Vec<Datom> {
    schema_datoms_from_specs(&layer_3_attributes(), tx)
}

/// Produce all schema datoms through Layer 3 (Layers 1 + 2 + 3).
///
/// Full domain schema including exploration/discovery attributes.
pub fn full_schema_datoms(tx: TxId) -> Vec<Datom> {
    let mut datoms = domain_schema_datoms(tx);
    datoms.extend(layer_3_datoms(tx));
    datoms
}

/// Convert a list of `AttributeSpec`s into schema-defining datoms.
///
/// Each attribute becomes an entity with 5 datoms:
/// `:db/ident`, `:db/valueType`, `:db/cardinality`, `:db/doc`, `:db/resolutionMode`.
fn schema_datoms_from_specs(specs: &[AttributeSpec], tx: TxId) -> Vec<Datom> {
    let mut datoms = Vec::new();

    for spec in specs {
        let entity = EntityId::from_ident(spec.ident.as_str());

        // :db/ident
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(spec.ident.as_str().to_string()),
            tx,
            Op::Assert,
        ));

        // :db/valueType
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/valueType"),
            Value::Keyword(spec.value_type.as_keyword().to_string()),
            tx,
            Op::Assert,
        ));

        // :db/cardinality
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/cardinality"),
            Value::Keyword(spec.cardinality.as_keyword().to_string()),
            tx,
            Op::Assert,
        ));

        // :db/doc
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String(spec.doc.clone()),
            tx,
            Op::Assert,
        ));

        // :db/resolutionMode
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/resolutionMode"),
            Value::Keyword(spec.resolution_mode.as_keyword().to_string()),
            tx,
            Op::Assert,
        ));
    }

    datoms
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::AgentId;

    #[test]
    fn genesis_produces_18_attributes() {
        let specs = axiomatic_attributes();
        assert_eq!(
            specs.len(),
            18,
            "INV-SCHEMA-002: exactly 18 axiomatic attributes"
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
    fn schema_from_genesis_has_18_attributes() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(0, 0, agent);
        let datoms: BTreeSet<Datom> = genesis_datoms(tx).into_iter().collect();
        let schema = Schema::from_datoms(&datoms);
        assert_eq!(schema.len(), 18);
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
            18,
            "all 18 attributes should be flagged as removed"
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

        assert_eq!(new_schema.len(), 19);
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

    // -------------------------------------------------------------------
    // Layer 1 tests — Trilateral Domain Attributes
    // -------------------------------------------------------------------

    #[test]
    fn layer_1_produces_24_attributes() {
        let specs = layer_1_attributes();
        assert_eq!(
            specs.len(),
            24,
            "Layer 1 must have exactly 24 trilateral attributes"
        );
    }

    #[test]
    fn layer_1_datoms_are_deterministic() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(1, 0, agent);
        let d1 = layer_1_datoms(tx);
        let d2 = layer_1_datoms(tx);
        assert_eq!(d1, d2, "Layer 1 datoms must be deterministic");
    }

    #[test]
    fn layer_1_schema_from_datoms() {
        let agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, agent);
        let l1_tx = TxId::new(1, 0, agent);

        let mut datoms: BTreeSet<Datom> = genesis_datoms(genesis_tx).into_iter().collect();
        for d in layer_1_datoms(l1_tx) {
            datoms.insert(d);
        }
        let schema = Schema::from_datoms(&datoms);
        assert_eq!(schema.len(), 18 + 24, "genesis + L1 = 42 attributes");
    }

    #[test]
    fn layer_1_has_correct_value_types() {
        let agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, agent);
        let l1_tx = TxId::new(1, 0, agent);

        let mut datoms: BTreeSet<Datom> = genesis_datoms(genesis_tx).into_iter().collect();
        for d in layer_1_datoms(l1_tx) {
            datoms.insert(d);
        }
        let schema = Schema::from_datoms(&datoms);

        // Intent attributes — all String
        for ident in &[
            ":intent/decision",
            ":intent/rationale",
            ":intent/source",
            ":intent/goal",
            ":intent/constraint",
            ":intent/preference",
            ":intent/noted",
        ] {
            let attr = Attribute::from_keyword(ident);
            let def = schema
                .attribute(&attr)
                .unwrap_or_else(|| panic!("L1 missing {ident}"));
            assert_eq!(
                def.value_type,
                ValueType::String,
                "{ident} should be String"
            );
        }

        // Spec attributes with specific types
        let attr = Attribute::from_keyword(":spec/stage");
        assert_eq!(schema.attribute(&attr).unwrap().value_type, ValueType::Long);

        let attr = Attribute::from_keyword(":spec/element-type");
        assert_eq!(
            schema.attribute(&attr).unwrap().value_type,
            ValueType::Keyword
        );

        let attr = Attribute::from_keyword(":spec/witnessed");
        assert_eq!(
            schema.attribute(&attr).unwrap().value_type,
            ValueType::Boolean
        );

        // Impl attributes
        let attr = Attribute::from_keyword(":impl/implements");
        assert_eq!(schema.attribute(&attr).unwrap().value_type, ValueType::Ref);

        let attr = Attribute::from_keyword(":impl/coverage");
        assert_eq!(
            schema.attribute(&attr).unwrap().value_type,
            ValueType::Double
        );
    }

    #[test]
    fn layer_1_is_valid_evolution_of_genesis() {
        let agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, agent);
        let l1_tx = TxId::new(1, 0, agent);

        let genesis_set: BTreeSet<Datom> = genesis_datoms(genesis_tx).into_iter().collect();
        let genesis_schema = Schema::from_datoms(&genesis_set);

        let mut full_set = genesis_set;
        for d in layer_1_datoms(l1_tx) {
            full_set.insert(d);
        }
        let l1_schema = Schema::from_datoms(&full_set);

        let errors = genesis_schema.validate_evolution(&l1_schema);
        assert!(
            errors.is_empty(),
            "L1 must be a valid evolution of genesis: {:?}",
            errors
        );
    }

    // -------------------------------------------------------------------
    // Layer 2 tests — Specification Element Attributes
    // -------------------------------------------------------------------

    #[test]
    fn layer_2_produces_36_attributes() {
        let specs = layer_2_attributes();
        assert_eq!(
            specs.len(),
            LAYER_2_COUNT,
            "Layer 2 must have exactly {LAYER_2_COUNT} specification element attributes"
        );
    }

    #[test]
    fn layer_2_datoms_are_deterministic() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(2, 0, agent);
        let d1 = layer_2_datoms(tx);
        let d2 = layer_2_datoms(tx);
        assert_eq!(d1, d2, "Layer 2 datoms must be deterministic");
    }

    #[test]
    fn layer_2_schema_from_datoms() {
        let agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, agent);
        let l1_tx = TxId::new(1, 0, agent);
        let l2_tx = TxId::new(2, 0, agent);

        let mut datoms: BTreeSet<Datom> = genesis_datoms(genesis_tx).into_iter().collect();
        for d in layer_1_datoms(l1_tx) {
            datoms.insert(d);
        }
        for d in layer_2_datoms(l2_tx) {
            datoms.insert(d);
        }
        let schema = Schema::from_datoms(&datoms);
        assert_eq!(
            schema.len(),
            18 + 24 + LAYER_2_COUNT,
            "genesis(18) + L1(24) + L2({LAYER_2_COUNT}) = {} attributes",
            18 + 24 + LAYER_2_COUNT
        );
    }

    #[test]
    fn layer_2_has_correct_value_types() {
        let agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, agent);
        let l2_tx = TxId::new(2, 0, agent);

        let mut datoms: BTreeSet<Datom> = genesis_datoms(genesis_tx).into_iter().collect();
        for d in layer_2_datoms(l2_tx) {
            datoms.insert(d);
        }
        let schema = Schema::from_datoms(&datoms);

        // Core element attributes
        let cases: Vec<(&str, ValueType)> = vec![
            (":element/id", ValueType::String),
            (":element/type", ValueType::Keyword),
            (":element/title", ValueType::String),
            (":element/body", ValueType::String),
            (":element/namespace", ValueType::Keyword),
            (":element/traces-to", ValueType::String),
            (":element/status", ValueType::Keyword),
            (":element/confidence", ValueType::Double),
            // Invariant-specific
            (":inv/statement", ValueType::String),
            (":inv/falsification", ValueType::String),
            (":inv/verification", ValueType::String),
            (":inv/property-type", ValueType::Keyword),
            // ADR-specific
            (":adr/problem", ValueType::String),
            (":adr/decision", ValueType::String),
            (":adr/alternatives", ValueType::String),
            (":adr/consequences", ValueType::String),
            (":adr/superseded-by", ValueType::String),
            // Negative case
            (":neg/violation", ValueType::String),
            (":neg/detection", ValueType::String),
            (":neg/mitigation", ValueType::String),
            // Cross-reference
            (":dep/from", ValueType::Ref),
            (":dep/to", ValueType::Ref),
            (":dep/type", ValueType::Keyword),
            // Session
            (":session/agent", ValueType::Ref),
            (":session/task", ValueType::String),
            (":session/start-tx", ValueType::Ref),
            (":session/harvest-quality", ValueType::Double),
            // Methodology
            (":methodology/score", ValueType::Double),
            (":methodology/trend", ValueType::Keyword),
            (":methodology/harvest-count", ValueType::Long),
            (":methodology/turn-count", ValueType::Long),
            // Coherence
            (":coherence/phi", ValueType::Double),
            (":coherence/quadrant", ValueType::Keyword),
            (":coherence/d-is", ValueType::Double),
            (":coherence/d-sp", ValueType::Double),
            (":coherence/beta-1", ValueType::Long),
        ];

        for (ident, expected_type) in &cases {
            let attr = Attribute::from_keyword(ident);
            let def = schema
                .attribute(&attr)
                .unwrap_or_else(|| panic!("L2 missing {ident}"));
            assert_eq!(
                def.value_type, *expected_type,
                "{ident}: expected {:?}, got {:?}",
                expected_type, def.value_type
            );
        }
    }

    #[test]
    fn layer_2_element_id_has_unique_identity() {
        let specs = layer_2_attributes();
        let element_id_spec = specs
            .iter()
            .find(|s| s.ident.as_str() == ":element/id")
            .expect(":element/id must exist in L2");
        assert_eq!(
            element_id_spec.unique,
            Some(Uniqueness::Identity),
            ":element/id must have uniqueness = identity for upsert"
        );
    }

    #[test]
    fn layer_2_adr_alternatives_is_cardinality_many() {
        let specs = layer_2_attributes();
        let alt_spec = specs
            .iter()
            .find(|s| s.ident.as_str() == ":adr/alternatives")
            .expect(":adr/alternatives must exist in L2");
        assert_eq!(
            alt_spec.cardinality,
            Cardinality::Many,
            ":adr/alternatives must be cardinality many"
        );
    }

    #[test]
    fn layer_2_element_traces_to_is_cardinality_many() {
        let specs = layer_2_attributes();
        let spec = specs
            .iter()
            .find(|s| s.ident.as_str() == ":element/traces-to")
            .expect(":element/traces-to must exist in L2");
        assert_eq!(
            spec.cardinality,
            Cardinality::Many,
            ":element/traces-to must be cardinality many"
        );
    }

    #[test]
    fn layer_2_is_valid_evolution_of_genesis_plus_l1() {
        let agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, agent);
        let l1_tx = TxId::new(1, 0, agent);
        let l2_tx = TxId::new(2, 0, agent);

        // Build L0+L1 schema
        let mut l01_datoms: BTreeSet<Datom> = genesis_datoms(genesis_tx).into_iter().collect();
        for d in layer_1_datoms(l1_tx) {
            l01_datoms.insert(d);
        }
        let l01_schema = Schema::from_datoms(&l01_datoms);

        // Build L0+L1+L2 schema
        let mut full_datoms = l01_datoms;
        for d in layer_2_datoms(l2_tx) {
            full_datoms.insert(d);
        }
        let l012_schema = Schema::from_datoms(&full_datoms);

        let errors = l01_schema.validate_evolution(&l012_schema);
        assert!(
            errors.is_empty(),
            "L2 must be a valid evolution of L0+L1: {:?}",
            errors
        );
    }

    #[test]
    fn all_layer_2_idents_are_unique() {
        let specs = layer_2_attributes();
        let mut seen = std::collections::HashSet::new();
        for spec in &specs {
            let ident = spec.ident.as_str().to_string();
            assert!(seen.insert(ident.clone()), "Duplicate L2 ident: {ident}");
        }
    }

    #[test]
    fn no_layer_2_ident_collides_with_layer_0_or_1() {
        let l0 = axiomatic_attributes();
        let l1 = layer_1_attributes();
        let l2 = layer_2_attributes();

        let l0_idents: std::collections::HashSet<String> =
            l0.iter().map(|s| s.ident.as_str().to_string()).collect();
        let l1_idents: std::collections::HashSet<String> =
            l1.iter().map(|s| s.ident.as_str().to_string()).collect();

        for spec in &l2 {
            let ident = spec.ident.as_str().to_string();
            assert!(
                !l0_idents.contains(&ident),
                "L2 ident {ident} collides with L0"
            );
            assert!(
                !l1_idents.contains(&ident),
                "L2 ident {ident} collides with L1"
            );
        }
    }

    #[test]
    fn domain_schema_datoms_combines_l1_and_l2() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(1, 0, agent);

        let combined = domain_schema_datoms(tx);
        let l1 = layer_1_datoms(tx);
        let l2 = layer_2_datoms(tx);

        assert_eq!(
            combined.len(),
            l1.len() + l2.len(),
            "domain_schema_datoms must combine L1 and L2"
        );
    }

    #[test]
    fn domain_schema_has_78_attributes() {
        let agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, agent);
        let domain_tx = TxId::new(1, 0, agent);

        let mut datoms: BTreeSet<Datom> = genesis_datoms(genesis_tx).into_iter().collect();
        for d in domain_schema_datoms(domain_tx) {
            datoms.insert(d);
        }
        let schema = Schema::from_datoms(&datoms);

        let expected = 18 + 24 + LAYER_2_COUNT; // 78
        assert_eq!(
            schema.len(),
            expected,
            "Domain schema (L0+L1+L2) must have {expected} attributes, got {}",
            schema.len()
        );
    }

    // -------------------------------------------------------------------
    // Layer 3 tests — Discovery/Exploration Attributes
    // -------------------------------------------------------------------

    #[test]
    fn layer_3_produces_20_attributes() {
        let specs = layer_3_attributes();
        assert_eq!(
            specs.len(),
            LAYER_3_COUNT,
            "Layer 3 must have exactly {LAYER_3_COUNT} exploration attributes"
        );
    }

    #[test]
    fn layer_3_datoms_are_deterministic() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(3, 0, agent);
        let d1 = layer_3_datoms(tx);
        let d2 = layer_3_datoms(tx);
        assert_eq!(d1, d2, "Layer 3 datoms must be deterministic");
    }

    #[test]
    fn layer_3_schema_from_datoms() {
        let agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, agent);
        let domain_tx = TxId::new(1, 0, agent);
        let l3_tx = TxId::new(3, 0, agent);

        let mut datoms: BTreeSet<Datom> = genesis_datoms(genesis_tx).into_iter().collect();
        for d in domain_schema_datoms(domain_tx) {
            datoms.insert(d);
        }
        for d in layer_3_datoms(l3_tx) {
            datoms.insert(d);
        }
        let schema = Schema::from_datoms(&datoms);
        assert_eq!(
            schema.len(),
            18 + 24 + LAYER_2_COUNT + LAYER_3_COUNT,
            "genesis(18) + L1(24) + L2({LAYER_2_COUNT}) + L3({LAYER_3_COUNT}) = {} attributes",
            18 + 24 + LAYER_2_COUNT + LAYER_3_COUNT
        );
    }

    #[test]
    fn layer_3_has_correct_value_types() {
        let agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, agent);
        let l3_tx = TxId::new(3, 0, agent);

        let mut datoms: BTreeSet<Datom> = genesis_datoms(genesis_tx).into_iter().collect();
        for d in layer_3_datoms(l3_tx) {
            datoms.insert(d);
        }
        let schema = Schema::from_datoms(&datoms);

        let cases: Vec<(&str, ValueType)> = vec![
            (":exploration/id", ValueType::String),
            (":exploration/source", ValueType::String),
            (":exploration/category", ValueType::Keyword),
            (":exploration/confidence", ValueType::Double),
            (":exploration/maturity", ValueType::Keyword),
            (":exploration/body", ValueType::String),
            (":exploration/title", ValueType::String),
            (":exploration/tags", ValueType::Keyword),
            (":promotion/status", ValueType::Keyword),
            (":promotion/target-element", ValueType::String),
            (":promotion/target-namespace", ValueType::Keyword),
            (":promotion/target-type", ValueType::Keyword),
            (":promotion/promoted-tx", ValueType::Ref),
            (":promotion/phi-before", ValueType::Double),
            (":promotion/phi-after", ValueType::Double),
            (":exploration/depends-on", ValueType::Ref),
            (":exploration/refines", ValueType::Ref),
            (":exploration/related-spec", ValueType::Ref),
            (":exploration/source-session", ValueType::Ref),
            (":exploration/evidence", ValueType::String),
        ];

        for (ident_str, expected_type) in cases {
            let attr = Attribute::from_keyword(ident_str);
            let def = schema
                .attribute(&attr)
                .unwrap_or_else(|| panic!("L3 missing {ident_str}"));
            assert_eq!(
                def.value_type, expected_type,
                "{ident_str} should be {expected_type:?}"
            );
        }
    }

    #[test]
    fn layer_3_is_valid_evolution_of_l0_l1_l2() {
        let agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, agent);
        let domain_tx = TxId::new(1, 0, agent);
        let l3_tx = TxId::new(3, 0, agent);

        // Build L0+L1+L2 schema
        let mut l012_datoms: BTreeSet<Datom> = genesis_datoms(genesis_tx).into_iter().collect();
        for d in domain_schema_datoms(domain_tx) {
            l012_datoms.insert(d);
        }
        let l012_schema = Schema::from_datoms(&l012_datoms);

        // Build L0+L1+L2+L3 schema
        let mut full_datoms = l012_datoms;
        for d in layer_3_datoms(l3_tx) {
            full_datoms.insert(d);
        }
        let l0123_schema = Schema::from_datoms(&full_datoms);

        let errors = l012_schema.validate_evolution(&l0123_schema);
        assert!(
            errors.is_empty(),
            "L3 must be a valid evolution of L0+L1+L2: {:?}",
            errors
        );
    }

    #[test]
    fn all_layer_3_idents_are_unique() {
        let specs = layer_3_attributes();
        let mut seen = std::collections::HashSet::new();
        for spec in &specs {
            let ident = spec.ident.as_str().to_string();
            assert!(seen.insert(ident.clone()), "Duplicate L3 ident: {ident}");
        }
    }

    #[test]
    fn no_layer_3_ident_collides_with_lower_layers() {
        let l0 = axiomatic_attributes();
        let l1 = layer_1_attributes();
        let l2 = layer_2_attributes();
        let l3 = layer_3_attributes();

        let lower_idents: std::collections::HashSet<String> = l0
            .iter()
            .chain(l1.iter())
            .chain(l2.iter())
            .map(|s| s.ident.as_str().to_string())
            .collect();

        for spec in &l3 {
            let ident = spec.ident.as_str().to_string();
            assert!(
                !lower_idents.contains(&ident),
                "L3 ident {ident} collides with L0/L1/L2"
            );
        }
    }

    #[test]
    fn layer_3_only_references_layer_0_types() {
        let valid_l0_types = [
            ValueType::String,
            ValueType::Keyword,
            ValueType::Boolean,
            ValueType::Long,
            ValueType::Double,
            ValueType::Instant,
            ValueType::Uuid,
            ValueType::Ref,
            ValueType::Bytes,
        ];

        for spec in &layer_3_attributes() {
            assert!(
                valid_l0_types.contains(&spec.value_type),
                "L3 attribute {} uses non-L0 type {:?}",
                spec.ident.as_str(),
                spec.value_type
            );
        }
    }

    #[test]
    fn full_schema_datoms_combines_all_layers() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(1, 0, agent);

        let full = full_schema_datoms(tx);
        let l1 = layer_1_datoms(tx);
        let l2 = layer_2_datoms(tx);
        let l3 = layer_3_datoms(tx);

        assert_eq!(
            full.len(),
            l1.len() + l2.len() + l3.len(),
            "full_schema_datoms must combine L1, L2, and L3"
        );
    }

    #[test]
    fn full_schema_has_98_attributes() {
        let agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, agent);
        let full_tx = TxId::new(1, 0, agent);

        let mut datoms: BTreeSet<Datom> = genesis_datoms(genesis_tx).into_iter().collect();
        for d in full_schema_datoms(full_tx) {
            datoms.insert(d);
        }
        let schema = Schema::from_datoms(&datoms);

        let expected = 18 + 24 + LAYER_2_COUNT + LAYER_3_COUNT; // 98
        assert_eq!(
            schema.len(),
            expected,
            "Full schema (L0+L1+L2+L3) must have {expected} attributes, got {}",
            schema.len()
        );
    }

    #[test]
    fn layer_2_only_references_layer_0_types() {
        // All L2 value types must be basic types available in L0.
        // This verifies INV-SCHEMA-006 layer dependency ordering.
        let valid_l0_types = [
            ValueType::String,
            ValueType::Keyword,
            ValueType::Boolean,
            ValueType::Long,
            ValueType::Double,
            ValueType::Instant,
            ValueType::Uuid,
            ValueType::Ref,
            ValueType::Bytes,
        ];

        for spec in &layer_2_attributes() {
            assert!(
                valid_l0_types.contains(&spec.value_type),
                "L2 attribute {} uses non-L0 type {:?}",
                spec.ident.as_str(),
                spec.value_type
            );
        }
    }

    // -------------------------------------------------------------------
    // Schema semilattice witness (proptest)
    //
    // Schema forms a join-semilattice under set union. The schema is
    // derived from the store's datom set, and store merge IS set union,
    // so the semilattice properties of Schema are inherited from the
    // semilattice properties of Store.merge(). We witness this directly:
    // merge two stores, derive their schemas, verify the four
    // semilattice axioms hold on the resulting attribute sets.
    // -------------------------------------------------------------------

    mod semilattice_proptests {
        use super::*;
        use crate::proptest_strategies::{arb_store, arb_store_pair};
        use proptest::prelude::*;

        fn schema_attr_set(schema: &Schema) -> BTreeSet<String> {
            schema
                .attributes()
                .map(|(a, _)| a.as_str().to_string())
                .collect()
        }

        proptest! {
            // L1 — Closure: merging two stores produces a valid schema
            // whose attribute set is the union of both input schemas.
            #[test]
            fn schema_semilattice_closure((s1, s2) in arb_store_pair(2)) {
                let attrs_1 = schema_attr_set(s1.schema());
                let attrs_2 = schema_attr_set(s2.schema());
                let expected_union: BTreeSet<String> =
                    attrs_1.union(&attrs_2).cloned().collect();

                let mut merged = s1.clone_store();
                merged.merge(&s2);
                let merged_attrs = schema_attr_set(merged.schema());

                // Merged schema contains at least the union of both inputs
                for attr in &expected_union {
                    prop_assert!(
                        merged_attrs.contains(attr),
                        "Closure violated: merged schema missing attribute {attr}"
                    );
                }
                // Merged schema contains no attributes absent from either input
                for attr in &merged_attrs {
                    prop_assert!(
                        attrs_1.contains(attr) || attrs_2.contains(attr),
                        "Closure violated: merged schema has spurious attribute {attr}"
                    );
                }
            }

            // L2 — Commutativity: schema(A merge B) == schema(B merge A)
            #[test]
            fn schema_semilattice_commutativity((s1, s2) in arb_store_pair(2)) {
                let mut left = s1.clone_store();
                left.merge(&s2);

                let mut right = s2.clone_store();
                right.merge(&s1);

                let left_attrs = schema_attr_set(left.schema());
                let right_attrs = schema_attr_set(right.schema());

                prop_assert_eq!(
                    left_attrs, right_attrs,
                    "Commutativity violated: schema(A∪B) != schema(B∪A)"
                );
            }

            // L3 — Associativity: schema((A merge B) merge C) == schema(A merge (B merge C))
            #[test]
            fn schema_semilattice_associativity(
                s1 in arb_store(2),
                s2 in arb_store(2),
                s3 in arb_store(2),
            ) {
                // (A ∪ B) ∪ C
                let mut ab = s1.clone_store();
                ab.merge(&s2);
                ab.merge(&s3);

                // A ∪ (B ∪ C)
                let mut bc = s2.clone_store();
                bc.merge(&s3);
                let mut a_bc = s1.clone_store();
                a_bc.merge(&bc);

                let left_attrs = schema_attr_set(ab.schema());
                let right_attrs = schema_attr_set(a_bc.schema());

                prop_assert_eq!(
                    left_attrs, right_attrs,
                    "Associativity violated: schema((A∪B)∪C) != schema(A∪(B∪C))"
                );
            }

            // L4 — Idempotency: schema(A merge A) == schema(A)
            #[test]
            fn schema_semilattice_idempotency(store in arb_store(3)) {
                let before_attrs = schema_attr_set(store.schema());

                let mut merged = store.clone_store();
                merged.merge(&store);

                let after_attrs = schema_attr_set(merged.schema());

                prop_assert_eq!(
                    before_attrs, after_attrs,
                    "Idempotency violated: schema(A∪A) != schema(A)"
                );
            }

            // Monotonicity witness: schema only grows under merge (INV-SCHEMA-003).
            // schema(A) ⊆ schema(A merge B) for all A, B.
            #[test]
            fn schema_semilattice_monotonicity((s1, s2) in arb_store_pair(2)) {
                let before_attrs = schema_attr_set(s1.schema());

                let mut merged = s1.clone_store();
                merged.merge(&s2);

                let after_attrs = schema_attr_set(merged.schema());

                for attr in &before_attrs {
                    prop_assert!(
                        after_attrs.contains(attr),
                        "Monotonicity violated: attribute {attr} lost after merge"
                    );
                }
            }

            // Schema evolution compatibility: merged schema is a valid evolution
            // of either input schema.
            #[test]
            fn schema_merge_is_valid_evolution((s1, s2) in arb_store_pair(2)) {
                let mut merged = s1.clone_store();
                merged.merge(&s2);

                let errors_from_s1 = s1.schema().validate_evolution(merged.schema());
                let errors_from_s2 = s2.schema().validate_evolution(merged.schema());

                prop_assert!(
                    errors_from_s1.is_empty(),
                    "Merged schema is not a valid evolution of s1: {:?}",
                    errors_from_s1
                );
                prop_assert!(
                    errors_from_s2.is_empty(),
                    "Merged schema is not a valid evolution of s2: {:?}",
                    errors_from_s2
                );
            }
        }
    }
}
