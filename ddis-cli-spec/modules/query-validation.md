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

# Query Validation Module

This module specifies the mechanical validation pipeline, structural diff engine, BFS impact analysis, and fragment assembly subsystem of the DDIS CLI. These four subsystems share a common architectural constraint: they are **read-only, deterministic projections** of the spec index. Given the same SQLite database and spec ID, every function in this module produces identical output regardless of execution order, wall-clock time, host environment, or which other functions have been called.

The validation pipeline provides 13 composable checks that verify spec integrity against DDIS structural requirements. The diff engine computes complete structural deltas between two spec index snapshots. The impact analyzer performs bounded BFS over the cross-reference graph. The fragment assembly system extracts and enriches individual spec elements for query and context consumption.

The module maintains four invariants: validation determinism (APP-INV-002), cross-reference integrity (APP-INV-003), diff completeness (APP-INV-007), and check composability (APP-INV-011). It implements one ADR: APP-ADR-004 (Cobra CLI Framework).

**Invariants interfaced from other modules (INV-018 compliance --- restated at point of use):**

- APP-INV-001: Round-Trip Fidelity --- parse then render produces byte-identical output (maintained by parse-pipeline). *Validation checks read from the parsed index; if parsing corrupts content, validation results are meaningless --- garbage in, garbage out.*
- APP-INV-005: Context Self-Containment --- bundles include all 9 intelligence signals (maintained by search-intelligence). *The context command calls QueryTarget from this module to assemble signal 1 (target content); a broken query corrupts every downstream signal.*
- APP-INV-006: Transaction State Machine --- only pending->committed or pending->rolled_back (maintained by lifecycle-ops). *Transaction-aware validation must respect state constraints; validating during a pending transaction could see partial writes.*
- APP-INV-008: RRF Fusion Correctness --- score equals weighted sum across ranking signals (maintained by search-intelligence). *Validation Check 1 (cross-reference integrity) feeds into the authority scoring graph that RRF consumes; broken references produce phantom PageRank nodes.*
- APP-INV-009: Monolith-Modular Equivalence --- parsing monolith produces same index as assembled modules (maintained by parse-pipeline). *Validation checks must produce identical results regardless of input format; the checks operate on the index, not the source files.*
- APP-INV-015: Deterministic Hashing --- SHA-256 with no salt (maintained by parse-pipeline). *The diff engine uses content hashes to detect modifications; non-deterministic hashing would report every element as modified.*
- APP-INV-016: Implementation Traceability --- every invariant with implementation claims has valid Source/Tests/Validates-via paths (maintained by lifecycle-ops). *This module's own invariants carry Implementation Trace annotations that must resolve.*

---

## Invariants

This module maintains four invariants. Each invariant is fully specified with all six components: plain-language statement, semi-formal expression, violation scenario, validation method, WHY THIS MATTERS annotation, and implementation trace.

---

**APP-INV-002: Validation Determinism**

*Running the same set of validation checks against the same spec index database MUST produce identical results regardless of execution order, wall-clock time, or host environment.*

```
FOR ALL db, specID, checks, t1, t2, env1, env2:
  Validate(db, specID, {CheckIDs: checks}, t1, env1).Results
  = Validate(db, specID, {CheckIDs: checks}, t2, env2).Results

AND FOR ALL permutation P of checks:
  Validate(db, specID, {CheckIDs: checks}).Results
  = Validate(db, specID, {CheckIDs: P(checks)}).Results

where each Check.Run(db, specID) is a pure function:
  no os.Getenv(), no time.Now(), no rand.*,
  no package-level mutable state, no file I/O beyond db queries
  (exception: Check 13 reads source files via CodeRoot, but its Applicable()
   returns false when CodeRoot is empty, making it opt-in)
```

Violation scenario: A validation check reads `os.Getenv("TZ")` to determine timezone-sensitive behavior, producing different findings on CI (UTC) versus a developer laptop (EST). The same spec appears to pass in one environment and fail in another. A RALPH audit agent running on a cloud instance gets 12 findings while the developer running locally gets 11 findings. The improvement loop cannot converge because the audit baseline shifts between environments.

Validation: Run the full validation suite twice on the same database with identical inputs. Compare `CheckResult` arrays element-by-element: same `CheckID`, same `Passed`, same `Findings` list (same severity, message, location for each finding), same `Summary`. Any difference violates APP-INV-002. Additionally, run all checks in forward order `[1,2,...,13]` and reverse order `[13,...,2,1]` and verify identical per-check results. Run in two separate processes to confirm no process-level state leaks.

// WHY THIS MATTERS: Non-deterministic validation undermines the trust model of the RALPH improvement loop. If an audit agent gets different results than a human reviewer running the same checks on the same spec, the loop cannot distinguish genuine improvements from environmental noise. Determinism is the prerequisite for convergence.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/validator/validator.go::Validate`
- Source: `internal/validator/validator.go::AllChecks`
- Source: `internal/validator/checks.go::checkXRefIntegrity`
- Tests: `tests/validate_test.go::TestValidateAllChecks`
- Tests: `tests/validate_test.go::TestValidateSelectiveChecks`
- Tests: `tests/validate_test.go::TestValidateJSON`
- Validates-via: `internal/validator/validator.go::Validate` (filter loop is order-independent)

----

**APP-INV-003: Cross-Reference Integrity**

*Every cross-reference marked as resolved in the spec index MUST point to an element (section, invariant, ADR, gate, glossary entry) that exists in the same spec index. Template references (NNN, XXX, N.M patterns) are excluded from error reporting.*

```
FOR ALL xref IN cross_references WHERE xref.resolved = true:
  EXISTS element IN (sections UNION invariants UNION adrs UNION quality_gates UNION glossary_entries):
    element.id = xref.ref_target
    AND element.spec_id = xref.spec_id

AND FOR ALL xref IN cross_references WHERE xref.resolved = false:
  isTemplateRef(xref.ref_target) => severity = info
  NOT isTemplateRef(xref.ref_target) => severity = error

where isTemplateRef(target) =
  lowercase(target) contains "nnn" OR "xxx" OR "n.m"
  OR target IN {"§N.M", "INV-NNN", "ADR-NNN"}
```

Violation scenario: An invariant references "APP-ADR-005" which existed in a previous version but was renumbered to "APP-ADR-006" during a refactor. The cross-reference resolver marks it resolved based on stale data, but the target element no longer exists in the current index. An LLM editor following this reference produces an edit referencing a phantom ADR. The user trusts the edit because the CLI's own index appeared to validate the reference.

Validation: For every resolved cross-reference in the database, execute a lookup query for the target element by type and ID. Any miss violates APP-INV-003. The validation check (`checkXRefIntegrity`, Check 1) performs exactly this query via `storage.GetUnresolvedRefs`. Additionally, insert a known-bad resolved reference (target does not exist), run Check 1, and verify it appears as an error finding. Insert a template reference (`INV-NNN`), run Check 1, and verify it appears as info (not error).

// WHY THIS MATTERS: Broken cross-references silently degrade the specification's navigability and the authority scoring graph. An LLM following a reference to a non-existent element will hallucinate. For the search engine, dangling references produce phantom nodes in PageRank (APP-INV-004 interface), inflating authority scores for elements that do not exist.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/parser/xref.go::ResolveCrossReferences`
- Source: `internal/validator/checks.go::checkXRefIntegrity`
- Source: `internal/validator/checks.go::isTemplateRef`
- Tests: `tests/validate_test.go::TestValidateXRefIntegrity`
- Tests: `tests/parser_test.go::TestCrossReferenceResolution`
- Validates-via: `internal/validator/checks.go::checkXRefIntegrity` (queries storage.GetUnresolvedRefs)

----

**APP-INV-007: Diff Completeness**

*The structural diff between two spec index snapshots MUST report every element addition, removal, and modification. No change may be silently dropped. Identity is determined by string keys and SHA-256 content hashes --- never floating-point comparison.*

```
LET delta = ComputeDiff(base_db, head_db, base_spec, head_spec)
FOR ALL element e IN (base_elements UNION head_elements):
  changed(e, base, head) => EXISTS change IN delta.Changes:
    change.ElementID = e.id
    AND change.ElementType = e.type
    AND change.Action IN {"added", "removed", "modified"}

where changed(e, base, head) =
  (e IN base AND e NOT IN head)                                // removed
  OR (e NOT IN base AND e IN head)                             // added
  OR (e IN base AND e IN head AND hash(e, base) != hash(e, head))  // modified

AND delta.Summary.Added + delta.Summary.Removed + delta.Summary.Modified
    + delta.Summary.Unchanged = |base_elements UNION head_elements|
```

Violation scenario: An invariant's violation scenario text is updated, but the diff algorithm only compares invariant titles (not content hashes). The modification goes unreported. A RALPH judge comparing versions misses the improvement, falsely concluding that the iteration made no progress. The loop converges prematurely because it thinks the score has plateaued.

Validation: Create two spec databases with known differences: (a) add an invariant to head, (b) remove an ADR from head, (c) modify a section's body text (different content hash), (d) rename a section path (fuzzy-matched). Run `ComputeDiff`. Verify: the added invariant appears with `action="added"`, the removed ADR with `action="removed"`, the modified section with `action="modified"`, and the renamed section is matched (not reported as remove+add). Any missing change violates APP-INV-007. Also verify `Summary` counts are consistent: `Added + Removed + Modified + Unchanged = total paired elements`.

// WHY THIS MATTERS: The RALPH improvement loop depends on accurate diff output to judge whether an iteration made progress. Silent drops cause the judge to undercount improvements, potentially triggering false convergence. In the opposite direction, hallucinated changes cause the judge to overcount, masking regressions.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/diff/diff.go::ComputeDiff`
- Source: `internal/diff/diff.go::RenderDiff`
- Source: `internal/diff/match.go::MatchElements`
- Source: `internal/diff/match.go::matchSections`
- Source: `internal/diff/match.go::matchByID`
- Source: `internal/diff/match.go::fuzzyMatchSection`
- Source: `internal/diff/match.go::levenshtein`
- Tests: `tests/diff_test.go::TestDiffIdentical`
- Tests: `tests/diff_test.go::TestDiffSummary`
- Tests: `tests/diff_test.go::TestDiffJSON`
- Tests: `tests/diff_test.go::TestDiffHumanReadable`
- Tests: `tests/diff_test.go::TestDiffMonolithVsModular`
- Validates-via: `internal/diff/match.go::MatchElements` (exhaustive pairing over union of element sets)

----

**APP-INV-011: Check Composability**

*Running a subset S of validation checks MUST produce the same results for those checks as running all checks and filtering to S. Checks are independent pure functions sharing no mutable state.*

```
FOR ALL S SUBSET_OF AllChecks, FOR ALL db, specID:
  Validate(db, specID, {CheckIDs: S}).Results
  = filter(Validate(db, specID, {}).Results, lambda r: r.CheckID IN S)

Equivalently: FOR ALL check C IN AllChecks:
  C.Run(db, specID) is invariant under the presence or absence
  of any other check's execution in the same process.

Structural guarantee:
  - No package-level variables are mutated by any Check.Run()
  - Each Check is a struct with no mutable fields (except Check 13's CodeRoot,
    which is set once before any Run() call and never modified during execution)
  - All intermediate data is local to the Run() function
```

Violation scenario: Check 5 (`checkINV013InvariantOwnership`) stores intermediate results in a package-level variable that Check 7 (`checkINV015DeclDef`) reads. Running checks [5,7] produces different results than running check 7 alone, because the shared state from check 5 is unpopulated when check 7 runs independently. A developer using `--checks 7` to quickly verify declaration-definition consistency gets a false pass because the missing shared state defaults to empty (no mismatches detected).

Validation: For each of the 13 checks, run it individually via `Validate(db, specID, {CheckIDs: [i]})` and as part of the full suite via `Validate(db, specID, {})`. Extract the result for check i from the full suite. Compare: same `Passed`, same `Findings` count, same `Summary`. Additionally, run checks in forward `[1..13]` and reverse `[13..1]` order and verify identical per-check results. Any result difference violates APP-INV-011.

// WHY THIS MATTERS: The `--checks` flag allows users to run a subset of checks for faster feedback during iterative editing. If checks have hidden dependencies, subset results are unreliable. The RALPH audit agent selectively runs checks relevant to specific improvements --- check composability ensures the selective results are trustworthy.

**Confidence:** falsified

**Implementation Trace:**
- Source: `internal/validator/validator.go::Validate`
- Source: `internal/validator/validator.go::AllChecks`
- Tests: `tests/validate_test.go::TestValidateSelectiveChecks`
- Tests: `tests/validate_test.go::TestValidateAllChecks`
- Validates-via: `internal/validator/validator.go::Validate` (filter by CheckIDs is applied independently per check)

----

## Architecture Decision Records

---

### APP-ADR-004: Cobra CLI Framework

#### Problem

The CLI needs a command framework that supports subcommands (`parse`, `render`, `query`, `validate`, `diff`, `impact`, `search`, `context`, `log`, `tx`, `seed`), typed flags, help generation, and shell completion. The framework must route 13 commands with heterogeneous flag sets while keeping registration code concise.

#### Options

A) **Raw `flag` package + manual dispatch** --- Use Go's standard library `flag` package with a hand-written command dispatcher.
- Pros: No external dependency. Full control over argument parsing.
- Cons: No built-in subcommand support; requires manual `os.Args` slicing and a switch statement that grows linearly with command count. No automatic help generation. No shell completion. Flag namespacing requires manual prefixing (e.g., `--validate-checks` vs `--diff-format`). Error messages are generic.

B) **`github.com/spf13/cobra`** --- Declarative command tree with automatic flag parsing, persistent flags, help generation, and shell completion.
- Pros: De facto standard for Go CLIs (used by kubectl, docker, gh, hugo). Subcommand routing is declarative: each command is a `cobra.Command` struct added via `AddCommand()`. Persistent flags (e.g., `--json`) propagate to all subcommands. Built-in help, usage, and shell completion generation. Error handling integrates with `RunE` pattern.
- Cons: External dependency (~5MB in binary). Opinionated about flag parsing (POSIX-style `--flag=value`).

C) **`github.com/urfave/cli`** --- Alternative CLI framework with similar capabilities.
- Pros: Simpler API for small CLIs. Built-in flag completion.
- Cons: Less ecosystem adoption than Cobra. No persistent flag inheritance without manual propagation. Fewer examples and community patterns.

#### Decision

**Option B: Cobra.** The CLI has 13 commands across 4 domains, each with distinct flags. Cobra's declarative command tree maps directly to this structure. Persistent flags enable the `--json` output format flag to be declared once on `rootCmd` and inherited by all subcommands.

The registration pattern is:
```go
func init() {
    rootCmd.AddCommand(parseCmd)
    rootCmd.AddCommand(renderCmd)
    rootCmd.AddCommand(queryCmd)
    rootCmd.AddCommand(validateCmd)
    rootCmd.AddCommand(diffCmd)
    rootCmd.AddCommand(impactCmd)
    rootCmd.AddCommand(logCmd)
    rootCmd.AddCommand(txCmd)
    rootCmd.AddCommand(seedCmd)
    rootCmd.AddCommand(searchCmd)
    rootCmd.AddCommand(contextCmd)
}
```

// WHY NOT raw `flag`? 13 subcommands with heterogeneous flag sets would require ~200 lines of manual dispatch code. Each new command requires editing the dispatcher. Cobra reduces this to one `AddCommand()` call per command.

// WHY NOT `urfave/cli`? Cobra's persistent flag inheritance eliminates the need to declare `--json` on every subcommand individually. With `urfave/cli`, the `--json` flag would need to be declared 13 times or handled via manual flag propagation.

#### Consequences

- All commands registered via `rootCmd.AddCommand()` in `internal/cli/root.go`
- Flags declared per-command in `init()` functions within each command file
- Global flags (e.g., `--json`) available via persistent flags on `rootCmd`
- Shell completion generated automatically via `cobra.Command.GenBashCompletion()` and variants
- Each command file (`parse.go`, `validate.go`, `diff.go`, etc.) is self-contained: command definition, flag declarations, and `RunE` handler
- Exit code 1 for validation failures via `ErrValidationFailed` sentinel error

#### Tests

- `internal/cli/root.go::Execute()` runs the root command
- All 13 commands are registered and respond to `--help`
- `--json` flag produces valid JSON output for all commands that support it

---

## Implementation

### Chapter: The 13 Validation Checks

**Preserves:** APP-INV-002 (Validation Determinism --- each check is a pure function), APP-INV-011 (Check Composability --- checks share no mutable state), APP-INV-003 (Cross-Reference Integrity --- Check 1 enforces this invariant directly).

**Interfaces:** APP-INV-001 (Round-Trip Fidelity --- checks read from the index which must faithfully represent the source), APP-INV-015 (Deterministic Hashing --- content hashes compared by checks must be stable).

The validation pipeline consists of 13 checks, each implementing the `Check` interface:

```go
type Check interface {
    ID() int
    Name() string
    Applicable(sourceType string) bool
    Run(db *sql.DB, specID int64) CheckResult
}
```

`AllChecks()` returns the fixed, ordered list. `Validate()` iterates this list, filters by `Applicable()` and by `ValidateOptions.CheckIDs` (if non-empty), calls `Run()`, and accumulates results into a `Report`. The filter is a simple map lookup --- O(1) per check --- and is order-independent (APP-INV-011).

#### Check Registry

| ID | Name | Struct | Applicable | Validates |
|----|------|--------|------------|-----------|
| 1 | Cross-reference integrity | `checkXRefIntegrity` | all | APP-INV-003 |
| 2 | INV-003: Invariant falsifiability | `checkINV003Falsifiability` | all | DDIS INV-003 |
| 3 | INV-006: Cross-reference density | `checkINV006XRefDensity` | all | DDIS INV-006 |
| 4 | INV-009: Glossary completeness | `checkINV009GlossaryCompleteness` | all | DDIS INV-009 |
| 5 | INV-013: Invariant ownership | `checkINV013InvariantOwnership` | modular | DDIS INV-013 |
| 6 | INV-014: Bundle budget | `checkINV014BundleBudget` | modular | DDIS INV-014 |
| 7 | INV-015: Declaration-definition consistency | `checkINV015DeclDef` | modular | DDIS INV-015 |
| 8 | INV-016: Manifest-spec sync | `checkINV016ManifestSync` | modular | DDIS INV-016 |
| 9 | INV-017: Negative spec coverage | `checkINV017NegSpecCoverage` | all | DDIS INV-017 |
| 10 | Gate-1: Structural conformance | `checkGate1Structural` | all | Gate-1 |
| 11 | Proportional weight | `checkProportionalWeight` | all | balance |
| 12 | Namespace consistency | `checkNamespaceConsistency` | all | counts |
| 13 | Implementation traceability | `checkImplementationTraceability` | CodeRoot != "" | APP-INV-016 |

Checks 5--8 are **modular-only**: `Applicable(sourceType)` returns `true` only when `sourceType == "modular"`. Check 13 is **opt-in**: `Applicable()` returns `true` only when the `CodeRoot` field is set (via the `--code-root` CLI flag).

#### Check 1: Cross-Reference Integrity (APP-INV-003)

Queries `storage.GetUnresolvedRefs(db, specID)` for all cross-references with `resolved = 0`. Categorizes each:
- **Template references** (detected by `isTemplateRef`): severity `info`. Patterns: lowercase target contains `"nnn"`, `"xxx"`, or `"n.m"`, or target equals `"§N.M"`, `"INV-NNN"`, `"ADR-NNN"`.
- **Non-template unresolved**: severity `error`, check fails.

The `isTemplateRef` function prevents false positives from instructional text that uses placeholder identifiers (e.g., "reference §N.M in your spec"). Without this guard, every guidance section would generate spurious errors.

**Complexity:** O(|unresolved_refs|) --- one pass over the unresolved set.

#### Check 2: INV-003 Falsifiability

Queries `storage.ListInvariants(db, specID)`. For each invariant, checks four components: `Statement`, `SemiFormal`, `ViolationScenario`, `ValidationMethod`. Missing components are `warning` severity (not error, because invariants at `falsified` confidence may legitimately lack a semi-formal predicate during early development).

**Component check logic:**
```
FOR EACH invariant inv:
  FOR EACH component IN [statement, semi_formal, violation_scenario, validation_method]:
    IF component == "": emit warning "inv.ID missing component"
```

#### Check 3: INV-006 Cross-Reference Density

Queries all sections and their reference counts via `storage.GetSectionRefCounts(db, specID)`. A section is an **orphan** if it has 0 incoming AND 0 outgoing references. Exempt sections are skipped:
- Paths in the set `{"PART-0", "Glossary", "Appendix-A", "Appendix-B", "Appendix-C", "Preamble"}` or their children
- Headings at level 1 or below (PART-level structural headings)

Orphans are `warning` severity. The check always passes (orphans are informational, not structural failures).

#### Check 4: INV-009 Glossary Completeness

Scans all section `RawText` for bold terms matching `\*\*([A-Z][A-Za-z\s-]{2,40})\*\*`. Counts occurrences. Any bold term appearing >= 3 times across the spec that is NOT in the glossary is a `warning`. The threshold of 3 prevents one-off bold emphasis from triggering false positives.

#### Check 5: INV-013 Invariant Ownership (modular only)

Queries `storage.GetModuleRelationships(db, specID)` for all `maintains` relationships. Counts owners per invariant. Multi-owned invariants (count > 1) are `error` severity. Invariants with no owner (not in any `maintains` list) are `warning`.

#### Check 6: INV-014 Bundle Budget (modular only)

Queries `storage.GetManifest(db, specID)` for the `hard_ceiling_lines` value. Queries `storage.GetSourceFiles(db, specID)` for all non-manifest files. Any file exceeding the ceiling is `error`.

#### Check 7: INV-015 Declaration-Definition Consistency (modular only)

Compares two sets bidirectionally:
1. **Registry set**: invariant IDs from `storage.GetInvariantRegistryEntries`
2. **Definition set**: invariant IDs from `storage.ListInvariants`

In registry but not defined = `error`. Defined but not in registry = `warning`. This bidirectional check catches both stale registry entries and undeclared invariants.

#### Check 8: INV-016 Manifest-Spec Sync (modular only)

Compares module names from `source_files` (where `file_role = "module"`) against module names from the `modules` table. Modules in the table but not backed by source files = `error`. Source files not in the modules table = `warning`.

#### Check 9: INV-017 Negative Spec Coverage

Accumulates negative spec counts per top-level implementation chapter. Uses `findChapterPath` to normalize section paths to their chapter root: `"§4.2.1"` -> `"§4"`, `"Chapter-3/subsection"` -> `"Chapter-3"`, `"§0.5"` -> `""` (skip preamble). Any chapter with < 3 negative specs is `error`.

#### Check 10: Gate-1 Structural Conformance

Verifies two structural requirements:
1. **Required sections** exist: `§0.1`, `§0.5`, `§0.6`, `§0.7` (executive summary, invariants, gates, glossary).
2. **Required element types** are non-empty: invariants, ADRs, quality gates, negative specs, glossary entries, cross-references.

Any missing section or empty element table is `error`.

#### Check 11: Proportional Weight

Finds implementation chapters (top-level `§N` where N >= 1, or `Chapter-N`). Computes mean line count. Chapters deviating > +/-20% from mean are `warning`; deviating > +/-50% are `error`. Fewer than 2 chapters skips the check (nothing to compare).

**Deviation formula:**
```
deviation = (chapter_lines / mean_lines) - 1.0
|deviation| > 0.20: warning
|deviation| > 0.50: error
```

#### Check 12: Namespace Consistency

Scans section `RawText` for range declarations matching `(INV|ADR|Gate)-(\d{1,3})\s+through\s+(?:INV|ADR|Gate)-(\d{1,3})`. Extracts `[lo, hi]` bounds and counts actual elements in that range. Declared count != actual count is `warning`.

#### Check 13: Implementation Traceability (opt-in)

Parses `Implementation Trace` annotations from invariant `RawText` using the regex:
```
^\s*-\s*(Source|Tests|Validates-via):\s*`([^`]+)::(\w+)`
```

For each annotation: (a) verify the file exists at `CodeRoot/FilePath`, (b) scan the file for a function/type declaration matching `FuncName`. For `Tests` annotations, prepends `"Test"` if not already present. Missing files or functions are `error`. Valid annotations are `info`.

**Implementation Trace:**
- Source: `internal/validator/validator.go::Validate`
- Source: `internal/validator/validator.go::AllChecks`
- Source: `internal/validator/validator.go::ParseCheckIDs`
- Source: `internal/validator/checks.go::checkXRefIntegrity`
- Source: `internal/validator/checks.go::checkINV003Falsifiability`
- Source: `internal/validator/checks.go::checkINV006XRefDensity`
- Source: `internal/validator/checks.go::checkINV009GlossaryCompleteness`
- Source: `internal/validator/checks.go::checkINV013InvariantOwnership`
- Source: `internal/validator/checks.go::checkINV014BundleBudget`
- Source: `internal/validator/checks.go::checkINV015DeclDef`
- Source: `internal/validator/checks.go::checkINV016ManifestSync`
- Source: `internal/validator/checks.go::checkINV017NegSpecCoverage`
- Source: `internal/validator/checks.go::checkGate1Structural`
- Source: `internal/validator/checks.go::checkProportionalWeight`
- Source: `internal/validator/checks.go::checkNamespaceConsistency`
- Source: `internal/validator/checks.go::isTemplateRef`
- Source: `internal/validator/traceability.go::checkImplementationTraceability`
- Source: `internal/validator/traceability.go::parseTraceAnnotations`
- Source: `internal/validator/traceability.go::funcExistsInFile`
- Source: `internal/validator/report.go::RenderReport`
- Tests: `tests/validate_test.go::TestValidateAllChecks`
- Tests: `tests/validate_test.go::TestValidateXRefIntegrity`
- Tests: `tests/validate_test.go::TestValidateINV003`
- Tests: `tests/validate_test.go::TestValidateINV017`
- Tests: `tests/validate_test.go::TestValidateGate1`
- Tests: `tests/validate_test.go::TestValidateModularChecks`
- Tests: `tests/validate_test.go::TestValidateSelectiveChecks`
- Tests: `tests/validate_test.go::TestValidateJSON`

---

### Chapter: Structural Diff

**Preserves:** APP-INV-007 (Diff Completeness --- every change reported, no silent drops).

**Interfaces:** APP-INV-015 (Deterministic Hashing --- content hashes are the change detection mechanism).

The structural diff engine compares two spec index snapshots and produces a complete set of element-level changes. It operates on paired elements: each base element is matched to its head counterpart (or marked as removed), and each unmatched head element is marked as added. Change detection uses SHA-256 content hashes --- never floating-point comparison.

#### Element Matching Algorithm

`MatchElements(baseDB, headDB, baseSpec, headSpec)` orchestrates matching across all element types:

```
Algorithm: Exhaustive Element Matching
Input: base and head spec databases
Output: list of MatchPair (ElementType, ElementID, BaseDBID?, HeadDBID?, BaseHash, HeadHash)

1. Match sections via matchSections (two-pass: exact + fuzzy)
2. Match invariants via matchByID (ID-based)
3. Match ADRs via matchByID (ID-based)
4. Match quality gates via matchByID (ID-based)
5. Match glossary entries via matchByID (ID-based)
6. Return concatenated pairs
```

**Section matching** uses a two-pass strategy because section paths can change across versions (e.g., renumbering):

**Pass 1 --- Exact match:** Index both base and head sections by `section_path`. For each base section, if a head section with the same path exists, pair them.

**Pass 2 --- Fuzzy match:** For unmatched base sections, attempt fuzzy matching against unmatched head sections. Two sections fuzzy-match if:
- They share the same parent path (`sectionParent` strips the last `.N` segment)
- AND their titles match via either prefix containment or Levenshtein distance <= 3

The `sectionParent` function: `"§4.2.1"` -> `"§4.2"`, `"§4"` -> `"§"` (root). Paths with `~N` disambiguation suffixes have the suffix stripped before parent extraction.

The `levenshtein` function computes standard edit distance using the two-row dynamic programming approach:
```
levenshtein(a, b):
  prev[j] = j for j in 0..|b|
  for i in 1..|a|:
    curr[0] = i
    for j in 1..|b|:
      cost = 0 if a[i-1] == b[j-1] else 1
      curr[j] = min(curr[j-1]+1, prev[j]+1, prev[j-1]+cost)
    swap(prev, curr)
  return prev[|b|]
```

**Complexity:** O(|a| x |b|) per pair. The fuzzy match iterates over unmatched base x unmatched head, but in practice the vast majority of sections match exactly in pass 1, leaving < 10 sections for fuzzy matching.

**ID-based matching** (`matchByID`) is used for invariants, ADRs, gates, and glossary entries. These element types have canonical IDs (`INV-001`, `ADR-003`, `Gate-1`, glossary term strings) that are stable across versions. The algorithm:

```
Algorithm: ID-Based Element Matching
Input: base IDs (map[string]int64), head IDs (map[string]int64), hash getter function
Output: list of MatchPair

1. For each base ID:
   a. If ID exists in head: pair them, compute hashes for both
   b. If ID not in head: mark as removed
2. For each head ID not in base: mark as added
```

#### Diff Computation

`ComputeDiff` drives the pipeline:
1. Retrieve spec index metadata for base and head
2. Call `MatchElements` to produce the pairing
3. For each pair, determine the change action:
   - `BaseDBID == nil`: added (new in head)
   - `HeadDBID == nil`: removed (gone from head)
   - `BaseHash != HeadHash`: modified (content changed)
   - Otherwise: unchanged
4. Accumulate counts in `DiffSummary`: `{Added, Removed, Modified, Unchanged}`

**Key invariant enforcement (APP-INV-007):** Every element in the union of base and head appears in exactly one `MatchPair`. The matching functions ensure exhaustive coverage: `matchByID` iterates over all base IDs (pairing or marking removed) and then iterates over all head IDs (marking unmatched ones as added). `matchSections` similarly handles all base sections (pass 1 + pass 2 + unmatched = removed) and all head sections (matched or added).

#### Rendering

`RenderDiff` supports JSON (`json.MarshalIndent`) and human-readable formats. The human format groups changes by action (`Added`, `Removed`, `Modified`) with element type and ID annotations.

**Implementation Trace:**
- Source: `internal/diff/diff.go::ComputeDiff`
- Source: `internal/diff/diff.go::RenderDiff`
- Source: `internal/diff/match.go::MatchElements`
- Source: `internal/diff/match.go::matchSections`
- Source: `internal/diff/match.go::matchByID`
- Source: `internal/diff/match.go::fuzzyMatchSection`
- Source: `internal/diff/match.go::sectionParent`
- Source: `internal/diff/match.go::levenshtein`
- Tests: `tests/diff_test.go::TestDiffIdentical`
- Tests: `tests/diff_test.go::TestDiffSummary`
- Tests: `tests/diff_test.go::TestDiffJSON`
- Tests: `tests/diff_test.go::TestDiffHumanReadable`
- Tests: `tests/diff_test.go::TestDiffMonolithVsModular`

---

### Chapter: BFS Impact Analysis

**Preserves:** APP-INV-003 (Cross-Reference Integrity --- impact traversal depends on resolved references).

**Interfaces:** APP-INV-013 (Impact Termination --- BFS visits each node at most once, maintained by lifecycle-ops). The visited set prevents infinite loops on cycles.

The impact analysis subsystem answers two questions: "If I change element X, what else might need updating?" (forward) and "What does element X depend on?" (backward). It performs breadth-first search over the cross-reference graph with a configurable depth bound.

#### Target Parsing

`parseImpactTarget` normalizes user input into (elementType, canonicalID) using four regex patterns:

| Pattern | Regex | Example Input | Canonical Output |
|---------|-------|---------------|-----------------|
| Section | `^§(\d+(?:\.\d+)*)$` | `§4.2` | `("section", "§4.2")` |
| Invariant | `^((?:APP-)?INV-\d{3})$` | `APP-INV-003` | `("invariant", "APP-INV-003")` |
| ADR | `^((?:APP-)?ADR-\d{3})$` | `ADR-002` | `("adr", "ADR-002")` |
| Gate | `^Gate-?((?:M-)?[1-9]\d*)$` | `Gate-1` | `("gate", "Gate-1")` |

Additionally, raw structural paths (`PART-N`, `Chapter-N`, `Appendix-X`) are accepted as section targets.

#### Analysis Algorithm

`Analyze(db, specID, target, opts)` orchestrates BFS:

```
Algorithm: Bounded BFS Impact Analysis
Input: target element ID, MaxDepth (default 2, clamped to [0, 5]), Direction
Output: ImpactResult with list of ImpactNodes

1. Clamp MaxDepth to [0, 5] (safety bound)
2. Set default Direction to "forward" if empty
3. Parse and normalize target via parseImpactTarget
4. Verify target exists in spec index via elementExists
5. Initialize visited set with {target}

6. Switch on Direction:
   a. "forward":  nodes = bfsForward(db, specID, target, MaxDepth, visited)
   b. "backward": nodes = bfsBackward(db, specID, target, MaxDepth, visited)
   c. "both":
      fwd = bfsForward(db, specID, target, MaxDepth, visited)
      visited2 = {target} UNION {n.ElementID for n in fwd}
      bwd = bfsBackward(db, specID, target, MaxDepth, visited2)
      nodes = fwd ++ bwd

7. Return ImpactResult{Target, Direction, MaxDepth, Nodes, TotalCount}
```

**Forward BFS** (`bfsForward`) answers "what is affected if I change this element?" by following **backlinks** --- cross-references that point TO the current node. For each backlink, the source section path is resolved via `resolveSourceID` (queries the section path from `source_section_id`). This traverses the graph in the reverse direction of reference edges: if §4.2 references INV-003, then changing INV-003 **impacts** §4.2.

**Backward BFS** (`bfsBackward`) answers "what does this element depend on?" by following **outgoing references** from the element's containing section. It first resolves the target to a section ID via `resolveSectionID` (tries sections, then invariants, ADRs, and gates tables for the containing `section_id`). Then queries `storage.GetOutgoingRefs` for that section.

#### Cycle Protection

Both BFS functions maintain a `visited` map. A node is added to `visited` immediately upon discovery (before being queued for further exploration). This guarantees each node is visited at most once, preventing infinite loops on cyclic cross-reference graphs (e.g., INV-001 -> ADR-001 -> INV-001).

For the `"both"` direction, the forward pass populates `visited`, and the backward pass starts with `visited2` seeded from both the target and all forward-discovered nodes. This prevents the backward pass from re-discovering nodes already found in the forward pass.

#### Depth Clamping

`MaxDepth` is clamped to `[0, 5]` as a safety bound. Depth 0 means no traversal (only the target). Depth 5 captures transitive dependencies up to 5 hops. For a typical DDIS specification with 200 elements and a cross-reference graph of average degree 3, depth 2 covers the immediate neighborhood (~9 nodes), while depth 5 could theoretically reach ~243 nodes but in practice is bounded by the graph's actual connectivity.

#### Title Resolution

`resolveTitle(db, specID, elementID)` attempts to find a human-readable title for each discovered node. It tries, in order: `sections.title`, `invariants.title`, `adrs.title`, `quality_gates.title`. Returns empty string if not found.

#### Rendering

`RenderImpact` produces JSON or human-readable output. The human format uses indentation proportional to distance (`strings.Repeat("  ", distance)`) and shows the via-path for each node.

**Implementation Trace:**
- Source: `internal/impact/impact.go::Analyze`
- Source: `internal/impact/impact.go::parseImpactTarget`
- Source: `internal/impact/impact.go::bfsForward`
- Source: `internal/impact/impact.go::bfsBackward`
- Source: `internal/impact/impact.go::resolveSourceID`
- Source: `internal/impact/impact.go::resolveSectionID`
- Source: `internal/impact/impact.go::resolveTitle`
- Source: `internal/impact/impact.go::elementExists`
- Source: `internal/impact/impact.go::RenderImpact`
- Tests: `tests/impact_test.go::TestImpactForward`
- Tests: `tests/impact_test.go::TestImpactBackward`
- Tests: `tests/impact_test.go::TestImpactDepthLimit`
- Tests: `tests/impact_test.go::TestImpactBothDirections`
- Tests: `tests/impact_test.go::TestImpactBadTarget`
- Tests: `tests/impact_test.go::TestImpactJSON`

---

### Chapter: Fragment Assembly (Query System)

**Preserves:** APP-INV-003 (Cross-Reference Integrity --- resolved refs in fragments point to existing elements).

**Interfaces:** APP-INV-005 (Context Self-Containment --- BuildContext calls QueryTarget to assemble signal 1), APP-INV-001 (Round-Trip Fidelity --- fragment RawText matches the parsed index content).

The query system extracts individual spec elements from the index and assembles them into enriched `Fragment` structs. This is the foundation for the `query` command and the first signal in context bundles.

#### Target Parsing

`parseTarget` normalizes user input into (FragmentType, canonicalID) using the same four regex patterns as impact analysis:

| FragmentType | Regex Pattern | Example |
|---|---|---|
| `FragmentSection` | `^§(\d+(?:\.\d+)*)$` | `§0.5` -> `("section", "§0.5")` |
| `FragmentInvariant` | `^((?:APP-)?INV-\d{3})$` | `INV-006` -> `("invariant", "INV-006")` |
| `FragmentADR` | `^((?:APP-)?ADR-\d{3})$` | `APP-ADR-003` -> `("adr", "APP-ADR-003")` |
| `FragmentGate` | `^Gate-?((?:M-)?[1-9]\d*)$` | `Gate-1` -> `("gate", "Gate-1")` |

Structural paths (`PART-N`, `Chapter-N`, `Appendix-X`) are accepted as section targets.

#### Fragment Assembly

`QueryTarget(db, specID, target, opts)` orchestrates assembly:

```
Algorithm: Fragment Assembly with Enrichment
Input: target string, QueryOptions{ResolveRefs, IncludeGlossary, Backlinks}
Output: Fragment with type, ID, title, raw_text, line range, section_path + enrichments

1. Parse target -> (FragmentType, canonicalID)
2. Assemble base fragment via assembleFragment:
   - Section: storage.GetSection -> Fragment with SectionPath, RawText, LineStart, LineEnd
   - Invariant: storage.GetInvariant -> Fragment with InvariantID, look up parent section path
   - ADR: storage.GetADR -> Fragment with ADRID, look up parent section path
   - Gate: storage.GetQualityGate -> Fragment with GateID, look up parent section path
   Returns fragment + owning sectionID

3. If ResolveRefs AND sectionID > 0:
   - Query storage.GetOutgoingRefs(db, specID, sectionID)
   - For each ref: build ResolvedRef{RefType, Target, RefText, Resolved}
   - If ref.Resolved: fetch target raw text via fetchRefTargetText

4. If IncludeGlossary:
   - Query all glossary entries
   - For each entry where lowercase(fragment.RawText) contains lowercase(term):
     - Add GlossaryDef{Term, Definition} to fragment

5. If Backlinks:
   - Query storage.GetBacklinks(db, specID, canonicalID)
   - For each backlink: resolve source section path via getSectionByID
   - Add Backlink{SourceSection, SourceLine, RefType, RefText} to fragment

6. Return enriched fragment
```

#### Reference Resolution (`fetchRefTargetText`)

When `ResolveRefs` is true, each outgoing resolved cross-reference is enriched with the target element's raw text. The function dispatches by `refType`:
- `"section"`: queries `sections.raw_text` by section_path
- `"invariant"` or `"app_invariant"`: queries `invariants.raw_text` by invariant_id
- `"adr"` or `"app_adr"`: queries `adrs.raw_text` by adr_id
- `"gate"`: queries `quality_gates.raw_text` by gate_id

Returns empty string on any error (graceful degradation --- a missing target does not fail the query).

#### Fragment Rendering

`RenderFragment` supports three output formats:
- **JSON**: `json.MarshalIndent` of the full Fragment struct
- **Raw**: `f.RawText` verbatim
- **Markdown**: formatted output with header, metadata, raw text, outgoing references (with first 3 lines of target text), glossary terms, and backlinks

#### Fragment Struct

```go
type Fragment struct {
    Type         FragmentType  // section, invariant, adr, gate
    ID           string        // canonical ID (§0.5, INV-003, ADR-002, Gate-1)
    Title        string        // human-readable title
    RawText      string        // full element text from index
    LineStart    int           // 1-indexed start line in source
    LineEnd      int           // 1-indexed end line in source
    SectionPath  string        // containing section path
    ResolvedRefs []ResolvedRef // outgoing references (if ResolveRefs)
    GlossaryDefs []GlossaryDef // matched glossary terms (if IncludeGlossary)
    Backlinks    []Backlink    // incoming references (if Backlinks)
}
```

**Implementation Trace:**
- Source: `internal/query/query.go::QueryTarget`
- Source: `internal/query/query.go::parseTarget`
- Source: `internal/query/query.go::assembleFragment`
- Source: `internal/query/query.go::fetchRefTargetText`
- Source: `internal/query/query.go::getSectionByID`
- Source: `internal/query/fragment.go::Fragment`
- Source: `internal/query/fragment.go::RenderFragment`

---

### Chapter: Validation Report and CLI Integration

**Preserves:** APP-INV-002 (Validation Determinism --- report rendering is deterministic), APP-INV-011 (Check Composability --- report aggregates independently computed check results).

The validation pipeline integrates with the CLI via Cobra (APP-ADR-004). The `validate` command accepts a spec path, parses it, runs applicable checks, and renders a report.

#### Data Structures

```go
type Finding struct {
    CheckID     int      // which check produced this
    CheckName   string   // human-readable check name
    Severity    Severity // "error", "warning", "info"
    Message     string   // what is wrong
    Location    string   // where (section path, line number)
    InvariantID string   // related invariant ID (optional)
}

type CheckResult struct {
    CheckID   int       // check identifier
    CheckName string    // human-readable name
    Passed    bool      // true if no errors
    Findings  []Finding // all findings (including info)
    Summary   string    // one-line summary
}

type Report struct {
    SpecPath    string        // path to the parsed spec
    SourceType  string        // "monolith" or "modular"
    TotalChecks int           // number of checks run
    Passed      int           // checks with Passed=true
    Failed      int           // checks with Passed=false
    Errors      int           // count of error-severity findings
    Warnings    int           // count of warning-severity findings
    Results     []CheckResult // per-check results
}
```

#### ValidateOptions

```go
type ValidateOptions struct {
    CheckIDs []int  // empty = run all applicable checks
    CodeRoot string // path to source code root for Check 13
}
```

`ParseCheckIDs` converts a comma-separated string (e.g., `"1,3,10"`) to `[]int` for the `--checks` flag.

#### Report Rendering

`RenderReport` supports JSON and human-readable formats. The human format shows a header (spec path, source type, totals), followed by per-check results grouped by pass/fail status, with each finding on its own line showing severity, message, and location.

**Implementation Trace:**
- Source: `internal/validator/validator.go::Validate`
- Source: `internal/validator/validator.go::ValidateOptions`
- Source: `internal/validator/validator.go::ParseCheckIDs`
- Source: `internal/validator/report.go::RenderReport`
- Tests: `tests/validate_test.go::TestValidateJSON`

---

## Negative Specifications

These constraints prevent the most likely implementation errors and LLM hallucination patterns for the validation, diff, impact, and query subsystems. Each addresses a failure mode that an LLM, given only the positive specification, would plausibly introduce.

**DO NOT** rely on execution order for validation check correctness. Each check is a pure function of `(db, specID)`. Reordering, parallelizing, or selectively running checks must not change individual check results. No check may read from or write to package-level mutable state. No check may depend on another check having run first. The only shared state is the `*sql.DB` handle, which is read-only during validation. Rationale: the `--checks` flag promises composability (APP-INV-011); hidden ordering dependencies would make subset results unreliable.

**DO NOT** use floating-point comparison for structural diff matching. Element identity in the diff engine is determined by string keys (`section_path`, `invariant_id`, `adr_id`, `gate_id`, glossary `term`) and content hashes (SHA-256 hex strings). The fuzzy section matching uses integer Levenshtein distance (threshold <= 3), not floating-point similarity. Change detection uses string inequality on content hashes, not numerical proximity. Rationale: floating-point comparison introduces platform-dependent behavior (different rounding on ARM vs x86) that would violate APP-INV-007 by making diff results non-deterministic.

**DO NOT** produce false positives from template references. Template patterns (`INV-NNN`, `ADR-NNN`, `§N.M`) appearing in instructional or guidance text must be recognized by `isTemplateRef` and categorized as `info` severity, not `error`. The `isTemplateRef` function checks: lowercase target contains `"nnn"`, `"xxx"`, or `"n.m"`, or target equals one of `"§N.M"`, `"INV-NNN"`, `"ADR-NNN"`. Without this guard, every module's guidance text would generate spurious cross-reference integrity errors. Rationale: false positives erode trust in the validation report and cause users to ignore legitimate errors.

**DO NOT** cache check results across invocations. Each call to `Check.Run(db, specID)` must query the database fresh. No check may store results in struct fields that persist between `Run()` calls. If a check struct has configuration fields (like Check 13's `CodeRoot`), those fields must be set before the first `Run()` call and never modified during execution. Rationale: stale caches violate APP-INV-002 determinism when the database has changed between invocations.

**DO NOT** use 0-indexed ranks in diff matching or impact distance computation. In BFS impact analysis, distance is computed as `parent_distance + 1` where the start node has distance 0 (not traversed) and the first discovered neighbors have distance 1. In diff matching, the Levenshtein threshold is compared with `<=` (not `<`), meaning distance 3 is accepted but distance 4 is rejected. Rationale: off-by-one errors in distance computation cause impact analysis to include or exclude nodes incorrectly, and diff matching to miss or spuriously match renamed sections.

**DO NOT** allow BFS traversal without a visited set. Both `bfsForward` and `bfsBackward` must maintain a visited map that is checked before enqueuing any node. The visited set must include the start node before traversal begins. Without this guard, cyclic cross-reference graphs (e.g., INV-001 -> ADR-001 -> INV-001) cause infinite traversal. The `MaxDepth` bound is a secondary safety measure, not a substitute for cycle protection. Rationale: cycles in cross-reference graphs are legitimate (mutual references between related elements) and must not cause the CLI to hang.

**DO NOT** modify the spec index during validation, diff, impact, or query operations. All four subsystems are read-only projections. No `INSERT`, `UPDATE`, or `DELETE` statement may be executed against the spec database during any operation in this module. The `*sql.DB` handle is shared across checks; a mutating check would violate APP-INV-002 by changing the database state that subsequent checks read. Rationale: read-only guarantees are the foundation of determinism and composability.

**DO NOT** silently drop unmatched elements in the diff engine. Every element in the base set must appear in the output as either matched (unchanged/modified) or removed. Every element in the head set must appear as either matched or added. The `matchByID` function iterates all base IDs (pairing or marking removed) and then all head IDs (marking unmatched as added). The `matchSections` function handles all base sections (exact match, fuzzy match, or removed) and all head sections (matched or added). Rationale: a silently dropped element violates APP-INV-007 and causes the RALPH judge to miss changes.

---

## Verification Prompt

Use this self-check after implementing or modifying the query-validation subsystem.

**Positive checks (DOES the implementation...):**

- DOES `Validate` produce identical results for identical inputs, regardless of check execution order? (APP-INV-002)
- DOES `checkXRefIntegrity` correctly categorize template references as `info` and real broken references as `error`? (APP-INV-003, NEG-QV-003)
- DOES `ComputeDiff` report every added, removed, and modified element when comparing two known-different databases? (APP-INV-007)
- DOES running `Validate(db, specID, {CheckIDs: [i]})` for a single check produce the same result as extracting check i from the full `Validate(db, specID, {})` run? (APP-INV-011)
- DOES `matchSections` use two-pass matching (exact path, then fuzzy with Levenshtein <= 3)? (APP-INV-007)
- DOES `Analyze` clamp `MaxDepth` to `[0, 5]` and initialize the visited set with the start node before BFS? (APP-INV-013 interface)
- DOES `QueryTarget` with `ResolveRefs=true` enrich each resolved reference with target raw text? (APP-INV-005 interface)
- DOES `ParseCheckIDs("1,3,10")` return `[1, 3, 10]` without error? (APP-ADR-004)
- DOES each check struct implement the `Check` interface with a `Run()` method that takes only `(db, specID)` and no other state? (APP-INV-002, APP-INV-011)

**Negative checks (does NOT the implementation...):**

- Does NOT any check read `os.Getenv()`, `time.Now()`, or `rand.*` during `Run()`? (APP-INV-002, NEG-QV-001)
- Does NOT the diff engine use floating-point comparison for element matching or change detection? (NEG-QV-002)
- Does NOT `checkXRefIntegrity` report template references (`INV-NNN`, `§N.M`) as errors? (NEG-QV-003)
- Does NOT any check store mutable state in package-level variables? (APP-INV-011, NEG-QV-004)
- Does NOT `bfsForward` or `bfsBackward` enqueue a node without first checking the visited set? (NEG-QV-006)
- Does NOT any function in this module execute `INSERT`, `UPDATE`, or `DELETE` against the database? (NEG-QV-007)
- Does NOT `MatchElements` silently skip any element in the base or head set? (APP-INV-007, NEG-QV-008)

---

## Referenced Invariants from Other Modules

Per APP-INV-018 (Cross-Module Reference Completeness), this section lists invariants owned by other modules that this module depends on or interfaces with:

| Invariant | Owner | Relationship | Usage in This Module |
|-----------|-------|--------------|------------------------------------------------------|
| APP-INV-001 | parse-pipeline | interfaces | Checks read from the round-trip-faithful index; corrupt parsing invalidates all validation results |
| APP-INV-005 | search-intelligence | interfaces | Context bundles call QueryTarget for signal 1 (target content); fragment assembly must be correct |
| APP-INV-006 | lifecycle-ops | interfaces | Transaction state constrains when validation runs; pending transactions may expose partial writes |
| APP-INV-008 | search-intelligence | interfaces | RRF fusion depends on authority scores computed from the cross-reference graph that Check 1 validates |
| APP-INV-009 | parse-pipeline | interfaces | Monolith-modular equivalence means validation results are format-independent; checks operate on the index |
| APP-INV-013 | lifecycle-ops | interfaces | Impact analysis BFS termination guarantee; visited set prevents infinite loops on cycles |
| APP-INV-015 | parse-pipeline | interfaces | Content hashes used by the diff engine for change detection must be deterministic |
| APP-INV-016 | lifecycle-ops | interfaces | Implementation Traceability annotations consumed by Check 13 must have valid Source/Tests/Validates-via paths |
