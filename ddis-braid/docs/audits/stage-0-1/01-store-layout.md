# Store + Storage Layout — Stage 0/1 Audit
> Wave 1 Domain Audit | Date: 2026-03-17 | Agent: Opus 4.6 | Methodology: Fagan Inspection + IEEE Walkthrough

## Domain Inventory

### STORE Namespace (spec/01-store.md)
- **INVs**: INV-STORE-001 through INV-STORE-016 (16 invariants)
- **ADRs**: ADR-STORE-001 through ADR-STORE-021 (21 ADRs)
- **NEGs**: NEG-STORE-001 through NEG-STORE-005 (5 negative cases)

### LAYOUT Namespace (spec/01b-storage-layout.md)
- **INVs**: INV-LAYOUT-001 through INV-LAYOUT-011 (11 invariants)
- **ADRs**: ADR-LAYOUT-001 through ADR-LAYOUT-007 (7 ADRs)
- **NEGs**: NEG-LAYOUT-001 through NEG-LAYOUT-005 (5 negative cases)

---

## Findings

### FINDING-001: EntityId::from_raw_bytes bypasses content-addressable identity guarantee (INV-STORE-003 / ADR-STORE-014)
Severity: HIGH
Type: DIVERGENCE
Sources: spec/01-store.md:INV-STORE-003,ADR-STORE-014 vs crates/braid-kernel/src/datom.rs:66-68
Evidence: ADR-STORE-014 states "Private inner field. Content-addressable identity (C2) means EntityIds must correspond to actual content hashes. A public constructor allows creating EntityIds from arbitrary bytes, bypassing the hash." Yet `EntityId::from_raw_bytes` at datom.rs:66 is `pub` (not `pub(crate)`), and its doc comment says "for deserialization only." This means any external crate can construct an EntityId from arbitrary bytes, breaking the guarantee. The spec (01-store.md line 321) specifies a `pub(crate)` deserialization constructor at the "trusted boundary," but the implementation uses unrestricted `pub`. Furthermore, `from_raw_bytes` is used extensively in kani proofs (kani_proofs.rs lines 70,838,909,1017,1089,1795,1860,1910,1989) and proptest strategies (proptest_strategies.rs:40) to create EntityIds without content hashing, widening the attack surface.
Impact: External consumers of the `braid-kernel` crate can construct EntityIds that do not correspond to any content hash, silently violating C2. Datoms containing such "fake" EntityIds would be accepted by the store, polluting the content-addressable invariant.

### FINDING-002: Causal predecessor validation uses wrong error variant
Severity: MEDIUM
Type: DIVERGENCE
Sources: spec/01-store.md:INV-STORE-010 (Level 2) vs crates/braid-kernel/src/store.rs:156-162
Evidence: The spec defines a `TxValidationError::MissingCausalPredecessor(TxId)` error for when a causal predecessor is not found. The implementation at store.rs:158 instead returns `StoreError::DuplicateTransaction(format!("causal predecessor not found: {:?}", pred))` -- reusing the wrong error variant. The error message says "predecessor not found" but the variant name says "DuplicateTransaction", creating a misleading error. The spec (01-store.md line 804) shows `TxValidationError::MissingCausalPredecessor(*pred)` as the correct return.
Impact: Error handling code that matches on `DuplicateTransaction` will confuse duplicate-transaction errors with missing-predecessor errors. Agents relying on error types for programmatic recovery will take wrong corrective actions.

### FINDING-003: Genesis attribute count inconsistency across spec, guide, code, and SEED
Severity: HIGH
Type: CONTRADICTION
Sources: spec/01-store.md:266 vs spec/01-store.md:706 vs crates/braid-kernel/src/schema.rs:488 vs docs/guide/01-store.md:108
Evidence: The genesis attribute count is stated differently across documents:
- spec/01-store.md line 266: "the 17 axiomatic attribute definitions" (in GENESIS state machine)
- spec/01-store.md line 706: "exactly the 18 axiomatic meta-schema attributes" (INV-STORE-008 Level 1)
- spec/01b-storage-layout.md line 439: "18 axiomatic meta-schema attributes (SR-008)"
- crates/braid-kernel/src/schema.rs:488: `GENESIS_ATTR_COUNT: usize = 19`
- docs/guide/01-store.md:108: "hardcoded 17 axiomatic attributes"
- docs/guide/01b-storage-layout.md:491: "17 axiomatic meta-schema attributes"
- docs/guide/02-schema.md:81: "The 17 axiomatic meta-schema attributes"
The actual code has 19 attributes. The spec says both 17 and 18. The guides say 17.
Impact: This is a three-way contradiction. Any verification test that checks the genesis count against a spec-stated number will fail depending on which document it references. The inconsistency indicates the genesis set has grown without all documents being updated, creating a persistent confusion source.

### FINDING-004: VAET and AVET indexes specified but not implemented
Severity: MEDIUM
Type: UNIMPLEMENTED
Sources: spec/01-store.md:283-286 (Index Invariants) + docs/guide/01-store.md:114-122 vs crates/braid-kernel/src/store.rs:443-461
Evidence: The spec (01-store.md Section 1.2) defines four indexes: EAVT, AEVT, VAET, AVET. The guide (01-store.md) describes all four with their key orderings and use cases. The implementation in store.rs has:
- EAVT: Implicit via `BTreeSet<Datom>` ordering (entity -> attribute -> value -> tx -> op)
- `entity_index: BTreeMap<EntityId, Vec<Datom>>` (entity lookup)
- `attribute_index: BTreeMap<Attribute, Vec<Datom>>` (attribute scan)
- No VAET index (no reverse-reference lookup structure)
- No AVET index (no unique/range lookup structure)
The guide at 01-store.md:145 notes "Stage 2 extension (LIVE index, INV-STORE-012-013)" but the missing VAET and AVET are spec'd as Stage 0.
Impact: Reverse-reference traversal queries ("who references entity E?") and unique/range lookups ("which entity has A=V?") cannot be served in O(1). They require full datom scans. The spec claims these indexes exist at Stage 0.

### FINDING-005: Store::as_of and SnapshotView specified but not implemented
Severity: MEDIUM
Type: UNIMPLEMENTED
Sources: spec/01-store.md:425-435 (SnapshotView) + spec/01-store.md:458-459 (Store::as_of) vs crates/braid-kernel/src/store.rs
Evidence: The spec defines `Store::as_of(&self, frontier: &Frontier) -> SnapshotView` and a `SnapshotView` struct with `current()`, `len()`, and `datoms()` methods. The guide (01-store.md:157-178) shows `SnapshotView<'a>` with a full implementation contract. A grep for `as_of`, `SnapshotView`, and `current.*entity` in store.rs returns no matches. The `Frontier::at()` method exists (store.rs:280) as a building block, but the higher-level `as_of` API that uses it is absent.
Impact: Time-travel queries ("what was the store state at transaction T?") cannot be performed through the specified API. The `Frontier` type provides the primitives, but the spec-defined composition is missing.

### FINDING-006: INV-STORE-009 (Frontier Durability) not enforced in kernel
Severity: HIGH
Type: UNIMPLEMENTED
Sources: spec/01-store.md:730-751 (INV-STORE-009) vs crates/braid-kernel/src/store.rs:556-641
Evidence: INV-STORE-009 states "frontier(alpha) is durably stored BEFORE the operation returns" with Level 2 code showing `self.persist_frontier()?; // fsync before returning`. A grep for `persist_frontier`, `fsync`, and `sync_all` in store.rs returns zero matches. The `transact()` method at store.rs:569-641 updates the in-memory frontier at line 626 (`self.frontier.insert(tx_data.agent, tx_id)`) but performs no persistence. The `DiskLayout` (crates/braid/src/layout.rs) persists transaction files with `sync_all()` but the frontier itself is derived -- it is not independently persisted before the response returns.
Impact: On crash after a successful transact but before the layout writes complete, the frontier is lost. The spec requires the frontier to survive crashes. While it is reconstructable from the datom set via `Store::from_datoms()`, this reconstruction is not the same as "durably stored BEFORE the operation returns." INV-STORE-016 (Frontier Computability) provides a recovery mechanism, but INV-STORE-009 requires something stronger: the frontier must be durable before any response to the caller.

### FINDING-007: INV-STORE-013 (Working Set Isolation) declared Stage 2 but referenced in Stage 0 transact
Severity: LOW
Type: MISALIGNMENT
Sources: spec/01-store.md:928-963 (INV-STORE-013, Stage: 2) vs crates/braid-kernel/src/store.rs:566 comment
Evidence: INV-STORE-013 is explicitly marked "Stage: 2" in the spec. Yet store.rs:566 lists `INV-STORE-013: Working set isolation -- only committed datoms enter store` in its doc comment for the `transact()` method as a currently-enforced invariant. The `WorkingSet` type specified in INV-STORE-013 Level 2 does not exist anywhere in the codebase. The Store::transact method only accepts `Transaction<Committed>`, which provides a weaker form of the guarantee (committed-before-apply), but the full working set isolation concept (private local datoms invisible to other agents) is not implemented.
Impact: Low severity because the claim is aspirational documentation rather than a functional defect. However, it creates confusion about what is actually enforced at Stage 0 vs. what is deferred.

### FINDING-008: INV-STORE-014 (Every Command Is a Transaction) partially implemented for read commands
Severity: MEDIUM
Type: GAP
Sources: spec/01-store.md:970-1024 (INV-STORE-014) vs crates/braid-kernel/src/store.rs
Evidence: INV-STORE-014 Level 0 states: "Read commands (query, status, seed, guidance): P is a local record in the agent's working set W_alpha." The spec requires read commands to produce provenance records in the agent's working set. Since the working set (INV-STORE-013) is not implemented (Stage 2), read commands produce no provenance records at all. The `query` function in the query module takes `&Store` immutably and produces results without any provenance recording. For mutating commands, transaction metadata IS recorded (store.rs:606-623 inserts `:tx/time`, `:tx/agent`, `:tx/provenance`, `:tx/rationale`), so the mutating half of INV-STORE-014 is implemented.
Impact: No audit trail for read operations. The spec envisions using read provenance for significance weighting and drift detection. Without it, "what was this agent curious about?" queries cannot be answered.

### FINDING-009: INV-STORE-015 (Agent Entity Completeness) not enforced
Severity: MEDIUM
Type: UNIMPLEMENTED
Sources: spec/01-store.md:1027-1065 (INV-STORE-015) vs crates/braid-kernel/src/store.rs:836-843
Evidence: INV-STORE-015 requires that every `:tx/agent` Ref resolves to an entity with `:agent/ident`, `:agent/program`, and `:agent/model` attributes. The spec Level 2 shows `ensure_agent_entity()` should auto-create agent entities if they don't exist. The implementation at store.rs:836-843 creates a `:tx/agent` Ref pointing to `EntityId::from_content(tx_data.agent.as_bytes())`, but it never creates agent entity datoms (`:agent/ident`, `:agent/program`, `:agent/model`) for non-genesis agents. Only the genesis transaction creates the system agent entity (via `genesis_datoms()` in schema.rs). Any user agent transacting will have a dangling `:tx/agent` Ref.
Impact: The `:tx/agent` references produced by `transact()` point to entities that have no attributes, making them opaque. Queries asking "which agent created this transaction?" can follow the Ref but find no descriptive attributes. This silently violates the invariant for every non-genesis transaction.

### FINDING-010: Datom sort order in layout serialization does not sort by datom tx field
Severity: LOW
Type: DIVERGENCE
Sources: spec/01b-storage-layout.md:660-663 (INV-LAYOUT-011 Level 1) vs crates/braid-kernel/src/layout.rs:233-236
Evidence: The spec (01b-storage-layout.md INV-LAYOUT-011 Level 1) states canonical form guarantees: "Datom vectors sorted by (entity, attribute, value, op)." The datom ordering in the Datom struct's `derive(Ord)` is `(entity, attribute, value, tx, op)` (datom.rs:386-398), which includes tx in the sort. The serialization writes datoms in the order they appear in `tx.datoms` (layout.rs:233) without sorting -- it writes them in the order provided by the `TxFile.datoms` Vec. The spec says datoms should be sorted by `(entity, attribute, value, op)` -- notably omitting `tx` from the sort key. The code does not explicitly sort before writing. However, during deserialization, all datoms within a single TxFile share the same tx_id (layout.rs:771: `datom.tx = tx_id`), so the sort key difference is moot within a file.
Impact: Low impact because within a single TxFile, all datoms share the same tx_id, making the sort key difference irrelevant. However, the serializer does not explicitly sort datoms at all -- it relies on the caller to provide them in order. If a TxFile is constructed with unsorted datoms, the canonical serialization invariant is violated, producing different hashes for logically identical transactions.

### FINDING-011: Layout module split between kernel and binary crate diverges from guide prescription
Severity: LOW
Type: DIVERGENCE
Sources: docs/guide/01b-storage-layout.md:12-32 vs actual crate structure
Evidence: The guide (01b-storage-layout.md) prescribes: "crates/braid-kernel/src/layout.rs -- Canonical serialization, deserialization, hashing (pure functions)" and "crates/braid/src/persistence.rs -- Filesystem I/O". The actual implementation places filesystem I/O in `crates/braid/src/layout.rs` (not `persistence.rs`). The kernel layout.rs correctly contains pure functions. The naming divergence (`layout.rs` vs. `persistence.rs`) is minor but means the guide's module references are wrong.
Impact: Minimal functional impact, but developers following the guide will look for `persistence.rs` and not find it. The architectural split (pure kernel / IO binary) is preserved -- only the filename differs.

### FINDING-012: Spec INV-STORE-008 says "18 axiomatic" but Genesis state machine says "17 axiomatic"
Severity: MEDIUM
Type: CONTRADICTION
Sources: spec/01-store.md:266 vs spec/01-store.md:706
Evidence: Within the same spec file (01-store.md), the Genesis state machine at line 266 says "meta_schema_datoms = the 17 axiomatic attribute definitions" while INV-STORE-008 Level 1 at line 706 says "The genesis transaction installs exactly the 18 axiomatic meta-schema attributes." These two statements in the same document contradict each other. (See FINDING-003 for the broader inconsistency including code.)
Impact: A falsification test for INV-STORE-008 would need to know the exact count. The spec gives two different numbers within itself.

### FINDING-013: No LIVE index exists in implementation
Severity: MEDIUM
Type: UNIMPLEMENTED
Sources: spec/01-store.md:288-294 (LIVE Index) + spec/01-store.md:880-924 (INV-STORE-012) vs crates/braid-kernel/src/store.rs
Evidence: The spec defines a LIVE index as "LIVE(S) = fold(causal_sort(S), apply_resolution)" with INV-STORE-012 requiring it to be maintained incrementally and match a full recompute. The guide (01-store.md:145-148) defers LIVE to "Stage 2 extension." The store.rs implementation has `entity_index` and `attribute_index` but no LIVE materialized view. There is no `LiveIndex` struct, no `apply_tx()`, and no `recompute()`. Resolution logic exists in `resolution.rs` but is not integrated as a materialized index.
Impact: "Current value" queries must be computed on-the-fly from raw datoms and the resolution module, rather than served from a pre-computed materialized view. This is a performance concern more than a correctness one at Stage 0, but the spec places INV-STORE-012 at Stage 0.

### FINDING-014: Layout read_tx in binary crate does not verify content hash
Severity: MEDIUM
Type: GAP
Sources: spec/01b-storage-layout.md:INV-LAYOUT-005 (Integrity Self-Verification) + docs/guide/01b-storage-layout.md:214-222 vs crates/braid/src/layout.rs:135-151
Evidence: The guide prescribes that `read_transaction` should verify `BLAKE3(bytes) == expected_hash` on every read. The binary crate's `DiskLayout::read_tx()` at layout.rs:135-151 reads the file and deserializes it but does NOT verify the content hash matches the filename. It only checks that the filename input is valid hex. The guide explicitly shows: `let actual = transaction_hash(&bytes); if actual != *hash { return Err(...) }`. This hash-on-read check is absent from the implementation.
Impact: Corrupt transaction files (bit rot, manual tampering) will be silently loaded and their invalid datoms merged into the store. The `verify_integrity()` method does check hashes, but it must be called explicitly -- it is not a guardrail on every read.

### FINDING-015: EDN serializer does not sort datoms before serialization
Severity: MEDIUM
Type: DIVERGENCE
Sources: spec/01b-storage-layout.md:INV-LAYOUT-011 (lines 644-684) + docs/guide/01b-storage-layout.md:108-111 vs crates/braid-kernel/src/layout.rs:215-239
Evidence: The spec (INV-LAYOUT-011 Level 1) requires "Datom vectors sorted by (entity, attribute, value, op)." The guide (01b-storage-layout.md:108-111) shows: "1. Sort datoms by (entity, attribute, value, op); 2. Sort causal predecessors by HLC." The implementation's `serialize_tx()` at layout.rs:215-239 iterates `tx.datoms` in order without sorting: `for datom in &tx.datoms { ... }`. The `canonical_edn()` function mentioned in the spec is not called. Neither datoms nor causal predecessors are sorted before writing. This means two `TxFile` instances with identical logical content but different datom ordering will produce different byte sequences, different BLAKE3 hashes, and different filenames -- violating the prerequisite for INV-LAYOUT-001.
Impact: Content-addressed identity breaks for logically identical transactions with different datom orderings. Two agents constructing the same logical transaction independently may produce datoms in different orders, creating duplicate files that should have been deduplicated. This undermines the G-Set deduplication property at the filesystem level.

---

## Quantitative Summary

### STORE Namespace
| Metric | Count |
|--------|-------|
| Total INVs | 16 |
| Implemented | 10 (INV-STORE-001,002,003,004,005,006,007,008,011,014-partial) |
| Unimplemented | 4 (INV-STORE-009-durability,012-LIVE,013-WorkingSet,015-AgentEntity) |
| Divergent | 2 (INV-STORE-010-error-variant, INV-STORE-014-read-provenance) |
| Total ADRs | 21 |
| Reflected in code | 18 |
| Drifted | 3 (ADR-STORE-005-only2of4indexes, ADR-STORE-014-pub-not-pubcrate, ADR-STORE-015-merge-is-method-not-free) |
| Total NEGs | 5 |
| Enforced | 5 (NEG-STORE-001..005 all hold -- BTreeSet only grows, no compaction, no sequential IDs) |
| Reachable | 0 |

### LAYOUT Namespace
| Metric | Count |
|--------|-------|
| Total INVs | 11 |
| Implemented | 8 (INV-LAYOUT-001,002,003,005,006,007,008,010) |
| Unimplemented | 1 (INV-LAYOUT-009-rebuild_cache not exposed in binary crate) |
| Divergent | 2 (INV-LAYOUT-011-no-sorting, INV-LAYOUT-004-kernel-only-no-filesystem-merge) |
| Total ADRs | 7 |
| Reflected in code | 7 (all reflected) |
| Drifted | 0 |
| Total NEGs | 5 |
| Enforced | 5 (all enforced -- no delete/modify/append/transport-specific code in layout) |
| Reachable | 0 |

---

## Domain Health Assessment

**Strongest aspect**: The algebraic foundation is solid. The G-Set CRDT properties (INV-STORE-004 through INV-STORE-007) are thoroughly tested with unit tests, property-based tests (proptest), and bounded model checking (Kani harnesses). The merge operation is correctly implemented as BTreeSet union, and commutativity/associativity/idempotency/monotonicity are all verified. The typestate transaction pattern (Building -> Committed -> Applied) correctly prevents applying un-validated transactions at compile time. The content-addressed layout serialization is deterministic (same input = same bytes) and the EDN round-trip works correctly. Negative cases are well-enforced: the API surface genuinely has no delete, modify, or compact operations.

**Most concerning gap**: **FINDING-015** (serializer does not sort datoms) is the most structurally dangerous finding because it undermines the Store-Layout Isomorphism that the entire LAYOUT namespace depends on. If two agents independently construct a `TxFile` with the same logical transaction but datoms in different vector order, the serializer produces different bytes, different BLAKE3 hashes, and different filenames. The filesystem will then contain two files representing one logical transaction, defeating the content-addressed deduplication that makes "merge = directory union" a tautology. This is a prerequisite failure: INV-LAYOUT-011 (canonical serialization) is described in the spec as "a PREREQUISITE for INV-LAYOUT-001" and the guide's build order lists it as "Build FIRST." The fix is straightforward (sort datoms and causal_predecessors before serializing), but until it is applied, the isomorphism theorem has a silent hole.

The second most concerning cluster is the **genesis attribute count contradiction** (FINDING-003/012). The spec says 17 in one place and 18 in another within the same file; the code says 19; the guides say 17. This triple-disagreement means no document accurately describes the system, and any new contributor will be confused about the axiomatic foundation.
