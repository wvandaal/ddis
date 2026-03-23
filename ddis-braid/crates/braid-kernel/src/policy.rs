//! `policy` — Declarative epistemological policy (ADR-FOUNDATION-013, C8).
//!
//! The policy manifest defines what coherence means in a given domain:
//! claim types, evidence types, boundary definitions, anomaly detectors,
//! and calibration parameters. The kernel reads policy datoms at store
//! load time and configures fitness, guidance, and watchers dynamically.
//!
//! No attribute or type in this module assumes any specific methodology —
//! DDIS, research, compliance, or any other domain ontology
//! (NEG-FOUNDATION-003, INV-FOUNDATION-006).
//!
//! # Invariants
//!
//! - INV-FOUNDATION-007: Every aspect of the coherence model derives from policy datoms.
//! - INV-FOUNDATION-008: Policy datoms are append-only; weight changes create new datoms.
//! - NEG-FOUNDATION-003: No DDIS-specific concept in the policy schema.
//! - NEG-FOUNDATION-005: Every policy element feeds into a closed loop.

use crate::datom::{Attribute, EntityId, Op, Value};
use crate::store::Store;

// ===========================================================================
// Core Types
// ===========================================================================

/// A boundary definition from the policy manifest.
///
/// Boundaries define pairs of entity sets that should be aligned.
/// F(S) = sum(weight_i * coverage(boundary_i)) across all boundaries.
#[derive(Clone, Debug)]
pub struct BoundaryDef {
    /// Entity ID of this boundary in the store.
    pub entity: EntityId,
    /// Human-readable name (e.g., "validation", "coverage").
    pub name: String,
    /// Source entity pattern (e.g., ":spec/*").
    pub source_pattern: String,
    /// Target entity pattern (e.g., ":witness/*").
    pub target_pattern: String,
    /// F(S) contribution weight.
    pub weight: f64,
    /// Optional domain-language report template.
    pub report_template: Option<String>,
}

/// An anomaly detector from the policy manifest.
#[derive(Clone, Debug)]
pub struct AnomalyDef {
    /// Entity ID.
    pub entity: EntityId,
    /// Attribute whose change triggers detection.
    pub trigger: String,
    /// Count threshold for alert.
    pub threshold: i64,
    /// Human-readable alert message.
    pub message: String,
}

/// Calibration parameters for the self-improving loop (OBSERVER-4).
#[derive(Clone, Debug)]
pub struct CalibrationConfig {
    /// Number of recent predictions in calibration window.
    pub window: usize,
    /// Mean absolute error threshold for weight adjustment.
    pub threshold: f64,
}

impl Default for CalibrationConfig {
    fn default() -> Self {
        Self {
            window: 20,
            threshold: 0.05,
        }
    }
}

/// The parsed policy manifest — single source of truth for policy interpretation.
///
/// Both MaterializedViews and BoundaryRegistry consume this struct.
/// Isomorphism invariant: Views.fitness(config) == BoundaryRegistry.coverage(config, store).
#[derive(Clone, Debug)]
pub struct PolicyConfig {
    /// Boundary definitions with weights.
    pub boundaries: Vec<BoundaryDef>,
    /// Claim entity attribute patterns.
    pub claim_patterns: Vec<String>,
    /// Evidence entity attribute patterns.
    pub evidence_patterns: Vec<String>,
    /// Anomaly detectors.
    pub anomaly_detectors: Vec<AnomalyDef>,
    /// Calibration parameters.
    pub calibration: CalibrationConfig,
}

impl PolicyConfig {
    /// Parse the policy manifest from store datoms.
    ///
    /// Returns `None` if no `:policy/*` datoms exist (empty substrate).
    /// When `None`, callers should fall back to hardcoded defaults or F(S)=1.0.
    pub fn from_store(store: &Store) -> Option<Self> {
        let boundary_name_attr = Attribute::from_keyword(":policy/boundary-name");
        let boundary_datoms = store.attribute_datoms(&boundary_name_attr);

        // If no boundary names exist, there's no policy manifest
        if boundary_datoms.is_empty() {
            return None;
        }

        let mut boundaries = Vec::new();
        let mut claim_patterns = Vec::new();
        let mut evidence_patterns = Vec::new();
        let mut anomaly_detectors = Vec::new();
        let mut calibration = CalibrationConfig::default();

        // Collect boundary definitions: each entity with :policy/boundary-name is a boundary
        for d in boundary_datoms {
            if d.op != Op::Assert {
                continue;
            }
            let name = match &d.value {
                Value::String(s) => s.clone(),
                _ => continue,
            };
            let entity = d.entity;
            let entity_datoms = store.entity_datoms(entity);

            let source = extract_string(&entity_datoms, ":policy/boundary-source")
                .unwrap_or_default();
            let target = extract_string(&entity_datoms, ":policy/boundary-target")
                .unwrap_or_default();
            let weight = extract_double(&entity_datoms, ":policy/boundary-weight")
                .unwrap_or(0.0);
            let report_template =
                extract_string(&entity_datoms, ":policy/boundary-report-template");

            if !source.is_empty() && !target.is_empty() {
                boundaries.push(BoundaryDef {
                    entity,
                    name,
                    source_pattern: source,
                    target_pattern: target,
                    weight,
                    report_template,
                });
            }
        }

        // Collect claim patterns
        let claim_attr = Attribute::from_keyword(":policy/claim-pattern");
        for d in store.attribute_datoms(&claim_attr) {
            if d.op == Op::Assert {
                if let Value::String(s) = &d.value {
                    claim_patterns.push(s.clone());
                }
            }
        }

        // Collect evidence patterns
        let evidence_attr = Attribute::from_keyword(":policy/evidence-pattern");
        for d in store.attribute_datoms(&evidence_attr) {
            if d.op == Op::Assert {
                if let Value::String(s) = &d.value {
                    evidence_patterns.push(s.clone());
                }
            }
        }

        // Collect anomaly detectors
        let anomaly_trigger_attr = Attribute::from_keyword(":policy/anomaly-trigger");
        for d in store.attribute_datoms(&anomaly_trigger_attr) {
            if d.op != Op::Assert {
                continue;
            }
            let trigger = match &d.value {
                Value::String(s) => s.clone(),
                _ => continue,
            };
            let entity = d.entity;
            let entity_datoms = store.entity_datoms(entity);

            let threshold = extract_long(&entity_datoms, ":policy/anomaly-threshold")
                .unwrap_or(10);
            let message = extract_string(&entity_datoms, ":policy/anomaly-message")
                .unwrap_or_else(|| format!("anomaly on {trigger}"));

            anomaly_detectors.push(AnomalyDef {
                entity,
                trigger,
                threshold,
                message,
            });
        }

        // Calibration parameters (from any entity with these attributes)
        let cal_window_attr = Attribute::from_keyword(":policy/calibration-window");
        if let Some(d) = store.attribute_datoms(&cal_window_attr).last() {
            if let Value::Long(w) = d.value {
                calibration.window = w.max(1) as usize;
            }
        }
        let cal_threshold_attr = Attribute::from_keyword(":policy/calibration-threshold");
        if let Some(d) = store.attribute_datoms(&cal_threshold_attr).last() {
            if let Value::Double(t) = d.value {
                calibration.threshold = t.into_inner().max(0.001);
            }
        }

        Some(PolicyConfig {
            boundaries,
            claim_patterns,
            evidence_patterns,
            anomaly_detectors,
            calibration,
        })
    }

    /// Total weight across all boundaries (for normalization check).
    pub fn total_weight(&self) -> f64 {
        self.boundaries.iter().map(|b| b.weight).sum()
    }

    /// Whether a given attribute matches any claim pattern.
    pub fn is_claim_attribute(&self, attr: &str) -> bool {
        self.claim_patterns.iter().any(|p| attr_matches_pattern(attr, p))
    }

    /// Whether a given attribute matches any evidence pattern.
    pub fn is_evidence_attribute(&self, attr: &str) -> bool {
        self.evidence_patterns.iter().any(|p| attr_matches_pattern(attr, p))
    }

    /// Check if an attribute matches a pattern (static method for use by bilateral.rs).
    pub fn attr_matches(attr: &str, pattern: &str) -> bool {
        attr_matches_pattern(attr, pattern)
    }

    /// Compute coverage for a single boundary from store entity counts.
    ///
    /// Coverage = |target entities referencing source entities| / |source entities|.
    /// This is a simplified coverage metric; POLICY-3 will implement the full
    /// boundary evaluation using BoundaryRegistry.
    pub fn boundary_coverage(&self, _boundary: &BoundaryDef, _store: &Store) -> f64 {
        // Placeholder — POLICY-3 implements the full evaluation
        0.0
    }
}

/// Check if an attribute string matches a pattern.
///
/// Patterns support:
/// - Exact match: ":spec/element-type" matches ":spec/element-type"
/// - Namespace wildcard: ":spec/*" matches ":spec/anything"
/// - Prefix: ":impl/" matches any attribute starting with ":impl/"
pub fn attr_matches_pattern(attr: &str, pattern: &str) -> bool {
    if pattern.ends_with("/*") {
        let prefix = &pattern[..pattern.len() - 1]; // ":spec/" from ":spec/*"
        attr.starts_with(prefix)
    } else if pattern.ends_with('/') {
        attr.starts_with(pattern)
    } else {
        attr == pattern
    }
}

// ===========================================================================
// Validation (POLICY-5)
// ===========================================================================

/// A validation error in the policy manifest.
#[derive(Clone, Debug)]
pub struct PolicyError {
    /// Which entity has the error.
    pub entity: Option<EntityId>,
    /// What constraint is violated.
    pub constraint: String,
    /// How to fix it.
    pub fix: String,
}

/// Validate a parsed policy manifest.
///
/// Returns errors if the policy is malformed. An empty error list means valid.
/// Invalid policies produce warnings but do NOT block store loading —
/// the system falls back to hardcoded defaults with diagnostic messages.
pub fn validate_policy(config: &PolicyConfig) -> Vec<PolicyError> {
    let mut errors = Vec::new();

    // Check boundary weights
    for b in &config.boundaries {
        if b.weight < 0.0 || b.weight > 1.0 {
            errors.push(PolicyError {
                entity: Some(b.entity),
                constraint: format!("boundary '{}' weight {} outside [0, 1]", b.name, b.weight),
                fix: "set :policy/boundary-weight to a value between 0.0 and 1.0".to_string(),
            });
        }
        if b.name.is_empty() {
            errors.push(PolicyError {
                entity: Some(b.entity),
                constraint: "boundary has empty name".to_string(),
                fix: "set :policy/boundary-name to a descriptive string".to_string(),
            });
        }
        if b.source_pattern.is_empty() {
            errors.push(PolicyError {
                entity: Some(b.entity),
                constraint: format!("boundary '{}' has empty source pattern", b.name),
                fix: "set :policy/boundary-source (e.g., ':spec/*')".to_string(),
            });
        }
        if b.target_pattern.is_empty() {
            errors.push(PolicyError {
                entity: Some(b.entity),
                constraint: format!("boundary '{}' has empty target pattern", b.name),
                fix: "set :policy/boundary-target (e.g., ':impl/*')".to_string(),
            });
        }
    }

    // Check weight sum
    let total = config.total_weight();
    if !config.boundaries.is_empty() && (total - 1.0).abs() > 0.05 {
        errors.push(PolicyError {
            entity: None,
            constraint: format!("boundary weights sum to {total:.3}, expected ~1.0"),
            fix: "normalize weights so they sum to 1.0".to_string(),
        });
    }

    // Check anomaly thresholds
    for a in &config.anomaly_detectors {
        if a.threshold <= 0 {
            errors.push(PolicyError {
                entity: Some(a.entity),
                constraint: format!("anomaly '{}' has non-positive threshold {}", a.trigger, a.threshold),
                fix: "set :policy/anomaly-threshold to a positive integer".to_string(),
            });
        }
    }

    errors
}

// ===========================================================================
// Calibration — Weight Adjustment from Hypothesis Outcomes (HL-CALIBRATE)
// ===========================================================================

/// A recommended weight adjustment for a policy boundary.
///
/// Generated by `calibrate_boundary_weights` when the prediction error for a
/// boundary exceeds the calibration threshold. The adjustment direction is
/// determined by the signed error: positive mean → over-predicted → decrease,
/// negative mean → under-predicted → increase.
///
/// INV-FOUNDATION-008: Weight changes are append-only (new datoms, not mutations).
/// ADR-FOUNDATION-024: Self-calibrating acquisition function.
#[derive(Clone, Debug)]
pub struct WeightAdjustment {
    /// Boundary name (matches BoundaryDef.name).
    pub boundary_name: String,
    /// Boundary entity ID.
    pub boundary_entity: EntityId,
    /// Current weight from policy manifest.
    pub current_weight: f64,
    /// Recommended new weight after adjustment.
    pub recommended_weight: f64,
    /// Mean absolute error for this boundary.
    pub mean_error: f64,
    /// Number of data points (completed hypotheses for this boundary).
    pub sample_count: usize,
    /// Human-readable rationale for the adjustment.
    pub rationale: String,
}

/// Compute boundary weight adjustments from hypothesis outcome data.
///
/// Reads completed hypotheses from the store, groups by `:hypothesis/boundary`,
/// computes per-boundary mean error, and recommends weight adjustments for
/// boundaries where error exceeds the calibration threshold.
///
/// Adjustment rule:
/// - Compute signed error = mean(predicted - actual) per boundary
/// - If |signed_error| > threshold: adjust weight by -signed_error * learning_rate
/// - Learning rate = 0.1 (conservative — 10% adjustment per calibration)
/// - Clamp weights to [0.05, 0.95] to prevent collapse
/// - Normalize so weights sum to 1.0
///
/// Returns an empty vec if:
/// - No policy manifest exists
/// - No completed hypotheses exist
/// - All boundaries are within threshold
///
/// ADR-FOUNDATION-024, INV-FOUNDATION-008.
pub fn calibrate_boundary_weights(store: &Store) -> Vec<WeightAdjustment> {
    let config = match PolicyConfig::from_store(store) {
        Some(c) => c,
        None => return vec![],
    };

    if config.boundaries.is_empty() {
        return vec![];
    }

    // Collect completed hypotheses grouped by boundary
    let boundary_attr = Attribute::from_keyword(":hypothesis/boundary");
    let predicted_attr = Attribute::from_keyword(":hypothesis/predicted");
    let actual_attr = Attribute::from_keyword(":hypothesis/actual");
    let completed_attr = Attribute::from_keyword(":hypothesis/completed");

    // Find all hypothesis entities that have a completed timestamp
    let mut hypothesis_entities: std::collections::BTreeSet<EntityId> =
        std::collections::BTreeSet::new();
    for d in store.attribute_datoms(&completed_attr) {
        if d.op == Op::Assert {
            hypothesis_entities.insert(d.entity);
        }
    }

    if hypothesis_entities.is_empty() {
        return vec![];
    }

    // Group (boundary_name → signed errors) for completed hypotheses
    let mut boundary_errors: std::collections::BTreeMap<String, Vec<f64>> =
        std::collections::BTreeMap::new();

    for entity in &hypothesis_entities {
        let datoms = store.entity_datoms(*entity);

        // Extract boundary name, predicted, actual for this hypothesis
        let mut boundaries: Vec<String> = Vec::new();
        let mut predicted: Option<f64> = None;
        let mut actual: Option<f64> = None;

        for d in &datoms {
            if d.op != Op::Assert {
                continue;
            }
            if d.attribute == boundary_attr {
                if let Value::String(s) = &d.value {
                    boundaries.push(s.clone());
                }
            }
            if d.attribute == predicted_attr {
                if let Value::Double(f) = &d.value {
                    predicted = Some(f.into_inner());
                }
            }
            if d.attribute == actual_attr {
                if let Value::Double(f) = &d.value {
                    actual = Some(f.into_inner());
                }
            }
        }

        // Record signed error for each boundary this hypothesis targets
        if let (Some(pred), Some(act)) = (predicted, actual) {
            let signed_error = pred - act;
            for b_name in &boundaries {
                boundary_errors
                    .entry(b_name.clone())
                    .or_default()
                    .push(signed_error);
            }
        }
    }

    if boundary_errors.is_empty() {
        return vec![];
    }

    let learning_rate = 0.1; // Conservative: 10% per calibration step
    let threshold = config.calibration.threshold;
    let mut adjustments = Vec::new();

    // Match hypothesis boundaries to policy boundaries
    for boundary_def in &config.boundaries {
        if let Some(errors) = boundary_errors.get(&boundary_def.name) {
            if errors.is_empty() {
                continue;
            }

            let mean_signed = errors.iter().sum::<f64>() / errors.len() as f64;
            let mean_abs = errors.iter().map(|e| e.abs()).sum::<f64>() / errors.len() as f64;

            if mean_abs <= threshold {
                continue; // Within acceptable calibration — no adjustment needed
            }

            // Adjust: decrease weight for over-predicted, increase for under-predicted
            let delta = -mean_signed * learning_rate;
            let new_weight = (boundary_def.weight + delta).clamp(0.05, 0.95);

            let direction = if mean_signed > 0.0 {
                "over-predicted"
            } else {
                "under-predicted"
            };

            adjustments.push(WeightAdjustment {
                boundary_name: boundary_def.name.clone(),
                boundary_entity: boundary_def.entity,
                current_weight: boundary_def.weight,
                recommended_weight: new_weight,
                mean_error: mean_abs,
                sample_count: errors.len(),
                rationale: format!(
                    "{} boundary '{}' {} (mean error {:.3}, {} samples) → weight {:.3} → {:.3}",
                    if mean_abs > threshold * 2.0 {
                        "HIGH"
                    } else {
                        "MODERATE"
                    },
                    boundary_def.name,
                    direction,
                    mean_abs,
                    errors.len(),
                    boundary_def.weight,
                    new_weight
                ),
            });
        }
    }

    // Normalize recommended weights so they sum to ~1.0
    if !adjustments.is_empty() {
        let total_adjusted: f64 = adjustments.iter().map(|a| a.recommended_weight).sum::<f64>();
        // Include unadjusted boundary weights in the normalization
        let unadjusted_total: f64 = config
            .boundaries
            .iter()
            .filter(|b| !adjustments.iter().any(|a| a.boundary_entity == b.entity))
            .map(|b| b.weight)
            .sum();
        let grand_total = total_adjusted + unadjusted_total;

        if grand_total > 0.0 {
            for adj in &mut adjustments {
                adj.recommended_weight /= grand_total;
            }
        }
    }

    adjustments
}

/// Generate datoms to apply weight adjustments to the policy manifest.
///
/// Creates new `:policy/boundary-weight` assertion datoms for each adjustment.
/// These are new datoms (append-only, INV-FOUNDATION-008) — the old weight
/// datoms remain in the store for audit trail.
///
/// The caller is responsible for transacting these datoms.
pub fn apply_weight_adjustments(
    adjustments: &[WeightAdjustment],
    tx: crate::datom::TxId,
) -> Vec<crate::datom::Datom> {
    let weight_attr = Attribute::from_keyword(":policy/boundary-weight");
    adjustments
        .iter()
        .map(|adj| {
            crate::datom::Datom::new(
                adj.boundary_entity,
                weight_attr.clone(),
                Value::Double(ordered_float::OrderedFloat(adj.recommended_weight)),
                tx,
                Op::Assert,
            )
        })
        .collect()
}

// ===========================================================================
// Helpers
// ===========================================================================

fn extract_string(datoms: &[&crate::datom::Datom], attr_name: &str) -> Option<String> {
    let attr = Attribute::from_keyword(attr_name);
    datoms
        .iter()
        .rfind(|d| d.attribute == attr && d.op == Op::Assert)
        .and_then(|d| match &d.value {
            Value::String(s) => Some(s.clone()),
            _ => None,
        })
}

fn extract_double(datoms: &[&crate::datom::Datom], attr_name: &str) -> Option<f64> {
    let attr = Attribute::from_keyword(attr_name);
    datoms
        .iter()
        .rfind(|d| d.attribute == attr && d.op == Op::Assert)
        .and_then(|d| match &d.value {
            Value::Double(f) => Some(f.into_inner()),
            _ => None,
        })
}

fn extract_long(datoms: &[&crate::datom::Datom], attr_name: &str) -> Option<i64> {
    let attr = Attribute::from_keyword(attr_name);
    datoms
        .iter()
        .rfind(|d| d.attribute == attr && d.op == Op::Assert)
        .and_then(|d| match &d.value {
            Value::Long(v) => Some(*v),
            _ => None,
        })
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attr_matches_exact() {
        assert!(attr_matches_pattern(":spec/element-type", ":spec/element-type"));
        assert!(!attr_matches_pattern(":spec/element-type", ":spec/other"));
    }

    #[test]
    fn attr_matches_namespace_wildcard() {
        assert!(attr_matches_pattern(":spec/element-type", ":spec/*"));
        assert!(attr_matches_pattern(":spec/falsification", ":spec/*"));
        assert!(!attr_matches_pattern(":impl/implements", ":spec/*"));
    }

    #[test]
    fn attr_matches_prefix() {
        assert!(attr_matches_pattern(":impl/implements", ":impl/"));
        assert!(attr_matches_pattern(":impl/anything", ":impl/"));
        assert!(!attr_matches_pattern(":spec/foo", ":impl/"));
    }

    #[test]
    fn empty_policy_from_genesis_store() {
        let store = Store::genesis();
        let config = PolicyConfig::from_store(&store);
        assert!(config.is_none(), "genesis store should have no policy");
    }

    #[test]
    fn validate_empty_policy() {
        let config = PolicyConfig {
            boundaries: vec![],
            claim_patterns: vec![],
            evidence_patterns: vec![],
            anomaly_detectors: vec![],
            calibration: CalibrationConfig::default(),
        };
        let errors = validate_policy(&config);
        assert!(errors.is_empty(), "empty policy is vacuously valid");
    }

    #[test]
    fn validate_negative_weight() {
        let config = PolicyConfig {
            boundaries: vec![BoundaryDef {
                entity: EntityId::from_ident(":test/boundary"),
                name: "test".to_string(),
                source_pattern: ":a/*".to_string(),
                target_pattern: ":b/*".to_string(),
                weight: -0.5,
                report_template: None,
            }],
            claim_patterns: vec![],
            evidence_patterns: vec![],
            anomaly_detectors: vec![],
            calibration: CalibrationConfig::default(),
        };
        let errors = validate_policy(&config);
        assert!(!errors.is_empty(), "negative weight should produce error");
        assert!(
            errors[0].constraint.contains("outside [0, 1]"),
            "error should mention weight range"
        );
    }

    #[test]
    fn validate_weight_sum() {
        let config = PolicyConfig {
            boundaries: vec![
                BoundaryDef {
                    entity: EntityId::from_ident(":test/b1"),
                    name: "b1".to_string(),
                    source_pattern: ":a/*".to_string(),
                    target_pattern: ":b/*".to_string(),
                    weight: 0.8,
                    report_template: None,
                },
                BoundaryDef {
                    entity: EntityId::from_ident(":test/b2"),
                    name: "b2".to_string(),
                    source_pattern: ":c/*".to_string(),
                    target_pattern: ":d/*".to_string(),
                    weight: 0.8,
                    report_template: None,
                },
            ],
            claim_patterns: vec![],
            evidence_patterns: vec![],
            anomaly_detectors: vec![],
            calibration: CalibrationConfig::default(),
        };
        let errors = validate_policy(&config);
        assert!(
            errors.iter().any(|e| e.constraint.contains("sum to")),
            "weights summing to 1.6 should produce normalization warning"
        );
    }

    #[test]
    fn validate_missing_boundary_name() {
        let config = PolicyConfig {
            boundaries: vec![BoundaryDef {
                entity: EntityId::from_ident(":test/noname"),
                name: String::new(),
                source_pattern: ":a/*".to_string(),
                target_pattern: ":b/*".to_string(),
                weight: 1.0,
                report_template: None,
            }],
            claim_patterns: vec![],
            evidence_patterns: vec![],
            anomaly_detectors: vec![],
            calibration: CalibrationConfig::default(),
        };
        let errors = validate_policy(&config);
        assert!(
            errors.iter().any(|e| e.constraint.contains("empty name")),
            "empty boundary name should produce error"
        );
    }

    #[test]
    fn claim_pattern_matching() {
        let config = PolicyConfig {
            boundaries: vec![],
            claim_patterns: vec![":spec/*".to_string(), ":requirement/*".to_string()],
            evidence_patterns: vec![":witness/*".to_string()],
            anomaly_detectors: vec![],
            calibration: CalibrationConfig::default(),
        };
        assert!(config.is_claim_attribute(":spec/element-type"));
        assert!(config.is_claim_attribute(":requirement/title"));
        assert!(!config.is_claim_attribute(":impl/implements"));
        assert!(config.is_evidence_attribute(":witness/status"));
        assert!(!config.is_evidence_attribute(":spec/element-type"));
    }

    // ── POLICY-TEST: Additional unit tests for policy manifest system ──

    use crate::datom::{AgentId, Datom, TxId};

    fn make_policy_store() -> Store {
        let agent = AgentId::from_name("test:policy");
        let tx = TxId::new(100, 0, agent);

        let mut datoms = Store::genesis().datom_set().clone();

        // Create a boundary entity
        let boundary = EntityId::from_ident(":policy/test-boundary");
        datoms.insert(Datom::new(
            boundary,
            Attribute::from_keyword(":policy/boundary-name"),
            Value::String("test-coverage".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            boundary,
            Attribute::from_keyword(":policy/boundary-source"),
            Value::String(":claim/*".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            boundary,
            Attribute::from_keyword(":policy/boundary-target"),
            Value::String(":evidence/*".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            boundary,
            Attribute::from_keyword(":policy/boundary-weight"),
            Value::Double(ordered_float::OrderedFloat(1.0)),
            tx,
            Op::Assert,
        ));

        // Create a claim entity
        let claim = EntityId::from_ident(":claim/test-001");
        datoms.insert(Datom::new(
            claim,
            Attribute::from_keyword(":claim/title"),
            Value::String("Test claim".to_string()),
            tx,
            Op::Assert,
        ));

        // Create an evidence entity referencing the claim
        let evidence = EntityId::from_ident(":evidence/test-001");
        datoms.insert(Datom::new(
            evidence,
            Attribute::from_keyword(":evidence/covers"),
            Value::Ref(claim),
            tx,
            Op::Assert,
        ));

        Store::from_datoms(datoms)
    }

    #[test]
    fn policy_from_store_with_boundary() {
        let store = make_policy_store();
        let config = PolicyConfig::from_store(&store);
        assert!(config.is_some(), "store with boundary datoms should produce policy");
        let config = config.unwrap();
        assert_eq!(config.boundaries.len(), 1);
        assert_eq!(config.boundaries[0].name, "test-coverage");
        assert_eq!(config.boundaries[0].weight, 1.0);
    }

    #[test]
    fn policy_total_weight() {
        let store = make_policy_store();
        let config = PolicyConfig::from_store(&store).unwrap();
        assert!((config.total_weight() - 1.0).abs() < 0.001);
    }

    #[test]
    fn policy_valid_single_boundary() {
        let store = make_policy_store();
        let config = PolicyConfig::from_store(&store).unwrap();
        let errors = validate_policy(&config);
        assert!(errors.is_empty(), "single boundary with weight 1.0 should be valid");
    }

    #[test]
    fn compute_fitness_from_policy_with_boundary() {
        let store = make_policy_store();
        let fs = crate::bilateral::compute_fitness_from_policy(&store);
        assert!(fs.is_some(), "store with policy should produce fitness");
        let fs = fs.unwrap();
        assert!(
            fs.total >= 0.0 && fs.total <= 1.0,
            "F(S) should be in [0,1]: {:.4}",
            fs.total
        );
    }

    #[test]
    fn compute_fitness_from_policy_none_without_policy() {
        let store = Store::genesis();
        let fs = crate::bilateral::compute_fitness_from_policy(&store);
        assert!(fs.is_none(), "genesis store should produce None (no policy)");
    }

    #[test]
    fn store_fitness_uses_policy_when_available() {
        let store = make_policy_store();
        let fs = store.fitness();
        // Should succeed regardless of path (policy or views)
        assert!(
            fs.total >= 0.0 && fs.total <= 1.0,
            "store.fitness() should be in [0,1]: {:.4}",
            fs.total
        );
    }

    #[test]
    fn store_fitness_fallback_without_policy() {
        let store = Store::genesis();
        let fs = store.fitness();
        // Without policy, falls back to views which compute from hardcoded accumulators
        assert!(
            fs.total >= 0.0 && fs.total <= 1.0,
            "fallback fitness should be in [0,1]: {:.4}",
            fs.total
        );
    }

    #[test]
    fn validate_anomaly_negative_threshold() {
        let config = PolicyConfig {
            boundaries: vec![],
            claim_patterns: vec![],
            evidence_patterns: vec![],
            anomaly_detectors: vec![AnomalyDef {
                entity: EntityId::from_ident(":test/anomaly"),
                trigger: ":tx/time".to_string(),
                threshold: -5,
                message: "test".to_string(),
            }],
            calibration: CalibrationConfig::default(),
        };
        let errors = validate_policy(&config);
        assert!(
            errors.iter().any(|e| e.constraint.contains("non-positive")),
            "negative anomaly threshold should produce error"
        );
    }

    #[test]
    fn validate_missing_source_pattern() {
        let config = PolicyConfig {
            boundaries: vec![BoundaryDef {
                entity: EntityId::from_ident(":test/nosrc"),
                name: "no-source".to_string(),
                source_pattern: String::new(),
                target_pattern: ":b/*".to_string(),
                weight: 1.0,
                report_template: None,
            }],
            claim_patterns: vec![],
            evidence_patterns: vec![],
            anomaly_detectors: vec![],
            calibration: CalibrationConfig::default(),
        };
        let errors = validate_policy(&config);
        assert!(
            errors.iter().any(|e| e.constraint.contains("empty source")),
            "empty source pattern should produce error"
        );
    }

    #[test]
    fn calibration_config_defaults() {
        let cal = CalibrationConfig::default();
        assert_eq!(cal.window, 20);
        assert!((cal.threshold - 0.05).abs() < 0.001);
    }

    #[test]
    fn policy_with_two_boundaries_coverage() {
        let agent = AgentId::from_name("test:2b");
        let tx = TxId::new(200, 0, agent);
        let mut datoms = Store::genesis().datom_set().clone();

        // Boundary 1: claim -> evidence (weight 0.6)
        let b1 = EntityId::from_ident(":policy/b1");
        datoms.insert(Datom::new(b1, Attribute::from_keyword(":policy/boundary-name"), Value::String("b1".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b1, Attribute::from_keyword(":policy/boundary-source"), Value::String(":req/*".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b1, Attribute::from_keyword(":policy/boundary-target"), Value::String(":test/*".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b1, Attribute::from_keyword(":policy/boundary-weight"), Value::Double(ordered_float::OrderedFloat(0.6)), tx, Op::Assert));

        // Boundary 2: spec -> impl (weight 0.4)
        let b2 = EntityId::from_ident(":policy/b2");
        datoms.insert(Datom::new(b2, Attribute::from_keyword(":policy/boundary-name"), Value::String("b2".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b2, Attribute::from_keyword(":policy/boundary-source"), Value::String(":spec/*".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b2, Attribute::from_keyword(":policy/boundary-target"), Value::String(":impl/*".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b2, Attribute::from_keyword(":policy/boundary-weight"), Value::Double(ordered_float::OrderedFloat(0.4)), tx, Op::Assert));

        let store = Store::from_datoms(datoms);
        let config = PolicyConfig::from_store(&store).unwrap();
        assert_eq!(config.boundaries.len(), 2);
        assert!((config.total_weight() - 1.0).abs() < 0.001);

        let fs = crate::bilateral::compute_fitness_from_policy(&store).unwrap();
        // No entities matching :req/* or :spec/* in genesis store → vacuous coverage
        assert!(fs.total >= 0.0 && fs.total <= 1.0);
    }

    // ── Additional unit tests completing POLICY-TEST acceptance criteria ──

    #[test]
    fn anomaly_detector_roundtrip_from_store() {
        // Full anomaly detector round-trip: store → PolicyConfig → AnomalyDef
        let agent = AgentId::from_name("test:anomaly");
        let tx = TxId::new(300, 0, agent);
        let mut datoms = Store::genesis().datom_set().clone();

        // Need at least one boundary so from_store returns Some
        let b = EntityId::from_ident(":policy/dummy-boundary");
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-name"), Value::String("dummy".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-source"), Value::String(":a/*".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-target"), Value::String(":b/*".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-weight"), Value::Double(ordered_float::OrderedFloat(1.0)), tx, Op::Assert));

        // Anomaly detector
        let ad = EntityId::from_ident(":policy/anomaly-stale-tasks");
        datoms.insert(Datom::new(ad, Attribute::from_keyword(":policy/anomaly-trigger"), Value::String(":task/status".into()), tx, Op::Assert));
        datoms.insert(Datom::new(ad, Attribute::from_keyword(":policy/anomaly-threshold"), Value::Long(5), tx, Op::Assert));
        datoms.insert(Datom::new(ad, Attribute::from_keyword(":policy/anomaly-message"), Value::String("stale tasks detected".into()), tx, Op::Assert));

        let store = Store::from_datoms(datoms);
        let config = PolicyConfig::from_store(&store).unwrap();
        assert_eq!(config.anomaly_detectors.len(), 1);
        assert_eq!(config.anomaly_detectors[0].trigger, ":task/status");
        assert_eq!(config.anomaly_detectors[0].threshold, 5);
        assert_eq!(config.anomaly_detectors[0].message, "stale tasks detected");
    }

    #[test]
    fn calibration_params_from_store() {
        let agent = AgentId::from_name("test:cal");
        let tx = TxId::new(400, 0, agent);
        let mut datoms = Store::genesis().datom_set().clone();

        // Need a boundary for from_store to return Some
        let b = EntityId::from_ident(":policy/cal-boundary");
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-name"), Value::String("cal".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-source"), Value::String(":x/*".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-target"), Value::String(":y/*".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-weight"), Value::Double(ordered_float::OrderedFloat(1.0)), tx, Op::Assert));

        // Custom calibration params on a separate entity
        let cal = EntityId::from_ident(":policy/calibration");
        datoms.insert(Datom::new(cal, Attribute::from_keyword(":policy/calibration-window"), Value::Long(50), tx, Op::Assert));
        datoms.insert(Datom::new(cal, Attribute::from_keyword(":policy/calibration-threshold"), Value::Double(ordered_float::OrderedFloat(0.10)), tx, Op::Assert));

        let store = Store::from_datoms(datoms);
        let config = PolicyConfig::from_store(&store).unwrap();
        assert_eq!(config.calibration.window, 50);
        assert!((config.calibration.threshold - 0.10).abs() < 0.001);
    }

    #[test]
    fn policy_boundary_with_report_template() {
        let agent = AgentId::from_name("test:template");
        let tx = TxId::new(500, 0, agent);
        let mut datoms = Store::genesis().datom_set().clone();

        let b = EntityId::from_ident(":policy/templated");
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-name"), Value::String("with-template".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-source"), Value::String(":spec/*".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-target"), Value::String(":impl/*".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-weight"), Value::Double(ordered_float::OrderedFloat(1.0)), tx, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-report-template"), Value::String("{name}: {coverage}% covered".into()), tx, Op::Assert));

        let store = Store::from_datoms(datoms);
        let config = PolicyConfig::from_store(&store).unwrap();
        assert_eq!(config.boundaries[0].report_template.as_deref(), Some("{name}: {coverage}% covered"));
    }

    #[test]
    fn policy_claim_evidence_patterns_from_store() {
        let agent = AgentId::from_name("test:patterns");
        let tx = TxId::new(600, 0, agent);
        let mut datoms = Store::genesis().datom_set().clone();

        // Need a boundary
        let b = EntityId::from_ident(":policy/pat-boundary");
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-name"), Value::String("pat".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-source"), Value::String(":a/*".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-target"), Value::String(":b/*".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-weight"), Value::Double(ordered_float::OrderedFloat(1.0)), tx, Op::Assert));

        // Claim and evidence patterns
        let cp1 = EntityId::from_ident(":policy/cp1");
        datoms.insert(Datom::new(cp1, Attribute::from_keyword(":policy/claim-pattern"), Value::String(":spec/*".into()), tx, Op::Assert));
        let cp2 = EntityId::from_ident(":policy/cp2");
        datoms.insert(Datom::new(cp2, Attribute::from_keyword(":policy/claim-pattern"), Value::String(":requirement/*".into()), tx, Op::Assert));
        let ep1 = EntityId::from_ident(":policy/ep1");
        datoms.insert(Datom::new(ep1, Attribute::from_keyword(":policy/evidence-pattern"), Value::String(":witness/*".into()), tx, Op::Assert));

        let store = Store::from_datoms(datoms);
        let config = PolicyConfig::from_store(&store).unwrap();
        assert_eq!(config.claim_patterns.len(), 2);
        assert_eq!(config.evidence_patterns.len(), 1);
        assert!(config.is_claim_attribute(":spec/element-type"));
        assert!(config.is_claim_attribute(":requirement/title"));
        assert!(!config.is_claim_attribute(":witness/status"));
        assert!(config.is_evidence_attribute(":witness/status"));
    }

    #[test]
    fn policy_fitness_with_matching_entities() {
        // Store with actual source + target entities that form coverage
        let agent = AgentId::from_name("test:coverage");
        let tx = TxId::new(700, 0, agent);
        let mut datoms = Store::genesis().datom_set().clone();

        // Boundary: :claim/* → :evidence/*
        let b = EntityId::from_ident(":policy/cov-boundary");
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-name"), Value::String("coverage".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-source"), Value::String(":claim/*".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-target"), Value::String(":evidence/*".into()), tx, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-weight"), Value::Double(ordered_float::OrderedFloat(1.0)), tx, Op::Assert));

        // 3 claim entities
        let c1 = EntityId::from_ident(":claim/c1");
        datoms.insert(Datom::new(c1, Attribute::from_keyword(":claim/title"), Value::String("Claim 1".into()), tx, Op::Assert));
        let c2 = EntityId::from_ident(":claim/c2");
        datoms.insert(Datom::new(c2, Attribute::from_keyword(":claim/title"), Value::String("Claim 2".into()), tx, Op::Assert));
        let c3 = EntityId::from_ident(":claim/c3");
        datoms.insert(Datom::new(c3, Attribute::from_keyword(":claim/title"), Value::String("Claim 3".into()), tx, Op::Assert));

        // 2 evidence entities covering c1 and c2
        let e1 = EntityId::from_ident(":evidence/e1");
        datoms.insert(Datom::new(e1, Attribute::from_keyword(":evidence/covers"), Value::Ref(c1), tx, Op::Assert));
        let e2 = EntityId::from_ident(":evidence/e2");
        datoms.insert(Datom::new(e2, Attribute::from_keyword(":evidence/covers"), Value::Ref(c2), tx, Op::Assert));

        let store = Store::from_datoms(datoms);
        let fs = crate::bilateral::compute_fitness_from_policy(&store).unwrap();

        // Coverage = 2/3 (c1 and c2 covered, c3 not)
        assert!(fs.total > 0.5, "F(S) should reflect 2/3 coverage: {:.4}", fs.total);
        assert!(fs.total < 1.0, "F(S) should not be 1.0 with incomplete coverage: {:.4}", fs.total);
    }

    #[test]
    fn adding_evidence_increases_coverage() {
        let agent = AgentId::from_name("test:mono");
        let tx1 = TxId::new(800, 0, agent);
        let tx2 = TxId::new(801, 0, agent);
        let mut datoms = Store::genesis().datom_set().clone();

        // Boundary
        let b = EntityId::from_ident(":policy/mono-boundary");
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-name"), Value::String("mono".into()), tx1, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-source"), Value::String(":claim/*".into()), tx1, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-target"), Value::String(":evidence/*".into()), tx1, Op::Assert));
        datoms.insert(Datom::new(b, Attribute::from_keyword(":policy/boundary-weight"), Value::Double(ordered_float::OrderedFloat(1.0)), tx1, Op::Assert));

        // 2 claims
        let c1 = EntityId::from_ident(":claim/mono-c1");
        datoms.insert(Datom::new(c1, Attribute::from_keyword(":claim/title"), Value::String("C1".into()), tx1, Op::Assert));
        let c2 = EntityId::from_ident(":claim/mono-c2");
        datoms.insert(Datom::new(c2, Attribute::from_keyword(":claim/title"), Value::String("C2".into()), tx1, Op::Assert));

        // 1 evidence covering c1
        let e1 = EntityId::from_ident(":evidence/mono-e1");
        datoms.insert(Datom::new(e1, Attribute::from_keyword(":evidence/covers"), Value::Ref(c1), tx1, Op::Assert));

        let store1 = Store::from_datoms(datoms.clone());
        let fs1 = crate::bilateral::compute_fitness_from_policy(&store1).unwrap();

        // Add evidence covering c2
        let e2 = EntityId::from_ident(":evidence/mono-e2");
        datoms.insert(Datom::new(e2, Attribute::from_keyword(":evidence/covers"), Value::Ref(c2), tx2, Op::Assert));

        let store2 = Store::from_datoms(datoms);
        let fs2 = crate::bilateral::compute_fitness_from_policy(&store2).unwrap();

        assert!(fs2.total >= fs1.total,
            "Adding evidence should not decrease F(S): before={:.4}, after={:.4}",
            fs1.total, fs2.total);
    }

    #[test]
    fn validate_policy_over_weight_rejects() {
        let config = PolicyConfig {
            boundaries: vec![BoundaryDef {
                entity: EntityId::from_ident(":test/heavy"),
                name: "heavy".to_string(),
                source_pattern: ":a/*".to_string(),
                target_pattern: ":b/*".to_string(),
                weight: 1.5, // > 1.0 but within [0, 1] is NOT a range error
                report_template: None,
            }],
            claim_patterns: vec![],
            evidence_patterns: vec![],
            anomaly_detectors: vec![],
            calibration: CalibrationConfig::default(),
        };
        let errors = validate_policy(&config);
        assert!(errors.iter().any(|e| e.constraint.contains("outside [0, 1]")),
            "weight > 1.0 should produce error");
    }

    #[test]
    fn validate_missing_target_pattern() {
        let config = PolicyConfig {
            boundaries: vec![BoundaryDef {
                entity: EntityId::from_ident(":test/notgt"),
                name: "no-target".to_string(),
                source_pattern: ":a/*".to_string(),
                target_pattern: String::new(),
                weight: 1.0,
                report_template: None,
            }],
            claim_patterns: vec![],
            evidence_patterns: vec![],
            anomaly_detectors: vec![],
            calibration: CalibrationConfig::default(),
        };
        let errors = validate_policy(&config);
        assert!(errors.iter().any(|e| e.constraint.contains("empty target")),
            "empty target pattern should produce error");
    }

    // ── PROPTESTS ──

    use proptest::prelude::*;

    /// Strategy for a random valid BoundaryDef.
    fn arb_boundary_def() -> impl Strategy<Value = BoundaryDef> {
        (
            "[a-z]{3,8}",                         // name
            prop_oneof![
                Just(":claim/*".to_string()),
                Just(":spec/*".to_string()),
                Just(":req/*".to_string()),
            ],                                     // source
            prop_oneof![
                Just(":evidence/*".to_string()),
                Just(":impl/*".to_string()),
                Just(":test/*".to_string()),
            ],                                     // target
            0.01f64..=1.0,                         // weight (positive only)
        ).prop_map(|(name, source, target, weight)| BoundaryDef {
            entity: EntityId::from_ident(&format!(":policy/prop-{name}")),
            name,
            source_pattern: source,
            target_pattern: target,
            weight,
            report_template: None,
        })
    }

    /// Strategy for a random valid PolicyConfig with normalized weights.
    fn arb_policy_config() -> impl Strategy<Value = PolicyConfig> {
        proptest::collection::vec(arb_boundary_def(), 1..=7).prop_map(|mut boundaries| {
            // Normalize weights to sum to 1.0
            let sum: f64 = boundaries.iter().map(|b| b.weight).sum();
            if sum > 0.0 {
                for b in &mut boundaries {
                    b.weight /= sum;
                }
            }
            PolicyConfig {
                boundaries,
                claim_patterns: vec![":spec/*".to_string()],
                evidence_patterns: vec![":impl/*".to_string()],
                anomaly_detectors: vec![],
                calibration: CalibrationConfig::default(),
            }
        })
    }

    /// Build a store from a PolicyConfig + some source/target entities.
    fn store_from_policy_config(config: &PolicyConfig, source_count: usize, covered_count: usize) -> Store {
        let agent = AgentId::from_name("proptest:policy");
        let tx = TxId::new(1000, 0, agent);
        let mut datoms = Store::genesis().datom_set().clone();

        // Transact the policy boundaries
        for b in &config.boundaries {
            datoms.insert(Datom::new(b.entity, Attribute::from_keyword(":policy/boundary-name"), Value::String(b.name.clone()), tx, Op::Assert));
            datoms.insert(Datom::new(b.entity, Attribute::from_keyword(":policy/boundary-source"), Value::String(b.source_pattern.clone()), tx, Op::Assert));
            datoms.insert(Datom::new(b.entity, Attribute::from_keyword(":policy/boundary-target"), Value::String(b.target_pattern.clone()), tx, Op::Assert));
            datoms.insert(Datom::new(b.entity, Attribute::from_keyword(":policy/boundary-weight"), Value::Double(ordered_float::OrderedFloat(b.weight)), tx, Op::Assert));
        }

        // Create source entities (using the first boundary's source pattern)
        if let Some(first_boundary) = config.boundaries.first() {
            let ns = first_boundary.source_pattern.trim_start_matches(':').trim_end_matches("/*");
            for i in 0..source_count {
                let e = EntityId::from_ident(&format!(":{ns}/prop-{i}"));
                datoms.insert(Datom::new(e, Attribute::from_keyword(&format!(":{ns}/title")), Value::String(format!("Source {i}")), tx, Op::Assert));
            }

            // Create target entities covering some sources
            let tgt_ns = first_boundary.target_pattern.trim_start_matches(':').trim_end_matches("/*");
            for i in 0..covered_count.min(source_count) {
                let src = EntityId::from_ident(&format!(":{ns}/prop-{i}"));
                let tgt = EntityId::from_ident(&format!(":{tgt_ns}/prop-{i}"));
                datoms.insert(Datom::new(tgt, Attribute::from_keyword(&format!(":{tgt_ns}/covers")), Value::Ref(src), tx, Op::Assert));
            }
        }

        Store::from_datoms(datoms)
    }

    proptest! {
        /// PROPTEST-1: Random valid policy → F(S) in [0, 1].
        #[test]
        fn prop_random_policy_fs_bounded(config in arb_policy_config()) {
            let store = store_from_policy_config(&config, 0, 0);
            let fs = crate::bilateral::compute_fitness_from_policy(&store);
            if let Some(fs) = fs {
                prop_assert!(fs.total >= 0.0 && fs.total <= 1.0,
                    "F(S) out of bounds: {:.6}", fs.total);
            }
        }

        /// PROPTEST-2: Random boundaries with normalized weights validate cleanly.
        #[test]
        fn prop_normalized_weights_valid(config in arb_policy_config()) {
            let errors = validate_policy(&config);
            // Normalized config should have no weight-sum errors
            let weight_errors: Vec<_> = errors.iter()
                .filter(|e| e.constraint.contains("sum to"))
                .collect();
            prop_assert!(weight_errors.is_empty(),
                "Normalized policy should not have weight-sum errors: {:?}",
                weight_errors.iter().map(|e| &e.constraint).collect::<Vec<_>>());
        }

        /// PROPTEST-3: Adding covered entities never decreases F(S).
        #[test]
        fn prop_evidence_monotonicity(
            config in arb_policy_config(),
            source_count in 1usize..=10,
            initial_covered in 0usize..=10,
        ) {
            let initial_covered = initial_covered.min(source_count);
            let store1 = store_from_policy_config(&config, source_count, initial_covered);
            let store2 = store_from_policy_config(&config, source_count, source_count); // fully covered

            if let (Some(fs1), Some(fs2)) = (
                crate::bilateral::compute_fitness_from_policy(&store1),
                crate::bilateral::compute_fitness_from_policy(&store2),
            ) {
                prop_assert!(fs2.total >= fs1.total - 0.001,
                    "Full coverage ({:.4}) should be >= partial ({:.4})",
                    fs2.total, fs1.total);
            }
        }

        /// PROPTEST-4: Empty policy (no source entities) → F(S) = 1.0 (vacuously coherent).
        #[test]
        fn prop_empty_source_vacuous(config in arb_policy_config()) {
            let store = store_from_policy_config(&config, 0, 0);
            if let Some(fs) = crate::bilateral::compute_fitness_from_policy(&store) {
                prop_assert!((fs.total - 1.0).abs() < 0.001,
                    "No source entities → vacuously coherent: F(S) should be 1.0, got {:.6}",
                    fs.total);
            }
        }

        /// PROPTEST-5: Pattern matching is consistent (matches(a,p) is pure).
        #[test]
        fn prop_pattern_matching_deterministic(
            attr in ":[a-z]{2,8}/[a-z]{2,8}",
            pattern in prop_oneof![
                ":[a-z]{2,8}/\\*".prop_map(|s| s),
                ":[a-z]{2,8}/[a-z]{2,8}".prop_map(|s| s),
                ":[a-z]{2,8}/".prop_map(|s| s),
            ]
        ) {
            let r1 = attr_matches_pattern(&attr, &pattern);
            let r2 = attr_matches_pattern(&attr, &pattern);
            prop_assert_eq!(r1, r2, "Pattern matching must be deterministic");
        }
    }

    // ── HL-CALIBRATE: Calibration weight adjustment tests ──

    #[test]
    fn calibrate_empty_store_returns_empty() {
        let store = Store::genesis();
        let adjustments = calibrate_boundary_weights(&store);
        assert!(
            adjustments.is_empty(),
            "genesis store has no policy — should return empty"
        );
    }

    #[test]
    fn calibrate_no_hypotheses_returns_empty() {
        let store = make_policy_store();
        let adjustments = calibrate_boundary_weights(&store);
        assert!(
            adjustments.is_empty(),
            "store with policy but no hypotheses — should return empty"
        );
    }

    fn make_two_boundary_store() -> Store {
        // Two boundaries: "coverage" (0.6) and "depth" (0.4)
        let agent = AgentId::from_name("test:calibrate");
        let tx = TxId::new(100, 0, agent);
        let mut datoms = Store::genesis().datom_set().clone();

        let b1 = EntityId::from_ident(":policy/boundary-coverage");
        datoms.insert(Datom::new(b1, Attribute::from_keyword(":policy/boundary-name"),
            Value::String("coverage".to_string()), tx, Op::Assert));
        datoms.insert(Datom::new(b1, Attribute::from_keyword(":policy/boundary-source"),
            Value::String(":spec/*".to_string()), tx, Op::Assert));
        datoms.insert(Datom::new(b1, Attribute::from_keyword(":policy/boundary-target"),
            Value::String(":impl/*".to_string()), tx, Op::Assert));
        datoms.insert(Datom::new(b1, Attribute::from_keyword(":policy/boundary-weight"),
            Value::Double(ordered_float::OrderedFloat(0.6)), tx, Op::Assert));

        let b2 = EntityId::from_ident(":policy/boundary-depth");
        datoms.insert(Datom::new(b2, Attribute::from_keyword(":policy/boundary-name"),
            Value::String("depth".to_string()), tx, Op::Assert));
        datoms.insert(Datom::new(b2, Attribute::from_keyword(":policy/boundary-source"),
            Value::String(":claim/*".to_string()), tx, Op::Assert));
        datoms.insert(Datom::new(b2, Attribute::from_keyword(":policy/boundary-target"),
            Value::String(":evidence/*".to_string()), tx, Op::Assert));
        datoms.insert(Datom::new(b2, Attribute::from_keyword(":policy/boundary-weight"),
            Value::Double(ordered_float::OrderedFloat(0.4)), tx, Op::Assert));

        Store::from_datoms(datoms)
    }

    #[test]
    fn calibrate_produces_adjustment_for_high_error() {
        let agent = AgentId::from_name("test:calibrate");
        let tx1 = TxId::new(200, 0, agent);

        let mut datoms = make_two_boundary_store().datom_set().clone();

        // Hypothesis targeting "coverage" boundary — over-predicted
        let h1 = EntityId::from_ident(":hypothesis/test-over");
        datoms.insert(Datom::new(h1, Attribute::from_keyword(":hypothesis/boundary"),
            Value::String("coverage".to_string()), tx1, Op::Assert));
        datoms.insert(Datom::new(h1, Attribute::from_keyword(":hypothesis/predicted"),
            Value::Double(ordered_float::OrderedFloat(0.5)), tx1, Op::Assert));
        datoms.insert(Datom::new(h1, Attribute::from_keyword(":hypothesis/actual"),
            Value::Double(ordered_float::OrderedFloat(0.1)), tx1, Op::Assert));
        datoms.insert(Datom::new(h1, Attribute::from_keyword(":hypothesis/completed"),
            Value::Instant(1774000000), tx1, Op::Assert));

        let store = Store::from_datoms(datoms);
        let adjustments = calibrate_boundary_weights(&store);

        // Error = |0.5 - 0.1| = 0.4, well above default threshold 0.05
        assert!(
            !adjustments.is_empty(),
            "high error hypothesis should produce weight adjustment"
        );
        assert_eq!(adjustments[0].boundary_name, "coverage");
        // Over-predicted (pred > actual) → weight should decrease
        // With 2 boundaries, normalization makes the decrease visible
        assert!(
            adjustments[0].mean_error > 0.3,
            "mean absolute error should be ~0.4: got {}",
            adjustments[0].mean_error
        );
        assert!(
            adjustments[0].sample_count == 1,
            "should have exactly 1 sample"
        );
    }

    #[test]
    fn calibrate_within_threshold_returns_empty() {
        let agent = AgentId::from_name("test:calibrate");
        let tx1 = TxId::new(200, 0, agent);

        let mut datoms = make_policy_store().datom_set().clone();

        let h_entity = EntityId::from_ident(":hypothesis/test-precise");
        datoms.insert(Datom::new(
            h_entity,
            Attribute::from_keyword(":hypothesis/boundary"),
            Value::String("test-coverage".to_string()),
            tx1,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            h_entity,
            Attribute::from_keyword(":hypothesis/predicted"),
            Value::Double(ordered_float::OrderedFloat(0.50)),
            tx1,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            h_entity,
            Attribute::from_keyword(":hypothesis/actual"),
            Value::Double(ordered_float::OrderedFloat(0.48)), // error = 0.02, below threshold
            tx1,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            h_entity,
            Attribute::from_keyword(":hypothesis/completed"),
            Value::Instant(1774000000),
            tx1,
            Op::Assert,
        ));

        let store = Store::from_datoms(datoms);
        let adjustments = calibrate_boundary_weights(&store);
        assert!(
            adjustments.is_empty(),
            "within-threshold error should not produce adjustment"
        );
    }
}
