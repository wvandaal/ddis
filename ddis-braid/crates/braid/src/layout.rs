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

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use braid_kernel::datom::{AgentId, Datom, TxId};
use braid_kernel::layout::{
    deserialize_tx, serialize_tx, ContentHash, IntegrityError, IntegrityReport, LayoutConfig,
    TxFile, TxFilePath,
};
use braid_kernel::Store;

use crate::error::BraidError;

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
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Idempotent: same content, same hash, same file.
            }
            Err(e) => return Err(BraidError::Io(e)),
        }

        Ok(file_path)
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

    /// Load the entire store from the layout (ψ function).
    ///
    /// This is `ψ(L) = ⋃ { tx.datoms | tx ∈ L.txns }`.
    /// Reconstructs the Store from all transaction files.
    pub fn load_store(&self) -> Result<Store, BraidError> {
        let hashes = self.list_tx_hashes()?;
        let mut all_datoms: BTreeSet<Datom> = BTreeSet::new();
        let mut frontier: HashMap<AgentId, TxId> = HashMap::new();

        for hash in &hashes {
            let tx = self.read_tx(hash)?;

            // Update frontier
            let agent = tx.agent;
            frontier
                .entry(agent)
                .and_modify(|existing| {
                    if tx.tx_id > *existing {
                        *existing = tx.tx_id;
                    }
                })
                .or_insert(tx.tx_id);

            // Collect datoms
            for datom in tx.datoms {
                all_datoms.insert(datom);
            }
        }

        Ok(Store::from_datoms(all_datoms))
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
}
