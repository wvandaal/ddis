//! Branch operations — metadata-only branching over the datom set.
//!
//! A branch is NOT a separate database. It is a **filter** on the datom set:
//!
//! ```text
//! branch(name) = { d in S : d.tx has :tx/branch = name }
//! ```
//!
//! Branch entities are first-class entities in the store with `:branch/*` attributes.
//! Every transaction tagged with `:tx/branch` belongs to that branch. Untagged
//! transactions belong to all branches (they are the "trunk" or "main" baseline).
//!
//! # Invariants
//!
//! - **INV-MERGE-006**: Branch as first-class entity — branch metadata stored as datoms.
//! - **INV-MERGE-003**: Branch isolation — working sets not leaked into merge.
//! - **INV-STORE-001**: Append-only — prune marks abandoned, never deletes.
//!
//! # Design Decisions
//!
//! - ADR-MERGE-002: Branching G-Set extension for working set isolation.
//! - ADR-MERGE-006: Branch comparison via entity type (metadata in datoms).

use crate::datom::{Attribute, Datom, EntityId, Op, TxId, Value};
use crate::store::Store;

/// Create a branch entity and its metadata datoms.
///
/// Returns the branch entity ID and the datoms to transact. The caller must
/// commit these datoms via a `Transaction` — this function produces raw datoms
/// so the caller controls the transaction boundary.
///
/// # Arguments
///
/// * `name` — Unique branch name (e.g., "feature/add-query-cache").
/// * `purpose` — Human-readable rationale for why this branch exists.
/// * `tx_id` — Transaction ID under which the branch entity is created.
pub fn create_branch(name: &str, purpose: &str, tx_id: TxId) -> (EntityId, Vec<Datom>) {
    let entity = EntityId::from_ident(&format!(":branch/{name}"));
    let datoms = vec![
        Datom::new(
            entity,
            Attribute::from_keyword(":branch/name"),
            Value::String(name.to_string()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":branch/status"),
            Value::Keyword(":branch.status/active".to_string()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":branch/purpose"),
            Value::String(purpose.to_string()),
            tx_id,
            Op::Assert,
        ),
    ];
    (entity, datoms)
}

/// Return all datoms whose transaction is tagged with `:tx/branch` = `branch_name`.
///
/// This implements the fundamental branch filter:
/// `branch(name) = { d in S : d.tx has :tx/branch = name }`
///
/// The filter scans the `:tx/branch` attribute index to find matching transaction
/// IDs, then collects all datoms whose `tx` matches.
pub fn branch_datoms<'a>(store: &'a Store, branch_name: &str) -> Vec<&'a Datom> {
    // Step 1: find all TxIds tagged with this branch name
    let branch_attr = Attribute::from_keyword(":tx/branch");
    let branch_value = Value::String(branch_name.to_string());

    let branch_tx_ids: Vec<TxId> = store
        .attribute_datoms(&branch_attr)
        .iter()
        .filter(|d| d.value == branch_value && d.op == Op::Assert)
        .map(|d| d.tx)
        .collect();

    // Step 2: collect all datoms with a matching TxId
    store
        .datoms()
        .filter(|d| branch_tx_ids.contains(&d.tx))
        .collect()
}

/// Merge a source branch into a target branch by producing datoms that
/// re-assert the source branch's datoms under the target branch.
///
/// Since branches are filters over the same datom set, "merge" means:
/// produce new assertion datoms that logically associate source-branch
/// content with the target branch. The caller transacts these under a
/// transaction tagged with `:tx/branch` = `target`.
///
/// Also produces a retraction marking the source branch as `:branch.status/merged`.
///
/// Returns the datoms to transact. The caller controls the transaction.
pub fn merge_branch(store: &Store, source: &str, target: &str, tx_id: TxId) -> Vec<Datom> {
    let mut result = Vec::new();

    // Collect source-branch datoms and re-assert them.
    // The caller will transact these under a tx tagged with :tx/branch = target.
    let source_datoms = branch_datoms(store, source);
    for d in source_datoms {
        // Skip transaction-specific and branch-management datoms.
        // User content (including :db/doc on user entities) is preserved.
        if d.attribute.as_str().starts_with(":tx/") || d.attribute.as_str().starts_with(":branch/")
        {
            continue;
        }
        result.push(Datom::new(
            d.entity,
            d.attribute.clone(),
            d.value.clone(),
            tx_id,
            Op::Assert,
        ));
    }

    // Tag the merge transaction with the target branch
    result.push(Datom::new(
        EntityId::from_ident(":tx/self"),
        Attribute::from_keyword(":tx/branch"),
        Value::String(target.to_string()),
        tx_id,
        Op::Assert,
    ));

    // Mark source branch as merged (append-only status change)
    let source_entity = EntityId::from_ident(&format!(":branch/{source}"));
    result.push(Datom::new(
        source_entity,
        Attribute::from_keyword(":branch/status"),
        Value::Keyword(":branch.status/merged".to_string()),
        tx_id,
        Op::Assert,
    ));

    result
}

/// Compare two branches, returning datoms unique to each.
///
/// Returns `(only_in_a, only_in_b)` — the symmetric difference of the two
/// branch datom sets. Datoms present in both branches are excluded.
///
/// Comparison is by content identity (entity, attribute, value, op) — the
/// `tx` field is ignored since the same logical datom may appear under
/// different transaction IDs in different branches.
pub fn compare_branches<'a>(
    store: &'a Store,
    a: &str,
    b: &str,
) -> (Vec<&'a Datom>, Vec<&'a Datom>) {
    let datoms_a = branch_datoms(store, a);
    let datoms_b = branch_datoms(store, b);

    // Content key: (entity, attribute, value, op) — ignoring tx
    let content_key = |d: &&Datom| -> (EntityId, String, String, Op) {
        (
            d.entity,
            d.attribute.as_str().to_string(),
            format!("{:?}", d.value),
            d.op,
        )
    };

    let keys_a: std::collections::HashSet<_> = datoms_a.iter().map(content_key).collect();
    let keys_b: std::collections::HashSet<_> = datoms_b.iter().map(content_key).collect();

    let only_a = datoms_a
        .into_iter()
        .filter(|d| !keys_b.contains(&content_key(d)))
        .collect();
    let only_b = datoms_b
        .into_iter()
        .filter(|d| !keys_a.contains(&content_key(d)))
        .collect();

    (only_a, only_b)
}

/// Prune a branch by marking it as abandoned.
///
/// Append-only: no datoms are deleted (INV-STORE-001). The branch entity
/// gains `:branch/status` = `:branch.status/abandoned`.
///
/// Returns the datoms to transact. The caller controls the transaction.
pub fn prune_branch(branch_entity: EntityId, tx_id: TxId) -> Vec<Datom> {
    vec![Datom::new(
        branch_entity,
        Attribute::from_keyword(":branch/status"),
        Value::Keyword(":branch.status/abandoned".to_string()),
        tx_id,
        Op::Assert,
    )]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// Witnesses: INV-MERGE-006, INV-MERGE-003, INV-STORE-001,
// ADR-MERGE-002, ADR-MERGE-006
#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{AgentId, ProvenanceType};
    use crate::store::Transaction;
    use std::collections::BTreeSet;

    /// Build a store with the full schema (L0 genesis + L1 + L2 + L3 + L4).
    fn store_with_full_schema() -> Store {
        let system_agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, system_agent);
        let mut datom_set = BTreeSet::new();
        for d in crate::schema::genesis_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        for d in crate::schema::full_schema_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        Store::from_datoms(datom_set)
    }

    /// Helper: transact a branch entity plus a tagged user datom.
    fn create_and_transact_branch(
        store: &mut Store,
        branch_name: &str,
        purpose: &str,
        user_entity: &str,
        user_value: &str,
    ) -> TxId {
        let agent = AgentId::from_name("test-agent");

        // First transaction: create the branch entity
        let (_, branch_datoms) = create_branch(
            branch_name,
            purpose,
            TxId::new(0, 0, agent), // placeholder, commit will replace
        );

        let mut tx_builder =
            Transaction::new(agent, ProvenanceType::Observed, "create branch entity");
        for d in &branch_datoms {
            tx_builder = tx_builder.assert(d.entity, d.attribute.clone(), d.value.clone());
        }
        let committed = tx_builder.commit(store).unwrap();
        store.transact(committed).unwrap();

        // Second transaction: user datom tagged with the branch
        let tx_builder = Transaction::new(agent, ProvenanceType::Observed, "branch work")
            .assert(
                EntityId::from_ident(user_entity),
                Attribute::from_keyword(":db/doc"),
                Value::String(user_value.to_string()),
            )
            .assert(
                EntityId::from_ident(":tx/self"),
                Attribute::from_keyword(":tx/branch"),
                Value::String(branch_name.to_string()),
            );
        let committed = tx_builder.commit(store).unwrap();
        let tx_id = committed.tx_id();
        store.transact(committed).unwrap();

        tx_id
    }

    // Verifies: INV-MERGE-006 — Branch as first-class entity
    // Verifies: ADR-MERGE-006 — Branch comparison via entity type
    #[test]
    fn create_branch_produces_valid_entity() {
        let agent = AgentId::from_name("test-agent");
        let tx = TxId::new(1, 0, agent);
        let (entity, datoms) = create_branch("feature/query-cache", "Add query caching", tx);

        // Entity is content-addressed from the branch ident
        assert_eq!(entity, EntityId::from_ident(":branch/feature/query-cache"));

        // Should produce 3 datoms: name, status, purpose
        assert_eq!(datoms.len(), 3, "branch entity needs name, status, purpose");

        let attrs: Vec<&str> = datoms.iter().map(|d| d.attribute.as_str()).collect();
        assert!(attrs.contains(&":branch/name"));
        assert!(attrs.contains(&":branch/status"));
        assert!(attrs.contains(&":branch/purpose"));

        // Status should be active
        let status_datom = datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":branch/status")
            .unwrap();
        assert_eq!(
            status_datom.value,
            Value::Keyword(":branch.status/active".to_string())
        );

        // All datoms should be assertions
        for d in &datoms {
            assert_eq!(d.op, Op::Assert);
        }
    }

    // Verifies: INV-MERGE-003 — Branch isolation
    // The branch filter returns only datoms tagged with that branch.
    #[test]
    fn branch_datoms_filters_correctly() {
        let mut store = store_with_full_schema();

        // Create two branches with different content
        create_and_transact_branch(
            &mut store,
            "branch-a",
            "Test branch A",
            ":test/entity-a",
            "value from branch A",
        );

        create_and_transact_branch(
            &mut store,
            "branch-b",
            "Test branch B",
            ":test/entity-b",
            "value from branch B",
        );

        // Filter for branch-a
        let a_datoms = branch_datoms(&store, "branch-a");
        let b_datoms = branch_datoms(&store, "branch-b");

        // branch-a should contain its user datom and the :tx/branch tag
        let a_has_entity_a = a_datoms
            .iter()
            .any(|d| d.value == Value::String("value from branch A".to_string()));
        assert!(a_has_entity_a, "branch-a must contain its own datoms");

        // branch-a should NOT contain branch-b's datoms
        let a_has_entity_b = a_datoms
            .iter()
            .any(|d| d.value == Value::String("value from branch B".to_string()));
        assert!(!a_has_entity_b, "branch-a must not contain branch-b datoms");

        // branch-b should contain its own datom
        let b_has_entity_b = b_datoms
            .iter()
            .any(|d| d.value == Value::String("value from branch B".to_string()));
        assert!(b_has_entity_b, "branch-b must contain its own datoms");
    }

    // Verifies: INV-MERGE-001 — Merge is set union
    // Branch merge re-asserts source datoms under the target branch.
    #[test]
    fn merge_branch_is_set_union() {
        let mut store = store_with_full_schema();

        // Create source branch with content
        create_and_transact_branch(
            &mut store,
            "feature",
            "Feature branch",
            ":test/feature-work",
            "feature implementation",
        );

        let agent = AgentId::from_name("test-agent");
        let merge_tx = TxId::new(99, 0, agent);

        let merge_datoms = merge_branch(&store, "feature", "main", merge_tx);

        // Should contain at least the re-asserted user datom + status change
        assert!(
            !merge_datoms.is_empty(),
            "merge must produce datoms for re-assertion"
        );

        // The status change should mark source as merged
        let status_change = merge_datoms.iter().find(|d| {
            d.attribute.as_str() == ":branch/status"
                && d.value == Value::Keyword(":branch.status/merged".to_string())
        });
        assert!(
            status_change.is_some(),
            "merge must mark source branch as merged"
        );

        // The re-asserted datom should preserve the user content
        let has_feature_content = merge_datoms.iter().any(|d| {
            d.value == Value::String("feature implementation".to_string())
                && d.attribute.as_str() == ":db/doc"
        });
        assert!(
            has_feature_content,
            "merge must re-assert source branch content"
        );
    }

    // Verifies: ADR-MERGE-006 — Branch comparison via entity type
    // Compare shows the symmetric difference between branches.
    #[test]
    fn compare_shows_symmetric_difference() {
        let mut store = store_with_full_schema();

        // Two branches with different content
        create_and_transact_branch(
            &mut store,
            "alpha",
            "Alpha branch",
            ":test/alpha-work",
            "alpha content",
        );

        create_and_transact_branch(
            &mut store,
            "beta",
            "Beta branch",
            ":test/beta-work",
            "beta content",
        );

        let (only_alpha, only_beta) = compare_branches(&store, "alpha", "beta");

        // alpha-only should contain alpha's content, not beta's
        let alpha_has_own = only_alpha
            .iter()
            .any(|d| d.value == Value::String("alpha content".to_string()));
        assert!(alpha_has_own, "only_alpha must contain alpha's content");

        let alpha_has_beta = only_alpha
            .iter()
            .any(|d| d.value == Value::String("beta content".to_string()));
        assert!(
            !alpha_has_beta,
            "only_alpha must not contain beta's content"
        );

        // beta-only should contain beta's content
        let beta_has_own = only_beta
            .iter()
            .any(|d| d.value == Value::String("beta content".to_string()));
        assert!(beta_has_own, "only_beta must contain beta's content");
    }

    // Verifies: INV-STORE-001 — Append-only immutability
    // Prune marks abandoned — never deletes the branch or its datoms.
    #[test]
    fn prune_marks_abandoned_append_only() {
        let agent = AgentId::from_name("test-agent");
        let tx = TxId::new(1, 0, agent);
        let branch_entity = EntityId::from_ident(":branch/old-experiment");

        let datoms = prune_branch(branch_entity, tx);

        assert_eq!(datoms.len(), 1, "prune produces exactly one datom");

        let d = &datoms[0];
        assert_eq!(d.entity, branch_entity);
        assert_eq!(d.attribute.as_str(), ":branch/status");
        assert_eq!(
            d.value,
            Value::Keyword(":branch.status/abandoned".to_string())
        );
        assert_eq!(d.op, Op::Assert, "prune is an assertion, not a retraction");
    }

    // Verifies: INV-STORE-001 — Store never shrinks after branch operations
    #[test]
    fn branch_operations_never_shrink_store() {
        let mut store = store_with_full_schema();
        let initial_count = store.datom_set().len();

        // Create a branch
        create_and_transact_branch(
            &mut store,
            "temp",
            "Temporary branch",
            ":test/temp-work",
            "temp content",
        );
        let after_create = store.datom_set().len();
        assert!(
            after_create > initial_count,
            "creating a branch must grow the store"
        );

        // Prune the branch
        let agent = AgentId::from_name("test-agent");
        let prune_datoms = prune_branch(
            EntityId::from_ident(":branch/temp"),
            TxId::new(0, 0, agent), // placeholder
        );
        let tx = Transaction::new(agent, ProvenanceType::Observed, "prune branch");
        let mut tx_builder = tx;
        for d in &prune_datoms {
            tx_builder = tx_builder.assert(d.entity, d.attribute.clone(), d.value.clone());
        }
        let committed = tx_builder.commit(&store).unwrap();
        store.transact(committed).unwrap();

        let after_prune = store.datom_set().len();
        assert!(
            after_prune >= after_create,
            "pruning must not shrink the store (append-only)"
        );
    }
}
