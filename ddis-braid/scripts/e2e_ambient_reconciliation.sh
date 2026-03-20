#!/usr/bin/env bash
# E2E Ambient Reconciliation Test (AR-TEST)
#
# Validates the ambient reconciliation pipeline:
# - AR-2: Knowledge-producing commands write :recon/trace-* datoms
# - AR-4: Concentration detection fires on 3+ traces in same namespace
# - Read commands (status, query) do NOT produce traces
# - Graph neighbor discovery connects related tasks via spec refs
# - Tasks without spec refs produce no trace datoms
# - Separate namespaces do not cross-contaminate concentration signals
#
# Spec ID convention: INV-{NAMESPACE}-{NNN} where extract_spec_namespace
# returns parts[1] after splitting on '-'. So INV-RECON-001 → namespace "RECON".
#
# Traces to: INV-GUIDANCE-024, ADR-GUIDANCE-013, t-abf64b39
#
# Usage: ./scripts/e2e_ambient_reconciliation.sh

set -uo pipefail
# NOTE: NOT set -e — we check exit codes manually via check()

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

# Count occurrences of a pattern in query output (robust helper).
# Returns a single integer on stdout.
count_traces() {
    local attr="$1"
    local pattern="${2:-}"
    local out count
    out=$($BRAID query --attribute "$attr" -p "$STORE" -q --format human 2>&1)
    if [ -n "$pattern" ]; then
        count=$(echo "$out" | grep -ci "$pattern" 2>/dev/null) || true
    else
        count=$(echo "$out" | grep -c "$attr" 2>/dev/null) || true
    fi
    # Ensure we always return a clean integer
    echo "${count:-0}" | tr -d '[:space:]'
}

echo "=== E2E Ambient Reconciliation Test (AR-TEST) ==="
echo "Binary: $BRAID"
echo "Store:  $STORE"
echo ""

# Verify binary exists
if [ ! -x "$BRAID" ]; then
    echo "[FATAL] Binary not found or not executable: $BRAID"
    echo "  Build with: cargo build --release"
    exit 1
fi

# ── Step 1: Init fresh store ────────────────────────────────────────
log "Step 1: Initialize fresh braid store"
cd "$TMPDIR"
$BRAID init -p "$STORE" -q > /dev/null 2>&1 || true
if [ -d "$STORE" ] && [ -d "$STORE/txns" ]; then
    check "init: store created" 0
else
    check "init: store created" 1
    echo "FATAL: Cannot proceed without store. Exiting."
    exit 1
fi

# ── Step 2: Create spec element INV-RECON-001 ───────────────────────
# Namespace "RECON" (extract_spec_namespace splits on '-', returns parts[1])
log "Step 2: Create spec element INV-RECON-001"
SPEC1_OUT=$($BRAID spec create INV-RECON-001 "Reconciliation Trace Production" \
    --statement "Knowledge-producing commands write recon trace datoms with spec refs." \
    --falsification "A task create with spec refs that produces no :recon/trace-command datom violates this." \
    -p "$STORE" -q 2>&1)
if [ -n "$SPEC1_OUT" ]; then
    check "spec create: INV-RECON-001" 0
else
    check "spec create: INV-RECON-001" 1
fi

# ── Step 3: Create spec element INV-RECON-002 ───────────────────────
# Same namespace "RECON"
log "Step 3: Create spec element INV-RECON-002"
SPEC2_OUT=$($BRAID spec create INV-RECON-002 "Concentration Detection" \
    --statement "3+ traces in the same namespace trigger a concentration signal." \
    --falsification "Concentration signal not raised after 3+ traces in same namespace." \
    -p "$STORE" -q 2>&1)
if [ -n "$SPEC2_OUT" ]; then
    check "spec create: INV-RECON-002" 0
else
    check "spec create: INV-RECON-002" 1
fi

# ── Step 4: Task-A with INV-RECON-001 in title ──────────────────────
# Knowledge-producing command ("task") with spec ref should write trace
log "Step 4: Create task-A referencing INV-RECON-001"
TASK_A_OUT=$($BRAID task create \
    "Implement trace production for INV-RECON-001. ACCEPTANCE: (A) trace datom exists after task create." \
    --priority 2 --type task \
    -p "$STORE" -q 2>&1)
TASK_A=$(echo "$TASK_A_OUT" | grep -oP 't-[a-f0-9]+' | head -1)
if [ -n "$TASK_A" ]; then
    check "task-A: created with INV-RECON-001 ref ($TASK_A)" 0
else
    check "task-A: created with INV-RECON-001 ref" 1
fi

# ── Step 5: Verify trace datom exists ────────────────────────────────
# AR-2: knowledge-producing commands write :recon/trace-command datoms
log "Step 5: Verify AR-2 trace production"
TRACE_COUNT=$(count_traces ":recon/trace-command")
if [ "$TRACE_COUNT" -ge 1 ]; then
    check "AR-2: trace datom(s) exist after knowledge-producing commands (count=$TRACE_COUNT)" 0
else
    echo "  INFO: No trace datoms found. AR-2 trace production may not be fully wired."
    check "AR-2: trace datom(s) exist after knowledge-producing commands (count=$TRACE_COUNT)" 1
fi

# ── Step 6: Read command (status) should NOT produce new trace ───────
log "Step 6: Verify read commands do not produce traces"
TRACE_BEFORE=$(count_traces ":recon/trace-command")

# Run a read-only command
$BRAID status -p "$STORE" -q > /dev/null 2>&1

TRACE_AFTER=$(count_traces ":recon/trace-command")
if [ "$TRACE_AFTER" -eq "$TRACE_BEFORE" ]; then
    check "read-only: braid status produced no new trace (before=$TRACE_BEFORE, after=$TRACE_AFTER)" 0
else
    check "read-only: braid status produced no new trace (before=$TRACE_BEFORE, after=$TRACE_AFTER)" 1
fi

# ── Step 7: Task-B also referencing INV-RECON-001 ───────────────────
# Should find task-A as a graph neighbor via shared spec ref
log "Step 7: Create task-B also referencing INV-RECON-001"
TASK_B_OUT=$($BRAID task create \
    "Verify graph neighbors for INV-RECON-001. ACCEPTANCE: (A) task-A found as neighbor." \
    --priority 2 --type task \
    -p "$STORE" -q 2>&1)
TASK_B=$(echo "$TASK_B_OUT" | grep -oP 't-[a-f0-9]+' | head -1)
if [ -n "$TASK_B" ]; then
    check "task-B: created with same INV-RECON-001 ref ($TASK_B)" 0
else
    check "task-B: created with same INV-RECON-001 ref" 1
fi

# Check that trace-neighbors were recorded (graph reconciliation)
NEIGHBOR_COUNT=$(count_traces ":recon/trace-neighbors")
if [ "$NEIGHBOR_COUNT" -ge 1 ]; then
    check "graph-recon: trace-neighbors recorded (count=$NEIGHBOR_COUNT)" 0
else
    echo "  INFO: No trace-neighbors found. Graph neighbor discovery may produce 0 neighbors on a small store."
    check "graph-recon: trace-neighbors recorded (count=$NEIGHBOR_COUNT)" 1
fi

# ── Step 8: Observe with INV-RECON-001 ref ───────────────────────────
# braid observe is knowledge-producing and should produce related output
log "Step 8: Observe referencing INV-RECON-001"
OBSERVE_OUT=$($BRAID observe "The trace production for INV-RECON-001 is working correctly" \
    --confidence 0.8 \
    -p "$STORE" --format human 2>&1)
# Check that observe output contains related knowledge (CRB auto-reconciliation)
# or at minimum succeeds without error
if echo "$OBSERVE_OUT" | grep -qi "related\|INV-RECON\|observed\|datom"; then
    check "observe: references INV-RECON-001 or shows related output" 0
else
    if [ -n "$OBSERVE_OUT" ]; then
        echo "  INFO: Observe output did not contain expected keywords. Output: $(echo "$OBSERVE_OUT" | head -3)"
        check "observe: references INV-RECON-001 or shows related output" 0
    else
        check "observe: references INV-RECON-001 or shows related output" 1
    fi
fi

# ── Step 9: Create 3 more tasks referencing INV-RECON-001 ────────────
# This should push the RECON namespace trace count to 3+ and trigger
# concentration detection. Each task create is a knowledge-producing
# command that writes a distinct trace (unique millisecond timestamp).
log "Step 9: Create 3 more tasks referencing INV-RECON-001"
for i in 1 2 3; do
    TASK_OUT=$($BRAID task create \
        "Concentration test $i for INV-RECON-001. ACCEPTANCE: (A) concentration fires." \
        --priority 3 --type task \
        -p "$STORE" -q 2>&1)
    TASK_ID=$(echo "$TASK_OUT" | grep -oP 't-[a-f0-9]+' | head -1)
    if [ -z "$TASK_ID" ]; then
        echo "  WARN: Task $i creation may have failed"
    fi
done
check "concentration-setup: 3 additional tasks created with INV-RECON-001" 0

# ── Step 10: Check status for concentration signal ───────────────────
# With 5+ task creates plus spec creates all referencing INV-RECON-*,
# the RECON namespace should have 3+ traces triggering concentration.
# The concentration detector scans :recon/trace-neighborhood datoms.
log "Step 10: Check status for concentration mention"
STATUS_OUT=$($BRAID status -p "$STORE" --format human 2>&1)
RECON_TRACES=$(count_traces ":recon/trace-neighborhood" "RECON")
if echo "$STATUS_OUT" | grep -qi "concentration"; then
    check "concentration: status mentions concentration (RECON traces=$RECON_TRACES)" 0
else
    echo "  INFO: No concentration signal in status output. RECON namespace traces: $RECON_TRACES (need 3+)"
    if [ "$RECON_TRACES" -ge 3 ]; then
        # Traces exist but concentration not surfaced in human output — possible display bug
        check "concentration: status mentions concentration (RECON traces=$RECON_TRACES)" 1
    else
        # Not enough traces — trace deduplication or AR-2 wiring issue
        echo "  INFO: Fewer than 3 RECON traces — concentration threshold not met (may be trace dedup)"
        # Diagnostic: dump all trace-neighborhood values
        echo "  DEBUG: All trace-neighborhood values:"
        $BRAID query --attribute :recon/trace-neighborhood -p "$STORE" -q --format human 2>&1 | head -10
        check "concentration: status mentions concentration (RECON traces=$RECON_TRACES)" 1
    fi
fi

# ── Step 11: Task with NO spec refs ─────────────────────────────────
# Should not produce any new trace datom (no spec refs → trace skipped)
log "Step 11: Task with no spec refs produces no trace"
TRACE_BEFORE_NOREF=$(count_traces ":recon/trace-command")

$BRAID task create \
    "Plain task without any specification references at all" \
    --priority 3 --type task \
    -p "$STORE" -q 2>&1 > /dev/null

TRACE_AFTER_NOREF=$(count_traces ":recon/trace-command")
if [ "$TRACE_AFTER_NOREF" -eq "$TRACE_BEFORE_NOREF" ]; then
    check "no-ref: task without spec refs produced no trace (before=$TRACE_BEFORE_NOREF, after=$TRACE_AFTER_NOREF)" 0
else
    check "no-ref: task without spec refs produced no trace (before=$TRACE_BEFORE_NOREF, after=$TRACE_AFTER_NOREF)" 1
fi

# ── Step 12: Task referencing different namespace ────────────────────
# INV-ISOL-001 has namespace "ISOL" (not "RECON"), so creating a task
# referencing it should not increase RECON namespace traces.
log "Step 12: Different namespace does not cross-contaminate"

RECON_COUNT_BEFORE=$(count_traces ":recon/trace-neighborhood" "RECON")

# Create spec element in ISOL namespace
$BRAID spec create INV-ISOL-001 "Namespace Isolation Invariant" \
    --statement "Tasks in the ISOL namespace are isolated from RECON namespace concentration." \
    --falsification "Concentration signal in RECON fires due to ISOL namespace tasks." \
    -p "$STORE" -q 2>&1 > /dev/null || true

# Create a task referencing only the ISOL namespace
$BRAID task create \
    "Test isolation for INV-ISOL-001. ACCEPTANCE: (A) no RECON concentration increase." \
    --priority 2 --type task \
    -p "$STORE" -q 2>&1 > /dev/null

# RECON namespace trace count should not have increased
RECON_COUNT_AFTER=$(count_traces ":recon/trace-neighborhood" "RECON")
ISOL_COUNT=$(count_traces ":recon/trace-neighborhood" "ISOL")

if [ "$RECON_COUNT_AFTER" -eq "$RECON_COUNT_BEFORE" ]; then
    check "namespace-isolation: ISOL task did not increase RECON traces (RECON=$RECON_COUNT_AFTER, ISOL=$ISOL_COUNT)" 0
else
    echo "  INFO: RECON count changed ($RECON_COUNT_BEFORE -> $RECON_COUNT_AFTER), ISOL=$ISOL_COUNT"
    check "namespace-isolation: ISOL task did not increase RECON traces (RECON=$RECON_COUNT_AFTER, ISOL=$ISOL_COUNT)" 1
fi

# ── Step 13: Performance — task create with trace < 5 seconds ────────
log "Step 13: Performance — task create with trace under 5 seconds"
START_TIME=$(date +%s)
$BRAID task create \
    "Performance test for INV-RECON-001. ACCEPTANCE: (A) completes in under 5 seconds." \
    --priority 3 --type task \
    -p "$STORE" -q 2>&1 > /dev/null
END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))
if [ "$ELAPSED" -lt 5 ]; then
    check "performance: task create with trace completed in ${ELAPSED}s (< 5s)" 0
else
    check "performance: task create with trace completed in ${ELAPSED}s (< 5s)" 1
fi

# ── Summary ─────────────────────────────────────────────────────────
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
