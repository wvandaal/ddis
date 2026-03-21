> **DEPRECATED**: This file is bootstrap scaffolding. The canonical source of truth is the braid datom store. Use `braid spec show` and `braid query` to access spec elements. See ADR-STORE-019.

---

> **Namespace**: COHERENCE | **Wave**: 4 (Integration) | **Stage**: 1+
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## &sect;20. COHERENCE &mdash; Coherence Geometry Framework

### &sect;20.0 Overview

The coherence geometry framework elevates the trilateral coherence model (TRILATERAL
namespace) from scalar divergence metrics (&Phi;, &beta;&#x2081;) to a full algebraic object:
the **coherence density matrix** &rho;. Every existing metric derives as a projection of &rho;.
The von Neumann entropy S(&rho;) is the universal coherence measure. The Bures metric
endows the space of coherence states with Riemannian geometry, enabling optimal transaction
selection (geodesic pursuit) and provably near-optimal seed construction (submodular
maximization with (1-1/e) guarantee).

**Traces to**: SEED.md &sect;1 (coherence verification), &sect;2 (divergence problem),
&sect;4 (datom algebra), &sect;5 (harvest/seed lifecycle), &sect;6 (reconciliation mechanisms),
&sect;7 (self-improvement loop)

**Source**: `exploration/coherence-geometry/00-density-matrix-coherence.md`,
`01-information-geometry-manifold.md`, `02-spectral-sequence-verification.md`

**Extends**: TRILATERAL (&Phi;, &beta;&#x2081; duality), BILATERAL (F(S) fitness),
QUERY (graph Laplacian, eigendecomposition), SEED (assembly optimization),
GUIDANCE (curvature-aware steering)

---

### &sect;20.1 Level 0: Algebraic Specification

#### Agreement Function

```
Given:
  Attribute a with resolution mode R_a (RESOLUTION namespace)
  Value domain V_a

The agreement function:
  agreement_a : V_a x V_a -> [0, 1]

  agreement_a(v1, v2) = match R_a {
      LWW      => delta(v1, v2)                     // Kronecker delta
      Lattice  => 1 - d_L(v1, v2) / diam(L)         // normalized lattice distance
      Multi    => |v1 cap v2| / |v1 cup v2|          // Jaccard similarity
  }

where:
  delta(v1, v2) = 1 if v1 = v2, 0 otherwise
  d_L(v1, v2) = shortest path in Hasse diagram of lattice L
  diam(L) = diameter of lattice L
```

#### Agreement Matrix

```
Given:
  Perspectives V = {alpha_1, ..., alpha_n}
  Entity-attribute pair (e, a)

Agreement matrix A(e,a) in R^{n x n}:
  A(e,a)_{ij} = agreement_a(view_{alpha_i}(e, a), view_{alpha_j}(e, a))

Properties (provable from agreement function definition):
  1. Symmetric: A_{ij} = A_{ji}                      (agreement is commutative)
  2. Unit diagonal: A_{ii} = 1                        (self-agreement)
  3. Positive semi-definite: A is a Gram matrix       (PSD)
  4. Trace: Tr(A) = n
```

#### Coherence Density Matrix

```
Per-attribute density matrix:
  rho(e,a) = A(e,a) / n

  Satisfies density matrix axioms:
    rho >= 0        (PSD, inherited from A)
    Tr(rho) = 1     (from Tr(A) = n)
    rho = rho^T     (symmetric, inherited from A)

Resolution-weighted total density matrix:
  rho_bar_w = (1/W) sum_{(e,a) in Omega} w(a) * rho(e,a)

  where W = sum_{(e,a)} w(a) is the normalizer

  Resolution weights (from INV-QUERY-023):
    w(a) = match resolution_mode(a) {
        Lattice  => 0.1     // auto-converges
        LWW      => 0.5     // converges after merge
        Multi    => 1.0     // requires deliberation
    }
```

#### Von Neumann Entropy

```
S(rho) = -Tr(rho * log(rho)) = -sum_i lambda_i * log(lambda_i)

where lambda_i are eigenvalues of rho, and 0 * log(0) := 0

Properties:
  1. S >= 0                                           (non-negativity)
  2. S = 0  iff  rho is pure (rank 1)                 (purity characterization)
  3. S <= log(n)  with equality iff rho = I/n          (maximum entropy)
  4. S is concave in rho                               (no local minima)
  5. S is continuous in rho                             (Fannes inequality)
  6. S(rho_AB) <= S(rho_A) + S(rho_B)                  (subadditivity)
```

#### Entropy Decomposition

```
S(rho_bar_w) = S_within + S_between

where:
  S_within  = (1/W) sum_{(e,a)} w(a) * S(rho(e,a))   (per-attribute entropy)
  S_between = S(rho_bar_w) - S_within                  (cross-attribute correlation)

By subadditivity: S_between >= 0
  S_between >> 0 implies correlated incoherence (cycles)
  S_between ~= 0 implies independent incoherence (fixable in any order)
```

#### ISP Triangle Specialization (Stage 0)

```
For Stage 0 (single agent), perspectives are LIVE views: {I, S, P}
The density matrix is 3x3:

rho_ISP = (1/3W) sum_{(e,a)} w(a) *
  [ 1           agree_IS(e,a)   agree_IP(e,a) ]
  [ agree_IS(e,a)   1           agree_SP(e,a) ]
  [ agree_IP(e,a)   agree_SP(e,a)   1         ]

Eigenvalues solvable analytically (cubic formula).
Von Neumann entropy has closed-form expression.
```

---

### &sect;20.2 Invariants

#### INV-COHERENCE-001: Density Matrix Axioms

**Traces to**: SEED.md &sect;4 (datom algebra, algebraic correctness)
**Type**: Invariant
**Statement**: For any store S and set of perspectives V, the coherence density
matrix &rho;&#x0304;_w satisfies the three density matrix axioms:
(1) &rho;&#x0304;_w &ge; 0 (positive semi-definite),
(2) Tr(&rho;&#x0304;_w) = 1 (unit trace),
(3) &rho;&#x0304;_w = &rho;&#x0304;_w^T (Hermitian/symmetric for real case).

**Falsification**: Any computation of &rho;&#x0304;_w that produces a matrix with a negative
eigenvalue, trace &ne; 1, or asymmetric entries violates this invariant.

**Verification**: Unit test constructing stores with known agreement structures,
computing &rho;&#x0304;_w, and asserting PSD (all eigenvalues &ge; -&epsilon;), |Tr(&rho;) - 1| < &epsilon;,
and &rho; = &rho;^T. Property test with arbitrary agreement matrices verifying
the construction preserves axioms.

---

#### INV-COHERENCE-002: Agreement-Resolution Consistency

**Traces to**: SEED.md &sect;4 (datom algebra), spec/04-resolution.md
**Type**: Invariant
**Statement**: For all resolution modes R_a:
agreement_a(v1, v2) = 1 &hArr; resolve_a(v1, v2) = v1 = v2.
Full agreement iff resolution is a no-op.

**Falsification**: Finding an attribute a and values v1, v2 where agreement_a(v1, v2) = 1
but resolve_a(v1, v2) &ne; v1, or where resolve_a(v1, v2) = v1 = v2 but
agreement_a(v1, v2) < 1, violates this invariant.

**Verification**: For each resolution mode (LWW, Lattice, Multi), enumerate representative
value pairs and assert bidirectional implication. Property test: random values under
each mode, verify agreement = 1 iff values are equal under resolution semantics.

---

#### INV-COHERENCE-003: Entropy Bounds

**Traces to**: SEED.md &sect;1 (coherence verification), &sect;2 (divergence problem)
**Type**: Invariant
**Statement**: For any store S with n perspectives:
(1) S(&rho;&#x0304;_w) &ge; 0,
(2) S(&rho;&#x0304;_w) = 0 &hArr; all perspectives agree on all weighted attributes (pure state),
(3) S(&rho;&#x0304;_w) &le; log(n).

**Falsification**: Computing S(&rho;) < 0, or S(&rho;) = 0 when there exist disagreements,
or S(&rho;) > log(n) + &epsilon;, violates this invariant.

**Verification**: Unit test with pure-state store (all agree) &rArr; S = 0. Unit test with
maximally mixed store (all disagree pairwise) &rArr; S = log(n). Property test with
random stores &rArr; S &isin; [0, log(n)].

---

#### INV-COHERENCE-004: Geodesic Pursuit Convergence

**Traces to**: SEED.md &sect;6 (reconciliation mechanisms), &sect;7 (self-improvement loop)
**Type**: Invariant
**Statement**: If at least one candidate transaction reduces the Bures distance
d_B(&rho;, &rho;_target), geodesic pursuit converges to the target in at most
&lceil;d_B(&rho;_0, &rho;_target)&sup2; / &delta;_min&sup2;&rceil; steps, where &delta;_min is the minimum
per-step distance reduction.

**Falsification**: A geodesic pursuit trace where the candidate selection function
correctly identifies the distance-minimizing transaction at each step, but
d_B does not decrease monotonically, violates this invariant.

**Verification**: Construct store with known Bures distance to a pure target. Apply
geodesic pursuit with synthetic transactions. Assert d_B decreases monotonically
and reaches &epsilon;-neighborhood of target within the step bound.

---

#### INV-COHERENCE-005: Bures Monotonicity Under Merge

**Traces to**: SEED.md &sect;6 (reconciliation), spec/07-merge.md
**Type**: Invariant
**Statement**: For any merge operation M (CRDT set union, INV-MERGE-001):
d_B(M(&rho;), &rho;_target) &le; d_B(&rho;, &rho;_target).
Merges never increase Bures distance to the coherence target.

**Falsification**: A merge operation that produces a post-merge density matrix farther
from the pure target than the pre-merge matrix violates this invariant.

**Verification**: Property test: random two-store merge, compute d_B before and after,
assert non-increase. Edge case: merge of identical stores (d_B unchanged).

---

#### INV-COHERENCE-006: Submodular Seed Guarantee

**Traces to**: SEED.md &sect;5 (harvest/seed lifecycle), spec/06-seed.md
**Type**: Invariant
**Statement**: The greedy seed assembly algorithm achieves fidelity
f(seed_greedy) &ge; (1 - 1/e) &middot; f(seed_optimal), where
f(&sigma;) = &langle;&psi;|&rho;(&sigma;)|&psi;&rangle; (fidelity with the pure target) and
seed_optimal is the cardinality-constrained optimum.

**Falsification**: A seed construction where greedy fidelity is less than 63.2% of
optimal fidelity violates this invariant.

**Verification**: Small stores (|S| &le; 20) where exhaustive search computes the true
optimum. Verify greedy achieves &ge; (1 - 1/e) &middot; optimal. Property test with
random stores, verify the greedy algorithm's fidelity satisfies the bound against
the exhaustive solution.

**Uncertainty**: Confidence 0.7. The bound assumes cardinality constraint (|seed| &le; k).
Token-budget constraint (knapsack) reduces the guarantee to (1-1/e)/2 unless the
cost-effective greedy variant is used. See OQ-2.

---

#### INV-COHERENCE-007: Phi as Rank Deficiency

**Traces to**: spec/18-trilateral.md INV-TRILATERAL-002
**Type**: Invariant
**Statement**: The divergence metric &Phi;(S) (INV-TRILATERAL-002) equals the count of
non-pure per-attribute density matrices on boundary entity-attribute pairs:
&Phi;(S) = |{(e,a) &isin; &Omega;_boundaries : rank(&rho;(e,a)) > 1}|.

**Falsification**: A store where the &Phi; count (boundary gap counting) disagrees with
the rank-deficiency count of per-attribute density matrices violates this invariant.

**Verification**: Compute &Phi; via existing INV-TRILATERAL-002 algorithm and via density
matrix rank counting. Assert equality on stores with varying gap structures.

---

#### INV-COHERENCE-008: Beta-1 as Entanglement Witness

**Traces to**: spec/18-trilateral.md INV-TRILATERAL-009, spec/03-query.md INV-QUERY-024
**Type**: Invariant
**Statement**: &beta;&#x2081;(G) > 0 &hArr; &exist; cycle C in G such that S_between(C) > 0,
where S_between(C) is the cross-edge entropy correlation around the cycle.
First Betti number detects correlated incoherence (entanglement).

**Falsification**: A store where &beta;&#x2081; > 0 but S_between = 0 for all cycles, or
&beta;&#x2081; = 0 but S_between > 0 for some cycle, violates this invariant.

**Verification**: Construct stores with known cycle structures. Verify &beta;&#x2081; and
S_between are consistently zero or non-zero. Property test: random graphs with
controlled cycle counts.

**Uncertainty**: Confidence 0.8. The formal proof (Theorem 4.2 in exploration/00)
relies on the relationship between subadditivity violation and cycle structure.
A rigorous proof connecting the averaged density matrix's entropy decomposition
to the graph's homology would strengthen the claim. See OQ-2 in exploration/00.

---

#### INV-COHERENCE-009: Spectral Sequence Convergence

**Traces to**: SEED.md &sect;1, spec/18-trilateral.md
**Type**: Invariant
**Statement**: For the three-level filtration F_0 &sube; F_1 &sube; F_2 = C*(S), the
spectral sequence converges at page E_3 = E_&infin;. The graded pieces of total
cohomology H*(C*(S)) are recoverable from E_3.

**Falsification**: A spectral sequence computation that fails to stabilize at page E_3
(i.e., E_3 &ne; E_4) violates this invariant.

**Verification**: Algebraic: for any length-L filtration, the spectral sequence
converges at page E_{L+1}. Here L = 2, so convergence at E_3. Unit test: construct
filtered complexes, verify E_3 = E_4.

---

#### INV-COHERENCE-010: E2 Degeneration Early Termination

**Traces to**: SEED.md &sect;1, spec/10-bilateral.md (AUTO_CYCLE)
**Type**: Invariant
**Statement**: If d_1 = 0 (all boundary maps vanish), then E_2 = E_1 and
H^q(C*(S)) = &oplus;_p E_1^{p,q}. Total cohomology is the direct sum of per-level
cohomologies. No cross-level computation is required.

**Falsification**: A store where d_1 = 0 but the total cohomology disagrees with the
direct sum of per-level cohomologies violates this invariant.

**Verification**: Construct stores with all boundary links consistent (d_1 = 0).
Compute total cohomology both directly and via per-level decomposition. Assert
equality.

---

#### INV-COHERENCE-011: Staged Coherence Reduces to (Phi, Beta-1) at Stage 0

**Traces to**: spec/18-trilateral.md INV-TRILATERAL-009
**Type**: Invariant
**Statement**: At Stage 0 (single agent, three LIVE views), the staged coherence
check reduces to: (1) d_1 computation = &Phi; (INV-TRILATERAL-002), (2) d_2
computation = ISP bypass check (INV-TRILATERAL-008). The spectral sequence
adds no new computation at Stage 0 &mdash; it formalizes the existing algorithm.

**Falsification**: A Stage 0 coherence check where the staged algorithm produces a
different result from the (&Phi;, &beta;&#x2081;) check violates this invariant.

**Verification**: Run both algorithms on the self-bootstrapped store. Assert identical
classification (Coherent, GapsOnly, GapsAndCycles).

---

#### INV-COHERENCE-012: CALM Stratification of Spectral Pages

**Traces to**: SEED.md &sect;6 (reconciliation), spec/08-sync.md
**Type**: Invariant
**Statement**: E_1 computation (per-level cohomology) is monotonic in the CALM
sense: it can be computed from local data without cross-level coordination.
d_1 and d_2 computations are non-monotonic: they require pairwise and global
coordination respectively. Each page transition requires a sync barrier.

**Falsification**: An E_1 computation that requires cross-level data, or a d_1
computation achievable from single-level data alone, violates this invariant.

**Verification**: Formal analysis of data dependencies. E_1 reads only F_p C* (single
level). d_1 reads F_{p+1} C* and F_p C* (two levels). d_2 reads all three levels.

---

#### INV-COHERENCE-013: Entropy Monotonicity Without Coordination

**Traces to**: SEED.md &sect;2 (divergence is natural state)
**Type**: Invariant
**Statement**: In the absence of coordinated action (merges, reviews, bilateral cycles),
coherence entropy is non-decreasing: S(&rho;(t+1)) &ge; S(&rho;(t)). This is the
specification-systems analogue of the second law of thermodynamics.

**Falsification**: Entropy decreasing in a store that has undergone no merge,
review, or bilateral cycle operation since the previous measurement.

**Verification**: Add uncoordinated transactions (new assertions without resolution)
to a store. Verify S non-decreasing. Apply a merge. Verify S may decrease.

**Uncertainty**: Confidence 0.7. The formal statement requires a precise definition
of "coordinated action" in the datom formalism. Currently defined operationally
(merges, reviews) rather than algebraically.

---

### &sect;20.3 Design Decisions

#### ADR-COHERENCE-001: Bures Distance as Canonical Coherence Metric

**Traces to**: SEED.md &sect;1, &sect;4
**Type**: ADR
**Problem**: Multiple distance metrics exist on the space of density matrices (trace distance,
Hilbert-Schmidt, Bures, Wasserstein). Which is the canonical metric for coherence?

**Decision**: Use the Bures distance d_B(&rho;, &sigma;) = &radic;(2(1 - F(&rho;, &sigma;))).

**Rationale**:
1. **Monotone under CPTP maps** (Property 5, &sect;20.1): merges never increase distance.
   This is INV-COHERENCE-005 &mdash; the most operationally important property.
2. **For pure targets, trivially computable**: d_B(&rho;, |&psi;&rangle;&langle;&psi;|) = &radic;(2(1 - (1/n)&Sigma;&rho;_{ij})).
   O(n&sup2;), microseconds for n &le; 10.
3. **Natural Riemannian structure**: geodesics give optimal transaction paths (INV-COHERENCE-004).
4. **Fidelity interpretation**: F(&rho;, &sigma;) is the maximum overlap between purifications.
   For coherence: fidelity = probability that the system "behaves coherently."

**Alternatives rejected**:
- Trace distance: monotone but no Riemannian structure.
- Hilbert-Schmidt: Riemannian but NOT monotone under CPTP (merges can increase distance).
- Wasserstein: requires a cost matrix on the state space; not canonical.

**Consequences**: Geodesic pursuit and seed optimization both use Bures. The metric diverges
near pure states (last bits of coherence cost exponentially more effort &mdash; &sect;20.1).

---

#### ADR-COHERENCE-002: Spectral Degeneration as Early Termination

**Traces to**: SEED.md &sect;10 (staged roadmap), spec/10-bilateral.md (AUTO_CYCLE)
**Type**: ADR
**Problem**: Full coherence verification examines all entity-attribute pairs across all views.
For well-maintained stores, most of this work is redundant. How to skip unnecessary computation?

**Decision**: Use the spectral sequence degeneration criterion: if per-level cohomology
(E_1) passes and boundary maps (d_1) vanish, total coherence follows without cross-level
computation. The staged check terminates early at E_2 degeneration.

**Rationale**:
1. **Formal correctness**: the degeneration theorem is a standard result in homological algebra.
   E_2 degeneration &rArr; total cohomology = direct sum of per-level cohomologies.
2. **CALM alignment**: per-level checks are monotonic (parallelizable), cross-level checks are
   non-monotonic (require coordination). Early termination avoids non-monotonic computation
   when possible.
3. **Backward compatibility**: at Stage 0, the staged check reduces to (&Phi;, &beta;&#x2081;)
   (INV-COHERENCE-011). No behavioral change for existing code.

**Alternatives rejected**:
- Always compute full coherence: wasteful for well-maintained stores.
- Heuristic skip conditions: no correctness guarantee.

**Consequences**: The bilateral loop's AUTO_CYCLE maps to "compute through E_2, stop if
degeneration." FULL_CYCLE maps to "compute through E_3 with human verification."

---

#### ADR-COHERENCE-003: F(S) Decomposition into State and Process Components

**Traces to**: spec/10-bilateral.md (fitness function), SEED.md &sect;7
**Type**: ADR
**Problem**: The current fitness function F(S) = 0.18V + 0.18C + ... mixes store-state
metrics (derivable from &rho;) with process metrics (harvest quality, uncertainty management).
This conflates "is the store coherent?" with "is the process working?"

**Decision**: Decompose F(S) into two orthogonal components:
F(S) = w_state &middot; (1 - S(&rho;&#x0304;_w)/log(n)) + w_process &middot; process_score.
Store-state component from &rho;, process component from methodology adherence.

**Rationale**: Separation enables independent optimization. A coherent store achieved
through bad process is fragile. Good process on an incoherent store is effective
but incomplete. Diagnosing which component is deficient requires separation.

**Alternatives rejected**:
- Keep monolithic F(S): loses diagnostic power.
- Replace F(S) entirely with S(&rho;): loses process quality signal.

**Consequences**: The bilateral loop can focus on the deficient component. Replaces
7 hand-weighted dimensions with one principled formula (entropy) plus one process score.

**Uncertainty**: Confidence 0.6. Whether the decomposition produces the same optimization
dynamics as the monolithic F(S) requires empirical validation during Stage 1.

---

### &sect;20.4 Negative Cases

#### NEG-COHERENCE-001: Non-PSD Agreement Matrix

**Traces to**: INV-COHERENCE-001
**Type**: Negative case
**Statement**: A computation of &rho;(e,a) that produces a matrix with a negative eigenvalue
is a defect in the agreement function implementation.

**Violation condition**: eigenmin(&rho;) < -&epsilon; where &epsilon; = 1e-12 (numerical tolerance).

**Required response**: Assert and fail. Log the entity-attribute pair and agreement values.
The agreement function for the attribute's resolution mode has a bug.

---

#### NEG-COHERENCE-002: Unbounded Entropy

**Traces to**: INV-COHERENCE-003
**Type**: Negative case
**Statement**: Von Neumann entropy exceeding log(n) indicates a mathematical error in the
eigenvalue computation or entropy formula.

**Violation condition**: S(&rho;) > log(n) + &epsilon; where &epsilon; = 1e-10.

**Required response**: Assert and fail. Dump eigenvalue spectrum for diagnosis.

---

#### NEG-COHERENCE-003: False Convergence

**Traces to**: INV-COHERENCE-004, INV-COHERENCE-005
**Type**: Negative case
**Statement**: A geodesic pursuit step that claims to reduce Bures distance but actually
increases it is a false convergence signal. This can occur if the transaction
application function incorrectly updates the density matrix.

**Violation condition**: d_B(&rho;_{after}, &rho;_target) > d_B(&rho;_{before}, &rho;_target) + &epsilon;
after a step that was selected as distance-reducing.

**Required response**: Revert the transaction. Log the before/after density matrices.
The apply_transaction function has a bug.

---

### &sect;20.5 Open Questions

#### OQ-COHERENCE-001: Cross-Type Boundary Agreement (Confidence: 0.7)

The :spec/traces-to attribute uses String values referencing SEED.md sections, while
:spec/implements uses Ref values pointing to entity IDs. Agreement across the ISP
boundary compares values in DIFFERENT type domains. Currently handled by checking
link existence (INV-TRILATERAL-008), not content agreement.

**What would resolve it**: Stage 0 implementation revealing cases where link existence
is insufficient and content agreement is needed.

#### OQ-COHERENCE-002: Tensor Product vs Average (Confidence: 0.8)

The averaged density matrix &rho;&#x0304;_w loses per-attribute correlation structure.
Whether S_between faithfully captures what &beta;&#x2081; detects (Theorem 4.2 in
exploration/00) requires either constructing an explicit counterexample store
where S_between = 0 but &beta;&#x2081; > 0, or proving no such store exists.

**What would resolve it**: Mathematical proof or counterexample.

#### OQ-COHERENCE-003: Curvature Computation Cost (Confidence: 0.8)

Full Riemann curvature tensor is O(n&sup8;). For Stage 0 (n=3), analytically solvable.
For Stage 2+ (n &le; 10), sectional curvature approximation O(n&sup4;) &asymp; 10&sup4; is fast.
Full curvature tensor only for advanced diagnostics.

**What would resolve it**: Stage 2 implementation with n &ge; 5.

#### OQ-COHERENCE-004: CPTP Model for Non-Merge Transactions (Confidence: 0.6)

Merge operations are naturally CPTP. But new assertions and retractions may change the
effective dimension of the state space. The convergence rate analysis assumes all
operations are CPTP. Extending to dimension-changing operations may require working in
a direct limit of finite-dimensional manifolds.

**What would resolve it**: Formal extension of the CPTP model to handle dimension changes.

#### OQ-COHERENCE-005: Persistent Spectral Sequences (Confidence: 0.5)

The spectral sequence at each transaction gives a coherence snapshot. The sequence of
spectral sequences over the transaction filtration is a persistent spectral sequence.
Whether this gives useful diagnostics beyond persistent H&sup1; alone is unclear.

**What would resolve it**: Stage 2 implementation with enough transaction history.

---

### &sect;20.6 Thermodynamic Interpretation

The density matrix formalism admits a thermodynamic reading:

| Physical Concept | DDIS Analogue | Formal Object |
|---|---|---|
| Entropy | Incoherence measure | S(&rho;) = -Tr(&rho; log &rho;) |
| Temperature | Difficulty of resolution | 1/&beta; where &beta; = -&part;S/&part;E |
| Free energy | Work remaining | F = S(&rho;_current) - S(&rho;_target) |
| Negentropy production | Productive work rate | N(t) = -dS/dt |
| Phase transition | Qualitative coherence change | |&Delta;S| > threshold per tx |
| Second law | Divergence is natural | S(t+1) &ge; S(t) without coordination |

**INV-COHERENCE-013** formalizes the second law. Maintaining coherence requires active
negentropy production &mdash; deliberate effort (merges, reviews, bilateral cycles) that
reduces entropy faster than the natural tendency toward disorder produces it.

Phase transitions (first-order: sudden jump in S; second-order: discontinuous dS/dt)
are detectable and should trigger signals (spec/09-signal.md):

```
SIGNAL_PHASE_TRANSITION: {
    type: FirstOrder | SecondOrder,
    delta_S: f64,
    affected_boundary: IS | SP | IP,
    triggering_tx: TxId,
}
```

---

### &sect;20.7 Unification Table

Every existing coherence metric derives from the density matrix:

| Existing Metric | Density Matrix Derivation | Reference |
|---|---|---|
| &Phi; (divergence) | Count of non-pure per-boundary &rho;(e,a) | INV-COHERENCE-007 |
| &beta;&#x2081; (cycles) | Entanglement witness (S_between > 0) | INV-COHERENCE-008 |
| F(S) (fitness) | w_state &middot; (1 - S/log n) + w_process &middot; P | ADR-COHERENCE-003 |
| F(T) (topology) | Spectral gap of merge superoperator | &sect;20.1 |
| Persistence diagram | Eigenvalue evolution of &rho;(S_t) over tx filtration | &sect;20.6 |
| Coherence quadrant | rank(&rho;) = 1 &hArr; pure &hArr; Coherent | INV-COHERENCE-001 |

---

### &sect;20.8 Implementation Path

#### Stage 1: ISP Triangle (3&times;3)

```rust
// All computation uses nalgebra. No new dependencies.
pub fn agreement(v1: &Value, v2: &Value, mode: ResolutionMode) -> f64;
pub fn isp_density_matrix(store: &Store) -> DMatrix<f64>;  // 3x3
pub fn von_neumann_entropy(rho: &DMatrix<f64>) -> f64;
pub fn bures_distance_to_pure(rho: &DMatrix<f64>) -> f64;
pub fn normalized_coherence(rho: &DMatrix<f64>) -> f64;    // 1 - S/log(n)
```

#### Stage 2+: N-Agent (n&times;n)

```rust
pub fn n_agent_density_matrix(store: &Store, agents: &[AgentId]) -> DMatrix<f64>;
pub fn staged_coherence_check(store: &Store) -> StageResult;
pub fn geodesic_pursuit(store: &Store, candidates: &[Transaction]) -> Option<Transaction>;
pub fn greedy_seed(store: &Store, task: &str, budget: usize) -> Seed;
```

Performance: eigendecomposition is O(n&sup3;). For n &le; 10, all operations complete in
microseconds. The existing SLQ (Stochastic Lanczos Quadrature) in trilateral.rs
provides fallback for large n.

---

### &sect;20.9 Cross-Reference Summary

| This Spec Element | References | Nature |
|---|---|---|
| INV-COHERENCE-001 | INV-STORE-001, INV-STORE-012 | Extends (store &rarr; density matrix) |
| INV-COHERENCE-002 | INV-RESOLUTION-001..003 | Derives (resolution mode &rarr; agreement) |
| INV-COHERENCE-005 | INV-MERGE-001, INV-TRILATERAL-004 | Generalizes (merge monotonicity) |
| INV-COHERENCE-006 | INV-SEED-001..002 | Extends (seed &rarr; optimal seed) |
| INV-COHERENCE-007 | INV-TRILATERAL-002 | Reinterprets (&Phi; as rank deficiency) |
| INV-COHERENCE-008 | INV-TRILATERAL-009, INV-QUERY-024 | Reinterprets (&beta;&#x2081; as entanglement) |
| INV-COHERENCE-009..012 | INV-TRILATERAL-008, INV-BILATERAL-002 | Formalizes (spectral sequence) |
| INV-COHERENCE-013 | SEED.md &sect;2 | Formalizes (second law) |
| ADR-COHERENCE-001 | INV-TRILATERAL-004 | Extends (convergence &rarr; metric) |
| ADR-COHERENCE-002 | INV-BILATERAL-002 (AUTO/FULL_CYCLE) | Formalizes (when to stop) |
| ADR-COHERENCE-003 | INV-BILATERAL-001 (F(S)) | Restructures (monolithic &rarr; decomposed) |
