# Recommendation: The DDIS MCP Protocol Server

**Date:** 2026-03-01
**Prerequisite:** [Cleanroom Audit Round 3](CLEANROOM_AUDIT_R3_2026-02-28.md), [Universality Field Report](UNIVERSALITY_FIELD_REPORT_2026-02-28.md), [Next Steps: Prove Universality](NEXT_STEPS_UNIVERSALITY_2026-02-28.md)
**Method:** Deep-read analysis of meta-spec, CLI spec (10 modules, 112 invariants, 80 ADRs), full implementation (38K+ LOC, 45 commands), event-sourcing subsystems, universality field report findings, and all three cleanroom audit rounds
**Analyst:** Claude Opus 4.6

---

## EXECUTIVE SUMMARY

Build an MCP (Model Context Protocol) server that exposes the DDIS bilateral lifecycle as composable, structured tools for AI agents. This is not a convenience wrapper — it is the enforcement mechanism that makes the bilateral lifecycle work.

The bilateral lifecycle — DDIS's unique innovation, with zero precedent in the literature ([NEXT_STEPS_UNIVERSALITY_2026-02-28.md](NEXT_STEPS_UNIVERSALITY_2026-02-28.md), §Novel Concepts) — is currently a paper tiger. The universality field report proved this empirically: zero absorb invocations, zero crystallize events, and zero event-first authoring across two production projects ([UNIVERSALITY_FIELD_REPORT_2026-02-28.md](UNIVERSALITY_FIELD_REPORT_2026-02-28.md), Findings 4-6). The MCP server closes this gap by making the event-sourcing pipeline the *only* write path available to AI agents — the primary consumer the spec explicitly optimizes for (meta-spec §0.2, APP-INV-018, APP-ADR-008).

---

## PART I: THE DIAGNOSIS

### What Works (Universal)

The [Universality Field Report](UNIVERSALITY_FIELD_REPORT_2026-02-28.md) tested DDIS on two non-DDIS projects (rr-cli: Go CLI, rr-edge: Next.js platform) and established:

| Capability | Status | Evidence |
|-----------|--------|----------|
| Parse pipeline | Universal | Both projects: init → parse → index. APP-INV-001 (Round-Trip Fidelity) holds. |
| Validation | Universal | rr-cli: 18/19 pass, rr-edge: 17/19 pass |
| Coverage | Universal | rr-cli: 100%, rr-edge: 90% |
| Drift measurement | Universal | rr-cli: 0, rr-edge: 4 (correct — 4 invariants unparsed) |
| Search (BM25+LSI+PageRank) | Universal | Operates on index; domain-agnostic |
| Context bundles | Universal | 9-signal assembly is a pure function of index state |

Source: [UNIVERSALITY_FIELD_REPORT_2026-02-28.md](UNIVERSALITY_FIELD_REPORT_2026-02-28.md), Part II

### What Partially Works

| Capability | Status | Evidence |
|-----------|--------|----------|
| Witness recording | Partial | rr-edge: 14/14 witnessed. rr-cli: 0/13 — no adoption without prompting. |
| Challenge verification | Partial | rr-edge: 14/14 challenged (events exist), but challenge results lost on re-parse (Finding-2). |
| Annotation scanning | Partial | rr-edge: 9 formal `ddis:tests` annotations. rr-cli: 0 formal (106 informal APP-INV refs). |

Source: [UNIVERSALITY_FIELD_REPORT_2026-02-28.md](UNIVERSALITY_FIELD_REPORT_2026-02-28.md), Findings 2, 4, 7

### What Does Not Work

| Capability | Status | Evidence |
|-----------|--------|----------|
| `ddis absorb` (impl → spec) | **Zero usage** | 0 invocations across both external projects. 0 `impl_finding` events. |
| Event-first authoring (`ddis crystallize`) | **Zero usage** | 0 `invariant_crystallized` or `adr_crystallized` events in external projects. |
| Bilateral lifecycle (discover ↔ absorb) | **Unexercised** | Both projects operate as one-way pipeline: human writes spec → tool validates. |
| Triage convergence (`ddis triage --auto`) | **Never run end-to-end** | Code exists ([NEXT_STEPS_UNIVERSALITY_2026-02-28.md](NEXT_STEPS_UNIVERSALITY_2026-02-28.md), Appendix B), but never exercised on a real workflow. |

Source: [UNIVERSALITY_FIELD_REPORT_2026-02-28.md](UNIVERSALITY_FIELD_REPORT_2026-02-28.md), Findings 5, 6; [NEXT_STEPS_UNIVERSALITY_2026-02-28.md](NEXT_STEPS_UNIVERSALITY_2026-02-28.md), §Phase C

### Root Cause

The field report identifies the root cause explicitly:

> *"The event-first authoring workflow requires users to compose JSON on stdin and pipe it to `ddis crystallize`. This is a high-friction interface compared to editing markdown directly."*
>
> — [UNIVERSALITY_FIELD_REPORT_2026-02-28.md](UNIVERSALITY_FIELD_REPORT_2026-02-28.md), Finding-6

The bilateral lifecycle fails because the write interface is optimized for humans (CLI flags, JSON stdin, markdown editing) while the spec declares the primary consumer is LLMs. This is a contradiction within the system's own axiological commitments.

### Remaining Quality Gaps

From [Cleanroom Audit Round 3](CLEANROOM_AUDIT_R3_2026-02-28.md) and current tool output:

| Gap | Detail |
|-----|--------|
| 3 missing witnesses | APP-INV-103 (Witness Lifecycle Completeness), APP-INV-104 (Task Witness Enrichment), APP-INV-105 (CI Witness Gate) |
| 6 coverage gaps | APP-INV-110, 111, 112 each missing `validation_method` + `why_this_matters` |
| Check 11 fail | Proportional weight: 14 chapters with >20% deviation from mean |
| 3 confirmed bugs (V2) | Migration data loss risk, FK enforcement gap, snapshot race condition |

Source: [CLEANROOM_AUDIT_V2_2026-02-28.md](CLEANROOM_AUDIT_V2_2026-02-28.md), Part II (BUG-1, BUG-2, BUG-3); `ddis witness --check`, `ddis coverage`, `ddis validate`

---

## PART II: THE PRESCRIPTION

### Core Thesis

Build an MCP Protocol Server that exposes the bilateral lifecycle as structured tools for AI agents. When an agent's only write path to a DDIS spec goes through MCP tools, the bilateral lifecycle is enforced by construction:

```
Agent discovers   → ddis_discover tool   → event in Stream 1 (Discovery)
Agent crystallizes → ddis_crystallize tool → event in Stream 2 (Specification)
Agent absorbs     → ddis_absorb tool     → bilateral reconciliation
Agent witnesses   → ddis_witness tool    → proof in Stream 3 (Implementation)
Agent challenges  → ddis_challenge tool  → verification in Stream 3
```

No Edit tool on markdown. No direct SQLite writes. Only events. The event-sourcing pipeline (APP-INV-071: Log Canonicality, APP-INV-073: Fold Determinism, APP-INV-020: Event Stream Append-Only) becomes the only write path — not by policy, but by architecture.

### The Formal Structure: A Verified MDP

The MCP server exposes a Markov Decision Process with provably monotone reward:

```
State    : μ(S) = (open_issues, unspecified, drift) ∈ ℕ³
           F(S) = w₁·V + w₂·C + w₃·(1-D) + w₄·H + w₅·(1-K) + w₆·(1-I) ∈ [0,1]
Actions  : {discover, crystallize, absorb, witness, challenge, refine, triage_step}
Reward   : ΔF(S) = F(S') - F(S) ≥ 0   (APP-INV-069: monotone fitness)
Terminal : μ(S) = (0,0,0)  ⟺  F(S) = 1.0
```

Termination is provable, not empirical. The lexicographic ordering on N^3 is well-founded — there are no infinite descending chains (APP-INV-068: Fixpoint Termination, APP-ADR-054: Well-Founded Ordering). Any strategy — greedy, BFS, random — converges. The MCP server exposes the MDP; the agent provides the policy.

Source: `ddis-cli-spec/modules/triage-workflow.md`, `internal/triage/fitness.go`, `internal/triage/measure.go`

### Categorical Structure

The MCP server turns DDIS from a monolith into a cartesian closed category of spec operations:

```
Objects     : Spec states (SQLite + JSONL snapshots)
Morphisms   : MCP tools (typed transformations on spec state)
Product     : Parallel tool execution (independent tools compose)
Exponential : Resource-parameterized tools (curried over spec context)
Identity    : ddis_state (read current state without mutation)
```

The adjunctions from the meta-spec (APP-ADR-024: The Inverse Principle) map directly to MCP tool pairs:

```
ddis_discover   ⊣ ddis_absorb       (idea ↔ impl)
ddis_crystallize ⊣ ddis_validate    (write ↔ verify)
ddis_witness    ⊣ ddis_challenge    (attest ↔ verify)
ddis_context    ⊣ ddis_triage_step  (read ↔ act)
```

Tool composition is well-typed because JSON schemas enforce input/output compatibility. The event log is the operational semantics (reduction trace). SQLite is the denotational semantics (value space).

---

## PART III: WHY THIS AND NOT ALTERNATIVES

| Alternative | Why Not |
|------------|---------|
| Fix remaining gaps (3 witnesses, 6 coverage, Check 11) | Housekeeping. Doesn't address the bilateral lifecycle failure. These gaps get fixed automatically during MCP server development. |
| Run bilateral lifecycle on external project (previous recommendation) | Already done ([UNIVERSALITY_FIELD_REPORT_2026-02-28.md](UNIVERSALITY_FIELD_REPORT_2026-02-28.md)). Diagnosed the problem. Now we need treatment, not more diagnosis. |
| Build IDE integration (LSP) | Optimizes for human developers. But the primary consumer is LLMs (meta-spec §0.2). LSP doesn't enforce the bilateral pipeline. |
| Improve the CLI UX | Same problem — the CLI is human-facing. The bilateral lifecycle fails because humans prefer editing markdown. Polishing the CLI doesn't change this. |
| Build the triage agent WITHOUT MCP | Agent would use Bash to call `ddis` commands. This works but doesn't enforce the bilateral pipeline — agent could also `echo > file.md`. MCP tools are the enforcement mechanism. |
| Build agent protocol API first | Rejected by [NEXT_STEPS_UNIVERSALITY_2026-02-28.md](NEXT_STEPS_UNIVERSALITY_2026-02-28.md) at the time because integration was untested. The universality field report has now completed that testing. The diagnosis is in hand; building the interface is no longer premature. |

---

## PART IV: WHAT THE MCP SERVER FIXES

Every open gap is addressed simultaneously:

| Gap | How MCP Server Fixes It |
|-----|------------------------|
| Bilateral lifecycle unused (Finding-5) | Agents go through bilateral tools by construction — no markdown editing path |
| Event-first authoring not adopted (Finding-6) | Tools emit events; agents never touch markdown files |
| Zero absorb usage (Finding-4) | `ddis_absorb` is a natural tool call, not a manual CLI invocation requiring JSON on stdin |
| Challenge persistence lost on re-parse (Finding-2) | Building the server exercises and fixes the write-read cycle |
| 3 missing witnesses (APP-INV-103, 104, 105) | Implementing the server exercises the lifecycle paths these invariants specify |
| 6 coverage gaps (INV-110, 111, 112) | Completed during server specification phase |
| Check 11 proportional weight | New module chapters rebalance proportional distribution |
| 3 confirmed V2 bugs | The write path exercises and surfaces them during development |
| Universality gaps | The MCP server IS the interface that makes bilateral lifecycle work for the primary consumer |

Everything built so far becomes MORE valuable:

- All 45 commands become MCP tools or tool building blocks
- The 9-signal context bundles (APP-INV-005) become tool responses
- The triage MDP (APP-INV-068, APP-INV-069, APP-INV-070) becomes the agent's control loop
- The event-sourcing pipeline (APP-INV-071..097) becomes the only write path
- The consistency checking tiers (Tiers 1-5, `internal/consistency/`) become background processors
- The search intelligence (BM25+LSI+PageRank, APP-INV-008) becomes resource queries
- The witness ⊣ challenge adjunction (APP-ADR-037) becomes paired tool calls

Nothing is discarded; everything is composed.

---

## PART V: AXIOLOGICAL ALIGNMENT

Verified against the 8 axiological commitments from [NEXT_STEPS_UNIVERSALITY_2026-02-28.md](NEXT_STEPS_UNIVERSALITY_2026-02-28.md), §Axiological Commitments:

| Commitment | Alignment |
|-----------|-----------|
| 1. Determinism is sacred (APP-INV-002, APP-INV-073) | MCP tools are pure: same input → same event → same state |
| 2. Append-only everything (APP-INV-010, APP-INV-020) | Every tool call generates an event; no mutation without trace |
| 3. Provenance chains unbreakable (APP-INV-025, APP-INV-084) | MCP request IDs become event `causes` fields |
| 4. Duality as foundation (APP-ADR-024) | Read tools ⊣ Write tools form adjunctions (see §Categorical Structure) |
| 5. Formality enables freedom | MCP JSON schemas = the type system enabling flexible agent composition |
| 6. Observation over prescription (APP-ADR-018, APP-ADR-043) | Server classifies cognitive modes via APP-INV-034 (State Monad Universality); never mandates |
| 7. Self-reference escapes circularity | The MCP server is specified in DDIS and developed using DDIS |
| 8. Convergence is guaranteed (APP-INV-068) | Triage MDP terminates by well-founded ordering on ℕ³ |

All 8 satisfied.

---

## PART VI: THE PLAN

### Phase 0: Specify the Protocol Server in DDIS (Spec-First)

New module: `protocol-server` (domain: `protocol`).

**Invariants (~6):**

1. **MCP Tool ↔ CLI Command Correspondence** — Every bilateral write operation has exactly one MCP tool; every MCP tool maps to a well-defined CLI code path
2. **Tool Call Event Provenance** — Every MCP tool invocation generates an event in the appropriate stream with the MCP request metadata as `causes`
3. **Read-Only Resource Purity** — MCP resources are pure functions of spec state; no side effects on read
4. **Triage MDP State Oracle Correctness** — `ddis_state` returns (μ(S), F(S), frontier, convergence trajectory) consistent with `triage.ComputeFitness` and `triage.ComputeMeasure`
5. **Agent Identity ↔ Contributor Topology** — MCP client identity maps to contributor topology for provenance (APP-INV-030 graceful degradation applies)
6. **Tool Schema ↔ CommandResult Isomorphism** — MCP tool output schemas are isomorphic to the `CommandResult` triple `(output, state, guidance)` defined by APP-INV-034

**ADRs (~3):**

1. **MCP over stdio** — Not HTTP. Matches Claude Code / Codex / Gemini CLI native transport. Single-process, zero-network.
2. **Tool Granularity: One Tool per Bilateral Operation** — Not one tool per CLI command (45 tools would overwhelm). Bilateral operations (discover, crystallize, absorb, witness, challenge, refine, triage_step) plus read operations (state, progress, context, search, validate).
3. **Resource URIs for Spec Fragments** — `ddis://spec/{id}`, `ddis://invariant/{id}`, `ddis://frontier`, `ddis://fitness` — read-only MCP resources for agent consumption

**Process:** Author via `ddis discover` → `ddis crystallize`. Parse. Validate at 19/19. Witness. Challenge. Don't touch implementation until spec passes all gates.

### Phase 1: Build the MCP Server Core

The server wraps existing Go packages — minimal new logic required:

**MCP Tools:**

| MCP Tool | Wraps (existing package) | Direction |
|----------|------------------------|-----------|
| `ddis_state` | `internal/triage/fitness.go` + `measure.go` | Read (state oracle) |
| `ddis_progress` | `internal/progress/progress.go` (Analyze) | Read (frontier/blocked) |
| `ddis_context` | Context bundle assembly (9 signals, `internal/cli/context.go`) | Read (impl guide) |
| `ddis_search` | `internal/search/` (BM25+LSI+PageRank RRF) | Read (discovery) |
| `ddis_validate` | `internal/validator/` (19 checks) | Read (quality gate) |
| `ddis_discover` | `internal/discover/` (thread operations) | Write → Stream 1 |
| `ddis_crystallize` | `internal/cli/crystallize.go` | Write → Stream 2 |
| `ddis_absorb` | `internal/cli/absorb.go` (code→spec bridge) | Write (bilateral) |
| `ddis_witness` | `internal/witness/witness.go` (Record) | Write → Stream 3 |
| `ddis_challenge` | `internal/challenge/` (5-level verification) | Write → Stream 3 |
| `ddis_triage_step` | `internal/triage/protocol.go` (GenerateProtocol + steepest-descent action) | Write (autonomous) |

**MCP Resources:**

| Resource URI | Returns |
|-------------|---------|
| `ddis://spec/{id}` | Full spec metadata (title, version, modules, invariant/ADR counts) |
| `ddis://invariant/{id}` | Invariant definition + witness status + challenge verdict |
| `ddis://frontier` | Current frontier items (ready to work, sorted by authority) |
| `ddis://fitness` | F(S), μ(S), convergence trajectory, ranked deficiencies |

**Implementation approach:** Go MCP server using stdio transport. The `ddis serve` command starts the server. Configuration via `~/.claude.json` (Claude Code), `~/.codex/config.toml` (Codex), `~/.gemini/mcp.json` (Gemini CLI).

### Phase 2: The Triage Agent as First Client

The triage agent is a thin loop that consumes MCP tools:

```
while ddis_state().mu != (0,0,0):
    state  = ddis_state()
    front  = ddis_progress()
    target = front.frontier[0]           # highest authority, most unblocks
    bundle = ddis_context(target.id)     # 9-signal implementation guide

    [implement target using bundle]      # the agent's own reasoning

    ddis_witness(target.id, evidence)    # record proof
    ddis_challenge(target.id)            # verify proof

    if challenge.verdict == "refuted":
        ddis_discover("Refutation: " + target.id + ": " + challenge.detail)
```

This is not a new idea — it is exactly what APP-INV-068 (Fixpoint Termination) and APP-INV-070 (Protocol Completeness) already specify. The MCP server makes it executable.

Source: `ddis-cli-spec/modules/triage-workflow.md`, `internal/triage/protocol.go`

### Phase 3: Prove Universality

Use the MCP server + triage agent to drive **rr-cli** (the Go CLI project from the [field report](UNIVERSALITY_FIELD_REPORT_2026-02-28.md)) from its current state to fixpoint:

- rr-cli has 13 invariants, 7 modules, 22 test files, 0 witnesses, 0 absorb usage
- The triage agent should: absorb existing code patterns, witness invariants from passing tests, challenge witnesses, measure drift, converge
- Target: F(S) = 1.0, μ(S) = (0,0,0)

If convergence succeeds: DDIS has proven universality on an arbitrary object, not just the initial object. The triage endofunctor is globally contractive.

If convergence fails: the failure point is the most valuable diagnostic possible — it reveals exactly where the bilateral lifecycle breaks under real-world conditions.

### Phase 4: Self-Bootstrap Closure

The `protocol-server` module is developed using the MCP server itself. An agent connected to the DDIS MCP server authors the spec for the MCP server, implements it, witnesses it, and challenges it — closing the self-referential loop.

This satisfies APP-INV-067 (Self-Bootstrap Closure): the triage module processes its own implementation through the complete lifecycle.

---

## PART VII: WHAT COULD GO WRONG

1. **Go MCP SDK maturity** — The MCP ecosystem is TypeScript-first. Go MCP server libraries may be immature. Mitigation: the MCP stdio protocol is simple JSON-RPC; worst case, implement the protocol handler directly (~200 LOC for tool dispatch + resource serving).

2. **Tool granularity mismatch** — Too few tools and agents can't express fine-grained intent; too many and agents are overwhelmed. The proposed 11-tool surface area (6 write + 5 read) is calibrated to the bilateral lifecycle's natural operations. Adjust based on empirical agent behavior.

3. **The bilateral lifecycle might still not compose** — absorb → refine → drift → triage might hit the spec-first gate (TENSION-1 from [CLEANROOM_AUDIT_V2_2026-02-28.md](CLEANROOM_AUDIT_V2_2026-02-28.md)) in a way that blocks progress. This would force resolution of the tension. This is GOOD — it's exactly the forcing function needed.

4. **Agent behavior is non-deterministic** — Different LLMs will use the tools differently. The MDP's monotone reward guarantee (APP-INV-069) holds regardless of strategy, but convergence speed varies. The `ddis_state` oracle provides the feedback signal that enables adaptive strategies.

---

## PART VIII: THE META-POINT

This recommendation is itself an instance of the bilateral lifecycle. The universality experiment ([UNIVERSALITY_FIELD_REPORT_2026-02-28.md](UNIVERSALITY_FIELD_REPORT_2026-02-28.md)) was the diagnostic (discovery phase). This document crystallizes findings into an actionable specification (crystallization phase). Implementation will feed back into the spec (absorb phase). The return path — the thing DDIS claims is its unique innovation — is what generated this recommendation.

The prescription: build the interface that makes this return path work for all agents, not just for this analyst in this conversation.

No specification management tool has ever attempted this. The landscape analysis ([NEXT_STEPS_UNIVERSALITY_2026-02-28.md](NEXT_STEPS_UNIVERSALITY_2026-02-28.md), §External Landscape) shows:

- Kiro, spec-kit, Tessl treat specs as ephemeral documents
- TLA+, Alloy verify single algorithms, not system-wide specs
- No tool has a proven-convergent autonomous spec improvement loop
- No tool treats spec-as-source with bilateral lifecycle
- No tool exposes a verified MDP for specification-driven development

DDIS achieving this would be a genuine first in the field.

---

## PRIMARY SOURCES CONSULTED

### Project Artifacts (VCS)

| Document | Path | Key Findings Used |
|----------|------|-------------------|
| Cleanroom Audit Round 1 | [`.ddis/specs/CLEANROOM_AUDIT_2026-02-28.md`](CLEANROOM_AUDIT_2026-02-28.md) | 8 HIGH-severity bugs found via static analysis |
| Cleanroom Audit Round 2 | [`.ddis/specs/CLEANROOM_AUDIT_R2_2026-02-28.md`](CLEANROOM_AUDIT_R2_2026-02-28.md) | 6 bugs, FK migration, 4 new invariants |
| Cleanroom Audit V2 (formal) | [`.ddis/specs/CLEANROOM_AUDIT_V2_2026-02-28.md`](CLEANROOM_AUDIT_V2_2026-02-28.md) | 3 confirmed bugs (BUG-1/2/3), 3 spec tensions, 5 false positives rejected |
| Cleanroom Audit Round 3 | [`.ddis/specs/CLEANROOM_AUDIT_R3_2026-02-28.md`](CLEANROOM_AUDIT_R3_2026-02-28.md) | F-09/F-13/F-14 resolved, 3 invariants, 2 ADRs, module-level DAG |
| Next Steps: Prove Universality | [`.ddis/specs/NEXT_STEPS_UNIVERSALITY_2026-02-28.md`](NEXT_STEPS_UNIVERSALITY_2026-02-28.md) | Axiological commitments, landscape analysis, formal grounding, universality plan |
| Universality Field Report | [`.ddis/specs/UNIVERSALITY_FIELD_REPORT_2026-02-28.md`](UNIVERSALITY_FIELD_REPORT_2026-02-28.md) | 7 findings: bilateral lifecycle failure, parser gap, challenge persistence loss |

### Specification Modules (VCS)

| Module | Path | Invariants/ADRs Used |
|--------|------|---------------------|
| Meta-spec constitution | `ddis-modular/constitution.md` | State space model, quality gates, formal properties |
| CLI constitution | `ddis-cli-spec/constitution/system.md` | 8-tuple state space, 44 command transitions, non-negotiables |
| Auto-prompting | `ddis-cli-spec/modules/auto-prompting.md` | APP-INV-022..036, APP-ADR-024 (Inverse Principle), bilateral lifecycle |
| Event-sourcing | `ddis-cli-spec/modules/event-sourcing.md` | APP-INV-071..097, free monoid structure, fold homomorphism |
| Triage-workflow | `ddis-cli-spec/modules/triage-workflow.md` | APP-INV-063..070, contractive endofunctor, Lyapunov function |
| Code-bridge | `ddis-cli-spec/modules/code-bridge.md` | APP-INV-017..021, APP-INV-111, annotation grammar |
| Workspace-ops | `ddis-cli-spec/modules/workspace-ops.md` | APP-INV-037..040, APP-INV-112, progressive validation |
| Lifecycle-ops | `ddis-cli-spec/modules/lifecycle-ops.md` | APP-INV-006..016, transaction state machine, witness adjunction |

### Implementation Files (VCS)

| Package | Path | Purpose |
|---------|------|---------|
| Triage fitness | `ddis-cli/internal/triage/fitness.go` | F(S) computation, RankDeficiencies |
| Triage measure | `ddis-cli/internal/triage/measure.go` | μ(S) = (open, unspec, drift) |
| Triage protocol | `ddis-cli/internal/triage/protocol.go` | GenerateProtocol, Lyapunov tracking |
| Fold engine | `ddis-cli/internal/materialize/fold.go` | CausalSort, Apply, Fold, FoldWithProcessors |
| Snapshot | `ddis-cli/internal/materialize/snapshot.go` | CreateSnapshot, LoadLatestSnapshot, VerifySnapshot |
| Progress | `ddis-cli/internal/progress/progress.go` | Module-level DAG, SCC condensation, frontier analysis |
| Impl-order | `ddis-cli/internal/implorder/implorder.go` | Tarjan's SCC, Kahn's topological sort, phase assignment |
| Cascade | `ddis-cli/internal/cascade/cascade.go` | All 4 rel_types, role categorization |
| Witness | `ddis-cli/internal/witness/witness.go` | Record, Verify, Check, ValidDoneSet |
| Consistency | `ddis-cli/internal/consistency/` | 5-tier contradiction detection (graph, SAT, heuristic, SMT, LLM) |
| LLM provider | `ddis-cli/internal/llm/provider.go` | AnthropicProvider (net/http), graceful degradation |
| Causal DAG | `ddis-cli/internal/causal/dag.go` | Merge (CRDT), Bisect, Provenance |
| Events | `ddis-cli/internal/events/` | 3-stream JSONL, append-only, 28+ event types |

### Tool Output (Live)

| Command | Result | Date |
|---------|--------|------|
| `ddis validate manifest.ddis.db` | 18/19 pass (Check 11 fail: proportional weight) | 2026-03-01 |
| `ddis coverage manifest.ddis.db` | 98% (109/112 INV, 80/80 ADR); 6 gaps in APP-INV-110/111/112 | 2026-03-01 |
| `ddis drift manifest.ddis.db` | 0 (aligned) | 2026-03-01 |
| `ddis witness --check manifest.ddis.db` | 109/112 valid (97%); missing: APP-INV-103, 104, 105 | 2026-03-01 |
| `ddis impl-order manifest.ddis.db` | 3 phases (43/34/35 elements) | 2026-03-01 |

---

*Recommendation developed 2026-03-01. Analyst: Claude Opus 4.6.*
*Based on: 4-agent parallel deep-read (meta-spec, CLI spec, implementation, event-sourcing subsystems) + synthesis against universality field report findings + live tool state verification.*
