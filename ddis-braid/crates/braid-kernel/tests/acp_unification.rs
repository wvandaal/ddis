// Witnesses: INV-BUDGET-007, INV-BUDGET-008, INV-BUDGET-009,
//   ADR-FOUNDATION-024, ADR-FOUNDATION-025,
//   INV-BUDGET-002, INV-BUDGET-006

//! ACP Unification test suite — verifies that all context blocks participate
//! in UAQ ranking via `ContextBlock::new_scored()` and that the precedence/score
//! sorting, novelty decay, presentation tracking, and calibration pipeline all
//! compose correctly after the ACP-SCORE-1 unification.
//!
//! Complements `tests/acquisition.rs` (algebraic AcquisitionScore properties)
//! by testing the INTEGRATION of scored blocks into the full ACP pipeline.
//!
//! Traces to: SEED.md §4 (datom abstraction), ADR-FOUNDATION-024/025 (UAQ)

use braid_kernel::budget::{
    novelty_from_count, AcquisitionScore, ActionProjection, ContextBlock, ObservationCost,
    ObservationKind, OutputPrecedence, ProjectedAction,
};
use braid_kernel::datom::{AgentId, Attribute, Op, Value};
use braid_kernel::guidance::{
    compute_calibration_metrics, compute_routing_with_calibration, record_block_presentations,
};
use braid_kernel::store::Store;

// ===========================================================================
// Unit tests: ContextBlock::new_scored() impact mapping
// ===========================================================================

/// ACP property: System precedence maps to maximum impact (1.0).
/// This ensures system-critical blocks (error messages, schema info, harvest
/// imperatives) always rank highest within the acquisition framework.
#[test]
fn new_scored_system_impact() {
    let block = ContextBlock::new_scored(OutputPrecedence::System, "system info".into(), 10);
    let score = block
        .attention
        .expect("new_scored must produce Some(attention)");
    assert!(
        (score.expected_delta_fs - 1.0).abs() < 0.01,
        "System precedence should produce ~1.0 impact (product of factors), got {}",
        score.expected_delta_fs
    );
}

/// ACP property: Methodology precedence maps to impact factor 0.8.
/// Methodology blocks (coherence metrics, drift signals) are second-highest priority.
#[test]
fn new_scored_methodology_impact() {
    let block =
        ContextBlock::new_scored(OutputPrecedence::Methodology, "methodology info".into(), 10);
    let score = block
        .attention
        .expect("new_scored must produce Some(attention)");
    // Impact factor is 0.8, but expected_delta_fs = impact * relevance * novelty * confidence
    // With defaults (1.0, 1.0, 1.0), expected_delta_fs = 0.8
    assert!(
        (score.expected_delta_fs - 0.8).abs() < 0.01,
        "Methodology precedence should produce 0.8 expected_delta_fs, got {}",
        score.expected_delta_fs
    );
}

/// ACP property: UserRequested precedence maps to impact factor 0.6.
/// Direct answers to user queries have moderate-high priority.
#[test]
fn new_scored_user_requested_impact() {
    let block = ContextBlock::new_scored(OutputPrecedence::UserRequested, "user answer".into(), 10);
    let score = block
        .attention
        .expect("new_scored must produce Some(attention)");
    assert!(
        (score.expected_delta_fs - 0.6).abs() < 0.01,
        "UserRequested precedence should produce 0.6 expected_delta_fs, got {}",
        score.expected_delta_fs
    );
}

/// ACP property: Speculative precedence maps to impact factor 0.4.
/// Suggestions and alternatives have lower priority than direct answers.
#[test]
fn new_scored_speculative_impact() {
    let block = ContextBlock::new_scored(OutputPrecedence::Speculative, "suggestion".into(), 10);
    let score = block
        .attention
        .expect("new_scored must produce Some(attention)");
    assert!(
        (score.expected_delta_fs - 0.4).abs() < 0.01,
        "Speculative precedence should produce 0.4 expected_delta_fs, got {}",
        score.expected_delta_fs
    );
}

/// ACP property: Ambient precedence maps to minimum impact factor 0.2.
/// Background/exploratory content gets the lowest priority in the UAQ ranking.
#[test]
fn new_scored_ambient_impact() {
    let block = ContextBlock::new_scored(OutputPrecedence::Ambient, "background".into(), 10);
    let score = block
        .attention
        .expect("new_scored must produce Some(attention)");
    assert!(
        (score.expected_delta_fs - 0.2).abs() < 0.01,
        "Ambient precedence should produce 0.2 expected_delta_fs, got {}",
        score.expected_delta_fs
    );
}

/// ACP unification invariant: new_scored() ALWAYS produces Some(attention).
/// This is the core property that eliminates the None/Some bifurcation —
/// every block participates in UAQ ranking, no exceptions.
#[test]
fn new_scored_always_has_attention() {
    let all_precedences = [
        OutputPrecedence::System,
        OutputPrecedence::Methodology,
        OutputPrecedence::UserRequested,
        OutputPrecedence::Speculative,
        OutputPrecedence::Ambient,
    ];

    for precedence in &all_precedences {
        let block = ContextBlock::new_scored(*precedence, "test content".into(), 5);
        assert!(
            block.attention.is_some(),
            "new_scored({:?}) must produce Some(attention), got None",
            precedence
        );
    }
}

/// ACP property: the token count argument propagates into the cost field.
/// This ensures the budget gate can correctly measure resource consumption
/// for each block in the acquisition function (alpha = E[delta_fs] / cost).
#[test]
fn new_scored_token_cost_propagates() {
    let tokens = 42;
    let block = ContextBlock::new_scored(OutputPrecedence::Methodology, "content".into(), tokens);
    let score = block.attention.expect("must have attention");
    assert!(
        (score.cost.attention_tokens - tokens) == 0,
        "token cost should propagate: expected {}, got {}",
        tokens,
        score.cost.attention_tokens
    );
}

/// ACP property: all scored blocks have positive alpha (E[delta_fs] / cost > 0).
/// A block with alpha == 0 would never be selected, defeating the purpose
/// of having it in the context at all.
#[test]
fn new_scored_alpha_positive() {
    let all_precedences = [
        OutputPrecedence::System,
        OutputPrecedence::Methodology,
        OutputPrecedence::UserRequested,
        OutputPrecedence::Speculative,
        OutputPrecedence::Ambient,
    ];

    for precedence in &all_precedences {
        // Use non-zero tokens to ensure cost is meaningful
        let block = ContextBlock::new_scored(*precedence, "test".into(), 10);
        let score = block.attention.expect("must have attention");
        assert!(
            score.alpha > 0.0,
            "new_scored({:?}) must produce positive alpha, got {}",
            precedence,
            score.alpha
        );
    }
}

// ===========================================================================
// Unit tests: novelty_from_count algebraic properties
// ===========================================================================

/// Algebraic properties of the novelty decay function: 1/sqrt(max(1, N)).
///
/// This function governs how quickly repeated presentations lose information value.
/// The properties ensure:
/// - N=0 (never seen) and N=1 (seen once) both give maximum novelty (1.0)
/// - N=4 gives exactly 0.5 (geometric midpoint)
/// - N=100 gives exactly 0.1 (rapid decay for well-known content)
/// - Strict monotonic decrease: more presentations means less novelty
#[test]
fn novelty_from_count_algebraic_properties() {
    // Fixed-point values
    assert!(
        (novelty_from_count(0) - 1.0).abs() < f64::EPSILON,
        "N=0 must give novelty 1.0, got {}",
        novelty_from_count(0)
    );
    assert!(
        (novelty_from_count(1) - 1.0).abs() < f64::EPSILON,
        "N=1 must give novelty 1.0, got {}",
        novelty_from_count(1)
    );
    assert!(
        (novelty_from_count(4) - 0.5).abs() < f64::EPSILON,
        "N=4 must give novelty 0.5, got {}",
        novelty_from_count(4)
    );
    assert!(
        (novelty_from_count(100) - 0.1).abs() < f64::EPSILON,
        "N=100 must give novelty 0.1, got {}",
        novelty_from_count(100)
    );

    // Monotonically decreasing: every increase in N decreases novelty
    let counts = [0, 1, 2, 3, 4, 9, 16, 25, 49, 100, 400, 10000];
    for window in counts.windows(2) {
        let (a, b) = (window[0], window[1]);
        assert!(
            novelty_from_count(a) >= novelty_from_count(b),
            "novelty must be monotonically decreasing: novelty({}) = {} >= novelty({}) = {}",
            a,
            novelty_from_count(a),
            b,
            novelty_from_count(b)
        );
    }
}

// ===========================================================================
// Unit tests: record_block_presentations retraction correctness
// ===========================================================================

/// Verify that repeated calls to record_block_presentations correctly retract
/// old counts before asserting new ones, so the LIVE count converges to the
/// actual number of presentations (not a cumulative sum of all historic values).
///
/// This test exercises the append-only retraction pattern (C1): retractions are
/// new datoms with op=Retract, so the total datom count grows, but the LIVE
/// count (Assert - Retract) for :attention/presentation-count should equal the
/// number of times we called record_block_presentations for that label.
#[test]
fn record_block_presentations_retraction() {
    let mut store = Store::genesis();
    // Add full schema (genesis only has Layer 1; attention attrs are Layer 2+)
    let schema_agent = AgentId::from_name("test:schema");
    let schema_tx_id = braid_kernel::datom::TxId::new(1, 0, schema_agent);
    let schema_datoms = braid_kernel::schema::full_schema_datoms(schema_tx_id);
    let mut tx = braid_kernel::store::Transaction::new(
        schema_agent,
        braid_kernel::datom::ProvenanceType::Derived,
        "bootstrap schema for test",
    );
    for d in &schema_datoms {
        tx = tx.assert(d.entity, d.attribute.clone(), d.value.clone());
    }
    let committed = tx.commit(&store).expect("schema commit");
    store.transact(committed).expect("schema transact");
    let agent = AgentId::from_name("test:acp");
    let count_attr = Attribute::from_keyword(":attention/presentation-count");
    let label = "test-block";

    // Call record_block_presentations 3 times for the same label, transacting each batch
    for i in 0..3 {
        let tx_id = braid_kernel::datom::TxId::new(1000 + i, 0, agent);
        let datoms = record_block_presentations(&store, &[label], tx_id);

        // Build and transact via Transaction (schema-validated path)
        let mut tx = braid_kernel::store::Transaction::new(
            agent,
            braid_kernel::datom::ProvenanceType::Observed,
            &format!("Record presentations round {}", i),
        );
        for d in &datoms {
            if d.op == Op::Assert {
                tx = tx.assert(d.entity, d.attribute.clone(), d.value.clone());
            } else {
                tx = tx.retract(d.entity, d.attribute.clone(), d.value.clone());
            }
        }
        let committed = tx.commit(&store).expect("commit must succeed");
        store.transact(committed).expect("transact must succeed");
    }

    // Now count LIVE :attention/presentation-count datoms for this label.
    // There should be exactly one Assert that isn't retracted, with value == 3.
    let all_count_datoms: Vec<_> = store
        .attribute_datoms(&count_attr)
        .iter()
        .filter(|d| {
            // Only consider datoms for the entity with our test label
            let label_attr = Attribute::from_keyword(":attention/block-label");
            store.entity_datoms(d.entity).iter().any(|ed| {
                ed.attribute == label_attr
                    && ed.op == Op::Assert
                    && matches!(&ed.value, Value::String(s) if s == label)
            })
        })
        .collect();

    let assert_count: i64 = all_count_datoms
        .iter()
        .filter(|d| d.op == Op::Assert)
        .count() as i64;
    let retract_count: i64 = all_count_datoms
        .iter()
        .filter(|d| d.op == Op::Retract)
        .count() as i64;

    // After 3 rounds: initial assert (1), retract 1 + assert (2), retract 2 + assert (3)
    // Total asserts: 3, total retracts: 2, LIVE count: 1 entity with value 3
    assert_eq!(
        assert_count, 3,
        "should have 3 assert datoms for presentation-count (one per round)"
    );
    assert_eq!(
        retract_count, 2,
        "should have 2 retract datoms for presentation-count (one per update)"
    );

    // The LIVE value (latest assert not retracted) should be 3
    let live_value = all_count_datoms
        .iter()
        .filter(|d| d.op == Op::Assert)
        .filter_map(|d| match &d.value {
            Value::Long(n) => Some(*n),
            _ => None,
        })
        .max()
        .expect("should have at least one assert");
    assert_eq!(
        live_value, 3,
        "LIVE presentation count should be 3 after 3 rounds"
    );
}

// ===========================================================================
// Unit tests: calibration cache equivalence
// ===========================================================================

/// Verify that compute_calibration_metrics returns the same CalibrationReport
/// whether called directly or obtained via compute_routing_with_calibration.
///
/// This ensures there is no divergence between the two code paths — both should
/// produce identical calibration data from the same store state.
#[test]
fn calibration_cache_equivalence() {
    let store = Store::genesis();

    let direct = compute_calibration_metrics(&store);
    let (_, via_routing) = compute_routing_with_calibration(&store);

    assert_eq!(
        direct.total_hypotheses, via_routing.total_hypotheses,
        "total_hypotheses must match: direct={} vs routing={}",
        direct.total_hypotheses, via_routing.total_hypotheses
    );
    assert_eq!(
        direct.completed_hypotheses, via_routing.completed_hypotheses,
        "completed_hypotheses must match"
    );
    assert!(
        (direct.mean_error - via_routing.mean_error).abs() < f64::EPSILON,
        "mean_error must match: direct={} vs routing={}",
        direct.mean_error,
        via_routing.mean_error
    );
    assert_eq!(
        direct.per_boundary_accuracy.len(),
        via_routing.per_boundary_accuracy.len(),
        "per_boundary_accuracy entry count must match"
    );
    assert_eq!(
        direct.per_type_accuracy.len(),
        via_routing.per_type_accuracy.len(),
        "per_type_accuracy entry count must match"
    );
}

// ===========================================================================
// Unit tests: project() sorting — precedence then score
// ===========================================================================

/// Verify that ActionProjection::project() sorts blocks by precedence first,
/// then by acquisition score (descending) within each precedence tier.
///
/// System blocks always appear before Methodology blocks regardless of score.
/// Within Methodology, higher-scored blocks appear first.
/// This is the core INV-BUDGET-002 (precedence ordering) + UAQ integration test.
#[test]
fn project_sorts_by_precedence_then_score() {
    let high_method_score = AcquisitionScore::from_factors(
        ObservationKind::ContextBlock,
        0.9,
        1.0,
        1.0,
        1.0,
        ObservationCost::from_tokens(5),
    );
    let low_method_score = AcquisitionScore::from_factors(
        ObservationKind::ContextBlock,
        0.1,
        1.0,
        1.0,
        1.0,
        ObservationCost::from_tokens(5),
    );
    let system_block_score = AcquisitionScore::from_factors(
        ObservationKind::ContextBlock,
        0.05, // intentionally LOW score
        1.0,
        1.0,
        1.0,
        ObservationCost::from_tokens(5),
    );

    let proj = ActionProjection {
        action: ProjectedAction {
            command: "braid status".to_string(),
            rationale: "test".to_string(),
            impact: 0.5,
        },
        context: vec![
            // Insert in "wrong" order to verify sorting works
            ContextBlock {
                precedence: OutputPrecedence::Methodology,
                content: "LOW-METHOD".to_string(),
                tokens: 3,
                attention: Some(low_method_score),
            },
            ContextBlock {
                precedence: OutputPrecedence::System,
                content: "SYSTEM-LOW-SCORE".to_string(),
                tokens: 3,
                attention: Some(system_block_score),
            },
            ContextBlock {
                precedence: OutputPrecedence::Methodology,
                content: "HIGH-METHOD".to_string(),
                tokens: 3,
                attention: Some(high_method_score),
            },
        ],
        evidence_pointer: String::new(),
    };

    let output = proj.project(200);

    // System block must come first (precedence wins over score)
    let sys_pos = output
        .find("SYSTEM-LOW-SCORE")
        .expect("system block should appear in output");
    let high_pos = output
        .find("HIGH-METHOD")
        .expect("high-method block should appear in output");
    let low_pos = output
        .find("LOW-METHOD")
        .expect("low-method block should appear in output");

    assert!(
        sys_pos < high_pos,
        "System block must appear before Methodology (even with low score): sys={}, high={}",
        sys_pos,
        high_pos
    );
    assert!(
        high_pos < low_pos,
        "Higher-scored Methodology block must appear before lower-scored: high={}, low={}",
        high_pos,
        low_pos
    );
}

// ===========================================================================
// Unit tests: from_command_output produces valid ACP
// ===========================================================================

/// Verify that ActionProjection::from_command_output() produces a valid
/// projection with System-precedence block and non-empty action.
///
/// This is the primary constructor for wrapping raw command output into the
/// ACP framework (INV-BUDGET-007: every output contains a complete action).
#[test]
fn from_command_output_produces_valid_acp() {
    let output = "Status: 42 datoms, 7 entities, F(S)=0.62";
    let proj = ActionProjection::from_command_output(output, "status");

    // Action must be non-empty
    assert!(
        !proj.action.command.is_empty(),
        "action command must be non-empty"
    );
    assert!(
        proj.action.command.contains("braid"),
        "action command should contain 'braid': got '{}'",
        proj.action.command
    );

    // Context must have exactly one block at System precedence
    assert_eq!(
        proj.context.len(),
        1,
        "from_command_output should produce exactly 1 context block"
    );
    assert_eq!(
        proj.context[0].precedence,
        OutputPrecedence::System,
        "context block should be System precedence"
    );

    // Block content must contain the original output
    assert!(
        proj.context[0].content.contains(output),
        "context block should contain the original output"
    );

    // Block must have a valid AcquisitionScore (new_scored path)
    let score = proj.context[0]
        .attention
        .as_ref()
        .expect("from_command_output block must have attention score");
    assert!(score.alpha > 0.0, "System block must have positive alpha");
    assert!(
        (score.expected_delta_fs - 1.0).abs() < 0.01,
        "System block should have ~1.0 expected_delta_fs (System impact factor)"
    );
}

// ===========================================================================
// Property-based tests (proptest)
// ===========================================================================

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    /// Precedence strategy for property tests.
    fn arb_precedence() -> impl Strategy<Value = OutputPrecedence> {
        prop_oneof![
            Just(OutputPrecedence::System),
            Just(OutputPrecedence::Methodology),
            Just(OutputPrecedence::UserRequested),
            Just(OutputPrecedence::Speculative),
            Just(OutputPrecedence::Ambient),
        ]
    }

    // ACP property: for any precedence level, new_scored() produces impact in [0.2, 1.0].
    // The lower bound (0.2) comes from Ambient precedence, the upper bound (1.0) from System.
    proptest! {
        #[test]
        fn prop_new_scored_impact_bounded(precedence in arb_precedence()) {
            let block = ContextBlock::new_scored(precedence, "test".into(), 10);
            let score = block.attention.expect("must have attention");
            prop_assert!(
                score.expected_delta_fs >= 0.2 && score.expected_delta_fs <= 1.0,
                "expected_delta_fs must be in [0.2, 1.0], got {} for {:?}",
                score.expected_delta_fs,
                precedence
            );
        }
    }

    // ACP property: for any precedence and token count in [1, 10000], alpha >= 0.
    // Alpha is the cost-adjusted ranking value (E[delta_fs] / cost).
    proptest! {
        #[test]
        fn prop_new_scored_alpha_nonnegative(
            precedence in arb_precedence(),
            tokens in 1usize..10000,
        ) {
            let block = ContextBlock::new_scored(precedence, "test".into(), tokens);
            let score = block.attention.expect("must have attention");
            prop_assert!(
                score.alpha >= 0.0,
                "alpha must be >= 0 for {:?} with {} tokens, got {}",
                precedence,
                tokens,
                score.alpha
            );
        }
    }

    // Novelty monotonicity: for any a < b in [0, 10000],
    // novelty_from_count(a) >= novelty_from_count(b) (1/sqrt(N) is non-increasing).
    proptest! {
        #[test]
        fn prop_novelty_monotonic(a in 0u64..10000, b in 0u64..10000) {
            // Only check when a < b (otherwise the property is trivially true)
            prop_assume!(a < b);
            let novelty_a = novelty_from_count(a);
            let novelty_b = novelty_from_count(b);
            prop_assert!(
                novelty_a >= novelty_b,
                "novelty must be monotonically decreasing: novelty({}) = {} >= novelty({}) = {}",
                a, novelty_a, b, novelty_b
            );
        }
    }
}
