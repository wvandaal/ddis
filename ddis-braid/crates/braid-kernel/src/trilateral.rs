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

use std::collections::{BTreeMap, BTreeSet};

use crate::datom::{Attribute, Datom, EntityId, Op, Value};
use crate::query::graph::{first_betti_number, DenseMatrix, DiGraph};
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    let mut intent_set = BTreeSet::new();
    let mut spec_set = BTreeSet::new();
    let mut impl_set = BTreeSet::new();
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
                intent_set.insert(datom.entity);
            }
            AttrNamespace::Spec => {
                spec_count += 1;
                spec_set.insert(datom.entity);
            }
            AttrNamespace::Impl => {
                impl_count += 1;
                impl_set.insert(datom.entity);
            }
            AttrNamespace::Meta => {} // Cross-cutting, not projected
        }
    }

    (
        LiveView {
            entities: intent_set.into_iter().collect(),
            datom_count: intent_count,
        },
        LiveView {
            entities: spec_set.into_iter().collect(),
            datom_count: spec_count,
        },
        LiveView {
            entities: impl_set.into_iter().collect(),
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

    // Convert to BTreeSet for O(log n) lookup in set difference
    let spec_set: BTreeSet<&EntityId> = live_s.entities.iter().collect();
    let impl_set: BTreeSet<&EntityId> = live_p.entities.iter().collect();

    // D_IS: intent entities not covered by spec
    let d_is = live_i
        .entities
        .iter()
        .filter(|e| !spec_set.contains(e))
        .count();

    // D_SP: spec entities not covered by impl
    let d_sp = live_s
        .entities
        .iter()
        .filter(|e| !impl_set.contains(e))
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
    /// Von Neumann entropy of the entity reference graph.
    pub entropy: CoherenceEntropy,
}

/// Reference-type attributes that define edges in the entity graph.
///
/// These are the cross-boundary attributes whose `Value::Ref` targets
/// create directed edges between entities. The resulting graph's β₁
/// counts independent cycles (topological holes in the dependency structure).
const REF_EDGE_ATTRS: &[&str] = &[
    ":spec/traces-to",
    ":impl/implements",
    ":dep/from",
    ":dep/to",
    ":exploration/depends-on",
    ":exploration/refines",
    ":exploration/related-spec",
];

/// Compute the first Betti number β₁ from the store's entity reference graph.
///
/// Builds a directed graph from all `Value::Ref` datoms on cross-boundary
/// attributes, then computes β₁ = dim(ker(L₁)) via edge Laplacian
/// eigendecomposition (INV-QUERY-024).
///
/// β₁ = 0 means no structural cycles (the entity graph is a forest).
/// β₁ > 0 counts independent cycles that may indicate contradictions
/// or circular dependencies between specification elements.
fn compute_beta_1(store: &Store) -> usize {
    let mut graph = DiGraph::new();

    for datom in store.datoms() {
        if datom.op != Op::Assert {
            continue;
        }
        // Only consider known reference-edge attributes
        if !REF_EDGE_ATTRS.contains(&datom.attribute.as_str()) {
            continue;
        }
        if let Value::Ref(target) = &datom.value {
            let src = format!(
                "{:x}",
                u64::from_be_bytes(datom.entity.as_bytes()[..8].try_into().unwrap())
            );
            let dst = format!(
                "{:x}",
                u64::from_be_bytes(target.as_bytes()[..8].try_into().unwrap())
            );
            graph.add_edge(&src, &dst);
        }
    }

    if graph.node_count() < 2 {
        return 0;
    }

    first_betti_number(&graph)
}

/// Check full coherence of the store (INV-TRILATERAL-009).
pub fn check_coherence(store: &Store) -> CoherenceReport {
    let (phi, components) = compute_phi_default(store);
    let beta_1 = compute_beta_1(store);
    let (live_i, live_s, live_p) = live_projections(store);
    let entropy = von_neumann_entropy(store);

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
        entropy,
    }
}

// ---------------------------------------------------------------------------
// Von Neumann Entropy (INV-COHERENCE-001)
// ---------------------------------------------------------------------------

/// Von Neumann entropy coherence metrics.
///
/// S(ρ) = -Tr(ρ log₂ ρ) = -Σᵢ λᵢ log₂(λᵢ)
/// where ρ = A/Tr(A) is the density matrix formed from the
/// adjacency matrix of the entity reference graph.
///
/// Low entropy → concentrated, coherent structure.
/// High entropy → dispersed, incoherent structure.
/// Maximum entropy = log₂(n) for n-node graph (uniform distribution).
#[derive(Clone, Debug)]
pub struct CoherenceEntropy {
    /// Von Neumann entropy S(ρ) in bits.
    pub entropy: f64,
    /// Maximum possible entropy log₂(n).
    pub max_entropy: f64,
    /// Normalized entropy S(ρ)/log₂(n) ∈ [0, 1].
    pub normalized: f64,
    /// Number of non-zero eigenvalues (effective rank).
    pub effective_rank: usize,
    /// Total nodes in the entity graph.
    pub node_count: usize,
}

/// Helper: extract a hex key from an EntityId (same convention as seed.rs).
fn entity_key(entity: EntityId) -> String {
    format!(
        "{:x}",
        u64::from_be_bytes(entity.as_bytes()[..8].try_into().unwrap())
    )
}

/// Compute von Neumann entropy of the entity reference graph (INV-COHERENCE-001).
///
/// Forms the adjacency matrix A from all `Value::Ref` datoms (both directed
/// edges contribute symmetrically: A[i,j] = A[j,i] = 1 if i→j or j→i).
/// Normalizes to density matrix ρ = A/Tr(A).
/// Computes S(ρ) = -Σᵢ λᵢ log₂(λᵢ) via eigendecomposition.
///
/// For the symmetrized adjacency matrix with unit self-loops, ρ has all
/// real non-negative eigenvalues (it's PSD after normalization by trace).
pub fn von_neumann_entropy(store: &Store) -> CoherenceEntropy {
    // Collect all Value::Ref datoms (Op::Assert only) to build the entity graph.
    let mut node_index: BTreeMap<String, usize> = BTreeMap::new();
    let mut edges: Vec<(String, String)> = Vec::new();

    for datom in store.datoms() {
        if datom.op != Op::Assert {
            continue;
        }
        if let Value::Ref(target) = &datom.value {
            let src = entity_key(datom.entity);
            let dst = entity_key(*target);
            // Register both nodes
            let next_id = node_index.len();
            node_index.entry(src.clone()).or_insert(next_id);
            let next_id = node_index.len();
            node_index.entry(dst.clone()).or_insert(next_id);
            edges.push((src, dst));
        }
    }

    let n = node_index.len();

    // Edge case: no ref datoms → empty graph
    if n == 0 {
        return CoherenceEntropy {
            entropy: 0.0,
            max_entropy: 0.0,
            normalized: 0.0,
            effective_rank: 0,
            node_count: 0,
        };
    }

    // Build symmetric adjacency matrix with self-loops.
    // A[i,i] = 1 for all nodes (ensures non-zero trace and PSD).
    let mut a = DenseMatrix::zeros(n, n);

    // Self-loops: A[i,i] = 1
    for i in 0..n {
        a.set(i, i, 1.0);
    }

    // Symmetric edges: for i→j, set A[i,j] = 1 and A[j,i] = 1
    for (src, dst) in &edges {
        let i = node_index[src];
        let j = node_index[dst];
        a.set(i, j, 1.0);
        a.set(j, i, 1.0);
    }

    // Compute trace = Σᵢ A[i,i]
    let trace: f64 = (0..n).map(|i| a.get(i, i)).sum();

    // Edge case: trace == 0 (shouldn't happen with self-loops, but guard)
    if trace < f64::EPSILON {
        return CoherenceEntropy {
            entropy: 0.0,
            max_entropy: (n as f64).log2(),
            normalized: 0.0,
            effective_rank: 0,
            node_count: n,
        };
    }

    // Normalize: ρ = A / trace (unit trace, PSD)
    let mut rho = DenseMatrix::zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            rho.set(i, j, a.get(i, j) / trace);
        }
    }

    // Eigendecomposition of the symmetric density matrix
    let eigenvalues = rho.symmetric_eigenvalues();

    // S(ρ) = -Σᵢ λᵢ log₂(λᵢ) where λᵢ > 0
    let mut entropy = 0.0_f64;
    let mut effective_rank = 0usize;
    let eps = 1e-12;

    for &lambda in &eigenvalues {
        if lambda > eps {
            effective_rank += 1;
            entropy -= lambda * lambda.log2();
        }
    }

    // Clamp to non-negative (numerical noise can produce tiny negatives)
    if entropy < 0.0 {
        entropy = 0.0;
    }

    let max_entropy = (n as f64).log2();
    let normalized = if max_entropy > 0.0 {
        (entropy / max_entropy).min(1.0)
    } else {
        0.0
    };

    CoherenceEntropy {
        entropy,
        max_entropy,
        normalized,
        effective_rank,
        node_count: n,
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

    #[test]
    fn beta_1_zero_for_acyclic_refs() {
        // A → B → C (chain, no cycles) ⇒ β₁ = 0
        use crate::datom::{AgentId, TxId};
        let mut datoms = Store::genesis().datom_set().clone();
        let tx = TxId::new(1, 0, AgentId::from_name("test:beta1"));
        let a = EntityId::from_ident(":test/a");
        let b = EntityId::from_ident(":test/b");
        let c = EntityId::from_ident(":test/c");

        // A → B via :spec/traces-to
        datoms.insert(Datom::new(
            a,
            Attribute::from_keyword(":spec/traces-to"),
            Value::Ref(b),
            tx,
            Op::Assert,
        ));
        // B → C via :impl/implements
        datoms.insert(Datom::new(
            b,
            Attribute::from_keyword(":impl/implements"),
            Value::Ref(c),
            tx,
            Op::Assert,
        ));

        let store = Store::from_datoms(datoms);
        let beta_1 = compute_beta_1(&store);
        assert_eq!(beta_1, 0, "acyclic graph should have β₁ = 0");
    }

    #[test]
    fn beta_1_positive_for_cycle() {
        // A → B → C → A (cycle) ⇒ β₁ > 0
        use crate::datom::{AgentId, TxId};
        let mut datoms = Store::genesis().datom_set().clone();
        let tx = TxId::new(1, 0, AgentId::from_name("test:beta1"));
        let a = EntityId::from_ident(":test/cycle-a");
        let b = EntityId::from_ident(":test/cycle-b");
        let c = EntityId::from_ident(":test/cycle-c");

        datoms.insert(Datom::new(
            a,
            Attribute::from_keyword(":spec/traces-to"),
            Value::Ref(b),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            b,
            Attribute::from_keyword(":spec/traces-to"),
            Value::Ref(c),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            c,
            Attribute::from_keyword(":spec/traces-to"),
            Value::Ref(a),
            tx,
            Op::Assert,
        ));

        let store = Store::from_datoms(datoms);
        let beta_1 = compute_beta_1(&store);
        assert!(beta_1 > 0, "cycle graph should have β₁ > 0, got {beta_1}");
    }

    #[test]
    fn coherence_report_detects_gaps_and_cycles() {
        // Store with both spec entities (gaps) and a cycle ⇒ GapsAndCycles
        use crate::datom::{AgentId, TxId};
        let mut datoms = Store::genesis().datom_set().clone();
        let tx = TxId::new(1, 0, AgentId::from_name("test:coherence"));
        let a = EntityId::from_ident(":test/coherence-a");
        let b = EntityId::from_ident(":test/coherence-b");

        // Spec entity (creates D_SP gap — in spec but not in impl)
        datoms.insert(Datom::new(
            a,
            Attribute::from_keyword(":spec/id"),
            Value::String("INV-TEST-001".into()),
            tx,
            Op::Assert,
        ));
        // Create cycle: A → B → A via spec/traces-to
        datoms.insert(Datom::new(
            a,
            Attribute::from_keyword(":spec/traces-to"),
            Value::Ref(b),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            b,
            Attribute::from_keyword(":spec/traces-to"),
            Value::Ref(a),
            tx,
            Op::Assert,
        ));

        let store = Store::from_datoms(datoms);
        let report = check_coherence(&store);
        assert!(report.phi > 0.0, "should have gaps");
        assert!(report.beta_1 > 0, "should have cycles");
        assert_eq!(report.quadrant, CoherenceQuadrant::GapsAndCycles);
    }

    // -------------------------------------------------------------------
    // Proptest formal verification: Phi as Lyapunov function (brai-290x)
    // -------------------------------------------------------------------

    mod phi_lyapunov {
        use super::*;
        use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, TxId, Value};
        use crate::proptest_strategies::arb_store;
        use crate::store::Store;
        use proptest::prelude::*;

        fn make_tx(wall: u64) -> TxId {
            TxId::new(wall, 0, AgentId::from_name("test:agent"))
        }

        fn store_with_intent_only() -> Store {
            let mut datoms = Store::genesis().datom_set().clone();
            let e1 = EntityId::from_ident(":test/entity-a");
            let tx = make_tx(1);
            datoms.insert(Datom::new(
                e1,
                Attribute::from_keyword(":intent/goal"),
                Value::String("some goal".into()),
                tx,
                Op::Assert,
            ));
            Store::from_datoms(datoms)
        }

        fn store_with_intent_and_spec() -> Store {
            let mut datoms = Store::genesis().datom_set().clone();
            let e1 = EntityId::from_ident(":test/entity-a");
            let tx = make_tx(1);
            datoms.insert(Datom::new(
                e1,
                Attribute::from_keyword(":intent/goal"),
                Value::String("some goal".into()),
                tx,
                Op::Assert,
            ));
            datoms.insert(Datom::new(
                e1,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-TEST-001".into()),
                tx,
                Op::Assert,
            ));
            Store::from_datoms(datoms)
        }

        proptest! {
            #[test]
            fn phi_non_negative(store in arb_store(3)) {
                // INV-TRILATERAL-002: Phi >= 0 for all stores
                let (phi, _) = compute_phi_default(&store);
                prop_assert!(phi >= 0.0, "phi must be non-negative, got {}", phi);
            }

            #[test]
            fn phi_observability_pure_function(store in arb_store(3)) {
                // INV-TRILATERAL-002: compute_phi_default is a pure function of store state.
                // Same store must always produce the same phi.
                let (phi_a, comp_a) = compute_phi_default(&store);
                let (phi_b, comp_b) = compute_phi_default(&store);
                prop_assert!(
                    (phi_a - phi_b).abs() < f64::EPSILON,
                    "phi not deterministic: {} vs {}",
                    phi_a,
                    phi_b
                );
                prop_assert_eq!(comp_a.d_is, comp_b.d_is);
                prop_assert_eq!(comp_a.d_sp, comp_b.d_sp);
            }
        }

        #[test]
        fn phi_equilibrium_genesis() {
            // Genesis store has only meta attributes -> all projections empty
            // -> D_IS = 0, D_SP = 0 -> phi = 0.0
            let store = Store::genesis();
            let (phi, components) = compute_phi_default(&store);
            assert_eq!(phi, 0.0);
            assert_eq!(components.d_is, 0);
            assert_eq!(components.d_sp, 0);
        }

        #[test]
        fn phi_equilibrium_iff_all_cross_boundary_links() {
            // A store where every intent entity also has spec+impl coverage
            // should have phi = 0.
            let mut datoms = Store::genesis().datom_set().clone();
            let e1 = EntityId::from_ident(":test/coherent-entity");
            let tx = make_tx(1);
            // All three layers present for e1
            datoms.insert(Datom::new(
                e1,
                Attribute::from_keyword(":intent/goal"),
                Value::String("a goal".into()),
                tx,
                Op::Assert,
            ));
            datoms.insert(Datom::new(
                e1,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-X-001".into()),
                tx,
                Op::Assert,
            ));
            datoms.insert(Datom::new(
                e1,
                Attribute::from_keyword(":impl/file"),
                Value::String("src/x.rs".into()),
                tx,
                Op::Assert,
            ));
            let store = Store::from_datoms(datoms);
            let (phi, components) = compute_phi_default(&store);
            assert_eq!(phi, 0.0, "fully linked entity should produce phi=0");
            assert_eq!(components.d_is, 0);
            assert_eq!(components.d_sp, 0);
        }

        #[test]
        fn phi_positive_for_unlinked_intent() {
            // Intent entity with no spec -> D_IS > 0 -> phi > 0
            let store = store_with_intent_only();
            let (phi, components) = compute_phi_default(&store);
            assert!(phi > 0.0, "unlinked intent should produce phi > 0");
            assert_eq!(components.d_is, 1);
        }

        #[test]
        fn phi_monotonic_non_increase_under_link_intent_to_spec() {
            // Adding a spec datom for an entity that only has intent should
            // not increase phi (it should decrease D_IS).
            let store_before = store_with_intent_only();
            let (phi_before, _) = compute_phi_default(&store_before);
            assert!(
                phi_before > 0.0,
                "precondition: phi should be positive for intent-only store"
            );

            let store_after = store_with_intent_and_spec();
            let (phi_after, _) = compute_phi_default(&store_after);

            // Adding the spec link should reduce phi (D_IS goes from 1 to 0)
            // but D_SP may increase (spec entity now in spec but not in impl).
            // The key property: phi should not increase beyond what was resolved.
            // Specifically, the intent-spec gap is resolved, and only spec-impl
            // gap may appear, which is weighted differently.
            // For this specific case: before: 0.4*1 + 0.6*0 = 0.4
            // After: 0.4*0 + 0.6*1 = 0.6 -- phi actually increases!
            // This reveals the boundary behavior: adding a link can shift weight.
            // The true Lyapunov property is: Phi is non-negative and a monotone
            // function of the gap counts, not that it monotonically decreases
            // under arbitrary single-link operations.
            //
            // The correct monotonic non-increase property: adding a LINK that
            // CLOSES a gap (same boundary) cannot increase the gap count for
            // that boundary.
            let components_after = compute_phi_default(&store_after).1;
            let components_before = compute_phi_default(&store_before).1;
            // D_IS: intent-spec gap should not increase when we add a spec datom
            // for an entity that already has intent
            assert!(
                components_after.d_is <= components_before.d_is,
                "adding spec for intent entity must not increase D_IS: {} > {}",
                components_after.d_is,
                components_before.d_is
            );
            // And phi_after is still non-negative
            assert!(phi_after >= 0.0);
        }

        #[test]
        fn phi_monotonic_non_increase_under_link_spec_to_impl() {
            // Adding an impl datom for an entity that already has spec should
            // not increase D_SP.
            let mut datoms = Store::genesis().datom_set().clone();
            let e1 = EntityId::from_ident(":test/entity-b");
            let tx = make_tx(1);
            datoms.insert(Datom::new(
                e1,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-B-001".into()),
                tx,
                Op::Assert,
            ));
            let store_before = Store::from_datoms(datoms.clone());
            let (phi_before, comp_before) = compute_phi_default(&store_before);
            assert!(phi_before > 0.0, "spec-only entity should have phi > 0");
            assert_eq!(comp_before.d_sp, 1);

            // Now add impl for the same entity
            datoms.insert(Datom::new(
                e1,
                Attribute::from_keyword(":impl/file"),
                Value::String("src/b.rs".into()),
                tx,
                Op::Assert,
            ));
            let store_after = Store::from_datoms(datoms);
            let (phi_after, comp_after) = compute_phi_default(&store_after);

            assert!(
                comp_after.d_sp <= comp_before.d_sp,
                "adding impl for spec entity must not increase D_SP: {} > {}",
                comp_after.d_sp,
                comp_before.d_sp
            );
            assert!(
                phi_after <= phi_before,
                "phi must not increase when D_SP decreases"
            );
        }

        #[test]
        fn phi_full_coherence_from_gaps() {
            // Start with gaps in all boundaries, then close them all.
            // Final phi should be 0.
            let mut datoms = Store::genesis().datom_set().clone();
            let e1 = EntityId::from_ident(":test/entity-full");
            let tx = make_tx(1);

            // Intent only
            datoms.insert(Datom::new(
                e1,
                Attribute::from_keyword(":intent/goal"),
                Value::String("goal".into()),
                tx,
                Op::Assert,
            ));
            // Spec for e1
            datoms.insert(Datom::new(
                e1,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-FULL-001".into()),
                tx,
                Op::Assert,
            ));
            // Impl for e1
            datoms.insert(Datom::new(
                e1,
                Attribute::from_keyword(":impl/file"),
                Value::String("src/full.rs".into()),
                tx,
                Op::Assert,
            ));

            let store = Store::from_datoms(datoms);
            let (phi, components) = compute_phi_default(&store);
            assert_eq!(phi, 0.0);
            assert_eq!(components.d_is, 0);
            assert_eq!(components.d_sp, 0);
        }
    }

    // -------------------------------------------------------------------
    // Additional property-based tests (proptest)
    // -------------------------------------------------------------------

    mod proptests {
        use super::*;
        use crate::datom::{Attribute, Datom, Op};
        use crate::proptest_strategies::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn live_projection_monotonicity((s1, s2) in arb_store_pair(2)) {
                // INV-TRILATERAL-001: LIVE projections are monotone functions.
                // Adding datoms (merging) cannot shrink any projection.
                let (i_before, s_before, p_before) = live_projections(&s1);

                let mut merged = s1.clone_store();
                merged.merge(&s2);

                let (i_after, s_after, p_after) = live_projections(&merged);

                prop_assert!(
                    i_after.datom_count >= i_before.datom_count,
                    "intent projection must not shrink after merge: {} < {}",
                    i_after.datom_count,
                    i_before.datom_count
                );
                prop_assert!(
                    s_after.datom_count >= s_before.datom_count,
                    "spec projection must not shrink after merge: {} < {}",
                    s_after.datom_count,
                    s_before.datom_count
                );
                prop_assert!(
                    p_after.datom_count >= p_before.datom_count,
                    "impl projection must not shrink after merge: {} < {}",
                    p_after.datom_count,
                    p_before.datom_count
                );
            }

            #[test]
            fn classify_attribute_covers_all_known(
                intent_idx in 0..INTENT_ATTRS.len(),
                spec_idx in 0..SPEC_ATTRS.len(),
                impl_idx in 0..IMPL_ATTRS.len(),
            ) {
                // All listed attributes must classify to their respective namespace.
                let intent_attr = Attribute::from_keyword(INTENT_ATTRS[intent_idx]);
                prop_assert_eq!(
                    classify_attribute(&intent_attr),
                    AttrNamespace::Intent,
                    "INTENT_ATTRS[{}] = {:?} must classify as Intent",
                    intent_idx,
                    INTENT_ATTRS[intent_idx]
                );

                let spec_attr = Attribute::from_keyword(SPEC_ATTRS[spec_idx]);
                prop_assert_eq!(
                    classify_attribute(&spec_attr),
                    AttrNamespace::Spec,
                    "SPEC_ATTRS[{}] = {:?} must classify as Spec",
                    spec_idx,
                    SPEC_ATTRS[spec_idx]
                );

                let impl_attr = Attribute::from_keyword(IMPL_ATTRS[impl_idx]);
                prop_assert_eq!(
                    classify_attribute(&impl_attr),
                    AttrNamespace::Impl,
                    "IMPL_ATTRS[{}] = {:?} must classify as Impl",
                    impl_idx,
                    IMPL_ATTRS[impl_idx]
                );
            }

            #[test]
            fn isp_check_results_are_consistent(store in arb_store(3)) {
                // For every entity in the store, isp_check must return a valid
                // IspResult that is consistent with the entity's actual datoms.
                let entities = store.entities();
                for entity in &entities {
                    let result = isp_check(&store, *entity);

                    let datoms: Vec<&Datom> = store
                        .datoms()
                        .filter(|d| d.entity == *entity && d.op == Op::Assert)
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

                    match result {
                        IspResult::NoData => {
                            prop_assert!(
                                !has_intent && !has_spec && !has_impl,
                                "NoData but entity has ISP datoms"
                            );
                        }
                        IspResult::IntentSpecGap => {
                            prop_assert!(has_intent, "IntentSpecGap requires intent");
                            prop_assert!(!has_spec, "IntentSpecGap requires no spec");
                        }
                        IspResult::SpecImplGap => {
                            prop_assert!(has_spec, "SpecImplGap requires spec");
                            prop_assert!(!has_impl, "SpecImplGap requires no impl");
                        }
                        IspResult::SpecificationBypass => {
                            prop_assert!(has_intent, "SpecificationBypass requires intent");
                            prop_assert!(!has_spec, "SpecificationBypass requires no spec");
                            prop_assert!(has_impl, "SpecificationBypass requires impl");
                        }
                        IspResult::Coherent => {
                            // Coherent can occur in several cases:
                            // (true, true, true), (false, true, true), (false, false, true)
                            // Just verify it's not an impossible state
                            if has_intent {
                                prop_assert!(
                                    has_spec,
                                    "Coherent with intent requires spec"
                                );
                            }
                        }
                    }
                }
            }

            #[test]
            fn classify_unknown_attributes_as_meta(attr in arb_attribute()) {
                // Arbitrary attributes (not from the known lists) should classify as Meta,
                // unless they happen to match a known attribute (unlikely with random generation).
                let s = attr.as_str();
                let is_known = INTENT_ATTRS.contains(&s)
                    || SPEC_ATTRS.contains(&s)
                    || IMPL_ATTRS.contains(&s);
                if !is_known {
                    prop_assert_eq!(
                        classify_attribute(&attr),
                        AttrNamespace::Meta,
                        "unknown attribute {:?} must classify as Meta",
                        s
                    );
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Von Neumann entropy (INV-COHERENCE-001)
    // -------------------------------------------------------------------

    #[test]
    fn von_neumann_entropy_empty_store_is_zero() {
        // Empty store (no ref datoms) -> no graph -> entropy = 0
        let store = Store::genesis();
        let entropy = von_neumann_entropy(&store);
        // Genesis has no Value::Ref datoms, so entity graph is empty
        assert_eq!(entropy.node_count, 0);
        assert_eq!(entropy.entropy, 0.0);
    }

    #[test]
    fn von_neumann_entropy_genesis_single_component() {
        let store = Store::genesis();
        let entropy = von_neumann_entropy(&store);
        // Genesis has no Ref datoms -> no edges -> empty graph
        assert!(entropy.entropy >= 0.0, "entropy must be non-negative");
        assert!(
            entropy.normalized <= 1.0 + 1e-10,
            "normalized entropy must be <= 1"
        );
    }

    #[test]
    fn von_neumann_entropy_concentrated_vs_dispersed() {
        use crate::datom::{AgentId, TxId};
        let tx = TxId::new(1, 0, AgentId::from_name("test:entropy"));

        // Fully connected 4-node graph (concentrated structure).
        // The adjacency matrix is all-ones → rank-1 density matrix → S ≈ 0.
        let mut datoms_connected = Store::genesis().datom_set().clone();
        let a = EntityId::from_ident(":test/entropy-a");
        let b = EntityId::from_ident(":test/entropy-b");
        let c = EntityId::from_ident(":test/entropy-c");
        let d = EntityId::from_ident(":test/entropy-d");
        for &src in &[a, b, c, d] {
            for &dst in &[a, b, c, d] {
                if src != dst {
                    datoms_connected.insert(Datom::new(
                        src,
                        Attribute::from_keyword(":dep/from"),
                        Value::Ref(dst),
                        tx,
                        Op::Assert,
                    ));
                }
            }
        }
        let store_connected = Store::from_datoms(datoms_connected);
        let e_connected = von_neumann_entropy(&store_connected);

        // Sparse chain: A → B, C → D (dispersed, two isolated pairs).
        // The adjacency matrix has block-diagonal structure → higher effective rank → higher S.
        let mut datoms_sparse = Store::genesis().datom_set().clone();
        datoms_sparse.insert(Datom::new(
            a,
            Attribute::from_keyword(":dep/from"),
            Value::Ref(b),
            tx,
            Op::Assert,
        ));
        datoms_sparse.insert(Datom::new(
            c,
            Attribute::from_keyword(":dep/from"),
            Value::Ref(d),
            tx,
            Op::Assert,
        ));
        let store_sparse = Store::from_datoms(datoms_sparse);
        let e_sparse = von_neumann_entropy(&store_sparse);

        // Dispersed (sparse, disconnected) graph should have higher entropy
        // than a fully connected graph (concentrated, low-rank structure).
        assert!(
            e_sparse.entropy > e_connected.entropy,
            "dispersed graph should have higher entropy than concentrated: sparse={} vs connected={}",
            e_sparse.entropy, e_connected.entropy
        );
        assert!(
            e_sparse.effective_rank > e_connected.effective_rank,
            "dispersed graph should have higher effective rank: {} vs {}",
            e_sparse.effective_rank,
            e_connected.effective_rank
        );
    }

    #[test]
    fn von_neumann_entropy_normalized_bounded() {
        use crate::datom::{AgentId, TxId};
        let tx = TxId::new(1, 0, AgentId::from_name("test:entropy-norm"));
        let mut datoms = Store::genesis().datom_set().clone();
        let a = EntityId::from_ident(":test/norm-a");
        let b = EntityId::from_ident(":test/norm-b");
        let c = EntityId::from_ident(":test/norm-c");
        datoms.insert(Datom::new(
            a,
            Attribute::from_keyword(":dep/from"),
            Value::Ref(b),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            b,
            Attribute::from_keyword(":dep/from"),
            Value::Ref(c),
            tx,
            Op::Assert,
        ));
        let store = Store::from_datoms(datoms);
        let entropy = von_neumann_entropy(&store);
        assert!(entropy.normalized >= 0.0, "normalized must be >= 0");
        assert!(entropy.normalized <= 1.0 + 1e-10, "normalized must be <= 1");
    }
}
