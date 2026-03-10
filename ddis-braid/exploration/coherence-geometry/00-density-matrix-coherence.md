# 00 — Density Matrix Coherence

> **Status**: EXPLORATION
> **Date**: 2026-03-09
> **Traces to**: SEED.md §1 (coherence verification), §2 (divergence problem),
>   §4 (datom algebra), §6 (reconciliation mechanisms)
> **Builds on**: exploration/sheaf-coherence/00-sheaf-cohomology-for-coherence.md (Čech construction),
>   exploration/sheaf-coherence/01-hodge-theory.md (spectral connection),
>   spec/04-resolution.md (per-attribute resolution modes),
>   spec/18-trilateral.md (Φ metric, (Φ, β₁) duality)
>
> **Thesis**: The coherence state of a DDIS store is a density matrix ρ — a positive
> semi-definite, unit-trace matrix encoding the full agreement structure across perspectives.
> The von Neumann entropy S(ρ) is the universal coherence metric from which all existing
> metrics (Φ, β₁, F(S)) derive as projections. The density matrix unifies divergence counting,
> topological cycle detection, and fitness evaluation under a single algebraic object.

---

## 1. First Principles: What IS Coherence?

### 1.1 The Epistemological Foundation

A DDIS store holds facts asserted by multiple perspectives (agents, abstraction levels,
time points). Coherence means: perspectives that SHOULD agree DO agree. But "agree" is
not a monolithic concept — it depends on what the system will DO with the agreement.

Two values "agree" to the extent that acting on one versus acting on the other would
produce the same outcome under the system's resolution semantics. This is not string
comparison — it is **behavioral equivalence under resolution**.

DDIS already formalizes this: the resolution mode system (spec/04-resolution.md) declares,
per attribute, how conflicting values converge. The resolution mode IS the definition of
agreement. Using any other definition would create a meta-incoherence: the coherence metric
would disagree with the resolution system about what "agreement" means.

### 1.2 Why Scalars Are Insufficient

Φ (INV-TRILATERAL-002) counts unlinked boundaries. It says "47 gaps exist" but not:
- Which pairs of perspectives contribute most to the incoherence
- Whether the gaps are independent (fixable in any order) or correlated (requiring coordination)
- How much each gap matters (a lattice-resolved disagreement vs. a multi-value divergence)

β₁ (INV-QUERY-024) detects cyclic incoherence. It says "3 cycles exist" but not:
- The degree of disagreement around each cycle
- The resolution difficulty (lattice cycles auto-converge; multi-value cycles require deliberation)
- The correlation structure between cycles

What's needed is a mathematical object that encodes the **full correlation structure** of
agreement across all perspectives, from which any scalar summary can be derived.

### 1.3 The Density Matrix as the Right Abstraction

In physics, the density matrix is the unique mathematical object that:
1. Encodes pure states (perfect correlation) as rank-1 matrices
2. Encodes mixed states (partial correlation) as higher-rank matrices
3. Encodes no correlation as the maximally mixed state (I/n)
4. Admits a natural entropy (von Neumann) measuring "how mixed" the state is
5. Lives on a manifold with natural geometry (Bures metric)

These five properties map exactly to the DDIS coherence problem:
1. Full coherence = pure state (all perspectives agree)
2. Partial coherence = mixed state (some agree, some don't)
3. No coherence = maximally mixed (no perspective agrees with any other)
4. Von Neumann entropy = universal incoherence measure
5. Bures geometry = optimal paths to coherence (see doc 01)

---

## 2. The Agreement Matrix Construction

### 2.1 Resolution-Derived Agreement

**Definition 2.1 (Agreement function).** For attribute a with resolution mode R_a and
value domain V_a, the agreement function is:

```
agreement_a : V_a × V_a → [0, 1]

agreement_a(v₁, v₂) = match R_a {
    LWW      => δ(v₁, v₂)                             // Kronecker delta
    Lattice  => 1 - d_L(v₁, v₂) / d_L_max             // normalized lattice distance
    Multi    => |v₁ ∩ v₂| / |v₁ ∪ v₂|                 // Jaccard similarity
}
```

where:
- δ(v₁, v₂) = 1 if v₁ = v₂, 0 otherwise
- d_L(v₁, v₂) = length of shortest path in Hasse diagram of lattice L between v₁ and v₂
- d_L_max = diameter of lattice L

**Theorem 2.1 (Agreement-resolution consistency).** For all resolution modes:
```
agreement_a(v₁, v₂) = 1  ⟺  resolve_a(v₁, v₂) = v₁ = v₂
```
That is, full agreement iff resolution is a no-op.

**Proof sketch**: For LWW: δ(v₁, v₂) = 1 iff v₁ = v₂ iff LWW resolves to either (same value).
For Lattice: d_L = 0 iff v₁ = v₂ iff join(v₁, v₂) = v₁ = v₂.
For Multi: Jaccard = 1 iff v₁ = v₂ (as sets) iff union = either set. ∎

This theorem is the formal justification for using resolution modes as agreement functions:
"agree" and "resolve to the same value" are provably equivalent.

### 2.2 Per-Attribute Coherence Matrix

**Definition 2.2 (Agreement matrix).** For agents V = {α₁, ..., αₙ} and entity-attribute
pair (e, a), the agreement matrix is:

```
A(e,a) ∈ ℝⁿˣⁿ

A(e,a)ᵢⱼ = agreement_a(view_αᵢ(e, a), view_αⱼ(e, a))
```

where view_αᵢ(e, a) = resolve(F_αᵢ, e, a) is agent αᵢ's resolved value (INV-STORE-012).

**Properties**:
- Symmetric: A(e,a)ᵢⱼ = A(e,a)ⱼᵢ (agreement is commutative)
- Unit diagonal: A(e,a)ᵢᵢ = 1 (self-agreement)
- Positive semi-definite: A is a Gram matrix
- Tr(A) = n

**Definition 2.3 (Per-attribute coherence density matrix).**
```
ρ(e,a) = A(e,a) / n
```

This satisfies the density matrix axioms: ρ ≥ 0, Tr(ρ) = 1, ρ = ρᵀ.

**Boundary cases**:
- All agents agree: A = 1·1ᵀ, ρ = |ψ⟩⟨ψ| where |ψ⟩ = (1/√n,...,1/√n). Pure state. S = 0.
- All agents disagree (pairwise): A = I, ρ = I/n. Maximally mixed. S = log(n).
- k agents agree, rest disagree: ρ has rank (n - k + 1). Partially mixed.

### 2.3 Total Coherence Matrix

**Definition 2.4 (Resolution-weighted total coherence matrix).** Given entity-attribute
universe Ω = {(e₁,a₁), ..., (e_m,a_m)} and resolution importance weights w(a):

```
ρ̄_w = (1/W) Σ_{(e,a) ∈ Ω} w(a) · ρ(e,a)

where W = Σ_{(e,a)} w(a) is the normalizer (ensures Tr(ρ̄_w) = 1)
```

The resolution weights from INV-QUERY-023:
```
w(a) = match resolution_mode(a) {
    Lattice  => 0.1    // auto-converges — low concern
    LWW      => 0.5    // converges after merge — medium concern
    Multi    => 1.0    // requires deliberation — high concern
}
```

These weights serve a DIFFERENT purpose than the agreement function:
- agreement_a measures the DEGREE of disagreement (how far apart are the values?)
- w(a) measures the IMPORTANCE of disagreement (how much does it matter?)

The total coherence matrix ρ̄_w combines both: it's the importance-weighted average of
degree-of-agreement matrices.

### 2.4 The ISP Triangle: Stage 0 Specialization

For Stage 0 (single agent), the "agents" are the three LIVE views: LIVE_I, LIVE_S, LIVE_P.
The density matrix is 3×3:

```
ρ_ISP = (1/3W) Σ_{(e,a)} w(a) ·
  ┌                                              ┐
  │  1           agree_IS(e,a)   agree_IP(e,a)   │
  │  agree_IS(e,a)   1           agree_SP(e,a)   │
  │  agree_IP(e,a)   agree_SP(e,a)   1           │
  └                                              ┘
```

where agree_XY(e,a) = agreement_a(LIVE_X(e,a), LIVE_Y(e,a)) for X,Y ∈ {I,S,P}.

For a 3×3 matrix, the eigenvalues are analytically solvable (cubic formula). The von
Neumann entropy has a closed-form expression in terms of three eigenvalues.

---

## 3. Von Neumann Entropy as Universal Coherence Metric

### 3.1 Definition and Properties

**Definition 3.1 (Von Neumann entropy).** The coherence entropy of store S is:

```
S(S) = S(ρ̄_w) = -Tr(ρ̄_w · log(ρ̄_w)) = -Σᵢ λᵢ log(λᵢ)
```

where λᵢ are the eigenvalues of ρ̄_w and 0 · log(0) := 0 by convention.

**Properties**:
1. **Non-negativity**: S ≥ 0 (eigenvalues ∈ [0,1], so -λ log λ ≥ 0)
2. **Purity characterization**: S = 0 ⟺ ρ̄_w is pure ⟺ all agents agree on all weighted attributes
3. **Maximum**: S ≤ log(n) with equality iff ρ̄_w = I/n (maximum disagreement)
4. **Concavity**: S is concave in ρ (important for optimization — no local minima)
5. **Continuity**: S is continuous in ρ (small changes in agreement → small changes in entropy)
6. **Subadditivity**: S(ρ_AB) ≤ S(ρ_A) + S(ρ_B) (total entropy ≤ sum of parts)

Property 6 is crucial: it means correlated disagreements (cycles) contribute MORE
entropy than independent disagreements. This is how the density matrix captures
what β₁ detects — cyclic incoherence increases entropy beyond the sum of individual
disagreements.

### 3.2 Normalized Coherence Score

For human-readable output, normalize to [0, 1]:

```
coherence(S) = 1 - S(S) / log(n)

coherence = 1.0  ⟺  fully coherent (pure state)
coherence = 0.0  ⟺  maximally incoherent (no agent agrees with any other)
```

This replaces the ad hoc F(S) = 0.18V + 0.18C + ... with a single principled formula.

### 3.3 Entropy Decomposition

The von Neumann entropy decomposes by **source of incoherence**:

```
S(ρ̄_w) = S_within + S_between

where:
  S_within = (1/W) Σ_{(e,a)} w(a) · S(ρ(e,a))    (per-attribute incoherence)
  S_between = S(ρ̄_w) - S_within                      (cross-attribute correlation)
```

By subadditivity, S_between ≥ 0. The magnitude of S_between relative to S_within tells
you whether incoherence is independent (S_between ≈ 0, fixable in any order) or correlated
(S_between >> 0, requires coordinated resolution).

For the ISP triangle, this further decomposes by boundary:

```
S_IS = entropy contribution from Intent-Spec disagreements
S_SP = entropy contribution from Spec-Impl disagreements
S_IP = entropy contribution from Intent-Impl disagreements (bypass detection)
```

This gives a diagnostic: "Your coherence entropy is 0.42, of which 0.35 comes from the
Spec↔Impl boundary and 0.07 from cross-boundary correlation."

---

## 4. Unification of Existing Metrics

### 4.1 Φ as Rank Deficiency Count

**Theorem 4.1.** The divergence metric Φ (INV-TRILATERAL-002) is the count of non-pure
per-attribute density matrices:

```
Φ(S) = |{(e, a) ∈ Ω_boundaries : rank(ρ(e,a)) > 1}|
```

where Ω_boundaries restricts to entity-attribute pairs relevant to cross-boundary links.

**Proof**: Φ counts unlinked boundaries — entities without :spec/traces-to or :spec/implements
links. An unlinked entity has undefined value in one view (LIVE_I, LIVE_S, or LIVE_P) while
having a defined value in another. The agreement between "defined" and "undefined" is 0,
making ρ(e,a) non-pure (rank > 1). Conversely, a linked entity with consistent values
across all views has agreement = 1, making ρ(e,a) pure (rank 1). ∎

**What S adds beyond Φ**: S(ρ(e,a)) ∈ [0, log(n)] gives the DEGREE of disagreement, not
just its existence. A lattice-resolved attribute near the join has high agreement (low entropy)
even if not identical. Φ conflates "close to agreeing" with "completely disagreeing."

### 4.2 β₁ as Entanglement Witness

**Theorem 4.2 (Entanglement-cycle correspondence).** For the coherence graph G = (V, E)
with per-edge agreement matrices:

```
β₁(G) > 0  ⟺  ∃ cycle C in G such that S_between(C) > 0
```

where S_between(C) is the cross-edge entropy correlation around the cycle.

**Interpretation**: β₁ detects cycles where **pairwise agreement does not compose to global
agreement**. Around the cycle, each edge may have high agreement (low per-edge entropy),
but the correlation between edges (S_between > 0) means the pairwise agreements are
mutually inconsistent. This is structurally identical to quantum entanglement: local states
appear pure but the global state is mixed.

**Formal connection**: The edge Laplacian L₁ (INV-QUERY-023) encodes disagreement on
edges. Its kernel (harmonic forms) consists of disagreement patterns that cannot be
eliminated by any per-vertex (per-agent) adjustment. These are exactly the patterns
where S_between > 0 around a cycle — the "non-local" incoherence.

The duality between ρ and L₁ is:

```
For edge e = (αᵢ, αⱼ):
  ρ̄_w(i,j) = (1/nW) Σ_{(e,a)} w(a) · agreement_a(vᵢ(a), vⱼ(a))    (agreement)
  L₁(e,e)  = Σ_{(e,a)} w(a) · (1 - agreement_a(vᵢ(a), vⱼ(a)))       (disagreement)

So: ρ̄_w(i,j) + L₁(e,e)/nW = 1/n    (complement relation)
```

The density matrix and edge Laplacian are **complementary views**: ρ encodes agreement,
L₁ encodes disagreement. S(ρ) and β₁ = nullity(L₁) are different projections of the
same underlying structure.

### 4.3 F(S) as Normalized Coherence

The current fitness function (spec/10-bilateral.md):
```
F(S) = 0.18V + 0.18C + 0.18(1-D) + 0.13H + 0.13(1-K) + 0.08(1-I) + 0.12(1-U)
```

This mixes **store-state metrics** (V, C, D, K, I — all derivable from ρ) with
**process metrics** (H = harvest quality, U = mean uncertainty — NOT derivable from ρ).

The density matrix formalism suggests a cleaner decomposition:

```
F(S) = w_state · (1 - S(ρ̄_w)/log(n))  +  w_process · process_score

where:
  store-state component = normalized coherence (from ρ)
  process component = harvest quality + uncertainty management (from methodology adherence)
  w_state + w_process = 1
```

This separates "is the store coherent?" (a mathematical property of ρ) from "is the
process working?" (an empirical property of agent behavior). The separation is important:
a coherent store achieved through bad process is fragile; good process on an incoherent
store is effective but incomplete.

### 4.4 F(T) as Spectral Gap

The topology fitness F(T) (exploration/topology/07-fitness-function.md) has 7 dimensions.
In the density matrix formalism, topology effectiveness reduces to a single quantity:
**how fast does the topology drive ρ toward purity?**

Formally, each merge operation on the topology is a **completely positive trace-preserving
(CPTP) map** Φ_T : 𝒟ₙ → 𝒟ₙ. The spectral gap of Φ_T determines the convergence rate:

```
S(Φ_T^k(ρ)) ≤ S(ρ) · e^{-μ_T · k}

where μ_T = spectral gap of the merge superoperator under topology T
```

A topology with large μ_T drives coherence quickly (effective coordination). A topology
with small μ_T converges slowly (poor coordination). So:

```
F(T) ∝ μ_T = spectral gap of merge superoperator
```

This replaces seven hand-weighted dimensions with one principled quantity.

### 4.5 Persistence as Eigenvalue Evolution

The persistence diagram (INV-TRILATERAL-010) tracks birth/death of H¹ generators. In
the density matrix formalism, the transaction filtration S₀ ⊂ S₁ ⊂ ... ⊂ Sₜ induces:

```
ρ(S₀), ρ(S₁), ..., ρ(Sₜ)
```

The eigenvalue spectrum of ρ(Sᵢ) evolves over time. Eigenvalues splitting away from
1/n toward 0 (purifying) correspond to coherence improvements. Eigenvalues moving
from 0 back toward 1/n correspond to new incoherence (births).

The entropy trajectory S(t) = S(ρ(Sₜ)) is the **coherence EKG** viewed
thermodynamically:
- Decreasing S(t): coherence improving (negentropy production)
- Increasing S(t): coherence degrading (entropy production)
- Rate of change: -dS/dt = **negentropy production rate**

---

## 5. Thermodynamic Interpretation

### 5.1 The Second Law of Coherence

Without active maintenance, coherence degrades:

```
In the absence of coordinated action (merges, reviews, bilateral cycles):
  S(ρ(t+1)) ≥ S(ρ(t))    (entropy non-decreasing)
```

This is the DDIS statement of SEED.md §2's fundamental insight: **divergence is the
natural state; coherence requires active work.** The second law of thermodynamics
applied to specification systems.

Maintaining coherence (decreasing S) requires **negentropy production** — deliberate,
coordinated effort that reduces disorder. The negentropy production rate:

```
N(t) = -dS(ρ)/dt = rate of coherence improvement
```

is a **formal measure of productive work**. A system with N(t) > 0 is effectively
maintaining coherence against entropic pressure. A system with N(t) < 0 is losing
coherence. N(t) = 0 is the boundary — the system is in steady state.

### 5.2 Free Energy and Available Work

In thermodynamics, the **free energy** F = E - TS tells you how much useful work can
be extracted from a system. For coherence:

```
F_coherence = S(ρ_current) - S(ρ_target)    (entropy surplus above target)
```

When F_coherence > 0, there is "work to do" — the store is more disordered than the
target. When F_coherence = 0, the target is reached.

The free energy connects to the guidance system's anti-drift energy (spec/12-guidance.md):

```
E_drift = E_preemption + E_injection + E_detection + E_gate + E_alarm + E_harvest
```

In the density matrix formalism: E_drift is the rate of negentropy injection by the
guidance system. The system is stable (coherence maintained) when E_drift > F_coherence
(the guidance injects more order than the natural tendency toward disorder produces).

### 5.3 Phase Transitions

The coherence entropy S(t) can exhibit **phase transitions** — sudden, discontinuous
changes in the eigenvalue structure of ρ. These correspond to:

- **First-order transitions**: A commit that introduces a fundamental contradiction,
  causing a sudden jump in S. Example: an implementation that reinterprets a spec
  element, creating a new ISP bypass cycle. Detectable as |ΔS| > threshold per transaction.

- **Second-order transitions**: A gradual accumulation of small disagreements that
  suddenly "percolate" into a connected incoherence structure. The entropy increases
  smoothly but the entropy DERIVATIVE changes discontinuously. Detectable as a change
  in the curvature of S(t).

Phase transitions are the formal characterization of "something qualitatively changed
in the project." The signal system (spec/09-signal.md) should emit signals at phase
transitions:

```
SIGNAL_PHASE_TRANSITION: {
    type: FirstOrder | SecondOrder,
    delta_S: f64,           // entropy change
    affected_boundary: IS | SP | IP,
    triggering_tx: TxId,
}
```

---

## 6. Connection to Sheaf Cohomology

### 6.1 The Entanglement-Cohomology Bridge

The sheaf cohomology framework (exploration/sheaf-coherence/) uses the edge Laplacian
L₁ to detect cyclic incoherence (β₁ = dim ker L₁). The density matrix ρ provides a
richer picture of the same phenomenon.

The bridge is the **purity function** on edges:

```
purity(e) = Tr(ρ_edge(e)²)    where ρ_edge(e) is the 2×2 reduced density matrix
                                for edge e = (αᵢ, αⱼ)
```

For a 2×2 density matrix:
- purity = 1 ⟺ pure state ⟺ agents agree
- purity = 1/2 ⟺ maximally mixed ⟺ agents maximally disagree

The per-edge purity defines a weight function on edges:

```
w_purity(e) = 1 - purity(e)    (0 for agreement, 1/2 for maximum disagreement)
```

This weight function IS the weighted edge Laplacian's weight matrix W from INV-QUERY-023.
So the weighted edge Laplacian L₁(W) is:

```
L₁(W) = Bᵀ · diag(w_purity) · B + Cᵀ · C
       = Bᵀ · diag(1 - Tr(ρ_edge²)) · B + Cᵀ · C
```

The Hodge decomposition of L₁(W) then gives:
- **Gradient component** (im Bᵀ): disagreements resolvable by per-agent corrections
- **Harmonic component** (ker L₁): irreducible cyclic incoherence (= entanglement)
- **Curl component** (im C): disagreements resolvable by triangle operations

This chain grounds the sheaf cohomology framework in the density matrix:
```
ρ → purity function → edge weights → L₁(W) → Hodge decomposition → β₁
```

Every step is a well-defined mathematical operation. β₁ is a projection of ρ through
five successive transformations.

### 6.2 What ρ Adds Beyond β₁

β₁ is a count (how many independent cycles). ρ provides:

1. **Cycle severity**: The harmonic energy ‖ω‖² of each harmonic representative ω ∈ ker(L₁),
   weighted by the purity function. High energy = severe cycle. Low energy = mild cycle.

2. **Resolution strategy**: The eigenvectors of ρ̄_w corresponding to eigenvalues < 1/n
   point in the "direction of maximum incoherence." These directions identify which
   agent-pairs and which attributes contribute most to the incoherence.

3. **Continuous monitoring**: β₁ is discrete (integer-valued, jumps). S(ρ) is continuous
   (real-valued, smooth). S can detect approaching phase transitions BEFORE β₁ changes.
   This gives advance warning: "entropy is rising toward a phase transition — a new cycle
   is about to form."

---

## 7. Implementation Path

### 7.1 Stage 0: ISP Triangle (3×3 density matrix)

```rust
use nalgebra::{DMatrix, DVector, SymmetricEigen};

/// Resolution-mode-derived agreement function (Theorem 2.1).
pub fn agreement(v1: &Value, v2: &Value, mode: ResolutionMode) -> f64 {
    match mode {
        ResolutionMode::Lww => if v1 == v2 { 1.0 } else { 0.0 },
        ResolutionMode::Lattice { lattice } => {
            1.0 - lattice.distance(v1, v2) as f64 / lattice.diameter() as f64
        }
        ResolutionMode::Multi => {
            let s1 = v1.as_set();
            let s2 = v2.as_set();
            let intersection = s1.intersection(&s2).count() as f64;
            let union = s1.union(&s2).count() as f64;
            if union == 0.0 { 1.0 } else { intersection / union }
        }
    }
}

/// Coherence density matrix for ISP triangle (3×3).
pub fn isp_density_matrix(
    live_i: &LiveView,
    live_s: &LiveView,
    live_p: &LiveView,
    entities: &[(EntityId, Attribute)],
    schema: &Schema,
) -> DMatrix<f64> {
    let n = 3;
    let mut rho = DMatrix::zeros(n, n);
    let mut total_weight = 0.0;

    for (e, a) in entities {
        let mode = schema.resolution_mode(a);
        let w = resolution_weight(mode);
        let vi = live_i.resolve(e, a);
        let vs = live_s.resolve(e, a);
        let vp = live_p.resolve(e, a);

        let a_is = agreement(&vi, &vs, mode);
        let a_sp = agreement(&vs, &vp, mode);
        let a_ip = agreement(&vi, &vp, mode);

        rho[(0, 0)] += w;       rho[(0, 1)] += w * a_is; rho[(0, 2)] += w * a_ip;
        rho[(1, 0)] += w * a_is; rho[(1, 1)] += w;       rho[(1, 2)] += w * a_sp;
        rho[(2, 0)] += w * a_ip; rho[(2, 1)] += w * a_sp; rho[(2, 2)] += w;

        total_weight += w;
    }

    rho / (n as f64 * total_weight)
}

/// Von Neumann entropy: S = -Σ λᵢ log(λᵢ).
pub fn von_neumann_entropy(rho: &DMatrix<f64>) -> f64 {
    let eigenvalues = rho.clone().symmetric_eigenvalues();
    eigenvalues.iter()
        .filter(|&&v| v > 1e-15)  // skip zero eigenvalues (0·log(0) = 0)
        .map(|&v| -v * v.ln())
        .sum()
}
```

### 7.2 Dependencies

All computation uses nalgebra (ADR-QUERY-012). No new dependencies.

| Operation | nalgebra API | Cost (n=3) | Cost (n=10) |
|-----------|-------------|------------|-------------|
| Eigendecomposition | `symmetric_eigen()` | ~1μs | ~10μs |
| Matrix log | eigenvalues → -λ log λ | ~0.1μs | ~1μs |
| Total per transaction | | ~2μs | ~20μs |

### 7.3 Self-Bootstrap Verification

After the spec self-bootstrap (transacting spec elements into the store), compute:

```
ρ_self = isp_density_matrix(LIVE_I_spec, LIVE_S_spec, LIVE_P_spec, ...)
S_self = von_neumann_entropy(ρ_self)
```

**Expected result**: S_self = 0 (the spec's own trilateral structure should be coherent).
Any S_self > 0 reveals internal incoherence in the specification itself — a self-bootstrap
coherence failure that must be resolved before the system can verify external coherence.

---

## 8. Open Questions and Uncertainties

### OQ-1: Cross-Type Boundary Agreement (Confidence: 0.7)
The :spec/traces-to attribute uses String values referencing SEED.md sections, while
:spec/implements uses Ref values pointing to entity IDs. Agreement across the ISP
boundary compares values in DIFFERENT type domains. Currently handled by checking
link existence (INV-TRILATERAL-008), not content agreement.

**What would resolve it**: Stage 0 implementation revealing cases where link existence
is insufficient and content agreement is needed.

### OQ-2: Tensor Product vs Average (Confidence: 0.8)
The averaged density matrix ρ̄_w loses per-attribute correlation structure. The claim
that S_between captures what β₁ detects (§3.3, §4.2) relies on the relationship between
subadditivity violation and the entanglement-cycle correspondence. A rigorous proof
would strengthen the unification claim.

**What would resolve it**: Constructing an explicit counterexample store where S_between = 0
but β₁ > 0, or proving no such store exists.

### OQ-3: F(S) Decomposition (Confidence: 0.6)
The proposed split of F(S) into store-state and process components (§4.3) changes the
semantics of the fitness function. The current F(S) is a single number that the bilateral
loop optimizes. Splitting it changes the optimization target.

**What would resolve it**: Empirical comparison during Stage 0 implementation — does the
density-matrix-derived coherence score correlate with the hand-weighted F(S)?

---

*The coherence density matrix is the mathematical unification that DDIS has been building
toward. Every metric in the system — Φ, β₁, F(S), F(T), persistence — is a shadow of a
single algebraic object. The density matrix encodes the full correlation structure of
agreement across perspectives, the von Neumann entropy provides a universal scalar summary,
and the thermodynamic interpretation gives physical intuition: divergence is entropy,
coherence is order, and intelligence is the rate of negentropy production.*
