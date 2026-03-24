//! LAYOUT IO operations — filesystem persistence for the content-addressed store.
//!
//! This module contains all IO operations for the Braid storage layout.
//! It calls pure kernel functions for computation and performs IO for persistence.
//!
//! # Invariants
//!
//! - **INV-LAYOUT-001**: filename = BLAKE3(bytes) — content-addressed identity.
//! - **INV-LAYOUT-002**: Transaction files are write-once (O_CREAT|O_EXCL).
//! - **INV-LAYOUT-003**: ψ(φ(S)) = S — directory-store isomorphism.
//! - **INV-LAYOUT-005**: verify_integrity detects corrupt files.
//! - **INV-LAYOUT-007**: init_layout creates well-formed directory structure.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use braid_kernel::datom::{AgentId, Datom, TxId};
use braid_kernel::layout::{
    deserialize_tx, serialize_tx, ContentHash, IntegrityError, IntegrityReport, LayoutConfig,
    TxFile, TxFilePath,
};
use braid_kernel::Store;
use serde::{Deserialize, Serialize};

use crate::error::BraidError;

// ---------------------------------------------------------------------------
// Cache metadata — persisted at .braid/.cache/meta.json
// ---------------------------------------------------------------------------

/// Cache metadata for freshness validation.
///
/// The `txn_fingerprint` is a BLAKE3 hash of the sorted, concatenated tx hashes.
/// If the set of transaction files changes (add, remove, corrupt), the fingerprint
/// changes and the cache is invalidated.
#[derive(Serialize, Deserialize, Debug)]
struct CacheMeta {
    /// BLAKE3 hash of sorted tx hash list — changes when any txn file is added/removed.
    txn_fingerprint: String,
    /// Number of datoms when the cache was written (diagnostic, not used for validation).
    datom_count: usize,
    /// Unix timestamp (seconds) when the cache was written.
    created_at: u64,
    /// POLICY-6: List of tx hashes when the cache was written (for incremental delta).
    /// When present, enables incremental loading: only new transactions are parsed.
    #[serde(default)]
    tx_hashes: Vec<String>,
}

/// On-disk layout handle.
///
/// All filesystem operations go through this struct.
/// The kernel computes; this struct persists.
pub struct DiskLayout {
    /// Root directory (e.g., `.braid/`).
    pub root: PathBuf,
    /// Layout configuration.
    pub config: LayoutConfig,
}

impl DiskLayout {
    /// Initialize a new layout at the given root path.
    ///
    /// Creates the directory structure:
    /// ```text
    /// {root}/
    /// ├── txns/           ← Content-addressed transaction files
    /// ├── heads/          ← Agent frontier caches
    /// ├── .cache/         ← Derived indexes (gitignored)
    /// └── .gitignore      ← Ignores .cache/
    /// ```
    ///
    /// Writes the genesis transaction to `txns/`. Idempotent: safe to call twice.
    pub fn init(root: &Path) -> Result<Self, BraidError> {
        let config = LayoutConfig::default();
        let layout = DiskLayout {
            root: root.to_path_buf(),
            config,
        };

        // Create directory structure
        fs::create_dir_all(root.join("txns"))?;
        fs::create_dir_all(root.join("heads"))?;
        fs::create_dir_all(root.join(".cache"))?;

        // Write .gitignore
        let gitignore_path = root.join(".gitignore");
        if !gitignore_path.exists() {
            fs::write(&gitignore_path, ".cache/\n")?;
        }

        // Write genesis transaction
        let genesis_store = Store::genesis();
        let genesis_tx = layout.build_genesis_tx_file(&genesis_store);
        layout.write_tx(&genesis_tx)?;

        // Write genesis.edn at well-known path (duplicate for convenience)
        let genesis_bytes = serialize_tx(&genesis_tx);
        let genesis_edn_path = root.join("genesis.edn");
        if !genesis_edn_path.exists() {
            fs::write(&genesis_edn_path, &genesis_bytes)?;
        }

        Ok(layout)
    }

    /// Open an existing layout.
    pub fn open(root: &Path) -> Result<Self, BraidError> {
        if !root.join("txns").is_dir() {
            return Err(BraidError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("not a valid braid layout: {}", root.display()),
            )));
        }
        Ok(DiskLayout {
            root: root.to_path_buf(),
            config: LayoutConfig::default(),
        })
    }

    /// Write a transaction file to `txns/`.
    ///
    /// Uses O_CREAT|O_EXCL semantics: creates new file atomically,
    /// silently succeeds if the file already exists (idempotent by content identity).
    pub fn write_tx(&self, tx: &TxFile) -> Result<TxFilePath, BraidError> {
        let bytes = serialize_tx(tx);
        let hash = ContentHash::of(&bytes);
        let file_path = TxFilePath::from_hash(&hash);

        let shard_dir = self.root.join("txns").join(&file_path.shard);
        fs::create_dir_all(&shard_dir)?;

        let full_path = shard_dir.join(&file_path.filename);

        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&full_path)
        {
            Ok(mut file) => {
                file.write_all(&bytes)?;
                file.sync_all()?;
                // Invalidate the store cache so the next load_store() picks up
                // this new transaction. Without this, commands that write and then
                // read within the same process (or rapid succession) would miss
                // the new data — causing silent data loss (CACHE-BUG t-03fd2cd5).
                self.invalidate_cache();
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Idempotent: same content, same hash, same file.
            }
            Err(e) => return Err(BraidError::Io(e)),
        }

        Ok(file_path)
    }

    /// Write a transaction file without invalidating the cache.
    ///
    /// Used by [`LiveStore`] which manages its own in-memory state and
    /// cache serialization via the dirty-flag pattern (INV-STORE-021).
    /// The cache is not invalidated because LiveStore will update it
    /// on `flush()` or `Drop`.
    pub fn write_tx_no_invalidate(&self, tx: &TxFile) -> Result<TxFilePath, BraidError> {
        let bytes = serialize_tx(tx);
        let hash = ContentHash::of(&bytes);
        let file_path = TxFilePath::from_hash(&hash);

        let shard_dir = self.root.join("txns").join(&file_path.shard);
        fs::create_dir_all(&shard_dir)?;

        let full_path = shard_dir.join(&file_path.filename);

        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&full_path)
        {
            Ok(mut file) => {
                file.write_all(&bytes)?;
                file.sync_all()?;
                // No cache invalidation — LiveStore handles this.
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Idempotent: same content, same hash, same file.
            }
            Err(e) => return Err(BraidError::Io(e)),
        }

        Ok(file_path)
    }

    /// Invalidate the store cache so the next load picks up new transactions.
    ///
    /// Called after every write_tx() to prevent stale reads. The cache files
    /// (.cache/store.bin and .cache/meta.json) are deleted; the next load_store()
    /// will do a full rebuild from txn files and write a fresh cache.
    fn invalidate_cache(&self) {
        let cache_dir = self.root.join(".cache");
        let _ = fs::remove_file(cache_dir.join("store.bin"));
        let _ = fs::remove_file(cache_dir.join("meta.json"));
    }

    /// Read a transaction file by its hash.
    ///
    /// `hash_hex` must be a valid lowercase hex string of at least 2 characters
    /// (BLAKE3 hashes are 64 hex chars). Returns an error for malformed input.
    ///
    /// INV-LAYOUT-005: After reading, verifies that the content hash of the file
    /// matches the expected hash derived from the filename. Returns an integrity
    /// error if the hash does not match (corrupt or tampered file).
    pub fn read_tx(&self, hash_hex: &str) -> Result<TxFile, BraidError> {
        if hash_hex.len() < 2 || !hash_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(BraidError::Parse(format!(
                "invalid tx hash: expected hex string >= 2 chars, got {:?}",
                hash_hex
            )));
        }
        let prefix = &hash_hex[..2];
        let path = self
            .root
            .join("txns")
            .join(prefix)
            .join(format!("{hash_hex}.edn"));
        let bytes = fs::read(&path)?;

        // INV-LAYOUT-005: Verify content hash matches expected hash from filename.
        let actual_hash = ContentHash::of(&bytes);
        let actual_hex = actual_hash.to_hex();
        if actual_hex != hash_hex {
            return Err(BraidError::Validation(format!(
                "INV-LAYOUT-005: content hash mismatch for tx {hash_hex}: \
                 expected {hash_hex}, got {actual_hex} (file may be corrupt or tampered)"
            )));
        }

        let tx = deserialize_tx(&bytes)?;
        Ok(tx)
    }

    /// List all transaction hashes in the layout.
    pub fn list_tx_hashes(&self) -> Result<Vec<String>, BraidError> {
        let txns_dir = self.root.join("txns");
        let mut hashes = Vec::new();

        if !txns_dir.is_dir() {
            return Ok(hashes);
        }

        for shard_entry in fs::read_dir(&txns_dir)? {
            let shard_entry = shard_entry?;
            if !shard_entry.file_type()?.is_dir() {
                continue;
            }
            for file_entry in fs::read_dir(shard_entry.path())? {
                let file_entry = file_entry?;
                let name = file_entry.file_name();
                let name_str = name.to_string_lossy();
                if let Some(hash) = name_str.strip_suffix(".edn") {
                    hashes.push(hash.to_string());
                }
            }
        }

        hashes.sort(); // Deterministic ordering
        Ok(hashes)
    }

    // -------------------------------------------------------------------
    // Cache persistence (.braid/.cache/)
    // -------------------------------------------------------------------

    /// Path to the cache directory: `.braid/.cache/`.
    pub fn cache_dir(&self) -> PathBuf {
        self.root.join(".cache")
    }

    /// Compute a fingerprint of the txns/ directory.
    ///
    /// The fingerprint is the BLAKE3 hash of the sorted, newline-joined tx hashes.
    /// Any change to the set of transaction files (add, remove, rename) changes
    /// the fingerprint and invalidates the cache.
    fn txn_fingerprint(&self, hashes: &[String]) -> String {
        let joined = hashes.join("\n");
        ContentHash::of(joined.as_bytes()).to_hex()
    }

    /// Write the full Store to `.braid/.cache/store.bin` with a
    /// freshness metadata file at `.braid/.cache/meta.json`.
    ///
    /// The cache contains a bincode-serialized `Store` (including all 6
    /// indexes, schema, frontier, and clock). Loading from cache avoids
    /// both parsing N individual EDN transaction files AND rebuilding
    /// indexes via `Store::from_datoms()`.
    ///
    /// Also writes the legacy `datoms.bin` for backward compatibility with
    /// any external tooling that reads the cache directly.
    pub fn write_index_cache(&self, store: &Store) -> Result<(), BraidError> {
        let cache_dir = self.cache_dir();
        fs::create_dir_all(&cache_dir)?;

        let datom_count = store.len();

        // Serialize full Store via bincode (includes all indexes).
        let store_encoded = bincode::serialize(store)
            .map_err(|e| BraidError::Parse(format!("bincode serialize store: {e}")))?;

        // Write store.bin atomically: write to .tmp, then rename.
        let store_bin_path = cache_dir.join("store.bin");
        let store_tmp_path = cache_dir.join("store.bin.tmp");
        fs::write(&store_tmp_path, &store_encoded)?;
        fs::rename(&store_tmp_path, &store_bin_path)?;

        // Write meta.json.
        let hashes = self.list_tx_hashes()?;
        let fingerprint = self.txn_fingerprint(&hashes);
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let meta = CacheMeta {
            txn_fingerprint: fingerprint,
            datom_count,
            created_at: now,
            tx_hashes: hashes,
        };
        let meta_json =
            serde_json::to_string_pretty(&meta).map_err(|e| BraidError::Parse(e.to_string()))?;
        let meta_path = cache_dir.join("meta.json");
        let meta_tmp = cache_dir.join("meta.json.tmp");
        fs::write(&meta_tmp, meta_json)?;
        fs::rename(&meta_tmp, &meta_path)?;

        // Clean up legacy datoms.bin if present (no longer used for loading).
        let legacy_path = cache_dir.join("datoms.bin");
        if legacy_path.exists() {
            let _ = fs::remove_file(&legacy_path);
        }

        Ok(())
    }

    /// Try to read the cached full Store from `.braid/.cache/store.bin`.
    ///
    /// Returns `None` if the cache is missing, stale, or corrupt.
    /// "Stale" means the txn_fingerprint in meta.json does not match the
    /// current txns/ directory contents.
    ///
    /// This loads the full Store including all 6 indexes, schema, frontier,
    /// and clock — skipping the expensive `Store::from_datoms()` rebuild.
    fn read_index_cache(&self, current_fingerprint: &str) -> Option<Store> {
        let cache_dir = self.cache_dir();

        // 1. Read and validate meta.json.
        let meta_bytes = fs::read(cache_dir.join("meta.json")).ok()?;
        let meta: CacheMeta = serde_json::from_slice(&meta_bytes).ok()?;

        if meta.txn_fingerprint != current_fingerprint {
            return None; // Cache is stale.
        }

        // 2. Read and deserialize store.bin (full Store with indexes).
        let bin_bytes = fs::read(cache_dir.join("store.bin")).ok()?;
        let store: Store = bincode::deserialize(&bin_bytes).ok()?;

        // Quick sanity check: datom count should match meta.
        if store.len() != meta.datom_count {
            return None; // Corrupt cache.
        }

        Some(store)
    }

    /// POLICY-6: Read cached store WITH the hash list it was built from.
    ///
    /// Used for incremental loading: if the cached store exists but the
    /// fingerprint doesn't match, the caller can compute the delta (new
    /// transactions) and apply them incrementally via Store::transact().
    fn read_index_cache_with_hashes(&self) -> Option<(Store, Vec<String>)> {
        let cache_dir = self.cache_dir();
        let meta_bytes = fs::read(cache_dir.join("meta.json")).ok()?;
        let meta: CacheMeta = serde_json::from_slice(&meta_bytes).ok()?;

        // Must have tx_hashes for incremental path
        if meta.tx_hashes.is_empty() {
            return None;
        }

        let bin_bytes = fs::read(cache_dir.join("store.bin")).ok()?;
        let store: Store = bincode::deserialize(&bin_bytes).ok()?;

        if store.len() != meta.datom_count {
            return None; // Corrupt cache
        }

        Some((store, meta.tx_hashes))
    }

    /// Load the entire store from the layout (ψ function).
    ///
    /// This is `ψ(L) = ⋃ { tx.datoms | tx ∈ L.txns }`.
    /// Reconstructs the Store from all transaction files.
    ///
    /// **Cache fast path**: If `.braid/.cache/store.bin` exists and is fresh
    /// (txn_fingerprint matches the current txns/ directory), the full Store
    /// (including all 6 indexes, schema, frontier, clock) is deserialized
    /// directly — skipping both EDN parsing AND `Store::from_datoms()` index
    /// rebuilding. This is the primary performance optimization for startup.
    ///
    /// After a slow-path load, the cache is written for subsequent calls.
    pub fn load_store(&self) -> Result<Store, BraidError> {
        let hashes = self.list_tx_hashes()?;
        let fingerprint = self.txn_fingerprint(&hashes);

        // Fast path: exact cache hit (skips from_datoms rebuild).
        if let Some(store) = self.read_index_cache(&fingerprint) {
            return Ok(store);
        }

        // POLICY-6: Incremental path — try loading cached store + delta.
        // If a cached store exists with a SUBSET of the current hashes,
        // load it and apply only the new transactions via Store::transact().
        // This is O(k) where k = new transactions since last cache, typically 1-5.
        if let Some((cached_store, cached_hashes)) = self.read_index_cache_with_hashes() {
            let cached_set: std::collections::HashSet<&str> =
                cached_hashes.iter().map(|s| s.as_str()).collect();
            let delta_hashes: Vec<&String> = hashes
                .iter()
                .filter(|h| !cached_set.contains(h.as_str()))
                .collect();

            // Only use incremental if delta is small (< 50% of total)
            // and cached hashes are a subset of current (no deletions, C1)
            let all_cached_present = cached_hashes.iter().all(|ch| hashes.contains(ch));
            if all_cached_present && !delta_hashes.is_empty() && delta_hashes.len() < hashes.len() / 2
            {
                let mut store = cached_store;
                for hash in &delta_hashes {
                    if let Ok(tx) = self.read_tx(hash) {
                        let agent = tx.agent;
                        let provenance = tx.provenance;
                        let rationale = tx.rationale.clone();
                        let mut builder = braid_kernel::store::Transaction::new(
                            agent,
                            provenance,
                            &rationale,
                        );
                        for datom in &tx.datoms {
                            builder = builder.assert(
                                datom.entity,
                                datom.attribute.clone(),
                                datom.value.clone(),
                            );
                        }
                        // Best-effort: if commit/transact fails, fall through to full rebuild
                        if let Ok(committed) = builder.commit(&store) {
                            if store.transact(committed).is_err() {
                                // Schema validation failure on incremental — fall through
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }

                // Verify: datom count should be >= cached (monotonic growth, C1)
                // If it is, cache the result and return
                if store.len() >= cached_hashes.len() {
                    let _ = self.write_index_cache(&store);
                    return Ok(store);
                }
                // Otherwise fall through to full rebuild
            }
        }

        // Slow path: parse all transaction files.
        let mut all_datoms: BTreeSet<Datom> = BTreeSet::new();

        for hash in &hashes {
            let tx = self.read_tx(hash)?;

            // Collect datoms
            for datom in tx.datoms {
                all_datoms.insert(datom);
            }
        }

        let store = Store::from_datoms(all_datoms);

        // Write cache for next time (best-effort — do not fail the load on cache write error).
        let _ = self.write_index_cache(&store);

        Ok(store)
    }

    /// Verify integrity of all transaction files.
    ///
    /// Checks that every file in `txns/` has:
    /// 1. A filename matching its BLAKE3 content hash (INV-LAYOUT-001)
    /// 2. Valid EDN content that parses as a transaction (INV-LAYOUT-009)
    pub fn verify_integrity(&self) -> Result<IntegrityReport, BraidError> {
        let txns_dir = self.root.join("txns");
        let mut report = IntegrityReport::default();

        if !txns_dir.is_dir() {
            return Ok(report);
        }

        for shard_entry in fs::read_dir(&txns_dir)? {
            let shard_entry = shard_entry?;
            if !shard_entry.file_type()?.is_dir() {
                continue;
            }
            for file_entry in fs::read_dir(shard_entry.path())? {
                let file_entry = file_entry?;
                let name = file_entry.file_name();
                let name_str = name.to_string_lossy();

                let Some(expected_hash_hex) = name_str.strip_suffix(".edn") else {
                    continue;
                };

                report.total_files += 1;

                let bytes = fs::read(file_entry.path())?;
                let actual_hash = ContentHash::of(&bytes);
                let actual_hex = actual_hash.to_hex();

                let file_path = TxFilePath::from_hash(&actual_hash);

                if actual_hex != expected_hash_hex {
                    let expected = ContentHash::of(expected_hash_hex.as_bytes()); // approximate
                    report.corrupted.push((
                        TxFilePath {
                            shard: expected_hash_hex[..2].to_string(),
                            filename: format!("{expected_hash_hex}.edn"),
                        },
                        IntegrityError::HashMismatch {
                            expected,
                            actual: actual_hash,
                        },
                    ));
                    continue;
                }

                // Try to parse
                match deserialize_tx(&bytes) {
                    Ok(_) => {
                        report.verified += 1;
                    }
                    Err(_) => {
                        report.orphaned.push(file_path);
                    }
                }
            }
        }

        Ok(report)
    }

    /// Build a TxFile from a genesis store (for writing).
    fn build_genesis_tx_file(&self, store: &Store) -> TxFile {
        let system_agent = AgentId::from_name("braid:system");
        let genesis_tx_id = TxId::new(0, 0, system_agent);

        TxFile {
            tx_id: genesis_tx_id,
            agent: system_agent,
            provenance: braid_kernel::datom::ProvenanceType::Observed,
            rationale: "Genesis: axiomatic meta-schema attributes".to_string(),
            causal_predecessors: vec![],
            datoms: store.datoms().cloned().collect(),
        }
    }
}

/// Walk up from `start` looking for a `.braid/` directory.
///
/// Mirrors git's `.git/` discovery. Returns the path to the `.braid/` dir if found.
/// Used when `--path .braid` (the default) doesn't exist in the current directory.
pub fn find_braid_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    // Canonicalize if possible for reliable traversal
    if let Ok(canonical) = current.canonicalize() {
        current = canonical;
    }
    loop {
        let candidate = current.join(".braid");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn find_braid_root_from_subdir() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        DiskLayout::init(&root).unwrap();

        // Create a subdirectory
        let subdir = tmp.path().join("src").join("deep");
        fs::create_dir_all(&subdir).unwrap();

        // find_braid_root should walk up and find .braid/
        let found = find_braid_root(&subdir);
        assert!(found.is_some(), "should find .braid from subdir");
        assert!(found.unwrap().ends_with(".braid"));
    }

    #[test]
    fn find_braid_root_returns_none_when_absent() {
        let tmp = TempDir::new().unwrap();
        let found = find_braid_root(tmp.path());
        assert!(found.is_none(), "should return None when no .braid exists");
    }

    #[test]
    fn init_creates_structure() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        let layout = DiskLayout::init(&root).unwrap();

        assert!(root.join("txns").is_dir());
        assert!(root.join("heads").is_dir());
        assert!(root.join(".cache").is_dir());
        assert!(root.join(".gitignore").is_file());
        assert!(root.join("genesis.edn").is_file());

        // Should have at least one transaction file (genesis)
        let hashes = layout.list_tx_hashes().unwrap();
        assert!(!hashes.is_empty(), "genesis transaction should be written");
    }

    #[test]
    fn init_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");

        let layout1 = DiskLayout::init(&root).unwrap();
        let hashes1 = layout1.list_tx_hashes().unwrap();

        let layout2 = DiskLayout::init(&root).unwrap();
        let hashes2 = layout2.list_tx_hashes().unwrap();

        assert_eq!(hashes1, hashes2, "init should be idempotent");
    }

    #[test]
    fn store_round_trip() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        let layout = DiskLayout::init(&root).unwrap();

        // Load the store from disk
        let loaded = layout.load_store().unwrap();
        let genesis = Store::genesis();

        // The loaded store should contain all genesis datoms
        let genesis_datoms: BTreeSet<_> = genesis.datoms().cloned().collect();
        let loaded_datoms: BTreeSet<_> = loaded.datoms().cloned().collect();

        assert_eq!(genesis_datoms, loaded_datoms, "INV-LAYOUT-003: ψ(φ(S)) = S");
    }

    #[test]
    fn verify_integrity_clean() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        let layout = DiskLayout::init(&root).unwrap();

        let report = layout.verify_integrity().unwrap();
        assert!(
            report.is_clean(),
            "fresh layout should have clean integrity"
        );
        assert!(report.total_files > 0);
        assert_eq!(report.verified, report.total_files);
    }

    #[test]
    fn verify_integrity_detects_corruption() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        let layout = DiskLayout::init(&root).unwrap();

        // Corrupt a file by writing different content
        let hashes = layout.list_tx_hashes().unwrap();
        let hash = &hashes[0];
        let prefix = &hash[..2];
        let path = root.join("txns").join(prefix).join(format!("{hash}.edn"));
        fs::write(&path, b"corrupted content").unwrap();

        let report = layout.verify_integrity().unwrap();
        assert!(
            !report.is_clean(),
            "corrupted layout should fail integrity check"
        );
    }

    #[test]
    fn write_and_read_tx() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        let layout = DiskLayout::init(&root).unwrap();

        let agent = AgentId::from_name("test-agent");
        let tx_id = TxId::new(1000, 0, agent);
        let entity = braid_kernel::datom::EntityId::from_ident(":test/thing");

        let tx = TxFile {
            tx_id,
            agent,
            provenance: braid_kernel::datom::ProvenanceType::Observed,
            rationale: "test write".to_string(),
            causal_predecessors: vec![],
            datoms: vec![braid_kernel::datom::Datom {
                entity,
                attribute: braid_kernel::datom::Attribute::from_keyword(":db/doc"),
                value: braid_kernel::datom::Value::String("test value".to_string()),
                tx: tx_id,
                op: braid_kernel::datom::Op::Assert,
            }],
        };

        let file_path = layout.write_tx(&tx).unwrap();
        let hash = file_path.filename.strip_suffix(".edn").unwrap().to_string();
        let read_back = layout.read_tx(&hash).unwrap();

        assert_eq!(read_back.tx_id, tx.tx_id);
        assert_eq!(read_back.rationale, tx.rationale);
        assert_eq!(read_back.datoms.len(), 1);
    }

    // Verifies: INV-LAYOUT-010 — Concurrent Write Safety (O_CREAT|O_EXCL)
    //
    // Two threads writing the SAME transaction (identical content hash) concurrently:
    // exactly one create_new(true) succeeds, the other gets AlreadyExists and is
    // silently absorbed. The file exists with correct content afterward.
    #[test]
    fn concurrent_writes_to_same_hash_are_safe() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        let _layout = DiskLayout::init(&root).unwrap();

        // Build a deterministic transaction (both threads will write the same bytes)
        let agent = AgentId::from_name("concurrent-agent");
        let tx_id = TxId::new(5000, 0, agent);
        let entity = braid_kernel::datom::EntityId::from_ident(":test/concurrent");

        let tx = TxFile {
            tx_id,
            agent,
            provenance: braid_kernel::datom::ProvenanceType::Observed,
            rationale: "concurrent write test".to_string(),
            causal_predecessors: vec![],
            datoms: vec![braid_kernel::datom::Datom {
                entity,
                attribute: braid_kernel::datom::Attribute::from_keyword(":db/doc"),
                value: braid_kernel::datom::Value::String("concurrent value".to_string()),
                tx: tx_id,
                op: braid_kernel::datom::Op::Assert,
            }],
        };

        // Pre-compute the expected hash so we can verify afterward
        let bytes = braid_kernel::layout::serialize_tx(&tx);
        let expected_hash = braid_kernel::layout::ContentHash::of(&bytes);
        let expected_hex = expected_hash.to_hex();

        // Share the layout root and tx across threads
        let root_arc = Arc::new(root.clone());
        let tx_arc = Arc::new(tx);
        let barrier = Arc::new(Barrier::new(2));

        let handles: Vec<_> = (0..2)
            .map(|_| {
                let root_c = Arc::clone(&root_arc);
                let tx_c = Arc::clone(&tx_arc);
                let barrier_c = Arc::clone(&barrier);
                thread::spawn(move || {
                    let layout = DiskLayout::open(&root_c).unwrap();
                    // Synchronize: both threads hit the barrier, then race to write
                    barrier_c.wait();
                    layout.write_tx(&tx_c)
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Both must succeed (one creates, the other gets AlreadyExists → Ok)
        for (i, r) in results.iter().enumerate() {
            assert!(
                r.is_ok(),
                "INV-LAYOUT-010: thread {i} should succeed, got: {:?}",
                r.as_ref().err()
            );
        }

        // The file must exist with the correct content
        let prefix = &expected_hex[..2];
        let full_path = root
            .join("txns")
            .join(prefix)
            .join(format!("{expected_hex}.edn"));
        assert!(
            full_path.exists(),
            "INV-LAYOUT-010: tx file must exist after concurrent writes"
        );

        let on_disk = fs::read(&full_path).unwrap();
        let on_disk_hash = braid_kernel::layout::ContentHash::of(&on_disk);
        assert_eq!(
            on_disk_hash.to_hex(),
            expected_hex,
            "INV-LAYOUT-010: on-disk content must match expected hash"
        );

        // Verify the file is readable and matches the original transaction
        let layout = DiskLayout::open(&root).unwrap();
        let read_back = layout.read_tx(&expected_hex).unwrap();
        assert_eq!(read_back.tx_id, tx_arc.tx_id);
        assert_eq!(read_back.rationale, tx_arc.rationale);
        assert_eq!(read_back.datoms.len(), 1);
    }

    // Verifies: INV-LAYOUT-005 — Content hash verified on read
    #[test]
    fn read_tx_verifies_content_hash() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        let layout = DiskLayout::init(&root).unwrap();

        // Write a valid transaction
        let agent = AgentId::from_name("hash-check-agent");
        let tx_id = TxId::new(2000, 0, agent);
        let entity = braid_kernel::datom::EntityId::from_ident(":test/hash-check");

        let tx = TxFile {
            tx_id,
            agent,
            provenance: braid_kernel::datom::ProvenanceType::Observed,
            rationale: "hash verification test".to_string(),
            causal_predecessors: vec![],
            datoms: vec![braid_kernel::datom::Datom {
                entity,
                attribute: braid_kernel::datom::Attribute::from_keyword(":db/doc"),
                value: braid_kernel::datom::Value::String("verify me".to_string()),
                tx: tx_id,
                op: braid_kernel::datom::Op::Assert,
            }],
        };

        let file_path = layout.write_tx(&tx).unwrap();
        let hash = file_path.filename.strip_suffix(".edn").unwrap().to_string();

        // Positive case: uncorrupted file reads successfully
        assert!(
            layout.read_tx(&hash).is_ok(),
            "INV-LAYOUT-005: valid file should pass hash verification"
        );

        // Corrupt the file content (but keep the same filename/hash)
        let prefix = &hash[..2];
        let path = root.join("txns").join(prefix).join(format!("{hash}.edn"));
        fs::write(&path, b"corrupted content that does not match the hash").unwrap();

        // Negative case: corrupted file should fail hash verification
        let result = layout.read_tx(&hash);
        assert!(
            result.is_err(),
            "INV-LAYOUT-005: corrupted file must fail hash verification"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("INV-LAYOUT-005") || err_msg.contains("content hash mismatch"),
            "Error should reference INV-LAYOUT-005: {err_msg}"
        );
    }

    // -------------------------------------------------------------------
    // Cache persistence tests
    // -------------------------------------------------------------------

    #[test]
    fn load_store_creates_cache_on_first_load() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        let layout = DiskLayout::init(&root).unwrap();

        // Cache should not exist yet (init does not populate it).
        assert!(
            !root.join(".cache").join("store.bin").exists(),
            "cache should not exist before first load_store"
        );

        let _store = layout.load_store().unwrap();

        // After load_store, cache should be populated.
        assert!(
            root.join(".cache").join("store.bin").exists(),
            "store.bin should exist after load_store"
        );
        assert!(
            root.join(".cache").join("meta.json").exists(),
            "meta.json should exist after load_store"
        );
    }

    #[test]
    fn load_store_uses_cache_on_second_load() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        let layout = DiskLayout::init(&root).unwrap();

        // First load: slow path, writes cache.
        let store1 = layout.load_store().unwrap();
        let datoms1: BTreeSet<_> = store1.datoms().cloned().collect();

        // Second load: should use cache fast path and produce identical store.
        let store2 = layout.load_store().unwrap();
        let datoms2: BTreeSet<_> = store2.datoms().cloned().collect();

        assert_eq!(
            datoms1, datoms2,
            "cached load must produce identical datom set"
        );
    }

    #[test]
    fn cache_invalidated_by_new_transaction() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        let layout = DiskLayout::init(&root).unwrap();

        // First load populates cache.
        let store1 = layout.load_store().unwrap();
        let count1 = store1.len();

        // Write a new transaction.
        let agent = AgentId::from_name("cache-test-agent");
        let tx_id = TxId::new(3000, 0, agent);
        let entity = braid_kernel::datom::EntityId::from_ident(":test/cache-invalidation");

        let tx = TxFile {
            tx_id,
            agent,
            provenance: braid_kernel::datom::ProvenanceType::Observed,
            rationale: "cache invalidation test".to_string(),
            causal_predecessors: vec![],
            datoms: vec![braid_kernel::datom::Datom {
                entity,
                attribute: braid_kernel::datom::Attribute::from_keyword(":db/doc"),
                value: braid_kernel::datom::Value::String("new datom".to_string()),
                tx: tx_id,
                op: braid_kernel::datom::Op::Assert,
            }],
        };
        layout.write_tx(&tx).unwrap();

        // Second load should detect stale cache and reload from txns/.
        let store2 = layout.load_store().unwrap();
        assert!(
            store2.len() > count1,
            "store should have more datoms after new tx: {} vs {}",
            store2.len(),
            count1,
        );
    }

    #[test]
    fn cache_handles_corrupt_store_bin() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        let layout = DiskLayout::init(&root).unwrap();

        // Populate cache.
        let store1 = layout.load_store().unwrap();
        let datoms1: BTreeSet<_> = store1.datoms().cloned().collect();

        // Corrupt store.bin.
        fs::write(root.join(".cache").join("store.bin"), b"garbage").unwrap();

        // Should fall through to slow path and produce correct result.
        let store2 = layout.load_store().unwrap();
        let datoms2: BTreeSet<_> = store2.datoms().cloned().collect();

        assert_eq!(
            datoms1, datoms2,
            "corrupt cache should fall back to slow path"
        );
    }

    #[test]
    fn cache_handles_missing_meta_json() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        let layout = DiskLayout::init(&root).unwrap();

        // Populate cache.
        let store1 = layout.load_store().unwrap();
        let datoms1: BTreeSet<_> = store1.datoms().cloned().collect();

        // Delete meta.json but keep datoms.bin.
        fs::remove_file(root.join(".cache").join("meta.json")).unwrap();

        // Should fall through to slow path.
        let store2 = layout.load_store().unwrap();
        let datoms2: BTreeSet<_> = store2.datoms().cloned().collect();

        assert_eq!(
            datoms1, datoms2,
            "missing meta.json should fall back to slow path"
        );
    }

    #[test]
    fn write_index_cache_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        let layout = DiskLayout::init(&root).unwrap();

        let store = layout.load_store().unwrap();

        // Write cache twice — should not fail and should produce identical files.
        layout.write_index_cache(&store).unwrap();
        let meta1 = fs::read(root.join(".cache").join("meta.json")).unwrap();
        let bin1 = fs::read(root.join(".cache").join("store.bin")).unwrap();

        layout.write_index_cache(&store).unwrap();
        let meta2 = fs::read(root.join(".cache").join("meta.json")).unwrap();
        let bin2 = fs::read(root.join(".cache").join("store.bin")).unwrap();

        // store.bin must be byte-identical (deterministic serialization).
        assert_eq!(bin1, bin2, "store.bin should be deterministic");
        // meta.json may differ in created_at but txn_fingerprint and datom_count should match.
        let m1: CacheMeta = serde_json::from_slice(&meta1).unwrap();
        let m2: CacheMeta = serde_json::from_slice(&meta2).unwrap();
        assert_eq!(m1.txn_fingerprint, m2.txn_fingerprint);
        assert_eq!(m1.datom_count, m2.datom_count);
    }

    #[test]
    fn cache_meta_records_correct_datom_count() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        let layout = DiskLayout::init(&root).unwrap();

        let store = layout.load_store().unwrap();
        let expected_count = store.len();

        let meta_bytes = fs::read(root.join(".cache").join("meta.json")).unwrap();
        let meta: CacheMeta = serde_json::from_slice(&meta_bytes).unwrap();

        assert_eq!(
            meta.datom_count, expected_count,
            "meta.datom_count should match store.len()"
        );
    }

    /// T2-3: Store bincode round-trip — serialize full Store, deserialize,
    /// verify datom count and entity set match exactly.
    #[test]
    fn store_bincode_round_trip() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        let layout = DiskLayout::init(&root).unwrap();

        // Add a non-genesis transaction to make the store non-trivial.
        let agent = AgentId::from_name("round-trip-agent");
        let tx_id = TxId::new(9000, 0, agent);
        let entity = braid_kernel::datom::EntityId::from_ident(":test/round-trip");

        let tx = TxFile {
            tx_id,
            agent,
            provenance: braid_kernel::datom::ProvenanceType::Observed,
            rationale: "bincode round-trip test".to_string(),
            causal_predecessors: vec![],
            datoms: vec![braid_kernel::datom::Datom {
                entity,
                attribute: braid_kernel::datom::Attribute::from_keyword(":db/doc"),
                value: braid_kernel::datom::Value::String("round-trip value".to_string()),
                tx: tx_id,
                op: braid_kernel::datom::Op::Assert,
            }],
        };
        layout.write_tx(&tx).unwrap();

        // Load the store (slow path — builds indexes via from_datoms).
        let original = layout.load_store().unwrap();

        // Serialize the full Store via bincode.
        let encoded = bincode::serialize(&original).expect("serialize should succeed");

        // Deserialize back.
        let restored: Store = bincode::deserialize(&encoded).expect("deserialize should succeed");

        // Datom count must match.
        assert_eq!(
            original.len(),
            restored.len(),
            "datom count must survive round-trip"
        );

        // Entity sets must match.
        let original_entities: BTreeSet<_> = original.datoms().map(|d| d.entity).collect();
        let restored_entities: BTreeSet<_> = restored.datoms().map(|d| d.entity).collect();
        assert_eq!(
            original_entities, restored_entities,
            "entity sets must survive round-trip"
        );

        // Datom sets must be identical.
        let original_datoms: BTreeSet<_> = original.datoms().cloned().collect();
        let restored_datoms: BTreeSet<_> = restored.datoms().cloned().collect();
        assert_eq!(
            original_datoms, restored_datoms,
            "datom sets must be byte-identical after round-trip"
        );
    }

    /// T2-3: Cached Store load skips from_datoms — verify second load
    /// produces a store with identical entity index and live view.
    #[test]
    fn cached_store_preserves_indexes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join(".braid");
        let layout = DiskLayout::init(&root).unwrap();

        // Add a transaction with a doc attribute to exercise AVET and LIVE indexes.
        let agent = AgentId::from_name("index-test-agent");
        let tx_id = TxId::new(8000, 0, agent);
        let entity = braid_kernel::datom::EntityId::from_ident(":test/index-check");

        let tx = TxFile {
            tx_id,
            agent,
            provenance: braid_kernel::datom::ProvenanceType::Observed,
            rationale: "index preservation test".to_string(),
            causal_predecessors: vec![],
            datoms: vec![braid_kernel::datom::Datom {
                entity,
                attribute: braid_kernel::datom::Attribute::from_keyword(":db/doc"),
                value: braid_kernel::datom::Value::String("index test value".to_string()),
                tx: tx_id,
                op: braid_kernel::datom::Op::Assert,
            }],
        };
        layout.write_tx(&tx).unwrap();

        // First load: slow path (from_datoms builds indexes, writes store.bin).
        let store1 = layout.load_store().unwrap();
        assert!(
            root.join(".cache").join("store.bin").exists(),
            "store.bin should exist after first load"
        );

        // Second load: fast path (deserializes store.bin, skips from_datoms).
        let store2 = layout.load_store().unwrap();

        // Verify entity lookups work on the cached store.
        let e1_datoms = store1.entity_datoms(entity);
        let e2_datoms = store2.entity_datoms(entity);
        assert_eq!(
            e1_datoms.len(),
            e2_datoms.len(),
            "entity_datoms count must match between slow-path and cache-path stores"
        );
        assert!(
            !e1_datoms.is_empty(),
            "test entity should have datoms in the store"
        );

        // Verify LIVE view works on the cached store.
        let live1 = store1.live_value(
            entity,
            &braid_kernel::datom::Attribute::from_keyword(":db/doc"),
        );
        let live2 = store2.live_value(
            entity,
            &braid_kernel::datom::Attribute::from_keyword(":db/doc"),
        );
        assert_eq!(
            live1, live2,
            "LIVE view must match between slow-path and cache-path stores"
        );
    }
}
