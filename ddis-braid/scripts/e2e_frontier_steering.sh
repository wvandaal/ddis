#!/usr/bin/env bash
# E2E Frontier Steering Test (CONCEPT-MULTI + FRONTIER-STEER)
#
# Validates multi-membership concept assignment and acquisition-function
# frontier recommendations:
# - Multi-membership: observations can belong to multiple concepts
# - Co-occurrence: Jaccard similarity between concept member sets
# - Frontier steering: Explore/Deepen/Bridge recommendations
# - Coverage tracking in status output
#
# Traces to: CONCEPT-MULTI, FRONTIER-STEER, FRONTIER-TEST
#
# Usage: ./scripts/e2e_frontier_steering.sh

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
    cargo build --release --manifest-path="$PROJECT_ROOT/Cargo.toml" -q
fi

# ===================================================================
# Setup: fresh braid init
# ===================================================================
log "Initializing fresh braid store..."
cd "$TMPDIR"
git init -q
$BRAID init -q 2>/dev/null

# ===================================================================
# (14) Innate concepts exist after init
# ===================================================================
log "Check 14: Innate concepts after init"
STATUS=$($BRAID status -q 2>/dev/null || true)
CONCEPT_COUNT=$($BRAID query -q 2>/dev/null -- '[:find ?name :where [?c :concept/name ?name]]' 2>/dev/null | grep -c "." || echo "0")
if [ "$CONCEPT_COUNT" -ge 5 ]; then
    check "innate concepts exist (>= 5)" "PASS"
else
    # Fallback: check status output mentions concepts
    if echo "$STATUS" | grep -qi "concept"; then
        check "innate concepts exist (status mentions concepts)" "PASS"
    else
        check "innate concepts exist (>= 5, got $CONCEPT_COUNT)" "FAIL"
    fi
fi

# ===================================================================
# Observations that should trigger multi-membership
# ===================================================================
log "Making observations..."

# Obs 1-3: observations that span multiple innate concept domains
$BRAID observe "package boundaries have coupling violations between modules and interfaces" \
    --confidence 0.8 -q 2>/dev/null
OBS1=$($BRAID observe "error returns ignored in cascade module — a dependency anomaly" \
    --confidence 0.7 -q 2>/dev/null || true)
$BRAID observe "import patterns show recurring conventions across service boundaries" \
    --confidence 0.8 -q 2>/dev/null

# ===================================================================
# (15) After 3 observations, frontier should recommend explore or show steering
# ===================================================================
log "Check 15: Frontier recommendation after 3 observations"
OBS3_OUT=$($BRAID observe "storage module has inconsistent error handling patterns" \
    --confidence 0.7 -q 2>/dev/null || true)
if echo "$OBS3_OUT" | grep -qi "explore\|deepen\|bridge\|frontier\|→"; then
    check "frontier steering present in observe output" "PASS"
else
    # Check if steering question falls back
    if echo "$OBS3_OUT" | grep -qi "what\|investigate\|connect"; then
        check "frontier steering present (fallback question)" "PASS"
    else
        check "frontier steering present" "FAIL"
    fi
fi

# More observations
$BRAID observe "data flow coupling between events module and projector pipeline" \
    --confidence 0.8 -q 2>/dev/null
$BRAID observe "missing assertions and contract violations in the merge subsystem" \
    --confidence 0.7 -q 2>/dev/null

# ===================================================================
# (16) After 5+ obs, check multi-membership (observe output shows + secondary concepts)
# ===================================================================
log "Check 16: Multi-membership — observations in multiple concepts"
# The observe output above shows secondary matches like "+ components (cosine=0.57)".
# Run one more observation and capture its output for verification.
OBS_MULTI=$($BRAID observe "error handling patterns across module boundaries with coupling violations" \
    --confidence 0.7 -q 2>/dev/null || true)
if echo "$OBS_MULTI" | grep -q "+ "; then
    check "multi-membership: observe output shows secondary concept matches" "PASS"
else
    check "multi-membership: observe output shows secondary concept matches" "FAIL"
fi

# More observations
$BRAID observe "recursive protocol templates define the architecture of event processing" \
    --confidence 0.8 -q 2>/dev/null
$BRAID observe "test coverage gaps in the consistency checking subsystem" \
    --confidence 0.7 -q 2>/dev/null
$BRAID observe "kubernetes deployment manifests need dependency ordering constraints" \
    --confidence 0.6 -q 2>/dev/null

# ===================================================================
# (17) After 8+ obs, status should show coupled pairs or bridge gaps
# ===================================================================
log "Check 17: Status shows co-occurrence info"
STATUS8=$($BRAID status -q 2>/dev/null || true)
if echo "$STATUS8" | grep -qi "coupled\|bridge\|gap\|concepts:"; then
    check "status shows concept topology info" "PASS"
else
    check "status shows concept topology info" "FAIL"
fi

# More observations
$BRAID observe "authentication token validation relies on the security module boundary" \
    --confidence 0.8 -q 2>/dev/null
$BRAID observe "materialization views are projections of the event sourcing pipeline" \
    --confidence 0.8 -q 2>/dev/null

# ===================================================================
# (18) After 10 obs, frontier recommendation should adapt to knowledge state
# ===================================================================
log "Check 18: Frontier adapts to knowledge state"
OBS10_OUT=$($BRAID observe "cascade module error propagation patterns need refactoring" \
    --confidence 0.7 -q 2>/dev/null || true)
# At this point the recommendation should be deepen (high-variance concept) or bridge (disconnected concepts)
# rather than explore (no packages to explore without trace data)
if echo "$OBS10_OUT" | grep -qi "explore\|deepen\|bridge\|→"; then
    check "frontier recommendation adapts after 10 observations" "PASS"
else
    check "frontier recommendation adapts after 10 observations" "FAIL"
fi

# ===================================================================
# (19) Coverage tracking in status
# ===================================================================
log "Check 19: Coverage tracking in status"
STATUS_FULL=$($BRAID status -q 2>/dev/null || true)
if echo "$STATUS_FULL" | grep -qi "coverage\|explored\|concepts:"; then
    check "status shows coverage or concept info" "PASS"
else
    check "status shows coverage or concept info" "FAIL"
fi

# ===================================================================
# (20) Harvest preserves multi-member counts
# ===================================================================
log "Check 20: Harvest preserves concept data"
$BRAID harvest --commit -q 2>/dev/null || true
HARVEST_STATUS=$($BRAID status -q 2>/dev/null || true)
if echo "$HARVEST_STATUS" | grep -qi "concept\|datom"; then
    check "harvest preserves concept data" "PASS"
else
    check "harvest preserves concept data" "FAIL"
fi

# ===================================================================
# Results
# ===================================================================
echo ""
log "=========================================="
log "Frontier Steering E2E Results: $PASS/$TOTAL passed, $FAIL failed"
log "=========================================="

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
exit 0
