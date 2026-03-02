> **Namespace**: BILATERAL | **Wave**: 3 (Intelligence) | **Stage**: 2
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §10. BILATERAL — Bilateral Feedback Loop

> **Purpose**: The bilateral loop is the convergence mechanism — it continuously checks
> alignment between specification and implementation in both directions until the gap
> between them reaches zero (or an explicitly documented residual).
>
> **Traces to**: SEED.md §3 (Bilateral feedback loop), §6 (Reconciliation Mechanisms),
> ADRS CO-004, CO-008, CO-009, CO-010, SQ-006, AS-006

### §10.1 Level 0: Algebraic Specification

The bilateral loop is an **adjunction** between forward and backward projections:

```
Forward:  F : Spec → ImplStatus     — does the implementation satisfy the spec?
Backward: B : Impl → SpecAlignment  — does the spec accurately describe the implementation?

The loop is the composition: (B ∘ F) applied repeatedly until fixpoint.
```

**Divergence measure** over the four-boundary chain (CO-010):

```
D(spec, impl) = Σᵢ wᵢ × |boundary_gap(i)|

where boundaries are:
  i=1: Intent → Spec       (axiological gap)
  i=2: Spec → Spec         (logical gap — contradictions)
  i=3: Spec → Impl         (structural gap)
  i=4: Impl → Behavior     (behavioral gap)
```

**Laws**:
- **L1 (Monotonic convergence)**: `D(spec', impl') ≤ D(spec, impl)` after each bilateral cycle — total divergence never increases
- **L2 (Fixpoint existence)**: The loop terminates when `D(spec, impl) = 0` or when all remaining divergence is explicitly documented as residual
- **L3 (Bilateral symmetry)**: Forward and backward checks use the same Datalog query apparatus (SQ-006)

**Fitness function** (CO-009):
```
F(S) = 0.18×V + 0.18×C + 0.18×(1-D) + 0.13×H + 0.13×(1-K) + 0.08×(1-I) + 0.12×(1-U)

where:
  V = validation score (invariants verified / total)
  C = coverage (goals traced to invariants and back)
  D = drift (spec-impl divergence)
  H = harvest quality (FP/FN rates)
  K = contradictions (weighted by severity)
  I = incompleteness (gaps between spec and impl)
  U = mean uncertainty
```

Target: `F(S) → 1.0`

### §10.2 Level 1: State Machine Specification

**State**: `Σ_bilateral = (divergence_map: Map<Boundary, Set<Gap>>, fitness: f64, cycle_count: u64, residuals: Set<DocumentedResidual>)`

**Transitions**:

```
FORWARD_SCAN(Σ, spec, impl) → Σ' where:
  POST: Σ'.divergence_map[SpecToImpl] = detected structural gaps
  POST: for each gap: emit Signal(type=BranchReady or GoalDrift)

BACKWARD_SCAN(Σ, impl, spec) → Σ' where:
  POST: Σ'.divergence_map[ImplToSpec] = detected spec inaccuracies
  POST: for each inaccuracy: emit Signal(type=GoalDrift)

COMPUTE_FITNESS(Σ) → Σ' where:
  POST: Σ'.fitness = F(S) computed from current state
  POST: fitness value recorded as datom

DOCUMENT_RESIDUAL(Σ, gap, rationale) → Σ' where:
  PRE:  gap ∈ Σ.divergence_map[any]
  POST: gap moved from divergence_map to residuals
  POST: rationale recorded with uncertainty marker

CYCLE(Σ, spec, impl) → Σ' where:
  POST: FORWARD_SCAN then BACKWARD_SCAN then COMPUTE_FITNESS
  POST: Σ'.cycle_count = Σ.cycle_count + 1
  INV:  Σ'.fitness ≥ Σ.fitness (monotonic convergence)
```

**Query layer bilateral structure** (SQ-006):

Forward-flow queries (planning):
- Epistemic uncertainty: what does the system not know?
- Crystallization candidates: what is stable enough to commit?
- Delegation: who should work on what?

Backward-flow queries (assessment):
- Conflict detection: where do agents disagree?
- Drift candidates: where has implementation departed from spec?
- Absorption triggers: what implementation patterns should update the spec?

Bridge queries (both):
- Commitment weight: how costly is changing this decision?
- Spectral authority: who has demonstrated competence here?

### §10.3 Level 2: Implementation Contract

```rust
pub struct BilateralLoop {
    pub divergence_map: HashMap<Boundary, Vec<Gap>>,
    pub fitness: f64,
    pub cycle_count: u64,
    pub residuals: Vec<DocumentedResidual>,
}

#[derive(Clone, Debug)]
pub enum Boundary {
    IntentToSpec,
    SpecToSpec,
    SpecToImpl,
    ImplToBehavior,
}

pub struct Gap {
    pub boundary: Boundary,
    pub source: EntityId,
    pub target: Option<EntityId>,
    pub severity: Severity,
    pub description: String,
}

impl BilateralLoop {
    /// Run one complete bilateral cycle
    pub fn cycle(&mut self, store: &mut Store) -> CycleReport {
        let forward = self.forward_scan(store);
        let backward = self.backward_scan(store);
        let fitness = self.compute_fitness(store);
        CycleReport { forward, backward, fitness, cycle: self.cycle_count }
    }
}
```

### §10.4 Invariants

### INV-BILATERAL-001: Monotonic Convergence

**Traces to**: ADRS CO-004
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 1

#### Level 0 (Algebraic Law)
`∀ cycle n: F(S_{n+1}) ≥ F(S_n)`
The fitness function never decreases across bilateral cycles. Each cycle either
reduces divergence or documents residual — both are non-decreasing fitness operations.

#### Level 1 (State Invariant)
For all reachable states (Σ, Σ') where Σ →[CYCLE] Σ':
`Σ'.fitness ≥ Σ.fitness`

#### Level 2 (Implementation Contract)
```rust
#[kani::ensures(|report| report.fitness >= old(self.fitness))]
fn cycle(&mut self, store: &mut Store) -> CycleReport { ... }
```

**Falsification**: A bilateral cycle produces a lower fitness score than the previous cycle.

**proptest strategy**: Run random sequences of bilateral cycles with random
spec/impl states. Verify fitness is monotonically non-decreasing.

**Stateright model**: 2 agents, 1 spec, 1 impl. Run bilateral cycles.
Verify fitness monotonicity across all reachable states.

---

### INV-BILATERAL-002: Five-Point Coherence Statement

**Traces to**: ADRS CO-008
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
The bilateral loop checks five coherence conditions:
```
C1: ¬∃ contradiction in spec         (spec self-consistency)
C2: impl ⊨ spec                      (impl satisfies spec)
C3: spec ≈ intent                     (spec matches intent)
C4: ∀ agents α,β: store_α ∪ store_β converges  (agent agreement)
C5: agent_behavior ⊨ methodology      (process adherence)
```

Full coherence: `C1 ∧ C2 ∧ C3 ∧ C4 ∧ C5`

#### Level 1 (State Invariant)
Each CYCLE evaluates all five conditions. The divergence map partitions gaps
by which coherence condition they violate.

**Falsification**: A bilateral cycle evaluates fewer than five conditions,
leaving a coherence dimension unchecked.

---

### INV-BILATERAL-003: Bilateral Symmetry

**Traces to**: ADRS SQ-006, AS-006
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
Forward and backward scans use the same Datalog query apparatus.
The branching mechanism (AS-006) works identically in both directions:
forward (spec → competing implementations → selection) and backward
(implementation → competing spec updates → selection).

#### Level 1 (State Invariant)
For all reachable states, the forward and backward scans produce gap types
drawn from the same type set, using the same query engine, stored as the
same datom types. No structural asymmetry exists between directions.

**Falsification**: The system supports branching for competing implementations
but requires linear spec modifications (or vice versa).

---

### INV-BILATERAL-004: Residual Documentation

**Traces to**: SEED §6 (explicitly acknowledged residual)
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
Every gap that persists beyond a bilateral cycle is either:
(a) resolved in the next cycle, or
(b) documented as a residual with uncertainty marker and rationale.

No gap persists undocumented.

#### Level 1 (State Invariant)
`∀ gap ∈ divergence_map: age(gap) > 1 cycle ⟹ gap ∈ residuals ∨ gap resolved`

**Falsification**: A gap appears in the divergence map for two consecutive cycles
without being either resolved or documented as a residual.

---

### INV-BILATERAL-005: Test Results as Datoms

**Traces to**: ADRS CO-011
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
Test outcomes are datoms in the store:
`test_passed(X, frontier_F) ⟺ ∃ d ∈ S: d.a = :test/result ∧ d.v = :passed ∧ d.e = X`

This extends the bilateral loop to the Impl→Behavior boundary.

#### Level 1 (State Invariant)
After any test execution, the result (pass/fail, error, frontier) is transacted
into the store as a datom with entity type `:test-result/*`.

**Falsification**: A test runs but its result is not in the store.

---

### §10.5 ADRs

### ADR-BILATERAL-001: Fitness Function Weights

**Traces to**: ADRS CO-009
**Stage**: 1

#### Problem
How should the fitness function weight its seven components?

#### Decision
Weights from CO-009: V=0.18, C=0.18, D=0.18, H=0.13, K=0.13, I=0.08, U=0.12.
Validation, coverage, and drift weighted equally (primary triad). Harvest and
contradiction weighted equally (secondary). Incompleteness lowest (subsumes coverage).
Uncertainty moderate (important coordination metric).

#### Formal Justification
The primary triad (V,C,D) directly measures the spec↔impl correspondence.
The secondary pair (H,K) measures methodology health. Incompleteness is partially
redundant with coverage. Uncertainty is actionable but not a defect per se.

**Uncertainty**: UNC-BILATERAL-001 — weights are theoretical. Empirical calibration
during Stage 0 may revise them. Confidence: 0.6.

---

### ADR-BILATERAL-002: Divergence Metric as Weighted Boundary Sum

**Traces to**: ADRS CO-010
**Stage**: 1

#### Problem
How should total divergence be quantified across the four-boundary chain?

#### Decision
`D(spec, impl) = Σᵢ wᵢ × |boundary_gap(i)|` where boundary weights
reflect the cost of divergence at each boundary. Default: equal weights.

#### Formal Justification
Each boundary contributes independently to total divergence. Weighted sum
is the simplest combination that captures per-boundary severity while
remaining decomposable for targeted remediation.

**Uncertainty**: UNC-BILATERAL-002 — boundary weights may need per-project tuning.
Confidence: 0.5.

---

### ADR-BILATERAL-003: Intent Validation as Periodic Session

**Traces to**: ADRS CO-012
**Stage**: 2

#### Problem
How is the Intent→Spec boundary checked?

#### Decision
Periodic intent validation sessions where the system assembles current spec state
for human review: "Does this still describe what I want?" The human's response
is a datom — either confirming alignment or asserting axiological divergence.

#### Formal Justification
The Intent→Spec boundary uniquely requires human judgment. No automated mechanism
can verify that a specification captures intent (this is the fundamental
limitation — intent exists outside the formal system). Periodic sessions
with structured output make this otherwise invisible boundary checkable.

---

### §10.6 Negative Cases

### NEG-BILATERAL-001: No Fitness Regression

**Traces to**: ADRS CO-004
**Verification**: `V:PROP`, `V:MODEL`

**Safety property**: `□ ¬(F(S_{n+1}) < F(S_n))`
No bilateral cycle may reduce the fitness score.

**proptest strategy**: Run 1000 random bilateral cycles. Verify strict
monotonicity of the fitness sequence.

**Stateright model**: Verify across all reachable states of a
2-agent, 10-invariant model.

---

### NEG-BILATERAL-002: No Unchecked Coherence Dimension

**Traces to**: ADRS CO-008
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ cycle that skips any of C1–C5)`
Every bilateral cycle must evaluate all five coherence conditions.

**proptest strategy**: Instrument cycle execution. Verify all five checks
execute for every cycle invocation.

---

