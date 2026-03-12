//! Store-first specification pipeline: promote exploration entities to spec elements.
//!
//! The fundamental insight: instead of writing markdown specs and parsing them into
//! the store (markdown -> store), we invert the flow (store -> markdown). Exploration
//! entities gain `:spec/*` and `:element/*` attributes via promotion, then the store
//! renders them back to spec markdown via `braid generate-spec`.
//!
//! This achieves self-verifying promotion: after promotion, the divergence Phi on the
//! exploration<->spec boundary should be 0.0, because the exploration entity IS the
//! spec entity — they share the same entity ID in the store.
//!
//! # Pipeline
//!
//! 1. **Ingest**: Exploration documents → entities with `:exploration/*` attributes
//! 2. **Promote**: `braid promote` adds `:spec/*` + `:element/*` attributes to entities
//! 3. **Generate**: `braid generate-spec` renders entities back to spec markdown
//! 4. **Verify**: Phi(exploration, spec) = 0 on the promoted boundary
//!
//! # Invariants
//!
//! - **INV-PROMOTE-001**: Promotion never removes exploration attributes (append-only, C1).
//! - **INV-PROMOTE-002**: Every promoted entity has both `:exploration/*` and `:element/*` attrs.
//! - **INV-PROMOTE-003**: Phi on exploration-spec boundary = 0 after successful promotion.
//! - **INV-PROMOTE-004**: Promotion is idempotent — re-promoting an already-promoted entity
//!   produces no new datoms.

use std::collections::BTreeSet;

use crate::datom::{Attribute, Datom, EntityId, Op, TxId, Value};

/// A promotion request: what to promote and where.
#[derive(Clone, Debug)]
pub struct PromotionRequest {
    /// Entity ID of the exploration entity to promote.
    pub entity: EntityId,
    /// Target spec element ID (e.g., "INV-TOPOLOGY-001").
    pub target_element_id: String,
    /// Target namespace (e.g., "TOPOLOGY").
    pub target_namespace: String,
    /// Target element type.
    pub target_type: PromotionTargetType,
    /// Formal statement text (for invariants).
    pub statement: Option<String>,
    /// Falsification condition (for invariants and negative cases).
    pub falsification: Option<String>,
    /// Problem statement (for ADRs).
    pub problem: Option<String>,
    /// Decision text (for ADRs).
    pub decision: Option<String>,
    /// Verification method.
    pub verification: Option<String>,
}

/// Target type for promotion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromotionTargetType {
    /// Promote to an invariant.
    Invariant,
    /// Promote to an architecture decision record.
    Adr,
    /// Promote to a negative case.
    NegativeCase,
}

impl PromotionTargetType {
    /// Keyword for `:element/type`.
    pub fn element_type_keyword(&self) -> &'static str {
        match self {
            PromotionTargetType::Invariant => ":element.type/invariant",
            PromotionTargetType::Adr => ":element.type/adr",
            PromotionTargetType::NegativeCase => ":element.type/negative-case",
        }
    }

    /// Keyword for `:promotion/target-type`.
    pub fn promotion_keyword(&self) -> &'static str {
        match self {
            PromotionTargetType::Invariant => ":element.type/invariant",
            PromotionTargetType::Adr => ":element.type/adr",
            PromotionTargetType::NegativeCase => ":element.type/negative-case",
        }
    }
}

/// Result of a promotion operation.
#[derive(Clone, Debug)]
pub struct PromotionResult {
    /// Datoms produced by the promotion (to be transacted).
    pub datoms: Vec<Datom>,
    /// Number of new attributes added.
    pub attrs_added: usize,
    /// Whether this was a no-op (entity already promoted with same target).
    pub was_noop: bool,
}

/// Check if an entity is already promoted by scanning existing datoms.
pub fn is_already_promoted(entity: EntityId, datoms: &BTreeSet<Datom>) -> bool {
    datoms.iter().any(|d| {
        d.entity == entity
            && d.op == Op::Assert
            && d.attribute.as_str() == ":promotion/status"
            && d.value == Value::Keyword(":promotion.status/promoted".to_string())
    })
}

/// Generate the datoms for promoting an exploration entity to a spec element.
///
/// This is the core of the store-first pipeline. The exploration entity gains
/// new attributes in the `:element/*`, `:spec/*`, and `:promotion/*` namespaces.
/// The entity ID remains the same — the exploration entity IS the spec entity.
///
/// INV-PROMOTE-001: No exploration attributes are removed.
/// INV-PROMOTE-004: If already promoted with same target, returns empty result.
pub fn promote(
    request: &PromotionRequest,
    existing_datoms: &BTreeSet<Datom>,
    tx: TxId,
) -> PromotionResult {
    // INV-PROMOTE-004: Idempotency check
    if is_already_promoted(request.entity, existing_datoms) {
        // Check if already promoted to the same target
        let same_target = existing_datoms.iter().any(|d| {
            d.entity == request.entity
                && d.op == Op::Assert
                && d.attribute.as_str() == ":promotion/target-element"
                && d.value == Value::String(request.target_element_id.clone())
        });
        if same_target {
            return PromotionResult {
                datoms: Vec::new(),
                attrs_added: 0,
                was_noop: true,
            };
        }
    }

    let e = request.entity;
    let mut datoms = Vec::new();

    // --- Element identity attributes ---
    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":element/id"),
        Value::String(request.target_element_id.clone()),
        tx,
        Op::Assert,
    ));

    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":element/type"),
        Value::Keyword(request.target_type.element_type_keyword().to_string()),
        tx,
        Op::Assert,
    ));

    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":element/namespace"),
        Value::Keyword(format!(
            ":element.ns/{}",
            request.target_namespace.to_lowercase()
        )),
        tx,
        Op::Assert,
    ));

    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":element/status"),
        Value::Keyword(":element.status/active".to_string()),
        tx,
        Op::Assert,
    ));

    // --- Type-specific attributes ---
    match request.target_type {
        PromotionTargetType::Invariant => {
            if let Some(ref stmt) = request.statement {
                datoms.push(Datom::new(
                    e,
                    Attribute::from_keyword(":inv/statement"),
                    Value::String(stmt.clone()),
                    tx,
                    Op::Assert,
                ));
            }
            if let Some(ref fals) = request.falsification {
                datoms.push(Datom::new(
                    e,
                    Attribute::from_keyword(":inv/falsification"),
                    Value::String(fals.clone()),
                    tx,
                    Op::Assert,
                ));
            }
            if let Some(ref ver) = request.verification {
                datoms.push(Datom::new(
                    e,
                    Attribute::from_keyword(":inv/verification"),
                    Value::String(ver.clone()),
                    tx,
                    Op::Assert,
                ));
            }
        }
        PromotionTargetType::Adr => {
            if let Some(ref prob) = request.problem {
                datoms.push(Datom::new(
                    e,
                    Attribute::from_keyword(":adr/problem"),
                    Value::String(prob.clone()),
                    tx,
                    Op::Assert,
                ));
            }
            if let Some(ref dec) = request.decision {
                datoms.push(Datom::new(
                    e,
                    Attribute::from_keyword(":adr/decision"),
                    Value::String(dec.clone()),
                    tx,
                    Op::Assert,
                ));
            }
        }
        PromotionTargetType::NegativeCase => {
            if let Some(ref fals) = request.falsification {
                datoms.push(Datom::new(
                    e,
                    Attribute::from_keyword(":neg/violation"),
                    Value::String(fals.clone()),
                    tx,
                    Op::Assert,
                ));
            }
        }
    }

    // --- Promotion lifecycle attributes ---
    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":promotion/status"),
        Value::Keyword(":promotion.status/promoted".to_string()),
        tx,
        Op::Assert,
    ));

    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":promotion/target-element"),
        Value::String(request.target_element_id.clone()),
        tx,
        Op::Assert,
    ));

    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":promotion/target-namespace"),
        Value::Keyword(format!(
            ":element.ns/{}",
            request.target_namespace.to_lowercase()
        )),
        tx,
        Op::Assert,
    ));

    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":promotion/target-type"),
        Value::Keyword(request.target_type.promotion_keyword().to_string()),
        tx,
        Op::Assert,
    ));

    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":promotion/promoted-tx"),
        Value::Ref(EntityId::from_ident(&format!(
            ":tx/{}-{}",
            tx.wall_time(),
            tx.logical()
        ))),
        tx,
        Op::Assert,
    ));

    let attrs_added = datoms.len();

    PromotionResult {
        datoms,
        attrs_added,
        was_noop: false,
    }
}

/// Verify that an entity satisfies INV-PROMOTE-002: has both exploration and element attrs.
pub fn verify_dual_identity(entity: EntityId, datoms: &BTreeSet<Datom>) -> DualIdentityCheck {
    let mut has_exploration = false;
    let mut has_element = false;
    let mut has_promotion = false;

    for d in datoms
        .iter()
        .filter(|d| d.entity == entity && d.op == Op::Assert)
    {
        let ns = d.attribute.namespace();
        match ns {
            "exploration" => has_exploration = true,
            "element" => has_element = true,
            "promotion" => has_promotion = true,
            _ => {}
        }
    }

    DualIdentityCheck {
        entity,
        has_exploration,
        has_element,
        has_promotion,
        is_valid: has_exploration && has_element && has_promotion,
    }
}

/// Result of dual identity verification.
#[derive(Clone, Debug)]
pub struct DualIdentityCheck {
    /// The entity being checked.
    pub entity: EntityId,
    /// Whether the entity has `:exploration/*` attributes.
    pub has_exploration: bool,
    /// Whether the entity has `:element/*` attributes.
    pub has_element: bool,
    /// Whether the entity has `:promotion/*` attributes.
    pub has_promotion: bool,
    /// Whether INV-PROMOTE-002 holds.
    pub is_valid: bool,
}

/// Batch promotion: promote multiple exploration entities at once.
///
/// Returns all datoms for the batch (to be transacted together) and a summary.
pub fn promote_batch(
    requests: &[PromotionRequest],
    existing_datoms: &BTreeSet<Datom>,
    tx: TxId,
) -> BatchPromotionResult {
    let mut all_datoms = Vec::new();
    let mut promoted = 0;
    let mut skipped = 0;

    for req in requests {
        let result = promote(req, existing_datoms, tx);
        if result.was_noop {
            skipped += 1;
        } else {
            promoted += 1;
            all_datoms.extend(result.datoms);
        }
    }

    BatchPromotionResult {
        datoms: all_datoms,
        promoted,
        skipped,
    }
}

/// Result of a batch promotion.
#[derive(Clone, Debug)]
pub struct BatchPromotionResult {
    /// All datoms produced (to be transacted).
    pub datoms: Vec<Datom>,
    /// Number of entities successfully promoted.
    pub promoted: usize,
    /// Number of entities skipped (already promoted).
    pub skipped: usize,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// Witnesses: INV-STORE-001, INV-STORE-003, INV-SCHEMA-001, ADR-STORE-003,
//   ADR-SCHEMA-001, NEG-STORE-001
// (Promotion exercises append-only datom construction, content-addressable identity,
//  schema-as-data patterns, and preserves exploration attributes per C1.)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::AgentId;

    fn test_tx() -> TxId {
        let agent = AgentId::from_name("test:promote");
        TxId::new(10, 0, agent)
    }

    fn exploration_entity(id: &str) -> (EntityId, BTreeSet<Datom>) {
        let e = EntityId::from_ident(&format!(":exploration/{id}"));
        let tx = test_tx();
        let mut datoms = BTreeSet::new();

        datoms.insert(Datom::new(
            e,
            Attribute::from_keyword(":exploration/id"),
            Value::String(format!("EXPL-{id}")),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            e,
            Attribute::from_keyword(":exploration/title"),
            Value::String(format!("Test exploration {id}")),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            e,
            Attribute::from_keyword(":exploration/category"),
            Value::Keyword(":exploration.cat/theorem".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            e,
            Attribute::from_keyword(":exploration/confidence"),
            Value::Double(0.9.into()),
            tx,
            Op::Assert,
        ));

        (e, datoms)
    }

    // Verifies: INV-STORE-001, INV-STORE-003, INV-SCHEMA-001, ADR-STORE-003
    // (Promotion adds element/promotion attrs as new datoms — append-only, content-addressed.)
    #[test]
    fn promote_adds_element_and_promotion_attrs() {
        let (entity, existing) = exploration_entity("topo-001");
        let agent = AgentId::from_name("test:promote");
        let promote_tx = TxId::new(11, 0, agent);

        let req = PromotionRequest {
            entity,
            target_element_id: "INV-TOPOLOGY-001".to_string(),
            target_namespace: "TOPOLOGY".to_string(),
            target_type: PromotionTargetType::Invariant,
            statement: Some("The topology T is a 4-tuple.".to_string()),
            falsification: Some("Any topology lacking G, Phi, Sigma, or Pi.".to_string()),
            verification: Some("V:PROP".to_string()),
            problem: None,
            decision: None,
        };

        let result = promote(&req, &existing, promote_tx);
        assert!(!result.was_noop);
        assert!(result.attrs_added >= 10); // element(4) + inv(3) + promotion(5)

        // Verify all datoms use the same entity ID (INV-PROMOTE-002 foundation)
        for d in &result.datoms {
            assert_eq!(
                d.entity, entity,
                "all promotion datoms must use same entity"
            );
        }

        // Check element/id was set
        assert!(result
            .datoms
            .iter()
            .any(|d| d.attribute.as_str() == ":element/id"
                && d.value == Value::String("INV-TOPOLOGY-001".to_string())));

        // Check promotion/status was set
        assert!(result
            .datoms
            .iter()
            .any(|d| d.attribute.as_str() == ":promotion/status"
                && d.value == Value::Keyword(":promotion.status/promoted".to_string())));
    }

    // Verifies: INV-STORE-001, INV-STORE-006
    // (Idempotent re-promotion produces no new datoms — append-only store is unchanged.)
    #[test]
    fn promote_is_idempotent() {
        let (entity, mut existing) = exploration_entity("topo-002");
        let agent = AgentId::from_name("test:promote");
        let promote_tx = TxId::new(11, 0, agent);

        let req = PromotionRequest {
            entity,
            target_element_id: "INV-TOPOLOGY-002".to_string(),
            target_namespace: "TOPOLOGY".to_string(),
            target_type: PromotionTargetType::Invariant,
            statement: None,
            falsification: None,
            verification: None,
            problem: None,
            decision: None,
        };

        // First promotion
        let result1 = promote(&req, &existing, promote_tx);
        assert!(!result1.was_noop);

        // Add promotion datoms to existing set
        for d in &result1.datoms {
            existing.insert(d.clone());
        }

        // Second promotion — should be idempotent (INV-PROMOTE-004)
        let result2 = promote(&req, &existing, promote_tx);
        assert!(result2.was_noop);
        assert_eq!(result2.datoms.len(), 0);
    }

    // Verifies: INV-STORE-001, ADR-STORE-003, ADR-SCHEMA-001
    // (ADR promotion uses schema-as-data attributes — :adr/problem, :adr/decision.)
    #[test]
    fn promote_adr_adds_adr_specific_attrs() {
        let (entity, existing) = exploration_entity("adr-001");
        let agent = AgentId::from_name("test:promote");
        let promote_tx = TxId::new(11, 0, agent);

        let req = PromotionRequest {
            entity,
            target_element_id: "ADR-TOPOLOGY-001".to_string(),
            target_namespace: "TOPOLOGY".to_string(),
            target_type: PromotionTargetType::Adr,
            statement: None,
            falsification: None,
            verification: None,
            problem: Some("How should topology transitions be coordinated?".to_string()),
            decision: Some("Use CALM stratification.".to_string()),
        };

        let result = promote(&req, &existing, promote_tx);
        assert!(!result.was_noop);

        // Check ADR-specific attributes
        assert!(result
            .datoms
            .iter()
            .any(|d| d.attribute.as_str() == ":adr/problem"));
        assert!(result
            .datoms
            .iter()
            .any(|d| d.attribute.as_str() == ":adr/decision"));
    }

    // Verifies: INV-TRILATERAL-001, INV-TRILATERAL-002
    // (Dual identity verification — entity has both exploration and element attributes.)
    #[test]
    fn verify_dual_identity_after_promotion() {
        let (entity, mut existing) = exploration_entity("dual-001");
        let agent = AgentId::from_name("test:promote");
        let promote_tx = TxId::new(11, 0, agent);

        // Before promotion: has exploration but not element
        let check_before = verify_dual_identity(entity, &existing);
        assert!(check_before.has_exploration);
        assert!(!check_before.has_element);
        assert!(!check_before.is_valid);

        // Promote
        let req = PromotionRequest {
            entity,
            target_element_id: "INV-TOPOLOGY-003".to_string(),
            target_namespace: "TOPOLOGY".to_string(),
            target_type: PromotionTargetType::Invariant,
            statement: None,
            falsification: None,
            verification: None,
            problem: None,
            decision: None,
        };
        let result = promote(&req, &existing, promote_tx);
        for d in &result.datoms {
            existing.insert(d.clone());
        }

        // After promotion: has both (INV-PROMOTE-002)
        let check_after = verify_dual_identity(entity, &existing);
        assert!(check_after.has_exploration);
        assert!(check_after.has_element);
        assert!(check_after.has_promotion);
        assert!(check_after.is_valid);
    }

    // Verifies: INV-STORE-001, INV-STORE-003
    // (Batch promotion transacts multiple entities at once — append-only, content-addressed.)
    #[test]
    fn batch_promotion() {
        let (e1, existing1) = exploration_entity("batch-001");
        let (e2, existing2) = exploration_entity("batch-002");

        let mut all_existing: BTreeSet<Datom> = BTreeSet::new();
        all_existing.extend(existing1);
        all_existing.extend(existing2);

        let agent = AgentId::from_name("test:promote");
        let promote_tx = TxId::new(11, 0, agent);

        let requests = vec![
            PromotionRequest {
                entity: e1,
                target_element_id: "INV-TOPOLOGY-010".to_string(),
                target_namespace: "TOPOLOGY".to_string(),
                target_type: PromotionTargetType::Invariant,
                statement: None,
                falsification: None,
                verification: None,
                problem: None,
                decision: None,
            },
            PromotionRequest {
                entity: e2,
                target_element_id: "ADR-TOPOLOGY-001".to_string(),
                target_namespace: "TOPOLOGY".to_string(),
                target_type: PromotionTargetType::Adr,
                statement: None,
                falsification: None,
                verification: None,
                problem: None,
                decision: None,
            },
        ];

        let result = promote_batch(&requests, &all_existing, promote_tx);
        assert_eq!(result.promoted, 2);
        assert_eq!(result.skipped, 0);
        assert!(!result.datoms.is_empty());
    }

    // Verifies: INV-STORE-001, ADR-STORE-003, ADR-SCHEMA-001
    // (Negative case promotion uses schema-as-data attributes — :neg/violation.)
    #[test]
    fn promote_neg_adds_violation_attr() {
        let (entity, existing) = exploration_entity("neg-001");
        let agent = AgentId::from_name("test:promote");
        let promote_tx = TxId::new(11, 0, agent);

        let req = PromotionRequest {
            entity,
            target_element_id: "NEG-TOPOLOGY-001".to_string(),
            target_namespace: "TOPOLOGY".to_string(),
            target_type: PromotionTargetType::NegativeCase,
            statement: None,
            falsification: Some("Direct topology mutation without protocol.".to_string()),
            verification: None,
            problem: None,
            decision: None,
        };

        let result = promote(&req, &existing, promote_tx);
        assert!(result
            .datoms
            .iter()
            .any(|d| d.attribute.as_str() == ":neg/violation"));
    }
}
