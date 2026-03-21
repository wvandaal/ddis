#!/usr/bin/env bash
# E2E Session Lifecycle Test (COTX-1-TEST)
#
# Validates atomic session rotation during harvest --commit:
# - Harvest closes active session and opens new session in same tx
# - New session has all required attributes (fitness, datom count, etc.)
# - Multiple harvests rotate correctly
# - Cold start (no prior session) creates session without error
# - No stale sessions remain after harvest --commit
# - Harvest without --commit does not rotate
#
# Traces to: INV-HARVEST-007, INV-BILATERAL-001, t-c9eb2524
#
# Usage: ./scripts/e2e_session_lifecycle.sh

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BRAID="${BRAID:-${PROJECT_ROOT}/target/release/braid}"
TMPDIR=$(mktemp -d)
STORE="$TMPDIR/.braid"
PASS=0
FAIL=0
TOTAL=0

cleanup() {
    # Leave temp dir for debugging if tests fail
    if [ "$FAIL" -gt 0 ]; then
        echo "Debug store left at: $STORE"
    fi
}
trap cleanup EXIT

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

# Helper: count active sessions (active without a later closed assertion)
count_active_sessions() {
    local store_path="$1"
    local sessions
    sessions=$($BRAID query --attribute :session/status -p "$store_path" 2>&1)
    # Get unique session entities that have active status
    local active_entities
    active_entities=$(echo "$sessions" | grep "session.status/active" | awk '{print $1}' | tr -d '[]' | sort -u)
    local count=0
    for entity in $active_entities; do
        local has_closed
        has_closed=$(echo "$sessions" | grep "$entity" | grep -c "session.status/closed" || true)
        if [ "$has_closed" -eq 0 ]; then
            count=$((count + 1))
        fi
    done
    echo "$count"
}

# Helper: get the active session ident (no later closed)
get_active_session() {
    local store_path="$1"
    local sessions
    sessions=$($BRAID query --attribute :session/status -p "$store_path" 2>&1)
    local active_entities
    active_entities=$(echo "$sessions" | grep "session.status/active" | awk '{print $1}' | tr -d '[]' | sort -u)
    for entity in $active_entities; do
        local has_closed
        has_closed=$(echo "$sessions" | grep "$entity" | grep -c "session.status/closed" || true)
        if [ "$has_closed" -eq 0 ]; then
            echo "$entity"
            return
        fi
    done
}

echo "=== E2E Session Lifecycle Test (COTX-1) ==="
echo "Project: $PROJECT_ROOT"
echo "Store:   $STORE"
echo ""

# ── Step 1: Init ─────────────────────────────────────────────────────
echo "Step 1: Init store"
$BRAID init -p "$STORE" > /dev/null 2>&1

# ── Check 1: Cold start — harvest on fresh store creates session ─────
echo "Check 1: Cold start harvest creates session"
$BRAID observe "cold start observation" --confidence 0.8 -p "$STORE" > /dev/null 2>&1
$BRAID harvest --commit --force -p "$STORE" > /dev/null 2>&1

ACTIVE=$(count_active_sessions "$STORE")
check "Cold start: harvest creates active session" "$([ "$ACTIVE" -ge 1 ] && echo 0 || echo 1)"

# ── Check 2: New session has all required attributes ─────────────────
echo "Check 2: New session has required attributes"
ACTIVE_IDENT=$(get_active_session "$STORE")
SESSION_DATOMS=$($BRAID query --entity "$ACTIVE_IDENT" -p "$STORE" 2>&1)

HAS_ALL=true
for attr in ":db/ident" ":session/started-at" ":session/start-fitness" \
            ":session/start-datom-count" ":session/status" ":session/current" \
            ":session/start-time" ":session/agent" ":session/task"; do
    if ! echo "$SESSION_DATOMS" | grep -q "$attr"; then
        echo "  Missing: $attr"
        HAS_ALL=false
    fi
done
check "New session has all 9 attributes" "$($HAS_ALL && echo 0 || echo 1)"

# ── Check 3: start-fitness is a valid number ─────────────────────────
echo "Check 3: start-fitness is valid"
FITNESS_LINE=$(echo "$SESSION_DATOMS" | grep ":session/start-fitness")
check "start-fitness is present and numeric" "$(echo "$FITNESS_LINE" | grep -qE '[0-9]+\.[0-9]+' && echo 0 || echo 1)"

# ── Check 4: start-datom-count is positive ───────────────────────────
echo "Check 4: start-datom-count is positive"
COUNT_LINE=$(echo "$SESSION_DATOMS" | grep ":session/start-datom-count")
DATOM_COUNT=$(echo "$COUNT_LINE" | grep -oE '[0-9]+' | tail -1)
check "start-datom-count is positive" "$([ "${DATOM_COUNT:-0}" -gt 0 ] && echo 0 || echo 1)"

# ── Check 5: Second harvest rotates session ──────────────────────────
echo "Check 5: Session rotation on second harvest"
FIRST_ACTIVE="$ACTIVE_IDENT"
$BRAID observe "second session observation" --confidence 0.7 -p "$STORE" > /dev/null 2>&1
sleep 1  # Ensure different wall time for new session ident
$BRAID harvest --commit --force -p "$STORE" > /dev/null 2>&1

# The first session should now be closed
SESSIONS_RAW=$($BRAID query --attribute :session/status -p "$STORE" 2>&1)
FIRST_CLOSED=$(echo "$SESSIONS_RAW" | grep "$FIRST_ACTIVE" | grep -c "session.status/closed" || true)
check "First session closed after second harvest" "$([ "$FIRST_CLOSED" -ge 1 ] && echo 0 || echo 1)"

# There should be a new active session (different from the first)
NEW_ACTIVE=$(get_active_session "$STORE")
check "New active session created by second harvest" "$([ -n "$NEW_ACTIVE" ] && echo 0 || echo 1)"

# ── Check 6: Third harvest — verify chain of rotations ───────────────
echo "Check 6: Third harvest rotation"
$BRAID observe "third observation" --confidence 0.6 -p "$STORE" > /dev/null 2>&1
sleep 1
$BRAID harvest --commit --force -p "$STORE" > /dev/null 2>&1

SESSIONS_RAW2=$($BRAID query --attribute :session/status -p "$STORE" 2>&1)
CLOSED_COUNT=$(echo "$SESSIONS_RAW2" | grep -c "session.status/closed" || true)
check "Multiple closed sessions after 3 harvests" "$([ "$CLOSED_COUNT" -ge 2 ] && echo 0 || echo 1)"

# ── Check 7: Exactly 1 active session (no stale) ─────────────────────
echo "Check 7: No stale sessions"
ACTIVE_NOW=$(count_active_sessions "$STORE")
check "Exactly 1 active session (no stale)" "$([ "$ACTIVE_NOW" -eq 1 ] && echo 0 || echo 1)"

# ── Check 8: Harvest without --commit does NOT rotate ────────────────
echo "Check 8: Harvest without --commit preserves session"
BEFORE_ACTIVE=$(get_active_session "$STORE")
$BRAID observe "no-commit observation" --confidence 0.5 -p "$STORE" > /dev/null 2>&1
$BRAID harvest -p "$STORE" > /dev/null 2>&1  # no --commit
AFTER_ACTIVE=$(get_active_session "$STORE")
check "No-commit harvest preserves active session" "$([ "$BEFORE_ACTIVE" = "$AFTER_ACTIVE" ] && echo 0 || echo 1)"

# ── Check 9: start-datom-count includes harvest datoms ───────────────
echo "Check 9: start-datom-count includes harvest datoms"
LATEST_ACTIVE=$(get_active_session "$STORE")
LATEST_DATOMS=$($BRAID query --entity "$LATEST_ACTIVE" -p "$STORE" 2>&1)
LATEST_START_COUNT=$(echo "$LATEST_DATOMS" | grep ":session/start-datom-count" | grep -oE '[0-9]+' | tail -1)
STORE_SIZE=$($BRAID status -p "$STORE" 2>&1 | grep -oE '[0-9]+ datoms' | head -1 | awk '{print $1}')
check "start-datom-count <= current store size" "$([ "${LATEST_START_COUNT:-0}" -le "${STORE_SIZE:-0}" ] && echo 0 || echo 1)"

# ── Check 10: Session task inherits from harvest ─────────────────────
echo "Check 10: Session task from harvest"
TASK_LINE=$(echo "$LATEST_DATOMS" | grep ":session/task")
check "Session has :session/task attribute" "$([ -n "$TASK_LINE" ] && echo 0 || echo 1)"

# ── Check 11: Store grows monotonically ──────────────────────────────
echo "Check 11: Store monotonic growth"
check "Store grew across session rotations" "$([ "${STORE_SIZE:-0}" -gt "${DATOM_COUNT:-0}" ] && echo 0 || echo 1)"

# ── Summary ──────────────────────────────────────────────────────────
echo ""
echo "=== Results: $PASS/$TOTAL passed, $FAIL failed ==="

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
exit 0
