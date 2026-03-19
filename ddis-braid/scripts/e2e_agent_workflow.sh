#!/usr/bin/env bash
# E2E Agent Workflow Integration Test
#
# Tests the FULL agent workflow: create → go → set → done
# Catches the class of bugs where CLI syntax changes break agent scripts.
#
# Traces to: INV-INTERFACE-008, INV-TASK-006
# Created: Session 025 (after discovering 13 silent task-set failures)

set -uo pipefail
# NOTE: NOT set -e — we check exit codes manually via check()

BRAID="${BRAID:-$(cd "$(dirname "$0")/.." && pwd)/target/release/braid}"
PASS=0
FAIL=0
TMPDIR=$(mktemp -d)
STORE="$TMPDIR/.braid"

cleanup() {
    rm -rf "$TMPDIR"
}
trap cleanup EXIT

check() {
    local name="$1"
    local result="$2"
    if [ "$result" -eq 0 ]; then
        echo "[PASS] $name"
        PASS=$((PASS + 1))
    else
        echo "[FAIL] $name"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== E2E Agent Workflow Test ==="
echo "Binary: $BRAID"
echo "Store:  $STORE"
echo ""

# ── Test 1: braid init ──────────────────────────────────────────────
echo "--- Test 1: braid init ---"
cd "$TMPDIR"
$BRAID init -p "$STORE" -q > /dev/null 2>&1 || true
# Init creates the store directory with txns/ subdirectory
if [ -d "$STORE" ] && [ -d "$STORE/txns" ]; then
    check "braid init creates store" 0
else
    echo "  DEBUG: STORE=$STORE exists=$(test -d "$STORE" && echo yes || echo no)"
    ls -la "$STORE" 2>/dev/null || echo "  DEBUG: store dir not found"
    check "braid init creates store" 1
fi

# ── Test 2: braid task create ────────────────────────────────────────
echo "--- Test 2: braid task create ---"
CREATE_OUT=$($BRAID task create "E2E test task. ACCEPTANCE: braid query --attribute :task/title returns results" --priority 1 --type task -p "$STORE" -q 2>&1)
TASK_ID=$(echo "$CREATE_OUT" | grep -o 't-[a-f0-9]*' | head -1)
test -n "$TASK_ID"
check "braid task create returns task ID ($TASK_ID)" $?

# ── Test 3: braid go (set in-progress) ──────────────────────────────
echo "--- Test 3: braid go ---"
$BRAID go "$TASK_ID" -p "$STORE" -q 2>&1 | grep -q "in-progress"
check "braid go sets status to in-progress" $?

# ── Test 4: braid task set --title (FLAG form) ──────────────────────
echo "--- Test 4: braid task set --title (flag) ---"
$BRAID task set "$TASK_ID" --title "Revised title via flag. ACCEPTANCE: query returns results" -p "$STORE" -q 2>&1 | grep -q "set:"
check "braid task set --title (flag form) works" $?

# Verify the title actually changed
TITLE=$($BRAID query --entity ":task/$TASK_ID" --attribute :task/title -p "$STORE" -q 2>&1 | grep "Revised title via flag")
test -n "$TITLE"
check "title actually changed (flag)" $?

# ── Test 5: braid task set title (POSITIONAL form) ──────────────────
echo "--- Test 5: braid task set title (positional) ---"
$BRAID task set "$TASK_ID" title "Revised again positional. ACCEPTANCE: query returns results" -p "$STORE" -q 2>&1 | grep -q "set:"
check "braid task set title (positional form) works" $?

# ── Test 6: braid done --attest ──────────────────────────────────────
echo "--- Test 6: braid done --attest ---"
$BRAID done "$TASK_ID" --attest "E2E test verified" -p "$STORE" -q 2>&1 | grep -q "closed:"
check "braid done --attest closes with attestation" $?

# Verify completion-method recorded
COMPLETION=$($BRAID query --entity ":task/$TASK_ID" --attribute :task/completion-method -p "$STORE" -q 2>&1)
echo "$COMPLETION" | grep -q "attested"
check "completion-method = attested" $?

# ── Test 7: braid done --force on task with failing acceptance ───────
echo "--- Test 7: braid done --force ---"
# Create a task with acceptance criteria that will fail
CREATE2=$($BRAID task create "Force test. ACCEPTANCE: braid query --attribute :nonexistent/attr returns results" --priority 1 --type task -p "$STORE" -q 2>&1)
TASK2=$(echo "$CREATE2" | grep -o 't-[a-f0-9]*' | head -1)
$BRAID go "$TASK2" -p "$STORE" -q 2>&1 > /dev/null
$BRAID done "$TASK2" --force -p "$STORE" -q 2>&1 | grep -q "closed:"
check "braid done --force bypasses failing acceptance" $?

# Verify force completion method
FORCE_METHOD=$($BRAID query --entity ":task/$TASK2" --attribute :task/completion-method -p "$STORE" -q 2>&1)
echo "$FORCE_METHOD" | grep -q "force"
check "completion-method = force" $?

# ── Test 8: braid status -q (no footer) ─────────────────────────────
echo "--- Test 8: braid status -q ---"
STATUS_Q=$($BRAID status -p "$STORE" -q 2>&1)
STATUS_FULL=$($BRAID status -p "$STORE" 2>&1)
# -q output should be shorter (no footer)
LEN_Q=$(echo "$STATUS_Q" | wc -c)
LEN_FULL=$(echo "$STATUS_FULL" | wc -c)
test "$LEN_Q" -lt "$LEN_FULL"
check "braid status -q suppresses footer" $?

# ── Test 9: braid transact ──────────────────────────────────────────
echo "--- Test 9: braid transact ---"
$BRAID transact -d ':test/e2e-workflow' :db/doc 'E2E test datom' -p "$STORE" -q 2>&1 | grep -q "asserted"
check "braid transact creates datom" $?

# Verify datom exists
$BRAID query --entity ':test/e2e-workflow' -p "$STORE" -q 2>&1 | grep -q "E2E test datom"
check "transacted datom is queryable" $?

# ── Summary ──────────────────────────────────────────────────────────
echo ""
echo "=== Results ==="
echo "PASS: $PASS"
echo "FAIL: $FAIL"
echo "TOTAL: $((PASS + FAIL))"

if [ "$FAIL" -gt 0 ]; then
    echo "STATUS: FAILED"
    exit 1
else
    echo "STATUS: ALL PASSED"
    exit 0
fi
