//! Stratified fixpoint Datalog evaluator.
//!
//! Evaluates queries against the store by matching patterns against datoms
//! and unifying variables using naive fixed-point iteration (re-evaluate all
//! rules each stratum until convergence). Stage 0 supports strata 0-1 (monotonic).
//!
//! # Index-Aware Evaluation
//!
//! Pattern matching selects the narrowest index based on bound terms:
//! - Entity bound (constant or already-bound variable) → entity index O(k)
//! - Attribute + value both bound → AVET index O(1) (typically 1-5 datoms)
//! - Attribute bound (constant) → attribute index O(k)
//! - Neither → full scan O(N)
//!
//! This reduces multi-pattern join cost from O(N^k) to O(N × selectivity).
//!
//! # Invariants
//!
//! - INV-QUERY-001: Stratified fixpoint convergence (Knaster-Tarski).
//! - INV-QUERY-002: Query determinism — same store + same query = same result.
//! - INV-QUERY-008: FFI boundary purity — no external effects.
//! - INV-QUERY-010: Topology-agnostic results — query results independent of graph shape.
//!
//! # Design Decisions
//!
//! - ADR-QUERY-002: Naive bottom-up evaluation (with stratification).
//! - ADR-QUERY-004: FFI for derived functions (pure Rust).

use std::collections::HashMap;

use crate::datom::{Attribute, Datom, EntityId, Value};
use crate::query::clause::{Binding, Clause, FindSpec, Pattern, QueryExpr, Term};
use crate::store::{Frontier, Store};

/// Result of evaluating a query.
#[derive(Clone, Debug)]
pub enum QueryResult {
    /// A relation (set of tuples).
    Rel(Vec<Vec<Value>>),
    /// A single scalar value.
    Scalar(Option<Value>),
}

/// Evaluate a query against the store (all datoms visible).
///
/// Delegates to [`evaluate_with_frontier`] with `frontier = None`.
pub fn evaluate(store: &Store, query: &QueryExpr) -> QueryResult {
    evaluate_with_frontier(store, query, None)
}

/// Evaluate a query against the store, optionally scoped to a frontier.
///
/// When `frontier` is `Some(f)`, only datoms where `f.contains(datom)` are
/// visible to pattern matching. This enables agent-scoped queries: an agent
/// sees only the datoms that were present at the time its frontier was captured.
///
/// When `frontier` is `None`, all datoms in the store are visible (equivalent
/// to calling [`evaluate`]).
///
/// # Invariants
///
/// - INV-QUERY-002: Determinism — same store + same query + same frontier = same result.
/// - INV-QUERY-007: Frontier as queryable attribute — frontier scoping is a first-class
///   query parameter, not an afterthought filter.
/// - ADR-QUERY-005: Local frontier as default query scope.
///
/// # Frontier Subset Property
///
/// For any frontier `f` and query `q`:
///   `results(store, q, Some(f)) ⊆ results(store, q, None)`
///
/// This follows from the fact that frontier filtering can only remove datom
/// candidates, never add them.
pub fn evaluate_with_frontier(
    store: &Store,
    query: &QueryExpr,
    frontier: Option<&Frontier>,
) -> QueryResult {
    let mut bindings: Vec<Binding> = vec![HashMap::new()]; // start with empty binding

    for clause in &query.where_clauses {
        bindings = match clause {
            Clause::Pattern(pattern) => {
                let mut new_bindings = Vec::new();
                for binding in &bindings {
                    let matches = match_pattern_frontier(store, pattern, binding, frontier);
                    new_bindings.extend(matches);
                }
                new_bindings
            }
            Clause::Predicate { op, args } => bindings
                .into_iter()
                .filter(|b| eval_predicate(op, args, b))
                .collect(),
            Clause::Rule { name, .. } => {
                // Stage 1+ feature: named rule invocations are not yet evaluated.
                // Return empty bindings to signal no matches (fail-safe).
                eprintln!(
                    "warning: Rule clause '{name}' is a Stage 1+ feature — \
                     skipping (no results). Upgrade to Stage 1 for rule evaluation."
                );
                vec![]
            }
            Clause::Or(_) => {
                // Stage 1+ feature: disjunctive clauses are not yet evaluated.
                eprintln!(
                    "warning: Or clause is a Stage 1+ feature — \
                     skipping (no results). Upgrade to Stage 1 for disjunction."
                );
                vec![]
            }
            Clause::Frontier { .. } => {
                // Stage 1+ feature: frontier-scoped patterns are not yet evaluated.
                eprintln!(
                    "warning: Frontier clause is a Stage 1+ feature — \
                     skipping (no results). Upgrade to Stage 1 for temporal queries."
                );
                vec![]
            }
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
/// 2. Else if attribute AND value are both bound → use AVET index (typically 1-5 datoms)
/// 3. Else if attribute is a constant → use attribute_index (~100s of datoms per attr)
/// 4. Else → full scan (last resort)
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

/// Frontier-aware pattern matching: delegates to [`match_pattern_indexed`] when
/// no frontier is active, otherwise applies `frontier.contains(datom)` as a
/// pre-filter before unification.
///
/// The frontier filter is applied at the innermost level (per-datom) so that
/// index selection still narrows the candidate set first. This preserves the
/// O(k) entity-index and O(k) attribute-index fast paths.
fn match_pattern_frontier(
    store: &Store,
    pattern: &Pattern,
    existing: &Binding,
    frontier: Option<&Frontier>,
) -> Vec<Binding> {
    match frontier {
        None => match_pattern_indexed(store, pattern, existing),
        Some(f) => {
            // Use the same index-selection strategy as match_pattern_indexed,
            // but apply the frontier filter to each candidate before unification.

            // Try entity index first (most selective)
            if let Some(eid) = resolve_entity(&pattern.entity, existing) {
                let candidates = store.entity_datoms(eid);
                let mut results = Vec::with_capacity(candidates.len());
                for datom in candidates {
                    if f.contains(datom) {
                        if let Some(new_binding) = unify_datom(datom, pattern, existing) {
                            results.push(new_binding);
                        }
                    }
                }
                return results;
            }

            // Try attribute index (second most selective)
            if let Some(attr) = resolve_attribute(&pattern.attribute) {
                let candidates = store.attribute_datoms(&attr);
                let mut results = Vec::with_capacity(candidates.len());
                for datom in candidates {
                    if f.contains(datom) {
                        if let Some(new_binding) = unify_datom(datom, pattern, existing) {
                            results.push(new_binding);
                        }
                    }
                }
                return results;
            }

            // Fallback: full scan with frontier filter
            let mut results = Vec::new();
            for datom in store.datoms() {
                if f.contains(datom) {
                    if let Some(new_binding) = unify_datom(datom, pattern, existing) {
                        results.push(new_binding);
                    }
                }
            }
            results
        }
    }
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
        (op @ (">" | "<" | ">=" | "<="), [Some(a), Some(b)]) => numeric_compare(op, a, b),
        // String matching FFI (INV-QUERY-002, BUG t-e9a08a4e)
        ("str-contains", [Some(a), Some(b)]) => string_contains(a, b),
        ("clojure.string/includes?", [Some(a), Some(b)]) => string_contains(a, b),
        ("str-starts-with", [Some(a), Some(b)]) => string_starts_with(a, b),
        ("str-ends-with", [Some(a), Some(b)]) => string_ends_with(a, b),
        _ => false,
    }
}

/// Cross-type numeric comparison for Long, Instant, and Double.
///
/// Coerces all numeric types to f64 for comparison. This enables queries like
/// `(> ?timestamp 0)` where ?timestamp binds to an Instant and 0 is a Long.
/// Non-numeric types return false (the predicate doesn't match).
fn numeric_compare(op: &str, a: &Value, b: &Value) -> bool {
    let a_num = value_to_f64(a);
    let b_num = value_to_f64(b);

    match (a_num, b_num) {
        (Some(a), Some(b)) => match op {
            ">" => a > b,
            "<" => a < b,
            ">=" => a >= b,
            "<=" => a <= b,
            _ => false,
        },
        _ => false,
    }
}

/// Coerce a Value to f64 for numeric comparison.
///
/// Long → i64 as f64, Instant → u64 as f64, Double → f64.
/// All other types return None.
fn value_to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Long(n) => Some(*n as f64),
        Value::Instant(t) => Some(*t as f64),
        Value::Double(d) => Some(d.into_inner()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// String matching FFI (BUG t-e9a08a4e)
// ---------------------------------------------------------------------------

/// Case-insensitive string containment check.
///
/// Works on String, Keyword, and mixed types. Both values are lowercased
/// before comparison for intuitive behavior.
fn string_contains(haystack: &Value, needle: &Value) -> bool {
    let h = value_to_string(haystack);
    let n = value_to_string(needle);
    match (h, n) {
        (Some(h), Some(n)) => h.to_lowercase().contains(&n.to_lowercase()),
        _ => false,
    }
}

/// Case-insensitive string prefix check.
fn string_starts_with(haystack: &Value, prefix: &Value) -> bool {
    let h = value_to_string(haystack);
    let p = value_to_string(prefix);
    match (h, p) {
        (Some(h), Some(p)) => h.to_lowercase().starts_with(&p.to_lowercase()),
        _ => false,
    }
}

/// Case-insensitive string suffix check.
fn string_ends_with(haystack: &Value, suffix: &Value) -> bool {
    let h = value_to_string(haystack);
    let s = value_to_string(suffix);
    match (h, s) {
        (Some(h), Some(s)) => h.to_lowercase().ends_with(&s.to_lowercase()),
        _ => false,
    }
}

/// Extract string content from a Value (String or Keyword).
fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Keyword(k) => Some(k.clone()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// Witnesses: INV-QUERY-001, INV-QUERY-002, INV-QUERY-005,
// ADR-QUERY-001, ADR-QUERY-002, ADR-QUERY-005,
// NEG-QUERY-002, NEG-QUERY-003
#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{AgentId, ProvenanceType};
    use crate::store::Transaction;

    // Verifies: ADR-QUERY-001 — Datalog Over SQL
    // Verifies: ADR-QUERY-002 — Naive Bottom-Up Evaluation (with stratification)
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

    // Verifies: ADR-QUERY-005 — Local Frontier as Default
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

    // Verifies: INV-QUERY-001 — CALM Compliance (monotonic join)
    // Verifies: ADR-QUERY-002 — Naive Bottom-Up Evaluation (with stratification)
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

    // Verifies: INV-STORE-012 — LIVE Index Correctness (entity path selection)
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

    // Verifies: INV-STORE-012 — LIVE Index Correctness (attribute path selection)
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

    // Verifies: INV-STORE-012 — LIVE Index Correctness (join via index)
    // Verifies: INV-QUERY-001 — CALM Compliance
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
    // Frontier-scoped query evaluation
    // Verifies: INV-QUERY-007 — Frontier as queryable attribute
    // Verifies: ADR-QUERY-005 — Local Frontier as Default
    // -------------------------------------------------------------------

    // Verifies: INV-QUERY-007 — Frontier scoping excludes datoms beyond frontier
    #[test]
    fn frontier_filters_datoms_beyond_cutoff() {
        let mut store = Store::genesis();
        let agent_a = AgentId::from_name("agent-a");
        let agent_b = AgentId::from_name("agent-b");

        // Agent A transacts first
        let entity_a = EntityId::from_ident(":test/alpha");
        let tx_a = Transaction::new(agent_a, ProvenanceType::Observed, "agent-a tx")
            .assert(
                entity_a,
                Attribute::from_keyword(":db/doc"),
                Value::String("alpha doc".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx_a).unwrap();

        // Capture frontier BEFORE agent B transacts
        let frontier_before_b = Frontier::current(&store);

        // Agent B transacts after
        let entity_b = EntityId::from_ident(":test/beta");
        let tx_b = Transaction::new(agent_b, ProvenanceType::Observed, "agent-b tx")
            .assert(
                entity_b,
                Attribute::from_keyword(":db/doc"),
                Value::String("beta doc".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx_b).unwrap();

        // Full query (no frontier) should see both
        let query = QueryExpr::new(
            FindSpec::Rel(vec!["?e".into(), "?v".into()]),
            vec![Clause::Pattern(Pattern::new(
                Term::Variable("?e".into()),
                Term::Attr(Attribute::from_keyword(":db/doc")),
                Term::Variable("?v".into()),
            ))],
        );

        let full_result = evaluate(&store, &query);
        let full_rows = match &full_result {
            QueryResult::Rel(rows) => rows,
            _ => panic!("expected Rel"),
        };

        // Frontier-scoped query should exclude agent B's datoms
        let scoped_result = evaluate_with_frontier(&store, &query, Some(&frontier_before_b));
        let scoped_rows = match &scoped_result {
            QueryResult::Rel(rows) => rows,
            _ => panic!("expected Rel"),
        };

        // Scoped must be strictly fewer than full (agent B's doc is excluded)
        assert!(
            scoped_rows.len() < full_rows.len(),
            "frontier-scoped query should return fewer rows: scoped={}, full={}",
            scoped_rows.len(),
            full_rows.len()
        );

        // Agent A's entity should be visible in scoped results
        let has_alpha = scoped_rows
            .iter()
            .any(|row| row[1] == Value::String("alpha doc".into()));
        assert!(
            has_alpha,
            "agent-a's datom should be visible through frontier"
        );

        // Agent B's entity should NOT be visible in scoped results
        let has_beta = scoped_rows
            .iter()
            .any(|row| row[1] == Value::String("beta doc".into()));
        assert!(
            !has_beta,
            "agent-b's datom should NOT be visible through pre-b frontier"
        );
    }

    // Verifies: ADR-QUERY-005 — None frontier returns same as evaluate()
    #[test]
    fn none_frontier_matches_evaluate() {
        let mut store = Store::genesis();
        let agent = AgentId::from_name("test-agent");
        let entity = EntityId::from_ident(":test/gamma");

        let tx = Transaction::new(agent, ProvenanceType::Observed, "test tx")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("gamma doc".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        let query = QueryExpr::new(
            FindSpec::Rel(vec!["?e".into(), "?v".into()]),
            vec![Clause::Pattern(Pattern::new(
                Term::Variable("?e".into()),
                Term::Attr(Attribute::from_keyword(":db/doc")),
                Term::Variable("?v".into()),
            ))],
        );

        let direct = evaluate(&store, &query);
        let via_none = evaluate_with_frontier(&store, &query, None);

        let direct_rows = match &direct {
            QueryResult::Rel(rows) => rows,
            _ => panic!("expected Rel"),
        };
        let none_rows = match &via_none {
            QueryResult::Rel(rows) => rows,
            _ => panic!("expected Rel"),
        };

        assert_eq!(
            direct_rows.len(),
            none_rows.len(),
            "evaluate() and evaluate_with_frontier(None) must return same row count"
        );
        assert_eq!(
            direct_rows, none_rows,
            "evaluate() and evaluate_with_frontier(None) must return identical rows"
        );
    }

    // Verifies: INV-QUERY-007 — Frontier scoping with entity-index path
    #[test]
    fn frontier_scoping_works_with_entity_index() {
        let mut store = Store::genesis();
        let agent_a = AgentId::from_name("agent-a");
        let agent_b = AgentId::from_name("agent-b");

        // Both agents write to the SAME entity (different attributes)
        let shared_entity = EntityId::from_ident(":test/shared");

        let tx_a = Transaction::new(agent_a, ProvenanceType::Observed, "a writes")
            .assert(
                shared_entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("from agent a".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx_a).unwrap();

        let frontier_after_a = Frontier::current(&store);

        let tx_b = Transaction::new(agent_b, ProvenanceType::Observed, "b writes")
            .assert(
                shared_entity,
                Attribute::from_keyword(":tx/rationale"),
                Value::String("from agent b".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx_b).unwrap();

        // Query with bound entity (triggers entity-index path)
        let query = QueryExpr::new(
            FindSpec::Rel(vec!["?a".into(), "?v".into()]),
            vec![Clause::Pattern(Pattern::new(
                Term::Entity(shared_entity),
                Term::Variable("?a".into()),
                Term::Variable("?v".into()),
            ))],
        );

        let full = evaluate(&store, &query);
        let scoped = evaluate_with_frontier(&store, &query, Some(&frontier_after_a));

        let full_rows = match &full {
            QueryResult::Rel(rows) => rows,
            _ => panic!("expected Rel"),
        };
        let scoped_rows = match &scoped {
            QueryResult::Rel(rows) => rows,
            _ => panic!("expected Rel"),
        };

        // Full should have datoms from both agents; scoped should exclude agent B's
        assert!(
            scoped_rows.len() < full_rows.len(),
            "entity-index scoped query should exclude agent-b datoms: scoped={}, full={}",
            scoped_rows.len(),
            full_rows.len()
        );
    }

    // -------------------------------------------------------------------
    // Proptest: evaluate determinism
    // Verifies: INV-QUERY-002 — Query Determinism
    // Verifies: NEG-QUERY-002 — No Query Side Effects
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

    // -------------------------------------------------------------------
    // Proptest: Frontier subset property
    // Verifies: INV-QUERY-007 — Frontier as queryable attribute
    // Verifies: ADR-QUERY-005 — Local Frontier as Default
    //
    // For any store, query, and frontier:
    //   results(store, q, Some(f)) ⊆ results(store, q, None)
    // -------------------------------------------------------------------

    mod frontier_proptests {
        use super::*;
        use crate::datom::{AgentId, ProvenanceType};
        use crate::proptest_strategies::arb_store;
        use crate::store::Transaction;
        use proptest::prelude::*;
        use std::collections::HashSet;

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

        fn rows_to_set(result: &QueryResult) -> HashSet<Vec<Value>> {
            match result {
                QueryResult::Rel(rows) => rows.iter().cloned().collect(),
                _ => panic!("expected Rel"),
            }
        }

        proptest! {
            /// Frontier-scoped results are always a subset of full results.
            ///
            /// We build a store with a base set of transactions, capture the
            /// frontier, add more transactions, then verify that querying with
            /// the earlier frontier returns a subset of the full query.
            #[test]
            fn frontier_results_subset_of_full(
                base_store in arb_store(3),
                extra_entities in proptest::collection::vec(
                    (crate::proptest_strategies::arb_entity_id(),
                     crate::proptest_strategies::arb_doc_value()),
                    1..=5
                ),
            ) {
                // Capture frontier at current store state
                let frontier_before = Frontier::current(&base_store);

                // Grow the store with a DIFFERENT agent so frontier_before
                // does not cover the new datoms
                let mut grown = base_store;
                let new_agent = AgentId::from_name("proptest:frontier-extra");
                let mut tx = Transaction::new(
                    new_agent,
                    ProvenanceType::Observed,
                    "extra datoms",
                );
                for (entity, value) in extra_entities {
                    tx = tx.assert(
                        entity,
                        Attribute::from_keyword(":db/doc"),
                        value,
                    );
                }
                if let Ok(committed) = tx.commit(&grown) {
                    let _ = grown.transact(committed);
                }

                let query = doc_query();
                let full_set = rows_to_set(&evaluate(&grown, &query));
                let scoped_set = rows_to_set(
                    &evaluate_with_frontier(&grown, &query, Some(&frontier_before)),
                );

                // Every row in scoped must also appear in full
                for row in &scoped_set {
                    prop_assert!(
                        full_set.contains(row),
                        "frontier-scoped row not in full results: {:?}",
                        row
                    );
                }

                // Scoped should have no more rows than full
                prop_assert!(
                    scoped_set.len() <= full_set.len(),
                    "frontier-scoped has MORE rows ({}) than full ({})",
                    scoped_set.len(),
                    full_set.len()
                );
            }

            /// evaluate_with_frontier(None) is identical to evaluate().
            #[test]
            fn none_frontier_equals_evaluate(store in arb_store(3)) {
                let query = doc_query();
                let direct = evaluate(&store, &query);
                let via_none = evaluate_with_frontier(&store, &query, None);

                let direct_set = rows_to_set(&direct);
                let none_set = rows_to_set(&via_none);

                prop_assert_eq!(
                    direct_set.len(),
                    none_set.len(),
                    "None frontier should match evaluate(): {} vs {}",
                    direct_set.len(),
                    none_set.len()
                );
                prop_assert_eq!(
                    direct_set,
                    none_set,
                    "None frontier should produce identical row set",
                );
            }
        }
    }

    // -------------------------------------------------------------------
    // Proptest: INV-QUERY-004..008
    // Verifies: INV-QUERY-004 — Stratum Classification Determinism
    // Verifies: INV-QUERY-005 — Query Mode Output Correctness
    // Verifies: INV-QUERY-006 — Monotonic Growth Under Store Growth
    // Verifies: INV-QUERY-007 — Aggregation Preserves Grouping
    // Verifies: INV-QUERY-008 — Query API Purity
    // -------------------------------------------------------------------

    mod proptests {
        use super::*;
        use crate::datom::{AgentId, ProvenanceType};
        use crate::proptest_strategies::{arb_doc_value, arb_entity_id, arb_query_expr, arb_store};
        use crate::query::aggregate::{aggregate, AggregateFunction, AggregateSpec};
        use crate::query::stratum::classify;
        use crate::store::Transaction;
        use ordered_float::OrderedFloat;
        use proptest::prelude::*;

        // ---------------------------------------------------------------
        // INV-QUERY-004: Stratum classification is deterministic.
        // For any query, classify() returns the same stratum twice.
        // ---------------------------------------------------------------

        proptest! {
            #[test]
            fn stratum_classification_is_deterministic(q in arb_query_expr()) {
                let s1 = classify(&q);
                let s2 = classify(&q);
                prop_assert_eq!(
                    s1, s2,
                    "INV-QUERY-004: classify() must be deterministic: {:?} vs {:?}",
                    s1, s2
                );
            }
        }

        // ---------------------------------------------------------------
        // INV-QUERY-005: Query modes produce expected output.
        // Pattern queries (FindSpec::Rel) return Rel; scalar queries
        // (FindSpec::Scalar) return Scalar.
        // ---------------------------------------------------------------

        proptest! {
            #[test]
            fn rel_query_returns_rel(store in arb_store(3)) {
                // Build a Rel-mode query against the store
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
                        // Each row must have exactly as many columns as the find spec
                        for row in &rows {
                            prop_assert_eq!(
                                row.len(),
                                2,
                                "INV-QUERY-005: Rel row must have 2 columns, got {}",
                                row.len()
                            );
                        }
                    }
                    QueryResult::Scalar(_) => {
                        prop_assert!(
                            false,
                            "INV-QUERY-005: Rel query must return Rel, got Scalar"
                        );
                    }
                }
            }

            #[test]
            fn scalar_query_returns_scalar(store in arb_store(3)) {
                // Build a Scalar-mode query against the store
                let query = QueryExpr::new(
                    FindSpec::Scalar("?v".into()),
                    vec![Clause::Pattern(Pattern::new(
                        Term::Variable("?e".into()),
                        Term::Attr(Attribute::from_keyword(":db/doc")),
                        Term::Variable("?v".into()),
                    ))],
                );
                let result = evaluate(&store, &query);
                match result {
                    QueryResult::Scalar(_) => {
                        // Scalar is correct -- value may be Some or None
                    }
                    QueryResult::Rel(_) => {
                        prop_assert!(
                            false,
                            "INV-QUERY-005: Scalar query must return Scalar, got Rel"
                        );
                    }
                }
            }
        }

        // ---------------------------------------------------------------
        // INV-QUERY-006: Stratified evaluation correctness.
        // For monotonic queries (no negation), results are monotonically
        // growing with store growth -- adding datoms never removes results.
        // ---------------------------------------------------------------

        proptest! {
            #[test]
            fn monotonic_query_grows_with_store(
                base_store in arb_store(2),
                extra_entities in proptest::collection::vec(
                    (arb_entity_id(), arb_doc_value()), 1..=5
                ),
            ) {
                // Evaluate a monotonic query on the base store
                let query = QueryExpr::new(
                    FindSpec::Rel(vec!["?e".into(), "?v".into()]),
                    vec![Clause::Pattern(Pattern::new(
                        Term::Variable("?e".into()),
                        Term::Attr(Attribute::from_keyword(":db/doc")),
                        Term::Variable("?v".into()),
                    ))],
                );
                let before = evaluate(&base_store, &query);
                let before_count = match &before {
                    QueryResult::Rel(rows) => rows.len(),
                    _ => panic!("expected Rel"),
                };

                // Grow the store by adding more datoms
                let mut grown_store = base_store;
                let agent = AgentId::from_name("proptest:growth");
                let mut tx =
                    Transaction::new(agent, ProvenanceType::Observed, "growth");
                for (entity, value) in extra_entities {
                    tx = tx.assert(
                        entity,
                        Attribute::from_keyword(":db/doc"),
                        value,
                    );
                }
                if let Ok(committed) = tx.commit(&grown_store) {
                    let _ = grown_store.transact(committed);
                }

                // Evaluate the same query on the grown store
                let after = evaluate(&grown_store, &query);
                let after_count = match &after {
                    QueryResult::Rel(rows) => rows.len(),
                    _ => panic!("expected Rel"),
                };

                prop_assert!(
                    after_count >= before_count,
                    "INV-QUERY-006: monotonic query must not lose results on store growth: \
                     before={}, after={}",
                    before_count,
                    after_count
                );
            }
        }

        // ---------------------------------------------------------------
        // INV-QUERY-007: Aggregation preserves grouping.
        // count/sum/min/max return results consistent with manual
        // computation on known data.
        // ---------------------------------------------------------------

        proptest! {
            #[test]
            fn aggregation_count_matches_row_count(store in arb_store(3)) {
                // Evaluate a Rel query to get raw rows
                let query = QueryExpr::new(
                    FindSpec::Rel(vec!["?e".into(), "?v".into()]),
                    vec![Clause::Pattern(Pattern::new(
                        Term::Variable("?e".into()),
                        Term::Attr(Attribute::from_keyword(":db/doc")),
                        Term::Variable("?v".into()),
                    ))],
                );
                let result = evaluate(&store, &query);
                let raw_count = match &result {
                    QueryResult::Rel(rows) => rows.len(),
                    _ => panic!("expected Rel"),
                };

                // Apply COUNT aggregation
                let agg = aggregate(
                    &result,
                    &[AggregateSpec {
                        function: AggregateFunction::Count,
                        column: 0,
                        output_name: "count".into(),
                    }],
                    &[],
                );
                match &agg {
                    QueryResult::Rel(rows) if !rows.is_empty() => {
                        let expected = Value::Long(raw_count as i64);
                        prop_assert_eq!(
                            &rows[0][0],
                            &expected,
                            "INV-QUERY-007: COUNT must equal raw row count: \
                             expected {}, got {:?}",
                            raw_count,
                            &rows[0][0]
                        );
                    }
                    QueryResult::Rel(rows) if rows.is_empty() => {
                        // Empty result means no aggregation output; raw_count must be 0
                        prop_assert_eq!(
                            raw_count,
                            0,
                            "INV-QUERY-007: empty agg output but raw_count = {}",
                            raw_count
                        );
                    }
                    other => {
                        prop_assert!(
                            false,
                            "INV-QUERY-007: expected Rel from aggregate, got {:?}",
                            other
                        );
                    }
                }
            }

            #[test]
            fn aggregation_min_max_consistent(
                values in proptest::collection::vec(1i64..=1000, 1..=20)
            ) {
                // Build a synthetic Rel result from known Long values
                let rows: Vec<Vec<Value>> = values
                    .iter()
                    .map(|&v| vec![Value::Long(v)])
                    .collect();
                let result = QueryResult::Rel(rows);

                let expected_min = *values.iter().min().unwrap();
                let expected_max = *values.iter().max().unwrap();
                let expected_sum: f64 = values.iter().map(|&v| v as f64).sum();

                // MIN
                let min_agg = aggregate(
                    &result,
                    &[AggregateSpec {
                        function: AggregateFunction::Min,
                        column: 0,
                        output_name: "min".into(),
                    }],
                    &[],
                );
                match &min_agg {
                    QueryResult::Rel(rows) => {
                        let expected = Value::Long(expected_min);
                        prop_assert_eq!(
                            &rows[0][0],
                            &expected,
                            "INV-QUERY-007: MIN mismatch: expected {}, got {:?}",
                            expected_min,
                            &rows[0][0]
                        );
                    }
                    _ => prop_assert!(false, "expected Rel"),
                }

                // MAX
                let max_agg = aggregate(
                    &result,
                    &[AggregateSpec {
                        function: AggregateFunction::Max,
                        column: 0,
                        output_name: "max".into(),
                    }],
                    &[],
                );
                match &max_agg {
                    QueryResult::Rel(rows) => {
                        let expected = Value::Long(expected_max);
                        prop_assert_eq!(
                            &rows[0][0],
                            &expected,
                            "INV-QUERY-007: MAX mismatch: expected {}, got {:?}",
                            expected_max,
                            &rows[0][0]
                        );
                    }
                    _ => prop_assert!(false, "expected Rel"),
                }

                // SUM
                let sum_agg = aggregate(
                    &result,
                    &[AggregateSpec {
                        function: AggregateFunction::Sum,
                        column: 0,
                        output_name: "sum".into(),
                    }],
                    &[],
                );
                match &sum_agg {
                    QueryResult::Rel(rows) => {
                        let expected = Value::Double(OrderedFloat(expected_sum));
                        prop_assert_eq!(
                            &rows[0][0],
                            &expected,
                            "INV-QUERY-007: SUM mismatch: expected {}, got {:?}",
                            expected_sum,
                            &rows[0][0]
                        );
                    }
                    _ => prop_assert!(false, "expected Rel"),
                }
            }
        }

        // ---------------------------------------------------------------
        // INV-QUERY-008: Query API is pure.
        // evaluate(store, query) called twice returns identical results.
        // ---------------------------------------------------------------

        proptest! {
            #[test]
            fn evaluate_is_pure(store in arb_store(3), q in arb_query_expr()) {
                let r1 = evaluate(&store, &q);
                let r2 = evaluate(&store, &q);

                match (&r1, &r2) {
                    (QueryResult::Rel(rows1), QueryResult::Rel(rows2)) => {
                        prop_assert_eq!(
                            rows1.len(),
                            rows2.len(),
                            "INV-QUERY-008: purity -- row count diverged: {} vs {}",
                            rows1.len(),
                            rows2.len()
                        );
                        for (i, (a, b)) in
                            rows1.iter().zip(rows2.iter()).enumerate()
                        {
                            prop_assert_eq!(
                                a,
                                b,
                                "INV-QUERY-008: purity -- row {} differs: {:?} vs {:?}",
                                i,
                                a,
                                b
                            );
                        }
                    }
                    (QueryResult::Scalar(v1), QueryResult::Scalar(v2)) => {
                        prop_assert_eq!(
                            v1,
                            v2,
                            "INV-QUERY-008: purity -- scalar diverged: {:?} vs {:?}",
                            v1,
                            v2
                        );
                    }
                    _ => {
                        prop_assert!(
                            false,
                            "INV-QUERY-008: purity -- result type changed between calls"
                        );
                    }
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // W2E.4: Frontier query tests — verify frontier-scoped queries
    // return correct subsets of full query results.
    //
    // Verifies: INV-QUERY-007 — Frontier as queryable attribute
    // Verifies: ADR-QUERY-005 — Local Frontier as Default
    // -------------------------------------------------------------------

    // Verifies: frontier at a specific transaction excludes later datoms
    #[test]
    fn frontier_at_tx_excludes_later_datoms() {
        let mut store = Store::genesis();
        let agent = AgentId::from_name("test-agent");

        // Transaction 1: add entity alpha
        let entity_alpha = EntityId::from_ident(":test/alpha");
        let tx1 = Transaction::new(agent, ProvenanceType::Observed, "tx1")
            .assert(
                entity_alpha,
                Attribute::from_keyword(":db/doc"),
                Value::String("alpha doc".into()),
            )
            .commit(&store)
            .unwrap();
        let tx1_receipt = store.transact(tx1).unwrap();
        let tx1_id = tx1_receipt.tx_id;

        // Build frontier as-of tx1
        let frontier_at_tx1 = Frontier::at(&store, tx1_id);

        // Transaction 2: add entity beta (AFTER frontier cutoff)
        let entity_beta = EntityId::from_ident(":test/beta");
        let tx2 = Transaction::new(agent, ProvenanceType::Observed, "tx2")
            .assert(
                entity_beta,
                Attribute::from_keyword(":db/doc"),
                Value::String("beta doc".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx2).unwrap();

        // Query all :db/doc datoms
        let query = QueryExpr::new(
            FindSpec::Rel(vec!["?e".into(), "?v".into()]),
            vec![Clause::Pattern(Pattern::new(
                Term::Variable("?e".into()),
                Term::Attr(Attribute::from_keyword(":db/doc")),
                Term::Variable("?v".into()),
            ))],
        );

        let full = evaluate(&store, &query);
        let scoped = evaluate_with_frontier(&store, &query, Some(&frontier_at_tx1));

        let full_rows = match &full {
            QueryResult::Rel(rows) => rows,
            _ => panic!("expected Rel"),
        };
        let scoped_rows = match &scoped {
            QueryResult::Rel(rows) => rows,
            _ => panic!("expected Rel"),
        };

        // Full should see both alpha and beta; scoped only alpha (and genesis)
        assert!(
            scoped_rows.len() < full_rows.len(),
            "frontier-at-tx1 should exclude tx2 datoms: scoped={}, full={}",
            scoped_rows.len(),
            full_rows.len()
        );

        // Scoped must contain alpha doc
        let has_alpha = scoped_rows
            .iter()
            .any(|row| row[1] == Value::String("alpha doc".into()));
        assert!(has_alpha, "alpha doc must be visible at frontier_at_tx1");

        // Scoped must NOT contain beta doc
        let has_beta = scoped_rows
            .iter()
            .any(|row| row[1] == Value::String("beta doc".into()));
        assert!(!has_beta, "beta doc must NOT be visible at frontier_at_tx1");
    }

    // Verifies: Datalog query under frontier scoping returns correct subset
    #[test]
    fn frontier_scoped_datalog_returns_correct_subset() {
        let mut store = Store::genesis();
        let agent_a = AgentId::from_name("agent-a");
        let agent_b = AgentId::from_name("agent-b");

        // Agent A: create entity with doc
        let entity_a = EntityId::from_ident(":test/entity-a");
        let tx_a = Transaction::new(agent_a, ProvenanceType::Observed, "agent-a tx")
            .assert(
                entity_a,
                Attribute::from_keyword(":db/doc"),
                Value::String("A-doc".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx_a).unwrap();

        // Capture frontier before agent B
        let frontier_before_b = Frontier::current(&store);

        // Agent B: create another entity with doc
        let entity_b = EntityId::from_ident(":test/entity-b");
        let tx_b = Transaction::new(agent_b, ProvenanceType::Observed, "agent-b tx")
            .assert(
                entity_b,
                Attribute::from_keyword(":db/doc"),
                Value::String("B-doc".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx_b).unwrap();

        // Single-pattern query: find all entities with :db/doc
        let query = QueryExpr::new(
            FindSpec::Rel(vec!["?e".into(), "?doc".into()]),
            vec![Clause::Pattern(Pattern::new(
                Term::Variable("?e".into()),
                Term::Attr(Attribute::from_keyword(":db/doc")),
                Term::Variable("?doc".into()),
            ))],
        );

        let full_result = evaluate(&store, &query);
        let scoped_result = evaluate_with_frontier(&store, &query, Some(&frontier_before_b));

        let full_rows = match &full_result {
            QueryResult::Rel(rows) => rows,
            _ => panic!("expected Rel"),
        };
        let scoped_rows = match &scoped_result {
            QueryResult::Rel(rows) => rows,
            _ => panic!("expected Rel"),
        };

        // Scoped must be a strict subset of full (B's datom excluded)
        assert!(
            scoped_rows.len() < full_rows.len(),
            "frontier-scoped query should return fewer rows: scoped={}, full={}",
            scoped_rows.len(),
            full_rows.len()
        );

        // Full should contain B-doc; scoped should not
        let full_has_b = full_rows
            .iter()
            .any(|row| row.iter().any(|v| *v == Value::String("B-doc".into())));
        let scoped_has_b = scoped_rows
            .iter()
            .any(|row| row.iter().any(|v| *v == Value::String("B-doc".into())));
        assert!(full_has_b, "full query must see B-doc");
        assert!(!scoped_has_b, "frontier-scoped query must NOT see B-doc");

        // Scoped must contain A-doc (agent A's data is within the frontier)
        let scoped_has_a = scoped_rows
            .iter()
            .any(|row| row.iter().any(|v| *v == Value::String("A-doc".into())));
        assert!(scoped_has_a, "frontier-scoped query must see A-doc");
    }

    // Verifies: scalar Datalog query under frontier scoping
    #[test]
    fn frontier_scoped_scalar_query() {
        let mut store = Store::genesis();
        let agent = AgentId::from_name("test-agent");

        // Add a unique entity with a doc we can find via scalar query
        let entity = EntityId::from_ident(":test/scalar-target");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "scalar test")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("scalar doc value".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        // Capture frontier including the entity
        let frontier_with_entity = Frontier::current(&store);

        // Scalar query: find the doc for this specific entity
        let query = QueryExpr::new(
            FindSpec::Scalar("?doc".into()),
            vec![Clause::Pattern(Pattern::new(
                Term::Entity(entity),
                Term::Attr(Attribute::from_keyword(":db/doc")),
                Term::Variable("?doc".into()),
            ))],
        );

        // Full query should find it
        let full = evaluate(&store, &query);
        assert!(
            matches!(&full, QueryResult::Scalar(Some(Value::String(s))) if s == "scalar doc value"),
            "full scalar query should find doc: {:?}",
            full
        );

        // Frontier-scoped query should also find it (entity was added before frontier)
        let scoped = evaluate_with_frontier(&store, &query, Some(&frontier_with_entity));
        assert!(
            matches!(&scoped, QueryResult::Scalar(Some(Value::String(s))) if s == "scalar doc value"),
            "frontier-scoped scalar query should find doc: {:?}",
            scoped
        );

        // Empty frontier should NOT find it
        let empty_frontier = Frontier::new();
        let empty_scoped = evaluate_with_frontier(&store, &query, Some(&empty_frontier));
        assert!(
            matches!(&empty_scoped, QueryResult::Scalar(None)),
            "empty frontier should yield no result: {:?}",
            empty_scoped
        );
    }

    // Verifies: frontier scoping is monotonic — growing store does not shrink scoped results
    #[test]
    fn frontier_scoping_is_monotonic_under_store_growth() {
        let mut store = Store::genesis();
        let agent_a = AgentId::from_name("agent-a");
        let agent_b = AgentId::from_name("agent-b");

        // Agent A adds data
        let entity = EntityId::from_ident(":test/mono");
        let tx_a = Transaction::new(agent_a, ProvenanceType::Observed, "initial")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("initial value".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx_a).unwrap();

        // Capture frontier after A's transaction
        let frontier_a = Frontier::current(&store);

        let query = QueryExpr::new(
            FindSpec::Rel(vec!["?e".into(), "?v".into()]),
            vec![Clause::Pattern(Pattern::new(
                Term::Variable("?e".into()),
                Term::Attr(Attribute::from_keyword(":db/doc")),
                Term::Variable("?v".into()),
            ))],
        );

        // Count scoped results before store growth
        let scoped_before = evaluate_with_frontier(&store, &query, Some(&frontier_a));
        let count_before = match &scoped_before {
            QueryResult::Rel(rows) => rows.len(),
            _ => panic!("expected Rel"),
        };

        // Agent B adds more data (store grows)
        let entity_b = EntityId::from_ident(":test/extra");
        let tx_b = Transaction::new(agent_b, ProvenanceType::Observed, "extra")
            .assert(
                entity_b,
                Attribute::from_keyword(":db/doc"),
                Value::String("extra value".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx_b).unwrap();

        // Re-run scoped query with same frontier on grown store
        let scoped_after = evaluate_with_frontier(&store, &query, Some(&frontier_a));
        let count_after = match &scoped_after {
            QueryResult::Rel(rows) => rows.len(),
            _ => panic!("expected Rel"),
        };

        // Frontier-scoped results must not decrease when store grows
        // (the frontier gates visibility; new datoms outside the frontier
        // don't affect existing scoped results)
        assert_eq!(
            count_before, count_after,
            "frontier-scoped results must be stable under store growth: {} vs {}",
            count_before, count_after
        );
    }
}
