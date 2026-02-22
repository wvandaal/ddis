#!/usr/bin/env bash
set -euo pipefail

# Allow nested claude -p calls when run from within a Claude Code session
unset CLAUDECODE 2>/dev/null || true

# Use Claude Max subscription auth, not API key (which may have low/no credits)
unset ANTHROPIC_API_KEY 2>/dev/null || true

# ═══════════════════════════════════════════════════════════════════════════════
# DDIS RALPH Loop: Recursive Autonomous LLM-driven Progressive Honing
# ═══════════════════════════════════════════════════════════════════════════════
#
# Uses Claude Code (`claude -p`) to recursively improve the DDIS specification
# standard using its own methodology. Each iteration:
#   1. IMPROVE: An LLM reads the current spec + improvement prompt → produces next version
#   2. JUDGE:   A separate LLM call compares versions → structured quality assessment
#   3. DECIDE:  Script checks multi-signal stopping condition
#
# Stopping condition (the hard problem):
#
#   Naive approaches fail:
#     - Fixed iteration count: arbitrary, wastes budget or stops too early
#     - "No changes": LLMs always produce changes, even cosmetic ones
#     - Self-assessment: the improver grades its own homework
#
#   Our approach uses THREE independent signals, any one of which can trigger stop:
#
#   Signal 1: DIMINISHING RETURNS
#     The judge counts substantive improvements (structural, not cosmetic).
#     When this drops below a threshold, the spec has converged.
#
#   Signal 2: QUALITY PLATEAU
#     The judge scores each version 0-100 against DDIS's own invariants.
#     When the delta between consecutive scores < threshold, further iteration
#     is reshuffling, not improving.
#
#   Signal 3: REGRESSION DETECTION
#     The judge checks whether any DDIS 1.0 quality gates regressed.
#     If a version is WORSE than its predecessor, we stop and keep the previous.
#
#   Safety valve: MAX_ITERATIONS caps total runs regardless of signals.
#
#   The judge is a SEPARATE LLM call from the improver, preventing self-evaluation
#   bias. The judge also feeds forward — each improver iteration receives the
#   previous judgment, creating a tighter feedback loop.
#
# ═══════════════════════════════════════════════════════════════════════════════

# ─── Configuration ────────────────────────────────────────────────────────────

MAX_ITERATIONS=${DDIS_MAX_ITERATIONS:-5}           # Safety cap (each iteration is expensive)
MIN_SUBSTANTIVE_IMPROVEMENTS=${DDIS_MIN_IMPROVEMENTS:-2}  # Signal 1 threshold
MIN_QUALITY_DELTA=${DDIS_MIN_DELTA:-3}             # Signal 2: minimum score improvement to continue
IMPROVER_MODEL=${DDIS_IMPROVER_MODEL:-"opus"}      # Best reasoning for improvement
JUDGE_MODEL=${DDIS_JUDGE_MODEL:-"sonnet"}           # Structured evaluation, lower cost
POLISH_ON_EXIT=${DDIS_POLISH:-true}                # Run a consolidation pass on final version
USE_BEADS=${DDIS_USE_BEADS:-auto}                  # "auto" = use if br/bv found, "yes" = require, "no" = skip
AUTO_MODULARIZE=${DDIS_AUTO_MODULARIZE:-true}      # Phase 0: auto-assess monoliths for modularization

# ─── Modular Mode Configuration ─────────────────────────────────────────────
MODULAR=false                                       # Set by --modular or --manifest
MANIFEST_PATH=""                                    # Path to manifest.yaml
MODULAR_PHASE="both"                               # "1" = constitution only, "2" = modules only, "both"

# ─── CLI Argument Parsing ────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --modular)   MODULAR=true; shift ;;
        --manifest)  MANIFEST_PATH="$2"; MODULAR=true; shift 2 ;;
        --phase)     MODULAR_PHASE="$2"; shift 2 ;;
        *)           break ;;
    esac
done

# ─── Directory Setup ──────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK_DIR="${SCRIPT_DIR}/ddis-evolution"
VERSIONS_DIR="${WORK_DIR}/versions"
JUDGMENTS_DIR="${WORK_DIR}/judgments"
LOGS_DIR="${WORK_DIR}/logs"

# Source files (expected in same directory as script)
SEED_SPEC="${SCRIPT_DIR}/ddis_standard.md"
IMPROVEMENT_PROMPT="${SCRIPT_DIR}/ddis_recursive_improvement_prompt.md"
KICKOFF_PROMPT="${SCRIPT_DIR}/kickoff_prompt.md"

# ─── Preflight Checks ────────────────────────────────────────────────────────

check_prereqs() {
    local missing=0

    if ! command -v claude &>/dev/null; then
        echo "ERROR: 'claude' CLI not found. Install Claude Code first." >&2
        echo "  npm install -g @anthropic-ai/claude-code" >&2
        missing=1
    fi

    if ! command -v jq &>/dev/null; then
        echo "ERROR: 'jq' not found. Install it:" >&2
        echo "  brew install jq  OR  apt-get install jq" >&2
        missing=1
    fi

    for f in "$SEED_SPEC" "$IMPROVEMENT_PROMPT" "$KICKOFF_PROMPT"; do
        if [[ ! -f "$f" ]]; then
            echo "ERROR: Required file not found: $f" >&2
            missing=1
        fi
    done

    if [[ $missing -ne 0 ]]; then
        exit 1
    fi
}

# ─── Beads Integration ────────────────────────────────────────────────────────
#
# br/bv are used for GAP TRACKING, not loop control. The judge still drives
# stopping decisions. Beads adds:
#   1. Pre-loop:  Initialize a beads workspace for the evolution project
#   2. Per-iteration: Convert judge's remaining_gaps into br issues; close resolved ones
#   3. Post-loop: Persist final gaps as a real backlog; run bv triage for prioritized next steps
#   4. Cross-run: If a previous beads workspace exists, feed persistent gaps into iteration 1
#
# This is deliberately lightweight. We don't use bv for loop control because
# the loop is 3-5 iterations — not enough nodes for graph analysis to matter.
# The value is persistence and triage of the RESIDUAL gaps after the loop ends.

BEADS_AVAILABLE=false
BV_AVAILABLE=false

check_beads() {
    if [[ "$USE_BEADS" == "no" ]]; then
        log "Beads integration disabled (DDIS_USE_BEADS=no)"
        return
    fi

    if command -v br &>/dev/null; then
        BEADS_AVAILABLE=true
    fi

    if command -v bv &>/dev/null; then
        BV_AVAILABLE=true
    fi

    if [[ "$USE_BEADS" == "yes" ]]; then
        if ! $BEADS_AVAILABLE; then
            echo "ERROR: DDIS_USE_BEADS=yes but 'br' not found." >&2
            echo "  Install: curl -fsSL https://raw.githubusercontent.com/Dicklesworthstone/beads_rust/main/install.sh | bash" >&2
            exit 1
        fi
    fi

    if $BEADS_AVAILABLE; then
        log "Beads integration: br=$(which br)${BV_AVAILABLE:+, bv=$(which bv)}"
    else
        log "Beads integration: not available (install br for gap tracking)"
    fi
}

# Initialize or reuse beads workspace in the evolution directory
beads_init() {
    $BEADS_AVAILABLE || return 0

    if [[ ! -f "${WORK_DIR}/.beads/beads.db" ]]; then
        log "BEADS: Initializing workspace in ${WORK_DIR}"
        (cd "$WORK_DIR" && br init --quiet 2>/dev/null) || true
    else
        log "BEADS: Reusing existing workspace ($(cd "$WORK_DIR" && br count 2>/dev/null || echo '?') issues)"
    fi
}

# Sync judge's remaining_gaps into beads issues.
# - New gaps → create issues (label: iteration-N, priority based on gap position)
# - Gaps from previous iteration that are no longer in remaining_gaps → close as resolved
beads_sync_gaps() {
    $BEADS_AVAILABLE || return 0

    local iteration=$1
    local judgment_file="$2"
    local prev_judgment="${3:-}"

    log "BEADS: Syncing gaps from iteration $iteration"

    # Extract current gaps
    local gaps
    gaps=$(jq -r '.remaining_gaps[]' "$judgment_file" 2>/dev/null) || return 0

    # Close gaps from previous iteration that no longer appear
    if [[ -n "$prev_judgment" && -f "$prev_judgment" ]]; then
        local prev_gaps
        prev_gaps=$(jq -r '.remaining_gaps[]' "$prev_judgment" 2>/dev/null) || true

        while IFS= read -r prev_gap; do
            [[ -z "$prev_gap" ]] && continue
            if ! echo "$gaps" | grep -qF "$prev_gap"; then
                # This gap was resolved — find and close the matching issue
                local issue_id
                issue_id=$(cd "$WORK_DIR" && br search "$prev_gap" --json 2>/dev/null \
                    | jq -r '.[0].id // empty' 2>/dev/null) || true
                if [[ -n "$issue_id" ]]; then
                    (cd "$WORK_DIR" && br close "$issue_id" \
                        --reason "Resolved in iteration $iteration" --quiet 2>/dev/null) || true
                    log "BEADS: Closed $issue_id (gap resolved)"
                fi
            fi
        done <<< "$prev_gaps"
    fi

    # Create issues for new gaps (skip if already exists)
    local priority=1
    while IFS= read -r gap; do
        [[ -z "$gap" ]] && continue

        # Check if this gap already has an open issue
        local existing
        existing=$(cd "$WORK_DIR" && br search "$gap" --json 2>/dev/null \
            | jq -r '[.[] | select(.status == "open")] | length' 2>/dev/null) || existing=0

        if [[ "$existing" == "0" ]]; then
            (cd "$WORK_DIR" && br create "$gap" \
                --type task \
                --priority "$priority" \
                --quiet 2>/dev/null) || true
            # Label with iteration number for tracking persistence
            local new_id
            new_id=$(cd "$WORK_DIR" && br list --json 2>/dev/null \
                | jq -r 'sort_by(.created_at) | last | .id // empty' 2>/dev/null) || true
            if [[ -n "$new_id" ]]; then
                (cd "$WORK_DIR" && br label add "$new_id" "iter-$iteration" "ddis-gap" --quiet 2>/dev/null) || true
            fi
        fi

        ((priority++)) || true
    done <<< "$gaps"

    local open_count
    open_count=$(cd "$WORK_DIR" && br count --status open 2>/dev/null | grep -oP '\d+' | head -1) || open_count="?"
    log "BEADS: $open_count open gaps tracked"
}

# Get bv triage output for feeding into the improvement prompt
# Returns empty string if bv not available or no issues exist
beads_get_triage() {
    $BV_AVAILABLE || { echo ""; return 0; }

    local triage
    triage=$(cd "$WORK_DIR" && bv --robot-triage --format toon 2>/dev/null) || triage=""

    echo "$triage"
}

# Get persistent gaps from a previous RALPH run (if beads workspace exists)
beads_get_persistent_gaps() {
    $BEADS_AVAILABLE || { echo ""; return 0; }

    if [[ ! -f "${WORK_DIR}/.beads/beads.db" ]]; then
        echo ""
        return 0
    fi

    local stale
    stale=$(cd "$WORK_DIR" && br list --status open --json 2>/dev/null \
        | jq -r '.[].title' 2>/dev/null) || stale=""

    echo "$stale"
}

# Post-loop: generate final triage report and sync to git
beads_finalize() {
    $BEADS_AVAILABLE || return 0

    log "BEADS: Generating final gap report"

    # Sync to JSONL for persistence
    (cd "$WORK_DIR" && br sync --flush-only --quiet 2>/dev/null) || true

    # Print summary
    local stats
    stats=$(cd "$WORK_DIR" && br stats 2>/dev/null) || stats="(stats unavailable)"
    log "BEADS: Issue stats: $stats"

    # If bv available, print triage
    if $BV_AVAILABLE; then
        local open_count
        open_count=$(cd "$WORK_DIR" && br count --status open 2>/dev/null | grep -oP '\d+' | head -1) || open_count=0
        if [[ "$open_count" -gt 0 ]]; then
            log ""
            log "BEADS: bv triage of remaining gaps:"
            log "─────────────────────────────────────"
            (cd "$WORK_DIR" && bv --robot-next 2>/dev/null) || true
            log "─────────────────────────────────────"
            log "Run 'cd ${WORK_DIR} && bv --robot-triage' for full analysis"
            log "Run 'cd ${WORK_DIR} && bv' for interactive TUI"
        fi
    fi
}

# ─── Judge Schema ─────────────────────────────────────────────────────────────
#
# The judge returns structured JSON so we can programmatically evaluate convergence.
# This schema is passed to `claude -p --json-schema`.

JUDGE_SCHEMA='{
  "type": "object",
  "properties": {
    "quality_score": {
      "type": "integer",
      "description": "Overall quality score 0-100 against DDIS own invariants and gates"
    },
    "substantive_improvements": {
      "type": "integer",
      "description": "Count of structural/meaningful improvements (not cosmetic rewording)"
    },
    "regressions": {
      "type": "integer",
      "description": "Count of quality gates or invariants that got WORSE"
    },
    "improvements_list": {
      "type": "array",
      "items": { "type": "string" },
      "description": "Brief description of each substantive improvement found"
    },
    "regressions_list": {
      "type": "array",
      "items": { "type": "string" },
      "description": "Brief description of each regression found"
    },
    "remaining_gaps": {
      "type": "array",
      "items": { "type": "string" },
      "description": "Top 3-5 remaining gaps that a next iteration should address"
    },
    "recommendation": {
      "type": "string",
      "enum": ["continue", "stop_converged", "stop_regressed", "stop_excellent"],
      "description": "Whether to continue iterating. stop_excellent means score >=95 and no remaining critical gaps"
    },
    "rationale": {
      "type": "string",
      "description": "2-3 sentence explanation of the recommendation"
    }
  },
  "required": [
    "quality_score",
    "substantive_improvements",
    "regressions",
    "improvements_list",
    "regressions_list",
    "remaining_gaps",
    "recommendation",
    "rationale"
  ]
}'

# ─── Structural Assessment Schema ──────────────────────────────────────────────
#
# Used by Phase 0 (auto-modularization) to evaluate whether a monolith
# should be decomposed. The LLM returns structured JSON matching this schema.

ASSESSMENT_SCHEMA='{
  "type": "object",
  "properties": {
    "should_modularize": {
      "type": "boolean",
      "description": "Whether the spec should be decomposed into modules"
    },
    "rationale": {
      "type": "string",
      "description": "2-3 sentence explanation of the modularization recommendation"
    },
    "usage_context_analysis": {
      "type": "string",
      "description": "How this spec is consumed: standalone implementation reference vs meta-standard consumed alongside other work"
    },
    "proposed_domains": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "name": { "type": "string", "description": "Domain name slug (e.g. meta-standard-core)" },
          "description": { "type": "string", "description": "What this domain covers" },
          "sections": {
            "type": "array",
            "items": { "type": "string" },
            "description": "Section numbers or chapter names from the monolith"
          }
        },
        "required": ["name", "description", "sections"]
      },
      "description": "Proposed domain groupings for the constitutional tiers"
    },
    "proposed_modules": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "name": { "type": "string", "description": "Module name slug (e.g. element-specs)" },
          "domain": { "type": "string", "description": "Domain this module belongs to" },
          "sections": {
            "type": "array",
            "items": { "type": "string" },
            "description": "Section numbers or chapter names to include"
          },
          "estimated_lines": { "type": "integer", "description": "Estimated line count" }
        },
        "required": ["name", "domain", "sections", "estimated_lines"]
      },
      "description": "Proposed modules with domain assignments and section mappings"
    },
    "tier_recommendation": {
      "type": "string",
      "enum": ["two-tier", "three-tier"],
      "description": "Whether to use two-tier (system + modules) or three-tier (system + domain + modules)"
    }
  },
  "required": ["should_modularize", "rationale", "usage_context_analysis", "proposed_domains", "proposed_modules", "tier_recommendation"]
}'

# ─── Helper Functions ─────────────────────────────────────────────────────────

timestamp() {
    date '+%Y-%m-%d %H:%M:%S'
}

log() {
    echo "[$(timestamp)] $*"
}

log_section() {
    echo ""
    echo "═══════════════════════════════════════════════════════════════════"
    echo "  $*"
    echo "═══════════════════════════════════════════════════════════════════"
    echo ""
}

# Get line count of a file (structural size metric)
line_count() {
    wc -l < "$1" | tr -d ' '
}

# Extract a field from a judgment JSON file
judgment_field() {
    local file="$1" field="$2"
    jq -r ".$field" "$file"
}

# ─── Improve Step ─────────────────────────────────────────────────────────────
#
# Takes current DDIS version + improvement prompt + previous judgment (if any)
# and produces the next version.

run_improve() {
    local iteration=$1
    local current_spec="$2"
    local output_spec="$3"
    local prev_judgment="${4:-}"  # Empty on first iteration
    local log_file="${LOGS_DIR}/improve_v${iteration}.log"

    local current_lines
    current_lines=$(line_count "$current_spec")
    log "IMPROVE: Reading v$((iteration - 1)) ($current_lines lines) → producing v${iteration}"

    # Build the improvement prompt dynamically.
    # On iteration 1, use the kickoff prompt verbatim + any persistent gaps from prior runs.
    # On subsequent iterations, include the previous judgment + bv triage as feedback.
    local prompt

    if [[ $iteration -eq 1 ]]; then
        # Check for persistent gaps from a previous RALPH run
        local persistent_gaps
        persistent_gaps=$(beads_get_persistent_gaps)
        local persistent_section=""
        if [[ -n "$persistent_gaps" ]]; then
            persistent_section="

## Persistent Gaps from Previous Improvement Runs

The following gaps were identified in previous RALPH runs and were never fully resolved. Pay special attention to these — they represent the hardest problems:

$(echo "$persistent_gaps" | sed 's/^/- /')
"
        fi

        prompt="$(cat "$KICKOFF_PROMPT")
${persistent_section}
---

The two files referenced above are provided below:

<file name=\"ddis_recursive_improvement_prompt.md\">
$(cat "$IMPROVEMENT_PROMPT")
</file>

<file name=\"ddis_standard.md\">
$(cat "$current_spec")
</file>

IMPORTANT: Output ONLY the complete improved DDIS standard (Artifact 2) — not the Improvement Spec (Artifact 1). Do the improvement spec work internally as your reasoning process, but your entire output should be the final, complete DDIS 2.0 document in markdown. Do not include any preamble, explanation, or wrapper — just the spec itself, starting with the first heading."
    else
        # Get bv triage if available (structured prioritization of remaining gaps)
        local triage_section=""
        local triage
        triage=$(beads_get_triage)
        if [[ -n "$triage" ]]; then
            triage_section="

## Gap Triage (from bv analysis)

The following is a dependency-aware, graph-analyzed triage of remaining gaps. Items are prioritized by impact (how many other improvements they unblock). Prefer addressing high-impact items first:

\`\`\`
${triage}
\`\`\`
"
        fi

        prompt="You are performing iteration $iteration of recursive self-improvement on the DDIS specification standard.

## Your Task

Read the current DDIS spec (version $((iteration - 1))) and the improvement methodology prompt below. Then read the JUDGE'S ASSESSMENT of the current version — this tells you exactly what the previous iteration got right, what it got wrong, and what gaps remain.

Your job: produce the next version that addresses the judge's remaining gaps while preserving all existing strengths. Do NOT regress on anything the judge marked as good.

## Critical Constraints

- Output ONLY the complete improved DDIS standard — no preamble, no explanation, no wrapper
- Start directly with the first heading of the spec
- Every improvement must be structural/substantive, not cosmetic rewording
- The spec must remain self-bootstrapping (conform to the format it defines)
- Focus especially on LLM-optimization: the primary consumer of DDIS-conforming specs is an LLM
- Do NOT increase document length by more than 20% — if you add content, cut something less valuable

## Files

<file name=\"ddis_recursive_improvement_prompt.md\">
$(cat "$IMPROVEMENT_PROMPT")
</file>

<file name=\"ddis_current.md\">
$(cat "$current_spec")
</file>

<file name=\"judge_assessment.json\">
$(cat "$prev_judgment")
</file>

## Remaining Gaps to Address (from judge)

$(jq -r '.remaining_gaps[]' "$prev_judgment" | sed 's/^/- /')

## Regressions to Fix (from judge)

$(jq -r '.regressions_list[]' "$prev_judgment" 2>/dev/null | sed 's/^/- /' || echo "- None identified")
${triage_section}
Begin. Output only the improved spec."
    fi

    # Run the improver using JSON output to capture text across all turns.
    # The spec may exceed a single response's output token limit, so we use
    # a continuation loop: if the output looks truncated, we --resume the
    # same session and ask the model to continue from where it left off.
    local raw_improve_output accumulated_text session_id
    accumulated_text=""

    # Initial call (--effort low prevents extended thinking from consuming output tokens)
    raw_improve_output=$(echo "$prompt" | claude -p \
        --model "$IMPROVER_MODEL" \
        --output-format json \
        --max-turns 2 \
        --effort low \
        --disallowedTools "Bash,Edit,Write,Read,Glob,Grep,WebFetch,WebSearch,NotebookEdit,Task" \
        2>"$log_file")

    accumulated_text=$(echo "$raw_improve_output" | jq -r '.result // empty' 2>/dev/null)
    session_id=$(echo "$raw_improve_output" | jq -r '.session_id // empty' 2>/dev/null)

    log "IMPROVE: Initial chunk: $(echo "$accumulated_text" | wc -l | tr -d ' ') lines (session: ${session_id:0:8}...)"

    # Continuation loop: resume the same session if output looks truncated
    local max_continuations=8
    local cont=0
    while [[ $cont -lt $max_continuations ]]; do
        # Check if the output looks complete (has PART X / Master TODO / Conclusion markers)
        if echo "$accumulated_text" | tail -50 | grep -qiE '(MASTER TODO|PART X|^## Conclusion|^---$)'; then
            log "IMPROVE: Output appears complete after $((cont)) continuations"
            break
        fi

        # Check minimum viable length — if already long enough, don't continue
        local current_line_count
        current_line_count=$(echo "$accumulated_text" | wc -l | tr -d ' ')
        if [[ $current_line_count -ge $((current_lines * 8 / 10)) ]]; then
            # At least 80% of original length — might be complete even without markers
            log "IMPROVE: Output is $current_line_count lines (≥80% of original) — accepting"
            break
        fi

        # If session_id is empty, we can't resume
        if [[ -z "$session_id" ]]; then
            log "IMPROVE: No session ID for continuation — accepting partial output"
            break
        fi

        ((cont++))
        log "IMPROVE: Output truncated at $current_line_count lines — continuation $cont/$max_continuations"

        local cont_output
        cont_output=$(echo "Continue writing the spec EXACTLY from where you left off. Do not repeat any content already written. Do not add preamble or explanation. Just continue the markdown spec from the exact point of truncation." \
            | claude -p \
                --model "$IMPROVER_MODEL" \
                --output-format json \
                --max-turns 2 \
                --resume "$session_id" \
                --disallowedTools "Bash,Edit,Write,Read,Glob,Grep,WebFetch,WebSearch,NotebookEdit,Task" \
                2>>"$log_file")

        local cont_text
        cont_text=$(echo "$cont_output" | jq -r '.result // empty' 2>/dev/null)

        if [[ -z "$cont_text" ]]; then
            log "IMPROVE: Continuation $cont returned empty — stopping"
            break
        fi

        accumulated_text+=$'\n'"$cont_text"
        log "IMPROVE: Continuation $cont added $(echo "$cont_text" | wc -l | tr -d ' ') lines"
    done

    # Write the accumulated output
    echo "$accumulated_text" > "$output_spec"

    local new_lines
    new_lines=$(line_count "$output_spec")
    log "IMPROVE: Produced v${iteration} ($new_lines lines, delta: $((new_lines - current_lines)))"

    # Sanity check: if output is suspiciously short, the LLM probably failed
    if [[ $new_lines -lt 200 ]]; then
        log "WARNING: v${iteration} is only $new_lines lines — likely a failed generation"
        log "         Check log: $log_file"
        return 1
    fi

    return 0
}

# ─── Judge Step ───────────────────────────────────────────────────────────────
#
# Compares version N-1 and version N. Produces a structured assessment.
# Critically, this is a SEPARATE call from the improver — the improver does
# not grade its own homework.

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

    local prompt="You are the JUDGE in a recursive self-improvement loop for the DDIS specification standard.

## Your Role

You evaluate whether version ${iteration} is better than version $((iteration - 1)). You are NOT the author — you are an independent assessor. Be rigorous and honest. Do not give credit for cosmetic changes.

## Evaluation Criteria

1. **DDIS Self-Conformance**: Does v${iteration} satisfy its own invariants (INV-001 through INV-010) and quality gates (Gates 1-6)?
2. **LLM Optimization**: Is v${iteration} more effective for LLM consumption than v$((iteration - 1))? (structural predictability, negative specifications, explicit cross-references, anti-hallucination provisions)
3. **Substantive vs Cosmetic**: Count ONLY structural improvements (new invariants, new ADRs, new element types, fixed self-conformance violations, new LLM-specific provisions). Do NOT count rewording, reformatting, or reorganization that doesn't change meaning.
4. **Regressions**: Did anything get WORSE? Missing sections, broken cross-references, violated invariants, loss of self-bootstrapping?
5. **Remaining Gaps**: What are the top 3-5 things still missing or weak?

## Scoring Guide

- 0-30: Fundamentally broken (doesn't self-bootstrap, missing major sections)
- 31-50: Structural gaps (missing invariants, ADRs, or required elements)
- 51-70: Functional but incomplete (has structure, lacks depth in key areas)
- 71-85: Good (complete structure, most invariants satisfied, some gaps)
- 86-95: Excellent (comprehensive, self-conforming, well-optimized for LLMs)
- 96-100: Near-perfect (reserve this — almost no spec ever gets here)

## Recommendation Logic

- **continue**: Score improved by ≥ ${MIN_QUALITY_DELTA} points AND ≥ ${MIN_SUBSTANTIVE_IMPROVEMENTS} substantive improvements AND remaining gaps are addressable
- **stop_converged**: Score improved by < ${MIN_QUALITY_DELTA} points OR < ${MIN_SUBSTANTIVE_IMPROVEMENTS} substantive improvements — diminishing returns
- **stop_regressed**: Regressions > 0 AND regressions outweigh improvements — keep previous version
- **stop_excellent**: Score ≥ 95 AND no critical remaining gaps — we're done

<file name=\"ddis_v$((iteration - 1)).md\">
$(cat "$prev_spec")
</file>

<file name=\"ddis_v${iteration}.md\">
$(cat "$curr_spec")
</file>

Evaluate carefully. Return your assessment as JSON matching the required schema."

    # Run the judge with structured output
    local raw_output
    raw_output=$(echo "$prompt" | claude -p \
        --model "$JUDGE_MODEL" \
        --output-format json \
        --max-turns 10 \
        --no-session-persistence \
        --disallowedTools "Bash,Edit,Write,Read,Glob,Grep,WebFetch,WebSearch,NotebookEdit,Task" \
        --json-schema "$JUDGE_SCHEMA" \
        2>"$log_file")

    # Extract the structured output from the response
    # With --json-schema, the result field contains the JSON string
    local extracted
    extracted=$(echo "$raw_output" | jq -r '.result // empty' 2>/dev/null)
    # If the result is itself valid JSON, use it directly; otherwise try parsing as text
    if echo "$extracted" | jq empty 2>/dev/null; then
        echo "$extracted" > "$output_json"
    else
        echo "$raw_output" | jq -r '.structured_output // .result // empty' > "$output_json" 2>/dev/null
    fi

    # Validate we got valid JSON
    if ! jq empty "$output_json" 2>/dev/null; then
        log "WARNING: Judge produced invalid JSON. Raw output saved to $log_file"
        # Write a fallback judgment that triggers continuation
        cat > "$output_json" <<'FALLBACK'
{
    "quality_score": 50,
    "substantive_improvements": 5,
    "regressions": 0,
    "improvements_list": ["Unable to parse judge output — assuming improvements exist"],
    "regressions_list": [],
    "remaining_gaps": ["Judge evaluation failed — re-evaluate on next iteration"],
    "recommendation": "continue",
    "rationale": "Judge output was unparseable. Defaulting to continue to avoid premature termination."
}
FALLBACK
    fi

    # Pretty-print the judgment
    local score improvements regressions recommendation
    score=$(judgment_field "$output_json" "quality_score")
    improvements=$(judgment_field "$output_json" "substantive_improvements")
    regressions=$(judgment_field "$output_json" "regressions")
    recommendation=$(judgment_field "$output_json" "recommendation")

    log "JUDGE: Score=$score | Improvements=$improvements | Regressions=$regressions | Recommendation=$recommendation"

    return 0
}

# ─── Polish Step ──────────────────────────────────────────────────────────────
#
# Optional final pass that consolidates rather than adds. Removes bloat that
# accumulated during improvement iterations, tightens prose, ensures proportional
# weight is respected. Does NOT add new concepts.

run_polish() {
    local input_spec="$1"
    local output_spec="$2"
    local log_file="${LOGS_DIR}/polish.log"

    local input_lines
    input_lines=$(line_count "$input_spec")
    log "POLISH: Consolidating v-final ($input_lines lines)"

    local prompt="You are performing a FINAL POLISH pass on the DDIS specification standard.

## Your Role

This is NOT an improvement iteration. Do NOT add new concepts, invariants, ADRs, or sections.

Your job is strictly to:
1. Remove any redundancy or bloat that accumulated during recursive improvement
2. Tighten prose (shorter sentences, fewer hedge words)
3. Verify proportional weight (PART II should be 35-45% of total)
4. Ensure cross-references are all valid (no broken section references)
5. Verify the Master TODO reflects the actual document contents
6. Ensure self-bootstrapping: the document conforms to the format it defines

## Hard Constraints

- Do NOT add new invariants, ADRs, sections, or concepts
- Do NOT increase document length — target 5-10% REDUCTION
- Do NOT change the meaning of any invariant or ADR
- Output ONLY the polished spec — no preamble, no explanation

<file name=\"ddis_final.md\">
$(cat "$input_spec")
</file>

Polish it. Output only the spec."

    # Same continuation strategy as run_improve
    local raw_polish_output accumulated_text session_id
    accumulated_text=""

    raw_polish_output=$(echo "$prompt" | claude -p \
        --model "$IMPROVER_MODEL" \
        --output-format json \
        --max-turns 2 \
        --disallowedTools "Bash,Edit,Write,Read,Glob,Grep,WebFetch,WebSearch,NotebookEdit,Task" \
        2>"$log_file")

    accumulated_text=$(echo "$raw_polish_output" | jq -r '.result // empty' 2>/dev/null)
    session_id=$(echo "$raw_polish_output" | jq -r '.session_id // empty' 2>/dev/null)

    local max_continuations=8
    local cont=0
    while [[ $cont -lt $max_continuations ]]; do
        if echo "$accumulated_text" | tail -50 | grep -qiE '(MASTER TODO|PART X|^## Conclusion|^---$)'; then
            break
        fi
        local current_line_count
        current_line_count=$(echo "$accumulated_text" | wc -l | tr -d ' ')
        if [[ $current_line_count -ge $((input_lines * 8 / 10)) ]]; then
            break
        fi
        if [[ -z "$session_id" ]]; then
            break
        fi

        ((cont++))
        log "POLISH: Continuation $cont — at $current_line_count lines"

        local cont_output
        cont_output=$(echo "Continue writing the spec EXACTLY from where you left off. Do not repeat content. No preamble. Just continue the markdown." \
            | claude -p \
                --model "$IMPROVER_MODEL" \
                --output-format json \
                --max-turns 2 \
                --resume "$session_id" \
                --disallowedTools "Bash,Edit,Write,Read,Glob,Grep,WebFetch,WebSearch,NotebookEdit,Task" \
                2>>"$log_file")

        local cont_text
        cont_text=$(echo "$cont_output" | jq -r '.result // empty' 2>/dev/null)
        if [[ -z "$cont_text" ]]; then
            break
        fi
        accumulated_text+=$'\n'"$cont_text"
    done

    echo "$accumulated_text" > "$output_spec"

    local output_lines
    output_lines=$(line_count "$output_spec")
    log "POLISH: Produced final ($output_lines lines, delta: $((output_lines - input_lines)))"
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

    # Signal 3: Regression detected
    if [[ "$recommendation" == "stop_regressed" ]] || [[ $regressions -gt 0 && $regressions -ge $improvements ]]; then
        log "STOP: Regression detected in v${iteration}. Keeping v$((iteration - 1))."
        echo "regressed"
        return 0
    fi

    # Signal: Judge says excellent
    if [[ "$recommendation" == "stop_excellent" ]]; then
        log "STOP: Judge rates v${iteration} as excellent (score=$score). Done."
        echo "excellent"
        return 0
    fi

    # Signal 1: Diminishing returns (too few substantive improvements)
    if [[ $improvements -lt $MIN_SUBSTANTIVE_IMPROVEMENTS ]]; then
        log "STOP: Only $improvements substantive improvements (threshold: $MIN_SUBSTANTIVE_IMPROVEMENTS). Converged."
        echo "converged"
        return 0
    fi

    # Signal 2: Quality plateau (score didn't improve enough)
    # We need the previous score for this — check if we have a prior judgment
    local prev_judgment="${JUDGMENTS_DIR}/judgment_v$((iteration - 1)).json"
    if [[ -f "$prev_judgment" ]]; then
        local prev_score
        prev_score=$(judgment_field "$prev_judgment" "quality_score")
        local delta=$((score - prev_score))
        if [[ $delta -lt $MIN_QUALITY_DELTA ]]; then
            log "STOP: Quality delta=$delta (threshold: $MIN_QUALITY_DELTA). Plateau reached."
            echo "plateau"
            return 0
        fi
    fi

    # Signal: Judge says converged
    if [[ "$recommendation" == "stop_converged" ]]; then
        log "STOP: Judge recommends stopping (converged)."
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

    # Table header
    printf "%-8s  %6s  %7s  %5s  %5s  %-12s\n" \
        "VERSION" "LINES" "SCORE" "IMPRV" "REGR" "RECOMMEND"
    printf "%-8s  %6s  %7s  %5s  %5s  %-12s\n" \
        "-------" "------" "-------" "-----" "-----" "------------"

    # V0 (seed) — no judgment
    printf "%-8s  %6s  %7s  %5s  %5s  %-12s\n" \
        "v0" "$(line_count "$VERSIONS_DIR/ddis_v0.md")" "—" "—" "—" "seed"

    # Each subsequent version
    for ((i = 1; i <= final_version; i++)); do
        local spec_file="${VERSIONS_DIR}/ddis_v${i}.md"
        local judg_file="${JUDGMENTS_DIR}/judgment_v${i}.json"

        if [[ -f "$judg_file" ]]; then
            printf "%-8s  %6s  %7s  %5s  %5s  %-12s\n" \
                "v${i}" \
                "$(line_count "$spec_file")" \
                "$(judgment_field "$judg_file" "quality_score")" \
                "$(judgment_field "$judg_file" "substantive_improvements")" \
                "$(judgment_field "$judg_file" "regressions")" \
                "$(judgment_field "$judg_file" "recommendation")"
        fi
    done

    # Final output
    if [[ -f "${VERSIONS_DIR}/ddis_final.md" ]]; then
        echo ""
        echo "Final polished version: ${VERSIONS_DIR}/ddis_final.md ($(line_count "${VERSIONS_DIR}/ddis_final.md") lines)"
    fi

    echo ""
    echo "All versions:  ${VERSIONS_DIR}/"
    echo "All judgments:  ${JUDGMENTS_DIR}/"
    echo "All logs:       ${LOGS_DIR}/"

    if $BEADS_AVAILABLE; then
        echo "Beads tracker:  ${WORK_DIR}/.beads/"
        local open_gaps
        open_gaps=$(cd "$WORK_DIR" && br count --status open 2>/dev/null | grep -oP '\d+' | head -1) || open_gaps=0
        local closed_gaps
        closed_gaps=$(cd "$WORK_DIR" && br count --status closed 2>/dev/null | grep -oP '\d+' | head -1) || closed_gaps=0
        echo "Gaps resolved:  $closed_gaps"
        echo "Gaps remaining: $open_gaps"
    fi

    # Print the final judgment's remaining gaps (even if we stopped)
    local last_judgment="${JUDGMENTS_DIR}/judgment_v${final_version}.json"
    if [[ -f "$last_judgment" ]]; then
        echo ""
        echo "Remaining gaps in final version:"
        jq -r '.remaining_gaps[]' "$last_judgment" | sed 's/^/  • /'
        echo ""
        echo "Judge's rationale: $(judgment_field "$last_judgment" "rationale")"
    fi
}

# ─── Auto-Modularization Functions ──────────────────────────────────────────
#
# Phase 0: Before the RALPH improvement cycle, assess whether a monolith
# should be decomposed into modules. If yes, decompose it and seamlessly
# transition to modular mode.
#
# The threshold isn't "is the spec too big to read?" — it's "does the spec
# leave enough room for the LLM to do useful work alongside it?" A spec
# consumed as a reference (like DDIS itself) needs modularization at a
# lower line count than a standalone spec.

# Parse <!-- FILE: path --> ... <!-- END FILE: path --> markers from a text
# file and write each extracted file to the output directory.
parse_file_markers() {
    local content_file="$1"
    local output_dir="$2"

    python3 - "$content_file" "$output_dir" <<'PYEOF'
import sys, re, os
with open(sys.argv[1]) as f:
    content = f.read()
output_dir = sys.argv[2]
# Primary pattern: explicit start and end markers
pattern = r'<!-- FILE: (.+?) -->\s*\n(.*?)<!-- END FILE: .+? -->'
matches = re.findall(pattern, content, re.DOTALL)
if not matches:
    # Fallback: markers without explicit END FILE
    pattern = r'<!-- FILE: (.+?) -->\s*\n(.*?)(?=<!-- FILE:|$)'
    matches = re.findall(pattern, content, re.DOTALL)
count = 0
for filepath, body in matches:
    filepath = filepath.strip()
    full_path = os.path.join(output_dir, filepath)
    os.makedirs(os.path.dirname(full_path), exist_ok=True)
    with open(full_path, 'w') as f:
        f.write(body.strip() + '\n')
    print(f"  Written: {full_path}")
    count += 1
print(f"  Total: {count} files written")
PYEOF
}

# Assess whether a monolith spec should be decomposed into modules.
# Returns structured JSON with the decision, rationale, and proposed structure.
run_structural_assessment() {
    local monolith="$1"
    local output_json="$2"
    local log_file="${LOGS_DIR}/structural_assessment.log"

    local monolith_lines
    monolith_lines=$(line_count "$monolith")
    log "ASSESSMENT: Evaluating monolith ($monolith_lines lines) for modularization"

    local prompt="You are evaluating whether a monolithic DDIS specification should be decomposed into modules using the DDIS Modularization Protocol (§0.13).

## Context

The spec below is $monolith_lines lines long. It may be consumed by LLMs in two ways:
1. **Standalone**: An LLM reads the spec to implement the system it describes
2. **Reference**: An LLM reads the spec as a meta-standard to write OTHER specs — it must hold this spec in context PLUS have room to produce a new spec

The modularization decision depends on BOTH the spec's size AND its usage context.

## Decision Framework (from §0.13.7)

- Spec > 4,000 lines → REQUIRED (exceeds safe bundle budget regardless of usage)
- Spec > 2,500 lines AND used as reference alongside other work → RECOMMENDED
- Spec > 2,500 lines AND standalone → OPTIONAL (fits in context alone)
- Spec ≤ 2,500 lines with no context pressure → MONOLITH (overhead not justified)

Key insight: A spec consumed as a reference leaves LESS room for the LLM's work product. The effective context budget is tighter because the LLM must hold both the reference AND produce output. For a ~2,300-line meta-standard, the LLM consuming it to write a new spec would need ~2,300 lines for the reference + ~2,000-3,000 lines for the output spec + reasoning overhead.

## Your Task

1. Analyze this spec's content structure and intended usage
2. Determine whether modularization is warranted
3. If yes, propose domain groupings and module boundaries
4. Recommend two-tier vs three-tier based on number of distinct domains

<file name=\"spec.md\">
$(cat "$monolith")
</file>

Return your structured assessment."

    local raw_output
    raw_output=$(echo "$prompt" | claude -p \
        --model "$JUDGE_MODEL" \
        --output-format json \
        --max-turns 2 \
        --no-session-persistence \
        --disallowedTools "Bash,Edit,Write,Read,Glob,Grep,WebFetch,WebSearch,NotebookEdit,Task" \
        --json-schema "$ASSESSMENT_SCHEMA" \
        2>"$log_file")

    # Extract structured JSON from response
    local extracted
    extracted=$(echo "$raw_output" | jq -r '.result // empty' 2>/dev/null)
    if echo "$extracted" | jq empty 2>/dev/null; then
        echo "$extracted" > "$output_json"
    else
        echo "$raw_output" | jq -r '.structured_output // .result // empty' > "$output_json" 2>/dev/null
    fi

    if ! jq empty "$output_json" 2>/dev/null; then
        log "WARNING: Structural assessment produced invalid JSON"
        return 1
    fi

    local should_modularize rationale tier_rec module_count
    should_modularize=$(jq -r '.should_modularize' "$output_json")
    rationale=$(jq -r '.rationale' "$output_json")
    tier_rec=$(jq -r '.tier_recommendation' "$output_json")
    module_count=$(jq '.proposed_modules | length' "$output_json")

    log "ASSESSMENT: should_modularize=$should_modularize | tier=$tier_rec | modules=$module_count"
    log "ASSESSMENT: $rationale"

    return 0
}

# Decompose a monolith into a modular structure (manifest + constitutions + modules).
# Uses two LLM calls:
#   Phase 1: Generate manifest.yaml + all constitution files (interdependent)
#   Phase 2: Generate all module files (based on manifest from Phase 1)
# Validates the result and attempts one correction pass if validation fails.
run_decomposition() {
    local monolith="$1"
    local assessment_json="$2"
    local output_dir="$3"
    local log_file="${LOGS_DIR}/decomposition.log"

    mkdir -p "$output_dir/constitution" "$output_dir/constitution/domains" "$output_dir/modules"

    local monolith_lines tier_rec module_count
    monolith_lines=$(line_count "$monolith")
    tier_rec=$(jq -r '.tier_recommendation' "$assessment_json")
    module_count=$(jq '.proposed_modules | length' "$assessment_json")

    log "DECOMPOSITION: Decomposing monolith ($monolith_lines lines) into $module_count modules ($tier_rec)"

    # ── Decomposition Phase 1: Manifest + Constitution ──

    local phase1_prompt="You are decomposing a monolithic DDIS specification into a modular structure per §0.13 (Modularization Protocol).

## Structural Assessment

The following assessment was produced by analyzing the monolith:
\`\`\`json
$(cat "$assessment_json")
\`\`\`

## Your Task

Produce the manifest.yaml and ALL constitution files for this decomposition. Follow the schema defined in §0.13.2–§0.13.6 of the spec itself.

## Constitution Content Guidelines

**System constitution (Tier 1)** — the shared substrate every module needs:
- Preamble and formal model
- Invariant DECLARATIONS (ID + one-line summary, NOT full definitions)
- ADR declarations (ID + title + status, NOT full analysis)
- Quality gate checklist (gate ID + name + pass criteria summary)
- Non-negotiables and glossary summary
- Module catalog (list of all modules with one-line descriptions)

**Domain constitutions (Tier 2, if three-tier)** — per-domain shared context:
- Full invariant DEFINITIONS for this domain's invariants
- Full ADR analysis for this domain's decisions
- Domain-specific guidance and patterns

## Output Format

Each file between markers:

<!-- FILE: manifest.yaml -->
(yaml content)
<!-- END FILE: manifest.yaml -->

<!-- FILE: constitution/system.md -->
(markdown content)
<!-- END FILE: constitution/system.md -->

Use paths relative to the manifest: constitution/system.md, constitution/domains/X.md, modules/X.md.

## Manifest Schema Requirements

\`\`\`yaml
spec_name: \"...\"
version: \"...\"
assembly:
  tier_model: $tier_rec
  target_budget: 4000
  ceiling_budget: 5000
constitution:
  system: constitution/system.md
  domains:  # if three-tier
    domain-name:
      file: constitution/domains/domain-name.md
modules:
  module-name:
    file: modules/module-name.md
    domain: domain-name
    maintains: [INV-XXX]
    interfaces_with: [INV-YYY]
    adjacent: [other-module]
    budget_lines: NNNN
\`\`\`

## The Monolith Spec

<file name=\"ddis_standard.md\">
$(cat "$monolith")
</file>

Produce the manifest and all constitution files now. Use FILE markers for each output file."

    local raw_output accumulated_text session_id
    accumulated_text=""

    raw_output=$(echo "$phase1_prompt" | claude -p \
        --model "$IMPROVER_MODEL" \
        --output-format json \
        --max-turns 2 \
        --effort low \
        --disallowedTools "Bash,Edit,Write,Read,Glob,Grep,WebFetch,WebSearch,NotebookEdit,Task" \
        2>"$log_file")

    accumulated_text=$(echo "$raw_output" | jq -r '.result // empty' 2>/dev/null)
    session_id=$(echo "$raw_output" | jq -r '.session_id // empty' 2>/dev/null)

    log "DECOMPOSITION: Phase 1 initial: $(echo "$accumulated_text" | wc -l | tr -d ' ') lines"

    # Continuation loop for constitution generation
    local max_continuations=8
    local cont=0
    while [[ $cont -lt $max_continuations ]]; do
        # Check if we have manifest + at least one constitution file
        if echo "$accumulated_text" | grep -q '<!-- END FILE: manifest.yaml'; then
            local const_ends
            const_ends=$(echo "$accumulated_text" | grep -c '<!-- END FILE: constitution/' || true)
            if [[ $const_ends -ge 1 ]]; then
                log "DECOMPOSITION: Phase 1 complete (manifest + $const_ends constitution files)"
                break
            fi
        fi
        if [[ -z "$session_id" ]]; then
            break
        fi
        ((cont++))
        log "DECOMPOSITION: Phase 1 continuation $cont"

        local cont_output cont_text
        cont_output=$(echo "Continue producing the remaining files. Do not repeat any file already output. Use the same <!-- FILE: path --> markers." \
            | claude -p \
                --model "$IMPROVER_MODEL" \
                --output-format json \
                --max-turns 2 \
                --resume "$session_id" \
                --disallowedTools "Bash,Edit,Write,Read,Glob,Grep,WebFetch,WebSearch,NotebookEdit,Task" \
                2>>"$log_file")

        cont_text=$(echo "$cont_output" | jq -r '.result // empty' 2>/dev/null)
        if [[ -z "$cont_text" ]]; then
            break
        fi
        accumulated_text+=$'\n'"$cont_text"
    done

    # Write Phase 1 output to temp file and parse FILE markers
    local phase1_raw="${output_dir}/.phase1_raw.txt"
    echo "$accumulated_text" > "$phase1_raw"

    log "DECOMPOSITION: Parsing manifest and constitution files..."
    parse_file_markers "$phase1_raw" "$output_dir"

    if [[ ! -f "${output_dir}/manifest.yaml" ]]; then
        log "ERROR: Decomposition Phase 1 failed to produce manifest.yaml"
        return 1
    fi

    # ── Decomposition Phase 2: Module files ──

    local modules_prompt="You are continuing a DDIS spec decomposition. The manifest and constitution files are done. Now produce ALL module files.

## Module Header Format (per §0.13.9)

Each module file must start with a proper header:

\`\`\`markdown
# Module: <Name>
<!-- domain: <domain-name> -->
<!-- maintains: INV-XXX, INV-YYY -->
<!-- interfaces_with: INV-AAA, INV-BBB -->
<!-- adjacent: module-a, module-b -->
<!-- budget: NNNN lines -->

## Negative Specifications
- This module MUST NOT ...
\`\`\`

## The Manifest

\`\`\`yaml
$(cat "${output_dir}/manifest.yaml")
\`\`\`

## The Monolith Spec

<file name=\"ddis_standard.md\">
$(cat "$monolith")
</file>

## Instructions

For each module in the manifest, extract the relevant content from the monolith and wrap it with a proper module header per §0.13.9. Include negative specifications for each module.

Output each module between FILE markers:

<!-- FILE: modules/module-name.md -->
(module content)
<!-- END FILE: modules/module-name.md -->

Use the exact file paths from the manifest. Extract and restructure content from the monolith — preserve substance, add module headers and negative specs."

    accumulated_text=""
    session_id=""

    raw_output=$(echo "$modules_prompt" | claude -p \
        --model "$IMPROVER_MODEL" \
        --output-format json \
        --max-turns 2 \
        --effort low \
        --disallowedTools "Bash,Edit,Write,Read,Glob,Grep,WebFetch,WebSearch,NotebookEdit,Task" \
        2>>"$log_file")

    accumulated_text=$(echo "$raw_output" | jq -r '.result // empty' 2>/dev/null)
    session_id=$(echo "$raw_output" | jq -r '.session_id // empty' 2>/dev/null)

    log "DECOMPOSITION: Phase 2 initial: $(echo "$accumulated_text" | wc -l | tr -d ' ') lines"

    # Continuation loop for module generation
    cont=0
    while [[ $cont -lt $max_continuations ]]; do
        local current_modules
        current_modules=$(echo "$accumulated_text" | grep -c '<!-- END FILE: modules/' || true)
        if [[ $current_modules -ge $module_count ]]; then
            log "DECOMPOSITION: Phase 2 complete ($current_modules/$module_count modules)"
            break
        fi
        if [[ -z "$session_id" ]]; then
            break
        fi
        ((cont++))
        log "DECOMPOSITION: Phase 2 continuation $cont ($current_modules/$module_count modules)"

        local cont_output cont_text
        cont_output=$(echo "Continue producing the remaining module files. Do not repeat any module already written. Use the same FILE marker format." \
            | claude -p \
                --model "$IMPROVER_MODEL" \
                --output-format json \
                --max-turns 2 \
                --resume "$session_id" \
                --disallowedTools "Bash,Edit,Write,Read,Glob,Grep,WebFetch,WebSearch,NotebookEdit,Task" \
                2>>"$log_file")

        cont_text=$(echo "$cont_output" | jq -r '.result // empty' 2>/dev/null)
        if [[ -z "$cont_text" ]]; then
            break
        fi
        accumulated_text+=$'\n'"$cont_text"
    done

    # Write Phase 2 output and parse
    local phase2_raw="${output_dir}/.phase2_raw.txt"
    echo "$accumulated_text" > "$phase2_raw"

    log "DECOMPOSITION: Parsing module files..."
    parse_file_markers "$phase2_raw" "$output_dir"

    # ── Validate the decomposition ──

    log "DECOMPOSITION: Validating modular structure..."
    local validate_output
    if validate_output=$("${SCRIPT_DIR}/ddis_validate.sh" -m "${output_dir}/manifest.yaml" -v 2>&1); then
        log "DECOMPOSITION: Validation PASSED"
    else
        log "DECOMPOSITION: Validation issues detected — attempting correction"

        # List actual files for the fix prompt
        local actual_files
        actual_files=$(find "$output_dir" -type f \( -name '*.yaml' -o -name '*.md' \) 2>/dev/null \
            | sed "s|${output_dir}/||" | sort | sed 's/^/- /')

        local fix_prompt="The DDIS modular decomposition has validation errors. Fix the manifest.yaml to resolve them.

## Validation Errors

\`\`\`
${validate_output}
\`\`\`

## Current Manifest

\`\`\`yaml
$(cat "${output_dir}/manifest.yaml")
\`\`\`

## Actual Files on Disk

${actual_files}

Output ONLY the corrected manifest.yaml content — no markers, no explanation, just the YAML."

        local fix_output fixed_manifest
        fix_output=$(echo "$fix_prompt" | claude -p \
            --model "$JUDGE_MODEL" \
            --output-format json \
            --max-turns 2 \
            --no-session-persistence \
            --disallowedTools "Bash,Edit,Write,Read,Glob,Grep,WebFetch,WebSearch,NotebookEdit,Task" \
            2>>"$log_file")

        fixed_manifest=$(echo "$fix_output" | jq -r '.result // empty' 2>/dev/null)
        if [[ -n "$fixed_manifest" ]]; then
            echo "$fixed_manifest" > "${output_dir}/manifest.yaml"
            log "DECOMPOSITION: Re-validating after correction..."
            if "${SCRIPT_DIR}/ddis_validate.sh" -m "${output_dir}/manifest.yaml" -q 2>/dev/null; then
                log "DECOMPOSITION: Validation PASSED after correction"
            else
                log "WARNING: Validation still has issues. Proceeding — modular RALPH will improve."
            fi
        fi
    fi

    # Preserve the original monolith as v0
    mkdir -p "${WORK_DIR}/versions"
    cp "$monolith" "${WORK_DIR}/versions/ddis_v0.md" 2>/dev/null || true

    log "DECOMPOSITION: Complete. Output at ${output_dir}/"
    return 0
}

# ─── Modular Mode Functions ──────────────────────────────────────────────────
#
# Two-phase improvement for modular DDIS specs:
#   Phase 1: Improve the constitution (Tier 1 + Tier 2 + Tier 3). This is the
#            shared substrate — changes here affect every module's bundle.
#   Phase 2: Improve each module in dependency order. Each module is assembled
#            into a bundle (via ddis_assemble.sh), improved, then the module
#            portion is extracted back. Cascade detection flags downstream
#            modules that need re-validation after upstream changes.

# Resolve manifest path and validate modular prerequisites
modular_preflight() {
    if [[ -z "$MANIFEST_PATH" ]]; then
        MANIFEST_PATH="${SCRIPT_DIR}/manifest.yaml"
    fi
    if [[ ! -f "$MANIFEST_PATH" ]]; then
        echo "ERROR: Manifest not found: $MANIFEST_PATH" >&2
        echo "  Use --manifest PATH or place manifest.yaml next to this script." >&2
        exit 1
    fi
    MANIFEST_DIR="$(cd "$(dirname "$MANIFEST_PATH")" && pwd)"
    MANIFEST_PATH="${MANIFEST_DIR}/$(basename "$MANIFEST_PATH")"

    # Check tooling
    for tool in python3 ddis_assemble.sh ddis_validate.sh; do
        local tool_path
        if [[ "$tool" == *.sh ]]; then
            tool_path="${SCRIPT_DIR}/${tool}"
        else
            tool_path=$(command -v "$tool" 2>/dev/null) || true
        fi
        if [[ -z "$tool_path" || ! -x "$tool_path" ]]; then
            echo "ERROR: Required tool not found/executable: $tool" >&2
            exit 1
        fi
    done

    # Validate manifest
    log "MODULAR: Validating manifest..."
    if ! "${SCRIPT_DIR}/ddis_validate.sh" -m "$MANIFEST_PATH" -q 2>/dev/null; then
        log "WARNING: Manifest has validation errors. Run ddis_validate.sh for details."
    fi
}

# Get module names in dependency order (topological sort using adjacency info)
# Modules with fewer dependencies come first so upstream improvements propagate.
get_module_order() {
    python3 - "$MANIFEST_PATH" <<'PYEOF'
import sys, yaml
with open(sys.argv[1]) as f:
    m = yaml.safe_load(f)
modules = m.get("modules", {})
# Simple topological ordering: modules with fewer adjacencies first,
# cross-cutting modules last (they depend on everything)
normal = []
cross = []
for name, cfg in modules.items():
    if cfg.get("domain") == "cross-cutting":
        cross.append(name)
    else:
        adj = cfg.get("adjacent") or []
        adj_count = 0 if adj == "all" else len(adj)
        normal.append((adj_count, name))
normal.sort(key=lambda x: x[0])
for _, name in normal:
    print(name)
for name in cross:
    print(name)
PYEOF
}

# Extract constitution files list from manifest
get_constitution_files() {
    python3 - "$MANIFEST_PATH" <<'PYEOF'
import sys, yaml, os
with open(sys.argv[1]) as f:
    m = yaml.safe_load(f)
manifest_dir = os.path.dirname(os.path.abspath(sys.argv[1]))
c = m.get("constitution", {})
# System constitution
if c.get("system"):
    print(os.path.join(manifest_dir, c["system"]))
# Domain constitutions
for dname, dcfg in (c.get("domains") or {}).items():
    df = dcfg.get("file") if isinstance(dcfg, dict) else dcfg
    print(os.path.join(manifest_dir, df))
PYEOF
}

# Concatenate all constitution files into a single document for improvement
concat_constitution() {
    local output="$1"
    local files
    files=$(get_constitution_files)
    > "$output"
    while IFS= read -r f; do
        [[ -z "$f" ]] && continue
        echo "<!-- FILE: $(basename "$f") -->" >> "$output"
        cat "$f" >> "$output"
        echo "" >> "$output"
        echo "<!-- END FILE: $(basename "$f") -->" >> "$output"
        echo "" >> "$output"
    done <<< "$files"
}

# Split an improved concatenated constitution back into individual files
split_constitution() {
    local improved="$1"
    python3 - "$improved" "$MANIFEST_PATH" <<'PYEOF'
import sys, re, yaml, os
improved_path = sys.argv[1]
manifest_path = sys.argv[2]
manifest_dir = os.path.dirname(os.path.abspath(manifest_path))
with open(manifest_path) as f:
    m = yaml.safe_load(f)
with open(improved_path) as f:
    content = f.read()
# Parse FILE markers
pattern = r'<!-- FILE: (.+?) -->\n(.*?)<!-- END FILE: .+? -->'
matches = re.findall(pattern, content, re.DOTALL)
c = m.get("constitution", {})
# Build filename→path map
file_map = {}
if c.get("system"):
    file_map[os.path.basename(c["system"])] = os.path.join(manifest_dir, c["system"])
for dname, dcfg in (c.get("domains") or {}).items():
    df = dcfg.get("file") if isinstance(dcfg, dict) else dcfg
    file_map[os.path.basename(df)] = os.path.join(manifest_dir, df)
for filename, body in matches:
    filename = filename.strip()
    if filename in file_map:
        with open(file_map[filename], 'w') as f:
            f.write(body.strip() + '\n')
        print(f"  Written: {file_map[filename]}")
    else:
        print(f"  WARNING: Unknown file marker '{filename}', skipping", file=sys.stderr)
PYEOF
}

# Phase 1: Improve the constitution using the standard RALPH cycle
run_phase_1() {
    log_section "MODULAR PHASE 1: Constitution Improvement"

    local const_dir="${WORK_DIR}/constitution_versions"
    mkdir -p "$const_dir"

    # Concatenate current constitution
    concat_constitution "${const_dir}/constitution_v0.md"
    local seed_lines
    seed_lines=$(line_count "${const_dir}/constitution_v0.md")
    log "PHASE 1: Constitution seed: $seed_lines lines"

    local best_version=0
    local stop_reason="max_iterations"

    for ((i = 1; i <= MAX_ITERATIONS; i++)); do
        log "──── Phase 1, Iteration $i / $MAX_ITERATIONS ────"

        local prev="${const_dir}/constitution_v$((i - 1)).md"
        local curr="${const_dir}/constitution_v${i}.md"
        local judgment="${JUDGMENTS_DIR}/constitution_judgment_v${i}.json"
        local prev_judgment="${JUDGMENTS_DIR}/constitution_judgment_v$((i - 1)).json"

        local prev_judg_arg=""
        [[ -f "$prev_judgment" ]] && prev_judg_arg="$prev_judgment"

        if ! run_improve "$i" "$prev" "$curr" "$prev_judg_arg"; then
            log "ERROR: Constitution improvement failed at iteration $i."
            stop_reason="improve_failed"
            break
        fi

        run_judge "$i" "$prev" "$curr" "$judgment"

        local decision
        decision=$(check_stop "$judgment" "$i")
        best_version=$i

        case "$decision" in
            regressed)
                stop_reason="regressed"
                best_version=$((i - 1))
                break ;;
            excellent|converged|plateau)
                stop_reason="$decision"
                break ;;
            continue)
                log "PHASE 1: Continuing to iteration $((i + 1))" ;;
        esac
    done

    # Write improved constitution back to source files
    local best_const="${const_dir}/constitution_v${best_version}.md"
    if [[ $best_version -gt 0 && -f "$best_const" ]]; then
        log "PHASE 1: Splitting improved constitution (v${best_version}) back to source files"
        split_constitution "$best_const"
    fi

    # Re-assemble all bundles with the improved constitution
    log "PHASE 1: Re-assembling all bundles with improved constitution"
    "${SCRIPT_DIR}/ddis_assemble.sh" -m "$MANIFEST_PATH" -q || true

    log "PHASE 1: Complete (stop reason: $stop_reason)"
}

# Phase 2: Improve each module in dependency order
run_phase_2() {
    log_section "MODULAR PHASE 2: Per-Module Improvement"

    local module_order
    module_order=$(get_module_order)
    local module_count
    module_count=$(echo "$module_order" | wc -l | tr -d ' ')
    log "PHASE 2: Improving $module_count modules in dependency order"

    local module_num=0
    local cascade_modules=""  # Modules flagged for re-validation by cascade

    while IFS= read -r module_name; do
        [[ -z "$module_name" ]] && continue
        ((module_num++))

        log_section "PHASE 2: Module $module_num/$module_count — $module_name"

        local module_dir="${WORK_DIR}/module_versions/${module_name}"
        mkdir -p "$module_dir"

        # Assemble the bundle for this module
        "${SCRIPT_DIR}/ddis_assemble.sh" -m "$MANIFEST_PATH" -q "$module_name" || {
            log "ERROR: Failed to assemble bundle for $module_name. Skipping."
            continue
        }

        local bundles_dir
        bundles_dir=$(python3 -c "
import yaml, os
with open('$MANIFEST_PATH') as f: m = yaml.safe_load(f)
print(os.path.join(os.path.dirname(os.path.abspath('$MANIFEST_PATH')), 'bundles'))
")

        local bundle="${bundles_dir}/${module_name}_bundle.md"
        if [[ ! -f "$bundle" ]]; then
            log "ERROR: Bundle not found: $bundle. Skipping."
            continue
        fi

        cp "$bundle" "${module_dir}/bundle_v0.md"
        local seed_lines
        seed_lines=$(line_count "${module_dir}/bundle_v0.md")
        log "PHASE 2 [$module_name]: Bundle seed: $seed_lines lines"

        # Check if this module was flagged by cascade
        if echo "$cascade_modules" | grep -qw "$module_name"; then
            log "PHASE 2 [$module_name]: Flagged by cascade — extra attention to changed invariants"
        fi

        # Run RALPH cycle on the bundle
        local best_version=0
        local stop_reason="max_iterations"

        for ((j = 1; j <= MAX_ITERATIONS; j++)); do
            log "── $module_name iteration $j / $MAX_ITERATIONS ──"

            local prev="${module_dir}/bundle_v$((j - 1)).md"
            local curr="${module_dir}/bundle_v${j}.md"
            local judgment="${JUDGMENTS_DIR}/module_${module_name}_judgment_v${j}.json"
            local prev_judgment="${JUDGMENTS_DIR}/module_${module_name}_judgment_v$((j - 1)).json"

            local prev_judg_arg=""
            [[ -f "$prev_judgment" ]] && prev_judg_arg="$prev_judgment"

            if ! run_improve "$j" "$prev" "$curr" "$prev_judg_arg"; then
                log "ERROR: Module improvement failed for $module_name at iteration $j."
                stop_reason="improve_failed"
                break
            fi

            run_judge "$j" "$prev" "$curr" "$judgment"

            local decision
            decision=$(check_stop "$judgment" "$j")
            best_version=$j

            case "$decision" in
                regressed)
                    stop_reason="regressed"
                    best_version=$((j - 1))
                    break ;;
                excellent|converged|plateau)
                    stop_reason="$decision"
                    break ;;
                continue)
                    ;;
            esac
        done

        # Extract improved module from the best bundle version
        local best_bundle="${module_dir}/bundle_v${best_version}.md"
        if [[ $best_version -gt 0 && -f "$best_bundle" ]]; then
            extract_module_from_bundle "$best_bundle" "$module_name"
        fi

        # Cascade detection: check if this module's improvement affects others
        run_cascade_detection "$module_name" cascade_modules

        log "PHASE 2 [$module_name]: Complete (stop reason: $stop_reason)"

    done <<< "$module_order"

    # Final validation
    log "PHASE 2: Running final validation..."
    "${SCRIPT_DIR}/ddis_validate.sh" -m "$MANIFEST_PATH" || true
}

# Extract the module portion from an assembled bundle (last section after all constitution tiers)
extract_module_from_bundle() {
    local bundle="$1"
    local module_name="$2"

    python3 - "$bundle" "$MANIFEST_PATH" "$module_name" <<'PYEOF'
import sys, yaml, os
bundle_path = sys.argv[1]
manifest_path = sys.argv[2]
module_name = sys.argv[3]
manifest_dir = os.path.dirname(os.path.abspath(manifest_path))
with open(manifest_path) as f:
    m = yaml.safe_load(f)
module_cfg = m["modules"][module_name]
module_file = os.path.join(manifest_dir, module_cfg["file"])
with open(bundle_path) as f:
    content = f.read()
# The module content is the last major section of the bundle.
# Strategy: read the original module to find its first heading, then
# extract everything from that heading onward in the bundle.
with open(module_file) as f:
    original = f.read()
first_line = original.strip().split('\n')[0] if original.strip() else ""
if first_line and first_line in content:
    idx = content.index(first_line)
    module_content = content[idx:]
    # Remove any trailing assembly comments
    if '<!-- ASSEMBLED' in module_content:
        module_content = module_content[:module_content.index('<!-- ASSEMBLED')]
    with open(module_file, 'w') as f:
        f.write(module_content.strip() + '\n')
    print(f"  Extracted improved module → {module_file}")
else:
    print(f"  WARNING: Could not locate module content in bundle for {module_name}", file=sys.stderr)
    print(f"  Module file unchanged: {module_file}")
PYEOF
}

# Check if module improvement triggered changes that affect downstream modules
run_cascade_detection() {
    local module_name="$1"
    local -n _cascade_ref=$2  # nameref to accumulate cascade targets

    # Use ddis_validate.sh cascade mode on each invariant this module maintains
    local maintains
    maintains=$(python3 -c "
import yaml
with open('$MANIFEST_PATH') as f: m = yaml.safe_load(f)
for inv in (m['modules'].get('$module_name', {}).get('maintains') or []):
    print(inv)
" 2>/dev/null) || return 0

    while IFS= read -r inv_id; do
        [[ -z "$inv_id" ]] && continue
        local cascade_result
        cascade_result=$("${SCRIPT_DIR}/ddis_validate.sh" -m "$MANIFEST_PATH" --check-cascade "$inv_id" --json 2>/dev/null) || continue

        local should_modules
        should_modules=$(echo "$cascade_result" | python3 -c "
import sys, json
d = json.load(sys.stdin)
for m in d.get('should_revalidate', []):
    print(m)
" 2>/dev/null) || continue

        while IFS= read -r downstream; do
            [[ -z "$downstream" ]] && continue
            if [[ "$downstream" != "$module_name" ]]; then
                if ! echo "$_cascade_ref" | grep -qw "$downstream"; then
                    _cascade_ref="${_cascade_ref:+$_cascade_ref }$downstream"
                    log "CASCADE: $module_name → $downstream (via $inv_id)"
                fi
            fi
        done <<< "$should_modules"
    done <<< "$maintains"
}

# Orchestrate modular RALPH loop
run_modular() {
    modular_preflight

    log_section "DDIS RALPH Loop — MODULAR MODE"
    log "Manifest: $MANIFEST_PATH"
    log "Phase:    $MODULAR_PHASE"

    mkdir -p "$VERSIONS_DIR" "$JUDGMENTS_DIR" "$LOGS_DIR"
    check_beads
    beads_init

    case "$MODULAR_PHASE" in
        1)    run_phase_1 ;;
        2)    run_phase_2 ;;
        both) run_phase_1; run_phase_2 ;;
        *)    echo "ERROR: Invalid phase '$MODULAR_PHASE'. Use 1, 2, or both." >&2; exit 2 ;;
    esac

    beads_finalize

    log_section "MODULAR RALPH COMPLETE"
    log "Run 'ddis_validate.sh -m $MANIFEST_PATH -v' to verify final state."
}

# ─── Main Loop ────────────────────────────────────────────────────────────────

main() {
    check_prereqs

    # ── Phase 0: Auto-Modularization Assessment ──
    # In monolith mode, assess whether the spec should be decomposed.
    # If yes, decompose and seamlessly transition to modular mode.
    if [[ "$AUTO_MODULARIZE" == "true" && "$MODULAR" == "false" ]]; then
        mkdir -p "$WORK_DIR" "$LOGS_DIR"
        log_section "PHASE 0: Structural Assessment"

        local assessment_json="${WORK_DIR}/structural_assessment.json"
        if run_structural_assessment "$SEED_SPEC" "$assessment_json"; then
            local should_modularize
            should_modularize=$(jq -r '.should_modularize' "$assessment_json" 2>/dev/null)

            if [[ "$should_modularize" == "true" ]]; then
                log_section "PHASE 0: Auto-Modularization — Decomposing Monolith"

                local decomp_dir="${SCRIPT_DIR}/ddis-modular"
                if run_decomposition "$SEED_SPEC" "$assessment_json" "$decomp_dir"; then
                    if [[ -f "${decomp_dir}/manifest.yaml" ]]; then
                        log "AUTO-MODULARIZE: Transitioning to modular mode"
                        MODULAR=true
                        MANIFEST_PATH="${decomp_dir}/manifest.yaml"
                    fi
                else
                    log "WARNING: Decomposition failed. Falling back to monolith mode."
                fi
            else
                log "ASSESSMENT: Monolith mode appropriate. Proceeding with standard RALPH cycle."
            fi
        else
            log "WARNING: Structural assessment failed. Proceeding in monolith mode."
        fi
    fi

    # Dispatch to modular mode if --modular, --manifest, or auto-detected above
    if $MODULAR; then
        run_modular
        return $?
    fi

    log_section "DDIS RALPH Loop — Recursive Self-Improvement"

    log "Configuration:"
    log "  Max iterations:          $MAX_ITERATIONS"
    log "  Min improvements to continue: $MIN_SUBSTANTIVE_IMPROVEMENTS"
    log "  Min quality delta:       $MIN_QUALITY_DELTA"
    log "  Improver model:          $IMPROVER_MODEL"
    log "  Judge model:             $JUDGE_MODEL"
    log "  Polish on exit:          $POLISH_ON_EXIT"
    log ""

    # Initialize workspace
    mkdir -p "$VERSIONS_DIR" "$JUDGMENTS_DIR" "$LOGS_DIR"
    cp "$SEED_SPEC" "$VERSIONS_DIR/ddis_v0.md"

    # Initialize beads gap tracking (if available)
    check_beads
    beads_init

    # Check for persistent gaps from a previous RALPH run
    local persistent_gaps
    persistent_gaps=$(beads_get_persistent_gaps)
    if [[ -n "$persistent_gaps" ]]; then
        local gap_count
        gap_count=$(echo "$persistent_gaps" | wc -l | tr -d ' ')
        log "BEADS: Found $gap_count persistent gaps from previous run — will feed into iteration 1"
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
        if [[ -f "$prev_judgment" ]]; then
            prev_judg_arg="$prev_judgment"
        fi

        if ! run_improve "$i" "$prev_spec" "$curr_spec" "$prev_judg_arg"; then
            log "ERROR: Improvement step failed at iteration $i. Keeping v$((i - 1))."
            stop_reason="improve_failed"
            break
        fi

        # ── Step 2: Judge ──
        run_judge "$i" "$prev_spec" "$curr_spec" "$judgment"

        # ── Step 2b: Sync gaps to beads (if available) ──
        local prev_judg_for_beads=""
        if [[ -f "${JUDGMENTS_DIR}/judgment_v$((i - 1)).json" ]]; then
            prev_judg_for_beads="${JUDGMENTS_DIR}/judgment_v$((i - 1)).json"
        fi
        beads_sync_gaps "$i" "$judgment" "$prev_judg_for_beads"

        # ── Step 3: Check stopping condition ──
        local decision
        decision=$(check_stop "$judgment" "$i")

        case "$decision" in
            regressed)
                stop_reason="regressed"
                # Don't update best_version — keep the previous one
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

    # If we exhausted iterations without stopping
    if [[ $best_version -eq 0 ]]; then
        best_version=$((MAX_ITERATIONS > 0 ? 1 : 0))
    fi

    # ── Optional Polish Pass ──
    if [[ "$POLISH_ON_EXIT" == "true" && $best_version -gt 0 ]]; then
        log_section "POLISH PASS"
        run_polish "${VERSIONS_DIR}/ddis_v${best_version}.md" "${VERSIONS_DIR}/ddis_final.md"
    else
        cp "${VERSIONS_DIR}/ddis_v${best_version}.md" "${VERSIONS_DIR}/ddis_final.md"
    fi

    # ── Finalize beads gap tracking ──
    beads_finalize

    # ── Summary ──
    print_summary "$best_version" "$stop_reason"

    # Copy final output to a convenient location
    cp "${VERSIONS_DIR}/ddis_final.md" "${SCRIPT_DIR}/ddis_final.md"
    log ""
    log "✓ Final spec written to: ${SCRIPT_DIR}/ddis_final.md"
}

main "$@"

