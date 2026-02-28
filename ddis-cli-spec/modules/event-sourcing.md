---
module: event-sourcing
domain: eventsourcing
maintains: [APP-INV-071, APP-INV-072, APP-INV-073, APP-INV-074, APP-INV-075, APP-INV-076, APP-INV-077, APP-INV-078, APP-INV-079, APP-INV-080, APP-INV-081, APP-INV-082, APP-INV-083, APP-INV-084, APP-INV-085, APP-INV-086, APP-INV-087, APP-INV-088, APP-INV-089, APP-INV-090, APP-INV-091, APP-INV-092, APP-INV-093, APP-INV-094, APP-INV-095, APP-INV-096, APP-INV-097, APP-INV-098, APP-INV-100, APP-INV-101, APP-INV-108, APP-INV-110]
interfaces: [APP-INV-001, APP-INV-002, APP-INV-010, APP-INV-015, APP-INV-016, APP-INV-020, APP-INV-025, APP-INV-048, APP-INV-053]
implements: [APP-ADR-058, APP-ADR-059, APP-ADR-060, APP-ADR-061, APP-ADR-062, APP-ADR-063, APP-ADR-064, APP-ADR-065, APP-ADR-066, APP-ADR-067, APP-ADR-068, APP-ADR-069, APP-ADR-070, APP-ADR-071, APP-ADR-072, APP-ADR-073, APP-ADR-074, APP-ADR-076, APP-ADR-078, APP-ADR-079]
adjacent: [parse-pipeline, code-bridge, lifecycle-ops, auto-prompting]
negative_specs:
  - "Must NOT read or write markdown as canonical source of truth"
  - "Must NOT store mutable state not derivable from the event log"
  - "Must NOT use raw_text blobs for rendering projections"
  - "Must NOT break causal ordering during merge operations"
  - "Must NOT allow non-deterministic materialize results"
---

# Event Sourcing Module

This module owns the JSONL-canonical event-sourcing architecture for the DDIS CLI. It defines the formal mechanisms by which append-only JSONL event logs serve as the single source of truth, SQLite databases are deterministic materialized views derived via fold, and markdown specifications are pure projections rendered from structured fields.

The architectural principle: **the event log is the free monoid, fold is a monoid homomorphism, and projections are pure functions.** The event log (Sigma*, ., epsilon) over the event type alphabet Sigma is the canonical representation. The fold function f : Sigma* -> S maps event sequences to state (the SQLite materialized view). The specification at any time t is spec(t) = foldl(empty_db, log[0:t]). Projections (markdown rendering) are pure functions of materialized state, carrying no additional information.

This module inverts the current data flow from markdown-first to JSONL-canonical. The current pipeline (markdown -> parse -> SQLite, with JSONL as audit trail) becomes the intended pipeline (JSONL -> materialize -> SQLite -> project -> markdown). The inversion is non-breaking: `ddis import` bridges existing markdown into events, and APP-INV-078 (Import Equivalence) gates the transition.

**Invariants interfaced from other modules (cross-module reference completeness --- restated at point of use):**

- APP-INV-001: Round-Trip Fidelity --- parse then render produces byte-identical output (maintained by parse-pipeline). *Import equivalence (APP-INV-078) requires that importing then materializing then projecting produces output indistinguishable from the original spec. Round-trip fidelity is the baseline this must match.*
- APP-INV-002: Validation Determinism --- results independent of clock, RNG, execution order (maintained by query-validation). *Fold determinism (APP-INV-073) extends validation determinism to the materialization pipeline: same events must always produce same validation results.*
- APP-INV-010: Oplog Append-Only --- no modification or deletion after write (maintained by lifecycle-ops). *The event log inherits the oplog append-only guarantee. Events in the canonical JSONL are immutable once written.*
- APP-INV-015: Deterministic Hashing --- SHA-256 with no salt (maintained by parse-pipeline). *Content hashes in events use the same deterministic hashing. Snapshot validation depends on reproducible hashes.*
- APP-INV-016: Implementation Traceability --- valid Source/Tests/Validates-via paths (maintained by lifecycle-ops). *Causal provenance (APP-INV-084) extends traceability from implementation paths to event provenance chains.*
- APP-INV-020: Event Stream Append-Only --- JSONL streams are immutable append logs (maintained by code-bridge). *This module depends on the append-only guarantee from code-bridge. All new content-bearing events inherit this property.*
- APP-INV-025: Discovery Provenance --- every artifact traces to source (maintained by auto-prompting). *Causal provenance (APP-INV-084) extends discovery provenance to the event level: every spec element traces to its crystallization event.*
- APP-INV-048: Event Stream VCS --- JSONL files are VCS-tracked primary data (maintained by code-bridge). *The canonical event log must be VCS-tracked since it is the source of truth. SQL databases and markdown projections are derived and may be gitignored.*
- APP-INV-053: Event Stream Completeness --- every state-mutating command emits a typed event (maintained by lifecycle-ops). *In the JSONL-canonical architecture, "emits a typed event" becomes "ALL mutations flow through events." Content-bearing events are the mechanism, not just audit metadata.*

---

## Formal Foundation

### The Free Monoid over Event Types

Let Sigma denote the finite alphabet of content-bearing event types:

```
Sigma = {
  spec_section_defined, spec_section_updated, spec_section_removed,
  invariant_crystallized, invariant_updated, invariant_removed,
  adr_crystallized, adr_updated, adr_superseded,
  negative_spec_added, quality_gate_defined,
  cross_ref_added, glossary_term_defined,
  module_registered, manifest_updated,
  witness_recorded, witness_revoked, witness_invalidated,
  challenge_completed, snapshot_created
}
```

The event log is the free monoid (Sigma*, ., epsilon) where:
- Sigma* = set of all finite sequences of events
- . = concatenation (append to log)
- epsilon = empty log (no events)

### Fold as Monoid Homomorphism

The fold function f : Sigma* -> S is a monoid homomorphism from the free monoid to the state monoid:

```
S = (SQLiteDB, compose, empty_db)

f(epsilon)      = empty_db                      (identity)
f(e1 . e2 ... en) = apply(... apply(apply(empty_db, e1), e2) ..., en)  (left fold)

spec(t) = foldl(empty_db, log[0:t])            (specification at time t)
```

The `apply : S x Sigma -> S` function maps a single event onto the current state. It is a pure function: no side effects, no I/O, no randomness. This guarantees fold determinism (APP-INV-073).

### Adjunction: Import/Materialize/Project

The event-sourced pipeline forms a commutative diagram with the legacy parse pipeline:

```
                import              materialize            project
Markdown ───────────────> Events ───────────────> SQLite ───────────> Markdown
    |                                               ^
    +──────────── parse (legacy) ───────────────────+
```

The key equivalence (APP-INV-078):

```
project(materialize(import(md))) ~ parse_then_render(md)
```

Where `~` denotes structural equivalence (identical content, modulo metadata like timestamps and event IDs). The parse command becomes a convenience alias for import + materialize.

### Causal Partial Order

Events form a partial order via the `causes` relation:

```
e1 < e2  iff  e1.ID in e2.causes
```

The transitive closure of `<` defines the causal DAG. Independent events (no causal path between them) commute under fold:

```
forall e1, e2 in Sigma:
  (NOT e1 < e2) AND (NOT e2 < e1)
  =>  apply(apply(s, e1), e2) = apply(apply(s, e2), e1)
```

This commutativity property (APP-INV-081) enables CRDT-style multi-agent merge: given two independent event subsequences A and B, merge(A, B) = merge(B, A).

### Snapshot Optimization

A snapshot is a pair (state, position) where:
- state = materialized SQLite database at event position
- position = index into the event log

The snapshot invariant (APP-INV-083):

```
forall valid snapshot (snap_state, snap_pos):
  foldl(snap_state, log[snap_pos:]) = foldl(empty_db, log)
```

Snapshots convert O(n) full replay into O(k) incremental replay where k = |log| - snap_pos.

---

## Invariants

**APP-INV-071: Log Canonicality**

*The JSONL event log is the single source of truth. SQLite databases and markdown files are derived artifacts. Deleting the SQLite database and all markdown files, then replaying the event log via materialize + project, must recover the full specification state.*

```
forall spec_state S derived from log L:
  materialize(L) = S
  project(materialize(L)) = render(S)
  delete(S); materialize(L) => S' where S' = S
```

Violation scenario: A command writes directly to SQLite without first appending an event. The SQLite state diverges from the event log. Replaying the log produces a different state than the live database.

Validation: Materialize from empty into a fresh SQLite file, then diff against the live database. Non-empty diff indicates canonicality violation. Implemented via `ddis materialize --verify`.

// WHY THIS MATTERS: Without log canonicality, the event log is merely an audit trail and the system reverts to markdown-as-source-of-truth. The entire architectural inversion depends on this property holding for all state-mutating operations.

---

**APP-INV-072: Event Content Completeness**

*Every content-mutating event carries the full structured payload needed to reconstruct the affected spec element. Events must NOT reference external files or rely on state not present in the event payload for content reconstruction.*

```
forall event e of content-mutating type:
  reconstruct(e.Payload) produces complete spec element
  no external file read required during apply(state, e)
```

Violation scenario: An `invariant_crystallized` event carries only the invariant ID and title, with the statement, semi-formal, and violation scenario stored in a separate markdown file. Replaying the event without the file produces an incomplete invariant.

Validation: For each content-bearing event type, verify that `apply(empty_state, e)` produces a complete element without I/O. Unit tests with mock filesystem that fails on any file access during apply.

// WHY THIS MATTERS: Content completeness is the precondition for fold determinism. If events are pointers to external state rather than self-contained records, the fold becomes non-deterministic (depends on filesystem state at replay time).

---

**APP-INV-073: Fold Determinism**

*Given the same event sequence, the fold function produces byte-identical SQLite state. The fold is a pure function: no system clock reads, no random number generation, no environment variable access, no I/O beyond the event stream input.*

```
forall event sequences L:
  materialize(L) = materialize(L)       (reproducibility)

forall event sequences L, environments E1 != E2:
  materialize_in(E1, L) = materialize_in(E2, L)  (environment independence)
```

Violation scenario: The `apply` function uses `time.Now()` to set a `created_at` field in the SQLite table. Replaying the same events at different times produces different database contents.

Validation: Materialize the same event log twice in succession and compare SQLite files byte-by-byte. Also materialize with modified system clock, locale, and working directory to verify environment independence.

// WHY THIS MATTERS: Fold determinism is the foundation of the entire event-sourcing architecture. Without it, temporal queries are meaningless (different replays produce different specs), snapshots are unreliable, and CRDT merge is undefined.

---

**APP-INV-074: Causal Ordering**

*Events carry a causes field referencing the IDs of events that causally precede them. The fold respects this partial order: if event e2 causes-references event e1, then e1 must be applied before e2. A topological sort of the causal DAG produces a valid linearization for fold.*

```
forall events e1, e2:
  e1.ID in e2.causes => position(e1) < position(e2) in any valid linearization

forall causal DAG G:
  topological_sort(G) is a valid fold order
  fold(topological_sort(G)) = fold(any_valid_linearization(G))
```

Violation scenario: An `invariant_updated` event references an `invariant_crystallized` event via `causes`, but the update event appears before the crystallization event in the log. The fold tries to update a non-existent invariant.

Validation: For each event with non-empty `causes`, verify that all referenced event IDs have lower positions in the log. Report violations as causal ordering errors. Implemented in `ddis materialize --verify`.

// WHY THIS MATTERS: Causal ordering is the bridge between the free monoid (unordered concatenation) and the partial order required for correct fold. Without it, events that depend on prior state may be applied in the wrong order, producing corrupted specifications.

---

**APP-INV-075: Materialization Idempotency**

*Deleting the SQLite database and replaying the full event log produces a state identical to the original database. The SQLite database is a disposable cache that can always be regenerated from the event log.*

```
forall event logs L, materialized state S = materialize(L):
  delete(S)
  S' = materialize(L)
  S' = S  (byte-identical after normalization for SQLite page ordering)
```

Violation scenario: The materialize function uses SQLite auto-increment IDs that depend on insertion order. After deleting and re-materializing, the IDs differ because the original database had interleaved insertions from concurrent commands.

Validation: Materialize, export all tables as sorted CSV, delete DB, re-materialize, export again, diff. Content must be identical. Auto-increment IDs are excluded from comparison (they are opaque identifiers).

// WHY THIS MATTERS: Idempotency is the litmus test for JSONL canonicality. If you cannot delete the database and get it back from the log, the log is not truly canonical — there is hidden state in the database that the log does not capture.

---

**APP-INV-076: Projection Purity**

*Projections (markdown rendering from SQLite state) are pure functions: they read from the materialized state and produce output without side effects. No projection may write to the database, modify the event log, or depend on state outside the SQLite index.*

```
forall materialized state S:
  project(S) is a pure function
  project(S) = project(S)  (deterministic)
  project reads only from S, never writes
  project does not access event log, filesystem, or network
```

Violation scenario: The project function reads `last_modified` timestamps from the filesystem to annotate the rendered markdown with freshness indicators. Projecting on a different machine (or after `touch`) produces different output.

Validation: Run project in a sandboxed environment with no filesystem access beyond the SQLite database. Verify output is identical to normal project. Unit tests with mock filesystem that panics on access.

// WHY THIS MATTERS: Projection purity ensures that markdown output is fully determined by the materialized state. If projections have side channels, the rendered markdown carries information not present in the event log, breaking the chain of derivability.

---

**APP-INV-077: Synthetic Render**

*Markdown projections are synthesized from structured fields stored in the SQLite database (title, statement, semi_formal, etc.), NOT by replaying raw_text blobs from events. The rendering format is defined by the projection engine and can evolve independently of the event schema.*

```
forall spec elements in materialized state S:
  rendered_element = synthesize(element.title, element.fields...)
  rendered_element != element.raw_text  (may differ in formatting)
  parse(rendered_element) ~ element     (content equivalence)
```

Violation scenario: The project function stores and replays a `raw_text` blob from the original crystallization event, bypassing the structured fields. When the rendering format changes (e.g., new heading level convention), the raw_text output is stale.

Validation: Modify the rendering template (e.g., change heading levels), re-project, verify that ALL output reflects the new template. If any element uses stale formatting, it is using raw_text instead of field synthesis.

// WHY THIS MATTERS: Synthetic render decouples content from format. Structured fields enable semantic queries, alternative projections (HTML, JSON schema), and format evolution without re-authoring. Raw_text replay locks content to its original formatting forever.

---

**APP-INV-078: Import Equivalence**

*Importing existing markdown via ddis import followed by ddis materialize produces SQLite state structurally equivalent to ddis parse on the same markdown. This equivalence gates the architectural transition from markdown-first to JSONL-canonical.*

```
forall valid DDIS markdown specs md:
  materialize(import(md)) ~ parse(md)

where ~ denotes structural equivalence:
  same sections (path, title, body)
  same invariants (id, title, statement, semi_formal, violation, validation, why)
  same ADRs (id, title, problem, options, decision, consequences, tests)
  same cross-references (source, target)
  same quality gates, negative specs, glossary entries
  metadata fields (timestamps, event_ids) excluded from comparison
```

Violation scenario: `ddis import` emits section events with the body as a single text blob, while `ddis parse` splits the body into sub-elements (invariants, ADRs, etc.). The materialized state from import has sections without extracted elements; the parsed state has both.

Validation: For each spec in the test corpus, run both pipelines and diff the resulting SQLite databases (excluding metadata tables). Zero structural differences = equivalence proven. Regression test in CI.

// WHY THIS MATTERS: Import equivalence is the migration gate. Without it, switching from parse to import+materialize would change the semantics of existing specs. The equivalence proof is what makes the architectural inversion non-breaking.

---

**APP-INV-079: Temporal Query Soundness**

*Folding the event log up to time t produces a valid specification state at time t. Temporal queries return the complete, consistent state as it existed at the queried moment, not a partial or corrupted view.*

```
forall times t within the log's range:
  S(t) = foldl(empty_db, log[0:t])
  S(t) is a valid DDIS state (passes structural validation)
  S(t).invariants = {inv : crystallize_event(inv).timestamp <= t AND NOT exists remove_event(inv).timestamp <= t}
```

Violation scenario: Folding to time t includes an `invariant_updated` event but not the preceding `invariant_crystallized` event (because the crystallization has a later timestamp due to clock skew). The fold tries to update a non-existent invariant, producing an error or corrupt state.

Validation: For a test corpus with known state at 10 checkpoints, fold to each checkpoint and verify the state matches the expected snapshot. Verify that folding to any arbitrary timestamp between checkpoints produces a valid (though potentially partial) state.

// WHY THIS MATTERS: Temporal queries are the killer feature of event sourcing. "What did the spec look like when we released v2.0?" is answerable only if fold-to-time produces correct results. This requires both causal ordering (APP-INV-074) and fold determinism (APP-INV-073).

---

**APP-INV-080: Stream Processor Reactivity**

*Content-bearing events trigger downstream stream processors (validation, consistency checking, drift measurement) that fire after the fold step and may append their own events to the log. Processors are composable and independently deployable.*

```
forall content events e:
  apply(state, e) triggers registered processors
  each processor p observes the post-apply state
  each processor p may append derived events to the log
  derived events have causes = [e.ID]
  processors are idempotent: processing the same event twice produces the same derived events
```

Violation scenario: A `spec_section_defined` event is applied but no validation processor fires. The materialized state contains the new section but no `validation_run` event is emitted, leaving the section unvalidated until a manual `ddis validate` call.

Validation: Emit a test content event, verify that all registered processors fire within the same materialize call. Verify that processor-emitted events have correct `causes` references. Verify processor idempotency by re-processing.

// WHY THIS MATTERS: Stream processors close the feedback loop between content changes and quality assessment. Without reactivity, the system requires manual validation after every change, defeating the purpose of event-driven architecture.

---

**APP-INV-081: CRDT Convergence**

*For independent events (no causal path between them), merge is commutative, associative, and idempotent. Two agents producing independent event subsequences A and B converge to the same state regardless of merge order.*

```
forall independent event subsequences A, B:
  merge(A, B) = merge(B, A)             (commutativity)
  merge(merge(A, B), C) = merge(A, merge(B, C))  (associativity)
  merge(A, A) = A                        (idempotency)

  materialize(interleave(A, B)) = materialize(interleave(B, A))
```

Violation scenario: Agent 1 crystallizes APP-INV-071 and Agent 2 crystallizes APP-INV-072 concurrently. Merging Agent 1's log then Agent 2's log assigns different SQLite row IDs than the reverse order. Downstream queries by row ID return different results.

Validation: Generate two independent event streams (no shared causal references). Merge in both orders. Materialize both merged logs. Diff the resulting states (excluding opaque IDs). Empty diff = commutativity proven. Repeat with 3+ streams for associativity.

// WHY THIS MATTERS: CRDT convergence enables concurrent multi-agent spec authoring. Without it, agents must serialize their work (one at a time) or risk divergent specification states that require manual reconciliation.

---

**APP-INV-082: Bisect Correctness**

*The bisect operation finds the earliest event in the log that introduces a specified defect (validation failure, contradiction, missing element). Binary search over event positions with fold-to-position produces the correct result in O(log n) materialize operations.*

```
forall defect predicates P, event logs L where P fails at some position:
  bisect(L, P) = e_k where:
    P(materialize(L[0:k-1])) = true   (defect absent before e_k)
    P(materialize(L[0:k]))   = false  (defect present after e_k)
    no e_j with j < k satisfies the same
```

Violation scenario: The bisect binary search checks the midpoint but materializes from scratch each time (O(n) per check). With a 10,000-event log, bisect takes 13 * 10,000 = 130,000 apply operations instead of using snapshots for O(k) per check.

Validation: Create a test log with a known defect-introducing event at position p. Run bisect, verify it identifies position p. Verify the total number of materialize operations is O(log n). Verify correctness for edge cases: defect at position 0, defect at last position, no defect.

// WHY THIS MATTERS: Bisect is the event-sourced analog of git bisect. When a specification becomes invalid, bisect identifies the exact event (and therefore the exact change and its author) that broke it. This is impossible in the markdown-first architecture where changes are opaque file edits.

---

**APP-INV-083: Snapshot Consistency**

*A snapshot is a materialized SQLite state plus the event log position at which it was taken. Folding from the snapshot state with the remaining events produces the same result as folding the complete log from scratch.*

```
forall event logs L, snapshot positions p:
  snap_state = materialize(L[0:p])
  incremental = foldl(snap_state, L[p:])
  full_replay  = materialize(L)
  incremental = full_replay
```

Violation scenario: A snapshot is taken at position p, but a later event at position p+5 modifies a table that the snapshot's state has foreign key constraints on. The incremental fold fails with a constraint violation because the snapshot state has stale foreign key relationships that the full replay would have updated at position p+3.

Validation: Take snapshots at positions 0, n/4, n/2, 3n/4, n-1. For each, fold incrementally from the snapshot and compare against full replay. All five must produce identical states.

// WHY THIS MATTERS: Snapshots are the performance optimization that makes event sourcing practical at scale. A 100,000-event log takes minutes to replay from scratch but seconds from a recent snapshot. Without snapshot consistency, this optimization is unsound.

---

**APP-INV-084: Causal Provenance**

*Every spec element in the materialized state traces back to its originating crystallization event via the causal DAG. Given any invariant, ADR, section, or other element, the provenance chain reconstructs the complete history of how that element came to exist and every modification it has undergone.*

```
forall spec elements e in materialized state S:
  provenance(e) = sequence of events [e_create, e_modify1, ..., e_modifyN]
  e_create is of type *_crystallized or *_defined
  each e_modifyI.causes contains e_create.ID or a prior e_modifyJ.ID
  apply_sequence(empty, provenance(e)) produces exactly e
```

Violation scenario: An invariant is crystallized via event e1, then updated via event e2, but e2.causes is empty (no causal reference to e1). The provenance chain for the invariant shows only e2, losing the history of e1's original content.

Validation: For each element in the materialized state, compute provenance(e) by following causes chains. Verify: (1) provenance is non-empty, (2) first event is a creation type, (3) each subsequent event has causes linking to prior events, (4) replaying provenance in isolation produces the element.

// WHY THIS MATTERS: Causal provenance is the "git blame" of event sourcing. It answers "who created this invariant, when, and why?" and "what was the chain of modifications that led to its current form?" Without provenance, the event log is an opaque sequence with no navigable structure.

---

**APP-INV-085: Import Content Completeness**

*The ddis import command must emit synthetic events for ALL content types: modules, invariants, ADRs, sections, cross-references, negative specs, quality gates, glossary entries.*

```
forall content_type T in {modules, invariants, adrs, sections, xrefs, negspecs, gates, glossary}:
  count(import(db).events_of_type(T)) = count(db.table(T).rows)
```

Violation scenario: Import emits events for invariants and ADRs but skips sections and cross-references. Materialize produces database missing sections.

Validation: Parse CLI spec, import, materialize, compare row counts per table.

// WHY THIS MATTERS: Partial import silently loses spec content. The migration from parse to import+materialize requires content completeness as a precondition for equivalence (APP-INV-078).

---

**APP-INV-086: Applier Spec-ID Parameterization**

*The sqlApplier derives spec_id from event payloads or configuration, never hardcoding values. Every inserted row references the correct spec_index and source_files entries.*

```
forall event E with payload.spec_id = N:
  apply(E).inserted_rows.spec_id = N
  foreign_key_check(apply(E)) passes
```

Violation scenario: sqlApplier hardcodes spec_id=1; when materializing events from spec ID 3, all rows reference non-existent spec_id=1.

Validation: Materialize events into fresh database, verify spec_id references resolve via foreign key check.

// WHY THIS MATTERS: Hardcoded spec_id breaks multi-spec materialization and foreign key integrity. Every row must reference the correct spec_index entry.

---

**APP-INV-087: Projector Section Rendering**

*The project command filters invariants, ADRs, sections, and other content by module ownership. Each projected module contains only the elements maintained by that module.*

```
forall module M with maintains = {E1, E2, ...}:
  project(db, M).elements = {E | E.id in M.maintains}
```

Violation scenario: project queries all invariants for every module, producing incorrect documents.

Validation: Project multi-module spec, verify each output file contains only elements from its maintains list.

// WHY THIS MATTERS: Module boundaries are the organizational unit of the spec. Cross-module element leakage produces incorrect documents.

---

**APP-INV-088: Single Write Path**

*Content-mutating commands write ONLY to the event log. SQLite databases are produced exclusively by materialize; markdown files exclusively by project.*

```
forall content-mutating command C:
  C.writes = {event_log}
  C.writes ∩ {sqlite, markdown} = empty
```

Violation scenario: Crystallize dual-writes to event log AND markdown; crash between writes produces inconsistent state.

Validation: Instrument crystallize write calls, verify only event stream writes occur.

// WHY THIS MATTERS: Dual writes create split-brain between event log and derived artifacts. Single write path is the architectural guarantee that the event log is the sole source of truth.

---

**APP-INV-089: Deprecation Compatibility Bridge**

*During migration Phase B, deprecated commands (parse, render) produce structurally equivalent output to their event-sourcing replacements (import+materialize, project).*

```
forall deprecated command D with replacement R:
  StructuralDiff(D(input), R(input)) = empty
```

Violation scenario: Replaced parse produces different section ordering than import+materialize.

Validation: Run both paths on same input, compare via StructuralDiff.

// WHY THIS MATTERS: The deprecation bridge enables incremental migration. Without equivalence verification, replacing commands silently changes behavior.

---

**APP-INV-090: Processor Idempotency**

*Processor Handle is deterministic: same event and same state always produce the same derived events. Re-fold fires processors only on primary events (those without derived_by).*

```
forall processor P, event E, state S:
  P.Handle(E, S) = P.Handle(E, S)  // deterministic
forall derived event D with D.derived_by != nil:
  processors_skip(D)
```

Violation scenario: Re-fold fires processors on derived events, producing duplicate validation warnings.

Validation: Fold same events twice, verify derived event count identical.

// WHY THIS MATTERS: Idempotency enables safe re-fold from any checkpoint. Without it, processor outputs accumulate unboundedly across replays.

---

**APP-INV-091: Processor Failure Isolation**

*A processor error is non-fatal: the engine logs the error, discards that processor's derived events, and continues fold. No single processor failure can halt materialization.*

```
forall processor P where P.Handle returns error:
  fold continues with remaining processors
  P.derived_events = discarded
  error logged to stream
```

Violation scenario: Validation processor panics on malformed invariant; fold aborts, leaving database in partial state.

Validation: Register error-returning processor, verify fold completes and database is consistent.

// WHY THIS MATTERS: Processor isolation ensures the core materialization pipeline is robust. A broken custom processor cannot corrupt the specification database.

---

**APP-INV-092: Derived Event Provenance**

*Derived events carry the triggering event ID in their causes array and a derived_by field identifying the producing processor.*

```
forall derived event D:
  D.causes contains trigger_event.ID
  D.payload.derived_by = processor.Name()
```

Violation scenario: Derived event without causes reference breaks causal tracing during bisect.

Validation: Fold events, verify all derived events have non-empty causes and derived_by fields.

// WHY THIS MATTERS: Derived event provenance enables causal tracing through processor outputs. Without it, bisect cannot distinguish primary from derived events.

---

**APP-INV-093: Snapshot Creation Determinism**

*StateHash computed over canonicalized content tables is deterministic: two materializations of the same event prefix produce identical hashes.*

```
forall event prefixes P, materializations M1(P) and M2(P):
  StateHash(M1) = StateHash(M2)
```

Violation scenario: StateHash includes auto-increment IDs; two materializations produce different hashes.

Validation: Materialize same events twice, verify hash equality.

// WHY THIS MATTERS: Snapshot verification depends on deterministic hashing. Non-deterministic hashes make it impossible to detect snapshot corruption.

---

**APP-INV-094: Snapshot Monotonicity**

*Snapshots are monotonically ordered by event position. A snapshot at position N certifies the state as of event N; no snapshot may exist at a position less than a prior snapshot.*

```
forall snapshots S1, S2 where S1.created_at < S2.created_at:
  S1.position <= S2.position
```

Violation scenario: Snapshot at position 500 followed by snapshot at position 300 causes accelerated fold to skip events 300-500.

Validation: Create multiple snapshots, verify positions are non-decreasing.

// WHY THIS MATTERS: Monotonicity ensures accelerated fold correctness. A non-monotonic snapshot sequence would cause events to be skipped or replayed.

---

**APP-INV-095: Snapshot Recovery Graceful Degradation**

*A corrupted snapshot or hash mismatch triggers full replay from the beginning of the event log. No silent corruption propagation.*

```
forall snapshots S where verify(S) = false:
  fold(log, S) = fold(log, empty)  // fallback to full replay
```

Violation scenario: Corrupted snapshot passes verification; fold resumes from corrupt state producing incorrect database.

Validation: Corrupt a snapshot, verify the engine falls back to full replay and produces correct state.

// WHY THIS MATTERS: Graceful degradation ensures snapshots are a pure optimization. A corrupt snapshot must never produce incorrect results.

---

**APP-INV-096: Pipeline Round-Trip Preservation**

*The full event-sourcing pipeline preserves all structural content modulo metadata. StructuralDiff(parsed, materialize(import(parsed))) = empty.*

```
forall parsed databases D:
  StructuralDiff(D, materialize(import(D))) = empty_set
```

Violation scenario: Project drops glossary entries; re-parsing produces database missing terms.

Validation: Full round-trip on CLI spec, compare parsed databases via StructuralDiff.

// WHY THIS MATTERS: Round-trip preservation proves the event-sourcing pipeline is a faithful representation. Any content loss indicates a structural defect.

---

**APP-INV-097: E2E Pipeline Determinism**

*Running the pipeline twice on the same input produces identical output. StateHash equality across runs.*

```
forall inputs I, pipeline runs R1(I) and R2(I):
  StateHash(materialize(R1)) = StateHash(materialize(R2))
```

Violation scenario: Timestamps in JSONL events differ between runs, breaking determinism.

Validation: Run pipeline twice, compare output byte-for-byte.

// WHY THIS MATTERS: Deterministic output enables reproducible builds, meaningful diffing, and reliable CI gates.

---

## Architecture Decision Records

### APP-ADR-058: JSONL as Canonical Representation

#### Problem

The DDIS CLI currently treats markdown files as the source of truth and uses JSONL only for audit-trail metadata. This inverts the intended event-sourced architecture and makes temporal queries, causal tracing, and deterministic replay impossible.

#### Options

1. **Keep markdown as source of truth** (status quo). Markdown is human-readable and VCS-friendly. But it lacks structured content, causal ordering, and deterministic replay.
2. **SQLite as source of truth**. Structured and queryable. But binary format is VCS-hostile, and SQLite does not guarantee append-only semantics.
3. **JSONL as source of truth**. Append-only, human-readable, VCS-friendly, naturally ordered, self-describing events with structured payloads.

#### Decision

**Option C: JSONL as source of truth.** JSONL event logs are the canonical representation. SQLite is a materialized view. Markdown is a rendered projection.

WHY NOT markdown: Markdown lacks structured fields, causal ordering, and deterministic replay. It conflates content with formatting. Changes are opaque diffs, not semantic events.
WHY NOT SQLite: Binary format makes VCS diffs meaningless. No append-only guarantee (mutable tables). Cannot be replayed to arbitrary point in time.

#### Consequences

- All content-mutating operations must append events before modifying any other state
- The SQLite database becomes disposable (delete and regenerate from log)
- VCS diffs become semantically meaningful (each line is a complete event)
- Temporal queries become trivial (fold to any point)
- Storage cost increases (events are more verbose than markdown)

#### Tests

- Test that deleting the SQLite DB and re-materializing produces identical state
- Test that every content-mutating command appends an event before any SQL write
- Test that the event log alone is sufficient to reconstruct the full spec

---

### APP-ADR-059: Deterministic Fold over Incremental Mutation

#### Problem

The materialization pipeline must convert event sequences into SQLite state. Two approaches: (1) deterministic fold from scratch (or snapshot), or (2) incremental mutation where each command directly modifies the live database.

#### Options

1. **Incremental mutation**. Each command directly modifies SQLite. Faster for single operations but state depends on execution history, not just event content.
2. **Deterministic fold**. Process events through a pure `apply` function. Same events always produce same state. Enables snapshots and temporal queries.

#### Decision

**Option B: Deterministic fold.** Full replay from scratch (or from a snapshot checkpoint). The `apply` function is a pure function of (current_state, event) -> new_state. No side effects, no I/O, no system clock.

WHY NOT incremental mutation: Incremental mutation makes state depend on execution order, concurrency timing, and environmental factors. It precludes temporal queries and makes debugging state corruption nearly impossible.

#### Consequences

- Full replay is O(n) in event count (mitigated by snapshots)
- The `apply` function must be a pure function (no time.Now, no rand, no os.Getenv)
- Schema changes require updating the apply function, not writing migration scripts
- Testing is trivial: same input always produces same output

#### Tests

- Test fold determinism: same events -> identical SQLite (byte-level comparison)
- Test environment independence: different system clock, locale -> identical result
- Test snapshot equivalence: fold(snapshot, remaining) = fold(all)

---

### APP-ADR-060: Event References for Causal Metadata

#### Problem

Events need causal metadata to support ordering, provenance, and CRDT merge. Several mechanisms exist: Lamport timestamps, vector clocks, or explicit causal references.

#### Options

1. **Lamport timestamps**. A single monotonically increasing counter. Simple but captures only total order, not causal structure.
2. **Vector clocks**. One counter per agent. Captures causal structure but size grows linearly with agent count.
3. **Explicit causal references**. Each event carries a `causes` array of event IDs. Captures exact causal structure with fixed overhead per reference.

#### Decision

**Option C: Explicit causal references.** `causes` array of event IDs on each event. Events reference the specific prior events they causally depend on. The transitive closure of these references defines the causal DAG.

WHY NOT Lamport timestamps: Total ordering is too strong; it serializes independent events unnecessarily. Lamport timestamps cannot distinguish "happened before" from "happened concurrently."
WHY NOT vector clocks: Vector clock size grows with agent count. DDIS supports unbounded agents. The fixed-size `causes` array is simpler and sufficient for DDIS's concurrency model.

#### Consequences

- Events carry a `causes: []string` field (possibly empty for genesis events)
- Causal DAG is explicit and queryable (BFS/DFS from any event)
- Merge can identify independent events (no causal path) vs. dependent events
- Event IDs must be globally unique and stable (already guaranteed by the existing ID scheme)

#### Tests

- Test causal ordering: for all events with causes, referenced events exist and precede them
- Test causal DAG is acyclic: no event transitively causes itself
- Test independent events: events without causal path commute under fold

---

### APP-ADR-061: Field Synthesis for Projections

#### Problem

Markdown projections must be generated from materialized state. Two approaches: (1) replay raw text blobs stored in events, or (2) synthesize from structured fields (title, statement, semi_formal, etc.).

#### Options

1. **Raw text replay**. Store the original markdown text in events, replay verbatim. Exact reproduction but locks format to authoring time.
2. **Field synthesis**. Store structured fields, reconstruct markdown from templates. Format can evolve independently. Enables alternative projections.

#### Decision

**Option B: Field synthesis.** Projections are assembled from structured fields using rendering templates. The rendered output is a function of the current template and the structured data, not a replay of historical text.

WHY NOT raw text replay: Raw text locks content to its original formatting. When the rendering convention changes (e.g., new heading levels, new ADR subheading format), raw-text elements retain stale formatting. Field synthesis allows format evolution without re-authoring content.

#### Consequences

- Events must carry structured fields, not just raw text
- Rendering templates are versioned and configurable
- Round-trip through import -> materialize -> project may change formatting (but not content)
- Alternative projections (HTML, JSON schema) become trivial

#### Tests

- Test that changing a render template changes all output (no stale raw_text)
- Test that project(materialize(import(md))) preserves content (not formatting)
- Test that structured fields are sufficient to reconstruct complete elements

---

### APP-ADR-062: Parse as Import Migration Path

#### Problem

The existing `ddis parse` command reads markdown and produces SQLite. The new JSONL-canonical architecture needs an `ddis import` command that reads markdown and produces events. How do we migrate without breaking existing workflows?

#### Options

1. **Big-bang migration**. Replace `parse` with `import + materialize` in a single release. Clean but risky.
2. **Parse as import wrapper**. Keep `parse` as a convenience alias that internally calls `import` + `materialize`. Gradual migration.
3. **Parallel pipelines**. Both `parse` (direct) and `import + materialize` (event-sourced) coexist until equivalence is proven.

#### Decision

**Option C: Parallel pipelines.** `ddis import` emits synthetic events from parsed markdown. `ddis materialize` folds events into SQLite. Both pipelines coexist. APP-INV-078 (Import Equivalence) gates the transition. Once equivalence is proven, `parse` becomes a thin wrapper around `import + materialize`.

WHY NOT big-bang: Too risky. Import equivalence must be proven before the switch.
WHY NOT immediate wrapper: Same risk as big-bang if import has bugs.

#### Consequences

- `ddis import` is a new command (non-breaking addition)
- `ddis parse` continues to work unchanged during the transition
- CI tests run both pipelines and diff results
- Once equivalence is proven in CI for N consecutive runs, parse can be aliased

#### Tests

- Test import equivalence: materialize(import(md)) ~ parse(md) for all test specs
- Test that import produces well-formed events (valid types, complete payloads)
- Test that import + materialize + project round-trips preserve content

---

### APP-ADR-063: Semilattice Merge for CRDT

#### Problem

Multiple agents may produce independent event streams concurrently. Merging these streams must be deterministic and order-independent. What algebraic structure governs the merge?

#### Options

1. **Total ordering** (Lamport). All events serialized. Simple but prevents concurrent authoring.
2. **Semilattice merge**. Independent events commute; causally dependent events ordered. Enables concurrency.
3. **Operational transformation** (OT). Transforms operations for concurrent application. Complex and error-prone.

#### Decision

**Option B: Semilattice merge.** The merge of independent event subsequences is a join in a semilattice. For events targeting the same element concurrently (true conflict), last-writer-wins (LWW) by timestamp with agent ID as tiebreaker.

WHY NOT total ordering: Serialization prevents concurrent multi-agent work.
WHY NOT OT: Unnecessary complexity. DDIS events are coarse-grained (element-level, not character-level). Semilattice commutativity is sufficient.

#### Consequences

- Independent events commute under fold (formally verified)
- Concurrent modifications to the same element use LWW resolution
- Merge produces the same result regardless of order
- Agent IDs are tiebreakers, so they must be unique and stable

#### Tests

- Test commutativity: merge(A, B) = merge(B, A) for independent A, B
- Test associativity: merge(merge(A, B), C) = merge(A, merge(B, C))
- Test LWW conflict resolution: concurrent updates to same element pick latest timestamp
- Test 3-way merge: three agents produce independent streams, all merge orders converge

---

### APP-ADR-064: Snapshot as Fold Checkpoint

#### Problem

Full replay from genesis is O(n) in event count. For large event logs (10,000+ events), this becomes unacceptably slow. How do we optimize without sacrificing correctness?

#### Options

1. **No optimization**. Always replay from scratch. Correct but slow.
2. **Incremental apply**. Only apply new events to existing state. Fast but state may drift from log.
3. **Snapshot checkpoints**. Save materialized state at positions. Replay from nearest snapshot. Correct and fast.

#### Decision

**Option C: Snapshot checkpoints.** The materialized SQLite state is saved at event positions along with the position index. Replay starts from the nearest snapshot and applies remaining events. Snapshots are disposable (can always replay from scratch if snapshot is corrupted).

WHY NOT incremental apply: State drift. If the live database is modified directly (bypassing events), incremental apply compounds the error. Full replay from snapshot detects and corrects drift.
WHY NOT no optimization: Impractical for event logs beyond ~1,000 events.

#### Consequences

- New `snapshots` table tracking position, state_hash, and creation time
- `ddis materialize --from-snapshot` uses nearest valid snapshot
- `ddis materialize --full` ignores snapshots (for verification)
- Snapshot validity: fold(snapshot, remaining) = fold(all)
- Snapshots are created automatically at configurable intervals

#### Tests

- Test snapshot consistency: fold from snapshot = fold from scratch
- Test snapshot at various positions: 0, n/4, n/2, 3n/4, n-1
- Test corrupted snapshot detection: invalid state_hash triggers full replay
- Test snapshot creation: automatic at every 1000 events

---

### APP-ADR-065: Stream Processors as Fold Observers

#### Problem

After content events are applied, downstream quality checks (validation, consistency, drift) need to run. How do these integrate with the fold pipeline?

#### Options

1. **Manual invocation**. User runs `ddis validate` after every change. Correct but burdensome.
2. **Post-fold hooks**. Processors register as observers, fire after fold steps. Automatic and composable.
3. **In-fold validation**. Validation runs inside the apply function. Tight coupling, blocks fold progress.

#### Decision

**Option B: Post-fold observers.** Stream processors register with the materialize engine and fire after each content event is applied. Processors observe the post-apply state and may append their own events (with `causes` referencing the triggering event). Processors are idempotent and independently deployable.

WHY NOT manual invocation: Defeats the purpose of event-driven architecture. Users should not need to remember to validate after every change.
WHY NOT in-fold validation: Coupling validation to fold makes the apply function impure (it now depends on validation rules, which may change). Observers maintain the purity of apply while still providing automatic quality feedback.

#### Consequences

- ProcessorRegistry in the materialize engine
- Each processor has: name, event_types (filter), handler function
- Handler receives: (event, post_apply_state) -> []Event (derived events)
- Derived events have causes = [triggering_event.ID]
- Processor failure is non-fatal (logs error, continues fold)
- Built-in processors: validation, consistency, drift

#### Tests

- Test processor registration and firing order
- Test processor idempotency: processing same event twice produces same derived events
- Test processor failure isolation: one processor error does not affect others
- Test derived event causality: all derived events reference their trigger

---

## Chapter 1: Event Content Schema

### Content-Bearing Event Types

The event-sourced architecture extends the existing 28 metadata-only event types with content-bearing types that carry full structured payloads. Each content event type corresponds to a single state mutation in the materialized SQLite database.

**Stream 2 (Specification) --- New Content Types:**

| Type | Payload Fields | State Mutation |
|---|---|---|
| `spec_section_defined` | module, path, title, body, level | INSERT into sections |
| `spec_section_updated` | module, path, title, body, changes | UPDATE sections |
| `spec_section_removed` | module, path, reason | DELETE from sections |
| `invariant_crystallized` | id, title, statement, semi_formal, violation, validation, why, module | INSERT into invariants |
| `invariant_updated` | id, fields_changed, new_values | UPDATE invariants |
| `invariant_removed` | id, reason, superseded_by | DELETE from invariants |
| `adr_crystallized` | id, title, problem, options, decision, consequences, tests, module | INSERT into adrs |
| `adr_updated` | id, fields_changed, new_values | UPDATE adrs |
| `adr_superseded` | id, superseded_by, reason | UPDATE adrs (status=superseded) |
| `negative_spec_added` | module, pattern, rationale | INSERT into negative_specs |
| `quality_gate_defined` | gate_number, title, predicate | INSERT into quality_gates |
| `cross_ref_added` | source, target, context | INSERT into cross_references |
| `glossary_term_defined` | term, definition, module | INSERT into glossary_entries |
| `module_registered` | name, domain, maintains, interfaces, implements, adjacent | INSERT into modules |
| `manifest_updated` | field, old_value, new_value | UPDATE manifest |
| `snapshot_created` | position, state_hash | INSERT into snapshots |

**Stream 3 (Implementation) --- Enhanced Content Types:**

| Type | Payload Fields | State Mutation |
|---|---|---|
| `witness_recorded` | invariant_id, evidence_type, evidence, by, model, code_hash, spec_hash | INSERT into invariant_witnesses |
| `witness_revoked` | invariant_id, reason | UPDATE invariant_witnesses (valid=false) |
| `witness_invalidated` | invariant_id, reason | UPDATE invariant_witnesses (valid=false) |
| `challenge_completed` | invariant_id, verdict, levels, score, detail | INSERT into challenge_results |

### Event Struct Extension

The existing Event struct gains two fields:

```
Event {
  ID        string          // Existing: auto-generated unique ID
  Type      string          // Existing: event type from schema
  Timestamp string          // Existing: RFC3339 UTC
  SpecHash  string          // Existing: SHA-256 of spec
  Stream    int             // Existing: 1, 2, or 3
  Payload   json.RawMessage // Existing: type-specific JSON
  Causes    []string        // NEW: IDs of causally preceding events
  Version   int             // NEW: schema version for forward compat
}
```

The `Causes` field enables causal DAG construction (APP-INV-074). The `Version` field enables forward-compatible event schema evolution without breaking existing consumers.

### Event Validation Rules

Extended validation for content-bearing events: all existing rules (ID, Timestamp, Type, Stream) remain, plus content events must have non-empty Payload with all required fields, Causes references must resolve to existing events, and content events must not duplicate element IDs (except update/supersede types).

---

## Chapter 2: Fold/Materialize Engine

### The Apply Function

The core of the materialization pipeline is the `apply` function, a pure function that maps (state, event) to a new state:

```
apply : (SQLiteState, Event) -> SQLiteState

apply(state, e) = case e.Type of
  <20 content types from Chapter 1 table>  -> mutation(state, e.Payload)
  _                                        -> state  // forward compat
```

Each content type maps to exactly one SQL mutation (INSERT, UPDATE, or DELETE) on its corresponding table. See Chapter 1 for the full type-to-mutation mapping. No file I/O, no system clock, no randomness --- the function is pure (APP-INV-073).

### Materialize Pipeline

The full materialize pipeline:

```
materialize(log, snapshot?) -> SQLiteState

1. If snapshot provided and valid:
     state = load(snapshot.state)
     events = log[snapshot.position:]
   Else:
     state = create_empty_db(schema)
     events = log[0:]

2. For each event e in events (respecting causal order):
     state = apply(state, e)
     for each processor p in registered_processors:
       derived = p.handle(e, state)
       for each d in derived:
         state = apply(state, d)
         append(log, d)

3. Return state
```

### Causal Sort and Error Handling

Events are sorted into causal order before folding (see Chapter 5 for the full algorithm). The engine collects errors without halting: unknown types become no-ops, and all errors are reported after the fold completes.

---

## Chapter 3: Projection/Synthetic Render

### Projection Architecture

Projections transform materialized SQLite state into human-readable output (markdown, HTML, JSON schema). Each module is projected independently, with the constitution aggregated separately:

```
project : SQLiteState -> {ModuleName -> MarkdownContent}

project(state) = for each module m in state.modules:
  parts = [render_frontmatter(m), render_overview(m),
           render_invariants(m, state), render_adrs(m, state),
           render_chapters(m, state), render_negative_specs(m, state),
           render_xref_index(m, state)]
  yield (m.name, join(parts))
```

### Element Rendering Templates

Invariant rendering assembles structured fields into the canonical format:

```
render_invariant(inv) ->
  **{inv.id}: {inv.title}**
  *{inv.statement}*
  ```{inv.semi_formal}```
  Violation scenario: {inv.violation_scenario}
  Validation: {inv.validation_method}
  // WHY THIS MATTERS: {inv.why_this_matters}
```

ADR rendering follows the heading-based format with Problem, Options, Decision, Consequences, and Tests subheadings. Both templates use structured fields exclusively --- no raw_text replay. This guarantees APP-INV-077 (Synthetic Render): changing the template changes ALL output.

### Constitution Rendering

The constitution is rendered separately since it aggregates declarations from all modules:

```
render_constitution(state) ->
  join([render_metadata, render_executive_summary,
        render_state_space_model, render_inv_registry(sorted by ID),
        render_adr_registry(sorted by ID), render_quality_gates,
        render_glossary, render_module_map])
```

Each component is a pure function of the materialized state. The invariant and ADR registries are sorted by ID to ensure deterministic output regardless of insertion order in the database.

---

## Chapter 4: Import Migration Bridge

### Import Pipeline

The `ddis import` command reads existing markdown specifications and emits synthetic events to the JSONL log. Each structural element becomes a content-bearing event.

```
import : MarkdownFile -> [Event]

import(md) =
  1. Parse markdown into sections (reusing existing 4-pass parser)
  2. For each module in manifest:
     emit module_registered event
  3. For each section in parsed tree:
     emit spec_section_defined event with structured fields
  4. For each invariant in parsed elements:
     emit invariant_crystallized event with all 6 mandatory fields
  5. For each ADR in parsed elements:
     emit adr_crystallized event with all fields
  6. For each cross-reference in parsed xref graph:
     emit cross_ref_added event
  7. For each negative spec in parsed modules:
     emit negative_spec_added event
  8. For each glossary entry in parsed elements:
     emit glossary_term_defined event
  9. For each quality gate in parsed elements:
     emit quality_gate_defined event
  10. Emit manifest_updated event with full manifest content
```

### Synthetic Event Properties

Events emitted by import are "synthetic" --- they were not produced by an interactive crystallize session. They carry the following distinguishing properties:

- `causes` is empty (no prior events in the log)
- Payload contains a `synthetic: true` flag
- Timestamp is the import time (not the original authoring time)
- SpecHash is computed from the current markdown content

Import equivalence (APP-INV-078) requires that materializing these synthetic events produces the same SQLite state as direct parsing.

### Conflict Detection

If importing into a non-empty event log, the import command detects conflicts:

- **Duplicate element**: An invariant with the same ID already exists in the log. Resolution: skip (the event log version is canonical) or `--force` (overwrite with import data, emitting an update event).
- **Schema mismatch**: A section path in the import does not match the module structure in the log. Resolution: report as warning, import with adjusted path.
- **Cross-ref target missing**: A cross-reference targets an element not present in the import set. Resolution: report as warning, import the reference anyway (it may resolve from existing log entries).

---

## Chapter 5: Causal DAG and CRDT Merge

### Causal DAG Construction

The causal DAG is constructed from the `causes` fields of all events:

```
build_dag(events) -> DAG

for each event e in events:
  add_node(dag, e.ID)
  for each cause_id in e.Causes:
    add_edge(dag, cause_id, e.ID)  // cause -> effect

verify_acyclic(dag)  // Error if cycle detected
```

### Topological Sort for Fold

Events must be folded in a causal-consistent order. The topological sort produces a valid linearization:

```
causal_sort(dag) -> [Event]

1. Compute in-degrees for all nodes
2. Initialize queue with nodes of in-degree 0
3. While queue is non-empty:
     e = dequeue (by timestamp for stable ordering)
     emit e
     for each successor s of e:
       decrement in_degree(s)
       if in_degree(s) == 0: enqueue(s)
4. If emitted count != node count: cycle detected (error)
```

### CRDT Merge Algorithm

Merging independent event streams from multiple agents:

```
merge(stream_A, stream_B) -> merged_stream

1. Partition: shared (both streams), unique_A, unique_B (by event ID)
2. Conflict detect: for each (a, b) targeting same element:
     winner = lww_resolve(a, b)  // later timestamp wins; agent ID tiebreaker
3. merged = shared + unique_A + unique_B (minus superseded losers)
4. causal_sort(merged); return
```

Two events are **independent** iff no causal path exists between them: `NOT reachable(dag, e1, e2) AND NOT reachable(dag, e2, e1)`. Independent events commute under fold (APP-INV-081), verified by applying in both orders and comparing states.

---

## Chapter 6: Temporal Queries, Bisect, and Blame

### Temporal Queries

A temporal query answers "what was the specification state at time t?" by folding events up to that timestamp:

```
query_at(log, t) -> SQLiteState
  events_at_t = filter(log, e -> e.Timestamp <= t)
  snap = latest_snapshot_before(snapshots, t)
  if snap: return foldl(snap.state, filter(log[snap.position:], e -> e.Timestamp <= t))
  else:    return materialize(events_at_t)
```

Temporal diff compares specification state between two time points, reusing the existing `ddis diff` infrastructure (APP-INV-007) with materialized-at-time states.

### Bisect

Binary search over event positions to find the earliest defect-introducing event:

```
bisect(log, predicate) -> Event
  lo, hi = 0, len(log) - 1
  while lo < hi:
    mid = (lo + hi) / 2
    state = materialize(log[0:mid+1])
    if predicate(state): hi = mid    // defect present
    else:                lo = mid + 1
  return log[lo]
```

Predicates: `validate(state).errors > 0`, `contradict(state).count > 0`, `lookup(state, id) == nil`. With snapshots, each materialize uses the nearest prior snapshot for O(k) per iteration.

### Blame

Trace a spec element back to its originating and modifying events:

```
blame(state, element_id) -> [Event]
  1. Find creation event: scan log for crystallized/defined with element_id
  2. Find modification events: scan log for updated/superseded with element_id
  3. Sort chronologically; return complete provenance chain
  Optimization: event_provenance table maps element_id -> [event_id]
```

The `ddis blame` command renders the provenance chain with diffs between consecutive versions.

### Replay

Materialize to a specific event (by ID or position):

```
replay(log, target) -> SQLiteState
  position = find_position(log, target) if target is event ID, else target
  return materialize(log[0:position+1])
```

`ddis replay` provides the materialize-to-position primitive that temporal queries, bisect, and blame build on.

---

## Negative Specifications

**DO NOT** read or write markdown files as the canonical source of truth. The event log is canonical; markdown is a projection. Any command that writes markdown must do so via the project pipeline (materialize -> project), never by direct file manipulation.

**DO NOT** store mutable state in the SQLite database that is not derivable from the event log. Every table, row, and column in the materialized database must be reproducible by replaying the event log from scratch. If deleting the database and re-materializing loses information, that information was stored outside the canonical representation.

**DO NOT** use raw_text blobs for rendering projections. All projected output must be synthesized from structured fields (title, statement, semi_formal, etc.) using rendering templates. Raw text replay locks content to its original formatting and prevents format evolution.

**DO NOT** break causal ordering during merge operations. When merging independent event streams, the causal DAG must remain acyclic and all causal references must resolve. An event that references a cause not present in the merged stream is an error, not a silent skip.

**DO NOT** allow non-deterministic materialize results. The fold function must be a pure function of the event sequence. No system clock reads (use event timestamps), no random number generation (use deterministic IDs), no environment variable access, no file I/O beyond reading the event stream.

**DO NOT** bypass the event log for content mutations. Commands like crystallize, witness, and challenge must append events to the JSONL log BEFORE (or instead of) modifying SQLite directly. Direct SQL modifications create state that is invisible to replay and will be lost when the database is regenerated.

---

## Cross-Reference Index

| Source | Target | Relationship |
|---|---|---|
| APP-INV-071 (Log Canonicality) | APP-INV-020 (Event Stream Append-Only) | Depends: canonicality requires append-only |
| APP-INV-071 (Log Canonicality) | APP-INV-048 (Event Stream VCS) | Depends: canonical log must be VCS-tracked |
| APP-INV-072 (Event Content Completeness) | APP-INV-073 (Fold Determinism) | Enables: content completeness is precondition for fold determinism |
| APP-INV-073 (Fold Determinism) | APP-INV-002 (Validation Determinism) | Extends: fold determinism generalizes validation determinism to full pipeline |
| APP-INV-073 (Fold Determinism) | APP-INV-015 (Deterministic Hashing) | Uses: fold uses deterministic hashing for content comparison |
| APP-INV-074 (Causal Ordering) | APP-INV-081 (CRDT Convergence) | Enables: causal ordering identifies independent events for CRDT merge |
| APP-INV-075 (Materialization Idempotency) | APP-INV-071 (Log Canonicality) | Proves: idempotent materialization confirms log canonicality |
| APP-INV-076 (Projection Purity) | APP-INV-001 (Round-Trip Fidelity) | Parallel: projection purity is the event-sourced analog of round-trip fidelity |
| APP-INV-077 (Synthetic Render) | APP-INV-076 (Projection Purity) | Implements: synthetic render is how projection purity is achieved |
| APP-INV-078 (Import Equivalence) | APP-INV-001 (Round-Trip Fidelity) | Gates: import equivalence verified against parse round-trip as baseline |
| APP-INV-079 (Temporal Query Soundness) | APP-INV-073 (Fold Determinism) | Requires: temporal queries depend on deterministic fold |
| APP-INV-079 (Temporal Query Soundness) | APP-INV-074 (Causal Ordering) | Requires: temporal queries need correct causal ordering to avoid corrupt state |
| APP-INV-080 (Stream Processor Reactivity) | APP-INV-053 (Event Stream Completeness) | Extends: stream processors ensure completeness of derived events |
| APP-INV-081 (CRDT Convergence) | APP-INV-074 (Causal Ordering) | Uses: CRDT merge depends on causal independence detection |
| APP-INV-082 (Bisect Correctness) | APP-INV-079 (Temporal Query Soundness) | Uses: bisect uses temporal queries to check defect at each position |
| APP-INV-082 (Bisect Correctness) | APP-INV-083 (Snapshot Consistency) | Optimized by: bisect uses snapshots for O(log n) efficiency |
| APP-INV-083 (Snapshot Consistency) | APP-INV-073 (Fold Determinism) | Requires: snapshots valid only if fold is deterministic |
| APP-INV-083 (Snapshot Consistency) | APP-INV-075 (Materialization Idempotency) | Proves: snapshot consistency is a special case of materialization idempotency |
| APP-INV-084 (Causal Provenance) | APP-INV-074 (Causal Ordering) | Uses: provenance chains follow causal edges |
| APP-INV-084 (Causal Provenance) | APP-INV-025 (Discovery Provenance) | Extends: event provenance extends discovery provenance to individual events |
| APP-ADR-058 (JSONL Canonical) | APP-ADR-007 (JSONL Oplog) | Generalizes: extends oplog's append-only JSONL to full content events |
| APP-ADR-059 (Deterministic Fold) | APP-ADR-058 (JSONL Canonical) | Implements: fold is the mechanism that derives state from the canonical log |
| APP-ADR-060 (Causal References) | APP-ADR-015 (Three-Stream Event Sourcing) | Extends: adds causal metadata to the three-stream model |
| APP-ADR-061 (Field Synthesis) | APP-ADR-058 (JSONL Canonical) | Complements: structured fields in events enable synthetic rendering |
| APP-ADR-062 (Parse as Import) | APP-ADR-009 (4-Pass Parse Pipeline) | Wraps: import reuses the 4-pass parser to extract structured data |
| APP-ADR-063 (Semilattice Merge) | APP-ADR-060 (Causal References) | Uses: merge depends on causal references to identify independent events |
| APP-ADR-064 (Snapshot Checkpoint) | APP-ADR-059 (Deterministic Fold) | Optimizes: snapshots accelerate the deterministic fold |
| APP-ADR-065 (Stream Processors) | APP-ADR-058 (JSONL Canonical) | Extends: processors append derived events to the canonical log |
| APP-INV-085 (Import Content Completeness) | APP-INV-078 (Import Equivalence) | Precondition: import must emit all content types for equivalence to hold |
| APP-INV-086 (Applier Spec-ID) | APP-INV-073 (Fold Determinism) | Requires: applier parameterization needed for deterministic multi-spec fold |
| APP-INV-087 (Projector Section Rendering) | APP-INV-076 (Projection Purity) | Implements: module-filtered projection is how purity is enforced |
| APP-INV-088 (Single Write Path) | APP-INV-071 (Log Canonicality) | Enforces: single write path ensures log is sole source of truth |
| APP-INV-089 (Deprecation Bridge) | APP-INV-078 (Import Equivalence) | Gates: deprecation bridge verified via structural equivalence |
| APP-INV-090 (Processor Idempotency) | APP-INV-080 (Stream Processor Reactivity) | Constrains: idempotency constrains how processors fire during re-fold |
| APP-INV-091 (Processor Failure Isolation) | APP-INV-080 (Stream Processor Reactivity) | Protects: failure isolation ensures processor errors don't halt fold |
| APP-INV-092 (Derived Event Provenance) | APP-INV-084 (Causal Provenance) | Extends: derived provenance extends causal provenance to processor outputs |
| APP-INV-093 (Snapshot Determinism) | APP-INV-073 (Fold Determinism) | Requires: snapshot hash determinism depends on fold determinism |
| APP-INV-094 (Snapshot Monotonicity) | APP-INV-083 (Snapshot Consistency) | Constrains: monotonicity ensures snapshots can be used for accelerated fold |
| APP-INV-095 (Snapshot Recovery) | APP-INV-083 (Snapshot Consistency) | Protects: recovery ensures corrupt snapshots don't produce corrupt state |
| APP-INV-096 (Pipeline Round-Trip) | APP-INV-078 (Import Equivalence) | Extends: round-trip extends import equivalence to full pipeline |
| APP-INV-097 (E2E Determinism) | APP-INV-073 (Fold Determinism) | Extends: E2E determinism extends fold determinism to full pipeline |
| APP-ADR-066 (Self-Bootstrap Gate) | APP-ADR-062 (Parse as Import) | Tests: self-bootstrap verifies the parse-to-import migration |
| APP-ADR-067 (Structural Equivalence) | APP-ADR-066 (Self-Bootstrap Gate) | Defines: what equality means for self-bootstrap comparison |
| APP-ADR-068 (Phased Migration) | APP-ADR-062 (Parse as Import) | Extends: defines the full migration lifecycle |
| APP-ADR-069 (Crystallize Event-Only) | APP-ADR-068 (Phased Migration) | Implements: crystallize event-only is Phase B for crystallize command |
| APP-ADR-070 (Processor Registration) | APP-ADR-065 (Stream Processors) | Specifies: how processors are registered and discovered |
| APP-ADR-071 (Derived Dedup) | APP-ADR-070 (Processor Registration) | Constrains: deduplication strategy for processor-produced events |
| APP-ADR-072 (Snapshot State Hash) | APP-ADR-064 (Snapshot Checkpoint) | Implements: state hash is how snapshot integrity is verified |
| APP-ADR-073 (Snapshot Interval) | APP-ADR-072 (Snapshot State Hash) | Configures: when snapshots are automatically created |
| APP-ADR-074 (E2E Test Architecture) | APP-ADR-066 (Self-Bootstrap Gate) | Specifies: test architecture for self-bootstrap and behavioral tests |

---

# Chapter 7: Self-Bootstrap Verification

This chapter defines the self-bootstrap pipeline: the mechanism by which the DDIS CLI validates its own event-sourcing pipeline by running it on the CLI specification itself. The pipeline verifies that `import → materialize → project` produces structurally equivalent output to the `parse` command.

## 7.1 Self-Bootstrap Pipeline Definition

The self-bootstrap pipeline is a 5-stage verification sequence:

1. **Parse**: `ddis parse manifest.yaml -o parsed.db` — baseline SQLite from direct parsing
2. **Import**: `ddis import --db parsed.db` — emit synthetic JSONL events from parsed database
3. **Materialize**: `ddis materialize --stream events/ -o materialized.db` — fold events into fresh SQLite
4. **Compare**: `StructuralDiff(parsed.db, materialized.db)` — verify structural equivalence
5. **Project** (optional): `ddis project --db materialized.db -o output/` — render markdown from materialized state

The pipeline exercises every content type: modules, invariants, ADRs, sections, cross-references, negative specs, quality gates, and glossary entries. A successful self-bootstrap run proves the event-sourcing pipeline is functionally equivalent to the legacy parse pipeline (APP-INV-078, APP-INV-085).

## 7.2 Structural Equivalence Comparator

`StructuralDiff(db1, db2) []Difference` compares two SQLite databases row-by-row for structural content equivalence.

**Compared**: content-bearing fields from invariants, adrs, sections, glossary, cross_refs, negative_specs, quality_gates, modules.
**Excluded**: auto-increment id, parsed_at, created_at, raw_text, content_hash, source_file_id, spec_id.

`StateHash(db, specID) string` computes SHA-256 over the deterministic serialization of all content tables. Rows are sorted by primary key, fields are serialized in schema order, and only content-bearing fields are included. This function is used for both snapshot verification (APP-INV-093) and structural equivalence shortcut (APP-ADR-067).

## 7.3 Per-Content-Type Completeness Verification

APP-INV-085 requires import to emit events for ALL content types. Verification compares event counts against parsed database row counts:

Verification: for each of the 8 content types (module, invariant, ADR, section, cross-ref, negative spec, quality gate, glossary), compare count of emitted events against parsed database row counts. All 8 must match for equivalence.

## 7.4 Integration Test Architecture

`TestSelfBootstrapEventPipeline` (build tag `integration`) uses the real CLI spec as fixture: parse → import → materialize → StructuralDiff (empty = pass) → per-content-type count verification (APP-INV-085). See APP-INV-085 through APP-INV-097 in the invariant definitions above.

### APP-ADR-066: Self-Bootstrap Pipeline as Integration Gate

#### Problem
The event-sourcing pipeline must be verified against the CLI spec itself.

#### Decision
**Option C: Self-bootstrap.** Run the full pipeline on the CLI spec as a CI gate, exercising all content types and edge cases. // WHY NOT A/B: Synthetic fixtures miss real-world edge cases.

Consequences: Self-bootstrap exercises all content types, cross-references, and edge cases present in the real spec.
#### Tests
TestSelfBootstrapEventPipeline in tests/pipeline_integration_test.go.

### APP-ADR-067: Structural Equivalence Definition

#### Problem
APP-INV-078 requires import+materialize to equal parse, but equality needs precise definition.

#### Decision
**Option C: Content-only comparison.** Excluding auto-increment id, parsed_at, created_at, raw_text, content_hash. Implemented as StructuralDiff and StateHash. // WHY NOT A: Metadata differs legitimately. B: Row counts miss field-level differences.

Consequences: Implemented as StructuralDiff and StateHash functions testing structured information.
#### Tests
TestStructuralDiff_Determinism, TestStructuralDiff_Sensitivity in internal/materialize/diff_test.go.

### APP-ADR-074: E2E Test Architecture

#### Problem
Event-sourcing tests span multiple packages and commands.

#### Decision
**Option C: Hybrid.** In-process for behavioral tests (fast, debuggable), subprocess for E2E (catches integration issues). Real CLI spec as fixture. // WHY NOT A/B: Pure unit tests miss integration; pure subprocess tests are slow.

Consequences: In-process tests are fast; subprocess E2E catches integration issues.
#### Tests
tests/invariant_behavioral_test.go, tests/e2e_pipeline_test.go, tests/pipeline_integration_test.go.

---

# Chapter 8: Architecture Migration Strategy

This chapter defines the phased migration from direct markdown/SQLite writes to event-only writes. The migration proceeds through three phases: Phase A (import bridge, done), Phase B (event-first with deprecation wrappers), and Phase C (remove old paths).

## 8.1 Migration Phase Definitions

| Phase | State | Write Path | Read Path |
|---|---|---|---|
| **A** (done) | Import bridge | Events emitted alongside direct SQL/markdown | Direct SQL reads |
| **B** (current) | Event-first | Events are sole write target; parse/render become wrappers | Materialize for SQL, project for markdown |
| **C** (target) | Event-only | Old write paths removed entirely | All reads from materialized views |

Phase transitions are gated by structural equivalence verification: the new path must produce identical content to the old path (APP-INV-089).

## 8.2 Write-Path Command Inventory

Commands that mutate content and must transition to event-only writes:

| Command | Current Write | Phase B | Phase C |
|---|---|---|---|
| `crystallize` | Event + markdown + manifest | Event only + auto-project | Event only |
| `refine` | Direct markdown edit | Event + auto-project | Event only |
| `absorb` | Direct markdown merge | Event + auto-project | Event only |
| `witness` | Event + SQL insert | Event only + materialize | Event only |
| `challenge` | Event + SQL insert | Event only + materialize | Event only |
| `parse` | Direct SQL insert | Wrapper: import + materialize | Removed (deprecated) |
| `render` | Direct markdown read from SQL | Wrapper: project | Removed (deprecated) |

## 8.3 Crystallize Event-Only Architecture

After Phase B, crystallize follows this sequence (APP-ADR-069):

1. Read JSON input, validate
2. Emit `invariant_crystallized` or `adr_crystallized` event to Stream 2
3. Emit `decision_crystallized` event to Stream 1
4. If `--no-project` is NOT set: run `materialize(stream) → project(db)` internally
5. Report result

The event is the sole write. Markdown exists only as a projection from the event stream.

## 8.4 Parse/Render Deprecation Wrapper Mechanism

During Phase B, `parse` and `render` are thin wrappers (APP-INV-089):

**parse wrapper**: `parse(manifest)` → emit deprecation warning → `import(parse_legacy(manifest))` → `materialize(events)` → return database
**render wrapper**: `render(db)` → emit deprecation warning → `project(db)` → return markdown

Both wrappers verify structural equivalence between old and new output before returning. If equivalence fails, the wrapper falls back to the legacy path and emits a warning.

## 8.5 Migration Gate & Rollback

Phase B → Phase C transition requires: (1) all behavioral tests pass with event-only paths, (2) self-bootstrap pipeline test passes (APP-INV-096), (3) zero structural differences between parse and import+materialize output, (4) 10+ consecutive green CI runs. Rollback to Phase A is always safe: event emission is additive and legacy paths remain functional. Events emitted during Phase B remain valid for future replay.

See APP-INV-088, APP-INV-089 in the invariant definitions section above.

### APP-ADR-068: Phased Migration Strategy

#### Problem
Transitioning from direct writes to event-only writes cannot be done atomically.

#### Decision
**Option B: Phased migration.** Structural equivalence (APP-INV-089) gates each transition. Rollback to Phase A always safe. // WHY NOT A: Big-bang rewrite is all-or-nothing. C: Feature flags add complexity.

Consequences: Incremental verification at each step. Rollback always safe.
#### Tests
TestParseDeprecationWrapper, TestRenderDeprecationWrapper verify equivalent output.

### APP-ADR-069: Crystallize Event-Only Path

#### Problem
Crystallize dual-writes to event log AND markdown, violating APP-INV-088.

#### Decision
**Option C: Crystallize event-only.** Emits event, then auto-projects via materialize+project (preserves UX). --no-project to skip. // WHY NOT A: Dual-write is the bug. B: Separate project breaks UX.

Consequences: Preserves UX of immediate markdown updates. Event is sole source of truth.
#### Tests
TestCrystallizeEventOnly, TestCrystallizeAutoProject.

---

# Chapter 9: Stream Processor Catalog

This chapter specifies the stream processor subsystem: how processors are registered, invoked during fold, and how derived events are managed. The `Engine` orchestrates processor execution after each content event's `Apply()` step.

## 9.1 Processor Interface Specification

```go
type Processor interface {
    Name() string
    Handle(evt *events.Event, state *sql.DB) ([]*events.Event, error)
}
```

- `Name()`: Returns the processor name (used in derived event provenance)
- `Handle(evt, state)`: Receives a content event and current materialized state. Returns zero or more derived events and an optional error. The processor MUST be deterministic: same event + same state → same output (APP-INV-090).

## 9.2 FoldWithProcessors Execution Model

`Engine.FoldWithProcessors(applier, evts)` extends `Fold` with processor invocation:

1. CausalSort(evts)
2. For each event:
   a. Apply(applier, evt) — mutate SQL state
   b. If evt is a primary event (no `derived_by` in payload):
      - For each registered processor:
        - Call processor.Handle(evt, state)
        - If error: log error, discard derived events, continue (APP-INV-091)
        - If success: append derived events to output stream
   c. If evt is a derived event: skip processor invocation (APP-ADR-071)
3. Return FoldResult with primary + derived event counts

## 9.3 Built-in Processor Catalog

Three built-in processors are registered by default. All produce `implementation_finding` derived events:

| Processor | Trigger Events | Checks |
|---|---|---|
| **Validation** | `invariant_crystallized`, `adr_crystallized` | Non-empty required fields (statement, semi_formal for INV; problem, decision, consequences for ADR), ID pattern `APP-INV/ADR-NNN` |
| **Consistency** | All content events | Cross-ref resolution, no broken references, acyclic module relationships |
| **Drift** | All content events | Annotation presence for crystallized invariants, stale ADR implementation references |

Each derived event includes `derived_by: "<processor_name>"` in its payload. See APP-INV-090, APP-INV-091, APP-INV-092 in the invariant definitions section above.

### APP-ADR-070: Processor Registration Mechanism

#### Problem
Stream processors need a registration mechanism supporting built-in and custom processors.

#### Decision
**Option C: RegisterProcessor() API.** Engine.RegisterProcessor(name, Processor) with three built-ins by default. // WHY NOT A: Hardcoded prevents extension. B: Plugin system adds excessive complexity.

Consequences: Extensible via RegisterProcessor() for domain-specific analysis.
#### Tests
TestProcessorRegistration, TestBuiltInProcessorValidation in internal/materialize/fold_test.go.

---

### APP-ADR-071: Derived Event Deduplication

#### Problem
During re-fold, processors fire on derived events, creating duplicates.

#### Decision
**Option B: Skip-derived-events.** Events with derived_by are applied but processors skip them. Simplest correct: no state tracking needed. // WHY NOT A: ID tracking adds state. C: Dedup burden on every processor.

Consequences: No state tracking or processor-side dedup needed.
#### Tests
TestSkipDerivedEvents, TestReFoldIdempotency.

---

# Chapter 10: Snapshot Implementation

This chapter specifies the snapshot subsystem: how snapshots are created, verified, used for accelerated fold, and pruned. Snapshots are SQLite state checkpoints at known event positions, enabling O(k) incremental replay instead of O(n) full replay.

## 10.1 Snapshot Creation

`CreateSnapshot(db, position, eventsDir)` creates a snapshot at the given event position:

1. Compute `StateHash(db, specID)` — SHA-256 over canonicalized content tables
2. Store snapshot record in `snapshots` table: position, state_hash, created_at
3. Emit `snapshot_created` event to Stream 2

Snapshots can be created manually via `ddis snapshot create` or automatically during materialize at configured intervals (APP-ADR-073).

## 10.2 Snapshot Verification

`VerifySnapshot(db, snapshot)` verifies a snapshot's integrity:

1. Recompute `StateHash(db, specID)` at the snapshot's position
2. Compare against stored `state_hash`
3. If mismatch: return error (snapshot is corrupt)

## 10.3 Accelerated Fold

`FoldFrom(applier, startPosition, evts)` resumes fold from a snapshot position:

1. Load latest valid snapshot: `LoadLatestSnapshot(db)`
2. Verify snapshot: `VerifySnapshot(db, snapshot)`
3. If verification fails: fall back to full replay (APP-INV-095)
4. If verification passes: fold only events after snapshot.position
5. Total cost: O(verification) + O(remaining_events) instead of O(all_events)

## 10.4 Snapshot Pruning

`PruneSnapshots(db, keepLatest int)` removes old snapshots, keeping only the N most recent:

1. Query snapshots ordered by position descending
2. Delete all except the latest `keepLatest` entries
3. Pruning is safe: snapshots are disposable optimization artifacts

## 10.5 Snapshot Lifecycle

Snapshot states: `created → valid → stale → pruned`

- **created**: Just created, not yet verified
- **valid**: Verified against current state
- **stale**: Invalidated by merge or event insertion before snapshot position
- **pruned**: Deleted during pruning

CRDT merge invalidates existing snapshots because the merged event stream may differ from the original.

See APP-INV-093, APP-INV-094, APP-INV-095 in the invariant definitions section above.

### APP-ADR-072: Snapshot as SQLite State Hash

#### Problem
Snapshots need a verification mechanism to detect corruption.

#### Decision
**Option B: SHA-256 state hash.** Over canonicalized content tables, excluding auto-IDs and timestamps. Same function used for structural equivalence. // WHY NOT A: File checksum includes non-content bytes. C: Row counts miss field-level corruption.

#### Consequences
Same function used for snapshot verification and structural equivalence, reducing implementation surface.

#### Tests
TestStateHash_Determinism, TestSnapshotVerification in internal/materialize/diff_test.go.

---

### APP-ADR-073: Automatic Snapshot Interval

#### Problem
Snapshots must be created at appropriate intervals for accelerated fold.

#### Decision
**Option C: Event-count-based interval.** Every 1000 events, configurable via --snapshot-interval, 0 to disable. // WHY NOT A: No automation. B: Time-based is non-deterministic across machines.

#### Consequences
Predictable, deterministic, independent of wall-clock time. Manual creation always available via snapshot create.

#### Tests
TestAutomaticSnapshotInterval, TestManualSnapshotCreation in internal/materialize/snapshot_test.go.

---

## Chapter 11: Cleanroom Audit Hardening

This chapter addresses findings from the 2026-02-28 cleanroom software engineering audit. Each invariant below was discovered through systematic code-level trace analysis and formalized via the bilateral specification cycle.

**APP-INV-098: Snapshot Position Event-Stream Ordinal**

*The snapshot position field MUST represent the count of events processed from the canonical event stream, NOT a count of materialized content elements (e.g., invariants, sections). Position forms a monotone counter in the stream's ordinal space: it strictly increases with each applied event and enables FoldFrom() to resume at the correct stream offset.*

```
Let pos(s) be the position stored in snapshot s, and let |E_applied| be the number of events applied during the fold that produced s. Then pos(s) = |E_applied|. For any subsequent snapshot s' with pos(s') > pos(s), FoldFrom(events, pos(s)) must apply exactly the events at indices [pos(s), pos(s')-1].
```

Violation scenario: Snapshot created with position = COUNT(invariants). After an invariant upsert (no new row), position stays at N. After an invariant deletion, position decreases to N-1. FoldFrom(events, N) skips or replays wrong events, producing divergent state.

Validation: Create snapshot, count events in stream file, verify snapshot.Position == event count. Delete an invariant from DB, create new snapshot, verify position still equals event count (not invariant count).

// WHY THIS MATTERS: Snapshot-accelerated fold (APP-INV-094) relies on position to skip already-applied events. If position tracks a projection (invariant count) instead of the stream ordinal, the skip window is wrong, breaking idempotency (APP-INV-075) and determinism (APP-INV-097).

---

**APP-INV-100: Event Applier Section Hierarchy Preservation**

*The event fold applier MUST preserve section hierarchy when materializing events into the SQLite state. Content elements (invariants, ADRs, glossary entries, negative specs, quality gates) MUST be associated with their correct containing section via section_id. Hardcoding section_id to a constant (e.g., 0) is prohibited.*

```
Let E be a content event with payload containing section_path P. Let S be the section table after applying all section events. Then Apply(E).section_id = lookup(S, P).id. The function lookup: SectionPath -> SectionID is total over the domain of section paths present in prior section events.
```

Violation scenario: sqlApplier.InsertInvariant sets section_id=0 for all invariants. Coverage analysis queries invariants by section — returns empty. project command reconstructs modules without section structure. Validation Check 5 cannot verify invariants are in correct sections.

Validation: After fold: SELECT COUNT(*) FROM invariants WHERE section_id = 0 MUST return 0. For each invariant with a section_path in its event payload, verify section_id points to the correct section row. Run ddis coverage on materialized DB — verify per-section completeness is computable.

// WHY THIS MATTERS: Section hierarchy is the structural backbone of DDIS specs (§0.1 State Space). Without it, the materialized state is a flat bag of elements — losing the tree structure that enables scoped queries, module-section relationships, and structural validation. This breaks the round-trip guarantee (APP-INV-096).

---

**APP-INV-101: Structural Diff Composite Key Completeness**

*The StructuralDiff function MUST use composite keys that include ALL discriminant fields when building comparison maps. For cross-references, the key MUST include (ref_type, ref_target, ref_text). Omitting any discriminant field from the key can cause silent collision and data loss in the diff output.*

```
For any table T with natural key K = (k1, k2, ..., kn), the diff map key MUST be the full tuple K. For cross_references, K = (ref_type, ref_target, ref_text). The map function m: Row -> Key is injective iff K contains all discriminant columns. If m is not injective, |image(m)| < |domain(m)| and the diff loses rows.
```

Violation scenario: diffCrossRefs uses key = target|text, omitting ref_type. Two cross-refs (type=invariant, target=APP-INV-071, text=See INV) and (type=app_invariant, target=APP-INV-071, text=See INV) collide. The second overwrites the first in the map. StateHash computed on the diff is wrong.

Validation: Insert two cross-refs with same target+text but different ref_type into DB1. Insert only one into DB2. Run StructuralDiff. Verify BOTH additions are reported, not just one.

// WHY THIS MATTERS: StructuralDiff is the foundation for StateHash (APP-INV-093), snapshot verification, and the event-sourcing integrity chain. A lossy diff means snapshots can verify as correct when they actually diverge.

---

### APP-ADR-076: Event Schema Carries Section Path for Hierarchy Reconstruction

#### Problem
The event fold applier (sqlApplier) hardcodes section_id=0 for all content elements because events do not carry section path information. This destroys the section hierarchy that is fundamental to DDIS spec structure, making coverage analysis, section-scoped queries, and structural validation impossible on materialized state.

#### Options
Option A: Add section_path to content event payloads. During fold, look up or create the section row and use its ID. Events become self-contained.
Option B: Run a post-fold reconciliation pass that matches elements to sections by line number or name heuristics. Fragile and lossy.
Option C: Store section events separately and reconstruct the tree before applying content events. Requires event ordering guarantees.
Option D: Accept section_id=0 and disable section-dependent features for materialized state. Violates round-trip (APP-INV-096).

#### Decision
**Option A: Enrich content event payloads with a section_path field.** The applier looks up the section by path (creating it if needed via a synthetic section event). This is the correct approach because it makes events self-describing and enables correct fold without external state. // WHY NOT B: Heuristic-dependent. C: Requires strict ordering. D: Unacceptable.

#### Consequences
Events become self-describing. Fold produces structurally complete state. Round-trip guarantee restored.

#### Tests
TestEventApplier_SectionHierarchy, TestFold_SectionLookup in internal/materialize/fold_test.go.

---

## Chapter 12: Cleanroom Audit Round 2 — Materialization Completeness

### §ES.12.1 Module Relationship Materialization

**APP-INV-108: Module relationship materialization completeness**

*The materialize fold applier must populate the module_relationships table from ModulePayload relationship arrays (maintains, interfaces, implements, adjacent), preserving the module ownership graph for projector filtering.*

```
forall m in ModulePayload:
  |m.Maintains| + |m.Interfaces| + |m.Implements| + |m.Adjacent|
  == |INSERT INTO module_relationships WHERE module_id = m.id|
```

Violation scenario: InsertModule receives ModulePayload with relationship arrays but only stores module_name and domain. The module_relationships table stays empty, breaking projector module filtering and APP-INV-087.

Validation: Test: emit module_registered event with maintains=[INV-001], materialize, verify module_relationships row exists with rel_type=maintains and target=INV-001.

// WHY THIS MATTERS: Module relationships are the structural backbone of the spec graph. Without them, cascade analysis, implementation order, and projector filtering all degrade to empty results.

---

### §ES.12.2 Snapshot-Accelerated Fold CLI Integration

### APP-ADR-078: Snapshot-accelerated fold CLI integration

#### Problem
The --from-snapshot flag exists on the materialize command but is never read. FoldFrom() exists but is unreachable from CLI. Users cannot benefit from snapshot optimization.

#### Options
A) Wire --from-snapshot to FoldFrom in runMaterializeInternal. B) Remove the flag and snapshots entirely. C) Auto-detect snapshots without flag.

#### Decision
**Option A: Wire flag to FoldFrom.** Wire --from-snapshot to FoldFrom. When set, load latest snapshot, verify its state hash, then call FoldFrom with the snapshot position. If verification fails, fall back to full replay (APP-INV-095 graceful degradation).

#### Consequences
Users gain incremental materialization for large event streams. Graceful degradation via snapshot verification ensures correctness.

#### Tests
1. Create snapshot at position N, add events, materialize --from-snapshot, verify only events after N are processed. 2. Corrupt snapshot hash, verify graceful fallback to full replay.

---

## Chapter 13: Cleanroom Audit Round 3 — ADR Options Round-Trip Fidelity

The event-sourcing pipeline must preserve ADR options across the full round-trip: parse → store → import → event → materialize → project. The parser correctly extracts options into the `adr_options` table during the initial parse phase. However, the import command does not query this table, the event payload carries an empty string for options, and the materializer does not reconstitute options from the payload. This chapter specifies the missing links.

**APP-INV-110: ADR Options Round-Trip Fidelity**

*ADR options stored in the adr_options table during parse must survive the complete event-sourcing round-trip: import, event emission, materialize, project, rendered markdown. No option label, name, pros, cons, is_chosen, or why_not field may be lost during the cycle.*

```
forall ADR A with options O = {o_1, ..., o_n} in parsed DB:
  let E = import(A).payload.Options
  let O' = materialize(E).adr_options
  |O| = |O'|
  forall o_i in O: exists o'_j in O' where
    o_i.label = o'_j.label AND o_i.name = o'_j.name AND o_i.is_chosen = o'_j.is_chosen
```

Violation scenario: Parse a spec with ADR containing 3 options (A, B, C) with pros/cons. Export via `ddis import`, materialize the resulting events, query `adr_options` — the table is empty. Options are silently dropped during import because the query does not JOIN on `adr_options`.

Validation method: Round-trip test: parse spec with multi-option ADR → import to events → materialize events → count rows in `adr_options`. Assert `COUNT(adr_options WHERE adr_id = A) >= 3`.

Why this matters: ADR options are the deliberative record of the specification process. Losing them breaks the bilateral discourse contract — the spec no longer captures WHY alternatives were rejected, only WHAT was chosen. This degrades spec quality on every round-trip through the event pipeline. Related: APP-INV-078 (Import Equivalence), APP-INV-085 (Import Content Completeness), APP-INV-087 (Materialization Structural Fidelity).

---

### APP-ADR-079: ADR Options Serialization in Event Payloads

#### Problem
The `ADRPayload.Options` field is a single string but the parser stores options as normalized rows in `adr_options` (label, name, pros, cons, is_chosen, why_not). The import command does not query `adr_options`, so the Options field is always empty. The materializer does not parse the Options field back into rows. Round-trip fidelity for ADR options is zero.

#### Options
A) Serialize options as structured JSON in the Options string field; deserialize on materialize. B) Add a new `TypeADROptionAdded` event type for each individual option. C) Serialize as markdown text matching the parser's input format, so the materializer can re-parse.

#### Decision
**Option A: Structured JSON serialization.** Serialize the options array as a JSON array within the ADRPayload.Options string field. On import, query `adr_options` joined to `adrs`, serialize each option as `{"label":"A","name":"Go","pros":"...","cons":"...","is_chosen":true,"why_not":"..."}`. On materialize, deserialize the JSON array and call `InsertADROption()` for each entry. This preserves all fields without adding new event types or relying on markdown re-parsing.

#### Consequences
JSON serialization is lossless and does not require new event types, keeping the event schema stable. The materializer gains a new code path in the ADR applier to parse and store options. Existing events with empty Options strings produce no options (backward compatible). New events carry full option structure. The query and projector paths can also use `GetADROptions()` for enrichment.

#### Tests
1. Parse spec with 3-option ADR, import to events, verify Options field is non-empty JSON array. 2. Materialize those events, verify `adr_options` table has 3 rows with correct labels. 3. Import ADR with no options, verify empty Options field, materialize succeeds with no options.

---
