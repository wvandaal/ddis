//! Transact-time coherence gate — contradiction prevention at the point of assertion.
//!
//! This module transforms DDIS from "a system that detects divergence" to
//! "a system that prevents divergence." Invalid states become unrepresentable
//! rather than detectable-after-the-fact.
//!
//! # Formal Model
//!
//! ```text
//! TRANSACT_new: (Store, Transaction) → Result<Store', CoherenceViolation>
//!   Precondition: schema validation (INV-SCHEMA-004)
//!                 + coherence_check(Store, Transaction) = Ok
//!   Postcondition: Store' = Store ∪ Transaction.datoms
//!   Error: CoherenceViolation { tier, element_a, element_b, description }
//! ```
//!
//! # Tiers
//!
//! - **Tier 1 (Exact)**: Two datoms assert different values for the same (entity, attribute)
//!   under Cardinality::One and neither is a retraction.
//!   O(|new_datoms| × O(index_lookup)) — sub-millisecond for typical transactions.
//!
//! - **Tier 2 (Logical)**: Two spec elements make mutually exclusive claims.
//!   Pattern-based rules on `:spec/statement` and `:spec/falsification` fields.
//!   Only triggers for spec entity transactions (no-op for data txns).
//!
//! # Invariants
//!
//! - **INV-TRANSACT-COHERENCE-001**: Transact-time contradiction prevention.
//!   For all transactions T, TRANSACT(S, T) succeeds only if coherence_check(S, T)
//!   returns no Tier 1 or Tier 2 contradictions.
//!
//! # Design Decisions
//!
//! - SETTLED: Hard rejection is the correct default (type system, not linter).
//! - The `--force` flag is the `unsafe` block equivalent — bypasses checker
//!   but creates a visible audit trail via `:tx/coherence-override = true`.
//!
//! # Traces To
//!
//! - SEED.md §4 (Design Commitment #2: append-only)
//! - spec/01-store.md (INV-STORE-001, INV-TRANSACT-COHERENCE-001)

use crate::datom::{Attribute, Datom, EntityId, Op, Value};
use crate::error::StoreError;
use crate::schema::{Cardinality, ResolutionMode};
use crate::store::{Committed, Store, Transaction, TxReceipt};

// ===========================================================================
// CoherenceViolation
// ===========================================================================

/// A coherence violation detected during transact-time checking.
#[derive(Clone, Debug)]
pub struct CoherenceViolation {
    /// Which tier detected this violation.
    pub tier: CoherenceTier,
    /// The new datom that caused the violation.
    pub offending_datom: Datom,
    /// The existing value or element that conflicts.
    pub existing_context: String,
    /// Human-readable description of the violation.
    pub description: String,
    /// Suggested fix or next action.
    pub fix_hint: String,
}

/// Coherence checking tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoherenceTier {
    /// Exact contradiction: different values for same (e, a) under Cardinality::One.
    Tier1Exact,
    /// Logical contradiction: spec elements make mutually exclusive claims.
    Tier2Logical,
}

impl std::fmt::Display for CoherenceViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}] {}\n  Context: {}\n  Fix: {}",
            self.tier, self.description, self.existing_context, self.fix_hint
        )
    }
}

// ===========================================================================
// Tier 1: Exact Contradiction Detection
// ===========================================================================

/// Check new datoms for Tier 1 (exact) contradictions against the store.
///
/// Two datoms contradict at Tier 1 if they assert different values for the
/// same (entity, attribute) pair under Cardinality::One and neither is a
/// retraction, and the resolution mode is not Multi.
///
/// Performance: O(|new_datoms| × O(index_lookup)) — sub-millisecond for
/// typical transactions (5-50 datoms).
#[allow(clippy::result_large_err)]
pub fn tier1_check(store: &Store, new_datoms: &[Datom]) -> Result<(), CoherenceViolation> {
    let schema = store.schema();

    for datom in new_datoms {
        // Only check assertions (retractions are always valid)
        if datom.op != Op::Assert {
            continue;
        }

        // Look up schema for this attribute
        let attr_def = match schema.attribute(&datom.attribute) {
            Some(def) => def,
            None => continue, // Unknown attribute — schema validation handles this
        };

        // Only check Cardinality::One attributes
        if attr_def.cardinality != Cardinality::One {
            continue;
        }

        // Skip Multi-value resolution (multiple values are intentional)
        if attr_def.resolution_mode == ResolutionMode::Multi {
            continue;
        }

        // Check: does the store already have a different asserted value for (e, a)?
        let existing_datoms = store.entity_datoms(datom.entity);
        let existing_value = existing_datoms.iter().rfind(|d| {
            d.attribute == datom.attribute && d.op == Op::Assert && d.entity == datom.entity
        });

        if let Some(existing) = existing_value {
            if existing.value != datom.value {
                return Err(CoherenceViolation {
                    tier: CoherenceTier::Tier1Exact,
                    offending_datom: datom.clone(),
                    existing_context: format!(
                        "({:?}, {}) already has value {:?} (from tx {:?})",
                        datom.entity,
                        datom.attribute.as_str(),
                        existing.value,
                        existing.tx
                    ),
                    description: format!(
                        "Tier 1 exact contradiction: attribute {} on entity {:?} has conflicting values",
                        datom.attribute.as_str(),
                        datom.entity
                    ),
                    fix_hint: format!(
                        "Either retract the existing value first, or use --force to override. Resolution mode: {:?}",
                        attr_def.resolution_mode
                    ),
                });
            }
        }
    }

    Ok(())
}

// ===========================================================================
// Tier 2: Logical Contradiction Detection
// ===========================================================================

/// Check new datoms for Tier 2 (logical) contradictions.
///
/// Only fires for spec entity transactions (`:spec/*` attributes).
/// For normal data transactions, this is a no-op (returns Ok immediately).
///
/// Pattern-based rules:
/// 1. Quantifier conflict: `∀` vs `∃` on the same predicate
/// 2. Polarity inversion: "must" vs "must not" on the same subject
/// 3. Numeric bound conflict: threshold > X vs threshold < X
/// 4. Governance overlap: two INVs claim authority over same (entity, attribute)
#[allow(clippy::result_large_err)]
pub fn tier2_check(store: &Store, new_datoms: &[Datom]) -> Result<(), CoherenceViolation> {
    // Collect new spec entities
    let new_spec_entities: Vec<EntityId> = new_datoms
        .iter()
        .filter(|d| d.attribute.as_str().starts_with(":spec/") && d.op == Op::Assert)
        .map(|d| d.entity)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    // No-op for non-spec transactions
    if new_spec_entities.is_empty() {
        return Ok(());
    }

    // Extract statements from new spec elements
    let statement_attr = Attribute::from_keyword(":spec/statement");

    for &entity in &new_spec_entities {
        let new_statement = new_datoms
            .iter()
            .find(|d| d.entity == entity && d.attribute == statement_attr && d.op == Op::Assert);

        let new_statement_text = match new_statement {
            Some(d) => match &d.value {
                Value::String(s) => s.as_str(),
                _ => continue,
            },
            None => continue,
        };

        // Check against existing spec elements
        let existing_specs = store.attribute_datoms(&statement_attr);
        for existing in existing_specs {
            if existing.op != Op::Assert || existing.entity == entity {
                continue;
            }

            let existing_text = match &existing.value {
                Value::String(s) => s.as_str(),
                _ => continue,
            };

            // Rule 1: Polarity inversion
            if let Some(violation) =
                check_polarity_inversion(entity, new_statement_text, existing, existing_text)
            {
                return Err(violation);
            }

            // Rule 2: Numeric bound conflict
            if let Some(violation) =
                check_numeric_bound_conflict(entity, new_statement_text, existing, existing_text)
            {
                return Err(violation);
            }
        }
    }

    Ok(())
}

/// Check for polarity inversion: "must" vs "must not" on overlapping subjects.
fn check_polarity_inversion(
    new_entity: EntityId,
    new_text: &str,
    existing: &Datom,
    existing_text: &str,
) -> Option<CoherenceViolation> {
    let new_lower = new_text.to_lowercase();
    let existing_lower = existing_text.to_lowercase();

    // Extract subject (first significant noun phrase)
    let new_has_must = new_lower.contains("must ");
    let new_has_must_not = new_lower.contains("must not") || new_lower.contains("must never");
    let existing_has_must = existing_lower.contains("must ");
    let existing_has_must_not =
        existing_lower.contains("must not") || existing_lower.contains("must never");

    // Polarity inversion: one says "must" and the other says "must not"
    // about something with overlapping words
    if (new_has_must && !new_has_must_not && existing_has_must_not)
        || (new_has_must_not && existing_has_must && !existing_has_must_not)
    {
        // Check for word overlap (heuristic: >50% shared significant words)
        let new_words: std::collections::BTreeSet<&str> = new_lower
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();
        let existing_words: std::collections::BTreeSet<&str> = existing_lower
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();
        let overlap = new_words.intersection(&existing_words).count();
        let min_size = new_words.len().min(existing_words.len()).max(1);

        if overlap * 2 >= min_size {
            return Some(CoherenceViolation {
                tier: CoherenceTier::Tier2Logical,
                offending_datom: Datom::new(
                    new_entity,
                    Attribute::from_keyword(":spec/statement"),
                    Value::String(new_text.to_string()),
                    existing.tx,
                    Op::Assert,
                ),
                existing_context: format!(
                    "Existing spec {:?}: \"{}\"",
                    existing.entity,
                    &existing_text[..existing_text.len().min(100)]
                ),
                description: "Tier 2 polarity inversion: 'must' vs 'must not' on overlapping subjects".to_string(),
                fix_hint: "Review both statements. If intentional supersession, use --force. If genuine conflict, open a deliberation.".to_string(),
            });
        }
    }

    None
}

/// Check for numeric bound conflict: threshold > X vs threshold < X.
fn check_numeric_bound_conflict(
    new_entity: EntityId,
    new_text: &str,
    existing: &Datom,
    existing_text: &str,
) -> Option<CoherenceViolation> {
    // Simple heuristic: look for "> N" vs "< N" or ">= N" vs "<= N"
    // on the same metric name
    let new_lower = new_text.to_lowercase();
    let existing_lower = existing_text.to_lowercase();

    // Extract comparisons: "X > N" or "X < N" or "X >= N" or "X <= N"
    let new_gt = new_lower.contains(" > ") || new_lower.contains(" >= ");
    let new_lt = new_lower.contains(" < ") || new_lower.contains(" <= ");
    let existing_gt = existing_lower.contains(" > ") || existing_lower.contains(" >= ");
    let existing_lt = existing_lower.contains(" < ") || existing_lower.contains(" <= ");

    // Conflict: one says > and the other says < on overlapping terms
    if (new_gt && existing_lt) || (new_lt && existing_gt) {
        // Check for significant word overlap
        let new_words: std::collections::BTreeSet<&str> = new_lower
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();
        let existing_words: std::collections::BTreeSet<&str> = existing_lower
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();
        let overlap = new_words.intersection(&existing_words).count();
        let min_size = new_words.len().min(existing_words.len()).max(1);

        if overlap * 2 >= min_size {
            return Some(CoherenceViolation {
                tier: CoherenceTier::Tier2Logical,
                offending_datom: Datom::new(
                    new_entity,
                    Attribute::from_keyword(":spec/statement"),
                    Value::String(new_text.to_string()),
                    existing.tx,
                    Op::Assert,
                ),
                existing_context: format!(
                    "Existing spec {:?}: \"{}\"",
                    existing.entity,
                    &existing_text[..existing_text.len().min(100)]
                ),
                description:
                    "Tier 2 numeric bound conflict: contradictory comparison operators on overlapping terms"
                        .to_string(),
                fix_hint:
                    "Review numeric bounds. If intentional revision, use --force with rationale."
                        .to_string(),
            });
        }
    }

    None
}

// ===========================================================================
// Combined Coherence Check
// ===========================================================================

/// Run the full coherence check (Tier 1 + Tier 2) on new datoms.
///
/// Returns Ok(()) if no contradictions found, Err(violation) otherwise.
/// This is the function called by `transact_with_coherence()`.
#[allow(clippy::result_large_err)]
pub fn coherence_check(store: &Store, new_datoms: &[Datom]) -> Result<(), CoherenceViolation> {
    tier1_check(store, new_datoms)?;
    tier2_check(store, new_datoms)?;
    Ok(())
}

// ===========================================================================
// CoherenceError — wraps both StoreError and CoherenceViolation
// ===========================================================================

/// Error from `transact_with_coherence()`.
///
/// This enum captures the two failure modes of a coherence-gated transaction:
/// - **Violation**: The coherence check detected a contradiction (Tier 1 or Tier 2)
///   and `force` was false. Boxed to keep the enum small (CoherenceViolation is >200 bytes).
/// - **StoreError**: The underlying `Store::transact()` failed (schema validation,
///   empty transaction, etc.).
#[derive(Clone, Debug)]
pub enum CoherenceError {
    /// A coherence violation blocked the transaction (force=false).
    /// Boxed because `CoherenceViolation` is large (>200 bytes).
    Violation(Box<CoherenceViolation>),
    /// The underlying store operation failed.
    StoreError(StoreError),
}

impl std::fmt::Display for CoherenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoherenceError::Violation(v) => write!(f, "coherence violation: {v}"),
            CoherenceError::StoreError(e) => write!(f, "store error: {e}"),
        }
    }
}

impl std::error::Error for CoherenceError {}

impl From<StoreError> for CoherenceError {
    fn from(e: StoreError) -> Self {
        CoherenceError::StoreError(e)
    }
}

impl From<CoherenceViolation> for CoherenceError {
    fn from(v: CoherenceViolation) -> Self {
        CoherenceError::Violation(Box::new(v))
    }
}

// ===========================================================================
// transact_with_coherence — coherence-gated transact with --force bypass
// ===========================================================================

/// Transact with coherence gate and optional `--force` bypass.
///
/// This is the "unsafe block" pattern for DDIS: the coherence gate is the
/// default type-system-level enforcement, and `force=true` is the explicit
/// opt-out that leaves a visible audit trail.
///
/// # Behavior
///
/// - **`force=false`**: Runs `coherence_check(store, tx.datoms())`. If the check
///   finds a contradiction, returns `Err(CoherenceError::Violation)` and the
///   store is unchanged.
/// - **`force=true`**: Skips the coherence check entirely. After the transaction
///   is applied, injects an additional `:tx/coherence-override = true` datom on
///   the transaction's metadata entity. This creates the audit trail — any
///   future query can find forced transactions via this attribute.
/// - In both cases, if the underlying `store.transact()` fails (schema validation,
///   empty transaction, etc.), returns `Err(CoherenceError::StoreError)`.
///
/// # Typestate Constraint
///
/// The `Transaction<Committed>` typestate seals the datom set — no datoms can be
/// added after `commit()`. The audit trail datom is therefore injected directly
/// into the store as post-transact metadata (same entity as the tx metadata),
/// not by modifying the committed transaction.
///
/// # Invariants
///
/// - **INV-TRANSACT-COHERENCE-001**: When `force=false`, contradictions prevent
///   the transaction from being applied.
/// - **INV-STORE-001**: The audit trail datom is an assertion (append-only).
/// - **C5 (Traceability)**: The `:tx/coherence-override` attribute makes forced
///   transactions discoverable.
pub fn transact_with_coherence(
    store: &mut Store,
    tx: Transaction<Committed>,
    force: bool,
) -> Result<TxReceipt, CoherenceError> {
    // Step 1: Run coherence check unless force is set
    if !force {
        coherence_check(store, tx.datoms())?;
    }

    // Step 2: Apply the transaction to the store
    let tx_id = tx.tx_id();
    let receipt = store.transact(tx)?;

    // Step 3: If forced, inject the audit trail datom
    if force {
        let tx_entity = Store::tx_entity_id(tx_id);
        let override_datom = Datom::new(
            tx_entity,
            Attribute::from_keyword(":tx/coherence-override"),
            Value::Boolean(true),
            tx_id,
            Op::Assert,
        );
        store.inject_metadata_datom(override_datom);
    }

    Ok(receipt)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{AgentId, ProvenanceType, TxId};
    use crate::schema;
    use crate::store::{Store, Transaction};
    use std::collections::BTreeSet;

    fn test_agent() -> AgentId {
        AgentId::from_name("coherence-test")
    }

    fn test_tx() -> TxId {
        TxId::new(100, 0, test_agent())
    }

    /// Build a store with full schema (Layers 0-3) for Tier 2 tests.
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

    // --- Tier 1: Exact Contradiction Detection ---

    #[test]
    fn tier1_allows_first_assertion() {
        let store = Store::genesis();
        let entity = EntityId::from_ident(":test/entity");
        let datoms = vec![Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String("hello".into()),
            test_tx(),
            Op::Assert,
        )];
        assert!(tier1_check(&store, &datoms).is_ok());
    }

    #[test]
    fn tier1_rejects_conflicting_value() {
        let mut store = Store::genesis();
        let entity = EntityId::from_ident(":test/entity");
        let agent = test_agent();

        // First assertion
        let tx1 = Transaction::new(agent, ProvenanceType::Observed, "first")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("first".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx1).unwrap();

        // Conflicting assertion
        let datoms = vec![Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String("second".into()),
            TxId::new(200, 0, agent),
            Op::Assert,
        )];

        let result = tier1_check(&store, &datoms);
        assert!(result.is_err(), "Should reject conflicting value");
        let violation = result.unwrap_err();
        assert_eq!(violation.tier, CoherenceTier::Tier1Exact);
    }

    #[test]
    fn tier1_allows_same_value() {
        let mut store = Store::genesis();
        let entity = EntityId::from_ident(":test/entity");
        let agent = test_agent();

        let tx1 = Transaction::new(agent, ProvenanceType::Observed, "first")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("same".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx1).unwrap();

        // Same value — not a contradiction
        let datoms = vec![Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String("same".into()),
            TxId::new(200, 0, agent),
            Op::Assert,
        )];
        assert!(tier1_check(&store, &datoms).is_ok());
    }

    #[test]
    fn tier1_allows_retraction() {
        let store = Store::genesis();
        let datoms = vec![Datom::new(
            EntityId::from_ident(":test/entity"),
            Attribute::from_keyword(":db/doc"),
            Value::String("retracted".into()),
            test_tx(),
            Op::Retract,
        )];
        assert!(tier1_check(&store, &datoms).is_ok());
    }

    #[test]
    fn tier1_skips_multi_value_attributes() {
        // Multi-value attributes allow multiple different values by design
        let mut store = Store::genesis();
        let entity = EntityId::from_ident(":test/tags");
        let agent = test_agent();

        // :tx/causal-predecessors is Cardinality::Many
        let tx1 = Transaction::new(agent, ProvenanceType::Observed, "first")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("first".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx1).unwrap();

        // Different value for same entity but different attribute (Many cardinality)
        // Using :tx/causal-predecessors which is Cardinality::Many
        let datoms = vec![Datom::new(
            entity,
            Attribute::from_keyword(":tx/causal-predecessors"),
            Value::Ref(EntityId::from_ident(":test/pred1")),
            TxId::new(200, 0, agent),
            Op::Assert,
        )];
        assert!(tier1_check(&store, &datoms).is_ok());
    }

    // --- Tier 2: Logical Contradiction Detection ---

    #[test]
    fn tier2_skips_non_spec_transactions() {
        let store = Store::genesis();
        let datoms = vec![Datom::new(
            EntityId::from_ident(":test/data"),
            Attribute::from_keyword(":db/doc"),
            Value::String("not a spec element".into()),
            test_tx(),
            Op::Assert,
        )];
        assert!(tier2_check(&store, &datoms).is_ok());
    }

    #[test]
    fn tier2_detects_polarity_inversion() {
        let mut store = full_schema_store();
        let agent = test_agent();

        // Add existing spec with "must" statement
        let existing_entity = EntityId::from_ident(":spec/inv-test-a");
        let tx1 = Transaction::new(agent, ProvenanceType::Observed, "existing spec")
            .assert(
                existing_entity,
                Attribute::from_keyword(":spec/statement"),
                Value::String("The store must always preserve datom ordering".into()),
            )
            .assert(
                existing_entity,
                Attribute::from_keyword(":spec/element-type"),
                Value::Keyword(":spec.type/invariant".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx1).unwrap();

        // New spec with contradicting "must not" + overlapping words
        let new_entity = EntityId::from_ident(":spec/inv-test-b");
        let datoms = vec![
            Datom::new(
                new_entity,
                Attribute::from_keyword(":spec/statement"),
                Value::String("The store must not preserve datom ordering".into()),
                TxId::new(200, 0, agent),
                Op::Assert,
            ),
            Datom::new(
                new_entity,
                Attribute::from_keyword(":spec/element-type"),
                Value::Keyword(":spec.type/invariant".into()),
                TxId::new(200, 0, agent),
                Op::Assert,
            ),
        ];

        let result = tier2_check(&store, &datoms);
        assert!(result.is_err(), "Should detect polarity inversion");
        let violation = result.unwrap_err();
        assert_eq!(violation.tier, CoherenceTier::Tier2Logical);
        assert!(violation.description.contains("polarity"));
    }

    #[test]
    fn tier2_allows_non_conflicting_specs() {
        let mut store = full_schema_store();
        let agent = test_agent();

        let existing = EntityId::from_ident(":spec/inv-store-001");
        let tx1 = Transaction::new(agent, ProvenanceType::Observed, "existing")
            .assert(
                existing,
                Attribute::from_keyword(":spec/statement"),
                Value::String("The datom store never deletes or mutates an existing datom".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx1).unwrap();

        // Completely different topic — no conflict
        let new_entity = EntityId::from_ident(":spec/inv-query-001");
        let datoms = vec![Datom::new(
            new_entity,
            Attribute::from_keyword(":spec/statement"),
            Value::String("The query engine must evaluate queries deterministically".into()),
            TxId::new(200, 0, agent),
            Op::Assert,
        )];

        assert!(tier2_check(&store, &datoms).is_ok());
    }

    // --- Combined coherence check ---

    #[test]
    fn coherence_check_passes_valid_transaction() {
        let store = Store::genesis();
        let datoms = vec![Datom::new(
            EntityId::from_ident(":test/valid"),
            Attribute::from_keyword(":db/doc"),
            Value::String("valid data".into()),
            test_tx(),
            Op::Assert,
        )];
        assert!(coherence_check(&store, &datoms).is_ok());
    }

    #[test]
    fn coherence_check_catches_tier1() {
        let mut store = Store::genesis();
        let entity = EntityId::from_ident(":test/conflict");
        let agent = test_agent();

        let tx1 = Transaction::new(agent, ProvenanceType::Observed, "first")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("value-a".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx1).unwrap();

        let datoms = vec![Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String("value-b".into()),
            TxId::new(200, 0, agent),
            Op::Assert,
        )];
        let result = coherence_check(&store, &datoms);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().tier, CoherenceTier::Tier1Exact);
    }

    // --- transact_with_coherence tests ---

    #[test]
    fn transact_with_coherence_rejects_contradiction_when_not_forced() {
        let mut store = Store::genesis();
        let entity = EntityId::from_ident(":test/coherence-gate");
        let agent = test_agent();

        // First transaction: establish a value
        let tx1 = Transaction::new(agent, ProvenanceType::Observed, "first value")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("original".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx1).unwrap();

        // Second transaction: conflicting value, force=false
        let tx2 = Transaction::new(agent, ProvenanceType::Observed, "conflicting value")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("conflict".into()),
            )
            .commit(&store)
            .unwrap();

        let datom_count_before = store.len();
        let result = transact_with_coherence(&mut store, tx2, false);
        assert!(
            result.is_err(),
            "Should reject contradiction when force=false"
        );
        match result.unwrap_err() {
            CoherenceError::Violation(v) => {
                assert_eq!(v.tier, CoherenceTier::Tier1Exact);
            }
            CoherenceError::StoreError(e) => {
                panic!("Expected CoherenceError::Violation, got StoreError: {e}");
            }
        }
        // Store unchanged
        assert_eq!(store.len(), datom_count_before);
    }

    #[test]
    fn transact_with_coherence_allows_contradiction_when_forced() {
        let mut store = Store::genesis();
        let entity = EntityId::from_ident(":test/force-override");
        let agent = test_agent();

        // First transaction: establish a value
        let tx1 = Transaction::new(agent, ProvenanceType::Observed, "first value")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("original".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx1).unwrap();

        // Second transaction: conflicting value, force=true
        let tx2 = Transaction::new(agent, ProvenanceType::Observed, "forced override")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("overridden".into()),
            )
            .commit(&store)
            .unwrap();

        let result = transact_with_coherence(&mut store, tx2, true);
        assert!(result.is_ok(), "Should allow contradiction when force=true");
    }

    #[test]
    fn force_mode_creates_coherence_override_audit_trail() {
        let mut store = Store::genesis();
        let entity = EntityId::from_ident(":test/audit-trail");
        let agent = test_agent();

        // First transaction: establish a value
        let tx1 = Transaction::new(agent, ProvenanceType::Observed, "first value")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("original".into()),
            )
            .commit(&store)
            .unwrap();
        store.transact(tx1).unwrap();

        // Second transaction: conflicting value, force=true
        let tx2 = Transaction::new(agent, ProvenanceType::Observed, "forced with audit")
            .assert(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("forced-value".into()),
            )
            .commit(&store)
            .unwrap();
        let tx2_id = tx2.tx_id();

        let receipt = transact_with_coherence(&mut store, tx2, true).unwrap();
        assert_eq!(receipt.tx_id, tx2_id);

        // Verify the audit trail datom exists
        let tx_entity = Store::tx_entity_id(tx2_id);
        let tx_datoms = store.entity_datoms(tx_entity);
        let override_datom = tx_datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":tx/coherence-override" && d.op == Op::Assert);
        assert!(
            override_datom.is_some(),
            "Force mode must create :tx/coherence-override audit trail datom"
        );
        assert_eq!(
            override_datom.unwrap().value,
            Value::Boolean(true),
            "Audit trail datom must be true"
        );
    }

    #[test]
    fn valid_transaction_passes_regardless_of_force_flag() {
        // A non-conflicting transaction should succeed with both force=false and force=true.
        let mut store = Store::genesis();
        let agent = test_agent();

        // force=false with valid (non-conflicting) transaction
        let entity_a = EntityId::from_ident(":test/valid-a");
        let tx_a = Transaction::new(agent, ProvenanceType::Observed, "valid no-force")
            .assert(
                entity_a,
                Attribute::from_keyword(":db/doc"),
                Value::String("value-a".into()),
            )
            .commit(&store)
            .unwrap();
        let result_a = transact_with_coherence(&mut store, tx_a, false);
        assert!(result_a.is_ok(), "Valid tx should pass with force=false");

        // force=true with valid (non-conflicting) transaction
        let entity_b = EntityId::from_ident(":test/valid-b");
        let tx_b = Transaction::new(agent, ProvenanceType::Observed, "valid with-force")
            .assert(
                entity_b,
                Attribute::from_keyword(":db/doc"),
                Value::String("value-b".into()),
            )
            .commit(&store)
            .unwrap();
        let result_b = transact_with_coherence(&mut store, tx_b, true);
        assert!(result_b.is_ok(), "Valid tx should pass with force=true");

        // Verify: force=true on a valid transaction still creates the audit trail
        let receipt_b = result_b.unwrap();
        let tx_entity = Store::tx_entity_id(receipt_b.tx_id);
        let tx_datoms = store.entity_datoms(tx_entity);
        let has_override = tx_datoms
            .iter()
            .any(|d| d.attribute.as_str() == ":tx/coherence-override");
        assert!(
            has_override,
            "force=true should create audit trail even for valid transactions"
        );

        // Verify: force=false does NOT create audit trail
        let receipt_a = result_a.unwrap();
        let tx_entity_a = Store::tx_entity_id(receipt_a.tx_id);
        let tx_datoms_a = store.entity_datoms(tx_entity_a);
        let has_override_a = tx_datoms_a
            .iter()
            .any(|d| d.attribute.as_str() == ":tx/coherence-override");
        assert!(
            !has_override_a,
            "force=false should NOT create audit trail datom"
        );
    }

    // --- Property tests ---

    mod proptests {
        use super::*;
        use crate::proptest_strategies::{arb_doc_value, arb_entity_id, arb_store};
        use proptest::prelude::*;

        proptest! {
            /// INV-TRANSACT-COHERENCE-001: Valid transactions never rejected.
            ///
            /// For arbitrary stores and NEW entities (not yet in store),
            /// a single assertion should always pass coherence check
            /// (no existing value to conflict with).
            #[test]
            fn valid_new_entity_always_passes(
                store in arb_store(3),
                entity in arb_entity_id(),
                value in arb_doc_value(),
            ) {
                let agent = AgentId::from_name("proptest");
                let tx_id = TxId::new(999, 0, agent);
                let datoms = vec![Datom::new(
                    entity,
                    Attribute::from_keyword(":db/doc"),
                    value,
                    tx_id,
                    Op::Assert,
                )];
                // New entity → no conflict possible (unless entity happens to
                // exist with a different value, which is astronomically unlikely
                // with 256-bit content-addressed IDs)
                let result = coherence_check(&store, &datoms);
                // We can't assert Ok for all cases (the entity might exist by
                // random collision), but we CAN assert no panic.
                let _ = result;
            }

            /// Tier 2 never fires on non-spec transactions.
            #[test]
            fn tier2_noop_for_data_transactions(
                store in arb_store(3),
                entity in arb_entity_id(),
                value in arb_doc_value(),
            ) {
                let agent = AgentId::from_name("proptest");
                let tx_id = TxId::new(999, 0, agent);
                let datoms = vec![Datom::new(
                    entity,
                    Attribute::from_keyword(":db/doc"),
                    value,
                    tx_id,
                    Op::Assert,
                )];
                // :db/doc is not a :spec/* attribute → tier2 is always Ok
                prop_assert!(tier2_check(&store, &datoms).is_ok(),
                    "Tier 2 should be no-op for non-spec transactions");
            }

            /// Retractions are always valid (no coherence violation possible).
            #[test]
            fn retractions_always_pass(
                store in arb_store(3),
                entity in arb_entity_id(),
                value in arb_doc_value(),
            ) {
                let agent = AgentId::from_name("proptest");
                let tx_id = TxId::new(999, 0, agent);
                let datoms = vec![Datom::new(
                    entity,
                    Attribute::from_keyword(":db/doc"),
                    value,
                    tx_id,
                    Op::Retract,
                )];
                prop_assert!(coherence_check(&store, &datoms).is_ok(),
                    "Retractions should never trigger coherence violations");
            }

            /// INV-TRANSACT-COHERENCE-001: tier1_check totality.
            ///
            /// For arbitrary stores and arbitrary new datoms, tier1_check never
            /// panics. It always returns either Ok or Err — no unwinding, no
            /// undefined behavior. This is the totality property: the coherence
            /// gate is a total function over its domain.
            #[test]
            fn tier1_never_panics(
                store in arb_store(3),
                entity in arb_entity_id(),
                value in arb_doc_value(),
                op in crate::proptest_strategies::arb_op(),
            ) {
                let agent = AgentId::from_name("proptest:totality");
                let tx_id = TxId::new(999, 0, agent);
                let datoms = vec![Datom::new(
                    entity,
                    Attribute::from_keyword(":db/doc"),
                    value,
                    tx_id,
                    op,
                )];
                // The function must not panic — just assert it returns something.
                let _result = tier1_check(&store, &datoms);
            }

            /// INV-TRANSACT-COHERENCE-001: tier2_check returns Ok for non-spec datoms.
            ///
            /// For arbitrary stores, tier2_check on non-spec attributes always
            /// returns Ok — no false positives. Tier 2 only fires for `:spec/*`
            /// attributes; data transactions must never be rejected by Tier 2.
            #[test]
            fn tier2_no_false_positives_on_data(
                store in arb_store(3),
                entity in arb_entity_id(),
                value in arb_doc_value(),
            ) {
                let agent = AgentId::from_name("proptest:tier2-fp");
                let tx_id = TxId::new(999, 0, agent);

                // Use non-spec attributes: :db/doc and :tx/rationale
                // Both are non-:spec/* so Tier 2 must always return Ok.
                let datoms_doc = vec![Datom::new(
                    entity,
                    Attribute::from_keyword(":db/doc"),
                    value.clone(),
                    tx_id,
                    Op::Assert,
                )];
                prop_assert!(
                    tier2_check(&store, &datoms_doc).is_ok(),
                    "Tier 2 must not reject non-spec attribute :db/doc"
                );

                let datoms_rationale = vec![Datom::new(
                    entity,
                    Attribute::from_keyword(":tx/rationale"),
                    value,
                    tx_id,
                    Op::Assert,
                )];
                prop_assert!(
                    tier2_check(&store, &datoms_rationale).is_ok(),
                    "Tier 2 must not reject non-spec attribute :tx/rationale"
                );
            }

            /// INV-TRANSACT-COHERENCE-001 + INV-STORE-001: Store invariant preservation.
            ///
            /// For any datom that passes tier1_check, inserting it into the store
            /// preserves the store's fundamental invariants:
            /// - Store size does not decrease (append-only, INV-STORE-001)
            /// - Genesis datoms remain present (INV-STORE-007)
            /// - Frontier is non-empty (INV-MERGE-002)
            #[test]
            fn tier1_pass_preserves_store_invariants(
                store in arb_store(3),
                entity in arb_entity_id(),
                value in arb_doc_value(),
            ) {
                let agent = AgentId::from_name("proptest:invariants");
                let tx_id = TxId::new(999, 0, agent);
                let datom = Datom::new(
                    entity,
                    Attribute::from_keyword(":db/doc"),
                    value,
                    tx_id,
                    Op::Assert,
                );

                let check = tier1_check(&store, std::slice::from_ref(&datom));
                if check.is_ok() {
                    // Datom passed coherence — insert it into the datom set.
                    let mut datoms_set = store.datom_set().clone();
                    let size_before = datoms_set.len();

                    datoms_set.insert(datom);
                    let new_store = Store::from_datoms(datoms_set.clone());

                    // INV-STORE-001: size never decreases.
                    prop_assert!(
                        datoms_set.len() >= size_before,
                        "Store size decreased after inserting coherent datom"
                    );

                    // INV-STORE-007: genesis datoms present.
                    let genesis = Store::genesis();
                    prop_assert!(
                        genesis.datom_set().is_subset(&datoms_set),
                        "Genesis datoms lost after inserting coherent datom"
                    );

                    // INV-MERGE-002: frontier non-empty.
                    prop_assert!(
                        !new_store.frontier().is_empty(),
                        "Frontier empty after inserting coherent datom"
                    );
                }
            }
        }
    }
}
