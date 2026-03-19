#!/usr/bin/env bash
# E2E test: topology quick plan (INV-TOPOLOGY-001..005, ADR-TOPOLOGY-004)
#
# Validates the full topology compilation pipeline from CLI invocation
# through kernel computation to output formatting.
#
# Usage: ./scripts/e2e_topology_quick.sh [--braid PATH]
#
# Traces to: spec/19-topology.md, QUICK-8 (t-ca4e98bd)

set -euo pipefail

BRAID="${1:-cargo run -q --}"
PASS=0
FAIL=0
TOTAL=0
# Use large budget to prevent output truncation in tests
BUDGET="--budget 50000 --format human"

pass() { PASS=$((PASS + 1)); TOTAL=$((TOTAL + 1)); echo "[PASS] $1"; }
fail() { FAIL=$((FAIL + 1)); TOTAL=$((TOTAL + 1)); echo "[FAIL] $1: $2"; }

echo "=== E2E Topology Quick Plan Tests ==="
echo ""

# Test 1: topology status command works
output=$($BRAID topology status -q $BUDGET 2>/dev/null) || true
if echo "$output" | grep -q "ready tasks"; then
    pass "Test 1: topology status outputs ready task count"
else
    fail "Test 1" "topology status missing 'ready tasks'"
fi

# Test 2: topology plan with --agents 2 produces output
output=$($BRAID topology plan --agents 2 -q $BUDGET 2>/dev/null) || true
if echo "$output" | grep -q "topology:"; then
    pass "Test 2: topology plan produces output header"
else
    fail "Test 2" "topology plan missing header"
fi

# Test 3: plan includes coupling entropy
if echo "$output" | grep -qE "S=|S\("; then
    pass "Test 3: plan includes coupling entropy"
else
    fail "Test 3" "missing coupling entropy in output"
fi

# Test 4: plan includes pattern classification
if echo "$output" | grep -qE "mesh|star|hybrid|solo|pipeline"; then
    pass "Test 4: plan includes pattern classification"
else
    fail "Test 4" "missing pattern classification"
fi

# Test 5: plan includes disjointness verification
if echo "$output" | grep -q "disjointness"; then
    pass "Test 5: plan includes disjointness verification"
else
    fail "Test 5" "missing disjointness check"
fi

# Test 6: JSON output is parseable
json_output=$($BRAID topology plan --agents 2 --json -q $BUDGET 2>/dev/null) || true
if echo "$json_output" | python3 -m json.tool > /dev/null 2>&1; then
    pass "Test 6: JSON output is valid JSON"
else
    fail "Test 6" "JSON output is not valid JSON"
fi

# Test 7: JSON includes required fields
if echo "$json_output" | grep -q '"parallelizability"' && \
   echo "$json_output" | grep -q '"coupling_entropy"' && \
   echo "$json_output" | grep -q '"disjointness_verified"'; then
    pass "Test 7: JSON includes required fields"
else
    fail "Test 7" "JSON missing required fields"
fi

# Test 8: --emit-seeds flag produces per-agent seeds
if $BRAID topology plan --agents 2 --emit-seeds -q $BUDGET 2>/dev/null | grep -q "Per-Agent Seeds"; then
    pass "Test 8: --emit-seeds produces per-agent seed prompts"
else
    fail "Test 8" "missing per-agent seeds in output"
fi

echo ""
echo "=== Results: $PASS/$TOTAL passed, $FAIL failed ==="

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
