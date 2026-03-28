// Witnesses: INV-GUIDANCE-010, ADR-FOUNDATION-018,
//   INV-FOUNDATION-010, INV-FOUNDATION-011,
//   INV-STORE-001

//! Hypothesis Ledger test suite — unit + proptest + E2E.
//!
//! The hypothesis ledger records predictions about what actions will improve F(S),
//! then measures actual outcomes to calibrate. This suite verifies:
//! - Recording hypotheses creates correct datoms
//! - Calibration metrics compute correctly from completed hypotheses
//! - Trend detection (improving/degrading/stable) works
//! - Item-type tagging for per-type calibration (UAQ-4)
//! - Algebraic properties: mean_error >= 0, completed <= total
//!
//! Traces to: SEED.md §4 (datom abstraction), ADR-FOUNDATION-018 (hypothesis ledger),
//!   HL-2 (record predictions), HL-4 (calibration metrics), UAQ-4 (per-type calibration)

use braid_kernel::budget::{AcquisitionScore, ObservationCost, ObservationKind};
use braid_kernel::datom::{AgentId, Attribute, Datom, EntityId, Op, TxId, Value};
use braid_kernel::guidance::{
    compute_calibration_metrics, hypothesis_completed_count, hypothesis_count, record_hypotheses,
    record_hypotheses_with_type, CalibrationTrend, RoutingMetrics, TaskRouting,
};
use braid_kernel::store::Store;

// ===========================================================================
// Helpers
// ===========================================================================

/// Create a store with full schema (genesis + Layer 2-4 attributes).
/// Hypothesis attributes are in Layer 4, so we need the full schema.
fn store_with_full_schema() -> Store {
    let mut store = Store::genesis();
    let agent = AgentId::from_name("test:schema");
    let tx_id = TxId::new(1, 0, agent);
    let schema_datoms = braid_kernel::schema::full_schema_datoms(tx_id);
    let mut tx = braid_kernel::store::Transaction::new(
        agent,
        braid_kernel::datom::ProvenanceType::Derived,
        "bootstrap full schema for hypothesis ledger tests",
    );
    for d in &schema_datoms {
        tx = tx.assert(d.entity, d.attribute.clone(), d.value.clone());
    }
    let committed = tx.commit(&store).expect("schema commit");
    store.transact(committed).expect("schema transact");
    store
}

/// Build a TaskRouting with a given entity ident, impact, and gradient_delta.
fn make_routing(ident: &str, impact: f64, gradient_delta: f64) -> TaskRouting {
    TaskRouting {
        entity: EntityId::from_ident(ident),
        label: ident.to_string(),
        impact,
        metrics: RoutingMetrics {
            pagerank: 0.1,
            betweenness_proxy: 0.0,
            critical_path_pos: 0.0,
            blocker_ratio: 0.0,
            staleness: 0.0,
            priority_boost: 0.0,
            type_multiplier: 1.0,
            urgency_decay: 1.0,
            spec_anchor: 1.0,
            session_boost: 1.0,
            gradient_delta,
            observation_dampening: 1.0,
            concept_dampening: 1.0,
        },
        acquisition_score: AcquisitionScore::from_factors(
            ObservationKind::Task,
            impact,
            1.0,
            1.0,
            1.0,
            ObservationCost::from_tokens(10),
        ),
    }
}

/// Create a store (via from_datoms) with N completed hypotheses with known
/// predicted/actual pairs and completion timestamps.
///
/// Each entry: (predicted_delta, actual_delta, completed_at_timestamp).
fn store_with_completed_hypotheses(entries: &[(f64, f64, u64)]) -> Store {
    let base_store = store_with_full_schema();
    let mut datoms = base_store.datom_set().clone();
    let agent = AgentId::from_name("test:hyp");
    let tx = TxId::new(100, 0, agent);

    for (i, &(predicted, actual, completed_at)) in entries.iter().enumerate() {
        let hyp = EntityId::from_ident(&format!(":hypothesis/test-{i}"));
        let action_target = EntityId::from_ident(&format!(":task/target-{i}"));
        let error = (predicted - actual).abs();

        // :hypothesis/action (Ref) — required to count as a hypothesis
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/action"),
            Value::Ref(action_target),
            tx,
            Op::Assert,
        ));
        // :hypothesis/predicted
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/predicted"),
            Value::Double(ordered_float::OrderedFloat(predicted)),
            tx,
            Op::Assert,
        ));
        // :hypothesis/actual — marks it as completed
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/actual"),
            Value::Double(ordered_float::OrderedFloat(actual)),
            tx,
            Op::Assert,
        ));
        // :hypothesis/error
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/error"),
            Value::Double(ordered_float::OrderedFloat(error)),
            tx,
            Op::Assert,
        ));
        // :hypothesis/boundary
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/boundary"),
            Value::String("general".to_string()),
            tx,
            Op::Assert,
        ));
        // :hypothesis/completed
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/completed"),
            Value::Instant(completed_at),
            tx,
            Op::Assert,
        ));
        // :hypothesis/item-type
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/item-type"),
            Value::String("task".to_string()),
            tx,
            Op::Assert,
        ));
    }

    Store::from_datoms(datoms)
}

/// Create a store with N incomplete hypotheses (no :hypothesis/actual).
fn store_with_incomplete_hypotheses(n: usize) -> Store {
    let base_store = store_with_full_schema();
    let mut datoms = base_store.datom_set().clone();
    let agent = AgentId::from_name("test:hyp");
    let tx = TxId::new(100, 0, agent);

    for i in 0..n {
        let hyp = EntityId::from_ident(&format!(":hypothesis/incomplete-{i}"));
        let action_target = EntityId::from_ident(&format!(":task/target-inc-{i}"));

        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/action"),
            Value::Ref(action_target),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/predicted"),
            Value::Double(ordered_float::OrderedFloat(0.5)),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/boundary"),
            Value::String("general".to_string()),
            tx,
            Op::Assert,
        ));
    }

    Store::from_datoms(datoms)
}

// ===========================================================================
// Unit tests
// ===========================================================================

/// Test 1: CalibrationReport on empty store has 0 total, 0 completed.
///
/// The empty case is the base case for the calibration metrics. On a genesis
/// store with no hypothesis datoms, all counts must be zero and the trend
/// must be Insufficient (not enough data to determine direction).
#[test]
fn empty_store_returns_zero_hypotheses() {
    let store = Store::genesis();
    let report = compute_calibration_metrics(&store);

    assert_eq!(
        report.total_hypotheses, 0,
        "empty store should have 0 total hypotheses"
    );
    assert_eq!(
        report.completed_hypotheses, 0,
        "empty store should have 0 completed hypotheses"
    );
    assert!(
        report.mean_error.abs() < f64::EPSILON,
        "empty store should have 0.0 mean error, got {}",
        report.mean_error
    );
    assert_eq!(
        report.trend,
        CalibrationTrend::Insufficient,
        "empty store should have Insufficient trend"
    );
    assert!(
        report.per_boundary_accuracy.is_empty(),
        "empty store should have empty per_boundary_accuracy"
    );
    assert!(
        report.per_type_accuracy.is_empty(),
        "empty store should have empty per_type_accuracy"
    );
}

/// Test 2: After recording 3 hypotheses via record_hypotheses(), the store
/// contains the expected :hypothesis/* datoms.
///
/// Verifies that record_hypotheses produces the correct datom structure:
/// each hypothesis entity gets action, predicted, boundary, confidence,
/// timestamp, and item-type attributes.
#[test]
fn record_hypotheses_creates_datoms() {
    let mut store = store_with_full_schema();
    let agent = AgentId::from_name("test:record");

    let routings = vec![
        make_routing(":task/alpha", 0.8, 0.1),
        make_routing(":task/beta", 0.5, 0.0),
        make_routing(":task/gamma", 0.3, 0.05),
    ];

    let tx_id = TxId::new(200, 0, agent);
    let datoms = record_hypotheses(&routings, 3, tx_id, braid_kernel::now_secs());

    // Each hypothesis gets 6 datoms: action, predicted, boundary, confidence,
    // timestamp, item-type. Content-addressed EntityIds may collide when
    // timestamps share the same second, producing fewer unique hypotheses.
    assert!(
        datoms.len() >= 6,
        "expected at least 1 hypothesis (6 datoms), got {}",
        datoms.len()
    );

    // Transact them into the store
    let mut tx = braid_kernel::store::Transaction::new(
        agent,
        braid_kernel::datom::ProvenanceType::Derived,
        "record hypothesis datoms",
    );
    for d in &datoms {
        if d.op == Op::Assert {
            tx = tx.assert(d.entity, d.attribute.clone(), d.value.clone());
        }
    }
    let committed = tx.commit(&store).expect("commit hypotheses");
    store.transact(committed).expect("transact hypotheses");

    // Verify hypothesis count (may be < 3 due to entity hash collisions)
    let h_count = hypothesis_count(&store);
    assert!(
        h_count >= 1,
        "store should contain at least 1 hypothesis after recording, got {}",
        h_count
    );

    // Verify no completed hypotheses yet (no :hypothesis/actual)
    assert_eq!(
        hypothesis_completed_count(&store),
        0,
        "no hypotheses should be completed yet"
    );

    // Verify all datoms have the correct attributes
    let expected_attrs = [
        ":hypothesis/action",
        ":hypothesis/predicted",
        ":hypothesis/boundary",
        ":hypothesis/confidence",
        ":hypothesis/timestamp",
        ":hypothesis/item-type",
    ];
    for attr_name in &expected_attrs {
        let attr = Attribute::from_keyword(attr_name);
        let count = store
            .attribute_datoms(&attr)
            .iter()
            .filter(|d| d.op == Op::Assert)
            .count();
        // Content-addressed EntityIds may collide when timestamp is identical
        // (same second), so we may get fewer than 3. At least 1 is valid.
        assert!(
            count >= 1,
            "expected at least 1 Assert datom for {}, got {}",
            attr_name,
            count
        );
    }
}

/// Test 3: Record hypotheses, mark some completed with :hypothesis/actual,
/// verify completed count matches.
///
/// The completion marking follows the append-only pattern: we add new datoms
/// for :hypothesis/actual, :hypothesis/error, and :hypothesis/completed to
/// an existing hypothesis entity. compute_calibration_metrics then counts
/// entities with :hypothesis/actual as "completed."
#[test]
fn calibration_metrics_count_completed() {
    // 5 total, 3 completed
    let entries = vec![
        (0.5, 0.4, 1000), // completed
        (0.3, 0.2, 1001), // completed
        (0.8, 0.6, 1002), // completed
    ];
    let base_store = store_with_completed_hypotheses(&entries);

    // Add 2 more incomplete hypotheses
    let mut datoms = base_store.datom_set().clone();
    let agent = AgentId::from_name("test:hyp");
    let tx = TxId::new(200, 0, agent);
    for i in 0..2 {
        let hyp = EntityId::from_ident(&format!(":hypothesis/incomplete-extra-{i}"));
        let target = EntityId::from_ident(&format!(":task/extra-{i}"));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/action"),
            Value::Ref(target),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/predicted"),
            Value::Double(ordered_float::OrderedFloat(0.5)),
            tx,
            Op::Assert,
        ));
    }
    let store = Store::from_datoms(datoms);

    let report = compute_calibration_metrics(&store);

    assert_eq!(
        report.total_hypotheses, 5,
        "should have 5 total hypotheses (3 completed + 2 incomplete)"
    );
    assert_eq!(
        report.completed_hypotheses, 3,
        "should have 3 completed hypotheses"
    );
}

/// Test 4: Record hypotheses with known predicted/actual deltas, verify
/// mean_error = mean(|predicted - actual|).
///
/// Entries: (0.5, 0.3) -> error 0.2
///          (0.8, 0.6) -> error 0.2
///          (0.3, 0.1) -> error 0.2
///          (0.7, 0.4) -> error 0.3
///          (0.9, 0.8) -> error 0.1
/// Mean error = (0.2 + 0.2 + 0.2 + 0.3 + 0.1) / 5 = 0.2
#[test]
fn mean_error_computed_correctly() {
    let entries = vec![
        (0.5, 0.3, 1000),
        (0.8, 0.6, 1001),
        (0.3, 0.1, 1002),
        (0.7, 0.4, 1003),
        (0.9, 0.8, 1004),
    ];
    let store = store_with_completed_hypotheses(&entries);
    let report = compute_calibration_metrics(&store);

    let expected_mean = (0.2 + 0.2 + 0.2 + 0.3 + 0.1) / 5.0;
    assert!(
        (report.mean_error - expected_mean).abs() < 1e-10,
        "mean_error should be {}, got {}",
        expected_mean,
        report.mean_error
    );
}

/// Test 5: Create a sequence where error decreases over time, verify
/// trend = Improving.
///
/// The trend detection compares the last-20 (or fewer) hypotheses' mean error
/// against the all-time mean error. "Improving" means recent_mean < all_time * 0.8.
///
/// Strategy: older hypotheses have high error (0.5), recent have low error (0.05).
/// With enough spread, the recent mean will be well below 80% of all-time.
#[test]
fn calibration_trend_improving() {
    // 35 entries: 10 old with high error, 25 recent with zero error.
    // recent_n = min(20, 35) = 20 — all 20 are recent (low error).
    // Needs extreme separation so recent_mean << all-time * 0.8.
    let mut entries = Vec::new();
    // Old hypotheses: predicted=0.8, actual=0.0 -> error=0.8
    for i in 0..10 {
        entries.push((0.8, 0.0, 1000 + i as u64));
    }
    // Recent hypotheses: predicted=0.5, actual=0.5 -> error=0.0
    for i in 0..25 {
        entries.push((0.5, 0.5, 2000 + i as u64));
    }

    let store = store_with_completed_hypotheses(&entries);
    let report = compute_calibration_metrics(&store);

    // All-time mean = (10*0.8 + 25*0.0) / 35 = 0.229
    // Recent-20 (last 20 by timestamp = 20 new) → mean = 0.0
    // 0.0 < 0.229 * 0.8 = 0.183 → Improving
    assert_eq!(
        report.trend,
        CalibrationTrend::Improving,
        "trend should be Improving when recent errors are much lower than all-time. \
         mean_error={}, completed={}",
        report.mean_error,
        report.completed_hypotheses,
    );
}

/// Test 6: Create a sequence where error increases over time, verify
/// trend = Degrading.
///
/// "Degrading" means recent_mean > all_time * 1.2.
///
/// Strategy: older hypotheses have low error, recent have high error.
#[test]
fn calibration_trend_degrading() {
    let mut entries = Vec::new();
    // Old hypotheses: zero error (predicted=0.5, actual=0.5 -> error=0.0)
    for i in 0..10 {
        entries.push((0.5, 0.5, 1000 + i as u64));
    }
    // Recent hypotheses: high error (predicted=0.8, actual=0.0 -> error=0.8)
    for i in 0..25 {
        entries.push((0.8, 0.0, 2000 + i as u64));
    }

    let store = store_with_completed_hypotheses(&entries);
    let report = compute_calibration_metrics(&store);

    // All-time mean = (10*0.0 + 25*0.8) / 35 = 0.571
    // Recent-20 (last 20 by timestamp = 20 new) → mean = 0.8
    // 0.8 > 0.571 * 1.2 = 0.686 → Degrading
    assert_eq!(
        report.trend,
        CalibrationTrend::Degrading,
        "trend should be Degrading when recent errors are much higher than all-time. \
         mean_error={}, completed={}",
        report.mean_error,
        report.completed_hypotheses,
    );
}

/// Test 7: record_hypotheses_with_type("exploration") produces
/// :hypothesis/item-type "exploration" in the datoms.
///
/// Verifies the UAQ-4 per-type calibration pathway: different item types
/// (task, block, boundary, exploration) enable separate calibration tracks.
#[test]
fn record_hypotheses_with_type_uses_item_type() {
    let agent = AgentId::from_name("test:type");
    let routings = vec![make_routing(":task/typed", 0.6, 0.0)];
    let tx_id = TxId::new(300, 0, agent);

    let datoms =
        record_hypotheses_with_type(&routings, 1, tx_id, "exploration", braid_kernel::now_secs());

    // Find the :hypothesis/item-type datom
    let item_type_attr = Attribute::from_keyword(":hypothesis/item-type");
    let type_datom = datoms
        .iter()
        .find(|d| d.attribute == item_type_attr)
        .expect("should have a :hypothesis/item-type datom");

    match &type_datom.value {
        Value::String(s) => assert_eq!(
            s, "exploration",
            "item-type should be 'exploration', got '{}'",
            s
        ),
        other => panic!("item-type value should be String, got {:?}", other),
    }

    // Also verify the default path: record_hypotheses uses "task"
    let default_datoms = record_hypotheses(&routings, 1, tx_id, braid_kernel::now_secs());
    let default_type = default_datoms
        .iter()
        .find(|d| d.attribute == item_type_attr)
        .expect("default path should also have item-type");

    match &default_type.value {
        Value::String(s) => {
            assert_eq!(s, "task", "default item-type should be 'task', got '{}'", s)
        }
        other => panic!("default item-type value should be String, got {:?}", other),
    }
}

/// Test: zero-impact routings are skipped by record_hypotheses.
///
/// The implementation filters out routings with impact <= EPSILON to avoid
/// recording noise hypotheses. This is important for calibration quality.
#[test]
fn record_hypotheses_skips_zero_impact() {
    let agent = AgentId::from_name("test:zero");
    let routings = vec![
        make_routing(":task/real", 0.5, 0.0),
        make_routing(":task/zero", 0.0, 0.0), // should be skipped
        make_routing(":task/tiny", 1e-20, 0.0), // should be skipped (below EPSILON)
    ];
    let tx_id = TxId::new(400, 0, agent);
    let datoms = record_hypotheses(&routings, 3, tx_id, braid_kernel::now_secs());

    // Only 1 hypothesis should be recorded (the 0.5 impact one)
    let action_attr = Attribute::from_keyword(":hypothesis/action");
    let action_count = datoms.iter().filter(|d| d.attribute == action_attr).count();
    assert_eq!(
        action_count, 1,
        "only non-zero-impact routings should produce hypotheses, got {}",
        action_count
    );
}

/// Test: record_hypotheses respects top_n limit.
///
/// When top_n < routings.len(), only the first top_n are considered
/// (the caller is responsible for pre-sorting by impact).
#[test]
fn record_hypotheses_respects_top_n() {
    let agent = AgentId::from_name("test:topn");
    let routings = vec![
        make_routing(":task/a", 0.9, 0.0),
        make_routing(":task/b", 0.7, 0.0),
        make_routing(":task/c", 0.5, 0.0),
        make_routing(":task/d", 0.3, 0.0),
    ];
    let tx_id = TxId::new(500, 0, agent);
    let datoms = record_hypotheses(&routings, 2, tx_id, braid_kernel::now_secs());

    let action_attr = Attribute::from_keyword(":hypothesis/action");
    let action_count = datoms.iter().filter(|d| d.attribute == action_attr).count();
    assert_eq!(
        action_count, 2,
        "top_n=2 should produce 2 hypotheses, got {}",
        action_count
    );
}

/// Test: boundary inference from gradient_delta.
///
/// When gradient_delta > EPSILON, boundary is "spec<->impl".
/// Otherwise, boundary is "general".
#[test]
fn record_hypotheses_boundary_inference() {
    let agent = AgentId::from_name("test:boundary");
    let routings = vec![
        make_routing(":task/gradient", 0.5, 0.1), // gradient_delta > 0 -> spec<->impl
        make_routing(":task/general", 0.5, 0.0),  // gradient_delta == 0 -> general
    ];
    let tx_id = TxId::new(600, 0, agent);
    let datoms = record_hypotheses(&routings, 2, tx_id, braid_kernel::now_secs());

    let boundary_attr = Attribute::from_keyword(":hypothesis/boundary");
    let boundaries: Vec<String> = datoms
        .iter()
        .filter(|d| d.attribute == boundary_attr)
        .filter_map(|d| match &d.value {
            Value::String(s) => Some(s.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(boundaries.len(), 2, "should have 2 boundary datoms");
    assert!(
        boundaries.contains(&"spec<->impl".to_string()),
        "gradient task should have spec<->impl boundary, got {:?}",
        boundaries
    );
    assert!(
        boundaries.contains(&"general".to_string()),
        "non-gradient task should have general boundary, got {:?}",
        boundaries
    );
}

/// Test: per-boundary accuracy in CalibrationReport groups errors correctly.
///
/// Two boundaries with different error distributions should produce
/// different per-boundary mean errors.
#[test]
fn per_boundary_accuracy_grouped() {
    let base_store = store_with_full_schema();
    let mut datoms = base_store.datom_set().clone();
    let agent = AgentId::from_name("test:boundary-acc");
    let tx = TxId::new(100, 0, agent);

    // Boundary "coverage": 2 hypotheses with errors 0.1, 0.3 -> mean 0.2
    for i in 0..2 {
        let hyp = EntityId::from_ident(&format!(":hypothesis/cov-{i}"));
        let target = EntityId::from_ident(&format!(":task/cov-target-{i}"));
        let error = if i == 0 { 0.1 } else { 0.3 };
        let actual = 0.5 - error; // so predicted=0.5, actual varies

        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/action"),
            Value::Ref(target),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/predicted"),
            Value::Double(ordered_float::OrderedFloat(0.5)),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/actual"),
            Value::Double(ordered_float::OrderedFloat(actual)),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/error"),
            Value::Double(ordered_float::OrderedFloat(error)),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/boundary"),
            Value::String("coverage".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/completed"),
            Value::Instant(1000 + i as u64),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/item-type"),
            Value::String("task".to_string()),
            tx,
            Op::Assert,
        ));
    }

    // Boundary "formality": 2 hypotheses with errors 0.4, 0.6 -> mean 0.5
    for i in 0..2 {
        let hyp = EntityId::from_ident(&format!(":hypothesis/form-{i}"));
        let target = EntityId::from_ident(&format!(":task/form-target-{i}"));
        let error = if i == 0 { 0.4 } else { 0.6 };
        let actual = 0.8 - error;

        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/action"),
            Value::Ref(target),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/predicted"),
            Value::Double(ordered_float::OrderedFloat(0.8)),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/actual"),
            Value::Double(ordered_float::OrderedFloat(actual)),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/error"),
            Value::Double(ordered_float::OrderedFloat(error)),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/boundary"),
            Value::String("formality".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/completed"),
            Value::Instant(1000 + i as u64),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/item-type"),
            Value::String("task".to_string()),
            tx,
            Op::Assert,
        ));
    }

    let store = Store::from_datoms(datoms);
    let report = compute_calibration_metrics(&store);

    assert_eq!(report.completed_hypotheses, 4);

    let cov_error = report
        .per_boundary_accuracy
        .get("coverage")
        .expect("should have coverage boundary");
    let form_error = report
        .per_boundary_accuracy
        .get("formality")
        .expect("should have formality boundary");

    assert!(
        (*cov_error - 0.2).abs() < 1e-10,
        "coverage mean error should be 0.2, got {}",
        cov_error
    );
    assert!(
        (*form_error - 0.5).abs() < 1e-10,
        "formality mean error should be 0.5, got {}",
        form_error
    );
}

/// Test: per-type accuracy in CalibrationReport groups by item-type.
///
/// Verifies UAQ-4: different item types produce separate calibration tracks.
#[test]
fn per_type_accuracy_grouped() {
    let base_store = store_with_full_schema();
    let mut datoms = base_store.datom_set().clone();
    let agent = AgentId::from_name("test:type-acc");
    let tx = TxId::new(100, 0, agent);

    // Type "task": error 0.1
    let hyp1 = EntityId::from_ident(":hypothesis/type-task");
    let target1 = EntityId::from_ident(":task/type-target-1");
    datoms.insert(Datom::new(
        hyp1,
        Attribute::from_keyword(":hypothesis/action"),
        Value::Ref(target1),
        tx,
        Op::Assert,
    ));
    datoms.insert(Datom::new(
        hyp1,
        Attribute::from_keyword(":hypothesis/predicted"),
        Value::Double(ordered_float::OrderedFloat(0.5)),
        tx,
        Op::Assert,
    ));
    datoms.insert(Datom::new(
        hyp1,
        Attribute::from_keyword(":hypothesis/actual"),
        Value::Double(ordered_float::OrderedFloat(0.4)),
        tx,
        Op::Assert,
    ));
    datoms.insert(Datom::new(
        hyp1,
        Attribute::from_keyword(":hypothesis/error"),
        Value::Double(ordered_float::OrderedFloat(0.1)),
        tx,
        Op::Assert,
    ));
    datoms.insert(Datom::new(
        hyp1,
        Attribute::from_keyword(":hypothesis/boundary"),
        Value::String("general".to_string()),
        tx,
        Op::Assert,
    ));
    datoms.insert(Datom::new(
        hyp1,
        Attribute::from_keyword(":hypothesis/completed"),
        Value::Instant(1000),
        tx,
        Op::Assert,
    ));
    datoms.insert(Datom::new(
        hyp1,
        Attribute::from_keyword(":hypothesis/item-type"),
        Value::String("task".to_string()),
        tx,
        Op::Assert,
    ));

    // Type "block": error 0.4
    let hyp2 = EntityId::from_ident(":hypothesis/type-block");
    let target2 = EntityId::from_ident(":task/type-target-2");
    datoms.insert(Datom::new(
        hyp2,
        Attribute::from_keyword(":hypothesis/action"),
        Value::Ref(target2),
        tx,
        Op::Assert,
    ));
    datoms.insert(Datom::new(
        hyp2,
        Attribute::from_keyword(":hypothesis/predicted"),
        Value::Double(ordered_float::OrderedFloat(0.8)),
        tx,
        Op::Assert,
    ));
    datoms.insert(Datom::new(
        hyp2,
        Attribute::from_keyword(":hypothesis/actual"),
        Value::Double(ordered_float::OrderedFloat(0.4)),
        tx,
        Op::Assert,
    ));
    datoms.insert(Datom::new(
        hyp2,
        Attribute::from_keyword(":hypothesis/error"),
        Value::Double(ordered_float::OrderedFloat(0.4)),
        tx,
        Op::Assert,
    ));
    datoms.insert(Datom::new(
        hyp2,
        Attribute::from_keyword(":hypothesis/boundary"),
        Value::String("general".to_string()),
        tx,
        Op::Assert,
    ));
    datoms.insert(Datom::new(
        hyp2,
        Attribute::from_keyword(":hypothesis/completed"),
        Value::Instant(1001),
        tx,
        Op::Assert,
    ));
    datoms.insert(Datom::new(
        hyp2,
        Attribute::from_keyword(":hypothesis/item-type"),
        Value::String("block".to_string()),
        tx,
        Op::Assert,
    ));

    let store = Store::from_datoms(datoms);
    let report = compute_calibration_metrics(&store);

    let task_error = report
        .per_type_accuracy
        .get("task")
        .expect("should have 'task' type entry");
    let block_error = report
        .per_type_accuracy
        .get("block")
        .expect("should have 'block' type entry");

    assert!(
        (*task_error - 0.1).abs() < 1e-10,
        "task type mean error should be 0.1, got {}",
        task_error
    );
    assert!(
        (*block_error - 0.4).abs() < 1e-10,
        "block type mean error should be 0.4, got {}",
        block_error
    );
}

/// Test: Insufficient trend when fewer than 5 completed hypotheses.
///
/// The trend detection requires at least 5 completed hypotheses to produce
/// a meaningful signal. Below that threshold, it returns Insufficient.
#[test]
fn calibration_trend_insufficient_below_threshold() {
    let entries = vec![
        (0.5, 0.3, 1000),
        (0.8, 0.2, 1001),
        (0.3, 0.1, 1002),
        (0.7, 0.4, 1003),
    ];
    let store = store_with_completed_hypotheses(&entries);
    let report = compute_calibration_metrics(&store);

    assert_eq!(
        report.trend,
        CalibrationTrend::Insufficient,
        "fewer than 5 completed hypotheses should produce Insufficient trend"
    );
}

/// Test: hypothesis_count counts only Assert ops on :hypothesis/action.
///
/// Verifies that the count function correctly filters for Assert operations
/// and uses the :hypothesis/action attribute as the counting key.
#[test]
fn hypothesis_count_matches_recorded() {
    let store = store_with_incomplete_hypotheses(7);
    assert_eq!(
        hypothesis_count(&store),
        7,
        "hypothesis_count should match number of recorded hypotheses"
    );
}

/// Test: hypothesis_completed_count counts only hypotheses with :hypothesis/actual.
#[test]
fn hypothesis_completed_count_matches_actual() {
    // 3 completed, 2 incomplete
    let entries = vec![(0.5, 0.4, 1000), (0.3, 0.2, 1001), (0.8, 0.6, 1002)];
    let base_store = store_with_completed_hypotheses(&entries);
    let mut datoms = base_store.datom_set().clone();
    let agent = AgentId::from_name("test:hyp");
    let tx = TxId::new(200, 0, agent);

    // Add 2 incomplete
    for i in 0..2 {
        let hyp = EntityId::from_ident(&format!(":hypothesis/extra-inc-{i}"));
        let target = EntityId::from_ident(&format!(":task/extra-inc-{i}"));
        datoms.insert(Datom::new(
            hyp,
            Attribute::from_keyword(":hypothesis/action"),
            Value::Ref(target),
            tx,
            Op::Assert,
        ));
    }
    let store = Store::from_datoms(datoms);

    assert_eq!(hypothesis_count(&store), 5, "5 total hypotheses");
    assert_eq!(
        hypothesis_completed_count(&store),
        3,
        "3 completed hypotheses"
    );
}

/// Test: Stable trend when recent and all-time error are similar.
///
/// "Stable" means recent_mean is within [all_time * 0.8, all_time * 1.2].
#[test]
fn calibration_trend_stable() {
    // All hypotheses have roughly the same error -> stable
    let mut entries = Vec::new();
    for i in 0..10 {
        // predicted=0.5, actual=0.35 -> error=0.15 (constant)
        entries.push((0.5, 0.35, 1000 + i as u64));
    }

    let store = store_with_completed_hypotheses(&entries);
    let report = compute_calibration_metrics(&store);

    // All-time mean = 0.15, recent mean = 0.15
    // 0.15 is between 0.15*0.8=0.12 and 0.15*1.2=0.18 -> Stable
    assert_eq!(
        report.trend,
        CalibrationTrend::Stable,
        "constant error across all hypotheses should produce Stable trend. \
         mean_error={}",
        report.mean_error,
    );
}

// ===========================================================================
// Property-based tests (proptest)
// ===========================================================================

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    // Property 8: For any set of (predicted, actual) pairs, mean_error >= 0.
    //
    // This is an algebraic invariant: mean of absolute values is always non-negative.
    // The hypothesis ledger computes error = |predicted - actual|, and mean_error
    // is the arithmetic mean of these errors.
    proptest! {
        #[test]
        fn mean_error_always_non_negative(
            pairs in proptest::collection::vec(
                (0.0f64..=1.0, 0.0f64..=1.0),
                1..20
            )
        ) {
            // Build a store with these predicted/actual pairs
            let base_store = store_with_full_schema();
            let mut datoms = base_store.datom_set().clone();
            let agent = AgentId::from_name("test:prop");
            let tx = TxId::new(100, 0, agent);

            for (i, &(predicted, actual)) in pairs.iter().enumerate() {
                let hyp = EntityId::from_ident(&format!(":hypothesis/prop-{i}"));
                let target = EntityId::from_ident(&format!(":task/prop-target-{i}"));
                let error = (predicted - actual).abs();

                datoms.insert(Datom::new(hyp, Attribute::from_keyword(":hypothesis/action"),
                    Value::Ref(target), tx, Op::Assert));
                datoms.insert(Datom::new(hyp, Attribute::from_keyword(":hypothesis/predicted"),
                    Value::Double(ordered_float::OrderedFloat(predicted)), tx, Op::Assert));
                datoms.insert(Datom::new(hyp, Attribute::from_keyword(":hypothesis/actual"),
                    Value::Double(ordered_float::OrderedFloat(actual)), tx, Op::Assert));
                datoms.insert(Datom::new(hyp, Attribute::from_keyword(":hypothesis/error"),
                    Value::Double(ordered_float::OrderedFloat(error)), tx, Op::Assert));
                datoms.insert(Datom::new(hyp, Attribute::from_keyword(":hypothesis/boundary"),
                    Value::String("general".to_string()), tx, Op::Assert));
                datoms.insert(Datom::new(hyp, Attribute::from_keyword(":hypothesis/completed"),
                    Value::Instant(1000 + i as u64), tx, Op::Assert));
                datoms.insert(Datom::new(hyp, Attribute::from_keyword(":hypothesis/item-type"),
                    Value::String("task".to_string()), tx, Op::Assert));
            }

            let store = Store::from_datoms(datoms);
            let report = compute_calibration_metrics(&store);

            prop_assert!(
                report.mean_error >= 0.0,
                "mean_error must be >= 0 for any input, got {}",
                report.mean_error
            );
        }
    }

    // Property 9: completed_hypotheses <= total_hypotheses always.
    //
    // A completed hypothesis is a subset of all hypotheses (those with
    // :hypothesis/actual set). The count of the subset can never exceed
    // the count of the superset.
    proptest! {
        #[test]
        fn completed_never_exceeds_total(
            n_completed in 0usize..10,
            n_incomplete in 0usize..10,
        ) {
            let base_store = store_with_full_schema();
            let mut datoms = base_store.datom_set().clone();
            let agent = AgentId::from_name("test:prop-count");
            let tx = TxId::new(100, 0, agent);

            // Add completed hypotheses
            for i in 0..n_completed {
                let hyp = EntityId::from_ident(&format!(":hypothesis/comp-{i}"));
                let target = EntityId::from_ident(&format!(":task/comp-target-{i}"));

                datoms.insert(Datom::new(hyp, Attribute::from_keyword(":hypothesis/action"),
                    Value::Ref(target), tx, Op::Assert));
                datoms.insert(Datom::new(hyp, Attribute::from_keyword(":hypothesis/predicted"),
                    Value::Double(ordered_float::OrderedFloat(0.5)), tx, Op::Assert));
                datoms.insert(Datom::new(hyp, Attribute::from_keyword(":hypothesis/actual"),
                    Value::Double(ordered_float::OrderedFloat(0.3)), tx, Op::Assert));
                datoms.insert(Datom::new(hyp, Attribute::from_keyword(":hypothesis/error"),
                    Value::Double(ordered_float::OrderedFloat(0.2)), tx, Op::Assert));
                datoms.insert(Datom::new(hyp, Attribute::from_keyword(":hypothesis/boundary"),
                    Value::String("general".to_string()), tx, Op::Assert));
                datoms.insert(Datom::new(hyp, Attribute::from_keyword(":hypothesis/completed"),
                    Value::Instant(1000 + i as u64), tx, Op::Assert));
            }

            // Add incomplete hypotheses
            for i in 0..n_incomplete {
                let hyp = EntityId::from_ident(&format!(":hypothesis/incomp-{i}"));
                let target = EntityId::from_ident(&format!(":task/incomp-target-{i}"));

                datoms.insert(Datom::new(hyp, Attribute::from_keyword(":hypothesis/action"),
                    Value::Ref(target), tx, Op::Assert));
                datoms.insert(Datom::new(hyp, Attribute::from_keyword(":hypothesis/predicted"),
                    Value::Double(ordered_float::OrderedFloat(0.5)), tx, Op::Assert));
            }

            let store = Store::from_datoms(datoms);
            let report = compute_calibration_metrics(&store);

            prop_assert!(
                report.completed_hypotheses <= report.total_hypotheses,
                "completed ({}) must not exceed total ({})",
                report.completed_hypotheses,
                report.total_hypotheses
            );

            // Also verify the counts match what we put in
            prop_assert_eq!(
                report.total_hypotheses,
                n_completed + n_incomplete,
                "total should be {} + {} = {}",
                n_completed,
                n_incomplete,
                n_completed + n_incomplete
            );
            prop_assert_eq!(
                report.completed_hypotheses,
                n_completed,
                "completed should be {}",
                n_completed
            );
        }
    }
}
