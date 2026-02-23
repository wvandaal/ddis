#!/usr/bin/env bash
set -euo pipefail

# Allow nested claude -p calls when run from within a Claude Code session.
# Claude Code sets env vars for nesting detection — unset them all.
unset CLAUDECODE 2>/dev/null || true
unset CLAUDE_CODE_ENTRYPOINT 2>/dev/null || true
unset CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS 2>/dev/null || true
for var in $(env | grep -o '^CLAUDE_CODE[^=]*'); do
    unset "$var" 2>/dev/null || true
done

# Use Claude Max subscription auth, not API key (which may have low/no credits)
unset ANTHROPIC_API_KEY 2>/dev/null || true

# ═══════════════════════════════════════════════════════════════════════════════
# DDIS RALPH Loop: Recursive Autonomous LLM-driven Progressive Honing
# ═══════════════════════════════════════════════════════════════════════════════
#
# Uses Claude Code (`claude -p`) to recursively improve the DDIS specification
# using its own methodology. The loop works on MODULAR files (self-bootstrapping
# per §0.13: the spec exceeds 2,500 lines and must demonstrate modularization).
#
# Each iteration:
#   1. AUDIT:   LLM reads modular spec + methodology → creates tasks in beads (br)
#   2. APPLY:   LLM picks tasks via bv triage → surgical edits on modules → closes tasks
#   3. JUDGE:   Separate LLM evaluates versioned modular snapshots
#   4. DECIDE:  Script checks multi-signal stopping condition
#
# Post-loop: Assemble modular files → monolith (mechanical concatenation)
#
# Stopping condition (three independent signals):
#   Signal 1: DIMINISHING RETURNS — substantive improvements below threshold
#   Signal 2: QUALITY PLATEAU — score delta below threshold
#   Signal 3: REGRESSION DETECTION — quality gates regressed
#   Safety valve: MAX_ITERATIONS caps total runs
#
# The judge is a SEPARATE call, preventing self-evaluation bias.
# ═══════════════════════════════════════════════════════════════════════════════

# ─── Configuration ────────────────────────────────────────────────────────────

MAX_ITERATIONS=${DDIS_MAX_ITERATIONS:-5}
MIN_SUBSTANTIVE_IMPROVEMENTS=${DDIS_MIN_IMPROVEMENTS:-2}
MIN_QUALITY_DELTA=${DDIS_MIN_DELTA:-3}
IMPROVER_MODEL=${DDIS_IMPROVER_MODEL:-"opus"}
JUDGE_MODEL=${DDIS_JUDGE_MODEL:-"opus"}
POLISH_ON_EXIT=${DDIS_POLISH:-true}
USE_BEADS=${DDIS_USE_BEADS:-auto}
VERBOSE=${DDIS_VERBOSE:-false}

# ─── CLI Arguments ────────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --verbose)      VERBOSE=true; shift ;;
        --no-polish)    POLISH_ON_EXIT=false; shift ;;
        *)              echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

# ─── Path Setup ──────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK_DIR="${SCRIPT_DIR}/ddis-evolution"
VERSIONS_DIR="${WORK_DIR}/versions"
JUDGMENTS_DIR="${WORK_DIR}/judgments"
LOGS_DIR="${WORK_DIR}/logs"

MODULAR_DIR="${SCRIPT_DIR}/ddis-modular"
SEED_SPEC="${SCRIPT_DIR}/ddis_standard.md"
IMPROVEMENT_PROMPT="${SCRIPT_DIR}/ddis_recursive_improvement_prompt.md"
KICKOFF_PROMPT="${SCRIPT_DIR}/kickoff_prompt.md"

# ─── Claude -p Flags ─────────────────────────────────────────────────────────
#
# All calls disable MCP servers (btca hangs during shutdown).
# Built-in tools (Read, Write, Edit, Glob, Grep, Bash) still work.

MCP_FLAGS=(--strict-mcp-config --mcp-config '{"mcpServers":{}}')

# Debug flag: --debug-file writes logs without changing output format
CLAUDE_DEBUG_FLAGS=()
if [[ "$VERBOSE" == "true" ]]; then
    mkdir -p "${WORK_DIR}/logs"
    CLAUDE_DEBUG_FLAGS=(--debug-file "${LOGS_DIR}/claude_debug.log")
fi

# Per-call-type tuning (audit + apply replace the old monolithic improve step)

JUDGE_MAX_TURNS=100
JUDGE_TIMEOUT=2400     # 40 minutes

POLISH_MAX_TURNS=50
POLISH_TIMEOUT=1800    # 30 minutes

MODULARIZE_MAX_TURNS=80
MODULARIZE_TIMEOUT=2400  # 40 minutes

# ─── Helpers ──────────────────────────────────────────────────────────────────

timestamp() { date '+%Y-%m-%d %H:%M:%S'; }
epoch_secs() { date '+%s'; }
log() { echo "[$(timestamp)] $*"; }

log_section() {
    echo ""
    echo "═══════════════════════════════════════════════════════════════════"
    echo "  $*"
    echo "═══════════════════════════════════════════════════════════════════"
    echo ""
}

line_count() { wc -l < "$1" | tr -d ' '; }

# Format elapsed seconds as "Xm Ys"
fmt_elapsed() {
    local secs=$1
    if [[ $secs -ge 60 ]]; then
        printf "%dm %ds" $((secs / 60)) $((secs % 60))
    else
        printf "%ds" "$secs"
    fi
}

judgment_field() {
    local file="$1" field="$2"
    jq -r ".$field" "$file"
}

# Extract .result from claude -p --output-format json
extract_result() {
    local raw="$1"
    echo "$raw" | jq -r '.result // empty' 2>/dev/null || true
}

# Extract .session_id from claude -p --output-format json
extract_session_id() {
    local raw="$1"
    echo "$raw" | jq -r '.session_id // empty' 2>/dev/null || true
}

# ─── Preflight ────────────────────────────────────────────────────────────────

check_prereqs() {
    local missing=0
    command -v claude &>/dev/null || { echo "ERROR: 'claude' not found." >&2; missing=1; }
    command -v jq &>/dev/null    || { echo "ERROR: 'jq' not found." >&2; missing=1; }
    for f in "$SEED_SPEC" "$IMPROVEMENT_PROMPT" "$KICKOFF_PROMPT"; do
        [[ -f "$f" ]] || { echo "ERROR: Not found: $f" >&2; missing=1; }
    done
    [[ $missing -ne 0 ]] && exit 1
    return 0
}

# ─── Beads Integration ────────────────────────────────────────────────────────
#
# br/bv are the TASK BACKBONE of the improvement loop. The audit agent creates
# granular improvement tasks directly via `br create`. The apply agent picks
# them off via `br ready` / `bv --robot-triage` and closes them via `br close`.
# Issues persist across iterations AND across RALPH runs.

BEADS_AVAILABLE=false
BV_AVAILABLE=false

check_beads() {
    [[ "$USE_BEADS" == "no" ]] && { log "Beads: disabled"; return; }
    command -v br &>/dev/null && BEADS_AVAILABLE=true
    command -v bv &>/dev/null && BV_AVAILABLE=true
    if [[ "$USE_BEADS" == "yes" ]] && ! $BEADS_AVAILABLE; then
        echo "ERROR: DDIS_USE_BEADS=yes but 'br' not found." >&2; exit 1
    fi
    $BEADS_AVAILABLE && log "Beads: br=$(which br)${BV_AVAILABLE:+, bv=$(which bv)}"
}

beads_init() {
    $BEADS_AVAILABLE || return 0
    if [[ ! -f "${WORK_DIR}/.beads/beads.db" ]]; then
        log "BEADS: Initializing workspace"
        (cd "$WORK_DIR" && br init --quiet 2>/dev/null) || true
    else
        log "BEADS: Reusing workspace ($(cd "$WORK_DIR" && br count 2>/dev/null || echo '?') issues)"
    fi
}

beads_open_count() {
    $BEADS_AVAILABLE || { echo "0"; return 0; }
    (cd "$WORK_DIR" && br count --status open 2>/dev/null \
        | grep -oP '\d+' | head -1) || echo "0"
}

beads_finalize() {
    $BEADS_AVAILABLE || return 0
    log "BEADS: Finalizing"
    (cd "$WORK_DIR" && br sync --flush-only --quiet 2>/dev/null) || true
    local open_count
    open_count=$(beads_open_count)
    [[ "$open_count" -gt 0 ]] && \
        log "BEADS: $open_count issues remain open. Run: cd ${WORK_DIR} && bv --robot-triage"
}

# ─── Self-Bootstrapping Preamble ─────────────────────────────────────────────
#
# Included in EVERY agent prompt. The spec must conform to the format it defines.

SELF_BOOTSTRAP_PREAMBLE="## SELF-BOOTSTRAPPING REQUIREMENT

This specification defines a standard that it MUST itself conform to. Every invariant
it prescribes must be satisfied BY the spec. Every quality gate it defines must be
passed BY the spec. Every structural requirement it mandates — including modularization
for documents exceeding 2,500 lines (§0.13) — must be demonstrated BY the spec.

You are working on the MODULAR form of the spec: a system constitution with declarations,
plus domain modules with full definitions. This IS the self-bootstrapping demonstration
of §0.13. The modular invariants (INV-011 through INV-016) apply to THIS decomposition.

You are not just improving content — you are ensuring the spec practices what it preaches."

# ─── Modular Working Form ───────────────────────────────────────────────────
#
# The loop operates on modular files, not a monolith. If no modular form
# exists, decompose the seed first. Assembly to monolith is the post-loop step.

MODULAR_FILES=(
    "${MODULAR_DIR}/constitution/system.md"
    "${MODULAR_DIR}/modules/core-standard.md"
    "${MODULAR_DIR}/modules/element-specifications.md"
    "${MODULAR_DIR}/modules/modularization.md"
    "${MODULAR_DIR}/modules/guidance-operations.md"
)
MANIFEST_FILE="${MODULAR_DIR}/manifest.yaml"

# Build a file listing string for agent prompts
modular_file_listing() {
    local idx=1
    echo "- **System constitution**: ${MODULAR_DIR}/constitution/system.md"
    for mod in "${MODULAR_DIR}/modules"/*.md; do
        [[ -f "$mod" ]] && echo "- **Module**: $mod"
    done
    echo "- **Manifest**: ${MANIFEST_FILE}"
}

# Total lines across all modular files (excluding manifest)
modular_total_lines() {
    local total=0
    for f in "${MODULAR_FILES[@]}"; do
        [[ -f "$f" ]] && total=$((total + $(line_count "$f")))
    done
    echo "$total"
}

# Snapshot the modular dir to a versioned copy
snapshot_modular() {
    local version=$1
    local dest="${VERSIONS_DIR}/v${version}"
    mkdir -p "$dest"
    cp -r "${MODULAR_DIR}/constitution" "${MODULAR_DIR}/modules" "${MODULAR_DIR}/manifest.yaml" "$dest/" 2>/dev/null || true
    log "SNAPSHOT: v${version} saved to $dest ($(modular_total_lines) lines)"
}

# Check if modular form exists and is populated
modular_form_exists() {
    [[ -f "${MODULAR_DIR}/constitution/system.md" ]] && \
    [[ -f "${MODULAR_DIR}/modules/core-standard.md" ]] && \
    [[ $(modular_total_lines) -ge 500 ]]
}

# Assemble modular files → monolith (mechanical concatenation)
run_assemble() {
    local output="$1"
    log "ASSEMBLE: Building monolith from modular files"
    {
        cat "${MODULAR_DIR}/constitution/system.md"
        echo ""
        for mod in "${MODULAR_DIR}/modules"/*.md; do
            [[ -f "$mod" ]] && { echo ""; cat "$mod"; }
        done
    } > "$output"
    local lines
    lines=$(line_count "$output")
    log "ASSEMBLE: Produced monolith ($lines lines)"
}

# ─── Bootstrap: Ensure Modular Working Form ─────────────────────────────────
#
# If no modular form exists (first run), decompose the monolithic seed.
# If modular form already exists (subsequent runs), use it directly.
# This is the self-bootstrapping entry point: the loop ALWAYS works on modular files.

ensure_modular_form() {
    if modular_form_exists; then
        local lines
        lines=$(modular_total_lines)
        log "MODULAR: Using existing modular form ($lines lines across modules)"
        return 0
    fi

    log_section "BOOTSTRAP: Decomposing monolith into modular form"
    log "MODULAR: No modular form found — decomposing seed spec"

    # Ensure directory structure
    mkdir -p "${MODULAR_DIR}/constitution" "${MODULAR_DIR}/modules"

    # Use run_modularize to do the decomposition
    run_modularize "$SEED_SPEC"

    if modular_form_exists; then
        local lines
        lines=$(modular_total_lines)
        log "MODULAR: Bootstrap complete ($lines lines across modules)"
    else
        log "ERROR: Bootstrap failed — modular form not created. Cannot continue."
        exit 1
    fi
}

# ─── Improve Step (Two-Phase: Audit → Apply) ────────────────────────────────
#
# Phase 1 — AUDIT: LLM reads spec + methodology, systematically evaluates it
#   against its own criteria, and creates improvement tasks directly in beads
#   via the `br` CLI. Reviews its own work for thoroughness. Pure analysis.
#
# Phase 2 — APPLY: LLM reads spec + picks tasks from beads via `br ready` /
#   `bv --robot-triage`, applies targeted edits using Edit tool, and closes
#   each task via `br close` when done. Surgical changes, no full rewrite.
#
# This replaces the monolithic "rewrite entire spec" approach that timed out
# on 3,000+ line specs.

AUDIT_TIMEOUT=2400   # 40 minutes — deep analysis + beads creation
AUDIT_MAX_TURNS=80
APPLY_TIMEOUT=2400   # 40 minutes — surgical edits
APPLY_MAX_TURNS=100

# ─── Phase 1: Audit ─────────────────────────────────────────────────────────

run_audit() {
    local iteration=$1
    local current_lines="$2"
    local prev_judgment="${3:-}"
    local log_file="${LOGS_DIR}/audit_v${iteration}.log"

    log "AUDIT: Analyzing v$((iteration - 1)) ($current_lines lines across modules)"

    # Build judgment context for iterations > 1
    local judgment_section=""
    if [[ -n "$prev_judgment" && -f "$prev_judgment" ]]; then
        judgment_section="3. **Judge's assessment of v$((iteration - 1))**: ${prev_judgment}
   Read this carefully — it tells you what the judge found wrong. Address every gap and regression."
    fi

    local file_listing
    file_listing=$(modular_file_listing)

    local prompt="You are the AUDITOR in iteration $iteration of the DDIS recursive self-improvement loop.
Your job is ANALYSIS ONLY — systematically evaluate the spec against its own criteria and create
granular improvement tasks using the \`br\` (beads) CLI. Do NOT edit the spec.

${SELF_BOOTSTRAP_PREAMBLE}

## Files to Read (use the Read tool)

1. **Improvement methodology**: ${IMPROVEMENT_PROMPT}
   This describes the audit framework, quality criteria, and anti-patterns. Read it first.

2. **DDIS spec (v$((iteration - 1))) — MODULAR FORM** (~${current_lines} total lines):
${file_listing}

   Read ALL files. The constitution has declarations; modules have full definitions.

${judgment_section}

## Beads Issue Tracker

The beads workspace is at: ${WORK_DIR}
Use Bash to run \`br\` and \`bv\` commands. Always \`cd ${WORK_DIR}\` first.

**Before creating new tasks**, check what already exists:
\`\`\`bash
cd ${WORK_DIR}
br list --status open --json    # See existing open issues
bv --robot-triage 2>/dev/null   # Dependency-aware prioritization (if bv available)
\`\`\`

## Your Task

Read the ENTIRE spec thoroughly. Systematically evaluate it against:

1. **Self-Conformance**: Does the spec satisfy its own invariants (INV-001 through INV-020)?
   Check EACH invariant. For every violation, create a task.
2. **Quality Gates**: Does it pass Gates 1-7? For every gate failure, create a task.
3. **LLM Optimization**: Is it maximally effective for LLM consumption? Structure, density,
   navigation aids, context efficiency.
4. **Structural completeness**: All required elements present per the spec's own template?
5. **Cross-references**: All valid? No orphan sections (INV-006)?
6. **Negative specifications**: Enough DO NOT constraints per chapter (INV-017)?
7. **Verification prompts**: Every element spec chapter has a verification block (INV-020)?
8. **Modular self-conformance** (the spec prescribes modularization — does it practice it?):
   - INV-011: Each module bundle (constitution + module) sufficient for standalone work?
   - INV-012: Cross-module references go through constitution, not direct section refs?
   - INV-013: Each invariant maintained by exactly one module?
   - INV-014: Bundle sizes within hard ceiling (constitution + any module < 5,000 lines)?
   - INV-015: Constitution declarations faithfully summarize module definitions?
   - INV-016: Manifest reflects current file state (line counts, module list)?

## Creating Tasks

For each improvement needed, use the \`br\` CLI:

\`\`\`bash
cd ${WORK_DIR} && br create \"<concise title>\" \\
  --type task \\
  --priority <0-3> \\
  --description \"FILE: <full path to the module file to edit>
SECTION: <section within that file>
PROBLEM: <what is wrong or missing>
FIX: <specific description of what to change>
INVARIANTS: <INV-NNN IDs this addresses>\" \\
  --labels \"ralph-iter-${iteration},<category>\"
\`\`\`

**IMPORTANT**: Every task description MUST specify which FILE to edit. Tasks affecting the
constitution go to \`${MODULAR_DIR}/constitution/system.md\`. Tasks affecting specific
content go to the relevant module. Tasks affecting structure go to the manifest.

Priority guide:
- P0: Critical invariant violation or broken self-bootstrapping
- P1: Significant gap (missing required element, quality gate failure)
- P2: Meaningful structural improvement
- P3: Minor polish (only if it directly serves an invariant)

Category labels: \`self-conformance\`, \`quality-gate\`, \`llm-optimization\`, \`structural\`, \`cross-ref\`, \`negative-spec\`, \`verification\`

After creating tasks, wire up dependencies where one fix depends on another:
\`\`\`bash
cd ${WORK_DIR} && br dep add <task-id> <depends-on-id>
\`\`\`

## Self-Review

After creating all tasks, review your own work:
\`\`\`bash
cd ${WORK_DIR} && br list --status open --json | jq length    # Total open
cd ${WORK_DIR} && br ready --json | jq length                 # Ready (unblocked)
cd ${WORK_DIR} && bv --robot-triage 2>/dev/null | jq '.quick_ref' 2>/dev/null
\`\`\`

Ask yourself:
- Did I check EVERY invariant (INV-001 through INV-020)?
- Did I check every quality gate (Gates 1-7)?
- Are my task descriptions specific enough that another LLM could implement each one
  without reading the audit? (Section, problem, exact fix, affected invariants)
- Are dependencies correct? (e.g., \"add INV-021\" must happen before \"update namespace note\")
- Did I miss anything? If so, create additional tasks.

Target 8-20 tasks. Focus on SUBSTANCE over cosmetics."

    log "AUDIT: Starting claude -p (model=$IMPROVER_MODEL, max_turns=$AUDIT_MAX_TURNS, timeout=${AUDIT_TIMEOUT}s)"

    local start_secs
    start_secs=$(epoch_secs)

    local raw_output=""
    raw_output=$(echo "$prompt" | timeout "$AUDIT_TIMEOUT" \
        claude -p "${CLAUDE_DEBUG_FLAGS[@]}" "${MCP_FLAGS[@]}" \
            --model "$IMPROVER_MODEL" \
            --output-format json \
            --max-turns "$AUDIT_MAX_TURNS" \
            --permission-mode bypassPermissions \
            2>"$log_file") || {
        local exit_code=$?
        if [[ $exit_code -eq 124 ]]; then
            log "AUDIT: TIMEOUT after $((AUDIT_TIMEOUT / 60)) minutes"
        else
            log "AUDIT: claude -p exited with code $exit_code"
        fi
    }

    local elapsed=$(( $(epoch_secs) - start_secs ))
    log "AUDIT: Elapsed: $(fmt_elapsed $elapsed)"

    local session_id
    session_id=$(extract_session_id "$raw_output") || true
    [[ -n "$session_id" ]] && log "AUDIT: Session: ${session_id}"

    # Check how many issues were created
    local open_count
    open_count=$(beads_open_count)
    log "AUDIT: $open_count open issues in tracker"

    if [[ "$open_count" == "0" ]]; then
        log "WARNING: Audit created no issues"
        return 1
    fi

    return 0
}

# ─── Phase 2: Apply ─────────────────────────────────────────────────────────

run_apply() {
    local iteration=$1
    local log_file="${LOGS_DIR}/apply_v${iteration}.log"

    local current_lines
    current_lines=$(modular_total_lines)

    local open_count
    open_count=$(beads_open_count)
    log "APPLY: $open_count open issues to address on v$((iteration - 1)) ($current_lines lines across modules)"

    local file_listing
    file_listing=$(modular_file_listing)

    local prompt="You are the APPLIER in iteration $iteration of the DDIS recursive self-improvement loop.
Your job is to apply improvements to the spec by working through the beads issue tracker.

${SELF_BOOTSTRAP_PREAMBLE}

## Files — MODULAR FORM

The spec is decomposed into modular files. Each beads task specifies which file to edit.

${file_listing}

Read the relevant files as needed. Edit them in place using the Edit tool.

**Improvement methodology** (for context): ${IMPROVEMENT_PROMPT}

## Beads Issue Tracker

The beads workspace is at: ${WORK_DIR}
Use Bash to run \`br\` and \`bv\` commands. Always \`cd ${WORK_DIR}\` first.

**Start by getting your prioritized work list:**
\`\`\`bash
cd ${WORK_DIR} && bv --robot-triage 2>/dev/null || br ready --json
\`\`\`

## Workflow for Each Task

1. **Pick the next task** (highest priority, unblocked):
   \`\`\`bash
   cd ${WORK_DIR} && br ready --json --limit 1
   \`\`\`

2. **Read its details**:
   \`\`\`bash
   cd ${WORK_DIR} && br show <id>
   \`\`\`

3. **Mark in-progress**:
   \`\`\`bash
   cd ${WORK_DIR} && br update <id> --status in_progress
   \`\`\`

4. **Read the relevant file** specified in the task description using the Read tool

5. **Apply the fix** using the Edit tool:
   - Find the exact text to change in the specified module file
   - Use Edit with precise old_string and new_string
   - If the task affects the constitution, update declarations there too
   - If the task adds/changes invariants, update the manifest
   - Verify cross-references remain valid after your edit
   - If the edit would break self-bootstrapping, skip it

6. **Close the task**:
   \`\`\`bash
   cd ${WORK_DIR} && br close <id> --reason \"Applied: <brief description of what was done>\"
   \`\`\`

7. **Repeat** until all ready tasks are done or you've run out of time

## CRITICAL RULES

- **Use the Edit tool**, NOT the Write tool. Make surgical edits to specific sections.
  The Edit tool replaces exact string matches. This preserves everything you don't touch.
- **NEVER rewrite the entire file.** Only change what needs changing.
- The spec MUST remain self-bootstrapping (conform to the format it defines)
- Do NOT increase total document length by more than 20%
- Do NOT make cosmetic-only changes — every edit must serve an invariant or quality gate
- If two tasks conflict, prefer the higher-priority one and close the other with a note
- Update namespace notes, TODOs, and cross-references affected by your edits
- After completing all tasks, run \`br list --status open\` and report what remains

## After All Edits

Provide a brief summary of:
- How many tasks you completed vs skipped
- Key changes made
- Any tasks that couldn't be applied and why"

    log "APPLY: Starting claude -p (model=$IMPROVER_MODEL, max_turns=$APPLY_MAX_TURNS, timeout=${APPLY_TIMEOUT}s)"

    local start_secs
    start_secs=$(epoch_secs)

    local raw_output=""
    raw_output=$(echo "$prompt" | timeout "$APPLY_TIMEOUT" \
        claude -p "${CLAUDE_DEBUG_FLAGS[@]}" "${MCP_FLAGS[@]}" \
            --model "$IMPROVER_MODEL" \
            --output-format json \
            --max-turns "$APPLY_MAX_TURNS" \
            --permission-mode bypassPermissions \
            2>"$log_file") || {
        local exit_code=$?
        if [[ $exit_code -eq 124 ]]; then
            log "APPLY: TIMEOUT after $((APPLY_TIMEOUT / 60)) minutes"
        else
            log "APPLY: claude -p exited with code $exit_code"
        fi
    }

    local elapsed=$(( $(epoch_secs) - start_secs ))
    log "APPLY: Elapsed: $(fmt_elapsed $elapsed)"

    local session_id
    session_id=$(extract_session_id "$raw_output") || true
    [[ -n "$session_id" ]] && log "APPLY: Session: ${session_id}"

    # Report beads status after apply
    local remaining
    remaining=$(beads_open_count)
    local closed=$((open_count - remaining))
    log "APPLY: Closed $closed issues, $remaining remain open"

    # Check modular files still exist and have content
    local new_lines
    new_lines=$(modular_total_lines)

    if [[ $new_lines -ge 500 ]]; then
        local min_acceptable=$(( current_lines * 70 / 100 ))
        if [[ $new_lines -lt $min_acceptable ]]; then
            log "APPLY: WARNING — modular total shrank to $new_lines lines (<70% of $current_lines)"
        fi
        log "APPLY: Produced v${iteration} ($new_lines lines, delta: $((new_lines - current_lines)))"

        # Snapshot the modular form for this version
        snapshot_modular "$iteration"
        return 0
    fi

    log "ERROR: Apply failed — modular files missing or too short ($new_lines lines)"
    log "       Session: ${session_id:-unknown}"
    log "       Log: $log_file"
    return 1
}

# ─── Combined Improve Step (Audit → Apply) ──────────────────────────────────

run_improve() {
    local iteration=$1
    local prev_judgment="${2:-}"

    local current_lines
    current_lines=$(modular_total_lines)
    log "IMPROVE: v$((iteration - 1)) ($current_lines lines) → v${iteration} [audit → apply on modular form]"

    # Phase 1: Audit — create improvement tasks in beads
    if ! run_audit "$iteration" "$current_lines" "$prev_judgment"; then
        log "ERROR: Audit failed at iteration $iteration — no improvements identified"
        return 1
    fi

    # Phase 2: Apply — work through beads tasks with surgical edits on module files
    if ! run_apply "$iteration"; then
        log "ERROR: Apply failed at iteration $iteration"
        return 1
    fi

    return 0
}

# ─── Judge Step ───────────────────────────────────────────────────────────────
#
# Separate LLM evaluates whether version N is better than N-1.
# Outputs raw JSON via prompt instruction (structured output via prompt, not flag).

run_judge() {
    local iteration=$1
    local output_json="$2"
    local log_file="${LOGS_DIR}/judge_v${iteration}.log"

    local prev_dir="${VERSIONS_DIR}/v$((iteration - 1))"
    local curr_dir="${VERSIONS_DIR}/v${iteration}"
    local prev_lines curr_lines
    prev_lines=$(wc -l "${prev_dir}"/constitution/*.md "${prev_dir}"/modules/*.md 2>/dev/null | tail -1 | awk '{print $1}')
    curr_lines=$(wc -l "${curr_dir}"/constitution/*.md "${curr_dir}"/modules/*.md 2>/dev/null | tail -1 | awk '{print $1}')
    log "JUDGE: Comparing v$((iteration - 1)) ($prev_lines lines) vs v${iteration} ($curr_lines lines) [modular]"

    # Build judge prompt — three evaluation dimensions:
    #   (a) Self-bootstrapping: how well does the spec conform to itself?
    #   (b) Comparison: is it better than the previous version?
    #   (c) LLM optimization: how well does it serve LLM generation/implementation goals?

    local comparison_context=""
    local recommendation_context=""
    if [[ $iteration -eq 1 ]]; then
        comparison_context="This is the FIRST iteration. v0 is the unmodified seed spec.
There is no prior score to compare against. Evaluate v1 on absolute quality."
        recommendation_context="- **continue**: Score < 90 AND >= ${MIN_SUBSTANTIVE_IMPROVEMENTS} substantive improvements AND remaining gaps are addressable
- **stop_converged**: < ${MIN_SUBSTANTIVE_IMPROVEMENTS} substantive improvements (v1 barely differs from seed)
- **stop_regressed**: v1 is WORSE than v0 — missing sections, broken cross-refs, content loss
- **stop_excellent**: Score >= 95 AND no critical remaining gaps"
    else
        comparison_context="Compare v${iteration} against v$((iteration - 1)). Both absolute quality and relative improvement matter."
        recommendation_context="- **continue**: Score improved by >= ${MIN_QUALITY_DELTA} AND >= ${MIN_SUBSTANTIVE_IMPROVEMENTS} substantive improvements AND gaps addressable
- **stop_converged**: Score improved by < ${MIN_QUALITY_DELTA} OR < ${MIN_SUBSTANTIVE_IMPROVEMENTS} improvements
- **stop_regressed**: Regressions outweigh improvements — keep previous version
- **stop_excellent**: Score >= 95 AND no critical remaining gaps"
    fi

    local prompt="You are the JUDGE in a recursive self-improvement loop for the DDIS specification.

${SELF_BOOTSTRAP_PREAMBLE}

## Files to Read (use the Read tool)

Read BOTH versions completely — they are in modular form (constitution + modules).

**Previous version (v$((iteration - 1))):**
$(ls "${prev_dir}"/constitution/*.md "${prev_dir}"/modules/*.md 2>/dev/null | sed 's/^/- /')

**Current version (v${iteration}):**
$(ls "${curr_dir}"/constitution/*.md "${curr_dir}"/modules/*.md 2>/dev/null | sed 's/^/- /')

**Improvement methodology**: ${IMPROVEMENT_PROMPT}

## Your Role

You are an independent assessor — NOT the author. Be rigorous. Do not credit cosmetic changes.
Evaluate v${iteration} across THREE dimensions:

${comparison_context}

### Dimension A: Self-Bootstrapping Adherence (40% of score)

The DDIS spec defines a format that it must itself conform to. How well does it eat its own dogfood?

- Check EACH invariant INV-001 through INV-020. For every one: does the spec satisfy it?
- Check quality gates 1-7. Does the spec pass each one?
- Are all required structural elements present (per §0.3 template)?
- Is the namespace note accurate? Do counts match reality?
- Does the Master TODO reflect the actual state of the document?
- Are cross-references valid (no broken links, no orphan sections)?

### Dimension B: Version Comparison (30% of score)

- What SUBSTANTIVE improvements were made? (New invariants, fixed violations, new sections,
  improved coverage — NOT rewording, reformatting, or cosmetic changes)
- What REGRESSIONS occurred? (Missing content, broken cross-refs, weakened invariants,
  lost sections, reduced coverage)
- Net: is v${iteration} objectively better than v$((iteration - 1))?

### Dimension C: LLM Optimization Goals (30% of score)

This spec's primary consumer is an LLM. How well does it serve these goals:

1. **Context efficiency**: Can an LLM load what it needs without loading everything?
   Cross-reference density, modular structure, section independence.
2. **Implementation clarity**: Could an LLM produce a correct v1 implementation from this
   spec alone, with zero clarifying questions? Worked examples, pseudocode, edge cases.
3. **Self-validation**: Can an LLM verify its own output against the spec? Verification
   prompts, falsifiable invariants, concrete violation scenarios.
4. **Iterative authoring**: Does the spec support efficient iterative improvement?
   Clear audit criteria, measurable quality, explicit gaps.
5. **Resistance to drift**: Does the spec prevent hallucination and content loss during
   LLM rewrites? Structural redundancy, namespace notes, checksums.

## Scoring Guide

- 0-30: Fundamentally broken (doesn't self-bootstrap, missing major sections)
- 31-50: Structural gaps (missing invariants, ADRs, or required elements)
- 51-70: Functional but incomplete
- 71-85: Good (complete, most invariants satisfied)
- 86-95: Excellent (comprehensive, self-conforming, LLM-optimized)
- 96-100: Near-perfect (reserve this — requires passing ALL invariants and gates)

## Recommendation Logic

${recommendation_context}

## Output Format — CRITICAL

Read both versions THOROUGHLY, then output ONLY a single raw JSON object. No markdown fences,
no explanation before or after — JUST the JSON. The object must have exactly these fields:

{
  \"quality_score\": <integer 0-100>,
  \"self_bootstrap_score\": <integer 0-100>,
  \"comparison_score\": <integer 0-100>,
  \"llm_optimization_score\": <integer 0-100>,
  \"substantive_improvements\": <integer>,
  \"regressions\": <integer>,
  \"improvements_list\": [\"<brief description of each substantive improvement>\"],
  \"regressions_list\": [\"<brief description of each regression>\"],
  \"invariant_violations\": [\"<INV-NNN: brief description of how it's violated>\"],
  \"gate_failures\": [\"<Gate N: brief description of failure>\"],
  \"remaining_gaps\": [\"<top 5-8 remaining gaps, ordered by impact>\"],
  \"recommendation\": \"<continue|stop_converged|stop_regressed|stop_excellent>\",
  \"rationale\": \"<detailed 3-5 sentence explanation covering all three dimensions>\"
}

Your ENTIRE response must be valid JSON. Nothing else."

    log "JUDGE: Starting claude -p (model=$JUDGE_MODEL, max_turns=$JUDGE_MAX_TURNS, timeout=${JUDGE_TIMEOUT}s)"

    local start_secs
    start_secs=$(epoch_secs)

    local raw_output=""
    raw_output=$(echo "$prompt" | timeout "$JUDGE_TIMEOUT" \
        claude -p "${CLAUDE_DEBUG_FLAGS[@]}" "${MCP_FLAGS[@]}" \
            --model "$JUDGE_MODEL" \
            --output-format json \
            --max-turns "$JUDGE_MAX_TURNS" \
            --permission-mode acceptEdits \
            2>"$log_file") || {
        local exit_code=$?
        if [[ $exit_code -eq 124 ]]; then
            log "JUDGE: TIMEOUT after $((JUDGE_TIMEOUT / 60)) minutes"
        else
            log "JUDGE: claude -p exited with code $exit_code"
        fi
    }

    local elapsed=$(( $(epoch_secs) - start_secs ))
    log "JUDGE: Elapsed: $(fmt_elapsed $elapsed)"

    # Extract structured JSON from response.
    # The model should output raw JSON as its response text, which ends up in .result.
    # But it might wrap it in markdown fences or include preamble text.
    local extracted
    extracted=$(extract_result "$raw_output") || true

    # Strategy 1: .result is valid JSON directly
    if [[ -n "$extracted" ]] && echo "$extracted" | jq -e '.quality_score' &>/dev/null; then
        echo "$extracted" > "$output_json"
    # Strategy 2: .result contains JSON inside markdown fences (```json ... ```)
    elif [[ -n "$extracted" ]] && echo "$extracted" | grep -qP '```(?:json)?\s*\n?\{'; then
        local fenced_json
        fenced_json=$(echo "$extracted" | sed -n '/```\(json\)\?/,/```/{/```/d;p}')
        if [[ -n "$fenced_json" ]] && echo "$fenced_json" | jq -e '.quality_score' &>/dev/null; then
            echo "$fenced_json" > "$output_json"
            log "JUDGE: Extracted JSON from markdown fences"
        fi
    # Strategy 3: .result has preamble text before a JSON object — extract first { ... }
    elif [[ -n "$extracted" ]] && echo "$extracted" | grep -qP '\{'; then
        local greedy_json
        greedy_json=$(echo "$extracted" | perl -0777 -ne 'print $1 if /(\{.*\})/s')
        if [[ -n "$greedy_json" ]] && echo "$greedy_json" | jq -e '.quality_score' &>/dev/null; then
            echo "$greedy_json" > "$output_json"
            log "JUDGE: Extracted JSON from text response"
        fi
    fi

    # If none of the strategies produced a valid judgment file, use fallback
    if [[ ! -f "$output_json" ]] || ! jq -e '.quality_score' "$output_json" &>/dev/null; then
        log "WARNING: Judge produced invalid JSON — using fallback"
        [[ -n "$extracted" ]] && log "WARNING: .result preview: $(echo "$extracted" | head -c 200)"
        cat > "$output_json" <<'FALLBACK'
{
    "quality_score": 50,
    "substantive_improvements": 5,
    "regressions": 0,
    "improvements_list": ["Judge output unparseable — assuming improvements exist"],
    "regressions_list": [],
    "remaining_gaps": ["Re-evaluate on next iteration"],
    "recommendation": "continue",
    "rationale": "Fallback judgment. Continuing to avoid premature stop."
}
FALLBACK
    fi

    # Normalize field names — judges sometimes use alternate key names.
    # Canonicalize to: quality_score, substantive_improvements, regressions (int),
    # recommendation, self_bootstrap_score, comparison_score, llm_optimization_score.
    jq '
      # Normalize improvements count
      .substantive_improvements = (
        .substantive_improvements // .improvements_count //
        (if (.improvements_list | type) == "array" then .improvements_list | length
         elif (.improvements | type) == "array" then .improvements | length
         else 0 end)
      ) |
      # Normalize regressions count
      .regressions = (
        if (.regressions | type) == "number" then .regressions
        elif (.regressions_count | type) == "number" then .regressions_count
        elif (.regressions_list | type) == "array" then .regressions_list | length
        elif (.regressions | type) == "array" then .regressions | length
        else 0 end
      ) |
      # Normalize sub-scores (may be nested under dimension_scores)
      .self_bootstrap_score = (.self_bootstrap_score // .dimension_scores.self_bootstrapping // .dimension_scores.self_bootstrap // null) |
      .comparison_score = (.comparison_score // .dimension_scores.version_comparison // .dimension_scores.comparison // null) |
      .llm_optimization_score = (.llm_optimization_score // .dimension_scores.llm_optimization // .dimension_scores.llm // null) |
      # Normalize improvements/regressions lists
      .improvements_list = (.improvements_list // .improvements // []) |
      .regressions_list = (
        if (.regressions_list | type) == "array" then .regressions_list
        else [] end
      )
    ' "$output_json" > "${output_json}.tmp" && mv "${output_json}.tmp" "$output_json"

    local score improvements regressions recommendation
    score=$(judgment_field "$output_json" "quality_score")
    improvements=$(judgment_field "$output_json" "substantive_improvements")
    regressions=$(judgment_field "$output_json" "regressions")
    recommendation=$(judgment_field "$output_json" "recommendation")

    # Sub-scores (may not exist in fallback judgments)
    local bootstrap_score comparison_score llm_score
    bootstrap_score=$(judgment_field "$output_json" "self_bootstrap_score" 2>/dev/null) || bootstrap_score="—"
    comparison_score=$(judgment_field "$output_json" "comparison_score" 2>/dev/null) || comparison_score="—"
    llm_score=$(judgment_field "$output_json" "llm_optimization_score" 2>/dev/null) || llm_score="—"

    log "JUDGE: Score=$score (bootstrap=$bootstrap_score, comparison=$comparison_score, llm=$llm_score)"
    log "JUDGE: Improvements=$improvements | Regressions=$regressions | Rec=$recommendation"

    local session_id
    session_id=$(extract_session_id "$raw_output") || true
    [[ -n "$session_id" ]] && log "JUDGE: Session: ${session_id}"

    return 0
}

# ─── Polish Step ──────────────────────────────────────────────────────────────
#
# Final consolidation pass. Removes bloat, tightens prose, verifies
# cross-references. Does NOT add new concepts.

run_polish() {
    local input_spec="$1"
    local output_spec="$2"
    local log_file="${LOGS_DIR}/polish.log"

    local input_lines
    input_lines=$(line_count "$input_spec")
    log "POLISH: Consolidating ($input_lines lines)"

    local prompt="You are performing a FINAL POLISH pass on the DDIS specification.

## File to Read (use the Read tool)

**Spec to polish**: ${input_spec}

## Your Role — Consolidation ONLY

1. Remove redundancy or bloat from recursive improvement iterations
2. Tighten prose (shorter sentences, fewer hedge words)
3. Verify all cross-references are valid
4. Verify self-bootstrapping (document conforms to the format it defines)
5. Ensure Master TODO reflects actual contents

## Hard Constraints

- Do NOT add new invariants, ADRs, sections, or concepts
- Target 5-10% REDUCTION in length
- Do NOT change the meaning of any invariant or ADR
- Do NOT increase document length

## Output

Write the polished spec to: ${output_spec}
Use the Write tool."

    log "POLISH: Starting claude -p (timeout=${POLISH_TIMEOUT}s)"

    local start_secs
    start_secs=$(epoch_secs)

    local raw_output=""
    raw_output=$(echo "$prompt" | timeout "$POLISH_TIMEOUT" \
        claude -p "${CLAUDE_DEBUG_FLAGS[@]}" "${MCP_FLAGS[@]}" \
            --model "$IMPROVER_MODEL" \
            --output-format json \
            --max-turns "$POLISH_MAX_TURNS" \
            --permission-mode acceptEdits \
            2>"$log_file") || true

    local elapsed=$(( $(epoch_secs) - start_secs ))
    log "POLISH: Elapsed: $(fmt_elapsed $elapsed)"

    if [[ -f "$output_spec" ]] && [[ $(wc -l < "$output_spec" | tr -d ' ') -ge 200 ]]; then
        local output_lines
        output_lines=$(line_count "$output_spec")
        log "POLISH: Produced $output_lines lines (delta: $((output_lines - input_lines)))"
    else
        log "POLISH: File write failed — copying input as-is"
        cp "$input_spec" "$output_spec"
    fi
}

# ─── Modularize Step ─────────────────────────────────────────────────────────
#
# Decomposes the monolithic spec into constitution + modules per §0.13.
# Called by ensure_modular_form() on the FIRST run to bootstrap from the
# monolithic seed. After that, the loop works on modular files directly.

run_modularize() {
    local input_spec="$1"
    local log_file="${LOGS_DIR}/modularize.log"

    local input_lines
    input_lines=$(line_count "$input_spec")
    log "MODULARIZE: Decomposing monolithic spec ($input_lines lines)"

    # Ensure modular directory structure
    mkdir -p "${MODULAR_DIR}/constitution" "${MODULAR_DIR}/modules"

    # Check if manifest exists (defines the decomposition structure)
    local manifest_ref=""
    if [[ -f "${MODULAR_DIR}/manifest.yaml" ]]; then
        manifest_ref="
3. **Module manifest (decomposition guide)**: ${MODULAR_DIR}/manifest.yaml
   This defines the module structure, invariant ownership, and bundle budgets.
   Follow its structure but update line counts after writing files."
        log "MODULARIZE: Using existing manifest for structure guidance"
    else
        log "MODULARIZE: No manifest found — LLM will create decomposition from scratch"
    fi

    local prompt="You are performing the MODULARIZATION step on the DDIS specification.
This is a self-bootstrapping requirement: DDIS §0.13 prescribes modularization
for specs exceeding 2,500 lines, and at ${input_lines} lines the spec exceeds
its own threshold.

## Files to Read (use the Read tool)

1. **Monolithic spec to decompose**: ${input_spec}
2. **Improvement methodology** (for context on §0.13 modularization protocol): ${IMPROVEMENT_PROMPT}
${manifest_ref}

## Your Task

Decompose the monolithic spec into TWO-TIER modular form per §0.13:

### Tier 1: System Constitution
- \`${MODULAR_DIR}/constitution/system.md\`
- DECLARATIONS ONLY: one-line summaries of all invariants, ADRs, quality gates
- Executive summary, first-principles derivation, document structure overview
- Cross-cutting concerns: performance budgets, glossary excerpt, architecture
- Target: 400-600 lines

### Tier 2: Modules (one file per knowledge domain)

1. \`${MODULAR_DIR}/modules/core-standard.md\` — PART 0 core + PART I foundations
   - Full invariant definitions (INV-001 through INV-010, INV-017 through INV-020)
   - Full ADR definitions (ADR-001 through ADR-005, ADR-008 through ADR-011)
   - Quality gates (Gate 1 through Gate 7)
   - State machine (§1.1), complexity analysis (§1.3), end-to-end trace (§1.4)

2. \`${MODULAR_DIR}/modules/element-specifications.md\` — Chapters 2-7
   - All element specification chapters with their verification prompt blocks
   - Maintains INV-020 (Verification Prompt Coverage)

3. \`${MODULAR_DIR}/modules/modularization.md\` — §0.13 protocol
   - Modularization invariants (INV-011 through INV-016)
   - Modularization ADRs (ADR-006, ADR-007)
   - Modularization quality gates (M-1 through M-5)
   - Cascade protocol, assembly/disassembly procedures

4. \`${MODULAR_DIR}/modules/guidance-operations.md\` — PART III + PART IV + Appendices + PART X
   - Voice & style guidance, anti-pattern catalog
   - Operational playbook, authoring sequence
   - Appendices: Glossary, Risk Register, Error Taxonomy, Quick-Reference Card
   - Master TODO, open questions, version history

### Updated Manifest

5. \`${MODULAR_DIR}/manifest.yaml\` — Updated with accurate line counts

## Critical Invariants for Modularization

- **ZERO content loss**: every section, invariant, ADR, gate, glossary term, verification
  prompt from the monolithic spec must appear in EXACTLY ONE module. Missing content is
  the single most common modularization failure mode.
- **INV-013 (Ownership Uniqueness)**: each INV-NNN is maintained by exactly one module
- **INV-014 (Bundle Budget)**: constitution + any single module must fit within 4,000 lines
- **INV-012 (Cross-Module Isolation)**: cross-module references use INV-NNN/ADR-NNN IDs
  through the constitution, not direct section number references
- **INV-018 (Structural Redundancy)**: restate key invariants at point of use within modules
- **INV-011 (Module Completeness)**: each module bundle (constitution + module) must be
  sufficient for an LLM to work on that domain without loading other modules
- **INV-015 (Declaration-Definition Consistency)**: constitution declarations must faithfully
  summarize the full definitions in modules

## Anti-Patterns to Avoid

- DO NOT create stub modules with placeholder content
- DO NOT lose negative specifications (DO NOT constraints) during decomposition
- DO NOT lose verification prompt blocks during decomposition
- DO NOT merge unrelated content just to balance module sizes
- DO NOT reference other modules by section number — use invariant/ADR IDs only
- DO NOT duplicate full definitions across modules (declarations in constitution are fine)

## Output

Write ALL 6 files using the Write tool. After writing, report:
- Line count for each file
- Total lines across all files
- Bundle sizes (constitution + each module)
- Any content that could not be cleanly placed"

    log "MODULARIZE: Starting claude -p (model=$IMPROVER_MODEL, max_turns=$MODULARIZE_MAX_TURNS, timeout=${MODULARIZE_TIMEOUT}s)"

    local start_secs
    start_secs=$(epoch_secs)

    local raw_output=""
    raw_output=$(echo "$prompt" | timeout "$MODULARIZE_TIMEOUT" \
        claude -p "${CLAUDE_DEBUG_FLAGS[@]}" "${MCP_FLAGS[@]}" \
            --model "$IMPROVER_MODEL" \
            --output-format json \
            --max-turns "$MODULARIZE_MAX_TURNS" \
            --permission-mode acceptEdits \
            2>"$log_file") || {
        local exit_code=$?
        if [[ $exit_code -eq 124 ]]; then
            log "MODULARIZE: TIMEOUT after $((MODULARIZE_TIMEOUT / 60)) minutes"
        else
            log "MODULARIZE: claude -p exited with code $exit_code"
        fi
    }

    local elapsed=$(( $(epoch_secs) - start_secs ))
    log "MODULARIZE: Elapsed: $(fmt_elapsed $elapsed)"

    local session_id
    session_id=$(extract_session_id "$raw_output") || true
    [[ -n "$session_id" ]] && log "MODULARIZE: Session: ${session_id}"

    # Validate modularization output
    validate_modularization "$input_spec"
}

validate_modularization() {
    local monolithic_spec="$1"
    local mono_lines
    mono_lines=$(line_count "$monolithic_spec")
    local all_ok=true

    log "MODULARIZE: Validating decomposition..."

    # Check all expected files exist and have content
    local expected_files=(
        "${MODULAR_DIR}/constitution/system.md"
        "${MODULAR_DIR}/modules/core-standard.md"
        "${MODULAR_DIR}/modules/element-specifications.md"
        "${MODULAR_DIR}/modules/modularization.md"
        "${MODULAR_DIR}/modules/guidance-operations.md"
        "${MODULAR_DIR}/manifest.yaml"
    )

    local total_lines=0
    local constitution_lines=0

    for f in "${expected_files[@]}"; do
        if [[ ! -f "$f" ]]; then
            log "MODULARIZE: MISSING: $f"
            all_ok=false
        else
            local flines
            flines=$(line_count "$f")
            if [[ "$f" == *"manifest.yaml" ]]; then
                log "MODULARIZE:   manifest.yaml ($flines lines)"
            else
                total_lines=$((total_lines + flines))
                log "MODULARIZE:   $(basename "$f") ($flines lines)"
                if [[ "$f" == *"system.md" ]]; then
                    constitution_lines=$flines
                fi
            fi
        fi
    done

    log "MODULARIZE: Total modular lines: $total_lines (monolithic: $mono_lines)"

    # Check bundle budgets (constitution + module)
    local hard_ceiling=5000
    for mod in "${MODULAR_DIR}/modules"/*.md; do
        [[ -f "$mod" ]] || continue
        local mod_lines bundle_size
        mod_lines=$(line_count "$mod")
        bundle_size=$((constitution_lines + mod_lines))
        local mod_name
        mod_name=$(basename "$mod")
        if [[ $bundle_size -gt $hard_ceiling ]]; then
            log "MODULARIZE: BUDGET EXCEEDED: $mod_name bundle = $bundle_size lines (ceiling: $hard_ceiling)"
            all_ok=false
        fi
    done

    # Content loss check: total modular lines should be ≥80% of monolithic
    local min_expected=$((mono_lines * 80 / 100))
    if [[ $total_lines -lt $min_expected ]]; then
        log "MODULARIZE: WARNING: Possible content loss — modular ($total_lines) < 80% of monolithic ($mono_lines)"
        all_ok=false
    fi

    if $all_ok; then
        log "MODULARIZE: Validation PASSED"
    else
        log "MODULARIZE: Validation FAILED — review output manually"
    fi
}

# ─── Stopping Condition ──────────────────────────────────────────────────────

check_stop() {
    local judgment_file="$1"
    local iteration=$2

    local recommendation score improvements regressions
    recommendation=$(judgment_field "$judgment_file" "recommendation")
    score=$(judgment_field "$judgment_file" "quality_score")
    improvements=$(judgment_field "$judgment_file" "substantive_improvements")
    regressions=$(judgment_field "$judgment_file" "regressions")

    # Defensive: ensure numeric values (judge may return null/string)
    [[ "$score" =~ ^[0-9]+$ ]] || score=50
    [[ "$improvements" =~ ^[0-9]+$ ]] || improvements=0
    [[ "$regressions" =~ ^[0-9]+$ ]] || regressions=0

    # Signal 3: Regression
    if [[ "$recommendation" == "stop_regressed" ]] || \
       [[ $regressions -gt 0 && $regressions -ge $improvements ]]; then
        log "STOP: Regression in v${iteration}. Keeping v$((iteration - 1))." >&2
        echo "regressed"
        return 0
    fi

    # Judge: excellent
    if [[ "$recommendation" == "stop_excellent" ]]; then
        log "STOP: Excellent (score=$score)." >&2
        echo "excellent"
        return 0
    fi

    # Signal 1: Diminishing returns
    if [[ $improvements -lt $MIN_SUBSTANTIVE_IMPROVEMENTS ]]; then
        log "STOP: Only $improvements improvements (min: $MIN_SUBSTANTIVE_IMPROVEMENTS)." >&2
        echo "converged"
        return 0
    fi

    # Signal 2: Quality plateau
    local prev_judgment="${JUDGMENTS_DIR}/judgment_v$((iteration - 1)).json"
    if [[ -f "$prev_judgment" ]]; then
        local prev_score delta
        prev_score=$(judgment_field "$prev_judgment" "quality_score")
        delta=$((score - prev_score))
        if [[ $delta -lt $MIN_QUALITY_DELTA ]]; then
            log "STOP: Quality delta=$delta (min: $MIN_QUALITY_DELTA)." >&2
            echo "plateau"
            return 0
        fi
    fi

    # Judge: converged
    if [[ "$recommendation" == "stop_converged" ]]; then
        log "STOP: Judge says converged." >&2
        echo "converged"
        return 0
    fi

    echo "continue"
    return 0
}

# ─── Summary Report ──────────────────────────────────────────────────────────

print_summary() {
    local final_version=$1
    local stop_reason=$2

    log_section "EVOLUTION SUMMARY"

    echo "Stop reason: $stop_reason"
    echo "Versions produced: $final_version"
    echo ""

    printf "%-8s  %6s  %7s  %5s  %5s  %-15s\n" \
        "VERSION" "LINES" "SCORE" "IMPRV" "REGR" "RECOMMENDATION"
    printf "%-8s  %6s  %7s  %5s  %5s  %-15s\n" \
        "-------" "------" "-------" "-----" "-----" "---------------"

    # v0 — modular snapshot (seed)
    local v0_lines="—"
    if [[ -d "${VERSIONS_DIR}/v0" ]]; then
        v0_lines=$(wc -l "${VERSIONS_DIR}/v0"/constitution/*.md "${VERSIONS_DIR}/v0"/modules/*.md 2>/dev/null | tail -1 | awk '{print $1}')
    fi
    printf "%-8s  %6s  %7s  %5s  %5s  %-15s\n" \
        "v0" "$v0_lines" "—" "—" "—" "seed"

    for ((i = 1; i <= final_version; i++)); do
        local judg="${JUDGMENTS_DIR}/judgment_v${i}.json"
        local vi_lines="—"
        if [[ -d "${VERSIONS_DIR}/v${i}" ]]; then
            vi_lines=$(wc -l "${VERSIONS_DIR}/v${i}"/constitution/*.md "${VERSIONS_DIR}/v${i}"/modules/*.md 2>/dev/null | tail -1 | awk '{print $1}')
        fi
        if [[ -f "$judg" ]]; then
            printf "%-8s  %6s  %7s  %5s  %5s  %-15s\n" \
                "v${i}" \
                "$vi_lines" \
                "$(judgment_field "$judg" "quality_score")" \
                "$(judgment_field "$judg" "substantive_improvements")" \
                "$(judgment_field "$judg" "regressions")" \
                "$(judgment_field "$judg" "recommendation")"
        fi
    done

    local final_spec="${VERSIONS_DIR}/ddis_final.md"
    if [[ -f "$final_spec" ]]; then
        echo ""
        echo "Final (assembled): ${final_spec} ($(line_count "$final_spec") lines)"
    fi

    echo ""
    echo "Versions:  ${VERSIONS_DIR}/"
    echo "Judgments:  ${JUDGMENTS_DIR}/"
    echo "Logs:       ${LOGS_DIR}/"
    echo "Modular:    ${MODULAR_DIR}/"

    echo ""
    echo "Module sizes (working form):"
    for mod_file in "${MODULAR_DIR}/constitution/system.md" "${MODULAR_DIR}/modules"/*.md; do
        [[ -f "$mod_file" ]] && printf "  %-35s %s lines\n" "$(basename "$mod_file")" "$(line_count "$mod_file")"
    done

    if $BEADS_AVAILABLE; then
        echo "Beads:      ${WORK_DIR}/.beads/"
    fi

    # Find the last available judgment — when best_version=0 (regression on
    # iteration 1), judgment_v0.json doesn't exist, but judgment_v1.json does.
    local last_judgment=""
    if [[ -f "${JUDGMENTS_DIR}/judgment_v${final_version}.json" ]]; then
        last_judgment="${JUDGMENTS_DIR}/judgment_v${final_version}.json"
    else
        # Walk backward to find the most recent judgment file
        for ((j = final_version + 1; j >= 1; j--)); do
            if [[ -f "${JUDGMENTS_DIR}/judgment_v${j}.json" ]]; then
                last_judgment="${JUDGMENTS_DIR}/judgment_v${j}.json"
                break
            fi
        done
    fi

    if [[ -n "$last_judgment" && -f "$last_judgment" ]]; then
        echo ""
        echo "Remaining gaps:"
        jq -r '.remaining_gaps[]' "$last_judgment" 2>/dev/null | sed 's/^/  - /'
        echo ""
        echo "Rationale: $(judgment_field "$last_judgment" "rationale")"
    fi
}

# ─── Main ─────────────────────────────────────────────────────────────────────

main() {
    check_prereqs

    log_section "DDIS RALPH Loop — Recursive Self-Improvement"

    log "Configuration:"
    log "  Max iterations:     $MAX_ITERATIONS"
    log "  Min improvements:   $MIN_SUBSTANTIVE_IMPROVEMENTS"
    log "  Min quality delta:  $MIN_QUALITY_DELTA"
    log "  Improver model:     $IMPROVER_MODEL"
    log "  Judge model:        $JUDGE_MODEL"
    log "  Polish on exit:     $POLISH_ON_EXIT"
    log "  Working form:       modular (self-bootstrapping per §0.13)"
    log "  Audit timeout:      $((AUDIT_TIMEOUT / 60))m | Apply: $((APPLY_TIMEOUT / 60))m | Judge: $((JUDGE_TIMEOUT / 60))m"
    log ""

    mkdir -p "$VERSIONS_DIR" "$JUDGMENTS_DIR" "$LOGS_DIR"

    check_beads
    beads_init

    # ── Bootstrap: ensure modular working form exists ──
    # The loop ALWAYS works on modular files (self-bootstrapping per §0.13).
    # First run decomposes the monolithic seed; subsequent runs use existing modules.
    ensure_modular_form

    # Snapshot v0 — the starting state of the modular form
    snapshot_modular 0

    local open_count
    open_count=$(beads_open_count)
    [[ "$open_count" != "0" ]] && log "BEADS: $open_count open issues from previous runs"

    local best_version=0
    local stop_reason="max_iterations"

    for ((i = 1; i <= MAX_ITERATIONS; i++)); do
        log_section "ITERATION $i / $MAX_ITERATIONS"

        local judgment="${JUDGMENTS_DIR}/judgment_v${i}.json"
        local prev_judgment="${JUDGMENTS_DIR}/judgment_v$((i - 1)).json"

        # ── Step 1: Improve (audit → apply on modular files) ──
        local prev_judg_arg=""
        [[ -f "$prev_judgment" ]] && prev_judg_arg="$prev_judgment"

        if ! run_improve "$i" "$prev_judg_arg"; then
            log "ERROR: Improvement failed at iteration $i. Keeping v$((i - 1))."
            stop_reason="improve_failed"
            break
        fi

        # ── Step 2: Judge (reads versioned modular snapshots) ──
        run_judge "$i" "$judgment"

        # ── Step 3: Check stopping condition ──
        local decision
        decision=$(check_stop "$judgment" "$i")

        case "$decision" in
            regressed)
                stop_reason="regressed"
                best_version=$((i - 1))
                # Restore previous modular form from snapshot
                log "ROLLBACK: Restoring modular form from v$((i - 1)) snapshot"
                local prev_snapshot="${VERSIONS_DIR}/v$((i - 1))"
                if [[ -d "$prev_snapshot" ]]; then
                    rm -rf "${MODULAR_DIR}/constitution" "${MODULAR_DIR}/modules"
                    cp -r "${prev_snapshot}/constitution" "${prev_snapshot}/modules" "${MODULAR_DIR}/"
                    [[ -f "${prev_snapshot}/manifest.yaml" ]] && \
                        cp "${prev_snapshot}/manifest.yaml" "${MODULAR_DIR}/"
                fi
                break
                ;;
            excellent)
                stop_reason="excellent"
                best_version=$i
                break
                ;;
            converged|plateau)
                stop_reason="$decision"
                best_version=$i
                break
                ;;
            continue)
                best_version=$i
                log "DECIDE: Continuing to iteration $((i + 1))"
                ;;
        esac
    done

    # ── Assemble monolith from modular form ──
    # The modular form is the working artifact; the monolith is the distribution form.
    log_section "ASSEMBLE PASS"
    local final_spec="${VERSIONS_DIR}/ddis_final.md"
    run_assemble "$final_spec"

    # ── Optional Polish Pass (on assembled monolith) ──
    if [[ "$POLISH_ON_EXIT" == "true" && $best_version -gt 0 ]]; then
        log_section "POLISH PASS"
        local polished_spec="${VERSIONS_DIR}/ddis_polished.md"
        run_polish "$final_spec" "$polished_spec"
        if [[ -f "$polished_spec" ]] && [[ $(line_count "$polished_spec") -ge 200 ]]; then
            cp "$polished_spec" "$final_spec"
        fi
    else
        log "POLISH: Skipping — using assembled monolith as final"
    fi

    beads_finalize
    print_summary "$best_version" "$stop_reason"

    # Copy to project root — but only if we actually improved beyond the seed.
    if [[ $best_version -gt 0 ]]; then
        cp "$final_spec" "${SCRIPT_DIR}/ddis_final.md"
        log ""
        log "Done. Final spec: ${SCRIPT_DIR}/ddis_final.md"
    else
        log ""
        log "Done. No improvement over seed — final remains at ${final_spec}"
        log "Seed preserved at: ${SEED_SPEC}"
    fi

    log "Modular (working form): ${MODULAR_DIR}/"
}

main "$@"
