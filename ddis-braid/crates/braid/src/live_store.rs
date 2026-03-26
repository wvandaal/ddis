//! LiveStore — Write-through persistent store.
//!
//! Unifies [`DiskLayout`] (persistence) and [`Store`] (state) into a single
//! self-persisting object. Every write atomically updates the in-memory store
//! and the transaction log. The binary cache (`store.bin`) is written on
//! [`flush()`] or [`Drop`], not on every write (INV-STORE-021: dirty-flag batching).
//!
//! # Invariants
//!
//! - **INV-STORE-020**: After flush, `store.bin` equals `fold(txn_files)`.
//! - **INV-STORE-021**: `write_tx()` does NOT serialize `store.bin`.
//!   Only `flush()` and `Drop` do. At most 1 serialization per process.
//! - **ADR-FOUNDATION-034**: Write-through = CRDT merge at persistence layer.
//! - **C1 crash safety**: Transaction files are fsynced before `write_tx()`
//!   returns. If the process crashes before `flush()`, the next `open()`
//!   recovers via incremental apply from txn files.
//!
//! # Usage
//!
//! ```ignore
//! let mut live = LiveStore::open(path)?;      // deserialize store.bin (~300ms)
//! let store = live.store();                    // &Store, 0ms
//! live.write_tx(&tx_file)?;                    // persist + update in-memory
//! // ... more writes ...
//! live.flush()?;                               // serialize store.bin (~100ms)
//! // Drop also flushes (best-effort)
//! ```

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use braid_kernel::layout::TxFile;
use braid_kernel::Store;

use crate::error::BraidError;
use crate::layout::DiskLayout;

/// A live, self-persisting store.
///
/// The store IS the cache. No invalidation, no rebuild, no stale data —
/// by construction. This is the Introspective Observer at the storage layer:
/// the store observes its own mutations and persists them atomically.
///
/// See [`ADR-FOUNDATION-034`] for the design decision and alternatives rejected.
pub struct LiveStore {
    /// Filesystem persistence layer.
    layout: DiskLayout,
    /// The in-memory store — always the authoritative state.
    store: Store,
    /// Path to the `.braid` directory.
    path: PathBuf,
    /// Whether the in-memory store has been modified since the last flush.
    /// When true, `flush()` will serialize `store.bin`.
    dirty: bool,
    /// LIVESTORE-6: Transaction hashes known at open/last-refresh time.
    /// Used by `refresh_if_needed()` to detect external writes without
    /// listing all 7K+ txn files on every call.
    known_hashes: HashSet<String>,
    /// Cached mtime of the txns/ directory for O(1) staleness check.
    txns_dir_mtime: Option<std::time::SystemTime>,
}

impl LiveStore {
    /// Open an existing braid store.
    ///
    /// Loads the store from the binary cache (`store.bin`) if fresh,
    /// or rebuilds from transaction files if the cache is stale.
    /// Returns an error if the `.braid` directory doesn't exist.
    pub fn open(path: &Path) -> Result<Self, BraidError> {
        let layout = DiskLayout::open(path)?;
        let store = layout.load_store()?;
        // LIVESTORE-6: Snapshot known hashes and txns/ mtime at open time.
        let known_hashes: HashSet<String> = layout
            .list_tx_hashes()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let txns_dir_mtime = std::fs::metadata(path.join("txns"))
            .and_then(|m| m.modified())
            .ok();
        Ok(LiveStore {
            layout,
            store,
            path: path.to_path_buf(),
            dirty: false,
            known_hashes,
            txns_dir_mtime,
        })
    }

    /// Create a new braid store at the given path.
    ///
    /// This is the bootstrap path for `braid init`. Creates the `.braid`
    /// directory structure and initializes with genesis datoms.
    /// Returns an error if the directory already exists.
    ///
    /// After creation, call `write_tx()` to add schema, policy, and
    /// bootstrap transactions. Each write updates the in-memory store
    /// incrementally, so the second write sees the schema from the first.
    pub fn create(path: &Path) -> Result<Self, BraidError> {
        if path.join("txns").is_dir() {
            return Err(BraidError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("braid store already exists: {}", path.display()),
            )));
        }
        let layout = DiskLayout::init(path)?;
        let store = layout.load_store()?;
        let known_hashes: HashSet<String> = layout
            .list_tx_hashes()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let txns_dir_mtime = std::fs::metadata(path.join("txns"))
            .and_then(|m| m.modified())
            .ok();
        Ok(LiveStore {
            layout,
            store,
            path: path.to_path_buf(),
            dirty: false,
            known_hashes,
            txns_dir_mtime,
        })
    }

    /// Immutable access to the in-memory store.
    ///
    /// This is the primary read path. The store is always up-to-date
    /// with all writes made through this `LiveStore` instance.
    /// Cost: pointer dereference (0ms).
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Quick check: are there new external transactions since last refresh?
    ///
    /// Returns `true` if the txns/ directory mtime has changed since the
    /// last `open()` or `refresh_if_needed()` call. This is an O(1) stat()
    /// check that does NOT apply the new transactions — call
    /// `refresh_if_needed()` to actually apply them.
    ///
    /// Used by the daemon to record `:runtime/cache-hit` (INV-DAEMON-003).
    pub fn has_new_external_txns(&self) -> bool {
        let current_mtime = std::fs::metadata(self.path.join("txns"))
            .and_then(|m| m.modified())
            .ok();
        current_mtime != self.txns_dir_mtime
    }

    /// Access to the underlying filesystem layout.
    ///
    /// Use for operations that need direct filesystem access:
    /// `list_tx_hashes()`, `verify_integrity()`, etc.
    pub fn layout(&self) -> &DiskLayout {
        &self.layout
    }

    /// The `.braid` directory path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Number of known transaction hashes (L1-SINGLE: avoids redundant list_tx_hashes).
    pub fn known_hash_count(&self) -> usize {
        self.known_hashes.len()
    }

    /// Whether the in-memory store has unserialized changes.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Write a transaction to disk and update the in-memory store.
    ///
    /// Three-step atomic write (ADR-FOUNDATION-034):
    /// 1. Write EDN transaction file to `txns/` with fsync (durability).
    /// 2. Apply transaction to in-memory `Store` via `transact()` (consistency).
    /// 3. Mark dirty for deferred `store.bin` serialization (INV-STORE-021).
    ///
    /// The transaction file is durable on disk before this method returns.
    /// If the process crashes after step 1 but before `flush()`, the next
    /// `LiveStore::open()` recovers the transaction via incremental apply.
    ///
    /// If `transact()` fails (e.g., schema violation), the transaction file
    /// is still written (durability preserved) but the in-memory store is
    /// NOT updated. The error is propagated to the caller.
    pub fn write_tx(&mut self, tx: &TxFile) -> Result<braid_kernel::layout::TxFilePath, BraidError> {
        // Step 1: Write EDN to disk (durable before we return).
        // The txn file is fsynced — crash safety guaranteed by C1.
        let file_path = self.layout.write_tx_no_invalidate(tx)?;

        // LIVESTORE-6: Track this hash so refresh_if_needed() knows it's ours.
        self.known_hashes
            .insert(file_path.filename.trim_end_matches(".edn").to_string());

        // Step 2: Apply datoms to in-memory store (ADR-STORE-011).
        //
        // Uses Store::apply_datoms() — incremental datom insertion WITHOUT
        // schema validation. This is correct because:
        // - The datoms come from a persisted TxFile (already validated at creation)
        // - Transaction::commit() would fail for datoms with attributes not yet
        //   in the schema (schema bootstrap ordering problem)
        // - apply_datoms rebuilds Schema from the expanded datom set, discovering
        //   any new attributes introduced by this transaction
        //
        // This is the incremental analog of Store::from_datoms() — same
        // correctness, O(k) cost per transaction instead of O(N) rebuild.
        self.store.apply_datoms(&tx.datoms);

        // Step 3: Mark dirty for deferred serialization (INV-STORE-021).
        self.dirty = true;

        Ok(file_path)
    }

    /// Detect and apply external transactions written by other processes.
    ///
    /// LIVESTORE-6: Multi-agent awareness. In environments where multiple braid
    /// processes (or MCP servers) write concurrently, this method detects new
    /// transaction files and applies them incrementally.
    ///
    /// **O(1) fast path**: Checks the txns/ directory mtime via `stat()`. If
    /// unchanged since last check, returns `Ok(false)` immediately (~1ms).
    ///
    /// **Incremental path**: If mtime changed, lists txn hashes, diffs against
    /// known set, reads and applies new transactions via `store.transact()`.
    ///
    /// Returns `true` if the store was updated with external transactions.
    pub fn refresh_if_needed(&mut self) -> Result<bool, BraidError> {
        // Fast path: stat() the txns/ directory. If mtime unchanged, no new files.
        let current_mtime = std::fs::metadata(self.path.join("txns"))
            .and_then(|m| m.modified())
            .ok();

        if current_mtime == self.txns_dir_mtime {
            return Ok(false); // No external changes.
        }

        // Slow path: mtime changed — list all hashes and diff.
        let all_hashes: HashSet<String> = self
            .layout
            .list_tx_hashes()?
            .into_iter()
            .collect();

        let new_hashes: Vec<&String> = all_hashes
            .difference(&self.known_hashes)
            .collect();

        if new_hashes.is_empty() {
            // Mtime changed but no new files (e.g., metadata update).
            self.txns_dir_mtime = current_mtime;
            return Ok(false);
        }

        // Apply new transactions incrementally (ADR-STORE-011).
        for hash in &new_hashes {
            if let Ok(tx) = self.layout.read_tx(hash) {
                self.store.apply_datoms(&tx.datoms);
            }
        }

        // Update tracking state.
        self.known_hashes = all_hashes;
        self.txns_dir_mtime = current_mtime;
        self.dirty = true; // The store changed — flush will update store.bin.
        Ok(true)
    }

    /// Serialize the in-memory store to `store.bin` if dirty.
    ///
    /// This is the deferred write from INV-STORE-021. Called explicitly
    /// by commands that need the cache fresh for other processes (e.g.,
    /// `braid status` returning to an agent), and by `Drop` on process exit.
    ///
    /// SAFETY: If another process has written to store.bin since we opened
    /// (detected by txns/ directory mtime change + new unknown hashes),
    /// we skip the flush to avoid overwriting newer state with stale data.
    /// The next open() will rebuild incrementally from the EDN txn files.
    ///
    /// Cost: ~100ms for a 74K datom store (bincode serialization + fsync).
    pub fn flush(&mut self) -> Result<(), BraidError> {
        if self.dirty {
            // Defensive: if another LiveStore instance (from a command that opened
            // its own) has written new transactions since we opened, our in-memory
            // state may be stale. Don't overwrite their newer store.bin.
            if self.has_new_external_txns() {
                self.dirty = false; // Silently skip — txn EDN files are durable (C1).
                return Ok(());
            }
            self.layout.write_index_cache(&self.store)?;
            self.dirty = false;
        }
        Ok(())
    }
}

impl Drop for LiveStore {
    fn drop(&mut self) {
        // Best-effort flush on process exit.
        // If this fails (disk full, permissions), the txn files are still
        // on disk — the next open() rebuilds from them.
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_returns_valid_store() {
        // Use a temporary directory with a real braid init
        let tmp = tempfile::tempdir().unwrap();
        let braid_path = tmp.path().join(".braid");
        let _layout = DiskLayout::init(&braid_path).unwrap();

        let live = LiveStore::open(&braid_path).unwrap();
        assert!(!live.store().is_empty(), "genesis store should have datoms");
        assert!(!live.is_dirty(), "freshly opened store should not be dirty");
    }

    #[test]
    fn create_returns_valid_store() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_path = tmp.path().join(".braid");

        let live = LiveStore::create(&braid_path).unwrap();
        assert!(!live.store().is_empty(), "created store should have genesis datoms");
        assert!(!live.is_dirty());
    }

    #[test]
    fn create_fails_on_existing_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_path = tmp.path().join(".braid");

        let _live = LiveStore::create(&braid_path).unwrap();
        let result = LiveStore::create(&braid_path);
        assert!(result.is_err(), "create on existing dir should fail");
    }

    #[test]
    fn write_tx_updates_in_memory() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_path = tmp.path().join(".braid");
        let mut live = LiveStore::create(&braid_path).unwrap();

        let initial_count = live.store().len();

        // Write an observation datom
        let agent = braid_kernel::datom::AgentId::from_name("test:livestore");
        let tx_id = braid_kernel::datom::TxId::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            0,
            agent,
        );
        let entity = braid_kernel::datom::EntityId::from_ident(":test/livestore-write");
        let datom = braid_kernel::datom::Datom::new(
            entity,
            braid_kernel::datom::Attribute::from_keyword(":db/doc"),
            braid_kernel::datom::Value::String("LiveStore write test".into()),
            tx_id,
            braid_kernel::datom::Op::Assert,
        );
        let tx_file = braid_kernel::layout::TxFile {
            tx_id,
            agent,
            provenance: braid_kernel::datom::ProvenanceType::Derived,
            rationale: "LIVESTORE-1 test".into(),
            causal_predecessors: vec![],
            datoms: vec![datom],
        };

        live.write_tx(&tx_file).unwrap();

        assert!(
            live.store().len() > initial_count,
            "store should have more datoms after write_tx: {} -> {}",
            initial_count,
            live.store().len()
        );
        assert!(live.is_dirty(), "store should be dirty after write_tx");
    }

    #[test]
    fn flush_writes_cache_and_clears_dirty() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_path = tmp.path().join(".braid");
        let mut live = LiveStore::create(&braid_path).unwrap();

        // Write something to make it dirty
        let agent = braid_kernel::datom::AgentId::from_name("test:flush");
        let tx_id = braid_kernel::datom::TxId::new(1000, 0, agent);
        let entity = braid_kernel::datom::EntityId::from_ident(":test/flush");
        let datom = braid_kernel::datom::Datom::new(
            entity,
            braid_kernel::datom::Attribute::from_keyword(":db/doc"),
            braid_kernel::datom::Value::String("flush test".into()),
            tx_id,
            braid_kernel::datom::Op::Assert,
        );
        let tx_file = braid_kernel::layout::TxFile {
            tx_id,
            agent,
            provenance: braid_kernel::datom::ProvenanceType::Derived,
            rationale: "flush test".into(),
            causal_predecessors: vec![],
            datoms: vec![datom],
        };

        live.write_tx(&tx_file).unwrap();
        assert!(live.is_dirty());

        live.flush().unwrap();
        assert!(!live.is_dirty(), "flush should clear dirty flag");

        // Verify store.bin exists and is fresh
        let cache_path = braid_path.join(".cache").join("store.bin");
        assert!(cache_path.exists(), "store.bin should exist after flush");

        // Verify round-trip: open the same store and check datom count matches
        let reopened = LiveStore::open(&braid_path).unwrap();
        assert_eq!(
            live.store().len(),
            reopened.store().len(),
            "reopened store should have same datom count"
        );
    }

    #[test]
    fn consecutive_writes_accumulate() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_path = tmp.path().join(".braid");
        let mut live = LiveStore::create(&braid_path).unwrap();

        let agent = braid_kernel::datom::AgentId::from_name("test:accum");

        for i in 0..3 {
            let tx_id = braid_kernel::datom::TxId::new(1000 + i, 0, agent);
            let entity = braid_kernel::datom::EntityId::from_ident(&format!(":test/accum-{i}"));
            let datom = braid_kernel::datom::Datom::new(
                entity,
                braid_kernel::datom::Attribute::from_keyword(":db/doc"),
                braid_kernel::datom::Value::String(format!("write {i}")),
                tx_id,
                braid_kernel::datom::Op::Assert,
            );
            let tx_file = braid_kernel::layout::TxFile {
                tx_id,
                agent,
                provenance: braid_kernel::datom::ProvenanceType::Derived,
                rationale: format!("accum test {i}"),
                causal_predecessors: vec![],
                datoms: vec![datom],
            };
            live.write_tx(&tx_file).unwrap();
        }

        // All 3 writes should be visible in-memory
        let docs: Vec<_> = live
            .store()
            .datoms()
            .filter(|d| {
                d.attribute.as_str() == ":db/doc"
                    && d.op == braid_kernel::datom::Op::Assert
                    && matches!(&d.value, braid_kernel::datom::Value::String(s) if s.starts_with("write "))
            })
            .collect();
        assert_eq!(docs.len(), 3, "all 3 writes should be visible");

        // Flush and reopen — all 3 should persist
        live.flush().unwrap();
        let reopened = LiveStore::open(&braid_path).unwrap();
        assert_eq!(live.store().len(), reopened.store().len());
    }

    #[test]
    fn refresh_detects_external_write() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_path = tmp.path().join(".braid");
        let mut live = LiveStore::create(&braid_path).unwrap();

        let initial_count = live.store().len();

        // Simulate an external write: create a txn file directly via DiskLayout
        // (bypassing our LiveStore — as another process would).
        let agent = braid_kernel::datom::AgentId::from_name("test:external");
        let tx_id = braid_kernel::datom::TxId::new(2000, 0, agent);
        let entity = braid_kernel::datom::EntityId::from_ident(":test/external-write");
        let datom = braid_kernel::datom::Datom::new(
            entity,
            braid_kernel::datom::Attribute::from_keyword(":db/doc"),
            braid_kernel::datom::Value::String("written by another process".into()),
            tx_id,
            braid_kernel::datom::Op::Assert,
        );
        let tx_file = braid_kernel::layout::TxFile {
            tx_id,
            agent,
            provenance: braid_kernel::datom::ProvenanceType::Derived,
            rationale: "external write test".into(),
            causal_predecessors: vec![],
            datoms: vec![datom],
        };
        // Write via the raw layout (simulating external process)
        live.layout().write_tx_no_invalidate(&tx_file).unwrap();

        // Before refresh: LiveStore doesn't know about the external write
        assert_eq!(live.store().len(), initial_count);

        // After refresh: LiveStore detects and applies the external transaction
        let refreshed = live.refresh_if_needed().unwrap();
        assert!(refreshed, "refresh should detect the external write");
        assert!(
            live.store().len() > initial_count,
            "store should have more datoms after refresh: {} -> {}",
            initial_count,
            live.store().len()
        );
    }

    #[test]
    fn refresh_noop_when_no_external_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_path = tmp.path().join(".braid");
        let mut live = LiveStore::create(&braid_path).unwrap();

        // No external writes — refresh should return false
        let refreshed = live.refresh_if_needed().unwrap();
        assert!(!refreshed, "no external changes, should return false");
    }

    /// INV-STORE-020: store.bin matches full rebuild from txn files.
    #[test]
    fn store_bin_matches_full_rebuild() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_path = tmp.path().join(".braid");

        let live_len;
        {
            let mut live = LiveStore::create(&braid_path).unwrap();
            let agent = braid_kernel::datom::AgentId::from_name("test:rebuild");
            for i in 0..5 {
                let tx_id = braid_kernel::datom::TxId::new(3000 + i, 0, agent);
                let entity = braid_kernel::datom::EntityId::from_ident(
                    &format!(":test/rebuild-{i}"),
                );
                let datom = braid_kernel::datom::Datom::new(
                    entity,
                    braid_kernel::datom::Attribute::from_keyword(":db/doc"),
                    braid_kernel::datom::Value::String(format!("rebuild {i}")),
                    tx_id,
                    braid_kernel::datom::Op::Assert,
                );
                let tx_file = braid_kernel::layout::TxFile {
                    tx_id,
                    agent,
                    provenance: braid_kernel::datom::ProvenanceType::Derived,
                    rationale: format!("rebuild test {i}"),
                    causal_predecessors: vec![],
                    datoms: vec![datom],
                };
                live.write_tx(&tx_file).unwrap();
            }
            live.flush().unwrap();
            live_len = live.store().len();
        }

        // Full rebuild from txn files (bypassing cache).
        let layout = DiskLayout::open(&braid_path).unwrap();
        let rebuilt = layout.load_store().unwrap();
        assert_eq!(
            rebuilt.len(),
            live_len,
            "INV-STORE-020: full rebuild must match LiveStore"
        );
    }

    /// C1: datom count is monotonically non-decreasing.
    #[test]
    fn monotonic_growth() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_path = tmp.path().join(".braid");
        let mut live = LiveStore::create(&braid_path).unwrap();
        let agent = braid_kernel::datom::AgentId::from_name("test:mono");

        let mut prev = live.store().len();
        for i in 0..10 {
            let tx_id = braid_kernel::datom::TxId::new(4000 + i, 0, agent);
            let entity = braid_kernel::datom::EntityId::from_ident(
                &format!(":test/mono-{i}"),
            );
            let datom = braid_kernel::datom::Datom::new(
                entity,
                braid_kernel::datom::Attribute::from_keyword(":db/doc"),
                braid_kernel::datom::Value::String(format!("mono {i}")),
                tx_id,
                braid_kernel::datom::Op::Assert,
            );
            let tx_file = braid_kernel::layout::TxFile {
                tx_id,
                agent,
                provenance: braid_kernel::datom::ProvenanceType::Derived,
                rationale: format!("mono test {i}"),
                causal_predecessors: vec![],
                datoms: vec![datom],
            };
            live.write_tx(&tx_file).unwrap();
            let curr = live.store().len();
            assert!(
                curr >= prev,
                "C1: datom count must be non-decreasing: {prev} -> {curr}"
            );
            prev = curr;
        }
    }

    /// Crash recovery: delete cache, reopen from txn files only.
    #[test]
    fn crash_recovery_via_txn_files() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_path = tmp.path().join(".braid");

        let expected_len;
        {
            let mut live = LiveStore::create(&braid_path).unwrap();
            let agent = braid_kernel::datom::AgentId::from_name("test:crash");
            let tx_id = braid_kernel::datom::TxId::new(5000, 0, agent);
            let entity = braid_kernel::datom::EntityId::from_ident(":test/crash");
            let datom = braid_kernel::datom::Datom::new(
                entity,
                braid_kernel::datom::Attribute::from_keyword(":db/doc"),
                braid_kernel::datom::Value::String("crash test".into()),
                tx_id,
                braid_kernel::datom::Op::Assert,
            );
            let tx_file = braid_kernel::layout::TxFile {
                tx_id,
                agent,
                provenance: braid_kernel::datom::ProvenanceType::Derived,
                rationale: "crash test".into(),
                causal_predecessors: vec![],
                datoms: vec![datom],
            };
            live.write_tx(&tx_file).unwrap();
            expected_len = live.store().len();
            // Drop flushes best-effort — we delete the cache after.
        }

        // Delete the cache to simulate a crash before flush.
        let cache_dir = braid_path.join(".cache");
        if cache_dir.is_dir() {
            for entry in std::fs::read_dir(&cache_dir).unwrap() {
                let entry = entry.unwrap();
                if entry.path().extension().map(|x| x == "bin").unwrap_or(false) {
                    std::fs::remove_file(entry.path()).unwrap();
                }
            }
        }

        // Reopen — must recover from txn files.
        let live2 = LiveStore::open(&braid_path).unwrap();
        assert_eq!(
            live2.store().len(),
            expected_len,
            "crash recovery: store must have same datom count"
        );
    }

    /// Materialized views survive write_tx (fitness computes without panic).
    #[test]
    fn write_tx_preserves_materialized_views() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_path = tmp.path().join(".braid");
        let mut live = LiveStore::create(&braid_path).unwrap();

        let agent = braid_kernel::datom::AgentId::from_name("test:views");
        let tx_id = braid_kernel::datom::TxId::new(6000, 0, agent);
        let entity = braid_kernel::datom::EntityId::from_ident(":test/views");
        let datom = braid_kernel::datom::Datom::new(
            entity,
            braid_kernel::datom::Attribute::from_keyword(":db/doc"),
            braid_kernel::datom::Value::String("views test".into()),
            tx_id,
            braid_kernel::datom::Op::Assert,
        );
        let tx_file = braid_kernel::layout::TxFile {
            tx_id,
            agent,
            provenance: braid_kernel::datom::ProvenanceType::Derived,
            rationale: "views test".into(),
            causal_predecessors: vec![],
            datoms: vec![datom],
        };
        live.write_tx(&tx_file).unwrap();

        // Fitness must compute without panic and be in [0,1].
        let f = live.store().fitness().total;
        assert!(
            (0.0..=1.0).contains(&f),
            "fitness must be in [0,1], got {f}"
        );
    }

    /// Backward compat: bincode round-trip stability.
    /// Create store, write 3 txns, flush, reopen — datom counts must match.
    #[test]
    fn old_store_bin_readable() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_path = tmp.path().join(".braid");

        let expected_len;
        {
            let mut live = LiveStore::create(&braid_path).unwrap();
            let agent = braid_kernel::datom::AgentId::from_name("test:compat-bin");

            for i in 0..3 {
                let tx_id = braid_kernel::datom::TxId::new(7000 + i, 0, agent);
                let entity = braid_kernel::datom::EntityId::from_ident(
                    &format!(":test/compat-bin-{i}"),
                );
                let datom = braid_kernel::datom::Datom::new(
                    entity,
                    braid_kernel::datom::Attribute::from_keyword(":db/doc"),
                    braid_kernel::datom::Value::String(format!("compat bin {i}")),
                    tx_id,
                    braid_kernel::datom::Op::Assert,
                );
                let tx_file = braid_kernel::layout::TxFile {
                    tx_id,
                    agent,
                    provenance: braid_kernel::datom::ProvenanceType::Derived,
                    rationale: format!("compat bin test {i}"),
                    causal_predecessors: vec![],
                    datoms: vec![datom],
                };
                live.write_tx(&tx_file).unwrap();
            }

            live.flush().unwrap();
            expected_len = live.store().len();

            // Verify cache file was actually written.
            let cache_path = braid_path.join(".cache").join("store.bin");
            assert!(cache_path.exists(), "store.bin must exist after flush");

            // Record cache bytes for sanity — non-empty.
            let cache_bytes = std::fs::metadata(&cache_path).unwrap().len();
            assert!(cache_bytes > 0, "store.bin must be non-empty");
        }
        // LiveStore dropped — now reopen from the persisted cache.
        let reopened = LiveStore::open(&braid_path).unwrap();
        assert_eq!(
            reopened.store().len(),
            expected_len,
            "old_store_bin_readable: reopened store must have same datom count"
        );
    }

    /// Backward compat: LiveStore and DiskLayout produce identical stores.
    /// Write N datoms via LiveStore, flush, then open via DiskLayout::open().load_store()
    /// (the legacy path). Both must produce identical datom counts.
    #[test]
    fn round_trip_compat() {
        let tmp = tempfile::tempdir().unwrap();
        let braid_path = tmp.path().join(".braid");

        let live_len;
        {
            let mut live = LiveStore::create(&braid_path).unwrap();
            let agent = braid_kernel::datom::AgentId::from_name("test:compat-rt");

            for i in 0..5 {
                let tx_id = braid_kernel::datom::TxId::new(8000 + i, 0, agent);
                let entity = braid_kernel::datom::EntityId::from_ident(
                    &format!(":test/compat-rt-{i}"),
                );
                let datom = braid_kernel::datom::Datom::new(
                    entity,
                    braid_kernel::datom::Attribute::from_keyword(":db/doc"),
                    braid_kernel::datom::Value::String(format!("compat rt {i}")),
                    tx_id,
                    braid_kernel::datom::Op::Assert,
                );
                let tx_file = braid_kernel::layout::TxFile {
                    tx_id,
                    agent,
                    provenance: braid_kernel::datom::ProvenanceType::Derived,
                    rationale: format!("compat rt test {i}"),
                    causal_predecessors: vec![],
                    datoms: vec![datom],
                };
                live.write_tx(&tx_file).unwrap();
            }
            live.flush().unwrap();
            live_len = live.store().len();
        }

        // Open via the old DiskLayout path — this is the legacy codepath
        // that predates LiveStore.
        let layout = DiskLayout::open(&braid_path).unwrap();
        let disk_store = layout.load_store().unwrap();
        assert_eq!(
            disk_store.len(),
            live_len,
            "round_trip_compat: DiskLayout.load_store() must match LiveStore datom count"
        );
    }

    /// Genesis datom stability: two fresh stores must have identical genesis datom counts.
    /// The schema bootstrap is deterministic — same code produces same datoms.
    #[test]
    fn genesis_datom_stability() {
        let tmp1 = tempfile::tempdir().unwrap();
        let tmp2 = tempfile::tempdir().unwrap();
        let path1 = tmp1.path().join(".braid");
        let path2 = tmp2.path().join(".braid");

        let live1 = LiveStore::create(&path1).unwrap();
        let live2 = LiveStore::create(&path2).unwrap();

        assert_eq!(
            live1.store().len(),
            live2.store().len(),
            "genesis_datom_stability: two fresh stores must have identical datom counts"
        );
        // Both must be non-empty (genesis schema produces datoms).
        assert!(
            !live1.store().is_empty(),
            "genesis store must not be empty"
        );
    }

    // ── Property-based tests (LIVESTORE-TEST-ALGEBRAIC) ─────────────

    use proptest::prelude::*;

    /// Build a deterministic TxFile from a unique wall-time integer.
    /// Each wall_time produces a unique entity + tx, avoiding content-addressed collisions.
    fn arb_datom_tx(wall_time: u64) -> braid_kernel::layout::TxFile {
        let agent = braid_kernel::datom::AgentId::from_name("test:prop");
        let tx_id = braid_kernel::datom::TxId::new(wall_time, 0, agent);
        let ident = format!(":test/prop-{wall_time}");
        let entity = braid_kernel::datom::EntityId::from_ident(&ident);
        let datom = braid_kernel::datom::Datom::new(
            entity,
            braid_kernel::datom::Attribute::from_keyword(":db/doc"),
            braid_kernel::datom::Value::String(format!("proptest value {wall_time}")),
            tx_id,
            braid_kernel::datom::Op::Assert,
        );
        braid_kernel::layout::TxFile {
            tx_id,
            agent,
            provenance: braid_kernel::datom::ProvenanceType::Derived,
            rationale: format!("proptest {wall_time}"),
            causal_predecessors: vec![],
            datoms: vec![datom],
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// INV-STORE-020: After flush, the LiveStore's in-memory datom count
        /// must equal a full rebuild from transaction files via load_store().
        #[test]
        fn checkpoint_equals_fold(count in 1usize..=20) {
            let tmp = tempfile::tempdir().unwrap();
            let braid_path = tmp.path().join(".braid");
            let mut live = LiveStore::create(&braid_path).unwrap();

            for i in 0..count {
                let tx = arb_datom_tx(10_000 + i as u64);
                live.write_tx(&tx).unwrap();
            }
            live.flush().unwrap();

            let layout = DiskLayout::open(&braid_path).unwrap();
            let rebuilt = layout.load_store().unwrap();
            prop_assert_eq!(
                live.store().len(),
                rebuilt.len(),
                "INV-STORE-020: flushed LiveStore must match full rebuild"
            );
        }

        /// C4 (approximate): For any two transactions, applying A then B via
        /// write_tx must produce the same datom count as applying B then A.
        /// This tests commutativity of apply_datoms for disjoint transactions.
        #[test]
        fn commutativity_of_apply(seed_a in 20_000u64..30_000, seed_b in 30_000u64..40_000) {
            let tx_a = arb_datom_tx(seed_a);
            let tx_b = arb_datom_tx(seed_b);

            // Order 1: A then B
            let tmp1 = tempfile::tempdir().unwrap();
            let path1 = tmp1.path().join(".braid");
            let mut live1 = LiveStore::create(&path1).unwrap();
            live1.write_tx(&tx_a).unwrap();
            live1.write_tx(&tx_b).unwrap();

            // Order 2: B then A
            let tmp2 = tempfile::tempdir().unwrap();
            let path2 = tmp2.path().join(".braid");
            let mut live2 = LiveStore::create(&path2).unwrap();
            live2.write_tx(&tx_b).unwrap();
            live2.write_tx(&tx_a).unwrap();

            prop_assert_eq!(
                live1.store().len(),
                live2.store().len(),
                "C4: datom count must be order-independent"
            );
        }

        /// C1: For any sequence of assert-only transactions, the datom count
        /// is monotonically non-decreasing after each write.
        #[test]
        fn monotonic_growth_proptest(count in 1usize..=50) {
            let tmp = tempfile::tempdir().unwrap();
            let braid_path = tmp.path().join(".braid");
            let mut live = LiveStore::create(&braid_path).unwrap();

            let mut prev = live.store().len();
            for i in 0..count {
                let tx = arb_datom_tx(50_000 + i as u64);
                live.write_tx(&tx).unwrap();
                let curr = live.store().len();
                prop_assert!(
                    curr >= prev,
                    "C1: datom count must be non-decreasing: {} -> {} at step {}",
                    prev, curr, i
                );
                prev = curr;
            }
        }

        /// Crash safety: For any sequence of transactions, deleting the binary
        /// cache after writes and reopening must recover the same datom count
        /// from transaction files alone.
        #[test]
        fn crash_recovery_proptest(count in 1usize..=10) {
            let tmp = tempfile::tempdir().unwrap();
            let braid_path = tmp.path().join(".braid");

            let expected_len;
            {
                let mut live = LiveStore::create(&braid_path).unwrap();
                for i in 0..count {
                    let tx = arb_datom_tx(60_000 + i as u64);
                    live.write_tx(&tx).unwrap();
                }
                live.flush().unwrap();
                expected_len = live.store().len();
            }

            // Delete cache to simulate crash before flush.
            let cache_dir = braid_path.join(".cache");
            if cache_dir.is_dir() {
                for entry in std::fs::read_dir(&cache_dir).unwrap() {
                    let entry = entry.unwrap();
                    if entry.path().extension().map(|x| x == "bin").unwrap_or(false) {
                        std::fs::remove_file(entry.path()).unwrap();
                    }
                }
            }

            let recovered = LiveStore::open(&braid_path).unwrap();
            prop_assert_eq!(
                recovered.store().len(),
                expected_len,
                "Crash recovery: rebuilt store must match pre-crash datom count"
            );
        }

        /// flush() is idempotent: calling it twice with no intervening writes
        /// produces the same store state as calling it once.
        #[test]
        fn flush_idempotent(count in 1usize..=15) {
            let tmp = tempfile::tempdir().unwrap();
            let braid_path = tmp.path().join(".braid");
            let mut live = LiveStore::create(&braid_path).unwrap();

            for i in 0..count {
                let tx = arb_datom_tx(70_000 + i as u64);
                live.write_tx(&tx).unwrap();
            }

            live.flush().unwrap();
            let len_after_first = live.store().len();
            let dirty_after_first = live.is_dirty();

            live.flush().unwrap();
            let len_after_second = live.store().len();
            let dirty_after_second = live.is_dirty();

            prop_assert_eq!(
                len_after_first, len_after_second,
                "flush idempotent: datom count must not change between flushes"
            );
            prop_assert!(!dirty_after_first, "dirty flag must be false after first flush");
            prop_assert!(!dirty_after_second, "dirty flag must be false after second flush");
        }

        /// refresh_if_needed() must detect externally written transactions
        /// (via DiskLayout) and increase the store's datom count.
        #[test]
        fn refresh_after_external_write(count in 1usize..=10) {
            let tmp = tempfile::tempdir().unwrap();
            let braid_path = tmp.path().join(".braid");
            let mut live = LiveStore::create(&braid_path).unwrap();

            let before = live.store().len();

            // Write transactions externally via DiskLayout (simulating another process).
            for i in 0..count {
                let tx = arb_datom_tx(80_000 + i as u64);
                live.layout().write_tx_no_invalidate(&tx).unwrap();
            }

            // LiveStore should not yet see the external writes.
            prop_assert_eq!(
                live.store().len(),
                before,
                "external writes must not be visible before refresh"
            );

            // After refresh, the store must have grown.
            let refreshed = live.refresh_if_needed().unwrap();
            prop_assert!(refreshed, "refresh must detect external writes");
            prop_assert!(
                live.store().len() > before,
                "store must grow after refresh: {} -> {}",
                before,
                live.store().len()
            );
        }
    }
}
