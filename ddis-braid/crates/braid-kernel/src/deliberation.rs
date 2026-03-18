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

use crate::coherence::{CoherenceTier, CoherenceViolation};
use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, TxId, Value};
use crate::store::Store;

// ===========================================================================
// Types
// ===========================================================================

/// Deliberation status lifecycle.
///
/// INV-DELIBERATION-001: Well-ordered lifecycle.
///
/// This is a PARTIAL order, not a total order. Some status pairs are
/// incomparable (e.g., Decided and Stalled are both successors of Active
/// but neither precedes the other). Only `PartialOrd` is implemented; `Ord`
/// is intentionally absent.
///
/// The defined orderings are:
///   Open < Active
///   Active < Decided
///   Active < Stalled
///   Stalled < Contested
///   Decided < Superseded
///   Contested < Superseded
///
/// For deterministic sorting (e.g., in BTreeSet keys), use
/// [`DeliberationStatus::sort_key`] which provides a numeric code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

impl PartialOrd for DeliberationStatus {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self == other {
            return Some(std::cmp::Ordering::Equal);
        }
        use std::cmp::Ordering::*;
        use DeliberationStatus::*;
        match (self, other) {
            (Open, Active | Decided | Stalled | Contested | Superseded) => Some(Less),
            (Active | Decided | Stalled | Contested | Superseded, Open) => Some(Greater),
            (Active, Decided | Stalled | Contested | Superseded) => Some(Less),
            (Decided | Stalled | Contested | Superseded, Active) => Some(Greater),
            (Stalled, Contested | Superseded) => Some(Less),
            (Contested | Superseded, Stalled) => Some(Greater),
            (Decided, Superseded) => Some(Less),
            (Superseded, Decided) => Some(Greater),
            (Contested, Superseded) => Some(Less),
            (Superseded, Contested) => Some(Greater),
            // Decided vs Stalled: INCOMPARABLE
            (Decided, Stalled) | (Stalled, Decided) => None,
            // Decided vs Contested: INCOMPARABLE
            (Decided, Contested) | (Contested, Decided) => None,
            _ => Some(Equal),
        }
    }
}

impl DeliberationStatus {
    /// Numeric sort key for deterministic ordering in collections.
    pub fn sort_key(self) -> u8 {
        match self {
            Self::Open => 0,
            Self::Active => 1,
            Self::Decided => 2,
            Self::Stalled => 3,
            Self::Contested => 4,
            Self::Superseded => 5,
        }
    }

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

/// Default stability minimum threshold.
///
/// INV-DELIBERATION-002: No decision is recorded unless `stability >= STABILITY_MIN`.
/// UNC-DELIBERATION-001: This default of 0.7 may be adjusted per entity type
/// after Stage 2 calibration. Stored as a configurable datom at `:config/stability-min`.
pub const STABILITY_MIN: f64 = 0.7;

/// Error returned when a decision fails the stability guard.
///
/// INV-DELIBERATION-002: Stability Guard Enforcement.
#[derive(Clone, Debug)]
pub struct StabilityError {
    /// The deliberation that failed the guard.
    pub deliberation: EntityId,
    /// The measured stability score.
    pub score: f64,
    /// The required minimum.
    pub required: f64,
    /// Number of positions at the time of the check.
    pub position_count: usize,
}

impl std::fmt::Display for StabilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "stability guard failed: score {:.2} < required {:.2} ({} positions)",
            self.score, self.required, self.position_count
        )
    }
}

impl std::error::Error for StabilityError {}

/// Record a decision for a deliberation with stability guard enforcement.
///
/// INV-DELIBERATION-002: `decide(d) => stability(d.chosen) >= stability_min`.
/// Checks `check_stability(store, deliberation)` before generating decision datoms.
/// Returns `Err(StabilityError)` if the stability score is below `STABILITY_MIN`.
///
/// Minimum requirements for the guard to pass:
/// - At least 2 positions submitted (ensures meaningful deliberation)
/// - Stability score >= STABILITY_MIN (0.7 default)
///
/// # Traces To
///
/// - spec/11-deliberation.md INV-DELIBERATION-002
/// - UNC-DELIBERATION-001 (threshold calibration)
pub fn decide_with_guard(
    store: &Store,
    deliberation: EntityId,
    chosen_position: EntityId,
    method: DecisionMethod,
    rationale: &str,
    tx_id: TxId,
) -> Result<(EntityId, Vec<Datom>), StabilityError> {
    let stability = check_stability(store, deliberation);

    // Guard: minimum position count (need at least 2 positions for meaningful deliberation)
    if stability.total_positions < 2 {
        return Err(StabilityError {
            deliberation,
            score: stability.score,
            required: STABILITY_MIN,
            position_count: stability.total_positions,
        });
    }

    // Guard: stability score must meet minimum threshold
    if stability.score < STABILITY_MIN {
        return Err(StabilityError {
            deliberation,
            score: stability.score,
            required: STABILITY_MIN,
            position_count: stability.total_positions,
        });
    }

    Ok(decide(
        deliberation,
        chosen_position,
        method,
        rationale,
        tx_id,
    ))
}

/// Record a decision for a deliberation (unchecked — no stability guard).
///
/// INV-DELIBERATION-005: Decision method matches available positions.
///
/// **Prefer `decide_with_guard`** which enforces INV-DELIBERATION-002.
/// This unchecked variant exists for cases where the caller has already
/// verified stability externally, or for backward compatibility in tests.
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
        // ADR-COHERENCE-001: Status transition uses retract-then-assert.
        // Retract the previous status (Open or Active — retract both to be safe;
        // retracting a non-existent value is harmless in an append-only store).
        Datom::new(
            deliberation,
            Attribute::from_keyword(":deliberation/status"),
            Value::Keyword(DeliberationStatus::Open.as_keyword().to_string()),
            tx_id,
            Op::Retract,
        ),
        Datom::new(
            deliberation,
            Attribute::from_keyword(":deliberation/status"),
            Value::Keyword(DeliberationStatus::Active.as_keyword().to_string()),
            tx_id,
            Op::Retract,
        ),
        // Assert the new status
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
// Coherence Gate → Deliberation Bridge
// ===========================================================================

/// Convert a Tier 2 coherence violation into a Deliberation entity with
/// both positions (existing spec + proposed spec) recorded as Position entities.
///
/// When the coherence gate detects a logical contradiction between spec elements,
/// this function creates the deliberation machinery so agents can resolve
/// the conflict through the structured decision pipeline rather than simply
/// rejecting the transaction.
///
/// Returns the deliberation entity ID and all datoms to transact (deliberation +
/// two positions: one for the existing spec element, one for the proposed).
///
/// # Invariants
///
/// - INV-DELIBERATION-001: Deliberation starts in Open status.
/// - INV-DELIBERATION-002: Both positions reference the deliberation.
///
/// # Traces To
///
/// - spec/07-deliberation.md (coherence → deliberation bridge)
/// - ADR-RESOLUTION-005 (Deliberation as entity)
pub fn coherence_violation_to_deliberation(
    violation: &CoherenceViolation,
    tx_id: TxId,
) -> (EntityId, Vec<Datom>) {
    // Determine topic from the violation description
    let topic = format!(
        "coherence-{}-{}",
        match violation.tier {
            CoherenceTier::Tier1Exact => "tier1-exact",
            CoherenceTier::Tier2Logical => "tier2-logical",
        },
        violation.offending_datom.entity.as_bytes()[..4]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );

    // Contested attribute is the one from the offending datom
    let contested_attrs = vec![violation.offending_datom.attribute.clone()];

    // Open the deliberation
    let (delib_entity, mut datoms) = open_deliberation(&topic, &contested_attrs, tx_id);

    // Position A: the existing spec element (what the store already has)
    let existing_stance = format!("existing: {}", &violation.existing_context);
    let existing_rationale = format!(
        "The store already contains this value. Context: {}",
        violation.existing_context
    );
    let existing_ident = format!(":position/existing-{}", tx_id.wall_time());
    let existing_pos = EntityId::from_ident(&existing_ident);

    let existing_datoms = vec![
        Datom::new(
            existing_pos,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(existing_ident),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            existing_pos,
            Attribute::from_keyword(":position/deliberation"),
            Value::Ref(delib_entity),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            existing_pos,
            Attribute::from_keyword(":position/stance"),
            Value::String(existing_stance),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            existing_pos,
            Attribute::from_keyword(":position/rationale"),
            Value::String(existing_rationale),
            tx_id,
            Op::Assert,
        ),
    ];

    // Position B: the proposed (offending) datom
    let proposed_value_desc = format!("{:?}", violation.offending_datom.value);
    let proposed_stance = format!("proposed: {}", &proposed_value_desc);
    let proposed_rationale = format!(
        "The transaction proposes this new value. Fix hint: {}",
        violation.fix_hint
    );
    let proposed_ident = format!(":position/proposed-{}", tx_id.wall_time());
    let proposed_pos = EntityId::from_ident(&proposed_ident);

    let proposed_datoms = vec![
        Datom::new(
            proposed_pos,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(proposed_ident),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            proposed_pos,
            Attribute::from_keyword(":position/deliberation"),
            Value::Ref(delib_entity),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            proposed_pos,
            Attribute::from_keyword(":position/stance"),
            Value::String(proposed_stance),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            proposed_pos,
            Attribute::from_keyword(":position/rationale"),
            Value::String(proposed_rationale),
            tx_id,
            Op::Assert,
        ),
    ];

    // Combine all datoms
    datoms.extend(existing_datoms);
    datoms.extend(proposed_datoms);

    (delib_entity, datoms)
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

        // ADR-COHERENCE-001: Check retract-then-assert status transition
        let status_assert = decision_datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":deliberation/status" && d.op == Op::Assert);
        assert_eq!(
            status_assert.unwrap().value,
            Value::Keyword(":deliberation.status/decided".into())
        );
        // Retractions for previous statuses should also be present
        let status_retracts: Vec<_> = decision_datoms
            .iter()
            .filter(|d| d.attribute.as_str() == ":deliberation/status" && d.op == Op::Retract)
            .collect();
        assert!(
            !status_retracts.is_empty(),
            "ADR-COHERENCE-001: decide() must retract old status"
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
        assert!(DeliberationStatus::Open < DeliberationStatus::Active);
        assert!(DeliberationStatus::Active < DeliberationStatus::Decided);
        assert!(DeliberationStatus::Active < DeliberationStatus::Stalled);
        // Decided and Stalled are INCOMPARABLE (partial order)
        assert_eq!(
            DeliberationStatus::Decided.partial_cmp(&DeliberationStatus::Stalled),
            None
        );
        assert_eq!(
            DeliberationStatus::Stalled.partial_cmp(&DeliberationStatus::Decided),
            None
        );
        assert!(DeliberationStatus::Decided < DeliberationStatus::Superseded);
        assert!(DeliberationStatus::Contested < DeliberationStatus::Superseded);
        assert!(DeliberationStatus::Stalled < DeliberationStatus::Contested);
        // sort_key provides a total order for collection use
        assert!(DeliberationStatus::Open.sort_key() < DeliberationStatus::Active.sort_key());
        assert!(DeliberationStatus::Decided.sort_key() < DeliberationStatus::Superseded.sort_key());
    }

    // --- W5B.5: Comprehensive deliberation tests ---

    /// Verifies: INV-DELIBERATION-001 -- Deliberation lifecycle traverses
    /// Open -> Active (implicit via positions) -> Decided.
    /// The full lifecycle: open a deliberation, add positions, decide,
    /// and verify the store reflects the Decided status.
    #[test]
    fn deliberation_reaches_decided_from_positions() {
        let agent = test_agent();

        // 1. Open deliberation
        let (delib, delib_datoms) = open_deliberation("lifecycle-test", &[], test_tx(100));

        // Verify starts Open
        let status = delib_datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":deliberation/status")
            .unwrap();
        assert_eq!(
            status.value,
            Value::Keyword(":deliberation.status/open".into()),
            "Deliberation must start in Open status"
        );

        // 2. Add two positions
        let (pos_a, pos_a_datoms) = add_position(
            delib,
            "keep-append-only",
            "Simpler model, proven CRDT merge",
            &[],
            agent,
            test_tx(200),
        );
        let (_pos_b, pos_b_datoms) = add_position(
            delib,
            "allow-mutation",
            "Performance optimization",
            &[],
            agent,
            test_tx(300),
        );

        // 3. Decide in favor of position A
        let (decision, decision_datoms) = decide(
            delib,
            pos_a,
            DecisionMethod::Authority,
            "Architecture team decided: append-only aligns with C1",
            test_tx(400),
        );

        // 4. Build a store with all datoms and verify
        let mut all_datoms = Store::genesis().datom_set().clone();
        for d in delib_datoms
            .iter()
            .chain(pos_a_datoms.iter())
            .chain(pos_b_datoms.iter())
            .chain(decision_datoms.iter())
        {
            all_datoms.insert(d.clone());
        }
        let store = Store::from_datoms(all_datoms);

        // Verify: decision entity references deliberation
        let decision_delib = store
            .entity_datoms(decision)
            .into_iter()
            .find(|d| d.attribute.as_str() == ":decision/deliberation")
            .expect("Decision must reference a deliberation");
        assert_eq!(decision_delib.value, Value::Ref(delib));

        // Verify: decision chose position A
        let chosen = store
            .entity_datoms(decision)
            .into_iter()
            .find(|d| d.attribute.as_str() == ":decision/chosen")
            .expect("Decision must reference the chosen position");
        assert_eq!(chosen.value, Value::Ref(pos_a));

        // Verify: deliberation status is Decided (latest assertion wins in store)
        let delib_datoms_in_store = store.entity_datoms(delib);
        let decided_status = delib_datoms_in_store.iter().any(|d| {
            d.attribute.as_str() == ":deliberation/status"
                && d.value == Value::Keyword(":deliberation.status/decided".into())
        });
        assert!(
            decided_status,
            "Deliberation must have a Decided status datom after decide()"
        );
    }

    /// Verifies: INV-DELIBERATION-003 -- Precedent queryable after decision.
    /// After a deliberation reaches Decided, find_precedent() must return it
    /// when queried with matching keywords.
    #[test]
    fn precedent_queryable_after_decision() {
        let agent = test_agent();

        // Create and decide a deliberation about "store mutability"
        let (delib, delib_datoms) = open_deliberation(
            "store mutability policy",
            &[Attribute::from_keyword(":store/mutability")],
            test_tx(100),
        );
        let (pos, pos_datoms) = add_position(
            delib,
            "append-only",
            "CRDT requires it",
            &[],
            agent,
            test_tx(200),
        );
        let (_, decision_datoms) = decide(
            delib,
            pos,
            DecisionMethod::Consensus,
            "Unanimous agreement",
            test_tx(300),
        );

        // Build store
        let mut all_datoms = Store::genesis().datom_set().clone();
        for d in delib_datoms
            .iter()
            .chain(pos_datoms.iter())
            .chain(decision_datoms.iter())
        {
            all_datoms.insert(d.clone());
        }
        let store = Store::from_datoms(all_datoms);

        // Query precedent with matching keywords
        let precedents = find_precedent(&store, &["store", "mutability"]);
        assert!(
            precedents.contains(&delib),
            "INV-DELIBERATION-003: Decided deliberation must be found as precedent. Got: {:?}",
            precedents
        );

        // Query with non-matching keywords should NOT return it
        let no_match = find_precedent(&store, &["network", "protocol"]);
        assert!(
            !no_match.contains(&delib),
            "Precedent search must not match unrelated keywords"
        );
    }

    /// Verifies: INV-DELIBERATION-004 -- Stability score converges to 1.0
    /// for unanimous positions. As more positions with the same stance are
    /// added, the stability score must remain at 1.0.
    #[test]
    fn stability_score_converges_to_1_for_unanimous() {
        let agent = test_agent();
        let (delib, delib_datoms) = open_deliberation("convergence-test", &[], test_tx(100));

        // Add progressively more positions, all with the same stance
        let mut all_datoms = Store::genesis().datom_set().clone();
        for d in &delib_datoms {
            all_datoms.insert(d.clone());
        }

        for i in 1u64..=5 {
            let (_, pos_datoms) = add_position(
                delib,
                "unanimous-stance",
                &format!("reason {i}"),
                &[],
                agent,
                test_tx(100 + i * 100),
            );
            for d in &pos_datoms {
                all_datoms.insert(d.clone());
            }

            let store = Store::from_datoms(all_datoms.clone());
            let stability = check_stability(&store, delib);

            assert_eq!(
                stability.total_positions, i as usize,
                "After {i} positions, total should be {i}"
            );
            assert!(
                stability.is_unanimous,
                "All positions have the same stance -- must be unanimous at step {i}"
            );
            assert!(
                (stability.score - 1.0).abs() < 1e-10,
                "INV-DELIBERATION-004: Unanimous positions must yield score 1.0, got {}",
                stability.score
            );
        }
    }

    /// Verifies: Coherence violation -> deliberation bridge.
    /// A Tier 2 CoherenceViolation is converted into a Deliberation entity
    /// with two Position entities (existing vs proposed).
    #[test]
    fn coherence_violation_creates_deliberation() {
        use crate::coherence::{CoherenceTier, CoherenceViolation};

        let tx = test_tx(500);

        // Simulate a Tier 2 logical contradiction
        let violation = CoherenceViolation {
            tier: CoherenceTier::Tier2Logical,
            offending_datom: Datom::new(
                EntityId::from_ident(":spec/new-inv"),
                Attribute::from_keyword(":spec/statement"),
                Value::String("The store must allow mutation".to_string()),
                tx,
                Op::Assert,
            ),
            existing_context: "Existing spec :spec/inv-store-001: \"The store must never mutate\""
                .to_string(),
            description: "Tier 2 polarity inversion: 'must' vs 'must not'".to_string(),
            fix_hint: "Open a deliberation to resolve the conflict.".to_string(),
        };

        let (delib_entity, datoms) = coherence_violation_to_deliberation(&violation, tx);

        // 1. Deliberation entity exists with Open status
        let delib_status = datoms
            .iter()
            .find(|d| d.entity == delib_entity && d.attribute.as_str() == ":deliberation/status");
        assert!(delib_status.is_some(), "Deliberation entity must exist");
        assert_eq!(
            delib_status.unwrap().value,
            Value::Keyword(":deliberation.status/open".into()),
            "INV-DELIBERATION-001: Must start in Open status"
        );

        // 2. Topic contains tier information
        let topic = datoms
            .iter()
            .find(|d| d.entity == delib_entity && d.attribute.as_str() == ":deliberation/topic");
        assert!(topic.is_some(), "Deliberation must have a topic");
        if let Value::String(t) = &topic.unwrap().value {
            assert!(
                t.contains("tier2-logical"),
                "Topic must contain tier info, got: {t}"
            );
        }

        // 3. Two positions exist, both referencing the deliberation
        let position_refs: Vec<&Datom> = datoms
            .iter()
            .filter(|d| {
                d.attribute.as_str() == ":position/deliberation"
                    && d.value == Value::Ref(delib_entity)
            })
            .collect();
        assert_eq!(
            position_refs.len(),
            2,
            "INV-DELIBERATION-002: Must have exactly 2 positions (existing + proposed)"
        );

        // 4. One position has "existing" stance, one has "proposed" stance
        let stances: Vec<&str> = datoms
            .iter()
            .filter(|d| d.attribute.as_str() == ":position/stance")
            .filter_map(|d| match &d.value {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            stances.iter().any(|s| s.starts_with("existing:")),
            "Must have an 'existing' position. Stances: {:?}",
            stances
        );
        assert!(
            stances.iter().any(|s| s.starts_with("proposed:")),
            "Must have a 'proposed' position. Stances: {:?}",
            stances
        );

        // 5. Contested attribute recorded
        let contested = datoms.iter().find(|d| {
            d.entity == delib_entity && d.attribute.as_str() == ":deliberation/contested-attrs"
        });
        assert!(
            contested.is_some(),
            "Deliberation must record contested attributes"
        );
        assert_eq!(
            contested.unwrap().value,
            Value::String(":spec/statement".to_string()),
            "Contested attribute must match the offending datom's attribute"
        );

        // 6. All datoms can be inserted into a store without error
        let mut all_datoms = Store::genesis().datom_set().clone();
        for d in &datoms {
            all_datoms.insert(d.clone());
        }
        let store = Store::from_datoms(all_datoms);
        assert!(
            store.entity_datoms(delib_entity).len() >= 3,
            "Deliberation entity must have ident + topic + status + contested-attrs"
        );
    }

    // --- INV-DELIBERATION-002: Stability Guard Enforcement tests ---

    /// Verifies: INV-DELIBERATION-002 — decide_with_guard rejects unstable deliberations.
    /// A deliberation with a split vote (score = 0.5) must be rejected.
    #[test]
    fn stability_guard_rejects_split_vote() {
        let agent = test_agent();

        let (delib, delib_datoms) = open_deliberation("guard-split", &[], test_tx(100));
        let (pos_a, pos_a_datoms) =
            add_position(delib, "option-a", "faster", &[], agent, test_tx(200));
        let (_, pos_b_datoms) = add_position(delib, "option-b", "safer", &[], agent, test_tx(300));

        let mut all_datoms = Store::genesis().datom_set().clone();
        for d in delib_datoms
            .iter()
            .chain(pos_a_datoms.iter())
            .chain(pos_b_datoms.iter())
        {
            all_datoms.insert(d.clone());
        }
        let store = Store::from_datoms(all_datoms);

        // Stability is 0.5 (split vote), below STABILITY_MIN (0.7)
        let result = decide_with_guard(
            &store,
            delib,
            pos_a,
            DecisionMethod::Authority,
            "trying to decide a split vote",
            test_tx(400),
        );

        assert!(
            result.is_err(),
            "INV-DELIBERATION-002: decide_with_guard must reject split vote (stability 0.5 < 0.7)"
        );
        let err = result.unwrap_err();
        assert!(
            err.score < STABILITY_MIN,
            "error must report score below STABILITY_MIN"
        );
        assert_eq!(err.position_count, 2);
    }

    /// Verifies: INV-DELIBERATION-002 — decide_with_guard accepts unanimous deliberations.
    /// A deliberation where all positions agree (score = 1.0) must pass.
    #[test]
    fn stability_guard_accepts_unanimous() {
        let agent = test_agent();

        let (delib, delib_datoms) = open_deliberation("guard-unanimous", &[], test_tx(100));
        let (pos_a, pos_a_datoms) =
            add_position(delib, "agree", "reason 1", &[], agent, test_tx(200));
        let (_, pos_b_datoms) = add_position(delib, "agree", "reason 2", &[], agent, test_tx(300));

        let mut all_datoms = Store::genesis().datom_set().clone();
        for d in delib_datoms
            .iter()
            .chain(pos_a_datoms.iter())
            .chain(pos_b_datoms.iter())
        {
            all_datoms.insert(d.clone());
        }
        let store = Store::from_datoms(all_datoms);

        // Stability is 1.0 (unanimous), well above STABILITY_MIN (0.7)
        let result = decide_with_guard(
            &store,
            delib,
            pos_a,
            DecisionMethod::Consensus,
            "unanimous agreement",
            test_tx(400),
        );

        assert!(
            result.is_ok(),
            "INV-DELIBERATION-002: decide_with_guard must accept unanimous (stability 1.0 >= 0.7)"
        );
        let (decision_entity, decision_datoms) = result.unwrap();
        // Verify decision entity was created
        let has_delib_ref = decision_datoms
            .iter()
            .any(|d| d.attribute.as_str() == ":decision/deliberation");
        assert!(has_delib_ref, "decision must reference deliberation");
        assert_ne!(
            decision_entity, delib,
            "decision entity must differ from deliberation entity"
        );
    }

    /// Verifies: INV-DELIBERATION-002 — decide_with_guard rejects when too few positions.
    /// A deliberation with fewer than 2 positions must be rejected regardless of score.
    #[test]
    fn stability_guard_rejects_insufficient_positions() {
        let agent = test_agent();

        let (delib, delib_datoms) = open_deliberation("guard-single", &[], test_tx(100));
        let (pos, pos_datoms) = add_position(
            delib,
            "only-option",
            "no alternatives",
            &[],
            agent,
            test_tx(200),
        );

        let mut all_datoms = Store::genesis().datom_set().clone();
        for d in delib_datoms.iter().chain(pos_datoms.iter()) {
            all_datoms.insert(d.clone());
        }
        let store = Store::from_datoms(all_datoms);

        // Only 1 position — guard must reject even though score would be 1.0
        let result = decide_with_guard(
            &store,
            delib,
            pos,
            DecisionMethod::Authority,
            "single position",
            test_tx(300),
        );

        assert!(
            result.is_err(),
            "INV-DELIBERATION-002: must reject with only 1 position"
        );
        assert_eq!(result.unwrap_err().position_count, 1);
    }

    /// Verifies: INV-DELIBERATION-002 — majority vote passes guard.
    /// 3 positions with 2 agreeing (score = 0.67) is below STABILITY_MIN,
    /// so the guard must reject it.
    #[test]
    fn stability_guard_rejects_weak_majority() {
        let agent = test_agent();

        let (delib, delib_datoms) = open_deliberation("guard-majority", &[], test_tx(100));
        let (pos_a, pos_a_datoms) =
            add_position(delib, "option-a", "reason 1", &[], agent, test_tx(200));
        let (_, pos_a2_datoms) =
            add_position(delib, "option-a", "reason 2", &[], agent, test_tx(300));
        let (_, pos_b_datoms) =
            add_position(delib, "option-b", "dissent", &[], agent, test_tx(400));

        let mut all_datoms = Store::genesis().datom_set().clone();
        for d in delib_datoms
            .iter()
            .chain(pos_a_datoms.iter())
            .chain(pos_a2_datoms.iter())
            .chain(pos_b_datoms.iter())
        {
            all_datoms.insert(d.clone());
        }
        let store = Store::from_datoms(all_datoms);

        // Stability = 2/3 = 0.667, below STABILITY_MIN (0.7)
        let result = decide_with_guard(
            &store,
            delib,
            pos_a,
            DecisionMethod::Majority,
            "two out of three",
            test_tx(500),
        );

        assert!(
            result.is_err(),
            "INV-DELIBERATION-002: 2/3 majority (0.67) is below STABILITY_MIN (0.7)"
        );
    }

    /// Verifies: decide_with_guard passes with strong majority (>= 0.7).
    /// 4 positions: 3 agree + 1 dissent → score = 0.75 >= 0.7 → pass.
    #[test]
    fn stability_guard_accepts_strong_majority() {
        let agent = test_agent();

        let (delib, delib_datoms) = open_deliberation("guard-strong", &[], test_tx(100));
        let (pos_a, pos_a_datoms) = add_position(delib, "option-a", "r1", &[], agent, test_tx(200));
        let (_, pos_a2_datoms) = add_position(delib, "option-a", "r2", &[], agent, test_tx(300));
        let (_, pos_a3_datoms) = add_position(delib, "option-a", "r3", &[], agent, test_tx(400));
        let (_, pos_b_datoms) =
            add_position(delib, "option-b", "dissent", &[], agent, test_tx(500));

        let mut all_datoms = Store::genesis().datom_set().clone();
        for d in delib_datoms
            .iter()
            .chain(pos_a_datoms.iter())
            .chain(pos_a2_datoms.iter())
            .chain(pos_a3_datoms.iter())
            .chain(pos_b_datoms.iter())
        {
            all_datoms.insert(d.clone());
        }
        let store = Store::from_datoms(all_datoms);

        // Stability = 3/4 = 0.75 >= 0.7 → pass
        let result = decide_with_guard(
            &store,
            delib,
            pos_a,
            DecisionMethod::Majority,
            "three out of four",
            test_tx(600),
        );

        assert!(
            result.is_ok(),
            "INV-DELIBERATION-002: 3/4 majority (0.75) meets STABILITY_MIN (0.7)"
        );
    }

    /// Verifies: STABILITY_MIN constant matches UNC-DELIBERATION-001 default.
    #[test]
    fn stability_min_matches_spec_default() {
        assert!(
            (STABILITY_MIN - 0.7).abs() < f64::EPSILON,
            "STABILITY_MIN must be 0.7 per UNC-DELIBERATION-001"
        );
    }

    /// Verifies: StabilityError provides useful diagnostic information.
    #[test]
    fn stability_error_display() {
        let err = StabilityError {
            deliberation: EntityId::from_ident(":test/delib"),
            score: 0.5,
            required: 0.7,
            position_count: 2,
        };
        let msg = format!("{err}");
        assert!(msg.contains("0.50"), "error message must show score");
        assert!(
            msg.contains("0.70"),
            "error message must show required threshold"
        );
        assert!(
            msg.contains("2 positions"),
            "error message must show position count"
        );
    }
}
