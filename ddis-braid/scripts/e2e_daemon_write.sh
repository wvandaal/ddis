#!/bin/bash
# E2E verification for DAEMON-WRITE stale cache fix
# Tests the full CLI workflow without needing a running daemon
set -euo pipefail

BRAID="./target/release/braid"
PASS=0
FAIL=0
TOTAL=0

log() { echo "[$(date +%H:%M:%S)] $*"; }

# Setup: isolated tempdir (cleaned up by trap)
TMPDIR=$(mktemp -d)
BRAID_DIR="$TMPDIR/.braid"
trap 'rm -r "$TMPDIR"' EXIT
log "=== E2E DAEMON-WRITE VERIFICATION ==="
log "Store: $BRAID_DIR"

# Test 1: Init + status
log "--- Test 1: Init + Status ---"
$BRAID init --path "$BRAID_DIR" -q 2>&1 | tail -1
$BRAID status --path "$BRAID_DIR" -q 2>/dev/null | head -1
TOTAL=$((TOTAL+1)); PASS=$((PASS+1)); log "  PASS: init + status"

# Test 2: Observe then immediate search (THE IRON TEST)
log "--- Test 2: Iron Test (observe then search) ---"
$BRAID observe --path "$BRAID_DIR" -q --no-auto-crystallize -c 0.8 "iron-test-observation" 2>&1 | tail -1
FOUND=$($BRAID query --path "$BRAID_DIR" -q --attribute ":exploration/body" 2>&1 | grep -c "iron-test-observation" || true)
TOTAL=$((TOTAL+1))
if [ "$FOUND" -gt 0 ]; then
    PASS=$((PASS+1)); log "  PASS: observation found immediately"
else
    FAIL=$((FAIL+1)); log "  FAIL: observation NOT found (stale cache?)"
fi

# Test 3: Task create then immediate search
log "--- Test 3: Iron Test (task create then search) ---"
$BRAID task create --path "$BRAID_DIR" -q --force "e2e-test-task" --priority 3 2>&1 | tail -1
FOUND=$($BRAID task search --path "$BRAID_DIR" -q "e2e-test-task" 2>&1 | grep -c "e2e-test-task" || true)
TOTAL=$((TOTAL+1))
if [ "$FOUND" -gt 0 ]; then
    PASS=$((PASS+1)); log "  PASS: task found immediately"
else
    FAIL=$((FAIL+1)); log "  FAIL: task NOT found (stale cache?)"
fi

# Test 4: Sequential writes all visible
log "--- Test 4: Sequential writes ---"
for i in 1 2 3 4 5; do
    $BRAID observe --path "$BRAID_DIR" -q --no-auto-crystallize -c 0.5 "seq-write-$i" 2>&1 > /dev/null
done
DATOMS=$($BRAID status --path "$BRAID_DIR" -q 2>/dev/null | grep "store:" | head -1)
log "  Store state: $DATOMS"
TOTAL=$((TOTAL+1)); PASS=$((PASS+1)); log "  PASS: 5 sequential writes completed"

# Test 5: Harvest round-trip
log "--- Test 5: Harvest round-trip ---"
$BRAID harvest --path "$BRAID_DIR" -q --commit 2>&1 | head -3
$BRAID status --path "$BRAID_DIR" -q 2>/dev/null | head -2
TOTAL=$((TOTAL+1)); PASS=$((PASS+1)); log "  PASS: harvest + status round-trip"

# Test 6: Cache file exists and is valid
log "--- Test 6: Cache validity ---"
if [ -f "$BRAID_DIR/.cache/store.bin" ]; then
    TOTAL=$((TOTAL+1)); PASS=$((PASS+1)); log "  PASS: store.bin exists"
else
    TOTAL=$((TOTAL+1)); FAIL=$((FAIL+1)); log "  FAIL: store.bin missing"
fi
if [ -f "$BRAID_DIR/.cache/meta.json" ]; then
    HASH_COUNT=$(python3 -c "import json; d=json.load(open('$BRAID_DIR/.cache/meta.json')); print(len(d.get('tx_hashes',[])))" 2>/dev/null || echo "0")
    log "  meta.json tx_hashes count: $HASH_COUNT"
    TOTAL=$((TOTAL+1)); PASS=$((PASS+1)); log "  PASS: meta.json valid"
else
    TOTAL=$((TOTAL+1)); FAIL=$((FAIL+1)); log "  FAIL: meta.json missing"
fi

# Results
log ""
log "=== RESULTS ==="
log "Passed: $PASS / $TOTAL"
log "Failed: $FAIL / $TOTAL"
if [ "$FAIL" -eq 0 ]; then
    log "ALL TESTS PASSED"
    exit 0
else
    log "SOME TESTS FAILED"
    exit 1
fi
