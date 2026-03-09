//! Stratum classification for Datalog queries.
//!
//! Classifies queries into 6 strata (S0–S5) per spec/03-query.md §3.5–3.7.
//! Stage 0 evaluates S0 and S1 only; S2+ is classified but rejected at evaluation time.
//!
//! # Invariants
//!
//! - **INV-QUERY-005**: Strata 0-1 at Stage 0 (S2+ rejected).
//! - **INV-QUERY-006**: Entity-centric view via index scan.
//! - **INV-QUERY-007**: CALM compliance — S0/S1 are monotone, no coordination needed.

use super::clause::{Clause, QueryExpr};

/// The six strata of Datalog queries.
///
/// Forms a total order: S0 < S1 < S2 < S3 < S4 < S5.
/// Higher strata require more expressive evaluation engines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stratum {
    /// Ground queries — no variables, lookup only.
    S0,
    /// Monotone — positive body atoms only, CALM-compliant.
    S1,
    /// Stratified negation — negation allowed, requires stratification.
    S2,
    /// Aggregation — aggregation operators (count, sum, max, min).
    S3,
    /// Recursive aggregation — fixpoint with aggregation.
    S4,
    /// Full Datalog — unrestricted (may not terminate).
    S5,
}

impl Stratum {
    /// Whether this stratum can be evaluated at Stage 0.
    ///
    /// Only S0 and S1 are evaluable. S2+ requires higher-stage engines.
    pub fn is_evaluable_stage0(&self) -> bool {
        matches!(self, Stratum::S0 | Stratum::S1)
    }

    /// Whether this stratum represents a monotone query.
    ///
    /// S0 and S1 are monotone → CALM-compliant → no coordination needed.
    pub fn is_monotone(&self) -> bool {
        matches!(self, Stratum::S0 | Stratum::S1)
    }

    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            Stratum::S0 => "S0 (ground)",
            Stratum::S1 => "S1 (monotone)",
            Stratum::S2 => "S2 (stratified negation)",
            Stratum::S3 => "S3 (aggregation)",
            Stratum::S4 => "S4 (recursive aggregation)",
            Stratum::S5 => "S5 (full Datalog)",
        }
    }
}

/// Classify a query into its stratum.
///
/// The classification is conservative: if in doubt, classify higher.
/// This ensures we never accidentally evaluate a non-monotone query
/// as if it were monotone (safety property).
///
/// # Current Stage 0 classification
///
/// Our `QueryExpr` supports:
/// - Pattern matching (Clause::Pattern)
/// - Predicate filters (Clause::Predicate) with `=`, `!=`, `>`, `<`, `>=`, `<=`
///
/// Classification rules:
/// - Empty where clauses → S0 (ground)
/// - All clauses are patterns or equality predicates → S1 (monotone)
/// - Any negation predicate (`!=`) → S2 (stratified negation)
///
/// Note: S3–S5 require features not yet in our query AST (aggregation,
/// recursion), so they're unreachable from current `QueryExpr`. The
/// classifier still handles them for forward compatibility.
pub fn classify(query: &QueryExpr) -> Stratum {
    if query.where_clauses.is_empty() {
        return Stratum::S0;
    }

    let mut has_negation = false;

    for clause in &query.where_clauses {
        match clause {
            Clause::Pattern(_) => {
                // Patterns are always monotone (positive atoms)
            }
            Clause::Predicate { op, .. } => {
                if op == "!=" {
                    has_negation = true;
                }
                // Other predicates (=, >, <, >=, <=) are monotone filters
            }
        }
    }

    if has_negation {
        Stratum::S2
    } else {
        Stratum::S1
    }
}

/// Check whether a query can be evaluated at Stage 0.
///
/// Returns `Ok(stratum)` if evaluable, `Err(stratum)` if not.
pub fn check_stage0(query: &QueryExpr) -> Result<Stratum, Stratum> {
    let stratum = classify(query);
    if stratum.is_evaluable_stage0() {
        Ok(stratum)
    } else {
        Err(stratum)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{Attribute, Value};
    use crate::query::clause::{FindSpec, Pattern, Term};

    fn ground_query() -> QueryExpr {
        // Empty where clause → S0
        QueryExpr::new(FindSpec::Rel(vec!["?e".into()]), vec![])
    }

    fn simple_pattern_query() -> QueryExpr {
        // Single pattern → S1
        QueryExpr::new(
            FindSpec::Rel(vec!["?e".into()]),
            vec![Clause::Pattern(Pattern::new(
                Term::Variable("?e".into()),
                Term::Attr(Attribute::from_keyword(":db/doc")),
                Term::Variable("?v".into()),
            ))],
        )
    }

    fn equality_predicate_query() -> QueryExpr {
        // Pattern + equality predicate → S1 (monotone)
        QueryExpr::new(
            FindSpec::Rel(vec!["?e".into()]),
            vec![
                Clause::Pattern(Pattern::new(
                    Term::Variable("?e".into()),
                    Term::Attr(Attribute::from_keyword(":db/doc")),
                    Term::Variable("?v".into()),
                )),
                Clause::Predicate {
                    op: "=".to_string(),
                    args: vec![
                        Term::Variable("?v".into()),
                        Term::Constant(Value::String("test".into())),
                    ],
                },
            ],
        )
    }

    fn negation_query() -> QueryExpr {
        // Pattern + != predicate → S2
        QueryExpr::new(
            FindSpec::Rel(vec!["?e".into()]),
            vec![
                Clause::Pattern(Pattern::new(
                    Term::Variable("?e".into()),
                    Term::Attr(Attribute::from_keyword(":db/doc")),
                    Term::Variable("?v".into()),
                )),
                Clause::Predicate {
                    op: "!=".to_string(),
                    args: vec![
                        Term::Variable("?v".into()),
                        Term::Constant(Value::String("excluded".into())),
                    ],
                },
            ],
        )
    }

    #[test]
    fn classify_ground_query() {
        assert_eq!(classify(&ground_query()), Stratum::S0);
    }

    #[test]
    fn classify_simple_pattern() {
        assert_eq!(classify(&simple_pattern_query()), Stratum::S1);
    }

    #[test]
    fn classify_equality_predicate() {
        assert_eq!(classify(&equality_predicate_query()), Stratum::S1);
    }

    #[test]
    fn classify_negation() {
        assert_eq!(classify(&negation_query()), Stratum::S2);
    }

    #[test]
    fn stage0_accepts_s0() {
        assert_eq!(check_stage0(&ground_query()), Ok(Stratum::S0));
    }

    #[test]
    fn stage0_accepts_s1() {
        assert_eq!(check_stage0(&simple_pattern_query()), Ok(Stratum::S1));
    }

    #[test]
    fn stage0_rejects_s2() {
        assert_eq!(check_stage0(&negation_query()), Err(Stratum::S2));
    }

    #[test]
    fn stratum_ordering() {
        assert!(Stratum::S0 < Stratum::S1);
        assert!(Stratum::S1 < Stratum::S2);
        assert!(Stratum::S2 < Stratum::S3);
        assert!(Stratum::S3 < Stratum::S4);
        assert!(Stratum::S4 < Stratum::S5);
    }

    #[test]
    fn monotone_classification() {
        assert!(Stratum::S0.is_monotone());
        assert!(Stratum::S1.is_monotone());
        assert!(!Stratum::S2.is_monotone());
        assert!(!Stratum::S3.is_monotone());
        assert!(!Stratum::S4.is_monotone());
        assert!(!Stratum::S5.is_monotone());
    }

    #[test]
    fn evaluable_stage0_classification() {
        assert!(Stratum::S0.is_evaluable_stage0());
        assert!(Stratum::S1.is_evaluable_stage0());
        assert!(!Stratum::S2.is_evaluable_stage0());
        assert!(!Stratum::S3.is_evaluable_stage0());
        assert!(!Stratum::S4.is_evaluable_stage0());
        assert!(!Stratum::S5.is_evaluable_stage0());
    }

    #[test]
    fn classification_is_deterministic() {
        let q = simple_pattern_query();
        let s1 = classify(&q);
        let s2 = classify(&q);
        assert_eq!(s1, s2);
    }

    #[test]
    fn comparison_predicates_are_monotone() {
        // >, <, >=, <= are all monotone filters
        for op in &[">", "<", ">=", "<="] {
            let q = QueryExpr::new(
                FindSpec::Rel(vec!["?e".into()]),
                vec![
                    Clause::Pattern(Pattern::new(
                        Term::Variable("?e".into()),
                        Term::Attr(Attribute::from_keyword(":db/doc")),
                        Term::Variable("?v".into()),
                    )),
                    Clause::Predicate {
                        op: op.to_string(),
                        args: vec![Term::Variable("?v".into()), Term::Constant(Value::Long(42))],
                    },
                ],
            );
            assert_eq!(
                classify(&q),
                Stratum::S1,
                "predicate {op} should be classified as S1"
            );
        }
    }

    // -------------------------------------------------------------------
    // Proptest: stratum classification properties
    // -------------------------------------------------------------------

    mod stratum_proptests {
        use super::*;
        use crate::datom::{Attribute, Value};
        use crate::query::clause::{FindSpec, Pattern, Term};
        use proptest::prelude::*;

        fn arb_monotone_op() -> impl Strategy<Value = String> {
            prop_oneof![
                Just("=".to_string()),
                Just(">".to_string()),
                Just("<".to_string()),
                Just(">=".to_string()),
                Just("<=".to_string()),
            ]
        }

        fn arb_monotone_clause() -> impl Strategy<Value = Clause> {
            prop_oneof![
                Just(Clause::Pattern(Pattern::new(
                    Term::Variable("?e".into()),
                    Term::Attr(Attribute::from_keyword(":db/doc")),
                    Term::Variable("?v".into()),
                ))),
                arb_monotone_op().prop_map(|op| Clause::Predicate {
                    op,
                    args: vec![Term::Variable("?v".into()), Term::Constant(Value::Long(42)),],
                }),
            ]
        }

        fn arb_monotone_query() -> impl Strategy<Value = QueryExpr> {
            proptest::collection::vec(arb_monotone_clause(), 1..=5)
                .prop_map(|clauses| QueryExpr::new(FindSpec::Rel(vec!["?e".into()]), clauses))
        }

        fn arb_negation_query() -> impl Strategy<Value = QueryExpr> {
            proptest::collection::vec(arb_monotone_clause(), 0..=3).prop_map(|mut clauses| {
                clauses.push(Clause::Predicate {
                    op: "!=".to_string(),
                    args: vec![
                        Term::Variable("?v".into()),
                        Term::Constant(Value::String("excluded".into())),
                    ],
                });
                QueryExpr::new(FindSpec::Rel(vec!["?e".into()]), clauses)
            })
        }

        proptest! {
            #[test]
            fn check_stage0_accepts_monotonic(q in arb_monotone_query()) {
                let result = check_stage0(&q);
                prop_assert!(
                    result.is_ok(),
                    "check_stage0 must accept monotone queries, got Err({:?})",
                    result.unwrap_err()
                );
                let stratum = result.unwrap();
                prop_assert!(
                    stratum.is_monotone(),
                    "accepted stratum {:?} must be monotone",
                    stratum
                );
            }

            #[test]
            fn check_stage0_rejects_non_monotonic(q in arb_negation_query()) {
                let result = check_stage0(&q);
                prop_assert!(
                    result.is_err(),
                    "check_stage0 must reject non-monotone queries with != predicate"
                );
                let stratum = result.unwrap_err();
                prop_assert!(
                    !stratum.is_monotone(),
                    "rejected stratum {:?} must not be monotone",
                    stratum
                );
                prop_assert!(
                    stratum >= Stratum::S2,
                    "rejected stratum {:?} must be S2+",
                    stratum
                );
            }

            #[test]
            fn classify_is_deterministic(q in arb_monotone_query()) {
                let s1 = classify(&q);
                let s2 = classify(&q);
                prop_assert_eq!(
                    s1, s2,
                    "classify must be deterministic: {:?} vs {:?}",
                    s1, s2
                );
            }
        }
    }
}
