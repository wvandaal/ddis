---
module: query-validation
domain: validation
maintains: [APP-INV-002, APP-INV-003, APP-INV-007, APP-INV-011]
interfaces: [APP-INV-001, APP-INV-005, APP-INV-006, APP-INV-008, APP-INV-009, APP-INV-015, APP-INV-016]
implements: [APP-ADR-004]
adjacent: [parse-pipeline, search-intelligence, lifecycle-ops]
negative_specs:
  - "Must NOT rely on execution order for validation check correctness"
  - "Must NOT use floating-point comparison for structural diff matching"
  - "Must NOT produce false positives from template references"
---

# Module: Query Validation

This module owns the mechanical validation pipeline and structural diff engine
for the DDIS CLI. It is responsible for deterministic, composable validation
checks that verify spec integrity, cross-reference resolution, and structural
change detection.

The module maintains four invariants governing validation determinism (APP-INV-002),
cross-reference integrity (APP-INV-003), diff completeness (APP-INV-007), and
check composability (APP-INV-011). It interfaces with the parse pipeline for
indexed data access and with lifecycle-ops for transaction-aware validation.

All 12 mechanical checks execute against the SQLite spec index produced by the
parse pipeline. Checks are stateless functions: given a database handle and spec
ID, they return a deterministic `CheckResult`. No check may depend on execution
order, wall-clock time, or random state.

**Invariants referenced from other modules (APP-INV-018 compliance):**
- APP-INV-001: Parse-render round-trip produces byte-identical output (maintained by parse-pipeline)
- APP-INV-005: Context bundles are self-contained for LLM consumption (maintained by search-intelligence)
- APP-INV-006: Transaction state machine: only pending→committed or pending→rolled_back (maintained by lifecycle-ops)
- APP-INV-008: RRF fusion score equals correctly computed weighted sum (maintained by search-intelligence)
- APP-INV-009: Parsing monolith ≡ parsing modules assembled from same source (maintained by parse-pipeline)
- APP-INV-015: Content hashes are deterministic — SHA-256, no salt (maintained by parse-pipeline)
- APP-INV-016: Every invariant with implementation claims has valid Source/Tests paths (maintained by lifecycle-ops)

---

## Invariants Maintained by This Module

**APP-INV-002: Validation Determinism**

*Running the same set of validation checks against the same spec index database MUST produce identical results regardless of execution order, wall-clock time, or host environment.*

```
∀ db, specID, checks:
  Validate(db, specID, checks) = Validate(db, specID, permute(checks))
  ∧ Validate(db, specID, checks, t₁) = Validate(db, specID, checks, t₂)
  where t₁, t₂ are arbitrary wall-clock times
```

Violation scenario: A validation check reads `os.Getenv("TZ")` to determine timezone-sensitive behavior, producing different findings on CI (UTC) versus a developer laptop (EST). The same spec appears to pass in one environment and fail in another.

Validation: Run the full validation suite twice on the same database. Compare CheckResult arrays element-by-element. Any difference violates APP-INV-002. Additionally, run with shuffled check order and verify identical per-check results.

// WHY THIS MATTERS: Non-deterministic validation undermines the trust model. If a RALPH audit agent gets different results than a human reviewer running the same checks, the improvement loop cannot converge.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/validator/validator.go::Validate`
- Source: `internal/validator/checks.go::checkXRefIntegrity`
- Tests: `tests/validate_test.go::TestValidateAllChecks`
- Validates-via: `internal/validator/validator.go::AllChecks`

---

**APP-INV-003: Cross-Reference Integrity**

*Every cross-reference marked as resolved in the spec index MUST point to an element (section, invariant, ADR, gate, glossary entry) that exists in the same spec index.*

```
∀ xref ∈ cross_references WHERE xref.resolved = true:
  ∃ element ∈ (sections ∪ invariants ∪ adrs ∪ quality_gates ∪ glossary)
    : element.id = xref.ref_target ∧ element.spec_id = xref.spec_id
```

Violation scenario: An invariant references "APP-ADR-005" which existed in a previous version but was renumbered to "APP-ADR-006" during a refactor. The cross-reference resolver marks it resolved based on stale data, but the target no longer exists.

Validation: For every resolved cross-reference in the database, execute a lookup query for the target element by type and ID. Any miss violates APP-INV-003. The validation check (`checkXRefIntegrity`) performs exactly this query.

// WHY THIS MATTERS: Broken cross-references silently degrade the specification's navigability. An LLM following a reference to a non-existent element will hallucinate or fail. For humans, broken links erode trust in the document.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/parser/xref.go::ResolveCrossReferences`
- Source: `internal/validator/checks.go::checkXRefIntegrity`
- Tests: `tests/validate_test.go::TestValidateXRefIntegrity`
- Tests: `tests/parser_test.go::TestCrossReferenceResolution`

---

**APP-INV-007: Diff Completeness**

*The structural diff between two spec index snapshots MUST report every element addition, removal, and modification. No change may be silently dropped.*

```
let delta = ComputeDiff(base_db, head_db, base_spec, head_spec)
∀ element e ∈ (base_elements ∪ head_elements):
  changed(e, base, head) ⟹ ∃ change ∈ delta.Changes : change.ElementID = e.id
where changed(e, base, head) ≡
  (e ∈ base ∧ e ∉ head) ∨ (e ∉ base ∧ e ∈ head) ∨
  (e ∈ base ∧ e ∈ head ∧ hash(e, base) ≠ hash(e, head))
```

Violation scenario: An invariant's violation scenario text is updated but the diff algorithm only compares invariant titles, not content hashes. The modification goes unreported, and a RALPH judge comparing versions misses the improvement.

Validation: Create two spec databases with known differences (add an invariant, remove an ADR, modify a section). Run `ComputeDiff`. Verify every known change appears in the diff output. Any missing change violates APP-INV-007.

// WHY THIS MATTERS: The RALPH improvement loop depends on accurate diff output to judge whether an iteration made progress. Silent drops cause the judge to undercount improvements, potentially triggering false convergence.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/diff/diff.go::ComputeDiff`
- Source: `internal/diff/match.go::MatchElements`
- Tests: `tests/diff_test.go::TestDiffSummary`
- Tests: `tests/diff_test.go::TestDiffMonolithVsModular`

---

**APP-INV-011: Check Composability**

*Running a subset S of validation checks MUST produce the same results for those checks as running all checks and filtering to S. Checks are independent and share no mutable state.*

```
∀ S ⊆ AllChecks, ∀ db, specID:
  Validate(db, specID, {CheckIDs: S}).Results
  = filter(Validate(db, specID, {}).Results, λr. r.CheckID ∈ S)
```

Violation scenario: Check 5 (INV-013 ownership) stores intermediate results in a package-level variable that Check 7 (INV-015 decl-def) reads. Running checks 5,7 produces different results than running check 7 alone, because the shared state is unpopulated.

Validation: For each check, run it individually and as part of the full suite. Compare results. Run checks in forward and reverse order. Any result difference violates APP-INV-011.

// WHY THIS MATTERS: The `--checks` flag allows users to run a subset of checks for faster feedback. If checks have hidden dependencies, subset results are unreliable. The RALPH audit agent selectively runs checks relevant to specific improvements.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/validator/validator.go::Validate`
- Source: `internal/validator/validator.go::AllChecks`
- Tests: `tests/validate_test.go::TestValidateSelectiveChecks`

---

## ADR Stubs

### APP-ADR-004: Cobra CLI Framework

#### Problem

The CLI needs a command framework supporting subcommands, flags,
help generation, and shell completion.

#### Decision

Adopt `github.com/spf13/cobra` as the CLI framework. Cobra
provides declarative command trees, automatic flag parsing, persistent flags for
global options (e.g., `--json`), and built-in help/completion generation. The
alternative (raw `flag` + manual dispatch) was rejected for its boilerplate cost
and lack of subcommand support.

#### Consequences

- All commands registered via `rootCmd.AddCommand()` in `internal/cli/root.go`
- Flags declared per-command in `init()` functions
- Global flags (like `--json`) available via persistent flags
- Shell completion generated automatically

> Full ADR body pending Phase 4.6.

---

## Implementation Chapters (Pending)

### Fragment Assembly

> Pending Phase 4.6. Will cover how validation checks discover and assemble
> spec fragments from the SQLite index.

### 12 Validation Checks

> Pending Phase 4.6. Will specify each of the 12 mechanical checks:
> cross-reference integrity, APP-INV-003 falsifiability, APP-INV-006 density,
> APP-INV-009 glossary, APP-INV-013 ownership, APP-INV-014 budget, APP-INV-015 decl-def,
> APP-INV-016 manifest sync, APP-INV-017 negative spec coverage, Gate-1 structural,
> proportional weight, and namespace consistency.

### Structural Diff

> Pending Phase 4.6. Will specify the structural diff algorithm that compares
> two spec index snapshots and produces a complete set of changes.

### BFS Impact Analysis

> Pending Phase 4.6. Will specify the breadth-first impact analysis that
> computes the transitive closure of elements affected by a change.

---

## Negative Specifications

The following constraints define what this module must NOT do. Violations
indicate architectural regression.

**DO NOT** rely on execution order for validation check correctness. Each check is a pure function of (db, specID). Reordering, parallelizing, or selectively running checks must not change individual check results. (Validates APP-INV-011)

**DO NOT** use floating-point comparison for structural diff matching. Element identity is determined by string keys (section paths, invariant IDs, ADR IDs) and content hashes (SHA-256). Floating-point proximity has no role in structural comparison. (Validates APP-INV-007)

**DO NOT** produce false positives from template references. Template patterns (APP-INV-NNN, APP-ADR-NNN, §N.M) appearing in instructional text must be recognized and excluded from cross-reference resolution checks. (Validates APP-INV-003)

**DO NOT** cache check results across invocations. Each call to `Run(db, specID)` must query the database fresh. Stale caches violate APP-INV-002 determinism when the database has changed between calls.

---

## Referenced Invariants from Other Modules

Per APP-INV-018 (Cross-Module Reference Completeness), this section lists invariants
owned by other modules that this module depends on or interfaces with:

| Invariant    | Owner              | Relationship | Usage in This Module                          |
|--------------|--------------------|--------------|-----------------------------------------------|
| APP-INV-001  | parse-pipeline     | interfaces   | Checks read from the round-trip-faithful index |
| APP-INV-005  | search-intelligence| interfaces   | Context bundles validated for self-containment |
| APP-INV-006  | lifecycle-ops      | interfaces   | Transaction state validated pre-commit         |
| APP-INV-008  | search-intelligence| interfaces   | RRF scores verified for formula correctness    |
| APP-INV-009  | parse-pipeline     | interfaces   | Monolith-modular equivalence assumed by checks |
| APP-INV-015  | parse-pipeline     | interfaces   | Content hashes assumed deterministic           |
| APP-INV-016  | lifecycle-ops      | interfaces   | Traceability annotations consumed by checks    |
