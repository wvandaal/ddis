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

CASCADE (all produce datoms):
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

/// Merge receipt — records what happened during merge.
pub struct MergeReceipt {
    pub datoms_added: usize,
    pub conflicts_detected: Vec<Conflict>,
    pub subscriptions_fired: usize,
    pub stale_projections: usize,
}

impl Store {
    /// Merge another store (set union + cascade).
    pub fn merge(&mut self, other: &Store) -> MergeReceipt;

    /// Create a branch.
    pub fn fork(&mut self, agent: AgentId, purpose: &str) -> Result<Branch, BranchError>;

    /// Commit a branch to trunk.
    pub fn commit_branch(&mut self, branch: &Branch) -> Result<TxReceipt, BranchError>;

    /// Compare branches.
    pub fn compare_branches(
        &mut self,
        branches: &[EntityId],
        criterion: ComparisonCriterion,
    ) -> Result<BranchComparison, BranchError>;
}
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
**Verification**: `V:PROP`, `V:MODEL`
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

