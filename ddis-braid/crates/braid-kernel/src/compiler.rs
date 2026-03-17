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

use crate::datom::{Attribute, EntityId, Op, Value};
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
        }
    }
}
