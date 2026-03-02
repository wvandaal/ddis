> **Namespace**: RESOLUTION | **Wave**: 1 (Foundation) | **Stage**: 0
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §4. RESOLUTION — Per-Attribute Conflict Resolution

### §4.0 Overview

Conflict resolution in Braid is per-attribute, not global. Different attributes have
different semantics and different natural resolution strategies. The resolution layer
operates at query time over the LIVE index, not during merge (merge is pure set union).

**Traces to**: SEED.md §4 Axiom 5
**ADRS.md sources**: FD-005, CR-001–007

---

### §4.1 Level 0: Algebraic Specification

#### Resolution as Semilattice

```
For each attribute a, the resolution mode defines a join-semilattice (V_a, ⊔_a):

Mode LWW (Last-Writer-Wins):
  V_a ordered by HLC timestamp
  v₁ ⊔ v₂ = v with max(v₁.tx, v₂.tx)
  Identity: ⊥ = no assertion

Mode Lattice:
  V_a ordered by a user-defined lattice L
  v₁ ⊔ v₂ = join_L(v₁, v₂)
  Identity: L.bottom

Mode Multi (Multi-Value):
  V_a = P(V) — power set of values
  v₁ ⊔ v₂ = v₁ ∪ v₂
  Identity: ∅

All three modes form semilattices, preserving CRDT semantics:
  ⊔ is commutative, associative, and idempotent.
```

#### Conflict Predicate

```
conflict(d₁, d₂) =
  d₁ = [e, a, v₁, t₁, Assert] ∧
  d₂ = [e, a, v₂, t₂, Assert] ∧
  v₁ ≠ v₂ ∧
  cardinality(a) = :one ∧
  ¬(t₁ < t₂) ∧ ¬(t₂ < t₁)

Critical: conflict requires CAUSAL INDEPENDENCE.
If one tx causally precedes the other, it is an update, not a conflict.
```

#### Resolution Composition

```
∀ attributes a, ∀ stores S:
  resolved_value(S, e, a) = resolution_mode(a).resolve(
    {d.v | d ∈ S, d.e = e, d.a = a, d.op = Assert, ¬retracted(S, d)}
  )

where retracted(S, d) = ∃ r ∈ S: r.e = d.e, r.a = d.a, r.v = d.v,
                                   r.op = Retract, r.tx > d.tx
```

---

### §4.2 Level 1: State Machine Specification

#### Three-Tier Conflict Routing

```
When conflict(d₁, d₂) is detected:

1. Compute severity = max(commitment_weight(d₁), commitment_weight(d₂))

2. Route by severity:
   TIER 1 — Automatic (low severity):
     Apply attribute's resolution mode (LWW/lattice/multi).
     Record resolution as datom. No human/agent notification.

   TIER 2 — Agent-with-Notification (medium severity):
     Apply resolution mode. Fire notification signal.
     Agent may override via deliberation.

   TIER 3 — Human-Required (high severity):
     Block resolution. Create Deliberation entity.
     Surface via TUI. Await human decision.

Severity thresholds are configurable as datoms.
```

#### Conflict Detection Pipeline

```
On MERGE or TRANSACT:
  1. For each new datom d = [e, a, v, tx, Assert] with cardinality(a) = :one:
     a. Find existing datom d' = [e, a, v', tx', Assert] where v ≠ v'
     b. Check causal independence: ¬(tx < tx') ∧ ¬(tx' < tx)
     c. If independent → assert Conflict entity
  2. Compute severity for each conflict
  3. Route to appropriate tier
  4. Update uncertainty (conflict increases σ_a for affected entity)
  5. Fire notification signals
  6. Invalidate cached query results for affected entities
```

#### Conservative Detection Invariant

```
conflicts_detected(frontier_local) ⊇ conflicts_actual(frontier_global)

Proof sketch: The causal-ancestor relation is monotonically growing.
Learning about new causal paths can only resolve apparent concurrency (discover that
two assertions are actually causally related), never create new concurrency.
An agent may waste effort on phantom conflicts (safe) but never miss a real one (critical).
```

---

### §4.3 Level 2: Interface Specification

```rust
/// Per-attribute resolution mode.
#[derive(Clone)]
pub enum ResolutionMode {
    /// Last-writer-wins, ordered by specified clock.
    Lww { clock: LwwClock },
    /// Join-semilattice resolution.
    Lattice { lattice_id: EntityId },
    /// Keep all values (cardinality :many semantics).
    Multi,
}

#[derive(Clone, Copy)]
pub enum LwwClock {
    Hlc,        // Hybrid Logical Clock (default)
    Wall,       // Wall-clock time
    AgentRank,  // Agent authority ranking
}

/// Conflict entity.
pub struct Conflict {
    pub entity: EntityId,
    pub attribute: Attribute,
    pub values: Vec<(Value, TxId)>,  // competing values with their transactions
    pub severity: f64,
    pub tier: ConflictTier,
    pub status: ConflictStatus,      // lattice: :detected < :routing < :resolving < :resolved
}

pub enum ConflictTier {
    Automatic,
    AgentNotification,
    HumanRequired,
}

/// Resolution result.
pub struct Resolution {
    pub conflict: EntityId,
    pub resolved_value: Value,
    pub method: ResolutionMethod,     // :lww | :lattice | :deliberation | :human
    pub rationale: String,
}

impl LiveIndex {
    /// Resolve current value for a cardinality-one attribute.
    pub fn resolve(&self, entity: EntityId, attr: &Attribute, schema: &Schema) -> Option<Value> {
        let mode = schema.resolution_mode(attr);
        let candidates = self.unretracted_values(entity, attr);
        mode.resolve(&candidates)
    }
}
```

#### CLI Commands

```
braid conflicts                           # List all unresolved conflicts
braid conflicts --entity <id>             # Conflicts for specific entity
braid resolve <conflict-id> --value <v>   # Manually resolve a conflict
braid resolve <conflict-id> --auto        # Apply automatic resolution
```

---

### §4.4 Invariants

### INV-RESOLUTION-001: Per-Attribute Resolution

**Traces to**: SEED §4 Axiom 5, ADRS FD-005
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ attributes a: ∃ resolution_mode(a) ∈ {LWW, Lattice, Multi}
  (every attribute declares its conflict resolution strategy)

resolution_mode is an attribute of the attribute entity:
  (attr_entity, :db/resolutionMode, mode, tx, Assert)
```

#### Level 1 (State Invariant)
No attribute exists without a declared resolution mode. The default (if not explicitly
set) is LWW with HLC clock.

#### Level 2 (Implementation Contract)
```rust
impl Schema {
    pub fn resolution_mode(&self, attr: &Attribute) -> ResolutionMode {
        self.attribute(attr)
            .and_then(|def| def.resolution_mode)
            .unwrap_or(ResolutionMode::Lww { clock: LwwClock::Hlc })
    }
}
```

**Falsification**: A conflict arising on an attribute with no defined resolution mode
and no default applied.

---

### INV-RESOLUTION-002: Resolution Commutativity

**Traces to**: ADRS AS-001, FD-005
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ resolution modes M, ∀ value sets V₁, V₂:
  M.resolve(V₁ ∪ V₂) = M.resolve(V₂ ∪ V₁)
  (resolution is order-independent — critical for CRDT consistency)
```

#### Level 1 (State Invariant)
Two agents independently resolving the same conflict arrive at the same value,
regardless of the order in which they receive the conflicting datoms.

**Falsification**: Two agents with the same datom set producing different resolved
values for the same entity-attribute pair.

**proptest strategy**: Generate random sets of conflicting values, resolve in all
permutations, assert identical results.

---

### INV-RESOLUTION-003: Conservative Conflict Detection

**Traces to**: ADRS CR-001
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ frontiers F_local, F_global where F_local ⊆ F_global:
  conflicts(F_local) ⊇ conflicts(F_global)
  (local frontier overestimates conflicts — no false negatives)
```

#### Level 1 (State Invariant)
An agent at a local frontier may see phantom conflicts (safe — wasted effort).
It MUST NOT miss real conflicts (critical — silent data corruption).

**Falsification**: A real conflict at the global frontier that is not detected at
some agent's local frontier that has received both conflicting datoms.

**Stateright model**: Model 3 agents independently transacting conflicting values.
Verify that every merge detects all conflicts, even with partial frontier views.

---

### INV-RESOLUTION-004: Conflict Predicate Correctness

**Traces to**: ADRS CR-006
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
conflict(d₁, d₂) ⟺
  same_entity(d₁, d₂) ∧ same_attribute(d₁, d₂) ∧
  different_value(d₁, d₂) ∧ both_assert(d₁, d₂) ∧
  cardinality_one(d₁.a) ∧ causally_independent(d₁.tx, d₂.tx)
```

#### Level 1 (State Invariant)
The conflict predicate requires ALL six conditions. Missing any condition
either misses real conflicts or flags non-conflicts:
- Without causal independence check: updates falsely flagged as conflicts
- Without cardinality check: multi-value attributes falsely flagged
- Without same-entity check: unrelated datoms falsely paired

**Falsification**: A pair of datoms satisfying all six conditions not flagged as conflict,
or a pair violating any condition that IS flagged.

**proptest strategy**: Generate datom pairs with systematic variation of each condition.
Verify conflict predicate matches expected boolean.

---

### INV-RESOLUTION-005: LWW Semilattice Properties

**Traces to**: ADRS FD-005
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
For LWW resolution with clock C:
  Commutativity: lww(v₁, v₂) = lww(v₂, v₁)
  Associativity: lww(lww(v₁, v₂), v₃) = lww(v₁, lww(v₂, v₃))
  Idempotency:   lww(v, v) = v
```

#### Level 1 (State Invariant)
LWW picks the value with the highest clock value. Ties broken by agent ID.

**Falsification**: Two agents resolving the same LWW conflict to different values.

---

### INV-RESOLUTION-006: Lattice Join Correctness

**Traces to**: ADRS SR-010, FD-005
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
For lattice resolution with lattice L:
  join_L(v₁, v₂) ≥ v₁ AND join_L(v₁, v₂) ≥ v₂    — upper bound
  ∀ u: u ≥ v₁ ∧ u ≥ v₂ ⟹ u ≥ join_L(v₁, v₂)       — least upper bound
  join_L(v₁, v₂) = join_L(v₂, v₁)                   — commutativity
```

#### Level 1 (State Invariant)
The lattice join produces the least upper bound of the competing values.
For diamond lattices (INV-SCHEMA-008), the join of two incomparable elements
produces the error signal element.

**Falsification**: A lattice join that is not the least upper bound, or
incomparable values in a diamond lattice that don't produce the error signal.

---

### INV-RESOLUTION-007: Three-Tier Routing Completeness

**Traces to**: ADRS CR-002
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ detected conflicts C:
  C is routed to exactly one of {Automatic, AgentNotification, HumanRequired}
  No conflict remains unrouted.
```

#### Level 1 (State Invariant)
Every detected conflict has a severity and a routing tier. The routing is total
(all conflicts are routed) and deterministic (same severity → same tier).

**Falsification**: A conflict that is detected but not routed to any tier.

---

### INV-RESOLUTION-008: Conflict Entity Datom Trail

**Traces to**: ADRS CR-003
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ conflict detections:
  Steps (1) assert Conflict entity, (2) compute severity, (3) route,
  (4) fire TUI, (5) update uncertainty, (6) invalidate caches
  ALL produce datoms in the store.
```

#### Level 1 (State Invariant)
The full conflict lifecycle is recorded as datoms, making it queryable
and auditable.

**Falsification**: Any step in the conflict pipeline that does not produce a datom.

---

### §4.5 ADRs

### ADR-RESOLUTION-001: Per-Attribute Over Global Policy

**Traces to**: SEED §4 Axiom 5, §11, ADRS FD-005
**Stage**: 0

#### Problem
Should conflict resolution be per-attribute or global?

#### Options
A) **Per-attribute** — each attribute declares its resolution mode (LWW, lattice, multi).
B) **Global policy** — one resolution strategy for all attributes.

#### Decision
**Option A.** Different attributes have different semantics. Task status has a natural
lattice (`todo < in-progress < done`). Person names do not. Forcing one resolution
policy on all attributes either loses information or produces nonsense.

#### Formal Justification
Per-attribute resolution preserves the semilattice property at the attribute level.
Global LWW would lose lattice semantics for status-like attributes. Global lattice
would require defining a lattice for every attribute, including those with no natural order.

---

### ADR-RESOLUTION-002: Resolution at Query Time, Not Merge Time

**Traces to**: C4, ADRS AS-001
**Stage**: 0

#### Problem
When does conflict resolution happen?

#### Options
A) **At merge time** — resolve conflicts during MERGE operation.
B) **At query time** — MERGE is pure set union; resolution happens in the LIVE index.

#### Decision
**Option B.** MERGE must be pure set union (C4). Conflict resolution at merge time
would make MERGE depend on schema and resolution mode, breaking the algebraic
properties (L1–L3 assume set union).

#### Formal Justification
If MERGE resolves conflicts, then `MERGE(S₁, S₂)` depends on schema — but schema is
itself data in the store. This creates a circular dependency that breaks L1–L3.
Resolution at query time avoids this: MERGE is always set union, and LIVE applies
resolution modes.

---

### ADR-RESOLUTION-003: Conservative Detection Over Precise

**Traces to**: ADRS CR-001
**Stage**: 0

#### Problem
Should conflict detection be conservative (may overcount) or precise (exact)?

#### Options
A) **Conservative** — flag potential conflicts even when uncertain.
   May waste effort on phantom conflicts. Never misses real conflicts.
B) **Precise** — only flag actual conflicts. Requires global knowledge.

#### Decision
**Option A.** The cost of a missed conflict (silent data corruption) far exceeds
the cost of a phantom conflict (wasted investigation effort). Conservative detection
is safe under partial information (local frontiers).

#### Formal Justification
Causal-ancestor relation is monotonically growing. Learning about new causal paths
can only resolve apparent concurrency, never create it. Conservative detection is
safe at any frontier.

---

### ADR-RESOLUTION-004: Three-Tier Routing

**Traces to**: ADRS CR-002
**Stage**: 0

#### Problem
How should conflicts be escalated?

#### Decision
Three tiers based on severity (commitment weight of conflicting datoms):
1. **Automatic** (low) — lattice/LWW per attribute. Recorded as datom.
2. **Agent-with-notification** (medium) — automatic + notification signal.
3. **Human-required** (high) — blocks. Creates Deliberation entity.

Severity = `max(commitment_weight(d₁), commitment_weight(d₂))`.
Thresholds configurable as datoms.

---

### ADR-RESOLUTION-005: Deliberation as Entity

**Traces to**: ADRS CR-004
**Stage**: 2

#### Problem
How to record conflict resolution decisions?

#### Decision
Three entity types: Deliberation (process), Position (stance), Decision (outcome).
Deliberation history forms a case law system — past decisions inform future conflicts
via the precedent query pattern (CR-007).

---

### §4.6 Negative Cases

### NEG-RESOLUTION-001: No Merge-Time Resolution

**Traces to**: C4
**Verification**: `V:TYPE`, `V:PROP`

**Safety property**: `□ ¬(∃ MERGE operation that applies conflict resolution)`
MERGE is pure set union. Conflict resolution happens at query time in the LIVE index.

**Rust type-level enforcement**: `fn merge(&mut self, other: &Store)` has no `Schema`
parameter. It cannot access resolution modes.

**proptest strategy**: Merge two stores with conflicting values. Verify both values
are present in the merged datom set (no automatic resolution during merge).

---

### NEG-RESOLUTION-002: No False Negative Conflict Detection

**Traces to**: ADRS CR-001
**Verification**: `V:MODEL`

**Safety property**: `□ ¬(∃ real conflict that is not detected at any frontier containing both datoms)`

**Stateright model**: 3 agents, each transacting conflicting values for shared entities.
Model all possible interleavings. Verify: if both conflicting datoms are in an agent's
frontier, the conflict is detected.

---

### NEG-RESOLUTION-003: No Resolution Without Provenance

**Traces to**: ADRS CR-003
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ conflict resolution that is not recorded as a datom)`
Every resolution — automatic, agent, or human — produces a datom trail.

**proptest strategy**: Trigger conflicts via random transactions. Verify every resolution
produces a Resolution entity in the store.

---

