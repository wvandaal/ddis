# 01 — Information Geometry of the Coherence Manifold

> **Status**: EXPLORATION
> **Date**: 2026-03-09
> **Traces to**: SEED.md §5 (harvest/seed lifecycle), §7 (self-improvement loop),
>   §8 (interface principles)
> **Builds on**: 00-density-matrix-coherence.md (density matrix construction),
>   spec/06-seed.md (seed assembly), spec/12-guidance.md (guidance injection),
>   spec/10-bilateral.md (bilateral loop convergence)
>
> **Thesis**: The space of coherence density matrices is a Riemannian manifold with a
> natural metric (Bures). Geodesics on this manifold give optimal transaction paths to
> coherence. The geodesic-optimal seed construction replaces heuristic scoring with a
> principled variational optimization that has a provable (1-1/e) approximation guarantee.

---

## 1. The Bures Metric on the Coherence Manifold

### 1.1 The Space of Coherence States

**Definition 1.1 (Coherence manifold).** The space of all possible coherence states for
n agents is:

```
𝒟ₙ = {ρ ∈ ℝⁿˣⁿ : ρ ≥ 0, Tr(ρ) = 1, ρ = ρᵀ}
```

This is a smooth manifold of dimension n(n+1)/2 - 1. For:
- n = 3 (ISP triangle): dim = 5
- n = 5: dim = 14
- n = 10: dim = 54

The interior 𝒟ₙ⁺ (full-rank density matrices) is an open convex set in ℝⁿˣⁿ.
The boundary ∂𝒟ₙ consists of rank-deficient density matrices, including the pure
states (rank 1) that are the coherence targets.

### 1.2 The Bures Metric

**Definition 1.2 (Bures distance).** For ρ, σ ∈ 𝒟ₙ:

```
d_B(ρ, σ) = √(2(1 - F(ρ, σ)))

where F(ρ, σ) = (Tr(√(√ρ · σ · √ρ)))²   (Uhlmann fidelity)
```

**Properties**:
1. d_B is a true metric (positive, symmetric, triangle inequality)
2. d_B(ρ, σ) = 0 ⟺ ρ = σ
3. d_B(ρ, σ) ≤ √2 (maximum distance between any two density matrices)
4. d_B is unitarily invariant: d_B(UρU†, UσU†) = d_B(ρ, σ) (relabeling agents is a symmetry)
5. d_B is monotone under CPTP maps: d_B(Φ(ρ), Φ(σ)) ≤ d_B(ρ, σ) (merges never increase distance)

Property 5 is the most important for DDIS: **any merge operation (a CPTP map) can only
bring states closer together, never push them apart.** This is the density-matrix
generalization of INV-TRILATERAL-004 (convergence monotonicity).

### 1.3 Simplification for Pure Target

The coherence target is a pure state ρ_target = |ψ⟩⟨ψ| where |ψ⟩ = (1/√n, ..., 1/√n).
For a pure target, the fidelity simplifies:

```
F(ρ, |ψ⟩⟨ψ|) = ⟨ψ|ρ|ψ⟩

d_B(ρ, |ψ⟩⟨ψ|) = √(2(1 - ⟨ψ|ρ|ψ⟩))
                  = √(2(1 - (1/n)Σᵢⱼ ρᵢⱼ))
```

This is computationally trivial: sum all entries of ρ, divide by n, subtract from 1,
take the square root. No matrix square roots needed. O(n²) — microseconds for n ≤ 10.

### 1.4 Infinitesimal Metric (The Riemannian Structure)

The Bures metric at a point ρ ∈ 𝒟ₙ⁺ is defined by the SLD inner product:

```
⟨X, Y⟩_ρ = (1/2) Tr(X · L_Y + L_Y · X)

where L_Y is the symmetric logarithmic derivative satisfying:
  Y = (L_Y · ρ + ρ · L_Y) / 2
```

For infinitesimal perturbations dρ, the line element is:

```
ds² = (1/2) Σᵢⱼ |dρᵢⱼ|² / (λᵢ + λⱼ)

where λᵢ are eigenvalues of ρ (in the eigenbasis)
```

**Observation**: The metric diverges as eigenvalues approach 0 (near the boundary ∂𝒟ₙ).
This means the coherence manifold is "increasingly steep" near pure states — the last
few percent of coherence improvement cost disproportionately more effort. This matches
the empirical observation that achieving full coherence is harder than achieving partial
coherence.

---

## 2. Geodesics: Optimal Transaction Paths

### 2.1 The Geodesic Equation

The geodesic on 𝒟ₙ from ρ₀ to ρ₁ is:

```
ρ(t) = (A(t))² / Tr((A(t))²)

where A(t) = (1-t) · √ρ₀ + t · √ρ₀ · (√ρ₀ · ρ₁ · √ρ₀)^{-1/2} · √ρ₁

for t ∈ [0, 1]
```

For a pure target ρ₁ = |ψ⟩⟨ψ|, this simplifies considerably.

### 2.2 Discrete Geodesic Pursuit

Transactions are discrete — the store changes in atomic steps, not continuously.
The practical algorithm is **geodesic pursuit**: at each step, choose the transaction
that moves closest to the target along the geodesic.

```
GEODESIC_PURSUIT(ρ_current, ρ_target, candidates: Vec<Transaction>) → Transaction:

  best_tx = None
  best_distance = d_B(ρ_current, ρ_target)

  for τ in candidates:
    ρ' = apply_transaction(ρ_current, τ)   // update density matrix after τ
    d' = d_B(ρ', ρ_target)
    if d' < best_distance:
      best_distance = d'
      best_tx = Some(τ)

  return best_tx
```

The `apply_transaction` function computes the density matrix update:
- Adding a link (e.g., :spec/traces-to): increases agreement for the linked entities →
  increases off-diagonal entries of ρ → moves toward purity
- Adding new content (e.g., new spec element): adds new entity-attribute pairs to Ω →
  changes the dimension of the effective coherence space
- Resolving a conflict: directly changes agreement_a(v₁, v₂) → directly modifies ρ

### 2.3 Convergence Guarantee

**Theorem 2.2 (Geodesic pursuit convergence).** If at least one transaction reduces the
Bures distance, geodesic pursuit converges to the target in at most
⌈d_B(ρ₀, ρ_target)² / δ_min²⌉ steps, where δ_min is the minimum per-step distance reduction.

**Proof**: Each step reduces d_B by at least δ_min. The initial distance is at most √2.
The number of steps is bounded by (√2)² / δ_min² = 2/δ_min². ∎

This is a worst-case bound. In practice, well-chosen transactions reduce distance by
much more than δ_min, giving faster convergence.

---

## 3. Optimal Seed Construction

### 3.1 The Seed Optimization Problem

Current seed construction (spec/06-seed.md §6.1):

```
SEED(S, task, k*) = ASSEMBLE(QUERY(ASSOCIATE(S, task)), k*)
```

with heuristic priority: score(e) = 0.5·relevance + 0.3·significance + 0.2·recency.

The information-geometric formulation:

```
OPTIMAL_SEED(S, task, k*) = argmin_{σ ⊆ S, |σ| ≤ k*} d_B(ρ(σ), ρ_target(task))
```

where ρ(σ) is the coherence density matrix achievable from seed σ, and ρ_target(task)
is the coherence state needed for the task.

### 3.2 Submodular Optimization

**Theorem 3.1 (Submodularity of fidelity).** The function f(σ) = ⟨ψ|ρ(σ)|ψ⟩ (fidelity
with the pure target) is a monotone submodular function of the seed set σ.

**Proof sketch**:
- **Monotonicity**: Adding a datom to the seed can only increase or maintain agreement
  (the new datom either resolves an uncertainty or is redundant). So f(σ ∪ {d}) ≥ f(σ).
- **Submodularity** (diminishing returns): The marginal gain of adding datom d to a
  larger set σ₂ ⊇ σ₁ is less than or equal to adding d to σ₁. This holds because
  a larger seed already has more context — additional datoms provide less incremental
  coherence improvement:
  f(σ₁ ∪ {d}) - f(σ₁) ≥ f(σ₂ ∪ {d}) - f(σ₂)  ∎

**Corollary (Nemhauser et al., 1978)**: The greedy algorithm for maximizing f(σ) subject
to |σ| ≤ k* achieves at least (1 - 1/e) ≈ 0.632 of the optimal fidelity.

### 3.3 Greedy Geodesic Seed Assembly

```
GREEDY_SEED(S, task, k*) → Seed:
  ρ_target = coherence_target(task)    // pure state for task's relevant entities
  |ψ⟩ = uniform superposition vector
  seed = ∅

  while |seed| < k*:
    best_datom = argmax_{d ∈ S \ seed} ⟨ψ|ρ(seed ∪ {d})|ψ⟩
    seed = seed ∪ {best_datom}

  return seed
```

**Key insight**: `⟨ψ|ρ(seed ∪ {d})|ψ⟩ = (1/n) Σᵢⱼ ρ(seed ∪ {d})ᵢⱼ`. Adding datom d
changes ρ by a rank-1 update (only one entity-attribute pair is affected). So the
marginal fidelity gain of adding d is:

```
Δf(d) = (1/nW) · w(a_d) · Σᵢ≠ⱼ Δagreement_ij(e_d, a_d)
```

where Δagreement_ij is the change in agreement between views i and j on (e_d, a_d)
when datom d is added to the seed. This is O(n²) per candidate — fast enough for
the greedy loop.

### 3.4 Connection to Current Score Function

The current heuristic score(e) = 0.5·relevance + 0.3·significance + 0.2·recency relates
to the geodesic formulation:

- **Relevance** correlates with Δf(d): a relevant datom increases fidelity for the task's
  target state. The geodesic formulation computes this EXACTLY instead of heuristically.
- **Significance** correlates with w(a_d): high-significance attributes have high resolution
  weights, contributing more to the weighted fidelity.
- **Recency** has no direct analog in the geodesic formulation — it's a process heuristic,
  not a coherence property. If recency matters, it can be incorporated as a multiplicative
  modifier on Δf(d).

The geodesic formulation SUBSUMES the heuristic score function while providing a provable
approximation guarantee (1 - 1/e) that the heuristic lacks.

---

## 4. Curvature and Problem Difficulty

### 4.1 Sectional Curvature

The **Ricci scalar** R at a point ρ ∈ 𝒟ₙ measures the average curvature:

```
R(ρ) = -n(n²-1) / 4 + correction_terms(eigenvalues)
```

For the Bures metric, the manifold has **non-positive Ricci curvature** near the
maximally mixed state (ρ = I/n) and **mixed curvature** near pure states.

**Interpretation for DDIS**:

| Curvature Region | Store State | Operational Meaning |
|---|---|---|
| Slightly negative | Near maximally mixed (many disagreements) | Many paths to improvement; any direction helps |
| Flat (κ ≈ 0) | Moderate coherence | Linear effort-to-improvement ratio |
| Mixed (pos + neg) | Near a pure state (few remaining issues) | Some fixes help, others can backfire |

### 4.2 Curvature-Aware Guidance

The curvature at the current state informs the guidance system:

```
guidance_from_curvature(ρ, task):
  κ = sectional_curvature(ρ, geodesic_direction(ρ, ρ_target))

  if κ < -threshold:
    // Flat/negative: wide basin, many good paths
    return "Multiple improvement paths available. Prioritize by budget."
  
  if κ > threshold:
    // Positive: narrow basin, wrong moves penalized
    return "⚠ Sensitive region. Verify each change reduces d_B before committing."
  
  if κ mixed (saddle point):
    // Some directions good, others bad
    return "⚠ Saddle point. Use FULL_CYCLE review. Geodesic pursuit recommended."
```

This connects to the bilateral loop strategy:
- AUTO_CYCLE (INV-BILATERAL-002) is appropriate when curvature is non-positive (safe basin)
- FULL_CYCLE (human-gated) is appropriate when curvature is mixed (sensitive region)

### 4.3 The Metric Divergence Near Purity

From §1.4, the Bures metric diverges as eigenvalues → 0:

```
ds² ~ |dρ|² / λ_min    as λ_min → 0
```

This means: **the last few bits of coherence cost exponentially more effort.** Going from
S = 0.5 to S = 0.3 is "easy" (large λ_min, small metric). Going from S = 0.01 to S = 0.0
is "hard" (small λ_min, large metric).

This has a practical implication for the specification: defining "coherent enough" at a
threshold S < ε (rather than demanding S = 0 exactly) avoids the divergent cost of perfect
coherence. The threshold ε can be stored as a datom (schema-as-data, C3):

```
(config:coherence :config/entropy-threshold 0.01 tx:config assert)
```

---

## 5. Merge Convergence Rate

### 5.1 Merges as Completely Positive Maps

Each merge operation M : 𝒟ₙ → 𝒟ₙ is a **completely positive trace-preserving (CPTP)
map** — the quantum-information-theoretic model for physical operations on density
matrices.

For a pairwise merge between agents αᵢ and αⱼ (CRDT set union):

```
M_{ij}(ρ) = ρ + Δ_{ij}

where Δ_{ij} is the rank-1 update from the merge resolving disagreements
on the (αᵢ, αⱼ) edge.
```

### 5.2 Spectral Gap and Convergence Rate

The **merge superoperator** for topology T is the composition of all per-edge merge maps:

```
Φ_T = M_{e₁} ∘ M_{e₂} ∘ ... ∘ M_{eₖ}    (one merge round)
```

The spectral gap μ_T of Φ_T determines the convergence rate:

```
S(Φ_T^k(ρ)) ≤ e^{-μ_T · k} · S(ρ)    (exponential convergence)
```

**For topology T**:
- Mesh (all pairs merge): μ_T is large (fast convergence)
- Star (all merge with hub): μ_T is medium (hub is bottleneck)
- Pipeline: μ_T is small (information propagates slowly, linearly)

This gives a principled F(T): **F(T) = 1 - e^{-μ_T}** (normalized convergence rate).

### 5.3 Connection to Heat Equation

The merge dynamics ρ(k+1) = Φ_T(ρ(k)) is the discrete analogue of the **heat equation
on the coherence manifold**:

```
∂ρ/∂t = -[L_T, ρ]    (Lindblad equation)

where L_T is the Lindbladian generator determined by the topology
```

The equilibrium (steady state) is the pure state ρ = |ψ⟩⟨ψ| where all agents agree.
The approach to equilibrium is exponential with rate μ_T. This connects directly to
the heat equation interpretation in sheaf-coherence/01-hodge-theory.md §3.

---

## 6. CLI Integration

### 6.1 New Commands

```bash
# Coherence report with density matrix diagnostics
braid coherence --density
# Output:
#   Density matrix ρ (3×3 for ISP):
#     [0.333  0.301  0.125]
#     [0.301  0.333  0.312]
#     [0.125  0.312  0.333]
#   Eigenvalues: [0.721, 0.245, 0.034]
#   Von Neumann entropy: S = 0.642
#   Bures distance to coherence: d_B = 0.391
#   Normalized coherence: 62.8%
#   Quadrant: GapsAndCycles (Φ=12, β₁=1)

# Geodesic pursuit recommendation
braid coherence --next 5
# Output:
#   Top 5 transactions by Bures distance reduction:
#   1. Link INV-STORE-005 → store::validate()    Δd_B = -0.082
#   2. Resolve :spec/statement for INV-QUERY-003  Δd_B = -0.061
#   3. Add :spec/traces-to SEED §6 Axiom 3       Δd_B = -0.047
#   4. Link NEG-MUTATION-001 → store::retract()   Δd_B = -0.039
#   5. Update :impl/behavior for query::execute() Δd_B = -0.031

# Optimal seed construction
braid seed --optimal --task "implement store::transact" --budget 2000
# Output:
#   Geodesic-optimal seed (fidelity: 0.89, budget: 1847/2000 tokens):
#   π₀: INV-STORE-001, INV-STORE-002, INV-STORE-003 (core store invariants)
#   π₁: SEED §4 (datom algebra summary)
#   π₂: INV-QUERY-001..003 (query dependency summaries)
#   π₃: 12 entity pointers (related schema, resolution modes)
#   Approximation guarantee: ≥ 63.2% of optimal fidelity

# Temperature history
braid coherence --temperature --since tx:100
# Output: ASCII plot of S(t) over transaction history
```

### 6.2 JSON Output for Programmatic Use

```bash
braid coherence --density --json
```

```json
{
  "density_matrix": [[0.333, 0.301, 0.125], [0.301, 0.333, 0.312], [0.125, 0.312, 0.333]],
  "eigenvalues": [0.721, 0.245, 0.034],
  "entropy": 0.642,
  "bures_distance": 0.391,
  "normalized_coherence": 0.628,
  "entropy_decomposition": {
    "intent_spec": 0.087,
    "spec_impl": 0.312,
    "intent_impl": 0.198,
    "cross_boundary": 0.045
  },
  "phi": 12,
  "beta_1": 1,
  "quadrant": "GapsAndCycles"
}
```

---

## 7. Open Questions

### OQ-1: Curvature Computation Cost (Confidence: 0.8)
The full Riemann curvature tensor for 𝒟ₙ is O(n⁸) to compute. For n ≤ 10, this is
10⁸ operations — potentially slow. The sectional curvature in the geodesic direction
is cheaper (O(n⁴)), but still non-trivial.

**Mitigation**: For Stage 0 (n=3), all curvature computations are analytically solvable.
For Stage 2+ (n ≤ 10), use the sectional curvature approximation (O(n⁴) ≈ 10⁴ — fast).
Full curvature tensor only if needed for advanced diagnostics.

### OQ-2: Seed Optimization with Non-Uniform Budgets (Confidence: 0.7)
The submodular maximization assumes a cardinality constraint (|σ| ≤ k*). In practice,
the budget is in TOKENS, not datom count — different datoms have different token costs.
This changes the constraint to a knapsack-type constraint, for which the greedy algorithm
gives a (1 - 1/e)/2 ≈ 0.316 approximation instead of (1 - 1/e) ≈ 0.632.

**What would resolve it**: Using the cost-effective greedy variant (sort by Δf(d)/cost(d)
instead of Δf(d) alone). This restores the (1-1/e) guarantee for knapsack constraints
under certain regularity conditions (Khuller, Moss & Naor, 1999).

### OQ-3: CPTP Model for Non-Merge Transactions (Confidence: 0.6)
Merge operations are naturally CPTP (they bring agent states closer). But other
transactions (adding new content, retracting facts) are not obviously CPTP — they may
change the dimension of the effective state space. The convergence rate analysis (§5)
assumes all operations are CPTP.

**What would resolve it**: Extending the CPTP model to handle dimension changes
(new entity-attribute pairs entering Ω). This may require working in a direct limit
of finite-dimensional manifolds rather than a fixed-dimension manifold.

---

*The information geometry of coherence gives DDIS three capabilities no other specification
framework has: (1) a principled distance metric between coherence states (Bures), (2)
optimal transaction selection via geodesic pursuit, and (3) provably near-optimal seed
construction via submodular maximization on the Bures fidelity. The mathematics is
elementary (eigendecomposition of small symmetric matrices), the computation is fast
(microseconds), and the results are actionable (ranked recommendations with distance
improvements).*