# C9 Second-Order Epistemic Closure — Implementation Prompt

> **Usage**: Feed to a Claude Opus agent with access to `ddis-braid/`. The agent
> should have file read, search, shell execution, and edit capabilities.
> Expected deliverables: 5 tasks implemented, 52+ tests passing, zero regressions.

---

## Ground Truth (Internalize Before Anything Else)

**Braid is not a knowledge store. It is a navigation system for a pre-existing
knowledge manifold.** Every string braid emits is a steering vector on the LLM's
activation manifold. The datom store contains the trajectory (where the agent
has been), the steering (where it should go next), and the calibration data
(how accurate the steering is). The quality of braid IS the quality of its steering.

Read `docs/design/STEERING_MANIFOLD.md` in full. Then verify your understanding:

- What are the three components of every steering vector? (concept membership,
  structural gap, the question)
- Why is "recorded +9 datoms" the worst possible steering? (receipt vs navigation)
- What is the gold standard output from Section 2? (three lines: concept placement,
  coverage gap, targeting question)
- Why is the question at the end the most valuable token braid can emit?
  (it IS the acquisition function rendered as natural language)

---

## The Problem This Solves

An agent with 150K+ datoms of accumulated knowledge — including 226 audit
observations from 4 independent agents — cannot synthesize them. The agent
falls back to reading markdown files. This is the exact failure the harvest/seed
lifecycle was designed to prevent.

Root cause: the store has first-order knowledge (facts about the codebase) but
not second-order knowledge (facts about the structure of its own fact-base).
The concept engine clusters domain observations but not meta-observations.
The seed scores by keyword relevance, not convergence across independent sources.
No mechanism detects F(S) stagnation. No CLI command lists or searches observations.

This is C9: the system must apply its own coherence machinery to its own knowledge.

Two invariants formalize this:

- **INV-REFLEXIVE-006**: Second-Order Epistemic Closure — the system applies its own
  coherence machinery to its own observation layer using the same functions. Result
  surfaceable via single CLI command. Every observation retrieval command ends with
  a computed steering question.
- **INV-REFLEXIVE-007**: Fixed-Point Property — functions for second-order analysis
  are identical to first-order. No conditional branches for meta vs domain. 
  apply(f, apply(f, data)) == apply(f, data).

---

## What Already Exists (Do Not Rebuild)

Four kernel components were implemented in Session 047 and are DONE:

| Component | File | Tests | What it does |
|-----------|------|-------|-------------|
| C9-P1 | concept.rs | 6 | `find_agreement_clusters()` — cross-agent convergence via Jaccard |
| C9-P2 | concept.rs | 9 | `extract_observation_links()` — parse refs into datom links |
| C9-P3 | seed.rs | backward-compat | `score_entity()` with corroboration boost + `build_orientation()` convergent findings |
| C9-5 | methodology.rs | 4 | `detect_stagnation()` — F(S) plateau detection |

Total: 19 new tests, 1665 kernel tests passing, zero regressions.

Read the implementations before writing any code. Understand the types,
the insertion points, and the patterns used.

---

## What Must Be Built (5 Tasks, Dependency-Ordered)

### Phase 1 (Parallel — no interdependencies)

**C9-SPEC (t-442a7ddd)**: Formalize INV-REFLEXIVE-006 and INV-REFLEXIVE-007 in
`spec/22-reflexive.md`. Add to store via `braid spec create`. INV-REFLEXIVE-006
has 5 falsification conditions including: every observe subcommand output must
include a computed steering question.

**C9-P4 (t-266f06d5)**: Automatic per-process agent identity in
`crates/braid/src/commands/mod.rs`. Three-tier resolution: explicit `--agent` flag >
`BRAID_AGENT` env var > PID-derived `braid:pid-{PID}`. Substrate-independent. Apply
to observe command dispatch; session auto-detect in `main.rs` uses same resolved ID.

**C9-P6 (t-4807b60b)**: Concept display names via TF-IDF distinguishing keywords in
`crates/braid-kernel/src/concept.rs`. Replace hash-artifact names like
"different-threshold-this" with meaningful 3-5 word names. Algorithm: TF within
concept, IDF across all observations. Top 3 distinguishing keywords joined.

### Phase 2 (Depends on C9-P6)

**C9-P5 (t-ae7bab6f)**: Steering-optimized observe subcommands in
`crates/braid/src/commands/observe.rs` + `crates/braid-kernel/src/concept.rs`.

Four subcommands mirroring `braid task` API:
- `braid observe list` — grouped by concept, annotated with member count + confidence
  range. Ends with steering question: bridge-gap between top concept pair.
- `braid observe search PATTERN` — ranked by composite score (0.4 relevance + 0.3
  link count + 0.2 confidence + 0.1 recency). Ends with steering question.
- `braid observe show ENTITY` — full detail with incoming/outgoing links. Ends with
  steering question: most surprising unlinked neighbor.
- `braid observe recent N` — most recent with concept tags. Ends with steering
  question: coverage gap.

The steering question at the end of each command is the acquisition function
rendered as natural language. It is computed from `co_occurrence_matrix()`,
`frontier_recommendation()`, and `concept_inventory()`. Zero LLM calls.

**Critical design requirement**: `braid observe TEXT` must still work for
creation (backward compatibility). Use clap subcommands with positional fallback.

### Phase 3 (Depends on ALL above)

**C9-TEST (t-2fd51aac)**: 52 test checks across 4 layers:
- Layer 1: 28 unit tests (all_observations, subcommands, agent identity, concept
  naming, scoring proptests, steering question computation)
- Layer 2: 5 invariant verification tests (INV-REFLEXIVE-006 conditions a-e,
  INV-REFLEXIVE-007 fixed-point)
- Layer 3: 3 integration scenarios (proactive seed, reactive observe, fixed-point)
- Layer 4: 16 E2E checks in `scripts/e2e_c9_complete.sh` with structured
  diagnostic logging

---

## Execution Protocol

For each task:
1. `braid task update <id> --status in-progress`
2. Read the existing code at the insertion points. Understand the patterns.
3. Implement. Follow the approach in the task description exactly.
4. `cargo check --all-targets` — must compile with zero errors.
5. `cargo test -p braid-kernel` — all 1665+ tests must pass.
6. `braid task close <id> --reason "<what was done>"`
7. If implementation reveals new issues: `braid observe` first, then continue.

Phase 1 tasks edit DIFFERENT files — they can be parallelized across agents.
C9-P5 (Phase 2) edits concept.rs and observe.rs — serialize with Phase 1 concept.rs
work (C9-P6). C9-TEST (Phase 3) depends on everything.

---

## Constraints

- **`#![forbid(unsafe_code)]`** — the kernel crate forbids unsafe. No exceptions.
- **Pure computation in kernel** — no IO, no SystemTime, no filesystem in kernel code.
  All IO is in the `braid` CLI crate.
- **C8 compliant** — no DDIS-specific logic in C9 code. The observe subcommands work
  for any domain, not just software specification.
- **Backward compatible** — empty corroboration map produces identical seed scores.
  Single-agent stores show no degradation. `braid observe TEXT` creation unchanged.
- **Every output is a steering event** — no flat dumps, no receipts, no data without
  structure. Every command output must activate productive reasoning, not passive reading.

---

## Verification Checklist

After all 5 tasks complete:

- [ ] `cargo check --all-targets` — zero errors
- [ ] `cargo clippy --all-targets -- -D warnings` — zero warnings  
- [ ] `cargo test -p braid-kernel` — 1665+ tests, 0 failures
- [ ] `cargo test -p braid` — CLI tests pass
- [ ] `braid observe list` on live store shows grouped observations with steering question
- [ ] `braid observe search "AUDIT"` returns ranked results with steering question
- [ ] `braid observe show <entity>` shows full detail with links and steering question
- [ ] `braid seed --task "synthesize"` shows convergent findings (if multi-agent data exists)
- [ ] INV-REFLEXIVE-006 and 007 exist in `spec/22-reflexive.md` and in the store
- [ ] `scripts/e2e_c9_complete.sh` exits 0 with all 16 checks passing
- [ ] Zero regressions in any existing functionality

---

## The Standard

This is cleanroom software engineering. Lab-grade. Zero-defect.

Every type must satisfy `|Type| = |ValidStates|`. Every function must preserve
stated invariants. Every output must steer the agent toward the highest-information-
gain region of the knowledge manifold. The steering question at the end of every
observation command is the acquisition function — `alpha(action) = E[delta_F(S)]/cost`
— rendered as natural language. It is the most valuable token braid can emit.

The measure of success: an agent starting a fresh session can type
`braid observe search "audit findings"` and immediately understand what
the store knows, what converges across sources, and what to investigate next —
without reading a single markdown file.
