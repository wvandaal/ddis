#!/usr/bin/env bash
# E2E Store Performance Test (T2-4)
#
# Validates that core braid operations complete within acceptable time bounds
# even after moderate store load (500 datoms across 50 transactions).
#
# Checks:
#   1. braid status   — average of 3 runs < 2.0 seconds
#   2. task close      — < 3.0 seconds
#   3. task ready      — < 2.0 seconds
#   4. harvest --commit — < 5.0 seconds
#
# Traces to: INV-BUDGET-001 (output within token/time budget),
#            INV-INTERFACE-001 (responsiveness under load)
#
# Usage: ./scripts/e2e_store_performance.sh

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

# time_cmd CMD...  — prints elapsed seconds (integer millisecond precision)
# Sets TIME_ELAPSED to the elapsed time as a decimal string.
time_cmd() {
    local start end
    start=$(date +%s%N)
    "$@" > /dev/null 2>&1 || true
    end=$(date +%s%N)
    local diff_ns=$(( end - start ))
    # Convert nanoseconds to seconds with 3 decimal places
    local secs=$(( diff_ns / 1000000000 ))
    local ms=$(( (diff_ns % 1000000000) / 1000000 ))
    TIME_ELAPSED=$(printf '%d.%03d' "$secs" "$ms")
}

# check_time NAME ELAPSED LIMIT
# Passes if ELAPSED < LIMIT (both are decimal strings like "1.234").
check_time() {
    local name="$1"
    local elapsed="$2"
    local limit="$3"
    # Use awk for floating-point comparison
    local ok
    ok=$(awk "BEGIN { print ($elapsed < $limit) ? 1 : 0 }")
    if [ "$ok" -eq 1 ]; then
        check "$name (${elapsed}s < ${limit}s)" 0
    else
        check "$name (${elapsed}s >= ${limit}s EXCEEDED)" 1
    fi
}

echo "=== E2E Store Performance Test (T2-4) ==="
echo "Project: $PROJECT_ROOT"
echo "Store:   $STORE"
echo ""

# ── Step 1: Initialize fresh braid store ─────────────────────────────────
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

# ── Step 2: Transact 500 datoms (50 transactions x 10 datoms each) ──────
log "Step 2: Transact 500 datoms (50 txns x 10 datoms)"
TX_FAIL=0
for tx in $(seq 1 50); do
    DATOM_ARGS=""
    for d in $(seq 1 10); do
        IDX=$(( (tx - 1) * 10 + d ))
        DATOM_ARGS="$DATOM_ARGS -d :test/perf-entity-${IDX} :db/doc Perf-test-datom-${IDX}"
    done
    if ! $BRAID_BIN transact $DATOM_ARGS \
        -r "E2E perf: batch $tx" \
        -p "$STORE" -q 2>/dev/null; then
        TX_FAIL=$((TX_FAIL + 1))
    fi
done

if [ "$TX_FAIL" -eq 0 ]; then
    check "transact: 50 batches of 10 datoms (500 total)" 0
else
    check "transact: $TX_FAIL/$50 batches failed" 1
fi

# Verify datom count grew
STATUS_PRE=$($BRAID_BIN status --format json -p "$STORE" -q 2>/dev/null)
DATOM_COUNT=$(echo "$STATUS_PRE" | python3 -c "import json,sys; print(json.load(sys.stdin).get('datom_count',0))" 2>/dev/null || echo 0)
log "  Store now has $DATOM_COUNT datoms"
if [ "$DATOM_COUNT" -gt 100 ]; then
    check "store: datom count > 100 (got $DATOM_COUNT)" 0
else
    check "store: datom count > 100 (got $DATOM_COUNT)" 1
fi

# ── Step 3: Time braid status — 3 runs, compute average ─────────────────
log "Step 3: Time braid status (3 runs)"
TOTAL_MS=0
for run in 1 2 3; do
    time_cmd $BRAID_BIN status -p "$STORE" -q
    log "  Run $run: ${TIME_ELAPSED}s"
    # Accumulate in milliseconds for averaging
    RUN_MS=$(awk "BEGIN { printf \"%d\", $TIME_ELAPSED * 1000 }")
    TOTAL_MS=$((TOTAL_MS + RUN_MS))
done
AVG_MS=$((TOTAL_MS / 3))
AVG_SEC=$(printf '%d.%03d' $((AVG_MS / 1000)) $((AVG_MS % 1000)))
log "  Average: ${AVG_SEC}s"
check_time "status: average of 3 runs" "$AVG_SEC" "2.0"

# ── Step 4: Create a task, then time task close ──────────────────────────
log "Step 4: Create task, then time task close"
TASK_OUT=$($BRAID_BIN task create "E2E perf test task" \
    --priority 3 --type task \
    -p "$STORE" -q 2>&1)
# Extract task ID from output (format: "created t-XXXXXXXX")
TASK_ID=$(echo "$TASK_OUT" | grep -oE 't-[0-9a-f]{8}' | head -1)

if [ -n "$TASK_ID" ]; then
    check "task create: got ID $TASK_ID" 0
    log "  Timing task close for $TASK_ID"
    time_cmd $BRAID_BIN task close "$TASK_ID" --reason "perf test" -p "$STORE" -q
    log "  task close: ${TIME_ELAPSED}s"
    check_time "task close" "$TIME_ELAPSED" "3.0"
else
    check "task create: failed to get task ID" 1
    check "task close: skipped (no task ID)" 1
fi

# ── Step 5: Time task ready ──────────────────────────────────────────────
log "Step 5: Time task ready"
time_cmd $BRAID_BIN task ready -p "$STORE" -q
log "  task ready: ${TIME_ELAPSED}s"
check_time "task ready" "$TIME_ELAPSED" "2.0"

# ── Step 6: Time harvest --commit ────────────────────────────────────────
log "Step 6: Time harvest --commit"

# Add some observations first so harvest has work to do
$BRAID_BIN observe "Performance test observation 1" -c 0.8 -p "$STORE" -q 2>/dev/null || true
$BRAID_BIN observe "Performance test observation 2" -c 0.7 -p "$STORE" -q 2>/dev/null || true

time_cmd $BRAID_BIN harvest --commit -p "$STORE" -q
log "  harvest --commit: ${TIME_ELAPSED}s"
check_time "harvest --commit" "$TIME_ELAPSED" "5.0"

# ── Timing Summary ───────────────────────────────────────────────────────
echo ""
echo "=== Timing Summary ==="
echo "Store size:       $DATOM_COUNT datoms"
echo "status (avg/3):   ${AVG_SEC}s  (limit: 2.0s)"
if [ -n "${TASK_ID:-}" ]; then
    echo "task close:       measured above  (limit: 3.0s)"
fi
echo "task ready:       measured above  (limit: 2.0s)"
echo "harvest --commit: measured above  (limit: 5.0s)"

# ── Results ──────────────────────────────────────────────────────────────
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
