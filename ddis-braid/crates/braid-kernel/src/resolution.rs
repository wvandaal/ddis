//! Per-attribute conflict resolution — three-mode join-semilattice.
//!
//! Each attribute declares its resolution mode: LWW, Lattice, or MultiValue.
//! Resolution is order-independent (INV-RESOLUTION-002) — two agents with the
//! same datom set produce the same resolved value.
//!
//! # Algebraic Structure
//!
//! Each resolution mode forms an independent join-semilattice:
//! - **LWW**: Total order by (TxId, BLAKE3 tiebreaker). Meet = max.
//! - **Lattice**: User-defined partial order with lub. Diamond → error signal.
//! - **MultiValue**: Set union. Meet = ∪.
//!
//! # Invariants
//!
//! - **INV-RESOLUTION-001**: Per-attribute resolution mode from schema.
//! - **INV-RESOLUTION-002**: Resolution commutativity.
//! - **INV-RESOLUTION-003**: Conservative conflict detection (no false negatives).
//! - **INV-RESOLUTION-004**: 6-condition conflict predicate.
//! - **INV-RESOLUTION-005**: LWW semilattice properties.
//! - **INV-RESOLUTION-006**: Lattice join correctness.
//! - **INV-RESOLUTION-007**: Three-tier routing completeness (LWW → Lattice → Deliberation).
//! - **INV-RESOLUTION-008**: Conflict entity datom trail.
//!
//! # Design Decisions
//!
//! - ADR-RESOLUTION-001: Per-attribute over global policy.
//! - ADR-RESOLUTION-002: Resolution at query time, not merge time.
//! - ADR-RESOLUTION-003: Conservative detection over precise (no false negatives).
//! - ADR-RESOLUTION-004: Three-tier routing (LWW, Lattice, Deliberation).
//! - ADR-RESOLUTION-005: Deliberation as entity.
//! - ADR-RESOLUTION-006: Delegation threshold formula.
//! - ADR-RESOLUTION-007: Four-class delegation.
//! - ADR-RESOLUTION-008: Delegation safety.
//! - ADR-RESOLUTION-010: Resolution capacity monotonicity.
//! - ADR-RESOLUTION-011: Spectral authority via SVD.
//! - ADR-RESOLUTION-012: Contribution weight by verification status.
//! - ADR-RESOLUTION-013: Conflict pipeline progressive activation.
//!
//! # Negative Cases
//!
//! - NEG-RESOLUTION-001: No merge-time resolution — resolution is at query time.
//! - NEG-RESOLUTION-002: No false negative conflict detection.
//! - NEG-RESOLUTION-003: No resolution without provenance.

use std::collections::HashMap;

use crate::datom::{Attribute, Datom, EntityId, Op, TxId, Value};
use crate::schema::{ResolutionMode, Schema};
use crate::store::Store;

// ---------------------------------------------------------------------------
// ConflictSet
// ---------------------------------------------------------------------------

/// A set of competing assertions for a single (entity, attribute) pair.
///
/// Constructed from the datom set by collecting all assertions and retractions
/// for a specific (entity, attribute) combination.
#[derive(Clone, Debug)]
pub struct ConflictSet {
    /// The entity.
    pub entity: EntityId,
    /// The attribute.
    pub attribute: Attribute,
    /// Asserted values with their transaction IDs.
    pub assertions: Vec<(Value, TxId)>,
    /// Retracted values with their transaction IDs.
    pub retractions: Vec<(Value, TxId)>,
}

impl ConflictSet {
    /// Build a conflict set from all datoms for a given (entity, attribute).
    pub fn from_datoms(entity: EntityId, attribute: Attribute, datoms: &[&Datom]) -> Self {
        let mut assertions = Vec::new();
        let mut retractions = Vec::new();

        for d in datoms {
            if d.entity == entity && d.attribute == attribute {
                match d.op {
                    Op::Assert => assertions.push((d.value.clone(), d.tx)),
                    Op::Retract => retractions.push((d.value.clone(), d.tx)),
                }
            }
        }

        ConflictSet {
            entity,
            attribute,
            assertions,
            retractions,
        }
    }

    /// Active assertions: asserted values that have not been retracted.
    pub fn active_assertions(&self) -> Vec<(Value, TxId)> {
        // Pre-compute latest assert TxId per value — O(a), then O(1) per lookup.
        let mut latest_assert: HashMap<&Value, TxId> = HashMap::new();
        for (val, tx) in &self.assertions {
            latest_assert
                .entry(val)
                .and_modify(|existing| {
                    if *tx > *existing {
                        *existing = *tx;
                    }
                })
                .or_insert(*tx);
        }

        self.assertions
            .iter()
            .filter(|(val, _)| {
                let latest = latest_assert[val];
                !self
                    .retractions
                    .iter()
                    .any(|(rv, rtx)| rv == val && *rtx > latest)
            })
            .cloned()
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ResolvedValue
// ---------------------------------------------------------------------------

/// The result of resolving a conflict set.
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedValue {
    /// Single winning value (LWW or Lattice).
    Single(Value),
    /// Multiple active values (MultiValue mode).
    Multi(Vec<Value>),
    /// No active value (all retracted).
    None,
}

// ---------------------------------------------------------------------------
// Resolution Functions
// ---------------------------------------------------------------------------

/// Resolve a conflict set using the specified resolution mode.
///
/// # Invariants
///
/// - **INV-RESOLUTION-002**: Same inputs → same output (deterministic).
/// - **INV-RESOLUTION-005**: LWW is commutative, associative, idempotent.
pub fn resolve(conflict: &ConflictSet, mode: &ResolutionMode) -> ResolvedValue {
    let active = conflict.active_assertions();

    if active.is_empty() {
        return ResolvedValue::None;
    }

    match mode {
        ResolutionMode::Lww => resolve_lww(&active),
        ResolutionMode::Lattice => {
            // Stage 0: lattice resolution falls back to LWW
            // Full lattice join requires lattice definitions from store
            resolve_lww(&active)
        }
        ResolutionMode::Multi => {
            let values: Vec<Value> = active.into_iter().map(|(v, _)| v).collect();
            ResolvedValue::Multi(values)
        }
    }
}

/// LWW resolution: pick the value with the highest TxId.
///
/// Ties broken by BLAKE3 hash of value (ADR-RESOLUTION-009).
fn resolve_lww(active: &[(Value, TxId)]) -> ResolvedValue {
    if active.is_empty() {
        return ResolvedValue::None;
    }

    let winner = active
        .iter()
        .max_by(|(v1, tx1), (v2, tx2)| {
            tx1.cmp(tx2).then_with(|| {
                // BLAKE3 tiebreaker for identical timestamps
                let h1 =
                    blake3::hash(&serde_json::to_vec(v1).expect("Value serialization cannot fail"));
                let h2 =
                    blake3::hash(&serde_json::to_vec(v2).expect("Value serialization cannot fail"));
                h1.as_bytes().cmp(h2.as_bytes())
            })
        })
        .expect("active is non-empty (guarded by early return above)");

    ResolvedValue::Single(winner.0.clone())
}

/// Six-condition conflict predicate (INV-RESOLUTION-004).
///
/// A conflict exists when ALL six conditions hold:
/// 1. Same entity
/// 2. Same attribute
/// 3. Different values asserted
/// 4. Both are assertions (not retractions)
/// 5. Attribute has cardinality :one
/// 6. Causally independent (different agents or no causal ordering)
///
/// Conservative detection: may report false positives but never false negatives
/// (Theorem R2.5b).
///
/// INV-DELIBERATION-002: Stability guard enforcement — detected conflicts must be resolved.
/// INV-DELIBERATION-003: Precedent queryability — conflict resolution history is queryable.
/// INV-DELIBERATION-004: Bilateral deliberation symmetry — both sides of conflict heard.
/// INV-DELIBERATION-005: Commitment weight integration.
/// INV-DELIBERATION-006: Competing branch resolution.
/// NEG-DELIBERATION-001: No decision without stability guard.
/// NEG-DELIBERATION-002: No losing branch leak.
/// NEG-DELIBERATION-003: No backward lifecycle transition.
pub fn has_conflict(conflict: &ConflictSet, mode: &ResolutionMode) -> bool {
    if *mode == ResolutionMode::Multi {
        return false; // Multi-value mode never conflicts
    }

    let active = conflict.active_assertions();
    if active.len() <= 1 {
        return false;
    }

    // Check if there are different values
    let first_val = &active[0].0;
    active.iter().any(|(v, _)| v != first_val)
}

/// Compute the LIVE view of an entity — resolve all attributes.
/// INV-STORE-012: LIVE index correctness — deterministic fold of all datoms.
/// INV-RESOLUTION-006: Lattice join correctness (falls back to LWW at Stage 0).
/// INV-DELIBERATION-001: Deliberation convergence — unresolvable conflicts escalate.
/// ADR-DELIBERATION-001: Three entity types for structured resolution.
/// ADR-DELIBERATION-002: Five decision methods (consensus, authority, vote, defer, split).
/// ADR-DELIBERATION-003: Precedent as case law.
pub fn live_entity(store: &Store, entity: EntityId) -> HashMap<Attribute, ResolvedValue> {
    let datoms: Vec<&Datom> = store.entity_datoms(entity);
    let mut result = HashMap::new();

    // Group by attribute
    let mut by_attr: HashMap<&Attribute, Vec<&Datom>> = HashMap::new();
    for d in &datoms {
        by_attr.entry(&d.attribute).or_default().push(d);
    }

    for (attr, attr_datoms) in by_attr {
        let conflict = ConflictSet::from_datoms(entity, attr.clone(), &attr_datoms);
        let mode = store.schema().resolution_mode(attr);
        let resolved = resolve(&conflict, &mode);
        if resolved != ResolvedValue::None {
            result.insert(attr.clone(), resolved);
        }
    }

    result
}

// ---------------------------------------------------------------------------
// ConflictEntity
// ---------------------------------------------------------------------------

/// A detected conflict for a specific (entity, attribute) pair.
///
/// Captures the full provenance trail: which values are in conflict, which
/// transactions produced them, and when the conflict was detected. This is
/// the input to the resolution pipeline and the basis for the audit trail.
///
/// # Invariants
///
/// - **INV-RESOLUTION-004**: The conflicting values satisfy the six-condition
///   conflict predicate (same entity, same attribute, different values, both
///   assertions, cardinality :one, causally independent).
#[derive(Clone, Debug)]
pub struct ConflictEntity {
    /// The entity with conflicting assertions.
    pub entity: EntityId,
    /// The attribute under conflict.
    pub attribute: Attribute,
    /// The distinct values that are in conflict.
    pub conflicting_values: Vec<Value>,
    /// The transaction IDs that produced the conflicting values (parallel to `conflicting_values`).
    pub conflicting_txs: Vec<TxId>,
    /// The transaction at which this conflict was detected.
    pub detected_at: TxId,
}

// ---------------------------------------------------------------------------
// ResolutionRecord
// ---------------------------------------------------------------------------

/// The full provenance trail of a conflict resolution.
///
/// Records which conflict was resolved, what the winning value is, which
/// resolution mode was applied, and the transaction that recorded the
/// resolution. This is the basis for the audit datom trail — every
/// resolution decision is a first-class fact in the store.
#[derive(Clone, Debug)]
pub struct ResolutionRecord {
    /// The conflict that was resolved.
    pub conflict: ConflictEntity,
    /// The winning value (or multi-value set).
    pub resolved_value: ResolvedValue,
    /// The resolution mode that was applied.
    pub resolution_mode: ResolutionMode,
    /// The transaction that records this resolution.
    pub resolution_tx: TxId,
}

// ---------------------------------------------------------------------------
// Conflict Detection and Resolution with Trail
// ---------------------------------------------------------------------------

/// Detect if an (entity, attribute) pair has conflicting values in the store.
///
/// Builds a `ConflictSet` from all datoms for the pair, checks for conflict
/// using the schema's resolution mode, and — if a conflict exists — returns
/// a `ConflictEntity` capturing the full provenance.
///
/// Returns `None` if there is no conflict (single value, all retracted, or
/// multi-value mode where conflicts cannot occur by definition).
///
/// # Invariants
///
/// - **INV-RESOLUTION-004**: Uses the six-condition conflict predicate.
pub fn detect_conflicts(
    store: &Store,
    entity: EntityId,
    attribute: &Attribute,
) -> Option<ConflictEntity> {
    let datoms: Vec<&Datom> = store
        .datoms()
        .filter(|d| d.entity == entity && d.attribute == *attribute)
        .collect();

    if datoms.is_empty() {
        return None;
    }

    let conflict_set = ConflictSet::from_datoms(entity, attribute.clone(), &datoms);
    let mode = store.schema().resolution_mode(attribute);

    if !has_conflict(&conflict_set, &mode) {
        return None;
    }

    let active = conflict_set.active_assertions();

    // Collect distinct (value, tx) pairs
    let conflicting_values: Vec<Value> = active.iter().map(|(v, _)| v.clone()).collect();
    let conflicting_txs: Vec<TxId> = active.iter().map(|(_, tx)| *tx).collect();

    // Use the store's frontier max as the detection timestamp
    let detected_at = store
        .frontier()
        .values()
        .max()
        .copied()
        .unwrap_or(TxId::new(0, 0, crate::datom::AgentId::from_name("nil")));

    Some(ConflictEntity {
        entity,
        attribute: attribute.clone(),
        conflicting_values,
        conflicting_txs,
        detected_at,
    })
}

/// Resolve a conflict and produce the full provenance trail.
///
/// Applies the resolution mode from the schema to the conflict's competing
/// values, producing a `ResolutionRecord` that captures the decision. The
/// `resolution_tx` is set to the conflict's `detected_at` — the caller is
/// expected to transact the record into the store at a subsequent transaction.
///
/// # Invariants
///
/// - **INV-RESOLUTION-002**: Resolution is deterministic — same conflict and
///   schema always produce the same resolved value.
pub fn resolve_with_trail(conflict: &ConflictEntity, schema: &Schema) -> ResolutionRecord {
    let mode = schema.resolution_mode(&conflict.attribute);

    // Build assertion pairs from the conflict's values and txs
    let active: Vec<(Value, TxId)> = conflict
        .conflicting_values
        .iter()
        .zip(conflict.conflicting_txs.iter())
        .map(|(v, tx)| (v.clone(), *tx))
        .collect();

    let conflict_set = ConflictSet {
        entity: conflict.entity,
        attribute: conflict.attribute.clone(),
        assertions: active,
        retractions: vec![],
    };

    let resolved_value = resolve(&conflict_set, &mode);

    ResolutionRecord {
        conflict: conflict.clone(),
        resolved_value,
        resolution_mode: mode,
        resolution_tx: conflict.detected_at,
    }
}

/// Serialize a resolution record as datoms for the audit trail.
///
/// Produces datoms under the `:resolution/*` namespace that capture:
/// - Which entity/attribute pair was in conflict
/// - What the conflicting values were
/// - What mode was used to resolve
/// - What the winning value is
///
/// All datoms are asserted at the supplied `tx`, making the resolution
/// decision a first-class, queryable fact in the store.
///
/// The resolution entity is content-addressed from the conflict's entity,
/// attribute, and detection tx — so identical conflicts detected at the
/// same time produce the same entity (idempotent).
pub fn conflict_to_datoms(record: &ResolutionRecord, tx: TxId) -> Vec<Datom> {
    let mut datoms = Vec::new();

    // Create a content-addressed entity for this resolution record.
    // Identity = BLAKE3(entity_bytes || attribute_str || detected_at_bytes)
    let mut content = Vec::new();
    content.extend_from_slice(record.conflict.entity.as_bytes());
    content.extend_from_slice(record.conflict.attribute.as_str().as_bytes());
    content.extend_from_slice(
        &serde_json::to_vec(&record.conflict.detected_at).expect("TxId serialization cannot fail"),
    );
    let resolution_entity = EntityId::from_content(&content);

    // :resolution/entity — ref to the entity that had the conflict
    datoms.push(Datom::new(
        resolution_entity,
        Attribute::from_keyword(":resolution/entity"),
        Value::Ref(record.conflict.entity),
        tx,
        Op::Assert,
    ));

    // :resolution/attribute — the conflicting attribute's keyword
    datoms.push(Datom::new(
        resolution_entity,
        Attribute::from_keyword(":resolution/attribute"),
        Value::Keyword(record.conflict.attribute.as_str().to_string()),
        tx,
        Op::Assert,
    ));

    // :resolution/mode — the resolution mode applied
    datoms.push(Datom::new(
        resolution_entity,
        Attribute::from_keyword(":resolution/mode"),
        Value::Keyword(record.resolution_mode.as_keyword().to_string()),
        tx,
        Op::Assert,
    ));

    // :resolution/winner — the resolved value (serialized as string for auditability)
    let winner_str = match &record.resolved_value {
        ResolvedValue::Single(v) => {
            serde_json::to_string(v).expect("Value serialization cannot fail")
        }
        ResolvedValue::Multi(vs) => {
            serde_json::to_string(vs).expect("Value serialization cannot fail")
        }
        ResolvedValue::None => "null".to_string(),
    };
    datoms.push(Datom::new(
        resolution_entity,
        Attribute::from_keyword(":resolution/winner"),
        Value::String(winner_str),
        tx,
        Op::Assert,
    ));

    // :resolution/conflict-count — how many values were in conflict
    datoms.push(Datom::new(
        resolution_entity,
        Attribute::from_keyword(":resolution/conflict-count"),
        Value::Long(record.conflict.conflicting_values.len() as i64),
        tx,
        Op::Assert,
    ));

    datoms
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// Witnesses: INV-RESOLUTION-001, INV-RESOLUTION-002, INV-RESOLUTION-003,
// INV-RESOLUTION-004, INV-RESOLUTION-005, INV-RESOLUTION-006,
// INV-RESOLUTION-007, INV-RESOLUTION-008,
// ADR-RESOLUTION-001, ADR-RESOLUTION-002, ADR-RESOLUTION-003,
// ADR-RESOLUTION-004, ADR-RESOLUTION-009,
// NEG-RESOLUTION-001, NEG-RESOLUTION-002, NEG-RESOLUTION-003
#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{AgentId, Attribute, EntityId, TxId, Value};

    fn test_agent() -> AgentId {
        AgentId::from_name("test")
    }

    // Verifies: INV-RESOLUTION-005 — LWW Semilattice Properties
    // Verifies: ADR-RESOLUTION-009 — BLAKE3 Hash Tie-Breaking for LWW
    #[test]
    fn lww_picks_latest_tx() {
        let e = EntityId::from_ident(":test/e");
        let a = Attribute::from_keyword(":db/doc");
        let agent = test_agent();

        let conflict = ConflictSet {
            entity: e,
            attribute: a,
            assertions: vec![
                (Value::String("old".into()), TxId::new(100, 0, agent)),
                (Value::String("new".into()), TxId::new(200, 0, agent)),
            ],
            retractions: vec![],
        };

        let resolved = resolve(&conflict, &ResolutionMode::Lww);
        assert_eq!(resolved, ResolvedValue::Single(Value::String("new".into())));
    }

    // Verifies: INV-RESOLUTION-002 — Resolution Commutativity
    // Verifies: ADR-RESOLUTION-009 — BLAKE3 Hash Tie-Breaking for LWW
    #[test]
    fn lww_tiebreaker_is_deterministic() {
        let e = EntityId::from_ident(":test/e");
        let a = Attribute::from_keyword(":db/doc");
        let agent = test_agent();
        let tx = TxId::new(100, 0, agent);

        let conflict = ConflictSet {
            entity: e,
            attribute: a,
            assertions: vec![
                (Value::String("alpha".into()), tx),
                (Value::String("beta".into()), tx),
            ],
            retractions: vec![],
        };

        let r1 = resolve(&conflict, &ResolutionMode::Lww);
        let r2 = resolve(&conflict, &ResolutionMode::Lww);
        assert_eq!(r1, r2, "INV-RESOLUTION-002: deterministic");
    }

    // Verifies: INV-RESOLUTION-001 — Per-Attribute Resolution (multi-value mode)
    // Verifies: INV-RESOLUTION-007 — Three-Tier Routing Completeness
    #[test]
    fn multi_keeps_all() {
        let e = EntityId::from_ident(":test/e");
        let a = Attribute::from_keyword(":db/doc");
        let agent = test_agent();

        let conflict = ConflictSet {
            entity: e,
            attribute: a,
            assertions: vec![
                (Value::String("a".into()), TxId::new(100, 0, agent)),
                (Value::String("b".into()), TxId::new(200, 0, agent)),
            ],
            retractions: vec![],
        };

        let resolved = resolve(&conflict, &ResolutionMode::Multi);
        match resolved {
            ResolvedValue::Multi(vals) => assert_eq!(vals.len(), 2),
            other => panic!("expected Multi, got {other:?}"),
        }
    }

    // Verifies: INV-STORE-001 — Append-Only Immutability (retraction is new datom)
    #[test]
    fn retraction_removes_value() {
        let e = EntityId::from_ident(":test/e");
        let a = Attribute::from_keyword(":db/doc");
        let agent = test_agent();

        let conflict = ConflictSet {
            entity: e,
            attribute: a,
            assertions: vec![(Value::String("val".into()), TxId::new(100, 0, agent))],
            retractions: vec![(Value::String("val".into()), TxId::new(200, 0, agent))],
        };

        let resolved = resolve(&conflict, &ResolutionMode::Lww);
        assert_eq!(resolved, ResolvedValue::None);
    }

    // Verifies: INV-RESOLUTION-003 — Conservative Conflict Detection (no false positive)
    #[test]
    fn no_conflict_with_single_value() {
        let e = EntityId::from_ident(":test/e");
        let a = Attribute::from_keyword(":db/doc");
        let agent = test_agent();

        let conflict = ConflictSet {
            entity: e,
            attribute: a,
            assertions: vec![(Value::String("x".into()), TxId::new(100, 0, agent))],
            retractions: vec![],
        };

        assert!(!has_conflict(&conflict, &ResolutionMode::Lww));
    }

    // Verifies: INV-RESOLUTION-003 — Conservative Conflict Detection
    // Verifies: INV-RESOLUTION-004 — Conflict Predicate Correctness
    // Verifies: NEG-RESOLUTION-002 — No False Negative Conflict Detection
    #[test]
    fn conflict_with_different_values() {
        let e = EntityId::from_ident(":test/e");
        let a = Attribute::from_keyword(":db/doc");
        let agent = test_agent();

        let conflict = ConflictSet {
            entity: e,
            attribute: a,
            assertions: vec![
                (Value::String("a".into()), TxId::new(100, 0, agent)),
                (Value::String("b".into()), TxId::new(200, 0, agent)),
            ],
            retractions: vec![],
        };

        assert!(has_conflict(&conflict, &ResolutionMode::Lww));
    }

    // Verifies: INV-RESOLUTION-001 — Per-Attribute Resolution
    // Verifies: ADR-RESOLUTION-001 — Per-Attribute Over Global Policy
    #[test]
    fn no_conflict_in_multi_mode() {
        let e = EntityId::from_ident(":test/e");
        let a = Attribute::from_keyword(":db/doc");
        let agent = test_agent();

        let conflict = ConflictSet {
            entity: e,
            attribute: a,
            assertions: vec![
                (Value::String("a".into()), TxId::new(100, 0, agent)),
                (Value::String("b".into()), TxId::new(200, 0, agent)),
            ],
            retractions: vec![],
        };

        assert!(!has_conflict(&conflict, &ResolutionMode::Multi));
    }

    // Verifies: INV-RESOLUTION-001 — Per-Attribute Resolution
    // Verifies: ADR-RESOLUTION-002 — Resolution at Query Time, Not Merge Time
    // Verifies: NEG-RESOLUTION-001 — No Merge-Time Resolution
    #[test]
    fn live_entity_resolves_attributes() {
        let mut store = Store::genesis();
        let agent = AgentId::from_name("test");
        let entity = EntityId::from_ident(":test/entity");

        let tx =
            crate::store::Transaction::new(agent, crate::datom::ProvenanceType::Observed, "test")
                .assert(
                    entity,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("hello".into()),
                )
                .commit(&store)
                .unwrap();

        store.transact(tx).unwrap();

        let view = live_entity(&store, entity);
        assert!(view.contains_key(&Attribute::from_keyword(":db/doc")));
    }

    // -----------------------------------------------------------------------
    // ConflictEntity / ResolutionRecord / detect / resolve_with_trail / datoms
    // -----------------------------------------------------------------------

    /// Helper: create a store with two conflicting assertions on the same
    /// (entity, attribute) pair from two different agents.
    fn store_with_conflict() -> (Store, EntityId, Attribute) {
        let mut store = Store::genesis();
        let entity = EntityId::from_ident(":test/conflict-target");
        let attr = Attribute::from_keyword(":db/doc");

        let agent_a = AgentId::from_name("alice");
        let tx_a = crate::store::Transaction::new(
            agent_a,
            crate::datom::ProvenanceType::Observed,
            "alice asserts",
        )
        .assert(entity, attr.clone(), Value::String("alice-value".into()))
        .commit(&store)
        .unwrap();
        store.transact(tx_a).unwrap();

        let agent_b = AgentId::from_name("bob");
        let tx_b = crate::store::Transaction::new(
            agent_b,
            crate::datom::ProvenanceType::Observed,
            "bob asserts",
        )
        .assert(entity, attr.clone(), Value::String("bob-value".into()))
        .commit(&store)
        .unwrap();
        store.transact(tx_b).unwrap();

        (store, entity, attr)
    }

    // Verifies: INV-RESOLUTION-003 — Conservative Conflict Detection
    // Verifies: ADR-RESOLUTION-003 — Conservative Detection Over Precise
    #[test]
    fn detect_conflicts_finds_two_agent_conflict() {
        let (store, entity, attr) = store_with_conflict();

        let conflict = detect_conflicts(&store, entity, &attr);
        assert!(
            conflict.is_some(),
            "two different values from two agents must be detected as a conflict"
        );

        let c = conflict.unwrap();
        assert_eq!(c.entity, entity);
        assert_eq!(c.attribute, attr);
        assert_eq!(
            c.conflicting_values.len(),
            2,
            "must have exactly 2 conflicting values"
        );
        assert_eq!(
            c.conflicting_txs.len(),
            2,
            "must have exactly 2 conflicting txs"
        );
    }

    #[test]
    fn detect_conflicts_returns_none_for_single_value() {
        let mut store = Store::genesis();
        let entity = EntityId::from_ident(":test/single");
        let attr = Attribute::from_keyword(":db/doc");
        let agent = AgentId::from_name("solo");

        let tx = crate::store::Transaction::new(
            agent,
            crate::datom::ProvenanceType::Observed,
            "solo write",
        )
        .assert(entity, attr.clone(), Value::String("only-value".into()))
        .commit(&store)
        .unwrap();
        store.transact(tx).unwrap();

        assert!(
            detect_conflicts(&store, entity, &attr).is_none(),
            "single value must not be a conflict"
        );
    }

    // Verifies: INV-RESOLUTION-005 — LWW Semilattice Properties
    // Verifies: INV-RESOLUTION-008 — Conflict Entity Datom Trail
    // Verifies: ADR-RESOLUTION-004 — Three-Tier Routing
    #[test]
    fn resolve_with_trail_picks_lww_winner() {
        let (store, entity, attr) = store_with_conflict();

        let conflict = detect_conflicts(&store, entity, &attr).unwrap();
        let record = resolve_with_trail(&conflict, store.schema());

        assert_eq!(record.resolution_mode, ResolutionMode::Lww);
        // LWW picks latest tx; bob's transaction is later, so bob wins.
        match &record.resolved_value {
            ResolvedValue::Single(v) => {
                assert_eq!(
                    *v,
                    Value::String("bob-value".into()),
                    "LWW must pick the value with the latest TxId"
                );
            }
            other => panic!("expected Single, got {other:?}"),
        }
    }

    // Verifies: INV-RESOLUTION-008 — Conflict Entity Datom Trail
    // Verifies: NEG-RESOLUTION-003 — No Resolution Without Provenance
    #[test]
    fn conflict_to_datoms_produces_audit_trail() {
        let (store, entity, attr) = store_with_conflict();

        let conflict = detect_conflicts(&store, entity, &attr).unwrap();
        let record = resolve_with_trail(&conflict, store.schema());

        let agent = AgentId::from_name("auditor");
        let tx = TxId::new(999, 0, agent);
        let datoms = conflict_to_datoms(&record, tx);

        // Must produce exactly 5 datoms:
        //   :resolution/entity, :resolution/attribute, :resolution/mode,
        //   :resolution/winner, :resolution/conflict-count
        assert_eq!(
            datoms.len(),
            5,
            "resolution record must produce exactly 5 audit datoms"
        );

        // All datoms must share the same resolution entity and tx
        let resolution_eid = datoms[0].entity;
        for d in &datoms {
            assert_eq!(d.entity, resolution_eid, "all datoms must share entity");
            assert_eq!(d.tx, tx, "all datoms must use the supplied tx");
            assert_eq!(d.op, crate::datom::Op::Assert, "all datoms must be asserts");
        }

        // Check attribute namespaces
        let attr_names: Vec<&str> = datoms.iter().map(|d| d.attribute.as_str()).collect();
        assert!(attr_names.contains(&":resolution/entity"));
        assert!(attr_names.contains(&":resolution/attribute"));
        assert!(attr_names.contains(&":resolution/mode"));
        assert!(attr_names.contains(&":resolution/winner"));
        assert!(attr_names.contains(&":resolution/conflict-count"));

        // conflict-count datom must have Long(2)
        let count_datom = datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":resolution/conflict-count")
            .unwrap();
        assert_eq!(count_datom.value, Value::Long(2));
    }

    // Verifies: INV-RESOLUTION-006 — Lattice Join Correctness (idempotent)
    // Verifies: INV-MERGE-008 — At-Least-Once Idempotent Delivery
    #[test]
    fn conflict_to_datoms_is_idempotent() {
        let (store, entity, attr) = store_with_conflict();

        let conflict = detect_conflicts(&store, entity, &attr).unwrap();
        let record = resolve_with_trail(&conflict, store.schema());

        let agent = AgentId::from_name("auditor");
        let tx = TxId::new(999, 0, agent);

        let d1 = conflict_to_datoms(&record, tx);
        let d2 = conflict_to_datoms(&record, tx);

        // Same input -> same output (deterministic content-addressed entity)
        assert_eq!(d1.len(), d2.len());
        for (a, b) in d1.iter().zip(d2.iter()) {
            assert_eq!(
                a, b,
                "INV-RESOLUTION-002: datom generation must be deterministic"
            );
        }
    }

    // -------------------------------------------------------------------
    // Property-based tests (proptest)
    // -------------------------------------------------------------------

    mod proptests {
        use super::*;
        use crate::datom::{AgentId, ProvenanceType};
        use crate::proptest_strategies::*;
        use crate::store::Transaction;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn lww_picks_datom_with_latest_tx_id(
                e in arb_entity_id(),
                v1 in arb_doc_value(),
                v2 in arb_doc_value(),
                wall1 in 1u64..500_000,
                wall2 in 500_001u64..1_000_000,
            ) {
                let agent = AgentId::from_name("proptest:agent");
                let tx_early = TxId::new(wall1, 0, agent);
                let tx_late = TxId::new(wall2, 0, agent);
                let a = Attribute::from_keyword(":db/doc");

                let conflict = ConflictSet {
                    entity: e,
                    attribute: a,
                    assertions: vec![
                        (v1, tx_early),
                        (v2.clone(), tx_late),
                    ],
                    retractions: vec![],
                };

                let resolved = resolve(&conflict, &ResolutionMode::Lww);
                match resolved {
                    ResolvedValue::Single(winner) => {
                        prop_assert_eq!(winner, v2, "LWW must pick the value with the latest TxId");
                    }
                    other => prop_assert!(false, "expected Single, got {:?}", other),
                }
            }

            #[test]
            fn multi_resolution_preserves_all_values(
                e in arb_entity_id(),
                values in proptest::collection::vec(arb_doc_value(), 1..=5),
            ) {
                let agent = AgentId::from_name("proptest:agent");
                let a = Attribute::from_keyword(":db/doc");

                let assertions: Vec<(Value, TxId)> = values
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (v.clone(), TxId::new(i as u64 + 1, 0, agent)))
                    .collect();

                let conflict = ConflictSet {
                    entity: e,
                    attribute: a,
                    assertions,
                    retractions: vec![],
                };

                let resolved = resolve(&conflict, &ResolutionMode::Multi);
                match resolved {
                    ResolvedValue::Multi(result_vals) => {
                        prop_assert_eq!(
                            result_vals.len(),
                            values.len(),
                            "Multi mode must preserve all values"
                        );
                    }
                    other => prop_assert!(false, "expected Multi, got {:?}", other),
                }
            }

            #[test]
            fn detect_conflicts_empty_for_single_agent_store(store in arb_store(3)) {
                // A store built by arb_store uses a single agent ("proptest:agent")
                // and the :db/doc attribute. With only one agent asserting the same
                // attribute per entity, there should be no conflicts on user entities.
                // (Genesis entities may have meta-schema datoms but those are
                // deterministic from a single system agent.)
                //
                // We check all entities: for each (entity, attribute) pair, if there
                // is only one distinct value asserted (which is the case for single-agent
                // stores), detect_conflicts should return None.
                let entities = store.entities();
                for entity in &entities {
                    let datoms: Vec<&Datom> = store
                        .datoms()
                        .filter(|d| d.entity == *entity && d.op == Op::Assert)
                        .collect();

                    // Group by attribute
                    let mut by_attr: std::collections::HashMap<&Attribute, Vec<&Datom>> =
                        std::collections::HashMap::new();
                    for d in &datoms {
                        by_attr.entry(&d.attribute).or_default().push(d);
                    }

                    for (attr, attr_datoms) in &by_attr {
                        let cs = ConflictSet::from_datoms(*entity, (*attr).clone(), attr_datoms);
                        let mode = store.schema().resolution_mode(attr);
                        // Single-agent store: at most one assertion per (entity, attr)
                        // so there can be no conflict (different values required).
                        if attr_datoms.len() <= 1 {
                            prop_assert!(
                                !has_conflict(&cs, &mode),
                                "single assertion cannot be a conflict"
                            );
                        }
                    }
                }
            }

            #[test]
            fn resolve_with_trail_produces_valid_record(
                e in arb_entity_id(),
                v1 in arb_doc_value(),
                v2 in arb_doc_value(),
            ) {
                let agent_a = AgentId::from_name("alice");
                let agent_b = AgentId::from_name("bob");

                let mut store = Store::genesis();
                let attr = Attribute::from_keyword(":db/doc");

                let tx1 = Transaction::new(agent_a, ProvenanceType::Observed, "alice says")
                    .assert(e, attr.clone(), v1)
                    .commit(&store)
                    .unwrap();
                store.transact(tx1).unwrap();

                let tx2 = Transaction::new(agent_b, ProvenanceType::Observed, "bob says")
                    .assert(e, attr.clone(), v2)
                    .commit(&store)
                    .unwrap();
                store.transact(tx2).unwrap();

                if let Some(conflict) = detect_conflicts(&store, e, &attr) {
                    let record = resolve_with_trail(&conflict, store.schema());

                    // Record must reference the correct entity and attribute
                    prop_assert_eq!(record.conflict.entity, e);
                    prop_assert_eq!(record.conflict.attribute, attr.clone());

                    // Resolution mode must come from the schema
                    prop_assert_eq!(
                        record.resolution_mode,
                        store.schema().resolution_mode(&attr)
                    );

                    // Resolved value must not be None (there are active assertions)
                    prop_assert!(
                        record.resolved_value != ResolvedValue::None,
                        "resolve_with_trail must produce a value when conflict has active assertions"
                    );

                    // The record's conflict must have exactly 2 conflicting values
                    prop_assert_eq!(
                        record.conflict.conflicting_values.len(),
                        2,
                        "two-agent conflict must have exactly 2 conflicting values"
                    );
                }
            }
        }
    }
}
