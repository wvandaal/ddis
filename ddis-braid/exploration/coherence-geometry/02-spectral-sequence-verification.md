# 02 — Spectral Sequence Verification: Staged Coherence via Filtration

> **Status**: EXPLORATION
> **Date**: 2026-03-09
> **Traces to**: SEED.md §1 (coherence verification), §6 (reconciliation),
>   §10 (staged roadmap)
> **Builds on**: 00-density-matrix-coherence.md (density matrix),
>   01-information-geometry-manifold.md (Bures metric),
>   exploration/sheaf-coherence/01-hodge-theory.md (Hodge decomposition),
>   spec/10-bilateral.md (AUTO_CYCLE, FULL_CYCLE),
>   spec/18-trilateral.md (ISP triangle, Φ, (Φ,β₁) duality)
>
> **Thesis**: The multi-level structure of DDIS (Intent → Spec → Impl) induces a
> filtration on the coherence complex. The spectral sequence of this filtration
> stratifies verification into per-level (cheap, parallelizable) and cross-level
> (expensive, coordination-requiring) stages. The E₂ degeneration criterion provides
> a formal early-termination guarantee: if per-level checks pass and boundary maps
> are consistent, total coherence follows without cross-level computation.

---

## 1. The Coherence Filtration

### 1.1 Motivation: Why Staged Verification?

The full coherence check (Φ, β₁, S) requires examining all entity-attribute pairs
across all three LIVE views. For a store with m entity-attribute pairs and n agents,
this is O(n²m) — fast in absolute terms (microseconds) but wasteful if most of the
store is already coherent.

More importantly, different parts of the coherence check have different **coordination
requirements** (CALM theorem). Per-level coherence (Is LIVE_S self-consistent?) is
monotonic — it can be computed from local data without cross-view coordination.
Cross-level coherence (Does LIVE_S agree with LIVE_P across :spec/implements links?)
is non-monotonic — it requires knowing both views simultaneously.

The spectral sequence FORMALIZES this stratification: each page corresponds to a
coordination stratum, and the degeneration criterion tells you exactly when you can
stop.

### 1.2 The Three-Level Filtration

The store S has three abstraction levels: Intent (I), Spec (S), Impl (P). The coherence
complex C* (from sheaf-coherence/00-sheaf-cohomology.md §3) decomposes by level:

```
C*(S) = total coherence complex (all entity-attribute pairs, all views)

Filtration:
  F₀C* = C*(LIVE_I)                    — Intent-only coherence
  F₁C* = C*(LIVE_I ∪ LIVE_S)           — Intent + Spec coherence
  F₂C* = C*(LIVE_I ∪ LIVE_S ∪ LIVE_P)  — Full coherence = C*(S)

Inclusions: F₀C* ⊆ F₁C* ⊆ F₂C* = C*(S)
```

This is a **finite filtration** of the coherence complex by abstraction level.
Each inclusion adds one LIVE view's contribution.

### 1.3 Graded Pieces

The **associated graded** complex captures per-level contributions:

```
Gr₀C* = F₀C*                           — Intent level alone
Gr₁C* = F₁C* / F₀C*                   — Spec level relative to Intent
Gr₂C* = F₂C* / F₁C*                   — Impl level relative to Intent+Spec
```

Each graded piece captures what the corresponding level ADDS to the coherence picture
beyond what the previous levels already contain.

---

## 2. The Spectral Sequence

### 2.1 Construction

The spectral sequence of a filtered complex is a sequence of bigraded pages:

```
E_r^{p,q}  for r = 0, 1, 2, ...

with differentials d_r : E_r^{p,q} → E_r^{p+r, q-r+1}

and E_{r+1}^{p,q} = ker(d_r) / im(d_r)    (each page is the cohomology of the previous)
```

**Convergence**: For a filtration of length L (here L = 2), the spectral sequence
converges at page E_{L+1} = E₃:

```
E₃^{p,q} = E_∞^{p,q} ≅ Gr_p H^{p+q}(C*(S))
```

The graded pieces of the total cohomology are computable from the E₃ page.

### 2.2 Page-by-Page Computation

**E₀ page (raw data)**:
```
E₀^{p,q} = Gr_p C^{p+q} = F_p C^{p+q} / F_{p-1} C^{p+q}

E₀^{0,q} = per-Intent-level cochains
E₀^{1,q} = per-Spec-level cochains (relative to Intent)
E₀^{2,q} = per-Impl-level cochains (relative to Intent+Spec)
```

d₀ is the coboundary within each level. Computing E₁ = H(E₀, d₀) gives per-level cohomology.

**E₁ page (per-level cohomology)**:
```
E₁^{p,q} = H^{p+q}(Gr_p C*)

E₁^{0,0} = H⁰(Intent level)     — connected components of Intent graph
E₁^{0,1} = H¹(Intent level)     — incoherence cycles within Intent
E₁^{1,0} = H⁰(Spec level, rel Intent)   — Spec entities not linked to Intent
E₁^{1,1} = H¹(Spec level, rel Intent)   — Spec-level cycles involving Intent boundary
E₁^{2,0} = H⁰(Impl level, rel Intent+Spec)
E₁^{2,1} = H¹(Impl level, rel Intent+Spec)
```

These are computed **independently per level** — no cross-level coordination needed.
This is the CALM-monotonic stratum.

**d₁ differential (boundary maps)**:
```
d₁ : E₁^{p,q} → E₁^{p+1,q}

d₁ : E₁^{0,q} → E₁^{1,q}     — Intent-to-Spec boundary map
d₁ : E₁^{1,q} → E₁^{2,q}     — Spec-to-Impl boundary map
```

d₁ is the **connecting homomorphism** from the long exact sequence of the pair
(F_{p+1}, F_p). It detects when within-level cohomology classes are "killed" or
"created" by the inclusion of the next level.

**DDIS interpretation of d₁**:
- d₁: E₁^{0,1} → E₁^{1,1}: "Does adding Spec-level data resolve or create Intent-level cycles?"
  - d₁ ≠ 0 on a class iff the Spec interpretation differs from the Intent interpretation on
    that cycle — a specification-level disagreement.
- d₁: E₁^{1,1} → E₁^{2,1}: "Does adding Impl-level data resolve or create Spec-level cycles?"
  - d₁ ≠ 0 on a class iff the implementation disagrees with the spec on a cycle —
    a specification bypass (INV-TRILATERAL-008).

**E₂ page (cross-level cohomology)**:
```
E₂^{p,q} = ker(d₁^{p,q}) / im(d₁^{p-1,q})
```

E₂ captures the surviving cohomology after accounting for cross-level interactions.

**d₂ differential (higher-order cross-level maps)**:
```
d₂ : E₂^{p,q} → E₂^{p+2, q-1}

d₂ : E₂^{0,1} → E₂^{2,0}
```

d₂ maps Intent-level cycles to Impl-level obstructions, BYPASSING the Spec level.
This is the **ISP triangle obstruction** — exactly the specification bypass that
INV-TRILATERAL-008 detects. d₂ ≠ 0 iff there exists a path I → P that contradicts
the I → S → P path.

**E₃ = E_∞ (final page)**:

For a 3-step filtration, E₃ = E_∞. The total coherence is recovered from E₃.

### 2.3 Summary Table

| Page | Computes | Coordination | CALM Stratum | Cost |
|------|----------|-------------|-------------|------|
| E₁ | Per-level H* | None (parallel) | Monotonic | O(n²m/3) per level |
| d₁ | Boundary maps | Pairwise (adjacent levels) | Non-monotonic | O(n²·\|links\|) |
| E₂ | Cross-level H* | Pairwise | Non-monotonic | O(n²·\|links\|) |
| d₂ | ISP bypass | Global (all 3 levels) | Non-monotonic | O(n²·\|triangles\|) |
| E₃ | Final answer | — | — | Free (just E₂ quotient) |

---

## 3. The Degeneration Criterion: When to Stop Early

### 3.1 E₂ Degeneration

**Theorem 3.1.** If d₁ = 0 (all boundary maps vanish), then E₂ = E₁ and:
```
H^q(C*(S)) = E₁^{0,q} ⊕ E₁^{1,q} ⊕ E₁^{2,q}
```

Total cohomology is the direct sum of per-level cohomologies. **No cross-level
computation is needed.**

**DDIS condition for d₁ = 0**: All :spec/traces-to links are consistent (the Spec
view's interpretation of each Intent entity matches the Intent view's value) AND
all :spec/implements links are consistent (the Impl view's realization of each Spec
entity matches the Spec view's statement).

In practice: if the links exist and are value-consistent, d₁ = 0.

### 3.2 E₃ Degeneration

Even if d₁ ≠ 0, d₂ may still vanish. If d₂ = 0:
```
E₃ = E₂    and    H^q(C*(S)) recovered from E₂
```

This means: there are boundary inconsistencies (d₁ ≠ 0) but no ISP bypass cycles (d₂ = 0).
The inconsistencies are "linear" (fixable at each boundary independently), not "cyclic"
(requiring coordinated multi-boundary resolution).

### 3.3 The Complete Decision Tree

```
STAGED_COHERENCE_CHECK(S):

  // Stage 1: Per-level computation (E₁ page)
  E₁ = compute_per_level_cohomology(S)        // parallelizable, O(n²m/3) each

  if ∀ (p,q): E₁^{p,q} = 0 for q > 0:
    // No within-level incoherence
    
    // Stage 2: Boundary check (d₁)
    d₁ = compute_boundary_differentials(S)      // O(n²·|links|)
    
    if d₁ = 0:
      return COHERENT                            // E₂ degeneration — EARLY EXIT
    else:
      // Stage 3: ISP check (d₂)
      d₂ = compute_isp_differential(S)          // O(n²·|triangles|)
      
      if d₂ = 0:
        return BOUNDARY_INCOHERENT(d₁)          // fixable at individual boundaries
      else:
        return CYCLE_INCOHERENT(d₂)             // requires coordinated ISP resolution
  else:
    // Within-level incoherence exists
    return LEVEL_INCOHERENT(E₁)                 // fix within-level first
```

### 3.4 Early Exit Probability

For a well-maintained project:
- **E₁ degeneration** (all levels internally coherent): Common. Single-agent projects
  always have per-level H¹ = 0 (no multi-agent cycles within a level).
- **d₁ = 0** (boundaries consistent): Common when bilateral cycles are running.
  The bilateral loop (INV-BILATERAL-002) specifically ensures Spec↔Impl consistency.
- **d₂ = 0** (no ISP bypass): Common when the specification process is followed.
  ISP bypasses occur when agents implement directly from intent, bypassing spec.

**Expected outcome in Stage 0**: The algorithm exits at "E₂ degeneration — EARLY EXIT"
in the vast majority of cases. The full d₂ computation is needed only when a
specification bypass has occurred — which is exactly when the full ISP check is most
valuable.

---

## 4. CALM Alignment

### 4.1 Stratification by Coordination Requirement

The spectral sequence pages align precisely with CALM:

```
E₁ computation = monotonic (per-level, no coordination)
  ↓ requires sync barrier to proceed (need all E₁ results) ↓
d₁ computation = non-monotonic (cross-level, pairwise coordination)
  ↓ requires sync barrier to proceed ↓
d₂ computation = non-monotonic (ISP triangle, global coordination)
```

Each sync barrier corresponds to a non-monotonic operation in the CALM sense:
checking boundary consistency requires knowing BOTH views (an absence query —
"are there ANY inconsistencies?"), which is non-monotonic.

### 4.2 Connection to Bilateral Loop

The bilateral loop stages map to spectral sequence pages:

| Bilateral Operation | Spectral Sequence Page | Computes |
|---|---|---|
| FORWARD_SCAN | E₁^{1,*} (Spec relative to Intent) | Per-level spec coherence |
| BACKWARD_SCAN | d₁^{1,*} (Impl → Spec boundary) | Spec↔Impl boundary consistency |
| COMPUTE_FITNESS | E₂ assembly | Cross-level coherence state |
| ISP_CHECK | d₂ | Specification bypass detection |

**AUTO_CYCLE** (INV-BILATERAL-002) computes through d₁ (machine-evaluable coherence
conditions CC-1, CC-2, CC-4, CC-5). **FULL_CYCLE** adds the d₂ computation plus
CC-3 (axiological alignment — the human-gated condition).

This means: the spectral sequence provides a **formal framework** for the bilateral
loop's existing staged structure. AUTO_CYCLE is "compute through E₂, stop if
degeneration occurs." FULL_CYCLE is "compute through E₃ with human verification of d₂."

---

## 5. Self-Bootstrap: The Specification Verifies Itself

### 5.1 Coherence of the Coherence Framework

The three exploration documents (00, 01, 02) and the promoted spec elements
(INV-QUERY-023..024, INV-TRILATERAL-008..010, ADR-QUERY-013, ADR-TRILATERAL-005..006)
form a coherence structure that should itself be verifiable:

```
Intent: SEED.md §1 (coherence verification), §2 (divergence), §6 (reconciliation)
  ↓ :spec/traces-to
Spec: INV-TRILATERAL-009 ((Φ,β₁) duality), INV-QUERY-023 (edge Laplacian), etc.
  ↓ :spec/implements
Impl: (Stage 0 implementation — not yet written)
```

After self-bootstrap, the density matrix of the coherence framework's own ISP triangle
should have S = 0 for the Intent↔Spec boundary (all exploration claims are formalized
as spec elements) and S > 0 for the Spec↔Impl boundary (implementation not yet written).

The spectral sequence should show:
- E₁: per-level H¹ = 0 (no within-level cycles — the spec doesn't contradict itself)
- d₁: non-zero on the Spec↔Impl boundary (implementation gaps exist)
- d₂: zero (no ISP bypasses — the spec faithfully formalizes the exploration)

### 5.2 Verifying the Self-Bootstrap

```bash
# After transacting spec elements into the store:
braid coherence --staged --filter coherence-geometry
# Expected:
#   E₁: Intent H¹ = 0, Spec H¹ = 0, Impl H¹ = 0 (no within-level cycles)
#   d₁: Intent→Spec = 0 (all traces-to links present)
#       Spec→Impl ≠ 0 (12 unimplemented invariants)
#   d₂: 0 (no specification bypasses)
#   Result: BOUNDARY_INCOHERENT on Spec↔Impl boundary (expected: impl not written yet)
```

---

## 6. Density Matrix + Spectral Sequence Integration

### 6.1 Per-Page Entropy

The density matrix and spectral sequence compose: compute the entropy at EACH page
to get a "resolution of entropy by coordination stratum":

```
S₁ = von_neumann_entropy(ρ_E₁)    // entropy from per-level structure
S₂ = von_neumann_entropy(ρ_E₂)    // entropy from cross-level structure
S₃ = von_neumann_entropy(ρ_E₃)    // total entropy

S₃ = S₁ + ΔS_boundary + ΔS_ISP

where:
  ΔS_boundary = S₂ - S₁    (entropy introduced by boundary inconsistencies)
  ΔS_ISP = S₃ - S₂         (entropy introduced by ISP bypass cycles)
```

This decomposition tells you WHERE the entropy comes from:
- High S₁: within-level problems (internal contradictions in spec or impl)
- High ΔS_boundary: boundary problems (spec↔impl misalignment)
- High ΔS_ISP: specification bypass (implementing from intent, not spec)

### 6.2 Geodesic Pursuit with Staged Computation

The geodesic pursuit algorithm (doc 01 §2.2) benefits from staged verification:

```
STAGED_GEODESIC_PURSUIT(ρ, ρ_target, candidates):
  // Stage 1: filter candidates by E₁ improvement
  candidates_filtered = candidates.filter(|τ|
    E₁_after(τ) improves or does not regress E₁_before
  )
  
  // Stage 2: among filtered, pick by full d_B reduction
  // (compute full density matrix only for filtered set)
  best = argmin_{τ ∈ candidates_filtered} d_B(apply(ρ, τ), ρ_target)
  
  return best
```

Stage 1 is cheap (per-level only, parallelizable). Stage 2 is expensive (full density
matrix) but runs on a reduced candidate set. This gives the same result as full geodesic
pursuit with potentially significant speedup.

---

## 7. Stage 0 Concrete Algorithm

### 7.1 The ISP Spectral Sequence (n = 3)

For Stage 0, the three "agents" are the LIVE views (I, S, P). The spectral sequence
specializes to a particularly clean form:

```
Agents: {I, S, P}
Graph: complete graph K₃ (all pairs communicate)
Edges: {(I,S), (S,P), (I,P)}
Triangles: {(I,S,P)}
```

**E₁ page**: Per-view coherence. Since each view is a single LIVE projection (not multi-agent),
H¹ within each view is 0. So E₁^{p,1} = 0 for all p. This means: **at Stage 0, E₁
always degenerates.** The interesting computation starts at d₁.

**d₁**: Boundary maps. These check :spec/traces-to (I→S) and :spec/implements (S→P)
link consistency. This is the existing Φ computation (INV-TRILATERAL-002).

**d₂**: ISP triangle check. This detects specification bypasses. This is the existing
ISP check (INV-TRILATERAL-008).

So at Stage 0, the spectral sequence reduces to:
1. Check Φ (d₁ computation)
2. If Φ = 0, check ISP bypass (d₂ computation)
3. If ISP clean, report COHERENT

This is exactly the existing (Φ, β₁) check — the spectral sequence FORMALIZES the
existing algorithm rather than replacing it. The value of the spectral sequence appears
at Stage 2+, where per-level H¹ can be non-zero and the early-termination criterion
becomes non-trivial.

### 7.2 Rust Implementation Sketch

```rust
pub enum StageResult {
    Coherent,
    LevelIncoherent { level: Level, beta_1: usize },
    BoundaryIncoherent { boundary: Boundary, phi: usize },
    CycleIncoherent { bypasses: Vec<IspBypass> },
}

pub fn staged_coherence_check(store: &Store) -> StageResult {
    // E₁: per-level cohomology
    let (h1_i, h1_s, h1_p) = rayon::join3(
        || betti_1_level(store, Level::Intent),
        || betti_1_level(store, Level::Spec),
        || betti_1_level(store, Level::Impl),
    );

    if h1_i > 0 { return StageResult::LevelIncoherent { level: Level::Intent, beta_1: h1_i } }
    if h1_s > 0 { return StageResult::LevelIncoherent { level: Level::Spec, beta_1: h1_s } }
    if h1_p > 0 { return StageResult::LevelIncoherent { level: Level::Impl, beta_1: h1_p } }

    // d₁: boundary maps (= Φ computation)
    let phi = compute_phi(store);  // INV-TRILATERAL-002
    if phi > 0 {
        return StageResult::BoundaryIncoherent {
            boundary: highest_phi_boundary(store),
            phi,
        };
    }

    // d₂: ISP triangle check
    let bypasses = isp_check_all(store);  // INV-TRILATERAL-008
    if !bypasses.is_empty() {
        return StageResult::CycleIncoherent { bypasses };
    }

    StageResult::Coherent
}
```

---

## 8. Open Questions

### OQ-1: Relative Cohomology Implementation (Confidence: 0.8)
The graded pieces Gr_p C* = F_p C* / F_{p-1} C* involve quotient complexes. Implementing
quotient chain complexes requires tracking which generators are "new" at each level vs.
inherited from the previous level. This is straightforward for the ISP filtration (the
attribute namespace partition gives an explicit decomposition) but may need care for
non-partition-based filtrations.

### OQ-2: Spectral Sequence for N > 3 Levels (Confidence: 0.7)
The quadrilateral extension (Intent ↔ Spec ↔ Impl ↔ Topology) has a 4-step filtration.
The spectral sequence converges at E₅ (one page beyond the filtration length plus one).
The d₃ differential detects "long-range" bypasses (Intent → Topology, skipping Spec
and Impl). Whether such bypasses are meaningful in practice needs Stage 3 experience.

### OQ-3: Persistent Spectral Sequences (Confidence: 0.5)
The spectral sequence at each transaction gives a "snapshot" of staged coherence. The
sequence of spectral sequences over the transaction filtration is a **persistent spectral
sequence** — a recent construction in algebraic topology (Edelsbrunner et al., 2021).
Whether persistent spectral sequences give useful diagnostics beyond persistent H¹
alone is unclear. This is the most speculative direction.

---

*The spectral sequence stratifies coherence verification by coordination cost: per-level
checks are free (parallelizable, monotonic), boundary checks require pairwise coordination,
and ISP cycle checks require global coordination. The degeneration criterion gives formal
early termination: if per-level coherence holds and boundaries are consistent, total
coherence follows without computing the full ISP check. For Stage 0, the spectral sequence
reduces to the existing (Φ, β₁) algorithm — it adds no new computation, only a formal
framework that justifies why the existing algorithm works and generalizes naturally to
multi-agent settings at Stage 2+.*