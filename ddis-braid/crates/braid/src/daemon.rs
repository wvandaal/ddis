//! Daemon foundation types — D4-1 (ADR-STORE-006).
//!
//! Defines error types, newtypes for file paths, and lock status
//! for the braid session daemon. These types enforce compile-time
//! separation between socket paths, lock paths, and request IDs
//! so call sites cannot accidentally swap them.

use std::path::{Path, PathBuf};

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
            writeln!(f, "{pid}").map_err(|e| DaemonError::BindFailed(e))?;
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
}
