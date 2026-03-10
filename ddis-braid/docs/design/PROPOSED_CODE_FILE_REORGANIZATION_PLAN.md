# Proposed Code & Documentation Reorganization Plan

> **Status**: PROPOSAL — requires review and approval before execution.
> **Date**: 2026-03-09
> **Scope**: All files in `ddis-braid/`

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Current State Analysis](#2-current-state-analysis)
3. [Design Principles](#3-design-principles)
4. [Proposed Structure](#4-proposed-structure)
5. [Code Reorganization](#5-code-reorganization)
6. [Documentation Reorganization](#6-documentation-reorganization)
7. [File Merge/Split Recommendations](#7-file-mergesplit-recommendations)
8. [Migration Mechanics](#8-migration-mechanics)
9. [Breaking Change Inventory](#9-breaking-change-inventory)
10. [Verification Protocol](#10-verification-protocol)

---

## 1. Executive Summary

The project root currently has **12 markdown files, 9 directories, and 3 config files** at
the top level, with an empty `src/` directory and empty `tests/` subdirectories. An agent
encountering this for the first time faces a 24-item flat listing with no hierarchy to
signal what matters.

This plan proposes consolidating the root to **6 items** that an agent can parse in one glance:

```
ddis-braid/
├── AGENTS.md          ← agent instructions (ambient context, ~500 tokens)
├── SEED.md            ← foundational design (load on demand)
├── Cargo.toml         ← workspace root
├── crates/            ← ALL Rust code (2 workspace members)
├── spec/              ← formal specification (19 namespace files)
├── docs/              ← everything else (guide, history, audits, exploration)
```

**Core moves**:
- `braid-kernel/`, `braid/` → `crates/braid-kernel/`, `crates/braid/`
- 8 root markdown files → `docs/` subdirectories by category
- Empty `src/`, empty `tests/` stubs → deleted
- `guide/`, `transcripts/`, `references/`, `exploration/`, `audits/` → under `docs/`

**What does NOT move**: `AGENTS.md`, `SEED.md`, `Cargo.toml`, `Cargo.lock`, `.gitignore`,
`.github/`, `.beads/`, `spec/`. These are either workspace-root artifacts, agent entry
points, or the living specification.

---

## 2. Current State Analysis

### 2.1 Root-Level Inventory (24 items)

```
ddis-braid/                              CATEGORY         SIZE      VERDICT
├── AGENTS.md                            Agent config     26 KB     KEEP AT ROOT
├── CLAUDE.md → AGENTS.md                Symlink          -         KEEP AT ROOT
├── SEED.md                              Foundation       48 KB     KEEP AT ROOT
├── Cargo.toml                           Build config     263 B     KEEP AT ROOT
├── Cargo.lock                           Lock file        28 KB     KEEP AT ROOT
├── .gitignore                           Git config       49 B      KEEP AT ROOT
├── .github/                             CI/CD            -         KEEP AT ROOT
├── .beads/                              Issue tracking   -         KEEP AT ROOT
├── spec/                                Specification    705 KB    KEEP AT ROOT
│
├── ADRS.md                              Design index     150 KB    → docs/design/
├── HARVEST.md                           Session log      167 KB    → docs/
├── FAILURE_MODES.md                     Failure catalog  131 KB    → docs/design/
├── GAP_ANALYSIS.md                      Audit            101 KB    → docs/audits/
├── DEFECT_SPEC.md                       Defect record    7.8 KB    → docs/audits/
├── onboarding.md                        Legacy guide     19 KB     → docs/reference/
├── SPEC.md                              Stub pointer     1 KB      DELETE (redundant)
├── IMPLEMENTATION_GUIDE.md              Stub pointer     1.4 KB    DELETE (redundant)
│
├── braid-kernel/                        Core library     12.7K LOC → crates/
├── braid/                               CLI binary       3K LOC    → crates/
├── src/                                 EMPTY            0         DELETE
├── tests/                               EMPTY stubs      0         DELETE
│
├── guide/                               Impl guide       450 KB    → docs/guide/
├── transcripts/                         Design history   1.2 MB    → docs/history/transcripts/
├── references/                          Reference docs   781 KB    → docs/history/references/
├── exploration/                         Research         800 KB    → docs/history/exploration/
└── audits/                              Audit reports    215 KB    → docs/audits/
```

### 2.2 Problems Identified

**P1: No hierarchy signals priority.** An agent must read 12 markdown filenames to figure
out which ones matter. `AGENTS.md` (read every session) sits next to `onboarding.md`
(read once, about the legacy Go CLI). Both look equally important at the root level.

**P2: Empty directories create confusion.** `src/` is empty. `tests/integration/`,
`tests/kani/`, `tests/proptest/` are empty. These are aspirational stubs (NEG-001) that
signal "something should be here" but contain nothing. An agent may waste time trying to
understand why code isn't in `src/`.

**P3: Stub files are indirection with no value.** `SPEC.md` (17 lines) says "go look at
`spec/README.md`." `IMPLEMENTATION_GUIDE.md` (26 lines) says "go look at `guide/README.md`."
These add a hop without adding information. An agent already reading the root listing will
see the `spec/` and `guide/` directories directly.

**P4: Documentation categories are invisible.** `ADRS.md` (design decisions), `HARVEST.md`
(session log), `FAILURE_MODES.md` (failure catalog), `GAP_ANALYSIS.md` (audit), and
`DEFECT_SPEC.md` (defect record) all sit at the root with no grouping. An agent cannot tell
which are "read every session" vs. "consult occasionally" vs. "historical record."

**P5: Code crates float at the root alongside docs.** `braid-kernel/` and `braid/` sit at
the same level as `transcripts/` and `exploration/`. There is no visual separation between
"this is the code" and "this is documentation."

**P6: 1.2 MB of transcripts dominate the root.** Seven 40–145 KB transcript files plus their
`.txt` duplicates are large, historical, and rarely read (the journal index exists for
surgical access). They deserve archival status, not root adjacency.

### 2.3 Codebase Statistics

| Category | Files | LOC | Location |
|----------|-------|-----|----------|
| braid-kernel (lib) | 20 .rs | 12,707 | `braid-kernel/src/` |
| braid (binary) | 16 .rs | 3,037 | `braid/src/` |
| Integration tests | 3 .rs | 2,168 | `braid-kernel/tests/` |
| Specification | 20 .md | ~9,700 lines | `spec/` |
| Implementation guide | 18 .md | ~5,200 lines | `guide/` |
| Design transcripts | 16 files | ~16,000 lines | `transcripts/` |
| Root docs | 8 .md | ~8,200 lines | root |
| Exploration | 28 files | ~12,000+ lines | `exploration/` |
| Audits | 18 files | ~5,000+ lines | `audits/` |
| References | 4 .md | ~5,000 lines | `references/` |

---

## 3. Design Principles

These principles are derived from prompt-optimization research (ms skill `prompt-optimization`
v5.2.0) and formal engineering best practices. They are ordered by priority.

### P1: Two-Layer Context Architecture

From the prompt-optimization skill: context operates on two layers:

- **Ambient layer** (~20 tokens per concept): Permanent, cheap, k*-exempt. Ensures the agent
  *knows things exist* without consuming attention budget. This is `AGENTS.md` at the root.
- **Active layer** (~2–6K tokens per file): Loaded on demand, competes for attention, must
  be shed when absorbed. These are the individual spec/ and guide/ files.

**Implication**: The root should contain only ambient-layer routing documents. Everything
else should be one directory hop away, organized so the agent can load exactly the file it
needs without reading an index first.

### P2: File Names as Navigation

An agent deciding which file to read relies on the filename more than any index document.
Filenames must be **self-describing**: an agent reading `docs/guide/03-query.md` knows
immediately it's the query implementation guide, section 3.

**Implication**: Numbered prefixes within directories. Descriptive directory names. No
generic names like `docs.md` or `notes/`.

### P3: Separate Discovery from Execution

Discovery documents (high degrees of freedom): "What is the domain? What invariants hold?"
Execution documents (low degrees of freedom): "Implement this exact interface."

Mixing both in the same file creates the mid-DoF saddle — output becomes hedged and generic.

**Implication**: `spec/` (what the system must do) and `docs/guide/` (how to build it) are
separate directories. Historical exploration (`docs/history/`) is separated from active
specification (`spec/`).

### P4: Depth = 2 Maximum

The user explicitly requested "not too many levels of nesting." Across the entire project,
no file should be more than 2 directory hops from the root:

- `crates/braid-kernel/src/store.rs` — 3 hops, but this is Cargo convention (unavoidable)
- `docs/history/transcripts/journal.md` — 3 hops, but transcripts are rarely accessed
- Everything else: ≤ 2 hops

### P5: Preserve Cargo Workspace Conventions

The `crates/` pattern (workspace members in a `crates/` subdirectory) is the dominant Rust
ecosystem convention for multi-crate workspaces (used by Bevy, Ruff, Turbopack, Helix,
Zed, etc.). This pattern:

- Keeps `Cargo.toml` and `Cargo.lock` at the workspace root (required)
- Groups all Rust code under one directory
- Scales to additional crates without cluttering the root

### P6: Eliminate Dead Weight

Empty directories, stub files pointing elsewhere, and duplicate content (`.md` + `.txt`
versions of the same transcript) consume attention for zero information. They must go.

### P7: Preserve All Historical Provenance

No document content is deleted. Files are moved, consolidated, or archived — never destroyed.
The git history preserves the full provenance trail regardless of reorganization.

---

## 4. Proposed Structure

### 4.1 Complete Directory Tree

```
ddis-braid/
│
│   ── Root (ambient layer: what an agent sees first) ──
│
├── AGENTS.md                              Agent operating instructions (symlink target)
├── CLAUDE.md → AGENTS.md                  Backward-compatibility symlink
├── SEED.md                                Foundational design document
├── Cargo.toml                             Workspace root (updated: members path)
├── Cargo.lock                             Dependency lock
├── .gitignore                             Git config (updated: remove src/, tests/)
├── .github/                               CI/CD workflows
│   └── workflows/
│       ├── ci.yml
│       └── kani.yml
├── .beads/                                Issue tracking state
│
│   ── Specification (the living standard) ──
│
├── spec/                                  Formal specification (UNCHANGED)
│   ├── README.md                          Master index
│   ├── 00-preamble.md
│   ├── 01-store.md
│   ├── ...                                (18 namespace files, no changes)
│   └── 18-trilateral.md
│
│   ── Implementation (all Rust code) ──
│
├── crates/                                Cargo workspace members
│   ├── braid-kernel/                      Core library (pure computation, no IO)
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── datom.rs
│   │   │   ├── store.rs
│   │   │   ├── schema.rs
│   │   │   ├── query/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── clause.rs
│   │   │   │   ├── evaluator.rs
│   │   │   │   ├── graph.rs
│   │   │   │   └── stratum.rs
│   │   │   ├── resolution.rs
│   │   │   ├── merge.rs
│   │   │   ├── harvest.rs
│   │   │   ├── seed.rs
│   │   │   ├── guidance.rs
│   │   │   ├── trilateral.rs
│   │   │   ├── layout.rs
│   │   │   ├── agent_md.rs
│   │   │   ├── error.rs
│   │   │   ├── stage.rs
│   │   │   ├── kani_proofs.rs
│   │   │   └── proptest_strategies.rs
│   │   ├── tests/
│   │   │   ├── cross_namespace.rs
│   │   │   ├── harvest_seed_cycle.rs
│   │   │   └── stateright_model.rs
│   │   └── proptest-regressions/
│   │       └── query/graph.txt
│   │
│   └── braid/                             CLI binary (IO, commands, MCP)
│       ├── Cargo.toml                     (updated: braid-kernel path)
│       └── src/
│           ├── main.rs
│           ├── bootstrap.rs
│           ├── layout.rs
│           ├── mcp.rs
│           ├── error.rs
│           ├── output.rs
│           └── commands/
│               ├── mod.rs
│               ├── transact.rs
│               ├── query.rs
│               ├── harvest.rs
│               ├── seed.rs
│               ├── guidance.rs
│               ├── merge.rs
│               ├── log.rs
│               ├── generate.rs
│               ├── init.rs
│               └── status.rs
│
│   ── Documentation (everything else) ──
│
└── docs/
    ├── README.md                          Documentation map (NEW — routing index)
    │
    ├── HARVEST.md                         Session log (moved from root)
    │
    ├── guide/                             Implementation guide (moved from root)
    │   ├── README.md
    │   ├── 00-architecture.md
    │   ├── 01-store.md
    │   ├── ...                            (all existing guide files)
    │   └── types.md
    │
    ├── design/                            Design artifacts (NEW grouping)
    │   ├── ADRS.md                        Design decision index (moved from root)
    │   ├── FAILURE_MODES.md               Failure mode catalog (moved from root)
    │   └── DEFECT_SPEC.md                 Defect record (moved from root)
    │
    ├── audits/                            Quality assurance (moved from root)
    │   ├── GAP_ANALYSIS.md                Gap analysis (moved from root)
    │   └── stage-0/                       Stage 0 audit files (moved from audits/)
    │       ├── V1_AUDIT_3-3-2026.md
    │       ├── V1_AUDIT_TRIAGE.md
    │       ├── wave1-findings-resolution.md
    │       ├── phantom-type-audit.md
    │       ├── divergence-resolution-matrix.md
    │       ├── R6-divergence-catalog-update.md
    │       ├── R7-consistency-scan.md
    │       ├── R7-crossref-coherence.md
    │       ├── R7-namespace-readiness.md
    │       └── research/
    │           ├── braid-crdt.tla
    │           ├── D1-scope-boundary.md
    │           ├── D2-datalog-engines.md
    │           ├── D3-kani-feasibility.md
    │           ├── D4-harvest-epistemology.md
    │           ├── D5-tokenizer-survey.md
    │           ├── free-functions-audit.md
    │           ├── seed-as-prompt-analysis.md
    │           └── tla-spec-guide.md
    │
    └── history/                           Historical record (read-only archive)
        ├── README.md                      History navigation guide (NEW)
        ├── onboarding.md                  Legacy DDIS project guide (moved from root)
        ├── transcripts/                   Design session transcripts (moved)
        │   ├── journal.md                 Transcript index
        │   ├── journal.txt
        │   ├── 01-datomic-rust-crdt-spec-foundation.md
        │   ├── 01-datomic-rust-crdt-spec-foundation.txt
        │   ├── ...                        (all 7 transcript pairs)
        │   └── 07-ddis-seed-document-finalization.txt
        ├── references/                    Reference material (moved)
        │   ├── AGENTIC_SYSTEMS_FORMAL_ANALYSIS.md
        │   ├── BRAID_IDEATION_TRANSCRIPT.md
        │   ├── DATOMIC_IN_RUST.md
        │   └── RUST_RESOURCES.md
        └── exploration/                   Research explorations (moved)
            ├── coherence-convergence/     (all existing files)
            ├── execution-topologies/      (all existing files)
            └── sheaf-coherence/           (all existing files)
```

### 4.2 Root-Level Comparison

| Before (24 items) | After (10 items) |
|---|---|
| AGENTS.md | AGENTS.md |
| CLAUDE.md (symlink) | CLAUDE.md (symlink) |
| SEED.md | SEED.md |
| Cargo.toml | Cargo.toml |
| Cargo.lock | Cargo.lock |
| .gitignore | .gitignore |
| .github/ | .github/ |
| .beads/ | .beads/ |
| spec/ | spec/ |
| **braid-kernel/** | **crates/** |
| **braid/** | **docs/** |
| **guide/** | |
| **transcripts/** | |
| **references/** | |
| **exploration/** | |
| **audits/** | |
| **src/** (empty) | |
| **tests/** (empty stubs) | |
| **ADRS.md** | |
| **HARVEST.md** | |
| **FAILURE_MODES.md** | |
| **GAP_ANALYSIS.md** | |
| **DEFECT_SPEC.md** | |
| **onboarding.md** | |
| **SPEC.md** (stub) | |
| **IMPLEMENTATION_GUIDE.md** (stub) | |

**Reduction**: 24 → 10 root items. An agent can parse the entire root in one `ls`.

### 4.3 Cognitive Load at Each Level

**Root** (10 items, ~5 seconds to parse):
- 3 files to read (AGENTS.md, SEED.md, Cargo.toml)
- 5 directories with self-describing names
- 2 config artifacts (.gitignore, CLAUDE.md symlink)

**`crates/`** (2 items):
- `braid-kernel/` — the pure computation engine
- `braid/` — the CLI and IO layer

**`docs/`** (5 items):
- `HARVEST.md` — the session log (read every session start)
- `guide/` — how to build each namespace
- `design/` — design decisions and failure modes
- `audits/` — quality assurance records
- `history/` — historical record (transcripts, references, exploration)

**`spec/`** (20 items — unchanged):
- Already well-organized with numbered prefixes
- Agent can identify the right namespace file from its name

---

## 5. Code Reorganization

### 5.1 Move Crates to `crates/`

**What**: Move `braid-kernel/` and `braid/` into `crates/`.

**Why**: The `crates/` convention is the dominant pattern in the Rust ecosystem for
multi-member workspaces. It immediately signals "all code lives here" to any agent or
developer. It separates code from documentation at the directory level.

**Cargo.toml changes**:
```toml
# Before
[workspace]
members = ["braid-kernel", "braid"]

# After
[workspace]
members = ["crates/braid-kernel", "crates/braid"]
```

**braid/Cargo.toml dependency path change**:
```toml
# Before
braid-kernel = { path = "../braid-kernel" }

# After
braid-kernel = { path = "../braid-kernel" }
# (unchanged — relative path still works since both are under crates/)
```

**CI changes** (`.github/workflows/ci.yml`, `kani.yml`):
- Any hardcoded paths to `braid-kernel/` or `braid/` must be updated.
- Cargo commands (`cargo test`, `cargo clippy`) run from workspace root and are
  path-independent — no changes needed for those.

### 5.2 Delete Empty Directories

**What**: Remove `src/` (empty) and `tests/` (empty subdirectories only).

**Why**: These are aspirational stubs (NEG-001). They contain no code and serve no purpose.
An agent encountering an empty `src/` directory will wonder why the code isn't there. The
actual test files live in `braid-kernel/tests/` where Cargo expects them.

**Risk**: Zero. These directories contain no files.

### 5.3 Delete Stub Files

**What**: Remove `SPEC.md` and `IMPLEMENTATION_GUIDE.md`.

**Why**: `SPEC.md` (17 lines) is a pointer to `spec/README.md`. `IMPLEMENTATION_GUIDE.md`
(26 lines) is a pointer to `guide/README.md` (which will become `docs/guide/README.md`).
Both add an indirection hop without adding information. The directories they point to are
visible in the root listing (spec/) or one level down (docs/guide/).

**Risk**: Any references to these files in AGENTS.md or other docs must be updated.
AGENTS.md currently references both in its "Staged Roadmap" and "Task-Specific Guidance"
sections — these references will be updated to point to `spec/README.md` and
`docs/guide/README.md` directly.

---

## 6. Documentation Reorganization

### 6.1 Rationale for `docs/` Hierarchy

The documentation falls into five distinct categories with different access patterns:

| Category | Access Pattern | Location |
|----------|---------------|----------|
| **Session log** | Every session start/end | `docs/HARVEST.md` |
| **Implementation guide** | When building a specific namespace | `docs/guide/` |
| **Design artifacts** | When making or reviewing design decisions | `docs/design/` |
| **Audit records** | When assessing quality or tracking defects | `docs/audits/` |
| **Historical archive** | Rarely; surgical access via journal index | `docs/history/` |

Grouping by access pattern means an agent can navigate to the right subdirectory based on
its current task phase:

- **Orienting?** → read `AGENTS.md` (root) and `docs/HARVEST.md`
- **Understanding a design choice?** → `docs/design/ADRS.md`
- **Building a namespace?** → `docs/guide/NN-namespace.md`
- **Auditing quality?** → `docs/audits/`
- **Tracing a historical decision?** → `docs/history/transcripts/journal.md`

### 6.2 File-by-File Disposition

#### Files Moving to `docs/HARVEST.md`

**HARVEST.md** (167 KB, 2,516 lines): Moves to `docs/HARVEST.md`. This is the session log
that every session reads at start and writes at end. It's the most-accessed doc file after
AGENTS.md. Placing it at `docs/` top level (not nested further) keeps it one hop away.

#### Files Moving to `docs/design/`

| File | Lines | Rationale |
|------|-------|-----------|
| `ADRS.md` | 1,390 | Design decision index — consulted when making or reviewing choices |
| `FAILURE_MODES.md` | 1,615 | Failure mode catalog — consulted when assessing risk or checking methodology |
| `DEFECT_SPEC.md` | 193 | Defect record — active tracking of known defects with fix plans |

These three files share a common access pattern: they are consulted during design work,
not during implementation. Grouping them makes the "design" vs. "build" distinction visible
at the directory level.

#### Files Moving to `docs/audits/`

| File | Lines | Rationale |
|------|-------|-----------|
| `GAP_ANALYSIS.md` | 1,265 | Comprehensive audit of Go CLI vs. SEED.md |
| `audits/stage-0/*` | ~5,000 | Stage 0 audit reports, research, and triage |

The gap analysis is fundamentally an audit artifact — it assesses the existing Go CLI
against the new specification. It belongs with other quality assurance records.

The `audits/resolved/` directory (currently empty) is removed. When resolved audits exist,
they can be tracked via git history or issue status.

#### Files Moving to `docs/history/`

| Directory | Files | Total Size | Rationale |
|-----------|-------|-----------|-----------|
| `transcripts/` | 16 | ~1.2 MB | Design session history (rarely accessed, large) |
| `references/` | 4 | ~781 KB | External reference material (read-once) |
| `exploration/` | 28 | ~800 KB | Research explorations (informational, not normative) |
| `onboarding.md` | 1 | 19 KB | Guide to the legacy Go CLI (historical context) |

All of these are "read once, consult surgically" documents. They are not part of the active
development workflow. Placing them under `docs/history/` signals their archival status while
preserving full access.

#### Files Moving to `docs/guide/`

`guide/` moves as-is to `docs/guide/`. Its internal structure (numbered files, README.md
index) is already well-organized.

**Note**: `guide/04-resolution.md` has a `.rs` extension in the guide directory listing
but is actually a markdown file. This should be verified and corrected if needed.

#### Files Being Deleted

| File | Lines | Reason |
|------|-------|--------|
| `SPEC.md` | 17 | Stub pointer to `spec/README.md` — the directory is already visible |
| `IMPLEMENTATION_GUIDE.md` | 26 | Stub pointer to `guide/README.md` — will be at `docs/guide/README.md` |
| `src/` | 0 (empty dir) | Aspirational stub, contains no files |
| `tests/integration/` | 0 (empty dir) | Aspirational stub |
| `tests/kani/` | 0 (empty dir) | Aspirational stub |
| `tests/proptest/` | 0 (empty dir) | Aspirational stub |
| `tests/` | 0 (empty dir) | Parent of empty stubs |
| `audits/resolved/` | 0 (empty dir) | Empty placeholder |

### 6.3 New `docs/README.md` — Documentation Map

A new routing index document will be created at `docs/README.md`:

```markdown
# Documentation Map

## Quick Access

| Task | Read This |
|------|-----------|
| Start a session | `HARVEST.md` (latest entry) |
| Understand a design choice | `design/ADRS.md` |
| Check failure modes for your task | `design/FAILURE_MODES.md` |
| Build a namespace | `guide/NN-namespace.md` |
| Review quality gates | `audits/` |
| Trace a historical decision | `history/transcripts/journal.md` |

## Directory Structure

- **`HARVEST.md`** — Session log. Read at start, write at end.
- **`guide/`** — Per-namespace implementation guides.
- **`design/`** — ADRS, failure modes, defect tracking.
- **`audits/`** — Gap analysis, formal audits, research.
- **`history/`** — Transcripts, references, explorations (archival).
```

### 6.4 New `docs/history/README.md`

A brief routing document explaining the historical archive:

```markdown
# Historical Archive

Read-only reference material. Access surgically via indices.

- **`onboarding.md`** — Guide to the legacy DDIS Go CLI (~62.5K LOC).
- **`transcripts/`** — 7 design session transcripts. Start with `journal.md`.
- **`references/`** — External research and ideation material.
- **`exploration/`** — Advanced research (coherence theory, execution topologies,
  sheaf cohomology). Informational, not normative.
```

---

## 7. File Merge/Split Recommendations

### 7.1 Code File Split Candidates

After careful analysis, I am recommending **no code file splits at this time**. Here is the
reasoning for each candidate that was evaluated:

#### `schema.rs` (1,949 lines) — DO NOT SPLIT

**Arguments for splitting**: Largest file. Contains layer definitions, attribute validation,
resolution mode config, genesis bootstrap, and evolution logic.

**Arguments against splitting**: The six schema layers (Layer 0: meta-schema through Layer 5:
trilateral) form a single cohesive concept. Splitting would create artificial boundaries
within a naturally unified type (`Schema`). Every consumer of the schema needs the full
`Schema` struct — there is no use case for loading "just the genesis attributes" or "just
the resolution modes" independently.

More importantly, splitting `schema.rs` into 5 files would create 5 new `pub use` re-export
paths, increase the number of files an agent must navigate, and add module boundary ceremony
(visibility modifiers, import paths) for zero functional benefit. The file is large but has
clear internal section boundaries marked by `// -----` separators.

**Verdict**: Keep as-is. If it grows past ~2,500 lines, reconsider.

#### `guidance.rs` (1,025 lines) — DO NOT SPLIT

**Arguments for splitting**: Mixes telemetry, M(t) computation, routing, and footer formatting.

**Arguments against splitting**: All four concerns are tightly coupled by the `MethodologyScore`
type, which is computed from telemetry, used by routing, and rendered by the footer. Splitting
would require passing `MethodologyScore` across module boundaries with no gain in encapsulation.
The test suite exercises the pipeline end-to-end (telemetry → score → footer).

**Verdict**: Keep as-is. The file is at the upper end of comfortable size but not over it.

#### `store.rs` (1,021 lines) — DO NOT SPLIT

**Arguments for splitting**: Transaction typestate pattern + Store struct + tests.

**Arguments against splitting**: `Transaction<S>` exists solely to feed into `Store::transact()`.
They are caller and callee of a single API. Splitting them would separate a type from its
only consumer, making the typestate pattern harder to understand (you'd need to read two files
to see the full state machine). The ~300 lines of proptest tests are standard inline tests.

**Verdict**: Keep as-is.

### 7.2 Code File Merge Candidates

#### `commands/init.rs` (19 lines) and `commands/status.rs` (27 lines) — DO NOT MERGE

**Arguments for merging**: Both are tiny. Could inline into `commands/mod.rs`.

**Arguments against merging**: The one-file-per-command pattern is consistent across all 10
command files. Merging two but not others creates an inconsistency that would confuse an agent
("why are init and status in mod.rs but harvest has its own file?"). The cognitive overhead
of two small files is lower than the cognitive overhead of an inconsistent pattern.

**Verdict**: Keep the consistent pattern. All commands get their own file.

#### `output.rs` (16 lines) — DO NOT MERGE

**Arguments for merging**: Single function, tiny file.

**Arguments against merging**: It exists as a named module, which means other modules can
`use crate::output::*` without importing from a grab-bag module. If output formatting grows
(likely in Stage 1 with budget-aware output), having the dedicated file already in place
is correct.

**Verdict**: Keep as-is.

### 7.3 Summary

**No code file splits or merges are recommended.** The codebase is already well-structured
with clear responsibility boundaries. The largest file (`schema.rs` at 1,949 lines) is at
the upper end of comfortable but does not justify the ceremony of splitting given the tight
coupling of its internal components. The smallest files maintain a consistent one-file-per-
command pattern that aids navigation.

**Rationale**: The prompt-optimization research emphasizes that **structure changes have
diminishing returns past the point where files are self-describing and findable by name**.
The current Rust files already satisfy this criterion. The reorganization effort is better
spent on the directory layout and documentation hierarchy, where the gains are much larger.

---

## 8. Migration Mechanics

### 8.1 Step-by-Step Execution Plan

All steps are idempotent and can be verified independently. The entire migration is a
single atomic commit.

#### Phase 1: Create Target Directories

```
mkdir -p crates
mkdir -p docs/design
mkdir -p docs/audits
mkdir -p docs/history
```

#### Phase 2: Move Crates (Code)

```
git mv braid-kernel crates/braid-kernel
git mv braid crates/braid
```

#### Phase 3: Update Cargo.toml Workspace Members

Edit `Cargo.toml`:
```toml
members = ["crates/braid-kernel", "crates/braid"]
```

Verify: `cargo check` must pass.

#### Phase 4: Move Documentation

```
# Session log
git mv HARVEST.md docs/HARVEST.md

# Design artifacts
git mv ADRS.md docs/design/ADRS.md
git mv FAILURE_MODES.md docs/design/FAILURE_MODES.md
git mv DEFECT_SPEC.md docs/design/DEFECT_SPEC.md

# Audit records
git mv GAP_ANALYSIS.md docs/audits/GAP_ANALYSIS.md
git mv audits/stage-0 docs/audits/stage-0

# Implementation guide
git mv guide docs/guide

# Historical archive
git mv onboarding.md docs/history/onboarding.md
git mv transcripts docs/history/transcripts
git mv references docs/history/references
git mv exploration docs/history/exploration
```

#### Phase 5: Delete Empty Directories and Stubs

```
# Empty dirs (need explicit user permission per RULE NUMBER 1)
rm -r src/
rm -r tests/
rm -r audits/  # now empty after stage-0 moved

# Stub files
git rm SPEC.md
git rm IMPLEMENTATION_GUIDE.md
```

**IMPORTANT**: Per RULE NUMBER 1 in AGENTS.md, file/directory deletion requires explicit
user permission. Phase 5 must not execute without written confirmation.

#### Phase 6: Create New Index Documents

Write `docs/README.md` and `docs/history/README.md` (content from §6.3 and §6.4).

#### Phase 7: Update Cross-References

Every file that references moved paths must be updated. See §9 for the complete inventory.

#### Phase 8: Verify

```
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

All 288 tests must pass identically.

### 8.2 Git Considerations

- Use `git mv` for all moves to preserve blame history.
- Single atomic commit with a descriptive message.
- No force-push, no rebase — standard commit on `main`.

---

## 9. Breaking Change Inventory

### 9.1 Cargo Configuration Changes

| File | Change | Risk |
|------|--------|------|
| `Cargo.toml` (root) | `members = ["crates/braid-kernel", "crates/braid"]` | LOW — path change only |
| `crates/braid/Cargo.toml` | Verify `braid-kernel` path is still `../braid-kernel` | NONE — relative path unchanged |

### 9.2 CI/CD Pipeline Changes

| File | Change Required | Detail |
|------|----------------|--------|
| `.github/workflows/ci.yml` | Check for hardcoded paths | Cargo commands use workspace root — likely no change |
| `.github/workflows/kani.yml` | Check for hardcoded paths | May reference `braid-kernel/` directly for Kani harnesses |

Must read both files and update any path references from `braid-kernel/` to
`crates/braid-kernel/` and `braid/` to `crates/braid/`.

### 9.3 Documentation Cross-Reference Updates

Every moved file that is referenced by another file needs a path update. Here is the
complete inventory of references that must be checked and updated:

#### AGENTS.md (root) — References to Update

| Current Reference | New Reference |
|---|---|
| `HARVEST.md` | `docs/HARVEST.md` |
| `FAILURE_MODES.md` | `docs/design/FAILURE_MODES.md` |
| `IMPLEMENTATION_GUIDE.md` | `docs/guide/README.md` |
| `GAP_ANALYSIS.md` | `docs/audits/GAP_ANALYSIS.md` |
| `ADRS.md` | `docs/design/ADRS.md` |
| `transcripts/journal.txt` | `docs/history/transcripts/journal.txt` |
| `transcripts/journal.md` | `docs/history/transcripts/journal.md` |
| `onboarding.md` | `docs/history/onboarding.md` |
| `guide/README.md` | `docs/guide/README.md` |

#### SEED.md — References to Check

SEED.md references `AGENTS.md` and `spec/` (both unchanged). It may also reference
`transcripts/` — all such references must be updated to `docs/history/transcripts/`.

#### spec/README.md — References to Check

The spec README may reference `guide/` files. Update to `docs/guide/`.

#### docs/guide/README.md — References to Check

Guide files may reference `spec/` (unchanged), `SEED.md` (unchanged), or other root docs
(need updating).

#### docs/guide/*.md — References to Check

Individual guide files may reference:
- `spec/NN-namespace.md` — path from guide files changes from `../spec/` to `../../spec/`
- Other guide files — unchanged (same directory)

#### docs/design/ADRS.md — References to Check

References `transcripts/` (update to `../history/transcripts/`), `SEED.md` (update to
`../../SEED.md`).

#### docs/design/FAILURE_MODES.md — References to Check

Similar to ADRS.md — references to root docs and transcripts need path updates.

### 9.4 Summary of Required Edits

| Category | Files to Edit | Estimated Changes |
|----------|--------------|-------------------|
| Cargo configuration | 1 | 1 line |
| CI/CD workflows | 2 | 0–4 lines each |
| AGENTS.md | 1 | ~15 path references |
| SEED.md | 1 | ~5 path references |
| spec/README.md | 1 | ~2 path references |
| docs/guide/README.md | 1 | ~5 path references |
| docs/guide/*.md files | ~15 | ~2 per file (relative path to spec/) |
| docs/design/*.md | 3 | ~10 path references total |
| New index files | 2 | docs/README.md, docs/history/README.md |

---

## 10. Verification Protocol

### 10.1 Pre-Migration Baseline

Before any changes:
```
cargo test --all-targets 2>&1 | tail -20    # Record: 288 tests pass
cargo clippy --all-targets -- -D warnings   # Record: clean
cargo fmt --check                           # Record: clean
```

### 10.2 Post-Migration Verification

After all changes:
```
cargo test --all-targets 2>&1 | tail -20    # Must match baseline exactly
cargo clippy --all-targets -- -D warnings   # Must be clean
cargo fmt --check                           # Must be clean
```

### 10.3 Cross-Reference Verification

Grep for any remaining references to old paths:
```
grep -rn 'braid-kernel/' --include='*.md' --include='*.yml' --include='*.toml' | \
  grep -v 'crates/braid-kernel' | grep -v '.git/'

grep -rn 'braid/' --include='*.md' --include='*.yml' --include='*.toml' | \
  grep -v 'crates/braid' | grep -v '.git/'
```

Any matches (excluding git internals and the crates/ prefix) indicate a missed update.

### 10.4 Documentation Link Verification

For each moved file, verify that all documents referencing it have been updated:
```
# Example: verify no stale references to root-level HARVEST.md
grep -rn 'HARVEST.md' --include='*.md' | grep -v 'docs/HARVEST.md'
```

---

## Appendix A: What This Plan Does NOT Change

The following are explicitly out of scope:

1. **`spec/` directory structure** — already well-organized with numbered prefixes
2. **Internal Rust module structure** — no splits, merges, or renames of `.rs` files
3. **Cargo dependency graph** — `braid` depends on `braid-kernel`, unchanged
4. **Test file locations** — integration tests stay in `crates/braid-kernel/tests/`
5. **`.beads/` issue tracking** — stays at root per ACFS convention
6. **`.github/` CI/CD location** — stays at root per GitHub convention
7. **Any file content** — files are moved, not edited (except cross-reference path updates)

## Appendix B: Future Considerations

These are NOT part of this plan but are noted for future sessions:

1. **`schema.rs` modularization** — If it grows past ~2,500 lines in Stage 1, split into
   `schema/` sub-module directory (core.rs, genesis.rs, evolution.rs, layers.rs).

2. **Transcript deduplication** — The `.md` and `.txt` versions of each transcript contain
   the same content in different formats. Consider keeping only the `.md` versions and
   generating `.txt` on demand if needed for programmatic parsing.

3. **`exploration/` consolidation** — The three exploration subdirectories
   (coherence-convergence, execution-topologies, sheaf-coherence) could potentially be
   consolidated into fewer files with clearer summaries of findings.

4. **HARVEST.md size management** — At 167 KB (2,516 lines), HARVEST.md is approaching the
   point where it should be split into per-session files or have older entries archived.

---

*This plan itself follows DDIS methodology: every proposed change has a rationale, every
risk is inventoried, and verification criteria are mechanically checkable. The plan is the
specification for the reorganization — the implementation must satisfy it exactly.*
