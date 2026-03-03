# §11. Worked Examples

> **Spec references**: Multiple — cross-namespace demonstrations.
> **Purpose**: Demonstrations encode format, style, depth, and domain simultaneously.
> Per prompt-optimization: one example encodes what seven constraints cannot.
> Each example shows both the data AND the LLM-optimized output format.

---

## §11.1 Self-Bootstrap Demonstration

The system's first act: transact its own specification elements as datoms (C7).

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
  for self-bootstrap. `braid transact --file spec-schema.jsonl`
```

### Step 2: Define Spec-Element Attributes

Add attributes for managing specification elements:

```jsonl
{"e":"blake3:a1b2...","a":":db/ident","v":{"Keyword":":spec/id"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:a1b2...","a":":db/valueType","v":{"Keyword":"string"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:a1b2...","a":":db/cardinality","v":{"Keyword":"one"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:a1b2...","a":":db/doc","v":{"String":"Spec element ID (e.g., INV-STORE-001)"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:a1b2...","a":":db/resolutionMode","v":{"Keyword":"lww"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:c3d4...","a":":db/ident","v":{"Keyword":":spec/type"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:c3d4...","a":":db/valueType","v":{"Keyword":"keyword"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:c3d4...","a":":db/cardinality","v":{"Keyword":"one"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:c3d4...","a":":db/doc","v":{"String":"Element type: invariant, adr, negative-case"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:c3d4...","a":":db/resolutionMode","v":{"Keyword":"lww"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:e5f6...","a":":db/ident","v":{"Keyword":":spec/statement"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:e5f6...","a":":db/valueType","v":{"Keyword":"string"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:e5f6...","a":":db/cardinality","v":{"Keyword":"one"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:e5f6...","a":":db/resolutionMode","v":{"Keyword":"lww"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:g7h8...","a":":db/ident","v":{"Keyword":":spec/namespace"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:g7h8...","a":":db/valueType","v":{"Keyword":"keyword"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:g7h8...","a":":db/cardinality","v":{"Keyword":"one"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:g7h8...","a":":db/resolutionMode","v":{"Keyword":"lww"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:i9j0...","a":":db/ident","v":{"Keyword":":spec/traces-to"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:i9j0...","a":":db/valueType","v":{"Keyword":"string"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:i9j0...","a":":db/cardinality","v":{"Keyword":"many"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:i9j0...","a":":db/resolutionMode","v":{"Keyword":"multi"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:k1l2...","a":":db/ident","v":{"Keyword":":spec/falsification"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:k1l2...","a":":db/valueType","v":{"Keyword":"string"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:k1l2...","a":":db/cardinality","v":{"Keyword":"one"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:k1l2...","a":":db/resolutionMode","v":{"Keyword":"lww"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:m3n4...","a":":db/ident","v":{"Keyword":":spec/verification"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:m3n4...","a":":db/valueType","v":{"Keyword":"keyword"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:m3n4...","a":":db/cardinality","v":{"Keyword":"many"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:m3n4...","a":":db/resolutionMode","v":{"Keyword":"multi"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:o5p6...","a":":db/ident","v":{"Keyword":":spec/stage"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:o5p6...","a":":db/valueType","v":{"Keyword":"long"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:o5p6...","a":":db/cardinality","v":{"Keyword":"one"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:o5p6...","a":":db/resolutionMode","v":{"Keyword":"lww"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:q7r8...","a":":db/ident","v":{"Keyword":":spec/depends-on"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:q7r8...","a":":db/valueType","v":{"Keyword":"ref"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:q7r8...","a":":db/cardinality","v":{"Keyword":"many"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
{"e":"blake3:q7r8...","a":":db/resolutionMode","v":{"Keyword":"multi"},"tx":"hlc:1709000001000-0-agent1","op":"assert"}
```

```
$ braid transact --file spec-schema.jsonl --format agent

[STORE] Transacted 36 datoms (9 spec attributes defined) in tx hlc:1709000001000-0-agent1.
Store: 121 datoms. Schema: 26 attributes (17 axiomatic + 9 spec).
---
↳ Schema extended (INV-SCHEMA-004). Next: transact INV-STORE-001 as the first spec datom.
  Self-bootstrap begins (C7).
```

### Step 3: Transact First Invariant

```jsonl
{"e":"blake3:s9t0...","a":":spec/id","v":{"String":"INV-STORE-001"},"tx":"hlc:1709000002000-0-agent1","op":"assert"}
{"e":"blake3:s9t0...","a":":spec/type","v":{"Keyword":"invariant"},"tx":"hlc:1709000002000-0-agent1","op":"assert"}
{"e":"blake3:s9t0...","a":":spec/namespace","v":{"Keyword":"STORE"},"tx":"hlc:1709000002000-0-agent1","op":"assert"}
{"e":"blake3:s9t0...","a":":spec/statement","v":{"String":"The datom store never deletes or mutates an existing datom. All state changes are new assertions."},"tx":"hlc:1709000002000-0-agent1","op":"assert"}
{"e":"blake3:s9t0...","a":":spec/falsification","v":{"String":"Any operation that removes a datom or modifies the five-tuple of an existing datom."},"tx":"hlc:1709000002000-0-agent1","op":"assert"}
{"e":"blake3:s9t0...","a":":spec/traces-to","v":{"String":"SEED §4 Axiom 2"},"tx":"hlc:1709000002000-0-agent1","op":"assert"}
{"e":"blake3:s9t0...","a":":spec/traces-to","v":{"String":"C1"},"tx":"hlc:1709000002000-0-agent1","op":"assert"}
{"e":"blake3:s9t0...","a":":spec/verification","v":{"Keyword":"V:PROP"},"tx":"hlc:1709000002000-0-agent1","op":"assert"}
{"e":"blake3:s9t0...","a":":spec/verification","v":{"Keyword":"V:KANI"},"tx":"hlc:1709000002000-0-agent1","op":"assert"}
{"e":"blake3:s9t0...","a":":spec/stage","v":{"Long":0},"tx":"hlc:1709000002000-0-agent1","op":"assert"}
```

```
$ braid transact --file inv-store-001.jsonl --format agent

[STORE] Transacted 10 datoms (INV-STORE-001) in tx hlc:1709000002000-0-agent1.
Store: 131 datoms. First spec element bootstrapped. Self-reference: the store now
contains the invariant that governs the store itself.
---
↳ Self-bootstrap active (C7). The system manages its own specification.
  Query: `braid query '[:find ?id :where [?e :spec/type "invariant"] [?e :spec/id ?id]]'`
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

### Turn 6: Guidance Check

```
$ braid guidance --format agent

[GUIDANCE] Drift: Basin A (spec-driven), 0 drift signals.
Progress: STORE 3/13 INVs implemented (001, 002, 003). Next: INV-STORE-004 (commutativity).
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

  4. [0.60] DEPENDENCY: INV-STORE-009 depends on redb fsync semantics (Structural)
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
Braid. Stage 0. STORE namespace. 3/13 INVs complete.

## Constraints
- ADR-STORE-002: BLAKE3 (w=12, settled)
- OBS-001: BTreeSet handles dedup (harvested)
- IMPL-001: Transaction typestate uses PhantomData (harvested)
- IMPL-002: Frontier as HashMap<AgentId, TxId> (harvested)
- DEP-001: INV-STORE-009 depends on redb fsync (harvested)

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
  :spec/statement "Schema as data"}' --store .braid/agent-b.redb --format agent

[STORE] Transacted 3 datoms in tx hlc:1709000002-0-agentB. Store: 150 datoms.
```

### Agent A merges Agent B's store

```
$ braid merge .braid/agent-b.redb --format agent

[MERGE] Merged 3 new datoms (147 duplicates deduplicated).
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
$ braid merge .braid/agent-b.redb --format agent

[MERGE] Merged 0 new datoms (150 duplicates deduplicated).
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

[QUERY] 13 results (Stratum 1, monotonic).
  INV-STORE-001  "The datom store never deletes or mutates..."
  INV-STORE-002  "Every transaction adds at least one datom..."
  INV-STORE-003  "Two datoms with identical five-tuples..."
  ...
---
↳ 13/14 STORE INVs in store (INV-STORE-013 is Stage 2, not yet transacted).
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
         [?e :spec/type "invariant"]]' --format agent

[QUERY] 9 results (Stratum 1, monotonic).
  STORE        13
  SCHEMA        7
  QUERY        10
  RESOLUTION    8
  HARVEST       5
  SEED          4
  MERGE         4
  GUIDANCE      6
  INTERFACE     5
---
↳ 62 Stage 0 INVs transacted across 9 namespaces (of 122 total).
```

### Query 4: Frontier-Relative Query

```
$ braid query '[:find ?id
  :where [?e :spec/id ?id]
         [?e :spec/stage 0]]'
  --mode monotonic --format agent

[QUERY] 61 results (Stratum 0, monotonic).
  INV-STORE-001, INV-STORE-002, ..., INV-INTERFACE-009
---
↳ All 61 Stage 0 invariants queryable. Full self-bootstrap complete.
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
$ braid transact --file inv-store-001.jsonl  # transacted previously

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
loads the store once from redb, and holds it via `Arc<Store>` for the session
lifetime. All tool calls below operate against this session-scoped store reference.

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
      {"entity_content": "INV-STORE-014", "attribute": ":spec/type", "value": {"Keyword": "invariant"}, "op": "assert"},
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
