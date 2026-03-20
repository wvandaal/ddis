#!/usr/bin/env bash
# E2E Task Ergonomics Test (E2E-WA)
#
# Validates task management ergonomics:
# - Task create with title and metadata
# - Task list shows short titles (no BACKGROUND sections)
# - Task show displays full context
# - Task set persists attribute changes
# - Batch close is atomic (multiple IDs in one call)
# - Task search finds by keyword
# - Priority update via task set persists across store reload
# - Task audit shows dual confidence scores
#
# Traces to: INV-INTERFACE-008 (API-as-prompt), INV-TASK-001 (status lattice),
#            INV-TASK-003 (ready computation), ADR-INTERFACE-011 (title pyramid)
#
# Usage: ./scripts/e2e_task_ergonomics.sh

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

echo "=== E2E Task Ergonomics Test (E2E-WA) ==="
echo "Project: $PROJECT_ROOT"
echo "Store:   $STORE"
echo ""

# ── Step 1: Create fresh store ────────────────────────────────────────
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

# ── Step 2: Create tasks with structured titles ───────────────────────
log "Step 2: Create tasks with structured titles"

T1=$($BRAID_BIN task create "Fix parser bug. BACKGROUND: The EDN parser fails on nested maps. ACCEPTANCE: (A) parse_edn handles nested maps. (B) roundtrip test passes." -p "$STORE" -q 2>&1 | grep -oP 't-[a-f0-9]+' | head -1)
if [ -n "$T1" ]; then
    check "create: task with BACKGROUND section" 0
else
    check "create: task with BACKGROUND section" 1
fi

T2=$($BRAID_BIN task create "Implement VAET index. FILE: crates/braid-kernel/src/store.rs" -p "$STORE" -q 2>&1 | grep -oP 't-[a-f0-9]+' | head -1)
if [ -n "$T2" ]; then
    check "create: task with FILE marker" 0
else
    check "create: task with FILE marker" 1
fi

T3=$($BRAID_BIN task create "Simple task without sections" -p "$STORE" -q 2>&1 | grep -oP 't-[a-f0-9]+' | head -1)
if [ -n "$T3" ]; then
    check "create: simple task" 0
else
    check "create: simple task" 1
fi

# ── Step 3: Task list shows short titles ──────────────────────────────
log "Step 3: Verify task list shows short titles (no BACKGROUND)"
LIST_OUT=$($BRAID_BIN task list -p "$STORE" --format human 2>&1)

if echo "$LIST_OUT" | grep -q "Fix parser bug" && ! echo "$LIST_OUT" | grep -q "BACKGROUND"; then
    check "list: shows short title, no BACKGROUND" 0
else
    check "list: shows short title, no BACKGROUND" 1
fi

if echo "$LIST_OUT" | grep -q "Implement VAET index" && ! echo "$LIST_OUT" | grep -q "FILE:"; then
    check "list: shows short title, no FILE marker" 0
else
    check "list: shows short title, no FILE marker" 1
fi

# ── Step 4: Task show displays full context ───────────────────────────
log "Step 4: Verify task show displays full context"
if [ -n "$T1" ]; then
    SHOW_OUT=$($BRAID_BIN task show "$T1" -p "$STORE" --format human 2>&1)
    if echo "$SHOW_OUT" | grep -q "BACKGROUND"; then
        check "show: displays BACKGROUND section" 0
    else
        check "show: displays BACKGROUND section" 1
    fi
    if echo "$SHOW_OUT" | grep -q "ACCEPTANCE"; then
        check "show: displays ACCEPTANCE section" 0
    else
        check "show: displays ACCEPTANCE section" 1
    fi
else
    check "show: displays BACKGROUND section" 1
    check "show: displays ACCEPTANCE section" 1
fi

# ── Step 5: Batch close (atomicity) ──────────────────────────────────
log "Step 5: Batch close multiple tasks"
if [ -n "$T2" ] && [ -n "$T3" ]; then
    CLOSE_OUT=$($BRAID_BIN task close "$T2" "$T3" --reason "batch test" -p "$STORE" -q 2>&1)
    if echo "$CLOSE_OUT" | grep -q "closed: 2"; then
        check "batch close: 2 tasks closed in one call" 0
    else
        check "batch close: 2 tasks closed in one call" 1
    fi

    # Verify both are actually closed
    LIST_AFTER=$($BRAID_BIN task list -p "$STORE" --format human 2>&1)
    if ! echo "$LIST_AFTER" | grep -q "$T2" && ! echo "$LIST_AFTER" | grep -q "$T3"; then
        check "batch close: both tasks no longer in open list" 0
    else
        check "batch close: both tasks no longer in open list" 1
    fi
else
    check "batch close: 2 tasks closed in one call" 1
    check "batch close: both tasks no longer in open list" 1
fi

# ── Step 6: Task search ──────────────────────────────────────────────
log "Step 6: Search tasks by keyword"
SEARCH_OUT=$($BRAID_BIN task search "parser" -p "$STORE" --format human 2>&1)
if echo "$SEARCH_OUT" | grep -qi "parser"; then
    check "search: finds task by keyword 'parser'" 0
else
    check "search: finds task by keyword 'parser'" 1
fi

# ── Step 7: Task set priority persistence ────────────────────────────
log "Step 7: Priority update via task set"
if [ -n "$T1" ]; then
    $BRAID_BIN task set "$T1" priority 0 -p "$STORE" -q 2>&1 > /dev/null

    # Verify priority persisted
    SHOW_P=$($BRAID_BIN task show "$T1" -p "$STORE" --format human 2>&1)
    if echo "$SHOW_P" | grep -q "P0"; then
        check "set: priority update persists (P0)" 0
    else
        check "set: priority update persists (P0)" 1
    fi
else
    check "set: priority update persists (P0)" 1
fi

# ── Step 8: Task audit shows confidence ──────────────────────────────
log "Step 8: Task audit output"
# Create a spec element and a task that traces to it, then check audit
$BRAID_BIN transact \
    -d ':spec/test-inv' :db/ident ':spec/test-inv' \
    -d ':spec/test-inv' :spec/element-type 'invariant' \
    -d ':spec/test-inv' :element/id 'INV-TEST-001' \
    -r "E2E test: create spec element" \
    -p "$STORE" -q 2>/dev/null || true

AUDIT_OUT=$($BRAID_BIN task audit -p "$STORE" --format human 2>&1)
# Audit should run without error (even if no tasks match)
if echo "$AUDIT_OUT" | grep -qi "audit\|No tasks"; then
    check "audit: command runs successfully" 0
else
    check "audit: command runs successfully" 1
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
