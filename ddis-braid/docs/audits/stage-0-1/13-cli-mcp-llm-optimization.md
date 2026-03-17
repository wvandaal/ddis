# CLI/MCP Surface Audit: LLM Optimization
<!-- audit date: 2026-03-17 | auditor: direct session inspection | method: live command execution -->

> **Scope**: Every command, output format, MCP tool definition, and guidance footer that an
> AI agent consumes. Evaluated against the prompt-optimization principle: every CLI response
> is a prompt turn that competes for agent attention budget.
>
> **Principle reference**: API-as-prompt — unoptimized CLI = unoptimized prompt. Each defect
> listed below costs tokens without activating the correct agent behavior.

---

## Executive Summary

The braid CLI has strong bones but 12 concrete defects in its LLM-facing surface. The most
critical: the guidance footer's "Next" action recommends `braid status --deep --full` which
**hangs the terminal** (bilateral computation takes >10s with no progress output). Any agent
following this recommendation freezes. The second most critical: the MCP server returns
5-line bare text from `braid_status` while the CLI returns a rich dashboard — agents using
MCP get a stripped-down view of the store that omits F(S), M(t), coherence, tasks, and the
next action.

| Severity | Count | Labels |
|----------|-------|--------|
| P0 (hang/crash) | 1 | FOOT-1 |
| P1 (agent-blocking) | 4 | FOOT-2, MCP-1, MCP-2, CMD-2 |
| P2 (token waste / wrong activation) | 5 | FOOT-3, CMD-1, CMD-3, CMD-4, HELP-1 |
| P3 (polish) | 2 | CMD-5, MCP-3 |

---

## FOOT-1 [P0]: Footer recommends `--deep` which hangs

**Observed**:
```
↳ M(t): 0.50 → (tx: ✗ | spec-lang: △ | q-div: ✗ | harvest: ✓) | Store: 11710 datoms | Turn 655
  Next: braid status --deep --full — verify INV-TRILATERAL-001 (``` LIVE_I, LIVE_S...
```

The footer's `Next:` action says `braid status --deep --full`. Running `braid status --deep`
hangs indefinitely (bilateral computation has no timeout, no progress output, no streaming
output). The `--full` flag doesn't exist — the bilateral command uses `--spectral`, not `--full`.

**Impact**: P0. Every agent following this footer recommendation hangs. This is the most
common guidance signal in the system (appears in every response footer). The guidance
mechanism designed to prevent drift is actively causing agents to freeze.

**Root cause**:
1. `derive_actions_with_budget()` in `guidance.rs` emits `braid status --deep --full` as the
   "CONNECT" action for high Phi values but `--deep` triggers unbounded bilateral computation.
2. `--full` flag doesn't exist on `braid status` (the real flag is `--spectral`).

**Fix**:
```rust
// In guidance.rs derive_actions_with_budget, CONNECT action:
// Replace:
GuidanceAction { command: "braid status --deep --full".into(), ... }
// With:
GuidanceAction { command: "braid bilateral".into(), ... }  // bilateral has own timeout handling
// OR: add --timeout <seconds> to braid status --deep
```

Also fix `--full` → `--spectral` in all generated action strings.

---

## FOOT-2 [P1]: Footer inlines spec statement bodies (~80 wasted tokens per response)

**Observed**:
```
Next: braid status --deep --full — verify INV-TRILATERAL-001 (``` LIVE_I, LIVE_S, LIVE_P
are monotone functions of the store semilattice (P...), INV-TRILATERAL-004 (``` ∀ LINK
operations adding :spec/traces-to or :spec/implements links: Φ(S')...)
```

The `build_command_footer()` calls `resolve_spec_summary()` which fetches the invariant
statement body from the store and inlines it into the footer. The invariant statements contain
formal math and code fences. This embeds ~80 tokens of non-actionable spec prose into every
guidance footer where it competes with actual task context.

**Impact**: P1. The guidance footer already competes for budget. Adding invariant bodies
makes the footer 3-4× longer with content the agent cannot act on directly. The agent
needs the invariant ID (to go look it up), not the full statement in the footer.

**Fix**: In `build_command_footer()`, emit only the invariant ID, not the body:
```
Next: braid bilateral — verify INV-TRILATERAL-001, INV-TRILATERAL-004
```

The `resolve_spec_summary()` function should return `None` at all non-Full guidance levels,
or cap to 10 tokens max.

---

## FOOT-3 [P2]: Footer sub-metric labels are opaque abbreviations

**Observed**:
```
↳ M(t): 0.50 → (tx: ✗ | spec-lang: △ | q-div: ✗ | harvest: ✓)
```

`tx` = transact_frequency, `spec-lang` = spec_language_ratio, `q-div` = query_diversity,
`harvest` = harvest_quality. An agent reading this footer cannot tell what to improve. The
check symbols (`✗`, `△`, `✓`) indicate sub-metrics are below/near/above threshold but the
labels don't connect to actionable commands.

**Prior audit**: This was captured as a failure mode (FM decision session 021: "Bilateral
output uses opaque single-letter component labels"). The fix was decided but not yet applied
to the M(t) sub-metric labels in `format_footer()`.

**Fix**: Expand labels to their actionable form, or add inline hints:
```
↳ M(t): 0.50 → (transact: ✗→braid write | spec-lang: △ | query-diversity: ✗→braid query | harvest: ✓)
```

Or at Full level only, add a legend line:
```
  M(t) legend: tx=transact_frequency spec-lang=spec_language_ratio q-div=query_diversity
```

---

## MCP-1 [P1]: `braid_status` MCP tool returns 5-line bare text vs CLI rich dashboard

**MCP response** (from `tool_status()`):
```
datoms: 11710
transactions: 659
entities: 1820
schema_attributes: 86
frontier_agents: 2
```

**CLI `braid status`**:
```
store: .braid (11710 datoms, 1820 entities, 659 txns)
coherence: Phi=259.8 B1=95 GapsAndCycles | M(t)=0.50 stable
harvest: 2 tx since last (ok) | tasks: 218 open (13 ready)
↳ M(t): 0.50 → ... Next: ...
```

The MCP tool omits: F(S), M(t), coherence quadrant, Phi, beta_1, harvest status, task counts,
next action. An agent using MCP to orient gets ~5% of the information an agent using CLI gets.
This directly violates ADR-INTERFACE-004 (library-mode MCP) and INV-INTERFACE-002 (thin wrapper).

**Fix**: `tool_status()` should call the same status pipeline as the CLI command, not a
hand-rolled subset:
```rust
fn tool_status(layout: &DiskLayout) -> Result<JsonValue, BraidError> {
    let store = layout.load_store()?;
    let footer = build_command_footer(&store, None);
    let text = format_status_agent(&store, &footer);  // reuse CLI status renderer
    Ok(json!({ "content": [{ "type": "text", "text": text }] }))
}
```

---

## MCP-2 [P1]: MCP server missing entire task workflow

**Current MCP tools**: `braid_status`, `braid_query`, `braid_write`, `braid_harvest`,
`braid_seed`, `braid_observe`.

**Missing**: `braid_task_ready`, `braid_next`, `braid_go`, `braid_done`.

An agent using MCP cannot:
- See what tasks are ready to work on
- Claim a task to signal in-progress status
- Mark a task done

This means the entire `braid next → braid go <id> → work → braid done <id>` workflow
documented in every help text is **unreachable via MCP**. Agents using MCP fall back to
`braid_query` with ad-hoc Datalog to find tasks, which is verbose and error-prone.

**Fix**: Add 4 tools:
```json
{
  "name": "braid_task_ready",
  "description": "Show tasks ready to work (unblocked, sorted by R(t) impact score). Use at session start to pick work. Returns task IDs, titles, priorities, and the claim command for the top task.",
  "inputSchema": { "type": "object", "properties": {}, "required": [] }
},
{
  "name": "braid_next",
  "description": "Single top-priority unblocked task + exact claim command. Use this instead of braid_task_ready when you just want to know what to work on next. Returns: id, title, priority, impact_score, claim_command.",
  "inputSchema": { "type": "object", "properties": {}, "required": [] }
},
{
  "name": "braid_go",
  "description": "Claim a task: marks it in-progress and records you as the working agent. Use after braid_next to claim the recommended task. Required before starting work (INV-TASK-001).",
  "inputSchema": {
    "type": "object",
    "properties": {
      "id": { "type": "string", "description": "Task ID from braid_next or braid_task_ready" }
    },
    "required": ["id"]
  }
},
{
  "name": "braid_done",
  "description": "Close a task as completed. Use after finishing work on a task claimed with braid_go. Provide a reason summarizing what was done.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "id": { "type": "string", "description": "Task ID to close" },
      "reason": { "type": "string", "description": "What was done (becomes task provenance)" }
    },
    "required": ["id"]
  }
}
```

---

## MCP-3 [P3]: `braid_query` MCP tool lacks Datalog

**Current**: `braid_query` accepts `entity` and `attribute` filters only.
**CLI**: `braid query` accepts full Datalog expressions: `[:find ?e ?v :where [?e :attr ?v]]`.

Datalog is the primary query interface in the spec (INV-QUERY-001). The MCP query tool only
exposes a subset. Agents reach for complex queries (e.g., find all tasks with a given status,
find all spec elements by namespace) via Datalog but MCP doesn't support it.

**Fix**: Add `datalog` parameter to `braid_query`:
```json
{
  "name": "braid_query",
  "description": "Search the store. Three modes: (1) entity filter: entity=':spec/inv-001' returns all datoms about that entity. (2) attribute filter: attribute=':db/doc' returns all values. (3) Datalog: datalog='[:find ?e ?v :where [?e :spec/namespace ?v]]' for joins. Omit all to scan (capped at 100).",
  "inputSchema": {
    "type": "object",
    "properties": {
      "entity": { "type": "string", "description": "Entity keyword, e.g. ':spec/inv-store-001'" },
      "attribute": { "type": "string", "description": "Attribute keyword, e.g. ':db/doc'" },
      "datalog": { "type": "string", "description": "Datalog expression: [:find ?vars :where [clauses]]" }
    },
    "required": []
  }
}
```

---

## CMD-1 [P2]: `braid next` returns all 13 ready tasks — should return ONE

**Observed**: `braid next` and `braid task ready` produce identical output — a list of all
13 ready tasks. The design intent (from CONTINUATION.md) is that `braid next` returns the
**single top task** with its exact claim command:

```
top task: t-bcee "EPIC: Stage 0 Merge Cascade" [P0, impact=0.51]
claim: braid go t-bcee
```

Currently `braid next --format json` returns:
```json
{ "ready_count": 13, "tasks": [{"id":"t-bcee","priority":0,...}, ...] }
```

No `impact_score` field despite R(t) being implemented. The "next" semantics are diluted —
an agent gets a list and must choose rather than receiving a direct recommendation.

**Fix**:
1. `braid next` (no flags) → returns single top task: id, title, priority, R(t) impact score,
   claim command (`braid go <id>`), and ONE sentence rationale.
2. `braid next --all` or `braid task ready` → returns full list.
3. Add `impact_score` field to `braid next --format json` output.

---

## CMD-2 [P1]: `braid task ready` truncates at 5 in agent mode

**Observed**:
```
ready: 13 tasks
  [P0] t-bcee "EPIC: Stage 0 Merge Cascade..." (epic)
  ...
  ... and 8 more
```

The `--format agent` output (which is the default) truncates at 5 tasks. For human mode,
truncation is ergonomic. For agent mode, it's harmful — the agent needs the full list to
make an informed priority decision. The "...and 8 more" message triggers a follow-up query,
wasting a full round-trip.

**Fix**: In agent mode, show all ready tasks. Human mode may truncate. If the list is very
long (>20), show all but group by priority tier.

---

## CMD-3 [P2]: `braid status --verbose` is identical to `braid status`

**Observed**: Running `braid status --verbose` produces the same 4-line output as
`braid status`. The help text promises "Full output with all metrics and actions" but the
flag appears to be unimplemented.

Progressive disclosure is documented (`bare → --verbose → --deep`) but only `--deep` (which
hangs) and bare (which works) are meaningfully different.

**Fix**: `--verbose` should add at minimum: all R(t) actions (not just top 1), M(t) sub-metric
explanations, F(S) breakdown by component (Coverage/Validation/Drift/Harvest/Contradiction).

---

## CMD-4 [P2]: F(S) absent from `braid status` default output

**Observed**: `braid status` shows `M(t)=0.50` but not `F(S)`. F(S) is the core fitness
function (SEED.md §primary_directive) and the primary measure of project health. It appears
only in `--orientation` output (`F(S)=0.66`) and in `braid bilateral`.

An agent reading `braid status` to understand project state gets M(t) (methodology adherence)
but not F(S) (spec-implementation coherence). These are complementary — omitting F(S) from
the dashboard gives an incomplete picture.

**Fix**: Add F(S) to the default status line:
```
store: .braid (11710 datoms, 1820 entities, 659 txns)
coherence: F(S)=0.66, Phi=259.8 B1=95 GapsAndCycles | M(t)=0.50 stable
```

---

## CMD-5 [P3]: `braid observe` vs `braid write assert` — no disambiguation

**Observed**: Both `braid observe` and `braid write assert` can record facts into the store.
An agent choosing between them has no clear signal for which to use. The descriptions:
- `observe`: "Capture knowledge as content-addressed entity"
- `write assert`: "Assert datoms into the store"

**Fix**: In the `observe` help text, add the disambiguation line:
> Use `braid observe` (not `write assert`) for knowledge capture during work. `write assert`
> is for raw structured mutations (schema entries, spec elements, inter-entity links).

---

## HELP-1 [P2]: Global flags repeated verbatim in every subcommand help (350 lines of noise)

**Observed**: Every subcommand's `--help` output repeats the full `--budget`, `--context-used`,
and `--format` flag documentation verbatim — 14 lines each × 25 commands ≈ 350 lines of
identical text that appears in every help request.

ADR-INTERFACE-011 says "command help as agent context." Repetitive global flag docs dilute
the per-command signal. When an agent reads `braid harvest --help` it gets 14 lines of
`--budget` documentation before reaching the harvest-specific flags.

**Fix**: In Clap, mark global flags with `.global(true)` and suppress their per-subcommand
display (they still apply). Add a one-line note at the bottom of each subcommand's help:
```
Global flags: --format (json|agent|human), --budget <tokens>, --context-used <0.0-1.0>
See: braid --help for full documentation.
```

---

## Summary Table: Actionable Fixes

| ID | File | Fix | Effort |
|----|------|-----|--------|
| FOOT-1 | `guidance.rs:derive_actions_with_budget` | Replace `braid status --deep --full` → `braid bilateral` as CONNECT action | S |
| FOOT-1b | `guidance.rs:derive_actions_with_budget` | Add timeout to `braid status --deep` or emit streaming output | M |
| FOOT-2 | `guidance.rs:build_command_footer` | Emit INV IDs only in footer, not statement bodies | S |
| FOOT-3 | `guidance.rs:format_footer` | Expand sub-metric abbreviations OR add inline legend | S |
| MCP-1 | `mcp.rs:tool_status` | Call shared status renderer, return full dashboard | M |
| MCP-2 | `mcp.rs:tool_definitions` | Add braid_task_ready, braid_next, braid_go, braid_done tools | M |
| MCP-3 | `mcp.rs:tool_definitions + tool_query` | Add `datalog` parameter to braid_query | S |
| CMD-1 | `crates/braid/src/commands/next.rs` | Return single top task + impact_score; `--all` for list | S |
| CMD-2 | `crates/braid/src/commands/task.rs` | Remove agent-mode truncation in task ready | S |
| CMD-3 | `crates/braid/src/commands/status.rs` | Implement `--verbose` with full metrics | M |
| CMD-4 | `crates/braid/src/commands/status.rs` | Add F(S) to default status output line | S |
| CMD-5 | `crates/braid/src/main.rs` | Add disambiguation comment to `observe` help | XS |
| HELP-1 | `crates/braid/src/main.rs` | Suppress repeated global flags in subcommand help | M |

**Total effort**: ~2-3 S0-session implementation pass

---

## Priority Order for Implementation

**Wave 1 (this session)**: P0+P1 fixes
1. FOOT-1 — fix hanging footer action (unblocks all agents immediately)
2. FOOT-2 — remove spec body from footer (immediate token savings)
3. CMD-2 — remove agent-mode task truncation (agents can see all ready tasks)
4. CMD-4 — add F(S) to status output
5. MCP-1 — upgrade braid_status MCP tool to match CLI

**Wave 2 (next session)**: P1+P2 fixes
6. MCP-2 — add task workflow tools to MCP
7. CMD-1 — fix braid next to return single task with impact score
8. FOOT-3 — expand sub-metric labels
9. CMD-3 — implement --verbose

**Wave 3**: P2+P3 polish
10. MCP-3 — add Datalog to MCP query
11. HELP-1 — suppress repeated global flags
12. CMD-5 — add observe disambiguation

---

## What's Working Well

The following surface areas are already well-optimized and should not be changed:

- **`braid --help` quick-start examples** — concrete 5-line workflow with live metric
  placeholders. Excellent activation language.
- **`braid --orientation`** — dense single-screen orientation with live store metrics.
  Clean demonstrations-first format.
- **`braid harvest --help`** — shows the complete workflow (observe → harvest → seed)
  as sequential examples. Correct DoF calibration.
- **`braid task --help`** — good lifecycle example at the bottom ("Workflow: ...").
- **MCP `braid_observe` description** — "Fastest way to capture knowledge" + concrete
  example. Leads with activation, shows input shape, sets expectations.
- **MCP `braid_seed` description** — "Start-of-session: load relevant context...
  Use this instead of manually reading files." Strong activation cue.
- **`braid harvest --commit --guard` progressive flags** — well-documented with clear
  semantics for each variant.
- **Global `--format` flag documentation** — the four-step resolution priority is clear
  and correct for agent-first design.
