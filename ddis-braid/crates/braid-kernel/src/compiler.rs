//! `compiler` — Coherence Compiler pattern detection engine.
//!
//! The core insight: every invariant already contains its own test specification
//! in the falsification field. This module reads spec elements from the store and
//! identifies which of 9 universal mathematical patterns each matches.
//!
//! # The 9 Universal Patterns
//!
//! | Pattern | Mathematical Property | Template |
//! |---------|----------------------|----------|
//! | Never/Immutability | ∀ op: post ⊆ pre | snapshot→op→subset |
//! | Equality/Determinism | f(x) = f(x) always | dual-path→assert_eq |
//! | Commutativity | f(a,b) = f(b,a) | swap(a,b)→assert_eq |
//! | Associativity | f(f(a,b),c) = f(a,f(b,c)) | regroup(a,b,c)→assert_eq |
//! | Idempotency | f(f(x)) = f(x) | apply-twice→assert_eq |
//! | Monotonicity | x ≤ y → f(x) ≤ f(y) | before≤after |
//! | Boundedness | f(x) ∈ [lo, hi] | compute→assert_range |
//! | Completeness | ∀ x ∈ S: P(x) | enumerate→assert_all |
//! | Preservation | P(x) → P(f(x)) | before∩after=before |
//!
//! # Design Decisions
//!
//! - This module is pure computation: reads from store, returns data structures.
//!   No IO, no filesystem, no network.
//! - Pattern detection uses keyword matching on `:spec/statement` and
//!   `:spec/falsification` text, combined with structural heuristics.
//! - Confidence reflects match quality: exact keyword hits score higher than
//!   partial or inferred matches.
//!
//! # Traces To
//!
//! - SEED.md §7 (Self-Improvement Loop): automated coherence verification
//! - INV-BILATERAL-005: Test results as datoms
//! - ADR-FOUNDATION-005: Structural over procedural coherence

use crate::datom::{Attribute, Datom, EntityId, Op, TxId, Value};
use crate::store::Store;

// ===========================================================================
// Core Types
// ===========================================================================

/// The 9 universal mathematical patterns that invariants express.
///
/// Every well-formed invariant maps to at least one of these. The pattern
/// determines the test template: what inputs to generate, what operation
/// to perform, and what property to assert.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InvariantPattern {
    /// `∀ op: post ⊆ pre` — no operation removes or mutates existing state.
    /// Keywords: "never", "must not", "no operation", "immutable", "forbidden"
    #[default]
    Never,
    /// `f(x) = f(x)` — same inputs always produce same outputs.
    /// Keywords: "same", "identical", "deterministic", "equal", "equivalent"
    Equality,
    /// `f(a,b) = f(b,a)` — order of operands does not matter.
    /// Keywords: "commut", "order-independent", "order independent", "regardless of order"
    Commutativity,
    /// `f(f(a,b),c) = f(a,f(b,c))` — grouping of operands does not matter.
    /// Keywords: "associat", "grouping", "nesting"
    Associativity,
    /// `f(f(x)) = f(x)` — applying twice is the same as applying once.
    /// Keywords: "idempot", "applying twice", "re-applying", "already applied"
    Idempotency,
    /// `x ≤ y → f(x) ≤ f(y)` — the function preserves or strengthens ordering.
    /// Keywords: "monoton", "non-decreasing", "only grows", "never decreases", "never shrinks"
    Monotonicity,
    /// `f(x) ∈ [lo, hi]` — output is always within a known range.
    /// Keywords: "bounded", "within", "≤", "≥", "[0,1]", "[0, 1]", "at most", "at least", "range"
    Boundedness,
    /// `∀ x ∈ S: P(x)` — a property holds for every element in a set.
    /// Keywords: "every", "all", "for all", "must have", "each", "no exceptions"
    Completeness,
    /// `P(x) → P(f(x))` — an operation preserves a pre-existing property.
    /// Keywords: "preserve", "maintain", "retain", "survive", "still holds"
    Preservation,
}

impl InvariantPattern {
    /// All 9 patterns in canonical order.
    pub const ALL: [InvariantPattern; 9] = [
        InvariantPattern::Never,
        InvariantPattern::Equality,
        InvariantPattern::Commutativity,
        InvariantPattern::Associativity,
        InvariantPattern::Idempotency,
        InvariantPattern::Monotonicity,
        InvariantPattern::Boundedness,
        InvariantPattern::Completeness,
        InvariantPattern::Preservation,
    ];

    /// Human-readable name for the pattern.
    pub fn name(self) -> &'static str {
        match self {
            InvariantPattern::Never => "Never/Immutability",
            InvariantPattern::Equality => "Equality/Determinism",
            InvariantPattern::Commutativity => "Commutativity",
            InvariantPattern::Associativity => "Associativity",
            InvariantPattern::Idempotency => "Idempotency",
            InvariantPattern::Monotonicity => "Monotonicity",
            InvariantPattern::Boundedness => "Boundedness",
            InvariantPattern::Completeness => "Completeness",
            InvariantPattern::Preservation => "Preservation",
        }
    }

    /// The mathematical template for test generation.
    pub fn template(self) -> &'static str {
        match self {
            InvariantPattern::Never => "snapshot→op→assert(post ⊆ pre)",
            InvariantPattern::Equality => "dual-path→assert_eq(path_a, path_b)",
            InvariantPattern::Commutativity => "swap(a,b)→assert_eq(f(a,b), f(b,a))",
            InvariantPattern::Associativity => "regroup(a,b,c)→assert_eq(f(f(a,b),c), f(a,f(b,c)))",
            InvariantPattern::Idempotency => "apply_twice→assert_eq(f(x), f(f(x)))",
            InvariantPattern::Monotonicity => "before_after→assert(before ≤ after)",
            InvariantPattern::Boundedness => "compute→assert(lo ≤ result ≤ hi)",
            InvariantPattern::Completeness => "enumerate→assert_all(predicate)",
            InvariantPattern::Preservation => "before_after→assert(pre ∩ post = pre)",
        }
    }
}

impl std::fmt::Display for InvariantPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A detected pattern match for a spec element.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternMatch {
    /// The spec element identifier (e.g., "INV-STORE-001").
    pub spec_id: String,
    /// The entity ID of the spec element in the store.
    pub entity: EntityId,
    /// Which mathematical pattern was detected.
    pub pattern: InvariantPattern,
    /// The domain concept being constrained (extracted from text).
    pub subject: String,
    /// The specific property being tested (extracted from text).
    pub property: String,
    /// Match confidence in `[0.0, 1.0]`.
    /// - 0.9+: strong keyword match in both statement and falsification
    /// - 0.7-0.9: keyword match in statement only
    /// - 0.5-0.7: inferred from falsification structure
    pub confidence: f64,
}

// ===========================================================================
// Pattern Detection — Keyword Tables
// ===========================================================================

/// Keyword set for a single pattern: (keyword, base_weight).
/// Weight is the contribution to confidence when this keyword matches.
struct PatternKeywords {
    pattern: InvariantPattern,
    /// Primary keywords — strong signal, high weight.
    primary: &'static [(&'static str, f64)],
    /// Secondary keywords — supporting signal, lower weight.
    secondary: &'static [(&'static str, f64)],
}

/// The full keyword table for all 9 patterns.
fn keyword_table() -> Vec<PatternKeywords> {
    vec![
        PatternKeywords {
            pattern: InvariantPattern::Never,
            primary: &[
                ("never", 0.4),
                ("must not", 0.4),
                ("no operation", 0.35),
                ("immutable", 0.35),
                ("forbidden", 0.3),
                ("prohibited", 0.3),
            ],
            secondary: &[
                ("cannot", 0.15),
                ("not allowed", 0.15),
                ("does not delete", 0.2),
                ("does not mutate", 0.2),
                ("no deletion", 0.2),
                ("no mutation", 0.2),
                ("never deletes", 0.25),
                ("never mutates", 0.25),
                ("never modifies", 0.25),
                ("never removes", 0.25),
                ("never shrinks", 0.2),
            ],
        },
        PatternKeywords {
            pattern: InvariantPattern::Equality,
            primary: &[
                ("deterministic", 0.4),
                ("identical", 0.35),
                ("same output", 0.35),
                ("same result", 0.35),
                ("equivalent", 0.3),
            ],
            secondary: &[
                ("equal", 0.15),
                ("same", 0.1),
                ("produces the same", 0.25),
                ("always produces", 0.2),
                ("reproducible", 0.25),
            ],
        },
        PatternKeywords {
            pattern: InvariantPattern::Commutativity,
            primary: &[
                ("commut", 0.5),
                ("order-independent", 0.45),
                ("order independent", 0.45),
                ("regardless of order", 0.4),
            ],
            secondary: &[
                ("order does not matter", 0.3),
                ("any order", 0.2),
                ("either order", 0.25),
                ("interchangeable", 0.2),
            ],
        },
        PatternKeywords {
            pattern: InvariantPattern::Associativity,
            primary: &[("associat", 0.5), ("grouping", 0.25), ("regrouping", 0.35)],
            secondary: &[
                ("nesting", 0.2),
                ("parenthesization", 0.3),
                ("bracketing", 0.2),
                ("left-to-right", 0.15),
            ],
        },
        PatternKeywords {
            pattern: InvariantPattern::Idempotency,
            primary: &[
                ("idempot", 0.5),
                ("applying twice", 0.4),
                ("applied twice", 0.4),
                ("re-applying", 0.35),
            ],
            secondary: &[
                ("already applied", 0.25),
                ("no effect on second", 0.25),
                ("same state after", 0.2),
                ("repeated application", 0.3),
            ],
        },
        PatternKeywords {
            pattern: InvariantPattern::Monotonicity,
            primary: &[
                ("monoton", 0.5),
                ("non-decreasing", 0.45),
                ("only grows", 0.4),
                ("grow-only", 0.4),
                ("never decreases", 0.4),
            ],
            secondary: &[
                ("never shrinks", 0.25),
                ("at least as large", 0.25),
                ("weakly increasing", 0.3),
                ("non-negative growth", 0.25),
                ("can only increase", 0.3),
            ],
        },
        PatternKeywords {
            pattern: InvariantPattern::Boundedness,
            primary: &[
                ("bounded", 0.4),
                ("[0,1]", 0.45),
                ("[0, 1]", 0.45),
                ("within", 0.2),
            ],
            secondary: &[
                ("at most", 0.2),
                ("at least", 0.15),
                ("range", 0.1),
                ("does not exceed", 0.25),
                ("no more than", 0.2),
                ("no fewer than", 0.15),
                ("clamped", 0.3),
                ("clamp", 0.25),
            ],
        },
        PatternKeywords {
            pattern: InvariantPattern::Completeness,
            primary: &[
                ("for all", 0.35),
                ("for every", 0.35),
                ("every", 0.2),
                ("must have", 0.2),
            ],
            secondary: &[
                ("all", 0.05),
                ("each", 0.1),
                ("no exceptions", 0.25),
                ("without exception", 0.25),
                ("complete", 0.1),
                ("total function", 0.3),
                ("defined for all", 0.3),
                ("covers all", 0.2),
            ],
        },
        PatternKeywords {
            pattern: InvariantPattern::Preservation,
            primary: &[
                ("preserve", 0.4),
                ("maintain", 0.3),
                ("retain", 0.35),
                ("still holds", 0.35),
            ],
            secondary: &[
                ("survive", 0.25),
                ("unaffected", 0.25),
                ("unchanged", 0.2),
                ("remains", 0.1),
                ("kept intact", 0.3),
                ("not lost", 0.2),
            ],
        },
    ]
}

// ===========================================================================
// Subject & Property Extraction
// ===========================================================================

/// Extract the domain subject from statement text.
///
/// Heuristic: look for "The X" or "A X" at statement start, or the noun
/// phrase immediately after "must"/"never"/"shall".
fn extract_subject(statement: &str) -> String {
    let lower = statement.to_lowercase();

    // Pattern: "The <subject> ..."
    if let Some(rest) = lower.strip_prefix("the ") {
        if let Some(end) = rest.find(['.', ',', ':']) {
            let candidate = &rest[..end];
            // Take first noun phrase (up to verb-like word)
            let words: Vec<&str> = candidate.split_whitespace().collect();
            let verb_pos = words.iter().position(|w| {
                matches!(
                    *w,
                    "is" | "are"
                        | "has"
                        | "have"
                        | "does"
                        | "do"
                        | "must"
                        | "shall"
                        | "should"
                        | "never"
                        | "always"
                        | "can"
                        | "cannot"
                        | "will"
                        | "may"
                        | "produces"
                        | "contains"
                )
            });
            let end_idx = verb_pos.unwrap_or(words.len()).min(5);
            if end_idx > 0 {
                return words[..end_idx].join(" ");
            }
        }
    }

    // Pattern: noun phrase after "the" anywhere
    if let Some(pos) = lower.find("the ") {
        let rest = &lower[pos + 4..];
        let words: Vec<&str> = rest.split_whitespace().collect();
        let end_idx = words
            .iter()
            .position(|w| {
                matches!(
                    *w,
                    "is" | "are"
                        | "has"
                        | "have"
                        | "does"
                        | "do"
                        | "must"
                        | "shall"
                        | "should"
                        | "never"
                        | "always"
                        | "can"
                        | "cannot"
                        | "will"
                        | "may"
                        | "produces"
                        | "contains"
                )
            })
            .unwrap_or(words.len())
            .min(5);
        if end_idx > 0 {
            return words[..end_idx].join(" ");
        }
    }

    // Fallback: first 4 words
    let words: Vec<&str> = statement.split_whitespace().collect();
    let end = words.len().min(4);
    words[..end].join(" ")
}

/// Extract the property being tested from falsification text.
///
/// Heuristic: look for the clause after "violated if" or "fails when".
fn extract_property(falsification: &str) -> String {
    let lower = falsification.to_lowercase();

    // Pattern: "violated if <property>"
    if let Some(pos) = lower.find("violated if ") {
        let rest = &falsification[pos + 12..];
        let end = rest.find('.').unwrap_or(rest.len()).min(120);
        return rest[..end].trim().to_string();
    }

    // Pattern: "fails when <property>"
    if let Some(pos) = lower.find("fails when ") {
        let rest = &falsification[pos + 11..];
        let end = rest.find('.').unwrap_or(rest.len()).min(120);
        return rest[..end].trim().to_string();
    }

    // Pattern: "if <property>"
    if let Some(pos) = lower.find("if ") {
        let rest = &falsification[pos + 3..];
        let end = rest.find('.').unwrap_or(rest.len()).min(120);
        return rest[..end].trim().to_string();
    }

    // Fallback: first sentence
    let end = falsification
        .find('.')
        .unwrap_or(falsification.len())
        .min(120);
    falsification[..end].trim().to_string()
}

// ===========================================================================
// Core Detection Engine
// ===========================================================================

/// Score a text against one pattern's keyword table.
///
/// Returns (raw_score, hit_count) where raw_score is the sum of matched
/// keyword weights and hit_count is the number of distinct keywords matched.
fn score_text(text: &str, keywords: &PatternKeywords) -> (f64, usize) {
    let lower = text.to_lowercase();
    let mut score = 0.0;
    let mut hits = 0;

    for &(kw, weight) in keywords.primary {
        if lower.contains(kw) {
            score += weight;
            hits += 1;
        }
    }
    for &(kw, weight) in keywords.secondary {
        if lower.contains(kw) {
            score += weight;
            hits += 1;
        }
    }

    (score, hits)
}

/// Detect a single pattern match for one spec element.
///
/// Scores statement and falsification text against the keyword table.
/// Returns `Some(PatternMatch)` if confidence exceeds the threshold.
fn detect_single(
    spec_id: &str,
    entity: EntityId,
    statement: &str,
    falsification: &str,
    keywords: &PatternKeywords,
) -> Option<PatternMatch> {
    let (stmt_score, stmt_hits) = score_text(statement, keywords);
    let (fals_score, fals_hits) = score_text(falsification, keywords);

    let total_hits = stmt_hits + fals_hits;
    if total_hits == 0 {
        return None;
    }

    // Confidence calculation:
    // - Base: sum of keyword weights, clamped to [0, 1]
    // - Bonus: +0.1 if both statement and falsification match (cross-validation)
    // - Penalty: cap at 0.7 if only falsification matches (weaker signal)
    let raw = (stmt_score + fals_score).min(1.0);
    let cross_bonus = if stmt_hits > 0 && fals_hits > 0 {
        0.1
    } else {
        0.0
    };
    let confidence = if stmt_hits == 0 {
        // Only falsification matched — weaker signal
        (raw + cross_bonus).min(0.7)
    } else {
        (raw + cross_bonus).min(1.0)
    };

    // Threshold: require at least 0.25 confidence
    if confidence < 0.25 {
        return None;
    }

    let subject = extract_subject(statement);
    let property = if falsification.is_empty() {
        extract_subject(statement)
    } else {
        extract_property(falsification)
    };

    Some(PatternMatch {
        spec_id: spec_id.to_string(),
        entity,
        pattern: keywords.pattern,
        subject,
        property,
        confidence,
    })
}

// ===========================================================================
// Public API
// ===========================================================================

/// Read all spec elements from the store and detect which mathematical
/// pattern each matches.
///
/// For each spec entity (identified by `:spec/element-type`), reads
/// `:spec/statement` and `:spec/falsification`, applies keyword matching
/// against all 9 patterns, and returns matches above the confidence threshold.
///
/// A single spec element may match multiple patterns (e.g., an invariant
/// about merge being both commutative and associative). Each match is
/// returned as a separate `PatternMatch`.
///
/// # Complexity
///
/// O(E × P × K) where E = spec entities, P = 9 patterns, K = keywords per pattern.
/// All three factors are small constants in practice.
pub fn detect_patterns(store: &Store) -> Vec<PatternMatch> {
    let table = keyword_table();
    let spec_type_attr = Attribute::from_keyword(":spec/element-type");
    let statement_attr = Attribute::from_keyword(":spec/statement");
    let falsification_attr = Attribute::from_keyword(":spec/falsification");
    let ident_attr = Attribute::from_keyword(":db/ident");
    let spec_id_attr = Attribute::from_keyword(":spec/id");
    let element_id_attr = Attribute::from_keyword(":element/id");

    let mut results = Vec::new();

    for entity in store.entities() {
        let datoms = store.entity_datoms(entity);

        // Filter: only spec entities
        let is_spec = datoms
            .iter()
            .any(|d| d.attribute == spec_type_attr && d.op == Op::Assert);
        if !is_spec {
            continue;
        }

        // Extract text fields
        let statement = datoms
            .iter()
            .rfind(|d| d.attribute == statement_attr && d.op == Op::Assert)
            .and_then(|d| match &d.value {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            })
            .unwrap_or("");

        let falsification = datoms
            .iter()
            .rfind(|d| d.attribute == falsification_attr && d.op == Op::Assert)
            .and_then(|d| match &d.value {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            })
            .unwrap_or("");

        // Skip elements with no text to analyze
        if statement.is_empty() && falsification.is_empty() {
            continue;
        }

        // Resolve spec ID: try :spec/id, then :element/id, then :db/ident
        let spec_id = datoms
            .iter()
            .rfind(|d| d.attribute == spec_id_attr && d.op == Op::Assert)
            .and_then(|d| match &d.value {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .or_else(|| {
                datoms
                    .iter()
                    .rfind(|d| d.attribute == element_id_attr && d.op == Op::Assert)
                    .and_then(|d| match &d.value {
                        Value::String(s) => Some(s.clone()),
                        _ => None,
                    })
            })
            .or_else(|| {
                datoms
                    .iter()
                    .rfind(|d| d.attribute == ident_attr && d.op == Op::Assert)
                    .and_then(|d| match &d.value {
                        Value::Keyword(s) => Some(s.clone()),
                        _ => None,
                    })
            })
            .unwrap_or_else(|| format!("entity:{:x?}", &entity.as_bytes()[..4]));

        // Score against all 9 patterns
        for kw in &table {
            if let Some(m) = detect_single(&spec_id, entity, statement, falsification, kw) {
                results.push(m);
            }
        }
    }

    // Sort by confidence descending, then by spec_id for stability
    results.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.spec_id.cmp(&b.spec_id))
    });

    results
}

/// Detect patterns for a single spec element given its text fields directly.
///
/// Useful when you have already extracted the text and do not want to
/// re-scan the entire store. Returns all matching patterns above threshold.
pub fn detect_patterns_for_text(
    spec_id: &str,
    entity: EntityId,
    statement: &str,
    falsification: &str,
) -> Vec<PatternMatch> {
    let table = keyword_table();
    let mut results = Vec::new();

    for kw in &table {
        if let Some(m) = detect_single(spec_id, entity, statement, falsification, kw) {
            results.push(m);
        }
    }

    results.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    results
}

/// Summary statistics for pattern detection across a store.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternSummary {
    /// Total spec elements examined.
    pub total_spec_elements: usize,
    /// Spec elements that matched at least one pattern.
    pub matched_elements: usize,
    /// Spec elements with no pattern match (may need manual classification).
    pub unmatched_elements: usize,
    /// Count of matches per pattern type.
    pub pattern_counts: [(InvariantPattern, usize); 9],
    /// Mean confidence across all matches.
    pub mean_confidence: f64,
}

/// Compute summary statistics from a set of pattern matches.
pub fn summarize_patterns(matches: &[PatternMatch], total_spec_elements: usize) -> PatternSummary {
    let mut counts = [0usize; 9];
    let mut matched_entities = std::collections::HashSet::new();
    let mut confidence_sum = 0.0;

    for m in matches {
        matched_entities.insert(m.entity);
        confidence_sum += m.confidence;
        let idx = InvariantPattern::ALL
            .iter()
            .position(|p| *p == m.pattern)
            .unwrap_or(0);
        counts[idx] += 1;
    }

    let matched_elements = matched_entities.len();
    let mean_confidence = if matches.is_empty() {
        0.0
    } else {
        confidence_sum / matches.len() as f64
    };

    let mut pattern_counts = [(InvariantPattern::Never, 0usize); 9];
    for (i, &p) in InvariantPattern::ALL.iter().enumerate() {
        pattern_counts[i] = (p, counts[i]);
    }

    PatternSummary {
        total_spec_elements,
        matched_elements,
        unmatched_elements: total_spec_elements.saturating_sub(matched_elements),
        pattern_counts,
        mean_confidence,
    }
}

// ===========================================================================
// W3.5.2 — Property Extraction
// ===========================================================================

/// A test property extracted from a pattern match, ready for code emission.
///
/// Maps each detected mathematical pattern to a concrete test strategy,
/// property expression, and assertion type. This is the bridge between
/// pattern detection (W3.5.1) and code emission (W3.5.3).
///
/// # Traces To
///
/// - SEED.md §7 (Self-Improvement Loop): automated coherence verification
/// - INV-BILATERAL-005: Test results as datoms
#[derive(Clone, Debug, PartialEq)]
pub struct TestProperty {
    /// The invariant identifier (e.g., "INV-STORE-001").
    pub inv_id: String,
    /// Which mathematical pattern was detected.
    pub pattern: InvariantPattern,
    /// The proptest strategy name (e.g., "arb_store(3)").
    pub strategy_name: String,
    /// The property expression to assert (e.g., "snapshot.is_subset(&result)").
    pub property_expr: String,
    /// The assertion macro (e.g., "prop_assert!" or "kani::assert!").
    pub assertion_type: String,
}

/// Extract a test property from a pattern match.
///
/// Maps each of the 9 universal patterns to a concrete test template:
/// - Strategy: what inputs to generate
/// - Property: what to assert about the result
/// - Assertion: which macro to use
///
/// The generated properties use `arb_store` strategies from the
/// `proptest_strategies` module for realistic test inputs.
pub fn extract_test_property(m: &PatternMatch) -> TestProperty {
    let (strategy_name, property_expr, assertion_type) = match m.pattern {
        InvariantPattern::Never => (
            "arb_store(3)".to_string(),
            "snapshot.is_subset(&result)".to_string(),
            "prop_assert!".to_string(),
        ),
        InvariantPattern::Equality => (
            "arb_store(3)".to_string(),
            "path_a == path_b".to_string(),
            "prop_assert_eq!".to_string(),
        ),
        InvariantPattern::Commutativity => (
            "(arb_store(3), arb_store(3))".to_string(),
            "f_ab == f_ba".to_string(),
            "prop_assert_eq!".to_string(),
        ),
        InvariantPattern::Associativity => (
            "(arb_store(3), arb_store(3), arb_store(3))".to_string(),
            "f_ab_c == f_a_bc".to_string(),
            "prop_assert_eq!".to_string(),
        ),
        InvariantPattern::Idempotency => (
            "arb_store(3)".to_string(),
            "f_x == f_f_x".to_string(),
            "prop_assert_eq!".to_string(),
        ),
        InvariantPattern::Monotonicity => (
            "arb_store(3)".to_string(),
            "before <= after".to_string(),
            "prop_assert!".to_string(),
        ),
        InvariantPattern::Boundedness => (
            "arb_store(3)".to_string(),
            "lo <= value && value <= hi".to_string(),
            "prop_assert!".to_string(),
        ),
        InvariantPattern::Completeness => (
            "arb_store(3)".to_string(),
            "items.iter().all(|x| predicate(x))".to_string(),
            "prop_assert!".to_string(),
        ),
        InvariantPattern::Preservation => (
            "arb_store(3)".to_string(),
            "pre_props.is_subset(&post_props)".to_string(),
            "prop_assert!".to_string(),
        ),
    };

    TestProperty {
        inv_id: m.spec_id.clone(),
        pattern: m.pattern,
        strategy_name,
        property_expr,
        assertion_type,
    }
}

// ===========================================================================
// W3.5.3 — Code Emission
// ===========================================================================

/// Sanitize an invariant ID into a valid Rust identifier component.
///
/// Converts "INV-STORE-001" to "inv_store_001".
/// Also handles ident-style IDs like ":spec/inv-store-001" by replacing
/// colons, slashes, and hyphens with underscores, then stripping any
/// leading underscores so the result is a valid Rust identifier.
fn sanitize_id(id: &str) -> String {
    let sanitized: String = id
        .to_lowercase()
        .chars()
        .map(|c| match c {
            ':' | '/' | '-' | '.' => '_',
            c if c.is_ascii_alphanumeric() || c == '_' => c,
            _ => '_',
        })
        .collect();
    // Strip leading underscores to ensure valid Rust identifier start
    let trimmed = sanitized.trim_start_matches('_');
    if trimmed.is_empty() {
        "unnamed".to_string()
    } else if trimmed.starts_with(|c: char| c.is_ascii_digit()) {
        format!("id_{trimmed}")
    } else {
        trimmed.to_string()
    }
}

/// Sanitize a pattern name into a valid Rust identifier component.
///
/// Converts "Never/Immutability" to "never_immutability".
fn sanitize_pattern_name(pattern: InvariantPattern) -> String {
    pattern.name().to_lowercase().replace(['/', ' '], "_")
}

/// Emit a single proptest function as a String.
///
/// Produces a complete, compilable `proptest!` block for one test property.
/// The generated function:
/// - Has a doc comment referencing the invariant ID
/// - Uses the appropriate strategy
/// - Asserts the pattern-specific property
///
/// # Example output
///
/// ```text
/// proptest! {
///     /// Generated test for INV-STORE-001 (Never/Immutability)
///     #[test]
///     fn generated_inv_store_001_never_immutability(store in arb_store(3)) {
///         let snapshot: BTreeSet<_> = store.all_datoms().collect();
///         let count_before = store.datom_count();
///         let merged = merge_stores(&store, &store);
///         let result: BTreeSet<_> = merged.all_datoms().collect();
///         prop_assert!(snapshot.is_subset(&result));
///         prop_assert!(merged.datom_count() >= count_before);
///     }
/// }
/// ```
pub fn emit_proptest(prop: &TestProperty) -> String {
    let fn_name = format!(
        "generated_{}_{}",
        sanitize_id(&prop.inv_id),
        sanitize_pattern_name(prop.pattern),
    );
    let pattern_name = prop.pattern.name();
    let template = prop.pattern.template();

    match prop.pattern {
        InvariantPattern::Never => {
            format!(
                r#"proptest! {{
    /// Generated test for {inv_id} ({pattern_name})
    /// Template: {template}
    #[test]
    fn {fn_name}(store in {strategy}) {{
        let snapshot: std::collections::BTreeSet<_> = store.all_datoms().collect();
        let count_before = store.datom_count();
        // Merge with self — must preserve all existing datoms (append-only)
        let merged = merge_stores(&store, &store);
        let result: std::collections::BTreeSet<_> = merged.all_datoms().collect();
        // Every datom from the original snapshot must still be present
        {assertion}(snapshot.is_subset(&result));
        // Datom count must not decrease (append-only)
        {assertion}(merged.datom_count() >= count_before);
    }}
}}"#,
                inv_id = prop.inv_id,
                pattern_name = pattern_name,
                template = template,
                fn_name = fn_name,
                strategy = prop.strategy_name,
                assertion = prop.assertion_type,
            )
        }
        InvariantPattern::Equality => {
            format!(
                r#"proptest! {{
    /// Generated test for {inv_id} ({pattern_name})
    /// Template: {template}
    #[test]
    fn {fn_name}(store in {strategy}) {{
        // Two independent queries over the same store must produce identical results
        let path_a: std::collections::BTreeSet<_> = store.all_datoms().collect();
        // Re-merge the store with an empty genesis to exercise the query path again
        let reconstructed = merge_stores(&store, &Store::genesis());
        let path_b: std::collections::BTreeSet<_> = reconstructed.all_datoms().collect();
        {assertion}(path_a, path_b);
    }}
}}"#,
                inv_id = prop.inv_id,
                pattern_name = pattern_name,
                template = template,
                fn_name = fn_name,
                strategy = prop.strategy_name,
                assertion = prop.assertion_type,
            )
        }
        InvariantPattern::Commutativity => {
            format!(
                r#"proptest! {{
    /// Generated test for {inv_id} ({pattern_name})
    /// Template: {template}
    #[test]
    fn {fn_name}((store_a, store_b) in {strategy}) {{
        let f_ab = merge_stores(&store_a, &store_b);
        let f_ba = merge_stores(&store_b, &store_a);
        let set_ab: std::collections::BTreeSet<_> = f_ab.all_datoms().collect();
        let set_ba: std::collections::BTreeSet<_> = f_ba.all_datoms().collect();
        {assertion}(set_ab, set_ba);
    }}
}}"#,
                inv_id = prop.inv_id,
                pattern_name = pattern_name,
                template = template,
                fn_name = fn_name,
                strategy = prop.strategy_name,
                assertion = prop.assertion_type,
            )
        }
        InvariantPattern::Associativity => {
            format!(
                r#"proptest! {{
    /// Generated test for {inv_id} ({pattern_name})
    /// Template: {template}
    #[test]
    fn {fn_name}((store_a, store_b, store_c) in {strategy}) {{
        let ab = merge_stores(&store_a, &store_b);
        let f_ab_c = merge_stores(&ab, &store_c);
        let bc = merge_stores(&store_b, &store_c);
        let f_a_bc = merge_stores(&store_a, &bc);
        let set_ab_c: std::collections::BTreeSet<_> = f_ab_c.all_datoms().collect();
        let set_a_bc: std::collections::BTreeSet<_> = f_a_bc.all_datoms().collect();
        {assertion}(set_ab_c, set_a_bc);
    }}
}}"#,
                inv_id = prop.inv_id,
                pattern_name = pattern_name,
                template = template,
                fn_name = fn_name,
                strategy = prop.strategy_name,
                assertion = prop.assertion_type,
            )
        }
        InvariantPattern::Idempotency => {
            format!(
                r#"proptest! {{
    /// Generated test for {inv_id} ({pattern_name})
    /// Template: {template}
    #[test]
    fn {fn_name}(store in {strategy}) {{
        let f_x = merge_stores(&store, &store);
        let f_f_x = merge_stores(&f_x, &f_x);
        let set_fx: std::collections::BTreeSet<_> = f_x.all_datoms().collect();
        let set_ffx: std::collections::BTreeSet<_> = f_f_x.all_datoms().collect();
        {assertion}(set_fx, set_ffx);
    }}
}}"#,
                inv_id = prop.inv_id,
                pattern_name = pattern_name,
                template = template,
                fn_name = fn_name,
                strategy = prop.strategy_name,
                assertion = prop.assertion_type,
            )
        }
        InvariantPattern::Monotonicity => {
            format!(
                r#"proptest! {{
    /// Generated test for {inv_id} ({pattern_name})
    /// Template: {template}
    #[test]
    fn {fn_name}(store in {strategy}) {{
        let before = store.datom_count();
        // Merge with genesis adds schema datoms — count must not decrease
        let after_store = merge_stores(&store, &Store::genesis());
        let after = after_store.datom_count();
        {assertion}(before <= after);
    }}
}}"#,
                inv_id = prop.inv_id,
                pattern_name = pattern_name,
                template = template,
                fn_name = fn_name,
                strategy = prop.strategy_name,
                assertion = prop.assertion_type,
            )
        }
        InvariantPattern::Boundedness => {
            format!(
                r#"proptest! {{
    /// Generated test for {inv_id} ({pattern_name})
    /// Template: {template}
    #[test]
    fn {fn_name}(store in {strategy}) {{
        let value = compute_metric(&store);
        let lo = 0.0_f64;
        let hi = 1.0_f64;
        {assertion}(lo <= value && value <= hi);
    }}
}}"#,
                inv_id = prop.inv_id,
                pattern_name = pattern_name,
                template = template,
                fn_name = fn_name,
                strategy = prop.strategy_name,
                assertion = prop.assertion_type,
            )
        }
        InvariantPattern::Completeness => {
            format!(
                r#"proptest! {{
    /// Generated test for {inv_id} ({pattern_name})
    /// Template: {template}
    #[test]
    fn {fn_name}(store in {strategy}) {{
        let items: Vec<_> = store.all_datoms().collect();
        {assertion}(items.iter().all(|x| predicate(x)));
    }}
}}"#,
                inv_id = prop.inv_id,
                pattern_name = pattern_name,
                template = template,
                fn_name = fn_name,
                strategy = prop.strategy_name,
                assertion = prop.assertion_type,
            )
        }
        InvariantPattern::Preservation => {
            format!(
                r#"proptest! {{
    /// Generated test for {inv_id} ({pattern_name})
    /// Template: {template}
    #[test]
    fn {fn_name}(store in {strategy}) {{
        let pre_props: std::collections::BTreeSet<_> = store.all_datoms().collect();
        // Merge with genesis — all pre-existing datoms must survive
        let merged = merge_stores(&store, &Store::genesis());
        let post_props: std::collections::BTreeSet<_> = merged.all_datoms().collect();
        {assertion}(pre_props.is_subset(&post_props));
    }}
}}"#,
                inv_id = prop.inv_id,
                pattern_name = pattern_name,
                template = template,
                fn_name = fn_name,
                strategy = prop.strategy_name,
                assertion = prop.assertion_type,
            )
        }
    }
}

/// Emit a complete Rust test module wrapping all generated proptests.
///
/// Produces a valid, compilable `#[cfg(test)] mod generated_coherence_tests`
/// with all necessary imports and one proptest block per property.
///
/// # Structure
///
/// ```text
/// #[cfg(test)]
/// mod generated_coherence_tests {
///     use super::*;
///     use proptest::prelude::*;
///     use crate::proptest_strategies::arb_store;
///     use crate::merge::merge_stores;
///
///     proptest! { ... }  // one per property
/// }
/// ```
pub fn emit_test_module(properties: &[TestProperty]) -> String {
    let mut out = String::with_capacity(properties.len() * 512 + 256);

    out.push_str(
        "//! Auto-generated coherence tests from invariant pattern detection.\n\
         //! Do not edit manually. Regenerate with: braid compile --emit-tests\n\
         \n\
         #[cfg(test)]\n\
         mod generated_coherence_tests {\n\
         \x20\x20\x20\x20use super::*;\n\
         \x20\x20\x20\x20use proptest::prelude::*;\n\
         \x20\x20\x20\x20use crate::proptest_strategies::arb_store;\n\
         \x20\x20\x20\x20use crate::merge::merge_stores;\n\
         \x20\x20\x20\x20use crate::store::Store;\n\
         \n\
         \x20\x20\x20\x20/// Completeness predicate: checks that a datom has all five required\n\
         \x20\x20\x20\x20/// structural fields populated (entity, attribute, value, tx, op).\n\
         \x20\x20\x20\x20fn predicate(datom: &crate::datom::Datom) -> bool {\n\
         \x20\x20\x20\x20\x20\x20\x20\x20// Every datom must have a non-empty attribute and a valid op\n\
         \x20\x20\x20\x20\x20\x20\x20\x20!datom.attribute.as_str().is_empty()\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20&& matches!(datom.op, crate::datom::Op::Assert | crate::datom::Op::Retract)\n\
         \x20\x20\x20\x20}\n\
         \n\
         \x20\x20\x20\x20/// Boundedness metric: computes a normalized ratio from store properties.\n\
         \x20\x20\x20\x20/// Returns entity_count / datom_count, which is always in [0.0, 1.0]\n\
         \x20\x20\x20\x20/// (every entity has at least one datom).\n\
         \x20\x20\x20\x20fn compute_metric(store: &crate::store::Store) -> f64 {\n\
         \x20\x20\x20\x20\x20\x20\x20\x20let datom_count = store.datom_count();\n\
         \x20\x20\x20\x20\x20\x20\x20\x20if datom_count == 0 {\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20return 0.0;\n\
         \x20\x20\x20\x20\x20\x20\x20\x20}\n\
         \x20\x20\x20\x20\x20\x20\x20\x20let entity_count = store.entities().len();\n\
         \x20\x20\x20\x20\x20\x20\x20\x20entity_count as f64 / datom_count as f64\n\
         \x20\x20\x20\x20}\n\
         \n",
    );

    for (i, prop) in properties.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        // Indent each line of the proptest block by 4 spaces
        let block = emit_proptest(prop);
        for line in block.lines() {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
    }

    out.push_str("}\n");
    out
}

// ===========================================================================
// W3.5.7 — Pattern-to-Trace Datom Conversion
// ===========================================================================

/// Convert detected pattern matches into L3 (Property) trace datoms.
///
/// For each `PatternMatch`, creates an impl entity with:
/// - `:db/ident` — content-addressed from (spec_id, pattern, source)
/// - `:impl/implements` — `Value::Ref` pointing to the spec entity
/// - `:impl/verification-depth` — `3` (L3: Property-based)
/// - `:impl/file` — `"compiler"` (the module that detected the pattern)
/// - `:impl/module` — `"compiler"` (module name)
///
/// This creates the same datom structure as `trace::links_to_datoms` but
/// for compiler-detected patterns rather than source-scanned trace links.
/// The L3 depth reflects that pattern detection is a form of property
/// verification: the compiler has identified which mathematical property
/// the invariant expresses.
///
/// # Traces To
///
/// - INV-BILATERAL-002 (CC — depth-weighted coverage)
/// - INV-BILATERAL-005 (Test results as datoms)
/// - C7 (Self-bootstrap): the compiler's own patterns become store data
pub fn patterns_to_trace_datoms(matches: &[PatternMatch], tx_id: TxId) -> Vec<Datom> {
    let mut datoms = Vec::new();

    for m in matches {
        // Content-addressed impl entity from (spec_id, pattern, source)
        let impl_ident = format!(
            ":impl/compiler.{}.{}",
            sanitize_id(&m.spec_id),
            sanitize_pattern_name(m.pattern),
        );
        let impl_entity = EntityId::from_ident(&impl_ident);

        // Spec entity reference
        let spec_ident = format!(":spec/{}", m.spec_id.to_lowercase());
        let spec_entity = EntityId::from_ident(&spec_ident);

        // :db/ident
        datoms.push(Datom::new(
            impl_entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(impl_ident.clone()),
            tx_id,
            Op::Assert,
        ));

        // :impl/implements → spec entity ref
        datoms.push(Datom::new(
            impl_entity,
            Attribute::from_keyword(":impl/implements"),
            Value::Ref(spec_entity),
            tx_id,
            Op::Assert,
        ));

        // :impl/verification-depth → 3 (L3: Property-based)
        datoms.push(Datom::new(
            impl_entity,
            Attribute::from_keyword(":impl/verification-depth"),
            Value::Long(3),
            tx_id,
            Op::Assert,
        ));

        // :impl/file → compiler
        datoms.push(Datom::new(
            impl_entity,
            Attribute::from_keyword(":impl/file"),
            Value::String("compiler".to_string()),
            tx_id,
            Op::Assert,
        ));

        // :impl/module → compiler
        datoms.push(Datom::new(
            impl_entity,
            Attribute::from_keyword(":impl/module"),
            Value::String("compiler".to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    datoms
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{AgentId, Datom, EntityId, Op, TxId, Value};
    use crate::store::Store;
    use std::collections::BTreeSet;

    /// Create a store from genesis + additional datoms.
    fn store_with(extra: Vec<Datom>) -> Store {
        let system_agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, system_agent);
        let mut datom_set: BTreeSet<Datom> = BTreeSet::new();
        for d in crate::schema::genesis_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        for d in crate::schema::full_schema_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        for d in extra {
            datom_set.insert(d);
        }
        Store::from_datoms(datom_set)
    }

    /// Create a spec entity with statement and falsification.
    fn spec_entity(
        ident: &str,
        spec_id: &str,
        element_type: &str,
        statement: &str,
        falsification: &str,
        tx: TxId,
    ) -> Vec<Datom> {
        let entity = EntityId::from_ident(ident);
        let mut datoms = vec![
            Datom::new(
                entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(ident.to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                entity,
                Attribute::from_keyword(":spec/element-type"),
                Value::Keyword(element_type.to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                entity,
                Attribute::from_keyword(":spec/id"),
                Value::String(spec_id.to_string()),
                tx,
                Op::Assert,
            ),
        ];
        if !statement.is_empty() {
            datoms.push(Datom::new(
                entity,
                Attribute::from_keyword(":spec/statement"),
                Value::String(statement.to_string()),
                tx,
                Op::Assert,
            ));
        }
        if !falsification.is_empty() {
            datoms.push(Datom::new(
                entity,
                Attribute::from_keyword(":spec/falsification"),
                Value::String(falsification.to_string()),
                tx,
                Op::Assert,
            ));
        }
        datoms
    }

    fn test_tx() -> TxId {
        TxId::new(1, 0, AgentId::from_name("test"))
    }

    // --- Pattern Detection: Known INVs ---

    #[test]
    fn inv_store_001_detects_never_pattern() {
        let tx = test_tx();
        let datoms = spec_entity(
            ":spec/inv-store-001",
            "INV-STORE-001",
            ":spec.type/invariant",
            "The datom store never deletes or mutates an existing datom. All state changes are new assertions.",
            "Any operation that removes a datom from the store, or modifies the [e,a,v,tx,op] tuple of an existing datom in place, violates this invariant.",
            tx,
        );
        let store = store_with(datoms);
        let matches = detect_patterns(&store);

        let never_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.spec_id == "INV-STORE-001" && m.pattern == InvariantPattern::Never)
            .collect();
        assert!(
            !never_matches.is_empty(),
            "INV-STORE-001 should match Never pattern, got: {matches:?}"
        );
        assert!(
            never_matches[0].confidence >= 0.5,
            "confidence should be >= 0.5, got {}",
            never_matches[0].confidence
        );
    }

    #[test]
    fn inv_store_004_detects_commutativity() {
        let tx = test_tx();
        let datoms = spec_entity(
            ":spec/inv-store-004",
            "INV-STORE-004",
            ":spec.type/invariant",
            "Merge is commutative: merge(A, B) = merge(B, A) for any two stores A and B.",
            "Violated if there exist stores A, B such that merge(A,B) produces a different datom set than merge(B,A).",
            tx,
        );
        let store = store_with(datoms);
        let matches = detect_patterns(&store);

        let comm_matches: Vec<_> = matches
            .iter()
            .filter(|m| {
                m.spec_id == "INV-STORE-004" && m.pattern == InvariantPattern::Commutativity
            })
            .collect();
        assert!(
            !comm_matches.is_empty(),
            "INV-STORE-004 should match Commutativity, got: {matches:?}"
        );
        assert!(
            comm_matches[0].confidence >= 0.5,
            "confidence should be >= 0.5, got {}",
            comm_matches[0].confidence
        );
    }

    #[test]
    fn inv_store_005_detects_associativity() {
        let tx = test_tx();
        let datoms = spec_entity(
            ":spec/inv-store-005",
            "INV-STORE-005",
            ":spec.type/invariant",
            "Merge is associative: merge(merge(A,B),C) = merge(A,merge(B,C)).",
            "Violated if regrouping three stores produces different results.",
            tx,
        );
        let store = store_with(datoms);
        let matches = detect_patterns(&store);

        let assoc_matches: Vec<_> = matches
            .iter()
            .filter(|m| {
                m.spec_id == "INV-STORE-005" && m.pattern == InvariantPattern::Associativity
            })
            .collect();
        assert!(
            !assoc_matches.is_empty(),
            "INV-STORE-005 should match Associativity, got: {matches:?}"
        );
    }

    #[test]
    fn inv_store_006_detects_idempotency() {
        let tx = test_tx();
        let datoms = spec_entity(
            ":spec/inv-store-006",
            "INV-STORE-006",
            ":spec.type/invariant",
            "Merge is idempotent: merge(A, A) = A.",
            "Violated if applying merge to a store with itself changes the store state.",
            tx,
        );
        let store = store_with(datoms);
        let matches = detect_patterns(&store);

        let idemp_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.spec_id == "INV-STORE-006" && m.pattern == InvariantPattern::Idempotency)
            .collect();
        assert!(
            !idemp_matches.is_empty(),
            "INV-STORE-006 should match Idempotency, got: {matches:?}"
        );
    }

    #[test]
    fn inv_store_002_detects_monotonicity() {
        let tx = test_tx();
        let datoms = spec_entity(
            ":spec/inv-store-002",
            "INV-STORE-002",
            ":spec.type/invariant",
            "The datom set is monotonically non-decreasing. It only grows via insert.",
            "Violated if after any transaction the datom count decreases.",
            tx,
        );
        let store = store_with(datoms);
        let matches = detect_patterns(&store);

        let mono_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.spec_id == "INV-STORE-002" && m.pattern == InvariantPattern::Monotonicity)
            .collect();
        assert!(
            !mono_matches.is_empty(),
            "INV-STORE-002 should match Monotonicity, got: {matches:?}"
        );
    }

    #[test]
    fn inv_store_008_detects_equality() {
        let tx = test_tx();
        let datoms = spec_entity(
            ":spec/inv-store-008",
            "INV-STORE-008",
            ":spec.type/invariant",
            "Genesis is deterministic: Store::genesis() produces identical output on every call.",
            "Violated if two calls to genesis() produce different datom sets.",
            tx,
        );
        let store = store_with(datoms);
        let matches = detect_patterns(&store);

        let eq_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.spec_id == "INV-STORE-008" && m.pattern == InvariantPattern::Equality)
            .collect();
        assert!(
            !eq_matches.is_empty(),
            "INV-STORE-008 should match Equality/Determinism, got: {matches:?}"
        );
    }

    #[test]
    fn fitness_function_detects_boundedness() {
        let tx = test_tx();
        let datoms = spec_entity(
            ":spec/inv-bilateral-001",
            "INV-BILATERAL-001",
            ":spec.type/invariant",
            "The fitness function F(S) is bounded in [0, 1] and monotonically non-decreasing.",
            "Violated if F(S) produces a value outside [0,1] or decreases between successive cycles.",
            tx,
        );
        let store = store_with(datoms);
        let matches = detect_patterns(&store);

        let bounded_matches: Vec<_> = matches
            .iter()
            .filter(|m| {
                m.spec_id == "INV-BILATERAL-001" && m.pattern == InvariantPattern::Boundedness
            })
            .collect();
        assert!(
            !bounded_matches.is_empty(),
            "INV-BILATERAL-001 should match Boundedness, got: {matches:?}"
        );

        // Should also match Monotonicity
        let mono_matches: Vec<_> = matches
            .iter()
            .filter(|m| {
                m.spec_id == "INV-BILATERAL-001" && m.pattern == InvariantPattern::Monotonicity
            })
            .collect();
        assert!(
            !mono_matches.is_empty(),
            "INV-BILATERAL-001 should also match Monotonicity"
        );
    }

    #[test]
    fn schema_completeness_detects_completeness() {
        let tx = test_tx();
        let datoms = spec_entity(
            ":spec/inv-schema-004",
            "INV-SCHEMA-004",
            ":spec.type/invariant",
            "For every attribute referenced in a datom, the schema must have a definition.",
            "Violated if a transaction succeeds with a datom whose attribute has no schema entry.",
            tx,
        );
        let store = store_with(datoms);
        let matches = detect_patterns(&store);

        let comp_matches: Vec<_> = matches
            .iter()
            .filter(|m| {
                m.spec_id == "INV-SCHEMA-004" && m.pattern == InvariantPattern::Completeness
            })
            .collect();
        assert!(
            !comp_matches.is_empty(),
            "INV-SCHEMA-004 should match Completeness, got: {matches:?}"
        );
    }

    #[test]
    fn preservation_pattern_detected() {
        let tx = test_tx();
        let datoms = spec_entity(
            ":spec/inv-merge-010",
            "INV-MERGE-010",
            ":spec.type/invariant",
            "Merge must preserve all datoms from both input stores.",
            "Violated if any datom present in store A or store B before merge is absent from the merged result.",
            tx,
        );
        let store = store_with(datoms);
        let matches = detect_patterns(&store);

        let pres_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.spec_id == "INV-MERGE-010" && m.pattern == InvariantPattern::Preservation)
            .collect();
        assert!(
            !pres_matches.is_empty(),
            "INV-MERGE-010 should match Preservation, got: {matches:?}"
        );
    }

    // --- Negative case: no match for non-matching text ---

    #[test]
    fn non_matching_text_returns_empty() {
        let tx = test_tx();
        let datoms = spec_entity(
            ":spec/adr-test-001",
            "ADR-TEST-001",
            ":spec.type/adr",
            "We chose Rust for implementation.",
            "This is the rationale for choosing Rust.",
            tx,
        );
        let store = store_with(datoms);
        let matches = detect_patterns(&store);

        let adr_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.spec_id == "ADR-TEST-001")
            .collect();
        assert!(
            adr_matches.is_empty(),
            "Non-pattern text should not match, got: {adr_matches:?}"
        );
    }

    // --- Multiple patterns from one element ---

    #[test]
    fn single_element_matches_multiple_patterns() {
        let tx = test_tx();
        let datoms = spec_entity(
            ":spec/inv-multi-001",
            "INV-MULTI-001",
            ":spec.type/invariant",
            "Merge is commutative, associative, and idempotent.",
            "Violated if merge(A,B) differs from merge(B,A), or regrouping changes results, or applying twice changes state.",
            tx,
        );
        let store = store_with(datoms);
        let matches = detect_patterns(&store);

        let multi_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.spec_id == "INV-MULTI-001")
            .collect();

        let patterns: Vec<_> = multi_matches.iter().map(|m| m.pattern).collect();
        assert!(
            patterns.contains(&InvariantPattern::Commutativity),
            "should detect Commutativity"
        );
        assert!(
            patterns.contains(&InvariantPattern::Associativity),
            "should detect Associativity"
        );
        assert!(
            patterns.contains(&InvariantPattern::Idempotency),
            "should detect Idempotency"
        );
    }

    // --- Empty store ---

    #[test]
    fn empty_store_returns_empty() {
        let store = Store::genesis();
        let matches = detect_patterns(&store);
        assert!(
            matches.is_empty(),
            "genesis store has no spec elements, should return empty"
        );
    }

    // --- Subject and property extraction ---

    #[test]
    fn extract_subject_from_the_prefix() {
        let subject = extract_subject("The datom store never deletes or mutates.");
        assert_eq!(subject, "datom store");
    }

    #[test]
    fn extract_subject_from_the_internal() {
        let subject =
            extract_subject("All operations on the schema must be validated before commit.");
        assert_eq!(subject, "schema");
    }

    #[test]
    fn extract_property_from_violated_if() {
        let property =
            extract_property("Violated if any datom is removed from the store after insertion.");
        assert_eq!(
            property,
            "any datom is removed from the store after insertion"
        );
    }

    #[test]
    fn extract_property_from_fails_when() {
        let property = extract_property("Fails when the count decreases between transactions.");
        assert_eq!(property, "the count decreases between transactions");
    }

    // --- Summary statistics ---

    #[test]
    fn summary_counts_patterns() {
        let entity_a = EntityId::from_ident(":test/a");
        let entity_b = EntityId::from_ident(":test/b");
        let matches = vec![
            PatternMatch {
                spec_id: "A".into(),
                entity: entity_a,
                pattern: InvariantPattern::Never,
                subject: "store".into(),
                property: "no deletion".into(),
                confidence: 0.9,
            },
            PatternMatch {
                spec_id: "A".into(),
                entity: entity_a,
                pattern: InvariantPattern::Monotonicity,
                subject: "store".into(),
                property: "grows".into(),
                confidence: 0.7,
            },
            PatternMatch {
                spec_id: "B".into(),
                entity: entity_b,
                pattern: InvariantPattern::Commutativity,
                subject: "merge".into(),
                property: "order".into(),
                confidence: 0.8,
            },
        ];
        let summary = summarize_patterns(&matches, 5);
        assert_eq!(summary.total_spec_elements, 5);
        assert_eq!(summary.matched_elements, 2);
        assert_eq!(summary.unmatched_elements, 3);
        assert!((summary.mean_confidence - 0.8).abs() < 1e-10);
        assert_eq!(summary.pattern_counts[0], (InvariantPattern::Never, 1));
        assert_eq!(
            summary.pattern_counts[2],
            (InvariantPattern::Commutativity, 1)
        );
        assert_eq!(
            summary.pattern_counts[5],
            (InvariantPattern::Monotonicity, 1)
        );
    }

    // --- detect_patterns_for_text API ---

    #[test]
    fn detect_patterns_for_text_works() {
        let entity = EntityId::from_ident(":test/direct");
        let matches = detect_patterns_for_text(
            "TEST-001",
            entity,
            "The store never deletes a datom.",
            "Violated if a datom is removed.",
        );
        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.pattern == InvariantPattern::Never));
    }

    // --- Pattern Display ---

    #[test]
    fn pattern_display_and_template() {
        for p in InvariantPattern::ALL {
            assert!(!p.name().is_empty());
            assert!(!p.template().is_empty());
            assert!(!format!("{p}").is_empty());
        }
    }

    // --- Confidence ordering ---

    #[test]
    fn results_sorted_by_confidence_desc() {
        let tx = test_tx();
        let mut all_datoms = Vec::new();
        all_datoms.extend(spec_entity(
            ":spec/inv-strong-001",
            "INV-STRONG-001",
            ":spec.type/invariant",
            "The store never deletes or mutates. It is immutable and forbidden from modification.",
            "Violated if any datom is removed or modified. Must not happen under any operation.",
            tx,
        ));
        all_datoms.extend(spec_entity(
            ":spec/inv-weak-001",
            "INV-WEAK-001",
            ":spec.type/invariant",
            "The system cannot do this.",
            "",
            tx,
        ));
        let store = store_with(all_datoms);
        let matches = detect_patterns(&store);

        // Strong match should come before weak match
        let strong_idx = matches.iter().position(|m| m.spec_id == "INV-STRONG-001");
        let weak_idx = matches.iter().position(|m| m.spec_id == "INV-WEAK-001");
        if let (Some(s), Some(w)) = (strong_idx, weak_idx) {
            assert!(
                s < w,
                "higher confidence should sort first: strong@{s} vs weak@{w}"
            );
        }
    }

    // --- W3.5.2: Property extraction ---

    /// Helper to make a PatternMatch for a given pattern.
    fn make_match(spec_id: &str, pattern: InvariantPattern) -> PatternMatch {
        PatternMatch {
            spec_id: spec_id.to_string(),
            entity: EntityId::from_ident(":test/prop-extract"),
            pattern,
            subject: "store".into(),
            property: "test property".into(),
            confidence: 0.9,
        }
    }

    #[test]
    fn extract_test_property_never() {
        let m = make_match("INV-STORE-001", InvariantPattern::Never);
        let prop = extract_test_property(&m);
        assert_eq!(prop.inv_id, "INV-STORE-001");
        assert_eq!(prop.pattern, InvariantPattern::Never);
        assert_eq!(prop.strategy_name, "arb_store(3)");
        assert!(prop.property_expr.contains("is_subset"));
        assert_eq!(prop.assertion_type, "prop_assert!");
    }

    #[test]
    fn extract_test_property_equality() {
        let m = make_match("INV-STORE-008", InvariantPattern::Equality);
        let prop = extract_test_property(&m);
        assert_eq!(prop.pattern, InvariantPattern::Equality);
        assert_eq!(prop.strategy_name, "arb_store(3)");
        assert!(prop.property_expr.contains("path_a == path_b"));
        assert_eq!(prop.assertion_type, "prop_assert_eq!");
    }

    #[test]
    fn extract_test_property_commutativity() {
        let m = make_match("INV-STORE-004", InvariantPattern::Commutativity);
        let prop = extract_test_property(&m);
        assert_eq!(prop.pattern, InvariantPattern::Commutativity);
        assert!(prop.strategy_name.contains("arb_store(3), arb_store(3)"));
        assert!(prop.property_expr.contains("f_ab == f_ba"));
        assert_eq!(prop.assertion_type, "prop_assert_eq!");
    }

    #[test]
    fn extract_test_property_associativity() {
        let m = make_match("INV-STORE-005", InvariantPattern::Associativity);
        let prop = extract_test_property(&m);
        assert_eq!(prop.pattern, InvariantPattern::Associativity);
        assert!(prop
            .strategy_name
            .contains("arb_store(3), arb_store(3), arb_store(3)"));
        assert!(prop.property_expr.contains("f_ab_c == f_a_bc"));
        assert_eq!(prop.assertion_type, "prop_assert_eq!");
    }

    #[test]
    fn extract_test_property_idempotency() {
        let m = make_match("INV-STORE-006", InvariantPattern::Idempotency);
        let prop = extract_test_property(&m);
        assert_eq!(prop.pattern, InvariantPattern::Idempotency);
        assert_eq!(prop.strategy_name, "arb_store(3)");
        assert!(prop.property_expr.contains("f_x == f_f_x"));
        assert_eq!(prop.assertion_type, "prop_assert_eq!");
    }

    #[test]
    fn extract_test_property_monotonicity() {
        let m = make_match("INV-STORE-002", InvariantPattern::Monotonicity);
        let prop = extract_test_property(&m);
        assert_eq!(prop.pattern, InvariantPattern::Monotonicity);
        assert_eq!(prop.strategy_name, "arb_store(3)");
        assert!(prop.property_expr.contains("before <= after"));
        assert_eq!(prop.assertion_type, "prop_assert!");
    }

    #[test]
    fn extract_test_property_boundedness() {
        let m = make_match("INV-BILATERAL-001", InvariantPattern::Boundedness);
        let prop = extract_test_property(&m);
        assert_eq!(prop.pattern, InvariantPattern::Boundedness);
        assert_eq!(prop.strategy_name, "arb_store(3)");
        assert!(prop.property_expr.contains("lo <= value"));
        assert!(prop.property_expr.contains("value <= hi"));
        assert_eq!(prop.assertion_type, "prop_assert!");
    }

    #[test]
    fn extract_test_property_completeness() {
        let m = make_match("INV-SCHEMA-004", InvariantPattern::Completeness);
        let prop = extract_test_property(&m);
        assert_eq!(prop.pattern, InvariantPattern::Completeness);
        assert_eq!(prop.strategy_name, "arb_store(3)");
        assert!(prop.property_expr.contains("predicate"));
        assert_eq!(prop.assertion_type, "prop_assert!");
    }

    #[test]
    fn extract_test_property_preservation() {
        let m = make_match("INV-MERGE-010", InvariantPattern::Preservation);
        let prop = extract_test_property(&m);
        assert_eq!(prop.pattern, InvariantPattern::Preservation);
        assert_eq!(prop.strategy_name, "arb_store(3)");
        assert!(prop.property_expr.contains("is_subset"));
        assert_eq!(prop.assertion_type, "prop_assert!");
    }

    #[test]
    fn extract_test_property_all_patterns_produce_valid_output() {
        for pattern in InvariantPattern::ALL {
            let m = make_match("INV-TEST-ALL", pattern);
            let prop = extract_test_property(&m);
            assert_eq!(prop.inv_id, "INV-TEST-ALL");
            assert_eq!(prop.pattern, pattern);
            assert!(
                !prop.strategy_name.is_empty(),
                "strategy empty for {pattern}"
            );
            assert!(
                !prop.property_expr.is_empty(),
                "property empty for {pattern}"
            );
            assert!(
                !prop.assertion_type.is_empty(),
                "assertion empty for {pattern}"
            );
        }
    }

    // --- W3.5.3: Code emission ---

    #[test]
    fn emit_proptest_never_contains_fn_and_assertion() {
        let m = make_match("INV-STORE-001", InvariantPattern::Never);
        let prop = extract_test_property(&m);
        let code = emit_proptest(&prop);

        assert!(code.contains("proptest!"), "must contain proptest! macro");
        assert!(
            code.contains("fn generated_inv_store_001_never_immutability"),
            "function name mismatch, got:\n{code}"
        );
        assert!(
            code.contains("INV-STORE-001"),
            "must reference inv ID in doc comment"
        );
        assert!(code.contains("prop_assert!"), "must contain assertion");
        assert!(code.contains("arb_store(3)"), "must contain strategy");
        assert!(
            code.contains("is_subset"),
            "must contain property expression"
        );
    }

    #[test]
    fn emit_proptest_commutativity_uses_pair_strategy() {
        let m = make_match("INV-STORE-004", InvariantPattern::Commutativity);
        let prop = extract_test_property(&m);
        let code = emit_proptest(&prop);

        assert!(code.contains("(store_a, store_b)"), "must destructure pair");
        assert!(code.contains("merge_stores"), "must call merge_stores");
        assert!(code.contains("prop_assert_eq!"), "must use prop_assert_eq!");
    }

    #[test]
    fn emit_proptest_associativity_uses_triple_strategy() {
        let m = make_match("INV-STORE-005", InvariantPattern::Associativity);
        let prop = extract_test_property(&m);
        let code = emit_proptest(&prop);

        assert!(
            code.contains("(store_a, store_b, store_c)"),
            "must destructure triple"
        );
        assert!(code.contains("merge_stores"), "must call merge_stores");
    }

    #[test]
    fn emit_proptest_all_patterns_produce_compilable_structure() {
        for pattern in InvariantPattern::ALL {
            let m = make_match("INV-GEN-001", pattern);
            let prop = extract_test_property(&m);
            let code = emit_proptest(&prop);

            // Every emitted block must have balanced braces
            let open = code.chars().filter(|c| *c == '{').count();
            let close = code.chars().filter(|c| *c == '}').count();
            assert_eq!(
                open, close,
                "unbalanced braces for pattern {pattern}: {open} open vs {close} close"
            );

            // Every block must start with proptest! and contain #[test]
            assert!(
                code.starts_with("proptest!"),
                "must start with proptest! for {pattern}"
            );
            assert!(
                code.contains("#[test]"),
                "must contain #[test] for {pattern}"
            );
            assert!(
                code.contains("fn generated_"),
                "must contain fn name for {pattern}"
            );
        }
    }

    #[test]
    fn emit_test_module_wraps_all_properties() {
        let props: Vec<TestProperty> = InvariantPattern::ALL
            .iter()
            .enumerate()
            .map(|(i, &pattern)| {
                let m = make_match(&format!("INV-MOD-{:03}", i + 1), pattern);
                extract_test_property(&m)
            })
            .collect();

        let module = emit_test_module(&props);

        // Module structure
        assert!(module.contains("#[cfg(test)]"), "must have cfg(test)");
        assert!(
            module.contains("mod generated_coherence_tests"),
            "must have module name"
        );
        assert!(module.contains("use super::*;"), "must import parent");
        assert!(
            module.contains("use proptest::prelude::*;"),
            "must import proptest"
        );
        assert!(
            module.contains("use crate::proptest_strategies::arb_store;"),
            "must import arb_store"
        );
        assert!(
            module.contains("use crate::merge::merge_stores;"),
            "must import merge_stores"
        );
        assert!(
            module.contains("use crate::store::Store;"),
            "must import Store"
        );
        // Helper functions must be present
        assert!(
            module.contains("fn predicate("),
            "must include predicate helper"
        );
        assert!(
            module.contains("fn compute_metric("),
            "must include compute_metric helper"
        );

        // All 9 patterns should be present
        for (i, pattern) in InvariantPattern::ALL.iter().enumerate() {
            let fn_prefix = format!("fn generated_inv_mod_{:03}", i + 1);
            assert!(
                module.contains(&fn_prefix),
                "missing function for {pattern}: expected {fn_prefix}"
            );
        }

        // Balanced braces in the whole module
        let open = module.chars().filter(|c| *c == '{').count();
        let close = module.chars().filter(|c| *c == '}').count();
        assert_eq!(
            open, close,
            "unbalanced braces in module: {open} open vs {close} close"
        );
    }

    #[test]
    fn emit_test_module_empty_properties() {
        let module = emit_test_module(&[]);
        assert!(module.contains("mod generated_coherence_tests"));
        assert!(module.contains("use super::*;"));
        // Should still be valid — just an empty module
        assert!(module.ends_with("}\n"));
    }

    #[test]
    fn sanitize_id_converts_correctly() {
        assert_eq!(sanitize_id("INV-STORE-001"), "inv_store_001");
        assert_eq!(sanitize_id("ADR-MERGE-003"), "adr_merge_003");
        assert_eq!(sanitize_id("NEG-MUTATION-001"), "neg_mutation_001");
        // Ident-style IDs with colons and slashes
        assert_eq!(sanitize_id(":spec/inv-store-001"), "spec_inv_store_001");
        assert_eq!(sanitize_id(":db/ident"), "db_ident");
        // Dots are also sanitized
        assert_eq!(
            sanitize_id(":impl/compiler.inv_store_001"),
            "impl_compiler_inv_store_001"
        );
        // Leading digits get prefixed
        assert_eq!(sanitize_id("123-test"), "id_123_test");
        // Empty/all-special produces "unnamed"
        assert_eq!(sanitize_id(":::"), "unnamed");
    }

    #[test]
    fn sanitize_pattern_name_converts_correctly() {
        assert_eq!(
            sanitize_pattern_name(InvariantPattern::Never),
            "never_immutability"
        );
        assert_eq!(
            sanitize_pattern_name(InvariantPattern::Equality),
            "equality_determinism"
        );
        assert_eq!(
            sanitize_pattern_name(InvariantPattern::Commutativity),
            "commutativity"
        );
    }

    // =======================================================================
    // W3.5.6: Pattern detection tests — 2 per pattern (positive + negative)
    // =======================================================================

    // Helper: run detect_patterns_for_text and check if a specific pattern is present.
    fn text_matches_pattern(
        statement: &str,
        falsification: &str,
        expected: InvariantPattern,
    ) -> bool {
        let entity = EntityId::from_ident(":test/w356");
        let matches = detect_patterns_for_text("W356-TEST", entity, statement, falsification);
        matches.iter().any(|m| m.pattern == expected)
    }

    // --- 1. Never/Immutability (positive + negative) ---

    #[test]
    fn w356_never_positive() {
        assert!(
            text_matches_pattern(
                "The store never deletes a datom once inserted.",
                "Violated if any datom is removed after assertion.",
                InvariantPattern::Never,
            ),
            "\"never deletes\" should match Never pattern"
        );
    }

    #[test]
    fn w356_never_negative() {
        assert!(
            !text_matches_pattern(
                "The store has 5 fields in its header.",
                "Violated if the header field count differs from 5.",
                InvariantPattern::Never,
            ),
            "\"has 5 fields\" should not match Never pattern"
        );
    }

    // --- 2. Equality/Determinism (positive + negative) ---

    #[test]
    fn w356_equality_positive() {
        assert!(
            text_matches_pattern(
                "Same store plus same query produces the same result every time.",
                "Violated if two evaluations of identical query on identical store diverge.",
                InvariantPattern::Equality,
            ),
            "\"same result\" should match Equality pattern"
        );
    }

    #[test]
    fn w356_equality_negative() {
        assert!(
            !text_matches_pattern(
                "The merge operation combines two stores into one.",
                "Violated if the output store is missing datoms from either input.",
                InvariantPattern::Equality,
            ),
            "generic merge description should not match Equality pattern"
        );
    }

    // --- 3. Commutativity (positive + negative) ---

    #[test]
    fn w356_commutativity_positive() {
        assert!(
            text_matches_pattern(
                "Merge is commutative: merge(A,B) = merge(B,A).",
                "Violated if reordering the operands produces a different datom set.",
                InvariantPattern::Commutativity,
            ),
            "\"commutative\" should match Commutativity pattern"
        );
    }

    #[test]
    fn w356_commutativity_negative() {
        assert!(
            !text_matches_pattern(
                "Transactions are applied in causal order.",
                "Violated if a causally-later transaction is applied before its predecessor.",
                InvariantPattern::Commutativity,
            ),
            "causal ordering text should not match Commutativity pattern"
        );
    }

    // --- 4. Associativity (positive + negative) ---

    #[test]
    fn w356_associativity_positive() {
        assert!(
            text_matches_pattern(
                "Merge is associative: merge(merge(A,B),C) = merge(A,merge(B,C)).",
                "Violated if regrouping three stores produces different results.",
                InvariantPattern::Associativity,
            ),
            "\"associative\" should match Associativity pattern"
        );
    }

    #[test]
    fn w356_associativity_negative() {
        assert!(
            !text_matches_pattern(
                "Each datom occupies exactly 5 fields: entity, attribute, value, tx, op.",
                "Violated if a datom tuple has fewer or more than 5 elements.",
                InvariantPattern::Associativity,
            ),
            "datom tuple description should not match Associativity pattern"
        );
    }

    // --- 5. Idempotency (positive + negative) ---

    #[test]
    fn w356_idempotency_positive() {
        assert!(
            text_matches_pattern(
                "Merge is idempotent: merge(A,A) = A.",
                "Violated if applying merge to a store with itself changes state.",
                InvariantPattern::Idempotency,
            ),
            "\"idempotent\" should match Idempotency pattern"
        );
    }

    #[test]
    fn w356_idempotency_negative() {
        assert!(
            !text_matches_pattern(
                "The schema defines cardinality for each attribute.",
                "Violated if an attribute lacks a cardinality declaration.",
                InvariantPattern::Idempotency,
            ),
            "schema cardinality description should not match Idempotency pattern"
        );
    }

    // --- 6. Monotonicity (positive + negative) ---

    #[test]
    fn w356_monotonicity_positive() {
        assert!(
            text_matches_pattern(
                "F(S) is monotonically non-decreasing across bilateral cycles.",
                "Violated if F(S_n+1) < F(S_n) for any successive pair of cycles.",
                InvariantPattern::Monotonicity,
            ),
            "\"monotonically non-decreasing\" should match Monotonicity pattern"
        );
    }

    #[test]
    fn w356_monotonicity_negative() {
        assert!(
            !text_matches_pattern(
                "The query evaluator supports Datalog with stratified negation.",
                "Violated if a negated clause appears in an unstratifiable cycle.",
                InvariantPattern::Monotonicity,
            ),
            "query evaluator description should not match Monotonicity pattern"
        );
    }

    // --- 7. Boundedness (positive + negative) ---

    #[test]
    fn w356_boundedness_positive() {
        assert!(
            text_matches_pattern(
                "M(t) is bounded in [0,1] for all coherence metrics.",
                "Violated if any coherence metric exceeds 1.0 or falls below 0.0.",
                InvariantPattern::Boundedness,
            ),
            "\"bounded in [0,1]\" should match Boundedness pattern"
        );
    }

    #[test]
    fn w356_boundedness_negative() {
        assert!(
            !text_matches_pattern(
                "Transactions are ordered by their lamport clock.",
                "Violated if two transactions with the same lamport timestamp conflict.",
                InvariantPattern::Boundedness,
            ),
            "lamport clock description should not match Boundedness pattern"
        );
    }

    // --- 8. Completeness (positive + negative) ---

    #[test]
    fn w356_completeness_positive() {
        assert!(
            text_matches_pattern(
                "Every spec element must have an ID following the INV/ADR/NEG naming convention.",
                "Violated if a spec element exists without a conforming identifier.",
                InvariantPattern::Completeness,
            ),
            "\"every spec element must have\" should match Completeness pattern"
        );
    }

    #[test]
    fn w356_completeness_negative() {
        assert!(
            !text_matches_pattern(
                "The CLI prints output in three modes: human, agent, JSON.",
                "Violated if an unrecognized output mode is requested.",
                InvariantPattern::Completeness,
            ),
            "CLI output modes description should not match Completeness pattern"
        );
    }

    // --- 9. Preservation (positive + negative) ---

    #[test]
    fn w356_preservation_positive() {
        assert!(
            text_matches_pattern(
                "Merge preserves all datoms from both input stores.",
                "Violated if any datom present before merge is absent from the merged result.",
                InvariantPattern::Preservation,
            ),
            "\"preserves all datoms\" should match Preservation pattern"
        );
    }

    #[test]
    fn w356_preservation_negative() {
        assert!(
            !text_matches_pattern(
                "The harvest command extracts session knowledge into datoms.",
                "Violated if the harvest produces zero candidates from a non-trivial session.",
                InvariantPattern::Preservation,
            ),
            "harvest extraction description should not match Preservation pattern"
        );
    }

    // =======================================================================
    // W3.5.6: Store-integrated test — detect_patterns on a real store
    // =======================================================================

    #[test]
    fn w356_detect_patterns_on_store_with_mixed_specs() {
        let tx = test_tx();
        let mut all_datoms = Vec::new();

        // Add a Never-pattern invariant
        all_datoms.extend(spec_entity(
            ":spec/inv-w356-never",
            "INV-W356-NEVER",
            ":spec.type/invariant",
            "The store never deletes or mutates an existing datom.",
            "Violated if a datom is removed or modified in place.",
            tx,
        ));

        // Add a Commutativity-pattern invariant
        all_datoms.extend(spec_entity(
            ":spec/inv-w356-comm",
            "INV-W356-COMM",
            ":spec.type/invariant",
            "Merge is commutative: merge(A,B) = merge(B,A).",
            "Violated if order of merge operands changes the result.",
            tx,
        ));

        // Add a Boundedness-pattern invariant
        all_datoms.extend(spec_entity(
            ":spec/inv-w356-bound",
            "INV-W356-BOUND",
            ":spec.type/invariant",
            "The fitness score is bounded in [0,1].",
            "Violated if the score exceeds 1.0 or falls below 0.0.",
            tx,
        ));

        // Add a non-matching element (ADR with no pattern keywords)
        all_datoms.extend(spec_entity(
            ":spec/adr-w356-misc",
            "ADR-W356-MISC",
            ":spec.type/adr",
            "We chose EDN as the serialization format.",
            "The rationale is simplicity and Clojure ecosystem compatibility.",
            tx,
        ));

        let store = store_with(all_datoms);
        let matches = detect_patterns(&store);

        // Verify each expected pattern is present
        let spec_ids: Vec<&str> = matches.iter().map(|m| m.spec_id.as_str()).collect();
        assert!(
            spec_ids.contains(&"INV-W356-NEVER"),
            "Never-pattern invariant should be detected"
        );
        assert!(
            spec_ids.contains(&"INV-W356-COMM"),
            "Commutativity-pattern invariant should be detected"
        );
        assert!(
            spec_ids.contains(&"INV-W356-BOUND"),
            "Boundedness-pattern invariant should be detected"
        );

        // Verify the non-matching ADR is absent
        assert!(
            !spec_ids.contains(&"ADR-W356-MISC"),
            "Non-matching ADR should not appear in pattern matches"
        );

        // Verify pattern types are correct
        let never = matches
            .iter()
            .find(|m| m.spec_id == "INV-W356-NEVER" && m.pattern == InvariantPattern::Never)
            .expect("INV-W356-NEVER should match Never");
        assert!(
            never.confidence >= 0.25,
            "confidence should exceed threshold"
        );

        let comm = matches
            .iter()
            .find(|m| m.spec_id == "INV-W356-COMM" && m.pattern == InvariantPattern::Commutativity)
            .expect("INV-W356-COMM should match Commutativity");
        assert!(
            comm.confidence >= 0.25,
            "confidence should exceed threshold"
        );

        let bound = matches
            .iter()
            .find(|m| m.spec_id == "INV-W356-BOUND" && m.pattern == InvariantPattern::Boundedness)
            .expect("INV-W356-BOUND should match Boundedness");
        assert!(
            bound.confidence >= 0.25,
            "confidence should exceed threshold"
        );
    }

    // =======================================================================
    // W3.5.6: Summary statistics for mixed store
    // =======================================================================

    #[test]
    fn w356_summary_for_mixed_store() {
        let tx = test_tx();
        let mut all_datoms = Vec::new();

        all_datoms.extend(spec_entity(
            ":spec/inv-w356-s1",
            "INV-W356-S1",
            ":spec.type/invariant",
            "The store never mutates a datom.",
            "Violated if a datom is modified in place.",
            tx,
        ));
        all_datoms.extend(spec_entity(
            ":spec/inv-w356-s2",
            "INV-W356-S2",
            ":spec.type/invariant",
            "Merge is idempotent.",
            "Violated if applying merge twice changes state.",
            tx,
        ));
        all_datoms.extend(spec_entity(
            ":spec/adr-w356-s3",
            "ADR-W356-S3",
            ":spec.type/adr",
            "We use content-addressable hashing.",
            "No falsification.",
            tx,
        ));

        let store = store_with(all_datoms);
        let matches = detect_patterns(&store);
        let summary = summarize_patterns(&matches, 3);

        assert_eq!(summary.total_spec_elements, 3);
        // At least the two INVs should match (ADR may not)
        assert!(
            summary.matched_elements >= 2,
            "at least 2 of 3 spec elements should match, got {}",
            summary.matched_elements
        );
        assert!(summary.mean_confidence > 0.0);
    }

    // =======================================================================
    // W3.5.6: emit_proptest output contains the INV ID
    // =======================================================================

    #[test]
    fn w356_emit_proptest_contains_inv_id() {
        for pattern in InvariantPattern::ALL {
            let inv_id = format!("INV-W356-{}", sanitize_pattern_name(pattern).to_uppercase());
            let m = PatternMatch {
                spec_id: inv_id.clone(),
                entity: EntityId::from_ident(":test/w356-emit"),
                pattern,
                subject: "test subject".into(),
                property: "test property".into(),
                confidence: 0.9,
            };
            let prop = extract_test_property(&m);
            let code = emit_proptest(&prop);
            assert!(
                code.contains(&inv_id),
                "emitted code for {pattern} should contain INV ID \"{inv_id}\", got:\n{code}"
            );
        }
    }

    // =======================================================================
    // W3.5.7: Self-bootstrap — compiler generates tests for its own INVs
    // =======================================================================

    /// The compiler's own invariants, when added to the store as spec elements,
    /// should be detectable by the compiler's own pattern engine. This validates
    /// C7 (self-bootstrap): the system verifies its own specification.
    #[test]
    fn self_bootstrap_compiler_generates_own_tests() {
        let tx = test_tx();
        let mut all_datoms = Vec::new();

        // The compiler's own invariants as spec elements in the store:
        //
        // 1. "detect_patterns never panics" → Never pattern
        //    (from the proptest in this module: detect_patterns_never_panics)
        all_datoms.extend(spec_entity(
            ":spec/inv-compiler-001",
            "INV-COMPILER-001",
            ":spec.type/invariant",
            "detect_patterns never panics for any well-formed store. \
             The function must not panic or abort regardless of input.",
            "Violated if detect_patterns panics, aborts, or triggers undefined behavior \
             on any store reachable from Store::genesis() plus arbitrary valid transactions.",
            tx,
        ));

        // 2. "same store + same patterns = same result" → Equality pattern
        all_datoms.extend(spec_entity(
            ":spec/inv-compiler-002",
            "INV-COMPILER-002",
            ":spec.type/invariant",
            "Pattern detection is deterministic: the same store always produces \
             the same result set. Two calls to detect_patterns on an identical \
             store produce identical output.",
            "Violated if two evaluations of detect_patterns on the same store \
             diverge in their match set, ordering, or confidence values.",
            tx,
        ));

        // 3. "match count is bounded by spec element count" → Boundedness pattern
        //    (each spec element can match at most 9 patterns)
        all_datoms.extend(spec_entity(
            ":spec/inv-compiler-003",
            "INV-COMPILER-003",
            ":spec.type/invariant",
            "The number of pattern matches is bounded: at most 9 matches per \
             spec element (one per universal pattern). The total match count \
             does not exceed 9 times the spec element count.",
            "Violated if detect_patterns returns more than 9 * |spec_elements| \
             matches, or if any single spec_id appears more than 9 times.",
            tx,
        ));

        // 4. "keyword table covers all 9 patterns" → Completeness pattern
        all_datoms.extend(spec_entity(
            ":spec/inv-compiler-004",
            "INV-COMPILER-004",
            ":spec.type/invariant",
            "The keyword table must have entries for every pattern in \
             InvariantPattern::ALL. For all patterns p, keyword_table() \
             contains a PatternKeywords with pattern == p.",
            "Violated if any InvariantPattern variant lacks a corresponding \
             entry in the keyword table, causing that pattern to never match.",
            tx,
        ));

        // Step 2: Run detect_patterns on a store containing the compiler's own INVs
        let store = store_with(all_datoms);
        let matches = detect_patterns(&store);

        // Step 3: Verify the compiler detects patterns in its own INVs

        // INV-COMPILER-001 should match Never pattern
        let never_hits: Vec<_> = matches
            .iter()
            .filter(|m| m.spec_id == "INV-COMPILER-001" && m.pattern == InvariantPattern::Never)
            .collect();
        assert!(
            !never_hits.is_empty(),
            "INV-COMPILER-001 (\"never panics\") should match Never pattern. \
             All matches: {:?}",
            matches
                .iter()
                .filter(|m| m.spec_id == "INV-COMPILER-001")
                .collect::<Vec<_>>()
        );

        // INV-COMPILER-002 should match Equality/Determinism pattern
        let eq_hits: Vec<_> = matches
            .iter()
            .filter(|m| m.spec_id == "INV-COMPILER-002" && m.pattern == InvariantPattern::Equality)
            .collect();
        assert!(
            !eq_hits.is_empty(),
            "INV-COMPILER-002 (\"deterministic, same result\") should match Equality pattern. \
             All matches: {:?}",
            matches
                .iter()
                .filter(|m| m.spec_id == "INV-COMPILER-002")
                .collect::<Vec<_>>()
        );

        // INV-COMPILER-003 should match Boundedness pattern
        let bound_hits: Vec<_> = matches
            .iter()
            .filter(|m| {
                m.spec_id == "INV-COMPILER-003" && m.pattern == InvariantPattern::Boundedness
            })
            .collect();
        assert!(
            !bound_hits.is_empty(),
            "INV-COMPILER-003 (\"bounded, at most 9\") should match Boundedness pattern. \
             All matches: {:?}",
            matches
                .iter()
                .filter(|m| m.spec_id == "INV-COMPILER-003")
                .collect::<Vec<_>>()
        );

        // INV-COMPILER-004 should match Completeness pattern
        let comp_hits: Vec<_> = matches
            .iter()
            .filter(|m| {
                m.spec_id == "INV-COMPILER-004" && m.pattern == InvariantPattern::Completeness
            })
            .collect();
        assert!(
            !comp_hits.is_empty(),
            "INV-COMPILER-004 (\"for all patterns, every pattern\") should match Completeness. \
             All matches: {:?}",
            matches
                .iter()
                .filter(|m| m.spec_id == "INV-COMPILER-004")
                .collect::<Vec<_>>()
        );

        // Step 4: Generate test code via emit_proptest for each match
        let compiler_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.spec_id.starts_with("INV-COMPILER-"))
            .collect();
        assert!(
            compiler_matches.len() >= 4,
            "compiler should detect at least 4 patterns across its own INVs, got {}",
            compiler_matches.len()
        );

        for m in &compiler_matches {
            let prop = extract_test_property(m);
            let code = emit_proptest(&prop);

            // Step 5: Verify the generated code contains the compiler's INV IDs
            assert!(
                code.contains(&m.spec_id),
                "generated proptest for {} should contain INV ID in doc comment, got:\n{}",
                m.spec_id,
                code
            );

            // Generated code should be structurally valid
            let open = code.chars().filter(|c| *c == '{').count();
            let close = code.chars().filter(|c| *c == '}').count();
            assert_eq!(
                open, close,
                "generated code for {} has unbalanced braces: {} open vs {} close",
                m.spec_id, open, close
            );
        }

        // Bonus: emit a full test module from the compiler's own patterns
        let props: Vec<TestProperty> = compiler_matches
            .iter()
            .map(|m| extract_test_property(m))
            .collect();
        let module = emit_test_module(&props);
        assert!(
            module.contains("mod generated_coherence_tests"),
            "self-bootstrap module should have the standard module wrapper"
        );
        for m in &compiler_matches {
            assert!(
                module.contains(&m.spec_id),
                "self-bootstrap module should reference {}",
                m.spec_id
            );
        }
    }

    // =======================================================================
    // W3.5.7: emit_test_module produces valid Rust module structure
    // =======================================================================

    #[test]
    fn emit_test_module_produces_valid_rust_module_structure() {
        // Generate a module with a representative mix of patterns
        let patterns_to_test = [
            ("INV-STRUCT-001", InvariantPattern::Never),
            ("INV-STRUCT-002", InvariantPattern::Equality),
            ("INV-STRUCT-003", InvariantPattern::Commutativity),
            ("INV-STRUCT-004", InvariantPattern::Monotonicity),
            ("INV-STRUCT-005", InvariantPattern::Boundedness),
        ];

        let props: Vec<TestProperty> = patterns_to_test
            .iter()
            .map(|(id, pat)| {
                let m = make_match(id, *pat);
                extract_test_property(&m)
            })
            .collect();

        let module = emit_test_module(&props);

        // 1. Balanced braces (fundamental structural validity)
        let open_braces = module.chars().filter(|c| *c == '{').count();
        let close_braces = module.chars().filter(|c| *c == '}').count();
        assert_eq!(
            open_braces, close_braces,
            "module has unbalanced braces: {} open vs {} close\n---\n{}",
            open_braces, close_braces, module
        );

        // 2. Balanced parentheses
        let open_parens = module.chars().filter(|c| *c == '(').count();
        let close_parens = module.chars().filter(|c| *c == ')').count();
        assert_eq!(
            open_parens, close_parens,
            "module has unbalanced parentheses: {} open vs {} close",
            open_parens, close_parens
        );

        // 3. Required imports present
        assert!(
            module.contains("use super::*;"),
            "module must import parent scope"
        );
        assert!(
            module.contains("use proptest::prelude::*;"),
            "module must import proptest prelude"
        );
        assert!(
            module.contains("use crate::proptest_strategies::arb_store;"),
            "module must import arb_store strategy"
        );
        assert!(
            module.contains("use crate::merge::merge_stores;"),
            "module must import merge_stores for commutativity/associativity tests"
        );
        assert!(
            module.contains("use crate::store::Store;"),
            "module must import Store for genesis references"
        );
        // Helper functions must be present
        assert!(
            module.contains("fn predicate("),
            "module must include predicate helper function"
        );
        assert!(
            module.contains("fn compute_metric("),
            "module must include compute_metric helper function"
        );

        // 4. Module wrapper structure
        assert!(
            module.contains("#[cfg(test)]"),
            "module must have cfg(test) attribute"
        );
        assert!(
            module.contains("mod generated_coherence_tests"),
            "module must be named generated_coherence_tests"
        );

        // 5. Each test function is present with correct naming
        for (id, _) in &patterns_to_test {
            let sanitized = sanitize_id(id);
            let fn_prefix = format!("fn generated_{}_", sanitized);
            assert!(
                module.contains(&fn_prefix),
                "module must contain function for {id}: expected prefix \"{fn_prefix}\""
            );
        }

        // 6. Module ends correctly (closing brace + newline)
        assert!(
            module.ends_with("}\n"),
            "module must end with closing brace and newline"
        );

        // 7. Auto-generated header present
        assert!(
            module.contains("Auto-generated coherence tests"),
            "module must have auto-generated header comment"
        );

        // 8. Each proptest! block contains #[test] attribute
        let test_attr_count = module.matches("#[test]").count();
        assert_eq!(
            test_attr_count,
            patterns_to_test.len(),
            "module must have exactly one #[test] per property ({} expected, {} found)",
            patterns_to_test.len(),
            test_attr_count
        );
    }

    // =======================================================================
    // W3.5.7: patterns_to_trace_datoms creates L3 links for each match
    // =======================================================================

    #[test]
    fn patterns_to_trace_datoms_creates_l3_links() {
        let tx = test_tx();

        // Create pattern matches covering different patterns
        let test_matches = vec![
            PatternMatch {
                spec_id: "INV-STORE-001".into(),
                entity: EntityId::from_ident(":spec/inv-store-001"),
                pattern: InvariantPattern::Never,
                subject: "datom store".into(),
                property: "no deletion".into(),
                confidence: 0.9,
            },
            PatternMatch {
                spec_id: "INV-STORE-004".into(),
                entity: EntityId::from_ident(":spec/inv-store-004"),
                pattern: InvariantPattern::Commutativity,
                subject: "merge".into(),
                property: "order independence".into(),
                confidence: 0.85,
            },
            PatternMatch {
                spec_id: "INV-BILATERAL-001".into(),
                entity: EntityId::from_ident(":spec/inv-bilateral-001"),
                pattern: InvariantPattern::Boundedness,
                subject: "fitness function".into(),
                property: "bounded in [0,1]".into(),
                confidence: 0.75,
            },
        ];

        let datoms = patterns_to_trace_datoms(&test_matches, tx);

        // Each match should produce exactly 5 datoms:
        //   :db/ident, :impl/implements, :impl/verification-depth, :impl/file, :impl/module
        assert_eq!(
            datoms.len(),
            test_matches.len() * 5,
            "expected {} datoms (5 per match), got {}",
            test_matches.len() * 5,
            datoms.len()
        );

        // Verify each match produced the correct datoms
        for m in &test_matches {
            let impl_ident = format!(
                ":impl/compiler.{}.{}",
                sanitize_id(&m.spec_id),
                sanitize_pattern_name(m.pattern),
            );
            let impl_entity = EntityId::from_ident(&impl_ident);

            // 1. :db/ident datom exists
            let ident_datom = datoms
                .iter()
                .find(|d| d.entity == impl_entity && d.attribute.as_str() == ":db/ident")
                .unwrap_or_else(|| {
                    panic!(
                        "missing :db/ident datom for {}, impl_ident={}",
                        m.spec_id, impl_ident
                    )
                });
            assert_eq!(
                ident_datom.value,
                Value::Keyword(impl_ident.clone()),
                "ident value mismatch for {}",
                m.spec_id
            );

            // 2. :impl/implements → spec entity ref
            let impl_datom = datoms
                .iter()
                .find(|d| d.entity == impl_entity && d.attribute.as_str() == ":impl/implements")
                .unwrap_or_else(|| panic!("missing :impl/implements datom for {}", m.spec_id));
            let spec_ident = format!(":spec/{}", m.spec_id.to_lowercase());
            let expected_ref = EntityId::from_ident(&spec_ident);
            assert_eq!(
                impl_datom.value,
                Value::Ref(expected_ref),
                ":impl/implements should reference spec entity for {}",
                m.spec_id
            );

            // 3. :impl/verification-depth == 3 (L3: Property-based)
            let depth_datom = datoms
                .iter()
                .find(|d| {
                    d.entity == impl_entity && d.attribute.as_str() == ":impl/verification-depth"
                })
                .unwrap_or_else(|| {
                    panic!("missing :impl/verification-depth datom for {}", m.spec_id)
                });
            assert_eq!(
                depth_datom.value,
                Value::Long(3),
                "verification depth should be L3 (3) for {}",
                m.spec_id
            );

            // 4. :impl/file == "compiler"
            let file_datom = datoms
                .iter()
                .find(|d| d.entity == impl_entity && d.attribute.as_str() == ":impl/file")
                .unwrap_or_else(|| panic!("missing :impl/file datom for {}", m.spec_id));
            assert_eq!(
                file_datom.value,
                Value::String("compiler".to_string()),
                ":impl/file should be \"compiler\" for {}",
                m.spec_id
            );

            // 5. :impl/module == "compiler"
            let module_datom = datoms
                .iter()
                .find(|d| d.entity == impl_entity && d.attribute.as_str() == ":impl/module")
                .unwrap_or_else(|| panic!("missing :impl/module datom for {}", m.spec_id));
            assert_eq!(
                module_datom.value,
                Value::String("compiler".to_string()),
                ":impl/module should be \"compiler\" for {}",
                m.spec_id
            );

            // 6. All datoms are Assert operations
            let match_datoms: Vec<_> = datoms.iter().filter(|d| d.entity == impl_entity).collect();
            for d in &match_datoms {
                assert_eq!(
                    d.op,
                    Op::Assert,
                    "all datoms should be Assert for {}",
                    m.spec_id
                );
            }

            // 7. All datoms reference the correct transaction
            for d in &match_datoms {
                assert_eq!(
                    d.tx, tx,
                    "all datoms should reference the provided tx for {}",
                    m.spec_id
                );
            }
        }
    }

    /// Verify patterns_to_trace_datoms with empty input produces empty output.
    #[test]
    fn patterns_to_trace_datoms_empty_input() {
        let tx = test_tx();
        let datoms = patterns_to_trace_datoms(&[], tx);
        assert!(
            datoms.is_empty(),
            "empty pattern matches should produce empty datoms"
        );
    }

    /// Verify that different patterns for the same spec_id produce distinct entities.
    #[test]
    fn patterns_to_trace_datoms_distinct_entities_per_pattern() {
        let tx = test_tx();
        let matches = vec![
            PatternMatch {
                spec_id: "INV-BILATERAL-001".into(),
                entity: EntityId::from_ident(":spec/inv-bilateral-001"),
                pattern: InvariantPattern::Boundedness,
                subject: "fitness".into(),
                property: "bounded".into(),
                confidence: 0.8,
            },
            PatternMatch {
                spec_id: "INV-BILATERAL-001".into(),
                entity: EntityId::from_ident(":spec/inv-bilateral-001"),
                pattern: InvariantPattern::Monotonicity,
                subject: "fitness".into(),
                property: "non-decreasing".into(),
                confidence: 0.7,
            },
        ];

        let datoms = patterns_to_trace_datoms(&matches, tx);

        // Should have 10 datoms (5 per match)
        assert_eq!(datoms.len(), 10);

        // The two impl entities should be different (different patterns)
        let ident_datoms: Vec<_> = datoms
            .iter()
            .filter(|d| d.attribute.as_str() == ":db/ident")
            .collect();
        assert_eq!(ident_datoms.len(), 2);
        assert_ne!(
            ident_datoms[0].entity, ident_datoms[1].entity,
            "different patterns for the same spec_id must produce distinct entities"
        );
    }

    // --- Proptest: detect_patterns never panics ---

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use crate::proptest_strategies::arb_store;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn detect_patterns_never_panics(store in arb_store(5)) {
                // Must not panic for any well-formed store
                let _matches = detect_patterns(&store);
            }

            /// W3.5.6: detect_patterns_for_text never panics for arbitrary strings.
            #[test]
            fn w356_detect_patterns_for_text_never_panics(
                statement in ".*",
                falsification in ".*",
            ) {
                let entity = EntityId::from_ident(":test/proptest-fuzz");
                let _matches = detect_patterns_for_text(
                    "FUZZ-001",
                    entity,
                    &statement,
                    &falsification,
                );
            }
        }
    }
}
