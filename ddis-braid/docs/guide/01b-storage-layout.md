# §1b. LAYOUT — Build Plan

> **Spec reference**: [spec/01b-storage-layout.md](../spec/01b-storage-layout.md)
> **Stage 0 elements**: INV-LAYOUT-001–011, ADR-LAYOUT-001–007, NEG-LAYOUT-001–005
> **Dependencies**: STORE (§1)
> **Cognitive mode**: Algebraic — structure-preserving maps, isomorphisms, functors

---

## §1b.1 Module Structure

```
crates/braid-kernel/src/
├── layout.rs        ← Canonical serialization, deserialization, hashing (pure functions)

crates/braid/src/
├── persistence.rs   ← Filesystem I/O: read/write transaction files, directory operations
```

### Key Design Decision

The Store-Layout Isomorphism (spec §1b.0) splits across two crate boundaries:

- **phi (Store -> Layout)**: `serialize_transaction` and `transaction_hash` are pure functions —
  they take a `Transaction<Applied>` and return bytes/hashes with no side effects. These live in
  `crates/braid-kernel/src/layout.rs` where they participate in the verification surface.

- **psi (Layout -> Store)**: `load_store` reads files from disk and reassembles the Store.
  This is I/O and lives in `crates/braid/src/persistence.rs`.

The split follows the workspace design invariant (guide/00-architecture.md §0.1): `braid-kernel`
is `#![forbid(unsafe_code)]`, no IO, no filesystem. `braid` is the thin IO wrapper.

### Public API Surface

```rust
// crates/braid-kernel/src/layout.rs — pure functions (INV-LAYOUT-011, INV-LAYOUT-001)
pub fn serialize_transaction(tx: &Transaction<Applied>) -> Vec<u8>;
pub fn deserialize_transaction(bytes: &[u8]) -> Result<Transaction<Applied>, LayoutError>;
pub fn transaction_hash(serialized: &[u8]) -> Blake3Hash;

// crates/braid/src/persistence.rs — I/O (INV-LAYOUT-003, INV-LAYOUT-007, INV-LAYOUT-009)
pub fn write_transaction(dir: &Path, tx: &Transaction<Applied>) -> Result<PathBuf, LayoutError>;
pub fn read_transaction(dir: &Path, hash: &Blake3Hash) -> Result<Transaction<Applied>, LayoutError>;
pub fn load_store(dir: &Path) -> Result<Store, LayoutError>;
pub fn init_layout(dir: &Path) -> Result<(), LayoutError>;
pub fn verify_integrity(dir: &Path) -> Result<IntegrityReport, LayoutError>;
pub fn merge_directories(src: &Path, dst: &Path) -> Result<MergeReceipt, LayoutError>;
pub fn rebuild_cache(dir: &Path) -> Result<(), LayoutError>;
```

### Error Types

```rust
/// Errors in the LAYOUT namespace (persistence layer).
pub enum LayoutError {
    /// Filesystem I/O failure (permissions, disk full, etc.).
    Io(std::io::Error),
    /// EDN deserialization failure (malformed transaction file).
    Deserialize(String),
    /// Content hash does not match filename — file is corrupt (INV-LAYOUT-001).
    HashMismatch { expected: Blake3Hash, actual: Blake3Hash, path: PathBuf },
    /// Layout directory does not exist or is missing required structure.
    NotInitialized(PathBuf),
}

/// Result from verify_integrity: lists any corrupt or unparseable transaction files.
pub struct IntegrityReport {
    pub total_files: usize,
    pub valid_files: usize,
    pub corrupt_files: Vec<(PathBuf, LayoutError)>,
}
```

See also the unified type catalog in guide/types.md for cross-namespace error type alignment.

---

## §1b.2 Three-Box Decomposition

### (1) Canonical Serializer

**Black box** (contract):
- `serialize_transaction` is a **total, deterministic, injective** function from
  `Transaction<Applied>` to `Vec<u8>` (INV-LAYOUT-011).
- Logically identical transactions produce byte-identical output.
- Round-trip: `deserialize_transaction(serialize_transaction(tx)) == tx` for all valid transactions.
- `transaction_hash(serialize_transaction(tx))` produces the content-addressed identity (INV-LAYOUT-001).
- No IO, no side effects, no filesystem access.

**State box** (internal design):
- EDN canonical form (ADR-LAYOUT-003) with fixed ordering guarantees:
  - Map keys: sorted lexicographically.
  - Datom vectors: sorted by `(entity, attribute, value, op)`.
  - Causal predecessor lists: sorted by HLC.
  - Whitespace: single space after key, newline after value in maps.
  - Encoding: UTF-8 NFC normalization, no trailing whitespace.
- Tagged literals for domain types: `#hlc` for HLC timestamps, `#blake3` for entity IDs.
- The serializer is the single source of determinism for the entire LAYOUT namespace.
  INV-LAYOUT-001 (content-addressed identity) depends on INV-LAYOUT-011 (canonical serialization).

**Clear box** (implementation):
```rust
// crates/braid-kernel/src/layout.rs

pub fn serialize_transaction(tx: &Transaction<Applied>) -> Vec<u8> {
    let mut buf = Vec::new();
    // 1. Sort datoms by (entity, attribute, value, op)
    let mut sorted_datoms = tx.datoms().to_vec();
    sorted_datoms.sort();
    // 2. Sort causal predecessors by HLC
    let mut sorted_predecessors = tx.causal_predecessors().to_vec();
    sorted_predecessors.sort();
    // 3. Write EDN with fixed key ordering: :tx/id, :tx/agent, :tx/provenance,
    //    :tx/causal-predecessors, :tx/rationale, :datoms
    write_edn_canonical(&mut buf, tx.tx_id(), tx.agent(), tx.provenance(),
                        &sorted_predecessors, tx.rationale(), &sorted_datoms);
    buf
}

pub fn deserialize_transaction(bytes: &[u8]) -> Result<Transaction<Applied>, LayoutError> {
    let parsed = parse_edn(bytes).map_err(|e| LayoutError::ParseError(e.to_string()))?;
    // Reconstruct Transaction<Applied> from parsed EDN
    Transaction::from_edn(parsed)
}

pub fn transaction_hash(serialized: &[u8]) -> Blake3Hash {
    Blake3Hash(blake3::hash(serialized).into())
}
```

**proptest strategy**:
```rust
proptest! {
    // INV-LAYOUT-011: Canonical serialization determinism
    fn canonical_determinism(tx in arb_applied_transaction()) {
        let bytes1 = serialize_transaction(&tx);
        let bytes2 = serialize_transaction(&tx);
        prop_assert_eq!(bytes1, bytes2);
    }

    // INV-LAYOUT-011: Round-trip identity
    fn canonical_roundtrip(tx in arb_applied_transaction()) {
        let bytes = serialize_transaction(&tx);
        let recovered = deserialize_transaction(&bytes).unwrap();
        prop_assert_eq!(tx, recovered);
        let rebytes = serialize_transaction(&recovered);
        prop_assert_eq!(bytes, rebytes);
    }

    // INV-LAYOUT-001 + INV-LAYOUT-011: Hash stability
    fn hash_stability(tx in arb_applied_transaction()) {
        let bytes = serialize_transaction(&tx);
        let h1 = transaction_hash(&bytes);
        let h2 = transaction_hash(&bytes);
        prop_assert_eq!(h1, h2);
    }
}
```

---

### (2) Content-Addressed Writer

**Black box** (contract):
- `write_transaction` writes a single transaction to the layout directory (INV-LAYOUT-001).
- The file is named by its BLAKE3 content hash, sharded under a 2-char hex prefix (INV-LAYOUT-008).
- Writing is atomic via `O_CREAT|O_EXCL` (INV-LAYOUT-010, ADR-LAYOUT-006).
- Writing the same content twice is idempotent — `AlreadyExists` is success, not error (INV-LAYOUT-010).
- Existing files are never modified (INV-LAYOUT-002, NEG-LAYOUT-001).
- Returns the path to the written file.

**State box** (internal design):
- Calls `serialize_transaction` (pure, from `braid-kernel`) to get canonical bytes.
- Calls `transaction_hash` to derive the BLAKE3 hash.
- Computes the sharded path: `txns/{hash[0..2]}/{full_hash}.edn`.
- Creates shard directory if needed (`fs::create_dir_all`).
- Opens file with `OpenOptions::new().write(true).create_new(true)` — this maps to
  `O_CREAT|O_EXCL` on POSIX, providing atomic create-if-not-exists.
- On `AlreadyExists`: returns success with the existing path (idempotent).
- On success: `file.write_all(bytes)` then `file.sync_all()` for durability.

**Clear box** (implementation):
```rust
// crates/braid/src/persistence.rs

pub fn write_transaction(dir: &Path, tx: &Transaction<Applied>) -> Result<PathBuf, LayoutError> {
    let bytes = serialize_transaction(tx);
    let hash = transaction_hash(&bytes);
    let hex = hex::encode(hash.as_bytes());
    let prefix = &hex[..2];
    let path = dir.join("txns").join(prefix).join(format!("{hex}.edn"));

    fs::create_dir_all(path.parent().unwrap())?;
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            // Idempotent: same content, same hash, same file. No-op.
        }
        Err(e) => return Err(LayoutError::IoError(e)),
    }
    Ok(path)
}

pub fn read_transaction(dir: &Path, hash: &Blake3Hash) -> Result<Transaction<Applied>, LayoutError> {
    let hex = hex::encode(hash.as_bytes());
    let prefix = &hex[..2];
    let path = dir.join("txns").join(prefix).join(format!("{hex}.edn"));
    let bytes = fs::read(&path)?;

    // Integrity check: verify hash matches contents (INV-LAYOUT-001)
    let actual = transaction_hash(&bytes);
    if actual != *hash {
        return Err(LayoutError::IntegrityError(IntegrityError::HashMismatch {
            expected: *hash,
            actual,
        }));
    }
    deserialize_transaction(&bytes)
}
```

**proptest strategy**:
```rust
proptest! {
    // INV-LAYOUT-001: Content-addressed identity
    fn content_addressed_identity(tx in arb_applied_transaction()) {
        let dir = tempdir().unwrap();
        let path = write_transaction(dir.path(), &tx).unwrap();
        let stem = path.file_stem().unwrap().to_str().unwrap();
        let bytes = fs::read(&path).unwrap();
        let hash_hex = hex::encode(blake3::hash(&bytes).as_bytes());
        prop_assert_eq!(stem, &hash_hex);
    }

    // INV-LAYOUT-002 + INV-LAYOUT-010: Idempotent write, immutability
    fn idempotent_write(tx in arb_applied_transaction()) {
        let dir = tempdir().unwrap();
        let path1 = write_transaction(dir.path(), &tx).unwrap();
        let bytes1 = fs::read(&path1).unwrap();
        let path2 = write_transaction(dir.path(), &tx).unwrap();
        let bytes2 = fs::read(&path2).unwrap();
        prop_assert_eq!(path1, path2);
        prop_assert_eq!(bytes1, bytes2);
    }

    // INV-LAYOUT-008: Sharded directory structure
    fn sharded_directory(tx in arb_applied_transaction()) {
        let dir = tempdir().unwrap();
        let path = write_transaction(dir.path(), &tx).unwrap();
        let parent_name = path.parent().unwrap().file_name().unwrap().to_str().unwrap();
        let stem = path.file_stem().unwrap().to_str().unwrap();
        prop_assert_eq!(parent_name, &stem[..2]);
    }
}
```

---

### (3) Directory-Union Merge

**Black box** (contract):
- `merge_directories(src, dst)` copies all transaction files from `src` to `dst` (INV-LAYOUT-004).
- Files already present in `dst` are skipped — content-addressed deduplication.
- Merge is commutative: `merge(A,B) ; merge(B,A)` produces the same file set as `merge(B,A) ; merge(A,B)`.
- Merge is associative: grouping is irrelevant to the final file set.
- Merge is idempotent: `merge(A,A)` changes nothing.
- Merge is monotonic: the file count in `dst` never decreases (INV-STORE-005, NEG-LAYOUT-002).
- No transport-specific logic — operates on `&Path` only (INV-LAYOUT-006, NEG-LAYOUT-004).
- No file-append operations (NEG-LAYOUT-003).
- Returns `MergeReceipt` with counts of new and deduplicated files.

**State box** (internal design):
- List all transaction hashes in `src` via directory scan.
- List all transaction hashes in `dst` via directory scan.
- Build a `HashSet` of `dst` hashes for O(1) dedup lookup.
- For each hash in `src` not in `dst`:
  - `read_transaction(src, hash)` to load the transaction.
  - `write_transaction(dst, tx)` to write it (content-addressed, idempotent).
- Tally `new_count` (files written) and `dup_count` (files skipped).

**Clear box** (implementation):
```rust
// crates/braid/src/persistence.rs

pub fn merge_directories(src: &Path, dst: &Path) -> Result<MergeReceipt, LayoutError> {
    let src_hashes = list_transaction_hashes(src)?;
    let dst_hashes: HashSet<Blake3Hash> = list_transaction_hashes(dst)?.into_iter().collect();

    let mut new_count = 0u64;
    let mut dup_count = 0u64;

    for hash in src_hashes {
        if dst_hashes.contains(&hash) {
            dup_count += 1;
            continue;
        }
        let tx = read_transaction(src, &hash)?;
        write_transaction(dst, &tx)?;
        new_count += 1;
    }

    Ok(MergeReceipt { new_count, dup_count })
}

fn list_transaction_hashes(dir: &Path) -> Result<Vec<Blake3Hash>, LayoutError> {
    let txns_dir = dir.join("txns");
    let mut hashes = Vec::new();
    for shard_entry in fs::read_dir(&txns_dir)? {
        let shard = shard_entry?.path();
        if !shard.is_dir() { continue; }
        for file_entry in fs::read_dir(&shard)? {
            let file = file_entry?.path();
            if file.extension().and_then(|e| e.to_str()) != Some("edn") { continue; }
            let hex_str = file.file_stem().unwrap().to_str().unwrap();
            let hash = Blake3Hash::from_hex(hex_str)
                .map_err(|_| LayoutError::ParseError(format!("Invalid hash filename: {hex_str}")))?;
            hashes.push(hash);
        }
    }
    Ok(hashes)
}
```

**proptest strategy**:
```rust
proptest! {
    // INV-LAYOUT-004: Merge commutativity
    fn merge_commutativity(
        txns_a in arb_transaction_set(5),
        txns_b in arb_transaction_set(5),
    ) {
        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();
        let dir_ab = tempdir().unwrap();
        let dir_ba = tempdir().unwrap();

        // Write initial state
        for tx in &txns_a { write_transaction(dir_a.path(), tx).unwrap(); }
        for tx in &txns_b { write_transaction(dir_b.path(), tx).unwrap(); }

        // Merge A into AB, then B into AB
        copy_layout(dir_a.path(), dir_ab.path());
        merge_directories(dir_b.path(), dir_ab.path()).unwrap();

        // Merge B into BA, then A into BA
        copy_layout(dir_b.path(), dir_ba.path());
        merge_directories(dir_a.path(), dir_ba.path()).unwrap();

        let hashes_ab = list_transaction_hashes(dir_ab.path()).unwrap();
        let hashes_ba = list_transaction_hashes(dir_ba.path()).unwrap();
        prop_assert_eq!(
            hashes_ab.into_iter().collect::<BTreeSet<_>>(),
            hashes_ba.into_iter().collect::<BTreeSet<_>>(),
        );
    }

    // INV-LAYOUT-004: Merge idempotency
    fn merge_idempotency(txns in arb_transaction_set(5)) {
        let dir = tempdir().unwrap();
        for tx in &txns { write_transaction(dir.path(), tx).unwrap(); }
        let before = list_transaction_hashes(dir.path()).unwrap();
        merge_directories(dir.path(), dir.path()).unwrap();
        let after = list_transaction_hashes(dir.path()).unwrap();
        prop_assert_eq!(before, after);
    }
}
```

---

### (4) Store Loader

**Black box** (contract):
- `load_store(dir)` reads all transaction files from `txns/` and reconstructs the in-memory
  `Store` (INV-LAYOUT-003).
- The resulting Store contains exactly the union of all datoms across all transaction files.
- No datom is lost, duplicated (beyond BTreeSet dedup), or fabricated.
- `load_store(dir)` after `write_transaction(dir, tx)` includes `tx`'s datoms in the result.
- This is `psi` in the isomorphism: `psi(phi(S)) = S`.

**State box** (internal design):
- Enumerate all transaction hashes via `list_transaction_hashes`.
- For each hash, call `read_transaction` (which verifies hash integrity per INV-LAYOUT-001).
- Accumulate all datoms into a `BTreeSet<Datom>`.
- Reconstruct frontier by scanning all transactions for the latest TxId per AgentId.
- Optionally load cached indexes from `.cache/` if present; rebuild if absent.
- Return `Store::from_datoms(datoms, frontier)`.

**Clear box** (implementation):
```rust
// crates/braid/src/persistence.rs

pub fn load_store(dir: &Path) -> Result<Store, LayoutError> {
    let hashes = list_transaction_hashes(dir)?;
    let mut datoms = BTreeSet::new();
    let mut frontier: Frontier = HashMap::new(); // Frontier = HashMap<AgentId, TxId> (see types.md)

    for hash in hashes {
        let tx = read_transaction(dir, &hash)?;
        for datom in tx.datoms() {
            datoms.insert(datom.clone());
        }
        // Update frontier: keep max TxId per agent
        let entry = frontier.entry(tx.agent().clone()).or_insert(tx.tx_id().clone());
        if tx.tx_id() > entry {
            *entry = tx.tx_id().clone();
        }
    }

    Ok(Store::from_datoms(datoms, frontier))
    // Note: Store::from_datoms internally rebuilds all four indexes (EAVT, AEVT, VAET, AVET)
    // by iterating the datom set. See guide/01-store.md §1.2 "Index Rebuild Strategy"
    // for the per-index insertion rules (which datoms go into which index).
}

/// Rebuild the .cache/ index files from txns/ alone (INV-LAYOUT-009).
/// This is the recovery path when .cache/ is missing, corrupt, or gitignored.
pub fn rebuild_cache(dir: &Path) -> Result<(), LayoutError> {
    let store = load_store(dir)?;
    // Serialize each index (EAVT, AEVT, VAET, AVET) to .cache/ files.
    // The serialized format is an implementation detail — these files are
    // derived artifacts, not authoritative. Deleting .cache/ and re-running
    // rebuild_cache produces identical results (INV-LAYOUT-009).
    write_cached_indexes(dir, store.indexes())?;
    Ok(())
}
```

**proptest strategy**:
```rust
proptest! {
    // INV-LAYOUT-003: Round-trip identity (phi ; psi = id)
    fn store_roundtrip(store in arb_store(5)) {
        let dir = tempdir().unwrap();
        init_layout(dir.path()).unwrap();
        // phi: store -> layout
        for tx in store.transactions() {
            write_transaction(dir.path(), tx).unwrap();
        }
        // psi: layout -> store
        let recovered = load_store(dir.path()).unwrap();
        prop_assert_eq!(store.datom_set(), recovered.datom_set());
    }

    // INV-LAYOUT-009: Index derivability
    fn index_derivability(store in arb_store(5)) {
        let dir = tempdir().unwrap();
        init_layout(dir.path()).unwrap();
        for tx in store.transactions() {
            write_transaction(dir.path(), tx).unwrap();
        }
        let store1 = load_store(dir.path()).unwrap();
        rebuild_cache(dir.path()).unwrap();
        let store2 = load_store(dir.path()).unwrap();
        prop_assert_eq!(store1.datom_set(), store2.datom_set());
    }

    // INV-LAYOUT-006: Transport independence
    fn transport_independence(store in arb_store(3)) {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        init_layout(dir1.path()).unwrap();
        for tx in store.transactions() {
            write_transaction(dir1.path(), tx).unwrap();
        }
        // Simulate transport: raw filesystem copy
        copy_dir_recursive(dir1.path().join("txns"), dir2.path().join("txns"));
        let loaded1 = load_store(dir1.path()).unwrap();
        let loaded2 = load_store(dir2.path()).unwrap();
        prop_assert_eq!(loaded1.datom_set(), loaded2.datom_set());
    }
}
```

---

### (5) Genesis File

**Black box** (contract):
- `init_layout(dir)` creates the `.braid/` directory structure and writes the genesis
  transaction (INV-LAYOUT-007, ADR-LAYOUT-007).
- Genesis content is a compile-time constant (`include_bytes!("genesis.edn")`).
- Genesis is written to both `.braid/genesis.edn` (discoverability) and
  `txns/{hash[0..2]}/{hash}.edn` (participation in standard operations).
- Both copies have identical content — verified by `verify_integrity`.
- Calling `init_layout` twice on the same directory is idempotent.
- The genesis transaction defines the 17 axiomatic meta-schema attributes (SR-008).

**State box** (internal design):
- Create directory tree: `txns/`, `heads/`, `.cache/`.
- Write `.gitignore` containing `.cache/\n`.
- Deserialize the compile-time genesis bytes into a `Transaction<Applied>`.
- Write `genesis.edn` at the well-known path.
- Write to `txns/` via `write_transaction` (which handles sharding and hashing).
- On second call: `genesis.edn` exists (overwrite is a no-op since content is identical),
  `write_transaction` gets `AlreadyExists` (idempotent).

**Clear box** (implementation):
```rust
// crates/braid/src/persistence.rs

const GENESIS_EDN: &[u8] = include_bytes!("genesis.edn");

pub fn init_layout(dir: &Path) -> Result<(), LayoutError> {
    let braid_dir = dir.join(".braid");
    fs::create_dir_all(braid_dir.join("txns"))?;
    fs::create_dir_all(braid_dir.join("heads"))?;
    fs::create_dir_all(braid_dir.join(".cache"))?;

    // Write genesis at well-known path
    fs::write(braid_dir.join("genesis.edn"), GENESIS_EDN)?;

    // Write genesis into txns/ under content hash
    let genesis_tx = deserialize_transaction(GENESIS_EDN)?;
    write_transaction(&braid_dir, &genesis_tx)?;

    // Write .gitignore
    fs::write(braid_dir.join(".gitignore"), ".cache/\n")?;

    Ok(())
}
```

**proptest strategy**:
```rust
proptest! {
    // INV-LAYOUT-007: Genesis determinism
    fn genesis_determinism() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        init_layout(dir1.path()).unwrap();
        init_layout(dir2.path()).unwrap();
        let g1 = fs::read(dir1.path().join(".braid/genesis.edn")).unwrap();
        let g2 = fs::read(dir2.path().join(".braid/genesis.edn")).unwrap();
        prop_assert_eq!(g1, g2);
    }

    // INV-LAYOUT-007: Genesis in txns/ matches genesis.edn
    fn genesis_dual_location() {
        let dir = tempdir().unwrap();
        init_layout(dir.path()).unwrap();
        let genesis_bytes = fs::read(dir.path().join(".braid/genesis.edn")).unwrap();
        let hash = transaction_hash(&genesis_bytes);
        let txn_bytes = fs::read(
            dir.path().join(".braid/txns")
                .join(&hex::encode(hash.as_bytes())[..2])
                .join(format!("{}.edn", hex::encode(hash.as_bytes())))
        ).unwrap();
        prop_assert_eq!(genesis_bytes, txn_bytes);
    }
}
```

---

## §1b.3 Type-Level Encoding

| INV | Compile-Time Guarantee | Mechanism |
|-----|----------------------|-----------|
| INV-LAYOUT-002 | No update/delete methods on Layout API | Type-level: only `write_transaction` (create) and `read_transaction` (read) exist |
| INV-LAYOUT-011 | Canonical serialization is a pure function in `braid-kernel` | Crate boundary: no IO in kernel, no domain logic in binary |
| NEG-LAYOUT-001 | No in-place file modification | `O_CREAT\|O_EXCL` via `create_new(true)` — cannot open existing files for writing |
| NEG-LAYOUT-002 | No file deletion from `txns/` | No `delete_tx`, `remove_tx`, or `compact` in the public API |
| NEG-LAYOUT-004 | No transport dependencies | `crates/braid-kernel/Cargo.toml` has zero transport-related crates |
| NEG-LAYOUT-005 | `.cache/` is not source of truth | `rebuild_cache` reads only from `txns/`; `.gitignore` excludes `.cache/` |

---

## §1b.4 LLM-Facing Outputs

### Agent-Mode Output — `braid init`

```
[LAYOUT] Initialized .braid/ at {path}. Genesis: {hash[..12]}... ({N} axiomatic attributes).
---
 {guidance_footer}
```

### Agent-Mode Output — `braid verify`

```
[LAYOUT] Verified {N} transaction files. {valid} valid, {corrupt} corrupt.
{if corrupt: list of corrupt files with error type}
---
 {guidance_footer}
```

### Agent-Mode Output — `braid merge --from {src}`

```
[LAYOUT] Merged {src} -> {dst}. {new_count} new transactions, {dup_count} deduplicated.
Store: {total} datoms across {tx_count} transactions.
---
 {guidance_footer}
```

### Error Messages

- **Layout not initialized**: `Layout error: .braid/ not found at {path} -- run "braid init" first -- See: INV-LAYOUT-007`
- **Corrupt transaction file**: `Layout error: BLAKE3 mismatch for {filename} -- expected {expected[..12]}..., got {actual[..12]}... -- run "braid verify" for full report -- See: INV-LAYOUT-001`
- **Parse error**: `Layout error: EDN parse failed for {filename} -- {parse_error} -- file may be corrupted -- See: INV-LAYOUT-011`

---

## §1b.5 Verification

### INV Coverage Matrix

| INV | Description | Crate | Module | Verification | proptest |
|-----|-------------|-------|--------|-------------|----------|
| INV-LAYOUT-011 | Canonical serialization determinism | braid-kernel | layout.rs | V:PROP, V:KANI | `canonical_determinism`, `canonical_roundtrip` |
| INV-LAYOUT-001 | Content-addressed file identity | braid | persistence.rs | V:PROP, V:KANI | `content_addressed_identity`, `hash_stability` |
| INV-LAYOUT-002 | Transaction file immutability | braid | persistence.rs | V:TYPE, V:PROP | `idempotent_write` |
| INV-LAYOUT-003 | Directory-store isomorphism | braid | persistence.rs | V:PROP, V:KANI | `store_roundtrip` |
| INV-LAYOUT-004 | Merge as directory union | braid | persistence.rs | V:PROP, V:KANI | `merge_commutativity`, `merge_idempotency` |
| INV-LAYOUT-005 | Integrity self-verification | braid | persistence.rs | V:PROP | `verify_detects_corruption` |
| INV-LAYOUT-006 | Transport independence | braid | persistence.rs | V:PROP | `transport_independence` |
| INV-LAYOUT-007 | Genesis file determinism | braid | persistence.rs | V:PROP, V:KANI | `genesis_determinism`, `genesis_dual_location` |
| INV-LAYOUT-008 | Sharded directory scalability | braid | persistence.rs | V:PROP | `sharded_directory` |
| INV-LAYOUT-009 | Index derivability | braid | persistence.rs | V:PROP | `index_derivability` |
| INV-LAYOUT-010 | Concurrent write safety | braid | persistence.rs | V:PROP, V:MODEL | `concurrent_writes` |

### Build Order

The dependency chain within the LAYOUT namespace determines build order:

```
INV-LAYOUT-011 (canonical serialization)        ← Build FIRST (prerequisite for everything)
    |
    v
INV-LAYOUT-001 (content-addressed identity)     ← Depends on 011 for deterministic hashing
    |
    v
INV-LAYOUT-002 (file immutability)              ← write_transaction uses 001 for naming
INV-LAYOUT-008 (sharded directory)              ← write_transaction uses 001 for path derivation
    |
    v
INV-LAYOUT-007 (genesis file)                   ← init_layout uses write_transaction
    |
    v
INV-LAYOUT-003 (directory-store isomorphism)    ← load_store reads what write_transaction wrote
INV-LAYOUT-005 (integrity verification)         ← verify_integrity re-hashes what write wrote
INV-LAYOUT-009 (index derivability)             ← rebuild_cache depends on load_store
    |
    v
INV-LAYOUT-004 (merge = directory union)        ← merge_directories uses read + write
INV-LAYOUT-006 (transport independence)         ← merge operates on generic Path
INV-LAYOUT-010 (concurrent write safety)        ← O_CREAT|O_EXCL, verified after write exists
```

### Crate Assignment

| Crate | Elements | Rationale |
|-------|----------|-----------|
| `braid-kernel` | INV-LAYOUT-011 (serialize/deserialize/hash) | Pure computation, no IO, verification surface |
| `braid` | INV-LAYOUT-001–010 (filesystem operations) | IO boundary: reads/writes files |

### Kani Harnesses

INV-LAYOUT-001, 003, 004, 007, 011 have V:KANI tags.

```rust
#[cfg(kani)]
mod kani_proofs {
    #[kani::proof]
    #[kani::unwind(4)]
    fn inv_layout_011_canonical() {
        let tx: Transaction<Applied> = kani::any();
        let b1 = serialize_transaction(&tx);
        let b2 = serialize_transaction(&tx);
        assert_eq!(b1, b2);
    }

    #[kani::proof]
    #[kani::unwind(4)]
    fn inv_layout_001_content_addressed() {
        let tx: Transaction<Applied> = kani::any();
        let bytes = serialize_transaction(&tx);
        let h1 = transaction_hash(&bytes);
        let h2 = transaction_hash(&bytes);
        assert_eq!(h1, h2);
    }

    #[kani::proof]
    #[kani::unwind(8)]
    fn inv_layout_004_merge_commutative() {
        // Bounded: 2 transactions per store
        let txns_a: Vec<Transaction<Applied>> = kani::any();
        let txns_b: Vec<Transaction<Applied>> = kani::any();
        // Verify: hash sets after merge A+B = hash sets after merge B+A
        let hashes_ab = merge_hash_sets(&txns_a, &txns_b);
        let hashes_ba = merge_hash_sets(&txns_b, &txns_a);
        assert_eq!(hashes_ab, hashes_ba);
    }
}
```

### Concurrency Verification

INV-LAYOUT-010 has a V:MODEL tag for stateright verification:

```rust
// Two agents writing interleaved transactions to the same layout.
// Verify: all reachable states satisfy INV-LAYOUT-001 (content-addressed),
// INV-LAYOUT-002 (immutability), and the layout's datom set equals
// the union of both agents' datom sets.

proptest! {
    fn concurrent_writes(
        txns_a in arb_transaction_set(3),
        txns_b in arb_transaction_set(3),
    ) {
        let dir = tempdir().unwrap();
        init_layout(dir.path()).unwrap();

        // Spawn two threads writing concurrently
        let dir_a = dir.path().to_path_buf();
        let dir_b = dir.path().to_path_buf();
        let handle_a = std::thread::spawn(move || {
            for tx in &txns_a { write_transaction(&dir_a, tx).unwrap(); }
        });
        let handle_b = std::thread::spawn(move || {
            for tx in &txns_b { write_transaction(&dir_b, tx).unwrap(); }
        });
        handle_a.join().unwrap();
        handle_b.join().unwrap();

        // Verify: all transactions present, no corruption
        let report = verify_integrity(dir.path()).unwrap();
        prop_assert_eq!(report.corrupt_files.len(), 0);

        let all_hashes: BTreeSet<_> = txns_a.iter().chain(txns_b.iter())
            .map(|tx| transaction_hash(&serialize_transaction(tx)))
            .collect();
        let loaded_hashes: BTreeSet<_> = list_transaction_hashes(dir.path()).unwrap()
            .into_iter().collect();
        // Loaded hashes is superset of expected (genesis may add one)
        for h in &all_hashes {
            prop_assert!(loaded_hashes.contains(h));
        }
    }
}
```

### Corruption Detection (INV-LAYOUT-005)

```rust
proptest! {
    fn verify_detects_corruption(
        txns in arb_transaction_set(10),
        corrupt_indices in proptest::collection::hash_set(0..10usize, 1..4),
    ) {
        let dir = tempdir().unwrap();
        init_layout(dir.path()).unwrap();
        let mut paths = Vec::new();
        for tx in &txns {
            paths.push(write_transaction(dir.path(), tx).unwrap());
        }

        // Corrupt selected files by flipping a byte
        for &idx in &corrupt_indices {
            if idx < paths.len() {
                let mut bytes = fs::read(&paths[idx]).unwrap();
                if !bytes.is_empty() {
                    bytes[0] ^= 0xFF;
                    fs::write(&paths[idx], &bytes).unwrap();
                }
            }
        }

        let report = verify_integrity(dir.path()).unwrap();
        // At least as many corrupt files detected as we corrupted
        // (may be fewer if byte flip happened to produce valid hash — astronomically unlikely)
        prop_assert!(report.corrupt_files.len() >= corrupt_indices.len().saturating_sub(1));
    }
}
```

---

## §1b.6 NEG Enforcement Summary

| NEG | Enforcement | Audit Check |
|-----|-------------|-------------|
| NEG-LAYOUT-001 (no in-place modification) | `write_transaction` uses `create_new(true)`; no `update_tx` exists | `grep -r "O_WRONLY\|O_RDWR" persistence.rs` returns 0 hits without `O_CREAT\|O_EXCL` |
| NEG-LAYOUT-002 (no file deletion) | No `delete_tx`, `remove_tx`, `compact` in API | `grep -r "remove_file\|remove_dir" persistence.rs` returns 0 hits on `txns/` paths |
| NEG-LAYOUT-003 (no merge via append) | `merge_directories` calls `write_transaction` (new files only) | `merge_directories` never opens existing files for writing |
| NEG-LAYOUT-004 (no transport deps) | `crates/braid-kernel/Cargo.toml` has no git/rsync/scp crates | `grep -r "git2\|libgit\|rsync" crates/braid-kernel/` returns 0 hits |
| NEG-LAYOUT-005 (no index as truth) | `rebuild_cache` reads only `txns/`; `.gitignore` excludes `.cache/` | `load_store` reads `txns/`, not `.cache/` |

---

## §1b.7 Implementation Checklist

- [ ] `serialize_transaction` produces deterministic EDN bytes (INV-LAYOUT-011)
- [ ] `deserialize_transaction` round-trips with `serialize_transaction` (INV-LAYOUT-011)
- [ ] `transaction_hash` returns BLAKE3 of serialized bytes (INV-LAYOUT-001)
- [ ] `write_transaction` names files by content hash, 2-char prefix sharding (INV-LAYOUT-001, INV-LAYOUT-008)
- [ ] `write_transaction` uses `O_CREAT|O_EXCL` for atomic create (INV-LAYOUT-010)
- [ ] `write_transaction` treats `AlreadyExists` as success (INV-LAYOUT-010)
- [ ] `read_transaction` verifies BLAKE3 hash on read (INV-LAYOUT-005)
- [ ] `init_layout` writes genesis to both `genesis.edn` and `txns/` (INV-LAYOUT-007)
- [ ] `init_layout` creates `.gitignore` excluding `.cache/` (INV-LAYOUT-009)
- [ ] `load_store` reconstructs Store from all `txns/` files (INV-LAYOUT-003)
- [ ] `load_store` rebuilds frontier from transaction metadata (INV-LAYOUT-003)
- [ ] `merge_directories` implements directory union via read+write (INV-LAYOUT-004)
- [ ] `merge_directories` deduplicates by content hash (INV-LAYOUT-004)
- [ ] `verify_integrity` detects hash mismatches and parse failures (INV-LAYOUT-005)
- [ ] `rebuild_cache` reproduces indexes from `txns/` alone (INV-LAYOUT-009)
- [ ] No method modifies or deletes files in `txns/` (NEG-LAYOUT-001, NEG-LAYOUT-002)
- [ ] No file-append in merge path (NEG-LAYOUT-003)
- [ ] No transport-specific dependencies in `braid-kernel` (NEG-LAYOUT-004)
- [ ] `cargo check --all-targets` passes (Gate 1)
- [ ] All proptest properties pass (Gate 2)
- [ ] All Kani harnesses pass (Gate 3)
- [ ] Integration: `init` -> `write_transaction` * N -> `load_store` -> verify datom set equality

---
