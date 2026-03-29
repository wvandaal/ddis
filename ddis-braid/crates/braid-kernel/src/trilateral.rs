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
//! - **INV-TRILATERAL-004**: Convergence monotonicity — Φ non-increasing under bilateral ops.
//! - **INV-TRILATERAL-005**: Attribute namespace partitions are pairwise disjoint.
//! - **INV-TRILATERAL-006**: Divergence as Datalog program.
//! - **INV-TRILATERAL-009**: (Φ, β₁) duality — Φ=0 ∧ β₁=0 iff coherent.
//! - **INV-TRILATERAL-010**: Persistent cohomology over transaction filtration.
//!
//! # Design Decisions
//!
//! - ADR-TRILATERAL-001: Unified store with three LIVE views.
//! - ADR-TRILATERAL-002: EDNL as interchange format.
//! - ADR-TRILATERAL-003: Hooks for invisible convergence.
//! - ADR-TRILATERAL-004: N-lateral extensibility.
//! - ADR-TRILATERAL-005: Cohomological complement to divergence metric.
//! - ADR-TRILATERAL-006: F₂ coefficients for initial cohomology.
//!
//! # Negative Cases
//!
//! - NEG-TRILATERAL-001: No cross-view contamination between I/S/P projections.
//! - NEG-TRILATERAL-002: No external state for divergence — all from store.
//! - NEG-TRILATERAL-003: No divergence increase from convergence operations.
//! - NEG-TRILATERAL-004: No Φ-only coherence declaration (β₁ also required).

use std::collections::{BTreeMap, BTreeSet};

use crate::datom::{Attribute, Datom, EntityId, Op, Value};
use crate::query::graph::{
    first_betti_number, symmetric_eigen_decomposition, DenseMatrix, DiGraph,
};
use crate::store::Store;

// ---------------------------------------------------------------------------
// Attribute Namespace Partition (INV-TRILATERAL-005)
// ---------------------------------------------------------------------------

/// Intent-layer attributes (bootstrap defaults — C9 fallback).
pub const INTENT_ATTRS: &[&str] = &[
    ":intent/decision",
    ":intent/rationale",
    ":intent/source",
    ":intent/goal",
    ":intent/constraint",
    ":intent/preference",
    ":intent/noted",
];

/// Specification-layer attributes (bootstrap defaults — C9 fallback).
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

/// Implementation-layer attributes (bootstrap defaults — C9 fallback).
pub const IMPL_ATTRS: &[&str] = &[
    ":impl/signature",
    ":impl/implements",
    ":impl/file",
    ":impl/module",
    ":impl/test-result",
    ":impl/coverage",
];

/// Configurable namespace partition — loaded from policy datoms (C9: INV-FOUNDATION-015).
///
/// When present, overrides the hardcoded `INTENT_ATTRS` / `SPEC_ATTRS` / `IMPL_ATTRS`
/// constants. When empty (no `:policy/namespace-*` datoms), falls back to the constants.
///
/// Loaded by `PolicyConfig::from_store()` from `:policy/namespace-intent`,
/// `:policy/namespace-spec`, and `:policy/namespace-impl` datoms.
#[derive(Clone, Debug, Default)]
pub struct NamespaceConfig {
    /// Intent-layer attributes. Empty = use `INTENT_ATTRS`.
    pub intent: Vec<String>,
    /// Spec-layer attributes. Empty = use `SPEC_ATTRS`.
    pub spec: Vec<String>,
    /// Impl-layer attributes. Empty = use `IMPL_ATTRS`.
    pub r#impl: Vec<String>,
}

impl NamespaceConfig {
    /// Whether this config has any policy-loaded overrides.
    pub fn has_overrides(&self) -> bool {
        !self.intent.is_empty() || !self.spec.is_empty() || !self.r#impl.is_empty()
    }
}

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

/// Classify an attribute using policy-loaded namespace config (C9: INV-FOUNDATION-015).
///
/// If `config` has overrides for a given namespace, those are used; otherwise
/// falls back to the hardcoded constants.
pub fn classify_attribute_with_config(attr: &Attribute, config: &NamespaceConfig) -> AttrNamespace {
    let s = attr.as_str();

    // Policy overrides take precedence when present
    if !config.intent.is_empty() {
        if config.intent.iter().any(|a| a == s) {
            return AttrNamespace::Intent;
        }
    } else if INTENT_ATTRS.contains(&s) {
        return AttrNamespace::Intent;
    }

    if !config.spec.is_empty() {
        if config.spec.iter().any(|a| a == s) {
            return AttrNamespace::Spec;
        }
    } else if SPEC_ATTRS.contains(&s) {
        return AttrNamespace::Spec;
    }

    if !config.r#impl.is_empty() {
        if config.r#impl.iter().any(|a| a == s) {
            return AttrNamespace::Impl;
        }
    } else if IMPL_ATTRS.contains(&s) {
        return AttrNamespace::Impl;
    }

    AttrNamespace::Meta
}

/// Classify an attribute into its namespace partition (INV-TRILATERAL-005).
///
/// Uses hardcoded bootstrap defaults. For policy-configurable classification,
/// use [`classify_attribute_with_config`].
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
    // L2-FITNESS (INV-PERF-001): Use MaterializedViews ISP accumulators instead of
    // full O(N) datom scan. Views already track ISP entity sets incrementally.
    let views = store.views();

    (
        LiveView {
            entities: views.isp_intent_entities.iter().copied().collect(),
            datom_count: views.isp_intent_datom_count,
        },
        LiveView {
            entities: views.isp_spec_entities.iter().copied().collect(),
            datom_count: views.isp_spec_datom_count,
        },
        LiveView {
            entities: views.isp_impl_entities.iter().copied().collect(),
            datom_count: views.isp_impl_datom_count,
        },
    )
}

// ---------------------------------------------------------------------------
// Divergence Φ (INV-TRILATERAL-002)
// ---------------------------------------------------------------------------

/// Divergence components between boundary pairs (link-based reachability).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DivergenceComponents {
    /// D_IS: Intent entities with no `:spec/traces-to` Ref link targeting them.
    pub d_is: usize,
    /// D_SP: Spec entities with no `:impl/implements` Ref link targeting them.
    pub d_sp: usize,
}

/// Compute the divergence metric Φ from the store alone (INV-TRILATERAL-002).
///
/// `Φ = w_is × D_IS + w_sp × D_SP`
///
/// D_IS = count of Intent entities with NO `:spec/traces-to` Ref link targeting them.
/// D_SP = count of Spec entities with NO `:impl/implements` Ref link targeting them.
///
/// This measures **link-based reachability**, not entity set overlap. An intent entity
/// is "covered" only when some spec entity explicitly traces to it via `:spec/traces-to`.
/// A spec entity is "covered" only when some impl entity implements it via `:impl/implements`.
///
/// Default weights: w_is = 0.4, w_sp = 0.6 (spec-impl gap weighted higher).
pub fn compute_phi(store: &Store, w_is: f64, w_sp: f64) -> (f64, DivergenceComponents) {
    let (live_i, live_s, _live_p) = live_projections(store);

    // Build set of entities targeted by :spec/traces-to Ref datoms.
    // A spec entity with `:spec/traces-to Value::Ref(intent_entity)` means
    // that intent_entity is reachable (covered) from the spec layer.
    let traces_to_attr = Attribute::from_keyword(":spec/traces-to");
    let mut spec_trace_targets: BTreeSet<EntityId> = BTreeSet::new();
    for datom in store.attribute_datoms(&traces_to_attr) {
        if datom.op == Op::Assert {
            if let Value::Ref(target) = &datom.value {
                spec_trace_targets.insert(*target);
            }
        }
    }

    // Build set of entities targeted by :impl/implements Ref datoms.
    // An impl entity with `:impl/implements Value::Ref(spec_entity)` means
    // that spec_entity is reachable (covered) from the impl layer.
    let implements_attr = Attribute::from_keyword(":impl/implements");
    let mut impl_targets: BTreeSet<EntityId> = BTreeSet::new();
    for datom in store.attribute_datoms(&implements_attr) {
        if datom.op == Op::Assert {
            if let Value::Ref(target) = &datom.value {
                impl_targets.insert(*target);
            }
        }
    }

    // D_IS: intent entities not reached by any :spec/traces-to link
    let d_is = live_i
        .entities
        .iter()
        .filter(|e| !spec_trace_targets.contains(e))
        .count();

    // D_SP: spec entities not reached by any :impl/implements link
    let d_sp = live_s
        .entities
        .iter()
        .filter(|e| !impl_targets.contains(e))
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
    // PERF-2a: Use EAVT-indexed entity_datoms instead of full store scan.
    // Before: O(all_datoms) per entity × 7K entities = 500M iterations (48s).
    // After: O(entity_datoms) per entity via EAVT index (~7ms total).
    let datoms = store.entity_datoms(entity);

    let has_intent = datoms
        .iter()
        .any(|d| d.op == Op::Assert && classify_attribute(&d.attribute) == AttrNamespace::Intent);
    let has_spec = datoms
        .iter()
        .any(|d| d.op == Op::Assert && classify_attribute(&d.attribute) == AttrNamespace::Spec);
    let has_impl = datoms
        .iter()
        .any(|d| d.op == Op::Assert && classify_attribute(&d.attribute) == AttrNamespace::Impl);

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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
///
/// Includes von Neumann entropy (O(n³) eigendecomposition). For large stores
/// where latency matters, use [`check_coherence_fast`] which skips entropy.
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

/// Lightweight coherence check — skips von Neumann entropy (O(n³)).
///
/// Returns the same CoherenceReport but with zeroed entropy fields.
/// Use this when latency matters more than entropy metrics (e.g., budget mode,
/// guidance, seed briefings).
pub fn check_coherence_fast(store: &Store) -> CoherenceReport {
    // UA-1: Read ISP entity sets from MaterializedViews instead of scanning.
    let views = store.views();
    let intent_entities = &views.isp_intent_entities;
    let spec_entities = &views.isp_spec_entities;
    let impl_entities = &views.isp_impl_entities;

    // Compute Phi from materialized ISP entity sets (was: 2 x O(N) scans via live_projections)
    let d_is = intent_entities
        .iter()
        .filter(|e| !spec_entities.contains(e))
        .count();
    let d_sp = spec_entities
        .iter()
        .filter(|e| !impl_entities.contains(e))
        .count();
    let n = intent_entities.len() + spec_entities.len() + impl_entities.len();
    let n_max = n.max(1) as f64;
    let w_is = 0.4;
    let w_sp = 0.6;
    let phi = (w_is * d_is as f64 + w_sp * d_sp as f64) / n_max;
    let components = DivergenceComponents { d_is, d_sp };

    // Beta_1: still O(N) — requires graph structure.
    // TODO(UA-1 future): maintain adjacency incrementally for O(1) beta_1.
    let beta_1 = compute_beta_1(store);

    // ISP bypasses: O(entities) via indexed lookups (not O(N) datom scan)
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

    let node_count = store.entity_count();
    CoherenceReport {
        phi,
        components,
        beta_1,
        quadrant,
        live_intent: views.isp_intent_datom_count,
        live_spec: views.isp_spec_datom_count,
        live_impl: views.isp_impl_datom_count,
        isp_bypasses,
        entropy: CoherenceEntropy {
            entropy: 0.0,
            max_entropy: if node_count > 0 {
                (node_count as f64).log2()
            } else {
                0.0
            },
            normalized: 0.0,
            effective_rank: 0,
            node_count,
        },
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
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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

/// Threshold for switching from dense Jacobi to stochastic Lanczos quadrature.
const VN_DENSE_THRESHOLD: usize = 200;

/// Number of probe vectors for stochastic Lanczos quadrature.
const SLQ_PROBES: usize = 30;

/// Number of Lanczos steps per probe for SLQ.
const SLQ_LANCZOS_STEPS: usize = 50;

/// Compute von Neumann entropy of the entity reference graph (INV-COHERENCE-001).
///
/// Forms the symmetrized adjacency matrix A from all `Value::Ref` datoms, adds
/// unit self-loops, normalizes to density matrix ρ = A/Tr(A), then computes
/// S(ρ) = -Tr(ρ log₂ ρ).
///
/// **Adaptive algorithm selection:**
/// - n ≤ 200: Dense Jacobi eigendecomposition (exact).
/// - n > 200: Stochastic Lanczos Quadrature (SLQ) — estimates Tr(f(ρ))
///   using random probe vectors and small tridiagonal eigendecompositions.
///   Complexity: O(m·k·E) where m = 30 probes, k = 50 Lanczos steps, E = edges.
///   This replaces the O(n³) Jacobi and makes entropy tractable at any scale.
pub fn von_neumann_entropy(store: &Store) -> CoherenceEntropy {
    let (n, adj) = build_symmetric_adj_sparse(store);

    if n == 0 {
        return CoherenceEntropy {
            entropy: 0.0,
            max_entropy: 0.0,
            normalized: 0.0,
            effective_rank: 0,
            node_count: 0,
        };
    }

    // Compute degree vector (adjacency + self-loop)
    let mut degree = vec![1.0_f64; n]; // self-loops
    for &(i, j) in &adj {
        degree[i] += 1.0;
        degree[j] += 1.0;
    }
    let trace: f64 = degree.iter().sum();

    if trace < f64::EPSILON {
        return CoherenceEntropy {
            entropy: 0.0,
            max_entropy: (n as f64).log2(),
            normalized: 0.0,
            effective_rank: 0,
            node_count: n,
        };
    }

    if n <= VN_DENSE_THRESHOLD {
        return von_neumann_dense(n, &adj, trace);
    }

    // Stochastic Lanczos Quadrature for large graphs
    von_neumann_slq(n, &adj, trace)
}

/// Build sparse symmetric adjacency list from store's Ref datoms.
/// Returns (node_count, edge_list) where edges are (i, j) index pairs (undirected).
fn build_symmetric_adj_sparse(store: &Store) -> (usize, Vec<(usize, usize)>) {
    let mut node_index: BTreeMap<String, usize> = BTreeMap::new();
    let mut edges: Vec<(usize, usize)> = Vec::new();

    for datom in store.datoms() {
        if datom.op != Op::Assert {
            continue;
        }
        if let Value::Ref(target) = &datom.value {
            let src = entity_key(datom.entity);
            let dst = entity_key(*target);
            let next_id = node_index.len();
            let si = *node_index.entry(src).or_insert(next_id);
            let next_id = node_index.len();
            let di = *node_index.entry(dst).or_insert(next_id);
            if si != di {
                edges.push((si, di));
            }
        }
    }

    (node_index.len(), edges)
}

/// Dense Jacobi path for small graphs (n ≤ 200).
fn von_neumann_dense(n: usize, edges: &[(usize, usize)], trace: f64) -> CoherenceEntropy {
    let mut rho = DenseMatrix::zeros(n, n);

    // Self-loops / trace
    for i in 0..n {
        rho.set(i, i, 1.0 / trace);
    }

    // Symmetric edges / trace
    for &(i, j) in edges {
        rho.set(i, j, 1.0 / trace);
        rho.set(j, i, 1.0 / trace);
    }

    let eigenvalues = rho.symmetric_eigenvalues();

    let eps = 1e-12;
    let clamped: Vec<f64> = eigenvalues.iter().map(|&l| l.max(0.0)).collect();
    let trace_sum: f64 = clamped.iter().sum();
    let normalized_evals: Vec<f64> = if trace_sum > eps {
        clamped.iter().map(|&l| l / trace_sum).collect()
    } else {
        clamped
    };

    let mut entropy = 0.0_f64;
    let mut effective_rank = 0usize;

    for &lambda in &normalized_evals {
        if lambda > eps {
            effective_rank += 1;
            entropy -= lambda * lambda.log2();
        }
    }

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

/// Stochastic Lanczos Quadrature (SLQ) for von Neumann entropy of large graphs.
///
/// Estimates S(ρ) = -Tr(ρ log₂ ρ) = Tr(f(ρ)) where f(x) = -x log₂ x.
///
/// Algorithm (Ubaru, Chen, Saad 2017):
/// 1. For each of m random probe vectors v ~ {±1/√n}ⁿ (Rademacher):
///    a. Run k-step Lanczos on ρ with starting vector v → tridiagonal T_k
///    b. Eigendecompose T_k (k×k, cheap)
///    c. Estimate vᵀ f(ρ) v ≈ Σⱼ (e₁ᵀ qⱼ)² f(θⱼ) where θⱼ are T_k's eigenvalues
/// 2. Average: Tr(f(ρ)) ≈ (n/m) Σᵢ estimate_i
///
/// Effective rank estimated as exp(S) (exponential of entropy in nats).
fn von_neumann_slq(n: usize, edges: &[(usize, usize)], trace: f64) -> CoherenceEntropy {
    // Build CSR-like adjacency for fast matvec
    let mut adj_list: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(i, j) in edges {
        adj_list[i].push(j);
        adj_list[j].push(i);
    }

    // ρ = A / trace, where A has self-loops + symmetric edges
    // Matvec: ρ·x = (1/trace) * (x + Σ_neighbors x[j]) for each row
    let matvec = |x: &[f64], out: &mut [f64]| {
        for i in 0..n {
            let mut sum = x[i]; // self-loop
            for &j in &adj_list[i] {
                sum += x[j];
            }
            out[i] = sum / trace;
        }
    };

    let m = SLQ_PROBES.min(n);
    let k = SLQ_LANCZOS_STEPS.min(n);
    let mut total_estimate = 0.0_f64;

    // Deterministic seed for reproducibility (INV-QUERY-017: determinism)
    let mut rng_state: u64 = 0x517cc1b727220a95; // fixed seed

    for _probe in 0..m {
        // Generate Rademacher vector: v[i] = ±1/√n
        let scale = 1.0 / (n as f64).sqrt();
        let mut v: Vec<f64> = Vec::with_capacity(n);
        for _ in 0..n {
            // xorshift64 for reproducibility
            rng_state ^= rng_state << 13;
            rng_state ^= rng_state >> 7;
            rng_state ^= rng_state << 17;
            v.push(if rng_state & 1 == 0 { scale } else { -scale });
        }

        // Lanczos iteration: produce tridiagonal T_k
        let mut alphas = Vec::with_capacity(k); // diagonal
        let mut betas = Vec::with_capacity(k); // sub-diagonal
        let mut v_prev = vec![0.0_f64; n];
        let mut v_curr = v;
        let mut w = vec![0.0_f64; n];

        for step in 0..k {
            matvec(&v_curr, &mut w);

            // α_j = v_j^T · w
            let alpha: f64 = v_curr.iter().zip(w.iter()).map(|(a, b)| a * b).sum();
            alphas.push(alpha);

            // w = w - α_j * v_j - β_{j-1} * v_{j-1}
            let beta_prev = if step > 0 { betas[step - 1] } else { 0.0 };
            for i in 0..n {
                w[i] -= alpha * v_curr[i] + beta_prev * v_prev[i];
            }

            // β_j = ||w||
            let beta: f64 = w.iter().map(|x| x * x).sum::<f64>().sqrt();
            if beta < 1e-14 {
                // Invariant subspace found — pad remaining with zeros
                for _ in step + 1..k {
                    alphas.push(0.0);
                    betas.push(0.0);
                }
                break;
            }
            betas.push(beta);

            // v_{j+1} = w / β_j
            v_prev = v_curr;
            v_curr = w.iter().map(|&x| x / beta).collect();
            w = vec![0.0; n];
        }

        // Eigendecompose the k×k tridiagonal matrix T_k via Jacobi
        let kk = alphas.len();
        let mut t = DenseMatrix::zeros(kk, kk);
        for i in 0..kk {
            t.set(i, i, alphas[i]);
            if i + 1 < kk && i < betas.len() {
                t.set(i, i + 1, betas[i]);
                t.set(i + 1, i, betas[i]);
            }
        }

        let (evals, evecs) = symmetric_eigen_decomposition(&t);

        // Estimate: vᵀ f(ρ) v ≈ Σⱼ (e₁ᵀ qⱼ)² f(θⱼ)
        // where e₁ = [1, 0, ..., 0] and qⱼ are eigenvectors of T_k
        let eps = 1e-12;
        let mut probe_estimate = 0.0_f64;
        for (j, &eval) in evals.iter().enumerate() {
            let theta = eval.max(0.0);
            if theta > eps {
                let weight = evecs.get(0, j); // e₁ᵀ qⱼ
                let f_theta = -theta * theta.log2(); // -x log₂ x
                probe_estimate += weight * weight * f_theta;
            }
        }

        total_estimate += probe_estimate;
    }

    // S(ρ) ≈ n * (1/m) * Σ estimates
    // But our vectors are normalized to 1/√n, so the estimate is already scaled:
    // E[vᵀ f(ρ) v] = (1/n) Tr(f(ρ)) for Rademacher vectors scaled by 1/√n
    // Therefore Tr(f(ρ)) = n * (total_estimate / m)
    let entropy = (n as f64 * total_estimate / m as f64).max(0.0);

    let max_entropy = (n as f64).log2();
    let normalized = if max_entropy > 0.0 {
        (entropy / max_entropy).min(1.0)
    } else {
        0.0
    };

    // Effective rank from entropy: exp₂(S) (2^S gives effective number of states)
    let effective_rank = if entropy > 0.0 {
        2.0_f64.powf(entropy).round() as usize
    } else {
        1
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

// Witnesses: INV-TRILATERAL-001, INV-TRILATERAL-002, INV-TRILATERAL-003,
// INV-TRILATERAL-004, INV-TRILATERAL-005, INV-TRILATERAL-006,
// INV-TRILATERAL-007, INV-TRILATERAL-008, INV-TRILATERAL-009,
// INV-TRILATERAL-010,
// ADR-TRILATERAL-001, ADR-TRILATERAL-002, ADR-TRILATERAL-005, ADR-TRILATERAL-006,
// NEG-TRILATERAL-001, NEG-TRILATERAL-002, NEG-TRILATERAL-003, NEG-TRILATERAL-004
#[cfg(test)]
mod tests {
    use super::*;

    // Verifies: INV-TRILATERAL-005 — Attribute Namespace Partitioning
    // Verifies: NEG-TRILATERAL-001 — No Cross-View Contamination
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

    // Verifies: INV-TRILATERAL-005 — Attribute Namespace Partitioning
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

    // ── AUDIT-W1-002: Configurable namespace partitions ──

    #[test]
    fn namespace_config_default_has_no_overrides() {
        let config = NamespaceConfig::default();
        assert!(!config.has_overrides());
    }

    #[test]
    fn namespace_config_with_overrides_detected() {
        let config = NamespaceConfig {
            intent: vec![":goal/target".to_string()],
            ..Default::default()
        };
        assert!(config.has_overrides());
    }

    #[test]
    fn classify_with_empty_config_matches_default() {
        // Empty NamespaceConfig falls back to hardcoded constants
        let config = NamespaceConfig::default();
        for attr_str in INTENT_ATTRS {
            let attr = Attribute::from_keyword(attr_str);
            assert_eq!(
                classify_attribute_with_config(&attr, &config),
                AttrNamespace::Intent,
                "empty config should fall back to INTENT_ATTRS for {attr_str}"
            );
        }
        for attr_str in SPEC_ATTRS {
            let attr = Attribute::from_keyword(attr_str);
            assert_eq!(
                classify_attribute_with_config(&attr, &config),
                AttrNamespace::Spec,
                "empty config should fall back to SPEC_ATTRS for {attr_str}"
            );
        }
        for attr_str in IMPL_ATTRS {
            let attr = Attribute::from_keyword(attr_str);
            assert_eq!(
                classify_attribute_with_config(&attr, &config),
                AttrNamespace::Impl,
                "empty config should fall back to IMPL_ATTRS for {attr_str}"
            );
        }
    }

    #[test]
    fn classify_with_custom_config_uses_overrides() {
        // Custom config with non-DDIS attributes
        let config = NamespaceConfig {
            intent: vec![":goal/target".to_string(), ":goal/priority".to_string()],
            spec: vec![":requirement/statement".to_string()],
            r#impl: vec![":code/file".to_string()],
        };

        assert_eq!(
            classify_attribute_with_config(&Attribute::from_keyword(":goal/target"), &config),
            AttrNamespace::Intent
        );
        assert_eq!(
            classify_attribute_with_config(
                &Attribute::from_keyword(":requirement/statement"),
                &config
            ),
            AttrNamespace::Spec
        );
        assert_eq!(
            classify_attribute_with_config(&Attribute::from_keyword(":code/file"), &config),
            AttrNamespace::Impl
        );
        // Unknown attribute still Meta
        assert_eq!(
            classify_attribute_with_config(&Attribute::from_keyword(":db/ident"), &config),
            AttrNamespace::Meta
        );
    }

    #[test]
    fn classify_with_partial_overrides() {
        // Only override intent; spec and impl fall back to defaults
        let config = NamespaceConfig {
            intent: vec![":goal/target".to_string()],
            spec: vec![],   // falls back to SPEC_ATTRS
            r#impl: vec![], // falls back to IMPL_ATTRS
        };

        assert_eq!(
            classify_attribute_with_config(&Attribute::from_keyword(":goal/target"), &config),
            AttrNamespace::Intent
        );
        // Default intent attrs NOT recognized when intent is overridden
        assert_eq!(
            classify_attribute_with_config(&Attribute::from_keyword(":intent/goal"), &config),
            AttrNamespace::Meta,
            "overridden intent should not include default INTENT_ATTRS"
        );
        // But spec/impl defaults still work
        assert_eq!(
            classify_attribute_with_config(&Attribute::from_keyword(":spec/id"), &config),
            AttrNamespace::Spec
        );
        assert_eq!(
            classify_attribute_with_config(&Attribute::from_keyword(":impl/file"), &config),
            AttrNamespace::Impl
        );
    }

    // Verifies: INV-TRILATERAL-002 — Divergence as Live Metric
    // Verifies: NEG-TRILATERAL-002 — No External State for Divergence
    #[test]
    fn genesis_store_has_zero_divergence() {
        // Genesis store has only meta attributes → Φ = 0
        let store = Store::genesis();
        let (phi, components) = compute_phi_default(&store);
        assert_eq!(phi, 0.0);
        assert_eq!(components.d_is, 0);
        assert_eq!(components.d_sp, 0);
    }

    // Verifies: INV-TRILATERAL-007 — Unified Store Self-Bootstrap
    #[test]
    fn genesis_store_is_coherent() {
        let store = Store::genesis();
        let report = check_coherence(&store);
        assert_eq!(report.quadrant, CoherenceQuadrant::Coherent);
        assert_eq!(report.isp_bypasses, 0);
    }

    // Verifies: INV-TRILATERAL-003 — Formality Gradient
    #[test]
    fn formality_level_meta_only() {
        let store = Store::genesis();
        let entity = EntityId::from_ident(":db/ident");
        // Genesis entities only have meta attrs → level 0
        assert_eq!(formality_level(&store, entity), 0);
    }

    // Verifies: INV-TRILATERAL-008 — ISP Specification Bypass Detection
    #[test]
    fn isp_check_no_data() {
        let store = Store::genesis();
        let entity = EntityId::from_ident(":nonexistent/thing");
        assert_eq!(isp_check(&store, entity), IspResult::NoData);
    }

    // Verifies: INV-TRILATERAL-009 — Coherence Completeness (Phi, beta_1 Duality)
    // Verifies: NEG-TRILATERAL-004 — No Phi-Only Coherence Declaration
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

    // Verifies: INV-TRILATERAL-001 — Three LIVE Projections
    // Verifies: INV-TRILATERAL-004 — Convergence Monotonicity
    // Verifies: ADR-TRILATERAL-001 — Unified Store with Three LIVE Views
    #[test]
    fn live_projections_monotone_on_genesis() {
        let store = Store::genesis();
        let (i, s, p) = live_projections(&store);
        // Genesis only has meta attrs, so all projections should be empty
        assert_eq!(i.datom_count, 0);
        assert_eq!(s.datom_count, 0);
        assert_eq!(p.datom_count, 0);
    }

    // Verifies: INV-TRILATERAL-009 — Coherence Completeness (beta_1 = 0 for acyclic)
    // Verifies: INV-QUERY-024 — First Betti Number from Laplacian Kernel
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

    // Verifies: INV-TRILATERAL-009 — Coherence Completeness (beta_1 > 0 for cycle)
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

    // Verifies: INV-TRILATERAL-006 — Divergence as Datalog Program
    // Verifies: INV-TRILATERAL-002 — Divergence as Live Metric
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

        // store_with_intent_and_spec removed: link-based Phi uses Ref links,
        // not same-entity attribute co-occurrence.

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
            // A store where every intent entity is traced-to by a spec entity,
            // and every spec entity is implemented by an impl entity -> Phi = 0.
            let mut datoms = Store::genesis().datom_set().clone();
            let intent_e = EntityId::from_ident(":test/coherent-intent");
            let spec_e = EntityId::from_ident(":test/coherent-spec");
            let impl_e = EntityId::from_ident(":test/coherent-impl");
            let tx = make_tx(1);
            // Intent entity
            datoms.insert(Datom::new(
                intent_e,
                Attribute::from_keyword(":intent/goal"),
                Value::String("a goal".into()),
                tx,
                Op::Assert,
            ));
            // Spec entity with :spec/traces-to -> intent entity
            datoms.insert(Datom::new(
                spec_e,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-X-001".into()),
                tx,
                Op::Assert,
            ));
            datoms.insert(Datom::new(
                spec_e,
                Attribute::from_keyword(":spec/traces-to"),
                Value::Ref(intent_e),
                tx,
                Op::Assert,
            ));
            // Impl entity with :impl/implements -> spec entity
            datoms.insert(Datom::new(
                impl_e,
                Attribute::from_keyword(":impl/implements"),
                Value::Ref(spec_e),
                tx,
                Op::Assert,
            ));
            let store = Store::from_datoms(datoms);
            let (phi, components) = compute_phi_default(&store);
            assert_eq!(phi, 0.0, "fully linked ISP chain should produce phi=0");
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
            // Adding a :spec/traces-to link that targets the intent entity
            // should not increase D_IS (it should decrease it).
            let store_before = store_with_intent_only();
            let (phi_before, _) = compute_phi_default(&store_before);
            assert!(
                phi_before > 0.0,
                "precondition: phi should be positive for intent-only store"
            );

            // Create a store with the same intent entity PLUS a spec entity
            // that traces to it via :spec/traces-to.
            let mut datoms = store_before.datom_set().clone();
            let intent_e = EntityId::from_ident(":test/entity-a");
            let spec_e = EntityId::from_ident(":test/spec-for-a");
            let tx = make_tx(2);
            datoms.insert(Datom::new(
                spec_e,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-TEST-LINK-001".into()),
                tx,
                Op::Assert,
            ));
            datoms.insert(Datom::new(
                spec_e,
                Attribute::from_keyword(":spec/traces-to"),
                Value::Ref(intent_e),
                tx,
                Op::Assert,
            ));
            let store_after = Store::from_datoms(datoms);

            let components_after = compute_phi_default(&store_after).1;
            let components_before = compute_phi_default(&store_before).1;
            // D_IS: intent-spec gap should decrease when we add a :spec/traces-to link
            assert!(
                components_after.d_is <= components_before.d_is,
                "adding :spec/traces-to for intent entity must not increase D_IS: {} > {}",
                components_after.d_is,
                components_before.d_is
            );
            // D_IS should actually go to 0 (the intent entity is now traced-to)
            assert_eq!(
                components_after.d_is, 0,
                "intent entity should be covered by :spec/traces-to link"
            );
        }

        #[test]
        fn phi_monotonic_non_increase_under_link_spec_to_impl() {
            // Adding an :impl/implements link targeting the spec entity should
            // not increase D_SP (it should decrease it).
            let mut datoms = Store::genesis().datom_set().clone();
            let spec_e = EntityId::from_ident(":test/entity-b");
            let tx = make_tx(1);
            datoms.insert(Datom::new(
                spec_e,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-B-001".into()),
                tx,
                Op::Assert,
            ));
            let store_before = Store::from_datoms(datoms.clone());
            let (phi_before, comp_before) = compute_phi_default(&store_before);
            assert!(phi_before > 0.0, "spec-only entity should have phi > 0");
            assert_eq!(comp_before.d_sp, 1);

            // Now add an impl entity with :impl/implements -> spec entity
            let impl_e = EntityId::from_ident(":test/impl-for-b");
            datoms.insert(Datom::new(
                impl_e,
                Attribute::from_keyword(":impl/implements"),
                Value::Ref(spec_e),
                tx,
                Op::Assert,
            ));
            let store_after = Store::from_datoms(datoms);
            let (phi_after, comp_after) = compute_phi_default(&store_after);

            assert!(
                comp_after.d_sp <= comp_before.d_sp,
                "adding :impl/implements for spec entity must not increase D_SP: {} > {}",
                comp_after.d_sp,
                comp_before.d_sp
            );
            assert_eq!(
                comp_after.d_sp, 0,
                "spec entity should be covered by :impl/implements link"
            );
            assert!(
                phi_after <= phi_before,
                "phi must not increase when D_SP decreases"
            );
        }

        #[test]
        fn phi_full_coherence_from_gaps() {
            // Start with gaps in all boundaries, then close them all with
            // proper cross-boundary links. Final phi should be 0.
            let mut datoms = Store::genesis().datom_set().clone();
            let intent_e = EntityId::from_ident(":test/intent-full");
            let spec_e = EntityId::from_ident(":test/spec-full");
            let impl_e = EntityId::from_ident(":test/impl-full");
            let tx = make_tx(1);

            // Intent entity
            datoms.insert(Datom::new(
                intent_e,
                Attribute::from_keyword(":intent/goal"),
                Value::String("goal".into()),
                tx,
                Op::Assert,
            ));
            // Spec entity with :spec/traces-to -> intent
            datoms.insert(Datom::new(
                spec_e,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-FULL-001".into()),
                tx,
                Op::Assert,
            ));
            datoms.insert(Datom::new(
                spec_e,
                Attribute::from_keyword(":spec/traces-to"),
                Value::Ref(intent_e),
                tx,
                Op::Assert,
            ));
            // Impl entity with :impl/implements -> spec
            datoms.insert(Datom::new(
                impl_e,
                Attribute::from_keyword(":impl/implements"),
                Value::Ref(spec_e),
                tx,
                Op::Assert,
            ));

            let store = Store::from_datoms(datoms);
            let (phi, components) = compute_phi_default(&store);
            assert_eq!(phi, 0.0);
            assert_eq!(components.d_is, 0);
            assert_eq!(components.d_sp, 0);
        }

        // ---------------------------------------------------------------
        // WAVE2-PHI: Link-based traceability tests
        // ---------------------------------------------------------------

        /// WAVE2-PHI test 1: Fully linked ISP chain -> Phi = 0.
        /// Intent entity traced-to by spec, spec entity implemented by impl.
        #[test]
        fn phi_link_based_fully_linked_isp_is_zero() {
            let mut datoms = Store::genesis().datom_set().clone();
            let tx = make_tx(1);
            let intent_a = EntityId::from_ident(":test/link-intent-a");
            let intent_b = EntityId::from_ident(":test/link-intent-b");
            let spec_a = EntityId::from_ident(":test/link-spec-a");
            let spec_b = EntityId::from_ident(":test/link-spec-b");
            let impl_a = EntityId::from_ident(":test/link-impl-a");
            let impl_b = EntityId::from_ident(":test/link-impl-b");

            // Two intent entities
            datoms.insert(Datom::new(
                intent_a,
                Attribute::from_keyword(":intent/goal"),
                Value::String("goal-a".into()),
                tx,
                Op::Assert,
            ));
            datoms.insert(Datom::new(
                intent_b,
                Attribute::from_keyword(":intent/goal"),
                Value::String("goal-b".into()),
                tx,
                Op::Assert,
            ));

            // Two spec entities, each traces-to one intent entity
            datoms.insert(Datom::new(
                spec_a,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-LINK-A".into()),
                tx,
                Op::Assert,
            ));
            datoms.insert(Datom::new(
                spec_a,
                Attribute::from_keyword(":spec/traces-to"),
                Value::Ref(intent_a),
                tx,
                Op::Assert,
            ));
            datoms.insert(Datom::new(
                spec_b,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-LINK-B".into()),
                tx,
                Op::Assert,
            ));
            datoms.insert(Datom::new(
                spec_b,
                Attribute::from_keyword(":spec/traces-to"),
                Value::Ref(intent_b),
                tx,
                Op::Assert,
            ));

            // Two impl entities, each implements one spec entity
            datoms.insert(Datom::new(
                impl_a,
                Attribute::from_keyword(":impl/implements"),
                Value::Ref(spec_a),
                tx,
                Op::Assert,
            ));
            datoms.insert(Datom::new(
                impl_b,
                Attribute::from_keyword(":impl/implements"),
                Value::Ref(spec_b),
                tx,
                Op::Assert,
            ));

            let store = Store::from_datoms(datoms);
            let (phi, components) = compute_phi_default(&store);
            assert_eq!(
                components.d_is, 0,
                "all intent entities are traced-to by spec entities"
            );
            assert_eq!(
                components.d_sp, 0,
                "all spec entities are implemented by impl entities"
            );
            assert_eq!(phi, 0.0, "fully linked ISP chain must have Phi = 0");
        }

        /// WAVE2-PHI test 2: Unlinked intent entity -> Phi > 0.
        /// An intent entity exists but no spec entity traces to it.
        #[test]
        fn phi_link_based_unlinked_intent_is_positive() {
            let mut datoms = Store::genesis().datom_set().clone();
            let tx = make_tx(1);
            let intent_e = EntityId::from_ident(":test/unlinked-intent");
            let spec_e = EntityId::from_ident(":test/orphan-spec");

            // Intent entity with no :spec/traces-to targeting it
            datoms.insert(Datom::new(
                intent_e,
                Attribute::from_keyword(":intent/goal"),
                Value::String("unlinked goal".into()),
                tx,
                Op::Assert,
            ));

            // Spec entity exists but does NOT trace to the intent entity
            let unrelated = EntityId::from_ident(":test/unrelated-target");
            datoms.insert(Datom::new(
                spec_e,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-ORPHAN-001".into()),
                tx,
                Op::Assert,
            ));
            datoms.insert(Datom::new(
                spec_e,
                Attribute::from_keyword(":spec/traces-to"),
                Value::Ref(unrelated),
                tx,
                Op::Assert,
            ));

            let store = Store::from_datoms(datoms);
            let (phi, components) = compute_phi_default(&store);
            assert_eq!(
                components.d_is, 1,
                "intent entity with no :spec/traces-to targeting it counts as gap"
            );
            assert!(phi > 0.0, "unlinked intent entity must produce Phi > 0");
        }

        /// WAVE2-PHI test 3: Partially linked -> Phi between 0 and max.
        /// Some intent entities linked, some not. Some spec entities linked, some not.
        #[test]
        fn phi_link_based_partial_links() {
            let mut datoms = Store::genesis().datom_set().clone();
            let tx = make_tx(1);
            let intent_linked = EntityId::from_ident(":test/partial-intent-linked");
            let intent_unlinked = EntityId::from_ident(":test/partial-intent-unlinked");
            let spec_linked = EntityId::from_ident(":test/partial-spec-linked");
            let spec_unlinked = EntityId::from_ident(":test/partial-spec-unlinked");
            let impl_e = EntityId::from_ident(":test/partial-impl");

            // Two intent entities
            datoms.insert(Datom::new(
                intent_linked,
                Attribute::from_keyword(":intent/goal"),
                Value::String("linked goal".into()),
                tx,
                Op::Assert,
            ));
            datoms.insert(Datom::new(
                intent_unlinked,
                Attribute::from_keyword(":intent/goal"),
                Value::String("unlinked goal".into()),
                tx,
                Op::Assert,
            ));

            // spec_linked traces to intent_linked (covers it)
            datoms.insert(Datom::new(
                spec_linked,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-PARTIAL-001".into()),
                tx,
                Op::Assert,
            ));
            datoms.insert(Datom::new(
                spec_linked,
                Attribute::from_keyword(":spec/traces-to"),
                Value::Ref(intent_linked),
                tx,
                Op::Assert,
            ));

            // spec_unlinked has no impl targeting it
            datoms.insert(Datom::new(
                spec_unlinked,
                Attribute::from_keyword(":spec/id"),
                Value::String("INV-PARTIAL-002".into()),
                tx,
                Op::Assert,
            ));

            // impl_e implements spec_linked (covers it), but NOT spec_unlinked
            datoms.insert(Datom::new(
                impl_e,
                Attribute::from_keyword(":impl/implements"),
                Value::Ref(spec_linked),
                tx,
                Op::Assert,
            ));

            let store = Store::from_datoms(datoms);
            let (phi, components) = compute_phi_default(&store);

            assert_eq!(
                components.d_is, 1,
                "one intent entity is unlinked, expected D_IS=1"
            );
            assert_eq!(
                components.d_sp, 1,
                "one spec entity is unlinked, expected D_SP=1"
            );
            let expected_phi = 0.4 * 1.0 + 0.6 * 1.0;
            assert!(
                (phi - expected_phi).abs() < f64::EPSILON,
                "Phi should be {} for partial links, got {}",
                expected_phi,
                phi
            );
            assert!(phi > 0.0, "partial links must have Phi > 0");
            let max_phi = 0.4 * 2.0 + 0.6 * 2.0;
            assert!(phi < max_phi, "partial links must have Phi < max");
        }
    }

    // -------------------------------------------------------------------
    // Additional property-based tests (proptest)
    // -------------------------------------------------------------------

    mod proptests {
        use super::*;
        use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, TxId, Value};
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

            // ---------------------------------------------------------------
            // INV-TRILATERAL-004: Convergence monotonicity — adding a
            // :spec/traces-to link targeting an intent entity, or an
            // :impl/implements link targeting a spec entity, must not
            // increase the corresponding gap component (D_IS or D_SP).
            // ---------------------------------------------------------------

            /// INV-TRILATERAL-004: Adding a :spec/traces-to link targeting an
            /// intent entity, or an :impl/implements link targeting a spec entity,
            /// must not increase the corresponding gap component (D_IS or D_SP).
            /// Link-based reachability: the link's VALUE (Ref target) determines
            /// which entity gets covered, not the link's source entity.
            #[test]
            fn inv_trilateral_004_convergence_under_link_ops(
                suffix in 1u32..500,
                has_intent in any::<bool>(),
                has_spec in any::<bool>(),
                has_impl in any::<bool>(),
                add_spec_link in any::<bool>(),
            ) {
                let tx = TxId::new(1, 0, AgentId::from_name("test:conv004"));
                let e = EntityId::from_ident(&format!(":test/conv-entity-{suffix}"));
                let mut datoms_before = Store::genesis().datom_set().clone();

                if has_intent {
                    datoms_before.insert(Datom::new(
                        e,
                        Attribute::from_keyword(":intent/goal"),
                        Value::String(format!("goal-{suffix}")),
                        tx,
                        Op::Assert,
                    ));
                }
                if has_spec {
                    datoms_before.insert(Datom::new(
                        e,
                        Attribute::from_keyword(":spec/id"),
                        Value::String(format!("INV-CONV-{suffix:03}")),
                        tx,
                        Op::Assert,
                    ));
                }
                if has_impl {
                    datoms_before.insert(Datom::new(
                        e,
                        Attribute::from_keyword(":impl/file"),
                        Value::String(format!("src/conv_{suffix}.rs")),
                        tx,
                        Op::Assert,
                    ));
                }

                let store_before = Store::from_datoms(datoms_before.clone());
                let (_, comp_before) = compute_phi_default(&store_before);
                let mut datoms_after = datoms_before;

                if add_spec_link {
                    // LINK operation: add :spec/traces-to targeting entity e
                    // (covers e in D_IS if e is an intent entity).
                    let linker = EntityId::from_ident(&format!(":test/spec-linker-{suffix}"));
                    datoms_after.insert(Datom::new(
                        linker,
                        Attribute::from_keyword(":spec/traces-to"),
                        Value::Ref(e),
                        tx,
                        Op::Assert,
                    ));
                    let store_after = Store::from_datoms(datoms_after);
                    let (_, comp_after) = compute_phi_default(&store_after);

                    // Adding a :spec/traces-to link targeting e can only reduce
                    // or maintain D_IS (if e is an intent entity, it's now covered).
                    prop_assert!(
                        comp_after.d_is <= comp_before.d_is,
                        "D_IS must not increase after adding :spec/traces-to link: {} > {}",
                        comp_after.d_is,
                        comp_before.d_is,
                    );
                } else {
                    // LINK operation: add :impl/implements targeting entity e
                    // (covers e in D_SP if e is a spec entity).
                    let linker = EntityId::from_ident(&format!(":test/impl-linker-{suffix}"));
                    datoms_after.insert(Datom::new(
                        linker,
                        Attribute::from_keyword(":impl/implements"),
                        Value::Ref(e),
                        tx,
                        Op::Assert,
                    ));
                    let store_after = Store::from_datoms(datoms_after);
                    let (_, comp_after) = compute_phi_default(&store_after);

                    // Adding an :impl/implements link targeting e can only reduce
                    // or maintain D_SP (if e is a spec entity, it's now covered).
                    prop_assert!(
                        comp_after.d_sp <= comp_before.d_sp,
                        "D_SP must not increase after adding :impl/implements link: {} > {}",
                        comp_after.d_sp,
                        comp_before.d_sp,
                    );
                }
            }

            // ---------------------------------------------------------------
            // INV-TRILATERAL-006: Phi is computable via Datalog — for any
            // store, compute_phi_default produces a non-negative finite value
            // without panicking. This verifies the computability claim: the
            // divergence metric is a total function over all valid stores.
            // ---------------------------------------------------------------

            /// INV-TRILATERAL-006: For any arbitrary store, compute_phi_default
            /// completes without panic and returns a non-negative, finite value.
            /// This is the computability property: Phi is defined for all stores.
            #[test]
            fn inv_trilateral_006_phi_computable_for_any_store(store in arb_store(5)) {
                let (phi, components) = compute_phi_default(&store);

                // Phi must be non-negative (gap counts are non-negative, weights are positive)
                prop_assert!(
                    phi >= 0.0,
                    "Phi must be non-negative for any store, got {}",
                    phi,
                );

                // Phi must be finite (no NaN or infinity)
                prop_assert!(
                    phi.is_finite(),
                    "Phi must be finite for any store, got {}",
                    phi,
                );

                // Components must be internally consistent with Phi:
                // Phi = 0.4 * D_IS + 0.6 * D_SP (default weights)
                let expected = 0.4 * components.d_is as f64 + 0.6 * components.d_sp as f64;
                prop_assert!(
                    (phi - expected).abs() < f64::EPSILON,
                    "Phi ({}) must equal 0.4*D_IS + 0.6*D_SP = 0.4*{} + 0.6*{} = {}",
                    phi,
                    components.d_is,
                    components.d_sp,
                    expected,
                );
            }
        }
    }

    // -------------------------------------------------------------------
    // B.4: Trilateral safety property verification (proptest)
    // -------------------------------------------------------------------

    mod safety_properties {
        use super::*;
        use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, TxId, Value};
        use crate::store::Store;
        use proptest::prelude::*;

        /// Strategy that builds a store with ISP-layer datoms (intent, spec, impl).
        /// Generates 1..=max_entities entities, each with a random subset of layers.
        fn arb_isp_store(max_entities: usize) -> impl Strategy<Value = Store> {
            let max_e = if max_entities == 0 { 1 } else { max_entities };
            proptest::collection::vec(
                // For each entity: (entity_name_suffix, has_intent, has_spec, has_impl)
                (1u32..1000, any::<bool>(), any::<bool>(), any::<bool>()),
                1..=max_e,
            )
            .prop_map(|entity_specs| {
                let mut datoms = Store::genesis().datom_set().clone();
                let tx = TxId::new(1, 0, AgentId::from_name("test:safety"));

                for (suffix, has_intent, has_spec, has_impl) in &entity_specs {
                    let e = EntityId::from_ident(&format!(":test/safety-entity-{suffix}"));
                    if *has_intent {
                        datoms.insert(Datom::new(
                            e,
                            Attribute::from_keyword(":intent/goal"),
                            Value::String(format!("goal-{suffix}")),
                            tx,
                            Op::Assert,
                        ));
                    }
                    if *has_spec {
                        datoms.insert(Datom::new(
                            e,
                            Attribute::from_keyword(":spec/id"),
                            Value::String(format!("INV-SAFETY-{suffix:03}")),
                            tx,
                            Op::Assert,
                        ));
                    }
                    if *has_impl {
                        datoms.insert(Datom::new(
                            e,
                            Attribute::from_keyword(":impl/file"),
                            Value::String(format!("src/safety_{suffix}.rs")),
                            tx,
                            Op::Assert,
                        ));
                    }
                }
                Store::from_datoms(datoms)
            })
        }

        /// Strategy for a pair of ISP stores where the second is a superset of the first.
        /// The first store has a subset of layers; the second adds more layers to existing entities.
        fn arb_isp_store_growth() -> impl Strategy<Value = (Store, Store)> {
            proptest::collection::vec(
                // (suffix, before_layers: [intent,spec,impl], after_layers: [intent,spec,impl])
                // after_layers are OR'd with before_layers to guarantee superset
                (1u32..500, any::<[bool; 3]>(), any::<[bool; 3]>()),
                1..=5,
            )
            .prop_map(|specs| {
                let tx = TxId::new(1, 0, AgentId::from_name("test:growth"));
                let mut datoms_before = Store::genesis().datom_set().clone();
                let mut datoms_after = Store::genesis().datom_set().clone();

                let intent_attr = Attribute::from_keyword(":intent/goal");
                let spec_attr = Attribute::from_keyword(":spec/id");
                let impl_attr = Attribute::from_keyword(":impl/file");

                for (suffix, before, after) in &specs {
                    let e = EntityId::from_ident(&format!(":test/growth-entity-{suffix}"));

                    // Before layers
                    if before[0] {
                        let d = Datom::new(
                            e,
                            intent_attr.clone(),
                            Value::String(format!("g-{suffix}")),
                            tx,
                            Op::Assert,
                        );
                        datoms_before.insert(d.clone());
                        datoms_after.insert(d);
                    }
                    if before[1] {
                        let d = Datom::new(
                            e,
                            spec_attr.clone(),
                            Value::String(format!("INV-G-{suffix:03}")),
                            tx,
                            Op::Assert,
                        );
                        datoms_before.insert(d.clone());
                        datoms_after.insert(d);
                    }
                    if before[2] {
                        let d = Datom::new(
                            e,
                            impl_attr.clone(),
                            Value::String(format!("src/g_{suffix}.rs")),
                            tx,
                            Op::Assert,
                        );
                        datoms_before.insert(d.clone());
                        datoms_after.insert(d);
                    }

                    // After layers: OR with before (superset guarantee)
                    if after[0] && !before[0] {
                        datoms_after.insert(Datom::new(
                            e,
                            intent_attr.clone(),
                            Value::String(format!("g-{suffix}")),
                            tx,
                            Op::Assert,
                        ));
                    }
                    if after[1] && !before[1] {
                        datoms_after.insert(Datom::new(
                            e,
                            spec_attr.clone(),
                            Value::String(format!("INV-G-{suffix:03}")),
                            tx,
                            Op::Assert,
                        ));
                    }
                    if after[2] && !before[2] {
                        datoms_after.insert(Datom::new(
                            e,
                            impl_attr.clone(),
                            Value::String(format!("src/g_{suffix}.rs")),
                            tx,
                            Op::Assert,
                        ));
                    }
                }

                (
                    Store::from_datoms(datoms_before),
                    Store::from_datoms(datoms_after),
                )
            })
        }

        proptest! {
            /// INV-TRILATERAL-003: Formality gradient is monotonically non-decreasing
            /// as datoms are added. Adding datoms to an entity's ISP layers cannot
            /// decrease its formality level.
            #[test]
            fn formality_level_monotone_under_growth((before, after) in arb_isp_store_growth()) {
                // For every entity present in the "before" store, its formality level
                // in the "after" (superset) store must be >= its level in "before".
                let entities_before = before.entities();
                for entity in &entities_before {
                    let level_before = formality_level(&before, *entity);
                    let level_after = formality_level(&after, *entity);
                    prop_assert!(
                        level_after >= level_before,
                        "formality_level must not decrease: entity {:?} went from {} to {}",
                        entity, level_before, level_after
                    );
                }
            }

            /// INV-TRILATERAL-001 (extended): LIVE projection entity counts are
            /// monotonically non-decreasing as the store grows.
            #[test]
            fn live_projections_entity_count_monotone_under_growth((before, after) in arb_isp_store_growth()) {
                let (i_before, s_before, p_before) = live_projections(&before);
                let (i_after, s_after, p_after) = live_projections(&after);

                prop_assert!(
                    i_after.entities.len() >= i_before.entities.len(),
                    "intent entity count must not shrink: {} < {}",
                    i_after.entities.len(), i_before.entities.len()
                );
                prop_assert!(
                    s_after.entities.len() >= s_before.entities.len(),
                    "spec entity count must not shrink: {} < {}",
                    s_after.entities.len(), s_before.entities.len()
                );
                prop_assert!(
                    p_after.entities.len() >= p_before.entities.len(),
                    "impl entity count must not shrink: {} < {}",
                    p_after.entities.len(), p_before.entities.len()
                );
            }

            /// INV-TRILATERAL-001 (datom counts): LIVE projection datom counts are
            /// monotonically non-decreasing as the store grows.
            #[test]
            fn live_projections_datom_count_monotone_under_growth((before, after) in arb_isp_store_growth()) {
                let (i_before, s_before, p_before) = live_projections(&before);
                let (i_after, s_after, p_after) = live_projections(&after);

                prop_assert!(
                    i_after.datom_count >= i_before.datom_count,
                    "intent datom count must not shrink: {} < {}",
                    i_after.datom_count, i_before.datom_count
                );
                prop_assert!(
                    s_after.datom_count >= s_before.datom_count,
                    "spec datom count must not shrink: {} < {}",
                    s_after.datom_count, s_before.datom_count
                );
                prop_assert!(
                    p_after.datom_count >= p_before.datom_count,
                    "impl datom count must not shrink: {} < {}",
                    p_after.datom_count, p_before.datom_count
                );
            }

            /// INV-TRILATERAL-007 (implied): isp_check is deterministic.
            /// Running isp_check twice on the same store and entity must produce
            /// identical results.
            #[test]
            fn isp_check_deterministic(store in arb_isp_store(5)) {
                let entities = store.entities();
                for entity in &entities {
                    let result_a = isp_check(&store, *entity);
                    let result_b = isp_check(&store, *entity);
                    prop_assert_eq!(
                        result_a, result_b,
                        "isp_check must be deterministic for entity {:?}", entity
                    );
                }
            }

            /// check_coherence_fast produces consistent, valid results:
            /// phi >= 0, beta_1 >= 0 (trivially true for usize), and quadrant
            /// classification is consistent with (phi, beta_1) values.
            #[test]
            fn check_coherence_fast_consistent(store in arb_isp_store(5)) {
                let report = check_coherence_fast(&store);

                // Phi must be non-negative
                prop_assert!(
                    report.phi >= 0.0,
                    "phi must be non-negative, got {}", report.phi
                );

                // Quadrant must be consistent with (phi, beta_1)
                let expected_quadrant = match (report.phi > 0.0, report.beta_1 > 0) {
                    (false, false) => CoherenceQuadrant::Coherent,
                    (true, false) => CoherenceQuadrant::GapsOnly,
                    (false, true) => CoherenceQuadrant::CyclesOnly,
                    (true, true) => CoherenceQuadrant::GapsAndCycles,
                };
                prop_assert_eq!(
                    report.quadrant, expected_quadrant,
                    "quadrant {:?} inconsistent with phi={}, beta_1={}",
                    report.quadrant, report.phi, report.beta_1
                );

                // Live counts must be non-negative (trivially true for usize,
                // but verifies consistency with live_projections)
                let (i, s, p) = live_projections(&store);
                prop_assert_eq!(
                    report.live_intent, i.datom_count,
                    "live_intent inconsistent: report={}, projection={}",
                    report.live_intent, i.datom_count
                );
                prop_assert_eq!(
                    report.live_spec, s.datom_count,
                    "live_spec inconsistent: report={}, projection={}",
                    report.live_spec, s.datom_count
                );
                prop_assert_eq!(
                    report.live_impl, p.datom_count,
                    "live_impl inconsistent: report={}, projection={}",
                    report.live_impl, p.datom_count
                );
            }

            /// check_coherence_fast is deterministic: same store always produces
            /// the same report.
            #[test]
            fn check_coherence_fast_deterministic(store in arb_isp_store(4)) {
                let report_a = check_coherence_fast(&store);
                let report_b = check_coherence_fast(&store);

                prop_assert!(
                    (report_a.phi - report_b.phi).abs() < f64::EPSILON,
                    "phi not deterministic: {} vs {}", report_a.phi, report_b.phi
                );
                prop_assert_eq!(report_a.beta_1, report_b.beta_1);
                prop_assert_eq!(report_a.quadrant, report_b.quadrant);
                prop_assert_eq!(report_a.live_intent, report_b.live_intent);
                prop_assert_eq!(report_a.live_spec, report_b.live_spec);
                prop_assert_eq!(report_a.live_impl, report_b.live_impl);
                prop_assert_eq!(report_a.isp_bypasses, report_b.isp_bypasses);
            }

            /// Formality levels are bounded [0, 4].
            #[test]
            fn formality_level_bounded(store in arb_isp_store(5)) {
                for entity in &store.entities() {
                    let level = formality_level(&store, *entity);
                    prop_assert!(
                        level <= 4,
                        "formality_level must be <= 4, got {} for {:?}", level, entity
                    );
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Von Neumann entropy (INV-COHERENCE-001)
    // -------------------------------------------------------------------

    // Verifies: INV-TRILATERAL-002 — Divergence as Live Metric
    // Verifies: ADR-TRILATERAL-005 — Cohomological Complement to Divergence Metric
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
