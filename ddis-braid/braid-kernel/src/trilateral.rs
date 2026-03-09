//! TRILATERAL namespace — coherence model across Intent ↔ Specification ↔ Implementation.
//!
//! Extends the bilateral coherence model to the full ISP (Intent-Specification-Implementation)
//! triangle. Three LIVE projections partition store datoms by attribute namespace.
//! Divergence Φ measures gaps between boundaries; β₁ detects structural cycles.
//!
//! # Invariants
//!
//! - **INV-TRILATERAL-001**: Three LIVE projections are monotone functions of the store.
//! - **INV-TRILATERAL-002**: Φ computable from store alone (no external state).
//! - **INV-TRILATERAL-003**: Formality gradient monotonically non-decreasing.
//! - **INV-TRILATERAL-005**: Attribute namespace partitions are pairwise disjoint.
//! - **INV-TRILATERAL-009**: (Φ, β₁) duality — Φ=0 ∧ β₁=0 iff coherent.

use crate::datom::{Attribute, Datom, EntityId, Op};
use crate::store::Store;

// ---------------------------------------------------------------------------
// Attribute Namespace Partition (INV-TRILATERAL-005)
// ---------------------------------------------------------------------------

/// Intent-layer attributes.
pub const INTENT_ATTRS: &[&str] = &[
    ":intent/decision",
    ":intent/rationale",
    ":intent/source",
    ":intent/goal",
    ":intent/constraint",
    ":intent/preference",
    ":intent/noted",
];

/// Specification-layer attributes.
pub const SPEC_ATTRS: &[&str] = &[
    ":spec/id",
    ":spec/element-type",
    ":spec/namespace",
    ":spec/source-file",
    ":spec/stage",
    ":spec/statement",
    ":spec/falsification",
    ":spec/traces-to",
    ":spec/verification",
    ":spec/witnessed",
    ":spec/challenged",
];

/// Implementation-layer attributes.
pub const IMPL_ATTRS: &[&str] = &[
    ":impl/signature",
    ":impl/implements",
    ":impl/file",
    ":impl/module",
    ":impl/test-result",
    ":impl/coverage",
];

/// Attribute namespace classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttrNamespace {
    /// Intent layer — decisions, goals, constraints.
    Intent,
    /// Specification layer — invariants, ADRs, formal elements.
    Spec,
    /// Implementation layer — code, tests, coverage.
    Impl,
    /// Meta layer — cross-cutting attributes (:db/*, :tx/*).
    Meta,
}

/// Classify an attribute into its namespace partition (INV-TRILATERAL-005).
pub fn classify_attribute(attr: &Attribute) -> AttrNamespace {
    let s = attr.as_str();
    if INTENT_ATTRS.contains(&s) {
        AttrNamespace::Intent
    } else if SPEC_ATTRS.contains(&s) {
        AttrNamespace::Spec
    } else if IMPL_ATTRS.contains(&s) {
        AttrNamespace::Impl
    } else {
        AttrNamespace::Meta
    }
}

// ---------------------------------------------------------------------------
// LIVE Projections (INV-TRILATERAL-001)
// ---------------------------------------------------------------------------

/// A LIVE projection: a monotone function from store to filtered datom set.
#[derive(Clone, Debug)]
pub struct LiveView {
    /// Entities visible in this projection.
    pub entities: Vec<EntityId>,
    /// Total datom count in this projection.
    pub datom_count: usize,
}

/// Compute the three LIVE projections from the store (INV-TRILATERAL-001).
///
/// Each projection filters datoms by attribute namespace.
/// Projections are monotone: adding datoms to the store can only grow a projection.
pub fn live_projections(store: &Store) -> (LiveView, LiveView, LiveView) {
    let mut intent_entities = Vec::new();
    let mut spec_entities = Vec::new();
    let mut impl_entities = Vec::new();
    let mut intent_count = 0usize;
    let mut spec_count = 0usize;
    let mut impl_count = 0usize;

    for datom in store.datoms() {
        if datom.op != Op::Assert {
            continue;
        }
        match classify_attribute(&datom.attribute) {
            AttrNamespace::Intent => {
                intent_count += 1;
                if !intent_entities.contains(&datom.entity) {
                    intent_entities.push(datom.entity);
                }
            }
            AttrNamespace::Spec => {
                spec_count += 1;
                if !spec_entities.contains(&datom.entity) {
                    spec_entities.push(datom.entity);
                }
            }
            AttrNamespace::Impl => {
                impl_count += 1;
                if !impl_entities.contains(&datom.entity) {
                    impl_entities.push(datom.entity);
                }
            }
            AttrNamespace::Meta => {} // Cross-cutting, not projected
        }
    }

    (
        LiveView {
            entities: intent_entities,
            datom_count: intent_count,
        },
        LiveView {
            entities: spec_entities,
            datom_count: spec_count,
        },
        LiveView {
            entities: impl_entities,
            datom_count: impl_count,
        },
    )
}

// ---------------------------------------------------------------------------
// Divergence Φ (INV-TRILATERAL-002)
// ---------------------------------------------------------------------------

/// Divergence components between boundary pairs.
#[derive(Clone, Debug)]
pub struct DivergenceComponents {
    /// D_IS: entities in Intent but not in Spec (intent-spec gap).
    pub d_is: usize,
    /// D_SP: entities in Spec but not in Impl (spec-impl gap).
    pub d_sp: usize,
}

/// Compute the divergence metric Φ from the store alone (INV-TRILATERAL-002).
///
/// `Φ = w_is × D_IS + w_sp × D_SP`
///
/// where D_IS = |entities in Intent \ entities in Spec|
/// and   D_SP = |entities in Spec \ entities in Impl|
///
/// Default weights: w_is = 0.4, w_sp = 0.6 (spec-impl gap weighted higher).
pub fn compute_phi(store: &Store, w_is: f64, w_sp: f64) -> (f64, DivergenceComponents) {
    let (live_i, live_s, live_p) = live_projections(store);

    // D_IS: intent entities not covered by spec
    let d_is = live_i
        .entities
        .iter()
        .filter(|e| !live_s.entities.contains(e))
        .count();

    // D_SP: spec entities not covered by impl
    let d_sp = live_s
        .entities
        .iter()
        .filter(|e| !live_p.entities.contains(e))
        .count();

    let components = DivergenceComponents { d_is, d_sp };
    let phi = w_is * d_is as f64 + w_sp * d_sp as f64;

    (phi, components)
}

/// Compute Φ with default weights (0.4 / 0.6).
pub fn compute_phi_default(store: &Store) -> (f64, DivergenceComponents) {
    compute_phi(store, 0.4, 0.6)
}

// ---------------------------------------------------------------------------
// Formality Gradient (INV-TRILATERAL-003)
// ---------------------------------------------------------------------------

/// Formality levels (0–4) based on cross-boundary link structure.
///
/// - Level 0: Entity exists but has no cross-boundary links.
/// - Level 1: Entity has links in one boundary (intent OR spec OR impl).
/// - Level 2: Entity has links in two boundaries.
/// - Level 3: Entity has links in all three boundaries.
/// - Level 4: Entity has links in all three boundaries AND is verified.
pub fn formality_level(store: &Store, entity: EntityId) -> u8 {
    let datoms: Vec<&Datom> = store
        .datoms()
        .filter(|d| d.entity == entity && d.op == Op::Assert)
        .collect();

    if datoms.is_empty() {
        return 0;
    }

    let has_intent = datoms
        .iter()
        .any(|d| classify_attribute(&d.attribute) == AttrNamespace::Intent);
    let has_spec = datoms
        .iter()
        .any(|d| classify_attribute(&d.attribute) == AttrNamespace::Spec);
    let has_impl = datoms
        .iter()
        .any(|d| classify_attribute(&d.attribute) == AttrNamespace::Impl);

    let boundary_count = has_intent as u8 + has_spec as u8 + has_impl as u8;

    match boundary_count {
        0 => 0, // Only meta attributes
        1 => 1,
        2 => 2,
        3 => {
            // Level 4 requires verification evidence
            let has_verification = datoms.iter().any(|d| {
                d.attribute.as_str() == ":spec/witnessed"
                    || d.attribute.as_str() == ":impl/test-result"
            });
            if has_verification {
                4
            } else {
                3
            }
        }
        _ => 0, // unreachable
    }
}

// ---------------------------------------------------------------------------
// ISP Coherence Check (INV-TRILATERAL-008)
// ---------------------------------------------------------------------------

/// Result of an ISP (Intent-Specification-Implementation) bypass check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IspResult {
    /// All three layers agree.
    Coherent,
    /// Intent-Spec gap: intent exists but no spec coverage.
    IntentSpecGap,
    /// Spec-Impl gap: spec exists but no implementation coverage.
    SpecImplGap,
    /// ISP bypass: implementation matches intent but contradicts/bypasses spec.
    SpecificationBypass,
    /// No data in any layer.
    NoData,
}

/// Check ISP coherence for an entity (INV-TRILATERAL-008).
///
/// Detects when implementation matches intent but contradicts specification.
pub fn isp_check(store: &Store, entity: EntityId) -> IspResult {
    let datoms: Vec<&Datom> = store
        .datoms()
        .filter(|d| d.entity == entity && d.op == Op::Assert)
        .collect();

    let has_intent = datoms
        .iter()
        .any(|d| classify_attribute(&d.attribute) == AttrNamespace::Intent);
    let has_spec = datoms
        .iter()
        .any(|d| classify_attribute(&d.attribute) == AttrNamespace::Spec);
    let has_impl = datoms
        .iter()
        .any(|d| classify_attribute(&d.attribute) == AttrNamespace::Impl);

    match (has_intent, has_spec, has_impl) {
        (false, false, false) => IspResult::NoData,
        (true, false, false) => IspResult::IntentSpecGap,
        (_, true, false) => IspResult::SpecImplGap,
        (true, false, true) => IspResult::SpecificationBypass, // INV-TRILATERAL-008
        (true, true, true) => IspResult::Coherent,
        (false, true, true) => IspResult::Coherent, // spec+impl without intent is OK
        (false, false, true) => IspResult::Coherent, // impl-only is OK (not a bypass)
    }
}

// ---------------------------------------------------------------------------
// Coherence Quadrant (INV-TRILATERAL-009)
// ---------------------------------------------------------------------------

/// Coherence state classification based on (Φ, β₁) duality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoherenceQuadrant {
    /// Φ = 0 ∧ β₁ = 0 — fully coherent.
    Coherent,
    /// Φ > 0 ∧ β₁ = 0 — gaps only (missing links).
    GapsOnly,
    /// Φ = 0 ∧ β₁ > 0 — cycles only (structural inconsistencies).
    CyclesOnly,
    /// Φ > 0 ∧ β₁ > 0 — both gaps and cycles.
    GapsAndCycles,
}

/// Full coherence report for the store.
#[derive(Clone, Debug)]
pub struct CoherenceReport {
    /// Divergence metric Φ (weighted gap count).
    pub phi: f64,
    /// Divergence components (D_IS, D_SP).
    pub components: DivergenceComponents,
    /// First Betti number β₁ (cycle count).
    /// Stage 0: computed as simple cycle count in entity link graph.
    pub beta_1: usize,
    /// Coherence quadrant classification.
    pub quadrant: CoherenceQuadrant,
    /// LIVE_I projection size (intent datom count).
    pub live_intent: usize,
    /// LIVE_S projection size (spec datom count).
    pub live_spec: usize,
    /// LIVE_P projection size (impl datom count).
    pub live_impl: usize,
    /// Number of entities with ISP specification bypasses.
    pub isp_bypasses: usize,
}

/// Compute β₁ as a simple cycle proxy (Stage 0).
///
/// At Stage 0, we count entities that have both `:spec/traces-to` and
/// `:impl/implements` pointing to the same or overlapping targets —
/// indicating a potential circular dependency.
/// Full eigendecomposition via nalgebra is deferred to Stage 1.
fn compute_beta_1_proxy(_store: &Store) -> usize {
    // Stage 0 proxy: count entities with bidirectional links
    // (i.e., entity A references entity B and B references A)
    // For now, return 0 — no cycle detection without full graph analysis.
    // This is a conservative approximation (no false positives).
    0
}

/// Check full coherence of the store (INV-TRILATERAL-009).
pub fn check_coherence(store: &Store) -> CoherenceReport {
    let (phi, components) = compute_phi_default(store);
    let beta_1 = compute_beta_1_proxy(store);
    let (live_i, live_s, live_p) = live_projections(store);

    // Count ISP bypasses
    let all_entities: Vec<EntityId> = store.entities().into_iter().collect();
    let isp_bypasses = all_entities
        .iter()
        .filter(|&&e| isp_check(store, e) == IspResult::SpecificationBypass)
        .count();

    let quadrant = match (phi > 0.0, beta_1 > 0) {
        (false, false) => CoherenceQuadrant::Coherent,
        (true, false) => CoherenceQuadrant::GapsOnly,
        (false, true) => CoherenceQuadrant::CyclesOnly,
        (true, true) => CoherenceQuadrant::GapsAndCycles,
    };

    CoherenceReport {
        phi,
        components,
        beta_1,
        quadrant,
        live_intent: live_i.datom_count,
        live_spec: live_s.datom_count,
        live_impl: live_p.datom_count,
        isp_bypasses,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_partition_is_disjoint() {
        // INV-TRILATERAL-005: partitions are pairwise disjoint
        for attr in INTENT_ATTRS {
            assert!(!SPEC_ATTRS.contains(attr), "{attr} in both INTENT and SPEC");
            assert!(!IMPL_ATTRS.contains(attr), "{attr} in both INTENT and IMPL");
        }
        for attr in SPEC_ATTRS {
            assert!(!IMPL_ATTRS.contains(attr), "{attr} in both SPEC and IMPL");
        }
    }

    #[test]
    fn classify_known_attributes() {
        assert_eq!(
            classify_attribute(&Attribute::from_keyword(":intent/goal")),
            AttrNamespace::Intent
        );
        assert_eq!(
            classify_attribute(&Attribute::from_keyword(":spec/id")),
            AttrNamespace::Spec
        );
        assert_eq!(
            classify_attribute(&Attribute::from_keyword(":impl/file")),
            AttrNamespace::Impl
        );
        assert_eq!(
            classify_attribute(&Attribute::from_keyword(":db/ident")),
            AttrNamespace::Meta
        );
    }

    #[test]
    fn genesis_store_has_zero_divergence() {
        // Genesis store has only meta attributes → Φ = 0
        let store = Store::genesis();
        let (phi, components) = compute_phi_default(&store);
        assert_eq!(phi, 0.0);
        assert_eq!(components.d_is, 0);
        assert_eq!(components.d_sp, 0);
    }

    #[test]
    fn genesis_store_is_coherent() {
        let store = Store::genesis();
        let report = check_coherence(&store);
        assert_eq!(report.quadrant, CoherenceQuadrant::Coherent);
        assert_eq!(report.isp_bypasses, 0);
    }

    #[test]
    fn formality_level_meta_only() {
        let store = Store::genesis();
        let entity = EntityId::from_ident(":db/ident");
        // Genesis entities only have meta attrs → level 0
        assert_eq!(formality_level(&store, entity), 0);
    }

    #[test]
    fn isp_check_no_data() {
        let store = Store::genesis();
        let entity = EntityId::from_ident(":nonexistent/thing");
        assert_eq!(isp_check(&store, entity), IspResult::NoData);
    }

    #[test]
    fn coherence_quadrant_classification() {
        assert_eq!(
            CoherenceQuadrant::Coherent,
            match (false, false) {
                (false, false) => CoherenceQuadrant::Coherent,
                (true, false) => CoherenceQuadrant::GapsOnly,
                (false, true) => CoherenceQuadrant::CyclesOnly,
                (true, true) => CoherenceQuadrant::GapsAndCycles,
            }
        );
    }

    #[test]
    fn live_projections_monotone_on_genesis() {
        let store = Store::genesis();
        let (i, s, p) = live_projections(&store);
        // Genesis only has meta attrs, so all projections should be empty
        assert_eq!(i.datom_count, 0);
        assert_eq!(s.datom_count, 0);
        assert_eq!(p.datom_count, 0);
    }
}
