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
//! - **INV-QUERY-009**: Bilateral query symmetry.
//! - **INV-QUERY-011**: Projection reification.
//!
//! # Design Decisions
//!
//! - ADR-QUERY-003: Six-stratum classification.
//! - ADR-QUERY-005: Local frontier as default query scope.
//! - ADR-QUERY-006: Frontier as datom attribute.

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
/// - All clauses are patterns or predicate filters → S1 (monotone)
///
/// Key distinction: `!=` as a predicate filter on bound variables is NOT
/// stratified negation — it is a selection condition (like SQL WHERE x != y).
/// True S2 would require negation-as-failure (`not` clauses), which is not
/// yet in our AST. All current `Clause::Predicate` operations (including `!=`)
/// are therefore monotone filters and classified as S1.
///
/// Note: S2–S5 require features not yet in our query AST (negation-as-failure,
/// aggregation, recursion), so they're unreachable from current `QueryExpr`.
/// The classifier still handles them for forward compatibility.
pub fn classify(query: &QueryExpr) -> Stratum {
    if query.where_clauses.is_empty() {
        return Stratum::S0;
    }

    // Check for Stage 1+ clause variants that require higher strata.
    // Rule, Or, and Frontier clauses are at least S2 (non-monotonic potential).
    // When we add Clause::Not (negation-as-failure), we'll classify as S2.
    // When we add Clause::Aggregate, we'll classify as S3.
    for clause in &query.where_clauses {
        match clause {
            Clause::Pattern(_) | Clause::Predicate { .. } => {
                // S1: monotone pattern matching and predicate filters.
            }
            Clause::Rule { .. } | Clause::Or(_) | Clause::Frontier { .. } => {
                // S2: these clause types may introduce non-monotonic behavior
                // (rules can be recursive, or-branches may diverge, frontiers
                // compare across agent views). Classified as S2 to trigger
                // Stage 0 rejection via check_stage0.
                return Stratum::S2;
            }
        }
    }

    // All clauses are Pattern or Predicate — monotone, S1.
    Stratum::S1
}

/// Query evaluation mode (INV-QUERY-005).
///
/// Controls the evaluation strategy for the query engine. Each mode supports
/// a subset of the strata — `compatible_with()` encodes the compatibility
/// matrix.
///
/// # Compatibility Matrix
///
/// | Mode        | S0  | S1  | S2  | S3  | S4  | S5  |
/// |-------------|-----|-----|-----|-----|-----|-----|
/// | BottomUp    |  Y  |  Y  |  Y  |  N  |  N  |  N  |
/// | TopDown     |  Y  |  Y  |  Y  |  Y  |  Y  |  Y  |
/// | Incremental |  Y  |  Y  |  N  |  N  |  N  |  N  |
///
/// # Design Decisions
///
/// - ADR-QUERY-002: Naive bottom-up evaluation (with stratification) over top-down.
/// - ADR-QUERY-003: Six-stratum classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryMode {
    /// Naive bottom-up fixpoint evaluation. Stage 0 default.
    /// Supports S0–S2 (ground, monotone, stratified negation).
    BottomUp,
    /// Top-down (goal-directed) evaluation. Stage 1+.
    /// Supports all strata (S0–S5).
    TopDown,
    /// Incremental maintenance. Stage 2+.
    /// Supports S0–S1 (monotone only — CALM-compliant, no coordination needed).
    Incremental,
}

impl QueryMode {
    /// Whether this mode can evaluate queries at the given stratum.
    ///
    /// Encodes the mode-stratum compatibility matrix per INV-QUERY-005.
    ///
    /// **Falsification**: Returns `true` for a (mode, stratum) pair not in the
    /// compatibility matrix, or `false` for a pair that is in it.
    pub fn compatible_with(self, stratum: Stratum) -> bool {
        match self {
            // BottomUp: S0, S1, S2 — ground, monotone, stratified negation.
            // S3 (aggregation) requires top-down for efficient computation.
            QueryMode::BottomUp => matches!(stratum, Stratum::S0 | Stratum::S1 | Stratum::S2),
            // TopDown: all strata — goal-directed evaluation handles everything.
            QueryMode::TopDown => true,
            // Incremental: S0, S1 only — monotone queries are CALM-compliant
            // and can be maintained incrementally without coordination.
            QueryMode::Incremental => matches!(stratum, Stratum::S0 | Stratum::S1),
        }
    }

    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            QueryMode::BottomUp => "BottomUp (Stage 0 default)",
            QueryMode::TopDown => "TopDown (Stage 1+)",
            QueryMode::Incremental => "Incremental (Stage 2+)",
        }
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

// Witnesses: INV-QUERY-005, INV-QUERY-006, INV-QUERY-001,
// ADR-QUERY-003, NEG-QUERY-001, NEG-QUERY-003
#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{Attribute, Value};
    use crate::query::clause::{Clause, FindSpec, Pattern, Term};

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

    fn inequality_filter_query() -> QueryExpr {
        // Pattern + != predicate filter → S1 (filter, not negation-as-failure)
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

    // Verifies: ADR-QUERY-003 — Six-Stratum Classification
    #[test]
    fn classify_ground_query() {
        assert_eq!(classify(&ground_query()), Stratum::S0);
    }

    // Verifies: ADR-QUERY-003 — Six-Stratum Classification
    #[test]
    fn classify_simple_pattern() {
        assert_eq!(classify(&simple_pattern_query()), Stratum::S1);
    }

    // Verifies: ADR-QUERY-003 — Six-Stratum Classification
    #[test]
    fn classify_equality_predicate() {
        assert_eq!(classify(&equality_predicate_query()), Stratum::S1);
    }

    // Verifies: INV-QUERY-005 — Stratum Safety
    // Verifies: INV-QUERY-001 — CALM Compliance (monotone filter)
    #[test]
    fn classify_inequality_filter_is_monotone() {
        // != as a predicate filter is still monotone (S1), not S2.
        // S2 requires negation-as-failure (Clause::Not), not yet in AST.
        assert_eq!(classify(&inequality_filter_query()), Stratum::S1);
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
    fn stage0_accepts_inequality_filter() {
        // != as filter is evaluable at Stage 0
        assert_eq!(check_stage0(&inequality_filter_query()), Ok(Stratum::S1));
    }

    // Verifies: ADR-QUERY-003 — Six-Stratum Classification (ordering)
    #[test]
    fn stratum_ordering() {
        assert!(Stratum::S0 < Stratum::S1);
        assert!(Stratum::S1 < Stratum::S2);
        assert!(Stratum::S2 < Stratum::S3);
        assert!(Stratum::S3 < Stratum::S4);
        assert!(Stratum::S4 < Stratum::S5);
    }

    // Verifies: INV-QUERY-001 — CALM Compliance (monotone classification)
    // Verifies: NEG-QUERY-001 — No Non-Monotonic Queries in Monotonic Mode
    #[test]
    fn monotone_classification() {
        assert!(Stratum::S0.is_monotone());
        assert!(Stratum::S1.is_monotone());
        assert!(!Stratum::S2.is_monotone());
        assert!(!Stratum::S3.is_monotone());
        assert!(!Stratum::S4.is_monotone());
        assert!(!Stratum::S5.is_monotone());
    }

    // Verifies: INV-QUERY-005 — Stratum Safety (Stage 0 evaluability)
    #[test]
    fn evaluable_stage0_classification() {
        assert!(Stratum::S0.is_evaluable_stage0());
        assert!(Stratum::S1.is_evaluable_stage0());
        assert!(!Stratum::S2.is_evaluable_stage0());
        assert!(!Stratum::S3.is_evaluable_stage0());
        assert!(!Stratum::S4.is_evaluable_stage0());
        assert!(!Stratum::S5.is_evaluable_stage0());
    }

    // Verifies: INV-QUERY-002 — Query Determinism
    #[test]
    fn classification_is_deterministic() {
        let q = simple_pattern_query();
        let s1 = classify(&q);
        let s2 = classify(&q);
        assert_eq!(s1, s2);
    }

    // Verifies: INV-QUERY-001 — CALM Compliance (comparison predicates monotone)
    // Verifies: INV-QUERY-005 — Stratum Safety
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
    // QueryMode — Mode-Stratum Compatibility Matrix
    // Verifies: INV-QUERY-005 — Mode-Stratum Compatibility
    // Verifies: ADR-QUERY-002 — Naive Bottom-Up Evaluation
    // Verifies: ADR-QUERY-003 — Six-Stratum Classification
    // -------------------------------------------------------------------

    // Verifies: INV-QUERY-005 — BottomUp supports S0, S1, S2 only
    #[test]
    fn bottomup_compatibility() {
        assert!(QueryMode::BottomUp.compatible_with(Stratum::S0));
        assert!(QueryMode::BottomUp.compatible_with(Stratum::S1));
        assert!(QueryMode::BottomUp.compatible_with(Stratum::S2));
        assert!(!QueryMode::BottomUp.compatible_with(Stratum::S3));
        assert!(!QueryMode::BottomUp.compatible_with(Stratum::S4));
        assert!(!QueryMode::BottomUp.compatible_with(Stratum::S5));
    }

    // Verifies: INV-QUERY-005 — TopDown supports all strata
    #[test]
    fn topdown_compatibility() {
        assert!(QueryMode::TopDown.compatible_with(Stratum::S0));
        assert!(QueryMode::TopDown.compatible_with(Stratum::S1));
        assert!(QueryMode::TopDown.compatible_with(Stratum::S2));
        assert!(QueryMode::TopDown.compatible_with(Stratum::S3));
        assert!(QueryMode::TopDown.compatible_with(Stratum::S4));
        assert!(QueryMode::TopDown.compatible_with(Stratum::S5));
    }

    // Verifies: INV-QUERY-005 — Incremental supports S0, S1 only (CALM-compliant)
    #[test]
    fn incremental_compatibility() {
        assert!(QueryMode::Incremental.compatible_with(Stratum::S0));
        assert!(QueryMode::Incremental.compatible_with(Stratum::S1));
        assert!(!QueryMode::Incremental.compatible_with(Stratum::S2));
        assert!(!QueryMode::Incremental.compatible_with(Stratum::S3));
        assert!(!QueryMode::Incremental.compatible_with(Stratum::S4));
        assert!(!QueryMode::Incremental.compatible_with(Stratum::S5));
    }

    // Verifies: INV-QUERY-005 — Incremental is a subset of BottomUp capability
    #[test]
    fn incremental_subset_of_bottomup() {
        for stratum in &[
            Stratum::S0,
            Stratum::S1,
            Stratum::S2,
            Stratum::S3,
            Stratum::S4,
            Stratum::S5,
        ] {
            if QueryMode::Incremental.compatible_with(*stratum) {
                assert!(
                    QueryMode::BottomUp.compatible_with(*stratum),
                    "Incremental accepts {:?} but BottomUp does not — invariant violated",
                    stratum
                );
            }
        }
    }

    // Verifies: INV-QUERY-005 — BottomUp is a subset of TopDown capability
    #[test]
    fn bottomup_subset_of_topdown() {
        for stratum in &[
            Stratum::S0,
            Stratum::S1,
            Stratum::S2,
            Stratum::S3,
            Stratum::S4,
            Stratum::S5,
        ] {
            if QueryMode::BottomUp.compatible_with(*stratum) {
                assert!(
                    QueryMode::TopDown.compatible_with(*stratum),
                    "BottomUp accepts {:?} but TopDown does not — invariant violated",
                    stratum
                );
            }
        }
    }

    // Verifies: ADR-QUERY-002 — BottomUp as Stage 0 default supports Stage 0 evaluable strata
    #[test]
    fn bottomup_covers_stage0_evaluable() {
        assert!(QueryMode::BottomUp.compatible_with(Stratum::S0));
        assert!(QueryMode::BottomUp.compatible_with(Stratum::S1));
        // S0 and S1 are the Stage 0 evaluable strata
        assert!(Stratum::S0.is_evaluable_stage0());
        assert!(Stratum::S1.is_evaluable_stage0());
    }

    #[test]
    fn query_mode_names() {
        assert!(QueryMode::BottomUp.name().contains("BottomUp"));
        assert!(QueryMode::TopDown.name().contains("TopDown"));
        assert!(QueryMode::Incremental.name().contains("Incremental"));
    }

    // -------------------------------------------------------------------
    // Proptest: stratum classification properties
    // Verifies: INV-QUERY-005 — Stratum Safety
    // Verifies: INV-QUERY-006 — Fixpoint Termination
    // Verifies: NEG-QUERY-003 — No Unbounded Query Evaluation
    // -------------------------------------------------------------------

    mod stratum_proptests {
        use super::*;
        use crate::datom::{Attribute, Value};
        use crate::query::clause::{Clause, FindSpec, Pattern, Term};
        use proptest::prelude::*;

        fn arb_filter_op() -> impl Strategy<Value = String> {
            prop_oneof![
                Just("=".to_string()),
                Just("!=".to_string()),
                Just(">".to_string()),
                Just("<".to_string()),
                Just(">=".to_string()),
                Just("<=".to_string()),
            ]
        }

        fn arb_clause() -> impl Strategy<Value = Clause> {
            prop_oneof![
                Just(Clause::Pattern(Pattern::new(
                    Term::Variable("?e".into()),
                    Term::Attr(Attribute::from_keyword(":db/doc")),
                    Term::Variable("?v".into()),
                ))),
                arb_filter_op().prop_map(|op| Clause::Predicate {
                    op,
                    args: vec![Term::Variable("?v".into()), Term::Constant(Value::Long(42)),],
                }),
            ]
        }

        fn arb_query() -> impl Strategy<Value = QueryExpr> {
            proptest::collection::vec(arb_clause(), 1..=5)
                .prop_map(|clauses| QueryExpr::new(FindSpec::Rel(vec!["?e".into()]), clauses))
        }

        proptest! {
            #[test]
            fn all_current_ast_queries_are_stage0_evaluable(q in arb_query()) {
                // All expressible queries (patterns + predicate filters including !=)
                // are evaluable at Stage 0 because != is a filter, not negation-as-failure.
                let result = check_stage0(&q);
                prop_assert!(
                    result.is_ok(),
                    "check_stage0 must accept all current AST queries, got Err({:?})",
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
            fn classify_is_deterministic(q in arb_query()) {
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
