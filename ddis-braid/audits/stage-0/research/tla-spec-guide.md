# TLA+ Specification Guide: Braid CRDT Merge Algebra

> **File**: `audits/stage-0/research/braid-crdt.tla`
> **Spec references**: `spec/01-store.md`, `spec/07-merge.md`, `spec/04-resolution.md`
> **Date**: 2026-03-03

---

## 1. What the TLA+ Spec Captures

The specification models Braid's datom store as a replicated G-Set CvRDT across multiple agents.
Each agent maintains a local store (a set of datoms). Agents can:

1. **Transact** -- add new datoms to their local store.
2. **Merge** -- incorporate another agent's store via pure set union.

On top of the raw datom set, the **LIVE function** computes resolved state by applying
per-attribute resolution modes (LWW, Lattice, Multi) to the unretracted candidate values.

### Spec-to-Braid Mapping

| TLA+ Concept | Braid Concept | Spec Reference |
|---|---|---|
| `Datom` record | `[e, a, v, tx, op]` tuple | `spec/01-store.md` section 1.1 |
| `Store` (SUBSET Datom) | G-Set CvRDT `(P(D), \union)` | `spec/01-store.md` section 1.1 |
| `Merge(S1, S2)` | `S1 \union S2` (set union) | `spec/07-merge.md` section 7.1 |
| `IsRetracted(S, d)` | Retraction check: exists `r` with same `(e,a,v)`, `op=retract`, later `tx` | `spec/04-resolution.md` section 4.1 |
| `Candidates(S, e, a)` | Unretracted assert datoms for `(e, a)` | `spec/04-resolution.md` section 4.1 |
| `LwwResolve` | LWW: max-tx assertion, BLAKE3 tiebreak | `spec/04-resolution.md` INV-RESOLUTION-005, ADR-RESOLUTION-009 |
| `LatticeResolve` | User-defined lattice join (LUB) | `spec/04-resolution.md` INV-RESOLUTION-006 |
| `MultiResolve` | Set of all unretracted values | `spec/04-resolution.md` section 4.1 |
| `Live(S)` | `LIVE(S) = fold(causal_sort(S), apply_resolution)` | `spec/01-store.md` section 1.2 |
| `stores` (per-agent) | Agent local replicas | `spec/07-merge.md` section 7.1 (W_alpha) |
| `Transact` action | `TRANSACT(S, agent, datoms, tx_data)` | `spec/01-store.md` section 1.2 |
| `MergeStores` action | `MERGE(S1, S2)` between replicas | `spec/07-merge.md` section 7.1 |

### Properties Checked

| # | Property | Invariant ID | Type |
|---|---|---|---|
| 1 | Merge Commutativity | INV-STORE-004 (L1) | Safety |
| 2 | Merge Associativity | INV-STORE-005 (L2) | Safety |
| 3 | Merge Idempotency | INV-STORE-006 (L3) | Safety |
| 4 | Merge Monotonicity | INV-STORE-007 (L4) | Safety |
| 5 | LIVE Determinism | -- | Safety |
| 6 | Strong Eventual Consistency | -- | Safety |
| 7 | Idempotent Delivery | INV-MERGE-008 | Safety |
| 8 | Resolution-Merge Commutativity | INV-RESOLUTION-002, section 4.3.1 | Safety |
| 9 | Resolution-Merge Associativity | section 4.3.1 | Safety |
| 10 | No Merge Data Loss | NEG-MERGE-001 | Safety |
| 11 | Append-Only Monotonicity | INV-STORE-001 | Temporal |
| 12 | Eventual Convergence | SEC | Liveness |

---

## 2. How to Run the TLA+ Model Checker (TLC)

### Prerequisites

Install the TLA+ Toolbox or the standalone TLC model checker:

```bash
# Option A: TLA+ Toolbox (GUI)
# Download from https://github.com/tlaplus/tlaplus/releases

# Option B: Standalone TLC (command-line)
# Requires Java 11+
wget https://github.com/tlaplus/tlaplus/releases/download/v1.8.0/tla2tools.jar
```

### Creating a TLC Configuration File

Create `braid-crdt.cfg` in the same directory as the `.tla` file:

```
\* braid-crdt.cfg -- TLC model checking configuration

CONSTANTS
    EntityIds   = {"e1", "e2"}
    Attributes  = {"name", "status"}
    Values      = {"v1", "v2", "v3"}
    TxIds       = {1, 2, 3}
    Agents      = {"alpha", "beta"}
    ResolutionModes = [name |-> "lww", status |-> "lattice"]
    LatticeOrder    = {<<"v1", "v2">>, <<"v2", "v3">>, <<"v1", "v3">>}

SPECIFICATION Spec

INVARIANTS
    SafetyProperties

PROPERTIES
    AppendOnlyMonotonicity
    EventualConvergence
```

### Running TLC

```bash
# From the audits/stage-0/research/ directory:

# Standalone TLC (command-line):
java -jar tla2tools.jar -config braid-crdt.cfg -workers auto BraidCRDT

# With increased heap for larger state spaces:
java -Xmx8g -jar tla2tools.jar -config braid-crdt.cfg -workers auto BraidCRDT

# With depth limit (useful for initial exploration):
java -jar tla2tools.jar -config braid-crdt.cfg -workers auto -depth 20 BraidCRDT
```

### Using the TLA+ Toolbox (GUI)

1. Open `braid-crdt.tla` in the Toolbox.
2. Create a new model (Model -> New Model).
3. Set the constants as shown above.
4. Add `SafetyProperties` as an invariant.
5. Add `AppendOnlyMonotonicity` and `EventualConvergence` as temporal properties.
6. Run TLC.

---

## 3. State Space Exploration Bounds

### Recommended Small Configuration (fast exploration)

This configuration explores ~10^4 to 10^5 states and completes in seconds:

```
EntityIds   = {"e1"}
Attributes  = {"a1"}
Values      = {"v1", "v2"}
TxIds       = {1, 2}
Agents      = {"alpha", "beta"}
```

**Purpose**: Verify the spec parses, all operators are well-defined, basic properties hold.

### Recommended Medium Configuration (thorough checking)

This configuration explores ~10^6 to 10^8 states:

```
EntityIds   = {"e1", "e2"}
Attributes  = {"name", "status"}
Values      = {"v1", "v2", "v3"}
TxIds       = {1, 2, 3}
Agents      = {"alpha", "beta"}
```

**Purpose**: Check all CRDT properties with interesting conflict scenarios (two entities,
three values, two agents, mixed resolution modes).

### Recommended Large Configuration (full verification)

This configuration may explore 10^9+ states and requires significant compute:

```
EntityIds   = {"e1", "e2", "e3"}
Attributes  = {"name", "status", "score"}
Values      = {"v1", "v2", "v3", "v4"}
TxIds       = {1, 2, 3, 4}
Agents      = {"alpha", "beta", "gamma"}
```

**Purpose**: Three-agent scenarios needed for associativity edge cases and the
full multi-agent convergence property. This is the configuration that matches
the Stateright models specified in `spec/04-resolution.md` INV-RESOLUTION-003.

### State Space Explosion Mitigation

The state space grows as `|Datom|^(|Agents| * max_store_size)`. Key mitigations:

1. **Symmetry reduction**: TLC can exploit symmetry in `Agents` and `EntityIds`.
   Add to the model: `SYMMETRY Permutations(Agents) \union Permutations(EntityIds)`.

2. **Depth bounding**: Use `-depth N` to limit exploration depth. Start with 15-20.

3. **Action constraints**: Add `CONSTRAINT` to limit maximum store size per agent:
   ```
   CONSTRAINT
       \A a \in Agents : Cardinality(stores[a]) <= 6
   ```

4. **Simulation mode**: For very large configs, use random simulation instead of
   exhaustive checking: `java -jar tla2tools.jar -simulate -depth 100 BraidCRDT`.

---

## 4. How the TLA+ Spec Maps to Braid's Rust Implementation

### Data Structures

| TLA+ | Rust (from spec/01-store.md section 1.3) |
|---|---|
| `Datom` record | `pub struct Datom { entity, attribute, value, tx, op }` |
| `Store == SUBSET Datom` | `pub struct Store { datoms: BTreeSet<Datom>, ... }` |
| `Merge(S1, S2)` | `impl Store { pub fn merge(&mut self, other: &Store) -> MergeReceipt }` |
| `Resolve(S, e, a)` | `impl LiveIndex { pub fn resolve(&self, entity, attr, schema) -> Option<Value> }` |
| `Live(S)` | The LIVE index computed via `fold(causal_sort(S), apply_resolution)` |

### Key Implementation Notes

1. **Content-addressable identity**: In TLA+, datom equality is structural (all five fields).
   In Rust, `Hash` and `Eq` are derived from all five fields, and `BTreeSet`/`HashSet`
   automatically deduplicates. This directly implements the Identity Axiom from section 1.1.

2. **Merge has no Schema parameter**: The TLA+ `Merge` operator takes two `Store` values and
   returns their union with no reference to `ResolutionModes`. In Rust, `fn merge(&mut self, other: &Store)`
   has no `Schema` parameter -- this is enforced at the type level (NEG-RESOLUTION-001).
   Resolution happens only in `LiveIndex::resolve()`, which does take a `Schema`.

3. **LWW tie-breaking**: The TLA+ spec models tie-breaking as `max(value)` for simplicity.
   The Rust implementation uses BLAKE3 hash comparison (ADR-RESOLUTION-009). Both are
   deterministic total orders that preserve commutativity, so the algebraic properties
   verified by TLC transfer to the implementation.

4. **Lattice mode**: The TLA+ spec uses an explicit `LatticeOrder` relation. The Rust
   implementation uses `EntityId`-referenced lattice definitions stored as datoms in the
   schema (INV-SCHEMA-007, INV-SCHEMA-008). The algebraic properties are identical.

5. **Transaction metadata**: The TLA+ spec models transactions as simple datom additions.
   The Rust implementation adds transaction metadata datoms (provenance, causal predecessors)
   via the typestate `Transaction<Building> -> Transaction<Committed> -> Transaction<Applied>`
   pipeline. The additional metadata datoms only strengthen the monotonicity property
   (more datoms added per transaction).

### Verification Strategy Across Tools

The TLA+ spec is one layer in a multi-tool verification strategy (from `spec/00-preamble.md`):

| Level | TLA+ Role | Complementary Tool |
|---|---|---|
| Level 0 (Algebraic) | Verify L1-L4 hold for all reachable store combinations | `proptest` for random input coverage |
| Level 1 (State Machine) | Verify protocol safety/liveness across agent interleavings | `stateright` for Rust-native model checking |
| Level 2 (Implementation) | N/A (TLA+ is above implementation level) | `kani` for bounded model checking on Rust code |

The TLA+ spec verifies that the **design** is correct. `proptest` and `kani` verify that the
**implementation** faithfully implements the design. Together they provide end-to-end assurance.

---

## 5. Modeling Decisions and Simplifications

### What Is Modeled Faithfully

- **G-Set CvRDT**: The store is exactly a grow-only set of datoms under set union.
- **Three resolution modes**: LWW, Lattice, and Multi are all modeled with correct semilattice semantics.
- **Retraction semantics**: Retraction datoms are modeled as first-class datoms (assert + retract).
- **Multi-agent replication**: Each agent has an independent local store; merges are pairwise.
- **All 11 safety properties**: Every relevant invariant from the spec is checked.
- **Liveness**: Eventual convergence under fair scheduling.

### What Is Simplified

1. **HLC timestamps modeled as integers**: Real HLC timestamps are `(wall_time, logical, agent)`.
   The TLA+ spec uses plain integers for `TxIds`, which preserves the total order property
   needed for LWW resolution. The causal ordering aspects of HLC are not modeled (they
   affect conflict *detection*, not conflict *resolution* or merge correctness).

2. **BLAKE3 tie-breaking modeled as max(value)**: Any deterministic total order preserves the
   semilattice properties. The specific choice of BLAKE3 vs. max is irrelevant to the
   algebraic guarantees.

3. **Merge cascade not modeled**: The 5-step merge cascade (conflict detection, cache
   invalidation, projection staleness, uncertainty update, subscription notification) from
   `spec/07-merge.md` section 7.2 is not modeled. These are post-merge effects that do not
   affect the core CRDT properties. INV-MERGE-002 (cascade completeness) should be verified
   separately via a dedicated Stateright model or integration tests.

4. **Branching not modeled**: The branching G-Set extension (fork, commit, combine, rebase,
   abandon) from `spec/07-merge.md` section 7.1 is Stage 2 scope. A separate TLA+ module
   should model branching properties (P1-P5) when Stage 2 work begins.

5. **Working set (W_alpha) not modeled**: Agent working sets are private and excluded from
   merge operations. Their isolation property (NEG-MERGE-003) should be verified separately.

6. **Schema validation not modeled**: The TLA+ spec allows any datom to be transacted.
   Schema validation (type checking, cardinality enforcement) is a precondition on
   `TRANSACT` that does not affect merge or resolution correctness.

---

## 6. Extending the Spec

### Adding Branching (Stage 2)

When Stage 2 implementation begins, extend the spec with:

```tla
VARIABLES branches  \* Function: Agents -> Set of Branch records

Branch == [
    id     : BranchIds,
    base   : TxIds,
    agent  : Agents,
    status : {"active", "proposed", "committed", "abandoned"},
    datoms : SUBSET Datom,
    competing : SUBSET BranchIds
]

\* Verify P1-P5 from spec/07-merge.md section 7.1
BranchMonotonicity == ...     \* P1: commit(b, S) >= S
BranchIsolation    == ...     \* P2: branches can't see each other
CompetingLock      == ...     \* INV-MERGE-004
```

### Adding Causal Ordering

To model the causal independence requirement in conflict detection
(INV-RESOLUTION-004), extend `TxId` to include causal predecessors:

```tla
TxRecord == [id : TxIds, predecessors : SUBSET TxIds]

CausallyIndependent(tx1, tx2) ==
    /\ tx1 \notin TransitiveClosure(tx2.predecessors)
    /\ tx2 \notin TransitiveClosure(tx1.predecessors)
```

This would enable verification of INV-RESOLUTION-003 (conservative conflict detection).

---

## 7. Prior CRDT and Merge Design Discussions (cass/cm Search Results)

### cass search findings

Searches across session history (`cass search "CRDT merge"`, `"set union merge"`,
`"conflict resolution LWW"`, `"join semilattice"`) returned 26 total hits across
multiple DDIS project sessions. Key themes from the session snippets:

1. **APP-INV-081 (CRDT Convergence)** in the Go CLI spec: The existing CLI already
   specifies `merge(A,B) = merge(B,A)` and `merge(merge(A,B),C) = merge(A,merge(B,C))`
   for event streams. This validates that the algebraic properties in Braid's spec
   are consistent with the prior implementation's requirements, even though the
   underlying data model differs (event streams vs. datom sets).

2. **Semilattice merge with LWW conflict resolution**: Multiple sessions discuss the
   LWW resolution strategy applied after set-union merge. The design decision to
   separate merge (set union) from resolution (query-time) was deliberated across
   at least 3 sessions and is now settled as ADR-MERGE-001 / ADR-RESOLUTION-002.

3. **Event-sourcing CRDT design**: The Go CLI's event-sourcing arc (Phase 1-8,
   completed 2026-02-28) implemented CRDT merge for JSONL event streams. Key lesson:
   independent events commute (semilattice property), causally-dependent events form
   a partial order. This directly informs Braid's causal ordering model.

4. **Multi-agent merge complexity**: Session notes flag that "the semilattice merge
   for CRDT convergence is mathematically subtle" and recommend deferring multi-agent
   merge to later phases. Braid's staged roadmap follows this advice: Stage 0 is
   single-agent, Stage 3 introduces multi-agent CRDT merge.

### cm context findings

The `cm context "CRDT merge algebra"` query returned one directly relevant rule:

- **b-mma75m5r**: "DDIS-BRAID uses multi-tool verification strategy: proptest for
  Level 0 (algebraic laws), Stateright/TLA+ for Level 1 (state machines), Kani/Miri
  for Level 2 (Rust implementation)." This confirms the TLA+ spec occupies the
  correct position in the verification hierarchy -- it verifies Level 1 (state machine)
  properties, complemented by proptest (Level 0) and Kani (Level 2).

No anti-patterns or harmful rules were flagged for this topic.

---

*This guide is a companion to `braid-crdt.tla`. Both files trace to the Braid specification
(`spec/01-store.md`, `spec/07-merge.md`, `spec/04-resolution.md`) and should be updated
if the specification's algebraic foundations change.*
