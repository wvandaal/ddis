# Harvest + Seed Lifecycle — Stage 0/1 Audit
> Wave 1 Domain Audit | Date: 2026-03-17 | Agent: Opus 4.6 | Methodology: Fagan Inspection + IEEE Walkthrough

## Domain Inventory

### Harvest (spec/05-harvest.md)

| Element | Total | Implemented | Unimplemented | Divergent |
|---------|-------|-------------|---------------|-----------|
| INVs | 9 | 6 | 1 | 2 |
| ADRs | 7 | 5 | 2 | 0 |
| NEGs | 3 | 2 | 0 | 1 |

### Seed (spec/06-seed.md)

| Element | Total | Implemented | Unimplemented | Divergent |
|---------|-------|-------------|---------------|-----------|
| INVs | 8 | 5 | 2 | 1 |
| ADRs | 7 | 5 | 1 | 1 |
| NEGs | 2 | 1 | 0 | 1 |

---

## Findings

### FINDING-001: HarvestCandidate struct diverges from spec interface

Severity: MEDIUM
Type: DIVERGENCE
Sources: spec/05-harvest.md:171-180 vs crates/braid-kernel/src/harvest.rs:60-74
Evidence: The spec defines `HarvestCandidate` with fields `id: usize`, `datom_spec: Vec<Datom>`, `weight: f64`, `extraction_context: String`, and `reconciliation_type: ReconciliationType`. The implementation defines `entity: EntityId`, `assertions: Vec<(Attribute, Value)>`, `rationale: String`, and omits `id`, `weight`, `extraction_context`, and `reconciliation_type` entirely.
Impact: The missing `weight` field means the commitment weight estimation described in the guide (docs/guide/05-harvest.md:358-364) cannot be computed. The crystallization guard (INV-HARVEST-006) in the code uses `stability_score()` (harvest.rs:699) which is based on session diversity and confidence, not on the spec-defined `weight`-gated check. The missing `reconciliation_type` means harvested knowledge cannot be classified by divergence type as required by the reconciliation taxonomy.

---

### FINDING-002: CandidateStatus::Rejected loses reason string

Severity: LOW
Type: DIVERGENCE
Sources: spec/05-harvest.md:196 vs crates/braid-kernel/src/harvest.rs:100-101
Evidence: Spec defines `Rejected(String)` with a reason. Implementation defines `Rejected` as a unit variant without a reason field. The spec comment on line 197 says "Rejected with reason (terminal state)."
Impact: Rejected candidates cannot carry their rejection rationale, which prevents FP/FN analysis from understanding why candidates were rejected (INV-HARVEST-004).

---

### FINDING-003: Observation staleness model specified but not implemented in harvest pipeline

Severity: MEDIUM
Type: GAP
Sources: spec/05-harvest.md:153-163 (ADR-HARVEST-005) vs crates/braid-kernel/src/harvest.rs (entire file)
Evidence: The spec defines observation staleness metadata (`:observation/source`, `:observation/timestamp`, `:observation/hash`, `:observation/stale-after`) and a freshness check during harvest (line 160-162). The harvest pipeline implementation has no code that reads or writes any `:observation/*` attributes. The `observation_staleness()` function exists in guidance.rs (line 1296) but operates on exploration entities (`:exploration/source`), not on the spec-defined `:observation/*` attributes. The harvest pipeline does not flag stale observations.
Impact: The harvest pipeline cannot distinguish fresh from stale observations. Stale observations flow through without freshness warnings, violating the spec's intent that "stale observations are surfaced as warnings" (ADR-HARVEST-005, spec:770).

---

### FINDING-004: ExternalizationAnnotation and ingest_annotations entirely absent from code

Severity: HIGH
Type: UNIMPLEMENTED
Sources: spec/05-harvest.md:561-577 (INV-HARVEST-009 Level 2) vs crates/braid-kernel/src/harvest.rs (entire file)
Evidence: The spec defines `ExternalizationAnnotation` struct and `ingest_annotations()` function at Level 2 (implementation contract). The guide repeats these at docs/guide/05-harvest.md:97-112. Neither the struct nor the function exists anywhere in the codebase. A grep for `ExternalizationAnnotation` across all .rs files returns zero results.
Impact: INV-HARVEST-009 is tagged as Stage 2 in the spec (line 476), so this is expected to be unimplemented at Stage 0. However, the guide lists it in the implementation checklist (docs/guide/05-harvest.md:549-550) without noting the staging, which could mislead implementers into thinking it should already exist.

---

### FINDING-005: INV-HARVEST-004 (FP/FN calibration) partially implemented, threshold adjustment absent

Severity: MEDIUM
Type: GAP
Sources: spec/05-harvest.md:337-361 vs crates/braid-kernel/src/harvest.rs:910-1008
Evidence: The `calibrate_harvest()` function exists (harvest.rs:924) and correctly computes precision, recall, F1, and MCC. However, this function is never called from the harvest pipeline or CLI. The spec's calibration rule (line 351-353: "FP_rate > threshold -> raise extraction confidence threshold") is not implemented. No code adjusts thresholds based on calibration results. The function appears to exist only for testing.
Impact: INV-HARVEST-004 is tagged Stage 1, so the absence of runtime calibration is expected. However, the guide says "data collection should start at Stage 0" (docs/guide/05-harvest.md:399) but the pipeline does not record accepted/rejected decisions as datoms that Stage 1 could retroactively analyze.

---

### FINDING-006: INV-HARVEST-005 proactive warning thresholds diverge between spec and implementation

Severity: HIGH
Type: DIVERGENCE
Sources: spec/05-harvest.md:372-384 vs crates/braid-kernel/src/guidance.rs:57-60,140-153
Evidence: The spec says (line 382-384): "Stage 0 simplification: warn at turn 20, imperative at turn 40" per ADR-HARVEST-007. The implementation in guidance.rs uses Q(t)-based thresholds: None > 0.6, Info [0.3, 0.6], Warn [0.15, 0.3], Critical < 0.15. Furthermore, the Q(t) thresholds themselves diverge from the spec: spec says "warn at Q(t) < 0.15, imperative at Q(t) < 0.05" (line 372-376), but the code adds two extra levels (Info at [0.3, 0.6] and Warn at [0.15, 0.3]) and makes Critical at < 0.15 rather than < 0.05. The spec's two-level system (warn/imperative) has become a four-level system (None/Info/Warn/Critical). The critical threshold in the code (< 0.15) differs from the spec's imperative threshold (< 0.05) by a factor of 3.
Impact: Harvest imperative fires at Q(t) < 0.15 in code vs Q(t) < 0.05 in spec. This means harvest imperatives trigger much earlier than specified. While this is the "conservative bias" direction (safer), it is still a spec-implementation divergence.

---

### FINDING-007: Drift score semantics differ between spec, kernel, and guide

Severity: MEDIUM
Type: MISALIGNMENT
Sources: spec/05-harvest.md:325 vs harvest.rs:304-308 vs docs/guide/05-harvest.md:117-119
Evidence: The spec defines drift_score as "|uncommitted observations| at harvest time" (an integer count). The kernel computes drift_score as a ratio: `count as f64 / total_knowledge as f64` (harvest.rs:307) which is a float in [0,1]. The guide acknowledges the type mismatch: "drift_score semantics differ by context -- HarvestSession.drift_score (u32) counts observations during the session; HarvestResult.drift_score (f64) measures gap magnitude |delta(t)| at harvest time" (guide line 117-119). The guide's claim that HarvestSession.drift_score is u32 is itself wrong -- the code stores it as Value::Double (harvest.rs:847).
Impact: The quality bands defined in the spec ("0-2 = excellent, 3-5 = minor, 6+ = significant" at spec:331) assume integer counts but the implementation produces fractional ratios in [0,1]. These bands are meaningless against the actual drift_score computation.

---

### FINDING-008: ReviewTopology enum exists in guide but not in kernel code

Severity: LOW
Type: UNIMPLEMENTED
Sources: spec/05-harvest.md:209-215, docs/guide/05-harvest.md:130-137 vs crates/braid-kernel/src/harvest.rs (entire file)
Evidence: The spec defines `ReviewTopology` with five variants (SelfReview, PeerReview, SwarmVote, HierarchicalDelegation, HumanReview). The guide repeats it. The enum does not exist in the kernel code. The harvest pipeline operates in implicit self-review mode -- there is no topology selection mechanism.
Impact: INV-HARVEST-008 is Stage 2, so this is expected. However, the `build_harvest_commit` function (harvest.rs:791) and the CLI harvest command both lack any topology parameter, meaning the data model for eventual topology support is absent.

---

### FINDING-009: INV-SEED-005 (Demonstration Density) only partially enforced

Severity: MEDIUM
Type: GAP
Sources: spec/06-seed.md:372-394 vs crates/braid-kernel/src/seed.rs:3855-3940
Evidence: The spec requires "at least one demonstration per constraint cluster when budget > 1000 tokens." The implementation has a proptest that verifies demonstration density (seed.rs:3862) but the actual `assemble()` function does not contain any constraint cluster detection or demonstration injection logic. The assemble function (seed.rs:1799-2320) adds spec entities as "backfill" (seed.rs:2059-2089) at Summary projection level, but this is structurally-central-entity backfill, not cluster-aware demonstration injection. There is no code that identifies constraint clusters or generates demonstrations for them.
Impact: Seeds produced for tasks governed by multiple related invariants will lack worked examples. The spec specifically states "A 30-token demonstration is worth approximately 10x its token cost in behavioral activation" (spec:386), making this a significant quality gap in seed assembly.

---

### FINDING-010: INV-SEED-006 (Intention Anchoring) not implemented per spec

Severity: HIGH
Type: DIVERGENCE
Sources: spec/06-seed.md:403-415 vs crates/braid-kernel/src/seed.rs:1799-2320
Evidence: The spec requires "active intentions pinned at pi_0 (full detail) regardless of budget pressure" and specifies a pre-allocation protocol: "Compute B_pinned = sum|intention_i at pi_0|... If B_pinned >= budget: emit BudgetExhaustedByIntentions signal" (spec:104-108). The `verify_seed()` function (seed.rs:2486-2496) checks for INV-SEED-006 only by verifying a Directive section exists, which is a necessary but grossly insufficient condition. The actual assemble function has no intention querying, no pi_0 pinning, and no BudgetExhaustedByIntentions signal. The Directive section contains guidance actions and task text, not pinned intentions.
Impact: Active intentions are not guaranteed to appear in the seed at any projection level, let alone pi_0. An agent starting a session will not see its active intentions unless they happen to score high enough in the general-purpose entity scoring. This defeats the purpose of intention anchoring.

---

### FINDING-011: INV-SEED-003 boundedness check uses wrong max_results constant

Severity: LOW
Type: DIVERGENCE
Sources: spec/06-seed.md:316-319 vs crates/braid-kernel/src/seed.rs:2461-2474
Evidence: The spec says "|result.entities| <= depth x breadth." The `assemble_seed()` function uses depth=3, breadth=25 (seed.rs:2328-2329), so max_results should be 75. But AssociateCue::Semantic computes max_results as depth * breadth = 75. The `verify_seed()` function hardcodes max_results = 50 (seed.rs:2463), which is the wrong value. Additionally, the AssociateCue::Explicit variant adds seeds.len() to depth*breadth in its max_results computation (seed.rs:112-113), which is not in the spec.
Impact: The verification check could falsely pass (actual = 75, check allows 50 would fail, but since entities are typically truncated to the max before verification, the real issue is that the hardcoded 50 doesn't match the actual cue parameters). The Explicit cue's looser bound is a spec gap.

---

### FINDING-012: SEED.md section 5 specifies 70%/85%/95% thresholds, spec uses Q(t) < 0.15/0.05

Severity: MEDIUM
Type: MISALIGNMENT
Sources: SEED.md:180 vs spec/05-harvest.md:121-131
Evidence: SEED.md says: "The system issues proactive harvest warnings at context consumption thresholds (70%, 85%, 95%)." Three thresholds. The spec translates this to two Q(t) thresholds: Q(t) < 0.15 for warning, Q(t) < 0.05 for imperative (spec:121-131). The implementation has four levels (None, Info, Warn, Critical). None of these three representations are aligned with each other. SEED.md's 70/85/95% are consumption percentages (so remaining is 30/15/5). Spec's Q(t) < 0.15 and < 0.05 somewhat correspond to remaining = 15% and 5%, but 30% (SEED's 70%) has no spec equivalent at all.
Impact: The SEED.md commitment to three warning thresholds is partially honored. The middle threshold (85% consumed / 15% remaining) is captured by the spec and implementation, but the first threshold (70% consumed / 30% remaining) is absent from the spec and only partially captured by the implementation's Info level at Q(t) in [0.3, 0.6].

---

### FINDING-013: SEED.md specifies 7-step session lifecycle; code has no lifecycle state machine

Severity: MEDIUM
Type: GAP
Sources: SEED.md:180 ("seven steps: orient, plan, execute, monitor, harvest, seed, handoff") vs crates/braid-kernel/src/harvest.rs, seed.rs (entire files)
Evidence: SEED.md describes a "20-30 turn lifecycle (seven steps: orient, plan, execute, monitor, harvest, seed, handoff)." INV-HARVEST-007 (spec:427-443) references this as a "bounded cycle: SEED -> work(20-30 turns) -> HARVEST -> conversation_end -> SEED." The implementation has no session lifecycle state machine. There is a `SessionContext` struct and session start/end detection via timestamps, but no step tracking (orient/plan/execute/monitor/harvest/seed/handoff) and no turn counting mechanism at the kernel level. The CLI status command (braid/src/commands/status.rs:325) has a simple tx-count threshold of 15 for harvest warnings.
Impact: The bounded lifecycle invariant (INV-HARVEST-007) is not mechanically enforced. Session length is not tracked at the kernel level. The turn-count proxy (ADR-HARVEST-007) is partially implemented via the guidance system's tx-count heuristic, but this counts transactions, not turns.

---

### FINDING-014: NEG-HARVEST-001 (No Unharvested Session Termination) cannot be enforced

Severity: HIGH
Type: UNIMPLEMENTED
Sources: spec/05-harvest.md:980-993 vs crates/braid-kernel/src/harvest.rs (entire file)
Evidence: NEG-HARVEST-001 requires: "Every session that ends with uncommitted observations MUST have issued at least one harvest warning before termination." The safety property is "there exists no session termination with drift_score > 0 and no harvest warning issued." There is no session termination detection in the codebase. The guidance system emits harvest warnings through footer injection (guidance.rs:547-548), but these are advisory -- there is no mechanism that detects session termination and verifies a warning was issued. The CLI harvest command does not check whether warnings were shown before allowing session end.
Impact: Sessions can terminate with unharvested knowledge and no prior warning. This is the most critical failure mode in the harvest lifecycle per SEED.md: "knowledge loss from unharvested sessions is the single most damaging failure mode" (ADR-HARVEST-007, spec:870).

---

### FINDING-015: Seed SeedOutput struct diverges significantly from spec and guide

Severity: MEDIUM
Type: DIVERGENCE
Sources: spec/06-seed.md:163-196, docs/guide/06-seed.md:27-37 vs crates/braid-kernel/src/seed.rs:188-199
Evidence: The spec defines `SeedOutput` with `context: AssembledContext`, `agent: AgentId`, `task: String`, `entities_discovered: usize`. The guide defines a flat five-field struct: `orientation: String, constraints: String, state: String, warnings: String, directive: String`. The implementation matches the spec's nested structure (SeedOutput wraps AssembledContext), not the guide's flat structure. The guide's `SeedOutput` and the spec's `SeedOutput` are different types with different fields. The guide also specifies functions `relevance_score()` and `compress_seed()` that do not exist in the code.
Impact: The guide is unreliable as an implementation reference for the seed module. An agent following the guide's API surface would write code that does not match the implementation.

---

### FINDING-016: generate_claude_md specified in spec but not in seed.rs

Severity: LOW
Type: DIVERGENCE
Sources: spec/06-seed.md:224-231 vs crates/braid-kernel/src/seed.rs (entire file) vs crates/braid-kernel/src/agent_md.rs
Evidence: The spec defines `generate_claude_md(store, focus, agent, budget) -> Result<String, SeedError>` as a free function in the seed module. The implementation places this in a separate `agent_md.rs` module as `generate_agent_md(store, config) -> AgentMdOutput` with a different signature (takes `AgentMdConfig` struct instead of individual parameters, returns a structured output instead of a string). The function name and module location differ from spec.
Impact: The spec's stated composition `SEED = assemble . query . associate` with `GENERATE-CLAUDE-MD` as part of the seed namespace is architecturally violated. The functionality exists but in a different location with a different interface.

---

### FINDING-017: NEG-SEED-002 (No Budget Overflow) check is approximate

Severity: LOW
Type: DIVERGENCE
Sources: spec/06-seed.md:826-831 vs crates/braid-kernel/src/seed.rs:2448-2459
Evidence: The safety property is "no ASSEMBLE output exceeding declared budget." The verify_seed function checks `seed.context.total_tokens <= budget` (seed.rs:2449). However, the total_tokens computation at line 2311 explicitly clamps: `let total_tokens = (overhead + state_tokens).min(budget)`. This means total_tokens is forced to be <= budget by construction, making the verification check tautological. The actual content emitted (orientation + constraints + state + warnings + directive) could exceed the budget in tokens if the fixed sections (orientation, directive, constraints, warnings) alone exceed the budget -- they are not compressed or truncated, only the state section is budget-bounded.
Impact: When the combined token cost of orientation, constraints, warnings, and directive exceeds the budget, the seed output actually exceeds the budget even though total_tokens reports compliance. This is a genuine INV-SEED-002 violation masked by clamping.

---

### FINDING-018: Harvest candidate confidence floor diverges from spec detection rules

Severity: LOW
Type: MISALIGNMENT
Sources: docs/guide/05-harvest.md:341-356 vs crates/braid-kernel/src/harvest.rs:209-211
Evidence: The guide specifies detailed confidence ranges by category (Decision: 0.6-1.0, Observation: 0.7-0.9, etc.). The implementation uses Fisher-Rao information-geometric scoring (harvest.rs:547-603) which produces confidence values based on namespace distribution proximity, sample size, identity bonus, and reference density. These are structurally different scoring models. The guide's "Signal Source" column (e.g., "Explicit decision language in conversation") describes heuristics that the implementation does not use.
Impact: Confidence values in practice will not match the guide's expectations. Decisions will not score 0.9-1.0 unless their namespace distribution closely matches the ideal distribution [0.6, 0.1, 0.1, 0.2]. The guide's confidence ranges are aspirational, not descriptive of the actual algorithm.

---

## Domain Health Assessment

**Strongest aspect**: The harvest pipeline's core architecture (EXTRACT -> CLASSIFY -> SCORE -> GAP-DETECT -> PROPOSE) is well-implemented with a sophisticated Fisher-Rao information-geometric scoring model. The seed assembly with PageRank-based entity scoring, five-part template output, and budget-aware projection levels is a substantial and working implementation. The crystallization guard provides genuine stability gating. The FP/FN calibration infrastructure exists even if unused at runtime. The harvest-seed round-trip cycle works end-to-end: `braid harvest --commit` persists rich narrative summaries that `braid seed` recovers into structured context.

**Most concerning gap**: INV-SEED-006 (Intention Anchoring) and NEG-HARVEST-001 (No Unharvested Session Termination) are the two most architecturally significant gaps. Intention anchoring is the mechanism that prevents goal dilution -- without it, the seed's Directive section carries generic guidance rather than the agent's committed intentions. Unharvested session termination detection is the safety net for the entire lifecycle -- without it, the system relies purely on agent discipline to harvest, which is exactly the failure mode DDIS was designed to eliminate. The proactive warning system (INV-HARVEST-005) is implemented via Q(t)-based guidance footers but with divergent thresholds from the spec, and without session termination detection, warnings alone cannot prevent knowledge loss.
