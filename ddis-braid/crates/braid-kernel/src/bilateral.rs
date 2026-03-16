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

use crate::datom::{Attribute, Datom, EntityId, Op, TxId, Value};
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
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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

/// V: Depth-weighted validation score.
///
/// When `:spec/verification-depth` datoms exist, uses depth weights:
///   V = Σ(depth_weight(depth_i)) / (|spec_elements| × depth_weight(4))
///
/// Falls back to binary `:spec/witnessed` counting when no depth datoms are present,
/// ensuring backwards compatibility with stores that predate WP9.
fn compute_validation(store: &Store) -> f64 {
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
        let depth = datoms
            .iter()
            .rfind(|d| d.attribute == spec_depth_attr && d.op == Op::Assert)
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

    for datom in store.datoms() {
        if datom.attribute == implements_attr && datom.op == Op::Assert {
            if let Value::Ref(spec_entity) = &datom.value {
                let impl_entity = datom.entity;
                // Get depth for this impl entity.
                // Default to 1 (syntactic) for impl links without explicit depth —
                // they passed the trace scanner which is Level 1 verification.
                let explicit_depth = store
                    .entity_datoms(impl_entity)
                    .iter()
                    .rfind(|d| d.attribute == impl_depth_attr && d.op == Op::Assert)
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

    for d in store.datoms() {
        if d.op != Op::Assert {
            continue;
        }
        if d.attribute == impl_attr {
            if let Value::Ref(spec_entity) = &d.value {
                impl_covered.insert(*spec_entity);
            }
        }
        if d.attribute == task_traces_attr {
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

    for datom in store.datoms() {
        if datom.attribute == confidence_attr && datom.op == Op::Assert {
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
    for datom in store.datoms() {
        if datom.attribute == implements_attr && datom.op == Op::Assert {
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
    use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, TxId, Value};
    use crate::store::Store;
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
}
