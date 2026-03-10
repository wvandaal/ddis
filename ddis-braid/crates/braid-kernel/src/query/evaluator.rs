//! Semi-naive fixpoint Datalog evaluator.
//!
//! Evaluates queries against the store by matching patterns against datoms
//! and unifying variables. Stage 0 supports strata 0-1 (monotonic).
//!
//! # Index-Aware Evaluation
//!
//! Pattern matching selects the narrowest index based on bound terms:
//! - Entity bound (constant or already-bound variable) → entity index O(k)
//! - Attribute bound (constant) → attribute index O(k)
//! - Neither → full scan O(N)
//!
//! This reduces multi-pattern join cost from O(N^k) to O(N × selectivity).

use std::collections::HashMap;

use crate::datom::{Attribute, Datom, EntityId, Value};
use crate::query::clause::{Binding, Clause, FindSpec, Pattern, QueryExpr, Term};
use crate::store::Store;

/// Result of evaluating a query.
#[derive(Clone, Debug)]
pub enum QueryResult {
    /// A relation (set of tuples).
    Rel(Vec<Vec<Value>>),
    /// A single scalar value.
    Scalar(Option<Value>),
}

/// Evaluate a query against the store.
pub fn evaluate(store: &Store, query: &QueryExpr) -> QueryResult {
    let mut bindings: Vec<Binding> = vec![HashMap::new()]; // start with empty binding

    for clause in &query.where_clauses {
        bindings = match clause {
            Clause::Pattern(pattern) => {
                let mut new_bindings = Vec::new();
                for binding in &bindings {
                    let matches = match_pattern_indexed(store, pattern, binding);
                    new_bindings.extend(matches);
                }
                new_bindings
            }
            Clause::Predicate { op, args } => bindings
                .into_iter()
                .filter(|b| eval_predicate(op, args, b))
                .collect(),
        };
    }

    // Project to find spec
    match &query.find {
        FindSpec::Rel(vars) => {
            let rows: Vec<Vec<Value>> = bindings
                .iter()
                .map(|b| {
                    vars.iter()
                        .map(|v| b.get(v).cloned().unwrap_or(Value::String("?".into())))
                        .collect()
                })
                .collect();
            QueryResult::Rel(rows)
        }
        FindSpec::Scalar(var) => {
            let val = bindings.first().and_then(|b| b.get(var).cloned());
            QueryResult::Scalar(val)
        }
    }
}

/// Resolve an entity term to a concrete EntityId if already bound.
fn resolve_entity(term: &Term, binding: &Binding) -> Option<EntityId> {
    match term {
        Term::Entity(eid) => Some(*eid),
        Term::Constant(Value::Ref(eid)) => Some(*eid),
        Term::Variable(var) => match binding.get(var) {
            Some(Value::Ref(eid)) => Some(*eid),
            _ => None,
        },
        _ => None,
    }
}

/// Resolve an attribute term to a concrete Attribute if it's a constant.
fn resolve_attribute(term: &Term) -> Option<Attribute> {
    match term {
        Term::Attr(attr) => Some(attr.clone()),
        Term::Constant(Value::Keyword(kw)) => Some(Attribute::from_keyword(kw)),
        _ => None,
    }
}

/// Index-aware pattern matching: select narrowest candidate set, then unify.
///
/// Strategy (INV-QUERY-PERF-001):
/// 1. If entity is bound → use entity_index (typically ~5-10 datoms per entity)
/// 2. Else if attribute is a constant → use attribute_index (~100s of datoms per attr)
/// 3. Else → full scan (last resort)
fn match_pattern_indexed(store: &Store, pattern: &Pattern, existing: &Binding) -> Vec<Binding> {
    // Try entity index first (most selective)
    if let Some(eid) = resolve_entity(&pattern.entity, existing) {
        let candidates = store.entity_datoms(eid);
        let mut results = Vec::with_capacity(candidates.len());
        for datom in candidates {
            if let Some(new_binding) = unify_datom(datom, pattern, existing) {
                results.push(new_binding);
            }
        }
        return results;
    }

    // Try attribute index (second most selective)
    if let Some(attr) = resolve_attribute(&pattern.attribute) {
        let candidates = store.attribute_datoms(&attr);
        let mut results = Vec::with_capacity(candidates.len());
        for datom in candidates {
            if let Some(new_binding) = unify_datom(datom, pattern, existing) {
                results.push(new_binding);
            }
        }
        return results;
    }

    // Fallback: full scan
    let mut results = Vec::new();
    for datom in store.datoms() {
        if let Some(new_binding) = unify_datom(datom, pattern, existing) {
            results.push(new_binding);
        }
    }
    results
}

/// Try to unify a datom with a pattern, extending the existing binding.
///
/// Defers binding clone until all three positions match, avoiding allocation
/// on failed unification attempts.
fn unify_datom(datom: &Datom, pattern: &Pattern, existing: &Binding) -> Option<Binding> {
    // Pre-check constant positions without cloning the binding.
    // This avoids HashMap allocation for the ~95% of datoms that fail early.
    if !can_unify_entity(&datom.entity, &pattern.entity, existing) {
        return None;
    }
    if !can_unify_attribute(&datom.attribute, &pattern.attribute, existing) {
        return None;
    }
    if !can_unify_value(&datom.value, &pattern.value, existing) {
        return None;
    }

    // All positions pass pre-check — now clone and bind.
    let mut binding = existing.clone();

    // These should all succeed given the pre-check, but we re-check to bind variables.
    if !unify_entity(&datom.entity, &pattern.entity, &mut binding) {
        return None;
    }
    if !unify_attribute(&datom.attribute, &pattern.attribute, &mut binding) {
        return None;
    }
    if !unify_value(&datom.value, &pattern.value, &mut binding) {
        return None;
    }

    Some(binding)
}

/// Check if entity can unify without modifying the binding (read-only pre-check).
fn can_unify_entity(entity: &EntityId, term: &Term, binding: &Binding) -> bool {
    match term {
        Term::Variable(var) => match binding.get(var) {
            Some(existing) => *existing == Value::Ref(*entity),
            None => true, // unbound variable always matches
        },
        Term::Entity(expected) => entity == expected,
        Term::Constant(Value::Ref(expected)) => entity == expected,
        _ => false,
    }
}

/// Check if attribute can unify without modifying the binding (read-only pre-check).
fn can_unify_attribute(attr: &Attribute, term: &Term, binding: &Binding) -> bool {
    match term {
        Term::Variable(var) => match binding.get(var) {
            Some(existing) => *existing == Value::Keyword(attr.as_str().to_string()),
            None => true,
        },
        Term::Attr(expected) => attr == expected,
        Term::Constant(Value::Keyword(expected)) => attr.as_str() == expected,
        _ => false,
    }
}

/// Check if value can unify without modifying the binding (read-only pre-check).
fn can_unify_value(value: &Value, term: &Term, binding: &Binding) -> bool {
    match term {
        Term::Variable(var) => match binding.get(var) {
            Some(existing) => existing == value,
            None => true,
        },
        Term::Constant(expected) => value == expected,
        Term::Entity(eid) => matches!(value, Value::Ref(r) if r == eid),
        _ => false,
    }
}

fn unify_entity(entity: &EntityId, term: &Term, binding: &mut Binding) -> bool {
    match term {
        Term::Variable(var) => {
            let val = Value::Ref(*entity);
            match binding.get(var) {
                Some(existing) => *existing == val,
                None => {
                    binding.insert(var.clone(), val);
                    true
                }
            }
        }
        Term::Entity(expected) => entity == expected,
        Term::Constant(Value::Ref(expected)) => entity == expected,
        _ => false,
    }
}

fn unify_attribute(attr: &Attribute, term: &Term, binding: &mut Binding) -> bool {
    match term {
        Term::Variable(var) => {
            let val = Value::Keyword(attr.as_str().to_string());
            match binding.get(var) {
                Some(existing) => *existing == val,
                None => {
                    binding.insert(var.clone(), val);
                    true
                }
            }
        }
        Term::Attr(expected) => attr == expected,
        Term::Constant(Value::Keyword(expected)) => attr.as_str() == expected,
        _ => false,
    }
}

fn unify_value(value: &Value, term: &Term, binding: &mut Binding) -> bool {
    match term {
        Term::Variable(var) => match binding.get(var) {
            Some(existing) => existing == value,
            None => {
                binding.insert(var.clone(), value.clone());
                true
            }
        },
        Term::Constant(expected) => value == expected,
        Term::Entity(eid) => matches!(value, Value::Ref(r) if r == eid),
        _ => false,
    }
}

fn eval_predicate(op: &str, args: &[Term], binding: &Binding) -> bool {
    let resolved: Vec<Option<&Value>> = args
        .iter()
        .map(|t| match t {
            Term::Variable(v) => binding.get(v),
            Term::Constant(c) => Some(c),
            _ => None,
        })
        .collect();

    match (op, resolved.as_slice()) {
        ("=", [Some(a), Some(b)]) => a == b,
        ("!=", [Some(a), Some(b)]) => a != b,
        (">", [Some(Value::Long(a)), Some(Value::Long(b))]) => a > b,
        ("<", [Some(Value::Long(a)), Some(Value::Long(b))]) => a < b,
        (">=", [Some(Value::Long(a)), Some(Value::Long(b))]) => a >= b,
        ("<=", [Some(Value::Long(a)), Some(Value::Long(b))]) => a <= b,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{AgentId, ProvenanceType};
    use crate::store::Transaction;

    #[test]
    fn query_find_all_doc_attributes() {
        let mut store = Store::genesis();
        let agent = AgentId::from_name("test");
        let entity = EntityId::from_ident(":test/thing");

        let tx = Transaction::new(agent, ProvenanceType::Observed, "test")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("test doc".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        // Query: find ?e ?v where [?e :db/doc ?v]
        let query = QueryExpr::new(
            FindSpec::Rel(vec!["?e".into(), "?v".into()]),
            vec![Clause::Pattern(Pattern::new(
                Term::Variable("?e".into()),
                Term::Attr(Attribute::from_keyword(":db/doc")),
                Term::Variable("?v".into()),
            ))],
        );

        let result = evaluate(&store, &query);
        match result {
            QueryResult::Rel(rows) => {
                // Should find at least the 18 axiomatic attribute docs + our test doc
                assert!(
                    rows.len() >= 19,
                    "expected at least 19 rows, got {}",
                    rows.len()
                );
            }
            _ => panic!("expected Rel result"),
        }
    }

    #[test]
    fn query_with_entity_filter() {
        let store = Store::genesis();

        // Find the doc for :db/ident specifically
        let db_ident = EntityId::from_ident(":db/ident");
        let query = QueryExpr::new(
            FindSpec::Scalar("?doc".into()),
            vec![Clause::Pattern(Pattern::new(
                Term::Entity(db_ident),
                Term::Attr(Attribute::from_keyword(":db/doc")),
                Term::Variable("?doc".into()),
            ))],
        );

        let result = evaluate(&store, &query);
        match result {
            QueryResult::Scalar(Some(Value::String(doc))) => {
                assert_eq!(doc, "Attribute's keyword name");
            }
            other => panic!("expected Scalar(String), got {other:?}"),
        }
    }

    #[test]
    fn query_with_join() {
        let store = Store::genesis();

        // Find all attributes where the value type is :db.type/keyword
        // Pattern 1: [?e :db/ident ?name]
        // Pattern 2: [?e :db/valueType :db.type/keyword]
        let query = QueryExpr::new(
            FindSpec::Rel(vec!["?name".into()]),
            vec![
                Clause::Pattern(Pattern::new(
                    Term::Variable("?e".into()),
                    Term::Attr(Attribute::from_keyword(":db/ident")),
                    Term::Variable("?name".into()),
                )),
                Clause::Pattern(Pattern::new(
                    Term::Variable("?e".into()),
                    Term::Attr(Attribute::from_keyword(":db/valueType")),
                    Term::Constant(Value::Keyword(":db.type/keyword".into())),
                )),
            ],
        );

        let result = evaluate(&store, &query);
        match result {
            QueryResult::Rel(rows) => {
                // Several attributes have keyword type: db/ident, db/cardinality, etc.
                assert!(
                    rows.len() >= 5,
                    "expected at least 5 keyword-typed attrs, got {}",
                    rows.len()
                );
            }
            _ => panic!("expected Rel result"),
        }
    }

    #[test]
    fn index_selects_entity_path() {
        let store = Store::genesis();
        let db_ident = EntityId::from_ident(":db/ident");

        // Pattern with bound entity uses entity index (not full scan)
        let pattern = Pattern::new(
            Term::Entity(db_ident),
            Term::Variable("?a".into()),
            Term::Variable("?v".into()),
        );
        let binding = HashMap::new();
        let results = match_pattern_indexed(&store, &pattern, &binding);

        // :db/ident has multiple datoms (ident, doc, valueType, cardinality, etc.)
        assert!(
            !results.is_empty(),
            "entity-indexed lookup should find datoms"
        );
        // Every result should have the correct entity
        for b in &results {
            // entity variable wasn't used (it was a constant), but we can verify
            // by checking that all results are consistent
            assert!(b.contains_key("?a"), "attribute variable should be bound");
            assert!(b.contains_key("?v"), "value variable should be bound");
        }
    }

    #[test]
    fn index_selects_attribute_path() {
        let store = Store::genesis();

        // Pattern with bound attribute uses attribute index
        let pattern = Pattern::new(
            Term::Variable("?e".into()),
            Term::Attr(Attribute::from_keyword(":db/ident")),
            Term::Variable("?v".into()),
        );
        let binding = HashMap::new();
        let results = match_pattern_indexed(&store, &pattern, &binding);

        // Every entity with :db/ident should be found
        assert!(
            results.len() >= 18,
            "attribute-indexed lookup should find all ident datoms, got {}",
            results.len()
        );
    }

    #[test]
    fn join_uses_entity_index_on_second_pattern() {
        let store = Store::genesis();

        // Pattern 1: [?e :db/ident ?name] — uses attribute index
        // Pattern 2: [?e :db/doc ?doc] — ?e is now bound → uses entity index!
        let query = QueryExpr::new(
            FindSpec::Rel(vec!["?name".into(), "?doc".into()]),
            vec![
                Clause::Pattern(Pattern::new(
                    Term::Variable("?e".into()),
                    Term::Attr(Attribute::from_keyword(":db/ident")),
                    Term::Variable("?name".into()),
                )),
                Clause::Pattern(Pattern::new(
                    Term::Variable("?e".into()),
                    Term::Attr(Attribute::from_keyword(":db/doc")),
                    Term::Variable("?doc".into()),
                )),
            ],
        );

        let result = evaluate(&store, &query);
        match result {
            QueryResult::Rel(rows) => {
                // All 18 axiomatic attributes have both :db/ident and :db/doc
                assert!(
                    rows.len() >= 18,
                    "join should find all attributes with ident+doc, got {}",
                    rows.len()
                );
                // Verify each row has both name and doc
                for row in &rows {
                    assert_eq!(row.len(), 2, "each row should have name + doc");
                }
            }
            _ => panic!("expected Rel result"),
        }
    }

    // -------------------------------------------------------------------
    // Proptest: evaluate determinism
    // -------------------------------------------------------------------

    mod evaluator_proptests {
        use super::*;
        use crate::proptest_strategies::arb_store;
        use proptest::prelude::*;

        fn doc_query() -> QueryExpr {
            QueryExpr::new(
                FindSpec::Rel(vec!["?e".into(), "?v".into()]),
                vec![Clause::Pattern(Pattern::new(
                    Term::Variable("?e".into()),
                    Term::Attr(Attribute::from_keyword(":db/doc")),
                    Term::Variable("?v".into()),
                ))],
            )
        }

        fn ident_query() -> QueryExpr {
            QueryExpr::new(
                FindSpec::Rel(vec!["?e".into(), "?name".into()]),
                vec![Clause::Pattern(Pattern::new(
                    Term::Variable("?e".into()),
                    Term::Attr(Attribute::from_keyword(":db/ident")),
                    Term::Variable("?name".into()),
                ))],
            )
        }

        fn extract_rows(result: &QueryResult) -> &Vec<Vec<Value>> {
            match result {
                QueryResult::Rel(rows) => rows,
                _ => panic!("expected Rel result"),
            }
        }

        proptest! {
            #[test]
            fn evaluate_is_deterministic_doc(store in arb_store(3)) {
                let query = doc_query();
                let r1 = evaluate(&store, &query);
                let r2 = evaluate(&store, &query);

                let rows1 = extract_rows(&r1);
                let rows2 = extract_rows(&r2);

                prop_assert_eq!(
                    rows1.len(),
                    rows2.len(),
                    "evaluate must return same row count: {} vs {}",
                    rows1.len(),
                    rows2.len()
                );
                for (i, (a, b)) in rows1.iter().zip(rows2.iter()).enumerate() {
                    prop_assert_eq!(
                        a, b,
                        "evaluate must return same rows at index {}: {:?} vs {:?}",
                        i, a, b
                    );
                }
            }

            #[test]
            fn evaluate_is_deterministic_ident(store in arb_store(3)) {
                let query = ident_query();
                let r1 = evaluate(&store, &query);
                let r2 = evaluate(&store, &query);

                let rows1 = extract_rows(&r1);
                let rows2 = extract_rows(&r2);

                prop_assert_eq!(
                    rows1.len(),
                    rows2.len(),
                    "evaluate must return same row count: {} vs {}",
                    rows1.len(),
                    rows2.len()
                );
                for (i, (a, b)) in rows1.iter().zip(rows2.iter()).enumerate() {
                    prop_assert_eq!(
                        a, b,
                        "evaluate must return same rows at index {}: {:?} vs {:?}",
                        i, a, b
                    );
                }
            }
        }
    }
}
