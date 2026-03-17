//! Schema-as-data: attribute definitions stored as datoms (C3).
//!
//! The schema is derived from the store, not stored separately. Schema
//! evolution is a transaction, not a migration. The axiomatic meta-schema
//! attributes describe themselves (INV-SCHEMA-005).
//!
//! # Six-Layer Schema Architecture (INV-SCHEMA-006)
//!
//! - **Layer 0** (Meta-Schema): `GENESIS_ATTR_COUNT` axiomatic attributes — `:db/*`, `:lattice/*`, `:tx/*`.
//! - **Layer 1** (Trilateral): `LAYER_1_COUNT` domain attributes — `:intent/*`, `:spec/*`, `:impl/*`.
//! - **Layer 2** (Specification Elements): `LAYER_2_COUNT` rich-metadata attributes —
//!   `:element/*`, `:inv/*`, `:adr/*`, `:neg/*`, `:dep/*`, `:session/*`,
//!   `:methodology/*`, `:coherence/*`.
//! - **Layer 3** (Discovery/Exploration): `LAYER_3_COUNT` attributes — `:exploration/*`, `:promotion/*`, `:signal/*`, `:proposal/*`, `:branch/*`.
//! - **Layers 4–5**: Coordination, Workflow (future stages).
//!
//! Each layer depends only on layers below it. Layer 0 is installed at genesis.
//! Layers 1–2 are installed via schema-evolution transactions.
//!
//! # Invariants
//!
//! - **INV-SCHEMA-001**: Schema is a subset of the store, not separate DDL.
//! - **INV-SCHEMA-002**: Genesis contains exactly `GENESIS_ATTR_COUNT` axiomatic attributes.
//! - **INV-SCHEMA-003**: Schema can only grow (monotonicity).
//! - **INV-SCHEMA-004**: Every transacted datom is validated against schema.
//! - **INV-SCHEMA-005**: Axiomatic attributes describe themselves using A₀.
//! - **INV-SCHEMA-006**: Six-layer architecture with dependency ordering.
//! - **INV-SCHEMA-007**: Lattice definition completeness.
//! - **INV-SCHEMA-008**: Diamond lattice signal generation.
//!
//! # Design Decisions
//!
//! - ADR-SCHEMA-001: Schema-as-data over external DDL.
//! - ADR-SCHEMA-002: Axiomatic attributes (see `GENESIS_ATTR_COUNT`).
//! - ADR-SCHEMA-003: Six-layer architecture with dependency ordering.
//! - ADR-SCHEMA-004: Twelve named lattices for resolution.
//! - ADR-SCHEMA-005: Owned schema with borrow API.
//! - ADR-SCHEMA-006: Value type union (9 variants at Stage 0).
//! - ADR-SCHEMA-007: Typed spec element relationships.
//! - ADR-SCHEMA-008: Coordination lattice pre-registration.
//!
//! # Negative Cases
//!
//! - NEG-SCHEMA-001: No external schema — schema lives in the store.
//! - NEG-SCHEMA-002: No schema deletion — schema attributes only grow.
//! - NEG-SCHEMA-003: No circular layer dependencies.

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

    /// Cardinality for an attribute (defaults to One if unknown).
    pub fn cardinality(&self, attr: &Attribute) -> Cardinality {
        self.attrs
            .get(attr)
            .map(|def| def.cardinality)
            .unwrap_or(Cardinality::One)
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

/// Number of axiomatic meta-schema attributes (Layer 0).
///
/// This count includes the 9 `:db/*` meta-schema attributes, 5 `:lattice/*`
/// definition attributes, and 5 `:tx/*` transaction metadata attributes
/// (including `:tx/coherence-override` for the coherence gate audit trail).
///
/// INV-SCHEMA-002: Genesis contains exactly this many axiomatic attributes.
pub const GENESIS_ATTR_COUNT: usize = 19;

/// Number of Layer 1 (Trilateral) attributes.
///
/// Intent (7) + Spec (11) + Impl (6) = 24 domain attributes.
pub const LAYER_1_COUNT: usize = 24;

/// Produce the genesis datom set — the axiomatic meta-schema attributes.
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

/// The axiomatic meta-schema attributes (INV-SCHEMA-002).
/// Count must equal `GENESIS_ATTR_COUNT`.
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
        // Transaction metadata (5 attributes)
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
        attr(
            ":tx/coherence-override",
            ValueType::Boolean,
            Cardinality::One,
            "Audit trail: true when --force bypassed coherence gate",
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

/// The Layer 1 (Trilateral) attributes: Intent (7) + Spec (11) + Impl (6).
/// Count must equal `LAYER_1_COUNT`.
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

/// Number of Layer 3 exploration/discovery/proposal/branch attributes.
pub const LAYER_3_COUNT: usize = 36;

/// The 36 Layer 3 (Discovery/Exploration/Proposal/Branch) attributes.
///
/// These capture the lifecycle of exploratory knowledge — from initial
/// discovery through promotion to formal specification elements. They enable
/// the store-first specification pipeline where exploration entities gain
/// `:spec/*` attributes via `braid promote` rather than being re-entered
/// from markdown.
///
/// Organized into 5 groups:
/// - Exploration Identity (9): source, category, confidence, maturity, content-hash
/// - Promotion Lifecycle (7): promotion status, target element, verification
/// - Exploration Cross-Reference (5): links between exploration entities
/// - Proposal Lifecycle (10): spec proposals with review workflow
/// - Branch Metadata (5): branch entities and per-transaction branch tagging
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
        attr(
            ":exploration/content-hash",
            ValueType::Bytes,
            Cardinality::One,
            "BLAKE3 hash of :exploration/body for cross-session dedup (INV-HARVEST-006)",
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
        // =================================================================
        // Proposal Lifecycle Attributes (10) — spec proposal review workflow
        // =================================================================
        attr(
            ":proposal/type",
            ValueType::Keyword,
            Cardinality::One,
            "Proposal element type: :proposal.type/invariant, :proposal.type/adr, :proposal.type/negative-case",
        ),
        attr(
            ":proposal/status",
            ValueType::Keyword,
            Cardinality::One,
            "Review status: :proposal.status/proposed, :proposal.status/reviewed, :proposal.status/accepted, :proposal.status/rejected",
        ),
        attr(
            ":proposal/source",
            ValueType::Ref,
            Cardinality::One,
            "Reference to the entity that triggered this proposal (e.g., exploration or harvest entity)",
        ),
        attr(
            ":proposal/suggested-id",
            ValueType::String,
            Cardinality::One,
            "Suggested spec element ID (e.g., INV-STORE-017, ADR-MERGE-005, NEG-SCHEMA-004)",
        ),
        attr(
            ":proposal/statement",
            ValueType::String,
            Cardinality::One,
            "The proposed formal statement text for invariants or negative cases",
        ),
        attr(
            ":proposal/falsification",
            ValueType::String,
            Cardinality::One,
            "Proposed falsification condition: how to verify violation (C6)",
        ),
        attr(
            ":proposal/traces-to",
            ValueType::String,
            Cardinality::One,
            "SEED.md section reference that motivates this proposal (C5 traceability)",
        ),
        attr(
            ":proposal/confidence",
            ValueType::Double,
            Cardinality::One,
            "Classification confidence for the proposal (0.0-1.0). Auto-accept threshold: 0.9",
        ),
        attr(
            ":proposal/reviewer",
            ValueType::Ref,
            Cardinality::One,
            "Reference to the agent or human entity that reviewed this proposal",
        ),
        attr(
            ":proposal/review-note",
            ValueType::String,
            Cardinality::One,
            "Reviewer rationale for accepting or rejecting the proposal",
        ),
        // =================================================================
        // Branch Metadata Attributes (5) — branch operations (INV-MERGE-006)
        // =================================================================
        attr(
            ":tx/branch",
            ValueType::String,
            Cardinality::One,
            "Branch name on this transaction — filters datom set into branch view",
        ),
        attr_unique(
            ":branch/name",
            ValueType::String,
            Cardinality::One,
            "Branch entity name (unique identity for the branch)",
            Uniqueness::Identity,
        ),
        attr(
            ":branch/status",
            ValueType::Keyword,
            Cardinality::One,
            "Branch lifecycle status: :branch.status/active, :branch.status/merged, :branch.status/abandoned",
        ),
        attr(
            ":branch/purpose",
            ValueType::String,
            Cardinality::One,
            "Why this branch exists — human-readable rationale",
        ),
        attr(
            ":branch/parent",
            ValueType::Ref,
            Cardinality::One,
            "Reference to the parent branch entity",
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
    datoms.extend(layer_4_datoms(tx));
    datoms
}

// ---------------------------------------------------------------------------
// Layer 4 — Workflow / Coordination Attributes (INV-SCHEMA-006)
// ---------------------------------------------------------------------------

/// Number of Layer 4 workflow/coordination attributes.
pub const LAYER_4_COUNT: usize = 32;

/// The Layer 4 (Workflow/Coordination) attributes.
///
/// Organized into 4 groups:
/// - Session Extensions (2): persistent session status and wall-clock time
/// - Task Core (10): issue tracking as datoms — replaces external issue trackers
/// - Task Lifecycle (4): creation, closure, sourcing
/// - Plan (5): plan documents linked to tasks and spec elements
/// - Comment (4): per-task discussion as datoms
///
/// Task status uses lattice resolution (open < in-progress < closed) to ensure
/// monotonic progression under CRDT merge (INV-TASK-001). Dependencies form
/// a DAG (INV-TASK-002) enforced at insertion time.
///
/// Depends only on Layer 0 value types (INV-SCHEMA-006 layer ordering).
pub fn layer_4_attributes() -> Vec<AttributeSpec> {
    vec![
        // =================================================================
        // Session Extensions (2) — persistent session identity
        // =================================================================
        attr(
            ":session/started-at",
            ValueType::Long,
            Cardinality::One,
            "Wall-clock time (unix seconds) when the session started",
        ),
        attr(
            ":session/status",
            ValueType::Keyword,
            Cardinality::One,
            "Session lifecycle status: :session.status/active, :session.status/closed",
        ),
        // =================================================================
        // Task Core (10) — issue tracking as datoms (INV-TASK-001..004)
        // =================================================================
        attr_unique(
            ":task/id",
            ValueType::String,
            Cardinality::One,
            "Short task ID (e.g., t-aB3c)",
            Uniqueness::Identity,
        ),
        attr(
            ":task/title",
            ValueType::String,
            Cardinality::One,
            "Human-readable task title",
        ),
        attr(
            ":task/description",
            ValueType::String,
            Cardinality::One,
            "Detailed task description",
        ),
        attr(
            ":task/status",
            ValueType::Keyword,
            Cardinality::One,
            "Task status (lattice-resolved): :task.status/open, :task.status/in-progress, :task.status/closed",
        ),
        attr(
            ":task/priority",
            ValueType::Long,
            Cardinality::One,
            "Priority level: 0=critical, 1=high, 2=medium, 3=low, 4=backlog",
        ),
        attr(
            ":task/type",
            ValueType::Keyword,
            Cardinality::One,
            "Task type: :task.type/task, :task.type/bug, :task.type/feature, :task.type/epic, :task.type/question, :task.type/docs",
        ),
        attr_multi(
            ":task/labels",
            ValueType::Keyword,
            Cardinality::Many,
            "Categorical labels for filtering and grouping",
        ),
        attr_multi(
            ":task/depends-on",
            ValueType::Ref,
            Cardinality::Many,
            "Dependency edges to other task entities (must form a DAG, INV-TASK-002)",
        ),
        attr_multi(
            ":task/traces-to",
            ValueType::Ref,
            Cardinality::Many,
            "Links to spec/observation entities this task relates to",
        ),
        attr(
            ":task/parent",
            ValueType::Ref,
            Cardinality::One,
            "Parent epic entity (for task hierarchy)",
        ),
        // =================================================================
        // Task Lifecycle (4) — creation, closure, sourcing
        // =================================================================
        attr(
            ":task/created-at",
            ValueType::Long,
            Cardinality::One,
            "Wall-clock time (unix seconds) when the task was created",
        ),
        attr(
            ":task/closed-at",
            ValueType::Long,
            Cardinality::One,
            "Wall-clock time (unix seconds) when the task was closed",
        ),
        attr(
            ":task/close-reason",
            ValueType::String,
            Cardinality::One,
            "Why the task was closed",
        ),
        attr(
            ":task/source",
            ValueType::String,
            Cardinality::One,
            "Origin of the task (e.g., beads:brai-114c, manual, harvest)",
        ),
        // =================================================================
        // Plan (5) — structured plans as datoms
        // =================================================================
        attr_unique(
            ":plan/id",
            ValueType::String,
            Cardinality::One,
            "Plan identifier",
            Uniqueness::Identity,
        ),
        attr(
            ":plan/title",
            ValueType::String,
            Cardinality::One,
            "Plan name",
        ),
        attr(
            ":plan/body",
            ValueType::String,
            Cardinality::One,
            "Full markdown content of the plan",
        ),
        attr(
            ":plan/status",
            ValueType::Keyword,
            Cardinality::One,
            "Plan lifecycle status: :plan.status/draft, :plan.status/active, :plan.status/completed",
        ),
        attr_multi(
            ":plan/tasks",
            ValueType::Ref,
            Cardinality::Many,
            "Task entities covered by this plan",
        ),
        // =================================================================
        // Comment (4) — per-task discussion as datoms
        // =================================================================
        attr(
            ":comment/body",
            ValueType::String,
            Cardinality::One,
            "Comment text content",
        ),
        attr(
            ":comment/author",
            ValueType::String,
            Cardinality::One,
            "Author of the comment",
        ),
        attr(
            ":comment/task",
            ValueType::Ref,
            Cardinality::One,
            "Back-reference to the task entity this comment belongs to",
        ),
        attr(
            ":comment/created-at",
            ValueType::Long,
            Cardinality::One,
            "Wall-clock time (unix seconds) when the comment was created",
        ),
        // =================================================================
        // Config (4) — configuration as datoms (ADR-INTERFACE-005, WP2)
        // =================================================================
        attr_unique(
            ":config/key",
            ValueType::String,
            Cardinality::One,
            "Configuration key name (e.g., output.default-mode)",
            Uniqueness::Identity,
        ),
        attr(
            ":config/value",
            ValueType::String,
            Cardinality::One,
            "Configuration value (string-encoded)",
        ),
        attr(
            ":config/scope",
            ValueType::Keyword,
            Cardinality::One,
            "Configuration scope: :config.scope/global, :config.scope/project, :config.scope/session",
        ),
        attr(
            ":config/description",
            ValueType::String,
            Cardinality::One,
            "What this config key controls",
        ),
        // =================================================================
        // Verification Depth (3) — F(S) honesty (WP9, INV-DEPTH-001..003)
        // =================================================================
        attr(
            ":impl/verification-depth",
            ValueType::Long,
            Cardinality::One,
            "Verification depth level (0=unverified, 1=syntactic, 2=structural, 3=property, 4=formal). Lattice-resolved: monotonically non-decreasing.",
        ),
        attr(
            ":impl/verification-evidence",
            ValueType::String,
            Cardinality::One,
            "How the verification depth was determined (e.g., 'test_inv_store_001 found at line 42')",
        ),
        attr(
            ":spec/verification-depth",
            ValueType::Long,
            Cardinality::One,
            "Highest verification depth achieved across all :impl entities for this spec element",
        ),
    ]
}

/// Produce datoms for all Layer 4 attributes.
///
/// These should be transacted as a schema-evolution transaction after Layer 3.
/// Depends only on Layer 0 value types (INV-SCHEMA-006 layer ordering).
pub fn layer_4_datoms(tx: TxId) -> Vec<Datom> {
    schema_datoms_from_specs(&layer_4_attributes(), tx)
}

/// Check if Layer 4 schema attributes are installed in a set of datoms.
///
/// Returns true if `:task/id` attribute is registered (canary check).
pub fn has_layer_4(datoms: &BTreeSet<Datom>) -> bool {
    datoms.iter().any(|d| {
        d.attribute.as_str() == ":db/ident"
            && matches!(&d.value, Value::Keyword(k) if k == ":task/id")
    })
}

/// Generate a Layer 4 schema evolution transaction.
///
/// Returns None if Layer 4 is already installed.
pub fn layer_4_evolution_tx(datoms: &BTreeSet<Datom>, tx: TxId) -> Option<Vec<Datom>> {
    if has_layer_4(datoms) {
        None
    } else {
        Some(layer_4_datoms(tx))
    }
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

// Witnesses: INV-SCHEMA-001, INV-SCHEMA-002, INV-SCHEMA-003, INV-SCHEMA-004,
// INV-SCHEMA-005, INV-SCHEMA-006, INV-SCHEMA-007, INV-SCHEMA-009,
// ADR-SCHEMA-001, ADR-SCHEMA-002, ADR-SCHEMA-003, ADR-SCHEMA-005, ADR-SCHEMA-006,
// ADR-SCHEMA-007, ADR-SCHEMA-008,
// NEG-SCHEMA-001, NEG-SCHEMA-002, NEG-SCHEMA-003, NEG-BOOTSTRAP-001
#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::AgentId;

    // Verifies: INV-SCHEMA-002 — Genesis Completeness
    // Verifies: ADR-SCHEMA-002 — 17 Axiomatic Attributes
    #[test]
    fn genesis_produces_correct_attribute_count() {
        let specs = axiomatic_attributes();
        assert_eq!(
            specs.len(),
            GENESIS_ATTR_COUNT,
            "INV-SCHEMA-002: axiomatic attributes must match GENESIS_ATTR_COUNT"
        );
    }

    // Verifies: INV-STORE-008 — Genesis Determinism
    #[test]
    fn genesis_datoms_are_deterministic() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(0, 0, agent);
        let d1 = genesis_datoms(tx);
        let d2 = genesis_datoms(tx);
        assert_eq!(d1, d2, "INV-STORE-008: genesis is deterministic");
    }

    // Verifies: INV-SCHEMA-001 — Schema-as-Data
    // Verifies: ADR-SCHEMA-001 — Schema-as-Data Over DDL
    #[test]
    fn schema_from_genesis_has_correct_count() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(0, 0, agent);
        let datoms: BTreeSet<Datom> = genesis_datoms(tx).into_iter().collect();
        let schema = Schema::from_datoms(&datoms);
        assert_eq!(schema.len(), GENESIS_ATTR_COUNT);
    }

    // Verifies: INV-SCHEMA-004 — Schema Validation on Transact
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

    // Verifies: INV-SCHEMA-004 — Schema Validation on Transact (rejection path)
    // Verifies: NEG-SCHEMA-001 — No External Schema
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

    // Verifies: ADR-SCHEMA-006 — Value Type Union
    #[test]
    fn value_type_matches() {
        assert!(ValueType::String.matches(&Value::String("hi".into())));
        assert!(!ValueType::String.matches(&Value::Long(1)));
        assert!(ValueType::Ref.matches(&Value::Ref(EntityId::from_content(b"x"))));
    }

    // Verifies: INV-SCHEMA-003 — Schema Monotonicity (reflexive evolution)
    // Verifies: NEG-SCHEMA-002 — No Schema Deletion
    #[test]
    fn evolution_reflexive() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(0, 0, agent);
        let datoms: BTreeSet<Datom> = genesis_datoms(tx).into_iter().collect();
        let schema = Schema::from_datoms(&datoms);
        let errors = schema.validate_evolution(&schema);
        assert!(errors.is_empty(), "evolution(S, S) must be valid");
    }

    // Verifies: NEG-SCHEMA-002 — No Schema Deletion (detects removal)
    // Verifies: INV-SCHEMA-003 — Schema Monotonicity
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
            GENESIS_ATTR_COUNT,
            "all genesis attributes should be flagged as removed"
        );
        assert!(errors
            .iter()
            .all(|e| matches!(e, SchemaEvolutionError::AttributeRemoved(_))));
    }

    // Verifies: INV-SCHEMA-003 — Schema Monotonicity (additive evolution)
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

        assert_eq!(new_schema.len(), GENESIS_ATTR_COUNT + 1); // genesis + 1 custom
        let errors = old_schema.validate_evolution(&new_schema);
        assert!(errors.is_empty(), "adding attributes is valid evolution");
    }

    // Verifies: INV-SCHEMA-003 — Schema Monotonicity (superset check)
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
    // Verifies: INV-SCHEMA-006 — Six-Layer Schema Architecture
    // Verifies: ADR-SCHEMA-003 — Six-Layer Architecture
    // Verifies: NEG-SCHEMA-003 — No Circular Layer Dependencies
    // -------------------------------------------------------------------

    // Verifies: INV-SCHEMA-006 — Six-Layer Schema Architecture
    #[test]
    fn layer_1_produces_correct_count() {
        let specs = layer_1_attributes();
        assert_eq!(
            specs.len(),
            LAYER_1_COUNT,
            "Layer 1 must have exactly {LAYER_1_COUNT} trilateral attributes"
        );
    }

    // Verifies: INV-STORE-008 — Genesis Determinism (layer 1)
    #[test]
    fn layer_1_datoms_are_deterministic() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(1, 0, agent);
        let d1 = layer_1_datoms(tx);
        let d2 = layer_1_datoms(tx);
        assert_eq!(d1, d2, "Layer 1 datoms must be deterministic");
    }

    // Verifies: INV-SCHEMA-001 — Schema-as-Data (layer 1)
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
        assert_eq!(
            schema.len(),
            GENESIS_ATTR_COUNT + LAYER_1_COUNT,
            "genesis + L1 attributes"
        );
    }

    // Verifies: ADR-SCHEMA-006 — Value Type Union
    // Verifies: ADR-SCHEMA-007 — Typed Spec Element Relationships
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

    // Verifies: INV-SCHEMA-003 — Schema Monotonicity (L1 valid evolution)
    // Verifies: NEG-SCHEMA-003 — No Circular Layer Dependencies
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
    // Verifies: INV-SCHEMA-006 — Six-Layer Schema Architecture
    // Verifies: INV-SCHEMA-009 — Spec Dependency Graph Completeness
    // Verifies: ADR-SCHEMA-007 — Typed Spec Element Relationships
    // -------------------------------------------------------------------

    // Verifies: INV-SCHEMA-006 — Six-Layer Schema Architecture (layer 2 count)
    #[test]
    fn layer_2_produces_correct_count() {
        let specs = layer_2_attributes();
        assert_eq!(
            specs.len(),
            LAYER_2_COUNT,
            "Layer 2 must have exactly {LAYER_2_COUNT} specification element attributes"
        );
    }

    // Verifies: INV-STORE-008 — Genesis Determinism (layer 2)
    #[test]
    fn layer_2_datoms_are_deterministic() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(2, 0, agent);
        let d1 = layer_2_datoms(tx);
        let d2 = layer_2_datoms(tx);
        assert_eq!(d1, d2, "Layer 2 datoms must be deterministic");
    }

    // Verifies: INV-SCHEMA-001 — Schema-as-Data (layer 2)
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
            GENESIS_ATTR_COUNT + LAYER_1_COUNT + LAYER_2_COUNT,
            "genesis({GENESIS_ATTR_COUNT}) + L1({LAYER_1_COUNT}) + L2({LAYER_2_COUNT}) = {} attributes",
            GENESIS_ATTR_COUNT + LAYER_1_COUNT + LAYER_2_COUNT
        );
    }

    // Verifies: ADR-SCHEMA-006 — Value Type Union
    // Verifies: ADR-SCHEMA-007 — Typed Spec Element Relationships
    // Verifies: INV-SCHEMA-009 — Spec Dependency Graph Completeness
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

    // Verifies: INV-STORE-003 — Content-Addressable Identity (element ID uniqueness)
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

    // Verifies: ADR-SCHEMA-007 — Typed Spec Element Relationships
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

    // Verifies: ADR-SCHEMA-007 — Typed Spec Element Relationships
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

    // Verifies: INV-SCHEMA-003 — Schema Monotonicity (L2 valid evolution)
    // Verifies: NEG-SCHEMA-003 — No Circular Layer Dependencies
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

    // Verifies: INV-STORE-003 — Content-Addressable Identity
    #[test]
    fn all_layer_2_idents_are_unique() {
        let specs = layer_2_attributes();
        let mut seen = std::collections::HashSet::new();
        for spec in &specs {
            let ident = spec.ident.as_str().to_string();
            assert!(seen.insert(ident.clone()), "Duplicate L2 ident: {ident}");
        }
    }

    // Verifies: NEG-SCHEMA-003 — No Circular Layer Dependencies
    // Verifies: INV-SCHEMA-006 — Six-Layer Schema Architecture
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
    fn domain_schema_has_correct_count() {
        let agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, agent);
        let domain_tx = TxId::new(1, 0, agent);

        let mut datoms: BTreeSet<Datom> = genesis_datoms(genesis_tx).into_iter().collect();
        for d in domain_schema_datoms(domain_tx) {
            datoms.insert(d);
        }
        let schema = Schema::from_datoms(&datoms);

        let expected = GENESIS_ATTR_COUNT + LAYER_1_COUNT + LAYER_2_COUNT;
        assert_eq!(
            schema.len(),
            expected,
            "Domain schema (L0+L1+L2) must have {expected} attributes, got {}",
            schema.len()
        );
    }

    // -------------------------------------------------------------------
    // Layer 3 tests — Discovery/Exploration Attributes
    // Verifies: INV-SCHEMA-006 — Six-Layer Schema Architecture
    // Verifies: ADR-SCHEMA-003 — Six-Layer Architecture
    // -------------------------------------------------------------------

    // Verifies: INV-SCHEMA-006 — Six-Layer Schema Architecture (layer 3)
    #[test]
    fn layer_3_produces_correct_count() {
        let specs = layer_3_attributes();
        assert_eq!(
            specs.len(),
            LAYER_3_COUNT,
            "Layer 3 must have exactly {LAYER_3_COUNT} exploration attributes"
        );
    }

    // Verifies: INV-STORE-008 — Genesis Determinism (layer 3)
    #[test]
    fn layer_3_datoms_are_deterministic() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(3, 0, agent);
        let d1 = layer_3_datoms(tx);
        let d2 = layer_3_datoms(tx);
        assert_eq!(d1, d2, "Layer 3 datoms must be deterministic");
    }

    // Verifies: INV-SCHEMA-001 — Schema-as-Data (layer 3)
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
            GENESIS_ATTR_COUNT + LAYER_1_COUNT + LAYER_2_COUNT + LAYER_3_COUNT,
            "genesis({GENESIS_ATTR_COUNT}) + L1({LAYER_1_COUNT}) + L2({LAYER_2_COUNT}) + L3({LAYER_3_COUNT}) = {} attributes",
            GENESIS_ATTR_COUNT + LAYER_1_COUNT + LAYER_2_COUNT + LAYER_3_COUNT
        );
    }

    // Verifies: ADR-SCHEMA-006 — Value Type Union (layer 3)
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
            (":exploration/content-hash", ValueType::Bytes),
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

    // Verifies: INV-SCHEMA-003 — Schema Monotonicity (L3 valid evolution)
    // Verifies: NEG-SCHEMA-003 — No Circular Layer Dependencies
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

    // Verifies: INV-STORE-003 — Content-Addressable Identity (L3)
    #[test]
    fn all_layer_3_idents_are_unique() {
        let specs = layer_3_attributes();
        let mut seen = std::collections::HashSet::new();
        for spec in &specs {
            let ident = spec.ident.as_str().to_string();
            assert!(seen.insert(ident.clone()), "Duplicate L3 ident: {ident}");
        }
    }

    // Verifies: NEG-SCHEMA-003 — No Circular Layer Dependencies
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

    // Verifies: NEG-SCHEMA-003 — No Circular Layer Dependencies
    // Verifies: INV-SCHEMA-006 — Six-Layer Schema Architecture
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

    // Verifies: INV-SCHEMA-006 — Six-Layer Schema Architecture
    #[test]
    fn full_schema_datoms_combines_all_layers() {
        let agent = AgentId::from_name("braid:system");
        let tx = TxId::new(1, 0, agent);

        let full = full_schema_datoms(tx);
        let l1 = layer_1_datoms(tx);
        let l2 = layer_2_datoms(tx);
        let l3 = layer_3_datoms(tx);
        let l4 = layer_4_datoms(tx);

        assert_eq!(
            full.len(),
            l1.len() + l2.len() + l3.len() + l4.len(),
            "full_schema_datoms must combine L1, L2, L3, and L4"
        );
    }

    // Verifies: INV-SCHEMA-006 — Six-Layer Schema Architecture (full count)
    // Verifies: INV-SCHEMA-005 — Meta-Schema Self-Description
    #[test]
    fn full_schema_has_correct_total_count() {
        let agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, agent);
        let full_tx = TxId::new(1, 0, agent);

        let mut datoms: BTreeSet<Datom> = genesis_datoms(genesis_tx).into_iter().collect();
        for d in full_schema_datoms(full_tx) {
            datoms.insert(d);
        }
        let schema = Schema::from_datoms(&datoms);

        let expected =
            GENESIS_ATTR_COUNT + LAYER_1_COUNT + LAYER_2_COUNT + LAYER_3_COUNT + LAYER_4_COUNT;
        assert_eq!(
            schema.len(),
            expected,
            "Full schema (L0+L1+L2+L3) must have {expected} attributes, got {}",
            schema.len()
        );
    }

    // Verifies: NEG-SCHEMA-003 — No Circular Layer Dependencies
    // Verifies: INV-SCHEMA-006 — Six-Layer Schema Architecture
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
    // Witnesses: INV-SCHEMA-003, INV-SCHEMA-007, INV-STORE-004,
    // INV-STORE-005, INV-STORE-006, INV-STORE-007,
    // ADR-SCHEMA-005, ADR-SCHEMA-008
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

    // ===================================================================
    // Property-Based Tests (INV-SCHEMA-004, INV-SCHEMA-005, INV-SCHEMA-006)
    // ===================================================================

    mod proptests {
        use super::*;
        use crate::proptest_strategies::{arb_entity_id, arb_store, arb_tx_id};
        use proptest::prelude::*;

        // INV-SCHEMA-004: Schema validation gate.
        //
        // For every attribute with a defined value type in the genesis schema,
        // a datom bearing a mismatched value type MUST be rejected by
        // schema.validate_datom(). We generate arbitrary entity/tx IDs and
        // pair each known-type attribute with a deliberately wrong Value variant.
        proptest! {
            #[test]
            fn schema_rejects_wrong_value_types(
                entity in arb_entity_id(),
                tx in arb_tx_id(),
            ) {
                let agent = crate::datom::AgentId::from_name("braid:system");
                let genesis_tx = TxId::new(0, 0, agent);
                let datoms: BTreeSet<Datom> = genesis_datoms(genesis_tx).into_iter().collect();
                let schema = Schema::from_datoms(&datoms);

                // For each attribute in the genesis schema, pick a value of the wrong type
                // and assert that validate_datom rejects it.
                for (attr, def) in schema.attributes() {
                    let wrong_value = match def.value_type {
                        // Attribute expects String -> supply Long
                        ValueType::String => Value::Long(42),
                        // Attribute expects Keyword -> supply Boolean
                        ValueType::Keyword => Value::Boolean(true),
                        // Attribute expects Boolean -> supply String
                        ValueType::Boolean => Value::String("not-a-bool".into()),
                        // Attribute expects Long -> supply String
                        ValueType::Long => Value::String("not-a-long".into()),
                        // Attribute expects Double -> supply Long
                        ValueType::Double => Value::Long(99),
                        // Attribute expects Instant -> supply String
                        ValueType::Instant => Value::String("not-an-instant".into()),
                        // Attribute expects Uuid -> supply Long
                        ValueType::Uuid => Value::Long(0),
                        // Attribute expects Ref -> supply String
                        ValueType::Ref => Value::String("not-a-ref".into()),
                        // Attribute expects Bytes -> supply Long
                        ValueType::Bytes => Value::Long(1),
                    };

                    let bad_datom = Datom::new(
                        entity,
                        attr.clone(),
                        wrong_value,
                        tx,
                        Op::Assert,
                    );

                    let result = schema.validate_datom(&bad_datom);
                    prop_assert!(
                        result.is_err(),
                        "INV-SCHEMA-004 violated: validate_datom accepted wrong type for {}",
                        attr.as_str()
                    );

                    // Verify the error is specifically a SchemaViolation, not UnknownAttribute
                    match result.unwrap_err() {
                        StoreError::SchemaViolation { attr: err_attr, .. } => {
                            prop_assert_eq!(
                                err_attr.as_str(),
                                attr.as_str(),
                                "SchemaViolation should reference the offending attribute"
                            );
                        }
                        other => {
                            prop_assert!(
                                false,
                                "Expected SchemaViolation for {}, got: {:?}",
                                attr.as_str(),
                                other
                            );
                        }
                    }
                }
            }
        }

        // INV-SCHEMA-005: Schema query API consistency.
        //
        // For any store, calling schema.attributes() twice yields the same
        // set of attribute idents. The schema is a deterministic projection
        // of the datom set — repeated queries must return identical results.
        proptest! {
            #[test]
            fn schema_attributes_returns_consistent_results(store in arb_store(3)) {
                let schema = store.schema();

                let first: BTreeSet<String> = schema
                    .attributes()
                    .map(|(a, _)| a.as_str().to_string())
                    .collect();

                let second: BTreeSet<String> = schema
                    .attributes()
                    .map(|(a, _)| a.as_str().to_string())
                    .collect();

                prop_assert_eq!(
                    first, second,
                    "INV-SCHEMA-005 violated: schema.attributes() returned different results on consecutive calls"
                );

                // Also verify each attribute's definition is consistent between calls
                for (attr, def) in schema.attributes() {
                    let looked_up = schema.attribute(attr);
                    prop_assert!(
                        looked_up.is_some(),
                        "Attribute {} from attributes() not found via attribute()",
                        attr.as_str()
                    );
                    let looked_up = looked_up.unwrap();
                    prop_assert_eq!(
                        def.value_type, looked_up.value_type,
                        "Value type mismatch for {} between attributes() and attribute()",
                        attr.as_str()
                    );
                    prop_assert_eq!(
                        def.cardinality, looked_up.cardinality,
                        "Cardinality mismatch for {} between attributes() and attribute()",
                        attr.as_str()
                    );
                }
            }
        }

        // INV-SCHEMA-006: Schema error messages.
        //
        // Every error produced by schema validation (UnknownAttribute and
        // SchemaViolation) must have a non-empty Display string. This ensures
        // agents and humans always get actionable error descriptions.
        proptest! {
            #[test]
            fn schema_validation_errors_have_nonempty_descriptions(
                entity in arb_entity_id(),
                tx in arb_tx_id(),
            ) {
                let agent = crate::datom::AgentId::from_name("braid:system");
                let genesis_tx = TxId::new(0, 0, agent);
                let datoms: BTreeSet<Datom> = genesis_datoms(genesis_tx).into_iter().collect();
                let schema = Schema::from_datoms(&datoms);

                // Case 1: Unknown attribute error has non-empty description
                let unknown_datom = Datom::new(
                    entity,
                    Attribute::from_keyword(":nonexistent/attr"),
                    Value::String("test".into()),
                    tx,
                    Op::Assert,
                );
                let err = schema.validate_datom(&unknown_datom).unwrap_err();
                let msg = format!("{err}");
                prop_assert!(
                    !msg.is_empty(),
                    "INV-SCHEMA-006 violated: UnknownAttribute error has empty description"
                );
                prop_assert!(
                    msg.contains("nonexistent"),
                    "UnknownAttribute error should mention the attribute name, got: {msg}"
                );

                // Case 2: SchemaViolation error has non-empty description
                // Use :db/doc (expects String) with a Long value
                let type_mismatch_datom = Datom::new(
                    entity,
                    Attribute::from_keyword(":db/doc"),
                    Value::Long(42),
                    tx,
                    Op::Assert,
                );
                let err = schema.validate_datom(&type_mismatch_datom).unwrap_err();
                let msg = format!("{err}");
                prop_assert!(
                    !msg.is_empty(),
                    "INV-SCHEMA-006 violated: SchemaViolation error has empty description"
                );
                prop_assert!(
                    msg.contains("db/doc") || msg.contains(":db/doc"),
                    "SchemaViolation error should mention the attribute, got: {msg}"
                );
                prop_assert!(
                    msg.contains("expected") || msg.contains("got"),
                    "SchemaViolation error should describe the type mismatch, got: {msg}"
                );
            }
        }
    }
}
