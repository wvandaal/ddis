#!/usr/bin/env bash
# E2E Policy Manifest Lifecycle Test
#
# Validates the full policy manifest lifecycle:
# - Init with/without manifest
# - Policy datom querying
# - F(S) computation from policy boundaries
# - Custom policy behavior
# - Backward compatibility (no-policy stores)
# - Validation error handling
#
# Traces to: ADR-FOUNDATION-013 (declarative policy), C8 (substrate independence),
#            INV-FOUNDATION-006 (methodology agnosticism), INV-FOUNDATION-007 (policy completeness)
#
# Usage: ./scripts/e2e_policy_manifest.sh

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BRAID_BIN="cargo run -q --manifest-path ${PROJECT_ROOT}/Cargo.toml --"
TMPDIR=$(mktemp -d)
PASS=0
FAIL=0
TOTAL=0

cleanup() {
    rm -rf "$TMPDIR"
}
trap cleanup EXIT

check() {
    local name="$1"
    local result="$2"
    local detail="${3:-}"
    TOTAL=$((TOTAL + 1))
    if [ "$result" -eq 0 ]; then
        echo "[PASS] $name"
        PASS=$((PASS + 1))
    else
        echo "[FAIL] $name"
        if [ -n "$detail" ]; then
            echo "       $detail"
        fi
        FAIL=$((FAIL + 1))
    fi
}

# Helper: compute BLAKE3 hash of a file (for content-addressed txn storage)
blake3_hash() {
    b3sum "$1" | awk '{print $1}'
}

# Helper: copy a .edn file into a store's txns/ as a content-addressed file
inject_tx() {
    local store_path="$1"
    local edn_file="$2"
    local hash
    hash=$(blake3_hash "$edn_file")
    local shard="${hash:0:2}"
    mkdir -p "$store_path/txns/$shard"
    cp "$edn_file" "$store_path/txns/$shard/${hash}.edn"
}

echo "=== E2E Policy Manifest Lifecycle Test ==="
echo "Project: $PROJECT_ROOT"
echo "Temp:    $TMPDIR"
echo ""

# ── Prepare: Create a 3-boundary test policy manifest ──
# Entity IDs are valid 64-char hex strings (all 0-9a-f)
MANIFEST="$TMPDIR/test-policy.edn"
cat > "$MANIFEST" <<'POLICY'
{:tx/id #hlc "999999999/0/aabb0000000000000000000000000001"
 :tx/agent "aabb0000000000000000000000000001"
 :tx/provenance :observed
 :tx/rationale "E2E test: 3-boundary policy manifest"
 :tx/causal-predecessors []
 :datoms [
   {:e #blake3 "aa00000000000000000000000000000000000000000000000000000000000b01" :a :policy/boundary-name :v "claim-evidence" :op :assert}
   {:e #blake3 "aa00000000000000000000000000000000000000000000000000000000000b01" :a :policy/boundary-source :v ":claim/*" :op :assert}
   {:e #blake3 "aa00000000000000000000000000000000000000000000000000000000000b01" :a :policy/boundary-target :v ":evidence/*" :op :assert}
   {:e #blake3 "aa00000000000000000000000000000000000000000000000000000000000b01" :a :policy/boundary-weight :v 0.5 :op :assert}
   {:e #blake3 "aa00000000000000000000000000000000000000000000000000000000000b02" :a :policy/boundary-name :v "spec-impl" :op :assert}
   {:e #blake3 "aa00000000000000000000000000000000000000000000000000000000000b02" :a :policy/boundary-source :v ":spec/*" :op :assert}
   {:e #blake3 "aa00000000000000000000000000000000000000000000000000000000000b02" :a :policy/boundary-target :v ":impl/*" :op :assert}
   {:e #blake3 "aa00000000000000000000000000000000000000000000000000000000000b02" :a :policy/boundary-weight :v 0.3 :op :assert}
   {:e #blake3 "aa00000000000000000000000000000000000000000000000000000000000b03" :a :policy/boundary-name :v "req-test" :op :assert}
   {:e #blake3 "aa00000000000000000000000000000000000000000000000000000000000b03" :a :policy/boundary-source :v ":req/*" :op :assert}
   {:e #blake3 "aa00000000000000000000000000000000000000000000000000000000000b03" :a :policy/boundary-target :v ":test/*" :op :assert}
   {:e #blake3 "aa00000000000000000000000000000000000000000000000000000000000b03" :a :policy/boundary-weight :v 0.2 :op :assert}
   {:e #blake3 "aa000000000000000000000000000000000000000000000000000000000c0001" :a :policy/claim-pattern :v ":claim/*" :op :assert}
   {:e #blake3 "aa000000000000000000000000000000000000000000000000000000000c0002" :a :policy/claim-pattern :v ":spec/*" :op :assert}
   {:e #blake3 "aa000000000000000000000000000000000000000000000000000000000e0001" :a :policy/evidence-pattern :v ":evidence/*" :op :assert}
   {:e #blake3 "aa000000000000000000000000000000000000000000000000000000000e0002" :a :policy/evidence-pattern :v ":impl/*" :op :assert}
 ]}
POLICY

# ── Check 1: Init with --manifest creates policy datoms ──
echo "--- Check 1: Init with manifest ---"
STORE1="$TMPDIR/store1/.braid"
mkdir -p "$TMPDIR/store1"
cd "$TMPDIR/store1"
INIT_OUT=$($BRAID_BIN init -p "$STORE1" --manifest "$MANIFEST" -q 2>&1) || true
HAS_POLICY=$(echo "$INIT_OUT" | grep -c "policy:" || true)
check "init with manifest: policy loaded" "$([ "$HAS_POLICY" -ge 1 ] && echo 0 || echo 1)" "$INIT_OUT"

# ── Check 2: Query policy boundaries returns 3 ──
echo "--- Check 2: Query policy boundaries ---"
BOUNDARY_COUNT=$($BRAID_BIN query --attribute :policy/boundary-name -p "$STORE1" -q 2>&1 | grep -c '^\[' || true)
check "query: 3 policy boundaries found" "$([ "$BOUNDARY_COUNT" -eq 3 ] && echo 0 || echo 1)" "got $BOUNDARY_COUNT"

# ── Check 3: Status uses policy-driven F(S) ──
echo "--- Check 3: Status with policy F(S) ---"
STATUS_OUT=$($BRAID_BIN status -p "$STORE1" -q 2>&1)
HAS_FS=$(echo "$STATUS_OUT" | grep -c "F(S)" || true)
check "status: F(S) present in output" "$([ "$HAS_FS" -ge 1 ] && echo 0 || echo 1)" "$STATUS_OUT"

# ── Check 4: Policy boundary weights sum to 1.0 ──
echo "--- Check 4: Weight sum ---"
WEIGHT_QUERY=$($BRAID_BIN query --attribute :policy/boundary-weight -p "$STORE1" -q 2>&1)
# Extract float values from the output
WEIGHT_SUM=$(echo "$WEIGHT_QUERY" | grep -oP '[\d]+\.[\d]+' | awk '{s+=$1} END {printf "%.1f", s}')
check "weights sum to 1.0" "$([ "$WEIGHT_SUM" = "1.0" ] && echo 0 || echo 1)" "sum=$WEIGHT_SUM"

# ── Check 5: Init without manifest = no policy datoms (C8) ──
echo "--- Check 5: Init without manifest (empty substrate) ---"
STORE2="$TMPDIR/store2/.braid"
mkdir -p "$TMPDIR/store2"
cd "$TMPDIR/store2"
$BRAID_BIN init -p "$STORE2" -q > /dev/null 2>&1 || true
NO_POLICY=$($BRAID_BIN query --attribute :policy/boundary-name -p "$STORE2" -q 2>&1 | grep -c '^\[' || true)
check "init without manifest: 0 policy boundaries (C8)" "$([ "$NO_POLICY" -eq 0 ] && echo 0 || echo 1)" "got $NO_POLICY"

# ── Check 6: Status on no-policy store falls back to views ──
echo "--- Check 6: No-policy fallback F(S) ---"
STATUS_NOPOL=$($BRAID_BIN status -p "$STORE2" -q 2>&1)
HAS_FS2=$(echo "$STATUS_NOPOL" | grep -c "F(S)" || true)
check "no-policy status: F(S) via fallback" "$([ "$HAS_FS2" -ge 1 ] && echo 0 || echo 1)"

# ── Check 7: Transact custom 2-boundary policy ──
echo "--- Check 7: Custom 2-boundary policy ---"
STORE3="$TMPDIR/store3/.braid"
CUSTOM_MANIFEST="$TMPDIR/custom-policy.edn"
cat > "$CUSTOM_MANIFEST" <<'CUSTOM'
{:tx/id #hlc "888888888/0/aabb0000000000000000000000000002"
 :tx/agent "aabb0000000000000000000000000002"
 :tx/provenance :observed
 :tx/rationale "E2E test: custom 2-boundary policy"
 :tx/causal-predecessors []
 :datoms [
   {:e #blake3 "bb0000000000000000000000000000000000000000000000000000000000aa01" :a :policy/boundary-name :v "requirement-test" :op :assert}
   {:e #blake3 "bb0000000000000000000000000000000000000000000000000000000000aa01" :a :policy/boundary-source :v ":requirement/*" :op :assert}
   {:e #blake3 "bb0000000000000000000000000000000000000000000000000000000000aa01" :a :policy/boundary-target :v ":verification/*" :op :assert}
   {:e #blake3 "bb0000000000000000000000000000000000000000000000000000000000aa01" :a :policy/boundary-weight :v 0.6 :op :assert}
   {:e #blake3 "bb0000000000000000000000000000000000000000000000000000000000aa02" :a :policy/boundary-name :v "design-review" :op :assert}
   {:e #blake3 "bb0000000000000000000000000000000000000000000000000000000000aa02" :a :policy/boundary-source :v ":design/*" :op :assert}
   {:e #blake3 "bb0000000000000000000000000000000000000000000000000000000000aa02" :a :policy/boundary-target :v ":review/*" :op :assert}
   {:e #blake3 "bb0000000000000000000000000000000000000000000000000000000000aa02" :a :policy/boundary-weight :v 0.4 :op :assert}
 ]}
CUSTOM
mkdir -p "$TMPDIR/store3"
cd "$TMPDIR/store3"
$BRAID_BIN init -p "$STORE3" --manifest "$CUSTOM_MANIFEST" -q > /dev/null 2>&1 || true
CUSTOM_COUNT=$($BRAID_BIN query --attribute :policy/boundary-name -p "$STORE3" -q 2>&1 | grep -c '^\[' || true)
check "custom policy: 2 boundaries loaded" "$([ "$CUSTOM_COUNT" -eq 2 ] && echo 0 || echo 1)" "got $CUSTOM_COUNT"

# ── Check 8: Custom policy status uses only custom boundaries ──
echo "--- Check 8: Custom policy in status ---"
STATUS_CUSTOM=$($BRAID_BIN status -p "$STORE3" -q 2>&1)
HAS_FS3=$(echo "$STATUS_CUSTOM" | grep -c "F(S)" || true)
check "custom policy status: F(S) present" "$([ "$HAS_FS3" -ge 1 ] && echo 0 || echo 1)"

# ── Check 9: Both policy stores produce valid F(S) ──
echo "--- Check 9: Both policies produce valid F(S) ---"
VALID1=$(echo "$STATUS_OUT" | grep -c "F(S)=" || true)
VALID3=$(echo "$STATUS_CUSTOM" | grep -c "F(S)=" || true)
check "both policies produce valid F(S)" "$([ "$VALID1" -ge 1 ] && [ "$VALID3" -ge 1 ] && echo 0 || echo 1)"

# ── Check 10: Adding source entities to policy store ──
echo "--- Check 10: Add source entities ---"
CLAIM_TX="$TMPDIR/claim-tx.edn"
cat > "$CLAIM_TX" <<'CLAIMTX'
{:tx/id #hlc "999999999/1/aabb0000000000000000000000000001"
 :tx/agent "aabb0000000000000000000000000001"
 :tx/provenance :observed
 :tx/rationale "E2E: add claim entities for coverage testing"
 :tx/causal-predecessors []
 :datoms [
   {:e #blake3 "cc00000000000000000000000000000000000000000000000000000000000001" :a :claim/title :v "Test Claim 1" :op :assert}
   {:e #blake3 "cc00000000000000000000000000000000000000000000000000000000000002" :a :claim/title :v "Test Claim 2" :op :assert}
 ]}
CLAIMTX
inject_tx "$STORE1" "$CLAIM_TX"
STATUS_AFTER=$($BRAID_BIN status -p "$STORE1" -q 2>&1)
HAS_FS4=$(echo "$STATUS_AFTER" | grep -c "F(S)" || true)
check "adding claims: status still produces F(S)" "$([ "$HAS_FS4" -ge 1 ] && echo 0 || echo 1)"

# ── Check 11: Adding evidence for claims ──
echo "--- Check 11: Add evidence entities ---"
EVIDENCE_TX="$TMPDIR/evidence-tx.edn"
cat > "$EVIDENCE_TX" <<'EVIDENCETX'
{:tx/id #hlc "999999999/2/aabb0000000000000000000000000001"
 :tx/agent "aabb0000000000000000000000000001"
 :tx/provenance :observed
 :tx/rationale "E2E: add evidence covering claims"
 :tx/causal-predecessors []
 :datoms [
   {:e #blake3 "dd00000000000000000000000000000000000000000000000000000000000001" :a :evidence/covers :v #blake3 "cc00000000000000000000000000000000000000000000000000000000000001" :op :assert}
   {:e #blake3 "dd00000000000000000000000000000000000000000000000000000000000002" :a :evidence/covers :v #blake3 "cc00000000000000000000000000000000000000000000000000000000000002" :op :assert}
 ]}
EVIDENCETX
inject_tx "$STORE1" "$EVIDENCE_TX"
STATUS_AFTER2=$($BRAID_BIN status -p "$STORE1" -q 2>&1)
HAS_FS5=$(echo "$STATUS_AFTER2" | grep -c "F(S)" || true)
check "adding evidence: status still produces F(S)" "$([ "$HAS_FS5" -ge 1 ] && echo 0 || echo 1)"

# ── Check 12: Invalid policy (negative weight) loads with warning ──
echo "--- Check 12: Invalid policy warning ---"
STORE4="$TMPDIR/store4/.braid"
INVALID_MANIFEST="$TMPDIR/invalid-policy.edn"
cat > "$INVALID_MANIFEST" <<'INVALID'
{:tx/id #hlc "777777777/0/aabb0000000000000000000000000003"
 :tx/agent "aabb0000000000000000000000000003"
 :tx/provenance :observed
 :tx/rationale "E2E test: invalid policy (negative weight)"
 :tx/causal-predecessors []
 :datoms [
   {:e #blake3 "ee00000000000000000000000000000000000000000000000000000000000001" :a :policy/boundary-name :v "bad-boundary" :op :assert}
   {:e #blake3 "ee00000000000000000000000000000000000000000000000000000000000001" :a :policy/boundary-source :v ":bad/*" :op :assert}
   {:e #blake3 "ee00000000000000000000000000000000000000000000000000000000000001" :a :policy/boundary-target :v ":worse/*" :op :assert}
   {:e #blake3 "ee00000000000000000000000000000000000000000000000000000000000001" :a :policy/boundary-weight :v -0.5 :op :assert}
 ]}
INVALID
mkdir -p "$TMPDIR/store4"
cd "$TMPDIR/store4"
INVALID_OUT=$($BRAID_BIN init -p "$STORE4" --manifest "$INVALID_MANIFEST" -q 2>&1) || true
HAS_WARN=$(echo "$INVALID_OUT" | grep -c "warn" || true)
check "invalid policy: loads with warning" "$([ "$HAS_WARN" -ge 1 ] && echo 0 || echo 1)" "$INVALID_OUT"

# ── Check 13: Store with policy still accepts observations ──
echo "--- Check 13: Observe with policy store ---"
cd "$TMPDIR/store1"
OBS_OUT=$($BRAID_BIN observe "E2E test observation" --confidence 0.8 -p "$STORE1" -q 2>&1) || true
HAS_OBS=$(echo "$OBS_OUT" | grep -c "observed:" || true)
check "observe on policy store: accepted" "$([ "$HAS_OBS" -ge 1 ] && echo 0 || echo 1)" "$OBS_OUT"

# ── Check 14: Performance — status < 5s on policy store ──
echo "--- Check 14: Performance ---"
START_TIME=$(date +%s%N)
$BRAID_BIN status -p "$STORE1" -q > /dev/null 2>&1 || true
END_TIME=$(date +%s%N)
ELAPSED_MS=$(( (END_TIME - START_TIME) / 1000000 ))
check "performance: status < 5s (${ELAPSED_MS}ms)" "$([ "$ELAPSED_MS" -lt 5000 ] && echo 0 || echo 1)"

# ── Check 15: Missing manifest file produces error ──
echo "--- Check 15: Missing manifest error ---"
STORE5="$TMPDIR/store5/.braid"
mkdir -p "$TMPDIR/store5"
cd "$TMPDIR/store5"
MISSING_OUT=$($BRAID_BIN init -p "$STORE5" --manifest "/nonexistent/manifest.edn" -q 2>&1) || true
HAS_ERR=$(echo "$MISSING_OUT" | grep -ci "not found\|error" || true)
check "missing manifest: error reported" "$([ "$HAS_ERR" -ge 1 ] && echo 0 || echo 1)" "$MISSING_OUT"

# ── Summary ──
echo ""
echo "=== Results ==="
echo "PASS: $PASS"
echo "FAIL: $FAIL"
echo "TOTAL: $TOTAL"
if [ "$FAIL" -eq 0 ]; then
    echo "STATUS: ALL PASSED"
    exit 0
else
    echo "STATUS: FAILED"
    exit 1
fi
