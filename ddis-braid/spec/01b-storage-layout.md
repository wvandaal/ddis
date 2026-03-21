> **DEPRECATED**: This file is bootstrap scaffolding. The canonical source of truth is the braid datom store. Use `braid spec show` and `braid query` to access spec elements. See ADR-STORE-019.

---

> **Section**: §1b. Storage Layout | **Namespace**: LAYOUT | **Wave**: 1 (Foundation)
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)
> **Depends on**: [01-store.md](01-store.md) (STORE namespace — algebraic datom store)
> **Supersedes**: ADR-STORE-007 Options A (trunk.ednl) and B (redb target)

## §1b. Storage Layout — Content-Addressed Transaction Files

> **Purpose**: Specifies the physical on-disk layout for the Braid datom store. The layout
> is a *faithful functor* from the algebraic store `(P(D), ∪)` to the filesystem
> `(P(F), ∪_dir)` — every STORE invariant has a corresponding LAYOUT invariant that
> is its image under the structure-preserving map φ.
>
> **Motivation**: The G-Set CvRDT axioms (INV-STORE-003 through INV-STORE-007) require
> that merge is set union. A single append-only file (trunk.ednl, per ADR-STORE-007 Option A)
> creates git merge conflicts on concurrent agent writes — the physical format does not
> reflect the algebraic structure. Per-transaction content-addressed files make the
> filesystem isomorphic to the algebra: merge *is* directory union, verification *is*
> `blake3sum`, and concurrent writes produce different files by construction.
>
> **Key insight**: This is not a storage optimization. It is a *structure-preserving map*
> that makes the seven CRDT axioms into filesystem tautologies.

---

### §1b.0 The Store-Layout Isomorphism

**THEOREM (Store-Layout Isomorphism)**:

```
Let S = (P(D), ∪) be the datom store (G-Set CvRDT, INV-STORE-003–007).
Let L = (P(F), ∪_dir) be a layout where F = set of transaction files
  and ∪_dir = directory union (copy files, dedup by name).

Define φ : S → L by φ(S) = { serialize(tx) | tx ∈ transactions(S) }
Define ψ : L → S by ψ(L) = ⋃ { deserialize(f).datoms | f ∈ L }

Then:
  (1) ψ(φ(S)) = S                              — round-trip identity
  (2) φ(MERGE(S₁, S₂)) = φ(S₁) ∪_dir φ(S₂)    — merge is functorial
  (3) BLAKE3(f) = BLAKE3(g) ⟹ f = g            — identity preservation

The layout is a faithful functor from (Store, MERGE) to (Directory, ∪_dir).
```

This theorem is the formal foundation for every element in this namespace. Each invariant,
ADR, and negative case derives from properties of φ, ψ, and the commutativity of the
diagram `MERGE ; φ = (φ × φ) ; ∪_dir`.

**Corollary (CRDT axioms as filesystem properties)**:
- **L1 (commutativity)**: `∪_dir` is commutative (directory order irrelevant)
- **L2 (associativity)**: `∪_dir` is associative (copy order irrelevant)
- **L3 (idempotency)**: `∪_dir` is idempotent (copying same file twice = one file)
- **L4 (least element)**: Empty directory is identity for `∪_dir`
- **L5 (monotonicity)**: `∪_dir` never removes files (directory only grows)
- **Content identity**: BLAKE3 naming deduplicates identical transactions
- **Concurrent safety**: Different transactions → different files → no conflict

---

### §1b.1 Directory Layout

```
.braid/
├── txns/                          ← Transaction files (content-addressed)
│   ├── {hash[0..2]}/              ← 2-char hex prefix (256-way sharding)
│   │   └── {full_blake3_hex}.edn  ← One transaction per file, immutable after creation
│   └── ...
├── heads/                         ← Agent frontier pointers (performance caches)
│   └── {agent_id}.ref             ← Text file: latest TxId hex for this agent
├── genesis.edn                    ← Genesis transaction (compile-time constant)
├── .cache/                        ← Derived indexes (gitignored, rebuilt from txns/)
│   ├── eavt.idx
│   ├── aevt.idx
│   ├── vaet.idx
│   ├── avet.idx
│   └── live.idx
└── .gitignore                     ← Ignores .cache/
```

**Transaction file format** (EDN, one file per transaction):
```clojure
;; txns/a1/a1b2c3d4e5f6789012345678...abcdef01.edn
;; Invariant: BLAKE3(these bytes) = a1b2c3d4e5f6789012345678...abcdef01
{:tx/id    #hlc "2026-03-05T14:22:01.000Z/agent-1/42"
 :tx/agent "agent-1"
 :tx/provenance :observed
 :tx/causal-predecessors [#hlc "2026-03-05T14:21:58.000Z/agent-1/41"]
 :tx/rationale "Assert spec element INV-STORE-001"
 :datoms [
   {:e #blake3 "a1b2c3..." :a :spec/type       :v :invariant       :op :assert}
   {:e #blake3 "a1b2c3..." :a :spec/id         :v "INV-STORE-001"  :op :assert}
   {:e #blake3 "a1b2c3..." :a :spec/statement  :v "Append-only immutability" :op :assert}
   {:e #blake3 "a1b2c3..." :a :spec/traces-to  :v "SEED §4"        :op :assert}
 ]}
```

**`heads/*.ref` files are caches, not truth sources.** They are derivable from scanning
`txns/` for the latest TxId per agent. `verify_integrity` can rebuild them. This avoids
tension with ADR-STORE-019 (all durable information as datoms) — heads are performance
shortcuts, not authoritative state.

**Access logging is not a layout concern.** Each transaction carries full provenance
(agent, HLC timestamp, causal predecessors, rationale). Access logging belongs to the
QUERY or GUIDANCE namespace at the interface layer.

---

## Invariants

### INV-LAYOUT-001: Content-Addressed File Identity

**Traces to**: SEED §4 Axiom 1, C2, ADRS FD-007, FD-013
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ transaction files f in the layout:
  filename(f) = hex(BLAKE3(bytes(f)))

Corollary (idempotent overwrite):
  write(f₁) ; write(f₂) where bytes(f₁) = bytes(f₂)
  ⟹ filename(f₁) = filename(f₂)
  ⟹ the directory contains one file (filesystem dedup by name)

This is the physical realization of INV-STORE-003 (content-addressable identity).
At the store level, two identical datoms produce one entry in the BTreeSet.
At the layout level, two identical transaction files produce one file in the directory.
The isomorphism preserves the identity axiom.
```

#### Level 1 (State Invariant)
For all reachable layout states L:
  No file in `txns/` has a name that differs from the BLAKE3 hash of its contents.
  A file whose name ≠ BLAKE3(contents) is corrupt and detected by `verify_integrity`.

#### Level 2 (Implementation Contract)
```rust
impl Layout {
    pub fn write_tx(&self, tx: &TxFile) -> Result<PathBuf, LayoutError> {
        let bytes = canonical_edn(tx);
        let hash = blake3::hash(&bytes);
        let hex = hex::encode(hash.as_bytes());
        let prefix = &hex[..2];
        let path = self.root.join("txns").join(prefix).join(format!("{hex}.edn"));
        // O_CREAT | O_EXCL: atomic create, fails if exists (= same content, idempotent)
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
}
```

**Falsification**: A file in `txns/` whose `BLAKE3(read_bytes(file))` does not equal
the filename (minus the `.edn` extension and directory prefix).

**proptest strategy**: Generate 10,000 random transactions, write each, read back,
verify `BLAKE3(read_bytes) == filename_stem` for every file.

---

### INV-LAYOUT-002: Transaction File Immutability

**Traces to**: SEED §4 Axiom 2, C1, ADRS FD-001
**Verification**: `V:TYPE`, `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ f ∈ L.txns, ∀ t₁ < t₂:
  contents(f, t₁) = contents(f, t₂)

Files in txns/ are write-once. After creation, their contents never change.
This is the physical realization of INV-STORE-001 (append-only immutability).
At the store level, existing datoms are never mutated.
At the layout level, existing files are never modified.
```

#### Level 1 (State Invariant)
For all state transitions L → L':
  For every file f present in L.txns, the bytes of f in L' are identical to the bytes of f in L.
  The only valid transition that adds to txns/ is creating a new file (write_tx).
  No transition modifies or removes an existing file.

#### Level 2 (Implementation Contract)
```rust
impl Layout {
    // write_tx uses O_CREAT | O_EXCL — cannot overwrite existing files.
    // No method exists for modifying or deleting transaction files.
    // The Layout API exposes: write_tx (create), read_tx (read), list_txns (enumerate).
    // There is no update_tx, delete_tx, or truncate.

    // Type-level enforcement: Layout holds a PathBuf (read-only handle).
    // The only mutation point is write_tx, which uses create_new(true).
}
```

**Falsification**: Any operation that changes the byte contents of an existing file in `txns/`,
or any API that enables such modification.

**proptest strategy**: Write N transactions, snapshot file hashes, write N more, verify
all original file hashes are unchanged.

---

### INV-LAYOUT-003: Directory-Store Isomorphism

**Traces to**: SEED §4, C4, ADRS AS-001, FD-001, FD-007
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Define:
  φ : Store → Layout    by  φ(S) = { serialize(tx) | tx ∈ transactions(S) }
  ψ : Layout → Store    by  ψ(L) = ⋃ { deserialize(f).datoms | f ∈ L.txns }

Round-trip identity:
  ψ(φ(S)) = S   for all stores S
  φ(ψ(L)) = L   for all well-formed layouts L

The pair (φ, ψ) is an isomorphism between the algebraic store and the physical layout.
Every STORE invariant (INV-STORE-001–014) holds in the layout iff it holds in the store.
```

#### Level 1 (State Invariant)
For all reachable layout states L:
  `load_all(L).datoms` = the mathematical set union of all datoms across all transaction
  files in `L.txns/`. No datom is lost, duplicated (beyond dedup), or fabricated during loading.

#### Level 2 (Implementation Contract)
```rust
impl Layout {
    pub fn load_all(&self) -> Result<Store, LayoutError> {
        let mut datoms = BTreeSet::new();
        for hash in self.list_txns()? {
            let tx = self.read_tx(&hash)?;
            for datom in tx.datoms {
                datoms.insert(datom);
            }
        }
        Ok(Store::from_datoms(datoms))
    }
}
```

**Falsification**: A datom present in some transaction file in `txns/` that is absent from
`load_all().datoms`, or a datom in `load_all().datoms` not traceable to any transaction file.

**proptest strategy**: Generate random store, serialize via φ, deserialize via ψ, compare
datom sets for equality. Run 1,000 iterations with stores of 1–1000 datoms.

---

### INV-LAYOUT-004: Merge as Directory Union

**Traces to**: SEED §4 Axiom 2, C4, ADRS AS-001
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ layouts L₁, L₂:
  merge_layouts(L₁, L₂).txns = L₁.txns ∪ L₂.txns

Merge is directory union: copy all files from source to target. Files with identical
names (= identical content, by INV-LAYOUT-001) are deduplicated by the filesystem.

This is the physical realization of INV-MERGE-001 (merge preserves datoms).
The isomorphism commutes with merge:
  φ(MERGE(S₁, S₂)) = φ(S₁) ∪_dir φ(S₂)

Merge inherits all G-Set CvRDT properties from directory union:
  L1: L₁ ∪ L₂ = L₂ ∪ L₁                    (commutativity — copy order irrelevant)
  L2: (L₁ ∪ L₂) ∪ L₃ = L₁ ∪ (L₂ ∪ L₃)    (associativity — grouping irrelevant)
  L3: L ∪ L = L                              (idempotency — duplicate copy is no-op)
  L5: |L₁ ∪ L₂| ≥ max(|L₁|, |L₂|)          (monotonicity — merge never shrinks)
```

#### Level 1 (State Invariant)
For all merge operations merge_layouts(target, source) → target':
  - Every file in source.txns/ exists in target'.txns/
  - Every file in target.txns/ exists in target'.txns/
  - No file in target'.txns/ is absent from both source.txns/ and target.txns/

#### Level 2 (Implementation Contract)
```rust
pub fn merge_layouts(target: &Layout, source: &Layout) -> Result<MergeReceipt, LayoutError> {
    let source_txns = source.list_txns()?;
    let target_txns = target.list_txns()?;
    let target_set: HashSet<[u8; 32]> = target_txns.into_iter().collect();

    let mut new_count = 0u64;
    let mut dup_count = 0u64;

    for hash in source_txns {
        if target_set.contains(&hash) {
            dup_count += 1;
            continue; // Already present — content-addressed dedup
        }
        let tx = source.read_tx(&hash)?;
        target.write_tx(&tx)?;
        new_count += 1;
    }

    Ok(MergeReceipt { new_count, dup_count })
}
```

**Falsification**: A merge operation that (a) loses a file present in either source or target,
(b) produces a file not present in either source or target, or (c) produces a different result
depending on the order of source/target arguments.

**proptest strategy**: Generate two random layouts, merge both directions, verify resulting
file sets are identical (commutativity). Merge three layouts in different associations,
verify results are identical (associativity).

---

### INV-LAYOUT-005: Integrity Self-Verification

**Traces to**: C7 (self-bootstrap), ADRS FD-006
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
verify(L) = ∀ f ∈ L.txns: BLAKE3(contents(f)) = filename(f)

Integrity verification is a structural tautology: if every file is named by its hash,
and the hash function is correct, then any corruption is detectable by re-hashing.

No external trust anchor is required. The layout is self-verifying.
```

#### Level 1 (State Invariant)
For all reachable layout states L:
  `verify_integrity(L)` completes in O(total_bytes) time and reports every corrupt file —
  a file whose BLAKE3 hash does not match its filename.

#### Level 2 (Implementation Contract)
```rust
impl Layout {
    pub fn verify_integrity(&self) -> Result<IntegrityReport, LayoutError> {
        let mut report = IntegrityReport::default();
        for hash in self.list_txns()? {
            report.total_files += 1;
            let path = self.tx_path(&hash);
            let bytes = fs::read(&path)?;
            let actual = blake3::hash(&bytes);
            if actual.as_bytes() != &hash {
                report.corrupt_files.push((path, IntegrityError::HashMismatch {
                    expected: hash,
                    actual: *actual.as_bytes(),
                }));
            } else {
                // Also verify EDN parse succeeds
                match parse_edn::<TxFile>(&bytes) {
                    Ok(_) => report.valid_files += 1,
                    Err(e) => report.corrupt_files.push((path, IntegrityError::ParseError(e.to_string()))),
                }
            }
        }
        Ok(report)
    }
}
```

**Falsification**: A corrupt file (hash mismatch or parse failure) that `verify_integrity`
does not report, or a valid file that it incorrectly reports as corrupt.

**proptest strategy**: Write N valid transactions. Corrupt K random files by flipping bytes.
Verify `verify_integrity` reports exactly K corrupt files with correct error details.

---

### INV-LAYOUT-006: Transport Independence

**Traces to**: SEED §4 Axiom 2, C4, ADRS AS-001
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ transport mechanisms T₁, T₂ (git clone, rsync, cp -r, tar, scp, ...):
  ψ(T₁(φ(S))) = ψ(T₂(φ(S))) = S

The store recovered from a layout is independent of how the files were copied.
Any file-preserving transport produces the same datom set.

This follows from: (a) files are immutable (INV-LAYOUT-002), (b) file identity is
by content hash (INV-LAYOUT-001), (c) directory union is the merge operation
(INV-LAYOUT-004). The transport only needs to preserve file contents and names.
```

#### Level 1 (State Invariant)
For all layouts L and transport mechanisms T:
  If T preserves file names and contents, then `load_all(T(L))` = `load_all(L)`.

#### Level 2 (Implementation Contract)
```rust
// No transport-specific code in Layout. The API operates on PathBuf.
// merge_layouts reads files from source and writes to target via write_tx.
// The source could be a local directory, a git clone, an NFS mount, etc.
// Layout does not import, depend on, or reference any transport library.
```

**Falsification**: A merge result that differs depending on whether the source layout
was obtained via git clone, rsync, or direct filesystem copy.

**proptest strategy**: Create a layout, copy it via three methods (filesystem copy,
tar/untar, symlink), load_all each copy, verify all three produce identical datom sets.

---

### INV-LAYOUT-007: Genesis File Determinism

**Traces to**: SEED §4, ADRS FD-006, SR-008
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
genesis.edn is a compile-time constant.

∀ Layout instances L₁, L₂:
  contents(L₁.genesis.edn) = contents(L₂.genesis.edn)
  BLAKE3(L₁.genesis.edn) = BLAKE3(L₂.genesis.edn)

The genesis transaction defines the 19 axiomatic meta-schema attributes (SR-008).
Its content is fixed at compile time — every Braid instance starts from the same seed.

This is the physical realization of INV-STORE-010 (genesis determinism, INV-SCHEMA-001).
At the store level, genesis is the first transaction. At the layout level, genesis.edn is
a constant file that is also present in txns/ under its content hash.
```

#### Level 1 (State Invariant)
For all reachable layout states L:
  `genesis.edn` exists at `.braid/genesis.edn` and its content equals the compile-time constant.
  `txns/{hash[0..2]}/{hash}.edn` also exists where hash = BLAKE3(genesis content).
  Both files have identical content.

#### Level 2 (Implementation Contract)
```rust
impl Layout {
    pub fn init(root: impl AsRef<Path>) -> Result<Self, LayoutError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("txns"))?;
        fs::create_dir_all(root.join("heads"))?;
        fs::create_dir_all(root.join(".cache"))?;

        let genesis_bytes = include_bytes!("genesis.edn");
        fs::write(root.join("genesis.edn"), genesis_bytes)?;

        // Also write to txns/ under content hash
        let layout = Layout { root };
        let genesis_tx = parse_edn::<TxFile>(genesis_bytes)?;
        layout.write_tx(&genesis_tx)?;

        // Write .gitignore
        fs::write(layout.root.join(".gitignore"), ".cache/\n")?;

        Ok(layout)
    }
}
```

**Falsification**: Two Layout::init() calls on different machines producing different
`genesis.edn` content, or a genesis.edn whose content does not match the compile-time constant.

**proptest strategy**: Call Layout::init() 100 times in different temp directories,
verify all produce byte-identical genesis.edn files.

---

### INV-LAYOUT-008: Sharded Directory Scalability

**Traces to**: ADRS SR-006
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Transaction files are organized under 256-way hash-prefix sharding:
  txns/{hash[0..2]}/{full_hash}.edn

The first two hex characters of the BLAKE3 hash form a directory prefix.
This creates up to 256 subdirectories, each containing ~N/256 files for N total
transactions. The sharding is deterministic (derived from the hash) and
adds no information beyond what the hash already encodes.

This mirrors the git objects storage pattern (objects/{xx}/{remaining_hash}).
```

#### Level 1 (State Invariant)
For all files f in txns/:
  f is located at `txns/{hash(f)[0..2]}/{hash(f)}.edn` where hash(f) is the full
  BLAKE3 hex of f's contents. No file exists at an incorrect prefix path.

#### Level 2 (Implementation Contract)
```rust
impl Layout {
    fn tx_path(&self, hash: &[u8; 32]) -> PathBuf {
        let hex = hex::encode(hash);
        let prefix = &hex[..2];
        self.root.join("txns").join(prefix).join(format!("{hex}.edn"))
    }
}
```

**Falsification**: A transaction file located at a prefix that does not match the first
two hex characters of its content hash.

**proptest strategy**: Write 10,000 transactions, verify every file's parent directory
name equals the first two characters of the file's stem.

---

### INV-LAYOUT-009: Index Derivability

**Traces to**: C7 (self-bootstrap), ADRS FD-006
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ layout states L:
  .cache/ = derive_indexes(L.txns)

The .cache/ directory is a pure function of the transaction files. It contains
no information that cannot be recomputed from txns/. Deleting .cache/ and
calling rebuild_cache() reproduces identical indexes.

Formally: the indexes are a projection — a surjective function from the
transaction set to a more-efficient representation. The projection is
idempotent: derive(derive(txns)) = derive(txns).
```

#### Level 1 (State Invariant)
For all reachable layout states L:
  Deleting `.cache/` and calling `rebuild_cache()` produces a layout L' where
  `load_all(L)` = `load_all(L')` and all query operations return identical results.

#### Level 2 (Implementation Contract)
```rust
impl Layout {
    pub fn rebuild_cache(&self) -> Result<(), LayoutError> {
        // Delete existing cache
        let cache_dir = self.root.join(".cache");
        if cache_dir.exists() {
            fs::remove_dir_all(&cache_dir)?;
        }
        fs::create_dir_all(&cache_dir)?;

        // Rebuild from txns/
        let store = self.load_all()?;
        // Write EAVT, AEVT, VAET, AVET, LIVE indexes to .cache/
        store.write_indexes(&cache_dir)?;
        Ok(())
    }
}
```

**Falsification**: A query result that differs between a layout with a warm cache and the
same layout after `rebuild_cache()`, given the same txns/ contents.

**proptest strategy**: Build a layout with 1,000 transactions, run 100 random queries,
delete .cache/, rebuild, run the same 100 queries, verify identical results.

---

### INV-LAYOUT-010: Concurrent Write Safety

**Traces to**: ADRS SR-007 (supersedes flock coordination)
**Verification**: `V:PROP`, `V:MODEL`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ concurrent write_tx operations W₁, W₂:
  Case 1: tx(W₁) ≠ tx(W₂)
    ⟹ hash(W₁) ≠ hash(W₂)
    ⟹ filename(W₁) ≠ filename(W₂)
    ⟹ W₁ and W₂ write to different files
    ⟹ no filesystem conflict

  Case 2: tx(W₁) = tx(W₂)
    ⟹ hash(W₁) = hash(W₂)
    ⟹ filename(W₁) = filename(W₂)
    ⟹ O_CREAT|O_EXCL: one succeeds, one gets AlreadyExists
    ⟹ AlreadyExists is not an error (same content already present)
    ⟹ no data loss, no corruption

Concurrent writes are safe by construction. No locking, no coordination,
no flock(). The content-addressed naming scheme structurally eliminates
contention. This supersedes SR-007 (flock-based coordination).
```

#### Level 1 (State Invariant)
For all concurrent write_tx executions:
  The resulting layout is identical to some sequential execution of the same writes.
  (Linearizability follows from atomic file creation via O_CREAT|O_EXCL.)

#### Level 2 (Implementation Contract)
```rust
// write_tx (INV-LAYOUT-001 Level 2) uses:
//   OpenOptions::new().write(true).create_new(true).open(&path)
//
// create_new(true) maps to O_CREAT | O_EXCL on POSIX:
//   - Atomically checks existence and creates
//   - If file exists, returns ErrorKind::AlreadyExists
//   - Race-free: two concurrent create_new on same path → exactly one succeeds
//
// AlreadyExists is handled as idempotent success (same content → same hash → same file).
```

**Falsification**: Two concurrent write_tx operations that produce data loss, corruption,
or a state that could not result from any sequential execution of the same writes.

**stateright model**: Two agents writing interleaved transactions to the same layout.
Verify: all reachable states satisfy INV-LAYOUT-001 (content-addressed), INV-LAYOUT-002
(immutability), and the layout's datom set equals the union of both agents' datom sets.

---

### INV-LAYOUT-011: Canonical Serialization Determinism

**Traces to**: SEED §4 Axiom 1, C2, ADRS FD-007
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ transactions tx:
  canonical_edn(tx) is a total function producing unique byte sequences per logical tx.

Formally: canonical_edn is injective on logical transaction identity.
  tx₁ ≡_logical tx₂  ⟹  canonical_edn(tx₁) = canonical_edn(tx₂)

This is a PREREQUISITE for INV-LAYOUT-001. If serialization is non-deterministic,
two agents serializing the same logical transaction produce different bytes,
different BLAKE3 hashes, different filenames, and the G-Set deduplication
breaks. The isomorphism theorem's identity preservation (axiom 3) depends
on canonical serialization.
```

#### Level 1 (State Invariant)
EDN canonical form guarantees:
  - Map keys sorted lexicographically
  - No trailing whitespace
  - UTF-8 NFC normalization
  - Consistent spacing (single space after key, newline after value in maps)
  - Datom vectors sorted by (entity, attribute, value, op)
  - Causal predecessor lists sorted by HLC

#### Level 2 (Implementation Contract)
```rust
pub fn canonical_edn(tx: &TxFile) -> Vec<u8> {
    let mut buf = Vec::new();
    // Fixed-order serialization: tx metadata fields in declared order,
    // datoms sorted by (e, a, v, op), predecessors sorted by HLC.
    let mut sorted_datoms = tx.datoms.clone();
    sorted_datoms.sort();
    let normalized = TxFile {
        tx_meta: tx.tx_meta.clone(),
        datoms: sorted_datoms,
    };
    write_edn(&normalized, &mut buf);
    buf
}

// Round-trip property:
//   parse_edn(canonical_edn(tx)) = tx   for all valid transactions
//   canonical_edn(parse_edn(bytes)) = bytes   for all canonical bytes
```

**Falsification**: Two calls to `canonical_edn` with logically-identical transactions
producing different byte sequences, or a byte sequence that round-trips to different bytes.

**proptest strategy**: Generate 10,000 random transactions, serialize each twice,
verify byte-identical output. Parse and re-serialize, verify byte-identical.

---

## Architectural Decision Records

### ADR-LAYOUT-001: Per-Transaction Files Over Single Append Log

**Traces to**: ADRS SR-006
**Stage**: 0
**Supersedes**: ADR-STORE-007 Option A (trunk.ednl)

#### Problem
What is the unit of physical storage for transactions?

#### Options
A) **Single append-only file** (`trunk.ednl`) — all transactions in one file, one line per datom.
   Simple, grepable, but concurrent appends cause git merge conflicts because git merges lines,
   not logical records. Agents must coordinate appends via flock or similar.

B) **Per-transaction files** — one file per transaction, named by content hash. Concurrent
   writes produce different files. Merge = directory union. No coordination needed.

C) **Log-structured merge tree** — write-ahead log with periodic compaction. High throughput
   but introduces compaction complexity, GC, and a non-trivial merge story.

D) **Embedded database** (redb/SQLite) — proven technology, but introduces a binary format
   that is not git-mergeable, requires tooling for inspection, and moves the merge operation
   from a filesystem tautology to a database-specific protocol.

#### Decision
**Option B.** Per-transaction content-addressed files. The filesystem structure mirrors the
algebraic structure: the G-Set is a set of files, set union is directory union. This makes
the CRDT axioms (L1–L5) into filesystem tautologies that require no implementation effort.

#### Formal Justification
The Store-Layout Isomorphism Theorem (§1b.0) proves that per-transaction files are a
*faithful functor* from (Store, MERGE) to (Directory, ∪_dir). This is the strongest possible
relationship between the algebra and the physical layout — every algebraic property is
automatically preserved by the filesystem.

Option A breaks this isomorphism: appending to a shared file introduces ordering dependencies
(which append goes first?) that have no counterpart in the unordered G-Set algebra.
Option D introduces an opaque binary layer that obscures the algebraic structure.

#### Consequences
- ADR-STORE-007 Option A (trunk.ednl) is superseded
- SR-007 (flock coordination) is superseded — concurrent writes are structurally safe
- Merge can use any file-copying mechanism (git, rsync, cp, scp)
- Files are individually inspectable with `cat` and `blake3sum`
- Git history shows per-transaction granularity

---

### ADR-LAYOUT-002: Content-Addressed Naming Over Sequential Naming

**Traces to**: ADRS FD-007, C2
**Stage**: 0

#### Problem
How are transaction files named?

#### Options
A) **Sequential numbering** (`000001.edn`, `000002.edn`) — simple, human-readable, but
   requires a global counter, creating a coordination bottleneck for concurrent agents.

B) **UUID naming** (`{uuid}.edn`) — no coordination needed, but UUIDs carry no semantic
   content. Two identical transactions get different names, breaking deduplication.

C) **Content-addressed naming** (`{BLAKE3(contents)}.edn`) — the filename IS the content
   hash. Two identical transactions get the same name. No coordination needed.
   Deduplication is automatic. Integrity is self-verifiable.

#### Decision
**Option C.** Content-addressed naming. The filename encodes the hash of the file's contents,
making identity, deduplication, and integrity verification structural properties of the naming
scheme rather than runtime operations.

#### Formal Justification
Content-addressed naming is the *unique* naming scheme that preserves C2 (identity by content)
at the filesystem level. Sequential naming (A) requires coordination, violating concurrent
write safety. UUID naming (B) breaks deduplication, violating the idempotency property of
the G-Set. Only content-addressed naming satisfies both requirements simultaneously.

#### Consequences
- File identity is by content, not by creation order or random assignment
- Deduplication is free (same content → same name → one file)
- Integrity verification is `blake3sum` — a standard tool, no custom tooling needed
- File names are 64-character hex strings, not human-memorable

---

### ADR-LAYOUT-003: EDN Serialization Format

**Traces to**: ADRS SR-006
**Stage**: 0

#### Problem
What serialization format for transaction files?

#### Options
A) **EDN (Extensible Data Notation)** — Clojure-native, Datomic-aligned, human-readable.
   Supports tagged literals for domain types (#hlc, #blake3). Compact for small documents.

B) **JSON** — ubiquitous, extensive tooling, but lacks tagged literals, sets, and keywords.
   Would require encoding conventions for domain types.

C) **CBOR/MessagePack** — compact binary, schema-optional, but not human-readable.
   Inspection requires tooling. Git diffs are meaningless.

D) **Protobuf/FlatBuffers** — schema-required, compact, fast, but schema evolution is
   constrained by wire format compatibility rules. Not human-readable.

#### Decision
**Option A.** EDN. The Datomic heritage of the datom model is best served by its native
serialization format. Tagged literals (#hlc for HLC timestamps, #blake3 for entity IDs)
provide type-safe extensibility without encoding conventions.

#### Formal Justification
EDN's tagged literals provide a clean encoding for the domain types that other formats
require convention-based encoding for:
- `#hlc "timestamp"` vs JSON `{"_type": "hlc", "value": "timestamp"}`
- `#blake3 "hash"` vs JSON `{"_type": "blake3", "value": "hash"}`

The canonical form (INV-LAYOUT-011) ensures deterministic serialization despite EDN's
flexible syntax. Human readability supports the shell-bootstrap phase (SR-005) where
`cat` and `grep` are valid inspection tools.

#### Consequences
- Transaction files are human-readable (cat, grep, less)
- Git diffs show meaningful content changes
- Requires an EDN parser in Rust (crate: `edn-rs` or custom)
- Canonical form must be enforced to guarantee deterministic hashing (INV-LAYOUT-011)

---

### ADR-LAYOUT-004: Hash-Prefix Directory Sharding

**Traces to**: ADRS SR-006
**Stage**: 0

#### Problem
How are transaction files organized within `txns/`?

#### Options
A) **Flat directory** — all files directly in `txns/`. Simple but degrades on filesystems
   with O(n) directory listing (ext4 without dir_index, or >100K files with any filesystem).

B) **Two-character hex prefix** — `txns/{hash[0..2]}/{hash}.edn`. 256 subdirectories,
   each holding ~N/256 files. Git objects pattern.

C) **Deeper prefix tree** — `txns/{hash[0..2]}/{hash[2..4]}/{hash}.edn`. 65,536
   subdirectories. Overkill for the expected scale (thousands, not millions, of transactions).

#### Decision
**Option B.** Two-character hex prefix, matching the git objects storage pattern. This
provides O(N/256) directory entries per subdirectory, handling up to ~25,000 transactions
per prefix (at 6.4M total) before any single directory has >100K entries.

#### Formal Justification
The sharding function `f(hash) = hash[0..2]` is a uniform hash partition (BLAKE3 is
uniformly distributed). Expected files per directory = N/256 with variance ~N/256.
For the target deployment (single-digit agents, thousands of datoms), this keeps each
directory well under filesystem performance limits.

#### Consequences
- Up to 256 subdirectories created on first use
- File lookup is O(1): compute hash → derive path → open file
- Compatible with git packfile compression (git packs small files efficiently)
- Filesystem-specific limits (inode count, directory entry count) are deferred
  to UNC-LAYOUT-001

---

### ADR-LAYOUT-005: Pure Filesystem Over Database Backend

**Traces to**: ADRS SR-003
**Stage**: 0
**Supersedes**: ADR-STORE-007 Option B (redb target)

#### Problem
Should the persistent storage backend be a database (redb, LMDB) or a pure filesystem?

#### Options
A) **redb** — Rust-native MVCC B-tree. Proven, fast, transactional. But creates a binary
   file that is not git-mergeable, requires database-specific tooling for inspection,
   and moves merge from a filesystem operation to a database operation.

B) **LMDB** — battle-tested, memory-mapped. Same tradeoffs as redb plus requires
   C FFI bindings.

C) **Pure filesystem** — content-addressed files in directories. Merge = copy files.
   Verification = blake3sum. Inspection = cat. Git-native. No binary format, no database
   dependency, no MVCC layer. Indexes are derived caches in `.cache/` (gitignored).

#### Decision
**Option C.** Pure filesystem. The content-addressed layout makes the G-Set CvRDT axioms
into filesystem tautologies. A database backend interposes an opaque binary layer between
the algebraic structure and its physical realization, obscuring the isomorphism and
requiring database-specific merge protocols.

#### Formal Justification
The isomorphism theorem (§1b.0) proves that the filesystem layout faithfully preserves
all algebraic properties. A database backend is a lossy projection: the database's internal
B-tree structure encodes ordering and page layout information that has no counterpart in
the G-Set algebra. Merge via database requires reconstructing the algebraic merge from
the database's physical representation — reversing a projection that discarded structural
information.

The filesystem approach has no such impedance mismatch: the physical structure (set of files)
IS the algebraic structure (set of transactions), just written to disk.

#### Consequences
- No redb, LMDB, or SQLite dependency for the core store
- Indexes in `.cache/` are derived caches, rebuilt from txns/ on demand
- Cold start requires scanning txns/ and rebuilding indexes (~O(N) where N = total datoms)
- Hot path uses in-memory indexes (BTreeSet<Datom>) loaded from .cache/
- ADR-STORE-007 Option B (redb target) is superseded
- SR-003 (LMDB/redb for MVCC) is superseded for the primary store

---

### ADR-LAYOUT-006: O_CREAT|O_EXCL Over flock for Concurrency

**Traces to**: ADRS SR-007
**Stage**: 0
**Supersedes**: SR-007 (flock coordination)

#### Problem
How do concurrent agents safely write to the layout?

#### Options
A) **flock (advisory file locking)** — agents acquire a lock before writing. Serializes
   writes, preventing concurrent access. But advisory locks are not enforced on all
   platforms, create a single point of contention, and are a process-coordination mechanism
   outside the datom store — violating FD-012 (store as sole coordination channel).

B) **O_CREAT|O_EXCL (atomic file creation)** — each write_tx creates a new file with a
   unique name (content hash). If the file already exists, the write is idempotent.
   No locking, no contention, no coordination. Safety is structural, not procedural.

C) **Optimistic concurrency (compare-and-swap on a version file)** — agents read a version,
   write their transaction, then atomically update the version. Requires a mutable shared
   state (the version file), conflicting with C1 (append-only).

#### Decision
**Option B.** O_CREAT|O_EXCL. Content-addressed naming means different transactions produce
different files (no conflict), and identical transactions produce the same file (idempotent).
Concurrency safety is a structural consequence of the naming scheme, not a runtime protocol.

#### Formal Justification
This is a direct application of INV-LAYOUT-010 (concurrent write safety). The proof has
two cases:
1. Different content → different hash → different file → no conflict (trivially safe)
2. Same content → same hash → same file → O_EXCL makes one succeed, one get AlreadyExists
   → AlreadyExists is not an error (content already present) → idempotent

No locking mechanism can improve on this: the structural elimination of contention is
strictly stronger than any coordination protocol, because it holds without any agent
needing to participate in a protocol.

#### Consequences
- SR-007 (flock coordination) is superseded
- No lock files, no contention, no deadlocks, no stale locks
- Works correctly on NFS, FUSE, and other filesystems that support O_EXCL
- Process crashes cannot leave corrupt state (write_tx is atomic: file exists or it doesn't)
- No ordering guarantee between concurrent writes (but the G-Set is unordered by definition)

---

### ADR-LAYOUT-007: Genesis as Standalone File

**Traces to**: ADRS SR-008, FD-006
**Stage**: 0

#### Problem
Where is the genesis transaction stored in the layout?

#### Options
A) **Only in txns/** — genesis is stored under its content hash like any other transaction.
   Consistent but requires computing the genesis hash to locate it.

B) **Only at .braid/genesis.edn** — genesis has a well-known path. Easy to find but creates
   a special case for one transaction.

C) **Both locations** — genesis.edn at the well-known path AND in txns/ under its content
   hash. The well-known path provides discoverability; the txns/ entry ensures it participates
   in all standard operations (list_txns, load_all, verify_integrity).

#### Decision
**Option C.** Genesis exists at both `.braid/genesis.edn` (discoverability) and
`txns/{hash[0..2]}/{hash}.edn` (participation in standard operations). The two copies
have identical content — this is enforced by Layout::init() and verified by
verify_integrity().

#### Formal Justification
The genesis transaction must be discoverable without computing its hash (for bootstrapping:
a new agent needs to find genesis before it can compute any hashes). It must also participate
in standard layout operations (load_all, verify_integrity) without special-casing. Both
locations store the same bytes, so BLAKE3(genesis.edn) = BLAKE3(txns/{hash}/{hash}.edn) —
the content-addressed identity property is preserved.

#### Consequences
- Layout::init() writes genesis to both locations
- verify_integrity() checks both copies agree
- load_all() deduplicates via the content hash (genesis appears once in the Store)
- New agents can discover genesis at a well-known path

---

## Negative Cases

### NEG-LAYOUT-001: No In-Place File Modification

**Traces to**: C1, INV-STORE-001, INV-LAYOUT-002
**Stage**: 0

**Safety property**: `□ ¬(∃ operation that modifies bytes of an existing file in txns/)`

**Statement**: No operation in the Layout API writes to, truncates, or modifies an existing
transaction file. The only filesystem write operation that targets `txns/` is `write_tx`,
which creates a *new* file via `O_CREAT|O_EXCL`.

**Enforcement mechanism**: Type-level. The Layout struct exposes `write_tx` (create),
`read_tx` (read), `list_txns` (enumerate). There is no `update_tx`, `modify_tx`,
`truncate_tx`, or any method that takes an existing path and modifies its contents.

**Violation condition**: The existence of any code path that opens an existing file in
`txns/` with write permissions (O_WRONLY or O_RDWR without O_CREAT|O_EXCL).

---

### NEG-LAYOUT-002: No File Deletion

**Traces to**: C1, INV-STORE-001, INV-STORE-005 (monotonicity)
**Stage**: 0

**Safety property**: `□ ¬(∃ removal of a transaction file from txns/)`

**Statement**: No operation in the Layout API deletes, moves, or renames a transaction file.
The transaction set is monotonically non-decreasing. This is C1 (append-only) at the
filesystem level.

**Enforcement mechanism**: Type-level. The Layout API has no `delete_tx`, `remove_tx`,
`cleanup_txns`, or `compact` method. The only deletion permitted is `.cache/` contents
(which are derived caches, not source of truth — INV-LAYOUT-009).

**Violation condition**: Any code path that calls `fs::remove_file`, `fs::remove_dir`,
or `fs::rename` on a path within `txns/`.

---

### NEG-LAYOUT-003: No Merge via File Append

**Traces to**: C4, INV-MERGE-001, INV-LAYOUT-004
**Stage**: 0

**Safety property**: `□ ¬(∃ merge operation that appends to an existing file)`

**Statement**: Merge is directory union — copying files between directories. No merge
operation appends data to an existing file. This prevents the trunk.ednl anti-pattern
where merging two stores requires interleaving lines in a shared file.

**Enforcement mechanism**: `merge_layouts` calls `write_tx` for each new transaction,
which creates new files. It never opens existing files for writing.

**Violation condition**: A merge implementation that opens an existing transaction file
and appends data to it.

---

### NEG-LAYOUT-004: No Transport-Specific Merge Logic

**Traces to**: INV-LAYOUT-006 (transport independence)
**Stage**: 0

**Safety property**: `□ ¬(∃ merge code path that depends on a specific transport mechanism)`

**Statement**: `merge_layouts` operates on two Layout instances via `read_tx` and `write_tx`.
It does not import, reference, or depend on git, rsync, scp, or any transport library.
The merge result is independent of how files arrived in the layout.

**Enforcement mechanism**: Code review and dependency audit. The `layout` module's
`Cargo.toml` section has no transport-related dependencies. `merge_layouts` takes
`&Layout` references, not transport handles.

**Violation condition**: An `import git` / `use git2` / or similar transport dependency
in the layout module, or merge behavior that differs based on transport mechanism.

---

### NEG-LAYOUT-005: No Index as Source of Truth

**Traces to**: INV-LAYOUT-009 (index derivability), C1
**Stage**: 0

**Safety property**: `□ ¬(∃ datum in .cache/ absent from txns/)`

**Statement**: The `.cache/` directory is a derived projection of `txns/`. Every datum in
every index file in `.cache/` must be traceable to a datom in a transaction file in `txns/`.
Deleting `.cache/` and rebuilding produces an identical store.

**Enforcement mechanism**: `rebuild_cache()` reads only from `txns/` and writes only to
`.cache/`. No operation writes to `.cache/` from any other source. The `.gitignore` excludes
`.cache/` from version control, reinforcing its derived status.

**Violation condition**: A datum present in `.cache/` that cannot be found in any transaction
file in `txns/`, or a `load_all()` result that differs before and after `rebuild_cache()`.

---

## Uncertainty Markers

### UNC-LAYOUT-001: Filesystem Performance at Scale

**Source**: INV-LAYOUT-008 (sharded directory)
**Confidence**: 0.85
**Stage affected**: 0+

**Claim**: 256-way hash-prefix sharding provides adequate filesystem performance up to
100,000 transaction files (~390 files per directory).

**Why uncertain**: Filesystem performance depends on the specific filesystem (ext4, XFS,
ZFS, btrfs), inode allocation strategy, and directory hashing implementation. Some
filesystems degrade significantly above certain directory entry counts.

**Impact if wrong**: Startup time (scanning txns/ for load_all) or write_tx latency
becomes unacceptable. May require deeper sharding (4-char prefix, 65,536 dirs) or
alternative enumeration strategies.

**Resolution**: Benchmark with 100K synthetic transaction files on ext4 and XFS.
Measure: write_tx latency, list_txns throughput, load_all wall time.

**What breaks**: Layout correctness is unaffected. Only performance is at risk.

---

### UNC-LAYOUT-002: EDN Parser Throughput for Bulk Startup

**Source**: INV-LAYOUT-003 (directory-store isomorphism)
**Confidence**: 0.90
**Stage affected**: 0+

**Claim**: An EDN parser in Rust can process transaction files fast enough for interactive
startup. Target: load 10,000 transaction files in < 1 second.

**Why uncertain**: EDN parsing throughput depends on implementation quality and transaction
file size. If transactions carry large values (e.g., full document text as datom values),
parsing may bottleneck on string allocation.

**Impact if wrong**: Cold start (no .cache/) is too slow for interactive use. Mitigated by
index caching (.cache/ avoids full re-parse on warm start).

**Resolution**: Implement EDN parser, benchmark with realistic transaction sizes.

**What breaks**: Warm starts (cached indexes) are unaffected. Only cold-start and
rebuild_cache performance is at risk.

---

### UNC-LAYOUT-003: Git Packfile Efficiency with Small Files

**Source**: ADR-LAYOUT-004 (hash-prefix sharding)
**Confidence**: 0.80
**Stage affected**: 3+

**Claim**: Git packfile compression handles 100K+ small EDN files efficiently, keeping
repository clone size manageable.

**Why uncertain**: Git packfiles delta-compress between objects. Small, structurally
similar EDN files should compress well, but the per-object overhead (header, SHA-1 hash)
may dominate for very small files. Git's default gc/pack thresholds may trigger
too-frequent repacking.

**Impact if wrong**: Repository clone size grows linearly instead of sub-linearly.
Network transfer for merge-via-git is slower than expected.

**Resolution**: Create a test repository with 100K synthetic transaction files, measure
pack size and clone time. Compare with equivalent data in a single file.

**What breaks**: Store correctness and merge correctness are unaffected. Only git-based
transport performance is at risk. Non-git transports (rsync, cp) are unaffected.

---

## Level 2 Types — Complete Reference

```rust
/// Content-addressed transaction file.
/// Invariant: hash = BLAKE3(canonical_edn(self))
pub struct TxFile {
    pub hash: [u8; 32],       // BLAKE3 of canonical serialization
    pub tx_meta: TxMeta,      // HLC id, agent, provenance, predecessors, rationale
    pub datoms: Vec<Datom>,   // Datoms in this transaction
}

/// Transaction metadata.
pub struct TxMeta {
    pub id: TxId,                        // HLC timestamp (SR-004)
    pub agent: AgentId,                  // Authoring agent
    pub provenance: Provenance,          // :observed | :derived | :inferred | :hypothesized (PD-002)
    pub causal_predecessors: Vec<TxId>,  // Causal ordering
    pub rationale: String,               // Human/agent-readable reason
}

/// Physical layout handle.
pub struct Layout {
    pub root: PathBuf,        // .braid/ directory
}

impl Layout {
    /// Open an existing layout at the given root.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, LayoutError>;

    /// Initialize a new layout: create directories, write genesis, write .gitignore.
    pub fn init(root: impl AsRef<Path>) -> Result<Self, LayoutError>;

    /// Write a transaction file. Content-addressed: idempotent if same content exists.
    pub fn write_tx(&self, tx: &TxFile) -> Result<PathBuf, LayoutError>;

    /// Read a transaction file by its BLAKE3 hash.
    pub fn read_tx(&self, hash: &[u8; 32]) -> Result<TxFile, LayoutError>;

    /// List all transaction hashes in the layout.
    pub fn list_txns(&self) -> Result<Vec<[u8; 32]>, LayoutError>;

    /// Verify integrity: every file's name matches BLAKE3(contents).
    pub fn verify_integrity(&self) -> Result<IntegrityReport, LayoutError>;

    /// Load all transactions into an in-memory Store.
    pub fn load_all(&self) -> Result<Store, LayoutError>;

    /// Rebuild .cache/ indexes from txns/.
    pub fn rebuild_cache(&self) -> Result<(), LayoutError>;
}

/// Free functions (ADR-ARCHITECTURE-001, SR-013)
pub fn merge_layouts(target: &Layout, source: &Layout) -> Result<MergeReceipt, LayoutError>;
pub fn verify_layout(layout: &Layout) -> Result<IntegrityReport, LayoutError>;
pub fn canonical_edn(tx: &TxFile) -> Vec<u8>;

/// Merge receipt — records what happened during a merge.
pub struct MergeReceipt {
    pub new_count: u64,       // Files copied from source (new transactions)
    pub dup_count: u64,       // Files skipped (already present — content-addressed dedup)
}

/// Integrity report — results of verify_integrity.
pub struct IntegrityReport {
    pub total_files: usize,
    pub valid_files: usize,
    pub corrupt_files: Vec<(PathBuf, IntegrityError)>,
}

/// Integrity error types.
pub enum IntegrityError {
    HashMismatch { expected: [u8; 32], actual: [u8; 32] },
    ParseError(String),
    IoError(std::io::Error),
}

/// Layout errors.
pub enum LayoutError {
    /// Filesystem I/O error.
    IoError(std::io::Error),
    /// EDN parse error.
    ParseError(String),
    /// Layout not initialized (missing genesis.edn or txns/).
    NotInitialized,
    /// Integrity check failed during read.
    IntegrityError(IntegrityError),
}
```

---

## Cross-Reference Summary

### LAYOUT ← Realizes STORE (via φ)

| LAYOUT Element | Realizes | Mechanism |
|---|---|---|
| INV-LAYOUT-001 (content-addressed file) | INV-STORE-003 (content-addressable identity), C2 | φ preserves identity: same datom → same hash → same file |
| INV-LAYOUT-002 (file immutability) | INV-STORE-001 (append-only), C1 | φ preserves append-only: no file modification = no datom mutation |
| INV-LAYOUT-003 (directory-store isomorphism) | All STORE invariants | The isomorphism theorem: ψ ∘ φ = id |
| INV-LAYOUT-004 (merge = dir union) | INV-MERGE-001 (set union), C4 | φ commutes with merge: φ(S₁ ∪ S₂) = φ(S₁) ∪_dir φ(S₂) |
| INV-LAYOUT-005 (self-verification) | C7 (self-bootstrap) | Store verifies itself via layout hash-checking |
| INV-LAYOUT-006 (transport independence) | C4, INV-MERGE-001 | Merge result independent of file transport |
| INV-LAYOUT-007 (genesis determinism) | INV-STORE-010 (genesis), INV-SCHEMA-001 | φ preserves genesis identity |
| INV-LAYOUT-008 (sharded scalability) | (performance, not algebraic) | Filesystem optimization for the isomorphism |
| INV-LAYOUT-009 (index derivability) | C7 (self-bootstrap) | Indexes are projections of txns/, not truth sources |
| INV-LAYOUT-010 (concurrent safety) | SR-007 (supersedes flock) | Structural elimination of contention |
| INV-LAYOUT-011 (canonical serialization) | INV-LAYOUT-001 (prerequisite), C2 | Identity preservation requires deterministic bytes |
| ADR-LAYOUT-001 | ADR-STORE-007 Option A (supersedes) | Per-txn files replace trunk.ednl |
| ADR-LAYOUT-005 | ADR-STORE-007 Option B (supersedes) | Pure filesystem replaces redb target |
| ADR-LAYOUT-006 | SR-007 (supersedes) | O_CREAT|O_EXCL replaces flock |

---
