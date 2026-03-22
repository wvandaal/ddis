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
fn attr_matches_pattern(attr: &str, pattern: &str) -> bool {
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
}
