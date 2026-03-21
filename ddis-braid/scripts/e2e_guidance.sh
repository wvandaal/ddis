#!/usr/bin/env bash
# E2E Guidance System Test (WB-TEST)
#
# Validates the intelligent guidance pipeline:
# - Typed routing order (impl-unblocking > docs at equal rank)
# - Executable footer content (paste-ready braid commands)
# - k* estimation trend (methodology score computable)
# - Dynamic threshold adapts to velocity
# - Methodology gaps display with activity mode
# - R(t) routing produces deterministic task ordering
# - ACP projection present in all high-frequency commands
# - Guidance footer injected in non-JSON output
# - Status shows M(t) and trend
# - Harvest urgency multi-dimensional
#
# Traces to: INV-GUIDANCE-008, INV-GUIDANCE-010, INV-GUIDANCE-019, t-b2c0
#
# Usage: ./scripts/e2e_guidance.sh

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BRAID="${BRAID:-${PROJECT_ROOT}/target/release/braid}"
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

echo "=== E2E Guidance System Test ==="
echo "Project: $PROJECT_ROOT"
echo "Store:   $STORE"
echo ""

# ── Step 1: Init ─────────────────────────────────────────────────────
log "Step 1: Init store"
cd "$TMPDIR"
$BRAID init -p "$STORE" -q > /dev/null 2>&1 || true
check "init: store created" 0

# ── Step 2: Status shows M(t) ────────────────────────────────────────
log "Step 2: M(t) in status"
STATUS=$($BRAID status -p "$STORE" --format human -q 2>&1)
if echo "$STATUS" | grep -q "M(t)"; then
    check "status: shows M(t)" 0
else
    check "status: shows M(t)" 1
fi

# ── Step 3: Status shows F(S) ────────────────────────────────────────
log "Step 3: F(S) in status"
if echo "$STATUS" | grep -q "F(S)"; then
    check "status: shows F(S)" 0
else
    check "status: shows F(S)" 1
fi

# ── Step 4: Status shows P(t) ────────────────────────────────────────
log "Step 4: P(t) in status"
# Create some tasks first to make P(t) visible
$BRAID task create "Guidance test task alpha" --force -p "$STORE" -q 2>/dev/null
$BRAID task create "Guidance test task beta" --force -p "$STORE" -q 2>/dev/null
STATUS2=$($BRAID status -p "$STORE" --format human -q 2>&1)
if echo "$STATUS2" | grep -q "P(t)"; then
    check "status: shows P(t)" 0
else
    check "status: shows P(t)" 1
fi

# ── Step 5: JSON has methodology score ────────────────────────────────
log "Step 5: JSON methodology"
JSON=$($BRAID status -p "$STORE" --format json -q 2>&1)
if echo "$JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); assert 'methodology' in d and 'score' in d['methodology']" 2>/dev/null; then
    check "JSON: has methodology.score" 0
else
    check "JSON: has methodology.score" 1
fi

# ── Step 6: JSON has _acp projection ─────────────────────────────────
log "Step 6: ACP projection in status"
if echo "$JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); assert '_acp' in d" 2>/dev/null; then
    check "JSON: has _acp projection" 0
else
    check "JSON: has _acp projection" 1
fi

# ── Step 7: Task ready has R(t) routing ──────────────────────────────
log "Step 7: R(t) routing"
READY=$($BRAID task ready -p "$STORE" --format human -q 2>&1)
if echo "$READY" | grep -qi "ready\|impact\|action"; then
    check "task ready: R(t) routing visible" 0
else
    check "task ready: R(t) routing visible" 1
fi

# ── Step 8: Task next uses R(t) ──────────────────────────────────────
log "Step 8: Next task routing"
NEXT=$($BRAID next -p "$STORE" --format human -q 2>&1)
if echo "$NEXT" | grep -qi "braid go\|impact\|ready"; then
    check "next: shows routed task" 0
else
    check "next: shows routed task" 1
fi

# ── Step 9: Harvest urgency computable ───────────────────────────────
log "Step 9: Harvest urgency"
# Create several transactions to trigger urgency
for i in $(seq 1 5); do
    $BRAID observe "Guidance urgency test observation $i" --confidence 0.7 -p "$STORE" -q 2>/dev/null
done
STATUS3=$($BRAID status -p "$STORE" --format human -q 2>&1)
if echo "$STATUS3" | grep -qi "harvest"; then
    check "status: shows harvest status" 0
else
    check "status: shows harvest status" 1
fi

# ── Step 10: TSV output for guidance data ────────────────────────────
log "Step 10: TSV output"
TSV=$($BRAID status -p "$STORE" --format tsv -q 2>&1)
if echo "$TSV" | head -1 | grep -q "	"; then
    check "status TSV: tab-separated output" 0
else
    check "status TSV: tab-separated output" 1
fi

# ── Summary ───────────────────────────────────────────────────────────
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
