> **Namespace**: RESOLUTION | **Wave**: 1 (Foundation) | **Stage**: 0
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §4. RESOLUTION — Per-Attribute Conflict Resolution

### §4.0 Overview

Conflict resolution in Braid is per-attribute, not global. Different attributes have
different semantics and different natural resolution strategies. The resolution layer
operates at query time over the LIVE index, not during merge (merge is pure set union).

**Traces to**: SEED.md §4 Axiom 5
**docs/design/ADRS.md sources**: FD-005, CR-001–007

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
**Verification**: `V:TYPE`, `V:PROP`, `V:KANI`
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
**Verification**: `V:PROP`, `V:KANI`
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

**Stage 0 simplification**: At Stage 0, multi-agent conflicts are rare (single agent
only). Steps (1)-(3) are fully implemented: assert Conflict entity, compute severity,
route to resolution tier. Steps (4)-(6) produce stub datoms recording that the step
ran but performing no work: (4) TUI notification is deferred to Stage 4 (TUI layer),
(5) uncertainty update is deferred to Stage 1 (uncertainty tensor requires BUDGET),
(6) cache invalidation is deferred to Stage 1 (caching is an optimization). The L0
invariant holds — all 6 steps produce datoms — but steps 4-6 produce metadata-only
datoms at Stage 0. Full behavior activates progressively: Stage 1 adds (5)+(6),
Stage 4 adds (4).

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

**Traces to**: INV-RESOLUTION-005, ADR-STORE-013, ADRS CR-009
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

### ADR-RESOLUTION-006: Delegation Threshold Formula

**Traces to**: SEED §6, ADRS UA-004
**Stage**: 2

#### Problem
How should the system determine whether a spec element is safe for an agent to work on
autonomously, or whether it requires consultation, delegation, or human escalation? The
delegation decision depends on multiple factors — graph centrality, uncertainty, conflict
exposure — and needs a principled combination formula.

#### Options
A) **Single-factor threshold** — delegate based solely on uncertainty level. Simple but
   ignores structural importance (a low-uncertainty but highly-connected entity is still
   risky to delegate).
B) **Multi-factor weighted formula** — combine betweenness centrality, in-degree,
   consequential uncertainty, and conflict surface into a single threshold score, with
   weights configurable as datoms.
C) **Rule-based classification** — a decision tree with hard-coded cutoffs per factor.
   Interpretable but brittle and not adaptable to different project topologies.

#### Decision
**Option B.** The delegation threshold is computed as:

```
threshold = 0.3 × betweenness + 0.2 × in_degree + 0.3 × σ_c + 0.2 × conflict_surface
```

where `conflict_surface` = fraction of the entity's cardinality-one attributes that are
currently in conflict. The four weights are stored as datoms and configurable per deployment.

The threshold feeds into a four-class delegation classification (ADR-RESOLUTION-007):
- `delegatable`: resolvers > 0 AND uncertainty < 0.2
- `contested`: resolvers > 0 AND uncertainty >= 0.2
- `escalated`: resolvers = 0 AND uncertainty < 0.5
- `human-required`: resolvers = 0 AND uncertainty >= 0.5

#### Formal Justification
The multi-factor formula captures four independent risk dimensions:
- **Betweenness** (0.3): structural importance — high-betweenness entities are bottlenecks
  whose incorrect modification cascades through the dependency graph.
- **In-degree** (0.2): direct dependence — entities referenced by many others carry higher
  revision cost.
- **σ_c** (0.3): consequential uncertainty — entities whose downstream DAG contains high
  uncertainty are riskier to modify (changes propagate through uncertain territory).
- **Conflict surface** (0.2): active contention — entities with many cardinality-one
  attributes in conflict are actively contested and unsafe for unilateral work.

The weights sum to 1.0 and are stored as datoms (C3: schema-as-data), enabling empirical
calibration without code changes. The formula is topology-agnostic (PD-005), depending
only on graph metrics computable at any agent's local frontier.

#### Consequences
- Delegation decisions are reproducible: any agent with the same frontier computes the
  same threshold for the same entity
- Weights are empirically tunable — Stage 0 uses defaults; later stages calibrate from
  observed delegation outcomes (successful autonomous work vs. conflicts discovered post-hoc)
- The formula degrades gracefully with incomplete frontiers: missing graph data increases
  uncertainty terms, biasing toward conservative (non-delegation) classification

#### Falsification
The formula is wrong if: (1) entities classified as `delegatable` consistently produce
conflicts when worked on autonomously (threshold too permissive), or (2) entities classified
as `human-required` are routinely resolved without human intervention (threshold too
conservative). Calibration should reduce both error modes over time.

---

### ADR-RESOLUTION-007: Four-Class Delegation

**Traces to**: SEED §6, ADRS UA-005
**Stage**: 2

#### Problem
How should the system classify work items for agent delegation? A binary delegatable/not
classification is too coarse — it conflates "needs a quick consultation" with "requires
human judgment."

#### Options
A) **Binary classification** — delegatable or not. Simple but forces agents to either
   proceed alone or stop entirely.
B) **Three classes** — self-handle, delegate, escalate. Missing the intermediate "consult"
   class where an agent can proceed but should check with a peer.
C) **Four classes** — self-handle, consult, delegate, escalate to human. Provides granular
   routing that matches the spectrum of uncertainty and authority levels.

#### Decision
**Option C.** Four delegation classes:

1. **Self-handle** — agent has sufficient authority and the entity is stable. Proceed
   autonomously. No coordination required.
2. **Consult** — agent can proceed but should notify a peer agent or check the result.
   The work is not blocked but a second opinion reduces risk.
3. **Delegate** — the entity requires a specialist agent (higher authority in the relevant
   domain per spectral authority UA-003). The current agent should hand off rather than
   attempt the work.
4. **Escalate to human** — uncertainty is too high or no agent has sufficient authority.
   The entity is blocked until a human makes a decision. Creates a Deliberation entity
   (ADR-RESOLUTION-005).

#### Formal Justification
The four classes map to the delegation threshold (ADR-RESOLUTION-006) and the three-tier
conflict routing (ADR-RESOLUTION-004):
- Self-handle corresponds to Tier 1 (automatic) — low severity, low uncertainty.
- Consult corresponds to Tier 2 (agent-with-notification) — medium severity.
- Delegate routes to the agent with highest spectral authority for the entity's domain.
- Escalate corresponds to Tier 3 (human-required) — high severity or zero resolvers.

The classification is topology-agnostic (PD-005): in a hierarchy, "delegate" routes upward;
in a swarm, "delegate" routes to the highest-authority peer; in single-agent mode, "consult"
degrades to self-review and "delegate" degrades to "escalate."

#### Consequences
- Agents have a clear protocol for each class — no ambiguity about when to proceed vs. stop
- The classification feeds into the harvest delegation topology (ADR-HARVEST-004):
  self-handle -> self-review, consult -> peer-review, delegate -> specialist, escalate -> human
- Single-agent deployments collapse to two effective classes: self-handle and escalate

#### Falsification
The four classes are wrong if a significant fraction of work items fall into gaps between
classes (e.g., too risky for self-handle but not warranting consultation), or if the consult
class provides no measurable benefit over binary self-handle/escalate.

---

### ADR-RESOLUTION-008: Delegation Safety

**Traces to**: SEED §6, ADRS UA-011
**Stage**: 2

#### Problem
What prevents an agent from beginning work on a spec element that is actively contested
by concurrent agents? Without a safety check, an agent might implement a function whose
signature is being debated in a parallel branch, producing wasted work or conflicting
artifacts.

#### Options
A) **Optimistic delegation** — agents proceed freely; conflicts detected and resolved
   after the fact via the conflict pipeline (§4.2). Simple but wastes effort on contested
   entities.
B) **Pessimistic locking** — agents acquire exclusive locks on entities before working.
   Prevents conflicts but creates contention and deadlock risks.
C) **Delegation safety predicate** — agents check a delegatability predicate before
   beginning work. Not a lock — the check is advisory and based on the agent's local
   frontier. Work on non-delegatable entities is blocked, not prevented by coordination.

#### Decision
**Option C.** An agent MUST NOT begin work on spec element `e` unless `delegatable(e) = true`
at the agent's local frontier.

```
delegatable(e) =
  ∀ a ∈ attributes(e): ¬conflict(e, a)     — no active conflicts on any attribute
  ∧ stability(e) ≥ delegation_threshold      — stability above configured threshold
```

This is a local check — no coordination with other agents required. The predicate is
conservative: it may block work on entities that are actually safe (because the agent's
frontier is behind), but it never permits work on entities that are genuinely contested.

#### Formal Justification
The safety predicate is conservative by the same argument as INV-RESOLUTION-003 (conservative
conflict detection): the agent's local frontier may overestimate conflicts (seeing phantom
conflicts from missing causal paths) but never underestimates them. Therefore:

```
delegatable(F_local, e) = true ⟹ delegatable(F_global, e) = true
```

An entity that passes the local delegatability check is genuinely safe. An entity that fails
may or may not be safe — the agent errs on the side of caution.

This is NOT a lock. Two agents may independently determine that the same entity is
delegatable and begin concurrent work. This is acceptable because their work produces
datoms that merge via set union (C4), and any resulting conflicts are detected by the
conflict pipeline. The safety predicate reduces the frequency of conflicts, not eliminates
them.

#### Consequences
- Agents avoid wasted work on contested entities without requiring a distributed locking
  protocol
- The predicate is computable from local state — no network round-trips or coordination
- False negatives (blocking safe work) decrease as the agent's frontier grows
- The delegation_threshold is configurable as a datom, enabling per-deployment tuning

#### Falsification
The safety predicate is violated if an agent begins implementing a function whose signature
is contested by concurrent planning agents at the global frontier, and the agent's local
frontier contained both conflicting datoms (meaning the predicate should have returned
false but did not). Also violated if the predicate blocks work on entities that are
consistently safe, causing unnecessary starvation.

---

### ADR-RESOLUTION-010: Resolution Capacity Monotonicity

**Traces to**: SEED §6, ADRS UA-012
**Stage**: 2

#### Problem
When uncertainty about an entity increases, should the set of agents authorized to resolve
conflicts on that entity expand, contract, or remain the same? The answer has direct
implications for system liveness: if rising uncertainty reduces resolver capacity, high-
uncertainty entities can become permanently stuck.

#### Options
A) **Static resolver set** — the set of authorized resolvers is fixed regardless of
   uncertainty. Simple but ignores the increased difficulty of resolving high-uncertainty
   conflicts.
B) **Monotonically expanding** — as uncertainty increases, more agents (or higher-authority
   agents) become authorized to resolve. Ensures liveness: the most uncertain entities
   get the most resolution capacity.
C) **Hierarchical escalation** — uncertainty triggers escalation through a fixed hierarchy:
   agent -> team lead -> architect -> human. Requires a predefined hierarchy, violating
   PD-005 (topology-agnostic).

#### Decision
**Option B.** Resolution capacity is monotone in uncertainty:

```
∀ t1 < t2: uncertainty(e, t1) < uncertainty(e, t2) ⟹ resolvers(e, t2) ⊇ resolvers(e, t1)
```

The implementation is topology-agnostic (PD-005):
- In a hierarchy: higher-level agents are added to the resolver set.
- In a swarm: the quorum threshold for resolution votes increases (more agents participate).
- In a market topology: the reputation threshold for resolver eligibility decreases
  (allowing less-established agents to contribute).
- In single-agent mode: the escalation threshold for human involvement decreases.

#### Formal Justification
This decision revises the retracted INV-CASCADE-001 (Transcript 01:1096-1113), which
mandated hierarchical escalation specifically. The revision preserves the essential property
(rising uncertainty -> more resolution capacity) while removing the topology constraint.

The monotonicity property prevents a liveness failure: if rising uncertainty could shrink
the resolver set, an entity could enter a state where it is too uncertain for any agent
to resolve, but no mechanism exists to reduce uncertainty (because resolution is the
mechanism that reduces uncertainty). Monotonic expansion breaks this deadlock by ensuring
that the hardest problems always have at least as many resolvers as easier ones.

#### Consequences
- No entity can become permanently stuck due to rising uncertainty
- The property composes with spectral authority (ADR-RESOLUTION-011): as uncertainty rises,
  the SVD-derived authority threshold for resolver eligibility decreases, naturally expanding
  the resolver set
- Human authority is axiomatically unbounded (UA-003 exception), so the ultimate resolver
  set always includes humans for sufficiently uncertain entities

#### Falsification
Violated if there exist times t1 < t2 where uncertainty(e, t1) < uncertainty(e, t2) but
an agent that was authorized to resolve conflicts on e at t1 is no longer authorized at t2.
Also violated if high-uncertainty entities consistently lack sufficient resolver capacity,
leading to resolution starvation.

---

### ADR-RESOLUTION-011: Spectral Authority via SVD

**Traces to**: SEED §6, ADRS UA-003
**Stage**: 2

#### Problem
How should agent authority over entities be determined? Authority governs who can resolve
conflicts, who reviews harvest candidates, and who is delegated work. The authority model
must be earned (not configured), transitive (contribution to related entities confers
partial authority), and computable from the store.

#### Options
A) **Configured authority** — administrators assign authority levels to agents per entity
   or per domain. Simple but requires manual maintenance, creates bottlenecks, and cannot
   adapt to shifting contribution patterns.
B) **Direct contribution counting** — authority proportional to the number of datoms an
   agent has transacted for an entity. Computable but misses transitive authority: an agent
   who built the foundation that entity X depends on has no authority over X despite
   structural expertise.
C) **Spectral decomposition (SVD)** — compute authority via singular value decomposition
   of the bipartite agent-entity contribution matrix. Captures transitive authority through
   latent factors. Mathematically identical to Latent Semantic Indexing applied to the
   agent-entity matrix.

#### Decision
**Option C.** Authority is computed via truncated SVD of the agent-entity contribution
matrix M, where M[α, e] = weighted contribution of agent α to entity e.

```
M = U Σ V^T    (truncated SVD, k = min(50, agent_count, entity_count))

authority(α, e) = |u_α · Σ · v_e^T|
  where u_α is agent α's row in U, v_e is entity e's row in V
```

The contribution weights in M are modulated by verification status (ADR-RESOLUTION-012):
unverified = 1, witnessed = 2, challenge-confirmed = 3.

**Exception**: Human authority is axiomatically unbounded — humans are not subject to
spectral authority computation. This is the one configurable authority override.

#### Formal Justification
SVD projects agents and entities into a shared latent space where proximity equals
structural similarity. "LSI finds relevant documents; spectral authority finds
authoritative agents." The key property is transitivity: if agent α contributed to
entities A, B, C that are structurally related to entity D, the SVD places α near D
in latent space even though α never directly touched D.

This aligns with the invariant INV-AUTHORITY-001: authority MUST be derived from the
weighted spectral decomposition of the contribution graph, MUST NOT be assigned by
configuration. The SVD-derived authority is:
- **Earned**: based on actual contributions recorded as datoms in the store
- **Transitive**: captures indirect authority through latent factor proximity
- **Computable**: requires only the contribution matrix, which is derivable from
  the store's transaction history
- **Adaptive**: recomputation after new transactions automatically adjusts authority

Truncation to k factors (default: 50) provides regularization — noise in individual
contributions is smoothed out, and the dominant structural patterns in the contribution
graph are amplified.

#### Consequences
- No manual authority configuration required (except the human exception)
- Authority adapts automatically as agents contribute more to different domains
- The LSI analogy provides well-studied mathematical properties and efficient algorithms
- Recomputation cost is O(m * n * k) where m = agents, n = entities, k = truncation rank;
  practical for expected scale (single-digit agents, thousands of entities)
- Creates a virtuous feedback loop with verification weighting (ADR-RESOLUTION-012):
  verified work -> higher authority -> more delegation -> more work -> more verification

#### Falsification
The SVD authority model is wrong if: (1) agents with high spectral authority over an entity
consistently make poor decisions about that entity (authority does not correlate with
competence), or (2) the transitive authority property creates false positives where agents
gain authority over unrelated entities due to spurious latent-factor correlations. Monitor
conflict resolution outcomes by resolver authority to calibrate.

---

### ADR-RESOLUTION-012: Contribution Weight by Verification Status

**Traces to**: SEED §6, ADRS UA-010
**Stage**: 2

#### Problem
Should all contributions to the authority graph be weighted equally, or should verified
contributions carry more weight than unverified ones? The spectral authority computation
(ADR-RESOLUTION-011) takes a contribution matrix as input — the question is how to weight
the entries in that matrix.

#### Options
A) **Uniform weighting** — all contributions weight 1. Simple but treats speculative
   hypothesized datoms the same as challenge-confirmed invariants.
B) **Binary weighting** — verified = 1, unverified = 0. Too harsh: it completely ignores
   unverified contributions, which may be correct but simply not yet reviewed.
C) **Tiered weighting** — weight contributions by their verification status on a three-level
   scale: unverified (1), witnessed/valid (2), challenge-confirmed (3).

#### Decision
**Option C.** Contribution weights in the authority matrix M are:

```
weight(d) = {
  1  if d is unverified
  2  if d is witnessed (INV verified) or witness-valid
  3  if d is challenge-confirmed (challenged and verdict = :confirmed)
}

M[α, e] = Σ_{d ∈ contributions(α, e)} weight(d) × provenance_factor(d)
```

where `provenance_factor` comes from the provenance typing lattice (PD-002):
`:observed` = 1.0, `:derived` = 0.8, `:inferred` = 0.5, `:hypothesized` = 0.2.

#### Formal Justification
The tiered weighting creates a feedback loop that incentivizes verification:

```
verified work → higher contribution weight → higher spectral authority
  → more delegation → more work → more verification opportunities
```

This loop is self-reinforcing and aligns incentives: agents that produce high-quality,
verifiable work accumulate authority faster than agents that produce unverified
contributions. The provenance factor multiplier further distinguishes between observed
facts (high trust) and hypothesized claims (low trust).

The weights (1, 2, 3) are deliberately modest ratios. A challenge-confirmed contribution
is worth 3x an unverified one, not 100x. This prevents verified contributions from
completely dominating the authority matrix while still providing meaningful signal.

#### Consequences
- The authority graph becomes quality-weighted, not just quantity-weighted
- Agents are incentivized to seek verification of their contributions (witnesses, challenges)
- The feedback loop accelerates convergence: verified knowledge stabilizes authority,
  which stabilizes delegation, which stabilizes the work plan
- Provenance typing (PD-002) composes naturally — a hypothesized, unverified contribution
  has effective weight 0.2 × 1 = 0.2, while an observed, challenge-confirmed contribution
  has effective weight 1.0 × 3 = 3.0, a 15:1 ratio

#### Falsification
The weighting is wrong if: (1) the 1/2/3 ratio is too compressed (unverified contributions
dominate despite verification providing meaningful signal), or (2) too spread (verified
agents monopolize authority, preventing newer agents from contributing). Monitor the
distribution of authority scores across agents and calibrate ratios if authority becomes
too concentrated or too diffuse.

---

### ADR-RESOLUTION-013: Conflict Pipeline Progressive Activation

**Traces to**: SEED §10 (staged roadmap), INV-RESOLUTION-008, ADRS CR-011
**Stage**: 0

#### Problem
INV-RESOLUTION-008 requires that the full 6-step conflict pipeline — (1) assert Conflict
entity, (2) compute severity, (3) route to resolution tier, (4) fire TUI notification,
(5) update uncertainty tensor, (6) invalidate caches — produces a datom at every step.
However, steps 4-6 each depend on subsystems that are not available at Stage 0:

- **Step 4** (TUI notification) requires the TUI interaction layer, a Stage 4 deliverable.
  Without a TUI, there is no notification target.
- **Step 5** (uncertainty tensor update) requires the uncertainty computation system, which
  depends on the BUDGET namespace (§13) for σ_c (consequential uncertainty) propagation.
  BUDGET is a Stage 1 deliverable.
- **Step 6** (cache invalidation) requires query result caching infrastructure. Caching is
  a performance optimization deferred to Stage 1; Stage 0 queries are direct store reads
  without a cache layer.

The L0 invariant of INV-RESOLUTION-008 — "all 6 steps produce datoms" — is an audit trail
guarantee. This ADR resolves how to preserve that guarantee when three of the six steps
cannot perform their intended work.

#### Options
A) **Full pipeline implementation** — pull TUI, BUDGET, and caching infrastructure into
   Stage 0 so all 6 steps are fully operational. This violates the staged roadmap: each
   of these subsystems has its own dependency chain (TUI requires terminal rendering,
   BUDGET requires token tracking and attention decay modeling, caching requires
   invalidation tracking and storage management), and none are prerequisites for the
   core Stage 0 hypothesis ("harvest/seed transforms the workflow").

B) **Stub datoms for unavailable steps** — steps 4-6 execute and produce datoms recording
   that the step ran, but the datoms carry metadata-only content rather than performing
   the actual work. The stub datoms preserve the audit trail and establish the pipeline
   skeleton that later stages fill in with real behavior.

C) **Defer entire conflict pipeline to Stage 1** — no conflict detection, severity
   computation, or routing at Stage 0. Conflicts from concurrent assertions are silently
   ignored until Stage 1 provides the full infrastructure. This loses all conflict
   awareness at Stage 0.

D) **Implement steps 1-3 only** — fully implement conflict detection, severity computation,
   and routing, but omit steps 4-6 entirely (no datoms produced for those steps). This
   provides functional correctness for the parts that matter but breaks the L0 invariant
   of INV-RESOLUTION-008 (which requires ALL steps to produce datoms) and loses the
   audit trail for the omitted steps.

#### Decision
**Option B.** At Stage 0, the conflict pipeline executes all 6 steps. Steps 1-3 are
fully implemented:

```
Step 1: assert Conflict entity with conflicting datom references     — FULL
Step 2: compute severity = max(commitment_weight(d₁), commitment_weight(d₂)) — FULL
Step 3: route to {Automatic, AgentNotification, HumanRequired}       — FULL
```

Steps 4-6 produce **stub datoms** — datoms that record the step executed but note that
the actual work is deferred:

```
Step 4: assert [:conflict/tui-notification, :stub, "deferred to Stage 4"]
Step 5: assert [:conflict/uncertainty-update, :stub, "deferred to Stage 1"]
Step 6: assert [:conflict/cache-invalidation, :stub, "deferred to Stage 1"]
```

The stub datoms carry the `:stub` marker value and a human-readable deferral reason.
When the corresponding subsystem activates, the pipeline step replaces stub generation
with real work. The stub datoms themselves are never retracted — they remain in the
store as historical records of when the pipeline was operating in degraded mode.

#### Formal Justification
The L0 invariant of INV-RESOLUTION-008 states:

```
∀ conflict detections:
  Steps (1)-(6) ALL produce datoms in the store.
```

Stub datoms satisfy this invariant unconditionally. The invariant requires datom
*production*, not datom *effect*. A stub datom recording "step 5 ran but uncertainty
tensor is not yet available" is still a datom in the store, satisfying the audit trail
guarantee.

The functional correctness of the conflict pipeline is preserved because the
correctness-critical steps are 1-3:
- **Step 1** (detection): Identifies which datom pairs are in conflict. This is the
  core safety property — undetected conflicts lead to silent data corruption.
- **Step 2** (severity): Determines how serious the conflict is, enabling prioritization.
- **Step 3** (routing): Determines who resolves the conflict, enabling escalation.

Steps 4-6 are **notification and bookkeeping** — they improve the agent's awareness of
conflicts and keep derived state fresh, but they do not affect the correctness of conflict
detection or resolution. Specifically:
- Skipping step 4 means agents learn about conflicts only through explicit queries, not
  push notifications. At Stage 0 (single agent), the agent can poll for conflicts.
- Skipping step 5 means the uncertainty tensor is not updated when conflicts occur. In
  single-agent Stage 0, the uncertainty tensor is not yet used for delegation decisions
  (ADR-RESOLUTION-006 is Stage 2), so stale uncertainty has no downstream effect.
- Skipping step 6 means query caches are not invalidated after conflicts. At Stage 0,
  there is no query cache (caching is a Stage 1 optimization), so there is nothing to
  invalidate.

INV-MERGE-010 (cascade determinism) is preserved because stub datom generation is a
deterministic function of the merged store state: given the same conflict, the same stub
datom is produced regardless of which agent runs the cascade.

#### Consequences
- **Stale uncertainty tensor**: When conflicts are detected at Stage 0, the uncertainty
  tensor (σ) is NOT updated. This means entity uncertainty values do not reflect active
  conflicts. At Stage 0 this has no operational impact because the uncertainty tensor is
  not yet used for delegation decisions, spectral authority computation, or budget
  allocation. At Stage 1, when BUDGET activates, step 5 must be implemented before
  uncertainty-based decisions become reliable.
- **No push notifications**: Agents learn about conflicts only by querying for Conflict
  entities, not through real-time TUI notifications. In single-agent Stage 0, this is
  acceptable — the agent can include a conflict check in its seed assembly. In multi-agent
  Stage 3+, TUI notifications become important for timely conflict awareness.
- **No cache staleness**: Since Stage 0 has no query cache, the omission of step 6 has
  zero functional impact. When caching is introduced at Stage 1, step 6 must activate
  simultaneously to prevent stale cache reads after conflicts.
- **Progressive activation schedule**:
  - Stage 1: Steps 5 (uncertainty update) and 6 (cache invalidation) become fully operational.
  - Stage 4: Step 4 (TUI notification) becomes fully operational.
- **Audit trail completeness**: The stub datoms create a complete chronological record
  of pipeline execution, including the degraded-mode period. Future queries can distinguish
  real pipeline effects from stubs via the `:stub` marker, enabling accurate retrospective
  analysis of conflict handling quality across stages.

#### Falsification
This simplification is inadequate if: (1) a Stage 0 agent makes a consequential decision
based on an entity's uncertainty value that is stale due to undetected conflict-driven
uncertainty increase (i.e., the uncertainty tensor's staleness causes a real downstream
error), or (2) multiple conflicts accumulate at Stage 0 without agent awareness because
the polling-based discovery mechanism is insufficient (agents consistently fail to check
for conflicts, and push notifications would have caught the issue), or (3) the stub datom
overhead (three additional datoms per conflict detection) materially impacts Stage 0 store
size for workloads with high conflict rates.

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

