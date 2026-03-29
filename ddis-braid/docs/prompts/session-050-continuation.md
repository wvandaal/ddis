# Braid Execution Prompt — Session 050: Performance-First + Completion

> **Scope**: 32 tasks across 4 phases. ~1,600 LOC.
> **Mandate**: Lab-grade, zero-defect, production-ready. Fully integrated.
> **Method**: Spec-driven, first-principles, property-verified.
> **Prior session**: 049 — DAEMON-SCALE + soundness + 78 integration tests.
> **Critical insight from Session 049**: The daemon infrastructure is architecturally
> correct but operationally blocked. `braid_status` takes ~10s at 181K datoms due
> to O(N) scans, exceeding the daemon's 10s read timeout. The CLI falls through to
> direct mode on every status call. **Wave 3 performance is the critical path.**
> Without it, DAEMON-SCALE delivers zero benefit for the most common command.

---

## Phase 0: Context Recovery (do this FIRST)

1. Run `braid seed --task "Performance-first: Wave 3 then verification"`
2. Read `SEED.md` (project soul), `CLAUDE.md` (hard constraints C1-C10)
3. Read `docs/prompts/session-047-continuation.md` — the 66-task master plan
4. Read `docs/testing/INTEGRATION_TEST_GAP_ANALYSIS.md` — 90-case test catalog
5. Run `ms load rust-formal-engineering -m --full`
6. Run `ms load spec-first-design -m --full`

**Checkpoint**: Before writing code, verify:
- `CARGO_TARGET_DIR=/data/cargo-target` (NOT /tmp — see Build Notes in CLAUDE.md)
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
- **Phase 6 Wave 2 (8/8)**: causally_independent (resolution.rs:236), LIVE per-attribute resolution (store.rs:1355), NaN guard (budget.rs:2274), validate_evolution in transact (store.rs:1775), retraction existence check (store.rs:1795), crystallization guard (harvest.rs:346,954), Phi link-based (trilateral.rs), guidance feedback loop functions (context.rs — see CRITICAL below)

### Key decisions
- DS2 runtime datoms go through CommitHandle (daemon.rs:1686), tool writes still use direct write_tx under write lock
- Read/write lock split: read tools use shared.read() (daemon.rs:1553), write tools use shared.write() (daemon.rs:1572)
- Read-only MCP tool variants: call_tool_read (mcp.rs:420), is_read_only_tool (mcp.rs:402)
- GROUP_COMMIT_INITIAL_INTERVAL_MS = 25 (daemon.rs:2090)
- Content-addressed task identity: generate_task_id_full hashes title+description+priority+type (task.rs:224)
- Daemon startup: enrichment (capability census + FEGH) deferred to background thread after socket bind, reducing startup from ~5s to ~1.5s

### Bugs found and fixed
- Task ID resolution failure: shell for-loop IFS parsing mangled task IDs, LWW overwrote correct values. Fixed with content-addressed identity (Option B) + duplicate guard.
- routing_from_store_graph_impact_beats_priority: dep_add_datom used title-only hash, create_task_datoms used full hash. Fixed by aligning to generate_task_id_full.
- Flaky timing tests (rwlock_concurrent_reads, scale_50_concurrent): resource contention under parallel test execution. Pass with --test-threads=4.

### CRITICAL: Two unwired systems

**1. Guidance feedback loop (ADR-FOUNDATION-014: close every loop)**
- Functions exist in context.rs: worst_metric_name, guidance_recommendation_datoms, measure_guidance_effectiveness, detect_ineffective_guidance (context.rs:2274+)
- compute_methodology_score_dampened exists (methodology.rs:342)
- Schema attributes exist: :guidance/recommendation, :guidance/given-at, :guidance/target-metric, :guidance/ineffective (schema.rs:2752)
- Tests pass for the functions in isolation
- BUT: No production code CALLS guidance_recommendation_datoms. The functions are defined but not wired into any CLI command, daemon dispatch, or build_command_footer path. Guidance recommendations remain ephemeral.

**2. Daemon routing for braid_status (the entire point of DAEMON-SCALE)**
- Auto-start works (daemon.rs:1083-1114). Socket appears in ~1.5s.
- Routing works: CLI connects to socket, sends JSON-RPC (strace confirmed).
- BUT: braid_status computation inside the daemon takes ~10s at 181K datoms (O(N) scans in guidance, bilateral, methodology). The read timeout is 10s (daemon.rs:1124). The response times out. try_route_through_daemon returns None. CLI falls through to direct mode (6.7s user CPU, 430MB RSS). The daemon infrastructure delivers zero benefit for the most common command.
- Root cause: O(N) full datom scans in the status pipeline. Wave 3 is the fix.

### Risks
- CARGO_TARGET_DIR is now /data/cargo-target (real disk, 529GB). cargo-sweep cron runs daily at 4am removing artifacts >1 day old. /tmp is no longer used.
- Agents MUST NOT run cargo commands (CLAUDE.md Agent Launch Protocol rule 3). Orchestrator is the single build/test authority.

---

## Execution Scope

### Phase A: Wave 3 — Performance (5 tasks, ~400 LOC) — THE CRITICAL PATH

This phase unblocks daemon routing for braid_status. Without it, DAEMON-SCALE
is an engine with no transmission. Every task directly reduces the O(N) cost
of the status hot path.

**Measure first**: Before any optimization, profile `braid_status` inside the
daemon to identify the actual bottlenecks. Use `eprintln!` timestamps or
`std::time::Instant` around each major phase (fitness computation, guidance
footer, methodology score, task counts, coherence). The Session 047 audit
identified these candidates but did not measure them. Do not optimize blindly.

**A1. Deduplicate all_tasks() calls in status** (~30 LOC)
- `braid status` calls `all_tasks()` 4x, each O(N) scan. Cache the result.
- File: commands/status.rs
- Traces to: PERF-002

**A2. Use attribute_index range queries everywhere** (~80 LOC)
- Replace `store.datoms().filter(|d| d.attribute == attr)` with `store.attribute_datoms(&attr)`
- Audit every O(N) scan in status, guidance, methodology, bilateral hot paths
- File: methodology.rs, guidance.rs, bilateral.rs, context.rs
- Traces to: PERF-003
- This is likely the highest-impact single task.

**A3. Intern Attribute strings** (~60 LOC)
- Attribute::from_keyword allocates a String per call. Intern via a global pool or arena.
- File: datom.rs
- Traces to: PERF-004

**A4. Index-by-offset architecture** (~120 LOC)
- Replace linear search in entity_datoms with offset-based lookup
- File: store.rs
- Traces to: PERF-005

**A5. Replace live_projections with MaterializedViews** (~100 LOC)
- live_projections recomputes trilateral ISP projections from scratch. MaterializedViews is incremental.
- File: trilateral.rs, store.rs
- Traces to: PERF-006

**After Phase A**: Re-measure `braid_status` through daemon. Target: < 1s.
If achieved, the daemon's 10s read timeout becomes generous instead of tight.

### Phase B: Prove it + Close loops (5 tasks, ~500 LOC)

Now that performance works, measure and verify. Also wire the last open loop.

**B0. Wire guidance feedback loop into production** (~50 LOC)
- In build_command_footer (context.rs) or daemon post-dispatch: call guidance_recommendation_datoms() and detect_ineffective_guidance()
- Wire ineffective_metric_names() into compute_methodology_score_dampened() at the M(t) callsite
- ACCEPTANCE: After 3+ stagnant cycles, :guidance/ineffective appears in store AND dampened weights are used
- Traces to: ADR-FOUNDATION-014

**B1. BENCH-1: 50-agent P99 latency benchmark** (~150 LOC)
- 50 threads x 20 writes through daemon socket, measure P99/P95/P50
- 50 threads x 100 reads through daemon socket, measure P99
- Assert: P99 write < 50ms, P99 read < 500ms
- File: tests/daemon_integration.rs
- Traces to: DS7, INV-DAEMON-012

**B2. BENCH-2: Sustained 60s load test** (~200 LOC)
- Daemon 60s, 10 agents, observe/query/harvest cycles
- WAL growth -> passive checkpoint -> full checkpoint -> WAL truncation -> SIGKILL -> recovery
- Assert: zero data loss
- File: tests/daemon_integration.rs
- Traces to: INV-DAEMON-006, DS3, DS5
- Depends on: B1

**B3. Verify daemon auto-start works end-to-end** (~50 LOC)
- Integration test: cold start (no socket), run braid status, verify it routed through daemon (check CLI CPU < 0.1s or check daemon.sock exists after)
- If auto-start timeout (3s) is still too short after Wave 3, increase to 5s
- Traces to: INV-DAEMON-011

**B4. Measure braid status < 1s at 170K+ datoms** (~50 LOC)
- Integration test or E2E script: init store, load 170K datoms, time braid status through daemon
- Assert: wall clock < 1s
- This is the spec target. Prove it.

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

**Performance work protocol (Phase A specific)**:
1. Profile BEFORE optimizing. Add timing instrumentation to identify actual bottlenecks.
2. Optimize the measured bottleneck, not the assumed one.
3. Measure AFTER. Verify the optimization delivered measurable improvement.
4. Remove timing instrumentation (or gate behind cfg(debug_assertions)).
5. Run full test suite — performance optimizations must not change behavior.

**Integration mandate**: No task is complete until its functionality is wired into the live system and reachable through real execution paths. Every feature must have unit tests (property-based where applicable), integration tests confirming wiring, and E2E test coverage. Explicitly reject: partial implementation presented as complete, isolated code with no live wiring, "follow-up will connect it later" unless that boundary is recorded in braid.

**Bug protocol**: observe -> crystallize -> task -> execute. Blocking bugs fix before proceeding. Non-blocking create braid task and continue.

**Subagent orchestration**: Parallel Opus 4.6 subagents for disjoint-file tasks. Never parallelize tasks touching the same file. Agents MUST NOT run cargo commands — edit files only. Orchestrator runs cargo check/clippy/test ONCE after all agents complete.

**Quality standard**: Production-ready Rust. No unwrap() in production code. Result everywhere. No panic paths. Spec-first. Zero known blocking correctness issues at handoff.

**Build environment**: `CARGO_TARGET_DIR=/data/cargo-target` (529GB real disk). cargo-sweep cron cleans artifacts >1 day old at 4am daily. Between phases: `rm -rf /data/cargo-target/debug/incremental/` to free ~8GB.

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
2. `braid status` < 1s at 170K+ datoms (measured through daemon, not direct mode)
3. P99 write < 50ms at 50 concurrent agents (BENCH-1)
4. P99 read < 500ms at 50 concurrent agents (BENCH-1)
5. 60s sustained load with zero data loss after SIGKILL (BENCH-2)
6. Zero O(N) scans on the status hot path (Wave 3)
7. All 78+ integration tests pass
8. Guidance feedback loop wired and producing datoms in production
9. `cargo clippy --all-targets -- -D warnings` — zero warnings
10. Daemon auto-start → route → respond cycle works end-to-end for braid status

---

## Stop Conditions

Stop and escalate to the user if:
- Spec ambiguity cannot be resolved from canonical sources
- A blocking bug requires a design decision outside agent scope
- Scope expansion beyond the defined phases
- Disk space exhaustion prevents compilation
- Any C1-C10 violation discovered in existing code
- Performance profiling reveals a bottleneck that requires architectural change (not just optimization)
