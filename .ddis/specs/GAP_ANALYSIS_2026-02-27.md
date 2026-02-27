# DDIS Comprehensive Gap Analysis & Triage Report

**Date**: 2026-02-27
**Method**: DDIS discovery workflow (ddis search → ddis query → code inspection → formal analysis)
**Current State**: 16/17 checks pass, 100% coverage (55/55 INV, 42/42 ADR), drift=0

---

## Part 0: Agent Feedback Validation Summary

The agent's feedback is **substantively correct on all 5 points**, with varying severity. I uncovered a **6th issue (live bug)** and a **7th issue (.ddis/ directory tracking)** during investigation. Below is the formal analysis.

---

## GAP 1: Bootstrapping Problem (Agent Point #1)

### Severity: **P1 — High** (blocks new project onramp)

### What the Agent Reported
> When starting a new spec, you need files to exist before `ddis parse` works. But `ddis discover crystallize` needs a parsed DB to write into. And `ddis skeleton` creates a fresh project, not modules from an existing manifest.

### Formal Analysis

The DDIS lifecycle has a **dependency cycle** in its initialization phase:

```
manifest.yaml (declares modules) → module files (must exist) → ddis parse (requires files) → index.db (required for discover/crystallize)
```

This is a **fixed-point initialization problem**. The existing tools address two distinct use-cases but miss the middle:

| Starting Point | Existing Tool | Gap |
|---|---|---|
| Nothing → full scaffold | `ddis skeleton` | Works |
| Existing files → DB | `ddis parse` | Works |
| manifest.yaml (no module files) → stub files | **NOTHING** | **GAP** |
| DB exists → crystallize into module | `ddis discover crystallize` | Works (requires module file exists) |
| Empty dir → workspace infrastructure | `ddis init` | Works |

**What `ddis skeleton` does**: Generates the ENTIRE structure (manifest + constitution + all modules) from `--domain` flags. It's **generative-complete** but requires you to know all domains upfront.

**What `ddis init` does**: Creates workspace infrastructure (`.ddis/`, event streams, manifest template) but NOT module files. The generated `modules: {}` is empty.

**The missing morphism**: There's no `manifest.yaml → module stub files` operation. If you hand-author a manifest (or receive one from `ddis init --workspace` and then edit the manifest to declare modules), there's no way to generate the corresponding `.md` files.

### Spec Grounding

- **APP-ADR-026** mandates: "`ddis init` creates the complete project scaffolding in one command. `ddis init && ddis parse manifest.yaml -o index.db && ddis validate index.db` succeeds on the first try."
- **APP-INV-036** (Human Format Transparency): Generated content must be indistinguishable from hand-authored.

APP-ADR-026 is satisfied for the `ddis init` flow (workspace → parse → validate works). But the flow `edit manifest → need stubs → parse` is unsupported.

### Recommendation

**Add `ddis manifest scaffold` subcommand** that:
1. Parses `manifest.yaml`
2. For each module declared in `modules:` whose `.file` doesn't exist on disk
3. Generates a stub `.md` file with correct YAML frontmatter (module, domain, maintains, implements, adjacent, negative_specs) and placeholder content (one template INV per `maintains` entry, one template ADR per `implements` entry)
4. Generated stubs pass Level 1 validation

This is the **bilateral dual** of `ddis manifest sync` (which goes files → manifest). The scaffold operation goes manifest → files. Together they form a bijection.

### Complexity: **Medium** — ~200 LOC, reuses `skeleton/templates.go` formatting

---

## GAP 2: Parser Silent Failures (Agent Point #2)

### Severity: **P0 — Critical** (correctness violation)

### What the Agent Reported
> The invariants showed 0% coverage despite having substantive content. The issue was that `**Semi-formal:**` (bold label) didn't match what the parser expects. There was no warning saying "found APP-INV-001 but couldn't parse its semi_formal field."

### Formal Analysis

The parser (`internal/parser/invariants.go`) implements a **6-state finite automaton**:

```
idle → headerSeen → statementSeen → inCodeBlock → codeDone → afterCode
```

**Critical silent failure at line 59-62**:
```go
if m := InvStatementRe.FindStringSubmatch(trimmed); m != nil {
    current.Statement = m[1]
    state = statementSeen
} else {
    state = idle  // SILENTLY DISCARDS ENTIRE INVARIANT
}
```

When the statement doesn't match `*italic text*` (e.g., it's plain text, bold, or a different format), the **entire invariant is silently abandoned**. No log, no warning, no counter. The invariant simply doesn't appear in the DB.

**Additional silent failure paths**:
- `Violation scenario:` must match exactly `^Violation scenario:\s*(.+)$` — case-sensitive, no alternatives
- `Validation:` must match exactly `^Validation:\s*(.+)$`
- `// WHY THIS MATTERS:` must match exactly `^//\s*WHY THIS MATTERS:\s*(.+)$`
- The `semi_formal` field requires a code fence (`` ``` ``) — no fallback for `**Semi-formal:**` bold label format

**Evidence of intent without execution**: The function `isInvariantComplete()` (line 212-215) exists but is **never called anywhere in the codebase**. This is a dead function that was clearly intended to provide completeness validation but was never integrated.

**Impact quantification**: In the current CLI spec, all 55 invariants happen to be correctly formatted, so 0 are silently dropped. But for ANY new spec author, this is a trap: content exists but the DB says 0% coverage, with no explanation.

### Spec Grounding

- **APP-INV-046** (Error Recovery Guidance): "For every actionable error, the CLI must emit a recovery hint."
- **APP-INV-002** (Validation Determinism): "Two runs of `ddis validate` on the same spec must produce identical outputs."

The silent discard violates APP-INV-046 — a parse that silently drops content IS an actionable error that should have guidance. It also creates a confusing interaction with APP-INV-002: validation is deterministic but the user can't understand WHY coverage is 0% because the parser gave no explanation.

### Recommendation

**Add parser diagnostic emission** via a new `ParseDiagnostics` struct returned alongside the parse result:

1. When an invariant header matches but the statement doesn't match `*...*`, emit: `WARNING: {inv_id}: found header but statement not in italic format (*...*) at line {N}. Content not extracted.`
2. When an invariant is extracted but `semi_formal` is empty, emit: `WARNING: {inv_id}: no code fence (```) found for semi_formal field.`
3. When `violation_scenario` or `validation_method` is empty after extraction, emit: `WARNING: {inv_id}: missing {field_name} (expected format: "Violation scenario: ..." or "Validation: ...")`
4. Integrate `isInvariantComplete()` as a post-extraction filter that emits diagnostics

### Complexity: **Low-Medium** — ~100 LOC, mostly adding `fmt.Fprintf(os.Stderr, ...)` at failure points

---

## GAP 3: Crystallize Module Auto-Detection (Agent Point #3)

### Severity: **P2 — Medium** (ergonomics, not correctness)

### What the Agent Reported
> If I'm crystallizing APP-INV-007 and the manifest says `query-validation` owns it, the CLI should auto-target the correct module.

### Formal Analysis

The manifest `modules.*.maintains` field creates a **surjection** from invariant IDs to module names:

```
maintains: APP-INV-007 → query-validation
maintains: APP-INV-022 → auto-prompting
...
```

Combined with the `invariant_registry` which also maps `{inv_id → owner, domain}`, this is fully deterministic. Given any invariant ID, the owning module is unambiguous.

Currently, `crystallize` at line 74 requires `--module` explicitly:
```go
if crystallizeModule == "" {
    return fmt.Errorf("--module is required")
}
```

But the JSON input already contains `owner` and `domain` fields (lines 46-48), AND the manifest can be queried for `modules[*].maintains` to find the match.

### Recommendation

**Auto-detect module from invariant ID** when `--module` is omitted:
1. Parse manifest
2. For invariant type: search `modules[*].maintains` for the input ID
3. For ADR type: search `modules[*].implements` for the input ID
4. If exactly one match: use it (with log message showing auto-detection)
5. If zero or multiple matches: require `--module` as fallback

### Complexity: **Low** — ~30 LOC in `runCrystallize()`

---

## GAP 4: Patch Replace-All (Agent Point #4)

### Severity: **P3 — Low** (deliberate design, not a bug)

### What the Agent Reported
> When doing bulk renames (INV-001 → APP-INV-001 across many files), `ddis patch` can only replace one unique occurrence. A `--replace-all` flag would be valuable.

### Formal Analysis

The single-occurrence enforcement at `patch.go:174-176` and `patch.go:245-246` is **deliberate safety design**:

```go
if count > 1 {
    return fmt.Errorf("text found %d times in ... — provide more context to make it unique", count)
}
```

This is the **correctness-first principle**: in a spec, replacing text in the wrong location can change semantic meaning. The single-occurrence constraint is essentially a **uniqueness proof** that the replacement is unambiguous.

However, for **mechanical renames** (e.g., `INV-001` → `APP-INV-001`), the uniqueness constraint is the wrong abstraction. The intent is "every occurrence should change," and forcing N separate `ddis patch` calls with increasing context is busywork.

### Spec Grounding

- **APP-INV-016** (Oplog Append-Only): All spec edits must flow through oplog.
- **APP-INV-001** (Round-Trip Fidelity): `render(parse(S)) = S`

A `--replace-all` that logs all replacements to the oplog would satisfy APP-INV-016. The fidelity concern is about accidental replacement, which `--replace-all` explicitly opts into.

### Recommendation

**Add `ddis rename` subcommand** (NOT `--replace-all` on patch) that:
1. Takes `--from` and `--to` arguments
2. Operates across ALL module files in the manifest
3. Shows a full diff preview (like `--dry-run`)
4. Requires `--confirm` flag to execute
5. Logs a single oplog entry with all affected files and line counts

This separates the "rename" use-case (mechanical, high-arity) from "patch" (surgical, single-occurrence).

### Complexity: **Medium** — ~150 LOC for the new command

---

## GAP 5: DB Path Ergonomics (Agent Point #5)

### Severity: **P2 — Medium** (usability trap)

### What the Agent Reported
> Some commands need the db path as a positional argument, some discover it automatically, and sometimes the same command works differently depending on context.

### Formal Analysis

I catalogued ALL 33 commands. The actual patterns are:

| Pattern | Commands | Count |
|---|---|---|
| `[index.db]` optional positional, FindDB fallback | validate, coverage, drift, search, query, context, exemplar, ... | **22** |
| No DB arg (workspace/utility) | init, version, log | **3** |
| Subcommand-specific explicit args | tx (begin/commit/rollback/list/show) | **1** |
| Flag or multi-resolution | refine (`--spec` / positional / FindDB) | **1** |
| Hardcoded fallback paths | manifest sync (tries `manifest.yaml`, `ddis-cli-spec/manifest.yaml`) | **1** |

**The 22-command majority IS consistent** — they all use `FindDB()` which globs `*.ddis.db` in CWD. The agent's confusion likely stems from:

1. **Argument ordering trap**: For `search` and `query`, the DB is the SECOND positional arg (`search <query> [db]`, `query <target> [db]`). But for all other commands, it's the first. Passing `ddis search mydb.db "query"` treats the DB path as the query string.

2. **Silent wrong-DB creation**: When the DB path argument is misplaced, `storage.Open()` creates a NEW SQLite database from whatever string is passed (because modernc.org/sqlite auto-creates). The schema is applied to the empty DB, and `GetFirstSpecID` returns "no rows" — an error that says "no spec found" when the real problem is "you opened the wrong file."

3. **The `tx` command outlier**: `tx begin <db> "description"` requires the DB as first arg, contradicting the 22-command pattern.

### Live Bug Demonstration

I reproduced this during investigation:
```
ddis search manifest.ddis.db "bootstrapping"    # FAILS: "no spec found"
ddis search "bootstrapping" manifest.ddis.db    # WORKS: correct results
```

The first form silently creates a file called `bootstrapping` (!) as a SQLite database, opens it, finds it empty, and reports "no spec found." This is a **correctness bug** masked as a usability issue.

### Recommendation

Two fixes:

**Fix A (immediate, low-effort)**: Add file-existence validation in `storage.Open()`:
```go
if _, err := os.Stat(dbPath); os.IsNotExist(err) {
    return nil, fmt.Errorf("database file not found: %s\nTip: ddis parse manifest.yaml", dbPath)
}
```
This prevents silent creation of wrong-name databases.

**Fix B (ergonomic, medium-effort)**: Normalize all commands to accept `--db` flag as primary, with positional as fallback:
```
ddis search "query" --db index.db
ddis query APP-INV-001 --db index.db
ddis validate --db index.db
```
This eliminates positional ambiguity entirely while preserving backward compatibility.

### Complexity: Fix A: **Trivial** (~5 LOC), Fix B: **Medium** (~50 LOC across 22 commands)

---

## GAP 6: `.ddis/` Directory Tracking (User Observation)

### Severity: **P1 — High** (data integrity)

### What Was Noted
> Omitting the `.ddis/` directory is WRONG. The canonical data record for the entire DDIS state for a project is the JSONL logs for events, oplog, and spec edits.

### Formal Analysis

This is **correct and spec-mandated**:

**APP-INV-048** (Event Stream VCS Primacy):
> *Event stream JSONL files are primary data artifacts, tracked in version control. They must never be gitignored, and init must create them with spec-conformant names (stream-N.jsonl).*
> Semi-formal: `forall ws in Workspaces: .ddis/events/stream-{1,2,3}.jsonl in VCS(ws) AND NOT in .gitignore(ws)`

**APP-INV-020** (Event Stream Append-Only):
> *The JSONL event stream is strictly append-only... The stream survives database recreation.*

The JSONL event streams at `.ddis/events/stream-{1,2,3}.jsonl` and `.ddis/events/threads.jsonl` are the **temporal backbone** — they survive `ddis parse --force` because they exist outside the SQLite DB. The `.gitignore` for DDIS projects correctly tracks these:

```
# Current .gitignore status:
.ddis/events/stream-1.jsonl     TRACKED
.ddis/events/stream-2.jsonl     TRACKED
.ddis/events/stream-3.jsonl     TRACKED
.ddis/events/threads.jsonl      TRACKED
```

However, the `.ddis/index.db` and `manifest.ddis.db` are **derived artifacts** (rebuilt by `ddis parse`). These SHOULD be gitignored.

### Current State

The existing `.gitignore` correctly handles this:
- `.ddis/events/*.jsonl` — TRACKED (correct)
- `.ddis/index.db` — gitignored per init template (correct)
- `manifest.ddis.db` — gitignored at project root (correct)

**The agent's feedback implicitly suggests that some new project templates or documentation might recommend gitignoring `.ddis/` entirely**, which would violate APP-INV-048. This needs explicit documentation and validation.

### Recommendation

**Add a validation check** (Check 18) that verifies:
1. `.ddis/events/` directory exists
2. Stream files are not in `.gitignore`
3. `index.db` IS in `.gitignore` (derived artifact should not be tracked)

This is the mechanical enforcement of APP-INV-048's semi-formal property.

### Complexity: **Low** — ~40 LOC in `internal/validator/`

---

## GAP 7: Stale Binary / Build Ergonomics (Discovered During Investigation)

### Severity: **P2 — Medium**

During investigation, I initially thought `GetFirstSpecID` was returning "no rows" due to a code bug. After extensive debugging, I discovered the binary at `ddis-cli/bin/ddis` was compiled at 12:07 but source had changed since. Running `make build` fixed the binary, but the argument-ordering issue (Gap 5) was the actual root cause of the "no spec found" error.

This reveals a **developer experience gap**: when working on the DDIS CLI itself, there's no mechanism to detect that the installed binary is stale relative to source changes. The `ddis version` output (`dev (unknown) built unknown`) doesn't help.

### Recommendation

Add a `--check-build` flag to `ddis version` that compares binary timestamp against source file mtimes, or improve the Makefile to install to PATH automatically on build.

### Complexity: **Low** — ~20 LOC

---

## Triage Priority Matrix

| Gap | Severity | Effort | Impact | Priority Score | Recommendation |
|-----|----------|--------|--------|----------------|----------------|
| **G2**: Parser silent failures | P0 | Low-Med | Correctness | **10** | Implement first |
| **G5a**: DB silent creation bug | P1 | Trivial | Correctness | **9** | Implement with G2 |
| **G1**: Bootstrapping (manifest scaffold) | P1 | Medium | Onramp | **8** | Implement second |
| **G6**: `.ddis/` VCS validation check | P1 | Low | Data integrity | **7** | Implement with G1 |
| **G3**: Crystallize auto-detect module | P2 | Low | Ergonomics | **5** | Quick win |
| **G5b**: Normalize DB arg to `--db` flag | P2 | Medium | Ergonomics | **4** | Next iteration |
| **G4**: `ddis rename` command | P3 | Medium | Convenience | **3** | Backlog |
| **G7**: Stale binary detection | P2 | Low | DX | **2** | Backlog |

---

## Cross-Cutting Observations

### What the Agent Got Right (Validated by Code Inspection)

1. **"Parse/validate/coverage/drift loop is genuinely powerful"** — Confirmed. 17 mechanical checks, tiered consistency (Graph/SAT/Heuristic/SMT/LLM), 100% coverage tracking. This IS the killer feature.

2. **"ddis patch — Much safer than raw file editing"** — Confirmed. The single-occurrence constraint is a genuine safety mechanism, not a limitation. The oplog audit trail is real.

3. **"ddis exemplar — invaluable"** — Confirmed. Corpus-derived formatting guidance using `WeakScore` quality assessment. This is the bilateral lifecycle's "format oracle."

4. **"RALPH loop is sound architecture"** — Confirmed. `audit → plan → apply → judge` with drift monotonicity enforcement. Formally, this is a **convergent fixpoint iteration** — each cycle must decrease drift or the judge rejects.

### What the Agent Got Wrong or Incomplete

1. **"ddis skeleton creates a fresh project, not modules from an existing manifest"** — This is true but incomplete. The agent didn't discover `ddis init` (which creates workspace infrastructure) as a separate operation. The gap is specifically between `init` (creates manifest template) and `parse` (requires module files), not between `skeleton` and parse.

2. **"The bootstrapping problem"** — The agent frames this as chicken-and-egg, but it's actually a **missing morphism** in a otherwise well-structured category. The operations `skeleton → (manifest + modules)`, `parse → DB`, `crystallize → module` all exist. The missing arrow is `manifest → module stubs`.

### Formal Properties Affected

| Property | Status | Gap Impact |
|---|---|---|
| APP-INV-046 (Error Recovery) | **Partially violated** | G2: Parser gives no guidance on silent failures |
| APP-INV-048 (VCS Primacy) | **Unenforced** | G6: No mechanical check validates VCS tracking |
| APP-INV-036 (Format Transparency) | **At risk** | G2: Users can't discover the format requirements |
| APP-ADR-026 (Full Workspace Init) | **Satisfied but incomplete** | G1: Post-init→pre-parse gap |
| APP-ADR-028 (Progressive Validation) | **Working** | No impact |
| APP-INV-002 (Validation Determinism) | **Working** | No impact |

---

## Summary

The agent's feedback identifies **real, substantive gaps** in the DDIS CLI's authoring onramp. The parse/validate/coverage/drift loop is genuinely excellent — the issues are all in the **content ingestion** phase (getting spec content into the correct format and the correct place). The 3 highest-priority items are:

1. **Parser diagnostics** (G2) — silent failures violate the error recovery guidance invariant
2. **DB path validation** (G5a) — SQLite auto-creation masks argument ordering errors
3. **Manifest scaffold** (G1) — missing morphism in the initialization category

All three are fixable with moderate effort and would dramatically improve the new-project experience.

---

## Code Evidence Summary

### Files Inspected
- `internal/storage/queries.go` — GetFirstSpecID, GetLatestSpecID (line 27-61)
- `internal/storage/db.go` — Open(), migrateSchema() (line 18-79)
- `internal/storage/schema.go` — 30 tables, 476 lines of DDL
- `internal/parser/invariants.go` — ExtractInvariants 6-state FSM (line 10-216)
- `internal/parser/patterns.go` — 37 compiled regex patterns (line 1-114)
- `internal/parser/adrs.go` — ADR extraction with dual format support
- `internal/cli/crystallize.go` — crystallize command, no module auto-detect (line 73-197)
- `internal/cli/search.go` — search command, query-first arg order (line 52-96)
- `internal/cli/query.go` — FindDB() at line 160-173
- `internal/cli/patch.go` — single-occurrence enforcement (line 174-176, 245-246)
- `internal/cli/root.go` — 33 command registration, emitRecoveryHint()
- `internal/skeleton/skeleton.go` — scaffold generator (129 lines)
- `internal/skeleton/templates.go` — Go templates for module/constitution (271 lines)
- `internal/workspace/init.go` — workspace initialization (337 lines)
- `internal/discover/` — thread management, mode classification, convergence
- `internal/cli/validate.go` — 17 checks, GetFirstSpecID usage

### Spec Elements Queried
- APP-INV-048 (Event Stream VCS Primacy)
- APP-INV-020 (Event Stream Append-Only)
- APP-INV-046 (Error Recovery Guidance)
- APP-INV-036 (Human Format Transparency)
- APP-INV-037 (Workspace Isolation)
- APP-INV-002 (Validation Determinism)
- APP-INV-001 (Round-Trip Fidelity)
- APP-INV-016 (Oplog Append-Only)
- APP-ADR-026 (Full Workspace Init)
- APP-ADR-028 (Progressive Validation)

### Live Bugs Reproduced
- `ddis search manifest.ddis.db "query"` — wrong arg order creates empty DB, misleading error
- `ddis query manifest.ddis.db "APP-INV-001"` — same arg order trap
- Statement format mismatch → silent invariant discard (confirmed by code path analysis)
