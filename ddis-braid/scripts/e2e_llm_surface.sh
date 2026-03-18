#!/usr/bin/env bash
# E2E LLM Surface Validation — Task 12 (t-8e0d)
#
# Validates all outputs pass prompt-optimization quality checks.
# Traces to: INV-INTERFACE-008, INV-INTERFACE-010
#
# Usage: ./scripts/e2e_llm_surface.sh [braid-binary] [store-path]
#
# Exit codes: 0 = all pass, 1 = failures found

set -euo pipefail

BRAID="${1:-./target/release/braid}"
STORE="${2:-.braid}"
PASS=0
FAIL=0
TOTAL=0

check() {
    local name="$1"
    local condition="$2"
    TOTAL=$((TOTAL + 1))
    if eval "$condition"; then
        echo "[PASS] $name"
        PASS=$((PASS + 1))
    else
        echo "[FAIL] $name"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== E2E LLM Surface Validation ==="
echo "Binary: $BRAID"
echo "Store:  $STORE"
echo ""

# Verify binary exists
if [ ! -x "$BRAID" ]; then
    echo "[FATAL] Binary not found: $BRAID"
    exit 1
fi

# ── Test 1: MCP parity ──────────────────────────────────────────────
# Every CLI agent-mode command has a corresponding MCP tool.
echo "--- Test 1: MCP tool parity ---"
MCP_TOOLS=$(printf '%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  | timeout 5 $BRAID mcp serve 2>/dev/null | grep -o '"name":"braid_[^"]*"' | sort || true)
check "MCP: braid_status tool exists" "echo '$MCP_TOOLS' | grep -q 'braid_status'"
check "MCP: braid_query tool exists" "echo '$MCP_TOOLS' | grep -q 'braid_query'"
check "MCP: braid_harvest tool exists" "echo '$MCP_TOOLS' | grep -q 'braid_harvest'"
check "MCP: braid_seed tool exists" "echo '$MCP_TOOLS' | grep -q 'braid_seed'"
check "MCP: braid_observe tool exists" "echo '$MCP_TOOLS' | grep -q 'braid_observe'"
check "MCP: braid_task_ready tool exists" "echo '$MCP_TOOLS' | grep -q 'braid_task_ready'"
check "MCP: braid_task_go tool exists" "echo '$MCP_TOOLS' | grep -q 'braid_task_go'"
check "MCP: braid_task_close tool exists" "echo '$MCP_TOOLS' | grep -q 'braid_task_close'"
check "MCP: braid_task_create tool exists" "echo '$MCP_TOOLS' | grep -q 'braid_task_create'"

# ── Test 2: MCP orientation ─────────────────────────────────────────
echo ""
echo "--- Test 2: MCP orientation in initialize ---"
INIT_RESPONSE=$(printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}' \
  | timeout 5 $BRAID mcp serve 2>/dev/null || true)
check "MCP: initialize has instructions" "echo '$INIT_RESPONSE' | grep -q 'instructions'"
check "MCP: instructions mentions Braid" "echo '$INIT_RESPONSE' | grep -q 'Braid'"

# ── Test 3: Guidance footer presence ────────────────────────────────
echo ""
echo "--- Test 3: Guidance footer in CLI output ---"
STATUS_OUT=$($BRAID status 2>&1 || true)
check "CLI: status has M(t) footer" "echo '$STATUS_OUT' | grep -q 'M(t):'"
check "CLI: footer has sub-metric hints" "echo '$STATUS_OUT' | grep -qE '✗→(write|trace|query|harvest)|✓|△'"

# ── Test 4: Seed basin activation ───────────────────────────────────
echo ""
echo "--- Test 4: Seed output structure ---"
SEED_OUT=$($BRAID seed --task "test task" --seed-budget 2000 2>&1 || true)
check "Seed: starts with Orientation" "echo '$SEED_OUT' | grep -q '## Orientation'"
check "Seed: no Key attrs parasitic content" "! echo '$SEED_OUT' | grep -q 'Key attrs:'"
check "Seed: has Protocol line" "echo '$SEED_OUT' | grep -q 'Protocol:'"

# ── Test 5: Status progressive disclosure ───────────────────────────
echo ""
echo "--- Test 5: Status progressive disclosure ---"
# Use --format human to see verbose content (default mode is Agent in pipes)
TERSE_HUMAN=$($BRAID status --format human 2>&1 || true)
TERSE_LEN=$(echo "$TERSE_HUMAN" | wc -c)
VERBOSE_HUMAN=$($BRAID status --verbose --format human 2>&1 || true)
VERBOSE_LEN=$(echo "$VERBOSE_HUMAN" | wc -c)
check "Status: verbose > terse (human mode)" "[ $VERBOSE_LEN -gt $TERSE_LEN ]"
check "Status: verbose has F(S) formula" "echo '$VERBOSE_HUMAN' | grep -q 'F(S)'"
check "Status: verbose has Weakest line" "echo '$VERBOSE_HUMAN' | grep -qiE 'weakest|lowest'"

# ── Test 6: R18 braid go format ─────────────────────────────────────
echo ""
echo "--- Test 6: R18 action uses braid go ---"
STATUS_JSON=$($BRAID status --format json 2>&1 || true)
check "R18: actions use braid go" "echo '$STATUS_JSON' | grep -q 'braid go'"
check "R18: no braid task update in actions" "! echo '$STATUS_JSON' | grep -q 'braid task update.*in-progress'"

# ── Test 7: Help text efficiency ────────────────────────────────────
echo ""
echo "--- Test 7: Help text efficiency ---"
HARVEST_HELP=$($BRAID harvest --help 2>&1 || true)
check "Help: harvest --help no full --budget desc" "! echo '$HARVEST_HELP' | grep -q 'Token budget for output'"

# ── Test 8: Observe disambiguation ──────────────────────────────────
echo ""
echo "--- Test 8: Observe vs write assert disambiguation ---"
OBSERVE_HELP=$($BRAID observe --help 2>&1 || true)
ASSERT_HELP=$($BRAID write assert --help 2>&1 || true)
check "Observe help: mentions write assert" "echo '$OBSERVE_HELP' | grep -q 'write assert'"
check "Assert help: mentions braid observe" "echo '$ASSERT_HELP' | grep -q 'braid observe'"

# ── Test 9: Parasitic content removal ───────────────────────────────
echo ""
echo "--- Test 9: Parasitic content removal ---"
check "Seed: no :impl/ entity idents in state" "! echo '$SEED_OUT' | grep -v 'Session' | grep -q ':impl/'"

# ── Summary ─────────────────────────────────────────────────────────
echo ""
echo "=== Results: $PASS/$TOTAL passed, $FAIL failed ==="

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
exit 0
