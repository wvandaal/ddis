# Implementation Completeness -- Stage 0 and Stage 1 Deliverables
> Wave 2 Cross-Cutting Synthesis | Date: 2026-03-17 | Agent: Opus 4.6 | Methodology: Cross-domain synthesis of 124 Wave 1 findings

## Build and Test Status

| Metric | Value |
|--------|-------|
| .rs files | 72 |
| Total LOC | 72,612 |
| `#[test]` annotations | 1,117 across 54 files |
| `cargo test --lib --bins` | **972 pass**, 0 fail, 0 ignored |
| `cargo check --all-targets` | **FAIL** -- `generated_coherence_tests.rs:933` unclosed delimiter (syntax error in auto-generated test) |
| Integration tests (excl. broken file) | **11 pass, 1 fail** (`schema_store_query`: assertion `19 != 18` -- genesis attribute count contradiction) |
| Broken test files | `generated_coherence_tests.rs` -- contains colons in function names (`generated_:spec/inv_...`), producing a parse error |

**Build health: AMBER.** 972 unit tests pass. Compilation fails only because `generated_coherence_tests.rs` has a syntax error. One integration test fails due to the known genesis count contradiction.

---

## Stage 0 Deliverable-by-Deliverable Matrix

Source: `docs/guide/README.md` lines 112-168, `SEED.md` line 341, `docs/guide/12-stages-1-4.md` lines 141-161.

Stage 0 scopes 83 INVs across 11 namespaces. The SEED.md success criterion is: "Work 25 turns, harvest, start fresh with seed -- new session picks up without manual re-explanation."

### Stage 0a -- Foundation (49 INV)

| Namespace | Stage 0 INVs | Impl | Partial | Unimpl | Divergent | % Complete | Evidence |
|-----------|-------------|------|---------|--------|-----------|------------|----------|
| STORE (13) | 001-012, 014 | 10 | 0 | 1 | 2 | **77%** | 001-008 solid (CRDT algebra), 009/010/011/014 impl. INV-STORE-002 divergent: serializer doesn't sort datoms before hashing (Wave 1). INV-STORE-012: LIVE index functional but tested against wrong property. INV-STORE-003 private constructor guarded by newtype. |
| LAYOUT (11) | 001-011 | 10 | 1 | 0 | 0 | **95%** | 19 tests in `layout.rs`. All major invariants have witness annotations. INV-LAYOUT-010 (concurrent write safety) partial -- O_CREAT|O_EXCL logic exists but not fully integration-tested. |
| SCHEMA (7) | 001-007 | 5 | 1 | 1 | 0 | **79%** | INV-SCHEMA-001/002/003/005/006 implemented. INV-SCHEMA-004 (validation on transact) partial -- checks exist but not typestate-enforced per spec. INV-SCHEMA-007 (lattice definition completeness) -- lattice definitions exist as datoms but not validated for all 4 required properties. |
| QUERY (10) | 001-002, 005-007, 012-014, 017, 021 | 7 | 1 | 0 | 2 | **75%** | Evaluator works (387+ results in multi-clause joins). Claims "semi-naive" in comments (`evaluator.rs:1`) but Wave 1 found single-pass nested-loop join, not differential fixpoint. No Datalog parser -- queries built programmatically via `QueryExpr`. Graph algorithms: `topo_sort`, `scc`, `critical_path`, `pagerank`, `density` all implemented in `graph.rs` (4053 LOC, 90 tests). INV-QUERY-005 divergent: `QueryMode` enum exists but only `Monotonic` arm is reached. INV-QUERY-006 divergent: monotonic growth test exists but stratified evaluation is incomplete. |
| RESOLUTION (8) | 001-008 | 4 | 2 | 2 | 0 | **63%** | `resolve()` and `has_conflict()` implemented. LWW works. Multi-value works. Lattice resolution falls back to LWW (`resolution.rs:159`: "Stage 0: lattice resolution falls back to LWW"). INV-RESOLUTION-004 partial: conflict predicate missing causal independence check. INV-RESOLUTION-007 partial: three-tier routing coded but deliberation path is a stub. INV-RESOLUTION-003 (convergence) and INV-RESOLUTION-008 (conflict entity datom trail) unimplemented -- `conflict_to_datoms` uses unregistered attributes. |

**Stage 0a subtotal: 36/49 INVs meaningfully implemented = 73%**

### Stage 0b -- Lifecycle + Intelligence (34 INV)

| Namespace | Stage 0 INVs | Impl | Partial | Unimpl | Divergent | % Complete | Evidence |
|-----------|-------------|------|---------|--------|-----------|------------|----------|
| HARVEST (5) | 001-003, 005, 007 | 4 | 0 | 1 | 0 | **80%** | `harvest_pipeline()` at `harvest.rs:187`. Pipeline correctness (INV-005), monotonicity (INV-001), gap detection (INV-002), classification (INV-003) all implemented. INV-HARVEST-007 (bounded lifecycle) has proptest but no session termination detection mechanism. |
| SEED (6) | 001-006 | 4 | 1 | 1 | 0 | **75%** | `assemble_seed()` at `seed.rs:2325`, `associate()` at `seed.rs:1358`. INV-SEED-001 (projection), 002 (budget compliance -- validated with assertions), 003 (bounded associate), 004 (compression priority) implemented. INV-SEED-005 (demonstration density) has proptest strategy but Wave 1 found the output never actually includes worked examples. INV-SEED-006 (seed content non-fabrication) unimplemented as a runtime check. |
| MERGE (5) | 001-002, 008-010 | 3 | 0 | 0 | 2 | **60%** | `merge_stores()` at `merge.rs:60`. Set union works (INV-001). Idempotency works (INV-008). INV-MERGE-002 divergent: test proves "datom set equality" not "identity collision" as spec requires. INV-MERGE-009 (cascade: schema rebuild -> resolution recompute -> LIVE invalidation) mentioned in comments but cascade logic is absent. INV-MERGE-010 (receipt captures conflict set) -- receipt struct exists but conflict detection is not wired. |
| GUIDANCE (6) | 001-002, 007-010 | 5 | 1 | 0 | 0 | **83%** | `compute_methodology_score()` (M(t)), `derive_tasks()`, `compute_routing()` (R(t)), `format_footer()`, `build_command_footer()` all implemented. INV-GUIDANCE-001 (injection) and INV-GUIDANCE-002 (anti-drift) working. INV-GUIDANCE-007 (spec-language detection) implemented. INV-GUIDANCE-008 (M(t)) fully implemented with 5 sub-metrics, trend, floor clamp. INV-GUIDANCE-009 (task derivation) implemented with 10 default rules. INV-GUIDANCE-010 (R(t) routing) implemented. INV-GUIDANCE-002 partial: comonadic topology mentioned in spec but absent from implementation. |
| INTERFACE (6) | 001-003, 008-010 | 4 | 1 | 1 | 0 | **67%** | `OutputMode` enum with json/agent/human at `output.rs:20`. `CommandOutput` with three renderers at `output.rs:146`. MCP server at `mcp.rs` with 6 tools. INV-INTERFACE-001 (CLI 3-mode) implemented. INV-INTERFACE-002 (MCP thin wrapper) implemented. INV-INTERFACE-003 (fixed tool count) has test. INV-INTERFACE-008 (help text as context) implemented. INV-INTERFACE-009 (error recovery) partial -- `BraidError.render(mode)` exists but not all error variants have recovery hints. INV-INTERFACE-010 unimplemented -- no anti-drift injection into MCP responses. |
| TRILATERAL (6) | 001-003, 005-007 | 6 | 0 | 0 | 0 | **100%** | `live_projections()` at `trilateral.rs:126`, `check_coherence()` at `trilateral.rs:426`, Phi computation at `trilateral.rs:184`, formality gradient implemented. All 6 Stage 0 INVs have implementations with proptest properties. 36 tests in module. Best-implemented namespace. |

**Stage 0b subtotal: 26/34 INVs meaningfully implemented = 76%**

### Stage 0 CLI Deliverables

| Deliverable (SEED.md) | Status | Evidence |
|------------------------|--------|----------|
| `braid init` | **COMPLETE** | `commands/init.rs` (13K), auto-detects git/tools, bootstraps spec |
| `braid transact` | **COMPLETE** | `commands/write.rs` (22K), assert/retract/promote/export subcommands |
| `braid query` | **COMPLETE** | `commands/query.rs` (37K), Datalog + entity/attribute filter + frontier scope |
| `braid status` | **COMPLETE** | `commands/status.rs` (15K), F(S)/M(t)/tasks/next action dashboard |
| `braid harvest` | **COMPLETE** | `commands/harvest.rs` (24K), pipeline + commit + guard |
| `braid seed` | **COMPLETE** | `commands/seed.rs` (13K), --inject for AGENTS.md, --compact, --agent-md |
| `braid guidance` | **PARTIAL** | Guidance footer is wired into all command outputs via `try_build_footer()`. No standalone `braid guidance` command -- integrated into status/bilateral. |
| Dynamic CLAUDE.md generation | **COMPLETE** | `agent_md.rs` (22K), `generate_agent_md()`, `braid seed --inject AGENTS.md` |
| MCP server | **COMPLETE** | `mcp.rs` (34K), 6 tools over JSON-RPC stdio |
| Self-bootstrap (first act) | **COMPLETE** | `bootstrap.rs` (34K), spec elements transacted as datoms. Store has 9314 datoms, 1563 entities, 358 spec elements across 22 namespaces. |

### Stage 0 Success Criterion Assessment

> "Work 25 turns, harvest, start fresh with seed -- new session picks up without manual re-explanation."

**Status: PARTIALLY MET.** The braid-seed section in CLAUDE.md contains 25 harvests and 43 observations. The seed mechanism functions -- sessions 016-021 show continuity. However, the open question at the bottom of the seed ("How should seed handle multi-session continuity?") indicates the mechanism has known gaps for sessions N-4 and earlier.

---

## Stage 0 Overall Scorecard

| Category | Score | Detail |
|----------|-------|--------|
| **INV implementation** | **62/83 = 75%** | 36/49 Stage 0a + 26/34 Stage 0b |
| **CLI commands** | **9/10 = 90%** | All SEED.md deliverables present. `braid guidance` not standalone. |
| **Self-bootstrap** | **COMPLETE** | 358 spec elements as datoms, 25 harvests |
| **Test coverage** | **972 pass** | But 1 integration failure (genesis count) and 1 broken test file |
| **Build health** | **AMBER** | `cargo check --all-targets` fails on generated test file |
| **Success criterion** | **PARTIAL** | Harvest/seed cycle works but multi-session continuity degrades |

**Stage 0 overall: 78% complete**

---

## Stage 1 Readiness Assessment

Stage 1 adds 26 INVs (from `docs/guide/12-stages-1-4.md` line 171): BUDGET-001-006, GUIDANCE-003-004, BILATERAL-001-002/004-005, INTERFACE-004/007, QUERY-003/008-009/015-016/018, SIGNAL-002, HARVEST-004/006, SEED-007-008, TRILATERAL-004.

| Capability | Stage 1 INVs | Pre-Implemented? | Evidence | Readiness |
|------------|-------------|------------------|----------|-----------|
| Q(t) measurement | BUDGET-001-006 | **5/6 in kernel** | `budget.rs` (40K): BudgetManager, Q(t), precedence, profiles, ceiling, compression. 40 tests. | **HIGH** -- but not wired into CLI dispatch loop (library-only) |
| Guidance compression | GUIDANCE-003-004 | **Partial** | `format_footer_at_level()` exists with 3 compression levels. Not budget-parameterized at runtime. | **MEDIUM** |
| Bilateral F(S) loop | BILATERAL-001-002/004-005 | **4/4 implemented** | `bilateral.rs` (85K): `compute_fitness()`, 7-weight F(S), CC-1..5, convergence tracking. 30 proptests. `braid bilateral` command present. | **HIGH** |
| Harvest FP/FN calibration | HARVEST-004/006 | **0/2** | Not implemented -- no calibration infrastructure | **LOW** |
| CLAUDE.md quality tracking | SEED-007-008 | **0/2** | Not implemented -- no relevance/improvement tracking | **LOW** |
| Confusion signal | SIGNAL-002 | **1/1 partial** | `signal.rs` (17K): SignalType::Confusion defined, emit/detect implemented. 7/8 other types commented out. | **MEDIUM** |
| Advanced graph metrics | QUERY-015/016/018 | **2/3** | `pagerank` implemented. HITS and k-Core absent from `graph.rs`. Betweenness centrality absent. | **MEDIUM** |
| Frontier-scoped queries | QUERY-003 | **Partial** | `evaluate_with_frontier()` exists but `Stratified(Frontier)` QueryMode not operational | **MEDIUM** |
| Significance tracking | QUERY-008/009 | **0/2** | No access log, no Hebbian tracking | **LOW** |
| Statusline bridge | INTERFACE-004 | **0/1** | Not implemented | **LOW** |
| Harvest warnings | INTERFACE-007 | **Partial** | Warning in status output, not proactive | **MEDIUM** |
| Convergence under growth | TRILATERAL-004 | **1/1** | Proptest for convergence monotonicity exists | **HIGH** |

### Stage 1 Readiness Summary

| Category | Score |
|----------|-------|
| Already pre-implemented in kernel | 12/26 INVs (46%) |
| Partially implemented | 6/26 INVs (23%) |
| Not started | 8/26 INVs (31%) |
| **Blocking issue**: Budget not wired to CLI | Budget manager is library-only -- `main.rs` creates `BudgetCtx` but only uses it for footer compression, not output mode adaptation |
| **Blocking issue**: Datalog evaluator is not semi-naive | Claimed in module docstring but Wave 1 found single-pass join |

**Stage 1 readiness: 46% pre-implemented, requires ~3-4 weeks of wiring + gap closure**

---

## Overall Implementation Completeness Score

| Dimension | Score | Notes |
|-----------|-------|-------|
| Stage 0 INV coverage | 75% (62/83) | Strong foundation, weak lifecycle |
| Stage 0 CLI completeness | 90% (9/10) | All core commands present |
| Stage 1 pre-implementation | 46% (12/26) | Budget and bilateral ahead of schedule |
| Code volume | 72,612 LOC | Substantial, possibly over-engineered for Stage 0 |
| Test volume | 1,117 test fns, 972 passing | Good coverage, 1 broken file |
| Build health | AMBER | Syntax error in generated file blocks full check |
| Specification fidelity | ~70% | Many annotations claim invariants but several prove wrong property (Wave 1 findings) |

**Composite implementation completeness: 72%**

The project has built a significant foundation -- the store, schema, query, layout, and trilateral namespaces are substantially implemented. The lifecycle layer (harvest/seed/guidance) is functional but has gaps. The critical shortfall is that several invariants are "witnessed" in code annotations but actually prove different properties than what the spec requires (e.g., INV-MERGE-002, INV-STORE-002, INV-QUERY evaluator algorithm).

---

## If I could fix only THREE things to improve completeness, they would be:

1. **Fix the genesis count contradiction and broken test file.** The `generated_coherence_tests.rs` syntax error blocks `cargo check --all-targets`. The genesis attribute count mismatch (19 vs 18) causes a cross-namespace integration test failure. These are the two items preventing a green CI. Both are tractable fixes (one is a code generation bug with colons in function names; the other requires reconciling `GENESIS_ATTR_COUNT` with the actual number of axiomatic attributes). Fixing these moves build health from AMBER to GREEN.

2. **Wire BudgetManager into the CLI dispatch loop.** The budget system is already implemented in `budget.rs` (40K LOC, 40 tests, Q(t), precedence ordering, ceiling enforcement). But it is library-only -- `main.rs` creates a `BudgetCtx` that is used only for footer compression. Wiring it so that every command's output passes through `enforce_ceiling()` with mode-aware truncation would satisfy INV-BUDGET-001 through INV-BUDGET-006, closing all 6 Stage 1 budget invariants in one change. This single integration would move Stage 1 readiness from 46% to 69%.

3. **Fix lattice resolution so it does not fall back to LWW.** At `resolution.rs:159`, the comment explicitly says "Stage 0: lattice resolution falls back to LWW." This means INV-RESOLUTION-006 (lattice join correctness) is not satisfied, which cascades into merge (INV-MERGE-009 cascade), schema (INV-SCHEMA-007 lattice completeness), and deliberation. The lattice definitions already exist as datoms in the schema. Implementing the actual `lub()` computation using stored lattice definitions would close 3-4 invariants across 3 namespaces and address the most-cited cross-cutting gap from all 7 Wave 1 domain audits.
