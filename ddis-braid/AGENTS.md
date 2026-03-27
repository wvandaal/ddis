# Braid

> Braid is a runtime for formal epistemology — not a software tool.
> It replaces the Go CLI (`../ddis-cli/`) with a system built from
> first principles on `SEED.md`. The Go CLI and its specs are reference
> material — not the codebase you extend.

---

## True North

DDIS is a formal epistemology — how shared knowledge grows and stays coherent
across observers, time, and domains. Braid provides the universal substrate:
append-only datom store, CRDT merge, boundary registry, fitness gradient,
extractor framework, and harvest/seed lifecycle.

**DDIS methodology is braid's first APPLICATION, not its identity** (C8).
The INV/ADR/NEG ontology, 7-component F(S), and witness protocol are one
epistemological policy among many. Others (research, compliance, manufacturing)
run on the same substrate via different policy manifests. The policy is datoms —
at `braid init`, a manifest is transacted and the kernel configures itself.

**One atomic operation at every level**: observe reality, compare to model,
reduce discrepancy. The 8-type divergence taxonomy (SEED.md §6) classifies
every way a model diverges from reality. Three learning loops close the system:
weight calibration (OBSERVER-4), structure discovery (OBSERVER-5), ontology
discovery (OBSERVER-6).

**The steering principle**: Braid is a navigation system for the LLM's pre-existing
knowledge manifold, not a knowledge store. The datom store contains trajectory (where
the agent has been), steering (where to go next), and calibration (steering accuracy).
The LLM builds an in-context model of braid from every token it encounters — commands,
flags, help text, outputs, errors, schema attributes, silences, cross-command patterns.
That model's quality bounds the agent's behavior. Braid is not a tool the LLM uses;
it is a language the LLM speaks. The API must be a coherent formal language that an
in-context learner converges on within 3-5 interactions. Inconsistencies (e.g.
`braid task list` vs `braid observe --list`) are parasitic constraints that degrade
reasoning across all subsequent turns. Command names, error messages, output structure,
and vocabulary must be grammatically, semantically, and structurally uniform across
the entire surface. A CLI response that returns only a receipt wastes the most powerful
steering moment in the interaction.
(Full theory: `docs/design/STEERING_MANIFOLD.md`)

**Substrate test**: "Would this make sense for a React project? A research lab?
A compliance team?" If no, it belongs in the policy layer, not the kernel.
Every change must close a loop, not open one (ADR-FOUNDATION-014).

---

## The One Rule

**Every session must leave the project in a better state than it found it.**

No speculative scaffolding, no aspirational stubs, no "TODO: implement later."
Every artifact committed must be complete within its scope, tested if code,
traceable to `SEED.md`. Reduce scope before reducing quality.

---

## Core Abstractions

| Term | Definition |
|---|---|
| **Datom** | `[entity, attribute, value, transaction, operation]` — atomic fact |
| **Store** | `(P(D), ∪)` — grow-only set of datoms. Merge = set union |
| **Transaction** | Entity carrying provenance (who, when, why, causal predecessors) |
| **Resolution** | Per-attribute: lattice-resolved, last-writer-wins, or multi-value |
| **Frontier** | All datoms known to a specific agent at a specific point |
| **Harvest** | End-of-session extraction of un-transacted knowledge into store |
| **Seed** | Start-of-session assembly of relevant knowledge from store |
| **Guidance** | Methodology pointer injected into tool responses |
| **F(S)** | Fitness: coverage, depth, coherence, completeness, formality. Target: 1.0 |

---

## Hard Constraints

Non-negotiable. Violating any is a defect regardless of other merits.

**C1: Append-only store.** Never delete or mutate. Retractions are new datoms (`op=retract`).

**C2: Identity by content.** Datom = `[e, a, v, tx, op]`. Same fact = same datom.

**C3: Schema-as-data.** Schema defined as datoms. Evolution is a transaction, not a migration.

**C4: CRDT merge = set union.** No heuristics at merge time. Conflict resolution is query-layer.

**C5: Traceability.** Every artifact traces to a spec element traces to a SEED.md goal. Orphans are defects.

**C6: Falsifiability.** Every invariant needs "violated if..." — no falsification condition = not an invariant.

**C7: Self-bootstrap.** Spec elements are the system's first data. The system's first coherence check is its own spec.

**C8: Substrate independence.** The kernel (`braid-kernel`) must not contain logic specific to
any language, test framework, spec format, or methodology — including DDIS itself. All
domain-specific behavior enters through plugin/extractor: registered as datoms, discovered
by query, invoked at runtime. **Test**: "would this make sense for a React project with Jest
and Jira?" If no, it belongs in application-layer, not kernel. **Falsification**: any kernel
function that imports, parses, or assumes a specific ecosystem or methodology.

**C9: Parameter substrate independence.** (INV-FOUNDATION-015) No hardcoded domain-specific
*values* — thresholds, weights, sizes, intervals. C8 = no hardcoded logic; C9 = no hardcoded
numbers. Every parameter follows: bootstrap default (compile-time fallback) → config override
(`:config/*` datom via `get_config`) → self-calibration (OBSERVER-4 learning loop).
**Falsification**: any kernel constant that fails the substrate test and lacks a `get_config`
override path.

---

## Critical Failure Modes

**NEG-001: No aspirational stubs.** `unimplemented!()` or `// TODO` = waste. Implement fully
or don't create the file. Partial implementations that compile but don't work are worse than
no implementation.

**NEG-002: No relitigating settled decisions.** Check `docs/design/ADRS.md` before proposing
alternatives to any design choice. ADRs exist to prevent re-derivation. Only revisit on
formal contradiction with another ADR or invariant.

**NEG-009: No "software tool" framing.** Features must generalize beyond software projects —
research labs, compliance teams, manufacturing. DDIS ontology (INV/ADR/NEG) is one policy;
don't hardcode it as the only one.

**NEG-010: No open loops.** Every datum the system creates or receives must connect to a
boundary evaluation, gradient computation, or calibration measurement. Open loops caused
every problem identified in Session 033. (ADR-FOUNDATION-014)

---

## Specification Methodology

Every spec element requires: **ID** (`INV-{NS}-{NNN}` / `ADR-{NS}-{NNN}` / `NEG-{NS}-{NNN}`),
**type**, **traceability** to SEED.md, **falsification condition**, and **uncertainty marker**
where applicable (confidence 0.0–1.0, plus what would resolve it).

```
### INV-STORE-001: Append-Only Immutability
**Traces to**: SEED.md §4 (Design Commitment #2)
**Type**: Invariant
**Statement**: The datom store never deletes or mutates an existing datom.
**Falsification**: Any operation that removes or modifies a datom's [e,a,v,tx,op] tuple.
**Verification**: After any operation sequence, datom count is monotonically non-decreasing
and all previously-observed tuples remain present.
```

---

## Session Protocol

**Start**: Read this file (especially the braid-seed section below for prior session context).
Or run `braid seed --task "your task"` for fresh context assembly.
Trace task to bedrock vision, convergence thesis (ADR-FOUNDATION-014), or specific INV/ADR.
Ask: "Does this close a loop or open one?" and "Would this work for a non-DDIS project?"

**During**: `braid observe "insight" --confidence 0.8` to capture knowledge.
Record failure modes in `docs/design/FAILURE_MODES.md` with FM-NNN IDs.
Document every design decision as an ADR.

**End**:
```bash
braid harvest --task "<task>" --commit   # Persist session knowledge
braid seed --inject AGENTS.md           # Refresh seed for next session
cargo check --all-targets && cargo clippy --all-targets -- -D warnings
cargo fmt --check && cargo test          # Quality gates
# Then: git add, git commit, git push
```

**Verification**: All new files complete (no stubs/TODOs). All spec elements have IDs,
traceability, falsification. No C1–C9 violations. No domain-specific logic in kernel
without going through policy/extractor pattern.

---

## Agent Launch Protocol (INV-TOPOLOGY-003)

Prevents build-lock contention and file overwrites (learned from Session 025).

1. **Commit before launch.** Agents' `cargo fmt` will overwrite uncommitted edits.
2. **Disjoint file sets.** Each agent edits DIFFERENT files. Same file = serialize, not parallelize.
3. **Agents don't build.** Edit only — no `cargo fmt/clippy/test`. Orchestrator verifies after.
4. **No worktrees.** Launch without `isolation: "worktree"`.
5. **Use `-q` flag.** `braid ... -q` suppresses guidance footer. Never `2>/dev/null`.

---

## Codebase Orientation

| Path | Purpose |
|---|---|
| `SEED.md` | Foundational design document — read after this file |
| `spec/README.md` | Master index for 21 namespace specifications |
| `crates/braid-kernel/` | Core: datom store, schema, query, harvest/seed, merge, guidance |
| `crates/braid/` | CLI: init, status, transact, query, harvest, seed, guidance, merge, log |
| `docs/design/ADRS.md` | Settled design decisions with rationale — check before relitigating |
| `docs/design/FAILURE_MODES.md` | Agentic failure mode catalog with acceptance criteria |
| `docs/guide/README.md` | Implementation guide index by namespace |
| `docs/audits/GAP_ANALYSIS.md` | Go CLI vs Braid mapping (ALIGNED/DIVERGENT/EXTRA/BROKEN/MISSING) |
| `docs/history/transcripts/journal.md` | 7 design session transcripts — read surgically, not whole |

**Sibling dirs** (reference only — do not port Go to Rust):
`../ddis-modular/` (meta-standard), `../ddis-cli-spec/` (97 INVs, 74 ADRs),
`../ddis-cli/` (Go impl, ~62.5K LOC). Consult when spec is ambiguous.

---

## Build Notes

`CARGO_TARGET_DIR=/tmp/cargo-target` — must `cp` binary after `cargo build --release`.

Stage 1 is ~85% complete. Stage 0 (spec, guide, gap analysis, kernel, CLI) is done.
Details in SEED.md §10.

---

## Dynamic Store Context

> Auto-generated by `braid seed --inject AGENTS.md`.
> Regenerate: `braid seed --inject AGENTS.md --task "your task"`

<braid-methodology>
<!-- Generated by braid. Do not edit manually. Regenerate: braid seed --inject AGENTS.md -->
<!-- Updated: 1774592366 | Store: 153602 datoms -->

## Methodology Gaps
- 143 observations with uncrystallized spec IDs → braid spec create
- 35 tasks with unresolved spec refs → crystallize first
- 278 current-stage INVs untested → add L2+ witness
- concentration: 3 traces in GUIDANCE — Review: braid task search INV-GUIDANCE
- concentration: 6 traces in STORE — Review: braid task search INV-STORE

## Ceremony Protocol (k*=0.6)
Standard: observe + execute → retroactive crystallize
For known-category bug fixes: execute-first OK if provenance chain exists after commit.

## Next Actions (R(t) pre-computed)
1. "RDI-5: Precedent query — find_resolution_precedent(store, " (impact=0.78) → braid go t-d9536d87
2. "SPECTRAL-TEST: Verification suite for approximate spectral. " (impact=0.63) → braid go t-f12bff46
3. "WALPHA-1: W-alpha (Working Set Alpha) implementation. Privat" (impact=0.34) → braid go t-56b8f57e

</braid-methodology>

<braid-seed>
<!-- Generated by braid. Do not edit manually. Regenerate: braid seed --inject AGENTS.md -->
<!-- Updated: 1774592366 | Store: 153602 datoms, 11890 entities -->

### Session Context

### Next Actions


Next actions:
  1. Harvest — 56 transactions since last harvest. Knowledge at risk of loss.
     run: braid harvest --task "<current task>" --commit
  2. Work — R(t) top: "RDI-5: Precedent query — find_resolution_precedent(store, attribute) -> Vec<Decision> for case law retrieval. BACKGROUND: The case law system enables querying past resolution outcomes for a given attribute. Uses AVET index on :resolution/attribute for O(1) lookup, then follows :resolution/decision Ref to load Decision entities. Results sorted by stability_score descending, then tx descending. The function is dual-purpose: (1) inform human reviewers with conflict history, (2) provide data for mode escalation signals. APPROACH: (1) Implement find_resolution_precedent(store, attribute) -> Vec<(EntityId, DecisionMethod, Value, TxId)>. Uses store.avet_lookup(:resolution/attribute, attribute_keyword) to find all resolution entities, then follows :resolution/decision Ref to load Decision details. (2) Add precedent_summary(store, attribute) -> PrecedentSummary struct with total_count, mode_distribution: BTreeMap<DecisionMethod, usize>, override_count, last_resolution_tx. (3) Wire into guidance: when R(t) computes impact for a task, attributes with high conflict history get a contention_boost. ACCEPTANCE: (A) Empty store returns empty vec. (B) After 5 resolutions of same attribute, returns all 5 sorted correctly. (C) PrecedentSummary mode_distribution sums to total_count. (D) AVET index is used (not linear scan). DEPENDS-ON: RDI-4. FILE: crates/braid-kernel/src/resolution.rs" (impact=0.78) — t-d9536d87
     run: braid go t-d9536d87

Protocol: observe → status → observe → harvest --commit | seed --inject AGENTS.md

Active intentions (3 tasks):
  [t-d9536d87] RDI-5: Precedent query — find_resolution_precedent(store, attribute) -> Vec<Decision> for case law retrieval. BACKGROUND: The case law system enables querying past resolution outcomes for a given attribute. Uses AVET index on :resolution/attribute for O(1) lookup, then follows :resolution/decision Ref to load Decision entities. Results sorted by stability_score descending, then tx descending. The function is dual-purpose: (1) inform human reviewers with conflict history, (2) provide data for mode escalation signals. APPROACH: (1) Implement find_resolution_precedent(store, attribute) -> Vec<(EntityId, DecisionMethod, Value, TxId)>. Uses store.avet_lookup(:resolution/attribute, attribute_keyword) to find all resolution entities, then follows :resolution/decision Ref to load Decision details. (2) Add precedent_summary(store, attribute) -> PrecedentSummary struct with total_count, mode_distribution: BTreeMap<DecisionMethod, usize>, override_count, last_resolution_tx. (3) Wire into guidance: when R(t) computes impact for a task, attributes with high conflict history get a contention_boost. ACCEPTANCE: (A) Empty store returns empty vec. (B) After 5 resolutions of same attribute, returns all 5 sorted correctly. (C) PrecedentSummary mode_distribution sums to total_count. (D) AVET index is used (not linear scan). DEPENDS-ON: RDI-4. FILE: crates/braid-kernel/src/resolution.rs (task, P2/medium, in-progress)
    Depends on: RDI-4: Wire resolution-to-decision into resolve_with_trail + cascade_full. BACKGROUND: resolve_with_trail(conflict, schema) -> ResolutionRecord already produces resolution datoms via conflict_to_datoms(). The bridge requires calling resolution_to_decision() and appending the Decision datoms to the same transaction. cascade_full() in merge.rs orchestrates the post-merge cascade; step 1 calls detect_conflicts + resolve, which should now also produce Decision entities. The key invariant: the Resolution datoms and Decision datoms MUST be in the same transaction (atomicity). APPROACH: (1) Extend resolve_with_trail to also return Decision datoms via resolution_to_decision(). (2) Extend conflict_to_datoms to include Decision+Position datoms. (3) Wire :resolution/decision Ref from the resolution entity to the Decision entity. (4) In cascade_full(), ensure the Decision datoms are included in the cascade output. ACCEPTANCE: (A) resolve_with_trail output includes Decision datoms. (B) Resolution entity has :resolution/decision Ref to Decision entity. (C) cascade_full includes Decision datoms in output. (D) All datoms in same tx (atomicity). DEPENDS-ON: RDI-3. FILE: crates/braid-kernel/src/resolution.rs + crates/braid-kernel/src/merge.rs (t-e401eaa4)
    Blocks: RDI-6: Mode escalation signals — detect override patterns and recommend resolution mode changes. BACKGROUND: When humans or higher-authority agents override automated resolutions, the pattern indicates the current mode is inappropriate. The escalation_signal function analyzes the precedent base for a given attribute and fires when the override rate exceeds 0.3. This is the self-tuning feedback loop: resolution→Decision, override→counter-Decision, detection→escalation signal→guidance gap→mode change recommendation. APPROACH: (1) Add :resolution/override-count (Long, One) to schema. (2) Implement escalation_signal(store, attribute) -> Option<EscalationSignal>. EscalationSignal: attribute, current_mode, recommended_mode, override_count, total_count, confidence. (3) Recommendation logic: override_rate > 0.3 AND current=LWW → Lattice; override_rate > 0.5 AND current=Lattice → Deliberation; override_rate < 0.1 → downgrade to LWW. (4) Wire into methodology_gaps() as a new gap category: resolution_escalation_count. (5) Wire into braid status: show 'resolution: N attributes need mode review' when escalation signals exist. ACCEPTANCE: (A) No overrides → no signal. (B) 4/10 overrides on LWW attribute → EscalationSignal with recommended=Lattice. (C) 6/10 overrides on Lattice → recommended=Deliberation. (D) 0/10 overrides → signal to downgrade. (E) Shows in braid status. DEPENDS-ON: RDI-5. FILE: crates/braid-kernel/src/resolution.rs + crates/braid-kernel/src/guidance.rs + crates/braid/src/commands/status.rs (t-c9645957)
  [t-f12bff46] SPECTRAL-TEST: Verification suite for approximate spectral. UNIT: (1) Empty store returns spectral_gap=1.0. (2) Single-partition store (all intent, no spec/impl) returns low gap. (3) Well-connected store (balanced ISP with cross-links) returns high gap. (4) Proptest: random stores, approximate gap within 10x of exact Fiedler (when computable). INTEGRATION: (5) braid status shows connectivity metric. (6) Fragmented store triggers guidance warning. (7) Threshold respects config datom override. DIAGNOSTICS: Log exact vs approximate gap, ISP partition sizes, cross-boundary count. DEPENDS-ON: SPECTRAL-1, SPECTRAL-2. TRACES: INV-SPECTRAL-010, INV-DIVERGENCE-009. (task, P1/high, in-progress)
    Depends on: SPECTRAL-2: Wire approximate spectral into guidance as Type 9 detector. CONTEXT: System computes convergence data and ignores it (Type 9 reflexive divergence) (t-455f1ee8), SPECTRAL-1: Add approximate_spectral_gap to MaterializedViews. CONTEXT: Exact eigendecomposition costs 110s and is never used for decisions. Cheeger inequality gives bounded approximation at O(1) (t-2594be01)
  [t-eeaf0160] TOPO-CALM: Implement CALM classification of task phases — Tier M (parallel) vs Tier NM (sequential barrier) (ADR-TOPOLOGY-002, INV-TOPOLOGY-006). BACKGROUND: The CALM theorem (Consistency As Logical Monotonicity) partitions operations: monotonic operations can execute without coordination (Tier M), non-monotonic operations require sync barriers (Tier NM). In the density matrix framework: Tier M = near-zero eigenvalues (independent work), Tier NM = strong eigenvalues (coupled work). For our agent coordination: editing code is Tier M (parallel, each agent edits different files), verification is Tier NM (cargo test/clippy requires all edits complete, sequential barrier). The phase plan is: Phase 1 (Tier M) = all agents edit in parallel, Phase 2 (Tier NM) = barrier (git commit + merge + cargo test), Phase 3 (Tier M) = continue with updated coupling. APPROACH: (1) Define CalmTier enum: MonotonicParallel, NonMonotonicBarrier. (2) classify_task_phase(task: &TaskSummary) -> CalmTier. Tasks involving only file edits = Tier M. Tasks involving verification, merge, schema change = Tier NM. (3) phase_plan(partition: &[Vec<EntityId>], tiers: &BTreeMap<EntityId, CalmTier>) -> Vec<Phase>. Groups consecutive Tier M tasks into parallel phases, inserts barriers before Tier NM tasks. ACCEPTANCE: (A) Pure edit tasks classified as Tier M. (B) Tasks with spec-refs to INV-MERGE-* classified as Tier NM. (C) Phase plan alternates M and NM phases. DEPENDS-ON: t-21af64fc (TOPO-SPECTRAL). FILE: crates/braid-kernel/src/topology.rs (task, P2/medium, closed)
    Depends on: TOPO-SPECTRAL: Implement spectral topology selection — Fiedler partition, Cheeger quality, topology pattern classification (INV-TOPOLOGY-005, ADR-TOPOLOGY-004 middle-end). BACKGROUND: The eigenstructure of rho_C directly determines the optimal topology pattern. The spectral gap Delta = lambda_1 - lambda_2 indicates how dominant the primary coupling cluster is. The Fiedler vector (2nd eigenvector of graph Laplacian L = D - C) gives the optimal binary partition. Recursive Fiedler partitioning gives the full cluster hierarchy. The Cheeger constant h(G) >= lambda_2/2 bounds partition quality. We already have fiedler(), cheeger(), symmetric_eigen_decomposition() in query/graph.rs. APPROACH: (1) Define TopologyPattern enum: Solo, Mesh, Star(hub), Pipeline, Hybrid(clusters). (2) select_topology(analysis: &CouplingAnalysis, agent_count: usize) -> TopologyPattern. Uses intrinsic spectral metrics: if p > 0.8 -> Parallel, if 0.3 < p <= 0.8 -> Hybrid (Fiedler partition), if p <= 0.3 -> Sequential/Star. (3) spectral_partition(coupling: &DenseMatrix, k: usize) -> Vec<Vec<usize>>. Recursive Fiedler bisection into k groups. (4) partition_quality(partition, coupling) -> f64 using Cheeger-like metric. ACCEPTANCE: (A) Identity coupling matrix -> Solo topology. (B) Block-diagonal coupling -> Hybrid with matching cluster count. (C) Fully coupled -> Mesh. (D) select_topology is deterministic (INV-TOPOLOGY-005). DEPENDS-ON: t-3f8df462 (TOPO-DENSITY). FILE: crates/braid-kernel/src/topology.rs (t-21af64fc)
    Blocks: TOPO-PLAN: Implement TopologyPlan struct and compute_plan() — the full compilation back-end (ADR-TOPOLOGY-004 back-end, INV-TOPOLOGY-005). BACKGROUND: This is the back-end of the topology compilation pipeline. It assembles all previous computations (coupling density matrix, spectral partition, CALM classification) into a complete execution plan. The plan includes per-agent task assignments, file sets (verified disjoint), seed commands, CARGO_TARGET_DIR assignments, phase ordering, and estimated F(T). The plan is a pure data structure — no IO, no agent management. It can be serialized to JSON for external orchestrators or displayed as human-readable text. APPROACH: (1) TopologyPlan struct with: agent_count, agents: Vec<AgentAssignment>, phases: Vec<Phase>, coupling_analysis: CouplingAnalysis, estimated_ft: f64. (2) AgentAssignment struct: agent_id: String, tasks: Vec<EntityId>, files: BTreeSet<String>, seed_command: String, cargo_target_dir: String, estimated_impact: f64. (3) compute_plan(store: &Store, agent_count: usize) -> TopologyPlan. Orchestrates: ready_task_files -> composite_coupling -> density_matrix -> spectral_partition -> calm_classify -> balance_assignment -> emit_plan. (4) balance_assignment: greedy bin-packing assigns highest-impact unassigned task to agent with lowest cumulative impact, subject to coupling constraints (coupled tasks to same agent). Uses R(t) impact scores from compute_routing. ACCEPTANCE: (A) All tasks assigned to exactly one agent. (B) File sets across agents are disjoint (no shared files). (C) Coupled tasks (rho_C > 0) assigned to same agent. (D) Impact variance across agents < 0.5 * mean_impact (balanced). (E) Plan is deterministic (INV-TOPOLOGY-005). DEPENDS-ON: t-eeaf0160 (TOPO-CALM), t-21af64fc (TOPO-SPECTRAL). FILE: crates/braid-kernel/src/topology.rs (t-57c4333f)
[Note: Intention context exhausts budget — other sections compressed]

### Quick Reference
```bash
braid status                           # Dashboard + next action
braid observe "..." --confidence 0.7    # Capture knowledge
braid harvest --commit                 # End-of-session extraction
braid seed --inject AGENTS.md          # Refresh this section
```
</braid-seed>
