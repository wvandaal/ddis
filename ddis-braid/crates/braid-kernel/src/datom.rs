//! Core data types for the Braid datom store.
//!
//! The datom is the atomic unit of information: a five-tuple `[entity, attribute,
//! value, tx, op]`. All types here are immutable after construction and
//! content-addressed where applicable.
//!
//! # Invariants
//!
//! - **INV-STORE-001**: Append-only immutability — datoms are never mutated.
//! - **INV-STORE-003**: Content-addressable identity — EntityId = BLAKE3(content).
//! - **INV-STORE-011**: HLC monotonicity — TxId ordering respects causality.
//!
//! # Design Decisions
//!
//! - ADR-STORE-002: EAV data model — datom = [entity, attribute, value, tx, op].
//! - ADR-STORE-003: Content-addressable entity IDs via BLAKE3.
//! - ADR-STORE-004: Hybrid logical clocks for transaction IDs.
//! - ADR-STORE-008: Provenance typing lattice (Observed > Derived > Inferred > Hypothesized).
//! - ADR-STORE-013: BLAKE3 for content hashing.
//! - ADR-STORE-014: Private EntityId inner field.
//! - ADR-STORE-020: Agent entity identification via AgentId.

use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};

use crate::error::SchemaError;

// ---------------------------------------------------------------------------
// EntityId
// ---------------------------------------------------------------------------

/// Content-addressed entity identifier.
///
/// BLAKE3 hash of semantic content. No public constructor from raw bytes —
/// only through content hashing. This ensures INV-STORE-003 (content-addressable
/// identity) by construction.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct EntityId([u8; 32]);

impl EntityId {
    /// The all-zeros entity ID, used as a sentinel/placeholder value.
    ///
    /// This is NOT a valid content-addressed identity -- it exists only for
    /// cases where a real EntityId is not yet known (e.g., `ResolutionMode::Lattice`
    /// parsed from a keyword before the `:db/latticeOrder` ref is resolved).
    pub const ZERO: EntityId = EntityId([0u8; 32]);

    /// Create an entity ID from arbitrary content bytes.
    ///
    /// `EntityId = BLAKE3(content)`. Deterministic: same bytes → same ID.
    pub fn from_content(content: &[u8]) -> Self {
        let hash = blake3::hash(content);
        EntityId(*hash.as_bytes())
    }

    /// Create an entity ID from a keyword identifier.
    ///
    /// Used for the 18 axiomatic schema attributes whose identity IS their keyword.
    /// `EntityId = BLAKE3(keyword_bytes)`.
    pub fn from_ident(keyword: &str) -> Self {
        Self::from_content(keyword.as_bytes())
    }

    /// Access the raw 32-byte hash.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Create an entity ID from raw bytes (for deserialization only).
    ///
    /// This bypasses the content-hash guarantee. Only use for deserializing
    /// data that was previously serialized from a valid EntityId.
    pub(crate) fn from_raw_bytes(bytes: [u8; 32]) -> Self {
        EntityId(bytes)
    }
}

// ---------------------------------------------------------------------------
// Attribute
// ---------------------------------------------------------------------------

/// A namespaced keyword attribute (e.g., `:db/ident`, `:spec/type`).
///
/// Must start with `:` and contain exactly one `/` separating namespace from name.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Attribute(String);

impl Attribute {
    /// Create a new attribute from a keyword string.
    ///
    /// # Errors
    ///
    /// Returns `SchemaError::InvalidAttribute` if the keyword does not match
    /// the pattern `:namespace/name`.
    pub fn new(keyword: &str) -> Result<Self, SchemaError> {
        Self::validate(keyword)?;
        Ok(Attribute(keyword.to_string()))
    }

    /// Create an attribute at compile time for bootstrap constants.
    ///
    /// # Panics
    ///
    /// Panics if the keyword format is invalid. Only use for known-good constants.
    pub fn from_keyword(keyword: &str) -> Self {
        Self::validate(keyword)
            .unwrap_or_else(|e| panic!("invalid bootstrap keyword '{keyword}': {e}"));
        Attribute(keyword.to_string())
    }

    /// The namespace portion (before the `/`).
    pub fn namespace(&self) -> &str {
        let s = &self.0[1..]; // skip leading ':'
        let slash = s.find('/').expect("validated on construction");
        &s[..slash]
    }

    /// The name portion (after the `/`).
    pub fn name(&self) -> &str {
        let s = &self.0[1..]; // skip leading ':'
        let slash = s.find('/').expect("validated on construction");
        &s[slash + 1..]
    }

    /// The full keyword string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(keyword: &str) -> Result<(), SchemaError> {
        if !keyword.starts_with(':') {
            return Err(SchemaError::InvalidAttribute(format!(
                "must start with ':', got '{keyword}'"
            )));
        }
        // EDN keywords must be ASCII — non-ASCII chars corrupt the store on serialization
        if !keyword.is_ascii() {
            return Err(SchemaError::InvalidAttribute(format!(
                "must be ASCII only, got non-ASCII in '{keyword}'"
            )));
        }
        let body = &keyword[1..];
        let slash_count = body.chars().filter(|c| *c == '/').count();
        if slash_count != 1 {
            return Err(SchemaError::InvalidAttribute(format!(
                "must contain exactly one '/' in body, got {slash_count} in '{keyword}'"
            )));
        }
        let slash_pos = body.find('/').unwrap();
        if slash_pos == 0 || slash_pos == body.len() - 1 {
            return Err(SchemaError::InvalidAttribute(format!(
                "namespace and name must be non-empty in '{keyword}'"
            )));
        }
        Ok(())
    }
}

impl std::fmt::Display for Attribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Value
// ---------------------------------------------------------------------------

/// The value domain for datom values.
///
/// Stage 0 implements 9 of the 14 spec-defined types. BigInt, BigDec, Tuple,
/// Json, and URI are deferred to later stages.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Value {
    /// UTF-8 string.
    String(String),
    /// Keyword (e.g., `:db/ident`).
    Keyword(String),
    /// Boolean.
    Boolean(bool),
    /// 64-bit signed integer.
    Long(i64),
    /// 64-bit IEEE 754 float with total ordering (via `OrderedFloat`).
    Double(OrderedFloat<f64>),
    /// Milliseconds since Unix epoch.
    Instant(u64),
    /// 128-bit UUID.
    Uuid([u8; 16]),
    /// Reference to another entity.
    Ref(EntityId),
    /// Opaque byte array.
    Bytes(Vec<u8>),
}

impl Value {
    /// Returns the type name of this value for error reporting.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::String(_) => "string",
            Value::Keyword(_) => "keyword",
            Value::Boolean(_) => "boolean",
            Value::Long(_) => "long",
            Value::Double(_) => "double",
            Value::Instant(_) => "instant",
            Value::Uuid(_) => "uuid",
            Value::Ref(_) => "ref",
            Value::Bytes(_) => "bytes",
        }
    }
}

// ---------------------------------------------------------------------------
// TxId — Hybrid Logical Clock
// ---------------------------------------------------------------------------

/// Transaction identifier using a Hybrid Logical Clock (HLC).
///
/// Provides a total order on transactions that respects causality (INV-STORE-011).
/// Ordering: wall_time → logical → agent (lexicographic).
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct TxId {
    /// Milliseconds since Unix epoch.
    pub wall_time: u64,
    /// Logical counter for same-millisecond ordering.
    pub logical: u32,
    /// The agent that created this transaction.
    pub agent: AgentId,
}

impl TxId {
    /// Create a new TxId.
    pub fn new(wall_time: u64, logical: u32, agent: AgentId) -> Self {
        TxId {
            wall_time,
            logical,
            agent,
        }
    }

    /// Wall time (milliseconds since Unix epoch).
    pub fn wall_time(&self) -> u64 {
        self.wall_time
    }

    /// Logical counter for same-millisecond ordering.
    pub fn logical(&self) -> u32 {
        self.logical
    }

    /// The agent that created this transaction.
    pub fn agent(&self) -> AgentId {
        self.agent
    }

    /// Tick the clock: advance to at least `now`, preserving monotonicity.
    ///
    /// If `now > self.wall_time`, resets logical to 0.
    /// If `now == self.wall_time`, increments logical.
    /// If `now < self.wall_time` (clock regression), keeps wall_time and increments logical.
    pub fn tick(&self, now: u64, agent: AgentId) -> Self {
        if now > self.wall_time {
            TxId {
                wall_time: now,
                logical: 0,
                agent,
            }
        } else if now == self.wall_time {
            TxId {
                wall_time: now,
                logical: self.logical + 1,
                agent,
            }
        } else {
            // Clock regression — maintain monotonicity
            TxId {
                wall_time: self.wall_time,
                logical: self.logical + 1,
                agent,
            }
        }
    }

    /// Merge with a remote TxId: take the max, then tick.
    ///
    /// Used during store merge to advance the clock past both local and remote frontiers.
    pub fn merge(&self, remote: &TxId, now: u64, agent: AgentId) -> Self {
        let max_wall = self.wall_time.max(remote.wall_time).max(now);
        let max_logical = if max_wall == self.wall_time && max_wall == remote.wall_time {
            self.logical.max(remote.logical) + 1
        } else if max_wall == self.wall_time {
            self.logical + 1
        } else if max_wall == remote.wall_time {
            remote.logical + 1
        } else {
            0
        };
        TxId {
            wall_time: max_wall,
            logical: max_logical,
            agent,
        }
    }
}

// ---------------------------------------------------------------------------
// AgentId
// ---------------------------------------------------------------------------

/// Agent identifier — UUID or hash of agent name.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct AgentId([u8; 16]);

impl AgentId {
    /// Create an AgentId from a name string (BLAKE3-truncated to 16 bytes).
    pub fn from_name(name: &str) -> Self {
        let hash = blake3::hash(name.as_bytes());
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&hash.as_bytes()[..16]);
        AgentId(bytes)
    }

    /// Create an AgentId from raw UUID bytes.
    pub fn from_uuid(uuid: [u8; 16]) -> Self {
        AgentId(uuid)
    }

    /// Create an AgentId from raw bytes (for deserialization).
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        AgentId(bytes)
    }

    /// Access the raw 16-byte identifier.
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Op
// ---------------------------------------------------------------------------

/// Datom operation: assert a fact or retract a fact.
///
/// Retractions are themselves datoms (INV-STORE-001). The store never deletes;
/// it records that a fact was retracted.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Op {
    /// Assert this fact as true.
    Assert,
    /// Retract a previously asserted fact.
    Retract,
}

// ---------------------------------------------------------------------------
// ProvenanceType
// ---------------------------------------------------------------------------

/// Provenance typing lattice for transaction metadata.
///
/// Forms a total order: Hypothesized < Inferred < Derived < Observed.
/// Each level has an associated confidence weight.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum ProvenanceType {
    /// Speculative — confidence 0.2.
    Hypothesized,
    /// Logically derived from other facts — confidence 0.5.
    Inferred,
    /// Computed from observation — confidence 0.8.
    Derived,
    /// Directly witnessed — confidence 1.0.
    Observed,
}

impl ProvenanceType {
    /// The confidence weight for this provenance level.
    pub fn confidence(&self) -> f64 {
        match self {
            ProvenanceType::Hypothesized => 0.2,
            ProvenanceType::Inferred => 0.5,
            ProvenanceType::Derived => 0.8,
            ProvenanceType::Observed => 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Datom
// ---------------------------------------------------------------------------

/// The atomic unit of information in Braid.
///
/// A five-tuple `[entity, attribute, value, tx, op]`. Content-addressed:
/// identity is the hash of all five fields. Immutable after construction.
///
/// # Invariants
///
/// - **INV-STORE-001**: Never mutated after creation.
/// - **INV-STORE-003**: `Hash`/`Eq` derived from all 5 fields (content identity).
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Datom {
    /// The entity this datom describes.
    pub entity: EntityId,
    /// The attribute being asserted or retracted.
    pub attribute: Attribute,
    /// The value of the attribute.
    pub value: Value,
    /// The transaction that produced this datom.
    pub tx: TxId,
    /// Assert or retract.
    pub op: Op,
}

impl Datom {
    /// Create a new datom.
    pub fn new(entity: EntityId, attribute: Attribute, value: Value, tx: TxId, op: Op) -> Self {
        Datom {
            entity,
            attribute,
            value,
            tx,
            op,
        }
    }

    /// Compute the content hash of this datom (BLAKE3 of serialized form).
    pub fn content_hash(&self) -> [u8; 32] {
        let bytes = serde_json::to_vec(self).expect("datom serialization cannot fail");
        *blake3::hash(&bytes).as_bytes()
    }
}

/// Find the latest Assert datom for a given attribute in a datom slice.
///
/// Uses max-by-tx semantics: the Assert with the highest (wall_time, logical)
/// wins. This is the correct LWW read — NOT `.rfind()` or `.rev().find()`,
/// which use BTreeSet ordering (by value, not by tx) and return wrong results
/// when a newer tx writes a smaller value.
///
/// Use this for any LWW attribute read from `entity_datoms()` or similar.
pub fn latest_assert<'a>(datoms: &[&'a Datom], attr: &Attribute) -> Option<&'a Datom> {
    datoms
        .iter()
        .filter(|d| d.attribute == *attr && d.op == Op::Assert)
        .max_by_key(|d| (d.tx.wall_time(), d.tx.logical()))
        .copied()
}

/// Same as [`latest_assert`] but for owned datom slices (`&[Datom]`).
pub fn latest_assert_owned<'a>(datoms: &'a [Datom], attr: &Attribute) -> Option<&'a Datom> {
    datoms
        .iter()
        .filter(|d| d.attribute == *attr && d.op == Op::Assert)
        .max_by_key(|d| (d.tx.wall_time(), d.tx.logical()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// Witnesses: INV-STORE-003, INV-STORE-011,
// ADR-STORE-002, ADR-STORE-003, ADR-STORE-004, ADR-STORE-008, ADR-STORE-013,
// ADR-STORE-014, ADR-STORE-020,
// NEG-STORE-003
#[cfg(test)]
mod tests {
    use super::*;

    // Verifies: INV-STORE-003 — Content-Addressable Identity
    // Verifies: ADR-STORE-003 — Content-Addressable Entity IDs
    // Verifies: ADR-STORE-013 — BLAKE3 for Content Hashing
    #[test]
    fn entity_id_from_content_is_deterministic() {
        let a = EntityId::from_content(b"hello");
        let b = EntityId::from_content(b"hello");
        assert_eq!(a, b);
    }

    // Verifies: INV-STORE-003 — Content-Addressable Identity (uniqueness)
    // Verifies: NEG-STORE-003 — No Sequential ID Assignment
    #[test]
    fn entity_id_from_content_differs_for_different_inputs() {
        let a = EntityId::from_content(b"hello");
        let b = EntityId::from_content(b"world");
        assert_ne!(a, b);
    }

    // Verifies: INV-STORE-003 — Content-Addressable Identity
    // Verifies: ADR-STORE-014 — Private EntityId Inner Field
    #[test]
    fn entity_id_from_ident_matches_content() {
        let a = EntityId::from_ident(":db/ident");
        let b = EntityId::from_content(b":db/ident");
        assert_eq!(a, b);
    }

    // Verifies: ADR-STORE-002 — EAV Over Relational (keyword attribute format)
    #[test]
    fn attribute_valid_keyword() {
        let attr = Attribute::new(":db/ident").unwrap();
        assert_eq!(attr.namespace(), "db");
        assert_eq!(attr.name(), "ident");
        assert_eq!(attr.as_str(), ":db/ident");
    }

    #[test]
    fn attribute_rejects_no_colon() {
        assert!(Attribute::new("db/ident").is_err());
    }

    #[test]
    fn attribute_rejects_no_slash() {
        assert!(Attribute::new(":dbident").is_err());
    }

    #[test]
    fn attribute_rejects_double_slash() {
        assert!(Attribute::new(":db/foo/bar").is_err());
    }

    #[test]
    fn attribute_rejects_empty_namespace() {
        assert!(Attribute::new(":/name").is_err());
    }

    #[test]
    fn attribute_rejects_empty_name() {
        assert!(Attribute::new(":ns/").is_err());
    }

    #[test]
    fn value_type_name() {
        assert_eq!(Value::Long(42).type_name(), "long");
        assert_eq!(Value::String("hi".into()).type_name(), "string");
        assert_eq!(Value::Ref(EntityId::from_content(b"x")).type_name(), "ref");
    }

    #[test]
    fn op_ordering() {
        assert!(Op::Assert < Op::Retract);
    }

    #[test]
    fn provenance_ordering() {
        assert!(ProvenanceType::Hypothesized < ProvenanceType::Inferred);
        assert!(ProvenanceType::Inferred < ProvenanceType::Derived);
        assert!(ProvenanceType::Derived < ProvenanceType::Observed);
    }

    #[test]
    fn provenance_confidence() {
        assert!((ProvenanceType::Hypothesized.confidence() - 0.2).abs() < f64::EPSILON);
        assert!((ProvenanceType::Observed.confidence() - 1.0).abs() < f64::EPSILON);
    }

    // Verifies: INV-STORE-011 — HLC Monotonicity
    // Verifies: ADR-STORE-004 — Hybrid Logical Clocks for Transaction IDs
    #[test]
    fn txid_tick_advances_wall_time() {
        let agent = AgentId::from_name("test");
        let t0 = TxId::new(100, 0, agent);
        let t1 = t0.tick(200, agent);
        assert_eq!(t1.wall_time, 200);
        assert_eq!(t1.logical, 0);
    }

    // Verifies: INV-STORE-011 — HLC Monotonicity (logical increment)
    #[test]
    fn txid_tick_same_wall_time_increments_logical() {
        let agent = AgentId::from_name("test");
        let t0 = TxId::new(100, 0, agent);
        let t1 = t0.tick(100, agent);
        assert_eq!(t1.wall_time, 100);
        assert_eq!(t1.logical, 1);
    }

    // Verifies: INV-STORE-011 — HLC Monotonicity (clock regression safety)
    #[test]
    fn txid_tick_clock_regression() {
        let agent = AgentId::from_name("test");
        let t0 = TxId::new(200, 5, agent);
        let t1 = t0.tick(100, agent);
        // Should keep wall_time at 200 (monotonicity) and increment logical
        assert_eq!(t1.wall_time, 200);
        assert_eq!(t1.logical, 6);
    }

    // Verifies: INV-STORE-011 — HLC Monotonicity (merge takes max)
    #[test]
    fn txid_merge_takes_max() {
        let a1 = AgentId::from_name("alice");
        let a2 = AgentId::from_name("bob");
        let local = TxId::new(100, 3, a1);
        let remote = TxId::new(200, 1, a2);
        let merged = local.merge(&remote, 150, a1);
        assert_eq!(merged.wall_time, 200);
        assert_eq!(merged.logical, 2); // remote.logical + 1
    }

    // Verifies: INV-STORE-003 — Content-Addressable Identity (datom hash)
    // Verifies: ADR-STORE-013 — BLAKE3 for Content Hashing
    #[test]
    fn datom_content_hash_is_deterministic() {
        let agent = AgentId::from_name("test");
        let tx = TxId::new(100, 0, agent);
        let d1 = Datom::new(
            EntityId::from_ident(":db/ident"),
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":db/ident".into()),
            tx,
            Op::Assert,
        );
        let d2 = d1.clone();
        assert_eq!(d1.content_hash(), d2.content_hash());
    }

    #[test]
    fn datom_eq_uses_all_five_fields() {
        let agent = AgentId::from_name("test");
        let tx1 = TxId::new(100, 0, agent);
        let tx2 = TxId::new(200, 0, agent);
        let d1 = Datom::new(
            EntityId::from_ident(":db/ident"),
            Attribute::from_keyword(":db/ident"),
            Value::Long(1),
            tx1,
            Op::Assert,
        );
        let d2 = Datom::new(
            EntityId::from_ident(":db/ident"),
            Attribute::from_keyword(":db/ident"),
            Value::Long(1),
            tx2, // different tx
            Op::Assert,
        );
        assert_ne!(d1, d2); // different tx → different datom
    }

    #[test]
    fn agent_id_from_name_is_deterministic() {
        let a1 = AgentId::from_name("alice");
        let a2 = AgentId::from_name("alice");
        assert_eq!(a1, a2);
    }

    #[test]
    fn agent_id_differs_for_different_names() {
        let a1 = AgentId::from_name("alice");
        let a2 = AgentId::from_name("bob");
        assert_ne!(a1, a2);
    }

    // ---- Canonical serialization proptests (C2, INV-STORE-003) ----

    mod datom_proptests {
        use super::*;
        use proptest::prelude::*;

        fn arb_entity_id() -> impl Strategy<Value = EntityId> {
            prop::string::string_regex("[a-z]{3,10}")
                .unwrap()
                .prop_map(|s| EntityId::from_ident(&format!(":test/{s}")))
        }

        fn arb_attribute() -> impl Strategy<Value = Attribute> {
            prop::string::string_regex("[a-z]{3,10}")
                .unwrap()
                .prop_map(|s| Attribute::from_keyword(&format!(":test/{s}")))
        }

        fn arb_value() -> impl Strategy<Value = Value> {
            prop_oneof![
                any::<i64>().prop_map(Value::Long),
                prop::string::string_regex("[a-zA-Z0-9 ]{0,50}")
                    .unwrap()
                    .prop_map(Value::String),
                Just(Value::Boolean(true)),
                Just(Value::Boolean(false)),
            ]
        }

        fn arb_datom() -> impl Strategy<Value = Datom> {
            (arb_entity_id(), arb_attribute(), arb_value(), 1u64..1000u64).prop_map(
                |(e, a, v, time)| {
                    let agent = AgentId::from_name("proptest");
                    let tx = TxId::new(time, 0, agent);
                    Datom::new(e, a, v, tx, Op::Assert)
                },
            )
        }

        proptest! {
            // Verifies: C2 (Identity by content) — same datom content produces
            // identical hash regardless of construction order.
            #[test]
            fn content_hash_deterministic(d in arb_datom()) {
                let h1 = d.content_hash();
                let clone = d.clone();
                let h2 = clone.content_hash();
                prop_assert_eq!(h1, h2, "same datom must produce same hash");
            }

            // Verifies: INV-STORE-003 — Two datoms with same [e,a,v,tx,op]
            // produce the same content hash.
            #[test]
            fn canonical_serialization_order_independent(
                e in arb_entity_id(),
                a in arb_attribute(),
                v in arb_value(),
                time in 1u64..1000u64,
            ) {
                let agent = AgentId::from_name("proptest");
                let tx = TxId::new(time, 0, agent);
                let d1 = Datom::new(e, a.clone(), v.clone(), tx, Op::Assert);
                let d2 = Datom::new(e, a, v, tx, Op::Assert);
                prop_assert_eq!(
                    d1.content_hash(),
                    d2.content_hash(),
                    "identical content → identical hash (C2)"
                );
            }

            // Verifies: C2 — Different content must produce different hash
            // (collision resistance).
            #[test]
            fn different_content_different_hash(
                d1 in arb_datom(),
                d2 in arb_datom(),
            ) {
                if d1 != d2 {
                    // Different datoms SHOULD have different hashes (probabilistic).
                    // With BLAKE3 on structured data, collisions are astronomically unlikely.
                    prop_assert_ne!(
                        d1.content_hash(),
                        d2.content_hash(),
                        "distinct datoms should have distinct hashes"
                    );
                }
            }
        }
    }
}
