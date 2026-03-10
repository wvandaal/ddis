# §11. Worked Examples

> **Spec references**: Multiple — cross-namespace demonstrations.
> **Purpose**: Demonstrations encode format, style, depth, and domain simultaneously.
> Per prompt-optimization: one example encodes what seven constraints cannot.
> Each example shows both the data AND the LLM-optimized output format.

---

## §11.1 Self-Bootstrap Demonstration

The system's first act: transact its own specification elements as datoms (C7).
The unified self-bootstrap obligation is formalized in INV-TRILATERAL-007
(spec/18-trilateral.md): every trilateral store must bootstrap its own schema,
spec elements, and verification metadata as datoms before any external data.

> **Cross-reference**: This worked example should be verified against
> [spec/18-trilateral.md](../spec/18-trilateral.md) (INV-TRILATERAL-001 through
> INV-TRILATERAL-007) for consistency with the TRILATERAL coherence model.

### Step 1: Genesis

```
$ braid status --format agent

[STATUS] Store: 0 datoms, 0 entities. No genesis transaction.
Schema: uninitialized.
---
↳ Run `braid transact --genesis` to bootstrap the schema. See: INV-SCHEMA-001.
```

```
$ braid transact --genesis --format agent

[STORE] Genesis: 17 axiomatic attributes installed in tx hlc:0-0-system.
Store: 85 datoms (17 attributes × 5 meta-properties each). Frontier: {system: tx_0}.
Schema: 17 attributes. Self-description verified: all meta-schema attributes describe themselves.
---
↳ Genesis complete (INV-SCHEMA-001, INV-SCHEMA-002). Next: define spec-element attributes
  for self-bootstrap. `braid transact --file spec-schema.ednl`
```

### Step 2: Define Spec-Element Attributes

Add attributes for managing specification elements.

> **Note**: Timestamp format (e.g., `#hlc "1709000001000-0-agent1"`) follows canonical EDN
> representation from spec/01b-storage-layout.md. The `#blake3` and `#hlc` reader macros
> are tagged literals in EDN notation.

```clojure
{:e #blake3 "a1b2..." :a :db/ident :v :spec/id :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "a1b2..." :a :db/valueType :v :string :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "a1b2..." :a :db/cardinality :v :one :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "a1b2..." :a :db/doc :v "Spec element ID (e.g., INV-STORE-001)" :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "a1b2..." :a :db/resolutionMode :v :lww :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "c3d4..." :a :db/ident :v :spec/type :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "c3d4..." :a :db/valueType :v :string :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "c3d4..." :a :db/cardinality :v :one :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "c3d4..." :a :db/doc :v "Element type: invariant, adr, negative-case" :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "c3d4..." :a :db/resolutionMode :v :lww :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "e5f6..." :a :db/ident :v :spec/statement :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "e5f6..." :a :db/valueType :v :string :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "e5f6..." :a :db/cardinality :v :one :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "e5f6..." :a :db/resolutionMode :v :lww :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "g7h8..." :a :db/ident :v :spec/namespace :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "g7h8..." :a :db/valueType :v :keyword :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "g7h8..." :a :db/cardinality :v :one :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "g7h8..." :a :db/resolutionMode :v :lww :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "i9j0..." :a :db/ident :v :spec/traces-to :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "i9j0..." :a :db/valueType :v :string :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "i9j0..." :a :db/cardinality :v :many :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "i9j0..." :a :db/resolutionMode :v :multi :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "k1l2..." :a :db/ident :v :spec/falsification :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "k1l2..." :a :db/valueType :v :string :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "k1l2..." :a :db/cardinality :v :one :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "k1l2..." :a :db/resolutionMode :v :lww :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "m3n4..." :a :db/ident :v :spec/verification :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "m3n4..." :a :db/valueType :v :keyword :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "m3n4..." :a :db/cardinality :v :many :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "m3n4..." :a :db/resolutionMode :v :multi :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "o5p6..." :a :db/ident :v :spec/stage :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "o5p6..." :a :db/valueType :v :long :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "o5p6..." :a :db/cardinality :v :one :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "o5p6..." :a :db/resolutionMode :v :lww :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "q7r8..." :a :db/ident :v :spec/depends-on :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "q7r8..." :a :db/valueType :v :ref :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "q7r8..." :a :db/cardinality :v :many :tx #hlc "1709000001000-0-agent1" :op :assert}
{:e #blake3 "q7r8..." :a :db/resolutionMode :v :multi :tx #hlc "1709000001000-0-agent1" :op :assert}
```

```
$ braid transact --file spec-schema.ednl --format agent

[STORE] Transacted 36 datoms (9 spec attributes defined) in tx hlc:1709000001000-0-agent1.
Store: 121 datoms. Schema: 26 attributes (17 axiomatic + 9 spec).
---
↳ Schema extended (INV-SCHEMA-004). Next: transact INV-STORE-001 as the first spec datom.
  Self-bootstrap begins (C7).
```

### Step 3: Transact First Invariant (with Dependencies)

INV-STORE-001 has no spec-element dependencies (it's foundational). But we also
transact INV-STORE-004 (commutativity), which depends on INV-STORE-001 and
INV-STORE-003, to demonstrate dependency edges.

```clojure
;; INV-STORE-001: Append-Only Immutability (no dependencies — foundational)
{:e #blake3 "s9t0..." :a :spec/id :v "INV-STORE-001" :tx #hlc "1709000002000-0-agent1" :op :assert}
{:e #blake3 "s9t0..." :a :spec/type :v "invariant" :tx #hlc "1709000002000-0-agent1" :op :assert}
{:e #blake3 "s9t0..." :a :spec/namespace :v :STORE :tx #hlc "1709000002000-0-agent1" :op :assert}
{:e #blake3 "s9t0..." :a :spec/statement :v "The datom store never deletes or mutates an existing datom. All state changes are new assertions." :tx #hlc "1709000002000-0-agent1" :op :assert}
{:e #blake3 "s9t0..." :a :spec/falsification :v "Any operation that removes a datom or modifies the five-tuple of an existing datom." :tx #hlc "1709000002000-0-agent1" :op :assert}
{:e #blake3 "s9t0..." :a :spec/traces-to :v "SEED §4 Axiom 2" :tx #hlc "1709000002000-0-agent1" :op :assert}
{:e #blake3 "s9t0..." :a :spec/traces-to :v "C1" :tx #hlc "1709000002000-0-agent1" :op :assert}
{:e #blake3 "s9t0..." :a :spec/verification :v :V:PROP :tx #hlc "1709000002000-0-agent1" :op :assert}
{:e #blake3 "s9t0..." :a :spec/verification :v :V:KANI :tx #hlc "1709000002000-0-agent1" :op :assert}
{:e #blake3 "s9t0..." :a :spec/stage :v 0 :tx #hlc "1709000002000-0-agent1" :op :assert}

;; INV-STORE-004: CRDT Merge Commutativity (depends on INV-STORE-001 and INV-STORE-003)
{:e #blake3 "u1v2..." :a :spec/id :v "INV-STORE-004" :tx #hlc "1709000002000-0-agent1" :op :assert}
{:e #blake3 "u1v2..." :a :spec/type :v "invariant" :tx #hlc "1709000002000-0-agent1" :op :assert}
{:e #blake3 "u1v2..." :a :spec/namespace :v :STORE :tx #hlc "1709000002000-0-agent1" :op :assert}
{:e #blake3 "u1v2..." :a :spec/statement :v "CRDT Merge Commutativity" :tx #hlc "1709000002000-0-agent1" :op :assert}
{:e #blake3 "u1v2..." :a :spec/stage :v 0 :tx #hlc "1709000002000-0-agent1" :op :assert}
;; DEPENDENCY EDGES — typed relationships (INV-SCHEMA-009, ADR-SCHEMA-007)
{:e #blake3 "u1v2..." :a :spec/depends-on :v #blake3 "s9t0..." :tx #hlc "1709000002000-0-agent1" :op :assert}
{:e #blake3 "u1v2..." :a :spec/depends-on :v #blake3 "w3x4..." :tx #hlc "1709000002000-0-agent1" :op :assert}
```

> **Key change (INV-SCHEMA-009)**: The `:spec/depends-on` datoms use Ref values pointing
> to the EntityIds of the target spec elements. This builds the spec dependency graph as
> first-class data in the store, not just prose cross-references. The bootstrap EDNL
> generator must extract these relationships from spec markdown and emit them as ref datoms.

```
$ braid transact --file inv-store-001-004.ednl --format agent

[STORE] Transacted 17 datoms (INV-STORE-001 + INV-STORE-004) in tx hlc:1709000002000-0-agent1.
Store: 138 datoms. First spec elements bootstrapped with dependency edges.
Self-reference: the store now contains the invariants that govern the store itself.
Dependency graph: INV-STORE-004 → {INV-STORE-001, INV-STORE-003}.
---
↳ Self-bootstrap active (C7). Dependency graph queryable:
  `braid query '[:find ?from ?to :where [?e :spec/id ?from] [?e :spec/depends-on ?d] [?d :spec/id ?to]]'`
```

### Step 4: Query Bootstrapped Spec

```
$ braid query '[:find ?id :where [?e :spec/type "invariant"] [?e :spec/id ?id]]' --format agent

[QUERY] 1 result (Stratum 0, monotonic).
  INV-STORE-001
---
↳ Self-bootstrap verified: spec element queryable from the store it governs (C7, INV-SCHEMA-008).
  Continue: transact remaining STORE invariants (INV-STORE-002 through INV-STORE-014).
```

---

## §11.2 Harvest/Seed Session Transcript

Condensed session demonstrating the full harvest/seed lifecycle. Shows 10 representative
turns (not the full 25-turn success criterion from SEED.md §10 — a 25-turn transcript
would be too verbose for a worked example). The key validation is that the NEW session
(§11.2 "New Session: Seed Picks Up") recovers full context without manual re-explanation.

### Turn 1: Seed at Session Start

```
$ braid seed --task "Implement Store::transact per INV-STORE-001" --format agent

[SEED] Session context assembled from 147 store datoms.

## Orientation
You are working on Braid. Phase: Stage 0 implementation. Namespace: STORE.

## Constraints
- ADR-STORE-002: BLAKE3 for content hashing (w=12, do not relitigate)
- ADR-STORE-004: HLC for transaction ordering (w=8)
- INV-STORE-001: Append-only immutability
- NEG-001: No aspirational stubs

## State
Last tx: spec-bootstrap (tx_2). Frontier: {agent1: tx_2}. Drift: 0.0.
Store: 147 datoms. 13 spec elements transacted.

## Warnings
UNC-SCHEMA-001: 17 axiomatic attributes — verify sufficiency during implementation.

## Directive
Implement Store::transact per INV-STORE-001 (append-only) and INV-STORE-002 (strict growth).
Traces to: SEED §4 Axiom 2. First action: write typestate Transaction<Building>.
---
↳ What algebraic law must Store::transact preserve? (L4: monotonicity, L5: growth-only)
```

### Turns 2–5: Implementation Work (agent writes code)

The agent implements `Transaction<Building>`, `commit()`, and `Store::transact()`.
After each code change, it transacts observations:

```
$ braid transact --inline '{:spec/id "OBS-001" :spec/type "observation" \
  :spec/statement "BTreeSet handles dedup automatically via content identity"}'

[STORE] Transacted 4 datoms in tx hlc:1709000010000-0-agent1.
---
↳ Observation recorded. What invariant does this satisfy? (INV-STORE-003)
```

> **Note**: Shorthand map form (`'{:spec/id "OBS-001" ...}'`) is a planned CLI convenience.
> The canonical datom format is the five-tuple `[e a v tx op]` per spec/01-store.md.

### Turn 6: Guidance Check

```
$ braid guidance --format agent

[GUIDANCE] Drift: Basin A (spec-driven), 0 drift signals.
Progress: STORE 3/13 Stage 0 INVs implemented (001, 002, 003). Next: INV-STORE-004 (commutativity).
Recommendation: Write proptest for merge commutativity before implementing merge.
---
↳ INV-STORE-004 is algebraic (Level 0). What property does `S₁ ∪ S₂ = S₂ ∪ S₁` require?
```

### Turns 7–9: Continue Implementation

Agent implements merge, proptest properties, Kani harnesses.

### Turn 10: Harvest at Session End

```
$ braid harvest --format agent

[HARVEST] 5 candidates detected (3 high, 1 medium, 1 low confidence). Drift: 1.4.

  1. [0.95] DECISION: Transaction typestate uses PhantomData<S> (Epistemic)
     Accept? [Y/n] → Y

  2. [0.90] OBSERVATION: BTreeSet::insert returns false for duplicates (Epistemic)
     Accept? [Y/n] → Y

  3. [0.85] DECISION: Frontier stored as HashMap<AgentId, TxId> (Epistemic)
     Accept? [Y/n] → Y

  4. [0.60] DEPENDENCY: INV-STORE-009 depends on fsync semantics for txn files (Structural)
     Accept? [Y/n] → Y

  5. [0.35] UNCERTAINTY: Proptest shrinking slow for large stores (Consequential)
     Accept? [Y/n] → n (too vague)

[HARVEST] Committed 4/5 candidates. Session entity recorded.
Drift: 1.4 → 0.2. Quality: excellent (≤2 uncommitted at harvest).
---
↳ Harvest complete. 4 decisions preserved. Next session can pick up from frontier {agent1: tx_15}.
```

### New Session: Seed Picks Up

```
$ braid seed --task "Continue STORE implementation: INV-STORE-004 through 008" --format agent

[SEED] Session context assembled from 163 store datoms.

## Orientation
Braid. Stage 0. STORE namespace. 3/13 Stage 0 INVs complete.

## Constraints
- ADR-STORE-002: BLAKE3 (w=12, settled)
- OBS-001: BTreeSet handles dedup (harvested)
- IMPL-001: Transaction typestate uses PhantomData (harvested)
- IMPL-002: Frontier as HashMap<AgentId, TxId> (harvested)
- DEP-001: INV-STORE-009 depends on layout fsync (harvested)

## State
Last harvest: 4 candidates committed. Frontier: {agent1: tx_15}. Drift: 0.2.
Store: 163 datoms. STORE progress: INV-001–003 done, INV-004–014 remaining.

## Warnings
UNC-SCHEMA-001: Monitor attribute sufficiency.

## Directive
Implement INV-STORE-004 (commutativity), 005 (associativity), 006 (idempotency),
007 (monotonicity), 008 (genesis determinism). These are the CRDT algebra laws.
First action: proptest for commutativity (test before implement).
---
↳ These are Level 0 algebraic laws. Proof by construction: set union is commutative.
  What does the proptest verify that the type system cannot?
```

The new session has full context without any manual re-explanation. Success criterion met.

---

## §11.2b Merge Worked Example

Two agents working on separate stores, then merging.

### Agent A: Working on STORE

```
$ braid transact --inline '{:spec/id "INV-STORE-001" :spec/type "invariant" \
  :spec/statement "Append-only immutability"}' --format agent

[STORE] Transacted 3 datoms in tx hlc:1709000001-0-agentA. Store: 150 datoms.
```

### Agent B: Working on SCHEMA (separate store)

```
$ braid transact --inline '{:spec/id "INV-SCHEMA-001" :spec/type "invariant" \
  :spec/statement "Schema as data"}' --store .braid-agent-b/ --format agent

[STORE] Transacted 3 datoms in tx hlc:1709000002-0-agentB. Store: 150 datoms.
```

### Agent A merges Agent B's store

```
$ braid merge .braid-agent-b/ --format agent

[MERGE] Merged 3 new datoms (147 duplicates deduplicated via content-addressed dedup).
Store: 153 datoms. Frontier updated for {agentB}.
---
↳ Merge is pure set union (INV-MERGE-001). No datoms were lost.
  Check LIVE view for resolution changes: `braid entity INV-SCHEMA-001`
```

### Verify: datoms from both agents present

```
$ braid query '[:find ?id :where [?e :spec/type "invariant"] [?e :spec/id ?id]]' --format agent

[QUERY] 2 results (Stratum 0, monotonic).
  INV-STORE-001
  INV-SCHEMA-001
```

### Idempotent re-merge (INV-MERGE-008)

```
$ braid merge .braid-agent-b/ --format agent

[MERGE] Merged 0 new datoms (150 duplicates deduplicated via content-addressed dedup).
Store: 153 datoms. No frontier changes.
---
↳ Re-merge is a no-op (INV-MERGE-008). Store unchanged.
```

---

## §11.3 Datalog Query Examples

Five queries against self-bootstrapped spec datoms. Each shows agent-mode output.

### Query 1: All Invariants in STORE Namespace

```
$ braid query '[:find ?id ?stmt
  :where [?e :spec/type "invariant"]
         [?e :spec/namespace "STORE"]
         [?e :spec/id ?id]
         [?e :spec/statement ?stmt]]' --format agent

[QUERY] 13 results (Stratum 0, monotonic).
  INV-STORE-001  "The datom store never deletes or mutates..."
  INV-STORE-002  "Every transaction adds at least one datom..."
  INV-STORE-003  "Two datoms with identical five-tuples..."
  ...
  INV-STORE-014  "Every DDIS command is a store transaction"
---
↳ 13/13 Stage 0 STORE INVs in store (INV-STORE-013 is Stage 2, not yet transacted).
```

### Query 2: Dependency Graph Traversal

```
$ braid query '[:find ?from ?to
  :where [?e :spec/id ?from]
         [?e :spec/depends-on ?d]
         [?d :spec/id ?to]]' --format agent

[QUERY] 8 results (Stratum 1, monotonic).
  INV-MERGE-001   → INV-STORE-001
  INV-HARVEST-001 → INV-STORE-001
  INV-SCHEMA-004  → INV-STORE-001
  INV-STORE-006   → INV-STORE-003
  ...
---
↳ Dependency graph matches spec/17-crossref.md §17.2. Implementation order validated.
```

### Query 3: Cross-Namespace References

```
$ braid query '[:find ?ns (count ?e)
  :where [?e :spec/namespace ?ns]
         [?e :spec/type "invariant"]]' --mode stratified --format agent

[QUERY] 11 results (Stratum 2, stratified — count aggregation requires stratification).
  STORE        13
  LAYOUT       11
  SCHEMA        7
  QUERY        10
  RESOLUTION    8
  HARVEST       5
  SEED          6
  MERGE         5
  GUIDANCE      6
  INTERFACE     6
  TRILATERAL    6
---
↳ 83 Stage 0 INVs transacted across 11 namespaces (of 145 total across 16 namespaces).
  5 namespaces (SYNC, SIGNAL, BILATERAL, DELIBERATION, BUDGET) have no Stage 0 INVs.
```

### Query 4: Frontier-Relative Query

```
$ braid query '[:find ?id
  :where [?e :spec/id ?id]
         [?e :spec/stage 0]]'
  --mode monotonic --format agent

[QUERY] 83 results (Stratum 0, monotonic).
  INV-STORE-001, INV-STORE-002, ..., INV-TRILATERAL-006
---
↳ All 83 Stage 0 elements queryable. Full self-bootstrap complete.
```

### Query 5: Resolution Mode Lookup

```
$ braid query '[:find ?attr ?mode
  :where [?a :db/ident ?attr]
         [?a :db/resolutionMode ?mode]]' --format agent

[QUERY] 26 results (Stratum 0, monotonic).
  :db/ident         lww
  :db/valueType     lww
  :spec/id          lww
  :spec/traces-to   multi
  :spec/verification multi
  ...
---
↳ Schema completeness: all 26 attributes have explicit resolution modes (INV-RESOLUTION-001).
```

---

## §11.4 Error Recovery Demonstrations

### Error 1: Schema Validation Failure

```
$ braid transact --inline '{:spec/bogus "test"}'

Tx error: attribute `:spec/bogus` not in schema
— Unknown attribute (not in genesis or any schema transaction)
— Add attribute first: define `:spec/bogus` with type/cardinality, then retry
— Check available: `braid query '[:find ?a :where [_ :db/ident ?a]]'`
— See: INV-SCHEMA-005 (attribute existence), INV-SCHEMA-003 (schema-as-data)
```

### Error 2: Transaction Conflict (Duplicate Datom)

```
$ braid transact --file inv-store-001.ednl  # transacted previously

[STORE] Transacted 0 new datoms (10 duplicates — content-identity dedup).
All datoms already present in store. Transaction recorded for provenance.
---
↳ Content-identity dedup (INV-STORE-003). The same fact asserted twice = one datom.
  This is correct CRDT behavior (INV-STORE-006: idempotency).
```

### Error 3: Query Stratum Violation

```
$ braid query '[:find ?ns (count ?e)
  :where [?e :spec/namespace ?ns]
         [?e :spec/type "invariant"]
  :with ?e]' --mode monotonic

Query error: aggregation (count) in monotonic mode
— Aggregation requires stratified evaluation (Stratum 2+)
— Use --mode stratified: `braid query '...' --mode stratified`
— See: INV-QUERY-005 (mode-stratum compatibility), ADR-QUERY-003 (CALM compliance)
```

---

## §11.5 MCP Tool Interaction Demonstration

The MCP server is a persistent process launched via `braid serve`. At startup, it
completes the MCP 3-phase initialization handshake (handled by the rmcp crate),
loads the store once from the layout directory, and holds it via `ArcSwap<Store>` for the session
lifetime (Datomic connection model — immutable Store values, atomic pointer swap
on writes). All tool calls below operate against this session-scoped store. Reads
load the current snapshot (lock-free); write operations (transact, harvest) swap
in a new Store atomically.

### Transact via MCP

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "braid_transact",
  "params": {
    "datoms": [
      {"entity_content": "INV-STORE-014", "attribute": ":spec/id", "value": {"String": "INV-STORE-014"}, "op": "assert"},
      {"entity_content": "INV-STORE-014", "attribute": ":spec/type", "value": {"String": "invariant"}, "op": "assert"},
      {"entity_content": "INV-STORE-014", "attribute": ":spec/statement", "value": {"String": "Every DDIS command is a store transaction"}, "op": "assert"}
    ],
    "provenance": "observed",
    "rationale": "Bootstrapping spec element INV-STORE-014"
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "tx_id": "hlc:1709000003000-0-agent1",
    "datom_count": 3,
    "new_datoms": 3,
    "store_size": 134,
    "agent_summary": "[STORE] Transacted 3 datoms (INV-STORE-014) in tx hlc:1709000003000-0-agent1.\n---\n↳ Every command = transaction (INV-STORE-014). This transact is itself a transaction."
  }
}
```

### Query via MCP

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "braid_query",
  "params": {
    "query": "[:find ?id :where [?e :spec/type \"invariant\"] [?e :spec/namespace \"STORE\"] [?e :spec/id ?id]]",
    "mode": "monotonic"
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "bindings": [
      {"?id": "INV-STORE-001"},
      {"?id": "INV-STORE-014"}
    ],
    "stratum": "S0_Ground",
    "count": 2,
    "agent_summary": "[QUERY] 2 results (Stratum 0, monotonic).\n  INV-STORE-001\n  INV-STORE-014\n---\n↳ Self-bootstrap: store manages its own invariants (C7)."
  }
}
```

---

## §11.6 Self-Bootstrap: Spec-to-Datom Migration

This section demonstrates the complete pipeline for migrating spec elements into the
datom store. The pipeline is the system's first act of self-reference (C7): the
specification that defines the store becomes data managed by the store.

### §11.6.1 Parsing a Spec Element

Source: `spec/01-store.md`, element INV-STORE-001.

The parser extracts structured fields from the spec markdown:

```
Input (spec/01-store.md):
─────────────────────────
### INV-STORE-001: Append-Only Immutability

**Traces to**: SEED §4 Axiom 2, C1, ADRS FD-001
**Verification**: `V:TYPE`, `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
∀ S ∈ Store, S' = TRANSACT(S, T) for any T:
  S ⊆ S'
  (monotonicity: once asserted, never lost)

#### Level 1 (State Invariant)
For all reachable states (S, S') where S →[op] S':
  S.datoms ⊆ S'.datoms

#### Level 2 (Implementation Contract)
#[kani::ensures(|result| old(store.datoms.len()) <= store.datoms.len())]
fn transact(store: &mut Store, tx: Transaction<Committed>) -> Result<TxReceipt, TxApplyError>;

**Falsification**: Any operation that reduces store.datoms.len() or removes a
previously-observed datom from the set.

**proptest strategy**: Generate random sequences of TRANSACT/RETRACT operations.
After each operation, verify all previously-observed datoms remain present.
```

```
Parsed output (structured fields):
───────────────────────────────────
  id:            "INV-STORE-001"
  type:          invariant
  namespace:     STORE
  statement:     "Append-Only Immutability"
  traces_to:     ["SEED §4 Axiom 2", "C1", "ADRS FD-001"]
  verification:  [V:TYPE, V:PROP, V:KANI]
  stage:         0
  level_0:       "∀ S ∈ Store, S' = TRANSACT(S, T) for any T:\n  S ⊆ S'\n  (monotonicity: once asserted, never lost)"
  level_1:       "For all reachable states (S, S') where S →[op] S':\n  S.datoms ⊆ S'.datoms"
  level_2:       "#[kani::ensures(|result| old(store.datoms.len()) <= store.datoms.len())]\nfn transact(store: &mut Store, tx: Transaction<Committed>) -> Result<TxReceipt, TxApplyError>;"
  falsification: "Any operation that reduces store.datoms.len() or removes a previously-observed datom from the set."
  proptest:      "Generate random sequences of TRANSACT/RETRACT operations. After each operation, verify all previously-observed datoms remain present."
```

### §11.6.2 Generating Transaction Datoms

Each parsed field maps to a datom. The entity ID is content-addressed (BLAKE3 hash of the
spec element ID — the canonical identifier for this entity across all stores).

Entity identity: `blake3("spec-element:INV-STORE-001")` = `blake3:a7c9e2...`

Transaction: `hlc:1709100000000-0-bootstrap`

```clojure
{:e #blake3 "a7c9e2..." :a :spec/id :v "INV-STORE-001" :tx #hlc "1709100000000-0-bootstrap" :op :assert}
{:e #blake3 "a7c9e2..." :a :spec/type :v "invariant" :tx #hlc "1709100000000-0-bootstrap" :op :assert}
{:e #blake3 "a7c9e2..." :a :spec/namespace :v :STORE :tx #hlc "1709100000000-0-bootstrap" :op :assert}
{:e #blake3 "a7c9e2..." :a :spec/statement :v "Append-Only Immutability" :tx #hlc "1709100000000-0-bootstrap" :op :assert}
{:e #blake3 "a7c9e2..." :a :spec/traces-to :v "SEED §4 Axiom 2" :tx #hlc "1709100000000-0-bootstrap" :op :assert}
{:e #blake3 "a7c9e2..." :a :spec/traces-to :v "C1" :tx #hlc "1709100000000-0-bootstrap" :op :assert}
{:e #blake3 "a7c9e2..." :a :spec/traces-to :v "ADRS FD-001" :tx #hlc "1709100000000-0-bootstrap" :op :assert}
{:e #blake3 "a7c9e2..." :a :spec/verification :v :V:TYPE :tx #hlc "1709100000000-0-bootstrap" :op :assert}
{:e #blake3 "a7c9e2..." :a :spec/verification :v :V:PROP :tx #hlc "1709100000000-0-bootstrap" :op :assert}
{:e #blake3 "a7c9e2..." :a :spec/verification :v :V:KANI :tx #hlc "1709100000000-0-bootstrap" :op :assert}
{:e #blake3 "a7c9e2..." :a :spec/stage :v 0 :tx #hlc "1709100000000-0-bootstrap" :op :assert}
{:e #blake3 "a7c9e2..." :a :spec/falsification :v "Any operation that reduces store.datoms.len() or removes a previously-observed datom from the set." :tx #hlc "1709100000000-0-bootstrap" :op :assert}
{:e #blake3 "a7c9e2..." :a :spec/proptest :v "Generate random sequences of TRANSACT/RETRACT operations. After each operation, verify all previously-observed datoms remain present." :tx #hlc "1709100000000-0-bootstrap" :op :assert}
{:e #blake3 "a7c9e2..." :a :spec/level-0 :v "∀ S ∈ Store, S' = TRANSACT(S, T) for any T:\n  S ⊆ S'\n  (monotonicity: once asserted, never lost)" :tx #hlc "1709100000000-0-bootstrap" :op :assert}
{:e #blake3 "a7c9e2..." :a :spec/level-1 :v "For all reachable states (S, S') where S →[op] S':\n  S.datoms ⊆ S'.datoms" :tx #hlc "1709100000000-0-bootstrap" :op :assert}
{:e #blake3 "a7c9e2..." :a :spec/level-2 :v "#[kani::ensures(|result| old(store.datoms.len()) <= store.datoms.len())]\nfn transact(store: &mut Store, tx: Transaction<Committed>) -> Result<TxReceipt, TxApplyError>;" :tx #hlc "1709100000000-0-bootstrap" :op :assert}
```

**Datom count**: 16 datoms for one invariant. Multi-valued attributes (`:spec/traces-to`,
`:spec/verification`) produce one datom per value. The three-level refinement texts
(`:spec/level-0`, `:spec/level-1`, `:spec/level-2`) preserve the full fidelity of the
spec's cleanroom refinement chain.

### §11.6.3 Resulting Datom Set (After INV-STORE-001 Bootstrap)

Assuming genesis (85 datoms) + spec-element schema (21 attrs x ~4 datoms each = ~84 datoms)
have already been transacted:

```
$ braid status --format agent

[STATUS] Store: 185 datoms. 17 axiomatic + 21 spec-element attributes defined.
Entities: 39 (17 meta-schema attrs + 21 spec attrs + 1 INV-STORE-001).
Frontier: {bootstrap: tx_2}.
---
↳ Self-bootstrap in progress: 1/83 Stage 0 elements transacted.
  Continue: `braid bootstrap --spec-dir spec/`
```

The datom set for INV-STORE-001 viewed as a table (EAVT index order):

```
Entity            Attribute             Value                                          Tx                    Op
────────────────  ────────────────────  ─────────────────────────────────────────────  ────────────────────  ──────
blake3:a7c9e2..   :spec/id              "INV-STORE-001"                                hlc:..000-bootstrap   assert
blake3:a7c9e2..   :spec/type            "invariant"                                    hlc:..000-bootstrap   assert
blake3:a7c9e2..   :spec/namespace       :STORE                                         hlc:..000-bootstrap   assert
blake3:a7c9e2..   :spec/statement       "Append-Only Immutability"                     hlc:..000-bootstrap   assert
blake3:a7c9e2..   :spec/traces-to       "SEED §4 Axiom 2"                              hlc:..000-bootstrap   assert
blake3:a7c9e2..   :spec/traces-to       "C1"                                           hlc:..000-bootstrap   assert
blake3:a7c9e2..   :spec/traces-to       "ADRS FD-001"                                  hlc:..000-bootstrap   assert
blake3:a7c9e2..   :spec/verification    :V:TYPE                                        hlc:..000-bootstrap   assert
blake3:a7c9e2..   :spec/verification    :V:PROP                                        hlc:..000-bootstrap   assert
blake3:a7c9e2..   :spec/verification    :V:KANI                                        hlc:..000-bootstrap   assert
blake3:a7c9e2..   :spec/stage           0                                              hlc:..000-bootstrap   assert
blake3:a7c9e2..   :spec/falsification   "Any operation that reduces store.datoms..."   hlc:..000-bootstrap   assert
blake3:a7c9e2..   :spec/proptest        "Generate random sequences of TRANSACT..."     hlc:..000-bootstrap   assert
blake3:a7c9e2..   :spec/level-0         "∀ S ∈ Store, S' = TRANSACT(S, T)..."          hlc:..000-bootstrap   assert
blake3:a7c9e2..   :spec/level-1         "For all reachable states (S, S')..."           hlc:..000-bootstrap   assert
blake3:a7c9e2..   :spec/level-2         "#[kani::ensures(|result| old(store...)...]"   hlc:..000-bootstrap   assert
```

**Key observations**:
- One entity, 16 datoms. Content-addressed identity means the same element bootstrapped
  by two independent agents produces the same datoms (INV-STORE-003, C2).
- Multi-valued attributes (`:spec/traces-to`, `:spec/verification`) use resolution mode
  `:multi` — all values coexist, no conflict.
- Single-valued attributes (`:spec/statement`, `:spec/stage`) use `:lww` — last writer wins
  if two agents disagree on the wording.
- The three-level refinement texts are separate datoms, enabling queries like "find all
  Level 0 algebraic laws" without parsing the full statement.

### §11.6.4 Datalog Queries Over Spec Datoms

After bootstrapping all 83 Stage 0 elements, the store is queryable:

#### Query A: All Invariants (basic enumeration)

```
$ braid query '[:find ?id ?stmt
  :where [?e :spec/type "invariant"]
         [?e :spec/id ?id]
         [?e :spec/statement ?stmt]]' --format agent

[QUERY] 83 results (Stratum 0, monotonic).
  INV-STORE-001   "Append-Only Immutability"
  INV-STORE-002   "Strict Transaction Growth"
  INV-STORE-003   "Content-Addressed Identity"
  ...
  INV-TRILATERAL-007 "Self-Bootstrap Obligation"
---
↳ Full self-bootstrap: all 83 Stage 0 invariants queryable from the store (C7).
```

#### Query B: All Stage 0 Elements by Namespace (aggregation)

```
$ braid query '[:find ?ns (count ?e)
  :where [?e :spec/stage 0]
         [?e :spec/namespace ?ns]]' --mode stratified --format agent

[QUERY] 11 results (Stratum 1, stratified — aggregation requires stratification).
  STORE        13
  LAYOUT       11
  SCHEMA        7
  QUERY        10
  RESOLUTION    8
  HARVEST       5
  SEED          6
  MERGE         5
  GUIDANCE      6
  INTERFACE     6
  TRILATERAL    6
---
↳ 83 elements across 11 namespaces (of 16 total). Matches spec/16-verification.md matrix.
```

#### Query C: Elements with Level 0 Algebraic Laws

```
$ braid query '[:find ?id ?law
  :where [?e :spec/type "invariant"]
         [?e :spec/id ?id]
         [?e :spec/level-0 ?law]]' --format agent

[QUERY] 48 results (Stratum 0, monotonic).
  INV-STORE-001   "∀ S ∈ Store, S' = TRANSACT(S, T)..."
  INV-STORE-003   "∀ d₁, d₂ ∈ D: (d₁.e = d₂.e ∧ ...) ⟹ d₁ = d₂"
  INV-SCHEMA-001  "schema(S) ⊂ S"
  ...
---
↳ 48 of 83 Stage 0 invariants have Level 0 algebraic laws. Remaining 35 are defined
  at Level 1 (state machine invariants without a standalone algebraic formulation).
```

#### Query D: Traceability — Which Invariants Trace to C1 (Append-Only)?

```
$ braid query '[:find ?id
  :where [?e :spec/traces-to "C1"]
         [?e :spec/id ?id]]' --format agent

[QUERY] 6 results (Stratum 0, monotonic).
  INV-STORE-001
  INV-STORE-002
  INV-STORE-005
  INV-SCHEMA-003
  INV-MERGE-001
  NEG-SCHEMA-002
---
↳ 6 elements depend on Hard Constraint C1 (append-only). Modifying C1 would
  require reviewing all 6. This is the traceability lattice in action (C5).
```

#### Query E: Dependency Graph Traversal (Find What INV-MERGE-001 Depends On)

```
$ braid query '[:find ?dep-id
  :where [?e :spec/id "INV-MERGE-001"]
         [?e :spec/depends-on ?d]
         [?d :spec/id ?dep-id]]' --format agent

[QUERY] 2 results (Stratum 0, monotonic).
  INV-STORE-001
  INV-STORE-003
---
↳ INV-MERGE-001 depends on append-only (001) and content identity (003).
  Merge is set union — it requires that datoms are never lost (001)
  and that identity is structural (003). Verified against spec/17-crossref.md.
```

### §11.6.5 ADR as Datoms — Worked Example

ADR-STORE-013 (BLAKE3 for Content Hashing) demonstrates the ADR-specific attributes:

```clojure
{:e #blake3 "f4d1b8..." :a :spec/id :v "ADR-STORE-013" :tx #hlc "1709100001000-0-bootstrap" :op :assert}
{:e #blake3 "f4d1b8..." :a :spec/type :v "adr" :tx #hlc "1709100001000-0-bootstrap" :op :assert}
{:e #blake3 "f4d1b8..." :a :spec/namespace :v :STORE :tx #hlc "1709100001000-0-bootstrap" :op :assert}
{:e #blake3 "f4d1b8..." :a :spec/statement :v "BLAKE3 for Content Hashing" :tx #hlc "1709100001000-0-bootstrap" :op :assert}
{:e #blake3 "f4d1b8..." :a :spec/stage :v 0 :tx #hlc "1709100001000-0-bootstrap" :op :assert}
{:e #blake3 "f4d1b8..." :a :spec/traces-to :v "ADRS FD-007" :tx #hlc "1709100001000-0-bootstrap" :op :assert}
{:e #blake3 "f4d1b8..." :a :spec/adr-problem :v "Which hash function for content-addressed entity IDs?" :tx #hlc "1709100001000-0-bootstrap" :op :assert}
{:e #blake3 "f4d1b8..." :a :spec/adr-options :v "A) BLAKE3 — fast, cryptographic, parallelizable, 256-bit" :tx #hlc "1709100001000-0-bootstrap" :op :assert}
{:e #blake3 "f4d1b8..." :a :spec/adr-options :v "B) SHA-256 — ubiquitous, well-audited, slower" :tx #hlc "1709100001000-0-bootstrap" :op :assert}
{:e #blake3 "f4d1b8..." :a :spec/adr-options :v "C) xxHash — non-cryptographic, fastest, collision risk" :tx #hlc "1709100001000-0-bootstrap" :op :assert}
{:e #blake3 "f4d1b8..." :a :spec/adr-decision :v "Option A. BLAKE3 provides cryptographic collision resistance with performance matching non-cryptographic hashes." :tx #hlc "1709100001000-0-bootstrap" :op :assert}
{:e #blake3 "f4d1b8..." :a :spec/adr-alternatives :v "B rejected: 3-5x slower, no parallelism, same security margin unnecessary for content addressing." :tx #hlc "1709100001000-0-bootstrap" :op :assert}
{:e #blake3 "f4d1b8..." :a :spec/adr-alternatives :v "C rejected: collision risk unacceptable when identity correctness depends on hash uniqueness (INV-STORE-003)." :tx #hlc "1709100001000-0-bootstrap" :op :assert}
```

**Datom count**: 14 datoms. Multi-valued `:spec/adr-options` and `:spec/adr-alternatives`
capture all options and rejection reasons as independent datoms. This structure enables:

```
$ braid query '[:find ?id ?decision
  :where [?e :spec/type "adr"]
         [?e :spec/id ?id]
         [?e :spec/adr-decision ?decision]]' --format agent

[QUERY] 38 results (Stratum 0, monotonic).
  ADR-STORE-001   "Option A. EAV over relational..."
  ADR-STORE-013   "Option A. BLAKE3 provides cryptographic..."
  ...
```

---

## §11.7 Stage 0 Contradiction Detection

After self-bootstrap, the store contains its own specification. The simplest form of
coherence verification (C7) is detecting contradictions within that specification.
At Stage 0, contradiction detection is constrained to what the Datalog query engine
can compute without the full five-tier contradiction engine (which arrives at Stage 2+).

### §11.7.1 Minimal Viable Contradiction Detection

Three classes of contradiction are detectable at Stage 0 using only monotonic Datalog
queries (Stratum 0-1) and simple aggregation:

**Class 1: Duplicate Spec IDs**

Two entities asserting the same `:spec/id` value. Since spec element IDs are unique
identifiers by convention (one invariant = one entity), this indicates either a parsing
error (duplicate extraction) or a genuine contradiction (two conflicting definitions
under the same ID).

```
$ braid query '[:find ?id (count ?e)
  :where [?e :spec/id ?id]
  :having (> (count ?e) 1)]' --mode stratified --format agent

[QUERY] 0 results (Stratum 1, stratified).
---
↳ No duplicate spec IDs. Each element has a unique entity. Self-consistency check passed.
```

If a duplicate were found:

```
[QUERY] 1 result (Stratum 1, stratified).
  INV-STORE-001  2
---
↳ CONTRADICTION: INV-STORE-001 has 2 distinct entities. This means two different
  content hashes claim to define the same invariant. Inspect both:
  `braid query '[:find ?e ?stmt :where [?e :spec/id "INV-STORE-001"] [?e :spec/statement ?stmt]]'`
```

**Class 2: Conflicting Resolution Modes on Same Attribute**

If two agents independently define `:spec/stage` with different resolution modes, the
schema is inconsistent. This query detects attributes with more than one resolution mode
asserted (which can happen after a merge of two independently-evolved stores).

```
$ braid query '[:find ?attr (count-distinct ?mode)
  :where [?a :db/ident ?attr]
         [?a :db/resolutionMode ?mode]
  :having (> (count-distinct ?mode) 1)]' --mode stratified --format agent

[QUERY] 0 results (Stratum 1, stratified).
---
↳ All attributes have a single resolution mode. Schema consistency verified.
```

If a conflict were found:

```
[QUERY] 1 result (Stratum 1, stratified).
  :spec/stage  2
---
↳ CONTRADICTION: :spec/stage has 2 different resolution modes asserted.
  This can happen after merging a store where :spec/stage was defined as LWW
  and another where it was defined as MULTI. Resolve via ADR:
  `braid query '[:find ?mode ?tx :where [?a :db/ident ":spec/stage"] [?a :db/resolutionMode ?mode ?tx _]]'`
```

**Class 3: Attribute Uniqueness Violations**

For attributes with `:db/unique :identity` (like `:spec/id`), the LIVE index should
never resolve to two different entities having the same value. This query checks the
post-resolution state:

```
$ braid query '[:find ?v (count-distinct ?e)
  :where [?e :spec/id ?v]
  :having (> (count-distinct ?e) 1)]' --mode stratified --format agent

[QUERY] 0 results (Stratum 1, stratified).
---
↳ No uniqueness violations. Every :spec/id value maps to exactly one entity.
```

### §11.7.2 Contradiction Detection Pipeline (Stage 0)

The three checks above form a minimal pipeline that runs after every bootstrap
transaction and after every merge:

```
$ braid verify --self-consistency --format agent

[VERIFY] Running Stage 0 self-consistency checks (3 classes):

  1. Duplicate spec IDs ................. PASS (0 duplicates in 83 elements)
  2. Resolution mode conflicts .......... PASS (0 conflicts in 38 attributes)
  3. Uniqueness violations .............. PASS (0 violations)

Self-consistency: 3/3 checks passed. Specification is internally coherent.
---
↳ Stage 0 contradiction detection complete. For deeper analysis (semantic
  contradictions, invariant-vs-invariant conflicts), see Stage 2: five-tier
  contradiction engine (INV-DELIBERATION-001 through 006).
```

### §11.7.3 What Stage 0 Cannot Detect

Stage 0 contradiction detection is structural, not semantic. It catches:
- Duplicate identifiers (mechanical parse errors)
- Schema definition conflicts (merge-induced)
- Uniqueness constraint violations (LIVE index integrity)

It does NOT catch:
- **Semantic contradictions** — e.g., INV-STORE-001 (append-only) conflicting with a
  hypothetical INV-STORE-099 (garbage collection). Both can coexist structurally as
  distinct entities with distinct IDs. Detecting that their *meanings* conflict requires
  the five-tier contradiction engine (Stage 2+: SAT/SMT, LLM-as-judge).
- **Completeness gaps** — e.g., a namespace with invariants but no corresponding ADRs.
  This is a coverage check, not a contradiction check. Handled by `braid coverage`.
- **Traceability orphans** — e.g., a spec element with no `:spec/traces-to`. This is
  a traceability check (C5). Detectable at Stage 0 via simple query but is a quality
  metric, not a contradiction:

```
$ braid query '[:find ?id
  :where [?e :spec/id ?id]
         (not [?e :spec/traces-to _])]' --format agent

[QUERY] 0 results (Stratum 1, negation — still monotone-safe via stratification).
---
↳ All 83 Stage 0 elements have at least one traceability reference.
  Traceability obligation (C5) satisfied.
```

### §11.7.4 Contradiction After Merge — Worked Scenario

Two agents independently extend the schema, then merge. The merge (pure set union)
introduces a resolution mode conflict that the contradiction pipeline catches:

```
Agent A defines :task/priority with resolution mode :lattice:
  (blake3:x1..., :db/ident, :task/priority, hlc:A1, assert)
  (blake3:x1..., :db/resolutionMode, :lattice, hlc:A1, assert)

Agent B defines :task/priority with resolution mode :lww:
  (blake3:x2..., :db/ident, :task/priority, hlc:B1, assert)
  (blake3:x2..., :db/resolutionMode, :lww, hlc:B1, assert)
```

Note: `blake3:x1...` and `blake3:x2...` are *different* entity IDs because the agents
used different content to generate them (different transaction context). After merge:

```
$ braid merge .braid-agent-b/ --format agent

[MERGE] Merged 4 new datoms. Store: 197 datoms.
---
↳ Warning: 2 new attribute definitions for :task/priority. Run self-consistency check.
```

```
$ braid verify --self-consistency --format agent

[VERIFY] Running Stage 0 self-consistency checks (3 classes):

  1. Duplicate spec IDs ................. PASS
  2. Resolution mode conflicts .......... FAIL
     :task/priority has 2 resolution modes: :lattice (Agent A) and :lww (Agent B).
     Both are valid datoms — the store faithfully records the disagreement.
     Resolution requires human or deliberation decision.
  3. Uniqueness violations .............. PASS (note: :task/priority has 2 defining
     entities — this is expected when two agents define the same attribute independently
     with different content-addressing context)

Self-consistency: 2/3 checks passed. 1 conflict requires resolution.
---
↳ Action needed: `braid deliberate --attribute :task/priority`
  This will create a deliberation thread to decide the resolution mode.
  Until resolved, :task/priority datoms are accepted but LIVE resolution is undefined.
```

This scenario demonstrates the fundamental property: the store *never loses information*
(C1, INV-STORE-001). Both agents' assertions are preserved. The contradiction is detected
*after* merge by the query layer, and resolution is deferred to the deliberation protocol
(Stage 2+). At Stage 0, the contradiction is flagged and the human decides.

---
