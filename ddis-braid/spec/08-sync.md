> **Namespace**: SYNC | **Wave**: 2 (Lifecycle) | **Stage**: 3
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §8. SYNC — Sync Barriers

### §8.0 Overview

Sync barriers establish consistent cuts — shared reference points where all participating
agents agree on the same facts. This is the most expensive coordination mechanism because
it requires blocking until all participants report their frontiers. It is necessary for
decisions that depend on the absence of certain facts (non-monotonic queries).

**Traces to**: SEED.md §6
**ADRS.md sources**: PO-010, SQ-001, SQ-004, PD-005

---

### §8.1 Level 0: Algebraic Specification

#### Consistent Cut

```
A consistent cut C is a set of frontiers such that:
  ∀ agents α, β participating in C:
    C[α] and C[β] are causally consistent
    (no message "in flight" — all sent messages are received)

Formally: C = {(α, F_α) | α ∈ participants}
  where ∀ α: F_α = frontier of α at barrier completion

A consistent cut enables answering "what is NOT in the store" —
the set of facts absent from the cut is meaningful because all
participants agree on what IS present.
```

#### Barrier as Frontier Intersection

```
Given agents {α₁, ..., αₙ} with frontiers {F₁, ..., Fₙ}:

Barrier establishes: ∀ i, j: known(αᵢ, F_barrier) = known(αⱼ, F_barrier)
  where F_barrier = the consistent cut

Post-barrier: non-monotonic queries at F_barrier produce
deterministic results across all participants.
```

---

### §8.2 Level 1: State Machine Specification

#### Barrier Protocol

```
SYNC-BARRIER(participants, timeout) → BarrierResult

PROTOCOL:
  1. INITIATE: Barrier initiator creates Barrier entity in store.
     barrier.status = :initiated
     barrier.participants = [agent IDs]
     barrier.timeout = duration

  2. EXCHANGE: Each participant:
     a. Reports current frontier to barrier entity
     b. Shares all datoms not yet received by others (delta sync)
     c. Waits for all other participants to report

  3. RESOLVE:
     If all participants report within timeout:
       barrier.status = :resolved
       barrier.cut = consistent cut (the agreed-upon frontier)
       All participants now have identical datom sets (up to the cut)
     If timeout expires:
       barrier.status = :timed-out
       barrier records which participants responded

  4. QUERY-ENABLE:
     Post-resolution, non-monotonic queries reference the barrier:
       QueryMode::Barriered(barrier_id)
     Results are deterministic across all participants.

POST:
  Barrier entity in store with full provenance
  All participants at same frontier (if resolved)
```

#### Topology-Dependent Implementation

```
The protocol provides primitives; deployment chooses topology.

Star topology:   coordinator collects and distributes
Ring topology:   each agent passes to next
Mesh topology:   all-to-all exchange
Hierarchical:    tree-structured aggregation

The sync result is topology-independent (SQ-005):
  same participants + same datoms → same consistent cut
```

---

### §8.3 Level 2: Interface Specification

```rust
/// Sync barrier entity.
pub struct Barrier {
    pub id: EntityId,
    pub participants: Vec<AgentId>,
    pub status: BarrierStatus,     // lattice: :initiated < :exchanging < :resolved | :timed-out
    pub timeout: Duration,
    pub cut: Option<Frontier>,     // set after resolution
    pub responses: HashMap<AgentId, Frontier>,
}

pub enum BarrierResult {
    Resolved { cut: Frontier },
    TimedOut { responded: Vec<AgentId>, missing: Vec<AgentId> },
}

impl Store {
    /// Initiate a sync barrier.
    pub fn sync_barrier(
        &mut self,
        participants: &[AgentId],
        timeout: Duration,
    ) -> Result<EntityId, SyncError>;

    /// Participate in a barrier (report frontier, share deltas).
    pub fn barrier_participate(
        &mut self,
        barrier_id: EntityId,
        agent: AgentId,
    ) -> Result<(), SyncError>;

    /// Check barrier status.
    pub fn barrier_status(&self, barrier_id: EntityId) -> BarrierStatus;

    /// Query at a barrier's consistent cut.
    pub fn query_at_barrier(
        &mut self,
        expr: &QueryExpr,
        barrier_id: EntityId,
    ) -> Result<QueryResult, QueryError>;
}
```

#### CLI Commands

```
braid sync --with agent-1,agent-2       # Initiate barrier
braid sync --timeout 30s                # With timeout
braid sync status <barrier-id>          # Check barrier status
braid query --barrier <barrier-id> '[:find ...]'  # Query at barrier
```

---

### §8.4 Invariants

### INV-SYNC-001: Barrier Produces Consistent Cut

**Traces to**: SEED §6, ADRS PO-010
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 3

#### Level 0 (Algebraic Law)
```
∀ resolved barriers B with participants {α₁, ..., αₙ}:
  ∀ i, j: datoms_visible(αᵢ, B.cut) = datoms_visible(αⱼ, B.cut)
  (all participants see the same datom set at the cut)
```

#### Level 1 (State Invariant)
A resolved barrier guarantees that all participants have exchanged all
datoms up to the cut point. Non-monotonic queries at this cut produce
identical results regardless of which participant evaluates them.

**Falsification**: Two participants at a resolved barrier producing different
results for the same non-monotonic query.

**Stateright model**: 3 agents with different initial datom sets. Run barrier
protocol. Verify post-barrier query determinism across all agents.

---

### INV-SYNC-002: Barrier Timeout Safety

**Traces to**: ADRS PO-010
**Verification**: `V:PROP`
**Stage**: 3

#### Level 0 (Algebraic Law)
```
∀ barriers B with timeout T:
  B resolves within T OR B times out with status :timed-out
  No barrier hangs indefinitely.
```

#### Level 1 (State Invariant)
A barrier always terminates — either by resolution (all respond) or by
timeout (deadline reached). The timed-out barrier records which participants
responded and which did not, for crash-recovery (PD-003).

**Falsification**: A barrier that neither resolves nor times out.

---

### INV-SYNC-003: Barrier Is Topology-Independent

**Traces to**: ADRS PD-005, SQ-005
**Verification**: `V:MODEL`
**Stage**: 3

#### Level 0 (Algebraic Law)
```
∀ topologies T₁, T₂, ∀ participant sets P, ∀ datom sets D:
  barrier(P, D, T₁).cut = barrier(P, D, T₂).cut
  (the consistent cut depends only on participants and datoms, not topology)
```

#### Level 1 (State Invariant)
Star, ring, mesh, and hierarchical topologies all produce the same consistent
cut for the same inputs.

**Falsification**: Two different topologies producing different cuts for the same
participants and datom sets.

**Stateright model**: Run barrier protocol under 3 topologies (star, ring, mesh).
Verify identical cuts.

---

### INV-SYNC-004: Barrier Entity Provenance

**Traces to**: ADRS FD-012
**Verification**: `V:PROP`
**Stage**: 3

#### Level 0 (Algebraic Law)
```
∀ barrier operations:
  ∃ Barrier entity in the store recording:
    participants, status, timeout, cut (if resolved), responses
```

#### Level 1 (State Invariant)
Every barrier — resolved or timed-out — produces a Barrier entity in the store.
The barrier history is queryable.

**Falsification**: A barrier operation that completes without creating a Barrier entity.

---

### INV-SYNC-005: Non-Monotonic Queries Require Barrier

**Traces to**: ADRS SQ-004
**Verification**: `V:PROP`
**Stage**: 3

#### Level 0 (Algebraic Law)
```
∀ queries Q with mode = Barriered(barrier_id):
  barrier_id references a resolved Barrier entity
  Q is evaluated against barrier.cut (not local frontier)
```

#### Level 1 (State Invariant)
A Barriered query mode requires a valid, resolved barrier. The query engine
rejects Barriered queries referencing unresolved or timed-out barriers.

**Falsification**: A Barriered query executing against a timed-out or nonexistent barrier.

---

### §8.5 ADRs

### ADR-SYNC-001: Barrier as Explicit Coordination Point

**Traces to**: ADRS SQ-001, PO-010
**Stage**: 3

#### Problem
How to handle non-monotonic queries that depend on the absence of facts?

#### Options
A) **Always consistent** — all queries require global consistency. Too expensive.
B) **Never consistent** — all queries are local frontier. Non-monotonic results vary.
C) **Explicit barriers** — monotonic queries run locally; non-monotonic queries can
   optionally use a barrier for consistency.

#### Decision
**Option C.** Most queries (Strata 0–1) are monotonic and need no coordination.
Non-monotonic queries (Strata 2–5) produce useful approximate results at local
frontier but can use a barrier when precision is critical.

#### Formal Justification
CALM theorem: monotonic programs have coordination-free implementations.
Barriers are needed only for non-monotonic queries where correctness
depends on knowing what is NOT present.

---

### ADR-SYNC-002: Topology-Agnostic Protocol

**Traces to**: ADRS PD-005
**Stage**: 3

#### Problem
Should the sync protocol prescribe a topology?

#### Decision
No. The protocol provides primitives (initiate, report, exchange, resolve).
Topology emerges from deployment. Single-agent (trivial — barrier with self),
bilateral (two agents exchange), flat swarm (all-to-all), hierarchy (tree) are
all valid using the same primitives.

#### Formal Justification
Prescribing topology limits applicability. The invariant (INV-SYNC-003) that
results are topology-independent means the protocol can support any topology
without changing the correctness guarantees.

---

### ADR-SYNC-003: Barrier Timeout Over Blocking

**Traces to**: ADRS PO-010
**Stage**: 3

#### Problem
What happens when a barrier participant doesn't respond?

#### Decision
Timeouts. Every barrier has a deadline. Unresponsive participants cause
timeout, not deadlock. The timed-out barrier records who responded,
enabling crash-recovery (PD-003) — the recovering agent can query the
barrier record to understand what was missed.

---

### §8.6 Negative Cases

### NEG-SYNC-001: No Unbounded Barrier Wait

**Traces to**: ADRS PO-010
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ barrier that blocks indefinitely)`
Every barrier either resolves or times out within its declared timeout.

**proptest strategy**: Create barriers with varying participant counts and
response patterns. Verify all complete within timeout.

---

### NEG-SYNC-002: No Barrier at Inconsistent Cut

**Traces to**: ADRS PO-010
**Verification**: `V:MODEL`

**Safety property**: `□ ¬(∃ resolved barrier where participants disagree on datom set)`

**Stateright model**: 3 agents with partial connectivity. Run barrier protocol.
Verify that resolution only occurs when all participants have identical visible sets.

---

---

