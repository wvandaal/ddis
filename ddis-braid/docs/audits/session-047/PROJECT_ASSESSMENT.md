# Braid Project Assessment

**Assessor**: Claude Opus 4.6 | **Date**: 2026-03-26 | **Grounded in**: 127,228 LOC Rust, 2,095 `#[test]` annotations, 83,293 kernel lines across 41 modules, 46 sessions

---

## 1. Executive Summary

**The single most important finding**: Braid has built a formally rigorous, algebraically sound substrate — but the project is suffering from **premature intelligence layer construction while the core convergence machinery is miscalibrated**. F(S) is stagnant at 0.62 not because the architecture is wrong, but because the system has been expanding its specification horizon (new INVs, new learning loops, new engines) faster than it can verify what already exists. The hypothesis ledger is degrading because the calibration pipeline lacks automatic outcome detection — predictions are recorded but outcomes require manual annotation that rarely happens. The project has the rare and genuine architectural soundness to deliver on its vision, but it is currently building floors 3-5 while floor 2 has known structural cracks. The optimal path is: stop building new intelligence features, fix the calibration pipeline, verify the existing 265 untested invariants, and prove the saw-tooth convergence model works. If F(S) can be pushed from 0.62 to 0.78 through verification alone, the architecture is vindicated. If it can't, something deeper is wrong — and that's worth knowing before building more.

---

## 2. Architectural Assessment

### Hard Constraint Compliance

| Constraint | Verdict | Evidence |
|-----------|---------|----------|
| **C1: Append-only** | **MAINTAINED** | `#![forbid(unsafe_code)]`, typestate `Transaction<Building>` → `Transaction<Committed>`, `BTreeSet<Datom>` with insert-only interface. Retractions are datoms with `op=Retract`. No delete paths exist. Kani proof `prove_append_only` verifies monotonic growth. |
| **C2: Identity by content** | **MAINTAINED** | `EntityId::from_content()` uses BLAKE3. `EntityId::from_ident()` for keyword-based identity. Same bytes → same ID everywhere. Kani proof `prove_content_identity` verified. |
| **C3: Schema-as-data** | **MAINTAINED** | `Schema::from_datoms()` reconstructs schema from store contents. 9 axiomatic meta-schema attributes describe themselves (Layer 0). Schema evolution = transaction. No DDL file. |
| **C4: CRDT merge** | **MAINTAINED** | `merge_stores()` is pure set union. Kani proofs verify commutativity, associativity, idempotency. Conflict resolution deferred to query-time per `ResolutionMode`. No merge-time heuristics. |
| **C5: Traceability** | **MAINTAINED** | Every transaction carries provenance (`TxId` with HLC + agent). `task/traces-to` links tasks to spec elements. Spec coverage at 90%. |
| **C6: Falsifiability** | **AT RISK** | 265 current-stage invariants lack L2+ witness verification. Invariants exist with falsification conditions in spec text, but the verification pipeline hasn't kept pace with specification growth. The falsification conditions are *specified* but not *tested*. |
| **C7: Self-bootstrap** | **MAINTAINED** | Genesis determinism verified. Schema attributes describe themselves. Spec elements are datoms. First coherence check is on the system's own specification. |
| **C8: Substrate independence** | **MAINTAINED with minor tension** | Policy manifest is datoms. `policy.rs` uses general-purpose boundary patterns. `guidance.rs` core logic is methodology-neutral. However, the *practice* of 46 sessions has hardened DDIS-specific vocabulary in observations, task titles, and seed context. The architecture is clean; the data reflects one customer. |

### Architectural Convergence vs. Accretion

The architecture is **converging**, not accreting. Evidence:

- Session 036 **falsified** the original 5-stage roadmap and replaced it with a 5-phase model. The system applied its own methodology to itself and corrected course.
- Session 033 unified the convergence engine: `MaterializedViews`, `project_delta`, gradient routing all compose cleanly.
- The policy manifest (ADR-FOUNDATION-013) successfully separated substrate from application.
- The extractor framework (Session 034) completed the plugin architecture.

But there is a **growth pattern concern**: Sessions 044-046 added the concept engine (198 KB), inquiry engine, online calibration, and PERF-REGRESSION epic. These are Phase C/D features being built while Phase B (S0-CLOSE) isn't complete. The observe→compare→reduce loop is present at every level architecturally, but the *inner* loops (calibration, structure discovery) are being built before the *outer* loop (harvest/seed replacing manual context) is proven to work reliably on external projects (DOGFOOD-2 scored 3.48/10).

---

## 3. Engineering Quality Report

### Strengths

1. **`#![forbid(unsafe_code)]`** — Zero unsafe blocks in 83,293 kernel lines. This is exceptional for a system of this complexity.

2. **Typestate pattern for transactions** — `Transaction<Building>` → `Transaction<Committed>` makes illegal state transitions a compile error. Sealed trait prevents external extension.

3. **Error algebra** — `KernelError` → `StoreError` hierarchy with 6+ variants, each with `recovery_hint()`. Caller-distinguishable and actionable.

4. **Three-tier verification** — Kani symbolic proofs (135 harnesses), proptest property-based testing, StateRight model checking. This is beyond what most production systems achieve.

5. **Free functions over methods** — `merge_stores(target, source)` instead of `Store::merge()`. Enables composition and testing without object construction.

### Top 5 Engineering Improvements

**1. Decompose the six >150 KB files (CRITICAL)**

| File | Size | Lines |
|------|------|-------|
| guidance.rs | 260 KB | 7,119 |
| store.rs | 227 KB | 5,848 |
| seed.rs | 210 KB | 5,314 |
| concept.rs | 202 KB | 5,362 |
| schema.rs | 184 KB | 4,957 |
| bilateral.rs | 173 KB | 4,769 |

These files violate NEG-008 (no monolithic files). `guidance.rs` at 260 KB / 7,119 lines contains session management, methodology scoring, context assembly, routing integration, and footer injection — at least 4 distinct concerns. Each file should be a module directory (`guidance/mod.rs`, `guidance/session.rs`, `guidance/scoring.rs`, etc.). This isn't cosmetic — when an AI agent works on "guidance," it must load 260 KB of context to touch any part of it.

**Trace**: Module decomposition directly improves the *system's ability to learn about itself* — smaller files mean more precise bilateral scans, better coverage attribution, and cleaner spec↔impl links. This advances C5 (traceability) and the structure discovery loop (OBSERVER-5).

**2. Fix coverage computation to validate spec existence**

`bilateral.rs` `compute_depth_weighted_coverage()` counts `:impl/implements` refs as coverage even if the referenced spec entity has been retracted or doesn't appear in the LIVE projection. This inflates F(S)'s C component. Fix: intersect with live spec entities before counting.

**Trace**: Advances C6 (falsifiability) — the coverage metric should be falsifiable, and currently it isn't because phantom references can't be detected.

**3. Add automatic hypothesis outcome detection**

The hypothesis ledger records predictions but requires manual `:action/outcome` annotation. Without outcomes, per-type calibration never activates (minimum 5 completed hypotheses per type). Fix: when a task is closed, automatically generate `:action.outcome/followed` if the task's spec-refs overlap with the hypothesis's predicted boundary.

**Trace**: This is the single highest-leverage fix for the degrading hypothesis ledger. It closes the calibration loop (OBSERVER-4) which is currently open — violating NEG-010.

**4. Replace hash embedder with TF-IDF or BM25 for concept clustering**

The `HashEmbedder` produces embeddings based on word overlap via BLAKE3 hashing. With `JOIN_THRESHOLD = 0.20`, any text sharing common project vocabulary ("store," "entity," "spec") clusters together, causing concept collapse. The fundamental problem: hash embeddings can't distinguish semantic domains that share vocabulary. A TF-IDF weighted embedding (where common words are downweighted by inverse document frequency) would solve this without requiring external ML models.

**Trace**: Fixes the concept collapse bug and unblocks OBSERVER-6 (ontology discovery loop).

**5. Test the test infrastructure**

2,095 `#[test]` annotations across 72 files, but the ratio reveals a problem: 233 of those are in `guidance.rs` alone. The 265 untested invariants represent denominator growth in F(S) without numerator growth. The project needs a meta-test: "for each INV-* in the spec, does at least one `#[test]` assert its negation (falsification condition)?" This is a bilateral check on the verification pipeline itself.

**Trace**: Directly addresses the saw-tooth invariant. The spec→test gap is the primary reason F(S) is stagnant.

### Module Decomposition Assessment

The 41 kernel files break into clear architectural layers:

- **Core data model** (datom, store, schema, merge, resolution): Well-factored, appropriately sized except `store.rs` (5,848 lines).
- **Query engine** (query/evaluator, query/graph, compiler): Clean Datalog implementation. `compiler.rs` at 3,032 lines does both pattern compilation and test property emission — should be split.
- **Intelligence layer** (bilateral, trilateral, concept, routing, guidance): This is where the bloat lives. 6 files averaging 4,500 lines each. These should be module directories.
- **Lifecycle** (harvest, seed, context, methodology): Appropriately factored but large.
- **Verification** (kani_proofs, proptest_strategies): Good separation.

**Test density**: 2,095 tests / 127,228 LOC = 1 test per 60 lines. For a formal verification system, this should be closer to 1:30. The Kani proofs cover the core algebraic properties well; the gap is in the intelligence layer (bilateral, concept, routing).

---

## 4. Convergence Diagnosis

### Why F(S) is stagnant at 0.62

**Diagnosis: Specification denominator growth outpacing verification numerator growth.**

F(S) is computed from 7 weighted components:

```
F(S) = 0.18×V + 0.18×C + 0.18×D + 0.13×H + 0.13×K + 0.08×I + 0.12×U
```

The three components most sensitive to the verification gap:

- **V (Validation, weight 0.18)**: Witness depth-weighted scoring. 265 untested INVs score V=0 for those elements, dragging down the average.
- **C (Coverage, weight 0.18)**: Forward scan checks `:impl/implements` links. 142 spec entities lack any impl link. As new specs are added, C drops unless implementations follow immediately.
- **I (Incompleteness, weight 0.08)**: 4-tier partial credit means unfalsifiable specs score 0.15 instead of 0.0, but 265 untested invariants still register as incomplete.

**Quantitative estimate**: If all 265 untested INVs were verified to L2+ witness:
- V would increase by approximately `265 / total_INVs × 0.18 ≈ 0.06-0.10`
- I would increase as unfalsifiable specs gain coverage credit
- **Predicted F(S) ≈ 0.72-0.78**

This matches the saw-tooth model prediction: specification growth (surprise) → F(S) dip → verification (consolidation) → F(S) recovery. **We see the dip but not the recovery.** The verification pipeline is the bottleneck.

### Why the hypothesis ledger is degrading

**Diagnosis: Open calibration loop — predictions recorded, outcomes not matched.**

The ledger records predictions via `record_hypotheses_with_type()` at harvest time. But outcome matching requires explicit `:action/outcome` datoms. The CLI's `braid close` command should generate these automatically but doesn't wire them to hypothesis entities. Without outcomes:

1. Per-type confidence stays at the 0.5 prior
2. The minimum-5-per-type threshold is never met for most types
3. Mean error degrades because new predictions are made against an uncalibrated prior
4. The "trend: DEGRADING" is actually "trend: UNCALIBRATED" — error is rising because the baseline is drifting, not because predictions are getting worse

**Evidence**: Mean error was 0.254 in Session 036 (when 24 hypotheses were manually completed), then degraded to 0.521 as subsequent sessions added predictions without matching outcomes. This is the classic "denominator without numerator" pattern again — more predictions, same number of outcome measurements.

**Fix**: Wire `braid close` → automatic `:action.outcome/followed` datom when the closed task has `:hypothesis/action` refs. This is a small change (< 100 lines in `commands/task.rs`) with massive leverage.

### The 142 spec↔impl gaps

These are **real gaps**, not measurement artifacts. The bilateral forward scan finds spec entities without `:impl/implements` references. As new spec elements are added (Sessions 044-046 added concept engine, inquiry engine, PERF-REGRESSION specs), the gap count grows unless implementations follow. The gaps are **opening**, not closing — 142 is up from approximately 100 in Session 036.

### Concept collapse

**Root cause**: `JOIN_THRESHOLD = 0.20` with `HashEmbedder`. Hash embeddings are based on word co-occurrence. In a project where most observations discuss "store," "entity," "spec," "implementation," and "convergence," all observations share enough vocabulary to exceed 0.20 cosine similarity. The agglomerative clustering merges them into a single mega-cluster.

**This is a measurement problem, not a conceptual one.** The observations genuinely contain distinct concepts (convergence, implementation, schema, query), but the embedding can't distinguish them because it lacks term-frequency weighting. TF-IDF would downweight common terms and surface distinguishing terms.

### Is the saw-tooth observable?

No. F(S) has been at 0.62 ± 0.02 for 13 sessions (033-046). The saw-tooth model predicts oscillation, but we see flatness. This could mean:

1. **Specification and verification are growing at the same rate** (unlikely — 265 untested INVs says verification is behind)
2. **The oscillation period is longer than 13 sessions** (possible but not useful)
3. **F(S) has a structural ceiling below the theoretical maximum** (the most concerning possibility)

The likely explanation is (1's inverse): specification growth consistently outpaces verification, creating a pseudo-steady state where each session adds ~10 specs and ~10 verifications, keeping the ratio constant. To break this equilibrium and observe the saw-tooth, the project needs a **verification-only sprint** — 3-5 sessions with no new specs.

---

## 5. Roadmap Critique

### What's working

1. **The staged architecture is sound.** Phases A (Substrate) and B (Lifecycle) are genuinely ~95% and ~85% complete. The core abstractions survived 46 sessions of contact with reality.

2. **Velocity was extraordinary.** 812 issues closed in 30 days, 1,600+ tests, 108K+ datoms. This is an aggressive pace for a solo-developer-with-AI-agents project.

3. **The specification methodology works.** 22 spec files, 83+ Stage 0 invariants formalized, 30+ ADR categories documented. The DDIS-on-DDIS self-bootstrap is genuine.

4. **The policy manifest is the right architecture.** C8 compliance through datom-driven policies is elegant and correct. Other methodologies can use this substrate.

### What's not working

1. **S0-CLOSE is still open.** The original target was "1-2 weeks." It's been 46 sessions over ~5 weeks. Stage 0 was supposed to validate one thing: *harvest/seed transforms workflow from "fight context loss" to "ride context waves."* The success criterion was "work 25 turns, harvest, start fresh — new session picks up seamlessly." **Has this been validated?** The DOGFOOD-2 score of 3.48/10 on an external project suggests not yet.

2. **Scope creep into Phase C/D while Phase B is unfinished.** Sessions 044-046 built the concept engine (198 KB), inquiry engine, online calibration (scored 4.1/10), and performance regression analysis — all Phase C/D work. Meanwhile, the hypothesis ledger is degrading, 265 INVs are untested, and F(S) hasn't moved in 13 sessions. The project is **building intelligence on top of uncalibrated infrastructure**.

3. **The velocity stall.** 0 issues closed in the last 7 days (was 133/week). This isn't necessarily bad — it could indicate the project has entered a different phase. But combined with F(S) stagnation and ledger degradation, it suggests the project may have hit diminishing returns on the current approach.

4. **883 issues total is issue inflation.** Even with 813 closed (92%), the 70 open issues span 8+ epics across 4 phases. The backlog has become a second project to manage. Issue management overhead is visible in the session transcripts (Sessions 036, 028 spent significant time on administrative convergence rather than implementation).

### What should change

**Be brutally honest**: The project has confused **capability building** with **hypothesis validation**. The bedrock claim is "harvest/seed transforms workflow." After 46 sessions, the test of this claim is DOGFOOD-2, which scored 3.48/10. Everything else — the concept engine, topology pipeline, inquiry engine, online calibration — is premature optimization of a system whose core value proposition hasn't been proven on external data.

The concept engine work (Sessions 044-045) is particularly concerning. The `HashEmbedder` was known to be a bottleneck, online calibration scored 4.1/10, and concept collapse persists. Three sessions were spent on a feature that doesn't work yet, while the core calibration pipeline (which *does* work when outcomes are recorded) was left broken.

---

## 6. The Optimal Path Forward

### Priority 1: Fix the Calibration Pipeline (Sessions 47-48)

**What**: Wire automatic outcome detection into `braid close`. When a task is closed, check if any hypothesis entity references it via `:hypothesis/action`. If so, generate `:action.outcome/followed` datoms automatically.

**Why highest-leverage**: This is the one change that closes the hypothesis ledger loop. Without it, OBSERVER-4 (calibration) is open — the system makes predictions but never learns from outcomes. Every other intelligence feature (routing, concept clustering, inquiry) depends on calibrated predictions. This is the keystone.

**What it unblocks**: Hypothesis ledger recovery (mean error should drop from 0.521 back toward 0.254), per-type confidence calibration activation, R(t) routing accuracy improvement.

**Scope**: Small (< 100 lines in `commands/task.rs` + harvest pipeline).

**Advances**: OBSERVER-4, NEG-010 (no open loops), ADR-FOUNDATION-017 (hypothetico-deductive loop).

### Priority 2: Verification Sprint — 265 Untested INVs (Sessions 48-52)

**What**: Freeze all new specification and feature work. For each of the 265 untested current-stage INVs, add at least one L2+ witness test. Prioritize by F(S) component weight: V (0.18) and C (0.18) first.

**Why highest-leverage**: This is the **saw-tooth recovery phase**. The specification horizon expanded; now verification must catch up. If F(S) rises to ~0.75-0.78 after this sprint, the convergence model is validated. If it doesn't, we learn something important about F(S)'s structural ceiling.

**What it unblocks**: F(S) movement (proving the system can learn), saw-tooth model validation, spec↔impl gap reduction (currently 142 → target < 50).

**Scope**: Large (5 sessions, 265 tests to write or verify).

**Advances**: C6 (falsifiability), OBSERVER-4 (ground truth for calibration), V and C components of F(S).

### Priority 3: Fix Coverage Validation Bug (Session 48)

**What**: In `bilateral.rs` `compute_depth_weighted_coverage()`, intersect `:impl/implements` refs with live spec projection before counting coverage. Currently, refs to retracted or phantom specs inflate C.

**Why**: Measurement integrity. If C is inflated, F(S) is inflated, and the saw-tooth model can't be validated.

**Scope**: Small (< 50 lines).

**Advances**: C6 (falsifiability of the measurement itself).

### Priority 4: S0-CLOSE — Validate the Core Hypothesis (Session 49-50)

**What**: Define and execute the S0 success criterion mechanically: work 25 turns on an external project, harvest, start a fresh session with seed only, and measure whether the new session can continue without manual re-explanation. Score rigorously.

**Why**: This is the **foundational hypothesis**. 46 sessions have been spent building infrastructure. The question "does harvest/seed actually work?" needs a definitive answer.

**What it unblocks**: Strategic direction. If S0 validates: proceed to Stages 1-4 with confidence. If S0 fails: diagnose whether the problem is seed assembly, harvest quality, or the concept itself.

**Scope**: Medium (2 sessions of focused external validation).

**Advances**: The entire project's raison d'être.

### Priority 5: Module Decomposition of >150 KB Files (Sessions 50-52)

**What**: Convert `guidance.rs`, `store.rs`, `seed.rs`, `concept.rs`, `schema.rs`, `bilateral.rs` from single files to module directories. No behavioral changes — pure refactor.

**Why**: These files are too large for effective AI-assisted development. An agent working on seed assembly loads 210 KB of context. After splitting, it loads ~30 KB. This directly improves the system's ability to modify itself — which is the meta-level of the same convergence problem Braid is trying to solve.

**Scope**: Large (parallel subagent work, pure refactor with no behavior change).

**Advances**: NEG-008 (no monolithic files), OBSERVER-5 (structure discovery — the system should be able to discover its own module structure).

### Priority 6: Replace HashEmbedder (Session 52-53)

**What**: Implement TF-IDF weighted embeddings using store vocabulary statistics. No external ML dependency needed — compute IDF from the store's own attribute/value corpus.

**Why**: Concept collapse blocks OBSERVER-6 (ontology discovery). The hash embedder is a known bottleneck (DOGFOOD-2, Session 045 testing).

**Scope**: Medium (TF-IDF implementation + concept clustering re-evaluation).

**Advances**: OBSERVER-6, DOGFOOD-2 score improvement.

---

## 7. Strategic Risks

### Risk 1: The Convergence Thesis May Have a Structural Ceiling

F(S) at 0.62 for 13 sessions may not be a verification bottleneck — it may be the equilibrium point of the current F(S) formulation. The 7-component weighted sum may asymptotically approach a value below 1.0 due to structural interdependencies between components. For example, improving V (validation) requires adding tests, which adds spec elements, which increases the denominator for C (coverage) and I (incompleteness). If the components are **anti-correlated by construction**, F(S) may have a structural maximum well below 1.0.

**Mitigation**: The verification sprint (Priority 2) will empirically test this. If F(S) doesn't move after 265 INVs are verified, the formulation needs revision.

### Risk 2: Single-Customer Overfitting

46 sessions of DDIS-on-DDIS have optimized the system for one domain: software specification verification. The policy manifest architecture (C8) is correct, but the *implementations* — harvest pipeline, seed assembly, concept clustering, routing — have all been tuned to work with INV/ADR/NEG ontologies. The DOGFOOD-2 score of 3.48/10 on a Go CLI project may indicate that the "universal substrate" has quietly become a "DDIS substrate."

**Mitigation**: Priority 4 (S0-CLOSE external validation) directly tests this. If the system can't manage a non-DDIS project, the substrate/application boundary needs recalibration despite the clean architectural separation.

### Risk 3: Complexity Acceleration Without Consolidation

127,228 LOC in 46 sessions ≈ 2,766 LOC/session average. The codebase is growing faster than it can be verified (265 untested INVs), faster than it can be documented (142 spec↔impl gaps), and faster than its own intelligence can track (hypothesis ledger degrading). If this trajectory continues, the system will reach a point where no single session can meaningfully understand the whole.

**Mitigation**: The verification sprint + module decomposition create a consolidation phase. The project needs to **slow down to speed up** — a smaller, fully verified codebase is more valuable than a larger, partially verified one.

### Risk 4: The Daemon May Be a Distraction

Phase E (Active Runtime) requires a daemon for continuous observation. But the CLI architecture may be sufficient for the core value proposition (harvest/seed lifecycle). If the daemon is necessary for convergence but the CLI can't deliver a proof-of-concept without it, the project has a chicken-and-egg problem. If the CLI *can* deliver the proof-of-concept, the daemon becomes an optimization rather than a prerequisite.

**Mitigation**: Validate S0 with the CLI first. Only build the daemon if the CLI validates the core hypothesis but demonstrates clear performance/automation bottlenecks that require continuous operation.

### Risk 5: The Formalism May Be Too Expensive for the Value It Delivers

The project carries enormous formal overhead: 22 spec files, 83+ invariants, 30+ ADRs, 265 verification tasks, hypothesis ledger, bilateral scans, fitness functions, 8-type divergence taxonomy. Each of these is internally consistent and well-motivated. But the aggregate cost of maintaining this formalism may exceed the value it produces — especially if the primary user is a single developer working with AI agents.

**Mitigation**: This is not a risk to eliminate but to monitor. If S0 validates and external users adopt the system, the formalism pays for itself through reduced divergence. If the project remains a solo effort, a lighter-weight formalism might be more appropriate. The saw-tooth model is the diagnostic: if F(S) moves after the verification sprint, the formalism is earning its keep.

---

## 8. The Deepest Question

**The question the project should be asking but isn't:**

> **Is the system observing its own failure to converge?**

The project's bedrock claim is that it implements the atomic operation at every level: *observe reality → compare to model → reduce the discrepancy*. But the system's most important discrepancy — F(S) stagnant at 0.62 for 13 sessions, hypothesis ledger degrading, 265 untested INVs accumulating — is not being detected, diagnosed, or reduced by the system *itself*.

A human reading the status dashboard sees these numbers. But does the guidance system emit "your verification pipeline is the bottleneck"? Does the routing function prioritize verification tasks over new feature tasks when the spec↔impl gap is widening? Does the harvest capture "F(S) hasn't moved in 5 sessions — this is anomalous"?

If Braid is truly a learning system, it should be able to detect its own stagnation as a Type 9 divergence (Reflexive: system vs system's-model-of-itself) and recommend the verification sprint that this assessment recommends. If it can't — if it takes an external assessment to identify the obvious — then the self-improvement loop (the Y-combinator property) isn't yet operational.

The test is simple: run `braid status` and `braid seed --task "improve convergence"` and see if the system recommends what this assessment recommends. If it does, the system is learning. If it doesn't, the most important invariant — **the system can diagnose its own convergence failures** — is untested and possibly violated.

This is Type 9 divergence, and it is the only divergence type that matters at this stage of the project.

---

*This assessment was grounded in the actual codebase (127,228 LOC, 2,095 tests, 41 kernel modules), git history (46 sessions, 92 recent commits), and running tests (all passing). The recommendations trace to the project's own formalism — not conventional wisdom. The system is designed to learn from honest assessment. This is what the data says.*
