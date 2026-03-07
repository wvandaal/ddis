> **Namespace**: MERGE | **Wave**: 2 (Lifecycle) | **Stage**: 3
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §7. MERGE — Store Merge & CRDT

### §7.0 Overview

Merge combines knowledge from independent agents. The core operation is set union (C4),
but merge also triggers a cascade of consequences: conflict detection, cache invalidation,
uncertainty updates, and subscription notifications. The branching extension (W_α, patch
branches) provides isolated workspaces with explicit commit to the shared store.

**Traces to**: SEED.md §6
**ADRS.md sources**: AS-001, AS-003–006, AS-010, PD-001, PD-004, PO-006, PO-007

---

### §7.1 Level 0: Algebraic Specification

#### Core Merge

```
MERGE : Store × Store → Store
MERGE(S₁, S₂) = S₁ ∪ S₂

Properties (from STORE namespace, restated for completeness):
  L1: MERGE(S₁, S₂) = MERGE(S₂, S₁)           — commutativity
  L2: MERGE(MERGE(S₁, S₂), S₃) = MERGE(S₁, MERGE(S₂, S₃))  — associativity
  L3: MERGE(S, S) = S                             — idempotency
  L4: S ⊆ MERGE(S, S')                            — monotonicity
```

#### Branching Extension

```
Branching G-Set: (S, B, ⊑, commit, combine)
  S = trunk (shared store, a G-Set over D)
  B = set of branches, each a G-Set over D
  ⊑ = ancestry relation
  commit : Branch × S → S'
  combine : Branch × Branch → Branch

Properties:
  P1 (Monotonicity):    commit(b, S) ⊇ S
  P2 (Isolation):       ∀ b₁ ≠ b₂: visible(b₁) ∩ branch_only(b₂) = ∅
  P3 (Combination commutativity): combine(b₁, b₂) = combine(b₂, b₁)
  P4 (Commit-combine equivalence): commit(combine(b₁, b₂), S) = commit(b₂, commit(b₁, S))
  P5 (Fork snapshot):   b.base = S|_{frontier(t_fork)}
```

#### Working Set (W_α)

```
Each agent α maintains private W_α using the same datom structure as S.

Local query view: visible(α) = W_α ∪ S
Commit: commit(W_α, S) = S ∪ selected(W_α)   — agent chooses what to commit

W_α datoms are NOT included in MERGE operations.
W_α datoms are invisible to other agents.
```

---

### §7.2 Level 1: State Machine Specification

#### Merge Cascade

```
MERGE(S₁, S₂) → S'

POST (set union):
  S'.datoms = S₁.datoms ∪ S₂.datoms

INFRASTRUCTURE PRECONDITION (not a cascade step):
  REBUILD SCHEMA:
     If merge introduced schema datoms (any datom with a = :db/ident, :db/valueType,
     :db/cardinality, :db/resolutionMode, or :db/doc on a schema entity):
       Schema::from_store(merged_datoms) → new Schema
     Schema rebuild occurs as part of Store construction (ADR-SCHEMA-005) before
     cascade steps execute. Schema is owned by Store, so a new Store value (with its
     new Schema) is produced by the merge. This is structural, not a cascade step.

CASCADE (exactly 5 steps, all produce datoms):
  1. DETECT CONFLICTS:
     For each new datom d entering from the merge:
       if conflict(d, d_existing) → assert Conflict entity
  2. INVALIDATE CACHES:
     Mark query results as stale for entities affected by new datoms
  3. MARK STALE PROJECTIONS:
     Existing projection patterns touching affected entities → refresh needed
  4. RECOMPUTE UNCERTAINTY:
     σ(e) updated for entities with new assertions or conflicts
  5. FIRE SUBSCRIPTIONS:
     Notify subscribers whose patterns match new datoms
```

#### Branch Operations

```
Six sub-operations:

FORK(S, agent, purpose) → Branch
  POST: branch.base_tx = current frontier
        branch.status = :active
        branch entity created in S

COMMIT(branch, S) → S'
  PRE:  branch.status = :active
        if branch.competing_with ≠ ∅: comparison/deliberation completed
  POST: S' = S ∪ branch.datoms
        branch.status = :committed

COMBINE(b₁, b₂, strategy) → Branch
  strategies:
    Union — b₁.datoms ∪ b₂.datoms
    SelectiveUnion — agent-curated subset
    ConflictToDeliberation — conflicts → Deliberation entity
  POST: result preserves properties P1–P4

REBASE(branch, S_new) → Branch'
  POST: branch'.base_tx = S_new.frontier
        branch' sees trunk datoms up to S_new.frontier

ABANDON(branch) → ()
  POST: branch.status = :abandoned (datom, not deletion)

COMPARE(branches, criterion) → BranchComparison
  criteria: FitnessScore | TestSuite | UncertaintyReduction | AgentReview | Custom
  POST: BranchComparison entity created with scores, winner, rationale
```

#### Competing Branch Lock

```
∀ branches b₁, b₂ where b₁.competing_with = b₂:
  COMMIT(b₁, S) is BLOCKED until:
    ∃ BranchComparison c: c.branches ⊇ {b₁, b₂} ∧ c.winner is decided
  OR:
    ∃ Deliberation d resolving the competition

This prevents first-to-commit from winning by default.
```

---

### §7.3 Level 2: Interface Specification

```rust
/// Branch entity.
pub struct Branch {
    pub id: EntityId,
    pub ident: String,
    pub base_tx: TxId,
    pub agent: AgentId,
    pub status: BranchStatus,       // lattice: :active < :proposed < :committed < :abandoned
    pub purpose: String,
    pub competing_with: Vec<EntityId>,
    pub datoms: BTreeSet<Datom>,
}

pub enum CombineStrategy {
    Union,
    SelectiveUnion { selected: Vec<Datom> },
    ConflictToDeliberation,
}

pub enum ComparisonCriterion {
    FitnessScore,
    TestSuite,
    UncertaintyReduction,
    AgentReview,
    Custom(String),
}

pub struct BranchComparison {
    pub branches: Vec<EntityId>,
    pub criterion: ComparisonCriterion,
    pub scores: HashMap<EntityId, f64>,
    pub winner: Option<EntityId>,
    pub rationale: String,
}

/// Merge receipt — records the set-union operation (INV-MERGE-009).
/// Cascade side-effects are tracked separately in CascadeReceipt.
pub struct MergeReceipt {
    pub new_datoms:      usize,                                   // Stage 0
    pub duplicate_datoms: usize,                                  // Stage 0
    pub frontier_delta:  HashMap<AgentId, (Option<TxId>, TxId)>,  // Stage 0
}

/// Cascade receipt — records the 5-step cascade (INV-MERGE-002).
/// Returned alongside MergeReceipt from the full merge operation.
pub struct CascadeReceipt {
    pub conflicts_detected: usize,       // Stage 0 — count; Vec<ConflictSet> in Stage 2
    pub caches_invalidated: usize,       // Stage 0
    pub projections_staled: usize,       // Stage 0
    pub uncertainties_updated: usize,    // Stage 0
    pub notifications_sent: usize,       // Stage 0
    pub cascade_datoms: Vec<Datom>,      // Stage 0 — datom trail per INV-MERGE-002
}

// --- Free functions (ADR-ARCHITECTURE-001) ---

/// Merge another store into target (set union + cascade).
/// Free function: merge is a set-algebraic operation with its own cascade
/// sequence. Both stores are explicit parameters, making the semantics clear.
/// Returns (MergeReceipt, CascadeReceipt) — merge statistics and cascade effects.
pub fn merge(target: &mut Store, source: &Store) -> (MergeReceipt, CascadeReceipt);

// Stage 2 branch operations — also free functions per ADR-ARCHITECTURE-001.
// Signatures shown in free function form; implementation deferred to Stage 2.

/// Create a branch (Stage 2).
pub fn fork(store: &mut Store, agent: AgentId, purpose: &str) -> Result<Branch, BranchError>;

/// Commit a branch to trunk (Stage 2).
pub fn commit_branch(store: &mut Store, branch: &Branch) -> Result<TxReceipt, BranchError>;

/// Compare branches (Stage 2).
pub fn compare_branches(
    store: &Store,
    branches: &[EntityId],
    criterion: ComparisonCriterion,
) -> Result<BranchComparison, BranchError>;
```

#### CLI Commands

```
braid merge --from <store-path>       # Merge another store
braid branch create "experiment-x"    # Fork a branch
braid branch list                     # List all branches
braid branch commit <branch>          # Commit branch to trunk
braid branch compare <b1> <b2>        # Compare two branches
braid branch abandon <branch>         # Mark branch as abandoned
```

---

### §7.4 Invariants

### INV-MERGE-001: Merge Is Set Union

**Traces to**: SEED §4, C4, ADRS AS-001
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ S₁, S₂: MERGE(S₁, S₂).datoms = S₁.datoms ∪ S₂.datoms
  (no heuristics, no resolution, no filtering — pure set union)
```

#### Level 1 (State Invariant)
The merge operation at the store level is exactly set union. All conflict detection,
resolution, and cascade effects are post-merge operations, not part of merge itself.

**Falsification**: A merge operation that produces a datom set different from the
mathematical set union of the two input sets.

---

### INV-MERGE-002: Merge Cascade Completeness

**Traces to**: ADRS PO-006
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ merge operations MERGE(S₁, S₂):
  all 5 cascade steps execute:
    (1) conflict detection, (2) cache invalidation,
    (3) projection staleness, (4) uncertainty update,
    (5) subscription notification
  all cascade steps produce datoms
```

#### Level 1 (State Invariant)
No cascade step is skipped. Each step produces datoms recording its effects.
The merge cascade is atomic — either all 5 steps complete or the merge fails.

**Falsification**: A merge that completes without running conflict detection,
or a cascade step that produces no datom trail.

**Stateright model**: Model merge operations between 3 agents. Verify that
every merge triggers all 5 cascade steps in all interleavings.

---

### INV-MERGE-003: Branch Isolation

**Traces to**: ADRS AS-003, AS-004
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
∀ branches b₁, b₂ where b₁ ≠ b₂:
  branch_datoms(b₁) ∩ visible(b₂) = ∅
  (branches cannot see each other's uncommitted datoms)
```

#### Level 1 (State Invariant)
A query against branch b₁ never returns datoms from branch b₂.
Branch visibility is exactly: `{trunk datoms at fork point} ∪ {b₁'s own datoms}`.

**Falsification**: A query against branch b₁ returning a datom from b₂.

**proptest strategy**: Create two branches from the same fork point. Add different
datoms to each. Verify queries against each branch see only their own datoms.

---

### INV-MERGE-004: Competing Branch Lock

**Traces to**: ADRS AS-005, PO-007
**Verification**: `V:PROP`, `V:KANI`, `V:MODEL`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
∀ branches b₁, b₂ where b₁.competing_with = b₂:
  COMMIT(b₁) is BLOCKED until:
    ∃ comparison or deliberation resolving {b₁, b₂}
```

#### Level 1 (State Invariant)
A branch marked as competing with another branch cannot be committed
until a BranchComparison or Deliberation entity exists that resolves
the competition.

**Falsification**: A competing branch committed without a prior comparison or deliberation.

**Stateright model**: Two competing branches, two agents. Verify that no
interleaving allows commit without comparison.

---

### INV-MERGE-005: Branch Commit Monotonicity

**Traces to**: ADRS AS-003 Property P1
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
∀ branch commits: commit(b, S) ⊇ S
  (committing a branch only adds datoms to trunk)
```

#### Level 1 (State Invariant)
Branch commit is a union operation: trunk grows, never shrinks.

**Falsification**: A branch commit that removes datoms from trunk.

---

### INV-MERGE-006: Branch as First-Class Entity

**Traces to**: ADRS AS-005
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
∀ branches b: b is an entity in the datom store with:
  :branch/ident, :branch/base-tx, :branch/agent,
  :branch/status, :branch/purpose, :branch/competing-with
```

#### Level 1 (State Invariant)
Branch metadata is queryable via the same Datalog engine as any other data.
The `:branch/competing-with` attribute enables the competing branch lock.

**Falsification**: A branch whose metadata is not queryable via Datalog.

---

### INV-MERGE-007: Bilateral Branch Duality

**Traces to**: ADRS AS-006
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
```
The DCC (diverge-compare-converge) pattern works identically:
  Forward: spec → competing implementations → selection
  Backward: implementation → competing spec updates → selection
Same algebraic structure, same comparison machinery.
```

#### Level 1 (State Invariant)
If the system supports branching for implementation alternatives, it must
also support branching for specification alternatives.

**Falsification**: The system supports implementation branches but requires
linear (non-branching) spec modifications.

---

### INV-MERGE-008: At-Least-Once Idempotent Delivery

**Traces to**: ADRS PD-004
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ stores S, R:
  MERGE(MERGE(S, R), R) = MERGE(S, R)
  (duplicate delivery produces same result — idempotency from L3)
```

#### Level 1 (State Invariant)
Duplicate merge operations are harmless. An agent that receives the same
datoms twice produces the same store state as receiving them once.

**Falsification**: A duplicate merge that changes the store state.

---

### INV-MERGE-009: Merge Receipt Completeness

**Traces to**: SEED §6, ADRS PO-006
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ merge operations M = MERGE(S₁, S₂):
  receipt(M) = (|S₁ ∪ S₂| - |S₁|, |S₁ ∩ S₂|, Δfrontier(S₁, S₁ ∪ S₂))
  (every merge produces a receipt documenting new datom count,
   duplicate count, and per-agent frontier delta)
```

#### Level 1 (State Invariant)
Every merge operation returns a `MergeReceipt` that accurately records what
changed: the number of new datoms introduced, the number of duplicates
deduplicated, and the per-agent frontier advancement. The receipt is a
deterministic function of the pre-merge and post-merge store states.

#### Level 2 (Interface Constraint)
```rust
pub struct MergeReceipt {
    pub new_datoms:      usize,
    pub duplicate_datoms: usize,
    pub frontier_delta:  HashMap<AgentId, (Option<TxId>, TxId)>,
}
```

The `merge()` function MUST return a `MergeReceipt`. A merge that completes
without returning a receipt, or a receipt whose fields do not match the actual
store delta, violates this invariant.

**Falsification**: A merge operation that (a) does not return a receipt,
(b) returns a receipt where `new_datoms` differs from the actual count of
datoms added, or (c) returns a receipt where `frontier_delta` does not
reflect the actual frontier change.

---

### INV-MERGE-010: Cascade Determinism

**Traces to**: ADRS PO-006, ADR-MERGE-005
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
CASCADE : Store → Set<Datom>
CASCADE(S₁ ∪ S₂) = CASCADE(S₂ ∪ S₁)    — follows from S₁ ∪ S₂ = S₂ ∪ S₁

∀ stores S, ∀ agents α, β:
  cascade_α(S) = cascade_β(S) = CASCADE(S)
  (cascade output depends ONLY on the merged datom set, not on
   which agent executes the cascade, when it executes, or any
   external non-deterministic state)
```

This property is what restores L1 (commutativity) and L2 (associativity)
at the full post-cascade store level. INV-MERGE-001 guarantees the datom
set union is commutative. INV-MERGE-010 guarantees the cascade layer
preserves that commutativity. Together: the total post-merge state
(datoms + cascade datoms) is commutative and associative.

#### Level 1 (State Invariant)
Every cascade step (conflict detection, cache invalidation, projection
staleness, uncertainty update, subscription notification) is a pure
function of the merged store state. No cascade step reads or incorporates:
- The executing agent's identity
- Wall-clock timestamps or HLC values generated during cascade
- Random values or external I/O results
- The order in which input stores were supplied to merge

Cascade datoms carry provenance (the merge transaction's TxId), but
this TxId is itself deterministic: it is derived from the content of
the merge operation (content-addressable identity, INV-STORE-003).

**Falsification**: Two agents independently merging the same two stores
produce different cascade datom sets, OR a cascade step that reads
`AgentId::current()`, `SystemTime::now()`, or any non-store state to
produce its output datoms.

#### Level 2 (Implementation Contract)
```rust
/// Cascade functions take ONLY the merged store. No agent ID, no clock,
/// no RNG. The function signature enforces determinism by construction.
pub fn run_cascade(store: &Store, new_datoms: &[Datom]) -> CascadeReceipt {
    let conflicts   = detect_conflicts(store, new_datoms);       // &Store only
    let invalidated = invalidate_caches(store, new_datoms);      // &Store only
    let stale       = mark_projections_stale(store, new_datoms); // &Store only
    let uncertainty  = update_uncertainties(store, new_datoms);   // &Store only
    let notifications = compute_notifications(store, new_datoms);// &Store only
    CascadeReceipt { conflicts, invalidated, stale, uncertainty, notifications }
}

// Cascade datom identity is content-addressable from the conflict/change itself,
// not from who detected it or when:
fn conflict_datom(entity: EntityId, attr: Attribute, v1: &Value, v2: &Value) -> Datom {
    // Deterministic: same conflict always produces same datom
    Datom::new(conflict_entity_id(entity, attr, v1, v2), ...)
}
```

**proptest harness**: Merge two stores in both orders (A∪B, B∪A), run
cascade on each result, verify identical cascade datom sets.

---

### §7.5 ADRs

### ADR-MERGE-001: Set Union Over Heuristic Merge

**Traces to**: C4, ADRS AS-001
**Stage**: 0

#### Problem
How should stores be merged?

#### Options
A) **Pure set union** — mathematical operation. Conflicts detected post-merge.
B) **Resolution during merge** — apply conflict resolution during merge.
C) **Selective merge** — agent chooses which datoms to accept.

#### Decision
**Option A.** MERGE is `S₁ ∪ S₂`. Conflict detection and resolution are separate
operations (RESOLUTION namespace) that run after merge completes. This preserves
L1–L3 (CRDT properties) and avoids making merge depend on schema.

#### Formal Justification
Option B makes MERGE depend on resolution modes (schema), creating a circular dependency:
merge needs schema, schema is data in the store, store is modified by merge. Option A
breaks this cycle: merge is pure set union, resolution is query-time.

---

### ADR-MERGE-002: Branching G-Set Extension

**Traces to**: ADRS AS-003
**Stage**: 2

#### Problem
How do agents get isolated workspaces?

#### Decision
The pure G-Set is extended to a Branching G-Set with five properties (P1–P5).
Branches are G-Sets themselves, preserving all CRDT properties. Trunk monotonicity
is preserved: `commit(b, S) ⊇ S`.

#### Formal Justification
The extension preserves the core G-Set properties while adding isolation.
Each branch is a G-Set that can be composed with trunk via union (commit).

---

### ADR-MERGE-003: Competing Branch Lock

**Traces to**: ADRS AS-005, PO-007
**Stage**: 2

#### Problem
How to prevent first-to-commit from winning by default?

#### Decision
Branches can declare `:branch/competing-with` pointing to another branch.
Competing branches MUST NOT commit until a BranchComparison or Deliberation
resolves the competition.

#### Formal Justification
Without the lock, the first agent to commit "wins" by making its datoms part
of trunk. The competing branch then sees those datoms and may be unable to
diverge. The lock ensures comparison before commitment.

---

### ADR-MERGE-004: Three Combine Strategies

**Traces to**: ADRS PO-007
**Stage**: 2

#### Problem
How to combine two branches?

#### Decision
Three strategies: Union (merge both), SelectiveUnion (agent curates),
ConflictToDeliberation (conflicts become Deliberation entities).

ConflictToDeliberation opens a structured resolution process instead
of forcing an immediate choice.

---

### ADR-MERGE-005: Cascade as Post-Merge Deterministic Layer

**Traces to**: C4, INV-STORE-004, INV-MERGE-010
**Stage**: 0

#### Problem
Merge cascade (5 post-merge steps) produces metadata datoms. If these datoms
carry agent-specific or time-specific information, then `MERGE(A,B) + cascade
≠ MERGE(B,A) + cascade` — breaking L1 (commutativity) and L2 (associativity)
at the total post-merge store level.

#### Options
A) **Cascade as separate deterministic layer** — cascade is a pure function
   of the merged datom set. L1/L2 hold for merge by construction (set union);
   cascade preserves them because its input is identical regardless of merge order.

B) **Content-addressable cascade datoms** — cascade datoms derive identity from
   the conflict/change itself, not from who detected them. Makes cascade datoms
   identical across agents, but loses provenance (who detected, when).

C) **Cascade as query-layer computation** — no cascade datoms at all; conflicts,
   staleness, etc. computed on-the-fly at query time. Cleanest algebra but no
   audit trail and slower queries.

D) **Cascade included in merge definition** — prove L1/L2 for the full
   merge+cascade operation end-to-end. Requires cascade to produce zero
   non-deterministic content; tightly couples merge and cascade.

#### Decision
**Option A.** The standard CRDT approach: the semilattice state (datom set) is
separate from derived computations (cascade). The cascade is a deterministic
function `CASCADE: Store → Set<Datom>` that reads only the merged store state.
Since `S₁ ∪ S₂ = S₂ ∪ S₁` (INV-MERGE-001), and CASCADE is a pure function,
`CASCADE(S₁ ∪ S₂) = CASCADE(S₂ ∪ S₁)`.

Option B is subsumed by Option A: content-addressable cascade datom identity
is a consequence of the cascade being a pure function of the merged state.
Option C loses the audit trail. Option D creates unnecessary coupling.

#### Formal Justification
The proof that L1 holds at the total post-merge level:
```
Let M = S₁ ∪ S₂ = S₂ ∪ S₁              (set union commutativity)
Total state = M ∪ CASCADE(M)
CASCADE(S₁ ∪ S₂) = CASCADE(M) = CASCADE(S₂ ∪ S₁)  (function of identical input)
∴ M ∪ CASCADE(M) is the same regardless of argument order.  QED.
```

The same argument applies to L2 (associativity): intermediate merges produce
intermediate sets, and CASCADE is applied once to the final merged set —
not at each intermediate step. If CASCADE is applied at intermediate steps,
L2 holds because each intermediate CASCADE is deterministic from its input.

---

### ADR-MERGE-006: Branch Comparison Entity Type

**Traces to**: SEED §6, ADRS AS-010
**Stage**: 2

#### Problem
When competing branches exist (INV-MERGE-004), the system must record why one branch was
selected over another. The comparison process produces structured data: which branches
were compared, what criterion was used, what scores resulted, which branch won, and what
rationale was provided. Where and how should this comparison outcome be stored?

#### Options
A) **Unstructured text** — Record comparison outcomes as free-text datoms on the branch
   entities. Simple but not queryable: "Why was branch X selected?" requires parsing
   prose rather than querying attributes.
B) **Branch Comparison as first-class entity type** — A dedicated entity type with a
   defined schema: `:comparison/branches` (ref :many), `:comparison/criterion`,
   `:comparison/method`, `:comparison/scores` (json), `:comparison/winner` (ref),
   `:comparison/rationale`, `:comparison/agent`. Fully queryable and auditable.
C) **Comparison as transaction metadata** — Attach comparison data to the commit
   transaction's provenance. Ties comparison to the commit action but loses queryability
   outside the transaction context.

#### Decision
**Option B.** Branch Comparisons are first-class entities with the following schema:

```
:comparison/branches    — Ref    :many   — the branches being compared
:comparison/criterion   — Keyword :one   — what was being compared
                                            (e.g., :fitness-score, :test-suite,
                                             :uncertainty-reduction, :agent-review)
:comparison/method      — Keyword :one   — how comparison was conducted
                                            (:automated-test | :fitness-score |
                                             :agent-review | :human-review)
:comparison/scores      — Json    :one   — per-branch scores (structured payload)
:comparison/winner      — Ref     :one   — the selected branch (or nil if undecided)
:comparison/rationale   — String  :one   — human/agent-readable explanation
:comparison/agent       — Ref     :one   — agent who conducted the comparison
```

#### Formal Justification
The competing branch lock (INV-MERGE-004) requires that a comparison or deliberation
exists before a competing branch can commit. Making comparisons first-class entities
means the lock enforcement is a simple Datalog query:

```
[:find ?c
 :where [?c :comparison/branches ?b1]
        [?c :comparison/branches ?b2]
        [?c :comparison/winner ?winner]]
```

If this query returns results for the competing branches, the lock is satisfied. Under
Option A, lock enforcement would require parsing unstructured text — fragile and
non-composable. Under Option C, the comparison is only accessible through the transaction
log, making it invisible to standard queries.

#### Consequences
- BranchComparison entities are stored in the datom store like any other entity
- Comparison history is queryable: "How many times has this branch been compared?"
- The competing branch lock (INV-MERGE-004) is enforced by a Datalog existence check
- Multiple comparisons for the same branch set are allowed (different criteria)
- Deliberation entities (DELIBERATION namespace) can reference comparisons for context
- The `:comparison/scores` field uses Json (ADR-SCHEMA-006) for flexible scoring payloads

#### Falsification
This decision is wrong if: comparison outcomes are so rare or simple that a dedicated entity
type creates unnecessary schema complexity, and a simpler representation (Option A or C)
would be sufficient for all lock enforcement and audit trail needs.

---

### ADR-MERGE-007: Merge Cascade Stub Datoms at Stage 0

**Traces to**: SEED §10 (staged roadmap), INV-MERGE-002, INV-MERGE-010
**Stage**: 0

#### Problem
INV-MERGE-002 (merge cascade completeness) requires that every merge operation executes
all 5 cascade steps, each producing datoms:
1. Conflict detection
2. Query/cache invalidation
3. New conflicts from cascade (projection staleness)
4. Uncertainty deltas
5. Stale projection marking

Step 1 is core to merge correctness — without conflict detection, agents operate on
silently inconsistent state. Steps 2-5 maintain derived state consistency:

- **Step 2** (query/cache invalidation) requires query result caching infrastructure,
  which is a Stage 1 performance optimization. At Stage 0, queries are direct store
  reads without a cache layer.
- **Step 3** (new conflicts from cascade) requires tracking which projections exist and
  detecting when cascade-introduced datoms create secondary conflicts. This depends on
  the projection management system (Stage 1+).
- **Step 4** (uncertainty deltas) requires the uncertainty tensor computation system,
  connected to the BUDGET namespace (§13, Stage 1). Without the uncertainty tensor,
  there is no σ value to update.
- **Step 5** (stale projection marking) requires projection tracking infrastructure
  to know which projections exist and which entities they depend on (Stage 1+).

INV-MERGE-002's L0 invariant is an audit trail guarantee: "all 5 cascade steps produce
datoms." This ADR resolves how to preserve that guarantee when steps 2-5 cannot perform
their intended work.

#### Options
A) **Full cascade implementation** — pull query caching, projection management, and the
   uncertainty tensor into Stage 0 so all 5 cascade steps are fully operational. This
   violates the staged roadmap by adding three substantial subsystems (each with its own
   dependency chain) to Stage 0, whose purpose is validating the core harvest/seed
   hypothesis — not implementing performance optimizations and multi-agent bookkeeping.

B) **Stub datoms for unavailable steps** — steps 2-5 execute and produce datoms recording
   that the step ran, with metadata-only content. Step 1 (conflict detection) is fully
   implemented. The stub datoms preserve the audit trail and the pipeline skeleton that
   later stages expand with real behavior.

C) **Skip cascade entirely at Stage 0** — perform only the set union (INV-MERGE-001),
   with no cascade at all. This violates INV-MERGE-002 directly ("no cascade step is
   skipped") and loses all post-merge metadata, including conflict detection — the one
   cascade step that matters for correctness at every stage.

D) **Implement step 1 only, defer steps 2-5 without stubs** — fully implement conflict
   detection but produce no datoms for steps 2-5. This provides functional conflict
   awareness but breaks the L0 invariant of INV-MERGE-002 (which requires ALL steps to
   produce datoms) and creates a gap in the audit trail: post-hoc analysis cannot
   distinguish "step 2 ran and found nothing to invalidate" from "step 2 was not
   implemented."

#### Decision
**Option B.** At Stage 0, the merge cascade executes all 5 steps. Step 1 is fully
implemented:

```
Step 1: For each new datom d entering from merge:
          if conflict(d, d_existing) → assert Conflict entity     — FULL
```

Steps 2-5 produce **stub datoms** — datoms that record the step executed but performed
no substantive work:

```
Step 2: assert [:cascade/cache-invalidation,  :stub, merge_tx_id, count: 0]
Step 3: assert [:cascade/secondary-conflicts, :stub, merge_tx_id, count: 0]
Step 4: assert [:cascade/uncertainty-delta,    :stub, merge_tx_id, count: 0]
Step 5: assert [:cascade/projection-staleness, :stub, merge_tx_id, count: 0]
```

Each stub datom carries: the cascade step identifier, the `:stub` marker, a reference to
the merge transaction, and a count field (0 at Stage 0, populated with real counts when
the step becomes fully operational). The stub datoms are content-addressable from the
merge transaction identity, preserving INV-MERGE-010 (cascade determinism): given the
same merge, the same stub datoms are produced regardless of which agent executes the
cascade.

#### Formal Justification
INV-MERGE-002's L0 invariant states:

```
∀ merge operations MERGE(S₁, S₂):
  all 5 cascade steps execute
  all cascade steps produce datoms
```

Stub datoms satisfy this invariant. The invariant requires datom *production* at each
step, not that the datom records substantive work. A stub datom asserting "step 2 ran,
0 caches invalidated" is a valid datom in the store.

INV-MERGE-010 (cascade determinism) is preserved because stub datom generation is a
pure function of the merged store state:

```
CASCADE(S₁ ∪ S₂) with stubs:
  Step 1: detect_conflicts(store, new_datoms)  → conflict datoms (deterministic)
  Steps 2-5: stub_datom(step_id, merge_tx_id)  → stub datoms (deterministic from merge_tx_id)

Since merge_tx_id is content-addressable (INV-STORE-003), and stub datoms are
derived from merge_tx_id + step_id, the cascade output is fully deterministic.
∴ CASCADE(S₁ ∪ S₂) = CASCADE(S₂ ∪ S₁)  — preserved.
```

The merge itself (INV-MERGE-001) remains pure set union. The cascade is a post-merge
layer (ADR-MERGE-005). Stub datoms do not alter the merge semantics — they are part
of the cascade layer only.

#### Consequences
- **Stale queries after merge**: At Stage 0, after a merge introduces new datoms, any
  previously-computed query results are NOT invalidated (step 2 is a stub). An agent
  querying after a merge might receive results that do not reflect the merged state.
  In practice this is mitigated by two factors: (a) Stage 0 has no query cache, so
  queries are always fresh reads from the store, and (b) Stage 0 is primarily
  single-agent, so merges are infrequent (self-merges from branch-to-trunk at most).
- **Self-merge residual risk**: Even in single-agent Stage 0, branch-to-trunk commits
  trigger the merge cascade. If the agent has cached query results in application
  memory (not in a formal cache layer), those results become stale after the commit.
  The agent must re-query after any branch commit. This is a discipline requirement,
  not an infrastructure guarantee, until Step 2 activates at Stage 1.
- **No uncertainty propagation**: Merging datoms that introduce conflicts does NOT
  update the uncertainty tensor (σ) for affected entities. At Stage 0, the uncertainty
  tensor is not used for delegation decisions (ADR-RESOLUTION-006, Stage 2) or budget
  allocation (BUDGET, Stage 1), so stale σ values have no downstream effect.
- **No projection refresh**: Existing projections are NOT marked stale after merge.
  At Stage 0, projections are not yet implemented, so there is nothing to mark stale.
  When projections activate (Stage 1+), step 5 must activate simultaneously.
- **Progressive activation schedule**:
  - Stage 1: Steps 2 (cache invalidation), 4 (uncertainty deltas), and 5 (projection
    staleness) become fully operational when their respective subsystems are built.
  - Stage 2+: Step 3 (secondary conflict detection from cascade) activates when the
    projection system is mature enough to generate cascade-induced conflicts.
- **Audit trail completeness**: Stub datoms create a full chronological record of every
  merge cascade, including the degraded-mode period. Future queries can distinguish
  real cascade effects from stubs via the `:stub` marker, enabling retrospective
  analysis of merge cascade quality across the staged rollout.
- **Determinism preserved**: Stub datoms are derived from content-addressable inputs
  (merge_tx_id + step_id), so INV-MERGE-010 holds without modification. The proptest
  harness for INV-MERGE-010 (merge A∪B vs B∪A, verify identical cascade datom sets)
  works identically with stub datoms.

#### Falsification
This simplification is inadequate if: (1) a Stage 0 agent makes a consequential decision
based on a stale query result that would have been invalidated by step 2 in the full
cascade — specifically, if the agent reads an entity's value post-merge and acts on a
pre-merge value because the query was served from stale application-level state, AND this
causes a detectable error (wrong code generated, incorrect spec element written, etc.),
or (2) self-merge operations in Stage 0 are frequent enough (>5 per session) that the
cumulative staleness from unrefreshed projections and uninvalidated caches causes
observable drift between the agent's working state and the store's actual state, or
(3) the 4 stub datoms per merge (one per step 2-5) materially impact store size for
workloads with high merge frequency.

---

### §7.6 Negative Cases

### NEG-MERGE-001: No Merge Data Loss

**Traces to**: C4, L4
**Verification**: `V:KANI`, `V:PROP`

**Safety property**: `□ ¬(∃ d ∈ S₁ ∪ S₂: d ∉ MERGE(S₁, S₂))`
No datom from either input is lost during merge.

**Kani harness**: For all store pairs of size ≤ N, verify merged datom set
is the exact union.

---

### NEG-MERGE-002: No Merge Without Cascade

**Traces to**: ADRS PO-006
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ merge completing without all 5 cascade steps)`

**proptest strategy**: Instrument each cascade step. After merge, verify all 5
were executed and produced datom trails.

---

### NEG-MERGE-003: No Working Set Leak

**Traces to**: ADRS PD-001
**Verification**: `V:PROP`, `V:KANI`

**Safety property**: `□ ¬(∃ W_α datom visible to agent β where α ≠ β)`
Working set datoms are never included in merge operations.

**Kani harness**: For two agents with working sets, verify merge of their
shared stores does not include any working set datom.

---

