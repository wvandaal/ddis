# PROPOSED FILE REORGANIZATION PLAN

> Date: 2026-03-01
> Status: DRAFT — Awaiting user review before any changes are made

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Current State Analysis](#2-current-state-analysis)
3. [Problems With the Current Structure](#3-problems-with-the-current-structure)
4. [Proposed New Structure](#4-proposed-new-structure)
5. [Detailed Change Inventory](#5-detailed-change-inventory)
6. [Files to Remove (Safe Deletions)](#6-files-to-remove-safe-deletions)
7. [Files to Consolidate](#7-files-to-consolidate)
8. [Reference Updates Required](#8-reference-updates-required)
9. [Risk Assessment](#9-risk-assessment)
10. [Implementation Order](#10-implementation-order)
11. [Flagged Items Requiring Clarification](#11-flagged-items-requiring-clarification)

---

## 1. Executive Summary

The DDIS project root currently contains **13 tracked non-dot files** at the top level, plus **4 major subdirectories** (`ddis-cli/`, `ddis-cli-spec/`, `ddis-modular/`, `ddis-evolution/`) and **5 dot-directories** (`.beads/`, `.cass/`, `.claude/`, `.ddis/`, `.vscode/`). The primary problems are:

1. **Root-level clutter**: 6 files related to the RALPH improvement loop are scattered at root level alongside unrelated files
2. **Exact duplicates**: `ddis_standard.md` and `ddis_final.md` at root are byte-identical copies of files already in `ddis-evolution/versions/`
3. **Mixed concerns in `ddis-evolution/`**: Active design documents are mixed with pure historical version checkpoints
4. **Mixed concerns in `.ddis/specs/`**: Superseded audit iterations sit alongside current strategic documents with no organizational distinction
5. **Orphaned artifacts**: Empty log files, placeholder files, and scratch databases that serve no purpose
6. **Naming inconsistency**: Mix of `SCREAMING_CASE`, `kebab-case`, and `snake_case` with no convention

### What does NOT change

The following directories are well-structured, load-bearing, and should not be reorganized:

- **`ddis-cli/`** — Self-contained Go project with standard Go package layout. Untouched.
- **`ddis-cli-spec/`** — Canonical CLI specification with manifest + constitution + modules. Untouched.
- **`ddis-modular/`** — Canonical meta-spec. Untouched.
- **`.beads/`** — Issue tracking state. Untouched.
- **`.cass/`** — Session history. Untouched.
- **`.claude/`** — Claude settings. Untouched.
- **`.vscode/`** — IDE settings (gitignored). Untouched.

### Proposed changes by impact level

| Impact | Count | Description |
|--------|-------|-------------|
| **Remove** | 12 files | Exact duplicates, empty files, orphaned scratch artifacts |
| **Move** | 14 files | Root-level RALPH files → `ralph/`, design docs → `docs/design/`, audits → organized subdirs |
| **Consolidate** | 3 groups | Superseded audit rounds → single archive file, polished spec versions → remove intermediates |
| **Keep in place** | ~320 files | All load-bearing spec/code/config files |

---

## 2. Current State Analysis

### 2.1 Root-Level Files (13 tracked + 2 untracked)

| File | Size | Type | Status | Verdict |
|------|------|------|--------|---------|
| `README.md` | 78 KB | Documentation | **Active** | Keep at root |
| `AGENTS.md` | 887 B | Config | **Active** | Keep at root |
| `CLAUDE.md` | symlink | Config | **Active** | Keep at root |
| `.gitignore` | 778 B | Config | **Active** | Keep at root |
| `ddis_ralph_loop.sh` | 60 KB | RALPH tool | **Active** | Move to `ralph/` |
| `ddis_assemble.sh` | 13 KB | RALPH tool | **Active** | Move to `ralph/` |
| `ddis_validate.sh` | 19 KB | RALPH tool | **Active** | Move to `ralph/` |
| `ddis_ralph_readme.md` | 11 KB | RALPH docs | **Active** | Move to `ralph/` |
| `ddis_recursive_improvement_prompt.md` | symlink | RALPH config | **Active** | Move to `ralph/` |
| `kickoff_prompt.md` | 3 KB | RALPH artifact | Historical | Move to `ralph/` |
| `ddis_standard.md` | 166 KB | Monolith spec | **DUPLICATE** | Remove (identical to `ddis-evolution/versions/ddis_v0.md`) |
| `ddis_final.md` | 209 KB | Monolith spec | **DUPLICATE** | Remove (identical to `ddis-evolution/versions/ddis_final.md`) |
| `ddis_modularization_protocol.md` | 31 KB | Reference doc | Historical | Move to `docs/reference/` |
| `DDIS_CLI_PLAN.md` | 19 KB | Design doc | Historical | Move to `docs/design/` |
| `manifest.ddis.db` | 300 KB | SQLite | **Gitignored** | Already gitignored; no action |
| `APP-INV-047` | 268 KB | SQLite scratch | **Gitignored** | Already gitignored; safe to delete |

### 2.2 `ddis-evolution/` (47 tracked files)

| Area | Files | Status |
|------|-------|--------|
| Root design docs (10 files) | Active design/architecture documents | Move to `docs/design/` |
| `versions/` monolith specs (7 files) | Historical version checkpoints | Keep as-is in `ddis-evolution/versions/` |
| `versions/v0,v1,v2/` modular reconstructions (15 files) | Historical reconstructions | Keep as-is |
| `constitution_versions/` (3 files, 1 empty) | Historical constitutions | Keep; remove empty placeholder |
| `judgments/` (2 files) | RALPH convergence evaluations | Move to `ralph/judgments/` |
| `logs/` (8 empty files) | Empty log placeholders | Remove all |
| `.beads/` (6 files) | Archived issue tracking | Keep as-is |
| `.gitignore` | Config | Keep |

### 2.3 `.ddis/specs/` (12 files)

| File | Size | Status | Verdict |
|------|------|--------|---------|
| `cleanroom-audit-2026-03-01.md` | 14 KB | **Current** | Keep |
| `cleanroom-audit-remediation-plan.md` | 18 KB | **Current** | Keep |
| `CLEANROOM_AUDIT_V2_2026-02-28.md` | 19 KB | **Authoritative** | Keep |
| `CLEANROOM_AUDIT_2026-02-28.md` | 11 KB | Superseded by V2 | Consolidate into archive |
| `CLEANROOM_AUDIT_R2_2026-02-28.md` | 7 KB | Superseded by V2 | Consolidate into archive |
| `CLEANROOM_AUDIT_R3_2026-02-28.md` | 3 KB | Superseded by V2 | Consolidate into archive |
| `NEXT_STEPS_UNIVERSALITY_2026-02-28.md` | 30 KB | **Active strategy** | Keep |
| `RECOMMENDATION_MCP_PROTOCOL_SERVER_2026-03-01.md` | 25 KB | **Active strategy** | Keep |
| `UNIVERSALITY_FIELD_REPORT_2026-02-28.md` | 33 KB | **Active evidence** | Keep |
| `GAP_ANALYSIS_2026-02-27.md` | 22 KB | **Active reference** | Keep |
| `CEREMONIAL_VS_LOADBEARING_TOOL_USAGE_2026-02-28.md` | 6 KB | **Active insight** | Keep |
| `DDIS_RECURSIVE_IMPROVEMENT_STRATEGY.md` | 23 KB | **Active** (symlink target) | Keep |

### 2.4 `.ddis/events/` (3 files)

All 3 files are canonical event data required by the DDIS CLI. **No changes.**

---

## 3. Problems With the Current Structure

### 3.1 Root-Level Clutter

A newcomer looking at the project root sees:

```
README.md                              # What is this project?
AGENTS.md / CLAUDE.md                  # Agent config
DDIS_CLI_PLAN.md                       # ??? (historical plan doc)
ddis_assemble.sh                       # ??? (what does this assemble?)
ddis_final.md                          # ??? (final version of what?)
ddis_modularization_protocol.md        # ??? (reference doc)
ddis_ralph_loop.sh                     # ??? (what is RALPH?)
ddis_ralph_readme.md                   # ??? (README for RALPH)
ddis_recursive_improvement_prompt.md   # ??? (symlink somewhere)
ddis_standard.md                       # ??? (166 KB monolith)
ddis_validate.sh                       # ??? (validation script)
kickoff_prompt.md                      # ??? (one-time kickoff)
manifest.ddis.db                       # ??? (database file)
```

This is 10 non-obvious files at root level. A developer must read each one to understand why it exists. The fundamental issue: **the RALPH improvement loop (5+ files) and historical monolith specs (2 files) are mixed with project config files**, creating confusion about what's active infrastructure vs. historical artifact.

### 3.2 Exact File Duplication

Verified via `diff` (zero output = byte-identical):

| Root file | Identical copy in | Size wasted |
|-----------|-------------------|-------------|
| `ddis_standard.md` | `ddis-evolution/versions/ddis_v0.md` | 166 KB |
| `ddis_final.md` | `ddis-evolution/versions/ddis_final.md` | 209 KB |

These root copies are referenced by `ddis_ralph_loop.sh` as `SEED_SPEC` and as the final output copy target. But the authoritative versions already exist in `ddis-evolution/versions/`. The RALPH script can be updated to reference those instead.

### 3.3 Orphaned Empty Files

`ddis-evolution/logs/` contains 8 files, all 0 bytes:

```
apply_v1.log    apply_v2.log    audit_v1.log    audit_v2.log
judge_v1.log    judge_v2.log    modularize.log  polish.log
```

These are log file placeholders created by the RALPH loop that were never populated. They contain no information.

`ddis-evolution/constitution_versions/constitution_v2.md` is 1 byte (empty newline). No content.

### 3.4 `ddis-evolution/` Mixes Active Design with Pure History

The directory contains two very different kinds of content:

**Active design/architecture docs** (still referenced, still relevant):
- `EVENT_STREAM_DESIGN_AUDIT_2026-02-26.md` — Identifies real architectural debt
- `event-sourcing-architecture.md` — Blueprint for addressing that debt
- `IMPLEMENTATION_PROMPT.md` — Next implementation directions
- `PROGRESS_REVIEW_2026-02-24.md` — Strategic assessment
- `TOOLING_EXPLORATION.md` — Tool gap analysis
- `WORKFLOW_WITNESS_PLAN.md` — Methodology verification framework
- `feature-discovery-ddis-skill.md` — Active process spec
- `feature-discovery-state-template.json` — Active schema

**Pure historical checkpoints** (never referenced except as archives):
- `versions/ddis_v0.md`, `ddis_v1.md`, `ddis_v2.md` — Monolith snapshots
- `versions/ddis_polished*.md` — Intermediate working copies
- `versions/v0/`, `v1/`, `v2/` — Reconstructed modular snapshots
- `constitution_versions/` — Constitution history
- `judgments/` — RALPH convergence evaluations (terminal — judge said `stop_converged`)
- `structural_assessment.json` — One-time modularization planning

These should be separated for clarity.

### 3.5 `.ddis/specs/` Mixes Superseded Audits with Current Strategy

Three rounds of cleanroom audits (R1, R2, R3) were superseded by V2, which consolidated and corrected them. But all four files sit side-by-side with no indication of status. A developer encountering `CLEANROOM_AUDIT_2026-02-28.md` alongside `CLEANROOM_AUDIT_V2_2026-02-28.md` must read both to discover the first is superseded.

---

## 4. Proposed New Structure

```
/data/projects/ddis/
│
├── README.md                          # Project overview (UNCHANGED)
├── AGENTS.md                          # Agent guidelines (UNCHANGED)
├── CLAUDE.md → AGENTS.md             # Symlink (UNCHANGED)
├── .gitignore                         # Git config (UPDATED: add ralph/ patterns)
│
├── ddis-cli/                          # Go CLI implementation (UNCHANGED)
├── ddis-cli-spec/                     # CLI specification (UNCHANGED)
├── ddis-modular/                      # Meta-spec (DDIS standard) (UNCHANGED)
│
├── ralph/                             # RALPH improvement loop toolchain (NEW)
│   ├── README.md                      # ← was ddis_ralph_readme.md
│   ├── ddis_ralph_loop.sh             # ← was root level
│   ├── ddis_assemble.sh               # ← was root level
│   ├── ddis_validate.sh               # ← was root level
│   ├── kickoff_prompt.md              # ← was root level
│   ├── improvement_strategy.md        # ← was .ddis/specs/DDIS_RECURSIVE_IMPROVEMENT_STRATEGY.md
│   └── judgments/                     # ← was ddis-evolution/judgments/
│       ├── judgment_v1.json
│       └── judgment_v2.json
│
├── docs/                              # Project documentation (NEW)
│   ├── design/                        # Design & architecture documents
│   │   ├── event-sourcing-architecture.md        # ← from ddis-evolution/
│   │   ├── event-stream-design-audit.md          # ← from ddis-evolution/
│   │   ├── implementation-prompt.md              # ← from ddis-evolution/
│   │   ├── workflow-witness-plan.md              # ← from ddis-evolution/
│   │   ├── tooling-exploration.md                # ← from ddis-evolution/
│   │   ├── feature-discovery-skill.md            # ← from ddis-evolution/
│   │   ├── feature-discovery-state-template.json # ← from ddis-evolution/
│   │   └── cli-plan.md                           # ← was DDIS_CLI_PLAN.md at root
│   └── reference/                     # Reference material
│       ├── modularization-protocol.md            # ← was ddis_modularization_protocol.md at root
│       ├── progress-review-2026-02-24.md         # ← from ddis-evolution/
│       └── evaluation-v17.md                     # ← from ddis-evolution/
│
├── ddis-evolution/                    # Historical version archive (REORGANIZED)
│   ├── .gitignore
│   ├── .beads/                        # Historical issue tracking (UNCHANGED)
│   ├── structural_assessment.json     # Modularization planning (historical)
│   ├── constitution_versions/         # Historical constitutions
│   │   ├── constitution_v0.md
│   │   └── constitution_v1.md         # (constitution_v2.md REMOVED — empty)
│   └── versions/                      # Monolith & modular spec checkpoints (UNCHANGED)
│       ├── ddis_v0.md                 # v0 (= the original ddis_standard.md)
│       ├── ddis_v1.md
│       ├── ddis_v2.md
│       ├── ddis_final.md              # Final RALPH output
│       ├── v0/                        # Modular reconstruction
│       ├── v1/
│       └── v2/
│
├── .ddis/                             # DDIS work products & events
│   ├── events/                        # Canonical event streams (UNCHANGED)
│   │   ├── stream-1.jsonl
│   │   ├── stream-2.jsonl
│   │   └── threads.jsonl
│   └── specs/                         # Analysis & strategy documents (REORGANIZED)
│       ├── cleanroom-audit-2026-03-01.md              # Current audit
│       ├── cleanroom-audit-remediation-plan.md        # Current remediation plan
│       ├── CLEANROOM_AUDIT_V2_2026-02-28.md           # Authoritative formal audit
│       ├── NEXT_STEPS_UNIVERSALITY_2026-02-28.md      # Strategy
│       ├── RECOMMENDATION_MCP_PROTOCOL_SERVER_2026-03-01.md  # Strategy
│       ├── UNIVERSALITY_FIELD_REPORT_2026-02-28.md    # Evidence
│       ├── GAP_ANALYSIS_2026-02-27.md                 # Reference
│       ├── CEREMONIAL_VS_LOADBEARING_TOOL_USAGE_2026-02-28.md  # Insight
│       └── audit-archive/                             # Superseded audits (NEW subdir)
│           ├── CLEANROOM_AUDIT_2026-02-28.md          # Round 1 (superseded)
│           ├── CLEANROOM_AUDIT_R2_2026-02-28.md       # Round 2 (superseded)
│           └── CLEANROOM_AUDIT_R3_2026-02-28.md       # Round 3 (superseded)
│
├── .beads/                            # Issue tracking (UNCHANGED)
├── .cass/                             # Session history (UNCHANGED)
├── .claude/                           # Claude settings (UNCHANGED)
└── .apr/                              # APR workspace (UNCHANGED — empty/reserved)
```

### 4.1 Rationale for This Structure

**Why `ralph/`?**
The RALPH improvement loop is a self-contained toolchain: 3 shell scripts, a README, a methodology document, a kickoff prompt, and convergence evaluation results. These 7+ files are tightly coupled — the main script (`ddis_ralph_loop.sh`) directly references the others via `SCRIPT_DIR`. Grouping them makes their relationship obvious and removes 6 files from the root. The name "ralph" is already well-established in the project vocabulary.

**Why `docs/` with `design/` and `reference/`?**
The active design documents in `ddis-evolution/` (event-sourcing architecture, implementation prompts, tooling exploration, etc.) are *forward-looking* artifacts that guide current and future work. They don't belong in an "evolution" archive — they belong where developers look for design guidance. Separating `design/` (active architectural blueprints) from `reference/` (stable reference material like the modularization protocol) matches how developers naturally search for information: "How should I build this?" → `design/`, "What are the rules?" → `reference/`.

**Why keep `ddis-evolution/` but slim it down?**
The version checkpoints (`versions/ddis_v0.md` through `ddis_final.md`) and constitution snapshots are pure history — valuable for understanding how the spec evolved, but not consulted during active development. Keeping them in `ddis-evolution/` with a focused "historical archive" role makes the directory self-documenting. The design docs that were here are moved to `docs/design/` where they'll actually be found and used.

**Why `audit-archive/` inside `.ddis/specs/`?**
The three superseded audit rounds (R1, R2, R3) are valuable for understanding the audit progression but misleading when placed alongside the authoritative V2. A simple subdirectory with a clear name prevents confusion without losing the historical record.

**Why NOT deeper nesting?**
Every proposed directory is at most 3 levels deep from root (`docs/design/event-sourcing-architecture.md`). Deeper nesting would hurt discoverability — the whole point of reorganizing is to make things findable at a glance.

---

## 5. Detailed Change Inventory

### 5.1 Files to Move

| Current Location | New Location | Reason |
|-----------------|--------------|--------|
| `ddis_ralph_loop.sh` | `ralph/ddis_ralph_loop.sh` | RALPH toolchain grouping |
| `ddis_assemble.sh` | `ralph/ddis_assemble.sh` | RALPH toolchain grouping |
| `ddis_validate.sh` | `ralph/ddis_validate.sh` | RALPH toolchain grouping |
| `ddis_ralph_readme.md` | `ralph/README.md` | Renamed for convention; RALPH docs |
| `kickoff_prompt.md` | `ralph/kickoff_prompt.md` | RALPH artifact |
| `ddis_modularization_protocol.md` | `docs/reference/modularization-protocol.md` | Reference material |
| `DDIS_CLI_PLAN.md` | `docs/design/cli-plan.md` | Historical design doc |
| `ddis-evolution/event-sourcing-architecture.md` | `docs/design/event-sourcing-architecture.md` | Active architecture blueprint |
| `ddis-evolution/EVENT_STREAM_DESIGN_AUDIT_2026-02-26.md` | `docs/design/event-stream-design-audit.md` | Active architectural audit |
| `ddis-evolution/IMPLEMENTATION_PROMPT.md` | `docs/design/implementation-prompt.md` | Active implementation roadmap |
| `ddis-evolution/WORKFLOW_WITNESS_PLAN.md` | `docs/design/workflow-witness-plan.md` | Active design spec |
| `ddis-evolution/TOOLING_EXPLORATION.md` | `docs/design/tooling-exploration.md` | Active tool gap analysis |
| `ddis-evolution/feature-discovery-ddis-skill.md` | `docs/design/feature-discovery-skill.md` | Active process spec |
| `ddis-evolution/feature-discovery-state-template.json` | `docs/design/feature-discovery-state-template.json` | Active schema |
| `ddis-evolution/PROGRESS_REVIEW_2026-02-24.md` | `docs/reference/progress-review-2026-02-24.md` | Strategic reference |
| `ddis-evolution/EVALUATION_v17.md` | `docs/reference/evaluation-v17.md` | Quality baseline reference |
| `ddis-evolution/judgments/judgment_v1.json` | `ralph/judgments/judgment_v1.json` | RALPH convergence data |
| `ddis-evolution/judgments/judgment_v2.json` | `ralph/judgments/judgment_v2.json` | RALPH convergence data |
| `.ddis/specs/CLEANROOM_AUDIT_2026-02-28.md` | `.ddis/specs/audit-archive/CLEANROOM_AUDIT_2026-02-28.md` | Superseded |
| `.ddis/specs/CLEANROOM_AUDIT_R2_2026-02-28.md` | `.ddis/specs/audit-archive/CLEANROOM_AUDIT_R2_2026-02-28.md` | Superseded |
| `.ddis/specs/CLEANROOM_AUDIT_R3_2026-02-28.md` | `.ddis/specs/audit-archive/CLEANROOM_AUDIT_R3_2026-02-28.md` | Superseded |
| `.ddis/specs/DDIS_RECURSIVE_IMPROVEMENT_STRATEGY.md` | `ralph/improvement_strategy.md` | RALPH methodology document |

### 5.2 Symlinks to Update

| Current Symlink | Current Target | New Target | Reason |
|----------------|----------------|------------|--------|
| `ddis_recursive_improvement_prompt.md` | `.ddis/specs/DDIS_RECURSIVE_IMPROVEMENT_STRATEGY.md` | **Remove symlink entirely** — the file moves to `ralph/improvement_strategy.md` and the RALPH script is updated to reference it directly |

---

## 6. Files to Remove (Safe Deletions)

### 6.1 Exact Duplicates (verified via `diff` — zero output)

| File to Remove | Canonical Copy Location | Size Recovered |
|---------------|------------------------|----------------|
| `ddis_standard.md` (root) | `ddis-evolution/versions/ddis_v0.md` | 166 KB |
| `ddis_final.md` (root) | `ddis-evolution/versions/ddis_final.md` | 209 KB |

**Verification**: `diff /data/projects/ddis/ddis_standard.md /data/projects/ddis/ddis-evolution/versions/ddis_v0.md` produces zero output (byte-identical). Same for `ddis_final.md`.

**Impact**: The RALPH script `ddis_ralph_loop.sh` references `SEED_SPEC="${SCRIPT_DIR}/ddis_standard.md"` on line 71 and copies the final spec to `${SCRIPT_DIR}/ddis_final.md` on line 1476. After moving the script to `ralph/`, these references will be updated to point to `../ddis-evolution/versions/ddis_v0.md` and `../ddis-evolution/versions/ddis_final.md`.

### 6.2 Empty Files (Zero Content)

| File | Size | Reason |
|------|------|--------|
| `ddis-evolution/logs/apply_v1.log` | 0 B | Empty placeholder |
| `ddis-evolution/logs/apply_v2.log` | 0 B | Empty placeholder |
| `ddis-evolution/logs/audit_v1.log` | 0 B | Empty placeholder |
| `ddis-evolution/logs/audit_v2.log` | 0 B | Empty placeholder |
| `ddis-evolution/logs/judge_v1.log` | 0 B | Empty placeholder |
| `ddis-evolution/logs/judge_v2.log` | 0 B | Empty placeholder |
| `ddis-evolution/logs/modularize.log` | 0 B | Empty placeholder |
| `ddis-evolution/logs/polish.log` | 0 B | Empty placeholder |
| `ddis-evolution/constitution_versions/constitution_v2.md` | 1 B | Empty newline placeholder |

**Note**: After removing all 8 log files, the `ddis-evolution/logs/` directory itself should be removed since it will be empty.

### 6.3 Intermediate Polished Versions

| File | Size | Reason |
|------|------|--------|
| `ddis-evolution/versions/ddis_polished_working.md` | 211 KB | Intermediate working copy; `ddis_final.md` is the terminal version |
| `ddis-evolution/versions/ddis_polished.md` | 209 KB | Intermediate checkpoint; superseded by `ddis_final.md` |
| `ddis-evolution/versions/ddis_polished_v2.md` | 209 KB | Intermediate checkpoint; superseded by `ddis_final.md` |

**Rationale**: The RALPH loop produced: `ddis_v0.md` → `ddis_v1.md` → `ddis_v2.md` → (polish phase) → `ddis_polished_working.md` → `ddis_polished.md` → `ddis_polished_v2.md` → `ddis_final.md`. The `ddis_v*` versions represent meaningful version jumps (scored by judges). The `ddis_polished*` files are intermediate polish iterations that converged into `ddis_final.md`. Keeping them adds ~629 KB with no information not captured in the final and the judgment files.

### 6.4 Orphaned Scratch Artifacts

| File | Size | Reason |
|------|------|--------|
| `APP-INV-047` (root) | 268 KB | SQLite scratch database created by CLI. Already gitignored. |

**Note**: This file is already in `.gitignore` (`APP-INV-*` pattern) and is not tracked by git. Deletion is purely for hygiene.

### 6.5 Total Space Recovered by Removals

| Category | Files | Size |
|----------|-------|------|
| Exact duplicates | 2 | 375 KB |
| Empty files | 9 | ~0 KB |
| Polish intermediates | 3 | 629 KB |
| Scratch artifacts | 1 | 268 KB |
| **Total** | **15** | **~1.27 MB** |

---

## 7. Files to Consolidate

### 7.1 `ddis-modular/output.md` — Generated Artifact

`output.md` (36 KB, 536 lines) in `ddis-modular/` is a rendered copy of `constitution/system.md`. It is:
- Already gitignored (line 58 of `.gitignore`: `ddis-modular/output.md`)
- Regenerable via `ddis render`
- Not referenced by any other file

**Recommendation**: No action needed — it's already gitignored and will be cleaned up naturally. Mentioning here for completeness.

### 7.2 `ddis-evolution/structural_assessment.json` — Keep but Context

This 7 KB JSON file was the one-time modularization planning document that produced the current module structure. It's historical but small and contextually valuable within `ddis-evolution/`. **Keep as-is.**

---

## 8. Reference Updates Required

This is the most critical section. Every move requires verifying and updating references.

### 8.1 RALPH Script Internal References

`ddis_ralph_loop.sh` uses `SCRIPT_DIR` to locate sibling files. After moving to `ralph/`:

| Line | Current Reference | New Reference | Impact |
|------|-------------------|---------------|--------|
| 71 | `SEED_SPEC="${SCRIPT_DIR}/ddis_standard.md"` | `SEED_SPEC="${SCRIPT_DIR}/../ddis-evolution/versions/ddis_v0.md"` | RALPH start point |
| 72 | `IMPROVEMENT_PROMPT="${SCRIPT_DIR}/ddis_recursive_improvement_prompt.md"` | `IMPROVEMENT_PROMPT="${SCRIPT_DIR}/improvement_strategy.md"` | Direct reference (no symlink) |
| 73 | `KICKOFF_PROMPT="${SCRIPT_DIR}/kickoff_prompt.md"` | `KICKOFF_PROMPT="${SCRIPT_DIR}/kickoff_prompt.md"` | No change (file moves with script) |
| 1476 | `cp "$final_spec" "${SCRIPT_DIR}/ddis_final.md"` | `cp "$final_spec" "${SCRIPT_DIR}/../ddis-evolution/versions/ddis_final.md"` | Output location |

Additionally, `ddis_ralph_loop.sh` references `ddis_assemble.sh` and `ddis_validate.sh`:
- Grep shows these are sourced/called via `SCRIPT_DIR`-relative paths
- After the move, all three scripts are in the same `ralph/` directory, so these references remain correct

### 8.2 `ddis_ralph_readme.md` References

This file (becoming `ralph/README.md`) contains a directory tree showing the expected layout:

```
├── ddis_recursive_improvement_prompt.md  # The improvement methodology
├── ddis_standard.md                       # Starting spec (seed)
├── ddis_ralph_loop.sh                     # Main RALPH loop
├── ddis_assemble.sh                       # Module assembler
├── ddis_validate.sh                       # Validation checks
├── kickoff_prompt.md                      # Initial prompt
```

**Update needed**: The directory tree in this README should be updated to reflect the new `ralph/` layout and the new locations of `ddis_standard.md` (now referenced from `ddis-evolution/versions/`).

### 8.3 `kickoff_prompt.md` References

References `ddis_recursive_improvement_prompt.md` by name (line 7). After move, this reference should be updated to `improvement_strategy.md`.

### 8.4 `.ddis/specs/` Internal Cross-References

The `.ddis/specs/` documents reference each other by filename (e.g., `cleanroom-audit-remediation-plan.md` references `cleanroom-audit-2026-03-01.md`). Since most files stay in `.ddis/specs/` and only the superseded audits move to a subdirectory, the cross-references in the remaining files are unaffected.

The superseded audit files (R1, R2, R3) reference each other (e.g., R2 references "findings from Round 1"). Since they all move together into `audit-archive/`, their relative references remain correct.

### 8.5 `.gitignore` Updates

Add entry for `ralph/` build artifacts if any are produced:

```gitignore
# RALPH loop working artifacts (generated during runs)
ralph/versions/
ralph/logs/
```

No other `.gitignore` changes needed — the patterns are path-agnostic.

### 8.6 `AGENTS.md` / `CLAUDE.md` Updates

The project-level `AGENTS.md` does not reference any of the moved files by path. No updates needed.

### 8.7 `README.md` References

The README mentions the RALPH loop and may reference root-level files. Quick-start and toolchain sections should be checked for path references after the move.

### 8.8 Beads Issues

`.beads/issues.jsonl` contains references to `ddis_recursive_improvement_prompt.md` in issue descriptions. These are historical log entries and do not need updating (they document what happened, not where files are now).

### 8.9 `ddis-evolution/` Internal References

Several `ddis-evolution/` design docs reference each other and files in the root:
- `EVALUATION_v17.md` references `versions/ddis_final.md` — after move to `docs/reference/`, this relative reference breaks. **Update needed**: Change to `../../ddis-evolution/versions/ddis_final.md`
- `IMPLEMENTATION_PROMPT.md` references `ddis-evolution/` paths — after move to `docs/design/`, update relative references
- `ddis_ralph_readme.md` references `ddis-evolution/` — after move to `ralph/`, the README update covers this

### 8.10 DDIS CLI Code References

The Go CLI in `ddis-cli/` does NOT reference any of the moved files. It operates on arbitrary paths passed as CLI arguments (e.g., `ddis parse manifest.yaml`). **No code changes needed.**

### 8.11 Symlink Removal

The root-level symlink `ddis_recursive_improvement_prompt.md → .ddis/specs/DDIS_RECURSIVE_IMPROVEMENT_STRATEGY.md` should be removed. The file it points to moves to `ralph/improvement_strategy.md`, and the RALPH script is updated to reference it directly.

---

## 9. Risk Assessment

### 9.1 Low Risk (Safe)

| Change | Risk | Mitigation |
|--------|------|------------|
| Remove empty `logs/` files | None | Zero content; nothing references them |
| Remove `constitution_v2.md` (1 byte) | None | Empty placeholder with no content |
| Remove `APP-INV-047` | None | Already gitignored; scratch artifact |
| Move superseded audits to `audit-archive/` | None | Internal references preserved; no external references |
| Remove polished intermediates | Very low | Only `ddis_final.md` matters (terminal version); judges scored v0/v1/v2, not polished versions |

### 9.2 Medium Risk (Careful)

| Change | Risk | Mitigation |
|--------|------|------------|
| Move RALPH scripts to `ralph/` | Script path references break | Update all `SCRIPT_DIR` references in `ddis_ralph_loop.sh`; test by running `--help` |
| Remove root `ddis_standard.md` and `ddis_final.md` | RALPH script references break | Update script before removing files; verify with `grep -n` |
| Move `DDIS_RECURSIVE_IMPROVEMENT_STRATEGY.md` | Symlink breaks | Remove symlink first, then move file, then update script |
| Remove symlink `ddis_recursive_improvement_prompt.md` | RALPH script references break | Update script reference before removing symlink |

### 9.3 Things That Do NOT Break

- **`ddis` CLI functionality**: The CLI operates on explicitly-passed paths. No hardcoded references to any moved files.
- **`ddis-modular/`**: Completely untouched.
- **`ddis-cli-spec/`**: Completely untouched. Its `parent_spec: ../ddis-modular/manifest.yaml` remains valid.
- **`ddis-cli/`**: Completely untouched. The binary at `ddis-cli/bin/ddis` is unaffected.
- **Validation/drift/coverage**: `ddis validate`, `ddis drift`, `ddis coverage` all operate on database files passed as arguments. Unaffected.
- **Event streams**: `.ddis/events/` is completely untouched.
- **Issue tracking**: `.beads/` is completely untouched.

---

## 10. Implementation Order

The changes should be executed in this exact order to prevent breakage at any intermediate step:

### Phase 1: Create New Directories

```bash
mkdir -p ralph/judgments
mkdir -p docs/design
mkdir -p docs/reference
mkdir -p .ddis/specs/audit-archive
```

### Phase 2: Move RALPH Files (Must Be Atomic)

1. Copy all RALPH-related files to `ralph/`
2. Update `ddis_ralph_loop.sh` path references
3. Update `ralph/README.md` (was `ddis_ralph_readme.md`) directory tree
4. Update `kickoff_prompt.md` reference
5. Move `improvement_strategy.md` from `.ddis/specs/`
6. Move `judgments/` from `ddis-evolution/`
7. Remove the root-level symlink `ddis_recursive_improvement_prompt.md`
8. Verify: `cd ralph && bash ddis_ralph_loop.sh --help` (should show usage)
9. Remove the root-level originals only after verification

### Phase 3: Move Design Docs

1. Move design docs from `ddis-evolution/` to `docs/design/`
2. Move reference docs to `docs/reference/`
3. Update any internal cross-references in moved files

### Phase 4: Move Superseded Audits

1. Move 3 superseded audit files to `.ddis/specs/audit-archive/`

### Phase 5: Remove Duplicates and Empty Files

1. Remove root `ddis_standard.md` (after Phase 2 script update)
2. Remove root `ddis_final.md` (after Phase 2 script update)
3. Remove all 8 empty log files and the `logs/` directory
4. Remove empty `constitution_v2.md`
5. Remove 3 polished intermediates from `ddis-evolution/versions/`
6. Remove `APP-INV-047` (gitignored scratch artifact)

### Phase 6: Final Verification

1. `ddis validate ddis-cli-spec/manifest.ddis.db --json` — should pass (unaffected)
2. `ddis spec ddis-modular/manifest.ddis.db` — should show meta-spec (unaffected)
3. `ddis drift ddis-cli-spec/manifest.ddis.db` — should show 0 drift (unaffected)
4. `git status` — verify expected changes
5. Verify no broken symlinks: `find . -type l ! -exec test -e {} \; -print`

---

## 11. Flagged Items Requiring Clarification

### FLAG 1: `ddis-evolution/.beads/` — Active or Archive?

The `ddis-evolution/` directory has its own `.beads/` subdirectory with 6 files (640 KB). This appears to be issue tracking specific to the evolution phase. It has not been updated since 2026-02-22.

**Question**: Should this stay in `ddis-evolution/` (as historical issue tracking for that phase), or should it be removed since the main project's `.beads/` is the active tracker?

**My recommendation**: Keep as-is. It's small, self-contained, and documents the issues that drove the evolution work. Removing it loses historical context.

### FLAG 2: `.apr/` Directory — Keep or Remove?

The `.apr/` directory contains two empty subdirectories (`rounds/`, `workflows/`). It was initialized for APR (Agentic Prompt Refinement) but never used.

**Question**: Is APR planned for future use, or can this be removed?

**My recommendation**: Keep as-is. It's zero-size overhead and removing it would be lossy if APR is planned.

### FLAG 3: Root `manifest.ddis.db` — Clarify Gitignore Intention

The root-level `manifest.ddis.db` (300 KB) is gitignored via line 56 of `.gitignore` (`/manifest.ddis.db`). Running `ddis spec manifest.ddis.db` returns "no spec found" — the database appears stale/empty.

**Question**: Is this intentionally kept as a convenience DB for the meta-spec (`ddis-modular/`), or is it orphaned? The meta-spec has its own `ddis-modular/manifest.ddis.db` (2.5 MB) that works correctly.

**My recommendation**: This appears orphaned. It's already gitignored so it doesn't bloat the repo. Safe to delete for hygiene.

### FLAG 4: `ddis-evolution/versions/v0,v1,v2/` Reconstructed Modular Specs

Each of `v0/`, `v1/`, `v2/` contains a reconstructed modular spec structure (manifest.yaml + constitution + 4 modules). These were created as "what-if the spec had been modular at that version" snapshots.

**Question**: Are these ever used, or purely historical? They total ~400 KB across 15 files.

**My recommendation**: Keep as-is within `ddis-evolution/versions/`. They're a natural part of the version history and help understand how modularization evolved.

### FLAG 5: Consolidating `.ddis/specs/` Superseded Audits Further?

Instead of moving the 3 superseded audit rounds to a subdirectory, an alternative is to consolidate them into a single `CLEANROOM_AUDIT_PROGRESSION.md` file with three sections (Round 1 findings, Round 2 fixes, Round 3 resolution). This would compress 21 KB across 3 files into ~15 KB in 1 file.

**Question**: Prefer the subdirectory approach (preserves original files) or the consolidation approach (fewer files, tighter)?

**My recommendation**: Subdirectory approach. It's simpler, reversible, and preserves exact original content. The consolidation would be a lossy transformation of historical records.

### FLAG 6: `ddis-evolution/` Directory Name

After moving the active design docs out, `ddis-evolution/` becomes purely a historical version archive. Should it be renamed to something like `archive/` or `history/` to better reflect its reduced scope?

**Question**: Keep the name `ddis-evolution/` or rename?

**My recommendation**: Keep `ddis-evolution/`. The name still accurately describes its contents (the evolution of the DDIS spec through versions), and renaming would change tracked paths in git for 30+ files. The cost exceeds the benefit.

---

## Summary of Changes

| Metric | Before | After |
|--------|--------|-------|
| Root-level files (tracked) | 13 | 4 (README, AGENTS, CLAUDE, .gitignore) |
| Total files removed | 0 | 15 |
| Total files moved | 0 | ~23 |
| Files consolidated | 0 | 0 (subdirectory approach for audits) |
| New directories created | 0 | 4 (ralph/, docs/design/, docs/reference/, audit-archive/) |
| Maximum nesting depth | N/A | 3 levels (docs/design/file.md) |
| Broken references | N/A | 0 (all tracked and updated in Phase 2-3) |
| CLI functionality affected | N/A | None |
| Spec integrity affected | N/A | None |

### Before: Root Directory Listing
```
AGENTS.md              ddis_assemble.sh                    ddis_ralph_readme.md
CLAUDE.md              ddis_final.md                       ddis_recursive_improvement_prompt.md
DDIS_CLI_PLAN.md       ddis_modularization_protocol.md     ddis_standard.md
README.md              ddis_ralph_loop.sh                  ddis_validate.sh
manifest.ddis.db       kickoff_prompt.md                   APP-INV-047
```

### After: Root Directory Listing
```
AGENTS.md              ddis-cli/          ddis-modular/     ralph/
CLAUDE.md              ddis-cli-spec/     docs/
README.md              ddis-evolution/
```

The root is now **self-documenting**: a developer immediately sees the project README, agent config, and 5 clearly-named directories for the spec, CLI, meta-spec, improvement toolchain, and documentation.
