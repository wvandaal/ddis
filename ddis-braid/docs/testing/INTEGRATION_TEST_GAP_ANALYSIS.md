# Integration Test Gap Analysis — DAEMON-SCALE + Full System

> Generated from primary source analysis of all invariants, code paths, and existing tests.
> References: daemon.rs (3431 LOC), wal.rs (1459 LOC), live_store.rs (1526 LOC),
> layout.rs (1572 LOC), mcp.rs (1796 LOC), 9 integration test files, 29 E2E scripts.

---

## 1. Existing Test Coverage Summary

### Unit Tests (in-source)
| Module | Tests | Key Coverage |
|--------|-------|-------------|
| daemon.rs | 66 | RwLock, commit handle, checkpoint, runtime datoms, scale |
| wal.rs | 21 | Round-trip, chain hash, corruption, concurrent, recovery |
| live_store.rs | 29 | Open, write, flush, refresh, crash recovery, DW0b guard |
| layout.rs | 12 | Init, write_tx, content hash, integrity, cache |
| mcp.rs | 18 | All 11 tools, Datalog, protocol, tool count |

### Integration Tests (tests/)
| File | Tests | Key Coverage |
|------|-------|-------------|
| daemon_write.rs | 9 | DW0 fingerprint, DW0b flush guard, iron test, marshal |

### E2E Scripts (scripts/)
| Script | Coverage |
|--------|----------|
| e2e_daemon.sh | Basic daemon start/stop, status query via socket |
| e2e_daemon_write.sh | Write through daemon, verify persistence |
| e2e_livestore.sh | LiveStore persistence, crash recovery |

---

## 2. Invariant-to-Test Mapping

### TESTED (has at least one dedicated test)
| Invariant | Test Location | Confidence |
|-----------|---------------|------------|
| C1 (append-only) | live_store::monotonic_growth, proptest | HIGH |
| C4 (CRDT merge) | live_store::commutativity_of_apply, stateright | HIGH |
| INV-STORE-020 (flush=fold) | live_store::store_bin_matches_full_rebuild | HIGH |
| INV-STORE-021 (deferred flush) | live_store::flush_writes_cache_and_clears_dirty | HIGH |
| INV-LAYOUT-001 (content hash) | daemon_write::fingerprint_uses_known_hashes | HIGH |
| INV-DAEMON-003 (runtime datoms) | daemon::handle_with_observation_emits_datoms | MEDIUM |
| DS1-001..004 (WAL integrity) | wal::21 tests | HIGH |
| DS5 (crash recovery) | live_store::8 recovery tests | HIGH |

### UNTESTED (no dedicated integration test)
| Invariant | Source | Why Critical |
|-----------|--------|-------------|
| **INV-DAEMON-001** (single daemon) | daemon.rs:28 | Lock file uniqueness prevents data corruption |
| **INV-DAEMON-002** (refresh before dispatch) | daemon.rs:29 | Stale reads in multi-process environments |
| **INV-DAEMON-004** (semantic equivalence) | daemon.rs:31 | Direct mode != daemon mode = silent bugs |
| **INV-DAEMON-005** (stale lock recovery) | daemon.rs:32 | Daemon fails to start after crash |
| **INV-DAEMON-006** (graceful shutdown) | daemon.rs:33 | Data loss on SIGTERM |
| **INV-DAEMON-007** (auto-detect fallback) | daemon.rs:35 | CLI hangs when daemon unavailable |
| **INV-DAEMON-011** (idle timeout) | daemon.rs:34 | Daemon leaks resources |
| **INV-DAEMON-012** (non-blocking accept) | daemon.rs:34 | Second client blocked by first |
| **INV-DS2-001** (commit linearizable) | daemon.rs | Group commit returns before durable |
| **DS3-001** (passive no truncate) | daemon.rs | WAL truncated during passive = data loss |
| **DS3-002** (full checkpoint) | daemon.rs | harvest --commit returns before .edn ready |
| **DS6** (integration wiring) | daemon.rs | Checkpoint not triggered on harvest --commit |
| **INV-STORE-022** (no stale cache) | live_store.rs | Concurrent processes overwrite each other's cache |

---

## 3. Code Paths Without Integration Tests

### Path A: CLI → Daemon Socket → Tool Dispatch → Response
**No integration test sends a real JSON-RPC request over a Unix socket and verifies the response.**
- marshal_command tested in unit tests (correct JSON generation)
- MCP tools tested in unit tests (correct execution)
- But the full path (connect → send → dispatch → observe → respond) is NEVER tested

### Path B: Daemon Auto-Start (INV-DAEMON-011)
**No test verifies that `try_route_through_daemon` forks a daemon when the socket is missing.**
- The fork + poll + retry logic is ~40 LOC with 3 timing-sensitive race conditions
- The fallback to direct mode is untested for the timeout case

### Path C: Multi-Connection Interleaving
**No test sends requests from multiple concurrent socket connections.**
- DS4 tests use `Arc<RwLock<LiveStore>>` directly (bypass socket layer)
- The thread-per-connection spawn, per-request lock, and request counting are untested at integration level

### Path D: Harvest --Commit → Full Checkpoint (DS6)
**No test verifies that `braid harvest --commit` through the daemon triggers CheckpointSignal::Full.**
- DS3 unit tests verify checkpoint_thread receives signals
- But the full path (CLI → daemon → tool_harvest → detect commit flag → send Full signal → wait for .edn conversion) is untested

### Path E: Daemon Shutdown Sequence
**No test verifies the shutdown sequence preserves all state.**
- Stop checkpoint thread (final passive checkpoint)
- Acquire write lock (handle poisoning)
- Flush LiveStore
- Remove socket + lock files (CleanupGuard)

### Path F: Cross-Process Write Visibility
**No test has two separate braid processes writing simultaneously and verifying mutual visibility.**
- daemon_write.rs:sequential_writes_with_external_interleave tests ONE process writing + ONE checking
- But two actual `braid observe` processes running in parallel is untested

---

## 4. Complete Test Case Catalog

### Category 1: Daemon Lifecycle (12 tests)

1.1 `daemon_start_creates_socket_and_lock` — Start daemon, verify .braid/daemon.sock and daemon.lock exist
1.2 `daemon_start_writes_pid_to_lock` — Lock file contains running daemon's PID
1.3 `daemon_start_installs_runtime_schema` — After start, store has :runtime/* attributes
1.4 `daemon_start_runs_capability_census` — After start, store has :capability/* datoms
1.5 `daemon_stop_removes_socket_and_lock` — After stop, both files removed
1.6 `daemon_stop_flushes_store` — After stop, store.bin is fresh (fingerprint matches)
1.7 `daemon_stop_sends_checkpoint_stop` — After stop, pending WAL entries are checkpointed
1.8 `daemon_second_instance_blocked` — Starting second daemon returns LockHeld error
1.9 `daemon_stale_lock_recovered` — Lock with dead PID auto-recovered on start
1.10 `daemon_idle_timeout_self_terminates` — Daemon exits after idle period, cleanup runs
1.11 `daemon_signal_sigterm_graceful_shutdown` — SIGTERM triggers flush + cleanup
1.12 `daemon_open_with_wal_on_restart` — After crash, daemon uses WAL recovery

### Category 2: Socket Communication (10 tests)

2.1 `socket_initialize_handshake` — Send "initialize" method, get capabilities response
2.2 `socket_tools_list_returns_11_tools` — Send "tools/list", verify 11 tool names
2.3 `socket_ping_returns_empty` — Send "ping", get empty result
2.4 `socket_unknown_method_returns_error` — Send "invalid/method", get METHOD_NOT_FOUND
2.5 `socket_malformed_json_returns_parse_error` — Send garbage, get -32700 error
2.6 `socket_status_returns_store_metrics` — tools/call braid_status, verify datom_count/entity_count
2.7 `socket_query_returns_matching_datoms` — tools/call braid_query with entity filter
2.8 `socket_daemon_status_returns_uptime` — daemon/status method returns pid, uptime, request_count
2.9 `socket_daemon_shutdown_stops_daemon` — daemon/shutdown method sets stop flag
2.10 `socket_notification_no_response` — Send notification (no id), verify no response

### Category 3: Tool Dispatch Through Socket (11 tests)

3.1 `socket_observe_persists_datom` — braid_observe through socket, query verifies datom exists
3.2 `socket_write_assert_persists` — braid_write through socket, query verifies
3.3 `socket_harvest_returns_candidates` — braid_harvest through socket
3.4 `socket_seed_returns_context` — braid_seed through socket
3.5 `socket_task_create_returns_id` — braid_task_create through socket
3.6 `socket_task_go_claims_task` — braid_task_go through socket
3.7 `socket_task_close_marks_done` — braid_task_close through socket
3.8 `socket_task_ready_returns_ranked` — braid_task_ready through socket
3.9 `socket_guidance_returns_methodology` — braid_guidance through socket
3.10 `socket_query_datalog_evaluates` — braid_query with datalog through socket
3.11 `socket_observe_then_query_visible` — Write through socket, read back immediately

### Category 4: Runtime Observation (7 tests)

4.1 `runtime_datom_emitted_on_success` — After tools/call, store has :runtime/outcome "success"
4.2 `runtime_datom_emitted_on_error` — After failed tools/call, :runtime/outcome "error"
4.3 `runtime_latency_us_positive` — :runtime/latency-us > 0 for any tool call
4.4 `runtime_command_matches_tool_name` — :runtime/command = tool name for each call
4.5 `runtime_request_id_matches` — :runtime/request-id matches JSON-RPC id
4.6 `runtime_cache_hit_true_when_no_external` — :runtime/cache-hit = true when no external writes
4.7 `runtime_datom_count_pre_request` — :runtime/datom-count records pre-request state

### Category 5: Semantic Equivalence INV-DAEMON-004 (6 tests)

5.1 `equivalence_status_daemon_vs_direct` — braid status output identical through daemon and direct
5.2 `equivalence_observe_daemon_vs_direct` — braid observe produces same datoms
5.3 `equivalence_query_daemon_vs_direct` — Same query returns same results
5.4 `equivalence_harvest_daemon_vs_direct` — Same harvest candidates
5.5 `equivalence_task_create_daemon_vs_direct` — Same task entity created
5.6 `equivalence_seed_daemon_vs_direct` — Same seed context assembled

### Category 6: Multi-Connection Concurrency (8 tests)

6.1 `concurrent_2_connections_no_deadlock` — Two socket clients send requests simultaneously
6.2 `concurrent_5_connections_all_succeed` — Five clients each send 3 requests
6.3 `concurrent_write_read_visibility` — Client A writes, Client B reads → visible
6.4 `concurrent_writes_all_persisted` — 5 clients each write 1 datom → all 5 in store
6.5 `concurrent_request_count_accurate` — After N requests from M connections, daemon/status shows N
6.6 `connection_disconnect_mid_request` — Client disconnects → daemon continues serving others
6.7 `rapid_connect_disconnect_no_leak` — 50 connect/disconnect cycles → no socket leak
6.8 `long_running_request_doesnt_block_accept` — Slow tool call doesn't prevent new connections

### Category 7: Checkpoint Integration DS3/DS6 (8 tests)

7.1 `passive_checkpoint_converts_wal_entries` — After daemon writes, timer-triggered checkpoint creates .edn files
7.2 `passive_checkpoint_does_not_truncate_wal` — WAL file size unchanged after passive checkpoint
7.3 `full_checkpoint_truncates_wal` — harvest --commit through daemon truncates WAL to 0
7.4 `full_checkpoint_all_edn_present` — After full checkpoint, count(.edn) >= count(WAL entries)
7.5 `harvest_commit_triggers_full_checkpoint` — Verify DS6 wiring: harvest --commit sends Full signal
7.6 `checkpoint_after_shutdown` — Stop signal triggers final passive checkpoint
7.7 `checkpoint_idempotent` — Two passive checkpoints on same WAL entries = same .edn count
7.8 `checkpoint_thread_survives_wal_corruption` — Corrupt WAL entry doesn't crash checkpoint thread

### Category 8: WAL Integration (6 tests)

8.1 `daemon_writes_to_wal` — After daemon processes write, WAL has new entries
8.2 `wal_entries_match_store_datoms` — WAL entry datoms match what's in the store
8.3 `wal_survives_daemon_crash` — Kill daemon (SIGKILL), WAL entries recoverable
8.4 `wal_chain_hash_valid_after_daemon_session` — Full chain hash verification after N writes
8.5 `wal_and_edn_consistent_after_checkpoint` — After full checkpoint, WAL empty + .edn complete
8.6 `wal_recovery_on_daemon_restart` — Restart daemon, verify WAL entries applied to store

### Category 9: Cross-Process Coordination (7 tests)

9.1 `two_processes_write_no_data_loss` — Process A and B each write 5 datoms, all 10 visible
9.2 `cli_write_visible_to_daemon` — CLI direct write, daemon refresh sees it
9.3 `daemon_write_visible_to_cli` — Daemon write, CLI direct open sees it
9.4 `flush_guard_prevents_stale_overwrite` — Two processes flush, guard prevents subset write
9.5 `external_write_triggers_refresh` — External .edn file, daemon detects on next request
9.6 `concurrent_flush_no_corruption` — Two processes flush simultaneously, store.bin valid
9.7 `cache_fingerprint_mismatch_triggers_rebuild` — External write → CLI detects → full rebuild

### Category 10: Error Paths & Edge Cases (10 tests)

10.1 `socket_timeout_on_slow_tool` — Client times out, daemon handles gracefully
10.2 `invalid_tool_name_returns_error` — tools/call with unknown tool name → error
10.3 `missing_required_param_returns_error` — tools/call braid_query with no params → error
10.4 `daemon_lock_file_permission_denied` — Lock file in read-only dir → BindFailed
10.5 `socket_path_too_long` — Path exceeds 108-byte Unix socket limit → error
10.6 `store_bin_corrupt_on_open` — Corrupt store.bin → fallback to .edn rebuild
10.7 `empty_store_all_tools_succeed` — All 11 tools work on genesis-only store
10.8 `max_payload_wal_entry` — WAL entry near 4096 bytes (O_APPEND boundary)
10.9 `daemon_handles_rapid_start_stop` — Start/stop 3 times → no stale socket/lock
10.10 `read_timeout_closes_connection` — Idle connection after 30s → server closes

### Category 11: Performance & Regression (5 tests)

11.1 `status_through_daemon_under_100ms` — braid status via daemon < 100ms response time
11.2 `observe_through_daemon_under_50ms` — braid observe via daemon < 50ms
11.3 `10_sequential_requests_under_1s` — 10 tool calls complete within 1 second
11.4 `store_bin_not_written_per_request` — INV-STORE-021: store.bin mtime unchanged between requests
11.5 `daemon_memory_stable_after_100_requests` — No memory leak across 100 requests (RSS check)

---

## 5. Priority Matrix

### P0 — Must Have (blocks production confidence)
- Category 1: 1.1, 1.5, 1.6, 1.8, 1.9
- Category 2: 2.1, 2.2, 2.6
- Category 3: 3.1, 3.11
- Category 5: 5.1, 5.2
- Category 6: 6.1, 6.4
- Category 7: 7.3, 7.5
- Category 9: 9.1, 9.2, 9.3

### P1 — Should Have (important for robustness)
- Category 1: 1.2, 1.3, 1.7, 1.10, 1.11, 1.12
- Category 2: 2.3, 2.4, 2.5, 2.8, 2.9
- Category 3: 3.2 through 3.10
- Category 4: 4.1 through 4.7
- Category 6: 6.2, 6.3, 6.5
- Category 7: 7.1, 7.2, 7.4, 7.6
- Category 8: 8.1, 8.3, 8.6
- Category 9: 9.4, 9.5

### P2 — Nice to Have (edge cases, hardening)
- Category 6: 6.6, 6.7, 6.8
- Category 7: 7.7, 7.8
- Category 8: 8.2, 8.4, 8.5
- Category 9: 9.6, 9.7
- Category 10: all
- Category 11: all
