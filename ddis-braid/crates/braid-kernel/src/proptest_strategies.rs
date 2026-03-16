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
//! - docs/guide/10-verification.md §10.4
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
// DiGraph strategies (XC.1 — INV-QUERY-016, INV-QUERY-018)
// ---------------------------------------------------------------------------

/// Strategy for arbitrary directed graphs with configurable size bounds.
///
/// Generates graphs with node labels "n0", "n1", ..., "n{max_nodes-1}"
/// and random edges between them. Supports self-loops.
///
/// # Traces To
/// - INV-QUERY-016 (HITS convergence)
/// - INV-QUERY-018 (k-Core decomposition)
pub fn arb_digraph(
    max_nodes: usize,
    max_edges: usize,
) -> impl Strategy<Value = crate::query::graph::DiGraph> {
    let max_n = if max_nodes == 0 { 1 } else { max_nodes };
    let max_e = if max_edges == 0 { 1 } else { max_edges };
    (
        1..=max_n,
        proptest::collection::vec((0..max_n, 0..max_n), 0..max_e),
    )
        .prop_map(move |(n, edges)| {
            let mut g = crate::query::graph::DiGraph::new();
            let n = n.min(max_n);
            for i in 0..n {
                g.add_node(&format!("n{i}"));
            }
            for (src, dst) in edges {
                if src < n && dst < n {
                    g.add_edge(&format!("n{src}"), &format!("n{dst}"));
                }
            }
            g
        })
}

/// Strategy for connected directed graphs (at least a spanning path).
///
/// Ensures the graph has a path from n0 to n{n-1} so graph algorithms
/// that require connectivity don't degenerate.
pub fn arb_connected_digraph(
    max_nodes: usize,
) -> impl Strategy<Value = crate::query::graph::DiGraph> {
    let max_n = if max_nodes < 2 { 2 } else { max_nodes };
    (
        2..=max_n,
        proptest::collection::vec((0..max_n, 0..max_n), 0..max_n * 2),
    )
        .prop_map(move |(n, extra_edges)| {
            let mut g = crate::query::graph::DiGraph::new();
            let n = n.min(max_n);
            for i in 0..n {
                g.add_node(&format!("n{i}"));
            }
            // Spanning path: n0 → n1 → ... → n{n-1}
            for i in 0..n - 1 {
                g.add_edge(&format!("n{i}"), &format!("n{}", i + 1));
            }
            // Additional random edges
            for (src, dst) in extra_edges {
                if src < n && dst < n {
                    g.add_edge(&format!("n{src}"), &format!("n{dst}"));
                }
            }
            g
        })
}

// ---------------------------------------------------------------------------
// Clause / Query strategies (XC.1 — INV-QUERY-004..008)
// ---------------------------------------------------------------------------

/// Strategy for a Term (variable or constant).
pub fn arb_term() -> impl Strategy<Value = crate::query::clause::Term> {
    use crate::query::clause::Term;
    prop_oneof![
        arb_binding_name().prop_map(Term::Variable),
        arb_value().prop_map(Term::Constant),
        arb_entity_id().prop_map(Term::Entity),
        arb_attribute().prop_map(Term::Attr),
    ]
}

/// Strategy for a variable-only Term (used in patterns where we want bindings).
pub fn arb_variable_term() -> impl Strategy<Value = crate::query::clause::Term> {
    arb_binding_name().prop_map(crate::query::clause::Term::Variable)
}

/// Strategy for a Pattern (entity, attribute, value positions).
///
/// Generates patterns where at least one position is a variable (otherwise
/// the pattern is a ground fact check, which is valid but less interesting).
pub fn arb_pattern() -> impl Strategy<Value = crate::query::clause::Pattern> {
    use crate::query::clause::Pattern;
    // At least entity is a variable to ensure the pattern binds something
    (arb_variable_term(), arb_term(), arb_term()).prop_map(|(e, a, v)| Pattern::new(e, a, v))
}

/// Strategy for a Clause (pattern or predicate).
pub fn arb_clause() -> impl Strategy<Value = crate::query::clause::Clause> {
    use crate::query::clause::Clause;
    prop_oneof![
        // Pattern clause (most common)
        arb_pattern().prop_map(Clause::Pattern),
        // Predicate clause
        (
            prop_oneof![
                Just(">".into()),
                Just("<".into()),
                Just("=".into()),
                Just("!=".into())
            ],
            proptest::collection::vec(arb_term(), 2..=3),
        )
            .prop_map(|(op, args)| Clause::Predicate { op, args }),
    ]
}

/// Strategy for a FindSpec.
pub fn arb_find_spec() -> impl Strategy<Value = crate::query::clause::FindSpec> {
    use crate::query::clause::FindSpec;
    prop_oneof![
        proptest::collection::vec(arb_binding_name(), 1..=4).prop_map(FindSpec::Rel),
        arb_binding_name().prop_map(FindSpec::Scalar),
    ]
}

/// Strategy for a complete QueryExpr.
pub fn arb_query_expr() -> impl Strategy<Value = crate::query::clause::QueryExpr> {
    (
        arb_find_spec(),
        proptest::collection::vec(arb_clause(), 1..=4),
    )
        .prop_map(|(find, clauses)| crate::query::clause::QueryExpr::new(find, clauses))
}

// ---------------------------------------------------------------------------
// Bilateral strategies (XC.1 — INV-BILATERAL-001..005)
// ---------------------------------------------------------------------------

/// Strategy for FitnessComponents with values in [0, 1].
///
/// Each component is independently generated in the valid range.
/// This is for testing bilateral computations, NOT for testing that
/// compute_fitness() produces correct values (that's a different test).
pub fn arb_fitness_components() -> impl Strategy<Value = crate::bilateral::FitnessComponents> {
    use crate::bilateral::FitnessComponents;
    (
        0.0f64..=1.0,
        0.0f64..=1.0,
        0.0f64..=1.0,
        0.0f64..=1.0,
        0.0f64..=1.0,
        0.0f64..=1.0,
        0.0f64..=1.0,
    )
        .prop_map(|(v, c, d, h, k, i, u)| FitnessComponents {
            validation: v,
            coverage: c,
            drift: d,
            harvest_quality: h,
            contradiction: k,
            incompleteness: i,
            uncertainty: u,
        })
}

/// Strategy for FitnessScore with valid total = weighted sum.
///
/// The total is computed from components using the standard weights,
/// ensuring internal consistency. Unmeasured components are empty.
pub fn arb_fitness_score() -> impl Strategy<Value = crate::bilateral::FitnessScore> {
    use crate::bilateral::FitnessScore;
    arb_fitness_components().prop_map(|components| {
        // Standard weights from spec: V=0.15, C=0.20, D=0.15, H=0.10, K=0.15, I=0.15, U=0.10
        let total = 0.15 * components.validation
            + 0.20 * components.coverage
            + 0.15 * components.drift
            + 0.10 * components.harvest_quality
            + 0.15 * components.contradiction
            + 0.15 * components.incompleteness
            + 0.10 * components.uncertainty;
        FitnessScore {
            total,
            components,
            unmeasured: vec![],
        }
    })
}

/// Strategy for a trajectory of F(S) values (for convergence analysis).
///
/// Generates a sequence of fitness scores in [0, 1]. May or may not
/// be monotonically increasing (testing convergence detection).
pub fn arb_fitness_trajectory(max_len: usize) -> impl Strategy<Value = Vec<f64>> {
    let max = if max_len == 0 { 1 } else { max_len };
    proptest::collection::vec(0.0f64..=1.0, 1..=max)
}

/// Strategy for a monotonically non-decreasing fitness trajectory.
///
/// For testing INV-BILATERAL-001: F(S) must be monotonically non-decreasing
/// across well-formed transitions.
pub fn arb_monotone_trajectory(max_len: usize) -> impl Strategy<Value = Vec<f64>> {
    let max = if max_len == 0 { 1 } else { max_len };
    proptest::collection::vec(0.0f64..=1.0, 1..=max).prop_map(|mut vals| {
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        vals
    })
}

// ---------------------------------------------------------------------------
// Layout / Transaction strategies (XC.1 — INV-LAYOUT-001..006)
// ---------------------------------------------------------------------------

/// Strategy for a committed transaction (ready for layout serialization).
///
/// The transaction contains 1-5 schema-compliant datoms and valid
/// transaction metadata. Returns the transaction along with its datoms
/// for verification (since committed transactions seal their contents).
pub fn arb_transaction_datoms(max_datoms: usize) -> impl Strategy<Value = Vec<Datom>> {
    let max = if max_datoms == 0 { 1 } else { max_datoms };
    proptest::collection::vec(arb_schema_compliant_datom(), 1..=max)
}

/// Strategy for pairs of transactions (for merge/diff testing).
pub fn arb_transaction_pair() -> impl Strategy<Value = (Vec<Datom>, Vec<Datom>)> {
    (arb_transaction_datoms(5), arb_transaction_datoms(5))
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

        // === New XC.1 strategy tests ===

        /// Generated digraphs have the expected node count.
        #[test]
        fn digraphs_have_valid_structure(g in arb_digraph(8, 16)) {
            prop_assert!(g.node_count() <= 8);
            prop_assert!(g.edge_count() <= 16);
        }

        /// Connected digraphs have at least n-1 edges (spanning path).
        #[test]
        fn connected_digraphs_have_spanning_path(g in arb_connected_digraph(6)) {
            let n = g.node_count();
            prop_assert!(n >= 2);
            // At least spanning path edges exist
            prop_assert!(g.edge_count() >= n - 1);
        }

        /// Generated clauses are well-formed (all variants constructible).
        #[test]
        fn clauses_are_constructible(c in arb_clause()) {
            // Just verify no panic during construction
            let _ = format!("{c:?}");
        }

        /// Generated query expressions have at least one clause.
        #[test]
        fn query_exprs_have_clauses(q in arb_query_expr()) {
            prop_assert!(!q.where_clauses.is_empty());
        }

        /// Fitness components are all in [0, 1].
        #[test]
        fn fitness_components_bounded(fc in arb_fitness_components()) {
            prop_assert!(fc.validation >= 0.0 && fc.validation <= 1.0);
            prop_assert!(fc.coverage >= 0.0 && fc.coverage <= 1.0);
            prop_assert!(fc.drift >= 0.0 && fc.drift <= 1.0);
            prop_assert!(fc.harvest_quality >= 0.0 && fc.harvest_quality <= 1.0);
            prop_assert!(fc.contradiction >= 0.0 && fc.contradiction <= 1.0);
            prop_assert!(fc.incompleteness >= 0.0 && fc.incompleteness <= 1.0);
            prop_assert!(fc.uncertainty >= 0.0 && fc.uncertainty <= 1.0);
        }

        /// Fitness score total is in [0, 1] (weighted sum of bounded components).
        #[test]
        fn fitness_score_bounded(fs in arb_fitness_score()) {
            prop_assert!(fs.total >= 0.0 && fs.total <= 1.0,
                "F(S) = {} out of [0,1]", fs.total);
        }

        /// Monotone trajectories are actually non-decreasing.
        #[test]
        fn monotone_trajectories_are_sorted(vals in arb_monotone_trajectory(10)) {
            for w in vals.windows(2) {
                prop_assert!(w[0] <= w[1],
                    "Not monotone: {} > {}", w[0], w[1]);
            }
        }

        /// Transaction datom strategies produce non-empty sets.
        #[test]
        fn transaction_datoms_nonempty(datoms in arb_transaction_datoms(5)) {
            prop_assert!(!datoms.is_empty());
            prop_assert!(datoms.len() <= 5);
        }
    }
}
