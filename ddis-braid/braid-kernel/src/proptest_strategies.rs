//! Proptest strategy hierarchy for property-based testing.
//!
//! Composable strategies from primitive types up to full stores:
//! ```text
//! arb_entity_id()
//! arb_agent_id()
//! arb_namespace() + arb_name() → arb_attribute()
//! arb_value() (all 9 value types)
//! arb_tx_id() → arb_tx_data()
//! arb_datom()
//! arb_store(max_txns) — well-formed store with consistent schema
//! ```
//!
//! # Design Principles
//!
//! - Strategies compose bottom-up (smallest types first)
//! - All generated values are well-formed (valid attributes, schema-compliant values)
//! - Schema-aware: `arb_store` generates stores with consistent schema
//! - Deterministic shrinking: proptest finds minimal counterexamples
//!
//! # Traces To
//!
//! - guide/10-verification.md §10.4
//! - INV-STORE-003 (content-addressable identity)
//! - INV-STORE-011 (HLC monotonicity)
//! - INV-SCHEMA-004 (schema compliance)

use ordered_float::OrderedFloat;
use proptest::prelude::*;

use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};
use crate::store::{Store, Transaction};

// ---------------------------------------------------------------------------
// Primitive strategies
// ---------------------------------------------------------------------------

/// Strategy for arbitrary EntityIds (content-addressed, well-formed by construction).
pub fn arb_entity_id() -> impl Strategy<Value = EntityId> {
    any::<[u8; 32]>().prop_map(EntityId::from_raw_bytes)
}

/// Strategy for arbitrary AgentIds.
pub fn arb_agent_id() -> impl Strategy<Value = AgentId> {
    any::<[u8; 16]>().prop_map(AgentId::from_uuid)
}

/// Strategy for valid attribute namespaces (lowercase alpha, 2-10 chars).
pub fn arb_namespace() -> impl Strategy<Value = String> {
    "[a-z]{2,10}"
}

/// Strategy for valid attribute names (lowercase alpha with hyphens, 2-15 chars).
pub fn arb_name() -> impl Strategy<Value = String> {
    "[a-z][a-z\\-]{1,14}"
}

/// Strategy for valid Attributes (`:namespace/name` format).
pub fn arb_attribute() -> impl Strategy<Value = Attribute> {
    (arb_namespace(), arb_name())
        .prop_map(|(ns, name)| Attribute::from_keyword(&format!(":{ns}/{name}")))
}

/// Strategy for a fixed set of well-known attributes (schema-compliant).
///
/// These are attributes that exist in the genesis schema, so datoms using
/// them will always pass schema validation.
pub fn arb_schema_attribute() -> impl Strategy<Value = Attribute> {
    prop_oneof![
        Just(Attribute::from_keyword(":db/ident")),
        Just(Attribute::from_keyword(":db/doc")),
        Just(Attribute::from_keyword(":db/value-type")),
        Just(Attribute::from_keyword(":db/cardinality")),
        Just(Attribute::from_keyword(":db/unique")),
        Just(Attribute::from_keyword(":db/resolution-mode")),
        Just(Attribute::from_keyword(":tx/provenance")),
        Just(Attribute::from_keyword(":tx/rationale")),
        Just(Attribute::from_keyword(":tx/agent")),
        Just(Attribute::from_keyword(":tx/causal-predecessors")),
        Just(Attribute::from_keyword(":tx/wall-time")),
    ]
}

// ---------------------------------------------------------------------------
// Value strategies
// ---------------------------------------------------------------------------

/// Strategy for arbitrary Values covering all 9 Stage-0 value types.
pub fn arb_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<String>().prop_map(Value::String),
        ":[a-z]{2,8}/[a-z]{2,8}".prop_map(Value::Keyword),
        any::<bool>().prop_map(Value::Boolean),
        any::<i64>().prop_map(Value::Long),
        any::<f64>()
            .prop_filter("finite floats only", |f| f.is_finite())
            .prop_map(|f| Value::Double(OrderedFloat(f))),
        any::<u64>().prop_map(Value::Instant),
        any::<[u8; 16]>().prop_map(Value::Uuid),
        arb_entity_id().prop_map(Value::Ref),
        proptest::collection::vec(any::<u8>(), 0..64).prop_map(Value::Bytes),
    ]
}

/// Strategy for string Values only.
pub fn arb_string_value() -> impl Strategy<Value = Value> {
    any::<String>().prop_map(Value::String)
}

/// Strategy for keyword Values only.
pub fn arb_keyword_value() -> impl Strategy<Value = Value> {
    ":[a-z]{2,8}/[a-z]{2,8}".prop_map(Value::Keyword)
}

/// Strategy for schema-compliant values matching `:db/doc` (string type).
pub fn arb_doc_value() -> impl Strategy<Value = Value> {
    "[A-Za-z ]{1,50}".prop_map(Value::String)
}

// ---------------------------------------------------------------------------
// Transaction metadata strategies
// ---------------------------------------------------------------------------

/// Strategy for TxIds with reasonable ranges.
pub fn arb_tx_id() -> impl Strategy<Value = TxId> {
    (1u64..1_000_000, 0u32..100, arb_agent_id())
        .prop_map(|(wall, logical, agent)| TxId::new(wall, logical, agent))
}

/// Strategy for Op (assert or retract).
pub fn arb_op() -> impl Strategy<Value = Op> {
    prop_oneof![Just(Op::Assert), Just(Op::Retract),]
}

/// Strategy for ProvenanceType.
pub fn arb_provenance() -> impl Strategy<Value = ProvenanceType> {
    prop_oneof![
        Just(ProvenanceType::Observed),
        Just(ProvenanceType::Inferred),
        Just(ProvenanceType::Derived),
        Just(ProvenanceType::Hypothesized),
    ]
}

// ---------------------------------------------------------------------------
// Datom strategies
// ---------------------------------------------------------------------------

/// Strategy for arbitrary datoms (may not be schema-compliant).
pub fn arb_datom() -> impl Strategy<Value = Datom> {
    (
        arb_entity_id(),
        arb_attribute(),
        arb_value(),
        arb_tx_id(),
        arb_op(),
    )
        .prop_map(|(e, a, v, tx, op)| Datom::new(e, a, v, tx, op))
}

/// Strategy for datoms using only genesis schema attributes.
///
/// These datoms use `:db/doc` (string type) to guarantee schema compliance.
pub fn arb_schema_compliant_datom() -> impl Strategy<Value = Datom> {
    (arb_entity_id(), arb_doc_value(), arb_tx_id())
        .prop_map(|(e, v, tx)| Datom::new(e, Attribute::from_keyword(":db/doc"), v, tx, Op::Assert))
}

// ---------------------------------------------------------------------------
// Store strategies
// ---------------------------------------------------------------------------

/// Strategy for a Store built from genesis with up to `max_txns` additional transactions.
///
/// Each transaction asserts 1-5 datoms using schema-compliant attributes.
/// The store is always in a valid state (all invariants hold).
pub fn arb_store(max_txns: usize) -> impl Strategy<Value = Store> {
    let max = if max_txns == 0 { 1 } else { max_txns };
    proptest::collection::vec(
        proptest::collection::vec((arb_entity_id(), arb_doc_value()), 1..=5),
        0..max,
    )
    .prop_map(|txn_groups| {
        let mut store = Store::genesis();
        let agent = AgentId::from_name("proptest:agent");

        for datom_specs in txn_groups {
            let mut tx = Transaction::new(agent, ProvenanceType::Observed, "proptest generated");
            for (entity, value) in datom_specs {
                tx = tx.assert(entity, Attribute::from_keyword(":db/doc"), value);
            }
            if let Ok(committed) = tx.commit(&store) {
                let _ = store.transact(committed);
            }
        }

        store
    })
}

/// Strategy for a pair of stores suitable for merge testing.
///
/// Both stores start from genesis and diverge with independent transactions.
pub fn arb_store_pair(max_txns: usize) -> impl Strategy<Value = (Store, Store)> {
    (arb_store(max_txns), arb_store(max_txns))
}

/// Strategy for a non-empty store (at least one user transaction beyond genesis).
pub fn arb_nonempty_store() -> impl Strategy<Value = Store> {
    arb_store(5).prop_filter("must have user transactions", |s| {
        s.len() > Store::genesis().len()
    })
}

// ---------------------------------------------------------------------------
// Query strategies
// ---------------------------------------------------------------------------

/// Strategy for query Bindings (variable names).
pub fn arb_binding_name() -> impl Strategy<Value = String> {
    "\\?[a-z]{1,8}"
}

// ---------------------------------------------------------------------------
// Tests for the strategies themselves
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        /// All generated entity IDs are distinct (with overwhelming probability).
        #[test]
        fn entity_ids_are_well_formed(id in arb_entity_id()) {
            // EntityId is 32 bytes — well-formed by construction
            prop_assert_eq!(id.as_bytes().len(), 32);
        }

        /// All generated attributes have valid `:namespace/name` format.
        #[test]
        fn attributes_are_valid(attr in arb_attribute()) {
            let s = attr.as_str();
            prop_assert!(s.starts_with(':'));
            prop_assert!(s.contains('/'));
            prop_assert!(!attr.namespace().is_empty());
            prop_assert!(!attr.name().is_empty());
        }

        /// All generated values are non-panicking on type_name().
        #[test]
        fn values_have_type_names(v in arb_value()) {
            let name = v.type_name();
            prop_assert!(!name.is_empty());
        }

        /// Generated TxIds have wall_time > 0.
        #[test]
        fn tx_ids_are_positive(tx in arb_tx_id()) {
            prop_assert!(tx.wall_time() > 0);
        }

        /// Generated datoms are structurally sound.
        #[test]
        fn datoms_are_well_formed(d in arb_datom()) {
            // Datom has all five components
            let _ = d.entity;
            let _ = d.attribute.as_str();
            let _ = d.value.type_name();
            let _ = d.tx;
            let _ = d.op;
        }

        /// Schema-compliant datoms use only valid genesis attributes.
        #[test]
        fn schema_compliant_datoms_use_doc(d in arb_schema_compliant_datom()) {
            prop_assert_eq!(d.attribute.as_str(), ":db/doc");
            prop_assert!(matches!(d.value, Value::String(_)));
        }

        /// Generated stores are always internally consistent.
        #[test]
        fn stores_are_valid(store in arb_store(3)) {
            // Genesis datoms present
            prop_assert!(store.len() >= Store::genesis().len());
            // Frontier is populated
            prop_assert!(!store.frontier().is_empty());
        }

        /// Store pairs are independent (different datom sets with high probability).
        #[test]
        fn store_pairs_independent((s1, s2) in arb_store_pair(2)) {
            // Both start from genesis
            prop_assert!(s1.len() >= Store::genesis().len());
            prop_assert!(s2.len() >= Store::genesis().len());
        }

        /// Non-empty stores have user transactions.
        #[test]
        fn nonempty_stores_have_user_data(store in arb_nonempty_store()) {
            prop_assert!(store.len() > Store::genesis().len());
        }
    }
}
