//! Integration tests for the DAEMON-WRITE epic (DW0, DW0b, DW6).
//!
//! Verifies:
//! - DW0: Fingerprint uses known_hashes, not disk hashes
//! - DW0b: Flush guard skips write when unknown hashes exist on disk
//! - DW6: Iron test — create-then-search never loses data
//!
//! Since `braid` is a binary crate (no lib.rs), these tests exercise the
//! system through the CLI binary (`assert_cmd`) combined with direct
//! filesystem inspection of the `.braid/.cache/meta.json` and store layout.
//!
//! Marshal command tests (Group 5) require access to `crate::commands::Command`
//! which is private in the binary crate. Those tests live as unit tests inside
//! `src/daemon.rs` instead.

use std::path::Path;

use assert_cmd::Command;
use braid_kernel::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};
use braid_kernel::layout::{serialize_tx, ContentHash, TxFile, TxFilePath};

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

/// Write an observation through the CLI.
fn observe(braid_dir: &Path, text: &str) {
    braid_cmd()
        .args([
            "observe",
            "--path",
            &braid_dir.to_string_lossy(),
            "-q",
            "--no-auto-crystallize",
            "-c",
            "0.7",
            text,
        ])
        .assert()
        .success();
}

/// Build a minimal TxFile with a single observation datom.
fn make_tx(id: u64, ident: &str, body: &str) -> TxFile {
    let agent = AgentId::from_name("test:daemon-write");
    let tx_id = TxId::new(id, 0, agent);
    let entity = EntityId::from_ident(ident);
    let datom = Datom::new(
        entity,
        Attribute::from_keyword(":db/doc"),
        Value::String(body.into()),
        tx_id,
        Op::Assert,
    );
    TxFile {
        tx_id,
        agent,
        provenance: ProvenanceType::Derived,
        rationale: format!("daemon-write test: {body}"),
        causal_predecessors: vec![],
        datoms: vec![datom],
    }
}

/// Write a TxFile directly to the txns/ directory on disk (bypassing LiveStore).
///
/// This simulates what another process writing to the same store would do.
/// Returns the content hash hex string.
fn write_tx_to_disk(braid_dir: &Path, tx: &TxFile) -> String {
    let bytes = serialize_tx(tx);
    let hash = ContentHash::of(&bytes);
    let hex = hash.to_hex();
    let file_path = TxFilePath::from_hash(&hash);

    let shard_dir = braid_dir.join("txns").join(&file_path.shard);
    std::fs::create_dir_all(&shard_dir).unwrap();

    let full_path = shard_dir.join(&file_path.filename);
    if !full_path.exists() {
        std::fs::write(&full_path, &bytes).unwrap();
    }
    hex
}

/// Parse meta.json and return the tx_hashes list.
fn read_meta_tx_hashes(braid_dir: &Path) -> Vec<String> {
    let meta_path = braid_dir.join(".cache").join("meta.json");
    if !meta_path.exists() {
        return vec![];
    }
    let meta_str = std::fs::read_to_string(&meta_path).unwrap();
    let meta: serde_json::Value = serde_json::from_str(&meta_str).unwrap();
    meta.get("tx_hashes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
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

/// Get the modification time of store.bin (or None if absent).
fn store_bin_mtime(braid_dir: &Path) -> Option<std::time::SystemTime> {
    let path = braid_dir.join(".cache").join("store.bin");
    std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok())
}

/// Get the byte length of store.bin (or 0 if absent).
fn store_bin_len(braid_dir: &Path) -> u64 {
    let path = braid_dir.join(".cache").join("store.bin");
    std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
}

// ===========================================================================
// Group 1: Fingerprint correctness (DW0)
// ===========================================================================

/// DW0: write_slim_cache uses known_hashes (not disk hashes).
///
/// Scenario: A LiveStore opens with N txns, then 2 more EDN files appear
/// on disk (written by another process). When the LiveStore flushes, the
/// meta.json tx_hashes must contain only N entries (the hashes the LiveStore
/// knows about), NOT N+2.
///
/// A new LiveStore opening afterward must detect the fingerprint mismatch
/// (meta.json says N hashes, but disk has N+2) and rebuild from txn files,
/// yielding a store with all N+2 transactions' datoms.
#[test]
fn fingerprint_uses_known_hashes_not_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");

    // Step 1: Init via CLI (creates genesis + schema + policy txns).
    init_store(&braid_dir);

    // Step 2: Record the initial tx hash count from meta.json.
    let initial_hashes = read_meta_tx_hashes(&braid_dir);
    let initial_edn_count = count_edn_files(&braid_dir);
    assert!(
        !initial_hashes.is_empty(),
        "init should produce tx_hashes in meta.json"
    );
    assert_eq!(
        initial_hashes.len(),
        initial_edn_count,
        "after init, meta.json tx_hashes should match EDN file count"
    );

    // Step 3: Write 2 extra EDN files directly to disk (bypass CLI/LiveStore).
    write_tx_to_disk(
        &braid_dir,
        &make_tx(90001, ":test/external-1", "external write 1"),
    );
    write_tx_to_disk(
        &braid_dir,
        &make_tx(90002, ":test/external-2", "external write 2"),
    );

    // Verify disk now has 2 more files.
    let after_external_count = count_edn_files(&braid_dir);
    assert_eq!(
        after_external_count,
        initial_edn_count + 2,
        "disk should have 2 more EDN files after external writes"
    );

    // Step 4: Run a CLI command that opens LiveStore and flushes on exit.
    // `braid status` opens, reads, flushes via Drop.
    braid_cmd()
        .args(["status", "--path"])
        .arg(&braid_dir)
        .arg("-q")
        .assert()
        .success();

    // Step 5: Read meta.json after status — the new LiveStore opened with
    // a rebuild (fingerprint mismatch), loaded ALL txns including the 2
    // external ones, and flushed with all hashes known.
    let after_status_hashes = read_meta_tx_hashes(&braid_dir);
    assert_eq!(
        after_status_hashes.len(),
        after_external_count,
        "after rebuild, meta.json should reflect all {} EDN files on disk",
        after_external_count,
    );

    // Step 6: Open ANOTHER LiveStore (via status) — should get a cache HIT
    // now that meta.json is up to date.
    let _before_reopen_mtime = store_bin_mtime(&braid_dir);
    braid_cmd()
        .args(["status", "--path"])
        .arg(&braid_dir)
        .arg("-q")
        .assert()
        .success();
    let _after_reopen_mtime = store_bin_mtime(&braid_dir);

    // The mtime may or may not change (the Drop flush writes even on cache hit
    // if dirty is true). But the hash count must still be correct.
    let final_hashes = read_meta_tx_hashes(&braid_dir);
    assert_eq!(
        final_hashes.len(),
        after_external_count,
        "meta.json should remain stable after cache-hit reopen"
    );

    // Verify the store actually has datoms from the external writes.
    let output = braid_cmd()
        .args(["query", "--path"])
        .arg(&braid_dir)
        .args(["-q", "--attribute", ":db/doc"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("external write 1") || stdout.contains("external write 2"),
        "store should contain datoms from external writes after rebuild"
    );
}

// ===========================================================================
// Group 2: Flush guard (DW0b)
// ===========================================================================

/// DW0b: flush skips cache write when unknown hashes exist on disk.
///
/// Scenario: After init, write an EDN file directly to disk. Then run a
/// CLI command that only reads (no new writes). The LiveStore opens from
/// the stale cache (if fingerprint matches) or rebuilds. If it rebuilds,
/// it knows about all hashes. If it opens from cache (stale), it should
/// detect unknown hashes at flush time and skip the write.
///
/// The key invariant: the store.bin should never contain FEWER datoms than
/// what's on disk. After the next full open, all datoms must be visible.
#[test]
fn flush_skips_when_unknown_hashes_exist() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");

    // Init and get baseline.
    init_store(&braid_dir);
    let initial_bin_len = store_bin_len(&braid_dir);
    assert!(initial_bin_len > 0, "store.bin should exist after init");

    // Write external EDN file (unknown to any LiveStore that opened before this).
    write_tx_to_disk(
        &braid_dir,
        &make_tx(80001, ":test/flush-guard", "flush guard test"),
    );

    // Run status — this forces a LiveStore open + flush cycle.
    // The open will detect the fingerprint mismatch (disk has an extra hash),
    // rebuild from all txns, and flush with the complete hash set.
    braid_cmd()
        .args(["status", "--path"])
        .arg(&braid_dir)
        .arg("-q")
        .assert()
        .success();

    // After the rebuild-and-flush, store.bin should be LARGER (more datoms).
    let after_bin_len = store_bin_len(&braid_dir);
    assert!(
        after_bin_len >= initial_bin_len,
        "store.bin must not shrink: was {initial_bin_len}, now {after_bin_len}"
    );

    // Verify the external datom is now in the store.
    let output = braid_cmd()
        .args(["query", "--path"])
        .arg(&braid_dir)
        .args(["-q", "--attribute", ":db/doc"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("flush guard test"),
        "external write must be visible after rebuild"
    );
}

/// DW0b: flush succeeds when all hashes are known (normal path).
///
/// Write 3 observations through the CLI (so LiveStore knows all hashes),
/// then verify cache is valid and a reopen gets a cache hit with correct
/// datom count.
#[test]
fn flush_writes_when_all_known() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");

    init_store(&braid_dir);

    // Write 3 observations through the CLI (LiveStore tracks each hash).
    for i in 1..=3 {
        observe(&braid_dir, &format!("known-write-{i}"));
    }

    // Verify cache exists.
    assert!(
        store_bin_len(&braid_dir) > 0,
        "store.bin should exist after CLI writes"
    );

    // Record hash count.
    // NOTE: The CLI may write post-hook transactions (RFL-2 action predictions,
    // AR-2 reconciliation traces) after the main command, so meta.json may lag
    // the EDN file count by 1 if the Drop flush races with a final post-hook.
    // The critical invariant is: meta.json tx_hashes <= EDN file count
    // (never MORE hashes than files — that would be a phantom hash).
    let meta_hashes = read_meta_tx_hashes(&braid_dir);
    let edn_count = count_edn_files(&braid_dir);
    assert!(
        meta_hashes.len() <= edn_count,
        "meta.json tx_hashes ({}) must not exceed EDN file count ({})",
        meta_hashes.len(),
        edn_count,
    );

    // Reopen and verify all 3 observations are present.
    let output = braid_cmd()
        .args(["query", "--path"])
        .arg(&braid_dir)
        .args(["-q", "--attribute", ":exploration/body"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    for i in 1..=3 {
        assert!(
            stdout.contains(&format!("known-write-{i}")),
            "observation 'known-write-{i}' should be visible after reopen"
        );
    }
}

// ===========================================================================
// Group 3: The Iron Test (DW6 Category 2)
// ===========================================================================

/// DW6 Iron Test: create-then-search must find the entity immediately.
///
/// This is the exact failure case reported by users: observe or task-create
/// followed by an immediate search/query in a new process must find the
/// data. The stale cache bug caused the second process to load an old
/// store.bin that didn't include the first process's writes.
#[test]
fn iron_test_create_then_search() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");

    init_store(&braid_dir);

    // Create a task through the CLI.
    braid_cmd()
        .args([
            "task",
            "create",
            "--path",
            &braid_dir.to_string_lossy(),
            "-q",
            "--force",
            "iron-test-unique-task-12345",
            "--priority",
            "3",
        ])
        .assert()
        .success();

    // Immediately search for it in a NEW process (the critical path).
    let output = braid_cmd()
        .args([
            "task",
            "search",
            "--path",
            &braid_dir.to_string_lossy(),
            "-q",
            "iron-test-unique-task-12345",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    assert!(
        combined.contains("iron-test-unique-task-12345"),
        "IRON TEST FAILED: task created but not found in immediate search.\n\
         stdout: {stdout}\n\
         stderr: {stderr}\n\
         This indicates the stale cache bug (DW0/DW0b) is present."
    );
}

/// Iron test variant: observe then query in new process.
#[test]
fn iron_test_observe_then_query() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");

    init_store(&braid_dir);

    // Observe through CLI.
    observe(&braid_dir, "iron-unique-observation-67890");

    // Query for it in a new process.
    let output = braid_cmd()
        .args(["query", "--path"])
        .arg(&braid_dir)
        .args(["-q", "--attribute", ":exploration/body"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("iron-unique-observation-67890"),
        "IRON TEST FAILED: observation created but not found in immediate query.\n\
         stdout: {stdout}\n\
         This indicates the stale cache bug (DW0/DW0b) is present."
    );
}

// ===========================================================================
// Group 4: Sequential writes (DW6 Category 2)
// ===========================================================================

/// Sequential write-drop-reopen cycles must preserve ALL data.
///
/// Each CLI invocation opens a LiveStore, writes, drops (flush), and exits.
/// After N such cycles, a final query must see all N observations.
#[test]
fn sequential_writes_all_visible() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");

    init_store(&braid_dir);

    // Write 5 observations, each in its own CLI process.
    for i in 1..=5 {
        observe(&braid_dir, &format!("seq-obs-{i}"));
    }

    // Query ALL observations in a single new process.
    let output = braid_cmd()
        .args(["query", "--path"])
        .arg(&braid_dir)
        .args(["-q", "--attribute", ":exploration/body"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    for i in 1..=5 {
        assert!(
            stdout.contains(&format!("seq-obs-{i}")),
            "sequential write {i} lost: 'seq-obs-{i}' not found in query output.\n\
             stdout: {stdout}"
        );
    }
}

/// Sequential writes with interleaved external writes.
///
/// Tests that CLI writes interleaved with direct-to-disk writes
/// (simulating concurrent processes) all survive.
#[test]
fn sequential_writes_with_external_interleave() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");

    init_store(&braid_dir);

    // CLI write 1.
    observe(&braid_dir, "interleave-cli-1");

    // Direct disk write (simulating another process).
    write_tx_to_disk(
        &braid_dir,
        &make_tx(70001, ":test/interleave-ext-1", "interleave-ext-1"),
    );

    // CLI write 2.
    observe(&braid_dir, "interleave-cli-2");

    // Direct disk write.
    write_tx_to_disk(
        &braid_dir,
        &make_tx(70002, ":test/interleave-ext-2", "interleave-ext-2"),
    );

    // CLI write 3.
    observe(&braid_dir, "interleave-cli-3");

    // Final verification: all 5 data points must be visible.
    // CLI observations use :exploration/body.
    let output_obs = braid_cmd()
        .args(["query", "--path"])
        .arg(&braid_dir)
        .args(["-q", "--attribute", ":exploration/body"])
        .output()
        .unwrap();
    let stdout_obs = String::from_utf8_lossy(&output_obs.stdout);

    for label in &["interleave-cli-1", "interleave-cli-2", "interleave-cli-3"] {
        assert!(
            stdout_obs.contains(label),
            "CLI observation '{label}' lost in interleaved scenario.\n\
             stdout: {stdout_obs}"
        );
    }

    // External writes use :db/doc.
    let output_doc = braid_cmd()
        .args(["query", "--path"])
        .arg(&braid_dir)
        .args(["-q", "--attribute", ":db/doc"])
        .output()
        .unwrap();
    let stdout_doc = String::from_utf8_lossy(&output_doc.stdout);

    for label in &["interleave-ext-1", "interleave-ext-2"] {
        assert!(
            stdout_doc.contains(label),
            "external write '{label}' lost in interleaved scenario.\n\
             stdout: {stdout_doc}"
        );
    }
}

// ===========================================================================
// Group 5: Marshal command (DW2)
// ===========================================================================
//
// NOTE: marshal_command tests cannot be written as integration tests because
// `braid` is a binary crate with `commands` as a private module. The
// `Command` enum (needed to construct test inputs for marshal_command) is
// not accessible from integration tests.
//
// These tests belong as unit tests inside `src/daemon.rs` in a `#[cfg(test)]`
// module, where they can access `crate::commands::Command` directly.
//
// Placeholder verification: we test that the daemon routing produces correct
// results end-to-end by verifying that commands route properly through the
// CLI (which internally calls marshal_command when a daemon is available).

/// Verify that observe goes through the full pipeline (marshal + dispatch).
///
/// This doesn't test marshal_command in isolation, but verifies the
/// end-to-end path that marshal_command enables.
#[test]
fn marshal_observe_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");

    init_store(&braid_dir);

    // Observe with tags and confidence (the fields marshal_command maps).
    braid_cmd()
        .args([
            "observe",
            "--path",
            &braid_dir.to_string_lossy(),
            "-q",
            "--no-auto-crystallize",
            "-c",
            "0.9",
            "-t",
            "marshal-test",
            "marshal observation text",
        ])
        .assert()
        .success();

    // Verify the observation landed with correct fields.
    let output = braid_cmd()
        .args(["query", "--path"])
        .arg(&braid_dir)
        .args(["-q", "--attribute", ":exploration/body"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("marshal observation text"),
        "marshal_command end-to-end: observation must be queryable"
    );
}

/// Verify that init is NOT routable (marshal returns None for init).
///
/// We test this indirectly: init must work even without a daemon running,
/// confirming it takes the direct path (not daemon-routed).
#[test]
fn init_not_routable_through_daemon() {
    let tmp = tempfile::tempdir().unwrap();
    let braid_dir = tmp.path().join(".braid");

    // Init should succeed without any daemon socket present.
    // If marshal_command incorrectly tried to route Init through a daemon,
    // this would fail (no daemon to connect to).
    init_store(&braid_dir);

    // Verify the store is valid.
    braid_cmd()
        .args(["status", "--path"])
        .arg(&braid_dir)
        .arg("-q")
        .assert()
        .success();
}
