// Witnesses: INV-BILATERAL-005, INV-TRANSACT-COHERENCE-001, INV-DELIBERATION-001,
//   INV-DELIBERATION-002, INV-DELIBERATION-005, INV-STORE-013, INV-MERGE-003,
//   INV-HARVEST-005, INV-HARVEST-010, INV-HARVEST-011
//
//! Wave 3.5 / 4 / 5 gate integration tests.
//!
//! Each test validates the GATE criteria for a wave milestone:
//!
//! - **W3.5**: Pattern detection covers >= 60% of spec elements; generated code
//!   has valid structure (balanced braces, valid function names).
//! - **W4**: Closed distillation loop — observation with universal quantifier +
//!   high confidence flows through harvest -> proposal -> accept -> spec datoms.
//! - **W4-DOGFOOD**: Real braid design decisions distilled through the pipeline.
//! - **W5**: Multi-agent staging — two AgentStores commit against a shared store
//!   with coherence gating, conflict detection, and deliberation resolution.
//!
//! Traces to: SEED.md §7 (Self-Improvement Loop), §10 (Staged Roadmap)

use std::collections::BTreeSet;

use braid_kernel::agent_store::AgentStore;
use braid_kernel::coherence::CoherenceViolation;
use braid_kernel::compiler::{
    detect_patterns, emit_proptest, emit_test_module, extract_test_property, summarize_patterns,
};
use braid_kernel::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};
use braid_kernel::deliberation::{
    add_position, check_stability, coherence_violation_to_deliberation, decide, open_deliberation,
    DecisionMethod,
};
use braid_kernel::harvest::{
    classify_spec_candidate, contains_universal_quantifier, harvest_pipeline, SessionContext,
    SpecCandidateType,
};
use braid_kernel::proposal::{accept_proposal, pending_proposals, proposal_to_datoms};
use braid_kernel::schema;
use braid_kernel::store::{Store, Transaction};

// ===========================================================================
// Helpers
// ===========================================================================

fn test_agent(name: &str) -> AgentId {
    AgentId::from_name(name)
}

fn test_tx(wall: u64, agent: AgentId) -> TxId {
    TxId::new(wall, 0, agent)
}

/// Build a store with full schema (Layers 0-3) for agent store tests.
fn full_schema_store() -> Store {
    let system_agent = AgentId::from_name("braid:system");
    let genesis_tx = TxId::new(0, 0, system_agent);
    let mut datom_set: BTreeSet<Datom> = BTreeSet::new();
    for d in schema::genesis_datoms(genesis_tx) {
        datom_set.insert(d);
    }
    for d in schema::full_schema_datoms(genesis_tx) {
        datom_set.insert(d);
    }
    Store::from_datoms(datom_set)
}

/// Populate a store with a set of spec elements using `:spec/element-type`,
/// `:spec/statement`, and `:spec/falsification`. Returns the count of spec
/// entities added.
fn populate_spec_store(store: &mut Store, agent: AgentId) -> usize {
    // Representative spec elements spanning the 9 universal patterns.
    // Each tuple: (ident, element-type, statement, falsification).
    let specs: Vec<(&str, &str, &str, &str)> = vec![
        (
            ":spec/inv-store-001",
            ":element.type/invariant",
            "The datom store never deletes or mutates an existing datom. All state changes are new assertions.",
            "Any operation that removes a datom or modifies an existing tuple violates this invariant.",
        ),
        (
            ":spec/inv-store-003",
            ":element.type/invariant",
            "A datom's identity is determined by its content. Two agents asserting the same fact produce identical datoms with equal entity IDs.",
            "Two datoms with identical (e,a,v,tx,op) produce different entity IDs, or different content produces the same entity ID.",
        ),
        (
            ":spec/inv-store-004",
            ":element.type/invariant",
            "Merging two stores is commutative: merge(A,B) = merge(B,A). The order of merge operands does not matter.",
            "merge(A,B) produces a different datom set than merge(B,A) for any stores A and B.",
        ),
        (
            ":spec/inv-store-005",
            ":element.type/invariant",
            "Merging is associative: merge(merge(A,B),C) = merge(A,merge(B,C)).",
            "The grouping of merge operands produces a different result.",
        ),
        (
            ":spec/inv-store-006",
            ":element.type/invariant",
            "Merging a store with itself is idempotent: merge(S,S) = S.",
            "merge(S,S) produces a store with more datoms than S.",
        ),
        (
            ":spec/inv-store-007",
            ":element.type/invariant",
            "The store is monotonically non-decreasing: len(S) never decreases after any operation. The store only grows, never shrinks.",
            "After any operation, len(S_post) < len(S_pre).",
        ),
        (
            ":spec/inv-query-001",
            ":element.type/invariant",
            "Query results are deterministic: the same query on the same store produces the same result every time.",
            "Running the same query twice on an unchanged store produces different results.",
        ),
        (
            ":spec/inv-schema-004",
            ":element.type/invariant",
            "Every transacted datom must reference an attribute that exists in the schema. Schema validation is bounded: O(|new_datoms| * O(1)).",
            "A datom referencing an unregistered attribute is accepted into the store.",
        ),
        (
            ":spec/inv-harvest-002",
            ":element.type/invariant",
            "Harvest monotonically extends the store: every harvest commit adds datoms, never removes them. The store maintains all existing knowledge.",
            "A harvest commit results in fewer datoms than before the commit.",
        ),
        (
            ":spec/inv-seed-001",
            ":element.type/invariant",
            "The seed contains only information from the store — nothing fabricated. Every element in the seed output must have a traceable source datom.",
            "A seed output contains a fact not present in any store datom.",
        ),
    ];

    let count = specs.len();

    for (ident, element_type, statement, falsification) in specs {
        let entity = EntityId::from_ident(ident);
        let tx = Transaction::new(agent, ProvenanceType::Observed, &format!("Add {ident}"))
            .assert(
                entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(ident.to_string()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":spec/element-type"),
                Value::Keyword(element_type.to_string()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":spec/statement"),
                Value::String(statement.to_string()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":spec/falsification"),
                Value::String(falsification.to_string()),
            )
            .assert(
                entity,
                Attribute::from_keyword(":spec/id"),
                Value::String(ident.trim_start_matches(":spec/").to_string()),
            );

        let committed = tx
            .commit(store)
            .expect("spec element transaction must succeed");
        store
            .transact(committed)
            .expect("spec element transact must succeed");
    }

    count
}

/// Add an exploration observation to a store. Returns the observation entity ID.
fn add_observation(
    store: &mut Store,
    agent: AgentId,
    ident: &str,
    body: &str,
    category: &str,
    confidence: f64,
    _wall_time: u64,
) -> EntityId {
    let entity = EntityId::from_ident(ident);
    let tx = Transaction::new(agent, ProvenanceType::Observed, "observe")
        .assert(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(ident.to_string()),
        )
        .assert(
            entity,
            Attribute::from_keyword(":exploration/body"),
            Value::String(body.to_string()),
        )
        .assert(
            entity,
            Attribute::from_keyword(":exploration/category"),
            Value::Keyword(category.to_string()),
        )
        .assert(
            entity,
            Attribute::from_keyword(":exploration/confidence"),
            Value::Double(confidence.into()),
        )
        .assert(
            entity,
            Attribute::from_keyword(":exploration/source"),
            Value::String("wave-gate-test".to_string()),
        )
        .assert(
            entity,
            Attribute::from_keyword(":exploration/maturity"),
            Value::Keyword(":exploration.maturity/sketch".to_string()),
        )
        .assert(
            entity,
            Attribute::from_keyword(":exploration/content-hash"),
            Value::Bytes(blake3::hash(body.as_bytes()).as_bytes().to_vec()),
        );

    let committed = tx.commit(store).expect("observation tx must succeed");
    store
        .transact(committed)
        .expect("observation transact must succeed");
    entity
}

// ===========================================================================
// W3.5 Gate: Pattern Detection Coverage (brai-17kc.8)
// ===========================================================================

/// W3.5 GATE: Pattern detection covers >= 60% of spec elements.
///
/// Loads a store with representative spec elements (invariants with statement
/// and falsification fields), runs `detect_patterns`, and asserts that:
/// 1. At least 60% of spec elements match at least one pattern.
/// 2. The summary statistics are consistent.
///
/// Traces to: SEED.md section 7 (Self-Improvement Loop), INV-BILATERAL-005.
#[test]
fn w35_gate_pattern_detection_coverage() {
    let agent = test_agent("w35-gate");
    let mut store = full_schema_store();

    let spec_count = populate_spec_store(&mut store, agent);
    assert!(
        spec_count >= 10,
        "Test requires at least 10 spec elements, got {spec_count}"
    );

    // Run pattern detection
    let matches = detect_patterns(&store);

    // Summarize
    let summary = summarize_patterns(&matches, spec_count);

    // GATE CRITERION: >= 60% coverage
    let coverage = summary.matched_elements as f64 / spec_count as f64;
    assert!(
        coverage >= 0.6,
        "W3.5 GATE FAILED: pattern detection coverage is {:.1}% ({}/{}) — must be >= 60%",
        coverage * 100.0,
        summary.matched_elements,
        spec_count
    );

    // Consistency: matched + unmatched = total
    assert_eq!(
        summary.matched_elements + summary.unmatched_elements,
        spec_count,
        "Summary matched + unmatched must equal total spec elements"
    );

    // All matches must have confidence > 0
    for m in &matches {
        assert!(
            m.confidence > 0.0,
            "Pattern match for {} must have positive confidence, got {}",
            m.spec_id,
            m.confidence
        );
    }

    // At least 3 distinct patterns should be represented (we have 9 patterns
    // and 10 spec elements designed to cover different patterns)
    let distinct_patterns: std::collections::HashSet<_> =
        matches.iter().map(|m| m.pattern).collect();
    assert!(
        distinct_patterns.len() >= 3,
        "Expected at least 3 distinct patterns, got {} ({:?})",
        distinct_patterns.len(),
        distinct_patterns,
    );
}

/// W3.5 GATE: Generated test code is structurally valid.
///
/// Extracts test properties from pattern matches, generates proptest code via
/// `emit_proptest` and `emit_test_module`, and checks structural validity:
/// balanced braces, valid function names, proper module wrapper.
#[test]
fn w35_gate_generated_code_structure() {
    let agent = test_agent("w35-codegen");
    let mut store = full_schema_store();
    populate_spec_store(&mut store, agent);

    let matches = detect_patterns(&store);
    assert!(
        !matches.is_empty(),
        "Need at least one match for code generation"
    );

    // Extract test properties from matches
    let properties: Vec<_> = matches.iter().map(extract_test_property).collect();
    assert!(
        !properties.is_empty(),
        "extract_test_property must produce at least one property"
    );

    // Generate individual proptest blocks
    for prop in &properties {
        let code = emit_proptest(prop);

        // Balanced braces
        let open_braces = code.chars().filter(|c| *c == '{').count();
        let close_braces = code.chars().filter(|c| *c == '}').count();
        assert_eq!(
            open_braces, close_braces,
            "Generated proptest for {} has unbalanced braces: {} open, {} close",
            prop.inv_id, open_braces, close_braces
        );

        // Must contain a function name starting with "generated_"
        assert!(
            code.contains("fn generated_"),
            "Generated proptest for {} must have a fn named generated_*",
            prop.inv_id
        );

        // Must contain proptest! macro
        assert!(
            code.contains("proptest!"),
            "Generated code for {} must use proptest! macro",
            prop.inv_id
        );
    }

    // Generate complete module
    let module = emit_test_module(&properties);

    // Module must have balanced braces
    let open_braces = module.chars().filter(|c| *c == '{').count();
    let close_braces = module.chars().filter(|c| *c == '}').count();
    assert_eq!(
        open_braces, close_braces,
        "Generated test module has unbalanced braces: {} open, {} close",
        open_braces, close_braces
    );

    // Module must have the #[cfg(test)] wrapper
    assert!(
        module.contains("#[cfg(test)]"),
        "Generated module must have #[cfg(test)] attribute"
    );
    assert!(
        module.contains("mod generated_coherence_tests"),
        "Generated module must be named generated_coherence_tests"
    );
}

// ===========================================================================
// W4 Gate: Closed Distillation Loop (brai-qtf6.5)
// ===========================================================================

/// W4 GATE: Observation with universal quantifier + high confidence flows
/// through the full distillation loop: observe -> classify -> proposal ->
/// accept -> spec datoms in store.
///
/// Traces to: SEED.md section 7 (Self-Improvement Loop)
#[test]
fn w4_gate_closed_distillation_loop() {
    let agent = test_agent("w4-distill");
    let mut store = full_schema_store();

    // Step 1: Create an observation with universal quantifier language + high confidence
    let obs_body = "Every transaction must always include a valid provenance type. \
         This invariant ensures traceability across the entire audit trail.";
    assert!(
        contains_universal_quantifier(obs_body),
        "Test precondition: observation body must contain universal quantifier"
    );

    let obs_entity = add_observation(
        &mut store,
        agent,
        ":observation/w4-test-provenance-invariant",
        obs_body,
        "design-decision",
        0.92, // High confidence (>= 0.8 threshold)
        1000,
    );

    // Step 2: Classify as spec candidate (the harvest pipeline does this internally)
    let candidate = classify_spec_candidate(obs_entity, &store);
    assert!(
        candidate.is_some(),
        "W4 GATE: Observation with universal quantifier + confidence >= 0.8 must classify as spec candidate"
    );
    let candidate = candidate.unwrap();
    assert_eq!(
        candidate.candidate_type,
        SpecCandidateType::Invariant,
        "W4 GATE: Must classify as Invariant (has universal quantifier + high confidence)"
    );
    assert!(
        candidate.confidence >= 0.8,
        "Candidate confidence must be >= 0.8, got {}",
        candidate.confidence
    );

    // Step 3: Convert to proposal datoms and transact
    let proposal_tx = test_tx(2000, agent);
    let proposal_datoms = proposal_to_datoms(&candidate, proposal_tx);
    assert!(
        !proposal_datoms.is_empty(),
        "proposal_to_datoms must produce datoms"
    );

    // Transact proposal into store
    let mut all_datoms = store.datom_set().clone();
    for d in &proposal_datoms {
        all_datoms.insert(d.clone());
    }
    store = Store::from_datoms(all_datoms);

    // Step 4: Verify proposal is pending
    let pending = pending_proposals(&store);
    assert!(
        !pending.is_empty(),
        "W4 GATE: After transacting proposal, pending_proposals must be non-empty"
    );

    // Find our proposal
    let proposal_entity = proposal_datoms[0].entity;
    let our_proposal = pending.iter().find(|(e, _, _)| *e == proposal_entity);
    assert!(
        our_proposal.is_some(),
        "Our proposal entity must appear in pending proposals"
    );

    // Step 5: Accept the proposal
    let accept_tx = test_tx(3000, agent);
    let accept_datoms = accept_proposal(&store, proposal_entity, accept_tx);
    assert!(
        !accept_datoms.is_empty(),
        "W4 GATE: accept_proposal must produce datoms for a pending proposal"
    );

    // Transact acceptance
    let mut all_datoms = store.datom_set().clone();
    for d in &accept_datoms {
        all_datoms.insert(d.clone());
    }
    store = Store::from_datoms(all_datoms);

    // Step 6: Verify the closed loop — spec datoms now in store
    let entity_datoms = store.entity_datoms(proposal_entity);

    // Must have :proposal/status = :proposal.status/accepted
    let accepted = entity_datoms.iter().any(|d| {
        d.attribute.as_str() == ":proposal/status"
            && d.value == Value::Keyword(":proposal.status/accepted".to_string())
    });
    assert!(
        accepted,
        "W4 GATE: Proposal must have status :proposal.status/accepted after accept"
    );

    // Must have :spec/element-type (promoted to spec element)
    let has_spec_type = entity_datoms
        .iter()
        .any(|d| d.attribute.as_str() == ":spec/element-type" && d.op == Op::Assert);
    assert!(
        has_spec_type,
        "W4 GATE: Accepted proposal must have :spec/element-type (promoted to spec element)"
    );

    // Must have :spec/statement (the observation body carried through)
    let has_statement = entity_datoms
        .iter()
        .any(|d| d.attribute.as_str() == ":spec/statement" && d.op == Op::Assert);
    assert!(
        has_statement,
        "W4 GATE: Accepted proposal must have :spec/statement"
    );

    // Verify the closed loop: the observation -> proposal -> spec element
    // chain is complete. The entity has datoms from all three phases.
    let has_proposal_source = entity_datoms
        .iter()
        .any(|d| d.attribute.as_str() == ":proposal/source" && d.op == Op::Assert);
    assert!(
        has_proposal_source,
        "W4 GATE: Spec element must retain :proposal/source tracing back to observation"
    );
}

// ===========================================================================
// W4 Dogfood: Distill From Own Design Decisions
// ===========================================================================

/// W4 DOGFOOD: Real braid design decisions flow through the distillation pipeline.
///
/// Adds braid's own design decisions as observations (with the content and
/// confidence they would naturally have), then runs harvest + classification
/// to verify proposals are generated for genuine architectural decisions.
///
/// Traces to: C7 (Self-bootstrap), SEED.md section 7
#[test]
fn w4_dogfood_distill_from_own_sessions() {
    let agent = test_agent("w4-dogfood");
    let mut store = full_schema_store();

    // Real braid design decisions as observations
    let decisions: Vec<(&str, &str, &str, f64)> = vec![
        (
            ":observation/adr-eav-model",
            "Chose EAV datom model over relational tables. EAV gives schema flexibility \
             and content-addressable identity. Relational was considered but rejected: \
             too rigid for evolving specifications. Instead of fixed tables, every fact \
             is a datom.",
            "design-decision",
            0.95,
        ),
        (
            ":observation/inv-append-only",
            "The store must always be append-only. Every operation must preserve existing \
             datoms. This invariant is fundamental to CRDT merge correctness.",
            "design-decision",
            0.92,
        ),
        (
            ":observation/neg-no-deletion",
            "Direct deletion of datoms is forbidden and prohibited by the protocol. \
             Retractions are new datoms, not removals. The system prevents and avoids \
             any operation that removes existing data.",
            "observation",
            0.75,
        ),
        (
            ":observation/adr-crdt-merge",
            "Chose set union as merge semantics over operational transforms. \
             Set union is commutative, associative, idempotent — rather than \
             complex OT reconciliation. Compared to Raft consensus, CRDT merge \
             requires no coordination.",
            "design-decision",
            0.90,
        ),
    ];

    let mut obs_entities = Vec::new();
    for (i, (ident, body, category, confidence)) in decisions.iter().enumerate() {
        let entity = add_observation(
            &mut store,
            agent,
            ident,
            body,
            category,
            *confidence,
            (i as u64 + 1) * 1000,
        );
        obs_entities.push(entity);
    }

    // Classify each observation
    let mut invariant_candidates = 0;
    let mut adr_candidates = 0;
    let mut negative_candidates = 0;

    for entity in &obs_entities {
        if let Some(candidate) = classify_spec_candidate(*entity, &store) {
            match candidate.candidate_type {
                SpecCandidateType::Invariant => invariant_candidates += 1,
                SpecCandidateType::ADR => adr_candidates += 1,
                SpecCandidateType::NegativeCase => negative_candidates += 1,
            }
        }
    }

    // At least one proposal generated per type
    assert!(
        invariant_candidates >= 1,
        "W4 DOGFOOD: At least 1 invariant candidate expected from real design decisions, got {}",
        invariant_candidates
    );
    assert!(
        adr_candidates >= 1,
        "W4 DOGFOOD: At least 1 ADR candidate expected from real design decisions, got {}",
        adr_candidates
    );
    assert!(
        negative_candidates >= 1,
        "W4 DOGFOOD: At least 1 negative case candidate expected from real design decisions, got {}",
        negative_candidates
    );

    // Total: at least 3 out of 4 observations should classify
    let total = invariant_candidates + adr_candidates + negative_candidates;
    assert!(
        total >= 3,
        "W4 DOGFOOD: At least 3/4 real design decisions should classify as spec candidates, got {}",
        total
    );

    // Run harvest pipeline to verify end-to-end.
    // session_start_tx must be before the observation transactions. Since
    // full_schema_store() has clock at wall_time=0, our observations got
    // TxIds at wall_time=0 with logical > 0. Use the genesis TxId so all
    // observation datoms satisfy datom.tx > session_start_tx.
    let system_agent = AgentId::from_name("braid:system");
    let context = SessionContext {
        agent,
        agent_name: "w4-dogfood".into(),
        session_start_tx: TxId::new(0, 0, system_agent),
        task_description: "Dogfood: distill braid's own design decisions".into(),
        session_knowledge: vec![],
    };

    let result = harvest_pipeline(&store, &context);
    assert!(
        result.session_entities > 0,
        "W4 DOGFOOD: Harvest pipeline must detect session entities"
    );
}

// ===========================================================================
// W5 Gate: Multi-Agent Staging (brai-1ry7.4)
// ===========================================================================

/// W5 GATE: Multi-agent staging with coherence gating and deliberation.
///
/// Two AgentStores commit against a shared store. Agent 1 commits successfully.
/// Agent 2 tries a conflicting commit, gets a coherence violation, which opens
/// a deliberation. Positions are added, a decision is made, and the shared
/// store ends up consistent.
///
/// Traces to: SEED.md section 4 (Design Commitment #2), spec/07-deliberation.md,
/// INV-STORE-013, INV-MERGE-003, INV-TRANSACT-COHERENCE-001, INV-DELIBERATION-001
#[test]
fn w5_gate_multi_agent_staging() {
    let alice = test_agent("alice");
    let bob = test_agent("bob");
    let shared = full_schema_store();

    // Both agents get their own AgentStore over the same shared trunk
    let mut alice_store = AgentStore::new(shared.clone_store(), alice);
    let mut bob_store = AgentStore::new(shared, bob);

    let entity = EntityId::from_ident(":config/max-retries");

    // Step 1: Both agents assert locally (different values for same entity+attribute)
    alice_store
        .assert_local(
            vec![(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("max-retries = 3".into()),
            )],
            ProvenanceType::Observed,
            "Alice sets max-retries to 3",
        )
        .expect("Alice local assert must succeed");

    bob_store
        .assert_local(
            vec![(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("max-retries = 5".into()),
            )],
            ProvenanceType::Observed,
            "Bob sets max-retries to 5",
        )
        .expect("Bob local assert must succeed");

    // Step 2: Alice commits first -- should succeed (no conflict in shared)
    let alice_receipt = alice_store.commit(&[entity]);
    assert!(
        alice_receipt.is_ok(),
        "W5 GATE: Alice's first commit must succeed (no conflict): {:?}",
        alice_receipt.err()
    );

    // Verify Alice's value is in the shared store
    let shared_after_alice = alice_store.shared();
    let alice_value = shared_after_alice.datoms().any(|d| {
        d.entity == entity
            && d.attribute.as_str() == ":db/doc"
            && d.value == Value::String("max-retries = 3".into())
    });
    assert!(
        alice_value,
        "W5 GATE: After Alice commits, shared store must contain her value"
    );

    // Step 3: Bob commits -- should FAIL with coherence violation
    // (Alice's value already in shared, Bob's value conflicts)
    //
    // Bob's AgentStore still has the old shared snapshot. We need to give
    // Bob the updated shared store (simulating a sync/pull).
    let updated_shared = alice_store.shared().clone_store();
    let bob_working_datoms = bob_store.working().datom_set().clone();
    let mut bob_store = AgentStore::recover(updated_shared, bob_working_datoms, bob);

    let bob_result = bob_store.commit(&[entity]);
    assert!(
        bob_result.is_err(),
        "W5 GATE: Bob's conflicting commit must fail with coherence violation"
    );

    // Step 4: Open deliberation from the coherence violation
    let delib_tx = test_tx(5000, bob);
    let (delib_entity, delib_datoms) = open_deliberation(
        "max-retries config conflict",
        &[Attribute::from_keyword(":db/doc")],
        delib_tx,
    );

    // Step 5: Add positions from both agents
    let (alice_pos, alice_pos_datoms) = add_position(
        delib_entity,
        "max-retries = 3",
        "Lower retry count reduces latency and resource consumption",
        &[],
        alice,
        test_tx(5100, alice),
    );

    let (_bob_pos, bob_pos_datoms) = add_position(
        delib_entity,
        "max-retries = 5",
        "Higher retry count improves reliability under transient failures",
        &[],
        bob,
        test_tx(5200, bob),
    );

    // Transact deliberation + positions into a store for stability check
    let mut delib_store_datoms = bob_store.shared().datom_set().clone();
    for d in delib_datoms
        .iter()
        .chain(alice_pos_datoms.iter())
        .chain(bob_pos_datoms.iter())
    {
        delib_store_datoms.insert(d.clone());
    }
    let delib_store = Store::from_datoms(delib_store_datoms.clone());

    // Verify deliberation stability (should be split, not unanimous)
    let stability = check_stability(&delib_store, delib_entity);
    assert_eq!(
        stability.total_positions, 2,
        "W5 GATE: Deliberation must have exactly 2 positions"
    );
    assert!(
        !stability.is_unanimous,
        "W5 GATE: Positions are conflicting, must not be unanimous"
    );

    // Step 6: Decide in favor of Alice's position (e.g., Authority method)
    let (_decision_entity, decision_datoms) = decide(
        delib_entity,
        alice_pos,
        DecisionMethod::Authority,
        "Architecture decision: lower retries reduces blast radius",
        test_tx(5300, alice),
    );

    // Transact the decision
    for d in &decision_datoms {
        delib_store_datoms.insert(d.clone());
    }
    let final_store = Store::from_datoms(delib_store_datoms);

    // Step 7: Verify final store is consistent
    // Deliberation is Decided
    let delib_decided = final_store.entity_datoms(delib_entity).iter().any(|d| {
        d.attribute.as_str() == ":deliberation/status"
            && d.value == Value::Keyword(":deliberation.status/decided".into())
    });
    assert!(
        delib_decided,
        "W5 GATE: Deliberation must be in Decided state after decide()"
    );

    // Alice's original value persists in shared (it was committed first)
    let alice_in_shared = final_store.datoms().any(|d| {
        d.entity == entity
            && d.attribute.as_str() == ":db/doc"
            && d.value == Value::String("max-retries = 3".into())
    });
    assert!(
        alice_in_shared,
        "W5 GATE: Alice's value must persist in the shared store after deliberation"
    );

    // Shared store is monotonically growing (more datoms than before)
    assert!(
        final_store.len() > alice_store.shared().len(),
        "W5 GATE: Final store must have more datoms than after Alice's commit alone \
         (deliberation entities added)"
    );
}

/// W5 GATE: Coherence violation bridge to deliberation creates valid entities.
///
/// Simulates the exact path: coherence_violation_to_deliberation produces a
/// deliberation with two positions that can be transacted and decided.
#[test]
fn w5_gate_coherence_to_deliberation_bridge() {
    let agent = test_agent("w5-bridge");
    let tx = test_tx(6000, agent);

    // Simulate a Tier 1 coherence violation
    let violation = CoherenceViolation {
        tier: braid_kernel::coherence::CoherenceTier::Tier1Exact,
        offending_datom: Datom::new(
            EntityId::from_ident(":config/max-retries"),
            Attribute::from_keyword(":db/doc"),
            Value::String("max-retries = 5".into()),
            tx,
            Op::Assert,
        ),
        existing_context: "Existing: :db/doc = \"max-retries = 3\"".to_string(),
        description: "Tier 1 exact contradiction: different values for :db/doc on same entity"
            .to_string(),
        fix_hint: "Open deliberation or use --force to override".to_string(),
    };

    // Convert to deliberation
    let (delib_entity, datoms) = coherence_violation_to_deliberation(&violation, tx);

    // Verify structure
    assert!(
        !datoms.is_empty(),
        "W5 GATE: coherence_violation_to_deliberation must produce datoms"
    );

    // Deliberation entity has Open status
    let open_status = datoms.iter().any(|d| {
        d.entity == delib_entity
            && d.attribute.as_str() == ":deliberation/status"
            && d.value == Value::Keyword(":deliberation.status/open".into())
    });
    assert!(
        open_status,
        "W5 GATE: Auto-created deliberation must start in Open status"
    );

    // Two positions (existing + proposed)
    let position_count = datoms
        .iter()
        .filter(|d| {
            d.attribute.as_str() == ":position/deliberation" && d.value == Value::Ref(delib_entity)
        })
        .count();
    assert_eq!(
        position_count, 2,
        "W5 GATE: Must have exactly 2 positions (existing + proposed)"
    );

    // Can transact into a store
    let mut all_datoms = Store::genesis().datom_set().clone();
    for d in &datoms {
        all_datoms.insert(d.clone());
    }
    let store = Store::from_datoms(all_datoms);

    // Deliberation entity has at least 3 datoms (ident + topic + status)
    assert!(
        store.entity_datoms(delib_entity).len() >= 3,
        "W5 GATE: Deliberation entity must have ident + topic + status in store"
    );

    // Can check stability (should show 2 positions, split)
    let stability = check_stability(&store, delib_entity);
    assert_eq!(
        stability.total_positions, 2,
        "W5 GATE: Stability check must see 2 positions"
    );
    assert!(
        !stability.is_unanimous,
        "W5 GATE: Auto-created positions from violation are adversarial — not unanimous"
    );

    // Can decide
    let pos_entities: Vec<EntityId> = datoms
        .iter()
        .filter(|d| d.attribute.as_str() == ":position/deliberation")
        .map(|d| d.entity)
        .collect();
    assert!(pos_entities.len() >= 2);

    let (decision_entity, decision_datoms) = decide(
        delib_entity,
        pos_entities[0],
        DecisionMethod::Majority,
        "Existing value retained after review",
        test_tx(7000, agent),
    );

    // Decision entity references the deliberation
    let decision_refs_delib = decision_datoms.iter().any(|d| {
        d.entity == decision_entity
            && d.attribute.as_str() == ":decision/deliberation"
            && d.value == Value::Ref(delib_entity)
    });
    assert!(
        decision_refs_delib,
        "W5 GATE: Decision must reference the deliberation"
    );
}

/// W5 GATE: Working set isolation — uncommitted data invisible across agents.
///
/// This is a focused gate check for the INV-STORE-013 invariant in the
/// multi-agent context, complementing the unit tests in agent_store.rs.
#[test]
fn w5_gate_working_set_isolation() {
    let alice = test_agent("alice-iso");
    let bob = test_agent("bob-iso");
    let shared = full_schema_store();

    let mut alice_store = AgentStore::new(shared.clone_store(), alice);
    let bob_store = AgentStore::new(shared, bob);

    // Alice asserts privately
    let private_entity = EntityId::from_ident(":test/alice-secret");
    alice_store
        .assert_local(
            vec![(
                private_entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("alice's uncommitted work".into()),
            )],
            ProvenanceType::Observed,
            "private work",
        )
        .expect("Alice local assert must succeed");

    // Alice sees it
    let alice_view = alice_store.query_local();
    let alice_sees = alice_view
        .datoms()
        .any(|d| d.entity == private_entity && d.attribute.as_str() == ":db/doc");
    assert!(
        alice_sees,
        "W5 GATE: Alice must see her own uncommitted working set"
    );

    // Bob does NOT see it
    let bob_view = bob_store.query_local();
    let bob_sees = bob_view
        .datoms()
        .any(|d| d.entity == private_entity && d.attribute.as_str() == ":db/doc");
    assert!(
        !bob_sees,
        "W5 GATE (INV-STORE-013): Bob must NOT see Alice's uncommitted working set"
    );

    // Shared store does NOT have it
    let shared_has = alice_store
        .shared()
        .datoms()
        .any(|d| d.entity == private_entity && d.attribute.as_str() == ":db/doc");
    assert!(
        !shared_has,
        "W5 GATE (INV-STORE-013): Shared store must not contain uncommitted working set data"
    );
}
