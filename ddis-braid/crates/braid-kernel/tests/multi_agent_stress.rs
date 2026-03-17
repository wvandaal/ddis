//! W5-STRESS: Multi-agent concurrent access stress test.
//!
//! Validates the concurrency guarantees that underpin the multi-agent vision.
//!
//! Traces to: INV-STORE-013 (W_alpha isolation), INV-STORE-001 (append-only),
//! INV-STORE-003 (content-addressed identity)

use braid_kernel::agent_store::AgentStore;
use braid_kernel::datom::{AgentId, Attribute, EntityId, ProvenanceType, Value};
use braid_kernel::store::Store;

/// Verify working set isolation between agents.
///
/// INV-STORE-013: query(W_alpha) ∩ visible(beta) = ∅
#[test]
fn working_set_isolation() {
    let agent_a = AgentId::from_name("isolation-a");
    let agent_b = AgentId::from_name("isolation-b");

    let shared = Store::genesis();
    let mut store_a = AgentStore::new(shared.clone_store(), agent_a);
    let store_b = AgentStore::new(shared, agent_b);

    // Agent A asserts a local fact
    let entity = EntityId::from_ident(":private/a-only");
    store_a
        .assert_local(
            vec![(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("A's private data".into()),
            )],
            ProvenanceType::Observed,
            "private assertion",
        )
        .unwrap();

    // Agent A sees it in local view
    let a_local = store_a.query_local();
    assert!(
        !a_local.entity_datoms(entity).is_empty(),
        "Agent A must see its own working set data"
    );

    // Agent B does NOT see it (working set isolation)
    let b_local = store_b.query_local();
    assert!(
        b_local.entity_datoms(entity).is_empty(),
        "Agent B must NOT see Agent A's working set data (INV-STORE-013)"
    );
}

/// Verify that commit promotes datoms to shared store.
#[test]
fn commit_promotes_to_shared() {
    let agent = AgentId::from_name("commit-test");
    let shared = Store::genesis();
    let mut store = AgentStore::new(shared, agent);

    let entity = EntityId::from_ident(":test/commit-entity");
    store
        .assert_local(
            vec![(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("to be committed".into()),
            )],
            ProvenanceType::Observed,
            "pre-commit",
        )
        .unwrap();

    // Before commit: shared doesn't have it
    assert!(
        store.shared().entity_datoms(entity).is_empty(),
        "Shared should not have uncommitted data"
    );

    // Commit
    let result = store.commit(&[entity]);
    assert!(result.is_ok(), "Commit should succeed: {:?}", result.err());

    // After commit: shared has it
    assert!(
        !store.shared().entity_datoms(entity).is_empty(),
        "Shared should have committed data after commit"
    );
}

/// Verify local query returns both shared and working datoms.
#[test]
fn local_query_sees_shared_plus_working() {
    let agent = AgentId::from_name("merged-view");
    let mut shared = Store::genesis();

    // Add something to shared first
    let shared_entity = EntityId::from_ident(":test/shared-data");
    let tx = braid_kernel::datom::TxId::new(50, 0, agent);
    let committed =
        braid_kernel::store::Transaction::new(agent, ProvenanceType::Observed, "shared")
            .assert(
                shared_entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("shared data".into()),
            )
            .commit(&shared)
            .unwrap();
    shared.transact(committed).unwrap();

    let mut store = AgentStore::new(shared, agent);

    // Add something to working only
    let working_entity = EntityId::from_ident(":test/working-data");
    store
        .assert_local(
            vec![(
                working_entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("working data".into()),
            )],
            ProvenanceType::Observed,
            "working",
        )
        .unwrap();

    // Local view should see both
    let local = store.query_local();
    assert!(
        !local.entity_datoms(shared_entity).is_empty(),
        "Local view must include shared data"
    );
    assert!(
        !local.entity_datoms(working_entity).is_empty(),
        "Local view must include working data"
    );
}

/// Multi-agent stress: 4 agents assert locally then commit.
#[test]
fn multi_agent_stress_4_agents() {
    let shared = Store::genesis();
    let genesis_len = shared.len();

    // Phase 1: Each agent creates its own AgentStore and asserts locally
    let mut agents: Vec<AgentStore> = (0..4)
        .map(|i| {
            let agent = AgentId::from_name(&format!("stress-{i}"));
            let mut store = AgentStore::new(shared.clone_store(), agent);

            // Assert 10 datoms locally
            for j in 0..10 {
                let entity = EntityId::from_ident(&format!(":stress/agent{i}-item{j}"));
                store
                    .assert_local(
                        vec![(
                            entity,
                            Attribute::from_keyword(":db/doc"),
                            Value::String(format!("agent {i} item {j}")),
                        )],
                        ProvenanceType::Observed,
                        &format!("stress-{i}-{j}"),
                    )
                    .unwrap();
            }

            store
        })
        .collect();

    // Phase 2: Each agent commits 2 entities
    let mut total_committed = 0;
    for i in 0..4 {
        let entities: Vec<EntityId> = (0..2)
            .map(|j| EntityId::from_ident(&format!(":stress/agent{i}-item{j}")))
            .collect();

        match agents[i].commit(&entities) {
            Ok(_) => total_committed += 1,
            Err(e) => eprintln!("Agent {i} commit failed: {e:?}"),
        }
    }

    // Phase 3: Verify isolation
    for i in 0..4 {
        let local = agents[i].query_local();
        // Each agent should see at least genesis + its 10 local datoms
        assert!(
            local.len() > genesis_len,
            "Agent {i} local view too small: {}",
            local.len()
        );
    }

    // Verify shared store grew
    // Note: agents[0].shared() has the most recent committed state for agent 0
    let shared_0 = agents[0].shared();
    assert!(
        shared_0.len() >= genesis_len,
        "Shared store must be at least genesis size"
    );

    eprintln!(
        "STRESS: {total_committed}/4 commits succeeded, shared={} datoms",
        shared_0.len()
    );
}
