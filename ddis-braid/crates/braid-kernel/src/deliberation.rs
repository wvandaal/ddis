//! Deliberation pipeline — structured conflict resolution over a lattice of positions.
//!
//! When agents disagree (or the coherence gate detects a Tier 2 contradiction),
//! the deliberation system provides structured resolution via positions, evidence,
//! and decision methods.
//!
//! # Lifecycle
//!
//! ```text
//! :open → :active → :decided | :stalled → :contested → :superseded
//! ```
//!
//! # Entity Types
//!
//! - **Deliberation**: The topic being deliberated, contested attributes, method, status.
//! - **Position**: A stance taken by an agent, with rationale and evidence.
//! - **Decision**: The chosen position, method used, and rationale for the choice.
//!
//! # Decision Methods
//!
//! 1. **Consensus**: All positions agree (strongest).
//! 2. **Majority**: >50% of positions agree.
//! 3. **Authority**: Highest-authority agent decides.
//! 4. **HumanOverride**: Human provides explicit decision.
//! 5. **Automated**: Lattice or LWW resolution (weakest).
//!
//! # Invariants
//!
//! - **INV-DELIBERATION-001**: Deliberation lifecycle is well-ordered.
//! - **INV-DELIBERATION-002**: Every position references exactly one deliberation.
//! - **INV-DELIBERATION-003**: Precedent is queryable after decision.
//! - **INV-DELIBERATION-004**: Stability score converges.
//! - **INV-DELIBERATION-005**: Decision method matches positions.
//! - **INV-DELIBERATION-006**: Deliberation eventually terminates.
//!
//! # Traces To
//!
//! - SEED.md §7 (Deliberation)
//! - spec/07-deliberation.md
//! - ADR-RESOLUTION-005 (Deliberation as entity)

use std::collections::BTreeSet;

use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, TxId, Value};
use crate::store::Store;

// ===========================================================================
// Types
// ===========================================================================

/// Deliberation status lifecycle.
///
/// INV-DELIBERATION-001: Well-ordered lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeliberationStatus {
    /// Newly opened, awaiting positions.
    Open,
    /// Positions submitted, discussion in progress.
    Active,
    /// Decision reached.
    Decided,
    /// No new positions for N turns, requires escalation.
    Stalled,
    /// Decision contested by new evidence.
    Contested,
    /// Superseded by a later deliberation.
    Superseded,
}

impl DeliberationStatus {
    /// Convert to keyword for datom storage.
    pub fn as_keyword(&self) -> &'static str {
        match self {
            Self::Open => ":deliberation.status/open",
            Self::Active => ":deliberation.status/active",
            Self::Decided => ":deliberation.status/decided",
            Self::Stalled => ":deliberation.status/stalled",
            Self::Contested => ":deliberation.status/contested",
            Self::Superseded => ":deliberation.status/superseded",
        }
    }
}

/// Decision method — how the deliberation was resolved.
///
/// Ordered by strength: Consensus > Majority > Authority > HumanOverride > Automated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DecisionMethod {
    /// Lattice or LWW resolution (weakest).
    Automated,
    /// Human provides explicit decision.
    HumanOverride,
    /// Highest-authority agent decides.
    Authority,
    /// >50% of positions agree.
    Majority,
    /// All positions agree (strongest).
    Consensus,
}

impl DecisionMethod {
    /// Convert to keyword for datom storage.
    pub fn as_keyword(&self) -> &'static str {
        match self {
            Self::Consensus => ":decision.method/consensus",
            Self::Majority => ":decision.method/majority",
            Self::Authority => ":decision.method/authority",
            Self::HumanOverride => ":decision.method/human-override",
            Self::Automated => ":decision.method/automated",
        }
    }
}

/// Stability score for a deliberation — how close it is to convergence.
#[derive(Clone, Debug)]
pub struct StabilityScore {
    /// Number of unique stances.
    pub unique_stances: usize,
    /// Total positions submitted.
    pub total_positions: usize,
    /// Whether all positions agree.
    pub is_unanimous: bool,
    /// Stability metric in [0, 1]. 1.0 = fully converged (all agree).
    pub score: f64,
}

// ===========================================================================
// Core Operations
// ===========================================================================

/// Open a new deliberation.
///
/// Creates a deliberation entity with the given topic and contested attributes.
/// Returns the datoms to transact and the deliberation entity ID.
pub fn open_deliberation(
    topic: &str,
    contested_attrs: &[Attribute],
    tx_id: TxId,
) -> (EntityId, Vec<Datom>) {
    let ident = format!(":deliberation/{}", topic.replace(' ', "-").to_lowercase());
    let entity = EntityId::from_ident(&ident);

    let mut datoms = vec![
        Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(ident),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":deliberation/topic"),
            Value::String(topic.to_string()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":deliberation/status"),
            Value::Keyword(DeliberationStatus::Open.as_keyword().to_string()),
            tx_id,
            Op::Assert,
        ),
    ];

    // Add contested attributes
    for attr in contested_attrs {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":deliberation/contested-attrs"),
            Value::String(attr.as_str().to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    (entity, datoms)
}

/// Add a position to an existing deliberation.
///
/// INV-DELIBERATION-002: Every position references exactly one deliberation.
pub fn add_position(
    deliberation: EntityId,
    stance: &str,
    rationale: &str,
    evidence: &[EntityId],
    agent: AgentId,
    tx_id: TxId,
) -> (EntityId, Vec<Datom>) {
    let ident = format!(
        ":position/{}-{}",
        stance.replace(' ', "-").to_lowercase(),
        tx_id.wall_time()
    );
    let entity = EntityId::from_ident(&ident);

    let mut datoms = vec![
        Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(ident),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":position/deliberation"),
            Value::Ref(deliberation),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":position/stance"),
            Value::String(stance.to_string()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":position/rationale"),
            Value::String(rationale.to_string()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":position/agent"),
            Value::Ref(EntityId::from_content(agent.as_bytes())),
            tx_id,
            Op::Assert,
        ),
    ];

    // Add evidence references
    for ev in evidence {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":position/evidence"),
            Value::Ref(*ev),
            tx_id,
            Op::Assert,
        ));
    }

    (entity, datoms)
}

/// Record a decision for a deliberation.
///
/// INV-DELIBERATION-005: Decision method matches available positions.
pub fn decide(
    deliberation: EntityId,
    chosen_position: EntityId,
    method: DecisionMethod,
    rationale: &str,
    tx_id: TxId,
) -> (EntityId, Vec<Datom>) {
    let ident = format!(":decision/{}", tx_id.wall_time());
    let entity = EntityId::from_ident(&ident);

    let datoms = vec![
        Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(ident),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":decision/deliberation"),
            Value::Ref(deliberation),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":decision/chosen"),
            Value::Ref(chosen_position),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":decision/method"),
            Value::Keyword(method.as_keyword().to_string()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":decision/rationale"),
            Value::String(rationale.to_string()),
            tx_id,
            Op::Assert,
        ),
        // Update deliberation status to Decided
        Datom::new(
            deliberation,
            Attribute::from_keyword(":deliberation/status"),
            Value::Keyword(DeliberationStatus::Decided.as_keyword().to_string()),
            tx_id,
            Op::Assert,
        ),
    ];

    (entity, datoms)
}

/// Find precedent: previous deliberations on similar topics.
///
/// INV-DELIBERATION-003: Precedent queryable after decision.
pub fn find_precedent(store: &Store, topic_keywords: &[&str]) -> Vec<EntityId> {
    let topic_attr = Attribute::from_keyword(":deliberation/topic");
    let status_attr = Attribute::from_keyword(":deliberation/status");
    let decided_kw = DeliberationStatus::Decided.as_keyword();

    let mut results = Vec::new();

    for datom in store.attribute_datoms(&topic_attr) {
        if datom.op != Op::Assert {
            continue;
        }
        let topic = match &datom.value {
            Value::String(s) => s.to_lowercase(),
            _ => continue,
        };

        // Check if any keyword matches
        let matches = topic_keywords
            .iter()
            .any(|kw| topic.contains(&kw.to_lowercase()));

        if !matches {
            continue;
        }

        // Check if this deliberation is decided
        let entity_datoms = store.entity_datoms(datom.entity);
        let is_decided = entity_datoms.iter().any(|d| {
            d.attribute == status_attr
                && d.op == Op::Assert
                && d.value == Value::Keyword(decided_kw.to_string())
        });

        if is_decided {
            results.push(datom.entity);
        }
    }

    results
}

/// Check stability of a deliberation — are positions converging?
///
/// INV-DELIBERATION-004: Stability score converges.
pub fn check_stability(store: &Store, deliberation: EntityId) -> StabilityScore {
    let position_delib_attr = Attribute::from_keyword(":position/deliberation");
    let stance_attr = Attribute::from_keyword(":position/stance");

    // Find all positions for this deliberation
    let mut stances: Vec<String> = Vec::new();

    for datom in store.attribute_datoms(&position_delib_attr) {
        if datom.op == Op::Assert && datom.value == Value::Ref(deliberation) {
            let position_entity = datom.entity;
            // Get the stance
            let entity_datoms = store.entity_datoms(position_entity);
            if let Some(stance_datom) = entity_datoms
                .iter()
                .find(|d| d.attribute == stance_attr && d.op == Op::Assert)
            {
                if let Value::String(s) = &stance_datom.value {
                    stances.push(s.clone());
                }
            }
        }
    }

    let total = stances.len();
    let unique: BTreeSet<&str> = stances.iter().map(|s| s.as_str()).collect();
    let unique_count = unique.len();

    let score = if total == 0 {
        0.0
    } else if unique_count == 1 {
        1.0 // Unanimous
    } else {
        // Most common stance frequency / total
        let mut freq: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for s in &stances {
            *freq.entry(s.as_str()).or_default() += 1;
        }
        let max_freq = freq.values().copied().max().unwrap_or(0);
        max_freq as f64 / total as f64
    };

    StabilityScore {
        unique_stances: unique_count,
        total_positions: total,
        is_unanimous: unique_count <= 1 && total > 0,
        score,
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::AgentId;

    fn test_agent() -> AgentId {
        AgentId::from_name("delib-test")
    }

    fn test_tx(wall: u64) -> TxId {
        TxId::new(wall, 0, test_agent())
    }

    #[test]
    fn open_deliberation_creates_entity() {
        let (_entity, datoms) = open_deliberation(
            "append-only vs mutable store",
            &[Attribute::from_keyword(":store/mutability")],
            test_tx(100),
        );

        assert!(datoms.len() >= 3, "need ident + topic + status + attrs");
        // Check status is Open
        let status = datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":deliberation/status");
        assert!(status.is_some());
        assert_eq!(
            status.unwrap().value,
            Value::Keyword(":deliberation.status/open".into())
        );
    }

    #[test]
    fn add_position_references_deliberation() {
        let (delib_entity, _) = open_deliberation("test topic", &[], test_tx(100));

        let (_pos_entity, pos_datoms) = add_position(
            delib_entity,
            "keep append-only",
            "Simpler model, CRDT merge",
            &[],
            test_agent(),
            test_tx(200),
        );

        // Check deliberation reference
        let delib_ref = pos_datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":position/deliberation");
        assert!(delib_ref.is_some());
        assert_eq!(delib_ref.unwrap().value, Value::Ref(delib_entity));
    }

    #[test]
    fn decide_updates_status() {
        let (delib_entity, _) = open_deliberation("test", &[], test_tx(100));
        let (pos_entity, _) = add_position(
            delib_entity,
            "option-a",
            "best",
            &[],
            test_agent(),
            test_tx(200),
        );

        let (_decision_entity, decision_datoms) = decide(
            delib_entity,
            pos_entity,
            DecisionMethod::Consensus,
            "All agents agreed",
            test_tx(300),
        );

        // Check decision references deliberation
        let delib_ref = decision_datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":decision/deliberation");
        assert_eq!(delib_ref.unwrap().value, Value::Ref(delib_entity));

        // Check status updated to Decided
        let status = decision_datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":deliberation/status");
        assert_eq!(
            status.unwrap().value,
            Value::Keyword(":deliberation.status/decided".into())
        );
    }

    #[test]
    fn stability_unanimous() {
        let store = Store::genesis();
        let agent = test_agent();

        let (delib, delib_datoms) = open_deliberation("test", &[], test_tx(100));

        // Add two positions with same stance
        let (_, pos1) = add_position(delib, "agree", "reason 1", &[], agent, test_tx(200));
        let (_, pos2) = add_position(delib, "agree", "reason 2", &[], agent, test_tx(300));

        // Transact all
        let _tx =
            crate::store::Transaction::new(agent, crate::datom::ProvenanceType::Observed, "test")
                .commit(&store);
        // Use from_datoms for simplicity
        let mut all_datoms = store.datom_set().clone();
        for d in delib_datoms.iter().chain(pos1.iter()).chain(pos2.iter()) {
            all_datoms.insert(d.clone());
        }
        let store = Store::from_datoms(all_datoms);

        let stability = check_stability(&store, delib);
        assert_eq!(stability.total_positions, 2);
        assert!(stability.is_unanimous);
        assert!((stability.score - 1.0).abs() < 1e-10);
    }

    #[test]
    fn stability_split() {
        let agent = test_agent();

        let (delib, delib_datoms) = open_deliberation("split", &[], test_tx(100));
        let (_, pos1) = add_position(delib, "option-a", "faster", &[], agent, test_tx(200));
        let (_, pos2) = add_position(delib, "option-b", "safer", &[], agent, test_tx(300));

        let mut all_datoms = Store::genesis().datom_set().clone();
        for d in delib_datoms.iter().chain(pos1.iter()).chain(pos2.iter()) {
            all_datoms.insert(d.clone());
        }
        let store = Store::from_datoms(all_datoms);

        let stability = check_stability(&store, delib);
        assert_eq!(stability.total_positions, 2);
        assert!(!stability.is_unanimous);
        assert!((stability.score - 0.5).abs() < 1e-10); // 50% agreement
    }

    #[test]
    fn decision_method_ordering() {
        // Strength ordering: Consensus > Majority > Authority > HumanOverride > Automated
        assert!(DecisionMethod::Consensus > DecisionMethod::Majority);
        assert!(DecisionMethod::Majority > DecisionMethod::Authority);
        assert!(DecisionMethod::Authority > DecisionMethod::HumanOverride);
        assert!(DecisionMethod::HumanOverride > DecisionMethod::Automated);
    }

    #[test]
    fn deliberation_status_lifecycle() {
        // Open < Active < Decided
        assert!(DeliberationStatus::Open < DeliberationStatus::Active);
        assert!(DeliberationStatus::Active < DeliberationStatus::Decided);
        assert!(DeliberationStatus::Decided < DeliberationStatus::Stalled);
    }
}
