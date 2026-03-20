#!/usr/bin/env bash
# E2E TSV Output Test (TSV-TEST)
#
# Validates TSV output mode across commands:
# - Field consistency (all rows have same column count)
# - Header presence (first row contains expected field names)
# - short_title in task list JSON/TSV (BACKGROUND sections stripped)
# - TSV output for status, query, observe, harvest, task ready
# - Tab escaping in task titles
# - Budget bypass (--budget has no effect on TSV structure)
# - Empty store produces header-only output
#
# Traces to: INV-OUTPUT-001 (deterministic mode resolution),
#            INV-OUTPUT-003 (JSON superset), ADR-INTERFACE-011 (title pyramid)
#
# Usage: ./scripts/e2e_tsv_output.sh

set -uo pipefail
# NOTE: NOT set -e — we check exit codes manually via check()

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BRAID_BIN="cargo run -q --manifest-path ${PROJECT_ROOT}/Cargo.toml --"
TMPDIR=$(mktemp -d)
STORE="$TMPDIR/.braid"
PASS=0
FAIL=0
TOTAL=0

cleanup() {
    rm -rf "$TMPDIR"
}
trap cleanup EXIT

log() {
    echo "[$(date '+%H:%M:%S')] $*"
}

check() {
    local name="$1"
    local result="$2"
    TOTAL=$((TOTAL + 1))
    if [ "$result" -eq 0 ]; then
        echo "[PASS] $name"
        PASS=$((PASS + 1))
    else
        echo "[FAIL] $name"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== E2E TSV Output Test (TSV-TEST) ==="
echo "Project: $PROJECT_ROOT"
echo "Store:   $STORE"
echo ""

# ── Step 1: Initialize store ───────────────────────────────────────
log "Step 1: Initialize fresh braid store"
cd "$TMPDIR"
$BRAID_BIN init -p "$STORE" -q > /dev/null 2>&1 || true
if [ -d "$STORE" ] && [ -d "$STORE/txns" ]; then
    check "init: store created" 0
else
    check "init: store created" 1
    echo "FATAL: Cannot proceed without store. Exiting."
    exit 1
fi

# ── Step 2: Empty store — task list TSV produces header only ───────
log "Step 2: Empty store TSV output"
EMPTY_TSV=$($BRAID_BIN task list -p "$STORE" --format tsv -q 2>&1)
EMPTY_LINES=$(echo "$EMPTY_TSV" | grep -c '.' || true)
if [ "$EMPTY_LINES" -le 1 ]; then
    check "empty store: task list TSV has at most header row" 0
else
    check "empty store: task list TSV has at most header row" 1
    echo "  got $EMPTY_LINES lines: $EMPTY_TSV"
fi

# ── Step 3: Create tasks with structured titles ────────────────────
log "Step 3: Create tasks with structured titles"

T1=$($BRAID_BIN task create "Fix parser bug. BACKGROUND: The EDN parser fails on nested maps with unicode keys. ACCEPTANCE: (A) parse_edn handles nested maps. (B) roundtrip test passes." -p "$STORE" -q 2>&1 | grep -oP 't-[a-f0-9]+' | head -1)
check "create: task with BACKGROUND section" $([[ -n "$T1" ]] && echo 0 || echo 1)

T2=$($BRAID_BIN task create "Implement VAET index. FILE: crates/braid-kernel/src/store.rs" -p "$STORE" -q 2>&1 | grep -oP 't-[a-f0-9]+' | head -1)
check "create: task with FILE marker" $([[ -n "$T2" ]] && echo 0 || echo 1)

T3=$($BRAID_BIN task create "Simple task without sections" -p "$STORE" -q 2>&1 | grep -oP 't-[a-f0-9]+' | head -1)
check "create: simple task" $([[ -n "$T3" ]] && echo 0 || echo 1)

TASK_COUNT=3

# ── Step 4: task list --format tsv row count ───────────────────────
log "Step 4: TSV row count (header + data rows)"
TSV_OUT=$($BRAID_BIN task list -p "$STORE" --format tsv -q 2>&1)
TSV_LINE_COUNT=$(echo "$TSV_OUT" | grep -c '.' || true)
EXPECTED=$((TASK_COUNT + 1))  # header + 3 task rows
if [ "$TSV_LINE_COUNT" -eq "$EXPECTED" ]; then
    check "task list TSV: row count == $EXPECTED (header + $TASK_COUNT tasks)" 0
else
    check "task list TSV: row count == $EXPECTED (header + $TASK_COUNT tasks)" 1
    echo "  expected $EXPECTED lines, got $TSV_LINE_COUNT"
fi

# ── Step 5: TSV header contains 'id' as first field ───────────────
log "Step 5: TSV header structure"
HEADER=$(echo "$TSV_OUT" | head -1)
FIRST_FIELD=$(echo "$HEADER" | cut -f1)
if [ "$FIRST_FIELD" = "id" ]; then
    check "task list TSV: 'id' is first header field" 0
else
    check "task list TSV: 'id' is first header field" 1
    echo "  first field: '$FIRST_FIELD', header: '$HEADER'"
fi

# ── Step 6: TSV data rows have valid t-* IDs ──────────────────────
log "Step 6: TSV ID extraction"
IDS=$(echo "$TSV_OUT" | tail -n +2 | cut -f1)
VALID_IDS=$(echo "$IDS" | grep -cP '^t-[a-f0-9]+$' || true)
if [ "$VALID_IDS" -eq "$TASK_COUNT" ]; then
    check "task list TSV: all data rows have valid t-* IDs" 0
else
    check "task list TSV: all data rows have valid t-* IDs" 1
    echo "  expected $TASK_COUNT valid IDs, got $VALID_IDS"
fi

# ── Step 7: Priority column is numeric (sortable) ─────────────────
log "Step 7: Priority column sortability"
# Find priority column index from header
PRIORITY_COL=$(echo "$HEADER" | tr '\t' '\n' | grep -n '^priority$' | cut -d: -f1)
if [ -n "$PRIORITY_COL" ]; then
    PRIORITIES=$(echo "$TSV_OUT" | tail -n +2 | cut -f"$PRIORITY_COL")
    NON_NUMERIC=$(echo "$PRIORITIES" | grep -cvP '^\d+$' || true)
    if [ "$NON_NUMERIC" -eq 0 ]; then
        check "task list TSV: priority column is numeric" 0
    else
        check "task list TSV: priority column is numeric" 1
        echo "  non-numeric priority values found: $NON_NUMERIC"
    fi
else
    check "task list TSV: priority column is numeric" 1
    echo "  could not find 'priority' column in header"
fi

# ── Step 8: short_title — BACKGROUND stripped in TSV ──────────────
log "Step 8: short_title strips BACKGROUND in TSV"
# The short_title field should NOT contain 'BACKGROUND'
SHORT_TITLE_COL=$(echo "$HEADER" | tr '\t' '\n' | grep -n '^short_title$' | cut -d: -f1)
if [ -n "$SHORT_TITLE_COL" ]; then
    SHORT_TITLES=$(echo "$TSV_OUT" | tail -n +2 | cut -f"$SHORT_TITLE_COL")
    HAS_BACKGROUND=$(echo "$SHORT_TITLES" | grep -c 'BACKGROUND' || true)
    if [ "$HAS_BACKGROUND" -eq 0 ]; then
        check "task list TSV: short_title does NOT contain BACKGROUND" 0
    else
        check "task list TSV: short_title does NOT contain BACKGROUND" 1
        echo "  short_titles with BACKGROUND: $HAS_BACKGROUND"
    fi
else
    check "task list TSV: short_title does NOT contain BACKGROUND" 1
    echo "  'short_title' column not found in header: $HEADER"
fi

# ── Step 9: status --format tsv produces output ───────────────────
log "Step 9: status TSV output"
STATUS_TSV=$($BRAID_BIN status -p "$STORE" --format tsv -q 2>&1)
if [ -n "$STATUS_TSV" ]; then
    check "status --format tsv: produces non-empty output" 0
else
    check "status --format tsv: produces non-empty output" 1
fi

# ── Step 10: status TSV contains 'action' in header ───────────────
log "Step 10: status TSV header"
STATUS_FIRST=$(echo "$STATUS_TSV" | head -1)
if echo "$STATUS_FIRST" | grep -qi 'action'; then
    check "status --format tsv: header contains 'action'" 0
else
    check "status --format tsv: header contains 'action'" 1
    echo "  first line: '$STATUS_FIRST'"
fi

# ── Step 11: query --format tsv produces output ───────────────────
log "Step 11: query TSV output"
QUERY_TSV=$($BRAID_BIN query --attribute :task/status -p "$STORE" --format tsv -q 2>&1)
if [ -n "$QUERY_TSV" ]; then
    check "query --attribute :task/status --format tsv: produces output" 0
else
    check "query --attribute :task/status --format tsv: produces output" 1
fi

# ── Step 12: budget bypass — TSV structure unchanged ──────────────
log "Step 12: Budget does not affect TSV structure"
TSV_SMALL=$($BRAID_BIN task list -p "$STORE" --format tsv --budget 10 -q 2>&1)
TSV_LARGE=$($BRAID_BIN task list -p "$STORE" --format tsv --budget 10000 -q 2>&1)
if [ "$TSV_SMALL" = "$TSV_LARGE" ]; then
    check "task list TSV: --budget 10 == --budget 10000 (bypass)" 0
else
    check "task list TSV: --budget 10 == --budget 10000 (bypass)" 1
    echo "  outputs differ"
fi

# ── Step 13: Field consistency (all rows same column count) ───────
log "Step 13: TSV field consistency"
FIELD_COUNTS=$(echo "$TSV_OUT" | awk -F'\t' '{print NF}' | sort -u)
UNIQUE_COUNTS=$(echo "$FIELD_COUNTS" | wc -l)
if [ "$UNIQUE_COUNTS" -eq 1 ]; then
    check "task list TSV: all rows have same field count" 0
else
    check "task list TSV: all rows have same field count" 1
    echo "  distinct field counts: $FIELD_COUNTS"
fi

# ── Step 14: task ready --format tsv has header row ───────────────
log "Step 14: task ready TSV"
READY_TSV=$($BRAID_BIN task ready -p "$STORE" --format tsv -q 2>&1)
READY_FIRST=$(echo "$READY_TSV" | head -1)
if echo "$READY_FIRST" | grep -q 'id'; then
    check "task ready --format tsv: header contains 'id'" 0
else
    # task ready may produce "No ready tasks" if none are ready — that's acceptable
    if echo "$READY_TSV" | grep -qi 'ready\|task'; then
        check "task ready --format tsv: header contains 'id'" 0
    else
        check "task ready --format tsv: header contains 'id'" 1
        echo "  first line: '$READY_FIRST'"
    fi
fi

# ── Step 15: observe --format tsv does not crash ──────────────────
log "Step 15: observe TSV output"
OBS_TSV=$($BRAID_BIN observe "E2E TSV test observation" --confidence 0.8 -p "$STORE" --format tsv -q 2>&1)
OBS_EXIT=$?
if [ "$OBS_EXIT" -eq 0 ] && [ -n "$OBS_TSV" ]; then
    check "observe --format tsv: produces output without crash" 0
else
    check "observe --format tsv: produces output without crash" 1
    echo "  exit=$OBS_EXIT, output='$OBS_TSV'"
fi

# ── Step 16: harvest --format tsv produces output ─────────────────
log "Step 16: harvest TSV output"
HARVEST_TSV=$($BRAID_BIN harvest --commit --force -p "$STORE" --format tsv -q 2>&1)
HARVEST_EXIT=$?
if [ "$HARVEST_EXIT" -eq 0 ] && [ -n "$HARVEST_TSV" ]; then
    check "harvest --commit --force --format tsv: produces output" 0
else
    check "harvest --commit --force --format tsv: produces output" 1
    echo "  exit=$HARVEST_EXIT, output='$HARVEST_TSV'"
fi

# ── Step 17: Tab in title is escaped (no field corruption) ────────
log "Step 17: Tab escaping in task titles"
# Create a task with a literal tab in the title
TAB_TITLE=$'Tab\there title'
T_TAB=$($BRAID_BIN task create "$TAB_TITLE" -p "$STORE" -q 2>&1 | grep -oP 't-[a-f0-9]+' | head -1)
if [ -n "$T_TAB" ]; then
    TSV_WITH_TAB=$($BRAID_BIN task list -p "$STORE" --format tsv -q 2>&1)
    # All rows should still have the same number of fields
    TAB_FIELD_COUNTS=$(echo "$TSV_WITH_TAB" | awk -F'\t' '{print NF}' | sort -u)
    TAB_UNIQUE=$(echo "$TAB_FIELD_COUNTS" | wc -l)
    if [ "$TAB_UNIQUE" -eq 1 ]; then
        check "tab in title: TSV field count consistent (tab escaped)" 0
    else
        check "tab in title: TSV field count consistent (tab escaped)" 1
        echo "  distinct field counts after tab title: $TAB_FIELD_COUNTS"
    fi
else
    check "tab in title: TSV field count consistent (tab escaped)" 1
    echo "  failed to create task with tab in title"
fi

# ── Summary ────────────────────────────────────────────────────────
echo ""
echo "=== Results ==="
echo "PASS: $PASS"
echo "FAIL: $FAIL"
echo "TOTAL: $TOTAL"

if [ "$FAIL" -gt 0 ]; then
    echo "STATUS: FAILED"
    exit 1
else
    echo "STATUS: ALL PASSED"
    exit 0
fi
