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
    /// Last-writer-wins, ordered by HLC (default). Clock variant (HLC/Wall/AgentRank)
    /// is a schema-level attribute (:db/lwwClock), not part of this enum — see §2 SCHEMA.
    LastWriterWins,
    /// Join-semilattice resolution — lattice definition stored as datoms (C3).
    Lattice { lattice_id: EntityId },
    /// Keep all values (cardinality :many semantics).
    MultiValue,
}

/// A set of competing assertions for a single (entity, attribute) pair.
/// Detection-stage type: captures the datom-level facts. Routing metadata
/// (severity, tier, status) are computed during the conflict routing pipeline
/// (INV-RESOLUTION-007) and stored as separate datoms, not embedded here.
pub struct ConflictSet {
    pub entity:     EntityId,
    pub attribute:  Attribute,
    pub assertions:  Vec<(Value, TxId)>,  // competing values with their transactions
    pub retractions: Vec<(Value, TxId)>,  // relevant retractions for conservative detection
}

/// Three-tier routing for detected conflicts (INV-RESOLUTION-007).
pub enum RoutingTier {
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

### §4.3.1 Resolution--Merge Composition Proof (R2.2)

The audit concern: if per-attribute resolution operates in the LIVE index layer while
merge operates in the store layer (pure set union), do these compose correctly? That is,
does resolution commute and associate over set-union merge?

**Theorem**: For all resolution modes M and all stores S1, S2, S3:

```
LIVE(MERGE(S1, S2)) = LIVE(MERGE(S2, S1))                         -- commutativity
LIVE(MERGE(MERGE(S1, S2), S3)) = LIVE(MERGE(S1, MERGE(S2, S3)))   -- associativity
```

where LIVE is computed per-attribute using the declared resolution mode.

**Proof sketch by mode**:

Define for entity e and attribute a:
```
candidates(S, e, a) = {d.v | d in S, d.e = e, d.a = a, d.op = Assert, not retracted(S, d)}
```

Since MERGE(S1, S2) = S1 union S2 (set union), the candidate set for MERGE is:
```
candidates(S1 union S2, e, a) = candidates(S1, e, a) union candidates(S2, e, a)
  minus values retracted in S1 union S2
```

Note: retraction membership is also determined by set union -- a retraction datom in
either S1 or S2 is present in S1 union S2. Since set union is commutative and associative,
the candidate set after retraction filtering is commutative and associative over merge.

**Case LWW**: The resolved value is the assertion with the greatest HLC timestamp among
unretracted assertions:
```
lww_resolve(candidates(S1 union S2, e, a)) = max_by_hlc(candidates(S1 union S2, e, a))
```
Since the candidate set is the same regardless of merge order (set union commutativity
and associativity), `max_by_hlc` produces the same result. The `max` function over a
totally-ordered domain (HLC with BLAKE3 tiebreak per ADR-RESOLUTION-009) is commutative,
associative, and idempotent. Therefore LWW resolution composes correctly with set-union
merge.

**Case Lattice**: The resolved value is the lattice join over all unretracted assertions:
```
lattice_resolve(candidates(S1 union S2, e, a)) = join{v | v in candidates(S1 union S2, e, a)}
```
The lattice join is, by the definition of join-semilattice (verified at schema-registration
time per INV-SCHEMA-007), commutative, associative, and idempotent. Since the candidate set
is identical regardless of merge order, and the folded join over any permutation of a set
yields the same result (commutativity + associativity of the join operator), lattice
resolution composes correctly with set-union merge.

**Case Multi**: The resolved value is the set of all unretracted values:
```
multi_resolve(candidates(S1 union S2, e, a)) = candidates(S1 union S2, e, a)
```
This is the identity function on the candidate set. Since the candidate set is
commutative and associative over merge (set union), multi resolution trivially
composes correctly.

**Conclusion**: All three resolution modes form join-semilattices (INV-RESOLUTION-005,
INV-RESOLUTION-006, section 4.1), and each operates on a candidate set derived via set union
(which is itself a semilattice). The composition of two semilattice operations preserves
commutativity and associativity. Therefore, per-attribute resolution in the LIVE index
is fully compatible with set-union merge, and the CRDT laws L1-L3 from section 1 STORE are
preserved end-to-end through to the resolved state visible to agents.

**Corollary**: LIVE is a **derived CRDT**. If S forms a G-Set CvRDT and LIVE is a
monotonic function from S to a per-attribute semilattice, then LIVE inherits strong
eventual consistency from S.

---

### §4.3.2 Conservative Conflict Detection Completeness Proof (R2.5b)

The audit concern: conflict detection operates on local frontiers — partial views of the
global datom set. Can a real conflict be invisible to an agent that holds both conflicting
datoms? This proof shows that the detection predicate is conservative: it may produce false
positives (phantom conflicts) but never false negatives (missed real conflicts).

#### Definitions

**Definition 1 (True conflict).** A pair of datoms `(d₁, d₂)` constitutes a *true conflict*
in store S iff all six conditions of INV-RESOLUTION-004 hold with respect to the complete
causal history:

```
true_conflict(S, d₁, d₂) ⟺
  (1) d₁.e = d₂.e                                           — same entity
  (2) d₁.a = d₂.a                                           — same attribute
  (3) d₁.v ≠ d₂.v                                           — different value
  (4) d₁.op = Assert ∧ d₂.op = Assert                       — both assertions
  (5) cardinality(d₁.a) = :one                               — cardinality-one attribute
  (6) d₁.tx ∥ d₂.tx                                         — causally independent
      where ∥ means: ¬(d₁.tx <_causal d₂.tx) ∧ ¬(d₂.tx <_causal d₁.tx)
      and <_causal is the transitive closure of the predecessor relation
```

**Definition 2 (Frontier).** A frontier F for agent α is a set of datoms:
```
F_α = {d ∈ S | d is visible to α}
```
The global frontier is the full store: `F_global = S`.
A local frontier is a subset: `F_α ⊆ S`.

**Definition 3 (Detection predicate).** At frontier F, the detection predicate is:
```
detect(F, d₁, d₂) ⟺
  (1)-(5) as above, evaluated over F
  (6') ¬(d₁.tx <_F d₂.tx) ∧ ¬(d₂.tx <_F d₁.tx)
       where <_F is the causal order restricted to transactions visible in F
```

The key distinction: condition (6') uses `<_F` (the causal order visible at frontier F),
while condition (6) uses `<_causal` (the true causal order over the complete store).

#### Theorem (Conservative Detection Completeness)

```
∀ stores S, ∀ frontiers F ⊆ S, ∀ datom pairs (d₁, d₂) where d₁ ∈ F ∧ d₂ ∈ F:
  true_conflict(S, d₁, d₂) ⟹ detect(F, d₁, d₂)

Equivalently: no false negatives. If (d₁, d₂) is a true conflict in the global store,
then any frontier containing both datoms detects it.
```

#### Proof

We prove the contrapositive: `¬detect(F, d₁, d₂) ⟹ ¬true_conflict(S, d₁, d₂)`.

If `detect(F, d₁, d₂)` is false, then at least one of conditions (1)-(5) or (6') fails.

**Case A: One of conditions (1)-(5) fails.**
Conditions (1)-(5) depend only on the datoms d₁ and d₂ themselves and on the schema
(specifically the cardinality of the attribute). Since d₁, d₂ ∈ F and schema is globally
consistent (INV-SCHEMA-001: schema is datoms in the store, and schema monotonicity
INV-SCHEMA-003 ensures that schema at F ⊆ schema at S), conditions (1)-(5) evaluate
identically at F and at S. Therefore if any of (1)-(5) fails at F, it also fails at S,
and `true_conflict(S, d₁, d₂)` is false. **QED for Case A.**

**Case B: Conditions (1)-(5) hold at F, but (6') fails.**
Condition (6') fails means:
```
d₁.tx <_F d₂.tx  ∨  d₂.tx <_F d₁.tx
```
That is, there exists a causal path from one transaction to the other *that is visible
within F*. Since F ⊆ S, every causal path visible in F is also a valid causal path in S:

```
Lemma (Causal Path Monotonicity):
  ∀ F ⊆ S: d₁.tx <_F d₂.tx ⟹ d₁.tx <_causal d₂.tx

Proof: <_F is the transitive closure of the predecessor relation restricted to
transactions in F. <_causal is the transitive closure of the FULL predecessor relation
in S. Since F ⊆ S, every predecessor edge in F is also an edge in S. Therefore every
path in F is a path in S, and <_F ⊆ <_causal. □
```

So if `d₁.tx <_F d₂.tx`, then `d₁.tx <_causal d₂.tx`, which means condition (6)
of `true_conflict` fails. Therefore `true_conflict(S, d₁, d₂)` is false. **QED for Case B.**

Combining Cases A and B, the contrapositive holds. By contraposition, the theorem holds. **QED.**

#### Key insight: why the converse does NOT hold (false positives are possible)

The detection predicate can flag conflicts that are not true conflicts. This happens when:

```
d₁.tx ∥_F d₂.tx  BUT  d₁.tx <_causal d₂.tx

That is: at frontier F, no causal path from d₁.tx to d₂.tx is visible (because
intermediate transactions carrying the causal chain have not yet been merged into F),
so the detector conservatively flags a conflict. But in the full store, a causal path
exists — d₁ is actually an update superseded by d₂, not a concurrent conflict.
```

This is exactly the situation described in the proof sketch of §4.2: the causal-ancestor
relation is *monotonically growing* as an agent's frontier expands. Learning about new
causal paths can only *resolve* apparent concurrency (turning phantom conflicts into
recognized updates), never *create* new concurrency.

Formally:
```
∀ F₁ ⊆ F₂ ⊆ S:
  conflicts_detected(F₂) ⊆ conflicts_detected(F₁)

Proof: If d₁.tx ∥_{F₁} d₂.tx but d₁.tx <_{F₂} d₂.tx, then the conflict detected
at F₁ is resolved at F₂ (the causal path is now visible). No new conflict can appear
at F₂ that was not at F₁, because <_{F₁} ⊆ <_{F₂} — strictly MORE causal paths are
visible, which can only reduce concurrency, never increase it. □
```

**Corollary (Anti-monotonicity of detected conflicts):** The set of detected conflicts
is anti-monotone in the frontier: as an agent receives more datoms, its detected conflict
set can only shrink (or stay the same), never grow. This means:

```
conflicts_detected(F_local) ⊇ conflicts_detected(F_global)
```

which is exactly INV-RESOLUTION-003.

#### Relationship between conflict detection and resolution modes

The detection predicate (Definition 3) is the same for all resolution modes — it identifies
*which* (entity, attribute) pairs have causally independent competing assertions. What differs
per mode is how those conflicts are **resolved**:

**LWW conflicts**: Two causally independent assertions for a cardinality-one LWW attribute.
The conflict is detected, but *immediately resolved* by picking the assertion with the
greater HLC timestamp (ties broken by BLAKE3 hash per ADR-RESOLUTION-009). The conflict
entity is created and immediately resolved in a single cascade. Conservative detection
ensures neither assertion is silently lost — even if the "losing" value is discarded by
LWW, the fact that a conflict occurred is recorded as a datom (INV-RESOLUTION-008).

**Lattice conflicts**: Two causally independent assertions for a cardinality-one lattice
attribute. If the values are comparable in the lattice order, the lattice join resolves
them immediately (no real conflict). If the values are incomparable, the join produces
the lattice top element — which for diamond lattices (INV-SCHEMA-008) is an error signal
element triggering escalation. Conservative detection ensures that all incomparable
pairs are detected; the lattice join ensures that comparable pairs are silently resolved.

**Multi-value**: Cardinality-many attributes never trigger the conflict predicate
(condition (5) requires cardinality :one). All values coexist. There is no conflict
to detect, and the detection predicate correctly returns false.

**Summary of mode-detection interaction:**

```
Mode      | Detection fires? | Resolution        | Severity routing
----------|------------------|-------------------|------------------
LWW       | Yes              | max(HLC)          | Tier 1 (automatic)
Lattice   | Yes (incomp.)    | join / error-top  | Tier 1 or 2
Multi     | No (card :many)  | all values kept   | N/A
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
            .unwrap_or(ResolutionMode::LastWriterWins)
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

Tie-breaking (equal clock values):
  When C(d₁) = C(d₂), the winner is the datom whose BLAKE3 hash is
  lexicographically greater: lww(d₁, d₂) = d where blake3(d) = max(blake3(d₁), blake3(d₂)).
  This preserves commutativity and associativity because byte comparison is a total order.
  See ADR-RESOLUTION-009.
```

#### Level 1 (State Invariant)
LWW picks the value with the highest clock value. When two concurrent assertions
have identical HLC timestamps, the tie is broken by BLAKE3 hash comparison of the
datom content (ADR-RESOLUTION-009). This is deterministic and topology-agnostic:
both agents independently compute the same winner without coordination.

**Falsification**: Two agents resolving the same LWW conflict to different values,
including when the conflicting datoms have identical HLC timestamps.

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

### ADR-RESOLUTION-009: BLAKE3 Hash Tie-Breaking for LWW

**Traces to**: INV-RESOLUTION-005, ADR-STORE-013
**Stage**: 0

#### Problem
When two concurrent assertions have identical HLC timestamps for a cardinality-one
LWW attribute, the resolution is undefined. Without a deterministic tie-breaker, two
agents resolving the same conflict could pick different winners, violating
INV-RESOLUTION-002 (commutativity) and breaking CRDT convergence.

#### Options
A) **BLAKE3 hash of datom content** — compute `blake3([e, a, v, tx, op])` for each
   competing datom; the datom with the lexicographically greater hash wins. Deterministic,
   content-addressable, consistent with ADR-STORE-013 (BLAKE3 already used for EntityId).
B) **Agent ID lexicographic comparison** — break ties by comparing the agent ID embedded
   in the HLC timestamp. Simple but creates an implicit agent hierarchy: agents with
   lexicographically later IDs always win ties, incentivizing ID manipulation.
C) **Leave undefined** — let implementations choose. Breaks CRDT convergence guarantee
   across heterogeneous implementations.

#### Decision
**Option A.** BLAKE3 hash comparison provides a deterministic, topology-agnostic total
order over datoms. Both agents independently compute the same winner without coordination.
The hash is already computed for EntityId generation (ADR-STORE-013), so the marginal
cost is zero for entities and negligible for the full datom tuple.

#### Formal Justification
The tie-breaking rule preserves all three CRDT semilattice properties required by
INV-RESOLUTION-005:

- **Commutativity**: `max(blake3(d₁), blake3(d₂)) = max(blake3(d₂), blake3(d₁))` —
  byte comparison is commutative under max.
- **Associativity**: `max(max(h₁, h₂), h₃) = max(h₁, max(h₂, h₃))` — max over a
  total order is associative.
- **Idempotency**: `max(h, h) = h` — trivially holds.

BLAKE3's collision resistance (2^{-128} birthday bound) makes hash equality between
distinct datoms practically impossible. In the astronomically unlikely event of a
hash collision, both agents still agree (they compute the same hash), so convergence
is preserved.

#### Consequences
- No new dependencies (BLAKE3 already required by ADR-STORE-013)
- No agent hierarchy — tie-breaking is content-derived, not identity-derived
- Deterministic across all implementations that use BLAKE3 (the only permitted hash per ADR-STORE-013)
- The tie-breaking case is rare in practice (requires exact HLC equality, which implies
  near-simultaneous assertions on different agents with identical logical clock state)

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

