> **Namespace**: DELIBERATION | **Wave**: 3 (Intelligence) | **Stage**: 2
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §11. DELIBERATION — Structured Conflict Resolution

> **Purpose**: Deliberation is the structured resolution mechanism for conflicts that
> automated mechanisms (lattice join, LWW) cannot handle. It produces three entity types
> — Deliberation, Position, Decision — stored as datoms, creating a queryable case law
> system where past decisions inform future conflicts.
>
> **Traces to**: SEED.md §6 (Deliberation and Decision), ADRS CR-004, CR-005, CR-007,
> PO-007, AS-002, AA-001

### §11.1 Level 0: Algebraic Specification

A **deliberation** is a convergence process over a lattice of positions:

```
Deliberation = (question: String, positions: Set<Position>, decision: Option<Decision>)
Position = (stance: Stance, rationale: String, evidence: Set<DatomRef>)
Decision = (method: DecisionMethod, chosen: Position, rationale: String)

Stance = Advocate | Oppose | Neutral | Synthesize
DecisionMethod = Consensus | Majority | Authority | HumanOverride | Automated
```

**Deliberation lifecycle lattice**:
```
:open < :active < :decided < :superseded
         ↗ :stalled (incomparable with :decided)
```

**Laws**:
- **L1 (Convergence)**: Every deliberation either reaches `:decided` or `:stalled` in finite steps
- **L2 (Monotonicity)**: `lifecycle(d, t1) ⊑ lifecycle(d, t2)` for `t1 < t2` — deliberations progress forward in the lattice, never backward
- **L3 (Precedent preservation)**: Decided deliberations remain queryable as precedent (growth-only store guarantees this by construction)
- **L4 (Stability guard)**: A decision may only be reached when crystallization conditions are met (CR-005)

### §11.2 Level 1: State Machine Specification

**State**: `Σ_delib = (deliberations: Map<EntityId, Deliberation>, precedent_index: Map<(EntityType, Attr), Set<EntityId>>)`

**Transitions**:

```
OPEN(Σ, question, context) → Σ' where:
  POST: new deliberation entity with status :open
  POST: conflict signal recorded as causal predecessor

POSITION(Σ, delib_id, stance, rationale, evidence) → Σ' where:
  PRE:  Σ.deliberations[delib_id].status ∈ {:open, :active}
  POST: new position entity linked to deliberation
  POST: Σ.deliberations[delib_id].status = :active (if was :open)

DECIDE(Σ, delib_id, method, chosen, rationale) → Σ' where:
  PRE:  Σ.deliberations[delib_id].status = :active
  PRE:  stability_guard(chosen) passes (CR-005)
  POST: new decision entity linked to deliberation
  POST: Σ.deliberations[delib_id].status = :decided
  POST: competing branches resolved (winner committed, losers marked :abandoned)

STALL(Σ, delib_id, reason) → Σ' where:
  PRE:  Σ.deliberations[delib_id].status = :active
  POST: Σ.deliberations[delib_id].status = :stalled
  POST: reason recorded as uncertainty marker (UNC-*)
  POST: escalation signal emitted (DelegationRequest or GoalDrift)
```

**Crystallization stability guard** (CR-005):
- Status `:refined` (or position has substantive evidence)
- Thread `:active` (deliberation is ongoing, not stalled)
- Parent entity confidence ≥ 0.6
- Coherence score ≥ 0.6
- No unresolved conflicts on the decided entity
- Commitment weight `w(d) ≥ stability_min` (default 0.7)

### §11.3 Level 2: Implementation Contract

```rust
/// Deliberation entity — stored as datoms via schema Layer 2
pub struct Deliberation {
    pub entity: EntityId,
    pub question: String,
    pub status: DeliberationStatus,
    pub positions: Vec<EntityId>,  // refs to Position entities
    pub decision: Option<EntityId>, // ref to Decision entity
}

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub enum DeliberationStatus {
    Open,
    Active,
    Stalled,
    Decided,
    Superseded,
}

pub struct Position {
    pub entity: EntityId,
    pub deliberation: EntityId,
    pub stance: Stance,
    pub rationale: String,
    pub evidence: Vec<DatomRef>,
    pub agent: AgentId,
}

pub struct Decision {
    pub entity: EntityId,
    pub deliberation: EntityId,
    pub method: DecisionMethod,
    pub chosen_position: EntityId,
    pub rationale: String,
    pub commitment_weight: f64,
}
```

### §11.4 Invariants

### INV-DELIBERATION-001: Deliberation Convergence

**Traces to**: ADRS CR-004
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 2

#### Level 0 (Algebraic Law)
`∀ deliberation d: ◇(d.status = :decided ∨ d.status = :stalled)`
Every deliberation eventually reaches a terminal state.

#### Level 1 (State Invariant)
No deliberation remains in `:open` or `:active` indefinitely. Either positions
converge to a decision, or a timeout/stall condition triggers escalation.

#### Level 2 (Implementation Contract)
Deliberations carry a timeout. If no decision is reached within the timeout,
the deliberation transitions to `:stalled` and emits an escalation signal.

**Falsification**: A deliberation remains `:active` past its timeout without
transitioning to `:decided` or `:stalled`.

**Stateright model**: 3 agents filing positions on a deliberation. Verify
that all executions reach a terminal state.

---

### INV-DELIBERATION-002: Stability Guard Enforcement

**Traces to**: ADRS CR-005
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 2

#### Level 0 (Algebraic Law)
`∀ decision d: decide(d) ⟹ stability(d.chosen) ≥ stability_min`
No decision is recorded unless the crystallization stability guard passes.

#### Level 1 (State Invariant)
The DECIDE transition requires all stability guard conditions (CR-005) to hold.
A decision attempted with insufficient stability is rejected.

#### Level 2 (Implementation Contract)
```rust
#[kani::requires(stability_score(&position) >= STABILITY_MIN)]
fn decide(delib: &mut Deliberation, position: EntityId, method: DecisionMethod)
    -> Result<Decision, StabilityError> { ... }
```

**Falsification**: A decision is recorded where `stability(chosen) < stability_min`.

---

### INV-DELIBERATION-003: Precedent Queryability

**Traces to**: ADRS CR-007
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
All decided deliberations are indexed by entity type and contested attributes,
enabling precedent lookup:
```
find-precedent(entity_type, attributes) =
  {d ∈ deliberations | d.status = :decided
                     ∧ d.entity_type = entity_type
                     ∧ d.contested_attrs ∩ attributes ≠ ∅}
```

#### Level 1 (State Invariant)
The precedent index is maintained as a materialized view, updated on every DECIDE.
Precedent queries return all matching decided deliberations.

**Falsification**: A decided deliberation with matching entity type and attributes
is not returned by a precedent query.

---

### INV-DELIBERATION-004: Bilateral Deliberation Symmetry

**Traces to**: ADRS CR-004 (INV-DELIBERATION-BILATERAL-001), AS-006
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
Deliberation supports both forward and backward flow with identical entity structure:
- Forward: "Given this spec, which of these competing implementations is better?"
- Backward: "Given this implementation, which of these spec interpretations is correct?"

#### Level 1 (State Invariant)
The Deliberation/Position/Decision entity structure is direction-agnostic.
Forward and backward deliberations use the same schema, same lifecycle,
same stability guard, same precedent query.

**Falsification**: The system creates a structural asymmetry where forward
deliberations have capabilities that backward deliberations lack (or vice versa).

---

### INV-DELIBERATION-005: Commitment Weight Integration

**Traces to**: ADRS AS-002
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 2

#### Level 0 (Algebraic Law)
The decision's commitment weight is computed from its forward causal cone:
`w(decision) = |{d' ∈ S : decision ∈ causes*(d')}|`

Decisions with high commitment weight are harder to overturn (require
stronger evidence, higher authority).

#### Level 1 (State Invariant)
When a new decision is recorded, its commitment weight is computed and stored.
As downstream decisions reference it, the weight monotonically increases.

**Falsification**: A decision's commitment weight decreases after downstream
decisions are recorded.

---

### INV-DELIBERATION-006: Competing Branch Resolution

**Traces to**: ADRS PO-007
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 2

#### Level 0 (Algebraic Law)
When a deliberation produces a decision selecting one competing branch:
- The winning branch is committed to trunk
- Losing branches are marked `:abandoned` (remain readable for provenance)
- No losers' datoms leak into trunk

#### Level 1 (State Invariant)
For all reachable states where DECIDE selects a branch:
```
trunk' = trunk ∪ winner.datoms
∀ loser: loser.status = :abandoned
∀ loser: loser.datoms ∩ trunk' = loser.datoms ∩ trunk  (no new datoms from losers)
```

**Falsification**: A losing branch's datoms appear in trunk after the decision.

**Stateright model**: 2 agents with competing branches. Deliberation decides.
Verify loser's datoms never appear in trunk.

---

### §11.5 ADRs

### ADR-DELIBERATION-001: Three Entity Types for Structured Resolution

**Traces to**: ADRS CR-004
**Stage**: 2

#### Problem
What entities are needed for structured conflict resolution?

#### Decision
Three: Deliberation (the process), Position (a stance with rationale and evidence),
Decision (the outcome with method and chosen position). All stored as datoms.

#### Formal Justification
The separation into three entity types mirrors legal proceedings: a case (Deliberation),
arguments (Positions), and a ruling (Decision). This structure enables precedent queries
(CR-007) — past Decisions inform future Deliberations. A single entity type would lose
the distinction between process, argument, and outcome.

---

### ADR-DELIBERATION-002: Five Decision Methods

**Traces to**: ADRS CR-004
**Stage**: 2

#### Problem
What decision methods should be supported?

#### Options
A) Consensus only — simplest, but may never converge
B) Authority only — fast, but ignores evidence quality
C) Five methods: Consensus, Majority, Authority, HumanOverride, Automated

#### Decision
**Option C.** Different conflicts warrant different resolution methods. Low-stakes
conflicts can use Automated (lattice join). Medium-stakes use Majority or Authority.
High-stakes require HumanOverride. Consensus is the ideal but not always achievable.

#### Formal Justification
The method selection aligns with the three-tier conflict routing (CR-002):
Tier 1 (Low) → Automated, Tier 2 (Medium) → Majority/Authority,
Tier 3 (High) → HumanOverride. Consensus is orthogonal — achievable at any tier
but never required.

---

### ADR-DELIBERATION-003: Precedent as Case Law

**Traces to**: ADRS CR-007
**Stage**: 2

#### Problem
Should past deliberation outcomes inform future conflicts?

#### Decision
Yes. Decided deliberations are indexed by entity type and contested attributes.
When a new conflict arises, the system queries for precedent — past decisions
on the same entity type and attributes. Precedent doesn't bind (not stare decisis)
but is surfaced as context for the new deliberation.

#### Formal Justification
The growth-only store guarantees precedent preservation by construction (no deliberation
is ever deleted). Indexing by entity type and attributes is the natural decomposition:
conflicts on the same kind of entity tend to have similar resolution patterns.

---

### ADR-DELIBERATION-004: Crystallization Guard Over Immediate Commit

**Traces to**: ADRS CR-005
**Stage**: 2

#### Problem
Should decisions take effect immediately or after a stability period?

#### Decision
Stability guard. The default `stability_min = 0.7` ensures a decision is not
committed prematurely. The guard checks six conditions (status, thread, confidence,
coherence, conflicts, commitment weight). This prevents the failure mode where
a quick decision with incomplete evidence cascades into downstream errors.

#### Formal Justification
Premature crystallization is an S0-severity failure mode (silently wrong artifacts
with no detection signal). The stability guard is the direct countermeasure.
The cost (delayed commitment) is justified by the risk (cascading incompleteness,
FM-004).

---

### §11.6 Negative Cases

### NEG-DELIBERATION-001: No Decision Without Stability Guard

**Traces to**: ADRS CR-005
**Verification**: `V:PROP`, `V:KANI`

**Safety property**: `□ ¬(decision_recorded ∧ stability < stability_min)`

No decision may be recorded if the stability guard conditions are not met.

**proptest strategy**: Generate random deliberation states with varying stability.
Attempt DECIDE. Verify rejection when stability < threshold.

**Kani harness**: Exhaustive check over all stability dimension combinations
that `decide()` rejects when any dimension is below threshold.

---

### NEG-DELIBERATION-002: No Losing Branch Leak

**Traces to**: ADRS PO-007
**Verification**: `V:PROP`, `V:MODEL`

**Safety property**: `□ ¬(branch.status = :abandoned ∧ branch.datoms ∩ trunk' ⊃ branch.datoms ∩ trunk)`

No new datoms from an abandoned branch appear in trunk.

**Stateright model**: 3 agents, 2 competing branches. Decision selects one.
Verify no datoms from the loser appear in trunk post-decision.

---

### NEG-DELIBERATION-003: No Backward Lifecycle Transition

**Traces to**: ADRS CR-004
**Verification**: `V:PROP`, `V:KANI`

**Safety property**: `□ ¬(lifecycle(d,t2) ⊏ lifecycle(d,t1) for t2 > t1)`

Deliberation lifecycle progresses monotonically: open → active → decided/stalled.
No backward transitions (e.g., decided → active) are permitted.

**proptest strategy**: Generate random transition sequences. Verify lattice
monotonicity after each transition.

---

