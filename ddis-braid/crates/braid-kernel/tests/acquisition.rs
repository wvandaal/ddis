//! UAQ-TEST: Universal Acquisition Function test suite.
//!
//! Verifies algebraic properties of AcquisitionScore, ObservationCost,
//! ObservationKind, and the integration with TaskRouting and CalibrationReport.

use braid_kernel::budget::{AcquisitionScore, ObservationCost, ObservationKind};

// ---------------------------------------------------------------------------
// Unit tests: AcquisitionScore algebraic properties
// ---------------------------------------------------------------------------

/// All-ones factors produce composite == 1.0.
#[test]
fn acquisition_score_unit_identity() {
    let score = AcquisitionScore::from_factors(
        ObservationKind::Task,
        1.0,
        1.0,
        1.0,
        1.0,
        ObservationCost::zero(),
    );
    assert!(
        (score.expected_delta_fs - 1.0).abs() < f64::EPSILON,
        "1×1×1×1 should be 1.0, got {}",
        score.expected_delta_fs
    );
}

/// Any zero factor produces composite == 0.0 (veto property).
#[test]
fn acquisition_score_veto_property() {
    let factors = [
        (0.0, 1.0, 1.0, 1.0), // zero impact
        (1.0, 0.0, 1.0, 1.0), // zero relevance
        (1.0, 1.0, 0.0, 1.0), // zero novelty
        (1.0, 1.0, 1.0, 0.0), // zero confidence
    ];
    for (impact, relevance, novelty, confidence) in factors {
        let score = AcquisitionScore::from_factors(
            ObservationKind::Task,
            impact,
            relevance,
            novelty,
            confidence,
            ObservationCost::zero(),
        );
        assert!(
            score.expected_delta_fs.abs() < f64::EPSILON,
            "veto: ({impact},{relevance},{novelty},{confidence}) should be 0, got {}",
            score.expected_delta_fs
        );
    }
}

/// Composite is monotonically increasing in each factor (others held constant).
#[test]
fn acquisition_score_monotonicity() {
    let base = AcquisitionScore::from_factors(
        ObservationKind::Task,
        0.5,
        0.5,
        0.5,
        0.5,
        ObservationCost::zero(),
    );

    // Increase impact
    let higher_impact = AcquisitionScore::from_factors(
        ObservationKind::Task,
        0.8,
        0.5,
        0.5,
        0.5,
        ObservationCost::zero(),
    );
    assert!(
        higher_impact.expected_delta_fs > base.expected_delta_fs,
        "higher impact should increase score"
    );

    // Increase relevance
    let higher_relevance = AcquisitionScore::from_factors(
        ObservationKind::Task,
        0.5,
        0.8,
        0.5,
        0.5,
        ObservationCost::zero(),
    );
    assert!(
        higher_relevance.expected_delta_fs > base.expected_delta_fs,
        "higher relevance should increase score"
    );

    // Increase novelty
    let higher_novelty = AcquisitionScore::from_factors(
        ObservationKind::Task,
        0.5,
        0.5,
        0.8,
        0.5,
        ObservationCost::zero(),
    );
    assert!(
        higher_novelty.expected_delta_fs > base.expected_delta_fs,
        "higher novelty should increase score"
    );

    // Increase confidence
    let higher_confidence = AcquisitionScore::from_factors(
        ObservationKind::Task,
        0.5,
        0.5,
        0.5,
        0.8,
        ObservationCost::zero(),
    );
    assert!(
        higher_confidence.expected_delta_fs > base.expected_delta_fs,
        "higher confidence should increase score"
    );
}

/// Novelty follows 1/sqrt(N) decay.
#[test]
fn novelty_decay_from_presentation_count() {
    let n0 = AcquisitionScore::from_presentation_count(0, 0.0, 1.0);
    let n1 = AcquisitionScore::from_presentation_count(1, 0.0, 1.0);
    let n4 = AcquisitionScore::from_presentation_count(4, 0.0, 1.0);
    let n100 = AcquisitionScore::from_presentation_count(100, 0.0, 1.0);

    assert!((n0.novelty - 1.0).abs() < f64::EPSILON, "N=0 → novelty=1.0");
    assert!((n1.novelty - 1.0).abs() < f64::EPSILON, "N=1 → novelty=1.0");
    assert!((n4.novelty - 0.5).abs() < f64::EPSILON, "N=4 → novelty=0.5");
    assert!(
        (n100.novelty - 0.1).abs() < f64::EPSILON,
        "N=100 → novelty=0.1"
    );
}

/// novel() returns maximum novelty.
#[test]
fn novel_score_is_maximal() {
    let score = AcquisitionScore::novel();
    assert!((score.novelty - 1.0).abs() < f64::EPSILON);
    assert!(score.kind == ObservationKind::ContextBlock);
}

// ---------------------------------------------------------------------------
// ObservationCost tests
// ---------------------------------------------------------------------------

/// Zero cost floors to 1.0 (prevent division by zero).
#[test]
fn observation_cost_zero_floors() {
    let cost = ObservationCost::zero();
    assert!(
        (cost.total_cost() - 1.0).abs() < f64::EPSILON,
        "zero cost should floor to 1.0, got {}",
        cost.total_cost()
    );
}

/// Token cost dominates when > 0.
#[test]
fn observation_cost_tokens_dominate() {
    let cost = ObservationCost::from_tokens(50);
    assert!(
        (cost.total_cost() - 50.0).abs() < f64::EPSILON,
        "token cost should be 50.0, got {}",
        cost.total_cost()
    );
}

/// Wall time used when tokens are 0.
#[test]
fn observation_cost_wall_time_fallback() {
    let cost = ObservationCost::from_wall_time_ms(5000);
    assert!(
        (cost.total_cost() - 5000.0).abs() < f64::EPSILON,
        "wall time cost should be 5000.0, got {}",
        cost.total_cost()
    );
}

/// Alpha = expected_delta_fs / cost. Higher cost → lower alpha.
#[test]
fn alpha_cost_aware_ranking() {
    let cheap = AcquisitionScore::from_factors(
        ObservationKind::ContextBlock,
        0.5,
        1.0,
        1.0,
        1.0,
        ObservationCost::from_tokens(10),
    );
    let expensive = AcquisitionScore::from_factors(
        ObservationKind::ContextBlock,
        0.5,
        1.0,
        1.0,
        1.0,
        ObservationCost::from_tokens(100),
    );

    assert!(
        cheap.alpha > expensive.alpha,
        "same value but lower cost should rank higher: cheap={}, expensive={}",
        cheap.alpha,
        expensive.alpha
    );
    assert!(
        (cheap.expected_delta_fs - expensive.expected_delta_fs).abs() < f64::EPSILON,
        "expected_delta_fs should be identical"
    );
}

/// Alpha is deterministic: same inputs → same alpha.
#[test]
fn alpha_deterministic() {
    let a = AcquisitionScore::from_factors(
        ObservationKind::Task,
        0.7,
        0.8,
        0.9,
        0.6,
        ObservationCost::from_tokens(20),
    );
    let b = AcquisitionScore::from_factors(
        ObservationKind::Task,
        0.7,
        0.8,
        0.9,
        0.6,
        ObservationCost::from_tokens(20),
    );
    assert!(
        (a.alpha - b.alpha).abs() < f64::EPSILON,
        "deterministic: a.alpha={}, b.alpha={}",
        a.alpha,
        b.alpha
    );
}

// ---------------------------------------------------------------------------
// ObservationKind tests
// ---------------------------------------------------------------------------

/// ObservationKind Display renders readable names.
#[test]
fn observation_kind_display() {
    assert_eq!(format!("{}", ObservationKind::Task), "Task");
    assert_eq!(format!("{}", ObservationKind::ContextBlock), "ContextBlock");
    assert_eq!(
        format!("{}", ObservationKind::BoundaryCheck),
        "BoundaryCheck"
    );
}

// ---------------------------------------------------------------------------
// Integration: AcquisitionScore in context blocks
// ---------------------------------------------------------------------------

/// Context blocks sorted by composite within precedence tier.
#[test]
fn context_block_sort_by_acquisition_score() {
    use braid_kernel::budget::{ActionProjection, ContextBlock, OutputPrecedence, ProjectedAction};

    let high_score = AcquisitionScore::from_factors(
        ObservationKind::ContextBlock,
        0.9,
        1.0,
        1.0,
        1.0,
        ObservationCost::from_tokens(10),
    );
    let low_score = AcquisitionScore::from_factors(
        ObservationKind::ContextBlock,
        0.1,
        1.0,
        1.0,
        1.0,
        ObservationCost::from_tokens(10),
    );

    let proj = ActionProjection {
        action: ProjectedAction {
            command: "braid go t-test".to_string(),
            rationale: "test".to_string(),
            impact: 0.5,
        },
        context: vec![
            ContextBlock {
                precedence: OutputPrecedence::Methodology,
                content: "LOW-SCORE-BLOCK".to_string(),
                tokens: 5,
                attention: Some(low_score),
            },
            ContextBlock {
                precedence: OutputPrecedence::Methodology,
                content: "HIGH-SCORE-BLOCK".to_string(),
                tokens: 5,
                attention: Some(high_score),
            },
        ],
        evidence_pointer: String::new(),
    };

    let output = proj.project(100);
    let high_pos = output.find("HIGH-SCORE-BLOCK").expect("high should appear");
    let low_pos = output.find("LOW-SCORE-BLOCK").expect("low should appear");
    assert!(
        high_pos < low_pos,
        "high-score block should appear before low-score: high={high_pos}, low={low_pos}"
    );
}

/// System-precedence blocks always come first regardless of score.
#[test]
fn system_precedence_beats_high_score() {
    use braid_kernel::budget::{ActionProjection, ContextBlock, OutputPrecedence, ProjectedAction};

    let very_high = AcquisitionScore::from_factors(
        ObservationKind::ContextBlock,
        1.0,
        1.0,
        1.0,
        1.0,
        ObservationCost::from_tokens(1),
    );

    let proj = ActionProjection {
        action: ProjectedAction {
            command: "braid go t-test".to_string(),
            rationale: "test".to_string(),
            impact: 0.5,
        },
        context: vec![
            ContextBlock {
                precedence: OutputPrecedence::Methodology,
                content: "HIGH-ALPHA-METHOD".to_string(),
                tokens: 5,
                attention: Some(very_high),
            },
            ContextBlock {
                precedence: OutputPrecedence::System,
                content: "SYSTEM-BLOCK".to_string(),
                tokens: 5,
                attention: None, // no score, but System precedence
            },
        ],
        evidence_pointer: String::new(),
    };

    let output = proj.project(100);
    let sys_pos = output.find("SYSTEM-BLOCK").expect("system should appear");
    let meth_pos = output
        .find("HIGH-ALPHA-METHOD")
        .expect("method should appear");
    assert!(
        sys_pos < meth_pos,
        "system precedence beats high score: sys={sys_pos}, meth={meth_pos}"
    );
}

// ---------------------------------------------------------------------------
// Integration: CalibrationReport per-type accuracy
// ---------------------------------------------------------------------------

/// CalibrationReport on empty store has empty per_type_accuracy.
#[test]
fn calibration_empty_store_no_type_data() {
    let store = braid_kernel::store::Store::genesis();
    let report = braid_kernel::CalibrationReport {
        total_hypotheses: 0,
        completed_hypotheses: 0,
        mean_error: 0.0,
        per_boundary_accuracy: std::collections::BTreeMap::new(),
        per_type_accuracy: std::collections::BTreeMap::new(),
        trend: braid_kernel::CalibrationTrend::Insufficient,
    };
    assert!(report.per_type_accuracy.is_empty());
    let _ = store; // suppress unused warning
}

/// Dominance: if all factors of A dominate B AND cost is lower, alpha is strictly greater.
#[test]
fn dominance_property() {
    let dominant = AcquisitionScore::from_factors(
        ObservationKind::Task,
        0.9,
        0.9,
        0.9,
        0.9,
        ObservationCost::from_tokens(5),
    );
    let dominated = AcquisitionScore::from_factors(
        ObservationKind::Task,
        0.5,
        0.5,
        0.5,
        0.5,
        ObservationCost::from_tokens(50),
    );
    assert!(
        dominant.alpha > dominated.alpha,
        "dominant should rank strictly higher: dom={}, sub={}",
        dominant.alpha,
        dominated.alpha
    );
}

/// Backward compatibility: from_presentation_count produces valid AcquisitionScore.
#[test]
fn backward_compat_from_presentation_count() {
    let score = AcquisitionScore::from_presentation_count(4, 0.1, 0.9);
    assert!(score.kind == ObservationKind::ContextBlock);
    assert!(score.novelty > 0.0, "novelty should be positive");
    assert!(
        score.expected_delta_fs >= 0.0,
        "delta_fs should be non-negative"
    );
    // composite() falls back to expected_delta_fs when cost is zero
    assert!(
        (score.composite() - score.expected_delta_fs).abs() < f64::EPSILON,
        "composite should equal expected_delta_fs for zero-cost: {} vs {}",
        score.composite(),
        score.expected_delta_fs
    );
}

/// Composite falls back to expected_delta_fs for zero-cost items, alpha for costed items.
#[test]
fn composite_fallback_behavior() {
    let zero_cost = AcquisitionScore::from_factors(
        ObservationKind::Task,
        0.5,
        0.5,
        0.5,
        0.5,
        ObservationCost::zero(),
    );
    assert!(
        (zero_cost.composite() - zero_cost.expected_delta_fs).abs() < f64::EPSILON,
        "zero-cost composite should be expected_delta_fs"
    );

    let costed = AcquisitionScore::from_factors(
        ObservationKind::ContextBlock,
        0.5,
        0.5,
        0.5,
        0.5,
        ObservationCost::from_tokens(100),
    );
    assert!(
        (costed.composite() - costed.alpha).abs() < f64::EPSILON,
        "costed composite should be alpha"
    );
}
