# Braid Implementation Guide — Master Index

> **Identity**: This guide is the implementing agent's operating manual for building Braid.
> It is not documentation — it is an optimized prompt. Every structural choice is a prompt
> design choice. Demonstrations outperform constraints. Exact types outperform prose.
>
> **Methodology**: Cleanroom three-box protocol (Mills). Each namespace proceeds:
> Black box (contract) → State box (design) → Clear box (implementation).
> The guide parallels the specification: `guide/NN-*.md` pairs with `spec/NN-*.md`.
>
> **LLM-Native Principle**: Every output surface consumed by an LLM is an optimized prompt.
> CLI output, MCP descriptions, error messages, guidance footers, seed output, dynamic
> CLAUDE.md — all co-designed for coherence with the data substrate.

---

## Cognitive Phase Protocol

### One namespace at a time

For each namespace in build order:

1. **Load** `spec/{NN}-{name}.md` — read FIRST (the *what*)
2. **Load** `guide/{NN}-{name}.md` — read SECOND (the *how*)
3. **Implement** — write code, following the three-box decomposition
4. **Verify** — run the quality gates for that namespace
5. **Unload** — move to the next namespace

### One cognitive mode per namespace

| Namespace | Cognitive Mode | Primary Reasoning |
|-----------|---------------|-------------------|
| STORE | Algebraic | Set theory, CRDT laws, commutativity proofs |
| SCHEMA | Ontological | Category theory, bootstrap, self-description |
| QUERY | Language-theoretic | Datalog semantics, CALM, fixpoint evaluation |
| RESOLUTION | Order-theoretic | Lattices, partial orders, conflict predicates |
| MERGE | Set-theoretic | Union, deduplication, monotonicity |
| HARVEST | Information-theoretic | Epistemic gaps, information gain, pipeline |
| SEED | Retrieval-theoretic | Relevance, compression, trajectory seeds |
| GUIDANCE | Control-theoretic | Basin dynamics, anti-drift, feedback loops |
| INTERFACE | Prompt-engineering | LLM activation, output algebra, token budgets |

### Reading order per namespace

spec (what it must do) → guide (how to build it) → code (the implementation)

---

## Spec Cross-Reference

| Guide File | Spec File | SEED.md §§ | ADRS.md Categories |
|------------|-----------|------------|---------------------|
| [00-architecture.md](00-architecture.md) | [00-preamble.md](../spec/00-preamble.md) | §4, §10, §11 | FD, AS, SR |
| [01-store.md](01-store.md) | [01-store.md](../spec/01-store.md) | §4, §11 | FD-001–012, AS-001–010 |
| [02-schema.md](02-schema.md) | [02-schema.md](../spec/02-schema.md) | §4 | SR-008–010, FD-005/008 |
| [03-query.md](03-query.md) | [03-query.md](../spec/03-query.md) | §4 | FD-003, SQ-001–010 |
| [04-resolution.md](04-resolution.md) | [04-resolution.md](../spec/04-resolution.md) | §4 | FD-005, CR-001–006 |
| [05-harvest.md](05-harvest.md) | [05-harvest.md](../spec/05-harvest.md) | §5 | LM-005–006, LM-012–013 |
| [06-seed.md](06-seed.md) | [06-seed.md](../spec/06-seed.md) | §5, §8 | IB-010, PO-014, GU-004 |
| [07-merge-basic.md](07-merge-basic.md) | [07-merge.md](../spec/07-merge.md) | §6 | AS-001 |
| [08-guidance.md](08-guidance.md) | [12-guidance.md](../spec/12-guidance.md) | §7, §8 | GU-001–008 |
| [09-interface.md](09-interface.md) | [14-interface.md](../spec/14-interface.md) | §8 | IB-001–012 |
| [10-verification.md](10-verification.md) | [16-verification.md](../spec/16-verification.md) | — | — |
| [11-worked-examples.md](11-worked-examples.md) | Multiple | §4, §5, §8, §10 | — |
| [12-stages-1-4.md](12-stages-1-4.md) | [17-crossref.md](../spec/17-crossref.md) | §10 | — |

---

## Build Order

The dependency graph (from spec/17-crossref.md §17.2) determines the implementation order:

```
1. STORE ──────────────────────────────────────── guide/01-store.md
   ↓
2. SCHEMA ─────────────────────────────────────── guide/02-schema.md
   ↓
3. QUERY ──────────────────────────────────────── guide/03-query.md
   ↓
4. RESOLUTION ─────────────────────────────────── guide/04-resolution.md
   ↓
5. HARVEST ────────────────────────────────────── guide/05-harvest.md
   ↓
6. SEED ───────────────────────────────────────── guide/06-seed.md
   ↓
7. MERGE (basic: INV-MERGE-001, 008–009) ─────── guide/07-merge-basic.md
   ↓
8. GUIDANCE ───────────────────────────────────── guide/08-guidance.md
   ↓
9. INTERFACE ──────────────────────────────────── guide/09-interface.md
```

**Gate between namespaces**: Before advancing to the next namespace, all quality gates
for the current namespace must pass (see guide/10-verification.md).

---

## Stage 0 Scope

**62 invariants** across 9 namespaces (full inclusion for STORE, RESOLUTION; partial
for SCHEMA, QUERY, HARVEST, SEED, MERGE, GUIDANCE, INTERFACE).
Full list in spec/17-crossref.md Appendix C and spec/16-verification.md matrix.

**Count verification** (from spec/16-verification.md, confirmed by D1-scope-boundary.md):

| Namespace | Count | Elements |
|-----------|-------|----------|
| STORE | 13 | 001-012, 014 |
| SCHEMA | 7 | 001-007 (006 progressive, 008 deferred to Stage 2) |
| QUERY | 10 | 001-002, 005-007, 012-014, 017, 021 |
| RESOLUTION | 8 | 001-008 (all) |
| HARVEST | 5 | 001-003, 005, 007 |
| SEED | 4 | 001-004 |
| MERGE | 4 | 001-002, 008-009 |
| GUIDANCE | 6 | 001-002, 007-010 |
| INTERFACE | 5 | 001-003, 008-009 |
| **Total** | **62** | |

Note: D1 report counted 61 because Appendix C's detailed prose does not explicitly list
INV-MERGE-009, but the verification matrix (spec/16-verification.md) assigns MERGE-009 to
Stage 0. The authoritative count is **62**, consistent with Appendix C's header and Appendix B.

### Sub-Staging Recommendation (from D1-scope-boundary.md)

62 invariants is aggressive for the SEED.md "1-2 week" target. The recommended approach
is to split Stage 0 into two sub-stages:

**Stage 0a — Foundation** (38 INV, ~2-3 weeks):
- STORE (13): Append-only datom store, CRDT algebra, HLC, indexes
- SCHEMA (7): Genesis, axiomatic attributes, six-layer architecture
- QUERY (10): Datalog engine (Strata 0-1), graph algorithms (topo sort, SCC, PageRank, critical path, density)
- RESOLUTION (8): Per-attribute conflict handling, three-tier routing

**Stage 0b — Lifecycle + Intelligence** (24 INV, ~2 weeks):
- HARVEST (5): Epistemic gap detection, pipeline, proactive warnings
- SEED (4): Associate/assemble/compress, dynamic CLAUDE.md
- MERGE (4): Pure set-union merge, deduplication
- GUIDANCE (6): Injection, anti-drift, M(t), task derivation, R(t) routing
- INTERFACE (5): CLI modes, MCP tools, error recovery

**Rationale**: Stage 0a validates the core store hypothesis (append-only, content-addressed,
CRDT merge) before building the lifecycle layer. Stage 0b cannot function without a working
store+schema+query+resolution foundation. This matches the natural dependency order
(spec/17-crossref.md section 17.2).

**Cross-stage dependency note**: INV-GUIDANCE-010 (R(t) routing) references betweenness
centrality (INV-QUERY-015, Stage 1). At Stage 0, R(t) uses only Stage 0 graph metrics
(PageRank + critical path + topo sort), degrading gracefully without betweenness. See
guide/08-guidance.md section 8.2 (R(t) state box) for the proxy_betweenness implementation.

**Success criterion**: Work 25 turns, harvest, start fresh with seed — new session picks
up without manual re-explanation.

**First act**: Migrate the specification elements from `spec/` into the store as datoms.

---

## Quality Gates

| Gate | Command | Checks | Blocks |
|------|---------|--------|--------|
| 1: compile | `cargo check --all-targets` | V:TYPE patterns compile | Every commit |
| 2: test | `cargo test` | V:PROP properties hold | Every commit |
| 3: kani | `cargo kani` | V:KANI bounded proofs | PRs to main |
| 4: clippy | `cargo clippy -- -D warnings` | Style, correctness lints | Every commit |
| 5: format | `cargo fmt --check` | Consistent formatting | Every commit |

---

## File Index

| File | Lines | Purpose |
|------|-------|---------|
| [00-architecture.md](00-architecture.md) | ~600 | Crate layout, type catalog, CLI/MCP specs, LLM-native design |
| [01-store.md](01-store.md) | ~300 | STORE build plan — append-only datom store, CRDT algebra |
| [02-schema.md](02-schema.md) | ~250 | SCHEMA build plan — genesis, axiomatic attributes, layers |
| [03-query.md](03-query.md) | ~300 | QUERY build plan — Datalog engine, strata 0–1 |
| [04-resolution.md](04-resolution.md) | ~250 | RESOLUTION build plan — per-attribute conflict handling |
| [05-harvest.md](05-harvest.md) | ~250 | HARVEST build plan — epistemic gap detection, pipeline |
| [06-seed.md](06-seed.md) | ~300 | SEED build plan — associate/assemble/compress, dynamic CLAUDE.md |
| [07-merge-basic.md](07-merge-basic.md) | ~200 | MERGE Stage 0 subset — pure set union |
| [08-guidance.md](08-guidance.md) | ~250 | GUIDANCE build plan — injection, anti-drift, spec-language |
| [09-interface.md](09-interface.md) | ~300 | INTERFACE build plan — CLI modes, MCP tools, LLM surfaces |
| [10-verification.md](10-verification.md) | ~300 | Verification pipeline, CI gates, coverage matrix |
| [11-worked-examples.md](11-worked-examples.md) | ~550 | Self-bootstrap, session transcripts, Datalog queries |
| [12-stages-1-4.md](12-stages-1-4.md) | ~200 | Future roadmap, extension points |
