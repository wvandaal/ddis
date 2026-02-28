---
module: event-sourcing
domain: eventsourcing
maintains: [APP-INV-071, APP-INV-072, APP-INV-073, APP-INV-074, APP-INV-075, APP-INV-076, APP-INV-077, APP-INV-078, APP-INV-079, APP-INV-080, APP-INV-081, APP-INV-082, APP-INV-083, APP-INV-084]
interfaces: [APP-INV-001, APP-INV-002, APP-INV-010, APP-INV-015, APP-INV-016, APP-INV-020, APP-INV-025, APP-INV-048, APP-INV-053]
implements: [APP-ADR-058, APP-ADR-059, APP-ADR-060, APP-ADR-061, APP-ADR-062, APP-ADR-063, APP-ADR-064, APP-ADR-065]
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

Extended validation for content-bearing events:
1. All existing rules (ID, Timestamp, Type, Stream correspondence) remain
2. Content events must have non-empty Payload with all required fields
3. If Causes is non-empty, all referenced IDs must exist in prior events
4. Version must be a positive integer (current version = 1)
5. Content events must not duplicate an existing element ID (except for update/supersede types)

---

## Chapter 2: Fold/Materialize Engine

### The Apply Function

The core of the materialization pipeline is the `apply` function, a pure function that maps (state, event) to a new state:

```
apply : (SQLiteState, Event) -> SQLiteState

apply(state, e) = case e.Type of
  "spec_section_defined"    -> insertSection(state, e.Payload)
  "spec_section_updated"    -> updateSection(state, e.Payload)
  "spec_section_removed"    -> removeSection(state, e.Payload)
  "invariant_crystallized"  -> insertInvariant(state, e.Payload)
  "invariant_updated"       -> updateInvariant(state, e.Payload)
  "invariant_removed"       -> removeInvariant(state, e.Payload)
  "adr_crystallized"        -> insertADR(state, e.Payload)
  "adr_updated"             -> updateADR(state, e.Payload)
  "adr_superseded"          -> supersedeADR(state, e.Payload)
  "negative_spec_added"     -> insertNegativeSpec(state, e.Payload)
  "quality_gate_defined"    -> insertGate(state, e.Payload)
  "cross_ref_added"         -> insertCrossRef(state, e.Payload)
  "glossary_term_defined"   -> insertGlossaryTerm(state, e.Payload)
  "module_registered"       -> insertModule(state, e.Payload)
  "manifest_updated"        -> updateManifest(state, e.Payload)
  "witness_recorded"        -> insertWitness(state, e.Payload)
  "witness_revoked"         -> revokeWitness(state, e.Payload)
  "witness_invalidated"     -> invalidateWitness(state, e.Payload)
  "challenge_completed"     -> insertChallenge(state, e.Payload)
  "snapshot_created"        -> recordSnapshot(state, e.Payload)
  _                         -> state  // Unknown types are no-ops (forward compat)
```

Each case function is a pure SQL mutation: INSERT, UPDATE, or DELETE on the appropriate tables. No file I/O, no system clock, no randomness.

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

### Causal Sort

Before applying events, the materialize engine sorts them into a valid causal order. Events without causal references preserve their log order. Events with causal references are topologically sorted.

```
causal_sort(events) -> sorted_events

1. Build dependency graph G from causes references
2. Verify G is acyclic (error if cycle detected)
3. Topological sort G
4. Within each topological level, sort by timestamp (stable)
5. Return sorted events
```

### Error Handling

The materialize engine collects errors without halting: unknown types are skipped (forward compat), missing causal references and schema violations are reported, and the fold continues processing valid events. All errors are reported after the full fold completes, producing a best-effort state.

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
