---
module: lifecycle-ops
domain: lifecycle
maintains: [APP-INV-006, APP-INV-010, APP-INV-013, APP-INV-016, APP-INV-041, APP-INV-050, APP-INV-051, APP-INV-052, APP-INV-053, APP-INV-059, APP-INV-062, APP-INV-103, APP-INV-105, APP-INV-106, APP-INV-107]
interfaces: [APP-INV-001, APP-INV-002, APP-INV-003, APP-INV-007, APP-INV-008, APP-INV-009, APP-INV-011, APP-INV-012, APP-INV-015]
implements: [APP-ADR-007, APP-ADR-008, APP-ADR-011, APP-ADR-030, APP-ADR-037, APP-ADR-039, APP-ADR-041, APP-ADR-046, APP-ADR-050, APP-ADR-052, APP-ADR-075, APP-ADR-077]
adjacent: [parse-pipeline, search-intelligence, query-validation]
negative_specs:
  - "Must NOT modify or delete existing oplog records"
  - "Must NOT allow transaction state transitions outside the defined state machine"
  - "Must NOT claim spec-to-code traceability without mechanical verification"
---

# Lifecycle Operations Module

This module owns the transactional mutation, operation logging, impact analysis termination, and implementation traceability subsystems of the DDIS CLI. It governs how spec modifications flow through a strict transaction state machine, how every operation is durably recorded in an append-only JSONL oplog, how BFS impact traversals terminate safely in the presence of cycles, and how spec-to-code annotations are mechanically verified against the source tree.

The architectural principle: **lifecycle operations are write-once, state-machine-governed, and mechanically auditable.** Transactions follow a two-terminal state machine with no backward transitions. The oplog is an immutable audit trail --- records are appended but never modified or deleted. Impact analysis terminates in bounded time regardless of graph topology. Implementation Trace annotations are verified by scanning the actual source tree, not by trusting the spec author.

**Invariants interfaced from other modules (cross-module reference completeness --- restated at point of use):**

- APP-INV-001: Round-Trip Fidelity --- parse then render produces byte-identical output (maintained by parse-pipeline). *Transactional edits depend on the round-trip guarantee; a commit that silently corrupts whitespace during re-render produces a dirty working tree after rollback.*
- APP-INV-002: Validation Determinism --- results independent of clock, RNG, execution order (maintained by query-validation). *Validation records in the oplog must be reproducible; if re-running validation on the same spec produces different results, the oplog's historical records are untrustworthy.*
- APP-INV-003: Cross-Reference Integrity --- every resolved reference points to an existing element (maintained by query-validation). *Impact analysis traverses the cross-reference graph; dangling references produce phantom nodes that inflate the impact radius and mislead editors.*
- APP-INV-007: Structural Diff Completeness --- diff reports every change, no silent drops (maintained by query-validation). *Diff records in the oplog consume the structural diff engine; a silent drop means the oplog understates the scope of a change.*
- APP-INV-008: RRF Fusion Correctness --- score equals correctly computed weighted sum (maintained by search-intelligence). *Search scores may appear in context bundles logged alongside transactions; incorrect scores in the oplog mislead future audits.*
- APP-INV-009: Monolith-Modular Equivalence --- parsing a monolith produces the same index as parsing assembled modules (maintained by parse-pipeline). *The seed command validates both formats; equivalence ensures the genesis record is meaningful regardless of input format.*
- APP-INV-011: Validation Composability --- checks are independent and composable (maintained by query-validation). *Selective validation during transactions (running only affected checks) depends on check independence.*
- APP-INV-012: LSI Dimension Bound --- k never exceeds document count (maintained by search-intelligence). *LSI dimensions must remain stable across index updates triggered by transactions.*
- APP-INV-015: Deterministic Hashing --- SHA-256 with no salt produces identical hash for identical input (maintained by parse-pipeline). *Content hashes in oplog validate and diff records must be reproducible; non-deterministic hashing makes historical comparisons meaningless.*

---

## Invariants

This module maintains five invariants. Each invariant is fully specified with all six components: plain-language statement, semi-formal expression, violation scenario, validation method, WHY THIS MATTERS annotation, and implementation trace.

---

**APP-INV-006: Transaction State Machine**

*The transaction lifecycle follows a strict three-state machine: only `pending -> committed` and `pending -> rolled_back` transitions are permitted. A committed or rolled-back transaction is immutable --- no further state changes are possible.*

```
FOR ALL tx IN transactions:
  tx.status IN {pending, committed, rolled_back}
  AND (tx.status = committed  IMPLIES prev(tx.status) = pending)
  AND (tx.status = rolled_back IMPLIES prev(tx.status) = pending)
  AND (tx.status IN {committed, rolled_back} IMPLIES immutable(tx))

WHERE:
  The SQL schema enforces: CHECK(status IN ('pending', 'committed', 'rolled_back'))
  CommitTransaction: UPDATE ... WHERE tx_id = ? AND status = 'pending'
  RollbackTransaction: UPDATE ... WHERE tx_id = ? AND status = 'pending'
  RowsAffected = 0 triggers error "transaction not found or not pending"
```

Violation scenario: A bug in the rollback handler omits the `AND status = 'pending'` predicate from the UPDATE WHERE clause. A developer calls `ddis tx rollback` on a transaction that was already committed. The UPDATE succeeds (the row exists), transitioning the transaction from `committed` to `rolled_back`. An auditor reviewing the oplog sees a transaction that was both committed and then rolled back, destroying the audit trail's integrity. The developer now has no way to know whether the spec changes from that transaction are live or reverted.

Validation: Attempt every possible state transition across the six-element product set `{pending, committed, rolled_back} x {pending, committed, rolled_back}`:

| From | To | Expected |
|---|---|---|
| pending | committed | ACCEPT |
| pending | rolled_back | ACCEPT |
| committed | pending | REJECT (RowsAffected = 0) |
| committed | rolled_back | REJECT (RowsAffected = 0) |
| rolled_back | pending | REJECT (RowsAffected = 0) |
| rolled_back | committed | REJECT (RowsAffected = 0) |

For each rejected transition, verify that the database row remains unchanged and the function returns an error. Additionally, verify that the SQL CHECK constraint rejects any direct UPDATE that bypasses the function (e.g., `UPDATE transactions SET status = 'invalid' WHERE tx_id = ?` must fail with a constraint violation).

// WHY THIS MATTERS: The transaction state machine is the concurrency control mechanism for spec modifications. Invalid transitions corrupt the operation log and make it impossible to determine which version of a spec element is authoritative. In the RALPH improvement loop, each phase (audit, apply, judge) opens and closes transactions --- if a committed phase could be rolled back, the loop cannot trust its own history.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/storage/queries.go::CreateTransaction`
- Source: `internal/storage/queries.go::CommitTransaction`
- Source: `internal/storage/queries.go::RollbackTransaction`
- Source: `internal/storage/queries.go::GetTransaction`
- Source: `internal/storage/queries.go::ListTransactions`
- Source: `internal/storage/schema.go::SchemaSQL` (CHECK constraint on status column)
- Tests: `tests/tx_test.go::TestTxBeginCommit`
- Tests: `tests/tx_test.go::TestTxRollback`
- Tests: `tests/tx_test.go::TestTxOperations`
- Tests: `tests/tx_test.go::TestTxList`
- Tests: `tests/tx_test.go::TestTxFlushToOplog`
- Validates-via: `internal/storage/queries.go::CommitTransaction` (WHERE status='pending' guard)

----

**APP-INV-010: Oplog Append-Only**

*The operation log (oplog) is strictly append-only. Once a record is written, it is never modified or deleted. The record count is monotonically non-decreasing. The file is opened exclusively with O_APPEND|O_CREATE|O_WRONLY --- no seek, truncate, or in-place overwrite is possible through the API.*

```
FOR ALL record r IN oplog, FOR ALL times t1 < t2:
  read(oplog, r.offset, t1) = read(oplog, r.offset, t2)
  AND count(oplog, t2) >= count(oplog, t1)
  AND open_flags(oplog) = O_APPEND | O_CREATE | O_WRONLY
  AND NOT EXISTS function f IN oplog_package:
    f.semantics IN {truncate, seek_write, delete, update_in_place}
```

Violation scenario: A performance optimization adds a "compact" command that removes duplicate validation records from the oplog to save disk space. The command opens the file with O_RDWR, reads all records, filters duplicates, truncates the file, and rewrites the deduplicated set. A RALPH audit agent that previously observed a validation failure at timestamp T1 queries the oplog for records since T1 and finds the failure record missing. The agent concludes the spec has always been clean and skips re-auditing the section --- a silent regression in spec quality.

Validation: (1) Append N records to the oplog via `oplog.Append`. Read all records via `oplog.ReadAll`. Verify count = N and each record's content matches what was written. (2) Inspect the `Append` function source to confirm the `os.OpenFile` call uses exactly `os.O_APPEND|os.O_CREATE|os.O_WRONLY` with mode `0o644`. (3) Verify that no function in the `oplog` package opens the file with `O_RDWR`, `O_TRUNC`, or calls `Seek`, `Truncate`, or `WriteAt`. (4) Verify that `ReadAll` and `ReadFiltered` open the file with `os.Open` (read-only). (5) Append records from two goroutines concurrently; verify no interleaved partial lines (atomic append guarantee from O_APPEND on POSIX for writes under PIPE_BUF).

// WHY THIS MATTERS: The oplog is the audit trail for all spec mutations. If records can be silently modified or deleted, the RALPH loop cannot reliably compare versions or detect regressions. Append-only guarantees historical completeness --- every validation result, every diff, every transaction boundary is permanently recorded. This is the foundation of the "structured intent over formal derivation" philosophy (APP-ADR-011): mechanical verification replaces trust.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/oplog/oplog.go::Append` (os.OpenFile with O_APPEND|O_CREATE|O_WRONLY)
- Source: `internal/oplog/oplog.go::ReadAll`
- Source: `internal/oplog/oplog.go::ReadFiltered`
- Source: `internal/oplog/oplog.go::HasGenesisTransaction`
- Source: `internal/oplog/oplog.go::RenderLog`
- Tests: `tests/oplog_test.go::TestOplogAppend`
- Tests: `tests/oplog_test.go::TestOplogFilter`
- Tests: `tests/oplog_test.go::TestOplogDiffRecord`
- Tests: `tests/oplog_test.go::TestOplogValidateRecord`
- Tests: `tests/oplog_test.go::TestOplogTxRecord`
- Tests: `tests/oplog_test.go::TestOplogEmpty`
- Tests: `tests/oplog_test.go::TestOplogRenderJSON`
- Validates-via: Source inspection of `os.OpenFile` flags in `Append`

----

**APP-INV-013: Impact Termination**

*The BFS impact analysis algorithm visits each node at most once and terminates within bounded time, even in the presence of cyclic cross-references. The visited set grows monotonically; its cardinality never exceeds the total number of elements in the spec index. Traversal depth never exceeds the configured maximum (default 2, hard ceiling 5).*

```
LET visited = {} (empty set)
FOR ALL step IN BFS(start_node, direction, max_depth):
  step.node NOT IN visited                    // no revisits
  AND visited' = visited UNION {step.node}    // monotonic growth
  AND |visited| <= |all_nodes_in_spec|        // bounded cardinality
  AND step.depth <= max_depth                 // depth-bounded
  AND max_depth <= 5                          // hard ceiling

WHERE:
  BFS = bfsForward (backlinks traversal) OR bfsBackward (outgoing refs traversal)
  visited is initialized with {start_node}
  For direction="both": forward runs first, backward inherits forward's visited set
```

Violation scenario: Two sections cross-reference each other: section A references section B, and section B references section A. A user runs `ddis impact A --depth 10`. Without the visited-set guard, `bfsForward` follows A's backlinks to B, then B's backlinks to A, then A's backlinks to B, ad infinitum. The queue grows without bound, consuming memory until the process is killed by the OOM killer. The `ddis impact` command hangs indefinitely on any spec with mutual cross-references --- which describes most real specifications.

Validation: (1) Create a spec with a deliberate cycle: two sections where each references the other. Run `ddis impact` on one with `--depth 10`. The command must terminate and produce a result with exactly 1 node (the other section) at distance 1. (2) Create a 3-node cycle (A->B->C->A). Run forward impact from A with depth 3. Verify the result contains B (distance 1) and C (distance 2) but does not contain A again. (3) Verify that `ImpactResult.TotalCount <= total_elements_in_spec`. (4) Verify that `MaxDepth` is clamped to 5 regardless of the `--depth` flag value. (5) Verify that direction="both" does not double-count nodes (the backward pass inherits the forward pass's visited set, augmented with forward results).

// WHY THIS MATTERS: Cross-reference cycles are common in specifications --- mutual dependencies between subsystems, invariants that reference each other's violation scenarios, ADRs that cite competing alternatives. Impact analysis must handle these gracefully. An infinite loop does not just waste resources; it makes the `impact` and `context` commands unusable on real-world specs, since context bundles include impact radius as signal 8.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/impact/impact.go::Analyze` (MaxDepth clamping, visited initialization, direction dispatch)
- Source: `internal/impact/impact.go::bfsForward` (visited-set guard, depth check)
- Source: `internal/impact/impact.go::bfsBackward` (visited-set guard, depth check)
- Source: `internal/impact/impact.go::resolveSourceID`
- Source: `internal/impact/impact.go::resolveSectionID`
- Source: `internal/impact/impact.go::elementExists`
- Tests: `tests/impact_test.go::TestImpactForward`
- Tests: `tests/impact_test.go::TestImpactBackward`
- Tests: `tests/impact_test.go::TestImpactDepthLimit`
- Validates-via: Assertion `visited[sourceID]` check before enqueue in `bfsForward`

----

**APP-INV-016: Implementation Traceability**

*Every invariant that carries Implementation Trace annotations (Source, Tests, Validates-via) must reference files that exist on disk and functions that can be mechanically located in the source tree by regex matching. Broken references --- files that do not exist or functions that cannot be found --- are reported as errors, not silently accepted.*

```
FOR ALL inv IN invariants, FOR ALL annotation IN inv.implementation_trace:
  LET abs_path = resolve(code_root, annotation.file_path)
  THEN:
    file_exists(abs_path)
    AND (
      regex_match("func\s+(\([^)]+\)\s+)?" + annotation.func_name + "\(", abs_path)
      OR regex_match("type\s+" + annotation.func_name + "\s+", abs_path)
    )

WHERE:
  annotation = parsed from lines matching:
    /^\s*-\s*(Source|Tests|Validates-via):\s*`([^`]+)::(\w+)`/
  resolve(root, path) = filepath.Join(root, path) if not absolute
  For Tests kind: auto-prefix "Test" if func_name does not start with "Test"
```

Violation scenario: APP-INV-008 in the search-intelligence module claims `Source: internal/search/engine.go::FusionSearch`. During a refactor, the function is renamed to `Search` and the file is split into `engine.go` and `ranker.go`. The annotation is now a dead reference --- the spec claims traceability to code that no longer exists under that name or in that file. Without mechanical verification, the dead reference persists indefinitely, creating false confidence that the invariant is implemented. Check 13 detects this drift: it reports "APP-INV-008 Source annotation: function FusionSearch not found in internal/search/engine.go" as an error.

Validation: (1) Create a spec with an invariant that has valid Implementation Trace annotations pointing to existing files and functions. Run Check 13 (`--code-root` set). Verify all annotations are reported as "OK" (severity: info). (2) Rename one referenced function. Re-run Check 13. Verify the broken annotation is reported as an error with the message "function X not found in Y". (3) Delete a referenced file. Re-run Check 13. Verify the missing file is reported as "file not found". (4) Add a `Tests:` annotation without the `Test` prefix (e.g., `Tests: foo_test.go::TxBeginCommit`). Verify Check 13 auto-prepends `Test` and looks for `TestTxBeginCommit`. (5) Verify that invariants without any Implementation Trace block are skipped (no error, informational message only). (6) Verify the summary line format: "N annotations: M valid, P broken (Q invariants with broken refs)".

// WHY THIS MATTERS: Implementation Trace annotations are the bridge between specification claims and executable code. They are the mechanism by which APP-ADR-011 (Structured Intent over Formal Derivation) achieves trustworthiness without formal proofs. Dead references create false confidence --- the spec appears traced but the link is broken. Without mechanical verification, annotations rot silently as the codebase evolves. Check 13 is the enforcement mechanism that keeps the bridge intact.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/validator/traceability.go::checkImplementationTraceability`
- Source: `internal/validator/traceability.go::parseTraceAnnotations`
- Source: `internal/validator/traceability.go::funcExistsInFile`
- Tests: `tests/traceability_test.go::TestTraceabilitySkippedWithoutCodeRoot`
- Tests: `tests/traceability_test.go::TestTraceabilityValidAnnotation`
- Tests: `tests/traceability_test.go::TestTraceabilityBrokenFile`
- Tests: `tests/traceability_test.go::TestTraceabilityBrokenFunction`
- Tests: `tests/traceability_test.go::TestTraceabilityMethodReceiver`
- Tests: `tests/traceability_test.go::TestTraceabilityNoAnnotations`
- Validates-via: `internal/validator/validator.go::Validate` (orchestrates all checks including Check 13)

----

**APP-INV-041: Witness Auto-Invalidation**

*When a spec is re-parsed and an invariant's content_hash changes, any witness whose spec_hash no longer matches the invariant's content_hash MUST be automatically set to `stale_spec` status.*

```
FOR ALL w IN invariant_witnesses WHERE w.status = 'valid':
  EXISTS inv IN invariants
    WHERE inv.invariant_id = w.invariant_id
    AND inv.content_hash = w.spec_hash
```

Violation scenario: An invariant is modified and re-parsed, but its witness still shows `status = 'valid'` with the old hash, giving a false sense of implementation coverage.

Validation: Record a witness for an invariant, modify the invariant's text (changing its content_hash), re-parse the spec, then verify the witness status has changed to `stale_spec`.

// WHY THIS MATTERS: Without auto-invalidation, witnesses become stale silently. A team could believe invariants are implemented when the spec has evolved past the witnessed version, creating a dangerous false confidence in spec-implementation alignment.

**Implementation Trace:**
- Source: `internal/storage/insert.go::InvalidateWitnesses`
- Source: `internal/witness/witness.go::Record`
- Source: `internal/witness/witness.go::Refresh`
- Source: `internal/cli/parse.go::runParse`
- Tests: `internal/witness/witness_test.go::TestCheck_StalenessDetection`
- Tests: `internal/witness/witness_test.go::TestInvalidateWitnesses`

----

## Architecture Decision Records

---

### APP-ADR-007: JSONL Oplog Format

#### Problem

The operation log needs a durable, human-readable, append-friendly storage format that survives database recreation, works with standard Unix tools, and guarantees atomic writes. How should oplog records be persisted?

#### Options

A) **SQLite table** --- Store oplog records in a dedicated table within the spec index database.
- Pros: Transactional consistency with spec data. SQL queries for filtering. Already have a database open.
- Cons: The oplog must survive database recreation (re-parse destroys and rebuilds the SQLite file). Coupling oplog durability to the index database means a failed parse can lose the audit trail. Cannot be inspected without SQLite tooling.

B) **JSONL (newline-delimited JSON)** --- One JSON record per line in a flat file.
- Pros: Append-only via `O_APPEND` (atomic on POSIX for writes under PIPE_BUF, ~4KB --- each record is well under this). Human-readable with `cat`, `grep`, `jq`. Survives database recreation. Trivially parseable. Each line is self-contained --- no framing protocol.
- Cons: No query optimization (linear scan). No built-in compression. Large oplogs may be slow to read in full.

C) **Protocol Buffers with length-prefix framing** --- Binary records with size headers.
- Pros: Compact. Schema-versioned.
- Cons: Not human-readable. Requires protobuf tooling to inspect. Binary append is not atomic (must write length prefix and payload in two writes, creating a torn-write risk on crash).

D) **Structured log (e.g., JSON Lines with external rotation)** --- JSONL with logrotate-style management.
- Pros: Scalable to very large oplogs.
- Cons: Rotation splits the audit trail across files, complicating queries that span rotation boundaries. Unnecessary for specification-scale data (thousands of records, not millions).

#### Decision

**Option B: JSONL.** Each oplog record is a single JSON line with a fixed envelope: `{version, type, timestamp, tx_id, data}`. The file is opened with `os.OpenFile(path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o644)`. The `json.Encoder` writes one line per `Encode` call (Go's `json.Encoder` appends a newline after each JSON object). The default path is `.ddis/oplog.jsonl` alongside the spec.

// WHY NOT Option A (SQLite)? The oplog must survive `ddis parse`, which drops and recreates tables. Storing the oplog in the same database means a parse failure can destroy the audit trail. The oplog's value comes from its independence from the index.

// WHY NOT Option C (protobuf)? Human readability is a first-class requirement. RALPH audit agents and human operators both inspect the oplog. Binary formats require tooling that may not be available in every environment.

// WHY NOT Option D (rotated logs)? Specification oplogs grow at ~3 records per RALPH iteration (begin + validate + commit). Even after 1,000 iterations, the file is ~500KB. Rotation adds complexity with no benefit at this scale.

#### Consequences

- Append is atomic for single-record writes under PIPE_BUF (4KB on Linux). Multi-record writes (e.g., genesis seed writes 3 records) use a single `Append` call with one file open, minimizing the torn-write window.
- Each record is self-contained: `{version: 1, type: "transaction"|"validate"|"diff", timestamp: RFC3339, tx_id: "tx-...", data: {...}}`.
- Three record types cover all lifecycle events: `transaction` (begin/commit/rollback), `validate` (check results), `diff` (structural changes).
- `ReadAll` and `ReadFiltered` use `bufio.Scanner` with a 10MB max line buffer. Empty lines are skipped. Non-existent files return nil (not error) for idempotent reads.
- Standard Unix tool compatibility: `cat oplog.jsonl | jq '.type'` filters by type; `grep '"tx_id":"tx-abc"' oplog.jsonl` finds transaction records; `wc -l oplog.jsonl` counts records.

#### Tests

- (Validated by APP-INV-010) Append followed by ReadAll produces identical records.
- `tests/oplog_test.go::TestOplogAppend` verifies basic write-read cycle.
- `tests/oplog_test.go::TestOplogFilter` verifies type, tx_id, since, and limit filtering.
- `tests/oplog_test.go::TestOplogEmpty` verifies non-existent file returns nil, not error.
- `tests/oplog_test.go::TestOplogRenderJSON` verifies JSON output fidelity.

---

### APP-ADR-008: Surgical Edit Strategy

#### Problem

Spec modifications need to update the minimum set of elements while maintaining referential integrity across the index. How should the CLI apply changes to a parsed specification?

#### Options

A) **Full re-parse on every change** --- After any modification, re-parse the entire specification from scratch.
- Pros: Simple. Guarantees a consistent index after every edit. No incremental logic to maintain.
- Cons: Re-parsing a 3,000-line spec takes 200-500ms. In a RALPH loop applying 10 changes per iteration, that is 2-5 seconds of re-parsing. Re-parsing also destroys the transaction history in the index database (tables are dropped and recreated), requiring the oplog to be the sole source of truth.

B) **Surgical element-level edits** --- Target individual elements by ID, update their content in the database, recompute affected hashes, and propagate changes through the impact graph. Full re-parse is reserved for structural changes.
- Pros: Fast (microseconds per element update vs. hundreds of milliseconds for re-parse). Preserves transaction history. Supports undo by rolling back individual operations. Impact analysis identifies cascading changes before they are applied.
- Cons: More complex. Must handle hash recomputation, cross-reference re-resolution, and FTS5 index updates. Structural changes (heading moves, section splits) still require re-parse.

C) **Patch-based editing** --- Apply changes as text patches (unified diff format) to the source files, then re-parse.
- Pros: Familiar format. Git-friendly.
- Cons: Patches are fragile --- context lines may not match after concurrent edits. Combining patches from multiple transactions requires merge logic. Still requires re-parse after applying the patch.

#### Decision

**Option B: Surgical element-level edits.** Modifications target individual elements by their database ID. The transaction system records each operation with its impact set (computed via `impact.Analyze`). Element-level updates use `UPDATE` SQL statements that modify content, recompute `content_hash = sha256Hex(new_content)`, and mark affected cross-references for re-resolution.

// WHY NOT Option A (full re-parse)? Re-parsing drops and recreates tables, destroying the transaction history and all search index state (FTS5, LSI vectors, authority scores). A RALPH iteration that applies 10 changes would rebuild the search index 10 times. Surgical edits update only what changed.

// WHY NOT Option C (patches)? Text patches require 3 lines of context around each hunk. Concurrent edits from different transactions can cause context mismatch. Patches also require re-parse after application, combining the worst aspects of Options A and C.

#### Consequences

- Transaction operations are recorded in `tx_operations` with ordinal, type, data (JSON), and impact set.
- `AddTxOperation(db, txID, ordinal, opType, opData, impactSet)` inserts each operation.
- Impact sets are computed by `impact.Analyze` before the edit is applied, identifying elements that may need cascading updates.
- Full re-parse is the fallback for structural mutations (heading level changes, section splits/merges). The CLI detects structural changes by comparing section trees before and after the edit.
- Hash recomputation uses the same `sha256Hex` function as the parser (APP-INV-015 interface), ensuring consistency.
- The oplog records both the operation (via transaction records) and its effect (via diff records computed after the edit).

#### Tests

- `tests/tx_test.go::TestTxOperations` verifies that operations are recorded with correct ordinals and data.
- `tests/tx_test.go::TestTxFlushToOplog` verifies that committed transactions produce oplog records.
- `tests/impact_test.go::TestImpactForward` verifies that impact sets correctly identify affected elements.

---

### APP-ADR-011: Structured Intent over Formal Derivation

#### Problem

The specification needs to communicate design rationale and invariant definitions to LLM agents in a way that is both precise enough for mechanical validation and accessible enough for agents without formal methods training. Should invariants be expressed in a formal specification language (TLA+, Alloy) or in structured natural language?

#### Options

A) **Formal specification language (TLA+, Alloy)** --- Express invariants as formal predicates with mathematical precision.
- Pros: Unambiguous. Model-checkable. Proven to catch subtle concurrency and state machine bugs.
- Cons: Requires TLA+/Alloy tooling (not available in all environments). LLM agents have unreliable TLA+ generation capabilities --- hallucinated temporal operators and incorrect liveness properties are common. Human contributors must learn a specialized language. Tooling dependency (TLC model checker) contradicts the single-binary design.

B) **Structured natural language with semi-formal predicates** --- Each invariant has a natural-language statement, a pseudo-code predicate readable without specialized tooling, a concrete violation scenario, a validation method, and a WHY THIS MATTERS annotation.
- Pros: LLM agents can read and act on definitions directly. Pseudo-code predicates are precise enough for test generation. Violation scenarios ground the invariant in concrete failure modes. Mechanical validation (Check 13, traceability) enforces the spec-to-code link without requiring formal proofs.
- Cons: Less precise than formal specifications. Cannot be model-checked. Relies on testing and manual review for correctness assurance.

C) **Purely natural language** --- Free-form prose describing each invariant.
- Pros: No learning curve. Fastest to write.
- Cons: Ambiguous. Different readers (and LLMs) interpret prose differently. No mechanical enforcement. Impossible to validate automatically.

#### Decision

**Option B: Structured natural language with semi-formal predicates.** Each invariant definition includes six components: (1) plain-language statement, (2) semi-formal predicate in pseudo-code, (3) concrete violation scenario, (4) validation method, (5) WHY THIS MATTERS annotation, and (6) Implementation Trace with Source/Tests/Validates-via paths.

The trade-off is explicit: mathematical rigor is sacrificed for accessibility and mechanical enforceability. Correctness assurance comes from three mechanisms, not formal proof:

1. **Testing** --- Every invariant has test references that exercise its claims.
2. **Check 13 (Implementation Traceability)** --- Mechanically verifies that Source/Tests/Validates-via annotations point to existing code (APP-INV-016).
3. **RALPH loop** --- The recursive improvement loop audits invariant completeness and detects regressions across iterations.

// WHY NOT Option A (formal methods)? LLM agents hallucinate TLA+ temporal operators at unacceptable rates. A specification intended for LLM consumption must use a representation that LLMs reliably interpret. Semi-formal predicates in pseudo-code hit the sweet spot: precise enough for test derivation, readable enough for LLM agents.

// WHY NOT Option C (pure prose)? "The transaction must be safe" is not actionable. An LLM reading this has no basis for generating tests, verifying compliance, or detecting violations. Structured components (especially the violation scenario and validation method) transform vague intent into concrete, testable claims.

#### Consequences

- Invariants use pseudo-code predicates (FOR ALL, EXISTS, IMPLIES, AND/OR/NOT), not TLA+ or Alloy.
- Check 13 (`checkImplementationTraceability`) mechanically verifies Implementation Trace annotations. This is the primary enforcement mechanism.
- Confidence levels track verification depth: `falsified` (tested and survived), `property-checked` (property test passed), `property-derived` (derived from mathematical property), `structurally-verified` (structural inspection confirmed).
- LLM agents can read invariant definitions directly and generate tests, edits, and audits without formal methods tooling.
- The violation scenario serves as a negative test case: if the scenario can occur, the invariant is violated.
- The WHY THIS MATTERS annotation prevents well-meaning simplifications that inadvertently remove a safety guarantee.

#### Tests

- (Validated by APP-INV-016) Every invariant with Implementation Trace has valid Source/Tests/Validates-via paths.
- `tests/traceability_test.go::TestTraceabilityValidAnnotation` verifies that valid annotations are accepted.
- `tests/traceability_test.go::TestTraceabilityBrokenFunction` verifies that stale annotations are detected.
- The six-component structure (statement, semi-formal, violation, validation, WHY, trace) is enforced by the query-validation module's invariant completeness check.

---

### APP-ADR-030: Persistent Witnesses over Ephemeral Done Flags

#### Problem

The `--done` flag on `ddis progress` is ephemeral --- passed per invocation, forgotten immediately. In multi-agent workflows, no agent knows the global done set. Progress tracking requires external coordination.

#### Options

A) **External done file** --- Store completed invariant IDs in a flat file alongside the spec.
- Pros: Simple, portable, no schema changes required.
- Cons: No auto-invalidation when spec changes, manual management burden, no evidence tracking.

B) **Persistent witness table** --- Store witnesses in an `invariant_witnesses` table with content_hash-based auto-invalidation.
- Pros: Auto-invalidation on spec change, queryable via SQL, per-invariant evidence with provenance, integrates with existing SQLite index.
- Cons: Additional schema table, slightly more complex than a flat file.

C) **Session state KV** --- Reuse the existing `session_state` key-value table to track done invariants.
- Pros: No new table needed, reuses existing infrastructure.
- Cons: No per-invariant granularity, no evidence tracking, no auto-invalidation, conflates session state with persistent proof receipts.

#### Decision

**Option B: Persistent witness table.** Witnesses persist in the `invariant_witnesses` table, auto-invalidate on spec change via content_hash comparison, and are loaded by `ddis progress` by default (via `--witness` flag, default true). The `--done` flag remains as an additive override.

// WHY NOT Option A (external done file)? Auto-invalidation is the core property (APP-INV-041). A flat file has no mechanism to detect that the spec changed underneath --- it would require external tooling to compare hashes, duplicating what the SQLite index already provides.

// WHY NOT Option C (session state KV)? Session state is per-session and ephemeral by design. Witnesses are persistent proof receipts that survive across sessions. Conflating these concerns violates the separation between transient state and durable evidence.

**Confidence:** Committed

#### Consequences

- `ddis progress` reads witnesses by default; the `--witness` flag (default true) loads valid witnesses into the done set before `--done` expansion, making them additive.
- Stale witnesses are drift signals: `ddis drift --report` includes stale witnesses in the drift count and quality breakdown.
- `ddis validate` Check 14 (Witness Freshness) reports stale witnesses as warnings, integrating witness health into the standard validation pipeline.
- The `ddis parse` command auto-invalidates stale witnesses on every re-parse, ensuring APP-INV-041 is mechanically enforced.

#### Tests

- `internal/witness/witness_test.go::TestProgressWithWitnesses` --- witnesses populate done set
- `internal/witness/witness_test.go::TestProgressWitnessAndDoneFlag` --- witnesses and --done are additive
- `internal/witness/witness_test.go::TestInvalidateWitnesses` --- stale witnesses detected on re-parse
- `internal/witness/witness_test.go::TestCheck_StalenessDetection` --- staleness detection and reporting

---

## Implementation

### Chapter: Transaction State Machine

**Preserves:** APP-INV-006 (Transaction State Machine --- only pending->committed and pending->rolled_back are valid).

**Interfaces:** APP-INV-015 (Deterministic Hashing --- content hashes in transaction records must be reproducible).

The transaction subsystem manages the lifecycle of spec modifications. Each transaction progresses through a strict state machine: it begins as `pending`, and terminates as either `committed` or `rolled_back`. No other transitions are possible.

#### Schema

The `transactions` table enforces the state machine at the database level:

```sql
CREATE TABLE IF NOT EXISTS transactions (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    tx_id TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL,
    status TEXT NOT NULL
      CHECK(status IN ('pending', 'committed', 'rolled_back')),
    created_at TEXT NOT NULL,
    committed_at TEXT,
    parent_tx_id TEXT REFERENCES transactions(tx_id)
);

CREATE TABLE IF NOT EXISTS tx_operations (
    id INTEGER PRIMARY KEY,
    tx_id TEXT NOT NULL REFERENCES transactions(tx_id),
    ordinal INTEGER NOT NULL,
    operation_type TEXT NOT NULL,
    operation_data TEXT NOT NULL,
    impact_set TEXT,
    applied_at TEXT
);
```

The `CHECK(status IN ('pending', 'committed', 'rolled_back'))` constraint is the first defense: any attempt to set status to an invalid value is rejected by SQLite. The second defense is the `WHERE status = 'pending'` predicate in both `CommitTransaction` and `RollbackTransaction`, which prevents transitions from terminal states.

#### Transaction ID Generation

`generateTxID` produces unique identifiers with the format `tx-` followed by 16 hexadecimal characters (8 bytes from `crypto/rand`):

```
Algorithm: Transaction ID Generation
Input: none
Output: string of format "tx-" + 16 hex chars

1. Allocate 8-byte buffer
2. Fill from crypto/rand.Read
3. Return "tx-" + hex.EncodeToString(buffer)
4. Fallback: return "tx-fallback" if crypto/rand fails (should never happen)
```

The result space is 2^64 (18.4 quintillion) possible IDs. At 1,000 transactions per day, the birthday collision probability does not reach 50% until ~4.8 billion transactions.

#### State Transition Functions

**CreateTransaction(db, specID, txID, description):**

```
INSERT INTO transactions (spec_id, tx_id, description, status, created_at)
VALUES (?, ?, ?, 'pending', datetime('now'))
```

Always inserts with `status = 'pending'`. The UNIQUE constraint on `tx_id` prevents duplicate transactions.

**CommitTransaction(db, txID):**

```
UPDATE transactions
SET status = 'committed', committed_at = datetime('now')
WHERE tx_id = ? AND status = 'pending'
```

The `AND status = 'pending'` predicate is the critical guard. If the transaction is already committed or rolled back, the UPDATE matches zero rows, `RowsAffected()` returns 0, and the function returns an error: "transaction %s not found or not pending".

**RollbackTransaction(db, txID):**

```
UPDATE transactions
SET status = 'rolled_back', committed_at = datetime('now')
WHERE tx_id = ? AND status = 'pending'
```

Identical guard logic to `CommitTransaction`. The `committed_at` column is reused for the rollback timestamp (it records when the terminal state was reached, regardless of which terminal state).

#### Transaction Operations

Each operation within a transaction is recorded in `tx_operations` with:

- `ordinal`: sequential order (0-based) within the transaction
- `operation_type`: string identifying the operation kind (e.g., "edit", "validate", "diff")
- `operation_data`: JSON blob with operation-specific payload
- `impact_set`: optional JSON blob listing affected element IDs (computed by `impact.Analyze`)
- `applied_at`: timestamp of execution

`AddTxOperation(db, txID, ordinal, opType, opData, impactSet)` inserts one row. Operations are retrieved by `GetTxOperations(db, txID)`, ordered by ordinal.

#### CLI Subcommands

The `tx` command exposes five actions:

| Action | Usage | Behavior |
|---|---|---|
| `begin` | `ddis tx begin <db> "description"` | Creates pending transaction, writes begin record to oplog, prints tx_id |
| `commit` | `ddis tx commit <tx_id> <db>` | Transitions to committed, writes commit record to oplog |
| `rollback` | `ddis tx rollback <tx_id> <db>` | Transitions to rolled_back, writes rollback record to oplog |
| `list` | `ddis tx list <db>` | Lists all transactions, ordered by creation time descending |
| `show` | `ddis tx show <tx_id> <db>` | Outputs JSON with transaction details and all operations |

Each mutating action (begin, commit, rollback) writes a corresponding oplog record in addition to updating the database. This dual-write ensures the oplog has a complete lifecycle record even if the database is later recreated.

**Implementation Trace:**
- Source: `internal/cli/tx.go::txBegin`
- Source: `internal/cli/tx.go::txCommit`
- Source: `internal/cli/tx.go::txRollback`
- Source: `internal/cli/tx.go::txList`
- Source: `internal/cli/tx.go::txShow`
- Source: `internal/cli/tx.go::generateTxID`
- Source: `internal/storage/queries.go::CreateTransaction`
- Source: `internal/storage/queries.go::CommitTransaction`
- Source: `internal/storage/queries.go::RollbackTransaction`
- Source: `internal/storage/queries.go::AddTxOperation`
- Source: `internal/storage/queries.go::GetTxOperations`
- Tests: `tests/tx_test.go::TestTxBeginCommit`
- Tests: `tests/tx_test.go::TestTxRollback`
- Tests: `tests/tx_test.go::TestTxOperations`
- Tests: `tests/tx_test.go::TestTxList`
- Tests: `tests/tx_test.go::TestTxFlushToOplog`

---

### Chapter: Oplog Schema

**Preserves:** APP-INV-010 (Oplog Append-Only --- records are written via O_APPEND and never modified).

**Interfaces:** APP-INV-007 (Diff Completeness --- diff records consume the structural diff engine), APP-INV-002 (Validation Determinism --- validate records capture deterministic check results).

The oplog stores every lifecycle event as a JSONL record with a fixed envelope schema and type-specific payload. Three record types cover all events: transactions (state machine transitions), validations (check results), and diffs (structural changes).

#### Record Envelope

Every JSONL line conforms to this envelope:

```json
{
  "version": 1,
  "type": "transaction" | "validate" | "diff",
  "timestamp": "2026-02-23T14:30:00Z",
  "tx_id": "tx-a1b2c3d4e5f67890",
  "data": { ... }
}
```

| Field | Type | Description |
|---|---|---|
| `version` | int | Schema version (currently 1). Enables future schema evolution without breaking readers. |
| `type` | string | Record type discriminator. One of: `transaction`, `validate`, `diff`. |
| `timestamp` | string | RFC3339 UTC timestamp from `time.Now().UTC().Format(time.RFC3339)`. |
| `tx_id` | string | Transaction ID that this record belongs to. Optional (omitted for standalone records). |
| `data` | object | Type-specific payload. Structure depends on `type`. |

The `data` field is stored as `json.RawMessage` in the Go struct, enabling type-safe decoding via `DecodeTx()`, `DecodeValidate()`, and `DecodeDiff()` methods.

#### Transaction Records (type: "transaction")

Payload structure (`TxData`):

```json
{
  "action": "begin" | "commit" | "rollback",
  "description": "Genesis: initial spec state",
  "parent_tx_id": "tx-parent123"
}
```

| Field | Type | Description |
|---|---|---|
| `action` | string | One of `begin`, `commit`, `rollback`. Maps to the transaction state machine. |
| `description` | string | Human-readable description of the transaction purpose. Present on `begin` records. |
| `parent_tx_id` | string | Optional. Links nested transactions. |

A complete transaction lifecycle produces exactly 2 or 3 records: `begin`, then zero or more intermediate records (validate, diff), then `commit` or `rollback`.

#### Validate Records (type: "validate")

Payload structure (`ValidateData`):

```json
{
  "spec_path": "/path/to/spec.md",
  "content_hash": "a1b2c3...",
  "total_checks": 12,
  "passed": 11,
  "failed": 1,
  "errors": 1,
  "warnings": 0,
  "results": [
    {
      "check_id": 3,
      "check_name": "Cross-reference integrity",
      "passed": false,
      "summary": "42 of 45 references resolved (93.3%)"
    }
  ]
}
```

`ImportValidation` converts a `validator.Report` to `ValidateData`, mapping each `CheckResult` to a `ValidateResult`. The `content_hash` enables linking validation results to specific spec versions.

#### Diff Records (type: "diff")

Payload structure (`DiffData`):

```json
{
  "base": {"spec_path": "/v1/spec.md", "content_hash": "abc..."},
  "head": {"spec_path": "/v2/spec.md", "content_hash": "def..."},
  "summary": {"added": 3, "removed": 1, "modified": 5, "unchanged": 42},
  "changes": [
    {
      "element_type": "invariant",
      "element_id": "INV-006",
      "action": "modified",
      "section_path": "§0.5",
      "content_hash_before": "abc...",
      "content_hash_after": "def...",
      "detail": "Updated violation scenario"
    }
  ]
}
```

The `SpecRef` pair (base and head) identifies the two spec versions being compared. Each `Change` records one element-level difference with before/after hashes for traceability.

#### Record Construction

Three factory functions create records with correct envelope fields:

- `NewTxRecord(txID, *TxData) -> (*Record, error)` --- Marshals TxData to JSON, sets type to "transaction".
- `NewValidateRecord(txID, *ValidateData) -> (*Record, error)` --- Marshals ValidateData, sets type to "validate".
- `NewDiffRecord(txID, *DiffData) -> (*Record, error)` --- Marshals DiffData, sets type to "diff".

All three call `Now()` for the timestamp (`time.Now().UTC().Format(time.RFC3339)`), set `Version = 1`, and marshal the type-specific data into `json.RawMessage`.

#### Filtering

`ReadFiltered` supports four filter dimensions:

| Filter | Field | Behavior |
|---|---|---|
| `Types` | `[]RecordType` | Include only records matching any listed type. Empty = all types. |
| `TxID` | `string` | Include only records with this transaction ID. Empty = all. |
| `Since` | `string` (RFC3339) | Include only records with timestamp >= this value. Empty = no lower bound. |
| `Limit` | `int` | Return at most this many records. 0 = unlimited. |

Filters are applied in order: type, tx_id, since, limit. The `bufio.Scanner` uses a 10MB max line buffer to handle large diff records.

**Implementation Trace:**
- Source: `internal/oplog/record.go::Record` (envelope struct)
- Source: `internal/oplog/record.go::NewTxRecord`
- Source: `internal/oplog/record.go::NewValidateRecord`
- Source: `internal/oplog/record.go::NewDiffRecord`
- Source: `internal/oplog/record.go::DecodeTx`
- Source: `internal/oplog/record.go::DecodeValidate`
- Source: `internal/oplog/record.go::DecodeDiff`
- Source: `internal/oplog/oplog.go::Append`
- Source: `internal/oplog/oplog.go::ReadFiltered`
- Source: `internal/oplog/oplog.go::ImportValidation`
- Tests: `tests/oplog_test.go::TestOplogAppend`
- Tests: `tests/oplog_test.go::TestOplogFilter`
- Tests: `tests/oplog_test.go::TestOplogDiffRecord`
- Tests: `tests/oplog_test.go::TestOplogValidateRecord`
- Tests: `tests/oplog_test.go::TestOplogTxRecord`

---

### Chapter: Seed and Log Commands

**Preserves:** APP-INV-006 (Transaction State Machine --- seed creates a complete begin-validate-commit transaction), APP-INV-010 (Oplog Append-Only --- both commands only append or read).

The `seed` and `log` commands provide the entry points for oplog lifecycle management: `seed` creates the genesis transaction that establishes the epoch state, and `log` reads and displays oplog records with filtering.

#### Seed Command

`ddis seed <index.db>` creates the genesis transaction --- the oplog's epoch record that captures the specification's initial validation state. All future diffs and audits compare against this baseline.

```
Algorithm: Genesis Seed
Input: index.db path, optional --oplog-path
Output: 3 oplog records (begin, validate, commit)

1. Open database, get first spec ID
2. Resolve oplog path (custom or default .ddis/oplog.jsonl)
3. Idempotency check: HasGenesisTransaction(oplogPath)
   - Scans all "transaction" records for action="begin" with description starting "Genesis:"
   - If found: print "Genesis transaction already exists, skipping." and return
4. Get spec metadata (spec_path, content_hash)
5. Generate transaction ID: generateTxID()
6. Create begin record:  NewTxRecord(txID, {action: "begin", description: "Genesis: initial spec state"})
7. Run full validation: validator.Validate(db, specID, {})
8. Import results:       ImportValidation(report, specPath, contentHash)
9. Create validate record: NewValidateRecord(txID, validateData)
10. Create commit record:  NewTxRecord(txID, {action: "commit"})
11. Append all three atomically: oplog.Append(oplogPath, beginRec, validateRec, commitRec)
12. Print summary: "Genesis transaction {txID} seeded (N checks: M passed, P failed)"
```

**Idempotency:** `HasGenesisTransaction` prevents duplicate genesis records. It reads all transaction-type records and checks for `action = "begin"` with `description` starting `"Genesis:"`. This is a prefix match, not an exact match, allowing variations in the description text.

**Atomicity:** The three records are passed to a single `oplog.Append` call. While the file is opened once and the encoder writes three lines sequentially, the write is not truly atomic (a crash between lines could produce a partial genesis). However, `ReadFiltered` with `TxID` filter would return only the records that were successfully written, and a subsequent `seed` call would detect the partial genesis via `HasGenesisTransaction`.

**Worked example:**

After parsing a specification that produces 12 validation checks (11 passed, 1 failed), the seed command appends:

```jsonl
{"version":1,"type":"transaction","timestamp":"2026-02-23T14:30:00Z","tx_id":"tx-a1b2c3d4e5f67890","data":{"action":"begin","description":"Genesis: initial spec state"}}
{"version":1,"type":"validate","timestamp":"2026-02-23T14:30:01Z","tx_id":"tx-a1b2c3d4e5f67890","data":{"spec_path":"spec.md","content_hash":"abc...","total_checks":12,"passed":11,"failed":1,"errors":1,"warnings":0,"results":[...]}}
{"version":1,"type":"transaction","timestamp":"2026-02-23T14:30:01Z","tx_id":"tx-a1b2c3d4e5f67890","data":{"action":"commit"}}
```

#### Log Command

`ddis log <oplog.jsonl>` reads and displays oplog records with optional filtering:

| Flag | Type | Description |
|---|---|---|
| `--json` | bool | Output as JSON array (via `json.MarshalIndent`) instead of human-readable format |
| `--type` | string | Filter by record type: `diff`, `validate`, or `transaction` |
| `--tx` | string | Filter by transaction ID |
| `--since` | string | Filter records after this RFC3339 timestamp |
| `--limit` | int | Maximum number of records to display (0 = unlimited) |

**Human-readable output** (default) is produced by `RenderLog`:

```
Operation Log (3 records)
═══════════════════════════════════════════

[1] 2026-02-23T14:30:00Z  type=transaction  tx=tx-a1b2c3d4e5f67890
    action=begin  "Genesis: initial spec state"

[2] 2026-02-23T14:30:01Z  type=validate  tx=tx-a1b2c3d4e5f67890
    spec.md: 12 checks, 11 passed, 1 failed (1 errors)

[3] 2026-02-23T14:30:01Z  type=transaction  tx=tx-a1b2c3d4e5f67890
    action=commit
```

**Edge case:** Non-existent oplog file returns an empty record set (not an error), and `RenderLog` displays "Operation Log (0 records)".

**Implementation Trace:**
- Source: `internal/cli/seed.go::runSeed`
- Source: `internal/cli/log.go::runLog`
- Source: `internal/oplog/oplog.go::HasGenesisTransaction`
- Source: `internal/oplog/oplog.go::RenderLog`
- Source: `internal/oplog/oplog.go::ReadFiltered`
- Source: `internal/oplog/oplog.go::ImportValidation`
- Tests: `tests/oplog_test.go::TestOplogAppend`
- Tests: `tests/oplog_test.go::TestOplogFilter`
- Tests: `tests/oplog_test.go::TestOplogRenderJSON`

---

### Chapter: RALPH Integration

**Preserves:** APP-INV-006 (Transaction State Machine --- RALPH phases are bracketed by transactions), APP-INV-010 (Oplog Append-Only --- RALPH records are appended).

**Interfaces:** APP-INV-002 (Validation Determinism --- RALPH compares validation results across iterations).

The RALPH (Recursive Autonomous Language Protocol Heuristic) improvement loop is the primary consumer of the lifecycle-ops subsystem. Each RALPH iteration is a structured sequence of phases --- audit, apply, judge --- and each phase interacts with transactions, validation records, and diff records.

#### Phase-Transaction Mapping

| RALPH Phase | Transaction Pattern | Oplog Records |
|---|---|---|
| **Bootstrap** | `seed` (genesis, if first run) | begin + validate + commit |
| **Audit** | Read-only (no transaction) | May append validate records for targeted checks |
| **Apply** | `tx begin` -> edits -> `tx commit` or `tx rollback` | begin + (validate)* + (diff)* + commit/rollback |
| **Judge** | Read-only (no transaction) | May append validate records for before/after comparison |
| **Polish** | `tx begin` -> consolidation edits -> `tx commit` | begin + diff + commit |

#### Seed as Epoch Marker

The `seed` command establishes the epoch --- the baseline against which all RALPH iterations are compared. The genesis transaction captures:

1. The spec's content hash at time zero (links validation results to a specific version).
2. The full validation report (how many checks passed/failed before any improvements).
3. A complete transaction lifecycle (begin + validate + commit) that anchors the oplog timeline.

RALPH's judge phase compares the current iteration's validation results against the genesis record (or the previous iteration's record) to measure improvement.

#### Diff Records for Structural Changes

After the apply phase modifies spec elements, a diff record captures the structural delta between the pre-edit and post-edit spec versions. The diff record includes:

- `base` and `head` SpecRef pairs (path + content hash).
- `summary` with aggregate counts (added, removed, modified, unchanged).
- `changes` array with element-level detail.

The RALPH judge phase reads these diff records to quantify the scope of changes and detect regressions (e.g., an apply phase that removes more elements than it adds).

#### Validate Records for Progress Tracking

Validation records capture the spec's health at checkpoints throughout the RALPH loop. By querying the oplog for validate records ordered by timestamp, the loop can compute:

- **Improvement trajectory**: is the pass rate monotonically increasing?
- **Regression detection**: did a specific check that was passing start failing?
- **Convergence signal**: are the last N iterations producing the same score?

The `ImportValidation` function bridges the `validator.Report` struct (used by the validation engine) and the `ValidateData` struct (used by the oplog), mapping each `CheckResult` to a `ValidateResult` with check_id, check_name, passed, and summary.

#### Transaction Boundaries for Phase Isolation

Each RALPH apply phase is wrapped in a transaction. This provides two guarantees:

1. **Atomicity**: If the apply phase fails or is interrupted, the transaction can be rolled back, and the oplog records the rollback. The judge phase knows to skip this iteration.
2. **Auditability**: The `tx_id` links all oplog records from one apply phase. A query for `--tx tx-abc123` returns exactly the records from that phase, enabling targeted replay or analysis.

The `parent_tx_id` field in `TxData` supports nested transactions (e.g., a RALPH iteration transaction that contains per-edit sub-transactions), though the current implementation does not use nesting.

**Implementation Trace:**
- Source: `internal/cli/seed.go::runSeed`
- Source: `internal/cli/tx.go::txBegin`
- Source: `internal/cli/tx.go::txCommit`
- Source: `internal/cli/tx.go::txRollback`
- Source: `internal/oplog/oplog.go::Append`
- Source: `internal/oplog/oplog.go::ImportValidation`
- Source: `internal/oplog/record.go::NewTxRecord`
- Source: `internal/oplog/record.go::NewValidateRecord`
- Source: `internal/oplog/record.go::NewDiffRecord`
- Tests: `tests/tx_test.go::TestTxBeginCommit`
- Tests: `tests/tx_test.go::TestTxFlushToOplog`
- Tests: `tests/oplog_test.go::TestOplogAppend`

---

### Chapter: Implementation Mirror (Check 13)

**Preserves:** APP-INV-016 (Implementation Traceability --- every annotation references existing files and locatable functions).

**Interfaces:** APP-INV-001 (Round-Trip Fidelity --- annotations are parsed from invariant `raw_text`, which must faithfully represent the source), APP-INV-011 (Validation Composability --- Check 13 is composable with other checks).

Check 13 is the mechanical enforcement mechanism for the spec-to-code bridge. It scans every invariant's raw text for Implementation Trace annotations, resolves each annotation against the source tree, and reports broken references as errors. This is the runtime enforcement of APP-INV-016 and the practical expression of APP-ADR-011's "mechanical verification replaces formal proof" philosophy.

#### Annotation Regex

The annotation pattern matches lines of this form:

```
- Source: `internal/storage/queries.go::CreateTransaction`
- Tests: `tests/tx_test.go::TestTxBeginCommit`
- Validates-via: `internal/validator/validator.go::Validate`
```

The regex:

```
^\s*-\s*(Source|Tests|Validates-via):\s*`([^`]+)::(\w+)`
```

Capture groups:

| Group | Content | Example |
|---|---|---|
| 1 | Kind | `Source`, `Tests`, or `Validates-via` |
| 2 | File path | `internal/storage/queries.go` |
| 3 | Function name | `CreateTransaction` |

`parseTraceAnnotations(rawText)` scans every line of an invariant's raw text and returns a slice of `traceAnnotation{Kind, FilePath, FuncName}` structs.

#### File Resolution

Each annotation's file path is resolved against the `--code-root` flag:

```
abs_path = filepath.Join(code_root, annotation.file_path)
```

If the path is already absolute (starts with `/`), it is used as-is. The file is checked with `os.Stat`:

- **File exists**: proceed to function matching.
- **File does not exist**: report error `"file not found: {path}"` and skip function matching.

#### Function Matching

`funcExistsInFile(filePath, funcName)` reads the file line by line and checks two regex patterns:

1. **Function pattern**: `func\s+(\([^)]+\)\s+)?FuncName\(` --- matches both standalone functions (`func FuncName(`) and method receivers (`func (r *Receiver) FuncName(`).
2. **Type pattern**: `type\s+FuncName\s+` --- matches type declarations (`type FuncName struct`).

This covers the three forms of Go declarations: package-level functions, methods with receivers, and type definitions.

#### Test Auto-Prefix

For annotations with kind `Tests`, the function name is auto-prefixed with `Test` if it does not already start with `Test`:

```
if ann.Kind == "Tests" && !strings.HasPrefix(expectedFunc, "Test") {
    expectedFunc = "Test" + expectedFunc
}
```

This allows annotation authors to write `Tests: tests/tx_test.go::TxBeginCommit` instead of the verbose `Tests: tests/tx_test.go::TestTxBeginCommit`. The auto-prefix adds `Test` and looks for `TestTxBeginCommit`.

#### Gating: CodeRoot Required

Check 13 only runs when `CodeRoot` is non-empty (set via the `--code-root` CLI flag). The `Applicable` method returns `c.CodeRoot != ""`. When CodeRoot is empty, the check is skipped entirely --- it does not report failures for missing annotations, it simply does not execute. This is intentional: validation without a source tree cannot verify file existence.

#### Reporting

Each annotation produces one finding:

| Condition | Severity | Message Pattern |
|---|---|---|
| File exists, function found | info | `"{INV-ID} {Kind} annotation OK: {path}::{func}"` |
| File does not exist | error | `"{INV-ID} {Kind} annotation: file not found: {path}"` |
| File exists, function not found | error | `"{INV-ID} {Kind} annotation: function {func} not found in {path}"` |
| File read error | error | `"{INV-ID} {Kind} annotation: error reading {path}: {err}"` |

The summary line aggregates results:

```
"{N} annotations: {M} valid, {P} broken ({Q} invariants with broken refs)"
```

If no invariants have Implementation Trace blocks, the summary is `"no annotations to verify"` (severity: info).

**Worked example:**

Given an invariant with three annotations:

```markdown
**Implementation Trace:**
- Source: `internal/storage/queries.go::CreateTransaction`
- Source: `internal/storage/queries.go::MissingFunction`
- Tests: `tests/tx_test.go::TxBeginCommit`
```

With `--code-root /data/projects/ddis/ddis-cli`:

1. `queries.go::CreateTransaction` --- file exists, `func CreateTransaction(` found -> info: "OK"
2. `queries.go::MissingFunction` --- file exists, no `func MissingFunction(` -> error: "function MissingFunction not found"
3. `tx_test.go::TxBeginCommit` --- auto-prefix -> `TestTxBeginCommit`, file exists, `func TestTxBeginCommit(` found -> info: "OK"

Summary: "3 annotations: 2 valid, 1 broken (1 invariants with broken refs)"

Check result: `Passed = false` (any broken annotation fails the check).

**Implementation Trace:**
- Source: `internal/validator/traceability.go::checkImplementationTraceability`
- Source: `internal/validator/traceability.go::parseTraceAnnotations`
- Source: `internal/validator/traceability.go::funcExistsInFile`
- Tests: `tests/traceability_test.go::TestTraceabilitySkippedWithoutCodeRoot`
- Tests: `tests/traceability_test.go::TestTraceabilityValidAnnotation`
- Tests: `tests/traceability_test.go::TestTraceabilityBrokenFile`
- Tests: `tests/traceability_test.go::TestTraceabilityBrokenFunction`
- Tests: `tests/traceability_test.go::TestTraceabilityMethodReceiver`
- Tests: `tests/traceability_test.go::TestTraceabilityNoAnnotations`

---

## Negative Specifications

These constraints prevent the most likely implementation errors and LLM hallucination patterns for the lifecycle operations subsystem. Each addresses a failure mode that an LLM, given only the positive specification, would plausibly introduce.

**DO NOT** modify or delete existing oplog records. The oplog is append-only by design. Any code path that opens the JSONL file with `O_RDWR`, `O_TRUNC`, or calls `Seek`, `WriteAt`, or `Truncate` violates the audit trail guarantee. This includes "optimization" operations like compaction, deduplication, or garbage collection. The only write operation is `Append` via `O_APPEND|O_CREATE|O_WRONLY`. (Validates APP-INV-010)

**DO NOT** allow transaction state transitions outside the defined state machine. Only `pending -> committed` and `pending -> rolled_back` are valid. Direct transitions between `committed` and `rolled_back`, from terminal states back to `pending`, or to any state not in the set `{pending, committed, rolled_back}` are forbidden. The SQL CHECK constraint and the `WHERE status = 'pending'` guard in CommitTransaction/RollbackTransaction are both required --- removing either one opens a state machine violation path. (Validates APP-INV-006)

**DO NOT** claim spec-to-code traceability without mechanical verification. Implementation Trace annotations are only valid if Check 13 confirms that referenced files exist and referenced functions are locatable. Unverified annotations must be flagged, not silently accepted. An LLM generating spec text must not add Implementation Trace annotations without running Check 13 to verify them. (Validates APP-INV-016)

**DO NOT** expose oplog file handles for random-access write. The `Append` function opens the file, writes, and closes it within a single function scope. No file handle is returned to callers. No public API in the `oplog` package accepts a writable file handle. This prevents callers from seeking to a record and overwriting it in place. (Validates APP-INV-010)

**DO NOT** skip the visited-set check in BFS impact analysis. Both `bfsForward` and `bfsBackward` must check `visited[nodeID]` before enqueueing a node. Removing this check causes infinite loops on cyclic cross-reference graphs. The visited set must be a `map[string]bool`, not a list (which would make the check O(n) instead of O(1) and degrade performance on large graphs). (Validates APP-INV-013)

**DO NOT** allow BFS depth to exceed 5. The `Analyze` function clamps `MaxDepth` to the range [1, 5] regardless of user input. A depth of 10 on a specification with 200 cross-references could visit every element in the spec, producing an unhelpfully large impact report and consuming significant time for database queries. The hard ceiling of 5 is a safety bound, not a performance optimization. (Validates APP-INV-013)

**DO NOT** auto-generate transaction IDs with predictable seeds. `generateTxID` must use `crypto/rand`, not `math/rand` or a timestamp-based scheme. Predictable IDs enable collision attacks (crafting a tx_id that matches an existing transaction, potentially overwriting its state). The 8-byte random space (2^64 possibilities) makes collisions negligible. (Validates APP-INV-006)

**DO NOT** run Check 13 without a code root. When `--code-root` is not set, the check's `Applicable` method returns false, and the check is skipped entirely. An LLM must not bypass this gate by hardcoding a path or defaulting to the current directory. Traceability verification requires an explicit, user-provided code root to avoid silently validating against the wrong source tree. (Validates APP-INV-016)

**DO NOT** auto-create SQLite databases from read-only command invocations. Commands that only read the database MUST call `storage.OpenExisting()` which verifies the file exists before opening. (Validates APP-INV-059)

**DO NOT** accept database paths via inconsistent positional conventions without also supporting the unified `--db` flag. Every command that accepts a database path MUST register the `--db` named flag. (Validates APP-ADR-050)

**DO NOT** add state transitions without verifying lifecycle reachability. Every new transition's output state must have a forward path to ValidatedSpec. (Validates APP-INV-062)

---

## Verification Prompt

Use this self-check after implementing or modifying the lifecycle operations subsystem.

**Positive checks (DOES the implementation...):**

- DOES `CommitTransaction` include `WHERE status = 'pending'` in the UPDATE clause? (APP-INV-006)
- DOES `RollbackTransaction` include `WHERE status = 'pending'` in the UPDATE clause? (APP-INV-006)
- DOES the transactions table include `CHECK(status IN ('pending', 'committed', 'rolled_back'))`? (APP-INV-006)
- DOES both `CommitTransaction` and `RollbackTransaction` check `RowsAffected() == 0` and return an error? (APP-INV-006)
- DOES `Append` open the file with exactly `os.O_APPEND|os.O_CREATE|os.O_WRONLY`? (APP-INV-010)
- DOES `ReadFiltered` return nil (not error) for non-existent files? (APP-INV-010, idempotent reads)
- DOES `bfsForward` check `visited[sourceID]` before enqueueing each node? (APP-INV-013)
- DOES `bfsBackward` check `visited[targetID]` before enqueueing each node? (APP-INV-013)
- DOES `Analyze` clamp `MaxDepth` to at most 5? (APP-INV-013)
- DOES `checkImplementationTraceability.Applicable` return false when `CodeRoot` is empty? (APP-INV-016)
- DOES `funcExistsInFile` match both standalone functions and method receivers? (APP-INV-016)
- DOES the Tests kind auto-prefix `Test` when the function name lacks it? (APP-INV-016)
- DOES the seed command check `HasGenesisTransaction` before creating the genesis record? (Idempotency)
- DOES each `tx` subcommand (begin, commit, rollback) write a corresponding oplog record? (Dual-write)

**Negative checks (does NOT the implementation...):**

- Does NOT any function in the `oplog` package open the file with `O_RDWR` or `O_TRUNC`? (NEG-LIFECYCLE-001, APP-INV-010)
- Does NOT `CommitTransaction` or `RollbackTransaction` omit the `AND status = 'pending'` guard? (NEG-LIFECYCLE-002, APP-INV-006)
- Does NOT Check 13 silently accept annotations without verifying file existence and function presence? (NEG-LIFECYCLE-003, APP-INV-016)
- Does NOT the oplog package expose a writable file handle to callers? (NEG-LIFECYCLE-004, APP-INV-010)
- Does NOT `bfsForward` or `bfsBackward` enqueue a node that is already in the visited set? (NEG-LIFECYCLE-005, APP-INV-013)
- Does NOT `Analyze` accept `MaxDepth > 5` from user input? (NEG-LIFECYCLE-006, APP-INV-013)
- Does NOT `generateTxID` use `math/rand` or any predictable seed? (NEG-LIFECYCLE-007, APP-INV-006)
- Does NOT Check 13 execute when `--code-root` is not provided? (NEG-LIFECYCLE-008, APP-INV-016)

---

## Referenced Invariants from Other Modules

Per the cross-module reference completeness convention, this section lists invariants
owned by other modules that this module depends on or interfaces with:

| Invariant    | Owner              | Relationship | Usage in This Module                                            |
|--------------|--------------------|--------------|------------------------------------------------------------------|
| APP-INV-001  | parse-pipeline     | interfaces   | Round-trip fidelity ensures transactional edits preserve content |
| APP-INV-002  | query-validation   | interfaces   | Validation determinism ensures oplog records are reproducible    |
| APP-INV-003  | query-validation   | interfaces   | Cross-ref integrity ensures impact analysis graph is accurate    |
| APP-INV-007  | query-validation   | interfaces   | Diff completeness ensures oplog diff records capture all changes |
| APP-INV-008  | search-intelligence| interfaces   | RRF scores in context bundles logged alongside transactions      |
| APP-INV-009  | parse-pipeline     | interfaces   | Monolith-modular equivalence for seed validation baseline        |
| APP-INV-011  | query-validation   | interfaces   | Check composability enables selective validation in transactions |
| APP-INV-012  | search-intelligence| interfaces   | LSI dimensions stable across index updates from transactions     |
| APP-INV-015  | parse-pipeline     | interfaces   | Deterministic hashing for content comparison in oplog records    |

**APP-INV-050: Challenge-Witness Adjunction Fidelity**

*For every invariant with a valid witness, challenge(witness(inv)) must return a verdict in {confirmed, provisional, refuted, inconclusive}. If refuted, the witness is automatically invalidated.*

```
FOR ALL inv IN invariants WHERE witness(inv).status = valid: challenge(inv).verdict IN {confirmed, provisional, refuted, inconclusive} AND (challenge(inv).verdict = refuted IMPLIES witness(inv).status = invalidated)
```

Violation scenario: An agent records a witness claiming full gophersat rewrite via test evidence. Challenge Level 3 finds no ddis:tests annotation. Level 5 finds low keyword overlap between invariant statement and evidence. Witness remains valid because no challenge was issued.

Validation: Record a witness. Run ddis challenge with --code-root. Verdicts: confirmed (test passed), provisional (code annotations + consistent semi-formal + confidence > 0.3), refuted (SAT contradiction or test failure), inconclusive (insufficient evidence). Modify test to not test invariant. Re-challenge. Verify refuted and witness invalidated.

// WHY THIS MATTERS: Without a mechanical challenge mechanism, the bilateral lifecycle is an open chain. Witness operates by attestation --- the implementing agent declares success, but nothing verifies the claim. At high implementation depth, k-star budget exhaustion causes silent constraint dropping: the agent satisfies proxy goals (tests pass) while violating the foundational invariant. Challenge closes the chain into a convergent cycle, catching attestation drift before it compounds.

**Confidence:** falsified

---

### APP-ADR-037: Challenge as Right Adjoint of Witness

#### Problem

Witnesses operate by attestation. The implementing agent declares success but nothing mechanically verifies the claim. K-star budget exhaustion at implementation depth causes silent constraint dropping.

#### Options

A) Challenge as separate command with 5-level mechanical verification. B) Strengthen witness to require proof at record time. C) External CI verification.

#### Decision

**Option A: Challenge as separate command with 5-level mechanical verification.** Challenge is a separate command operating with fresh context, using the spec as oracle. Five levels: formal (SAT), uncertainty (evidence scoring), causal (annotation lookup), practical (test execution), meta-reflective (keyword overlap). Separates concerns: witness records claims, challenge verifies them.

// WHY NOT Option B (strengthen witness)? Witness and challenge serve different roles. Witness records claims at implementation time. Requiring proof at record time conflates the claiming and verification concerns, and the implementing agent's context is already k-star-exhausted --- it cannot reliably verify its own work.

// WHY NOT Option C (external CI)? CI operates outside the bilateral lifecycle. Challenge must be a first-class DDIS command so it participates in the adjunction chain (witness ⊣ challenge) and its results feed back into the spec.

**Confidence:** Committed

#### Consequences

New adjunction witness-challenge in the bilateral lifecycle. Check 17 flags unchallenged witnesses. Challenge results stored in `challenge_results` table with per-level detail. Witness auto-invalidated on refutation. Self-bootstrapping: `ddis challenge APP-INV-050` verifies the challenge mechanism itself.

#### Tests

- Record witness, challenge, verify verdict in {confirmed, refuted, inconclusive}
- On refutation, verify witness status transitions to invalidated
- Verify 5-level decomposition: formal SAT, uncertainty scoring, causal annotation, practical test exec, meta keyword overlap
- Self-bootstrap: `ddis challenge --all` on CLI spec with 0 refuted

---

### APP-ADR-039: Evidence Accumulation Verdicts over Single-Path Confirmation

#### Problem

45 of 50 invariants are permanently stuck at provisional verdict because the single confirmation path requires a ddis:tests annotation pointing to a passing test. Many invariants describe architectural properties (append-only, monotonicity, isolation) that have strong code evidence — multiple independent ddis:implements/maintains annotations across packages, consistent semi-formals, passing test suite — but no dedicated behavioral test. The binary test_found→confirmed / no_test→provisional model discards compounding evidence.

#### Options

A) Write dedicated behavioral tests for all 45 invariants. Strongest proof but ~1500 LOC of test code, many properties resist unit testing. B) Dempster-Shafer inspired evidence accumulation — combine independent signals (multi-package annotations, semi-formal consistency, evidence type confidence, keyword overlap) into a belief mass that can cross the confirmation threshold without a dedicated test. C) Lower the confirmation bar — accept any annotation as sufficient.

#### Decision

**Option B: Evidence accumulation with targeted behavioral tests.** Base confidence from witness type (test=0.9, scan=0.7, review=0.8, annotation=0.6), boosted by independent code grounding signals — multi-package annotation spread (annotations from N distinct packages), implements verb presence (stronger than maintains-only), semi-formal parseable and consistent, high keyword overlap. Confirmation threshold: accumulated score >= 0.85. Dedicated ddis:tests written for directly-testable invariants (~15). Architectural invariants reach confirmed through accumulated evidence mass.

#### Consequences

computeVerdict becomes an evidence accumulation function rather than a strict chain. Verdicts become more nuanced — strong provisional (score 0.7-0.84) distinguished from weak provisional (0.3-0.69). Confirmed count expected to rise from 5 to 20+. The semantic distinction between declared and demonstrated is preserved — accumulated evidence from multiple independent sources provides demonstration without a single dedicated test.

#### Tests

TestComputeVerdict_EvidenceAccumulation, TestComputeVerdict_MultiPackageBoost, TestComputeVerdict_ImplementsVerbBoost

---

**APP-INV-051: Challenge-Informed Navigation**

*The ddis next command MUST query challenge_results and incorporate verdict distribution into its recommendation, prioritizing refuted invariant remediation above all other suggestions.*

```
FOR ALL states s: next(s).recommendations INCLUDES challenge_upgrade_actions(s) WHEN provisional_count(s) > 0; next(s).priority[0] = remediate(refuted(s)) WHEN refuted_count(s) > 0
```

Violation scenario: An agent runs ddis next after receiving 10 provisional and 2 refuted challenge verdicts. The next command reports "all aligned" because it only checks validation, coverage, and drift — never querying challenge_results. The refuted invariants silently remain unaddressed.

Validation: Run ddis challenge --all. Then run ddis next. Verify that next output includes challenge summary and recommends actions for provisional/refuted invariants. Verify refuted invariants appear as highest priority.

// WHY THIS MATTERS: Without challenge-informed navigation, the bilateral lifecycle is open — verdicts are computed but never consumed. Agents using ddis next never discover that invariants need strengthening, creating an invisible quality ceiling that compounds over time.

---

**APP-INV-052: Challenge-Driven Task Derivation**

*The ddis tasks command MUST derive actionable work items from challenge verdicts, with refuted invariant remediation at highest priority and provisional upgrade actions for invariants lacking behavioral tests.*

```
FOR ALL inv IN provisional_invariants: EXISTS task IN tasks(--from-challenges) WHERE task.target = inv AND task.action IN {write_test, add_annotations}; FOR ALL inv IN refuted_invariants: EXISTS task IN tasks(--from-challenges) WHERE task.target = inv AND task.action = remediate AND task.priority > max(discovery_task_priorities)
```

Violation scenario: An agent runs ddis tasks to find work. 23 invariants have provisional verdicts and 1 has refuted. The tasks command returns only discovery-derived items because it lacks challenge-aware derivation rules. The agent works on low-priority discovery tasks while critical refutation remediation goes unaddressed.

Validation: Run ddis challenge --all. Then run ddis tasks --from-challenges. Verify that refuted invariants produce remediation tasks at highest priority. Verify provisional invariants produce upgrade tasks (write test, add annotations). Verify challenge-derived tasks sort above discovery-derived tasks.

// WHY THIS MATTERS: Task derivation without challenge feedback is like navigating without a compass. The most actionable quality signal — verification verdicts — never generates work items, causing agents to optimize for discovery breadth while ignoring verification depth.

---

**APP-INV-053: Event Stream Completeness**

*Every state-mutating CLI command MUST emit a typed event to the appropriate event stream before returning, covering parse, validate, drift, contradict, patch, render, skeleton, manifest-sync, state, absorb, refine, init, and witness operations.*

```
FOR ALL commands c IN {parse, validate, drift, contradict, patch, render, skeleton, manifest-sync, state, absorb, refine, init, witness}: post(c) IMPLIES EXISTS event e IN stream(c) WHERE e.type = event_type(c) AND e.spec_hash = current_spec_hash AND e.timestamp <= now()
```

Violation scenario: An agent runs ddis parse followed by ddis validate. Both succeed, but no events appear on Stream 2 or Stream 3. A downstream event consumer (ddis drift, monitoring) has no visibility into these state changes. The event stream shows only discovery activity, giving a false impression that no spec work occurred.

Validation: Run each of the 13 covered commands and verify the corresponding event appears on its stream. Stream 2 (Specification): parse emits spec_parsed, validate emits validation_run, drift emits drift_measured, contradict emits contradiction_detected, patch emits amendment_applied, manifest-sync emits amendment_applied. Stream 1 (Discovery): render emits artifact_written, skeleton emits artifact_written, init emits artifact_written, refine emits finding_recorded. Stream 3 (Implementation): state emits status_changed, absorb emits implementation_finding, witness emits status_changed.

// WHY THIS MATTERS: The spec defines 22 event types across 3 streams, but only discovery and challenge events are wired. Silent state mutations break the observability contract and prevent the bilateral lifecycle from closing its feedback loops through event-driven reactions.

---

### APP-ADR-041: Challenge-Feedback Loop Closes Bilateral Lifecycle

#### Problem

The bilateral lifecycle has 4 missing morphisms: challenge verdicts do not flow back to navigation (ddis next), challenge verdicts do not generate tasks (ddis tasks), state-mutating commands do not emit events, and no LLM-based evaluation evidence type exists. These broken arrows mean the lifecycle category is not traced — feedback loops are open.

#### Options

A) Targeted 4-phase closure with specific code changes per morphism. B) Generic event bus deriving everything from events. C) Leave open and rely on manual intervention.

#### Decision

**Option A: Targeted 4-phase closure.** Phase 1 wires challenge-to-next, challenge-to-tasks, and command-to-events. Phase 2 adds honest behavioral tests for mechanically-testable invariants. Phase 3 adds LLM provider abstraction with eval evidence type. Phase 4 adds Tier 6 semantic contradiction detection.

#### Consequences

ddis next becomes challenge-aware (APP-INV-051). ddis tasks derives from challenges (APP-INV-052). Five event types get wired (APP-INV-053). LLM provider enables eval-based witnessing (APP-INV-054). Tier 6 catches semantic contradictions formal methods miss (APP-ADR-042). Each phase follows bilateral lifecycle: discover, crystallize, parse, validate, implement, scan, drift, witness, challenge.

#### Tests

TestNextIncludesChallengeResults, TestTasksFromChallenges, TestEventEmission_Parse, TestEventEmission_Validate, TestEventEmission_Drift

---

**APP-INV-059: Database Path Validation**

*Before opening a SQLite database, the CLI MUST validate that the path refers to an existing file when the operation is read-only (validate, query, search, coverage, drift, etc.). Auto-creation is permitted ONLY for write-intending operations (parse, init). For read-only commands, a non-existent path MUST produce a clear error with recovery guidance.*

```
FOR ALL cmd IN ReadOnlyCommands, path = cmd.db_path: NOT FileExists(path) IMPLIES cmd.Execute() = Error("database not found: " + path, RecoveryHint("ddis parse <manifest> -o " + path)) AND NOT FileCreated(path)
```

Violation scenario: A user runs ddis validate my-spec.md manifest.ddis.db with swapped arguments. storage.Open silently creates a new empty database at my-spec.md, applies the schema, and reports no spec found.

Validation: For each read-only command, invoke with a non-existent database path. Verify error exit, stderr contains path and recovery hint, and no file was created.

// WHY THIS MATTERS: Auto-creation is correct for parse but catastrophically wrong for read commands. Silent creation from mistyped paths produces confusing errors and can corrupt existing files.

---

**APP-INV-062: Lifecycle Reachability**

*Every state reachable from T_init via defined state transitions MUST have a forward transition path to ValidatedSpec. No dead-end states exist in the lifecycle graph. No orphan states exist. The transition graph is connected.*

```
FOR ALL s IN ReachableStates(T_init): EXISTS path [T_1,...,T_n] from s to ValidatedSpec WHERE each T_i IN DefinedTransitions(section_0_2_1) AND |path| is finite. FOR ALL T IN DefinedTransitions: OutputState(T) IN ReachableStates(T_init) -- No orphan states: every transition output is reachable from init
```

Violation scenario: User runs ddis init, adds module declarations to manifest, reaches state (manifest exists, no module files). No CLI command produces module stubs from manifest. State is a dead-end -- unreachable from ValidatedSpec without manual file creation outside the CLI. This violates CLI-mediates-all-interactions.

Validation: Extract transition graph from section 0.2.1. Run BFS from T_init output state. Verify every reachable state has at least one outgoing transition toward ValidatedSpec. Report dead-end states as validation failures.

// WHY THIS MATTERS: Individual gap fixes are reactive. Reachability verification is preventive. It catches the ENTIRE CLASS of missing morphisms -- any future command that creates a state without a forward path triggers Check 20 at spec-validation time. This is the fixed point of self-bootstrapping.

---

**APP-INV-103: Witness Lifecycle Completeness**

*For every spec-mutating CLI operation (crystallize, absorb, refine apply, parse), the witness system responds: auto-recording (crystallize at Level 1), auto-suggesting (absorb correspondences, refine guidance), or auto-invalidating (parse). No spec mutation exits without witness system acknowledgment.*

```
FOR ALL op IN {crystallize, absorb, refine_apply, parse}:
  LET (S, S') = op(args) WHERE S != S':
    response(op, S, S') != EMPTY

-- crystallize: storage.InsertWitness(db, witness{EvidenceType: "attestation", ProvenBy: "ddis-crystallize"})
-- absorb: IF correspondence.Score > 0.8 THEN fmt.Fprintf(os.Stderr, "Suggest: ddis witness ...")
-- refine_apply: fmt.Fprintf(os.Stderr, "Re-witness modified: ...")
-- parse: storage.InvalidateWitnesses(db, oldSpecID, newSpecID) [EXISTS]
```

Violation scenario: Agent crystallizes APP-INV-104 via `ddis discover crystallize`. The invariant is written to the module file and the manifest is updated, but no witness is auto-recorded and no guidance suggests witnessing. The invariant enters "proven by nothing" limbo.

Validation: Crystallize an invariant, verify `storage.ListWitnesses` returns an entry with `EvidenceType: "attestation"` and `ProvenBy: "ddis-crystallize"`. Run `ddis absorb ./src --against db.db` with a high-confidence correspondence, verify stderr contains `ddis witness` guidance. Run `ddis refine apply`, verify output includes re-witness guidance for modified elements.

// WHY THIS MATTERS: Without lifecycle completeness, the witness system has a blind spot: it detects staleness (parse) but ignores creation (crystallize) and modification (absorb, refine). New invariants born from code evidence should start life as witnessed. Without this, the witness flywheel has a cold-start problem.

---

**APP-INV-105: CI Witness Gate**

*`ddis witness --check --strict` exits with code 1 if any witnesses are stale or if witness coverage is below a configurable threshold. Without `--strict`, the check is informational only (exit 0). Threshold defaults to 0% (only staleness triggers failure) and is configurable via `--threshold`.*

```
LET summary = witness.Check(db, specID, CheckOptions{AsJSON: witnessJSON}):
  IF NOT strict: RETURN nil  -- exit 0
  IF summary.Stale > 0: os.Exit(1)
  IF threshold > 0 AND summary.Total > 0:
    LET cov = float64(summary.Valid) / float64(summary.Total):
      IF cov < threshold: os.Exit(1)
  RETURN nil  -- exit 0
```

Violation scenario: CI pipeline runs `ddis witness --check` after spec changes. Three witnesses are stale. The command prints a summary but exits 0. CI proceeds, deploys code that no longer satisfies its invariants.

Validation: Witness some invariants, stale one via spec change, run `ddis witness --check` (exits 0). Run `ddis witness --check --strict` (exits 1). Test `--threshold 0.8` when coverage is 70% (exits 1). Test with 100% valid, no stale (exits 0).

// WHY THIS MATTERS: Without a non-zero exit code, witness checking is advisory. The --strict flag turns observation into enforcement -- the single place in DDIS where the system prescribes rather than observes, and it's opt-in.

---

### APP-ADR-075: Witness Lifecycle Automation Strategy

#### Problem

DDIS has five spec-mutating operations (parse, crystallize, absorb, refine apply, patch). The witness system responds to exactly one (parse -> auto-invalidate via `storage.InvalidateWitnesses`). The other four are witness-blind: they mutate the spec without any witness system acknowledgment.

#### Options

A) Full automation: Every operation auto-records or auto-invalidates witnesses. Risk: auto-recording Level 1 attestation for modifications where the agent has not verified anything. B) Guidance only: No auto-recording anywhere. All operations emit guidance text only. Misses the insight that crystallization IS attestation. C) Hybrid: Auto-record on creation (crystallize -> Level 1), guidance on modification (absorb, refine), auto-invalidation on parse (existing).

#### Decision

**Option C: Hybrid.** Crystallization is attestation: the agent creates the invariant FROM code evidence, so Level 1 auto-witness is semantically correct. For modifications (absorb correspondences, refine improvements), the correct witness level depends on what the agent does next, so guidance is appropriate. Parse auto-invalidation is already implemented. Must NOT auto-record witnesses above Level 1 without mechanical verification. Must NOT block primary operations on witness failures (best-effort). Must NOT emit witness guidance when --prompt-only or --json mode is active. WHY NOT Option A? Creates false confidence for modifications. WHY NOT Option B? Misses the crystallize-is-attestation insight.

#### Consequences

ddis discover crystallize auto-records Level 1 attestation witness (best-effort, stderr on failure). ddis absorb --against prints witness suggestion for correspondences. ddis refine audit/apply prints stale-witness warnings and re-witness guidance. ddis tasks --from-challenges --spec enriches output with witness status and priority boost. ddis witness --check --strict provides CI gate with configurable threshold. No changes to parse or progress (already complete).

#### Tests

Crystallize invariant -> storage.GetWitness returns non-nil with EvidenceType="attestation". Absorb with correspondence -> stderr contains "ddis witness". Tasks with stale witness -> priority decremented, JSON has witness_status. witness --check --strict with stale -> exit code 1. witness --check without strict with stale -> exit code 0.

---

### APP-ADR-046: Existence-Check Before Open over Auto-Create

#### Problem

storage.Open() auto-creates databases. Read-only commands silently create empty databases from mistyped paths.

#### Options

A) Split Open into OpenExisting and OpenCreate. B) Add create bool parameter. C) Check existence in each commands RunE.

#### Decision

**Option A: Split into OpenExisting and OpenCreate.** OpenExisting checks os.Stat before sql.Open. WHY NOT Option B? Open(path, false) is less readable than OpenExisting(path). WHY NOT Option C? 22 commands would duplicate the same 4-line check.

#### Consequences

OpenExisting = os.Stat + sql.Open + migration + PRAGMAs. OpenCreate = current behavior. 22 read-only commands switch to OpenExisting.

#### Tests

OpenExisting on non-existent path: error, no file created. ddis validate nonexistent.db: error with recovery hint. All 22 read-only commands use OpenExisting.

---

### APP-ADR-050: Unified --db Flag over Positional Ambiguity

#### Problem

22 commands accept DB as 1st positional; search/query accept it as 2nd. Inconsistency triggers auto-creation bug.

#### Options

A) Unified --db named flag. B) Normalize all to DB-first. C) Normalize all to DB-last.

#### Decision

**Option A: Unified --db flag on root command.** Positional retained for backward compatibility. --db takes precedence. WHY NOT Option B? ddis search index.db query reads as if DB is the query. WHY NOT Option C? Multi-word queries make boundary ambiguous.

#### Consequences

Global --db flag on root command. --db precedence over positional. Auto-discovery fallback. Backward compatible.

#### Tests

ddis validate --db path.db: uses path.db. ddis search --db path.db query: unambiguous. --db + positional: --db wins.

---

### APP-ADR-052: Reachability Check (Check 20) over Manual Audit

#### Problem

The lifecycle state transition graph can have dead-end states (states with no forward path to ValidatedSpec) that manual audit fails to catch. G1 was missed by 3 convergence audits.

#### Options

A) Check 20: BFS reachability on transition graph. B) Manual audit at each convergence. C) External model checker.

#### Decision

**Option A: Check 20 extracts state transition graph from section 0.2.1.** Runs BFS from T_init output states, reports dead-end states. WHY NOT Option B? Manual review missed G1 despite 3 audits. Missing-morphism errors are systematic. WHY NOT Option C? External tools violate self-bootstrapping. The CLI must verify its own spec.

#### Consequences

New Check 20: checkLifecycleReachability. Graph-theoretic (BFS/DFS). Graceful degradation when transition graph not extractable. Level 3 progressive validation.

#### Tests

Transition graph with no dead-ends: passes. Graph with one dead-end: reports it. Disconnected component: reports unreachable states.

---

### Manifest Integrity

**APP-INV-099: Manifest Mutation via Structured Serialization**

*Any programmatic modification to manifest.yaml MUST use a YAML parser for reading and a YAML serializer for writing. String concatenation, substring insertion, or regex-based edits on the raw file content are prohibited. The serialization round-trip must preserve all existing keys, comments within the YAML data model, and structural relationships.*

```
Let M be a manifest file, parse(M) its YAML AST, and serialize(AST) the inverse. For any mutation f on the AST: serialize(f(parse(M))) must be valid YAML AND parse(serialize(f(parse(M)))) = f(parse(M)). String-level operations g(text(M)) do NOT satisfy this because g is not a homomorphism from the character monoid to the YAML CFG.
```

Violation scenario: crystallize command appends a registry entry via string concat before the last newline. If the file lacks a trailing newline, the entry is appended to the last line. If a concurrent crystallize interleaves, both writes corrupt each other.

Validation: Round-trip test: parse manifest, mutate AST, serialize, parse again, verify equality. Concurrent test: two parallel crystallize calls, verify both entries present and YAML valid.

// WHY THIS MATTERS: The manifest is the spec's structural skeleton (APP-ADR-010). Corrupting it via string manipulation breaks parse (APP-INV-001), module resolution, and the entire bilateral lifecycle. The crystallize command is the primary spec authoring path (APP-INV-088) — it must be safe.

### APP-ADR-075: Structured YAML Serialization for Manifest Mutations

#### Problem
The crystallize command modifies manifest.yaml by appending strings before the last newline. This bypasses the YAML grammar, risking corruption on edge cases (no trailing newline, concurrent writes, inline comments) and violating the bilateral lifecycle's requirement that all spec authoring be mechanically sound.

#### Options
Option A: YAML parse-mutate-serialize via gopkg.in/yaml.v3 (already a dependency). Reads file, unmarshals to map, modifies map, marshals back.
Option B: Template-based generation. Regenerate entire manifest from DB state each time.
Option C: Patch-based editing via a YAML-aware diff/patch library.
Option D: Status quo (string concatenation). Rejected due to corruption risk.

#### Decision
**Option A: Use gopkg.in/yaml.v3 Unmarshal/Marshal for all manifest mutations.** Read file, unmarshal to typed struct, modify struct fields, marshal back. Simplest correct approach using existing dependency. // WHY NOT B: Loses manual formatting. C: Over-engineered. D: Provably unsafe.

#### Consequences
Safe manifest mutation by construction. Round-trip YAML validity guaranteed. Existing dependency reused.

#### Tests
Round-trip: parse manifest, add entry, serialize, parse again. Concurrent: two parallel crystallize calls. Edge: no trailing newline, empty registry.

---

### Witness and Drift Integrity

**APP-INV-107: Witness ID stability under upsert**

*Witness upsert operations must preserve the row ID when updating an existing witness, ensuring FK references from challenge_results remain valid.*

```
forall w in invariant_witnesses: upsert(w) => w.id_after == w.id_before
AND forall c in challenge_results: c.witness_id IN invariant_witnesses.id
```

Violation scenario: InsertWitness uses INSERT OR REPLACE which deletes the old row (destroying its ID) then inserts a new row with a new auto-increment ID. challenge_results.witness_id references the old ID, causing FK constraint failure.

Validation: Test: insert witness, insert challenge referencing it, upsert witness, verify challenge FK still valid.

// WHY THIS MATTERS: Without ID stability, the witness-challenge adjunction breaks. Challenges become orphaned, Check 17 fails, and the bilateral verification loop is severed.

**APP-INV-106: Drift contradiction integration**

*The drift analysis formula must integrate contradictions detected by the consistency checker, not leave the contradictions counter at zero. The 2x penalty for contradictions in the drift formula must be exercised.*

```
drift.contradictions >= |consistency.Analyze(db, specID).filter(t => t.Type == contradiction)|
```

Violation scenario: drift.go declares `contradictions := 0` but never increments it. The drift formula includes 2*contradictions but the value is always 0, meaning contradictory specs get the same drift score as consistent ones.

Validation: Test: create spec with contradictory invariants, run drift analysis, verify contradictions > 0 and drift score includes 2x penalty.

// WHY THIS MATTERS: Contradictions are the most severe form of drift — a spec that contradicts itself is worse than one with missing elements. The 2x penalty exists for this reason but is dead code.

### APP-ADR-077: ON CONFLICT DO UPDATE for witness upsert

#### Problem
INSERT OR REPLACE on witnesses deletes the old row and creates a new one with a different auto-increment ID. This orphans FK references in challenge_results, causing FOREIGN KEY constraint failures.

#### Options
A) ON CONFLICT DO UPDATE (preserves row ID). B) ON DELETE CASCADE on FK (auto-deletes challenges). C) Remove FK constraint entirely (loose coupling).

#### Decision
**Option A: ON CONFLICT DO UPDATE.** Use INSERT ... ON CONFLICT(spec_id, invariant_id) DO UPDATE SET ... which preserves the existing row ID while updating all fields. This maintains FK integrity without requiring cascade rules or removing constraints.

#### Consequences
ON CONFLICT DO UPDATE is the correct SQLite pattern for upsert-with-identity-preservation. It avoids the DELETE+INSERT semantics of INSERT OR REPLACE, keeping the row ID stable. CASCADE would silently destroy challenge evidence. Removing the FK would weaken data integrity.

#### Tests
1. Insert witness (ID=5), insert challenge with witness_id=5, upsert witness, verify challenge.witness_id=5 still valid. 2. Verify LastInsertId returns existing ID on conflict path.

---
