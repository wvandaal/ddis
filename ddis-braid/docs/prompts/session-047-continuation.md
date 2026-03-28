# Braid Execution Prompt — DAEMON-SCALE + WRITER + Audit Remediation

> **Scope**: 66 tasks across 6 execution phases. ~5,000 LOC of cleanroom Rust.
> **Mandate**: Lab-grade, zero-defect, production-ready implementation.
> **Method**: Spec-driven, algebraically grounded, property-verified.
> **Parallelism**: Opus 4.6 subagents with /effort max at every phase boundary.

---

## Quality Exemplar

This is what excellent braid code looks like. Your output must match this standard.

```rust
/// Flush the in-memory store to disk if dirty.
///
/// **Black box**: If dirty, serialize store.bin atomically (tmp+rename).
///   If another process has newer data, skip to preserve their state.
/// **State box**: dirty flag + known_hashes + flock coordination.
/// **Clear box**: See implementation below.
///
/// INV-STORE-020: After flush, store.bin = fold(txn_files).
/// INV-STORE-022: No stale cache writes.
pub fn flush(&mut self) -> Result<(), BraidError> {
    if !self.dirty {
        return Ok(());
    }
    let lock_file = fs::OpenOptions::new()
        .create(true).write(true).open(&lock_path)?;
    // LOCK_NB: non-blocking — skip rather than hang (DEFECT-004 fix)
    let lock_result = unsafe {
        libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB)
    };
    if lock_result != 0 {
        self.dirty = false;
        return Ok(()); // Txn files are durable (C1). Next open rebuilds.
    }
    // Under lock: verify known_hashes >= disk_hashes
    let disk_hashes: HashSet<String> = self.layout
        .list_tx_hashes().unwrap_or_default().into_iter().collect();
    if disk_hashes.difference(&self.known_hashes).count() > 0 {
        self.dirty = false;  // We'd write a subset. Skip.
        return Ok(());       // DW0 fingerprint ensures next reader rebuilds.
    }
    // We have all data. Write.
    let mut sorted: Vec<String> = self.known_hashes.iter().cloned().collect();
    sorted.sort();
    self.layout.write_slim_cache(&self.store, &sorted)?;
    self.dirty = false;
    Ok(())
}
```

Every function you write must have: (1) doc comment with invariant references,
(2) three-box decomposition where non-trivial, (3) explicit error handling (no
unwrap in library code), (4) the simplest correct implementation.

---

## Hard Constraints

These are non-negotiable. Violating any is a CRITICAL defect.

- **C1**: Append-only store. Never delete or mutate datoms. Retractions are new datoms.
- **C2**: Content-addressable identity. EntityId = BLAKE3(content).
- **C4**: CRDT merge by set union. Commutative, associative, idempotent.
- **C8**: Substrate independence. Kernel must not hardcode DDIS methodology.
- **INV-STORE-020**: After flush, store.bin = fold(txn_files).
- **INV-STORE-022**: No stale cache writes.
- **INV-DAEMON-004**: Semantic equivalence between daemon and direct mode.
- **INV-LAYOUT-006**: Transaction files immutable after write (0o444 + chattr).
- **`#![forbid(unsafe_code)]`** in braid-kernel. Unsafe only in braid crate with SAFETY comments.

---

## Phase 0: Load Context (do this FIRST — before writing ANY code)

1. Run `braid seed --task "DAEMON-SCALE + WRITER + Audit remediation"`.
2. Read `SEED.md` — the project's soul. All 11 sections.
3. Read `docs/audits/session-047/AUDIT_REPORT.md` — the 72-finding audit.
4. Read `docs/audits/session-047/DAEMON_MIGRATION_PROPOSAL.md` — the Option F architecture.
5. Explore the 6 files modified in DAEMON-WRITE (layout.rs, live_store.rs, daemon.rs, mcp.rs, main.rs, commands/mod.rs) — understand what was just built.
6. Use `cass search "DAEMON-SCALE WAL group commit checkpoint"` for session context.
7. Use `cm context "daemon WAL concurrent agents flush"` for procedural memory.
8. Run `ms load rust-formal-engineering -m --full` — internalize the Curry-Howard lens, typestate patterns, cardinality analysis, three-box cleanroom protocol.

**Checkpoint**: Before writing any code, you must be able to articulate:
- What is the three-concern separation (visibility, durability, git-readiness)?
- Why is braid's grow-only set simpler than a database (no MVCC, no page locking, no GC)?
- What is the single-writer principle and why does it prevent stale cache?
- What did DAEMON-WRITE (Phase 1) already fix and what remains?

If you cannot answer all four with file:line evidence, go back and read more.

---

## Phase 1: Pre-Work (2 tasks)

Fix the two defects from the cleanroom review before starting new work.

| Task | What | File | LOC |
|------|------|------|-----|
| DEFECT-006 | Unit test for DW0b flush guard skip path | live_store.rs | 30 |
| DEFECT-003 | Resolve harvest default task divergence | daemon.rs | 5 |

**Verify**: `cargo test -p braid` passes. Mark both done in braid.

---

## Phase 2: DAEMON-SCALE (12 tasks, ~2000 LOC)

The Option F Hybrid WAL architecture. Three concerns, each optimized independently:

```
Agent CLI --socket--> Daemon (multi-threaded)
                       |-- RwLock<Store>           (VISIBILITY: in-memory, ~1ms)
                       |-- WAL buffer + fsync      (DURABILITY: append-only, ~200us)
                       |-- Checkpoint thread       (GIT-READY: WAL->edn, background)
                       '-- Thread pool             (CONCURRENCY: unlimited reads)
```

### Execution order (5 waves, parallelize within each):

**Wave A** (parallel, no shared deps):
- `DS1 t-f590b219` — WAL binary format + append writer (crates/braid-kernel/src/wal.rs, ~300 LOC)
- `DS4 t-69562080` — RwLock multi-threaded dispatch (crates/braid/src/daemon.rs, ~100 LOC)

**Wave B** (after Wave A):
- `DS1-TEST t-ab94bbe4` — WAL tests (9 tests incl concurrent append)
- `DS2 t-fdd704dd` — Group commit thread (daemon.rs, ~200 LOC)
- `DS3 t-3cbb5eb7` — PASSIVE checkpoint thread (daemon.rs, ~150 LOC)
- `DS4-TEST t-45f009cd` — Dispatch tests (6 tests incl 50-agent simulation)

**Wave C** (after Wave B):
- `DS2-TEST t-f9c3b10b` — Group commit tests (8 tests incl 50-writer)
- `DS3-TEST t-4e7bf629` — Checkpoint tests (7 tests incl harvest E2E)
- `DS5 t-3a27170e` — Crash recovery (live_store.rs, ~100 LOC)

**Wave D** (after Wave C):
- `DS5-TEST t-1afedb8e` — Recovery tests (8 tests incl kill-and-recover)
- `DS6 t-04183d5b` — Integration: wire all + rename store.bin->checkpoint.bin (~200 LOC)

**Wave E** (after Wave D):
- `DS7 t-56c06582` — Comprehensive scale verification (7 categories, 50-agent simulation)

### Key technical decisions:

**WAL entry format** (DS1):
```
[4-byte length (LE u32)][bincode(TxFile)][4-byte CRC32][32-byte BLAKE3 chain hash]
```
Chain hash: `entry_n.hash = BLAKE3(entry_{n-1}.hash || content_hash)`. Corruption detection exact to entry boundary. O_APPEND for entries < 4096 bytes (kernel-atomic). Flock for larger entries.

**Group commit** (DS2): mpsc channel + commit thread. Drains queue every 5-50ms (adaptive: shorter under load, longer when idle). Single fsync per batch. At 50 agents: ~20 fsyncs/s instead of 50. DurabilityPermit pattern: CLI receives response only after fsync completes.

**PASSIVE checkpoint** (DS3): Background thread converts WAL entries to .edn files without blocking reads or writes. FULL checkpoint triggered by `braid harvest --commit` — ensures all .edn files exist before git commit. Interval configurable via `:config/checkpoint-interval-secs` datom (ADR-FOUNDATION-031).

**RwLock** (DS4): Reads take shared lock (unlimited concurrency, nanosecond acquisition). Writes submit to group commit channel and wait for response. At 50 agents with 50ms batch interval: exclusive lock held ~100us per batch = 0.2% contention.

**Recovery** (DS5): Three levels: Fast (checkpoint + WAL delta, O(1)+O(k)), Medium (checkpoint + edn delta, O(1)+O(F)), Slow (full edn rebuild, O(F) — existing path, always works).

### Phase 2 verification target:
- P99 write < 50ms at 50 concurrent agents
- P99 read < 500ms at 50 concurrent agents
- Zero stale cache reads under concurrent load
- All 2,016+ existing tests still pass

---

## Phase 3: WRITER-* (5 tasks, ~400 LOC)

Wire single pre_opened LiveStore to ALL commands. Eliminates redundant store loads in direct mode (when daemon not running).

### Execution order:

**Wave A**: Read `WRITER-SPEC t-abb1f8b5` — it has the exact Rust borrow pattern with NLL explanation, command-by-command wiring order, and strace verification methodology.

**Wave B** (parallel — each agent wires different command modules):
- `WRITER-2 t-ef33a18a` — Wire task commands (create/close/update/set)
- `WRITER-3 t-1670b3b8` — Wire harvest/seed/spec/write/challenge/extract/bilateral/session
- `WRITER-4 t-8900f57b` — Restrict session detection to pre_opened commands

**Wave C**: `WRITER-TEST t-075af7ae` — strace verification: exactly 1 store.bin open per CLI invocation. Full lifecycle test: init -> observe -> task create -> harvest -> seed -> status, each sees prior writes.

### The pattern (from WRITER-SPEC):
```rust
let mut fallback;
let live = match pre_opened {
    Some(l) => l,
    None => { fallback = LiveStore::open(path)?; &mut fallback }
};
```

---

## Phase 4: Audit WAVE 0 — Blocking Defects (5 tasks, ~200 LOC)

| ID | Task | File | Traces to |
|----|------|------|-----------|
| AUDIT-W0-001 | Move bootstrap_hypotheses.rs to CLI crate | bootstrap_hypotheses.rs | C8, lib.rs preamble |
| AUDIT-W0-002 | Inject now:u64 replacing SystemTime::now() | guidance.rs, task.rs | lib.rs preamble (determinism) |
| AUDIT-W0-003 | Fix store cache pipeline | layout.rs | INV-STORE-020 |
| AUDIT-W0-004 | Redefine F(S) monotonicity scope | spec/10-bilateral.md, bilateral.rs | INV-BILATERAL-001 |
| AUDIT-W0-005 | Handle retractions in MaterializedViews | store.rs observe_datom() | C1, INV-STORE-017 |

---

## Phase 5: Audit WAVE 1 — Policy Manifest / C8 Fix (6 tasks, ~500 LOC)

The highest-leverage architectural change in the project: make the kernel substrate-independent by wiring PolicyConfig into everything that currently hardcodes DDIS attributes.

| ID | Task | File | Traces to |
|----|------|------|-----------|
| AUDIT-W1-001 | F(S) weights from PolicyConfig.boundaries[].weight | bilateral.rs | C8, INV-FOUNDATION-007 |
| AUDIT-W1-002 | Trilateral namespace partitions from policy boundaries | trilateral.rs | C8, ADR-TRILATERAL-004 |
| AUDIT-W1-003 | Schema layers 1-4 to ddis.edn policy manifest | schema.rs | C8, ADR-FOUNDATION-013 |
| AUDIT-W1-004 | MaterializedViews observed namespaces from policy | store.rs | C8 |
| AUDIT-W1-005 | Spec_id element types from policy datoms | spec_id.rs | C8 |
| AUDIT-W1-006 | Harvest entity profiles from policy | harvest.rs | C8 |

---

## Phase 6: Audit WAVES 2-4 + Remaining (36 tasks, ~1250 LOC)

### WAVE 2 — Soundness Recovery (8 tasks):
Implement causally_independent(), fix LIVE view per-attribute resolution, guard NaN in Cheeger/Fiedler, wire validate_evolution into transact, add retraction existence check, add crystallization guard, fix Phi to use link-based traceability, close guidance feedback loop.

### WAVE 3 — Performance (5 tasks):
Deduplicate all_tasks() calls, use attribute_index range queries, intern Attribute strings, implement index-by-offset architecture, replace live_projections with MaterializedViews.

### WAVE 4 — Verification Completeness (6 tasks):
Update genesis attr count in spec, implement verify_semilattice(), add test-result ingestion, implement three-tier conflict routing, validate Value::Keyword on construction, replace EntityId::ZERO with Option.

### Remaining type/arch/spec/loop fixes (17 tasks):
TxId private fields, TaskId newtype, SpecId enum, BraidError structured variants, SchemaError split, Store initialization typestate, HarvestCandidate lifecycle typestate, remove guidance.rs re-exports, wire signal datoms into boundaries, seed upward dependency cleanup, CC-3 staleness tracking, DocumentedResidual type, FP/FN harvest calibration, seed demonstration density, merge intermediate state docs, witness cognitive independence, expect() safety documentation, CALM parse-time rejection.

---

## Execution Protocol

### For each task:

1. `braid go <task-id>` — claim it.
2. Read the full task description (`braid task show <task-id>`). Each contains: context, rationale, approach, acceptance criteria, file list, risks.
3. Implement following the three-box cleanroom protocol (black box -> state box -> clear box).
4. `cargo check --all-targets && cargo clippy --all-targets -- -D warnings`
5. Run relevant tests.
6. `braid task close <task-id>`.
7. If implementation reveals new issues: `braid observe` first, then continue.

### Parallelization:

Within each wave, launch parallel Opus 4.6 subagents with /effort max for tasks with **disjoint file sets**:
- Each subagent gets: task ID, task description, file list, acceptance criteria, and the quality exemplar.
- **Never** parallelize tasks touching the same file.
- After all subagents return: verify combined build, run full test suite, commit.

### Commit after each wave:

1. `cargo check --all-targets && cargo clippy --all-targets -- -D warnings`
2. `cargo test` (full suite — must be zero failures)
3. Stage only files this wave touched.
4. Commit with detailed message referencing task IDs and invariants.
5. `git push`
6. `braid harvest --commit`
7. `cargo build --release`

### Skill loading (one reasoning mode per phase):

| Phase | Skill | Mode | Shed When |
|-------|-------|------|-----------|
| 0 (Context) | None | Raw understanding | — |
| 2 (DAEMON-SCALE) | `rust-formal-engineering` | Type theory, state machines, concurrency | Phase 2 complete |
| 3 (WRITER-*) | None | Mechanical refactoring | — |
| 4-5 (Audit W0-W1) | `spec-first-design` | Formalize then implement | Phase 5 complete |
| 6 (Audit W2-4) | None | Apply absorbed patterns | — |

---

## Constraints

- **Zero-defect standard**: Every function has invariant references. Every new type has cardinality analysis. Every error path is tested. No unwrap() in library code.
- **No regressions**: The 2,016 existing tests must pass after every wave. The Iron Test must pass continuously.
- **No assumptions**: If a task's spec is ambiguous, stop and ask. If uncertain about an architectural choice, stop and ask.
- **No shortcuts**: A task marked done must fully satisfy its acceptance criteria. Incomplete work will be sent back.
- **Dogfood braid**: Use `braid observe`, `braid task`, `braid harvest` throughout.
- **Formal rigor**: Algebraic properties stated and verified. Proptest for algorithmic invariants. Stateright for protocol properties where applicable.
- **Sub-1s status**: Hard user requirement. No O(N) scans on the status hot path.
- Use /effort max for all subagents.

---

## Success Criteria

When all 66 tasks are complete:

1. `cargo test` — all tests pass (target: 2,200+).
2. `braid status` < 1s at 170K+ datoms.
3. 50-agent concurrent write test — all writes visible, zero stale reads.
4. `braid observe` through daemon < 5ms latency.
5. C8 compliance — kernel hardcoded methodology attributes replaced with policy-loaded references.
6. F(S) monotonicity — correctly scoped to bilateral operations.
7. MaterializedViews — handles retractions correctly.
8. Iron Test — passes 100/100 iterations under concurrent load.
