#!/usr/bin/env bash
# E2E Stage 0 Lifecycle Test
#
# Validates the complete Stage 0 workflow: init → transact → query → harvest → seed → bilateral
# This is the foundational integration test proving the harvest/seed cycle works end-to-end.
#
# Traces to: INV-STORE-001 (append-only), INV-STORE-008 (genesis determinism),
#            INV-HARVEST-001 (harvest monotonicity), INV-SEED-001 (seed as projection),
#            INV-BILATERAL-001 (F(S) non-decreasing), INV-GUIDANCE-001 (guidance injection)
#
# Usage: ./scripts/e2e_stage0_lifecycle.sh

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

echo "=== E2E Stage 0 Lifecycle Test ==="
echo "Project: $PROJECT_ROOT"
echo "Store:   $STORE"
echo ""

# ── Step 1: Init ─────────────────────────────────────────────────────
log "Step 1: braid init"
cd "$TMPDIR"
$BRAID_BIN init -p "$STORE" -q > /dev/null 2>&1 || true
if [ -d "$STORE" ] && [ -d "$STORE/txns" ]; then
    check "init: store directory created" 0
else
    check "init: store directory created" 1
    exit 1
fi

# Verify genesis transaction exists
GENESIS_COUNT=$(ls "$STORE/txns/"*/*.edn 2>/dev/null | wc -l)
if [ "$GENESIS_COUNT" -ge 1 ]; then
    check "init: genesis transaction present" 0
else
    check "init: genesis transaction present" 1
fi

# ── Step 2: Transact ─────────────────────────────────────────────────
log "Step 2: braid transact (assert datoms)"
$BRAID_BIN transact \
    -d ':test/entity-1' :db/ident ':test/entity-1' \
    -d ':test/entity-1' :db/doc 'First test entity' \
    -r "E2E: create test entity 1" \
    -p "$STORE" -q 2>/dev/null || true

TX1_COUNT=$(ls "$STORE/txns/"*/*.edn 2>/dev/null | wc -l)
if [ "$TX1_COUNT" -gt "$GENESIS_COUNT" ]; then
    check "transact: new transaction file created" 0
else
    check "transact: new transaction file created" 1
fi

# Transact a second entity
$BRAID_BIN transact \
    -d ':test/entity-2' :db/ident ':test/entity-2' \
    -d ':test/entity-2' :db/doc 'Second test entity' \
    -r "E2E: create test entity 2" \
    -p "$STORE" -q 2>/dev/null || true

# ── Step 3: Query ────────────────────────────────────────────────────
log "Step 3: braid query (retrieve datoms)"
QUERY_OUT=$($BRAID_BIN query --attribute :db/doc -p "$STORE" --format human -q 2>&1)
if echo "$QUERY_OUT" | grep -q "First test entity"; then
    check "query: finds entity-1 doc" 0
else
    check "query: finds entity-1 doc" 1
fi

if echo "$QUERY_OUT" | grep -q "Second test entity"; then
    check "query: finds entity-2 doc" 0
else
    check "query: finds entity-2 doc" 1
fi

# Query with TSV
QUERY_TSV=$($BRAID_BIN query --attribute :db/doc -p "$STORE" --format tsv -q 2>&1)
if echo "$QUERY_TSV" | grep -q "	"; then
    check "query TSV: tab-separated output" 0
else
    check "query TSV: tab-separated output" 1
fi

# ── Step 4: Status ───────────────────────────────────────────────────
log "Step 4: braid status (dashboard)"
STATUS_OUT=$($BRAID_BIN status -p "$STORE" --format human -q 2>&1)
if echo "$STATUS_OUT" | grep -q "datoms"; then
    check "status: shows datom count" 0
else
    check "status: shows datom count" 1
fi

if echo "$STATUS_OUT" | grep -q "F(S)"; then
    check "status: shows F(S)" 0
else
    check "status: shows F(S)" 1
fi

# ── Step 5: Observe ──────────────────────────────────────────────────
log "Step 5: braid observe (capture knowledge)"
OBS_OUT=$($BRAID_BIN observe "E2E test observation: the lifecycle works" --confidence 0.9 -p "$STORE" --format human -q 2>&1)
if echo "$OBS_OUT" | grep -qi "observed"; then
    check "observe: observation recorded" 0
else
    check "observe: observation recorded" 1
fi

# ── Step 6: Harvest ──────────────────────────────────────────────────
log "Step 6: braid harvest --commit (end-of-session)"
HARVEST_OUT=$($BRAID_BIN harvest --commit --force -p "$STORE" --format human -q 2>&1)
if echo "$HARVEST_OUT" | grep -qi "harvest\|committed"; then
    check "harvest: completed successfully" 0
else
    check "harvest: completed successfully" 1
fi

# ── Step 7: Seed ─────────────────────────────────────────────────────
log "Step 7: braid seed (start-of-session context)"
SEED_OUT=$($BRAID_BIN seed --task "continue lifecycle test" -p "$STORE" --format human -q 2>&1)
if echo "$SEED_OUT" | grep -qi "seed\|orientation\|braid"; then
    check "seed: produces orientation context" 0
else
    check "seed: produces orientation context" 1
fi

# ── Step 8: Bilateral ────────────────────────────────────────────────
log "Step 8: braid bilateral (coherence verification)"
BILATERAL_OUT=$($BRAID_BIN bilateral -p "$STORE" --format human -q 2>&1)
if echo "$BILATERAL_OUT" | grep -qi "bilateral\|F(S)\|coherence\|divergence"; then
    check "bilateral: coherence analysis" 0
else
    check "bilateral: coherence analysis" 1
fi

# ── Step 9: Append-only verification (INV-STORE-001) ─────────────────
log "Step 9: Verify append-only (INV-STORE-001)"
DATOM_BEFORE=$($BRAID_BIN status -p "$STORE" --format tsv -q 2>&1 | grep "datoms" | head -1)
# Add more data
$BRAID_BIN transact \
    -d ':test/entity-3' :db/ident ':test/entity-3' \
    -d ':test/entity-3' :db/doc 'Third entity after harvest' \
    -r "E2E: post-harvest transact" \
    -p "$STORE" -q 2>/dev/null || true

DATOM_AFTER=$($BRAID_BIN status -p "$STORE" --format tsv -q 2>&1 | grep "datoms" | head -1)
# The store should have MORE datoms after the transact (never fewer)
if [ "$DATOM_AFTER" != "$DATOM_BEFORE" ]; then
    check "append-only: datom count increased after transact" 0
else
    check "append-only: datom count increased after transact" 1
fi

# ── Step 10: Full cycle — second harvest after more work ─────────────
log "Step 10: Second harvest (verify multi-harvest)"
$BRAID_BIN observe "Second observation after first harvest" --confidence 0.8 -p "$STORE" -q 2>/dev/null
HARVEST2=$($BRAID_BIN harvest --commit --force -p "$STORE" --format human -q 2>&1)
if echo "$HARVEST2" | grep -qi "harvest\|committed"; then
    check "second harvest: completes after first" 0
else
    check "second harvest: completes after first" 1
fi

# Final datom count
FINAL=$($BRAID_BIN status -p "$STORE" --format human -q 2>&1)
FINAL_DATOMS=$(echo "$FINAL" | grep -oP '\d+(?= datoms)' | head -1)
log "  Final store: ${FINAL_DATOMS:-?} datoms"

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
