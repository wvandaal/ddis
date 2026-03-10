# 01 — Hodge Theory and the Spectral Connection

> **Status**: EXPLORATION
> **Date**: 2026-03-09
> **Traces to**: SEED.md §4 (datom algebra), spec/03-query.md (spectral computation),
>   ADR-QUERY-012 (nalgebra), INV-QUERY-022 (spectral correctness)
> **Builds on**: 00-sheaf-cohomology-for-coherence.md (Čech cohomology construction)
>
> **Thesis**: The Hodge decomposition theorem connects sheaf cohomology (discrete) to
> spectral graph theory (already specified in Braid). The existing Laplacian infrastructure
> computes H⁰ for free; extending to the 1-form Laplacian gives H¹ with no new dependencies.

---

## 1. The Discrete Hodge Decomposition

The Hodge decomposition theorem for graphs states that the space of 1-forms (edge functions)
on a graph decomposes into three orthogonal subspaces:

```
C¹(G) = im(δ₀) ⊕ ker(L₁) ⊕ im(δ₁ᵀ)
```

where:
- **im(δ₀)** = gradient flows (exact 1-forms) — edge functions that come from vertex
  potentials. These are "tree-like" disagreements that can be resolved by assigning
  each agent a correct value.
- **ker(L₁)** = harmonic 1-forms ≅ H¹ — edge functions that are neither gradients nor
  co-gradients. These are the **irreducible cyclic disagreements**.
- **im(δ₁ᵀ)** = curl flows (co-exact 1-forms) — edge functions that come from face
  potentials. These are "locally resolvable" through triangle operations.

### 1.1 The Three Laplacians

```
L₀ = δ₀ᵀ δ₀                           — vertex Laplacian (standard graph Laplacian)
L₁ = δ₀ δ₀ᵀ + δ₁ᵀ δ₁                 — edge Laplacian (1-form Laplacian)
L₂ = δ₁ δ₁ᵀ                           — triangle Laplacian (2-form Laplacian)
```

**Key relationships**:
- ker(L₀) ≅ H⁰ — the vertex Laplacian's kernel gives connected components
- ker(L₁) ≅ H¹ — the edge Laplacian's kernel gives incoherence cycles
- ker(L₂) ≅ H² — the triangle Laplacian's kernel gives voids

### 1.2 What Braid Already Computes

INV-QUERY-022 (Spectral Computation Correctness) specifies:

```
Given: dependency graph G_dep with n nodes
Compute:
  L = D - A                    (graph Laplacian)
  eigenvalues λ₀ ≤ λ₁ ≤ ... ≤ λₙ₋₁
  Fiedler vector v₁            (eigenvector for λ₁)
```

This is L₀ — the vertex Laplacian. Its kernel dimension equals β₀ (number of connected
components). The Fiedler value λ₁ measures algebraic connectivity.

**To get H¹, we need L₁** — the 1-form Laplacian. This requires:
1. The incidence matrix B (n × m matrix, already implicit in L₀ = BBᵀ)
2. The triangle incidence matrix C (m × t matrix)
3. L₁ = BᵀB + CᵀC

All of these are sparse matrices computable from the graph structure alone. The nalgebra
dependency (ADR-QUERY-012) already provides the eigenvalue solver.

### 1.3 The Extension: From L₀ to L₁

```rust
/// Existing: compute the vertex Laplacian L₀.
pub fn vertex_laplacian(graph: &DepGraph) -> DMatrix<f64> {
    let b = incidence_matrix(graph);  // n × m
    &b * b.transpose()               // n × n
}

/// New: compute the edge Laplacian L₁.
pub fn edge_laplacian(graph: &DepGraph) -> DMatrix<f64> {
    let b = incidence_matrix(graph);    // n × m
    let c = triangle_matrix(graph);     // m × t
    b.transpose() * &b + c.transpose() * &c  // m × m
}

/// Extract H¹ from L₁.
pub fn h1_rank(graph: &DepGraph) -> usize {
    let l1 = edge_laplacian(graph);
    let eigenvalues = l1.symmetric_eigenvalues();
    eigenvalues.iter().filter(|&&v| v.abs() < 1e-10).count()
}
```

The `nalgebra` symmetric eigenvalue solver (already a dependency) handles this directly.

---

## 2. Weighted Hodge Theory for Resolution Modes

### 2.1 Per-Attribute Weighting

Not all disagreements are equal. A disagreement on `:task/priority` (lattice-resolved,
will auto-converge to max) is less concerning than a disagreement on `:spec/statement`
(multi-value, requires human resolution).

Define a weight function w: A → R⁺ that assigns importance to each attribute:

```
w(a) = {
    0.1   if resolution_mode(a) = :lattice   (auto-converges)
    0.5   if resolution_mode(a) = :lww       (converges after merge)
    1.0   if resolution_mode(a) = :multi     (requires deliberation)
}
```

The **weighted edge Laplacian**:

```
L₁(w) = Bᵀ W B + Cᵀ W' C
```

where W is the diagonal weight matrix on edges (weighted by the attributes that disagree
on each edge). The kernel of L₁(w) gives **weighted H¹** — incoherence cycles where the
disagreements are semantically significant.

### 2.2 Harmonic Representatives

The Hodge decomposition doesn't just detect H¹ ≠ 0 — it gives the **harmonic
representative** of each cohomology class. This is the unique 1-form in each class
that minimizes the L₂ norm (energy).

For DDIS, the harmonic representative of an H¹ generator is the **minimum-weight cycle
of disagreement** — the simplest characterization of a cyclic incoherence. This is
directly useful for diagnostic output:

```
Incoherence cycle [2]:
  Harmonic representative: α→β (disagreement: :spec/statement for INV-STORE-001)
                         → β→γ (disagreement: :impl/behavior for store::transact)
                         → γ→α (disagreement: :intent/goal for SEED §4 Axiom 2)
  Energy: 2.3 (weighted by resolution mode)
  Recommendation: resolve at the β→γ edge (highest individual weight: :impl/behavior)
```

---

## 3. The Heat Equation Interpretation

### 3.1 Diffusion on the Coherence Complex

The heat equation on a graph is:

```
∂f/∂t = -L₀ f     (vertex heat equation)
```

This models information diffusion: heat flows from high to low, reaching equilibrium
when all vertices have the same temperature. For DDIS, this models **merge propagation**:
knowledge flows from agents who have it to agents who don't, reaching coherence when
all agents agree.

The heat equation on 1-forms:

```
∂ω/∂t = -L₁ ω     (edge heat equation)
```

This models **disagreement diffusion**: the 1-form ω represents pairwise disagreements
between agents. Heat flow drives ω toward the harmonic part (the projection onto ker(L₁)).
The non-harmonic part decays exponentially; the harmonic part persists forever.

**Translation to DDIS**: After sufficient merges, all "tree-like" disagreements (the exact
part of ω) diffuse away. What remains is the harmonic part — the cyclic incoherence that
cannot be resolved by pairwise merges alone. This is exactly H¹.

### 3.2 Convergence Rate from Spectral Gap

The smallest nonzero eigenvalue of L₁ (the **1-form spectral gap** μ₁) determines how
fast tree-like disagreements diffuse:

```
||ω(t) - ω_harmonic|| ≤ e^(-μ₁ t) ||ω(0) - ω_harmonic||
```

For DDIS, μ₁ gives the **merge convergence rate**: how quickly pairwise merges resolve
non-structural disagreements. A larger μ₁ means faster convergence — the communication
topology is more effective at propagating information.

This connects directly to the topology fitness function F(T) from the execution-topologies
exploration: the optimal topology maximizes μ₁ (fastest convergence) subject to merge
overhead constraints. The spectral gap of L₁ is a formal measure of topology effectiveness.

---

## 4. Sheaf Laplacian: Beyond Binary Disagreement

### 4.1 The Sheaf Laplacian (Hansen-Ghrist Construction)

The standard graph Laplacian treats edges as binary (connected/not connected). The
**sheaf Laplacian** (Hansen and Ghrist, 2019) generalizes this to sheaves with
non-trivial restriction maps.

For the coherence sheaf V:

```
L_F = δ₀ᵀ δ₀
```

where δ₀ is the sheaf coboundary map (not just the graph incidence matrix). Each edge
contributes a block to δ₀ based on the restriction maps ρ_αβ.

For binary coefficients (agree/disagree), the sheaf Laplacian reduces to the standard
weighted Laplacian. For richer coefficients (the actual resolved values), it captures
the full structure of the disagreement.

### 4.2 Connection to Existing Resolution Modes

Each resolution mode defines a different restriction map:

| Resolution Mode | Restriction Map | Sheaf Laplacian Contribution |
|----------------|----------------|------------------------------|
| LWW | ρ(v₁, v₂) = (v₁ = v₂) | Binary: 0 if agree, 1 if disagree |
| Lattice | ρ(v₁, v₂) = d_lattice(v₁, v₂) | Continuous: lattice distance |
| Multi-value | ρ(v₁, v₂) = |v₁ △ v₂| / |v₁ ∪ v₂| | Jaccard distance |

The sheaf Laplacian with resolution-aware restriction maps gives a **resolution-mode-aware
coherence metric**: disagreements on lattice-resolved attributes contribute less than
disagreements on multi-value attributes, because lattice-resolved attributes will
auto-converge.

### 4.3 The Spectral Sheaf Gap

The smallest nonzero eigenvalue of L_F (the sheaf spectral gap) gives a tighter bound
on convergence rate than the standard spectral gap, because it accounts for the sheaf
structure:

```
μ_F ≤ μ₁    (sheaf spectral gap ≤ standard spectral gap)
```

If μ_F << μ₁, the sheaf structure (resolution modes) creates bottlenecks that the
standard graph topology doesn't reveal. This means: the communication graph looks
well-connected, but the *semantic structure* of the attributes creates slow convergence.

**This is a genuinely useful diagnostic**: a project where μ_F << μ₁ should change its
resolution modes or attribute structure, not its communication topology.

---

## 5. Algorithmic Summary

### 5.1 The Full Computation Pipeline

```
Input: Store S, Agent set V, Communication graph G = (V, E)

Step 1: Build the resolved views
  For each α ∈ V: V(α) = LIVE(F_α)

Step 2: Build the coboundary matrices
  B = incidence matrix (n × m)     — from G
  C = triangle matrix (m × t)      — from triangles in G
  W = weight matrix (m × m)        — from resolution modes

Step 3: Compute Laplacians
  L₀ = B Bᵀ                        — vertex Laplacian (already computed for spectral ops)
  L₁ = Bᵀ W B + Cᵀ W' C           — weighted edge Laplacian

Step 4: Compute cohomology
  β₀ = nullity(L₀)                 — connected components
  β₁ = nullity(L₁)                 — incoherence cycles
  generators = eigenvectors of L₁ with eigenvalue 0

Step 5: Diagnostics
  If β₁ > 0:
    For each generator g:
      Extract the cycle (which agents, which attributes)
      Compute the harmonic energy (weighted importance)
      Compute the birth time (when this cycle appeared)
      Emit guidance: "Cyclic incoherence in {cycle}, coordinate resolution"

Output: (Φ, β₀, β₁, generators, persistence_diagram)
```

### 5.2 Incremental Updates

When a new transaction arrives, the full recomputation is unnecessary. Only edges
where the pairwise disagreement changes need updating:

```
On TRANSACT(datoms):
  For each datom d in datoms:
    For each agent α that received d:
      For each neighbor β of α:
        If resolve(F_α, d.a) changed:
          Update Δ(α, β) for attribute d.a
          Mark edge (α, β) as dirty

  If any edge is dirty:
    Recompute L₁ (sparse update: only dirty edges change)
    Recompute nullity(L₁)
    If β₁ changed: recompute generators
```

Amortized cost: O(degree(α) × |dirty_attributes|) per transaction — typically O(1)
for single-datom transactions.

---

## 6. Concrete Example: Self-Bootstrap Coherence Check

### 6.1 Setup

After the self-bootstrap (transacting spec elements into the store), we have:
- Intent: SEED.md sections (transacted as intent datoms)
- Spec: INV/ADR/NEG elements (transacted as spec datoms)
- Impl: (not yet implemented at bootstrap time)

The ISP triangle at bootstrap:
```
I ——(:spec/traces-to)—— S
                          |
                    (no :spec/implements yet)
                          |
                          P = ∅
```

At this point:
- Φ = D_IS + D_SP
- D_IS = count of unlinked intents (SEED sections without traces-to)
- D_SP = count of specs (all, since no impl exists)
- H¹ = 0 (no cycle — P is empty, no I→P bypass possible)

### 6.2 After Stage 0 Implementation Begins

As code is written:
```
I ——(:spec/traces-to)—— S ——(:spec/implements)—— P
 \                                                /
  \—————————(:impl/source-intent)————————————————/
```

If an implementor reads SEED.md directly and writes code without checking the spec:
- The :impl/source-intent link creates the I→P bypass edge
- The ISP triangle now has a cycle
- If the implementor's interpretation of SEED.md differs from the spec's interpretation:
  **H¹ ≠ 0** — specification bypass detected

The system reports:
```
COHERENCE WARNING: Specification bypass detected
  Intent: SEED §4 Axiom 2 (Append-Only Immutability)
  Spec: INV-STORE-001 (statement: "store never deletes or mutates")
  Impl: store.rs:transact() (behavior: allows retraction of datoms)

  The spec says "never deletes or mutates" but the implementation allows
  retraction (which is a form of logical deletion). The intent (SEED §4)
  says "retractions are new datoms with op=retract" — the implementation
  follows the intent directly but contradicts the spec's phrasing.

  Resolution: Update INV-STORE-001 statement to clarify that retraction
  is an assertion of op=retract, not a deletion. This is a spec phrasing
  issue, not an implementation bug.
```

This is precisely the kind of subtle coherence issue that Φ misses (all links exist!)
but H¹ catches (the links are inconsistent around the cycle).

---

*This document establishes the spectral connection between the sheaf cohomology framework
(doc 00) and the existing nalgebra-based spectral infrastructure (INV-QUERY-022). The key
insight: extending from L₀ (vertex Laplacian, already specified) to L₁ (edge Laplacian,
new) gives H¹ computation with no new dependencies. The Hodge decomposition provides
both the diagnostic (harmonic generators = irreducible incoherence) and the convergence
rate (spectral gap = merge effectiveness). The sheaf Laplacian generalizes the construction
to account for resolution modes, giving resolution-mode-aware coherence metrics.*
