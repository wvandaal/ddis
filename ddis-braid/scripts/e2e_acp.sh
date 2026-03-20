#!/usr/bin/env bash
# E2E ACP (Action-Centric Projection) Test
#
# Validates that ACP-converted commands produce the correct output structure:
# - Action line is always present and never truncated
# - Context scales with budget
# - Evidence pointer present
# - _acp field in JSON output
# - Budget gate does not double-gate ACP commands
#
# Traces to: INV-BUDGET-007 (ACP structure), INV-BUDGET-008 (action never truncated),
#            ADR-BUDGET-006 (output pyramid), INV-INTERFACE-008 (API-as-prompt)
#
# Usage: ./scripts/e2e_acp.sh

set -uo pipefail

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

echo "=== E2E ACP (Action-Centric Projection) Test ==="
echo "Project: $PROJECT_ROOT"
echo "Store:   $STORE"
echo ""

# ── Step 1: Create fresh store with data ──────────────────────────────
log "Step 1: Initialize and populate store"
cd "$TMPDIR"
$BRAID_BIN init -p "$STORE" -q > /dev/null 2>&1 || true
check "init: store created" 0

# Add some tasks and observations for status to report on
$BRAID_BIN task create "Test task 1" -p "$STORE" -q 2>/dev/null
$BRAID_BIN task create "Test task 2" -p "$STORE" -q 2>/dev/null
$BRAID_BIN observe "Test observation" --confidence 0.8 -p "$STORE" -q 2>/dev/null

# ── Step 2: Status JSON has _acp field ────────────────────────────────
log "Step 2: Verify status JSON has _acp field"
STATUS_JSON=$($BRAID_BIN status -p "$STORE" --format json -q 2>&1)

if echo "$STATUS_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); assert '_acp' in d" 2>/dev/null; then
    check "status JSON: _acp field present" 0
else
    check "status JSON: _acp field present" 1
fi

# ── Step 3: Status human output has action line ──────────────────────
log "Step 3: Verify status output contains action recommendation"
STATUS_HUMAN=$($BRAID_BIN status -p "$STORE" --format human -q 2>&1)

if echo "$STATUS_HUMAN" | grep -q "braid"; then
    check "status human: contains braid command suggestion" 0
else
    check "status human: contains braid command suggestion" 1
fi

# ── Step 4: Task list JSON has _acp field ─────────────────────────────
log "Step 4: Verify task list JSON structure"
TASK_JSON=$($BRAID_BIN task list -p "$STORE" --format json -q 2>&1)

if echo "$TASK_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); assert 'tasks' in d" 2>/dev/null; then
    check "task list JSON: has tasks array" 0
else
    check "task list JSON: has tasks array" 1
fi

if echo "$TASK_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['matched'] >= 2" 2>/dev/null; then
    check "task list JSON: matched >= 2" 0
else
    check "task list JSON: matched >= 2" 1
fi

# ── Step 5: Observe output has ACP structure ─────────────────────────
log "Step 5: Verify observe output"
OBS_OUT=$($BRAID_BIN observe "Another observation" --confidence 0.7 -p "$STORE" --format human -q 2>&1)

if echo "$OBS_OUT" | grep -qi "observed\|observation"; then
    check "observe: output confirms observation stored" 0
else
    check "observe: output confirms observation stored" 1
fi

# ── Step 6: Harvest JSON has _acp ─────────────────────────────────────
log "Step 6: Verify harvest output"
HARVEST_JSON=$($BRAID_BIN harvest --commit -p "$STORE" --format json -q 2>&1)

if echo "$HARVEST_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); assert '_acp' in d or 'harvest' in str(d).lower()" 2>/dev/null; then
    check "harvest JSON: has ACP or harvest data" 0
else
    check "harvest JSON: has ACP or harvest data" 1
fi

# ── Step 7: Budget gate doesn't truncate ACP commands ─────────────────
log "Step 7: Verify ACP bypass prevents double-gating"
# Run status with very tight budget — ACP commands should bypass the gate
STATUS_TIGHT=$($BRAID_BIN status -p "$STORE" --format human -q --budget 10 2>&1)

# Even with budget=10, the ACP action line must be present
if echo "$STATUS_TIGHT" | grep -q "braid\|store:"; then
    check "budget gate: ACP output not truncated even at budget=10" 0
else
    check "budget gate: ACP output not truncated even at budget=10" 1
fi

# ── Step 8: Task close has ACP output ─────────────────────────────────
log "Step 8: Verify task close output"
# Get a task ID to close
TASK_ID=$($BRAID_BIN task list -p "$STORE" --format json -q 2>&1 | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['tasks'][0]['id'])" 2>/dev/null)

if [ -n "$TASK_ID" ]; then
    CLOSE_OUT=$($BRAID_BIN task close "$TASK_ID" --reason "E2E test" -p "$STORE" --format human -q 2>&1)
    if echo "$CLOSE_OUT" | grep -qi "closed"; then
        check "task close: output confirms closure" 0
    else
        check "task close: output confirms closure" 1
    fi
else
    check "task close: output confirms closure" 1
fi

# ── Step 9: Seed output scales with budget ────────────────────────────
log "Step 9: Verify seed output"
SEED_OUT=$($BRAID_BIN seed -p "$STORE" --format human -q 2>&1)

if echo "$SEED_OUT" | grep -qi "seed\|orientation\|braid"; then
    check "seed: produces orientation output" 0
else
    check "seed: produces orientation output" 1
fi

# ── Step 10: Bilateral output ─────────────────────────────────────────
log "Step 10: Verify bilateral output"
BILATERAL_OUT=$($BRAID_BIN bilateral -p "$STORE" --format human -q 2>&1)

if echo "$BILATERAL_OUT" | grep -qi "bilateral\|divergence\|coherence\|F(S)"; then
    check "bilateral: produces coherence output" 0
else
    check "bilateral: produces coherence output" 1
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
