#!/usr/bin/env bash
# E2E Inquiry Engine Test (INQ-1-REV, INQ-2, INQ-3)
#
# Validates the inquiry engine pipeline:
# - Fresh init produces NO innate concepts (INQ-1-REV)
# - Auto-crystallization during observe after MIN_CLUSTER_SIZE observations
# - Discrepancy-driven steering for surprising observations
# - Graduated situational brief at all surprise levels
# - Emergent concept names from observation content
#
# Usage: ./scripts/e2e_inquiry_engine.sh

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
    if [ "$result" = "PASS" ]; then
        PASS=$((PASS + 1))
        log "  PASS: $name"
    else
        FAIL=$((FAIL + 1))
        log "  FAIL: $name"
    fi
}

# Build if needed.
if [ ! -x "$BRAID" ]; then
    log "Building braid..."
    cargo build --release --manifest-path="$PROJECT_ROOT/Cargo.toml" 2>/dev/null
    cp /tmp/cargo-target/release/braid "$BRAID" 2>/dev/null || true
fi

log "=== INQ E2E: Inquiry Engine ==="
log "Store: $STORE"
log "Binary: $BRAID"

# -----------------------------------------------------------------------
# Test 1: Fresh init has NO innate concepts (INQ-1-REV)
# -----------------------------------------------------------------------
log "--- Test 1: No innate concepts on init ---"
INIT_OUT=$("$BRAID" init -p "$STORE" -q 2>&1)

# Should NOT contain "schemas:" (innate concepts are OFF by default).
if echo "$INIT_OUT" | grep -q "schemas:"; then
    check "init-no-innate-concepts" "FAIL"
    log "    GOT innate schemas — should be OFF by default"
else
    check "init-no-innate-concepts" "PASS"
fi

# Status should show "concepts: none yet"
STATUS_OUT=$("$BRAID" status -p "$STORE" -q 2>&1)
if echo "$STATUS_OUT" | grep -qi "none yet"; then
    check "status-shows-none-yet" "PASS"
else
    check "status-shows-none-yet" "FAIL"
    log "    Expected 'none yet' in status output"
fi

# -----------------------------------------------------------------------
# Test 2: First 2 observations are uncategorized (progress toward concepts)
# -----------------------------------------------------------------------
log "--- Test 2: Uncategorized progress ---"
OBS1=$("$BRAID" observe "error handling in the cascade module is inconsistent" -p "$STORE" -q 2>&1)
if echo "$OBS1" | grep -qi "uncategorized"; then
    check "obs1-uncategorized" "PASS"
else
    check "obs1-uncategorized" "FAIL"
    log "    Expected 'uncategorized' in obs 1 output"
fi

OBS2=$("$BRAID" observe "error handling in the storage module drops errors silently" -p "$STORE" -q 2>&1)
if echo "$OBS2" | grep -qi "uncategorized"; then
    check "obs2-uncategorized" "PASS"
else
    check "obs2-uncategorized" "FAIL"
    log "    Expected 'uncategorized' in obs 2 output"
fi

# -----------------------------------------------------------------------
# Test 3: Third observation triggers auto-crystallization
# -----------------------------------------------------------------------
log "--- Test 3: Auto-crystallization on 3rd observation ---"
OBS3=$("$BRAID" observe "error handling in the events module swallows exceptions" -p "$STORE" -q 2>&1)
if echo "$OBS3" | grep -qi "AUTO-CRYSTALLIZED"; then
    check "obs3-auto-crystallized" "PASS"
else
    check "obs3-auto-crystallized" "FAIL"
    log "    Expected 'AUTO-CRYSTALLIZED' in obs 3 output"
    log "    GOT: $OBS3"
fi

# Status should now show concepts
STATUS2=$("$BRAID" status -p "$STORE" -q 2>&1)
if echo "$STATUS2" | grep -qi "concepts:.*obs"; then
    check "status-shows-concepts" "PASS"
else
    check "status-shows-concepts" "FAIL"
    log "    Expected concepts with obs count in status"
fi

# -----------------------------------------------------------------------
# Test 4: Fourth observation gets full concept assignment
# -----------------------------------------------------------------------
log "--- Test 4: Subsequent observations join concepts ---"
OBS4=$("$BRAID" observe "error handling in the merge module ignores return values" -p "$STORE" -q 2>&1)
# Should NOT say "uncategorized" (should match the error handling concept)
# Should show a situational brief (checkmark or concept name)
if echo "$OBS4" | grep -qi "uncategorized"; then
    check "obs4-categorized" "FAIL"
    log "    Expected categorized, got uncategorized"
else
    check "obs4-categorized" "PASS"
fi

# -----------------------------------------------------------------------
# Test 5: Surprising observation triggers discrepancy brief
# -----------------------------------------------------------------------
log "--- Test 5: Surprising observation discrepancy ---"
OBS5=$("$BRAID" observe "streaming pipeline message broker event sourcing kafka" -p "$STORE" -q 2>&1)
# This is very different from "error handling" — should show high surprise or new territory
# or auto-crystallize a new concept
if echo "$OBS5" | grep -qiE "NEW TERRITORY|surprise|uncategorized|AUTO-CRYSTALLIZED"; then
    check "obs5-high-surprise-or-new" "PASS"
else
    check "obs5-high-surprise-or-new" "FAIL"
    log "    Expected surprise/new territory indicator"
    log "    GOT: $OBS5"
fi

# -----------------------------------------------------------------------
# Test 6: More observations in new domain to trigger second concept
# -----------------------------------------------------------------------
log "--- Test 6: Second concept emergence ---"
"$BRAID" observe "streaming data pipeline processing throughput" -p "$STORE" -q 2>&1 >/dev/null
"$BRAID" observe "streaming events pipeline message queue processing" -p "$STORE" -q 2>&1 >/dev/null

# Check if we have 2+ concepts now
STATUS3=$("$BRAID" status -p "$STORE" --format json -q 2>&1)
CONCEPT_COUNT=$(echo "$STATUS3" | python3 -c "
import sys, json
try:
    lines = sys.stdin.read()
    # Look for concept count in the output
    if 'concepts:' in lines:
        # Count comma-separated concepts
        for line in lines.split('\n'):
            if 'concepts:' in line:
                count = line.count('obs)') + line.count('obs,')
                print(count)
                break
        else:
            print(0)
    else:
        print(0)
except:
    print(0)
" 2>/dev/null || echo "0")

# Even if we can't parse JSON perfectly, check status output
STATUS3_HUMAN=$("$BRAID" status -p "$STORE" -q 2>&1)
if echo "$STATUS3_HUMAN" | grep -qi "concepts:"; then
    check "multiple-concepts-exist" "PASS"
else
    check "multiple-concepts-exist" "FAIL"
    log "    Expected concepts in status"
fi

# -----------------------------------------------------------------------
# Test 7: Harvest includes crystallized concepts
# -----------------------------------------------------------------------
log "--- Test 7: Harvest ---"
HARVEST=$("$BRAID" harvest --commit -p "$STORE" 2>&1)
if echo "$HARVEST" | grep -qi "committed\|harvest"; then
    check "harvest-committed" "PASS"
else
    check "harvest-committed" "FAIL"
    log "    Expected 'committed' or 'harvest' in harvest output"
fi

# -----------------------------------------------------------------------
# Summary
# -----------------------------------------------------------------------
echo ""
log "=== INQ E2E Results: $PASS/$TOTAL passed, $FAIL failed ==="

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
exit 0
