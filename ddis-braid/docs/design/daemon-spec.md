# Braid Daemon — Persistent Store Process Design Specification

> **Status**: Design proposal (T2-5)
> **Traces to**: ADR-INTERFACE-004, INV-INTERFACE-001
> **Depends on**: T2-1 (redundant load elimination), T2-2 (incremental transact), T2-3 (cached indexes)

## Problem

Every `braid` CLI invocation loads the full store from disk, rebuilds 6 indexes,
performs one operation, and exits. At 23,000+ datoms, this takes ~200-500ms per
invocation (with index caching from T2-3, down from ~1500ms). For rapid command
sequences (closing 10 tasks, checking status, harvesting), the cumulative latency
is noticeable.

## Proposal

A long-running `braid daemon` process that holds the store in memory and serves
CLI commands via Unix domain socket IPC.

## Architecture

```
┌──────────┐     Unix socket      ┌──────────────┐
│ braid    │ ◄──────────────────► │ braid daemon │
│ (CLI)    │   JSON-RPC 2.0      │ (long-lived) │
└──────────┘                      └──────┬───────┘
                                         │
                                    ┌────┴────┐
                                    │  Store  │ (in-memory, 6 indexes)
                                    │ 23k+    │
                                    │ datoms  │
                                    └────┬────┘
                                         │ inotify
                                    ┌────┴────┐
                                    │ .braid/ │ (disk, txn files)
                                    │ txns/   │
                                    └─────────┘
```

## IPC Protocol

JSON-RPC 2.0 over Unix domain socket at `.braid/.daemon.sock`.

### Request format

```json
{"jsonrpc": "2.0", "id": 1, "method": "run", "params": {"args": ["task", "ready"]}}
```

### Response format

```json
{"jsonrpc": "2.0", "id": 1, "result": {"json": {...}, "human": "...", "agent": {...}}}
```

### Methods

| Method | Params | Description |
|--------|--------|-------------|
| `run` | `args: Vec<String>` | Execute a braid command |
| `status` | none | Daemon health check |
| `reload` | none | Force store reload from disk |
| `shutdown` | none | Graceful shutdown |

## Lifecycle

```bash
braid daemon start    # Fork background process, create socket
braid daemon stop     # Send shutdown, remove socket
braid daemon status   # Check if running (socket exists + responds)
```

The daemon writes its PID to `.braid/.daemon.pid` for process management.

## CLI Auto-Detection

When the CLI starts, it checks for `.braid/.daemon.sock`:
1. If socket exists and responds to `status`: send command via IPC (fast path, ~5ms)
2. If socket exists but unresponsive: remove stale socket, fall back to direct load
3. If no socket: direct file load (current behavior)

This is transparent — the user experience is identical, just faster.

## Cache Invalidation

The daemon watches `.braid/txns/` via `inotify` (Linux) or `kqueue` (macOS).
When a new `.edn` file appears:
1. Parse the new transaction file
2. Call `Store::transact()` to incrementally update the in-memory store
3. Update the transaction fingerprint

This handles multi-agent scenarios: another agent writes a transaction,
the daemon picks it up automatically.

## Security Considerations

- Socket is in `.braid/` (project-local, not world-accessible)
- Socket permissions: owner-only (0600)
- No authentication beyond filesystem permissions
- Daemon runs as the invoking user (no privilege escalation)

## Implementation Phases

1. **Phase 1**: Basic daemon with `start/stop/status` and `run` method
2. **Phase 2**: inotify-based cache invalidation for multi-agent
3. **Phase 3**: MCP server integration (daemon IS the MCP server)

Phase 3 unifies the daemon with ADR-INTERFACE-004 (library-mode MCP server).
The daemon becomes the MCP transport layer, serving both CLI and MCP clients
through the same in-memory store.

## Performance Targets

| Operation | Current (no daemon) | With daemon |
|-----------|-------------------|-------------|
| `braid status` | ~200ms | ~5ms |
| `braid task close` | ~300ms | ~10ms |
| `braid task ready` | ~200ms | ~5ms |
| `braid harvest --commit` | ~2s | ~1.5s (IO-bound) |

## Alternatives Rejected

1. **Shared memory / mmap**: Complex, platform-specific, doesn't handle index structures
2. **SQLite backend**: Violates ADR-STORE-017 (datom store over relational DB)
3. **Redis/external store**: Adds deployment complexity, violates self-contained principle
4. **Always-on MCP server**: Too heavyweight for CLI-only workflows; daemon is simpler
