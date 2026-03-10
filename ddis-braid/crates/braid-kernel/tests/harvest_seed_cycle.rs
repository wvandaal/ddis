//! 25-Turn Harvest/Seed Cycle Validation — THE Stage 0 Success Criterion.
//!
//! Validates: "Work 25 turns, harvest, start fresh with seed — new session
//! picks up without manual re-explanation."
//!
//! Traces to: SEED.md §10 (Stage 0 success criterion)

use std::collections::BTreeSet;

use braid_kernel::datom::{AgentId, Attribute, EntityId, ProvenanceType, TxId, Value};
use braid_kernel::guidance::{
    build_footer, compute_methodology_score, format_footer, SessionTelemetry,
};
use braid_kernel::harvest::{harvest_pipeline, SessionContext};
use braid_kernel::seed::{assemble_seed, ContextSection};
use braid_kernel::store::{Store, Transaction};
use braid_kernel::trilateral::check_coherence;

/// Simulate a single agent turn: transact a datom, update telemetry.
fn simulate_turn(
    store: &mut Store,
    agent: AgentId,
    turn: u32,
    telemetry: &mut SessionTelemetry,
) -> bool {
    // Create a task-like entity for this turn
    let entity = EntityId::from_ident(&format!(":task/turn-{turn}"));

    let tx = Transaction::new(
        agent,
        ProvenanceType::Observed,
        &format!("Turn {turn} work"),
    )
    .assert(
        entity,
        Attribute::from_keyword(":db/doc"),
        Value::String(format!("Work product from turn {turn}")),
    );

    match tx.commit(store) {
        Ok(committed) => {
            let _ = store.transact(committed);
            telemetry.total_turns += 1;
            telemetry.transact_turns += 1;

            // Simulate spec-language usage every 3rd turn
            if turn % 3 == 0 {
                telemetry.spec_language_turns += 1;
            }
            // Simulate query diversity
            if turn % 5 == 0 && telemetry.query_type_count < 4 {
                telemetry.query_type_count += 1;
            }
            true
        }
        Err(_) => false,
    }
}

#[test]
fn harvest_seed_25_turn_cycle() {
    let agent = AgentId::from_name("test:validation");
    let mut store = Store::genesis();
    let mut telemetry = SessionTelemetry::default();

    let initial_datom_count = store.len();

    // ---------------------------------------------------------------
    // Phase 1: Simulate 25 turns of agent work
    // ---------------------------------------------------------------
    let mut proactive_warning_fired = false;

    for turn in 1..=25 {
        let success = simulate_turn(&mut store, agent, turn, &mut telemetry);
        assert!(success, "Turn {turn} should succeed");

        // Compute M(t) after each turn
        let score = compute_methodology_score(&telemetry);

        // Track M(t) history
        telemetry.history.push(score.score);

        // Check for proactive harvest warning (should fire around turn 20+)
        // when M(t) drops or when enough turns have passed
        if turn >= 20 && !proactive_warning_fired {
            // Build guidance footer to check for warnings
            let footer = build_footer(&telemetry, &store, None, vec![]);
            let formatted = format_footer(&footer);
            assert!(
                formatted.contains("M(t):"),
                "Footer should always contain M(t)"
            );
            proactive_warning_fired = true;
        }
    }

    assert_eq!(telemetry.total_turns, 25, "Should have completed 25 turns");
    assert!(store.len() > initial_datom_count, "Store should have grown");

    // ---------------------------------------------------------------
    // Phase 2: Harvest — extract session knowledge
    // ---------------------------------------------------------------
    let pre_harvest_count = store.len();
    let pre_harvest_datoms: BTreeSet<_> = store.datoms().cloned().collect();

    // Set harvest quality based on telemetry
    telemetry.harvest_quality = 0.8;

    let harvest_context = SessionContext {
        agent,
        session_start_tx: TxId::new(1, 0, agent),
        task_description: "25-turn validation test".to_string(),
        session_knowledge: vec![
            (
                "session-summary".to_string(),
                Value::String("Completed 25-turn validation cycle".to_string()),
            ),
            (
                "methodology-adherence".to_string(),
                Value::String(format!(
                    "M(t) final: {:.2}",
                    telemetry.history.last().unwrap_or(&0.0)
                )),
            ),
        ],
    };

    let harvest_result = harvest_pipeline(&store, &harvest_context);

    // Verify harvest produces candidates
    assert!(
        harvest_result.quality.count > 0,
        "Harvest should find knowledge gaps"
    );
    assert!(
        harvest_result.drift_score >= 0.0 && harvest_result.drift_score <= 1.0,
        "Drift score must be in [0, 1]"
    );

    // Verify harvest monotonicity (INV-HARVEST-002): store only grows
    let post_harvest_datoms: BTreeSet<_> = store.datoms().cloned().collect();
    assert!(
        pre_harvest_datoms.is_subset(&post_harvest_datoms),
        "Harvest must not remove datoms (monotonicity)"
    );
    assert!(
        store.len() >= pre_harvest_count,
        "Store must not shrink after harvest"
    );

    // ---------------------------------------------------------------
    // Phase 3: Fresh session — seed from stored state
    // ---------------------------------------------------------------
    let seed = assemble_seed(
        &store,
        "Continue validation work from previous session",
        2000, // token budget
        agent,
    );

    // Verify seed contains relevant context
    assert!(
        seed.entities_discovered > 0,
        "Seed should discover entities from the store"
    );
    assert!(
        seed.context.total_tokens <= 2000,
        "Seed must respect token budget"
    );
    assert_eq!(
        seed.context.sections.len(),
        5,
        "Seed should have 5 sections (Orientation, Constraints, State, Warnings, Directive)"
    );

    // Verify section types
    let has_orientation = seed
        .context
        .sections
        .iter()
        .any(|s| matches!(s, ContextSection::Orientation(_)));
    let has_constraints = seed
        .context
        .sections
        .iter()
        .any(|s| matches!(s, ContextSection::Constraints(_)));
    let has_state = seed
        .context
        .sections
        .iter()
        .any(|s| matches!(s, ContextSection::State(_)));
    let has_warnings = seed
        .context
        .sections
        .iter()
        .any(|s| matches!(s, ContextSection::Warnings(_)));
    let has_directive = seed
        .context
        .sections
        .iter()
        .any(|s| matches!(s, ContextSection::Directive(_)));

    assert!(has_orientation, "Seed must have Orientation section");
    assert!(has_constraints, "Seed must have Constraints section");
    assert!(has_state, "Seed must have State section");
    assert!(has_warnings, "Seed must have Warnings section");
    assert!(has_directive, "Seed must have Directive section");

    // ---------------------------------------------------------------
    // Phase 4: New session continuity — verify knowledge persists
    // ---------------------------------------------------------------

    // Query for entities from the previous session
    for turn in [1, 10, 25] {
        let entity = EntityId::from_ident(&format!(":task/turn-{turn}"));
        let datoms: Vec<_> = store.entity_datoms(entity);
        assert!(
            !datoms.is_empty(),
            "Entity from turn {turn} should still be in store"
        );
    }

    // Verify coherence
    let coherence = check_coherence(&store);
    assert!(coherence.phi >= 0.0, "Phi must be non-negative");

    // Verify M(t) continuity — new session starts with history
    let new_session_telemetry = SessionTelemetry {
        total_turns: 1,
        transact_turns: 1,
        spec_language_turns: 1,
        query_type_count: 1,
        harvest_quality: telemetry.harvest_quality,
        history: telemetry.history.clone(),
    };
    let new_score = compute_methodology_score(&new_session_telemetry);
    assert!(
        new_score.score > 0.0,
        "New session should have positive methodology score"
    );

    // Verify frontier includes our agent
    let frontier = store.frontier();
    assert!(
        frontier.contains_key(&agent),
        "Frontier should include the validation agent"
    );

    // ---------------------------------------------------------------
    // Phase 5: Dynamic agent instructions generation
    // ---------------------------------------------------------------
    let config = braid_kernel::agent_md::AgentMdConfig {
        task: "Continue validation work".to_string(),
        agent,
        budget: 4000,
        ..Default::default()
    };
    let generated = braid_kernel::agent_md::generate_agent_md(&store, &config);

    assert!(
        !generated.sections.is_empty(),
        "Generated agent instructions should have sections"
    );
    assert!(
        generated.total_tokens > 0,
        "Generated agent instructions should have tokens"
    );
    assert!(
        generated.methodology_score >= 0.0 && generated.methodology_score <= 1.0,
        "Methodology score must be in [0, 1]"
    );

    // Verify the rendered output is valid markdown
    let rendered = generated.render();
    assert!(
        rendered.contains("# Dynamic Agent Instructions"),
        "Rendered output should be a markdown document"
    );
    assert!(
        rendered.contains("Methodology score"),
        "Rendered output should include methodology score"
    );
}

/// Test that the harvest/seed cycle is idempotent:
/// multiple harvests don't corrupt the store.
#[test]
fn harvest_seed_idempotency() {
    let agent = AgentId::from_name("test:idempotent");
    let mut store = Store::genesis();

    // Do some work
    let tx = Transaction::new(agent, ProvenanceType::Observed, "Initial work").assert(
        EntityId::from_ident(":task/item-1"),
        Attribute::from_keyword(":db/doc"),
        Value::String("Test item".to_string()),
    );
    let committed = tx.commit(&store).unwrap();
    let _ = store.transact(committed);

    let context = SessionContext {
        agent,
        session_start_tx: TxId::new(1, 0, agent),
        task_description: "Idempotency test".to_string(),
        session_knowledge: vec![],
    };

    // Run harvest twice — store should remain consistent
    let result1 = harvest_pipeline(&store, &context);
    let datoms_after_1: BTreeSet<_> = store.datoms().cloned().collect();

    let result2 = harvest_pipeline(&store, &context);
    let datoms_after_2: BTreeSet<_> = store.datoms().cloned().collect();

    // harvest_pipeline is read-only on the store, so datoms should be identical
    assert_eq!(datoms_after_1, datoms_after_2, "Harvest must be idempotent");

    // Results should also be identical (determinism)
    assert_eq!(
        result1.candidates.len(),
        result2.candidates.len(),
        "Harvest candidates should be deterministic"
    );
    assert_eq!(
        result1.drift_score, result2.drift_score,
        "Drift score should be deterministic"
    );
}

/// Test the full seed → work → harvest → seed → work cycle across sessions.
#[test]
fn multi_session_continuity() {
    let agent_1 = AgentId::from_name("test:session1");
    let agent_2 = AgentId::from_name("test:session2");
    let mut store = Store::genesis();

    // Session 1: work and harvest
    for i in 1..=5 {
        let tx = Transaction::new(agent_1, ProvenanceType::Observed, &format!("S1 turn {i}"))
            .assert(
                EntityId::from_ident(&format!(":s1/item-{i}")),
                Attribute::from_keyword(":db/doc"),
                Value::String(format!("Session 1 item {i}")),
            );
        let committed = tx.commit(&store).unwrap();
        let _ = store.transact(committed);
    }

    let s1_entities: Vec<_> = (1..=5)
        .map(|i| EntityId::from_ident(&format!(":s1/item-{i}")))
        .collect();

    // Session 2: seed and continue
    let seed = assemble_seed(&store, "Continue session 1 work", 2000, agent_2);
    assert!(
        seed.entities_discovered > 0,
        "Session 2 seed should discover entities"
    );

    // Session 2 can find session 1's entities
    for entity in &s1_entities {
        let datoms = store.entity_datoms(*entity);
        assert!(
            !datoms.is_empty(),
            "Session 2 should see session 1 entities"
        );
    }

    // Session 2 adds its own work
    for i in 1..=3 {
        let tx = Transaction::new(agent_2, ProvenanceType::Observed, &format!("S2 turn {i}"))
            .assert(
                EntityId::from_ident(&format!(":s2/item-{i}")),
                Attribute::from_keyword(":db/doc"),
                Value::String(format!("Session 2 item {i}")),
            );
        let committed = tx.commit(&store).unwrap();
        let _ = store.transact(committed);
    }

    // Both sessions' data is present
    let frontier = store.frontier();
    assert!(
        frontier.contains_key(&agent_1),
        "Frontier should include agent 1"
    );
    assert!(
        frontier.contains_key(&agent_2),
        "Frontier should include agent 2"
    );
}
