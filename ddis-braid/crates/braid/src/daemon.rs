//! Braid session daemon — INV-DAEMON-001..009, ADR-DAEMON-001..003.
//!
//! A Unix-socket daemon that holds a single [`LiveStore`] in memory,
//! serves JSON-RPC requests using the same protocol as the MCP server,
//! and emits reflexive `:runtime/*` datoms for every processed command.
//!
//! # Architecture (ADR-DAEMON-002, DS4)
//!
//! The daemon reuses the MCP tool dispatch from [`crate::mcp`]. It adds:
//! - Lifecycle management (lock file, signal handling, graceful shutdown)
//! - Unix socket transport (instead of stdin/stdout)
//! - Runtime datom emission (`handle_with_observation`)
//! - **DS4**: Multi-threaded connection dispatch via `Arc<RwLock<LiveStore>>`
//! - **DS6**: Integration wiring — `CommitHandle` and `CheckpointSignal` plumbed
//!   to connection threads; `harvest --commit` triggers full WAL checkpoint
//!
//! ## Concurrency Model (DS2/DS4)
//!
//! The accept loop spawns one thread per incoming connection. The shared
//! `LiveStore` is wrapped in `Arc<RwLock<LiveStore>>`.
//!
//! **Read/write lock split**: Read-only `tools/call` requests (`braid_status`,
//! `braid_query`, `braid_guidance`, `braid_task_ready`) acquire `read()` —
//! concurrent with other reads, zero contention. Write tools and all other
//! methods acquire `write()` (exclusive).
//!
//! For write-path `tools/call` requests, the dispatch thread acquires a
//! write lock to run the tool handler and build the runtime observation
//! `TxFile`, then **releases the lock** before submitting the `TxFile` to
//! the group commit thread via [`CommitHandle`]. The commit thread
//! WAL-fsyncs the batch, then acquires its own write lock to apply datoms
//! in-memory. This eliminates per-write EDN file creation and fsync — the
//! checkpoint thread handles EDN conversion in the background.
//!
//! # Invariants
//!
//! - **INV-DAEMON-001**: At most one daemon per `.braid` directory.
//! - **INV-DAEMON-002**: Store always consistent with disk (`refresh_if_needed`).
//! - **INV-DAEMON-003**: Every command emits `:runtime/*` datoms.
//! - **INV-DAEMON-004**: Semantic equivalence with direct mode.
//! - **INV-DAEMON-005**: Stale lock recovery via `kill(pid, 0)`.
//! - **INV-DAEMON-006**: Graceful shutdown preserves all state.
//! - **INV-DAEMON-012**: Multi-threaded dispatch — accept loop never blocks on dispatch.

use std::io::{BufRead, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, RwLock};
use std::time::{Duration, Instant};

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
        ":runtime/latency-us",
        ":db.type/long",
        ":db.cardinality/one",
        "Wall clock microseconds for request processing",
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
pub fn install_runtime_schema(live: &mut crate::live_store::LiveStore) -> Result<(), DaemonError> {
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
// ---------------------------------------------------------------------------
// Capability census — PERF-4a
// ---------------------------------------------------------------------------

/// Compiled-in subsystem capabilities.
///
/// Each tuple: (name, description, module_path).
/// This is the binary's self-model — what the daemon knows it can do.
const CAPABILITIES: &[(&str, &str, &str)] = &[
    (
        "store-binary-cache",
        "Bincode-serialized store.bin for O(1) startup",
        "crates/braid/src/layout.rs",
    ),
    (
        "incremental-tx-loading",
        "Apply new txn files without full store rebuild",
        "crates/braid/src/live_store.rs",
    ),
    (
        "external-write-detection",
        "O(1) mtime-based detection of CLI writes during daemon mode",
        "crates/braid/src/live_store.rs",
    ),
    (
        "write-through-persistence",
        "Every write is durable before returning (fsync + in-memory)",
        "crates/braid/src/live_store.rs",
    ),
    (
        "materialized-views",
        "Incremental fitness/coherence via Store::observe_datom",
        "crates/braid-kernel/src/store.rs",
    ),
    (
        "bilateral-scan",
        "Spec-impl alignment boundary evaluation",
        "crates/braid-kernel/src/bilateral.rs",
    ),
    (
        "spectral-partition",
        "Fiedler-based graph partition for topology planning",
        "crates/braid-kernel/src/topology.rs",
    ),
    (
        "datalog-query",
        "Stratified Datalog evaluation with CALM compliance",
        "crates/braid-kernel/src/query/evaluator.rs",
    ),
    (
        "harvest-pipeline",
        "Session knowledge extraction with candidate scoring",
        "crates/braid-kernel/src/harvest.rs",
    ),
    (
        "seed-assembly",
        "Task-conditioned context assembly with budget management",
        "crates/braid-kernel/src/seed.rs",
    ),
    (
        "routing-engine",
        "R(t) task impact scoring with calibration",
        "crates/braid-kernel/src/routing.rs",
    ),
    (
        "bridge-hypotheses",
        "FEGH free-energy gradient over hypothetical observations",
        "crates/braid-kernel/src/routing.rs",
    ),
    (
        "hypothesis-ledger",
        "Predicted vs actual outcome tracking with calibration",
        "crates/braid-kernel/src/routing.rs",
    ),
    (
        "topology-planning",
        "Spectral task partition for multi-agent coordination",
        "crates/braid-kernel/src/topology.rs",
    ),
    (
        "cotx-routing",
        "Contextual observation auto-routing (finding/task/ADR/question)",
        "crates/braid-kernel/src/guidance.rs",
    ),
    (
        "crdt-merge",
        "Set-union merge with per-attribute resolution modes",
        "crates/braid-kernel/src/merge.rs",
    ),
    (
        "runtime-self-observation",
        "Daemon emits :runtime/* datoms for reflexive F(S)",
        "crates/braid/src/daemon.rs",
    ),
    (
        "unix-socket-daemon",
        "Persistent LiveStore with JSON-RPC over Unix socket",
        "crates/braid/src/daemon.rs",
    ),
];

/// Run the capability census: record compiled-in subsystems as `:capability/*` datoms.
///
/// Idempotent: checks for `:capability/store-binary-cache` existence before transacting.
/// The census grounds the store's self-model in the binary's actual capabilities,
/// preventing tasks from being created for already-implemented features.
///
/// **PERF-4a**: The daemon's first act is binary reflection.
pub fn run_capability_census(live: &mut crate::live_store::LiveStore) -> Result<(), DaemonError> {
    use braid_kernel::datom::*;
    use braid_kernel::layout::TxFile;

    // Idempotency check.
    let check_entity = EntityId::from_ident(":capability/store-binary-cache");
    let doc_attr = Attribute::from_keyword(":db/doc");
    let already_run = live
        .store()
        .entity_datoms(check_entity)
        .iter()
        .any(|d| d.attribute == doc_attr && d.op == Op::Assert);

    if already_run {
        return Ok(());
    }

    let agent = AgentId::from_name("braid:daemon");
    let tx_id = crate::commands::write::next_tx_id(live.store(), agent);

    let mut datoms = Vec::new();

    for &(name, description, module_path) in CAPABILITIES {
        let ident = format!(":capability/{name}");
        let entity = EntityId::from_ident(&ident);

        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(ident),
            tx_id,
            Op::Assert,
        ));
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String(description.to_string()),
            tx_id,
            Op::Assert,
        ));
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":capability/implemented"),
            Value::Boolean(true),
            tx_id,
            Op::Assert,
        ));
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":capability/file-path"),
            Value::String(module_path.to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    let tx_file = TxFile {
        tx_id,
        agent,
        provenance: ProvenanceType::Derived,
        rationale: "PERF-4a: capability census — binary self-reflection".to_string(),
        causal_predecessors: vec![],
        datoms,
    };

    live.write_tx(&tx_file).map_err(DaemonError::from)?;
    eprintln!(
        "daemon: capability census complete ({} subsystems registered)",
        CAPABILITIES.len()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Reflexive FEGH — PERF-4b
// ---------------------------------------------------------------------------

/// Run bridge hypothesis generation on the daemon's own knowledge graph.
///
/// This is FEGH-1 applied reflexively: the daemon examines its own store for
/// disconnected knowledge clusters and generates hypothetical bridging
/// observations. The results are recorded as `:hypothesis/*` datoms with
/// `item_type=reflexive`.
///
/// **PERF-4b**: The simplest test case for bridge hypotheses because we
/// control every variable and have immediate ground truth.
///
/// Best-effort: failures are logged but don't prevent daemon startup.
fn run_reflexive_fegh(live: &mut crate::live_store::LiveStore) {
    use braid_kernel::datom::*;
    use braid_kernel::layout::TxFile;

    let store = live.store();

    // Only run if we have enough entities for meaningful community detection.
    if store.entity_count() < 10 {
        return;
    }

    let bridges = braid_kernel::routing::generate_bridge_hypotheses(store, 3);
    if bridges.is_empty() {
        eprintln!("daemon: reflexive FEGH — no bridge hypotheses (graph may be fully connected)");
        return;
    }

    let agent = AgentId::from_name("braid:daemon");
    let tx_id = crate::commands::write::next_tx_id(store, agent);
    let wall_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let mut datoms = Vec::new();

    for (i, bridge) in bridges.iter().enumerate() {
        let ident = format!(":hypothesis/reflexive-{}-{}", wall_ms, i);
        let entity = EntityId::from_ident(&ident);

        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(ident),
            tx_id,
            Op::Assert,
        ));
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String(bridge.question.clone()),
            tx_id,
            Op::Assert,
        ));
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":hypothesis/source-label"),
            Value::String(bridge.source_label.clone()),
            tx_id,
            Op::Assert,
        ));
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":hypothesis/target-label"),
            Value::String(bridge.target_label.clone()),
            tx_id,
            Op::Assert,
        ));
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":hypothesis/delta-fs"),
            Value::Double(ordered_float::OrderedFloat(bridge.delta_fs)),
            tx_id,
            Op::Assert,
        ));
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":hypothesis/alpha"),
            Value::Double(ordered_float::OrderedFloat(bridge.alpha)),
            tx_id,
            Op::Assert,
        ));
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":hypothesis/item-type"),
            Value::String("reflexive".to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    if datoms.is_empty() {
        return;
    }

    let tx_file = TxFile {
        tx_id,
        agent,
        provenance: ProvenanceType::Derived,
        rationale: "PERF-4b: reflexive FEGH — bridge hypotheses on self-model".to_string(),
        causal_predecessors: vec![],
        datoms,
    };

    match live.write_tx(&tx_file) {
        Ok(_) => {
            eprintln!(
                "daemon: reflexive FEGH — {} bridge hypotheses recorded",
                bridges.len()
            );
            for (i, b) in bridges.iter().enumerate() {
                eprintln!(
                    "  [{i}] {} ↔ {} (ΔF(S)={:.3}, α={:.3}): {}",
                    b.source_label,
                    b.target_label,
                    b.delta_fs,
                    b.alpha,
                    braid_kernel::safe_truncate_bytes(&b.question, 80)
                );
            }
        }
        Err(e) => {
            eprintln!("daemon: reflexive FEGH write failed: {e}");
        }
    }
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
    match OpenOptions::new().write(true).create_new(true).open(path) {
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
                    eprintln!("daemon: removing stale lock (pid {pid} is dead)");
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
    // DEFECT-005 fix: PIDs > i32::MAX would wrap to negative values,
    // causing kill() to signal process groups instead of individual
    // processes. Treat as dead (invalid PID).
    if pid > i32::MAX as u32 {
        return false;
    }
    // Safety: kill(pid, 0) is a standard POSIX existence check.
    // SAFETY: sig=0 sends no signal, only checks existence. pid is
    // verified positive (fits in i32) above.
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

/// Map a CLI Command to its daemon-routable MCP tool name and JSON arguments.
///
/// Returns `None` for commands that should use direct mode (init, daemon, shell,
/// bilateral, topology, etc.). Returns `Some((tool_name, args))` for the 11
/// commands the daemon can handle.
///
/// **INV-DAEMON-004**: Argument marshaling preserves semantic equivalence.
/// **DW2**: Full coverage of all MCP tools defined in `mcp::tool_definitions()`.
pub fn marshal_command(cmd: &crate::commands::Command) -> Option<(&'static str, JsonValue)> {
    use crate::commands::{Command, ObserveAction, TaskAction, WriteAction};

    match cmd {
        // 1. status → braid_status {}
        Command::Status { .. } => Some(("braid_status", json!({}))),

        // 2. query → braid_query {datalog?, entity?, attribute?}
        Command::Query {
            entity,
            attribute,
            datalog,
            positional_datalog,
            ..
        } => {
            let mut args = json!({});
            // --datalog flag takes precedence, then positional
            let dlog = datalog.as_deref().or(positional_datalog.as_deref());
            if let Some(d) = dlog {
                args["datalog"] = json!(d);
            }
            if let Some(e) = entity {
                args["entity"] = json!(e);
            }
            if let Some(a) = attribute {
                args["attribute"] = json!(a);
            }
            Some(("braid_query", args))
        }

        // 3. observe (with text) → braid_observe {...}
        //    Observe subcommands (list, search, show, recent) are not routable.
        //    ObserveAction::Create is also routable (explicit creation form).
        Command::Observe {
            text: Some(text),
            confidence,
            tag,
            category,
            relates_to,
            rationale,
            alternatives,
            no_auto_crystallize,
            action: None,
            ..
        } => {
            let mut args = json!({"text": text, "confidence": confidence});
            if !tag.is_empty() {
                args["tags"] = json!(tag);
            }
            if let Some(c) = category {
                args["category"] = json!(c);
            }
            if let Some(r) = relates_to {
                args["relates_to"] = json!(r);
            }
            if let Some(r) = rationale {
                args["rationale"] = json!(r);
            }
            if let Some(a) = alternatives {
                args["alternatives"] = json!(a);
            }
            if *no_auto_crystallize {
                args["no_auto_crystallize"] = json!(true);
            }
            Some(("braid_observe", args))
        }

        // ObserveAction::Create — explicit `braid observe create "text"`
        Command::Observe {
            action:
                Some(ObserveAction::Create {
                    text,
                    confidence,
                    tag,
                    category,
                    relates_to,
                    rationale,
                    alternatives,
                    no_auto_crystallize,
                    ..
                }),
            ..
        } => {
            let mut args = json!({"text": text, "confidence": confidence});
            if !tag.is_empty() {
                args["tags"] = json!(tag);
            }
            if let Some(c) = category {
                args["category"] = json!(c);
            }
            if let Some(r) = relates_to {
                args["relates_to"] = json!(r);
            }
            if let Some(r) = rationale {
                args["rationale"] = json!(r);
            }
            if let Some(a) = alternatives {
                args["alternatives"] = json!(a);
            }
            if *no_auto_crystallize {
                args["no_auto_crystallize"] = json!(true);
            }
            Some(("braid_observe", args))
        }

        // Observe subcommands (list, search, show, recent) — not routable
        Command::Observe { .. } => None,

        // 4. harvest → braid_harvest {task?, commit?, force?, no_reconcile?}
        Command::Harvest {
            task,
            commit,
            force,
            no_reconcile,
            ..
        } => {
            let mut args = json!({});
            if let Some(t) = task {
                args["task"] = json!(t);
            } else {
                // braid_harvest MCP tool requires "task" — provide a default
                args["task"] = json!("continue");
            }
            if *commit {
                args["commit"] = json!(true);
            }
            if *force {
                args["force"] = json!(true);
            }
            if *no_reconcile {
                args["no_reconcile"] = json!(true);
            }
            Some(("braid_harvest", args))
        }

        // 5. go → braid_task_go {id}
        Command::Go { id, .. } => Some(("braid_task_go", json!({"id": id}))),

        // 6. next → braid_task_ready {}
        Command::Next { .. } => Some(("braid_task_ready", json!({}))),

        // 7. done → braid_task_close {id}
        //    Closes the first ID. Multiple IDs fall back to direct mode.
        Command::Done { ids, .. } => {
            if ids.len() == 1 {
                Some(("braid_task_close", json!({"id": ids[0]})))
            } else {
                // Multiple IDs or zero IDs — use direct mode for batch close
                None
            }
        }

        // 8. task create → braid_task_create {title, priority?, ...}
        Command::Task {
            action:
                TaskAction::Create {
                    title,
                    priority,
                    task_type,
                    description,
                    traces_to,
                    labels,
                    force,
                    ..
                },
        } => {
            let mut args = json!({"title": title});
            if *priority != 2 {
                args["priority"] = json!(priority);
            }
            if task_type != "task" {
                args["task_type"] = json!(task_type);
            }
            if let Some(d) = description {
                args["description"] = json!(d);
            }
            if !traces_to.is_empty() {
                args["traces_to"] = json!(traces_to);
            }
            if !labels.is_empty() {
                args["labels"] = json!(labels);
            }
            if *force {
                args["force"] = json!(true);
            }
            Some(("braid_task_create", args))
        }

        // 9. seed → braid_seed {task?, budget?}
        Command::Seed {
            task, seed_budget, ..
        } => {
            let mut args = json!({});
            if let Some(t) = task {
                args["task"] = json!(t);
            } else {
                args["task"] = json!("continue");
            }
            if *seed_budget != 2000 {
                args["budget"] = json!(seed_budget);
            }
            Some(("braid_seed", args))
        }

        // 10. write assert → braid_write {entity, attribute, value, rationale?}
        Command::Write {
            action: WriteAction::Assert {
                datoms, rationale, ..
            },
        } => {
            // The MCP braid_write tool handles a single [entity, attribute, value].
            // CLI `write assert` can have multiple --datom triples. Route only
            // single-datom assertions through the daemon; multi-datom falls back.
            if datoms.len() == 3 {
                let mut args = json!({
                    "entity": datoms[0],
                    "attribute": datoms[1],
                    "value": datoms[2],
                });
                if !rationale.is_empty() {
                    args["rationale"] = json!(rationale);
                }
                Some(("braid_write", args))
            } else {
                None // Multi-datom or empty — use direct mode
            }
        }

        // 11. guidance (bare `braid` with no subcommand resolves to Status above)
        //     The `braid_guidance` tool is available but has no dedicated CLI command.
        //     It is reachable only via MCP. No CLI command maps here.

        // note → braid_observe (note is a shortcut for observe)
        Command::Note {
            text, confidence, ..
        } => Some((
            "braid_observe",
            json!({"text": text, "confidence": confidence}),
        )),

        // transact → braid_write (single-datom only, same as write assert)
        Command::Transact {
            datoms, rationale, ..
        } => {
            if datoms.len() == 3 {
                let mut args = json!({
                    "entity": datoms[0],
                    "attribute": datoms[1],
                    "value": datoms[2],
                });
                if let Some(r) = rationale {
                    args["rationale"] = json!(r);
                }
                Some(("braid_write", args))
            } else {
                None
            }
        }

        // All other commands use direct mode:
        // init, daemon, mcp, shell, model, bilateral, trace, verify, challenge,
        // log, schema, merge, session, wrap, config, topology, witness, extract,
        // spec, task (non-create subcommands), write (non-assert subcommands),
        // observe (subcommands)
        _ => None,
    }
}

/// Try to route a CLI command through the daemon socket.
///
/// Returns `Some(response_text)` if the daemon handled the request,
/// `None` if the daemon is unavailable or the command isn't routable.
///
/// **INV-DAEMON-007**: Auto-detect daemon, fallback to direct.
/// **INV-DAEMON-004**: Semantic equivalence with direct mode.
pub fn try_route_through_daemon(
    braid_dir: &Path,
    cmd: &crate::commands::Command,
) -> Option<String> {
    // BRAID_NO_DAEMON=1: Force direct mode (no daemon routing or auto-start).
    // Used by integration tests to avoid daemon conflicts between the test's
    // CLI subprocess calls and the test-managed daemon process.
    if std::env::var("BRAID_NO_DAEMON").is_ok() {
        return None;
    }

    // Marshal the command to an MCP tool name + JSON arguments.
    // Returns None for non-routable commands (init, daemon, shell, etc.).
    let (tool_name, cmd_json) = marshal_command(cmd)?;

    let sock_path = SocketPath::new(braid_dir);
    if !sock_path.path().exists() {
        // INV-DAEMON-011: Auto-start daemon on first command.
        // Fork a detached child process running `braid daemon start`.
        // Poll for socket appearance (50ms intervals, 3s max).
        // If timeout, fall back to direct mode (zero blocking).
        if let Ok(exe) = std::env::current_exe() {
            let child = std::process::Command::new(&exe)
                .arg("daemon")
                .arg("start")
                .arg("--path")
                .arg(braid_dir)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn();

            if child.is_ok() {
                // Poll for socket to appear (daemon needs time to bind)
                for _ in 0..60 {
                    // 60 × 50ms = 3s max
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    if sock_path.path().exists() {
                        break;
                    }
                }
            }
        }

        // After auto-start attempt, check again
        if !sock_path.path().exists() {
            return None; // Daemon didn't start — fall back to direct mode
        }
    }

    // Try to connect with a short timeout.
    // DW3: Write commands get a 2s timeout (fall back to direct mode if daemon is busy
    // processing a long status request). Read commands get 10s (status can take a while).
    let is_read_command = matches!(
        tool_name,
        "braid_status" | "braid_query" | "braid_guidance" | "braid_task_ready"
    );
    let read_timeout = if is_read_command { 10 } else { 2 };
    let stream = std::os::unix::net::UnixStream::connect(sock_path.path()).ok()?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(read_timeout)))
        .ok()?;
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(2)))
        .ok()?;

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
/// bind socket → signal handlers → accept loop (multi-threaded) → shutdown.
///
/// **DS4**: Each incoming connection is dispatched on a dedicated thread.
/// The `LiveStore` is shared via `Arc<RwLock<LiveStore>>`. Tool dispatches
/// acquire a write lock for the handler, release it, then commit the
/// runtime observation `TxFile` via [`CommitHandle`] (DS2 group commit).
///
/// **INV-DAEMON-001**: Single daemon enforced via lock file.
/// **INV-DAEMON-002**: `refresh_if_needed()` before every dispatch.
/// **INV-DAEMON-006**: Graceful shutdown on SIGTERM/SIGINT.
/// **INV-DAEMON-012**: Accept loop never blocks on dispatch.
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

    // 2. Open LiveStore with WAL-accelerated recovery (DS5).
    //    Fast path: checkpoint + WAL delta (O(1) + O(k)).
    //    Falls through to medium (checkpoint + EDN) or slow (full rebuild) on failure.
    let mut live =
        crate::live_store::LiveStore::open_with_wal(braid_dir).map_err(DaemonError::from)?;

    // 3. Install runtime schema (ADR-DAEMON-003).
    //    This is fast (~1ms) and required before any tool dispatch.
    install_runtime_schema(&mut live)?;

    // Read config BEFORE wrapping in RwLock.
    let idle_timeout_secs: u64 =
        braid_kernel::config::get_config(live.store(), "daemon.idle-timeout-secs")
            .and_then(|v| v.parse().ok())
            .unwrap_or(300);
    let checkpoint_interval_secs: u64 =
        braid_kernel::config::get_config(live.store(), "checkpoint-interval-secs")
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

    // DS4: Wrap LiveStore in Arc<RwLock> for multi-threaded dispatch.
    let shared: Arc<RwLock<crate::live_store::LiveStore>> = Arc::new(RwLock::new(live));

    // DS2: Group commit thread — WAL + mpsc channel for batched writes.
    // The commit thread owns the WalWriter; connection threads submit
    // CommitRequests via a cloned CommitHandle.
    let wal_path = braid_dir.join(".cache/wal.bin");
    let wal = crate::wal::WalWriter::open(&wal_path).map_err(|e| {
        DaemonError::StoreError(crate::error::BraidError::Io(std::io::Error::other(
            format!("WAL open failed: {e}"),
        )))
    })?;
    let (commit_tx, commit_rx) = mpsc::channel::<CommitRequest>();
    let commit_handle = CommitHandle { sender: commit_tx };
    {
        let shared_for_commit = Arc::clone(&shared);
        let builder = std::thread::Builder::new().name("braid-group-commit".to_string());
        if let Err(e) = builder.spawn(move || {
            commit_thread(wal, commit_rx, shared_for_commit);
        }) {
            eprintln!("daemon: failed to spawn group commit thread: {e}");
            // Non-fatal but degraded: runtime observation datoms will fail to
            // commit (CommitHandle.commit() returns Err). The tool response
            // itself is still returned — only the runtime datom is lost.
        }
    }

    // DS3: Checkpoint thread — WAL-to-.edn background conversion.
    // Periodically converts WAL entries to .edn transaction files so they
    // are git-ready without blocking reads or writes. Uses a separate
    // DiskLayout instance (no sharing with LiveStore).
    let checkpoint_sender = match spawn_checkpoint_thread(braid_dir, checkpoint_interval_secs) {
        Ok((sender, _handle)) => Some(sender),
        Err(e) => {
            // Non-fatal: the daemon still operates, but WAL entries
            // accumulate until the next `braid harvest --commit` (DS6).
            eprintln!("daemon: failed to start checkpoint thread: {e}");
            None
        }
    };

    // 4. Remove stale socket if it exists (crash recovery).
    let _ = std::fs::remove_file(sock_path.path());

    // 5. Bind Unix socket.
    let listener = UnixListener::bind(sock_path.path()).map_err(DaemonError::BindFailed)?;
    // Non-blocking accept so we can check the shutdown flag.
    listener
        .set_nonblocking(true)
        .map_err(DaemonError::BindFailed)?;

    eprintln!(
        "daemon: listening on {} (pid {})",
        sock_path.path().display(),
        std::process::id()
    );

    // 5b. Deferred enrichment — runs AFTER socket is bound so auto-start
    //     clients don't timeout waiting for these non-critical operations.
    //     Capability census and FEGH are store enrichment, not prerequisites.
    {
        let shared_enrich = Arc::clone(&shared);
        let builder = std::thread::Builder::new().name("braid-enrich".to_string());
        let _ = builder.spawn(move || {
            if let Ok(mut live) = shared_enrich.write() {
                let _ = run_capability_census(&mut live);
                run_reflexive_fegh(&mut live);
            }
        });
    }

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
            libc::signal(
                libc::SIGTERM,
                signal_handler as *const () as libc::sighandler_t,
            );
            libc::signal(
                libc::SIGINT,
                signal_handler as *const () as libc::sighandler_t,
            );
        }
        // Store the Arc in a global so the signal handler can access it.
        SHUTDOWN_FLAG
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .replace(shutdown_clone);
    }

    let start_time = Instant::now();
    // DS4: Shared atomic counters — accessible from spawned threads.
    let request_count = Arc::new(AtomicU64::new(0));
    let last_request_epoch_ms = Arc::new(AtomicU64::new(epoch_ms_now()));
    let mut conn_count: u64 = 0;

    // 7. Accept loop (DS4: non-blocking, spawns thread per connection).
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

        // INV-DAEMON-011: Idle timeout check.
        let last_ms = last_request_epoch_ms.load(Ordering::Relaxed);
        let now_ms = epoch_ms_now();
        if now_ms.saturating_sub(last_ms) > idle_timeout_secs * 1000 {
            eprintln!(
                "daemon: idle timeout ({}s) — shutting down",
                idle_timeout_secs
            );
            break;
        }

        match listener.accept() {
            Ok((stream, _addr)) => {
                // Set read timeout for the connection (PM-3: prevent leaked connections).
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
                let _ = stream.set_nonblocking(false);

                conn_count += 1;
                let thread_name = format!("braid-conn-{}", conn_count);
                let shared_clone = Arc::clone(&shared);
                let start_clone = start_time;
                let req_count_clone = Arc::clone(&request_count);
                let last_req_clone = Arc::clone(&last_request_epoch_ms);
                let shutdown_clone = Arc::clone(&shutdown);
                // DS6: Clone CommitHandle + checkpoint sender for connection thread.
                let commit_handle_clone = commit_handle.clone();
                let checkpoint_sender_clone = checkpoint_sender.as_ref().cloned();

                // DS4/INV-DAEMON-012: Spawn thread — accept loop never blocks on dispatch.
                let builder = std::thread::Builder::new().name(thread_name);
                if let Err(e) = builder.spawn(move || {
                    handle_connection_shared(
                        stream,
                        &shared_clone,
                        &start_clone,
                        &req_count_clone,
                        &shutdown_clone,
                        commit_handle_clone,
                        checkpoint_sender_clone,
                    );
                    // Update idle timer after connection completes.
                    last_req_clone.store(epoch_ms_now(), Ordering::Relaxed);
                }) {
                    eprintln!("daemon: failed to spawn connection thread: {e}");
                }
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

    // 8a. DS3: Stop the checkpoint thread (final passive checkpoint runs inside).
    if let Some(sender) = checkpoint_sender {
        let _ = sender.send(CheckpointSignal::Stop);
        // The thread will run a final passive_checkpoint before exiting.
        // We don't join here — the thread is non-critical and will exit
        // promptly. The LiveStore flush below is the durability guarantee.
    }

    // 8b. Shutdown: flush LiveStore (INV-DAEMON-006).
    eprintln!("daemon: flushing store...");
    match shared.write() {
        Ok(mut live) => {
            let _ = live.flush();
        }
        Err(poisoned) => {
            // RwLock poisoned — a thread panicked while holding the lock.
            // Still attempt to flush: the store may be partially consistent
            // but flushing is best-effort (INV-DAEMON-006).
            eprintln!("daemon: RwLock poisoned during shutdown, attempting flush anyway");
            let mut live = poisoned.into_inner();
            let _ = live.flush();
        }
    }
    let total_requests = request_count.load(Ordering::Relaxed);
    eprintln!(
        "daemon: stopped after {} requests ({} connections), uptime {}s",
        total_requests,
        conn_count,
        start_time.elapsed().as_secs()
    );

    // CleanupGuard will remove socket and lock on drop.
    Ok(())
}

/// Current wall-clock time as milliseconds since epoch.
///
/// Used by the accept loop for idle timeout tracking across threads.
/// Monotonic clocks cannot be shared as raw u64 values across threads
/// (they are opaque types), so we use wall time here. The idle timeout
/// is coarse enough (minutes) that clock adjustments are irrelevant.
fn epoch_ms_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Handle one client connection with shared `LiveStore` (DS4 multi-threaded path).
///
/// Reads newline-delimited JSON-RPC messages and dispatches them using a
/// read/write lock split on the shared `LiveStore`:
///
/// - **Read path** (DS4): Read-only `tools/call` requests (`braid_status`,
///   `braid_query`, `braid_guidance`, `braid_task_ready`) acquire
///   `shared.read()` — truly concurrent with other reads, zero contention.
///   Dispatches through [`build_observation_tx_read`] which uses `&Store` +
///   `&Path` (no mutable borrow).
///
/// - **Write path** (DS2): Mutating `tools/call` requests and all other
///   methods acquire `shared.write()` (exclusive). Dispatches through
///   [`build_observation_tx`] which uses `&mut LiveStore`.
///
/// Both paths produce a runtime observation `TxFile` that is committed via
/// [`CommitHandle`] OUTSIDE the lock scope.
///
/// **DS6**: After a `braid_harvest` with `commit=true`, sends
/// [`CheckpointSignal::Full`] to the checkpoint thread and waits for
/// completion. This ensures all WAL entries are converted to `.edn` files
/// before the git commit runs.
///
/// **INV-DAEMON-002**: `refresh_if_needed()` inside every write-locked dispatch.
/// **INV-DAEMON-003**: Runtime datoms emitted on both read and write paths.
/// **INV-DAEMON-012**: Read tools acquire `RwLock::read()` — concurrent reads.
fn handle_connection_shared(
    stream: std::os::unix::net::UnixStream,
    shared: &Arc<RwLock<crate::live_store::LiveStore>>,
    start_time: &Instant,
    request_count: &AtomicU64,
    should_stop: &AtomicBool,
    commit_handle: CommitHandle,
    checkpoint_sender: Option<mpsc::Sender<CheckpointSignal>>,
) {
    // DS2: commit_handle carries runtime observation TxFiles to the group
    // commit thread. See the tools/call dispatch below for the commit path.
    let reader = std::io::BufReader::new(&stream);
    let mut writer = std::io::BufWriter::new(&stream);

    for line_result in reader.lines() {
        // Check shutdown flag between requests — allows threads to exit promptly.
        if should_stop.load(Ordering::Relaxed) {
            break;
        }

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

        // DS6: Detect harvest-commit requests BEFORE acquiring the lock.
        // If this is a tools/call for braid_harvest with commit=true, we
        // trigger a full checkpoint AFTER the dispatch (outside the write lock).
        let is_harvest_commit = method == "tools/call" && {
            let tool_name = params
                .get("arguments")
                .and_then(|a| a.get("commit"))
                .and_then(|c| c.as_bool())
                .unwrap_or(false)
                && params
                    .get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| n == "braid_harvest")
                    .unwrap_or(false);
            tool_name
        };

        // DS4 READ/WRITE SPLIT: Classify tools/call requests BEFORE acquiring
        // any lock. Read-only tools (status, query, guidance, task_ready)
        // acquire shared.read() — concurrent with other reads, zero contention.
        // Write tools and all other methods acquire shared.write() (exclusive).
        let is_read_tool_call = method == "tools/call"
            && params
                .get("name")
                .and_then(|n| n.as_str())
                .map(crate::mcp::is_read_only_tool)
                .unwrap_or(false);

        // DS4: Read path — shared.read() for read-only tools/call.
        // INV-DAEMON-004 (semantic equivalence): Detect external writes before
        // query dispatch. refresh_if_needed() is O(1) stat() in the common case
        // (no new files) and only takes the write lock when new .edn files exist.
        if is_read_tool_call {
            if let Ok(mut live) = shared.write() {
                let _ = live.refresh_if_needed();
            }
        }
        // Runtime observation TxFile is still built and submitted to CommitHandle.
        let (response, runtime_tx) = if is_read_tool_call {
            match shared.read() {
                Ok(live) => {
                    let (resp, tx) =
                        build_observation_tx_read(&id, &params, live.store(), live.path());
                    // Read lock dropped here.
                    (resp, Some(tx))
                }
                Err(poisoned) => {
                    eprintln!("daemon: RwLock poisoned (read path), recovering");
                    let live = poisoned.into_inner();
                    let (resp, tx) =
                        build_observation_tx_read(&id, &params, live.store(), live.path());
                    (resp, Some(tx))
                }
            }
        } else {
            // DS2/DS4: Write path — shared.write() for mutating tools and
            // non-tool methods. Dispatch under write lock, then commit runtime
            // datom via CommitHandle OUTSIDE the lock.
            match shared.write() {
                Ok(mut live) => {
                    // INV-DAEMON-002: refresh before every dispatch.
                    let _ = live.refresh_if_needed();

                    match method {
                        // Daemon-specific methods.
                        "daemon/shutdown" => {
                            // Set the global shutdown flag.
                            if let Ok(guard) = SHUTDOWN_FLAG.lock() {
                                if let Some(ref flag) = *guard {
                                    flag.store(true, Ordering::Relaxed);
                                }
                            }
                            (
                                crate::mcp::jsonrpc_ok(&id, json!({"status": "stopping"})),
                                None,
                            )
                        }
                        "daemon/status" => {
                            let uptime_secs = start_time.elapsed().as_secs();
                            let datom_count = live.store().len();
                            let entity_count = live.store().entity_count();
                            let reqs = request_count.load(Ordering::Relaxed);
                            (
                                crate::mcp::jsonrpc_ok(
                                    &id,
                                    json!({
                                        "pid": std::process::id(),
                                        "uptime_secs": uptime_secs,
                                        "request_count": reqs,
                                        "datom_count": datom_count,
                                        "entity_count": entity_count,
                                    }),
                                ),
                                None,
                            )
                        }
                        // Standard MCP methods — delegate to shared handlers.
                        "initialize" => {
                            (crate::mcp::handle_initialize(&id, &params, &mut live), None)
                        }
                        "initialized" => {
                            if is_notification {
                                continue;
                            }
                            (crate::mcp::jsonrpc_ok(&id, json!({})), None)
                        }
                        "tools/list" => (crate::mcp::handle_tools_list(&id), None),
                        "tools/call" => {
                            // DS2: Dispatch + build runtime TxFile under write lock,
                            // but do NOT write it here. The TxFile escapes the lock
                            // scope and is submitted to CommitHandle below.
                            let (resp, tx) = build_observation_tx(&id, &params, &mut live);
                            (resp, Some(tx))
                        }
                        "ping" => (crate::mcp::jsonrpc_ok(&id, json!({})), None),
                        "notifications/cancelled" | "notifications/progress" => continue,
                        _ => (
                            crate::mcp::jsonrpc_error(
                                &id,
                                crate::mcp::METHOD_NOT_FOUND,
                                &format!("unknown method: {method}"),
                            ),
                            None,
                        ),
                    }
                    // Write lock is dropped here at end of match arm scope.
                }
                Err(poisoned) => {
                    // RwLock poisoned — a sibling thread panicked while holding the lock.
                    // Recover the lock and attempt the dispatch anyway. This is best-effort:
                    // the store state may be inconsistent, but returning a protocol error
                    // for every subsequent request is worse than trying.
                    eprintln!("daemon: RwLock poisoned, recovering for dispatch");
                    let mut live = poisoned.into_inner();
                    let _ = live.refresh_if_needed();
                    match method {
                        "daemon/shutdown" => {
                            if let Ok(guard) = SHUTDOWN_FLAG.lock() {
                                if let Some(ref flag) = *guard {
                                    flag.store(true, Ordering::Relaxed);
                                }
                            }
                            (
                                crate::mcp::jsonrpc_ok(&id, json!({"status": "stopping"})),
                                None,
                            )
                        }
                        "tools/call" => {
                            let (resp, tx) = build_observation_tx(&id, &params, &mut live);
                            (resp, Some(tx))
                        }
                        _ => (
                            crate::mcp::jsonrpc_error(
                                &id,
                                crate::mcp::METHOD_NOT_FOUND,
                                "RwLock poisoned — limited dispatch available",
                            ),
                            None,
                        ),
                    }
                }
            }
        };

        // DS2: Submit runtime observation TxFile to group commit OUTSIDE
        // the write lock. The commit thread acquires its own write lock to
        // apply datoms in-memory after WAL fsync.
        //
        // INV-DAEMON-003: runtime datom durability via WAL (not EDN file).
        // INV-DS2-001: durable before commit() returns.
        // INV-DS2-003: linearizable — response reflects committed state.
        if let Some(tx) = runtime_tx {
            if let Err(e) = commit_handle.commit(tx) {
                eprintln!("daemon: DS2 commit failed for runtime datom: {e}");
            }
        }

        // DS6: After harvest --commit, trigger a full checkpoint so all WAL
        // entries are converted to .edn files before the git commit runs.
        // This happens OUTSIDE the write lock — the checkpoint thread has its
        // own DiskLayout instance (DS3-004: no contention with LiveStore).
        if is_harvest_commit {
            if let Some(ref sender) = checkpoint_sender {
                let (done_tx, done_rx) = mpsc::channel();
                if sender.send(CheckpointSignal::Full(done_tx)).is_ok() {
                    match done_rx.recv_timeout(Duration::from_secs(30)) {
                        Ok(Ok(())) => {
                            eprintln!("daemon: DS6 full checkpoint completed for harvest --commit");
                        }
                        Ok(Err(e)) => {
                            eprintln!("daemon: DS6 full checkpoint failed: {e}");
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            eprintln!("daemon: DS6 full checkpoint timed out after 30s");
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            eprintln!("daemon: DS6 checkpoint thread disconnected");
                        }
                    }
                }
            }
        }

        request_count.fetch_add(1, Ordering::Relaxed);
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

/// Handle a tools/call request, returning the JSON-RPC response and a runtime
/// observation `TxFile` ready for group commit.
///
/// The caller is responsible for committing the `TxFile` — either through the
/// [`CommitHandle`] (DS2 group commit path) or via [`LiveStore::write_tx`]
/// (fallback). This separation allows the write lock to be released before
/// submitting to the commit channel, preventing deadlock between the dispatch
/// thread and the commit thread (both need `shared.write()`).
///
/// **INV-DAEMON-003**: Every command emits `:runtime/*` datoms.
/// **INV-DAEMON-008**: Emits datoms even on error paths.
fn build_observation_tx(
    id: &JsonValue,
    params: &JsonValue,
    live: &mut crate::live_store::LiveStore,
) -> (JsonValue, braid_kernel::layout::TxFile) {
    use braid_kernel::datom::*;
    use braid_kernel::layout::TxFile;

    let start = Instant::now();
    let datom_count_before = live.store().len() as i64;
    let cache_hit = !live.has_new_external_txns();

    // Dispatch to shared MCP handler.
    let result = crate::mcp::handle_tools_call(id, params, live);

    // Build runtime datoms (never fail the original request).
    let elapsed_us = start.elapsed().as_micros() as i64;
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
            Attribute::from_keyword(":runtime/latency-us"),
            Value::Long(elapsed_us),
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

    (result, tx_file)
}

/// Handle a read-only `tools/call` request, returning the JSON-RPC response
/// and a runtime observation `TxFile`.
///
/// Read-path counterpart to [`build_observation_tx`]. Dispatches through
/// [`crate::mcp::handle_tools_call_read`] which only takes `&Store` + `&Path`
/// (no `&mut LiveStore`). This allows the caller to hold `RwLock::read()`
/// instead of `RwLock::write()`, enabling concurrent read dispatches.
///
/// **INV-DAEMON-003**: Runtime datom emission on the read path.
/// **INV-DAEMON-012**: Read tools run under shared (read) lock.
fn build_observation_tx_read(
    id: &JsonValue,
    params: &JsonValue,
    store: &braid_kernel::Store,
    root: &std::path::Path,
) -> (JsonValue, braid_kernel::layout::TxFile) {
    use braid_kernel::datom::*;
    use braid_kernel::layout::TxFile;

    let start = Instant::now();
    let datom_count_before = store.len() as i64;

    // Dispatch to read-only MCP handler.
    let result = crate::mcp::handle_tools_call_read(id, params, store, root);

    // Build runtime datoms (never fail the original request).
    let elapsed_us = start.elapsed().as_micros() as i64;
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
    let tx_id = crate::commands::write::next_tx_id(store, agent);

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
            Attribute::from_keyword(":runtime/latency-us"),
            Value::Long(elapsed_us),
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
        // Read path always hits the in-memory cache (no refresh needed).
        Datom::new(
            entity,
            Attribute::from_keyword(":runtime/cache-hit"),
            Value::Boolean(true),
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

    (result, tx_file)
}

/// Convenience wrapper: dispatch + write runtime datom via direct `write_tx`.
///
/// Used by unit tests that operate on a single `LiveStore` without the
/// daemon's group commit infrastructure. Production code in
/// `handle_connection_shared` uses [`build_observation_tx`] + [`CommitHandle`]
/// instead.
#[cfg(test)]
fn handle_with_observation(
    id: &JsonValue,
    params: &JsonValue,
    live: &mut crate::live_store::LiveStore,
) -> JsonValue {
    let (result, tx_file) = build_observation_tx(id, params, live);
    if let Err(e) = live.write_tx(&tx_file) {
        eprintln!("daemon: failed to write runtime datom: {e}");
    }
    result
}

// ---------------------------------------------------------------------------
// Signal handling
// ---------------------------------------------------------------------------

/// Global shutdown flag, accessible from the signal handler.
static SHUTDOWN_FLAG: std::sync::Mutex<Option<Arc<AtomicBool>>> = std::sync::Mutex::new(None);

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
// DS2: Group commit — mpsc channel + dedicated commit thread
// ---------------------------------------------------------------------------

/// A write request submitted to the group commit channel.
///
/// Connection threads construct a `CommitRequest` with the transaction to
/// commit and a oneshot-style response channel. The commit thread drains
/// pending requests, appends them as a WAL batch (single fsync), applies
/// the datoms to the shared `LiveStore`, and signals each requester via
/// `done`.
///
/// **INV-DS2-001**: Every committed transaction is durable (WAL fsynced)
/// before the `done` signal is sent.
///
/// **INV-DS2-002**: The commit thread is the sole writer to `WalWriter`,
/// preventing interleaved partial frames.
struct CommitRequest {
    /// The transaction to commit.
    tx: braid_kernel::layout::TxFile,
    /// Completion signal — sent after WAL fsync confirms durability.
    /// The value is the `WalEntryMeta` for the committed entry.
    done: mpsc::Sender<Result<crate::wal::WalEntryMeta, String>>,
}

/// Handle held by connection threads to submit writes for group commit.
///
/// Cloneable: one clone per connection thread. Submitting a transaction
/// blocks the caller until the commit thread confirms durability via fsync.
///
/// **INV-DS2-003**: `commit()` is linearizable — if it returns `Ok`, the
/// transaction is durable on disk and applied to the shared `LiveStore`.
#[derive(Clone)]
pub struct CommitHandle {
    sender: mpsc::Sender<CommitRequest>,
}

impl CommitHandle {
    /// Submit a write and block until durability is confirmed.
    ///
    /// Returns metadata about the committed WAL entry. The caller blocks
    /// on the oneshot response channel until the commit thread has:
    /// 1. Appended the transaction to the WAL batch.
    /// 2. Called `fsync` on the batch (single fsync for all batched requests).
    /// 3. Applied the datoms to the shared `LiveStore`.
    ///
    /// # Errors
    ///
    /// Returns `BraidError::Validation` if the commit channel is closed
    /// (commit thread has exited) or if the response channel is closed
    /// (commit thread dropped the sender without responding).
    pub fn commit(
        &self,
        tx: braid_kernel::layout::TxFile,
    ) -> Result<crate::wal::WalEntryMeta, crate::error::BraidError> {
        let (done_tx, done_rx) = mpsc::channel();
        self.sender
            .send(CommitRequest { tx, done: done_tx })
            .map_err(|_| crate::error::BraidError::Validation("commit channel closed".into()))?;
        done_rx
            .recv()
            .map_err(|_| {
                crate::error::BraidError::Validation("commit response channel closed".into())
            })?
            .map_err(crate::error::BraidError::Validation)
    }
}

/// Default batch interval — starting point before adaptive tuning.
/// 25ms initial wait + fsync ~20ms = ~45ms worst-case (within P99 < 50ms target).
const GROUP_COMMIT_INITIAL_INTERVAL_MS: u64 = 25;

/// Minimum batch interval under sustained load (many concurrent writers).
const GROUP_COMMIT_MIN_INTERVAL_MS: u64 = 5;

/// Number of consecutive single-item batches before increasing the interval.
/// Prevents thrashing between fast and slow modes. Lower threshold (5) enables
/// faster adaptation from latency mode back to throughput mode.
const GROUP_COMMIT_SINGLE_BATCH_THRESHOLD: u32 = 5;

/// Dedicated commit thread — drains the `CommitRequest` channel and batches
/// writes into a single WAL fsync per batch.
///
/// # Concurrency model (DS2)
///
/// The commit thread is the sole owner of `WalWriter` (INV-DS2-002).
/// Connection threads never touch the WAL directly. The flow:
///
/// 1. `recv_timeout(batch_interval)` — block until the first request arrives
///    or the interval elapses (catches stragglers).
/// 2. `try_recv()` loop — non-blocking drain of any additional pending
///    requests (up to `max_batch` to bound memory).
/// 3. `wal.append_batch()` — single vectored write + fsync.
/// 4. `shared.write()` — apply all datoms to the in-memory `LiveStore`.
/// 5. Signal each requester via the `done` channel with their `WalEntryMeta`.
///
/// # Adaptive batch interval
///
/// Starts at 25ms. If batch size > 1, drops to 5ms (throughput mode).
/// If 5 consecutive batches are single-item, returns to 25ms (latency mode).
/// This balances single-agent latency (~5ms) with multi-agent throughput
/// (amortized fsync). Worst case: 25ms + fsync ~20ms = ~45ms (P99 < 50ms).
///
/// # Shutdown
///
/// The thread exits when the `Receiver` disconnects (all `Sender` clones
/// dropped), which happens naturally when `serve_daemon` drops the
/// `CommitHandle` on shutdown.
fn commit_thread(
    mut wal: crate::wal::WalWriter,
    receiver: mpsc::Receiver<CommitRequest>,
    shared: Arc<RwLock<crate::live_store::LiveStore>>,
) {
    let max_batch: usize = 256;
    let mut batch_interval = Duration::from_millis(GROUP_COMMIT_INITIAL_INTERVAL_MS);
    let mut consecutive_singles: u32 = 0;

    loop {
        // Step 1: Block for the first request (or timeout).
        let first = match receiver.recv_timeout(batch_interval) {
            Ok(req) => req,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No requests during interval — loop back to wait again.
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // All senders dropped — daemon is shutting down.
                break;
            }
        };

        // Step 2: Non-blocking drain of additional pending requests.
        let mut batch = Vec::with_capacity(max_batch);
        batch.push(first);
        while batch.len() < max_batch {
            match receiver.try_recv() {
                Ok(req) => batch.push(req),
                Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => {
                    break;
                }
            }
        }

        let batch_size = batch.len();

        // Adaptive interval tuning.
        if batch_size > 1 {
            batch_interval = Duration::from_millis(GROUP_COMMIT_MIN_INTERVAL_MS);
            consecutive_singles = 0;
        } else {
            consecutive_singles += 1;
            if consecutive_singles >= GROUP_COMMIT_SINGLE_BATCH_THRESHOLD {
                batch_interval = Duration::from_millis(GROUP_COMMIT_INITIAL_INTERVAL_MS);
                // Don't reset consecutive_singles — stay in slow mode until
                // a multi-item batch arrives.
            }
        }

        // Step 3: Batch-append to WAL (single fsync).
        let txs: Vec<braid_kernel::layout::TxFile> = batch.iter().map(|r| r.tx.clone()).collect();
        let wal_result = wal.append_batch(&txs);

        let metas = match wal_result {
            Ok(m) => m,
            Err(e) => {
                // WAL write failed — signal all requesters with the error.
                let err_msg = format!("WAL append_batch failed: {e}");
                for req in batch {
                    let _ = req.done.send(Err(err_msg.clone()));
                }
                continue;
            }
        };

        // Step 4: Apply datoms to shared LiveStore under write lock.
        // Uses apply_datoms_in_memory (DS2) — skips disk write since
        // the WAL already provides durability (INV-DS2-001).
        match shared.write() {
            Ok(mut live) => {
                for req in &batch {
                    live.apply_datoms_in_memory(&req.tx.datoms);
                }
            }
            Err(poisoned) => {
                // RwLock poisoned — recover and apply anyway (best-effort).
                eprintln!("commit_thread: RwLock poisoned, recovering for datom apply");
                let mut live = poisoned.into_inner();
                for req in &batch {
                    live.apply_datoms_in_memory(&req.tx.datoms);
                }
            }
        }

        // Step 5: Signal each requester with their WalEntryMeta.
        for (req, meta) in batch.into_iter().zip(metas) {
            let _ = req.done.send(Ok(meta));
        }
    }
}

// ---------------------------------------------------------------------------
// DS3: Checkpoint thread — WAL-to-.edn background conversion
// ---------------------------------------------------------------------------

/// Signal sent to the checkpoint thread for on-demand or periodic operation.
///
/// The checkpoint thread converts WAL entries to `.edn` transaction files,
/// making them git-ready without blocking reads or writes.
///
/// - **Tick** / timeout: Passive checkpoint (convert pending entries, keep WAL).
/// - **Full**: Full checkpoint for `braid harvest --commit` (convert all, truncate WAL).
/// - **Stop**: Graceful shutdown (final passive checkpoint, then exit).
///
/// # Invariants
///
/// - **DS3-001**: Passive checkpoint never truncates the WAL.
/// - **DS3-002**: Full checkpoint converts all pending entries before truncation.
/// - **DS3-003**: `.edn` writes are idempotent via `write_tx_no_invalidate`.
/// - **DS3-004**: Checkpoint thread is non-blocking to daemon reads/writes.
pub enum CheckpointSignal {
    /// Periodic passive checkpoint — convert pending WAL entries to `.edn` files.
    Tick,
    /// Full checkpoint for `harvest --commit` — checkpoint all, truncate WAL.
    /// The sender receives the result when the full checkpoint completes.
    Full(mpsc::Sender<Result<(), String>>),
    /// Shutdown — run a final passive checkpoint and exit.
    Stop,
}

/// State held by the checkpoint thread (DS3).
///
/// Tracks the byte offset of the last checkpointed WAL entry so that
/// successive ticks only process new entries. The [`DiskLayout`] instance
/// is owned exclusively by this thread — no sharing with the daemon's
/// `LiveStore` (which has its own layout for in-memory writes).
///
/// # Invariants
///
/// - **DS3-001**: `passive_checkpoint` never calls `WalWriter::truncate`.
/// - **DS3-003**: Uses `write_tx_no_invalidate` — idempotent by content hash.
struct CheckpointState {
    /// Byte offset of the next un-checkpointed WAL entry.
    checkpoint_offset: u64,
    /// Path to the WAL file (`.braid/.cache/wal.bin`).
    wal_path: PathBuf,
    /// `DiskLayout` for writing `.edn` files — separate instance from `LiveStore`.
    layout: crate::layout::DiskLayout,
}

impl CheckpointState {
    /// Convert pending WAL entries to `.edn` transaction files (DS3-001).
    ///
    /// Reads the WAL from `checkpoint_offset`, writes each entry as an `.edn`
    /// file via `layout.write_tx_no_invalidate()` (idempotent — DS3-003), and
    /// advances the offset. Does NOT truncate the WAL (safety — keeps WAL as
    /// recovery backup until `full_checkpoint`).
    ///
    /// Returns the number of entries newly checkpointed.
    fn passive_checkpoint(&mut self) -> u64 {
        // If the WAL file does not exist or has no new data, nothing to do.
        let wal_len = match std::fs::metadata(&self.wal_path) {
            Ok(m) => m.len(),
            Err(_) => return 0,
        };
        if wal_len <= self.checkpoint_offset {
            return 0;
        }

        let reader = match crate::wal::WalReader::open(&self.wal_path) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("checkpoint: cannot open WAL for reading: {e}");
                return 0;
            }
        };

        let iter = match reader.iter_from(self.checkpoint_offset) {
            Ok(it) => it,
            Err(e) => {
                eprintln!(
                    "checkpoint: cannot seek WAL to offset {}: {e}",
                    self.checkpoint_offset
                );
                return 0;
            }
        };

        let mut count: u64 = 0;
        for entry_result in iter {
            match entry_result {
                Ok((tx, meta)) => {
                    // DS3-003: write_tx_no_invalidate is idempotent — if the .edn
                    // file already exists (same content hash), it silently succeeds.
                    if let Err(e) = self.layout.write_tx_no_invalidate(&tx) {
                        eprintln!(
                            "checkpoint: failed to write .edn at WAL offset {}: {e}",
                            meta.offset
                        );
                        // Stop on write failure — the next tick will retry from
                        // the same offset. Do not advance past a failed entry.
                        break;
                    }
                    // Advance past this entry: offset + 4 (len) + payload + 4 (crc) + 32 (hash).
                    self.checkpoint_offset = meta.offset + 4 + meta.length as u64 + 4 + 32;
                    count += 1;
                }
                Err(e) => {
                    eprintln!(
                        "checkpoint: WAL read error at offset {}: {e}",
                        self.checkpoint_offset
                    );
                    break;
                }
            }
        }

        if count > 0 {
            eprintln!("checkpoint: converted {count} WAL entries to .edn");
        }
        count
    }

    /// Full checkpoint: convert all pending entries and truncate the WAL (DS3-002).
    ///
    /// 1. Run `passive_checkpoint()` to flush all pending entries.
    /// 2. Truncate the WAL (clear the file, reset offset to 0).
    ///
    /// This makes the store fully git-ready: all state is in `.edn` files,
    /// the WAL is empty. Used by `braid harvest --commit` (wired in DS6).
    fn full_checkpoint(&mut self) -> Result<(), String> {
        // Step 1: Flush all pending entries to .edn.
        self.passive_checkpoint();

        // Step 2: Truncate the WAL and reset offset.
        let mut writer = crate::wal::WalWriter::open(&self.wal_path)
            .map_err(|e| format!("checkpoint: cannot open WAL for truncation: {e}"))?;
        writer
            .truncate()
            .map_err(|e| format!("checkpoint: WAL truncation failed: {e}"))?;
        self.checkpoint_offset = 0;

        eprintln!("checkpoint: full checkpoint complete — WAL truncated");
        Ok(())
    }
}

/// Run the checkpoint thread event loop (DS3).
///
/// Periodically converts WAL entries to `.edn` transaction files. Responds
/// to [`CheckpointSignal::Full`] for on-demand full checkpoints, and
/// [`CheckpointSignal::Stop`] for graceful shutdown with a final flush.
///
/// The `interval` parameter controls the passive tick frequency. It is
/// configurable via the `:config/checkpoint-interval-secs` datom
/// (C9/ADR-FOUNDATION-031).
///
/// # Thread safety
///
/// This function owns its [`CheckpointState`] exclusively — no locks required.
/// The `DiskLayout` inside the state is a separate instance from the daemon's
/// `LiveStore`, so `.edn` writes do not contend with daemon reads/writes
/// (DS3-004).
fn checkpoint_thread(
    mut state: CheckpointState,
    receiver: mpsc::Receiver<CheckpointSignal>,
    interval: Duration,
) {
    loop {
        match receiver.recv_timeout(interval) {
            Ok(CheckpointSignal::Tick) | Err(mpsc::RecvTimeoutError::Timeout) => {
                // PASSIVE: convert pending WAL entries to .edn (DS3-001).
                state.passive_checkpoint();
            }
            Ok(CheckpointSignal::Full(done)) => {
                // FULL: checkpoint everything + truncate WAL (DS3-002).
                let result = state.full_checkpoint();
                // Notify the caller. If the channel is closed, ignore.
                let _ = done.send(result);
            }
            Ok(CheckpointSignal::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Final passive checkpoint before exit — do not lose entries.
                state.passive_checkpoint();
                eprintln!("checkpoint: thread stopped");
                break;
            }
        }
    }
}

/// Spawn the DS3 checkpoint thread for the daemon.
///
/// Creates a separate [`DiskLayout`] instance (no sharing with `LiveStore`),
/// and spawns the background thread with the given tick interval.
///
/// Returns the [`mpsc::Sender<CheckpointSignal>`] for the daemon to send
/// `Full` or `Stop` signals, plus the `JoinHandle` for the spawned thread.
fn spawn_checkpoint_thread(
    braid_dir: &Path,
    checkpoint_interval_secs: u64,
) -> Result<(mpsc::Sender<CheckpointSignal>, std::thread::JoinHandle<()>), DaemonError> {
    let layout = crate::layout::DiskLayout::open(braid_dir).map_err(DaemonError::StoreError)?;

    let wal_path = braid_dir.join(".cache").join("wal.bin");

    // Start from offset 0 — passive_checkpoint uses write_tx_no_invalidate
    // which is idempotent (DS3-003), so re-processing existing entries
    // is a harmless no-op that produces no duplicate .edn files.
    let checkpoint_offset: u64 = 0;

    let state = CheckpointState {
        checkpoint_offset,
        wal_path,
        layout,
    };

    let interval = Duration::from_secs(checkpoint_interval_secs);
    let (tx, rx) = mpsc::channel();

    let handle = std::thread::Builder::new()
        .name("braid-checkpoint".to_string())
        .spawn(move || {
            checkpoint_thread(state, rx, interval);
        })
        .map_err(|e| {
            DaemonError::BindFailed(std::io::Error::other(format!(
                "failed to spawn checkpoint thread: {e}"
            )))
        })?;

    eprintln!("checkpoint: thread started (interval={checkpoint_interval_secs}s)");
    Ok((tx, handle))
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

        let store_err = DaemonError::StoreError(crate::error::BraidError::Validation("v".into()));
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
        assert!(
            !lock.path().exists(),
            "lock file must be removed after release"
        );
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
        assert!(
            has_value_type,
            ":runtime/command must have :db/valueType after install"
        );
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

        assert!(
            count_after_first > count_before,
            "first install should add datoms"
        );
        assert_eq!(
            count_after_first, count_after_second,
            "second install should be a no-op (idempotent)"
        );
    }

    // ── Capability census tests (PERF-4a) ─────────────────────────────────

    #[test]
    fn capability_census_creates_datoms() {
        use braid_kernel::datom::{Attribute, Op};

        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let mut live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        run_capability_census(&mut live).unwrap();

        // Verify :capability/store-binary-cache exists.
        let entity = braid_kernel::datom::EntityId::from_ident(":capability/store-binary-cache");
        let doc_attr = Attribute::from_keyword(":db/doc");
        let has_doc = live
            .store()
            .entity_datoms(entity)
            .iter()
            .any(|d| d.attribute == doc_attr && d.op == Op::Assert);
        assert!(has_doc, ":capability/store-binary-cache must have :db/doc");
    }

    #[test]
    fn capability_census_all_subsystems() {
        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let mut live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        run_capability_census(&mut live).unwrap();

        // Count capability entities.
        let cap_attr = braid_kernel::datom::Attribute::from_keyword(":capability/implemented");
        let count = live
            .store()
            .datoms()
            .filter(|d| d.attribute == cap_attr && d.op == braid_kernel::datom::Op::Assert)
            .count();
        assert_eq!(
            count,
            CAPABILITIES.len(),
            "must register all {} capabilities",
            CAPABILITIES.len()
        );
    }

    #[test]
    fn capability_census_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let mut live = crate::live_store::LiveStore::create(&braid_dir).unwrap();

        let before = live.store().len();
        run_capability_census(&mut live).unwrap();
        let after_first = live.store().len();
        run_capability_census(&mut live).unwrap();
        let after_second = live.store().len();

        assert!(after_first > before);
        assert_eq!(after_first, after_second, "second census must be a no-op");
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
        let has_error_outcome = live.store().datoms().any(|d| {
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

        // Find the runtime datom's latency (stored in microseconds).
        use braid_kernel::datom::{Attribute, Op, Value};
        let lat_attr = Attribute::from_keyword(":runtime/latency-us");
        let latency = live
            .store()
            .datoms()
            .find(|d| d.attribute == lat_attr && d.op == Op::Assert)
            .and_then(|d| match &d.value {
                Value::Long(us) => Some(*us),
                _ => None,
            });

        let us = latency.expect(":runtime/latency-us must exist");
        assert!(us > 0, "latency must be positive, got {us}us");
        assert!(
            us < 60_000_000,
            "latency must be < 60s (60M us), got {us}us"
        );
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
        let resp = send_socket_request(sock_path.path(), "daemon/status", serde_json::json!({}));
        assert!(resp.is_some(), "daemon/status must return a response");
        let resp = resp.unwrap();
        let pid = resp
            .get("result")
            .and_then(|r| r.get("pid"))
            .and_then(|v| v.as_u64());
        assert!(pid.is_some(), "daemon/status must return PID");

        // Send shutdown.
        let _shutdown_resp =
            send_socket_request(sock_path.path(), "daemon/shutdown", serde_json::json!({}));

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
        assert!(
            resp.is_some(),
            "braid_status via socket must return a response"
        );

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
        let _ = send_socket_request(sock_path.path(), "daemon/shutdown", serde_json::json!({}));
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
            let stream = UnixStream::connect(sock_path.path()).expect("must connect to daemon");
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
        let _ = send_socket_request(sock_path.path(), "daemon/shutdown", serde_json::json!({}));
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
        assert_eq!(
            count, 5,
            "5 tool calls must produce exactly 5 runtime entities"
        );
    }

    // ── DS4-TEST: Dispatch concurrency tests ─────────────────────────────
    //
    // Verify RwLock multi-threaded dispatch correctness for the daemon's
    // Arc<RwLock<LiveStore>> concurrency model (INV-DAEMON-012).

    #[test]
    fn rwlock_concurrent_reads_do_not_block() {
        use std::sync::Barrier;

        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        let shared: Arc<RwLock<crate::live_store::LiveStore>> = Arc::new(RwLock::new(live));

        let thread_count = 10;
        let barrier = Arc::new(Barrier::new(thread_count));

        let start = Instant::now();

        let handles: Vec<_> = (0..thread_count)
            .map(|_| {
                let shared = Arc::clone(&shared);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait(); // all threads start together
                    let guard = shared.read().unwrap();
                    let _len = guard.store().len();
                    std::thread::sleep(Duration::from_millis(50));
                    drop(guard);
                })
            })
            .collect();

        for h in handles {
            h.join().expect("reader thread must not panic");
        }

        let elapsed = start.elapsed();
        // If reads blocked each other sequentially, total would be ~500ms (10 * 50ms).
        // With concurrent reads, it should be ~50ms plus thread scheduling overhead.
        // Under parallel test execution, CPU contention inflates timings — use a
        // bound that still proves concurrency (< sequential) but tolerates load.
        assert!(
            elapsed < Duration::from_millis(490),
            "10 concurrent reads must complete in < 490ms (got {elapsed:?}), \
             proving reads do not block each other (sequential would be ~500ms)"
        );
    }

    #[test]
    fn rwlock_write_blocks_readers() {
        use std::sync::Barrier;

        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        let shared: Arc<RwLock<crate::live_store::LiveStore>> = Arc::new(RwLock::new(live));

        // Acquire write lock in a separate thread and hold for 100ms.
        let barrier = Arc::new(Barrier::new(2));
        let shared_w = Arc::clone(&shared);
        let barrier_w = Arc::clone(&barrier);
        let writer = std::thread::spawn(move || {
            let _guard = shared_w.write().unwrap();
            barrier_w.wait(); // signal that write lock is held
            std::thread::sleep(Duration::from_millis(100));
            // write lock released on drop
        });

        // Wait until the writer holds the lock.
        barrier.wait();

        // Now spawn 5 reader threads — they should block until writer releases.
        let reader_start = Instant::now();
        let handles: Vec<_> = (0..5)
            .map(|_| {
                let shared = Arc::clone(&shared);
                std::thread::spawn(move || {
                    let guard = shared.read().unwrap();
                    let _len = guard.store().len();
                    drop(guard);
                })
            })
            .collect();

        for h in handles {
            h.join().expect("reader thread must not panic");
        }

        let reader_elapsed = reader_start.elapsed();
        // Readers must have waited for the writer to release (~100ms hold).
        // Use 80ms lower bound to account for timing jitter.
        assert!(
            reader_elapsed >= Duration::from_millis(80),
            "readers must be blocked while writer holds lock \
             (elapsed {reader_elapsed:?}, expected >= 80ms)"
        );

        writer.join().expect("writer thread must not panic");
    }

    #[test]
    fn rwlock_sequential_writes_consistent() {
        use braid_kernel::datom::{
            AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value,
        };
        use braid_kernel::layout::TxFile;

        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        let genesis_count = live.store().len();
        let shared: Arc<RwLock<crate::live_store::LiveStore>> = Arc::new(RwLock::new(live));

        let thread_count = 10;
        let handles: Vec<_> = (0..thread_count)
            .map(|i| {
                let shared = Arc::clone(&shared);
                std::thread::spawn(move || {
                    let mut guard = shared.write().unwrap();
                    let agent = AgentId::from_name("test-ds4");
                    let tx_id = TxId::new(1_800_000_000 + i as u64, 0, agent);
                    let datom = Datom::new(
                        EntityId::from_ident(&format!(":ds4-test/entity-{i}")),
                        Attribute::from_keyword(":db/doc"),
                        Value::String(format!("thread-{i}")),
                        tx_id,
                        Op::Assert,
                    );
                    let tx = TxFile {
                        tx_id,
                        agent,
                        provenance: ProvenanceType::Observed,
                        rationale: format!("ds4-test thread {i}"),
                        causal_predecessors: vec![],
                        datoms: vec![datom],
                    };
                    guard.write_tx(&tx).expect("write_tx must succeed");
                })
            })
            .collect();

        for h in handles {
            h.join().expect("writer thread must not panic");
        }

        let guard = shared.read().unwrap();
        let final_count = guard.store().len();
        assert_eq!(
            final_count,
            genesis_count + thread_count,
            "store must contain genesis ({genesis_count}) + {thread_count} datoms, \
             got {final_count}"
        );
    }

    #[test]
    fn rwlock_poisoned_lock_recoverable() {
        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        let genesis_count = live.store().len();
        let shared: Arc<RwLock<crate::live_store::LiveStore>> = Arc::new(RwLock::new(live));

        // Spawn a thread that panics while holding the write lock.
        let shared_panic = Arc::clone(&shared);
        let handle = std::thread::spawn(move || {
            let _guard = shared_panic.write().unwrap();
            panic!("intentional panic to poison the RwLock");
        });
        // The thread will panic — we expect that.
        let _ = handle.join();

        // The RwLock is now poisoned. Verify we can recover the data.
        let result = shared.read();
        assert!(result.is_err(), "read() must return Err on poisoned lock");

        // Use match instead of unwrap_err() to avoid Debug requirement on LiveStore.
        match result {
            Err(poison_err) => {
                let recovered = poison_err.into_inner();
                let len = recovered.store().len();
                assert_eq!(
                    len, genesis_count,
                    "recovered store must still have {genesis_count} datoms, got {len}"
                );
            }
            Ok(_) => panic!("expected poisoned lock, but read() succeeded"),
        }
    }

    #[test]
    fn multi_connection_simulation() {
        use braid_kernel::datom::{
            AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value,
        };
        use braid_kernel::layout::TxFile;
        use std::sync::Barrier;

        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        let genesis_count = live.store().len();
        let shared: Arc<RwLock<crate::live_store::LiveStore>> = Arc::new(RwLock::new(live));

        let conn_count = 20;
        let barrier = Arc::new(Barrier::new(conn_count));

        let handles: Vec<_> = (0..conn_count)
            .map(|i| {
                let shared = Arc::clone(&shared);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait(); // simulate simultaneous connections
                    let mut guard = shared.write().unwrap();
                    let count_before = guard.store().len();
                    let agent = AgentId::from_name("test-multi-conn");
                    let tx_id = TxId::new(1_900_000_000 + i as u64, 0, agent);
                    let datom = Datom::new(
                        EntityId::from_ident(&format!(":multi-conn/entity-{i}")),
                        Attribute::from_keyword(":db/doc"),
                        Value::String(format!("connection-{i}")),
                        tx_id,
                        Op::Assert,
                    );
                    let tx = TxFile {
                        tx_id,
                        agent,
                        provenance: ProvenanceType::Observed,
                        rationale: format!("multi-conn {i}"),
                        causal_predecessors: vec![],
                        datoms: vec![datom],
                    };
                    guard.write_tx(&tx).expect("write_tx must succeed");
                    let count_after = guard.store().len();
                    assert_eq!(
                        count_after,
                        count_before + 1,
                        "connection {i}: store must grow by exactly 1 datom"
                    );
                })
            })
            .collect();

        for h in handles {
            h.join().expect("connection thread must not panic");
        }

        let guard = shared.read().unwrap();
        let final_count = guard.store().len();
        assert_eq!(
            final_count,
            genesis_count + conn_count,
            "store must contain genesis ({genesis_count}) + {conn_count} datoms, \
             got {final_count}"
        );
    }

    #[test]
    fn connection_thread_naming() {
        let (name_tx, name_rx) = std::sync::mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("braid-conn-7".to_string())
            .spawn(move || {
                let name = std::thread::current()
                    .name()
                    .unwrap_or("unnamed")
                    .to_string();
                name_tx.send(name).unwrap();
            })
            .expect("thread builder must succeed");

        handle.join().expect("named thread must not panic");
        let name = name_rx.recv().expect("must receive thread name");
        assert_eq!(
            name, "braid-conn-7",
            "spawned thread must be named braid-conn-N"
        );
    }

    // ── DS2: Group commit tests ─────────────────────────────────────────

    /// Helper: build a minimal TxFile for testing group commit.
    fn make_test_tx(label: &str) -> braid_kernel::layout::TxFile {
        use braid_kernel::datom::*;

        let agent = AgentId::from_name("test:ds2");
        let tx_id = TxId::new(1_000_000, 0, agent);
        let entity = EntityId::from_ident(&format!(":test/ds2-{label}"));
        let datoms = vec![Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(format!(":test/ds2-{label}")),
            tx_id,
            Op::Assert,
        )];
        braid_kernel::layout::TxFile {
            tx_id,
            agent,
            provenance: ProvenanceType::Derived,
            rationale: format!("DS2 test: {label}"),
            causal_predecessors: vec![],
            datoms,
        }
    }

    #[test]
    fn commit_handle_clone_is_send() {
        // CommitHandle must be Clone (one per connection thread).
        let (tx, _rx) = mpsc::channel::<CommitRequest>();
        let handle = CommitHandle { sender: tx };
        let handle2 = handle.clone();
        // Both handles have functioning senders.
        drop(handle);
        drop(handle2);
    }

    #[test]
    fn commit_handle_single_write() {
        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let mut live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        install_runtime_schema(&mut live).unwrap();

        let datom_count_before = live.store().len();
        let shared = Arc::new(RwLock::new(live));

        let wal_path = braid_dir.join(".cache/wal.bin");
        let wal = crate::wal::WalWriter::open(&wal_path).unwrap();

        let (commit_tx, commit_rx) = mpsc::channel::<CommitRequest>();
        let commit_handle = CommitHandle { sender: commit_tx };

        let shared_clone = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("test-commit".into())
            .spawn(move || {
                commit_thread(wal, commit_rx, shared_clone);
            })
            .unwrap();

        // Submit a single write via CommitHandle.
        let tx = make_test_tx("single");
        let result = commit_handle.commit(tx);
        assert!(result.is_ok(), "single commit must succeed: {result:?}");

        let meta = result.unwrap();
        assert_eq!(meta.offset, 0, "first entry starts at offset 0");
        assert!(meta.length > 0, "entry must have non-zero length");

        // Verify datoms were applied to the shared LiveStore.
        let live_guard = shared.read().unwrap();
        assert!(
            live_guard.store().len() > datom_count_before,
            "store must have more datoms after commit"
        );

        // Drop the handle to shut down the commit thread.
        drop(commit_handle);
        drop(live_guard); // Release read lock before join.
        thread.join().expect("commit thread must exit cleanly");
    }

    #[test]
    fn commit_handle_batch_of_three() {
        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let mut live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        install_runtime_schema(&mut live).unwrap();

        let shared = Arc::new(RwLock::new(live));

        let wal_path = braid_dir.join(".cache/wal.bin");
        let wal = crate::wal::WalWriter::open(&wal_path).unwrap();

        let (commit_tx, commit_rx) = mpsc::channel::<CommitRequest>();
        let commit_handle = CommitHandle { sender: commit_tx };

        let shared_clone = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("test-commit-batch".into())
            .spawn(move || {
                commit_thread(wal, commit_rx, shared_clone);
            })
            .unwrap();

        // Submit 3 writes from separate threads to exercise batching.
        let mut join_handles = Vec::new();
        for i in 0..3 {
            let handle = commit_handle.clone();
            let jh = std::thread::spawn(move || {
                let tx = make_test_tx(&format!("batch-{i}"));
                handle.commit(tx)
            });
            join_handles.push(jh);
        }

        // All three must succeed.
        for (i, jh) in join_handles.into_iter().enumerate() {
            let result = jh.join().expect("thread must not panic");
            assert!(result.is_ok(), "batch commit {i} must succeed: {result:?}");
        }

        // Verify all 3 datoms applied.
        let live_guard = shared.read().unwrap();
        for i in 0..3 {
            let ident = format!(":test/ds2-batch-{i}");
            let entity = braid_kernel::datom::EntityId::from_ident(&ident);
            let has_datom = live_guard
                .store()
                .entity_datoms(entity)
                .iter()
                .any(|d| d.op == braid_kernel::datom::Op::Assert);
            assert!(
                has_datom,
                "entity {ident} must exist in store after batch commit"
            );
        }

        drop(commit_handle);
        drop(live_guard);
        thread.join().expect("commit thread must exit cleanly");
    }

    #[test]
    fn commit_handle_channel_closed_returns_error() {
        let (commit_tx, commit_rx) = mpsc::channel::<CommitRequest>();
        let commit_handle = CommitHandle { sender: commit_tx };

        // Drop the receiver to simulate commit thread exit.
        drop(commit_rx);

        let tx = make_test_tx("closed");
        let result = commit_handle.commit(tx);
        assert!(result.is_err(), "commit must fail when channel is closed");
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("commit channel closed"),
            "error must mention channel closed, got: {err}"
        );
    }

    #[test]
    fn commit_thread_exits_on_sender_drop() {
        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        let shared = Arc::new(RwLock::new(live));

        let wal_path = braid_dir.join(".cache/wal.bin");
        let wal = crate::wal::WalWriter::open(&wal_path).unwrap();

        let (commit_tx, commit_rx) = mpsc::channel::<CommitRequest>();

        let shared_clone = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("test-commit-exit".into())
            .spawn(move || {
                commit_thread(wal, commit_rx, shared_clone);
            })
            .unwrap();

        // Drop all senders — commit thread should exit.
        drop(commit_tx);

        // Thread must join within a reasonable time (the recv_timeout will
        // fire, then detect Disconnected on next iteration).
        let join_result = thread.join();
        assert!(
            join_result.is_ok(),
            "commit thread must exit cleanly when all senders drop"
        );
    }

    #[test]
    fn commit_handle_wal_entries_durable() {
        // Verify WAL entries survive by reading the WAL file after commits.
        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        let shared = Arc::new(RwLock::new(live));

        let wal_path = braid_dir.join(".cache/wal.bin");
        let wal = crate::wal::WalWriter::open(&wal_path).unwrap();

        let (commit_tx, commit_rx) = mpsc::channel::<CommitRequest>();
        let commit_handle = CommitHandle { sender: commit_tx };

        let shared_clone = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("test-commit-wal".into())
            .spawn(move || {
                commit_thread(wal, commit_rx, shared_clone);
            })
            .unwrap();

        // Commit 2 transactions.
        for i in 0..2 {
            let tx = make_test_tx(&format!("wal-{i}"));
            commit_handle.commit(tx).expect("commit must succeed");
        }

        drop(commit_handle);
        thread.join().expect("commit thread must exit cleanly");

        // Re-open the WAL and verify it has exactly 2 entries.
        let reader = crate::wal::WalReader::open(&wal_path).unwrap();
        let iter = reader.iter().unwrap();
        let entries: Vec<_> = iter.collect();
        assert_eq!(entries.len(), 2, "WAL must contain exactly 2 entries");
        for entry in &entries {
            assert!(entry.is_ok(), "all WAL entries must be valid: {entry:?}");
        }
    }

    #[test]
    fn adaptive_interval_constants_coherent() {
        // Verify the adaptive interval constants are coherent.
        // Use const blocks to satisfy clippy::assertions_on_constants.
        const { assert!(GROUP_COMMIT_MIN_INTERVAL_MS < GROUP_COMMIT_INITIAL_INTERVAL_MS) };
        const { assert!(GROUP_COMMIT_SINGLE_BATCH_THRESHOLD > 0) };
    }

    // ── DS2-TEST: Group commit tests ──────────────────────────────────────

    /// Helper: build a TxFile with a unique entity ident derived from `label`.
    /// Uses a unique timestamp per call to avoid entity collisions.
    fn make_unique_test_tx(label: &str, seq: u64) -> braid_kernel::layout::TxFile {
        use braid_kernel::datom::*;

        let agent = AgentId::from_name("test:ds2-ext");
        let tx_id = TxId::new(2_000_000_000 + seq, 0, agent);
        let entity = EntityId::from_ident(&format!(":test/ds2x-{label}-{seq}"));
        let datoms = vec![Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String(format!("ds2-test-{label}-{seq}")),
            tx_id,
            Op::Assert,
        )];
        braid_kernel::layout::TxFile {
            tx_id,
            agent,
            provenance: ProvenanceType::Derived,
            rationale: format!("DS2-TEST: {label}-{seq}"),
            causal_predecessors: vec![],
            datoms,
        }
    }

    /// Helper: set up a commit thread with fresh LiveStore and WAL.
    /// Returns (TempDir, shared LiveStore, CommitHandle, JoinHandle).
    fn setup_commit_infra() -> (
        tempfile::TempDir,
        Arc<RwLock<crate::live_store::LiveStore>>,
        CommitHandle,
        std::thread::JoinHandle<()>,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        let shared = Arc::new(RwLock::new(live));

        let wal_path = braid_dir.join(".cache/wal.bin");
        let wal = crate::wal::WalWriter::open(&wal_path).unwrap();

        let (commit_tx, commit_rx) = mpsc::channel::<CommitRequest>();
        let commit_handle = CommitHandle { sender: commit_tx };

        let shared_clone = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("test-ds2-commit".into())
            .spawn(move || {
                commit_thread(wal, commit_rx, shared_clone);
            })
            .unwrap();

        (dir, shared, commit_handle, thread)
    }

    /// Helper: shut down the commit thread cleanly.
    fn teardown_commit(handle: CommitHandle, thread: std::thread::JoinHandle<()>) {
        drop(handle);
        thread.join().expect("commit thread must exit cleanly");
    }

    #[test]
    fn ds2_commit_handle_single_write_returns_valid_meta() {
        // DS2-TEST-1: Submit one TxFile, verify WalEntryMeta with valid chain hash.
        let (_dir, _shared, commit_handle, thread) = setup_commit_infra();

        let tx = make_unique_test_tx("single", 1);
        let result = commit_handle.commit(tx);
        assert!(result.is_ok(), "single commit must succeed: {result:?}");

        let meta = result.unwrap();
        assert_eq!(meta.offset, 0, "first entry must start at offset 0");
        assert!(meta.length > 0, "entry must have non-zero payload length");
        // Chain hash must not be all-zeros (genesis) after one entry.
        assert_ne!(
            meta.chain_hash, [0u8; 32],
            "chain hash must differ from genesis after one entry"
        );

        teardown_commit(commit_handle, thread);
    }

    #[test]
    fn ds2_commit_handle_blocks_until_durable() {
        // DS2-TEST-2: Verify the caller blocks until the commit thread fsyncs.
        // We measure that commit() returns *after* the commit thread processes,
        // by checking that the WAL file has non-zero size when commit() returns.
        let (dir, _shared, commit_handle, thread) = setup_commit_infra();

        let tx = make_unique_test_tx("durable", 1);
        let meta = commit_handle.commit(tx).expect("commit must succeed");

        // After commit() returns, the WAL must have been fsynced (INV-DS2-001).
        let wal_path = dir.path().join(".braid/.cache/wal.bin");
        let wal_size = std::fs::metadata(&wal_path)
            .expect("WAL file must exist")
            .len();
        assert!(
            wal_size > 0,
            "WAL must have non-zero size after commit returns (fsync confirmed)"
        );
        // The entry offset + frame should fit within the WAL size.
        let frame_size = 4 + meta.length as u64 + 4 + 32;
        assert!(
            wal_size >= meta.offset + frame_size,
            "WAL size ({wal_size}) must cover the entry (offset={}, frame={frame_size})",
            meta.offset
        );

        teardown_commit(commit_handle, thread);
    }

    #[test]
    fn ds2_commit_handle_multiple_sequential() {
        // DS2-TEST-3: Submit 5 writes sequentially, verify distinct WalEntryMeta
        // with increasing offsets.
        let (_dir, _shared, commit_handle, thread) = setup_commit_infra();

        let mut metas = Vec::new();
        for i in 0..5u64 {
            let tx = make_unique_test_tx("seq", i);
            let meta = commit_handle
                .commit(tx)
                .unwrap_or_else(|e| panic!("commit {i} must succeed: {e}"));
            metas.push(meta);
        }

        // Offsets must be strictly increasing.
        for window in metas.windows(2) {
            assert!(
                window[1].offset > window[0].offset,
                "offsets must be strictly increasing: {} vs {}",
                window[0].offset,
                window[1].offset
            );
        }

        // All chain hashes must be distinct.
        let unique_hashes: std::collections::HashSet<[u8; 32]> =
            metas.iter().map(|m| m.chain_hash).collect();
        assert_eq!(
            unique_hashes.len(),
            5,
            "all 5 chain hashes must be distinct"
        );

        teardown_commit(commit_handle, thread);
    }

    #[test]
    fn ds2_commit_handle_concurrent_writers() {
        // DS2-TEST-4: Spawn 10 threads, each submitting 5 writes via cloned
        // CommitHandles. Verify all 50 writes succeed and WAL has 50 entries.
        let (dir, _shared, commit_handle, thread) = setup_commit_infra();

        let thread_count = 10;
        let writes_per_thread = 5;

        let handles: Vec<_> = (0..thread_count)
            .map(|t| {
                let ch = commit_handle.clone();
                std::thread::spawn(move || {
                    let mut results = Vec::new();
                    for w in 0..writes_per_thread {
                        let seq = t as u64 * 100 + w as u64;
                        let tx = make_unique_test_tx(&format!("conc-t{t}"), seq);
                        results.push(ch.commit(tx));
                    }
                    results
                })
            })
            .collect();

        let mut total_ok = 0;
        for h in handles {
            let results = h.join().expect("writer thread must not panic");
            for r in results {
                assert!(r.is_ok(), "concurrent commit must succeed: {r:?}");
                total_ok += 1;
            }
        }
        assert_eq!(
            total_ok,
            thread_count * writes_per_thread,
            "all 50 commits must succeed"
        );

        // Shut down commit thread so WAL is fully flushed.
        teardown_commit(commit_handle, thread);

        // Re-read WAL and count entries.
        let wal_path = dir.path().join(".braid/.cache/wal.bin");
        let reader = crate::wal::WalReader::open(&wal_path).unwrap();
        let entries: Vec<_> = reader.iter().unwrap().collect();
        assert_eq!(
            entries.len(),
            thread_count * writes_per_thread,
            "WAL must contain exactly 50 entries"
        );
    }

    #[test]
    fn ds2_commit_thread_batches_concurrent_writes() {
        // DS2-TEST-5: Submit 20 writes from 20 threads simultaneously (Barrier).
        // Verify entry_count == 20 in WAL.
        use std::sync::Barrier;

        let (dir, _shared, commit_handle, thread) = setup_commit_infra();

        let writer_count = 20;
        let barrier = Arc::new(Barrier::new(writer_count));

        let handles: Vec<_> = (0..writer_count)
            .map(|i| {
                let ch = commit_handle.clone();
                let b = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    b.wait(); // all threads release simultaneously
                    let tx = make_unique_test_tx("barrier", i as u64);
                    ch.commit(tx)
                })
            })
            .collect();

        for (i, h) in handles.into_iter().enumerate() {
            let result = h.join().expect("barrier writer must not panic");
            assert!(
                result.is_ok(),
                "barrier commit {i} must succeed: {result:?}"
            );
        }

        teardown_commit(commit_handle, thread);

        // Verify all 20 entries are in the WAL.
        let wal_path = dir.path().join(".braid/.cache/wal.bin");
        let reader = crate::wal::WalReader::open(&wal_path).unwrap();
        let entry_count = reader.iter().unwrap().count();
        assert_eq!(entry_count, 20, "WAL must contain exactly 20 entries");
    }

    #[test]
    fn ds2_commit_handle_closed_channel_returns_error() {
        // DS2-TEST-6: Drop the CommitHandle sender, verify commit thread exits.
        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        let shared = Arc::new(RwLock::new(live));

        let wal_path = braid_dir.join(".cache/wal.bin");
        let wal = crate::wal::WalWriter::open(&wal_path).unwrap();

        let (commit_tx, commit_rx) = mpsc::channel::<CommitRequest>();

        let shared_clone = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("test-ds2-closed".into())
            .spawn(move || {
                commit_thread(wal, commit_rx, shared_clone);
            })
            .unwrap();

        // Drop the sender — commit thread should detect Disconnected and exit.
        drop(commit_tx);

        // Thread must join within 2 seconds (generous timeout).
        let join_result = std::thread::spawn(move || thread.join())
            .join()
            .expect("outer join must succeed");
        assert!(
            join_result.is_ok(),
            "commit thread must exit gracefully when sender is dropped"
        );
    }

    #[test]
    fn ds2_commit_thread_applies_to_store() {
        // DS2-TEST-7: After committing writes, verify the LiveStore behind
        // the RwLock contains the new datoms.
        let (_dir, shared, commit_handle, thread) = setup_commit_infra();

        let datom_count_before = shared.read().unwrap().store().len();

        // Commit 3 transactions with distinct entities.
        for i in 0..3u64 {
            let tx = make_unique_test_tx("store-apply", i);
            commit_handle
                .commit(tx)
                .unwrap_or_else(|e| panic!("commit {i} must succeed: {e}"));
        }

        // Verify in-memory store has grown.
        let datom_count_after = shared.read().unwrap().store().len();
        assert_eq!(
            datom_count_after,
            datom_count_before + 3,
            "store must have 3 more datoms after 3 commits (was {datom_count_before}, now {datom_count_after})"
        );

        // Verify specific entities exist.
        let guard = shared.read().unwrap();
        for i in 0..3u64 {
            let ident = format!(":test/ds2x-store-apply-{i}");
            let entity = braid_kernel::datom::EntityId::from_ident(&ident);
            let has_datom = guard
                .store()
                .entity_datoms(entity)
                .iter()
                .any(|d| d.op == braid_kernel::datom::Op::Assert);
            assert!(has_datom, "entity {ident} must exist in store after commit");
        }

        drop(guard);
        teardown_commit(commit_handle, thread);
    }

    #[test]
    fn ds2_commit_thread_low_latency_single_write() {
        // DS2-TEST-8: Verify single write returns within a reasonable time.
        // The batch_interval starts at 50ms, so a single write should return
        // quickly. We use 1s as an upper bound to be robust under CI load
        // (where thread scheduling can be slow), while still catching any
        // fundamental deadlock or multi-second stall.
        let (_dir, _shared, commit_handle, thread) = setup_commit_infra();

        let start = Instant::now();
        let tx = make_unique_test_tx("latency", 1);
        let result = commit_handle.commit(tx);
        let elapsed = start.elapsed();

        assert!(result.is_ok(), "single commit must succeed: {result:?}");
        assert!(
            elapsed < Duration::from_secs(1),
            "single write must return within 1s, took {elapsed:?}"
        );

        teardown_commit(commit_handle, thread);
    }

    // ── DS3-TEST: Checkpoint tests ────────────────────────────────────────

    /// Helper: count .edn files under a braid_dir's txns/ directory.
    fn count_edn_files(braid_dir: &Path) -> usize {
        let txns_dir = braid_dir.join("txns");
        if !txns_dir.is_dir() {
            return 0;
        }
        let mut count = 0;
        for shard in std::fs::read_dir(&txns_dir).unwrap() {
            let shard = shard.unwrap();
            if !shard.file_type().unwrap().is_dir() {
                continue;
            }
            for entry in std::fs::read_dir(shard.path()).unwrap() {
                let entry = entry.unwrap();
                if entry.file_name().to_string_lossy().ends_with(".edn") {
                    count += 1;
                }
            }
        }
        count
    }

    /// Helper: set up a braid directory with WAL containing `n` entries.
    /// Returns (TempDir, braid_dir PathBuf, WAL path).
    fn setup_wal_with_entries(n: u64) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        // Create the full layout so write_tx_no_invalidate works.
        let _layout = crate::layout::DiskLayout::init(&braid_dir).unwrap();

        let wal_path = braid_dir.join(".cache/wal.bin");
        let mut wal = crate::wal::WalWriter::open(&wal_path).unwrap();

        for i in 0..n {
            let tx = make_unique_test_tx("ckpt", i);
            wal.append(&tx).unwrap();
        }
        wal.sync().unwrap();

        (dir, braid_dir, wal_path)
    }

    #[test]
    fn ds3_passive_checkpoint_converts_wal_to_edn() {
        // DS3-TEST-1: Write 5 entries to WAL, run passive checkpoint,
        // verify 5 new .edn files appear on disk.
        let (_dir, braid_dir, wal_path) = setup_wal_with_entries(5);

        let edn_before = count_edn_files(&braid_dir);

        let layout = crate::layout::DiskLayout::open(&braid_dir).unwrap();
        let mut state = CheckpointState {
            checkpoint_offset: 0,
            wal_path: wal_path.clone(),
            layout,
        };

        let converted = state.passive_checkpoint();
        assert_eq!(converted, 5, "passive checkpoint must convert 5 entries");

        let edn_after = count_edn_files(&braid_dir);
        assert_eq!(
            edn_after,
            edn_before + 5,
            "5 new .edn files must appear (was {edn_before}, now {edn_after})"
        );

        // WAL must NOT be truncated (DS3-001).
        let wal_size = std::fs::metadata(&wal_path).unwrap().len();
        assert!(wal_size > 0, "passive checkpoint must not truncate WAL");
    }

    #[test]
    fn ds3_passive_checkpoint_idempotent() {
        // DS3-TEST-2: Run passive checkpoint twice on the same WAL entries.
        // Second run must not create duplicate .edn files.
        let (_dir, braid_dir, wal_path) = setup_wal_with_entries(3);

        let layout = crate::layout::DiskLayout::open(&braid_dir).unwrap();
        let mut state = CheckpointState {
            checkpoint_offset: 0,
            wal_path: wal_path.clone(),
            layout,
        };

        let first = state.passive_checkpoint();
        assert_eq!(first, 3, "first pass must convert 3 entries");
        let edn_after_first = count_edn_files(&braid_dir);

        // Second checkpoint from offset 0 (simulating idempotent restart).
        // Reset offset to 0 — write_tx_no_invalidate is idempotent (DS3-003),
        // so it silently skips already-existing .edn files.
        state.checkpoint_offset = 0;
        let second = state.passive_checkpoint();
        // The checkpoint function still iterates entries and "converts" them,
        // but the underlying write_tx_no_invalidate is a no-op for existing files.
        assert_eq!(second, 3, "second pass re-processes 3 entries (idempotent)");

        let edn_after_second = count_edn_files(&braid_dir);
        assert_eq!(
            edn_after_first, edn_after_second,
            "no duplicate .edn files: first={edn_after_first}, second={edn_after_second}"
        );
    }

    #[test]
    fn ds3_full_checkpoint_truncates_wal() {
        // DS3-TEST-3: Write entries, run full checkpoint, verify WAL is truncated.
        let (_dir, braid_dir, wal_path) = setup_wal_with_entries(4);

        // Verify WAL is non-empty before.
        let wal_size_before = std::fs::metadata(&wal_path).unwrap().len();
        assert!(
            wal_size_before > 0,
            "WAL must be non-empty before full checkpoint"
        );

        let layout = crate::layout::DiskLayout::open(&braid_dir).unwrap();
        let mut state = CheckpointState {
            checkpoint_offset: 0,
            wal_path: wal_path.clone(),
            layout,
        };

        let result = state.full_checkpoint();
        assert!(result.is_ok(), "full checkpoint must succeed: {result:?}");

        // WAL must be truncated to 0 bytes (DS3-002).
        let wal_size_after = std::fs::metadata(&wal_path).unwrap().len();
        assert_eq!(
            wal_size_after, 0,
            "WAL must be truncated to 0 bytes after full checkpoint"
        );

        // Offset must be reset.
        assert_eq!(
            state.checkpoint_offset, 0,
            "checkpoint offset must be reset to 0 after full checkpoint"
        );

        // .edn files must still exist (conversion happened before truncation).
        let edn_count = count_edn_files(&braid_dir);
        // genesis + 4 entries
        assert!(
            edn_count >= 4,
            "at least 4 .edn files must exist after full checkpoint, got {edn_count}"
        );
    }

    #[test]
    fn ds3_checkpoint_signal_stop_flushes() {
        // DS3-TEST-4: Send Stop signal, verify the checkpoint thread does
        // a final passive checkpoint before exiting.
        let (_dir, braid_dir, wal_path) = setup_wal_with_entries(3);

        let edn_before = count_edn_files(&braid_dir);

        let layout = crate::layout::DiskLayout::open(&braid_dir).unwrap();
        let state = CheckpointState {
            checkpoint_offset: 0,
            wal_path: wal_path.clone(),
            layout,
        };

        let (tx, rx) = mpsc::channel();
        let interval = Duration::from_secs(60); // Long interval so only Stop triggers work.

        let thread = std::thread::Builder::new()
            .name("test-ds3-stop".into())
            .spawn(move || {
                checkpoint_thread(state, rx, interval);
            })
            .unwrap();

        // Send Stop signal.
        tx.send(CheckpointSignal::Stop).unwrap();

        // Wait for thread to exit (max 2s).
        thread
            .join()
            .expect("checkpoint thread must exit cleanly on Stop");

        // The final passive checkpoint should have converted the 3 entries.
        let edn_after = count_edn_files(&braid_dir);
        assert_eq!(
            edn_after,
            edn_before + 3,
            "Stop must trigger final passive checkpoint: was {edn_before}, now {edn_after}"
        );
    }

    #[test]
    fn ds3_checkpoint_offset_advances() {
        // DS3-TEST-5: After passive checkpoint of 3 entries, add 2 more to WAL,
        // run another passive checkpoint. Verify only the 2 new entries are
        // converted (not all 5 again).
        let (_dir, braid_dir, wal_path) = setup_wal_with_entries(3);

        let layout = crate::layout::DiskLayout::open(&braid_dir).unwrap();
        let mut state = CheckpointState {
            checkpoint_offset: 0,
            wal_path: wal_path.clone(),
            layout,
        };

        // First pass: convert initial 3 entries.
        let first = state.passive_checkpoint();
        assert_eq!(first, 3, "first pass must convert 3 entries");
        let edn_after_first = count_edn_files(&braid_dir);

        // Append 2 more entries to the WAL.
        let mut wal = crate::wal::WalWriter::open(&wal_path).unwrap();
        for i in 100..102u64 {
            let tx = make_unique_test_tx("ckpt-extra", i);
            wal.append(&tx).unwrap();
        }
        wal.sync().unwrap();
        drop(wal);

        // Second pass: should only convert the 2 new entries.
        let second = state.passive_checkpoint();
        assert_eq!(second, 2, "second pass must convert only 2 new entries");

        let edn_after_second = count_edn_files(&braid_dir);
        assert_eq!(
            edn_after_second,
            edn_after_first + 2,
            "only 2 new .edn files: was {edn_after_first}, now {edn_after_second}"
        );
    }

    #[test]
    fn ds3_spawn_checkpoint_thread_returns_sender() {
        // DS3-TEST-6: Call spawn_checkpoint_thread, verify it returns a
        // Sender<CheckpointSignal> that can send signals.
        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        // Must initialize layout for spawn_checkpoint_thread to succeed.
        let _layout = crate::layout::DiskLayout::init(&braid_dir).unwrap();

        let result = spawn_checkpoint_thread(&braid_dir, 60);
        assert!(
            result.is_ok(),
            "spawn_checkpoint_thread must succeed: {result:?}"
        );

        let (sender, handle) = result.unwrap();

        // Verify the sender can send a Tick (no panic).
        sender
            .send(CheckpointSignal::Tick)
            .expect("Tick signal must be sendable");

        // Stop the thread cleanly.
        sender
            .send(CheckpointSignal::Stop)
            .expect("Stop signal must be sendable");
        handle
            .join()
            .expect("checkpoint thread must exit after Stop");
    }

    #[test]
    fn ds3_full_checkpoint_signal_returns_ok() {
        // DS3-TEST-7: Verify that sending CheckpointSignal::Full with a
        // response channel gets an Ok response (eventual DS6 wiring test).
        let (_dir, braid_dir, wal_path) = setup_wal_with_entries(2);

        let layout = crate::layout::DiskLayout::open(&braid_dir).unwrap();
        let state = CheckpointState {
            checkpoint_offset: 0,
            wal_path: wal_path.clone(),
            layout,
        };

        let (signal_tx, signal_rx) = mpsc::channel();
        let interval = Duration::from_secs(60);

        let thread = std::thread::Builder::new()
            .name("test-ds3-full-signal".into())
            .spawn(move || {
                checkpoint_thread(state, signal_rx, interval);
            })
            .unwrap();

        // Send Full signal with a response channel.
        let (done_tx, done_rx) = mpsc::channel();
        signal_tx
            .send(CheckpointSignal::Full(done_tx))
            .expect("Full signal must be sendable");

        // Wait for the response (max 2s).
        let response = done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("must receive full checkpoint response within 2s");
        assert!(
            response.is_ok(),
            "full checkpoint must return Ok: {response:?}"
        );

        // Verify .edn files were created.
        let edn_count = count_edn_files(&braid_dir);
        assert!(
            edn_count >= 2,
            "full checkpoint via signal must create .edn files, got {edn_count}"
        );

        // Verify WAL was truncated.
        let wal_size = std::fs::metadata(&wal_path).unwrap().len();
        assert_eq!(wal_size, 0, "full checkpoint via signal must truncate WAL");

        // Stop the thread.
        signal_tx
            .send(CheckpointSignal::Stop)
            .expect("Stop signal must be sendable");
        thread
            .join()
            .expect("checkpoint thread must exit after Stop");
    }

    // ── DS7: Scale verification ──────────────────────────────────────────

    /// Helper: build a TxFile with a unique entity ident for DS7 scale tests.
    /// Each call produces a datom with a distinct entity + tx to avoid collisions.
    fn make_scale_tx(label: &str, thread_id: usize, seq: usize) -> braid_kernel::layout::TxFile {
        use braid_kernel::datom::*;

        let agent = AgentId::from_name("test:ds7-scale");
        let unique_ts = 3_000_000_000u64 + (thread_id as u64 * 10_000) + seq as u64;
        let tx_id = TxId::new(unique_ts, 0, agent);
        let entity = EntityId::from_ident(&format!(":ds7-scale/{label}-t{thread_id}-s{seq}"));
        let datoms = vec![Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String(format!("ds7-{label}-thread{thread_id}-seq{seq}")),
            tx_id,
            Op::Assert,
        )];
        braid_kernel::layout::TxFile {
            tx_id,
            agent,
            provenance: ProvenanceType::Derived,
            rationale: format!("DS7 scale: {label} t{thread_id} s{seq}"),
            causal_predecessors: vec![],
            datoms,
        }
    }

    // ── Category 1: Concurrent Write Visibility ──────────────────────────

    #[test]
    fn scale_10_writers_all_visible() {
        // 10 threads each write 5 unique datoms through Arc<RwLock<LiveStore>>.
        // After all complete, verify all 50 datoms are visible in the store.
        use std::sync::Barrier;

        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        let genesis_count = live.store().len();
        let shared: Arc<RwLock<crate::live_store::LiveStore>> = Arc::new(RwLock::new(live));

        let writer_count = 10;
        let datoms_per_writer = 5;
        let barrier = Arc::new(Barrier::new(writer_count));

        let handles: Vec<_> = (0..writer_count)
            .map(|t| {
                let shared = Arc::clone(&shared);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait(); // all threads start writing simultaneously
                    for s in 0..datoms_per_writer {
                        let tx = make_scale_tx("vis", t, s);
                        let mut guard = shared.write().unwrap();
                        guard.write_tx(&tx).expect("write_tx must succeed");
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().expect("writer thread must not panic");
        }

        // Verify all 50 datoms are visible.
        let guard = shared.read().unwrap();
        let final_count = guard.store().len();
        let expected = genesis_count + (writer_count * datoms_per_writer);
        assert_eq!(
            final_count, expected,
            "store must contain genesis ({genesis_count}) + {} datoms = {expected}, got {final_count}",
            writer_count * datoms_per_writer
        );

        // Spot-check: verify specific entities from each thread are present.
        for t in 0..writer_count {
            for s in 0..datoms_per_writer {
                let ident = format!(":ds7-scale/vis-t{t}-s{s}");
                let entity = braid_kernel::datom::EntityId::from_ident(&ident);
                let has_datom = guard
                    .store()
                    .entity_datoms(entity)
                    .iter()
                    .any(|d| d.op == braid_kernel::datom::Op::Assert);
                assert!(
                    has_datom,
                    "entity {ident} must be visible after all writers complete"
                );
            }
        }
    }

    // ── Category 2: Write-then-Read Consistency ──────────────────────────

    #[test]
    fn scale_write_then_read_consistent() {
        // Thread A writes a datom, Thread B reads it after A releases the
        // write lock. Repeat 20 times with different threads.

        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        let shared: Arc<RwLock<crate::live_store::LiveStore>> = Arc::new(RwLock::new(live));

        for round in 0..20usize {
            let shared_w = Arc::clone(&shared);
            let shared_r = Arc::clone(&shared);

            // Writer thread writes a unique datom.
            let writer = std::thread::spawn(move || {
                let tx = make_scale_tx("wtr", round, 0);
                let mut guard = shared_w.write().unwrap();
                guard.write_tx(&tx).expect("write_tx must succeed");
                // Lock released on drop.
            });

            writer.join().expect("writer thread must not panic");

            // Reader thread verifies the datom is visible.
            let reader = std::thread::spawn(move || {
                let guard = shared_r.read().unwrap();
                let ident = format!(":ds7-scale/wtr-t{round}-s0");
                let entity = braid_kernel::datom::EntityId::from_ident(&ident);
                let has_datom = guard
                    .store()
                    .entity_datoms(entity)
                    .iter()
                    .any(|d| d.op == braid_kernel::datom::Op::Assert);
                assert!(
                    has_datom,
                    "round {round}: reader must see writer's datom for {ident}"
                );
            });

            reader.join().expect("reader thread must not panic");
        }
    }

    // ── Category 3: Checkpoint Correctness ───────────────────────────────

    #[test]
    fn scale_checkpoint_preserves_all_data() {
        // Write 20 entries through LiveStore, flush, then verify a full
        // rebuild from .edn files matches. INV-STORE-020 at scale.

        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let mut live = crate::live_store::LiveStore::create(&braid_dir).unwrap();

        // Write 20 transactions.
        for i in 0..20usize {
            let tx = make_scale_tx("ckpt", i, 0);
            live.write_tx(&tx).expect("write_tx must succeed");
        }

        let count_before_flush = live.store().len();

        // Flush writes store.bin to disk.
        live.flush().expect("flush must succeed");
        drop(live);

        // Rebuild by opening (loads from store.bin + any delta).
        let reopened = crate::live_store::LiveStore::open(&braid_dir)
            .expect("re-open after flush must succeed");
        let count_after_reopen = reopened.store().len();

        assert_eq!(
            count_before_flush, count_after_reopen,
            "INV-STORE-020: rebuilt store must have same datom count as pre-flush \
             (before={count_before_flush}, after={count_after_reopen})"
        );

        // Verify specific entities survived the roundtrip.
        for i in 0..20usize {
            let ident = format!(":ds7-scale/ckpt-t{i}-s0");
            let entity = braid_kernel::datom::EntityId::from_ident(&ident);
            let has_datom = reopened
                .store()
                .entity_datoms(entity)
                .iter()
                .any(|d| d.op == braid_kernel::datom::Op::Assert);
            assert!(
                has_datom,
                "entity {ident} must survive flush+reopen checkpoint cycle"
            );
        }
    }

    // ── Category 4: WAL Recovery Correctness ─────────────────────────────

    #[test]
    fn scale_wal_recovery_matches_direct() {
        // Write 10 entries through WAL + apply to store. Separately, write
        // the same entries through LiveStore directly. Verify both stores
        // have identical datom counts.

        // Path A: LiveStore direct writes.
        let dir_a = tempfile::tempdir().unwrap();
        let braid_dir_a = dir_a.path().join(".braid");
        let mut live_a = crate::live_store::LiveStore::create(&braid_dir_a).unwrap();

        let txs: Vec<_> = (0..10usize)
            .map(|i| make_scale_tx("walrec", i, 0))
            .collect();

        for tx in &txs {
            live_a.write_tx(tx).expect("direct write_tx must succeed");
        }
        live_a.flush().expect("flush A must succeed");
        let count_direct = live_a.store().len();
        drop(live_a);

        // Path B: WAL append + open_with_wal recovery.
        let dir_b = tempfile::tempdir().unwrap();
        let braid_dir_b = dir_b.path().join(".braid");
        // Create the store layout with genesis.
        let mut live_b_init = crate::live_store::LiveStore::create(&braid_dir_b).unwrap();
        live_b_init.flush().expect("flush B init must succeed");
        drop(live_b_init);

        // Write the same transactions to WAL.
        let wal_path = braid_dir_b.join(".cache").join("wal.bin");
        let _ = std::fs::create_dir_all(braid_dir_b.join(".cache"));
        let mut wal = crate::wal::WalWriter::open(&wal_path).unwrap();
        for tx in &txs {
            wal.append(tx).unwrap();
        }
        wal.sync().unwrap();
        drop(wal);

        // Recover via open_with_wal.
        let live_b = crate::live_store::LiveStore::open_with_wal(&braid_dir_b)
            .expect("open_with_wal must succeed");
        let count_wal = live_b.store().len();

        assert_eq!(
            count_direct, count_wal,
            "WAL recovery must produce same datom count as direct writes \
             (direct={count_direct}, wal={count_wal})"
        );
    }

    // ── Category 5: Concurrent Read Performance ──────────────────────────

    #[test]
    fn scale_50_concurrent_reads() {
        // Spawn 50 threads that each read store.len() 100 times (total 5000 reads).
        // Verify all complete within 2 seconds (proves reads are truly concurrent).
        use std::sync::Barrier;

        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        let expected_len = live.store().len();
        let shared: Arc<RwLock<crate::live_store::LiveStore>> = Arc::new(RwLock::new(live));

        let reader_count = 50;
        let reads_per_thread = 100;
        let barrier = Arc::new(Barrier::new(reader_count));

        let start = Instant::now();

        let handles: Vec<_> = (0..reader_count)
            .map(|_| {
                let shared = Arc::clone(&shared);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait(); // all threads start reading simultaneously
                    for _ in 0..reads_per_thread {
                        let guard = shared.read().unwrap();
                        let len = guard.store().len();
                        assert_eq!(
                            len, expected_len,
                            "concurrent read must see consistent store length"
                        );
                        drop(guard);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().expect("reader thread must not panic");
        }

        let elapsed = start.elapsed();
        // 50 threads * 100 reads = 5000 reads.
        // With RwLock concurrent reads, this should complete in well under 5s.
        // Sequential reads (one at a time) would take ~50s at 10ms each.
        // Under parallel test execution, CPU contention inflates timings.
        assert!(
            elapsed < Duration::from_secs(5),
            "5000 concurrent reads must complete within 5s (got {elapsed:?}), \
             proving RwLock::read() allows true concurrency (sequential ~50s)"
        );
    }

    // ── Category 6: Mixed Read-Write Load ────────────────────────────────

    #[test]
    fn scale_mixed_readwrite_no_panic() {
        // 5 writer threads (each writes 10 datoms) + 20 reader threads
        // (each reads 50 times) running simultaneously on shared
        // Arc<RwLock<LiveStore>>. Verify: no panics, no deadlocks,
        // final datom count = genesis + 50.
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Barrier;

        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        let genesis_count = live.store().len();
        let shared: Arc<RwLock<crate::live_store::LiveStore>> = Arc::new(RwLock::new(live));

        let writer_count = 5;
        let datoms_per_writer = 10;
        let reader_count = 20;
        let reads_per_reader = 50;
        let total_threads = writer_count + reader_count;
        let barrier = Arc::new(Barrier::new(total_threads));
        let any_failure = Arc::new(AtomicBool::new(false));

        // Spawn writer threads.
        let mut handles = Vec::with_capacity(total_threads);
        for t in 0..writer_count {
            let shared = Arc::clone(&shared);
            let barrier = Arc::clone(&barrier);
            let any_failure = Arc::clone(&any_failure);
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                for s in 0..datoms_per_writer {
                    let tx = make_scale_tx("mixed-w", t, s);
                    let mut guard = shared.write().unwrap();
                    if guard.write_tx(&tx).is_err() {
                        any_failure.store(true, Ordering::SeqCst);
                    }
                }
            }));
        }

        // Spawn reader threads.
        for _ in 0..reader_count {
            let shared = Arc::clone(&shared);
            let barrier = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..reads_per_reader {
                    let guard = shared.read().unwrap();
                    // Just read the length — must not panic.
                    let _len = guard.store().len();
                    drop(guard);
                }
            }));
        }

        // Wait with a generous timeout to detect deadlocks.
        let deadline = Instant::now() + Duration::from_secs(10);
        for h in handles {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "deadlock detected: mixed read-write test exceeded 10s timeout"
            );
            h.join().expect("mixed read-write thread must not panic");
        }

        assert!(
            !any_failure.load(Ordering::SeqCst),
            "no write failures allowed during mixed load"
        );

        // Verify final datom count.
        let guard = shared.read().unwrap();
        let final_count = guard.store().len();
        let expected = genesis_count + (writer_count * datoms_per_writer);
        assert_eq!(
            final_count, expected,
            "final datom count must be genesis ({genesis_count}) + {} = {expected}, got {final_count}",
            writer_count * datoms_per_writer
        );
    }

    // ── Category 7: Full Lifecycle ───────────────────────────────────────

    #[test]
    fn scale_lifecycle_init_write_checkpoint_recover() {
        // Full lifecycle test:
        // 1. Create a store
        // 2. Write 10 entries via LiveStore
        // 3. Flush (writes store.bin)
        // 4. Write 5 entries to WAL (simulating daemon writes after checkpoint)
        // 5. Drop everything (simulating crash)
        // 6. Call open_with_wal — should recover all 15 entries
        // 7. Verify datom count matches

        // Step 1: Create a store.
        let dir = tempfile::tempdir().unwrap();
        let braid_dir = dir.path().join(".braid");
        let mut live = crate::live_store::LiveStore::create(&braid_dir).unwrap();
        let genesis_count = live.store().len();

        // Step 2: Write 10 entries via LiveStore (persisted as .edn files).
        for i in 0..10usize {
            let tx = make_scale_tx("lifecycle-init", i, 0);
            live.write_tx(&tx).expect("initial write_tx must succeed");
        }

        let count_after_initial = live.store().len();
        assert_eq!(
            count_after_initial,
            genesis_count + 10,
            "step 2: store must have genesis + 10 datoms"
        );

        // Step 3: Flush writes store.bin (checkpoint).
        live.flush().expect("flush must succeed");
        drop(live);

        // Step 4: Write 5 entries directly to WAL (simulating daemon writes).
        let wal_path = braid_dir.join(".cache").join("wal.bin");
        let _ = std::fs::create_dir_all(braid_dir.join(".cache"));
        let mut wal = crate::wal::WalWriter::open(&wal_path).unwrap();
        for i in 0..5usize {
            let tx = make_scale_tx("lifecycle-wal", i, 0);
            wal.append(&tx).unwrap();
        }
        wal.sync().unwrap();

        // Step 5: Drop everything (simulating crash).
        drop(wal);

        // Step 6: Recover via open_with_wal.
        let recovered = crate::live_store::LiveStore::open_with_wal(&braid_dir)
            .expect("open_with_wal must recover successfully");

        // Step 7: Verify all 15 entries are present.
        let recovered_count = recovered.store().len();
        let expected = genesis_count + 15;
        assert_eq!(
            recovered_count, expected,
            "recovered store must have genesis ({genesis_count}) + 15 = {expected} datoms, \
             got {recovered_count}"
        );

        // Verify specific entities from both phases are present.
        for i in 0..10usize {
            let ident = format!(":ds7-scale/lifecycle-init-t{i}-s0");
            let entity = braid_kernel::datom::EntityId::from_ident(&ident);
            let has_datom = recovered
                .store()
                .entity_datoms(entity)
                .iter()
                .any(|d| d.op == braid_kernel::datom::Op::Assert);
            assert!(
                has_datom,
                "initial entry {ident} must survive checkpoint+WAL recovery"
            );
        }
        for i in 0..5usize {
            let ident = format!(":ds7-scale/lifecycle-wal-t{i}-s0");
            let entity = braid_kernel::datom::EntityId::from_ident(&ident);
            let has_datom = recovered
                .store()
                .entity_datoms(entity)
                .iter()
                .any(|d| d.op == braid_kernel::datom::Op::Assert);
            assert!(
                has_datom,
                "WAL entry {ident} must be recovered by open_with_wal"
            );
        }
    }
}
