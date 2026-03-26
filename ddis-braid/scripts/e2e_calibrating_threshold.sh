#!/usr/bin/env bash
# E2E Calibrating Threshold Test (ADR-FOUNDATION-031)
#
# Validates the self-calibrating concept threshold pipeline:
# - Fresh init produces innate concepts with revised descriptions
# - Observations get sigmoid soft membership (not binary)
# - Harvest triggers Otsu calibration
# - Calibrated threshold differs from bootstrap default
# - Concept distribution has entropy > 0
#
# Traces to: ADR-FOUNDATION-031, INV-EMBEDDING-004, OBSERVER-4
#
# Usage: ./scripts/e2e_calibrating_threshold.sh

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
# (1) Innate concepts exist with revised descriptions
# ===================================================================
log "Check 1: Innate concepts with revised descriptions"
CONCEPTS=$($BRAID query -q 2>/dev/null -- '[:find ?name ?desc :where [?c :concept/innate true] [?c :concept/name ?name] [?c :concept/description ?desc]]' 2>/dev/null)
if echo "$CONCEPTS" | grep -q "Discrete isolated parts"; then
    check "revised innate description (components)" "PASS"
else
    check "revised innate description (components)" "FAIL"
fi

# ===================================================================
# Make 8 diverse observations
# ===================================================================
log "Making 8 observations..."
$BRAID observe "The main module handles command-line argument parsing and dispatches to subcommands" --confidence 0.8 -q 2>/dev/null
$BRAID observe "Database schema uses 15 normalized tables with foreign key constraints for referential integrity" --confidence 0.85 -q 2>/dev/null
$BRAID observe "Import graph shows circular dependency between the auth and session packages" --confidence 0.9 -q 2>/dev/null
$BRAID observe "All public API functions follow the Result pattern with custom error types" --confidence 0.8 -q 2>/dev/null
$BRAID observe "Test coverage is 40% overall but 0% in the migration subsystem which handles schema upgrades" --confidence 0.85 -q 2>/dev/null
$BRAID observe "The event sourcing pipeline has a race condition when two writers commit simultaneously" --confidence 0.9 -q 2>/dev/null
$BRAID observe "Configuration is loaded from YAML files with environment variable overrides using a layered approach" --confidence 0.75 -q 2>/dev/null
$BRAID observe "The retry logic uses exponential backoff with jitter but the maximum delay is hardcoded to 30 seconds" --confidence 0.8 -q 2>/dev/null

# ===================================================================
# (2) Not all observations in all concepts (sigmoid discrimination)
# ===================================================================
log "Check 2: Concept assignment shows discrimination"
STATUS=$($BRAID status -q 2>/dev/null || true)
# Check that NOT all concepts have the same member count
COUNTS=$($BRAID query -q 2>/dev/null -- '[:find ?name ?count :where [?c :concept/name ?name] [?c :concept/member-count ?count]]' 2>/dev/null | grep -v "^?" | grep -v "^-" | grep -v "^$")
if echo "$COUNTS" | grep -q "0"; then
    check "some concepts have 0 members (not collapsed)" "PASS"
else
    # Even if all have members, check they differ
    UNIQUE_COUNTS=$(echo "$COUNTS" | awk '{print $NF}' | sort -u | wc -l)
    if [ "$UNIQUE_COUNTS" -gt 1 ]; then
        check "concept member counts differ (not uniform)" "PASS"
    else
        check "concept discrimination (not all same count)" "FAIL"
    fi
fi

# ===================================================================
# (3) Harvest triggers calibration
# ===================================================================
log "Check 3: Harvest triggers threshold calibration"
HARVEST_OUT=$($BRAID harvest --commit -q 2>/dev/null || true)
# After harvest, check if :config/concept.join-threshold exists
THRESHOLD=$($BRAID query -q 2>/dev/null -- '[:find ?v :where [?e :db/ident ":config/concept.join-threshold"] [?e :config/value ?v]]' 2>/dev/null | grep -v "^?" | grep -v "^-" | grep -v "^$")
if [ -n "$THRESHOLD" ]; then
    check "calibrated threshold datom exists after harvest" "PASS"
    log "    calibrated threshold: $THRESHOLD"
else
    check "calibrated threshold datom exists after harvest" "FAIL"
fi

# ===================================================================
# (4) Calibrated threshold differs from default
# ===================================================================
log "Check 4: Calibrated threshold differs from bootstrap default (0.20)"
if [ -n "$THRESHOLD" ]; then
    # Extract numeric value
    THRESH_VAL=$(echo "$THRESHOLD" | tr -d '"' | tr -d ' ')
    if [ "$THRESH_VAL" != "0.2000" ] && [ "$THRESH_VAL" != "0.20" ] && [ "$THRESH_VAL" != "0.2" ]; then
        check "threshold calibrated away from default" "PASS"
        log "    threshold value: $THRESH_VAL (default was 0.20)"
    else
        check "threshold calibrated away from default" "FAIL"
    fi
else
    check "threshold calibrated away from default (no datom)" "FAIL"
fi

# ===================================================================
# (5) Temperature datom exists
# ===================================================================
log "Check 5: Sigmoid temperature datom exists"
TEMP=$($BRAID query -q 2>/dev/null -- '[:find ?v :where [?e :db/ident ":config/concept.sigmoid-temperature"] [?e :config/value ?v]]' 2>/dev/null | grep -v "^?" | grep -v "^-" | grep -v "^$")
if [ -n "$TEMP" ]; then
    check "sigmoid temperature datom exists" "PASS"
    log "    temperature: $TEMP"
else
    check "sigmoid temperature datom exists" "FAIL"
fi

# ===================================================================
# (6) Status shows concepts (system is functional end-to-end)
# ===================================================================
log "Check 6: Status shows concept information"
FINAL_STATUS=$($BRAID status -q 2>/dev/null || true)
if echo "$FINAL_STATUS" | grep -qi "concept"; then
    check "status shows concept information" "PASS"
else
    check "status shows concept information" "FAIL"
fi

# ===================================================================
# Results
# ===================================================================
echo ""
log "=========================================="
log "Calibrating Threshold E2E Results: $PASS/$TOTAL passed, $FAIL failed"
log "=========================================="

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
exit 0
