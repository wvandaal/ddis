# DDIS: Complete the Bilateral Specification Lifecycle

> This prompt implements Phases 10-14 of the DDIS roadmap.
> Gestalt-optimized: spec-first framing, one demonstration,
> five constraints, DoF-separated planning/execution.

---

## Read These Files First (This Order Matters)

Before doing anything, formalize your understanding of the domain.
Read each file completely. Do not skim.

```
1. docs/reference/progress-review-2026-02-24.md     — the honest gap assessment
2. ddis-cli-spec/constitution/system.md              — 8-tuple state space, 22 state transitions
3. ddis-cli-spec/modules/auto-prompting.md           — the bilateral lifecycle spec (1,932 lines)
4. ddis-cli-spec/modules/code-bridge.md              — annotation system + contradiction detection
5. ddis-cli-spec/modules/workspace-ops.md            — init, multi-spec, task derivation
6. ddis-cli-spec/manifest.yaml                       — invariant registry, module relationships
7. ~/.claude/plans/mutable-sauteeing-lollipop.md     — the full plan (Parts I-X, wave execution)
```

After reading, state what you understand about:
- The four self-reinforcing loops and which ones exist vs don't
- The state monad architecture (`CommandResult` = output + state + guidance)
- The five priority actions from the progress review
- The wave execution order and its dependencies

Do NOT proceed to implementation until you've demonstrated understanding.

---

## The Fundamental Gap

The tool validates its own spec (self-bootstrapping works for the existing 22 commands).
But the bilateral lifecycle — the plan's central thesis — is entirely unimplemented:

```
           discover
    idea ──────────→ spec        ← THESE ARROWS DON'T EXIST YET
                      │  ↑
              refine ↻│  │ absorb  ← NEITHER DO THESE
                      ↓  │
                     impl
              drift ←───→          ← ONLY THIS ONE EXISTS
```

Eight specified commands have no implementation: `discover`, `refine`, `absorb`,
`init`, `spec`, `tasks`, `scan`, `history`. The spec outruns implementation,
creating the exact forward-drift the system is designed to detect.

The goal: close every arrow in that diagram.

---

## The Demonstration: How `ddis drift` Was Built

This is the pattern. Every new command follows this exact lifecycle.
Study it — then replicate it for each new command.

**Step 1 — Spec module written first** (`ddis-modular/modules/drift-management.md`, 599 lines):
- 3 invariants (INV-021 detection completeness, INV-022 reconciliation monotonicity, INV-023 brownfield convergence)
- 3 ADRs (ADR-012 drift-as-first-class, ADR-013 planned divergence, ADR-014 brownfield-via-skeleton)
- Drift formula: `impl_drift = |unspecified| + |unimplemented| + 2*|contradictions|`
- Quality breakdown: correctness, depth, coherence sub-scores
- Verification prompt with positive and negative checks

**Step 2 — Go package implemented** (`internal/drift/`, 821 LOC, 4 files):
- `drift.go` — analysis engine (measurement + scoring)
- `classify.go` — categorization (direction/severity/intentionality)
- `remediate.go` — next-best remediation package generation
- `render.go` — human-readable + JSON output

**Step 3 — CLI command wired** (`internal/cli/drift.go`, 114 LOC):
- Cobra subcommand with `--report`, `--json`, `--intent` flags
- Calls internal package, renders output

**Step 4 — Tests written** (`tests/drift_test.go`, 466 LOC):
- Classification correctness, remediation ordering, rendering format

**Step 5 — Self-validated**:
```bash
ddis parse ddis-cli-spec/manifest.yaml -o /tmp/cli.db
ddis validate /tmp/cli.db --json          # must pass
ddis drift /tmp/cli.db --report           # must show 0 or decreasing drift
ddis coverage /tmp/cli.db                 # domain coverage must stay ≥ 99%
go test ./...                             # all tests must pass
```

**Step 6 — Dog-fooded on own spec**:
- Found parser bug: `Violation scenario (qualifier):` format not matched by ViolationRe
- Fixed the bug. Re-ran. Drift = 0.
- The tool improved itself by being used on itself.

Step 6 is the most important. If a new command can't operate on its own spec or
improve its own tooling, the design is wrong.

---

## Five Priority Actions

These are ordered by leverage. Do them in this order.

### 1. Fix the ADR parser bug (5 minutes, eliminates 29/31 coverage gaps)

In `internal/parser/adrs.go` line ~173:
```go
current.ChosenOption = "Option " + m[1]  // Always ~9 chars
```
The `ChosenOption` field stores only the label, not the rationale.
`WeakScore` threshold is 20 chars. Result: `9/20 = 0.45` for ALL ADRs.

Fix: capture the full decision section content into `ChosenOption`.
Add `ChosenLabel` field for the option identifier.
Update `WeakScore` thresholds in `internal/exemplar/gaps.go`.
Verify: `ddis coverage` should show 0 WEAK `chosen_option` gaps.

### 2. Implement `ddis scan` + annotate existing code

New package: `internal/annotate/` (~800-1200 LOC).
Grammar: `<comment-marker> ddis:<verb> <target>` (see code-bridge.md §CB.4).
14+ language families. 8 verbs: maintains, implements, interfaces, tests,
validates-via, postcondition, relates-to, satisfies.

Then annotate ALL 22 existing command files + key internal packages with
`// ddis:maintains`, `// ddis:implements`, `// ddis:tests` annotations.
This makes spec-code drift measurable for the first time.

Verify: `ddis scan ddis-cli/ --spec /tmp/cli.db --verify` shows correspondence.

### 3. Implement `ddis init` + `ddis tasks`

`ddis init`: creates manifest template, SQLite DB, JSONL event log, discovery
directory, `.gitignore` entries, basic constitution template.

`ddis tasks --from-discovery <path>`: generates implementation tasks from
discovery artifact maps using the 8 derivation rules (see workspace-ops.md §WO.7).

Dog-food: feed the plan's own artifact map through `ddis tasks`. If it can't
reproduce the existing beads, the derivation rules are incomplete.

### 4. Implement `ddis refine` (minimal viable version)

Start with: `ddis refine audit --prompt-only` generates an audit prompt from
drift report + validation results + coverage gaps.

Then: use it to fix the parent spec's 7 unresolved cross-references.
This is the first real dog-fooding of the bilateral lifecycle.

Verify: parent spec goes from 9/12 to 10/12 checks passing.

### 5. Implement `ddis discover` + `ddis absorb`

These are the hardest. They close the bilateral lifecycle.

`ddis discover`: loads context bundle, thread topology, infers thread from
conversation content (not declared), returns `CommandResult(output, state, guidance)`.

`ddis absorb ddis-cli/ --against /tmp/cli.db`: scans code, generates draft spec,
reconciles against existing spec. Outputs gaps in BOTH directions.

Dog-food: `ddis absorb` on the ddis-cli source should produce a draft resembling
the existing ddis-cli-spec. The delta measures what human discovery added.

---

## Quality Invariant

One constraint that subsumes all others:

**After every change, the tool must be able to validate itself:**
```bash
ddis parse ddis-cli-spec/manifest.yaml -o /tmp/cli.db && \
ddis validate /tmp/cli.db --json && \
ddis drift /tmp/cli.db --report && \
ddis coverage /tmp/cli.db && \
go test ./...
```

If any of these regress, stop and fix before proceeding.
Drift must not increase. Coverage must not decrease. Tests must pass.

---

## Wave Execution

The plan (Part VI) defines 5 waves. Respect the dependencies.

**Wave 1** (parallel, all independent — fix foundations):
- ADR parser bug fix (Action 1)
- Progress circular deps fix (`internal/progress/progress.go` — only `implements` edges, not `interfaces`)
- Parent spec 7 unresolved cross-refs (fix in `ddis-modular/` modules)

**Wave 2** (depends on Wave 1 — build the bridge):
- `ddis scan` + `internal/annotate/` (Action 2)
- Contradiction detection Tier 1 (`internal/contradiction/` — graph-based predicate analysis)
- Event sourcing: JSONL on each `ddis parse`, `ddis history` command
- Intent drift LSI replacement (`internal/drift/intent.go` — composite scoring)

**Wave 3** (depends on Wave 2 — workspace + tasks):
- `ddis init` + `ddis spec add/list` (Action 3)
- `ddis tasks --from-discovery` (Action 3)
- Classification → actionable (affects `Remediate()` priority)
- Test coverage for all Wave 1-2 packages

**Wave 4** (depends on Wave 3 — the bilateral lifecycle):
- `ddis refine` full engine (Action 4)
- `ddis discover` full engine (Action 5)
- `ddis absorb` with `--against` reconciliation (Action 5)
- Auto-prompting spec module updated with implementation experience

**Wave 5** (final — adoption surface):
- README, install script, tutorial
- `ddis doctor` — integration diagnostics
- `ddis-workflow` skill auto-derived from prompt templates

**Phase gate between each wave:**
```bash
ddis parse ddis-cli-spec/manifest.yaml -o /tmp/cli.db
ddis validate /tmp/cli.db --json          # more checks pass than before
ddis drift /tmp/cli.db --report           # drift ≤ previous wave
ddis coverage /tmp/cli.db                 # coverage ≥ previous wave
go test ./...                             # all pass
```

---

## The Meta-Constraint: Dog-Food Everything

Every new capability is tested on the system's own artifacts first:
- `ddis scan` scans the ddis-cli source code
- `ddis refine` improves the ddis-modular spec
- `ddis discover` runs against a real feature exploration
- `ddis absorb` reconciles ddis-cli code against ddis-cli-spec
- `ddis tasks` generates tasks from the plan's artifact map
- `ddis init` bootstraps a fresh workspace and validates it
- `ddis history` shows the evolution of the DDIS spec itself

If a command can't operate on itself, the design is wrong.

---

## What NOT To Do

- Do NOT rewrite existing working commands. They are stable.
- Do NOT add MCP support. It was explicitly descoped.
- Do NOT use `--json-schema` with Claude. It causes StructuredOutput interference.
- Do NOT use `replace_all` on partial strings like `ADR-015` — it matches `APP-ADR-015`.
- Do NOT put `**INV-NNN:**` patterns in code blocks — the parser extracts them as real invariants. Use non-numeric IDs (INV-XYZ) for fictional examples.
- Do NOT implement Tier 2 (Z3 SMT) before Tier 1 (graph-based) proves what contradictions actually occur.

---

## Key Architecture References

- **ParseModularSpec signature**: `ParseModularSpec(manifestPath string, db storage.DB)` — path FIRST
- **source_files column**: `raw_text` NOT `content`
- **file_role values**: monolith, manifest, system_constitution, domain_constitution, deep_context, module
- **State monad return type**: `CommandResult { Output string, State StateSnapshot, Guidance Guidance }`
- **k* budget function**: `k_star_eff(depth) = max(3, 12 - floor(depth / 5))`
- **Drift formula**: `impl_drift = |unspecified| + |unimplemented| + 2*|contradictions|`

---

## Success Criteria

The project earns an A when:
1. All four arrows in the bilateral lifecycle diagram have working implementations
2. `ddis absorb ddis-cli/ --against cli.db` produces a meaningful reconciliation
3. `ddis refine` has been used to improve at least one spec module
4. `ddis discover` has been used for at least one real feature exploration
5. Every Go file in ddis-cli/ has `// ddis:` annotations and `ddis scan` verifies them
6. `ddis validate` passes 12/12 checks on the CLI spec (including fixing Check 11)
7. Coverage = 100% (all invariants, all ADRs, zero WEAK scores)
8. The tool can bootstrap a new DDIS project from scratch via `ddis init`
