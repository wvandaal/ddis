# Braid Execution Prompt — Session 050: Performance Verification + Hardening

> **Scope**: 32 tasks across 4 phases. ~1,600 LOC.
> **Mandate**: Lab-grade, zero-defect, production-ready. Fully integrated.
> **Method**: Spec-driven, first-principles, property-verified.
> **Prior session**: 049 — DAEMON-SCALE + soundness + 78 integration tests.

---

## Phase 0: Context Recovery (do this FIRST)

1. Run `braid seed --task "Performance verification + hardening"`
2. Read `SEED.md` (project soul), `CLAUDE.md` (hard constraints C1-C10)
3. Read `docs/prompts/session-047-continuation.md` — the 66-task master plan
4. Read `docs/testing/INTEGRATION_TEST_GAP_ANALYSIS.md` — 90-case test catalog
5. Run `ms load rust-formal-engineering -m --full`
6. Run `ms load spec-first-design -m --full`

**Checkpoint**: Before writing code, verify:
- `cargo check --all-targets` passes
- `cargo clippy --all-targets -- -D warnings` passes
- 1,694 kernel tests + 374 braid tests + 78 integration tests pass
- Git is clean on `main`, pushed to origin

---

## Session 049 Summary (bridge facts)

### Completed (36/66 tasks)
- **Phase 1**: DEFECT-003 (harvest task optional), DEFECT-006 (DW0b test)
- **Phase 2 DAEMON-SCALE (12/12)**: DS1 WAL (wal.rs 1459 LOC, 21 tests), DS2 group commit (CommitHandle + commit_thread, adaptive batching), DS3 checkpoint (passive + full, CheckpointSignal), DS4 RwLock dispatch (thread-per-connection), DS5 crash recovery (three-level: WAL/checkpoint/edn), DS6 integration wiring, DS7 scale verification (7 tests)
- **Phase 3 WRITER (3/5)**: WRITER-2/3/4 done. WRITER-SPEC and WRITER-TEST skipped.
- **Phase 4 Audit W0 (5/5)**: bootstrap_hypotheses moved, SystemTime injected, cache verified, F(S) monotonicity scoped, MaterializedViews retractions
- **Phase 5 Audit W1 (6/6)**: PolicyConfig extended with namespace_attributes, element_types, fitness_weights, isp_prefixes, harvest_category_weights
- **Phase 6 Wave 2 (8/8)**: causally_independent (resolution.rs:236), LIVE per-attribute resolution (store.rs:1355), NaN guard (budget.rs:2274), validate_evolution in transact (store.rs:1775), retraction existence check (store.rs:1795), crystallization guard (harvest.rs:346,954), Phi link-based (trilateral.rs), guidance feedback loop (context.rs)

### Key decisions
- DS2 runtime datoms go through CommitHandle (daemon.rs:1686), tool writes still use direct write_tx under write lock
- Read/write lock split: read tools use shared.read() (daemon.rs:1553), write tools use shared.write() (daemon.rs:1572)
- Read-only MCP tool variants: call_tool_read (mcp.rs:420), is_read_only_tool (mcp.rs:402)
- GROUP_COMMIT_INITIAL_INTERVAL_MS = 25 (daemon.rs:2090)
- Content-addressed task identity: generate_task_id_full hashes title+description+priority+type (task.rs:224)

### Bugs found and fixed
- Task ID resolution failure: shell for-loop IFS parsing mangled task IDs, LWW overwrote correct values. Fixed with content-addressed identity (Option B) + duplicate guard.
- routing_from_store_graph_impact_beats_priority: dep_add_datom used title-only hash, create_task_datoms used full hash. Fixed by aligning to generate_task_id_full.
- Flaky timing tests (rwlock_concurrent_reads, scale_50_concurrent): resource contention under parallel test execution. Pass with --test-threads=4.

### CRITICAL: Unwired guidance feedback loop
- Functions exist: worst_metric_name, guidance_recommendation_datoms, measure_guidance_effectiveness, detect_ineffective_guidance (context.rs:2274+)
- compute_methodology_score_dampened exists (methodology.rs:342)
- Schema attributes exist: :guidance/recommendation, :guidance/given-at, :guidance/target-metric, :guidance/ineffective (schema.rs:2752)
- Tests pass for the functions
- BUT: No production code CALLS guidance_recommendation_datoms. The functions are defined but not wired into any CLI command, daemon dispatch, or build_command_footer path. THE SUCCESSOR MUST WIRE THESE before claiming Wave 2 is complete.

### Deferred
- WRITER-SPEC (read spec), WRITER-TEST (strace verification)
- BENCH-1 (50-agent P99 benchmark), BENCH-2 (sustained 60s load test)
- Wave 3 (5 perf tasks), Wave 4 (6 verification tasks), 17 type/arch tasks
- /tmp disk fills at ~22GB during test compilation — clean cargo cache between builds

### Risks
- /tmp tmpfs is 32GB. Kernel + braid test binaries consume ~22GB. Parallel agent compilation causes disk exhaustion. Mitigate: serialize compilation, clean incremental cache.
- The guidance feedback loop being unwired means guidance recommendations are still ephemeral. This is the last open soundness item.

---

## Execution Scope

### Phase A: Wire guidance feedback + BENCH (3 tasks, ~400 LOC)

**A0. Wire guidance feedback loop into production path**
- In `build_command_footer` (context.rs) or the daemon's post-dispatch hook: call `guidance_recommendation_datoms()` to persist what was recommended, and `detect_ineffective_guidance()` to emit ineffective signals.
- Wire `ineffective_metric_names()` into `compute_methodology_score_dampened()` at the M(t) computation callsite.
- ACCEPTANCE: After 3+ guidance cycles with no improvement on a metric, the `:guidance/ineffective` datom appears in the store AND `compute_methodology_score_dampened` uses dampened weights.
- Verify by adding an integration test or extending existing tests.

**A1. BENCH-1: 50-agent P99 latency benchmark** (~150 LOC)
- 50 threads x 20 writes through daemon socket, measure P99/P95/P50
- 50 threads x 100 reads, measure P99
- Assert: P99 write < 50ms, P99 read < 500ms
- File: tests/daemon_integration.rs
- Traces to: DS7, INV-DAEMON-012

**A2. BENCH-2: Sustained 60s load test** (~200 LOC)
- Daemon 60s, 10 agents, observe/query/harvest cycles
- WAL growth -> passive checkpoint -> full checkpoint -> WAL truncation -> SIGKILL -> recovery
- Assert: zero data loss
- File: tests/daemon_integration.rs
- Traces to: INV-DAEMON-006, DS3, DS5
- Depends on: A1

### Phase B: Wave 3 — Performance (5 tasks, ~400 LOC)

All tasks trace to PERF findings from the Session 047 audit.

**B1.** Deduplicate all_tasks() calls in status (commands/status.rs, ~30 LOC)
**B2.** Use attribute_index range queries everywhere (methodology.rs, guidance.rs, bilateral.rs, ~80 LOC)
**B3.** Intern Attribute strings (datom.rs, ~60 LOC)
**B4.** Index-by-offset architecture (store.rs, ~120 LOC)
**B5.** Replace live_projections with MaterializedViews (trilateral.rs, store.rs, ~100 LOC)

### Phase C: Wave 4 — Verification Completeness (6 tasks, ~250 LOC)

**C1.** Update genesis attr count in spec (spec/05-schema.md, ~5 LOC)
**C2.** Implement verify_semilattice() (store.rs, ~80 LOC)
**C3.** Add test-result ingestion (extract.rs, ~60 LOC)
**C4.** Three-tier conflict routing (resolution.rs, ~50 LOC)
**C5.** Validate Value::Keyword on construction (datom.rs, ~20 LOC)
**C6.** Replace EntityId::ZERO with Option (datom.rs + callers, ~30 LOC)

### Phase D: Remaining (19 tasks, ~600 LOC)

D1-D2. WRITER-SPEC, WRITER-TEST
D3-D19. TxId private fields, TaskId newtype, SpecId enum, BraidError structured variants, SchemaError split, Store initialization typestate, HarvestCandidate lifecycle typestate, remove guidance.rs re-exports, wire signal datoms into boundaries, seed upward dependency cleanup, CC-3 staleness tracking, DocumentedResidual type, FP/FN harvest calibration, seed demonstration density, merge intermediate state docs, witness cognitive independence, expect() safety documentation, CALM parse-time rejection.

---

## Execution Protocol

For each task: select (highest-impact unblocked) -> mark in-progress via braid -> implement -> verify (cargo check + clippy + tests) -> mark complete via braid -> observe if new issues found.

**Integration mandate**: No task is complete until its functionality is wired into the live system and reachable through real execution paths. Every feature must have unit tests (property-based where applicable), integration tests confirming wiring, and E2E test coverage. Explicitly reject: partial implementation presented as complete, isolated code with no live wiring, "follow-up will connect it later" unless that boundary is recorded in braid.

**Bug protocol**: observe -> crystallize -> task -> execute. Blocking bugs fix before proceeding. Non-blocking create braid task and continue.

**Subagent orchestration**: Parallel Opus 4.6 subagents for disjoint-file tasks. Never parallelize tasks touching the same file. Each subagent tests its own work. Parent validates combined build after all complete.

**Quality standard**: Production-ready Rust. No unwrap() in production code. Result everywhere. No panic paths. Spec-first. Zero known blocking correctness issues at handoff.

**Disk management**: /tmp fills at ~22GB during test compilation. Between major builds: `rm -rf /tmp/cargo-target/debug/incremental/ /tmp/cargo-target/debug/deps/ /tmp/cargo-target/debug/build/` (requires user permission).

---

## Hard Constraints

- **C1**: Append-only store. Never delete or mutate datoms.
- **C2**: Content-addressed identity. EntityId = BLAKE3(content).
- **C4**: CRDT merge by set union. Commutative, associative, idempotent.
- **C8**: Substrate independence. Kernel must not hardcode DDIS methodology.
- **C9**: Parameter substrate independence. No hardcoded domain-specific values.
- **C10**: CLI coherence. Every command is a steering event.
- **INV-STORE-020**: After flush, store.bin = fold(txn_files).
- **INV-STORE-022**: No stale cache writes.
- **INV-DAEMON-004**: Semantic equivalence between daemon and direct mode.
- **INV-DAEMON-012**: Accept loop never blocks on dispatch.
- **`#![forbid(unsafe_code)]`** in braid-kernel.

---

## Success Criteria (10/10)

1. `cargo test` — 2,200+ tests, 0 failures
2. `braid status` < 1s at 170K+ datoms
3. P99 write < 50ms at 50 concurrent agents (BENCH-1)
4. P99 read < 500ms at 50 concurrent agents (BENCH-1)
5. 60s sustained load with zero data loss after SIGKILL (BENCH-2)
6. Zero O(N) scans on the status hot path (Wave 3)
7. All 78+ integration tests pass
8. Guidance feedback loop wired and producing datoms in production
9. `cargo clippy --all-targets -- -D warnings` — zero warnings
10. Every invariant in the spec has an L2+ test

---

## Stop Conditions

Stop and escalate to the user if:
- Spec ambiguity cannot be resolved from canonical sources
- A blocking bug requires a design decision outside agent scope
- Scope expansion beyond the defined phases
- Disk space exhaustion prevents compilation
- Any C1-C10 violation discovered in existing code
