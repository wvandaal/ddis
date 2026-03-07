> **Namespace**: SIGNAL | **Wave**: 3 (Intelligence) | **Stage**: 3
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §9. SIGNAL — Divergence Signal Routing

> **Purpose**: Signals are the nervous system of DDIS — typed events that detect divergence
> and route it to the appropriate resolution mechanism. Every signal is a datom, making
> the system's self-awareness queryable and auditable.
>
> **Traces to**: SEED.md §6 (Reconciliation Mechanisms), ADRS PO-004, PO-005, PO-008,
> CO-003, CR-002, CR-003, AS-009

### §9.1 Level 0: Algebraic Specification

A **signal** is a typed divergence detection event:

```
Signal = (type: SignalType, source: EntityId, target: EntityId,
          severity: Severity, payload: Value)

SignalType = Confusion | Conflict | UncertaintySpike | ResolutionProposal
           | DelegationRequest | GoalDrift | BranchReady | DeliberationTurn

Severity = Low | Medium | High | Critical
  with total order: Low < Medium < High < Critical
```

The **signal dispatch function** maps signal types to resolution mechanisms:

```
dispatch : Signal → ResolutionMechanism
dispatch(Confusion(_))         = ReAssociate       — epistemic divergence
dispatch(Conflict(_))          = Route(severity)    — aleatory divergence
dispatch(UncertaintySpike(_))  = Guidance           — consequential divergence
dispatch(GoalDrift(_))         = Escalate(human)    — axiological divergence
dispatch(DelegationRequest(_)) = Delegate           — authority resolution
dispatch(BranchReady(_))       = Compare            — structural divergence
dispatch(DeliberationTurn(_))  = Deliberate         — logical divergence
dispatch(ResolutionProposal(_))= Evaluate           — resolution convergence
```

**Laws**:
- **L1 (Totality)**: Every signal type has a defined dispatch target
- **L2 (Monotonicity)**: `severity(s1) ≤ severity(s2) ⟹ cost(dispatch(s1)) ≤ cost(dispatch(s2))` — higher severity signals route to more expensive resolution mechanisms
- **L3 (Completeness)**: Every divergence type in the reconciliation taxonomy (CO-003) maps to at least one signal type

### §9.2 Level 1: State Machine Specification

**State**: `Σ_signal = (pending: Set<Signal>, active: Set<Signal>, resolved: Set<Signal>, subscriptions: Map<Pattern, Set<Callback>>)`

**Transitions**:

```
EMIT(Σ, signal) → Σ' where:
  PRE:  signal.source ∈ known_entities(store)
  POST: Σ'.pending = Σ.pending ∪ {signal}
  POST: signal recorded as datom in store
  POST: matching subscriptions fired

ROUTE(Σ, signal) → Σ' where:
  PRE:  signal ∈ Σ.pending
  POST: Σ'.pending = Σ.pending \ {signal}
  POST: Σ'.active = Σ.active ∪ {signal}
  POST: dispatch(signal) invoked

RESOLVE(Σ, signal, resolution) → Σ' where:
  PRE:  signal ∈ Σ.active
  POST: Σ'.active = Σ.active \ {signal}
  POST: Σ'.resolved = Σ.resolved ∪ {signal}
  POST: resolution recorded as datom with causal link to signal

SUBSCRIBE(Σ, pattern, callback) → Σ' where:
  POST: Σ'.subscriptions[pattern] = Σ.subscriptions[pattern] ∪ {callback}
  INV:  subscription persists until explicitly removed
```

**Conflict routing cascade** (from CR-003):
1. Assert Conflict entity as datom
2. Compute severity = `max(w(d₁), w(d₂))` (commitment weights)
3. Route by severity tier: automated (Low) → agent-with-notification (Medium) → human-required (High/Critical)
4. Fire TUI notification if severity ≥ Medium
5. Update uncertainty tensor for affected entities
6. Invalidate cached query results touching affected entities

### §9.3 Level 2: Implementation Contract

```rust
/// Signal types — sum type covering all divergence classes
#[derive(Clone, Debug)]
pub enum SignalType {
    Confusion(ConfusionKind),
    Conflict { datom_a: DatomRef, datom_b: DatomRef },
    UncertaintySpike { entity: EntityId, delta: f64 },
    ResolutionProposal { deliberation: EntityId, position: EntityId },
    DelegationRequest { entity: EntityId, from: AgentId, to: AgentId },
    GoalDrift { intention: EntityId, observed_delta: f64 },
    BranchReady { branch: EntityId, comparison_criteria: Vec<Criterion> },
    DeliberationTurn { deliberation: EntityId, position: EntityId },
}

#[derive(Clone, Debug)]
pub enum ConfusionKind {
    NeedMore,       // insufficient context
    Contradictory,  // conflicting information
    GoalUnclear,    // ambiguous intention
    SchemaUnknown,  // unknown entity type or attribute
}

pub struct Signal {
    pub signal_type: SignalType,
    pub source: EntityId,
    pub target: EntityId,
    pub severity: Severity,
    pub timestamp: TxId,
}

/// Subscription — Datalog-like pattern with callback
pub struct Subscription {
    pub pattern: SignalPattern,
    pub callback: Box<dyn Fn(&Signal) -> Vec<Datom>>,
    pub debounce: Option<Duration>,
}
```

### §9.4 Invariants

### INV-SIGNAL-001: Signal as Datom

**Traces to**: ADRS PO-004
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 3

#### Level 0 (Algebraic Law)
Every signal is a datom. Signal history is a subset of the store:
`∀ s ∈ signals_emitted: ∃ d ∈ S such that d encodes s`

#### Level 1 (State Invariant)
For all reachable states, every emitted signal has a corresponding datom in the store
with entity type `:signal/*` and attributes recording type, source, target, severity.

#### Level 2 (Implementation Contract)
```rust
// Every emit produces a transact
fn emit_signal(store: &mut Store, signal: Signal) -> TxReceipt {
    let datoms = signal.to_datoms(); // deterministic encoding
    store.transact(Transaction::from(datoms).commit(&store.schema()).unwrap())
        .unwrap()
}
```

**Falsification**: A signal is emitted but no corresponding datom exists in the store.

**proptest strategy**: Emit random signals. After each, query store for `:signal/type`
matching the emitted type. Verify 1:1 correspondence.

---

### INV-SIGNAL-002: Confusion Triggers Re-Association

**Traces to**: ADRS PO-005
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
`dispatch(Confusion(cue)) = ReAssociate(cue)` — confusion signals trigger the
associative retrieval pipeline within one agent cycle (not a full round-trip).

#### Level 1 (State Invariant)
For all reachable states where a Confusion signal is emitted:
within the same agent cycle, ASSOCIATE + ASSEMBLE execute with the confusion cue
as input, producing an updated context.

#### Level 2 (Implementation Contract)
The agent cycle handler intercepts Confusion signals and invokes the
`associate → query → assemble` pipeline before proceeding to the next action.

**Falsification**: A Confusion signal is emitted and the agent proceeds to the next
action without re-association.

**proptest strategy**: Inject Confusion signals at random points in agent cycle
simulations. Verify re-association always executes before the next action.

---

### INV-SIGNAL-003: Subscription Completeness

**Traces to**: ADRS PO-008
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 3

#### Level 0 (Algebraic Law)
`∀ subscription s, signal σ: matches(s.pattern, σ) ⟹ s.callback(σ) is invoked`

No matching signal is silently dropped.

#### Level 1 (State Invariant)
For all reachable states where EMIT produces a signal matching a subscription pattern,
the subscription callback fires within one refresh cycle. Debounced subscriptions
batch within their declared window but still fire.

#### Level 2 (Implementation Contract)
```rust
#[kani::ensures(|_| subscriptions.iter()
    .filter(|s| s.pattern.matches(&signal))
    .all(|s| s.fired_count > old(s.fired_count)))]
fn emit_and_dispatch(signal: Signal, subscriptions: &mut [Subscription]) { ... }
```

**Falsification**: A subscription pattern matches a signal, but the callback is never invoked.

---

### INV-SIGNAL-004: Severity-Ordered Routing

**Traces to**: ADRS CR-002
**Verification**: `V:PROP`
**Stage**: 3

#### Level 0 (Algebraic Law)
For all signal types t and signals s1, s2:
  `severity(s1) <= severity(s2) implies cost(dispatch(s1)) <= cost(dispatch(s2))`

For conflict signals specifically:
  `severity(s) = max(w(d₁), w(d₂))` (commitment weights of the conflicting datoms)

Routing by severity tier:
```
Low      → Tier 1 (automated lattice/LWW resolution)
Medium   → Tier 2 (agent-with-notification)
High     → Tier 3 (human-required, blocks progress)
Critical → Tier 3 + immediate TUI alert
```

#### Level 1 (State Invariant)
No High/Critical severity signal is resolved by an automated mechanism.
No Low severity signal blocks agent progress.

#### Level 2 (Implementation Contract)
The routing function's output tier is determined by a match on severity,
with the mapping configured as datoms (enabling per-deployment tuning).

**Falsification**: A Critical-severity conflict is silently resolved by LWW
without human/agent review.

---

### INV-SIGNAL-005: Diamond Lattice Signal Generation

**Traces to**: ADRS AS-009
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 2

#### Level 0 (Algebraic Law)
For diamond lattices (challenge-verdict, finding-lifecycle, proposal-lifecycle),
when two incomparable values are merged (CRDT join), the result is the lattice top
which encodes a coordination signal:
```
join(confirmed, refuted) = contradicted    → emits Conflict signal
join(proposed_A, proposed_B) = contested   → emits DeliberationTurn signal
```

#### Level 1 (State Invariant)
For all reachable states where a lattice merge produces a diamond-top value,
a signal of the corresponding type is emitted within the same transaction.

#### Level 2 (Implementation Contract)
Lattice join implementations for diamond lattices include a signal-emission
side effect when the join produces the top element.

**Falsification**: Two incomparable lattice values merge to produce a top element
but no signal is emitted.

**proptest strategy**: Generate random concurrent assertions on diamond-lattice
attributes. Verify that every top-join produces exactly one signal.

---

### INV-SIGNAL-006: Taxonomy Completeness

**Traces to**: ADRS CO-003
**Verification**: `V:PROP`
**Stage**: 3

#### Level 0 (Algebraic Law)
The signal type set covers all eight divergence types in the reconciliation taxonomy:
```
Epistemic    → Confusion
Structural   → BranchReady (forward), GoalDrift (backward)
Consequential → UncertaintySpike
Aleatory     → Conflict
Logical      → DeliberationTurn
Axiological  → GoalDrift
Temporal     → (detected by frontier comparison, surfaced as UncertaintySpike)
Procedural   → (detected by drift detection, surfaced as GoalDrift)
```

#### Level 1 (State Invariant)
Every detected divergence, regardless of type, produces at least one signal.
No divergence class lacks a signal pathway.

**Falsification**: A divergence is detected by some mechanism but no signal
is emitted, leaving it invisible to the resolution layer.

---

### §9.5 ADRs

### ADR-SIGNAL-001: Eight Signal Types Cover Reconciliation Taxonomy

**Traces to**: ADRS PO-004, CO-003
**Stage**: 3

#### Problem
How many signal types are needed, and how do they map to divergence types?

#### Options
A) One generic signal type with metadata — simple but loses type safety
B) One signal type per divergence type (8) — exact coverage but some divergence
   types don't map to a natural signal
C) Eight signal types, some covering multiple divergence types — pragmatic mapping

#### Decision
**Option C.** Eight concrete signal types (from PO-004) with a surjective mapping
from divergence types. Some divergence types (Temporal, Procedural) are detected by
specialized mechanisms and surfaced through existing signal types.

#### Formal Justification
The taxonomy completeness law (L3) requires surjection from divergence types to signal
types, not bijection. A 1:1 mapping would force artificial signal types for divergences
that are better detected by existing mechanisms (e.g., temporal divergence is naturally
frontier comparison, not a separate signal).

#### Consequences
- All eight divergence types from the reconciliation taxonomy have at least one signal pathway
- Signal type exhaustive matching in Rust provides compile-time coverage guarantees
- Some divergence types (Temporal, Procedural) are detected by specialized mechanisms and
  surfaced through existing signal types rather than dedicated ones
- Adding a ninth divergence type requires either mapping it to an existing signal type or
  extending the SignalType enum (ADR-BILATERAL-010 taxonomy extensibility)

#### Falsification
This decision is wrong if: a divergence type is identified that cannot be mapped to any of
the eight signal types — specifically, if the detection mechanism for the divergence produces
information that none of the eight signal types can represent without semantic loss.

---

### ADR-SIGNAL-002: Conflict Routing Cascade as Datom Trail

**Traces to**: ADRS CR-003
**Stage**: 3

#### Problem
Should conflict routing produce durable records or be ephemeral dispatch?

#### Options
A) **Ephemeral dispatch** — conflict routing is in-memory only, no persistence.
   Lowest overhead but no audit trail and no queryable history.
B) **Summary-only persistence** — persist the final resolution but not intermediate cascade steps.
   Moderate overhead, partial audit trail.
C) **Full datom trail** — every cascade step produces datoms recording the routing decision.
   Full audit trail, queryable resolution history, higher storage cost per conflict.

#### Decision
Every step of the routing cascade (assert conflict → compute severity → route → notify →
update uncertainty → invalidate caches) produces datoms. The cascade is a transaction.
This makes the full resolution history queryable: "How was this conflict detected?
What severity was it assigned? Who resolved it? What was the rationale?"

#### Formal Justification
FD-012 (every command is a transaction) applies to signal routing. Ephemeral routing
would create state outside the store, violating the single-source-of-truth property.

#### Consequences
- Every conflict routing step is auditable after the fact via store queries
- The full resolution history is queryable: detection, severity assignment, routing,
  notification, uncertainty update, cache invalidation
- Cascade datoms are content-addressable from conflict content (INV-MERGE-010), preserving
  determinism even though routing produces multiple datoms per conflict
- Storage cost scales linearly with conflict count — each conflict cascade produces O(6) datoms

#### Falsification
This decision is wrong if: datom overhead per conflict cascade measurably impacts
high-conflict workloads — specifically, if a merge introducing N conflicts produces
6N+ cascade datoms whose storage and indexing cost exceeds the value of the audit trail,
causing observable performance degradation in scenarios with >100 conflicts per merge.

---

### ADR-SIGNAL-003: Subscription Debounce Over Immediate Fire

**Traces to**: ADRS PO-008
**Stage**: 3

#### Problem
Should subscriptions fire immediately on every match, or debounce rapid-fire events?

#### Options
A) **Immediate fire** — every matching signal fires every subscription immediately.
   Lowest latency but O(N) fires for N rapid signals in a burst.
B) **Global debounce window** — all subscriptions share a single debounce interval.
   Simple but prevents latency-sensitive subscriptions from firing promptly.
C) **Per-subscription debounce policy** — each subscription declares its own policy
   (immediate, windowed, or batch). Supports mixed latency requirements at the cost
   of more complex dispatch logic.

#### Decision
Optional debounce parameter per subscription. Debounced subscriptions batch matching
signals within a time window and fire once with the full batch. Immediate fire
remains the default for latency-sensitive subscriptions (e.g., TUI notifications).

#### Formal Justification
MERGE cascade can produce many signals in rapid succession. Without debounce,
N conflicts from a single merge produce N subscription fires. Debounce reduces
to 1 batched fire containing N signals — same information, lower overhead.

#### Consequences
- Default behavior is immediate fire — no debounce unless explicitly configured
- Debounced subscriptions batch signals within their declared window but still fire (INV-SIGNAL-003)
- TUI notifications use immediate fire (latency-sensitive); background analytics use debounce
- Debounce window is per-subscription, not global — different subscribers can have different latencies

#### Falsification
This decision is wrong if: debounce causes latency-sensitive subscriptions to miss timing
windows — specifically, if a subscription with debounce enabled fails to fire within the
declared window, or if the batching mechanism drops signals that arrive during the
debounce cooldown period, violating INV-SIGNAL-003 (subscription completeness).

---

### ADR-SIGNAL-004: Four-Type Divergence Taxonomy (Original)

**Traces to**: SEED §6, ADRS CO-002
**Status**: SUPERSEDED by ADR-SIGNAL-001
**Stage**: 3

#### Problem
What types of divergence does the system need to detect and resolve? The reconciliation
framework requires a taxonomy that maps divergence types to detection mechanisms and
resolution strategies. The initial analysis identified four fundamental types.

#### Options
A) **Single divergence type** — All divergence is treated uniformly. Simple but loses
   the ability to route different divergence types to appropriate resolution mechanisms.
   A specification-implementation mismatch requires different handling than an agent-agent
   disagreement.
B) **Four-type taxonomy** — Epistemic (store vs. agent knowledge), Structural
   (implementation vs. spec), Consequential (current state vs. future risk), and
   Aleatory (agent vs. agent). Each type has distinct detection and resolution patterns.
C) **Open-ended taxonomy** — Divergence types are user-defined, with no fixed
   classification. Maximum flexibility but no systematic guarantee that all divergence
   types have resolution pathways.

#### Decision
**Option B (subsequently expanded).** The original four-type taxonomy was:

1. **Epistemic** — Gap between what the store knows and what the agent knows.
   Detection: harvest gap analysis. Resolution: harvest (promote to datoms).
2. **Structural** — Gap between implementation and specification.
   Detection: bilateral scan, drift measurement. Resolution: bilateral loop.
3. **Consequential** — Gap between current state and future risk.
   Detection: uncertainty tensor analysis. Resolution: guidance (redirect before action).
4. **Aleatory** — Disagreement between agents on the same fact.
   Detection: merge conflict detection. Resolution: deliberation + decision.

#### Formal Justification
The four types correspond to four distinct boundaries across which coherence can break:
agent-store (epistemic), code-spec (structural), present-future (consequential), and
agent-agent (aleatory). Each boundary has fundamentally different detection mechanisms
and resolution strategies — collapsing them into one type (Option A) would force a
one-size-fits-all resolution that handles none well.

#### Consequences
- This taxonomy was the starting point but proved incomplete
- Four additional divergence types were identified in subsequent analysis (CO-003):
  Logical (invariant vs. invariant), Axiological (implementation vs. goals),
  Temporal (agent frontier vs. agent frontier), and Procedural (agent behavior vs. methodology)
- The expanded eight-type taxonomy is formalized as ADR-SIGNAL-001
- This ADR is preserved as historical record of the design evolution

#### Falsification
This decision (in its original four-type form) was falsified by the identification of
divergence types that do not fit any of the four categories — specifically, logical
contradictions within the specification itself, and temporal divergence between agent
frontiers. This led to the eight-type expansion in ADR-SIGNAL-001.

---

### ADR-SIGNAL-005: Four Recognized Taxonomy Gaps

**Traces to**: SEED §6, ADRS CO-007
**Stage**: 3

#### Problem
Even the expanded eight-type reconciliation taxonomy (ADR-SIGNAL-001) has known coverage
gaps. Four specific divergence scenarios were identified that do not cleanly map to any
of the eight types, or where the existing resolution mechanisms are insufficient. Should
these gaps be ignored, patched into existing types, or explicitly documented with planned
resolutions?

#### Options
A) **Ignore gaps** — The eight types cover the important cases; edge cases can be handled
   ad hoc. Risks silent divergence in the uncovered scenarios.
B) **Force-fit into existing types** — Map each gap to the closest existing type. Preserves
   the eight-type taxonomy but creates awkward mappings where detection and resolution
   mechanisms don't align.
C) **Explicitly document gaps with planned resolutions** — Acknowledge the gaps, document
   what each gap covers, and specify how each will be addressed by existing or new
   mechanisms. Honest about coverage limits while maintaining a plan for closure.

#### Decision
**Option C.** Four coverage gaps are explicitly recognized and individually addressed:

**Gap 1: Spec-to-Intent Divergence**
The specification may drift from the original intent without any structural signal. The
eight-type taxonomy covers spec-vs-implementation (structural) and implementation-vs-goals
(axiological), but not spec-vs-intent directly.
*Addressed by*: Intent validation sessions — periodic human review where the specification
is checked against the original goals in SEED.md. This is a procedural mechanism (an
acknowledged exception to ADR-FOUNDATION-005's structural preference) because intent is
inherently subjective and cannot be mechanically verified.

**Gap 2: Implementation-to-Behavior Divergence**
The implementation may match the specification but produce unexpected behavior in practice.
The eight-type taxonomy covers code-vs-spec (structural) but not code-vs-observed-behavior.
*Addressed by*: Test results as datoms — behavioral test outcomes are transacted into the
store, enabling queries like "Does the observed behavior match the specified behavior?"
Test failures surface as structural divergence signals.

**Gap 3: Cross-Project Coherence**
Multiple projects using DDIS may make inconsistent decisions about shared concepts. The
eight-type taxonomy is project-scoped; cross-project divergence is out of scope.
*Addressed by*: Deferred to future architecture. Cross-project coherence requires a
meta-store or federation protocol that is out of scope for Stages 0–4. The datom store's
CRDT merge semantics (C4) provide the foundation: merging cross-project stores is
mathematically valid, but the governance layer (who resolves conflicts?) is unspecified.

**Gap 4: Temporal Degradation of Observations**
Observations about external state (file contents, test results, system behavior) degrade
over time — the external state may change while the datom remains. The eight-type taxonomy
includes temporal divergence (agent frontiers) but not observation staleness.
*Addressed by*: Observation staleness model — each observation datom carries a
`:observation/observed-at` timestamp. Queries can filter by freshness, and the guidance
system can signal when critical observations exceed their staleness threshold.

#### Formal Justification
Explicitly documenting gaps is more honest than claiming complete coverage. Each gap has
a planned resolution mechanism, making the gap a temporary condition rather than a permanent
blind spot. The gaps are ordered by addressability: Gaps 1–2 are addressable within the
current architecture, Gap 3 requires architectural extension, and Gap 4 requires a new
attribute on observation datoms.

By documenting gaps as an ADR rather than hiding them, the system's self-awareness
includes its own limitations — consistent with the structural coherence philosophy
(ADR-FOUNDATION-005) applied to the specification itself.

#### Consequences
- The eight-type taxonomy is acknowledged as incomplete, with four documented gaps
- Each gap has a named resolution mechanism and a scope (current architecture vs. future)
- Gap 3 (cross-project) is explicitly deferred — it requires federation semantics beyond
  Stages 0–4
- Gap 4 (observation staleness) motivates the `:observation/observed-at` attribute in the
  Layer 2 schema (INV-SCHEMA-006)
- Future taxonomy extensions should check these four gaps first for coverage

#### Falsification
This decision is wrong if: the four gaps are not actually gaps (each scenario maps cleanly
to an existing eight-type divergence class with adequate detection and resolution), making
the explicit documentation unnecessary complexity.

---

### §9.6 Negative Cases

### NEG-SIGNAL-001: No Silent Signal Drop

**Traces to**: ADRS PO-004
**Verification**: `V:PROP`, `V:KANI`

**Safety property**: `□ ¬(∃ signal emitted ∧ ¬recorded_as_datom)`

Every emitted signal is recorded in the store. No signal is lost between
emission and recording.

**proptest strategy**: Emit signals under concurrent load (multiple agents).
Verify store contains exactly the emitted signal set after quiescence.

---

### NEG-SIGNAL-002: No Confusion Without Re-Association

**Traces to**: ADRS PO-005
**Verification**: `V:PROP`

**Safety property**: `□ ¬(confusion_emitted ∧ ¬reassociation_within_cycle)`

A Confusion signal that doesn't trigger re-association is a protocol violation.
The agent must not proceed with stale context after signaling confusion.

**proptest strategy**: Inject Confusion signals. Verify agent cycle always
executes ASSOCIATE+ASSEMBLE before the next action step.

---

### NEG-SIGNAL-003: No High-Severity Automated Resolution

**Traces to**: ADRS CR-002
**Verification**: `V:PROP`

**Safety property**: `□ ¬(severity ≥ High ∧ resolved_by_automated_mechanism)`

High and Critical severity conflicts must involve agent or human review.
Automated resolution (lattice join, LWW) is restricted to Low severity.

**proptest strategy**: Generate conflicts with all severity levels. Verify
that High/Critical conflicts are never closed by automated resolution.

---

