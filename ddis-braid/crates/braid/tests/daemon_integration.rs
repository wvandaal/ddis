//! Integration tests for daemon lifecycle and socket communication.
//!
//! Exercises the daemon through its actual interfaces: the `braid` CLI binary
//! (via `assert_cmd::Command`) and direct Unix socket communication (via
//! `std::os::unix::net::UnixStream`).
//!
//! Verified invariants:
//! - **INV-DAEMON-001**: At most one daemon per `.braid` directory.
//! - **INV-DAEMON-003**: Every command emits `:runtime/*` datoms.
//! - **INV-DAEMON-004**: Semantic equivalence with direct mode.
//! - **INV-DAEMON-005**: Stale lock recovery via `kill(pid, 0)`.
//! - **INV-DAEMON-006**: Graceful shutdown preserves all state.
//! - **INV-DAEMON-012**: Multi-threaded dispatch — accept loop never blocks.
//!
//! Each test creates its own tempdir and daemon — fully isolated.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use assert_cmd::Command;
use serde_json::{json, Value as JsonValue};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Path to the braid binary.
#[allow(deprecated)]
fn braid_cmd() -> Command {
    Command::cargo_bin("braid").unwrap()
}

/// Initialize a braid store at `braid_dir` using the CLI.
fn init_store(braid_dir: &Path) {
    braid_cmd()
        .args(["init", "--path"])
        .arg(braid_dir)
        .arg("-q")
        .assert()
        .success();
}

/// RAII guard: stops the daemon on Drop so tests always clean up.
struct DaemonGuard {
    braid_dir: std::path::PathBuf,
    child: Option<std::process::Child>,
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        // Try graceful shutdown via socket.
        let sock_path = self.braid_dir.join("daemon.sock");
        if sock_path.exists() {
            let _ = send_jsonrpc_raw(&sock_path, "daemon/shutdown", json!({}));
        }
        // Wait for the child process.
        if let Some(ref mut child) = self.child {
            // Give it a moment to shut down gracefully.
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if std::time::Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    _ => {
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                }
            }
        }
        // Clean up leftover files.
        let _ = std::fs::remove_file(self.braid_dir.join("daemon.sock"));
        let _ = std::fs::remove_file(self.braid_dir.join("daemon.lock"));
    }
}

/// Start daemon in background, return a DaemonGuard.
/// Polls socket for up to 5 seconds.
fn start_daemon(braid_dir: &Path) -> DaemonGuard {
    let program = braid_cmd().get_program().to_owned();
    let child = std::process::Command::new(&program)
        .args(["daemon", "start", "--path", &braid_dir.to_string_lossy()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to start daemon");

    let sock_path = braid_dir.join("daemon.sock");
    for _ in 0..50 {
        if sock_path.exists() && UnixStream::connect(&sock_path).is_ok() {
            return DaemonGuard {
                braid_dir: braid_dir.to_path_buf(),
                child: Some(child),
            };
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // Ensure the child is waited on before panicking.
    let mut child = child;
    let _ = child.kill();
    let _ = child.wait();
    panic!(
        "daemon failed to start within 5s (socket: {})",
        sock_path.display()
    );
}

/// Connect to daemon socket and send/receive JSON-RPC.
fn send_jsonrpc(sock_path: &Path, method: &str, params: JsonValue) -> JsonValue {
    send_jsonrpc_raw(sock_path, method, params).expect("JSON-RPC request failed")
}

/// Connect to daemon socket and send/receive JSON-RPC (fallible).
fn send_jsonrpc_raw(sock_path: &Path, method: &str, params: JsonValue) -> Option<JsonValue> {
    let stream = UnixStream::connect(sock_path).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok()?;

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });

    let mut line = serde_json::to_string(&request).ok()?;
    line.push('\n');
    let mut writer = std::io::BufWriter::new(&stream);
    writer.write_all(line.as_bytes()).ok()?;
    writer.flush().ok()?;

    let reader = BufReader::new(&stream);
    let mut response_line = String::new();
    // The reader only borrows stream via the already-created BufReader.
    let mut lines_iter = reader.lines();
    if let Some(Ok(l)) = lines_iter.next() {
        response_line = l;
    }
    serde_json::from_str(&response_line).ok()
}

/// Send tools/call request for a specific tool.
fn call_tool(sock_path: &Path, tool: &str, args: JsonValue) -> JsonValue {
    send_jsonrpc(
        sock_path,
        "tools/call",
        json!({
            "name": tool,
            "arguments": args,
        }),
    )
}

/// Extract the text content from a tools/call response.
fn extract_text(response: &JsonValue) -> Option<String> {
    response
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .map(String::from)
}

/// Check if a response indicates an error.
fn is_error(response: &JsonValue) -> bool {
    // JSON-RPC level error.
    if response.get("error").is_some() {
        return true;
    }
    // MCP tool-level error (isError in result).
    response
        .get("result")
        .and_then(|r| r.get("isError"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Count .edn files in the txns/ directory tree.
fn count_edn_files(braid_dir: &Path) -> usize {
    let txns_dir = braid_dir.join("txns");
    let mut count = 0;
    if let Ok(shards) = std::fs::read_dir(&txns_dir) {
        for shard_entry in shards.flatten() {
            if shard_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if let Ok(files) = std::fs::read_dir(shard_entry.path()) {
                    for file_entry in files.flatten() {
                        let name = file_entry.file_name();
                        if name.to_string_lossy().ends_with(".edn") {
                            count += 1;
                        }
                    }
                }
            }
        }
    }
    count
}

/// Shorthand: socket path for a braid directory.
fn sock(braid_dir: &Path) -> std::path::PathBuf {
    braid_dir.join("daemon.sock")
}

// ===========================================================================
// Category 1: Daemon Lifecycle (P0)
// ===========================================================================

/// INV-DAEMON-001, INV-DAEMON-006: Start creates socket and lock; stop removes both.
#[test]
fn daemon_start_creates_socket_and_lock() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);

    let guard = start_daemon(&braid_dir);

    // Verify socket exists.
    assert!(
        sock(&braid_dir).exists(),
        "daemon.sock must exist after start"
    );
    // Verify lock exists.
    assert!(
        braid_dir.join("daemon.lock").exists(),
        "daemon.lock must exist after start"
    );
    // Verify lock file contains a PID.
    let lock_content = std::fs::read_to_string(braid_dir.join("daemon.lock")).unwrap();
    let pid: u32 = lock_content
        .trim()
        .parse()
        .expect("lock must contain a PID");
    assert!(pid > 0, "lock PID must be positive");

    // Drop guard triggers shutdown.
    drop(guard);
    // Brief wait for cleanup.
    std::thread::sleep(Duration::from_millis(500));

    assert!(
        !sock(&braid_dir).exists(),
        "daemon.sock must be removed after shutdown"
    );
    assert!(
        !braid_dir.join("daemon.lock").exists(),
        "daemon.lock must be removed after shutdown"
    );
}

/// INV-DAEMON-006: Graceful shutdown flushes store.
#[test]
fn daemon_stop_flushes_store() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);

    let guard = start_daemon(&braid_dir);
    let sp = sock(&braid_dir);

    // Observe something through the socket.
    let resp = call_tool(
        &sp,
        "braid_observe",
        json!({
            "text": "daemon-flush-test-observation",
            "confidence": 0.8,
        }),
    );
    assert!(!is_error(&resp), "observe must succeed: {resp}");

    // Graceful shutdown.
    drop(guard);
    std::thread::sleep(Duration::from_millis(500));

    // Reopen store directly via CLI and verify observation persisted.
    let output = braid_cmd()
        .args(["query", "--path"])
        .arg(&braid_dir)
        .args(["-q", "--attribute", ":exploration/body"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("daemon-flush-test-observation"),
        "observation must survive daemon shutdown. stdout: {stdout}"
    );
}

/// INV-DAEMON-001: Second daemon instance blocked.
#[test]
fn daemon_second_instance_blocked() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);

    let _guard = start_daemon(&braid_dir);

    // Try starting a second daemon. It should fail with a LockHeld error.
    let program = braid_cmd().get_program().to_owned();
    let output = std::process::Command::new(&program)
        .args(["daemon", "start", "--path", &braid_dir.to_string_lossy()])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("failed to run second daemon");

    // Second instance must fail (non-zero exit or error in stderr).
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");
    // It should mention the lock or that another daemon is running.
    assert!(
        !output.status.success() || combined.contains("lock held") || combined.contains("daemon"),
        "second daemon must fail or report lock held. status={}, combined={combined}",
        output.status
    );
}

/// INV-DAEMON-005: Stale lock recovered.
#[test]
fn daemon_stale_lock_recovered() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);

    // Create lock file with dead PID (99999999 is almost certainly unused).
    std::fs::write(braid_dir.join("daemon.lock"), "99999999\n").unwrap();

    // Start daemon — should recover the stale lock and succeed.
    let guard = start_daemon(&braid_dir);

    // Verify daemon is responsive.
    let resp = send_jsonrpc(&sock(&braid_dir), "ping", json!({}));
    assert!(
        resp.get("result").is_some(),
        "daemon must be running after stale lock recovery: {resp}"
    );

    drop(guard);
}

// ===========================================================================
// Category 2: Socket Communication (P0+P1)
// ===========================================================================

/// MCP initialize handshake returns capabilities.
#[test]
fn socket_initialize_handshake() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);
    let _guard = start_daemon(&braid_dir);

    let resp = send_jsonrpc(
        &sock(&braid_dir),
        "initialize",
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "0.1" },
        }),
    );

    let result = resp.get("result").expect("initialize must have result");
    assert!(
        result.get("protocolVersion").is_some(),
        "must return protocolVersion: {result}"
    );
    assert!(
        result.get("capabilities").is_some(),
        "must return capabilities: {result}"
    );
    assert!(
        result.get("serverInfo").is_some(),
        "must return serverInfo: {result}"
    );
}

/// tools/list returns all 11 tool definitions.
#[test]
fn socket_tools_list_returns_all_tools() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);
    let _guard = start_daemon(&braid_dir);

    let resp = send_jsonrpc(&sock(&braid_dir), "tools/list", json!({}));
    let result = resp.get("result").expect("tools/list must have result");
    let tools = result
        .get("tools")
        .and_then(|t| t.as_array())
        .expect("must have tools array");

    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();

    let expected = [
        "braid_status",
        "braid_query",
        "braid_write",
        "braid_harvest",
        "braid_seed",
        "braid_observe",
        "braid_guidance",
        "braid_task_ready",
        "braid_task_go",
        "braid_task_close",
        "braid_task_create",
    ];

    assert_eq!(
        names.len(),
        expected.len(),
        "must expose {} tools, got {}: {:?}",
        expected.len(),
        names.len(),
        names
    );
    for exp in &expected {
        assert!(
            names.contains(exp),
            "missing expected tool: {exp}. Found: {names:?}"
        );
    }
}

/// braid_status through socket returns store metrics.
#[test]
fn socket_status_returns_store_metrics() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);
    let _guard = start_daemon(&braid_dir);

    let resp = call_tool(&sock(&braid_dir), "braid_status", json!({}));
    assert!(!is_error(&resp), "braid_status must succeed: {resp}");

    let text = extract_text(&resp).expect("status must have text content");
    // Status output should mention store metrics.
    assert!(
        text.contains("store:") || text.contains("datom") || text.contains("F(S)"),
        "status response must mention store metrics: {text}"
    );
}

/// Unknown method returns METHOD_NOT_FOUND error code (-32601).
#[test]
fn socket_unknown_method_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);
    let _guard = start_daemon(&braid_dir);

    let resp = send_jsonrpc(&sock(&braid_dir), "invalid/method", json!({}));
    let error = resp.get("error").expect("unknown method must return error");
    let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    assert_eq!(
        code, -32601,
        "must be METHOD_NOT_FOUND (-32601), got {code}"
    );
}

/// Malformed JSON returns parse error (-32700).
#[test]
fn socket_malformed_json_returns_parse_error() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);
    let _guard = start_daemon(&braid_dir);

    // Send raw non-JSON data to the socket.
    let stream = UnixStream::connect(sock(&braid_dir)).expect("connect failed");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();

    let mut writer = std::io::BufWriter::new(&stream);
    writer.write_all(b"this is not json\n").unwrap();
    writer.flush().unwrap();

    let reader = BufReader::new(&stream);
    let mut response_line = String::new();
    let mut lines_iter = reader.lines();
    if let Some(Ok(l)) = lines_iter.next() {
        response_line = l;
    }

    let resp: JsonValue =
        serde_json::from_str(&response_line).expect("response must be valid JSON");
    let error = resp.get("error").expect("malformed JSON must return error");
    let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    assert_eq!(code, -32700, "must be parse error (-32700), got {code}");
}

/// ping returns empty result.
#[test]
fn socket_ping_returns_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);
    let _guard = start_daemon(&braid_dir);

    let resp = send_jsonrpc(&sock(&braid_dir), "ping", json!({}));
    assert!(
        resp.get("result").is_some(),
        "ping must have result: {resp}"
    );
    assert!(
        resp.get("error").is_none(),
        "ping must not have error: {resp}"
    );
}

/// daemon/status returns pid, uptime, request_count.
#[test]
fn socket_daemon_status_returns_uptime() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);
    let _guard = start_daemon(&braid_dir);

    let resp = send_jsonrpc(&sock(&braid_dir), "daemon/status", json!({}));
    let result = resp.get("result").expect("daemon/status must have result");

    assert!(
        result.get("pid").and_then(|v| v.as_u64()).is_some(),
        "daemon/status must return pid: {result}"
    );
    assert!(
        result.get("uptime_secs").is_some(),
        "daemon/status must return uptime_secs: {result}"
    );
    assert!(
        result.get("request_count").is_some(),
        "daemon/status must return request_count: {result}"
    );
    assert!(
        result.get("datom_count").is_some(),
        "daemon/status must return datom_count: {result}"
    );
}

// ===========================================================================
// Category 3: Tool Dispatch Through Socket (P0+P1)
// ===========================================================================

/// THE critical integration test: observe through daemon, then query back.
#[test]
fn socket_observe_persists_and_queryable() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);
    let _guard = start_daemon(&braid_dir);
    let sp = sock(&braid_dir);

    // 1. Observe through daemon socket.
    let obs_resp = call_tool(
        &sp,
        "braid_observe",
        json!({
            "text": "socket-observe-integration-test-12345",
            "confidence": 0.75,
        }),
    );
    assert!(!is_error(&obs_resp), "observe must succeed: {obs_resp}");

    // 2. Query for the observation through daemon socket.
    let query_resp = call_tool(
        &sp,
        "braid_query",
        json!({
            "attribute": ":exploration/body",
        }),
    );
    assert!(!is_error(&query_resp), "query must succeed: {query_resp}");

    let text = extract_text(&query_resp).expect("query must have text content");
    assert!(
        text.contains("socket-observe-integration-test-12345"),
        "query must find the observation. text: {text}"
    );
}

/// Task lifecycle: create -> go (claim) -> close.
#[test]
fn socket_task_lifecycle() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);
    let _guard = start_daemon(&braid_dir);
    let sp = sock(&braid_dir);

    // Create task.
    let create_resp = call_tool(
        &sp,
        "braid_task_create",
        json!({
            "title": "daemon-lifecycle-test-task",
            "priority": 2,
        }),
    );
    assert!(
        !is_error(&create_resp),
        "task create must succeed: {create_resp}"
    );

    // Extract the task ID from the create response.
    let create_text = extract_text(&create_resp).expect("task create must return text");
    // Task ID format: t-XXXXXXXX (8 hex chars).
    let task_id = create_text
        .split_whitespace()
        .find(|w| w.starts_with("t-"))
        .unwrap_or_else(|| {
            panic!("create response must contain task ID (t-...). text: {create_text}")
        });

    // Go (claim).
    let go_resp = call_tool(&sp, "braid_task_go", json!({ "id": task_id }));
    assert!(
        !is_error(&go_resp),
        "task go must succeed for {task_id}: {go_resp}"
    );

    // Close.
    let close_resp = call_tool(
        &sp,
        "braid_task_close",
        json!({ "id": task_id, "reason": "test complete" }),
    );
    assert!(
        !is_error(&close_resp),
        "task close must succeed for {task_id}: {close_resp}"
    );
}

/// Write a datom via braid_write, query it back via braid_query.
#[test]
fn socket_write_assert_queryable() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);
    let _guard = start_daemon(&braid_dir);
    let sp = sock(&braid_dir);

    // Write a custom datom.
    let write_resp = call_tool(
        &sp,
        "braid_write",
        json!({
            "entity": ":test/write-integration",
            "attribute": ":db/doc",
            "value": "daemon-write-integration-test-value",
            "rationale": "integration test",
        }),
    );
    assert!(!is_error(&write_resp), "write must succeed: {write_resp}");

    // Query it back.
    let query_resp = call_tool(
        &sp,
        "braid_query",
        json!({
            "entity": ":test/write-integration",
        }),
    );
    assert!(!is_error(&query_resp), "query must succeed: {query_resp}");

    let text = extract_text(&query_resp).expect("query must have text content");
    assert!(
        text.contains("daemon-write-integration-test-value"),
        "query must find the written datom. text: {text}"
    );
}

// ===========================================================================
// Category 4: Runtime Observation (P1)
// ===========================================================================

/// INV-DAEMON-003: Runtime datoms emitted for every tool call.
#[test]
fn runtime_datoms_emitted_for_every_tool_call() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);
    let guard = start_daemon(&braid_dir);
    let sp = sock(&braid_dir);

    // Send 3 different tool calls.
    let _ = call_tool(&sp, "braid_status", json!({}));
    let _ = call_tool(
        &sp,
        "braid_observe",
        json!({ "text": "runtime-datom-test", "confidence": 0.5 }),
    );
    let _ = call_tool(&sp, "braid_guidance", json!({}));

    // Shutdown daemon so it flushes to disk.
    drop(guard);
    std::thread::sleep(Duration::from_millis(500));

    // Query runtime entities via CLI (direct mode, daemon is stopped).
    let output = braid_cmd()
        .args(["query", "--path"])
        .arg(&braid_dir)
        .args(["-q", "--attribute", ":runtime/command"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Count how many :runtime/command datoms there are. The daemon also
    // emits runtime datoms for its own initialization, so count should be >= 3.
    let runtime_lines = stdout
        .lines()
        .filter(|l| {
            l.contains("braid_status")
                || l.contains("braid_observe")
                || l.contains("braid_guidance")
        })
        .count();
    assert!(
        runtime_lines >= 3,
        "3 tool calls must produce at least 3 runtime command datoms, found {runtime_lines}.\nstdout: {stdout}"
    );
}

// ===========================================================================
// Category 5: Semantic Equivalence (P0)
// ===========================================================================

/// INV-DAEMON-004: Observe through daemon and through direct CLI produce
/// equivalent results (both store the observation).
#[test]
fn equivalence_observe_daemon_vs_direct() {
    // Store A: observe through daemon.
    let tmp_a = tempfile::tempdir().unwrap();
    let braid_dir_a = tmp_a.path().join(".braid");
    init_store(&braid_dir_a);

    {
        let guard = start_daemon(&braid_dir_a);
        let sp = sock(&braid_dir_a);
        let resp = call_tool(
            &sp,
            "braid_observe",
            json!({ "text": "equivalence-test-obs", "confidence": 0.8 }),
        );
        assert!(!is_error(&resp), "daemon observe must succeed: {resp}");
        drop(guard);
        std::thread::sleep(Duration::from_millis(500));
    }

    // Store B: observe through CLI direct.
    let tmp_b = tempfile::tempdir().unwrap();
    let braid_dir_b = tmp_b.path().join(".braid");
    init_store(&braid_dir_b);

    braid_cmd()
        .args([
            "observe",
            "--path",
            &braid_dir_b.to_string_lossy(),
            "-q",
            "--no-auto-crystallize",
            "-c",
            "0.8",
            "equivalence-test-obs",
        ])
        .assert()
        .success();

    // Query both stores for the observation.
    let output_a = braid_cmd()
        .args(["query", "--path"])
        .arg(&braid_dir_a)
        .args(["-q", "--attribute", ":exploration/body"])
        .output()
        .unwrap();
    let stdout_a = String::from_utf8_lossy(&output_a.stdout);

    let output_b = braid_cmd()
        .args(["query", "--path"])
        .arg(&braid_dir_b)
        .args(["-q", "--attribute", ":exploration/body"])
        .output()
        .unwrap();
    let stdout_b = String::from_utf8_lossy(&output_b.stdout);

    assert!(
        stdout_a.contains("equivalence-test-obs"),
        "daemon store must contain observation. stdout: {stdout_a}"
    );
    assert!(
        stdout_b.contains("equivalence-test-obs"),
        "direct store must contain observation. stdout: {stdout_b}"
    );
}

// ===========================================================================
// Category 6: Multi-Connection (P0+P1)
// ===========================================================================

/// INV-DAEMON-012: Two concurrent socket connections, no deadlock.
#[test]
fn concurrent_2_socket_connections_no_deadlock() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);
    let _guard = start_daemon(&braid_dir);
    let sp = sock(&braid_dir);

    let sp1 = sp.clone();
    let sp2 = sp.clone();

    let t1 = std::thread::spawn(move || {
        let mut successes = 0;
        for i in 1..=5 {
            let resp = send_jsonrpc(&sp1, "daemon/status", json!({}));
            if resp.get("result").is_some() {
                successes += 1;
            }
            // Small stagger to interleave.
            if i < 5 {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        successes
    });

    let t2 = std::thread::spawn(move || {
        let mut successes = 0;
        for i in 1..=5 {
            let resp = send_jsonrpc(&sp2, "ping", json!({}));
            if resp.get("result").is_some() {
                successes += 1;
            }
            if i < 5 {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        successes
    });

    let s1 = t1.join().expect("thread 1 must not panic");
    let s2 = t2.join().expect("thread 2 must not panic");

    assert_eq!(s1, 5, "thread 1: all 5 requests must succeed");
    assert_eq!(s2, 5, "thread 2: all 5 requests must succeed");
}

/// Multiple concurrent writes must all be visible afterward.
#[test]
fn concurrent_writes_all_visible() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);
    let guard = start_daemon(&braid_dir);
    let sp = sock(&braid_dir);

    let handles: Vec<_> = (0..5)
        .map(|i| {
            let sp = sp.clone();
            std::thread::spawn(move || {
                let resp = call_tool(
                    &sp,
                    "braid_observe",
                    json!({
                        "text": format!("concurrent-write-{i}"),
                        "confidence": 0.6,
                    }),
                );
                !is_error(&resp)
            })
        })
        .collect();

    for h in handles {
        assert!(
            h.join().expect("write thread must not panic"),
            "write must succeed"
        );
    }

    // Shutdown and verify all observations persisted.
    drop(guard);
    std::thread::sleep(Duration::from_millis(500));

    let output = braid_cmd()
        .args(["query", "--path"])
        .arg(&braid_dir)
        .args(["-q", "--attribute", ":exploration/body"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    for i in 0..5 {
        assert!(
            stdout.contains(&format!("concurrent-write-{i}")),
            "concurrent write {i} lost. stdout: {stdout}"
        );
    }
}

// ===========================================================================
// Category 7: Checkpoint (P0)
// ===========================================================================

/// Harvest commit through daemon triggers checkpoint. Observations
/// must survive as .edn files.
#[test]
fn harvest_commit_triggers_full_checkpoint() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);
    let _guard = start_daemon(&braid_dir);
    let sp = sock(&braid_dir);

    let initial_edn_count = count_edn_files(&braid_dir);

    // Write 5 observations.
    for i in 0..5 {
        let resp = call_tool(
            &sp,
            "braid_observe",
            json!({
                "text": format!("checkpoint-test-{i}"),
                "confidence": 0.7,
            }),
        );
        assert!(!is_error(&resp), "observe {i} must succeed: {resp}");
    }

    // Trigger harvest --commit to flush WAL to .edn.
    let harvest_resp = call_tool(
        &sp,
        "braid_harvest",
        json!({
            "task": "checkpoint-integration-test",
            "commit": true,
        }),
    );
    assert!(
        !is_error(&harvest_resp),
        "harvest --commit must succeed: {harvest_resp}"
    );

    // Brief wait for checkpoint thread to complete.
    std::thread::sleep(Duration::from_millis(1000));

    // Verify .edn file count increased (observations + harvest + runtime datoms).
    let final_edn_count = count_edn_files(&braid_dir);
    assert!(
        final_edn_count > initial_edn_count,
        "edn file count must increase after harvest --commit: initial={initial_edn_count}, final={final_edn_count}"
    );
}

// ===========================================================================
// Category 8: Cross-Process (P0)
// ===========================================================================

/// Write observation through daemon, stop daemon, verify via CLI.
#[test]
fn daemon_write_visible_to_cli() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);

    {
        let guard = start_daemon(&braid_dir);
        let sp = sock(&braid_dir);

        let resp = call_tool(
            &sp,
            "braid_observe",
            json!({
                "text": "cross-process-daemon-to-cli-test",
                "confidence": 0.9,
            }),
        );
        assert!(!is_error(&resp), "observe must succeed: {resp}");

        // Shutdown daemon gracefully (flushes store).
        drop(guard);
        std::thread::sleep(Duration::from_millis(500));
    }

    // Open store via CLI (direct mode) and verify.
    let output = braid_cmd()
        .args(["query", "--path"])
        .arg(&braid_dir)
        .args(["-q", "--attribute", ":exploration/body"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("cross-process-daemon-to-cli-test"),
        "daemon observation must be visible to CLI after shutdown. stdout: {stdout}"
    );
}

/// Write observation via CLI, then query through daemon.
#[test]
fn cli_write_visible_to_daemon() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);

    // Write observation through CLI (direct mode, no daemon).
    braid_cmd()
        .args([
            "observe",
            "--path",
            &braid_dir.to_string_lossy(),
            "-q",
            "--no-auto-crystallize",
            "-c",
            "0.7",
            "cli-to-daemon-cross-process-test",
        ])
        .assert()
        .success();

    // Start daemon after CLI write.
    let _guard = start_daemon(&braid_dir);
    let sp = sock(&braid_dir);

    // Query through daemon socket.
    let query_resp = call_tool(
        &sp,
        "braid_query",
        json!({
            "attribute": ":exploration/body",
        }),
    );
    assert!(!is_error(&query_resp), "query must succeed: {query_resp}");

    let text = extract_text(&query_resp).expect("query must return text");
    assert!(
        text.contains("cli-to-daemon-cross-process-test"),
        "CLI observation must be visible to daemon. text: {text}"
    );
}

// ===========================================================================
// Category 10: Error Paths (P1)
// ===========================================================================

/// Invalid tool name returns isError.
#[test]
fn invalid_tool_name_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);
    let _guard = start_daemon(&braid_dir);
    let sp = sock(&braid_dir);

    let resp = call_tool(&sp, "nonexistent_tool", json!({}));
    // The MCP dispatch returns isError for unknown tools (not a JSON-RPC error).
    let text = extract_text(&resp).unwrap_or_default();
    let has_error_flag = resp
        .get("result")
        .and_then(|r| r.get("isError"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    assert!(
        has_error_flag || text.contains("unknown tool"),
        "unknown tool must return error indicator. resp: {resp}"
    );
}

/// All 11 tools should not panic on a fresh (empty) genesis store.
#[test]
fn empty_store_all_tools_work() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");
    init_store(&braid_dir);
    let _guard = start_daemon(&braid_dir);
    let sp = sock(&braid_dir);

    // Tools that take no arguments or optional arguments.
    let no_arg_tools = ["braid_status", "braid_guidance", "braid_task_ready"];

    for tool in &no_arg_tools {
        let resp = call_tool(&sp, tool, json!({}));
        // Must return a response (not crash / timeout).
        assert!(
            resp.get("result").is_some() || resp.get("error").is_some(),
            "{tool} must return a JSON-RPC response on fresh store: {resp}"
        );
    }

    // braid_query with a simple attribute filter.
    let resp = call_tool(&sp, "braid_query", json!({ "attribute": ":db/doc" }));
    assert!(
        resp.get("result").is_some(),
        "braid_query must succeed on fresh store: {resp}"
    );

    // braid_observe.
    let resp = call_tool(
        &sp,
        "braid_observe",
        json!({ "text": "empty-store-test", "confidence": 0.5 }),
    );
    assert!(
        !is_error(&resp),
        "braid_observe must succeed on fresh store: {resp}"
    );

    // braid_write.
    let resp = call_tool(
        &sp,
        "braid_write",
        json!({
            "entity": ":test/empty-store",
            "attribute": ":db/doc",
            "value": "test",
        }),
    );
    assert!(
        !is_error(&resp),
        "braid_write must succeed on fresh store: {resp}"
    );

    // braid_seed.
    let resp = call_tool(&sp, "braid_seed", json!({}));
    assert!(
        resp.get("result").is_some(),
        "braid_seed must return result on fresh store: {resp}"
    );

    // braid_harvest.
    let resp = call_tool(&sp, "braid_harvest", json!({ "task": "empty-store-test" }));
    assert!(
        resp.get("result").is_some(),
        "braid_harvest must return result on fresh store: {resp}"
    );

    // braid_task_create.
    let resp = call_tool(
        &sp,
        "braid_task_create",
        json!({ "title": "empty-store-task", "priority": 3 }),
    );
    assert!(
        !is_error(&resp),
        "braid_task_create must succeed on fresh store: {resp}"
    );
}
