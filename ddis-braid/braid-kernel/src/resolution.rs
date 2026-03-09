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
//! - **INV-RESOLUTION-004**: 6-condition conflict predicate.
//! - **INV-RESOLUTION-005**: LWW semilattice properties.

use std::collections::HashMap;

use crate::datom::{Attribute, Datom, EntityId, Op, TxId, Value};
use crate::schema::ResolutionMode;
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
        self.assertions
            .iter()
            .filter(|(val, _)| {
                !self
                    .retractions
                    .iter()
                    .any(|(rv, rtx)| rv == val && rtx > &self.latest_assert_tx(val))
            })
            .cloned()
            .collect()
    }

    fn latest_assert_tx(&self, val: &Value) -> TxId {
        self.assertions
            .iter()
            .filter(|(v, _)| v == val)
            .map(|(_, tx)| *tx)
            .max()
            .unwrap_or(TxId::new(0, 0, crate::datom::AgentId::from_name("nil")))
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
                let h1 = blake3::hash(&serde_json::to_vec(v1).unwrap());
                let h2 = blake3::hash(&serde_json::to_vec(v2).unwrap());
                h1.as_bytes().cmp(h2.as_bytes())
            })
        })
        .unwrap();

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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{AgentId, Attribute, EntityId, TxId, Value};

    fn test_agent() -> AgentId {
        AgentId::from_name("test")
    }

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
}
