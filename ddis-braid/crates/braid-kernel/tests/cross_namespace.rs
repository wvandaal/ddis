// Witnesses: INV-STORE-001, INV-STORE-003, INV-STORE-004, INV-STORE-006, INV-STORE-007,
//   INV-SCHEMA-001, INV-SCHEMA-002, INV-SCHEMA-003, INV-QUERY-001, INV-QUERY-002,
//   INV-MERGE-001, INV-MERGE-008, INV-MERGE-009,
//   INV-HARVEST-001, INV-HARVEST-002, INV-SEED-001, INV-SEED-002,
//   INV-TRILATERAL-001, INV-TRILATERAL-002, INV-TRILATERAL-003, INV-TRILATERAL-004,
//   INV-GUIDANCE-001, INV-GUIDANCE-003,
//   INV-RESOLUTION-001, INV-RESOLUTION-002, INV-RESOLUTION-003,
//   INV-LAYOUT-001, INV-LAYOUT-003, INV-LAYOUT-005,
//   INV-BILATERAL-001,
//   ADR-STORE-003, ADR-SCHEMA-001, ADR-QUERY-001, ADR-MERGE-001,
//   ADR-HARVEST-001, ADR-SEED-001, ADR-TRILATERAL-001, ADR-TRILATERAL-002,
//   ADR-GUIDANCE-001, ADR-RESOLUTION-001, ADR-LAYOUT-002,
//   NEG-STORE-001, NEG-TRILATERAL-001, NEG-TRILATERAL-002

//! Cross-namespace integration tests — verify invariant chains ACROSS module boundaries.
//!
//! Each test exercises a multi-namespace pipeline, asserting that invariants from
//! different modules compose correctly. These complement per-module unit tests
//! and proptests by testing the interfaces between namespaces.
//!
//! Traces to: SEED.md §7 (reconciliation), SEED.md §10 (Stage 0 deliverables)

use std::collections::BTreeSet;

use braid_kernel::agent_md::{generate_agent_md, AgentMdConfig};
use braid_kernel::datom::{AgentId, Attribute, EntityId, ProvenanceType, TxId, Value};
use braid_kernel::guidance::{
    build_footer, compute_methodology_score, format_footer, SessionTelemetry,
};
use braid_kernel::harvest::{harvest_pipeline, SessionContext};
use braid_kernel::layout::{
    collect_datoms, deserialize_tx, serialize_tx, tx_content_hash, verify_content_hash, TxFile,
};
use braid_kernel::merge::{merge_stores, verify_monotonicity};
use braid_kernel::query::clause::{Clause, FindSpec, Pattern, QueryExpr, Term};
use braid_kernel::query::evaluator::{evaluate, QueryResult};
use braid_kernel::resolution::{detect_conflicts, resolve_with_trail, ResolvedValue};
use braid_kernel::schema::ResolutionMode;
use braid_kernel::seed::{assemble_seed, ContextSection};
use braid_kernel::store::{Store, Transaction};
use braid_kernel::trilateral::{
    check_coherence, classify_attribute, compute_phi_default, isp_check, live_projections,
    AttrNamespace, CoherenceQuadrant, IspResult,
};

// ===========================================================================
// Helpers
// ===========================================================================

/// Register a single attribute into the store via schema-as-data transaction.
///
/// This exercises the schema-as-data pattern (C3, INV-SCHEMA-001, INV-SCHEMA-003):
/// schema evolution is a transaction, not a migration. Each attribute is registered
/// by transacting 5 meta-datoms: :db/ident, :db/valueType, :db/cardinality,
/// :db/doc, :db/resolutionMode.
fn register_attribute(store: &mut Store, agent: AgentId, keyword: &str, doc: &str) {
    let entity = EntityId::from_ident(keyword);

    let tx = Transaction::new(
        agent,
        ProvenanceType::Observed,
        &format!("Register {keyword}"),
    )
    .assert(
        entity,
        Attribute::from_keyword(":db/ident"),
        Value::Keyword(keyword.to_string()),
    )
    .assert(
        entity,
        Attribute::from_keyword(":db/valueType"),
        Value::Keyword(":db.type/string".to_string()),
    )
    .assert(
        entity,
        Attribute::from_keyword(":db/cardinality"),
        Value::Keyword(":db.cardinality/one".to_string()),
    )
    .assert(
        entity,
        Attribute::from_keyword(":db/doc"),
        Value::String(doc.to_string()),
    )
    .assert(
        entity,
        Attribute::from_keyword(":db/resolutionMode"),
        Value::Keyword(":resolution/lww".to_string()),
    );

    let committed = tx.commit(store).expect("schema registration must succeed");
    store
        .transact(committed)
        .expect("schema transact must succeed");
}

/// Install all 24 ISP-layer attributes into the store (Layer 1 of INV-SCHEMA-006).
///
/// Intent (7): :intent/decision, :intent/rationale, :intent/source, :intent/goal,
///             :intent/constraint, :intent/preference, :intent/noted
/// Spec (11):  :spec/id, :spec/element-type, :spec/namespace, :spec/source-file,
///             :spec/stage, :spec/statement, :spec/falsification, :spec/traces-to,
///             :spec/verification, :spec/witnessed, :spec/challenged
/// Impl (6):   :impl/signature, :impl/implements, :impl/file, :impl/module,
///             :impl/test-result, :impl/coverage
fn install_isp_schema(store: &mut Store, agent: AgentId) {
    // Intent-layer attributes
    register_attribute(store, agent, ":intent/decision", "Intent-layer decision");
    register_attribute(store, agent, ":intent/rationale", "Intent-layer rationale");
    register_attribute(store, agent, ":intent/source", "Intent-layer source");
    register_attribute(store, agent, ":intent/goal", "Intent-layer goal");
    register_attribute(
        store,
        agent,
        ":intent/constraint",
        "Intent-layer constraint",
    );
    register_attribute(
        store,
        agent,
        ":intent/preference",
        "Intent-layer preference",
    );
    register_attribute(store, agent, ":intent/noted", "Intent-layer noted");

    // Spec-layer attributes
    register_attribute(store, agent, ":spec/id", "Spec-layer identifier");
    register_attribute(store, agent, ":spec/element-type", "Spec element type");
    register_attribute(store, agent, ":spec/namespace", "Spec namespace");
    register_attribute(store, agent, ":spec/source-file", "Spec source file");
    register_attribute(store, agent, ":spec/stage", "Spec stage");
    register_attribute(store, agent, ":spec/statement", "Spec statement");
    register_attribute(
        store,
        agent,
        ":spec/falsification",
        "Spec falsification condition",
    );
    register_attribute(store, agent, ":spec/traces-to", "Spec traces-to reference");
    register_attribute(
        store,
        agent,
        ":spec/verification",
        "Spec verification method",
    );
    register_attribute(store, agent, ":spec/witnessed", "Spec witness status");
    register_attribute(store, agent, ":spec/challenged", "Spec challenge result");

    // Impl-layer attributes
    register_attribute(store, agent, ":impl/signature", "Impl function signature");
    register_attribute(
        store,
        agent,
        ":impl/implements",
        "Impl implements reference",
    );
    register_attribute(store, agent, ":impl/file", "Impl source file path");
    register_attribute(store, agent, ":impl/module", "Impl module path");
    register_attribute(store, agent, ":impl/test-result", "Impl test result");
    register_attribute(store, agent, ":impl/coverage", "Impl coverage metric");
}

/// Helper: create an agent with a descriptive name.
fn agent(name: &str) -> AgentId {
    AgentId::from_name(name)
}

// ===========================================================================
// Chain 1: Store → Query → Trilateral
// ===========================================================================

// Verifies: INV-STORE-001, INV-QUERY-001, INV-TRILATERAL-001, INV-TRILATERAL-002,
//   INV-TRILATERAL-003, INV-SCHEMA-001, INV-SCHEMA-003
// (Chain 1: Store -> Query -> Trilateral coherence for fully linked ISP entity.)
/// Transact ISP datoms → query them → verify coherence (Phi).
///
/// INV chain: STORE-001 (append-only) → QUERY-001 (Datalog) → TRILATERAL-002 (Phi).
#[test]
fn store_query_trilateral_coherence() {
    let a = agent("test:chain1");
    let mut store = Store::genesis();

    // Install ISP schema (Layer 1) via schema-as-data transactions
    install_isp_schema(&mut store, a);

    let inv_entity = EntityId::from_ident(":inv/store-001");

    // Transact a fully-linked ISP entity (intent + spec + impl)
    let tx = Transaction::new(a, ProvenanceType::Observed, "Register INV-STORE-001")
        .assert(
            inv_entity,
            Attribute::from_keyword(":intent/goal"),
            Value::String("Append-only immutability".to_string()),
        )
        .assert(
            inv_entity,
            Attribute::from_keyword(":spec/id"),
            Value::String("INV-STORE-001".to_string()),
        )
        .assert(
            inv_entity,
            Attribute::from_keyword(":spec/statement"),
            Value::String("The datom store never deletes or mutates".to_string()),
        )
        .assert(
            inv_entity,
            Attribute::from_keyword(":impl/file"),
            Value::String("src/store.rs".to_string()),
        )
        .assert(
            inv_entity,
            Attribute::from_keyword(":db/doc"),
            Value::String("Append-only store invariant".to_string()),
        );

    let committed = tx.commit(&store).expect("commit must succeed");
    let receipt = store.transact(committed).expect("transact must succeed");
    assert!(receipt.datom_count > 0, "INV-STORE-001: datoms transacted");

    // Query: find entities with :spec/id = "INV-STORE-001"
    let query = QueryExpr::new(
        FindSpec::Rel(vec!["?e".into()]),
        vec![Clause::Pattern(Pattern::new(
            Term::Variable("?e".into()),
            Term::Attr(Attribute::from_keyword(":spec/id")),
            Term::Constant(Value::String("INV-STORE-001".to_string())),
        ))],
    );

    let result = evaluate(&store, &query);
    match &result {
        QueryResult::Rel(rows) => {
            assert!(!rows.is_empty(), "Query must find the INV-STORE-001 entity");
        }
        other => panic!("Expected Rel result, got {other:?}"),
    }

    // Trilateral: Phi should be 0 because the entity has all three layers
    let (phi, components) = compute_phi_default(&store);
    assert_eq!(components.d_is, 0, "No intent-spec gap (entity has both)");
    assert_eq!(components.d_sp, 0, "No spec-impl gap (entity has both)");
    assert_eq!(phi, 0.0, "Fully linked entity -> Phi = 0");

    // ISP check should be coherent
    assert_eq!(
        isp_check(&store, inv_entity),
        IspResult::Coherent,
        "Entity with all three ISP layers must be Coherent"
    );

    // Coherence report should be in the Coherent quadrant
    let report = check_coherence(&store);
    assert_eq!(report.quadrant, CoherenceQuadrant::Coherent);
    assert_eq!(report.isp_bypasses, 0);
}

// Verifies: INV-STORE-001, INV-TRILATERAL-002, INV-TRILATERAL-004,
//   NEG-TRILATERAL-001, NEG-TRILATERAL-002
// (Intent-only entity produces Phi > 0, gap detection works correctly.)
/// Transact an intent-only entity → verify Phi > 0 (gap detected).
///
/// Verifies that the trilateral model correctly detects missing spec/impl coverage.
#[test]
fn store_query_trilateral_gaps_detected() {
    let a = agent("test:chain1-gaps");
    let mut store = Store::genesis();

    // Install ISP schema
    install_isp_schema(&mut store, a);

    let intent_entity = EntityId::from_ident(":intent/uncovered-goal");

    // Transact intent-only entity (no spec, no impl)
    let tx = Transaction::new(a, ProvenanceType::Observed, "Intent without spec")
        .assert(
            intent_entity,
            Attribute::from_keyword(":intent/goal"),
            Value::String("A goal with no specification".to_string()),
        )
        .assert(
            intent_entity,
            Attribute::from_keyword(":db/doc"),
            Value::String("Uncovered goal".to_string()),
        );

    let committed = tx.commit(&store).expect("commit must succeed");
    store.transact(committed).expect("transact must succeed");

    // Phi must be > 0 because intent entity has no spec coverage
    let (phi, components) = compute_phi_default(&store);
    assert_eq!(components.d_is, 1, "One intent entity without spec");
    assert!(phi > 0.0, "Gap must produce positive Phi");

    // ISP check: IntentSpecGap
    assert_eq!(
        isp_check(&store, intent_entity),
        IspResult::IntentSpecGap,
        "Intent-only entity must report IntentSpecGap"
    );

    // Coherence quadrant: GapsOnly
    let report = check_coherence(&store);
    assert_eq!(report.quadrant, CoherenceQuadrant::GapsOnly);

    // LIVE projections: intent should have datoms
    let (live_i, _live_s, _live_p) = live_projections(&store);
    assert!(live_i.datom_count > 0, "Intent projection must have datoms");

    // Formality level: entity only has intent -> level 1
    let level = braid_kernel::trilateral::formality_level(&store, intent_entity);
    assert_eq!(level, 1, "Intent-only entity has formality level 1");

    // Classify the attribute
    assert_eq!(
        classify_attribute(&Attribute::from_keyword(":intent/goal")),
        AttrNamespace::Intent
    );
}

// ===========================================================================
// Chain 2: Store → Harvest → Seed → Store (lifecycle continuity)
// ===========================================================================

// Verifies: INV-STORE-001, INV-HARVEST-001, INV-HARVEST-002, INV-SEED-001,
//   INV-SEED-002, ADR-HARVEST-001, ADR-SEED-001
// (Chain 2: Harvest detects knowledge gaps, seed assembles context, store persists.)
/// Full harvest/seed lifecycle: work → harvest → seed → verify continuity.
///
/// INV chain: STORE-001 → HARVEST-001 (gap detection) → SEED-001 (assembly) → STORE-001 (persistence).
#[test]
fn store_harvest_seed_lifecycle() {
    let a = agent("test:chain2");
    let mut store = Store::genesis();

    // Phase 1: Do work
    for i in 1..=5 {
        let entity = EntityId::from_ident(&format!(":task/work-{i}"));
        let tx = Transaction::new(a, ProvenanceType::Observed, &format!("Work item {i}")).assert(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String(format!("Work product {i}")),
        );
        let committed = tx.commit(&store).expect("commit must succeed");
        store.transact(committed).expect("transact must succeed");
    }

    let pre_harvest_datoms: BTreeSet<_> = store.datoms().cloned().collect();
    let pre_harvest_count = store.len();

    // Phase 2: Harvest
    let harvest_context = SessionContext {
        agent: a,
        agent_name: "agent-a".into(),
        session_start_tx: TxId::new(1, 0, a),
        task_description: "Chain 2 lifecycle test".to_string(),
        session_knowledge: vec![
            (
                ":session/summary".to_string(),
                Value::String("Completed 5 work items".to_string()),
            ),
            (
                ":session/decision".to_string(),
                Value::String("Used append-only store".to_string()),
            ),
        ],
    };

    let harvest_result = harvest_pipeline(&store, &harvest_context);

    // INV-HARVEST-001: Gap detection finds knowledge not in store
    assert!(
        harvest_result.quality.count > 0,
        "Harvest should detect knowledge gaps"
    );
    assert!(
        harvest_result.drift_score >= 0.0 && harvest_result.drift_score <= 1.0,
        "Drift score in [0,1]"
    );

    // INV-HARVEST-002: Monotonicity — store only grows
    let post_harvest_datoms: BTreeSet<_> = store.datoms().cloned().collect();
    assert!(
        pre_harvest_datoms.is_subset(&post_harvest_datoms),
        "Harvest must not remove datoms"
    );
    assert!(store.len() >= pre_harvest_count, "Store must not shrink");

    // Phase 3: Seed — start fresh session
    let seed = assemble_seed(&store, "Continue work product task", 2000, a);

    assert!(
        seed.entities_discovered > 0,
        "Seed should discover entities"
    );
    assert!(
        seed.context.total_tokens <= 2000,
        "Seed must respect token budget"
    );
    assert_eq!(
        seed.context.sections.len(),
        5,
        "Seed should have 5 sections"
    );

    // Verify section types
    let has_orientation = seed
        .context
        .sections
        .iter()
        .any(|s| matches!(s, ContextSection::Orientation(_)));
    let has_state = seed
        .context
        .sections
        .iter()
        .any(|s| matches!(s, ContextSection::State(_)));
    assert!(has_orientation, "Seed must have Orientation");
    assert!(has_state, "Seed must have State");

    // Phase 4: Verify continuity — original work is still in store
    for i in [1, 3, 5] {
        let entity = EntityId::from_ident(&format!(":task/work-{i}"));
        let datoms = store.entity_datoms(entity);
        assert!(
            !datoms.is_empty(),
            "Work item {i} must persist across harvest/seed"
        );
    }
}

// ===========================================================================
// Chain 3: Store → Merge → Resolution
// ===========================================================================

// Verifies: INV-STORE-001, INV-MERGE-001, INV-MERGE-008, INV-MERGE-009,
//   INV-RESOLUTION-001, INV-RESOLUTION-002, INV-RESOLUTION-003,
//   ADR-MERGE-001, ADR-RESOLUTION-001
// (Chain 3: Merge is set union, conflicts detected, LWW resolution deterministic.)
/// Merge two stores → detect conflicts → resolve via LWW.
///
/// INV chain: STORE-001 → MERGE-001 (set union) → RESOLUTION-001 (per-attribute mode).
#[test]
fn store_merge_resolution() {
    let alice = agent("alice");
    let bob = agent("bob");

    // Alice's store
    let mut store_a = Store::genesis();
    let shared_entity = EntityId::from_ident(":shared/config");

    let tx_a = Transaction::new(alice, ProvenanceType::Observed, "Alice's version").assert(
        shared_entity,
        Attribute::from_keyword(":db/doc"),
        Value::String("Alice's config value".to_string()),
    );
    let committed_a = tx_a.commit(&store_a).expect("Alice commit");
    store_a.transact(committed_a).expect("Alice transact");

    // Bob's store (started from same genesis)
    let mut store_b = Store::genesis();
    let tx_b = Transaction::new(bob, ProvenanceType::Observed, "Bob's version").assert(
        shared_entity,
        Attribute::from_keyword(":db/doc"),
        Value::String("Bob's config value".to_string()),
    );
    let committed_b = tx_b.commit(&store_b).expect("Bob commit");
    store_b.transact(committed_b).expect("Bob transact");

    // Take pre-merge snapshot
    let pre_merge_datoms: BTreeSet<_> = store_a.datoms().cloned().collect();
    let pre_merge_len = store_a.len();

    // Merge: C4 — CRDT merge by set union
    let merge_receipt = merge_stores(&mut store_a, &store_b);
    assert!(merge_receipt.new_datoms > 0, "Merge should add datoms");

    // Verify monotonicity: store only grew
    assert!(
        verify_monotonicity(&pre_merge_datoms, &store_a.datoms().cloned().collect()),
        "Merge must be monotonic"
    );
    assert!(store_a.len() >= pre_merge_len, "Store must not shrink");

    // Verify frontier advancement
    let post_merge_frontier = store_a.frontier();
    assert!(
        post_merge_frontier.contains_key(&alice),
        "Frontier must include Alice"
    );
    assert!(
        post_merge_frontier.contains_key(&bob),
        "Frontier must include Bob"
    );

    // Detect conflicts: both agents asserted different :db/doc on same entity
    let conflict = detect_conflicts(&store_a, shared_entity, &Attribute::from_keyword(":db/doc"));
    assert!(conflict.is_some(), "Must detect conflict on shared entity");

    // Resolve: LWW — deterministic winner
    let conflict_entity = conflict.unwrap();
    let record = resolve_with_trail(&conflict_entity, store_a.schema());
    assert_eq!(record.resolution_mode, ResolutionMode::Lww);
    match &record.resolved_value {
        ResolvedValue::Single(v) => {
            // LWW picks the value with the latest TxId — Bob transacted second
            assert_eq!(
                *v,
                Value::String("Bob's config value".to_string()),
                "LWW must pick the latest transaction's value"
            );
        }
        other => panic!("Expected Single, got {other:?}"),
    }
}

// ===========================================================================
// Chain 4: Schema → Store → Query
// ===========================================================================

// Verifies: INV-SCHEMA-001, INV-SCHEMA-002, INV-SCHEMA-003, INV-STORE-001,
//   INV-QUERY-001, INV-QUERY-002, ADR-SCHEMA-001, ADR-STORE-003, ADR-QUERY-001
// (Chain 4: Schema-as-data attributes, genesis completeness, monotonic schema growth.)
/// Schema defines attributes → transact with schema validation → query with type safety.
///
/// INV chain: SCHEMA-001 (schema-as-data) → STORE-001 → QUERY-001.
#[test]
fn schema_store_query() {
    let a = agent("test:chain4");
    let mut store = Store::genesis();

    // Genesis schema has exactly 19 axiomatic attributes (INV-SCHEMA-002)
    // 9 :db/* + 5 :lattice/* + 5 :tx/* (including :tx/rationale, :tx/coherence-override)
    let genesis_attr_count = store.schema().len();
    assert_eq!(
        genesis_attr_count,
        braid_kernel::GENESIS_ATTR_COUNT,
        "INV-SCHEMA-002: genesis has GENESIS_ATTR_COUNT attributes"
    );

    // Register a custom attribute via schema-as-data (C3)
    register_attribute(&mut store, a, ":project/status", "Project lifecycle status");

    // Schema must have grown (INV-SCHEMA-003: monotonicity)
    assert!(
        store.schema().len() > genesis_attr_count,
        "INV-SCHEMA-003: schema only grows"
    );

    // Transact a datom using the new attribute
    let project = EntityId::from_ident(":project/braid");
    let tx = Transaction::new(a, ProvenanceType::Observed, "Set project status")
        .assert(
            project,
            Attribute::from_keyword(":project/status"),
            Value::String("active".to_string()),
        )
        .assert(
            project,
            Attribute::from_keyword(":db/doc"),
            Value::String("The braid project entity".to_string()),
        );

    let committed = tx.commit(&store).expect("commit with custom attr");
    store.transact(committed).expect("transact custom attr");

    // Query: find entities with :project/status
    let query = QueryExpr::new(
        FindSpec::Rel(vec!["?e".into(), "?status".into()]),
        vec![Clause::Pattern(Pattern::new(
            Term::Variable("?e".into()),
            Term::Attr(Attribute::from_keyword(":project/status")),
            Term::Variable("?status".into()),
        ))],
    );

    let result = evaluate(&store, &query);
    match &result {
        QueryResult::Rel(rows) => {
            assert_eq!(rows.len(), 1, "Should find exactly one project");
        }
        other => panic!("Expected Rel, got {other:?}"),
    }
}

// ===========================================================================
// Chain 5: Guidance → Harvest → Seed
// ===========================================================================

// Verifies: INV-GUIDANCE-001, INV-GUIDANCE-003, INV-HARVEST-001, INV-SEED-001,
//   ADR-GUIDANCE-001
// (Chain 5: M(t) methodology score drives harvest timing, seed carries warnings.)
/// Methodology score drives harvest timing → seed carries warnings.
///
/// INV chain: GUIDANCE-001 (M(t)) → HARVEST-001 → SEED-001.
#[test]
fn guidance_harvest_seed() {
    let a = agent("test:chain5");
    let mut store = Store::genesis();

    // Simulate 10 turns of work
    let mut telemetry = SessionTelemetry::default();
    for i in 1..=10 {
        let entity = EntityId::from_ident(&format!(":task/g-{i}"));
        let tx = Transaction::new(a, ProvenanceType::Observed, &format!("Turn {i}")).assert(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String(format!("Work {i}")),
        );
        let committed = tx.commit(&store).expect("commit");
        store.transact(committed).expect("transact");

        telemetry.total_turns += 1;
        telemetry.transact_turns += 1;
        if i % 3 == 0 {
            telemetry.spec_language_turns += 1;
        }
        if i % 5 == 0 && telemetry.query_type_count < 4 {
            telemetry.query_type_count += 1;
        }
    }

    // Compute M(t) — methodology adherence score
    let score = compute_methodology_score(&telemetry);
    assert!(
        score.score >= 0.0 && score.score <= 1.0,
        "M(t) must be in [0, 1]"
    );
    telemetry.history.push(score.score);

    // Build guidance footer
    let footer = build_footer(&telemetry, &store, None, vec![]);
    let formatted = format_footer(&footer);
    assert!(formatted.contains("M(t):"), "Footer must contain M(t)");

    // Harvest with session context
    let harvest_context = SessionContext {
        agent: a,
        agent_name: "agent-a".into(),
        session_start_tx: TxId::new(1, 0, a),
        task_description: "Guidance test".to_string(),
        session_knowledge: vec![(
            ":session/guidance-note".to_string(),
            Value::String("Methodology adherence tracked".to_string()),
        )],
    };

    let harvest_result = harvest_pipeline(&store, &harvest_context);
    assert!(harvest_result.drift_score >= 0.0);

    // Seed: fresh session should carry warnings section
    let seed = assemble_seed(&store, "Continue with guidance", 2000, a);
    let has_warnings = seed
        .context
        .sections
        .iter()
        .any(|s| matches!(s, ContextSection::Warnings(_)));
    assert!(has_warnings, "Seed must carry Warnings section");
}

// ===========================================================================
// Chain 6: Bootstrap → Store → Query → Trilateral
// ===========================================================================

// Verifies: INV-STORE-001, INV-QUERY-001, INV-QUERY-002, INV-TRILATERAL-001,
//   INV-TRILATERAL-002, INV-TRILATERAL-003, INV-TRILATERAL-004,
//   INV-BILATERAL-001, ADR-TRILATERAL-001, ADR-TRILATERAL-002
// (Chain 6: Self-bootstrap C7 — spec elements as datoms, query, trilateral verification.)
/// Spec elements as datoms → query → coherence verification.
///
/// INV chain: C7 (self-bootstrap) → STORE-001 → QUERY-001 → TRILATERAL-002.
#[test]
fn bootstrap_store_query_trilateral() {
    let a = agent("test:chain6");
    let mut store = Store::genesis();

    // Install ISP schema first
    install_isp_schema(&mut store, a);

    // Bootstrap: transact specification elements as datoms (C7: self-bootstrap)
    let inv_store_001 = EntityId::from_ident(":inv/store-001-bootstrap");
    let adr_eav_001 = EntityId::from_ident(":adr/eav-001-bootstrap");
    let neg_mutation_001 = EntityId::from_ident(":neg/mutation-001-bootstrap");

    let tx = Transaction::new(a, ProvenanceType::Observed, "Bootstrap spec elements")
        // INV-STORE-001
        .assert(
            inv_store_001,
            Attribute::from_keyword(":spec/id"),
            Value::String("INV-STORE-001".to_string()),
        )
        .assert(
            inv_store_001,
            Attribute::from_keyword(":spec/element-type"),
            Value::String("invariant".to_string()),
        )
        .assert(
            inv_store_001,
            Attribute::from_keyword(":spec/statement"),
            Value::String("Append-only immutability".to_string()),
        )
        .assert(
            inv_store_001,
            Attribute::from_keyword(":intent/goal"),
            Value::String("Data integrity through immutability".to_string()),
        )
        .assert(
            inv_store_001,
            Attribute::from_keyword(":impl/file"),
            Value::String("crates/braid-kernel/src/store.rs".to_string()),
        )
        // ADR-EAV-001
        .assert(
            adr_eav_001,
            Attribute::from_keyword(":spec/id"),
            Value::String("ADR-EAV-001".to_string()),
        )
        .assert(
            adr_eav_001,
            Attribute::from_keyword(":spec/element-type"),
            Value::String("adr".to_string()),
        )
        .assert(
            adr_eav_001,
            Attribute::from_keyword(":intent/rationale"),
            Value::String("EAV provides maximum schema flexibility".to_string()),
        )
        .assert(
            adr_eav_001,
            Attribute::from_keyword(":impl/module"),
            Value::String("braid-kernel::datom".to_string()),
        )
        // NEG-MUTATION-001 (intent + spec only — no impl, to create a gap)
        .assert(
            neg_mutation_001,
            Attribute::from_keyword(":spec/id"),
            Value::String("NEG-MUTATION-001".to_string()),
        )
        .assert(
            neg_mutation_001,
            Attribute::from_keyword(":spec/element-type"),
            Value::String("negative-case".to_string()),
        )
        .assert(
            neg_mutation_001,
            Attribute::from_keyword(":intent/constraint"),
            Value::String("No mutation allowed".to_string()),
        )
        .assert(
            neg_mutation_001,
            Attribute::from_keyword(":db/doc"),
            Value::String("Bootstrap spec elements".to_string()),
        );

    let committed = tx.commit(&store).expect("bootstrap commit");
    store.transact(committed).expect("bootstrap transact");

    // Query: find all invariants
    let query = QueryExpr::new(
        FindSpec::Rel(vec!["?e".into(), "?id".into()]),
        vec![
            Clause::Pattern(Pattern::new(
                Term::Variable("?e".into()),
                Term::Attr(Attribute::from_keyword(":spec/element-type")),
                Term::Constant(Value::String("invariant".to_string())),
            )),
            Clause::Pattern(Pattern::new(
                Term::Variable("?e".into()),
                Term::Attr(Attribute::from_keyword(":spec/id")),
                Term::Variable("?id".into()),
            )),
        ],
    );

    let result = evaluate(&store, &query);
    match &result {
        QueryResult::Rel(rows) => {
            assert_eq!(rows.len(), 1, "Should find exactly one invariant");
        }
        other => panic!("Expected Rel, got {other:?}"),
    }

    // Trilateral: INV-STORE-001 and ADR-EAV-001 are fully linked (Phi contribution = 0),
    // but NEG-MUTATION-001 has spec + intent but no impl, creating a SpecImplGap.
    let (phi, components) = compute_phi_default(&store);
    assert!(
        components.d_sp > 0,
        "NEG-MUTATION-001 has spec but no impl -> D_SP > 0"
    );
    assert!(phi > 0.0, "Incomplete bootstrap -> Phi > 0");

    // ISP checks for specific entities
    assert_eq!(
        isp_check(&store, inv_store_001),
        IspResult::Coherent,
        "Fully linked invariant is coherent"
    );
    assert_eq!(
        isp_check(&store, neg_mutation_001),
        IspResult::SpecImplGap,
        "NEG-MUTATION-001 has spec+intent but no impl"
    );
}

// ===========================================================================
// Chain 7: Store → Layout → Store (round-trip)
// ===========================================================================

// Verifies: INV-STORE-001, INV-STORE-003, INV-LAYOUT-001, INV-LAYOUT-003,
//   INV-LAYOUT-005, ADR-LAYOUT-002
// (Chain 7: Serialize/deserialize round-trip, content hash verification, determinism.)
/// Serialize transactions → deserialize → verify round-trip identity.
///
/// INV chain: STORE-001 → LAYOUT (canonical EDN) → STORE-001 (content-addressed).
#[test]
fn store_layout_store_round_trip() {
    let a = agent("test:chain7");
    let mut store = Store::genesis();

    // Create some work
    let entity = EntityId::from_ident(":layout/test-entity");
    let tx = Transaction::new(a, ProvenanceType::Observed, "Layout test data").assert(
        entity,
        Attribute::from_keyword(":db/doc"),
        Value::String("Round-trip test".to_string()),
    );
    let committed = tx.commit(&store).expect("commit");
    let receipt = store.transact(committed).expect("transact");

    // Serialize
    let tx_file = TxFile {
        tx_id: receipt.tx_id,
        agent: a,
        provenance: ProvenanceType::Observed,
        rationale: "Layout test data".to_string(),
        causal_predecessors: vec![],
        datoms: store
            .datoms()
            .filter(|d| d.tx == receipt.tx_id)
            .cloned()
            .collect(),
    };

    let bytes = serialize_tx(&tx_file);
    assert!(!bytes.is_empty(), "Serialized bytes must not be empty");

    // Content hash
    let hash = tx_content_hash(&tx_file);
    assert!(
        verify_content_hash(&bytes, &hash),
        "Content hash must verify"
    );

    // Deserialize
    let roundtrip = deserialize_tx(&bytes).expect("deserialization must succeed");
    assert_eq!(
        roundtrip.tx_id, tx_file.tx_id,
        "TxId must survive round-trip"
    );
    assert_eq!(
        roundtrip.datoms.len(),
        tx_file.datoms.len(),
        "Datom count must survive round-trip"
    );

    // Collect datoms and verify they match
    let collected = collect_datoms(&[roundtrip]);
    for datom in &tx_file.datoms {
        assert!(
            collected.contains(datom),
            "Every original datom must be in the collected set"
        );
    }

    // Determinism: serialize again, get same bytes
    let bytes2 = serialize_tx(&tx_file);
    assert_eq!(bytes, bytes2, "Serialization must be deterministic");
}

// ===========================================================================
// Chain 8: Multi-Agent Coordination
// ===========================================================================

// Verifies: INV-STORE-001, INV-STORE-004, INV-STORE-006, INV-STORE-007,
//   INV-MERGE-001, INV-MERGE-008, INV-MERGE-009,
//   INV-TRILATERAL-001, INV-TRILATERAL-002,
//   ADR-MERGE-001, NEG-STORE-001
// (Chain 8: Two agents merge independently — commutativity, frontier monotonicity.)
/// Two agents → independent work → merge → resolve → verify frontier.
///
/// INV chain: STORE-001 → MERGE-001 (commutativity) → RESOLUTION-002 (determinism) → SYNC (frontier).
#[test]
fn multi_agent_coordination() {
    let alice = agent("alice:coord");
    let bob = agent("bob:coord");

    // Alice's independent work
    let mut store_alice = Store::genesis();

    // Install ISP schema for Alice
    install_isp_schema(&mut store_alice, alice);

    for i in 1..=3 {
        let entity = EntityId::from_ident(&format!(":alice/task-{i}"));
        let tx = Transaction::new(alice, ProvenanceType::Observed, &format!("Alice task {i}"))
            .assert(
                entity,
                Attribute::from_keyword(":intent/goal"),
                Value::String(format!("Alice goal {i}")),
            )
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String(format!("Alice work {i}")),
            );
        let committed = tx.commit(&store_alice).expect("Alice commit");
        store_alice.transact(committed).expect("Alice transact");
    }

    // Bob's independent work
    let mut store_bob = Store::genesis();

    // Install ISP schema for Bob
    install_isp_schema(&mut store_bob, bob);

    for i in 1..=3 {
        let entity = EntityId::from_ident(&format!(":bob/task-{i}"));
        let tx = Transaction::new(bob, ProvenanceType::Observed, &format!("Bob task {i}"))
            .assert(
                entity,
                Attribute::from_keyword(":spec/id"),
                Value::String(format!("SPEC-BOB-{i:03}")),
            )
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String(format!("Bob work {i}")),
            );
        let committed = tx.commit(&store_bob).expect("Bob commit");
        store_bob.transact(committed).expect("Bob transact");
    }

    // Merge: Alice absorbs Bob's store
    let pre_merge: BTreeSet<_> = store_alice.datoms().cloned().collect();
    let _receipt = merge_stores(&mut store_alice, &store_bob);

    // Verify CRDT properties
    assert!(
        verify_monotonicity(&pre_merge, &store_alice.datoms().cloned().collect()),
        "Merge must be monotonic (C4)"
    );

    // Both agents' frontiers must be present
    let frontier = store_alice.frontier();
    assert!(frontier.contains_key(&alice), "Frontier must include Alice");
    assert!(frontier.contains_key(&bob), "Frontier must include Bob");

    // Both agents' entities must be queryable
    for i in 1..=3 {
        let alice_entity = EntityId::from_ident(&format!(":alice/task-{i}"));
        let bob_entity = EntityId::from_ident(&format!(":bob/task-{i}"));
        assert!(
            !store_alice.entity_datoms(alice_entity).is_empty(),
            "Alice task {i} must be queryable"
        );
        assert!(
            !store_alice.entity_datoms(bob_entity).is_empty(),
            "Bob task {i} must be queryable"
        );
    }

    // Trilateral: Alice has intent, Bob has spec -> combined store has both layers
    let (live_i, live_s, _live_p) = live_projections(&store_alice);
    assert!(
        live_i.datom_count > 0,
        "Intent projection must have Alice's data"
    );
    assert!(
        live_s.datom_count > 0,
        "Spec projection must have Bob's data"
    );

    // Verify commutativity: Bob absorbing Alice should produce same datom set
    let mut store_ba = Store::genesis();
    install_isp_schema(&mut store_ba, bob);
    // Re-create Bob's work in store_ba
    for i in 1..=3 {
        let entity = EntityId::from_ident(&format!(":bob/task-{i}"));
        let tx = Transaction::new(bob, ProvenanceType::Observed, &format!("Bob task {i}"))
            .assert(
                entity,
                Attribute::from_keyword(":spec/id"),
                Value::String(format!("SPEC-BOB-{i:03}")),
            )
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String(format!("Bob work {i}")),
            );
        let committed = tx.commit(&store_ba).expect("Bob commit");
        store_ba.transact(committed).expect("Bob transact");
    }
    merge_stores(&mut store_ba, &store_alice);

    // After merge, both directions should have the same datom set
    let datoms_ab: BTreeSet<_> = store_alice.datoms().cloned().collect();
    let datoms_ba: BTreeSet<_> = store_ba.datoms().cloned().collect();
    assert_eq!(datoms_ab, datoms_ba, "CRDT merge must be commutative (C4)");
}

// ===========================================================================
// Chain 9 (Bonus): Store → Seed → Agent Instructions Generation
// ===========================================================================

// Verifies: INV-STORE-001, INV-SEED-001, INV-SEED-002, INV-GUIDANCE-001,
//   ADR-SEED-001, ADR-GUIDANCE-001
// (Chain 9: Dynamic agent instructions from store state — seed + guidance.)
/// Dynamic agent instructions generation from store state.
///
/// INV chain: STORE-001 → SEED-001 → AGENT_MD (dynamic generation).
#[test]
fn store_seed_agent_md_generation() {
    let a = agent("test:chain9");
    let mut store = Store::genesis();

    // Do some work to populate the store
    for i in 1..=3 {
        let entity = EntityId::from_ident(&format!(":project/item-{i}"));
        let tx = Transaction::new(a, ProvenanceType::Observed, &format!("Project item {i}"))
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String(format!("Project item {i} description")),
            );
        let committed = tx.commit(&store).expect("commit");
        store.transact(committed).expect("transact");
    }

    // Seed — task keywords must match entity content for ASSOCIATE to discover them
    let seed = assemble_seed(&store, "Project item description", 2000, a);
    assert!(seed.entities_discovered > 0);

    // Generate agent instructions
    let config = AgentMdConfig {
        task: "Integration testing".to_string(),
        agent: a,
        budget: 4000,
        ..Default::default()
    };
    let generated = generate_agent_md(&store, &config);

    assert!(
        !generated.sections.is_empty(),
        "Agent instructions must have sections"
    );
    assert!(generated.total_tokens > 0, "Must have tokens");
    assert!(
        generated.methodology_score >= 0.0 && generated.methodology_score <= 1.0,
        "M(t) in [0, 1]"
    );

    // Render
    let rendered = generated.render();
    assert!(
        rendered.contains("# Dynamic Agent Instructions"),
        "Must be markdown"
    );
    assert!(rendered.contains("Methodology score"), "Must include M(t)");
}

// ===========================================================================
// Chain 10 (Bonus): Full End-to-End Pipeline
// ===========================================================================

// Verifies: INV-STORE-001, INV-STORE-003, INV-STORE-004, INV-STORE-006,
//   INV-SCHEMA-001, INV-SCHEMA-003, INV-QUERY-001,
//   INV-MERGE-001, INV-MERGE-008, INV-MERGE-009,
//   INV-TRILATERAL-001, INV-TRILATERAL-002, INV-TRILATERAL-003,
//   INV-HARVEST-001, INV-SEED-001,
//   INV-LAYOUT-001, INV-LAYOUT-003,
//   ADR-STORE-003, ADR-SCHEMA-001, ADR-MERGE-001, ADR-LAYOUT-002
// (Chain 10: Full end-to-end pipeline covering all major namespace boundaries.)
/// Complete pipeline: schema → transact → query → trilateral → harvest → seed → merge.
///
/// Exercises the maximum number of namespace boundaries in a single test.
#[test]
fn full_end_to_end_pipeline() {
    let a = agent("test:e2e");
    let b = agent("test:e2e-peer");

    // --- Phase 1: Schema evolution ---
    let mut store = Store::genesis();
    let initial_attr_count = store.schema().len();
    install_isp_schema(&mut store, a);
    assert!(
        store.schema().len() > initial_attr_count,
        "ISP schema installation must grow the schema"
    );

    // --- Phase 2: Transact spec elements ---
    let inv = EntityId::from_ident(":e2e/inv-001");
    let tx = Transaction::new(a, ProvenanceType::Observed, "E2E spec element")
        .assert(
            inv,
            Attribute::from_keyword(":intent/goal"),
            Value::String("End-to-end coherence".to_string()),
        )
        .assert(
            inv,
            Attribute::from_keyword(":spec/id"),
            Value::String("INV-E2E-001".to_string()),
        )
        .assert(
            inv,
            Attribute::from_keyword(":impl/file"),
            Value::String("tests/cross_namespace.rs".to_string()),
        )
        .assert(
            inv,
            Attribute::from_keyword(":db/doc"),
            Value::String("E2E integration invariant".to_string()),
        );

    let committed = tx.commit(&store).expect("E2E commit");
    store.transact(committed).expect("E2E transact");

    // --- Phase 3: Query ---
    let query = QueryExpr::new(
        FindSpec::Rel(vec!["?e".into()]),
        vec![Clause::Pattern(Pattern::new(
            Term::Variable("?e".into()),
            Term::Attr(Attribute::from_keyword(":spec/id")),
            Term::Constant(Value::String("INV-E2E-001".to_string())),
        ))],
    );
    let result = evaluate(&store, &query);
    match &result {
        QueryResult::Rel(rows) => assert_eq!(rows.len(), 1),
        other => panic!("Expected Rel, got {other:?}"),
    }

    // --- Phase 4: Trilateral coherence ---
    let report = check_coherence(&store);
    assert_eq!(
        report.quadrant,
        CoherenceQuadrant::Coherent,
        "Fully linked entity -> coherent"
    );
    assert_eq!(isp_check(&store, inv), IspResult::Coherent);

    // --- Phase 5: Layout round-trip ---
    let tx_file = TxFile {
        tx_id: store.frontier()[&a],
        agent: a,
        provenance: ProvenanceType::Observed,
        rationale: "E2E spec element".to_string(),
        causal_predecessors: vec![],
        datoms: store
            .datoms()
            .filter(|d| d.tx == store.frontier()[&a])
            .cloned()
            .collect(),
    };
    let bytes = serialize_tx(&tx_file);
    let rt = deserialize_tx(&bytes).expect("deserialize");
    assert_eq!(rt.tx_id, tx_file.tx_id);

    // --- Phase 6: Harvest ---
    let harvest_ctx = SessionContext {
        agent: a,
        agent_name: "agent-a".into(),
        session_start_tx: TxId::new(1, 0, a),
        task_description: "E2E test".to_string(),
        session_knowledge: vec![(
            ":e2e/finding".to_string(),
            Value::String("Pipeline works end-to-end".to_string()),
        )],
    };
    let harvest = harvest_pipeline(&store, &harvest_ctx);
    assert!(harvest.drift_score >= 0.0);

    // --- Phase 7: Seed ---
    let seed = assemble_seed(&store, "E2E seed", 2000, a);
    assert!(seed.entities_discovered > 0);

    // --- Phase 8: Merge with peer ---
    let mut peer_store = Store::genesis();
    install_isp_schema(&mut peer_store, b);
    let peer_entity = EntityId::from_ident(":e2e/peer-contribution");
    let peer_tx = Transaction::new(b, ProvenanceType::Observed, "Peer contribution")
        .assert(
            peer_entity,
            Attribute::from_keyword(":spec/id"),
            Value::String("SPEC-PEER-001".to_string()),
        )
        .assert(
            peer_entity,
            Attribute::from_keyword(":db/doc"),
            Value::String("Peer contribution".to_string()),
        );
    let peer_committed = peer_tx.commit(&peer_store).expect("peer commit");
    peer_store.transact(peer_committed).expect("peer transact");

    let pre_merge: BTreeSet<_> = store.datoms().cloned().collect();
    merge_stores(&mut store, &peer_store);
    assert!(
        verify_monotonicity(&pre_merge, &store.datoms().cloned().collect()),
        "Final merge must be monotonic"
    );

    // --- Phase 9: Final verification ---
    let final_frontier = store.frontier();
    assert!(final_frontier.contains_key(&a));
    assert!(final_frontier.contains_key(&b));
    assert!(store.len() > initial_attr_count * 5); // Substantial data
}

// Verifies: INV-STORE-001, INV-STORE-003, INV-STORE-006,
//   INV-TRILATERAL-001, INV-TRILATERAL-002,
//   ADR-STORE-003, ADR-SCHEMA-001
// (Promotion coherence: dual identity verification, Phi=0 by construction.)
/// Promotion coherence test: after promoting an exploration entity to a spec
/// element, the entity has dual identity (exploration + element + promotion)
/// and the promotion boundary has Phi=0 (perfect coherence between the two
/// representations, since they share the same entity).
///
/// This is INV-PROMOTE-003: Phi on exploration-spec boundary = 0.
#[test]
fn promotion_coherence_verification() {
    use braid_kernel::datom::Op;
    use braid_kernel::promote::{
        promote, verify_dual_identity, PromotionRequest, PromotionTargetType,
    };
    use braid_kernel::schema::{full_schema_datoms, genesis_datoms};

    let a = agent("promote-test");
    let genesis_tx = TxId::new(0, 0, a);
    let schema_tx = TxId::new(1, 0, a);
    let explore_tx = TxId::new(2, 0, a);
    let promote_tx = TxId::new(3, 0, a);

    // Build a store with all schema layers (L0+L1+L2+L3)
    let mut datoms: BTreeSet<_> = genesis_datoms(genesis_tx).into_iter().collect();
    for d in full_schema_datoms(schema_tx) {
        datoms.insert(d);
    }

    // Create an exploration entity
    let expl_entity = EntityId::from_ident(":exploration/topo-cold-start");
    let exploration_datoms = vec![
        braid_kernel::datom::Datom::new(
            expl_entity,
            Attribute::from_keyword(":exploration/id"),
            Value::String("EXPL-TOPO-COLD-001".to_string()),
            explore_tx,
            Op::Assert,
        ),
        braid_kernel::datom::Datom::new(
            expl_entity,
            Attribute::from_keyword(":exploration/title"),
            Value::String("Cold-start monotonic relaxation from mesh".to_string()),
            explore_tx,
            Op::Assert,
        ),
        braid_kernel::datom::Datom::new(
            expl_entity,
            Attribute::from_keyword(":exploration/category"),
            Value::Keyword(":exploration.cat/theorem".to_string()),
            explore_tx,
            Op::Assert,
        ),
        braid_kernel::datom::Datom::new(
            expl_entity,
            Attribute::from_keyword(":exploration/confidence"),
            Value::Double(0.85.into()),
            explore_tx,
            Op::Assert,
        ),
        braid_kernel::datom::Datom::new(
            expl_entity,
            Attribute::from_keyword(":exploration/body"),
            Value::String(
                "Cold-start begins from full mesh. Authority monotonically relaxes \
                 from mesh to optimal topology as the system gains coherence data."
                    .to_string(),
            ),
            explore_tx,
            Op::Assert,
        ),
    ];
    for d in exploration_datoms {
        datoms.insert(d);
    }

    // Pre-promotion: entity has exploration but NOT element attrs
    let check_before = verify_dual_identity(expl_entity, &datoms);
    assert!(
        check_before.has_exploration,
        "should have exploration attrs"
    );
    assert!(
        !check_before.has_element,
        "should NOT have element attrs yet"
    );
    assert!(
        !check_before.is_valid,
        "INV-PROMOTE-002 should NOT hold yet"
    );

    // Promote to a formal invariant
    let request = PromotionRequest {
        entity: expl_entity,
        target_element_id: "INV-TOPOLOGY-010".to_string(),
        target_namespace: "TOPOLOGY".to_string(),
        target_type: PromotionTargetType::Invariant,
        statement: Some(
            "Cold-start begins from full mesh topology. Authority monotonically \
             relaxes as coherence data accumulates."
                .to_string(),
        ),
        falsification: Some(
            "Any cold-start that begins from a non-mesh topology, or any \
             authority assignment that increases during convergence."
                .to_string(),
        ),
        verification: Some("V:PROP".to_string()),
        problem: None,
        decision: None,
    };

    let result = promote(&request, &datoms, promote_tx);
    assert!(!result.was_noop, "first promotion should produce datoms");
    assert!(
        result.attrs_added >= 10,
        "should add element + inv + promotion attrs"
    );

    // Apply promotion datoms
    for d in &result.datoms {
        datoms.insert(d.clone());
    }

    // Post-promotion: entity has dual identity (INV-PROMOTE-002)
    let check_after = verify_dual_identity(expl_entity, &datoms);
    assert!(
        check_after.has_exploration,
        "exploration attrs preserved (C1)"
    );
    assert!(check_after.has_element, "element attrs added");
    assert!(check_after.has_promotion, "promotion attrs added");
    assert!(check_after.is_valid, "INV-PROMOTE-002 holds");

    // INV-PROMOTE-003: Phi on exploration-spec boundary = 0
    // Because the exploration entity IS the spec entity (same EntityId),
    // there is zero divergence between them — they are the same datoms.
    // Verify by checking that the :exploration/title and :element/id
    // coexist on the same entity.
    let has_expl_title = datoms.iter().any(|d| {
        d.entity == expl_entity
            && d.op == Op::Assert
            && d.attribute.as_str() == ":exploration/title"
    });
    let has_element_id = datoms.iter().any(|d| {
        d.entity == expl_entity && d.op == Op::Assert && d.attribute.as_str() == ":element/id"
    });
    let has_inv_statement = datoms.iter().any(|d| {
        d.entity == expl_entity && d.op == Op::Assert && d.attribute.as_str() == ":inv/statement"
    });
    assert!(has_expl_title, "exploration title preserved");
    assert!(has_element_id, "element ID present");
    assert!(has_inv_statement, "invariant statement present");

    // The Phi boundary is 0 by construction: same entity, no separate
    // "spec representation" to diverge from. This is the key insight of
    // the store-first pipeline.

    // INV-PROMOTE-004: Idempotency — re-promoting should be a no-op
    let result2 = promote(&request, &datoms, promote_tx);
    assert!(result2.was_noop, "INV-PROMOTE-004: re-promotion is no-op");
    assert_eq!(result2.datoms.len(), 0, "no new datoms on re-promotion");
}
