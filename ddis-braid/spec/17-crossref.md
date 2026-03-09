> **Section**: Cross-Reference Index & Appendices | **Wave**: 4 (Integration)
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §17. Cross-Reference Index

> **Purpose**: Maps every element to its source documents, tracks inter-invariant
> dependencies, and provides stage-based views for implementation planning.

### §17.1 Namespace → SEED.md → ADRS.md

| Namespace | SEED.md §§ | ADRS.md Categories | Primary Concerns |
|-----------|------------|---------------------|-----------------|
| FOUNDATION | §1–§11 | LM-001–002, LM-008–009, AA-002, CO-006 | Braid-as-new-impl, methodology-before-tooling, self-bootstrap, structural coherence |
| STORE | §4, §9, §11 | FD-001–012, AS-001–010, SR-001–011, PD-001–004, PO-001, PO-012, LM-007 | Append-only datom store, CRDT merge, content identity, HLC ordering, indexes |
| LAYOUT | §4, §11 | SR-006, SR-007, SR-014, FD-007, FD-013 | Content-addressed transaction files, directory-union merge, filesystem isomorphism, canonical serialization |
| SCHEMA | §4 | SR-008–010, FD-005, FD-008, SQ-008 | 17 axiomatic attributes, genesis, schema-as-data, six-layer architecture, value type union |
| QUERY | §4 | FD-003, SQ-001–010, PO-013, AS-007, AA-001, UA-009 | Datalog, CALM, six strata, FFI boundary, significance tracking, graph engine, stability score |
| RESOLUTION | §4 | FD-005, CR-001–006, UA-003–005, UA-010–012 | Per-attribute resolution, conflict predicate, three-tier routing, spectral authority, delegation |
| HARVEST | §5 | LM-005–006, LM-012–014, IB-012, CR-005, UA-007 | Epistemic gap detection, pipeline, FP/FN calibration, proactive warnings, staleness, DDR feedback |
| SEED | §5, §8 | IB-010, PO-002–003, PO-014, GU-003–004, SQ-007, AA-004, LM-016 | Associate→query→assemble, dynamic CLAUDE.md, rate-distortion, projection pyramid, four knowledge types |
| MERGE | §6 | AS-001, AS-003–006, AS-010, PD-001, PD-004, PO-006–007, CR-010 | Set-union merge, branching G-Set, W_α, merge cascade, cascade determinism, branch comparison |
| SYNC | §6 | PO-010, SQ-001, SQ-004, PD-005 | Consistent cut, barrier protocol, topology independence |
| SIGNAL | §6 | PO-004–005, PO-008, CR-002–003, AS-009, CO-002–003, CO-007 | Eight signal types, dispatch, subscription, diamond lattice signals, taxonomy gaps |
| BILATERAL | §3, §6 | CO-001, CO-004–005, CO-008–010, CO-013–014, SQ-006, AS-006, CO-011, PD-006, LM-004, LM-010 | Adjunction, fitness function, five-point coherence, bilateral authority, reconciliation taxonomy |
| DELIBERATION | §6 | CR-004–005, CR-007, PO-007, AS-002, AA-001 | Three entity types, stability guard, precedent, commitment weight |
| GUIDANCE | §7, §8 | GU-001–008, IB-006, PO-009, AA-007 | Comonad, basin competition, six anti-drift mechanisms, spec-language, M(t)/R(t)/T(t), S1/S2 diagnosis |
| BUDGET | §8 | IB-004–007, IB-011, SQ-007, IMPL-002 | k* measurement, Q(t), precedence, projection pyramid, rate-distortion, tokenization |
| INTERFACE | §8 | IB-001–003, IB-008–009, IB-012–013, SR-011, AA-003, AA-006, FD-011, PO-011, LM-015 | Five layers, CLI modes, MCP tools, TUI, statusline, harvest warning, ten primitives, Rust |
| UNCERTAINTY | §3 | UA-001–002, UA-006, UA-008 | Three-axis tensor, temporal decay, first-class markers, self-referential exclusion |
| VERIFICATION | §10 | IMPL-003 | Three-tier Kani CI pipeline |
| TRILATERAL | §1, §2, §3, §5, §6 | CO-001, CO-004–005, FD-001, AS-001, FD-003 | Trilateral coherence, three LIVE projections, Φ divergence, formality gradient, attribute namespace partitioning |

### §17.2 Invariant Dependency Graph

Key inter-invariant dependencies (an edge A → B means B depends on A holding):

```
INV-STORE-001 (append-only) ──→ INV-MERGE-001 (merge preserves)
                              ──→ INV-HARVEST-001 (harvest commits to store)
                              ──→ INV-SCHEMA-004 (schema monotonicity)
                              ──→ INV-LAYOUT-002 (file immutability — φ preserves C1)

INV-STORE-003 (content identity) ──→ INV-LAYOUT-001 (content-addressed files — φ preserves C2)
INV-MERGE-001 (set union) ──→ INV-LAYOUT-004 (merge = dir union — φ preserves C4)
INV-LAYOUT-011 (canonical serialization) ──→ INV-LAYOUT-001 (prerequisite for identity)

INV-STORE-002 (content identity) ──→ INV-STORE-006 (idempotency)
                                  ──→ INV-MERGE-001 (deduplication)

INV-STORE-004/005/006 (CRDT laws) ──→ INV-MERGE-002 (merge cascade)
                                    ──→ INV-MERGE-010 (cascade determinism)
                                    ──→ INV-SYNC-001 (consistent cut)
                                    ──→ INV-BILATERAL-001 (convergence)

INV-STORE-008 (genesis determinism) ──→ INV-STORE-014 (every command is transaction)

INV-STORE-011 (HLC monotonicity) ──→ INV-STORE-009 (frontier durability)
                                   ──→ INV-SYNC-003 (topology independence)

INV-SCHEMA-001 (genesis) ──→ INV-SCHEMA-002 (self-description)
                           ──→ INV-STORE-014 (every command is transaction)

INV-QUERY-001 (CALM) ──→ INV-SYNC-001 (barriers for non-monotonic)
                       ──→ INV-RESOLUTION-003 (conservative detection)

INV-RESOLUTION-004 (conflict predicate) ──→ INV-SIGNAL-004 (severity routing)
                                          ──→ INV-DELIBERATION-001 (deliberation entry)

INV-HARVEST-001 (epistemic gap) ──→ INV-SEED-001 (seed from store)
                                  ──→ INV-BILATERAL-001 (convergence)

INV-HARVEST-009 (continuous externalization) ──→ INV-GUIDANCE-007 (dynamic CLAUDE.md)
                                               ──→ INV-HARVEST-001 (epistemic gap reduction)

INV-GUIDANCE-001 (continuous injection) ──→ INV-BUDGET-004 (compression by budget)
                                         ──→ INV-INTERFACE-007 (harvest warning)
                                         ──→ INV-INTERFACE-009 (error recovery)

INV-INTERFACE-003 (six MCP tools) ──→ INV-INTERFACE-008 (tool description quality)

INV-INTERFACE-002 (MCP thin wrapper) ──→ INV-INTERFACE-010 (CLI/MCP semantic equivalence)
INV-INTERFACE-010 (CLI/MCP parity) ──→ INV-INTERFACE-003 (six tools — parity per tool)

INV-BUDGET-001 (output budget cap) ──→ INV-BUDGET-006 (token efficiency)

INV-QUERY-012 (topological sort) ──→ INV-QUERY-017 (critical path)
                                   ──→ INV-GUIDANCE-010 (work routing)
INV-QUERY-013 (cycle detection) ──→ INV-QUERY-012 (topological sort precondition)
INV-QUERY-014 (PageRank) ──→ INV-GUIDANCE-010 (work routing)
INV-QUERY-015 (betweenness) ──→ INV-GUIDANCE-010 (work routing)
INV-QUERY-016 (HITS) ──→ INV-QUERY-019 (eigenvector centrality)

INV-GUIDANCE-008 (M(t) adherence) ──→ INV-GUIDANCE-004 (drift detection trigger)
INV-GUIDANCE-009 (task derivation) ──→ INV-GUIDANCE-010 (R(t) routes derived tasks)
INV-GUIDANCE-010 (R(t) routing) ──→ INV-GUIDANCE-001 (injected in every response)

INV-BILATERAL-001 (convergence) ──→ INV-DELIBERATION-001 (deliberation convergence)
                                  ──→ INV-GUIDANCE-004 (drift detection)
                                  ──→ INV-TRILATERAL-004 (convergence monotonicity)

INV-STORE-012 (LIVE index) ──→ INV-TRILATERAL-001 (three LIVE projections)
INV-SCHEMA-001 (genesis) ──→ INV-TRILATERAL-005 (attribute namespace partition)
INV-QUERY-001 (CALM) ──→ INV-TRILATERAL-006 (Φ as Datalog program)
INV-LAYOUT-001 (content-addressed) ──→ INV-TRILATERAL-007 (unified store self-bootstrap)
```

**Dependency depth** (longest chain from leaf to root):
- Depth 0: INV-STORE-001/002/008, INV-SCHEMA-001, INV-QUERY-001
- Depth 1: INV-STORE-004–006, INV-STORE-009, INV-MERGE-001, INV-SCHEMA-002
- Depth 2: INV-MERGE-002, INV-SYNC-001, INV-BILATERAL-001, INV-HARVEST-001
- Depth 3: INV-SEED-001, INV-DELIBERATION-001, INV-GUIDANCE-001
- Depth 4: INV-BUDGET-004, INV-INTERFACE-007

This confirms the implementation order: STORE → LAYOUT → SCHEMA → QUERY → RESOLUTION →
HARVEST → SEED → MERGE → GUIDANCE → INTERFACE → TRILATERAL → SYNC → SIGNAL → BILATERAL →
DELIBERATION → BUDGET.

### §17.3 Stage Mapping

#### Stage 0 — Harvest/Seed Cycle (83 INV, core)

The foundational layer. Must be complete before any other stage.

**Namespaces fully included**: STORE (13/14 INV), LAYOUT (11/11 INV), RESOLUTION (8/8)
**Namespaces partially included**: SCHEMA (7/8, incl. 006 progressive), QUERY (10/21), HARVEST (5/9), SEED (6/8), MERGE (5/10), GUIDANCE (6/11), INTERFACE (6/10), TRILATERAL (6/7)
**Namespaces excluded**: SYNC, SIGNAL, BILATERAL, DELIBERATION, BUDGET

**Success criterion**: Work 25 turns, harvest, start fresh with seed — new session
picks up without manual re-explanation. First act: migrate SPEC.md elements as datoms.

#### Stage 1 — Budget-Aware Output + Guidance Injection (26 INV)

Builds on Stage 0 with attention budget management and enhanced guidance.

**New capabilities**: Q(t) measurement, output precedence, guidance compression,
harvest warnings, statusline bridge, significance tracking, frontier-scoped queries,
bilateral loop (basic), signal processing (confusion only), token efficiency metrics,
FP/FN calibration, crystallization guard, CLAUDE.md relevance/improvement,
betweenness centrality, HITS scoring, k-core decomposition.

**Key invariants**: INV-BUDGET-001–006, INV-GUIDANCE-003–004, INV-BILATERAL-001–002/004–005,
INV-INTERFACE-004/007, INV-QUERY-003/008–009/015–016/018, INV-SIGNAL-002,
INV-HARVEST-004/006, INV-SEED-007–008, INV-TRILATERAL-004.

#### Stage 2 — Branching + Deliberation (23 INV)

Adds isolated workspaces, competing proposals, and structured conflict resolution.

**New capabilities**: W_α working set, patch branches, branch comparison, deliberation
lifecycle, precedent queries, stability guard, lookahead via branch simulation,
bilateral symmetry, diamond lattice signal generation, projection reification,
eigenvector centrality, articulation points, topology fitness.

**Key invariants**: INV-STORE-013, INV-MERGE-003–007, INV-SCHEMA-008,
INV-DELIBERATION-001–006, INV-SIGNAL-005, INV-GUIDANCE-006/011, INV-BILATERAL-003,
INV-QUERY-004/011/019–020, INV-HARVEST-008–009.

#### Stage 3 — Multi-Agent Coordination (11 INV)

Adds multi-agent primitives: sync barriers, signal routing, subscription system.

**New capabilities**: Full sync barrier protocol, eight signal types with three-tier
routing, subscription completeness, taxonomy coverage, human signal injection,
topology-agnostic queries.

**Key invariants**: INV-SYNC-001–005, INV-SIGNAL-001/003–004/006,
INV-QUERY-010, INV-INTERFACE-006.

#### Stage 4 — Advanced Intelligence (2 INV)

Adds learned guidance, spectral authority, significance-weighted retrieval, TUI.

**New capabilities**: Learned guidance effectiveness tracking, TUI subscription liveness.

**Key invariants**: INV-GUIDANCE-005, INV-INTERFACE-005.

### §17.4 Hard Constraint Traceability

Every hard constraint (C1–C7) traces to specific invariants:

| Constraint | Description | Enforcing Invariants |
|------------|-------------|---------------------|
| C1 | Append-only store | INV-STORE-001, INV-STORE-005, NEG-STORE-001, INV-LAYOUT-002, NEG-LAYOUT-001, NEG-LAYOUT-002 |
| C2 | Identity by content | INV-STORE-002, NEG-STORE-002, INV-LAYOUT-001, INV-LAYOUT-011 |
| C3 | Schema-as-data | INV-SCHEMA-003, INV-SCHEMA-004, INV-SCHEMA-008, NEG-SCHEMA-001 |
| C4 | CRDT merge by set union | INV-STORE-003, INV-STORE-004–007, INV-MERGE-001, NEG-MERGE-001, INV-LAYOUT-004, NEG-LAYOUT-003 |
| C5 | Traceability | All elements have `Traces to` fields; INV-BILATERAL-002 (five-point coherence) |
| C6 | Falsifiability | All INVs have `Falsification` sections; structural property of the specification |
| C7 | Self-bootstrap | INV-SCHEMA-001 (genesis), INV-STORE-014 (every command is transaction), INV-BILATERAL-005 (test results as datoms), INV-LAYOUT-005 (self-verification), INV-LAYOUT-009 (index derivability), INV-TRILATERAL-007 (unified store self-bootstrap) |

### §17.5 Failure Mode Traceability

Each failure mode (FAILURE_MODES.md) maps to the DDIS/Braid mechanisms that prevent it:

| FM | Class | Preventing Invariants | Preventing ADRs |
|----|-------|-----------------------|-----------------|
| FM-001 | Knowledge loss across sessions | INV-HARVEST-001–005, INV-SEED-001–004, INV-INTERFACE-007 | ADR-HARVEST-001, ADR-SEED-001 |
| FM-002 | Provenance fabrication | INV-STORE-014 (every command is tx), INV-SIGNAL-001 (signal as datom) | ADR-STORE-008 (provenance typing) |
| FM-003 | Anchoring bias in analysis scope | INV-SEED-001 (full store query), INV-BILATERAL-003 (bilateral symmetry) | ADR-SEED-002 (priority scoring) |
| FM-004 | Cascading incompleteness | INV-BILATERAL-001 (convergence), INV-DELIBERATION-002 (stability guard) | ADR-DELIBERATION-004 (crystallization guard) |

---

## Appendix A: Element Count Summary (Complete)

| Namespace | INV | ADR | NEG | Total | Wave |
|-----------|-----|-----|-----|-------|------|
| FOUNDATION | 0  | 6   | 0   | 6     | 1    |
| STORE     | 14  | 19  | 5   | 38    | 1    |
| LAYOUT    | 11  | 7   | 5   | 23    | 1    |
| SCHEMA    | 8   | 6   | 3   | 17    | 1    |
| QUERY     | 21  | 11  | 4   | 36    | 1    |
| RESOLUTION| 8   | 13  | 3   | 24    | 1    |
| HARVEST   | 9   | 7   | 3   | 19    | 2    |
| SEED      | 8   | 7   | 2   | 17    | 2    |
| MERGE     | 10  | 7   | 3   | 20    | 2    |
| SYNC      | 5   | 3   | 2   | 10    | 2    |
| SIGNAL    | 6   | 5   | 3   | 14    | 3    |
| BILATERAL | 5   | 10  | 2   | 17    | 3    |
| DELIBERATION | 6 | 4  | 3   | 13    | 3    |
| GUIDANCE  | 11  | 9   | 3   | 23    | 3    |
| BUDGET    | 6   | 4   | 2   | 12    | 3    |
| INTERFACE | 10  | 10  | 4   | 24    | 3    |
| TRILATERAL | 7  | 3   | 3   | 13    | 4    |
| UNCERTAINTY | 0 | 4   | 0   | 4     | 4    |
| VERIFICATION | 0 | 1  | 0   | 1     | 4    |
| **Total** | **145** | **136** | **50** | **331** |      |

**Additional Wave 4 content**: 13 TRILATERAL elements (7 INV, 3 ADR, 3 NEG) (§18),
13 uncertainty entries + 4 ADR-UNCERTAINTY elements (§15),
145-row verification matrix + 1 ADR-VERIFICATION element (§16),
19-namespace cross-reference index with dependency graph and stage mapping (§17). Total: 331.

## Appendix B: Traceability Statistics

### Backward Traceability (spec → ADRS.md): 100%

All 136 spec ADR elements include `Traces to: ADRS` references linking back to the
design decisions that motivated them. 72/72 were verified during the V1 audit (R6);
3 additional ADRs added post-audit also include backward links; 45 ADRs added in the
ADR formalization pass (Session 013) all include backward links; 6 additional ADRs
formalizing Stage 0 simplification decisions (Session 014) all include backward links;
10 additional ADRs added in Phase 4 (Session 015) for LAYOUT and TRILATERAL namespaces
all include backward links.

### Forward Traceability (ADRS.md → spec): 100%

All 159 ADRS.md entries carry `Formalized as` or `Formalized across` forward annotations:

| Annotation Type | Count | Description |
|-----------------|-------|-------------|
| `Formalized as` / `Formalized across` | 159 | Entry has 1:1 or 1:N mapping to spec elements (INV, ADR, NEG) |
| `Scope` (meta-level or implementation-level) | 0 | All former Scope entries formalized in Session 013 |
| **Total** | **159** | **100% formalized** |

### Forward Annotation History

**Phase 1 (V1 Audit R6)**: 109 entries linked to spec elements; 23 annotated as
meta-level scope; 21 annotated as implementation-level scope.

**Phase 2 (ADR Formalization, Session 013)**: All 44 remaining `Scope` entries
formalized as ADR elements across 15 spec files (+3 new namespaces: FOUNDATION,
UNCERTAINTY, VERIFICATION). Total ADRs increased from 75 to 120 (+45).

**Phase 3 (Simplification Formalization, Session 014)**: 6 Stage 0 simplification
decisions formalized as ADR elements: ADR-HARVEST-007 (turn-count proxy),
ADR-GUIDANCE-008 (footer progressive enrichment), ADR-GUIDANCE-009 (betweenness
degree-product proxy), ADR-RESOLUTION-013 (conflict pipeline progressive activation),
ADR-MERGE-007 (merge cascade stub datoms), ADR-INTERFACE-010 (harvest warning
turn-count proxy). Total ADRs increased from 120 to 126 (+6).

**Phase 4 (LAYOUT + TRILATERAL, Session 015)**: 7 LAYOUT ADRs (ADR-LAYOUT-001–007)
and 3 TRILATERAL ADRs (ADR-TRILATERAL-001–003) formalized. Total spec ADRs increased
from 126 to 136 (+10).

---

## Appendix C: Verification Statistics (Final)

| Metric | Count | Coverage |
|--------|-------|----------|
| Total INVs | 145 | — |
| V:PROP | 143/145 | 98.6% |
| V:TYPE (compile-time) | 11/145 | 7.6% |
| V:PROP or V:TYPE or V:MODEL (minimum) | 145/145 | 100.0% |
| V:KANI (critical) | 48/145 | 33.1% |
| V:MODEL (protocol) | 14/145 | 9.7% |
| V:DEDUCTIVE (candidate) | 5 | Deferred to post-Stage 2 |
| Stage 0 INVs | 83 | 57.2% |
| Stage 1 INVs | 26 | 17.9% |
| Stage 2 INVs | 23 | 15.9% |
| Stage 3 INVs | 11 | 7.6% |
| Stage 4 INVs | 2 | 1.4% |
| Uncertainty markers | 15 | — |
| High-urgency uncertainties | 3 | Resolve during Stage 0 |

## Appendix D: Stage 0 Elements

Elements required for Stage 0 (Harvest/Seed cycle):

| Element | Namespace | Summary |
|---------|-----------|---------|
| INV-STORE-001–012, 014 | STORE | Core store operations (13 INV) |
| INV-LAYOUT-001–011 | LAYOUT | Content-addressed layout, isomorphism, merge, verification (11 INV) |
| INV-SCHEMA-001–007 | SCHEMA | Schema bootstrap (7 INV; 006 progressive 0–4, 008 deferred to Stage 2) |
| INV-QUERY-001–002, 005–007, 012–014, 017, 021 | QUERY | Core query + graph engine (10 INV) |
| INV-RESOLUTION-001–008 | RESOLUTION | Full conflict handling (all 8 INV) |
| INV-HARVEST-001–003, 005, 007 | HARVEST | Core harvest pipeline (5 INV; 004,006 Stage 1; 008–009 Stage 2) |
| INV-SEED-001–006 | SEED | Seed assembly pipeline (6 INV; 007–008 Stage 1) |
| INV-MERGE-001–002, 008–010 | MERGE | Core merge incl. cascade, receipt, and cascade determinism (5 INV) |
| INV-GUIDANCE-001–002, 007–010 | GUIDANCE | Injection, spec-language, dynamic CLAUDE.md, M(t), task derivation, R(t) (6 INV) |
| INV-INTERFACE-001–003, 008–010 | INTERFACE | CLI modes, MCP wrapper, tools, description quality, error recovery, CLI/MCP parity (6 INV) |
| ADR-FOUNDATION-001–006 | FOUNDATION | Project-level decisions: Braid-as-new-impl, methodology-before-tooling, D-centric formalism, DDIS formalism, structural coherence, self-bootstrap |
| ADR-STORE-001–019 | STORE | Foundation decisions (19 ADR, incl. vector-DB-rejection, JSONL-replacement, datom-exclusive) |
| ADR-LAYOUT-001–007 | LAYOUT | Layout decisions (per-txn files, content-addressed naming, EDN format, sharding, pure filesystem, O_CREAT\|O_EXCL, genesis location) |
| ADR-SCHEMA-001–006 | SCHEMA | Schema decisions (incl. value type union) |
| ADR-QUERY-001–003, 005–006, 010–011 | QUERY | Query engine decisions (incl. agent-store composition, stability score) |
| ADR-RESOLUTION-001–013 | RESOLUTION | Resolution decisions (incl. delegation, spectral authority, contribution weight, progressive activation) |
| ADR-HARVEST-001–007 | HARVEST | Harvest decisions (incl. staleness model, DDR feedback, turn-count proxy) |
| ADR-SEED-001–007 | SEED | Seed decisions (incl. four knowledge types, dynamic CLAUDE.md, eleven-section structure) |
| ADR-MERGE-001, 005–007 | MERGE | Core merge decision, cascade-as-deterministic-fixpoint, branch comparison, stub datoms |
| ADR-GUIDANCE-002, 004, 006–009 | GUIDANCE | Basin competition, spec-language, guidance graph query, S1/S2 diagnosis, footer enrichment, betweenness proxy |
| ADR-BUDGET-001–004 | BUDGET | Budget decisions (incl. chars/4 tokenization) |
| ADR-INTERFACE-001–010 | INTERFACE | Layers, agent-mode, trajectory, MCP, heuristics, ten primitives, Rust, agent cycle, staged alignment, harvest warning proxy |
| ADR-UNCERTAINTY-001–004 | UNCERTAINTY | Tensor, temporal decay, first-class markers, self-referential exclusion |
| ADR-VERIFICATION-001 | VERIFICATION | Three-tier Kani CI pipeline |
| INV-TRILATERAL-001–003, 005–007 | TRILATERAL | Trilateral coherence (6 Stage 0 INV) |
| ADR-TRILATERAL-001–002 | TRILATERAL | Unified store, EDNL interchange |
| NEG-TRILATERAL-001–003 | TRILATERAL | Trilateral safety |
| NEG-STORE-001–005 | STORE | Store safety |
| NEG-LAYOUT-001–005 | LAYOUT | Layout safety (no modification, no deletion, no append merge, no transport dependency, no index as truth) |
| NEG-SCHEMA-001–003 | SCHEMA | Schema safety |
| NEG-QUERY-001–004 | QUERY | Query safety |
| NEG-RESOLUTION-001–003 | RESOLUTION | Resolution safety |
| NEG-HARVEST-001–003 | HARVEST | Harvest safety |
| NEG-SEED-001–002 | SEED | Seed safety |
| NEG-MERGE-001, 003 | MERGE | Merge safety (no data loss, no W_α leak) |
| NEG-GUIDANCE-001 | GUIDANCE | No tool response without footer |
| NEG-INTERFACE-003–004 | INTERFACE | No harvest warning suppression, no error without recovery hint |
