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
`severity(s) = max(w(d₁), w(d₂))` for conflict signals. The routing tier is
monotonically determined by severity:
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

---

### ADR-SIGNAL-002: Conflict Routing Cascade as Datom Trail

**Traces to**: ADRS CR-003
**Stage**: 3

#### Problem
Should conflict routing produce durable records or be ephemeral dispatch?

#### Decision
Every step of the routing cascade (assert conflict → compute severity → route → notify →
update uncertainty → invalidate caches) produces datoms. The cascade is a transaction.
This makes the full resolution history queryable: "How was this conflict detected?
What severity was it assigned? Who resolved it? What was the rationale?"

#### Formal Justification
FD-012 (every command is a transaction) applies to signal routing. Ephemeral routing
would create state outside the store, violating the single-source-of-truth property.

---

### ADR-SIGNAL-003: Subscription Debounce Over Immediate Fire

**Traces to**: ADRS PO-008
**Stage**: 3

#### Problem
Should subscriptions fire immediately on every match, or debounce rapid-fire events?

#### Decision
Optional debounce parameter per subscription. Debounced subscriptions batch matching
signals within a time window and fire once with the full batch. Immediate fire
remains the default for latency-sensitive subscriptions (e.g., TUI notifications).

#### Formal Justification
MERGE cascade can produce many signals in rapid succession. Without debounce,
N conflicts from a single merge produce N subscription fires. Debounce reduces
to 1 batched fire containing N signals — same information, lower overhead.

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

