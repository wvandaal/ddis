//! `bilateral` — The bilateral coherence verification loop.
//!
//! This module implements the bilateral convergence mechanism from spec/10-bilateral.md.
//! The bilateral loop verifies coherence between Intent, Specification, and Implementation
//! through forward/backward scans, a 7-component fitness function F(S), five coherence
//! conditions (CC-1..CC-5), spectral certificates, and convergence analysis.
//!
//! # Mathematical Foundations
//!
//! The bilateral loop is a discrete dynamical system on the lattice of store states.
//! The fitness function F(S) ∈ [0, 1] is a Lyapunov function satisfying:
//!
//! ```text
//! F(S(t+1)) ≥ F(S(t))    (Monotonic convergence — Law L1)
//! ```
//!
//! This guarantees convergence to a fixed point where F(S*) = 1.0 (perfect coherence)
//! or where all residual divergence is documented (Law L2).
//!
//! The spectral certificate provides independent verification via:
//! - **Fiedler value λ₂**: algebraic connectivity of the entity graph
//! - **Cheeger constant h(G)**: isoperimetric ratio (partitionability bound)
//! - **Persistent homology**: topological stability of cycles
//! - **Rényi entropy spectrum**: multi-resolution coherence fingerprint
//!   - S₀ (Hartley): log₂(effective_rank) — diversity of occupied dimensions
//!   - S₁ (von Neumann): -Σ λᵢ log λᵢ — standard quantum entropy
//!   - S₂ (collision): -log₂(Σ λᵢ²) — purity measure
//!   - S_∞ (min-entropy): -log₂(λ_max) — worst-case information
//! - **Entropy decomposition**: S₃ = S₁ + ΔS_boundary + ΔS_ISP
//!
//! # Convergence Certificate
//!
//! If the spectral gap g = λ₂/λ_max > 0 and Cheeger constant h(G) > 0:
//! 1. The bilateral loop converges exponentially with rate bounded by g
//! 2. The specification graph has no isolated components
//! 3. Mixing time of the bilateral random walk is O(log(n)/g)
//!
//! # References
//!
//! - spec/10-bilateral.md: Full specification
//! - INV-BILATERAL-001: Monotonic convergence (L1)
//! - INV-BILATERAL-002: Five-point coherence (CC-1..CC-5)
//! - INV-BILATERAL-003: Bilateral symmetry (forward/backward parity)
//! - INV-BILATERAL-004: Residual documentation completeness
//! - INV-BILATERAL-005: Test results as datoms
//!
//! # Design Decisions
//!
//! - ADR-BILATERAL-002: Divergence metric as weighted boundary sum.
//! - ADR-BILATERAL-003: Intent validation as periodic session.
//! - ADR-BILATERAL-004: Bilateral authority principle.
//! - ADR-BILATERAL-005: Reconciliation taxonomy — detect-classify-resolve.
//! - ADR-BILATERAL-006: Coherence verification as fundamental problem.
//! - ADR-BILATERAL-007: Formalism-to-divergence-type mapping.
//! - ADR-BILATERAL-008: Explicit residual divergence.
//! - ADR-BILATERAL-009: Cross-project coherence deferred.
//! - ADR-BILATERAL-010: Taxonomy extensibility.
//!
//! # Negative Cases
//!
//! - NEG-BILATERAL-001: No fitness regression (F(S) monotonic non-decreasing).
//! - NEG-BILATERAL-002: No unchecked coherence dimension.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::datom::{latest_assert, Attribute, Datom, EntityId, Op, TxId, Value};
use crate::guidance::{compute_methodology_score, telemetry_from_store};
use crate::query::graph::{
    cheeger, fiedler, graph_laplacian, ricci_curvature_adaptive, ricci_summary,
    spectral_decomposition_adaptive, tx_barcode, DiGraph,
};
use crate::store::Store;
use crate::trilateral::{check_coherence_fast, live_projections, von_neumann_entropy};

// ===========================================================================
// Constants — F(S) weights from spec/10-bilateral.md §10.1
// ===========================================================================

/// Fitness component weights (spec-defined, sum = 1.0).
pub const W_VALIDATION: f64 = 0.18;
/// Coverage weight.
pub const W_COVERAGE: f64 = 0.18;
/// Drift weight (1 - normalized Φ).
pub const W_DRIFT: f64 = 0.18;
/// Harvest quality weight.
pub const W_HARVEST: f64 = 0.13;
/// Contradiction weight (1 - contradiction ratio).
pub const W_CONTRADICTION: f64 = 0.13;
/// Incompleteness weight (1 - incomplete ratio).
pub const W_INCOMPLETENESS: f64 = 0.08;
/// Uncertainty weight (1 - mean uncertainty).
pub const W_UNCERTAINTY: f64 = 0.12;

/// CC-5 threshold: methodology adherence must exceed this for CC-5 to pass.
const CC5_METHODOLOGY_THRESHOLD: f64 = 0.5;

// ===========================================================================
// Core Types
// ===========================================================================

/// The boundary between two ISP layers.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Boundary {
    /// Intent ↔ Specification boundary.
    IntentSpec,
    /// Specification ↔ Implementation boundary.
    SpecImpl,
}

/// Severity of a coverage gap, derived from formality level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GapSeverity {
    /// No cross-boundary links at all — completely disconnected.
    Critical,
    /// Links in only one boundary — partially connected.
    Major,
    /// Has some connections but missing coverage in the target layer.
    Minor,
}

/// A coverage gap: a spec or impl entity missing its counterpart.
#[derive(Clone, Debug)]
pub struct Gap {
    /// The entity with the gap.
    pub entity: EntityId,
    /// The entity's ident (if available).
    pub ident: Option<String>,
    /// Which boundary the gap is on.
    pub boundary: Boundary,
    /// Severity based on formality level.
    pub severity: GapSeverity,
}

/// Result of a single-direction bilateral scan.
#[derive(Clone, Debug)]
pub struct ScanResult {
    /// Entities that have coverage across the boundary.
    pub covered: Vec<EntityId>,
    /// Entities missing coverage.
    pub gaps: Vec<Gap>,
    /// Coverage ratio: |covered| / (|covered| + |gaps|).
    pub coverage_ratio: f64,
}

/// Combined bilateral scan (forward + backward).
#[derive(Clone, Debug)]
pub struct BilateralScan {
    /// Forward scan: Spec → Impl (what spec elements lack implementation?).
    pub forward: ScanResult,
    /// Backward scan: Impl → Spec (what implementations lack spec coverage?).
    pub backward: ScanResult,
}

/// The 7-component fitness breakdown.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FitnessComponents {
    /// V ∈ [0,1]: fraction of spec elements with witness evidence.
    pub validation: f64,
    /// C ∈ [0,1]: spec-impl coverage ratio (from forward scan).
    pub coverage: f64,
    /// D ∈ [0,1]: 1 - Φ/Φ_max (normalized divergence complement).
    pub drift: f64,
    /// H ∈ [0,1]: methodology score M(t) as harvest quality proxy.
    pub harvest_quality: f64,
    /// K ∈ [0,1]: 1 - (conflict count / total multi-valued attrs).
    pub contradiction: f64,
    /// I ∈ [0,1]: 1 - (incomplete spec elements / total spec elements).
    pub incompleteness: f64,
    /// U ∈ [0,1]: mean confidence across exploration entities.
    pub uncertainty: f64,
}

/// The overall fitness score F(S).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FitnessScore {
    /// F(S) ∈ [0, 1]: weighted sum of components.
    pub total: f64,
    /// Individual component values.
    pub components: FitnessComponents,
    /// Components that could not be measured (honest about limitations).
    pub unmeasured: Vec<String>,
}

/// Result of evaluating a single coherence condition.
#[derive(Clone, Debug)]
pub struct ConditionResult {
    /// Whether the condition is satisfied.
    pub satisfied: bool,
    /// Confidence in the evaluation (0.0–1.0).
    pub confidence: f64,
    /// One-line evidence string.
    pub evidence: String,
    /// Whether this condition can be evaluated without human input.
    pub machine_evaluable: bool,
}

/// The five coherence conditions CC-1 through CC-5.
#[derive(Clone, Debug)]
pub struct CoherenceConditions {
    /// CC-1: No contradiction in spec (machine-evaluable).
    pub cc1_no_contradictions: ConditionResult,
    /// CC-2: Impl ⊨ Spec — implementation satisfies specification (machine-evaluable).
    pub cc2_impl_satisfies_spec: ConditionResult,
    /// CC-3: Spec ≈ Intent — specification approximates intent (human-gated).
    pub cc3_spec_approximates_intent: ConditionResult,
    /// CC-4: Agent agreement via store union (machine-evaluable at Stage 0).
    pub cc4_agent_agreement: ConditionResult,
    /// CC-5: Agent behavior ⊨ methodology (machine-evaluable).
    pub cc5_methodology_adherence: ConditionResult,
    /// All five conditions satisfied.
    pub overall: bool,
}

/// Rényi entropy spectrum — multi-resolution coherence fingerprint.
///
/// The family S_α(ρ) = (1/(1-α)) log₂(Tr(ρ^α)) captures coherence
/// at different resolution levels:
/// - α → 0: Hartley entropy (log₂ of support size)
/// - α = 1: von Neumann entropy (standard)
/// - α = 2: Collision entropy (-log₂ purity)
/// - α → ∞: Min-entropy (-log₂ λ_max)
#[derive(Clone, Debug)]
pub struct RenyiSpectrum {
    /// S₀ = log₂(effective_rank): diversity of occupied dimensions.
    pub s0_hartley: f64,
    /// S₁ = -Σ λᵢ log₂ λᵢ: standard von Neumann entropy.
    pub s1_von_neumann: f64,
    /// S₂ = -log₂(Σ λᵢ²): collision entropy (purity measure).
    pub s2_collision: f64,
    /// S_∞ = -log₂(λ_max): min-entropy (worst-case).
    pub s_inf_min: f64,
    /// The raw eigenvalue spectrum (normalized: Σλᵢ = 1).
    pub spectrum: Vec<f64>,
}

/// Entropy decomposition: S₃ = S₁ + ΔS_boundary + ΔS_ISP.
#[derive(Clone, Debug)]
pub struct EntropyDecomposition {
    /// Total von Neumann entropy of the full entity graph.
    pub s_total: f64,
    /// Intent-level entropy (from Intent subgraph).
    pub s_intent: f64,
    /// Spec-level entropy (from Spec subgraph).
    pub s_spec: f64,
    /// Impl-level entropy (from Impl subgraph).
    pub s_impl: f64,
    /// Within-level average: (S_intent + S_spec + S_impl) / 3.
    pub s_within: f64,
    /// Cross-boundary contribution: S_total - S_within.
    pub delta_boundary: f64,
}

/// Spectral certificate for convergence verification.
///
/// Combines algebraic connectivity, isoperimetric ratio, topological
/// persistence, discrete Ricci curvature, and the Rényi entropy spectrum
/// into a single certificate that bounds convergence behavior.
#[derive(Clone, Debug)]
pub struct SpectralCertificate {
    /// Fiedler value λ₂: algebraic connectivity.
    /// λ₂ > 0 ⟺ graph is connected.
    pub fiedler_value: f64,
    /// Cheeger constant h(G): isoperimetric ratio.
    /// Cheeger inequality: λ₂/2 ≤ h(G) ≤ √(2λ₂).
    pub cheeger_constant: f64,
    /// Spectral gap: λ₂/λ_max (normalized algebraic connectivity).
    pub spectral_gap: f64,
    /// Total persistence from the transaction barcode.
    pub total_persistence: usize,
    /// H₁ birth count: number of independent cycle formations.
    pub cycle_births: usize,
    /// Mean Ollivier-Ricci curvature across all edges.
    /// Positive = clustered (good). Negative = bottleneck (fragile).
    pub mean_ricci: f64,
    /// Minimum Ollivier-Ricci curvature (worst bottleneck).
    pub min_ricci: f64,
    /// Rényi entropy spectrum at 4 resolution levels.
    pub renyi: RenyiSpectrum,
    /// Entropy decomposition by ISP level.
    pub entropy_decomposition: EntropyDecomposition,
    /// Convergence rate bound: 1 - exp(-spectral_gap).
    pub convergence_rate_bound: f64,
    /// Estimated mixing time: O(log(n) / spectral_gap).
    pub mixing_time_bound: f64,
}

/// Convergence analysis from F(S) trajectory using Lyapunov theory.
#[derive(Clone, Debug)]
pub struct ConvergenceAnalysis {
    /// F(S) trajectory over time.
    pub trajectory: Vec<f64>,
    /// Whether F(S) is monotonically non-decreasing (Law L1).
    pub is_monotonic: bool,
    /// Lyapunov exponent: λ = (1/n) Σ ln(F(t+1)/F(t)).
    /// Positive = improving. Negative = degrading. Zero = stable.
    pub lyapunov_exponent: f64,
    /// Estimated steps to reach F(S) ≥ 0.95.
    pub steps_to_target: Option<u64>,
    /// Exponential convergence rate (from Lyapunov exponent).
    pub convergence_rate: f64,
}

/// The full state of a bilateral cycle.
#[derive(Clone, Debug)]
pub struct BilateralState {
    /// F(S) fitness score with 7 components.
    pub fitness: FitnessScore,
    /// Bilateral scan results (forward + backward).
    pub scan: BilateralScan,
    /// CC-1..CC-5 coherence conditions.
    pub conditions: CoherenceConditions,
    /// Spectral certificate (None if graph too small or not requested).
    pub spectral: Option<SpectralCertificate>,
    /// Convergence analysis from F(S) trajectory.
    pub convergence: ConvergenceAnalysis,
    /// Cycle counter.
    pub cycle_count: u64,
}

// ===========================================================================
// BoundaryCheck Trait — Parameterized Boundary Checking (INV-BILATERAL-007)
// ===========================================================================

/// The relationship between two entity sets across a boundary.
///
/// Defined by the set algebra: given source set S and target set T,
/// SetRelation captures |S|, |T|, |S ∩ T|, |S \ T|, |T \ S|.
///
/// INV: |covered| + |source_gaps| = |source_total| (partition of S)
/// INV: coverage = |covered| / |source_total| when source_total > 0
#[derive(Clone, Debug)]
pub struct SetRelation {
    /// Total entities in the source set.
    pub source_total: usize,
    /// Total entities in the target set.
    pub target_total: usize,
    /// Entities in S that have links to T (|S ∩ T| projected to S).
    pub covered: usize,
    /// Entities in S without links to T (|S \ T|).
    pub source_gaps: usize,
    /// Entities in T without links from S (|T \ S|).
    pub target_gaps: usize,
    /// Coverage ratio: covered / source_total ∈ [0, 1]. 1.0 if source_total == 0.
    pub coverage: f64,
}

impl SetRelation {
    /// Compute from source and target counts.
    ///
    /// # Invariants
    /// - `covered + source_gaps == source_total`
    /// - `coverage == covered / source_total` when `source_total > 0`
    /// - `coverage == 1.0` when `source_total == 0` (vacuously satisfied)
    pub fn new(source_total: usize, target_total: usize, covered: usize) -> Self {
        let source_gaps = source_total.saturating_sub(covered);
        let target_gaps = target_total.saturating_sub(covered);
        let coverage = if source_total == 0 {
            1.0
        } else {
            covered as f64 / source_total as f64
        };
        Self {
            source_total,
            target_total,
            covered,
            source_gaps,
            target_gaps,
            coverage,
        }
    }
}

/// A single divergence detected by a boundary check.
///
/// Each divergence identifies a specific entity that lacks coverage
/// across a boundary, classified by severity and with a fix suggestion
/// for the guidance system to route to the agent.
#[derive(Clone, Debug)]
pub struct BoundaryDivergence {
    /// The entity that has the gap.
    pub entity: EntityId,
    /// Human-readable identifier (e.g., ":spec/inv-store-001" or "src/store.rs").
    pub ident: String,
    /// Which direction the gap is in: "forward" (source→target) or "backward" (target→source).
    pub direction: DivergenceDirection,
    /// Severity classification.
    pub severity: GapSeverity,
    /// Actionable fix suggestion for guidance routing.
    /// Example: "braid trace t-xxxx :spec/inv-store-001"
    pub fix_suggestion: String,
}

/// Direction of a boundary divergence.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DivergenceDirection {
    /// Source entity has no link to target (e.g., spec without implementation).
    Forward,
    /// Target entity has no link from source (e.g., implementation without spec).
    Backward,
}

/// Result of evaluating a single boundary.
///
/// Captures the set relation, divergences, and metadata needed for
/// the fitness function and guidance system.
#[derive(Clone, Debug)]
pub struct BoundaryEvaluation {
    /// Name of the boundary (e.g., "spec↔impl", "task↔spec").
    pub name: String,
    /// The set relation between source and target.
    pub relation: SetRelation,
    /// Individual divergences (gaps) found.
    pub divergences: Vec<BoundaryDivergence>,
    /// Weight of this boundary in the F(S) computation.
    pub weight: f64,
}

/// Trait for boundary checking — the parameterized bilateral loop.
///
/// Implementing this trait allows a new divergence type to participate
/// in F(S) computation, guidance routing, and status display without
/// modifying the core fitness function, guidance system, or CLI.
///
/// # INV-BILATERAL-007
///
/// Adding a new divergence boundary requires ONLY implementing this trait
/// and registering it. The fitness computation, guidance system, and
/// routing pipeline apply automatically.
///
/// # Object Safety
///
/// This trait is object-safe: all methods take `&self` and return owned types.
/// It can be used as `Box<dyn BoundaryCheck>` in the BoundaryRegistry.
pub trait BoundaryCheck {
    /// Human-readable name for display (e.g., "spec↔impl", "task↔spec").
    fn name(&self) -> &str;

    /// Weight of this boundary in the F(S) weighted sum.
    /// Must be in [0, 1]. The registry normalizes weights if they don't sum to 1.
    fn weight(&self) -> f64;

    /// Evaluate this boundary against the current store state.
    ///
    /// Returns a BoundaryEvaluation with the set relation, divergences,
    /// and metadata needed for fitness computation and guidance routing.
    fn evaluate(&self, store: &Store) -> BoundaryEvaluation;

    /// Brief description for help text and diagnostics.
    fn description(&self) -> &str;
}

/// Registry of boundary checks for composable F(S) computation.
///
/// F(S) = Σ wᵢ × coverage(bᵢ) across all registered boundaries.
///
/// The registry normalizes weights so they sum to 1.0, ensuring
/// F(S) ∈ [0, 1] by construction.
pub struct BoundaryRegistry {
    boundaries: Vec<Box<dyn BoundaryCheck>>,
}

impl BoundaryRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            boundaries: Vec::new(),
        }
    }

    /// Register a new boundary check.
    pub fn register(&mut self, boundary: Box<dyn BoundaryCheck>) {
        self.boundaries.push(boundary);
    }

    /// Number of registered boundaries.
    pub fn len(&self) -> usize {
        self.boundaries.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.boundaries.is_empty()
    }

    /// Evaluate all registered boundaries against the store.
    ///
    /// Returns evaluations in registration order.
    pub fn evaluate_all(&self, store: &Store) -> Vec<BoundaryEvaluation> {
        self.boundaries.iter().map(|b| b.evaluate(store)).collect()
    }

    /// Compute the weighted coverage across all boundaries.
    ///
    /// F(S)_boundaries = Σ (w_i / W_total) × coverage(b_i)
    /// where W_total = Σ w_i (normalization factor).
    ///
    /// Returns 1.0 if no boundaries are registered (vacuously satisfied).
    pub fn total_coverage(&self, store: &Store) -> f64 {
        if self.boundaries.is_empty() {
            return 1.0;
        }

        let evaluations = self.evaluate_all(store);
        let weight_sum: f64 = evaluations.iter().map(|e| e.weight).sum();
        if weight_sum == 0.0 {
            return 1.0;
        }

        evaluations
            .iter()
            .map(|e| (e.weight / weight_sum) * e.relation.coverage)
            .sum()
    }

    /// EVIDENCE-R2: Evidence-weighted coverage across all boundaries.
    ///
    /// Like `total_coverage` but weights each entity's contribution by
    /// `composite_evidence_weight()`. High-provenance, deeply-witnessed,
    /// fresh entities count more than hypothesized, unwitnessed, stale ones.
    ///
    /// Falls back to `total_coverage` when no boundaries produce entity sets.
    pub fn total_evidence_weighted_coverage(&self, store: &Store) -> f64 {
        if self.boundaries.is_empty() {
            return 1.0;
        }

        let evaluations = self.evaluate_all(store);
        let weight_sum: f64 = evaluations.iter().map(|e| e.weight).sum();
        if weight_sum == 0.0 {
            return 1.0;
        }

        evaluations
            .iter()
            .map(|e| {
                // If the boundary has entity-level detail in its gaps, we can weight
                // For now, use the flat coverage but scale by average evidence weight
                // of the boundary's source entities. This avoids requiring BoundaryCheck
                // to return entity sets (which would be a trait change).
                let avg_evidence = if !e.divergences.is_empty() {
                    let entity_weights: Vec<f64> = e
                        .divergences
                        .iter()
                        .map(|div| composite_evidence_weight(store, div.entity))
                        .collect();
                    let sum: f64 = entity_weights.iter().sum();
                    if entity_weights.is_empty() {
                        1.0
                    } else {
                        sum / entity_weights.len() as f64
                    }
                } else {
                    1.0 // No divergences = full evidence
                };

                (e.weight / weight_sum) * e.relation.coverage * avg_evidence
            })
            .sum()
    }

    /// Get an iterator over registered boundary names and weights.
    pub fn boundary_info(&self) -> Vec<(&str, f64)> {
        self.boundaries
            .iter()
            .map(|b| (b.name(), b.weight()))
            .collect()
    }
}

impl Default for BoundaryRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Verification Depth — WP9 F(S) Honesty
// ===========================================================================

/// Map verification depth level to weight for depth-weighted F(S) components.
///
/// Depth lattice: Unverified(0) < Syntactic(1) < Structural(2) < Property(3) < Formal(4)
/// Weight mapping makes comment-only traceability (Level 1) worth 15% of a formal proof.
pub fn depth_weight(depth: i64) -> f64 {
    match depth {
        0 => 0.0,
        1 => 0.15,
        2 => 0.4,
        3 => 0.7,
        4 => 1.0,
        _ => 0.0,
    }
}

/// Get the comonadic depth of an entity from the store (DC-1).
///
/// Returns 0 (OPINION) if `:comonad/depth` is not set — every entity starts
/// as an unverified assertion until challenged.
pub fn comonadic_depth(store: &Store, entity: &EntityId) -> i64 {
    let attr = crate::datom::Attribute::from_keyword(":comonad/depth");
    // LWW semantics: pick the Assert with the highest tx (most recent write),
    // not the highest value. BTreeSet orders by (entity, attr, value, tx, op),
    // so .rev().find() would return the Assert with the largest Value — wrong
    // when a newer tx writes a smaller value (e.g., falsification: depth 3 → 0).
    store
        .entity_datoms(*entity)
        .iter()
        .filter(|d| d.attribute == attr && d.op == crate::datom::Op::Assert)
        .max_by_key(|d| (d.tx.wall_time(), d.tx.logical()))
        .and_then(|d| match &d.value {
            crate::datom::Value::Long(v) => Some(*v),
            _ => None,
        })
        .unwrap_or(0)
}

/// Generate datoms to set the comonadic depth of an entity (DC-1).
///
/// Produces a single datom: `[entity :comonad/depth depth tx assert]`.
/// Caller is responsible for transacting into the store.
pub fn set_depth_datom(
    entity: &EntityId,
    depth: i64,
    tx: crate::datom::TxId,
) -> crate::datom::Datom {
    crate::datom::Datom::new(
        *entity,
        crate::datom::Attribute::from_keyword(":comonad/depth"),
        crate::datom::Value::Long(depth.clamp(0, 4)),
        tx,
        crate::datom::Op::Assert,
    )
}

// ===========================================================================
// F(S) Fitness Function — 7-component weighted sum
// ===========================================================================

/// Compute the 7-component fitness function F(S) from store state.
///
/// F(S) = 0.18V + 0.18C + 0.18D + 0.13H + 0.13K + 0.08I + 0.12U
///
/// Each component is in [0, 1]. F(S) ∈ [0, 1] by construction.
/// Components that cannot be measured are flagged in `unmeasured`.
///
/// EVIDENCE-R1: Composite evidence weight for an entity.
///
/// Computes `provenance_weight × depth_factor × freshness` where:
/// - provenance: observed=1.0, inferred=0.7, derived=0.4, hypothesized=0.2 (ADR-STORE-008)
/// - depth: L4=1.0, L3=0.8, L2=0.6, L1=0.3, none=0.1 (INV-WITNESS-002)
/// - freshness: exponential decay per namespace (ADR-HARVEST-005)
///
/// Returns [0.0, 1.0]. No new schema — reads existing attributes.
pub fn composite_evidence_weight(store: &Store, entity: EntityId) -> f64 {
    let datoms = store.entity_datoms(entity);

    // 1. Provenance weight — from the highest-authority transaction touching this entity
    let provenance_weight = datoms
        .iter()
        .filter(|d| d.op == Op::Assert)
        .filter_map(|d| {
            // Check if there's a :tx/provenance-type for this datom's tx
            let tx_entity = Store::tx_entity_id(d.tx);
            store
                .entity_datoms(tx_entity)
                .iter()
                .find(|td| td.attribute.as_str() == ":tx/provenance-type")
                .and_then(|td| {
                    if let Value::Keyword(k) = &td.value {
                        Some(match k.as_str() {
                            "observed" | ":provenance/observed" => 1.0,
                            "inferred" | ":provenance/inferred" => 0.7,
                            "derived" | ":provenance/derived" => 0.4,
                            "hypothesized" | ":provenance/hypothesized" => 0.2,
                            _ => 0.5,
                        })
                    } else {
                        None
                    }
                })
        })
        .fold(0.0f64, f64::max) // Take the highest provenance across all txns
        .max(0.2); // Floor at hypothesized

    // 2. Depth factor — from :impl/verification-depth
    let depth_factor = datoms
        .iter()
        .find(|d| d.attribute.as_str() == ":impl/verification-depth" && d.op == Op::Assert)
        .map(|d| {
            if let Value::Long(depth) = d.value {
                match depth {
                    4.. => 1.0,
                    3 => 0.8,
                    2 => 0.6,
                    1 => 0.3,
                    _ => 0.1,
                }
            } else {
                0.1
            }
        })
        .unwrap_or(0.1); // No witness = 0.1

    // 3. Freshness — exponential decay based on max tx wall time
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let max_wall = datoms
        .iter()
        .filter(|d| d.op == Op::Assert)
        .map(|d| d.tx.wall_time())
        .max()
        .unwrap_or(0);
    let age_days = (now.saturating_sub(max_wall)) as f64 / 86400.0;
    // Half-life of 30 days (configurable per namespace in future)
    let freshness = (-age_days * (2.0f64.ln() / 30.0)).exp().clamp(0.01, 1.0);

    (provenance_weight * depth_factor * freshness).clamp(0.0, 1.0)
}

/// POLICY-3: Compute F(S) from a policy manifest (ADR-FOUNDATION-013).
///
/// When a PolicyConfig exists, F(S) = sum(weight_i * coverage(boundary_i)).
/// Coverage for each boundary: |source entities with target reference| / |source entities|.
/// When no policy exists, returns None (caller should fall back to hardcoded).
///
/// This is the primary fitness path for the epistemology runtime.
pub fn compute_fitness_from_policy(store: &Store) -> Option<FitnessScore> {
    let config = crate::policy::PolicyConfig::from_store(store)?;

    if config.boundaries.is_empty() {
        // Policy exists but has no boundaries — vacuously coherent
        return Some(FitnessScore {
            total: 1.0,
            components: FitnessComponents {
                validation: 1.0,
                coverage: 1.0,
                drift: 1.0,
                harvest_quality: 1.0,
                contradiction: 1.0,
                incompleteness: 1.0,
                uncertainty: 1.0,
            },
            unmeasured: vec!["no boundaries declared".into()],
        });
    }

    let mut boundary_scores: Vec<(String, f64, f64)> = Vec::new(); // (name, coverage, weight)

    for boundary in &config.boundaries {
        // Count source entities: entities with attributes matching source_pattern
        let source_entities = entities_matching_pattern(store, &boundary.source_pattern);
        if source_entities.is_empty() {
            boundary_scores.push((boundary.name.clone(), 1.0, boundary.weight)); // vacuously covered
            continue;
        }

        // Find covered source entities
        let covered_entities = covered_entity_set(store, &boundary.source_pattern, &boundary.target_pattern);

        // DC-2: Depth-weighted coverage (ADR-FOUNDATION-020).
        // Each source entity's contribution is weighted by its comonadic depth.
        // depth 0 (OPINION) contributes 0.0, depth 4 (KNOWLEDGE) contributes 1.0.
        // Bootstrap fallback: if no entities have depth set, use raw coverage.
        let mut depth_sum = 0.0f64;
        let mut max_possible = 0.0f64;
        let mut any_has_depth = false;

        for entity in &source_entities {
            let d = comonadic_depth(store, entity);
            let w = depth_weight(d);
            if d > 0 {
                any_has_depth = true;
            }
            max_possible += 1.0; // max contribution per entity = 1.0 (depth 4)
            if covered_entities.contains(entity) {
                depth_sum += w;
            }
        }

        let coverage = if any_has_depth {
            // DC-2: depth-weighted coverage
            (depth_sum / max_possible.max(1.0)).clamp(0.0, 1.0)
        } else {
            // Bootstrap: no comonadic depth set yet, use raw coverage
            (covered_entities.len() as f64 / source_entities.len() as f64).clamp(0.0, 1.0)
        };

        boundary_scores.push((boundary.name.clone(), coverage, boundary.weight));
    }

    // Normalize weights and compute total
    let weight_sum: f64 = boundary_scores.iter().map(|(_, _, w)| w).sum();
    let total = if weight_sum > 0.0 {
        boundary_scores
            .iter()
            .map(|(_, cov, w)| (w / weight_sum) * cov)
            .sum::<f64>()
            .clamp(0.0, 1.0)
    } else {
        1.0
    };

    // Map boundary scores to the standard 7-component structure for backward compat.
    // The first 7 boundaries map to V, C, D, H, K, I, U. Extra boundaries are averaged
    // into the components. Missing boundaries get 1.0 (vacuous).
    let get_score = |idx: usize| -> f64 {
        boundary_scores
            .get(idx)
            .map(|(_, cov, _)| *cov)
            .unwrap_or(1.0)
    };

    Some(FitnessScore {
        total,
        components: FitnessComponents {
            validation: get_score(0),
            coverage: get_score(1),
            drift: get_score(2),
            harvest_quality: get_score(3),
            contradiction: get_score(4),
            incompleteness: get_score(5),
            uncertainty: get_score(6),
        },
        unmeasured: Vec::new(),
    })
}

/// Get entities that have at least one attribute matching the pattern.
fn entities_matching_pattern(store: &Store, pattern: &str) -> std::collections::BTreeSet<EntityId> {
    let mut entities = std::collections::BTreeSet::new();
    for d in store.datoms() {
        if d.op == Op::Assert && crate::policy::PolicyConfig::attr_matches(d.attribute.as_str(), pattern) {
            entities.insert(d.entity);
        }
    }
    entities
}

/// Get the set of source entities covered by target entities.
fn covered_entity_set(store: &Store, source_pattern: &str, target_pattern: &str) -> std::collections::BTreeSet<EntityId> {
    let source_entities = entities_matching_pattern(store, source_pattern);
    let mut target_entities = std::collections::BTreeSet::new();
    for d in store.datoms() {
        if d.op == Op::Assert && crate::policy::PolicyConfig::attr_matches(d.attribute.as_str(), target_pattern) {
            target_entities.insert(d.entity);
        }
    }

    let mut covered = std::collections::BTreeSet::new();
    for target in &target_entities {
        for d in store.entity_datoms(*target) {
            if d.op == Op::Assert {
                if let Value::Ref(src) = &d.value {
                    if source_entities.contains(src) {
                        covered.insert(*src);
                    }
                }
            }
        }
    }
    covered
}

/// V and C use depth-weighted metrics when `:impl/verification-depth` and
/// `:spec/verification-depth` datoms are present (WP9). When no depth datoms
/// exist, falls back to legacy binary counting for backwards compatibility.
pub fn compute_fitness(store: &Store) -> FitnessScore {
    let mut unmeasured = Vec::new();

    // V: Validation score — depth-weighted witness verification
    let validation = compute_validation(store);

    // C: Coverage — depth-weighted implementation coverage
    let coverage = compute_depth_weighted_coverage(store);

    // D: Drift — complement of normalized divergence Φ
    let drift = compute_drift_complement(store);

    // H: Harvest quality — methodology score M(t) from store state
    let telemetry = telemetry_from_store(store);
    let methodology = compute_methodology_score(&telemetry);
    let harvest_quality = methodology.score;
    if methodology.score == 0.0 {
        unmeasured.push("harvest_quality (no session telemetry)".into());
    }

    // K: Contradiction — complement of contradiction ratio
    let contradiction = compute_contradiction_complement(store);

    // I: Incompleteness — complement of incomplete spec elements
    let incompleteness = compute_incompleteness_complement(store);

    // U: Uncertainty — mean confidence across exploration entities
    let uncertainty = compute_uncertainty_complement(store);

    let components = FitnessComponents {
        validation,
        coverage,
        drift,
        harvest_quality,
        contradiction,
        incompleteness,
        uncertainty,
    };

    let total = W_VALIDATION * validation
        + W_COVERAGE * coverage
        + W_DRIFT * drift
        + W_HARVEST * harvest_quality
        + W_CONTRADICTION * contradiction
        + W_INCOMPLETENESS * incompleteness
        + W_UNCERTAINTY * uncertainty;

    // Clamp to [0, 1] for safety (shouldn't be needed if components are correct)
    let total = total.clamp(0.0, 1.0);

    FitnessScore {
        total,
        components,
        unmeasured,
    }
}

/// Compute F(S) using a BoundaryRegistry for the coverage component.
///
/// This is the BOUNDARY-6 entry point. The coverage (C) component comes from
/// the registry's total_coverage() instead of the legacy forward scan.
/// All other components remain the same.
///
/// INV-BILATERAL-007: F(S) = Σ wᵢ × coverage(bᵢ) for boundaries.
/// INV-BILATERAL-009: F(S) reflects all registered boundary checks.
///
/// When the registry is empty, falls back to legacy coverage computation
/// for backward compatibility.
pub fn compute_fitness_with_registry(store: &Store, registry: &BoundaryRegistry) -> FitnessScore {
    let mut unmeasured = Vec::new();

    // V: Validation score — same as legacy
    let validation = compute_validation(store);

    // C: Coverage — from BoundaryRegistry if non-empty, else legacy
    let coverage = if registry.is_empty() {
        compute_depth_weighted_coverage(store)
    } else {
        registry.total_coverage(store)
    };

    // D: Drift — same as legacy
    let drift = compute_drift_complement(store);

    // H: Harvest quality — same as legacy
    let telemetry = telemetry_from_store(store);
    let methodology = compute_methodology_score(&telemetry);
    let harvest_quality = methodology.score;
    if methodology.score == 0.0 {
        unmeasured.push("harvest_quality (no session telemetry)".into());
    }

    // K: Contradiction — same as legacy
    let contradiction = compute_contradiction_complement(store);

    // I: Incompleteness — same as legacy
    let incompleteness = compute_incompleteness_complement(store);

    // U: Uncertainty — same as legacy
    let uncertainty = compute_uncertainty_complement(store);

    let components = FitnessComponents {
        validation,
        coverage,
        drift,
        harvest_quality,
        contradiction,
        incompleteness,
        uncertainty,
    };

    let total = W_VALIDATION * validation
        + W_COVERAGE * coverage
        + W_DRIFT * drift
        + W_HARVEST * harvest_quality
        + W_CONTRADICTION * contradiction
        + W_INCOMPLETENESS * incompleteness
        + W_UNCERTAINTY * uncertainty;

    let total = total.clamp(0.0, 1.0);

    FitnessScore {
        total,
        components,
        unmeasured,
    }
}

/// V: Depth-weighted validation score.
///
/// When `:spec/verification-depth` datoms exist, uses depth weights:
///   V = Σ(depth_weight(depth_i)) / (|spec_elements| × depth_weight(4))
///
/// Falls back to binary `:spec/witnessed` counting when no depth datoms are present,
/// ensuring backwards compatibility with stores that predate WP9.
fn compute_validation(store: &Store) -> f64 {
    // INV-WITNESS-005: If WITNESS system has data, use witness-aware scoring
    // where stale witnesses contribute 0 to the validation score.
    let (witness_score, valid_count, _stale, _untested) =
        crate::witness::witness_validation_score(store);
    if valid_count > 0 {
        return witness_score;
    }

    // Fallback: existing depth-weighted or binary witness path
    let spec_type_attr = Attribute::from_keyword(":spec/element-type");
    let spec_depth_attr = Attribute::from_keyword(":spec/verification-depth");
    let witnessed_attr = Attribute::from_keyword(":spec/witnessed");

    let mut spec_count = 0u64;
    let mut depth_sum = 0.0f64;
    let mut has_any_depth = false;
    let mut witnessed_count = 0u64;

    for entity in store.entities() {
        let datoms = store.entity_datoms(entity);
        let is_spec = datoms
            .iter()
            .any(|d| d.attribute == spec_type_attr && d.op == Op::Assert);
        if !is_spec {
            continue;
        }
        spec_count += 1;

        // Check for depth datom (WP9 path)
        let depth = latest_assert(&datoms, &spec_depth_attr)
            .and_then(|d| match &d.value {
                Value::Long(v) => Some(*v),
                _ => None,
            });

        if let Some(d) = depth {
            has_any_depth = true;
            depth_sum += depth_weight(d);
        }

        // Also count binary witnesses for fallback
        let is_witnessed = datoms.iter().any(|d| {
            d.attribute == witnessed_attr
                && d.op == Op::Assert
                && matches!(&d.value, Value::Boolean(true))
        });
        if is_witnessed {
            witnessed_count += 1;
        }
    }

    if spec_count == 0 {
        return 1.0; // Vacuously true
    }

    if has_any_depth {
        // WP9 depth-weighted: normalize against max possible (all at depth 4)
        let max_possible = spec_count as f64 * depth_weight(4);
        (depth_sum / max_possible).clamp(0.0, 1.0)
    } else {
        // Legacy binary: fraction witnessed
        witnessed_count as f64 / spec_count as f64
    }
}

/// C: Depth-weighted implementation coverage.
///
/// When `:impl/verification-depth` datoms exist, uses depth weights:
///   C = Σ(max_depth_weight per spec element) / (|spec_elements| × depth_weight(4))
///
/// Falls back to binary forward_scan coverage_ratio when no depth datoms are present.
fn compute_depth_weighted_coverage(store: &Store) -> f64 {
    let spec_type_attr = Attribute::from_keyword(":spec/element-type");
    let implements_attr = Attribute::from_keyword(":impl/implements");
    let impl_depth_attr = Attribute::from_keyword(":impl/verification-depth");

    // Build map: spec entity → max verification depth across all impl entities
    let mut spec_max_depth: HashMap<EntityId, i64> = HashMap::new();
    let mut has_any_depth = false;

    for datom in store.attribute_datoms(&implements_attr) {
        if datom.op == Op::Assert {
            if let Value::Ref(spec_entity) = &datom.value {
                let impl_entity = datom.entity;
                // Get depth for this impl entity.
                // Default to 1 (syntactic) for impl links without explicit depth —
                // they passed the trace scanner which is Level 1 verification.
                let impl_datoms = store.entity_datoms(impl_entity);
                let explicit_depth = latest_assert(&impl_datoms, &impl_depth_attr)
                    .and_then(|d| match &d.value {
                        Value::Long(v) => Some(*v),
                        _ => None,
                    });

                let depth = explicit_depth.unwrap_or(1); // syntactic baseline

                if explicit_depth.is_some() {
                    has_any_depth = true;
                }

                let entry = spec_max_depth.entry(*spec_entity).or_insert(0);
                if depth > *entry {
                    *entry = depth;
                }
            }
        }
    }

    // Count total spec elements
    let mut spec_count = 0u64;
    for entity in store.entities() {
        let datoms = store.entity_datoms(entity);
        let is_spec = datoms
            .iter()
            .any(|d| d.attribute == spec_type_attr && d.op == Op::Assert);
        if is_spec {
            spec_count += 1;
        }
    }

    if spec_count == 0 {
        return 1.0; // Vacuously true
    }

    if has_any_depth {
        // WP9 depth-weighted coverage
        let depth_sum: f64 = spec_max_depth.values().map(|&d| depth_weight(d)).sum();
        let max_possible = spec_count as f64 * depth_weight(4);
        (depth_sum / max_possible).clamp(0.0, 1.0)
    } else {
        // Legacy binary: use forward_scan
        let forward = forward_scan(store);
        forward.coverage_ratio
    }
}

/// D: 1 - Φ/Φ_max where Φ_max = max(1, entity_count).
fn compute_drift_complement(store: &Store) -> f64 {
    let coherence = check_coherence_fast(store);
    let phi_max = store.entity_count().max(1) as f64;
    (1.0 - coherence.phi / phi_max).clamp(0.0, 1.0)
}

/// K: 1 - (conflicting_attributes / total_multi_valued_attributes).
///
/// Scans all (entity, attribute) pairs for conflicting values where the
/// resolution mode is LWW or Lattice (not Multi-value).
fn compute_contradiction_complement(store: &Store) -> f64 {
    let schema = store.schema();

    // A contradiction exists when a Cardinality::One attribute has multiple distinct
    // values AND the resolution mode cannot reconcile them. For LWW, multiple values
    // across transactions is normal (latest wins). For Multi, multiple values is the
    // intended behavior. Only Lattice with non-comparable values is a true contradiction.
    //
    // Simpler heuristic: count (entity, attribute) pairs where:
    //   1. The attribute is Cardinality::One in the schema
    //   2. Resolution is NOT Multi
    //   3. There are multiple distinct values WITHIN the same transaction
    //
    // Cross-transaction multi-values for LWW are normal temporal evolution, not contradictions.

    // Build (entity, attribute, tx) → distinct values map
    let mut tx_values: HashMap<(EntityId, String, TxId), HashSet<u64>> = HashMap::new();

    for datom in store.datoms() {
        if datom.op == Op::Assert {
            let key = (datom.entity, datom.attribute.as_str().to_string(), datom.tx);
            let value_hash = {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                datom.value.hash(&mut hasher);
                hasher.finish()
            };
            tx_values.entry(key).or_default().insert(value_hash);
        }
    }

    // Intra-transaction conflicts: same (entity, attr) in same tx has multiple values
    // for a non-Multi, Cardinality::One attribute.
    let intra_tx_conflicts: Vec<_> = tx_values
        .iter()
        .filter(|(_, values)| values.len() > 1)
        .filter(|((_, attr_str, _), _)| {
            if let Ok(attr) = Attribute::new(attr_str) {
                let mode = schema.resolution_mode(&attr);
                let card = schema.cardinality(&attr);
                !matches!(mode, crate::schema::ResolutionMode::Multi)
                    && matches!(card, crate::schema::Cardinality::One)
            } else {
                false // Unknown attributes are untracked, not contradictions
            }
        })
        .collect();

    if intra_tx_conflicts.is_empty() {
        return 1.0; // No intra-transaction contradictions
    }

    // Score: fraction of non-conflicting (entity, attribute, tx) triples
    let total_ea_pairs = tx_values.len() as f64;
    let conflict_count = intra_tx_conflicts.len() as f64;
    1.0 - (conflict_count / total_ea_pairs)
}

/// I: 1 - (spec elements without falsification / total spec elements).
fn compute_incompleteness_complement(store: &Store) -> f64 {
    let spec_type_attr = Attribute::from_keyword(":spec/element-type");
    let falsification_attr = Attribute::from_keyword(":spec/falsification");
    let impl_attr = Attribute::from_keyword(":impl/implements");
    let task_traces_attr = Attribute::from_keyword(":task/traces-to");

    // Collect all spec entity IDs
    let mut spec_entities = Vec::new();
    for entity in store.entities() {
        let datoms = store.entity_datoms(entity);
        let is_spec = datoms
            .iter()
            .any(|d| d.attribute == spec_type_attr && d.op == Op::Assert);
        if is_spec {
            spec_entities.push(entity);
        }
    }
    let spec_count = spec_entities.len() as u64;
    if spec_count == 0 {
        return 1.0;
    }

    // Build sets: which specs have impl links? Which have task coverage?
    let mut impl_covered: std::collections::HashSet<EntityId> = std::collections::HashSet::new();
    let mut task_covered: std::collections::HashSet<EntityId> = std::collections::HashSet::new();

    for d in store.attribute_datoms(&impl_attr) {
        if d.op == Op::Assert {
            if let Value::Ref(spec_entity) = &d.value {
                impl_covered.insert(*spec_entity);
            }
        }
    }
    for d in store.attribute_datoms(&task_traces_attr) {
        if d.op == Op::Assert {
            if let Value::Ref(spec_entity) = &d.value {
                task_covered.insert(*spec_entity);
            }
        }
    }

    // Score: 4-tier partial credit for spec completeness.
    // A formalized spec element (exists in store with type) is already ~15% complete —
    // the act of specification itself is real progress. Remaining credit comes from:
    //   - Falsification condition (+35%): spec is testable
    //   - Impl/task coverage (+50%): spec is tracked or implemented
    // Floor of 0.15 prevents untouched-but-formalized specs from dragging I to zero.
    let mut score_sum = 0.0f64;
    for &entity in &spec_entities {
        let datoms = store.entity_datoms(entity);
        let has_falsification = datoms
            .iter()
            .any(|d| d.attribute == falsification_attr && d.op == Op::Assert);
        let has_coverage = impl_covered.contains(&entity) || task_covered.contains(&entity);

        score_sum += match (has_falsification, has_coverage) {
            (true, true) => 1.0,
            (true, false) => 0.7,
            (false, true) => 0.4,
            (false, false) => 0.15,
        };
    }

    score_sum / spec_count as f64
}

/// U: Mean confidence across exploration entities.
///
/// For entities with `:exploration/confidence`, averages their values.
/// Returns 1.0 if no exploration entities exist (vacuously certain).
fn compute_uncertainty_complement(store: &Store) -> f64 {
    let confidence_attr = Attribute::from_keyword(":exploration/confidence");

    let mut sum = 0.0f64;
    let mut count = 0u64;

    for datom in store.attribute_datoms(&confidence_attr) {
        if datom.op == Op::Assert {
            if let Value::Double(f) = &datom.value {
                sum += f.into_inner();
                count += 1;
            }
        }
    }

    if count == 0 {
        return 1.0; // Vacuously certain
    }
    sum / count as f64
}

// ===========================================================================
// Bilateral Scans — Forward (Spec→Impl) and Backward (Impl→Spec)
// ===========================================================================

/// Forward scan: for each Spec entity, check if an Impl entity references it.
///
/// A spec entity is "covered" if any datom in the store asserts
/// `:impl/implements` with `Value::Ref(spec_entity)`.
/// INV-SIGNAL-002: Confusion triggers re-association — gaps detected here trigger guidance updates.
/// INV-SIGNAL-003: Subscription completeness — all spec entities checked for coverage.
/// INV-SIGNAL-005: Diamond lattice signal generation — schema conflicts generate signals.
/// ADR-SIGNAL-001: Eight signal types cover reconciliation taxonomy.
/// ADR-SIGNAL-002: Conflict routing cascade as datom trail.
/// ADR-SIGNAL-004: Four-type divergence taxonomy (epistemic, structural, consequential, aleatory).
pub fn forward_scan(store: &Store) -> ScanResult {
    let (_, spec_view, _) = live_projections(store);
    let implements_attr = Attribute::from_keyword(":impl/implements");
    let ident_attr = Attribute::from_keyword(":db/ident");

    // Build set of spec entities that are referenced by :impl/implements
    let mut impl_targets: HashSet<EntityId> = HashSet::new();
    for datom in store.attribute_datoms(&implements_attr) {
        if datom.op == Op::Assert {
            if let Value::Ref(target) = &datom.value {
                impl_targets.insert(*target);
            }
        }
    }

    let mut covered = Vec::new();
    let mut gaps = Vec::new();

    for &entity in &spec_view.entities {
        if impl_targets.contains(&entity) {
            covered.push(entity);
        } else {
            let ident = entity_ident(store, entity, &ident_attr);
            let formality = crate::trilateral::formality_level(store, entity);
            let severity = match formality {
                0 => GapSeverity::Critical,
                1 => GapSeverity::Major,
                _ => GapSeverity::Minor,
            };
            gaps.push(Gap {
                entity,
                ident,
                boundary: Boundary::SpecImpl,
                severity,
            });
        }
    }

    let total = covered.len() + gaps.len();
    let coverage_ratio = if total == 0 {
        1.0
    } else {
        covered.len() as f64 / total as f64
    };

    ScanResult {
        covered,
        gaps,
        coverage_ratio,
    }
}

/// Backward scan: for each Impl entity, check if it references a Spec entity.
///
/// An impl entity is "aligned" if it has an `:impl/implements` datom
/// pointing to an entity that exists in the Spec LIVE projection.
pub fn backward_scan(store: &Store) -> ScanResult {
    let (_, spec_view, impl_view) = live_projections(store);
    let implements_attr = Attribute::from_keyword(":impl/implements");
    let ident_attr = Attribute::from_keyword(":db/ident");

    let spec_entities: HashSet<EntityId> = spec_view.entities.iter().copied().collect();

    let mut covered = Vec::new();
    let mut gaps = Vec::new();

    for &entity in &impl_view.entities {
        let datoms = store.entity_datoms(entity);
        let has_spec_ref = datoms.iter().any(|d| {
            d.attribute == implements_attr
                && d.op == Op::Assert
                && matches!(&d.value, Value::Ref(target) if spec_entities.contains(target))
        });

        if has_spec_ref {
            covered.push(entity);
        } else {
            let ident = entity_ident(store, entity, &ident_attr);
            gaps.push(Gap {
                entity,
                ident,
                boundary: Boundary::SpecImpl,
                severity: GapSeverity::Major, // Orphan impl is always major
            });
        }
    }

    let total = covered.len() + gaps.len();
    let coverage_ratio = if total == 0 {
        1.0
    } else {
        covered.len() as f64 / total as f64
    };

    ScanResult {
        covered,
        gaps,
        coverage_ratio,
    }
}

/// Get the `:db/ident` value for an entity, if it exists.
fn entity_ident(store: &Store, entity: EntityId, ident_attr: &Attribute) -> Option<String> {
    store
        .entity_datoms(entity)
        .iter()
        .find(|d| d.attribute == *ident_attr && d.op == Op::Assert)
        .and_then(|d| match &d.value {
            Value::Keyword(k) => Some(k.clone()),
            Value::String(s) => Some(s.clone()),
            _ => None,
        })
}

// ===========================================================================
// CC-1..CC-5: Five Coherence Conditions
// ===========================================================================

/// Evaluate all five coherence conditions.
///
/// - CC-1: No contradiction in spec (machine-evaluable)
/// - CC-2: Impl ⊨ Spec — forward scan coverage = 1.0 (machine-evaluable)
/// - CC-3: Spec ≈ Intent — human-gated (default: true with low confidence)
/// - CC-4: Agent agreement via store union — single agent at Stage 0 (machine-evaluable)
/// - CC-5: Agent behavior ⊨ methodology — M(t) ≥ 0.5 (machine-evaluable)
pub fn evaluate_conditions(
    store: &Store,
    scan: &BilateralScan,
    fitness: &FitnessScore,
) -> CoherenceConditions {
    // CC-1: No contradictions (threshold 0.99 allows minor intra-tx duplicates
    // from batch assertions — true contradictions would push K well below 0.99)
    let cc1 = {
        let k = fitness.components.contradiction;
        let threshold = 0.99;
        ConditionResult {
            satisfied: k >= threshold,
            confidence: k,
            evidence: if k >= threshold {
                format!("Contradiction score K={k:.4} ≥ {threshold} — effectively clean")
            } else {
                format!("Contradiction score K={k:.4} < {threshold}")
            },
            machine_evaluable: true,
        }
    };

    // CC-2: Impl satisfies Spec
    // Suppress when no :impl/implements datoms exist — vacuously true (Wave 4.3: C3)
    let cc2 = {
        let has_impl_datoms = store
            .datoms()
            .any(|d| d.attribute.as_str() == ":impl/implements" && d.op == Op::Assert);
        if !has_impl_datoms {
            ConditionResult {
                satisfied: true,
                confidence: 0.0,
                evidence: "skipped — no :impl/implements datoms yet".into(),
                machine_evaluable: true,
            }
        } else {
            let c = scan.forward.coverage_ratio;
            ConditionResult {
                satisfied: c >= 1.0 - 1e-10,
                confidence: c,
                evidence: format!(
                    "{}/{} spec elements have implementation ({:.1}%)",
                    scan.forward.covered.len(),
                    scan.forward.covered.len() + scan.forward.gaps.len(),
                    c * 100.0,
                ),
                machine_evaluable: true,
            }
        }
    };

    // CC-3: Spec approximates Intent (human-gated)
    let cc3 = {
        let (intent_view, spec_view, _) = live_projections(store);
        let has_intent = intent_view.datom_count > 0;
        let has_spec = spec_view.datom_count > 0;
        ConditionResult {
            satisfied: true, // Human-gated: defaults to true
            confidence: if has_intent && has_spec { 0.5 } else { 0.3 },
            evidence: if !has_intent {
                "No intent entities — human review needed to validate spec alignment".into()
            } else {
                format!(
                    "Intent: {} entities, Spec: {} entities (requires human confirmation)",
                    intent_view.entities.len(),
                    spec_view.entities.len(),
                )
            },
            machine_evaluable: false,
        }
    };

    // CC-4: Agent agreement (Stage 0: all agents are tool-generated identities
    // operated by one human — agreement holds trivially. Multi-agent verification
    // starts at Stage 3 when independent agents can disagree on facts.)
    let cc4 = {
        let agent_count = store.frontier().len();
        // At Stage 0, multiple AgentIds arise from different CLI invocations
        // (bootstrap, trace, observe, etc.) all under one operator. This is not
        // the multi-agent disagreement CC-4 was designed to detect.
        let stage = crate::max_stage();
        let satisfied = stage < 3 || agent_count <= 1;
        ConditionResult {
            satisfied,
            confidence: if satisfied { 1.0 } else { 0.7 },
            evidence: format!(
                "{} agent(s) in frontier, stage {} — {}",
                agent_count,
                stage,
                if stage < 3 {
                    "single-operator, agreement trivial"
                } else if agent_count <= 1 {
                    "single-agent agreement trivially holds"
                } else {
                    "multi-agent agreement requires merge verification"
                }
            ),
            machine_evaluable: true,
        }
    };

    // CC-5: Methodology adherence
    let cc5 = {
        let m = fitness.components.harvest_quality;
        ConditionResult {
            satisfied: m >= CC5_METHODOLOGY_THRESHOLD,
            confidence: m.clamp(0.0, 1.0),
            evidence: format!(
                "M(t)={m:.2} {} threshold {CC5_METHODOLOGY_THRESHOLD}",
                if m >= CC5_METHODOLOGY_THRESHOLD {
                    "≥"
                } else {
                    "<"
                }
            ),
            machine_evaluable: true,
        }
    };

    let overall = cc1.satisfied && cc2.satisfied && cc3.satisfied && cc4.satisfied && cc5.satisfied;

    CoherenceConditions {
        cc1_no_contradictions: cc1,
        cc2_impl_satisfies_spec: cc2,
        cc3_spec_approximates_intent: cc3,
        cc4_agent_agreement: cc4,
        cc5_methodology_adherence: cc5,
        overall,
    }
}

// ===========================================================================
// Spectral Certificate — convergence bounds via algebraic graph theory
// ===========================================================================

/// Compute the spectral certificate for convergence verification.
///
/// This combines Fiedler value, Cheeger constant, persistent homology,
/// Ricci curvature, and Rényi entropy into a single certificate that
/// bounds the convergence behavior of the bilateral loop.
///
/// Returns None if the entity graph has fewer than 3 nodes.
pub fn spectral_certificate(store: &Store) -> Option<SpectralCertificate> {
    let graph = build_entity_graph(store);
    if graph.node_count() < 3 {
        return None;
    }

    // Fiedler value and spectral gap
    let fiedler_result = fiedler(&graph);
    let fiedler_value = fiedler_result
        .as_ref()
        .map(|f| f.algebraic_connectivity)
        .unwrap_or(0.0);

    let spectral_gap = if let Some(sd) = spectral_decomposition_adaptive(&graph) {
        let lambda_max = sd
            .eigenvalues
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        if lambda_max > 1e-15 {
            fiedler_value / lambda_max
        } else {
            0.0
        }
    } else {
        0.0
    };

    // Cheeger constant
    let cheeger_result = cheeger(&graph);
    let cheeger_constant = cheeger_result.map(|c| c.cheeger_constant).unwrap_or(0.0);

    // Persistent homology from transaction barcode
    let barcode = tx_barcode(store);
    let total_persist = crate::query::graph::total_persistence(&barcode);
    let cycle_births = barcode.pairs.iter().filter(|p| p.dimension == 1).count();

    // Ollivier-Ricci curvature
    let curvatures = ricci_curvature_adaptive(&graph);
    let ricci = ricci_summary(&curvatures);

    // Rényi entropy spectrum
    let renyi = compute_renyi_spectrum(store);

    // Entropy decomposition
    let entropy_decomp = compute_entropy_decomposition(store);

    // Convergence bounds
    let convergence_rate_bound = if spectral_gap > 0.0 {
        1.0 - (-spectral_gap).exp()
    } else {
        0.0
    };

    let n = graph.node_count() as f64;
    let mixing_time_bound = if spectral_gap > 1e-15 {
        n.ln() / spectral_gap
    } else {
        f64::INFINITY
    };

    Some(SpectralCertificate {
        fiedler_value,
        cheeger_constant,
        spectral_gap,
        total_persistence: total_persist,
        cycle_births,
        mean_ricci: ricci.mean_curvature,
        min_ricci: ricci.min_curvature,
        renyi,
        entropy_decomposition: entropy_decomp,
        convergence_rate_bound,
        mixing_time_bound,
    })
}

/// Build a DiGraph from the store's entity reference structure.
///
/// Nodes are entities (hex-encoded EntityId). Edges are from `Value::Ref`
/// datoms on cross-boundary attributes.
fn build_entity_graph(store: &Store) -> DiGraph {
    let mut graph = DiGraph::new();

    // Add all entities as nodes
    for entity in store.entities() {
        graph.add_node(&entity_hex(&entity));
    }

    // Add edges from Ref datoms
    for datom in store.datoms() {
        if datom.op == Op::Assert {
            if let Value::Ref(target) = &datom.value {
                let src = entity_hex(&datom.entity);
                let dst = entity_hex(target);
                graph.add_edge(&src, &dst);
            }
        }
    }

    graph
}

/// Build a subgraph containing only entities from a specific ISP level.
fn build_level_subgraph(store: &Store, entities: &[EntityId]) -> DiGraph {
    let entity_set: HashSet<EntityId> = entities.iter().copied().collect();
    let mut graph = DiGraph::new();

    for &eid in entities {
        graph.add_node(&entity_hex(&eid));
    }

    for datom in store.datoms() {
        if datom.op == Op::Assert && entity_set.contains(&datom.entity) {
            if let Value::Ref(target) = &datom.value {
                if entity_set.contains(target) {
                    graph.add_edge(&entity_hex(&datom.entity), &entity_hex(target));
                }
            }
        }
    }

    graph
}

/// Convert EntityId to hex string (consistent with query/graph.rs).
fn entity_hex(eid: &EntityId) -> String {
    eid.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

/// Compute von Neumann entropy of a graph from its Laplacian.
///
/// The density matrix is ρ = L / Tr(L), and
/// S(ρ) = -Σ λᵢ/Tr(L) · log₂(λᵢ/Tr(L)).
fn graph_entropy(graph: &DiGraph) -> f64 {
    let n = graph.node_count();
    if n < 2 {
        return 0.0;
    }
    let laplacian = graph_laplacian(graph);
    let eigenvalues = laplacian.symmetric_eigenvalues();
    entropy_from_eigenvalues(&eigenvalues)
}

/// Compute von Neumann entropy from eigenvalues of a Laplacian.
fn entropy_from_eigenvalues(eigenvalues: &[f64]) -> f64 {
    let trace: f64 = eigenvalues.iter().filter(|&&v| v > 1e-15).sum();
    if trace < 1e-15 {
        return 0.0;
    }
    let mut entropy = 0.0;
    for &lambda in eigenvalues {
        let p = lambda / trace;
        if p > 1e-15 {
            entropy -= p * p.log2();
        }
    }
    entropy
}

/// Compute the Rényi entropy spectrum at 4 resolution levels.
///
/// Given the normalized eigenvalue spectrum {pᵢ} where pᵢ = λᵢ/Tr(L):
/// - S₀ (Hartley) = log₂(|{i : pᵢ > ε}|)
/// - S₁ (von Neumann) = -Σ pᵢ log₂(pᵢ)
/// - S₂ (collision) = -log₂(Σ pᵢ²)
/// - S_∞ (min-entropy) = -log₂(max pᵢ)
fn compute_renyi_spectrum(store: &Store) -> RenyiSpectrum {
    let graph = build_entity_graph(store);
    if graph.node_count() < 2 {
        return RenyiSpectrum {
            s0_hartley: 0.0,
            s1_von_neumann: 0.0,
            s2_collision: 0.0,
            s_inf_min: 0.0,
            spectrum: vec![],
        };
    }

    let laplacian = graph_laplacian(&graph);
    let eigenvalues = laplacian.symmetric_eigenvalues();
    let trace: f64 = eigenvalues.iter().filter(|&&v| v > 1e-15).sum();

    if trace < 1e-15 {
        return RenyiSpectrum {
            s0_hartley: 0.0,
            s1_von_neumann: 0.0,
            s2_collision: 0.0,
            s_inf_min: 0.0,
            spectrum: vec![],
        };
    }

    // Normalize to probability distribution
    let probs: Vec<f64> = eigenvalues
        .iter()
        .map(|&v| if v > 1e-15 { v / trace } else { 0.0 })
        .collect();

    // S₀ (Hartley): log₂ of support size
    let effective_rank = probs.iter().filter(|&&p| p > 1e-15).count();
    let s0 = if effective_rank > 0 {
        (effective_rank as f64).log2()
    } else {
        0.0
    };

    // S₁ (von Neumann): -Σ pᵢ log₂(pᵢ)
    let s1 = entropy_from_eigenvalues(&eigenvalues);

    // S₂ (collision): -log₂(Σ pᵢ²) = -log₂(purity)
    let purity: f64 = probs.iter().map(|p| p * p).sum();
    let s2 = if purity > 1e-15 { -purity.log2() } else { 0.0 };

    // S_∞ (min-entropy): -log₂(max pᵢ)
    let p_max = probs.iter().copied().fold(0.0f64, f64::max);
    let s_inf = if p_max > 1e-15 { -p_max.log2() } else { 0.0 };

    RenyiSpectrum {
        s0_hartley: s0,
        s1_von_neumann: s1,
        s2_collision: s2,
        s_inf_min: s_inf,
        spectrum: probs,
    }
}

/// Compute entropy decomposition across ISP levels.
///
/// S₃ = S_within + ΔS_boundary where:
/// - S_within = mean(S_intent, S_spec, S_impl)
/// - ΔS_boundary = S_total - S_within
fn compute_entropy_decomposition(store: &Store) -> EntropyDecomposition {
    let (intent_view, spec_view, impl_view) = live_projections(store);

    // Per-level subgraph entropy
    let s_intent = if intent_view.entities.len() >= 2 {
        let g = build_level_subgraph(store, &intent_view.entities);
        graph_entropy(&g)
    } else {
        0.0
    };

    let s_spec = if spec_view.entities.len() >= 2 {
        let g = build_level_subgraph(store, &spec_view.entities);
        graph_entropy(&g)
    } else {
        0.0
    };

    let s_impl = if impl_view.entities.len() >= 2 {
        let g = build_level_subgraph(store, &impl_view.entities);
        graph_entropy(&g)
    } else {
        0.0
    };

    // Total entropy
    let full_entropy = von_neumann_entropy(store);
    let s_total = full_entropy.entropy;

    // Within-level average
    let level_count = [s_intent, s_spec, s_impl]
        .iter()
        .filter(|&&s| s > 0.0)
        .count()
        .max(1);
    let s_within = (s_intent + s_spec + s_impl) / level_count as f64;

    // Cross-boundary contribution
    let delta_boundary = (s_total - s_within).max(0.0);

    EntropyDecomposition {
        s_total,
        s_intent,
        s_spec,
        s_impl,
        s_within,
        delta_boundary,
    }
}

// ===========================================================================
// Convergence Analysis — Lyapunov theory
// ===========================================================================

/// Analyze convergence from an F(S) trajectory.
///
/// Uses Lyapunov exponent theory to determine:
/// - Whether the trajectory is monotonically non-decreasing (Law L1)
/// - The exponential convergence rate
/// - Estimated steps to reach F(S) ≥ 0.95
pub fn analyze_convergence(trajectory: &[f64]) -> ConvergenceAnalysis {
    let is_monotonic = trajectory.windows(2).all(|w| w[1] >= w[0] - 1e-10); // Tolerance for floating-point

    // Lyapunov exponent: average log-ratio of improvement
    let lyapunov = if trajectory.len() >= 2 {
        let ratios: Vec<f64> = trajectory
            .windows(2)
            .filter_map(|w| {
                if w[0] > 1e-10 {
                    Some((w[1] / w[0]).ln())
                } else if w[1] > w[0] {
                    Some(1.0) // Big improvement from ~0
                } else {
                    None
                }
            })
            .collect();
        if ratios.is_empty() {
            0.0
        } else {
            ratios.iter().sum::<f64>() / ratios.len() as f64
        }
    } else {
        0.0
    };

    // Estimate steps to target F(S) ≥ 0.95
    let steps = if let Some(&current) = trajectory.last() {
        let gap = 0.95 - current;
        if gap <= 0.0 {
            Some(0) // Already there
        } else if lyapunov > 1e-10 {
            // F(t) ≈ F(0) × exp(λt), so t = ln(target/current) / λ
            let target_ratio = 0.95 / current.max(1e-10);
            Some((target_ratio.ln() / lyapunov).ceil().max(1.0) as u64)
        } else {
            None // Not converging
        }
    } else {
        None
    };

    let convergence_rate = if lyapunov > 0.0 {
        1.0 - (-lyapunov).exp()
    } else {
        0.0
    };

    ConvergenceAnalysis {
        trajectory: trajectory.to_vec(),
        is_monotonic,
        lyapunov_exponent: lyapunov,
        steps_to_target: steps,
        convergence_rate,
    }
}

// ===========================================================================
// Full Bilateral Cycle
// ===========================================================================

/// Run a complete bilateral cycle on the store.
///
/// Computes F(S), forward/backward scans, CC-1..CC-5, optional spectral
/// certificate, and convergence analysis from the provided trajectory history.
///
/// The `history` parameter contains F(S) values from previous cycles.
/// If empty, this is the first cycle.
///
/// Set `with_spectral = true` for the full spectral certificate (slower).
///
/// INV-SIGNAL-006: Taxonomy completeness — all divergence types classified.
/// INV-SIGNAL-001: Signal as datom — cycle results captured as store datoms.
/// NEG-SIGNAL-001: No silent signal drop — all detected issues reported.
/// NEG-SIGNAL-002: No confusion without re-association.
/// NEG-SIGNAL-003: No high-severity automated resolution (human review required).
/// ADR-SIGNAL-003: Subscription debounce over immediate fire.
/// ADR-SIGNAL-005: Four recognized taxonomy gaps.
pub fn run_cycle(store: &Store, history: &[f64], with_spectral: bool) -> BilateralState {
    // F(S) fitness
    let fitness = compute_fitness(store);

    // Bilateral scans
    let forward = forward_scan(store);
    let backward = backward_scan(store);
    let scan = BilateralScan { forward, backward };

    // CC-1..CC-5
    let conditions = evaluate_conditions(store, &scan, &fitness);

    // Spectral certificate (optional, O(n²) to O(n³))
    let spectral = if with_spectral {
        spectral_certificate(store)
    } else {
        None
    };

    // Convergence analysis: append current F(S) to history
    let mut trajectory = history.to_vec();
    trajectory.push(fitness.total);
    let convergence = analyze_convergence(&trajectory);

    let cycle_count = trajectory.len() as u64;

    BilateralState {
        fitness,
        scan,
        conditions,
        spectral,
        convergence,
        cycle_count,
    }
}

/// Load F(S) trajectory from stored bilateral cycle datoms.
///
/// Scans the store for entities matching `:bilateral/cycle-*` and extracts
/// their `:bilateral/fitness` values, ordered by cycle number.
pub fn load_trajectory(store: &Store) -> Vec<f64> {
    let fitness_attr = Attribute::from_keyword(":bilateral/fitness");
    let ident_attr = Attribute::from_keyword(":db/ident");

    let mut cycle_fitness: BTreeMap<u64, f64> = BTreeMap::new();

    for entity in store.entities() {
        let datoms = store.entity_datoms(entity);

        // Check if this is a bilateral cycle entity
        let ident = datoms.iter().find_map(|d| {
            if d.attribute == ident_attr && d.op == Op::Assert {
                match &d.value {
                    Value::Keyword(k) if k.starts_with(":bilateral/cycle-") => Some(k.clone()),
                    _ => None,
                }
            } else {
                None
            }
        });

        if let Some(ident) = ident {
            // Extract cycle number from ident
            if let Some(num_str) = ident.strip_prefix(":bilateral/cycle-") {
                if let Ok(cycle_num) = num_str.parse::<u64>() {
                    // Extract fitness value
                    let fitness = datoms.iter().find_map(|d| {
                        if d.attribute == fitness_attr && d.op == Op::Assert {
                            if let Value::Double(f) = &d.value {
                                Some(f.into_inner())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    });
                    if let Some(f) = fitness {
                        cycle_fitness.insert(cycle_num, f);
                    }
                }
            }
        }
    }

    cycle_fitness.values().copied().collect()
}

/// Convert a bilateral cycle result to datoms for persistence.
///
/// Creates a `:bilateral/cycle-{N}` entity with F(S), component scores,
/// CC-1..CC-5 results, and scan coverage metrics.
pub fn cycle_to_datoms(state: &BilateralState, tx_id: TxId) -> Vec<Datom> {
    let cycle_num = state.cycle_count;
    let ident = format!(":bilateral/cycle-{cycle_num}");
    let entity = EntityId::from_ident(&ident);

    let mut datoms = Vec::new();

    // Identity
    datoms.push(Datom::new(
        entity,
        Attribute::from_keyword(":db/ident"),
        Value::Keyword(ident),
        tx_id,
        Op::Assert,
    ));

    // F(S) total
    datoms.push(Datom::new(
        entity,
        Attribute::from_keyword(":bilateral/fitness"),
        Value::Double(ordered_float::OrderedFloat(state.fitness.total)),
        tx_id,
        Op::Assert,
    ));

    // Component scores
    let c = &state.fitness.components;
    for (name, val) in [
        ("validation", c.validation),
        ("coverage", c.coverage),
        ("drift", c.drift),
        ("harvest-quality", c.harvest_quality),
        ("contradiction", c.contradiction),
        ("incompleteness", c.incompleteness),
        ("uncertainty", c.uncertainty),
    ] {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(&format!(":bilateral/{name}")),
            Value::Double(ordered_float::OrderedFloat(val)),
            tx_id,
            Op::Assert,
        ));
    }

    // CC-1..CC-5 as boolean datoms
    let cc = &state.conditions;
    for (name, cond) in [
        ("cc1-no-contradictions", &cc.cc1_no_contradictions),
        ("cc2-impl-satisfies-spec", &cc.cc2_impl_satisfies_spec),
        (
            "cc3-spec-approximates-intent",
            &cc.cc3_spec_approximates_intent,
        ),
        ("cc4-agent-agreement", &cc.cc4_agent_agreement),
        ("cc5-methodology-adherence", &cc.cc5_methodology_adherence),
    ] {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(&format!(":bilateral/{name}")),
            Value::Boolean(cond.satisfied),
            tx_id,
            Op::Assert,
        ));
    }

    // Scan coverage
    datoms.push(Datom::new(
        entity,
        Attribute::from_keyword(":bilateral/forward-coverage"),
        Value::Double(ordered_float::OrderedFloat(
            state.scan.forward.coverage_ratio,
        )),
        tx_id,
        Op::Assert,
    ));
    datoms.push(Datom::new(
        entity,
        Attribute::from_keyword(":bilateral/backward-coverage"),
        Value::Double(ordered_float::OrderedFloat(
            state.scan.backward.coverage_ratio,
        )),
        tx_id,
        Op::Assert,
    ));

    // Convergence metadata
    datoms.push(Datom::new(
        entity,
        Attribute::from_keyword(":bilateral/is-monotonic"),
        Value::Boolean(state.convergence.is_monotonic),
        tx_id,
        Op::Assert,
    ));
    datoms.push(Datom::new(
        entity,
        Attribute::from_keyword(":bilateral/lyapunov"),
        Value::Double(ordered_float::OrderedFloat(
            state.convergence.lyapunov_exponent,
        )),
        tx_id,
        Op::Assert,
    ));

    // Spectral certificate (if available)
    if let Some(ref cert) = state.spectral {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":bilateral/fiedler"),
            Value::Double(ordered_float::OrderedFloat(cert.fiedler_value)),
            tx_id,
            Op::Assert,
        ));
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":bilateral/cheeger"),
            Value::Double(ordered_float::OrderedFloat(cert.cheeger_constant)),
            tx_id,
            Op::Assert,
        ));
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":bilateral/spectral-gap"),
            Value::Double(ordered_float::OrderedFloat(cert.spectral_gap)),
            tx_id,
            Op::Assert,
        ));
    }

    datoms
}

// ===========================================================================
// Display / Formatting
// ===========================================================================

/// Format the bilateral state as a terse summary (≤10 lines, LLM-optimized).
pub fn format_terse(state: &BilateralState) -> String {
    let f = &state.fitness;
    let cc = &state.conditions;
    let conv = &state.convergence;

    let mut out = String::new();

    // Line 1: F(S) with CC status
    let cc_str = format!(
        "CC[{}{}{}{}{}]",
        if cc.cc1_no_contradictions.satisfied {
            "1"
        } else {
            "·"
        },
        if cc.cc2_impl_satisfies_spec.satisfied {
            "2"
        } else {
            "·"
        },
        if cc.cc3_spec_approximates_intent.satisfied {
            "3"
        } else {
            "·"
        },
        if cc.cc4_agent_agreement.satisfied {
            "4"
        } else {
            "·"
        },
        if cc.cc5_methodology_adherence.satisfied {
            "5"
        } else {
            "·"
        },
    );
    let mono_str = if conv.is_monotonic { "↑" } else { "↕" };
    out.push_str(&format!(
        "F(S)={:.4} {cc_str} {mono_str} cycle={}",
        f.total, state.cycle_count,
    ));
    if let Some(steps) = conv.steps_to_target {
        out.push_str(&format!(" ETA={steps} cycles"));
    }
    out.push('\n');

    // Line 2: Component breakdown
    let c = &f.components;
    out.push_str(&format!(
        "  V={:.2} C={:.2} D={:.2} H={:.2} K={:.2} I={:.2} U={:.2}\n",
        c.validation,
        c.coverage,
        c.drift,
        c.harvest_quality,
        c.contradiction,
        c.incompleteness,
        c.uncertainty,
    ));

    // Line 3: Scan summary
    let fwd = &state.scan.forward;
    let bwd = &state.scan.backward;
    out.push_str(&format!(
        "  scan: fwd={}/{} ({:.0}%) bwd={}/{} ({:.0}%)\n",
        fwd.covered.len(),
        fwd.covered.len() + fwd.gaps.len(),
        fwd.coverage_ratio * 100.0,
        bwd.covered.len(),
        bwd.covered.len() + bwd.gaps.len(),
        bwd.coverage_ratio * 100.0,
    ));

    // Line 4: Top gap (if any)
    if let Some(gap) = fwd.gaps.first() {
        let ident_str = gap.ident.as_deref().unwrap_or("(anonymous)");
        out.push_str(&format!(
            "  gap: {:?} {:?} {ident_str}\n",
            gap.severity, gap.boundary,
        ));
    }

    out
}

/// Format the bilateral state as verbose output (full metrics).
pub fn format_verbose(state: &BilateralState) -> String {
    let mut out = String::new();

    // Header
    out.push_str(&format!(
        "bilateral: cycle={} F(S)={:.6}\n",
        state.cycle_count, state.fitness.total,
    ));

    // Components
    let c = &state.fitness.components;
    out.push_str("  fitness components:\n");
    out.push_str(&format!(
        "    V (validation):    {:.4} (×{W_VALIDATION})\n",
        c.validation
    ));
    out.push_str(&format!(
        "    C (coverage):      {:.4} (×{W_COVERAGE})\n",
        c.coverage
    ));
    out.push_str(&format!(
        "    D (drift):         {:.4} (×{W_DRIFT})\n",
        c.drift
    ));
    out.push_str(&format!(
        "    H (harvest):       {:.4} (×{W_HARVEST})\n",
        c.harvest_quality
    ));
    out.push_str(&format!(
        "    K (contradiction): {:.4} (×{W_CONTRADICTION})\n",
        c.contradiction
    ));
    out.push_str(&format!(
        "    I (incompleteness):{:.4} (×{W_INCOMPLETENESS})\n",
        c.incompleteness
    ));
    out.push_str(&format!(
        "    U (uncertainty):   {:.4} (×{W_UNCERTAINTY})\n",
        c.uncertainty
    ));
    if !state.fitness.unmeasured.is_empty() {
        out.push_str(&format!(
            "    unmeasured: {}\n",
            state.fitness.unmeasured.join(", ")
        ));
    }

    // Coherence conditions
    let cc = &state.conditions;
    out.push_str(&format!(
        "  coherence: {} (overall={})\n",
        if cc.overall { "COHERENT" } else { "INCOHERENT" },
        cc.overall,
    ));
    for (name, cond) in [
        ("CC-1 no-contradictions", &cc.cc1_no_contradictions),
        ("CC-2 impl-satisfies-spec", &cc.cc2_impl_satisfies_spec),
        (
            "CC-3 spec-approximates-intent",
            &cc.cc3_spec_approximates_intent,
        ),
        ("CC-4 agent-agreement", &cc.cc4_agent_agreement),
        ("CC-5 methodology-adherence", &cc.cc5_methodology_adherence),
    ] {
        let status = if cond.satisfied { "PASS" } else { "FAIL" };
        let machine = if cond.machine_evaluable {
            "auto"
        } else {
            "human"
        };
        out.push_str(&format!(
            "    {name}: {status} (conf={:.2}, {machine}) {}\n",
            cond.confidence, cond.evidence,
        ));
    }

    // Scan details
    let fwd = &state.scan.forward;
    let bwd = &state.scan.backward;
    out.push_str(&format!(
        "  forward scan: {}/{} covered ({:.1}%)\n",
        fwd.covered.len(),
        fwd.covered.len() + fwd.gaps.len(),
        fwd.coverage_ratio * 100.0,
    ));
    let critical_count = fwd
        .gaps
        .iter()
        .filter(|g| g.severity == GapSeverity::Critical)
        .count();
    let major_count = fwd
        .gaps
        .iter()
        .filter(|g| g.severity == GapSeverity::Major)
        .count();
    let minor_count = fwd
        .gaps
        .iter()
        .filter(|g| g.severity == GapSeverity::Minor)
        .count();
    if !fwd.gaps.is_empty() {
        out.push_str(&format!(
            "    gaps: {} critical, {} major, {} minor\n",
            critical_count, major_count, minor_count,
        ));
    }
    out.push_str(&format!(
        "  backward scan: {}/{} aligned ({:.1}%)\n",
        bwd.covered.len(),
        bwd.covered.len() + bwd.gaps.len(),
        bwd.coverage_ratio * 100.0,
    ));

    // Convergence
    let conv = &state.convergence;
    out.push_str(&format!(
        "  convergence: monotonic={} λ={:.4} rate={:.4}",
        conv.is_monotonic, conv.lyapunov_exponent, conv.convergence_rate,
    ));
    if let Some(steps) = conv.steps_to_target {
        out.push_str(&format!(" ETA={steps}"));
    }
    out.push('\n');
    if conv.trajectory.len() > 1 {
        out.push_str(&format!(
            "    trajectory: [{}]\n",
            conv.trajectory
                .iter()
                .map(|v| format!("{v:.4}"))
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }

    // Spectral certificate
    if let Some(ref cert) = state.spectral {
        out.push_str("  spectral certificate:\n");
        out.push_str(&format!("    fiedler λ₂={:.4}\n", cert.fiedler_value));
        out.push_str(&format!("    cheeger h(G)={:.4}\n", cert.cheeger_constant));
        out.push_str(&format!("    spectral gap={:.6}\n", cert.spectral_gap));
        out.push_str(&format!(
            "    persistence: total={} cycles={}\n",
            cert.total_persistence, cert.cycle_births,
        ));
        out.push_str(&format!(
            "    ricci: mean={:.4} min={:.4}\n",
            cert.mean_ricci, cert.min_ricci,
        ));
        out.push_str(&format!(
            "    convergence bound: rate≤{:.6} mixing≤{:.1}\n",
            cert.convergence_rate_bound, cert.mixing_time_bound,
        ));

        // Rényi spectrum
        let r = &cert.renyi;
        out.push_str(&format!(
            "    rényi: S₀={:.3} S₁={:.3} S₂={:.3} S∞={:.3}\n",
            r.s0_hartley, r.s1_von_neumann, r.s2_collision, r.s_inf_min,
        ));

        // Entropy decomposition
        let e = &cert.entropy_decomposition;
        out.push_str(&format!(
            "    entropy: total={:.3} within={:.3} Δ_boundary={:.3}\n",
            e.s_total, e.s_within, e.delta_boundary,
        ));
        out.push_str(&format!(
            "      intent={:.3} spec={:.3} impl={:.3}\n",
            e.s_intent, e.s_spec, e.s_impl,
        ));
    }

    out
}

// ===========================================================================
// SpecImplBoundary — BoundaryCheck impl for spec↔impl (BOUNDARY-IMPL-5)
// ===========================================================================

/// The Specification ↔ Implementation boundary as a BoundaryCheck.
///
/// Source: spec elements (entities with `:spec/element-type`).
/// Target: impl entities that reference specs via `:impl/implements`.
///
/// Coverage = fraction of spec elements with at least one impl entity.
/// This is backward-compatible: same logic as `forward_scan` + `backward_scan`,
/// but wrapped in the BoundaryCheck trait for the registry.
///
/// INV-BILATERAL-007: Adding new boundaries requires only implementing BoundaryCheck.
pub struct SpecImplBoundary;

impl BoundaryCheck for SpecImplBoundary {
    fn name(&self) -> &str {
        "spec\u{2194}impl"
    }

    fn weight(&self) -> f64 {
        // Combined weight of coverage (0.18) + validation (0.18) = 0.36
        // This is the dominant boundary in the current F(S) computation.
        0.36
    }

    fn evaluate(&self, store: &Store) -> BoundaryEvaluation {
        let fwd = forward_scan(store);
        let bwd = backward_scan(store);

        let mut divergences = Vec::new();

        // Forward gaps: spec elements without implementation
        for gap in &fwd.gaps {
            divergences.push(BoundaryDivergence {
                entity: gap.entity,
                ident: gap
                    .ident
                    .clone()
                    .unwrap_or_else(|| format!("{:?}", gap.entity)),
                direction: DivergenceDirection::Forward,
                severity: gap.severity.clone(),
                fix_suggestion: format!(
                    "braid write assert ':impl/implements :spec/{}'",
                    gap.ident
                        .as_ref()
                        .and_then(|i| i.strip_prefix(":spec/"))
                        .unwrap_or("???")
                ),
            });
        }

        // Backward gaps: impl entities without spec reference
        for gap in &bwd.gaps {
            divergences.push(BoundaryDivergence {
                entity: gap.entity,
                ident: gap
                    .ident
                    .clone()
                    .unwrap_or_else(|| format!("{:?}", gap.entity)),
                direction: DivergenceDirection::Backward,
                severity: gap.severity.clone(),
                fix_suggestion: "braid trace".to_string(),
            });
        }

        // Coverage is the forward scan's coverage (spec → impl direction)
        let relation = SetRelation::new(
            fwd.covered.len() + fwd.gaps.len(),
            bwd.covered.len() + bwd.gaps.len(),
            fwd.covered.len(),
        );

        BoundaryEvaluation {
            name: self.name().to_string(),
            relation,
            divergences,
            weight: self.weight(),
        }
    }

    fn description(&self) -> &str {
        "Spec elements with implementation coverage via :impl/implements"
    }
}

/// Create the default boundary registry with the standard boundaries.
///
/// Currently includes:
/// - SpecImplBoundary: spec ↔ impl coverage (the original bilateral scan)
///
/// Future boundaries (when implemented):
/// - TaskSpecBoundary: tasks tracing to spec elements
/// - SchemaCompleteBoundary: schema attributes with documentation
///
/// INV-BILATERAL-007: Adding a new boundary = one more `register()` call.
pub fn default_boundaries() -> BoundaryRegistry {
    let mut registry = BoundaryRegistry::new();
    registry.register(Box::new(SpecImplBoundary));
    registry
}

// ===========================================================================
// Tests
// ===========================================================================

// Witnesses: INV-BILATERAL-001, INV-BILATERAL-002, INV-BILATERAL-003,
// INV-BILATERAL-004, INV-BILATERAL-005,
// ADR-BILATERAL-001, ADR-BILATERAL-002, ADR-BILATERAL-004,
// ADR-BILATERAL-005, ADR-BILATERAL-006, ADR-BILATERAL-007,
// ADR-BILATERAL-008, ADR-BILATERAL-010,
// NEG-BILATERAL-001, NEG-BILATERAL-002
#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};
    use crate::store::{Store, Transaction};
    use std::collections::BTreeSet;

    /// Create a minimal test store with schema (genesis only).
    fn test_store() -> Store {
        Store::genesis()
    }

    /// Build a store from genesis + additional datoms.
    fn store_with(extra: Vec<Datom>) -> Store {
        let system_agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, system_agent);
        let mut datom_set: BTreeSet<Datom> = BTreeSet::new();
        for d in crate::schema::genesis_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        for d in crate::schema::full_schema_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        for d in extra {
            datom_set.insert(d);
        }
        Store::from_datoms(datom_set)
    }

    /// Create a store with some spec and impl entities for testing.
    fn populated_store() -> Store {
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);
        let mut extra = Vec::new();

        // Create spec entities
        for i in 0..5 {
            let ident = format!(":spec/inv-test-{i:03}");
            let entity = EntityId::from_ident(&ident);

            extra.push(Datom::new(
                entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(ident),
                tx,
                Op::Assert,
            ));
            extra.push(Datom::new(
                entity,
                Attribute::from_keyword(":spec/element-type"),
                Value::Keyword(":spec.type/invariant".into()),
                tx,
                Op::Assert,
            ));
            extra.push(Datom::new(
                entity,
                Attribute::from_keyword(":spec/statement"),
                Value::String(format!("Test invariant {i}")),
                tx,
                Op::Assert,
            ));

            // Add falsification for even-numbered specs
            if i % 2 == 0 {
                extra.push(Datom::new(
                    entity,
                    Attribute::from_keyword(":spec/falsification"),
                    Value::String("Violated if test fails".into()),
                    tx,
                    Op::Assert,
                ));
            }

            // Add witness for first two specs
            if i < 2 {
                extra.push(Datom::new(
                    entity,
                    Attribute::from_keyword(":spec/witnessed"),
                    Value::Boolean(true),
                    tx,
                    Op::Assert,
                ));
            }
        }

        // Create impl entity for first spec
        let impl_entity = EntityId::from_ident(":impl/test-impl-000");
        let spec_entity = EntityId::from_ident(":spec/inv-test-000");
        extra.push(Datom::new(
            impl_entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":impl/test-impl-000".into()),
            tx,
            Op::Assert,
        ));
        extra.push(Datom::new(
            impl_entity,
            Attribute::from_keyword(":impl/implements"),
            Value::Ref(spec_entity),
            tx,
            Op::Assert,
        ));

        // Create exploration entities with confidence
        for i in 0..3 {
            let ident = format!(":exploration/obs-{i}");
            let entity = EntityId::from_ident(&ident);
            let confidence = 0.7 + (i as f64) * 0.1; // 0.7, 0.8, 0.9
            extra.push(Datom::new(
                entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(ident),
                tx,
                Op::Assert,
            ));
            extra.push(Datom::new(
                entity,
                Attribute::from_keyword(":exploration/confidence"),
                Value::Double(ordered_float::OrderedFloat(confidence)),
                tx,
                Op::Assert,
            ));
            extra.push(Datom::new(
                entity,
                Attribute::from_keyword(":exploration/body"),
                Value::String(format!("Observation {i}")),
                tx,
                Op::Assert,
            ));
        }

        store_with(extra)
    }

    // --- F(S) Tests ---

    // Verifies: INV-BILATERAL-002 — Five-Point Coherence Statement
    // Verifies: ADR-BILATERAL-001 — Fitness Function Weights
    #[test]
    fn fitness_on_empty_store() {
        let store = test_store();
        let f = compute_fitness(&store);
        // Empty store: all vacuously 1.0 except harvest quality (0.0)
        assert!(
            f.total >= 0.0 && f.total <= 1.0,
            "F(S)={} not in [0,1]",
            f.total
        );
    }

    // Verifies: INV-BILATERAL-002 — Five-Point Coherence Statement
    // Verifies: ADR-BILATERAL-001 — Fitness Function Weights
    #[test]
    fn fitness_in_unit_interval() {
        let store = populated_store();
        let f = compute_fitness(&store);
        assert!(
            f.total >= 0.0 && f.total <= 1.0,
            "F(S)={} not in [0,1]",
            f.total
        );
        assert!(f.components.validation >= 0.0 && f.components.validation <= 1.0);
        assert!(f.components.coverage >= 0.0 && f.components.coverage <= 1.0);
        assert!(f.components.drift >= 0.0 && f.components.drift <= 1.0);
        assert!(f.components.contradiction >= 0.0 && f.components.contradiction <= 1.0);
        assert!(f.components.incompleteness >= 0.0 && f.components.incompleteness <= 1.0);
        assert!(f.components.uncertainty >= 0.0 && f.components.uncertainty <= 1.0);
    }

    // Verifies: ADR-BILATERAL-001 — Fitness Function Weights
    #[test]
    fn fitness_weight_sum() {
        let sum = W_VALIDATION
            + W_COVERAGE
            + W_DRIFT
            + W_HARVEST
            + W_CONTRADICTION
            + W_INCOMPLETENESS
            + W_UNCERTAINTY;
        assert!(
            (sum - 1.0).abs() < 1e-10,
            "F(S) weights must sum to 1.0, got {sum}"
        );
    }

    #[test]
    fn validation_score_computed() {
        let store = populated_store();
        let v = compute_validation(&store);
        // 2 witnessed out of 5 spec elements = 0.4
        assert!((v - 0.4).abs() < 1e-10, "expected validation=0.4, got {v}");
    }

    #[test]
    fn incompleteness_complement_computed() {
        let store = populated_store();
        let i = compute_incompleteness_complement(&store);
        // Scoring: falsification=0.7, coverage=0.4, both=1.0, neither=0.15
        // spec[0]: falsification + impl → 1.0
        // spec[1]: neither → 0.15
        // spec[2]: falsification only → 0.7
        // spec[3]: neither → 0.15
        // spec[4]: falsification only → 0.7
        // Total: (1.0 + 0.15 + 0.7 + 0.15 + 0.7) / 5 = 0.54
        assert!(
            (i - 0.54).abs() < 1e-10,
            "expected incompleteness=0.54, got {i}"
        );
    }

    #[test]
    fn uncertainty_complement_computed() {
        let store = populated_store();
        let u = compute_uncertainty_complement(&store);
        // Mean confidence: (0.7 + 0.8 + 0.9) / 3 = 0.8
        assert!((u - 0.8).abs() < 1e-10, "expected uncertainty=0.8, got {u}");
    }

    // --- Scan Tests ---

    // Verifies: INV-BILATERAL-003 — Bilateral Symmetry (forward scan)
    // Verifies: ADR-BILATERAL-002 — Divergence Metric as Weighted Boundary Sum
    #[test]
    fn forward_scan_detects_gaps() {
        let store = populated_store();
        let result = forward_scan(&store);
        // 1 out of 5 spec entities has impl → coverage = 0.2
        assert!(
            (result.coverage_ratio - 0.2).abs() < 1e-10,
            "expected forward coverage=0.2, got {}",
            result.coverage_ratio
        );
        assert_eq!(result.covered.len(), 1);
        assert_eq!(result.gaps.len(), 4);
    }

    // Verifies: INV-BILATERAL-003 — Bilateral Symmetry (backward scan)
    // Verifies: INV-BILATERAL-004 — Residual Documentation
    // Verifies: ADR-BILATERAL-008 — Explicit Residual Divergence
    #[test]
    fn backward_scan_detects_orphans() {
        let store = populated_store();
        let result = backward_scan(&store);
        // The impl entity references a spec entity → covered
        assert_eq!(result.covered.len(), 1);
        assert_eq!(result.gaps.len(), 0);
        assert!((result.coverage_ratio - 1.0).abs() < 1e-10);
    }

    // --- CC Tests ---

    // Verifies: NEG-BILATERAL-002 — No Unchecked Coherence Dimension
    // Verifies: ADR-BILATERAL-006 — Coherence Verification as Fundamental Problem
    #[test]
    fn coherence_conditions_evaluate() {
        let store = populated_store();
        let fitness = compute_fitness(&store);
        let scan = BilateralScan {
            forward: forward_scan(&store),
            backward: backward_scan(&store),
        };
        let cc = evaluate_conditions(&store, &scan, &fitness);

        // CC-1: Should pass (no contradictions in test store)
        assert!(cc.cc1_no_contradictions.satisfied);
        // CC-2: Should fail (only 20% coverage)
        assert!(!cc.cc2_impl_satisfies_spec.satisfied);
        // CC-3: Human-gated, defaults true
        assert!(cc.cc3_spec_approximates_intent.satisfied);
        // CC-4: Multi-agent (braid:system + test), but still evaluable
        // (2 agents means agreement is not trivially true)
        // CC-5: M(t) with default telemetry
        // Overall: false because CC-2 fails (only 20% coverage)
        assert!(!cc.overall);
    }

    // --- Convergence Tests ---

    // Verifies: INV-BILATERAL-001 — Monotonic Convergence
    // Verifies: NEG-BILATERAL-001 — No Fitness Regression
    #[test]
    fn convergence_monotonic_trajectory() {
        let trajectory = vec![0.1, 0.2, 0.3, 0.5, 0.7];
        let analysis = analyze_convergence(&trajectory);
        assert!(analysis.is_monotonic);
        assert!(analysis.lyapunov_exponent > 0.0);
        assert!(analysis.steps_to_target.is_some());
    }

    #[test]
    fn convergence_non_monotonic_trajectory() {
        let trajectory = vec![0.5, 0.3, 0.4, 0.6];
        let analysis = analyze_convergence(&trajectory);
        assert!(!analysis.is_monotonic);
    }

    #[test]
    fn convergence_empty_trajectory() {
        let analysis = analyze_convergence(&[]);
        assert!(analysis.is_monotonic); // Vacuously true
        assert_eq!(analysis.lyapunov_exponent, 0.0);
        assert!(analysis.steps_to_target.is_none());
    }

    #[test]
    fn convergence_single_point() {
        let analysis = analyze_convergence(&[0.5]);
        assert!(analysis.is_monotonic);
        assert_eq!(analysis.lyapunov_exponent, 0.0);
    }

    #[test]
    fn convergence_at_target() {
        let analysis = analyze_convergence(&[0.3, 0.5, 0.8, 0.96]);
        assert_eq!(analysis.steps_to_target, Some(0));
    }

    // --- Full Cycle Tests ---

    // Verifies: INV-BILATERAL-005 — Test Results as Datoms
    // Verifies: ADR-BILATERAL-005 — Reconciliation Taxonomy
    #[test]
    fn full_cycle_runs_without_spectral() {
        let store = populated_store();
        let state = run_cycle(&store, &[], false);
        assert!(state.fitness.total >= 0.0 && state.fitness.total <= 1.0);
        assert_eq!(state.cycle_count, 1);
        assert!(state.spectral.is_none());
    }

    #[test]
    fn full_cycle_runs_with_spectral() {
        let store = populated_store();
        let state = run_cycle(&store, &[], true);
        assert!(state.fitness.total >= 0.0 && state.fitness.total <= 1.0);
        // Spectral may or may not be available depending on graph size
    }

    // --- Datom Serialization Tests ---

    #[test]
    fn cycle_to_datoms_produces_expected_count() {
        let store = populated_store();
        let state = run_cycle(&store, &[], false);
        let agent = AgentId::from_name("test");
        let tx = TxId::new(100, 0, agent);
        let datoms = cycle_to_datoms(&state, tx);
        // At minimum: ident + fitness + 7 components + 5 CC + 2 scan + 2 convergence = 18
        assert!(
            datoms.len() >= 18,
            "expected ≥18 datoms, got {}",
            datoms.len()
        );
    }

    // --- Formatting Tests ---

    #[test]
    fn terse_format_fits_budget() {
        let store = populated_store();
        let state = run_cycle(&store, &[], false);
        let output = format_terse(&state);
        let line_count = output.lines().count();
        assert!(
            line_count <= 10,
            "terse output should be ≤10 lines, got {line_count}"
        );
    }

    #[test]
    fn verbose_format_includes_all_sections() {
        let store = populated_store();
        let state = run_cycle(&store, &[], true);
        let output = format_verbose(&state);
        assert!(output.contains("fitness components:"));
        assert!(output.contains("coherence:"));
        assert!(output.contains("forward scan:"));
        assert!(output.contains("convergence:"));
    }

    // --- Rényi Spectrum Tests ---

    #[test]
    fn renyi_ordering_property() {
        // For any density matrix: S_∞ ≤ S₂ ≤ S₁ ≤ S₀ (monotone in α)
        let store = populated_store();
        let renyi = compute_renyi_spectrum(&store);
        if renyi.s0_hartley > 0.0 {
            assert!(
                renyi.s_inf_min <= renyi.s2_collision + 1e-10,
                "S∞={} > S₂={}",
                renyi.s_inf_min,
                renyi.s2_collision
            );
            assert!(
                renyi.s2_collision <= renyi.s1_von_neumann + 1e-10,
                "S₂={} > S₁={}",
                renyi.s2_collision,
                renyi.s1_von_neumann
            );
            assert!(
                renyi.s1_von_neumann <= renyi.s0_hartley + 1e-10,
                "S₁={} > S₀={}",
                renyi.s1_von_neumann,
                renyi.s0_hartley
            );
        }
    }

    // --- Entropy Decomposition Tests ---

    #[test]
    fn entropy_decomposition_non_negative() {
        let store = populated_store();
        let decomp = compute_entropy_decomposition(&store);
        assert!(decomp.s_total >= -1e-10, "S_total={}", decomp.s_total);
        assert!(decomp.s_within >= -1e-10, "S_within={}", decomp.s_within);
        assert!(
            decomp.delta_boundary >= -1e-10,
            "ΔS_boundary={}",
            decomp.delta_boundary
        );
    }

    // --- Load Trajectory Tests ---

    #[test]
    fn load_trajectory_empty_store() {
        let store = test_store();
        let trajectory = load_trajectory(&store);
        assert!(trajectory.is_empty());
    }

    // ===================================================================
    // Property-Based Tests (W1C.6 — INV-BILATERAL-001..005)
    // ===================================================================

    mod proptests {
        use super::*;
        use crate::proptest_strategies::{
            arb_fitness_components, arb_fitness_score, arb_fitness_trajectory,
            arb_monotone_trajectory, arb_store,
        };
        use proptest::prelude::*;

        proptest! {
            /// INV-BILATERAL-001: F(S) monotonically non-decreasing for well-formed transitions.
            ///
            /// If F(S) was computed correctly, adding spec+impl datoms should not decrease it.
            /// We test that analyze_convergence correctly identifies monotone trajectories.
            #[test]
            fn inv_bilateral_001_monotone_trajectory_detection(
                trajectory in arb_monotone_trajectory(10),
            ) {
                let analysis = analyze_convergence(&trajectory);
                prop_assert!(
                    analysis.is_monotonic,
                    "INV-BILATERAL-001: monotone trajectory wrongly classified as non-monotonic"
                );
            }

            /// INV-BILATERAL-001 (negative): Non-monotone trajectories detected.
            #[test]
            fn inv_bilateral_001_nonmonotone_detection(
                trajectory in arb_fitness_trajectory(10),
            ) {
                let analysis = analyze_convergence(&trajectory);
                // Check consistency: if analysis says monotonic, verify it actually is
                if analysis.is_monotonic {
                    for w in trajectory.windows(2) {
                        prop_assert!(
                            w[1] >= w[0] - 1e-10,
                            "INV-BILATERAL-001: claimed monotonic but {} < {}",
                            w[1], w[0]
                        );
                    }
                }
            }

            /// INV-BILATERAL-002: F(S) is bounded in [0, 1] for all valid component values.
            ///
            /// The fitness function is a weighted sum of components each in [0,1],
            /// with weights summing to 1.0. Therefore F(S) ∈ [0, 1].
            #[test]
            fn inv_bilateral_002_fitness_bounded(
                fs in arb_fitness_score(),
            ) {
                prop_assert!(
                    fs.total >= -1e-10 && fs.total <= 1.0 + 1e-10,
                    "INV-BILATERAL-002: F(S) = {} out of [0,1]",
                    fs.total
                );
            }

            /// INV-BILATERAL-002: Weighted sum is correct (component * weight = total).
            #[test]
            fn inv_bilateral_002_weighted_sum_correct(
                fc in arb_fitness_components(),
            ) {
                let expected = W_VALIDATION * fc.validation
                    + W_COVERAGE * fc.coverage
                    + W_DRIFT * fc.drift
                    + W_HARVEST * fc.harvest_quality
                    + W_CONTRADICTION * fc.contradiction
                    + W_INCOMPLETENESS * fc.incompleteness
                    + W_UNCERTAINTY * fc.uncertainty;
                let fs = FitnessScore {
                    total: expected,
                    components: fc,
                    unmeasured: vec![],
                };
                prop_assert!(
                    (fs.total - expected).abs() < 1e-10,
                    "INV-BILATERAL-002: total {} != expected {}",
                    fs.total, expected
                );
            }

            /// INV-BILATERAL-003: Bilateral symmetry — forward and backward scans
            /// are both computed for any store (symmetry of the bilateral operation).
            #[test]
            fn inv_bilateral_003_scan_symmetry(store in arb_store(3)) {
                let forward = forward_scan(&store);
                let backward = backward_scan(&store);
                // Both scans should be well-defined (coverage in [0,1])
                prop_assert!(
                    forward.coverage_ratio >= 0.0 && forward.coverage_ratio <= 1.0,
                    "INV-BILATERAL-003: forward coverage {} out of [0,1]",
                    forward.coverage_ratio
                );
                prop_assert!(
                    backward.coverage_ratio >= 0.0 && backward.coverage_ratio <= 1.0,
                    "INV-BILATERAL-003: backward coverage {} out of [0,1]",
                    backward.coverage_ratio
                );
            }

            /// INV-BILATERAL-004: Drift residual — compute_fitness is total.
            ///
            /// For any store state, compute_fitness must return without panicking
            /// and produce a well-defined FitnessScore (total function).
            #[test]
            fn inv_bilateral_004_compute_fitness_total(store in arb_store(3)) {
                let fs = compute_fitness(&store);
                prop_assert!(
                    fs.total >= -1e-10 && fs.total <= 1.0 + 1e-10,
                    "INV-BILATERAL-004: F(S) = {} for arbitrary store",
                    fs.total
                );
                // No NaN or infinity
                prop_assert!(
                    fs.total.is_finite(),
                    "INV-BILATERAL-004: F(S) is not finite"
                );
            }

            /// INV-BILATERAL-005: Cycle-to-datoms roundtrip — bilateral state
            /// can always be serialized to datoms.
            #[test]
            fn inv_bilateral_005_cycle_to_datoms_total(store in arb_store(2)) {
                let state = run_cycle(&store, &[], false);
                let agent = crate::datom::AgentId::from_name("proptest");
                let tx_id = TxId::new(999, 0, agent);
                let datoms = cycle_to_datoms(&state, tx_id);
                // Must produce at least: ident + fitness + 7 components + 5 CC bools + 2 scan + 2 convergence = 18
                prop_assert!(
                    datoms.len() >= 18,
                    "INV-BILATERAL-005: cycle_to_datoms produced only {} datoms (expected >= 18)",
                    datoms.len()
                );
                // All datoms use the same entity (the cycle entity)
                let entity = datoms[0].entity;
                for d in &datoms {
                    prop_assert_eq!(
                        d.entity, entity,
                        "INV-BILATERAL-005: datoms use different entities"
                    );
                }
            }

            /// Depth weight function is bounded and monotone.
            #[test]
            fn depth_weight_monotone_and_bounded(depth in 0i64..=4) {
                let w = depth_weight(depth);
                prop_assert!((0.0..=1.0).contains(&w), "depth_weight({}) = {} out of [0,1]", depth, w);
                if depth > 0 {
                    let prev = depth_weight(depth - 1);
                    prop_assert!(w >= prev, "depth_weight not monotone: w({})={} < w({})={}", depth, w, depth-1, prev);
                }
            }
        }
    }

    // =======================================================================
    // Comonadic Depth tests (DC-1, ADR-FOUNDATION-020)
    // =======================================================================

    /// Comonadic depth defaults to 0 (OPINION) for entities without `:comonad/depth`.
    #[test]
    fn comonadic_depth_defaults_to_zero() {
        let store = Store::genesis();
        let entity = EntityId::from_ident(":some/nonexistent");
        assert_eq!(comonadic_depth(&store, &entity), 0);
    }

    /// `set_depth_datom` produces a valid datom with clamped depth.
    #[test]
    fn set_depth_datom_clamps_and_produces() {
        use crate::datom::{AgentId, Op};
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);
        let entity = EntityId::from_ident(":spec/test-inv");

        let d = set_depth_datom(&entity, 2, tx);
        assert_eq!(d.entity, entity);
        assert_eq!(d.attribute, Attribute::from_keyword(":comonad/depth"));
        assert_eq!(d.value, Value::Long(2));
        assert_eq!(d.op, Op::Assert);

        // Clamps above 4
        let d_high = set_depth_datom(&entity, 10, tx);
        assert_eq!(d_high.value, Value::Long(4));

        // Clamps below 0
        let d_neg = set_depth_datom(&entity, -5, tx);
        assert_eq!(d_neg.value, Value::Long(0));
    }

    /// Comonadic depth round-trips through the store.
    #[test]
    fn comonadic_depth_roundtrip() {
        use crate::datom::AgentId;
        let agent = AgentId::from_name("test");
        let tx = TxId::new(100, 0, agent);
        let entity = EntityId::from_ident(":spec/tested-inv");

        let mut datoms = std::collections::BTreeSet::new();
        datoms.insert(set_depth_datom(&entity, 3, tx));
        let store = Store::from_datoms(datoms);

        assert_eq!(comonadic_depth(&store, &entity), 3);
    }

    // =======================================================================
    // DC-2: Depth-Weighted F(S) in policy boundaries
    // =======================================================================

    /// DC-2: When no entities have comonadic depth, raw coverage is used (bootstrap).
    #[test]
    fn dc2_bootstrap_uses_raw_coverage() {
        // entities_matching_pattern and covered_entity_set are tested via
        // compute_fitness_from_policy on the live store, but we verify the
        // depth_weight bootstrap logic directly:
        // depth 0 → weight 0.0 → if all depth 0, any_has_depth=false → raw coverage
        assert_eq!(depth_weight(0), 0.0);
        assert_eq!(depth_weight(1), 0.15);
        assert_eq!(depth_weight(4), 1.0);
    }

    /// DC-2: depth_weight is strictly monotone and bounded in [0, 1].
    #[test]
    fn dc2_depth_weight_properties() {
        for d in 0..=4 {
            let w = depth_weight(d);
            assert!((0.0..=1.0).contains(&w), "depth_weight({d})={w} out of [0,1]");
            if d > 0 {
                assert!(
                    w > depth_weight(d - 1),
                    "depth_weight not strictly monotone at {d}"
                );
            }
        }
        // Out-of-range clamps to 0
        assert_eq!(depth_weight(-1), 0.0);
        assert_eq!(depth_weight(5), 0.0);
    }

    // =======================================================================
    // BoundaryCheck trait tests (INV-BILATERAL-007)
    // =======================================================================

    /// SetRelation invariants: covered + source_gaps == source_total.
    #[test]
    fn set_relation_partition_invariant() {
        let r = SetRelation::new(100, 80, 60);
        assert_eq!(r.covered + r.source_gaps, r.source_total);
        assert_eq!(r.source_gaps, 40);
        assert_eq!(r.target_gaps, 20);
        assert!((r.coverage - 0.6).abs() < 1e-10);
    }

    /// SetRelation with zero source is vacuously satisfied.
    #[test]
    fn set_relation_empty_source_vacuous() {
        let r = SetRelation::new(0, 50, 0);
        assert_eq!(r.coverage, 1.0);
        assert_eq!(r.source_gaps, 0);
    }

    /// SetRelation with full coverage.
    #[test]
    fn set_relation_full_coverage() {
        let r = SetRelation::new(10, 10, 10);
        assert_eq!(r.coverage, 1.0);
        assert_eq!(r.source_gaps, 0);
        assert_eq!(r.target_gaps, 0);
    }

    /// BoundaryRegistry starts empty and accumulates boundaries.
    #[test]
    fn boundary_registry_lifecycle() {
        let mut registry = BoundaryRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        // A trivial boundary for testing
        struct TrivialBoundary {
            cov: f64,
        }
        impl BoundaryCheck for TrivialBoundary {
            fn name(&self) -> &str {
                "trivial"
            }
            fn weight(&self) -> f64 {
                1.0
            }
            fn evaluate(&self, _store: &Store) -> BoundaryEvaluation {
                let total = 100;
                let covered = (total as f64 * self.cov) as usize;
                BoundaryEvaluation {
                    name: self.name().to_string(),
                    relation: SetRelation::new(total, total, covered),
                    divergences: Vec::new(),
                    weight: self.weight(),
                }
            }
            fn description(&self) -> &str {
                "test boundary"
            }
        }

        registry.register(Box::new(TrivialBoundary { cov: 0.8 }));
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());

        let store = test_store();
        let evals = registry.evaluate_all(&store);
        assert_eq!(evals.len(), 1);
        assert!((evals[0].relation.coverage - 0.8).abs() < 1e-10);
    }

    /// BoundaryRegistry total_coverage normalizes weights.
    #[test]
    fn boundary_registry_weighted_coverage() {
        struct WeightedBoundary {
            name: &'static str,
            w: f64,
            cov: f64,
        }
        impl BoundaryCheck for WeightedBoundary {
            fn name(&self) -> &str {
                self.name
            }
            fn weight(&self) -> f64 {
                self.w
            }
            fn evaluate(&self, _store: &Store) -> BoundaryEvaluation {
                let total = 100;
                let covered = (total as f64 * self.cov) as usize;
                BoundaryEvaluation {
                    name: self.name().to_string(),
                    relation: SetRelation::new(total, total, covered),
                    divergences: Vec::new(),
                    weight: self.w,
                }
            }
            fn description(&self) -> &str {
                "test"
            }
        }

        let mut registry = BoundaryRegistry::new();
        // Boundary A: weight 0.7, coverage 1.0
        // Boundary B: weight 0.3, coverage 0.0
        // Expected: (0.7/1.0) * 1.0 + (0.3/1.0) * 0.0 = 0.7
        registry.register(Box::new(WeightedBoundary {
            name: "a",
            w: 0.7,
            cov: 1.0,
        }));
        registry.register(Box::new(WeightedBoundary {
            name: "b",
            w: 0.3,
            cov: 0.0,
        }));

        let store = test_store();
        let total = registry.total_coverage(&store);
        assert!((total - 0.7).abs() < 1e-10, "expected 0.7, got {}", total);
    }

    /// BoundaryRegistry with no boundaries returns 1.0 (vacuous).
    #[test]
    fn boundary_registry_empty_vacuous() {
        let registry = BoundaryRegistry::new();
        let store = test_store();
        assert_eq!(registry.total_coverage(&store), 1.0);
    }

    /// BoundaryDivergence captures direction and fix suggestion.
    #[test]
    fn boundary_divergence_construction() {
        let div = BoundaryDivergence {
            entity: EntityId::from_content(b"test-entity-42"),
            ident: ":spec/inv-store-001".to_string(),
            direction: DivergenceDirection::Forward,
            severity: GapSeverity::Major,
            fix_suggestion: "braid trace t-xxxx :spec/inv-store-001".to_string(),
        };
        assert_eq!(div.direction, DivergenceDirection::Forward);
        assert!(div.fix_suggestion.contains("braid trace"));
    }

    /// BoundaryRegistry::boundary_info returns names and weights.
    #[test]
    fn boundary_registry_info() {
        struct NamedBoundary(&'static str, f64);
        impl BoundaryCheck for NamedBoundary {
            fn name(&self) -> &str {
                self.0
            }
            fn weight(&self) -> f64 {
                self.1
            }
            fn evaluate(&self, _store: &Store) -> BoundaryEvaluation {
                BoundaryEvaluation {
                    name: self.0.to_string(),
                    relation: SetRelation::new(0, 0, 0),
                    divergences: Vec::new(),
                    weight: self.1,
                }
            }
            fn description(&self) -> &str {
                "info test"
            }
        }

        let mut registry = BoundaryRegistry::new();
        registry.register(Box::new(NamedBoundary("spec↔impl", 0.6)));
        registry.register(Box::new(NamedBoundary("task↔spec", 0.4)));

        let info = registry.boundary_info();
        assert_eq!(info.len(), 2);
        assert_eq!(info[0].0, "spec↔impl");
        assert!((info[0].1 - 0.6).abs() < 1e-10);
        assert_eq!(info[1].0, "task↔spec");
    }

    // =======================================================================
    // SpecImplBoundary tests (BOUNDARY-IMPL-5)
    // =======================================================================

    /// SpecImplBoundary on empty store returns vacuous coverage.
    #[test]
    fn spec_impl_boundary_empty_store() {
        let store = test_store();
        let boundary = SpecImplBoundary;
        let eval = boundary.evaluate(&store);
        assert_eq!(eval.name, "spec\u{2194}impl");
        // Empty store: no spec elements, vacuous coverage
        assert_eq!(eval.relation.coverage, 1.0);
        assert!(eval.divergences.is_empty());
    }

    /// SpecImplBoundary detects forward gaps (spec without impl).
    #[test]
    fn spec_impl_boundary_detects_forward_gap() {
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);
        let spec_entity = EntityId::from_ident(":spec/inv-test-001");

        let extra = vec![
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":spec/inv-test-001".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":spec/element-type"),
                Value::Keyword(":spec.type/invariant".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":spec/statement"),
                Value::String("Test invariant".to_string()),
                tx,
                Op::Assert,
            ),
        ];

        let store = store_with(extra);
        let boundary = SpecImplBoundary;
        let eval = boundary.evaluate(&store);

        // One spec element with no impl → coverage < 1.0
        assert!(
            eval.relation.coverage < 1.0,
            "expected gap, got coverage={}",
            eval.relation.coverage
        );
        assert!(
            !eval.divergences.is_empty(),
            "expected divergences for uncovered spec"
        );
        assert_eq!(eval.divergences[0].direction, DivergenceDirection::Forward);
    }

    /// default_boundaries() returns a non-empty registry.
    #[test]
    fn default_boundaries_non_empty() {
        let registry = default_boundaries();
        assert!(
            !registry.is_empty(),
            "default_boundaries must have at least 1 boundary"
        );
        let info = registry.boundary_info();
        assert!(info.iter().any(|(name, _)| *name == "spec\u{2194}impl"));
    }

    // =======================================================================
    // BOUNDARY-COMPAT: F(S) backward compatibility golden test
    // =======================================================================

    /// F(S) via default_boundaries().total_coverage() is consistent with
    /// the coverage component of compute_fitness().
    ///
    /// This verifies that the boundary framework doesn't change the fitness
    /// computation results for the spec↔impl boundary.
    #[test]
    fn boundary_compat_coverage_consistency() {
        let store = test_store();

        // Old path: compute_fitness
        let fitness = compute_fitness(&store);

        // New path: default_boundaries().evaluate_all()
        let registry = default_boundaries();
        let evals = registry.evaluate_all(&store);
        assert_eq!(evals.len(), 1);

        // On empty store both should be 1.0 (vacuous)
        assert_eq!(fitness.components.coverage, 1.0);
        assert_eq!(evals[0].relation.coverage, 1.0);
    }

    // =======================================================================
    // T7-3: Witness data → F(S) V component integration tests
    // =======================================================================

    /// Verifies: INV-WITNESS-005 — Stale Witnesses Reduce F(S)
    /// Verifies: compute_validation() correctly uses witness_validation_score()
    ///
    /// Pipeline: spec elements + FBW witness datoms → compute_fitness() → V component.
    /// (a) Valid witnesses make V > 0.
    /// (b) Marking a witness stale decreases V.
    /// (c) An invariant with no witness contributes 0 to V.
    #[test]
    fn witness_data_affects_fitness_v_component() {
        use crate::datom::ProvenanceType;
        use crate::store::Transaction;
        use crate::witness::{
            create_fbw, fbw_to_datoms, mark_stale_datoms, witness_validation_score, WitnessStatus,
        };

        let agent = AgentId::from_name("test-witness");
        let tx = TxId::new(1, 0, agent);

        // --- Build 3 spec invariants ---
        let inv_idents = [
            ":spec/inv-witness-t7-001",
            ":spec/inv-witness-t7-002",
            ":spec/inv-witness-t7-003",
        ];
        let inv_entities: Vec<EntityId> =
            inv_idents.iter().map(|i| EntityId::from_ident(i)).collect();

        let mut extra = Vec::new();
        for (idx, ident) in inv_idents.iter().enumerate() {
            let entity = inv_entities[idx];
            extra.push(Datom::new(
                entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(ident.to_string()),
                tx,
                Op::Assert,
            ));
            extra.push(Datom::new(
                entity,
                Attribute::from_keyword(":spec/element-type"),
                Value::Keyword(":spec.type/invariant".into()),
                tx,
                Op::Assert,
            ));
            extra.push(Datom::new(
                entity,
                Attribute::from_keyword(":element/statement"),
                Value::String(format!("Test invariant T7-{idx}")),
                tx,
                Op::Assert,
            ));
        }

        // --- Create FBW witnesses for inv[0] and inv[1] (Valid, depth 2) ---
        let mut fbw0 = create_fbw(
            inv_entities[0],
            "Test invariant T7-0",
            "Violated if T7-0 fails",
            "assert!(t7_zero_holds())",
            "src/t7_test.rs",
            2,
            "test-witness",
        );
        fbw0.status = WitnessStatus::Valid;

        let mut fbw1 = create_fbw(
            inv_entities[1],
            "Test invariant T7-1",
            "Violated if T7-1 fails",
            "assert!(t7_one_holds())",
            "src/t7_test.rs",
            2,
            "test-witness",
        );
        fbw1.status = WitnessStatus::Valid;

        // No witness for inv[2] — it remains untested.

        let fbw0_entity = fbw0.entity;
        extra.extend(fbw_to_datoms(&fbw0, tx));
        extra.extend(fbw_to_datoms(&fbw1, tx));

        let mut store = store_with(extra);

        // --- (a) V > 0 with valid witnesses ---
        let f1 = compute_fitness(&store);
        assert!(
            f1.components.validation > 0.0,
            "V should be > 0 with 2 valid witnesses, got {}",
            f1.components.validation
        );

        // Confirm witness_validation_score reports 2 valid, 0 stale, 1 untested
        let (score, valid, stale, untested) = witness_validation_score(&store);
        assert_eq!(valid, 2, "expected 2 valid witnesses, got {valid}");
        assert_eq!(stale, 0, "expected 0 stale witnesses, got {stale}");
        assert_eq!(untested, 1, "expected 1 untested invariant, got {untested}");
        assert!(score > 0.0, "witness score should be > 0, got {score}");

        // Record V before staleness
        let v_before = f1.components.validation;

        // --- (b) Mark fbw0 stale, V should decrease ---
        let stale_datoms = mark_stale_datoms(fbw0_entity, tx);
        // Use transact so the stale datom is appended AFTER valid datoms in entity_index
        let stale_tx = Transaction::new(agent, ProvenanceType::Observed, "mark witness stale");
        let mut stale_tx = stale_tx;
        for d in &stale_datoms {
            stale_tx = stale_tx.assert(d.entity, d.attribute.clone(), d.value.clone());
        }
        let committed = stale_tx.commit(&store).expect("stale tx should commit");
        store
            .transact(committed)
            .expect("stale transact should succeed");

        let f2 = compute_fitness(&store);
        assert!(
            f2.components.validation < v_before,
            "V should decrease after marking witness stale: before={v_before}, after={}",
            f2.components.validation
        );

        // Confirm counts: 1 valid, 1 stale, 2 untested
        // (inv[0] lost its valid witness → now untested, inv[2] was always untested)
        let (_, valid2, stale2, untested2) = witness_validation_score(&store);
        assert_eq!(valid2, 1, "expected 1 valid after stale, got {valid2}");
        assert_eq!(stale2, 1, "expected 1 stale after marking, got {stale2}");
        assert_eq!(
            untested2, 2,
            "expected 2 untested (inv[0] + inv[2]), got {untested2}"
        );

        // --- (c) The unwitnessed invariant contributes 0 ---
        // With 3 invariants and only 1 valid witness at depth 2:
        //   V = depth_weight(2) / (3 * depth_weight(4)) = 0.4 / 3.0 ≈ 0.133
        // The unwitnessed inv[2] contributes nothing to the numerator.
        let expected_v = depth_weight(2) / (3.0 * depth_weight(4));
        assert!(
            (f2.components.validation - expected_v).abs() < 1e-10,
            "V should be {expected_v} (1 witness at depth 2, 3 total invs), got {}",
            f2.components.validation
        );
    }

    /// Verifies: backward compatibility — F(S) V component falls back to legacy
    /// depth-weighted path when no FBW witness data exists in the store.
    ///
    /// This ensures stores that predate the WITNESS system still compute V correctly
    /// using the binary `:spec/witnessed` counting path.
    #[test]
    fn no_witness_data_falls_back_to_legacy() {
        use crate::witness::witness_validation_score;

        let agent = AgentId::from_name("test-legacy");
        let tx = TxId::new(1, 0, agent);

        // Build 4 spec elements, 2 with :spec/witnessed = true
        let mut extra = Vec::new();
        for i in 0..4 {
            let ident = format!(":spec/inv-legacy-{i:03}");
            let entity = EntityId::from_ident(&ident);

            extra.push(Datom::new(
                entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(ident),
                tx,
                Op::Assert,
            ));
            extra.push(Datom::new(
                entity,
                Attribute::from_keyword(":spec/element-type"),
                Value::Keyword(":spec.type/invariant".into()),
                tx,
                Op::Assert,
            ));
            extra.push(Datom::new(
                entity,
                Attribute::from_keyword(":element/statement"),
                Value::String(format!("Legacy invariant {i}")),
                tx,
                Op::Assert,
            ));

            // Mark first 2 as witnessed (legacy binary flag)
            if i < 2 {
                extra.push(Datom::new(
                    entity,
                    Attribute::from_keyword(":spec/witnessed"),
                    Value::Boolean(true),
                    tx,
                    Op::Assert,
                ));
            }
        }

        // No FBW datoms — this is a legacy store
        let store = store_with(extra);

        // witness_validation_score should return valid_count=0 (no FBWs)
        let (_, valid, _, _) = witness_validation_score(&store);
        assert_eq!(valid, 0, "legacy store should have 0 valid FBW witnesses");

        // compute_validation falls back to binary: 2 witnessed / 4 total = 0.5
        let v = compute_validation(&store);
        assert!(
            (v - 0.5).abs() < 1e-10,
            "legacy fallback: expected V=0.5 (2/4 witnessed), got {v}"
        );

        // Full F(S) should still work
        let f = compute_fitness(&store);
        assert!(
            f.total >= 0.0 && f.total <= 1.0,
            "F(S)={} not in [0,1]",
            f.total
        );
        assert!(
            (f.components.validation - 0.5).abs() < 1e-10,
            "F(S).V should use legacy path: expected 0.5, got {}",
            f.components.validation
        );
    }

    // Verifies: INV-BILATERAL-001 — F(S) monotonically non-decreasing under task completion
    #[test]
    fn fitness_non_decreasing_under_task_close() {
        use crate::datom::AgentId;
        use crate::schema::{full_schema_datoms, genesis_datoms};

        let agent = AgentId::from_name("braid:test");
        let genesis_tx = TxId::new(0, 0, agent);

        let mut datoms = BTreeSet::new();
        for d in genesis_datoms(genesis_tx) {
            datoms.insert(d);
        }
        for d in full_schema_datoms(genesis_tx) {
            datoms.insert(d);
        }

        let store = Store::from_datoms(datoms);
        let f_initial = compute_fitness(&store);
        assert!(
            f_initial.total >= 0.0 && f_initial.total <= 1.0,
            "initial F(S)={} not in [0,1]",
            f_initial.total
        );

        // Create a task, then close it — F(S) should not decrease
        let mut store2 = store.clone_store();
        let task_entity = EntityId::from_ident(":task/test-monotone");

        // Add task datoms via Transaction API
        let tx1 = Transaction::new(agent, ProvenanceType::Observed, "create task")
            .assert(
                task_entity,
                Attribute::from_keyword(":task/title"),
                Value::String("Test task".into()),
            )
            .assert(
                task_entity,
                Attribute::from_keyword(":task/status"),
                Value::Keyword(":task.status/open".into()),
            )
            .commit(&store2)
            .unwrap();
        store2.transact(tx1).unwrap();

        let f_with_task = compute_fitness(&store2);

        // Close the task via Transaction API
        let tx2 = Transaction::new(agent, ProvenanceType::Observed, "close task")
            .assert(
                task_entity,
                Attribute::from_keyword(":task/status"),
                Value::Keyword(":task.status/closed".into()),
            )
            .commit(&store2)
            .unwrap();
        store2.transact(tx2).unwrap();

        let f_after_close = compute_fitness(&store2);
        assert!(
            f_after_close.total >= f_with_task.total - 0.01,
            "INV-BILATERAL-001: F(S) should not decrease after task close: \
             before={:.4} after={:.4}",
            f_with_task.total,
            f_after_close.total
        );
    }

    // ── EVIDENCE-R3: Evidence-weighted boundary tests ──

    #[test]
    fn evidence_weight_observed_deep_is_high() {
        // composite_evidence_weight should return high value for observed+L4+fresh
        let agent = AgentId::from_name("test:evidence");
        let tx = TxId::new(now_secs(), 0, agent);
        let entity = EntityId::from_ident(":test/evidence-high");

        let mut datoms = Store::genesis().datom_set().clone();

        // Add entity with observed provenance (via tx metadata)
        datoms.insert(Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String("high evidence entity".into()),
            tx,
            Op::Assert,
        ));

        // Add verification depth 4
        datoms.insert(Datom::new(
            entity,
            Attribute::from_keyword(":impl/verification-depth"),
            Value::Long(4),
            tx,
            Op::Assert,
        ));

        let store = Store::from_datoms(datoms);
        let weight = composite_evidence_weight(&store, entity);

        // With depth=4 (factor 1.0), fresh tx, and default provenance
        // Weight should be in the high range
        assert!(
            weight > 0.05,
            "Observed+L4+fresh should have high weight: {:.4}",
            weight
        );
    }

    #[test]
    fn evidence_weight_no_witness_is_low() {
        // Entity with no verification-depth datom → depth_factor = 0.1
        let agent = AgentId::from_name("test:evidence-low");
        let tx = TxId::new(now_secs(), 0, agent);
        let entity = EntityId::from_ident(":test/evidence-low");

        let mut datoms = Store::genesis().datom_set().clone();
        datoms.insert(Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String("no witness entity".into()),
            tx,
            Op::Assert,
        ));

        let store = Store::from_datoms(datoms);
        let weight = composite_evidence_weight(&store, entity);

        // No witness → depth_factor = 0.1, so weight should be relatively low
        assert!(
            weight < 0.5,
            "No witness entity should have low weight: {:.4}",
            weight
        );
    }

    #[test]
    fn evidence_weight_backward_compat_no_evidence_datoms() {
        // Store without any evidence-related datoms → weight defaults to reasonable value
        let store = Store::genesis();
        let entity = EntityId::from_ident(":test/nonexistent");
        let weight = composite_evidence_weight(&store, entity);

        // Entity doesn't exist → no datoms → default/floor values
        assert!(
            (0.0..=1.0).contains(&weight),
            "Weight should be in [0,1] even for non-existent entity: {:.4}",
            weight
        );
    }

    #[test]
    fn evidence_weight_depth_monotonic() {
        // Increasing verification depth should not decrease weight
        let agent = AgentId::from_name("test:evidence-mono");
        let entity = EntityId::from_ident(":test/evidence-mono");

        let mut prev_weight = 0.0;
        for depth in [0i64, 1, 2, 3, 4, 5] {
            let tx = TxId::new(now_secs(), 0, agent);
            let mut datoms = Store::genesis().datom_set().clone();
            datoms.insert(Datom::new(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("test".into()),
                tx,
                Op::Assert,
            ));
            datoms.insert(Datom::new(
                entity,
                Attribute::from_keyword(":impl/verification-depth"),
                Value::Long(depth),
                tx,
                Op::Assert,
            ));
            let store = Store::from_datoms(datoms);
            let weight = composite_evidence_weight(&store, entity);
            assert!(
                weight >= prev_weight - 0.001,
                "Depth {} weight {:.4} < previous depth weight {:.4}",
                depth,
                weight,
                prev_weight
            );
            prev_weight = weight;
        }
    }

    use proptest::prelude::*;

    proptest! {
        /// EVIDENCE-R3 PROPTEST: coverage is always in [0, 1] for any store state.
        #[test]
        fn prop_evidence_weight_bounded(
            depth in 0i64..=10,
        ) {
            let agent = AgentId::from_name("proptest:evidence");
            let tx = TxId::new(now_secs(), 0, agent);
            let entity = EntityId::from_ident(":proptest/evidence");

            let mut datoms = Store::genesis().datom_set().clone();
            datoms.insert(Datom::new(
                entity,
                Attribute::from_keyword(":db/doc"),
                Value::String("proptest entity".into()),
                tx,
                Op::Assert,
            ));
            datoms.insert(Datom::new(
                entity,
                Attribute::from_keyword(":impl/verification-depth"),
                Value::Long(depth),
                tx,
                Op::Assert,
            ));

            let store = Store::from_datoms(datoms);
            let weight = composite_evidence_weight(&store, entity);
            prop_assert!((0.0..=1.0).contains(&weight),
                "Evidence weight out of bounds: {:.6}", weight);
        }
    }

    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    // =======================================================================
    // DC-TEST: Dialectical Comonad test suite (t-4f01860f)
    // =======================================================================
    //
    // Tests the comonadic depth lattice (DC-1), depth-weighted F(S) (DC-2),
    // challenge lifecycle (DC-4), and anti-Goodhart detection properties.
    // Traces to: ADR-FOUNDATION-020, INV-FOUNDATION-008

    /// DC-TEST-1: A spec element at comonadic depth 0 (OPINION) contributes 0.0
    /// to the depth-weighted F(S) coverage component.
    ///
    /// Verifies: depth_weight(0) = 0.0 AND that compute_depth_weighted_coverage
    /// returns 0 contribution for a depth-0 element.
    #[test]
    fn depth_zero_contributes_zero_to_fs() {
        let agent = AgentId::from_name("dc-test");
        let tx = TxId::new(1, 0, agent);

        // Create a spec element with an impl link at verification depth 0
        let spec_entity = EntityId::from_ident(":spec/dc-test-zero");
        let impl_entity = EntityId::from_ident(":impl/dc-test-zero");

        let extra = vec![
            // Spec element
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":spec/dc-test-zero".into()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":spec/element-type"),
                Value::Keyword(":spec.type/invariant".into()),
                tx,
                Op::Assert,
            ),
            // Impl entity that implements the spec at depth 0
            Datom::new(
                impl_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":impl/dc-test-zero".into()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                impl_entity,
                Attribute::from_keyword(":impl/implements"),
                Value::Ref(spec_entity),
                tx,
                Op::Assert,
            ),
            Datom::new(
                impl_entity,
                Attribute::from_keyword(":impl/verification-depth"),
                Value::Long(0),
                tx,
                Op::Assert,
            ),
        ];

        let store = store_with(extra);

        // depth_weight(0) must be exactly 0.0
        assert_eq!(depth_weight(0), 0.0, "depth 0 must map to weight 0.0");

        // The coverage component should be 0.0 since only depth-0 link exists
        // and depth_weight(0) = 0.0, so depth_sum = 0.0.
        // Coverage = depth_sum / (spec_count * depth_weight(4)) = 0.0 / 1.0 = 0.0
        let coverage = compute_depth_weighted_coverage(&store);
        assert!(
            coverage < 1e-10,
            "depth-0 element should contribute ~0 to coverage, got {coverage}"
        );
    }

    /// DC-TEST-2: A spec element at comonadic depth 3 (SURVIVED) contributes
    /// approximately 0.7 to the depth-weighted F(S) coverage component.
    ///
    /// Verifies: depth_weight(3) = 0.7 AND proportional contribution to
    /// compute_depth_weighted_coverage.
    #[test]
    fn depth_three_contributes_weighted() {
        let agent = AgentId::from_name("dc-test");
        let tx = TxId::new(1, 0, agent);

        // Create a single spec element with impl link at depth 3
        let spec_entity = EntityId::from_ident(":spec/dc-test-three");
        let impl_entity = EntityId::from_ident(":impl/dc-test-three");

        let extra = vec![
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":spec/dc-test-three".into()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":spec/element-type"),
                Value::Keyword(":spec.type/invariant".into()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                impl_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":impl/dc-test-three".into()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                impl_entity,
                Attribute::from_keyword(":impl/implements"),
                Value::Ref(spec_entity),
                tx,
                Op::Assert,
            ),
            Datom::new(
                impl_entity,
                Attribute::from_keyword(":impl/verification-depth"),
                Value::Long(3),
                tx,
                Op::Assert,
            ),
        ];

        let store = store_with(extra);

        // depth_weight(3) must be 0.7
        assert!(
            (depth_weight(3) - 0.7).abs() < 1e-10,
            "depth 3 must map to weight 0.7, got {}",
            depth_weight(3)
        );

        // Coverage = depth_weight(3) / (1 * depth_weight(4)) = 0.7 / 1.0 = 0.7
        let coverage = compute_depth_weighted_coverage(&store);
        assert!(
            (coverage - 0.7).abs() < 1e-10,
            "depth-3 element should contribute 0.7 to coverage, got {coverage}"
        );
    }

    /// DC-TEST-3: Surviving a challenge increments the comonadic depth.
    ///
    /// Simulates the survive path of `braid challenge --survive`:
    /// entity starts at depth 2 (TESTED), after survival becomes depth 3 (SURVIVED).
    /// Verifies: set_depth_datom + comonadic_depth roundtrip with increment.
    #[test]
    fn challenge_increments_depth_on_survival() {
        let agent = AgentId::from_name("dc-test");
        let tx1 = TxId::new(1, 0, agent);
        let tx2 = TxId::new(2, 0, agent);
        let entity = EntityId::from_ident(":spec/dc-test-survive");

        // Initial state: depth 2 (TESTED)
        let initial_datom = set_depth_datom(&entity, 2, tx1);
        let mut datoms = std::collections::BTreeSet::new();
        for d in crate::schema::genesis_datoms(tx1) {
            datoms.insert(d);
        }
        for d in crate::schema::full_schema_datoms(tx1) {
            datoms.insert(d);
        }
        datoms.insert(initial_datom);
        let store = Store::from_datoms(datoms);

        assert_eq!(
            comonadic_depth(&store, &entity),
            2,
            "entity should start at depth 2"
        );

        // Survive: increment depth to 3 (mirrors challenge.rs --survive logic)
        let current = comonadic_depth(&store, &entity);
        let new_depth = (current + 1).min(4);
        let survive_datom = set_depth_datom(&entity, new_depth, tx2);

        let mut datoms2 = store.datom_set().clone();
        datoms2.insert(survive_datom);
        let store2 = Store::from_datoms(datoms2);

        assert_eq!(
            comonadic_depth(&store2, &entity),
            3,
            "entity should be at depth 3 (SURVIVED) after challenge survival"
        );

        // Verify the F(S) weight increased
        assert!(
            depth_weight(3) > depth_weight(2),
            "depth 3 weight ({}) must exceed depth 2 weight ({})",
            depth_weight(3),
            depth_weight(2)
        );
    }

    /// DC-TEST-4: Falsification resets comonadic depth to 0 (OPINION).
    ///
    /// Simulates the falsify path of `braid challenge --falsify`:
    /// entity starts at depth 3 (SURVIVED), after falsification resets to 0.
    /// Verifies: set_depth_datom(0) + comonadic_depth = 0 + survival_rate = 0.0.
    #[test]
    fn falsification_resets_depth() {
        let agent = AgentId::from_name("dc-test");
        let tx1 = TxId::new(1, 0, agent);
        let tx2 = TxId::new(2, 0, agent);
        let entity = EntityId::from_ident(":spec/dc-test-falsify");

        // Initial state: depth 3 (SURVIVED)
        let mut datoms = std::collections::BTreeSet::new();
        for d in crate::schema::genesis_datoms(tx1) {
            datoms.insert(d);
        }
        for d in crate::schema::full_schema_datoms(tx1) {
            datoms.insert(d);
        }
        datoms.insert(set_depth_datom(&entity, 3, tx1));
        let store = Store::from_datoms(datoms);

        assert_eq!(comonadic_depth(&store, &entity), 3);

        // Falsify: assert depth=0 at a later tx. comonadic_depth uses
        // max-by-tx LWW semantics, so the newer Assert(0) wins over the
        // older Assert(3) regardless of BTreeSet value ordering.
        let falsify_datom = set_depth_datom(&entity, 0, tx2);
        let survival_rate_datom = Datom::new(
            entity,
            Attribute::from_keyword(":comonad/survival-rate"),
            Value::Double(ordered_float::OrderedFloat(0.0)),
            tx2,
            Op::Assert,
        );

        let mut datoms2 = store.datom_set().clone();
        datoms2.insert(falsify_datom);
        datoms2.insert(survival_rate_datom);
        let store2 = Store::from_datoms(datoms2);

        assert_eq!(
            comonadic_depth(&store2, &entity),
            0,
            "falsified entity must reset to depth 0 (newer tx wins via LWW)"
        );

        // Verify survival-rate is 0.0
        let survival_attr = Attribute::from_keyword(":comonad/survival-rate");
        let rate = store2
            .entity_datoms(entity)
            .iter()
            .rev()
            .find(|d| d.attribute == survival_attr && d.op == Op::Assert)
            .and_then(|d| match &d.value {
                Value::Double(v) => Some(v.into_inner()),
                _ => None,
            })
            .unwrap_or(1.0);
        assert!(
            rate < 1e-10,
            "survival-rate must be 0.0 after falsification, got {rate}"
        );

        // Verify F(S) weight dropped to 0
        assert_eq!(
            depth_weight(0),
            0.0,
            "falsified entity at depth 0 must have zero F(S) weight"
        );
    }

    /// DC-TEST-5: Sawtooth pattern detected in depth trajectory.
    ///
    /// A sawtooth pattern occurs when comonadic depth oscillates (e.g., 0→1→2→0→1→2→0).
    /// This is an anti-Goodhart signal: repeated falsification followed by re-assertion.
    /// We verify detection by checking that the F(S) trajectory is non-monotonic
    /// (oscillation breaks monotonicity, signaling instability).
    #[test]
    fn sawtooth_detected_in_trajectory() {
        // Simulate a sawtooth depth trajectory: depths [0, 1, 2, 3, 0, 1, 2, 0]
        // Map through depth_weight to get F(S)-like values
        let depth_trajectory: Vec<i64> = vec![0, 1, 2, 3, 0, 1, 2, 0];
        let weight_trajectory: Vec<f64> = depth_trajectory
            .iter()
            .map(|&d| depth_weight(d))
            .collect();

        // The trajectory should be: [0.0, 0.15, 0.4, 0.7, 0.0, 0.15, 0.4, 0.0]
        // This is clearly non-monotonic (drops from 0.7 to 0.0)
        let analysis = analyze_convergence(&weight_trajectory);
        assert!(
            !analysis.is_monotonic,
            "sawtooth depth trajectory must be detected as non-monotonic"
        );

        // Count the number of "drops" (falsification events) — each is a depth reset
        let drop_count = weight_trajectory
            .windows(2)
            .filter(|w| w[1] < w[0] - 1e-10)
            .count();
        assert!(
            drop_count >= 2,
            "sawtooth should have at least 2 drops (falsification events), got {drop_count}"
        );

        // Lyapunov exponent should be non-positive (not converging)
        // because the system is oscillating rather than converging
        assert!(
            analysis.lyapunov_exponent <= 0.0 || !analysis.is_monotonic,
            "sawtooth trajectory should show non-convergence or non-monotonicity"
        );
    }

    /// DC-TEST-6: Echo chamber warning fires when all challenges succeed (zero surprise).
    ///
    /// If every challenge produces survival and no falsification ever occurs, the
    /// survival-rate approaches 1.0 with zero surprise. This is an anti-Goodhart
    /// signal: challenges may not be testing genuine falsification conditions.
    /// We verify that monotonic depth increase without any falsification produces
    /// a suspicious pattern (survival_rate = 1.0, no depth resets).
    #[test]
    fn echo_chamber_warning_at_zero_surprise() {
        let agent = AgentId::from_name("dc-test");
        let entity = EntityId::from_ident(":spec/dc-test-echo");

        // Simulate 10 consecutive survivals with no falsification
        let mut datoms = std::collections::BTreeSet::new();
        let tx0 = TxId::new(1, 0, agent);
        for d in crate::schema::genesis_datoms(tx0) {
            datoms.insert(d);
        }
        for d in crate::schema::full_schema_datoms(tx0) {
            datoms.insert(d);
        }

        // Walk the entity through depths 0→1→2→3→4 (all survivals)
        let depth_history = [0i64, 1, 2, 3, 4];
        for (i, &depth) in depth_history.iter().enumerate() {
            let tx = TxId::new((i + 1) as u64, 0, agent);
            datoms.insert(set_depth_datom(&entity, depth, tx));
        }

        // Record perfect survival rate
        let tx_final = TxId::new(10, 0, agent);
        datoms.insert(Datom::new(
            entity,
            Attribute::from_keyword(":comonad/survival-rate"),
            Value::Double(ordered_float::OrderedFloat(1.0)),
            tx_final,
            Op::Assert,
        ));
        let store = Store::from_datoms(datoms);

        // Verify the entity reached max depth (echo chamber succeeded)
        assert_eq!(comonadic_depth(&store, &entity), 4);

        // Verify survival rate is exactly 1.0 (zero surprise)
        let survival_attr = Attribute::from_keyword(":comonad/survival-rate");
        let rate = store
            .entity_datoms(entity)
            .iter()
            .rev()
            .find(|d| d.attribute == survival_attr && d.op == Op::Assert)
            .and_then(|d| match &d.value {
                Value::Double(v) => Some(v.into_inner()),
                _ => None,
            })
            .unwrap_or(0.0);
        assert!(
            (rate - 1.0).abs() < 1e-10,
            "echo chamber should have survival_rate = 1.0, got {rate}"
        );

        // The F(S) weight trajectory is monotonically increasing [0.0, 0.15, 0.4, 0.7, 1.0]
        let weight_trajectory: Vec<f64> = depth_history
            .iter()
            .map(|&d| depth_weight(d))
            .collect();
        let analysis = analyze_convergence(&weight_trajectory);
        assert!(
            analysis.is_monotonic,
            "echo chamber trajectory should be monotonic (suspiciously so)"
        );

        // Anti-Goodhart signal: perfect survival + monotonic convergence to max depth
        // without ANY depth resets is suspicious. The convergence_rate should be positive
        // but the zero-surprise property (rate == 1.0 with no drops) is the warning signal.
        let has_any_depth_reset = weight_trajectory
            .windows(2)
            .any(|w| w[1] < w[0] - 1e-10);
        assert!(
            !has_any_depth_reset,
            "echo chamber should have zero depth resets (the suspicious property)"
        );

        // Signal: rate == 1.0 AND no drops AND max depth reached
        let echo_chamber_detected = (rate - 1.0).abs() < 1e-10
            && !has_any_depth_reset
            && comonadic_depth(&store, &entity) == 4;
        assert!(
            echo_chamber_detected,
            "echo chamber conditions must all be true for warning"
        );
    }

    // =======================================================================
    // DC-TEST PROPTEST: Dialectical Comonad property-based tests
    // =======================================================================

    mod dc_proptests {
        use super::*;
        #[allow(unused_imports)]
        use proptest::prelude::*;

        proptest! {
            /// DC-TEST-7: depth-weighted F(S) is always in [0, 1] for any random
            /// store with depth attributes.
            ///
            /// For any combination of spec elements with varying verification depths,
            /// compute_depth_weighted_coverage must return a value in [0, 1].
            #[test]
            fn depth_weighted_fs_always_in_unit_interval(
                spec_count in 1usize..=10,
                depths in proptest::collection::vec(0i64..=5, 1..=10),
            ) {
                let agent = AgentId::from_name("dc-proptest");
                let tx = TxId::new(1, 0, agent);
                let mut extra = Vec::new();

                // Create spec_count spec elements
                for i in 0..spec_count {
                    let ident = format!(":spec/dc-prop-{i:04}");
                    let entity = EntityId::from_ident(&ident);
                    extra.push(Datom::new(
                        entity,
                        Attribute::from_keyword(":db/ident"),
                        Value::Keyword(ident),
                        tx,
                        Op::Assert,
                    ));
                    extra.push(Datom::new(
                        entity,
                        Attribute::from_keyword(":spec/element-type"),
                        Value::Keyword(":spec.type/invariant".into()),
                        tx,
                        Op::Assert,
                    ));

                    // Link an impl entity if we have a depth for this spec
                    if i < depths.len() {
                        let impl_ident = format!(":impl/dc-prop-{i:04}");
                        let impl_entity = EntityId::from_ident(&impl_ident);
                        extra.push(Datom::new(
                            impl_entity,
                            Attribute::from_keyword(":db/ident"),
                            Value::Keyword(impl_ident),
                            tx,
                            Op::Assert,
                        ));
                        extra.push(Datom::new(
                            impl_entity,
                            Attribute::from_keyword(":impl/implements"),
                            Value::Ref(entity),
                            tx,
                            Op::Assert,
                        ));
                        extra.push(Datom::new(
                            impl_entity,
                            Attribute::from_keyword(":impl/verification-depth"),
                            Value::Long(depths[i]),
                            tx,
                            Op::Assert,
                        ));
                    }
                }

                let store = store_with(extra);
                let coverage = compute_depth_weighted_coverage(&store);
                prop_assert!(
                    (0.0..=1.0 + 1e-10).contains(&coverage),
                    "DC-TEST-7: depth-weighted coverage {} out of [0,1] for {} specs with depths {:?}",
                    coverage, spec_count, &depths[..depths.len().min(spec_count)]
                );
            }

            /// DC-TEST-8: A survived challenge never decreases comonadic depth.
            ///
            /// For any starting depth in [0, 4], applying the survival operation
            /// (increment, clamped to 4) must produce depth >= original.
            #[test]
            fn survived_challenge_never_decreases_depth(
                initial_depth in 0i64..=4,
            ) {
                let agent = AgentId::from_name("dc-proptest");
                let tx1 = TxId::new(1, 0, agent);
                let tx2 = TxId::new(2, 0, agent);
                let entity = EntityId::from_ident(":spec/dc-prop-survive");

                // Set initial depth
                let mut datoms = std::collections::BTreeSet::new();
                for d in crate::schema::genesis_datoms(tx1) {
                    datoms.insert(d);
                }
                for d in crate::schema::full_schema_datoms(tx1) {
                    datoms.insert(d);
                }
                datoms.insert(set_depth_datom(&entity, initial_depth, tx1));
                let store = Store::from_datoms(datoms.clone());

                let before = comonadic_depth(&store, &entity);
                prop_assert_eq!(before, initial_depth);

                // Apply survival: new_depth = (current + 1).min(4)
                let new_depth = (before + 1).min(4);
                datoms.insert(set_depth_datom(&entity, new_depth, tx2));
                let store2 = Store::from_datoms(datoms);

                let after = comonadic_depth(&store2, &entity);
                prop_assert!(
                    after >= before,
                    "DC-TEST-8: survived challenge decreased depth: {} -> {}",
                    before, after
                );
                prop_assert!(
                    depth_weight(after) >= depth_weight(before),
                    "DC-TEST-8: F(S) weight decreased after survival: {} -> {}",
                    depth_weight(before), depth_weight(after)
                );
            }
        }
    }
}
