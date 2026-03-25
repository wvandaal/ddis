#!/usr/bin/env bash
# E2E Steering Loop Test (STEER-TEST)
#
# Validates the complete steering loop with model2vec:
# - Concept names are meaningful (not function words)
# - Concept assignment fires on natural text
# - Near-miss feedback shown when below threshold
# - Member counts increment on join
# - Harvest crystallizes emergent concepts
# - Status shows accurate concept inventory
#
# Requires: potion-base-8M model installed at ~/.braid/models/
# Traces to: STEER-1b, STEER-2, STEER-3b, LIFECYCLE-OBSERVE, LIFECYCLE-HARVEST

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BRAID="${BRAID:-${PROJECT_ROOT}/target/release/braid}"
TMPDIR=$(mktemp -d)
PASS=0
FAIL=0
TOTAL=0

cleanup() {
    cd /
    # Leave tmpdir for debugging on failure
    if [ "$FAIL" -eq 0 ]; then
        rm -rf "$TMPDIR" 2>/dev/null || true
    else
        echo "  TMPDIR preserved for debugging: $TMPDIR"
    fi
}
trap cleanup EXIT

log() { echo "[$(date '+%H:%M:%S')] $*"; }

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

# ===================================================================
# Setup
# ===================================================================
log "Initializing fresh braid store with model2vec..."
cd "$TMPDIR"
git init -q
$BRAID init -q 2>/dev/null

# --- Check 1: Model status ---
MODEL_STATUS=$($BRAID model status 2>&1)
if echo "$MODEL_STATUS" | grep -qi "model2vec"; then
    check "model2vec detected" "PASS"
else
    log "WARNING: model2vec not available. Tests will use hash embedder."
    log "Install model: mkdir -p ~/.braid/models/potion-base-8M && download model files"
    check "model2vec detected" "FAIL"
fi

# --- Check 2: Innate concept names are meaningful ---
STATUS=$($BRAID status -q 2>&1)
if echo "$STATUS" | grep -q "dependencies\|patterns\|invariants\|anomalies\|components"; then
    check "innate concept names are meaningful words" "PASS"
else
    check "innate concept names are meaningful words" "FAIL"
    log "  STATUS: $STATUS"
fi

# ===================================================================
# Concept Assignment + Lifecycle
# ===================================================================
log "Making 5 observations about error handling..."

OBS1=$($BRAID observe "the cascade module has five ignored error returns from Exec calls" -q 2>&1)
log "  obs1: $(echo "$OBS1" | grep 'concept:\|near:')"

OBS2=$($BRAID observe "storage database ignores error returns from write operations" -q 2>&1)
log "  obs2: $(echo "$OBS2" | grep 'concept:\|near:')"

OBS3=$($BRAID observe "error handling missing in event processing pipeline fold" -q 2>&1)
log "  obs3: $(echo "$OBS3" | grep 'concept:\|near:')"

OBS4=$($BRAID observe "materialize diff has missing rows.Err checks for error propagation" -q 2>&1)
log "  obs4: $(echo "$OBS4" | grep 'concept:\|near:')"

OBS5=$($BRAID observe "search engine swallows FTS5 errors with inconsistent handling strategy" -q 2>&1)
log "  obs5: $(echo "$OBS5" | grep 'concept:\|near:')"

# --- Check 3: At least one observation matched a concept ---
ALL_OBS="$OBS1 $OBS2 $OBS3 $OBS4 $OBS5"
if echo "$ALL_OBS" | grep -qi "concept:"; then
    check "at least one observation matched a concept" "PASS"
else
    check "at least one observation matched a concept" "FAIL"
fi

# --- Check 4: Near-miss feedback shown for non-matching obs ---
if echo "$ALL_OBS" | grep -qi "near:"; then
    check "near-miss feedback shown" "PASS"
else
    # If all matched, near-miss wouldn't appear — still OK
    if echo "$ALL_OBS" | grep -c "concept:" | grep -q "5"; then
        check "near-miss feedback shown (all matched, N/A)" "PASS"
    else
        check "near-miss feedback shown" "FAIL"
    fi
fi

# --- Check 5: No Δ-cryst noise in output ---
if echo "$ALL_OBS" | grep -qi "cryst:"; then
    check "no Δ-cryst noise (STEER-3b)" "FAIL"
else
    check "no Δ-cryst noise (STEER-3b)" "PASS"
fi

# --- Check 6: No 'details:' noise in output ---
if echo "$ALL_OBS" | grep -qi "details: braid query"; then
    check "no details: noise (STEER-3b)" "FAIL"
else
    check "no details: noise (STEER-3b)" "PASS"
fi

# --- Check 7: Status shows concept with member count > 0 ---
STATUS2=$($BRAID status -q 2>&1)
log "Status after 5 obs:"
echo "$STATUS2" | grep "concept" | head -3

if echo "$STATUS2" | grep -q "([1-9]"; then
    check "status shows concept with member_count > 0" "PASS"
else
    check "status shows concept with member_count > 0" "FAIL"
fi

# ===================================================================
# Harvest Crystallization
# ===================================================================
log "Adding 5 architecture observations for crystallization..."
$BRAID observe "the storage package is imported by 30 of 38 packages as universal dependency" -q 2>/dev/null
$BRAID observe "the CLI package has 54 files with one cobra command per file" -q 2>/dev/null
$BRAID observe "the parser package is a pure leaf with 19 specialized sub-parsers" -q 2>/dev/null
$BRAID observe "the autoprompt package defines the state monad triple for agent responses" -q 2>/dev/null
$BRAID observe "the bilateral cycle implements discover refine drift absorb as a dependency diamond" -q 2>/dev/null

log "Running harvest..."
HARVEST=$($BRAID harvest --commit -q 2>&1)
log "Harvest output (first 5 lines):"
echo "$HARVEST" | head -5

# --- Check 8: Harvest completes without error ---
if echo "$HARVEST" | grep -qi "error\|panic"; then
    check "harvest completes without error" "FAIL"
else
    check "harvest completes without error" "PASS"
fi

# --- Check 9: Status after harvest ---
STATUS3=$($BRAID status -q 2>&1)
log "Status after harvest:"
echo "$STATUS3" | grep "concept" | head -3

if echo "$STATUS3" | grep -q "concept"; then
    check "concepts exist after harvest" "PASS"
else
    check "concepts exist after harvest" "FAIL"
fi

# ===================================================================
# Summary
# ===================================================================
echo ""
log "========================================="
log "Steering Loop E2E Results: $PASS/$TOTAL passed"
if [ "$FAIL" -gt 0 ]; then
    log "$FAIL FAILED"
    exit 1
else
    log "ALL PASSED"
    exit 0
fi
