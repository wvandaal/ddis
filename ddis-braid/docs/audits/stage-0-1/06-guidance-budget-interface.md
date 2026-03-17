# Guidance + Budget + Interface — Stage 0/1 Audit
> Wave 1 Domain Audit | Date: 2026-03-17 | Agent: Opus 4.6 | Methodology: Fagan Inspection + IEEE Walkthrough

## Domain Inventory

### Spec Elements Catalog

**GUIDANCE (spec/12-guidance.md)**: 11 INV, 9 ADR, 3 NEG = 23 total
**BUDGET (spec/13-budget.md)**: 6 INV, 4 ADR, 2 NEG = 12 total
**INTERFACE (spec/14-interface.md)**: 10 INV, 10 ADR, 4 NEG = 24 total

**Domain total: 27 INV, 23 ADR, 9 NEG = 59 spec elements**

---

## Findings

### FINDING-001: M(t) has 4 components, spec requires 5

- **Severity**: MEDIUM
- **Type**: DIVERGENCE
- **Sources**: spec/12-guidance.md:376-442 (INV-GUIDANCE-008) vs crates/braid-kernel/src/guidance.rs:169-175
- **Evidence**: The spec defines M(t) as 5 independently measurable components with weights `(0.25, 0.20, 0.15, 0.25, 0.15)` summing to 1.0: `[transact_freq, spec_lang, query_div, harvest_q, guidance_c]`. The implementation uses 4 components with weights `[0.30, 0.23, 0.17, 0.30]`, omitting `guidance_compliance` (m5). The code comment at line 169 says "Stage 0 methodology adherence weights (renormalized without m5)." The spec does not authorize this renormalization for Stage 0 -- the spec says Stage 0 elements include INV-GUIDANCE-008 per the guide header (docs/guide/08-guidance.md:4).
- **Impact**: M(t) scores are computed on a different weighting basis than the spec specifies. Trend comparisons across stages will be incompatible when m5 is added at Stage 1.

---

### FINDING-002: GuidanceTopology comonadic structure is absent

- **Severity**: HIGH
- **Type**: UNIMPLEMENTED
- **Sources**: spec/12-guidance.md:15-26 (comonadic W(A)), docs/guide/08-guidance.md:27-44 (GuidanceTopology struct) vs crates/braid-kernel/src/guidance.rs (full file)
- **Evidence**: The spec defines guidance as a comonad `W(A) = (StoreState, A)` with `extract`, `extend`, and `duplicate` operations. The guide specifies concrete types: `GuidanceTopology`, `GuidanceNode` with `predicate: QueryExpr` and `actions: Vec<GuidanceAction>`. The implementation has none of these. There is no `GuidanceTopology` struct, no Datalog predicate evaluation per guidance node, and no `query_guidance()` function. Instead, the implementation uses a simpler rule-based `derive_actions()` function with hardcoded rules (R11-R18). The comonad test module at line 1787 tests determinism and basic properties but does not test actual comonadic structure (no `duplicate` or `extend`).
- **Impact**: The guidance system cannot learn from store state changes dynamically -- the topology that would allow Datalog-evaluated, graph-structured guidance decisions is missing entirely.

---

### FINDING-003: Dynamic CLAUDE.md generation missing INV-GUIDANCE-007 typestate pipeline

- **Severity**: HIGH
- **Type**: UNIMPLEMENTED
- **Sources**: spec/12-guidance.md:276-367 (INV-GUIDANCE-007), docs/guide/08-guidance.md:257-293 (five-stage typestate pipeline) vs crates/braid-kernel/src/ (searched)
- **Evidence**: INV-GUIDANCE-007 specifies a five-stage typestate pipeline: `MeasureDrift -> DiagnoseDrift -> SelectCorrections -> ValidateBudget -> Emit`. The guide specifies `ClaudeMdConfig` with `AmbientSection` (<=80 tokens) and `ActiveSection` with demonstration density constraints. An `agent_md` module exists (`braid_kernel::agent_md::generate_agent_md`), referenced in `crates/braid/src/commands/seed.rs:10`, but the INV-GUIDANCE-007 four formal constraints (constraint budget, ambient/active partition, demonstration density, effectiveness tracking after 5 sessions) are not enforced via typestate. There is no `MeasureDrift`, `DiagnoseDrift`, `SelectCorrections`, `ValidateBudget` pipeline in code.
- **Impact**: The dynamic CLAUDE.md generator cannot guarantee constraint budget compliance or demonstration density. Ineffective corrections are never pruned.

---

### FINDING-004: Guidance footer not injected into MCP tool responses

- **Severity**: HIGH
- **Type**: DIVERGENCE
- **Sources**: spec/12-guidance.md:136-170 (INV-GUIDANCE-001: every tool response), spec/14-interface.md:1137-1149 (NEG-GUIDANCE-001) vs crates/braid/src/mcp.rs:262-294 (tool_status output)
- **Evidence**: INV-GUIDANCE-001 states "every tool response includes a guidance footer." NEG-GUIDANCE-001's falsification condition is "A CLI command in agent mode returns output without the trailing guidance section." The MCP server's tool responses contain plain text without any guidance footer. For example, `tool_status` at line 279-293 returns a text block with datom count, transactions, entities, schema attributes, and frontier agents -- but no M(t) footer, no next action, and no INV references. The `build_command_footer()` function that injects footers in CLI mode (crates/braid/src/commands/mod.rs:1412-1423) is never called from the MCP path.
- **Impact**: Agents using the MCP interface receive no methodology steering, violating the anti-drift mechanism's coverage guarantee. Basin B capture is unmitigated through MCP.

---

### FINDING-005: MCP tool set diverges from spec (braid_guidance missing, braid_write/braid_observe not in spec)

- **Severity**: MEDIUM
- **Type**: DIVERGENCE
- **Sources**: spec/14-interface.md:225-267 (INV-INTERFACE-003: six tools), docs/guide/09-interface.md:214-228 (MCPTool enum) vs crates/braid/src/mcp.rs:112-232 (tool_definitions)
- **Evidence**: The spec and guide define exactly 6 MCP tools: `braid_transact`, `braid_query`, `braid_status`, `braid_harvest`, `braid_seed`, `braid_guidance`. The implementation exposes 6 tools: `braid_status`, `braid_query`, `braid_write`, `braid_harvest`, `braid_seed`, `braid_observe`. The differences: (1) `braid_transact` replaced by `braid_write` -- semantically equivalent but name differs from spec. (2) `braid_guidance` missing -- no MCP tool for methodology steering. (3) `braid_observe` not in spec -- added beyond spec surface. The test at mcp.rs:784 verifies the wrong set of tool names matches.
- **Impact**: The compile-time tool count enforcement that INV-INTERFACE-003 specifies (fixed-size array that forces a compile error on change) is not implemented. The spec's guarantee that guidance is accessible via MCP is broken.

---

### FINDING-006: MCP server does not use ArcSwap; reloads store from disk per call

- **Severity**: MEDIUM
- **Type**: DIVERGENCE
- **Sources**: spec/14-interface.md:225-267 (INV-INTERFACE-002: store loaded once), docs/guide/09-interface.md:113-147 (ArcSwap<Store> design) vs crates/braid/src/mcp.rs:617-625 (serve function)
- **Evidence**: The guide specifies `ArcSwap<Store>` with store loaded once at initialization and held for session lifetime (lock-free reads, atomic swaps on writes). The implementation at mcp.rs:264 `tool_status` calls `layout.load_store()?` on every tool call. The `serve()` function at line 624-625 opens the layout once but every tool dispatch re-reads the store from disk. There is no `ArcSwap`, no persistent `Store` held in memory, and no atomic swap pattern.
- **Impact**: Performance degradation on every MCP tool call (disk I/O). No snapshot isolation -- concurrent reads and writes are not atomic.

---

### FINDING-007: Budget manager not integrated into CLI command dispatch

- **Severity**: MEDIUM
- **Type**: GAP
- **Sources**: spec/13-budget.md:55-78 (MEASURE/ALLOCATE/PROJECT transitions), crates/braid-kernel/src/budget.rs:296-394 (BudgetManager) vs crates/braid/src/commands/mod.rs (command dispatch)
- **Evidence**: The `BudgetManager` struct is fully implemented in `budget.rs` with `measure()`, `allocate()`, `projection_level()`, and `guidance_level()`. However, the CLI command dispatch in `commands/mod.rs` only uses the budget context for guidance footer compression level (`budget_ctx.k_eff()` at line 1420). The `allocate()` method (precedence-ordered truncation) is never called from the command pipeline. No command output passes through `BudgetManager::allocate()` for content-level truncation. The `enforce_ceiling()` function exists but is also not called in the main command dispatch path.
- **Impact**: INV-BUDGET-001 (output budget as hard cap) and INV-BUDGET-002 (precedence-ordered truncation) are implemented at the library level but not wired into the actual CLI output path. Commands can emit unbounded output.

---

### FINDING-008: Layer 4.5 statusline bridge not implemented

- **Severity**: MEDIUM
- **Type**: UNIMPLEMENTED
- **Sources**: spec/14-interface.md:268-289 (INV-INTERFACE-004: Statusline Zero-Cost to Agent), spec/13-budget.md:80-87 (Budget source precedence, item 3: session state file `.ddis/session/context.json`)
- **Evidence**: The spec defines Layer 4.5 as a "persistent low-bandwidth agent-to-human signal" that provides real-time context consumption data via a session state file. INV-INTERFACE-004 specifies statusline "zero-cost to agent" -- the agent never needs to read it explicitly. The budget source precedence in spec/13-budget.md lists `.ddis/session/context.json` (from statusline hook) as priority 3. No file named `context.json` is written or read anywhere in the codebase. No statusline hook exists. The `Statusline` mentioned in `Information flow` at spec/14-interface.md line 23 is entirely absent from implementation.
- **Impact**: The budget system cannot get ground-truth context consumption from a statusline hook. It falls back to either the `--budget`/`--context-used` flags or defaults, reducing budget accuracy.

---

### FINDING-009: INV-GUIDANCE-006 (Lookahead via Branch Simulation) unimplemented

- **Severity**: LOW
- **Type**: UNIMPLEMENTED
- **Sources**: spec/12-guidance.md:254-275 (INV-GUIDANCE-006), docs/guide/08-guidance.md:64 (`lookahead: u8`)
- **Evidence**: INV-GUIDANCE-006 specifies "branch simulation lookahead" where guidance evaluates potential future states to recommend actions. The guide defines `query_guidance()` with a `lookahead: u8` parameter. Neither `query_guidance()` nor any lookahead computation exists in the codebase. NEG-GUIDANCE-002 (no lookahead branch leak) is therefore vacuously satisfied.
- **Impact**: Guidance operates purely reactively on current state. No forward-looking recommendations. Low severity because this is likely Stage 1+.

---

### FINDING-010: INV-GUIDANCE-011 (T(t) Topology Fitness) unimplemented

- **Severity**: LOW
- **Type**: UNIMPLEMENTED
- **Sources**: spec/12-guidance.md:622-717 (INV-GUIDANCE-011)
- **Evidence**: INV-GUIDANCE-011 specifies T(t) topology fitness, a metric measuring the guidance graph's structural quality. The code doc header at guidance.rs:22 lists it as implemented (`INV-GUIDANCE-011: T(t) topology fitness`), but there is no `T(t)` computation, no `TopologyFitness` struct, and no topology fitness metric in the code.
- **Impact**: The claimed coverage in the code header is inaccurate. The unified guidance function `G = M(t) x R(t) x T(t)` (ADR-GUIDANCE-005) lacks its third component.

---

### FINDING-011: Error protocol diverges from spec's RecoveryAction enum

- **Severity**: MEDIUM
- **Type**: DIVERGENCE
- **Sources**: spec/14-interface.md:405-456 (INV-INTERFACE-009), docs/guide/09-interface.md:354-370 (RecoveryAction enum) vs crates/braid/src/error.rs:1-135
- **Evidence**: The spec defines `RecoveryAction` as an enum with four variants: `RetryWith`, `CheckPrecondition`, `UseAlternative`, `EscalateToHuman`. The guide defines `RecoveryHint { action: RecoveryAction, spec_ref: &'static str }`. The implementation uses `ErrorInfo { what, why, fix, spec_ref }` where `fix` is a `&'static str`, not a typed `RecoveryAction` enum. The `fix` field is free-text rather than a structured variant. For example, at error.rs:73: `fix: "Check inputs and retry. Run 'braid status' for store state."` -- this is prose, not `RecoveryAction::RetryWith("...")`.
- **Impact**: NEG-INTERFACE-004 (no error without recovery hint) is satisfied in spirit (all errors have fix text) but the type-level totality guarantee specified by the enum-based design is absent. A new error variant could omit the fix without compiler enforcement.

---

### FINDING-012: `from_human()` bridge still used for several commands

- **Severity**: LOW
- **Type**: STALE
- **Sources**: docs/guide/09-interface.md:50-55 (INV-INTERFACE-001: every command supports three modes) vs crates/braid/src/commands/ (multiple files)
- **Evidence**: `CommandOutput::from_human()` creates a degenerate three-mode output where agent mode just echoes the human string. Found in: `status.rs:59,70` (verify mode, deep mode), `seed.rs:37,42` (json mode, human briefing mode), `task.rs:159,259`, `verify.rs:201`. These bypass the spec's requirement that agent mode has a structured three-part format (context <= 50 tokens, content <= 200 tokens, footer <= 50 tokens). The `from_human()` path at output.rs:175-183 produces an `AgentOutput` with empty context and footer.
- **Impact**: Agent consumers of these commands get unstructured prose instead of the navigative three-part format, reducing agent output quality.

---

### FINDING-013: INV-INTERFACE-005 (TUI Subscription Liveness) and INV-INTERFACE-006 (Human Signal Injection) unimplemented

- **Severity**: LOW
- **Type**: UNIMPLEMENTED
- **Sources**: spec/14-interface.md:288-330 (INV-INTERFACE-005, INV-INTERFACE-006)
- **Evidence**: INV-INTERFACE-005 specifies TUI subscription liveness: "human monitoring dashboard" with subscription-driven updates. INV-INTERFACE-006 specifies human signal injection via the TUI. No TUI exists in the codebase. No subscription mechanism exists.
- **Impact**: Low severity because TUI is explicitly tagged as Stage 2+ in the spec header. The spec element exists for completeness but implementation is not expected yet.

---

### FINDING-014: Attention decay formula diverges between spec and implementation at k < 0.3

- **Severity**: LOW
- **Type**: DIVERGENCE
- **Sources**: spec/13-budget.md:24-28 (attention_decay formula) vs crates/braid-kernel/src/budget.rs:409-417
- **Evidence**: The spec defines: `k < 0.3: (k / 0.3)^2`. The implementation defines: `k < 0.3: 0.5 * (k / 0.3)^2`. The implementation adds a 0.5 coefficient for C0 continuity at the boundary (k=0.3: linear gives 0.5, quadratic with 0.5 coefficient gives 0.5). The spec formula would give `(0.3/0.3)^2 = 1.0` at k=0.3, creating a discontinuity with the linear regime's value of 0.5. The code is mathematically correct; the spec has a continuity error.
- **Impact**: The spec's Level 0 formula is wrong (discontinuous at k=0.3). The code fixes this but does not document the correction as an ADR. Tests at budget.rs:543-548 verify the code's formula produces 0.5 at k=0.3.

---

### FINDING-015: Spec's four-part guidance footer structure not implemented as specified

- **Severity**: MEDIUM
- **Type**: DIVERGENCE
- **Sources**: spec/14-interface.md (cross-ref from spec/12-guidance.md INV-GUIDANCE-001: "four-part guidance footer: next action, methodology reminder, context pointer, harvest status") vs crates/braid-kernel/src/guidance.rs:468-483 (GuidanceFooter struct)
- **Evidence**: The audit prompt and spec reference a "four-part" footer: (1) next action, (2) methodology reminder, (3) context pointer, (4) harvest status. The `GuidanceFooter` struct has: `methodology` (M(t) score), `next_action`, `invariant_refs`, `store_datom_count`, `turn`, `harvest_warning`. The formatted output (line 492-533) produces a two-line format: `M(t): ... | Store: ... | Turn N` and `Next: action -- verify INV-XXX`. The "methodology reminder" and "context pointer" as distinct semantic sections do not appear in the formatted output.
- **Impact**: The guidance footer is informative but does not have the four-part structure the spec envisions. It conflates methodology into M(t) score and omits an explicit context pointer.

---

### FINDING-016: INV-INTERFACE-007 (Proactive Harvest Warning) threshold mismatch

- **Severity**: LOW
- **Type**: DIVERGENCE
- **Sources**: spec/14-interface.md:330-351 (INV-INTERFACE-007) vs crates/braid/src/commands/status.rs:123-129 (harvest warning thresholds)
- **Evidence**: The MCP module documents INV-INTERFACE-007 (line 24), but the status command's harvest warning uses hardcoded tx-count thresholds (>=15 OVERDUE, >=8 harvest?) at status.rs:123-129 rather than the Q(t)-based thresholds from INV-HARVEST-005. The `derive_actions_with_budget()` function in guidance.rs:866 does support Q(t)-based thresholds when a budget signal is available, but the status command's build_terse() at line 121 calls `derive_actions(store)` without any Q(t) parameter.
- **Impact**: Status command's harvest urgency display is heuristic-based rather than Q(t)-based, contrary to ADR-BUDGET-001 (Measured Context Over Heuristic).

---

## Quantitative Summary

### GUIDANCE (11 INV, 9 ADR, 3 NEG)

| Element | Status | Notes |
|---------|--------|-------|
| INV-GUIDANCE-001 | **Implemented** (partial) | CLI has footer injection. MCP does not. |
| INV-GUIDANCE-002 | **Implemented** | Footers reference INV/ADR/NEG IDs |
| INV-GUIDANCE-003 | **Implemented** | `modulate_actions()` adapts to drift |
| INV-GUIDANCE-004 | **Implemented** | Drift detection triggers at M(t) < 0.5 |
| INV-GUIDANCE-005 | **Implemented** (partial) | Observation staleness tracked, but no effectiveness tracking for guidance nodes |
| INV-GUIDANCE-006 | **Unimplemented** | No lookahead/branch simulation |
| INV-GUIDANCE-007 | **Unimplemented** (partial) | agent_md module exists but typestate pipeline missing |
| INV-GUIDANCE-008 | **Divergent** | 4/5 components implemented, weights differ |
| INV-GUIDANCE-009 | **Implemented** | 10 derivation rules, task generation works |
| INV-GUIDANCE-010 | **Implemented** | R(t) routing with 6 metrics |
| INV-GUIDANCE-011 | **Unimplemented** | T(t) topology fitness absent despite code claiming it |
| ADR-GUIDANCE-001 | **Unimplemented** | No comonadic topology |
| ADR-GUIDANCE-002 | **Implemented** | Basin competition model in code comments and tests |
| ADR-GUIDANCE-003 | **Implemented** (partial) | 6 anti-drift mechanisms present as rules, not as topology nodes |
| ADR-GUIDANCE-004 | **Implemented** | Spec-language in footers |
| ADR-GUIDANCE-005 | **Divergent** | M(t) and R(t) present, T(t) absent |
| ADR-GUIDANCE-006 | **Unimplemented** | No query-over-guidance-graph |
| ADR-GUIDANCE-007 | **Implemented** | Drift modulation at crisis/warning levels |
| ADR-GUIDANCE-008 | **Implemented** | Progressive footer enrichment |
| ADR-GUIDANCE-009 | **Implemented** | Degree-product betweenness proxy |
| NEG-GUIDANCE-001 | **Violated** | MCP responses have no footer |
| NEG-GUIDANCE-002 | **Vacuously satisfied** | No lookahead to leak |
| NEG-GUIDANCE-003 | **Unimplemented** | No effectiveness tracking, so no pruning of ineffective guidance |

**GUIDANCE totals**: Implemented: 7/11 INV (3 partial), 5/9 ADR (1 partial), 0/3 NEG fully enforced. Unimplemented: 3 INV, 3 ADR. Divergent: 1 INV, 1 ADR.

### BUDGET (6 INV, 4 ADR, 2 NEG)

| Element | Status | Notes |
|---------|--------|-------|
| INV-BUDGET-001 | **Implemented** (library only) | `enforce_ceiling()` exists but not wired into dispatch |
| INV-BUDGET-002 | **Implemented** (library only) | `allocate()` respects precedence, not called in dispatch |
| INV-BUDGET-003 | **Implemented** | Q(t) formula correct with continuity fix |
| INV-BUDGET-004 | **Implemented** | Footer compression by k_eff level works |
| INV-BUDGET-005 | **Implemented** | Command profiles classified correctly |
| INV-BUDGET-006 | **Implemented** (struct only) | `TokenEfficiency` exists, density monotonicity not enforced in pipeline |
| ADR-BUDGET-001 | **Implemented** (partial) | Q(t) path exists but default path uses tx-count heuristic |
| ADR-BUDGET-002 | **Divergent** | Implementation corrects spec's discontinuity, undocumented |
| ADR-BUDGET-003 | **Implemented** (conceptual) | Rate-distortion framework guides design, not explicitly computed |
| ADR-BUDGET-004 | **Implemented** | `TokenCounter` trait + `ApproxTokenCounter` (chars/4) |
| NEG-BUDGET-001 | **Not enforced** in dispatch | `enforce_ceiling` exists but not called on command output |
| NEG-BUDGET-002 | **Implemented** (library) | Proptest verifies precedence ordering in `allocate()` |

**BUDGET totals**: Implemented: 5/6 INV (3 library-only), 3/4 ADR (1 partial), 1/2 NEG (library only). Divergent: 0/6 INV, 1/4 ADR.

### INTERFACE (10 INV, 10 ADR, 4 NEG)

| Element | Status | Notes |
|---------|--------|-------|
| INV-INTERFACE-001 | **Implemented** | Three output modes (Json/Agent/Human) |
| INV-INTERFACE-002 | **Divergent** | MCP reloads store per call, not ArcSwap |
| INV-INTERFACE-003 | **Divergent** | 6 tools present but wrong set (missing guidance, has write/observe) |
| INV-INTERFACE-004 | **Unimplemented** | No statusline bridge |
| INV-INTERFACE-005 | **Unimplemented** | No TUI (Stage 2+) |
| INV-INTERFACE-006 | **Unimplemented** | No human signal injection (Stage 2+) |
| INV-INTERFACE-007 | **Implemented** (partial) | Harvest warning exists but uses heuristic, not Q(t) |
| INV-INTERFACE-008 | **Implemented** | Tool descriptions tested for quality |
| INV-INTERFACE-009 | **Divergent** | Four-part errors exist but `ErrorInfo` not typed enum |
| INV-INTERFACE-010 | **Implemented** | CLI/MCP both use kernel functions |
| ADR-INTERFACE-001..010 | Various | Most reflected in architecture |
| NEG-INTERFACE-001 | **Implemented** | All state in store |
| NEG-INTERFACE-002 | **Implemented** | MCP delegates to kernel |
| NEG-INTERFACE-003 | **Not verified** for MCP | MCP has no harvest warning at all |
| NEG-INTERFACE-004 | **Implemented** (soft) | All errors have fix text, not typed enum |

**INTERFACE totals**: Implemented: 5/10 INV (1 partial), ~7/10 ADR, 2/4 NEG. Unimplemented: 3/10 INV. Divergent: 2/10 INV.

---

## Domain Health Assessment

**Strongest aspect**: The Budget subsystem at the library level. `BudgetManager`, `attention_decay()`, `OutputPrecedence`, `GuidanceLevel`, `BudgetProjection`, `enforce_ceiling()`, `TokenCounter` trait, and `classify_command()` are all implemented, tested (including extensive proptest coverage), and aligned with the spec. The attention decay continuity fix (FINDING-014) is actually an improvement over the spec.

**Most concerning gap**: The MCP interface is a dead zone for anti-drift mechanisms. FINDING-004 (no guidance footer in MCP responses) combined with FINDING-005 (missing `braid_guidance` tool) means agents using the MCP pathway receive zero methodology steering. This directly undermines the basin competition model that the entire GUIDANCE namespace is built around. The spec's core claim -- that Basin B capture is prevented by continuous energy injection -- fails completely through MCP.

The second most concerning pattern is the "library but not wired" phenomenon (FINDINGS-007): the budget system's precedence-ordered truncation pipeline (`BudgetManager::allocate()`) and hard ceiling enforcement (`enforce_ceiling()`) are implemented and tested in isolation but never actually called from the CLI command dispatch path. This means INV-BUDGET-001 (output budget as hard cap) is satisfied in unit tests but not in production usage.
