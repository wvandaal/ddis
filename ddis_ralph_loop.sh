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
# using its own methodology. Each iteration:
#   1. IMPROVE: LLM reads spec + methodology from disk → writes improved spec
#   2. JUDGE:   Separate LLM reads both versions → structured quality assessment
#   3. DECIDE:  Script checks multi-signal stopping condition
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
JUDGE_MODEL=${DDIS_JUDGE_MODEL:-"sonnet"}
POLISH_ON_EXIT=${DDIS_POLISH:-true}
USE_BEADS=${DDIS_USE_BEADS:-auto}
VERBOSE=${DDIS_VERBOSE:-false}

# ─── CLI Arguments ────────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --verbose)   VERBOSE=true; shift ;;
        --no-polish) POLISH_ON_EXIT=false; shift ;;
        *)           echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

# ─── Path Setup ──────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK_DIR="${SCRIPT_DIR}/ddis-evolution"
VERSIONS_DIR="${WORK_DIR}/versions"
JUDGMENTS_DIR="${WORK_DIR}/judgments"
LOGS_DIR="${WORK_DIR}/logs"

SEED_SPEC="${SCRIPT_DIR}/ddis_standard.md"
IMPROVEMENT_PROMPT="${SCRIPT_DIR}/ddis_recursive_improvement_prompt.md"
KICKOFF_PROMPT="${SCRIPT_DIR}/kickoff_prompt.md"

# ─── Claude -p Flags ─────────────────────────────────────────────────────────
#
# All calls disable MCP servers (btca hangs during shutdown).
# Built-in tools (Read, Write, Edit, Glob, Grep, Bash) still work.

MCP_FLAGS='--strict-mcp-config --mcp-config {"mcpServers":{}}'

# Debug flag: --debug-file writes logs without changing output format
CLAUDE_DEBUG_FLAG=""
if [[ "$VERBOSE" == "true" ]]; then
    mkdir -p "${WORK_DIR}/logs"
    CLAUDE_DEBUG_FLAG="--debug-file ${LOGS_DIR}/claude_debug.log"
fi

# Per-call-type tuning
IMPROVE_MAX_TURNS=100
IMPROVE_TIMEOUT=2400   # 40 minutes

JUDGE_MAX_TURNS=15
JUDGE_TIMEOUT=600      # 10 minutes

POLISH_MAX_TURNS=50
POLISH_TIMEOUT=1800    # 30 minutes

# ─── Helpers ──────────────────────────────────────────────────────────────────

timestamp() { date '+%Y-%m-%d %H:%M:%S'; }
log() { echo "[$(timestamp)] $*"; }

log_section() {
    echo ""
    echo "═══════════════════════════════════════════════════════════════════"
    echo "  $*"
    echo "═══════════════════════════════════════════════════════════════════"
    echo ""
}

line_count() { wc -l < "$1" | tr -d ' '; }

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
# br/bv track gaps ACROSS RALPH runs. Lightweight — doesn't control the loop.

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

beads_sync_gaps() {
    $BEADS_AVAILABLE || return 0
    local iteration=$1 judgment_file="$2" prev_judgment="${3:-}"

    local gaps
    gaps=$(jq -r '.remaining_gaps[]' "$judgment_file" 2>/dev/null) || return 0

    # Close resolved gaps from previous iteration
    if [[ -n "$prev_judgment" && -f "$prev_judgment" ]]; then
        local prev_gaps
        prev_gaps=$(jq -r '.remaining_gaps[]' "$prev_judgment" 2>/dev/null) || true
        while IFS= read -r prev_gap; do
            [[ -z "$prev_gap" ]] && continue
            if ! echo "$gaps" | grep -qF "$prev_gap"; then
                local issue_id
                issue_id=$(cd "$WORK_DIR" && br search "$prev_gap" --json 2>/dev/null \
                    | jq -r '.[0].id // empty' 2>/dev/null) || true
                [[ -n "$issue_id" ]] && \
                    (cd "$WORK_DIR" && br close "$issue_id" \
                        --reason "Resolved in iteration $iteration" --quiet 2>/dev/null) || true
            fi
        done <<< "$prev_gaps"
    fi

    # Create issues for new gaps
    local priority=1
    while IFS= read -r gap; do
        [[ -z "$gap" ]] && continue
        local existing
        existing=$(cd "$WORK_DIR" && br search "$gap" --json 2>/dev/null \
            | jq -r '[.[] | select(.status == "open")] | length' 2>/dev/null) || existing=0
        if [[ "$existing" == "0" ]]; then
            (cd "$WORK_DIR" && br create "$gap" \
                --type task --priority "$priority" --quiet 2>/dev/null) || true
        fi
        ((priority++)) || true
    done <<< "$gaps"
}

beads_get_persistent_gaps() {
    $BEADS_AVAILABLE || { echo ""; return 0; }
    [[ -f "${WORK_DIR}/.beads/beads.db" ]] || { echo ""; return 0; }
    (cd "$WORK_DIR" && br list --status open --json 2>/dev/null \
        | jq -r '.[].title' 2>/dev/null) || echo ""
}

beads_get_triage() {
    $BV_AVAILABLE || { echo ""; return 0; }
    (cd "$WORK_DIR" && bv --robot-triage --format toon 2>/dev/null) || echo ""
}

beads_finalize() {
    $BEADS_AVAILABLE || return 0
    log "BEADS: Finalizing"
    (cd "$WORK_DIR" && br sync --flush-only --quiet 2>/dev/null) || true
    if $BV_AVAILABLE; then
        local open_count
        open_count=$(cd "$WORK_DIR" && br count --status open 2>/dev/null \
            | grep -oP '\d+' | head -1) || open_count=0
        [[ "$open_count" -gt 0 ]] && \
            log "BEADS: $open_count gaps remain. Run: cd ${WORK_DIR} && bv --robot-triage"
    fi
}

# ─── Improve Step ─────────────────────────────────────────────────────────────
#
# Design: The model reads files from disk (via Read tool), reasons deeply
# (extended thinking enabled by default — no --effort flag), and writes the
# improved spec to disk (via Write tool).
#
# No text extraction. No continuation loop. No WRITTEN_TO_FILE protocol.

run_improve() {
    local iteration=$1
    local current_spec="$2"
    local output_spec="$3"
    local prev_judgment="${4:-}"
    local log_file="${LOGS_DIR}/improve_v${iteration}.log"

    local current_lines
    current_lines=$(line_count "$current_spec")
    log "IMPROVE: v$((iteration - 1)) ($current_lines lines) → v${iteration}"

    # Build the prompt. Key principle: SHORT prompt + file paths on disk.
    # The model reads the actual files using the Read tool, keeping our
    # prompt small and letting the model manage its own context.
    local prompt

    if [[ $iteration -eq 1 ]]; then
        local persistent_gaps
        persistent_gaps=$(beads_get_persistent_gaps)
        local gaps_section=""
        if [[ -n "$persistent_gaps" ]]; then
            gaps_section="
## Persistent Gaps from Previous Runs
These gaps were never fully resolved. Pay special attention:
$(echo "$persistent_gaps" | sed 's/^/- /')
"
        fi

        prompt="You are performing the first iteration of recursive self-improvement on the DDIS specification standard.

## Files to Read (use the Read tool)

1. **Improvement methodology**: ${IMPROVEMENT_PROMPT}
   This describes the audit framework, quality criteria, and anti-patterns. Read it first.

2. **Current DDIS spec (v0)**: ${current_spec}
   This is the ~2,300-line self-bootstrapping meta-specification you will improve.

## What to Do

$(cat "$KICKOFF_PROMPT")
${gaps_section}

## CRITICAL OVERRIDE

The improvement methodology says to produce two artifacts. **IGNORE THAT.**
Produce ONLY the improved DDIS standard (Artifact 2). Do the audit and
improvement spec work in your reasoning/thinking — do not output Artifact 1.

## Output Instructions

Write the complete improved DDIS standard to this exact path:
  ${output_spec}

Use the Write tool to create the file. The file must be the complete spec —
start with the first heading, end with the last section. Target 2,000-3,000
lines. After writing, briefly confirm what you wrote and any key changes made."

    else
        local triage_section=""
        local triage
        triage=$(beads_get_triage)
        if [[ -n "$triage" ]]; then
            triage_section="
## Gap Triage (dependency-aware prioritization)
\`\`\`
${triage}
\`\`\`
"
        fi

        prompt="You are performing iteration $iteration of recursive self-improvement on the DDIS specification standard.

## Files to Read (use the Read tool)

1. **Improvement methodology**: ${IMPROVEMENT_PROMPT}
2. **Current DDIS spec (v$((iteration - 1)))**: ${current_spec}
3. **Judge's assessment of v$((iteration - 1))**: ${prev_judgment}

## Your Task

Read the judge's assessment carefully. Produce the next version that:
- Addresses every remaining gap the judge identified
- Preserves all existing strengths (do NOT regress)
- Makes only structural/substantive improvements (not cosmetic)

## Key Constraints

- The spec must remain self-bootstrapping (conform to the format it defines)
- Focus on LLM-optimization: the primary consumer is an LLM
- Do NOT increase document length by more than 20%

## Remaining Gaps to Address (summary — read the full judge file for details)

$(jq -r '.remaining_gaps[]' "$prev_judgment" 2>/dev/null | sed 's/^/- /' || echo "- Read judge assessment for details")

## Regressions to Fix

$(jq -r '.regressions_list[]' "$prev_judgment" 2>/dev/null | sed 's/^/- /' || echo "- None identified")
${triage_section}
## Output Instructions

Write the complete improved DDIS standard to this exact path:
  ${output_spec}

Use the Write tool. After writing, briefly confirm what you wrote."
    fi

    log "IMPROVE: Starting claude -p (model=$IMPROVER_MODEL, max_turns=$IMPROVE_MAX_TURNS, timeout=${IMPROVE_TIMEOUT}s)"

    local raw_output=""
    raw_output=$(echo "$prompt" | timeout "$IMPROVE_TIMEOUT" \
        claude -p $CLAUDE_DEBUG_FLAG $MCP_FLAGS \
            --model "$IMPROVER_MODEL" \
            --output-format json \
            --max-turns "$IMPROVE_MAX_TURNS" \
            --permission-mode acceptEdits \
            2>"$log_file") || {
        local exit_code=$?
        if [[ $exit_code -eq 124 ]]; then
            log "IMPROVE: TIMEOUT after $((IMPROVE_TIMEOUT / 60)) minutes"
        else
            log "IMPROVE: claude -p exited with code $exit_code"
        fi
    }

    # Log session ID for post-hoc analysis
    local session_id
    session_id=$(extract_session_id "$raw_output") || true
    [[ -n "$session_id" ]] && log "IMPROVE: Session: ${session_id}"

    # Primary check: did the model write the output file?
    if [[ -f "$output_spec" ]] && [[ $(wc -l < "$output_spec" | tr -d ' ') -ge 200 ]]; then
        local new_lines
        new_lines=$(line_count "$output_spec")
        log "IMPROVE: Produced v${iteration} ($new_lines lines, delta: $((new_lines - current_lines)))"
        return 0
    fi

    # Fallback: model may have output the spec as response text instead of writing a file
    local result_text
    result_text=$(extract_result "$raw_output") || true
    if [[ -n "$result_text" ]] && [[ $(echo "$result_text" | wc -l | tr -d ' ') -ge 200 ]]; then
        log "IMPROVE: Model output spec as text — writing to file"
        echo "$result_text" > "$output_spec"
        local new_lines
        new_lines=$(line_count "$output_spec")
        log "IMPROVE: Produced v${iteration} ($new_lines lines, delta: $((new_lines - current_lines)))"
        return 0
    fi

    log "ERROR: v${iteration} not produced. Output file missing or too short."
    log "       Session: ${session_id:-unknown}"
    log "       Log: $log_file"
    return 1
}

# ─── Judge Step ───────────────────────────────────────────────────────────────
#
# Separate LLM evaluates whether version N is better than N-1.
# Uses --json-schema for structured output.

run_judge() {
    local iteration=$1
    local prev_spec="$2"
    local curr_spec="$3"
    local output_json="$4"
    local log_file="${LOGS_DIR}/judge_v${iteration}.log"

    local prev_lines curr_lines
    prev_lines=$(line_count "$prev_spec")
    curr_lines=$(line_count "$curr_spec")
    log "JUDGE: Comparing v$((iteration - 1)) ($prev_lines lines) vs v${iteration} ($curr_lines lines)"

    local prompt="You are the JUDGE in a recursive self-improvement loop for the DDIS specification.

## Files to Read (use the Read tool)

1. **Previous version (v$((iteration - 1)))**: ${prev_spec}
2. **Current version (v${iteration})**: ${curr_spec}

## Your Role

Evaluate whether v${iteration} is better than v$((iteration - 1)). You are NOT the
author — you are an independent assessor. Be rigorous. Do not credit cosmetic changes.

## Evaluation Criteria

1. **DDIS Self-Conformance**: Does v${iteration} satisfy its own invariants (INV-001 through INV-010+) and quality gates?
2. **LLM Optimization**: Is v${iteration} more effective for LLM consumption?
3. **Substantive vs Cosmetic**: Count ONLY structural improvements (new invariants, fixed violations, new LLM provisions). NOT rewording.
4. **Regressions**: Did anything get WORSE? Missing sections, broken cross-refs, violated invariants?
5. **Remaining Gaps**: Top 3-5 things still missing or weak?

## Scoring Guide

- 0-30: Fundamentally broken (doesn't self-bootstrap, missing major sections)
- 31-50: Structural gaps (missing invariants, ADRs, or required elements)
- 51-70: Functional but incomplete
- 71-85: Good (complete, most invariants satisfied)
- 86-95: Excellent (comprehensive, self-conforming, LLM-optimized)
- 96-100: Near-perfect (reserve this)

## Recommendation Logic

- **continue**: Score improved by >= ${MIN_QUALITY_DELTA} AND >= ${MIN_SUBSTANTIVE_IMPROVEMENTS} substantive improvements AND gaps addressable
- **stop_converged**: Score improved by < ${MIN_QUALITY_DELTA} OR < ${MIN_SUBSTANTIVE_IMPROVEMENTS} improvements
- **stop_regressed**: Regressions outweigh improvements — keep previous version
- **stop_excellent**: Score >= 95 AND no critical remaining gaps

## Output Format — CRITICAL

Read both versions carefully, then output ONLY a single raw JSON object. No markdown fences, no explanation before or after — JUST the JSON. The object must have exactly these fields:

{
  \"quality_score\": <integer 0-100>,
  \"substantive_improvements\": <integer>,
  \"regressions\": <integer>,
  \"improvements_list\": [\"<brief description of each improvement>\"],
  \"regressions_list\": [\"<brief description of each regression>\"],
  \"remaining_gaps\": [\"<top 3-5 remaining gaps>\"],
  \"recommendation\": \"<continue|stop_converged|stop_regressed|stop_excellent>\",
  \"rationale\": \"<2-3 sentence explanation>\"
}

Your ENTIRE response must be valid JSON. Nothing else."

    log "JUDGE: Starting claude -p (model=$JUDGE_MODEL, max_turns=$JUDGE_MAX_TURNS, timeout=${JUDGE_TIMEOUT}s)"

    local raw_output=""
    raw_output=$(echo "$prompt" | timeout "$JUDGE_TIMEOUT" \
        claude -p $CLAUDE_DEBUG_FLAG $MCP_FLAGS \
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

    local score improvements regressions recommendation
    score=$(judgment_field "$output_json" "quality_score")
    improvements=$(judgment_field "$output_json" "substantive_improvements")
    regressions=$(judgment_field "$output_json" "regressions")
    recommendation=$(judgment_field "$output_json" "recommendation")

    log "JUDGE: Score=$score | Improvements=$improvements | Regressions=$regressions | Rec=$recommendation"

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

    local raw_output=""
    raw_output=$(echo "$prompt" | timeout "$POLISH_TIMEOUT" \
        claude -p $CLAUDE_DEBUG_FLAG $MCP_FLAGS \
            --model "$IMPROVER_MODEL" \
            --output-format json \
            --max-turns "$POLISH_MAX_TURNS" \
            --permission-mode acceptEdits \
            2>"$log_file") || true

    if [[ -f "$output_spec" ]] && [[ $(wc -l < "$output_spec" | tr -d ' ') -ge 200 ]]; then
        local output_lines
        output_lines=$(line_count "$output_spec")
        log "POLISH: Produced $output_lines lines (delta: $((output_lines - input_lines)))"
    else
        log "POLISH: File write failed — copying input as-is"
        cp "$input_spec" "$output_spec"
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

    printf "%-8s  %6s  %7s  %5s  %5s  %-15s\n" \
        "v0" "$(line_count "$VERSIONS_DIR/ddis_v0.md")" "—" "—" "—" "seed"

    for ((i = 1; i <= final_version; i++)); do
        local judg="${JUDGMENTS_DIR}/judgment_v${i}.json"
        if [[ -f "$judg" ]]; then
            printf "%-8s  %6s  %7s  %5s  %5s  %-15s\n" \
                "v${i}" \
                "$(line_count "${VERSIONS_DIR}/ddis_v${i}.md")" \
                "$(judgment_field "$judg" "quality_score")" \
                "$(judgment_field "$judg" "substantive_improvements")" \
                "$(judgment_field "$judg" "regressions")" \
                "$(judgment_field "$judg" "recommendation")"
        fi
    done

    if [[ -f "${VERSIONS_DIR}/ddis_final.md" ]]; then
        echo ""
        echo "Final: ${VERSIONS_DIR}/ddis_final.md ($(line_count "${VERSIONS_DIR}/ddis_final.md") lines)"
    fi

    echo ""
    echo "Versions:  ${VERSIONS_DIR}/"
    echo "Judgments:  ${JUDGMENTS_DIR}/"
    echo "Logs:       ${LOGS_DIR}/"

    if $BEADS_AVAILABLE; then
        echo "Beads:      ${WORK_DIR}/.beads/"
    fi

    local last_judgment="${JUDGMENTS_DIR}/judgment_v${final_version}.json"
    if [[ -f "$last_judgment" ]]; then
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
    log "  Improve timeout:    $((IMPROVE_TIMEOUT / 60))m | Judge: $((JUDGE_TIMEOUT / 60))m"
    log ""

    mkdir -p "$VERSIONS_DIR" "$JUDGMENTS_DIR" "$LOGS_DIR"
    cp "$SEED_SPEC" "$VERSIONS_DIR/ddis_v0.md"

    check_beads
    beads_init

    local persistent_gaps
    persistent_gaps=$(beads_get_persistent_gaps)
    if [[ -n "$persistent_gaps" ]]; then
        local gap_count
        gap_count=$(echo "$persistent_gaps" | wc -l | tr -d ' ')
        log "BEADS: $gap_count persistent gaps from previous run"
    fi

    local best_version=0
    local stop_reason="max_iterations"

    for ((i = 1; i <= MAX_ITERATIONS; i++)); do
        log_section "ITERATION $i / $MAX_ITERATIONS"

        local prev_spec="${VERSIONS_DIR}/ddis_v$((i - 1)).md"
        local curr_spec="${VERSIONS_DIR}/ddis_v${i}.md"
        local judgment="${JUDGMENTS_DIR}/judgment_v${i}.json"
        local prev_judgment="${JUDGMENTS_DIR}/judgment_v$((i - 1)).json"

        # ── Step 1: Improve ──
        local prev_judg_arg=""
        [[ -f "$prev_judgment" ]] && prev_judg_arg="$prev_judgment"

        if ! run_improve "$i" "$prev_spec" "$curr_spec" "$prev_judg_arg"; then
            log "ERROR: Improvement failed at iteration $i. Keeping v$((i - 1))."
            stop_reason="improve_failed"
            break
        fi

        # ── Step 2: Judge ──
        run_judge "$i" "$prev_spec" "$curr_spec" "$judgment"

        # ── Step 2b: Sync gaps to beads ──
        local prev_judg_for_beads=""
        [[ -f "${JUDGMENTS_DIR}/judgment_v$((i - 1)).json" ]] && \
            prev_judg_for_beads="${JUDGMENTS_DIR}/judgment_v$((i - 1)).json"
        beads_sync_gaps "$i" "$judgment" "$prev_judg_for_beads"

        # ── Step 3: Check stopping condition ──
        local decision
        decision=$(check_stop "$judgment" "$i")

        case "$decision" in
            regressed)
                stop_reason="regressed"
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

    [[ $best_version -eq 0 ]] && best_version=1

    # ── Optional Polish Pass ──
    if [[ "$POLISH_ON_EXIT" == "true" && $best_version -gt 0 ]]; then
        log_section "POLISH PASS"
        run_polish "${VERSIONS_DIR}/ddis_v${best_version}.md" "${VERSIONS_DIR}/ddis_final.md"
    else
        cp "${VERSIONS_DIR}/ddis_v${best_version}.md" "${VERSIONS_DIR}/ddis_final.md"
    fi

    beads_finalize
    print_summary "$best_version" "$stop_reason"

    cp "${VERSIONS_DIR}/ddis_final.md" "${SCRIPT_DIR}/ddis_final.md"
    log ""
    log "Done. Final spec: ${SCRIPT_DIR}/ddis_final.md"
}

main "$@"
