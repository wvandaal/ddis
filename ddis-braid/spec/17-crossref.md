> **Section**: Cross-Reference Index & Appendices | **Wave**: 4 (Integration)
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §17. Cross-Reference Index

> **Purpose**: Maps every element to its source documents, tracks inter-invariant
> dependencies, and provides stage-based views for implementation planning.

### §17.1 Namespace → SEED.md → ADRS.md

| Namespace | SEED.md §§ | ADRS.md Categories | Primary Concerns |
|-----------|------------|---------------------|-----------------|
| STORE | §4, §9, §11 | FD-001–012, AS-001–010, SR-001–011, PD-001–004, PO-001, PO-012 | Append-only datom store, CRDT merge, content identity, HLC ordering, indexes |
| SCHEMA | §4 | SR-008–010, FD-005, FD-008 | 17 axiomatic attributes, genesis, schema-as-data, six-layer architecture |
| QUERY | §4 | FD-003, SQ-001–010, PO-013, AS-007 | Datalog, CALM, six strata, FFI boundary, significance tracking, graph engine |
| RESOLUTION | §4 | FD-005, CR-001–006 | Per-attribute resolution, conflict predicate, three-tier routing |
| HARVEST | §5 | LM-005–006, LM-012–013, IB-012, CR-005 | Epistemic gap detection, pipeline, FP/FN calibration, proactive warnings |
| SEED | §5, §8 | IB-010, PO-002–003, PO-014, GU-003–004, SQ-007 | Associate→query→assemble, dynamic CLAUDE.md, rate-distortion, projection pyramid |
| MERGE | §6 | AS-001, AS-003–006, PD-001, PD-004, PO-006–007 | Set-union merge, branching G-Set, W_α, merge cascade, competing branch lock |
| SYNC | §6 | PO-010, SQ-001, SQ-004, PD-005 | Consistent cut, barrier protocol, topology independence |
| SIGNAL | §6 | PO-004–005, PO-008, CR-002–003, AS-009, CO-003 | Eight signal types, dispatch, subscription, diamond lattice signals |
| BILATERAL | §3, §6 | CO-004, CO-008–010, SQ-006, AS-006, CO-011 | Adjunction, fitness function, five-point coherence, bilateral symmetry |
| DELIBERATION | §6 | CR-004–005, CR-007, PO-007, AS-002, AA-001 | Three entity types, stability guard, precedent, commitment weight |
| GUIDANCE | §7, §8 | GU-001–008, IB-006 | Comonad, basin competition, six anti-drift mechanisms, spec-language, M(t)/R(t)/T(t) |
| BUDGET | §8 | IB-004–007, IB-011, SQ-007 | k* measurement, Q(t), precedence, projection pyramid, rate-distortion |
| INTERFACE | §8 | IB-001–003, IB-008–009, IB-012, SR-011, AA-003 | Five layers, CLI modes, MCP tools, TUI, statusline, harvest warning |

### §17.2 Invariant Dependency Graph

Key inter-invariant dependencies (an edge A → B means B depends on A holding):

```
INV-STORE-001 (append-only) ──→ INV-MERGE-001 (merge preserves)
                              ──→ INV-HARVEST-001 (harvest commits to store)
                              ──→ INV-SCHEMA-004 (schema monotonicity)

INV-STORE-002 (content identity) ──→ INV-STORE-006 (idempotency)
                                  ──→ INV-MERGE-001 (deduplication)

INV-STORE-004/005/006 (CRDT laws) ──→ INV-MERGE-002 (merge cascade)
                                    ──→ INV-SYNC-001 (consistent cut)
                                    ──→ INV-BILATERAL-001 (convergence)

INV-STORE-008 (HLC monotonicity) ──→ INV-STORE-009 (frontier durability)
                                   ──→ INV-SYNC-003 (topology independence)

INV-SCHEMA-001 (genesis) ──→ INV-SCHEMA-002 (self-description)
                           ──→ INV-STORE-014 (every command is transaction)

INV-QUERY-001 (CALM) ──→ INV-SYNC-001 (barriers for non-monotonic)
                       ──→ INV-RESOLUTION-003 (conservative detection)

INV-RESOLUTION-004 (conflict predicate) ──→ INV-SIGNAL-004 (severity routing)
                                          ──→ INV-DELIBERATION-001 (deliberation entry)

INV-HARVEST-001 (epistemic gap) ──→ INV-SEED-001 (seed from store)
                                  ──→ INV-BILATERAL-001 (convergence)

INV-GUIDANCE-001 (continuous injection) ──→ INV-BUDGET-004 (compression by budget)
                                         ──→ INV-INTERFACE-007 (harvest warning)
                                         ──→ INV-INTERFACE-009 (error recovery)

INV-INTERFACE-003 (six MCP tools) ──→ INV-INTERFACE-008 (tool description quality)

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
```

**Dependency depth** (longest chain from leaf to root):
- Depth 0: INV-STORE-001/002/008, INV-SCHEMA-001, INV-QUERY-001
- Depth 1: INV-STORE-004–006, INV-STORE-009, INV-MERGE-001, INV-SCHEMA-002
- Depth 2: INV-MERGE-002, INV-SYNC-001, INV-BILATERAL-001, INV-HARVEST-001
- Depth 3: INV-SEED-001, INV-DELIBERATION-001, INV-GUIDANCE-001
- Depth 4: INV-BUDGET-004, INV-INTERFACE-007

This confirms the implementation order: STORE → SCHEMA → QUERY → RESOLUTION → HARVEST
→ SEED → MERGE → SYNC → SIGNAL → BILATERAL → DELIBERATION → GUIDANCE → BUDGET → INTERFACE.

### §17.3 Stage Mapping

#### Stage 0 — Harvest/Seed Cycle (61 INV, core)

The foundational layer. Must be complete before any other stage.

**Namespaces fully included**: STORE (13/14 INV), RESOLUTION (8/8)
**Namespaces partially included**: SCHEMA (7/8, incl. 006 progressive), QUERY (10/21), HARVEST (5/8), SEED (4/6), MERGE (3/8), GUIDANCE (6/11), INTERFACE (5/9)
**Namespaces excluded**: SYNC, SIGNAL, BILATERAL, DELIBERATION, BUDGET

**Success criterion**: Work 25 turns, harvest, start fresh with seed — new session
picks up without manual re-explanation. First act: migrate SPEC.md elements as datoms.

#### Stage 1 — Budget-Aware Output + Guidance Injection (25 INV)

Builds on Stage 0 with attention budget management and enhanced guidance.

**New capabilities**: Q(t) measurement, output precedence, guidance compression,
harvest warnings, statusline bridge, significance tracking, frontier-scoped queries,
bilateral loop (basic), signal processing (confusion only), token efficiency metrics,
FP/FN calibration, crystallization guard, CLAUDE.md relevance/improvement,
betweenness centrality, HITS scoring, k-core decomposition.

**Key invariants**: INV-BUDGET-001–006, INV-GUIDANCE-003–004, INV-BILATERAL-001–002/004–005,
INV-INTERFACE-004/007, INV-QUERY-003/008–009/015–016/018, INV-SIGNAL-002,
INV-HARVEST-004/006, INV-SEED-005–006.

#### Stage 2 — Branching + Deliberation (22 INV)

Adds isolated workspaces, competing proposals, and structured conflict resolution.

**New capabilities**: W_α working set, patch branches, branch comparison, deliberation
lifecycle, precedent queries, stability guard, lookahead via branch simulation,
bilateral symmetry, diamond lattice signal generation, projection reification,
eigenvector centrality, articulation points, topology fitness.

**Key invariants**: INV-STORE-013, INV-MERGE-003–007, INV-SCHEMA-008,
INV-DELIBERATION-001–006, INV-SIGNAL-005, INV-GUIDANCE-006/011, INV-BILATERAL-003,
INV-QUERY-004/011/019–020, INV-HARVEST-008.

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
| C1 | Append-only store | INV-STORE-001, INV-STORE-005, NEG-STORE-001 |
| C2 | Identity by content | INV-STORE-002, NEG-STORE-002 |
| C3 | Schema-as-data | INV-SCHEMA-003, INV-SCHEMA-004, INV-SCHEMA-008, NEG-SCHEMA-001 |
| C4 | CRDT merge by set union | INV-STORE-003, INV-STORE-004–007, INV-MERGE-001, NEG-MERGE-001 |
| C5 | Traceability | All elements have `Traces to` fields; INV-BILATERAL-002 (five-point coherence) |
| C6 | Falsifiability | All INVs have `Falsification` sections; structural property of the specification |
| C7 | Self-bootstrap | INV-SCHEMA-001 (genesis), INV-STORE-014 (every command is transaction), INV-BILATERAL-005 (test results as datoms) |

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
| STORE     | 14  | 14  | 5   | 33    | 1    |
| SCHEMA    | 8   | 5   | 3   | 16    | 1    |
| QUERY     | 21  | 9   | 4   | 34    | 1    |
| RESOLUTION| 8   | 5   | 3   | 16    | 1    |
| HARVEST   | 8   | 4   | 3   | 15    | 2    |
| SEED      | 6   | 4   | 2   | 12    | 2    |
| MERGE     | 8   | 4   | 3   | 15    | 2    |
| SYNC      | 5   | 3   | 2   | 10    | 2    |
| SIGNAL    | 6   | 3   | 3   | 12    | 3    |
| BILATERAL | 5   | 3   | 2   | 10    | 3    |
| DELIBERATION | 6 | 4  | 3   | 13    | 3    |
| GUIDANCE  | 11  | 5   | 3   | 19    | 3    |
| BUDGET    | 6   | 3   | 2   | 11    | 3    |
| INTERFACE | 9   | 4   | 4   | 17    | 3    |
| **Total** | **121** | **70** | **42** | **233** |      |

**Additional Wave 4 content**: 10 uncertainty entries (§15), 121-row verification matrix (§16),
14-namespace cross-reference index with dependency graph and stage mapping (§17).

## Appendix B: Verification Statistics (Final)

| Metric | Count | Coverage |
|--------|-------|----------|
| Total INVs | 121 | — |
| V:PROP (minimum) | 121/121 | 100.0% |
| V:KANI (critical) | 38/121 | 31.4% |
| V:MODEL (protocol) | 15/121 | 12.4% |
| V:TYPE (compile-time) | 12/121 | 9.9% |
| V:DEDUCTIVE (candidate) | 5 | Deferred to post-Stage 2 |
| Stage 0 INVs | 61 | 50.4% |
| Stage 1 INVs | 25 | 20.7% |
| Stage 2 INVs | 22 | 18.2% |
| Stage 3 INVs | 11 | 9.1% |
| Stage 4 INVs | 2 | 1.7% |
| Uncertainty markers | 10 | — |
| High-urgency uncertainties | 3 | Resolve during Stage 0 |

## Appendix C: Stage 0 Elements

Elements required for Stage 0 (Harvest/Seed cycle):

| Element | Namespace | Summary |
|---------|-----------|---------|
| INV-STORE-001–012, 014 | STORE | Core store operations (13 INV) |
| INV-SCHEMA-001–007 | SCHEMA | Schema bootstrap (7 INV; 006 progressive 0–4, 008 deferred to Stage 2) |
| INV-QUERY-001–002, 005–007, 012–014, 017, 021 | QUERY | Core query + graph engine (10 INV) |
| INV-RESOLUTION-001–008 | RESOLUTION | Full conflict handling (all 8 INV) |
| INV-HARVEST-001–003, 005, 007 | HARVEST | Core harvest pipeline (5 INV; 004,006 Stage 1; 008 Stage 2) |
| INV-SEED-001–004 | SEED | Seed assembly pipeline (4 INV) |
| INV-MERGE-001–002, 008 | MERGE | Core merge incl. cascade (3 INV) |
| INV-GUIDANCE-001–002, 007–010 | GUIDANCE | Injection, spec-language, dynamic CLAUDE.md, M(t), task derivation, R(t) (6 INV) |
| INV-INTERFACE-001–003, 008–009 | INTERFACE | CLI modes, MCP wrapper, tools, description quality, error recovery (5 INV) |
| ADR-STORE-001–014 | STORE | Foundation decisions |
| ADR-SCHEMA-001–005 | SCHEMA | Schema decisions |
| ADR-QUERY-001–003, 005–006 | QUERY | Query engine decisions |
| ADR-RESOLUTION-001–004 | RESOLUTION | Resolution decisions |
| ADR-HARVEST-001–004 | HARVEST | Harvest decisions |
| ADR-SEED-001–004 | SEED | Seed decisions |
| ADR-MERGE-001 | MERGE | Core merge decision |
| ADR-GUIDANCE-002, 004 | GUIDANCE | Basin competition, spec-language |
| ADR-INTERFACE-001–004 | INTERFACE | Layers, agent-mode, trajectory, MCP library model |
| NEG-STORE-001–005 | STORE | Store safety |
| NEG-SCHEMA-001–003 | SCHEMA | Schema safety |
| NEG-QUERY-001–004 | QUERY | Query safety |
| NEG-RESOLUTION-001–003 | RESOLUTION | Resolution safety |
| NEG-HARVEST-001–003 | HARVEST | Harvest safety |
| NEG-SEED-001–002 | SEED | Seed safety |
| NEG-MERGE-001, 003 | MERGE | Merge safety (no data loss, no W_α leak) |
| NEG-GUIDANCE-001 | GUIDANCE | No tool response without footer |
| NEG-INTERFACE-003–004 | INTERFACE | No harvest warning suppression, no error without recovery hint |
