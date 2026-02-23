---
module: lifecycle-ops
domain: lifecycle
maintains: [APP-INV-006, APP-INV-010, APP-INV-013, APP-INV-016]
interfaces: [APP-INV-001, APP-INV-002, APP-INV-003, APP-INV-007, APP-INV-008, APP-INV-009, APP-INV-011, APP-INV-012, APP-INV-015]
implements: [APP-ADR-007, APP-ADR-008, APP-ADR-011]
adjacent: [parse-pipeline, search-intelligence, query-validation]
negative_specs:
  - "Must NOT modify or delete existing oplog records"
  - "Must NOT allow transaction state transitions outside the defined state machine"
  - "Must NOT claim spec-to-code traceability without mechanical verification"
---

# Module: Lifecycle Operations

This module owns the transactional mutation, operation logging, and
implementation traceability subsystems of the DDIS CLI. It governs how spec
modifications flow through the transaction state machine, how operations are
durably recorded in the append-only oplog, and how the spec-to-code mapping
is mechanically verified.

The module maintains four invariants: transaction state machine correctness
(APP-INV-006), oplog append-only integrity (APP-INV-010), BFS impact
termination (APP-INV-013), and implementation traceability (APP-INV-016).
It interfaces broadly with all other modules since lifecycle operations
touch parsed data, search indices, and validation results.

The RALPH recursive improvement loop integrates with this module through
the `seed` and `log` commands, which record improvement cycles in the oplog
and drive the audit-apply-judge workflow.

**Invariants referenced from other modules (APP-INV-018 compliance):**
- APP-INV-001: Parse-render round-trip produces byte-identical output (maintained by parse-pipeline)
- APP-INV-002: Validation results are deterministic (maintained by query-validation)
- APP-INV-003: Every resolved cross-reference points to existing element (maintained by query-validation)
- APP-INV-007: Structural diff reports every change, no silent drops (maintained by query-validation)
- APP-INV-008: RRF fusion score equals correctly computed weighted sum (maintained by search-intelligence)
- APP-INV-009: Parsing monolith ≡ parsing modules from same source (maintained by parse-pipeline)
- APP-INV-011: Validation checks are composable (maintained by query-validation)
- APP-INV-012: LSI k-dimensions ≤ doc count (maintained by search-intelligence)
- APP-INV-015: Content hashes are deterministic — SHA-256, no salt (maintained by parse-pipeline)

---

## Invariants Maintained by This Module

**APP-INV-006: Transaction State Machine**

*The transaction lifecycle MUST follow a strict state machine: only `pending → committed` or `pending → rolled_back` transitions are permitted. A committed or rolled-back transaction is immutable.*

```
∀ tx ∈ transactions:
  tx.status ∈ {pending, committed, rolled_back}
  ∧ (tx.status = committed ⟹ prev(tx.status) = pending)
  ∧ (tx.status = rolled_back ⟹ prev(tx.status) = pending)
  ∧ (tx.status ∈ {committed, rolled_back} ⟹ immutable(tx))
```

Violation scenario: A bug in the rollback handler allows a `committed` transaction to transition to `rolled_back`. An auditor reviewing the oplog sees a transaction that was both committed and rolled back, destroying the audit trail's integrity.

Validation: Attempt every possible state transition: pending→committed (valid), pending→rolled_back (valid), committed→pending (must fail), committed→rolled_back (must fail), rolled_back→pending (must fail), rolled_back→committed (must fail). Any accepted invalid transition violates APP-INV-006.

// WHY THIS MATTERS: The transaction state machine is the concurrency control mechanism. Invalid transitions corrupt the operation log and make it impossible to determine which version of a spec element is authoritative.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/storage/queries.go::CreateTransaction`
- Source: `internal/storage/queries.go::CommitTransaction`
- Source: `internal/storage/queries.go::RollbackTransaction`
- Tests: `tests/tx_test.go::TestTxBeginCommit`
- Tests: `tests/tx_test.go::TestTxRollback`

---

**APP-INV-010: Oplog Append-Only**

*The operation log (oplog) MUST be strictly append-only. Once a record is written, it MUST never be modified or deleted.*

```
∀ record r ∈ oplog, ∀ time t₁ < t₂:
  read(oplog, r.id, t₁) = read(oplog, r.id, t₂)
  ∧ count(oplog, t₂) ≥ count(oplog, t₁)
```

Violation scenario: An optimization adds a "compact" command that removes duplicate validation records from the oplog to save disk space. A RALPH audit agent that previously observed a validation failure can no longer find the record, breaking the improvement history.

Validation: Append N records to the oplog. Read all records. Verify count = N and content matches. Attempt to open the file for write-in-place (truncate or overwrite) — the API must not expose such an operation. Verify no CLI command modifies existing records.

// WHY THIS MATTERS: The oplog is the audit trail for all spec mutations. If records can be silently modified or deleted, the RALPH loop cannot reliably compare versions or detect regressions. Append-only guarantees historical completeness.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/oplog/oplog.go::Append`
- Source: `internal/oplog/oplog.go::ReadAll`
- Tests: `tests/oplog_test.go::TestOplogAppend`
- Tests: `tests/oplog_test.go::TestOplogFilter`

---

**APP-INV-013: Impact Termination**

*The BFS impact analysis algorithm MUST visit each node at most once and terminate in bounded time, even in the presence of cyclic cross-references.*

```
let visited = ∅
∀ step ∈ BFS(start_node, direction, max_depth):
  step.node ∉ visited
  ∧ visited' = visited ∪ {step.node}
  ∧ |visited| ≤ |all_nodes|
  ∧ step.depth ≤ max_depth
```

Violation scenario: Two sections cross-reference each other (§A → §B and §B → §A). The BFS impact analysis follows the cycle indefinitely, consuming memory until the process is killed. The `ddis impact` command hangs.

Validation: Create a spec with a deliberate cycle (two sections referencing each other). Run `ddis impact` on one of them with depth=10. The command must terminate and the visited set must contain each node at most once. Node count must not exceed total elements in the spec.

// WHY THIS MATTERS: Cross-reference cycles are common in specifications (mutual dependencies between subsystems). The impact analysis must handle them gracefully. An infinite loop renders the `impact` command unusable on real-world specs.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/impact/impact.go::bfsForward`
- Source: `internal/impact/impact.go::bfsBackward`
- Tests: `tests/impact_test.go::TestImpactForward`
- Tests: `tests/impact_test.go::TestImpactDepthLimit`

---

**APP-INV-016: Implementation Traceability**

*Every invariant that carries Implementation Trace annotations (Source, Tests, Validates-via) MUST reference files that exist and functions that can be mechanically located in the source tree.*

```
∀ inv ∈ invariants, ∀ annotation ∈ inv.implementation_trace:
  file_exists(code_root / annotation.file_path)
  ∧ grep("func.*" + annotation.function_name + "\\(", file) ≠ ∅
```

Violation scenario: APP-INV-008 claims `Source: internal/search/engine.go::FusionSearch` but the function was renamed to `fuseResults` during a refactor. The annotation is now a dead reference — the spec claims traceability to code that no longer exists under that name.

Validation: For every `Source:`, `Tests:`, and `Validates-via:` annotation in every invariant, resolve the file path relative to `--code-root`, verify the file exists, and grep for `func FunctionName(`. Report broken references. This is exactly what Check 13 (`CheckImplementationTraceability`) performs.

// WHY THIS MATTERS: Implementation Trace annotations are the bridge between specification claims and executable code. Dead references create false confidence — the spec appears traced but the link is broken. Check 13 mechanically detects this drift.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/validator/traceability.go::checkImplementationTraceability`
- Tests: `tests/traceability_test.go::TestTraceabilityValidAnnotation`
- Tests: `tests/traceability_test.go::TestTraceabilityBrokenFunction`
- Validates-via: `internal/validator/validator.go::Validate`

---

## ADR Stubs

### APP-ADR-007: JSONL Oplog Format

#### Problem

The operation log needs a durable, human-readable, append-friendly
storage format.

#### Decision

Use newline-delimited JSON (JSONL) files for the oplog. Each line
is a self-contained JSON record with a timestamp, operation type, and payload.
JSONL supports atomic append (single write syscall), is trivially parseable, and
works with standard Unix tools (grep, jq).

#### Consequences

- Append via `os.OpenFile(path, os.O_APPEND|os.O_WRONLY|os.O_CREATE, 0644)`
- Each record is one line — no multi-line JSON
- Survives database recreation (oplog is independent of SQLite)
- Records include: `type` (diff/validate/tx), `timestamp`, `payload`

> Full ADR body pending Phase 4.6.

---

### APP-ADR-008: Surgical Edit Strategy

#### Problem

Spec modifications need to update the minimum set of elements
while maintaining referential integrity across the index.

#### Decision

Adopt a surgical edit strategy: modifications target individual
elements by ID, recompute affected hashes, and propagate through the impact
graph. Full re-parse is avoided except when structural changes (heading moves,
section splits) invalidate the section tree.

#### Consequences

- Element-level updates via `UPDATE` SQL statements
- Impact analysis determines cascading changes
- Full re-parse as fallback for structural mutations

> Full ADR body pending Phase 4.6.

---

### APP-ADR-011: Structured Intent over Formal Derivation

#### Problem

The spec needs to communicate design rationale to LLM agents,
but formal specification languages (TLA+, Alloy) impose a learning barrier
and tooling dependency.

#### Decision

Use structured natural language with semi-formal predicates
rather than full formal methods. Each invariant includes a natural-language
statement and a pseudo-code predicate that is readable without specialized
tooling. This trades mathematical rigor for accessibility, accepting that
mechanical validation is the primary enforcement mechanism.

#### Consequences

- Invariants use pseudo-code predicates, not TLA+ or Alloy
- Check 13 verifies implementation traces mechanically
- Confidence levels track verification depth without requiring proofs
- LLM agents can read and act on invariant definitions directly

> Full ADR body pending Phase 4.6.

---

## Implementation Chapters (Pending)

### Transaction State Machine

> Pending Phase 4.6. Will specify the `pending → committed | rolled_back`
> state machine, concurrency control, and the `tx begin` / `tx commit` /
> `tx rollback` command implementations.

### Oplog Schema

> Pending Phase 4.6. Will specify the JSONL record schema for validate,
> change, seed, and log operations, including the record type discriminator
> and payload structures.

### Seed / Log Commands

> Pending Phase 4.6. Will specify the `seed` command (register a new spec
> version in the oplog) and the `log` command (query and display the oplog).

### RALPH Integration

> Pending Phase 4.6. Will specify how the RALPH recursive improvement loop
> consumes oplog records and drives the audit-apply-judge workflow through
> the CLI's validate and change commands.

### Implementation Mirror (Check 13)

> Pending Phase 4.6. Will specify Check 13 (Implementation Traceability),
> the validation check that mechanically verifies Source/Tests/Validates-via
> annotations in invariant raw text against the actual source tree. This is
> the enforcement mechanism for APP-INV-016.

---

## Negative Specifications

The following constraints define what this module must NOT do. Violations
indicate architectural regression.

**DO NOT** modify or delete existing oplog records. The oplog is append-only by design. Any code path that updates or removes an existing JSONL record violates the audit trail guarantee. (Validates APP-INV-010)

**DO NOT** allow transaction state transitions outside the defined state machine. Only `pending → committed` and `pending → rolled_back` are valid. Direct transitions between `committed` and `rolled_back`, or from terminal states back to `pending`, are forbidden. (Validates APP-INV-006)

**DO NOT** claim spec-to-code traceability without mechanical verification. Implementation Trace annotations are only valid if Check 13 confirms that referenced files exist and referenced functions are locatable. Unverified annotations must be flagged, not silently accepted. (Validates APP-INV-016)

**DO NOT** expose oplog file handles for random-access write. The only write operation is append. No seek-and-overwrite, no truncation, no in-place modification. (Validates APP-INV-010)

---

## Referenced Invariants from Other Modules

Per APP-INV-018 (Cross-Module Reference Completeness), this section lists invariants
owned by other modules that this module depends on or interfaces with:

| Invariant    | Owner              | Relationship | Usage in This Module                          |
|--------------|--------------------|--------------|-----------------------------------------------|
| APP-INV-001  | parse-pipeline     | interfaces   | Parsed index consumed by change operations     |
| APP-INV-002  | query-validation   | interfaces   | Validation determinism assumed by oplog records |
| APP-INV-003  | query-validation   | interfaces   | Cross-ref integrity maintained across edits    |
| APP-INV-007  | query-validation   | interfaces   | Diff completeness consumed by change tracking  |
| APP-INV-008  | search-intelligence| interfaces   | RRF scores may appear in context bundles logged |
| APP-INV-009  | parse-pipeline     | interfaces   | Monolith-modular equivalence for re-parse      |
| APP-INV-011  | query-validation   | interfaces   | Check composability for selective validation    |
| APP-INV-012  | search-intelligence| interfaces   | LSI dimensions stable across index updates     |
| APP-INV-015  | parse-pipeline     | interfaces   | Deterministic hashing for content comparison   |
