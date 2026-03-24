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
}
