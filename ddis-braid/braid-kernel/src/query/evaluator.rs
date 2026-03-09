//! Semi-naive fixpoint Datalog evaluator.
//!
//! Evaluates queries against the store by matching patterns against datoms
//! and unifying variables. Stage 0 supports strata 0-1 (monotonic).
//!
//! # Algorithm
//!
//! 1. Initialize binding set from first clause.
//! 2. For each subsequent clause, join with existing bindings.
//! 3. Apply predicate filters.
//! 4. Project result to the find specification.

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
                    let matches = match_pattern(store, pattern, binding);
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

/// Match a pattern against all datoms, producing new bindings.
fn match_pattern(store: &Store, pattern: &Pattern, existing: &Binding) -> Vec<Binding> {
    let mut results = Vec::new();

    for datom in store.datoms() {
        if let Some(new_binding) = unify_datom(datom, pattern, existing) {
            results.push(new_binding);
        }
    }

    results
}

/// Try to unify a datom with a pattern, extending the existing binding.
fn unify_datom(datom: &Datom, pattern: &Pattern, existing: &Binding) -> Option<Binding> {
    let mut binding = existing.clone();

    // Match entity
    if !unify_entity(&datom.entity, &pattern.entity, &mut binding) {
        return None;
    }

    // Match attribute
    if !unify_attribute(&datom.attribute, &pattern.attribute, &mut binding) {
        return None;
    }

    // Match value
    if !unify_value(&datom.value, &pattern.value, &mut binding) {
        return None;
    }

    Some(binding)
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
                // Should find at least the 17 axiomatic attribute docs + our test doc
                assert!(
                    rows.len() >= 18,
                    "expected at least 18 rows, got {}",
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
}
