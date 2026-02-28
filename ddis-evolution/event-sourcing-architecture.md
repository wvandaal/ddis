# Event-Sourcing Architecture: JSONL-Canonical Inversion

**Date**: 2026-02-27
**Status**: Architectural Decision — Implementation Plan
**Provenance**: Discovery thread via `ddis discover`, crystallized as APP-INV-071..084, APP-ADR-058..065

---

## 1. Problem Statement

The DDIS project's *intended* architecture is **JSONL -> SQLite -> Markdown Projections**, where the append-only event log is the single source of truth, SQLite is a disposable materialized view, and markdown specs are rendered projections. However, the current implementation has this **exactly backwards**: markdown files are the source of truth, parsed into SQLite, with JSONL as a metadata-only audit trail.

### Current Data Flow (Inverted)

```
Markdown (source of truth) --> ddis parse --> SQLite (derived)
                                              |
                                              v
                                          JSONL (audit trail only)
```

Events in the JSONL streams carry metadata (timestamps, spec hashes, event types) but NOT the actual content that defines the specification. When `ddis crystallize` runs, it:
1. Reads JSON from stdin
2. Formats it as markdown
3. **Writes directly to a .md file** (the "source of truth")
4. Emits a `decision_crystallized` event (metadata only, no content payload)

Similarly, `ddis witness` and `ddis challenge` insert directly into SQLite tables, bypassing the event log for content.

### Intended Data Flow (Event-Sourced)

```
JSONL Event Log (source of truth) --> ddis materialize --> SQLite (materialized view)
                                                            |
                                                            v
                                                       ddis project --> Markdown (projection)
```

Every content mutation flows through the event log. SQLite is deterministically derivable from the log. Markdown is a pure projection of SQL state. Delete the SQLite DB, replay the log, get identical state.

---

## 2. Architectural Assessment

### What Already Exists

The event infrastructure is substantial:
- **Three-stream model** (Stream 1: Discovery, Stream 2: Specification, Stream 3: Implementation)
- **28 event types** with stream-type validation
- **Append-only I/O** (O_APPEND|O_CREATE|O_WRONLY)
- **Cross-stream correlation** by artifact ID
- **Event emission** from most CLI commands (validate, parse, drift, witness, challenge, crystallize)

### What's Missing

1. **Content-bearing events**: Events carry metadata but not structured content payloads
2. **Deterministic fold**: No mechanism to replay events into SQLite state
3. **Projection engine**: No mechanism to render SQLite -> markdown synthetically
4. **Import bridge**: No way to convert existing markdown -> events
5. **Causal ordering**: Events have timestamps but no causal references (`causes` field)
6. **CRDT merge**: No commutativity guarantee for independent events
7. **Temporal queries**: Cannot ask "what was the spec at time t?"
8. **Bisect/blame**: Cannot trace elements to their originating events

### Gap Analysis

| Capability | Status | Gap |
|---|---|---|
| Event types | 28 types | Need ~20 content-bearing types |
| Event struct | ID, Type, Timestamp, SpecHash, Stream, Payload | Need Causes, Version fields |
| Fold/Materialize | None | Core engine needed |
| Projection | None | Synthetic render needed |
| Import | None | markdown -> events bridge needed |
| Causal DAG | None | causes[] + topological sort needed |
| CRDT merge | None | Semilattice merge needed |
| Temporal query | None | fold(log[0:t]) needed |
| Bisect | None | Binary search over event sequence needed |
| Blame | None | Provenance trace needed |
| Snapshots | None | Fold checkpoints needed |
| Stream processors | Partial (validators fire) | Reactive observers needed |

---

## 3. Algebraic Foundation

### Free Monoid over Event Types

The event log is a free monoid (Sigma*, ., epsilon) where:
- Sigma = alphabet of event types (spec_section_defined, invariant_crystallized, etc.)
- . = concatenation (append to log)
- epsilon = empty log

### Fold as Monoid Homomorphism

The fold function f: Sigma* -> S maps event sequences to state:
- S = (SQLite state monoid, compose, empty_db)
- f(epsilon) = empty_db
- f(e1 . e2) = apply(f(e1), e2)
- spec(t) = foldl(empty_db, log[0:t])

### Properties

1. **Determinism**: Same sequence -> identical state (f is a function)
2. **Idempotency**: delete(SQL), replay(log) = original SQL (derivability)
3. **Commutativity for independent events**: If e1, e2 have no causal dependency, apply(apply(s, e1), e2) = apply(apply(s, e2), e1) (semilattice)
4. **Causal ordering**: If e2 `causes` e1, then e1 must precede e2 in any valid linearization

### Adjunction with Parse

The current `parse` function p: Markdown -> SQL has a left adjoint `render`: SQL -> Markdown. The event-sourced architecture adds:
- `import`: Markdown -> Events (synthetic event emission)
- `materialize`: Events -> SQL (fold)
- `project`: SQL -> Markdown (synthetic render)

These form a commutative diagram:

```
Markdown --import--> Events --materialize--> SQL --project--> Markdown
    |                                         ^
    +----------parse (legacy)----------------+
```

With the equivalence: materialize(import(md)) ~ parse(md) (APP-INV-078).

---

## 4. Event Content Schema

### New Content-Bearing Event Types

| Event Type | Stream | Content Fields |
|---|---|---|
| spec_section_defined | 2 | module, path, title, body, level |
| spec_section_updated | 2 | module, path, title, body, changes |
| spec_section_removed | 2 | module, path, reason |
| invariant_crystallized | 2 | id, title, statement, semi_formal, violation, validation, why |
| invariant_updated | 2 | id, fields_changed, new_values |
| invariant_removed | 2 | id, reason, superseded_by |
| adr_crystallized | 2 | id, title, problem, options, decision, consequences, tests |
| adr_updated | 2 | id, fields_changed, new_values |
| adr_superseded | 2 | id, superseded_by, reason |
| negative_spec_added | 2 | module, pattern, rationale |
| quality_gate_defined | 2 | gate_number, title, predicate |
| cross_ref_added | 2 | source, target, context |
| glossary_term_defined | 2 | term, definition, module |
| module_registered | 2 | name, domain, maintains, interfaces, implements |
| manifest_updated | 2 | field, old_value, new_value |
| witness_recorded | 3 | invariant_id, evidence_type, evidence, by, model, code_hash |
| witness_revoked | 3 | invariant_id, reason |
| witness_invalidated | 3 | invariant_id, reason |
| challenge_completed | 3 | invariant_id, verdict, levels, score |
| snapshot_created | 2 | position, state_hash |

### Causal Metadata

Every event gains:
- `causes: []string` — IDs of events that causally precede this one
- `version: int` — schema version for forward compatibility

---

## 5. Core Design Decisions

### 5.1 JSONL as Canonical Representation (APP-ADR-058)

JSONL is chosen over SQL and markdown because:
- Append-only guarantees immutability
- Line-oriented format enables simple tooling (grep, tail, jq)
- Human-readable for debugging
- No schema migration (events are self-describing)
- Natural fit for event sourcing
- Already implemented (three-stream model)

### 5.2 Deterministic Fold (APP-ADR-059)

Full replay (or from snapshot) is chosen over incremental mutation because:
- Eliminates mutable state bugs
- Enables temporal queries trivially
- Makes testing deterministic
- Allows schema evolution without migration scripts
- Snapshot optimization prevents performance cliff

### 5.3 Causal References (APP-ADR-060)

`causes` array of event IDs is chosen over vector clocks or Lamport timestamps because:
- Simpler to implement and debug
- Sufficient for DDIS's concurrency model (single-writer with occasional multi-agent)
- Event IDs already exist and are unique
- Causal DAG is explicit and queryable

### 5.4 Field Synthesis for Projections (APP-ADR-061)

Reconstructing markdown from structured fields is chosen over raw_text replay because:
- Structured fields enable semantic queries
- Format can evolve independently of content
- Enables alternative projections (HTML, JSON schema, etc.)
- Prevents format drift between source and projection

### 5.5 Parse as Import Migration Path (APP-ADR-062)

Making `ddis parse` emit synthetic events bridges the old and new architectures:
- Existing markdown specs become importable
- No big-bang migration required
- APP-INV-078 (Import Equivalence) gates the transition
- parse remains available as convenience alias

### 5.6 Semilattice Merge for CRDT (APP-ADR-063)

Independent events commute (semilattice property) while causally dependent events maintain order:
- Enables concurrent multi-agent spec authoring
- Merge is associative, commutative, idempotent for independent events
- Conflict detection for causally related events
- LWW (last-writer-wins) for concurrent updates to same element

### 5.7 Snapshot as Fold Checkpoint (APP-ADR-064)

Materialized SQL + event position enables fast replay:
- Full replay from genesis is O(n) in event count
- Snapshots reduce to O(k) where k = events since snapshot
- Snapshot validity: fold(snapshot, log[snap_pos:]) = fold(log)
- Snapshots are disposable (can always replay from scratch)

### 5.8 Stream Processors as Fold Observers (APP-ADR-065)

Processors fire after fold steps and append their own events:
- Validation runs reactively after content events
- Consistency checks fire on invariant changes
- Drift measurement triggers on specification updates
- Processors are composable and independently deployable

---

## 6. Migration Strategy

### Phase A: Import Bridge (Non-Breaking)

`ddis import` reads existing markdown specs and emits synthetic events to the JSONL log. Each section, invariant, ADR, etc. becomes a content-bearing event. The existing `ddis parse` pipeline continues unchanged.

### Phase B: Materialize + Project (Parallel Path)

`ddis materialize` replays the event log into a fresh SQLite database. `ddis project` renders that database back to markdown. The gate for Phase C is APP-INV-078: the materialized state must match the parsed state (modulo metadata like timestamps).

### Phase C: Architecture Flip

Once import equivalence is proven:
- `ddis crystallize` rewires to append JSONL events (instead of writing .md files)
- `ddis witness` and `ddis challenge` rewire to append events (instead of direct SQL)
- `ddis parse` becomes an alias for `import` + `materialize`
- `ddis render` becomes an alias for `project`

---

## 7. Self-Bootstrap Verification

This architectural change is itself subject to DDIS methodology:
1. **Discovery**: Opened via `ddis discover`
2. **Crystallization**: 14 invariants + 8 ADRs crystallized into event-sourcing module
3. **Validation**: `ddis validate` must pass with new module
4. **Implementation**: Go code in ddis-cli implementing the fold/materialize/project pipeline
5. **Witness + Challenge**: All 14 new invariants witnessed and challenged
6. **Drift**: Must converge to 0

The specification of the event-sourcing architecture is itself event-sourced once the implementation is complete — the ultimate self-bootstrap test.
