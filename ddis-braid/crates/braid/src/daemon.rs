//! Braid session daemon — INV-DAEMON-001..009, ADR-DAEMON-001..003.
//!
//! A Unix-socket daemon that holds a single [`LiveStore`] in memory,
//! serves JSON-RPC requests using the same protocol as the MCP server,
//! and emits reflexive `:runtime/*` datoms for every processed command.
//!
//! # Architecture (ADR-DAEMON-002)
//!
//! The daemon reuses the MCP tool dispatch from [`crate::mcp`]. It adds:
//! - Lifecycle management (lock file, signal handling, graceful shutdown)
//! - Unix socket transport (instead of stdin/stdout)
//! - Runtime datom emission (`handle_with_observation`)
//!
//! # Invariants
//!
//! - **INV-DAEMON-001**: At most one daemon per `.braid` directory.
//! - **INV-DAEMON-002**: Store always consistent with disk (`refresh_if_needed`).
//! - **INV-DAEMON-003**: Every command emits `:runtime/*` datoms.
//! - **INV-DAEMON-004**: Semantic equivalence with direct mode.
//! - **INV-DAEMON-005**: Stale lock recovery via `kill(pid, 0)`.
//! - **INV-DAEMON-006**: Graceful shutdown preserves all state.

use std::io::{BufRead, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value as JsonValue};

// ---------------------------------------------------------------------------
// DaemonError
// ---------------------------------------------------------------------------

/// Daemon-specific error type with structured recovery information.
///
/// Each variant maps to a distinct failure mode in the daemon lifecycle:
/// startup (lock/bind), runtime (store/protocol), shutdown (already stopping),
/// and client communication (connection/timeout).
#[derive(Debug)]
pub enum DaemonError {
    /// Another daemon instance holds the lock file.
    LockHeld { pid: u32 },
    /// Lock file exists but the owning process is dead. Auto-recoverable.
    LockStale { pid: u32 },
    /// Cannot bind the Unix domain socket.
    BindFailed(std::io::Error),
    /// Shutdown already in progress — duplicate stop request.
    AlreadyStopping,
    /// No daemon is running (stop/status on absent daemon).
    NotRunning,
    /// Client cannot connect to the daemon socket.
    ConnectionFailed(std::io::Error),
    /// Client connection or read timed out.
    Timeout,
    /// Underlying store operation failed.
    StoreError(crate::error::BraidError),
    /// Malformed JSON-RPC message.
    ProtocolError(String),
}

impl std::fmt::Display for DaemonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DaemonError::LockHeld { pid } => {
                write!(
                    f,
                    "error: daemon lock held\n  why: another daemon is running (pid {pid})\n  fix: stop the existing daemon with `braid daemon stop`\n  ref: ADR-STORE-006"
                )
            }
            DaemonError::LockStale { pid } => {
                write!(
                    f,
                    "error: stale daemon lock\n  why: lock file references dead process (pid {pid})\n  fix: the lock will be auto-recovered on next start\n  ref: ADR-STORE-006"
                )
            }
            DaemonError::BindFailed(e) => {
                write!(
                    f,
                    "error: cannot bind daemon socket\n  why: {e}\n  fix: check permissions on .braid/ and ensure no stale socket file exists\n  ref: ADR-STORE-006"
                )
            }
            DaemonError::AlreadyStopping => {
                write!(
                    f,
                    "error: daemon already stopping\n  why: a shutdown is already in progress\n  fix: wait for the current shutdown to complete\n  ref: ADR-STORE-006"
                )
            }
            DaemonError::NotRunning => {
                write!(
                    f,
                    "error: daemon not running\n  why: no daemon process is active\n  fix: start the daemon with `braid daemon start`\n  ref: ADR-STORE-006"
                )
            }
            DaemonError::ConnectionFailed(e) => {
                write!(
                    f,
                    "error: daemon connection failed\n  why: {e}\n  fix: verify the daemon is running with `braid daemon status`\n  ref: ADR-STORE-006"
                )
            }
            DaemonError::Timeout => {
                write!(
                    f,
                    "error: daemon request timed out\n  why: no response within the configured deadline\n  fix: check daemon health with `braid daemon status` or restart\n  ref: ADR-STORE-006"
                )
            }
            DaemonError::StoreError(e) => {
                write!(
                    f,
                    "error: daemon store error\n  why: {e}\n  fix: run `braid status` to diagnose store state\n  ref: ADR-STORE-006"
                )
            }
            DaemonError::ProtocolError(msg) => {
                write!(
                    f,
                    "error: daemon protocol error\n  why: {msg}\n  fix: ensure client sends valid JSON-RPC 2.0 messages\n  ref: ADR-STORE-006"
                )
            }
        }
    }
}

impl std::error::Error for DaemonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DaemonError::BindFailed(e) | DaemonError::ConnectionFailed(e) => Some(e),
            DaemonError::StoreError(e) => Some(e),
            DaemonError::LockHeld { .. }
            | DaemonError::LockStale { .. }
            | DaemonError::AlreadyStopping
            | DaemonError::NotRunning
            | DaemonError::Timeout
            | DaemonError::ProtocolError(_) => None,
        }
    }
}

impl From<std::io::Error> for DaemonError {
    fn from(e: std::io::Error) -> Self {
        DaemonError::ConnectionFailed(e)
    }
}

impl From<crate::error::BraidError> for DaemonError {
    fn from(e: crate::error::BraidError) -> Self {
        DaemonError::StoreError(e)
    }
}

// ---------------------------------------------------------------------------
// Newtypes — SocketPath, LockPath, RequestId
// ---------------------------------------------------------------------------

/// Type-safe wrapper for the daemon Unix socket path (.braid/daemon.sock).
#[derive(Debug, Clone)]
pub struct SocketPath(pub PathBuf);

impl SocketPath {
    /// Construct a socket path by appending `daemon.sock` to the store base.
    pub fn new(base: &Path) -> Self {
        Self(base.join("daemon.sock"))
    }

    /// Borrow the inner path.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// Type-safe wrapper for the daemon lock file path (.braid/daemon.lock).
#[derive(Debug, Clone)]
pub struct LockPath(pub PathBuf);

impl LockPath {
    /// Construct a lock path by appending `daemon.lock` to the store base.
    pub fn new(base: &Path) -> Self {
        Self(base.join("daemon.lock"))
    }

    /// Borrow the inner path.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// Type-safe wrapper for a JSON-RPC request ID.
///
/// The JSON-RPC 2.0 spec allows `id` to be a string, number, or null.
/// Wrapping it in a newtype prevents accidental use as a generic JSON value.
#[derive(Debug, Clone)]
pub struct RequestId(pub serde_json::Value);

// ---------------------------------------------------------------------------
// LockStatus
// ---------------------------------------------------------------------------

/// Result of inspecting the daemon lock file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockStatus {
    /// The lock file exists and the owning process (PID) is alive.
    Live(u32),
    /// The lock file exists but the owning process (PID) is dead.
    Stale(u32),
    /// No lock file present.
    Absent,
}

// ---------------------------------------------------------------------------
// Runtime schema — ADR-DAEMON-003, INV-DAEMON-003
// ---------------------------------------------------------------------------

/// Runtime schema attribute definitions.
///
/// Each tuple: (ident, value_type_keyword, cardinality_keyword, doc).
const RUNTIME_ATTRS: &[(&str, &str, &str, &str)] = &[
    (
        ":runtime/command",
        ":db.type/string",
        ":db.cardinality/one",
        "Command name or tool name processed by the daemon",
    ),
    (
        ":runtime/request-id",
        ":db.type/string",
        ":db.cardinality/one",
        "JSON-RPC request ID as string",
    ),
    (
        ":runtime/latency-ms",
        ":db.type/long",
        ":db.cardinality/one",
        "Wall clock milliseconds for request processing",
    ),
    (
        ":runtime/outcome",
        ":db.type/string",
        ":db.cardinality/one",
        "Request outcome: success or error",
    ),
    (
        ":runtime/datom-count",
        ":db.type/long",
        ":db.cardinality/one",
        "Store datom count at time of request",
    ),
    (
        ":runtime/cache-hit",
        ":db.type/boolean",
        ":db.cardinality/one",
        "Whether refresh_if_needed found no new transactions (O(1) fast path)",
    ),
];

/// Install runtime schema attributes into the store if not already present.
///
/// Idempotent: checks for `:runtime/command` existence before transacting.
/// Uses `live.write_tx()` for persistence (C3: schema-as-data).
///
/// **ADR-DAEMON-003**: Runtime attributes are schema datoms, not config.
pub fn install_runtime_schema(
    live: &mut crate::live_store::LiveStore,
) -> Result<(), DaemonError> {
    use braid_kernel::datom::*;
    use braid_kernel::layout::TxFile;

    // Idempotency check: if :runtime/command already has a :db/valueType datom,
    // the schema is already installed.
    let check_entity = EntityId::from_ident(":runtime/command");
    let value_type_attr = Attribute::from_keyword(":db/valueType");
    let already_installed = live
        .store()
        .entity_datoms(check_entity)
        .iter()
        .any(|d| d.attribute == value_type_attr && d.op == Op::Assert);

    if already_installed {
        return Ok(());
    }

    let agent = AgentId::from_name("braid:daemon");
    let tx_id = crate::commands::write::next_tx_id(live.store(), agent);

    let mut datoms = Vec::new();

    for &(ident, value_type, cardinality, doc) in RUNTIME_ATTRS {
        let entity = EntityId::from_ident(ident);

        // :db/ident
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(ident.to_string()),
            tx_id,
            Op::Assert,
        ));
        // :db/valueType
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/valueType"),
            Value::Keyword(value_type.to_string()),
            tx_id,
            Op::Assert,
        ));
        // :db/cardinality
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/cardinality"),
            Value::Keyword(cardinality.to_string()),
            tx_id,
            Op::Assert,
        ));
        // :db/doc
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String(doc.to_string()),
            tx_id,
            Op::Assert,
        ));
        // :db/resolutionMode — LWW for all runtime attributes
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/resolutionMode"),
            Value::Keyword(":db.resolution/lww".to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    let tx_file = TxFile {
        tx_id,
        agent,
        provenance: ProvenanceType::Derived,
        rationale: "D4-4: install runtime schema (ADR-DAEMON-003)".to_string(),
        causal_predecessors: vec![],
        datoms,
    };

    live.write_tx(&tx_file).map_err(DaemonError::from)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Lock file management — INV-DAEMON-001, INV-DAEMON-005
// ---------------------------------------------------------------------------

/// Acquire the daemon lock file atomically.
///
/// Creates `.braid/daemon.lock` with `O_CREAT | O_EXCL` semantics:
/// - If the file does not exist, creates it and writes the current PID.
/// - If the file exists and the owning PID is alive → `DaemonError::LockHeld`.
/// - If the file exists and the owning PID is dead → removes the stale lock
///   and retries (INV-DAEMON-005).
///
/// **INV-DAEMON-001**: At most one daemon per `.braid` directory.
pub fn acquire_lock(lock_path: &LockPath) -> Result<(), DaemonError> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let path = lock_path.path();

    // Attempt exclusive create.
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut f) => {
            // Write our PID.
            let pid = std::process::id();
            writeln!(f, "{pid}").map_err(DaemonError::BindFailed)?;
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Lock file exists — check if the owner is alive.
            match check_lock(lock_path) {
                LockStatus::Live(pid) => Err(DaemonError::LockHeld { pid }),
                LockStatus::Stale(pid) => {
                    // Remove stale lock and retry (INV-DAEMON-005).
                    eprintln!(
                        "daemon: removing stale lock (pid {pid} is dead)"
                    );
                    let _ = std::fs::remove_file(path);
                    // Recurse once. If this fails, surface the error.
                    acquire_lock(lock_path)
                }
                LockStatus::Absent => {
                    // Race: file disappeared between our open and check.
                    acquire_lock(lock_path)
                }
            }
        }
        Err(e) => Err(DaemonError::BindFailed(e)),
    }
}

/// Release the daemon lock file.
///
/// Removes `.braid/daemon.lock`. Silently ignores `NotFound` (idempotent).
pub fn release_lock(lock_path: &LockPath) {
    let _ = std::fs::remove_file(lock_path.path());
}

/// Check the status of the daemon lock file.
///
/// Reads the PID from the lock file and probes whether the process is alive
/// using `kill(pid, 0)` (signal 0 = existence check, no signal delivered).
///
/// Returns:
/// - `LockStatus::Live(pid)` if the lock file exists and the process is alive.
/// - `LockStatus::Stale(pid)` if the lock file exists but the process is dead.
/// - `LockStatus::Absent` if the lock file does not exist or is unreadable.
pub fn check_lock(lock_path: &LockPath) -> LockStatus {
    let contents = match std::fs::read_to_string(lock_path.path()) {
        Ok(c) => c,
        Err(_) => return LockStatus::Absent,
    };

    let pid: u32 = match contents.trim().parse() {
        Ok(p) => p,
        Err(_) => return LockStatus::Absent, // Corrupted lock file
    };

    if is_process_alive(pid) {
        LockStatus::Live(pid)
    } else {
        LockStatus::Stale(pid)
    }
}

/// Check whether a process with the given PID is alive.
///
/// Uses `kill(pid, 0)` which sends no signal but checks process existence.
/// Returns `true` if the process exists (or we lack permission to signal it),
/// `false` if `ESRCH` (no such process).
fn is_process_alive(pid: u32) -> bool {
    // Safety: kill(pid, 0) is a standard POSIX existence check.
    // SAFETY: sig=0 sends no signal, only checks existence.
    let ret = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if ret == 0 {
        return true; // Process exists and we can signal it.
    }
    // ret == -1: check errno.
    let errno = std::io::Error::last_os_error();
    // EPERM means process exists but we can't signal it (still alive).
    // ESRCH means no such process (dead).
    errno.raw_os_error() != Some(libc::ESRCH)
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// CLI auto-routing — D4-8, INV-DAEMON-007
// ---------------------------------------------------------------------------

/// Commands that bypass daemon routing (always use direct mode).
const DAEMON_EXEMPT_COMMANDS: &[&str] = &[
    "init", "daemon", "mcp", "shell", "merge", "session",
    "bilateral", "trace", "schema", "witness", "challenge",
    "extract", "wrap", "config", "topology", "verify", "analyze",
    "log",
];

/// Try to route a CLI command through the daemon socket.
///
/// Returns `Some(response_text)` if the daemon handled the request,
/// `None` if the daemon is unavailable or the command isn't routable.
///
/// **INV-DAEMON-007**: Auto-detect daemon, fallback to direct.
/// **INV-DAEMON-004**: Semantic equivalence with direct mode.
pub fn try_route_through_daemon(
    braid_dir: &Path,
    cmd_name: &str,
    cmd_json: &JsonValue,
) -> Option<String> {
    // Skip daemon-exempt commands.
    if DAEMON_EXEMPT_COMMANDS.contains(&cmd_name) {
        return None;
    }

    let sock_path = SocketPath::new(braid_dir);
    if !sock_path.path().exists() {
        return None;
    }

    // Try to connect with a short timeout.
    let stream = std::os::unix::net::UnixStream::connect(sock_path.path()).ok()?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .ok()?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(2))).ok()?;

    // Map CLI command to JSON-RPC tools/call.
    let tool_name = match cmd_name {
        "status" => "braid_status",
        "query" => "braid_query",
        "observe" => "braid_observe",
        "harvest" => "braid_harvest",
        "seed" => "braid_seed",
        "guidance" => "braid_guidance",
        _ => return None, // Not yet mapped — use direct mode.
    };

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": cmd_json,
        },
    });

    let mut writer = std::io::BufWriter::new(&stream);
    let bytes = serde_json::to_vec(&request).ok()?;
    writer.write_all(&bytes).ok()?;
    writer.write_all(b"\n").ok()?;
    writer.flush().ok()?;

    // Read response.
    let reader = std::io::BufReader::new(&stream);
    let line = reader.lines().next()?.ok()?;
    let resp: JsonValue = serde_json::from_str(&line).ok()?;

    // Extract text content from MCP response.
    let result = resp.get("result")?;
    let content = result.get("content")?.as_array()?;
    let text = content
        .iter()
        .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
        .collect::<Vec<_>>()
        .join("\n");

    if text.is_empty() {
        return None;
    }

    Some(text)
}

// ---------------------------------------------------------------------------
// Daemon server — D4-5, ADR-DAEMON-001
// ---------------------------------------------------------------------------

/// Run the daemon server (foreground mode).
///
/// Sequence: acquire lock → open LiveStore → install runtime schema →
/// bind socket → signal handlers → accept loop → shutdown.
///
/// **INV-DAEMON-001**: Single daemon enforced via lock file.
/// **INV-DAEMON-002**: `refresh_if_needed()` before every dispatch.
/// **INV-DAEMON-006**: Graceful shutdown on SIGTERM/SIGINT.
pub fn serve_daemon(braid_dir: &Path) -> Result<(), DaemonError> {
    let lock_path = LockPath::new(braid_dir);
    let sock_path = SocketPath::new(braid_dir);

    // 1. Acquire lock (INV-DAEMON-001).
    acquire_lock(&lock_path)?;

    // Ensure cleanup on all exit paths.
    let _guard = CleanupGuard {
        lock_path: lock_path.clone(),
        sock_path: sock_path.clone(),
    };

    // 2. Open LiveStore.
    let mut live = crate::live_store::LiveStore::open(braid_dir)
        .map_err(DaemonError::from)?;

    // 3. Install runtime schema (ADR-DAEMON-003).
    install_runtime_schema(&mut live)?;

    // 4. Remove stale socket if it exists (crash recovery).
    let _ = std::fs::remove_file(sock_path.path());

    // 5. Bind Unix socket.
    let listener = UnixListener::bind(sock_path.path())
        .map_err(DaemonError::BindFailed)?;
    // Non-blocking accept so we can check the shutdown flag.
    listener
        .set_nonblocking(true)
        .map_err(DaemonError::BindFailed)?;

    eprintln!(
        "daemon: listening on {} (pid {})",
        sock_path.path().display(),
        std::process::id()
    );

    // 6. Install signal handlers.
    // Reset any stale shutdown flag from a previous daemon run (or test).
    if let Ok(mut guard) = SHUTDOWN_FLAG.lock() {
        *guard = None;
    }
    let shutdown = Arc::new(AtomicBool::new(false));
    {
        let shutdown_clone = Arc::clone(&shutdown);
        // SAFETY: signal_hook_registry or manual signal handling.
        // We use a simple approach: set the flag on SIGTERM/SIGINT.
        unsafe {
            libc::signal(libc::SIGTERM, signal_handler as *const () as libc::sighandler_t);
            libc::signal(libc::SIGINT, signal_handler as *const () as libc::sighandler_t);
        }
        // Store the Arc in a global so the signal handler can access it.
        SHUTDOWN_FLAG
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .replace(shutdown_clone);
    }

    let start_time = Instant::now();
    let mut request_count: u64 = 0;

    // 7. Accept loop.
    loop {
        if shutdown.load(Ordering::Relaxed) {
            eprintln!("daemon: shutdown signal received");
            break;
        }

        // Check the global flag too (set by signal handler).
        if let Ok(guard) = SHUTDOWN_FLAG.lock() {
            if let Some(ref flag) = *guard {
                if flag.load(Ordering::Relaxed) {
                    eprintln!("daemon: shutdown signal received (via handler)");
                    break;
                }
            }
        }

        match listener.accept() {
            Ok((stream, _addr)) => {
                // Set read timeout for the connection (PM-3: prevent leaked connections).
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
                let _ = stream.set_nonblocking(false);

                handle_connection(stream, &mut live, &start_time, &mut request_count);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No pending connection — sleep briefly and retry.
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
            Err(e) => {
                eprintln!("daemon: accept error: {e}");
                continue;
            }
        }
    }

    // 8. Shutdown: flush LiveStore (INV-DAEMON-006).
    eprintln!("daemon: flushing store...");
    let _ = live.flush();
    eprintln!(
        "daemon: stopped after {} requests, uptime {}s",
        request_count,
        start_time.elapsed().as_secs()
    );

    // CleanupGuard will remove socket and lock on drop.
    Ok(())
}

/// Handle one client connection: read lines, dispatch, respond.
fn handle_connection(
    stream: std::os::unix::net::UnixStream,
    live: &mut crate::live_store::LiveStore,
    start_time: &Instant,
    request_count: &mut u64,
) {
    let reader = std::io::BufReader::new(&stream);
    let mut writer = std::io::BufWriter::new(&stream);

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => break, // Client disconnected or timeout.
        };

        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let msg: JsonValue = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": format!("parse error: {e}"),
                    },
                });
                let _ = write_json_line(&mut writer, &resp);
                continue;
            }
        };

        let id = msg.get("id").cloned().unwrap_or(JsonValue::Null);
        let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let is_notification = msg.get("id").is_none();

        // INV-DAEMON-002: refresh before every dispatch.
        let _ = live.refresh_if_needed();

        let response = match method {
            // Daemon-specific methods.
            "daemon/shutdown" => {
                // Set the global shutdown flag.
                if let Ok(guard) = SHUTDOWN_FLAG.lock() {
                    if let Some(ref flag) = *guard {
                        flag.store(true, Ordering::Relaxed);
                    }
                }
                crate::mcp::jsonrpc_ok(&id, json!({"status": "stopping"}))
            }
            "daemon/status" => {
                let uptime_secs = start_time.elapsed().as_secs();
                let datom_count = live.store().len();
                let entity_count = live.store().entity_count();
                crate::mcp::jsonrpc_ok(
                    &id,
                    json!({
                        "pid": std::process::id(),
                        "uptime_secs": uptime_secs,
                        "request_count": *request_count,
                        "datom_count": datom_count,
                        "entity_count": entity_count,
                    }),
                )
            }
            // Standard MCP methods — delegate to shared handlers.
            "initialize" => crate::mcp::handle_initialize(&id, &params, live),
            "initialized" => {
                if is_notification {
                    continue;
                }
                crate::mcp::jsonrpc_ok(&id, json!({}))
            }
            "tools/list" => crate::mcp::handle_tools_list(&id),
            "tools/call" => {
                // D4-6: Wrap with runtime datom emission (INV-DAEMON-003).
                handle_with_observation(&id, &params, live)
            }
            "ping" => crate::mcp::jsonrpc_ok(&id, json!({})),
            "notifications/cancelled" | "notifications/progress" => continue,
            _ => crate::mcp::jsonrpc_error(
                &id,
                crate::mcp::METHOD_NOT_FOUND,
                &format!("unknown method: {method}"),
            ),
        };

        *request_count += 1;
        let _ = write_json_line(&mut writer, &response);
    }
}

/// Write a JSON value as a newline-delimited line.
fn write_json_line(writer: &mut impl Write, value: &JsonValue) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(value).expect("JSON serialization cannot fail");
    writer.write_all(&bytes)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

/// Handle a tools/call request with runtime datom emission.
///
/// **INV-DAEMON-003**: Every command emits `:runtime/*` datoms.
/// **INV-DAEMON-008**: Emits datoms even on error paths.
fn handle_with_observation(
    id: &JsonValue,
    params: &JsonValue,
    live: &mut crate::live_store::LiveStore,
) -> JsonValue {
    use braid_kernel::datom::*;
    use braid_kernel::layout::TxFile;

    let start = Instant::now();
    let datom_count_before = live.store().len() as i64;
    let cache_hit = !live.has_new_external_txns();

    // Dispatch to shared MCP handler.
    let result = crate::mcp::handle_tools_call(id, params, live);

    // Emit runtime datoms (best-effort — never fail the original request).
    let elapsed_ms = start.elapsed().as_millis() as i64;
    let is_error = result
        .get("result")
        .and_then(|r| r.get("isError"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        || result.get("error").is_some();
    let outcome = if is_error { "error" } else { "success" };

    let tool_name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let request_id_str = format!("{}", id);

    let wall_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let agent = AgentId::from_name("braid:daemon");
    let tx_id = crate::commands::write::next_tx_id(live.store(), agent);

    let ident = format!(
        ":runtime/req-{}",
        &blake3::hash(format!("{}:{}", request_id_str, wall_ms).as_bytes()).to_hex()[..16]
    );
    let entity = EntityId::from_ident(&ident);

    let datoms = vec![
        Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(ident),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":runtime/command"),
            Value::String(tool_name.to_string()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":runtime/request-id"),
            Value::String(request_id_str),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":runtime/latency-ms"),
            Value::Long(elapsed_ms),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":runtime/outcome"),
            Value::String(outcome.to_string()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":runtime/datom-count"),
            Value::Long(datom_count_before),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":runtime/cache-hit"),
            Value::Boolean(cache_hit),
            tx_id,
            Op::Assert,
        ),
    ];

    let tx_file = TxFile {
        tx_id,
        agent,
        provenance: ProvenanceType::Derived,
        rationale: format!("runtime observation: {tool_name}"),
        causal_predecessors: vec![],
        datoms,
    };

    // Best-effort: do not fail the original request on observation failure.
    if let Err(e) = live.write_tx(&tx_file) {
        eprintln!("daemon: failed to write runtime datom: {e}");
    }

    result
}

// ---------------------------------------------------------------------------
// Signal handling
// ---------------------------------------------------------------------------

/// Global shutdown flag, accessible from the signal handler.
static SHUTDOWN_FLAG: std::sync::Mutex<Option<Arc<AtomicBool>>> =
    std::sync::Mutex::new(None);

/// Signal handler that sets the shutdown flag.
///
/// SAFETY: Only accesses an atomic bool (async-signal-safe on all platforms).
extern "C" fn signal_handler(_sig: libc::c_int) {
    if let Ok(guard) = SHUTDOWN_FLAG.lock() {
        if let Some(ref flag) = *guard {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

/// RAII guard that cleans up socket and lock files on drop.
struct CleanupGuard {
    lock_path: LockPath,
    sock_path: SocketPath,
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.sock_path.path());
        release_lock(&self.lock_path);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_appends_filename() {
        let sp = SocketPath::new(Path::new("/tmp/.braid"));
        assert_eq!(sp.path(), Path::new("/tmp/.braid/daemon.sock"));
    }

    #[test]
    fn lock_path_appends_filename() {
        let lp = LockPath::new(Path::new("/tmp/.braid"));
        assert_eq!(lp.path(), Path::new("/tmp/.braid/daemon.lock"));
    }

    #[test]
    fn socket_path_inner_accessible() {
        let sp = SocketPath(PathBuf::from("/a/b/c.sock"));
        assert_eq!(sp.0, PathBuf::from("/a/b/c.sock"));
    }

    #[test]
    fn lock_path_inner_accessible() {
        let lp = LockPath(PathBuf::from("/a/b/c.lock"));
        assert_eq!(lp.0, PathBuf::from("/a/b/c.lock"));
    }

    #[test]
    fn request_id_wraps_json_value() {
        let rid = RequestId(serde_json::json!(42));
        assert_eq!(rid.0, serde_json::json!(42));
    }

    #[test]
    fn request_id_string_variant() {
        let rid = RequestId(serde_json::json!("abc-123"));
        assert_eq!(rid.0, serde_json::json!("abc-123"));
    }

    #[test]
    fn request_id_null_variant() {
        let rid = RequestId(serde_json::Value::Null);
        assert!(rid.0.is_null());
    }

    #[test]
    fn lock_status_eq() {
        assert_eq!(LockStatus::Live(100), LockStatus::Live(100));
        assert_ne!(LockStatus::Live(100), LockStatus::Stale(100));
        assert_eq!(LockStatus::Absent, LockStatus::Absent);
    }

    #[test]
    fn daemon_error_display_lock_held() {
        let err = DaemonError::LockHeld { pid: 1234 };
        let msg = err.to_string();
        assert!(msg.contains("error:"), "must have error prefix");
        assert!(msg.contains("why:"), "must have why section");
        assert!(msg.contains("fix:"), "must have fix section");
        assert!(msg.contains("ref:"), "must have ref section");
        assert!(msg.contains("1234"), "must include the PID");
    }

    #[test]
    fn daemon_error_display_all_variants_structured() {
        let variants: Vec<DaemonError> = vec![
            DaemonError::LockHeld { pid: 1 },
            DaemonError::LockStale { pid: 2 },
            DaemonError::BindFailed(std::io::Error::other("test")),
            DaemonError::AlreadyStopping,
            DaemonError::NotRunning,
            DaemonError::ConnectionFailed(std::io::Error::other("test")),
            DaemonError::Timeout,
            DaemonError::StoreError(crate::error::BraidError::Validation("test".into())),
            DaemonError::ProtocolError("bad json".into()),
        ];
        for v in &variants {
            let msg = v.to_string();
            assert!(msg.contains("error:"), "error: missing for {msg}");
            assert!(msg.contains("why:"), "why: missing for {msg}");
            assert!(msg.contains("fix:"), "fix: missing for {msg}");
            assert!(msg.contains("ref:"), "ref: missing for {msg}");
        }
    }

    #[test]
    fn daemon_error_source_delegates() {
        use std::error::Error;

        let io_err = DaemonError::BindFailed(std::io::Error::other("bind"));
        assert!(io_err.source().is_some());

        let conn_err = DaemonError::ConnectionFailed(std::io::Error::other("conn"));
        assert!(conn_err.source().is_some());

        let store_err =
            DaemonError::StoreError(crate::error::BraidError::Validation("v".into()));
        assert!(store_err.source().is_some());

        let timeout = DaemonError::Timeout;
        assert!(timeout.source().is_none());

        let proto = DaemonError::ProtocolError("bad".into());
        assert!(proto.source().is_none());
    }

    #[test]
    fn from_io_error_produces_connection_failed() {
        let io_err = std::io::Error::other("test io");
        let daemon_err: DaemonError = io_err.into();
        assert!(matches!(daemon_err, DaemonError::ConnectionFailed(_)));
    }

    #[test]
    fn from_braid_error_produces_store_error() {
        let braid_err = crate::error::BraidError::Validation("test".into());
        let daemon_err: DaemonError = braid_err.into();
        assert!(matches!(daemon_err, DaemonError::StoreError(_)));
    }

    // ── Lock management tests (D4-2) ────────────────────────────────────

    #[test]
    fn acquire_lock_success() {
        let dir = tempfile::tempdir().unwrap();
        let lock = LockPath::new(dir.path());
        acquire_lock(&lock).expect("should acquire lock on clean directory");
        assert!(lock.path().exists(), "lock file must exist after acquire");
        // Verify PID content.
        let contents = std::fs::read_to_string(lock.path()).unwrap();
        let pid: u32 = contents.trim().parse().unwrap();
        assert_eq!(pid, std::process::id(), "lock must contain our PID");
    }

    #[test]
    fn acquire_lock_already_held() {
        let dir = tempfile::tempdir().unwrap();
        let lock = LockPath::new(dir.path());
        // Write our own PID (we're alive).
        std::fs::write(lock.path(), format!("{}\n", std::process::id())).unwrap();
        let result = acquire_lock(&lock);
        assert!(
            matches!(result, Err(DaemonError::LockHeld { .. })),
            "should fail with LockHeld: {result:?}"
        );
    }

    #[test]
    fn acquire_lock_stale_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let lock = LockPath::new(dir.path());
        // Write a PID that is very likely dead (max PID on Linux is 2^22).
        std::fs::write(lock.path(), "4194300\n").unwrap();
        // acquire_lock should detect stale, remove, and succeed.
        acquire_lock(&lock).expect("should recover from stale lock");
        // Verify we now own the lock.
        let contents = std::fs::read_to_string(lock.path()).unwrap();
        let pid: u32 = contents.trim().parse().unwrap();
        assert_eq!(pid, std::process::id());
    }

    #[test]
    fn release_lock_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let lock = LockPath::new(dir.path());
        acquire_lock(&lock).unwrap();
        assert!(lock.path().exists());
        release_lock(&lock);
        assert!(!lock.path().exists(), "lock file must be removed after release");
    }

    #[test]
    fn release_lock_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let lock = LockPath::new(dir.path());
        // Release without acquire should not panic.
        release_lock(&lock);
        release_lock(&lock);
    }

    #[test]
    fn check_lock_absent() {
        let dir = tempfile::tempdir().unwrap();
        let lock = LockPath::new(dir.path());
        assert_eq!(check_lock(&lock), LockStatus::Absent);
    }

    #[test]
    fn check_lock_stale() {
        let dir = tempfile::tempdir().unwrap();
        let lock = LockPath::new(dir.path());
        std::fs::write(lock.path(), "4194300\n").unwrap();
        assert_eq!(check_lock(&lock), LockStatus::Stale(4194300));
    }

    #[test]
    fn check_lock_live() {
        let dir = tempfile::tempdir().unwrap();
        let lock = LockPath::new(dir.path());
        std::fs::write(lock.path(), format!("{}\n", std::process::id())).unwrap();
        assert_eq!(check_lock(&lock), LockStatus::Live(std::process::id()));
    }

    #[test]
    fn check_lock_corrupted_returns_absent() {
        let dir = tempfile::tempdir().unwrap();
        let lock = LockPath::new(dir.path());
        std::fs::write(lock.path(), "not-a-pid\n").unwrap();
        assert_eq!(check_lock(&lock), LockStatus::Absent);
    }

    #[test]
    fn is_process_alive_returns_true_for_self() {
        assert!(is_process_alive(std::process::id()));
    }

    #[test]
    fn is_process_alive_returns_false_for_dead_pid() {
        // PID 4194300 is near the Linux max and very likely not in use.
        assert!(!is_process_alive(4194300));
    }

    // ── Runtime schema tests (D4-4) ─────────────────────────────────────

    #[test]
    fn runtime_schema_installed() {
        use braid_kernel::datom::{Attribute, Op};

        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let mut live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        install_runtime_schema(&mut live).unwrap();

        // Verify :runtime/command has :db/valueType
        let store = live.store();
        let entity = braid_kernel::datom::EntityId::from_ident(":runtime/command");
        let vt_attr = Attribute::from_keyword(":db/valueType");
        let has_value_type = store
            .entity_datoms(entity)
            .iter()
            .any(|d| d.attribute == vt_attr && d.op == Op::Assert);
        assert!(has_value_type, ":runtime/command must have :db/valueType after install");
    }

    #[test]
    fn runtime_schema_all_six_attrs() {
        use braid_kernel::datom::{Attribute, Op};

        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let mut live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        install_runtime_schema(&mut live).unwrap();

        let store = live.store();
        let vt_attr = Attribute::from_keyword(":db/valueType");

        for &(ident, _, _, _) in RUNTIME_ATTRS {
            let entity = braid_kernel::datom::EntityId::from_ident(ident);
            let has_schema = store
                .entity_datoms(entity)
                .iter()
                .any(|d| d.attribute == vt_attr && d.op == Op::Assert);
            assert!(has_schema, "{ident} must have :db/valueType after install");
        }
    }

    #[test]
    fn runtime_schema_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let mut live = crate::live_store::LiveStore::create(&braid_dir).unwrap();

        let count_before = live.store().len();
        install_runtime_schema(&mut live).unwrap();
        let count_after_first = live.store().len();
        install_runtime_schema(&mut live).unwrap();
        let count_after_second = live.store().len();

        assert!(count_after_first > count_before, "first install should add datoms");
        assert_eq!(
            count_after_first, count_after_second,
            "second install should be a no-op (idempotent)"
        );
    }

    // ── handle_with_observation tests (D4-TEST-2) ───────────────────────

    /// Helper: create a fresh LiveStore with runtime schema installed.
    fn setup_live_with_schema() -> (tempfile::TempDir, crate::live_store::LiveStore) {
        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let mut live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        install_runtime_schema(&mut live).unwrap();
        (dir, live)
    }

    /// Helper: count entities with :runtime/command attribute.
    fn count_runtime_entities(store: &braid_kernel::Store) -> usize {
        use braid_kernel::datom::{Attribute, Op};
        let cmd_attr = Attribute::from_keyword(":runtime/command");
        store
            .datoms()
            .filter(|d| d.attribute == cmd_attr && d.op == Op::Assert)
            .count()
    }

    #[test]
    fn handle_with_observation_emits_datoms() {
        let (_dir, mut live) = setup_live_with_schema();
        let before = count_runtime_entities(live.store());

        // Simulate a braid_status tool call.
        let id = serde_json::json!(1);
        let params = serde_json::json!({
            "name": "braid_status",
            "arguments": {},
        });

        let _result = handle_with_observation(&id, &params, &mut live);

        let after = count_runtime_entities(live.store());
        assert_eq!(
            after,
            before + 1,
            "handle_with_observation must emit exactly 1 runtime entity"
        );
    }

    #[test]
    fn handle_with_observation_error_path_emits_datoms() {
        let (_dir, mut live) = setup_live_with_schema();
        let before = count_runtime_entities(live.store());

        // Simulate a call to a nonexistent tool (produces isError response).
        let id = serde_json::json!(42);
        let params = serde_json::json!({
            "name": "nonexistent_tool",
            "arguments": {},
        });

        let result = handle_with_observation(&id, &params, &mut live);

        // Verify the result is an error.
        let is_error = result
            .get("result")
            .and_then(|r| r.get("isError"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert!(is_error, "nonexistent tool should produce isError response");

        // INV-DAEMON-008: runtime datom must still be emitted on error.
        let after = count_runtime_entities(live.store());
        assert_eq!(
            after,
            before + 1,
            "INV-DAEMON-008: error path must still emit runtime datom"
        );

        // Verify the outcome is "error".
        use braid_kernel::datom::{Attribute, Op};
        let outcome_attr = Attribute::from_keyword(":runtime/outcome");
        let has_error_outcome = live
            .store()
            .datoms()
            .any(|d| {
                d.attribute == outcome_attr
                    && d.op == Op::Assert
                    && d.value == braid_kernel::datom::Value::String("error".to_string())
            });
        assert!(
            has_error_outcome,
            "error path runtime datom must have outcome='error'"
        );
    }

    #[test]
    fn handle_with_observation_latency_plausible() {
        let (_dir, mut live) = setup_live_with_schema();

        let id = serde_json::json!(1);
        let params = serde_json::json!({
            "name": "braid_status",
            "arguments": {},
        });

        let _result = handle_with_observation(&id, &params, &mut live);

        // Find the runtime datom's latency.
        use braid_kernel::datom::{Attribute, Op, Value};
        let lat_attr = Attribute::from_keyword(":runtime/latency-ms");
        let latency = live
            .store()
            .datoms()
            .find(|d| d.attribute == lat_attr && d.op == Op::Assert)
            .and_then(|d| match &d.value {
                Value::Long(ms) => Some(*ms),
                _ => None,
            });

        let ms = latency.expect(":runtime/latency-ms must exist");
        assert!(ms > 0, "latency must be positive, got {ms}");
        assert!(ms < 60_000, "latency must be < 60s, got {ms}ms");
    }

    #[test]
    fn handle_with_observation_request_id_matches() {
        let (_dir, mut live) = setup_live_with_schema();

        let id = serde_json::json!("req-abc-123");
        let params = serde_json::json!({
            "name": "braid_status",
            "arguments": {},
        });

        let _result = handle_with_observation(&id, &params, &mut live);

        // Find the runtime datom's request-id.
        use braid_kernel::datom::{Attribute, Op, Value};
        let rid_attr = Attribute::from_keyword(":runtime/request-id");
        let request_id = live
            .store()
            .datoms()
            .find(|d| d.attribute == rid_attr && d.op == Op::Assert)
            .and_then(|d| match &d.value {
                Value::String(s) => Some(s.clone()),
                _ => None,
            });

        let rid = request_id.expect(":runtime/request-id must exist");
        assert!(
            rid.contains("req-abc-123"),
            "request-id must contain the original JSON-RPC id, got: {rid}"
        );
    }

    // ── Integration tests (D4-TEST-3) ────────────────────────────────────
    //
    // Integration tests use a global SHUTDOWN_FLAG and must not run in parallel.
    // We use a Mutex to serialize them within the test process.

    static INTEGRATION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Helper: start daemon in a background thread, return the join handle.
    /// The daemon runs on the given braid_dir.
    fn start_daemon_thread(
        braid_dir: std::path::PathBuf,
    ) -> std::thread::JoinHandle<Result<(), DaemonError>> {
        std::thread::spawn(move || serve_daemon(&braid_dir))
    }

    /// Helper: send a JSON-RPC request to the daemon socket and return response.
    fn send_socket_request(
        sock_path: &Path,
        method: &str,
        params: serde_json::Value,
    ) -> Option<serde_json::Value> {
        use std::io::{BufRead, Write};
        use std::os::unix::net::UnixStream;

        let stream = UnixStream::connect(sock_path).ok()?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .ok()?;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        let mut writer = std::io::BufWriter::new(&stream);
        let bytes = serde_json::to_vec(&request).ok()?;
        writer.write_all(&bytes).ok()?;
        writer.write_all(b"\n").ok()?;
        writer.flush().ok()?;

        let reader = std::io::BufReader::new(&stream);
        let line = reader.lines().next()?.ok()?;
        serde_json::from_str(&line).ok()
    }

    #[test]
    fn daemon_start_stop_lifecycle() {
        let _lock = INTEGRATION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let mut live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        install_runtime_schema(&mut live).unwrap();
        let _ = live.flush();
        drop(live);

        let sock_path = SocketPath::new(&braid_dir);
        let lock_path = LockPath::new(&braid_dir);

        // Start daemon in background thread.
        let braid_dir_clone = braid_dir.clone();
        let handle = start_daemon_thread(braid_dir_clone);
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Verify socket exists.
        assert!(
            sock_path.path().exists(),
            "daemon.sock must exist after start"
        );

        // Verify lock exists with valid PID.
        assert!(
            matches!(check_lock(&lock_path), LockStatus::Live(_)),
            "daemon.lock must contain a live PID"
        );

        // Send daemon/status and verify response.
        let resp = send_socket_request(
            sock_path.path(),
            "daemon/status",
            serde_json::json!({}),
        );
        assert!(resp.is_some(), "daemon/status must return a response");
        let resp = resp.unwrap();
        let pid = resp
            .get("result")
            .and_then(|r| r.get("pid"))
            .and_then(|v| v.as_u64());
        assert!(pid.is_some(), "daemon/status must return PID");

        // Send shutdown.
        let _shutdown_resp = send_socket_request(
            sock_path.path(),
            "daemon/shutdown",
            serde_json::json!({}),
        );

        // Wait for daemon thread to finish.
        let result = handle.join().expect("daemon thread must not panic");
        assert!(result.is_ok(), "daemon must exit cleanly: {result:?}");

        // Verify cleanup.
        assert!(
            !sock_path.path().exists(),
            "daemon.sock must be removed after shutdown"
        );
        assert!(
            !lock_path.path().exists(),
            "daemon.lock must be removed after shutdown"
        );
    }

    #[test]
    fn daemon_status_query_via_socket() {
        let _lock = INTEGRATION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let mut live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        install_runtime_schema(&mut live).unwrap();
        let _ = live.flush();
        drop(live);

        let sock_path = SocketPath::new(&braid_dir);
        let braid_dir_clone = braid_dir.clone();
        let handle = start_daemon_thread(braid_dir_clone);
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Send braid_status tool call via socket.
        let resp = send_socket_request(
            sock_path.path(),
            "tools/call",
            serde_json::json!({"name": "braid_status", "arguments": {}}),
        );
        assert!(resp.is_some(), "braid_status via socket must return a response");

        // Verify response has content.
        let resp = resp.unwrap();
        let text = resp
            .get("result")
            .and_then(|r| r.get("content"))
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str());
        assert!(text.is_some(), "response must have text content");
        let text = text.unwrap();
        assert!(
            text.contains("store:") || text.contains("datom"),
            "status response must mention store or datoms: {text}"
        );

        // Shutdown.
        let _ = send_socket_request(
            sock_path.path(),
            "daemon/shutdown",
            serde_json::json!({}),
        );
        let _ = handle.join();
    }

    #[test]
    fn daemon_runtime_datoms_after_tool_calls() {
        let _lock = INTEGRATION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        use std::io::{BufRead, Write};
        use std::os::unix::net::UnixStream;

        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let mut live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        install_runtime_schema(&mut live).unwrap();
        let _ = live.flush();
        drop(live);

        let sock_path = SocketPath::new(&braid_dir);
        let braid_dir_clone = braid_dir.clone();
        let handle = start_daemon_thread(braid_dir_clone);
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Send 3 tool calls on a SINGLE connection (line-delimited protocol).
        let response_count = {
            let stream = UnixStream::connect(sock_path.path())
                .expect("must connect to daemon");
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .ok();
            {
                let mut writer = std::io::BufWriter::new(&stream);
                for i in 1..=3 {
                    let request = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": i,
                        "method": "tools/call",
                        "params": {"name": "braid_status", "arguments": {}},
                    });
                    let bytes = serde_json::to_vec(&request).unwrap();
                    writer.write_all(&bytes).unwrap();
                    writer.write_all(b"\n").unwrap();
                    writer.flush().unwrap();
                }
            }

            // Read 3 responses.
            let reader = std::io::BufReader::new(&stream);
            let mut count = 0;
            for line in reader.lines() {
                if line.is_ok() {
                    count += 1;
                }
                if count >= 3 {
                    break;
                }
            }
            count
        };

        assert_eq!(response_count, 3, "must get 3 responses from daemon");

        // Shutdown.
        let _ = send_socket_request(
            sock_path.path(),
            "daemon/shutdown",
            serde_json::json!({}),
        );
        let _ = handle.join();

        // Now open the store directly and count runtime entities.
        let live = crate::live_store::LiveStore::open(&braid_dir).unwrap();
        let count = count_runtime_entities(live.store());
        assert!(
            count >= 3,
            "3 tool calls must produce at least 3 runtime entities, got {count}"
        );
    }

    #[test]
    fn handle_with_observation_five_calls_five_entities() {
        let (_dir, mut live) = setup_live_with_schema();

        for i in 1..=5 {
            let id = serde_json::json!(i);
            let params = serde_json::json!({
                "name": "braid_status",
                "arguments": {},
            });
            let _ = handle_with_observation(&id, &params, &mut live);
        }

        let count = count_runtime_entities(live.store());
        assert_eq!(count, 5, "5 tool calls must produce exactly 5 runtime entities");
    }
}
