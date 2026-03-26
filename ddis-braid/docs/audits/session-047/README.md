# Session 047 Audit — Full Structural Audit

> **Date**: 2026-03-26
> **Auditor**: Claude Opus 4.6 (1M context)
> **Scope**: Complete codebase audit across 10 dimensions (type system, invariants, errors, concurrency, spec alignment, C8 compliance, architecture, performance, formal verification, tests)

## Files

| File | Contents | Words |
|------|----------|-------|
| [FULL_AUDIT.md](FULL_AUDIT.md) | Three-document deliverable: Audit Report (31 findings), Architectural Assessment (soundness + tensions + loops + risks), Implementation Roadmap (37 tasks, 6 waves, 264h) | ~10,000 |
| [FULL_AUDIT_PROMPT.md](FULL_AUDIT_PROMPT.md) | The optimized audit prompt — 5-phase DoF-separated instrument designed using 4 composition skills (prompt-optimization, spec-first-design, rust-formal-engineering, skill-composition). Trajectory-seeded with mandatory 10-file reading order. | ~3,500 |
| [FORMAL_SOUNDNESS_AUDIT_REPORT.md](FORMAL_SOUNDNESS_AUDIT_REPORT.md) | Formal soundness audit executed from the prompt above. 9 parallel investigation agents across 5 phases: soundness (CRDT, typestate, coherence gate, query, F(S), schema, C8), architecture (coupling, type algebra, performance, learning loops), coherence (216-INV coverage matrix), accretive path (ranked actions). | ~3,800 |
| [PROJECT_ASSESSMENT.md](PROJECT_ASSESSMENT.md) | Deep first-principles assessment: C1-C8 compliance, convergence diagnosis (F(S) stagnation + hypothesis ledger degradation), engineering quality (top 5 improvements), roadmap critique, 6-priority optimal path forward, 5 strategic risks, Type 9 divergence question | ~4,500 |

## Key Metrics at Audit Time

| Metric | Value |
|--------|-------|
| LOC | 117,661 (83,293 kernel + 34,368 CLI) |
| Tests | 2,043 passing, 0 failing, 1 ignored |
| Store | 108,627 datoms, 10,245 entities |
| Spec elements | 201 INVs, 156 ADRs, 69 NEGs (433 total) |
| F(S) | 0.62 |
| Hypothesis accuracy | mean error 0.521, trend degrading |

## Finding Summary (Combined)

| Severity | Count | Top Finding |
|----------|-------|-------------|
| Critical | 6 | Coherence gate unwired from write path; F(S) monotonicity claim false; hypothesis dimensional mismatch; schema C3/C8 violations; LIVE index retraction ordering |
| High | 10 | 10 HARD C8 violations (schema L1-L2, INV/ADR/NEG prefixes, MaterializedViews ISP); Loop 1 last-mile open; 265 INVs lack witnesses |
| Medium | 15 | 4 circular deps; 8 god modules >4K LOC; NaN in Value::Double; Wasserstein-1 greedy; merge() O(N) full rebuild; ~20 full-scan bottlenecks in braid status |
| Low | 10 | Dead Applied typestate; Keyword lacks validation; boolean blindness; documentation gaps |

## Roadmap Summary

| Wave | Focus | Hours | Critical Path |
|------|-------|-------|---------------|
| 0 | Blocking defects | 17 | Yes |
| 1 | Soundness recovery | 26 | Yes |
| 2 | Architectural (C8) | 92 | Yes |
| 3 | Verification | 43 | No (parallel) |
| 4 | Performance | 22 | No (parallel) |
| 5 | Production readiness | 64 | Yes |
| **Total** | | **264** | **W0->W1->W2->W5 = 199h** |

## Formal Soundness Audit (FORMAL_SOUNDNESS_AUDIT_REPORT.md)

Executed using the optimized prompt (FULL_AUDIT_PROMPT.md) with 9 parallel investigation agents:

| Phase | Agents | Focus |
|-------|--------|-------|
| 1: Soundness | 6 | CRDT algebra, typestate, coherence gate, query engine, F(S)+schema, C8 |
| 2: Architecture | 2 | Module coupling + type algebra, performance + learning loops |
| 3: Coherence | 1 | 216-invariant coverage matrix across 20 namespaces |
| 4-5: Synthesis | 0 | Accretive path analysis + report compilation (not delegated) |

**Top 3 critical findings**: (1) coherence gate unwired — ~20-line fix, 90× ROI; (2) F(S) non-monotonicity — design change needed; (3) hypothesis dimensional mismatch — predicted R(t) scores vs actual ΔF(S).

**Coverage**: 216 INVs total, 174 implemented (80.6%), 143 tested (66.2%), 27 Kani-proven (12.5%), 70 proptest-covered (32.4%). Strongest: STORE (100%). Weakest: COHERENCE (8%), SYNC (0%).

**C8 audit**: 19 violations (10 HARD). Highest-impact fix: move schema L1-L2 to DDIS policy manifest.

**Learning loops**: Loop 1 (calibration) partially closed, Loop 2 (structure) fully open, Loop 3 (ontology) partially closed.

## Relationship to Prior Audits

- **stage-0/**: Early-stage audit (2026-03-09) — focused on spec coherence, phantom types, namespace readiness
- **stage-0-1/**: Mid-stage audit (2026-03-17) — 14-part deep dive into each subsystem with execution plan
- **session-047/**: This audit (2026-03-26) — full codebase maturity audit with quantified findings and prioritized roadmap

The three audits form a progression: stage-0 found spec-level issues, stage-0-1 found implementation gaps, session-047 assesses production readiness.
