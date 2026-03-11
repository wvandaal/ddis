//! Zero-result query diagnostics (INV-INTERFACE-012).
//!
//! When a query returns an empty result set, diagnose WHY and produce
//! actionable suggestions. The diagnostic function
//! `D(q,s) = unknown_attrs ∪ type_mismatches ∪ near_matches`.
//!
//! # Design
//!
//! Diagnostics are zero-cost on successful queries — only invoked when
//! `|result| = 0`. Uses Levenshtein edit distance for attribute suggestion
//! (optimal string alignment).
//!
//! # Invariants
//!
//! - **INV-INTERFACE-012**: Zero-Result Query Diagnostics.

use crate::datom::{Attribute, Value};
use crate::query::clause::{Clause, Pattern, QueryExpr, Term};
use crate::schema::{Schema, ValueType};
use crate::store::Store;

/// A diagnostic for a zero-result query.
#[derive(Clone, Debug)]
pub struct QueryDiagnostic {
    /// What kind of issue was detected.
    pub kind: DiagnosticKind,
    /// Human-readable explanation.
    pub message: String,
    /// Suggested fix (copy-pasteable).
    pub suggestion: Option<String>,
}

/// Classification of diagnostic issues.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// Attribute used in query does not exist in the store's schema.
    UnknownAttribute {
        /// The attribute the user tried.
        given: String,
        /// Top 3 closest known attributes by edit distance.
        suggestions: Vec<String>,
    },
    /// Value type doesn't match the attribute's schema type.
    TypeMismatch {
        /// The attribute.
        attr: String,
        /// Expected value type.
        expected: String,
        /// What was provided.
        got: String,
        /// Example of correct usage.
        example: String,
    },
    /// Store is empty (no datoms beyond genesis schema).
    EmptyStore,
    /// Query patterns are valid but no entities match all clauses together.
    NoMatchingEntities {
        /// How many entities match each individual clause.
        per_clause_counts: Vec<usize>,
    },
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Diagnose why a query returned zero results.
///
/// Only call this when `evaluate()` returns `Rel(vec![])` or `Scalar(None)`.
/// Complexity: `O(|patterns| × |schema|)` for attribute checks — negligible
/// compared to the query itself.
pub fn diagnose_empty_results(store: &Store, query: &QueryExpr) -> Vec<QueryDiagnostic> {
    let mut diagnostics = Vec::new();
    let schema = store.schema();

    // Check for empty store (only genesis schema datoms — 18 entities).
    if store.entity_count() <= 18 {
        diagnostics.push(QueryDiagnostic {
            kind: DiagnosticKind::EmptyStore,
            message: "Store contains only genesis schema — no user data.".into(),
            suggestion: Some("braid init && braid bootstrap".into()),
        });
        return diagnostics;
    }

    // Collect all known attribute names for fuzzy matching.
    let known_attrs: Vec<String> = schema
        .attributes()
        .map(|(a, _)| a.as_str().to_string())
        .collect();

    for clause in &query.where_clauses {
        if let Clause::Pattern(pattern) = clause {
            check_attribute(pattern, schema, &known_attrs, &mut diagnostics);
            check_value_type(pattern, schema, &mut diagnostics);
        }
    }

    // If no attribute/type issues found, check per-clause match counts.
    if diagnostics.is_empty() {
        check_clause_selectivity(store, query, &mut diagnostics);
    }

    diagnostics
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// If the attribute position is a constant, check if it exists in the schema.
/// If not, compute edit distance to all known attrs and suggest the top 3.
fn check_attribute(
    pattern: &Pattern,
    schema: &Schema,
    known_attrs: &[String],
    diagnostics: &mut Vec<QueryDiagnostic>,
) {
    let attr_str = match &pattern.attribute {
        Term::Attr(a) => a.as_str().to_string(),
        Term::Constant(Value::Keyword(kw)) => kw.clone(),
        _ => return, // Variable — nothing to check.
    };

    // Try to look it up in the schema.
    if schema
        .attribute(&Attribute::from_keyword(&attr_str))
        .is_some()
    {
        return; // Known attribute — fine.
    }

    // Unknown. Find nearest matches by edit distance.
    let mut candidates: Vec<(usize, &str)> = known_attrs
        .iter()
        .map(|k| (levenshtein(&attr_str, k), k.as_str()))
        .collect();
    candidates.sort_by_key(|(dist, _)| *dist);
    let suggestions: Vec<String> = candidates
        .iter()
        .take(3)
        .map(|(_, name)| (*name).to_string())
        .collect();

    let suggestion_text = if suggestions.is_empty() {
        None
    } else {
        Some(format!("Did you mean: {}?", suggestions.join(", ")))
    };

    diagnostics.push(QueryDiagnostic {
        kind: DiagnosticKind::UnknownAttribute {
            given: attr_str.clone(),
            suggestions: suggestions.clone(),
        },
        message: format!(
            "Unknown attribute `{}`. Nearest: {}",
            attr_str,
            suggestions.join(", ")
        ),
        suggestion: suggestion_text,
    });
}

/// If both attribute and value are constants, check type compatibility.
fn check_value_type(pattern: &Pattern, schema: &Schema, diagnostics: &mut Vec<QueryDiagnostic>) {
    // Extract attribute keyword.
    let attr_str = match &pattern.attribute {
        Term::Attr(a) => a.as_str().to_string(),
        Term::Constant(Value::Keyword(kw)) => kw.clone(),
        _ => return,
    };

    // Look up the attribute definition.
    let def = match schema.attribute(&Attribute::from_keyword(&attr_str)) {
        Some(d) => d,
        None => return, // Unknown attr — already reported by check_attribute.
    };

    // Extract the value constant.
    let val = match &pattern.value {
        Term::Constant(v) => v,
        _ => return, // Variable — nothing to check.
    };

    // Check if value type matches.
    if def.value_type.matches(val) {
        return; // All good.
    }

    let expected = def.value_type;
    let got_type = val.type_name();
    let example = format_type_example(&attr_str, expected);

    diagnostics.push(QueryDiagnostic {
        kind: DiagnosticKind::TypeMismatch {
            attr: attr_str.clone(),
            expected: expected.as_keyword().to_string(),
            got: got_type.to_string(),
            example: example.clone(),
        },
        message: format!(
            "Type mismatch on `{}`: expected {} but got {}",
            attr_str,
            expected.as_keyword(),
            got_type,
        ),
        suggestion: Some(format!(
            "Use a {} value, e.g. {}",
            expected.as_keyword(),
            example
        )),
    });
}

/// Evaluate each clause independently and report per-clause match counts.
/// This helps the user identify which clause is the bottleneck.
fn check_clause_selectivity(
    store: &Store,
    query: &QueryExpr,
    diagnostics: &mut Vec<QueryDiagnostic>,
) {
    use std::collections::HashMap;

    let mut per_clause_counts = Vec::new();

    for clause in &query.where_clauses {
        let count = match clause {
            Clause::Pattern(pattern) => {
                let binding: HashMap<String, Value> = HashMap::new();
                count_pattern_matches(store, pattern, &binding)
            }
            Clause::Predicate { .. } => {
                // Predicates filter bindings — without input bindings we
                // cannot evaluate them standalone. Report 0.
                0
            }
        };
        per_clause_counts.push(count);
    }

    // Only emit if we have clauses and the pattern is informative.
    if per_clause_counts.is_empty() {
        return;
    }

    // Build human-readable summary.
    let clause_lines: Vec<String> = per_clause_counts
        .iter()
        .enumerate()
        .map(|(i, c)| format!("clause {}: {} matches", i, c))
        .collect();

    diagnostics.push(QueryDiagnostic {
        kind: DiagnosticKind::NoMatchingEntities {
            per_clause_counts: per_clause_counts.clone(),
        },
        message: format!(
            "All attributes and types are valid, but no entities satisfy all clauses together. \
             Per-clause match counts: {}",
            clause_lines.join("; ")
        ),
        suggestion: Some(
            "Try running each clause as a standalone query to see which \
             clause eliminates all results."
                .into(),
        ),
    });
}

/// Count how many datoms match a single pattern (independent evaluation).
fn count_pattern_matches(
    store: &Store,
    pattern: &Pattern,
    _binding: &std::collections::HashMap<String, Value>,
) -> usize {
    // Use the attribute index if possible for efficiency.
    let attr = match &pattern.attribute {
        Term::Attr(a) => Some(a.clone()),
        Term::Constant(Value::Keyword(kw)) => Some(Attribute::from_keyword(kw)),
        _ => None,
    };

    if let Some(ref a) = attr {
        let datoms = store.attribute_datoms(a);
        // If value is also a constant, further filter.
        if let Term::Constant(ref v) = pattern.value {
            return datoms.iter().filter(|d| &d.value == v).count();
        }
        return datoms.len();
    }

    // No attribute constant — full scan.
    store.datoms().count()
}

/// Produce an example value for the expected ValueType.
fn format_type_example(attr: &str, vtype: ValueType) -> String {
    match vtype {
        ValueType::Keyword => format!("Value::Keyword(\"{}\")", example_keyword(attr)),
        ValueType::String => "Value::String(\"some text\")".to_string(),
        ValueType::Long => "Value::Long(42)".to_string(),
        ValueType::Double => "Value::Double(3.14)".to_string(),
        ValueType::Boolean => "Value::Boolean(true)".to_string(),
        ValueType::Instant => "Value::Instant(1700000000000)".to_string(),
        ValueType::Ref => "Value::Ref(EntityId::from_ident(\":some/entity\"))".to_string(),
        ValueType::Uuid => "Value::Uuid([0u8; 16])".to_string(),
        ValueType::Bytes => "Value::Bytes(vec![1, 2, 3])".to_string(),
    }
}

/// Given an attribute keyword, produce a plausible example keyword value.
fn example_keyword(attr: &str) -> String {
    match attr {
        ":db/valueType" => ":db.type/string".to_string(),
        ":db/cardinality" => ":db.cardinality/one".to_string(),
        ":db/unique" => ":db.unique/identity".to_string(),
        ":spec/type" => ":spec.element/invariant".to_string(),
        _ => ":example/value".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Levenshtein edit distance
// ---------------------------------------------------------------------------

/// Compute Levenshtein edit distance between two strings.
///
/// Standard dynamic-programming implementation, O(|a| × |b|) time and O(|b|) space
/// (single-row optimization).
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    // prev[j] = edit distance between a[0..i-1] and b[0..j]
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1) // deletion
                .min(curr[j - 1] + 1) // insertion
                .min(prev[j - 1] + cost); // substitution
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{AgentId, ProvenanceType};
    use crate::query::clause::{FindSpec, Pattern, Term};
    use crate::store::Transaction;

    #[test]
    fn test_levenshtein_basic() {
        assert_eq!(levenshtein("", ""), 0);
        assert_eq!(levenshtein("a", ""), 1);
        assert_eq!(levenshtein("", "a"), 1);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("saturday", "sunday"), 3);
        assert_eq!(levenshtein("abc", "abc"), 0);
        assert_eq!(levenshtein("abc", "abd"), 1);
        assert_eq!(levenshtein(":db/type", ":db/valueType"), 6);
    }

    #[test]
    fn test_unknown_attribute_suggests_similar() {
        let mut store = Store::genesis();
        let agent = AgentId::from_name("test");

        // Add user data to get past the empty-store check (genesis has exactly 18 entities).
        let entity = crate::datom::EntityId::from_ident(":test/entity");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "test data")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("test doc".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        // Query with :db/valuType (typo, missing 'e') — should suggest :db/valueType
        let query = QueryExpr::new(
            FindSpec::Rel(vec!["?e".into(), "?v".into()]),
            vec![Clause::Pattern(Pattern::new(
                Term::Variable("?e".into()),
                Term::Attr(Attribute::from_keyword(":db/valuType")),
                Term::Variable("?v".into()),
            ))],
        );

        let diags = diagnose_empty_results(&store, &query);
        assert!(!diags.is_empty(), "should produce at least one diagnostic");

        let found_unknown = diags.iter().any(|d| {
            matches!(
                &d.kind,
                DiagnosticKind::UnknownAttribute { given, suggestions }
                    if given == ":db/valuType" && suggestions.contains(&":db/valueType".to_string())
            )
        });
        assert!(
            found_unknown,
            "should suggest :db/valueType for :db/valuType, got: {:?}",
            diags
        );
    }

    #[test]
    fn test_type_mismatch_string_vs_keyword() {
        // Build a store with some user data so we don't hit EmptyStore.
        let mut store = Store::genesis();
        let agent = AgentId::from_name("test");

        // Add user data to get past the empty-store check.
        let entity = crate::datom::EntityId::from_ident(":test/entity");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "test data")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("test doc".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        // Query :db/valueType with a String value instead of Keyword.
        // :db/valueType expects Keyword values.
        let query = QueryExpr::new(
            FindSpec::Rel(vec!["?e".into()]),
            vec![Clause::Pattern(Pattern::new(
                Term::Variable("?e".into()),
                Term::Attr(Attribute::from_keyword(":db/valueType")),
                Term::Constant(Value::String(":db.type/string".into())),
            ))],
        );

        let diags = diagnose_empty_results(&store, &query);
        let found_mismatch = diags.iter().any(|d| {
            matches!(
                &d.kind,
                DiagnosticKind::TypeMismatch { attr, expected, got, .. }
                    if attr == ":db/valueType"
                        && expected == ":db.type/keyword"
                        && got == "string"
            )
        });
        assert!(
            found_mismatch,
            "should detect type mismatch (String vs Keyword), got: {:?}",
            diags
        );
    }

    #[test]
    fn test_empty_store_detected() {
        let store = Store::genesis();

        let query = QueryExpr::new(
            FindSpec::Rel(vec!["?e".into(), "?v".into()]),
            vec![Clause::Pattern(Pattern::new(
                Term::Variable("?e".into()),
                Term::Attr(Attribute::from_keyword(":spec/type")),
                Term::Variable("?v".into()),
            ))],
        );

        let diags = diagnose_empty_results(&store, &query);
        let found_empty = diags
            .iter()
            .any(|d| matches!(&d.kind, DiagnosticKind::EmptyStore));
        assert!(found_empty, "should detect empty store, got: {:?}", diags);
    }

    #[test]
    fn test_valid_query_no_matching_entities() {
        // Build a store with user data so attributes are known and types match,
        // but the specific value doesn't exist.
        let mut store = Store::genesis();
        let agent = AgentId::from_name("test");

        let entity = crate::datom::EntityId::from_ident(":test/thing");
        let tx = Transaction::new(agent, ProvenanceType::Observed, "test data")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("hello world".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx).unwrap();

        // Query for a doc value that doesn't exist — attributes and types are fine.
        let query = QueryExpr::new(
            FindSpec::Rel(vec!["?e".into()]),
            vec![Clause::Pattern(Pattern::new(
                Term::Variable("?e".into()),
                Term::Attr(Attribute::from_keyword(":db/doc")),
                Term::Constant(Value::String("nonexistent doc value xyz".into())),
            ))],
        );

        let diags = diagnose_empty_results(&store, &query);
        let found_no_match = diags.iter().any(|d| {
            matches!(
                &d.kind,
                DiagnosticKind::NoMatchingEntities { per_clause_counts }
                    if !per_clause_counts.is_empty()
            )
        });
        assert!(
            found_no_match,
            "should produce NoMatchingEntities diagnostic, got: {:?}",
            diags
        );
    }
}
