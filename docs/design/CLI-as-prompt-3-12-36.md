# Plan: CLI-as-Prompt — Braid's Output as Agent Activation Architecture

## Thesis

Braid's CLI is not a tool that produces data — it is a **prompt architecture** that shapes the AI agent's activation manifold. Every interaction (command output, error message, help text, guidance footer) is a field configuration designed to:

1. **Anchor** the agent in the current store state (context = ambient awareness)
2. **Deliver** results using specification language that activates the deep formal-methods substrate (content = active context)
3. **Navigate** toward the next accretive action by demonstrating outcomes (footer = comonadic extract)
4. **Degrade gracefully** under budget pressure via rate-distortion optimal compression

This is the comonadic formulation made concrete: `extract(store_state) → (context, content, footer)` where context is ambient, content is active, and footer is navigative.

### Governing Principles

| Principle | Source | Application to CLI |
|-----------|--------|--------------------|
| Demonstrations over constraints | prompt-optimization Rule 2 | Help text shows ONE example + expected output, not a parameter list |
| Spec-language over instruction-language | spec-first-design Rule 1 | Output says "INV-STORE-001 holds" not "the store is immutable" |
| Make invalid states unrepresentable | rust-formal-engineering Rule 1 | Native CommandOutput enforces three-part structure; from_human() is structural debt |
| Structure first, content second | prompt-optimization Rule 3 | Agent-mode: context → content → footer (never flat text) |
| Budget-aware compression | prompt-optimization Rule 7 | Footer compresses Full(200)→Compressed(60)→Minimal(20)→HarvestOnly(10) as k* shrinks |
| Navigative, not instructive | prompt-optimization Pitfall 4 | Footer demonstrates outcomes: "3 gaps → braid trace" not just "run trace" |
| Ambient/active separation | skill-composition Layer 1/2 | AGENTS.md = ambient (k*-exempt); CLI output = active (competes for k*) |
| Sequence, don't stack | skill-composition Rule 1 | ONE clear next action per footer, not a menu of options |

### What We Have

- 50,240 LOC Rust, 697 tests, 9,239 datoms, 1,555 entities, F(S) = 0.7766
- CommandOutput struct with json/agent/human fields + AgentOutput with context/content/footer
- Comonadic guidance: M(t) methodology score, 18 action derivation rules, 4 compression levels
- Budget-aware output: k*_eff attention decay, GuidanceLevel, projection pyramid π₀–π₃
- from_human() bridge for 19/22 commands (loses the three-part structure)
- Existing spec: INV-INTERFACE-001..010, INV-GUIDANCE-001..011, INV-BUDGET-001..006

### What's Missing (Ordered by Formal Severity)

**Structural (breaks the prompt architecture):**
1. **19/22 commands use from_human() bridge** — agent gets flat text instead of context/content/footer structure. This violates the comonadic contract: extract should always produce structured output.
2. **Piped default is JSON** — agents calling braid in subprocess get machine data, not navigative agent-mode. The primary consumer (AI agent) gets the wrong output mode.
3. **No AGENTS.md auto-creation** — init silently skips injection when no AGENTS.md exists. The "Use braid" chain breaks at the first link.

**Correctness (spec violations):**
4. **Bilateral JSON detection missing** — is_json_output() is not total over Command variants. Footer corrupts JSON. (INV-INTERFACE-001 violation)
5. **Dead --skip flag** — defined but ignored on Next. Dead code is a violated proof obligation.
6. **`which` instead of `command -v`** — POSIX portability violation in tool detection.

**Quality (sub-optimal prompt activation):**
7. **Help text is instructive prose** — "Record an observation" activates surface compliance, not deep reasoning. Should be demonstrative.
8. **Flag proliferation on status** — `--deep --full --spectral --verbose --verify --commit` creates mid-DoF saddle (agent doesn't know which combination to use).
9. **Footer shows commands, not outcomes** — "braid trace" vs "3 unwitnessed INVs → braid trace --commit" — the latter is a micro-demonstration that activates understanding.
10. **install.sh dumps full orientation JSON** — first impression is overwhelming, not inviting.
11. **Footer injection duplicated 6x** — structural fragility; makes INV-GUIDANCE-001 a procedural obligation instead of a structural guarantee.

### What's Already Excellent (Preserve These)

- **Command vocabulary forms a narrative**: `init → observe → harvest → seed` (knowledge lifecycle), `next → go → done` (task lifecycle), `status → query → schema → log` (inspection). This is good prompt design — the names ARE the workflow.
- **Shorthand commands**: `next` ("what should I do?"), `go` ("I'm doing this"), `done` ("finished") — conversational, zero-friction task management.
- **Config-as-datoms**: No config files to manage. `braid config` reads/writes to the store. Clean.
- **Error algebra**: Four-part error model (what/why/fix/spec_ref) with ErrorInfo struct. Just needs completeness audit.
- **Budget system**: GuidanceLevel compression, projection pyramid, k*_eff attention decay. Sophisticated and well-designed.

---

## Phase 0: Correctness Foundation (~35 LOC)

**Motivation**: Type safety and portability invariants must hold before restructuring. These are proof obligations.

### 0A: Fix bilateral JSON detection

**File**: `crates/braid/src/commands/mod.rs`
**Principle**: `is_json_output()` must be a total function over Command variants (rust-formal-engineering Rule 1). Missing Bilateral means footer corrupts JSON, violating INV-INTERFACE-001.

**Fix**: Add `| Command::Bilateral { json: true, .. }` to the `is_json_output()` match arm.
**Test**: Update `is_json_output_detects_all_json_variants` test.

### 0B: Implement --skip on Next command

**Files**: `crates/braid/src/commands/task.rs`, `crates/braid/src/commands/mod.rs`
**Principle**: Dead code is a violated proof obligation (rust-formal-engineering Rule 3). The `skip: _` wildcard pattern silently discards user input — the type system permitted the invalid state.

**Fix**: Pass `skip: Option<String>` through to `task::ready()`. Filter out matching task before selecting top pick. Update dispatch in mod.rs.

### 0C: `which` → `command -v` for POSIX portability

**File**: `crates/braid/src/commands/init.rs` (line 300-306)
**Principle**: `which` is not POSIX-guaranteed. `command -v` is the portable alternative. This affects tool detection in minimal containers and embedded systems.

**Fix**:
```rust
fn tool_available(name: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {} >/dev/null 2>&1", name)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
```

**Gate**: `cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test`

---

## Phase 1: Prompt Pipeline — Agent-First Output Architecture (~70 LOC net)

**Motivation**: The output pipeline IS the prompt architecture. These changes ensure every agent interaction gets the structured three-part output optimized for LLM activation.

### 1A: Agent-First Piped Default

**File**: `crates/braid/src/output.rs` (line ~81)

**Current resolution**:
```
flag > env > piped(non-TTY) → Json > TTY → Human > default: Agent
```

**New resolution**:
```
flag > env > TTY → Human > default: Agent
```

**Rationale**: AI agents are the primary consumer (this is the entire thesis). Agent frameworks call CLIs in piped contexts. Piped-to-JSON was a machine-integration default; agent-mode IS the designed-for-LLM format. Users who want JSON use `--format json` or `BRAID_OUTPUT=json` explicitly.

**Changes**:
- `output.rs` line ~81: Remove the pipe-detection branch. Non-TTY default becomes Agent (same as the general default).
- Update docstrings in `output.rs` (lines 11-13) and `main.rs` (line 42) to reflect new priority.
- Update test `resolve_mode_invalid_flag_falls_through`: non-TTY result is now Agent, not Json.

### 1B: Universal Footer Injection via `maybe_inject_footer()`

**File**: `crates/braid/src/commands/mod.rs`

**Principle**: INV-GUIDANCE-001 requires every tool response to include a guidance footer. The current 6× duplication is a procedural obligation that can be violated by forgetting to copy the pattern. A single helper makes it structural.

**Extract**:
```rust
fn maybe_inject_footer(
    cmd_output: CommandOutput,
    skip_footer: bool,
    path: Option<&Path>,
    budget_ctx: &BudgetCtx,
) -> CommandOutput {
    if skip_footer { return cmd_output; }
    match path.and_then(|p| try_build_footer(p, budget_ctx)) {
        Some(footer) => inject_footer(cmd_output, &footer),
        None => cmd_output,
    }
}
```

**Replace**: All 6 inline injection sites + the from_human() fallthrough path → single-line calls.

**skip_footer logic** (unified): `is_json_output(cmd) || is_generative(cmd)` where generative = seed --inject, write export (output piped to files).

**Net**: ~-50 LOC (code reduction through deduplication).

**Gate**: `cargo test`. Verify `braid status | cat` shows agent-mode output with guidance footer (not JSON).

---

## Phase 2: CLI Surface Optimization — Every Touch Point as Prompt (~80 LOC)

**Motivation**: The CLI surface area (help text, flag names, error messages) is the prompt interface. Per prompt-optimization, structure and demonstrations dominate content and constraints for output quality.

### 2A: Help Text as Navigative Demonstrations

**Files**: `crates/braid/src/commands/mod.rs` (Command enum doc-comments), `crates/braid/src/main.rs` (Cli doc-comment)

**Principle**: One demonstration encodes format, style, depth, and workflow — far more information per attention unit than a constraint list (prompt-optimization Rule 2). Help text should activate deep reasoning ("what invariants does this preserve?") not instruct surface compliance ("this command takes these parameters").

**Pattern**: Every command's doc-comment (which clap uses as help text) follows:
```
/// {One-line in spec-language: what this does and WHY}.
///
/// Example: braid {command} {typical-args}
///   → {Expected output summary (demonstrates the result)}
```

**Specific help text changes** (Command enum variants):

| Command | Current | New (navigative, demonstrative) |
|---------|---------|--------------------------------------|
| `Init` | "Initialize a new braid store" | "Create store, detect environment, record config as datoms. `braid init` → .braid/ + AGENTS.md + seed" |
| `Status` | "Show project coherence dashboard" | "Where you are: F(S), M(t), tasks, next action. `braid status` → store: 9k datoms, F(S)=0.77" |
| `Observe` | "Record an observation" | "Capture knowledge as content-addressed entity. `braid observe 'CRDT merge commutes' -c 0.9`" |
| `Harvest` | "Extract session knowledge" | "End-of-session: observations → datoms. `braid harvest --commit` → 5 candidates crystallized" |
| `Seed` | "Assemble context" | "Start-of-session: store → agent context. `braid seed --task 'my work'` → 5-section briefing" |
| `Query` | "Query the datom store" | "Datalog or entity/attribute filter. `braid query '[:find ?e :where [?e :spec/type \"invariant\"]]'`" |
| `Bilateral` | "Run coherence scan" | "Coherence: F(S) + CC-1..5. `braid bilateral` → F(S)=0.77, CC=4/5, next: trace 3 gaps" |
| `Next` | "Show top ready task" | "Top unblocked task + claim command. `braid next` → T-42: implement merge → `braid go T-42`" |
| `Log` | "Show transaction log" | "Browse transactions, newest first. `braid log --limit 5` → 5 txns with agent, rationale" |

**Why this works**: Each help text IS a micro-demonstration. An agent reading `braid status --help` sees not just "what status does" but "what status output looks like" — activating the correct expectation substrate.

### 2B: Flag Progressive Disclosure on Status

**File**: `crates/braid/src/commands/mod.rs` (Status variant)

**Problem**: `--deep --full --spectral --verbose --verify --commit` on status is 6 flags. An agent seeing all 6 doesn't know which combination to use — this is the mid-DoF saddle (prompt-optimization: mid-DoF produces the worst of both worlds).

**Solution**: Three progressive levels:
```
braid status              → 6-line terse dashboard
braid status --verbose    → full methodology breakdown
braid status --deep       → bilateral F(S) + spectral + 14-algorithm analysis
```

**Implementation**: In the status dispatch arm, if `deep == true`, set `full = true` and `spectral = true` automatically. Mark `--full` and `--spectral` as hidden (`#[arg(hide = true)]`) to preserve backward compatibility while decluttering the help text.

**Help text for status**: "Progressive detail: bare → --verbose → --deep"

### 2C: --orientation as Optimal Seed Turn

**File**: `crates/braid/src/commands/orientation.rs`

**Principle**: The orientation prompt is the agent's FIRST interaction with braid after installation. Per trajectory-dynamics (prompt-optimization Rule 8), turns 1-2 establish the activation basin for the entire session. Orientation must be the optimal seed turn.

**Current**: Dumps a mixed text block of instructions and information.

**New**: Three-part structured output with demonstration-first content:

- **Context**: `"braid v{version} — append-only datom store for human/AI coherence verification"`
- **Content** (workflow demonstration, not command list):
  ```
  Session lifecycle:
    braid status              → F(S)=0.77, M(t)=0.82, next: trace 3 unwitnessed INVs
    braid observe "insight"   → entity :exploration/insight-hash (confidence: 0.7)
    braid harvest --commit    → 5 candidates → 12 datoms crystallized
    braid seed --inject AGENTS.md → context refreshed for next session

  Knowledge model: datom [entity, attribute, value, tx, op] — append-only, CRDT merge = set union
  ```
- **Footer**: `"Start: braid status | Full help: braid <command> --help"`

**Enhancement**: Detect if `.braid/` store exists and include live metrics in the demonstration (not placeholder values).

---

## Phase 3: Native Tri-Mode CommandOutput — Top 8 Commands (~200 LOC)

**Motivation**: The from_human() bridge is the single biggest quality gap in the prompt architecture. It collapses the three-part comonadic structure (context/content/footer) into flat text, destroying the information architecture that makes agent-mode output a well-formed prompt.

**Principle**: Make invalid output states unrepresentable (rust-formal-engineering Rule 1). Native CommandOutput with proper fields is a structural guarantee; from_human() is an escape hatch.

### Conversion Priority (by agent interaction frequency)

| Priority | Command | Why | Context (≤50 tok) | Footer (≤50 tok) |
|----------|---------|-----|------|--------|
| 1 | observe / note | Most called during work | "observed: {preview}" | "verify: braid query ..." |
| 2 | query | Primary inspection | "query: {preview}" | "refine: ... \| explore: braid schema" |
| 3 | bilateral | Coherence verification | "coherence: F(S)={f}" | "improve: {top_failing_CC action}" |
| 4 | log | Tx browsing | "log: {n} txns" | "detail: braid log --datoms --limit 1" |
| 5 | schema | Attribute discovery | "schema: {n} attrs" | "explore: braid query '[:find ...]'" |
| 6 | seed | Session start | "seed: {task}" | "start: braid session start" |
| 7 | config | Configuration | "config: {key}" | "set: braid config {key} {value}" |
| 8 | init | First interaction | "init: {path}" | "next: braid status" |

### Conversion Pattern (demonstrated for observe)

```rust
// Build structured JSON with actual fields (not {"output": "..."})
let json = serde_json::json!({
    "entity": entity_ident,
    "confidence": confidence,
    "category": category,
    "tx_id": tx_id,
});

let agent = AgentOutput {
    context: format!("observed: {}", truncate(&text, 40)),
    content: format!(
        "entity {} — confidence: {}, category: {}\ntx: {} ({} datoms)",
        entity_ident, confidence, category, tx_id, datom_count
    ),
    footer: format!(
        "verify: braid query '[:find ?v :where [{} :exploration/body ?v]]'",
        entity_ident
    ),
};

CommandOutput { json, agent, human: /* existing text output */ }
```

**Each conversion follows this pattern**: ~25 LOC per command. The json field gets structured data, the agent field gets the three-part prompt, the human field keeps the existing text output.

### Dispatch Updates

In `crates/braid/src/commands/mod.rs`, each converted command's dispatch arm changes from:
```rust
let output = some_fn::run(...)?;
// (falls through to from_human bridge)
```
to:
```rust
let cmd_output = some_fn::run(...)?;
return Ok(maybe_inject_footer(cmd_output, skip_footer, path.as_deref(), budget_ctx));
```

**Gate per command**: `braid <cmd> --format json | python3 -m json.tool` produces valid structured JSON. `braid <cmd> --format agent` shows context/content/footer (not flat text).

---

## Phase 4: Bootstrap Chain — "Use braid" End-to-End (~80 LOC)

**Motivation**: The complete chain from installation to productive use must work without gaps: `install → init → AGENTS.md exists → agent reads → agent uses braid`. Every link is a prompt for the next step.

### 4A: Auto-Create AGENTS.md as Optimal Seed Turn

**File**: `crates/braid/src/commands/init.rs` (around line 189-209)

**Principle**: The auto-created AGENTS.md is the agent's FIRST encounter with braid's methodology. Per trajectory-dynamics (prompt-optimization Rule 8), this is the seed turn that establishes the activation basin. Per prompt-optimization Rule 2, one demonstration beats seven constraints.

**When**: Neither AGENTS.md nor CLAUDE.md exists at project root.

**Content** (designed as a seed turn):
```markdown
# AGENTS.md

> Use braid — append-only knowledge store with coherence verification.

## Session Lifecycle

```bash
braid status                              # Where you are + next action
braid observe "insight" --confidence 0.8  # Capture knowledge
braid harvest --commit                    # End-of-session: knowledge → datoms
braid seed --inject AGENTS.md             # Refresh this section
```

## Dynamic Store Context

<braid-seed>
<!-- braid will inject dynamic context here on `braid seed --inject AGENTS.md` -->
</braid-seed>
```

**Why this content works (prompt-optimization analysis)**:
- Line 1: Identity in spec-language ("append-only", "coherence verification") → activates formal substrate
- Lines 3-8: ONE demonstration of full lifecycle → encodes workflow, syntax, flags, expected behavior in 4 commands
- Lines 10-13: Injection point → the self-bootstrap mechanism (C7) that keeps this file alive with store context

**After creation**: Init proceeds to inject seed into the new AGENTS.md, populating `<braid-seed>` with live store metrics. The agent reads this file and has immediate context.

**Implementation**:
```rust
// In init.rs, the inject_target section (around line 189-209):
// When neither AGENTS.md nor CLAUDE.md exists, create AGENTS.md
if inject_target.is_none() {
    let minimal_content = /* the content above */;
    std::fs::write(&agents_md, minimal_content)
        .map_err(|e| BraidError::io(e, &agents_md))?;
    out.push_str("  created: AGENTS.md (with <braid-seed> tags)\n");
    inject_target = Some(agents_md);
}
```

### 4B: Install Script Polish

**File**: `install.sh` (line 147)

**Current**: `"${BIN_DIR}/braid" --orientation 2>/dev/null` — dumps full JSON, overwhelming.

**New**: The install script IS a prompt for the human. Its job: get the human to say "Use braid" to their agent.
```sh
echo ""
echo "Braid installed successfully!"
echo "  Binary: ${BIN_DIR}/braid"
echo ""
echo "Quick start:"
echo "  cd your-project"
echo "  braid init              # detect environment, create store"
echo "  braid status            # see where you are"
echo ""
echo "Tell your AI agent: 'Use braid'"
```

Three elements: where (binary path), how (two commands), and the trigger phrase ("Use braid"). Minimal, memorable, actionable.

### 4C: End-to-End Chain Verification

```bash
# Create fresh project
mkdir /tmp/e2e-test && cd /tmp/e2e-test && git init
echo 'fn main() {}' > main.rs

# Step 1: Init (should create .braid/ + AGENTS.md + inject seed)
braid init
cat AGENTS.md  # Verify: <braid-seed> section populated with store context

# Step 2: Status (should show agent-mode output with footer, not JSON)
braid status

# Step 3: Observe (should confirm with structured output)
braid observe "test observation" --confidence 0.8

# Step 4: Harvest (should extract session knowledge)
braid harvest --commit

# Step 5: Seed inject (should refresh AGENTS.md)
braid seed --inject AGENTS.md
cat AGENTS.md  # Verify: <braid-seed> updated with harvest context
```

---

## Phase 5: Guidance Footer Enhancement — Demonstrative Comonadic Steering (~100 LOC)

**Motivation**: The guidance footer is the continuous comonadic steering signal (INV-GUIDANCE-001). It maintains P(Basin_A) > τ against drift toward pretrained patterns. Currently it shows commands; it should demonstrate outcomes. This is the difference between instructive and navigative guidance.

### 5A: Outcome-Demonstrative Actions

**File**: `crates/braid-kernel/src/guidance.rs` (derive_actions, lines 650-860; format_footer_at_level, lines 427-475)

**Current format**: `↳ M=0.82↑ | braid query [:find ...]`
**New format**: `↳ M=0.82↑ | 3 unwitnessed INVs → braid trace --commit`

**Principle**: A demonstration encodes more information per token than a constraint (prompt-optimization Rule 2). The current footer says WHAT to do. The new footer says WHY (rationale) + WHAT (command) + EXPECTED RESULT (outcome) — a micro-demonstration in ≤50 tokens.

**Implementation**: Enrich GuidanceAction with rationale:
```rust
struct GuidanceAction {
    command: String,       // "braid trace --commit"
    rationale: String,     // "3 unwitnessed INVs"
    priority: Priority,
}
```

In `format_footer_at_level`:
- Level 0 (Full, k>0.7): `↳ M=0.82↑ [transact: 0.90 spec: 0.85 query: 0.60 harvest: 0.90]\n  {rationale} → {command}`
- Level 1 (Compressed, k∈[0.4,0.7]): `↳ M=0.82↑ | {rationale} → {command}`
- Level 2 (Minimal, k∈[0.2,0.4]): `↳ M=0.82 → {command}`
- Level 3 (HarvestOnly, k≤0.2): `⚠ HARVEST: braid harvest --commit`

The rationale is what makes this navigative rather than instructive. "3 unwitnessed INVs" tells the agent WHY this action matters — activating understanding, not just compliance.

### 5B: Workflow Phase Detection

**File**: `crates/braid-kernel/src/guidance.rs` (derive_actions)

**Principle**: The agent's position in the workflow state machine determines what guidance is relevant (skill-composition Rule 1: sequence, don't stack).

**Implementation**: Detect workflow phase from store state:
```
FRESH:       no session entity        → "Start: braid session start --task '...'"
WORKING:     session active, <20 txns → work-specific guidance (spec gaps, tasks)
LATE:        session active, >30 txns → "Context filling — braid harvest --commit soon"
HARVEST_DUE: >50 txns or k_eff < 0.3 → "⚠ HARVEST: braid harvest --commit"
```

This maps to the attention budget model: as context fills, guidance shifts from work-specific to harvest-imperative. The agent experiences graceful pressure to crystallize knowledge before context loss.

### 5C: Demonstration Density in Seed

**File**: `crates/braid-kernel/src/seed.rs`

**Principle**: INV-SEED-005 requires that for every constraint cluster with ≥2 constraints and sufficient budget, at least one micro-demonstration is included. A 30-token demonstration activates ~10× its cost in behavioral quality.

**Enhancement**: When building the Constraints section, for clusters of ≥2 active INVs, include one micro-example:
```
Active: INV-STORE-001 (append-only), INV-STORE-003 (content-addressed)
  Demo: braid write assert --datom :e :db/doc "fact" → datom [E, :db/doc, "fact", TX, true]
```

---

## Session Scope & Execution Order

### This Session: Phases 0 + 1 + 2(A,B) + 4(A,B) — The Critical Path

**~245 LOC across 7 files. Establishes the "Use braid" chain + CLI surface optimization.**

```
Phase 0 (bugs, ~35 LOC)           ← ENTRY POINT
  ├── 0A: bilateral JSON fix
  ├── 0B: --skip implementation
  └── 0C: which → command -v
        │
Phase 1 (pipeline, ~70 LOC)       ← depends on Phase 0
  ├── 1A: Agent-first default
  └── 1B: Footer dedup
        │
Phase 2A,2B (surface, ~60 LOC)    ← depends on Phase 1
  ├── 2A: Help text demonstrations
  └── 2B: Status flag progressive disclosure
        │
Phase 4A,4B (bootstrap, ~80 LOC)  ← depends on Phase 0C
  ├── 4A: AGENTS.md auto-creation
  └── 4B: Install script polish
```

**Quality gates after each phase**: `cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test`

### Next Session: Phase 3 — Native Tri-Mode Conversion

Convert top 8 commands from from_human() bridge to native CommandOutput (~200 LOC). Clear pattern established; follows the observe example in Phase 3.

### Future Session: Phase 2(C,D) + Phase 5 — Deep Enhancement

- 2C: --orientation redesign as optimal seed turn
- 5A: Demonstrative guidance footer
- 5B: Workflow phase detection
- 5C: Demonstration density in seed

---

## Files Changed (This Session)

| File | Phases | Change Type | Net LOC |
|------|--------|-------------|---------|
| `crates/braid/src/commands/mod.rs` | 0A, 0B, 1B, 2A, 2B | Bug fix + Refactor + Help text | ~-20 |
| `crates/braid/src/output.rs` | 1A | Default + docstring | +8 |
| `crates/braid/src/main.rs` | 1A, 2A | Docstring + Help text | +10 |
| `crates/braid/src/commands/init.rs` | 0C, 4A | Bug fix + AGENTS.md creation | +45 |
| `crates/braid/src/commands/task.rs` | 0B | Bug fix (--skip) | +15 |
| `install.sh` | 4B | Polish | +5 |
| **Total this session** | | | **~+63 net** |

## Files Changed (All Sessions)

| File | Phases | Change Type | Net LOC |
|------|--------|-------------|---------|
| `crates/braid/src/commands/mod.rs` | 0A, 0B, 1B, 2A, 2B, 3 | Bug fix + Refactor + Help text + Dispatch | ~-10 |
| `crates/braid/src/output.rs` | 1A | Default + docstring | +8 |
| `crates/braid/src/main.rs` | 1A, 2A | Docstring + Help text | +10 |
| `crates/braid/src/commands/init.rs` | 0C, 4A | Bug fix + AGENTS.md creation | +45 |
| `crates/braid/src/commands/task.rs` | 0B | Bug fix (--skip) | +15 |
| `crates/braid/src/commands/observe.rs` | 3 | Native CommandOutput | +30 |
| `crates/braid/src/commands/bilateral.rs` | 3 | Native CommandOutput | +40 |
| `crates/braid/src/commands/orientation.rs` | 2C | Redesign | +30 |
| `crates/braid-kernel/src/guidance.rs` | 5A, 5B | Footer enhancement | +60 |
| `crates/braid-kernel/src/seed.rs` | 5C | Demo density | +30 |
| `install.sh` | 4B | Polish | +5 |
| + 6 more command files | 3 | Native CommandOutput | ~150 |
| **Total all sessions** | | | **~+413 net** |

---

## Verification Plan

### After Phase 0 (Bug Fixes)
```bash
cargo check --all-targets && cargo clippy --all-targets -- -D warnings && cargo test
# Bilateral JSON not corrupted:
cargo run --quiet -- bilateral --format json --path .braid 2>/dev/null | python3 -m json.tool
```

### After Phase 1 (Prompt Pipeline)
```bash
cargo test
# Agent-mode output (not JSON) when piped:
cargo run --quiet -- status --path .braid 2>/dev/null | head -5
# Should show "store: .braid (...)" with guidance footer
```

### After Phase 2 (CLI Surface)
```bash
# Help text shows demonstrations:
cargo run --quiet -- --help 2>&1 | head -20
cargo run --quiet -- status --help 2>&1
# --deep implies --full --spectral:
cargo run --quiet -- status --deep --path .braid 2>/dev/null
```

### After Phase 4 (Bootstrap Chain)
```bash
cd /tmp && rm -rf e2e-braid && mkdir e2e-braid && cd e2e-braid && git init
echo 'fn main() {}' > main.rs
cargo run --manifest-path /data/projects/ddis/ddis-braid/Cargo.toml --quiet -- init
cat AGENTS.md  # Must exist with populated <braid-seed>
cargo run --manifest-path /data/projects/ddis/ddis-braid/Cargo.toml --quiet -- status
# Must show agent-mode dashboard with guidance footer
```

### Full Quality Gates (Every Phase)
```bash
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test
```

---

## Formal Traceability

| Change | Spec Element | Principle |
|--------|-------------|-----------|
| Agent-first default | INV-INTERFACE-001, ADR-INTERFACE-002 | Primary consumer is AI agent |
| Footer dedup → `maybe_inject_footer` | INV-GUIDANCE-001 | Structural guarantee > procedural obligation |
| AGENTS.md auto-creation | C7 (self-bootstrap), INV-SEED-001 | Bootstrap chain must be complete |
| Help text as demonstrations | ADR-INTERFACE-002 (demonstration style) | Demonstrations > constraints |
| Status flag progressive disclosure | INV-BUDGET-006 (info density) | Reduce mid-DoF saddle |
| Native CommandOutput | INV-OUTPUT-002 (three-part structure) | Make invalid states unrepresentable |
| Demonstrative footer | INV-GUIDANCE-001, ADR-GUIDANCE-005 | Navigative > instructive |
| Bilateral JSON fix | INV-INTERFACE-001 | is_json_output() must be total |
| --skip implementation | rust-formal-eng Rule 3 | Dead code = violated proof obligation |
| POSIX command -v | INV-INTERFACE-009 (portability) | Boundary constraint |

---

## What This Plan Does NOT Do (and Why)

1. **Convert all 22 commands to native CommandOutput this session** — Phase 3 (next session) converts the top 8 by impact. The from_human() bridge is functional. Full conversion follows the established pattern incrementally.

2. **Redesign the Datalog query syntax** — The `[:find ?e :where ...]` syntax is kernel-deep. Changing it is a separate research problem. The syntax works.

3. **Add MCP tools beyond Stage 0's 6** — INV-INTERFACE-003 explicitly limits Stage 0. MCP expansion is a separate session with per-tool schema design.

4. **Release CI for pre-built binaries** — GitHub Actions + cross-compilation is infrastructure work. install.sh already falls back to `cargo install` gracefully.

5. **Learned guidance effectiveness** — Tracking which footer recommendations led to productive agent actions requires store schema additions and longitudinal measurement. Stage 1+ work.

6. **TUI (Layer 4)** — The spec defers this to Stage 2+. Layer 4 is the human monitoring interface, not the agent interface we're optimizing.

7. **Error algebra completeness audit (Phase 2C)** — The ErrorInfo struct and four-part model exist. A full audit of all variants is valuable but lower priority than the structural changes. Can be folded into Phase 3 or a future session.
