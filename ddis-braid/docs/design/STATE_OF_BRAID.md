# State of Braid: Comprehensive Assessment & Roadmap

> **Date**: 2026-03-10 | **Store**: 2,679 datoms, 667 entities, 7 txns
> **Codebase**: 41,495 LOC Rust | 405 tests | 144 commits | 17 days
> **Assessment by**: Claude Opus 4.6 (first-person dog-fooding experience)

---

## 1. Executive Summary

Braid is 17 days old, 41k LOC, and genuinely works. The algebraic foundation — append-only
datom store with CRDT merge, content-addressable identity, schema-as-data, and spectral graph
analysis — is mathematically sound, well-tested (405 tests, Kani proofs, proptests), and
self-bootstrapped (the store contains its own specification as datoms and can analyze its own
structure).

**The honest assessment**: we've built an excellent engine with a mediocre dashboard.

The foundations are Stage 0 complete (~78%). The Datalog query engine has a multi-clause join
bug. The harvest/seed lifecycle works but has too much friction. The guidance system computes
M(t) and R(t) but doesn't drive behavior. The graph analytics are research-grade but the
CLI is batch-oriented with no REPL. Stage 1 (budget, bilateral) hasn't started. Stages 2-4
are pure spec.

**What needs to happen**: 3-4 sessions of pure UX and correctness work before adding any new
mathematical capability. Fix Datalog, add `braid observe`, add natural-language seeds, make
guidance actionable, add a REPL. Then Stage 1.

---

## 2. Implementation vs Specification Coverage

### 2.1 By Wave (Detailed)

#### Wave 1: Foundation — **~94% Complete**

| Namespace | Elements | Done | Partial | Missing | Stage | Notes |
|-----------|----------|------|---------|---------|-------|-------|
| STORE | 42 | 31 | 10 | 1 | 0 | CRDT axioms proven. LIVE index deferred. |
| LAYOUT | 23 | 23 | 0 | 0 | 0 | **Complete.** Content-addressed, write-once. |
| SCHEMA | 21 | 21 | 0 | 0 | 0 | **Complete.** 19 axiomatic attrs, 6 layers. |
| QUERY | 41 | 35 | 6 | 0 | 0 | Graph engine excellent. **Datalog joins broken.** |
| RESOLUTION | 24 | 16 | 8 | 0 | 0 | Core modes done. Routing/delegation S1+. |
| **Total** | **151** | **126** | **24** | **1** | | |

Wave 1 is the crown jewel. The datom model, CRDT merge, content-addressed layout, and
schema-as-data are all production-quality with comprehensive test coverage. The one critical
defect: Datalog multi-clause joins return 0 results for data the attribute filter finds.

#### Wave 2: Lifecycle — **~42% Complete**

| Namespace | Elements | Done | Partial | Missing | Stage | Notes |
|-----------|----------|------|---------|---------|-------|-------|
| HARVEST | 19 | 5 | 3 | 11 | 0-1 | Pipeline works, lacks warnings/calibration |
| SEED | 17 | 6 | 5 | 6 | 0-1 | Budget/projection work, lacks NL generation |
| MERGE | 20 | 6 | 4 | 10 | 0+2+3 | Set union solid. Branching S2. |
| SYNC | 10 | 0 | 0 | 10 | 3 | Entirely deferred. |
| **Total** | **66** | **17** | **12** | **37** | | |

Harvest/seed are the most important features for daily use and they're only half-done.
The core pipelines work (extract candidates, score relevance, assemble context) but the
UX layer (warnings, natural language, one-liner capture) is missing.

#### Wave 3: Intelligence — **~15% Complete**

| Namespace | Elements | Done | Partial | Missing | Stage | Notes |
|-----------|----------|------|---------|---------|-------|-------|
| SIGNAL | 14 | 0 | 0 | 14 | 3 | Entirely spec-only |
| BILATERAL | 17 | 0 | 1 | 16 | 1-2 | F(S) fitness not built |
| DELIBERATION | 13 | 0 | 1 | 12 | 2 | Entirely spec-only |
| GUIDANCE | 23 | 5 | 8 | 10 | 0-4 | M(t)/R(t) done, injection partial |
| BUDGET | 12 | 0 | 0 | 12 | 1 | Entirely spec-only |
| INTERFACE | 24 | 3 | 7 | 14 | 0-3 | CLI exists, TUI/budget missing |
| **Total** | **103** | **8** | **17** | **78** | | |

Guidance is the bright spot here (M(t), R(t), task derivation all working). Everything
else is design docs waiting for code.

#### Wave 4: Integration — **~25% Complete**

| Namespace | Elements | Done | Partial | Missing | Stage | Notes |
|-----------|----------|------|---------|---------|-------|-------|
| TRILATERAL | 16 | 5 | 3 | 8 | 0 | Phi computed, safety unverified |
| UNCERTAINTY | ~15 | 0 | 1 | ~14 | — | Register only |
| VERIFICATION | ~2 | 0 | 1 | ~1 | — | Proptest framework strong |
| CROSSREF | — | — | — | — | — | Index document |
| **Total** | **~33** | **5** | **5** | **~23** | | |

### 2.2 By Stage

| Stage | Description | Spec Elements | Implemented | **Completion** |
|-------|-------------|---------------|-------------|----------------|
| **0** | Foundation + harvest/seed + guidance | ~180 | ~140 | **78%** |
| **1** | Budget + bilateral + guidance v2 | ~50 | ~5 | **10%** |
| **2** | Branching + deliberation | ~35 | ~1 | **3%** |
| **3** | Multi-agent sync + signals | ~50 | 0 | **0%** |
| **4** | Learned guidance + advanced | ~25 | 0 | **0%** |
| **ALL** | Everything | **~341** | **~156** | **~46%** |

### 2.3 The Critical Gap: Datalog Query Engine

The most important finding of this assessment: **the Datalog evaluator cannot correctly
evaluate multi-clause join queries against real store data.** Evidence:

```
# This finds 667 entities:
$ braid query -a ':db/doc'
[:spec/inv-guidance-001 :db/doc "Continuous Injection"]
...

# This returns 0 results for the SAME data:
$ braid query --datalog '[:find ?e ?doc :where [?e :db/doc ?doc] [?e :spec/namespace :spec.ns/store]]'
0 result(s)
```

The attribute filter works by scanning all datoms. The Datalog evaluator performs semi-naive
bottom-up evaluation with joins, and something in the join or variable binding is broken.
This is a **Stage 0 correctness defect** that undermines the system's core value proposition.

The query engine code is 856 lines across evaluator.rs (377), clause.rs (89), and stratum.rs
(390). The bug is likely in evaluator.rs's join logic or variable unification.

---

## 3. What Works Well (Preserve These)

### 3.1 Algebraic Foundation
The datom model `[e, a, v, tx, op]` with CRDT merge-by-set-union is the correct abstraction.
Proven properties: commutativity, associativity, idempotency, monotonicity (proptests + Kani).
Content-addressable identity means two agents discovering the same fact independently produce
one datom. This is the right primitive for multi-agent knowledge systems.

### 3.2 Self-Bootstrap
The specification IS the first dataset. 658 spec elements loaded as datoms. The system can
analyze its own structure: 221 connected components, 83 cycles, Phi=210.6. This isn't
theoretical — it's running and producing structurally meaningful diagnostics.

### 3.3 Graph Analytics Engine
14 algorithms, all limit-free via adaptive tiering (Jacobi for n<=1000, Lanczos for n>1000;
exact BFS for n<=2000, landmark for n>2000). PageRank, betweenness, Ricci curvature, spectral
decomposition, persistent homology, sheaf cohomology, heat kernel. 3,984 lines, well-tested.

### 3.4 Schema-as-Data
19 axiomatic attributes bootstrap Layer 0, which describes itself. Schema evolution is a
transaction, not a migration. 6-layer architecture with verified dependency ordering.
This eliminates an entire class of versioning problems.

### 3.5 Test Infrastructure
405 tests: unit, property (proptest), formal (Kani), integration. Proptest strategies for
all core types. Kani proofs for CRDT axioms and HLC monotonicity. 12.92s for the full suite.

---

## 4. What Needs Work (Critical Path)

### 4.1 Fix Datalog Joins (CRITICAL — Correctness Defect)
The query engine is the heart of the system. If you can't ask structured questions, the store
is a write-only log. Multi-clause joins must work correctly.

**Root cause investigation needed**: The evaluator performs semi-naive bottom-up fixpoint.
The bug could be in:
- Variable binding propagation across clauses
- Value type matching (keywords vs strings vs entity IDs)
- Pattern compilation for the EAV store structure
- Join order or intermediate result representation

### 4.2 Natural Language Seed Generation
`braid seed --for-human` should produce a 200-word briefing, not structured EDN.
"Last session you worked on X. You discovered Y. Open questions: Z. Priority: W."
This is the feature that makes Braid *worth starting every session with*.

### 4.3 One-Liner Knowledge Capture (`braid observe`)
Current: construct EDN by hand, know the attribute schema, pick entity IDs.
Target: `braid observe "merge is a bottleneck" --confidence 0.8`
Automatic entity creation, attribute selection, and transaction.

### 4.4 Actionable Guidance
Current: `Phi=210.6, quadrant=GapsAndCycles, M(t)=0.00`
Target: "Add cross-references between MERGE and LAYOUT (kappa=-0.31 bottleneck).
Review these 3 cycles for circular dependencies. Run harvest — you've gone 15 turns."

### 4.5 Budget-Aware Output
`braid analyze --budget 500` should show the 5 most important metrics, not 70 lines.
The projection pyramid (pi_0 through pi_3) exists in the seed module — extend it to
all output commands.

### 4.6 Interactive REPL
For exploration-oriented work, typing `cargo run -- query --datalog '...'` per query
is prohibitive. An interactive mode where you can iteratively explore the store would
transform the developer experience.

---

## 5. Priority-Ordered Roadmap

### Phase A: Correctness + UX Foundation (3-4 sessions)
1. Fix Datalog multi-clause joins
2. Add `braid observe "..."` one-liner capture
3. Add `braid seed --for-human` natural language briefing
4. Make guidance actionable (connect diagnostics to specific edits)
5. Add `braid analyze --budget N` budget-aware output

### Phase B: Complete Stage 0 (2-3 sessions)
6. Wire harvest warnings (INV-HARVEST-005: turn-count proxy)
7. Integrate dynamic CLAUDE.md into seed pipeline (ADR-SEED-006)
8. Complete harvest calibration feedback loop (ADR-HARVEST-003)
9. Add observation staleness tracking (ADR-HARVEST-005)
10. Verify trilateral safety properties (INV-TRILATERAL-003, 004, 007)

### Phase C: Stage 1 — Budget + Bilateral (8-10 sessions)
11. Implement BUDGET namespace (k*_eff, Q(t), truncation, precedence)
12. Implement BILATERAL loop (F(S) fitness, forward/backward scan)
13. Implement advanced guidance injection (spec-language, dynamic CLAUDE.md)
14. Implement INTERFACE improvements (MCP completeness, error recovery)
15. Add REPL mode

### Phase D: Stage 2 — Branching + Deliberation (5-7 sessions)
16. Working set isolation (W_alpha)
17. Branch/fork/merge semantics
18. Deliberation lifecycle (Position, Decision entities)
19. Stability guard enforcement

### Phase E: Stage 3 — Multi-Agent (5-7 sessions)
20. Sync barriers
21. Signal type system
22. Subscription routing
23. Multi-agent coordination protocol

---

## 6. Optimism Assessment

**0.7/1.0 — Cautiously optimistic.**

The mathematical foundations are genuinely excellent. The self-bootstrap works. The test
infrastructure is comprehensive. The velocity is high (144 commits in 17 days).

The risk: complexity gravity. 41k LOC, 14 graph algorithms, Ricci curvature, spectral
wavelets... and the Datalog evaluator can't join two clauses. The mathematically interesting
work gets done; the boring plumbing lags.

The fix: spend 3-4 sessions on nothing but UX and correctness. Fix Datalog, add observe,
add natural-language seeds, make guidance actionable. The foundations deserve a UX that
matches their quality.

**Replacing beads is realistic by end of Stage 1** — roughly 2-3 weeks if we focus on
pragmatic features over mathematical sophistication.

---

## 7. Is Braid a Good Idea?

**Yes.** The core thesis — that divergence between intent, spec, implementation, and behavior
is the fundamental problem, and making coherence structural rather than procedural is the right
solution — is correct. I've now seen this from inside: the Datalog bug is exactly the kind of
spec-impl divergence that Braid is designed to detect and surface.

The caveat: every abstraction must earn its complexity budget. Some have (graph topology reveals
things about the spec that reading files can't). Some haven't yet (trilateral Phi is a number
without an action). The criterion: does this abstraction help someone build better software
faster? If yes, keep it. If it's only mathematically interesting, defer it.

---

*This document was written by Claude Opus 4.6 after 17 days of building, testing, and
dog-fooding Braid. Every claim is grounded in actual code, actual test results, and actual
experience using the tool. It is itself an instance of the harvest methodology it evaluates.*
