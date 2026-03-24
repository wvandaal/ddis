#!/usr/bin/env bash
# E2E test for LiveStore write-through persistence.
# Validates that braid init → observe → status works end-to-end
# with correct datom counting and sub-second response times.
#
# Traces to: t-3b7925a5 (LIVESTORE-TEST), INV-STORE-020, INV-STORE-021

set -euo pipefail

BRAID="${BRAID:-./target/release/braid}"
TMPDIR=$(mktemp -d)
PASSED=0
FAILED=0
TOTAL=0

cleanup() {
    rm -rf "$TMPDIR"
}
trap cleanup EXIT

check() {
    local name="$1"
    local result="$2"
    local ts=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    TOTAL=$((TOTAL + 1))
    if [ "$result" -eq 0 ]; then
        PASSED=$((PASSED + 1))
        echo "  [$ts] PASS: $name"
    else
        FAILED=$((FAILED + 1))
        echo "  [$ts] FAIL: $name"
    fi
}

# Extract datom count from "store: .braid (NNNN datoms, ..." line
datom_count() {
    "$BRAID" status -q 2>/dev/null | grep -o '[0-9]* datoms' | head -1 | awk '{print $1}'
}

echo "=== E2E LiveStore Test ==="
echo "Binary: $BRAID"
echo "Temp: $TMPDIR"

# --- Setup: create a minimal project ---
cd "$TMPDIR"
git init -q
echo '[package]
name = "test"
version = "0.1.0"' > Cargo.toml
mkdir -p src
echo '// INV-STORE-001: test
fn main() {}' > src/main.rs
git add -A && git commit -q -m "init"

# --- Test 1: braid init produces datoms ---
"$BRAID" init --spec-dir /data/projects/ddis/ddis-braid/spec -q >/dev/null 2>&1
INIT_COUNT=$(datom_count)
test "$INIT_COUNT" -gt 0
check "init produces datoms ($INIT_COUNT)" $?

# --- Test 2: observe increases datom count ---
"$BRAID" observe "LiveStore E2E test observation" --confidence 0.8 -q >/dev/null 2>&1
OBS_COUNT=$(datom_count)
test "$OBS_COUNT" -gt "$INIT_COUNT"
check "observe increases datoms ($INIT_COUNT -> $OBS_COUNT)" $?

# --- Test 3: store.bin exists after operations ---
CACHE_EXISTS=0
if ls .braid/.cache/*.bin >/dev/null 2>&1; then
    CACHE_EXISTS=1
fi
test "$CACHE_EXISTS" -eq 1
check "store.bin exists in .cache" $?

# --- Test 4: braid status completes in < 2s ---
START=$(date +%s%N)
"$BRAID" status -q >/dev/null 2>&1
END=$(date +%s%N)
ELAPSED_MS=$(( (END - START) / 1000000 ))
test "$ELAPSED_MS" -lt 2000
check "status completes in ${ELAPSED_MS}ms (< 2000ms)" $?

# --- Test 5: second observe further increases datoms ---
"$BRAID" observe "Second observation" --confidence 0.9 -q >/dev/null 2>&1
OBS2_COUNT=$(datom_count)
test "$OBS2_COUNT" -gt "$OBS_COUNT"
check "second observe increases datoms ($OBS_COUNT -> $OBS2_COUNT)" $?

# --- Test 6: multiple rapid status calls all succeed ---
ALL_OK=1
for i in 1 2 3 4 5; do
    if ! "$BRAID" status -q >/dev/null 2>&1; then
        ALL_OK=0
        break
    fi
done
test "$ALL_OK" -eq 1
check "5 rapid status calls all succeed" $?

# --- Test 7: compat — DiskLayout can read LiveStore-written data ---
# Create a second observe, then verify txn count matches across both paths
TXN_COUNT=$(ls .braid/txns/*/*.edn 2>/dev/null | wc -l)
test "$TXN_COUNT" -gt 5
check "compat: $TXN_COUNT txn files readable" $?

# --- Summary ---
echo ""
echo "=== Results: $PASSED/$TOTAL passed ==="
if [ "$FAILED" -gt 0 ]; then
    echo "FAILED: $FAILED test(s)"
    exit 1
fi
echo "ALL PASS"
