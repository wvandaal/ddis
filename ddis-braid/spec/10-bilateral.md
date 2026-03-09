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

#### F(S) Component Mapping

| Symbol | SEED.md Name | Description | Weight |
|--------|-------------|-------------|--------|
| V | coverage | Invariants verified / total | 0.18 |
| C | coherence | Goals traced to invariants and back | 0.18 |
| D | drift | Spec-impl divergence | 0.18 |
| H | completeness | Harvest quality (FP/FN rates) | 0.13 |
| K | commitment | Contradictions weighted by severity | 0.13 |
| I | formality | Incompleteness gaps | 0.08 |
| U | certainty | Mean uncertainty | 0.12 |

### §10.2 Level 1: State Machine Specification

**State**: `Σ_bilateral = (divergence_map: Map<Boundary, Set<Gap>>, fitness: f64, cycle_count: u64, residuals: Set<DocumentedResidual>)`

**Transitions**:

```
FORWARD_SCAN(Σ, spec, impl) → Σ' where:
  POST: Σ'.divergence_map[SpecToImpl] = detected structural gaps
  POST: for each gap: emit Signal(type=BranchReady or GoalDrift)
  -- NOTE: At Stage 1, only Confusion signal available (INV-SIGNAL-002).
  -- BranchReady and GoalDrift require Stage 3 signal infrastructure.
  -- Stage 1 implementation: record divergence as datom; defer signal emission.

BACKWARD_SCAN(Σ, impl, spec) → Σ' where:
  POST: Σ'.divergence_map[ImplToSpec] = detected spec inaccuracies
  POST: for each inaccuracy: emit Signal(type=GoalDrift)
  -- NOTE: At Stage 1, only Confusion signal available (INV-SIGNAL-002).
  -- GoalDrift requires Stage 3 signal infrastructure.
  -- Stage 1 implementation: record divergence as datom; defer signal emission.

COMPUTE_FITNESS(Σ) → Σ' where:
  POST: Σ'.fitness = F(S) computed from current state
  POST: fitness value recorded as datom

DOCUMENT_RESIDUAL(Σ, gap, rationale) → Σ' where:
  PRE:  gap ∈ Σ.divergence_map[any]
  POST: gap moved from divergence_map to residuals
  POST: rationale recorded with uncertainty marker

AUTO_CYCLE(Σ, spec, impl) → Σ' where:
  POST: FORWARD_SCAN then BACKWARD_SCAN then COMPUTE_FITNESS
  POST: evaluates CC-1, CC-2, CC-4, CC-5 (machine-evaluable coherence conditions)
  POST: CC-3 carried forward from last FULL_CYCLE (INV-BILATERAL-002)
  POST: Σ'.cycle_count = Σ.cycle_count + 1
  INV:  Σ'.fitness ≥ Σ.fitness (monotonic convergence)

FULL_CYCLE(Σ, spec, impl, human_session) → Σ' where:
  POST: FORWARD_SCAN then BACKWARD_SCAN then COMPUTE_FITNESS
  POST: evaluates all five coherence conditions CC-1 through CC-5
  POST: CC-3 (axiological alignment) evaluated via human session (ADR-BILATERAL-003)
  POST: Σ'.cycle_count = Σ.cycle_count + 1
  POST: Σ'.last_full_cycle = Σ'.cycle_count
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
The bilateral loop checks five coherence conditions (CC-1 through CC-5):
```
CC-1: ¬∃ contradiction in spec         (spec self-consistency)       — machine-evaluable
CC-2: impl ⊨ spec                      (impl satisfies spec)        — machine-evaluable
CC-3: spec ≈ intent                     (spec matches intent)        — human-gated (ADR-BILATERAL-003)
CC-4: ∀ agents α,β: store_α ∪ store_β = store_β ∪ store_α  (agent agreement)  — machine-evaluable
CC-5: agent_behavior ⊨ methodology      (process adherence)         — machine-evaluable
```

Full coherence: `CC-1 ∧ CC-2 ∧ CC-3 ∧ CC-4 ∧ CC-5`

Two cycle modes (resolves tension with ADR-BILATERAL-003):
- **AUTO_CYCLE**: evaluates CC-1, CC-2, CC-4, CC-5 (machine-evaluable). CC-3 is carried
  forward from the last FULL_CYCLE. Stale after `cc3_staleness_threshold` cycles
  (default: 10) without a FULL_CYCLE.
- **FULL_CYCLE**: evaluates all five conditions including CC-3 via human session.

#### Level 1 (State Invariant)
Each AUTO_CYCLE evaluates the four machine-evaluable conditions. CC-3 is carried
forward from the most recent FULL_CYCLE. The divergence map partitions gaps by
which coherence condition they violate. When cycles since the last FULL_CYCLE
exceed `cc3_staleness_threshold`, the system emits a CC3StaleWarning signal
requesting a human intent-validation session.

Each FULL_CYCLE evaluates all five conditions, including CC-3 via a human
intent-validation session (ADR-BILATERAL-003). The CC-3 result is recorded as
a datom and carried forward in subsequent AUTO_CYCLEs.

**Falsification**: An AUTO_CYCLE evaluates fewer than four machine-evaluable
conditions (CC-1, CC-2, CC-4, CC-5), OR a FULL_CYCLE evaluates fewer than all five
conditions, OR CC-3 is carried forward beyond `cc3_staleness_threshold` cycles
without emitting CC3StaleWarning.

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
How is the Intent→Spec boundary (coherence condition CC-3) checked? CC-3 requires
human judgment, but INV-BILATERAL-002 requires all coherence conditions to be
evaluated in every cycle.

#### Decision
Periodic intent validation sessions where the system assembles current spec state
for human review: "Does this still describe what I want?" The human's response
is a datom — either confirming alignment or asserting axiological divergence.

This is reconciled with INV-BILATERAL-002 via two cycle modes:
- **AUTO_CYCLE** evaluates CC-1, CC-2, CC-4, CC-5 (machine-evaluable) and carries CC-3
  forward from the most recent FULL_CYCLE.
- **FULL_CYCLE** evaluates all five including CC-3 via a human session.

CC-3 goes stale after `cc3_staleness_threshold` AUTO_CYCLEs (default: 10) without
a FULL_CYCLE, triggering a CC3StaleWarning signal.

#### Formal Justification
The Intent→Spec boundary uniquely requires human judgment. No automated mechanism
can verify that a specification captures intent (this is the fundamental
limitation — intent exists outside the formal system). Periodic sessions
with structured output make this otherwise invisible boundary checkable.

The two-mode split resolves the tension between "every cycle checks everything"
(INV-BILATERAL-002) and "CC-3 requires human sessions" (this ADR): AUTO_CYCLEs
carry forward the last CC-3 result, bounding staleness via the configurable
threshold. This is analogous to caching a human judgment with a TTL.

---

### ADR-BILATERAL-004: Bilateral Authority Principle

**Traces to**: SEED §6, ADRS PD-006
**Stage**: 2

#### Problem
How does authority flow in the bilateral loop? If authority is static (e.g., human
always overrides agent), the system cannot leverage agent-discovered divergence.
If authority is unconstrained, agents may override human intent.

#### Options
A) **Top-down only** — human specifies, agents implement, authority is structural and one-directional.
B) **Delegation inversion** — agents surface divergence upward, but authority flows only backward (from agent to human).
C) **Bilateral authority** — forward and backward flows of authority, with fixpoint termination and emergent authority derived from the contribution graph.

#### Decision
**Option C.** Authority flows bilaterally: (1) Forward — human initiates exploration;
as findings stabilize, work flows outward to agents as uncertainty decreases.
(2) Backward — agents surface divergence they cannot resolve; as contradiction
severity increases, resolution flows inward toward agents/humans with broader
context. (3) The bilateral loop reaches fixpoint when forward and backward flows
produce no further changes. (4) Authority is emergent from the contribution graph
(UA-003), not structural — an agent that has demonstrated competence in a domain
carries more weight than one that has not. (5) Coordination topology emerges from
spectral structure rather than being imposed.

#### Formal Justification
Top-down authority (Option A) ignores information that agents discover during
implementation — structural divergence detected by agents cannot flow back to
improve the specification. Backward-only delegation (Option B) captures only
half the picture: it was the original "Delegation Inversion" concept which was
retracted because it modeled only the backward flow. Bilateral authority
preserves both information flows and terminates at fixpoint (L2: the loop
terminates when `D(spec, impl) = 0` or all residual is documented). The
emergent authority model prevents credential-based authority from overriding
evidence-based competence.

#### Consequences
- The system must track contribution provenance to compute emergent authority
- Fixpoint detection requires measuring whether forward and backward flows produce changes
- No static role hierarchy — authority is a dynamic, per-domain function of the contribution graph
- Coordination topology is a derived quantity, not a configuration parameter

#### Falsification
Authority is determined by a static role hierarchy rather than by contribution
graph analysis, OR the forward flow operates but the backward flow is suppressed
(agents cannot surface divergence that modifies the specification), OR the loop
never reaches fixpoint (no termination condition).

---

### ADR-BILATERAL-005: Reconciliation Taxonomy — Detect-Classify-Resolve

**Traces to**: SEED §6, ADRS LM-004
**Stage**: 1

#### Problem
How should the diversity of reconciliation operations be organized? Different
operations (harvest, merge, drift detection, contradiction resolution) seem
unrelated, risking ad-hoc proliferation of distinct mechanisms.

#### Options
A) **Per-operation design** — each reconciliation operation designed independently with its own semantics.
B) **Single universal reconciliation** — one algorithm that handles all divergence types.
C) **Taxonomy with shared structure** — all operations are instances of a single pattern (detect divergence, classify it, resolve it), but with type-specific behavior at each step.

#### Decision
**Option C.** All protocol operations are instances of one fundamental operation:
detect divergence, classify it, resolve it back to coherence. Eight divergence
types are identified: epistemic, structural, consequential, aleatory, logical,
axiological, temporal, and procedural. Each type has a specific detection
mechanism, classification rule, and resolution strategy — but all share the
three-phase structure.

#### Formal Justification
Per-operation design (Option A) produces a fragmented system where adding a new
reconciliation need requires designing a new mechanism from scratch. A single
universal algorithm (Option B) cannot capture the semantic differences between,
say, epistemic divergence (agent doesn't know a fact) and axiological divergence
(implementation contradicts goals). The taxonomy preserves the structural unity
(all operations are detect-classify-resolve) while permitting type-specific
behavior. New divergence types extend the taxonomy without redesigning existing
mechanisms.

#### Consequences
- Every reconciliation operation maps to exactly one divergence type
- Detection, classification, and resolution are independently implementable per type
- The eight types form a closed set at current understanding but are extensible (ADR-BILATERAL-010)
- Each type maps to specific Datalog queries for detection and specific resolution strategies

#### Falsification
A reconciliation operation is implemented that does not follow the detect-classify-resolve
pattern, OR a divergence is encountered that cannot be classified into any of the
eight types and the taxonomy provides no extension mechanism.

---

### ADR-BILATERAL-006: Coherence Verification as Fundamental Problem

**Traces to**: SEED §1, SEED §2, ADRS CO-001
**Stage**: 0

#### Problem
What is the fundamental problem that DDIS solves? Without a precise framing,
the system risks being designed as a solution to the wrong problem.

#### Options
A) **Memory system** — DDIS solves the memory problem (LLM context loss across sessions).
B) **Documentation system** — DDIS maintains accurate documentation.
C) **Coherence verification system** — DDIS maintains verifiable non-divergence between intent, specification, implementation, and observed behavior.

#### Decision
**Option C.** DDIS solves coherence verification — maintaining verifiable
non-divergence between intent, specification, implementation, and observed
behavior. The memory problem is the presenting symptom; divergence is the deeper
disease. Memory loss is one cause of epistemic divergence, but there are seven
other divergence types. A system that solves only memory would miss structural,
logical, axiological, and other divergence categories entirely.

#### Formal Justification
Framing hierarchy: coherence leads, memory is subordinated as one mechanism
within the epistemic divergence type. A memory system (Option A) would store
and retrieve facts but not verify that those facts are mutually consistent,
that they align with implementation, or that implementation matches intent.
A documentation system (Option B) addresses only the spec→impl boundary.
Coherence verification subsumes both: it requires storage (memory), documentation
(specification), and additionally checks all four boundaries (intent→spec,
spec→spec, spec→impl, impl→behavior).

#### Consequences
- The system is designed around divergence detection, not around storage or retrieval
- Success is measured by convergence (F(S) → 1.0), not by recall or storage capacity
- Every feature must justify itself as a divergence detection or resolution mechanism
- The bilateral loop is the central mechanism, not the store (the store is infrastructure)

#### Falsification
The system is evaluated primarily on information retrieval metrics (recall, precision)
rather than on coherence metrics (F(S), divergence measure, convergence rate), OR
the system lacks the ability to detect divergence at any of the four boundaries.

---

### ADR-BILATERAL-007: Formalism-to-Divergence-Type Mapping

**Traces to**: SEED §3, ADRS CO-005
**Stage**: 1

#### Problem
How do the specification formalism elements (invariants, ADRs, negative cases)
relate to the reconciliation taxonomy? Without this mapping, the formalism is
disconnected from the divergence detection it is supposed to enable.

#### Options
A) **No mapping** — formalism elements are general-purpose; agents decide how to use them.
B) **One-to-one mapping** — each formalism element maps to exactly one divergence type.
C) **Primary mapping with secondary roles** — each formalism element has a primary divergence detection role but may contribute to detecting other types.

#### Decision
**Option C.** Each formalism element maps to a primary divergence detection role:
- **Invariants** detect **logical divergence** — they are falsifiable claims about system behavior. A violated invariant is a concrete instance of logical divergence (spec contradicts itself or implementation contradicts spec).
- **ADRs** detect **axiological divergence** — they record why decisions were made. Without ADRs, decisions get reversed without knowing why they were originally made, causing the system to oscillate between alternatives (goal drift).
- **Negative cases** detect **structural divergence** — they bound the solution space. Without negative cases, a specification can be overspecified in one dimension and underspecified in another, causing structural gaps.

#### Formal Justification
This mapping makes the purpose of each formalism element precise and falsifiable.
An invariant is not just "a rule" — it is specifically a logical divergence
detector. An ADR is not just "a note" — it is specifically an axiological
divergence preventer. This precision enables automated divergence classification:
when an invariant is violated, the system knows this is logical divergence without
additional analysis. The secondary roles (e.g., invariants can also detect
structural divergence when they specify interface contracts) are permitted but
not primary.

#### Consequences
- Automated divergence classification becomes possible: formalism type implies divergence type
- Each formalism element must justify its existence as a divergence detector for its primary type
- The formalism is not arbitrary — it is the minimal set covering the three most critical divergence types
- Missing formalism elements for other divergence types (e.g., temporal) must be detected by other mechanisms (queries, signals)

#### Falsification
An invariant violation is classified as a non-logical divergence type, OR an ADR
is created that does not record decision rationale (failing its axiological
detection role), OR a negative case is written that does not bound the solution
space (failing its structural detection role).

---

### ADR-BILATERAL-008: Explicit Residual Divergence

**Traces to**: SEED §6, ADRS LM-010
**Stage**: 1

#### Problem
What happens when a bilateral cycle detects divergence that cannot be resolved?
If unresolvable divergence is silently ignored, the fitness function misreports
the system's state. If the loop blocks on unresolvable divergence, it never
terminates.

#### Options
A) **Block until resolved** — the bilateral loop does not proceed until all divergence is resolved.
B) **Silently ignore** — unresolvable divergence is dropped from the divergence map after a timeout.
C) **Explicit documentation** — unresolvable divergence is recorded as a residual with an uncertainty marker and rationale.

#### Decision
**Option C.** Unresolvable divergence is recorded explicitly with an uncertainty
marker. The residual becomes a datom in the store with attributes including
`:residual/rationale`, `:residual/uncertainty`, and `:residual/boundary`. This
preserves the information that divergence exists while allowing the bilateral
loop to reach fixpoint (INV-BILATERAL-004: no gap persists undocumented).

#### Formal Justification
Blocking (Option A) prevents fixpoint when legitimate disagreements exist (e.g.,
two valid architectural approaches with no empirical basis for choosing). Silent
ignoring (Option B) produces a fitness score of 1.0 that is fraudulent — the
system claims full coherence when known divergence exists. Explicit documentation
(Option C) enables honest fitness measurement: `F(S) = 1.0` only when all
divergence is either resolved or explicitly documented. Documented residuals
count as "handled" for fitness purposes but remain visible for future resolution.

#### Consequences
- The fitness function distinguishes between "no divergence" and "all divergence documented"
- Residuals accumulate in the store and are queryable
- Future sessions can re-evaluate residuals as new information becomes available
- The bilateral loop always terminates (either by resolving or by documenting)

#### Falsification
The bilateral loop claims fixpoint while undocumented divergence exists in the
divergence map, OR residual documentation lacks an uncertainty marker, OR
residuals are not queryable as datoms in the store.

---

### ADR-BILATERAL-009: Cross-Project Coherence Deferred

**Traces to**: SEED §6, ADRS CO-013
**Stage**: 3+

#### Problem
Can axiological divergence occur between projects? If two projects share goals
or specifications, divergence at the intent→spec boundary in one project may
propagate to the other. Should cross-project coherence be addressed now?

#### Options
A) **Address immediately** — build cross-project reconciliation into Stage 0.
B) **Defer entirely** — cross-project coherence is out of scope permanently.
C) **Defer with architectural preparation** — the store architecture supports cross-project merge (multiple stores are mergeable via C4), but the reconciliation machinery for cross-store contradiction detection is deferred to post-Stage-2.

#### Decision
**Option C.** The store architecture already supports cross-project coherence by
construction: any two stores can be merged via set union (C4). What is missing
is the reconciliation layer — cross-store contradiction detection requires
comparing invariants from different specification contexts, which introduces
namespace scoping complexity that is premature at Stage 0–2. The architectural
foundation is in place; the intelligence layer is deferred.

#### Formal Justification
Addressing cross-project coherence immediately (Option A) adds substantial
complexity to a system that has not yet proven single-project coherence works.
Deferring entirely (Option B) would require architectural changes later if the
store model did not support cross-store operations. Option C is the staged
approach: the append-only, content-addressed, set-union-mergeable store handles
the data layer by construction; only the query and reconciliation layers need
extension for cross-project support.

#### Consequences
- Stage 0–2 focuses exclusively on single-project coherence
- The store model requires no changes for cross-project support (C4 already provides set union merge)
- Cross-store contradiction detection is the specific gap to fill at Stage 3+
- Namespace scoping for invariant IDs across projects must be designed before cross-project reconciliation

#### Falsification
Cross-project coherence is needed before Stage 3 and the deferral causes
unresolvable architectural problems, OR the store model (C4 set union) proves
insufficient for cross-store merge (requiring store-level changes that were
assumed unnecessary).

---

### ADR-BILATERAL-010: Taxonomy Extensibility

**Traces to**: SEED §6, ADRS CO-014
**Stage**: 2

#### Problem
The reconciliation taxonomy defines eight divergence types. What happens when
a ninth type is discovered? Is the taxonomy closed or extensible?

#### Options
A) **Closed taxonomy** — eight types are exhaustive; any new divergence is a subtype of an existing type.
B) **Open taxonomy with ad-hoc extension** — new types can be added freely without constraints.
C) **Extensible by construction** — new divergence types yield new detection queries and new deliberation patterns, all constrained to produce datoms in the same store.

#### Decision
**Option C.** The taxonomy is extensible by construction. Adding a new divergence
type requires: (1) defining a detection query (Datalog) that identifies instances
of the divergence, (2) defining a classification rule that distinguishes it from
existing types, and (3) defining a resolution strategy that produces datoms
resolving or documenting the divergence. The constraint is that the resolution
mechanism for all types — existing and future — must be datom-producing queries.
This ensures new types integrate with the existing store without requiring
architectural changes.

#### Formal Justification
A closed taxonomy (Option A) assumes complete knowledge of divergence types, which
contradicts the system's own uncertainty principles (NEG-007: do not treat
uncertainty as a defect). New types of divergence will inevitably be discovered
as the system is used in novel contexts. Ad-hoc extension (Option B) risks
incoherent additions that bypass the detect-classify-resolve structure. The
constructive approach (Option C) preserves structural unity: every new type
follows the same pattern and produces the same output (datoms), so existing
infrastructure (queries, fitness function, bilateral loop) works unchanged.

#### Consequences
- The eight current types are not special — they follow the same extension protocol that new types would
- Adding a divergence type is a specification act (new ADR + new detection query + new resolution strategy)
- The fitness function automatically incorporates new types because it operates over the divergence map, which is type-agnostic
- No store or query engine changes required for taxonomy extension

#### Falsification
A new divergence type requires changes to the store model, query engine, or
bilateral loop infrastructure (rather than just adding queries and resolution
strategies), OR a new type cannot be expressed as a detect-classify-resolve
triple producing datoms.

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

**Safety property (mode-relative)**:
```
□ ¬(∃ AUTO_CYCLE that skips any of {CC-1, CC-2, CC-4, CC-5})
□ ¬(∃ FULL_CYCLE that skips any of {CC-1, CC-2, CC-3, CC-4, CC-5})
□ ¬(cycles_since_last_full_cycle > cc3_staleness_threshold ∧ ¬CC3StaleWarning_emitted)
```

No coherence dimension is left unchecked relative to cycle mode:
- AUTO_CYCLE checks all four machine-evaluable conditions (CC-1, CC-2, CC-4, CC-5)
- FULL_CYCLE checks all five conditions including human-gated CC-3
- CC-3 staleness is bounded: after `cc3_staleness_threshold` AUTO_CYCLEs without
  a FULL_CYCLE, the system emits CC3StaleWarning

**proptest strategy**: Instrument cycle execution. Verify: (1) AUTO_CYCLE
executes CC-1, CC-2, CC-4, CC-5 checks; (2) FULL_CYCLE executes all five checks;
(3) CC3StaleWarning emitted when threshold exceeded.

---

