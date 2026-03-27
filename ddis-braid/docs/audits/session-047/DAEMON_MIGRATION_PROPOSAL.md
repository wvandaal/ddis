# Daemon Write-Path Migration — Full Proposal

> **Date**: 2026-03-27
> **Status**: PROPOSAL — awaiting approval before implementation
> **Traces to**: INV-DAEMON-004 (semantic equivalence), INV-STORE-020 (cache = fold(txns)),
>   INV-STORE-022 (stale cache prevention), ADR-STORE-006 (embedded + daemon)
> **Blocks**: Stale cache resolution, sub-1s status, multi-agent correctness

---

## 1. Problem Statement

### The Stale Cache Bug (proven root cause)

Two bugs compound to create persistent stale cache reads:

**BUG 1**: `LiveStore::has_new_external_txns()` (live_store.rs:142-147) checks `txns/`
directory mtime. On Linux, creating a file in `txns/ab/` does NOT propagate mtime to
`txns/`. With all 257 shard directories existing, the check is **permanently false**.
The defensive flush guard in `flush()` never fires.

**BUG 2**: `write_slim_cache()` (layout.rs:569) computes `meta.json` fingerprint from
`list_tx_hashes()` at flush time — which includes .edn files the flushing LiveStore
never loaded. The fingerprint makes the stale cache APPEAR valid to the next reader.

**Combined effect**: When a write command runs (observe, task create, harvest, etc.):
1. main.rs opens LiveStore L1 (loads store.bin state S0)
2. Command opens LiveStore L2 (loads same S0), writes datoms, L2 drops → flushes store.bin = S1
3. main.rs RFL-2 post-hook writes action prediction to L1 → L1 dirty with S0 + rfl2
4. L1 drops → flush guard FALSE (BUG 1) → flushes store.bin = S0' (overwrites S1)
5. meta.json fingerprint includes ALL .edn files (BUG 2) → next reader: cache hit → loads stale S0'

### Why This Exists

The daemon was designed to be the architectural solution: one process, one LiveStore,
zero overlapping writers. It was built (D4-1 through D4-8, verified in DOGFOOD-1) and
works correctly for `status` and `query`. But **write commands were deferred** because
argument marshaling from CLI flags to JSON-RPC was not implemented. The comment at
daemon.rs:846-848 says:

```
// ONLY read-only commands that need no args are daemon-routable.
// Write commands (observe, harvest, task, spec) need argument marshaling
// which is not yet implemented — they use direct mode.
```

Every write command falls through to direct mode, creating overlapping LiveStores.

### The Key Insight

**The daemon already supports all write operations.** The MCP layer (mcp.rs:324-335)
dispatches 11 tools including `braid_observe`, `braid_harvest`, `braid_write`,
`braid_task_go`, `braid_task_close`, `braid_task_create`. The daemon's accept loop
(daemon.rs:1124) routes `tools/call` to these handlers. The gap is **purely in the
CLI → daemon routing**: two locations in daemon.rs need changes.

---

## 2. First-Principles Decomposition

### The Fundamental Invariant

**INV-STORE-020**: After flush, `store.bin = fold(txn_files)`.

This is a **confluence property**: regardless of how many processes write transactions,
the cache must converge to the same state as a fresh `Store::from_datoms(all_txn_datoms)`.
The current architecture violates this because multiple LiveStores can flush with partial
state, and BUG 2 masks the violation.

### The Algebraic Structure

The datom store is a G-Set CvRDT: `(P(D), ∪)`. Set union is commutative, associative,
and idempotent. This means the **order** in which transactions are applied doesn't matter —
only the **completeness** of the set matters. The cache violation is not about ordering;
it's about **subset**: a stale flush writes a proper subset of the correct datom set.

### The Architectural Theorem

**Theorem**: If exactly one process owns the Store at any time, and that process applies
all transactions before flushing, then INV-STORE-020 holds by construction.

**Proof sketch**: Let P be the owner process. P loads `fold(txn_files_at_open)`. P applies
all transactions written through it: `state = fold(txn_files_at_open) ∪ new_txns`. If no
other process writes, `fold(all_txn_files) = state`. If another process writes to .edn files,
P's `refresh_if_needed()` applies them before flush: `state = fold(all_txn_files)`. QED.

The daemon IS this single-owner architecture. Completing the migration makes INV-STORE-020
hold by construction rather than by defensive guards that have proven brittle.

### Dependency Graph

```
BUG 1 (mtime) + BUG 2 (fingerprint) → Stale cache
  ↑ caused by: overlapping LiveStores
    ↑ caused by: write commands bypass daemon
      ↑ caused by: argument marshaling not implemented
        ↑ caused by: CLI→JSON-RPC mapping deferred at daemon MVP
```

The root cause is the lowest node. Everything above is a symptom.

---

## 3. The Proposal: Complete Daemon Write-Path Migration

### What Changes

**Two files, ~150 LOC total:**

#### Change 1: Argument marshaling in `try_route_through_daemon()` (daemon.rs)

Replace the hard-coded 2-command match with a complete CLI-to-JSON-RPC mapping:

```rust
// BEFORE (daemon.rs:849-852):
let tool_name = match cmd_name {
    "status" => "braid_status",
    "query" => "braid_query",
    _ => return None,
};

// AFTER:
let (tool_name, arguments) = match marshal_command(cmd_name, cmd_args) {
    Some(mapped) => mapped,
    None => return None, // Truly un-routable (init, daemon, mcp, shell)
};
```

The `marshal_command()` function maps each CLI command name + its arguments to
`(tool_name: &str, arguments: JsonValue)`:

| CLI command | MCP tool | Arguments to marshal |
|---|---|---|
| `status` | `braid_status` | `{}` (no args) |
| `query` | `braid_query` | `{datalog?, entity?, attribute?}` |
| `observe` | `braid_observe` | `{text, confidence?, category?, relates_to?, rationale?, alternatives?}` |
| `harvest` | `braid_harvest` | `{task?, knowledge?, commit?}` |
| `seed` | `braid_seed` | `{task?, budget?}` |
| `task ready` | `braid_task_ready` | `{}` |
| `task go` / `go` / `next` | `braid_task_go` | `{id}` |
| `task close` / `done` | `braid_task_close` | `{id, reason?}` |
| `task create` | `braid_task_create` | `{title, priority?, task_type?}` |
| `write assert` | `braid_write` | `{entity, attribute, value, rationale?}` |
| `guidance` | `braid_guidance` | `{}` |

Commands that remain direct-mode only (not daemon-routable):
- `init` — creates the store; daemon requires an existing store
- `daemon` — controls the daemon itself
- `mcp` — IS the MCP server (separate process)
- `shell` — interactive REPL (needs persistent connection)
- `trace` — reads source files (filesystem access the daemon shouldn't do)
- `bilateral` — heavy computation better done by caller for now
- `merge` — inter-store operation (two stores involved)

#### Change 2: Pass CLI arguments to `try_route_through_daemon()` (main.rs)

Currently main.rs:116 passes `&serde_json::json!({})`. Change to pass the
parsed `Cli` struct or a serialized representation of the command's arguments.

The cleanest approach: serialize the `Command` enum variant's fields to JSON
before the daemon routing check. This is a ~50 LOC function that matches on
`Command::Observe { text, confidence, .. }` → `json!({"text": text, "confidence": confidence})`,
etc.

### Why This Is Optimal

1. **Root cause elimination**: Overlapping LiveStores are eliminated for all daemon-routable
   commands. The daemon holds the single LiveStore. The CLI becomes a thin JSON-RPC client.

2. **Zero new architecture**: No new abstractions, no new types, no new protocols. The daemon,
   the MCP tools, the tool dispatch, and the socket transport all exist and are tested. We are
   wiring two existing endpoints together.

3. **Performance positive**: CLI no longer loads store.bin for write commands (~300ms saved).
   The daemon's warm in-memory store serves the request immediately.

4. **Defense in depth**: Even if the daemon isn't running (fallback to direct mode), BUG 2
   should also be fixed so staleness self-corrects. See "Complementary Fix" below.

### Complementary Fix: Fingerprint from known_hashes (BUG 2)

Even with the daemon migration, direct mode remains as a fallback. BUG 2 should be fixed
independently so that direct-mode staleness is transient:

In `write_slim_cache()` (layout.rs:569), replace:
```rust
let hashes = self.list_tx_hashes()?;
```
with a parameter passed from the caller:
```rust
pub fn write_slim_cache(&self, store: &Store, known_hashes: &[String]) -> Result<(), BraidError> {
    // ...
    let fingerprint = self.txn_fingerprint(known_hashes);
    // ...
}
```

LiveStore's `flush()` passes `self.known_hashes` (the hashes it actually loaded + wrote).
If the fingerprint doesn't include external hashes, the next reader sees a mismatch and
rebuilds correctly. Staleness becomes one-command transient, not persistent.

### Verification Sketch

1. **INV-STORE-020**: After daemon flush, `store.bin = fold(txn_files)`.
   - **Test**: Write 10 observations through daemon. Stop daemon (flush). Load store.bin
     directly. Verify datom count = sum of all .edn file datoms.
   - **Proptest**: For any sequence of CLI commands through daemon, `store.bin` datom set
     equals `Store::from_datoms(all_edn_datoms)`.

2. **INV-DAEMON-004**: Semantic equivalence between daemon mode and direct mode.
   - **Test**: For each routable command, run via daemon and via direct mode on the same
     store. Compare output text (modulo timing/formatting). Verify store state identical.

3. **Stale cache elimination**: Two concurrent agents cannot produce a stale cache.
   - **Test**: Agent A runs `braid observe "X"`. Agent B runs `braid observe "Y"` concurrently.
     Both route through daemon (serialized by socket accept). Third agent runs `braid status`.
     Both observations visible.

4. **Fallback correctness**: When daemon is not running, direct mode produces correct (if
   slow) results, and BUG 2 fix ensures staleness self-corrects on next load.
   - **Test**: Kill daemon. Run `braid observe "Z"` (direct mode). Run `braid status`.
     Observation Z visible.

5. **Performance**: Write commands through daemon complete in < 500ms (no store.bin load).
   - **Benchmark**: `time braid observe "test" -c 0.5` with daemon running < 0.5s.

### Risk / Tradeoffs

1. **Daemon dependency**: If the daemon crashes mid-request, the CLI gets no response and
   falls back to direct mode. The .edn file may or may not have been written. **Mitigation**:
   Direct mode as fallback handles this correctly (loads from .edn files).

2. **Auto-start latency**: First command after daemon timeout incurs ~1-3s for daemon
   startup (existing behavior). **Mitigation**: Already implemented in INV-DAEMON-011.
   Daemon idle timeout can be increased if this is annoying.

3. **Argument fidelity**: Some CLI flags (e.g., `--no-auto-crystallize`, `--force`,
   `--no-reconcile` on harvest) may not have MCP equivalents yet. **Mitigation**: For
   missing args, fall through to direct mode. Add MCP parameters incrementally.

4. **Debugging**: Errors from daemon-routed commands show daemon-side output, not CLI-side.
   Stack traces point to daemon.rs, not the command handler. **Mitigation**: Daemon wraps
   errors with the tool name and original parameters in the JSON-RPC error response.

---

## 4. Implementation Plan

### Wave 0: Fix BUG 2 independently (defense in depth)
- **File**: layout.rs — `write_slim_cache()` accepts `known_hashes: &[String]` parameter
- **File**: live_store.rs — `flush()` passes `self.known_hashes` to `write_slim_cache()`
- **Acceptance**: Stale flush produces fingerprint mismatch → next reader rebuilds
- **Effort**: ~20 LOC, 1 test

### Wave 1: Argument marshaling function
- **File**: daemon.rs — add `fn marshal_command(cmd_name: &str, cmd: &Command) -> Option<(&str, JsonValue)>`
- **Maps**: 11 CLI commands to MCP tool names + JSON arguments
- **Acceptance**: `marshal_command("observe", &Command::Observe{text:"X", confidence:0.8, ..})` returns `Some(("braid_observe", json!({"text":"X","confidence":0.8})))`
- **Effort**: ~80 LOC, 11 unit tests (one per command)

### Wave 2: Wire into main.rs
- **File**: main.rs — pass `&cmd` to `try_route_through_daemon()` instead of `&json!({})`
- **File**: daemon.rs — `try_route_through_daemon()` calls `marshal_command()` and uses result
- **Acceptance**: `braid observe "test"` routes through daemon when running
- **Effort**: ~20 LOC

### Wave 3: Remove main.rs post-hooks that dirty L1
- **File**: main.rs — RFL-2 action recording moves into the daemon's `handle_with_observation()`
  (which already does runtime datom emission). The CLI's post-hook RFL-2 at main.rs:201-256
  becomes a no-op when daemon-routed (the daemon already recorded the action).
- **Acceptance**: main.rs LiveStore stays clean for daemon-routed commands (flush is no-op)
- **Effort**: ~30 LOC (conditional skip + migration of RFL-2 logic)

### Wave 4: Verification
- **E2E test**: daemon running → all 11 commands produce correct output and store state
- **Concurrency test**: 3 agents running simultaneously → no stale cache
- **Fallback test**: daemon not running → all commands work in direct mode
- **Performance test**: write commands < 500ms through daemon
- **Effort**: ~100 LOC of tests

### Total: ~250 LOC across 4 files, 5 waves

### Quality Gates

| Gate | Criterion |
|------|-----------|
| Wave 0 → 1 | Stale flush produces fingerprint mismatch (test passes) |
| Wave 1 → 2 | All 11 marshal_command unit tests pass |
| Wave 2 → 3 | `braid observe "test"` routes through daemon (verified by daemon log) |
| Wave 3 → 4 | main.rs LiveStore stays clean for all daemon-routed commands |
| Wave 4 → DONE | All verification tests pass. `braid status` shows correct state after concurrent writes. |

---

## 5. What This Does NOT Address

- **O(N) datom scan performance** (audit PERF-002/PERF-005): The daemon's own computation
  is still 3s at 151K datoms. This is a separate optimization from the stale cache bug.
- **C8 substrate independence**: The policy manifest migration is orthogonal.
- **Multi-machine coordination**: The daemon is per-machine. Cross-machine sync requires
  the CRDT merge infrastructure (Stage 3).
